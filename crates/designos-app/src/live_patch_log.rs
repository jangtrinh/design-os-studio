//! Reads the append-only edit log `designos-mcp` writes for a session (`doc-patch.jsonl`).
//!
//! Runs on the UI thread every frame, so each poll reads a bounded amount, and a log that was
//! recreated or truncated is noticed even when it has grown past the old read position.

use designos_doc::live_patch::{
    log_version_refusal, CloseIntent, DocEpoch, LiveLogLine, LivePatch, LivePatchState, LIVE_PATCH_VERSION, MAX_LIVE_PATCH_LINE_BYTES, UPDATE_BOTH_HINT,
};
use designos_doc::Document;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Most bytes one poll reads; the rest waits for the next frame.
const MAX_POLL_BYTES: u64 = 64 << 20;
/// Bytes read at a time when looking for an unread complete line.
const SCAN_CHUNK_BYTES: usize = 64 << 10;
/// Leading bytes remembered to notice a log rewritten in place.
const HEAD_BYTES: usize = 256;

/// Which file is at the path: inode (Unix) and creation time.
type FileIdentity = (u64, Option<SystemTime>);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum StaleClose {
    #[default]
    None,
    Hidden,
    Before(u64),
}

fn identity(meta: &std::fs::Metadata) -> FileIdentity {
    #[cfg(unix)]
    let ino = std::os::unix::fs::MetadataExt::ino(meta);
    #[cfg(not(unix))]
    let ino = 0;
    (ino, meta.created().ok())
}

fn read_range(path: &Path, start: u64, len: u64) -> Option<Vec<u8>> {
    let mut buf = Vec::new();
    let mut f = crate::agent_session_files::open_session_file(path).ok()?;
    f.seek(SeekFrom::Start(start)).ok()?;
    f.take(len).read_to_end(&mut buf).ok()?;
    Some(buf)
}

/// Read position in a session's patch log, plus replay state across patches.
#[derive(Debug)]
pub struct LivePatchLog {
    pub path: PathBuf,
    /// Wakes an idle window when the agent appends to the log (see [`Self::wake_on_writes`]).
    wake: crate::live_log_wake::LogWake,
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
    /// Patches already read but not applied yet, because the user was in the middle of an edit.
    /// The read position is past them, so they live only here.
    pub pending: Vec<LivePatch>,
    /// Start byte offset in `doc-patch.jsonl` for each entry in [`Self::pending`].
    pub pending_offsets: Vec<u64>,
    /// Start byte offsets of lines returned by recent [`Self::poll`] calls, consumed by [`Self::ingest`].
    last_polled_offsets: std::collections::VecDeque<u64>,
    /// Set when a [`DocEpoch`] line was ingested and waiting to replace the document.
    pub pending_epoch_switch: bool,
    /// A document the agent switched to while the user was mid-edit. It replaces the open one
    /// once the canvas is idle, before any patch in `pending` (those were made on it).
    pub pending_doc: Option<Document>,
    /// The agent closed the document. Applied after held patches, never as a quit of other tabs.
    pub pending_close: Option<CloseIntent>,
    stale_close: StaleClose,
    /// Where the agent's documents are written: `doc-sync.designos` next to the log.
    doc_sync_path: PathBuf,
    /// Edits dropped because they were made on a document the agent had already left, or on one
    /// that could not be loaded before a later switch did. Shown with the next document switch.
    pub dropped_old_epoch: usize,
    /// The document the agent works on could not be loaded, so its edits are dropped until the
    /// next document switch.
    discarding: bool,
    /// What `discarding` was when the held close line was read, so a refused close puts it back (AC-02).
    discarding_before_close: Option<bool>,
    /// `discarding` is not a superseded switch but a failure the user must be told about: edits
    /// dropped meanwhile are reported at once, unless a switch that loads follows in the same
    /// batch (see `ingest`).
    load_failed: bool,
    /// An edit written by a newer build was read in this document epoch. It may have changed layers
    /// the edits after it build on, so none of them applies until the next document switch. Set once
    /// per epoch, which is also what limits the toast to one.
    fenced: bool,
    /// Readable edits dropped behind the fence. Reset with it.
    pub fenced_dropped: usize,
    /// The message for the fence, until the apply step has run. Edits before the newer one still apply
    /// and their own toast would replace the one `ingest` returned, so the apply step shows this after them.
    pub fence_notice: Option<String>,
}

/// The read position of a [`LivePatchLog`] at one moment (`#690` §6): enough to tell, later and from
/// another thread, whether the agent has appended a complete line since.
#[derive(Clone, Debug)]
pub struct UnreadProbe {
    path: PathBuf,
    offset: u64,
    identity: Option<FileIdentity>,
    head: Vec<u8>,
    skipping: bool,
    max_poll_bytes: u64,
    max_line_bytes: usize,
}

impl UnreadProbe {
    /// A complete line (ending in a newline) was appended that no poll has read yet. A line still
    /// being written, or left behind by a crashed writer, has no newline and does not count: it
    /// would hold the editor's canvas back until the next append. While an oversized line is being
    /// discarded its own tail never counts, but a normal line after the tail does.
    ///
    /// Reads at most one line's worth of bytes (the limit on a line) per sought newline, in
    /// chunks, and stops at the first newline that decides it.
    pub fn has_unread_complete_line(&self) -> bool {
        let Ok(meta) = crate::agent_session_files::session_file_metadata(&self.path) else { return false };
        let len = meta.len();
        // A recreated or truncated log is unread from its start, exactly as `poll_with_offsets` sees it.
        let rewritten = self.identity.is_some_and(|old| old != identity(&meta)) || len < self.offset || !self.head_unchanged();
        let (start, skipping) = if rewritten { (0, false) } else { (self.offset, self.skipping) };
        if len <= start {
            return false;
        }
        // The end of the discarded tail is not a line; the newline after it ends the next one.
        let mut newlines_needed = if skipping { 2 } else { 1 };
        let Ok(mut file) = crate::agent_session_files::open_session_file(&self.path) else { return false };
        if file.seek(SeekFrom::Start(start)).is_err() {
            return false;
        }
        let mut remaining = (len - start).min(self.max_poll_bytes);
        let mut chunk = vec![0u8; SCAN_CHUNK_BYTES];
        // Bytes since the last newline: a line longer than the limit is reported, never applied.
        let mut run = 0usize;
        while remaining > 0 {
            let want = chunk.len().min(remaining as usize);
            let Ok(n) = file.read(&mut chunk[..want]) else { return false };
            if n == 0 {
                return false;
            }
            remaining -= n as u64;
            for byte in &chunk[..n] {
                if *byte == b'\n' {
                    newlines_needed -= 1;
                    run = 0;
                    if newlines_needed == 0 {
                        return true;
                    }
                } else {
                    run += 1;
                }
            }
            let in_discarded_tail = skipping && newlines_needed == 2;
            if run > self.max_line_bytes && !in_discarded_tail {
                return false;
            }
        }
        false
    }

    fn head_unchanged(&self) -> bool {
        self.head.is_empty() || read_range(&self.path, 0, self.head.len() as u64).is_some_and(|now| now == self.head)
    }
}

impl LivePatchLog {
    pub fn bind(path: PathBuf) -> Self {
        let meta = crate::agent_session_files::session_file_metadata(&path).ok();
        let mut log = Self {
            offset: meta.as_ref().map_or(0, |m| m.len()),
            identity: meta.as_ref().map(identity),
            head: Vec::new(),
            skipping: false,
            max_poll_bytes: MAX_POLL_BYTES,
            max_line_bytes: MAX_LIVE_PATCH_LINE_BYTES,
            state: LivePatchState::default(),
            pending: Vec::new(),
            pending_offsets: Vec::new(),
            last_polled_offsets: std::collections::VecDeque::new(),
            pending_epoch_switch: false,
            pending_doc: None,
            pending_close: None,
            stale_close: StaleClose::None,
            wake: Default::default(),
            doc_sync_path: path.with_file_name("doc-sync.designos"),
            dropped_old_epoch: 0,
            discarding: false,
            discarding_before_close: None,
            load_failed: false,
            fenced: false,
            fenced_dropped: 0,
            fence_notice: None,
            path,
        };
        log.head = log.read_range(0, HEAD_BYTES as u64).unwrap_or_default();
        log
    }

    /// Keeps `ctx` awake for agent writes to this log and to `spec_path` (`live-spec.json`): call it every
    /// frame. A window with nothing to animate runs no frame, and the log is only read inside one.
    pub fn wake_on_writes(&mut self, ctx: &egui::Context, spec_path: &std::path::Path) {
        self.wake.watch(ctx, &[&self.path, spec_path]);
    }

    /// Stops the wake thread (a project tab that is parked: only the showing tab's log is read, so an agent
    /// writing in the background must not wake the window). `wake_on_writes` starts it again.
    pub fn stop_wake(&mut self) {
        self.wake = Default::default();
    }

    /// Whether the log's wake thread is running (tests).
    #[cfg(test)]
    pub fn wake_running(&self) -> bool {
        self.wake.is_running()
    }

    /// Current byte offset read in `doc-patch.jsonl`.
    pub fn current_offset(&self) -> u64 {
        self.offset
    }

    /// Agent changes wait for the canvas to go idle.
    pub fn has_held(&self) -> bool {
        !self.pending.is_empty() || self.pending_doc.is_some() || self.pending_close.is_some()
    }

    /// A complete line (ending in a newline) was appended that no poll has read yet; see
    /// [`UnreadProbe::has_unread_complete_line`].
    pub fn has_unread_complete_line(&self) -> bool {
        self.unread_probe().has_unread_complete_line()
    }

    /// A frozen copy of the read position, so a writer thread can ask the same question
    /// ([`UnreadProbe::has_unread_complete_line`]) a moment later without touching this log.
    pub fn unread_probe(&self) -> UnreadProbe {
        UnreadProbe {
            path: self.path.clone(),
            offset: self.offset,
            identity: self.identity,
            head: self.head.clone(),
            skipping: self.skipping,
            max_poll_bytes: self.max_poll_bytes,
            max_line_bytes: self.max_line_bytes,
        }
    }

    /// Complete log lines appended since the last poll, in log order. Unreadable and oversized
    /// lines come back as errors; nothing is skipped silently.
    pub fn poll(&mut self) -> (Vec<LiveLogLine>, Vec<String>) {
        let (lines, errors) = self.poll_with_offsets();
        let mut out = Vec::with_capacity(lines.len());
        for (off, line) in lines {
            self.last_polled_offsets.push_back(off);
            out.push(line);
        }
        (out, errors)
    }

    /// Complete log lines appended since the last poll along with each line's start byte offset
    /// in `doc-patch.jsonl`.
    pub fn poll_with_offsets(&mut self) -> (Vec<(u64, LiveLogLine)>, Vec<String>) {
        let Ok(meta) = crate::agent_session_files::session_file_metadata(&self.path) else { return Default::default() };
        let (len, id) = (meta.len(), identity(&meta));
        if self.stale_close == StaleClose::Hidden {
            self.stale_close = StaleClose::Before(len);
        }
        if len == self.offset && self.identity == Some(id) {
            return Default::default();
        }
        let rewritten = self.identity.is_some_and(|old| old != id) || len < self.offset || !self.head_unchanged();
        if rewritten {
            // A recreated or truncated log: everything in it is new.
            self.offset = 0;
            self.skipping = false;
            self.pending.clear();
            self.pending_offsets.clear();
            self.last_polled_offsets.clear();
            self.pending_epoch_switch = false;
            self.pending_doc = None;
            self.pending_close = None;
            self.stale_close = StaleClose::None;
            self.dropped_old_epoch = 0;
            self.discarding = false;
            self.discarding_before_close = None;
            self.load_failed = false;
            self.fenced = false;
            self.fenced_dropped = 0;
            self.fence_notice = None;
        }
        self.identity = Some(id);
        if rewritten || self.head.len() < HEAD_BYTES {
            self.head = self.read_range(0, HEAD_BYTES as u64).unwrap_or_default();
        }
        if len <= self.offset {
            return Default::default();
        }
        let base_offset = self.offset;
        let Some(buf) = self.read_range(self.offset, (len - self.offset).min(self.max_poll_bytes)) else { return Default::default() };
        let (consumed, patches, errors) = self.split_lines(base_offset, &buf);
        self.offset += consumed as u64;
        (patches, errors)
    }

    fn split_lines(&mut self, base_offset: u64, buf: &[u8]) -> (usize, Vec<(u64, LiveLogLine)>, Vec<String>) {
        let (mut patches, mut errors) = (Vec::new(), Vec::new());
        let mut pos = 0;
        if self.skipping {
            let Some(nl) = buf.iter().position(|b| *b == b'\n') else { return (buf.len(), patches, errors) };
            pos = nl + 1;
            self.skipping = false;
        }
        while let Some(nl) = buf[pos..].iter().position(|b| *b == b'\n') {
            let line_start = base_offset + pos as u64;
            let line = &buf[pos..pos + nl];
            pos += nl + 1;
            if line.len() > self.max_line_bytes {
                errors.push(self.too_large(line.len()));
            } else if !line.iter().all(u8::is_ascii_whitespace) {
                match LiveLogLine::parse(line) {
                    // A newer edit stays in the stream: `ingest` fences what follows it, so the order matters.
                    Ok(p) => patches.push((line_start, p)),
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

    /// Sorts what a poll read into what is held. The log is the one source of order: a document
    /// switch drops every edit still held for the previous document and makes the switched-to
    /// document the one to open; edits after it are held for that document. Returns what the
    /// user must be told.
    pub fn ingest(&mut self, lines: Vec<LiveLogLine>) -> Vec<String> {
        let with_offsets: Vec<(u64, LiveLogLine)> = lines.into_iter().map(|l| (self.last_polled_offsets.pop_front().unwrap_or(0), l)).collect();
        self.ingest_with_offsets(with_offsets)
    }

    /// Same as [`Self::ingest`], preserving each line's start byte offset in [`Self::pending_offsets`].
    pub fn ingest_with_offsets(&mut self, lines: Vec<(u64, LiveLogLine)>) -> Vec<String> {
        let (mut errors, mut dropped_while_stuck) = (Vec::new(), 0);
        for (line_offset, line) in lines {
            match line {
                LiveLogLine::Patch(_) if self.discarding => {
                    if self.load_failed {
                        dropped_while_stuck += 1;
                    } else {
                        self.dropped_old_epoch += 1;
                    }
                }
                LiveLogLine::Patch(_) if self.fenced => self.fenced_dropped += 1,
                LiveLogLine::Patch(patch) => {
                    self.pending.push(patch);
                    self.pending_offsets.push(line_offset);
                }
                // Edits before it stay held. Everything after it waits for a document switch, and the
                // user is told once.
                LiveLogLine::Newer { v } => {
                    if !self.fenced {
                        self.fenced = true;
                        let notice = newer_edit_error(v);
                        self.fence_notice = Some(notice.clone());
                        errors.push(notice);
                    }
                }
                LiveLogLine::DocEpoch(epoch) => {
                    self.fenced = false;
                    self.fenced_dropped = 0;
                    self.fence_notice = None;
                    self.dropped_old_epoch += self.pending.len();
                    self.pending.clear();
                    self.pending_offsets.clear();
                    self.pending_close = None;
                    self.discarding_before_close = None;
                    self.pending_epoch_switch = true;
                    self.pending_doc = None;
                    self.load_failed = false;
                    self.discarding = true;
                    match self.load_epoch(&epoch) {
                        Ok(Some(doc)) => {
                            self.pending_doc = Some(doc);
                            self.discarding = false;
                        }
                        // Overwritten by a later switch, whose own line follows.
                        Ok(None) => {}
                        Err(e) => {
                            self.load_failed = true;
                            errors.push(e);
                        }
                    }
                }
                LiveLogLine::Close(intent) => {
                    self.fenced = false;
                    self.fenced_dropped = 0;
                    self.fence_notice = None;
                    if self.drops_stale_close(line_offset) {
                        write_close_result_file(
                            &self.path,
                            &intent,
                            serde_json::json!({
                                "ok": false,
                                "code": "EDITOR_NOT_NOTIFIED",
                                "message": "The close was dropped because the tab was no longer shown.",
                            }),
                        );
                    } else {
                        self.pending_close = Some(intent);
                        self.discarding_before_close = Some(self.discarding);
                        self.discarding = true;
                    }
                }
            }
        }
        if dropped_while_stuck > 0 {
            if self.pending_doc.is_some() {
                // A switch later in this batch loaded (a failure leaves `pending_doc` empty), so
                // the toast that opens it replaces any error shown now. It reports these edits
                // with the ones dropped for the switch, once.
                self.dropped_old_epoch += dropped_while_stuck;
            } else {
                errors.push(format!("{dropped_while_stuck} agent edit(s) were not applied: Studio could not load the document the agent opened"));
            }
        }
        errors
    }

    /// The document written for `epoch`; `None` when a later switch has already replaced it.
    fn load_epoch(&self, epoch: &DocEpoch) -> Result<Option<Document>, String> {
        if log_version_refusal(epoch.v).is_some() {
            return Err(format!("Agent document not opened. {}", refusal_next_step(epoch.v)));
        }
        let bytes = crate::agent_session_files::read_session_file(&self.doc_sync_path)
            .map_err(|e| format!("Cannot read the document the agent opened: {}", crate::agent_session_files::with_next_step(&e)))?;
        if !epoch.matches(&bytes) {
            return Ok(None);
        }
        let raw = String::from_utf8(bytes).map_err(|e| format!("The document the agent opened is not valid text: {e}"))?;
        Document::from_json(&raw).map(Some).map_err(|e| format!("Cannot parse the document the agent opened: {e}"))
    }

    fn too_large(&self, bytes: usize) -> String {
        let mib = |b: usize| b as f64 / (1 << 20) as f64;
        format!("An agent edit of {:.1} MiB is over the {:.0} MiB limit and was not applied", mib(bytes), mib(self.max_line_bytes))
    }

    fn head_unchanged(&self) -> bool {
        self.head.is_empty() || self.read_range(0, self.head.len() as u64).is_some_and(|now| now == self.head)
    }

    fn read_range(&self, start: u64, len: u64) -> Option<Vec<u8>> {
        read_range(&self.path, start, len)
    }

    pub fn drop_close_on_hide(&mut self) -> Option<CloseIntent> {
        self.stale_close = StaleClose::Hidden;
        let intent = self.pending_close.take();
        if intent.is_some() {
            self.close_refused();
        }
        intent
    }

    /// A close line is read as "the document is going away", so edits behind it are dropped. When the editor then
    /// refuses the close (unsaved changes, a failed save, a tab not shown, a cancel) the document stays open: edits the
    /// agent writes after the refusal must land, so the reader goes back to what it was before the close line.
    pub fn close_refused(&mut self) {
        if let Some(before) = self.discarding_before_close.take() {
            self.discarding = before;
        }
    }

    fn drops_stale_close(&self, line_offset: u64) -> bool {
        match self.stale_close {
            StaleClose::Hidden => true,
            StaleClose::Before(before) => line_offset < before,
            StaleClose::None => false,
        }
    }
}

pub(crate) fn write_close_result_file(log_path: &Path, intent: &CloseIntent, mut body: serde_json::Value) {
    let Some(close_id) = intent.close_id else {
        return;
    };
    if let Some(fields) = body.as_object_mut() {
        fields.insert("close_id".into(), serde_json::json!(close_id));
        if let Some(tab_id) = intent.tab_id {
            fields.insert("tab_id".into(), serde_json::json!(tab_id));
        }
    }
    let Some(dir) = log_path.parent() else {
        return;
    };
    let path = dir.join(CloseIntent::RESULT_FILE);
    let Ok(bytes) = serde_json::to_vec(&body) else {
        return;
    };
    let _ = crate::agent_session_files::atomic_write_session_file(&path, &bytes);
}

/// What the user is told, once per document epoch, when an edit written by a newer build is read: what was
/// not applied, then the fix. `v` is the format version of that edit.
fn newer_edit_error(v: u32) -> String {
    format!("Agent edit not applied. {}", refusal_next_step(v))
}

/// The second sentence of a refusal toast: the fix for a newer log, the plain fact for a version nothing writes.
fn refusal_next_step(v: u32) -> String {
    if v > LIVE_PATCH_VERSION {
        UPDATE_BOTH_HINT.to_string()
    } else {
        log_version_refusal(v).unwrap_or_default()
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

    /// A line from a newer build, with an op this build has never heard of.
    fn newer_line() -> String {
        format!(r#"{{"v":{},"ops":[{{"from_the_future":{{"id":1}}}}]}}"#, LIVE_PATCH_VERSION + 1) + "\n"
    }

    /// A line written in format version 1, before the v0.6 operations.
    fn version_one_line(id: u64) -> String {
        let mut patch = LivePatch::new(vec![LiveOp::Remove { id: NodeId(id) }]);
        patch.v = 1;
        serde_json::to_string(&patch).unwrap() + "\n"
    }

    fn ids(lines: &[LiveLogLine]) -> Vec<u64> {
        lines
            .iter()
            .flat_map(|l| match l {
                LiveLogLine::Patch(p) => p.ops.iter().collect(),
                LiveLogLine::DocEpoch(_) | LiveLogLine::Close(_) | LiveLogLine::Newer { .. } => Vec::new(),
            })
            .map(|op| if let LiveOp::Remove { id } = op { id.0 } else { 0 })
            .collect()
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

    fn pending_ids(log: &LivePatchLog) -> Vec<u64> {
        log.pending.iter().flat_map(|p| &p.ops).map(|op| if let LiveOp::Remove { id } = op { id.0 } else { 0 }).collect()
    }

    /// Reads what the agent appended and hands it to `ingest`, as the editor does each frame.
    fn read(log: &mut LivePatchLog) -> Vec<String> {
        let (lines, poll_errors) = log.poll();
        let mut errors = poll_errors;
        errors.extend(log.ingest(lines));
        errors
    }

    fn newer_message() -> String {
        format!("Agent edit not applied. {UPDATE_BOTH_HINT}")
    }

    #[test]
    fn an_edit_before_a_newer_one_applies_and_every_edit_after_it_waits_for_the_next_switch() {
        let path = temp_log("fence");
        let mut log = LivePatchLog::bind(path.clone());
        append(&path, &(line(1) + &newer_line() + &line(2) + &newer_line() + &line(3)));
        let errors = read(&mut log);
        assert_eq!(errors, [newer_message()], "one message for the epoch");
        assert!(errors[0].split_whitespace().count() <= 14, "a toast, not a paragraph: {}", errors[0]);
        assert!(!errors[0].contains('('), "no parenthetical in the toast: {}", errors[0]);
        assert_eq!(pending_ids(&log), [1], "the edit before the newer one is applied, nothing after it is");
        assert_eq!(log.fenced_dropped, 2, "the readable edits behind the fence are counted, not lost silently");
        assert_eq!(log.fence_notice, Some(newer_message()), "kept for the apply step, which shows it after the edits that did apply");
    }

    #[test]
    fn an_edit_to_a_layer_a_newer_edit_may_have_removed_is_not_held() {
        // A newer build's line removes layer 7 (this build cannot tell); the next line, in a format this
        // build reads, renames layer 7. Applying it would act on a layer in an unknown state.
        let path = temp_log("fence-update");
        let mut log = LivePatchLog::bind(path.clone());
        let mut set = serde_json::Map::new();
        set.insert("name".into(), "Renamed".into());
        let rename = serde_json::to_string(&LivePatch::new(vec![LiveOp::Update { id: NodeId(7), set, unset: Vec::new() }])).unwrap() + "\n";
        append(&path, &(newer_line() + &rename));
        assert_eq!(read(&mut log), [newer_message()]);
        assert!(log.pending.is_empty(), "the later edit is not held: {:?}", log.pending);
        assert_eq!(log.fenced_dropped, 1);
    }

    #[test]
    fn the_message_shows_once_per_epoch_even_when_the_newer_edits_arrive_in_different_polls() {
        let path = temp_log("fence-polls");
        let mut log = LivePatchLog::bind(path.clone());
        append(&path, &newer_line());
        assert_eq!(read(&mut log), [newer_message()]);
        append(&path, &newer_line());
        assert!(read(&mut log).is_empty(), "the second poll says nothing more");
        append(&path, &line(5));
        assert!(read(&mut log).is_empty());
        assert!(log.pending.is_empty());
        assert_eq!(log.fenced_dropped, 1);
    }

    #[test]
    fn a_document_switch_lifts_the_fence_and_the_next_newer_edit_is_reported_again() {
        let path = temp_log("fence-switch");
        let mut log = LivePatchLog::bind(path.clone());
        let mut doc = Document::new("Switched");
        doc.add_page("Page");
        let (mark, bytes) = switch_line(&doc, 1);
        std::fs::write(path.with_file_name("doc-sync.designos"), bytes).unwrap();
        append(&path, &(newer_line() + &line(4)));
        assert_eq!(read(&mut log), [newer_message()]);
        assert_eq!(log.fenced_dropped, 1);
        // The agent opens another document: its edits apply again, and the count starts over.
        append(&path, &(mark + &line(7)));
        assert!(read(&mut log).is_empty(), "no message for edits that apply");
        assert_eq!(pending_ids(&log), [7]);
        assert_eq!(log.fenced_dropped, 0);
        assert_eq!(log.fence_notice, None, "a stale notice must not outlive the epoch it belongs to");
        assert_eq!(log.pending_doc.as_ref().map(|d| d.name.as_str()), Some("Switched"));
        // A newer edit on the new document is a new report.
        append(&path, &(newer_line() + &line(8)));
        assert_eq!(read(&mut log), [newer_message()]);
        assert_eq!(pending_ids(&log), [7]);
    }

    #[test]
    fn a_recreated_log_is_a_new_epoch_for_the_message() {
        let path = temp_log("fence-recreate");
        let mut log = LivePatchLog::bind(path.clone());
        append(&path, &(newer_line() + &line(1)));
        assert_eq!(read(&mut log), [newer_message()]);
        std::fs::remove_file(&path).unwrap();
        append(&path, &(line(2) + &newer_line() + &line(3)));
        assert_eq!(read(&mut log), [newer_message()], "the new session's newer edit is reported");
        assert_eq!(pending_ids(&log), [2], "and its earlier edit applies");
        assert_eq!(log.fenced_dropped, 1);
    }

    #[test]
    fn a_newer_edit_that_reaches_ingest_is_refused_too() {
        let mut log = LivePatchLog::bind(temp_log("newer-ingest"));
        let errors = log.ingest(vec![LiveLogLine::Newer { v: LIVE_PATCH_VERSION + 1 }]);
        assert_eq!(errors.len(), 1, "{errors:?}");
        assert_eq!(errors[0], format!("Agent edit not applied. {UPDATE_BOTH_HINT}"));
        assert!(log.pending.is_empty(), "nothing is held for replay");
    }

    #[test]
    fn version_one_edits_still_read() {
        let path = temp_log("version-one");
        let mut log = LivePatchLog::bind(path.clone());
        append(&path, &(version_one_line(3) + &line(4)));
        let (lines, errors) = log.poll();
        assert_eq!(ids(&lines), [3, 4]);
        assert!(errors.is_empty(), "{errors:?}");
        assert!(log.ingest(lines).is_empty());
        assert_eq!(log.pending.len(), 2);
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

    #[test]
    fn only_a_complete_unread_line_counts_as_unread() {
        let path = temp_log("unread");
        append(&path, &line(1));
        let mut log = LivePatchLog::bind(path.clone());
        assert!(!log.has_unread_complete_line(), "lines from before the bind are not unread");
        let next = line(2);
        append(&path, &next[..10]);
        assert!(!log.has_unread_complete_line(), "a line without a newline yet does not count");
        append(&path, &next[10..]);
        assert!(log.has_unread_complete_line());
        assert_eq!(ids(&log.poll().0), [2]);
        assert!(!log.has_unread_complete_line(), "a poll reads the line");
    }

    #[test]
    fn a_recreated_log_and_a_discarded_oversized_line_are_judged_like_the_poll_does() {
        let path = temp_log("unread-rotate");
        append(&path, &line(1));
        let mut log = LivePatchLog::bind(path.clone());
        std::fs::remove_file(&path).unwrap();
        append(&path, &(line(7) + &line(8)));
        assert!(log.has_unread_complete_line(), "everything in a recreated log is unread");
        assert_eq!(ids(&log.poll().0), [7, 8]);
        log.max_line_bytes = 64;
        append(&path, &"x".repeat(200));
        let _ = log.poll();
        assert!(log.skipping);
        append(&path, "\n");
        assert!(!log.has_unread_complete_line(), "the tail of an oversized line is dropped, so it must not hold the canvas back");
    }

    #[test]
    fn a_normal_line_after_an_oversized_tail_and_a_log_recreated_meanwhile_count_as_unread() {
        let path = temp_log("unread-skipping");
        append(&path, &line(1));
        let mut log = LivePatchLog::bind(path.clone());
        log.max_line_bytes = 64;
        append(&path, &"x".repeat(200));
        let _ = log.poll();
        assert!(log.skipping);
        assert!(!log.has_unread_complete_line(), "the unfinished oversized line is not an edit");
        append(&path, &format!("tail\n{}", line(2)));
        assert!(log.has_unread_complete_line(), "the line after the oversized tail is an unread edit");
        assert_eq!(ids(&log.poll().0), [2]);
        assert!(!log.skipping);

        append(&path, &"y".repeat(200));
        let _ = log.poll();
        assert!(log.skipping);
        std::fs::remove_file(&path).unwrap();
        append(&path, &line(3));
        assert!(log.has_unread_complete_line(), "a log recreated while skipping is unread from its start");
        assert_eq!(ids(&log.poll().0), [3]);
    }

    #[test]
    fn the_unread_check_stops_at_the_first_newline_and_ignores_a_line_over_the_limit() {
        let path = temp_log("unread-bounded");
        let mut log = LivePatchLog::bind(path.clone());
        log.max_line_bytes = 64;
        append(&path, &"z".repeat(300_000));
        assert!(!log.has_unread_complete_line(), "an unfinished line past the limit is never applied");
        append(&path, "\n");
        assert!(!log.has_unread_complete_line(), "a complete line over the limit is reported by the poll, not applied");
        let mut log = LivePatchLog::bind(temp_log("unread-bounded-ok"));
        append(&log.path.clone(), &(line(5) + &"q".repeat(300_000)));
        assert!(log.has_unread_complete_line(), "the first newline decides, whatever follows it");
        assert_eq!(ids(&log.poll().0), [5]);
    }

    fn switch_line(doc: &Document, epoch: u64) -> (String, Vec<u8>) {
        let bytes = doc.to_json().unwrap().into_bytes();
        (serde_json::to_string(&DocEpoch::of(epoch, &bytes)).unwrap() + "\n", bytes)
    }

    #[test]
    fn a_document_switch_drops_the_edits_held_before_it_and_keeps_those_after_it() {
        let path = temp_log("switch");
        let mut log = LivePatchLog::bind(path.clone());
        let mut doc = Document::new("Switched");
        doc.add_page("Page");
        let (mark, bytes) = switch_line(&doc, 1);
        std::fs::write(path.with_file_name("doc-sync.designos"), bytes).unwrap();
        append(&path, &(line(1) + &line(2) + &mark + &line(3)));
        let (lines, errors) = log.poll();
        assert!(errors.is_empty(), "{errors:?}");
        assert!(log.ingest(lines).is_empty());
        assert_eq!(log.pending.iter().flat_map(|p| &p.ops).count(), 1, "only the edit after the switch is held");
        assert_eq!(log.dropped_old_epoch, 2);
        assert_eq!(log.pending_doc.as_ref().map(|d| d.name.as_str()), Some("Switched"));
    }

    #[test]
    fn a_version_one_document_switch_still_loads() {
        let path = temp_log("switch-v1");
        let mut log = LivePatchLog::bind(path.clone());
        let mut doc = Document::new("Old switch");
        doc.add_page("Page");
        let bytes = doc.to_json().unwrap().into_bytes();
        let mark = DocEpoch { v: 1, ..DocEpoch::of(1, &bytes) };
        std::fs::write(path.with_file_name("doc-sync.designos"), &bytes).unwrap();
        append(&path, &(serde_json::to_string(&mark).unwrap() + "\n"));
        let (lines, _) = log.poll();
        assert!(log.ingest(lines).is_empty());
        assert_eq!(log.pending_doc.as_ref().map(|d| d.name.as_str()), Some("Old switch"));
    }

    #[test]
    fn a_switch_whose_document_was_overwritten_by_a_later_one_waits_for_that_later_switch() {
        let path = temp_log("superseded");
        let mut log = LivePatchLog::bind(path.clone());
        let mut first = Document::new("First");
        first.add_page("Page");
        let mut second = Document::new("Second");
        second.add_page("Page");
        let (mark1, _) = switch_line(&first, 1);
        let (mark2, bytes2) = switch_line(&second, 2);
        // Only the second document is on disk when the editor reads the first switch.
        std::fs::write(path.with_file_name("doc-sync.designos"), bytes2).unwrap();
        append(&path, &(mark1 + &line(1)));
        let (lines, _) = log.poll();
        assert!(log.ingest(lines).is_empty(), "a superseded switch is not an error");
        assert!(log.pending.is_empty() && log.pending_doc.is_none());
        assert_eq!(log.dropped_old_epoch, 1, "the edit made on the first document is dropped");
        append(&path, &(mark2 + &line(2)));
        let (lines, _) = log.poll();
        assert!(log.ingest(lines).is_empty());
        assert_eq!(log.pending_doc.as_ref().map(|d| d.name.as_str()), Some("Second"));
        assert_eq!(log.pending.len(), 1);
    }

    #[test]
    fn a_switch_to_a_document_that_cannot_be_read_is_reported_and_its_edits_are_dropped() {
        let path = temp_log("unreadable");
        let mut log = LivePatchLog::bind(path.clone());
        let mut doc = Document::new("Gone");
        doc.add_page("Page");
        let (mark, _) = switch_line(&doc, 1);
        append(&path, &(mark + &line(1)));
        let (lines, _) = log.poll();
        let errors = log.ingest(lines);
        assert_eq!(errors.len(), 2, "{errors:?}");
        assert!(errors[0].contains("Cannot read the document"), "{}", errors[0]);
        assert!(errors[1].contains("1 agent edit(s) were not applied"), "{}", errors[1]);
        assert!(log.pending.is_empty() && log.pending_doc.is_none());

        let mut next = Document::new("Recovered");
        next.add_page("Page");
        let (mark2, bytes2) = switch_line(&next, 2);
        std::fs::write(path.with_file_name("doc-sync.designos"), bytes2).unwrap();
        append(&path, &(mark2 + &line(2)));
        let (lines, _) = log.poll();
        assert!(log.ingest(lines).is_empty());
        assert_eq!(log.dropped_old_epoch, 0, "edits already reported during load failure must not be counted again");
    }

    #[test]
    fn edits_dropped_while_stuck_belong_to_the_switch_that_loads_later_in_the_same_batch() {
        let path = temp_log("stuck-then-switch");
        let mut log = LivePatchLog::bind(path.clone());
        let mut doc = Document::new("Recovered");
        doc.add_page("Page");
        let (good, bytes) = switch_line(&doc, 2);
        std::fs::write(path.with_file_name("doc-sync.designos"), bytes).unwrap();
        let unsupported = serde_json::to_string(&DocEpoch { v: LIVE_PATCH_VERSION + 1, doc_epoch: 1, bytes: 0, hash: 0 }).unwrap() + "\n";
        append(&path, &(unsupported + &line(1) + &good + &line(2)));
        let (lines, _) = log.poll();
        let errors = log.ingest(lines);
        assert_eq!(errors.len(), 1, "the switch that loads reports the dropped edit, so no second error: {errors:?}");
        assert_eq!(errors[0], format!("Agent document not opened. {UPDATE_BOTH_HINT}"));
        assert_eq!(log.dropped_old_epoch, 1, "the edit made while stuck is reported with the switch that follows");
        assert_eq!(log.pending.iter().flat_map(|p| &p.ops).count(), 1, "only the edit after the loaded switch is held");
        assert_eq!(log.pending_doc.as_ref().map(|d| d.name.as_str()), Some("Recovered"));
    }

    #[test]
    fn a_rewritten_log_resets_dropped_edit_count_when_replayed_from_zero() {
        let path = temp_log("rewrite-dropped-count");
        let mut log = LivePatchLog::bind(path.clone());
        let mut doc = Document::new("Switched");
        doc.add_page("Page");
        let (mark, bytes) = switch_line(&doc, 1);
        std::fs::write(path.with_file_name("doc-sync.designos"), bytes).unwrap();
        append(&path, &(line(1) + &line(2) + &mark));
        let (lines, _) = log.poll();
        assert!(log.ingest(lines).is_empty());
        assert_eq!(log.dropped_old_epoch, 2);

        // Truncate/rewrite the log in place and replay from 0 with 1 edit before the switch.
        std::fs::write(&path, line(10) + &mark).unwrap();
        let (lines, _) = log.poll();
        assert!(log.ingest(lines).is_empty());
        assert_eq!(log.dropped_old_epoch, 1, "rewritten log must reset prior dropped_old_epoch count");
    }

    fn close_line() -> String {
        serde_json::to_string(&CloseIntent::of("fail", None)).unwrap() + "\n"
    }

    #[test]
    fn edits_after_a_refused_close_are_held_not_dropped() {
        let path = temp_log("refused-close");
        let mut log = LivePatchLog::bind(path.clone());
        append(&path, &(line(1) + &close_line() + &line(2)));
        read(&mut log);
        assert!(log.pending_close.is_some(), "the close is held for the editor to answer");
        assert_eq!(pending_ids(&log), vec![1], "an edit behind a close line is for a document that is going away");
        assert_eq!(log.dropped_old_epoch, 1);

        // The editor refused: the document stayed open, and the agent writes again once it has read the answer.
        log.pending_close = None;
        log.close_refused();
        append(&path, &line(3));
        read(&mut log);
        assert_eq!(pending_ids(&log), vec![1, 3], "an edit after a refused close lands");
        assert_eq!(log.dropped_old_epoch, 1);
    }

    #[test]
    fn a_close_dropped_because_the_tab_was_hidden_stops_dropping_edits() {
        let path = temp_log("hidden-close");
        let mut log = LivePatchLog::bind(path.clone());
        append(&path, &close_line());
        read(&mut log);
        assert!(log.drop_close_on_hide().is_some());
        append(&path, &line(4));
        read(&mut log);
        assert_eq!(pending_ids(&log), vec![4]);
    }

    #[test]
    fn a_refused_close_does_not_undo_a_drop_that_was_already_on() {
        let path = temp_log("refused-close-while-stuck");
        let mut log = LivePatchLog::bind(path.clone());
        log.discarding = true;
        append(&path, &close_line());
        read(&mut log);
        log.pending_close = None;
        log.close_refused();
        append(&path, &line(5));
        read(&mut log);
        assert!(log.pending.is_empty(), "edits stay dropped when they were being dropped before the close line");
    }
}
