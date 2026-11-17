//! A scratch session folder outlives its tab as a folder holding only the mark of the closed tab, so an agent whose MCP server
//! outlives the tab is told "already closed" and can tell that from a folder that is simply gone.

use super::*;
use designos_doc::live_patch::CloseIntent;

fn scratch_state_with_a_folder() -> TerminalState {
    let st = TerminalState::default();
    assert!(crate::agent_session_files::is_ephemeral_session_dir(&st.session_dir), "a scratch folder: {}", st.session_dir.display());
    let _ = std::fs::remove_dir_all(&st.session_dir);
    crate::agent_session_files::ensure_session_dir(&st.session_dir).expect("create the folder");
    atomic_write_session_file(&st.session_dir.join("doc-patch.jsonl"), b"").expect("the agent's log");
    st
}

fn names_in(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> =
        std::fs::read_dir(dir).map(|rd| rd.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect()).unwrap_or_default();
    names.sort();
    names
}

#[test]
fn a_scratch_tab_that_shuts_down_leaves_only_the_mark_of_its_closed_tab() {
    let mut st = scratch_state_with_a_folder();
    let dir = st.session_dir.clone();
    st.shutdown();
    assert!(dir.join(CloseIntent::TAB_CLOSED_FILE).is_file(), "the closed tab left its mark");
    assert!(!dir.join("doc-patch.jsonl").exists(), "the rest of the scratch folder is gone: {:?}", names_in(&dir));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_scratch_tab_that_shuts_down_keeps_the_answer_to_a_close_and_the_mark() {
    let mut st = scratch_state_with_a_folder();
    let dir = st.session_dir.clone();
    atomic_write_session_file(&dir.join(CloseIntent::RESULT_FILE), b"{\"closed\":true}").expect("the answer");
    st.shutdown();
    assert!(dir.join(CloseIntent::RESULT_FILE).is_file(), "the answer to the close stays");
    assert!(dir.join(CloseIntent::TAB_CLOSED_FILE).is_file(), "and so does the mark");
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
    st.cleanup_ephemeral_session_dir();
    assert!(!dir.join(CloseIntent::TAB_CLOSED_FILE).exists(), "the project tab is still open: {:?}", names_in(&dir));
    let _ = std::fs::remove_dir_all(&dir);
}
