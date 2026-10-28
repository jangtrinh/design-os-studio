//! Unit and `Sim` tests for `--screenshot` capture retry, failure reasons, viewport level, and watchdog (`#1141`, US-17).

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

// Test 1: Run frames until one `ViewportCommand::Screenshot` is sent, then inject no image for 12 frames.
// A second `Screenshot` command is sent.
#[test]
fn lost_screenshot_request_is_sent_again() {
    let (mut s, path) = showcase_shot_sim("t1_re_request");
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

// Test 2: After case 1, inject `Event::Screenshot` with a 1600x1088 `ColorImage`.
// The PNG exists, and `outcome == Written`.
#[test]
fn capture_is_written_after_a_lost_request() {
    let (mut s, path) = showcase_shot_sim("t2_written_after_lost");
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

// Test 3: Never inject. `outcome == Failed` and the reason starts with `no image after 5 requests`.
#[test]
fn capture_gives_up_with_a_reason_after_five_lost_requests() {
    let (mut s, path) = showcase_shot_sim("t3_five_lost");
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

// Test 4 + AC-04 (Controller Ruling 2): Pure `fn failure_line(&ShotOutcome) -> String` starts with
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

// Test 5: Pure image-to-buffer conversion given a mismatched size returns `Err` naming `<w>x<h>` and the byte count.
#[test]
fn malformed_image_is_reported() {
    let err = image_to_rgba(1600, 1088, vec![0u8; 12]).expect_err("mismatched byte count must fail");
    assert!(err.contains("1600x1088") && err.contains("12 bytes"), "expected malformed image error naming '1600x1088' and '12 bytes', got: {err:?}");
    assert_eq!(err, "the image was malformed (1600x1088, 12 bytes)");
}

// Test 6: Pure `fn screenshot_viewport(w, h) -> ViewportBuilder` (moved out of `main`):
// `window_level == Some(AlwaysOnTop)` and `active == Some(false)`.
#[test]
fn screenshot_viewport_is_always_on_top() {
    let vp = screenshot_viewport(1440, 900);
    assert_eq!(vp.window_level, Some(egui::WindowLevel::AlwaysOnTop));
    assert_eq!(vp.active, Some(false));
}

// Test 6b: Pure watchdog state: `arm` is never called, and 60 s pass in a fake clock: no fire.
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

// Round 2 (§1): The frame flag is set and 31 s elapse. The pure `check` returns `None`.
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

// Round 2 (§1): Re-arm the clock when the frame ends, so the 30 s count starts after the frame.
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

// Round 2 (§1): A fired check plus `SHOT_OK` returns no exit. Test through the pure decision function; never call `process::exit`.
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

// Round 2 (§2): After a write error, one more `drive_screenshot` pass requests no screenshot and prints nothing.
#[test]
fn write_failure_stops_further_capture_passes() {
    // 1. Single-PNG write failure (`shell.rs` PNG save arm):
    let (mut s, path) = showcase_shot_sim("r2_write_fail_png");
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
    let seq_dir = std::env::temp_dir().join(format!("designos_shot_capture_{}_r2_write_fail_seq", std::process::id()));
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

// Round 2 (§3): Pin the merged exit contract (`exit_code` together with the line chosen per case).
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
