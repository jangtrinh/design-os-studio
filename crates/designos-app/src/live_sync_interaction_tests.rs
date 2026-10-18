//! Agent edits held while the user is mid-edit must stay coherent: the agent keeps seeing its own
//! held edit, an edit held for one document never lands on another, and an open shape builder
//! (which rebuilds from geometry it captured when it opened) counts as an edit in progress.

use crate::app::App;
use crate::canvas::{Interaction, MoveState};
use crate::edit::Tx;
use crate::live_sync_tests::unique_tab_id;
use crate::terminal::TerminalState;
use designos_doc::{Document, Node, NodeId, Rect, Vec2};
use serde_json::json;
use std::path::PathBuf;

pub(crate) struct Live {
    pub(crate) app: App,
    pub(crate) session: designos_mcp::server::Session,
    pub(crate) dir: PathBuf,
    pub(crate) workspace: PathBuf,
    pub(crate) dragged: NodeId,
}

pub(crate) fn live() -> Live {
    let mut doc = Document::new("Live");
    let page = doc.add_page("Page");
    let id = doc.alloc_id();
    doc.insert(page, Node::frame(id, Rect::new(0.0, 0.0, 100.0, 100.0))).unwrap();
    doc.get_mut(id).unwrap().name = "Hero".into();
    let mut app = App::new(doc);
    app.terminal = TerminalState::for_tab(unique_tab_id());
    let dir = app.terminal.session_dir.clone();
    let workspace = std::env::temp_dir().join(format!("designos-live-interaction-ws-{}", unique_tab_id()));
    std::fs::create_dir_all(&workspace).unwrap();
    app.terminal.sync_live_file_context(&app.doc, app.page, &app.selection, None);
    let session = designos_mcp::server::Session::with_session_dir(&workspace, &dir).expect("mcp session");
    Live { app, session, dir, workspace, dragged: id }
}

impl Live {
    pub(crate) fn agent_renames(&mut self, id: NodeId, name: &str) {
        self.session.call("designos_update_node", json!({ "node_id": id.0, "name": name })).expect("update_node");
    }

    /// The editor reads the log and runs what it finds, as every frame does.
    pub(crate) fn deliver(&mut self) {
        let actions = self.app.terminal.poll_live_agent_bridge();
        crate::shell::execute_terminal_actions(&mut self.app, actions);
    }

    /// The user is dragging `dragged` 50 px to the right; the document already shows it.
    pub(crate) fn start_drag(&mut self) {
        let mut tx = Tx::new();
        tx.ed.node(&mut self.app.doc, self.dragged).unwrap().transform.tx += 50.0;
        let state = MoveState {
            tx,
            ids: vec![self.dragged],
            start: Vec2::ZERO,
            orig_bbox: Rect::new(0.0, 0.0, 100.0, 100.0),
            targets: Vec::new(),
            moved: true,
            reorder: None,
            dup_blocked: None,
        };
        self.app.canvas.inter = Interaction::Move(state);
    }

    pub(crate) fn finish(mut self) {
        self.app.terminal.shutdown();
        let _ = std::fs::remove_dir_all(&self.dir);
        let _ = std::fs::remove_dir_all(&self.workspace);
    }
}

/// True when held patches were waiting and the idle-frame hook applied all of them.
pub(crate) fn apply_pending(app: &mut App) -> bool {
    let had = !app.terminal.live_patch.pending.is_empty();
    crate::live_patch_apply::apply_pending_live_patches(app);
    had && app.terminal.live_patch.pending.is_empty()
}

/// The agent switches documents while its earlier edit is held: the held edit must not land on the new document.
#[test]
fn probe_held_edit_does_not_land_on_a_document_the_agent_opened_later() {
    let mut l = live();
    let hero = l.dragged;
    l.start_drag();
    l.agent_renames(hero, "Agent hero");
    l.deliver();
    assert_eq!(l.app.terminal.live_patch.pending.len(), 1);
    let mut other = Document::new("Other");
    let op = other.add_page("Page");
    let theirs = other.alloc_id();
    other.insert(op, Node::frame(theirs, Rect::new(0.0, 0.0, 10.0, 10.0))).unwrap();
    other.get_mut(theirs).unwrap().name = "Fresh".into();
    std::fs::write(l.workspace.join("other.designos"), other.to_json().unwrap()).unwrap();
    l.session.call("designos_open_file", json!({ "path": "other.designos", "discard_changes": true })).expect("open_file");
    std::thread::sleep(std::time::Duration::from_millis(20));
    l.deliver();
    l.app.canvas.inter = Interaction::None; // drag state belongs to the replaced doc; drop it
    apply_pending(&mut l.app);
    let names: Vec<String> = l.app.doc.nodes.values().map(|n| n.name.clone()).collect();
    assert!(!names.iter().any(|n| n == "Agent hero"), "an edit for the old document landed on the new one: {names:?}");
    l.finish();
}

/// The canvas is republished after a drag ends, not on every frame of it.
#[test]
fn the_canvas_is_not_republished_during_a_drag_but_right_after_it() {
    let mut l = live();
    let hero = l.dragged;
    let published_x = |l: &Live| {
        let raw = std::fs::read_to_string(&l.app.terminal.live_canvas_path).unwrap();
        Document::from_json(&raw).unwrap().get(hero).unwrap().transform.tx
    };
    assert_eq!(published_x(&l), 0.0, "precondition: the idle canvas is published");
    l.start_drag();
    crate::live_patch_apply::publish_live_context(&mut l.app);
    assert_eq!(published_x(&l), 0.0, "a mid-drag state must not reach the agent");
    l.app.canvas.inter = Interaction::None;
    crate::live_patch_apply::publish_live_context(&mut l.app);
    assert_eq!(published_x(&l), 50.0, "the first idle frame publishes");
    l.finish();
}

/// Edits the agent makes on the new document queue behind it and land once the canvas is idle.
#[test]
fn edits_on_a_document_opened_during_a_drag_land_after_it_replaces_the_old_one() {
    let mut l = live();
    let hero = l.dragged;
    l.start_drag();
    l.agent_renames(hero, "Agent hero");
    let mut other = Document::new("Other");
    let op = other.add_page("Page");
    // Its layer must not share an id with the old document's hero, or the old edit could land on it.
    other.reserve_ids(NodeId(99));
    let theirs = other.alloc_id();
    assert_ne!(theirs, hero);
    other.insert(op, Node::frame(theirs, Rect::new(0.0, 0.0, 10.0, 10.0))).unwrap();
    other.get_mut(theirs).unwrap().name = "Fresh".into();
    std::fs::write(l.workspace.join("other.designos"), other.to_json().unwrap()).unwrap();
    l.session.call("designos_open_file", json!({ "path": "other.designos", "discard_changes": true })).expect("open_file");
    std::thread::sleep(std::time::Duration::from_millis(20));
    l.agent_renames(theirs, "Agent fresh");
    l.deliver();
    assert_eq!(l.app.doc.name, "Live", "the document is not replaced in the middle of a drag");
    l.app.canvas.inter = Interaction::None;
    assert!(apply_pending(&mut l.app));
    assert_eq!(l.app.doc.name, "Other");
    let names: Vec<String> = l.app.doc.nodes.values().map(|n| n.name.clone()).collect();
    assert!(names.iter().any(|n| n == "Agent fresh") && !names.iter().any(|n| n == "Agent hero"), "{names:?}");
    // The old edit was dropped on purpose; it was not tried on the new document and found missing.
    let toast = l.app.toast.as_ref().map(|t| t.text.clone()).unwrap_or_default();
    assert!(toast.contains("1 earlier edit"), "the dropped edit is reported: {toast:?}");
    l.finish();
}

/// While an edit is held, the editor republishes its canvas without it; the agent's next call must still see its own edit.
#[test]
fn probe_agent_still_sees_its_held_edit_after_the_editor_republishes() {
    let mut l = live();
    let hero = l.dragged;
    l.start_drag();
    let made = l.session.call("designos_create_node", json!({ "kind": "frame", "name": "Card", "width": 10.0, "height": 10.0 })).expect("create");
    let card = made["node"]["id"].as_u64().unwrap();
    l.deliver();
    assert_eq!(l.app.terminal.live_patch.pending.len(), 1, "held during the drag");
    // The drag continues: the document changes and the editor republishes its canvas.
    std::thread::sleep(std::time::Duration::from_millis(20));
    if let Interaction::Move(s) = &mut l.app.canvas.inter {
        s.tx.ed.node(&mut l.app.doc, hero).unwrap().transform.tx += 5.0;
    }
    l.app.terminal.sync_live_file_context(&l.app.doc, l.app.page, &l.app.selection, None);
    let child = l.session.call("designos_create_node", json!({ "kind": "rectangle", "name": "Inside", "parent_id": card, "width": 5.0, "height": 5.0 }));
    assert!(child.is_ok(), "agent lost sight of its own held edit: {child:?}");
    l.finish();
}

/// Shape builder session open (between strokes): the builder rebuilds the layers it touched from
/// geometry it captured, in a different coordinate frame. An agent move of such a layer is
/// refused and reported; an appearance edit still follows the layer to its replacement.
#[test]
fn probe_agent_move_survives_an_open_shape_builder() {
    let mut doc = Document::new("Live");
    let page = doc.add_page("Page");
    let a = doc.alloc_id();
    doc.insert(page, Node::rectangle(a, Rect::new(0.0, 0.0, 100.0, 100.0))).unwrap();
    let b = doc.alloc_id();
    doc.insert(page, Node::rectangle(b, Rect::new(50.0, 0.0, 100.0, 100.0))).unwrap();
    let mut app = App::new(doc);
    app.terminal = TerminalState::for_tab(unique_tab_id());
    let dir = app.terminal.session_dir.clone();
    let workspace = std::env::temp_dir().join(format!("designos-live-builder-ws-{}", unique_tab_id()));
    std::fs::create_dir_all(&workspace).unwrap();
    app.terminal.sync_live_file_context(&app.doc, app.page, &app.selection, None);
    let mut session = designos_mcp::server::Session::with_session_dir(&workspace, &dir).expect("mcp session");
    app.selection = vec![a, b];
    assert!(crate::canvas::builder::start(&mut app));
    session.call("designos_update_node", json!({ "node_id": a.0, "x": 500.0 })).expect("agent move");
    session.call("designos_update_node", json!({ "node_id": a.0, "name": "Agent A" })).expect("agent rename");
    let actions = app.terminal.poll_live_agent_bridge();
    crate::shell::execute_terminal_actions(&mut app, actions);
    // The user brushes the part of A that only A covers, then leaves the tool.
    crate::canvas::builder::press(&mut app, Vec2::new(25.0, 50.0), false);
    crate::canvas::builder::release(&mut app);
    crate::canvas::builder::finish(&mut app);
    apply_pending(&mut app);
    let layers: Vec<(String, f32)> = app.doc.nodes.values().filter(|n| n.parent == Some(page)).map(|n| (n.name.clone(), n.transform.tx)).collect();
    assert!(layers.iter().all(|(_, x)| *x < 400.0), "the agent's move landed on a layer in another coordinate frame: {layers:?}");
    assert!(layers.iter().any(|(name, _)| name == "Agent A"), "the rename did not follow A to its replacement: {layers:?}");
    let toast = app.toast.as_ref().map(|t| t.text.clone()).unwrap_or_default();
    assert!(toast.contains("reshaped by the shape builder"), "the refused move is reported: {toast:?}");
    app.terminal.shutdown();
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&workspace);
}

#[test]
fn comment_only_edit_in_studio_republishes_canvas_to_agent() {
    let mut l = live();
    let anchor = designos_doc::CommentAnchor { page: l.app.page, node: Some(l.dragged), offset: Vec2::new(10.0, 10.0) };
    std::thread::sleep(std::time::Duration::from_millis(20));
    assert!(l.app.post_comment(anchor, "Please tighten padding here").is_some());
    crate::live_patch_apply::publish_live_context(&mut l.app);
    let raw = std::fs::read_to_string(&l.app.terminal.live_canvas_path).unwrap();
    let published = Document::from_json(&raw).unwrap();
    assert_eq!(published.comments.len(), 1, "comment-only edit in Studio must be republished to live-canvas.designos");
    assert_eq!(published.comments[0].messages[0].text, "Please tighten padding here");
    let listed = l.session.call("designos_list_comments", json!({})).expect("list_comments");
    assert_eq!(listed["total"].as_u64(), Some(1), "agent must see the newly posted comment without waiting for a layer edit: {listed}");
    l.finish();
}
