//! A scratch session folder outlives its tab as a folder holding only the mark of the closed tab, so an agent whose MCP server
//! outlives the tab is told "already closed" and can tell that from a folder that is simply gone.

use super::*;
use designos_doc::live_patch::CloseIntent;

/// What a closed scratch folder holds: the owner marker and the mark of the closed tab, nothing else.
const CLOSED_SCRATCH_FOLDER: [&str; 2] = ["owner.json", "tab-closed.json"];

fn scratch_state_with_a_folder() -> TerminalState {
    let st = TerminalState::default();
    assert!(crate::agent_session_files::is_ephemeral_session_dir(&st.session_dir), "a scratch folder: {}", st.session_dir.display());
    let _ = std::fs::remove_dir_all(&st.session_dir);
    crate::agent_session_files::ensure_session_dir(&st.session_dir).expect("create the folder");
    atomic_write_session_file(&st.session_dir.join("doc-patch.jsonl"), b"").expect("the agent's log");
    atomic_write_session_file(&st.session_dir.join("canvas.designos"), b"{}").expect("the published canvas");
    std::fs::create_dir(st.session_dir.join("nested")).expect("a nested folder");
    st
}

fn names_in(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> =
        std::fs::read_dir(dir).map(|rd| rd.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect()).unwrap_or_default();
    names.sort();
    names
}

fn mcp_on(dir: &Path) -> (designos_mcp::server::Session, PathBuf) {
    let workspace = std::env::temp_dir().join(format!("designos-closed-mark-{}-{}", std::process::id(), crate::live_sync_tests::unique_tab_id()));
    std::fs::create_dir_all(&workspace).unwrap();
    let mut session = designos_mcp::server::Session::with_session_dir(&workspace, dir).expect("an MCP session on the folder");
    session.set_close_ack_timeout(std::time::Duration::from_secs(10));
    (session, workspace)
}

#[test]
fn a_scratch_tab_that_shuts_down_leaves_only_the_mark_of_its_closed_tab() {
    let mut st = scratch_state_with_a_folder();
    let dir = st.session_dir.clone();
    st.shutdown();
    assert_eq!(names_in(&dir), CLOSED_SCRATCH_FOLDER);
    let _ = std::fs::remove_dir_all(&dir);
}

/// The folder is emptied in place, never removed and made again, so it is there for the whole shutdown. (That the mark is in it
/// before anything is removed is the next test.)
#[cfg(unix)]
#[test]
fn the_scratch_folder_is_emptied_in_place_not_removed_and_made_again() {
    use std::os::unix::fs::MetadataExt;
    let mut st = scratch_state_with_a_folder();
    let dir = st.session_dir.clone();
    let before = std::fs::metadata(&dir).unwrap().ino();
    st.shutdown();
    assert_eq!(std::fs::metadata(&dir).unwrap().ino(), before, "the same folder, not a new one");
    assert!(dir.join(CloseIntent::TAB_CLOSED_FILE).is_file());
    let _ = std::fs::remove_dir_all(&dir);
}

/// Each entry removed, with what its folder held just before it went.
#[cfg(unix)]
type SeenAtRemoval = std::sync::Arc<std::sync::Mutex<Vec<(String, Vec<String>)>>>;

/// The mark is in the folder before the first file of the session is removed, so an agent's call that lands while the folder is
/// being emptied already finds it. Called directly: the shutdown writes the mark again afterwards, which would hide a late one.
#[cfg(unix)]
#[test]
fn the_mark_is_in_the_folder_before_anything_in_it_is_removed() {
    let st = scratch_state_with_a_folder();
    let dir = st.session_dir.clone();
    let seen = SeenAtRemoval::default();
    for name in ["doc-patch.jsonl", "canvas.designos", "nested"] {
        let (seen, dir_at) = (seen.clone(), dir.clone());
        crate::agent_session_files::set_before_remove_hook(&dir.join(name), Box::new(move || seen.lock().unwrap().push((name.to_string(), names_in(&dir_at)))));
    }
    remove_scratch_dir_keeping_the_close_answer(&dir, true);
    let seen = seen.lock().unwrap().clone();
    assert_eq!(seen.len(), 3, "every file of the session was removed through the routed helpers: {seen:?}");
    for (removed, held) in &seen {
        assert!(held.iter().any(|n| n == CloseIntent::TAB_CLOSED_FILE), "the mark was not there when {removed} was removed: {held:?}");
    }
    assert_eq!(names_in(&dir), CLOSED_SCRATCH_FOLDER, "the function itself leaves the mark, not a later step");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_scratch_tab_that_shuts_down_keeps_the_answer_to_a_close_and_the_mark() {
    let mut st = scratch_state_with_a_folder();
    let dir = st.session_dir.clone();
    atomic_write_session_file(&dir.join(CloseIntent::RESULT_FILE), b"{\"closed\":true}").expect("the answer");
    st.shutdown();
    assert_eq!(names_in(&dir), ["close-result.json", "owner.json", "tab-closed.json"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_tab_that_never_had_a_folder_leaves_none() {
    let mut st = TerminalState::default();
    let dir = st.session_dir.clone();
    let _ = std::fs::remove_dir_all(&dir);
    st.shutdown();
    assert!(!dir.exists(), "nothing was running here, so there is nobody to tell: {:?}", names_in(&dir));
}

#[test]
fn removing_the_scratch_folder_while_the_tab_stays_leaves_no_mark() {
    let mut st = scratch_state_with_a_folder();
    let dir = st.session_dir.clone();
    // The last agent tab closes: the project tab is still open, so its folder is not marked closed.
    st.cleanup_ephemeral_session_dir(false);
    assert!(!dir.join(CloseIntent::TAB_CLOSED_FILE).exists(), "the project tab is still open: {:?}", names_in(&dir));
    let _ = std::fs::remove_dir_all(&dir);
}

/// Opening a library file in the tab (or saving the tab's work into the library) ends the scratch session: the old folder goes, the
/// mark with it, so an agent that outlived the session is told the folder is gone, which can never be retried.
#[test]
fn rebinding_a_scratch_tab_to_a_library_file_answers_the_old_session_with_a_missing_folder() {
    let mut st = scratch_state_with_a_folder();
    let old = st.session_dir.clone();
    let (mut session, workspace) = mcp_on(&old);
    let lib = std::env::temp_dir().join(format!("designos-closed-mark-lib-{}-{}", std::process::id(), crate::live_sync_tests::unique_tab_id()));
    std::fs::create_dir_all(&lib).unwrap();
    st.bind_library_file(&lib, "file-rebind", &format!("p{}-wrebind-{}", std::process::id(), crate::live_sync_tests::unique_tab_id()));
    assert_ne!(st.session_dir, old, "the terminal moved to another folder");
    assert!(!old.exists(), "the old scratch folder is gone: {:?}", names_in(&old));
    let started = std::time::Instant::now();
    let err = session.call("designos_close_document", serde_json::json!({"unsaved": "discard"})).map_err(|e| e.to_json_value()).expect_err("no editor to ask");
    assert!(started.elapsed() < std::time::Duration::from_secs(2), "answered at once");
    assert_eq!(err["code"], "EDITOR_NOT_NOTIFIED", "{err}");
    assert_eq!(err["safe_to_retry"], false, "{err}");
    assert_eq!(err["message"], "The session folder is gone, so the editor cannot be asked.", "{err}");
    st.shutdown();
    let _ = std::fs::remove_dir_all(&workspace);
    let _ = std::fs::remove_dir_all(&lib);
}

/// A new session that gets a scratch folder name an earlier process left (the process id was recycled) must not inherit that tab's
/// mark or its owner marker.
#[test]
fn a_new_session_does_not_inherit_the_mark_or_the_owner_of_a_folder_an_earlier_process_left() {
    let st = TerminalState::default();
    let dir = st.session_dir.clone();
    let _ = std::fs::remove_dir_all(&dir);
    crate::agent_session_files::ensure_session_dir(&dir).expect("create the folder");
    let stale = crate::agent_session_files::OwnerMarker { pid: std::process::id().wrapping_add(1), started_ms: 1 };
    atomic_write_session_file(&dir.join("owner.json"), &serde_json::to_vec(&stale).unwrap()).expect("an earlier process's owner marker");
    atomic_write_session_file(&dir.join(CloseIntent::TAB_CLOSED_FILE), b"{\"tab_closed\":true}\n").expect("an earlier tab's mark");
    crate::agent_session_files::ensure_session_dir(&dir).expect("claim the folder");
    assert!(!dir.join(CloseIntent::TAB_CLOSED_FILE).exists(), "the earlier tab's mark does not apply to the new session");
    let owner = crate::agent_session_files::read_owner_marker(&dir).expect("an owner marker");
    assert_eq!(owner.pid, std::process::id(), "the new session owns the folder");
    assert_eq!(owner.started_ms, crate::agent_session_files::current_process_started_ms());
    let _ = std::fs::remove_dir_all(&dir);
}

/// The same process keeps its own folder and its mark across a repeated call: publishing does not lift a mark that is there.
#[test]
fn a_folder_this_process_owns_keeps_its_mark_when_it_is_ensured_again() {
    let st = TerminalState::default();
    let dir = st.session_dir.clone();
    let _ = std::fs::remove_dir_all(&dir);
    crate::agent_session_files::ensure_session_dir(&dir).expect("create the folder");
    atomic_write_session_file(&dir.join(CloseIntent::TAB_CLOSED_FILE), b"{\"tab_closed\":true}\n").expect("the mark");
    crate::agent_session_files::ensure_session_dir(&dir).expect("ensure again");
    assert!(dir.join(CloseIntent::TAB_CLOSED_FILE).is_file());
    let _ = std::fs::remove_dir_all(&dir);
}

/// A library file's tab marks its folder closed before it gives up the folder's lock: once the lock is free another window may own
/// the folder, and a mark written then would refuse that window's agents.
#[cfg(unix)]
#[test]
fn a_library_tab_marks_its_folder_closed_before_it_releases_the_lock() {
    let lib = std::env::temp_dir().join(format!("designos-closed-mark-lock-{}-{}", std::process::id(), crate::live_sync_tests::unique_tab_id()));
    std::fs::create_dir_all(&lib).unwrap();
    let mut st = TerminalState::default();
    st.bind_library_file(&lib, "file-lock-order", &format!("p{}-wlock-{}", std::process::id(), crate::live_sync_tests::unique_tab_id()));
    assert!(st.lock_owner.is_some(), "this window holds the folder's lock");
    let dir = st.session_dir.clone();
    assert_eq!(dir, crate::agent_sessions::stable_session_dir(&lib, "file-lock-order"), "the tab works in the file's own folder");
    let lock = dir.join("session.lock");
    assert!(lock.is_file(), "precondition: the lock is on disk");
    let seen = std::sync::Arc::new(std::sync::Mutex::new(None));
    let (seen_at, dir_at) = (seen.clone(), dir.clone());
    crate::agent_session_files::set_before_remove_hook(
        &lock,
        Box::new(move || *seen_at.lock().unwrap() = Some(dir_at.join(CloseIntent::TAB_CLOSED_FILE).is_file())),
    );
    st.shutdown();
    assert_eq!(*seen.lock().unwrap(), Some(true), "the lock was released before the folder was marked closed");
    assert!(!lock.exists(), "the lock is released");
    let _ = std::fs::remove_dir_all(&lib);
}
