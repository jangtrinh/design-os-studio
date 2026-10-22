//! macOS: the app menu's Quit item, and Cmd+Q, which the menu owns, ask about unsaved edits like closing the window
//! does. winit's own Quit item ends the app through `terminate:` and skipped the question; `macos_menu::install`
//! points the item at a target that asks for the window close instead. The tests send the item's action to that
//! target through the Objective-C runtime, as AppKit does, and feed the close it asks for back the way egui-winit does.
//! What a headless test cannot reach is the menu itself: that the item exists and is wired is the manual check.
//!
//! A hidden window is another case: eframe skips `App::ui` then, and ends the app on a close request that no frame
//! answered with `CancelClose`. Quit has to bring the window back and ask for the close once it is visible.

use super::leave_prompt::{asks_about, click, dialog_title};
use super::unsaved_dot::{dot, edit, edit_now, external_sim_with, lib, library_sim, nodes_on_disk, two_rects, wait, SETTLED};
use super::*;
use egui::{ViewportCommand, ViewportEvent};
use std::time::Duration;

/// What one `eframe_frame` did to the app.
struct Pass {
    /// eframe ends the app: the window got a close request and no `CancelClose` answered it.
    ends_app: bool,
    /// How long egui says it can wait before the next frame. `Duration::MAX` means a window left alone draws nothing.
    repaint_after: Duration,
}

/// One frame the way eframe runs the root viewport. A `hidden` window is minimized or occluded: its viewport info says
/// so (`ViewportInfo::visible` is false) and eframe skips `App::ui`, so `Shell::draw` does not run. A `Close` the
/// previous frame sent comes back as the window's close event, as egui-winit hands it over.
fn eframe_frame(s: &mut Sim, hidden: bool) -> Pass {
    let close_requested = s.commands.contains(&ViewportCommand::Close);
    s.t += 0.05;
    let mut raw = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::Vec2::new(1600.0, 1088.0))),
        time: Some(s.t),
        ..Default::default()
    };
    let info = raw.viewports.entry(egui::ViewportId::ROOT).or_default();
    info.minimized = Some(false);
    info.occluded = Some(hidden);
    if close_requested {
        info.events.push(ViewportEvent::Close);
    }
    let (ctx, shell) = (s.ctx.clone(), &mut s.shell);
    // eframe calls this for every frame, hidden or not
    eframe::App::raw_input_hook(shell, &ctx, &mut raw);
    let out = ctx.run_ui(raw, |ui| {
        if !hidden {
            shell.draw(ui);
        }
    });
    let root = out.viewport_output.get(&egui::ViewportId::ROOT);
    s.commands = root.map(|v| v.commands.clone()).unwrap_or_default();
    s.sent_commands.extend(s.commands.iter().cloned());
    Pass { ends_app: close_requested && !s.commands.contains(&ViewportCommand::CancelClose), repaint_after: root.map_or(Duration::MAX, |v| v.repaint_delay) }
}

/// `n` frames in which the app must not end, and what egui said about waiting for the next one after each.
fn keep_running(s: &mut Sim, hidden: bool, n: usize, why: &str) -> Vec<Duration> {
    (0..n)
        .map(|_| {
            let pass = eframe_frame(s, hidden);
            assert!(!pass.ends_app, "{why}");
            pass.repaint_after
        })
        .collect()
}

/// How many times the app has sent `command` since the test last cleared `sent_commands`.
fn sent(s: &Sim, command: &ViewportCommand) -> usize {
    s.sent_commands.iter().filter(|sent| *sent == command).count()
}

/// Picks Quit in the app menu: the Close it asks for reaches egui-winit, which hands it to the next frame as the
/// close event of the window.
fn pick_quit(s: &mut Sim) {
    s.sent_commands.clear();
    crate::macos_menu::tests::pick_quit(&s.ctx);
    s.frame(vec![]);
    assert!(s.sent_commands.contains(&egui::ViewportCommand::Close), "Quit asks for the close of the window");
    s.close_frame();
}

#[test]
fn quit_in_the_app_menu_asks_about_unsaved_edits_and_cancel_keeps_the_app_open() {
    let (mut s, _d, path) = external_sim_with(&two_rects());
    edit(&mut s);
    let (edited, on_disk) = (s.app().doc.nodes.len(), nodes_on_disk(&path));
    pick_quit(&mut s);
    assert!(asks_about(&s, "Outside"), "{:?}", dialog_title(&s));
    assert!(s.commands.contains(&egui::ViewportCommand::CancelClose), "the window stays open for the question");
    s.sent_commands.clear();
    click(&mut s, "dialog-cancel");
    wait(&mut s, SETTLED);
    assert!(s.app().dialog.is_none(), "answered");
    assert!(!s.sent_commands.contains(&egui::ViewportCommand::Close), "Cancel keeps the app open");
    assert_eq!(s.app().doc.nodes.len(), edited, "the edit is still there");
    assert!(dot(&s), "still unsaved");
    assert_eq!(nodes_on_disk(&path), on_disk, "nothing was written");
    // asked again by the next Quit; Save writes the file and the quit goes on
    pick_quit(&mut s);
    assert!(asks_about(&s, "Outside"));
    s.sent_commands.clear();
    click(&mut s, "dialog-ok");
    assert_eq!(nodes_on_disk(&path), edited, "Save wrote the edit");
    assert!(s.sent_commands.contains(&egui::ViewportCommand::Close), "and the quit went on");
    s.close_frame();
    assert!(!s.commands.contains(&egui::ViewportCommand::CancelClose), "the close goes through");
}

#[test]
fn quit_in_the_app_menu_flushes_the_autosave_of_a_library_file_without_asking() {
    let (mut s, _d) = library_sim();
    let id = s.app().lib_file.as_ref().unwrap().id.clone();
    let on_disk = lib(&s).load_file(&id).unwrap().nodes.len();
    edit_now(&mut s);
    let edited = s.app().doc.nodes.len();
    assert_ne!(edited, on_disk, "the edit is in the document");
    assert_eq!(lib(&s).load_file(&id).unwrap().nodes.len(), on_disk, "and autosave has not written it yet");
    pick_quit(&mut s);
    assert!(s.app().dialog.is_none(), "{:?}", dialog_title(&s));
    assert!(!s.commands.contains(&egui::ViewportCommand::CancelClose), "the close goes through");
    assert_eq!(lib(&s).load_file(&id).unwrap().nodes.len(), edited, "the edit was written before the app ended");
}

#[test]
fn quit_in_the_app_menu_with_nothing_unsaved_lets_the_close_through() {
    let (mut s, _d, _path) = external_sim_with(&two_rects());
    pick_quit(&mut s);
    assert!(s.app().dialog.is_none());
    assert!(!s.commands.contains(&egui::ViewportCommand::CancelClose));
}

#[test]
fn quit_while_the_window_is_hidden_brings_it_back_and_asks_once_it_is_visible() {
    let (mut s, _d, path) = external_sim_with(&two_rects());
    edit(&mut s);
    let (edited, on_disk) = (s.app().doc.nodes.len(), nodes_on_disk(&path));
    keep_running(&mut s, true, 1, "minimized");
    s.sent_commands.clear();
    crate::macos_menu::tests::pick_quit(&s.ctx);
    crate::macos_menu::tests::pick_quit(&s.ctx);
    let waiting = keep_running(&mut s, true, 4, "the app ended without the question while the window was hidden");
    assert_eq!(sent(&s, &ViewportCommand::Close), 0, "no close is asked for while no frame can draw the question");
    assert!(sent(&s, &ViewportCommand::Minimized(false)) >= 1, "Quit brings the window back");
    assert!(waiting[1..].iter().all(|after| *after == Duration::MAX), "waiting for the window repaints nothing: {waiting:?}");
    // the window is back: the pending quit goes out as one close request, and the next frame asks
    s.sent_commands.clear();
    assert!(!eframe_frame(&mut s, false).ends_app);
    assert_eq!(sent(&s, &ViewportCommand::Close), 1, "one close request, although Quit was picked twice");
    assert!(!eframe_frame(&mut s, false).ends_app, "the close waits for the answer");
    assert!(asks_about(&s, "Outside"), "{:?}", dialog_title(&s));
    assert!(s.commands.contains(&ViewportCommand::CancelClose), "the window stays open for the question");
    // Cancel leaves the app open, and nothing is left pending to ask again
    s.sent_commands.clear();
    click(&mut s, "dialog-cancel");
    keep_running(&mut s, false, 3, "Cancel keeps the app open");
    assert!(s.app().dialog.is_none(), "answered");
    assert_eq!(sent(&s, &ViewportCommand::Close), 0, "Cancel keeps the app open");
    assert_eq!(s.app().doc.nodes.len(), edited, "the edit is still there");
    assert!(dot(&s), "still unsaved");
    assert_eq!(nodes_on_disk(&path), on_disk, "nothing was written");
}

#[test]
fn quit_while_the_window_is_hidden_with_nothing_unsaved_quits_once_it_is_visible() {
    let (mut s, _d, _path) = external_sim_with(&two_rects());
    keep_running(&mut s, true, 1, "minimized");
    crate::macos_menu::tests::pick_quit(&s.ctx);
    keep_running(&mut s, true, 2, "the quit waits for the window");
    keep_running(&mut s, false, 1, "this frame asks for the close");
    assert!(eframe_frame(&mut s, false).ends_app, "nothing to ask about: the quit goes through");
    assert!(s.app().dialog.is_none());
}

#[test]
fn quit_while_the_window_is_hidden_flushes_the_autosave_of_a_library_file() {
    let (mut s, _d) = library_sim();
    let id = s.app().lib_file.as_ref().unwrap().id.clone();
    edit_now(&mut s);
    let edited = s.app().doc.nodes.len();
    assert_ne!(lib(&s).load_file(&id).unwrap().nodes.len(), edited, "autosave has not written the edit yet");
    keep_running(&mut s, true, 1, "minimized");
    crate::macos_menu::tests::pick_quit(&s.ctx);
    keep_running(&mut s, true, 2, "the quit waits for the window");
    keep_running(&mut s, false, 1, "this frame asks for the close");
    assert!(eframe_frame(&mut s, false).ends_app, "a library edit needs no question");
    assert_eq!(lib(&s).load_file(&id).unwrap().nodes.len(), edited, "the edit was written before the app ended");
}

#[test]
fn quit_while_the_window_is_visible_asks_for_the_close_at_once() {
    let (mut s, _d, _path) = external_sim_with(&two_rects());
    edit(&mut s);
    assert!(!eframe_frame(&mut s, false).ends_app);
    s.sent_commands.clear();
    crate::macos_menu::tests::pick_quit(&s.ctx);
    assert!(!eframe_frame(&mut s, false).ends_app);
    assert_eq!(sent(&s, &ViewportCommand::Close), 1, "the very next frame carries the close");
    assert_eq!(sent(&s, &ViewportCommand::Minimized(false)), 0, "a visible window is left as it is");
    assert_eq!(sent(&s, &ViewportCommand::Focus), 0);
    assert!(!eframe_frame(&mut s, false).ends_app);
    assert!(asks_about(&s, "Outside"), "{:?}", dialog_title(&s));
}

/// A quit that waited for a window which never came back is gone: the window showing up much later (an occluding
/// window moves away, say) must not end the app, and must not ask anything out of nowhere.
#[test]
fn a_pending_quit_the_window_does_not_answer_in_time_is_dropped() {
    let (mut s, _d, path) = external_sim_with(&two_rects());
    edit(&mut s);
    let (edited, on_disk) = (s.app().doc.nodes.len(), nodes_on_disk(&path));
    keep_running(&mut s, true, 1, "occluded");
    crate::macos_menu::tests::pick_quit(&s.ctx);
    s.sent_commands.clear();
    // about ten seconds of frames, far over the limit of a pending quit
    let waiting = keep_running(&mut s, true, 200, "the window stays hidden for a long time");
    assert_eq!(sent(&s, &ViewportCommand::Close), 0, "no close while the window is hidden");
    assert!(waiting[1..].iter().all(|after| *after == Duration::MAX), "waiting repaints nothing: {waiting:?}");
    keep_running(&mut s, false, 20, "a quit that ran out must not end the app when the window shows");
    assert_eq!(sent(&s, &ViewportCommand::Close), 0, "the pending quit was dropped, not sent");
    assert!(s.app().dialog.is_none(), "{:?}", dialog_title(&s));
    assert!(!eframe_frame(&mut s, false).ends_app);
    assert_eq!(s.app().doc.nodes.len(), edited, "the edit is still there");
    assert!(dot(&s), "still unsaved");
    assert_eq!(nodes_on_disk(&path), on_disk, "nothing was written");
}

/// Inside the limit nothing changes: the window shows, the close goes out once, and the question comes up.
#[test]
fn a_pending_quit_the_window_answers_within_the_limit_still_asks_once() {
    let (mut s, _d, path) = external_sim_with(&two_rects());
    edit(&mut s);
    let on_disk = nodes_on_disk(&path);
    keep_running(&mut s, true, 1, "minimized");
    crate::macos_menu::tests::pick_quit(&s.ctx);
    // 2.5 s of hidden frames
    keep_running(&mut s, true, 50, "the app ended without the question while the window was hidden");
    s.sent_commands.clear();
    assert!(!eframe_frame(&mut s, false).ends_app);
    assert_eq!(sent(&s, &ViewportCommand::Close), 1, "one close request");
    assert!(!eframe_frame(&mut s, false).ends_app, "the close waits for the answer");
    assert!(asks_about(&s, "Outside"), "{:?}", dialog_title(&s));
    assert!(s.commands.contains(&ViewportCommand::CancelClose), "the window stays open for the question");
    assert_eq!(nodes_on_disk(&path), on_disk, "nothing was written");
}

/// The wait counts from the first frame after the request. While a hidden window is left alone no frame runs, so the
/// clock egui last saw can be hours old when Quit is chosen; counting from it would drop a quit that is answered at once.
#[test]
fn a_quit_after_a_long_idle_stretch_counts_from_the_frame_it_wakes() {
    let (mut s, _d, _path) = external_sim_with(&two_rects());
    edit(&mut s);
    keep_running(&mut s, true, 1, "minimized");
    s.t += 3600.0; // an hour in which no frame ran
    crate::macos_menu::tests::pick_quit(&s.ctx);
    keep_running(&mut s, true, 2, "the quit waits for the window");
    s.sent_commands.clear();
    assert!(!eframe_frame(&mut s, false).ends_app);
    assert_eq!(sent(&s, &ViewportCommand::Close), 1, "the window came back at once: the quit is not stale");
    assert!(!eframe_frame(&mut s, false).ends_app, "the close waits for the answer");
    assert!(asks_about(&s, "Outside"), "{:?}", dialog_title(&s));
}

/// Quit chosen again while it waits is a new request: the wait starts over.
#[test]
fn a_second_quit_starts_the_wait_again() {
    let (mut s, _d, _path) = external_sim_with(&two_rects());
    edit(&mut s);
    keep_running(&mut s, true, 1, "occluded");
    crate::macos_menu::tests::pick_quit(&s.ctx);
    keep_running(&mut s, true, 200, "the first quit runs out");
    crate::macos_menu::tests::pick_quit(&s.ctx);
    keep_running(&mut s, true, 2, "the second quit waits for the window");
    s.sent_commands.clear();
    assert!(!eframe_frame(&mut s, false).ends_app);
    assert_eq!(sent(&s, &ViewportCommand::Close), 1, "the second quit is answered");
    assert!(!eframe_frame(&mut s, false).ends_app);
    assert!(asks_about(&s, "Outside"), "{:?}", dialog_title(&s));
}
