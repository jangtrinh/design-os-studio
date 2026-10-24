//! The launch gate: which Codex builds take the card path, and the path budget.

use super::support::{scratch_dir, tab_env, tab_env_with, wait_until, write_executable, write_script};
use crate::codex_remote::gate::{
    check_binary, judge_version, lookup_version, lookup_version_with, probe_version, rendezvous_path, resolve_in_path, warm, VersionLookup, VersionVerdict,
    MAX_RENDEZVOUS_BYTES, TESTED_CODEX,
};
use crate::codex_remote::shell_env::TabEnv;
use crate::codex_remote::FallbackReason;
use std::ffi::OsString;
use std::path::PathBuf;
use std::time::Duration;

#[test]
fn only_the_tested_release_line_is_supported() {
    assert_eq!(TESTED_CODEX, (0, 164));
    let outside = |v: &str| VersionVerdict::Outside(v.into());
    for (output, want) in [
        (Some("codex-cli 0.164.0\n"), VersionVerdict::Supported),
        (Some("codex-cli 0.164.9"), VersionVerdict::Supported),
        (Some("WARN npm notice\ncodex-cli 0.164.2\n"), VersionVerdict::Supported),
        (Some("codex-cli 0.163.0"), outside("0.163.0")),
        (Some("codex-cli 0.165.0"), outside("0.165.0")),
        (Some("codex-cli 1.164.0"), outside("1.164.0")),
        (Some("codex-cli 0.164.0-alpha.1"), outside("0.164.0-alpha.1")),
        (Some("codex-cli 0.164"), VersionVerdict::Unknown),
        (Some("codex 0.164.0"), VersionVerdict::Unknown),
        (Some("garbage"), VersionVerdict::Unknown),
        (Some(""), VersionVerdict::Unknown),
        (None, VersionVerdict::Unknown),
    ] {
        assert_eq!(judge_version(output), want, "{output:?}");
    }
}

#[test]
fn the_rendezvous_path_fits_the_unix_socket_limit_to_the_byte() {
    // `cx-7.sock` is 9 bytes after the folder and its separator.
    let fits = PathBuf::from("/".to_string() + &"a".repeat(MAX_RENDEZVOUS_BYTES - 11));
    let path = rendezvous_path(&fits, 7).expect("exactly the limit is allowed");
    assert_eq!(path.as_os_str().len(), MAX_RENDEZVOUS_BYTES);
    assert_eq!(path.file_name().unwrap(), "cx-7.sock");
    let too_long = PathBuf::from("/".to_string() + &"a".repeat(MAX_RENDEZVOUS_BYTES - 10));
    assert_eq!(rendezvous_path(&too_long, 7), Err(FallbackReason::PathTooLong));
}

#[test]
fn codex_is_resolved_in_path_order_to_an_absolute_executable() {
    let (first, second, third) = (scratch_dir("pa"), scratch_dir("pb"), scratch_dir("pc"));
    std::fs::write(first.join("codex"), "not executable").unwrap();
    let real = write_script(&second, "codex", "exit 0");
    write_script(&third, "codex", "exit 0");
    let join = |dirs: &[&std::path::Path]| std::env::join_paths(dirs.iter().map(|d| d.as_os_str())).unwrap();
    assert_eq!(resolve_in_path(&join(&[&first, &second, &third])), Some(real.clone()), "a file without the execute bit is skipped");
    let relative: OsString = std::env::join_paths(["relative/dir", "", second.to_str().unwrap()]).unwrap();
    assert_eq!(resolve_in_path(&relative), Some(real.clone()), "relative and empty entries never resolve");
    assert_eq!(resolve_in_path(&join(&[&first])), None);
    let shim_dir = scratch_dir("pd");
    std::os::unix::fs::symlink(&real, shim_dir.join("codex")).unwrap();
    assert_eq!(resolve_in_path(&join(&[&shim_dir])), Some(shim_dir.join("codex")), "the path handed on is the one found, not its target");
}

fn known(bin: &std::path::Path, env: &TabEnv) -> Option<Option<String>> {
    wait_until(Duration::from_secs(10), || matches!(lookup_version(bin, env), VersionLookup::Known(_)));
    match lookup_version(bin, env) {
        VersionLookup::Known(raw) => Some(raw),
        VersionLookup::Pending => None,
    }
}

#[test]
fn the_version_is_probed_off_the_caller_and_cached_per_binary_state() {
    let dir = scratch_dir("vc");
    let (bin, env) = (write_script(&dir, "codex", "sleep 0.2; echo 'codex-cli 0.164.0'"), tab_env());
    assert_eq!(check_binary(&bin, &env), Err(FallbackReason::VersionUnknown), "nothing is known until the probe has run, and asking does not block");
    assert!(matches!(lookup_version(&bin, &env), VersionLookup::Pending | VersionLookup::Known(_)));
    assert_eq!(known(&bin, &env).flatten().as_deref(), Some("codex-cli 0.164.0\n"));
    assert_eq!(check_binary(&bin, &env), Ok(()));
    // An upgrade changes the file's size and time, so the old answer is not reused.
    write_script(&dir, "codex", "echo 'codex-cli 0.165.0'; # newer");
    warm(&bin, &env);
    assert_eq!(known(&bin, &env).flatten().as_deref(), Some("codex-cli 0.165.0\n"));
    assert_eq!(check_binary(&bin, &env), Err(FallbackReason::VersionOutsideRange("0.165.0".into())));
}

#[test]
fn a_failing_hanging_or_missing_binary_is_an_unknown_version() {
    let dir = scratch_dir("vf");
    let (failing, env) = (write_script(&dir, "codex", "echo 'codex-cli 0.164.0'; exit 3"), tab_env());
    assert_eq!(known(&failing, &env), Some(None), "a failed run is not trusted even when it printed a version");
    assert_eq!(check_binary(&failing, &env), Err(FallbackReason::VersionUnknown));
    let hanging = write_script(&dir, "hang", "exec sleep 30");
    let started = std::time::Instant::now();
    assert_eq!(probe_version(&hanging, &env, Duration::from_millis(200)), None);
    assert!(started.elapsed() < Duration::from_secs(5), "the probe gives up and kills the process");
    assert_eq!(check_binary(&dir.join("absent"), &env), Err(FallbackReason::VersionUnknown));
}

#[test]
fn the_probe_runs_with_the_tabs_environment_so_an_env_shebang_finds_its_interpreter() {
    let (tools, bins) = (scratch_dir("ht"), scratch_dir("hb"));
    let helper = format!("cxhelper{}", std::process::id());
    write_script(&tools, &helper, "echo 'codex-cli 0.164.0'");
    let bin = write_executable(&bins, "codex", &format!("#!/usr/bin/env {helper}\n"));
    let tab_path = format!("{}:/usr/bin:/bin", tools.display());
    // Run it once, untimed (see `write_script`): the first run of a new file is slow.
    let _ = std::process::Command::new(&bin).env_clear().env("PATH", &tab_path).stdout(std::process::Stdio::null()).status();
    // A Finder-launched Studio has launchd's PATH, which lacks the folder that holds the interpreter (node, for npm).
    let studio = tab_env_with("PATH", "/usr/bin:/bin");
    let tab = tab_env_with("PATH", &tab_path);
    assert_eq!(known(&bin, &studio), Some(None), "without the tab's PATH the interpreter is not found");
    assert_eq!(check_binary(&bin, &studio), Err(FallbackReason::VersionUnknown));
    assert_eq!(known(&bin, &tab).flatten().as_deref(), Some("codex-cli 0.164.0\n"), "with it, the probe works");
    assert_eq!(check_binary(&bin, &tab), Ok(()));
    assert_eq!(check_binary(&bin, &studio), Err(FallbackReason::VersionUnknown), "the PATH is part of the cache key: one answer is not reused for the other");
}

#[test]
fn a_failed_probe_is_asked_again_after_the_delay_and_a_good_answer_is_kept() {
    let dir = scratch_dir("vr");
    let bin = write_script(&dir, "codex", &format!("[ -f {}/ok ] && echo 'codex-cli 0.164.0' || exit 1", dir.display()));
    let (env, retry) = (tab_env(), Duration::from_millis(400));
    let answer = |env: &TabEnv| lookup_version_with(&bin, env, retry);
    assert!(wait_until(Duration::from_secs(10), || matches!(answer(&env), VersionLookup::Known(None))), "the first probe fails");
    std::fs::write(dir.join("ok"), "").unwrap();
    assert!(matches!(answer(&env), VersionLookup::Known(None)), "not asked again before the delay");
    std::thread::sleep(retry + Duration::from_millis(100));
    assert!(wait_until(Duration::from_secs(10), || matches!(answer(&env), VersionLookup::Known(Some(_)))), "asked again once the delay has passed");
    std::fs::remove_file(dir.join("ok")).unwrap();
    std::thread::sleep(retry + Duration::from_millis(100));
    assert!(matches!(answer(&env), VersionLookup::Known(Some(_))), "a good answer is never asked again");
}

#[test]
fn a_probe_that_runs_out_of_time_takes_what_it_started_down_with_it() {
    let dir = scratch_dir("hk");
    let pid_file = dir.join("child.txt");
    let bin = write_script(&dir, "codex", &format!("sleep 30 & echo $! > {}; wait", pid_file.display()));
    assert_eq!(probe_version(&bin, &tab_env(), Duration::from_millis(600)), None);
    let child = std::fs::read_to_string(&pid_file).expect("the stub ran and started a child").trim().to_string();
    let alive = || std::process::Command::new("kill").args(["-0", &child]).stderr(std::process::Stdio::null()).status().is_ok_and(|status| status.success());
    let gone = wait_until(Duration::from_secs(3), || !alive());
    let _ = std::process::Command::new("kill").args(["-KILL", &child]).stderr(std::process::Stdio::null()).status();
    assert!(gone, "the child the version command started must not outlive the probe");
}
