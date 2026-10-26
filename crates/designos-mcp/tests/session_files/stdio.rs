//! The server as Studio starts it: a subprocess over stdio whose `DESIGNOS_SESSION_DIR` is the session folder.
use super::scratch::{assert_told, assert_untouched, link_sentence, not_plain_sentence, Scratch, VICTIM};
use rmcp::{model::CallToolRequestParams, transport::TokioChildProcess, ServiceExt};
use serde_json::{json, Value};
use std::time::Duration;

const SESSION_VARIABLES: [&str; 8] = [
    "DESIGNOS_LIVE_CANVAS",
    "DESIGNOS_LIVE_DOC_SYNC",
    "DESIGNOS_LIVE_DOC_PATCH",
    "DESIGNOS_LIVE_SPEC",
    "DESIGNOS_CONTEXT_JSON",
    "DESIGNOS_MCP_CONFIG",
    "DESIGNOS_MCP_TRANSCRIPT",
    "DESIGNOS_FILE",
];

fn call(name: &'static str, args: Value) -> CallToolRequestParams {
    CallToolRequestParams::new(name).with_arguments(args.as_object().unwrap().clone())
}

#[tokio::test]
async fn a_link_at_the_edit_log_and_a_fifo_at_the_context_are_refused_and_told() {
    tokio::time::timeout(Duration::from_secs(30), async {
        let scratch = Scratch::new();
        let victim = scratch.victim(VICTIM);
        let log = scratch.link("doc-patch.jsonl", &victim);
        let context = scratch.fifo("context.json");
        let mut command = tokio::process::Command::new(env!("CARGO_BIN_EXE_designos-mcp"));
        command.arg("--workspace").arg(&scratch.workspace).env("DESIGNOS_SESSION_DIR", &scratch.session);
        for variable in SESSION_VARIABLES {
            command.env_remove(variable);
        }
        let client = ().serve(TokioChildProcess::new(command).unwrap()).await.unwrap();

        let created = client.call_tool(call("designos_new_document", json!({"name": "Probe", "discard_changes": true}))).await.unwrap();
        assert_eq!(created.is_error, Some(true));
        let error = created.structured_content.unwrap();
        let message = error["error"]["message"].as_str().unwrap();
        assert!(message.contains(&link_sentence(&log)), "{message}");

        let document = client.call_tool(call("designos_get_document", json!({}))).await.unwrap();
        assert_eq!(document.is_error, Some(false));
        let reply = document.structured_content.unwrap();
        assert_told(&reply, &link_sentence(&log));
        assert_told(&reply, &not_plain_sentence(&context));
        assert_untouched(&victim, VICTIM, "the edit log");
        client.cancel().await.unwrap();
    })
    .await
    .expect("designos-mcp did not answer within 30 s");
}
