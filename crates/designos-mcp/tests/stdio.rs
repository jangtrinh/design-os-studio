//! Real MCP client/server lifecycle across a subprocess, not a mocked dispatcher.
use rmcp::{
    model::{CallToolRequestParams, GetPromptRequestParams},
    transport::TokioChildProcess,
    ServiceExt,
};
use serde_json::{json, Value};

#[tokio::test]
async fn stdio_handshake_discovery_edit_and_error_recovery() {
    tokio::time::timeout(std::time::Duration::from_secs(30), async {
        let mut command = tokio::process::Command::new(env!("CARGO_BIN_EXE_designos-mcp"));
        command.arg("--workspace").arg(env!("CARGO_MANIFEST_DIR"));
        let client = ().serve(TokioChildProcess::new(command).unwrap()).await.unwrap();
        assert_eq!(client.peer_info().unwrap().server_info.as_ref().unwrap().name, "designos-mcp");
        let tools = client.list_all_tools().await.unwrap();
        let snapshot_raw = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots/tools.json"))
            .expect("read tests/snapshots/tools.json");
        let snapshot: Vec<Value> = serde_json::from_str(&snapshot_raw).expect("parse tools.json");
        assert_eq!(tools.len(), snapshot.len());
        assert!(tools.iter().any(|t| t.name == "designos_lint_fix"));
        assert!(tools.iter().all(|t| t.annotations.is_some() && t.output_schema.is_some()));
        let prompts = client.list_all_prompts().await.unwrap();
        assert_eq!(prompts.len(), 1);
        assert_eq!(prompts[0].name, "create_design_system_rules");
        let prompt_result = client.get_prompt(GetPromptRequestParams::new("create_design_system_rules")).await.unwrap();
        assert_eq!(prompt_result.messages.len(), 1);
        let call = |name: &'static str, args: Value| CallToolRequestParams::new(name).with_arguments(args.as_object().unwrap().clone());
        let libs = client.call_tool(call("designos_get_libraries", json!({}))).await.unwrap();
        assert_eq!(libs.is_error, Some(false));
        assert_eq!(libs.structured_content.unwrap()["libraries"][0]["id"], "local");
        let guidelines_list = client.call_tool(call("designos_get_guidelines", json!({}))).await.unwrap();
        assert_eq!(guidelines_list.is_error, Some(false));
        let guidelines_ds = client.call_tool(call("designos_get_guidelines", json!({"topic":"design-system"}))).await.unwrap();
        assert_eq!(guidelines_ds.is_error, Some(false));
        assert!(guidelines_ds.structured_content.unwrap()["content"].as_str().unwrap().contains("# Design system rules"));
        let searched = client.call_tool(call("designos_search_design_system", json!({"query":"button"}))).await.unwrap();
        assert_eq!(searched.is_error, Some(false));
        assert_eq!(searched.structured_content.unwrap()["libraries_searched"], json!(["local"]));
        let doc = client.call_tool(call("designos_get_document", json!({}))).await.unwrap();
        assert_eq!(doc.is_error, Some(false));
        let page = doc.structured_content.unwrap()["pages"][0]["node_id"].clone();
        let created = client.call_tool(call("designos_create_node", json!({"parent_id":page,"kind":"rectangle","name":"MCP test","width":120}))).await.unwrap();
        let id = created.structured_content.unwrap()["node"]["id"].clone();
        let shot = client.call_tool(call("designos_get_screenshot", json!({"node_id":id}))).await.unwrap();
        assert_eq!(shot.is_error, Some(false));
        assert!(shot.content.first().and_then(|c| c.as_image()).is_some());
        let shot_meta = shot.structured_content.unwrap();
        assert_eq!(shot_meta["width"], 60);
        assert_eq!(shot_meta["bounds"]["width"], 120.0);
        let invalid = client.call_tool(call("designos_update_node", json!({"node_id":id,"width":-1}))).await.unwrap();
        assert_eq!(invalid.is_error, Some(true));
        assert_eq!(invalid.structured_content.as_ref().unwrap()["error"]["code"], "VALUE_OUT_OF_RANGE");
        assert_eq!(invalid.structured_content.as_ref().unwrap()["error"]["safe_to_retry"], true);
        let code = client.call_tool(call("designos_generate_code", json!({"node_id":id,"language":"css"}))).await.unwrap();
        assert!(code.structured_content.unwrap()["code"].as_str().unwrap().contains("120px"));
        client.call_tool(call("designos_undo", json!({}))).await.unwrap();
        let gone = client.call_tool(call("designos_get_node", json!({"node_id":id}))).await.unwrap();
        assert_eq!(gone.is_error, Some(true));
        client.call_tool(call("designos_redo", json!({}))).await.unwrap();
        let restored = client.call_tool(call("designos_get_node", json!({"node_id":id}))).await.unwrap();
        assert_eq!(restored.is_error, Some(false));
        let applied = client
            .call_tool(call(
                "designos_apply_spec",
                json!({"spec":{"name":"Stdio Screen","preset":"easeui-card","children":[{"name":"Title","kind":"text","text":"Hello MCP"}]}}),
            ))
            .await
            .unwrap();
        assert_eq!(applied.is_error, Some(false));
        let batch_res = client
            .call_tool(call(
                "designos_apply_batch",
                json!({
                    "ops": [
                        {
                            "op": "apply_spec",
                            "as": "hero",
                            "parent": page,
                            "spec": {
                                "kind": "frame",
                                "name": "Hero",
                                "layout": { "direction": "column", "gap": 16 },
                                "children": [
                                    { "kind": "text", "name": "Title", "text": "Plan your week" }
                                ]
                            }
                        },
                        {
                            "op": "update",
                            "node": "@hero/Title",
                            "opacity": 0.9
                        }
                    ],
                    "dry_run": false
                }),
            ))
            .await
            .unwrap();
        assert_eq!(batch_res.is_error, Some(false));
        let batch_body = batch_res.structured_content.unwrap();
        assert_eq!(batch_body["created_count"], 2);
        assert!(batch_body["name_to_id"]["hero/Title"].as_u64().is_some());
        let imported = client
            .call_tool(call(
                "designos_import_web_capture",
                json!({
                    "capture": {
                        "schema": "designos-webcapture",
                        "version": 1,
                        "url": "https://designos.dev/mcp",
                        "title": "MCP Capture",
                        "viewport": { "width": 320.0, "height": 180.0, "dpr": 1.0 },
                        "scroll": { "width": 320.0, "height": 180.0 },
                        "root": {
                            "tag": "div",
                            "id": "card",
                            "box": { "x": 0.0, "y": 0.0, "width": 320.0, "height": 180.0 },
                            "styles": { "display": "flex", "flex-direction": "column", "row-gap": "12px" },
                            "children": []
                        }
                    }
                }),
            ))
            .await
            .unwrap();
        assert_eq!(imported.is_error, Some(false));
        let linted = client.call_tool(call("designos_lint", json!({"preset":"agent"}))).await.unwrap();
        assert_eq!(linted.is_error, Some(false));
        let lint_body = linted.structured_content.unwrap();
        assert!(lint_body["findings"].is_array());
        assert!(lint_body.get("passed").is_none());
        let comments = client.call_tool(call("designos_list_comments", json!({}))).await.unwrap();
        assert_eq!(comments.is_error, Some(false));
        client.cancel().await.unwrap();
    })
    .await
    .expect("MCP subprocess must finish within 30 seconds");
}
