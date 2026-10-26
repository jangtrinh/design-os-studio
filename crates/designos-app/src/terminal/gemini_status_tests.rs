//! Gemini CLI in Ask mode: routing of its tagged hook records, the turn states they drive, and the shape of the hooks file.
//! The tab and the launch side are in `gemini_launch_tests`.

use super::*;
use crate::agent_notify::NotifyKind;
use crate::agent_turn_status::{route_record_to_tab, AgentEventRecord, TabAttributionInfo, TabTurnTracker};
use serde_json::json;
use std::io::Write;
use std::path::Path;

/// FNV-1a 64 of a JSON value, the way the other hook files are pinned.
fn pin(value: &serde_json::Value) -> u64 {
    crate::agent_sessions::fnv1a_64(value.to_string().as_bytes())
}

fn tab(idx: usize, tab_id: usize, preset: AgentCliPreset) -> TabAttributionInfo {
    TabAttributionInfo { idx, tab_id, preset, is_alive: true, session_uuid: None }
}

fn record(event: &str, tag: Option<&str>, payload: serde_json::Value) -> AgentEventRecord {
    AgentEventRecord { t: 10.0, e: event.to_string(), tab: tag.map(str::to_string), p: payload }
}

/// What Gemini's `Notification` hook prints on stdin when a tool asks for confirmation.
fn tool_permission(tag: Option<&str>) -> AgentEventRecord {
    record("Notification", tag, json!({ "session_id": "gem-1", "hook_event_name": "Notification", "notification_type": "ToolPermission" }))
}

#[test]
fn the_claude_grok_and_agy_status_hooks_stay_byte_identical() {
    let status = Path::new("/s/agent-status.json");
    let one = status_hook_command(status, "PreToolUse", "working");
    assert_eq!(
        one,
        r#"{ p=$(tr '\r\n' '  '); [ -z "$p" ] && p=null; line=$(printf '{"t":%s,"e":"PreToolUse","p":%s}' "$(date +%s)" "$p"); got_lock=0; i=0; while [ $i -lt 500 ]; do if mkdir '/s/agent-events.jsonl.lock' 2>/dev/null; then got_lock=1; break; fi; sleep 0.01; i=$((i+1)); done; printf '%s\n' "$line" >> '/s/agent-events.jsonl'; if [ "$got_lock" = 1 ]; then rmdir '/s/agent-events.jsonl.lock' 2>/dev/null || true; fi; }; printf '{"state":"working","run_id":"%s"}\n' "${DESIGNOS_AGENT_RUN_ID:-}" > '/s/agent-status.json'"#
    );
    let claude = session_hooks_settings_with_approval(status, Some((Path::new("/s/page.json"), Path::new("/s/boot.json"), Path::new("/s/marker"))), None, None);
    assert_eq!(pin(&claude), 9571181679558406141, "claude");
    assert_eq!(pin(&session_hooks_settings(status, None, None)), 1590823247331823233, "grok");
    assert_eq!(pin(&agy_session_hooks(status)), 17132879456753940662, "agy");
}

#[test]
fn gemini_tool_permission_record_routes_to_its_tab_beside_a_claude_tab() {
    let tabs = [tab(0, 1, AgentCliPreset::Claude), tab(1, 2, AgentCliPreset::Gemini)];
    assert_eq!(route_record_to_tab(&tool_permission(Some("2")), &tabs), Some(1));
    // The tag is the tab's id, not its place in the strip.
    let reversed = [tab(0, 2, AgentCliPreset::Gemini), tab(1, 1, AgentCliPreset::Claude)];
    assert_eq!(route_record_to_tab(&tool_permission(Some("2")), &reversed), Some(0));
    // A tag that names a Claude tab, a tab that is gone, or a Gemini tab that exited goes nowhere, never through the untagged rules.
    assert_eq!(route_record_to_tab(&tool_permission(Some("1")), &tabs), None);
    assert_eq!(route_record_to_tab(&tool_permission(Some("9")), &tabs), None);
    let mut exited = tabs.clone();
    exited[1].is_alive = false;
    assert_eq!(route_record_to_tab(&tool_permission(Some("2")), &exited), None);
}

#[test]
fn agy_records_still_route_with_a_gemini_tab_open() {
    let with_gemini = [tab(0, 1, AgentCliPreset::Antigravity), tab(1, 2, AgentCliPreset::Gemini)];
    for rec in [record("PreInvocation", None, json!({})), record("Stop", None, json!({})), record("Stop", None, json!({ "conversationId": "c-1" }))] {
        assert_eq!(route_record_to_tab(&rec, &with_gemini), Some(0), "agy {} record", rec.e);
    }
    let three = [tab(0, 1, AgentCliPreset::Claude), tab(1, 2, AgentCliPreset::Gemini), tab(2, 3, AgentCliPreset::Grok)];
    assert_eq!(route_record_to_tab(&record("PreToolUse", None, json!({ "session_id": "c", "tool_name": "Read" })), &three), Some(0));
    assert_eq!(route_record_to_tab(&record("PreToolUse", None, json!({ "sessionId": "g", "toolName": "Read" })), &three), Some(2));
}

#[test]
fn an_untagged_record_never_lands_on_a_gemini_tab() {
    let only = [tab(0, 1, AgentCliPreset::Gemini)];
    for rec in [
        record("PreToolUse", None, json!({ "session_id": "s", "tool_name": "Read" })),
        record("Stop", None, json!({})),
        record("PreInvocation", None, json!({})),
    ] {
        assert_eq!(route_record_to_tab(&rec, &only), None, "untagged {} record", rec.e);
    }
    let beside_claude = [tab(0, 1, AgentCliPreset::Gemini), tab(1, 2, AgentCliPreset::Claude)];
    assert_eq!(route_record_to_tab(&record("Stop", None, json!({})), &beside_claude), Some(1));
}

#[test]
fn a_tool_permission_notification_waits_for_approval() {
    let effect = TabTurnTracker::default().on_event(&tool_permission(Some("2")));
    assert!(matches!(effect.new_status, Some(AgentTabStatus::WaitingForInput(_))), "{:?}", effect.new_status);
    assert_eq!(effect.notify, Some(NotifyKind::NeedsApproval));
    let other = record("Notification", Some("2"), json!({ "notification_type": "Info" }));
    assert!(TabTurnTracker::default().on_event(&other).new_status.is_none(), "any other notification type changes nothing");
}

#[test]
fn the_status_hooks_cover_five_events_name_the_tab_and_print_nothing() {
    let session = std::env::temp_dir().join(format!("designos-gemini-hooks-json-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&session);
    std::fs::create_dir_all(&session).unwrap();
    let settings = crate::gemini_status_hooks::hooks_settings(&session, 7);
    let hooks = settings["hooks"].as_object().expect("hooks");
    let mut events: Vec<&str> = hooks.keys().map(String::as_str).collect();
    events.sort_unstable();
    assert_eq!(events, ["AfterAgent", "AfterTool", "BeforeAgent", "BeforeTool", "Notification"]);
    for (event, groups) in hooks {
        let handler = &groups[0]["hooks"][0];
        assert_eq!(
            (&groups[0]["matcher"], &handler["type"], &handler["name"], &handler["timeout"]),
            (&json!("*"), &json!("command"), &json!("designos-status"), &json!(5000)),
            "{event}"
        );
    }

    // Run the Notification handler the way Gemini does: the payload on stdin, through a shell.
    let command = hooks["Notification"][0]["hooks"][0]["command"].as_str().unwrap();
    let mut child =
        std::process::Command::new("sh").arg("-c").arg(command).stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).spawn().unwrap();
    child.stdin.take().unwrap().write_all(br#"{"session_id":"gem-1","notification_type":"ToolPermission"}"#).unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success() && out.stdout.is_empty(), "the handler prints nothing: {:?}", String::from_utf8_lossy(&out.stdout));
    let written = std::fs::read_to_string(session.join("agent-events.jsonl")).unwrap();
    let rec: AgentEventRecord = serde_json::from_str(written.trim()).expect("a record");
    assert_eq!((rec.e.as_str(), rec.tab.as_deref(), rec.notification_type()), ("Notification", Some("7"), Some("ToolPermission")));
    let _ = std::fs::remove_dir_all(&session);
}
