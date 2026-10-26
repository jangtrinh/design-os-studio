//! Clearing what a killed app-server leaves behind: the rendezvous symlink, Codex's socket and its lock.

use super::support::{short_scratch_dir, wait_until, Scratch};
use crate::codex_remote::stale_socket::clear_stale_rendezvous_in;
use crate::codex_remote::FallbackReason;
use std::os::unix::fs::symlink;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::time::Duration;

const HASH: &str = "a61a5688baf810f9c9456a16648bf5e31a3a357419289b6f9c5672f62f115efe";

/// A Codex daemon folder (canonical, as macOS resolves `/tmp`) and a channel folder beside it, with the guard that owns both.
fn folders() -> (Scratch, PathBuf, PathBuf) {
    let base = short_scratch_dir("st");
    let (daemon, channel) = (base.join("d"), base.join("c"));
    std::fs::create_dir_all(&daemon).unwrap();
    std::fs::create_dir_all(&channel).unwrap();
    (base, daemon, channel)
}

/// What SIGKILL leaves: a socket file nobody listens on, its lock, and the rendezvous symlink to it.
fn leftovers(daemon: &Path, channel: &Path) -> (PathBuf, PathBuf, PathBuf) {
    let (socket, lock, rendezvous) = (daemon.join(HASH), daemon.join(format!("{HASH}.lock")), channel.join("cx-1.sock"));
    drop(UnixListener::bind(&socket).unwrap());
    // A process spawned by another test in the instant the listener was created can still hold a copy of its
    // descriptor, which makes the dead socket answer. Wait until nothing does.
    assert!(wait_until(Duration::from_secs(10), || UnixStream::connect(&socket).is_err()), "the leftover socket must be dead");
    std::fs::write(&lock, "").unwrap();
    symlink(&socket, &rendezvous).unwrap();
    (socket, lock, rendezvous)
}

fn gone(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_err()
}

#[test]
fn nothing_there_is_fine_and_a_plain_leftover_is_removed() {
    let (_folder, daemon, channel) = folders();
    assert_eq!(clear_stale_rendezvous_in(&channel.join("cx-1.sock"), Some(&daemon)), Ok(()));
    let plain = channel.join("cx-2.sock");
    std::fs::write(&plain, "stale").unwrap();
    assert_eq!(clear_stale_rendezvous_in(&plain, Some(&daemon)), Ok(()));
    assert!(gone(&plain));
}

#[test]
fn the_symlink_socket_and_lock_of_a_killed_server_are_all_removed() {
    let (_folder, daemon, channel) = folders();
    let (socket, lock, rendezvous) = leftovers(&daemon, &channel);
    assert_eq!(clear_stale_rendezvous_in(&rendezvous, Some(&daemon)), Ok(()));
    assert!(gone(&socket) && gone(&lock) && gone(&rendezvous));
}

#[test]
fn a_dangling_link_into_the_daemon_folder_is_cleared_with_its_lock() {
    let (_folder, daemon, channel) = folders();
    let (socket, lock, rendezvous) = leftovers(&daemon, &channel);
    std::fs::remove_file(&socket).unwrap();
    assert_eq!(clear_stale_rendezvous_in(&rendezvous, Some(&daemon)), Ok(()));
    assert!(gone(&lock) && gone(&rendezvous));
}

#[test]
fn a_server_that_still_answers_is_left_alone() {
    let (_folder, daemon, channel) = folders();
    let rendezvous = channel.join("cx-1.sock");
    let socket = daemon.join(HASH);
    let _live = UnixListener::bind(&socket).unwrap();
    symlink(&socket, &rendezvous).unwrap();
    assert_eq!(clear_stale_rendezvous_in(&rendezvous, Some(&daemon)), Err(FallbackReason::SocketInUse));
    assert!(!gone(&socket) && !gone(&rendezvous), "nothing is removed from under a live server");
    let live_plain = channel.join("cx-3.sock");
    let _plain = UnixListener::bind(&live_plain).unwrap();
    assert_eq!(clear_stale_rendezvous_in(&live_plain, Some(&daemon)), Err(FallbackReason::SocketInUse), "a live socket in place of the link counts too");
}

#[test]
fn a_link_that_does_not_lead_to_a_codex_socket_only_loses_the_link() {
    let (_folder, daemon, channel) = folders();
    let elsewhere = channel.join("precious.txt");
    std::fs::write(&elsewhere, "keep").unwrap();
    let wrong_name = daemon.join("not-a-hash");
    std::fs::write(&wrong_name, "keep").unwrap();
    let other_folder = channel.join(HASH);
    std::fs::write(&other_folder, "keep").unwrap();
    for (n, target) in [&elsewhere, &wrong_name, &other_folder].into_iter().enumerate() {
        let rendezvous = channel.join(format!("cx-1{n}.sock"));
        symlink(target, &rendezvous).unwrap();
        assert_eq!(clear_stale_rendezvous_in(&rendezvous, Some(&daemon)), Ok(()));
        assert!(gone(&rendezvous) && target.exists(), "{} must survive", target.display());
    }
    let rendezvous = channel.join("cx-9.sock");
    symlink(daemon.join(HASH), &rendezvous).unwrap();
    std::fs::write(daemon.join(HASH), "a regular file, not a socket").unwrap();
    assert_eq!(clear_stale_rendezvous_in(&rendezvous, None), Ok(()), "without a known daemon folder nothing but the link is removed");
    assert!(gone(&rendezvous) && daemon.join(HASH).exists());
}
