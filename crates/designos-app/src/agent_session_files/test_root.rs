//! The sessions folder unit tests use: one per test process, and the removal of what dead processes left behind.

use super::folder::{create_session_folder_in, require_own_real_folder};
use std::io;
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
    ROOT.get_or_init(|| {
        prepare_base(&base_dir(), std::process::id(), may_be_alive).unwrap_or_else(|e| {
            panic!("the folder for the unit tests' session folders cannot be used, so no test can run: {}", crate::agent_session_files::with_next_step(&e))
        })
    })
    .clone()
}

/// Whether process `pid` may still run. Only a clean "no such process" from `kill -0` says no (the sweep must never remove
/// the folder of a process that runs), and where a process cannot be looked up every process may be alive.
fn may_be_alive(pid: u32) -> bool {
    cfg!(not(unix)) || may_be_alive_with(pid, || std::process::Command::new("/bin/kill").arg("-0").arg(pid.to_string()).output())
}

fn may_be_alive_with(pid: u32, run_kill: impl FnOnce() -> io::Result<std::process::Output>) -> bool {
    if pid == std::process::id() {
        return true;
    }
    match run_kill() {
        Ok(out) => out.status.success() || !String::from_utf8_lossy(&out.stderr).to_ascii_lowercase().contains("no such process"),
        Err(_) => true,
    }
}

/// Returns `<base>/<own_pid>/sessions`. `base` must first be a real folder of the user (made if it is missing); a link, a file
/// or a folder of another user is refused, and then nothing is swept or made. Then `base` is cleared: the folder of an earlier
/// process that had this id (nothing alive holds it but this process) and the folder of every other process `is_alive` says
/// is gone. A folder of a live process is never removed, and neither is a link or anything not named by a process id. Last,
/// this process's own folder is made, refusing whatever sits in its place.
pub(super) fn prepare_base(base: &Path, own_pid: u32, is_alive: impl Fn(u32) -> bool) -> io::Result<PathBuf> {
    if matches!(std::fs::symlink_metadata(base), Err(e) if e.kind() == io::ErrorKind::NotFound) {
        crate::agent_approval_channel::create_private_dir_all(base)?;
    }
    require_own_real_folder(None, base, base)?;
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
    // Our own folder is made here, exclusively: a link, a file or a folder of another user planted in its place is refused.
    let own_dir = base.join(own_pid.to_string());
    create_session_folder_in(None, &own_dir)?;
    Ok(own_dir.join("sessions"))
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

        let root = prepare_base(&base, 999_993, |pid| pid == 999_992).expect("a real folder");

        assert_eq!(root, own.join("sessions"));
        assert!(!dead.exists(), "the root of a process that is gone stays behind");
        assert!(live.join("sessions/p1-tab-1/owner.json").is_file(), "the root of a live process was removed");
        assert_eq!(std::fs::read_dir(&own).expect("our folder is made").count(), 0, "an earlier process with our id left folders that a test could trip over");
        assert!(legacy.join("sessions/p1-tab-1/owner.json").is_file(), "a folder that is not named by a process id is not ours to remove");
        let _ = std::fs::remove_dir_all(&base);
    }

    #[cfg(unix)]
    #[test]
    fn a_link_in_place_of_the_test_data_folder_is_not_swept_and_nothing_is_made_through_it() {
        let scratch = scratch_base("base-link");
        let target = scratch.join("target");
        let (dead, other) = (make_root(&target, "999991"), make_root(&target, "not-a-pid"));
        let linked_base = scratch.join("designos test data");
        std::os::unix::fs::symlink(&target, &linked_base).expect("plant the link");

        let err = prepare_base(&linked_base, 999_993, |_| false).expect_err("a link is not used as the test data folder");

        assert!(err.to_string().contains("is a link, not a folder"), "{err}");
        assert!(dead.join("sessions/p1-tab-1/owner.json").is_file(), "the sweep went through the link and removed a folder");
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

        let err = prepare_base(&base, 999_997, |_| false).expect_err("a link in place of our folder is refused");

        assert!(err.to_string().contains("is a link, not a folder"), "{err}");
        assert_eq!(std::fs::read_dir(&target).expect("target").count(), 0, "something was made through the link");
        let _ = std::fs::remove_dir_all(&scratch);
    }

    #[test]
    fn a_missing_test_data_folder_is_made_and_a_file_in_its_place_is_refused() {
        let scratch = scratch_base("base-made");
        let base = scratch.join("designos test data");
        let root = prepare_base(&base, 999_996, |_| false).expect("a missing base is made");
        assert_eq!(root, base.join("999996").join("sessions"));
        assert!(base.is_dir());

        let file_base = scratch.join("a file");
        std::fs::write(&file_base, b"not a folder").expect("file");
        let err = prepare_base(&file_base, 999_996, |_| false).expect_err("a file is not the test data folder");
        assert!(err.to_string().contains("is a file, not a folder"), "{err}");
        let _ = std::fs::remove_dir_all(&scratch);
    }

    #[cfg(unix)]
    #[test]
    fn only_a_clean_no_such_process_says_a_process_is_gone() {
        use std::os::unix::process::ExitStatusExt;
        let out = |code: i32, stderr: &str| {
            Ok(std::process::Output { status: std::process::ExitStatus::from_raw(code << 8), stdout: vec![], stderr: stderr.as_bytes().to_vec() })
        };
        assert!(may_be_alive_with(4_000_001, || out(0, "")), "kill -0 succeeded");
        assert!(!may_be_alive_with(4_000_001, || out(1, "kill: 4000001: No such process")), "a process that is gone");
        assert!(may_be_alive_with(4_000_001, || out(1, "kill: 4000001: Operation not permitted")), "exists, not ours to signal");
        assert!(may_be_alive_with(4_000_001, || out(1, "")), "a failure that says nothing is not proof of death");
        assert!(may_be_alive_with(4_000_001, || Err(std::io::Error::other("cannot start /bin/kill"))), "a kill that cannot start is not proof of death");
        assert!(may_be_alive_with(std::process::id(), || out(1, "No such process")), "this process is alive whatever kill says");
    }

    #[cfg(unix)]
    #[test]
    fn a_link_named_like_a_dead_process_is_not_followed() {
        let base = scratch_base("link");
        let target = make_root(&base, "target-not-a-pid");
        std::os::unix::fs::symlink(&target, base.join("999994")).expect("plant the link");

        prepare_base(&base, 999_995, |_| false).expect("a real folder");

        assert!(target.join("sessions/p1-tab-1/owner.json").is_file(), "the sweep went through a link");
        assert!(base.join("999994").symlink_metadata().is_ok(), "a link is left as it is");
        let _ = std::fs::remove_dir_all(&base);
    }
}
