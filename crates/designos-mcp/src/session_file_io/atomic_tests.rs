//! Files replaced as a whole, the temporary names, and the fallback for a platform without `O_NOFOLLOW`.
#![cfg(unix)]

use super::test_support::{refusal, victim_is_intact, Scratch};
use super::*;
use std::fs;
use std::os::unix::fs::{symlink, MetadataExt};

#[test]
fn an_atomic_write_replaces_a_link_and_never_writes_through_it() {
    let dir = Scratch::new("atomic-link");
    let victim = dir.victim();
    let target = dir.path("canvas.designos");
    symlink(&victim, &target).unwrap();
    write_atomic(&target, b"{\"name\":\"doc\"}", false).unwrap();
    victim_is_intact(&victim);
    assert!(fs::symlink_metadata(&target).unwrap().file_type().is_file());
    assert_eq!(fs::read(&target).unwrap(), b"{\"name\":\"doc\"}");
    assert_eq!(fs::metadata(&target).unwrap().mode() & 0o777, 0o600, "owner only");
}

#[test]
fn an_atomic_write_replaces_a_fifo_without_waiting_for_it() {
    let dir = Scratch::new("atomic-fifo");
    let target = dir.fifo("canvas.designos");
    write_atomic(&target, b"x", true).unwrap();
    assert!(fs::symlink_metadata(&target).unwrap().file_type().is_file());
}

#[test]
fn an_atomic_write_leaves_no_temporary_file_and_cleans_up_after_a_failed_rename() {
    let dir = Scratch::new("atomic-clean");
    let target = dir.path("canvas.designos");
    write_atomic(&target, b"one", false).unwrap();
    write_atomic(&target, b"two", true).unwrap();
    assert_eq!(fs::read(&target).unwrap(), b"two");
    let names = |dir: &Scratch| fs::read_dir(&dir.0).unwrap().map(|e| e.unwrap().file_name().into_string().unwrap()).collect::<Vec<_>>();
    assert_eq!(names(&dir), ["canvas.designos"]);
    let blocked = dir.path("doc-sync.designos");
    fs::create_dir(&blocked).unwrap();
    fs::write(blocked.join("inside"), b"x").unwrap();
    assert!(write_atomic(&blocked, b"x", false).is_err(), "a rename onto a non-empty folder fails");
    assert!(names(&dir).iter().all(|n| !n.ends_with(".tmp")), "{:?}", names(&dir));
}

#[test]
fn a_missing_folder_is_an_error_and_is_not_made() {
    let dir = Scratch::new("no-folder");
    let target = dir.path("gone").join("canvas.designos");
    assert_eq!(write_atomic(&target, b"x", false).unwrap_err().kind(), io::ErrorKind::NotFound);
    assert_eq!(append(&target, b"x", false).unwrap_err().kind(), io::ErrorKind::NotFound);
    assert!(!dir.path("gone").exists());
}

#[test]
fn a_temporary_name_is_hidden_and_carries_64_random_bits() {
    let mut seen = std::collections::BTreeSet::new();
    for _ in 0..2000 {
        let name = temp_name(OsStr::new("canvas.designos")).into_string().unwrap();
        let hex = name.strip_prefix(".canvas.designos.").and_then(|rest| rest.strip_suffix(".tmp")).expect("shape of a temporary name");
        assert_eq!(hex.len(), 16, "{name}");
        assert!(u64::from_str_radix(hex, 16).is_ok(), "{name}");
        assert!(seen.insert(name), "a temporary name came up twice");
    }
}

#[test]
fn the_path_based_fallback_refuses_a_link_that_is_already_there_and_writes_atomically() {
    use super::path_based as fallback;
    let dir = Scratch::new("fallback");
    let victim = dir.victim();
    let link = dir.path("doc-patch.jsonl");
    symlink(&victim, &link).unwrap();
    assert_eq!(refusal(fallback::read(&link)), Refusal::Link);
    assert_eq!(refusal(fallback::append(&link, b"x", false)), Refusal::Link);
    assert_eq!(refusal(fallback::truncate(&link, false)), Refusal::Link);
    victim_is_intact(&victim);
    let fifo = dir.fifo("context.json");
    assert_eq!(refusal(fallback::read(&fifo)), Refusal::NotPlainFile);
    let target = dir.path("canvas.designos");
    symlink(&victim, &target).unwrap();
    fallback::write_atomic(&target, b"doc", true).unwrap();
    victim_is_intact(&victim);
    fallback::append(&dir.path("log.jsonl"), b"a\n", false).unwrap();
    assert_eq!(fallback::read(&dir.path("log.jsonl")).unwrap(), b"a\n");
    fallback::truncate(&dir.path("log.jsonl"), false).unwrap();
    assert_eq!(fs::metadata(dir.path("log.jsonl")).unwrap().len(), 0);
    assert_eq!(fallback::truncate(&dir.path("absent"), false).ok(), Some(()));
}
