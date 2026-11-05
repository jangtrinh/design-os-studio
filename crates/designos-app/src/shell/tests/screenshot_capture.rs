//! Unit and `Sim` tests for `--screenshot` capture retry, failure reasons, viewport level, and watchdog.

use super::*;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

const SIZE: (u32, u32) = (1600, 1088);

fn showcase_shot_sim(tag: &str) -> (Sim, PathBuf) {
    let dir = std::env::temp_dir().join(format!("designos_shot_capture_{}_{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let path = dir.join("s.png");
    let opts = Options {
        fixture: Some(crate::showcase::FIXTURE.to_string()),
        select: Some("none".to_string()),
        size: Some(SIZE),
        screenshot: Some(path.clone()),
        ..Default::default()
    };
    (Sim::with(opts), path)
}

fn cleanup(path: &std::path::Path) {
    if let Some(dir) = path.parent() {
        let _ = std::fs::remove_dir_all(dir);
    }
}

fn frame_sent_screenshot(s: &Sim) -> bool {
    s.commands.iter().any(|c| matches!(c, egui::ViewportCommand::Screenshot(_)))
}

fn run_until_first_screenshot_cmd(s: &mut Sim, limit: u32) -> u32 {
    if s.sent_commands.iter().any(|c| matches!(c, egui::ViewportCommand::Screenshot(_))) {
        return 0;
    }
    for step in 1..=limit {
        s.frame(vec![]);
        if frame_sent_screenshot(s) {
            return step;
        }
    }
    panic!("no ViewportCommand::Screenshot sent within {limit} frames");
}

fn screenshot_event(w: usize, h: usize) -> Event {
    let image = Arc::new(egui::ColorImage::filled([w, h], egui::Color32::from_rgb(0x18, 0x18, 0x1B)));
    Event::Screenshot { viewport_id: egui::ViewportId::ROOT, user_data: egui::UserData::default(), image }
}

fn frame_screenshot_user_data(cmds: &[egui::ViewportCommand]) -> Option<egui::UserData> {
    cmds.iter().find_map(|c| if let egui::ViewportCommand::Screenshot(u) = c { Some(u.clone()) } else { None })
}

fn tagged_screenshot_event(c: egui::Color32, user_data: egui::UserData) -> Event {
    let image = Arc::new(egui::ColorImage::filled([SIZE.0 as usize, SIZE.1 as usize], c));
    Event::Screenshot { viewport_id: egui::ViewportId::ROOT, user_data, image }
}

// Run frames until one `ViewportCommand::Screenshot` is sent, then inject no image for 12 frames.
// A second `Screenshot` command is sent.
#[test]
fn lost_screenshot_request_is_sent_again() {
    let (mut s, path) = showcase_shot_sim("re_request");
    run_until_first_screenshot_cmd(&mut s, 60);
    let mut second_sent = false;
    for _ in 0..12 {
        s.frame(vec![]);
        if frame_sent_screenshot(&s) {
            second_sent = true;
            break;
        }
    }
    cleanup(&path);
    assert!(second_sent, "expected a second ViewportCommand::Screenshot within 12 frames after a lost request");
}

// After a lost request is retried, inject `Event::Screenshot` with a 1600x1088 `ColorImage`.
// The PNG exists, and `outcome == Written`.
#[test]
fn capture_is_written_after_a_lost_request() {
    let (mut s, path) = showcase_shot_sim("written_after_lost");
    run_until_first_screenshot_cmd(&mut s, 60);
    let mut second_sent = false;
    for _ in 0..12 {
        s.frame(vec![]);
        if frame_sent_screenshot(&s) {
            second_sent = true;
        }
    }
    assert!(second_sent, "precondition: second ViewportCommand::Screenshot must be sent after first is lost");
    s.frame(vec![screenshot_event(SIZE.0 as usize, SIZE.1 as usize)]);
    let exists = path.is_file() && path.metadata().is_ok_and(|m| m.len() > 0);
    let outcome = s.shell.shot.as_ref().expect("shot state exists").outcome.clone();
    cleanup(&path);
    assert!(exists, "PNG must be written after re-requested screenshot arrives");
    assert_eq!(outcome, ShotOutcome::Written);
}

// Never inject an image. `outcome == Failed` and the reason starts with `no image after 5 requests`.
#[test]
fn capture_gives_up_with_a_reason_after_five_lost_requests() {
    let (mut s, path) = showcase_shot_sim("five_lost");
    for _ in 0..260 {
        s.frame(vec![]);
        if s.commands.contains(&egui::ViewportCommand::Close) {
            break;
        }
    }
    let exists = path.exists();
    let outcome = s.shell.shot.as_ref().expect("shot state exists").outcome.clone();
    cleanup(&path);
    assert!(!exists, "abandoned capture must never write a partial or empty file");
    match outcome {
        ShotOutcome::Failed(reason) => {
            assert!(reason.starts_with("no image after 5 requests"), "expected reason starting with 'no image after 5 requests', got: {reason:?}");
            assert!(
                reason.contains("frames ") && reason.contains("; last surface status "),
                "expected frame list and surface status in reason, got: {reason:?}"
            );
        }
        other => panic!("expected ShotOutcome::Failed, got {other:?}"),
    }
}

// Pure `fn failure_line(&ShotOutcome) -> String` starts with
// `designos: screenshot was not captured: `, and every failure path keeps the exact INFRA prefix and exit code 1.
#[test]
fn failure_line_keeps_the_infra_prefix() {
    let cases = [
        ShotOutcome::Failed("no image after 5 requests (frames 12, 22, 32, 42, 52); last surface status none".to_string()),
        ShotOutcome::Failed(SHOT_OCCLUDED_REASON.to_string()),
        ShotOutcome::Failed("the image was malformed (1600x1088, 12 bytes)".to_string()),
        ShotOutcome::Failed(SHOT_CLOSED_REASON.to_string()),
        ShotOutcome::Pending,
    ];
    for outcome in &cases {
        let line = failure_line(outcome);
        assert!(
            line.starts_with("designos: screenshot was not captured: "),
            "failure_line({outcome:?}) must start with 'designos: screenshot was not captured: ', got: {line:?}"
        );
        let reason_part = &line["designos: screenshot was not captured: ".len()..];
        assert!(!reason_part.trim().is_empty(), "failure_line({outcome:?}) must include a non-empty reason, got: {line:?}");
        assert_eq!(failure_exit_code(outcome), 1, "every failure path must exit with code 1");
    }
}

// Pure image-to-buffer conversion given a mismatched size returns `Err` naming `<w>x<h>` and the byte count.
#[test]
fn malformed_image_is_reported() {
    let err = image_to_rgba(1600, 1088, vec![0u8; 12]).expect_err("mismatched byte count must fail");
    assert!(err.contains("1600x1088") && err.contains("12 bytes"), "expected malformed image error naming '1600x1088' and '12 bytes', got: {err:?}");
    assert_eq!(err, "the image was malformed (1600x1088, 12 bytes)");
}

// Pure `fn screenshot_viewport(w, h) -> ViewportBuilder` (moved out of `main`):
// `window_level == Some(AlwaysOnTop)` and `active == Some(false)`.
#[test]
fn screenshot_viewport_is_always_on_top() {
    let vp = screenshot_viewport(1440, 900);
    assert_eq!(vp.window_level, Some(egui::WindowLevel::AlwaysOnTop));
    assert_eq!(vp.active, Some(false));
}

// Pure watchdog state: `arm` is never called, and 60 s pass in a fake clock: no fire.
// Armed, then no frame for 31 s: fire, with the occluded reason.
// Frames every 1 s for 60 s: no fire.
#[test]
fn watchdog_is_not_armed_before_the_first_frame() {
    let un_armed = ShotWatchdog::new();
    assert_eq!(un_armed.check(Duration::from_secs(60)), None, "unarmed watchdog must not fire after 60 s of file loading");

    let mut stalled = ShotWatchdog::new();
    stalled.arm(Duration::from_secs(0));
    assert_eq!(stalled.check(Duration::from_secs(29)), None, "armed watchdog must not fire before 30 s");
    assert_eq!(
        stalled.check(Duration::from_secs(31)),
        Some("the window was not drawn for 30 s (occluded or minimised)"),
        "armed watchdog must fire after 31 s with the occluded reason"
    );

    let mut active = ShotWatchdog::new();
    for sec in 0..=60 {
        let now = Duration::from_secs(sec);
        active.arm(now);
        assert_eq!(active.check(now), None, "watchdog receiving frames every 1 s must never fire (at {sec} s)");
    }
}

// Edge case: An image arrives after it was counted lost (and a re-request was sent):
// the first complete image wins, and a second image arriving afterwards is ignored.
#[test]
fn late_image_after_lost_request_wins_and_second_image_is_ignored() {
    let (mut s, path) = showcase_shot_sim("edge_first_wins");
    run_until_first_screenshot_cmd(&mut s, 60);
    for _ in 0..12 {
        s.frame(vec![]);
    }
    let first_img = Arc::new(egui::ColorImage::filled([SIZE.0 as usize, SIZE.1 as usize], egui::Color32::from_rgb(0xE3, 0x63, 0x23)));
    let second_img = Arc::new(egui::ColorImage::filled([SIZE.0 as usize, SIZE.1 as usize], egui::Color32::from_rgb(0x35, 0x82, 0xFF)));
    s.frame(vec![
        Event::Screenshot { viewport_id: egui::ViewportId::ROOT, user_data: egui::UserData::default(), image: first_img },
        Event::Screenshot { viewport_id: egui::ViewportId::ROOT, user_data: egui::UserData::default(), image: second_img.clone() },
    ]);
    s.frame(vec![Event::Screenshot { viewport_id: egui::ViewportId::ROOT, user_data: egui::UserData::default(), image: second_img }]);
    let written = image::open(&path).expect("saved PNG must be readable").into_rgba8();
    let px = written.get_pixel(0, 0).0;
    cleanup(&path);
    assert_eq!(px, [0xE3, 0x63, 0x23, 0xFF], "first complete image must win and later images must be ignored");
}

// The frame flag is set and 31 s elapse. The pure `check` returns `None`.
#[test]
fn watchdog_does_not_fire_while_a_frame_is_running() {
    let mut w = ShotWatchdog::new();
    w.arm(Duration::from_secs(0));
    w.begin_frame();
    assert_eq!(
        w.check(Duration::from_secs(31)),
        None,
        "watchdog must not fire while a frame is running (SHOT_IN_FRAME={})",
        SHOT_IN_FRAME.load(Ordering::SeqCst)
    );
}

// Re-arm the clock when the frame ends, so the 30 s count starts after the frame.
#[test]
fn watchdog_fires_after_30s_without_frames_once_the_frame_ends() {
    let mut w = ShotWatchdog::new();
    w.arm(Duration::from_secs(0));
    w.begin_frame();
    assert_eq!(w.check(Duration::from_secs(35)), None, "watchdog must not fire during a 35 s frame");
    w.end_frame(Duration::from_secs(35));
    assert_eq!(w.check(Duration::from_secs(64)), None, "watchdog must not fire 29 s after the frame ended");
    assert_eq!(w.check(Duration::from_secs(65)), Some(SHOT_OCCLUDED_REASON), "watchdog must fire 30 s after the frame ended without a new frame");
    assert_eq!(w.check(Duration::from_secs(66)), Some(SHOT_OCCLUDED_REASON), "watchdog must still report occluded 31 s after the frame ended");
}

// A fired check plus `SHOT_OK` returns no exit. Test through the pure decision function; never call `process::exit`.
#[test]
fn watchdog_does_not_exit_after_the_png_was_written() {
    let mut w = ShotWatchdog::new();
    w.arm(Duration::from_secs(0));
    let fired = w.check(Duration::from_secs(31));
    assert_eq!(fired, Some(SHOT_OCCLUDED_REASON), "precondition: check has fired");
    assert_eq!(watchdog_exit_outcome(fired, false, true, false), None, "a fired check plus SHOT_OK must return no exit");
    assert_eq!(watchdog_exit_outcome(fired, false, false, true), None, "a fired check plus SHOT_WRITE_FAILED must return no exit");
    assert_eq!(watchdog_exit_outcome(fired, true, false, false), None, "a fired check plus done must return no exit");
    assert_eq!(
        watchdog_exit_outcome(fired, false, false, false),
        Some(ShotOutcome::Failed(SHOT_OCCLUDED_REASON.to_string())),
        "a fired check with no completion flag must return the Failed outcome"
    );
}

// After a write error, one more `drive_screenshot` pass requests no screenshot and prints nothing.
#[test]
fn write_failure_stops_further_capture_passes() {
    // 1. Single-PNG write failure (`shell.rs` PNG save arm):
    let (mut s, path) = showcase_shot_sim("write_fail_png");
    // Make `path` an existing directory so `buf.save(&shot.path)` fails with an OS write error.
    std::fs::create_dir_all(&path).expect("create directory at PNG path to force write failure");
    run_until_first_screenshot_cmd(&mut s, 60);
    s.frame(vec![screenshot_event(SIZE.0 as usize, SIZE.1 as usize)]);
    let shot_after_err = s.shell.shot.as_ref().expect("shot state exists");
    match &shot_after_err.outcome {
        ShotOutcome::Failed(reason) => assert!(reason.starts_with("cannot write "), "expected cannot write reason, got: {reason:?}"),
        other => panic!("expected terminal ShotOutcome::Failed after PNG write error, got: {other:?}"),
    }
    let frames_at_err = shot_after_err.frames;
    let requests_at_err = shot_after_err.requests;
    let images_at_err = shot_after_err.images_seen;

    // One more pass with an injected screenshot event plus 15 empty frames: no new screenshot request, no image processed.
    s.frame(vec![screenshot_event(SIZE.0 as usize, SIZE.1 as usize)]);
    assert!(!frame_sent_screenshot(&s), "write failure must not request another screenshot on the next pass");
    for _ in 0..15 {
        s.frame(vec![]);
        assert!(!frame_sent_screenshot(&s), "write failure must never re-request a screenshot on later frames");
    }
    let shot_later = s.shell.shot.as_ref().expect("shot state exists");
    assert_eq!(shot_later.frames, frames_at_err, "drive_screenshot must return early after write failure");
    assert_eq!(shot_later.requests, requests_at_err, "no additional screenshot request may be recorded");
    assert_eq!(shot_later.images_seen, images_at_err, "later screenshot events must be ignored without printing a second error");
    cleanup(&path);

    // 2. Sequence-frame write failure (`shell.rs` sequence frame save arm):
    let seq_dir = std::env::temp_dir().join(format!("designos_shot_capture_{}_write_fail_seq", std::process::id()));
    let _ = std::fs::remove_dir_all(&seq_dir);
    std::fs::create_dir_all(seq_dir.join("frame-0000.jpg")).expect("create directory at frame-0000.jpg to force sequence write failure");
    let seq_opts = Options {
        fixture: Some(crate::showcase::FIXTURE.to_string()),
        panel: Some("micro-scale-tool:seq:2".to_string()),
        size: Some(SIZE),
        screenshot: Some(seq_dir.clone()),
        ..Default::default()
    };
    let mut seq_sim = Sim::with(seq_opts);
    run_until_first_screenshot_cmd(&mut seq_sim, 60);
    seq_sim.frame(vec![screenshot_event(SIZE.0 as usize, SIZE.1 as usize)]);
    let seq_shot = seq_sim.shell.shot.as_ref().expect("seq shot state exists");
    match &seq_shot.outcome {
        ShotOutcome::Failed(reason) => assert!(reason.starts_with("cannot write "), "expected cannot write reason, got: {reason:?}"),
        other => panic!("expected terminal ShotOutcome::Failed after sequence write error, got: {other:?}"),
    }
    let seq_frames_at_err = seq_shot.frames;
    let seq_images_at_err = seq_shot.images_seen;
    seq_sim.frame(vec![screenshot_event(SIZE.0 as usize, SIZE.1 as usize)]);
    assert!(!frame_sent_screenshot(&seq_sim), "sequence write failure must not request another screenshot");
    let seq_shot_later = seq_sim.shell.shot.as_ref().expect("seq shot state exists");
    assert_eq!(seq_shot_later.frames, seq_frames_at_err, "drive_screenshot must return early after sequence write failure");
    assert_eq!(seq_shot_later.images_seen, seq_images_at_err, "later sequence screenshot events must be ignored");
    let _ = std::fs::remove_dir_all(&seq_dir);
    SHOT_WRITE_FAILED.store(false, Ordering::SeqCst);
}

// Pin the merged exit contract (`exit_code` together with the line chosen per case).
#[test]
fn screenshot_exit_contract() {
    // 1. `--select` miss: exits 2 and prints only its own message.
    let miss_msg = "--select \"missing\" matched no node on page \"Page 1\"";
    assert_eq!(crate::exit_code(true, false), 2);
    assert_eq!(crate::screenshot_exit_line(Some(miss_msg), false, false, &ShotOutcome::Pending), Some(format!("designos: {miss_msg}")));

    // 2. Write failure: exits 1, with no `failure_line`.
    let write_err = ShotOutcome::Failed("cannot write /tmp/out.png: Permission denied".to_string());
    assert_eq!(crate::exit_code(false, false), 1);
    assert_eq!(crate::screenshot_exit_line(None, false, true, &write_err), None);

    // 3. Not captured: exits 1 with `designos: screenshot was not captured: <reason>`.
    let reason = "no image after 5 requests (frames 12, 22, 32, 42, 52); last surface status Occluded";
    let not_captured = ShotOutcome::Failed(reason.to_string());
    assert_eq!(crate::exit_code(false, false), 1);
    assert_eq!(crate::screenshot_exit_line(None, false, false, &not_captured), Some(format!("designos: screenshot was not captured: {reason}")));

    // 4. Written: exits 0, nothing printed.
    assert_eq!(crate::exit_code(false, true), 0);
    assert_eq!(crate::screenshot_exit_line(None, true, false, &ShotOutcome::Written), None);
}

#[test]
fn sequence_drops_late_image_of_superseded_request_and_captures_every_state() {
    let dir = std::env::temp_dir().join(format!("designos_shot_capture_{}_seq_tagged_late", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create sequence temp dir");
    let opts = Options {
        fixture: Some(crate::showcase::FIXTURE.to_string()),
        panel: Some("micro-scale-tool:seq:3".to_string()),
        size: Some(SIZE),
        screenshot: Some(dir.clone()),
        ..Default::default()
    };
    let mut s = Sim::with(opts);
    let mut n = 0;
    while frame_screenshot_user_data(&s.commands).is_none() && n < 80 {
        s.frame(vec![]);
        n += 1;
    }
    let ud1 = frame_screenshot_user_data(&s.commands).expect("first sequence request for state 0");
    let mut ud2 = None;
    for _ in 1..=12 {
        s.frame(vec![]);
        if let Some(u) = frame_screenshot_user_data(&s.commands) {
            ud2 = Some(u);
            break;
        }
    }
    let ud2 = ud2.expect("re-request for state 0 after 10 frames");

    // Late image from request 1 arrives and captures state 0 (`frame-0000.jpg`).
    s.frame(vec![tagged_screenshot_event(egui::Color32::from_rgb(255, 0, 0), ud1)]);
    assert!(dir.join("frame-0000.jpg").is_file(), "frame-0000.jpg must be written from state 0");

    // Late image from superseded request 2 (also captured during state 0) arrives on the next frame:
    // it must be dropped and counted, and never saved as `frame-0001.jpg`.
    s.frame(vec![tagged_screenshot_event(egui::Color32::from_rgb(0, 0, 255), ud2)]);
    assert!(!dir.join("frame-0001.jpg").is_file(), "superseded request from state 0 must never be written as frame-0001.jpg");
    assert_eq!(s.shell.shot.as_ref().expect("shot exists").images_dropped, 1, "superseded request image must be counted as dropped");

    // Advance through the remaining sequence states (state 1 and state 2), answering each new request with its own color.
    let state_colors = [egui::Color32::from_rgb(0, 255, 0), egui::Color32::from_rgb(255, 255, 0)];
    let mut later_requests = 0usize;
    for _ in 0..80 {
        s.frame(vec![]);
        if let Some(u) = frame_screenshot_user_data(&s.commands) {
            let color = state_colors[later_requests.min(state_colors.len() - 1)];
            later_requests += 1;
            s.frame(vec![tagged_screenshot_event(color, u)]);
        }
        if s.shell.shot.as_ref().expect("shot exists").outcome != ShotOutcome::Pending {
            break;
        }
    }
    assert_eq!(later_requests, 2, "states 1 and 2 must each send their own screenshot request");
    assert_eq!(s.shell.shot.as_ref().expect("shot exists").outcome, ShotOutcome::Written);
    for idx in 0..3 {
        assert!(dir.join(format!("frame-{idx:04}.jpg")).is_file(), "frame-{idx:04}.jpg must exist");
    }
    assert!(!dir.join("frame-0003.jpg").is_file(), "seq:3 must write exactly 3 frames");
    let px0 = image::open(dir.join("frame-0000.jpg")).expect("open frame 0").into_rgb8().get_pixel(10, 10).0;
    let px1 = image::open(dir.join("frame-0001.jpg")).expect("open frame 1").into_rgb8().get_pixel(10, 10).0;
    let px2 = image::open(dir.join("frame-0002.jpg")).expect("open frame 2").into_rgb8().get_pixel(10, 10).0;
    let _ = std::fs::remove_dir_all(&dir);
    assert!(px0[0] > 200 && px0[1] < 30 && px0[2] < 30, "frame-0000.jpg must come from state 0 (red), got {px0:?}");
    assert!(px1[1] > 200 && px1[0] < 30 && px1[2] < 30, "frame-0001.jpg must come from state 1 (green), got {px1:?}");
    assert!(px2[0] > 200 && px2[1] > 200 && px2[2] < 30, "frame-0002.jpg must come from state 2 (yellow), got {px2:?}");
}

#[test]
fn gpu_late_image_from_capped_frame_is_never_saved_as_at_rest_capture() {
    const DT: f64 = 0.01;
    let dir = std::env::temp_dir().join(format!("designos_shot_capture_{}_gpu_tagged_late", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create gpu temp dir");
    let path = dir.join("shot.png");
    let mut s = Sim::with(Options { screenshot: Some(path.clone()), size: Some(SIZE), ..Default::default() });
    let g = &mut s.shell.app.canvas.gpu;
    (g.available, g.pref) = (true, true);
    g.set_max_texture(16_384);
    g.frame_work_override = Some(0.0);

    let gframe = |s: &mut Sim, events: Vec<Event>| -> (Option<egui::UserData>, bool) {
        s.t += DT;
        let ctx = s.ctx.clone();
        let shell = &mut s.shell;
        let raw = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(SIZE.0 as f32, SIZE.1 as f32))),
            time: Some(s.t),
            events,
            ..Default::default()
        };
        let out = ctx.run_ui(raw, |ui| shell.draw(ui));
        shell.app.canvas.gpu.ack_prepared_for_test();
        let cmds: Vec<_> = out.viewport_output.values().flat_map(|v| v.commands.iter().cloned()).collect();
        (frame_screenshot_user_data(&cmds), s.shell.app.canvas.gpu.at_rest())
    };

    let mut req_a = None;
    for _ in 1..=200 {
        if let (Some(u), _) = gframe(&mut s, vec![]) {
            req_a = Some(u);
            break;
        }
    }
    let req_a = req_a.expect("request A sent at rest");

    // Disturb the frame that captures request A (as `poll_fonts` does), so A's capture is painted capped.
    s.shell.app.changed();
    let (_, rest_after_disturb) = gframe(&mut s, vec![]);
    assert!(!rest_after_disturb, "disturbed frame must compile capped (!at_rest)");

    // Let request A time out and wait for request B to be sent once the GPU view settles back to at_rest.
    let mut req_b = None;
    for _ in 2..=300 {
        if let (Some(u), _) = gframe(&mut s, vec![]) {
            req_b = Some(u);
            break;
        }
    }
    let req_b = req_b.expect("request B sent after GPU view settles again");

    // Even if `shot.last_try` were flipped on a later request, request A recorded its own `last_try = false`.
    s.shell.shot.as_mut().expect("shot exists").last_try = true;

    // Request A's late image (from the capped frame) arrives after B is already in flight:
    // it must be dropped and counted, and never saved as the at-rest capture.
    let (asked_again, _) = gframe(&mut s, vec![tagged_screenshot_event(egui::Color32::from_rgb(200, 10, 10), req_a)]);
    assert!(!path.is_file(), "late capped image from request A must never be saved as the at-rest capture");
    assert_eq!(s.shell.shot.as_ref().expect("shot exists").outcome, ShotOutcome::Pending);
    assert_eq!(s.shell.shot.as_ref().expect("shot exists").images_dropped, 1, "capped request A image must be counted as dropped");
    assert!(asked_again.is_none(), "no duplicate request should be sent while request B is already in flight");

    // When request B's at-rest image arrives, it is saved.
    gframe(&mut s, vec![tagged_screenshot_event(egui::Color32::from_rgb(10, 210, 20), req_b)]);
    assert!(path.is_file(), "at-rest image from request B must be saved");
    assert_eq!(s.shell.shot.as_ref().expect("shot exists").outcome, ShotOutcome::Written);
    let px = image::open(&path).expect("open saved PNG").into_rgba8().get_pixel(0, 0).0;
    cleanup(&path);
    assert_eq!(px, [10, 210, 20, 255], "saved PNG must come from request B, not capped request A");
}

#[test]
fn last_surface_status_persists_across_drawn_frames_and_clears_on_new_request() {
    let (mut s, path) = showcase_shot_sim("surface_status_reset");
    // An early surface status during initial resize (before any request is sent) is ignored.
    record_surface_status("Outdated");
    run_until_first_screenshot_cmd(&mut s, 60);
    assert_eq!(s.shell.shot.as_ref().expect("shot exists").last_surface_status, None);

    // A surface status seen since the request was sent persists across later frames that draw,
    // and is cleared only when the next request is sent.
    record_surface_status("Outdated");
    s.frame(vec![]);
    assert_eq!(s.shell.shot.as_ref().expect("shot exists").last_surface_status.as_deref(), Some("Outdated"));
    s.frame(vec![]);
    assert_eq!(
        s.shell.shot.as_ref().expect("shot exists").last_surface_status.as_deref(),
        Some("Outdated"),
        "a frame that draws must not clear last_surface_status before a new request is sent"
    );
    let mut next_sent = false;
    for _ in 0..12 {
        s.frame(vec![]);
        if frame_sent_screenshot(&s) {
            next_sent = true;
            break;
        }
    }
    assert!(next_sent, "expected the next screenshot request to be sent");
    assert_eq!(s.shell.shot.as_ref().expect("shot exists").last_surface_status, None, "sending a new request must clear last_surface_status");
    cleanup(&path);

    // A status of `Timeout` during every attempt (with the remaining frames of each attempt drawing cleanly)
    // gives a failure reason that ends with "last surface status Timeout".
    let (mut s2, path2) = showcase_shot_sim("surface_status_timeout");
    let mut just_sent = frame_sent_screenshot(&s2);
    for _ in 0..260 {
        if just_sent {
            record_surface_status("Timeout");
        }
        s2.frame(vec![]);
        just_sent = frame_sent_screenshot(&s2);
        if s2.commands.contains(&egui::ViewportCommand::Close) {
            break;
        }
    }
    let outcome = s2.shell.shot.as_ref().expect("shot exists").outcome.clone();
    cleanup(&path2);
    match outcome {
        ShotOutcome::Failed(reason) => {
            assert!(reason.ends_with("last surface status Timeout"), "expected reason ending with 'last surface status Timeout', got: {reason:?}");
        }
        other => panic!("expected ShotOutcome::Failed, got {other:?}"),
    }
}

#[test]
fn draw_arms_watchdog_through_frame_guard_when_open_succeeded() {
    let (mut s, path) = showcase_shot_sim("frame_guard_arms");
    if let Ok(mut w) = SHOT_WATCHDOG.lock() {
        *w = ShotWatchdog::new();
    }
    s.frame(vec![]);
    let armed = SHOT_WATCHDOG.lock().expect("lock watchdog").last_frame_at.is_some();
    cleanup(&path);
    assert!(armed, "Shell::draw must enter ShotFrameGuard and arm SHOT_WATCHDOG on frame completion");
}
