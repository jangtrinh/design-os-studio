//! Engine-level `Session::call` coverage for `LOOP_SUSPECTED` (#1290).

use super::*;
use crate::loop_guard::LoopGuard;
use crate::tools::errors::ErrorCode;
use std::time::UNIX_EPOCH;

fn temp_ws(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "designos-1290-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
    ));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn session(tag: &str) -> (PathBuf, Session) {
    let ws = temp_ws(tag);
    let s = Session::new(&ws).unwrap();
    (ws, s)
}

fn missing_update() -> Value {
    json!({"node_id": 9_876_543_210u64, "name": "loop"})
}

fn assert_loop(err: &ToolError) {
    assert_eq!(err.code, ErrorCode::LoopSuspected, "{err:?}");
    assert!(!err.safe_to_retry);
    assert!(err.next_step.contains("Change the arguments"), "{err:?}");
    assert!(err.next_step.contains("read the document"), "{err:?}");
}

#[test]
fn three_identical_failing_modifies_return_loop_suspected() {
    let (ws, mut s) = session("ac01");
    let args = missing_update();
    let e1 = s.call("designos_update_node", args.clone()).unwrap_err();
    assert_eq!(e1.code, ErrorCode::NodeNotFound);
    let e2 = s.call("designos_update_node", args.clone()).unwrap_err();
    assert_eq!(e2.code, ErrorCode::NodeNotFound);
    let e3 = s.call("designos_update_node", args).unwrap_err();
    assert_loop(&e3);
    let _ = fs::remove_dir_all(ws);
}

#[test]
fn three_reads_whose_results_differ_do_not_trigger() {
    let (ws, mut s) = session("ac02");
    for i in 0..3 {
        s.engine.doc.name = format!("Doc{i}");
        let doc = s.call("designos_get_document", json!({})).unwrap();
        assert_eq!(doc["name"], json!(format!("Doc{i}")));
    }
    let _ = fs::remove_dir_all(ws);
}

#[test]
fn a_different_argument_hash_resets_the_counter() {
    let (ws, mut s) = session("ac04");
    let a = missing_update();
    assert_eq!(s.call("designos_update_node", a.clone()).unwrap_err().code, ErrorCode::NodeNotFound);
    assert_eq!(s.call("designos_update_node", a.clone()).unwrap_err().code, ErrorCode::NodeNotFound);
    s.call("designos_get_document", json!({})).unwrap();
    assert_eq!(s.call("designos_update_node", a.clone()).unwrap_err().code, ErrorCode::NodeNotFound);
    assert_eq!(s.call("designos_update_node", a.clone()).unwrap_err().code, ErrorCode::NodeNotFound);
    assert_loop(&s.call("designos_update_node", a).unwrap_err());
    let _ = fs::remove_dir_all(ws);
}

#[test]
fn idempotent_replays_do_not_count_as_a_loop() {
    let (ws, mut s) = session("ac06");
    let page = s.engine.doc.pages[0].0;
    let payload = json!({"parent_id": page, "kind": "frame", "name": "Once", "idempotency_key": "k-loop"});
    s.call("designos_create_node", payload.clone()).unwrap();
    for _ in 0..4 {
        let (reply, meta) = s.call_with_meta("designos_create_node", payload.clone()).unwrap();
        assert_eq!(meta.unwrap().get("designos/idempotent_replay"), Some(&json!(true)));
        assert!(reply.get("error").is_none(), "{reply}");
    }
    let args = missing_update();
    assert_eq!(s.call("designos_update_node", args.clone()).unwrap_err().code, ErrorCode::NodeNotFound);
    assert_eq!(s.call("designos_update_node", args.clone()).unwrap_err().code, ErrorCode::NodeNotFound);
    assert_loop(&s.call("designos_update_node", args).unwrap_err());
    let _ = fs::remove_dir_all(ws);
}

#[test]
fn sixth_identical_failing_call_still_returns_loop_suspected() {
    let (ws, mut s) = session("ac09");
    let args = missing_update();
    for i in 1..=6 {
        let err = s.call("designos_update_node", args.clone()).unwrap_err();
        if i < 3 {
            assert_eq!(err.code, ErrorCode::NodeNotFound, "call {i}");
        } else {
            assert_loop(&err);
        }
    }
    s.call("designos_get_document", json!({})).unwrap();
    assert_eq!(s.call("designos_update_node", args.clone()).unwrap_err().code, ErrorCode::NodeNotFound);
    assert_eq!(s.call("designos_update_node", args.clone()).unwrap_err().code, ErrorCode::NodeNotFound);
    assert_loop(&s.call("designos_update_node", args).unwrap_err());
    let _ = fs::remove_dir_all(ws);
}

#[test]
fn a_document_revision_change_resets_the_window() {
    let (ws, mut s) = session("ac10");
    let args = missing_update();
    assert_eq!(s.call("designos_update_node", args.clone()).unwrap_err().code, ErrorCode::NodeNotFound);
    assert_eq!(s.call("designos_update_node", args.clone()).unwrap_err().code, ErrorCode::NodeNotFound);
    s.engine.revision = s.engine.revision.wrapping_add(1);
    assert_eq!(s.call("designos_update_node", args.clone()).unwrap_err().code, ErrorCode::NodeNotFound);
    assert_eq!(s.call("designos_update_node", args.clone()).unwrap_err().code, ErrorCode::NodeNotFound);
    assert_loop(&s.call("designos_update_node", args).unwrap_err());
    let _ = fs::remove_dir_all(ws);
}

#[test]
fn ping_pong_of_two_reads_with_unchanged_results_fires() {
    let (ws, mut s) = session("ac03");
    s.loop_guard = LoopGuard::from_overrides(Some("2"), None);
    let a = json!({"node_id": s.engine.doc.pages[0].0});
    let b = json!({"limit": 1});
    s.call("designos_get_node", a.clone()).unwrap();
    s.call("designos_list_nodes", b.clone()).unwrap();
    s.call("designos_get_node", a).unwrap();
    assert_loop(&s.call("designos_list_nodes", b).unwrap_err());
    let _ = fs::remove_dir_all(ws);
}
