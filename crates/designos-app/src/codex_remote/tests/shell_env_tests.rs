//! The tab's environment as a session keeps it: captured once, a failure falls back and is tried again, names only in Debug.
//! Every value here is made up for the test. Nothing prints a real one.

use super::support::{env_value, scratch_dir, shell_builder, stub_shell, wait_until, write_script};
use crate::codex_remote::gate::RETRY_FAILED_AFTER;
use crate::codex_remote::shell_capture::capture;
use crate::codex_remote::shell_env::{forget, lookup, lookup_with, tab_shell, EnvLookup, TabEnv};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

const LONG: Duration = Duration::from_secs(10);

/// The session's environment once it is `Ready` (the lookup is asked again until then).
fn ready(session: &str, shell: &Path, retry_after: Duration) -> Arc<TabEnv> {
    let mut found = None;
    assert!(wait_until(LONG, || match lookup_with(session, &shell_builder(), shell, LONG, retry_after) {
        EnvLookup::Ready(env) => {
            found = Some(env);
            true
        }
        EnvLookup::Pending => false,
    }));
    found.unwrap()
}

#[test]
fn debug_prints_names_and_never_values() {
    let env = capture(&shell_builder(), &stub_shell(&scratch_dir("ee"), "export CXTEST_FROM_RC=fake-rc-value", ""), LONG).unwrap();
    let shown = format!("{env:?}");
    assert!(shown.contains("CXTEST_FROM_RC") && shown.contains("CXTEST_PLAIN"), "{shown}");
    for secret in ["fake-rc-value", "plain-fake-1", "/x/file.designos"] {
        assert!(!shown.contains(secret), "a value leaked into Debug output");
    }
}

#[test]
fn the_environment_is_captured_once_per_session_and_a_failure_falls_back_to_the_builder() {
    let dir = scratch_dir("ef");
    let session = format!("sess-{}-a", std::process::id());
    let first = ready(&session, &stub_shell(&dir, "export CXTEST_FROM_RC=fake-rc-value", ""), LONG);
    assert_eq!(env_value(&first, "CXTEST_FROM_RC").as_deref(), Some("fake-rc-value"));
    assert!(Arc::ptr_eq(&first, &ready(&session, &write_script(&dir, "other", "exit 1"), LONG)), "a session is captured once");
    forget(&session);
    let failing = ready(&session, &write_script(&dir, "broken", "echo no marker"), LONG);
    assert_eq!(env_value(&failing, "CXTEST_FROM_RC"), None);
    assert_eq!(env_value(&failing, "CXTEST_PLAIN").as_deref(), Some("plain-fake-1"), "a failed capture uses the builder's environment");
    forget(&session);
}

#[test]
fn a_failed_capture_is_tried_again_after_the_delay_and_a_good_one_is_kept() {
    let dir = scratch_dir("ek");
    let (count, ok) = (dir.join("count.txt"), dir.join("ok"));
    let shell = stub_shell(&dir, &format!("echo run >> {}; [ -f {} ] || exit 5\nexport CXTEST_FROM_RC=fake-rc-value", count.display(), ok.display()), "");
    let (session, retry) = (format!("sess-{}-retry", std::process::id()), Duration::from_millis(400));
    let runs = || std::fs::read_to_string(&count).map_or(0, |text| text.lines().count());
    let fallback = ready(&session, &shell, retry);
    assert_eq!((env_value(&fallback, "CXTEST_FROM_RC"), runs()), (None, 1), "the builder's environment while the capture fails");
    std::fs::write(&ok, "").unwrap();
    assert_eq!(env_value(&ready(&session, &shell, retry), "CXTEST_FROM_RC"), None);
    assert_eq!(runs(), 1, "not asked again before the delay");
    std::thread::sleep(retry + Duration::from_millis(100));
    assert!(
        wait_until(
            LONG,
            || matches!(lookup_with(&session, &shell_builder(), &shell, LONG, retry), EnvLookup::Ready(env) if env_value(&env, "CXTEST_FROM_RC").is_some())
        ),
        "tried again once the delay has passed"
    );
    std::fs::remove_file(&ok).unwrap();
    std::thread::sleep(retry + Duration::from_millis(100));
    assert!(env_value(&ready(&session, &shell, retry), "CXTEST_FROM_RC").is_some(), "a captured environment is never asked for again");
    assert_eq!(runs(), 2, "one failure, one success");
    forget(&session);
}

#[test]
fn the_shell_is_the_builders_or_the_one_the_terminal_falls_back_to() {
    let mut with_shell = shell_builder();
    with_shell.env("SHELL", "/some/fish");
    assert_eq!(tab_shell(&with_shell), PathBuf::from("/some/fish"));
    let mut without = shell_builder();
    without.env_remove("SHELL");
    assert!(["/bin/zsh", "/bin/sh"].contains(&tab_shell(&without).to_str().unwrap()));
}

#[test]
fn the_lookup_a_tab_makes_waits_thirty_seconds_before_it_asks_a_failing_shell_again() {
    assert_eq!(RETRY_FAILED_AFTER, Duration::from_secs(30));
    let dir = scratch_dir("el");
    let count = dir.join("count.txt");
    let shell = stub_shell(&dir, &format!("echo run >> {}; exit 5", count.display()), "");
    let session = format!("sess-{}-default", std::process::id());
    let ask = || lookup(&session, &shell_builder(), &shell);
    assert!(wait_until(LONG, || matches!(ask(), EnvLookup::Ready(_))), "the builder's environment once the capture has failed");
    for _ in 0..3 {
        assert!(matches!(ask(), EnvLookup::Ready(_)));
    }
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(std::fs::read_to_string(&count).map_or(0, |text| text.lines().count()), 1, "not asked again within the delay");
    forget(&session);
}

#[test]
fn a_session_forgotten_while_its_capture_runs_keeps_no_result() {
    let dir = scratch_dir("q4");
    let (go, done) = (dir.join("go"), dir.join("done"));
    let wait = format!("while [ ! -f {} ]; do sleep 0.05; done", go.display());
    let shell = stub_shell(&dir, &wait, &format!("echo done >> {}", done.display()));
    let session = format!("sess-{}-forgotten", std::process::id());
    assert!(matches!(lookup_with(&session, &shell_builder(), &shell, LONG, LONG), EnvLookup::Pending), "the capture starts and waits");
    forget(&session);
    std::fs::write(&go, "").unwrap();
    assert!(wait_until(LONG, || done.exists()), "the capture ran to its end");
    std::thread::sleep(Duration::from_millis(500));
    let again = lookup_with(&session, &shell_builder(), &shell, LONG, LONG);
    assert!(matches!(again, EnvLookup::Pending), "nothing was stored for the session that ended: a new capture starts");
    forget(&session);
}
