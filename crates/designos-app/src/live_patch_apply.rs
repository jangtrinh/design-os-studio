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
    app.terminal.set_auto_fix_user(app.check_designs.user_auto_fix.clone());
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
    let mut auto_fix_toast: Option<String> = None;
    for patch in patches {
        let is_auto_fix = patch.auto_fix_report.is_some() || patch.label.as_deref().is_some_and(|l| l.starts_with("Auto-fix"));
        if is_auto_fix {
            let effective = app.effective_auto_fix();
            if effective.mode == designos_doc::AutoFixMode::Off {
                continue;
            }
            let rows: Vec<designos_doc::live_patch::FixRow> = patch
                .auto_fix_report
                .clone()
                .unwrap_or_default()
                .into_iter()
                .filter(|r| effective.rule_enabled(&r.rule))
                .map(|mut r| {
                    r.node = app.terminal.live_patch.state.target(&app.doc, r.node);
                    if r.node_name.is_empty() {
                        if let Some(n) = app.doc.get(r.node) {
                            r.node_name = n.name.clone();
                        }
                    }
                    r
                })
                .collect();
            let report_only =
                effective.mode == designos_doc::AutoFixMode::ReportOnly || patch.ops.is_empty() || patch.label.as_deref() == Some("Auto-fix (report only)");
            if report_only {
                if !rows.is_empty() {
                    let n = rows.len();
                    app.check_designs.record_auto_fix("Auto-fix (report only)", true, rows);
                    app.terminal.set_auto_fix_notice(n, true);
                    if !app.check_designs.open {
                        auto_fix_toast = Some(format!("{n} {} available", if n == 1 { "fix" } else { "fixes" }));
                    }
                }
                continue;
            }
            match apply_one(app, patch) {
                Ok((report, committed)) => {
                    if committed {
                        app.hist.set_top_undo_label(patch.label.as_deref().unwrap_or("Auto-fix"));
                        committed_any = true;
                        merge(&mut total, report);
                    }
                    if !rows.is_empty() {
                        let n = rows.len();
                        let label = patch.label.clone().unwrap_or_else(|| "Auto-fix".to_string());
                        app.check_designs.record_auto_fix(label, false, rows);
                        app.terminal.set_auto_fix_notice(n, false);
                        if !app.check_designs.open {
                            auto_fix_toast = Some(format!("Auto-fixed {n} {}", if n == 1 { "issue" } else { "issues" }));
                        }
                    }
                }
                Err(e) => failures.push(e),
            }
            continue;
        }
        match apply_one(app, patch) {
            Ok((report, committed)) => {
                if committed {
                    if let Some(label) = &patch.label {
                        app.hist.set_top_undo_label(label.clone());
                    }
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
    } else if let Some(msg) = auto_fix_toast {
        app.show_toast(msg, false);
    } else if let Some(msg) = toast_text(&total, committed_any) {
        app.show_toast(msg, !total.skipped.is_empty());
    }
}

fn apply_one(app: &mut App, patch: &LivePatch) -> Result<(LivePatchReport, bool), String> {
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
