//! Gemini's confirmation dialog on the tab's own screen is the source of truth for a wait: queued dialogs, the wait ending and
//! reopening, and a tab with no hooks file. The screen is drawn by the tests, as the CLI would draw it.

use super::gemini_test_support::*;
use super::*;
use crate::agent_composer::SendOutcome;
use crate::gemini_status_hooks::{self as hooks, SYSTEM_DEFAULTS_ENV};
use egui::{Event, Key, Modifiers};
use std::time::Duration;

#[test]
fn a_second_queued_dialog_keeps_the_wait_and_the_composer_closed() {
    let ctx = panel_ctx();
    let (dir, mut st) = start_gemini("queued", ASK, |_| {});
    show(&mut st, 0, &dialog_screen(1, 2));
    ask_permission(&mut st);
    ask_permission(&mut st);
    assert!(waiting(&st, 0) && st.needs_approval());
    // The user answers dialog 1 of 2 with Enter, and Gemini draws dialog 2 of 2: no hook fires between them.
    frame(&mut st, &ctx, true, vec![key(Key::Enter, Modifiers::NONE)]);
    show(&mut st, 0, &dialog_screen(2, 2));
    poll(&mut st);
    assert!(waiting(&st, 0) && st.needs_approval(), "one answer of two ended the wait");
    assert_eq!(send(&mut st), REFUSED, "the composer sent while dialog 2 of 2 was open");
    // A digit does not end a wait either, whatever it selects.
    frame(&mut st, &ctx, true, vec![Event::Text("4".into())]);
    poll(&mut st);
    assert!(waiting(&st, 0), "a digit ended the wait with the dialog still on screen");
    assert_eq!(send(&mut st), REFUSED);
    finish(dir, st);
}

#[test]
fn a_hook_wait_ends_when_the_dialog_is_gone_and_the_composer_then_sends() {
    let ctx = panel_ctx();
    let (dir, mut st) = start_gemini("gone", ASK, |_| {});
    show(&mut st, 0, &dialog_screen(1, 1));
    ask_permission(&mut st);
    assert!(waiting(&st, 0));
    // Esc is the answer No, but the screen decides: while the dialog is drawn the wait stands.
    frame(&mut st, &ctx, true, vec![key(Key::Escape, Modifiers::NONE)]);
    assert!(waiting(&st, 0), "Esc ended the wait with the dialog still on screen");
    show(&mut st, 0, CLEAR);
    poll(&mut st);
    assert!(waiting(&st, 0), "a single frame without the dialog ended the wait");
    std::thread::sleep(QUIET);
    poll(&mut st);
    assert_eq!(st.tabs[0].effective_status(), AgentTabStatus::Idle);
    assert!(!matches!(send(&mut st), SendOutcome::Refused(_)), "the composer still refuses after the dialog is gone");
    finish(dir, st);
}

#[test]
fn a_dialog_that_returns_before_a_tool_record_reopens_the_wait() {
    let (dir, mut st) = start_gemini("reopen", ASK, |_| {});
    let id = st.tabs[0].id;
    show(&mut st, 0, &dialog_screen(1, 2));
    ask_permission(&mut st);
    dialog_goes(&mut st, 0);
    assert_eq!(st.tabs[0].effective_status(), AgentTabStatus::Idle, "a gap longer than the debounce ends the wait");
    // Gemini draws dialog 2 of 2 after the gap: the same wait, since no tool record has ended it.
    show(&mut st, 0, &dialog_screen(2, 2));
    poll(&mut st);
    assert!(waiting(&st, 0) && st.needs_approval(), "the second dialog of the queue is not a wait");
    assert_eq!(send(&mut st), REFUSED);
    // A tool record ends the wait for good; a dialog on screen after that starts none, but the composer stays shut.
    send_record(&mut st, id, "PreToolUse", r#"{"session_id":"g"}"#);
    poll(&mut st);
    assert!(!waiting(&st, 0), "the tool record left the tab waiting");
    assert_eq!(send(&mut st), REFUSED, "the composer sent into an open dialog");
    finish(dir, st);
}

#[test]
fn with_no_hooks_file_only_the_dialog_on_screen_starts_a_wait() {
    let none_such = std::env::temp_dir().join(format!("designos-gemini-dialog-admin-{}", std::process::id()));
    let (dir, mut st) =
        hooks::with_system_config(&[(SYSTEM_DEFAULTS_ENV, "/admin/defaults.json")], &none_such, || start_stub("prose", ASK, STUB_PROSE, |_| {}));
    assert!(hook_files(&st).is_empty(), "this tab has no hooks file");
    let printed = st.session_dir.join("printed.txt");
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while !printed.is_file() {
        assert!(std::time::Instant::now() < deadline, "the stub never printed");
        std::thread::sleep(Duration::from_millis(25));
    }
    std::thread::sleep(Duration::from_millis(300));
    poll(&mut st);
    // Model prose with "Allow " and "(y/n)" is not a dialog.
    assert!(!waiting(&st, 0), "model prose raised a wait: {:?}", st.tabs[0].effective_status());
    assert!(!matches!(send(&mut st), SendOutcome::Refused(_)), "model prose locked the composer");
    show(&mut st, 0, &dialog_screen(1, 1));
    poll(&mut st);
    assert!(waiting(&st, 0) && st.needs_approval(), "the dialog on screen raised no wait");
    dialog_goes(&mut st, 0);
    assert!(!waiting(&st, 0), "the wait outlived the dialog");
    finish(dir, st);
}

#[test]
fn a_prompt_quoting_the_option_label_is_not_a_dialog() {
    let (dir, mut st) = start_gemini("own-prompt", ASK, |_| {});
    // The user's prompt can contain the same words as Gemini's last option. It must remain
    // sendable; only a numbered option row is a confirmation dialog.
    show(&mut st, 0, "No, suggest changes to the header instead\r\n");
    poll(&mut st);
    assert!(!st.tabs[0].gemini_dialog_open(), "ordinary prompt text was treated as a Gemini dialog");
    assert!(!matches!(send(&mut st), SendOutcome::Refused(_)), "ordinary prompt text locked the composer");
    finish(dir, st);
}
