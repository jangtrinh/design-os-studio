//! Gemini's confirmation dialog on the tab's own screen is the source of truth for the composer: a small screen, a tab nobody is
//! looking at, the delayed Enter, an edit in the external editor, and a dialog that never draws the marker.

use super::gemini_test_support::*;
use super::*;
use crate::agent_composer::SendOutcome;
use std::time::Duration;

#[test]
fn the_dialog_is_found_on_an_80_by_20_screen_whatever_the_status_says() {
    let (dir, mut st) = start_gemini("small", ASK, |_| {});
    st.tabs[0].resize(20, 80);
    show(&mut st, 0, &dialog_screen(1, 2));
    assert_eq!(st.tabs[0].effective_status(), AgentTabStatus::Idle);
    assert_eq!(send(&mut st), REFUSED, "an open dialog on an 80x20 screen was missed");
    show(&mut st, 0, CLEAR);
    assert!(!matches!(send(&mut st), SendOutcome::Refused(_)), "an empty screen refused a send");
    finish(dir, st);
}

#[test]
fn a_tab_nobody_is_looking_at_keeps_its_queued_prompt_while_its_dialog_is_open() {
    let (dir, mut st) = start_gemini("hidden", ASK, |_| {});
    st.spawn_tab(AgentCliPreset::Shell, None).expect("spawn a second tab");
    assert_eq!(st.active_tab, 1, "the shell tab is the one on show");
    st.tabs[0].spawned_at = std::time::Instant::now() - Duration::from_secs(60);
    show(&mut st, 0, &dialog_screen(1, 1));
    st.tabs[0].composer.pending_send = Some("queued while the dialog is open".to_string());
    st.tick_composer();
    assert!(st.tabs[0].composer.pending_send.is_some(), "a queued prompt was pasted into an open dialog of a hidden tab");
    // The dialog goes: the hidden tab's screen is current, and the prompt goes out.
    dialog_goes(&mut st, 0);
    st.tick_composer();
    assert!(st.tabs[0].composer.pending_send.is_none(), "the prompt stayed queued after the dialog was gone");
    // A hook wait on the hidden tab shows in its status.
    show(&mut st, 0, &dialog_screen(1, 1));
    ask_permission(&mut st);
    assert!(waiting(&st, 0) && st.active_tab == 1);
    finish(dir, st);
}

#[test]
fn the_delayed_enter_is_not_sent_when_a_dialog_opens_after_the_paste() {
    let (dir, mut st) = start_gemini("enter", ASK, |_| {});
    assert!(matches!(send(&mut st), SendOutcome::Written));
    assert!(st.tabs[0].composer.pending_enter_deadline.is_some());
    show(&mut st, 0, &dialog_screen(1, 1));
    let mark = st.tabs[0].written_bytes.len();
    st.flush_pending_enter_now();
    assert!(!st.tabs[0].written_bytes[mark..].contains(&b'\r'), "Enter went into the dialog that opened after the paste");
    assert_eq!(st.tabs[0].composer.last_outcome, Some(REFUSED));
    // The same through the frame tick.
    show(&mut st, 0, CLEAR);
    assert!(matches!(send(&mut st), SendOutcome::Written));
    show(&mut st, 0, &dialog_screen(1, 1));
    st.tabs[0].composer.pending_enter_deadline = Some(std::time::Instant::now() - Duration::from_millis(1));
    let mark = st.tabs[0].written_bytes.len();
    st.tick_composer();
    assert!(!st.tabs[0].written_bytes[mark..].contains(&b'\r'), "the tick sent Enter into an open dialog");
    finish(dir, st);
}

/// Writes the paste, then lets `waits` make the agent wait on the user, then flushes the delayed Enter of tab `tab`. True when the
/// Enter reached the tab's terminal.
fn enter_written_after(st: &mut TerminalState, tab: usize, waits: impl FnOnce(&mut TerminalState)) -> bool {
    assert!(matches!(st.send_to_agent("make it blue", None), SendOutcome::Written));
    assert!(st.tabs[tab].composer.pending_enter_deadline.is_some(), "the send left no Enter to write");
    waits(st);
    let mark = st.tabs[tab].written_bytes.len();
    st.flush_pending_enter_now();
    st.tabs[tab].written_bytes[mark..].contains(&b'\r')
}

#[test]
fn the_delayed_enter_is_not_sent_when_a_record_says_the_agent_waits_whatever_the_version() {
    for version in [None, Some("0.36.0"), Some("0.35.3")] {
        crate::gemini_dialog::with_gemini_version(version, || {
            let (dir, mut st) = start_gemini("enter-record", ASK, |_| {});
            // The record arrives between the paste and the Enter, and no dialog is drawn.
            let written = enter_written_after(&mut st, 0, ask_permission);
            assert!(!written, "{version:?}: Enter went to an agent that began to wait after the paste");
            assert_eq!(st.tabs[0].composer.last_outcome, Some(REFUSED));
            assert!(st.tabs[0].composer.notice.is_some(), "the refusal gave no notice");
            finish(dir, st);
        });
    }
}

#[test]
fn the_delayed_enter_is_not_sent_to_a_claude_tab_that_began_to_wait_after_the_paste() {
    let (dir, mut st) = start_gemini("enter-claude", ASK, |_| {});
    let claude = dir.join("bin/claude");
    std::fs::write(&claude, "#!/bin/sh\ntrap '' INT\nsleep 30\n").unwrap();
    std::fs::set_permissions(&claude, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    st.spawn_tab(AgentCliPreset::Claude, None).expect("spawn claude");
    st.tabs[1].spawned_at = std::time::Instant::now() - Duration::from_secs(60);
    assert_eq!((st.active_tab, st.tabs[1].spawn_preset), (1, AgentCliPreset::Claude));
    let wait = |st: &mut TerminalState| *st.tabs[1].agent_status.lock().unwrap() = AgentTabStatus::WaitingForInput(Some("Needs approval".into()));
    assert!(!enter_written_after(&mut st, 1, wait), "Enter went to a Claude tab that began to wait after the paste");
    assert_eq!(st.tabs[1].composer.last_outcome, Some(REFUSED));
    // Control: the same send to the same tab, with nothing waiting, does write its Enter.
    *st.tabs[1].agent_status.lock().unwrap() = AgentTabStatus::Idle;
    assert!(enter_written_after(&mut st, 1, |_| {}), "a Claude tab that waits for nothing got no Enter");
    finish(dir, st);
}

#[test]
fn a_dialog_with_no_options_on_screen_keeps_a_wait_and_never_starts_one() {
    let (dir, mut st) = start_gemini("modify", ASK, |_| {});
    let editor = format!("{CLEAR}Modify in progress: Save and close external editor to continue");
    show(&mut st, 0, &editor);
    poll(&mut st);
    assert!(!waiting(&st, 0), "an edit open in the external editor started a wait");
    show(&mut st, 0, &dialog_screen(1, 1));
    ask_permission(&mut st);
    // The user picks "Modify with external editor": the options go, the editor line stays.
    show(&mut st, 0, &editor);
    std::thread::sleep(QUIET);
    poll(&mut st);
    assert!(waiting(&st, 0), "the wait ended while the edit was still pending");
    dialog_goes(&mut st, 0);
    assert!(!waiting(&st, 0));
    finish(dir, st);
}

#[test]
fn a_dialog_that_draws_no_marker_holds_its_wait_until_a_hook_record() {
    let (dir, mut st) = start_gemini("holds", ASK, |_| {});
    let id = st.tabs[0].id;
    ask_permission(&mut st);
    std::thread::sleep(QUIET * 2);
    poll(&mut st);
    assert!(waiting(&st, 0), "a wait with no marker on screen ended without a record");
    send_record(&mut st, id, "PreToolUse", r#"{"session_id":"g"}"#);
    assert!(matches!(st.tabs[0].effective_status(), AgentTabStatus::Working(_)));
    finish(dir, st);
}
