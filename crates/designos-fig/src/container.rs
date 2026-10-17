//! `.fig` container: optional ZIP wrapper, `fig-kiwi` header and chunked,
//! compressed schema + message.

use crate::FigError;
use std::cell::Cell;
use std::collections::BTreeMap;
use std::io::{Cursor, Read, Seek, SeekFrom};
use std::rc::Rc;

/// Hard cap on any single decompressed chunk / zip entry (zip-bomb guard: 64 MiB).
const MAX_INFLATED: u64 = 64 << 20;
/// Cap on everything inflated while opening one archive (all entries together: 256 MiB).
const MAX_INFLATED_TOTAL: u64 = 256 << 20;
/// Cap on zip entries inspected; real files hold one entry per embedded image.
const MAX_ZIP_ENTRIES: usize = 100_000;
/// Grace window (1 MiB) before the streaming compression-ratio guard activates.
const RATIO_GRACE_BYTES: u64 = 1 << 20;
/// Maximum allowed `inflated_bytes / compressed_bytes_read` once past [`RATIO_GRACE_BYTES`].
const MAX_INFLATE_RATIO: u64 = 100;
/// Streaming read chunk size (64 KiB) so rejected streams never pre-allocate multi-GiB buffers.
const STREAM_CHUNK_BYTES: usize = 64 * 1024;

/// Reader wrapper that counts how many compressed input bytes have been pulled so far.
struct CountingReader<R> {
    inner: R,
    bytes_read: Rc<Cell<u64>>,
}

impl<R> CountingReader<R> {
    fn new(inner: R, bytes_read: Rc<Cell<u64>>) -> Self {
        Self { inner, bytes_read }
    }
}

impl<R: Read> Read for CountingReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let n = self.inner.read(buf)?;
        self.bytes_read.set(self.bytes_read.get().saturating_add(n as u64));
        Ok(n)
    }
}

impl<R: Seek> Seek for CountingReader<R> {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        self.inner.seek(pos)
    }
}

/// Inflate allowance shared by every read of one archive or container.
struct Budget {
    per_read: u64,
    left: u64,
}

impl Budget {
    fn new(per_read: u64, total: u64) -> Self {
        Self { per_read, left: total }
    }
}

pub const MAGIC_KIWI: &[u8; 8] = b"fig-kiwi";
pub const MAGIC_JAM: &[u8; 8] = b"fig-jam.";

#[derive(Debug, Default)]
pub struct Archive {
    /// Raw `canvas.fig` bytes (the fig-kiwi container).
    pub canvas: Vec<u8>,
    /// `images/<hash>` entries keyed by file name.
    pub images: BTreeMap<String, Vec<u8>>,
    pub meta: Option<serde_json::Value>,
    /// Inflate allowance left after opening the zip, so [`read_archive_chunks`] continues the same budget.
    /// `None` for a bare container (nothing was inflated yet).
    inflate_left: Option<u64>,
    /// Per-read inflate cap the archive was opened with; `None` uses [`MAX_INFLATED`].
    inflate_per_read: Option<u64>,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Compression {
    Deflate,
    Zstd,
}

/// Opens either a ZIP-wrapped `.fig` or a bare fig-kiwi container.
pub fn open(bytes: &[u8]) -> Result<Archive, FigError> {
    if bytes.starts_with(b"PK") {
        return open_zip(bytes);
    }
    if bytes.starts_with(MAGIC_KIWI) || bytes.starts_with(MAGIC_JAM) {
        return Ok(Archive { canvas: bytes.to_vec(), ..Default::default() });
    }
    Err(FigError::Format("missing fig-kiwi magic or zip header".into()))
}

/// Like [`open`], but every inflate (zip entries and the chunks read later by [`read_archive_chunks`]) draws on
/// one allowance of `total` bytes, no single read exceeding `per_read`. For untrusted payloads smaller than a
/// whole `.fig` file, such as a clipboard.
pub fn open_with_budget(bytes: &[u8], per_read: u64, total: u64) -> Result<Archive, FigError> {
    let mut budget = Budget::new(per_read, total);
    let mut archive = if bytes.starts_with(b"PK") { open_zip_with(bytes, &mut budget, MAX_ZIP_ENTRIES)? } else { open(bytes)? };
    archive.inflate_left = Some(budget.left);
    archive.inflate_per_read = Some(per_read);
    Ok(archive)
}

fn read_limited(mut r: impl Read, budget: &mut Budget, compressed_read: Option<&Cell<u64>>) -> Result<Vec<u8>, FigError> {
    let cap = budget.per_read.min(budget.left);
    let mut out = Vec::new();
    let mut chunk = [0u8; STREAM_CHUNK_BYTES];
    loop {
        let n = r.read(&mut chunk).map_err(|e| FigError::Format(format!("read error: {e}")))?;
        if n == 0 {
            break;
        }
        let next_len = (out.len() + n) as u64;
        if next_len > cap {
            return Err(FigError::Format("decompressed data too large".into()));
        }
        if next_len > RATIO_GRACE_BYTES {
            if let Some(comp_cell) = compressed_read {
                let comp = comp_cell.get().max(1);
                if next_len / comp > MAX_INFLATE_RATIO {
                    return Err(FigError::Format("decompressed data too large (compression ratio exceeded)".into()));
                }
            }
        }
        out.extend_from_slice(&chunk[..n]);
    }
    budget.left -= out.len() as u64;
    Ok(out)
}

fn open_zip(bytes: &[u8]) -> Result<Archive, FigError> {
    let mut budget = Budget::new(MAX_INFLATED, MAX_INFLATED_TOTAL);
    let mut archive = open_zip_with(bytes, &mut budget, MAX_ZIP_ENTRIES)?;
    archive.inflate_left = Some(budget.left);
    Ok(archive)
}

fn open_zip_with(bytes: &[u8], budget: &mut Budget, max_entries: usize) -> Result<Archive, FigError> {
    let compressed_read = Rc::new(Cell::new(0u64));
    let reader = CountingReader::new(Cursor::new(bytes), Rc::clone(&compressed_read));
    let mut zip = zip::ZipArchive::new(reader).map_err(|e| FigError::Format(format!("zip: {e}")))?;
    if zip.len() > max_entries {
        return Err(FigError::Format(format!("zip has too many entries ({})", zip.len())));
    }
    let mut arch = Archive::default();
    let mut have_canvas = false;
    for i in 0..zip.len() {
        let Ok(file) = zip.by_index(i) else { continue };
        if file.is_dir() {
            continue;
        }
        let Ok(name) = file.name().map(|n| n.replace('\\', "/")) else { continue };
        let cap = budget.per_read.min(budget.left);
        let declared_too_large = file.size() > cap
            || (file.size() > RATIO_GRACE_BYTES && file.compressed_size() > 0 && file.size() / file.compressed_size().max(1) > MAX_INFLATE_RATIO);
        if name == "canvas.fig" || name.ends_with("/canvas.fig") {
            if declared_too_large {
                return Err(FigError::Format("decompressed data too large".into()));
            }
            compressed_read.set(0);
            arch.canvas = read_limited(file, budget, Some(&compressed_read))?;
            have_canvas = true;
        } else if name == "meta.json" {
            if !declared_too_large {
                compressed_read.set(0);
                if let Ok(data) = read_limited(file, budget, Some(&compressed_read)) {
                    arch.meta = serde_json::from_slice(&data).ok();
                }
            }
        } else if let Some(h) = name.strip_prefix("images/") {
            if !h.is_empty() && !h.contains('/') && !declared_too_large {
                compressed_read.set(0);
                if let Ok(data) = read_limited(file, budget, Some(&compressed_read)) {
                    arch.images.insert(h.to_string(), data);
                }
            }
        }
    }
    if !have_canvas {
        return Err(FigError::Format("zip has no canvas.fig".into()));
    }
    Ok(arch)
}

/// Splits a fig-kiwi container into decompressed `(schema, message)` chunks.
///
/// Starts from a fresh inflate allowance; use [`read_archive_chunks`] for an opened [`Archive`] so the zip
/// entries and the chunks of one file draw on a single budget.
pub fn read_chunks(canvas: &[u8]) -> Result<(Vec<u8>, Vec<u8>), FigError> {
    read_chunks_with(canvas, &mut Budget::new(MAX_INFLATED, MAX_INFLATED_TOTAL))
}

/// Splits the canvas of an opened archive, continuing the inflate budget that opening it already drew on.
pub fn read_archive_chunks(archive: &Archive) -> Result<(Vec<u8>, Vec<u8>), FigError> {
    read_chunks_with(&archive.canvas, &mut Budget::new(archive.inflate_per_read.unwrap_or(MAX_INFLATED), archive.inflate_left.unwrap_or(MAX_INFLATED_TOTAL)))
}

fn read_chunks_with(canvas: &[u8], budget: &mut Budget) -> Result<(Vec<u8>, Vec<u8>), FigError> {
    if canvas.len() < 12 || !(canvas.starts_with(MAGIC_KIWI) || canvas.starts_with(MAGIC_JAM)) {
        return Err(FigError::Format("not a fig-kiwi container".into()));
    }
    let mut pos = 12; // magic + u32 version
    let mut chunks: Vec<&[u8]> = Vec::new();
    while pos + 4 <= canvas.len() && chunks.len() < 8 {
        let len = u32::from_le_bytes([canvas[pos], canvas[pos + 1], canvas[pos + 2], canvas[pos + 3]]) as usize;
        pos += 4;
        let end = pos.checked_add(len).filter(|&e| e <= canvas.len());
        let Some(end) = end else {
            return Err(FigError::Format(format!("chunk {} is truncated", chunks.len())));
        };
        chunks.push(&canvas[pos..end]);
        pos = end;
    }
    if chunks.len() < 2 {
        return Err(FigError::Format("container has fewer than two chunks".into()));
    }
    Ok((decompress_with(chunks[0], budget)?, decompress_with(chunks[1], budget)?))
}

pub fn container_version(canvas: &[u8]) -> Option<u32> {
    let b = canvas.get(8..12)?;
    Some(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

/// Detects zstd by magic, otherwise raw DEFLATE (falling back to zlib wrapping).
pub fn decompress(data: &[u8]) -> Result<Vec<u8>, FigError> {
    decompress_with(data, &mut Budget::new(MAX_INFLATED, MAX_INFLATED_TOTAL))
}

fn decompress_with(data: &[u8], budget: &mut Budget) -> Result<Vec<u8>, FigError> {
    let compressed_read = Rc::new(Cell::new(0u64));
    if data.starts_with(&[0x28, 0xB5, 0x2F, 0xFD]) {
        let reader = CountingReader::new(Cursor::new(data), Rc::clone(&compressed_read));
        let dec = ruzstd::decoding::StreamingDecoder::new(reader).map_err(|e| FigError::Format(format!("zstd: {e}")))?;
        return read_limited(dec, budget, Some(&compressed_read));
    }
    let reader = CountingReader::new(Cursor::new(data), Rc::clone(&compressed_read));
    match read_limited(flate2::read::DeflateDecoder::new(reader), budget, Some(&compressed_read)) {
        Ok(v) => Ok(v),
        Err(e) if e.to_string().contains("decompressed data too large") => Err(e),
        Err(e) => {
            // Some producers use zlib framing.
            compressed_read.set(0);
            let reader = CountingReader::new(Cursor::new(data), Rc::clone(&compressed_read));
            read_limited(flate2::read::ZlibDecoder::new(reader), budget, Some(&compressed_read)).map_err(|_| e)
        }
    }
}

pub fn compress(data: &[u8], how: Compression) -> Vec<u8> {
    use std::io::Write;
    match how {
        Compression::Deflate => {
            let mut e = flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::fast());
            let _ = e.write_all(data);
            e.finish().unwrap_or_default()
        }
        Compression::Zstd => ruzstd::encoding::compress_to_vec(data, ruzstd::encoding::CompressionLevel::Fastest),
    }
}

/// Builds a fig-kiwi container (header, version, schema chunk, message chunk).
pub fn write_container(schema: &[u8], message: &[u8], how: Compression, version: u32) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(MAGIC_KIWI);
    out.extend_from_slice(&version.to_le_bytes());
    for chunk in [schema, message] {
        let c = compress(chunk, how);
        out.extend_from_slice(&(c.len() as u32).to_le_bytes());
        out.extend_from_slice(&c);
    }
    out
}

/// Wraps a canvas and images into the ZIP layout Figma uses for "Save local copy".
pub fn write_zip(canvas: &[u8], images: &BTreeMap<String, Vec<u8>>, meta: Option<&serde_json::Value>) -> Vec<u8> {
    use std::io::Write;
    let mut w = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    let _ = w.start_file("canvas.fig", opts);
    let _ = w.write_all(canvas);
    if let Some(m) = meta {
        let _ = w.start_file("meta.json", opts);
        let _ = w.write_all(m.to_string().as_bytes());
    }
    for (k, v) in images {
        let _ = w.start_file(format!("images/{k}"), opts);
        let _ = w.write_all(v);
    }
    w.finish().map(|c| c.into_inner()).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compression_roundtrips() {
        let data: Vec<u8> = (0..5000u32).map(|i| (i % 251) as u8).collect();
        for how in [Compression::Deflate, Compression::Zstd] {
            assert_eq!(decompress(&compress(&data, how)).unwrap(), data, "{how:?}");
        }
    }

    #[test]
    fn container_roundtrip_and_zip() {
        let c = write_container(b"schema", b"message", Compression::Zstd, 15);
        assert_eq!(container_version(&c), Some(15));
        let (s, m) = read_chunks(&c).unwrap();
        assert_eq!((s.as_slice(), m.as_slice()), (&b"schema"[..], &b"message"[..]));
        let mut imgs = BTreeMap::new();
        imgs.insert("abcd".to_string(), vec![1, 2, 3]);
        let z = write_zip(&c, &imgs, Some(&serde_json::json!({"file_name": "x"})));
        let a = open(&z).unwrap();
        assert_eq!(a.canvas, c);
        assert_eq!(a.images["abcd"], vec![1, 2, 3]);
        assert_eq!(a.meta.unwrap()["file_name"], "x");
    }

    fn zip_of(entries: &[(&str, Vec<u8>)]) -> Vec<u8> {
        use std::io::Write;
        let mut w = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        for (name, data) in entries {
            w.start_file(*name, opts).unwrap();
            w.write_all(data).unwrap();
        }
        w.finish().unwrap().into_inner()
    }

    #[test]
    fn zip_total_inflate_budget_spans_entries() {
        // Each entry fits the per-read cap, but together they exceed the archive budget.
        let canvas = write_container(b"s", b"m", Compression::Deflate, 1);
        let zero = vec![0u8; 3000];
        let z = zip_of(&[("canvas.fig", canvas), ("images/a", zero.clone()), ("images/b", zero.clone()), ("images/c", zero)]);
        let mut roomy = Budget::new(4096, 1 << 20);
        assert_eq!(open_zip_with(&z, &mut roomy, 16).unwrap().images.len(), 3);
        let mut tight = Budget::new(4096, 7000);
        let a = open_zip_with(&z, &mut tight, 16).unwrap();
        assert!(a.images.len() < 3, "images past the shared budget must be dropped, got {}", a.images.len());
        let mut tiny = Budget::new(4096, 10);
        assert!(open_zip_with(&z, &mut tiny, 16).is_err(), "canvas over budget is an error");
    }

    #[test]
    fn chunk_inflation_shares_the_archive_budget() {
        // The container's two chunks each fit the per-read cap, but together with the zip entry they exceed
        // the one budget the whole file is allowed.
        let zero = vec![0u8; 3000];
        let canvas = write_container(&zero, &zero, Compression::Deflate, 1);
        let z = zip_of(&[("canvas.fig", canvas)]);

        let mut shared = Budget::new(4096, 5000);
        let archive = open_zip_with(&z, &mut shared, 16).unwrap();
        assert!(read_chunks_with(&archive.canvas, &mut shared).is_err(), "chunks must draw on what the archive left");
        // A fresh allowance (the old behaviour) would have accepted the same file.
        assert!(read_chunks(&archive.canvas).is_ok());
        let mut roomy = Budget::new(4096, 1 << 20);
        let archive = open_zip_with(&z, &mut roomy, 16).unwrap();
        assert!(read_chunks_with(&archive.canvas, &mut roomy).is_ok());
    }

    #[test]
    fn archive_records_what_opening_it_inflated() {
        let c = write_container(b"schema", b"message", Compression::Deflate, 15);
        let z = write_zip(&c, &BTreeMap::new(), None);
        let a = open(&z).unwrap();
        assert!(a.inflate_left.is_some_and(|left| left < MAX_INFLATED_TOTAL), "{:?}", a.inflate_left);
        let (s, m) = read_archive_chunks(&a).unwrap();
        assert_eq!((s.as_slice(), m.as_slice()), (&b"schema"[..], &b"message"[..]));
        // A bare container has no archive budget to continue from and starts with the full allowance.
        let bare = open(&c).unwrap();
        assert_eq!(bare.inflate_left, None);
        assert!(read_archive_chunks(&bare).is_ok());
    }

    #[test]
    fn explicit_budget_covers_zip_entries_and_chunks_of_a_bare_container_alike() {
        let zero = vec![0u8; 3000];
        let bare = write_container(&zero, &zero, Compression::Deflate, 1);
        let zipped = write_zip(&bare, &BTreeMap::new(), None);
        for bytes in [&bare, &zipped] {
            let roomy = open_with_budget(bytes, 4096, 1 << 20).unwrap();
            assert!(read_archive_chunks(&roomy).is_ok());
            // each chunk fits the per-read cap, the two together exceed the allowance
            let tight = open_with_budget(bytes, 4096, 5000).unwrap();
            assert!(read_archive_chunks(&tight).is_err(), "chunks draw on the explicit allowance");
            let per_read = open_with_budget(bytes, 2000, 1 << 20).unwrap();
            assert!(read_archive_chunks(&per_read).is_err(), "the per-read cap applies to chunks too");
        }
        assert!(open_with_budget(&zipped, 4096, 10).is_err(), "a zip canvas over the allowance fails at open");
    }

    #[test]
    fn zip_entry_count_is_capped() {
        let canvas = write_container(b"s", b"m", Compression::Deflate, 1);
        let mut entries = vec![("canvas.fig".to_string(), canvas)];
        entries.extend((0..20).map(|i| (format!("images/{i}"), vec![1u8])));
        let refs: Vec<(&str, Vec<u8>)> = entries.iter().map(|(n, d)| (n.as_str(), d.clone())).collect();
        let z = zip_of(&refs);
        assert!(open_zip_with(&z, &mut Budget::new(1 << 20, 1 << 20), 10).is_err());
        assert!(open_zip_with(&z, &mut Budget::new(1 << 20, 1 << 20), 100).is_ok());
    }

    #[test]
    fn garbage_is_an_error_not_a_panic() {
        assert!(open(b"").is_err());
        assert!(open(b"PK\x03\x04junk").is_err());
        let c = write_container(b"s", b"m", Compression::Deflate, 1);
        for n in 0..c.len() {
            let _ = read_chunks(&c[..n]);
        }
        assert!(decompress(&[0x28, 0xB5, 0x2F, 0xFD, 1, 2, 3]).is_err());
    }

    /// Builds a valid raw DEFLATE stream that inflates to `(2 GiB + 1)` bytes of zeros (~9.4 MB compressed)
    /// in milliseconds by repeating a flushed 1 MiB zero block 2048 times.
    fn deflate_bomb_2gib_plus_one() -> Vec<u8> {
        use std::io::Write;
        let one_mib = vec![0u8; 1 << 20];
        let mut enc = flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
        enc.write_all(&one_mib).unwrap();
        enc.flush().unwrap();
        let first_block = enc.get_ref().clone();
        enc.write_all(&one_mib).unwrap();
        enc.flush().unwrap();
        let repeat_block = enc.get_ref()[first_block.len()..].to_vec();
        let mut tail_enc = flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
        tail_enc.write_all(&[0u8]).unwrap();
        let tail_block = tail_enc.finish().unwrap();

        let mut out = Vec::with_capacity(first_block.len() + repeat_block.len() * 2047 + tail_block.len());
        out.extend_from_slice(&first_block);
        for _ in 0..2047 {
            out.extend_from_slice(&repeat_block);
        }
        out.extend_from_slice(&tail_block);
        out
    }

    /// Wraps a raw DEFLATE stream as `canvas.fig` inside a minimal Zip64 PKZIP archive.
    fn wrap_raw_deflate_in_zip(entry_name: &str, raw_deflate: &[u8], declared_uncompressed: u64) -> Vec<u8> {
        let name_bytes = entry_name.as_bytes();
        let comp_size = raw_deflate.len() as u64;
        // Zip64 extra field (header ID 0x0001, data size 16: 8B uncompressed + 8B compressed)
        let mut extra = Vec::with_capacity(20);
        extra.extend_from_slice(&0x0001u16.to_le_bytes());
        extra.extend_from_slice(&16u16.to_le_bytes());
        extra.extend_from_slice(&declared_uncompressed.to_le_bytes());
        extra.extend_from_slice(&comp_size.to_le_bytes());

        let mut zip = Vec::with_capacity(raw_deflate.len() + 256);
        // Local file header (PK\x03\x04)
        zip.extend_from_slice(b"PK\x03\x04");
        zip.extend_from_slice(&45u16.to_le_bytes()); // version needed (4.5 for Zip64)
        zip.extend_from_slice(&0u16.to_le_bytes()); // flags
        zip.extend_from_slice(&8u16.to_le_bytes()); // method = 8 (Deflate)
        zip.extend_from_slice(&0u16.to_le_bytes()); // mod time
        zip.extend_from_slice(&0u16.to_le_bytes()); // mod date
        zip.extend_from_slice(&0u32.to_le_bytes()); // crc32 (checked only at EOF)
        zip.extend_from_slice(&0xFFFF_FFFFu32.to_le_bytes());
        zip.extend_from_slice(&0xFFFF_FFFFu32.to_le_bytes());
        zip.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
        zip.extend_from_slice(&(extra.len() as u16).to_le_bytes());
        zip.extend_from_slice(name_bytes);
        zip.extend_from_slice(&extra);
        zip.extend_from_slice(raw_deflate);

        let cd_offset = zip.len() as u32;
        // Central directory header (PK\x01\x02)
        zip.extend_from_slice(b"PK\x01\x02");
        zip.extend_from_slice(&45u16.to_le_bytes()); // version made by
        zip.extend_from_slice(&45u16.to_le_bytes()); // version needed
        zip.extend_from_slice(&0u16.to_le_bytes()); // flags
        zip.extend_from_slice(&8u16.to_le_bytes()); // method = 8 (Deflate)
        zip.extend_from_slice(&0u16.to_le_bytes()); // mod time
        zip.extend_from_slice(&0u16.to_le_bytes()); // mod date
        zip.extend_from_slice(&0u32.to_le_bytes()); // crc32
        zip.extend_from_slice(&0xFFFF_FFFFu32.to_le_bytes());
        zip.extend_from_slice(&0xFFFF_FFFFu32.to_le_bytes());
        zip.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
        zip.extend_from_slice(&(extra.len() as u16).to_le_bytes());
        zip.extend_from_slice(&0u16.to_le_bytes()); // comment len
        zip.extend_from_slice(&0u16.to_le_bytes()); // disk number
        zip.extend_from_slice(&0u16.to_le_bytes()); // internal attrs
        zip.extend_from_slice(&0u32.to_le_bytes()); // external attrs
        zip.extend_from_slice(&0u32.to_le_bytes()); // local header offset
        zip.extend_from_slice(name_bytes);
        zip.extend_from_slice(&extra);
        let cd_size = (zip.len() as u32) - cd_offset;

        // End of central directory (PK\x05\x06)
        zip.extend_from_slice(b"PK\x05\x06");
        zip.extend_from_slice(&0u16.to_le_bytes()); // disk
        zip.extend_from_slice(&0u16.to_le_bytes()); // cd start disk
        zip.extend_from_slice(&1u16.to_le_bytes()); // entries on disk
        zip.extend_from_slice(&1u16.to_le_bytes()); // total entries
        zip.extend_from_slice(&cd_size.to_le_bytes());
        zip.extend_from_slice(&cd_offset.to_le_bytes());
        zip.extend_from_slice(&0u16.to_le_bytes()); // comment len
        zip
    }

    #[cfg(unix)]
    fn peak_rss_bytes() -> u64 {
        #[repr(C)]
        struct Timeval {
            tv_sec: isize,
            tv_usec: isize,
        }
        #[repr(C)]
        struct RUsage {
            ru_utime: Timeval,
            ru_stime: Timeval,
            ru_maxrss: isize,
            ru_ixrss: isize,
            ru_idrss: isize,
            ru_isrss: isize,
            ru_minflt: isize,
            ru_majflt: isize,
            ru_nswap: isize,
            ru_inblock: isize,
            ru_oublock: isize,
            ru_msgsnd: isize,
            ru_msgrcv: isize,
            ru_nsignals: isize,
            ru_nvcsw: isize,
            ru_nivcsw: isize,
        }
        unsafe extern "C" {
            fn getrusage(who: i32, usage: *mut RUsage) -> i32;
        }
        let mut usage = std::mem::MaybeUninit::<RUsage>::zeroed();
        let rc = unsafe { getrusage(0, usage.as_mut_ptr()) };
        if rc != 0 {
            return 0;
        }
        let raw = unsafe { usage.assume_init().ru_maxrss } as u64;
        if cfg!(target_os = "macos") {
            raw
        } else {
            raw * 1024
        }
    }

    #[test]
    fn zip_bomb_stream_rejected_under_100mb_rss() {
        let raw_deflate = deflate_bomb_2gib_plus_one();
        // ~2.1 MB compressed DEFLATE stream that inflates to 2 GiB + 1 byte of zeros.
        assert!(raw_deflate.len() > 1_000_000 && raw_deflate.len() < 12_000_000, "compressed len={}", raw_deflate.len());
        let bomb_zip = wrap_raw_deflate_in_zip("canvas.fig", &raw_deflate, (2u64 << 30) + 1);

        let err = open(&bomb_zip).expect_err("2 GiB + 1 zip bomb must be rejected");
        assert!(err.to_string().contains("decompressed data too large"), "unexpected error: {err}");

        // Even with the old 2 GiB / 4 GiB budget and a declared size under the cap, streaming ratio guard rejects early.
        let within_cap_zip = wrap_raw_deflate_in_zip("canvas.fig", &raw_deflate, 2u64 << 30);
        let err_ratio = open_with_budget(&within_cap_zip, 2 << 30, 4 << 30).expect_err("streaming ratio guard must reject");
        assert!(err_ratio.to_string().contains("decompressed data too large"), "unexpected error: {err_ratio}");

        let err2 = decompress(&raw_deflate).expect_err("raw 2 GiB + 1 deflate bomb must be rejected");
        assert!(err2.to_string().contains("decompressed data too large"), "unexpected error: {err2}");

        #[cfg(unix)]
        {
            let rss = peak_rss_bytes();
            assert!(rss < 100 << 20, "peak RSS reached {:.1} MiB (must stay under 100 MiB when rejecting 2 GiB zip bomb)", rss as f64 / (1024.0 * 1024.0));
        }
    }
}
