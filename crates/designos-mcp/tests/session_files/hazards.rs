//! Hazards that are neither a link nor a FIFO: a read-only file, a folder and a socket planted at a session-file name. None
//! writes anything or waits, and each ends in a sentence with a next step.
use super::scratch::{assert_told, assert_untouched, is_cant_sentence, not_plain_sentence, warnings, Scratch, VICTIM};
use serde_json::json;
use std::{fs, time::Instant};

const NEXT_STEP: &str = "Check its permissions and free space, then try again.";

#[test]
fn a_read_only_edit_log_fails_the_edit_with_a_next_step() {
    let scratch = Scratch::new();
    let Some(log) = scratch.read_only_file("doc-patch.jsonl", VICTIM) else {
        return; // this user can write any file (root): nothing to test
    };
    let mut session = scratch.session();
    let err = session.call("designos_new_document", json!({"name": "Probe", "discard_changes": true})).unwrap_err();
    assert!(err.contains(&format!("{} can't be written: ", log.display())), "{err}");
    assert!(err.contains(NEXT_STEP), "{err}");
    assert_untouched(&log, VICTIM, "the edit log");
}

#[test]
fn a_read_only_ledger_is_told_as_a_warning_with_a_next_step() {
    let scratch = Scratch::new();
    let Some(ledger) = scratch.read_only_file("idempotency-ledger.jsonl", VICTIM) else {
        return;
    };
    let mut session = scratch.session();
    let reply = session.call("designos_new_document", json!({"name": "Probe", "discard_changes": true})).expect("a new document opens whatever the ledger is");
    assert!(warnings(&reply).iter().any(|w| is_cant_sentence(w, "written", &ledger)), "{reply}");
    assert_untouched(&ledger, VICTIM, "the ledger");
}

#[test]
fn a_folder_at_the_canvas_fails_the_write_with_a_next_step_and_leaves_no_temporary_file() {
    let scratch = Scratch::new();
    let canvas = scratch.session.join("canvas.designos");
    fs::create_dir(&canvas).unwrap();
    fs::write(canvas.join("inside.txt"), VICTIM).unwrap();
    let mut session = scratch.session();
    let err = session.call("designos_new_document", json!({"name": "Probe", "discard_changes": true})).unwrap_err();
    assert!(err.contains(&not_plain_sentence(&canvas)), "{err}");
    assert_untouched(&canvas.join("inside.txt"), VICTIM, "the folder's file");
    let left: Vec<_> = fs::read_dir(&scratch.session).unwrap().map(|e| e.unwrap().file_name().into_string().unwrap()).filter(|n| n.ends_with(".tmp")).collect();
    assert!(left.is_empty(), "temporary files left behind: {left:?}");
}

#[test]
fn a_socket_at_the_context_is_refused_at_once_with_a_next_step() {
    let scratch = Scratch::new();
    let socket = scratch.socket("context.json");
    let started = Instant::now();
    let reply = scratch.session().call("designos_get_document", json!({})).unwrap();
    assert!(started.elapsed().as_secs() < 1, "the socket held the server up");
    assert_told(&reply, &not_plain_sentence(&socket));
}
