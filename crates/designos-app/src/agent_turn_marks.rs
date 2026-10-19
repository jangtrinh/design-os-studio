//! Hook-written turn boundary marks (`<session_dir>/turn-marks.jsonl`) for agent tabs (`#677`).
//!
//! Supported CLIs (`Claude`, `Grok`, `Antigravity`) append a single JSON line with the current
//! byte length of `doc-patch.jsonl` when a turn starts (`UserPromptSubmit` / `PreInvocation`) and
//! when a turn ends (`Stop` / `StopCancelled` / `StopFailure`).

use designos_doc::live_patch::LIVE_PATCH_VERSION;
use serde::{Deserialize, Serialize};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

const HEAD_BYTES: usize = 64;
const MAX_POLL_BYTES: u64 = 1 << 20;
const MAX_MARK_LINE_BYTES: usize = 64 << 10;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TurnPhase {
    Start,
    End,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TurnMarkLine {
    pub v: u32,
    pub turn_mark: TurnPhase,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cli: Option<String>,
    pub patch_bytes: u64,
}

impl TurnMarkLine {
    pub fn parse(line: &[u8]) -> Result<Self, String> {
        let mark: Self = serde_json::from_slice(line).map_err(|e| e.to_string())?;
        if mark.v != LIVE_PATCH_VERSION {
            return Err(format!("unsupported turn mark version {}", mark.v));
        }
        Ok(mark)
    }
}

/// Incremental byte-offset reader for `<session_dir>/turn-marks.jsonl`.
#[derive(Debug)]
pub struct TurnMarkLog {
    path: PathBuf,
    offset: u64,
    identity: Option<(u64, u64)>,
    head: Vec<u8>,
    skipping: bool,
}

impl TurnMarkLog {
    pub fn bind(path: PathBuf) -> Self {
        Self { path, offset: 0, identity: None, head: Vec::new(), skipping: false }
    }

    #[allow(dead_code)]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Reads complete [`TurnMarkLine`] entries appended since the last poll, in log order.
    pub fn poll(&mut self) -> Vec<TurnMarkLine> {
        let Ok(meta) = std::fs::metadata(&self.path) else { return Vec::new() };
        let (len, id) = (meta.len(), identity(&meta));
        if len == self.offset && self.identity == Some(id) {
            return Vec::new();
        }
        let rewritten = self.identity.is_some_and(|old| old != id) || len < self.offset || !self.head_unchanged();
        if rewritten {
            self.offset = 0;
            self.skipping = false;
        }
        self.identity = Some(id);
        if rewritten || self.head.len() < HEAD_BYTES {
            self.head = self.read_range(0, HEAD_BYTES as u64).unwrap_or_default();
        }
        if len <= self.offset {
            return Vec::new();
        }
        let Some(buf) = self.read_range(self.offset, (len - self.offset).min(MAX_POLL_BYTES)) else {
            return Vec::new();
        };
        let (consumed, marks) = self.split_lines(&buf);
        self.offset += consumed as u64;
        marks
    }

    fn split_lines(&mut self, buf: &[u8]) -> (usize, Vec<TurnMarkLine>) {
        let mut marks = Vec::new();
        let mut pos = 0;
        if self.skipping {
            let Some(nl) = buf.iter().position(|b| *b == b'\n') else {
                return (buf.len(), marks);
            };
            pos = nl + 1;
            self.skipping = false;
        }
        while let Some(nl) = buf[pos..].iter().position(|b| *b == b'\n') {
            let line = &buf[pos..pos + nl];
            pos += nl + 1;
            if line.len() <= MAX_MARK_LINE_BYTES && !line.iter().all(u8::is_ascii_whitespace) {
                if let Ok(m) = TurnMarkLine::parse(line) {
                    marks.push(m);
                }
            }
        }
        if buf.len() - pos > MAX_MARK_LINE_BYTES {
            self.skipping = true;
            pos = buf.len();
        }
        (pos, marks)
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

#[cfg(unix)]
fn identity(m: &std::fs::Metadata) -> (u64, u64) {
    use std::os::unix::fs::MetadataExt;
    (m.dev(), m.ino())
}

#[cfg(not(unix))]
fn identity(m: &std::fs::Metadata) -> (u64, u64) {
    let c = m.created().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_nanos() as u64);
    (c, 0)
}

fn sh_quote_path(path: &Path) -> String {
    format!("'{}'", path.display().to_string().replace('\'', "'\\''"))
}

/// Shell command that appends a `{"v":1,"turn_mark":"...","cli":"...","patch_bytes":N}` JSON line
/// to `mark_path`, where `N` is the current byte length of `patch_log_path` (or `0` when absent).
/// Produces no stdout so CLI hook output contracts remain untouched.
pub fn turn_mark_hook_command(mark_path: &Path, patch_log_path: &Path, phase: &str, cli: &str) -> String {
    let mark_q = sh_quote_path(mark_path);
    let patch_q = sh_quote_path(patch_log_path);
    format!(
        "n=$( (wc -c < {patch_q}) 2>/dev/null || echo 0); \
         printf '{{\"v\":1,\"turn_mark\":\"{phase}\",\"cli\":\"{cli}\",\"patch_bytes\":%d}}\\n' \"$n\" >> {mark_q}"
    )
}
