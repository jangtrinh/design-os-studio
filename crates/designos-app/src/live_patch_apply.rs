//! Replays agent edits from `designos-mcp` (`doc-patch.jsonl`) on the open document.
//!
//! The agent may have edited a scoped copy of a large file, so its edits arrive as node-level
//! ops, and each tool call lands as one undoable transaction: only the inserted or changed nodes
//! move, the view stays where it is, and auto-layout containers the edit touched are laid out
//! again with their real children.

use crate::app::App;
use crate::canvas::Interaction;
use crate::edit::Tx;
use designos_doc::live_patch::{replay_live_patch, LivePatch, LivePatchReport, LivePatchState};
use designos_doc::{Document, NodeId};

/// The user is in the middle of an edit that holds an open transaction or a snapshot: a canvas
/// drag, a text session, a vector point drag, a property scrub or an open colour picker. An agent
/// edit applied now would land inside it, and cancelling the user's edit would drop it too.
pub fn user_edit_in_progress(app: &App) -> bool {
    !matches!(app.canvas.inter, Interaction::None)
        || app.canvas.text.is_some()
        || app.canvas.vedit.as_ref().is_some_and(|v| v.drag.is_some())
        || app.scrub.is_some()
        || app.props.picker.is_some()
        || app.canvas.builder.is_some()
}

/// Publishes the editor's document to the agent session, unless an edit is in progress or agent
/// changes are still held (see `TerminalState::sync_live_file_context`).
pub fn publish_live_context(app: &mut App) {
    app.terminal.edit_in_progress = user_edit_in_progress(app);
    app.terminal.sync_live_file_context(&app.doc, app.page, &app.selection, app.file_path.as_deref());
}

fn replace_live_document(app: &mut App, mut new_doc: Document) {
    crate::io::ensure_page(&mut new_doc);
    let keep_page = if new_doc.pages.contains(&app.page) { app.page } else { new_doc.pages[0] };
    app.doc = new_doc;
    app.page = keep_page;
    // Id remaps belong to the document that was replaced. Patches still held were made on the new one.
    app.terminal.live_patch.state = LivePatchState::default();
    app.sanitize();
    app.changed();
    app.fit_all();
    publish_live_context(app);
    app.show_toast("Applied live Agent changes to current page", false);
}

/// Opens the document the agent switched to and applies the edits it made on it, unless the
/// user is mid-edit. Runs every frame. What the log held for the previous document was already
/// dropped when the switch was read (`LivePatchLog::ingest`); the toast says how many.
pub fn apply_pending_live_patches(app: &mut App) {
    if !app.terminal.live_patch.has_held() || user_edit_in_progress(app) {
        return;
    }
    let mut dropped = 0;
    if let Some(doc) = app.terminal.live_patch.pending_doc.take() {
        dropped = std::mem::take(&mut app.terminal.live_patch.dropped_old_epoch);
        replace_live_document(app, doc);
    }
    let patches = std::mem::take(&mut app.terminal.live_patch.pending);
    if !patches.is_empty() {
        app.terminal.status_msg = Some(format!("Applied {} live MCP edit(s) from {}", patches.len(), app.terminal.live_patch.path.display()));
        apply_live_patches(app, &patches);
    }
    if dropped > 0 {
        note_dropped_edits(app, dropped);
    }
    publish_live_context(app);
}

/// Adds to the toast on screen that edits made on the previous document were dropped.
fn note_dropped_edits(app: &mut App, dropped: usize) {
    let edits = if dropped == 1 { "1 earlier edit".to_string() } else { format!("{dropped} earlier edits") };
    let note = format!("{edits} made on the previous document dropped");
    let text = app.toast.as_ref().map_or(note.clone(), |t| format!("{} ({note})", t.text));
    app.show_toast(text, true);
}

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
    app.terminal.live_patch.state.leases = app.terminal.leases.clone();
    let saved = app.terminal.live_patch.state.clone();
    let mut tx = Tx::new();
    let replayed = replay_live_patch(&mut app.doc, patch, &mut app.terminal.live_patch.state, &mut |doc, cmd| tx.cmd(doc, cmd));
    let report = match replayed {
        Ok(r) => r,
        Err(e) => {
            tx.cancel(&mut app.doc);
            return Err(e.to_string());
        }
    };
    let committed = tx.commit(&mut app.doc, &mut app.hist).map_err(|e| {
        // Nothing of this patch landed, so neither do the id remaps it made.
        app.terminal.live_patch.state = saved;
        e.to_string()
    })?;
    if committed {
        app.hist.tag_last_undo_step(patch.run_id.clone(), patch.label.clone());
        if let Some(rid) = patch.run_id.as_deref() {
            app.terminal.note_run_edit(rid);
        }
        relayout_touched(&mut app.doc, &report.needs_layout);
    }
    if report.skipped.iter().any(|s| s.contains("OUTSIDE_OWNED_FRAMES")) {
        if let Some(rid) = patch.run_id.as_deref() {
            app.terminal.mark_run_status(rid, "violated");
            rollback_agent_run(app, rid, &format!("Agent run '{rid}' violated frame ownership"));
        }
    }
    Ok((report, committed))
}

/// Selectively rolls back all undo steps belonging to `run_id` (`History::undo_run`),
/// releases its frame lease, and updates the canvas and toast (`§3.4`).
pub fn rollback_agent_run(app: &mut App, run_id: &str, reason: &str) -> usize {
    let reverted = app.hist.undo_run(&mut app.doc, run_id).unwrap_or(0);
    app.terminal.finish_run_rollback(run_id, reverted);
    if reverted > 0 {
        let pages = app.doc.pages.clone();
        for p in pages {
            app.doc.relayout_subtree(p);
        }
        app.doc.flush_pending_relayout();
        app.sanitize();
        app.changed();
        publish_live_context(app);
    }
    app.show_toast(format!("{reason} (rolled back {reverted} step(s))"), true);
    reverted
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

#[cfg(test)]
mod tests {
    use super::*;
    use designos_doc::live_patch::{Lease, LeasePurpose, LiveOp, LivePatch, SiblingHint};
    use designos_doc::{Document, Node, Rect};

    #[test]
    fn failed_run_rolls_back() {
        let mut doc = Document::new("rd3-app-test");
        let page_id = doc.add_page("Page 1");
        let mut app = App::new(doc);
        let frame_a = app.doc.alloc_id();
        let frame_b = app.doc.alloc_id();
        app.doc.insert(page_id, Node::frame(frame_a, Rect::new(0.0, 0.0, 400.0, 300.0))).unwrap();
        app.doc.insert(page_id, Node::frame(frame_b, Rect::new(500.0, 0.0, 400.0, 300.0))).unwrap();

        app.terminal.leases.grant(Lease::new("agent-a", vec![frame_a], LeasePurpose::Split)).unwrap();
        app.terminal.leases.grant(Lease::new("agent-b", vec![frame_b], LeasePurpose::Split)).unwrap();
        app.terminal.live_patch.state.leases = app.terminal.leases.clone();

        // Agent B writes 5 batches into frame_b; Agent A writes 2 batches into frame_a.
        let mut patches = Vec::new();
        for i in 0..2 {
            let nid = app.doc.alloc_id();
            let node = Node::rectangle(nid, Rect::new(i as f32 * 10.0, 0.0, 20.0, 20.0));
            let patch = LivePatch::new(vec![LiveOp::Insert { parent: frame_a, at: SiblingHint::default(), nodes: vec![node] }])
                .with_run_id(Some("agent-a".to_string()))
                .with_label(Some(format!("a-batch-{i}")));
            patches.push(patch);
        }
        for i in 0..5 {
            let nid = app.doc.alloc_id();
            let node = Node::rectangle(nid, Rect::new(i as f32 * 10.0, 0.0, 20.0, 20.0));
            let patch = LivePatch::new(vec![LiveOp::Insert { parent: frame_b, at: SiblingHint::default(), nodes: vec![node] }])
                .with_run_id(Some("agent-b".to_string()))
                .with_label(Some(format!("b-batch-{i}")));
            patches.push(patch);
        }

        apply_live_patches(&mut app, &patches);
        assert_eq!(app.doc.get(frame_a).unwrap().children.len(), 2);
        assert_eq!(app.doc.get(frame_b).unwrap().children.len(), 5);

        // Stub agent B exits with status 1 -> rollback_agent_run reverts its 5 batches automatically.
        let reverted = rollback_agent_run(&mut app, "agent-b", "sub-agent exited with code 1");
        assert_eq!(reverted, 5);
        assert_eq!(app.doc.get(frame_b).unwrap().children.len(), 0);
        assert_eq!(app.doc.get(frame_a).unwrap().children.len(), 2);
    }
}
