//! Each tab keeps its own view of its file on disk: what it last saved and when the file last changed.
//! A tab switch is never taken for an outside write, and a file that did change under unsaved edits is
//! never loaded over them without asking.

use super::unsaved_dot::{dot, edit, external_sim_with, lib, library_sim, nodes_on_disk, switch_to, two_rects, wait, SETTLED};
use super::*;
use crate::home::Act;
use designos_doc::Document;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

const CONFLICT_TITLE: &str = "File changed on disk";

/// Goes Home and opens the library's first file in a tab of its own, leaving the tab of `path` parked.
/// Returns the index of the tab of `path`. (Takes the sim by reference: a `Sim` is large, and a test thread's
/// stack holds only a few copies of it.)
fn visit_a_library_tab(s: &mut Sim, path: &Path) -> usize {
    s.shell.app.go_home();
    wait(s, SETTLED);
    let id = s.app().home.as_ref().unwrap().snap.files.iter().find(|f| f.name == "Welcome").unwrap().id.clone();
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
fn leaving_the_tab_withdraws_the_question_and_coming_back_asks_again() {
    let (mut s, _d, path) = external_sim_with(&two_rects());
    edit(&mut s);
    let file_tab = visit_a_library_tab(&mut s, &path);
    switch_to(&mut s, file_tab);
    rewrite_on_disk(&path, &other_document());
    wait(&mut s, 1.5);
    assert!(conflict_open(&s));
    let other = 1 - file_tab;
    let mut tabs = std::mem::take(&mut s.shell.app.tabs);
    tabs.switch_to(other, &mut s.shell.app);
    s.shell.app.tabs = tabs;
    wait(&mut s, 1.0);
    assert!(!conflict_open(&s), "the question was about the other document");
    switch_to(&mut s, file_tab);
    wait(&mut s, 1.5);
    assert!(conflict_open(&s), "still unanswered");
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
    rewrite_on_disk(&path, &other_document());
    wait(&mut s, 1.5);
    assert!(conflict_open(&s));
    let log = Arc::new(Mutex::new(Vec::new()));
    let log_in_callback = log.clone();
    s.ctx.set_request_repaint_callback(move |info| log_in_callback.lock().unwrap().push(info.delay));
    for _ in 0..40 {
        s.frame(vec![]);
    }
    let requests = log.lock().unwrap().clone();
    assert!(!requests.contains(&Duration::ZERO), "an idle window with the question open asks for no immediate repaint: {requests:?}");
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
