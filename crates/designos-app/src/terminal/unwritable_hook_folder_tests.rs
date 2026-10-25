//! A hook whose folder it cannot write must still end at once, quietly and with success. Two things put a hook there: the user turns
//! on the CLI's own sandbox (Gemini, Grok), whose write grants never reach Studio's private hook folder, and a folder that is gone.
//! The tab then shows no status, but the CLI must not wait on its hooks or print a failure for every event. These tests run the real
//! command strings of every CLI's installed hooks, through `sh`, against a missing folder and a folder that refuses writes.

use super::hook_output_tests::{run_hook_with, window_with_hooks, EVERY_CLI};
use super::*;
use crate::agent_session_files::test_support::{running_as_root, tab_state_in};
use std::os::unix::fs::PermissionsExt;
use std::time::{Duration, Instant};

/// How long a hook that cannot write may take. A hook that waits out its lock loop takes 5 s and more, which is as long as Gemini lets a
/// hook run (`HOOK_TIMEOUT_MS`) before it kills it.
const QUICK: Duration = Duration::from_secs(1);

#[derive(Clone, Copy, Debug)]
enum Unwritable {
    /// The folder is not there (the channel went, or the user deleted it).
    Missing,
    /// The folder is there and refuses every write, like one the CLI's own sandbox does not grant.
    ReadOnly,
}

/// Makes the folder writable again when it drops, so a test that made it read-only can remove it.
struct Restore(PathBuf);

impl Drop for Restore {
    fn drop(&mut self) {
        let _ = std::fs::set_permissions(&self.0, std::fs::Permissions::from_mode(0o700));
    }
}

/// Whether `command` is the prompt hook of Claude: it reads the reply files Studio wrote, which is its purpose, and Claude's hooks are
/// never confined by a sandbox of the user's.
fn is_claudes_prompt_hook(command: &str) -> bool {
    command.contains("bootstrap.delivered.")
}

/// What one run of a hook looked like, when it did not end as it must.
fn complaint(command: &str, session: &Path) -> Option<String> {
    let started = Instant::now();
    let output = run_hook_with(command, session, "run-1", |_| {});
    let took = started.elapsed();
    let stderr = String::from_utf8_lossy(&output.stderr);
    let ends_well = output.status.code() == Some(0) && took < QUICK && stderr.is_empty();
    (!ends_well)
        .then(|| format!("exit {:?} after {took:?}, stderr {:?}: {}", output.status.code(), stderr.trim(), command.chars().take(110).collect::<String>()))
}

/// Runs every command at most eight at a time, so a hook that waits does not make a long test of it, and returns what was wrong.
fn complaints(commands: &[String], session: &Path) -> Vec<String> {
    let mut found = Vec::new();
    for chunk in commands.chunks(8) {
        std::thread::scope(|scope| {
            let handles: Vec<_> = chunk.iter().map(|command| scope.spawn(|| complaint(command, session))).collect();
            found.extend(handles.into_iter().filter_map(|handle| handle.join().expect("a hook run")));
        });
    }
    found
}

#[test]
fn a_hook_that_cannot_write_its_folder_ends_at_once_with_success_and_says_nothing() {
    for how in [Unwritable::Missing, Unwritable::ReadOnly] {
        if matches!(how, Unwritable::ReadOnly) && running_as_root() {
            continue;
        }
        for which in EVERY_CLI {
            let (st, commands) = window_with_hooks(which, None);
            let folder = st.prepared_hook_paths().dir().to_path_buf();
            let _restore = Restore(folder.clone());
            match how {
                Unwritable::Missing => std::fs::remove_dir_all(&folder).expect("the hook folder goes"),
                Unwritable::ReadOnly => std::fs::set_permissions(&folder, std::fs::Permissions::from_mode(0o500)).expect("the hook folder refuses writes"),
            }
            let mut commands: Vec<String> = commands.into_iter().filter(|command| !is_claudes_prompt_hook(command)).collect();
            commands.sort();
            commands.dedup();
            assert!(commands.len() >= 4, "{which:?}: only {} commands to run", commands.len());

            let found = complaints(&commands, &st.session_dir);
            assert!(
                found.is_empty(),
                "{which:?}, {how:?}: {} of {} hooks did not end at once with success:\n{}",
                found.len(),
                commands.len(),
                found.join("\n")
            );
        }
    }
}

#[test]
fn a_lock_another_hook_holds_for_a_moment_is_still_waited_for_and_the_record_is_then_appended() {
    let st = tab_state_in(PermissionMode::Skip);
    let paths = st.prepared_hook_paths();
    let lock = paths.dir().join("agent-events.jsonl.lock");
    std::fs::create_dir(&lock).expect("another hook holds the lock");
    let held = lock.clone();
    let holder = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(300));
        std::fs::remove_dir(&held).expect("the other hook lets go");
    });

    let started = Instant::now();
    let output = run_hook_with(&super::status_hook_command(&paths, "PreToolUse", "working"), &st.session_dir, "run-1", |_| {});
    let took = started.elapsed();
    holder.join().expect("the other hook");

    assert!(output.status.success());
    assert!(took >= Duration::from_millis(250), "the hook went on at once, over a lock another hook held: {took:?}");
    let events = std::fs::read_to_string(paths.events()).expect("the record was appended");
    assert_eq!(events.lines().count(), 1, "{events}");
    assert!(events.contains("\"e\":\"PreToolUse\""), "{events}");
    assert!(!lock.exists(), "the hook took the lock and gave it back");
    let status = std::fs::read_to_string(paths.status()).expect("the status was written");
    assert!(status.contains("\"state\":\"working\""), "{status}");
}
