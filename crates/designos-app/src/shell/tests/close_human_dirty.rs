use super::leave_prompt::{make_two_tabs, tab_id};
use super::live_agent_edits::finish;
use super::unsaved_dot::{dot, edit, external_sim_with, nodes_on_disk, two_rects, wait, SETTLED};
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

fn close_while_pumping(s: &mut Sim, session: &mut designos_mcp::server::Session, args: serde_json::Value) -> Result<serde_json::Value, serde_json::Value> {
    std::thread::scope(|scope| {
        let h = scope.spawn(|| session.call("designos_close_document", args));
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
