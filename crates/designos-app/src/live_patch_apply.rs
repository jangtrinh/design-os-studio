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
    app.agent_presence.reset();
    app.agent_changes_chip.clear();
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
    let expected_marks_path = app.terminal.live_patch.path.with_file_name("turn-marks.jsonl");
    if app.terminal.turn_marks.path() != expected_marks_path {
        app.terminal.turn_marks = crate::agent_turn_marks::TurnMarkLog::bind(expected_marks_path);
    }
    let marks = app.terminal.turn_marks.poll();
    if !marks.is_empty() {
        app.terminal.active_turn_ledger_mut().ingest_marks(marks, std::time::Instant::now());
    }
    if user_edit_in_progress(app) {
        return;
    }
    if let Some((tab_idx, turn_id)) = app.terminal.pending_turn_revert.take() {
        crate::agent_turn_card::execute_revert(app, tab_idx, turn_id);
    }
    if !app.terminal.live_patch.has_held() {
        return;
    }
    let epoch_switched = std::mem::take(&mut app.terminal.live_patch.pending_epoch_switch);
    let mut dropped = 0;
    if let Some(doc) = app.terminal.live_patch.pending_doc.take() {
        dropped = std::mem::take(&mut app.terminal.live_patch.dropped_old_epoch);
        replace_live_document(app, doc);
        app.terminal.active_turn_ledger_mut().on_doc_epoch(std::time::Instant::now());
    } else if epoch_switched {
        app.terminal.active_turn_ledger_mut().on_doc_epoch(std::time::Instant::now());
    }
    let patches = std::mem::take(&mut app.terminal.live_patch.pending);
    let offsets = std::mem::take(&mut app.terminal.live_patch.pending_offsets);
    if !patches.is_empty() {
        app.terminal.status_msg = Some(format!("Applied {} live MCP edit(s) from {}", patches.len(), app.terminal.live_patch.path.display()));
        apply_live_patches_with_offsets(app, &patches, &offsets);
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
#[cfg(test)]
pub fn apply_live_patches(app: &mut App, patches: &[LivePatch]) {
    apply_live_patches_with_offsets(app, patches, &[]);
}

/// Replays `patches` on `app.doc`, associating each patch with its start byte offset in
/// `doc-patch.jsonl` when provided.
pub fn apply_live_patches_with_offsets(app: &mut App, patches: &[LivePatch], offsets: &[u64]) {
    let mut total = LivePatchReport::default();
    let mut committed_any = false;
    let mut failures = Vec::new();
    let (default_sid, default_label) = app.terminal.active_agent_session_identity();
    let mut batch_turns: Vec<(String, u64)> = Vec::new();
    for (idx, patch) in patches.iter().enumerate() {
        let patch_offset = offsets.get(idx).copied().unwrap_or(idx as u64);
        match apply_one(app, patch, patch_offset) {
            Ok((report, committed)) => {
                if committed {
                    let sid = patch.session_id.as_deref().unwrap_or(&default_sid);
                    let clean_sid = if sid.trim().is_empty() { "default" } else { sid.trim() };
                    let turn_id = if let Some((_, t)) = batch_turns.iter().find(|(s, _)| s == clean_sid) {
                        *t
                    } else {
                        let t = app.agent_changes_chip.begin_next_turn(clean_sid);
                        batch_turns.push((clean_sid.to_string(), t));
                        t
                    };
                    let label = patch.agent_label.as_deref().unwrap_or(&default_label);
                    app.agent_presence.record_patch(sid, label, &app.doc, None, &report, app.now);
                    let colour_idx = app.agent_presence.sessions.iter().find(|s| s.id == clean_sid).map(|s| s.colour_index).unwrap_or(0);
                    app.agent_changes_chip.record_patch(clean_sid, colour_idx, Some(turn_id), &app.doc, &report, app.view, app.canvas_rect);
                }
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

fn collect_command_node_ids(cmd: &designos_doc::Command, out: &mut Vec<u64>) {
    match cmd {
        designos_doc::Command::Insert { nodes, .. } | designos_doc::Command::InsertPage { nodes, .. } => {
            out.extend(nodes.iter().map(|n| n.id.0));
        }
        designos_doc::Command::Remove { id } | designos_doc::Command::Reparent { id, .. } | designos_doc::Command::RemovePage { id } => {
            out.push(id.0);
        }
        designos_doc::Command::Update { node } => {
            out.push(node.id.0);
        }
        designos_doc::Command::Batch(cmds) => {
            for c in cmds {
                collect_command_node_ids(c, out);
            }
        }
        _ => {}
    }
}

fn apply_one(app: &mut App, patch: &LivePatch, patch_offset: u64) -> Result<(LivePatchReport, bool), String> {
    let saved = app.terminal.live_patch.state.clone();
    let (pre_names, pre_fps) = crate::agent_turn_ledger::snapshot_pre_step(&app.doc, patch);
    let mut tx = Tx::new();
    let mut touched_ids = Vec::new();
    let replayed = replay_live_patch(&mut app.doc, patch, &mut app.terminal.live_patch.state, &mut |doc, cmd| {
        collect_command_node_ids(&cmd, &mut touched_ids);
        tx.cmd(doc, cmd)
    });
    let report = match replayed {
        Ok(r) => r,
        Err(e) => {
            tx.cancel(&mut app.doc);
            return Err(e.to_string());
        }
    };
    let committed = tx.commit_labeled(&mut app.doc, &mut app.hist, patch.label.clone()).map_err(|e| {
        // Nothing of this patch landed, so neither do the id remaps it made.
        app.terminal.live_patch.state = saved;
        e.to_string()
    })?;
    if committed {
        relayout_touched(&mut app.doc, &report.needs_layout);
        if let Some(inv) = app.hist.peek_undo().cloned() {
            let now = std::time::Instant::now();
            let hist_depth = app.hist.undo_depth();
            let doc_ref = &app.doc;
            let hist_ref = &app.hist;
            app.terminal.active_turn_ledger_mut().record_step(doc_ref, hist_ref, patch_offset, hist_depth, inv, &report.touched, &pre_names, pre_fps, now);
        }
        app.terminal.record_touched_layers(&touched_ids);
    }
    Ok((report, committed))
}

fn merge(total: &mut LivePatchReport, r: LivePatchReport) {
    total.inserted_roots.extend(r.inserted_roots);
    total.needs_layout.extend(r.needs_layout);
    total.touched.extend(r.touched);
    total.removed_parents.extend(r.removed_parents);
    total.applied += r.applied;
    total.duplicates += r.duplicates;
    total.skipped.extend(r.skipped);
}

/// Lays out again, with their real children, the auto-layout containers an edit can have
/// changed: the node itself when it is one, and the unbroken chain of auto-layout ancestors
/// above it (Hug sizes propagate up). The agent laid them out on a copy that lacked children.
pub(crate) fn relayout_touched(doc: &mut Document, needs_layout: &[NodeId]) {
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
