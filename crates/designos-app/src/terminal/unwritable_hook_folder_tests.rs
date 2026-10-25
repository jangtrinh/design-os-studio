//! A hook whose folder it cannot write must still end at once, quietly and with success. Two things put a hook there: the user turns
//! on the CLI's own sandbox (Gemini, Grok), whose write grants never reach Studio's private hook folder, and a folder that is gone.
//! The tab then shows no status, but the CLI must not wait on its hooks or print a failure for every event. These tests run the real
//! command strings of every CLI's installed hooks, Claude's prompt hook included, through `sh`, against a missing folder and a folder
//! that refuses writes.

use super::hook_output_tests::{run_hook_in, run_hook_with, shells_here, window_with_hooks, Installed, EVERY_CLI};
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

/// What one run of a hook through `shell` looked like, when it did not end as it must.
fn complaint(shell: &str, command: &str, session: &Path) -> Option<String> {
    let started = Instant::now();
    let output = run_hook_in(shell, command, session, "run-1", |_| {});
    let took = started.elapsed();
    let stderr = String::from_utf8_lossy(&output.stderr);
    let ends_well = output.status.code() == Some(0) && took < QUICK && stderr.is_empty();
    (!ends_well).then(|| {
        format!("{shell}: exit {:?} after {took:?}, stderr {:?}: {}", output.status.code(), stderr.trim(), command.chars().take(110).collect::<String>())
    })
}

/// Runs every command through every shell here, at most eight at a time, so a hook that waits does not make a long test of it, and
/// returns what was wrong.
fn complaints(commands: &[String], session: &Path) -> Vec<String> {
    let mut found = Vec::new();
    let runs: Vec<(&str, &String)> = shells_here().into_iter().flat_map(|shell| commands.iter().map(move |command| (shell, command))).collect();
    for chunk in runs.chunks(8) {
        std::thread::scope(|scope| {
            let handles: Vec<_> = chunk.iter().map(|(shell, command)| scope.spawn(|| complaint(shell, command, session))).collect();
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
            if matches!(which, Installed::ClaudeAsk | Installed::ClaudeSkip) {
                // The prompt hook of Claude marks a first turn whose bootstrap block it printed, and prints the block: with the reply
                // there, it has a marker to make over a folder that refuses it. (Without a reply it waits up to half a second for one.)
                let reply = r#"{"hookSpecificOutput":{"hookEventName":"UserPromptSubmit","additionalContext":"BOOTSTRAP-BLOCK"}}"#;
                std::fs::write(st.prepared_hook_paths().bootstrap_reply(), reply).expect("the bootstrap reply");
            }
            match how {
                Unwritable::Missing => std::fs::remove_dir_all(&folder).expect("the hook folder goes"),
                Unwritable::ReadOnly => std::fs::set_permissions(&folder, std::fs::Permissions::from_mode(0o500)).expect("the hook folder refuses writes"),
            }
            let mut commands = commands;
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
fn the_transcript_capture_does_not_wait_for_a_lock_that_is_not_there() {
    // The capture takes its lock for at most 50 tries 10 ms apart, so one that waits over a missing folder takes half a second at the
    // least, which the 1 s bound of the test above cannot tell from a busy machine. A capture that leaves at once takes a few
    // milliseconds, so one of three runs is under the floor of the wait on any machine that is not stopped.
    let (st, commands) = window_with_hooks(Installed::ClaudeSkip, None);
    let capture: Vec<&String> = commands.iter().filter(|command| command.contains("transcript_path")).collect();
    assert!(!capture.is_empty(), "Claude's hooks include the transcript capture");
    let folder = st.prepared_hook_paths().dir().to_path_buf();
    std::fs::remove_dir_all(&folder).expect("the hook folder goes");
    for command in capture {
        let quickest = (0..3)
            .map(|_| {
                let started = Instant::now();
                run_hook_with(command, &st.session_dir, "run-1", |_| {});
                started.elapsed()
            })
            .min()
            .expect("three runs");
        assert!(quickest < Duration::from_millis(450), "the capture waits for a lock that is not there: its quickest of three runs took {quickest:?}");
    }
}

#[test]
fn claudes_prompt_hook_does_not_wait_for_a_reply_in_a_folder_that_is_gone() {
    // With the folder there, the first turn waits up to half a second for the bootstrap reply, which Studio builds in the background.
    // With the folder gone no reply can come, so it must not: the quickest of three runs is under that floor on any machine that is not
    // stopped (the 1 s bound of the sweep cannot tell half a second from a busy machine).
    let (st, commands) = window_with_hooks(Installed::ClaudeAsk, None);
    let prompt: Vec<&String> = commands.iter().filter(|command| command.contains("bootstrap.delivered.")).collect();
    assert_eq!(prompt.len(), 1, "Claude has one prompt hook with a marker");
    std::fs::remove_dir_all(st.prepared_hook_paths().dir()).expect("the hook folder goes");
    let quickest = (0..3)
        .map(|_| {
            let started = Instant::now();
            run_hook_with(prompt[0], &st.session_dir, "run-1", |_| {});
            started.elapsed()
        })
        .min()
        .expect("three runs");
    assert!(quickest < Duration::from_millis(400), "the prompt hook waits for a reply in a folder that is gone: its quickest of three runs took {quickest:?}");
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
