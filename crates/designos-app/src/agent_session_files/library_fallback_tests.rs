//! The line a library file shows when its own session folder cannot be made (not refused: made): what happens to its agents
//! comes before what to do about the folder.

use super::folder_tests::{scratch_dir, Cleanup};
use super::test_support::{running_as_root, Writable};
use crate::agent_sessions::{acquire_stable_session_dir, stable_session_dir, LockOutcome};
use crate::live_sync_tests::unique_tab_id;
use std::os::unix::fs::PermissionsExt;

#[test]
fn a_library_file_whose_folder_cannot_be_made_says_what_happens_to_its_agents_before_what_to_do() {
    if running_as_root() {
        return; // root makes a folder in a read-only one
    }
    let lib_root = scratch_dir("fallback-unwritable");
    let mut cleanup = Cleanup(vec![lib_root.clone()]);
    let agent_sessions = lib_root.join("agent-sessions");
    std::fs::create_dir_all(&agent_sessions).expect("the agent-sessions folder");
    std::fs::set_permissions(&agent_sessions, std::fs::Permissions::from_mode(0o500)).expect("make it read-only");
    let _writable = Writable(agent_sessions);
    let stable = stable_session_dir(&lib_root, "file-1");

    let outcome = acquire_stable_session_dir(&lib_root, "file-1", "win-a", &format!("p{}-fallback-{}", std::process::id(), unique_tab_id()));

    let LockOutcome::Fallback { dir, message } = outcome else { panic!("a folder that cannot be made must not be acquired") };
    cleanup.0.push(dir);
    assert_eq!(
        message,
        format!(
            "Couldn't use this file's agent folder: {} can't be created: Permission denied (os error 13). \
             This file's agents start in a temporary folder that is not kept. \
             Check its permissions and free space, then reopen this design file.",
            stable.display()
        )
    );
}
