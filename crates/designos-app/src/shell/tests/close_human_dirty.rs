use super::leave_prompt::{make_two_tabs, tab_id};
use super::live_agent_edits::finish;
use super::unsaved_dot::{dot, edit, external_sim_with, library_sim, nodes_on_disk, two_rects, wait, TmpDir, SETTLED};
use super::*;
use serde_json::json;
use std::time::Duration;

fn live_outside_autosave_off() -> (Sim, designos_mcp::server::Session, std::path::PathBuf, std::path::PathBuf, super::unsaved_dot::TmpDir) {
    let (s, dir, path) = external_sim_with(&two_rects());
    let root = s.app().lib.as_ref().expect("library").root().to_path_buf();
    crate::comments::settings::save_autosave_ai_changes(&root, false).unwrap();
    // Under the test TmpDir, so a panic cannot leak it (#1366).
    let workspace = dir.0.join("mcp-workspace");
    std::fs::create_dir_all(&workspace).unwrap();
    let mut session = designos_mcp::server::Session::with_session_dir(&workspace, &s.app().terminal.session_dir).expect("mcp session");
    session.set_close_ack_timeout(Duration::from_millis(800));
    (s, session, path, workspace, dir)
}

/// A sim on a library file (it has no `file_path`), autosave off so the agent's close is what writes.
fn live_library_autosave_off() -> (Sim, designos_mcp::server::Session, std::path::PathBuf, TmpDir) {
    let (s, dir) = library_sim();
    assert!(s.app().lib_file.is_some() && s.app().file_path.is_none(), "a library file has no path");
    let root = s.app().lib.as_ref().expect("library").root().to_path_buf();
    crate::comments::settings::save_autosave_ai_changes(&root, false).unwrap();
    let workspace = dir.0.join("mcp-workspace");
    std::fs::create_dir_all(&workspace).unwrap();
    let mut session = designos_mcp::server::Session::with_session_dir(&workspace, &s.app().terminal.session_dir).expect("mcp session");
    session.set_close_ack_timeout(Duration::from_millis(800));
    (s, session, workspace, dir)
}

fn library_nodes_on_disk(s: &Sim, id: &str) -> usize {
    s.app().lib.as_ref().expect("library").load_file(id).map(|d| d.nodes.len()).expect("library file loads")
}

#[test]
fn library_file_close_save_writes_the_edit_and_closes_not_path_required() {
    let (mut s, mut session, workspace, _dir) = live_library_autosave_off();
    let id = s.app().lib_file.as_ref().expect("library file").id.clone();
    let before = library_nodes_on_disk(&s, &id);
    edit(&mut s);
    let edited = s.app().doc.nodes.len();
    assert!(edited > before, "the edit adds a layer");
    assert!(s.app().has_unsaved_edits(), "human edit lights the tab");
    assert_eq!(library_nodes_on_disk(&s, &id), before, "autosave is off: the edit is not on disk yet");
    let out = close_while_pumping(&mut s, &mut session, json!({"unsaved": "save"})).expect("library save close");
    assert_eq!(out["closed"], true, "{out}");
    assert_eq!(out["saved"], true, "{out}");
    assert_eq!(s.app().tabs.entries.len(), 0, "the tab closed");
    assert_eq!(library_nodes_on_disk(&s, &id), edited, "the edit is in the library file");
    finish(s, workspace);
}

#[cfg(unix)]
#[test]
fn library_file_read_only_close_save_is_invalid_arguments_not_path_required() {
    use std::os::unix::fs::PermissionsExt;
    let (mut s, mut session, workspace, _dir) = live_library_autosave_off();
    edit(&mut s);
    let (lib, lf) = (s.app().lib.clone().unwrap(), s.app().lib_file.clone().unwrap());
    let folder = crate::home::library_file_disk_path(&lib, &lf.project_id, &lf.id).parent().unwrap().to_path_buf();
    std::fs::set_permissions(&folder, std::fs::Permissions::from_mode(0o555)).unwrap();
    if std::fs::File::create(folder.join("probe")).is_ok() {
        restore_mode(&folder, 0o755);
        finish(s, workspace);
        return;
    }
    let tabs = s.app().tabs.entries.len();
    let err = close_while_pumping(&mut s, &mut session, json!({"unsaved": "save"})).expect_err("read-only library save");
    restore_mode(&folder, 0o755);
    assert_ne!(err["code"], "PATH_REQUIRED", "a library file is not untitled: {err}");
    assert_eq!(err["code"], "INVALID_ARGUMENTS", "{err}");
    assert!(err["message"].as_str().unwrap_or("").contains("is read-only"), "{err}");
    let next = err["next_step"].as_str().unwrap_or("");
    assert!(next.contains("stayed open") && !next.contains("valid_options") && !next.contains("corrected_example") && !next.contains("`path`"), "{err}");
    assert_eq!(s.app().tabs.entries.len(), tabs, "tab stayed open");
    assert!(s.app().has_unsaved_edits(), "still dirty");
    finish(s, workspace);
}

fn close_while_pumping(s: &mut Sim, session: &mut designos_mcp::server::Session, args: serde_json::Value) -> Result<serde_json::Value, serde_json::Value> {
    std::thread::scope(|scope| {
        let h = scope.spawn(|| session.call("designos_close_document", args));
        // Until the call has answered: the call may not have written its close line when a fixed run of frames ends.
        let end = std::time::Instant::now() + Duration::from_secs(5);
        while !h.is_finished() && std::time::Instant::now() < end {
            wait(s, 0.1);
            std::thread::sleep(Duration::from_millis(5));
        }
        wait(s, SETTLED);
        h.join().expect("close thread").map_err(|e| e.to_json_value())
    })
}

#[test]
fn human_edit_then_agent_fail_keeps_the_tab_and_the_file() {
    let (mut s, mut session, path, workspace, _dir) = live_outside_autosave_off();
    let on_disk = nodes_on_disk(&path);
    edit(&mut s);
    assert!(s.app().has_unsaved_edits(), "human edit lights the tab");
    assert!(dot(&s));
    let tabs = s.app().tabs.entries.len();
    let err = close_while_pumping(&mut s, &mut session, json!({})).expect_err("fail on human-dirty");
    assert_eq!(err["code"], "UNSAVED_CHANGES", "{err}");
    assert_eq!(err["safe_to_retry"], false, "{err}");
    assert_eq!(s.app().tabs.entries.len(), tabs, "tab stayed open");
    assert!(s.app().has_unsaved_edits(), "human edit still unsaved");
    assert_eq!(nodes_on_disk(&path), on_disk, "file on disk unchanged");
    finish(s, workspace);
}

/// An agent edit written after a refused close must reach the canvas (AC-02): the refused close left the reader dropping every later patch.
fn apply_while_pumping(s: &mut Sim, session: &mut designos_mcp::server::Session, spec_name: &str) -> serde_json::Value {
    let page = *s.app().doc.pages.first().expect("a page");
    let out = session
        .call("designos_apply_spec", json!({"parent_id": page.0, "spec": {"name": spec_name, "kind": "frame", "width": 80.0, "height": 40.0}}))
        .expect("apply_spec after the refused close");
    wait(s, SETTLED);
    out
}

fn named(s: &Sim, name: &str) -> bool {
    s.app().doc.nodes.values().any(|n| n.name == name)
}

#[test]
fn apply_spec_after_a_refused_close_reaches_the_canvas() {
    let (mut s, mut session, _path, workspace, _dir) = live_outside_autosave_off();
    edit(&mut s);
    let err = close_while_pumping(&mut s, &mut session, json!({})).expect_err("fail on human-dirty");
    assert_eq!(err["code"], "UNSAVED_CHANGES", "{err}");
    let out = apply_while_pumping(&mut s, &mut session, "After Refused Close");
    assert_eq!(out["changed"], true, "{out}");
    assert!(named(&s, "After Refused Close"), "apply_spec said changed:true but the edit never reached the canvas");
    finish(s, workspace);
}

#[test]
fn human_edit_then_agent_save_writes_the_edit_and_closes() {
    let (mut s, mut session, path, workspace, _dir) = live_outside_autosave_off();
    let before = nodes_on_disk(&path);
    edit(&mut s);
    let edited = s.app().doc.nodes.len();
    assert!(edited > before);
    let out = close_while_pumping(&mut s, &mut session, json!({"unsaved": "save"})).expect("save close");
    assert_eq!(out["closed"], true, "{out}");
    assert_eq!(out["saved"], true, "{out}");
    assert!(out["doc_revision"].as_u64().is_some(), "{out}");
    assert_eq!(s.app().tabs.entries.len(), 0, "the tab closed");
    assert_eq!(nodes_on_disk(&path), edited, "human edit is on disk");
    finish(s, workspace);
}

#[test]
fn human_edit_then_agent_discard_closes_and_leaves_the_file() {
    let (mut s, mut session, path, workspace, _dir) = live_outside_autosave_off();
    let on_disk = nodes_on_disk(&path);
    edit(&mut s);
    let out = close_while_pumping(&mut s, &mut session, json!({"unsaved": "discard"})).expect("discard close");
    assert_eq!(out["closed"], true, "{out}");
    assert_eq!(out["saved"], false, "{out}");
    assert_eq!(s.app().tabs.entries.len(), 0, "the tab closed");
    assert_eq!(nodes_on_disk(&path), on_disk, "file on disk unchanged");
    finish(s, workspace);
}

/// What a former MCP process left in the session folder: its first close, answered with success. A new process must not take it
/// for its own answer (seen live: a read-only `close(save)` and a `close(fail)` on a dirty tab both returned `closed:true`).
fn leave_stale_success(session_dir: &std::path::Path) {
    let stale = json!({"close_id": 1u64, "ok": true, "closed": true, "saved": true, "unsaved": "save", "doc_revision": 3u64, "tab_id": 1u64});
    std::fs::write(session_dir.join("close-result.json"), stale.to_string()).unwrap();
    std::fs::write(session_dir.join("close-cancel.json"), json!({"close_id": 1u64}).to_string()).unwrap();
}

#[test]
fn a_stale_success_answer_does_not_close_or_save_for_a_new_process() {
    let (mut s, mut session, path, workspace, _dir) = live_outside_autosave_off();
    let on_disk = nodes_on_disk(&path);
    edit(&mut s);
    leave_stale_success(&s.app().terminal.session_dir.clone());
    let tabs = s.app().tabs.entries.len();
    let err = close_while_pumping(&mut s, &mut session, json!({})).expect_err("fail on a dirty tab is not closed:true");
    assert_eq!(err["code"], "UNSAVED_CHANGES", "{err}");
    assert_eq!(s.app().tabs.entries.len(), tabs, "tab stayed open");
    assert!(s.app().has_unsaved_edits(), "human edit still unsaved");
    assert_eq!(nodes_on_disk(&path), on_disk, "file on disk unchanged");
    finish(s, workspace);
}

#[test]
fn closing_an_already_closed_tab_is_not_retryable() {
    let (mut s, mut session, _path, workspace, _dir) = live_outside_autosave_off();
    let first = close_while_pumping(&mut s, &mut session, json!({"unsaved": "discard"})).expect("first close");
    assert_eq!(first["closed"], true, "{first}");
    assert_eq!(s.app().tabs.entries.len(), 0, "the tab closed");
    let err = close_while_pumping(&mut s, &mut session, json!({"unsaved": "discard"})).expect_err("nothing left to close");
    assert_ne!(err["safe_to_retry"], true, "asking again can never work: {err}");
    assert_ne!(err["code"], "PATH_REQUIRED", "{err}");
    assert!(err["next_step"].as_str().unwrap_or("").contains("already closed"), "{err}");
    finish(s, workspace);
}

#[test]
fn stale_queued_close_does_not_close_a_tab_the_user_switched_back_to() {
    let (mut s, mut session, path, workspace, _dir) = live_outside_autosave_off();
    make_two_tabs(&mut s);
    assert_eq!(s.app().tabs.entries.len(), 2);
    let first = tab_id(&s, 0);
    let mut tabs = std::mem::take(&mut s.shell.app.tabs);
    tabs.switch_to(1, &mut s.shell.app);
    s.shell.app.tabs = tabs;
    wait(&mut s, SETTLED);
    let err = close_while_pumping(&mut s, &mut session, json!({"unsaved": "discard"}));
    assert!(err.is_err(), "close of a hidden tab is not closed:true: {err:?}");
    let mut tabs = std::mem::take(&mut s.shell.app.tabs);
    if let Some(idx) = tabs.entries.iter().position(|t| t.id == first) {
        tabs.switch_to(idx, &mut s.shell.app);
    }
    s.shell.app.tabs = tabs;
    wait(&mut s, SETTLED);
    assert_eq!(s.app().tabs.entries.len(), 2, "coming back did not apply the stale close");
    assert!(s.app().tabs.entries.iter().any(|t| t.id == first), "the named tab is still open");
    let _ = path;
    finish(s, workspace);
}

#[cfg(unix)]
fn restore_mode(path: &std::path::Path, mode: u32) {
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode));
}

#[cfg(unix)]
#[test]
fn titled_read_only_file_close_save_is_not_path_required() {
    use std::os::unix::fs::PermissionsExt;
    let (mut s, mut session, path, workspace, _dir) = live_outside_autosave_off();
    let on_disk = std::fs::read(&path).unwrap();
    edit(&mut s);
    assert!(s.app().has_unsaved_edits(), "human edit lights the tab");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o444)).unwrap();
    if crate::io::save_path(&s.app().doc, &path).is_ok() {
        restore_mode(&path, 0o644);
        finish(s, workspace);
        return;
    }
    let tabs = s.app().tabs.entries.len();
    let err = close_while_pumping(&mut s, &mut session, json!({"unsaved": "save"})).expect_err("read-only save close");
    restore_mode(&path, 0o644);
    assert_ne!(err["code"], "PATH_REQUIRED", "read-only is not a missing path: {err}");
    assert_eq!(err["code"], "INVALID_ARGUMENTS", "{err}");
    let message = err["message"].as_str().unwrap_or("");
    assert!(message.contains("is read-only"), "{err}");
    assert!(message.contains(&path.display().to_string()) || message.contains("outside.designos"), "message names the path: {err}");
    assert_eq!(s.app().tabs.entries.len(), tabs, "tab stayed open");
    assert!(s.app().has_unsaved_edits(), "still dirty");
    assert_eq!(std::fs::read(&path).unwrap(), on_disk, "file bytes unchanged");
    finish(s, workspace);
}

#[test]
fn untitled_close_save_without_path_is_path_required() {
    let dir = TmpDir::new();
    let mut s = Sim::with(Options { library: Some(dir.0.join("lib")), ..Default::default() });
    s.shell.app.tabs.always_visible = true;
    wait(&mut s, SETTLED);
    crate::comments::settings::save_autosave_ai_changes(s.app().lib.as_ref().expect("library").root(), false).unwrap();
    s.shell.app.file_path = None;
    s.shell.app.lib_file = None;
    edit(&mut s);
    assert!(s.app().file_path.is_none(), "untitled");
    assert!(s.app().has_unsaved_edits(), "dirty untitled");
    let workspace = dir.0.join("mcp-workspace");
    std::fs::create_dir_all(&workspace).unwrap();
    let mut session = designos_mcp::server::Session::with_session_dir(&workspace, &s.app().terminal.session_dir).expect("mcp session");
    session.set_close_ack_timeout(Duration::from_millis(800));
    let tabs = s.app().tabs.entries.len();
    let err = close_while_pumping(&mut s, &mut session, json!({"unsaved": "save"})).expect_err("untitled save needs a path");
    assert_eq!(err["code"], "PATH_REQUIRED", "{err}");
    assert_eq!(s.app().tabs.entries.len(), tabs, "tab stayed open");
    assert!(s.app().has_unsaved_edits(), "still dirty");
    finish(s, workspace);
}

#[cfg(unix)]
#[test]
fn close_save_into_unwritable_directory_is_not_path_required() {
    use std::os::unix::fs::PermissionsExt;
    let (mut s, mut session, path, workspace, _dir) = live_outside_autosave_off();
    edit(&mut s);
    let locked = workspace.join("locked-dir");
    std::fs::create_dir_all(&locked).unwrap();
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o555)).unwrap();
    if std::fs::File::create(locked.join("probe")).is_ok() {
        restore_mode(&locked, 0o755);
        finish(s, workspace);
        return;
    }
    let dest = locked.join("out.designos");
    let tabs = s.app().tabs.entries.len();
    let err = close_while_pumping(&mut s, &mut session, json!({"unsaved": "save", "path": dest.to_string_lossy()})).expect_err("unwritable dir");
    restore_mode(&locked, 0o755);
    assert_ne!(err["code"], "PATH_REQUIRED", "unwritable directory is not a missing path: {err}");
    assert_eq!(err["code"], "INVALID_ARGUMENTS", "{err}");
    let message = err["message"].as_str().unwrap_or("");
    assert!(message.contains("is read-only") || message.contains("can't be written"), "{err}");
    assert!(
        message.contains(&dest.display().to_string()) || message.contains("out.designos") || message.contains("locked-dir"),
        "message names the path: {err}"
    );
    assert_eq!(s.app().tabs.entries.len(), tabs, "tab stayed open");
    assert!(s.app().has_unsaved_edits(), "still dirty");
    assert!(!dest.exists(), "no partial write");
    let _ = path;
    finish(s, workspace);
}
