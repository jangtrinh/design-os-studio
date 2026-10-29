//! Policy matrix for `designos_save_file` in-place save and `designos_close_document`.
//!
//! Dirty and clean × save / discard / fail / missing / invalid × titled / untitled × path / no path.

use designos_doc::live_patch::{CloseIntent, LiveLogLine};
use designos_mcp::server::Session;
use serde_json::{json, Value};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::{Duration, Instant},
};

static SEQ: AtomicU64 = AtomicU64::new(0);

struct Workspace(PathBuf);
impl Workspace {
    fn new() -> Self {
        let nonce = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let seq = SEQ.fetch_add(1, Ordering::Relaxed);
        let p = std::env::temp_dir().join(format!("designos-mcp-close-{}-{nonce}-{seq}", std::process::id()));
        fs::create_dir_all(&p).unwrap();
        Self(p)
    }
}
impl Drop for Workspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn page_id(s: &mut Session) -> u64 {
    s.call("designos_get_document", json!({})).unwrap()["pages"][0]["node_id"].as_u64().unwrap()
}

fn node_count(s: &mut Session) -> usize {
    s.call("designos_get_document", json!({})).unwrap()["node_count"].as_u64().unwrap() as usize
}

fn make_dirty(s: &mut Session) {
    let page = page_id(s);
    s.call("designos_create_node", json!({"parent_id": page, "kind": "frame", "name": "Edit"})).unwrap();
}

fn titled_dirty(dir: &Workspace) -> Session {
    let mut s = Session::new(&dir.0).unwrap();
    s.call("designos_save_file", json!({"path": "design.designos"})).unwrap();
    make_dirty(&mut s);
    s
}

fn untitled_dirty() -> (Workspace, Session) {
    let dir = Workspace::new();
    let mut s = Session::new(&dir.0).unwrap();
    s.call("designos_new_document", json!({"name": "Untitled", "discard_changes": true})).unwrap();
    make_dirty(&mut s);
    (dir, s)
}

fn err_json(s: &mut Session, args: Value) -> Value {
    s.call("designos_close_document", args).unwrap_err().to_json_value()
}

fn disk_bytes(dir: &Workspace, name: &str) -> Vec<u8> {
    fs::read(dir.0.join(name)).unwrap()
}

/// AC-03 / in-place save: a titled document saves without a path; an untitled one returns PATH_REQUIRED.
#[test]
fn save_file_in_place_or_path_required() {
    let dir = Workspace::new();
    let mut s = Session::new(&dir.0).unwrap();
    s.call("designos_save_file", json!({"path": "design.designos"})).unwrap();
    make_dirty(&mut s);
    let saved = s.call("designos_save_file", json!({})).expect("titled in-place save");
    assert_eq!(saved["saved"], true, "{saved}");
    assert!(saved["path"].as_str().unwrap().ends_with("design.designos"), "{saved}");

    let (u, mut untitled) = untitled_dirty();
    let err = untitled.call("designos_save_file", json!({})).unwrap_err().to_json_value();
    assert_eq!(err["code"], "PATH_REQUIRED", "{err}");
    assert_eq!(err["safe_to_retry"], true, "{err}");
    let _ = u;
    let _ = dir;
}

/// AC-02: fail on a dirty doc returns UNSAVED_CHANGES, stays open, does not suggest discard.
#[test]
fn close_fail_on_dirty_is_unsaved_changes_and_does_not_suggest_discard() {
    let dir = Workspace::new();
    let mut s = titled_dirty(&dir);
    let before = node_count(&mut s);
    let on_disk = disk_bytes(&dir, "design.designos");
    for args in [json!({"unsaved": "fail"}), json!({})] {
        let err = err_json(&mut s, args);
        assert_eq!(err["code"], "UNSAVED_CHANGES", "{err}");
        assert_eq!(err["safe_to_retry"], false, "{err}");
        assert!(err.get("error_kind").is_none(), "error_kind is #1291, not this issue: {err}");
        let blob = err.to_string();
        assert!(!blob.contains("discard"), "must not suggest discard: {err}");
        assert!(err.get("corrected_example").is_none() || err["corrected_example"].is_null(), "no corrected_example: {err}");
        assert_eq!(node_count(&mut s), before, "doc stays open");
        assert_eq!(disk_bytes(&dir, "design.designos"), on_disk, "disk unchanged");
    }
}

/// AC-03: untitled + save + no path is PATH_REQUIRED; with a path it saves there.
#[test]
fn close_save_untitled_needs_a_path() {
    let (dir, mut s) = untitled_dirty();
    let before = node_count(&mut s);
    let err = err_json(&mut s, json!({"unsaved": "save"}));
    assert_eq!(err["code"], "PATH_REQUIRED", "{err}");
    assert_eq!(err["safe_to_retry"], true, "{err}");
    assert_eq!(node_count(&mut s), before, "nothing closed");

    let out = s.call("designos_close_document", json!({"unsaved": "save", "path": "named.designos"})).unwrap();
    assert_eq!(out["unsaved"], "save", "{out}");
    assert_eq!(out["saved"], true, "{out}");
    assert_eq!(out["closed"], true, "{out}");
    assert!(dir.0.join("named.designos").is_file(), "file written");
    assert!(s.call("designos_new_document", json!({"name": "Next"})).is_ok(), "session is clean after close");
}

/// AC-01 / titled save: file is saved, session closes, no dialog (headless: no asking state).
#[test]
fn close_save_titled_writes_and_closes() {
    let dir = Workspace::new();
    let mut s = titled_dirty(&dir);
    let edited = node_count(&mut s);
    let out = s.call("designos_close_document", json!({"unsaved": "save"})).unwrap();
    assert_eq!(out["unsaved"], "save", "{out}");
    assert_eq!(out["saved"], true, "{out}");
    assert_eq!(out["closed"], true, "{out}");
    let saved = designos_doc::Document::from_json(&fs::read_to_string(dir.0.join("design.designos")).unwrap()).unwrap();
    assert_eq!(saved.nodes.len(), edited, "in-window edits are on disk");
    assert!(s.call("designos_new_document", json!({"name": "Next"})).is_ok());
}

/// AC-06: discard closes, drops edits, disk unchanged, result reports discard.
#[test]
fn close_discard_drops_edits_and_leaves_disk() {
    let dir = Workspace::new();
    let mut s = titled_dirty(&dir);
    let on_disk = disk_bytes(&dir, "design.designos");
    let out = s.call("designos_close_document", json!({"unsaved": "discard"})).unwrap();
    assert_eq!(out["unsaved"], "discard", "{out}");
    assert_eq!(out["closed"], true, "{out}");
    assert_eq!(out["saved"], false, "{out}");
    assert_eq!(disk_bytes(&dir, "design.designos"), on_disk, "file on disk unchanged");
    assert!(s.call("designos_new_document", json!({"name": "Next"})).is_ok(), "doc closed");
}

/// AC-07: invalid unsaved is INVALID_ARGUMENTS retryable and changes nothing. Omitted equals fail.
#[test]
fn close_invalid_unsaved_is_retryable_and_changes_nothing() {
    let dir = Workspace::new();
    let mut s = titled_dirty(&dir);
    let before = node_count(&mut s);
    let on_disk = disk_bytes(&dir, "design.designos");
    let err = err_json(&mut s, json!({"unsaved": "yeet"}));
    assert_eq!(err["code"], "INVALID_ARGUMENTS", "{err}");
    assert_eq!(err["safe_to_retry"], true, "{err}");
    assert_eq!(node_count(&mut s), before);
    assert_eq!(disk_bytes(&dir, "design.designos"), on_disk);
}

/// AC-08: a path that cannot be written (a directory) returns a named error, no partial write, doc stays dirty.
#[test]
fn close_save_to_a_directory_fails_named_and_keeps_the_doc() {
    let dir = Workspace::new();
    let mut s = titled_dirty(&dir);
    let before = node_count(&mut s);
    fs::create_dir(dir.0.join("locked.designos")).unwrap();
    let err = err_json(&mut s, json!({"unsaved": "save", "path": "locked.designos"}));
    assert_ne!(err["code"], "UNSAVED_CHANGES", "{err}");
    assert!(err["code"] == "INVALID_ARGUMENTS" || err["code"] == "INTERNAL_ERROR" || err["code"] == "FILE_EXISTS", "named error, got {err}");
    assert_eq!(node_count(&mut s), before, "doc stays open");
    assert!(dir.0.join("locked.designos").is_dir(), "no partial write over the directory");
    let fail = err_json(&mut s, json!({"unsaved": "fail"}));
    assert_eq!(fail["code"], "UNSAVED_CHANGES", "still dirty: {fail}");
}

/// AC-09: a clean document closes with any unsaved value.
#[test]
fn close_clean_succeeds_with_any_unsaved_value() {
    let dir = Workspace::new();
    for unsaved in [json!({"unsaved": "save"}), json!({"unsaved": "discard"}), json!({"unsaved": "fail"}), json!({})] {
        let mut s = Session::new(&dir.0).unwrap();
        s.call("designos_save_file", json!({"path": "clean.designos", "overwrite": true})).unwrap();
        let out = s.call("designos_close_document", unsaved.clone()).unwrap();
        assert_eq!(out["closed"], true, "{unsaved} {out}");
    }
}

/// Policy matrix remainder: discard on untitled, save with path on titled, fail on clean already covered.
#[test]
fn close_discard_untitled_closes_without_writing() {
    let (dir, mut s) = untitled_dirty();
    let out = s.call("designos_close_document", json!({"unsaved": "discard"})).unwrap();
    assert_eq!(out["unsaved"], "discard", "{out}");
    assert_eq!(out["closed"], true, "{out}");
    let entries: Vec<_> = fs::read_dir(&dir.0).unwrap().filter_map(|e| e.ok()).collect();
    assert!(!entries.iter().any(|e| e.path().extension().is_some_and(|x| x == "designos")), "discard of untitled wrote a file: {entries:?}");
}

fn live_session(dir: &Workspace, tab_id: u64) -> (PathBuf, Session) {
    let session_dir = dir.0.join(format!("win-tab-{tab_id}"));
    fs::create_dir_all(&session_dir).unwrap();
    let mut s = Session::with_session_dir(&dir.0, &session_dir).unwrap();
    s.set_close_ack_timeout(Duration::from_millis(80));
    (session_dir, s)
}

fn last_close_line(session_dir: &Path) -> CloseIntent {
    let raw = fs::read_to_string(session_dir.join("doc-patch.jsonl")).expect("close line written");
    let line = raw.lines().rev().find(|l| !l.trim().is_empty()).expect("a log line");
    match LiveLogLine::parse(line.as_bytes()).expect("parse close line") {
        LiveLogLine::Close(intent) => intent,
        other => panic!("expected close, got {other:?}"),
    }
}

#[test]
fn save_file_reports_doc_revision() {
    let dir = Workspace::new();
    let mut s = Session::new(&dir.0).unwrap();
    let out = s.call("designos_save_file", json!({"path": "design.designos"})).unwrap();
    assert_eq!(out["saved"], true, "{out}");
    assert!(out["doc_revision"].as_u64().is_some(), "save result must name doc_revision: {out}");
}

#[test]
fn close_save_reports_doc_revision() {
    let dir = Workspace::new();
    let mut s = titled_dirty(&dir);
    let out = s.call("designos_close_document", json!({"unsaved": "save"})).unwrap();
    assert_eq!(out["closed"], true, "{out}");
    assert_eq!(out["saved"], true, "{out}");
    assert!(out["doc_revision"].as_u64().is_some(), "close-save result must name doc_revision: {out}");
}

#[test]
fn live_close_without_editor_ack_is_not_closed() {
    let dir = Workspace::new();
    let (session_dir, mut s) = live_session(&dir, 7);
    s.call("designos_save_file", json!({"path": "design.designos"})).unwrap();
    let err = s.call("designos_close_document", json!({"unsaved": "discard"})).unwrap_err().to_json_value();
    assert_ne!(err["code"], json!("UNSAVED_CHANGES"), "timeout is not unsaved: {err}");
    assert_eq!(err["safe_to_retry"], false, "{err}");
    assert!(err.get("closed").is_none() || err["closed"] != true, "must not claim closed: {err}");
    let cancel: Value = serde_json::from_slice(&fs::read(session_dir.join("close-cancel.json")).expect("cancel written")).unwrap();
    assert!(cancel["close_id"].as_u64().is_some(), "{cancel}");
    let intent = last_close_line(&session_dir);
    assert_eq!(intent.tab_id, Some(7), "intent names the tab: {intent:?}");
    assert_eq!(intent.close_id, cancel["close_id"].as_u64());
}

#[test]
fn live_close_with_editor_ack_is_closed() {
    let dir = Workspace::new();
    let (session_dir, mut s) = live_session(&dir, 3);
    s.call("designos_save_file", json!({"path": "design.designos"})).unwrap();
    let session_dir_for_ack = session_dir.clone();
    thread::scope(|scope| {
        scope.spawn(move || {
            let deadline = Instant::now() + Duration::from_millis(500);
            loop {
                if let Ok(raw) = fs::read_to_string(session_dir_for_ack.join("doc-patch.jsonl")) {
                    if let Some(line) = raw.lines().rev().find(|l| l.contains("\"close\":true")) {
                        if let Ok(LiveLogLine::Close(intent)) = LiveLogLine::parse(line.as_bytes()) {
                            if let Some(close_id) = intent.close_id {
                                let ack = json!({
                                    "close_id": close_id,
                                    "ok": true,
                                    "closed": true,
                                    "saved": false,
                                    "unsaved": "discard",
                                    "doc_revision": 99u64,
                                    "tab_id": 3u64,
                                });
                                fs::write(session_dir_for_ack.join("close-result.json"), ack.to_string()).unwrap();
                                return;
                            }
                        }
                    }
                }
                if Instant::now() >= deadline {
                    panic!("close line with close_id never appeared");
                }
                thread::sleep(Duration::from_millis(5));
            }
        });
        let out = s.call("designos_close_document", json!({"unsaved": "discard"})).expect("acked close");
        assert_eq!(out["closed"], true, "{out}");
        assert_eq!(out.get("queued").and_then(Value::as_bool), None, "acked close is not queued: {out}");
        assert_eq!(out["doc_revision"], 99, "revision comes from the editor ack: {out}");
        assert_eq!(out["tab_id"], 3, "{out}");
        let intent = last_close_line(&session_dir);
        assert_eq!(intent.tab_id, Some(3));
        assert!(intent.close_id.is_some(), "{intent:?}");
    });
}

#[test]
fn live_close_fail_ack_is_unsaved_changes() {
    let dir = Workspace::new();
    let (session_dir, mut s) = live_session(&dir, 4);
    let session_dir_for_ack = session_dir.clone();
    thread::scope(|scope| {
        scope.spawn(move || {
            let deadline = Instant::now() + Duration::from_millis(500);
            loop {
                if let Ok(raw) = fs::read_to_string(session_dir_for_ack.join("doc-patch.jsonl")) {
                    if let Some(line) = raw.lines().rev().find(|l| l.contains("\"close\":true")) {
                        if let Ok(LiveLogLine::Close(intent)) = LiveLogLine::parse(line.as_bytes()) {
                            if let Some(close_id) = intent.close_id {
                                let ack = json!({
                                    "close_id": close_id,
                                    "ok": false,
                                    "code": "UNSAVED_CHANGES",
                                    "message": "Unsaved changes. The document stayed open.",
                                });
                                fs::write(session_dir_for_ack.join("close-result.json"), ack.to_string()).unwrap();
                                return;
                            }
                        }
                    }
                }
                if Instant::now() >= deadline {
                    panic!("close line with close_id never appeared");
                }
                thread::sleep(Duration::from_millis(5));
            }
        });
        let err = s.call("designos_close_document", json!({})).unwrap_err().to_json_value();
        assert_eq!(err["code"], "UNSAVED_CHANGES", "{err}");
        assert_eq!(err["safe_to_retry"], false, "{err}");
        assert!(err.get("closed").is_none() || err["closed"] != true, "{err}");
        assert!(s.call("designos_get_document", json!({})).is_ok(), "session stays open after fail");
    });
}
