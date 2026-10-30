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
    app.doc.prune_in_progress();
    app.terminal.current_undo_depth = app.hist.undo_depth();
    app.terminal.edit_in_progress = user_edit_in_progress(app);
    app.terminal.sync_live_file_context(&app.doc, app.page, &app.selection, app.file_path.as_deref());
    #[cfg(test)]
    app.terminal.flush_live_sync();
}

/// Puts the document the agent opened on screen. The agent's document never takes the file of the one it replaces: a
/// save of it would write the agent's document over the user's file. A library file is written first, and
/// then the document has nothing unsaved. A document outside the library with edits no save has written is kept in
/// a tab of its own, with its file, and the agent's document opens in the tab the agent works in. There is no
/// question, so a running agent never waits for one.
fn replace_live_document(app: &mut App, mut new_doc: Document) {
    crate::io::ensure_page(&mut new_doc);
    app.commit_open_edits();
    app.stop_plugin();
    app.exit_version_view();
    app.flush_autosave();
    if app.home.is_none() && !app.tabs.parked_in_home && app.tabs.is_hist_in_app() && app.has_unsaved_edits() {
        let mut tabs = std::mem::take(&mut app.tabs);
        tabs.show_agent_document_beside_the_users(app, new_doc);
        app.tabs = tabs;
    } else {
        let keep_page = if new_doc.pages.contains(&app.page) { app.page } else { new_doc.pages[0] };
        app.load_next_id = new_doc.peek_next_id();
        app.doc = new_doc;
        app.page = keep_page;
        app.file_path = None;
        app.lib_file = None;
        app.autosave.adopt_disk_state(Default::default(), None);
        // The user's steps were made on the document that went. Replayed on this one they would hit layers that only
        // share an id with the ones they changed, so the agent's document starts a history of its own, as when a file is opened.
        app.hist = designos_doc::History::default();
        app.history.reset();
        app.selection.clear();
        app.decks = Default::default();
    }
    // Either way the history on screen is a new one, at depth 0: a turn that began on the document left behind began above it.
    let depth = app.hist.undo_depth();
    for tab in &mut app.terminal.tabs {
        tab.turn_start_undo_depth = tab.turn_start_undo_depth.min(depth);
    }
    // No undo step comes with a whole document, so the history cannot tell this is a change.
    app.mark_edited_outside_history();
    // Id remaps belong to the document that was replaced. Patches still held were made on the new one.
    app.terminal.live_patch.state = LivePatchState::default();
    app.agent_presence.reset();
    app.agent_changes_chip.clear();
    app.agent_busy_frames.clear();
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
    app.terminal.poll_auto_allow_into_active();
    let expected_marks_path = app.terminal.hook_paths().turn_marks();
    if app.terminal.turn_marks.path() != expected_marks_path {
        app.terminal.turn_marks = crate::agent_turn_marks::TurnMarkLog::bind(expected_marks_path);
    }
    let marks = app.terminal.turn_marks.poll();
    app.terminal.active_turn_ledger_mut().ingest_marks(marks, std::time::Instant::now());
    if user_edit_in_progress(app) {
        return;
    }
    // A question about the shown document (Save changes, File changed on disk, and any other) is answered about the
    // document and the tab it was asked on, and the agent's document changes what both hold: it waits for the answer.
    // Edits made on the document it opened wait behind it.
    if (app.terminal.live_patch.pending_doc.is_some() || app.terminal.live_patch.pending_close.is_some()) && (app.leave.is_asking() || app.dialog.is_some()) {
        return;
    }
    if let Some((tab_idx, turn_id)) = app.terminal.pending_turn_revert.take() {
        crate::agent_turn_card::execute_revert(app, tab_idx, turn_id);
    }
    // A stopped turn is closed once its held patches are in: fold its agent undo steps into one and clear its flags (`#684`).
    let pending_close = app.terminal.pending_stop_close.take();
    let mut pending_depth = app.terminal.pending_stop_fold_depth.take().or_else(|| pending_close.as_ref().map(|c| c.turn_start_depth));
    // Shown after the edits that did apply: their toast would otherwise replace it.
    let fence_notice = app.terminal.live_patch.fence_notice.take();
    if !app.terminal.live_patch.has_held() {
        if let Some(depth) = pending_depth {
            crate::agent_stop::fold_stopped_turn(app, depth, pending_close.as_ref());
        }
        return;
    }
    let epoch_switched = std::mem::take(&mut app.terminal.live_patch.pending_epoch_switch);
    let mut dropped = 0;
    if let Some(doc) = app.terminal.live_patch.pending_doc.take() {
        dropped = std::mem::take(&mut app.terminal.live_patch.dropped_old_epoch);
        replace_live_document(app, doc);
        // The document left behind may have taken its history with it: nothing starts above the one on screen.
        pending_depth = pending_depth.map(|d| d.min(app.hist.undo_depth()));
        app.terminal.active_turn_ledger_mut().on_doc_epoch(std::time::Instant::now());
    } else if epoch_switched {
        app.terminal.active_turn_ledger_mut().on_doc_epoch(std::time::Instant::now());
    }
    let patches = std::mem::take(&mut app.terminal.live_patch.pending);
    let offsets = std::mem::take(&mut app.terminal.live_patch.pending_offsets);
    if !patches.is_empty() {
        app.terminal.status_msg = Some(format!("Applied {} live MCP edit(s) from {}", patches.len(), app.terminal.live_patch.path.display()));
        if let Some(depth) = pending_depth {
            // Patches that arrived before the stop closed belong to the stopped turn; later ones stay out of the fold.
            let pre_count = pending_close.as_ref().map_or(patches.len(), |c| c.pre_close_patch_count.min(patches.len()));
            let (pre_patches, post_patches) = patches.split_at(pre_count);
            let (pre_offsets, post_offsets) = offsets.split_at(pre_count.min(offsets.len()));
            if !pre_patches.is_empty() {
                apply_live_patches_with_offsets(app, pre_patches, pre_offsets);
            }
            crate::agent_stop::fold_stopped_turn(app, depth, pending_close.as_ref());
            if !post_patches.is_empty() {
                apply_live_patches_with_offsets(app, post_patches, post_offsets);
            }
        } else {
            apply_live_patches_with_offsets(app, &patches, &offsets);
        }
        maybe_autosave_ai_changes(app);
    } else if let Some(depth) = pending_depth {
        crate::agent_stop::fold_stopped_turn(app, depth, pending_close.as_ref());
    }
    if let Some(notice) = fence_notice.filter(|_| !patches.is_empty()) {
        app.show_toast(notice, true);
    }
    if dropped > 0 {
        note_dropped_edits(app, dropped);
    }
    if let Some(intent) = app.terminal.live_patch.pending_close.take() {
        apply_close_intent(app, intent);
    }
    publish_live_context(app);
}

pub fn abandon_queued_close(app: &mut App) {
    if let Some(intent) = app.terminal.live_patch.drop_close_on_hide() {
        crate::live_patch_log::write_close_result_file(
            &app.terminal.live_patch.path,
            &intent,
            serde_json::json!({
                "ok": false,
                "code": "EDITOR_NOT_NOTIFIED",
                "message": "The close was dropped because the tab was no longer shown.",
            }),
        );
    }
}

fn close_cancelled(app: &App, close_id: Option<u64>) -> bool {
    let Some(id) = close_id else {
        return false;
    };
    let path = app.terminal.session_dir.join(designos_doc::live_patch::CloseIntent::CANCEL_FILE);
    let Ok(raw) = crate::agent_session_files::read_session_file(&path) else {
        return false;
    };
    serde_json::from_slice::<serde_json::Value>(&raw).ok().and_then(|v| v.get("close_id").and_then(serde_json::Value::as_u64)) == Some(id)
}

/// Answers a close the editor will not do. The document stays open, so the edits the agent writes next must land: the
/// reader stops dropping them before the answer is written, because the agent only writes again after it reads it.
fn refuse_close(app: &mut App, intent: &designos_doc::live_patch::CloseIntent, body: serde_json::Value) {
    app.terminal.live_patch.close_refused();
    crate::live_patch_log::write_close_result_file(&app.terminal.live_patch.path, intent, body);
}

fn apply_close_intent(app: &mut App, intent: designos_doc::live_patch::CloseIntent) {
    if close_cancelled(app, intent.close_id) {
        app.terminal.live_patch.close_refused();
        return;
    }
    let active_id = app.tabs.entries.get(app.tabs.active_idx).map(|t| t.id);
    if let Some(named) = intent.tab_id {
        if app.tabs.entries.iter().all(|t| t.id != named) {
            // The tab is gone (a person closed it): asking again can never work, and the answer says so.
            refuse_close(
                app,
                &intent,
                serde_json::json!({
                    "ok": false,
                    "code": "EDITOR_NOT_NOTIFIED",
                    "reason": "tab_closed",
                    "message": "The tab is already closed, so there is nothing to close.",
                }),
            );
            return;
        }
        if active_id != Some(named) {
            refuse_close(
                app,
                &intent,
                serde_json::json!({
                    "ok": false,
                    "code": "EDITOR_NOT_NOTIFIED",
                    "message": "The close was dropped because that tab is not the one currently shown. Switch to it, then try again.",
                }),
            );
            return;
        }
    }
    let Some(id) = intent.tab_id.or(active_id) else {
        refuse_close(
            app,
            &intent,
            serde_json::json!({
                "ok": false,
                "code": "EDITOR_NOT_NOTIFIED",
                "reason": "tab_closed",
                "message": "No tab is open, so there is nothing to close.",
            }),
        );
        return;
    };
    app.commit_open_edits();
    let dirty = app.has_unsaved_edits();
    let log_path = app.terminal.live_patch.path.clone();
    match intent.unsaved.as_str() {
        "fail" if dirty => {
            refuse_close(
                app,
                &intent,
                serde_json::json!({
                    "ok": false,
                    "code": "UNSAVED_CHANGES",
                    "message": "Unsaved changes. The document stayed open.",
                }),
            );
        }
        "save" => {
            match save_for_agent_close(app, intent.path.as_deref()) {
                AgentCloseSave::Saved => {}
                AgentCloseSave::PathRequired => {
                    refuse_close(
                        app,
                        &intent,
                        serde_json::json!({
                            "ok": false,
                            "code": "PATH_REQUIRED",
                            "message": "Untitled documents need a path to save",
                        }),
                    );
                    return;
                }
                AgentCloseSave::Failed { code, message } => {
                    refuse_close(
                        app,
                        &intent,
                        serde_json::json!({
                            "ok": false,
                            "code": code,
                            "message": message,
                        }),
                    );
                    return;
                }
            }
            let revision = app.doc.mutation_epoch();
            let saved_path = app.file_path.as_ref().map(|p| p.display().to_string());
            crate::live_patch_log::write_close_result_file(
                &log_path,
                &intent,
                serde_json::json!({
                    "ok": true,
                    "closed": true,
                    "saved": true,
                    "unsaved": "save",
                    "doc_revision": revision,
                    "path": saved_path,
                }),
            );
            app.request_leave_from_agent(None, crate::home::Leave::CloseTab(id));
        }
        _ => {
            let revision = app.doc.mutation_epoch();
            crate::live_patch_log::write_close_result_file(
                &log_path,
                &intent,
                serde_json::json!({
                    "ok": true,
                    "closed": true,
                    "saved": false,
                    "unsaved": intent.unsaved,
                    "doc_revision": revision,
                }),
            );
            app.request_leave_from_agent(None, crate::home::Leave::CloseTab(id));
        }
    }
}

enum AgentCloseSave {
    Saved,
    PathRequired,
    Failed { code: &'static str, message: String },
}

fn save_for_agent_close(app: &mut App, path: Option<&str>) -> AgentCloseSave {
    if let Some(raw) = path.filter(|s| !s.is_empty()) {
        let target = std::path::PathBuf::from(raw);
        if app.file_path.as_ref() == Some(&target) {
            return save_open_document_for_agent_close(app);
        }
        return match app.persist_document_to(&target) {
            Ok(saved) => finish_close_save(app, Some(saved)),
            Err(e) => classify_save_io(&target, e),
        };
    }
    save_open_document_for_agent_close(app)
}

fn save_open_document_for_agent_close(app: &mut App) -> AgentCloseSave {
    if app.lib_file.is_some() {
        return match app.save_library_file_reporting() {
            Ok(()) => finish_close_save(app, app.file_path.clone()),
            Err(e) => classify_library_save(&e, app.lib_file.as_ref().map(|f| f.project.as_str())),
        };
    }
    let Some(p) = app.file_path.as_ref().filter(|p| crate::io::writes_native_format(p)).cloned() else {
        return AgentCloseSave::PathRequired;
    };
    match app.persist_document_to(&p) {
        Ok(saved) => finish_close_save(app, Some(saved)),
        Err(e) => classify_save_io(&p, e),
    }
}

fn finish_close_save(app: &App, path: Option<std::path::PathBuf>) -> AgentCloseSave {
    if app.has_unsaved_edits() {
        let display = path.as_ref().map(|p| p.display().to_string()).unwrap_or_else(|| "the document".into());
        AgentCloseSave::Failed { code: "INTERNAL_ERROR", message: format!("{display} was written but the document is still unsaved.") }
    } else {
        AgentCloseSave::Saved
    }
}

/// A write that failed because the file or its folder cannot be written to: not a disk fault.
fn is_read_only_kind(kind: std::io::ErrorKind) -> bool {
    matches!(kind, std::io::ErrorKind::PermissionDenied | std::io::ErrorKind::ReadOnlyFilesystem)
}

fn read_only(display: &str) -> AgentCloseSave {
    AgentCloseSave::Failed { code: "INVALID_ARGUMENTS", message: format!("{display} is read-only. Make it writable first, or choose a new path.") }
}

fn classify_save_io(path: &std::path::Path, err: std::io::Error) -> AgentCloseSave {
    let display_path = path.display().to_string();
    if is_read_only_kind(err.kind()) {
        read_only(&display_path)
    } else {
        AgentCloseSave::Failed {
            code: "INTERNAL_ERROR",
            message: format!("{display_path} can't be written: {err}. Check its permissions and free space, then try again."),
        }
    }
}

/// A library save failure maps like a titled file's: by the typed I/O kind, never by the text. A failure with no I/O kind (the
/// editor is showing an old version, the file is not in the library) is not a disk fault and keeps its own text.
fn classify_library_save(err: &crate::home::LibrarySaveError, project: Option<&str>) -> AgentCloseSave {
    let display = project.map_or_else(|| "the library file".to_string(), |p| format!("The library file in {p}"));
    match err.io_kind {
        Some(kind) if is_read_only_kind(kind) => read_only(&display),
        Some(_) => AgentCloseSave::Failed {
            code: "INTERNAL_ERROR",
            message: format!("{display} can't be written: {}. Check its permissions and free space, then try again.", err.message),
        },
        None => AgentCloseSave::Failed { code: "INVALID_ARGUMENTS", message: err.message.clone() },
    }
}

fn maybe_autosave_ai_changes(app: &mut App) {
    if app.lib_file.is_some() {
        return;
    }
    let Some(root) = app.lib.as_ref().map(|l| l.root().to_path_buf()) else { return };
    if !crate::comments::settings::load_autosave_ai_changes(&root) {
        return;
    }
    if app.file_path.is_none() {
        return;
    }
    if !app.has_unsaved_edits() {
        return;
    }
    app.save(false);
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
    // A running turn whose first patch lands before the bridge noticed it starts at the depth before these patches (`#684`).
    let pre_depth = app.hist.undo_depth();
    for tab in &mut app.terminal.tabs {
        if crate::agent_stop::is_tab_turn_running(tab) && !tab.was_working {
            tab.was_working = true;
            tab.turn_started_at_ms.get_or_insert_with(designos_doc::now_ms);
            tab.turn_start_undo_depth = pre_depth;
            tab.last_receipt = None;
            tab.still_running_notice = None;
        }
    }
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
                    app.agent_busy_frames.record_agent_patch_ops(&patch.ops, &app.terminal.live_patch.state);
                }
                committed_any |= committed;
                merge(&mut total, report);
            }
            Err(e) => failures.push(e),
        }
    }
    if committed_any {
        app.terminal.current_undo_depth = app.hist.undo_depth();
        if !total.inserted_roots.is_empty() {
            app.selection = total.inserted_roots.clone();
        }
        app.sanitize();
        app.changed();
    } else if total.applied > 0 {
        app.in_progress_changed();
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
    let saved_in_progress = app.doc.in_progress.clone();
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
            app.doc.in_progress = saved_in_progress;
            return Err(e.to_string());
        }
    };
    let post_replay_in_progress = app.doc.in_progress.clone();
    let committed = tx.commit_labeled(&mut app.doc, &mut app.hist, patch.label.clone()).map_err(|e| {
        // Nothing of this patch landed, so neither do the id remaps it made.
        app.terminal.live_patch.state = saved;
        app.doc.in_progress = saved_in_progress;
        e.to_string()
    })?;
    for (id, ip) in post_replay_in_progress {
        if app.doc.get(id).is_some() {
            app.doc.set_in_progress(id, Some(ip));
        }
    }
    if committed {
        app.hist.mark_last_undo_as_agent();
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
    total.tokens_applied += r.tokens_applied;
    total.tokens_duplicates += r.tokens_duplicates;
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

#[cfg(test)]
mod close_save_classification_tests {
    use super::*;
    use crate::home::LibrarySaveError;
    use std::io::ErrorKind;

    fn failed(save: AgentCloseSave) -> (&'static str, String) {
        match save {
            AgentCloseSave::Failed { code, message } => (code, message),
            AgentCloseSave::Saved => panic!("expected a failure, got Saved"),
            AgentCloseSave::PathRequired => panic!("a library save is never PATH_REQUIRED"),
        }
    }

    fn library(kind: Option<ErrorKind>, message: &str) -> (&'static str, String) {
        failed(classify_library_save(&LibrarySaveError { message: message.into(), io_kind: kind }, Some("Starter")))
    }

    #[test]
    fn library_failures_map_by_io_kind_not_by_text() {
        for kind in [ErrorKind::PermissionDenied, ErrorKind::ReadOnlyFilesystem] {
            let (code, message) = library(Some(kind), "Cannot save: io: whatever the OS said");
            assert_eq!(code, "INVALID_ARGUMENTS", "{kind:?}");
            assert!(message.contains("is read-only"), "{message}");
        }
        for kind in [ErrorKind::StorageFull, ErrorKind::NotFound, ErrorKind::Other] {
            let (code, message) = library(Some(kind), "Cannot save: io: Zugriff verweigert");
            assert_eq!(code, "INTERNAL_ERROR", "{kind:?}");
            assert!(message.contains("can't be written"), "{message}");
        }
        // Text that merely contains "permission denied" (a folder name) is not a read-only file.
        assert_eq!(library(Some(ErrorKind::StorageFull), "Cannot save: io: /permission denied/lib").0, "INTERNAL_ERROR");
    }

    #[test]
    fn a_failure_without_an_io_kind_keeps_its_own_text() {
        let (code, message) = library(None, "Go back to the current version before saving");
        assert_eq!(code, "INVALID_ARGUMENTS");
        assert_eq!(message, "Go back to the current version before saving");
    }

    #[test]
    fn titled_file_read_only_kinds_match_the_library_ones() {
        let path = std::path::Path::new("/x/a.designos");
        for kind in [ErrorKind::PermissionDenied, ErrorKind::ReadOnlyFilesystem] {
            assert_eq!(failed(classify_save_io(path, std::io::Error::from(kind))).0, "INVALID_ARGUMENTS", "{kind:?}");
        }
        assert_eq!(failed(classify_save_io(path, std::io::Error::from(ErrorKind::StorageFull))).0, "INTERNAL_ERROR");
    }
}
