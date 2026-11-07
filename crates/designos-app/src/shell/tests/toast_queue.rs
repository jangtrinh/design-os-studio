//! A toast with an action button is never silently replaced, and a reveal that fails says which folder.

use super::*;
use std::path::{Path, PathBuf};

fn painted_text(s: &Sim, text: &str) -> bool {
    s.shapes.iter().any(|c| matches!(&c.shape, egui::Shape::Text(t) if t.galley.text() == text))
}

/// One frame whose egui clock lands on `at` (each frame adds the sim's 0.05 s step).
fn frame_at(s: &mut Sim, at: f64) {
    s.t = at - 0.05;
    s.frame(vec![]);
}

fn reveal_path(name: &str) -> PathBuf {
    PathBuf::from(format!("/tmp/designos-toast-queue/{name}"))
}

fn no_file_manager(_: &Path) -> Result<(), String> {
    Err("no file manager".to_string())
}

/// Raises an export toast and paints it, so its clock is running.
fn export_toast_on_screen(s: &mut Sim, name: &str) -> String {
    let path = reveal_path(name);
    let text = format!("Exported {}", path.display());
    s.shell.app.show_toast_with_reveal(text.clone(), path);
    s.frame(vec![]);
    s.frame(vec![]);
    text
}

#[test]
fn a_plain_toast_waits_behind_a_live_export_toast_and_follows_it() {
    let mut s = Sim::new();
    let first = export_toast_on_screen(&mut s, "a.png");
    s.shell.app.show_toast("Saved the page", false);
    assert_eq!(s.app().toast.as_ref().unwrap().text, first, "the export toast stays");
    s.frame(vec![]);
    assert!(painted_text(&s, &first) && s.rect_of("toast-reveal").width() > 0.0, "Show in folder is still there");

    let started = s.app().now;
    frame_at(&mut s, started + crate::theme::TOAST_REVEAL_SECONDS + 0.1);
    assert_eq!(s.app().toast.as_ref().map(|t| t.text.as_str()), Some("Saved the page"), "the queued toast follows once the export toast ran out");
}

#[test]
fn a_second_export_toast_queues_behind_the_first_instead_of_replacing_it() {
    let mut s = Sim::new();
    let first = export_toast_on_screen(&mut s, "one.png");
    let second_path = reveal_path("two.png");
    s.shell.app.show_toast_with_reveal(format!("Exported {}", second_path.display()), second_path.clone());

    let on_screen = s.app().toast.as_ref().unwrap();
    assert_eq!((on_screen.text.as_str(), on_screen.reveal.as_deref()), (first.as_str(), Some(reveal_path("one.png").as_path())));
    assert_eq!(s.app().toast_queue.len(), 1, "the newer export waits its turn");

    let started = s.app().now;
    frame_at(&mut s, started + crate::theme::TOAST_REVEAL_SECONDS + 0.1);
    let now_showing = s.app().toast.as_ref().expect("the second export toast is next");
    assert_eq!(now_showing.reveal.as_deref(), Some(second_path.as_path()), "its Show in folder points at its own file");
    assert!(s.app().toast_queue.is_empty());
    // The queued toast gets its full time from its own first paint.
    let shown_at = s.app().now;
    frame_at(&mut s, shown_at + crate::theme::TOAST_REVEAL_SECONDS - 0.1);
    assert!(s.app().toast.is_some(), "the second export toast is still up just before its time");
}

#[test]
fn a_toast_that_has_not_been_painted_yet_is_not_replaced_either() {
    let mut s = Sim::new();
    let path = reveal_path("unpainted.png");
    s.shell.app.show_toast_with_reveal("Exported unpainted", path.clone());
    s.shell.app.now += 60.0;
    s.shell.app.show_toast("Saved the page", false);
    assert_eq!(s.app().toast.as_ref().unwrap().reveal.as_deref(), Some(path.as_path()));
    assert_eq!(s.app().toast_queue.len(), 1);
}

#[test]
fn an_expired_export_toast_is_replaced_at_once() {
    let mut s = Sim::new();
    export_toast_on_screen(&mut s, "old.png");
    s.shell.app.now += crate::theme::TOAST_REVEAL_SECONDS + 1.0;
    s.shell.app.show_toast("Saved the page", false);
    assert_eq!(s.app().toast.as_ref().unwrap().text, "Saved the page");
    assert!(s.app().toast_queue.is_empty());
}

#[test]
fn an_enter_a_path_toast_is_kept_and_its_queued_successor_keeps_its_own_action() {
    let mut s = Sim::new();
    s.shell.app.show_no_file_dialog_toast(true);
    assert!(s.app().toast.as_ref().unwrap().enter_path);
    // The same notice again adds nothing; an export toast with a button is raised and waits.
    s.shell.app.show_no_file_dialog_toast(true);
    assert!(s.app().toast_queue.is_empty(), "an identical error is not added twice");
    s.shell.app.show_toast_with_reveal("Exported b", reveal_path("b.png"));
    assert_eq!(s.app().toast_queue.len(), 1, "the export toast waits");
    assert!(!s.app().toast_queue[0].enter_path && s.app().toast.as_ref().unwrap().enter_path, "each toast keeps its own action");
    s.shell.app.show_toast("Saved the page", false);
    assert_eq!(s.app().toast_queue.len(), 2);
    assert!(!s.app().toast_queue[1].enter_path);
}

#[test]
fn a_failed_reveal_shows_an_error_toast_that_names_the_folder() {
    let mut s = Sim::new();
    s.shell.app.reveal_file = no_file_manager;
    let path = reveal_path("moved/design.png");
    s.shell.app.show_toast_with_reveal(format!("Exported {}", path.display()), path);
    s.frame(vec![]);
    s.frame(vec![]);
    let button = s.rect_of("toast-reveal");
    s.click(button.center());

    let toast = s.app().toast.as_ref().expect("the failure is shown");
    assert!(toast.error && toast.reveal.is_none(), "an error toast with no second action: {}", toast.text);
    assert_eq!(toast.text, "Couldn't open /tmp/designos-toast-queue/moved: no file manager");
}

#[test]
fn a_status_toast_takes_the_slot_and_the_export_toast_comes_back_after_it() {
    let mut s = Sim::new();
    let path = reveal_path("status.png");
    export_toast_on_screen(&mut s, "status.png");
    s.shell.app.run_after_status_paint("Saving\u{2026} loading all pages", |_| {});
    assert_eq!(s.app().toast.as_ref().unwrap().text, "Saving\u{2026} loading all pages", "the status is what is on screen");
    assert_eq!(s.app().toast_queue.len(), 1, "the export toast waits at the front");
    assert_eq!(s.app().toast_queue[0].reveal.as_deref(), Some(path.as_path()));

    s.frame(vec![]);
    let status_shown_at = s.app().now;
    frame_at(&mut s, status_shown_at + crate::theme::TOAST_SECONDS + 0.1);
    let back = s.app().toast.as_ref().expect("the export toast is back");
    assert_eq!(back.reveal.as_deref(), Some(path.as_path()));
    let back_at = s.app().now;
    frame_at(&mut s, back_at + crate::theme::TOAST_REVEAL_SECONDS - 0.5);
    assert!(s.app().toast.as_ref().is_some_and(|t| t.reveal.is_some()), "it gets its full time again");
}

#[test]
fn a_comments_warning_export_toast_does_not_flag_the_toast_already_on_screen() {
    let mut s = Sim::new();
    let first = export_toast_on_screen(&mut s, "first.fig");
    s.shell.app.show_fig_export_result(Ok(crate::fig_export::Exported { path: reveal_path("second.fig"), omitted_keys: 0, omitted_comment_threads: 2 }));
    let on_screen = s.app().toast.as_ref().unwrap();
    assert_eq!(on_screen.text, first);
    assert!(!on_screen.error, "the toast on screen keeps its own state");
    assert!(s.app().toast_queue[0].error, "the warning belongs to the queued toast");
}

#[test]
fn a_late_failure_for_an_old_export_leaves_the_newer_export_toast_alone() {
    let mut s = Sim::new();
    export_toast_on_screen(&mut s, "a.png");
    let b_path = reveal_path("b.png");
    s.shell.app.show_toast_with_reveal("Exported b", b_path.clone());
    let started = s.app().now;
    frame_at(&mut s, started + crate::theme::TOAST_REVEAL_SECONDS + 0.1);
    assert_eq!(s.app().toast.as_ref().unwrap().reveal.as_deref(), Some(b_path.as_path()), "precondition: B is on screen");

    // A's file manager gave up after A's toast was gone.
    let a_path = reveal_path("a.png");
    crate::fig_export::push_late_reveal_failure(a_path.clone(), crate::fig_export::reveal_failure_text(&a_path, ""));
    s.frame(vec![]);

    assert_eq!(s.app().toast.as_ref().unwrap().reveal.as_deref(), Some(b_path.as_path()), "B keeps its Show in folder");
    let queued = &s.app().toast_queue;
    assert_eq!(queued.len(), 1);
    assert!(queued[0].error && queued[0].text == "Couldn't open /tmp/designos-toast-queue", "the error waits and names A's folder: {}", queued[0].text);
}

#[test]
fn a_late_failure_replaces_the_export_toast_it_belongs_to() {
    let mut s = Sim::new();
    export_toast_on_screen(&mut s, "a.png");
    let a_path = reveal_path("a.png");
    crate::fig_export::push_late_reveal_failure(a_path.clone(), crate::fig_export::reveal_failure_text(&a_path, ""));
    s.frame(vec![]);
    let toast = s.app().toast.as_ref().expect("the error is on screen");
    assert!(toast.error && toast.reveal.is_none() && toast.text.starts_with("Couldn't open "), "{}", toast.text);
}

#[test]
fn a_stream_of_plain_toasts_does_not_hide_a_queued_export_toast() {
    let mut s = Sim::new();
    export_toast_on_screen(&mut s, "a.png");
    s.shell.app.show_toast("First plain", false);
    let b_path = reveal_path("b.png");
    s.shell.app.show_toast_with_reveal("Exported b", b_path.clone());
    let started = s.app().now;
    frame_at(&mut s, started + crate::theme::TOAST_REVEAL_SECONDS + 0.1);
    assert_eq!(s.app().toast.as_ref().unwrap().text, "First plain", "precondition: the plain toast popped ahead of B");

    // A plain toast every 3 s must neither replace the one on screen nor restart its clock.
    let plain_at = s.app().now;
    let mut b_seen_after = None;
    for step in 1..=30 {
        let t = plain_at + f64::from(step);
        if step % 3 == 0 {
            s.shell.app.show_toast(format!("Plain {step}"), false);
        }
        frame_at(&mut s, t);
        if s.app().toast.as_ref().is_some_and(|x| x.reveal.as_deref() == Some(b_path.as_path())) {
            b_seen_after = Some(t - plain_at);
            break;
        }
    }
    let waited = b_seen_after.expect("the export toast came back");
    assert!(waited <= crate::theme::TOAST_SECONDS + 1.1, "B waited {waited} s behind plain toasts, one plain toast lasts {} s", crate::theme::TOAST_SECONDS);
}

#[test]
fn relayout_notices_behind_an_export_toast_keep_only_the_latest_and_never_an_error() {
    let mut s = Sim::new();
    export_toast_on_screen(&mut s, "a.png");
    let prefix = designos_doc::RELAYOUT_NOTICE_PREFIX;
    for i in 0..200 {
        s.shell.app.show_toast(format!("{prefix}: {i} subtrees skipped past budget"), false);
        if i == 100 {
            s.shell.app.show_toast("Undo failed", true);
        }
    }
    let queue = &s.app().toast_queue;
    assert!(queue.len() <= 3, "queue holds {} toasts", queue.len());
    assert_eq!(queue.iter().filter(|t| !t.error).count(), 1, "one plain notice");
    assert!(queue.iter().any(|t| t.error && t.text == "Undo failed"), "the error is never merged away");
    assert!(queue.iter().any(|t| t.text.ends_with("199 subtrees skipped past budget")), "the latest notice is the one kept");

    // The relayout finishes: nothing is pending, the notice is retired from the queue too.
    s.shell.app.relayout_moved_geometry();
    assert!(!s.app().toast_queue.iter().any(|t| t.text.starts_with(prefix)), "no stale relayout notice is left");
    assert!(s.app().toast_queue.iter().any(|t| t.error), "the error stays");
}

#[test]
fn plain_notices_merge_and_error_toasts_are_never_dropped_from_the_queue() {
    let mut s = Sim::new();
    export_toast_on_screen(&mut s, "a.png");
    for i in 0..30 {
        s.shell.app.show_toast(format!("Plain {i}"), false);
        s.shell.app.show_toast(format!("Failed {i}"), true);
    }
    let queue = &s.app().toast_queue;
    assert_eq!(queue.iter().filter(|t| t.error).count(), 30, "no error toast is dropped");
    assert!(queue.iter().filter(|t| !t.error).count() <= 1, "plain notices merge to the latest, or give way once the queue is full");
}

#[test]
fn an_import_error_raised_behind_an_export_toast_is_still_counted() {
    let mut s = Sim::new();
    export_toast_on_screen(&mut s, "a.png");
    let before = s.app().error_toast_seq;
    s.shell.app.import_comments_file(Path::new("/nonexistent/designos-comments.json"));
    assert_eq!(s.app().error_toast_seq, before + 1, "the failed import is counted even though its toast waits");
    assert!(s.app().last_error_toast.starts_with("Cannot import comments"), "{}", s.app().last_error_toast);
    assert!(!s.app().toast.as_ref().unwrap().error, "the toast slot still shows the export toast");
}

#[test]
fn a_failed_click_with_a_newer_export_queued_shows_the_error_at_once() {
    let mut s = Sim::new();
    s.shell.app.reveal_file = no_file_manager;
    export_toast_on_screen(&mut s, "a.png");
    s.shell.app.show_toast_with_reveal("Exported b", reveal_path("b.png"));
    let button = s.rect_of("toast-reveal");
    s.click(button.center());
    s.frame(vec![]);
    let toast = s.app().toast.as_ref().expect("a toast is on screen");
    assert!(toast.error && toast.text == "Couldn't open /tmp/designos-toast-queue: no file manager", "on screen: {}", toast.text);
    assert_eq!(s.app().toast_queue.len(), 1, "the newer export toast still waits its turn");
    assert_eq!(s.app().toast_queue[0].reveal.as_deref(), Some(reveal_path("b.png").as_path()));
}

#[test]
fn a_late_failure_of_the_toast_on_screen_shows_the_error_at_once_with_an_export_queued() {
    let mut s = Sim::new();
    export_toast_on_screen(&mut s, "a.png");
    s.shell.app.show_toast_with_reveal("Exported b", reveal_path("b.png"));
    let a = reveal_path("a.png");
    crate::fig_export::push_late_reveal_failure(a.clone(), crate::fig_export::reveal_failure_text(&a, ""));
    s.frame(vec![]);
    let toast = s.app().toast.as_ref().expect("a toast is on screen");
    assert!(toast.error && toast.text == "Couldn't open /tmp/designos-toast-queue", "on screen: {}", toast.text);
    assert_eq!(s.app().toast_queue.len(), 1);
}

#[test]
fn a_late_failure_for_another_file_is_the_next_toast() {
    let mut s = Sim::new();
    export_toast_on_screen(&mut s, "a.png");
    s.shell.app.show_toast_with_reveal("Exported b", reveal_path("b.png"));
    let c = PathBuf::from("/tmp/designos-toast-other/c.png");
    crate::fig_export::push_late_reveal_failure(c.clone(), crate::fig_export::reveal_failure_text(&c, ""));
    s.frame(vec![]);
    assert!(s.app().toast.as_ref().unwrap().reveal.is_some(), "the export toast on screen keeps its button");
    let queue = &s.app().toast_queue;
    assert!(queue[0].error && queue[0].text == "Couldn't open /tmp/designos-toast-other", "the error is next: {}", queue[0].text);
    assert_eq!(queue[1].reveal.as_deref(), Some(reveal_path("b.png").as_path()));
}

#[test]
fn a_repeating_error_behind_an_export_toast_stays_within_the_cap_and_ends_with_its_producer() {
    let mut s = Sim::new();
    export_toast_on_screen(&mut s, "a.png");
    let t0 = s.app().now;
    // An error every 1.5 s, as an autosave retry on a full disk raises it, for 120 s.
    let mut next_error = t0;
    let mut max_len = 0;
    let mut t = t0;
    while t < t0 + 120.0 {
        if t >= next_error {
            s.shell.app.show_toast("Could not save: disk full", true);
            next_error += 1.5;
        }
        t += 0.2;
        frame_at(&mut s, t);
        max_len = max_len.max(s.app().toast_queue.len());
    }
    assert!(max_len <= crate::app::TOAST_QUEUE_CAP, "the queue grew to {max_len}");
    let stop = s.app().now;
    while s.app().toast.is_some() && s.app().now < stop + 600.0 {
        let next = s.app().now + 0.5;
        frame_at(&mut s, next);
    }
    assert!(s.app().now - stop < 30.0, "stale errors kept showing for {:.1} s after the producer stopped", s.app().now - stop);
}

#[test]
fn a_repeated_error_adds_no_toast_but_still_counts() {
    let mut s = Sim::new();
    export_toast_on_screen(&mut s, "a.png");
    let before = s.app().error_toast_seq;
    s.shell.app.show_toast("Cannot import comments: bad file", true);
    s.shell.app.show_toast("Cannot import comments: bad file", true);
    s.shell.app.show_toast("Cannot import comments: other file", true);
    assert_eq!(s.app().toast_queue.len(), 2, "the same error is queued once, a different one still is");
    assert_eq!(s.app().error_toast_seq, before + 3, "every raise counts, so a repeated failed import still returns an error");
    assert_eq!(s.app().last_error_toast, "Cannot import comments: other file");
}

#[test]
fn enter_a_path_raised_behind_an_export_toast_goes_to_the_queued_toast() {
    let mut s = Sim::new();
    export_toast_on_screen(&mut s, "a.png");
    s.shell.app.show_no_file_dialog_toast(true);
    let on_screen = s.app().toast.as_ref().unwrap();
    assert!(!on_screen.enter_path && on_screen.reveal.is_some(), "the export toast on screen keeps only its own action");
    assert_eq!(s.app().toast_queue.len(), 1, "the no-dialog error waits");
    assert!(s.app().toast_queue[0].enter_path, "Enter a path lands on the toast that was queued");
}
