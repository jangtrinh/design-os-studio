//! How a refused or failed session file reaches the agent: once while it lasts, naming the path.
use super::scratch::{assert_told, link_sentence, warnings, Scratch, VICTIM};
use designos_mcp::server::Session;
use serde_json::json;
use std::fs;

#[test]
fn a_problem_that_stays_is_told_once_and_one_that_returns_is_told_again() {
    let scratch = Scratch::new();
    let victim = scratch.victim(VICTIM);
    let link = scratch.link("context.json", &victim);
    let mut session = scratch.session();
    let get_document = |session: &mut Session| session.call("designos_get_document", json!({})).unwrap();
    assert_told(&get_document(&mut session), &link_sentence(&link));
    assert!(warnings(&get_document(&mut session)).is_empty(), "a problem that stays is told once");
    fs::remove_file(&link).unwrap();
    fs::write(&link, br#"{"run_id": "r1"}"#).unwrap();
    assert!(warnings(&get_document(&mut session)).is_empty(), "the agent deleted the link: nothing to tell");
    fs::remove_file(&link).unwrap();
    scratch.link("context.json", &victim);
    assert_told(&get_document(&mut session), &link_sentence(&link));
}

#[test]
fn a_session_folder_that_is_gone_is_an_error_naming_the_file_and_is_not_made_again() {
    let scratch = Scratch::new();
    let mut session = scratch.session();
    fs::remove_dir_all(&scratch.session).unwrap();
    let err = session.call("designos_new_document", json!({"name": "Probe", "discard_changes": true})).unwrap_err();
    let canvas = scratch.session.join("canvas.designos");
    assert!(err.contains(&format!("Cannot write {}: ", canvas.display())), "{err}");
    let reply = session.call("designos_configure_session", json!({"issues": {"max_items": 5}})).unwrap();
    let options = scratch.session.join("session-options.json");
    assert!(warnings(&reply).iter().any(|w| w.starts_with(&format!("Cannot write {}: ", options.display()))), "{reply}");
    assert!(!scratch.session.exists(), "designos-mcp made the session folder, which is Studio's");
}
