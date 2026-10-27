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
    assert_eq!(explain("write", &link, &err), format!("{} is a link, not a file. Delete the link, then try again.", link.display()));
    let fifo = dir.fifo("context.json");
    let err = read(&fifo).unwrap_err();
    assert_eq!(explain("read", &fifo, &err), format!("{} is not a plain file. Delete it, then try again.", fifo.display()));
    let second_name = dir.path("doc-sync.designos");
    fs::hard_link(&victim, &second_name).unwrap();
    let err = append(&second_name, b"x", false).unwrap_err();
    assert_eq!(explain("write", &second_name, &err), format!("{} has a second name (a hard link). Delete it, then try again.", second_name.display()));
    let err = io::Error::from_raw_os_error(libc::EACCES);
    assert_eq!(explain("write", &link, &err), format!("{} can't be written: {err}. Check its permissions and free space, then try again.", link.display()));
    assert_eq!(explain("read", &link, &err), format!("{} can't be read: {err}. Check its permissions and free space, then try again.", link.display()));
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

/// What an agent is told when `result` fails for `path`.
fn told(verb: &str, path: &Path, result: io::Result<impl std::fmt::Debug>) -> String {
    explain(verb, path, &result.expect_err("the path must be refused"))
}

#[test]
fn a_hard_link_is_refused_by_every_call_but_a_plain_file_with_one_name_is_not() {
    let dir = Scratch::new("hard");
    let victim = dir.victim();
    let second_name = dir.path("doc-patch.jsonl");
    fs::hard_link(&victim, &second_name).unwrap();
    let sentence = format!("{} has a second name (a hard link). Delete it, then try again.", second_name.display());
    assert_eq!(told("write", &second_name, append(&second_name, b"line\n", false)), sentence);
    assert_eq!(told("write", &second_name, truncate(&second_name, false)), sentence);
    assert_eq!(told("read", &second_name, read(&second_name)), sentence, "a read through a second name is refused too");
    assert_eq!(told("read", &second_name, stat(&second_name)), sentence);
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
    let sentence = explain("read", &path, &err);
    assert!(sentence.starts_with(&format!("{} can't be read: ", path.display())), "{sentence}");
    assert!(sentence.ends_with("Check its permissions and free space, then try again."), "{sentence}");
}

#[test]
fn a_folder_at_a_rename_target_and_a_socket_are_refused_as_not_plain_files() {
    let dir = Scratch::new("folder-socket");
    let target = dir.path("canvas.designos");
    fs::create_dir(&target).unwrap();
    let sentence = told("write", &target, write_atomic(&target, b"x", false));
    assert_eq!(sentence, format!("{} is not a plain file. Delete it, then try again.", target.display()));
    // A socket path is limited to about 100 bytes: bind under a short path, then move it in.
    let short = std::env::temp_dir().join(format!("ds{}.sock", std::process::id()));
    drop(std::os::unix::net::UnixListener::bind(&short).unwrap());
    let socket = dir.path("context.json");
    fs::rename(&short, &socket).unwrap();
    assert_eq!(refusal(read(&socket)), Refusal::NotPlainFile);
    assert_eq!(refusal(append(&socket, b"x", false)), Refusal::NotPlainFile);
}

#[test]
fn eloop_under_no_follow_is_a_link_even_when_the_entry_is_a_plain_file_again() {
    let dir = Scratch::new("eloop");
    let path = dir.path("doc-patch.jsonl");
    fs::write(&path, b"x").unwrap();
    let err = super::platform::open_failure(&path, io::Error::from_raw_os_error(libc::ELOOP));
    assert_eq!(explain("write", &path, &err), format!("{} is a link, not a file. Delete the link, then try again.", path.display()));
}

#[test]
fn a_file_is_handed_back_in_blocking_mode() {
    let dir = Scratch::new("blocking");
    let path = dir.path("doc-patch.jsonl");
    fs::write(&path, b"x").unwrap();
    let non_blocking = |file: &fs::File| {
        use std::os::fd::AsRawFd;
        // SAFETY: `file` owns a valid open descriptor for the whole call, and F_GETFL reads its status flags only.
        (unsafe { libc::fcntl(file.as_raw_fd(), libc::F_GETFL) }) & libc::O_NONBLOCK != 0
    };
    let (file, _) = super::platform::open_plain(&path, fs::OpenOptions::new().read(true)).unwrap();
    assert!(!non_blocking(&file), "a read stays in O_NONBLOCK mode");
    let (file, _) = super::platform::open_plain(&path, fs::OpenOptions::new().append(true)).unwrap();
    assert!(!non_blocking(&file), "an append stays in O_NONBLOCK mode");
}
