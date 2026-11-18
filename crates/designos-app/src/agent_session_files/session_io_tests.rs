//! The path calls of `session_io`, one by one: each refuses a link anywhere below the session folder, and what a refused call
//! leaves behind in the target of the link is nothing.

use super::folder_tests::{only_entry_is_the_sentinel, target_with_sentinel};
use super::no_follow::Dir;
use super::refusal::{refusal_of, Refusal};
use super::test_support::{made_session_folder, plant_link};
use super::*;
use std::os::unix::fs::{symlink, PermissionsExt};

fn assert_link_refused<T: std::fmt::Debug>(what: &str, result: std::io::Result<T>) {
    let e = result.expect_err(&format!("{what}: a link in the path must be refused"));
    assert_eq!(refusal_of(&e), Some(Refusal::Link), "{what}: {e}");
}

#[test]
fn every_call_refuses_a_link_below_the_session_folder_and_leaves_its_target_alone() {
    let (target, _target) = target_with_sentinel("calls-target");
    let (session, _session) = made_session_folder("calls");
    plant_link(&session, ".gemini", &target);
    let file = session.join(".gemini").join("settings.json");
    let in_target = target.join("settings.json");
    std::fs::write(&in_target, b"theirs").expect("a file in the target");

    assert_link_refused("create exclusive", create_session_file_exclusive(&session.join(".gemini").join("lock"), b"x"));
    assert_link_refused("read", read_session_file(&file));
    assert_link_refused("remove file", remove_session_file(&file));
    assert_link_refused("remove folder", remove_session_dir_all(&session.join(".gemini").join("jobs")));
    assert_link_refused("rename", rename_session_file(&file, &session.join(".gemini").join("old.json")));
    assert_link_refused("create folder", create_session_dir_all(&session.join(".gemini").join("jobs").join("job-1")));
    assert_link_refused("verified write", write_session_file_verified(&file, b"studio"));
    assert_eq!(std::fs::read(&in_target).expect("the file in the target"), b"theirs", "a call changed the target");
    std::fs::remove_file(&in_target).expect("remove it");
    assert!(only_entry_is_the_sentinel(&target), "a call made something in the target");
}

#[test]
fn removing_a_folder_removes_the_links_inside_it_and_never_what_they_point_at() {
    let (target, _target) = target_with_sentinel("remove-target");
    let (session, _session) = made_session_folder("remove");
    let jobs = session.join("image-jobs");
    std::fs::create_dir_all(jobs.join("job-1").join("deep")).expect("a tree");
    std::fs::write(jobs.join("job-1").join("deep").join("x.png"), b"x").expect("a file");
    symlink(&target, jobs.join("job-1").join("link")).expect("a link to a folder");
    symlink(target.join("keep.txt"), jobs.join("file-link")).expect("a link to a file");

    remove_session_dir_all(&jobs).expect("the folder goes");

    assert!(!jobs.exists(), "the folder is removed");
    assert!(only_entry_is_the_sentinel(&target), "the link's target lost or gained something");
    remove_session_dir_all(&jobs).expect("what is not there is already removed");
    symlink(&target, session.join("a-link")).expect("a link at the name");
    remove_session_dir_all(&session.join("a-link")).expect("a link is removed itself");
    assert!(session.join("a-link").symlink_metadata().is_err() && target.join("keep.txt").is_file());
}

#[test]
fn a_file_is_written_whole_private_and_without_leaving_a_temporary_file() {
    let (session, _session) = made_session_folder("whole");
    let path = session.join(".agents").join("hooks.json");
    atomic_write_session_file(&path, b"first").expect("write");
    atomic_write_session_file(&path, b"second").expect("rewrite");
    assert_eq!(std::fs::read(&path).expect("read"), b"second");
    assert_eq!(std::fs::metadata(&path).expect("file").permissions().mode() & 0o777, 0o600);
    assert_eq!(std::fs::metadata(path.parent().expect("folder")).expect("folder").permissions().mode() & 0o777, 0o700);

    let held = write_session_file_unless(&path, b"third", || true).expect("hold");
    assert!(held && std::fs::read(&path).expect("read") == b"second", "a held write leaves the file as it was");
    let names: Vec<_> = std::fs::read_dir(path.parent().expect("folder")).expect("list").flatten().map(|e| e.file_name()).collect();
    assert_eq!(names, [std::ffi::OsString::from("hooks.json")], "no temporary file is left");
}

#[test]
fn a_folder_swapped_for_a_link_after_it_was_opened_still_receives_the_write() {
    let (target, _target) = target_with_sentinel("opened-target");
    let (session, _session) = made_session_folder("opened");
    let held = Dir::open_session_folder(&session).expect("the session folder").open_child(std::ffi::OsStr::new(".gemini"), true).expect("the subfolder");

    std::fs::rename(session.join(".gemini"), session.join(".gemini-moved")).expect("the agent moves the folder");
    symlink(&target, session.join(".gemini")).expect("and plants a link in its place");
    held.write_atomic(std::ffi::OsStr::new("settings.json"), b"studio", || false).expect("the write goes to the folder that was opened");

    assert_eq!(std::fs::read(session.join(".gemini-moved").join("settings.json")).expect("in the moved folder"), b"studio");
    assert!(only_entry_is_the_sentinel(&target), "the swapped-in link received a byte");
}

#[test]
fn a_session_file_name_that_is_a_folder_or_a_link_to_one_is_not_written_through() {
    let (target, _target) = target_with_sentinel("name-target");
    let (session, _session) = made_session_folder("names");
    symlink(&target, session.join("AGENTS.md")).expect("a link to a folder at the file name");
    atomic_write_session_file(&session.join("AGENTS.md"), b"studio").expect("the link is replaced");
    assert_eq!(std::fs::read(session.join("AGENTS.md")).expect("studio's file"), b"studio");
    std::fs::create_dir(session.join("GEMINI.md")).expect("a folder at the file name");
    assert!(atomic_write_session_file(&session.join("GEMINI.md"), b"studio").is_err(), "a folder is not replaced by a file");
    assert!(only_entry_is_the_sentinel(&target));
}

fn sorted_names(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir).expect("list").flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    names.sort();
    names
}

/// The path helpers place a file by the session folder above it, and in a real build nothing above a session folder is one: the folder
/// itself is no place for them. So a closed tab's folder is marked and emptied through a handle of the folder itself, which works the
/// same in a test build, where a path in no session folder would be let through.
#[test]
fn a_session_folder_is_marked_and_emptied_through_its_own_handle() {
    let (session, _session) = made_session_folder("mark-empty");
    assert!(super::session_io::place_of_with(&session, false).is_err(), "a real build has no place for the session folder itself");
    std::fs::write(session.join("doc-patch.jsonl"), b"{}\n").expect("a log");
    std::fs::create_dir_all(session.join("nested").join("deeper")).expect("a nested folder");
    std::fs::write(session.join("close-result.json"), b"{}").expect("an answer");
    let marked = mark_and_empty_session_folder(&session, "tab-closed.json", b"{\"tab_closed\":true}\n", &["owner.json", "close-result.json"]);
    assert!(marked.expect("the folder is marked and emptied"), "a real folder is marked");
    assert_eq!(sorted_names(&session), ["close-result.json", "owner.json", "tab-closed.json"]);
    assert_eq!(std::fs::read(session.join("tab-closed.json")).expect("the mark"), b"{\"tab_closed\":true}\n");
}

/// A folder below a session folder is not one, and a link at a session folder's name is left alone with what it points at.
#[test]
fn only_a_real_session_folder_is_marked_and_emptied() {
    let (session, _session) = made_session_folder("mark-below");
    let below = session.join("image-jobs");
    std::fs::create_dir(&below).expect("a folder below");
    let e = mark_and_empty_session_folder(&below, "tab-closed.json", b"{}", &[]).expect_err("not a session folder");
    assert_eq!(e.kind(), std::io::ErrorKind::InvalidInput, "{e}");
    assert!(below.is_dir() && !below.join("tab-closed.json").exists(), "nothing was written or removed");

    let (target, _target) = target_with_sentinel("mark-link-target");
    let link = ephemeral_session_dir(&format!("p{}-mark-link-{}", std::process::id(), crate::live_sync_tests::unique_tab_id()));
    symlink(&target, &link).expect("a link at a session folder's name");
    assert!(!mark_and_empty_session_folder(&link, "tab-closed.json", b"{}", &[]).expect("no folder to mark"), "a link is not marked");
    assert!(only_entry_is_the_sentinel(&target), "nothing went through the link");
    std::fs::remove_file(&link).expect("remove the link");
    let gone = ephemeral_session_dir(&format!("p{}-mark-gone-{}", std::process::id(), crate::live_sync_tests::unique_tab_id()));
    assert!(!mark_and_empty_session_folder(&gone, "tab-closed.json", b"{}", &[]).expect("nothing there"));
    assert!(!gone.exists(), "a folder that is not there is not made");
}
