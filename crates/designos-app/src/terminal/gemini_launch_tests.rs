//! Gemini CLI tabs: the private status-hooks file a tab starts with, the environment it gets, and the hook records that drive
//! the tab. The tab is a stub `gemini` that records what it was started with.

use super::gemini_test_support::*;
use super::*;
use crate::agent_permissions::AgentPermissionMode;
use crate::gemini_status_hooks::{self as hooks, SYSTEM_DEFAULTS_ENV};
use std::os::unix::fs::PermissionsExt;

#[test]
fn in_ask_mode_gemini_reads_its_status_hooks_from_a_private_file() {
    let (dir, st) = start_gemini("private", AgentPermissionMode::Ask, |_| {});
    let tab_id = st.tabs[0].id;
    let file = hooks::tab_file_path(&st.session_dir, tab_id).expect("a private channel");
    assert_eq!(recorded(&st, "defaults-env.txt"), file.display().to_string());
    assert!(!file.starts_with(&st.session_dir), "the file Gemini reads sits in the session folder: {}", file.display());
    assert_eq!(std::fs::metadata(&file).unwrap().permissions().mode() & 0o777, 0o600);
    let seen: serde_json::Value = serde_json::from_str(&recorded(&st, "defaults-seen.txt")).expect("the stub read the hooks file");
    assert_eq!(seen, hooks::hooks_settings(&st.session_dir, tab_id));
    assert!(seen.to_string().contains(&format!("\\\"tab\\\":\\\"{tab_id}\\\"")), "the tab id is in the records the handlers write");
    // No hook reaches the session folder, where an agent can write.
    assert!(!recorded(&st, ".gemini/settings.json").contains("hooks"), "the session's Gemini settings carry hooks");
    assert!(st.tabs[0].gemini.hooks, "hook records start this tab's waits");
    assert!(!st.tabs[0].pty_waiting_heuristic.load(std::sync::atomic::Ordering::Acquire), "hook events drive the status, not the output");
    finish(dir, st);
}

#[test]
fn skip_mode_gemini_gets_no_hooks_file_and_keeps_its_environment() {
    let (dir, st) = start_gemini("skip", AgentPermissionMode::Skip, |_| {});
    assert_eq!(recorded(&st, "defaults-env.txt"), inherited());
    assert!(hooks::tab_file_path(&st.session_dir, st.tabs[0].id).is_some_and(|p| !p.exists()));
    assert!(!st.tabs[0].pty_waiting_heuristic.load(std::sync::atomic::Ordering::Acquire));
    finish(dir, st);
}

#[test]
fn gemini_with_system_defaults_of_its_own_gets_no_hooks_file_and_waits_from_its_screen() {
    let admin = std::env::temp_dir().join(format!("designos-gemini-admin-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&admin);
    std::fs::create_dir_all(&admin).unwrap();
    let no_such_folder = admin.join("none");
    // Studio's environment already names a defaults file.
    let (dir, st) =
        hooks::with_system_config(&[(SYSTEM_DEFAULTS_ENV, "/admin/defaults.json")], &no_such_folder, || start_gemini("env", AgentPermissionMode::Ask, |_| {}));
    assert_eq!(recorded(&st, "defaults-env.txt"), inherited(), "an inherited value is left alone");
    assert!(hook_files(&st).is_empty());
    assert!(!st.tabs[0].gemini.hooks, "no hooks file: only the dialog on screen starts a wait");
    assert!(!st.tabs[0].pty_waiting_heuristic.load(std::sync::atomic::Ordering::Acquire), "words in the output start no wait");
    finish(dir, st);
    // A system-defaults.json sits where Gemini looks for it.
    std::fs::write(admin.join("system-defaults.json"), "{}").unwrap();
    let (dir, st) = hooks::with_system_config(&[], &admin, || start_gemini("file", AgentPermissionMode::Ask, |_| {}));
    assert!(hook_files(&st).is_empty() && !st.tabs[0].gemini.hooks);
    finish(dir, st);
    // Positive control: a system settings variable that names a folder without defaults does not block the hooks.
    let elsewhere = admin.join("elsewhere/settings.json");
    let (dir, st) = hooks::with_system_config(&[("GEMINI_CLI_SYSTEM_SETTINGS_PATH", elsewhere.to_str().unwrap())], &admin, || {
        start_gemini("moved", AgentPermissionMode::Ask, |_| {})
    });
    assert_eq!(hook_files(&st).len(), 1);
    finish(dir, st);
    // ... but Gemini then looks for the defaults beside that file, and a system-defaults.json there is in force.
    std::fs::create_dir_all(admin.join("elsewhere")).unwrap();
    std::fs::write(admin.join("elsewhere/system-defaults.json"), "{}").unwrap();
    let (dir, st) = hooks::with_system_config(&[("GEMINI_CLI_SYSTEM_SETTINGS_PATH", elsewhere.to_str().unwrap())], &no_such_folder, || {
        start_gemini("moved-with-defaults", AgentPermissionMode::Ask, |_| {})
    });
    assert!(hook_files(&st).is_empty());
    finish(dir, st);
    let _ = std::fs::remove_dir_all(&admin);
}

#[test]
fn the_defaults_variable_is_set_only_for_a_tab_with_a_hooks_file() {
    let mut env = SessionEnv::for_session_id("gemini-env");
    let mut gemini = CommandBuilder::new("sh");
    gemini.env(SYSTEM_DEFAULTS_ENV, "/admin/defaults.json");
    configure_pty_env_with_session(&mut gemini, AgentCliPreset::Gemini, None, Some(&env));
    assert_eq!(gemini.get_env(SYSTEM_DEFAULTS_ENV), Some(std::ffi::OsStr::new("/admin/defaults.json")), "no file: the inherited value stays");
    env.gemini_system_defaults = Some("/private/gemini-tab-1.json".to_string());
    configure_pty_env_with_session(&mut gemini, AgentCliPreset::Gemini, None, Some(&env));
    assert_eq!(gemini.get_env(SYSTEM_DEFAULTS_ENV), Some(std::ffi::OsStr::new("/private/gemini-tab-1.json")));
    let mut claude = CommandBuilder::new("sh");
    configure_pty_env_with_session(&mut claude, AgentCliPreset::Claude, None, Some(&env));
    assert_eq!(
        claude.get_env(SYSTEM_DEFAULTS_ENV).map(|v| v.to_string_lossy().into_owned()),
        std::env::var(SYSTEM_DEFAULTS_ENV).ok(),
        "another CLI never gets it"
    );
}

#[test]
fn the_hooks_file_goes_with_its_tab_and_a_stale_one_goes_when_a_tab_starts() {
    let (dir, mut st) = start_gemini("close", AgentPermissionMode::Ask, |_| {});
    // A second tab keeps the private folder alive, which closing the last tab would remove along with every file in it.
    st.spawn_tab(AgentCliPreset::Gemini, None).expect("spawn a second gemini");
    let [first, second] = [0, 1].map(|i| hooks::tab_file_path(&st.session_dir, st.tabs[i].id).unwrap());
    assert!(first.is_file() && second.is_file() && first != second);
    st.close_tab(0);
    assert!(!first.exists(), "a closed tab's hooks file stays in the private folder");
    assert!(second.is_file(), "closing a tab removed another tab's hooks file");
    finish(dir, st);

    let (dir, st) = start_gemini("stale", AgentPermissionMode::Skip, |session| {
        let channel = crate::agent_approval_channel::prepare_channel_dir(session).unwrap();
        std::fs::write(channel.join("gemini-tab-1.json"), "stale").unwrap();
    });
    assert!(hook_files(&st).is_empty(), "an earlier run's file reached a tab that has no hooks");
    finish(dir, st);
}

#[test]
fn gemini_hook_records_drive_the_tab_through_waiting_working_and_done() {
    let (dir, mut st) = start_gemini("bridge", AgentPermissionMode::Ask, |_| {});
    let id = st.tabs[0].id;
    ask_permission(&mut st);
    assert!(matches!(st.tabs[0].effective_status(), AgentTabStatus::WaitingForInput(_)));
    assert!(st.needs_approval(), "the Open tab card shows");
    send_record(&mut st, id + 1, "PreToolUse", r#"{"session_id":"g"}"#);
    assert!(st.needs_approval(), "a record for another tab does not clear the wait");
    send_record(&mut st, id, "PreToolUse", r#"{"session_id":"g"}"#);
    assert!(matches!(st.tabs[0].effective_status(), AgentTabStatus::Working(_)) && !st.needs_approval());
    send_record(&mut st, id, "Stop", r#"{"session_id":"g"}"#);
    assert_eq!(st.tabs[0].effective_status(), AgentTabStatus::Done);
    finish(dir, st);
}

#[test]
fn off_unix_a_gemini_tab_gets_no_hooks_file() {
    // Gemini runs a hook through PowerShell on Windows, where the POSIX status command cannot parse.
    let session = std::env::temp_dir().join(format!("designos-gemini-off-unix-{}", std::process::id()));
    let ask = AgentPermissionMode::Ask;
    assert!(hooks::prepare_for_tab_on(false, &session, 1, ask, AgentCliPreset::Gemini).is_none());
    assert!(hooks::tab_file_path(&session, 1).is_some_and(|file| !file.exists()), "a file was written for a shell that cannot run it");
    let file = hooks::prepare_for_tab_on(true, &session, 1, ask, AgentCliPreset::Gemini).expect("a POSIX shell gets the file");
    assert!(file.is_file());
    crate::agent_approval_channel::remove_channel(&session);
}

#[test]
fn a_failed_spawn_leaves_no_hooks_file_behind() {
    // A session folder nobody can enter makes the PTY child fail to start, after the hooks file was written.
    let dir = std::env::temp_dir().join(format!("designos-gemini-failed-spawn-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    crate::agent_permissions::save_permission_mode(&dir, AgentPermissionMode::Ask).unwrap();
    let mut st = TerminalState::for_tab(crate::live_sync_tests::unique_tab_id());
    st.bind_library_file(&dir, "file-gemini-failed", &format!("p{}-w1-tab-1", std::process::id()));
    std::fs::create_dir_all(&st.session_dir).unwrap();
    struct Reopen(PathBuf);
    impl Drop for Reopen {
        fn drop(&mut self) {
            let _ = std::fs::set_permissions(&self.0, std::fs::Permissions::from_mode(0o755));
        }
    }
    let reopen = Reopen(st.session_dir.clone());
    std::fs::set_permissions(&st.session_dir, std::fs::Permissions::from_mode(0o000)).unwrap();
    assert!(st.spawn_tab(AgentCliPreset::Gemini, None).is_err(), "the spawn was meant to fail");
    assert!(st.tabs.is_empty());
    assert!(hook_files(&st).is_empty(), "the failed tab's hooks file stays in the private folder");
    drop(reopen);
    finish(dir, st);
}
