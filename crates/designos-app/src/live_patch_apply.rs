//! Replays agent edits from `designos-mcp` (`doc-patch.jsonl`) on the open document.
//!
//! The agent may have edited a scoped copy of a large file, so its edits arrive as node-level
//! ops and land as one undoable transaction: only the inserted or changed nodes move, the view
//! stays where it is, and auto-layout containers the edit touched are laid out again with their
//! real children.

use crate::app::App;
use crate::edit::Tx;
use designos_doc::live_patch::{replay_live_patch, LivePatch, LivePatchReport, LivePatchState};
use designos_doc::{Document, NodeId};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

/// Read position in a session's append-only patch log, plus replay state across patches.
#[derive(Debug)]
pub struct LivePatchLog {
    pub path: PathBuf,
    /// Bytes already consumed. Starts at the current length so lines from earlier runs are not
    /// replayed into a newly opened file.
    offset: u64,
    pub state: LivePatchState,
}

impl LivePatchLog {
    pub fn bind(path: PathBuf) -> Self {
        let offset = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
        Self { path, offset, state: LivePatchState::default() }
    }

    /// Complete patch lines appended since the last poll; malformed lines are returned as errors.
    pub fn poll(&mut self) -> (Vec<LivePatch>, Vec<String>) {
        let Ok(len) = std::fs::metadata(&self.path).map(|m| m.len()) else { return Default::default() };
        if len < self.offset {
            // The log was recreated: everything in it is new.
            self.offset = 0;
        }
        if len == self.offset {
            return Default::default();
        }
        let mut buf = Vec::new();
        let read = std::fs::File::open(&self.path).and_then(|mut f| {
            f.seek(SeekFrom::Start(self.offset))?;
            f.take(len - self.offset).read_to_end(&mut buf)
        });
        if read.is_err() {
            return Default::default();
        }
        // A line still being written has no newline yet; leave it for the next poll.
        let Some(end) = buf.iter().rposition(|b| *b == b'\n') else { return Default::default() };
        self.offset += end as u64 + 1;
        let (mut patches, mut errors) = (Vec::new(), Vec::new());
        for line in buf[..end].split(|b| *b == b'\n').filter(|l| !l.iter().all(u8::is_ascii_whitespace)) {
            match serde_json::from_slice::<LivePatch>(line) {
                Ok(p) => patches.push(p),
                Err(e) => errors.push(format!("Unreadable agent edit in {}: {e}", display_name(&self.path))),
            }
        }
        (patches, errors)
    }
}

fn display_name(path: &Path) -> String {
    path.file_name().and_then(|n| n.to_str()).unwrap_or("doc-patch.jsonl").to_string()
}

/// Replays `patches` on `app.doc` as one undo step and reports the result in a toast.
pub fn apply_live_patches(app: &mut App, patches: &[LivePatch]) {
    let mut tx = Tx::new();
    let mut total = LivePatchReport::default();
    for patch in patches {
        let replayed = replay_live_patch(&mut app.doc, patch, &mut app.terminal.live_patch.state, &mut |doc, cmd| tx.cmd(doc, cmd));
        match replayed {
            Ok(r) => merge(&mut total, r),
            Err(e) => {
                tx.cancel(&mut app.doc);
                app.show_toast(format!("Agent edit could not be applied: {e}"), true);
                return;
            }
        }
    }
    let committed = match tx.commit(&mut app.doc, &mut app.hist) {
        Ok(c) => c,
        Err(e) => {
            app.show_toast(format!("Agent edit could not be applied: {e}"), true);
            return;
        }
    };
    if committed {
        relayout_touched(&mut app.doc, &total.needs_layout);
        if !total.inserted_roots.is_empty() {
            app.selection = total.inserted_roots.clone();
        }
        app.sanitize();
        app.changed();
    }
    if let Some(msg) = toast_text(&total, committed) {
        app.show_toast(msg, !total.skipped.is_empty());
    }
}

fn merge(total: &mut LivePatchReport, r: LivePatchReport) {
    total.inserted_roots.extend(r.inserted_roots);
    total.needs_layout.extend(r.needs_layout);
    total.applied += r.applied;
    total.duplicates += r.duplicates;
    total.skipped.extend(r.skipped);
}

/// Lays out again, with their real children, the auto-layout containers an edit can have
/// changed: the node itself when it is one, and the unbroken chain of auto-layout ancestors
/// above it (Hug sizes propagate up). The agent laid them out on a copy that lacked children.
fn relayout_touched(doc: &mut Document, needs_layout: &[NodeId]) {
    let is_auto = |doc: &Document, id: NodeId| doc.get(id).and_then(|n| n.kind.frame_props()).is_some_and(|f| f.auto_layout.is_some());
    let mut roots: Vec<NodeId> = Vec::new();
    for &id in needs_layout {
        let mut top = is_auto(doc, id).then_some(id);
        let mut cur = doc.get(id).and_then(|n| n.parent);
        while let Some(c) = cur.filter(|c| is_auto(doc, *c)) {
            top = Some(c);
            cur = doc.get(c).and_then(|n| n.parent);
        }
        if let Some(t) = top.filter(|t| !roots.contains(t)) {
            roots.push(t);
        }
    }
    for r in roots {
        doc.relayout_subtree(r);
    }
}

fn toast_text(r: &LivePatchReport, committed: bool) -> Option<String> {
    if let Some(first) = r.skipped.first() {
        let more = r.skipped.len() - 1;
        let suffix = if more > 0 { format!(" (+{more} more)") } else { String::new() };
        return Some(format!("Part of the agent edit was skipped: {first}{suffix}"));
    }
    if !committed {
        return None;
    }
    let added = r.inserted_roots.len();
    Some(match added {
        0 => "Applied agent edit".to_string(),
        1 => "Applied agent edit: 1 new layer".to_string(),
        n => format!("Applied agent edit: {n} new layers"),
    })
}
