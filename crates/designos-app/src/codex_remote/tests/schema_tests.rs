//! The committed snapshot of the 0.164 types Studio uses (`fixtures/0.164/schema-used.json`), and what the code sends
//! and reads against it. A newer Codex changes the live schema first: the ignored test at the end finds out.

use super::support::{fixture, scratch_dir};
use crate::codex_remote::protocol::{answer_result, items_list_params, read_params, resume_params, CardKind, Decision};
use serde::Deserialize;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

#[derive(Deserialize)]
struct Snapshot {
    codex: String,
    files: BTreeMap<String, Shape>,
    enums: BTreeMap<String, Vec<String>>,
}

#[derive(Deserialize)]
struct Shape {
    required: Vec<String>,
    properties: Vec<String>,
}

fn snapshot() -> Snapshot {
    serde_json::from_value(fixture("schema-used")).expect("the snapshot decodes")
}

fn keys(value: &Value) -> Vec<String> {
    value.as_object().expect("an object").keys().cloned().collect()
}

fn within(snapshot: &Snapshot, file: &str, sent: &Value) {
    let shape = &snapshot.files[file];
    for key in keys(sent) {
        assert!(shape.properties.contains(&key), "{key} is not a property of {file}");
    }
    for required in &shape.required {
        assert!(sent.get(required).is_some(), "{required} is required by {file}");
    }
}

#[test]
fn the_snapshot_pins_the_decline_variants_studio_sends() {
    let snapshot = snapshot();
    assert_eq!(snapshot.codex, "0.164.0");
    for (kind, name, file) in [
        (CardKind::Command, "CommandExecutionApprovalDecision", "CommandExecutionRequestApprovalResponse.json"),
        (CardKind::FileChange, "FileChangeApprovalDecision", "FileChangeRequestApprovalResponse.json"),
        (CardKind::McpTool, "McpServerElicitationAction", "McpServerElicitationRequestResponse.json"),
    ] {
        for (decision, word) in [(Decision::Approve, "accept"), (Decision::Deny, "decline")] {
            assert!(snapshot.enums[name].iter().any(|token| token == word), "{name} must offer {word}");
            let reply = answer_result(kind, decision);
            within(&snapshot, file, &reply);
            assert!(reply.as_object().unwrap().values().any(|value| value == word), "{reply}");
        }
    }
}

#[test]
fn the_requests_studio_sends_use_keys_the_snapshot_has() {
    let snapshot = snapshot();
    within(&snapshot, "v2/ThreadResumeParams.json", &resume_params("t"));
    within(&snapshot, "v2/ThreadReadParams.json", &read_params("t"));
    let list = items_list_params("t", "u");
    within(&snapshot, "v2/ThreadItemsListParams.json", &list);
    assert!(snapshot.enums["SortDirection"].contains(&list["sortDirection"].as_str().unwrap().to_string()));
}

#[test]
fn the_fields_the_mapping_reads_are_in_the_snapshot_or_known_to_ride_on_the_wire() {
    let snapshot = snapshot();
    let reads = [
        (
            "CommandExecutionRequestApprovalParams.json",
            vec!["kind", "command", "commandActions", "networkApprovalContext", "proposedNetworkPolicyAmendments", "threadId", "turnId"],
        ),
        ("McpServerElicitationRequestParams.json", vec!["serverName", "threadId", "turnId", "mode", "requestedSchema", "_meta"]),
        ("FileChangeRequestApprovalParams.json", vec!["grantRoot", "itemId", "threadId", "turnId"]),
        ("v2/ItemStartedNotification.json", vec!["item", "threadId", "turnId"]),
        ("v2/TurnCompletedNotification.json", vec!["threadId", "turn"]),
        ("v2/ServerRequestResolvedNotification.json", vec!["requestId", "threadId"]),
        ("v2/ThreadStartedNotification.json", vec!["thread"]),
        ("v2/ThreadStatusChangedNotification.json", vec!["threadId", "status"]),
        ("v2/ThreadItemsListResponse.json", vec!["data"]),
    ];
    for (file, fields) in reads {
        for field in fields {
            assert!(snapshot.files[file].properties.iter().any(|p| p == field), "{field} is not in {file}");
        }
    }
    assert!(snapshot.enums["ThreadActiveFlag"].contains(&"waitingOnApproval".to_string()));
    // Codex sends the decisions it offers with a command request, but the stable schema does not list the field. Studio
    // reads it only when it is there (accept must be among them), so the recorded frame is the evidence it exists.
    assert!(!snapshot.files["CommandExecutionRequestApprovalParams.json"].properties.contains(&"availableDecisions".to_string()));
    assert!(fixture("command-approval-request")["params"]["availableDecisions"].is_array());
}

/// The snapshot rule: per file the required and property names (one level into oneOf/anyOf/allOf), the tokens of the
/// named enum definitions, and the shape of the two thread items Studio tracks.
fn project(schema: &Path, snapshot: &Snapshot) -> Value {
    let load = |name: &str| serde_json::from_str::<Value>(&std::fs::read_to_string(schema.join(name)).unwrap_or_else(|e| panic!("{name}: {e}"))).unwrap();
    let names = |node: &Value| -> BTreeSet<String> {
        let mut out: BTreeSet<String> = node["properties"].as_object().into_iter().flatten().map(|(key, _)| key.clone()).collect();
        for list in ["oneOf", "anyOf", "allOf"] {
            node[list]
                .as_array()
                .into_iter()
                .flatten()
                .for_each(|branch| out.extend(branch["properties"].as_object().into_iter().flatten().map(|(key, _)| key.clone())));
        }
        out
    };
    let tokens = |definition: &Value| -> BTreeSet<String> {
        let branches: Vec<&Value> = definition["oneOf"].as_array().map_or_else(|| vec![definition], |list| list.iter().collect());
        branches
            .into_iter()
            .flat_map(|branch| match (branch["enum"].as_array(), branch["properties"].as_object()) {
                (Some(words), _) => words.iter().filter_map(|word| word.as_str().map(String::from)).collect::<Vec<_>>(),
                (None, Some(fields)) => fields.keys().map(|key| format!("{{{key}}}")).collect(),
                _ => Vec::new(),
            })
            .collect()
    };
    let (mut files, mut enums, mut items) = (BTreeMap::new(), BTreeMap::new(), BTreeMap::new());
    for name in snapshot.files.keys() {
        let doc = load(name);
        let required: BTreeSet<String> = doc["required"].as_array().into_iter().flatten().filter_map(|r| r.as_str().map(String::from)).collect();
        files.insert(name.clone(), serde_json::json!({"required": required, "properties": names(&doc)}));
        for enum_name in snapshot.enums.keys().filter(|enum_name| doc["definitions"].get(enum_name.as_str()).is_some()) {
            enums.insert(enum_name.clone(), tokens(&doc["definitions"][enum_name.as_str()]));
        }
        for branch in doc["definitions"]["ThreadItem"]["oneOf"].as_array().into_iter().flatten() {
            let kind = branch["properties"]["type"]["enum"][0].as_str().unwrap_or_default();
            if ["mcpToolCall", "fileChange"].contains(&kind) {
                let required: BTreeSet<String> = branch["required"].as_array().into_iter().flatten().filter_map(|r| r.as_str().map(String::from)).collect();
                items.insert(kind.to_string(), serde_json::json!({"required": required, "properties": names(branch)}));
            }
        }
    }
    serde_json::json!({"files": files, "enums": enums, "items": items})
}

/// Needs `codex` 0.164 on the PATH and writes only below a scratch folder. Run with `--ignored`.
#[test]
#[ignore = "needs the installed codex"]
fn codex_it_live_schema_matches_the_committed_snapshot() {
    let out = scratch_dir("sch");
    let status = std::process::Command::new("codex")
        .args(["app-server", "generate-json-schema", "--out"])
        .arg(&out)
        .env("CODEX_HOME", out.join("home"))
        .status()
        .expect("codex is installed");
    assert!(status.success());
    let committed = fixture("schema-used");
    let live = project(&out, &snapshot());
    for part in ["files", "enums", "items"] {
        assert_eq!(live[part], committed[part], "{part} differ from the committed 0.164 snapshot: refresh it and re-run the integration tests");
    }
}
