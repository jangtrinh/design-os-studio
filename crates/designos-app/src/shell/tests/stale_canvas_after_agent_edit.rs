//! The canvas picture follows the document after an agent deletes a layer, after the review card's
//! Revert and after an undo of an agent step (#928: QA saw the old picture for 50 s or more on a slow
//! Linux machine with the GPU off).
//!
//! `Sim::frame` runs a frame whether or not egui asked for one, so every "no input" test written with it
//! passes for the wrong reason. These tests run frames the way `eframe` does: after the edit, a frame runs
//! only when the previous one asked for another (right away, or after a delay that elapses within the
//! window), and nothing else wakes the window.

use super::*;
use crate::canvas::tiles::picture_errors;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Seconds the window is watched after an edit. The Agents panel repaints every 50 ms or 500 ms, so a
/// picture that is still wrong after this long is stuck, not late.
const WATCH_SECS: f64 = 2.0;

/// Real time to wait for a wake-up that must come: generous, because a loaded machine runs a debug build slowly.
const WAKE_WAIT: Duration = Duration::from_secs(20);

/// A window bound to a live agent session, with the Agents panel closed (no shell is spawned, so no
/// timer repaints the window).
struct Live {
    sim: Sim,
    session: designos_mcp::server::Session,
    workspace: std::path::PathBuf,
    /// How long the last frame said the window can sleep before the next one.
    wake: Duration,
    /// Repaints another thread asked egui for while no frame ran, and how many of them a frame has answered.
    wakes: Arc<AtomicUsize>,
    answered_wakes: usize,
}

impl Live {
    fn new() -> Self {
        let sim = Sim::new();
        let wakes = Arc::new(AtomicUsize::new(0));
        let (count, ui_thread) = (wakes.clone(), std::thread::current().id());
        // Repaints requested by the frames themselves are in each frame's schedule; only another thread (the
        // log watcher) wakes a sleeping window from outside.
        sim.ctx.set_request_repaint_callback(move |info| {
            if info.delay == Duration::ZERO && std::thread::current().id() != ui_thread {
                count.fetch_add(1, Ordering::SeqCst);
            }
        });
        assert!(!sim.app().canvas.gpu.active(), "precondition: the CPU tile path, as on the QA machine (GPU off)");
        let workspace = std::env::temp_dir().join(format!("designos-stale-canvas-ws-{}", crate::live_sync_tests::unique_tab_id()));
        std::fs::create_dir_all(&workspace).unwrap();
        let session = designos_mcp::server::Session::with_session_dir(&workspace, &sim.app().terminal.session_dir).expect("mcp session");
        Live { sim, session, workspace, wake: Duration::ZERO, wakes, answered_wakes: 0 }
    }

    fn marks_path(&self) -> std::path::PathBuf {
        self.sim.app().terminal.session_dir.join("turn-marks.jsonl")
    }

    fn patch_path(&self) -> std::path::PathBuf {
        self.sim.app().terminal.session_dir.join("doc-patch.jsonl")
    }

    /// The CLI hook that brackets an agent turn (what makes the review card appear).
    fn turn_mark(&self, phase: &str) {
        let cmd = crate::agent_turn_marks::turn_mark_hook_command(&self.marks_path(), &self.patch_path(), phase, "codex");
        let status = std::process::Command::new("sh").arg("-c").arg(&cmd).status().expect("sh hook");
        assert!(status.success(), "hook command failed: {cmd}");
    }

    fn agent_deletes(&mut self, id: NodeId) {
        self.session.call("designos_delete_node", serde_json::json!({ "node_id": id.0 })).expect("agent delete");
    }

    /// A frame with an input event or an empty one (`Sim::frame`). egui schedules the frame after an event by itself.
    fn frame(&mut self) {
        self.sim.frame(vec![]);
        self.wake = Duration::ZERO;
    }

    fn cmd_key(&mut self, key: Key) {
        self.sim.cmd_key(key);
        self.wake = Duration::ZERO;
    }

    /// Frames as `eframe` runs them from now on: no input event, a frame only when the last one asked for
    /// another within the window. Returns how many ran.
    fn watch(&mut self) -> usize {
        let (frames, wake) = watch_idle(&mut self.sim, self.wake, WATCH_SECS);
        self.wake = wake;
        frames
    }

    /// Lets real time pass with the window untouched until `done` holds or `real` is up. A frame runs only
    /// when egui's schedule asks for one or something repaints the window from another thread (the log
    /// watcher): what `eframe` does with no input.
    fn idle_until(&mut self, real: Duration, done: impl Fn(&Sim) -> bool) -> bool {
        let deadline = Instant::now() + real;
        loop {
            let wakes = self.wakes();
            if wakes != self.answered_wakes {
                self.answered_wakes = wakes;
                self.wake = Duration::ZERO;
            }
            self.watch();
            if done(&self.sim) {
                return true;
            }
            if Instant::now() >= deadline {
                return false;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// Repaints requested from outside a frame so far (by the log watcher).
    fn wakes(&self) -> usize {
        self.wakes.load(Ordering::SeqCst)
    }

    fn picture_errors(&mut self) -> Vec<String> {
        let ppp = self.sim.ctx.pixels_per_point();
        picture_errors(&mut self.sim.shell.app, ppp)
    }

    /// The review card's Revert button: the action lands after the canvas painted in the frame of the click, and
    /// egui schedules the frame after it. The turn must end `Reverted` with the plain "Reverted" toast, the path
    /// QA's `ai-gui-11b` shows, not a refusal.
    fn revert(&mut self, turn: u64) {
        crate::agent_turn_card::apply_card_action(&mut self.sim.shell.app, 0, crate::agent_turn_card::TurnCardAction::Revert(turn));
        self.wake = Duration::ZERO;
        let state = self.sim.app().terminal.active_turn_ledger().turns.iter().find(|t| t.id == turn).map(|t| t.state);
        assert_eq!(state, Some(crate::agent_turn_ledger::TurnState::Reverted), "Revert refused");
        let toast = self.sim.app().toast.as_ref().map(|t| t.text.clone()).unwrap_or_default();
        assert!(toast.starts_with("Reverted"), "unexpected toast after Revert: {toast:?}");
    }

    fn turn_id(&self) -> u64 {
        self.sim.app().terminal.active_turn_ledger().visible_turn(std::time::Instant::now()).expect("the agent turn has a review card").id
    }
}

impl Drop for Live {
    fn drop(&mut self) {
        let dir = self.sim.app().terminal.session_dir.clone();
        self.sim.shell.app.terminal.shutdown();
        let _ = std::fs::remove_dir_all(dir);
        let _ = std::fs::remove_dir_all(&self.workspace);
    }
}

/// One frame like `Sim::run_frame`, returning how long egui says it can wait before the next one:
/// `Duration::ZERO` asks for a frame right away, `Duration::MAX` for none until an input event.
fn frame_and_wait(s: &mut Sim, events: Vec<Event>, dt: f64) -> Duration {
    s.t += dt;
    let mut raw = RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1600.0, 1088.0))),
        time: Some(s.t),
        events,
        modifiers: s.mods,
        ..Default::default()
    };
    let ctx = s.ctx.clone();
    let shell = &mut s.shell;
    eframe::App::raw_input_hook(shell, &ctx, &mut raw);
    let out = ctx.run_ui(raw, |ui| shell.draw(ui));
    s.cursor = out.platform_output.cursor_icon;
    s.shapes = out.shapes;
    s.accesskit_update = out.platform_output.accesskit_update;
    out.viewport_output.get(&egui::ViewportId::ROOT).map_or(Duration::MAX, |v| v.repaint_delay)
}

/// Runs a frame at each wake-up egui asked for, starting with `next` (how long the last frame said it could
/// wait), until the next one is more than `window` seconds away. Returns the frames run and the wake-up
/// still pending (`Duration::MAX` when the window sleeps until an input event).
fn watch_idle(s: &mut Sim, mut next: Duration, window: f64) -> (usize, Duration) {
    let mut frames = 0;
    let mut waited = 0.0;
    while next.as_secs_f64() + waited <= window && frames < 2_000 {
        let dt = next.as_secs_f64().max(0.016);
        waited += dt;
        next = frame_and_wait(s, vec![], dt);
        frames += 1;
    }
    (frames, next)
}

fn assert_picture(l: &mut Live, what: &str) {
    let errors = l.picture_errors();
    assert!(errors.is_empty(), "{what}: the canvas does not show the document: {errors:?}");
}

/// The middle child of the demo card: an auto-layout frame (Hug height, drop shadow), so removing it moves
/// the layers below it and shrinks the card, like the QA login card.
fn card_row(s: &Sim) -> NodeId {
    s.by_name("Revenue grew 18% over the last quarter.")
}

fn names_in(s: &Sim, parent: NodeId) -> Vec<String> {
    s.app().doc.get(parent).unwrap().children.iter().filter_map(|c| s.app().doc.get(*c)).map(|n| n.name.clone()).collect()
}

#[test]
fn precondition_the_window_paints_the_document_before_any_edit() {
    let mut l = Live::new();
    l.watch();
    assert_picture(&mut l, "before any edit");
}

/// The check is not vacuous: a document change that nothing marked as changed leaves the old picture on
/// screen, and the probe says so.
#[test]
fn the_picture_check_reports_a_canvas_that_was_never_told_about_a_change() {
    let mut l = Live::new();
    l.watch();
    let row = card_row(&l.sim);
    l.sim.shell.app.doc.remove(row).unwrap();
    l.frame();
    l.watch();
    let errors = l.picture_errors();
    assert!(errors.iter().any(|e| e.contains("does not show the document")), "the probe missed a stale picture: {errors:?}");
}

/// QA's first step: the agent deletes a layer through the MCP socket and the live edit log.
#[test]
fn an_agent_delete_through_the_live_log_is_on_the_canvas_within_two_frames() {
    let mut l = Live::new();
    l.watch();
    let (row, card) = (card_row(&l.sim), l.sim.by_name("Card"));
    let before = names_in(&l.sim, card);
    l.turn_mark("start");
    l.agent_deletes(row);
    l.turn_mark("end");
    // The window sees the edit on its next frame and draws it in that frame or the one after.
    l.frame();
    l.frame();
    assert!(l.sim.app().doc.get(row).is_none(), "the agent's delete reached the document");
    assert_eq!(names_in(&l.sim, card).len(), before.len() - 1);
    assert_picture(&mut l, "two frames after the agent delete");
}

/// Same step with the Agents panel closed and no input: no frame runs unless egui or another thread asks
/// for one. Before the log watcher the editor never read the log here, so the canvas showed the old
/// picture until the next mouse or key event.
#[test]
fn an_agent_delete_reaches_a_window_that_receives_no_input() {
    let mut l = Live::new();
    l.watch();
    let row = card_row(&l.sim);
    l.turn_mark("start");
    l.agent_deletes(row);
    l.turn_mark("end");
    let applied = l.idle_until(WAKE_WAIT, |s| s.app().doc.get(row).is_none());
    assert!(applied, "the agent deleted the layer {} s ago and the window (Agents panel closed, no input) never read the edit log", WAKE_WAIT.as_secs());
    assert_picture(&mut l, "after the agent delete, no input");
}

/// The wake-up is one repaint per write: a window an agent is not writing to schedules nothing.
#[test]
fn an_idle_window_with_a_live_session_asks_for_no_repaint() {
    let mut l = Live::new();
    // The log watcher looks at the files once on its own thread; let that settle, then nothing may follow.
    l.idle_until(Duration::from_secs(2), |_| false);
    let settled = l.wakes();
    l.idle_until(Duration::from_secs(1), |_| false);
    assert_eq!(l.wakes(), settled, "an idle window with nobody writing must stay idle");
    assert_eq!(l.wake, Duration::MAX, "egui sleeps until an input event (0% idle CPU)");
}

/// QA's second step: Revert in the review card. The click lands after the canvas painted in its frame, and
/// egui always schedules the frame after an input event.
#[test]
fn revert_in_the_review_card_puts_the_layer_back_on_the_canvas() {
    let mut l = Live::new();
    l.watch();
    let (row, card) = (card_row(&l.sim), l.sim.by_name("Card"));
    let before = names_in(&l.sim, card);
    l.turn_mark("start");
    l.agent_deletes(row);
    l.turn_mark("end");
    l.frame();
    l.frame();
    assert_picture(&mut l, "after the delete");
    let turn = l.turn_id();
    l.revert(turn);
    assert_eq!(names_in(&l.sim, card), before, "Revert restored the layer in the document");
    l.watch();
    assert_picture(&mut l, "after Revert");
}

/// QA pressed Ctrl+Z after the agent's delete.
#[test]
fn undo_of_an_agent_delete_puts_the_layer_back_on_the_canvas() {
    let mut l = Live::new();
    l.watch();
    let (row, card) = (card_row(&l.sim), l.sim.by_name("Card"));
    let before = names_in(&l.sim, card);
    l.turn_mark("start");
    l.agent_deletes(row);
    l.turn_mark("end");
    l.frame();
    l.frame();
    assert_picture(&mut l, "after the delete");
    l.cmd_key(Key::Z);
    assert_eq!(names_in(&l.sim, card), before, "Ctrl+Z restored the layer in the document");
    l.watch();
    assert_picture(&mut l, "after Ctrl+Z");
}

/// QA's whole sequence: delete, Ctrl+Z, then Revert on the turn that was already undone. The document and
/// the canvas must stay in step and the layer must not come back twice.
#[test]
fn revert_after_an_undo_of_the_same_turn_keeps_document_and_canvas_in_step() {
    let mut l = Live::new();
    l.watch();
    let (row, card) = (card_row(&l.sim), l.sim.by_name("Card"));
    let before = names_in(&l.sim, card);
    l.turn_mark("start");
    l.agent_deletes(row);
    l.turn_mark("end");
    l.frame();
    l.frame();
    let turn = l.turn_id();
    l.cmd_key(Key::Z);
    l.watch();
    assert_picture(&mut l, "after Ctrl+Z");
    l.revert(turn);
    l.watch();
    assert_eq!(names_in(&l.sim, card), before, "the layer is back exactly once");
    assert_picture(&mut l, "after Revert of an undone turn");
}

/// Appends a blank line to a log: its length changes, and the reader skips the line.
fn touch_log(path: &std::path::Path) {
    use std::io::Write;
    let mut f = std::fs::OpenOptions::new().create(true).append(true).open(path).expect("open the log");
    f.write_all(b"\n").expect("append to the log");
}

fn parked_log_watcher_running(s: &Sim, log: &std::path::Path) -> Option<bool> {
    s.app().tabs.entries.iter().find(|t| t.terminal.live_patch.path == log).map(|t| t.terminal.live_patch.wake_running())
}

/// A project tab that is not showing has no watcher: an agent still writing there must not wake the window for
/// frames that read nothing (only the active tab's log is polled). Showing the tab again restarts the watcher,
/// whose first look wakes for what was written meanwhile.
#[test]
fn a_parked_tabs_log_does_not_wake_the_active_window() {
    let mut l = Live::new();
    let first_log = l.patch_path();
    touch_log(&first_log);
    l.frame();
    l.idle_until(Duration::from_secs(2), |_| false);
    assert!(l.sim.app().terminal.live_patch.wake_running(), "precondition: the showing tab's log is watched");

    // A second project tab opens and the first is parked.
    l.sim.shell.app.open_new_design_tab();
    l.frame();
    l.frame();
    let second_dir = l.sim.app().terminal.session_dir.clone();
    assert_ne!(l.patch_path(), first_log, "precondition: the new tab has its own session");
    l.idle_until(Duration::from_secs(2), |_| false);
    let before = l.wakes();
    touch_log(&first_log);
    l.idle_until(Duration::from_secs(2), |_| false);
    assert_eq!(l.wakes(), before, "the parked tab's agent woke the window for frames that read nothing");
    assert_eq!(parked_log_watcher_running(&l.sim, &first_log), Some(false), "a parked tab keeps no watcher thread");

    // The first tab shows again and its watcher restarts. (Its first look wakes once for what was written
    // meanwhile; egui folds that wake into a repaint a frame already asked for, so it is not asserted here.)
    let first = l.sim.app().tabs.entries.iter().position(|t| t.terminal.live_patch.path == first_log).expect("the parked tab");
    l.sim.shell.app.switch_tab(first);
    l.frame();
    l.frame();
    assert_eq!(l.patch_path(), first_log);
    assert!(l.sim.app().terminal.live_patch.wake_running(), "the watcher did not restart when the tab showed again");
    l.idle_until(Duration::from_secs(2), |_| false);
    let (wakes, count) = (l.wakes.clone(), l.wakes());
    touch_log(&first_log);
    assert!(l.idle_until(WAKE_WAIT, move |_| wakes.load(Ordering::SeqCst) > count), "a write after the tab showed again woke nothing");

    for tab in &mut l.sim.shell.app.tabs.entries {
        tab.terminal.shutdown();
    }
    let _ = std::fs::remove_dir_all(second_dir);
}
