//! "Save changes to <name>?": closing a tab, quitting, closing the other tabs and opening a file over a
//! document outside the library that has edits no save has written. A library file autosaves and never asks;
//! a document with nothing unsaved is not asked about.

use super::unsaved_dot::{dot, edit, external_sim_with, lib, library_sim, nodes_on_disk, two_rects, wait, SETTLED};
use super::*;
use crate::canvas::tools::Tool;
use crate::home::{Answer, Leave};
use crate::io::{DialogOutcome, ScriptedDialogs};
use designos_doc::Document;
use std::path::Path;

pub(super) fn dialog_title(s: &Sim) -> Option<String> {
    s.app().dialog.as_ref().map(|d| d.title.clone())
}

pub(super) fn asks_about(s: &Sim, name: &str) -> bool {
    dialog_title(s).as_deref() == Some(&format!("Save changes to {name}?"))
}

pub(super) fn click(s: &mut Sim, name: &str) {
    let r = s.rect_of(name);
    s.click(r.center());
}

pub(super) fn tab_id(s: &Sim, idx: usize) -> u64 {
    s.app().tabs.entries[idx].id
}

pub(super) fn close_tab_by_click(s: &mut Sim, idx: usize) {
    let id = tab_id(s, idx);
    click(s, &format!("tab-close-{id}"));
    wait(s, 0.1);
}

fn rewrite(path: &Path, doc: &Document) {
    std::fs::write(path, doc.to_json().unwrap()).unwrap();
}

/// A second tab, an untitled document with edits no save has written. It becomes the active tab.
pub(super) fn add_an_unsaved_untitled_tab(s: &mut Sim) {
    let mut doc = two_rects();
    doc.name = "Untitled".into();
    s.shell.app.open_new_tab("Untitled", doc, crate::tabs::ProjectTabKind::Design);
    wait(s, SETTLED);
    edit(s);
}

/// Opens a second, clean tab (an untitled document) next to the file's tab, goes back to the file's tab, which is
/// at index 0, and edits it: the file has an edit no save has written. (Takes the sim by reference: a `Sim` is large,
/// and a test thread's stack holds only a few copies of it.)
pub(super) fn make_two_tabs(s: &mut Sim) {
    let mut doc = two_rects();
    doc.name = "Second".into();
    s.shell.app.open_new_tab("Second", doc, crate::tabs::ProjectTabKind::Design);
    wait(s, SETTLED);
    let mut tabs = std::mem::take(&mut s.shell.app.tabs);
    tabs.switch_to(0, &mut s.shell.app);
    s.shell.app.tabs = tabs;
    wait(s, SETTLED);
    edit(s);
    assert_eq!(s.app().tabs.entries.len(), 2);
    assert!(dot(s));
}

#[test]
fn closing_a_tab_with_unsaved_edits_asks_and_leaves_the_tab_open() {
    let (mut s, _d, _path) = external_sim_with(&two_rects());
    make_two_tabs(&mut s);
    let edited = s.app().doc.nodes.len();
    close_tab_by_click(&mut s, 0);
    assert!(asks_about(&s, "Outside"), "{:?}", dialog_title(&s));
    assert_eq!(s.app().tabs.entries.len(), 2, "nothing is closed until it is answered");
    assert_eq!(s.app().doc.nodes.len(), edited);
}

#[test]
fn save_writes_the_file_then_closes_the_tab() {
    let (mut s, _d, path) = external_sim_with(&two_rects());
    make_two_tabs(&mut s);
    let edited = s.app().doc.nodes.len();
    close_tab_by_click(&mut s, 0);
    click(&mut s, "dialog-ok");
    wait(&mut s, SETTLED);
    assert_eq!(nodes_on_disk(&path), edited, "the edit is in the file");
    assert_eq!(s.app().tabs.entries.len(), 1, "then the tab closed");
    assert!(s.app().dialog.is_none());
}

#[test]
fn dont_save_closes_the_tab_and_drops_the_edits() {
    let (mut s, _d, path) = external_sim_with(&two_rects());
    make_two_tabs(&mut s);
    let on_disk = nodes_on_disk(&path);
    close_tab_by_click(&mut s, 0);
    click(&mut s, "dialog-alt");
    wait(&mut s, SETTLED);
    assert_eq!(s.app().tabs.entries.len(), 1);
    assert_eq!(nodes_on_disk(&path), on_disk, "the file is as it was");
    assert!(s.app().dialog.is_none());
}

#[test]
fn cancel_escape_and_a_click_outside_leave_everything_as_it_was() {
    for how in ["button", "escape", "outside"] {
        let (mut s, _d, path) = external_sim_with(&two_rects());
        make_two_tabs(&mut s);
        let (edited, on_disk) = (s.app().doc.nodes.len(), nodes_on_disk(&path));
        let active = s.app().tabs.active_idx;
        close_tab_by_click(&mut s, 0);
        assert!(asks_about(&s, "Outside"));
        match how {
            "button" => click(&mut s, "dialog-cancel"),
            "escape" => s.key(Key::Escape),
            _ => s.click(Pos2::new(5.0, 5.0)),
        }
        wait(&mut s, SETTLED);
        assert!(s.app().dialog.is_none(), "answered ({how})");
        assert_eq!(s.app().tabs.entries.len(), 2, "the tab stays ({how})");
        assert_eq!(s.app().tabs.active_idx, active, "and so does the active tab ({how})");
        assert_eq!(s.app().doc.nodes.len(), edited, "the edit stays ({how})");
        assert!(dot(&s), "still unsaved ({how})");
        assert_eq!(nodes_on_disk(&path), on_disk, "nothing was written ({how})");
    }
}

#[test]
fn enter_means_save_and_the_action_goes_on() {
    // closing a tab
    let (mut s, _d, path) = external_sim_with(&two_rects());
    make_two_tabs(&mut s);
    let edited = s.app().doc.nodes.len();
    close_tab_by_click(&mut s, 0);
    assert!(asks_about(&s, "Outside"));
    s.key(Key::Enter);
    wait(&mut s, SETTLED);
    assert_eq!(nodes_on_disk(&path), edited, "Enter saved");
    assert_eq!(s.app().tabs.entries.len(), 1, "and the tab closed");
    // quitting
    let (mut s, _d, path) = external_sim_with(&two_rects());
    make_two_tabs(&mut s);
    let edited = s.app().doc.nodes.len();
    s.close_frame();
    assert!(asks_about(&s, "Outside"));
    s.sent_commands.clear();
    s.key(Key::Enter);
    assert_eq!(nodes_on_disk(&path), edited, "Enter saved");
    assert!(s.sent_commands.contains(&egui::ViewportCommand::Close), "and the quit went on");
}

#[test]
fn save_on_an_untitled_document_opens_the_save_dialog_and_cancelling_it_cancels_the_close() {
    let (mut s, d, _path) = external_sim_with(&two_rects());
    add_an_unsaved_untitled_tab(&mut s);
    let edited = s.app().doc.nodes.len();
    let idx = s.app().tabs.active_idx;
    for outcome in [DialogOutcome::Cancelled, DialogOutcome::Unavailable] {
        s.shell.app.request_leave(None, Leave::CloseTab(tab_id(&s, idx)));
        assert!(asks_about(&s, "Untitled"));
        s.shell.app.dialog = None;
        let dialogs = ScriptedDialogs::answering(outcome);
        s.shell.app.answer_save_changes_with(None, Answer::Save, &dialogs);
        wait(&mut s, SETTLED);
        assert_eq!(dialogs.requests.borrow().len(), 1, "the Save dialog was asked for");
        assert_eq!(s.app().tabs.entries.len(), 2, "no file was chosen, so the tab stays");
        assert_eq!(s.app().doc.nodes.len(), edited);
        assert!(dot(&s));
    }
    // a name picked: saved, then the tab closes
    s.shell.app.request_leave(None, Leave::CloseTab(tab_id(&s, idx)));
    s.shell.app.dialog = None;
    let target = d.0.join("untitled.designos");
    s.shell.app.answer_save_changes_with(None, Answer::Save, &ScriptedDialogs::answering(DialogOutcome::Picked(target.clone())));
    wait(&mut s, SETTLED);
    assert_eq!(nodes_on_disk(&target), edited, "the edit is in the file that was picked");
    assert_eq!(s.app().tabs.entries.len(), 1, "then the tab closed");
}

#[test]
fn closing_the_other_tabs_asks_for_each_unsaved_one_in_tab_order() {
    let (mut s, _d, path) = external_sim_with(&two_rects());
    make_two_tabs(&mut s);
    add_an_unsaved_untitled_tab(&mut s);
    let keep = {
        let mut doc = two_rects();
        doc.name = "Keeper".into();
        s.shell.app.open_new_tab("Keeper", doc, crate::tabs::ProjectTabKind::Design);
        wait(&mut s, SETTLED);
        tab_id(&s, s.app().tabs.active_idx)
    };
    assert_eq!(s.app().tabs.entries.len(), 4);
    let on_disk = nodes_on_disk(&path);
    s.shell.app.request_leave(None, Leave::CloseOthers(keep));
    assert!(asks_about(&s, "Outside"), "tab order: the file first, {:?}", dialog_title(&s));
    s.shell.app.dialog = None;
    s.shell.app.answer_save_changes_with(None, Answer::DontSave, &ScriptedDialogs::answering(DialogOutcome::Cancelled));
    assert!(asks_about(&s, "Untitled"), "then the untitled one, {:?}", dialog_title(&s));
    s.shell.app.dialog = None;
    s.shell.app.answer_save_changes_with(None, Answer::DontSave, &ScriptedDialogs::answering(DialogOutcome::Cancelled));
    wait(&mut s, SETTLED);
    assert_eq!(s.app().tabs.entries.len(), 1, "only the kept tab is left");
    assert_eq!(s.app().tabs.entries[0].id, keep);
    assert_eq!(nodes_on_disk(&path), on_disk);
}

#[test]
fn quitting_asks_cancels_the_close_and_lets_it_through_once_answered() {
    let (mut s, _d, _path) = external_sim_with(&two_rects());
    make_two_tabs(&mut s);
    s.close_frame();
    assert!(asks_about(&s, "Outside"), "{:?}", dialog_title(&s));
    assert!(s.commands.contains(&egui::ViewportCommand::CancelClose), "the window stays open for the question");
    s.sent_commands.clear();
    click(&mut s, "dialog-alt");
    assert!(s.sent_commands.contains(&egui::ViewportCommand::Close), "answered: the close is asked for again");
    s.close_frame();
    assert!(!s.commands.contains(&egui::ViewportCommand::CancelClose), "and goes through");
    assert!(s.app().dialog.is_none());
}

#[test]
fn quitting_asks_once_per_unsaved_document_in_tab_order_and_cancel_stops_the_quit() {
    let (mut s, _d, path) = external_sim_with(&two_rects());
    make_two_tabs(&mut s);
    add_an_unsaved_untitled_tab(&mut s);
    let untitled_edited = s.app().doc.nodes.len();
    let active = s.app().tabs.active_idx;
    let on_disk = nodes_on_disk(&path);
    s.close_frame();
    assert!(asks_about(&s, "Outside"), "tab order: the file first, {:?}", dialog_title(&s));
    s.sent_commands.clear();
    click(&mut s, "dialog-alt");
    assert!(asks_about(&s, "Untitled"), "then the untitled document, {:?}", dialog_title(&s));
    assert!(!s.sent_commands.contains(&egui::ViewportCommand::Close), "the quit waits for the second answer");
    click(&mut s, "dialog-cancel");
    wait(&mut s, SETTLED);
    assert!(!s.sent_commands.contains(&egui::ViewportCommand::Close), "Cancel on the second one stops the quit");
    assert!(s.app().dialog.is_none());
    assert_eq!(s.app().tabs.entries.len(), 3, "no tab was closed");
    assert_eq!(s.app().tabs.active_idx, active);
    assert_eq!(s.app().doc.nodes.len(), untitled_edited, "the edit is still there");
    assert_eq!(nodes_on_disk(&path), on_disk);
    // asked again by the next quit
    s.close_frame();
    assert!(asks_about(&s, "Outside"));
}

#[test]
fn a_close_request_while_a_question_is_open_changes_nothing() {
    let (mut s, _d, _path) = external_sim_with(&two_rects());
    make_two_tabs(&mut s);
    s.close_frame();
    assert!(asks_about(&s, "Outside"));
    s.close_frame();
    assert!(asks_about(&s, "Outside"), "still the same question");
    assert!(s.commands.contains(&egui::ViewportCommand::CancelClose));
    assert!(!s.commands.contains(&egui::ViewportCommand::Close));
}

#[test]
fn opening_a_file_over_unsaved_edits_asks_and_each_answer_does_what_it_says() {
    for answer in ["save", "dont-save", "cancel"] {
        let (mut s, d, path) = external_sim_with(&two_rects());
        edit(&mut s);
        let edited = s.app().doc.nodes.len();
        let second = d.0.join("second.designos");
        let mut other = two_rects();
        other.name = "Second file".into();
        rewrite(&second, &other);
        s.shell.app.open_dialog_with(&ScriptedDialogs::answering(DialogOutcome::Picked(second.clone())));
        assert!(asks_about(&s, "Outside"), "{answer}: {:?}", dialog_title(&s));
        assert_eq!(s.app().doc.name, "Outside", "nothing is replaced until it is answered ({answer})");
        s.frame(vec![]);
        click(
            &mut s,
            match answer {
                "save" => "dialog-ok",
                "dont-save" => "dialog-alt",
                _ => "dialog-cancel",
            },
        );
        wait(&mut s, SETTLED);
        assert!(s.app().dialog.is_none());
        match answer {
            "save" => {
                assert_eq!(nodes_on_disk(&path), edited, "the edit was saved");
                assert_eq!(s.app().doc.name, "Second file", "and the file opened");
            }
            "dont-save" => {
                assert_eq!(s.app().doc.name, "Second file", "the file opened");
                assert_ne!(nodes_on_disk(&path), edited, "the edit was dropped");
            }
            _ => {
                assert_eq!(s.app().doc.name, "Outside", "still the same document");
                assert_eq!(s.app().doc.nodes.len(), edited);
                assert!(dot(&s));
            }
        }
    }
}

#[test]
fn reopening_an_open_file_from_home_asks_and_each_answer_does_what_it_says() {
    for answer in ["save", "dont-save", "cancel"] {
        let (mut s, _d, path) = external_sim_with(&two_rects());
        edit(&mut s);
        let edited = s.app().doc.nodes.len();
        let before = nodes_on_disk(&path);
        s.shell.app.go_home();
        wait(&mut s, SETTLED);
        s.shell.app.open_path(path.clone());
        wait(&mut s, 0.2);
        assert!(asks_about(&s, "Outside"), "{answer}: {:?}", dialog_title(&s));
        click(
            &mut s,
            match answer {
                "save" => "dialog-ok",
                "dont-save" => "dialog-alt",
                _ => "dialog-cancel",
            },
        );
        wait(&mut s, 1.5);
        assert!(s.app().dialog.is_none());
        match answer {
            "save" => {
                assert_eq!(nodes_on_disk(&path), edited, "saved first");
                assert!(s.app().home.is_none(), "then opened");
                assert_eq!(s.app().doc.nodes.len(), edited);
                assert!(!dot(&s));
            }
            "dont-save" => {
                assert!(s.app().home.is_none(), "opened from the file");
                assert_eq!(s.app().doc.nodes.len(), before, "the edit was dropped");
                assert_eq!(nodes_on_disk(&path), before);
                assert!(!dot(&s), "a document just read from its file has nothing unsaved");
            }
            _ => {
                assert!(s.app().home.is_some(), "still on Home");
                s.shell.app.switch_tab(0);
                wait(&mut s, SETTLED);
                assert_eq!(s.app().doc.nodes.len(), edited, "the tab kept its edit");
                assert!(dot(&s));
            }
        }
    }
}

#[test]
fn a_library_file_never_asks() {
    let (mut s, _d) = library_sim();
    edit(&mut s);
    let id = s.app().lib_file.as_ref().unwrap().id.clone();
    let project = s.app().lib_file.as_ref().unwrap().project_id.clone();
    let other = lib(&s).create_file(&project, None, "Other", &two_rects()).unwrap();
    // quitting
    s.close_frame();
    assert!(s.app().dialog.is_none(), "{:?}", dialog_title(&s));
    assert!(!s.commands.contains(&egui::ViewportCommand::CancelClose));
    // opening another file over it
    let path = lib(&s).path("projects").join(&project).join("files").join(format!("{}.designos", other.id));
    s.shell.app.open_path(path);
    assert!(s.app().dialog.is_none());
    // closing its tab
    let idx = s.app().tabs.entries.iter().position(|t| t.lib_file.as_ref().is_some_and(|f| f.id == id));
    if let Some(idx) = idx {
        let tab = tab_id(&s, idx);
        s.shell.app.request_leave(None, Leave::CloseTab(tab));
        assert!(s.app().dialog.is_none());
    }
}

#[test]
fn a_document_with_nothing_unsaved_never_asks() {
    let (mut s, d, _path) = external_sim_with(&two_rects());
    let mut second = two_rects();
    second.name = "Second".into();
    s.shell.app.open_new_tab("Second", second, crate::tabs::ProjectTabKind::Design);
    wait(&mut s, SETTLED);
    // quit
    s.close_frame();
    assert!(s.app().dialog.is_none());
    assert!(!s.commands.contains(&egui::ViewportCommand::CancelClose));
    // open another file over the shown document
    let other = d.0.join("other.designos");
    rewrite(&other, &two_rects());
    s.shell.app.open_path(other);
    assert!(s.app().dialog.is_none());
    // close a tab
    close_tab_by_click(&mut s, 0);
    assert!(s.app().dialog.is_none());
}

#[test]
fn an_edit_in_progress_counts_as_unsaved() {
    let (mut s, _d, _path) = external_sim_with(&two_rects());
    let mut second = two_rects();
    second.name = "Second".into();
    s.shell.app.open_new_tab("Second", second, crate::tabs::ProjectTabKind::Design);
    wait(&mut s, SETTLED);
    let mut tabs = std::mem::take(&mut s.shell.app.tabs);
    tabs.switch_to(0, &mut s.shell.app);
    s.shell.app.tabs = tabs;
    wait(&mut s, SETTLED);
    assert!(!dot(&s));
    // a drag that has started: the pointer is down on a layer and has moved, and nothing is in the history yet
    let from = s.screen(25.0, 25.0);
    s.mv(from);
    s.button(true);
    s.mv(s.screen(60.0, 60.0));
    s.mv(s.screen(90.0, 90.0));
    assert!(crate::live_patch_apply::user_edit_in_progress(s.app()), "a drag is under way");
    let first = tab_id(&s, 0);
    s.shell.app.request_leave(None, Leave::CloseTab(first));
    assert!(asks_about(&s, "Outside"), "an edit under way is not dropped without a question: {:?}", dialog_title(&s));
}

#[test]
fn an_open_text_session_is_committed_and_counted() {
    let (mut s, _d, _path) = external_sim_with(&two_rects());
    make_two_tabs(&mut s);
    // a clean first tab: undo what two_tabs() edited
    s.shell.app.undo();
    wait(&mut s, SETTLED);
    assert!(!dot(&s));
    s.shell.app.set_tool(Tool::Text);
    // an empty spot of the canvas
    let p = s.app().canvas_rect.min + egui::vec2(100.0, 420.0);
    s.click(p);
    assert!(s.app().canvas.text.is_some(), "a text session is open");
    s.one_frame().text("hi").send();
    assert!(!dot(&s), "the typed text is not in the history yet, so the dot cannot show it");
    let first = tab_id(&s, 0);
    s.shell.app.request_leave(None, Leave::CloseTab(first));
    assert!(s.app().canvas.text.is_none(), "the session was committed");
    assert_eq!(s.app().hist.undo_depth(), 1, "its text is a step of the history");
    assert!(asks_about(&s, "Outside"), "{:?}", dialog_title(&s));
    assert_eq!(s.app().tabs.entries.len(), 2);
}

#[test]
fn an_open_question_asks_for_no_repaint() {
    let (mut s, _d, _path) = external_sim_with(&two_rects());
    make_two_tabs(&mut s);
    // Opening a file, switching tabs and editing make the canvas draw its tiles again, a few per frame, and a runner with
    // few cores draws them more slowly than a laptop: the question must open on a window that is idle (see `idle_repaint`).
    s.settle().expect("a window finishes the work it has after opening a file");
    close_tab_by_click(&mut s, 0);
    assert!(asks_about(&s, "Outside"));
    // A background thread of the window (a file watcher, a scan) can wake it once while the question waits: a wake, not a
    // repaint loop. The check waits for the window to go quiet (see `idle_repaint`).
    s.assert_idle_window_asks_for_no_repaint("the question open");
}

#[test]
fn quitting_from_home_asks_about_the_parked_tabs_and_cancel_leaves_home_as_it_was() {
    let (mut s, _d, path) = external_sim_with(&two_rects());
    edit(&mut s);
    let edited = s.app().doc.nodes.len();
    s.shell.app.go_home();
    wait(&mut s, SETTLED);
    s.close_frame();
    assert!(asks_about(&s, "Outside"), "{:?}", dialog_title(&s));
    assert!(s.commands.contains(&egui::ViewportCommand::CancelClose));
    s.sent_commands.clear();
    click(&mut s, "dialog-cancel");
    wait(&mut s, SETTLED);
    assert!(s.app().home.is_some(), "still on Home");
    assert!(!s.sent_commands.contains(&egui::ViewportCommand::Close));
    // Save from Home: the tab is shown to save it, then the window closes
    s.close_frame();
    s.sent_commands.clear();
    click(&mut s, "dialog-ok");
    assert_eq!(nodes_on_disk(&path), edited, "saved");
    assert!(s.sent_commands.contains(&egui::ViewportCommand::Close), "then the close goes through");
}

#[test]
fn a_new_file_or_a_library_file_opened_over_unsaved_edits_asks_first() {
    for what in ["new", "open"] {
        let (mut s, _d, _path) = external_sim_with(&two_rects());
        edit(&mut s);
        let edited = s.app().doc.nodes.len();
        let project = s.app().lib.as_ref().unwrap().list_projects()[0].id.clone();
        let other = lib(&s).create_file(&project, None, "Library file", &two_rects()).unwrap();
        let files = lib(&s).all_files().len();
        match what {
            "new" => s.shell.app.new_file_here(),
            _ => s.shell.app.open_library_file(&other.id),
        }
        assert!(asks_about(&s, "Outside"), "{what}: {:?}", dialog_title(&s));
        assert_eq!(s.app().doc.name, "Outside", "nothing is replaced until it is answered ({what})");
        assert_eq!(lib(&s).all_files().len(), files, "and no file is created before the answer ({what})");
        // Cancel keeps everything
        wait(&mut s, 0.1);
        click(&mut s, "dialog-cancel");
        wait(&mut s, SETTLED);
        assert_eq!(s.app().doc.nodes.len(), edited, "{what}: cancel");
        assert!(dot(&s));
        // Don't save: it goes ahead
        match what {
            "new" => s.shell.app.new_file_here(),
            _ => s.shell.app.open_library_file(&other.id),
        }
        wait(&mut s, 0.1);
        click(&mut s, "dialog-alt");
        wait(&mut s, SETTLED);
        assert!(s.app().lib_file.is_some(), "{what}: a library file is on screen");
        assert!(s.app().dialog.is_none());
    }
}

/// Where a layer is: the translation of its transform.
fn position_of(s: &Sim, id: designos_doc::NodeId) -> (f32, f32) {
    let t = s.app().doc.get(id).unwrap().transform;
    (t.tx, t.ty)
}

#[test]
fn a_drag_in_progress_is_ended_where_it_is_as_one_undo_step_before_the_question_and_save_writes_it() {
    let (mut s, _d, path) = external_sim_with(&two_rects());
    let mut second = two_rects();
    second.name = "Second".into();
    s.shell.app.open_new_tab("Second", second, crate::tabs::ProjectTabKind::Design);
    wait(&mut s, SETTLED);
    let mut tabs = std::mem::take(&mut s.shell.app.tabs);
    tabs.switch_to(0, &mut s.shell.app);
    s.shell.app.tabs = tabs;
    wait(&mut s, SETTLED);
    assert!(!dot(&s));
    let first = s.app().doc.get(s.app().page).unwrap().children[0];
    let before = position_of(&s, first);
    let depth = s.app().hist.undo_depth();
    // a drag that has started: the pointer is down on the layer and has moved
    let from = s.screen(25.0, 25.0);
    s.mv(from);
    s.button(true);
    s.mv(s.screen(60.0, 60.0));
    s.mv(s.screen(90.0, 90.0));
    assert!(crate::live_patch_apply::user_edit_in_progress(s.app()), "a drag is under way");
    let dragged = position_of(&s, first);
    assert_ne!(dragged, before, "the layer follows the pointer");
    assert_eq!(s.app().hist.undo_depth(), depth, "and nothing is in the history yet");
    // Ctrl+W while the pointer is still down
    s.cmd_key(Key::W);
    assert!(asks_about(&s, "Outside"), "{:?}", dialog_title(&s));
    assert!(!crate::live_patch_apply::user_edit_in_progress(s.app()), "the drag was ended before the question");
    assert_eq!(position_of(&s, first), dragged, "where it was");
    assert_eq!(s.app().hist.undo_depth(), depth + 1, "as one undo step");
    // one undo brings back the position before the drag, one redo the dragged one
    s.shell.app.undo();
    assert_eq!(position_of(&s, first), before, "one undo restores the pre-drag position");
    s.shell.app.redo();
    assert_eq!(position_of(&s, first), dragged);
    // Save writes a settled document: the file holds the dragged position
    s.shell.app.dialog = None;
    s.shell.app.answer_save_changes_with(None, Answer::Save, &ScriptedDialogs::answering(DialogOutcome::Cancelled));
    wait(&mut s, SETTLED);
    let saved = Document::from_json(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let t = saved.get(first).unwrap().transform;
    assert_eq!((t.tx, t.ty), dragged, "the file holds the dragged position");
    assert_eq!(s.app().tabs.entries.len(), 1, "and the tab closed");
}

#[test]
fn a_property_scrub_in_progress_is_ended_as_one_undo_step_before_the_question() {
    let (mut s, _d, _path) = external_sim_with(&two_rects());
    let mut second = two_rects();
    second.name = "Second".into();
    s.shell.app.open_new_tab("Second", second, crate::tabs::ProjectTabKind::Design);
    wait(&mut s, SETTLED);
    let mut tabs = std::mem::take(&mut s.shell.app.tabs);
    tabs.switch_to(0, &mut s.shell.app);
    s.shell.app.tabs = tabs;
    wait(&mut s, SETTLED);
    let first = s.app().doc.get(s.app().page).unwrap().children[0];
    let depth = s.app().hist.undo_depth();
    // the state a numeric field's scrub leaves: a snapshot of the document, and the layer already changed
    s.shell.app.scrub = Some(s.app().doc.nodes.clone());
    let mut n = s.app().doc.get(first).unwrap().clone();
    n.opacity = 0.25;
    s.shell.app.doc.nodes.insert(first, n);
    s.shell.app.changed();
    let tab = tab_id(&s, 0);
    s.shell.app.request_leave(None, Leave::CloseTab(tab));
    assert!(asks_about(&s, "Outside"), "{:?}", dialog_title(&s));
    assert!(s.app().scrub.is_none(), "the scrub was ended");
    assert_eq!(s.app().hist.undo_depth(), depth + 1, "as one undo step");
    assert_eq!(s.app().doc.get(first).unwrap().opacity, 0.25, "where it was");
    s.shell.app.undo();
    assert_eq!(s.app().doc.get(first).unwrap().opacity, 1.0, "one undo restores the value before the scrub");
}

#[test]
fn a_press_that_has_not_become_a_drag_is_not_made_into_an_edit_by_the_question() {
    // the rectangle tool, the pointer down on an empty spot and not moved: ending that as a release would draw a default shape
    let (mut s, _d, _path) = external_sim_with(&two_rects());
    let mut second = two_rects();
    second.name = "Second".into();
    s.shell.app.open_new_tab("Second", second, crate::tabs::ProjectTabKind::Design);
    wait(&mut s, SETTLED);
    let mut tabs = std::mem::take(&mut s.shell.app.tabs);
    tabs.switch_to(0, &mut s.shell.app);
    s.shell.app.tabs = tabs;
    wait(&mut s, SETTLED);
    s.shell.app.set_tool(Tool::Rectangle);
    let spot = s.app().canvas_rect.min + egui::vec2(100.0, 420.0);
    s.mv(spot);
    s.button(true);
    assert!(crate::live_patch_apply::user_edit_in_progress(s.app()), "a draw has started");
    let tab = tab_id(&s, 0);
    s.shell.app.request_leave(None, Leave::CloseTab(tab));
    assert!(s.app().dialog.is_none(), "the document had nothing unsaved: {:?}", dialog_title(&s));
    assert_eq!(s.app().tabs.entries.len(), 1, "so the tab closed");
}

#[test]
fn screenshot_run_closes_without_leave_questions_when_dirty() {
    let dir = super::unsaved_dot::TmpDir::new();
    let path = dir.0.join("outside.designos");
    std::fs::write(&path, two_rects().to_json().unwrap()).unwrap();
    let bytes_before = std::fs::read(&path).unwrap();
    let mtime_before = std::fs::metadata(&path).unwrap().modified().unwrap();
    let shot = dir.0.join("shot.png");

    let mut s = Sim::with(Options { file: Some(path.clone()), screenshot: Some(shot.clone()), ..Default::default() });
    // Dirty the document, open an in-progress text edit, and stage an agent's pending document.
    super::unsaved_dot::edit_now(&mut s);
    s.shell.app.set_tool(Tool::Text);
    let spot = s.app().canvas_rect.min + egui::vec2(100.0, 420.0);
    s.mv(spot);
    s.button(true);
    s.button(false);
    s.one_frame().text("draft").send();
    let mut pending_doc = two_rects();
    pending_doc.name = "Agent pending".into();
    s.shell.app.terminal.live_patch.pending_doc = Some(pending_doc);
    assert!(s.app().has_unsaved_edits(), "the document is dirty");
    assert!(s.app().canvas.text.is_some(), "an open edit is in progress");
    assert!(s.app().terminal.live_patch.pending_doc.is_some(), "an agent's pending document is staged");

    let requested = (0..60).any(|_| {
        if s.commands.iter().any(|c| matches!(c, egui::ViewportCommand::Screenshot(_))) {
            return true;
        }
        s.frame(vec![]);
        s.commands.iter().any(|c| matches!(c, egui::ViewportCommand::Screenshot(_)))
    });
    assert!(requested, "screenshot mode must request a capture");
    let image = std::sync::Arc::new(egui::ColorImage::filled([1600, 1088], egui::Color32::BLACK));
    s.frame(vec![Event::Screenshot { viewport_id: egui::ViewportId::ROOT, user_data: egui::UserData::default(), image }]);
    assert!(shot.exists(), "screenshot PNG was written");
    assert!(s.commands.contains(&egui::ViewportCommand::Close), "drive_screenshot requested window close");

    s.close_frame();
    assert!(!s.commands.contains(&egui::ViewportCommand::CancelClose), "close request must not be cancelled");
    assert!(!s.app().leave.is_asking(), "no leave question may be open");
    assert!(s.app().dialog.is_none(), "no dialog may be shown: {:?}", dialog_title(&s));
    assert!(s.app().has_unsaved_edits(), "dirty flag is not cleared");
    assert!(s.app().canvas.text.is_some(), "open edit was not committed by a leave prompt");
    assert!(s.app().terminal.live_patch.pending_doc.is_some(), "pending agent document was not consumed by a leave dialog");
    assert_eq!(std::fs::read(&path).unwrap(), bytes_before, "document file bytes are unchanged");
    assert_eq!(std::fs::metadata(&path).unwrap().modified().unwrap(), mtime_before, "document file mtime is unchanged");
    assert_eq!(s.app().autosave.saves, 0, "no autosave ran");
    let mut files: Vec<String> = std::fs::read_dir(&dir.0).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    files.sort();
    assert_eq!(files, vec!["outside.designos".to_string(), "shot.png".to_string()], "no autosave file exists");

    // A library file opened in screenshot mode also never writes the library or version history on close.
    let lib_root = dir.0.join("lib");
    let seeded = designos_library::Library::open(&lib_root).unwrap();
    let proj = seeded.create_project("Team").unwrap();
    let lib_id = seeded.create_file(&proj.id, None, "Lib", &two_rects()).unwrap().id;
    let lib_nodes_before = seeded.load_file(&lib_id).unwrap().nodes.len();
    let lib_shot = dir.0.join("lib-shot.png");
    let mut ls = Sim::with(Options { open_id: Some(lib_id.clone()), library: Some(lib_root), screenshot: Some(lib_shot), ..Default::default() });
    super::unsaved_dot::edit_now(&mut ls);
    assert_ne!(ls.app().doc.nodes.len(), lib_nodes_before, "library document is dirty in memory");
    ls.close_frame();
    assert!(!ls.commands.contains(&egui::ViewportCommand::CancelClose));
    assert_eq!(seeded.load_file(&lib_id).unwrap().nodes.len(), lib_nodes_before, "library file on disk is unchanged");
    assert!(seeded.list_versions(&lib_id).is_empty(), "no close version was written to the library");
    assert_eq!(ls.app().autosave.saves, 0, "no autosave ran");
}

#[test]
fn normal_run_still_asks_before_closing_a_dirty_document() {
    let (mut s, _d, path) = external_sim_with(&two_rects());
    edit(&mut s);
    let on_disk = nodes_on_disk(&path);
    let ctx = s.ctx.clone();
    assert!(!s.shell.app.may_quit(&ctx), "may_quit returns false for a dirty document in a normal run");
    assert!(s.app().leave.is_asking(), "the leave question is open");
    assert!(asks_about(&s, "Outside"), "{:?}", dialog_title(&s));
    s.close_frame();
    assert!(s.commands.contains(&egui::ViewportCommand::CancelClose), "the close request is cancelled while the question is open");
    assert!(asks_about(&s, "Outside"), "{:?}", dialog_title(&s));
    assert_eq!(nodes_on_disk(&path), on_disk, "nothing was written before an answer");
}
