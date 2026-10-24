//! Reads, appends and truncates on every hazard an agent can plant at a session-file name.
#![cfg(unix)]

use super::test_support::{refusal, victim_is_intact, Scratch};
use super::*;
use std::fs;
use std::io::Read;
use std::os::unix::fs::{symlink, OpenOptionsExt};

#[test]
fn a_missing_file_is_not_found_and_nothing_is_made_by_reading() {
    let dir = Scratch::new("missing");
    let err = read(&dir.path("context.json")).unwrap_err();
    assert_eq!(err.kind(), io::ErrorKind::NotFound);
    assert_eq!(truncate(&dir.path("context.json"), false).ok(), Some(()), "an absent file is already empty");
    assert!(!dir.path("context.json").exists());
}

#[test]
fn a_link_is_refused_by_every_call_and_its_victim_is_left_alone() {
    let dir = Scratch::new("link");
    let victim = dir.victim();
    let link = dir.path("doc-patch.jsonl");
    symlink(&victim, &link).unwrap();
    assert_eq!(refusal(read(&link)), Refusal::Link);
    assert_eq!(refusal(append(&link, b"line\n", false)), Refusal::Link);
    assert_eq!(refusal(truncate(&link, false)), Refusal::Link);
    assert_eq!(refusal(stat(&link)), Refusal::Link);
    victim_is_intact(&victim);
}

#[test]
fn the_refusal_reads_as_the_sentence_an_agent_is_told() {
    let dir = Scratch::new("sentence");
    let victim = dir.victim();
    let link = dir.path("doc-patch.jsonl");
    symlink(&victim, &link).unwrap();
    let err = append(&link, b"x", false).unwrap_err();
    assert_eq!(explain("write", &link, &err), format!("{} is a link, not a file. Delete the link and try again.", link.display()));
    let fifo = dir.fifo("context.json");
    let err = read(&fifo).unwrap_err();
    assert_eq!(explain("read", &fifo, &err), format!("{} is not a plain file. Delete it and try again.", fifo.display()));
    let err = io::Error::from_raw_os_error(libc::EACCES);
    assert!(explain("write", &link, &err).starts_with(&format!("Cannot write {}: ", link.display())));
}

#[test]
fn a_dangling_link_is_refused_and_its_missing_target_is_not_created() {
    let dir = Scratch::new("dangling");
    let missing = dir.path("not-yet-there.txt");
    let link = dir.path("doc-patch.jsonl");
    symlink(&missing, &link).unwrap();
    assert_eq!(refusal(append(&link, b"line\n", false)), Refusal::Link);
    assert_eq!(refusal(truncate(&link, false)), Refusal::Link);
    assert!(!missing.exists());
}

#[test]
fn a_hard_link_is_refused_for_a_write_but_a_plain_file_with_one_name_is_not() {
    let dir = Scratch::new("hard");
    let victim = dir.victim();
    let second_name = dir.path("doc-patch.jsonl");
    fs::hard_link(&victim, &second_name).unwrap();
    assert_eq!(refusal(append(&second_name, b"line\n", false)), Refusal::Link);
    assert_eq!(refusal(truncate(&second_name, false)), Refusal::Link);
    victim_is_intact(&victim);
    let own = dir.path("own.jsonl");
    append(&own, b"a\n", false).unwrap();
    append(&own, b"b\n", true).unwrap();
    assert_eq!(read_to_string(&own).unwrap(), "a\nb\n");
    truncate(&own, true).unwrap();
    assert_eq!(fs::metadata(&own).unwrap().len(), 0);
}

#[test]
fn a_fifo_is_refused_at_once_and_a_folder_too() {
    let dir = Scratch::new("fifo");
    let fifo = dir.fifo("doc-patch.jsonl");
    assert_eq!(refusal(append(&fifo, b"x", false)), Refusal::NotPlainFile, "a FIFO nobody reads");
    assert_eq!(refusal(truncate(&fifo, false)), Refusal::NotPlainFile);
    assert_eq!(refusal(read(&fifo)), Refusal::NotPlainFile, "a FIFO nobody writes");
    assert_eq!(refusal(stat(&fifo)), Refusal::NotPlainFile);
    fs::create_dir(dir.path("a-folder")).unwrap();
    assert_eq!(refusal(append(&dir.path("a-folder"), b"x", false)), Refusal::NotPlainFile);
    assert_eq!(refusal(read(&dir.path("a-folder"))), Refusal::NotPlainFile);
}

#[test]
fn a_fifo_somebody_reads_is_refused_after_the_open_and_receives_nothing() {
    let dir = Scratch::new("fifo-reader");
    let fifo = dir.fifo("idempotency-ledger.jsonl");
    let reader = fs::OpenOptions::new().read(true).custom_flags(libc::O_NONBLOCK).open(&fifo).unwrap();
    assert_eq!(refusal(append(&fifo, b"secret\n", true)), Refusal::NotPlainFile);
    assert_eq!(refusal(truncate(&fifo, true)), Refusal::NotPlainFile);
    match (&reader).read(&mut [0u8; 16]) {
        Ok(received) => assert_eq!(received, 0, "bytes reached the FIFO's reader"),
        Err(e) => assert_eq!(e.kind(), io::ErrorKind::WouldBlock, "bytes reached the FIFO's reader"),
    }
}

#[test]
fn an_unreadable_file_is_an_error_that_names_the_path_not_a_refusal() {
    let dir = Scratch::new("denied");
    let path = dir.path("context.json");
    fs::write(&path, b"{}").unwrap();
    fs::set_permissions(&path, std::os::unix::fs::PermissionsExt::from_mode(0o000)).unwrap();
    if fs::File::open(&path).is_ok() {
        return; // running as a user the mode does not bind (root): nothing to test
    }
    let err = read(&path).unwrap_err();
    assert_eq!(refusal_of(&err), None);
    assert!(explain("read", &path, &err).contains(&path.display().to_string()));
}
