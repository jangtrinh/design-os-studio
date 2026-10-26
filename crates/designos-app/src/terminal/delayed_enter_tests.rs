//! The Enter that submits a pasted prompt is written a moment after the paste. It is held back when the agent began to wait on the user in
//! between, because that Enter would answer the question: a Gemini dialog on screen, or a status set by hook records. A status read from
//! the CLI's output does not hold it: the echo of the pasted prompt can set that one, and it is no question. A send that finds the agent
//! already waiting is refused whatever the status came from. Every tab here is a stub CLI.

use super::gemini_test_support::*;
use super::*;
use crate::agent_composer::SendOutcome;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::time::{Duration, Instant};

/// A stub CLI that does nothing.
const STUB_SLEEP: &str = "#!/bin/sh\ntrap '' INT\nsleep 30\n";

/// A stub CLI that prints a line which reads as a question once the test says so, as the echo of a pasted prompt would.
const STUB_QUESTION_ECHO: &str = r#"#!/bin/sh
trap '' INT
while [ ! -f "$DESIGNOS_SESSION_DIR/echoed" ]; do sleep 0.05; done
printf 'Allow users to export (y/n)\r\n'
sleep 30
"#;

/// Starts a second tab on a stub `binary` beside the Gemini one and returns its index. It is the active tab, and past its start window.
fn add_tab(dir: &Path, st: &mut TerminalState, binary: &str, stub: &str, preset: AgentCliPreset) -> usize {
    let path = dir.join("bin").join(binary);
    std::fs::write(&path, stub).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    st.spawn_tab(preset, None).expect("spawn the tab");
    let tab = st.tabs.len() - 1;
    st.tabs[tab].spawned_at = Instant::now() - Duration::from_secs(60);
    assert_eq!((st.active_tab, st.tabs[tab].spawn_preset), (tab, preset));
    tab
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

fn set_waiting(st: &mut TerminalState, tab: usize) {
    *st.tabs[tab].agent_status.lock().unwrap() = AgentTabStatus::WaitingForInput(Some("Needs approval".into()));
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
    let claude = add_tab(&dir, &mut st, "claude", STUB_SLEEP, AgentCliPreset::Claude);
    assert!(!enter_written_after(&mut st, claude, |st| set_waiting(st, claude)), "Enter went to a Claude tab that began to wait after the paste");
    assert_eq!(st.tabs[claude].composer.last_outcome, Some(REFUSED));
    // Control: the same send to the same tab, with nothing waiting, does write its Enter.
    *st.tabs[claude].agent_status.lock().unwrap() = AgentTabStatus::Idle;
    assert!(enter_written_after(&mut st, claude, |_| {}), "a Claude tab that waits for nothing got no Enter");
    finish(dir, st);
}

#[test]
fn a_codex_tab_whose_echo_of_the_prompt_reads_as_a_question_still_gets_its_enter() {
    let (dir, mut st) = start_gemini("enter-codex", ASK, |_| {});
    let codex = add_tab(&dir, &mut st, "codex", STUB_QUESTION_ECHO, AgentCliPreset::Codex);
    let session = st.session_dir.clone();
    let echo = move |st: &mut TerminalState| {
        // The CLI prints "Allow users to export (y/n)" after the paste; the reader thread reads it as a question.
        std::fs::write(session.join("echoed"), b"x").unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while !matches!(st.tabs[codex].effective_status(), AgentTabStatus::WaitingForInput(_)) {
            assert!(Instant::now() < deadline, "the output never set a waiting status on the Codex tab");
            std::thread::sleep(Duration::from_millis(25));
        }
    };
    assert!(enter_written_after(&mut st, codex, echo), "the user's own Enter was held back for a question that was only the echo of the prompt");
    assert_ne!(st.tabs[codex].composer.last_outcome, Some(REFUSED));
    finish(dir, st);
}

#[test]
fn a_tab_with_the_output_heuristic_on_still_gets_its_enter_when_its_status_reads_as_a_question() {
    let (dir, mut st) = start_gemini("enter-agy", ASK, |_| {});
    let agy = add_tab(&dir, &mut st, "agy", STUB_SLEEP, AgentCliPreset::Antigravity);
    assert!(st.tabs[agy].pty_waiting_heuristic.load(std::sync::atomic::Ordering::Acquire), "an Ask-mode Antigravity tab reads its waits from its output");
    assert!(enter_written_after(&mut st, agy, |st| set_waiting(st, agy)), "the Enter was held back for a status that came from the output");
    finish(dir, st);
}

#[test]
fn a_tab_that_read_as_waiting_before_the_paste_is_still_refused_at_send_time_whatever_the_source() {
    let (dir, mut st) = start_gemini("enter-before", ASK, |_| {});
    let codex = add_tab(&dir, &mut st, "codex", STUB_SLEEP, AgentCliPreset::Codex);
    let agy = add_tab(&dir, &mut st, "agy", STUB_SLEEP, AgentCliPreset::Antigravity);
    for tab in [codex, agy] {
        st.active_tab = tab;
        set_waiting(&mut st, tab);
        let mark = st.tabs[tab].written_bytes.len();
        assert_eq!(send(&mut st), REFUSED, "{:?}: a send went into a tab that was already waiting", st.tabs[tab].spawn_preset);
        assert_eq!(st.tabs[tab].written_bytes.len(), mark, "{:?}: bytes reached a tab that was already waiting", st.tabs[tab].spawn_preset);
    }
    finish(dir, st);
}
