//! The edges of "Save changes to <name>?": what is still only a draft or a gesture when a question opens, a close
//! request while presenting, an open that arrives while a question is open, and the tab menu and the open paths
//! that the main tests drive directly.

use super::leave_prompt::{add_an_unsaved_untitled_tab, asks_about, click, close_tab_by_click, dialog_title, make_two_tabs, tab_id};
use super::unsaved_dot::{dot, edit, external_sim_with, nodes_on_disk, switch_to, two_rects, wait, SETTLED};
use super::*;
use crate::canvas::tools::Tool;
use crate::canvas::Interaction;
use crate::home::{Answer, Leave};
use crate::io::{DialogOutcome, ScriptedDialogs};
use designos_doc::{Document, DocumentKind, Node, Rect as DRect};
use std::path::Path;

const NOTE: &str = "Talking points the user typed";

/// A document with one frame (so it can be presented) holding a rectangle.
fn frame_doc() -> Document {
    let mut d = Document::new("Outside");
    let page = d.add_page("P");
    let frame = d.alloc_id();
    d.insert(page, Node::frame(frame, DRect::new(0.0, 0.0, 300.0, 200.0))).unwrap();
    let rect = d.alloc_id();
    d.insert(frame, Node::rectangle(rect, DRect::new(10.0, 10.0, 50.0, 50.0))).unwrap();
    d
}

/// A second, clean tab next to the file's tab; the file's tab (index 0) is shown again.
fn with_a_clean_second_tab(s: &mut Sim) {
    let mut doc = two_rects();
    doc.name = "Second".into();
    s.shell.app.open_new_tab("Second", doc, crate::tabs::ProjectTabKind::Design);
    wait(s, SETTLED);
    switch_to(s, 0);
}

fn write_doc(dir: &Path, file: &str, name: &str) -> std::path::PathBuf {
    let path = dir.join(file);
    let mut doc = two_rects();
    doc.name = name.into();
    std::fs::write(&path, doc.to_json().unwrap()).unwrap();
    path
}

/// A frame in which `path` is dropped on the window.
fn drop_file(s: &mut Sim, path: &Path) {
    s.t += 0.05;
    let raw = RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1600.0, 1088.0))),
        time: Some(s.t),
        dropped_files: vec![egui::DroppedFile { path: Some(path.to_path_buf()), ..Default::default() }],
        ..Default::default()
    };
    let ctx = s.ctx.clone();
    let shell = &mut s.shell;
    let _ = ctx.run_ui(raw, |ui| shell.draw(ui));
}

fn dialog_key() -> egui::Id {
    egui::Id::new(("test-rect", "dialog-alt".to_string()))
}

/// Forgets where the question was last drawn, so that [`dialog_drawn`] only sees a draw that comes after this.
fn forget_the_dialog_rect(s: &Sim) {
    s.ctx.data_mut(|d| d.remove::<Rect>(dialog_key()));
}

fn dialog_drawn(s: &Sim) -> bool {
    s.ctx.data(|d| d.get_temp::<Rect>(dialog_key())).is_some()
}

// ------------------------------------------------------------------ speaker notes

/// A deck from the tab bar's `+` menu (a right click; the menu then answers the arrow keys), with its first slide added
/// and saved to `path`: a saved deck outside the library, with nothing unsaved.
fn a_saved_deck_from_the_tab_bar(s: &mut Sim, path: &Path) {
    let plus = s.rect_of("tab-bar-new").center();
    s.right_click(plus);
    for _ in 0..3 {
        s.key(Key::ArrowDown);
    }
    s.key(Key::Enter);
    wait(s, SETTLED);
    assert!(s.app().is_deck(), "the New deck tab is shown");
    click(s, "deck-add");
    wait(s, SETTLED);
    assert!(s.app().active_slide().is_some(), "a slide to take notes");
    s.shell.app.save_with(false, &ScriptedDialogs::answering(DialogOutcome::Picked(path.to_path_buf())));
    wait(s, SETTLED);
    assert!(!s.app().has_unsaved_edits(), "saved");
}

/// Types a note into the speaker notes field and leaves the field alone: it is a draft until "Save notes" or a slide change.
fn type_a_note(s: &mut Sim) {
    s.click(s.rect_of("deck-notes").center());
    s.type_text(NOTE);
    assert_eq!(s.app().decks.notes_draft, NOTE);
    assert!(!s.app().has_unsaved_edits(), "a draft is in no history, so nothing shows it yet");
}

fn a_slide_holds_the_note(doc: &Document) -> bool {
    doc.nodes.values().any(|n| n.slide.as_ref().is_some_and(|slide| slide.notes == NOTE))
}

#[test]
fn speaker_notes_typed_in_a_new_deck_are_asked_about_when_its_tab_closes_and_save_writes_them() {
    let (mut s, d, _path) = external_sim_with(&two_rects());
    let deck_path = d.0.join("deck.designos");
    a_saved_deck_from_the_tab_bar(&mut s, &deck_path);
    type_a_note(&mut s);
    assert_eq!(s.app().tabs.entries.len(), 2);
    let deck_idx = s.app().tabs.active_idx;
    let title = s.app().tabs.entries[deck_idx].title.clone();
    close_tab_by_click(&mut s, deck_idx);
    assert!(asks_about(&s, &title), "{:?}", dialog_title(&s));
    assert_eq!(s.app().tabs.entries.len(), 2, "nothing is closed until it is answered");
    assert!(a_slide_holds_the_note(&s.app().doc), "the draft is in the document, as one undo step");
    let on_disk = Document::from_json(&std::fs::read_to_string(&deck_path).unwrap()).unwrap();
    assert!(!a_slide_holds_the_note(&on_disk), "and not yet in the file");
    s.shell.app.dialog = None;
    s.shell.app.answer_save_changes_with(None, Answer::Save, &ScriptedDialogs::answering(DialogOutcome::Cancelled));
    wait(&mut s, SETTLED);
    let saved = Document::from_json(&std::fs::read_to_string(&deck_path).unwrap()).unwrap();
    assert!(a_slide_holds_the_note(&saved), "Save wrote the note");
    assert_eq!(s.app().tabs.entries.len(), 1, "then the tab closed");
}

#[test]
fn speaker_notes_typed_in_a_new_deck_are_asked_about_when_the_window_closes() {
    let (mut s, d, _path) = external_sim_with(&two_rects());
    a_saved_deck_from_the_tab_bar(&mut s, &d.0.join("deck.designos"));
    type_a_note(&mut s);
    let title = s.app().tabs.entries[s.app().tabs.active_idx].title.clone();
    s.close_frame();
    assert!(asks_about(&s, &title), "{:?}", dialog_title(&s));
    assert!(s.commands.contains(&egui::ViewportCommand::CancelClose), "the window stays open for the question");
    assert!(a_slide_holds_the_note(&s.app().doc));
}

// ------------------------------------------------------------------ presenting

fn present(s: &mut Sim) {
    s.shell.app.start_present();
    wait(s, 0.2);
    assert!(s.app().proto.present.is_some(), "presenting");
}

#[test]
fn a_window_close_while_presenting_leaves_the_present_view_and_asks_over_the_editor() {
    let (mut s, _d, path) = external_sim_with(&frame_doc());
    edit(&mut s);
    let edited = s.app().doc.nodes.len();
    let on_disk = nodes_on_disk(&path);
    present(&mut s);
    s.key(Key::F11);
    assert!(s.app().proto.present.as_ref().is_some_and(|p| p.fullscreen), "the present view took the window full screen");
    forget_the_dialog_rect(&s);
    s.sent_commands.clear();
    s.close_frame();
    assert!(asks_about(&s, "Outside"), "{:?}", dialog_title(&s));
    assert!(s.commands.contains(&egui::ViewportCommand::CancelClose), "the window stays open for the question");
    assert!(s.app().proto.present.is_none(), "the present view is left first");
    assert!(s.sent_commands.contains(&egui::ViewportCommand::Fullscreen(false)), "and the window is as it was before it");
    wait(&mut s, 0.3);
    assert!(dialog_drawn(&s), "the question is drawn over the editor");
    // a second close request changes nothing
    s.close_frame();
    assert!(asks_about(&s, "Outside"));
    // Cancel: back in the editor with the edit
    click(&mut s, "dialog-cancel");
    wait(&mut s, SETTLED);
    assert!(s.app().dialog.is_none());
    assert!(s.app().proto.present.is_none());
    assert_eq!(s.app().doc.nodes.len(), edited, "the edit is still there");
    assert_eq!(nodes_on_disk(&path), on_disk, "nothing was written");
    assert!(dot(&s), "still unsaved");
}

#[test]
fn quitting_while_presenting_a_clean_tab_asks_about_the_other_tabs_over_the_editor() {
    let (mut s, _d, _path) = external_sim_with(&frame_doc());
    add_an_unsaved_untitled_tab(&mut s);
    switch_to(&mut s, 0);
    assert!(!dot(&s), "the shown tab has nothing unsaved");
    present(&mut s);
    forget_the_dialog_rect(&s);
    s.sent_commands.clear();
    s.close_frame();
    assert!(asks_about(&s, "Untitled"), "the other tab is asked about: {:?}", dialog_title(&s));
    assert!(s.app().proto.present.is_none(), "the present view is left first");
    wait(&mut s, 0.3);
    assert!(dialog_drawn(&s), "the question is drawn over the editor");
    assert!(!s.sent_commands.contains(&egui::ViewportCommand::Close), "the quit waits for the answer");
    click(&mut s, "dialog-alt");
    assert!(s.sent_commands.contains(&egui::ViewportCommand::Close), "answered: the close is asked for again");
    s.close_frame();
    assert!(!s.commands.contains(&egui::ViewportCommand::CancelClose), "and goes through");
}

// ------------------------------------------------------------------ a gesture under way

/// A saved vector in point editing, in the first of two tabs: the pointer goes down on the middle of a segment.
fn press_on_a_vector_segment(s: &mut Sim) -> (designos_doc::NodeId, usize) {
    with_a_clean_second_tab(s);
    s.key(Key::P);
    for (x, y) in [(10.0, 60.0), (50.0, 75.0), (10.0, 90.0)] {
        let p = s.screen(x, y);
        s.click(p);
    }
    s.key(Key::Enter);
    let id = *s.app().selection.first().expect("the pen made a vector");
    s.shell.app.save_with(false, &ScriptedDialogs::answering(DialogOutcome::Cancelled));
    wait(s, SETTLED);
    assert!(!s.app().has_unsaved_edits(), "saved");
    s.key(Key::Enter);
    assert!(s.app().canvas.vedit.is_some(), "point editing");
    let before = vertices(s, id);
    let mid = s.screen(30.0, 67.5);
    s.mv(mid);
    s.button(true);
    assert!(matches!(s.app().canvas.inter, Interaction::VectorDrag), "the press is a vector drag");
    (id, before)
}

fn vertices(s: &Sim, id: designos_doc::NodeId) -> usize {
    match &s.app().doc.get(id).unwrap().kind {
        designos_doc::NodeKind::Vector { network: Some(n), .. } => n.vertices.len(),
        _ => 0,
    }
}

#[test]
fn a_press_on_a_vector_segment_that_has_not_moved_adds_no_point_when_the_question_opens() {
    let (mut s, _d, _path) = external_sim_with(&two_rects());
    let (id, before) = press_on_a_vector_segment(&mut s);
    let depth = s.app().hist.undo_depth();
    s.shell.app.commit_open_edits();
    assert!(matches!(s.app().canvas.inter, Interaction::None), "the gesture is over");
    assert_eq!(vertices(&s, id), before, "no point was added");
    assert_eq!(s.app().hist.undo_depth(), depth, "and nothing is in the history");
    assert!(!s.app().has_unsaved_edits(), "so the document is as clean as it was");
    let first = tab_id(&s, 0);
    s.shell.app.request_leave(None, Leave::CloseTab(first));
    assert!(s.app().dialog.is_none(), "nothing to ask about: {:?}", dialog_title(&s));
    assert_eq!(s.app().tabs.entries.len(), 1, "so the tab closed");
}

#[test]
fn a_press_on_an_auto_layout_bar_that_has_not_moved_opens_no_inline_editor_behind_the_question() {
    let (mut s, _d, _path) = external_sim_with(&two_rects());
    with_a_clean_second_tab(&mut s);
    let frame = s.app().doc.get(s.app().page).unwrap().children[0];
    s.shell.app.canvas.inter = Interaction::AutoLayoutDrag(crate::canvas::AutoLayoutDragState {
        tx: crate::edit::Tx::new(),
        id: frame,
        kind: crate::canvas::handles::AutoLayoutHandleKind::Gap(0),
        start_page: designos_doc::Vec2::new(10.0, 10.0),
        start_value: 8.0,
        moved: false,
    });
    s.shell.app.commit_open_edits();
    assert!(matches!(s.app().canvas.inter, Interaction::None), "the gesture is over");
    assert!(s.app().canvas.al_inline.is_none(), "the gap or padding box did not open with focus");
    assert!(!s.app().has_unsaved_edits());
}

#[test]
fn ending_a_viewport_gesture_for_the_question_leaves_the_render_revision_alone_and_a_drag_bumps_it() {
    let (mut s, _d, _path) = external_sim_with(&two_rects());
    with_a_clean_second_tab(&mut s);
    let first = s.app().doc.get(s.app().page).unwrap().children[0];
    let page_pos = designos_doc::Vec2::new(30.0, 30.0);
    let viewport_gestures = [
        ("pan", Interaction::Pan),
        ("marquee", Interaction::Marquee { start: page_pos, cur: page_pos, base: Vec::new() }),
        ("zoom marquee", Interaction::ZoomMarquee { start: page_pos, cur: designos_doc::Vec2::new(230.0, 150.0), alt: false }),
        (
            "a pending click",
            Interaction::Pending(crate::canvas::Pending {
                node: first,
                start_screen: Pos2::new(40.0, 40.0),
                start_page: page_pos,
                shift: false,
                was_selected: true,
            }),
        ),
        ("pen drag", Interaction::PenDrag),
    ];
    for (name, inter) in viewport_gestures {
        s.shell.app.canvas.inter = inter;
        let (rev, zoom, offset, selection) = (s.app().canvas.tiles.rev(), s.app().view.zoom, s.app().view.offset, s.app().selection.clone());
        s.shell.app.commit_open_edits();
        s.frame(vec![]);
        assert!(matches!(s.app().canvas.inter, Interaction::None), "{name}: the gesture is over");
        assert_eq!(s.app().canvas.tiles.rev(), rev, "{name}: nothing in the document changed, so the render revision stays");
        assert_eq!((s.app().view.zoom, s.app().view.offset), (zoom, offset), "{name}: the view stays where it is");
        assert_eq!(s.app().selection, selection, "{name}: so does the selection");
    }
    // a drag that really moved a layer does change the document
    let from = s.screen(25.0, 25.0);
    s.mv(from);
    s.button(true);
    s.mv(s.screen(60.0, 60.0));
    s.mv(s.screen(90.0, 90.0));
    let rev = s.app().canvas.tiles.rev();
    s.shell.app.commit_open_edits();
    s.frame(vec![]);
    assert_ne!(s.app().canvas.tiles.rev(), rev, "a committed drag changes the document and the revision");
}

#[test]
fn a_dragged_sticky_has_no_text_session_open_when_the_question_opens() {
    let doc = crate::board::create::new_document("Board");
    let (mut s, _d, _path) = external_sim_with(&doc);
    assert_eq!(s.app().doc.kind, DocumentKind::Board);
    s.shell.app.set_tool(Tool::Sticky);
    let from = s.app().canvas_rect.min + egui::vec2(200.0, 300.0);
    s.mv(from);
    s.button(true);
    s.mv(from + egui::vec2(60.0, 40.0));
    s.mv(from + egui::vec2(140.0, 100.0));
    assert!(matches!(s.app().canvas.inter, Interaction::Draw(_)), "the sticky is being drawn");
    s.shell.app.commit_open_edits();
    assert!(s.app().canvas.text.is_none(), "the sticky's text session is not left open behind the question");
    assert!(s.app().has_unsaved_edits(), "the sticky is an edit");
}

// ------------------------------------------------------------------ an open while a question is open

/// A quit question about the shown tab of two, with a second document ready to be opened.
fn a_quit_question_with_a_file_to_open(s: &mut Sim, dir: &Path) -> std::path::PathBuf {
    with_a_clean_second_tab(s);
    edit(s);
    s.close_frame();
    assert!(asks_about(s, "Outside"), "{:?}", dialog_title(s));
    write_doc(dir, "dropped.designos", "Dropped")
}

fn assert_the_quit_question_is_still_pending(s: &mut Sim) {
    assert!(asks_about(s, "Outside"), "the same question: {:?}", dialog_title(s));
    assert_eq!(s.app().doc.name, "Outside", "no document was opened");
    assert_eq!(s.app().tabs.entries.len(), 2);
    s.sent_commands.clear();
    click(s, "dialog-alt");
    assert!(s.sent_commands.contains(&egui::ViewportCommand::Close), "the quit is still the action, so the window closes once it is answered");
    assert!(s.app().leave.quit_approved);
    assert_eq!(s.app().doc.name, "Outside", "and the dropped file was never opened");
}

#[test]
fn an_open_request_while_a_question_is_open_is_ignored_and_the_pending_action_stays() {
    let (mut s, d, _path) = external_sim_with(&two_rects());
    let other = a_quit_question_with_a_file_to_open(&mut s, &d.0);
    s.shell.app.open_path(other);
    assert_the_quit_question_is_still_pending(&mut s);
}

#[test]
fn another_leave_request_while_a_question_is_open_is_ignored_and_the_pending_action_stays() {
    let (mut s, d, _path) = external_sim_with(&two_rects());
    let _other = a_quit_question_with_a_file_to_open(&mut s, &d.0);
    let id = tab_id(&s, 1);
    s.shell.app.request_leave(None, Leave::CloseTab(id));
    s.shell.app.request_leave(None, Leave::CloseOthers(id));
    assert_the_quit_question_is_still_pending(&mut s);
}

#[test]
fn a_file_dropped_while_a_question_is_open_is_ignored_and_the_pending_action_stays() {
    let (mut s, d, _path) = external_sim_with(&two_rects());
    let other = a_quit_question_with_a_file_to_open(&mut s, &d.0);
    // an SVG would be placed on the canvas behind the question
    let svg = d.0.join("icon.svg");
    std::fs::write(&svg, r##"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20"><rect width="20" height="20" fill="#ff0000"/></svg>"##).unwrap();
    let nodes = s.count();
    drop_file(&mut s, &svg);
    wait(&mut s, 0.1);
    assert_eq!(s.count(), nodes, "the SVG was not placed");
    drop_file(&mut s, &other);
    wait(&mut s, 0.1);
    assert_the_quit_question_is_still_pending(&mut s);
}

// ------------------------------------------------------------------ the tab menu, and the dot after an open

fn pick_in_the_tab_menu(s: &mut Sim, tab_id: u64, row: usize) {
    let tab = s.rect_of(&format!("project-tab-id-{tab_id}")).center();
    s.right_click(tab);
    // the first arrow key highlights the first row
    for _ in 0..=row {
        s.key(Key::ArrowDown);
    }
    s.key(Key::Enter);
}

#[test]
fn the_tab_menus_close_other_tabs_asks_for_each_unsaved_tab_and_closes_nothing_before_the_answer() {
    let (mut s, _d, path) = external_sim_with(&two_rects());
    make_two_tabs(&mut s);
    let on_disk = nodes_on_disk(&path);
    let keep = tab_id(&s, 1);
    pick_in_the_tab_menu(&mut s, keep, 4);
    assert!(asks_about(&s, "Outside"), "the file's tab has edits no save has written: {:?}", dialog_title(&s));
    assert_eq!(s.app().tabs.entries.len(), 2, "nothing is closed until it is answered");
    click(&mut s, "dialog-alt");
    wait(&mut s, SETTLED);
    assert_eq!(s.app().tabs.entries.len(), 1, "Don't save: the other tab closed");
    assert_eq!(s.app().tabs.entries[0].id, keep);
    assert_eq!(nodes_on_disk(&path), on_disk, "the file is as it was");
}

#[test]
fn opening_a_file_over_a_flagged_document_leaves_the_new_one_without_a_dot() {
    // A flagged document is one that a writer outside the undo history changed (an agent that replaced it, Keep mine):
    // the flag is the whole reason for its dot. The file that replaces it is not that document.
    let (mut s, d, _path) = external_sim_with(&two_rects());
    with_a_clean_second_tab(&mut s);
    s.shell.app.mark_edited_outside_history();
    wait(&mut s, SETTLED);
    assert!(dot(&s), "the flag alone lights the dot");
    let other = write_doc(&d.0, "other.designos", "Other");
    s.shell.app.open_path(other.clone());
    wait(&mut s, 0.1);
    assert!(asks_about(&s, "Outside"), "{:?}", dialog_title(&s));
    click(&mut s, "dialog-alt");
    wait(&mut s, SETTLED);
    assert_eq!(s.app().doc.name, "Other", "the file is on screen");
    assert_eq!(s.app().file_path.as_deref(), Some(other.as_path()));
    assert!(!s.app().has_unsaved_edits(), "and nothing of the flagged document is left");
    assert!(!dot(&s), "so the tab shows no dot");
}
