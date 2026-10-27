//! The pieces of [`super::folder`]: where the ephemeral root is, which paths count as ephemeral, and the checks that
//! refuse a link, a file or a foreign folder.

use super::folder::{create_session_folder_in, probe_dir_for, UNAVAILABLE_ROOT};
use super::folder_owner::{check_owner, own_uid, probe_uid};
use super::folder_tests::{scratch_dir, Cleanup};
use super::*;
use std::os::unix::fs::{symlink, MetadataExt, PermissionsExt};

#[test]
fn the_production_root_sits_next_to_the_approval_channel_in_the_data_folder() {
    let private = Path::new("/data/designos/ap");
    assert_eq!(sessions_root_beside(private), Some(PathBuf::from("/data/designos/sessions")));

    match default_sessions_root() {
        Some(root) => {
            assert_eq!(root.file_name().and_then(|n| n.to_str()), Some("sessions"));
            assert!(!root.starts_with(std::env::temp_dir()), "an agent can write the temp folder: {}", root.display());
            let data = designos_library::app_data_dir().expect("a data folder exists when the root does");
            assert!(root.starts_with(&data), "{} is not under the data folder {}", root.display(), data.display());
        }
        None => assert!(crate::agent_approval_channel::default_private_root().is_none(), "no sessions root without a private root"),
    }
}

#[test]
fn only_a_direct_child_of_the_root_is_ephemeral_and_an_id_cannot_leave_the_root() {
    let root = ephemeral_sessions_root().expect("the test root");
    let dir = ephemeral_session_dir("p1-tab-1");
    assert_eq!(dir, root.join("p1-tab-1"));
    assert!(is_ephemeral_session_dir(&dir));

    let old_place = std::env::temp_dir().join("designos-sessions").join("p1-tab-1");
    assert!(!is_ephemeral_session_dir(&old_place), "a folder in the temp folder is never removed as an ephemeral session");
    assert!(!is_ephemeral_session_dir(&root.join("agent-sessions").join("p1-tab-1")));

    for id in ["../x", "..", ".", "", "a/b", r"a\b"] {
        assert_eq!(ephemeral_session_dir(id).parent(), Some(root.as_path()), "id {id:?} left the root");
    }
}

#[test]
fn a_folder_of_another_user_is_refused() {
    let scratch = scratch_dir("owner");
    let _cleanup = Cleanup(vec![scratch.clone()]);
    let meta = std::fs::symlink_metadata(&scratch).expect("metadata");
    assert_eq!(own_uid(|| Ok(scratch.clone())).expect("own id"), meta.uid(), "a folder Studio made belongs to the user Studio runs as");

    check_owner(&scratch, meta.uid(), meta.uid()).expect("own folder");
    let err = check_owner(&scratch, 502, 501).expect_err("a foreign owner is refused");
    assert!(err.to_string().contains("belongs to another user"), "{err}");
    // macOS shows id 99 on a drive that ignores ownership and means the current user by it; elsewhere 99 is just another user.
    assert_eq!(check_owner(&scratch, 99, 501).is_ok(), cfg!(target_os = "macos"));

    // End to end: a real folder somebody else owns is not taken as a session folder (not applicable when run as that owner).
    let foreign = Path::new("/usr");
    if foreign.is_dir() && std::fs::metadata(foreign).expect("metadata").uid() != meta.uid() {
        let err = ensure_session_dir(foreign).expect_err("a folder of another user is refused");
        assert!(err.to_string().contains("belongs to another user"), "{err}");
    }
}

#[test]
fn a_folder_that_cannot_be_made_says_where_and_what_to_do() {
    let scratch = scratch_dir("read-only-parent");
    let _cleanup = Cleanup(vec![scratch.clone()]);
    if own_uid(|| Ok(scratch.clone())).expect("own id") == 0 {
        return; // root can write a read-only folder
    }
    let parent = scratch.join("agent-sessions");
    std::fs::create_dir_all(&parent).expect("parent");
    std::fs::set_permissions(&parent, std::fs::Permissions::from_mode(0o500)).expect("make it read-only");

    let err = ensure_session_dir(&parent.join("file-1")).expect_err("no folder in a read-only parent");
    std::fs::set_permissions(&parent, std::fs::Permissions::from_mode(0o700)).expect("restore");
    let text = with_next_step(&err);
    assert!(text.contains(&parent.join("file-1").display().to_string()) && text.contains("can't be created"), "names the folder: {text}");
    assert!(text.ends_with("try again"), "names the next step: {text}");
}

#[test]
fn the_user_id_is_not_read_through_a_link_at_the_sessions_folder() {
    let target = scratch_dir("probe-target");
    let scratch = scratch_dir("probe-link");
    let _cleanup = Cleanup(vec![target.clone(), scratch.clone()]);
    let linked_root = scratch.join("sessions");
    symlink(&target, &linked_root).expect("plant the link");

    let err = probe_dir_for(Some(&linked_root), &scratch).expect_err("a link as the sessions folder is not used");
    assert!(err.to_string().contains("is a link"), "{err}");
    assert_eq!(std::fs::read_dir(&target).expect("target").count(), 0, "something was made through the link");

    let real_root = scratch.join("real").join("sessions");
    assert_eq!(probe_dir_for(Some(&real_root), &scratch).expect("a missing sessions folder is made"), real_root);
    assert!(real_root.is_dir());
    assert_eq!(probe_dir_for(None, &scratch).expect("without a sessions folder the one next to it is used"), scratch);
}

#[test]
fn each_test_process_has_a_sessions_folder_of_its_own() {
    let root = ephemeral_sessions_root().expect("the test root");
    assert_eq!(root.file_name().and_then(|n| n.to_str()), Some("sessions"));
    let owner = root.parent().and_then(|p| p.file_name()).and_then(|n| n.to_str());
    assert_eq!(owner, Some(std::process::id().to_string().as_str()), "{}", root.display());
}

#[test]
fn a_probe_that_cannot_write_names_the_folder_and_what_to_do() {
    let scratch = scratch_dir("probe-read-only");
    let _cleanup = Cleanup(vec![scratch.clone()]);
    let read_only = scratch.join("base");
    std::fs::create_dir_all(&read_only).expect("folder");
    std::fs::set_permissions(&read_only, std::fs::Permissions::from_mode(0o555)).expect("make it read-only");
    if std::fs::File::create(read_only.join("writable-for-root")).is_ok() {
        return; // root can write a read-only folder
    }

    let err = probe_uid(&read_only).expect_err("no file can be created in a read-only folder");

    std::fs::set_permissions(&read_only, std::fs::Permissions::from_mode(0o700)).expect("restore");
    let text = with_next_step(&err);
    assert!(text.contains(&read_only.display().to_string()), "names the folder: {text}");
    assert!(text.ends_with("try again"), "names the next step: {text}");
}

#[test]
fn without_a_private_data_folder_no_session_folder_is_made() {
    let dir = ephemeral_session_dir("p1-tab-1");
    let unavailable = Path::new(UNAVAILABLE_ROOT).join("p1-tab-1");
    let err = create_session_folder_in(None, &unavailable).expect_err("no data folder, no session folder");
    assert!(err.to_string().contains("no private data folder"), "{err}");
    assert!(!unavailable.exists(), "a folder was made where the data folder is missing");
    assert!(atomic_write_session_file(&unavailable.join("context.json"), b"{}").is_err(), "a session file was written without a data folder");
    assert!(dir.starts_with(ephemeral_sessions_root().expect("the test root")));
}

#[test]
fn a_link_in_place_of_the_sessions_folder_is_refused_and_a_loose_one_is_made_private() {
    let target = scratch_dir("root-target");
    let scratch = scratch_dir("root-link");
    let _cleanup = Cleanup(vec![target.clone(), scratch.clone()]);
    let linked_root = scratch.join("sessions");
    symlink(&target, &linked_root).expect("plant the link");
    let err = create_session_folder_in(Some(&linked_root), &linked_root.join("p1-tab-1")).expect_err("a link as the sessions folder");
    assert!(err.to_string().contains("is a link"), "{err}");
    assert_eq!(std::fs::read_dir(&target).expect("target").count(), 0, "a session folder was made through the link");

    let loose_root = scratch.join("loose").join("sessions");
    std::fs::create_dir_all(&loose_root).expect("loose root");
    std::fs::set_permissions(&loose_root, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    create_session_folder_in(Some(&loose_root), &loose_root.join("p1-tab-1")).expect("a real folder of the user is used");
    assert_eq!(std::fs::metadata(&loose_root).expect("metadata").permissions().mode() & 0o777, 0o700, "other users lose access");
}

#[test]
fn a_session_file_is_not_written_through_a_link_at_its_folder() {
    let target = scratch_dir("write-target");
    let scratch = scratch_dir("write-link");
    let mut cleanup = Cleanup(vec![target.clone(), scratch.clone()]);

    let library_dir = scratch.join("agent-sessions").join("file-1");
    std::fs::create_dir_all(library_dir.parent().expect("agent-sessions")).expect("agent-sessions");
    symlink(&target, &library_dir).expect("plant the library link");

    let ephemeral_dir = ephemeral_session_dir(&format!("p{}-write-{}", std::process::id(), crate::live_sync_tests::unique_tab_id()));
    ensure_session_dir(&ephemeral_dir.with_file_name("write-neighbour")).expect("the sessions folder is made");
    cleanup.0.push(ephemeral_dir.with_file_name("write-neighbour"));
    cleanup.0.push(ephemeral_dir.clone());
    symlink(&target, &ephemeral_dir).expect("plant the ephemeral link");

    for dir in [&library_dir, &ephemeral_dir] {
        assert!(atomic_write_session_file(&dir.join("context.json"), b"{}").is_err(), "wrote through the link at {}", dir.display());
        assert!(atomic_write_session_file(&dir.join(".claude").join("hooks.json"), b"{}").is_err(), "wrote below the link at {}", dir.display());
    }
    assert_eq!(std::fs::read_dir(&target).expect("target").count(), 0, "a file went into the link's target");
}

#[test]
fn a_file_written_into_a_real_session_folder_may_create_its_subfolders() {
    let scratch = scratch_dir("write-real");
    let _cleanup = Cleanup(vec![scratch.clone()]);
    let dir = scratch.join("agent-sessions").join("file-1");
    atomic_write_session_file(&dir.join(".claude").join("skills").join("SKILL.md"), b"skill").expect("write");
    assert_eq!(std::fs::read(dir.join(".claude").join("skills").join("SKILL.md")).expect("read"), b"skill");
    assert_eq!(std::fs::metadata(&dir).expect("metadata").permissions().mode() & 0o777, 0o700, "the folder was made by Studio");
}

#[test]
fn the_sweep_never_removes_a_folder_in_the_temp_folder_where_earlier_versions_kept_sessions() {
    let scratch = scratch_dir("old-place");
    let _cleanup = Cleanup(vec![scratch.clone()]);
    let old_root = scratch.join("designos-sessions");
    let stale = old_root.join("p999993-tab-1");
    std::fs::create_dir_all(&stale).expect("stale folder");
    let eight_days_ago = SystemTime::now() - Duration::from_secs(8 * 24 * 60 * 60);
    std::fs::File::open(&stale).and_then(|f| f.set_modified(eight_days_ago)).expect("age the folder");

    assert_eq!(sweep_stale_sessions(&old_root, SystemTime::now(), |_, _| false), 0);
    assert!(stale.exists(), "a folder outside the sessions folder was deleted");
}
