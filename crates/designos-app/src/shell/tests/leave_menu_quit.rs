//! macOS: the app menu's Quit item, and Cmd+Q, which the menu owns, ask about unsaved edits like closing the window
//! does. winit's own Quit item ends the app through `terminate:` and skipped the question; `macos_menu::install`
//! points the item at a target that asks for the window close instead. The tests send the item's action to that
//! target through the Objective-C runtime, as AppKit does, and feed the close it asks for back the way egui-winit does.
//! What a headless test cannot reach is the menu itself: that the item exists and is wired is the manual check.

use super::leave_prompt::{asks_about, click, dialog_title};
use super::unsaved_dot::{dot, edit, edit_now, external_sim_with, lib, library_sim, nodes_on_disk, two_rects, wait, SETTLED};
use super::*;

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
