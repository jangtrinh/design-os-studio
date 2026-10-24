//! A link planted at a session-file name: nothing is read through it, nothing is written through it, and the agent is told.
use super::scratch::{assert_told, assert_untouched, link_sentence, old_temp_name, second_name_sentence, warnings, Scratch, VICTIM};
use designos_doc::Document;
use designos_mcp::{server::Session, tools::session_options::SessionOptions};
use serde_json::{json, Value};
use std::fs;

fn new_document(session: &mut Session) -> Result<Value, String> {
    session.call("designos_new_document", json!({"name": "Probe", "discard_changes": true})).map_err(|e| e.to_string())
}

fn is_plain_file(path: &std::path::Path) -> bool {
    fs::symlink_metadata(path).map(|m| m.file_type().is_file()).unwrap_or(false)
}

/// A link at the read name `name` to a file holding `content`: the session must not read it, and the first reply says so.
/// `effect` checks that nothing of `content` reached the session.
fn a_read_name(name: &str, content: &[u8], effect: impl FnOnce(&mut Session, &Value)) {
    let scratch = Scratch::new();
    let victim = scratch.victim(content);
    let link = scratch.link(name, &victim);
    let mut session = scratch.session();
    let reply = session.call("designos_get_document", json!({})).unwrap();
    assert_told(&reply, &link_sentence(&link));
    assert_untouched(&victim, content, name);
    effect(&mut session, &reply);
}

#[test]
fn a_link_at_the_canvas_is_not_loaded() {
    let canvas = Document::new("Victim canvas").to_json().unwrap();
    a_read_name("canvas.designos", canvas.as_bytes(), |_, reply| assert_ne!(reply["name"], "Victim canvas"));
}

#[test]
fn a_link_at_the_context_is_not_read() {
    a_read_name("context.json", br#"{"run_id": "victim-run", "scoped": true}"#, |_, _| {});
}

#[test]
fn a_link_at_the_tool_config_is_not_read() {
    let config = br#"{"designos": {"enabled_tools": ["designos_get_document"]}}"#;
    a_read_name("mcp-config.json", config, |session, _| {
        new_document(session).expect("a tool the victim's config would have switched off must stay enabled");
    });
}

#[test]
fn a_link_at_the_bootstrap_is_not_read() {
    a_read_name("bootstrap.json", br#"{"generated_at": "victim-time"}"#, |session, _| assert_eq!(session.engine.bootstrap_generated_at, None));
}

#[test]
fn a_link_at_the_session_options_is_not_read() {
    let mut options = serde_json::to_value(SessionOptions::default()).unwrap();
    options["issues"]["max_items"] = json!(7);
    a_read_name("session-options.json", options.to_string().as_bytes(), |session, _| assert_ne!(session.engine.session_options.issues.max_items, 7));
}

#[test]
fn a_link_at_the_ledger_is_not_read() {
    let entry = json!({"key": "victim-key", "tool": "designos_create_node", "hash": 1, "undo_depth": 0, "reply": {}, "created_ms": 0});
    a_read_name("idempotency-ledger.jsonl", format!("{entry}\n").as_bytes(), |session, _| assert_eq!(session.ledger.len(), 0));
}

#[test]
fn a_link_at_the_edit_log_is_not_read() {
    a_read_name("doc-patch.jsonl", VICTIM, |_, _| {});
}

#[test]
fn a_link_at_the_edit_log_gets_no_append_and_the_edit_fails_naming_the_path() {
    let scratch = Scratch::new();
    let victim = scratch.victim(VICTIM);
    let link = scratch.link("doc-patch.jsonl", &victim);
    let mut session = scratch.session();
    let err = new_document(&mut session).unwrap_err();
    assert!(err.contains(&link_sentence(&link)), "{err}");
    let page = session.call("designos_get_document", json!({})).unwrap()["pages"][0]["node_id"].clone();
    let err = session.call("designos_create_node", json!({"kind": "frame", "parent_id": page, "name": "A"})).unwrap_err().to_string();
    assert!(err.contains(&link_sentence(&link)), "{err}");
    assert_untouched(&victim, VICTIM, "the edit log");
}

#[test]
fn a_dangling_link_at_the_edit_log_creates_nothing() {
    let scratch = Scratch::new();
    let missing = scratch.missing_victim();
    let link = scratch.link("doc-patch.jsonl", &missing);
    let mut session = scratch.session();
    let err = new_document(&mut session).unwrap_err();
    assert!(err.contains(&link_sentence(&link)), "{err}");
    assert!(!missing.exists(), "the link's missing target was created");
}

#[test]
fn a_hard_link_at_the_edit_log_gets_no_append() {
    let scratch = Scratch::new();
    let victim = scratch.victim(VICTIM);
    let link = scratch.hard_link("doc-patch.jsonl", &victim);
    let mut session = scratch.session();
    let err = new_document(&mut session).unwrap_err();
    assert!(err.contains(&second_name_sentence(&link)), "{err}");
    assert_untouched(&victim, VICTIM, "the edit log");
}

#[test]
fn a_hard_link_at_the_canvas_is_not_loaded() {
    let scratch = Scratch::new();
    let canvas = Document::new("Victim canvas").to_json().unwrap();
    let victim = scratch.victim(canvas.as_bytes());
    let link = scratch.hard_link("canvas.designos", &victim);
    let mut session = scratch.session();
    let reply = session.call("designos_get_document", json!({})).unwrap();
    assert_ne!(reply["name"], "Victim canvas", "the document behind a second name was loaded");
    assert_told(&reply, &second_name_sentence(&link));
    assert_untouched(&victim, canvas.as_bytes(), "the canvas");
}

#[test]
fn a_link_at_the_ledger_is_neither_truncated_nor_appended_to() {
    let scratch = Scratch::new();
    let victim = scratch.victim(VICTIM);
    let link = scratch.link("idempotency-ledger.jsonl", &victim);
    let mut session = scratch.session();
    let reply = new_document(&mut session).expect("a new document opens whatever the ledger is");
    assert_untouched(&victim, VICTIM, "the ledger after a new document");
    assert!(warnings(&reply).contains(&link_sentence(&link)), "{reply}");
    let page = session.call("designos_get_document", json!({})).unwrap()["pages"][0]["node_id"].clone();
    session.call("designos_create_node", json!({"kind": "frame", "parent_id": page, "name": "A", "idempotency_key": "k1"})).unwrap();
    assert_untouched(&victim, VICTIM, "the ledger after an idempotent write");
}

/// A link at the temporary name an earlier version used for `stem`, and the call `act` that writes `stem`.
fn a_link_at_the_old_temp_name(stem: &str, act: impl FnOnce(&mut Session)) {
    let scratch = Scratch::new();
    let victim = scratch.victim(VICTIM);
    scratch.link(&old_temp_name(stem), &victim);
    let mut session = scratch.session();
    act(&mut session);
    assert_untouched(&victim, VICTIM, stem);
}

#[test]
fn a_link_at_the_old_canvas_temp_name_gets_no_write() {
    a_link_at_the_old_temp_name("canvas", |session| {
        new_document(session).unwrap();
    });
}

#[test]
fn a_link_at_the_old_doc_sync_temp_name_gets_no_write() {
    a_link_at_the_old_temp_name("doc-sync", |session| {
        new_document(session).unwrap();
    });
}

#[test]
fn a_link_at_the_old_session_options_temp_name_gets_no_write() {
    a_link_at_the_old_temp_name("session-options", |session| {
        new_document(session).unwrap();
    });
}

/// A link at a name that is replaced as a whole: the link goes, the victim stays, and nothing is refused.
fn a_replaced_name(name: &str) {
    let scratch = Scratch::new();
    let victim = scratch.victim(VICTIM);
    let link = scratch.link(name, &victim);
    let mut session = scratch.session();
    new_document(&mut session).unwrap();
    assert_untouched(&victim, VICTIM, name);
    assert!(is_plain_file(&link), "{name} is still not a plain file");
}

#[test]
fn a_link_at_the_canvas_is_replaced_by_the_next_write() {
    a_replaced_name("canvas.designos");
}

#[test]
fn a_link_at_the_doc_sync_is_replaced_by_the_next_write() {
    a_replaced_name("doc-sync.designos");
}

#[test]
fn a_link_at_the_session_options_is_replaced_by_the_next_write() {
    a_replaced_name("session-options.json");
}
