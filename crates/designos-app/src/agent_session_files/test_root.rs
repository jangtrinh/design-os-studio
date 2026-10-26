//! The sessions folder unit tests use: one per test process, in a folder Studio checks before it uses it.

use super::folder::{create_session_folder_in, require_own_real_folder};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// `<temp>/designos test data` holds one `<pid>/sessions` folder per test process (with a space in its path, as macOS has in
/// `Application Support`), so no test process writes into the real data folder or into another process's folder. A test run
/// leaves its folder behind (the system clears unused temp files); nothing here ever removes a folder.
fn base_dir() -> PathBuf {
    std::env::temp_dir().join("designos test data")
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
/// whatever sits in its place (a link, a file, a folder of another user) is refused. Nothing is removed. `base` is checked once:
/// a link swapped in for it after this call can still receive the folders the tests make.
pub(super) fn prepare_base(base: &Path, own_pid: u32) -> io::Result<PathBuf> {
    if matches!(std::fs::symlink_metadata(base), Err(e) if e.kind() == io::ErrorKind::NotFound) {
        crate::agent_approval_channel::create_private_dir_all(base)?;
    }
    require_own_real_folder(None, base, base)?;
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
    fn no_folder_is_removed_when_the_test_folder_is_made_ready() {
        let base = scratch_base("keep-all");
        let roots = [make_root(&base, "999991"), make_root(&base, "999992"), make_root(&base, "999993"), make_root(&base, "sessions")];

        let root = prepare_base(&base, 999_993).expect("a real folder");

        assert_eq!(root, base.join("999993").join("sessions"));
        for kept in &roots {
            assert!(kept.join("sessions/p1-tab-1/owner.json").is_file(), "{} was removed", kept.display());
        }
        let _ = std::fs::remove_dir_all(&base);
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
        assert_eq!(root, base.join("999996").join("sessions"));
        assert!(base.is_dir());

        let file_base = scratch.join("a file");
        std::fs::write(&file_base, b"not a folder").expect("file");
        let err = prepare_base(&file_base, 999_996).expect_err("a file is not the test data folder");
        assert!(err.to_string().contains("is a file, not a folder"), "{err}");
        let _ = std::fs::remove_dir_all(&scratch);
    }
}
