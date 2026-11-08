//! PNG export of a Display P3 document carries the Display P3 colour profile tag and keeps the stored
//! colour numbers; an sRGB document is exported exactly as before.

use designos_doc::*;
use designos_export::{encode_png_pixmap, export_png, render_export};
use flate2::read::ZlibDecoder;
use std::io::Read;

const MID: Color = Color { r: 0.5, g: 0.4, b: 0.8, a: 1.0 };

fn doc_with(profile: ColorProfile, fill: Color) -> (Document, NodeId) {
    let mut doc = Document::new("t");
    let page = doc.add_page("p");
    let id = doc.alloc_id();
    let mut n = Node::rectangle(id, Rect::new(0.0, 0.0, 20.0, 20.0));
    n.fills = vec![Paint::solid(fill)];
    let id = doc.insert(page, n).unwrap();
    doc.color_profile = profile;
    (doc, id)
}

/// (chunk type, data) of every chunk after the 8-byte signature.
fn chunks(png: &[u8]) -> Vec<(String, Vec<u8>)> {
    assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
    let mut out = Vec::new();
    let mut i = 8;
    while i + 12 <= png.len() {
        let len = u32::from_be_bytes(png[i..i + 4].try_into().unwrap()) as usize;
        let ty = String::from_utf8_lossy(&png[i + 4..i + 8]).to_string();
        out.push((ty, png[i + 8..i + 8 + len].to_vec()));
        i += 12 + len;
    }
    out
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut c = 0xFFFF_FFFFu32;
    for b in bytes {
        c ^= *b as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
        }
    }
    !c
}

#[test]
fn a_display_p3_png_carries_cicp_and_an_embedded_profile_before_the_pixels() {
    let (doc, id) = doc_with(ColorProfile::DisplayP3, MID);
    let png = export_png(&doc, id, 1.0);
    let cs = chunks(&png);
    let names: Vec<&str> = cs.iter().map(|(n, _)| n.as_str()).collect();
    let pos = |n: &str| names.iter().position(|x| *x == n).unwrap_or_else(|| panic!("no {n} chunk in {names:?}"));
    assert_eq!(names[0], "IHDR");
    assert!(pos("cICP") < pos("IDAT") && pos("iCCP") < pos("IDAT"), "{names:?}");
    // primaries 12 = Display P3, transfer 13 = sRGB curve, matrix 0 = RGB, full range
    assert_eq!(cs[pos("cICP")].1, vec![12, 13, 0, 1]);
    let iccp = &cs[pos("iCCP")].1;
    let nul = iccp.iter().position(|b| *b == 0).unwrap();
    assert_eq!(&iccp[..nul], b"Display P3");
    assert_eq!(iccp[nul + 1], 0, "compression method 0 = zlib");
    let mut icc = Vec::new();
    ZlibDecoder::new(&iccp[nul + 2..]).read_to_end(&mut icc).unwrap();
    assert_eq!(u32::from_be_bytes(icc[0..4].try_into().unwrap()) as usize, icc.len(), "ICC header size matches the data");
    assert_eq!(&icc[12..16], b"mntr");
    assert_eq!(&icc[16..20], b"RGB ");
    assert_eq!(&icc[20..24], b"XYZ ");
    assert_eq!(&icc[36..40], b"acsp");
}

#[test]
fn every_chunk_of_a_tagged_png_has_a_valid_crc_and_the_file_decodes() {
    let (doc, id) = doc_with(ColorProfile::DisplayP3, MID);
    let png = export_png(&doc, id, 1.0);
    let mut i = 8;
    while i + 12 <= png.len() {
        let len = u32::from_be_bytes(png[i..i + 4].try_into().unwrap()) as usize;
        let stored = u32::from_be_bytes(png[i + 8 + len..i + 12 + len].try_into().unwrap());
        assert_eq!(stored, crc32(&png[i + 4..i + 8 + len]), "chunk at {i}");
        i += 12 + len;
    }
    let img = image::load_from_memory_with_format(&png, image::ImageFormat::Png).unwrap().to_rgba8();
    assert_eq!((img.width(), img.height()), (20, 20));
}

#[test]
fn a_display_p3_png_keeps_the_stored_colour_numbers() {
    // The pixels are written in the document's profile, so the tag is true: 0.5/0.4/0.8 -> 128/102/204.
    let (doc, id) = doc_with(ColorProfile::DisplayP3, MID);
    let img = image::load_from_memory(&export_png(&doc, id, 1.0)).unwrap().to_rgba8();
    let p = img.get_pixel(10, 10).0;
    assert!(p.iter().zip([128u8, 102, 204, 255]).all(|(a, b)| (*a as i32 - b as i32).abs() <= 1), "{p:?}");
}

#[test]
fn an_srgb_png_is_byte_identical_to_the_plain_encoder_and_has_no_profile_chunk() {
    let (doc, id) = doc_with(ColorProfile::Srgb, MID);
    let png = export_png(&doc, id, 1.0);
    assert_eq!(png, encode_png_pixmap(&render_export(&doc, id, 1.0).unwrap()));
    let cs = chunks(&png);
    assert!(cs.iter().all(|(n, _)| n != "iCCP" && n != "cICP" && n != "sRGB"), "{:?}", cs.iter().map(|c| &c.0).collect::<Vec<_>>());
}

#[test]
fn render_export_still_returns_srgb_pixels_for_a_display_p3_document() {
    // Pixel consumers (JPEG, WebP, screenshots) get colours converted for an sRGB display.
    let (doc, id) = doc_with(ColorProfile::DisplayP3, MID);
    let pm = render_export(&doc, id, 1.0).unwrap();
    let p = pm.pixel(10, 10).unwrap();
    assert!((p.red() as i32 - 132).abs() <= 1 && (p.green() as i32 - 101).abs() <= 1 && (p.blue() as i32 - 211).abs() <= 1, "{p:?}");
}
