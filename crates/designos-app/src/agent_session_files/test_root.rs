//! The sessions folder unit tests use: one per test process, and the removal of what dead processes left behind.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// `<temp>/designos test data` holds one `<pid>/sessions` folder per test process (with a space in its path, as macOS has in
/// `Application Support`), so no test process writes into the real data folder or into another process's folder.
fn base_dir() -> PathBuf {
    std::env::temp_dir().join("designos test data")
}

/// This process's sessions folder. On first use it clears what earlier processes left behind (see [`prepare_base`]).
pub(super) fn test_sessions_root() -> PathBuf {
    static ROOT: OnceLock<PathBuf> = OnceLock::new();
    // Where a process cannot be looked up (`is_pid_alive` says no to every other id), no other process's folder is removed.
    let is_alive = |pid: u32| cfg!(not(unix)) || crate::agent_sessions::is_pid_alive(pid);
    ROOT.get_or_init(|| prepare_base(&base_dir(), std::process::id(), is_alive)).clone()
}

/// Returns `<base>/<own_pid>/sessions` after clearing `base`: the folder of an earlier process that had this id (nothing alive
/// holds it but this process) and the folder of every other process `is_alive` says is gone. A folder of a live process is never
/// removed, and neither is a link or anything not named by a process id.
pub(super) fn prepare_base(base: &Path, own_pid: u32, is_alive: impl Fn(u32) -> bool) -> PathBuf {
    for entry in std::fs::read_dir(base).into_iter().flatten().flatten() {
        // `file_type` does not follow a link, so a link is never taken for a folder.
        let is_folder = entry.file_type().is_ok_and(|kind| kind.is_dir());
        let name = entry.file_name();
        let pid = name.to_str().filter(|name| name.bytes().all(|b| b.is_ascii_digit())).and_then(|name| name.parse::<u32>().ok()).filter(|pid| *pid > 0);
        if let (true, Some(pid)) = (is_folder, pid) {
            if pid == own_pid || !is_alive(pid) {
                let _ = std::fs::remove_dir_all(entry.path());
            }
        }
    }
    base.join(own_pid.to_string()).join("sessions")
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
    fn a_dead_process_root_is_removed_a_live_one_kept_and_our_own_id_starts_empty() {
        let base = scratch_base("sweep");
        let (dead, live, own, legacy) = (make_root(&base, "999991"), make_root(&base, "999992"), make_root(&base, "999993"), make_root(&base, "sessions"));

        let root = prepare_base(&base, 999_993, |pid| pid == 999_992);

        assert_eq!(root, own.join("sessions"));
        assert!(!dead.exists(), "the root of a process that is gone stays behind");
        assert!(live.join("sessions/p1-tab-1/owner.json").is_file(), "the root of a live process was removed");
        assert!(!own.exists(), "an earlier process with our id left folders that a test could trip over");
        assert!(legacy.join("sessions/p1-tab-1/owner.json").is_file(), "a folder that is not named by a process id is not ours to remove");
        let _ = std::fs::remove_dir_all(&base);
    }

    #[cfg(unix)]
    #[test]
    fn a_link_named_like_a_dead_process_is_not_followed() {
        let base = scratch_base("link");
        let target = make_root(&base, "target-not-a-pid");
        std::os::unix::fs::symlink(&target, base.join("999994")).expect("plant the link");

        prepare_base(&base, 999_995, |_| false);

        assert!(target.join("sessions/p1-tab-1/owner.json").is_file(), "the sweep went through a link");
        assert!(base.join("999994").symlink_metadata().is_ok(), "a link is left as it is");
        let _ = std::fs::remove_dir_all(&base);
    }
}
