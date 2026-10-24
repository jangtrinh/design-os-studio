//! `--screenshot` with the GPU canvas waits for the at-rest frame (the one full-quality
//! refine compile) instead of capturing the capped interactive frame.
//! The canvas is compiled for real (compiling needs no GPU device); only the drawing is absent.

use super::*;
use crate::canvas::gpu::IDLE_REFINE_DELAY;
use designos_doc::live_patch::{LiveOp, LivePatch};
use egui::RawInput;
use std::path::{Path, PathBuf};
use std::sync::Arc;

const SIZE: (u32, u32) = (1600, 1088);
/// Seconds of input time per frame, small enough that the idle delay spans many frames.
const DT: f64 = 0.01;

fn shot_sim(gpu: bool, tag: &str) -> (Sim, PathBuf) {
    let dir = std::env::temp_dir().join(format!("designos_gpu_screenshot_wait_{}_{tag}", std::process::id()));
    let path = dir.join("shot.png");
    let mut s = Sim::with(Options { screenshot: Some(path.clone()), size: Some(SIZE), ..Default::default() });
    if gpu {
        let g = &mut s.shell.app.canvas.gpu;
        (g.available, g.pref) = (true, true);
        g.set_max_texture(16_384);
    }
    (s, path)
}

fn raw_input(t: f64, events: Vec<Event>) -> RawInput {
    RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(SIZE.0 as f32, SIZE.1 as f32))), time: Some(t), events, ..Default::default() }
}

/// One window frame; true when it asked egui for the screenshot.
fn frame_requests_screenshot(s: &mut Sim) -> bool {
    s.t += DT;
    let ctx = s.ctx.clone();
    let shell = &mut s.shell;
    let out = ctx.run_ui(raw_input(s.t, vec![]), |ui| shell.draw(ui));
    out.viewport_output.values().any(|v| v.commands.iter().any(|c| matches!(c, egui::ViewportCommand::Screenshot(_))))
}

/// Frames run until the screenshot is requested, None past `limit` frames.
fn frames_until_screenshot(s: &mut Sim, limit: u32) -> Option<u32> {
    (1..=limit).find(|_| frame_requests_screenshot(s))
}

/// What a window run as eframe runs it did, frame by frame.
#[derive(Debug, Default)]
struct Run {
    /// Once the frame had painted, the canvas showed its at-rest frame (the GPU's refined one, or the CPU's).
    at_rest: Vec<bool>,
    /// Frames that issued a screenshot command.
    requests: Vec<usize>,
    /// The frame the saved image was captured from, once the app saved one.
    saved_from: Option<usize>,
    /// The frame that asked the window to close.
    closed_at: Option<usize>,
}

impl Run {
    /// The first frame whose paint was the at-rest frame.
    fn first_rest(&self) -> Option<usize> {
        self.at_rest.iter().position(|r| *r)
    }
}

/// Frames between the paint a screenshot is captured from and the frame whose input carries the
/// image: the read-back of the GPU frame is asynchronous (a real `--gpu` run delivers it two
/// frames after the capture).
const DELIVERY_LAG: usize = 2;

/// Runs the window like eframe does: a screenshot command issued by a frame is captured from the
/// paint of the next frame, and the image reaches the app as an event `lag` frames after that.
/// `before_frame` runs ahead of every frame, like input or a background load landing between two.
fn run_window(s: &mut Sim, shot: &Path, limit: usize, lag: usize, mut before_frame: impl FnMut(&mut Sim, &Run)) -> Run {
    let mut run = Run::default();
    let mut asked_in: Option<usize> = None;
    // (frame the image was captured from, frame whose input carries it)
    let mut in_flight: Option<(usize, usize)> = None;
    for n in 0..limit {
        before_frame(s, &run);
        let delivered = in_flight.filter(|(_, at)| *at == n).map(|(from, _)| from);
        if delivered.is_some() {
            in_flight = None;
        }
        let events = delivered
            .map(|_| {
                let pixels = [SIZE.0 as usize, SIZE.1 as usize];
                let image = Arc::new(egui::ColorImage::filled(pixels, egui::Color32::BLACK));
                vec![Event::Screenshot { viewport_id: egui::ViewportId::ROOT, user_data: egui::UserData::default(), image }]
            })
            .unwrap_or_default();
        s.t += DT;
        let ctx = s.ctx.clone();
        let shell = &mut s.shell;
        let out = ctx.run_ui(raw_input(s.t, events), |ui| shell.draw(ui));
        run.at_rest.push(s.app().canvas.gpu.at_rest());
        if asked_in.take().is_some() {
            in_flight = Some((n, n + lag));
        }
        let commands = || out.viewport_output.values().flat_map(|v| v.commands.iter());
        if commands().any(|c| matches!(c, egui::ViewportCommand::Screenshot(_))) {
            run.requests.push(n);
            asked_in = Some(n);
        }
        if delivered.is_some() && run.saved_from.is_none() && shot.exists() {
            run.saved_from = delivered;
        }
        if commands().any(|c| matches!(c, egui::ViewportCommand::Close)) {
            run.closed_at = Some(n);
            break;
        }
    }
    run
}

fn cleanup(shot: &Path) {
    if let Some(dir) = shot.parent() {
        let _ = std::fs::remove_dir_all(dir);
    }
}

/// The saved image came from a frame the canvas showed at rest.
fn assert_saved_at_rest(run: &Run) {
    let from = run.saved_from.unwrap_or_else(|| panic!("no image was saved: {run:?}"));
    assert!(run.at_rest[from], "the saved image was captured from capped frame {from}: {run:?}");
}

#[test]
fn gpu_screenshot_waits_for_the_refined_frame() {
    let (mut s, shot) = shot_sim(true, "waits");
    let n = frames_until_screenshot(&mut s, 150).expect("a screenshot is requested");
    assert!(s.app().canvas.gpu.at_rest(), "the capture is requested once the refined frame is cached");
    let idle_frames = (IDLE_REFINE_DELAY / DT) as u32;
    assert!(n > idle_frames, "took {n} frames, the idle delay alone is {idle_frames}: the capped interactive frame was not captured");
    cleanup(&shot);
}

#[test]
fn screenshot_of_a_canvas_not_on_the_gpu_is_taken_as_before() {
    let (mut s, shot) = shot_sim(false, "cpu");
    let n = frames_until_screenshot(&mut s, 150).expect("a screenshot is requested");
    assert!((1..=12).contains(&n), "after the usual dozen frames, took {n}");
    cleanup(&shot);
}

#[cfg(unix)]
#[test]
fn screenshot_never_writes_through_a_final_symlink() {
    let (mut s, shot) = shot_sim(false, "symlink");
    std::fs::create_dir_all(shot.parent().unwrap()).unwrap();
    let victim = shot.parent().unwrap().join("victim.png");
    std::fs::write(&victim, b"keep this image").unwrap();
    std::os::unix::fs::symlink(&victim, &shot).unwrap();
    let _ = run_window(&mut s, &shot, 40, DELIVERY_LAG, |_, _| {});
    assert_eq!(std::fs::read(&victim).unwrap(), b"keep this image");
    assert!(std::fs::symlink_metadata(&shot).unwrap().file_type().is_symlink());
    cleanup(&shot);
}

#[test]
fn screenshot_gives_up_waiting_for_a_view_that_never_settles() {
    let (mut s, shot) = shot_sim(true, "never_settles");
    // a view that moves every frame never goes idle
    let n = (1..=400)
        .find(|_| {
            s.shell.app.view.offset.x += 1.0;
            frame_requests_screenshot(&mut s)
        })
        .expect("the screenshot is eventually requested");
    assert!((100..240).contains(&n), "settle limit hit after {n} frames, before the 240 frame close");
    assert!(!s.app().canvas.gpu.at_rest(), "it captured the interactive frame, as announced");
    cleanup(&shot);
}

#[test]
fn a_settled_gpu_view_is_captured_once_from_its_refined_frame() {
    let (mut s, shot) = shot_sim(true, "settled");
    let run = run_window(&mut s, &shot, 150, DELIVERY_LAG, |_, _| {});
    assert_saved_at_rest(&run);
    assert_eq!(run.requests.len(), 1, "nothing disturbed the view, so the first capture is the one kept: {run:?}");
    assert!(run.closed_at.is_some(), "the window closes once the image is saved: {run:?}");
    cleanup(&shot);
}

#[test]
fn a_cpu_canvas_is_saved_after_the_usual_dozen_frames() {
    let (mut s, shot) = shot_sim(false, "cpu_saved");
    let run = run_window(&mut s, &shot, 150, DELIVERY_LAG, |_, _| {});
    assert_saved_at_rest(&run);
    assert_eq!(run.requests.len(), 1);
    assert!(run.closed_at.is_some_and(|n| n <= 14), "closed at frame {:?}", run.closed_at);
    cleanup(&shot);
}

#[test]
fn a_view_that_never_settles_is_saved_before_the_window_closes() {
    let (mut s, shot) = shot_sim(true, "never_saved");
    let run = run_window(&mut s, &shot, 400, DELIVERY_LAG, |s, _| s.shell.app.view.offset.x += 1.0);
    let from = run.saved_from.unwrap_or_else(|| panic!("no image was saved: {run:?}"));
    assert!(!run.at_rest[from], "the view never rested, so the interactive frame is what was kept");
    assert!(run.closed_at.is_some_and(|n| n + 3 < 240), "closed at frame {:?}, the run itself gives up after 240", run.closed_at);
    assert_eq!(run.requests.len(), 1, "no retries once the settle limit has passed: {run:?}");
    cleanup(&shot);
}

/// What `App::poll_fonts` does when a background font load returns fonts: `changed()`, which
/// marks the document modified so the next paint bumps the revision and compiles it capped.
fn fonts_arrive(s: &mut Sim) {
    s.shell.app.changed();
}

const AGENT_EDIT_NAME: &str = "Edited by an agent";

fn first_node(s: &Sim) -> designos_doc::NodeId {
    let page = s.app().page;
    *s.app().doc.get(page).and_then(|p| p.children.first()).expect("the page has a node")
}

/// An agent edit that was held while the user was mid-edit: the frame applies it after the
/// screenshot step at the top of the window frame and before the canvas is painted.
fn agent_edit_arrives(s: &mut Sim) {
    let id = first_node(s);
    let set = serde_json::Map::from_iter([("name".to_string(), serde_json::json!(AGENT_EDIT_NAME))]);
    s.shell.app.terminal.live_patch.pending.push(LivePatch::new(vec![LiveOp::Update { id, set, unset: Vec::new() }]));
}

fn agent_edit_landed(s: &Sim) -> bool {
    s.app().doc.get(first_node(s)).is_some_and(|n| n.name == AGENT_EDIT_NAME)
}

/// Fires `disturb` before the frame that follows the first one whose paint was at rest.
fn right_after_the_refine(disturb: fn(&mut Sim)) -> impl FnMut(&mut Sim, &Run) {
    let mut fired = false;
    move |s, run| {
        if !fired && run.first_rest().is_some_and(|r| run.at_rest.len() > r) {
            fired = true;
            disturb(s);
        }
    }
}

/// Fires `disturb` before frame `frame` of the run.
fn before_frame(frame: usize, disturb: fn(&mut Sim)) -> impl FnMut(&mut Sim, &Run) {
    move |s, run| {
        if run.at_rest.len() == frame {
            disturb(s);
        }
    }
}

#[test]
fn fonts_arriving_in_the_frame_after_the_refine_do_not_cost_the_capture_its_refine() {
    // that frame is the first one allowed to capture, and fonts that finished during the slow
    // refine compile are registered by the poll at the start of exactly that frame
    for lag in 1..=3 {
        let (mut s, shot) = shot_sim(true, &format!("fonts_after_refine_{lag}"));
        let run = run_window(&mut s, &shot, 300, lag, right_after_the_refine(fonts_arrive));
        assert_saved_at_rest(&run);
        cleanup(&shot);
    }
}

#[test]
fn an_agent_edit_applied_in_the_frame_after_the_refine_does_not_cost_the_capture_its_refine() {
    for lag in 1..=3 {
        let (mut s, shot) = shot_sim(true, &format!("edit_after_refine_{lag}"));
        let run = run_window(&mut s, &shot, 300, lag, right_after_the_refine(agent_edit_arrives));
        assert!(agent_edit_landed(&s), "lag {lag}: the edit never landed, the test proves nothing: {run:?}");
        assert_saved_at_rest(&run);
        cleanup(&shot);
    }
}

#[test]
fn a_disturbed_capture_is_asked_for_again_and_only_the_at_rest_one_is_saved() {
    for lag in 1..=3 {
        let (mut s, shot) = shot_sim(true, &format!("asked_again_{lag}"));
        let run = run_window(&mut s, &shot, 300, lag, right_after_the_refine(fonts_arrive));
        assert_eq!(run.requests.len(), 2, "lag {lag}: one capture was taken from the capped frame and discarded: {run:?}");
        assert_eq!(run.saved_from, Some(run.requests[1] + 1), "lag {lag}: the kept image is the retry's: {run:?}");
        cleanup(&shot);
    }
}

#[test]
fn no_frame_of_the_settle_is_one_a_disturbance_can_turn_into_a_capped_capture() {
    for (name, disturb) in [("fonts", fonts_arrive as fn(&mut Sim)), ("agent edit", agent_edit_arrives)] {
        for frame in 0..34 {
            let (mut s, shot) = shot_sim(true, &format!("sweep_{}_{frame}", name.replace(' ', "_")));
            let run = run_window(&mut s, &shot, 300, DELIVERY_LAG, before_frame(frame, disturb));
            let from = run.saved_from.unwrap_or_else(|| panic!("{name} before frame {frame}: no image was saved: {run:?}"));
            assert!(run.at_rest[from], "{name} before frame {frame}: the saved image was captured from capped frame {from}: {run:?}");
            cleanup(&shot);
        }
    }
}
