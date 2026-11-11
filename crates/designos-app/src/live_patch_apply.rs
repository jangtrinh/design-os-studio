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
        app.tabs.retain_converted_images(app);
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
    let mut report = match replayed {
        Ok(r) => r,
        Err(e) => {
            tx.cancel(&mut app.doc);
            app.doc.in_progress = saved_in_progress;
            return Err(e.to_string());
        }
    };
    let post_replay_in_progress = app.doc.in_progress.clone();
    let mut solved_before_commit = false;
    let has_connectors = !touched_ids.is_empty() && designos_doc::with_index(&app.doc, |idx| !idx.is_empty());
    if has_connectors {
        let mut reroute_nodes: Vec<NodeId> = touched_ids.iter().map(|&id| NodeId(id)).collect();
        // Connectors follow nodes where layout leaves them, so layout is solved before they are planned.
        if !report.needs_layout.is_empty() {
            for id in relayout_collecting_written(&mut app.doc, &report.needs_layout) {
                if !reroute_nodes.contains(&id) {
                    reroute_nodes.push(id);
                }
            }
            solved_before_commit = true;
        }
        let plan = designos_doc::plan_reroute(&app.doc, &reroute_nodes, None);
        for (cid, g) in plan.updates {
            if let Some(n) = tx.ed.node(&mut app.doc, cid) {
                g.apply(n);
            }
            if !touched_ids.contains(&cid.0) {
                touched_ids.push(cid.0);
            }
        }
    }
    let committed = tx.commit_labeled(&mut app.doc, &mut app.hist, patch.label.clone()).map_err(|e| {
        // Nothing of this patch landed, so neither do the id remaps it made.
        app.terminal.live_patch.state = saved;
        app.doc.in_progress = saved_in_progress;
        if solved_before_commit {
            // The solve before the commit moved nodes the failed step cannot put back.
            relayout_touched(&mut app.doc, &report.needs_layout);
        }
        e.to_string()
    })?;
    for (id, ip) in post_replay_in_progress {
        if app.doc.get(id).is_some() {
            app.doc.set_in_progress(id, Some(ip));
        }
    }
    if committed {
        app.hist.mark_last_undo_as_agent();
        if has_connectors {
            // The commit put the patch's own nodes back as the patch wrote them, so layout is solved
            // again. A connector that is a flow child of an auto-layout frame is measured and placed
            // by that solve, so routing gets the last word.
            let written = relayout_collecting_written(&mut app.doc, &report.needs_layout);
            if let Err(e) = reroute_into_last_step(app, &written, &mut touched_ids) {
                report.skipped.push(format!("connectors could not be re-routed after layout: {e}"));
            }
        } else {
            relayout_touched(&mut app.doc, &report.needs_layout);
        }
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

/// Plans connectors once more against the nodes the last solve wrote and folds the updates into the
/// step the patch just committed, so one undo still takes back the patch, the layout and every
/// connector. Once, not in a loop: a further solve would put a connector that is a flow child back in
/// its flow slot, so a plan would never come back empty. Nothing is written when no connector changed.
fn reroute_into_last_step(app: &mut App, written: &[NodeId], touched_ids: &mut Vec<u64>) -> Result<(), String> {
    if written.is_empty() {
        return Ok(());
    }
    let plan = designos_doc::plan_reroute(&app.doc, written, None);
    let mut updates = Vec::with_capacity(plan.updates.len());
    for (cid, geometry) in plan.updates {
        if let Some(current) = app.doc.get(cid) {
            let mut updated = current.clone();
            geometry.apply(&mut updated);
            updates.push(designos_doc::Command::Update { node: Box::new(updated) });
            if !touched_ids.contains(&cid.0) {
                touched_ids.push(cid.0);
            }
        }
    }
    let Some(inverse) = app.hist.peek_undo().cloned().filter(|_| !updates.is_empty()) else {
        return Ok(());
    };
    let label = app.hist.peek_undo_label().map(str::to_owned);
    let depth = app.hist.undo_depth();
    let forward = designos_doc::table_ops::apply_checked(&mut app.doc, inverse).map_err(|e| e.to_string())?;
    app.hist.drop_undo_steps_after(depth - 1);
    let mut commands = match forward {
        designos_doc::Command::Batch(commands) => commands,
        other => vec![other],
    };
    commands.extend(updates);
    app.hist.exec_labeled(&mut app.doc, designos_doc::Command::Batch(commands), label).map_err(|e| e.to_string())?;
    app.hist.mark_last_undo_as_agent();
    Ok(())
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
///
/// Returns the auto-layout containers it solved.
pub(crate) fn relayout_touched(doc: &mut Document, needs_layout: &[NodeId]) -> Vec<NodeId> {
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
    for &r in &roots {
        doc.relayout_subtree(r);
    }
    roots
}

/// [`relayout_touched`], returning the ids the solve wrote. When the document's change log cannot
/// tell (it overflowed, or the tree changed shape), that is every node under the solved containers.
fn relayout_collecting_written(doc: &mut Document, needs_layout: &[NodeId]) -> Vec<NodeId> {
    let epoch = doc.mutation_epoch();
    let roots = relayout_touched(doc, needs_layout);
    if doc.mutation_epoch() == epoch {
        return Vec::new();
    }
    let (mut written, mut removed) = (Vec::new(), Vec::new());
    if doc.dirty_changes_since(epoch, &mut written, &mut removed) {
        return written;
    }
    let mut scope = Vec::new();
    for root in roots {
        doc.walk(root, |n, _| scope.push(n.id));
    }
    scope
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

    #[test]
    fn live_patch_move_reroutes_attached_connectors() {
        use designos_doc::live_patch::live_ops_for_command;
        use designos_doc::*;

        let mut doc = Document::new("live_patch_reroute");
        let page = doc.add_page("Page 1");
        let a = doc.alloc_id();
        doc.insert(page, Node::frame(a, Rect::new(0.0, 0.0, 120.0, 80.0))).unwrap();
        let b = doc.alloc_id();
        doc.insert(page, Node::frame(b, Rect::new(300.0, 0.0, 120.0, 80.0))).unwrap();
        let c = doc.alloc_id();
        let props = ConnectorProps { start: ConnectorEnd::attached(a, Magnet::Auto), end: ConnectorEnd::attached(b, Magnet::Auto), ..Default::default() };
        doc.insert(page, Node::connector(c, props)).unwrap();
        sync_connector(&mut doc, c);
        let before_c = doc.get(c).unwrap().transform;

        let mut moved_b = doc.get(b).unwrap().clone();
        moved_b.transform.tx += 180.0;
        moved_b.transform.ty -= 120.0;
        let ops = live_ops_for_command(&doc, &Command::Update { node: Box::new(moved_b) }).unwrap();
        let mut patch = LivePatch::new(ops);
        patch.label = Some("move b".into());

        let mut app = App::new(doc);
        apply_live_patches(&mut app, &[patch]);

        let expected = connector_geometry(&app.doc, c).unwrap();
        assert_ne!(app.doc.get(c).unwrap().transform, before_c);
        assert_eq!(app.doc.get(c).unwrap().transform, expected.transform);
        assert_eq!(app.doc.get(c).unwrap().size, expected.size);
        assert_eq!(app.hist.undo_depth(), 1);
    }

    #[test]
    fn a_live_patch_with_a_refused_table_op_applies_nothing() {
        use designos_doc::live_patch::live_ops_for_command;
        use designos_doc::*;

        let mut doc = Document::new("live_patch_table");
        let page = doc.add_page("Page 1");
        let host = doc.alloc_id();
        doc.insert(page, Node::frame(host, Rect::new(100.0, 0.0, 50.0, 50.0))).unwrap();
        let rect = doc.alloc_id();

        // a rectangle inserted into `host`; the patch is recorded while `host` is a frame
        let recorded = Command::Insert { parent: host, index: 0, nodes: vec![Node::rectangle(rect, Rect::new(0.0, 0.0, 5.0, 5.0))] };
        let patch = LivePatch::new(live_ops_for_command(&doc, &recorded).unwrap());
        // by the time the patch arrives, `host` is a table, so the insert is refused
        doc.get_mut(host).unwrap().kind = NodeKind::Table(TableProps::grid(1, 1));
        let before = doc.to_json().unwrap();

        let mut app = App::new(doc);
        apply_live_patches(&mut app, &[patch]);

        assert!(app.doc.get(rect).is_none(), "the refused insert is absent");
        assert_eq!(app.hist.undo_depth(), 0);
        let (a, b) = (app.doc.to_json().unwrap(), before);
        let first_diff = a.bytes().zip(b.bytes()).position(|(x, y)| x != y).unwrap_or(a.len().min(b.len()));
        assert_eq!(a, b, "nothing of the patch is applied; first difference near {}", &a[first_diff.saturating_sub(60)..(first_diff + 60).min(a.len())]);
    }

    #[test]
    fn a_live_patch_that_leaves_a_table_without_a_cell_applies_nothing() {
        use designos_doc::live_patch::live_ops_for_command;
        use designos_doc::*;

        let mut doc = Document::new("live_patch_table_rollback");
        let page = doc.add_page("Page 1");
        let cmd = table_ops::create_table(&doc, page, Vec2::new(0.0, 0.0), 2, 2, &TableStyle::default()).unwrap();
        cmd.apply(&mut doc).unwrap();
        let table = *doc.get(page).unwrap().children.last().unwrap();
        let cell = doc.get(table).unwrap().children[0];
        // every op of the patch is valid alone; together they leave a live position without a cell
        let patch = LivePatch::new(live_ops_for_command(&doc, &Command::Remove { id: cell }).unwrap());
        let before = doc.to_json().unwrap();

        let mut app = App::new(doc);
        apply_live_patches(&mut app, &[patch]);

        assert!(app.doc.get(cell).is_some(), "the cell is still there");
        assert_eq!(app.hist.undo_depth(), 0, "no step was recorded");
        assert_eq!(app.doc.to_json().unwrap(), before, "the table is as it was");
    }

    #[test]
    fn live_patch_already_rerouted_connector_is_noop() {
        use designos_doc::live_patch::live_ops_for_command;
        use designos_doc::*;

        let mut doc = Document::new("live_patch_noop");
        let page = doc.add_page("Page 1");
        let a = doc.alloc_id();
        doc.insert(page, Node::frame(a, Rect::new(0.0, 0.0, 120.0, 80.0))).unwrap();
        let b = doc.alloc_id();
        doc.insert(page, Node::frame(b, Rect::new(300.0, 0.0, 120.0, 80.0))).unwrap();
        let c = doc.alloc_id();
        let props = ConnectorProps { start: ConnectorEnd::attached(a, Magnet::Auto), end: ConnectorEnd::attached(b, Magnet::Auto), ..Default::default() };
        doc.insert(page, Node::connector(c, props)).unwrap();
        sync_connector(&mut doc, c);

        let mut prev_doc = doc.clone();
        prev_doc.get_mut(c).unwrap().transform.tx -= 50.0;
        let current_c = doc.get(c).unwrap().clone();
        let ops = live_ops_for_command(&prev_doc, &Command::Update { node: Box::new(current_c) }).unwrap();
        let mut patch = LivePatch::new(ops);
        patch.label = Some("rerouted c".into());

        let mut app = App::new(doc);
        app.canvas.dirty = false;
        let epoch_before = app.doc.mutation_epoch();
        let rev_before = app.canvas.tiles.rev();
        let depth_before = app.hist.undo_depth();

        apply_live_patches(&mut app, &[patch]);

        assert!(!app.canvas.dirty);
        assert_eq!(app.doc.mutation_epoch(), epoch_before);
        assert_eq!(app.hist.undo_depth(), depth_before);
        assert_eq!(app.canvas.tiles.rev(), rev_before);
    }
}

#[cfg(test)]
mod reroute_after_layout_tests {
    use super::*;
    use designos_doc::live_patch::live_ops_for_command;
    use designos_doc::*;

    /// A vertical auto-layout frame (gap 0) holding `a` (100x40 at y 0) and `b` (100x40 at y 40),
    /// plus a page-level connector from a frame at the right to `b`.
    fn vertical_stack_with_connector_on_second_child() -> (Document, NodeId, NodeId, NodeId) {
        let mut doc = Document::new("reroute_after_layout");
        let page = doc.add_page("Page 1");
        let frame = doc.alloc_id();
        let mut stack = Node::frame(frame, Rect::new(0.0, 0.0, 100.0, 300.0));
        if let NodeKind::Frame(fp) = &mut stack.kind {
            fp.auto_layout = Some(AutoLayout { direction: LayoutDirection::Vertical, item_spacing: 0.0, ..AutoLayout::v1() });
        }
        doc.insert(page, stack).unwrap();
        let a = doc.alloc_id();
        doc.insert(frame, Node::rectangle(a, Rect::new(0.0, 0.0, 100.0, 40.0))).unwrap();
        let b = doc.alloc_id();
        doc.insert(frame, Node::rectangle(b, Rect::new(0.0, 40.0, 100.0, 40.0))).unwrap();
        let far = doc.alloc_id();
        doc.insert(page, Node::frame(far, Rect::new(300.0, 0.0, 100.0, 40.0))).unwrap();
        let c = doc.alloc_id();
        let props = ConnectorProps { start: ConnectorEnd::attached(far, Magnet::Auto), end: ConnectorEnd::attached(b, Magnet::Auto), ..Default::default() };
        doc.insert(page, Node::connector(c, props)).unwrap();
        doc.relayout_subtree(frame);
        sync_connector(&mut doc, c);
        assert_eq!(doc.get(b).unwrap().transform.ty, 40.0, "precondition: b sits under a");
        (doc, a, b, c)
    }

    fn assert_connector_follows_its_ends(doc: &Document, c: NodeId) {
        let expected = connector_geometry(doc, c).unwrap();
        let stored = doc.get(c).unwrap();
        assert_eq!(stored.transform, expected.transform, "connector transform");
        assert_eq!(stored.size, expected.size, "connector size");
        let NodeKind::Connector(props) = &stored.kind else { unreachable!() };
        assert_eq!(props.start.point, expected.start_point, "connector start");
        assert_eq!(props.end.point, expected.end_point, "connector end");
    }

    fn grow_a_patch(doc: &Document, a: NodeId, extra: Vec<Command>) -> LivePatch {
        let mut grown = doc.get(a).unwrap().clone();
        grown.size.y = 80.0;
        let mut cmds = vec![Command::Update { node: Box::new(grown) }];
        cmds.extend(extra);
        let cmd = if cmds.len() == 1 { cmds.pop().unwrap() } else { Command::Batch(cmds) };
        let mut patch = LivePatch::new(live_ops_for_command(doc, &cmd).unwrap());
        patch.label = Some("grow a".into());
        patch
    }

    #[test]
    fn live_patch_resizing_a_stack_child_reroutes_the_connector_on_the_sibling_the_solver_moves() {
        let (doc, a, b, c) = vertical_stack_with_connector_on_second_child();
        let before_c = doc.get(c).unwrap().clone();
        let patch = grow_a_patch(&doc, a, Vec::new());

        let mut app = App::new(doc);
        apply_live_patches(&mut app, &[patch]);

        assert_eq!(app.doc.get(b).unwrap().transform.ty, 80.0, "the solver moves b below the taller a");
        assert_connector_follows_its_ends(&app.doc, c);
        let NodeKind::Connector(props) = &app.doc.get(c).unwrap().kind else { unreachable!() };
        assert!((80.0..=120.0).contains(&props.end.point.y), "the connector ends on b at its new place, not on its old edge: {:?}", props.end.point);
        assert_eq!(app.hist.undo_depth(), 1, "patch, layout and connector share one step");

        app.hist.undo(&mut app.doc).unwrap();
        assert_eq!(app.doc.get(a).unwrap().size.y, 40.0);
        assert_eq!(app.doc.get(b).unwrap().transform.ty, 40.0);
        assert_eq!(app.doc.get(c).unwrap().transform, before_c.transform);
        assert_eq!(app.doc.get(c).unwrap().kind, before_c.kind);
        assert_connector_follows_its_ends(&app.doc, c);
    }

    #[test]
    fn live_patch_with_post_layout_connector_geometry_writes_nothing_extra() {
        let (doc, a, b, c) = vertical_stack_with_connector_on_second_child();

        // What the agent's side saved: a taller, the solver's move of b, and the connector re-routed against it.
        let mut after = doc.clone();
        after.get_mut(a).unwrap().size.y = 80.0;
        let frame = after.get(a).unwrap().parent.unwrap();
        after.relayout_subtree(frame);
        sync_connector(&mut after, c);
        let routed = after.get(c).unwrap().clone();
        assert_eq!(after.get(b).unwrap().transform.ty, 80.0);

        let patch = grow_a_patch(&doc, a, vec![Command::Update { node: Box::new(routed.clone()) }]);
        let mut app = App::new(doc);
        apply_live_patches(&mut app, std::slice::from_ref(&patch));

        assert_eq!(app.doc.get(b).unwrap().transform.ty, 80.0);
        assert_eq!(app.doc.get(c).unwrap().transform, routed.transform, "the connector keeps the geometry the patch carried");
        assert_eq!(app.doc.get(c).unwrap().size, routed.size);
        assert_eq!(app.doc.get(c).unwrap().kind, routed.kind);
        assert_connector_follows_its_ends(&app.doc, c);
        assert_eq!(app.hist.undo_depth(), 1, "no extra undo step");

        // Delivering the same patch again changes nothing: no write, no step, no repaint.
        app.canvas.dirty = false;
        let epoch = app.doc.mutation_epoch();
        let rev = app.canvas.tiles.rev();
        apply_live_patches(&mut app, std::slice::from_ref(&patch));
        assert!(!app.canvas.dirty);
        assert_eq!(app.doc.mutation_epoch(), epoch);
        assert_eq!(app.canvas.tiles.rev(), rev);
        assert_eq!(app.hist.undo_depth(), 1);
    }

    #[test]
    fn live_patch_resizing_a_child_of_a_long_stack_reroutes_the_connector_on_the_last_child() {
        // More children than the document's change log remembers, so the solve's writes cannot be listed.
        let (mut doc, a, _b, c) = vertical_stack_with_connector_on_second_child();
        let stack = doc.get(a).unwrap().parent.unwrap();
        let mut last = a;
        for _ in 0..600 {
            last = doc.alloc_id();
            doc.insert(stack, Node::rectangle(last, Rect::new(0.0, 0.0, 100.0, 40.0))).unwrap();
        }
        doc.get_mut(stack).unwrap().size.y = 40.0 * 603.0;
        doc.relayout_subtree(stack);
        doc.flush_pending_relayout();
        if let NodeKind::Connector(props) = &mut doc.get_mut(c).unwrap().kind {
            props.end.node = Some(last);
        }
        sync_connector(&mut doc, c);
        let last_ty = doc.get(last).unwrap().transform.ty;
        assert_eq!(last_ty, 40.0 * 601.0, "precondition: the stack is laid out");
        let patch = grow_a_patch(&doc, a, Vec::new());

        let mut app = App::new(doc);
        apply_live_patches(&mut app, &[patch]);

        assert_eq!(app.doc.get(last).unwrap().transform.ty, last_ty + 40.0);
        assert_connector_follows_its_ends(&app.doc, c);
        assert_eq!(app.hist.undo_depth(), 1);
    }

    #[test]
    fn live_patch_removing_a_large_stack_child_reroutes_the_connector_on_the_sibling_that_shifts() {
        // Removing more nodes than the change log tracks one by one: it cannot name what the solve wrote.
        let (mut doc, a, b, c) = vertical_stack_with_connector_on_second_child();
        let stack = doc.get(a).unwrap().parent.unwrap();
        let page = doc.get(stack).unwrap().parent.unwrap();
        let big = doc.alloc_id();
        doc.insert(page, Node::frame(big, Rect::new(0.0, 0.0, 100.0, 40.0))).unwrap();
        for _ in 0..70 {
            let leaf = doc.alloc_id();
            doc.insert(big, Node::rectangle(leaf, Rect::new(0.0, 0.0, 10.0, 10.0))).unwrap();
        }
        doc.reparent(big, stack, 0).unwrap();
        doc.relayout_subtree(stack);
        sync_connector(&mut doc, c);
        assert_eq!(doc.get(b).unwrap().transform.ty, 80.0, "precondition: b sits under big and a");
        let mut patch = LivePatch::new(live_ops_for_command(&doc, &Command::Remove { id: big }).unwrap());
        patch.label = Some("remove big".into());

        let mut app = App::new(doc);
        apply_live_patches(&mut app, &[patch]);

        assert!(app.doc.get(big).is_none());
        assert_eq!(app.doc.get(b).unwrap().transform.ty, 40.0, "b moves up into the gap");
        assert_connector_follows_its_ends(&app.doc, c);
        assert_eq!(app.hist.undo_depth(), 1);
    }

    #[test]
    fn live_patch_removing_a_stack_child_reroutes_the_connector_on_the_sibling_that_shifts() {
        let (doc, a, b, c) = vertical_stack_with_connector_on_second_child();
        let mut patch = LivePatch::new(live_ops_for_command(&doc, &Command::Remove { id: a }).unwrap());
        patch.label = Some("remove a".into());

        let mut app = App::new(doc);
        apply_live_patches(&mut app, &[patch]);

        assert!(app.doc.get(a).is_none());
        assert_eq!(app.doc.get(b).unwrap().transform.ty, 0.0, "b moves up into the gap");
        assert_connector_follows_its_ends(&app.doc, c);
        assert_eq!(app.hist.undo_depth(), 1);
    }

    /// A vertical stack of `a`, a connector that is itself part of the flow (start on `a`, free end),
    /// and `b`; a page-level connector runs from a frame below to `b`. The solver measures the flow
    /// connector, and routing resizes it when `a` changes, so the two feed each other.
    fn stack_with_a_connector_in_its_flow() -> (Document, NodeId, NodeId, NodeId, NodeId) {
        let mut doc = Document::new("reroute_flow_connector");
        let page = doc.add_page("Page 1");
        let stack = doc.alloc_id();
        let mut stack_node = Node::frame(stack, Rect::new(0.0, 0.0, 100.0, 300.0));
        if let NodeKind::Frame(fp) = &mut stack_node.kind {
            fp.auto_layout = Some(AutoLayout { direction: LayoutDirection::Vertical, item_spacing: 0.0, ..AutoLayout::v1() });
        }
        doc.insert(page, stack_node).unwrap();
        let a = doc.alloc_id();
        doc.insert(stack, Node::rectangle(a, Rect::new(0.0, 0.0, 100.0, 40.0))).unwrap();
        let flow_connector = doc.alloc_id();
        let flow_props =
            ConnectorProps { start: ConnectorEnd::attached(a, Magnet::Auto), end: ConnectorEnd::free(Vec2::new(250.0, 20.0)), ..Default::default() };
        doc.insert(stack, Node::connector(flow_connector, flow_props)).unwrap();
        let b = doc.alloc_id();
        doc.insert(stack, Node::rectangle(b, Rect::new(0.0, 0.0, 100.0, 40.0))).unwrap();
        let far = doc.alloc_id();
        doc.insert(page, Node::frame(far, Rect::new(300.0, 400.0, 100.0, 40.0))).unwrap();
        let page_connector = doc.alloc_id();
        let page_props =
            ConnectorProps { start: ConnectorEnd::attached(far, Magnet::Auto), end: ConnectorEnd::attached(b, Magnet::Auto), ..Default::default() };
        doc.insert(page, Node::connector(page_connector, page_props)).unwrap();
        // The flow connector is sized before layout measures it, and layout then leaves it in its slot.
        sync_connector(&mut doc, flow_connector);
        doc.relayout_subtree(stack);
        sync_connector(&mut doc, page_connector);
        let b_ty = doc.get(b).unwrap().transform.ty;
        doc.relayout_subtree(stack);
        assert_eq!(doc.get(b).unwrap().transform.ty, b_ty, "precondition: the stack is laid out");
        assert_eq!(b_ty, 40.0 + doc.get(flow_connector).unwrap().size.y, "precondition: b sits under the flow connector");
        let mut grown = doc.clone();
        grown.get_mut(a).unwrap().size.y = 80.0;
        sync_connector(&mut grown, flow_connector);
        assert_ne!(grown.get(flow_connector).unwrap().size, doc.get(flow_connector).unwrap().size, "precondition: a taller a resizes the flow connector");
        (doc, a, b, flow_connector, page_connector)
    }

    #[test]
    fn live_patch_resizing_a_stack_child_leaves_a_flow_connector_and_a_page_connector_on_their_routes() {
        let (doc, a, b, flow_connector, page_connector) = stack_with_a_connector_in_its_flow();
        let before: Vec<Node> = [a, b, flow_connector, page_connector].iter().map(|id| doc.get(*id).unwrap().clone()).collect();
        let patch = grow_a_patch(&doc, a, Vec::new());

        let mut app = App::new(doc);
        apply_live_patches(&mut app, &[patch]);

        assert_connector_follows_its_ends(&app.doc, flow_connector);
        assert_connector_follows_its_ends(&app.doc, page_connector);
        assert_eq!(app.doc.get(b).unwrap().transform.ty, 80.0 + app.doc.get(flow_connector).unwrap().size.y, "b sits under the resized flow connector");
        assert_eq!(app.hist.undo_depth(), 1);

        app.hist.undo(&mut app.doc).unwrap();
        for (id, was) in [a, b, flow_connector, page_connector].iter().zip(&before) {
            assert_eq!(app.doc.get(*id).unwrap(), was, "node {} after undo", id.0);
        }
    }
}
