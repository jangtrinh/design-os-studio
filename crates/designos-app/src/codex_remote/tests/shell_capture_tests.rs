//! Capturing the login shell's environment: the marker, the parse, the folder, the kill. Every value is made up.

use super::support::{env_value, scratch_dir, shell_builder, stub_shell, wait_until, write_script};
use crate::codex_remote::shell_capture::{capture, capture_script, new_marker, parse_capture, MARKER_PREFIX};
use crate::codex_remote::shell_env::{TabEnv, CAPTURE_TIMEOUT};
use std::ffi::OsString;
use std::os::unix::ffi::OsStringExt;
use std::process::Command;
use std::time::Duration;

const LONG: Duration = Duration::from_secs(10);
const TEST_MARKER: &str = "__DESIGNOS_ENV_00112233445566778899aabbccddeeff__";

fn is_marker(text: &str) -> bool {
    text.strip_prefix(MARKER_PREFIX)
        .and_then(|rest| rest.strip_suffix("__"))
        .is_some_and(|hex| hex.len() == 32 && hex.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')))
}

#[test]
fn the_login_shell_environment_is_read_after_the_marker_and_studios_values_win() {
    let dir = scratch_dir("ea");
    let rc = "echo 'rc noise'; printf '\\033]0;title\\007\\033[?2004h'; export CXTEST_FROM_RC=fake-rc-value PATH=/rc/bin:$PATH DESIGNOS_FILE=/rc/loses DESIGNOS_AGENT_RUNTIME=rc-loses";
    let env = capture(&shell_builder(), &stub_shell(&dir, rc, ""), LONG).expect("a captured environment");
    assert_eq!(env_value(&env, "CXTEST_FROM_RC").as_deref(), Some("fake-rc-value"), "what the rc files export reaches the tab");
    assert!(env_value(&env, "PATH").is_some_and(|path| path.starts_with("/rc/bin:/usr/bin")), "so does their PATH");
    assert_eq!(env_value(&env, "DESIGNOS_FILE").as_deref(), Some("/x/file.designos"), "Studio's DESIGNOS_* values win");
    assert_eq!(env_value(&env, "DESIGNOS_AGENT_RUNTIME").as_deref(), Some("codex"));
    assert_eq!(env_value(&env, "CXTEST_PLAIN").as_deref(), Some("plain-fake-1"), "it starts from the builder's environment");
    assert_eq!(env_value(&env, "HOME"), None, "and from nothing else");
}

#[test]
fn the_shell_gets_one_script_with_no_quotes_in_its_own_group_and_no_input() {
    let script = capture_script(TEST_MARKER);
    for banned in ['\'', '"', '\\', '$', '`', '(', ')', '|', '&', '<', '>'] {
        assert!(!script.contains(banned), "{banned:?} would not parse the same in fish, zsh and bash");
    }
    assert_eq!(script, format!("/bin/echo {TEST_MARKER}; /usr/bin/env -0"));
    assert_eq!(CAPTURE_TIMEOUT, Duration::from_secs(10));
    let dir = scratch_dir("eb");
    let (args, ids, stdin) = (dir.join("args.txt"), dir.join("ids.txt"), dir.join("stdin.txt"));
    let record = format!(
        "printf '%s\\n' \"$@\" > {}; echo \"$$ $(ps -o pgid= -p $$ | tr -d ' ')\" > {}; if read line; then echo OPEN > {s}; else echo EOF > {s}; fi",
        args.display(),
        ids.display(),
        s = stdin.display()
    );
    assert!(capture(&shell_builder(), &stub_shell(&dir, &record, ""), LONG).is_some());
    let given = std::fs::read_to_string(&args).unwrap();
    let words: Vec<&str> = given.lines().collect();
    assert_eq!(&words[..3], ["-l", "-i", "-c"]);
    let marker = words[3].split_whitespace().nth(1).unwrap().trim_end_matches(';');
    assert!(is_marker(marker) && words[3] == capture_script(marker), "one argument: the fixed script around a random marker");
    let ids = std::fs::read_to_string(&ids).unwrap();
    let mut parts = ids.split_whitespace();
    assert_eq!(parts.next(), parts.next(), "the shell leads its own process group");
    assert_eq!(std::fs::read_to_string(&stdin).unwrap().trim(), "EOF");
}

#[test]
fn the_marker_is_random_and_new_for_every_capture() {
    let (first, second) = (new_marker().unwrap(), new_marker().unwrap());
    assert!(is_marker(&first) && is_marker(&second), "{first} {second}");
    assert_ne!(first, second);
    let dir = scratch_dir("eg");
    let log = dir.join("scripts.txt");
    let shell = stub_shell(&dir, &format!("echo \"$4\" >> {}", log.display()), "");
    assert!(capture(&shell_builder(), &shell, LONG).is_some() && capture(&shell_builder(), &shell, LONG).is_some());
    let scripts = std::fs::read_to_string(&log).unwrap();
    let markers: Vec<&str> = scripts.lines().map(|line| line.split_whitespace().nth(1).unwrap().trim_end_matches(';')).collect();
    assert!(markers.len() == 2 && markers.iter().all(|m| is_marker(m)) && markers[0] != markers[1], "{markers:?}");
}

/// A startup file that prints a marker line and variables of its own, as an environment dump might.
#[test]
fn an_rc_file_that_prints_a_marker_and_variables_cannot_inject_them() {
    let dir = scratch_dir("eh");
    let fake = format!("printf '__DESIGNOS_ENV_BEGIN__\\nPATH=/fake/spoof\\0FAKE_ONLY=1\\0'; printf '{TEST_MARKER}\\nPATH=/fake/guess\\0FAKE_TOO=1\\0'");
    let env = capture(&shell_builder(), &stub_shell(&dir, &fake, ""), LONG).expect("the real marker is still found");
    assert_eq!(env_value(&env, "PATH").as_deref(), Some("/usr/bin:/bin"), "the real PATH, not the spoofed one");
    assert!(env_value(&env, "FAKE_ONLY").is_none() && env_value(&env, "FAKE_TOO").is_none());
}

#[test]
fn only_name_equals_value_entries_after_the_marker_are_taken() {
    let raw = format!("noise\x1b[?2004h{TEST_MARKER}\nA=1\0BAD\0B=two words\nand a line\0C=\0=x\0D=a=b\0");
    let pairs: Vec<(String, String)> =
        parse_capture(raw.as_bytes(), TEST_MARKER).expect("variables").iter().map(|(k, v)| (k.to_string_lossy().into(), v.to_string_lossy().into())).collect();
    let want = [("A", "1"), ("B", "two words\nand a line"), ("C", ""), ("D", "a=b")].map(|(k, v)| (k.to_string(), v.to_string()));
    assert_eq!(pairs, want, "nothing from before the marker, and only well-formed entries");
    assert!(parse_capture(b"A=1\0", TEST_MARKER).is_none(), "without the marker nothing is trusted");
    assert!(parse_capture(format!("{TEST_MARKER}\n").as_bytes(), TEST_MARKER).is_none(), "a marker with no variables after it");
    let raw = [TEST_MARKER.as_bytes(), b"\n", b"K=", &[0xFF, 0xFE], b"\0"].concat();
    assert_eq!(parse_capture(&raw, TEST_MARKER).unwrap()[0].1, OsString::from_vec(vec![0xFF, 0xFE]), "values that are not UTF-8 are kept");
}

#[test]
fn output_with_the_marker_line_twice_gives_no_variables() {
    let twice = format!("{TEST_MARKER}\nOLD=1\0{TEST_MARKER}\nA=1\0");
    assert!(parse_capture(twice.as_bytes(), TEST_MARKER).is_none(), "which marker is the real one cannot be told");
}

/// A startup file can learn the marker (it is in the script the shell was given) and print it too, early or from a
/// background job that outlives the dump. Either way the output holds the marker line twice, and the capture fails.
#[test]
fn a_second_marker_line_anywhere_in_the_output_fails_the_capture() {
    let learn = "m=${4#/bin/echo }; m=${m%%;*}";
    let printed = "printf '%s\\nPATH=/fake/spoof\\0LATE_ONLY=1\\0' \"$m\"";
    let (early, late, none) = (scratch_dir("q1"), scratch_dir("q2"), scratch_dir("q3"));
    assert!(capture(&shell_builder(), &stub_shell(&early, &format!("{learn}; {printed}"), ""), LONG).is_none(), "printed before the dump");
    let background_job = format!("( sleep 0.3; {printed} ) &");
    assert!(capture(&shell_builder(), &stub_shell(&late, learn, &background_job), LONG).is_none(), "printed after the dump, by a job that outlives the shell");
    assert!(capture(&shell_builder(), &stub_shell(&none, learn, ""), LONG).is_some(), "the same shell without the extra line is captured");
}

#[test]
fn a_name_that_repeats_keeps_its_last_value_for_get_and_for_a_child_alike() {
    let vars = parse_capture(format!("{TEST_MARKER}\nA=first\0B=1\0A=last\0").as_bytes(), TEST_MARKER).unwrap();
    let env = TabEnv::from_vars(vars);
    assert_eq!(env_value(&env, "A").as_deref(), Some("last"));
    assert_eq!(env.vars().filter(|(name, _)| *name == "A").count(), 1, "one entry per name");
    let seen = Command::new("/usr/bin/printenv").arg("A").env_clear().envs(env.vars()).output().unwrap();
    assert_eq!(String::from_utf8_lossy(&seen.stdout).trim(), "last", "a process started with the environment sees the same value");
}

#[test]
fn the_capture_runs_in_the_users_home_folder_never_where_studio_runs() {
    let (dir, home) = (scratch_dir("ei"), scratch_dir("ej"));
    let (here, studio_cwd) = (dir.join("pwd.txt"), std::env::current_dir().unwrap().canonicalize().unwrap());
    let shell = stub_shell(&dir, &format!("pwd -P > {}", here.display()), "");
    let ran_in = || std::path::PathBuf::from(std::fs::read_to_string(&here).unwrap().trim());
    let mut with_home = shell_builder();
    with_home.env("HOME", &*home);
    assert!(capture(&with_home, &shell, LONG).is_some());
    assert_eq!(ran_in(), home.canonicalize().unwrap(), "the builder's HOME");
    for missing in [None, Some("/no/such/folder"), Some("relative/dir")] {
        let mut builder = shell_builder();
        missing.into_iter().for_each(|dir| builder.env("HOME", dir));
        assert!(capture(&builder, &shell, LONG).is_some());
        let expected = std::env::home_dir().filter(|dir| dir.is_dir()).and_then(|dir| dir.canonicalize().ok()).unwrap_or_else(|| "/".into());
        assert!(ran_in() == expected && ran_in() != studio_cwd, "a missing or unusable HOME falls back to the user's home, else /: {missing:?}");
    }
}

#[test]
fn a_hung_shell_is_killed_with_everything_it_started_at_the_timeout() {
    let dir = scratch_dir("ec");
    let pid_file = dir.join("pid.txt");
    let shell = write_script(&dir, "shell", &format!("echo $$ > {}; sleep 30 & exec sleep 30", pid_file.display()));
    let started = std::time::Instant::now();
    assert!(capture(&shell_builder(), &shell, Duration::from_millis(600)).is_none());
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
fn a_shell_that_fails_or_prints_no_marker_gives_no_environment() {
    let (dir, b) = (scratch_dir("ed"), shell_builder());
    assert!(capture(&b, &stub_shell(&dir, "", "exit 3"), LONG).is_none(), "non-zero exit, although the output looks right");
    assert!(capture(&b, &write_script(&dir, "nomarker", "echo garbage"), LONG).is_none());
    assert!(capture(&b, &dir.join("absent"), LONG).is_none(), "no such shell");
}

/// A UUID would fix six of the 128 bits (version and variant), and a clock-based fallback would make it guessable.
#[test]
fn every_digit_of_the_marker_is_random_and_it_is_not_shaped_like_a_uuid() {
    let digits: Vec<Vec<u8>> = (0..64).map(|_| new_marker().unwrap().strip_prefix(MARKER_PREFIX).unwrap().bytes().take(32).collect()).collect();
    for at in 0..32 {
        assert!(digits.iter().any(|marker| marker[at] != digits[0][at]), "digit {at} is the same in 64 markers");
    }
}
