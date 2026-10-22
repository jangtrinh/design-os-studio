//! Replay, transcript and MCP-client tests: every call goes through a real `designos-mcp` process (E15-25 §3.10, §3.11).

mod support;

use designos_eval::mcp_client::McpClient;
use designos_eval::runner::{replay_transcript_file, rfc3339_utc, run_harness};
use designos_eval::transcript::{had_document_edits, TranscriptLine};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::fs;
use support::*;

const STUB_COMMAND: &str = r#"["stub-agent", "--script", "{workspace}/../script.jsonl", "--mcp-config", "{mcp_config}"]"#;

fn write_transcript(path: &std::path::Path, calls: &[(&str, Value, Option<Value>, usize)]) {
    let mut out = String::new();
    for (i, (tool, args, result, undo_depth)) in calls.iter().enumerate() {
        let mut line = json!({"seq": i + 1, "t_ms": 1, "tool": tool, "args": args, "is_error": result.is_none(), "undo_depth": undo_depth});
        match result {
            Some(r) => line["result"] = r.clone(),
            None => line["error"] = json!("recorded failure"),
        }
        out.push_str(&format!("{}\n", serde_json::to_string(&line).unwrap()));
    }
    fs::write(path, out).unwrap();
}

#[test]
fn replay_reproduces_metrics_of_the_original_run() {
    let root = temp_test_dir("replay-repro");
    write_register(&root);
    let mut t = task("s2-replay", "s2");
    t.stub_script = Some("script.jsonl".to_string());
    let suites_root = write_suite(&root, "s2", vec![t.clone()], &[("script.jsonl", script(&[create_hero_frame_call(), save_output_call()]))]);
    write_profile(&root, "stub", STUB_COMMAND);
    run_harness(&run_options(&root, suites_root.clone(), "s2", "stub", 1)).unwrap();
    let original = read_run_records(&root, "s2").remove(0);
    let transcript = root.join("results/v0.6.0/s2-replay/1/transcript.jsonl");

    let suite_dir = suites_root.join("s2");
    let r1 = replay_transcript_file(&mcp_bin(), &transcript, Some((&t, suite_dir.as_path())), &root.join("replay1")).unwrap();
    let r2 = replay_transcript_file(&mcp_bin(), &transcript, Some((&t, suite_dir.as_path())), &root.join("replay2")).unwrap();

    assert_eq!(r1.metrics.len(), r2.metrics.len());
    for (k, v1) in &r1.metrics {
        let v2 = r2.metrics.get(k).expect("same metric key");
        assert_eq!(v1.to_bits(), v2.to_bits(), "metric '{k}' must be bit-identical across replays ({v1} vs {v2})");
        let recorded = original["metrics"][k].as_f64().unwrap_or_else(|| panic!("run record lacks metric {k}"));
        assert_eq!(v1.to_bits(), recorded.to_bits(), "metric '{k}' on replay must equal the recorded run ({v1} vs {recorded})");
    }
    assert!(!r1.render_rel_paths.is_empty());
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn replay_runs_in_a_sandbox_prepared_like_the_run() {
    let root = temp_test_dir("replay-sandbox");
    let mut t = task("s2-start", "s2");
    t.start = "start.designos".to_string();
    t.output = Vec::new();
    let suites_root = write_suite(&root, "s2", vec![t.clone()], &[]);
    write_start_document(&suites_root.join("s2").join("start.designos"));

    // First call opens the start document, which only exists in the replay workspace if it was prepared.
    let transcript = root.join("transcript.jsonl");
    write_transcript(
        &transcript,
        &[("designos_open_file", json!({"path": "start.designos"}), Some(json!({})), 0), ("designos_get_document", json!({}), Some(json!({})), 0)],
    );
    let suite_dir = suites_root.join("s2");
    let evaluated = replay_transcript_file(&mcp_bin(), &transcript, Some((&t, suite_dir.as_path())), &root.join("replay"))
        .expect("open_file of the start document replays");
    assert_eq!(evaluated.metrics["render_success"], 0.0, "nothing was edited");
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn replay_follows_ids_and_creates_after_apply_spec_do_not_collide() {
    let root = temp_test_dir("replay-ids");
    let transcript = root.join("transcript.jsonl");
    let apply = create_hero_frame_call();
    // create_node after apply_spec used to reuse an id that apply_spec had already handed out.
    write_transcript(
        &transcript,
        &[
            (apply["tool"].as_str().unwrap(), apply["args"].clone(), Some(json!({})), 1),
            ("designos_create_node", json!({"kind": "rectangle", "name": "After spec", "width": 40, "height": 40}), Some(json!({})), 2),
        ],
    );
    let evaluated = replay_transcript_file(&mcp_bin(), &transcript, None, &root.join("replay")).expect("both calls replay on the real engine");
    assert_eq!(evaluated.metrics["render_success"], 1.0);
    let saved = fs::read_to_string(root.join("replay/workspace/output.designos")).unwrap();
    assert!(saved.contains("Hero Frame") && saved.contains("After spec"), "both nodes exist, so no id was reused");
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn replay_stops_with_replay_diverged_when_a_recorded_success_now_fails() {
    let root = temp_test_dir("replay-diverged");
    let transcript = root.join("transcript.jsonl");
    write_transcript(&transcript, &[("designos_get_node", json!({"node_id": 987_654_321u64}), Some(json!({})), 0)]);
    let err = replay_transcript_file(&mcp_bin(), &transcript, None, &root.join("replay")).expect_err("the node does not exist on a fresh engine");
    assert_eq!(err.code, 3);
    assert!(err.message.starts_with("replay_diverged: seq 1"), "{}", err.message);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn transcript_logging_failure_fails_the_call() {
    let root = temp_test_dir("transcript-fail");
    let ws = root.join("ws");
    fs::create_dir_all(&ws).unwrap();
    // A regular file where the transcript's parent directory should be, so appending cannot work.
    let blocker = root.join("not_a_dir");
    fs::write(&blocker, "blocker").unwrap();
    let env = BTreeMap::from([("DESIGNOS_MCP_TRANSCRIPT".to_string(), blocker.join("transcript.jsonl").to_string_lossy().to_string())]);

    let mut client = McpClient::spawn(&mcp_bin(), &["--workspace".to_string(), ws.to_string_lossy().to_string()], &env, &[]).unwrap();
    let out = client.call_tool("designos_get_document", &json!({})).unwrap();
    assert!(out.is_error, "failing to append the transcript line must fail the tool call");
    assert!(out.error_message().is_some_and(|m| m.starts_with("Transcript logging failed")), "{:?}", out.value);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn mcp_client_reports_a_failing_tool_as_a_result_not_a_transport_error() {
    let root = temp_test_dir("mcp-client");
    let mut client = McpClient::spawn(&mcp_bin(), &["--workspace".to_string(), root.to_string_lossy().to_string()], &BTreeMap::new(), &[]).unwrap();
    let ok = client.call_tool("designos_get_document", &json!({})).unwrap();
    assert!(!ok.is_error);
    assert!(ok.value["pages"].is_array(), "{:?}", ok.value);
    let bad = client.call_tool("designos_get_node", &json!({"node_id": 987_654_321u64})).unwrap();
    assert!(bad.is_error);
    let _ = fs::remove_dir_all(&root);
}

fn line(undo_depth: usize, is_error: bool) -> TranscriptLine {
    TranscriptLine { seq: 1, t_ms: 0, tool: "t".to_string(), args: Value::Null, result: None, error: None, is_error, undo_depth }
}

#[test]
fn only_a_growing_undo_stack_counts_as_an_edit() {
    assert!(!had_document_edits(&[]));
    assert!(!had_document_edits(&[line(0, false), line(0, false)]), "reads and opens leave the history empty");
    assert!(had_document_edits(&[line(0, false), line(1, false)]));
    assert!(!had_document_edits(&[line(0, false), line(1, true)]), "a failed call changed nothing");
    assert!(had_document_edits(&[line(3, false), line(0, false), line(1, false)]), "an edit after opening a file counts");
}

#[test]
fn started_timestamp_is_rfc3339_utc() {
    assert_eq!(rfc3339_utc(0), "1970-01-01T00:00:00Z");
    assert_eq!(rfc3339_utc(1_000_000_000), "2001-09-09T01:46:40Z");
    assert_eq!(rfc3339_utc(951_782_400 + 86_399), "2000-02-29T23:59:59Z");
    assert_eq!(rfc3339_utc(1_792_368_000), "2026-10-19T00:00:00Z");
}
