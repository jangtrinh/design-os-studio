//! Policy matrix for `designos_save_file` in-place save and `designos_close_document`.
//!
//! Dirty and clean × save / discard / fail / missing / invalid × titled / untitled × path / no path.

use designos_mcp::server::Session;
use serde_json::{json, Value};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
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
