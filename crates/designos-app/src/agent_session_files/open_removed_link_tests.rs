//! The agent can also delete the link it planted, between Studio's failed `openat` on it and Studio's look at the entry that says why.
//! Nothing is in the way by then, so the open is tried again; it is not reported as a failure to open.

use super::folder_tests::{only_entry_is_the_sentinel, scratch_dir, target_with_sentinel, Cleanup};
use super::no_follow::test_hooks::{clear_open_hook, set_open_hook, Step};
use super::test_support::made_session_folder;
use super::*;
use std::os::unix::fs::symlink;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Removes `link` once, after the first failed open of it.
fn remove_after_the_first_failed_open(link: &Path) -> Arc<AtomicBool> {
    let removed = Arc::new(AtomicBool::new(false));
    let (link_to_remove, flag) = (link.to_path_buf(), removed.clone());
    set_open_hook(link, move |step| {
        if step == Step::AfterFailedOpen && !flag.swap(true, Ordering::SeqCst) {
            let _ = std::fs::remove_file(&link_to_remove);
        }
    });
    removed
}

#[test]
fn a_link_the_agent_removes_after_the_failed_open_is_tried_again_not_reported_as_a_failure() {
    let (target, _target) = target_with_sentinel("removed-target");
    let (session, _session) = made_session_folder("removed");
    let folder = session.join(".gemini");
    symlink(&target, &folder).expect("plant the link");
    let removed = remove_after_the_first_failed_open(&folder);

    let result = atomic_write_session_file(&folder.join("x.json"), b"studio");

    clear_open_hook(&folder);
    assert!(removed.load(Ordering::SeqCst), "the agent removed its link after the open of it failed");
    result.expect("nothing is in the way any more, so the next attempt makes the folder and writes");
    assert_eq!(std::fs::read(folder.join("x.json")).expect("in the folder Studio made"), b"studio");
    assert!(only_entry_is_the_sentinel(&target), "the target received a byte");
}

#[test]
fn a_session_folder_link_the_agent_removes_after_the_failed_open_is_not_found_not_a_failure() {
    let (target, _target) = target_with_sentinel("removed-anchor-target");
    let lib_root = scratch_dir("removed-anchor-library");
    let _lib_root = Cleanup(vec![lib_root.clone()]);
    let folder = crate::agent_sessions::stable_session_dir(&lib_root, "file-1");
    std::fs::create_dir_all(folder.parent().expect("agent-sessions")).expect("agent-sessions folder");
    symlink(&target, &folder).expect("plant the link");
    let removed = remove_after_the_first_failed_open(&folder);

    let result = read_session_file(&folder.join("owner.json"));

    clear_open_hook(&folder);
    assert!(removed.load(Ordering::SeqCst));
    let e = result.expect_err("there is no owner marker in a folder that is gone");
    assert_eq!(e.kind(), std::io::ErrorKind::NotFound, "the folder is gone, which is not a failure to open it: {e}");
    assert!(only_entry_is_the_sentinel(&target), "the target received a byte");
}
