//! Each tab keeps its own view of its file on disk: what it last saved and when the file last changed.
//! A tab switch is never taken for an outside write, and a file that did change under unsaved edits is
//! never loaded over them without asking.

use super::unsaved_dot::{dot, edit, external_sim_with, lib, library_sim, nodes_on_disk, switch_to, two_rects, wait, SETTLED};
use super::*;
use crate::home::Act;
use designos_doc::Document;
use std::path::Path;
use std::time::Duration;

const CONFLICT_TITLE: &str = "File changed on disk";

/// Goes Home and opens the library's first file in a tab of its own, leaving the tab of `path` parked.
/// Returns the index of the tab of `path`. (Takes the sim by reference: a `Sim` is large, and a test thread's
/// stack holds only a few copies of it.)
fn visit_a_library_tab(s: &mut Sim, path: &Path) -> usize {
    s.shell.app.go_home();
    wait(s, SETTLED);
    let id = s.app().home.as_ref().unwrap().snap.files.iter().find(|f| f.name == "DESIGN-OS Studio Starter").unwrap().id.clone();
    crate::home::apply(&mut s.shell.app, Act::Open(id));
    wait(s, 2.0);
    s.app().tabs.entries.iter().position(|t| t.file_path.as_deref() == Some(path)).expect("a tab for the file")
}

/// Writes `doc` over the file with a later modification time than the one the app recorded.
fn rewrite_on_disk(path: &Path, doc: &Document) {
    std::thread::sleep(Duration::from_millis(30));
    std::fs::write(path, doc.to_json().unwrap()).unwrap();
}

fn other_document() -> Document {
    let mut d = Document::new("Written by another program");
    let page = d.add_page("P");
    for i in 0..7 {
        let id = d.alloc_id();
        d.insert(page, designos_doc::Node::rectangle(id, designos_doc::Rect::new(i as f32 * 60.0, 0.0, 50.0, 50.0))).unwrap();
    }
    d
}

fn conflict_open(s: &Sim) -> bool {
    s.app().dialog.as_ref().is_some_and(|d| d.title == CONFLICT_TITLE)
}

fn synced_toast(s: &Sim) -> bool {
    s.app().toast.as_ref().is_some_and(|t| t.text.starts_with("Synced external agent changes"))
}

fn click(s: &mut Sim, name: &str) {
    let r = s.rect_of(name);
    s.click(r.center());
}

#[test]
fn unsaved_edits_outside_the_library_survive_a_round_trip_through_a_library_tab() {
    let (mut s, _d, path) = external_sim_with(&two_rects());
    edit(&mut s);
    let edited = s.app().doc.nodes.len();
    let file_tab = visit_a_library_tab(&mut s, &path);
    let on_disk = nodes_on_disk(&path);
    switch_to(&mut s, file_tab);
    wait(&mut s, 2.0);
    assert_eq!(s.app().doc.nodes.len(), edited, "the edit is still on screen");
    assert!(!synced_toast(&s), "nothing was read from the file: {:?}", s.app().toast.as_ref().map(|t| &t.text));
    assert!(!conflict_open(&s), "the file did not change");
    assert!(dot(&s), "and it is still unsaved");
    assert_eq!(nodes_on_disk(&path), on_disk, "the file was not touched");
}

#[test]
fn switching_back_to_an_unedited_library_tab_does_not_rewrite_its_file() {
    let (mut s, _d) = library_sim();
    let a = s.app().lib_file.as_ref().unwrap().clone();
    let b = lib(&s).create_file(&a.project_id, None, "Second", &two_rects()).unwrap();
    s.shell.app.go_home();
    wait(&mut s, SETTLED);
    crate::home::apply(&mut s.shell.app, Act::Open(b.id.clone()));
    wait(&mut s, 3.0);
    let first = s.app().tabs.entries.iter().position(|t| t.lib_file.as_ref().is_some_and(|f| f.id == a.id)).unwrap();
    let (saves, modified) = (s.app().autosave.saves, lib(&s).file(&a.id).unwrap().modified);
    let mut tabs = std::mem::take(&mut s.shell.app.tabs);
    tabs.switch_to(first, &mut s.shell.app);
    s.shell.app.tabs = tabs;
    let end = s.t + 3.0;
    while s.t < end {
        s.frame(vec![]);
        assert!(!dot(&s), "an unedited file shows no dot, at {:.2}s after the switch", s.t);
    }
    assert_eq!(s.app().autosave.saves, saves, "nothing was written");
    assert_eq!(lib(&s).file(&a.id).unwrap().modified, modified, "the file was not rewritten");
}

#[test]
fn a_file_that_changed_under_unsaved_edits_asks_which_version_to_keep() {
    let (mut s, _d, path) = external_sim_with(&two_rects());
    edit(&mut s);
    let edited = s.app().doc.nodes.len();
    rewrite_on_disk(&path, &other_document());
    wait(&mut s, 1.5);
    assert!(conflict_open(&s), "the app asks");
    assert_eq!(s.app().doc.nodes.len(), edited, "while the edit stays on screen");
    assert!(!synced_toast(&s));
    // dismissing keeps the edit and does not ask again for the same write
    s.key(Key::Escape);
    wait(&mut s, 3.0);
    assert!(!conflict_open(&s), "asked once per write");
    assert_eq!(s.app().doc.nodes.len(), edited);
    assert!(dot(&s), "the edit is still unsaved");
    // saving writes the edit over the file and settles the question
    s.cmd_key(Key::S);
    wait(&mut s, SETTLED);
    assert_eq!(nodes_on_disk(&path), edited);
    assert!(!dot(&s));
}

#[test]
fn use_file_replaces_the_edits_with_what_is_on_disk() {
    let (mut s, _d, path) = external_sim_with(&two_rects());
    edit(&mut s);
    rewrite_on_disk(&path, &other_document());
    wait(&mut s, 1.5);
    assert!(conflict_open(&s));
    click(&mut s, "dialog-ok");
    wait(&mut s, SETTLED);
    assert!(!conflict_open(&s));
    assert_eq!(s.app().doc.name, "Written by another program", "the file's document is on screen");
    assert_eq!(s.app().doc.nodes.len(), other_document().nodes.len());
    assert!(!dot(&s), "it is the file");
    assert_eq!(s.app().hist.undo_depth(), 0, "no undo step reaches back into the discarded document");
}

#[test]
fn keep_mine_by_button_and_enter_leaves_the_edit_alone() {
    for by_enter in [false, true] {
        let (mut s, _d, path) = external_sim_with(&two_rects());
        edit(&mut s);
        let edited = s.app().doc.nodes.len();
        rewrite_on_disk(&path, &other_document());
        wait(&mut s, 1.5);
        assert!(conflict_open(&s));
        if by_enter {
            // Enter must not be the destructive choice
            s.key(Key::Enter);
        } else {
            click(&mut s, "dialog-cancel");
        }
        wait(&mut s, 2.0);
        assert!(!conflict_open(&s));
        assert_eq!(s.app().doc.nodes.len(), edited, "the edit is kept (enter: {by_enter})");
        assert!(dot(&s));
    }
}

#[test]
fn a_file_that_changed_while_its_tab_was_away_asks_when_the_tab_comes_back() {
    let (mut s, _d, path) = external_sim_with(&two_rects());
    edit(&mut s);
    let edited = s.app().doc.nodes.len();
    let file_tab = visit_a_library_tab(&mut s, &path);
    rewrite_on_disk(&path, &other_document());
    wait(&mut s, 1.0);
    assert!(!conflict_open(&s), "the file's tab is not the one on screen");
    switch_to(&mut s, file_tab);
    wait(&mut s, 1.5);
    assert!(conflict_open(&s), "the file changed while the tab was away");
    assert_eq!(s.app().doc.nodes.len(), edited);
}

#[test]
fn a_clean_document_still_follows_the_file_on_disk() {
    let (mut s, _d, path) = external_sim_with(&two_rects());
    rewrite_on_disk(&path, &other_document());
    wait(&mut s, 1.5);
    assert!(!conflict_open(&s), "nothing unsaved, nothing to ask");
    assert_eq!(s.app().doc.name, "Written by another program");
    assert!(synced_toast(&s));
    assert!(!dot(&s));
}

#[test]
fn rewriting_the_same_bytes_under_unsaved_edits_asks_nothing() {
    let (mut s, _d, path) = external_sim_with(&two_rects());
    let original = std::fs::read(&path).unwrap();
    edit(&mut s);
    let edited = s.app().doc.nodes.len();
    std::thread::sleep(Duration::from_millis(30));
    std::fs::write(&path, original).unwrap();
    wait(&mut s, 1.5);
    assert!(!conflict_open(&s), "the file holds what it held: no conflict");
    assert_eq!(s.app().doc.nodes.len(), edited);
    assert!(dot(&s));
}

#[test]
fn an_open_question_asks_for_no_repaint() {
    let (mut s, _d, path) = external_sim_with(&two_rects());
    edit(&mut s);
    // Opening a file and editing make the canvas draw its tiles again, a few per frame, and a runner with few cores draws
    // them more slowly than a laptop: the question must open on a window that is idle (see `idle_repaint`).
    s.settle().expect("a window finishes the work it has after opening a file");
    rewrite_on_disk(&path, &other_document());
    wait(&mut s, 1.5);
    assert!(conflict_open(&s));
    // The disk watcher looks at the file every half second of real time, so it reports the outside write once, at a moment
    // the test does not choose: a wake, not a repaint loop. The check waits for the window to go quiet (see `idle_repaint`).
    s.assert_idle_window_asks_for_no_repaint("the question open");
}

#[test]
fn opening_a_second_file_does_not_read_it_again_as_an_outside_write() {
    let (mut s, d, _path) = external_sim_with(&two_rects());
    let second = d.0.join("second.designos");
    std::fs::write(&second, other_document().to_json().unwrap()).unwrap();
    s.shell.app.open_path(second.clone());
    wait(&mut s, 3.0);
    assert_eq!(s.app().doc.name, "Written by another program");
    assert!(!synced_toast(&s), "nothing wrote the file: {:?}", s.app().toast.as_ref().map(|t| &t.text));
}

#[test]
fn reopening_an_open_library_file_from_home_does_not_rewrite_it() {
    let (mut s, _d) = library_sim();
    let a = s.app().lib_file.as_ref().unwrap().clone();
    wait(&mut s, 3.0);
    let (saves, modified) = (s.app().autosave.saves, lib(&s).file(&a.id).unwrap().modified);
    s.shell.app.go_home();
    wait(&mut s, SETTLED);
    crate::home::apply(&mut s.shell.app, Act::Open(a.id.clone()));
    let end = s.t + 3.0;
    while s.t < end {
        s.frame(vec![]);
        assert!(!dot(&s), "an unedited file shows no dot, at {:.2}s", s.t);
    }
    assert_eq!(s.app().tabs.entries.len(), 1, "the same file keeps its tab");
    assert_eq!(s.app().autosave.saves, saves, "nothing was written");
    assert_eq!(lib(&s).file(&a.id).unwrap().modified, modified);
}

#[test]
fn a_library_tab_whose_file_changed_while_it_was_away_follows_the_file() {
    let (mut s, _d) = library_sim();
    let a = s.app().lib_file.as_ref().unwrap().clone();
    let b = lib(&s).create_file(&a.project_id, None, "Second", &two_rects()).unwrap();
    s.shell.app.go_home();
    wait(&mut s, SETTLED);
    crate::home::apply(&mut s.shell.app, Act::Open(b.id.clone()));
    wait(&mut s, 2.0);
    // an agent writes File A's file on disk while its tab is parked
    let on_disk = lib(&s).load_file(&a.id).unwrap();
    let mut changed = on_disk.clone();
    let page = changed.pages[0];
    let id = changed.alloc_id();
    changed.insert(page, designos_doc::Node::rectangle(id, designos_doc::Rect::new(0.0, 0.0, 33.0, 33.0))).unwrap();
    let path = lib(&s).path("projects").join(&a.project_id).join("files").join(format!("{}.designos", a.id));
    std::thread::sleep(Duration::from_millis(30));
    designos_library::write_atomic(&path, changed.to_json().unwrap().as_bytes()).unwrap();
    let first = s.app().tabs.entries.iter().position(|t| t.lib_file.as_ref().is_some_and(|f| f.id == a.id)).unwrap();
    switch_to(&mut s, first);
    wait(&mut s, 1.5);
    assert!(synced_toast(&s), "the file changed while the tab was away: {:?}", s.app().toast.as_ref().map(|t| &t.text));
    assert!(!conflict_open(&s), "a library file has autosave, so nothing is asked");
    assert!(!dot(&s));
}

fn click_tab(s: &mut Sim, idx: usize) {
    let id = s.app().tabs.entries[idx].id;
    let r = s.rect_of(&format!("project-tab-id-{id}"));
    assert!(r.is_finite(), "tab {idx} is painted");
    s.click(r.center());
    wait(s, 0.1);
}

fn text_of(doc: &Document, id: designos_doc::NodeId) -> String {
    doc.get(id).and_then(|n| n.kind.any_text()).map(|t| t.content.clone()).unwrap_or_else(|| "<none>".into())
}

#[test]
fn a_write_of_exactly_the_document_on_screen_asks_nothing_and_clears_the_dot() {
    // another writer (an agent saving the edits it made live, a second window) writes what is already on screen
    let (mut s, _d, path) = external_sim_with(&two_rects());
    edit(&mut s);
    let depth = s.app().hist.undo_depth();
    let on_screen = s.app().doc.clone();
    rewrite_on_disk(&path, &on_screen);
    wait(&mut s, 1.5);
    assert!(!conflict_open(&s), "the file holds the document on screen: nothing to choose");
    assert!(!dot(&s), "the file is the document, so it is saved");
    assert_eq!(s.app().hist.undo_depth(), depth, "the undo history is intact");
    assert_eq!(s.app().doc.nodes.len(), on_screen.nodes.len());
    // and the next edit is unsaved again, against this file
    edit(&mut s);
    assert!(dot(&s));
}

#[test]
fn an_agent_that_saves_its_own_live_edit_over_the_open_file_asks_nothing_and_keeps_its_undo_steps() {
    let (mut s, d, path) = external_sim_with(&two_rects());
    let session_dir = s.app().terminal.session_dir.clone();
    {
        let app = &mut s.shell.app;
        let (doc, page, sel) = (app.doc.clone(), app.page, app.selection.clone());
        app.terminal.sync_live_file_context(&doc, page, &sel, Some(&path));
        app.terminal.flush_live_sync();
    }
    let mut session = designos_mcp::server::Session::with_session_dir(&d.0, &session_dir).expect("an mcp session");
    let before = s.app().doc.nodes.len();
    session
        .call("designos_create_node", serde_json::json!({ "kind": "rectangle", "name": "Agent layer", "width": 20.0, "height": 20.0 }))
        .expect("the agent creates a layer");
    let actions = s.shell.app.terminal.poll_live_agent_bridge();
    crate::shell::execute_terminal_actions(&mut s.shell.app, actions);
    wait(&mut s, SETTLED);
    assert_eq!(s.app().doc.nodes.len(), before + 1, "the live edit is on screen");
    assert!(dot(&s), "and unsaved");
    let depth = s.app().hist.undo_depth();
    assert!(depth >= 1, "as an undo step the turn revert can use");
    std::thread::sleep(Duration::from_millis(30));
    session.call("designos_save_file", serde_json::json!({ "path": "outside.designos", "overwrite": true })).expect("the agent saves the file");
    wait(&mut s, 1.5);
    assert_eq!(nodes_on_disk(&path), before + 1, "the file holds the agent's edit");
    assert!(!conflict_open(&s), "the file is the document on screen: no question");
    assert!(!dot(&s), "so it is saved");
    assert_eq!(s.app().hist.undo_depth(), depth, "the agent's undo steps are kept");
    s.shell.app.terminal.shutdown();
}

#[test]
fn a_tab_click_with_an_open_text_session_puts_the_typing_in_the_history_and_protects_it() {
    let mut doc = two_rects();
    let page = doc.pages[0];
    let text_id = doc.alloc_id();
    doc.insert(page, designos_doc::Node::text(text_id, designos_doc::Rect::new(0.0, 100.0, 200.0, 30.0), "Original", designos_doc::TextStyle::default()))
        .unwrap();
    let (mut s, _d, path) = external_sim_with(&doc);
    let file_tab = visit_a_library_tab(&mut s, &path);
    click_tab(&mut s, file_tab);
    wait(&mut s, SETTLED);
    crate::canvas::textsession::begin_text_edit(&mut s.shell.app, text_id);
    crate::canvas::textsession::insert_text(&mut s.shell.app, "Typed by the user", crate::textedit::EditKind::Typing);
    s.frame(vec![]);
    assert!(s.app().canvas.text.is_some(), "a text session is open");
    assert!(!dot(&s), "the typing is not in the history yet");
    // a real click on the other tab
    click_tab(&mut s, 1 - file_tab);
    wait(&mut s, SETTLED);
    let parked = &s.app().tabs.entries[file_tab];
    assert!(text_of(&parked.doc, text_id).contains("Typed by the user"), "the typing is in the parked document");
    assert!(!parked.hist.is_clean(), "and in its history: an undo step");
    assert!(super::unsaved_dot::dot_on_tab(&s, |t| t.file_path.as_deref() == Some(path.as_path())), "so the parked tab shows the dot");
    // another program rewrites the file while the tab is away; coming back asks instead of replacing the typing
    rewrite_on_disk(&path, &other_document());
    wait(&mut s, 1.0);
    click_tab(&mut s, file_tab);
    wait(&mut s, 1.5);
    assert!(conflict_open(&s), "the typing is unsaved, so the changed file is asked about");
    assert!(text_of(&s.app().doc, text_id).contains("Typed by the user"), "and the typing is still on screen");
}

#[test]
fn after_keep_mine_a_round_trip_does_not_ask_about_the_same_write_again() {
    let (mut s, _d, path) = external_sim_with(&two_rects());
    edit(&mut s);
    let file_tab = visit_a_library_tab(&mut s, &path);
    click_tab(&mut s, file_tab);
    wait(&mut s, SETTLED);
    rewrite_on_disk(&path, &other_document());
    wait(&mut s, 1.5);
    assert!(conflict_open(&s));
    s.key(Key::Escape);
    wait(&mut s, 2.0);
    assert!(!conflict_open(&s));
    click_tab(&mut s, 1 - file_tab);
    wait(&mut s, 1.0);
    click_tab(&mut s, file_tab);
    wait(&mut s, 2.0);
    assert!(!conflict_open(&s), "the write was answered: it is not asked about again");
    assert!(dot(&s), "the edit is still unsaved");
    // a new write is a new question
    let mut third = other_document();
    third.name = "A third version".into();
    rewrite_on_disk(&path, &third);
    wait(&mut s, 1.5);
    assert!(conflict_open(&s), "another write asks again");
}

#[test]
fn after_keep_mine_an_undo_back_to_the_opened_version_does_not_load_the_rejected_file() {
    let (mut s, _d, path) = external_sim_with(&two_rects());
    edit(&mut s);
    rewrite_on_disk(&path, &other_document());
    wait(&mut s, 1.5);
    assert!(conflict_open(&s));
    s.key(Key::Escape);
    wait(&mut s, 1.0);
    // Cmd+Z back to the version that was opened: the history says it is the saved one, but the file is not that any more
    s.cmd_key(Key::Z);
    wait(&mut s, 2.5);
    assert_eq!(s.app().doc.name, "Outside", "the document stays the user's");
    assert!(!synced_toast(&s), "the rejected file was not installed: {:?}", s.app().toast.as_ref().map(|t| &t.text));
    assert!(dot(&s), "it is still not what the file holds, so the dot stays lit");
    assert!(!conflict_open(&s), "and the write already answered is not asked about again");
    // redo still works, and Cmd+S writes the user's version over the file
    s.cmd_key(Key::S);
    wait(&mut s, SETTLED);
    let on_disk = Document::from_json(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(on_disk.name, "Outside", "the user's version is on disk");
    assert!(!dot(&s), "saved");
}
