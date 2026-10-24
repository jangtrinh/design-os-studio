//! A FIFO planted at a session-file name: designos-mcp returns at once and says which path to delete.
use super::scratch::{assert_told, not_plain_sentence, within_one_second, Scratch};
use designos_mcp::server::Session;
use serde_json::json;

/// A FIFO at the read name `name`: the session starts, answers, and the first reply names the FIFO.
fn a_read_name(name: &str) {
    let scratch = Scratch::new();
    let fifo = scratch.fifo(name);
    let (workspace, session) = (scratch.workspace.clone(), scratch.session.clone());
    let reply = within_one_second(&fifo, move || Session::with_session_dir(&workspace, &session).unwrap().call("designos_get_document", json!({})).unwrap());
    assert_told(&reply, &not_plain_sentence(&fifo));
}

#[test]
fn a_fifo_at_the_canvas_is_refused() {
    a_read_name("canvas.designos");
}

#[test]
fn a_fifo_at_the_context_is_refused() {
    a_read_name("context.json");
}

#[test]
fn a_fifo_at_the_tool_config_is_refused() {
    a_read_name("mcp-config.json");
}

#[test]
fn a_fifo_at_the_bootstrap_is_refused() {
    a_read_name("bootstrap.json");
}

#[test]
fn a_fifo_at_the_session_options_is_refused() {
    a_read_name("session-options.json");
}

#[test]
fn a_fifo_at_the_ledger_is_refused() {
    a_read_name("idempotency-ledger.jsonl");
}

#[test]
fn a_fifo_at_the_edit_log_is_refused_and_the_edit_fails_naming_it() {
    let scratch = Scratch::new();
    let fifo = scratch.fifo("doc-patch.jsonl");
    let (workspace, session) = (scratch.workspace.clone(), scratch.session.clone());
    let outcome = within_one_second(&fifo, move || {
        let mut session = Session::with_session_dir(&workspace, &session).unwrap();
        session.call("designos_new_document", json!({"name": "Probe", "discard_changes": true})).map(|_| ()).map_err(|e| e.to_string())
    });
    let err = outcome.unwrap_err();
    assert!(err.contains(&not_plain_sentence(&fifo)), "{err}");
}
