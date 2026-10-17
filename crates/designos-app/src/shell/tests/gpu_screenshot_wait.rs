//! `--screenshot` with the GPU canvas waits for the at-rest frame (the one full-quality
//! refine compile) instead of capturing the capped interactive frame.
//! The canvas is compiled for real (compiling needs no GPU device); only the drawing is absent.

use super::*;
use crate::canvas::gpu::IDLE_REFINE_DELAY;
use egui::RawInput;

const SIZE: (u32, u32) = (1600, 1088);
/// Seconds of input time per frame, small enough that the idle delay spans many frames.
const DT: f64 = 0.01;

fn shot_sim(gpu: bool) -> Sim {
    let dir = std::env::temp_dir().join(format!("designos_gpu_screenshot_wait_{}", std::process::id()));
    let mut s = Sim::with(Options { screenshot: Some(dir.join("shot.png")), size: Some(SIZE), ..Default::default() });
    if gpu {
        let g = &mut s.shell.app.canvas.gpu;
        (g.available, g.pref) = (true, true);
        g.set_max_texture(16_384);
    }
    s
}

/// One window frame; true when it asked egui for the screenshot.
fn frame_requests_screenshot(s: &mut Sim) -> bool {
    s.t += DT;
    let raw = RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(SIZE.0 as f32, SIZE.1 as f32))), time: Some(s.t), ..Default::default() };
    let ctx = s.ctx.clone();
    let shell = &mut s.shell;
    let out = ctx.run_ui(raw, |ui| shell.draw(ui));
    out.viewport_output.values().any(|v| v.commands.iter().any(|c| matches!(c, egui::ViewportCommand::Screenshot(_))))
}

/// Frames run until the screenshot is requested, None past `limit` frames.
fn frames_until_screenshot(s: &mut Sim, limit: u32) -> Option<u32> {
    (1..=limit).find(|_| frame_requests_screenshot(s))
}

#[test]
fn gpu_screenshot_waits_for_the_refined_frame() {
    let mut s = shot_sim(true);
    let n = frames_until_screenshot(&mut s, 150).expect("a screenshot is requested");
    assert!(s.app().canvas.gpu.at_rest(), "the capture is requested once the refined frame is cached");
    let idle_frames = (IDLE_REFINE_DELAY / DT) as u32;
    assert!(n > idle_frames, "took {n} frames, the idle delay alone is {idle_frames}: the capped interactive frame was not captured");
}

#[test]
fn screenshot_of_a_canvas_not_on_the_gpu_is_taken_as_before() {
    let mut s = shot_sim(false);
    let n = frames_until_screenshot(&mut s, 150).expect("a screenshot is requested");
    assert!((1..=12).contains(&n), "after the usual dozen frames, took {n}");
}

#[test]
fn screenshot_gives_up_waiting_for_a_view_that_never_settles() {
    let mut s = shot_sim(true);
    // a view that moves every frame never goes idle
    let n = (1..=400)
        .find(|_| {
            s.shell.app.view.offset.x += 1.0;
            frame_requests_screenshot(&mut s)
        })
        .expect("the screenshot is eventually requested");
    assert!((100..240).contains(&n), "settle limit hit after {n} frames, before the 240 frame close");
    assert!(!s.app().canvas.gpu.at_rest(), "it captured the interactive frame, as announced");
}
