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
    assert_eq!(err["safe_to_retry"], false, "{err}");
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
        assert_eq!(err["error_kind"], "needs_human", "{err}");
        let blob = err.to_string();
        // #1291's next_step says "Do not discard them", so the word is allowed; the discard_changes argument is not.
        assert!(!blob.contains("discard_changes"), "must not suggest the discard_changes argument: {err}");
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
    assert_eq!(err["safe_to_retry"], false, "{err}");
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
    assert_eq!(err["code"], json!("EDITOR_NOT_NOTIFIED"), "{err}");
    assert_eq!(err["safe_to_retry"], false, "a discard the editor may have begun is not retried blindly: {err}");
    assert_eq!(err["next_step"], json!("Check the document state with designos_get_document, then ask the user."), "{err}");
    assert!(err.get("closed").is_none() || err["closed"] != true, "must not claim closed: {err}");
    let cancel: Value = serde_json::from_slice(&fs::read(session_dir.join("close-cancel.json")).expect("cancel written")).unwrap();
    assert!(cancel["close_id"].as_u64().is_some(), "{cancel}");
    let intent = last_close_line(&session_dir);
    assert_eq!(intent.tab_id, Some(7), "intent names the tab: {intent:?}");
    assert_eq!(intent.close_id, cancel["close_id"].as_u64());
}

/// A close the editor never answers: `save` and `fail` are cancelled and safe to ask again; `discard` is not.
#[test]
fn unanswered_live_close_is_retryable_for_save_and_fail_but_not_discard() {
    for (policy, retry) in [("save", true), ("fail", true), ("discard", false)] {
        let dir = Workspace::new();
        let (session_dir, mut s) = live_session(&dir, 6);
        let err = s.call("designos_close_document", json!({"unsaved": policy})).unwrap_err().to_json_value();
        assert_eq!(err["code"], json!("EDITOR_NOT_NOTIFIED"), "{policy}: {err}");
        assert_eq!(err["safe_to_retry"], retry, "{policy}: {err}");
        let next = err["next_step"].as_str().unwrap_or("");
        assert_eq!(next.contains("call designos_close_document again"), retry, "{policy}: {err}");
        assert_eq!(next.contains("ask the user"), !retry, "{policy}: {err}");
        assert!(session_dir.join("close-cancel.json").exists(), "{policy}: the close is cancelled either way");
    }
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

#[test]
fn live_close_fail_ack_forwards_a_known_code() {
    let dir = Workspace::new();
    let (session_dir, mut s) = live_session(&dir, 5);
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
                                    "ok": false,
                                    "code": "INVALID_ARGUMENTS",
                                    "message": "locked.designos is read-only. Make it writable first, or choose a new path.",
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
        let err = s.call("designos_close_document", json!({"unsaved": "save"})).unwrap_err().to_json_value();
        assert_eq!(err["code"], "INVALID_ARGUMENTS", "{err}");
        assert_eq!(err["message"], "locked.designos is read-only. Make it writable first, or choose a new path.", "{err}");
        assert_eq!(err["safe_to_retry"], false, "{err}");
        assert!(s.call("designos_get_document", json!({})).is_ok(), "session stays open after fail");
    });
}

/// A file with no path of its own (a library file) closed with save while the editor is not answering: the
/// document is not "untitled", so the answer is that the editor is unavailable, and asking again is safe.
#[test]
fn live_close_save_with_an_unreachable_editor_is_retryable_not_path_required() {
    let dir = Workspace::new();
    let (session_dir, mut s) = live_session(&dir, 3);
    let err = s.call("designos_close_document", json!({"unsaved": "save"})).unwrap_err().to_json_value();
    assert_ne!(err["code"], json!("PATH_REQUIRED"), "{err}");
    assert_eq!(err["code"], json!("EDITOR_NOT_NOTIFIED"), "{err}");
    assert_eq!(err["safe_to_retry"], true, "{err}");
    let next = err["next_step"].as_str().unwrap_or("");
    assert!(next.contains("designos_close_document again") && !next.contains("valid_options"), "{err}");
    assert!(session_dir.join("close-cancel.json").exists(), "the close is cancelled so the editor cannot act on it later");
}

/// The editor's log cannot be written to (something that is not a file is in its place): the same answer, and the document stays open.
#[test]
fn live_close_save_that_cannot_reach_the_editor_log_is_retryable() {
    let dir = Workspace::new();
    let (session_dir, mut s) = live_session(&dir, 4);
    fs::create_dir(session_dir.join("doc-patch.jsonl")).unwrap();
    let err = s.call("designos_close_document", json!({"unsaved": "save"})).unwrap_err().to_json_value();
    assert_ne!(err["code"], json!("PATH_REQUIRED"), "{err}");
    assert_eq!(err["code"], json!("EDITOR_NOT_NOTIFIED"), "{err}");
    assert_eq!(err["safe_to_retry"], true, "{err}");
    assert!(err["message"].as_str().unwrap_or("").contains("could not be told"), "{err}");
}

/// The tab's folder is gone and holds no `tab-closed.json`: a closed scratch tab leaves that marker, so this folder was removed by hand
/// or never existed. The editor cannot be asked and asking again can never work, so the answer is its own error, at once and not after
/// the wait, and it does not claim the tab is closed.
#[test]
fn live_close_when_the_tab_folder_is_gone_is_its_own_non_retryable_error() {
    let dir = Workspace::new();
    let (session_dir, mut s) = live_session(&dir, 6);
    s.set_close_ack_timeout(Duration::from_secs(10));
    fs::remove_dir_all(&session_dir).unwrap();
    let started = std::time::Instant::now();
    let err = s.call("designos_close_document", json!({"unsaved": "save"})).unwrap_err().to_json_value();
    assert!(started.elapsed() < Duration::from_secs(2), "no wait for an editor that cannot be asked");
    assert_eq!(err["code"], json!("EDITOR_NOT_NOTIFIED"), "{err}");
    assert_eq!(err["safe_to_retry"], false, "asking again can never work: {err}");
    assert_eq!(err["message"], json!("The session folder is gone, so the editor cannot be asked."), "{err}");
    let next = err["next_step"].as_str().unwrap_or("");
    assert!(next.contains("designos_get_document") && !next.contains("already closed"), "{err}");
    assert!(!next.contains("designos_close_document"), "a retry of the close can never work and would only trip the loop guard: {err}");
    assert!(!session_dir.exists(), "the close did not bring the folder back");
}

/// A folder that is still there but holds the editor's `tab-closed.json` (a library file's folder outlives its tab) says the same.
#[test]
fn live_close_when_the_tab_left_its_mark_is_tab_closed_not_retryable() {
    let dir = Workspace::new();
    let (session_dir, mut s) = live_session(&dir, 7);
    s.set_close_ack_timeout(Duration::from_secs(10));
    fs::write(session_dir.join(CloseIntent::TAB_CLOSED_FILE), b"{\"tab_closed\":true}\n").unwrap();
    let started = std::time::Instant::now();
    let err = s.call("designos_close_document", json!({"unsaved": "discard"})).unwrap_err().to_json_value();
    assert!(started.elapsed() < Duration::from_secs(2), "no wait for an editor that is gone");
    assert_already_closed(&err, "close");
    assert_eq!(err["message"], json!("The tab is already closed, so there is nothing to close."), "{err}");
    assert!(!session_dir.join("doc-patch.jsonl").exists(), "nothing was written to a log nobody reads");
}

fn files_in(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir).map(|rd| rd.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect()).unwrap_or_default();
    names.sort();
    names
}

/// A session folder a closed tab left behind: the owner marker and the mark of the closed tab.
fn closed_tab_folder(dir: &Workspace, tab_id: u64) -> (PathBuf, Session) {
    let (session_dir, s) = live_session(dir, tab_id);
    fs::write(session_dir.join("owner.json"), b"{}").unwrap();
    fs::write(session_dir.join(CloseIntent::TAB_CLOSED_FILE), b"{\"tab_closed\":true}\n").unwrap();
    (session_dir, s)
}

/// What every call gets once the tab is closed: not retryable, and a next step that stops the agent and sends it to the user, never
/// to another call (each of them is refused the same way).
fn assert_already_closed(err: &Value, what: &str) {
    assert_eq!(err["code"], json!("EDITOR_NOT_NOTIFIED"), "{what}: {err}");
    assert_eq!(err["safe_to_retry"], false, "{what}: asking again can never work: {err}");
    let next = err["next_step"].as_str().unwrap_or("");
    assert!(next.contains("already closed"), "{what}: {err}");
    assert!(next.contains("tell the user") && !next.contains("designos_"), "{what}: the next step stops the agent and names no call: {err}");
}

/// [`assert_already_closed`] for a refused edit, open or new document: its message is about the change, not about a close.
fn assert_write_refused_as_closed(err: &Value, what: &str) {
    assert_already_closed(err, what);
    let message = err["message"].as_str().unwrap_or("");
    assert!(message.contains("already closed") && !message.contains("nothing to close"), "{what}: {err}");
}

/// The tab is closed and its folder stays: an edit would be appended to a log nobody reads and answered "ok". It is refused as
/// "already closed", nothing is written into the folder, and the agent's document is not changed.
#[test]
fn an_edit_after_the_tab_left_its_mark_is_refused_and_writes_nothing() {
    let dir = Workspace::new();
    let (session_dir, mut s) = closed_tab_folder(&dir, 8);
    let page = page_id(&mut s);
    let nodes = node_count(&mut s);
    let before = files_in(&session_dir);
    for (process, mut session) in [("the same process", s), ("a new process", Session::with_session_dir(&dir.0, &session_dir).unwrap())] {
        let err = session.call("designos_create_node", json!({"parent_id": page, "kind": "frame", "name": "Late"})).unwrap_err().to_json_value();
        assert_write_refused_as_closed(&err, process);
        assert_eq!(node_count(&mut session), nodes, "{process}: the edit was not kept");
        assert_eq!(files_in(&session_dir), before, "{process}: nothing was written into the folder");
    }
}

/// A new tab of the same file clears the mark on the same folder. A session that saw its tab closed stays refused and writes nothing
/// there; a new session on that folder is the new tab's and works, since nothing in the folder tells it of the earlier tab.
#[test]
fn a_session_that_saw_its_tab_closed_stays_refused_after_the_mark_is_cleared() {
    let dir = Workspace::new();
    let (session_dir, mut s) = closed_tab_folder(&dir, 22);
    let page = page_id(&mut s);
    let edit = json!({"parent_id": page, "kind": "frame", "name": "Late"});
    let err = s.call("designos_create_node", edit.clone()).unwrap_err().to_json_value();
    assert_write_refused_as_closed(&err, "while the mark is there");
    fs::remove_file(session_dir.join(CloseIntent::TAB_CLOSED_FILE)).unwrap();
    let before = files_in(&session_dir);
    let err = s.call("designos_create_node", edit.clone()).map_err(|e| e.to_json_value()).expect_err("still refused");
    assert_write_refused_as_closed(&err, "after the mark is cleared");
    assert_eq!(files_in(&session_dir), before, "nothing was written into the folder");
    let mut fresh = Session::with_session_dir(&dir.0, &session_dir).unwrap();
    fresh.call("designos_create_node", edit).expect("a new session on the folder works");
}

/// Opening or creating a document writes the whole document into the folder: refused the same way, and the open document stays.
#[test]
fn opening_or_creating_a_document_after_the_tab_left_its_mark_is_refused_and_writes_nothing() {
    let dir = Workspace::new();
    let (session_dir, mut s) = closed_tab_folder(&dir, 9);
    s.call("designos_save_file", json!({"path": "other.designos"})).unwrap();
    let before = files_in(&session_dir);
    let err = s.call("designos_open_file", json!({"path": "other.designos"})).unwrap_err().to_json_value();
    assert_write_refused_as_closed(&err, "open");
    let err = s.call("designos_new_document", json!({"name": "Late", "discard_changes": true})).unwrap_err().to_json_value();
    assert_write_refused_as_closed(&err, "new");
    assert_eq!(files_in(&session_dir), before, "nothing was written into the folder");
}

/// The editor itself answers that the tab is not the one shown: nothing changed, so it is retryable too.
#[test]
fn editor_ack_that_the_tab_is_not_shown_is_retryable() {
    let dir = Workspace::new();
    let (session_dir, mut s) = live_session(&dir, 5);
    let writer = editor_answers(session_dir.clone(), |id| vec![not_shown_answer(id)]);
    let err = s.call("designos_close_document", json!({"unsaved": "save"})).unwrap_err().to_json_value();
    writer.join().unwrap();
    assert_eq!(err["code"], json!("EDITOR_NOT_NOTIFIED"), "{err}");
    assert_eq!(err["safe_to_retry"], true, "{err}");
    assert!(err["message"].as_str().unwrap_or("").contains("not the one currently shown"), "{err}");
}

fn not_shown_answer(close_id: u64) -> Value {
    json!({"close_id": close_id, "ok": false, "code": "EDITOR_NOT_NOTIFIED", "message": "The close was dropped because that tab is not the one currently shown. Switch to it, then try again."})
}

/// Plays the editor: waits for the close line, then writes `answers(close_id)` as the result file, one after the other.
///
/// Answers are published back-to-back.  A wall-clock sleep between a stale answer and the real refusal makes this
/// test depend on the server's acknowledgement timeout and flakes when the runner is under load.  The separate
/// `an_answer_with_another_close_id_is_ignored` test covers the stale-id filtering while this test checks that the
/// matching refusal, rather than a stale `closed:true`, is the result.
fn editor_answers(session_dir: PathBuf, answers: impl FnOnce(u64) -> Vec<Value> + Send + 'static) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let end = Instant::now() + Duration::from_secs(5);
        while Instant::now() < end {
            if let Some(id) = last_close_id(&session_dir) {
                for body in answers(id) {
                    fs::write(session_dir.join(CloseIntent::RESULT_FILE), body.to_string()).unwrap();
                }
                return;
            }
            thread::sleep(Duration::from_millis(3));
        }
        panic!("close line with close_id never appeared");
    })
}

fn last_close_id(session_dir: &Path) -> Option<u64> {
    let raw = fs::read_to_string(session_dir.join("doc-patch.jsonl")).ok()?;
    let line = raw.lines().rev().find(|l| l.contains("\"close\":true"))?;
    match LiveLogLine::parse(line.as_bytes()) {
        Ok(LiveLogLine::Close(intent)) => intent.close_id,
        _ => None,
    }
}

/// What a former server process left behind: its first close id was 1 and the editor answered it with success.
fn leave_stale_close_files(session_dir: &Path) {
    let stale = json!({"close_id": 1u64, "ok": true, "closed": true, "saved": true, "unsaved": "save", "doc_revision": 7u64, "tab_id": 8u64});
    fs::write(session_dir.join(CloseIntent::RESULT_FILE), stale.to_string()).unwrap();
    fs::write(session_dir.join(CloseIntent::CANCEL_FILE), json!({"close_id": 1u64}).to_string()).unwrap();
}

/// A result file from a former process must never be taken for this close's answer, for any policy. With no editor answering,
/// the call times out (`EDITOR_NOT_NOTIFIED`), never `closed:true`, and the old files are gone before the close is published.
#[test]
fn a_stale_result_file_from_a_former_process_is_not_this_closes_answer() {
    for policy in ["save", "fail", "discard"] {
        let dir = Workspace::new();
        let (session_dir, mut s) = live_session(&dir, 9);
        leave_stale_close_files(&session_dir);
        let err = s.call("designos_close_document", json!({"unsaved": policy})).unwrap_err().to_json_value();
        assert_eq!(err["code"], json!("EDITOR_NOT_NOTIFIED"), "{policy}: {err}");
        assert!(err.get("closed").is_none(), "{policy}: {err}");
        let result = fs::read_to_string(session_dir.join(CloseIntent::RESULT_FILE));
        assert!(result.is_err(), "{policy}: the stale result was deleted before publishing: {result:?}");
        let cancel: Value = serde_json::from_slice(&fs::read(session_dir.join(CloseIntent::CANCEL_FILE)).unwrap()).unwrap();
        assert_ne!(cancel["close_id"], json!(1u64), "{policy}: the cancel file is this close's: {cancel}");
        assert_eq!(Some(cancel["close_id"].as_u64().unwrap()), last_close_id(&session_dir), "{policy}");
        assert!(s.call("designos_get_document", json!({})).is_ok(), "{policy}: the session stays open");
    }
}

/// A stale result (written late by another process, with another id) followed by the editor's real refusal: the refusal is
/// reported, `closed:true` is not.
#[test]
fn stale_result_plus_editor_refusal_is_not_closed() {
    let dir = Workspace::new();
    let (session_dir, mut s) = live_session(&dir, 10);
    // The simulated editor answers as soon as the server returns from its wait for the answer; a slow runner must not turn that
    // into a timeout, so this test does not depend on the short acknowledgement window the other tests use.
    s.set_close_ack_timeout(Duration::from_secs(10));
    s.call("designos_save_file", json!({"path": "design.designos"})).unwrap();
    make_dirty(&mut s);
    leave_stale_close_files(&session_dir);
    let writer = editor_answers(session_dir.clone(), |id| {
        vec![
            json!({"close_id": id ^ 1, "ok": true, "closed": true, "saved": false, "unsaved": "fail", "doc_revision": 1u64, "tab_id": 10u64}),
            json!({"close_id": id, "ok": false, "code": "UNSAVED_CHANGES", "message": "Unsaved changes. The document stayed open."}),
        ]
    });
    let outcome = s.call("designos_close_document", json!({}));
    writer.join().unwrap();
    let err = outcome.expect_err("a stale closed:true must not be reported").to_json_value();
    assert_eq!(err["code"], json!("UNSAVED_CHANGES"), "{err}");
    assert!(err.get("closed").is_none(), "{err}");
    assert!(s.call("designos_get_document", json!({})).is_ok(), "the document is still open");
}

/// An answer that carries another close's id is never taken, even when it is the only file there.
#[test]
fn an_answer_with_another_close_id_is_ignored() {
    let dir = Workspace::new();
    let (session_dir, mut s) = live_session(&dir, 11);
    let writer = editor_answers(session_dir.clone(), |id| {
        vec![json!({"close_id": id.wrapping_add(1), "ok": true, "closed": true, "saved": false, "unsaved": "discard", "doc_revision": 1u64, "tab_id": 11u64})]
    });
    let err = s.call("designos_close_document", json!({"unsaved": "discard"})).unwrap_err().to_json_value();
    writer.join().unwrap();
    assert_eq!(err["code"], json!("EDITOR_NOT_NOTIFIED"), "{err}");
    assert!(err.get("closed").is_none(), "{err}");
}

/// Two sessions (or two runs of the server) never share a close id, so one's answer cannot be taken for the other's.
#[test]
fn close_ids_differ_between_sessions() {
    let mut ids = Vec::new();
    for tab in [12, 13, 14] {
        let dir = Workspace::new();
        let (session_dir, mut s) = live_session(&dir, tab);
        let _ = s.call("designos_close_document", json!({"unsaved": "fail"}));
        ids.push(last_close_id(&session_dir).expect("close id published"));
    }
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), 3, "{ids:?}");
    assert!(ids.iter().all(|id| id >> 32 != 0), "the id carries the session's random part: {ids:?}");
}

/// PM condition: a `discard` the editor refuses because the tab is not the one shown changed nothing, so it stays retryable, and
/// the document is still open.
#[test]
fn discard_stays_retryable_when_the_tab_is_not_shown_and_the_document_stays_open() {
    let dir = Workspace::new();
    let (session_dir, mut s) = live_session(&dir, 15);
    s.call("designos_save_file", json!({"path": "design.designos"})).unwrap();
    make_dirty(&mut s);
    let before = node_count(&mut s);
    let writer = editor_answers(session_dir.clone(), |id| vec![not_shown_answer(id)]);
    let err = s.call("designos_close_document", json!({"unsaved": "discard"})).unwrap_err().to_json_value();
    writer.join().unwrap();
    assert_eq!(err["code"], json!("EDITOR_NOT_NOTIFIED"), "{err}");
    assert_eq!(err["safe_to_retry"], true, "{err}");
    assert!(err["next_step"].as_str().unwrap_or("").contains("designos_close_document again"), "{err}");
    assert!(err.get("closed").is_none(), "{err}");
    assert_eq!(node_count(&mut s), before, "the document is still open with its edit");
}

/// Closing a tab that is already closed is not retryable: asking again can never work.
#[test]
fn closing_an_already_closed_tab_is_not_retryable() {
    for policy in ["save", "fail", "discard"] {
        let dir = Workspace::new();
        let (session_dir, mut s) = live_session(&dir, 16);
        let writer = editor_answers(session_dir.clone(), |id| {
            vec![
                json!({"close_id": id, "ok": false, "code": "EDITOR_NOT_NOTIFIED", "reason": "tab_closed", "message": "The tab is already closed, so there is nothing to close."}),
            ]
        });
        let err = s.call("designos_close_document", json!({"unsaved": policy})).unwrap_err().to_json_value();
        writer.join().unwrap();
        assert_already_closed(&err, policy);
        assert!(err["message"].as_str().unwrap_or("").contains("already closed"), "{policy}: {err}");
        // The editor's answer ends the session as the mark does, also with no mark in the folder.
        assert!(!session_dir.join(CloseIntent::TAB_CLOSED_FILE).exists(), "{policy}: precondition: only the answer says so");
        let page = page_id(&mut s);
        let before = files_in(&session_dir);
        let late = json!({"parent_id": page, "kind": "frame", "name": "Late"});
        let err = s.call("designos_create_node", late).map_err(|e| e.to_json_value()).expect_err("the session is over");
        assert_write_refused_as_closed(&err, policy);
        assert_eq!(files_in(&session_dir), before, "{policy}: nothing was written into the folder");
    }
}

/// After a live close succeeded, a second close of the same session is refused at once (no editor is asked) and is not retryable.
#[test]
fn a_second_close_after_a_successful_live_close_is_not_retryable() {
    for policy in ["save", "fail", "discard"] {
        let dir = Workspace::new();
        let (session_dir, mut s) = live_session(&dir, 17);
        let writer = editor_answers(session_dir.clone(), |id| {
            vec![json!({"close_id": id, "ok": true, "closed": true, "saved": false, "unsaved": "discard", "doc_revision": 5u64, "tab_id": 17u64})]
        });
        let first = s.call("designos_close_document", json!({"unsaved": "discard"})).expect("first close");
        writer.join().unwrap();
        assert_eq!(first["closed"], true, "{first}");
        let sent = last_close_id(&session_dir);
        let err = s.call("designos_close_document", json!({"unsaved": policy})).unwrap_err().to_json_value();
        assert_already_closed(&err, policy);
        assert_eq!(last_close_id(&session_dir), sent, "{policy}: no second close was published");
    }
}

/// The editor's answer to this session's own close, as the editor writes it.
fn closed_answer(close_id: u64, tab_id: u64) -> Value {
    json!({"close_id": close_id, "ok": true, "closed": true, "saved": false, "unsaved": "discard", "doc_revision": 1u64, "tab_id": tab_id})
}

/// The agent closed its own tab: that session is over just as when the person closes the tab. Opening or creating a document is
/// refused, even before the editor has left its mark in the folder, and nothing is written into the folder.
#[test]
fn after_the_agents_own_close_opening_or_creating_a_document_is_refused() {
    let dir = Workspace::new();
    let (session_dir, mut s) = live_session(&dir, 18);
    s.set_close_ack_timeout(Duration::from_secs(10));
    s.call("designos_save_file", json!({"path": "other.designos"})).unwrap();
    let writer = editor_answers(session_dir.clone(), |id| vec![closed_answer(id, 18)]);
    let out = s.call("designos_close_document", json!({"unsaved": "discard"})).expect("the close");
    writer.join().unwrap();
    assert_eq!(out["closed"], true, "{out}");
    assert!(!session_dir.join(CloseIntent::TAB_CLOSED_FILE).exists(), "precondition: no mark yet, only this session knows");
    let before = files_in(&session_dir);
    let err = s.call("designos_open_file", json!({"path": "other.designos"})).unwrap_err().to_json_value();
    assert_write_refused_as_closed(&err, "open");
    let err = s.call("designos_new_document", json!({"name": "Late", "discard_changes": true})).unwrap_err().to_json_value();
    assert_write_refused_as_closed(&err, "new");
    assert_eq!(files_in(&session_dir), before, "nothing was written into the folder");
}

/// The answer to the agent's own close ends the session, so the options of the new, empty session are not saved into the folder.
#[test]
fn the_agents_own_close_saves_no_session_options_into_the_folder() {
    let dir = Workspace::new();
    let (session_dir, mut s) = live_session(&dir, 19);
    s.set_close_ack_timeout(Duration::from_secs(10));
    let options = session_dir.join("session-options.json");
    assert!(!options.exists(), "precondition: no options saved yet");
    let writer = editor_answers(session_dir.clone(), |id| vec![closed_answer(id, 19)]);
    let out = s.call("designos_close_document", json!({"unsaved": "discard"})).expect("the close");
    writer.join().unwrap();
    assert_eq!(out["closed"], true, "{out}");
    assert!(!options.exists(), "the closed tab's folder got the options of a session that has ended");
}

/// Options set after the tab is closed stay in force for the session, but are not written into the closed tab's folder.
#[test]
fn configuring_the_session_after_the_tab_left_its_mark_writes_nothing() {
    let dir = Workspace::new();
    let (session_dir, mut s) = closed_tab_folder(&dir, 20);
    let before = files_in(&session_dir);
    s.call("designos_configure_session", json!({"issues": {"mode": "off"}})).expect("the options apply to the session");
    assert_eq!(files_in(&session_dir), before, "nothing was written into the folder");
}

/// The person closes the tab while the agent's close waits for its answer: the close is not cancelled through the folder (nothing
/// reads it any more) and the answer is the same "already closed" as for any later call, not a retryable time-out.
#[test]
fn a_tab_closed_while_a_close_waits_writes_no_cancel_and_is_already_closed() {
    for policy in ["save", "fail", "discard"] {
        let dir = Workspace::new();
        let (session_dir, mut s) = live_session(&dir, 21);
        // Long enough that the mark is surely there when the wait ends, on a loaded runner too.
        s.set_close_ack_timeout(Duration::from_secs(2));
        let marker_dir = session_dir.clone();
        let editor = thread::spawn(move || {
            let end = Instant::now() + Duration::from_secs(5);
            while last_close_id(&marker_dir).is_none() {
                assert!(Instant::now() < end, "close line with close_id never appeared");
                thread::sleep(Duration::from_millis(3));
            }
            fs::write(marker_dir.join(CloseIntent::TAB_CLOSED_FILE), b"{\"tab_closed\":true}\n").unwrap();
        });
        let err = s.call("designos_close_document", json!({"unsaved": policy})).unwrap_err().to_json_value();
        editor.join().unwrap();
        assert_already_closed(&err, policy);
        assert!(!session_dir.join(CloseIntent::CANCEL_FILE).exists(), "{policy}: a cancel was written into the closed tab's folder");
    }
}
