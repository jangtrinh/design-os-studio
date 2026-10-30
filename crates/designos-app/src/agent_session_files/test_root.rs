//! The sessions folder unit tests use: one per test process, in a folder Studio checks before it uses it.

use super::folder::{create_session_folder_in, require_own_real_folder};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// The Cargo target profile folder holds one `designos-test-data/<pid>/sessions` folder per test process, so no test process
/// writes into the real data folder or into another process's folder. The target profile is outside the agent-writable temp
/// folder and is removed with the Cargo target cache.
fn base_dir() -> PathBuf {
    let executable = std::env::current_exe().expect("the test executable path is available");
    let profile = executable.parent().and_then(Path::parent).expect("the test executable is below a Cargo target profile");
    profile.join("designos-test-data")
}

/// This process's sessions folder, made ready on first use (see [`prepare_base`]).
pub(super) fn test_sessions_root() -> PathBuf {
    static ROOT: OnceLock<PathBuf> = OnceLock::new();
    ROOT.get_or_init(|| {
        prepare_base(&base_dir(), std::process::id()).unwrap_or_else(|e| {
            panic!("the folder for the unit tests' session folders cannot be used, so no test can run: {}", crate::agent_session_files::with_next_step(&e))
        })
    })
    .clone()
}

/// Returns `<base>/<own_pid>/sessions`. `base` must first be a real folder of the user (made if it is missing); a link, a file
/// or a folder of another user is refused, and then nothing is made. This process's own folder is then made exclusively, and
/// whatever sits in its place (a link, a file, a folder of another user) is refused. Nothing is removed.
pub(super) fn prepare_base(base: &Path, own_pid: u32) -> io::Result<PathBuf> {
    if matches!(std::fs::symlink_metadata(base), Err(e) if e.kind() == io::ErrorKind::NotFound) {
        crate::agent_approval_channel::create_private_dir_all(base)?;
    }
    require_own_real_folder(None, base, base)?;
    // Work from the checked real path after the ownership check. A later replacement of the
    // spelling of `base` cannot redirect cleanup into an agent-writable link target.
    let base = base.canonicalize()?;
    cleanup_stale_process_roots(&base, own_pid, process_is_alive)?;
    let own_dir = base.join(own_pid.to_string());
    create_session_folder_in(None, &own_dir)?;
    Ok(own_dir.join("sessions"))
}

/// Removes dead test-process roots left below the checked Cargo profile.
///
/// Only direct numeric children are candidates. Real folders owned by this user whose PID is
/// still alive, non-PID entries, and links are left untouched. The caller supplies liveness so
/// tests can cover the cleanup without depending on a particular unused PID.
fn cleanup_stale_process_roots(base: &Path, own_pid: u32, is_alive: impl Fn(u32) -> bool) -> io::Result<usize> {
    let mut removed = 0;
    for entry in std::fs::read_dir(base)? {
        let entry = entry?;
        let path = entry.path();
        let Some(pid) = entry.file_name().to_str().and_then(|name| name.parse::<u32>().ok()).filter(|pid| *pid > 0) else {
            continue;
        };
        if pid == own_pid || is_alive(pid) {
            continue;
        }
        let meta = std::fs::symlink_metadata(&path)?;
        if meta.file_type().is_symlink() || !meta.is_dir() {
            continue;
        }
        if require_own_real_folder(None, &path, base).is_err() {
            continue;
        }
        if std::fs::remove_dir_all(&path).is_ok() {
            removed += 1;
        }
    }
    Ok(removed)
}

#[cfg(unix)]
fn process_is_alive(pid: u32) -> bool {
    super::is_owner_alive(pid, None)
}

#[cfg(windows)]
fn process_is_alive(pid: u32) -> bool {
    // nextest starts several test processes on Windows. `is_pid_alive` has no Windows
    // implementation, so ask the native process list and fail closed if it is unavailable.
    let filter = format!("PID eq {pid}");
    let Ok(output) = std::process::Command::new("tasklist").args(["/FI", &filter, "/NH", "/FO", "CSV"]).output() else {
        return true;
    };
    if !output.status.success() {
        return true;
    }
    let pid = format!("\"{pid}\"");
    String::from_utf8_lossy(&output.stdout).lines().any(|line| line.contains(&pid))
}

#[cfg(not(any(unix, windows)))]
fn process_is_alive(_pid: u32) -> bool {
    // Unknown platforms cannot safely prove that a PID is dead; retain the folder.
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_root(base: &Path, name: &str) -> PathBuf {
        let dir = base.join(name).join("sessions").join("p1-tab-1");
        std::fs::create_dir_all(&dir).expect("root");
        std::fs::write(dir.join("owner.json"), b"{}").expect("file");
        base.join(name)
    }

    fn scratch_base(tag: &str) -> PathBuf {
        let base = std::env::temp_dir().join(format!("designos-test-root-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).expect("base");
        base
    }

    #[test]
    fn stale_process_roots_are_removed_without_touching_live_or_foreign_entries() {
        let base = scratch_base("keep-all");
        let stale = make_root(&base, "999991");
        let live = make_root(&base, "999992");
        let foreign = make_root(&base, "sessions");

        let removed = cleanup_stale_process_roots(&base, 999_993, |pid| pid == 999_992).expect("cleanup");

        assert_eq!(removed, 1);
        assert!(!stale.exists(), "dead process root was not removed");
        assert!(live.join("sessions/p1-tab-1/owner.json").is_file(), "live process root was removed");
        assert!(foreign.join("sessions/p1-tab-1/owner.json").is_file(), "foreign entry was removed");
        let _ = std::fs::remove_dir_all(&base);
    }

    #[cfg(unix)]
    #[test]
    fn stale_process_root_links_are_not_followed_or_removed() {
        let scratch = scratch_base("root-link");
        let base = scratch.join("profile");
        std::fs::create_dir_all(&base).expect("profile");
        let target = scratch.join("target");
        std::fs::create_dir_all(&target).expect("target");
        std::fs::write(target.join("keep.txt"), b"keep").expect("sentinel");
        std::os::unix::fs::symlink(&target, base.join("999991")).expect("root link");

        let removed = cleanup_stale_process_roots(&base, 999_993, |_| false).expect("cleanup");

        assert_eq!(removed, 0);
        assert!(base.join("999991").is_symlink(), "the planted root link was removed");
        assert!(target.join("keep.txt").is_file(), "the link target was touched");
        let _ = std::fs::remove_dir_all(&scratch);
    }

    #[test]
    fn test_sessions_root_is_derived_from_the_cargo_target_profile() {
        let root = test_sessions_root();
        let executable = std::env::current_exe().expect("the test executable path is available");
        let profile = executable.parent().and_then(Path::parent).expect("the test executable is below a Cargo target profile");

        assert!(root.starts_with(profile), "test session root {} is not below Cargo profile {}", root.display(), profile.display());
        assert!(!root.starts_with(std::env::temp_dir()), "test session root {} is still under the temp folder", root.display());
        assert_eq!(root.file_name().and_then(|name| name.to_str()), Some("sessions"));
    }

    #[cfg(unix)]
    #[test]
    fn a_link_in_place_of_the_test_data_folder_is_refused_and_nothing_is_made_or_removed_through_it() {
        let scratch = scratch_base("base-link");
        let target = scratch.join("target");
        let (dead, other) = (make_root(&target, "999991"), make_root(&target, "not-a-pid"));
        let linked_base = scratch.join("designos test data");
        std::os::unix::fs::symlink(&target, &linked_base).expect("plant the link");

        let err = prepare_base(&linked_base, 999_993).expect_err("a link is not used as the test data folder");

        assert!(err.to_string().contains("is a link, not a folder"), "{err}");
        assert!(dead.join("sessions/p1-tab-1/owner.json").is_file(), "a folder was removed through the link");
        assert!(other.join("sessions/p1-tab-1/owner.json").is_file());
        assert!(!target.join("999993").exists(), "a folder was made through the link");
        let _ = std::fs::remove_dir_all(&scratch);
    }

    #[cfg(unix)]
    #[test]
    fn a_link_in_place_of_this_process_folder_is_refused_and_nothing_is_made_through_it() {
        let scratch = scratch_base("pid-link");
        let target = scratch.join("target");
        let base = scratch.join("designos test data");
        std::fs::create_dir_all(&target).expect("target");
        std::fs::create_dir_all(&base).expect("base");
        std::os::unix::fs::symlink(&target, base.join("999997")).expect("plant the link");

        let err = prepare_base(&base, 999_997).expect_err("a link in place of our folder is refused");

        assert!(err.to_string().contains("is a link, not a folder"), "{err}");
        assert_eq!(std::fs::read_dir(&target).expect("target").count(), 0, "something was made through the link");
        let _ = std::fs::remove_dir_all(&scratch);
    }

    #[test]
    fn a_missing_test_data_folder_is_made_and_a_file_in_its_place_is_refused() {
        let scratch = scratch_base("base-made");
        let base = scratch.join("designos test data");
        let root = prepare_base(&base, 999_996).expect("a missing base is made");
        assert_eq!(root, base.canonicalize().unwrap().join("999996").join("sessions"));
        assert!(base.is_dir());

        let file_base = scratch.join("a file");
        std::fs::write(&file_base, b"not a folder").expect("file");
        let err = prepare_base(&file_base, 999_996).expect_err("a file is not the test data folder");
        assert!(err.to_string().contains("is a file, not a folder"), "{err}");
        let _ = std::fs::remove_dir_all(&scratch);
    }
}
