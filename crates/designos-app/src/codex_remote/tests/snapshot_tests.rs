//! The committed 0.164 fixtures: recorded frames, the schema subset, and the rule that they carry no personal data.

use super::support::{fixture, THREAD, TURN1, TURN2};
use crate::codex_remote::item_index::ItemIndex;
use crate::codex_remote::protocol::{classify, parse_user_agent, status_facts, thread_facts, Inbound};
use serde_json::Value;
use std::path::{Path, PathBuf};

const FIXTURE_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/src/codex_remote/fixtures");
const ZERO_UUID: &str = "00000000-0000-0000-0000-000000000000";

fn fixture_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for entry in std::fs::read_dir(dir).expect("fixture folder") {
        let path = entry.unwrap().path();
        if path.is_dir() {
            files.extend(fixture_files(&path));
        } else {
            files.push(path);
        }
    }
    files
}

/// Walks every (key, value) of a JSON tree.
fn walk(node: &Value, visit: &mut dyn FnMut(&str, &Value)) {
    match node {
        Value::Object(map) => map.iter().for_each(|(key, value)| {
            visit(key, value);
            walk(value, visit);
        }),
        Value::Array(items) => items.iter().for_each(|value| walk(value, visit)),
        _ => {}
    }
}

/// Names of the machine and its user, read at run time: writing the owner's own name into a public test would be the leak.
fn local_identity() -> Vec<String> {
    let host = std::process::Command::new("hostname").output().map(|out| String::from_utf8_lossy(&out.stdout).trim().to_string()).unwrap_or_default();
    let home = std::env::var("HOME").ok().and_then(|home| Path::new(&home).file_name().map(|name| name.to_string_lossy().into_owned()));
    let names = [std::env::var("USER").ok(), std::env::var("LOGNAME").ok(), home, host.split('.').next().map(str::to_string), Some(host.clone())];
    names.into_iter().flatten().filter(|name| name.len() >= 4).collect()
}

/// The shape of a secret: an `sk-` key, a JWT header, a bearer header.
fn secret_shape(text: &str) -> Option<&'static str> {
    let key_at = |at: usize| text[at..].chars().skip(3).take_while(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_')).count() >= 12;
    let sk_key = text.match_indices("sk-").any(|(at, _)| text[..at].chars().next_back().is_none_or(|before| !before.is_alphanumeric()) && key_at(at));
    [(sk_key, "an sk- key"), (text.contains("eyJ"), "a JWT"), (text.contains("Bearer "), "a bearer header")]
        .into_iter()
        .find(|(found, _)| *found)
        .map(|(_, what)| what)
}

#[test]
fn the_secret_shape_check_sees_what_it_is_for_and_leaves_ordinary_words_alone() {
    assert_eq!(secret_shape("key sk-proj-abcdefghijklmnop1234"), Some("an sk- key"));
    assert_eq!(secret_shape("{\"token\":\"eyJhbGciOiJIUzI1NiJ9.e30.x\"}"), Some("a JWT"));
    assert_eq!(secret_shape("Authorization: Bearer abc"), Some("a bearer header"));
    for fine in ["a task-list", "risk-assessment-for-everything", "sk-short", "designos_new_document", ""] {
        assert_eq!(secret_shape(fine), None, "{fine}");
    }
}

#[test]
fn fixtures_carry_no_personal_or_machine_data() {
    let files = fixture_files(Path::new(FIXTURE_DIR));
    assert!(files.len() >= 10, "the fixture folder is missing: {FIXTURE_DIR}");
    let identity = local_identity();
    for file in files {
        let text = std::fs::read_to_string(&file).unwrap();
        let name = file.display();
        for forbidden in ["/Users/", "/home/", ".local", "Mac", "/private/", "/var/folders", "/tmp/"] {
            assert!(!text.contains(forbidden), "{name} contains {forbidden:?}");
        }
        assert_eq!(secret_shape(&text), None, "{name} holds the shape of a secret");
        for who in &identity {
            assert!(!text.contains(who.as_str()), "{name} contains this machine's user or host name");
        }
        let Ok(json) = serde_json::from_str::<Value>(&text) else { continue };
        walk(&json, &mut |key, value| {
            if matches!(key, "installationId" | "environmentId") && value.as_str().is_some_and(|id| id.len() == 36) {
                assert_eq!(value, ZERO_UUID, "{name}: {key} must be the placeholder");
            }
            if matches!(key, "cwd" | "path" | "codexHome") {
                let path = value.as_str().unwrap_or_default();
                assert!(path.starts_with("/session") || path.starts_with("/codex-home"), "{name}: {key} = {path:?} is not a placeholder path");
            }
        });
    }
}

fn is_request(name: &str) -> bool {
    matches!(classify(&fixture(name)), Inbound::Request { .. })
}

#[test]
fn the_recorded_frames_decode_through_the_protocol_readers() {
    let hello = fixture("initialize-response");
    assert_eq!(parse_user_agent(hello["result"]["userAgent"].as_str().unwrap()), Some(("designos-studio-spike-A", "0.164.0")));
    for name in ["mcp-elicitation-request", "mcp-elicitation-request-late", "command-approval-request", "file-change-request.synthetic"] {
        assert!(is_request(name), "{name}");
    }
    for name in [
        "thread-started",
        "thread-status-active",
        "thread-status-waiting",
        "item-started-mcp",
        "item-completed-mcp",
        "item-started-command",
        "request-resolved",
        "turn-completed",
        "item-started-file-change.synthetic",
    ] {
        assert!(matches!(classify(&fixture(name)), Inbound::Notification { .. }), "{name}");
    }
    for name in ["initialize-response", "loaded-list-response", "thread-read-response", "items-list-response.synthetic"] {
        assert!(matches!(classify(&fixture(name)), Inbound::Response { outcome: Ok(_), .. }), "{name}");
    }
    assert!(matches!(classify(&fixture("resume-error-32600")), Inbound::Response { id: 6, outcome: Err(-32600) }));
    for (name, thread) in [
        ("thread-started", fixture("thread-started")["params"]["thread"].clone()),
        ("thread-read-response", fixture("thread-read-response")["result"]["thread"].clone()),
    ] {
        let facts = thread_facts(&thread).unwrap_or_else(|| panic!("{name} decodes"));
        assert_eq!((facts.id.as_str(), facts.root, facts.active, facts.waiting_on_approval), (THREAD, true, false, false), "{name}");
    }
    assert_eq!(status_facts(&fixture("thread-status-active")["params"]["status"]), (true, false));
    assert_eq!(status_facts(&fixture("thread-status-waiting")["params"]["status"]), (true, true));
    assert_eq!(fixture("loaded-list-response")["result"]["data"][0], THREAD);
    assert_eq!(fixture("turn-completed")["params"]["turn"]["id"], TURN1);
    assert_eq!(fixture("mcp-elicitation-request")["params"]["turnId"], TURN2);
}

#[test]
fn recorded_items_are_tracked_while_they_run_and_forgotten_when_they_complete() {
    let mut items = ItemIndex::default();
    let started = fixture("item-started-mcp");
    items.record(THREAD, TURN2, &started["params"]["item"]);
    assert_eq!(items.mcp_calls(THREAD, TURN2, "designos"), 1);
    let done = fixture("item-completed-mcp");
    items.record(THREAD, TURN2, &done["params"]["item"]);
    assert_eq!(items.mcp_calls(THREAD, TURN2, "designos"), 1, "a different call's completion leaves this one running");
    let finished = serde_json::json!({"type": "mcpToolCall", "id": started["params"]["item"]["id"], "server": "designos", "tool": "t", "status": "completed", "arguments": {}});
    items.record(THREAD, TURN2, &finished);
    assert_eq!(items.mcp_calls(THREAD, TURN2, "designos"), 0);
    items.record(THREAD, TURN2, &fixture("item-started-command")["params"]["item"]);
    assert_eq!(items.mcp_calls(THREAD, TURN2, "designos"), 0, "other item types are not tracked");
}
