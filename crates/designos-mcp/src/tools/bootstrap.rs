//! `designos_get_app_state` MCP tool (`E15-16 §3.3`, `§5`).
//!
//! Builds the 7-section `Bootstrap` block from the MCP session's document plus session facts
//! (`enabled_tools`, `ledger_entries`, `options`). Only when `include_selection: true` is
//! explicitly passed is the `selection` section included (`#698` owner ruling).

use crate::engine::Engine;
use designos_doc::NodeId;
use designos_export::agent_context::{bootstrap, BootstrapOptions, DEFAULT_BOOTSTRAP_MAX_TOKENS, MAX_BOOTSTRAP_MAX_TOKENS, MIN_BOOTSTRAP_MAX_TOKENS};
use serde::Deserialize;
use serde_json::{json, Value};

const VALID_SECTIONS: &[&str] = &["freshness", "document", "outline", "variables", "components", "style", "rules"];

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GetAppStateArgs {
    #[serde(default)]
    pub target_node_ids: Option<Vec<i64>>,
    #[serde(default)]
    pub max_tokens: Option<i64>,
    #[serde(default)]
    pub sections: Option<Vec<String>>,
    #[serde(default)]
    pub include_selection: bool,
}

/// JSON Schema definition for `designos_get_app_state` (`§5`).
pub fn tool_definition() -> Value {
    json!({
        "name": "designos_get_app_state",
        "description": "Return the pre-fetched design context block. Sections: freshness, document, depth-3 target frames outline, variables summary, components and libraries, active style, and rules pack, plus session state. Pass include_selection=true ONLY when the user explicitly asks about their canvas selection.",
        "inputSchema": {
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "target_node_ids": {
                    "type": "array",
                    "items": { "type": "integer", "minimum": 1 },
                    "maxItems": 50
                },
                "max_tokens": {
                    "type": "integer",
                    "minimum": MIN_BOOTSTRAP_MAX_TOKENS,
                    "maximum": MAX_BOOTSTRAP_MAX_TOKENS,
                    "default": DEFAULT_BOOTSTRAP_MAX_TOKENS
                },
                "sections": {
                    "type": "array",
                    "items": {
                        "enum": ["document", "outline", "variables", "components", "style", "rules"]
                    }
                },
                "include_selection": {
                    "type": "boolean",
                    "default": false
                }
            }
        },
        "outputSchema": crate::tools::errors::tool_output_schema(
            json!({
                "generated_at": {"type": "string"},
                "doc_revision": {"type": "integer"},
                "scoped": {"type": "boolean"},
                "sections": {"type": "array"},
                "markdown": {"type": "string"},
                "total_tokens": {"type": "integer"},
                "session": {"type": "object"}
            }),
            &["generated_at", "doc_revision", "scoped", "sections", "markdown", "total_tokens", "session"],
        ),
        "annotations": {
            "readOnlyHint": true,
            "destructiveHint": false,
            "idempotentHint": true,
            "openWorldHint": false
        }
    })
}

/// Executes `designos_get_app_state` against `engine` (`§3.3`, `§5`).
pub fn call_get_app_state(engine: &Engine, args: Value) -> Result<Value, String> {
    let parsed: GetAppStateArgs = serde_json::from_value(args).map_err(|e| {
        let msg = e.to_string();
        if msg.contains("unknown variant") || msg.contains("invalid value") {
            format!("VALUE_OUT_OF_RANGE: {msg}")
        } else {
            format!("invalid arguments: {msg}")
        }
    })?;

    let max_tokens = match parsed.max_tokens {
        Some(v) if v < MIN_BOOTSTRAP_MAX_TOKENS as i64 || v > MAX_BOOTSTRAP_MAX_TOKENS as i64 => {
            return Err(format!("VALUE_OUT_OF_RANGE: max_tokens must be between {MIN_BOOTSTRAP_MAX_TOKENS} and {MAX_BOOTSTRAP_MAX_TOKENS} (got {v})"));
        }
        Some(v) => v as usize,
        None => DEFAULT_BOOTSTRAP_MAX_TOKENS,
    };

    let mut target_node_ids = Vec::new();
    if let Some(raw_ids) = parsed.target_node_ids {
        if raw_ids.len() > 50 {
            return Err(format!("VALUE_OUT_OF_RANGE: target_node_ids may contain at most 50 items (got {})", raw_ids.len()));
        }
        for raw_id in raw_ids {
            if raw_id < 1 {
                return Err(format!("VALUE_OUT_OF_RANGE: target_node_ids must be >= 1 (got {raw_id})"));
            }
            let nid = NodeId(raw_id as u64);
            if engine.doc.get(nid).is_none() {
                return Err(format!("NODE_NOT_FOUND: node {} not found", nid.0));
            }
            target_node_ids.push(nid);
        }
    }

    if let Some(secs) = &parsed.sections {
        for s in secs {
            if !VALID_SECTIONS.contains(&s.as_str()) {
                return Err(format!("VALUE_OUT_OF_RANGE: unknown section '{s}'"));
            }
        }
    }

    let current_page_id = engine.resolve_default_page().ok();
    // The names `tools/list` exposes, and the pack that documents exactly those tools (`§3.1`, `§8.6`).
    let enabled_tools = crate::rules::enabled_tool_names(engine.enabled_tools.as_ref());
    let rules_pack = crate::rules::generate_rules_pack(engine.enabled_tools.as_ref()).map_err(|e| format!("INTERNAL_ERROR: {e}"))?;
    let options = serde_json::to_value(&engine.session_options).map_err(|e| format!("INTERNAL_ERROR: session options are not serializable: {e}"))?;

    let local_mutations = engine.history.undo_depth() as u64;
    let doc_revision = engine.base_doc_revision.map(|base| base.saturating_add(local_mutations)).unwrap_or_else(|| engine.doc.mutation_epoch());

    let generated_at = if local_mutations == 0 { engine.bootstrap_generated_at.clone() } else { None };

    let opts = BootstrapOptions {
        file_path: engine.file_path.clone(),
        current_page_id,
        target_node_ids,
        max_tokens,
        sections: parsed.sections,
        include_selection: parsed.include_selection,
        selected_node_ids: if parsed.include_selection { engine.selected_node_ids.clone() } else { Vec::new() },
        scoped: engine.scoped,
        generated_at,
        doc_revision: Some(doc_revision),
        active_style: None,
        in_progress: Default::default(),
    };

    let boot = bootstrap(&engine.doc, &opts, &engine.doc.library.enabled, &rules_pack);

    let sections_json: Vec<Value> = boot
        .sections
        .iter()
        .map(|sec| {
            let mut obj = serde_json::Map::new();
            obj.insert("id".into(), json!(sec.id));
            obj.insert("title".into(), json!(sec.title));
            obj.insert("tokens".into(), json!(sec.tokens));
            obj.insert("truncated".into(), json!(sec.truncated));
            if let Some(note) = &sec.truncation_note {
                obj.insert("truncation_note".into(), json!(note));
            }
            obj.insert("content".into(), json!(sec.content));
            Value::Object(obj)
        })
        .collect();

    Ok(json!({
        "generated_at": boot.generated_at,
        "doc_revision": boot.doc_revision,
        "scoped": boot.scoped,
        "sections": sections_json,
        "markdown": boot.markdown,
        "total_tokens": boot.total_tokens,
        "session": {
            "enabled_tools": enabled_tools,
            "ledger_entries": engine.ledger_entries,
            "options": options
        }
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use designos_doc::{Node, Rect};

    fn make_engine_with_selection() -> Engine {
        let mut engine = Engine::default();
        let pid = engine.doc.pages[0];
        let frame_id = engine.doc.alloc_id();
        let mut frame = Node::frame(frame_id, Rect::new(0.0, 0.0, 320.0, 200.0));
        frame.name = "Hero Card".into();
        let _ = engine.doc.insert(pid, frame);
        engine.selected_node_ids = vec![frame_id];
        engine.file_path = Some("/tmp/test-bootstrap.designos".into());
        engine.base_doc_revision = Some(42);
        engine.bootstrap_generated_at = Some("2026-06-21T10:00:00Z".into());
        engine
    }

    #[test]
    fn selection_only_by_explicit_argument() {
        let engine = make_engine_with_selection();

        // Default (include_selection omitted / false): no selection section.
        let res_default = call_get_app_state(&engine, json!({})).expect("default call succeeds");
        let secs_default = res_default["sections"].as_array().expect("sections array");
        assert!(!secs_default.iter().any(|s| s["id"] == "selection"), "default call must not include selection section");
        assert!(!res_default["markdown"].as_str().unwrap_or_default().contains("## Selection"), "default markdown must not include ## Selection");

        // Explicit include_selection: true -> includes selection section.
        let res_with_sel = call_get_app_state(&engine, json!({ "include_selection": true })).expect("call with selection");
        let secs_with_sel = res_with_sel["sections"].as_array().expect("sections array");
        let sel_sec = secs_with_sel.iter().find(|s| s["id"] == "selection").expect("selection section present when include_selection=true");
        assert!(sel_sec["content"].as_str().unwrap_or_default().contains("Hero Card"), "selection content must include selected node name");
    }

    #[test]
    fn app_state_tool_matches_studio_block() {
        let mut engine = make_engine_with_selection();
        // Reset mutation_epoch effect by building Studio's bootstrap with the same doc_revision.
        let doc_rev = engine.base_doc_revision.unwrap().saturating_add(engine.doc.mutation_epoch());
        let studio_opts = BootstrapOptions {
            file_path: engine.file_path.clone(),
            current_page_id: engine.resolve_default_page().ok(),
            doc_revision: Some(doc_rev),
            generated_at: Some("2026-06-21T10:00:00Z".into()),
            ..BootstrapOptions::default()
        };
        let rules_pack = crate::rules::generate_rules_pack(None).expect("rules pack");
        let studio_boot = bootstrap(&engine.doc, &studio_opts, &engine.doc.library.enabled, &rules_pack);

        // Simulate synced engine where doc.mutation_epoch() == 0 and base_doc_revision == doc_rev
        let synced = Engine {
            doc: serde_json::from_str(&serde_json::to_string(&engine.doc).unwrap()).unwrap(),
            file_path: engine.file_path.take(),
            base_doc_revision: Some(doc_rev),
            bootstrap_generated_at: Some("2026-06-21T10:00:00Z".into()),
            ..Default::default()
        };

        let tool_res = call_get_app_state(&synced, json!({})).expect("tool call succeeds");
        assert_eq!(tool_res["doc_revision"], json!(studio_boot.doc_revision));
        let tool_secs = tool_res["sections"].as_array().unwrap();
        assert_eq!(tool_secs.len(), studio_boot.sections.len());
        for (t_sec, s_sec) in tool_secs.iter().zip(studio_boot.sections.iter()) {
            assert_eq!(t_sec["id"], json!(s_sec.id));
            assert_eq!(t_sec["content"], json!(s_sec.content));
        }
    }

    /// Creates a workspace and a session directory under the OS temp dir (removed by the caller).
    fn session_dirs(tag: &str) -> (std::path::PathBuf, std::path::PathBuf) {
        let nonce = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).expect("clock").as_nanos();
        let base = std::env::temp_dir().join(format!("designos-app-state-{tag}-{}-{nonce}", std::process::id()));
        let (ws, sess) = (base.join("ws"), base.join("session"));
        std::fs::create_dir_all(&ws).expect("workspace dir");
        std::fs::create_dir_all(&sess).expect("session dir");
        (ws, sess)
    }

    fn string_set(value: &Value) -> std::collections::BTreeSet<String> {
        value.as_array().expect("array").iter().map(|v| v.as_str().expect("string").to_string()).collect()
    }

    fn rules_section(reply: &Value) -> &Value {
        reply["sections"].as_array().expect("sections").iter().find(|s| s["id"] == "rules").expect("rules section")
    }

    #[test]
    fn default_session_reports_every_registered_tool_and_the_generated_rules_pack() {
        let (ws, sess) = session_dirs("default");
        let server = crate::server::DesignOSServer::with_session_dir(&ws, &sess).expect("server");
        let reply = server.call("designos_get_app_state", json!({})).expect("designos_get_app_state");

        let listed: std::collections::BTreeSet<String> = server.list_enabled_tools().into_iter().map(|t| t.name.to_string()).collect();
        let reported = string_set(&reply["session"]["enabled_tools"]);
        assert_eq!(reported, listed, "session.enabled_tools must be the tools/list set");
        assert_eq!(reported.len(), crate::server::tools().len());

        let rules = rules_section(&reply);
        let pack = crate::rules::generate_rules_pack(None).expect("rules pack");
        assert_eq!(rules["content"].as_str().expect("content"), pack.trim(), "section 7 must be the generated E15-15 pack");
        assert_eq!(rules["truncated"], json!(false), "the generated pack must fit the section 7 cap: {} tokens", rules["tokens"]);
        let _ = std::fs::remove_dir_all(ws.parent().expect("base dir"));
    }

    #[test]
    fn a_disabled_tool_is_absent_from_rules_and_enabled_tools() {
        let (ws, sess) = session_dirs("disabled");
        let enabled: Vec<String> = crate::server::tools().into_iter().map(|t| t.name.to_string()).filter(|n| n != "designos_delete_node").collect();
        std::fs::write(sess.join("mcp-config.json"), json!({"designos": {"enabled_tools": enabled}}).to_string()).expect("mcp-config.json");
        let server = crate::server::DesignOSServer::with_session_dir(&ws, &sess).expect("server");
        let reply = server.call("designos_get_app_state", json!({})).expect("designos_get_app_state");

        let listed: std::collections::BTreeSet<String> = server.list_enabled_tools().into_iter().map(|t| t.name.to_string()).collect();
        let reported = string_set(&reply["session"]["enabled_tools"]);
        assert_eq!(reported, listed, "session.enabled_tools must be the tools/list set");
        assert!(!reported.contains("designos_delete_node") && reported.contains("designos_apply_batch"));
        assert_eq!(reported.len(), crate::server::tools().len() - 1);

        let expected_pack = crate::rules::generate_rules_pack(Some(&reported)).expect("filtered pack");
        let rules = rules_section(&reply)["content"].as_str().expect("content");
        assert_eq!(rules, expected_pack.trim(), "section 7 must be the generated pack for the enabled tools");
        assert!(!rules.contains("### `designos_delete_node`"), "a disabled tool must not be listed in section 7");
        assert!(rules.contains("### `designos_create_node`") && rules.contains("### `SlotFillSpec`"));
        let _ = std::fs::remove_dir_all(ws.parent().expect("base dir"));
    }

    #[test]
    fn session_facts_report_the_idempotency_ledger_and_the_session_options() {
        let (ws, sess) = session_dirs("facts");
        let server = crate::server::DesignOSServer::with_session_dir(&ws, &sess).expect("server");
        let page = server.call("designos_get_document", json!({})).expect("document")["pages"][0]["node_id"].as_u64().expect("page id");

        server.call("designos_configure_session", json!({"issues": {"mode": "off"}})).expect("configure session");
        server.call("designos_create_node", json!({"parent_id": page, "kind": "frame", "name": "Plain"})).expect("write without a key");
        for key in ["facts-key-1", "facts-key-2"] {
            server.call("designos_create_node", json!({"parent_id": page, "kind": "frame", "name": key, "idempotency_key": key})).expect("keyed write");
        }

        let reply = server.call("designos_get_app_state", json!({})).expect("designos_get_app_state");
        assert_eq!(reply["session"]["ledger_entries"], json!(2), "ledger size, not undo depth (3 writes were made): {}", reply["session"]);
        assert_eq!(reply["session"]["options"]["issues"]["mode"], json!("off"), "{}", reply["session"]);
        assert_eq!(reply["session"]["options"]["issues"]["preset"], json!("agent"));
        let _ = std::fs::remove_dir_all(ws.parent().expect("base dir"));
    }

    #[test]
    fn errors_on_out_of_range_and_missing_node() {
        let engine = make_engine_with_selection();
        let err_low = call_get_app_state(&engine, json!({ "max_tokens": 1000 })).unwrap_err();
        assert!(err_low.contains("VALUE_OUT_OF_RANGE"));
        let err_high = call_get_app_state(&engine, json!({ "max_tokens": 30000 })).unwrap_err();
        assert!(err_high.contains("VALUE_OUT_OF_RANGE"));
        let err_node = call_get_app_state(&engine, json!({ "target_node_ids": [999999] })).unwrap_err();
        assert!(err_node.contains("NODE_NOT_FOUND"));
    }
}
