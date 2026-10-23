//! "An idle window asks for no immediate repaint", the check the open-question tests share.
//!
//! The rule is about steady state: a window that has nothing to do must never ask egui for a repaint in a loop, or the
//! app burns a core while it sits idle. A frame that asks once is not a loop, though, because work outside the frame
//! can wake an idle window:
//! - a background job that finishes late (the disk watcher notices a write, a scan ends) calls `request_repaint()` from
//!   its own thread, at a moment no test controls, and
//! - egui answers every immediate request with one more pass that reports another zero-delay request, so one wake shows up
//!   as two requests: the one of the job and the one at the start of the next pass.
//!
//! On a loaded machine such a wake lands inside the frames a test counts, so a test that fails on the first immediate
//! request fails for a reason that has nothing to do with the app. This check waits for the window to go quiet instead:
//! it passes once [`QUIET_FRAMES`] frames in a row ask for nothing, and it fails when more than [`WAKE_ALLOWANCE`] frames
//! ask. A wake costs one or two asking frames, so it only postpones the quiet run; an app that repaints in a loop asks in
//! (nearly) every frame and runs out of the allowance within a few frames.

use super::unsaved_dot::{external_sim_with, two_rects};
use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

/// Frames in a row that must ask for no immediate repaint: the length of the idle window these tests always watched.
pub(super) const QUIET_FRAMES: usize = 40;

/// Frames that may ask for an immediate repaint before the window is called a loop. One wake from another thread costs
/// one or two; this leaves room for a few of them and none for a window that keeps asking.
pub(super) const WAKE_ALLOWANCE: usize = 8;

/// Why a window never went quiet.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct NeverQuiet {
    /// Frames that ran before the allowance was used up.
    pub frames: usize,
    /// Frames of those that asked for an immediate repaint.
    pub asking: usize,
}

/// Calls `frame` until `quiet` calls in a row returned `false` (the frame asked for no immediate repaint), and returns
/// how many ran. More than `allowed_asking` frames that returned `true` is an error: the window keeps asking.
pub(super) fn run_until_quiet(quiet: usize, allowed_asking: usize, mut frame: impl FnMut() -> bool) -> Result<usize, NeverQuiet> {
    let (mut frames, mut asking, mut quiet_in_a_row) = (0, 0, 0);
    loop {
        frames += 1;
        if frame() {
            asking += 1;
            quiet_in_a_row = 0;
            if asking > allowed_asking {
                return Err(NeverQuiet { frames, asking });
            }
        } else {
            quiet_in_a_row += 1;
            if quiet_in_a_row == quiet {
                return Ok(frames);
            }
        }
    }
}

impl Sim {
    /// Runs idle frames until [`QUIET_FRAMES`] of them in a row asked egui for no immediate repaint, and returns how many
    /// frames ran. A request from another thread between two frames is counted in the frame after it: egui reports the
    /// pass it owes for that request at the start of that frame.
    pub(super) fn idle_frames_until_quiet(&mut self) -> Result<usize, NeverQuiet> {
        self.idle_frames_until_quiet_with(|_| {})
    }

    /// [`Self::idle_frames_until_quiet`], with `before_each_frame` run on this thread just before every frame.
    fn idle_frames_until_quiet_with(&mut self, mut before_each_frame: impl FnMut(&egui::Context)) -> Result<usize, NeverQuiet> {
        let asked = Arc::new(AtomicUsize::new(0));
        let asked_in_callback = asked.clone();
        self.ctx.set_request_repaint_callback(move |info| {
            if info.delay.is_zero() {
                asked_in_callback.fetch_add(1, Ordering::SeqCst);
            }
        });
        let ctx = self.ctx.clone();
        run_until_quiet(QUIET_FRAMES, WAKE_ALLOWANCE, || {
            asked.store(0, Ordering::SeqCst);
            before_each_frame(&ctx);
            self.frame(vec![]);
            asked.load(Ordering::SeqCst) > 0
        })
    }

    /// The assertion of the open-question tests: the window goes quiet and stays quiet.
    pub(super) fn assert_idle_window_asks_for_no_repaint(&mut self, what: &str) {
        if let Err(never) = self.idle_frames_until_quiet() {
            panic!(
                "an idle window with {what} keeps asking for an immediate repaint: {} of the first {} frames asked, and no run of {QUIET_FRAMES} quiet frames came",
                never.asking, never.frames
            );
        }
    }
}

#[test]
fn a_window_that_asks_in_every_frame_never_goes_quiet() {
    let never = run_until_quiet(QUIET_FRAMES, WAKE_ALLOWANCE, || true).unwrap_err();
    assert_eq!(never, NeverQuiet { frames: WAKE_ALLOWANCE + 1, asking: WAKE_ALLOWANCE + 1 });
}

#[test]
fn a_window_that_asks_now_and_then_never_goes_quiet() {
    // a loop that fires every 30th frame leaves no run of 40 quiet frames
    let mut frame = 0;
    let never = run_until_quiet(QUIET_FRAMES, WAKE_ALLOWANCE, || {
        frame += 1;
        frame % 30 == 0
    })
    .unwrap_err();
    assert_eq!(never.asking, WAKE_ALLOWANCE + 1);
}

#[test]
fn a_wake_and_the_pass_egui_owes_it_only_postpone_the_quiet_run() {
    // the wake of a background job (frame 3) and the pass egui runs for it (frame 4)
    let mut frame = 0;
    let frames = run_until_quiet(QUIET_FRAMES, WAKE_ALLOWANCE, || {
        frame += 1;
        frame == 3 || frame == 4
    })
    .unwrap();
    assert_eq!(frames, 4 + QUIET_FRAMES, "the quiet run starts after the last frame that asked");
}

#[test]
fn a_window_that_never_asks_goes_quiet_after_exactly_the_quiet_frames() {
    assert_eq!(run_until_quiet(QUIET_FRAMES, WAKE_ALLOWANCE, || false), Ok(QUIET_FRAMES));
}

#[test]
fn a_repaint_another_thread_asks_for_is_waited_out() {
    let (mut s, _dir, _path) = external_sim_with(&two_rects());
    s.idle_frames_until_quiet().expect("an idle window goes quiet");
    // a job that finishes late wakes the idle window from its own thread, between two frames
    let ctx = s.ctx.clone();
    std::thread::spawn(move || ctx.request_repaint()).join().unwrap();
    let frames = s.idle_frames_until_quiet().expect("one wake does not make a loop");
    assert!(frames > QUIET_FRAMES, "the pass egui owes the wake counts as a frame that asked: {frames}");
    assert!(frames <= QUIET_FRAMES + WAKE_ALLOWANCE, "{frames}");
}

#[test]
fn a_window_that_repaints_in_every_frame_is_caught_on_a_real_sim() {
    let (mut s, _dir, _path) = external_sim_with(&two_rects());
    s.idle_frames_until_quiet().expect("an idle window goes quiet");
    let never = s.idle_frames_until_quiet_with(|ctx| ctx.request_repaint()).unwrap_err();
    assert_eq!(never.asking, WAKE_ALLOWANCE + 1, "{never:?}");
}
