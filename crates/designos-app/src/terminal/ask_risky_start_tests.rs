//! Ask when risky runs an agent CLI only when it shares the approval hook. Every way of starting a session asks that first and
//! fails closed: nothing starts, the status line gives the reason the agent menu shows for that CLI, and the text of a prompt that
//! asked for the start stays in the composer.

use super::*;
use crate::agent_approval_channel::without_private_root;
use crate::agent_composer::{RefuseReason, SendOutcome};
use crate::agent_permissions::{ask_risky_unsupported_reason, AgentPermissionMode, CUSTOM_CLI_ASK_RISKY_REASON};
use crate::agent_session_files::test_support::tab_state_in;

const WITHOUT_THE_HOOK: [AgentCliPreset; 5] =
    [AgentCliPreset::Codex, AgentCliPreset::Grok, AgentCliPreset::Gemini, AgentCliPreset::Antigravity, AgentCliPreset::OpenCode];

fn reason_for(preset: AgentCliPreset) -> &'static str {
    ask_risky_unsupported_reason(preset).expect("a CLI without the hook has a reason")
}

fn recorded_session(cli: &str, tab_id: u64) -> crate::agent_sessions::SessionEntry {
    crate::agent_sessions::SessionEntry {
        tab_id,
        title: format!("{cli} #{tab_id}"),
        cli: cli.into(),
        session_id: None,
        created_at: 100,
        last_active_at: 200,
        status: crate::agent_sessions::SessionEntryStatus::Exited,
        extra: Default::default(),
    }
}

/// A shell tab retagged as `preset`, the way the composer tests make an agent tab, running in `mode`.
fn tab_of(st: &mut TerminalState, preset: AgentCliPreset, mode: AgentPermissionMode) {
    st.spawn_tab(AgentCliPreset::Shell, None).expect("a tab");
    let tab = st.active_tab_mut().expect("active tab");
    tab.spawn_preset = preset;
    tab.preset = preset;
    tab.permission_mode = mode;
}

#[test]
fn a_new_tab_of_a_cli_without_the_hook_does_not_start() {
    for preset in WITHOUT_THE_HOOK {
        without_private_root(|| {
            let mut st = tab_state_in(AgentPermissionMode::AskRisky);
            let err = st.spawn_tab(preset, None).expect_err("the CLI has no approval hook");
            assert_eq!(err, reason_for(preset), "{preset:?}");
            assert_eq!(st.status_msg.as_deref(), Some(reason_for(preset)), "{preset:?}");
            assert!(st.tabs.is_empty(), "{preset:?}: no tab was made");
        });
    }
}

#[test]
fn claude_and_every_other_mode_still_start() {
    without_private_root(|| {
        let mut st = tab_state_in(AgentPermissionMode::AskRisky);
        st.spawn_tab(AgentCliPreset::Claude, None).expect("Claude has the hook");
        st.shutdown();
    });
    for mode in [AgentPermissionMode::Ask, AgentPermissionMode::Skip] {
        without_private_root(|| {
            let mut st = tab_state_in(mode);
            st.spawn_tab(AgentCliPreset::Codex, None).unwrap_or_else(|e| panic!("{mode:?}: {e}"));
            st.shutdown();
        });
    }
}

#[test]
fn a_custom_command_does_not_start() {
    without_private_root(|| {
        let mut st = tab_state_in(AgentPermissionMode::AskRisky);
        let err = st.spawn_custom_cli_tab("Custom", "true", None).expect_err("a custom command has no approval hook");
        assert_eq!(err, CUSTOM_CLI_ASK_RISKY_REASON);
        assert_eq!(st.status_msg.as_deref(), Some(CUSTOM_CLI_ASK_RISKY_REASON));
        assert!(st.tabs.is_empty());
    });
}

#[test]
fn resuming_a_session_of_a_cli_without_the_hook_does_not_start() {
    for cli in ["codex", "grok"] {
        without_private_root(|| {
            let mut st = tab_state_in(AgentPermissionMode::AskRisky);
            st.codex_cwd_scoped = true;
            st.session_registry.sessions.push(recorded_session(cli, 7));
            let preset = crate::agent_sessions::preset_from_cli_name(cli).expect("a known CLI");

            let by_id = st.resume_session_by_tab_id(7, None).expect_err("resume by id is refused");
            assert_eq!(by_id, reason_for(preset), "{cli}");
            let last = st.resume_last_session(None).expect_err("resume of the newest is refused");
            assert_eq!(last, reason_for(preset), "{cli}");
            assert!(st.tabs.is_empty(), "{cli}: no tab was made");
        });
    }
}

#[test]
fn make_design_with_no_tab_keeps_its_text_and_says_why() {
    without_private_root(|| {
        let mut st = tab_state_in(AgentPermissionMode::AskRisky);
        st.active_runtime = AgentCliPreset::Codex;

        let outcome = st.send_to_agent("make a hero", None);

        assert_eq!(outcome, SendOutcome::Refused(RefuseReason::ModeUnavailable));
        assert!(st.tabs.is_empty(), "no agent was started");
        assert_eq!(st.composer_draft(), "make a hero", "the text is in the composer");
        assert_eq!(st.active_composer().notice.as_deref(), Some(reason_for(AgentCliPreset::Codex)));
        assert_eq!(st.status_msg.as_deref(), Some(reason_for(AgentCliPreset::Codex)));
        assert!(st.active_composer().pending_send.is_empty(), "nothing waits to be sent to another agent later");
    });
}

#[test]
fn cmd_k_make_design_with_no_tab_keeps_its_text_once() {
    without_private_root(|| {
        let mut st = tab_state_in(AgentPermissionMode::AskRisky);
        st.active_runtime = AgentCliPreset::Grok;

        st.dispatch_figma_ai_prompt("Make Design", "a pricing table", None);

        assert!(st.tabs.is_empty());
        assert_eq!(st.composer_draft(), "a pricing table", "the text is in the composer");
        assert!(st.pending_prompt.is_none(), "and is not also held for a later start");
        assert_eq!(st.status_msg.as_deref(), Some(reason_for(AgentCliPreset::Grok)));
    });
}

#[test]
fn a_resumable_session_of_a_cli_without_the_hook_is_not_resumed_by_a_prompt() {
    without_private_root(|| {
        let mut st = tab_state_in(AgentPermissionMode::AskRisky);
        st.codex_cwd_scoped = true;
        st.active_runtime = AgentCliPreset::Shell;
        st.session_registry.sessions.push(recorded_session("codex", 3));

        let outcome = st.send_to_agent("make a hero", None);

        assert_eq!(outcome, SendOutcome::Refused(RefuseReason::ModeUnavailable));
        assert!(st.tabs.is_empty());
        assert_eq!(st.composer_draft(), "make a hero");
    });
}

#[test]
fn an_image_job_does_not_start_a_cli_without_the_hook() {
    without_private_root(|| {
        let mut st = tab_state_in(AgentPermissionMode::AskRisky);
        st.submit_system_image_prompt(AgentCliPreset::Codex, "Image task: remove the background.", None);
        assert!(st.tabs.is_empty(), "no agent was started");
        assert_eq!(st.status_msg.as_deref(), Some(reason_for(AgentCliPreset::Codex)));
    });
}

/// Stub agent CLIs that only record how they were called, so a login the tests drive never reaches a real sign-in.
#[cfg(unix)]
fn stub_agent_binaries(tag: &str) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let dir = std::env::temp_dir().join(format!("designos-login-stubs-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("stub folder");
    for preset in
        [AgentCliPreset::Claude, AgentCliPreset::Codex, AgentCliPreset::Grok, AgentCliPreset::Gemini, AgentCliPreset::Antigravity, AgentCliPreset::OpenCode]
    {
        let name = preset.binary_name();
        let path = dir.join(name);
        std::fs::write(&path, format!("#!/bin/sh\necho \"$*\" > \"{}/{name}-ran.txt\"\n", dir.display())).expect("stub script");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    }
    dir
}

/// Gemini and Antigravity have no login subcommand: their login is the agent itself, so it needs the same approval hook a start
/// does and Ask when risky refuses it with the reason a start gets.
#[cfg(unix)]
#[test]
fn run_login_for_gemini_and_antigravity_is_refused_in_ask_when_risky() {
    for preset in [AgentCliPreset::Gemini, AgentCliPreset::Antigravity] {
        without_private_root(|| {
            let stubs = stub_agent_binaries("refused");
            let mut st = tab_state_in(AgentPermissionMode::AskRisky);
            st.path_override = Some(stubs.clone().into_os_string());
            tab_of(&mut st, preset, AgentPermissionMode::AskRisky);
            let tabs_before = st.tabs.len();
            st.status_msg = None;

            st.apply_start_fix(crate::agent_start_failure::FixAction::RunLogin(crate::agent_start_failure::login_command(preset)), None);

            assert_eq!(st.tabs.len(), tabs_before, "{preset:?}: Run login spawned a tab");
            assert_eq!(st.status_msg.as_deref(), Some(reason_for(preset)), "{preset:?}: the reason is shown");
            assert!(!stubs.join(format!("{}-ran.txt", preset.binary_name())).exists(), "{preset:?}: the agent binary ran");
            st.shutdown();
            let _ = std::fs::remove_dir_all(&stubs);
        });
    }
}

/// The same login in Ask still runs: only the mode refuses it.
#[cfg(unix)]
#[test]
fn run_login_for_gemini_and_antigravity_still_runs_in_ask() {
    for preset in [AgentCliPreset::Gemini, AgentCliPreset::Antigravity] {
        without_private_root(|| {
            let stubs = stub_agent_binaries("ask");
            let mut st = tab_state_in(AgentPermissionMode::Ask);
            st.path_override = Some(stubs.clone().into_os_string());
            tab_of(&mut st, preset, AgentPermissionMode::Ask);
            let tabs_before = st.tabs.len();

            st.apply_start_fix(crate::agent_start_failure::FixAction::RunLogin(crate::agent_start_failure::login_command(preset)), None);

            assert_eq!(st.tabs.len(), tabs_before + 1, "{preset:?}: Run login starts the sign-in tab in Ask");
            wait_for_file(&stubs.join(format!("{}-ran.txt", preset.binary_name())));
            st.shutdown();
            let _ = std::fs::remove_dir_all(&stubs);
        });
    }
}

/// Claude, Codex, Grok and OpenCode sign in through a real login subcommand, not through the agent, so Ask when risky leaves
/// those logins alone: the stub records that it was called with the subcommand.
#[cfg(unix)]
#[test]
fn run_login_through_a_login_subcommand_still_runs_in_ask_when_risky() {
    for (preset, args) in
        [(AgentCliPreset::Claude, "auth login"), (AgentCliPreset::Codex, "login"), (AgentCliPreset::Grok, "login"), (AgentCliPreset::OpenCode, "auth login")]
    {
        without_private_root(|| {
            let stubs = stub_agent_binaries("subcommand");
            let mut st = tab_state_in(AgentPermissionMode::AskRisky);
            st.path_override = Some(stubs.clone().into_os_string());
            tab_of(&mut st, preset, AgentPermissionMode::AskRisky);
            let tabs_before = st.tabs.len();

            st.apply_start_fix(crate::agent_start_failure::FixAction::RunLogin(crate::agent_start_failure::login_command(preset)), None);

            assert_eq!(st.tabs.len(), tabs_before + 1, "{preset:?}: Run login starts the sign-in tab");
            let recorded = wait_for_file(&stubs.join(format!("{}-ran.txt", preset.binary_name())));
            assert_eq!(recorded.trim(), args, "{preset:?}: the login subcommand ran");
            st.shutdown();
            let _ = std::fs::remove_dir_all(&stubs);
        });
    }
}

/// Only a login that is the agent's bare binary counts as starting that agent; a login subcommand does not.
#[test]
fn only_a_bare_agent_binary_login_starts_an_agent() {
    use crate::agent_start_failure::{agent_started_by_login, login_command};
    assert_eq!(agent_started_by_login(login_command(AgentCliPreset::Gemini)), Some(AgentCliPreset::Gemini));
    assert_eq!(agent_started_by_login(login_command(AgentCliPreset::Antigravity)), Some(AgentCliPreset::Antigravity));
    for preset in [AgentCliPreset::Claude, AgentCliPreset::Codex, AgentCliPreset::Grok, AgentCliPreset::OpenCode] {
        assert_eq!(agent_started_by_login(login_command(preset)), None, "{preset:?} signs in through a login subcommand");
    }
    assert_eq!(agent_started_by_login("echo hello"), None);
}

#[cfg(unix)]
fn wait_for_file(path: &Path) -> String {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        if let Ok(text) = std::fs::read_to_string(path) {
            if !text.is_empty() {
                return text;
            }
        }
        assert!(std::time::Instant::now() < deadline, "{} was never written", path.display());
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
}

#[test]
fn an_existing_tab_in_that_state_shows_the_real_reason() {
    without_private_root(|| {
        let mut st = tab_state_in(AgentPermissionMode::Ask);
        tab_of(&mut st, AgentCliPreset::Codex, AgentPermissionMode::AskRisky);
        let reason = reason_for(AgentCliPreset::Codex);

        assert_eq!(crate::agent_composer_text::placeholder(&st, crate::agent_picker::CHOOSE_AGENT, false, true), reason, "the placeholder");
        assert_eq!(crate::agent_composer_text::caption(&st).as_deref(), Some(reason), "the caption");

        // Text typed in the box is refused and kept.
        st.set_composer_draft("typed in the box");
        assert_eq!(st.send_from_composer_box("typed in the box", None), SendOutcome::Refused(RefuseReason::ModeUnavailable));
        assert_eq!(st.composer_draft(), "typed in the box");
        assert_eq!(st.status_msg.as_deref(), Some(reason), "the status line gives the CLI's own reason, not 'Select an agent tab'");

        // Text from a prompt entry point is put into the composer.
        st.set_composer_draft("");
        assert_eq!(st.send_to_agent("from Make design", None), SendOutcome::Refused(RefuseReason::ModeUnavailable));
        assert_eq!(st.composer_draft(), "from Make design");
        st.shutdown();
    });
}
