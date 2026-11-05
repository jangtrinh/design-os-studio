//! AC-05: auto-save after an agent tool batch on a document outside the library.

use super::live_agent_edits::finish;
use super::unsaved_dot::{dot, external_sim_with, nodes_on_disk, two_rects, wait, SETTLED};
use super::*;
use serde_json::json;

fn live_outside() -> (Sim, designos_mcp::server::Session, std::path::PathBuf, std::path::PathBuf, super::unsaved_dot::TmpDir) {
    let (s, dir, path) = external_sim_with(&two_rects());
    let workspace = std::env::temp_dir().join(format!("designos-ai-autosave-ws-{}", crate::live_sync_tests::unique_tab_id()));
    std::fs::create_dir_all(&workspace).unwrap();
    let session = designos_mcp::server::Session::with_session_dir(&workspace, &s.app().terminal.session_dir).expect("mcp session");
    (s, session, path, workspace, dir)
}

#[test]
fn agent_batch_autosaves_outside_the_library_when_the_setting_is_on() {
    let (mut s, mut session, path, workspace, _dir) = live_outside();
    let on_disk = nodes_on_disk(&path);
    let page = session.call("designos_get_document", json!({})).unwrap()["pages"][0]["node_id"].as_u64().unwrap();
    session.call("designos_create_node", json!({"parent_id": page, "kind": "frame", "name": "AgentBox"})).expect("agent edit");
    wait(&mut s, SETTLED);
    assert!(s.app().lib_file.is_none(), "this file is outside the library");
    assert_ne!(nodes_on_disk(&path), on_disk, "default-on setting saved the agent edit");
    assert!(!s.app().has_unsaved_edits(), "dirty indicator cleared");
    assert!(!dot(&s));
    finish(s, workspace);
}

#[test]
fn agent_batch_keeps_dirty_when_autosave_is_off() {
    let (mut s, mut session, path, workspace, _dir) = live_outside();
    let root = s.app().lib.as_ref().expect("library").root().to_path_buf();
    crate::comments::settings::save_autosave_ai_changes(&root, false).unwrap();
    let on_disk = nodes_on_disk(&path);
    let page = session.call("designos_get_document", json!({})).unwrap()["pages"][0]["node_id"].as_u64().unwrap();
    session.call("designos_create_node", json!({"parent_id": page, "kind": "frame", "name": "AgentBox"})).expect("agent edit");
    wait(&mut s, SETTLED);
    assert_eq!(nodes_on_disk(&path), on_disk, "setting off: file unchanged");
    assert!(s.app().has_unsaved_edits(), "dirty stays until the agent saves");
    finish(s, workspace);
}
