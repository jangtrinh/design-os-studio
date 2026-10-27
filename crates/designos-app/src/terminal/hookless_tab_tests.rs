//! A tab whose hooks cannot report gets no hook record at all: Gemini or Grok with the user's own sandbox turned on confines the hooks
//! (hooks are children of the confined process), and the hook folder lies outside what that sandbox grants. These tests answer what
//! such a tab then does, by comparing it with a tab of OpenCode, which has no hooks and has always run like that: is it stuck on
//! "working", does Stop break, is a receipt wrong?

use super::*;
use crate::agent_session_files::test_support::tab_state_in;
use std::io::Write;

/// Opens a tab on `st` that stands for `preset` without starting any CLI: the shell behind it is the process.
fn tab_of(st: &mut crate::test_timing::ScopedTerminalState, preset: AgentCliPreset, session_uuid: &str) -> usize {
    st.spawn_tab(AgentCliPreset::Shell, None).expect("a shell tab");
    let idx = st.tabs.len() - 1;
    let tab = &mut st.tabs[idx];
    tab.spawn_preset = preset;
    tab.title = preset.label().to_string();
    tab.session_uuid = Some(session_uuid.to_string());
    idx
}

/// What the strip, the Stop control and the receipt row show of a tab.
fn shown_of(tab: &TerminalTab) -> (String, bool, bool, bool, bool, bool) {
    (
        format!("{:?}", tab.agent_status.lock().map(|s| s.clone()).unwrap_or_default()),
        crate::agent_stop::is_tab_turn_running(tab),
        crate::agent_stop::is_tab_stoppable(tab),
        tab.last_turn_receipt().is_some(),
        tab.still_running_notice.is_some(),
        tab.unread,
    )
}

#[test]
fn a_hook_driven_tab_that_gets_no_hook_record_shows_exactly_what_an_opencode_tab_shows() {
    let mut st = tab_of_state();
    let hooked = [AgentCliPreset::Claude, AgentCliPreset::Grok, AgentCliPreset::Antigravity, AgentCliPreset::Gemini];
    let mut indexes: Vec<usize> = hooked.iter().enumerate().map(|(i, preset)| tab_of(&mut st, *preset, &format!("sid-{i}"))).collect();
    indexes.push(tab_of(&mut st, AgentCliPreset::OpenCode, "sid-opencode"));

    // The tabs live and are polled for a while: no record ever reaches the events file, which is not even there.
    for _ in 0..8 {
        let _ = st.poll_live_agent_bridge();
        std::thread::sleep(std::time::Duration::from_millis(25));
    }

    let reference = shown_of(&st.tabs[*indexes.last().expect("the OpenCode tab")]);
    assert_eq!(reference.0, "Idle", "an OpenCode tab shows no status of its own");
    assert!(!reference.1 && !reference.2, "an OpenCode tab has no turn to stop and no Stop control");
    for (idx, preset) in indexes.iter().zip(hooked) {
        assert_eq!(shown_of(&st.tabs[*idx]), reference, "{preset:?} with no hook record shows what OpenCode shows");
    }
    st.shutdown();
}

#[test]
fn the_same_tabs_do_work_once_records_arrive_so_the_records_are_what_the_status_is_made_of() {
    let mut st = tab_of_state();
    let claude = tab_of(&mut st, AgentCliPreset::Claude, "sid-claude");
    let opencode = tab_of(&mut st, AgentCliPreset::OpenCode, "sid-opencode");
    let events = st.prepared_hook_paths().events();
    let append = |record: &str| {
        let mut file = std::fs::OpenOptions::new().create(true).append(true).open(&events).expect("the events file");
        writeln!(file, "{record}").expect("a record");
    };
    let _ = st.poll_live_agent_bridge();

    append(r#"{"t":10.0,"e":"UserPromptSubmit","p":{"session_id":"sid-claude"}}"#);
    let _ = st.poll_live_agent_bridge();
    assert!(crate::agent_stop::is_tab_turn_running(&st.tabs[claude]), "a UserPromptSubmit record starts the turn of the Claude tab");
    assert!(!crate::agent_stop::is_tab_turn_running(&st.tabs[opencode]), "and none of the OpenCode tab's");

    append(r#"{"t":11.0,"e":"Stop","p":{"session_id":"sid-claude"}}"#);
    let _ = st.poll_live_agent_bridge();
    assert!(!crate::agent_stop::is_tab_turn_running(&st.tabs[claude]), "a Stop record ends it");
    st.shutdown();
}

/// A window with a session folder, an Ask-mode permission and no CLI that can start.
fn tab_of_state() -> crate::test_timing::ScopedTerminalState {
    tab_state_in(PermissionMode::Ask)
}
