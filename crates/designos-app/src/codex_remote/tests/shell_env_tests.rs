//! The tab's login-shell environment: captured after a marker, Studio's values laid over it, any failure falls back.
//! Every value here is made up for the test. Nothing prints a real one.

use super::support::{scratch_dir, wait_until, write_script};
use crate::codex_remote::shell_env::{capture, forget, lookup, parse_capture, tab_shell, EnvLookup, TabEnv, CAPTURE_SCRIPT, CAPTURE_TIMEOUT, MARKER};
use portable_pty::CommandBuilder;
use std::ffi::OsString;
use std::os::unix::ffi::OsStringExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

const LONG: Duration = Duration::from_secs(10);

/// A tab builder with made-up values; no HOME, so a leak of Studio's own environment shows.
fn builder() -> CommandBuilder {
    let mut builder = CommandBuilder::new("true");
    builder.env_remove("HOME");
    for (name, value) in
        [("PATH", "/usr/bin:/bin"), ("CXTEST_PLAIN", "plain-fake-1"), ("DESIGNOS_FILE", "/x/file.designos"), ("DESIGNOS_AGENT_RUNTIME", "codex")]
    {
        builder.env(name, value);
    }
    builder
}

/// A login shell that does `pre` (what an rc file might) and then runs the script it was given, like `$SHELL -l -i -c`.
fn stub_shell(dir: &Path, pre: &str, after: &str) -> PathBuf {
    write_script(dir, "shell", &format!("{pre}\n/bin/sh -c \"$4\"\nstatus=$?\n{after}\nexit $status"))
}

fn value(env: &TabEnv, name: &str) -> Option<String> {
    env.get(name).map(|v| v.to_string_lossy().into_owned())
}

#[test]
fn the_login_shell_environment_is_read_after_the_marker_and_studios_values_win() {
    let dir = scratch_dir("ea");
    let rc = "echo 'rc noise'; printf '\\033]0;title\\007\\033[?2004h'; export CXTEST_FROM_RC=fake-rc-value PATH=/rc/bin:$PATH DESIGNOS_FILE=/rc/loses DESIGNOS_AGENT_RUNTIME=rc-loses";
    let env = capture(&builder(), &stub_shell(&dir, rc, ""), LONG).expect("a captured environment");
    assert_eq!(value(&env, "CXTEST_FROM_RC").as_deref(), Some("fake-rc-value"), "what the rc files export reaches the tab");
    assert!(value(&env, "PATH").is_some_and(|path| path.starts_with("/rc/bin:/usr/bin")), "so does their PATH");
    assert_eq!(value(&env, "DESIGNOS_FILE").as_deref(), Some("/x/file.designos"), "Studio's DESIGNOS_* values win");
    assert_eq!(value(&env, "DESIGNOS_AGENT_RUNTIME").as_deref(), Some("codex"));
    assert_eq!(value(&env, "CXTEST_PLAIN").as_deref(), Some("plain-fake-1"), "it starts from the builder's environment");
    assert_eq!(value(&env, "HOME"), None, "and from nothing else");
}

#[test]
fn the_shell_gets_one_script_with_no_quotes_in_its_own_group_and_no_input() {
    for banned in ['\'', '"', '\\', '$', '`', '(', ')', '|', '&', '<', '>'] {
        assert!(!CAPTURE_SCRIPT.contains(banned), "{banned:?} would not parse the same in fish, zsh and bash");
    }
    assert_eq!(CAPTURE_SCRIPT, format!("/bin/echo {MARKER}; /usr/bin/env -0"));
    assert_eq!(CAPTURE_TIMEOUT, Duration::from_secs(10));
    let dir = scratch_dir("eb");
    let (args, ids, stdin) = (dir.join("args.txt"), dir.join("ids.txt"), dir.join("stdin.txt"));
    let record = format!(
        "printf '%s\\n' \"$@\" > {}; echo \"$$ $(ps -o pgid= -p $$ | tr -d ' ')\" > {}; if read line; then echo OPEN > {s}; else echo EOF > {s}; fi",
        args.display(),
        ids.display(),
        s = stdin.display()
    );
    assert!(capture(&builder(), &stub_shell(&dir, &record, ""), LONG).is_some());
    assert_eq!(std::fs::read_to_string(&args).unwrap(), format!("-l\n-i\n-c\n{CAPTURE_SCRIPT}\n"));
    let ids = std::fs::read_to_string(&ids).unwrap();
    let mut parts = ids.split_whitespace();
    assert_eq!(parts.next(), parts.next(), "the shell leads its own process group");
    assert_eq!(std::fs::read_to_string(&stdin).unwrap().trim(), "EOF");
}

#[test]
fn a_hung_shell_is_killed_with_everything_it_started_at_the_timeout() {
    let dir = scratch_dir("ec");
    let pid_file = dir.join("pid.txt");
    let shell = write_script(&dir, "shell", &format!("echo $$ > {}; sleep 30 & exec sleep 30", pid_file.display()));
    let started = std::time::Instant::now();
    assert!(capture(&builder(), &shell, Duration::from_millis(600)).is_none());
    assert!(
        (Duration::from_millis(550)..Duration::from_secs(8)).contains(&started.elapsed()),
        "the timeout, not the shell, ends the wait: {:?}",
        started.elapsed()
    );
    let pgid = std::fs::read_to_string(&pid_file).expect("the shell ran and wrote its pid").trim().to_string();
    let group_left = || String::from_utf8_lossy(&Command::new("ps").args(["-axo", "pgid="]).output().unwrap().stdout).lines().any(|line| line.trim() == pgid);
    assert!(wait_until(Duration::from_secs(3), || !group_left()), "no process of the shell's group survives");
}

#[test]
fn a_shell_that_fails_prints_no_marker_or_prints_nothing_after_it_gives_no_environment() {
    let dir = scratch_dir("ed");
    let b = builder();
    assert!(capture(&b, &stub_shell(&dir, "", "exit 3"), LONG).is_none(), "non-zero exit, although the output looks right");
    assert!(capture(&b, &write_script(&dir, "nomarker", "echo garbage"), LONG).is_none());
    assert!(capture(&b, &write_script(&dir, "empty", &format!("echo {MARKER}")), LONG).is_none(), "a marker with no variables after it");
    assert!(capture(&b, &dir.join("absent"), LONG).is_none(), "no such shell");
}

#[test]
fn only_name_equals_value_entries_after_the_marker_line_are_taken() {
    let raw = format!("noise\x1b[?2004h{MARKER}\nA=1\0BAD\0B=two words\nand a line\0C=\0=x\0D=a=b\0");
    let vars = parse_capture(raw.as_bytes()).expect("variables");
    let pairs: Vec<(String, String)> = vars.iter().map(|(k, v)| (k.to_string_lossy().into(), v.to_string_lossy().into())).collect();
    let want = [("A", "1"), ("B", "two words\nand a line"), ("C", ""), ("D", "a=b")].map(|(k, v)| (k.to_string(), v.to_string()));
    assert_eq!(pairs, want);
    assert!(parse_capture(b"A=1\0").is_none(), "without the marker nothing is trusted");
    let raw = [MARKER.as_bytes(), b"\n", b"K=", &[0xFF, 0xFE], b"\0"].concat();
    assert_eq!(parse_capture(&raw).unwrap()[0].1, OsString::from_vec(vec![0xFF, 0xFE]), "values that are not UTF-8 are kept");
}

#[test]
fn debug_prints_names_and_never_values() {
    let env = capture(&builder(), &stub_shell(&scratch_dir("ee"), "export CXTEST_FROM_RC=fake-rc-value", ""), LONG).unwrap();
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
    let good = stub_shell(&dir, "export CXTEST_FROM_RC=fake-rc-value", "");
    let ready = |session: &str, shell: &Path| {
        let mut found = None;
        assert!(wait_until(LONG, || match lookup(session, &builder(), shell, LONG) {
            EnvLookup::Ready(env) => {
                found = Some(env);
                true
            }
            EnvLookup::Pending => false,
        }));
        found.unwrap()
    };
    let first = ready(&session, &good);
    assert_eq!(value(&first, "CXTEST_FROM_RC").as_deref(), Some("fake-rc-value"));
    assert!(std::sync::Arc::ptr_eq(&first, &ready(&session, &write_script(&dir, "other", "exit 1"))), "a session is captured once");
    forget(&session);
    let failing = ready(&session, &write_script(&dir, "broken", "echo no marker"));
    assert_eq!(value(&failing, "CXTEST_FROM_RC"), None);
    assert_eq!(value(&failing, "CXTEST_PLAIN").as_deref(), Some("plain-fake-1"), "a failed capture uses the builder's environment");
    forget(&session);
}

#[test]
fn the_shell_is_the_builders_or_the_one_the_terminal_falls_back_to() {
    let mut with_shell = builder();
    with_shell.env("SHELL", "/some/fish");
    assert_eq!(tab_shell(&with_shell), PathBuf::from("/some/fish"));
    let mut without = builder();
    without.env_remove("SHELL");
    assert!(["/bin/zsh", "/bin/sh"].contains(&tab_shell(&without).to_str().unwrap()));
}
