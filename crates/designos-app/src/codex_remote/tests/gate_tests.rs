//! The launch gate: which Codex builds take the card path, and the path budget.

use super::support::{scratch_dir, wait_until, write_script};
use crate::codex_remote::gate::{
    check_binary, judge_version, lookup_version, probe_version, rendezvous_path, resolve_in_path, warm, VersionLookup, VersionVerdict, MAX_RENDEZVOUS_BYTES,
    TESTED_CODEX,
};
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
    let join = |dirs: &[&PathBuf]| std::env::join_paths(dirs.iter().map(|d| d.as_os_str())).unwrap();
    assert_eq!(resolve_in_path(&join(&[&first, &second, &third])), Some(real.clone()), "a file without the execute bit is skipped");
    let relative: OsString = std::env::join_paths(["relative/dir", "", second.to_str().unwrap()]).unwrap();
    assert_eq!(resolve_in_path(&relative), Some(real.clone()), "relative and empty entries never resolve");
    assert_eq!(resolve_in_path(&join(&[&first])), None);
    let shim_dir = scratch_dir("pd");
    std::os::unix::fs::symlink(&real, shim_dir.join("codex")).unwrap();
    assert_eq!(resolve_in_path(&join(&[&shim_dir])), Some(shim_dir.join("codex")), "the path handed on is the one found, not its target");
}

fn known(bin: &std::path::Path) -> Option<Option<String>> {
    wait_until(Duration::from_secs(10), || matches!(lookup_version(bin), VersionLookup::Known(_)));
    match lookup_version(bin) {
        VersionLookup::Known(raw) => Some(raw),
        VersionLookup::Pending => None,
    }
}

#[test]
fn the_version_is_probed_off_the_caller_and_cached_per_binary_state() {
    let dir = scratch_dir("vc");
    let bin = write_script(&dir, "codex", "sleep 0.2; echo 'codex-cli 0.164.0'");
    assert_eq!(check_binary(&bin), Err(FallbackReason::VersionUnknown), "nothing is known until the probe has run, and asking does not block");
    assert!(matches!(lookup_version(&bin), VersionLookup::Pending | VersionLookup::Known(_)));
    assert_eq!(known(&bin).flatten().as_deref(), Some("codex-cli 0.164.0\n"));
    assert_eq!(check_binary(&bin), Ok(()));
    // An upgrade changes the file's size and time, so the old answer is not reused.
    write_script(&dir, "codex", "echo 'codex-cli 0.165.0'; # newer");
    warm(&bin);
    assert_eq!(known(&bin).flatten().as_deref(), Some("codex-cli 0.165.0\n"));
    assert_eq!(check_binary(&bin), Err(FallbackReason::VersionOutsideRange("0.165.0".into())));
}

#[test]
fn a_failing_hanging_or_missing_binary_is_an_unknown_version() {
    let dir = scratch_dir("vf");
    let failing = write_script(&dir, "codex", "echo 'codex-cli 0.164.0'; exit 3");
    assert_eq!(known(&failing), Some(None), "a failed run is not trusted even when it printed a version");
    assert_eq!(check_binary(&failing), Err(FallbackReason::VersionUnknown));
    let hanging = write_script(&dir, "hang", "exec sleep 30");
    let started = std::time::Instant::now();
    assert_eq!(probe_version(&hanging, Duration::from_millis(200)), None);
    assert!(started.elapsed() < Duration::from_secs(5), "the probe gives up and kills the process");
    assert_eq!(check_binary(&dir.join("absent")), Err(FallbackReason::VersionUnknown));
}
