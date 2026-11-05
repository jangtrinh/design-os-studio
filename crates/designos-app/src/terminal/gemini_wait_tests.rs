//! The card of a Gemini tab that waits, and the Stop control a Gemini tab does not get. Every tab here is a stub `gemini` spawn.
//! When a wait ends is in `gemini_dialog_tests`.

use super::gemini_test_support::*;
use super::*;
use crate::agent_permissions::AgentPermissionMode;

const SKIP: AgentPermissionMode = AgentPermissionMode::Skip;

#[test]
fn gemini_ask_tab_waiting_needs_approval_and_shows_open_tab() {
    let ctx = panel_ctx();
    let (dir, mut st) = start_gemini("card", ASK, |_| {});
    ask_permission(&mut st);
    assert_eq!(st.tabs[0].spawn_preset, AgentCliPreset::Gemini, "a real Gemini spawn, not a relabelled shell");
    assert!(st.needs_approval(), "an Ask-mode Gemini tab that waits needs approval");
    let painted = frame(&mut st, &ctx, false, vec![]);
    for expected in ["Gemini CLI needs approval in its terminal", "Open tab", "Needs approval"] {
        assert!(painted.iter().any(|t| t == expected), "{expected:?} is not painted: {painted:?}");
    }
    for absent in ["Approve", "Deny", "Select an agent tab"] {
        assert!(!painted.iter().any(|t| t == absent), "{absent:?} is painted: {painted:?}");
    }
    st.tabs[0].permission_mode = SKIP;
    assert!(!st.needs_approval(), "Skip mode raises no approval state");
    finish(dir, st);
}

#[test]
fn a_working_gemini_ask_tab_paints_no_stop_button() {
    let ctx = panel_ctx();
    let (dir, mut st) = start_gemini("stop", ASK, |_| {});
    let id = st.tabs[0].id;
    send_record(&mut st, id, "UserPromptSubmit", r#"{"session_id":"g","prompt":"x"}"#);
    send_record(&mut st, id, "PreToolUse", r#"{"session_id":"g","tool_name":"read_file"}"#);
    assert!(matches!(st.tabs[0].effective_status(), AgentTabStatus::Working(_)));
    // Gemini has no interrupt row (a second Ctrl+C exits it), so a Stop control would do nothing.
    assert!(crate::agent_stop::is_tab_turn_running(&st.tabs[0]), "the turn still counts as running for the undo bookkeeping");
    let painted = frame(&mut st, &ctx, true, vec![]);
    assert!(!painted.iter().any(|t| t == "Stop" || t == "Stop all"), "a Stop control is painted for Gemini: {painted:?}");
    assert_eq!(crate::agent_stop::running_tab_count(&st), 0);
    // Control: the same state for a CLI that has an interrupt row does paint it.
    st.tabs[0].spawn_preset = AgentCliPreset::Claude;
    let painted = frame(&mut st, &ctx, true, vec![]);
    assert!(painted.iter().any(|t| t == "Stop"), "the frame paints no Stop even for Claude: {painted:?}");
    assert_eq!(crate::agent_stop::running_tab_count(&st), 1);
    st.tabs[0].spawn_preset = AgentCliPreset::Gemini;
    finish(dir, st);
}
