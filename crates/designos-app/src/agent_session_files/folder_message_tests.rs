//! What a user reads when a session folder is refused: the cause, then what to do about it.

use super::folder::{create_session_folder_in, UNAVAILABLE_ROOT};
use super::folder_owner::own_uid;
use super::folder_tests::{make_parent_of, only_entry_is_the_sentinel, scratch_dir, target_with_sentinel, Cleanup};
use super::*;
use crate::live_sync_tests::unique_tab_id;
use crate::terminal::{AgentCliPreset, TerminalState};
use crate::test_timing::ScopedTerminalState;
use std::os::unix::fs::{symlink, MetadataExt};

fn shell_title() -> &'static str {
    AgentCliPreset::Shell.tab_title()
}

/// A tab whose session path holds what `plant` puts there.
fn tab_whose_session_path_holds(plant: impl FnOnce(&Path)) -> (ScopedTerminalState, PathBuf, Cleanup) {
    let st = ScopedTerminalState::new(TerminalState::for_tab(unique_tab_id()));
    let dir = st.session_dir.clone();
    let cleanup = make_parent_of(&dir);
    plant(&dir);
    (st, dir, cleanup)
}

/// The status message of a library file whose stable session path holds what `plant` puts there.
fn library_fallback_message_for(plant: impl FnOnce(&Path)) -> (String, PathBuf) {
    let lib_root = scratch_dir("message-library");
    let stable = crate::agent_sessions::stable_session_dir(&lib_root, "file-1");
    std::fs::create_dir_all(stable.parent().expect("agent-sessions")).expect("agent-sessions folder");
    plant(&stable);
    let outcome = crate::agent_sessions::acquire_stable_session_dir(&lib_root, "file-1", "win-a", &format!("p{}-msg-{}", std::process::id(), unique_tab_id()));
    let crate::agent_sessions::LockOutcome::Fallback { dir, message } = outcome else { panic!("a refused folder must not be acquired") };
    drop(Cleanup(vec![lib_root, dir]));
    (message, stable)
}

#[test]
fn a_tab_refused_a_link_says_to_delete_the_link() {
    let (target, _target) = target_with_sentinel("message-link-target");
    let (mut st, dir, _cleanup) = tab_whose_session_path_holds(|dir| symlink(&target, dir).expect("plant the link"));
    let err = st.spawn_tab(AgentCliPreset::Shell, None).expect_err("a link at the session path stops the tab");
    let expected = format!("Couldn't start {}: {} is a link, not a folder. Delete the link and try again.", shell_title(), dir.display());
    assert_eq!(err, expected);
    assert_eq!(st.status_msg.as_deref(), Some(expected.as_str()));
}

#[test]
fn a_tab_refused_a_file_says_to_delete_the_file() {
    let (mut st, dir, _cleanup) = tab_whose_session_path_holds(|dir| std::fs::write(dir, b"a file").expect("plant the file"));
    let err = st.spawn_tab(AgentCliPreset::Shell, None).expect_err("a file at the session path stops the tab");
    assert_eq!(err, format!("Couldn't start {}: {} is a file, not a folder. Delete the file at that path and try again.", shell_title(), dir.display()));
}

#[test]
fn a_tab_refused_a_folder_of_another_user_says_to_move_or_delete_it() {
    let scratch = scratch_dir("message-owner");
    let _cleanup = Cleanup(vec![scratch.clone()]);
    let foreign = Path::new("/usr");
    if !foreign.is_dir() || std::fs::metadata(foreign).expect("metadata").uid() == own_uid(|| Ok(scratch.clone())).expect("own id") {
        return; // not applicable when run as the owner of /usr
    }
    let mut st = TerminalState::for_tab(unique_tab_id());
    st.session_dir = foreign.to_path_buf();
    let err = st.spawn_tab(AgentCliPreset::Shell, None).expect_err("a folder of another user stops the tab");
    assert_eq!(err, format!("Couldn't start {}: /usr belongs to another user. Move or delete the folder and try again.", shell_title()));
}

#[test]
fn a_custom_cli_tab_refused_a_link_says_the_same() {
    let (target, _target) = target_with_sentinel("message-custom-target");
    let (mut st, dir, _cleanup) = tab_whose_session_path_holds(|dir| symlink(&target, dir).expect("plant the link"));
    let err = st.spawn_custom_cli_tab("My CLI", "true", None).expect_err("a link at the session path stops the tab");
    assert_eq!(err, format!("Couldn't start My CLI: {} is a link, not a folder. Delete the link and try again.", dir.display()));
    assert_eq!(st.status_msg.as_deref(), Some(err.as_str()));
}

#[test]
fn a_library_file_says_what_happens_to_its_agents_and_what_to_delete() {
    let (target, _target) = target_with_sentinel("message-library-target");
    let (message, stable) = library_fallback_message_for(|stable| symlink(&target, stable).expect("plant the link"));
    assert_eq!(
        message,
        format!(
            "Couldn't use this file's agent folder: {} is a link, not a folder. This file's agents start in a temporary folder that is not kept. Delete the link and reopen this design file.",
            stable.display()
        )
    );

    let (message, stable) = library_fallback_message_for(|stable| std::fs::write(stable, b"a file").expect("plant the file"));
    assert_eq!(
        message,
        format!(
            "Couldn't use this file's agent folder: {} is a file, not a folder. This file's agents start in a temporary folder that is not kept. Delete the file at that path and reopen this design file.",
            stable.display()
        )
    );
}

#[test]
fn the_status_line_of_a_failed_snapshot_says_what_to_do() {
    let (target, _target) = target_with_sentinel("message-snapshot-target");
    let (mut st, dir, _cleanup) = tab_whose_session_path_holds(|dir| symlink(&target, dir).expect("plant the link"));
    let mut doc = designos_doc::Document::new("Planted");
    let page = doc.add_page("Page 1");

    st.sync_live_file_context(&doc, page, &[], None);
    st.flush_live_sync();

    let d = dir.display();
    assert_eq!(st.status_msg, Some(format!("Failed to create session dir {d}: {d} is a link, not a folder. Delete the link and try again")));
}

#[test]
fn a_library_file_whose_temporary_folder_is_refused_too_says_so_with_a_next_step() {
    let (target, _target) = target_with_sentinel("message-double-target");
    let fallback_id = format!("p{}-msg-{}", std::process::id(), unique_tab_id());
    let fallback = ephemeral_session_dir(&fallback_id);
    let mut cleanup = make_parent_of(&fallback);
    cleanup.0.push(fallback.clone());
    symlink(&target, &fallback).expect("plant the link in the fallback folder");
    let lib_root = scratch_dir("message-double-library");
    cleanup.0.push(lib_root.clone());
    let stable = crate::agent_sessions::stable_session_dir(&lib_root, "file-1");
    std::fs::create_dir_all(stable.parent().expect("agent-sessions")).expect("agent-sessions folder");
    symlink(&target, &stable).expect("plant the link in the library folder");

    let outcome = crate::agent_sessions::acquire_stable_session_dir(&lib_root, "file-1", "win-a", &fallback_id);

    let crate::agent_sessions::LockOutcome::Fallback { message, .. } = outcome else { panic!("a refused folder must not be acquired") };
    assert_eq!(
        message,
        format!(
            "Couldn't use this file's agent folder: {} is a link, not a folder. Delete the link and reopen this design file. \
             The temporary session folder can't be used either: {} is a link, not a folder. Delete the link and try again.",
            stable.display(),
            fallback.display()
        )
    );
}

#[test]
fn sending_a_selection_to_the_terminal_says_what_to_do_when_the_session_folder_is_refused() {
    let (target, _target) = target_with_sentinel("message-selection-target");
    let (mut st, dir, _cleanup) = tab_whose_session_path_holds(|dir| symlink(&target, dir).expect("plant the link"));
    let mut doc = designos_doc::Document::new("Planted");
    let page = doc.add_page("Page 1");
    let frame = doc.alloc_id();
    doc.insert(page, designos_doc::Node::frame(frame, designos_doc::Rect::new(0.0, 0.0, 10.0, 10.0))).expect("frame");

    let err = crate::terminal::export_selection_and_comments_to_pty(&mut st, &doc, page, &[frame], None).expect_err("a planted link is refused");

    let spec = st.selection_spec_path.display().to_string();
    assert_eq!(err, format!("write {spec}: {} is a link, not a folder. Delete the link and try again", dir.display()));
    assert!(only_entry_is_the_sentinel(&target), "the selection went through the link");
}

#[test]
fn the_line_of_a_taken_lock_says_the_temporary_folder_cannot_be_used_without_either() {
    let (target, _target) = target_with_sentinel("message-lock-target");
    let fallback_id = format!("p{}-msg-{}", std::process::id(), unique_tab_id());
    let fallback = ephemeral_session_dir(&fallback_id);
    let mut cleanup = make_parent_of(&fallback);
    cleanup.0.push(fallback.clone());
    symlink(&target, &fallback).expect("plant the link in the fallback folder");
    let lib_root = scratch_dir("message-lock-library");
    cleanup.0.push(lib_root.clone());
    let stable = crate::agent_sessions::stable_session_dir(&lib_root, "file-1");
    ensure_session_dir(&stable).expect("the file's own folder");
    let lock = crate::agent_sessions::SessionLock { pid: 1, window: "another-window".into(), started_at: 0 };
    std::fs::write(stable.join("session.lock"), serde_json::to_vec_pretty(&lock).expect("lock json")).expect("another window's lock");

    let outcome = crate::agent_sessions::acquire_stable_session_dir_with(&lib_root, "file-1", "win-b", &fallback_id, |_| true);

    let crate::agent_sessions::LockOutcome::Fallback { message, .. } = outcome else { panic!("a lock another window holds must not be taken") };
    assert_eq!(
        message,
        format!(
            "{} The temporary session folder can't be used: {} is a link, not a folder. Delete the link and try again.",
            crate::agent_sessions::ANOTHER_WINDOW_OWNS_SESSION_MSG,
            fallback.display()
        )
    );
}

#[test]
fn a_missing_data_folder_says_what_to_check() {
    let err = create_session_folder_in(None, &Path::new(UNAVAILABLE_ROOT).join("p1-tab-1")).expect_err("no data folder, no session folder");
    assert_eq!(
        err.to_string(),
        "Studio has no private data folder for agent sessions (its user data folder is missing or inside a temporary folder). Make sure your home folder exists, then restart Studio"
    );
}
