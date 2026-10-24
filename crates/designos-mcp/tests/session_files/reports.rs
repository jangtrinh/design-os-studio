//! How a refused or failed session file reaches the agent: once while it lasts, naming the path.
use super::scratch::{assert_told, is_cant_sentence, link_sentence, warnings, Scratch, VICTIM};
use designos_mcp::server::Session;
use rmcp::model::CallToolResult;
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
    assert!(err.contains(&format!("{} can't be written: ", canvas.display())), "{err}");
    assert!(err.contains("Check its permissions and free space, then try again."), "{err}");
    let reply = session.call("designos_configure_session", json!({"issues": {"max_items": 5}})).unwrap();
    let options = scratch.session.join("session-options.json");
    assert!(warnings(&reply).iter().any(|w| is_cant_sentence(w, "written", &options)), "{reply}");
    assert!(!scratch.session.exists(), "designos-mcp made the session folder, which is Studio's");
}

/// Every text block of a tool result: what a client shows the model.
fn content_text(result: &CallToolResult) -> String {
    result.content.iter().filter_map(|block| block.as_text().map(|text| text.text.clone())).collect::<Vec<_>>().join("\n")
}

#[test]
fn a_warning_reaches_the_model_when_the_reply_text_is_the_metadata_xml() {
    let scratch = Scratch::new();
    let victim = scratch.victim(VICTIM);
    let link = scratch.link("session-options.json", &victim);
    let mut session = scratch.session();
    let result = session.call_tool_result("designos_get_metadata", json!({}));
    assert_ne!(result.is_error, Some(true), "{result:?}");
    let text = content_text(&result);
    assert!(text.contains('<'), "the metadata XML is still there: {text}");
    assert!(text.contains(&link_sentence(&link)), "the model's text does not carry the warning: {text}");
}

#[test]
fn a_warning_reaches_the_model_when_the_reply_is_a_screenshot() {
    let scratch = Scratch::new();
    let victim = scratch.victim(VICTIM);
    let link = scratch.link("session-options.json", &victim);
    let mut session = scratch.session();
    // Built on the engine, so that no tool reply drains the warning before the screenshot's.
    let page = session.engine.doc.pages[0].0;
    let created = session.engine.call("designos_create_node", json!({"parent_id": page, "kind": "rectangle", "name": "R", "width": 40, "height": 40})).unwrap();
    let result = session.call_tool_result("designos_get_screenshot", json!({"node_id": created["node"]["id"]}));
    assert_ne!(result.is_error, Some(true), "{result:?}");
    assert!(result.content.iter().any(|block| block.as_image().is_some()), "the screenshot is still there");
    let text = content_text(&result);
    assert!(text.contains(&link_sentence(&link)), "the model's text does not carry the warning: {text}");
}
