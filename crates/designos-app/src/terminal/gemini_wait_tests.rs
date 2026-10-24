//! A Gemini tab's wait: how it ends when the user answers Gemini's own dialog (Gemini fires no hook when the answer is No),
//! the card of a real Gemini tab, and the Stop control a Gemini tab does not get. Every tab here is a stub `gemini` spawn.

use super::gemini_test_support::*;
use super::*;
use crate::agent_composer::{RefuseReason, SendOutcome};
use crate::agent_permissions::AgentPermissionMode;
use crate::gemini_status_hooks::{self as hooks, SYSTEM_DEFAULTS_ENV};
use egui::{Event, Key, Modifiers};

const ASK: AgentPermissionMode = AgentPermissionMode::Ask;
const SKIP: AgentPermissionMode = AgentPermissionMode::Skip;

fn waiting(st: &TerminalState) -> bool {
    matches!(st.tabs[0].effective_status(), AgentTabStatus::WaitingForInput(_))
}

fn send(st: &mut TerminalState) -> SendOutcome {
    st.send_to_agent("make it blue", None)
}

fn text(s: &str) -> Event {
    Event::Text(s.to_string())
}

#[test]
fn a_key_that_answers_geminis_dialog_ends_a_hook_wait_and_other_keys_do_not() {
    let ctx = panel_ctx();
    let (dir, mut st) = start_gemini("answer", ASK, |_| {});
    ask_permission(&mut st);
    assert!(waiting(&st) && st.needs_approval());
    // The composer guard stays: a prompt pasted into the open dialog could pick one of its options.
    assert_eq!(send(&mut st), SendOutcome::Refused(RefuseReason::Permission));
    // Keys that only move around the dialog, other text, Alt+Enter and a paste leave the wait.
    let (none, shift) = (Modifiers::NONE, Modifiers::SHIFT);
    let moves =
        [key(Key::ArrowDown, none), key(Key::ArrowUp, none), key(Key::Tab, none), text("j"), text("9"), key(Key::Enter, shift), Event::Paste("1\n".into())];
    for event in moves {
        frame(&mut st, &ctx, true, vec![event.clone()]);
        assert!(waiting(&st), "{event:?} ended the wait");
    }
    // Esc, Ctrl+C (cancel), Enter (the highlighted option) and an option number answer it.
    let ctrl = Modifiers { ctrl: true, ..Modifiers::NONE };
    for (name, event) in [("Esc", key(Key::Escape, none)), ("Ctrl+C", key(Key::C, ctrl)), ("Enter", key(Key::Enter, none)), ("1", text("1")), ("3", text("3"))]
    {
        frame(&mut st, &ctx, true, vec![event]);
        assert!(!waiting(&st) && !st.needs_approval(), "{name} left the wait");
        ask_permission(&mut st);
        assert!(waiting(&st), "{name}: the tab was not waiting again: {:?}", st.tabs[0].effective_status());
    }
    // The key went to the PTY as well, and once the wait is over the composer sends.
    frame(&mut st, &ctx, true, vec![key(Key::Escape, none)]);
    assert!(st.tabs[0].written_bytes.contains(&0x1b), "Esc never reached Gemini's terminal");
    assert_eq!(st.tabs[0].effective_status(), AgentTabStatus::Idle);
    assert!(!matches!(send(&mut st), SendOutcome::Refused(_)), "the composer still refuses after the answer");
    finish(dir, st);
}

#[test]
fn the_output_check_wait_of_a_gemini_tab_with_no_hooks_ends_on_an_answer_too() {
    let ctx = panel_ctx();
    let none_such = std::env::temp_dir().join(format!("designos-gemini-wait-admin-{}", std::process::id()));
    let (dir, mut st) = hooks::with_system_config(&[(SYSTEM_DEFAULTS_ENV, "/admin/defaults.json")], &none_such, || {
        start_stub("fallback", ASK, STUB_PROMPT_THEN_SPINNER, |_| {})
    });
    assert!(st.tabs[0].pty_waiting_heuristic.load(std::sync::atomic::Ordering::Acquire), "no hooks file: the output check is on");
    let printed = st.session_dir.join("printed.txt");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while !printed.is_file() {
        assert!(std::time::Instant::now() < deadline, "the stub never printed");
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
    std::thread::sleep(std::time::Duration::from_millis(300));
    let _ = st.poll_live_agent_bridge();
    // Gemini's own spinner after the dialog does not end it: the output check only ever raises the wait.
    assert!(waiting(&st), "the dialog's text raised no wait: {:?}", st.tabs[0].effective_status());
    assert_eq!(send(&mut st), SendOutcome::Refused(RefuseReason::Permission));
    frame(&mut st, &ctx, true, vec![key(Key::Enter, Modifiers::NONE)]);
    assert!(!waiting(&st) && !st.needs_approval());
    assert!(!matches!(send(&mut st), SendOutcome::Refused(_)));
    finish(dir, st);
}

#[test]
fn gemini_ask_tab_waiting_needs_approval_and_shows_open_tab() {
    let ctx = panel_ctx();
    let (dir, mut st) = start_gemini("card", ASK, |_| {});
    ask_permission(&mut st);
    assert_eq!(st.tabs[0].spawn_preset, AgentCliPreset::Gemini, "a real Gemini spawn, not a relabelled shell");
    assert!(st.needs_approval(), "an Ask-mode Gemini tab that waits needs approval");
    let painted = frame(&mut st, &ctx, false, vec![]);
    for expected in ["Gemini CLI needs approval in its terminal", "Open tab", "Needs approval", "Ask for changes"] {
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
