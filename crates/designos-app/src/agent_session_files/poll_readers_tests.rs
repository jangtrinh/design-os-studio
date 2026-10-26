//! Studio polls some files of a session folder every frame, and tails others by offset, because the agent writes them by design.
//! The agent can also plant a FIFO at their names (a Codex workspace-write sandbox allows `mkfifo`), and a plain `open` of a FIFO
//! never returns: the UI thread freezes, and the unsaved edits with it. A link at the same names must not be read either. A refused
//! file is a poll with no data.

use super::folder_tests::target_with_sentinel;
use super::test_support::{made_session_folder, make_fifo, timed_with_fifo_watchdog};
use super::*;
use crate::live_sync_tests::unique_tab_id;
use crate::terminal::{TerminalAction, TerminalState};
use crate::test_timing::ScopedTerminalState;
use std::os::unix::fs::symlink;

/// The time a reader may take: a hang is found by the watchdog at this mark, so a reader that returns is far inside it.
const BUDGET: Duration = Duration::from_secs(30);

fn state_with_session_folder() -> ScopedTerminalState {
    let st = ScopedTerminalState::new(TerminalState::for_tab(unique_tab_id()));
    ensure_session_dir(&st.session_dir).expect("the session folder");
    st
}

fn cleared_run_ids(actions: &[TerminalAction]) -> Vec<String> {
    actions
        .iter()
        .filter_map(|action| match action {
            TerminalAction::ClearInProgress { run_id: Some(run_id), .. } => Some(run_id.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn a_fifo_at_agent_status_json_does_not_freeze_the_poll_of_every_frame() {
    let mut st = state_with_session_folder();
    let fifo = st.session_dir.join("agent-status.json");
    make_fifo(&fifo);

    let (actions, took) = timed_with_fifo_watchdog(&fifo, BUDGET, || st.poll_live_agent_bridge());

    assert!(took < BUDGET, "the poll of a frame was blocked on the FIFO for {took:?}");
    assert!(cleared_run_ids(&actions).is_empty(), "a FIFO is no status");
}

#[test]
fn a_link_at_agent_status_json_is_not_read_and_a_plain_file_is() {
    let status = br#"{"state":"done","run_id":"victim-run"}"#;
    let mut st = state_with_session_folder();
    std::fs::write(st.session_dir.join("agent-status.json"), status).expect("a plain status file");
    assert_eq!(cleared_run_ids(&st.poll_live_agent_bridge()), ["victim-run"], "control: a plain status file in the session folder is read");

    let (target, _target) = target_with_sentinel("status-link-target");
    let mut st = state_with_session_folder();
    std::fs::write(target.join("victim.json"), status).expect("a file elsewhere that holds a done status");
    symlink(target.join("victim.json"), st.session_dir.join("agent-status.json")).expect("plant the link");

    let actions = st.poll_live_agent_bridge();

    assert!(cleared_run_ids(&actions).is_empty(), "the victim's status reached the tab: {actions:?}");
    assert_eq!(std::fs::read(target.join("victim.json")).expect("the victim"), status, "the poll changed the target");
}

#[test]
fn a_fifo_at_a_log_or_a_capture_the_agent_writes_does_not_freeze_its_reader() {
    let mut frozen = Vec::new();
    let mut check = |name: &str, read: &dyn Fn(&Path)| {
        let (session, _session) = made_session_folder("fifo-readers");
        let fifo = session.join(name);
        make_fifo(&fifo);
        let ((), took) = timed_with_fifo_watchdog(&fifo, BUDGET, || read(&fifo));
        if took >= BUDGET {
            frozen.push(format!("{name}: blocked on the FIFO for {took:?}"));
        }
    };
    check("doc-patch.jsonl", &|path| drop(crate::live_patch_log::LivePatchLog::bind(path.to_path_buf())));
    check("turn-marks.jsonl", &|path| drop(crate::agent_turn_marks::TurnMarkLog::bind(path.to_path_buf()).poll()));
    check("claude-transcript.json", &|path| drop(crate::agent_usage::read_claude_transcript_meta(path)));
    assert!(frozen.is_empty(), "{}", frozen.join("\n"));
}

#[test]
fn a_link_swapped_in_for_the_events_log_is_not_tailed() {
    let (session, _session) = made_session_folder("events-link");
    let log = session.join("agent-events.jsonl");
    std::fs::write(&log, b"").expect("an empty events log");
    let stream = crate::agent_turn_status::AgentEventStream::bind(log.clone());
    let (target, _target) = target_with_sentinel("events-link-target");
    std::fs::write(target.join("victim.jsonl"), b"{\"t\":1.0,\"e\":\"Stop\",\"p\":{\"session_id\":\"victim\"}}\n")
        .expect("a file elsewhere that holds an event");
    std::fs::remove_file(&log).expect("the agent removes the log");
    symlink(target.join("victim.jsonl"), &log).expect("and plants a link in its place");

    let records = stream.poll(true);

    assert!(records.is_empty(), "the victim's event was tailed: {} record(s)", records.len());
}
