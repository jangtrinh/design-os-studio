//! A swapper that works in the session folder can restore a real folder between Studio's failed `openat` on the link it had
//! planted and Studio's look at the entry to say why it failed. Hooks put that swap at the exact point, so the race is not left
//! to chance. The failure of that look is a refusal of a link, never an untyped error, and nothing reaches the link's target.

use super::folder_tests::{only_entry_is_the_sentinel, scratch_dir, target_with_sentinel, Cleanup};
use super::no_follow::test_hooks::{clear_open_hook, set_open_hook, Step};
use super::refusal::{refusal_of, Refusal};
use super::test_support::made_session_folder;
use super::*;
use std::os::unix::fs::{symlink, DirBuilderExt};
use std::sync::atomic::{AtomicBool, Ordering};

fn is_link(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|meta| meta.file_type().is_symlink())
}

/// Makes `folder` a real folder again, the way a swapper that takes its link back does.
fn restore_folder(folder: &Path) {
    let _ = std::fs::remove_file(folder);
    let _ = std::fs::create_dir(folder);
}

#[test]
fn a_link_that_is_a_real_folder_again_before_it_is_looked_at_is_tried_again() {
    let (target, _target) = target_with_sentinel("race-retry-target");
    let (session, _session) = made_session_folder("race-retry");
    let folder = session.join(".gemini");
    symlink(&target, &folder).expect("plant the link");
    let restored = std::sync::Arc::new(AtomicBool::new(false));
    let hook_state = (folder.clone(), restored.clone());
    set_open_hook(&folder, move |step| {
        if step == Step::AfterFailedOpen && !hook_state.1.swap(true, Ordering::SeqCst) {
            restore_folder(&hook_state.0);
        }
    });

    let result = atomic_write_session_file(&folder.join("x.json"), b"studio");

    clear_open_hook(&folder);
    assert!(restored.load(Ordering::SeqCst), "the open of the link failed, and the hook restored the folder after it");
    result.expect("the folder is real again, so the next attempt opens it and writes");
    assert_eq!(std::fs::read(folder.join("x.json")).expect("in the real folder"), b"studio");
    assert!(only_entry_is_the_sentinel(&target), "the target received a byte");
}

#[test]
fn a_link_swapped_in_again_before_every_attempt_is_refused_as_a_link() {
    let (target, _target) = target_with_sentinel("race-exhaust-target");
    let (session, _session) = made_session_folder("race-exhaust");
    let folder = session.join(".gemini");
    std::fs::create_dir(&folder).expect("a real folder");
    let (hooked, link_to) = (folder.clone(), target.clone());
    set_open_hook(&folder, move |step| match step {
        Step::BeforeOpen if !is_link(&hooked) => {
            let _ = std::fs::remove_dir_all(&hooked);
            let _ = symlink(&link_to, &hooked);
        }
        Step::AfterFailedOpen => restore_folder(&hooked),
        Step::BeforeOpen => {}
    });

    let result = atomic_write_session_file(&folder.join("x.json"), b"studio");

    clear_open_hook(&folder);
    let e = result.expect_err("a link in the way at every attempt must be refused");
    assert_eq!(refusal_of(&e), Some(Refusal::Link), "not an untyped failure: {e}");
    assert!(e.to_string().contains(&folder.display().to_string()), "the line names the folder: {e}");
    assert!(only_entry_is_the_sentinel(&target), "the target received a byte");
}

#[test]
fn the_session_folder_itself_is_tried_again_and_refused_as_a_link_in_the_same_way() {
    let (target, _target) = target_with_sentinel("race-anchor-target");
    let lib_root = scratch_dir("race-anchor-library");
    let _lib_root = Cleanup(vec![lib_root.clone()]);
    let folder = crate::agent_sessions::stable_session_dir(&lib_root, "file-1");
    std::fs::create_dir_all(folder.parent().expect("agent-sessions")).expect("agent-sessions folder");
    symlink(&target, &folder).expect("plant the link");
    let restored = std::sync::Arc::new(AtomicBool::new(false));
    let hook_state = (folder.clone(), restored.clone());
    set_open_hook(&folder, move |step| {
        if step == Step::AfterFailedOpen && !hook_state.1.swap(true, Ordering::SeqCst) {
            let _ = std::fs::remove_file(&hook_state.0);
            let mut owner_only = std::fs::DirBuilder::new();
            let _ = owner_only.mode(0o700).create(&hook_state.0);
        }
    });

    let result = atomic_write_session_file(&folder.join("x.json"), b"studio");

    clear_open_hook(&folder);
    assert!(restored.load(Ordering::SeqCst));
    result.expect("the folder is real again, so the next attempt opens it and writes");
    assert!(only_entry_is_the_sentinel(&target), "the target received a byte");
}
