//! Gemini CLI loads `<session>/.gemini/settings.json` by itself, MCP servers and hooks included. In Ask mode a copy an agent
//! kept (`chmod 0555 .gemini` stops every rewrite) would keep running for good, so such a tab does not start.

use super::test_support::{running_as_root, Writable};
use super::*;
use crate::live_sync_tests::unique_tab_id;
use crate::terminal::{AgentCliPreset, PermissionMode, TerminalState};
use crate::test_timing::ScopedTerminalState;
use std::os::unix::fs::PermissionsExt;

/// A tab state in `mode` whose `.gemini` folder an agent made read-only, and no agent CLI that can start from its PATH.
fn state_with_read_only_gemini_folder(mode: PermissionMode) -> (ScopedTerminalState, Writable) {
    let mut st = ScopedTerminalState::new(TerminalState::for_tab(unique_tab_id()));
    st.permission_mode = mode;
    st.path_override = Some(std::ffi::OsString::new());
    ensure_session_dir(&st.session_dir).expect("the session folder");
    let gemini = st.session_dir.join(".gemini");
    std::fs::create_dir(&gemini).expect("the agent's own .gemini folder");
    std::fs::write(gemini.join("settings.json"), br#"{"agent":"kept"}"#).expect("the agent's settings");
    std::fs::set_permissions(&gemini, std::fs::Permissions::from_mode(0o555)).expect("chmod 0555 .gemini");
    (st, Writable(gemini))
}

#[test]
fn an_ask_mode_gemini_tab_does_not_start_when_studio_cannot_rewrite_its_settings() {
    if running_as_root() {
        return; // root writes into a read-only folder, so the agent's trick does not exist
    }
    let (mut st, _writable) = state_with_read_only_gemini_folder(PermissionMode::Ask);

    let result = st.spawn_tab(AgentCliPreset::Gemini, None);

    let message = result.expect_err("the agent's settings would keep running, so the tab must not start");
    assert!(st.tabs.is_empty(), "no tab was added");
    assert_eq!(st.status_msg.as_deref(), Some(message.as_str()), "the status line says why");
    let gemini = st.session_dir.join(".gemini").display().to_string();
    assert!(message.starts_with("Couldn't start Gemini CLI: "), "{message}");
    assert!(message.contains(&format!("{gemini} can't be written to")), "the line names the folder and says what failed: {message}");
    assert!(message.ends_with(". Make it writable or delete it, then try again."), "and what to do: {message}");
    assert_eq!(std::fs::read(st.session_dir.join(".gemini/settings.json")).expect("the agent's file"), br#"{"agent":"kept"}"#);
}

#[test]
fn a_gemini_tab_in_skip_mode_starts_whatever_the_state_of_its_settings_folder() {
    if running_as_root() {
        return;
    }
    let (mut st, _writable) = state_with_read_only_gemini_folder(PermissionMode::Skip);

    st.spawn_tab(AgentCliPreset::Gemini, None).expect("a tab in Skip mode runs with every tool allowed anyway");
}

#[test]
fn an_ask_mode_gemini_tab_does_not_start_when_its_settings_do_not_read_back_as_written() {
    let mut st = ScopedTerminalState::new(TerminalState::for_tab(unique_tab_id()));
    st.permission_mode = PermissionMode::Ask;
    st.path_override = Some(std::ffi::OsString::new());
    ensure_session_dir(&st.session_dir).expect("the session folder");
    let settings = st.session_dir.join(".gemini").join("settings.json");
    let replaced = settings.clone();
    super::session_io::test_hooks::set_before_read_back(
        &settings,
        Box::new(move || std::fs::write(&replaced, br#"{"hooks":"the agent's"}"#).expect("another agent replaces the file")),
    );

    let message = st.spawn_tab(AgentCliPreset::Gemini, None).expect_err("a file that is not what Studio wrote must not start a tab");

    assert!(st.tabs.is_empty(), "no tab was added");
    assert_eq!(
        message,
        format!("Couldn't start Gemini CLI: {} changed while Studio wrote it. Close this file's other agent tabs, then try again.", settings.display())
    );
}

#[test]
fn an_ask_mode_gemini_tab_starts_when_its_settings_were_written_and_read_back() {
    let mut st = ScopedTerminalState::new(TerminalState::for_tab(unique_tab_id()));
    st.permission_mode = PermissionMode::Ask;
    st.path_override = Some(std::ffi::OsString::new());

    st.spawn_tab(AgentCliPreset::Gemini, None).expect("the settings were written and read back");

    let written = std::fs::read_to_string(st.session_dir.join(".gemini").join("settings.json")).expect("the settings");
    assert!(written.contains("mcpServers") && !written.contains("\"trust\""), "{written}");
}
