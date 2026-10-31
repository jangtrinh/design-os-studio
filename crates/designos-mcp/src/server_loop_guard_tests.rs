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

fn assert_loop_warning(value: &Value, k: usize) {
    let err = crate::loop_guard::loop_suspected_error(k);
    assert_eq!(
        value.get("warning"),
        Some(&json!({
            "code": "LOOP_SUSPECTED",
            "message": err.message,
            "next_step": err.next_step,
            "safe_to_retry": false,
        })),
        "{value}"
    );
}

fn strip_warning(value: &Value) -> Value {
    let mut stripped = value.clone();
    if let Some(obj) = stripped.as_object_mut() {
        obj.remove("warning");
    }
    stripped
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
fn ping_pong_of_two_reads_with_unchanged_results_is_advisory() {
    let (ws, mut s) = session("ac03");
    s.loop_guard = LoopGuard::from_overrides(Some("2"), None);
    let a = json!({"node_id": s.engine.doc.pages[0].0});
    let b = json!({"limit": 1});
    s.call("designos_get_node", a.clone()).unwrap();
    let first_list = s.call("designos_list_nodes", b.clone()).unwrap();
    assert!(first_list.get("warning").is_none(), "{first_list}");
    s.call("designos_get_node", a).unwrap();
    let warned = s.call("designos_list_nodes", b).unwrap();
    assert_loop_warning(&warned, s.loop_guard.k());
    assert_eq!(strip_warning(&warned), first_list);
    let _ = fs::remove_dir_all(ws);
}

#[test]
fn ping_pong_ending_on_a_write_is_a_hard_error() {
    let (ws, mut s) = session("pp-write");
    s.loop_guard = LoopGuard::from_overrides(Some("2"), None);
    let read = json!({});
    let write = json!({});
    s.call("designos_get_document", read.clone()).unwrap();
    s.call("designos_undo", write.clone()).unwrap();
    s.call("designos_get_document", read).unwrap();
    assert_loop(&s.call("designos_undo", write).unwrap_err());
    let _ = fs::remove_dir_all(ws);
}

#[test]
fn three_identical_reads_return_ok_with_loop_warning() {
    let (ws, mut s) = session("adv-read");
    let args = json!({});
    let first = s.call("designos_get_document", args.clone()).unwrap();
    assert!(first.get("warning").is_none(), "{first}");
    let second = s.call("designos_get_document", args.clone()).unwrap();
    assert!(second.get("warning").is_none(), "{second}");
    let third = s.call("designos_get_document", args.clone()).unwrap();
    assert_loop_warning(&third, s.loop_guard.k());
    assert_eq!(strip_warning(&third), first);
    let fourth = s.call("designos_get_document", args).unwrap();
    assert_loop_warning(&fourth, s.loop_guard.k());
    assert_eq!(strip_warning(&fourth), first);
    let _ = fs::remove_dir_all(ws);
}

#[test]
fn three_identical_empty_undos_return_loop_suspected() {
    let (ws, mut s) = session("adv-undo");
    let args = json!({});
    let first = s.call("designos_undo", args.clone()).unwrap();
    assert_eq!(first["changed"], json!(false));
    assert!(first.get("warning").is_none(), "{first}");
    let second = s.call("designos_undo", args.clone()).unwrap();
    assert_eq!(second["changed"], json!(false));
    assert_loop(&s.call("designos_undo", args).unwrap_err());
    let _ = fs::remove_dir_all(ws);
}

#[test]
fn a_revision_change_clears_the_advisory() {
    let (ws, mut s) = session("adv-rev");
    let args = json!({});
    s.call("designos_get_document", args.clone()).unwrap();
    s.call("designos_get_document", args.clone()).unwrap();
    assert_loop_warning(&s.call("designos_get_document", args.clone()).unwrap(), s.loop_guard.k());
    s.engine.revision = s.engine.revision.wrapping_add(1);
    let after = s.call("designos_get_document", args).unwrap();
    assert!(after.get("warning").is_none(), "{after}");
    let _ = fs::remove_dir_all(ws);
}

#[test]
fn a_different_call_clears_the_advisory() {
    let (ws, mut s) = session("adv-diff");
    let args = json!({});
    s.call("designos_get_document", args.clone()).unwrap();
    s.call("designos_get_document", args.clone()).unwrap();
    assert_loop_warning(&s.call("designos_get_document", args.clone()).unwrap(), s.loop_guard.k());
    s.call("designos_list_nodes", json!({"limit": 1})).unwrap();
    let after = s.call("designos_get_document", args).unwrap();
    assert!(after.get("warning").is_none(), "{after}");
    let _ = fs::remove_dir_all(ws);
}

#[test]
fn three_identical_failing_reads_return_loop_suspected() {
    let (ws, mut s) = session("fail-read");
    let args = json!({"node_id": 9_876_543_210u64});
    let e1 = s.call("designos_get_node", args.clone()).unwrap_err();
    assert_eq!(e1.code, ErrorCode::NodeNotFound);
    let e2 = s.call("designos_get_node", args.clone()).unwrap_err();
    assert_eq!(e2.code, ErrorCode::NodeNotFound);
    let e3 = s.call("designos_get_node", args).unwrap_err();
    assert_loop(&e3);
    let _ = fs::remove_dir_all(ws);
}

#[test]
fn successful_read_loop_warning_is_a_named_object() {
    let (ws, mut s) = session("warn-obj");
    let args = json!({});
    s.call("designos_get_document", args.clone()).unwrap();
    s.call("designos_get_document", args.clone()).unwrap();
    let third = s.call("designos_get_document", args).unwrap();
    let err = crate::loop_guard::loop_suspected_error(s.loop_guard.k());
    assert_eq!(
        third.get("warning"),
        Some(&json!({
            "code": "LOOP_SUSPECTED",
            "message": err.message,
            "next_step": err.next_step,
            "safe_to_retry": false,
        })),
        "{third}"
    );
    let _ = fs::remove_dir_all(ws);
}

#[test]
fn identical_noop_writes_fire_on_the_kth_identical_result_after_the_first_mutating_call() {
    let (ws, mut s) = session("noop-write");
    let page = s.engine.doc.pages[0].0;
    let created = s.call("designos_create_node", json!({"parent_id": page, "kind": "frame", "name": "Original"})).unwrap();
    let node_id = created["node"]["id"].as_u64().unwrap();
    let args = json!({"node_id": node_id, "name": "Same"});
    let first = s.call("designos_update_node", args.clone()).unwrap();
    assert_eq!(first["changed"], json!(true), "{first}");
    assert!(first.get("warning").is_none(), "{first}");
    let second = s.call("designos_update_node", args.clone()).unwrap();
    assert_eq!(second["changed"], json!(false), "{second}");
    assert!(second.get("warning").is_none(), "{second}");
    let third = s.call("designos_update_node", args.clone()).unwrap();
    assert_eq!(third["changed"], json!(false), "{third}");
    assert!(third.get("warning").is_none(), "{third}");
    assert_loop(&s.call("designos_update_node", args).unwrap_err());
    let _ = fs::remove_dir_all(ws);
}
