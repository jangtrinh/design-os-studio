//! `live-spec.json` is a spec the agent writes for Studio to apply, hot, on the UI thread: `poll_live_agent_bridge` notices it by its
//! modification time and `execute_terminal_actions` reads it. The agent can plant a FIFO there (an idle window then freezes) or a link
//! to any file the user owns (which was then read and applied). A refused spec is no spec: nothing is applied and no toast is shown.

use super::folder_tests::{scratch_dir, target_with_sentinel, Cleanup};
use super::test_support::{make_fifo, timed_with_fifo_watchdog};
use super::*;
use crate::live_sync_tests::unique_tab_id;
use crate::terminal::{TerminalAction, TerminalState};
use std::os::unix::fs::symlink;

const BUDGET: Duration = Duration::from_secs(30);
const SPEC: &str = r#"{"name":"Spec From The Agent","preset":"easeui-card"}"#;

/// A window with one page and a terminal whose session folder Studio made.
fn window_with_session_folder(tag: &str) -> (crate::app::App, Cleanup) {
    let mut doc = designos_doc::Document::new(tag);
    doc.add_page("Page 1");
    let mut app = crate::app::App::new(doc);
    app.terminal = TerminalState::for_tab(unique_tab_id());
    let session = app.terminal.session_dir.clone();
    ensure_session_dir(&session).expect("the session folder");
    (app, Cleanup(vec![session]))
}

fn toast_of(app: &crate::app::App) -> Option<String> {
    app.toast.as_ref().map(|toast| toast.text.clone())
}

fn asks_for_the_spec(actions: &[TerminalAction]) -> bool {
    actions.iter().any(|action| matches!(action, TerminalAction::ApplySpecFile(_)))
}

#[test]
fn a_plain_live_spec_is_still_applied() {
    let (mut app, _session) = window_with_session_folder("live-spec-plain");
    std::fs::write(&app.terminal.live_spec_path, SPEC).expect("a spec");
    let nodes_before = app.doc.nodes.len();

    let actions = app.terminal.poll_live_agent_bridge();
    assert!(asks_for_the_spec(&actions), "control: a plain spec is noticed");
    crate::shell::execute_terminal_actions(&mut app, actions);

    assert!(app.doc.nodes.len() > nodes_before, "control: a plain spec is applied");
    assert!(app.specs.live_outputs.contains_key(&app.terminal.live_spec_path));
}

#[test]
fn a_fifo_at_live_spec_json_does_not_freeze_the_ui_thread() {
    let (mut app, _session) = window_with_session_folder("live-spec-fifo");
    let fifo = app.terminal.live_spec_path.clone();
    make_fifo(&fifo);

    let (toast, took) = timed_with_fifo_watchdog(&fifo, BUDGET, || {
        let actions = app.terminal.poll_live_agent_bridge();
        crate::shell::execute_terminal_actions(&mut app, actions);
        toast_of(&app)
    });

    assert!(took < BUDGET, "the poll and the handler of a frame were blocked on the FIFO for {took:?}");
    assert_eq!(toast, None, "a FIFO is no spec, and no toast says it is");
}

#[test]
fn a_link_at_live_spec_json_is_not_read_applied_or_reported() {
    let (target, _target) = target_with_sentinel("live-spec-link-target");
    for (name, content) in [("a-spec.json", SPEC), ("not-a-spec.txt", "a line of a file that is the user's\n")] {
        let (mut app, _session) = window_with_session_folder("live-spec-link");
        std::fs::write(target.join(name), content).expect("a file elsewhere");
        symlink(target.join(name), &app.terminal.live_spec_path).expect("plant the link");
        let nodes_before = app.doc.nodes.len();

        let actions = app.terminal.poll_live_agent_bridge();
        crate::shell::execute_terminal_actions(&mut app, actions);

        assert_eq!(app.doc.nodes.len(), nodes_before, "{name}: the file behind the link was applied");
        assert!(!app.specs.live_outputs.contains_key(&app.terminal.live_spec_path), "{name}: the file behind the link was read as a spec");
        assert_eq!(toast_of(&app), None, "{name}: a toast reports a file behind a link, which is a read of it");
    }
}

#[test]
fn a_change_to_the_target_of_a_link_at_live_spec_json_asks_for_nothing() {
    let scratch = scratch_dir("live-spec-link-changes");
    let _scratch = Cleanup(vec![scratch.clone()]);
    let (mut app, _session) = window_with_session_folder("live-spec-link-mtime");
    std::fs::write(scratch.join("elsewhere.json"), SPEC).expect("a file elsewhere");
    symlink(scratch.join("elsewhere.json"), &app.terminal.live_spec_path).expect("plant the link");

    assert!(!asks_for_the_spec(&app.terminal.poll_live_agent_bridge()), "a link is no spec");
    std::fs::write(scratch.join("elsewhere.json"), format!("{SPEC} ")).expect("the file behind the link changes");

    assert!(!asks_for_the_spec(&app.terminal.poll_live_agent_bridge()), "a change behind the link is no change of the spec");
}
