//! A toast's time starts when it is first painted, so a blocked frame before that paint does not spend it.

use super::*;

fn painted_text(s: &Sim, text: &str) -> bool {
    s.shapes.iter().any(|c| matches!(&c.shape, egui::Shape::Text(t) if t.galley.text() == text))
}

/// The next frame reads egui's clock (`app.now = i.time`), which a blocked UI thread has moved forward.
fn block_ui(s: &mut Sim, seconds: f64) {
    s.shell.app.now += seconds;
    s.t += seconds;
}

/// One frame whose egui clock lands on `at` (each frame adds the sim's 0.05 s step).
fn frame_at(s: &mut Sim, at: f64) {
    s.t = at - 0.05;
    s.frame(vec![]);
}

/// `show_toast` / `show_toast_with_reveal`, then 30 s with no frame: the next frame paints the toast,
/// it is still there `seconds - 0.1` after that paint, and gone `seconds + 0.1` after it.
fn toast_clock_runs_from_the_first_paint(raise: impl FnOnce(&mut Sim), seconds: f64) {
    let mut s = Sim::new();
    raise(&mut s);
    let text = s.app().toast.as_ref().expect("the toast is raised before the blocked frame").text.clone();
    block_ui(&mut s, 30.0);
    s.frame(vec![]);
    assert!(painted_text(&s, &text), "the frame after a 30 s block paints the toast, got {:?}", s.app().toast.as_ref().map(|t| &t.text));
    let started = s.app().now;
    frame_at(&mut s, started + seconds - 0.1);
    assert!(painted_text(&s, &text), "still painted {seconds} - 0.1 s after the first paint (now {})", s.app().now);
    frame_at(&mut s, started + seconds + 0.1);
    assert!(s.app().toast.is_none(), "gone {seconds} + 0.1 s after the first paint");
    assert!(!painted_text(&s, &text), "the expired toast is not painted");
}

#[test]
fn distinct_error_waits_for_the_error_on_screen_to_expire() {
    let mut s = Sim::new();
    s.shell.app.show_toast("First error", true);
    s.frame(vec![]);
    s.shell.app.show_toast("Second error", true);
    s.frame(vec![]);

    assert_eq!(s.app().toast.as_ref().map(|t| t.text.as_str()), Some("First error"), "the error on screen keeps its slot");
    assert_eq!(s.app().toast_queue.len(), 1, "the second error waits");
    assert!(painted_text(&s, "First error") && !painted_text(&s, "Second error"));

    let expires = s.app().now + crate::theme::TOAST_SECONDS + 0.1;
    frame_at(&mut s, expires);
    frame_at(&mut s, expires + 0.05);
    assert_eq!(s.app().toast.as_ref().map(|t| t.text.as_str()), Some("Second error"), "it appears once the first one is over");
    assert!(s.app().toast_queue.is_empty());
    assert!(painted_text(&s, "Second error"));
}

#[test]
fn error_after_a_plain_toast_still_takes_the_slot_at_once() {
    let mut s = Sim::new();
    s.shell.app.show_toast("Saved", false);
    s.frame(vec![]);
    s.shell.app.show_toast("Something failed", true);

    assert_eq!(s.app().toast.as_ref().map(|t| t.text.as_str()), Some("Something failed"));
    assert!(s.app().toast_queue.is_empty());
}

#[test]
fn toast_clock_reveal_toast_survives_a_30s_block_then_lasts_reveal_seconds() {
    toast_clock_runs_from_the_first_paint(
        |s| {
            let path = std::path::PathBuf::from("/tmp/designos-toast-clock-reveal.png");
            s.shell.app.show_toast_with_reveal(format!("Exported {}", path.display()), path);
        },
        crate::theme::TOAST_REVEAL_SECONDS,
    );
}

#[test]
fn toast_clock_plain_toast_survives_a_30s_block_then_lasts_toast_seconds() {
    toast_clock_runs_from_the_first_paint(|s| s.shell.app.show_toast("Saved the page", false), crate::theme::TOAST_SECONDS);
}
