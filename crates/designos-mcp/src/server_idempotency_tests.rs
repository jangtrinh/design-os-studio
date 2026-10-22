//! Reconnect, Studio-side undo and diff-image behaviour of the idempotency ledger (E15-6 §3.4, §3.6, §3.7, §8.6, §10).

use super::*;
use crate::tools::errors::ErrorCode;
use designos_doc::live_patch::{LiveOp, SiblingHint};
use designos_doc::{Document, Node, NodeId, Rect};
use std::time::{Duration, UNIX_EPOCH};

fn temp_dirs(tag: &str) -> (PathBuf, PathBuf) {
    let base = std::env::temp_dir().join(format!(
        "designos-e15-6b-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
    ));
    let (ws, sess) = (base.join("ws"), base.join("sess"));
    fs::create_dir_all(&ws).unwrap();
    fs::create_dir_all(&sess).unwrap();
    (ws, sess)
}

fn cleanup(ws: &Path) {
    let _ = fs::remove_dir_all(ws.parent().unwrap());
}

/// A document with one page and one frame per name; returns the page and the frame ids.
fn doc_with_frames(names: &[&str]) -> (Document, NodeId, Vec<NodeId>) {
    let mut doc = Document::new("Initial");
    let page = doc.add_page("Page 1");
    let mut ids = Vec::new();
    for name in names {
        let id = doc.alloc_id();
        let mut node = Node::frame(id, Rect::new(0.0, 0.0, 40.0, 40.0));
        node.name = (*name).to_string();
        doc.insert(page, node).unwrap();
        ids.push(id);
    }
    (doc, page, ids)
}

/// Writes `doc` as the editor's canvas snapshot, a moment after the previous one.
fn write_canvas(sess: &Path, doc: &Document) {
    std::thread::sleep(Duration::from_millis(20));
    fs::write(sess.join("canvas.designos"), doc.to_json().unwrap()).unwrap();
}

fn canvas_base(sess: &Path) -> u64 {
    fs::metadata(sess.join("canvas.designos")).unwrap().modified().unwrap().duration_since(UNIX_EPOCH).unwrap().as_nanos() as u64
}

fn patch_log_len(sess: &Path) -> u64 {
    fs::metadata(sess.join("doc-patch.jsonl")).map(|m| m.len()).unwrap_or(0)
}

fn name_of(s: &Session, id: NodeId) -> Option<String> {
    s.engine.doc.get(id).map(|n| n.name.clone())
}

#[test]
fn ledger_survives_reconnect_over_a_lagging_canvas() {
    let (ws, sess) = temp_dirs("lagging-canvas");
    let (doc, page, _) = doc_with_frames(&[]);
    write_canvas(&sess, &doc);
    let payload = json!({"idempotency_key": "k1", "ops": [{"op": "create", "parent": page.0, "kind": "frame", "name": "Hero"}]});
    let (created_id, first_reply) = {
        let mut s1 = Session::with_session_dir(&ws, &sess).unwrap();
        let reply = s1.call("designos_apply_batch", payload.clone()).unwrap();
        (reply["created"][0].as_u64().unwrap(), reply)
    };
    // Studio has not rewritten canvas.designos: the new process loads a canvas without the layer.
    let log_len = patch_log_len(&sess);

    let mut s2 = Session::with_session_dir(&ws, &sess).unwrap();
    assert!(s2.engine.doc.get(NodeId(created_id)).is_some(), "the unabsorbed patch line is replayed onto the loaded canvas");
    let count = s2.engine.doc.nodes.len();
    let (reply2, meta2) = s2.call_with_meta("designos_apply_batch", payload).unwrap();
    assert_eq!(reply2, first_reply);
    assert_eq!(meta2.unwrap().get("designos/idempotent_replay"), Some(&json!(true)));
    assert_eq!(s2.engine.doc.nodes.len(), count, "no second layer");
    assert_eq!(patch_log_len(&sess), log_len, "the reconnect replay must not append to the edit log");
    assert_eq!(s2.engine.history.undo_depth(), 0, "a replayed patch is not an undo step");
    cleanup(&ws);
}

/// Rewrite of the earlier `reconnect_without_document_state_is_stale`: with no canvas file at all,
/// the patch log alone now carries the write across the reconnect.
#[test]
fn reconnect_without_canvas_replays_the_patch_log() {
    let (ws, sess) = temp_dirs("no-canvas-patch-log");
    let payload;
    let created_id: u64;
    let first_reply: Value;
    {
        let mut s1 = Session::with_session_dir(&ws, &sess).unwrap();
        let page = s1.engine.doc.pages[0].0;
        payload = json!({"idempotency_key": "k1", "ops": [{"op": "create", "parent": page, "kind": "frame", "name": "Hero"}]});
        first_reply = s1.call("designos_apply_batch", payload.clone()).unwrap();
        created_id = first_reply["created"][0].as_u64().unwrap();
    }
    assert!(!sess.join("canvas.designos").exists(), "this reconnect has no canvas file");
    let log_len = patch_log_len(&sess);

    let mut s2 = Session::with_session_dir(&ws, &sess).unwrap();
    assert!(s2.engine.doc.get(NodeId(created_id)).is_some(), "the patch log restores the layer");
    let count = s2.engine.doc.nodes.len();
    let (reply, meta) = s2.call_with_meta("designos_apply_batch", payload).unwrap();
    assert_eq!(reply, first_reply);
    assert_eq!(meta.unwrap().get("designos/idempotent_replay"), Some(&json!(true)));
    assert_eq!(s2.engine.doc.nodes.len(), count);
    assert_eq!(patch_log_len(&sess), log_len);
    cleanup(&ws);
}

#[test]
fn reconnect_when_both_canvas_and_patch_hold_node_does_not_duplicate() {
    let (ws, sess) = temp_dirs("both-hold");
    let (doc, page, _) = doc_with_frames(&[]);
    write_canvas(&sess, &doc);
    let payload = json!({"idempotency_key": "k1", "ops": [{"op": "create", "parent": page.0, "kind": "frame", "name": "Hero"}]});
    let (created_id, mut doc) = {
        let mut s1 = Session::with_session_dir(&ws, &sess).unwrap();
        let id = s1.call("designos_apply_batch", payload.clone()).unwrap()["created"][0].as_u64().unwrap();
        (id, s1.engine.doc.clone())
    };
    // Studio absorbed the insert, then the user renamed the layer.
    doc.get_mut(NodeId(created_id)).unwrap().name = "Renamed by the user".into();
    write_canvas(&sess, &doc);
    let count = doc.nodes.len();

    let mut s2 = Session::with_session_dir(&ws, &sess).unwrap();
    assert_eq!(s2.engine.doc.nodes.len(), count, "the absorbed insert must not be applied again");
    assert_eq!(name_of(&s2, NodeId(created_id)).as_deref(), Some("Renamed by the user"), "the user's edit survives");
    let (_, meta) = s2.call_with_meta("designos_apply_batch", payload).unwrap();
    assert_eq!(meta.unwrap().get("designos/idempotent_replay"), Some(&json!(true)));
    assert_eq!(s2.engine.doc.nodes.len(), count);
    cleanup(&ws);
}

#[test]
fn reconnect_replays_pending_update_and_delete_patches() {
    let (ws, sess) = temp_dirs("pending-update-delete");
    let (doc, _, ids) = doc_with_frames(&["N", "M"]);
    let (n, m) = (ids[0], ids[1]);
    write_canvas(&sess, &doc);
    let k1 = json!({"idempotency_key": "k1", "node_id": n.0, "name": "N updated"});
    let k2 = json!({"idempotency_key": "k2", "node_id": m.0});
    {
        let mut s1 = Session::with_session_dir(&ws, &sess).unwrap();
        s1.call("designos_update_node", k1.clone()).unwrap();
        s1.call("designos_delete_node", k2.clone()).unwrap();
    }

    let mut s2 = Session::with_session_dir(&ws, &sess).unwrap();
    assert_eq!(name_of(&s2, n).as_deref(), Some("N updated"));
    assert!(s2.engine.doc.get(m).is_none(), "the pending delete is replayed");
    let (_, meta1) = s2.call_with_meta("designos_update_node", k1).unwrap();
    assert_eq!(meta1.unwrap().get("designos/idempotent_replay"), Some(&json!(true)));
    let (_, meta2) = s2.call_with_meta("designos_delete_node", k2).unwrap();
    assert_eq!(meta2.unwrap().get("designos/idempotent_replay"), Some(&json!(true)));
    cleanup(&ws);
}

#[test]
fn reconnect_does_not_reapply_absorbed_patches_after_user_edit() {
    let (ws, sess) = temp_dirs("absorbed-user-edit");
    let (doc, page, ids) = doc_with_frames(&["N", "Y"]);
    let (n, y) = (ids[0], ids[1]);
    write_canvas(&sess, &doc);
    let k1 = json!({"idempotency_key": "k1", "node_id": n.0, "name": "Agent name"});
    {
        let mut s1 = Session::with_session_dir(&ws, &sess).unwrap();
        s1.call("designos_update_node", k1.clone()).unwrap();
    }
    // Studio absorbed the rename, then the user renamed N again.
    let mut studio = doc.clone();
    studio.get_mut(n).unwrap().name = "User name".into();
    write_canvas(&sess, &studio);
    // A newer, unabsorbed insert from a later agent call reuses the id of another layer (Y).
    let mut clash = Node::frame(y, Rect::new(5.0, 5.0, 10.0, 10.0));
    clash.name = "Intruder".into();
    let patch = LivePatch::new(vec![LiveOp::Insert { parent: page, at: SiblingHint::default(), nodes: vec![clash] }]).with_base(Some(canvas_base(&sess)));
    let mut log = fs::OpenOptions::new().append(true).create(true).open(sess.join("doc-patch.jsonl")).unwrap();
    writeln!(log, "{}", serde_json::to_string(&patch).unwrap()).unwrap();
    drop(log);

    let mut s2 = Session::with_session_dir(&ws, &sess).unwrap();
    assert_eq!(name_of(&s2, n).as_deref(), Some("User name"), "the absorbed update must not overwrite the user's edit");
    assert_eq!(name_of(&s2, y).as_deref(), Some("Y"), "the editor's layer keeps its id");
    let intruders: Vec<_> = s2.engine.doc.nodes.values().filter(|node| node.name == "Intruder").collect();
    assert_eq!(intruders.len(), 1, "the unabsorbed colliding insert is applied once, under a fresh id");
    assert_ne!(intruders[0].id, y);
    let (_, meta) = s2.call_with_meta("designos_update_node", k1).unwrap();
    assert_eq!(meta.unwrap().get("designos/idempotent_replay"), Some(&json!(true)), "k1 still replays through the presence check");
    cleanup(&ws);
}

#[test]
fn reconnect_without_document_or_patch_state_is_stale() {
    let (ws, sess) = temp_dirs("no-canvas-no-patch");
    let created_id: u64;
    let page: u64;
    let payload;
    {
        let mut s1 = Session::with_session_dir(&ws, &sess).unwrap();
        page = s1.engine.doc.pages[0].0;
        payload = json!({"idempotency_key": "k1", "ops": [{"op": "create", "parent": page, "kind": "frame", "name": "Hero"}]});
        created_id = s1.call("designos_apply_batch", payload.clone()).unwrap()["created"][0].as_u64().unwrap();
    }
    // Neither canvas.designos nor doc-patch.jsonl holds the layer.
    fs::write(sess.join("doc-patch.jsonl"), "").unwrap();

    let mut s2 = Session::with_session_dir(&ws, &sess).unwrap();
    let count = s2.engine.doc.nodes.len();
    let depth = s2.engine.history.undo_depth();
    let err = s2.call("designos_apply_batch", payload).unwrap_err();
    assert_eq!(err.code, ErrorCode::IdempotencyKeyStale);
    assert!(err.safe_to_retry);
    assert!(err.next_step.contains("Do not send it again unless you have verified the target nodes are absent"), "{}", err.next_step);
    assert!(err.corrected_example.is_none(), "a ready-made resend with a new key invites the duplicate the error warns about: {err:?}");
    assert_eq!(s2.engine.doc.nodes.len(), count);
    assert_eq!(s2.engine.history.undo_depth(), depth);
    assert_eq!(s2.call("designos_get_node", json!({"node_id": created_id})).unwrap_err().code, ErrorCode::NodeNotFound);
    cleanup(&ws);
}

#[test]
fn unabsorbed_patches_keeps_only_lines_after_the_last_switch_made_on_this_snapshot() {
    use crate::tools::ledger_reconnect::unabsorbed_patches;
    let line = |base: Option<u64>, label: &str| serde_json::to_string(&LivePatch::new(Vec::new()).with_base(base).with_label(Some(label.into()))).unwrap();
    let switch = serde_json::to_string(&DocEpoch::of(1, b"doc")).unwrap();
    let log = [
        line(Some(7), "before the switch"),
        switch,
        "not json".to_string(),
        line(Some(9), "older snapshot"),
        line(Some(10), "this snapshot"),
        line(None, "no canvas"),
    ]
    .join("\n");
    let labels = |base: Option<u64>| unabsorbed_patches(log.as_bytes(), base).into_iter().filter_map(|p| p.label).collect::<Vec<_>>();
    assert_eq!(labels(Some(10)), vec!["this snapshot"]);
    assert_eq!(labels(None), vec!["no canvas"]);
    assert!(labels(Some(7)).is_empty(), "patches before the last switch belong to the previous document");
}

#[test]
fn a_reconnect_replays_a_version_one_edit_log() {
    use crate::tools::ledger_reconnect::replay_unabsorbed_patches;
    let (mut doc, _, frames) = doc_with_frames(&["Hero"]);
    let mut patch = LivePatch::new(vec![LiveOp::Remove { id: frames[0] }]).with_base(Some(10));
    patch.v = 1;
    let log = serde_json::to_string(&patch).unwrap() + "\n";
    assert_eq!(replay_unabsorbed_patches(&mut doc, log.as_bytes(), Some(10)), Ok(1));
    assert!(doc.get(frames[0]).is_none(), "the version 1 edit was replayed");
}

#[test]
fn a_reconnect_refuses_an_edit_log_from_a_newer_build_and_keeps_the_canvas() {
    use crate::tools::ledger_reconnect::replay_unabsorbed_patches;
    let (mut doc, _, frames) = doc_with_frames(&["Hero"]);
    let before = doc.to_json().unwrap();
    let newer = designos_doc::live_patch::LIVE_PATCH_VERSION + 1;
    // An older line that replays fine, then a line whose ops this build cannot read.
    let good = serde_json::to_string(&LivePatch::new(vec![LiveOp::Remove { id: frames[0] }]).with_base(Some(10))).unwrap();
    let log = format!("{good}\n{{\"v\":{newer},\"base\":10,\"ops\":[{{\"from_the_future\":{{}}}}]}}\n");
    let err = replay_unabsorbed_patches(&mut doc, log.as_bytes(), Some(10)).unwrap_err();
    assert!(err.contains(&format!("Log version {newer}")), "{err}");
    assert!(err.contains("Update Studio and designos-mcp to the same version."), "{err}");
    assert_eq!(doc.to_json().unwrap(), before, "the loaded canvas is kept as it was");
}

#[test]
fn a_newer_line_before_the_last_switch_does_not_block_the_reconnect() {
    use crate::tools::ledger_reconnect::replay_unabsorbed_patches;
    let (mut doc, _, frames) = doc_with_frames(&["Hero"]);
    let newer = designos_doc::live_patch::LIVE_PATCH_VERSION + 1;
    let switch = serde_json::to_string(&DocEpoch::of(1, b"doc")).unwrap();
    let good = serde_json::to_string(&LivePatch::new(vec![LiveOp::Remove { id: frames[0] }]).with_base(Some(10))).unwrap();
    let log = format!("{{\"v\":{newer},\"ops\":[]}}\n{switch}\n{good}\n");
    assert_eq!(replay_unabsorbed_patches(&mut doc, log.as_bytes(), Some(10)), Ok(1));
}

#[test]
fn studio_undo_lets_the_retry_run_as_a_new_write() {
    let (ws, sess) = temp_dirs("studio-undo");
    let (doc, page, _) = doc_with_frames(&[]);
    write_canvas(&sess, &doc);
    let mut s = Session::with_session_dir(&ws, &sess).unwrap();
    let make = |key: &str, name: &str| json!({"idempotency_key": key, "parent_id": page.0, "kind": "frame", "name": name});
    let a = s.call("designos_create_node", make("k1", "A")).unwrap()["node"]["id"].as_u64().unwrap();
    let b = s.call("designos_create_node", make("k2", "B")).unwrap()["node"]["id"].as_u64().unwrap();

    // The user undoes the agent's second layer in Studio: the canvas keeps A and loses B.
    let mut studio = s.engine.doc.clone();
    studio.remove(NodeId(b)).unwrap();
    write_canvas(&sess, &studio);

    let (reply, meta) = s.call_with_meta("designos_create_node", make("k2", "B")).unwrap();
    assert!(meta.is_none(), "the retry after a Studio undo is a new write, not a replay");
    assert!(s.engine.doc.get(NodeId(reply["node"]["id"].as_u64().unwrap())).is_some());
    let (_, meta_a) = s.call_with_meta("designos_create_node", make("k1", "A")).unwrap();
    assert_eq!(meta_a.unwrap().get("designos/idempotent_replay"), Some(&json!(true)), "k1 is still applied");
    assert!(s.engine.doc.get(NodeId(a)).is_some());
    cleanup(&ws);
}

#[test]
fn studio_undo_is_remembered_across_a_reconnect() {
    let (ws, sess) = temp_dirs("studio-undo-reconnect");
    let (doc, page, _) = doc_with_frames(&[]);
    write_canvas(&sess, &doc);
    let make = |key: &str, name: &str| json!({"idempotency_key": key, "parent_id": page.0, "kind": "frame", "name": name});
    {
        let mut s = Session::with_session_dir(&ws, &sess).unwrap();
        s.call("designos_create_node", make("k1", "A")).unwrap();
        let b = s.call("designos_create_node", make("k2", "B")).unwrap()["node"]["id"].as_u64().unwrap();
        let mut studio = s.engine.doc.clone();
        studio.remove(NodeId(b)).unwrap();
        write_canvas(&sess, &studio);
        s.call("designos_get_document", json!({})).unwrap();
    }
    let mut s2 = Session::with_session_dir(&ws, &sess).unwrap();
    let (_, meta) = s2.call_with_meta("designos_create_node", make("k2", "B")).unwrap();
    assert!(meta.is_none(), "the forgotten key is not resurrected from the ledger file");
    cleanup(&ws);
}

/// Writes `doc` as the editor's canvas snapshot with a chosen modification time.
fn write_canvas_at(sess: &Path, doc: &Document, modified: std::time::SystemTime) {
    let path = sess.join("canvas.designos");
    fs::write(&path, doc.to_json().unwrap()).unwrap();
    fs::File::options().write(true).open(&path).unwrap().set_modified(modified).unwrap();
}

fn canvas_mtime(sess: &Path) -> std::time::SystemTime {
    fs::metadata(sess.join("canvas.designos")).unwrap().modified().unwrap()
}

#[test]
fn a_canvas_written_before_the_agents_append_is_not_taken_for_a_studio_undo() {
    let (ws, sess) = temp_dirs("canvas-older-than-write");
    let (doc, page, _) = doc_with_frames(&[]);
    write_canvas(&sess, &doc);
    let mut s = Session::with_session_dir(&ws, &sess).unwrap();
    let make = |key: &str, name: &str| json!({"idempotency_key": key, "parent_id": page.0, "kind": "frame", "name": name});
    std::thread::sleep(Duration::from_millis(5));
    let a = s.call("designos_create_node", make("k1", "A")).unwrap()["node"]["id"].as_u64().unwrap();

    // Studio wrote a canvas while the call ran: it is newer than the one the call read, not newer
    // than the entry (the same millisecond counts as before it), and lacks the agent's frame.
    let created_ms = s.ledger.get("k1").unwrap().created_ms;
    assert!(UNIX_EPOCH + Duration::from_millis(created_ms) > canvas_mtime(&sess), "precondition: the entry is newer than the first canvas");
    write_canvas_at(&sess, &doc, UNIX_EPOCH + Duration::from_millis(created_ms));
    let err = s.call("designos_create_node", make("k1", "A")).unwrap_err();
    assert_eq!(err.code, ErrorCode::IdempotencyKeyStale, "the write is not in this canvas yet, so the retry must not run");
    assert_eq!(s.engine.doc.nodes.values().filter(|n| n.name == "A").count(), 0);
    assert!(s.ledger.get("k1").is_some_and(|e| !e.dead), "the key is kept and still valid");

    // Studio applied the patch and republished: the retry replays and nothing is duplicated.
    let mut applied = doc.clone();
    let mut node = Node::frame(NodeId(a), Rect::new(0.0, 0.0, 40.0, 40.0));
    node.name = "A".into();
    applied.insert(page, node).unwrap();
    write_canvas(&sess, &applied);
    let (_, meta) = s.call_with_meta("designos_create_node", make("k1", "A")).unwrap();
    assert_eq!(meta.unwrap().get("designos/idempotent_replay"), Some(&json!(true)));
    assert_eq!(s.engine.doc.nodes.values().filter(|n| n.name == "A").count(), 1, "no duplicate layer");
    cleanup(&ws);
}

#[test]
fn a_canvas_newer_than_the_write_is_still_a_studio_undo() {
    let (ws, sess) = temp_dirs("canvas-newer-than-write");
    let (doc, page, _) = doc_with_frames(&[]);
    write_canvas(&sess, &doc);
    let mut s = Session::with_session_dir(&ws, &sess).unwrap();
    let make = |key: &str, name: &str| json!({"idempotency_key": key, "parent_id": page.0, "kind": "frame", "name": name});
    s.call("designos_create_node", make("k1", "A")).unwrap();
    // The user undid A in Studio after the agent's write: the canvas is newer than k1.
    write_canvas(&sess, &doc);
    let (_, meta) = s.call_with_meta("designos_create_node", make("k1", "A")).unwrap();
    assert!(meta.is_none(), "a canvas newer than the write is a Studio undo: the retry is a new write");
    cleanup(&ws);
}

#[test]
fn an_mcp_undone_key_stays_dead_after_a_studio_undo_and_a_reconnect() {
    let (ws, sess) = temp_dirs("mcp-undo-studio-undo-reconnect");
    let (doc, page, _) = doc_with_frames(&[]);
    write_canvas(&sess, &doc);
    let make = |key: &str, name: &str| json!({"idempotency_key": key, "parent_id": page.0, "kind": "frame", "name": name});
    {
        let mut s = Session::with_session_dir(&ws, &sess).unwrap();
        s.call("designos_create_node", make("k1", "A")).unwrap();
        s.call("designos_undo", json!({})).unwrap();
        let b = s.call("designos_create_node", make("k2", "B")).unwrap()["node"]["id"].as_u64().unwrap();
        let mut studio = s.engine.doc.clone();
        studio.remove(NodeId(b)).unwrap();
        write_canvas(&sess, &studio);
        s.call("designos_get_document", json!({})).unwrap();
        assert_eq!(s.call("designos_create_node", make("k1", "A")).unwrap_err().code, ErrorCode::IdempotencyKeyStale);
    }
    let mut s2 = Session::with_session_dir(&ws, &sess).unwrap();
    assert_eq!(s2.call("designos_create_node", make("k1", "A")).unwrap_err().code, ErrorCode::IdempotencyKeyStale, "the undone key stays dead");
    let (_, meta) = s2.call_with_meta("designos_create_node", make("k2", "B")).unwrap();
    assert!(meta.is_none(), "the forgotten key is a new write");
    cleanup(&ws);
}

#[test]
fn a_different_document_does_not_resurrect_a_key_the_agent_undid() {
    let (ws, sess) = temp_dirs("different-document-undone-key");
    let (doc, page, _) = doc_with_frames(&[]);
    write_canvas(&sess, &doc);
    let mut s = Session::with_session_dir(&ws, &sess).unwrap();
    let make = |key: &str, name: &str| json!({"idempotency_key": key, "parent_id": page.0, "kind": "frame", "name": name});
    s.call("designos_create_node", make("k1", "A")).unwrap();
    s.call("designos_create_node", make("k2", "B")).unwrap();
    s.call("designos_undo", json!({})).unwrap();

    // Studio now shows a canvas that holds neither frame: k1 is applied and missing, k2 was undone.
    write_canvas(&sess, &doc);
    assert_eq!(s.call("designos_create_node", make("k2", "B")).unwrap_err().code, ErrorCode::IdempotencyKeyStale, "the undone write stays refused");
    let (_, meta) = s.call_with_meta("designos_create_node", make("k1", "A")).unwrap();
    assert!(meta.is_none(), "the applied entry missing from the canvas is a new write on retry");
    cleanup(&ws);
}

#[test]
fn scoped_canvas_omission_keeps_the_key_stale() {
    let (ws, sess) = temp_dirs("scoped-omission");
    let (doc, page, _) = doc_with_frames(&[]);
    write_canvas(&sess, &doc);
    let mut s = Session::with_session_dir(&ws, &sess).unwrap();
    let make = |key: &str, name: &str| json!({"idempotency_key": key, "parent_id": page.0, "kind": "frame", "name": name});
    s.call("designos_create_node", make("k1", "A")).unwrap();
    let b = s.call("designos_create_node", make("k2", "B")).unwrap()["node"]["id"].as_u64().unwrap();

    // The editor now shares a scoped copy that leaves B out; B is not undone.
    fs::write(sess.join("context.json"), json!({"current_page_id": page.0, "scoped": true}).to_string()).unwrap();
    let mut scoped = s.engine.doc.clone();
    scoped.remove(NodeId(b)).unwrap();
    write_canvas(&sess, &scoped);

    let err = s.call("designos_create_node", make("k2", "B")).unwrap_err();
    assert_eq!(err.code, ErrorCode::IdempotencyKeyStale);
    cleanup(&ws);
}

#[test]
fn a_scoped_canvas_that_leaves_out_every_applied_key_keeps_them_stale() {
    let (ws, sess) = temp_dirs("scoped-all-omitted");
    let (doc, page, _) = doc_with_frames(&[]);
    write_canvas(&sess, &doc);
    let mut s = Session::with_session_dir(&ws, &sess).unwrap();
    let make = |key: &str, name: &str| json!({"idempotency_key": key, "parent_id": page.0, "kind": "frame", "name": name});
    let a = s.call("designos_create_node", make("k1", "A")).unwrap()["node"]["id"].as_u64().unwrap();
    let b = s.call("designos_create_node", make("k2", "B")).unwrap()["node"]["id"].as_u64().unwrap();

    // A scoped copy (large file) leaves both layers out, e.g. they sit inside a frame over the layer budget.
    fs::write(sess.join("context.json"), json!({"current_page_id": page.0, "scoped": true}).to_string()).unwrap();
    let mut scoped = s.engine.doc.clone();
    scoped.remove(NodeId(a)).unwrap();
    scoped.remove(NodeId(b)).unwrap();
    write_canvas(&sess, &scoped);

    for (key, name) in [("k1", "A"), ("k2", "B")] {
        let err = s.call("designos_create_node", make(key, name)).unwrap_err();
        assert_eq!(err.code, ErrorCode::IdempotencyKeyStale, "{key} must not run again on a scoped copy");
    }
    let page_children = &s.engine.doc.get(page).unwrap().children;
    assert!(page_children.is_empty(), "no retry created a layer on the scoped copy: {page_children:?}");
    cleanup(&ws);
}

fn diff_batch(key: &str, name: &str, page: u64, x: f64) -> Value {
    json!({
        "idempotency_key": key,
        "diff_image": true,
        "ops": [{"op": "create", "parent": page, "kind": "rectangle", "name": name, "x": x, "y": 10.0, "width": 60.0, "height": 40.0, "fill": "#FF0000"}]
    })
}

#[test]
fn replayed_diff_uses_the_before_image_of_its_own_write() {
    let (ws, sess) = temp_dirs("diff-before");
    let mut s = Session::with_session_dir(&ws, &sess).unwrap();
    let page = s.engine.doc.pages[0].0;
    let r1 = s.call_tool_result("designos_apply_batch", diff_batch("k1", "First", page, 10.0));
    assert_eq!(r1.content.len(), 3);
    // A later write lands in the same region, so the top of the undo stack is no longer k1's step.
    s.call("designos_apply_batch", diff_batch("k2", "Second", page, 30.0)).unwrap();

    let replay = s.call_tool_result("designos_apply_batch", diff_batch("k1", "First", page, 10.0));
    assert_eq!(replay.content.len(), 3);
    assert_eq!(replay.content[0], r1.content[0], "the before image shows the page as it was before k1, not before k2");
    assert_ne!(replay.content[0], replay.content[1], "before and after differ");
    cleanup(&ws);
}

#[test]
fn images_rerendered_is_only_flagged_when_the_rerender_succeeded() {
    use crate::tools::ledger::{now_ms, LedgerEntry};
    let (ws, sess) = temp_dirs("rerender-flag");
    let mut s = Session::with_session_dir(&ws, &sess).unwrap();
    let page = s.engine.doc.pages[0].0;
    // A stored reply whose diff names a page the document does not have: nothing can be drawn.
    let args = json!({"diff_image": true, "ops": [{"op": "create", "parent": page, "kind": "rectangle", "name": "Ghost"}]});
    let mut hashed = args.clone();
    hashed.as_object_mut().unwrap().remove("idempotency_key");
    s.ledger.store(LedgerEntry {
        key: "ghost".into(),
        tool: "designos_apply_batch".into(),
        hash: crate::tools::ledger::payload_hash("designos_apply_batch", &hashed),
        undo_depth: Some(0),
        auto_fix_undo_depth: None,
        dead: false,
        live: Vec::new(),
        gone: Vec::new(),
        reply: json!({"created": [], "diff": {"pages": [{"page_id": 999_999, "images": ["before"], "region": {"x": 0, "y": 0, "width": 10, "height": 10}}]}}),
        pre_auto_fix_reply: None,
        created_ms: now_ms(),
    });
    let mut keyed = args;
    keyed["idempotency_key"] = json!("ghost");
    // An entry without checkable ids is stale after a reload only; in this process the depth rule decides.
    let res = s.call_tool_result("designos_apply_batch", keyed);
    assert_eq!(res.is_error, Some(false));
    let meta = res.meta.expect("a replay carries _meta");
    assert_eq!(meta.0.get("designos/idempotent_replay"), Some(&json!(true)));
    assert_eq!(meta.0.get("designos/images_rerendered"), None, "no image block was produced");
    assert!(res.content.len() <= 1, "only the structured text block, no image blocks: {:?}", res.content.len());
    cleanup(&ws);
}

#[test]
fn a_malformed_idempotency_key_gets_the_full_arguments_as_its_corrected_example() {
    let (ws, sess) = temp_dirs("key-format");
    let mut s = Session::with_session_dir(&ws, &sess).unwrap();
    s.verify_corrected_examples = true;
    let page = s.engine.doc.pages[0].0;
    let args = json!({"idempotency_key": "has space", "parent_id": page, "kind": "frame", "name": "Hero"});
    let err = s.call("designos_create_node", args).unwrap_err();
    assert_eq!(err.code, ErrorCode::InvalidArguments);
    assert_eq!(err.corrected_example, Some(json!({"idempotency_key": "run-1", "parent_id": page, "kind": "frame", "name": "Hero"})));
    cleanup(&ws);
}
