//! `.fig` container: optional ZIP wrapper, `fig-kiwi` header and chunked,
//! compressed schema + message.

use crate::FigError;
use std::collections::BTreeMap;
use std::io::{Cursor, Read};

/// Hard cap on any single decompressed chunk / zip entry (zip-bomb guard: 2 GiB).
const MAX_INFLATED: u64 = 2 << 30;
/// Cap on everything inflated while opening one archive (all entries together: 4 GiB).
const MAX_INFLATED_TOTAL: u64 = 4 << 30;
/// Cap on zip entries inspected; real files hold one entry per embedded image.
const MAX_ZIP_ENTRIES: usize = 100_000;
/// Output up to this size (16 MiB) is never judged by compression ratio.
const RATIO_GRACE_BYTES: u64 = 16 << 20;
/// Largest accepted `inflated / compressed` ratio of a whole stream, enforced past [`RATIO_GRACE_BYTES`].
const MAX_INFLATE_RATIO: u64 = 100;
/// Streaming read chunk size (64 KiB) so rejected streams never pre-allocate multi-GiB buffers.
const STREAM_CHUNK_BYTES: usize = 64 * 1024;
const TOO_LARGE: &str = "decompressed data too large";
const RATIO_EXCEEDED: &str = "decompressed data too large (compression ratio exceeded)";

/// Why a bounded inflate stopped early.
enum InflateError {
    /// The output crossed the size cap or the compression-ratio limit: the input is oversized or hostile.
    Limit(&'static str),
    /// The stream itself could not be read (bad checksum, truncated or corrupt data).
    Unreadable(String),
}

impl From<InflateError> for FigError {
    fn from(e: InflateError) -> Self {
        match e {
            InflateError::Limit(msg) => FigError::Format(msg.into()),
            InflateError::Unreadable(msg) => FigError::Format(msg),
        }
    }
}

/// The most output that `compressed` input may produce before it counts as a bomb: anything up to
/// [`RATIO_GRACE_BYTES`], otherwise just under [`MAX_INFLATE_RATIO`] + 1 times the input. With no input to compare
/// with (`compressed == 0`) there is no ratio to judge.
///
/// A decoder emits output in order, so a stream can start far denser than it is overall; judging the output so
/// far against the input read so far would reject such a stream whose whole ratio is fine. Judging it against
/// the whole input is safe to do early, because the unread rest of the stream can only add output and never
/// bring the ratio back down.
fn ratio_allowance(compressed: u64) -> u64 {
    if compressed == 0 {
        return u64::MAX;
    }
    RATIO_GRACE_BYTES.max(compressed.saturating_mul(MAX_INFLATE_RATIO + 1) - 1)
}

/// True once `inflated` output is more than [`MAX_INFLATE_RATIO`] times the whole `compressed` input.
fn ratio_exceeded(inflated: u64, compressed: u64) -> bool {
    inflated > ratio_allowance(compressed)
}

/// Inflate allowance shared by every read of one archive or container.
struct Budget {
    per_read: u64,
    left: u64,
    /// Output the whole file may still produce before it counts as a bomb (see [`ratio_allowance`]); entries that
    /// each pass the ratio check on their own draw on it together. `u64::MAX` when no file length is known.
    ratio_room: u64,
}

impl Budget {
    fn new(per_read: u64, total: u64) -> Self {
        Self { per_read, left: total, ratio_room: u64::MAX }
    }
}

pub const MAGIC_KIWI: &[u8; 8] = b"fig-kiwi";
pub const MAGIC_JAM: &[u8; 8] = b"fig-jam.";

#[derive(Debug, Default)]
pub struct Archive {
    /// Raw `canvas.fig` bytes (the fig-kiwi container).
    pub canvas: Vec<u8>,
    /// `images/<hash>` entries keyed by file name. A corrupt entry is left out; one past the size or ratio limits fails the open.
    pub images: BTreeMap<String, Vec<u8>>,
    pub meta: Option<serde_json::Value>,
    /// Inflate allowance left after opening the zip, so [`read_archive_chunks`] continues the same budget.
    /// `None` for a bare container (nothing was inflated yet).
    inflate_left: Option<u64>,
    /// Per-read inflate cap the archive was opened with; `None` uses [`MAX_INFLATED`].
    inflate_per_read: Option<u64>,
    /// What is left of the file-wide ratio room (see [`Budget::ratio_room`]) after opening the zip, so the chunks
    /// are judged against the size of the whole file too. `None` for a bare container.
    inflate_ratio_room: Option<u64>,
}

impl Archive {
    /// Records what is left of `budget` after opening, for [`read_archive_chunks`] to continue.
    fn keep_budget(&mut self, budget: &Budget) {
        self.inflate_left = Some(budget.left);
        self.inflate_ratio_room = Some(budget.ratio_room);
    }
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
    archive.keep_budget(&budget);
    archive.inflate_per_read = Some(per_read);
    Ok(archive)
}

/// Inflates `r` into memory in [`STREAM_CHUNK_BYTES`] steps and stops as soon as the size cap or a ratio limit
/// is crossed, so a bomb is never buffered up to the cap. `compressed_len` is the length of the whole compressed
/// input behind `r` (see [`ratio_allowance`]); the budget carries the same limit for the file as a whole.
fn read_limited(mut r: impl Read, budget: &mut Budget, compressed_len: u64) -> Result<Vec<u8>, InflateError> {
    let cap = budget.per_read.min(budget.left);
    let mut out = Vec::new();
    let mut chunk = [0u8; STREAM_CHUNK_BYTES];
    loop {
        let n = r.read(&mut chunk).map_err(|e| InflateError::Unreadable(format!("read error: {e}")))?;
        if n == 0 {
            break;
        }
        let next_len = (out.len() + n) as u64;
        if next_len > cap {
            return Err(InflateError::Limit(TOO_LARGE));
        }
        if ratio_exceeded(next_len, compressed_len) || next_len > budget.ratio_room {
            return Err(InflateError::Limit(RATIO_EXCEEDED));
        }
        out.extend_from_slice(&chunk[..n]);
    }
    budget.left -= out.len() as u64;
    budget.ratio_room = budget.ratio_room.saturating_sub(out.len() as u64);
    Ok(out)
}

fn open_zip(bytes: &[u8]) -> Result<Archive, FigError> {
    let mut budget = Budget::new(MAX_INFLATED, MAX_INFLATED_TOTAL);
    let mut archive = open_zip_with(bytes, &mut budget, MAX_ZIP_ENTRIES)?;
    archive.keep_budget(&budget);
    Ok(archive)
}

fn open_zip_with(bytes: &[u8], budget: &mut Budget, max_entries: usize) -> Result<Archive, FigError> {
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|e| FigError::Format(format!("zip: {e}")))?;
    if zip.len() > max_entries {
        return Err(FigError::Format(format!("zip has too many entries ({})", zip.len())));
    }
    // Every entry is judged against its own compressed size, and all of them together against the size of the file:
    // many entries that each stay under the grace window must not add up to a bomb.
    budget.ratio_room = budget.ratio_room.min(ratio_allowance(bytes.len() as u64));
    let mut arch = Archive::default();
    let mut have_canvas = false;
    for i in 0..zip.len() {
        let Ok(file) = zip.by_index(i) else { continue };
        if file.is_dir() {
            continue;
        }
        let Ok(name) = file.name().map(|n| n.replace('\\', "/")) else { continue };
        let is_canvas = name == "canvas.fig" || name.ends_with("/canvas.fig");
        let image_hash = name.strip_prefix("images/").filter(|h| !h.is_empty() && !h.contains('/'));
        if !is_canvas && name != "meta.json" && image_hash.is_none() {
            continue;
        }
        // The header's compressed size is attacker-controlled; the entry cannot hold more than what follows its data offset.
        let in_archive = file.data_start().map_or(bytes.len() as u64, |start| (bytes.len() as u64).saturating_sub(start));
        let compressed_len = file.compressed_size().min(in_archive);
        if file.size() > budget.per_read.min(budget.left) {
            return Err(FigError::Format(TOO_LARGE.into()));
        }
        if file.size() > budget.ratio_room || ratio_exceeded(file.size(), compressed_len) {
            return Err(FigError::Format(RATIO_EXCEEDED.into()));
        }
        let data = match read_limited(file, budget, compressed_len) {
            Ok(data) => data,
            Err(InflateError::Limit(msg)) => return Err(FigError::Format(msg.into())),
            // Only the canvas is indispensable: an image or meta.json that cannot be read is left out of the archive.
            Err(e @ InflateError::Unreadable(_)) if is_canvas => return Err(e.into()),
            Err(InflateError::Unreadable(_)) => continue,
        };
        if is_canvas {
            arch.canvas = data;
            have_canvas = true;
        } else if let Some(hash) = image_hash {
            arch.images.insert(hash.to_string(), data);
        } else {
            arch.meta = serde_json::from_slice(&data).ok();
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
    let mut budget = Budget::new(archive.inflate_per_read.unwrap_or(MAX_INFLATED), archive.inflate_left.unwrap_or(MAX_INFLATED_TOTAL));
    budget.ratio_room = archive.inflate_ratio_room.unwrap_or(u64::MAX);
    read_chunks_with(&archive.canvas, &mut budget)
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
    let compressed_len = data.len() as u64;
    if data.starts_with(&[0x28, 0xB5, 0x2F, 0xFD]) {
        let dec = ruzstd::decoding::StreamingDecoder::new(data).map_err(|e| FigError::Format(format!("zstd: {e}")))?;
        return Ok(read_limited(dec, budget, compressed_len)?);
    }
    match read_limited(flate2::read::DeflateDecoder::new(data), budget, compressed_len) {
        Ok(v) => Ok(v),
        Err(InflateError::Limit(msg)) => Err(FigError::Format(msg.into())),
        Err(InflateError::Unreadable(first)) => {
            // Some producers use zlib framing.
            read_limited(flate2::read::ZlibDecoder::new(data), budget, compressed_len).map_err(|e| match e {
                InflateError::Limit(msg) => FigError::Format(msg.into()),
                InflateError::Unreadable(_) => FigError::Format(first),
            })
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
    use std::cell::Cell;
    use std::rc::Rc;

    /// Reader wrapper that counts how many compressed input bytes have been pulled so far.
    struct CountingReader<R> {
        inner: R,
        bytes_read: Rc<Cell<u64>>,
    }

    impl<R: Read> Read for CountingReader<R> {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            let n = self.inner.read(buf)?;
            self.bytes_read.set(self.bytes_read.get().saturating_add(n as u64));
            Ok(n)
        }
    }

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
        let err = open_zip_with(&z, &mut tight, 16).expect_err("images past the shared budget must return an error, never drop silently");
        assert!(err.to_string().contains("decompressed data too large"), "unexpected error: {err}");
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
        wrap_raw_deflate_in_zip_declaring(entry_name, raw_deflate, declared_uncompressed, raw_deflate.len() as u64)
    }

    /// Like [`wrap_raw_deflate_in_zip`], but the headers claim `comp_size` compressed bytes whatever the stream holds.
    fn wrap_raw_deflate_in_zip_declaring(entry_name: &str, raw_deflate: &[u8], declared_uncompressed: u64, comp_size: u64) -> Vec<u8> {
        let name_bytes = entry_name.as_bytes();
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

    #[test]
    fn zip_bomb_stream_rejected_before_buffering() {
        let raw_deflate = deflate_bomb_2gib_plus_one();
        // ~2.1 MB compressed DEFLATE stream that inflates to 2 GiB + 1 byte of zeros.
        assert!(raw_deflate.len() > 1_000_000 && raw_deflate.len() < 12_000_000, "compressed len={}", raw_deflate.len());
        let bomb_zip = wrap_raw_deflate_in_zip("canvas.fig", &raw_deflate, (2u64 << 30) + 1);

        let err = open(&bomb_zip).expect_err("2 GiB + 1 zip bomb must be rejected");
        assert!(err.to_string().contains("decompressed data too large"), "unexpected error: {err}");

        // Even with the 2 GiB / 4 GiB budget and a declared size under the cap, streaming ratio guard rejects early.
        let within_cap_zip = wrap_raw_deflate_in_zip("canvas.fig", &raw_deflate, 2u64 << 30);
        let err_ratio = open_with_budget(&within_cap_zip, 2 << 30, 4 << 30).expect_err("streaming ratio guard must reject");
        assert!(err_ratio.to_string().contains("decompressed data too large"), "unexpected error: {err_ratio}");

        // Verify directly on the underlying stream that `read_limited` aborts as soon as the output is more than
        // `MAX_INFLATE_RATIO` times the whole 2.1 MB compressed input, after consuming < 256 KiB of it and never
        // buffering the 2 GiB stream.
        let comp_pulled = Rc::new(Cell::new(0u64));
        let reader = CountingReader { inner: Cursor::new(&raw_deflate), bytes_read: Rc::clone(&comp_pulled) };
        let mut budget = Budget::new(MAX_INFLATED, MAX_INFLATED_TOTAL);
        let err2 = FigError::from(
            read_limited(flate2::read::DeflateDecoder::new(reader), &mut budget, raw_deflate.len() as u64)
                .expect_err("raw 2 GiB + 1 deflate bomb must be rejected"),
        );
        assert!(err2.to_string().contains("decompressed data too large"), "unexpected error: {err2}");
        assert_eq!(budget.left, MAX_INFLATED_TOTAL, "failed read must not commit partial bytes");
        assert!(
            comp_pulled.get() < 256 * 1024,
            "streaming ratio guard must abort after pulling < 256 KiB of compressed input, pulled {} bytes",
            comp_pulled.get()
        );
    }

    #[test]
    fn entry_declaring_a_huge_compressed_size_is_still_ratio_checked() {
        // A header that claims a gigantic compressed size must not switch the ratio guard off: the entry cannot hold
        // more than the archive does, and that is what its declared output is judged against.
        let raw_deflate = deflate_bomb_2gib_plus_one();
        let declared_output = 250u64 << 20;
        assert!(declared_output / raw_deflate.len() as u64 > MAX_INFLATE_RATIO, "the declared output must be a bomb for the stream it sits on");
        for name in ["canvas.fig", "images/bomb"] {
            let lying = wrap_raw_deflate_in_zip_declaring(name, &raw_deflate, declared_output, 1 << 40);
            let err = open_with_budget(&lying, 256 << 20, 256 << 20).expect_err("a bomb behind a lying compressed size must be rejected");
            assert!(err.to_string().contains("decompressed data too large"), "{name}: unexpected error: {err}");
        }
    }

    /// Deterministic incompressible bytes (splitmix64), so the tests need no RNG dependency.
    fn pseudo_random_bytes(len: usize, seed: u64) -> Vec<u8> {
        let mut state = seed;
        let mut out = Vec::with_capacity(len + 8);
        while out.len() < len {
            state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut z = state;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            out.extend_from_slice(&(z ^ (z >> 31)).to_le_bytes());
        }
        out.truncate(len);
        out
    }

    /// A message whose zero prefix alone compresses far past [`MAX_INFLATE_RATIO`] once it crosses
    /// [`RATIO_GRACE_BYTES`], but whose incompressible tail keeps the ratio of the whole message low.
    fn skewed_message(how: Compression) -> Vec<u8> {
        let zeros = vec![0u8; RATIO_GRACE_BYTES as usize + (2 << 20)];
        let tail = pseudo_random_bytes(2 << 20, 7);
        let prefix_ratio = zeros.len() / compress(&zeros, how).len();
        let mut msg = zeros;
        msg.extend_from_slice(&tail);
        let whole_ratio = msg.len() / compress(&msg, how).len();
        assert!(prefix_ratio as u64 > MAX_INFLATE_RATIO, "{how:?}: the zero prefix must exceed the ratio limit on its own ({prefix_ratio}:1)");
        assert!((whole_ratio as u64) < MAX_INFLATE_RATIO, "{how:?}: the whole message must stay under the ratio limit ({whole_ratio}:1)");
        msg
    }

    fn assert_skewed_chunk_opens(how: Compression) {
        let msg = skewed_message(how);
        let canvas = write_container(b"schema", &msg, how, 1);
        let (schema, message) = read_chunks(&canvas).unwrap_or_else(|e| panic!("{how:?} bare container: {e}"));
        assert_eq!(schema, b"schema");
        assert!(message == msg, "{how:?} bare container message differs");

        let zipped = write_zip(&canvas, &BTreeMap::new(), None);
        let archive = open(&zipped).unwrap_or_else(|e| panic!("{how:?} zipped container: {e}"));
        let (_, message) = read_archive_chunks(&archive).unwrap_or_else(|e| panic!("{how:?} zipped container chunks: {e}"));
        assert!(message == msg, "{how:?} zipped container message differs");
    }

    #[test]
    fn deflate_chunk_with_compressible_prefix_and_low_overall_ratio_opens() {
        assert_skewed_chunk_opens(Compression::Deflate);
    }

    #[test]
    fn zstd_chunk_with_compressible_prefix_and_low_overall_ratio_opens() {
        assert_skewed_chunk_opens(Compression::Zstd);
    }

    #[test]
    fn chunk_whose_whole_ratio_is_past_the_limit_is_rejected_while_streaming() {
        // The same zero prefix with no incompressible tail behind it: the whole stream is a bomb.
        for how in [Compression::Deflate, Compression::Zstd] {
            let zeros = vec![0u8; RATIO_GRACE_BYTES as usize + (2 << 20)];
            let compressed = compress(&zeros, how);
            assert!((zeros.len() / compressed.len()) as u64 > MAX_INFLATE_RATIO, "{how:?}: the stream must be past the limit overall");
            let err = decompress(&compressed).expect_err("a chunk past the ratio limit overall must be rejected");
            assert!(err.to_string().contains("compression ratio exceeded"), "{how:?}: unexpected error: {err}");
        }
    }

    #[test]
    fn entries_each_under_the_grace_cannot_add_up_to_a_bomb() {
        // Every entry is below RATIO_GRACE_BYTES, so none is judged alone, yet together they inflate to
        // hundreds of times the size of the file.
        let canvas = write_container(b"s", b"m", Compression::Deflate, 1);
        let entry_len = (RATIO_GRACE_BYTES / 2) as usize;
        let images: BTreeMap<String, Vec<u8>> = (0..4).map(|i| (format!("{i}"), vec![0u8; entry_len])).collect();
        let z = write_zip(&canvas, &images, None);
        let inflated = (entry_len * images.len()) as u64;
        assert!(entry_len as u64 <= RATIO_GRACE_BYTES && inflated > RATIO_GRACE_BYTES, "each entry under the grace, their sum over it");
        assert!(inflated / z.len() as u64 > MAX_INFLATE_RATIO, "the whole archive must be past the ratio limit ({} -> {})", z.len(), inflated);
        let err = match open(&z) {
            Ok(archive) => panic!("entries that together exceed the ratio limit must fail the open, but {} images were kept", archive.images.len()),
            Err(e) => e,
        };
        assert!(err.to_string().contains("compression ratio exceeded"), "unexpected error: {err}");
    }

    #[test]
    fn chunks_of_a_zipped_container_draw_on_the_file_wide_ratio_room() {
        // The zip squeezes the repeated blocks of a bomb chunk down to a few KiB, so the container inside it is
        // small enough to pass the entry checks. The chunk is then judged against the whole file, not only
        // against its own (much larger) compressed length.
        let bomb_chunk = deflate_bomb_2gib_plus_one();
        let schema_chunk = compress(b"s", Compression::Deflate);
        let mut canvas = Vec::new();
        canvas.extend_from_slice(MAGIC_KIWI);
        canvas.extend_from_slice(&1u32.to_le_bytes());
        for chunk in [&schema_chunk, &bomb_chunk] {
            canvas.extend_from_slice(&(chunk.len() as u32).to_le_bytes());
            canvas.extend_from_slice(chunk);
        }
        let z = write_zip(&canvas, &BTreeMap::new(), None);
        assert!(
            (z.len() as u64 + 1) * (MAX_INFLATE_RATIO + 1) < RATIO_GRACE_BYTES,
            "the file is small enough for the grace window to be its whole room ({} bytes)",
            z.len()
        );

        // A per-read cap of 64 MiB keeps the unfixed run small; the ratio room is what must stop the chunk first.
        let archive = open_with_budget(&z, 64 << 20, 4 << 30).expect("the zip itself holds only a 2 MB container");
        let err = read_archive_chunks(&archive).expect_err("a chunk that inflates far past the ratio limit of the whole file must be rejected");
        assert!(err.to_string().contains("compression ratio exceeded"), "unexpected error: {err}");
    }

    #[test]
    fn dense_entries_listed_before_an_incompressible_one_still_open() {
        // The ratio is judged against the whole file, so a dense entry that comes first is not rejected for
        // the incompressible ones that follow it.
        let canvas = write_container(b"s", b"m", Compression::Deflate, 1);
        let mut images: BTreeMap<String, Vec<u8>> = (0..3).map(|i| (format!("a{i}"), vec![0u8; (RATIO_GRACE_BYTES / 2) as usize])).collect();
        images.insert("z".to_string(), pseudo_random_bytes(1 << 20, 11));
        let z = write_zip(&canvas, &images, None);
        let inflated: u64 = images.values().map(|v| v.len() as u64).sum();
        assert!(inflated > RATIO_GRACE_BYTES && inflated / (z.len() as u64) <= MAX_INFLATE_RATIO, "whole archive under the limit");
        let archive = open(&z).expect("an archive whose whole ratio is under the limit opens");
        assert!(archive.images == images);
    }

    #[test]
    fn zip_entry_with_compressible_prefix_and_low_overall_ratio_opens() {
        let image = skewed_message(Compression::Deflate);
        let canvas = write_container(b"s", b"m", Compression::Deflate, 1);
        let mut images = BTreeMap::new();
        images.insert("skewed".to_string(), image.clone());
        let archive = open(&write_zip(&canvas, &images, None)).expect("an entry whose whole ratio is under the limit opens");
        assert!(archive.images["skewed"] == image);
    }

    /// Flips the first compressed bytes of `name` so inflating it fails, and proves the damage is real.
    fn damage_entry(zip_bytes: &mut [u8], name: &str) {
        let start = {
            let mut archive = zip::ZipArchive::new(Cursor::new(&*zip_bytes)).unwrap();
            let entry = archive.by_name(name).unwrap();
            entry.data_start().expect("data offset of a located entry") as usize
        };
        zip_bytes[start] ^= 0xFF;
        zip_bytes[start + 1] ^= 0xFF;
        let mut archive = zip::ZipArchive::new(Cursor::new(&*zip_bytes)).unwrap();
        let mut sink = Vec::new();
        assert!(archive.by_name(name).unwrap().read_to_end(&mut sink).is_err(), "{name} must be unreadable after the damage");
    }

    #[test]
    fn damaged_image_or_meta_entry_is_skipped_not_fatal() {
        let canvas = write_container(b"schema", b"message", Compression::Deflate, 1);
        let payload = b"image bytes that will not survive".repeat(40);
        let mut z = zip_of(&[
            ("canvas.fig", canvas.clone()),
            ("meta.json", br#"{"file_name":"x"}"#.repeat(20)),
            ("images/bad", payload.clone()),
            ("images/good", payload.clone()),
        ]);
        damage_entry(&mut z, "images/bad");
        damage_entry(&mut z, "meta.json");

        let archive = open(&z).expect("one unreadable image or meta.json must not fail the whole file");
        assert_eq!(archive.canvas, canvas);
        assert_eq!(archive.images.keys().map(String::as_str).collect::<Vec<_>>(), ["good"], "only the damaged image is left out");
        assert_eq!(archive.images["good"], payload);
        assert!(archive.meta.is_none());
        let (schema, message) = read_archive_chunks(&archive).unwrap();
        assert_eq!((schema.as_slice(), message.as_slice()), (&b"schema"[..], &b"message"[..]));
    }

    #[test]
    fn image_entry_past_the_ratio_limit_still_fails_the_open() {
        // Skipping unreadable entries must not turn a real bomb into a silently dropped image.
        let canvas = write_container(b"s", b"m", Compression::Deflate, 1);
        let mut images = BTreeMap::new();
        images.insert("bomb".to_string(), vec![0u8; RATIO_GRACE_BYTES as usize + (4 << 20)]);
        let err = open(&write_zip(&canvas, &images, None)).expect_err("an image far past the ratio limit must fail the open");
        assert!(err.to_string().contains("decompressed data too large"), "unexpected error: {err}");
    }

    #[test]
    fn legitimate_160mib_stream_opens_cleanly() {
        // Build a 160 MiB message stream with realistic ~4:1 compression ratio (64-byte varied pattern)
        // plus 20 MiB of embedded images, matching real enterprise .fig files (e.g. VSF - PCP 182 MiB, Hugeicons 143 MiB).
        let msg_len = 160 << 20;
        let mut msg = vec![0u8; msg_len];
        let mut state = 0x9E37_79B9_7F4A_7C15u64;
        for chunk in msg.chunks_mut(64) {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            let bytes = state.to_le_bytes();
            // First 16 bytes vary per 64-byte record (~3-4x compression ratio, well under MAX_INFLATE_RATIO = 100).
            chunk[0..8].copy_from_slice(&bytes);
            chunk[8..16].copy_from_slice(&state.rotate_left(17).to_le_bytes());
        }
        let canvas = write_container(b"schema-bytes", &msg, Compression::Zstd, 106);
        let mut images = BTreeMap::new();
        images.insert("img1".to_string(), msg[..10 << 20].to_vec());
        images.insert("img2".to_string(), msg[10 << 20..20 << 20].to_vec());
        let zipped = write_zip(&canvas, &images, None);

        let archive = open(&zipped).expect("legitimate 160 MiB message + 20 MiB images archive must open");
        assert_eq!(archive.images.len(), 2);
        let (schema, decoded_msg) = read_archive_chunks(&archive).expect("160 MiB message chunk must decompress");
        assert_eq!(schema, b"schema-bytes");
        assert_eq!(decoded_msg.len(), msg_len);
        assert_eq!(&decoded_msg[..64], &msg[..64]);
        assert_eq!(&decoded_msg[msg_len - 64..], &msg[msg_len - 64..]);
    }
}
