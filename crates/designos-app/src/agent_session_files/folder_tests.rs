//! A session folder is a place the agent of its tab is allowed to write, so Studio must be the one who made it:
//! an existing link, file or foreign folder at that path is refused and nothing is written through it.

use super::*;
use crate::live_sync_tests::unique_tab_id;
use crate::terminal::{AgentCliPreset, TerminalState};
use crate::test_timing::ScopedTerminalState;
use std::os::unix::fs::{symlink, PermissionsExt};

/// Removes what a test made, links first (a link is removed itself, never what it points at).
pub(super) struct Cleanup(pub(super) Vec<PathBuf>);

impl Drop for Cleanup {
    fn drop(&mut self) {
        for path in &self.0 {
            if std::fs::remove_file(path).is_err() {
                let _ = std::fs::remove_dir_all(path);
            }
        }
    }
}

pub(super) fn scratch_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("designos-folder-test-{tag}-{}", unique_tab_id()));
    std::fs::create_dir_all(&dir).expect("scratch folder");
    dir
}

/// Makes the folder that holds `dir` the way Studio does (by making a folder of its own next to `dir`), so a link can be
/// planted at `dir`.
pub(super) fn make_parent_of(dir: &Path) -> Cleanup {
    let name = dir.file_name().and_then(|n| n.to_str()).unwrap_or("p0");
    let neighbour = dir.with_file_name(format!("{name}-neighbour"));
    ensure_session_dir(&neighbour).expect("the neighbour folder is made");
    Cleanup(vec![neighbour])
}

/// The folder an agent of a tab that does not exist yet would get.
fn next_ephemeral_session_dir() -> (PathBuf, Cleanup) {
    let dir = TerminalState::for_tab(unique_tab_id()).session_dir.clone();
    let mut cleanup = make_parent_of(&dir);
    cleanup.0.push(dir.clone());
    (dir, cleanup)
}

pub(super) fn only_entry_is_the_sentinel(target: &Path) -> bool {
    let names: Vec<_> = std::fs::read_dir(target).expect("read the target").flatten().map(|e| e.file_name()).collect();
    names == [std::ffi::OsString::from("keep.txt")]
}

pub(super) fn target_with_sentinel(tag: &str) -> (PathBuf, Cleanup) {
    let target = scratch_dir(tag);
    std::fs::write(target.join("keep.txt"), b"mine").expect("sentinel");
    (target.clone(), Cleanup(vec![target]))
}

#[test]
fn a_link_planted_at_the_next_ephemeral_session_path_is_refused() {
    let (target, _target_cleanup) = target_with_sentinel("ephemeral-target");
    let (dir, _cleanup) = next_ephemeral_session_dir();
    symlink(&target, &dir).expect("plant the link");

    let result = ensure_session_dir(&dir);

    assert!(result.is_err(), "a link at the session path must be refused");
    assert!(!target.join("owner.json").exists(), "the owner marker went through the link");
    assert!(only_entry_is_the_sentinel(&target), "something was written into the link's target");
}

#[test]
fn a_link_planted_at_a_library_session_path_is_refused() {
    let (target, _target_cleanup) = target_with_sentinel("library-target");
    let lib_root = scratch_dir("library-root");
    let _lib_cleanup = Cleanup(vec![lib_root.clone()]);
    let dir = crate::agent_sessions::stable_session_dir(&lib_root, "file-1");
    std::fs::create_dir_all(dir.parent().expect("agent-sessions")).expect("agent-sessions folder");
    symlink(&target, &dir).expect("plant the link");

    assert!(ensure_session_dir(&dir).is_err(), "a link at a library session path must be refused");
    assert!(only_entry_is_the_sentinel(&target), "something was written into the link's target");
}

#[test]
fn a_link_planted_at_a_library_session_path_gets_no_lock_file() {
    let (target, _target_cleanup) = target_with_sentinel("lock-target");
    let lib_root = scratch_dir("lock-root");
    let mut cleanup = Cleanup(vec![lib_root.clone()]);
    let dir = crate::agent_sessions::stable_session_dir(&lib_root, "file-1");
    std::fs::create_dir_all(dir.parent().expect("agent-sessions")).expect("agent-sessions folder");
    symlink(&target, &dir).expect("plant the link");

    let outcome = crate::agent_sessions::acquire_stable_session_dir(&lib_root, "file-1", "win-a", &format!("p{}-fold-{}", std::process::id(), unique_tab_id()));
    if let crate::agent_sessions::LockOutcome::Fallback { dir: fallback, .. } = &outcome {
        cleanup.0.push(fallback.clone());
    }

    assert!(!matches!(outcome, crate::agent_sessions::LockOutcome::Acquired(_)), "the planted folder was taken as the session folder");
    assert!(only_entry_is_the_sentinel(&target), "the lock file went through the link");
}

#[test]
fn a_file_at_a_session_path_is_refused() {
    let scratch = scratch_dir("file-at-path");
    let _cleanup = Cleanup(vec![scratch.clone()]);
    let dir = scratch.join("agent-sessions").join("file-1");
    std::fs::create_dir_all(dir.parent().expect("agent-sessions")).expect("agent-sessions folder");
    std::fs::write(&dir, b"a file").expect("plant the file");

    assert!(ensure_session_dir(&dir).is_err(), "a file at the session path must be refused");
    assert_eq!(std::fs::read(&dir).expect("the file stays"), b"a file");
}

#[test]
fn an_existing_real_session_folder_is_accepted_again() {
    let (dir, _cleanup) = next_ephemeral_session_dir();
    ensure_session_dir(&dir).expect("the first sync makes the folder");
    let marker = std::fs::read(dir.join("owner.json")).expect("owner marker");
    ensure_session_dir(&dir).expect("a re-sync accepts the folder it made");
    assert_eq!(std::fs::read(dir.join("owner.json")).expect("owner marker"), marker, "a re-sync keeps the marker");

    let scratch = scratch_dir("older-library-folder");
    let _scratch_cleanup = Cleanup(vec![scratch.clone()]);
    let older = scratch.join("agent-sessions").join("file-1");
    std::fs::create_dir_all(&older).expect("a folder an earlier version made");
    ensure_session_dir(&older).expect("a real folder of the user is accepted");
}

#[test]
fn new_session_folders_are_private_to_their_owner() {
    let mode_of = |path: &Path| std::fs::metadata(path).expect("metadata").permissions().mode() & 0o777;

    let (dir, _cleanup) = next_ephemeral_session_dir();
    ensure_session_dir(&dir).expect("ephemeral folder");
    assert_eq!(mode_of(&dir), 0o700, "ephemeral session folder");
    assert_eq!(mode_of(dir.parent().expect("root")), 0o700, "the folder that holds the ephemeral sessions");

    let scratch = scratch_dir("private-library");
    let _scratch_cleanup = Cleanup(vec![scratch.clone()]);
    let library = scratch.join("agent-sessions").join("file-1");
    ensure_session_dir(&library).expect("library folder");
    assert_eq!(mode_of(&library), 0o700, "library session folder");
}

#[test]
fn a_tab_does_not_start_in_a_planted_link_and_says_why() {
    let (target, _target_cleanup) = target_with_sentinel("spawn-target");
    let mut st = ScopedTerminalState::new(TerminalState::for_tab(unique_tab_id()));
    let dir = st.session_dir.clone();
    let _neighbour = make_parent_of(&dir);
    symlink(&target, &dir).expect("plant the link");

    let result = st.spawn_tab(AgentCliPreset::Shell, None);

    assert!(result.is_err(), "a tab must not start in a planted folder");
    assert!(st.tabs.is_empty(), "no tab was added");
    let message = st.status_msg.clone().expect("the tab says why it did not start");
    assert!(message.starts_with("Couldn't start"), "an honest error, not a success: {message}");
    assert!(only_entry_is_the_sentinel(&target), "session files went into the link's target");

    st.shutdown();
    assert!(target.join("keep.txt").is_file(), "closing the window removed what the link pointed at");
}

#[test]
fn a_library_file_whose_folder_is_a_planted_link_runs_in_the_private_fallback_and_writes_nothing_through_the_link() {
    let (target, _target_cleanup) = target_with_sentinel("bind-target");
    let lib_root = scratch_dir("bind-root");
    let window = format!("p{}-fold-{}", std::process::id(), unique_tab_id());
    let _cleanup = Cleanup(vec![lib_root.clone(), ephemeral_session_dir(&window)]);
    let stable = crate::agent_sessions::stable_session_dir(&lib_root, "file-1");
    std::fs::create_dir_all(stable.parent().expect("agent-sessions")).expect("agent-sessions folder");
    symlink(&target, &stable).expect("plant the link");

    let mut st = ScopedTerminalState::new(TerminalState::for_tab(unique_tab_id()));
    st.bind_library_file(&lib_root, "file-1", &window);

    let message = st.status_msg.clone().expect("says why the file's folder is not used");
    assert!(message.starts_with("Couldn't use this file's agent folder"), "{message}");
    assert!(st.lock_owner.is_none(), "a lock was taken on the planted folder");
    assert!(is_ephemeral_session_dir(&st.session_dir), "the tab runs in Studio's own folder: {}", st.session_dir.display());

    st.spawn_tab(AgentCliPreset::Shell, None).expect("a shell starts in the private fallback");
    st.close_tab(0);
    st.shutdown();
    assert!(only_entry_is_the_sentinel(&target), "something went through the link into its target");
}
