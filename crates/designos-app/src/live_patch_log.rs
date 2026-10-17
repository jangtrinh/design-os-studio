//! Reads the append-only edit log `designos-mcp` writes for a session (`doc-patch.jsonl`).
//!
//! Runs on the UI thread every frame, so each poll reads a bounded amount, and a log that was
//! recreated or truncated is noticed even when it has grown past the old read position.

use designos_doc::live_patch::{LivePatch, LivePatchState, MAX_LIVE_PATCH_LINE_BYTES};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Most bytes one poll reads; the rest waits for the next frame.
const MAX_POLL_BYTES: u64 = 64 << 20;
/// Leading bytes remembered to notice a log rewritten in place.
const HEAD_BYTES: usize = 256;

/// Which file is at the path: inode (Unix) and creation time.
type FileIdentity = (u64, Option<SystemTime>);

fn identity(meta: &std::fs::Metadata) -> FileIdentity {
    #[cfg(unix)]
    let ino = std::os::unix::fs::MetadataExt::ino(meta);
    #[cfg(not(unix))]
    let ino = 0;
    (ino, meta.created().ok())
}

/// Read position in a session's patch log, plus replay state across patches.
#[derive(Debug)]
pub struct LivePatchLog {
    pub path: PathBuf,
    /// Bytes already consumed. Starts at the current length so lines from earlier runs are not
    /// replayed into a newly opened file.
    offset: u64,
    identity: Option<FileIdentity>,
    head: Vec<u8>,
    /// Inside a line over the size limit: discard up to its newline.
    skipping: bool,
    max_poll_bytes: u64,
    max_line_bytes: usize,
    pub state: LivePatchState,
}

impl LivePatchLog {
    pub fn bind(path: PathBuf) -> Self {
        let meta = std::fs::metadata(&path).ok();
        let mut log = Self {
            offset: meta.as_ref().map_or(0, |m| m.len()),
            identity: meta.as_ref().map(identity),
            head: Vec::new(),
            skipping: false,
            max_poll_bytes: MAX_POLL_BYTES,
            max_line_bytes: MAX_LIVE_PATCH_LINE_BYTES,
            state: LivePatchState::default(),
            path,
        };
        log.head = log.read_range(0, HEAD_BYTES as u64).unwrap_or_default();
        log
    }

    /// Complete patch lines appended since the last poll. Unreadable and oversized lines come
    /// back as errors; nothing is skipped silently.
    pub fn poll(&mut self) -> (Vec<LivePatch>, Vec<String>) {
        let Ok(meta) = std::fs::metadata(&self.path) else { return Default::default() };
        let (len, id) = (meta.len(), identity(&meta));
        if len == self.offset && self.identity == Some(id) {
            return Default::default();
        }
        let rewritten = self.identity.is_some_and(|old| old != id) || len < self.offset || !self.head_unchanged();
        if rewritten {
            // A recreated or truncated log: everything in it is new.
            self.offset = 0;
            self.skipping = false;
        }
        self.identity = Some(id);
        if rewritten || self.head.len() < HEAD_BYTES {
            self.head = self.read_range(0, HEAD_BYTES as u64).unwrap_or_default();
        }
        if len <= self.offset {
            return Default::default();
        }
        let Some(buf) = self.read_range(self.offset, (len - self.offset).min(self.max_poll_bytes)) else { return Default::default() };
        let (consumed, patches, errors) = self.split_lines(&buf);
        self.offset += consumed as u64;
        (patches, errors)
    }

    fn split_lines(&mut self, buf: &[u8]) -> (usize, Vec<LivePatch>, Vec<String>) {
        let (mut patches, mut errors) = (Vec::new(), Vec::new());
        let mut pos = 0;
        if self.skipping {
            let Some(nl) = buf.iter().position(|b| *b == b'\n') else { return (buf.len(), patches, errors) };
            pos = nl + 1;
            self.skipping = false;
        }
        while let Some(nl) = buf[pos..].iter().position(|b| *b == b'\n') {
            let line = &buf[pos..pos + nl];
            pos += nl + 1;
            if line.len() > self.max_line_bytes {
                errors.push(self.too_large(line.len()));
            } else if !line.iter().all(u8::is_ascii_whitespace) {
                match serde_json::from_slice::<LivePatch>(line) {
                    Ok(p) => patches.push(p),
                    Err(e) => errors.push(format!("Unreadable agent edit in {}: {e}", display_name(&self.path))),
                }
            }
        }
        // A line still being written has no newline yet and waits for the next poll, unless it
        // is already over the limit.
        if buf.len() - pos > self.max_line_bytes {
            errors.push(self.too_large(buf.len() - pos));
            self.skipping = true;
            pos = buf.len();
        }
        (pos, patches, errors)
    }

    fn too_large(&self, bytes: usize) -> String {
        let mib = |b: usize| b as f64 / (1 << 20) as f64;
        format!("An agent edit of {:.1} MiB is over the {:.0} MiB limit and was not applied", mib(bytes), mib(self.max_line_bytes))
    }

    fn head_unchanged(&self) -> bool {
        self.head.is_empty() || self.read_range(0, self.head.len() as u64).is_some_and(|now| now == self.head)
    }

    fn read_range(&self, start: u64, len: u64) -> Option<Vec<u8>> {
        let mut buf = Vec::new();
        let mut f = std::fs::File::open(&self.path).ok()?;
        f.seek(SeekFrom::Start(start)).ok()?;
        f.take(len).read_to_end(&mut buf).ok()?;
        Some(buf)
    }
}

fn display_name(path: &Path) -> String {
    path.file_name().and_then(|n| n.to_str()).unwrap_or("doc-patch.jsonl").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use designos_doc::live_patch::LiveOp;
    use designos_doc::NodeId;

    fn line(id: u64) -> String {
        serde_json::to_string(&LivePatch::new(vec![LiveOp::Remove { id: NodeId(id) }])).unwrap() + "\n"
    }

    fn ids(patches: &[LivePatch]) -> Vec<u64> {
        patches.iter().flat_map(|p| &p.ops).map(|op| if let LiveOp::Remove { id } = op { id.0 } else { 0 }).collect()
    }

    fn temp_log(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("designos-patch-log-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir.join("doc-patch.jsonl")
    }

    fn append(path: &Path, text: &str) {
        use std::io::Write;
        std::fs::OpenOptions::new().create(true).append(true).open(path).unwrap().write_all(text.as_bytes()).unwrap();
    }

    #[test]
    fn old_lines_are_ignored_and_partial_lines_wait() {
        let path = temp_log("partial");
        append(&path, &line(1));
        let mut log = LivePatchLog::bind(path.clone());
        let next = line(2);
        append(&path, &next[..10]);
        assert!(log.poll().0.is_empty());
        append(&path, &next[10..]);
        assert_eq!(ids(&log.poll().0), [2]);
    }

    #[test]
    fn a_recreated_or_rewritten_log_is_read_from_the_start() {
        let path = temp_log("rotate");
        append(&path, &line(1));
        let mut log = LivePatchLog::bind(path.clone());
        // Recreated and refilled past the old read position.
        std::fs::remove_file(&path).unwrap();
        append(&path, &(line(20) + &line(21)));
        assert_eq!(ids(&log.poll().0), [20, 21]);
        // Rewritten in place (same file) and refilled past the old read position.
        std::fs::write(&path, line(30) + &line(31) + &line(32)).unwrap();
        assert_eq!(ids(&log.poll().0), [30, 31, 32]);
    }

    #[test]
    fn oversized_lines_are_reported_and_reading_continues() {
        let path = temp_log("caps");
        let mut log = LivePatchLog::bind(path.clone());
        log.max_line_bytes = 64;
        log.max_poll_bytes = 100;
        let big = format!("{}\n", "x".repeat(250));
        append(&path, &(line(1) + &big + &line(2)));
        let mut seen = Vec::new();
        let mut errors = Vec::new();
        for _ in 0..10 {
            let (p, e) = log.poll();
            seen.extend(ids(&p));
            errors.extend(e);
        }
        assert_eq!(seen, [1, 2]);
        assert_eq!(errors.len(), 1, "{errors:?}");
        assert!(errors[0].contains("over the"), "{}", errors[0]);
    }
}
