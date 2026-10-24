//! No test starts a real agent CLI against the test's home. Two checks, because a tab's launch races its test:
//!
//! - `no_test_starts_a_real_agent_cli` puts logging stand-ins for all six CLIs first on every lookup path a test could reach (`PATH`,
//!   and the folders under `$HOME` that `extra_cli_path_dirs` adds), runs the tests that used to start a real `claude` or `grok usage`
//!   in a child process of this binary, and wants the stand-ins' log empty. The `--version` probe and `grok usage` always log when
//!   they run. A tab's launch logs only if it reaches the stand-in before its test ends and kills it (0 to 1 of the 3 with the
//!   restriction below switched off), so a pass here is not proof about tabs.
//! - `an_agent_tab_in_a_test_gets_only_its_stubs_and_the_system_folders_on_its_path` is the check that does not race: it reads the
//!   `PATH` an agent tab really gets after its shell has started.

use super::gemini_test_support::*;
use super::*;
use crate::agent_permissions::AgentPermissionMode;
use std::os::unix::fs::PermissionsExt;
use std::process::Command as Process;

/// The tests that started a real `claude` (three times) or `grok usage` (twice).
const LAUNCHING_TESTS: &[&str] = &[
    "terminal::tests::notification_when_the_tab_is_hidden_but_window_focused",
    "terminal::tests::os_notification_sink_reports_denied_immediately_and_click_focuses_tab",
    "terminal::tests::a_click_fills_the_composer_and_changes_nothing_else",
    "terminal::tests::a_click_appends_to_an_existing_draft",
    // Ignored in a normal run; last by name, it gives the launches the tests start in the background time to happen.
    "terminal::tests::zz_let_background_launches_finish",
];

const CLIS: [&str; 6] = ["claude", "codex", "agy", "grok", "gemini", "opencode"];

/// Writes one logging stand-in per CLI into `dir`: it appends its name and arguments to `log` and exits. The path is written into the
/// script, since an agent tab's environment does not carry the test's variables.
fn stand_ins_in(dir: &Path, log: &Path) {
    std::fs::create_dir_all(dir).unwrap();
    for name in CLIS {
        let path = dir.join(name);
        let script = format!("#!/bin/sh\nprintf '%s|%s\\n' \"$(basename \"$0\")\" \"$*\" >> '{}'\nexit 0\n", log.display());
        std::fs::write(&path, script).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
}

#[test]
fn no_test_starts_a_real_agent_cli() {
    let root = std::env::temp_dir().join(format!("designos-real-cli-launch-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let home = root.join("home");
    let log = root.join("launches.log");
    // Everywhere the code looks for a CLI: `PATH`, and the folders `extra_cli_path_dirs` adds under `$HOME`.
    let folders = ["bin", "home/.local/bin", "home/.grok/bin", "home/.cargo/bin", "home/.antigravity/bin", "home/.npm-global/bin", "home/.opencode/bin"];
    for folder in folders {
        stand_ins_in(&root.join(folder), &log);
    }
    let path = format!("{}:/usr/bin:/bin:/usr/sbin:/sbin", root.join("bin").display());
    let mut child = Process::new(std::env::current_exe().unwrap());
    child.args(["--test-threads=1", "--include-ignored", "--exact"]).args(LAUNCHING_TESTS);
    // zsh as `$SHELL`, as on a developer machine: without a shell the tabs never start and the check would see nothing.
    child.env("PATH", path).env("HOME", &home).env("SHELL", "/bin/zsh");
    let out = child.output().expect("run the launching tests in a child process");
    let launches = std::fs::read_to_string(&log).unwrap_or_default();
    let ran = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success(), "the child run failed:\n{ran}\n{}", String::from_utf8_lossy(&out.stderr));
    assert!(LAUNCHING_TESTS.iter().all(|name| ran.contains(&format!("{name} ... ok"))), "not every test ran:\n{ran}");
    let _ = std::fs::remove_dir_all(&root);
    assert!(launches.is_empty(), "a test started a real agent CLI (name|arguments):\n{launches}");
}

#[test]
fn an_agent_tab_in_a_test_gets_only_its_stubs_and_the_system_folders_on_its_path() {
    let (dir, st) = start_stub("path", AgentPermissionMode::Ask, STUB_PATH, |_| {});
    let seen = std::fs::read_to_string(st.session_dir.join("path-seen.txt")).unwrap();
    let allowed = [dir.join("bin").display().to_string(), "/usr/bin".into(), "/bin".into(), "/usr/sbin".into(), "/sbin".into()];
    let stray: Vec<&str> = seen.split(':').filter(|entry| !allowed.iter().any(|ok| ok == entry)).collect();
    assert!(stray.is_empty(), "the tab's shell found a CLI outside the test's stubs: {stray:?}");
    assert!(seen.starts_with(&allowed[0]), "the stubs are not first: {seen}");
    finish(dir, st);
}
