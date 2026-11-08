//! Colour profile tags for PNG export.
//!
//! A Display P3 export is tagged twice, so every reader finds it: a `cICP` chunk (primaries 12 =
//! Display P3, transfer 13 = the sRGB curve) for modern decoders, and an `iCCP` chunk holding a
//! small matrix/TRC ICC profile named "Display P3" for the rest. Both describe the same space and
//! sit between `IHDR` and the first `IDAT`, where the PNG specification requires them.

use designos_doc::ColorProfile;
use flate2::write::ZlibEncoder;
use flate2::Compression;
use std::io::Write;
use std::sync::OnceLock;

/// `png` with the chunks that tag it as written in `profile`. An sRGB PNG is returned untouched.
pub(crate) fn tag_png(png: Vec<u8>, profile: ColorProfile) -> Vec<u8> {
    if profile.is_srgb() {
        return png;
    }
    // signature (8) + IHDR chunk (4 length + 4 type + 13 data + 4 crc)
    const AFTER_IHDR: usize = 8 + 25;
    if png.len() < AFTER_IHDR || &png[..8] != b"\x89PNG\r\n\x1a\n" || &png[12..16] != b"IHDR" {
        return png;
    }
    let mut out = Vec::with_capacity(png.len() + 1024);
    out.extend_from_slice(&png[..AFTER_IHDR]);
    out.extend_from_slice(&chunk(b"cICP", &[12, 13, 0, 1]));
    out.extend_from_slice(&chunk(b"iCCP", display_p3_iccp()));
    out.extend_from_slice(&png[AFTER_IHDR..]);
    out
}

fn chunk(ty: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(12 + data.len());
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    out.extend_from_slice(ty);
    out.extend_from_slice(data);
    let mut crc = Crc32::new();
    crc.update(ty);
    crc.update(data);
    out.extend_from_slice(&crc.finish().to_be_bytes());
    out
}

struct Crc32(u32);

impl Crc32 {
    fn new() -> Self {
        Self(0xFFFF_FFFF)
    }

    fn update(&mut self, bytes: &[u8]) {
        for b in bytes {
            self.0 ^= *b as u32;
            for _ in 0..8 {
                self.0 = if self.0 & 1 != 0 { 0xEDB8_8320 ^ (self.0 >> 1) } else { self.0 >> 1 };
            }
        }
    }

    fn finish(&self) -> u32 {
        !self.0
    }
}

/// Data of the `iCCP` chunk: profile name, NUL, compression method 0, zlib-compressed ICC profile.
fn display_p3_iccp() -> &'static [u8] {
    static DATA: OnceLock<Vec<u8>> = OnceLock::new();
    DATA.get_or_init(|| {
        let mut out = b"Display P3\0\0".to_vec();
        let mut z = ZlibEncoder::new(Vec::new(), Compression::best());
        // Writing into a Vec cannot fail.
        let compressed = z.write_all(&display_p3_icc_profile()).and_then(|()| z.finish());
        out.extend_from_slice(&compressed.unwrap_or_default());
        out
    })
}

fn s15(v: f64) -> [u8; 4] {
    ((v * 65536.0).round() as i32).to_be_bytes()
}

fn xyz_tag(x: f64, y: f64, z: f64) -> Vec<u8> {
    let mut t = b"XYZ \0\0\0\0".to_vec();
    for v in [x, y, z] {
        t.extend_from_slice(&s15(v));
    }
    t
}

/// ICC v2 matrix/TRC display profile for Display P3: P3 primaries adapted to the D50 profile
/// connection space (Bradford), D50 white point, and the sRGB transfer curve as a 1024-entry table.
fn display_p3_icc_profile() -> Vec<u8> {
    const TRC_ENTRIES: usize = 1024;
    let mut curve = b"curv\0\0\0\0".to_vec();
    curve.extend_from_slice(&(TRC_ENTRIES as u32).to_be_bytes());
    for i in 0..TRC_ENTRIES {
        let v = i as f64 / (TRC_ENTRIES - 1) as f64;
        let lin = if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) };
        curve.extend_from_slice(&((lin * 65535.0).round() as u16).to_be_bytes());
    }
    let text = |s: &str| {
        let mut t = b"text\0\0\0\0".to_vec();
        t.extend_from_slice(s.as_bytes());
        t.push(0);
        t
    };
    let desc = {
        let name = b"Display P3\0";
        let mut t = b"desc\0\0\0\0".to_vec();
        t.extend_from_slice(&(name.len() as u32).to_be_bytes());
        t.extend_from_slice(name);
        t.extend_from_slice(&[0; 4 + 4 + 2 + 1 + 67]); // no Unicode or ScriptCode description
        t
    };
    // Display P3 primaries in the D50 PCS (Bradford-adapted from D65).
    let tags: Vec<(&[u8; 4], Vec<u8>)> = vec![
        (b"desc", desc),
        (b"cprt", text("Copyright (c) 2026 Jang Trinh")),
        (b"wtpt", xyz_tag(0.9642, 1.0, 0.8249)),
        (b"rXYZ", xyz_tag(0.51512, 0.24119, -0.00105)),
        (b"gXYZ", xyz_tag(0.29198, 0.69224, 0.04188)),
        (b"bXYZ", xyz_tag(0.15710, 0.06657, 0.78407)),
        (b"rTRC", curve),
    ];
    let table_len = 4 + tags.len() * 12 + 2 * 12; // gTRC and bTRC reuse the rTRC data
    let mut offset = 128 + table_len;
    let mut table = (tags.len() as u32 + 2).to_be_bytes().to_vec();
    let mut data = Vec::new();
    let mut trc_at = (0usize, 0usize);
    for (sig, body) in &tags {
        let pad = (4 - body.len() % 4) % 4;
        table.extend_from_slice(*sig);
        table.extend_from_slice(&(offset as u32).to_be_bytes());
        table.extend_from_slice(&(body.len() as u32).to_be_bytes());
        if *sig == b"rTRC" {
            trc_at = (offset, body.len());
        }
        data.extend_from_slice(body);
        data.extend(std::iter::repeat_n(0u8, pad));
        offset += body.len() + pad;
    }
    for sig in [b"gTRC", b"bTRC"] {
        table.extend_from_slice(sig);
        table.extend_from_slice(&(trc_at.0 as u32).to_be_bytes());
        table.extend_from_slice(&(trc_at.1 as u32).to_be_bytes());
    }
    let mut header = vec![0u8; 128];
    let size = (128 + table.len() + data.len()) as u32;
    header[0..4].copy_from_slice(&size.to_be_bytes());
    header[8..12].copy_from_slice(&[2, 0x40, 0, 0]); // version 2.4
    header[12..16].copy_from_slice(b"mntr");
    header[16..20].copy_from_slice(b"RGB ");
    header[20..24].copy_from_slice(b"XYZ ");
    header[24..36].copy_from_slice(&[0x07, 0xE9, 0, 1, 0, 1, 0, 0, 0, 0, 0, 0]); // 2025-01-01 00:00:00
    header[36..40].copy_from_slice(b"acsp");
    header[68..80].copy_from_slice(&[s15(0.9642), s15(1.0), s15(0.8249)].concat()); // PCS illuminant D50
    let mut profile = header;
    profile.extend_from_slice(&table);
    profile.extend_from_slice(&data);
    profile
}
