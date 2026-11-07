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
    s.shell.app.show_no_file_dialog_toast(true);
    assert_eq!(s.app().toast_queue.len(), 1, "the second notice waits");
    assert!(s.app().toast_queue[0].enter_path, "the action went to the toast that was queued, not to the one on screen");
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
