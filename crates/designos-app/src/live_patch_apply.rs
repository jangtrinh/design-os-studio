//! Replays agent edits from `designos-mcp` (`doc-patch.jsonl`) on the open document.
//!
//! The agent may have edited a scoped copy of a large file, so its edits arrive as node-level
//! ops, and each tool call lands as one undoable transaction: only the inserted or changed nodes
//! move, the view stays where it is, and auto-layout containers the edit touched are laid out
//! again with their real children.

use crate::app::App;
use crate::edit::Tx;
use designos_doc::live_patch::{replay_live_patch, LivePatch, LivePatchReport};
use designos_doc::{Document, NodeId};

/// Replays `patches` on `app.doc`, one undo step per patch (one agent tool call), and reports
/// the result in a toast. A patch that cannot be applied is reported and the rest still are: the
/// log has already been read past them.
pub fn apply_live_patches(app: &mut App, patches: &[LivePatch]) {
    let mut total = LivePatchReport::default();
    let mut committed_any = false;
    let mut failures = Vec::new();
    for patch in patches {
        match apply_one(app, patch) {
            Ok((report, committed)) => {
                committed_any |= committed;
                merge(&mut total, report);
            }
            Err(e) => failures.push(e),
        }
    }
    if committed_any {
        if !total.inserted_roots.is_empty() {
            app.selection = total.inserted_roots.clone();
        }
        app.sanitize();
        app.changed();
    }
    if let Some(first) = failures.first() {
        let msg = match patches.len() {
            1 => format!("Agent edit could not be applied: {first}"),
            n => format!("{} of {n} agent edits could not be applied: {first}", failures.len()),
        };
        app.show_toast(msg, true);
    } else if let Some(msg) = toast_text(&total, committed_any) {
        app.show_toast(msg, !total.skipped.is_empty());
    }
}

fn apply_one(app: &mut App, patch: &LivePatch) -> Result<(LivePatchReport, bool), String> {
    let mut tx = Tx::new();
    let replayed = replay_live_patch(&mut app.doc, patch, &mut app.terminal.live_patch.state, &mut |doc, cmd| tx.cmd(doc, cmd));
    let report = match replayed {
        Ok(r) => r,
        Err(e) => {
            tx.cancel(&mut app.doc);
            return Err(e.to_string());
        }
    };
    let committed = tx.commit(&mut app.doc, &mut app.hist).map_err(|e| e.to_string())?;
    if committed {
        relayout_touched(&mut app.doc, &report.needs_layout);
    }
    Ok((report, committed))
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
