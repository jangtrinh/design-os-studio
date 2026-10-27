//! The open that the tailers use: a plain file of a session folder, relative to the folder's handle, never a link, never a FIFO, and
//! blocking again once it is open.

use super::folder_tests::{scratch_dir, target_with_sentinel, Cleanup};
use super::test_support::{made_session_folder, make_fifo, timed_with_fifo_watchdog};
use super::*;
use std::io::{Read, Seek, SeekFrom};
use std::os::fd::AsRawFd;
use std::os::unix::fs::symlink;

fn is_blocking(file: &std::fs::File) -> bool {
    // SAFETY: the descriptor belongs to `file`, which outlives the call.
    (unsafe { libc::fcntl(file.as_raw_fd(), libc::F_GETFL) }) & libc::O_NONBLOCK == 0
}

#[test]
fn a_plain_file_opens_for_reading_by_offset_and_is_blocking_again() {
    let (session, _session) = made_session_folder("plain-open");
    let log = session.join("doc-patch.jsonl");
    std::fs::write(&log, b"0123456789").expect("a log");

    let mut file = open_session_file(&log).expect("a plain file of the session folder opens");

    assert!(is_blocking(&file), "O_NONBLOCK was set for the open only");
    file.seek(SeekFrom::Start(4)).expect("seek");
    let mut rest = String::new();
    file.read_to_string(&mut rest).expect("read");
    assert_eq!(rest, "456789");
}

#[test]
fn a_link_a_fifo_and_a_folder_are_refused_with_the_reason() {
    let (session, _session) = made_session_folder("plain-refused");
    let (target, _target) = target_with_sentinel("plain-refused-target");
    symlink(target.join("keep.txt"), session.join("link.json")).expect("a link to a file");
    make_fifo(&session.join("fifo.json"));
    std::fs::create_dir(session.join("folder.json")).expect("a folder");
    symlink(&target, session.join("linked")).expect("a link to a folder");

    let reasons = |name: &str| open_session_file(&session.join(name)).expect_err(name);

    let line_of = |name: &str| with_next_step(&reasons(name));
    let path = |name: &str| session.join(name).display().to_string();
    assert_eq!(line_of("link.json"), format!("{} is a link, not a file. Delete the link, then try again", path("link.json")));
    for name in ["fifo.json", "folder.json"] {
        assert_eq!(line_of(name), format!("{} is not a plain file. Delete it, then try again", path(name)));
    }
    let through_a_linked_folder = open_session_file(&session.join("linked").join("keep.txt")).expect_err("a link in the path");
    assert_eq!(super::refusal::refusal_of(&through_a_linked_folder), Some(super::refusal::Refusal::Link), "{through_a_linked_folder}");
}

#[test]
fn outside_a_session_folder_a_link_is_followed_and_a_fifo_is_refused_at_once() {
    let scratch = scratch_dir("plain-outside");
    let _scratch = Cleanup(vec![scratch.clone()]);
    std::fs::write(scratch.join("transcript.jsonl"), b"{}\n").expect("a transcript");
    symlink(scratch.join("transcript.jsonl"), scratch.join("named.jsonl")).expect("a link to it");
    let mut text = String::new();
    open_plain_file(&scratch.join("named.jsonl")).expect("a link to a plain file opens").read_to_string(&mut text).expect("read");
    assert_eq!(text, "{}\n");

    let fifo = scratch.join("fifo.jsonl");
    make_fifo(&fifo);
    let (result, took) = timed_with_fifo_watchdog(&fifo, Duration::from_secs(30), || open_plain_file(&fifo));
    assert!(took < Duration::from_secs(30), "blocked on the FIFO for {took:?}");
    assert!(result.expect_err("a FIFO is no transcript").to_string().contains("not a plain file"));
}
