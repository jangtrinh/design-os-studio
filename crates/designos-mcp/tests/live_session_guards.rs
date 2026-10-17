//! A session bound to an open editor (`DESIGNOS_SESSION_DIR`): what the agent may undo after the
//! editor rewrote the canvas, and what happens when its edit cannot reach the editor.

use designos_doc::{Document, Node, NodeId, Rect};
use designos_mcp::server::Session;
use serde_json::json;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, SystemTime},
};

static SEQ: AtomicU64 = AtomicU64::new(0);

struct Dirs {
    workspace: PathBuf,
    session: PathBuf,
}

impl Dirs {
    fn new() -> Self {
        let nonce = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_nanos();
        let seq = SEQ.fetch_add(1, Ordering::Relaxed);
        let base = std::env::temp_dir().join(format!("designos-mcp-guards-{}-{nonce}-{seq}", std::process::id()));
        let (workspace, session) = (base.join("ws"), base.join("session"));
        fs::create_dir_all(&workspace).unwrap();
        fs::create_dir_all(&session).unwrap();
        Self { workspace, session }
    }
}

impl Drop for Dirs {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(self.workspace.parent().unwrap());
    }
}

/// Writes the editor's canvas; `age` keeps its modification time ahead of the previous write.
fn write_canvas(session: &Path, doc: &Document, seconds_ahead: u64) {
    let path = session.join("canvas.designos");
    fs::write(&path, doc.to_json().unwrap()).unwrap();
    let file = fs::OpenOptions::new().write(true).open(&path).unwrap();
    file.set_modified(SystemTime::now() + Duration::from_secs(seconds_ahead)).unwrap();
}

fn node_id(created: &serde_json::Value) -> NodeId {
    NodeId(created["node"]["id"].as_u64().expect("created node id"))
}

fn name_of(session: &Session, id: NodeId) -> Option<String> {
    session.engine.doc.get(id).map(|n| n.name.clone())
}

#[test]
fn undo_after_the_editor_rewrote_the_canvas_never_touches_the_editors_nodes() {
    let dirs = Dirs::new();
    let mut doc = Document::new("Canvas");
    let page = doc.add_page("Page");
    write_canvas(&dirs.session, &doc, 0);
    let mut session = Session::with_session_dir(&dirs.workspace, &dirs.session).unwrap();

    let created = session.call("designos_create_node", json!({ "kind": "frame", "name": "Agent frame", "width": 50.0, "height": 50.0 })).unwrap();
    let agent_id = node_id(&created);

    // Meanwhile the user drew a layer that took the agent's id; the editor renumbered the agent's
    // frame and republished its canvas.
    let mut editor = doc.clone();
    let user = editor.alloc_id();
    assert_eq!(user, agent_id, "precondition: the user's layer takes the id the agent used");
    editor.insert(page, Node::frame(user, Rect::new(0.0, 0.0, 20.0, 20.0))).unwrap();
    editor.get_mut(user).unwrap().name = "User frame".into();
    let renumbered = editor.alloc_id();
    editor.insert(page, Node::frame(renumbered, Rect::new(0.0, 0.0, 50.0, 50.0))).unwrap();
    editor.get_mut(renumbered).unwrap().name = "Agent frame".into();
    write_canvas(&dirs.session, &editor, 5);

    let undone = session.call("designos_undo", json!({})).unwrap();
    assert_eq!(undone["changed"], false, "an edit recorded against the old canvas must not be undone on the new one");
    assert_eq!(name_of(&session, user).as_deref(), Some("User frame"), "undo removed or changed the user's layer");
    assert_eq!(name_of(&session, renumbered).as_deref(), Some("Agent frame"));
    let redone = session.call("designos_redo", json!({})).unwrap();
    assert_eq!(redone["changed"], false);
    assert_eq!(name_of(&session, user).as_deref(), Some("User frame"));

    // Edits made on the new canvas are undoable again, and only those.
    let second = session.call("designos_create_node", json!({ "kind": "frame", "name": "Second", "width": 10.0, "height": 10.0 })).unwrap();
    let second = node_id(&second);
    assert_eq!(session.call("designos_undo", json!({})).unwrap()["changed"], true);
    assert_eq!(name_of(&session, second), None);
    assert_eq!(name_of(&session, user).as_deref(), Some("User frame"));
}

#[test]
fn an_edit_the_editor_never_received_is_rolled_back_and_reported() {
    let dirs = Dirs::new();
    let mut doc = Document::new("Canvas");
    doc.add_page("Page");
    write_canvas(&dirs.session, &doc, 0);
    let mut session = Session::with_session_dir(&dirs.workspace, &dirs.session).unwrap();
    let log = dirs.session.join("doc-patch.jsonl");

    let first = node_id(&session.call("designos_create_node", json!({ "kind": "frame", "name": "A", "width": 10.0, "height": 10.0 })).unwrap());
    assert!(log.is_file());

    // The log cannot be appended to (here: something else sits at its path).
    fs::remove_file(&log).unwrap();
    fs::create_dir(&log).unwrap();
    let err = session.call("designos_create_node", json!({ "kind": "frame", "name": "B", "width": 10.0, "height": 10.0 })).unwrap_err();
    assert!(err.contains("doc-patch.jsonl"), "{err}");
    let nodes_named = |s: &Session, name: &str| s.engine.doc.nodes.values().filter(|n| n.name == name).count();
    assert_eq!(nodes_named(&session, "B"), 0, "an edit the editor did not get must not stay in the agent's document");
    assert!(session.engine.history.can_undo(), "the earlier edit stays undoable");

    let renamed_err = session.call("designos_update_node", json!({ "node_id": first.0, "name": "Renamed" })).unwrap_err();
    assert!(renamed_err.contains("doc-patch.jsonl"), "{renamed_err}");
    assert_eq!(name_of(&session, first).as_deref(), Some("A"));

    // Once the log works again, undo reverts the last edit that did arrive, and the log holds
    // only edits that were really applied.
    fs::remove_dir(&log).unwrap();
    assert_eq!(session.call("designos_undo", json!({})).unwrap()["changed"], true);
    assert_eq!(name_of(&session, first), None, "undo must remove A, not an edit that was rolled back");
    let lines: Vec<String> = fs::read_to_string(&log).unwrap().lines().map(str::to_string).collect();
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert!(lines[0].contains("\"remove\""), "{}", lines[0]);
}

#[test]
fn a_failed_write_to_the_editor_keeps_what_the_agent_can_redo() {
    let dirs = Dirs::new();
    let mut doc = Document::new("Canvas");
    doc.add_page("Page");
    write_canvas(&dirs.session, &doc, 0);
    let mut session = Session::with_session_dir(&dirs.workspace, &dirs.session).unwrap();
    let log = dirs.session.join("doc-patch.jsonl");

    let first = node_id(&session.call("designos_create_node", json!({ "kind": "frame", "name": "A", "width": 10.0, "height": 10.0 })).unwrap());
    assert_eq!(session.call("designos_undo", json!({})).unwrap()["changed"], true);
    assert_eq!(name_of(&session, first), None);
    assert!(session.engine.history.can_redo(), "precondition: the undone edit can be redone");

    // The next edit cannot reach the editor (something else sits at the log's path), so it is
    // rolled back, and the redo step it cleared comes back with it.
    fs::remove_file(&log).unwrap();
    fs::create_dir(&log).unwrap();
    let err = session.call("designos_create_node", json!({ "kind": "frame", "name": "B", "width": 10.0, "height": 10.0 })).unwrap_err();
    assert!(err.contains("doc-patch.jsonl"), "{err}");
    assert!(session.engine.history.can_redo(), "a failed edit must not cost the agent its redo");

    fs::remove_dir(&log).unwrap();
    assert_eq!(session.call("designos_redo", json!({})).unwrap()["changed"], true);
    assert_eq!(name_of(&session, first).as_deref(), Some("A"));
}
