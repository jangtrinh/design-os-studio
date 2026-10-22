//! The unsaved dot on a tab: it follows what the last save (or the open) wrote, not the undo stack.
//! A save that did not happen (cancelled dialog, no dialog, write error) keeps the dot.
//!
//! The dot is current once the last change settled: every test waits [`SETTLED`] after an edit.

use super::*;
use crate::home::Act;
use crate::io::{DialogOutcome, ScriptedDialogs};
use designos_doc::{Document, Node, Rect as DRect};
use designos_library::Library;
use std::path::{Path, PathBuf};

struct TmpDir(PathBuf);

impl TmpDir {
    fn new() -> Self {
        static N: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let n = N.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let p = std::env::temp_dir().join(format!("designos_unsaved_dot_{}_{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        Self(p)
    }
}

impl Drop for TmpDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A document with two rectangles on its first page.
fn two_rects() -> Document {
    let mut d = Document::new("Outside");
    let page = d.add_page("P");
    for i in 0..2 {
        let id = d.alloc_id();
        d.insert(page, Node::rectangle(id, DRect::new(i as f32 * 100.0, 0.0, 50.0, 50.0))).unwrap();
    }
    d
}

fn write_designos(path: &Path) {
    std::fs::write(path, two_rects().to_json().unwrap()).unwrap();
}

/// A sim on the library's seeded "Welcome" file, tab bar shown.
fn library_sim() -> (Sim, TmpDir) {
    let dir = TmpDir::new();
    let mut s = Sim::with(Options { home: true, library: Some(dir.0.join("lib")), ..Default::default() });
    s.shell.app.tabs.always_visible = true;
    let id = {
        let home = s.app().home.as_ref().unwrap();
        home.snap.files.iter().find(|f| f.name == "Welcome").unwrap().id.clone()
    };
    crate::home::apply(&mut s.shell.app, Act::Open(id));
    wait(&mut s, SETTLED);
    (s, dir)
}

/// A sim on a `.designos` file outside the library.
fn external_sim() -> (Sim, TmpDir, PathBuf) {
    let dir = TmpDir::new();
    let path = dir.0.join("outside.designos");
    write_designos(&path);
    let mut s = Sim::with(Options { file: Some(path.clone()), library: Some(dir.0.join("lib")), ..Default::default() });
    s.shell.app.tabs.always_visible = true;
    wait(&mut s, SETTLED);
    (s, dir, path)
}

/// Longer than the settle delay of a changed revision.
const SETTLED: f64 = 0.5;

fn wait(s: &mut Sim, secs: f64) {
    let end = s.t + secs;
    while s.t < end {
        s.frame(vec![]);
    }
}

/// Whether the active tab was painted with the unsaved dot in the last frame.
fn dot(s: &Sim) -> bool {
    let id = s.app().tabs.entries[s.app().tabs.active_idx].id;
    s.rect_of(&format!("tab-unsaved-dot-{id}")).is_finite()
}

/// One undoable edit: duplicates the first layer on the page, then waits for the dot to settle.
fn edit(s: &mut Sim) {
    let app = &mut s.shell.app;
    let first = app.doc.get(app.page).unwrap().children[0];
    app.selection = vec![first];
    app.duplicate_selection();
    wait(s, SETTLED);
}

fn lib(s: &Sim) -> Library {
    s.app().lib.clone().expect("a library")
}

#[test]
fn a_fresh_document_shows_no_dot_and_an_edit_shows_it() {
    let (mut s, _d, _path) = external_sim();
    assert!(!dot(&s), "nothing edited yet");
    edit(&mut s);
    assert!(dot(&s), "an edit no save has written");
}

#[test]
fn ctrl_s_clears_the_dot_on_a_library_file() {
    let (mut s, _d) = library_sim();
    let id = s.app().lib_file.as_ref().unwrap().id.clone();
    edit(&mut s);
    assert!(dot(&s), "the edit is waiting for autosave");
    let nodes = s.app().doc.nodes.len();
    s.cmd_key(Key::S);
    wait(&mut s, SETTLED);
    assert_eq!(lib(&s).load_file(&id).unwrap().nodes.len(), nodes, "Ctrl+S wrote the edit");
    assert!(!dot(&s), "the file on disk is the document: no dot");
}

#[test]
fn autosave_clears_the_dot_on_a_library_file() {
    let (mut s, _d) = library_sim();
    edit(&mut s);
    assert!(dot(&s));
    wait(&mut s, 2.5);
    assert_eq!(s.app().autosave.saves, 1, "autosave ran");
    assert!(!dot(&s), "autosave wrote the edit: no dot");
}

#[test]
fn ctrl_s_clears_the_dot_on_a_designos_file_outside_the_library() {
    let (mut s, _d, path) = external_sim();
    edit(&mut s);
    assert!(dot(&s));
    s.cmd_key(Key::S);
    wait(&mut s, SETTLED);
    let on_disk = Document::from_json(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(on_disk.nodes.len(), s.app().doc.nodes.len(), "Ctrl+S wrote the edit");
    assert!(!dot(&s), "the file on disk is the document: no dot");
}

#[test]
fn saving_an_imported_document_with_a_picked_path_clears_the_dot() {
    let dir = TmpDir::new();
    let mut s = Sim::with(Options { library: Some(dir.0.join("lib")), ..Default::default() });
    s.shell.app.tabs.always_visible = true;
    // an imported `.fig` keeps its path, but Save asks where the `.designos` copy goes
    s.shell.app.replace_document(two_rects(), Some(dir.0.join("imported.fig")));
    wait(&mut s, SETTLED);
    edit(&mut s);
    assert!(dot(&s));
    let target = dir.0.join("imported.designos");
    s.shell.app.save_with(false, &ScriptedDialogs::answering(DialogOutcome::Picked(target.clone())));
    wait(&mut s, SETTLED);
    assert!(target.is_file(), "the picked path was written");
    assert!(!dot(&s), "saved: no dot");
}

#[test]
fn a_save_that_did_not_happen_keeps_the_dot_and_says_why() {
    let dir = TmpDir::new();
    let mut s = Sim::with(Options { library: Some(dir.0.join("lib")), ..Default::default() });
    s.shell.app.tabs.always_visible = true;
    s.shell.app.replace_document(two_rects(), None);
    wait(&mut s, SETTLED);
    edit(&mut s);
    assert!(dot(&s));

    // no dialog can open (a Linux box without zenity or a desktop portal)
    s.shell.app.save_with(false, &ScriptedDialogs::answering(DialogOutcome::Unavailable));
    wait(&mut s, SETTLED);
    assert!(dot(&s), "nothing was written");
    let toast = s.app().toast.as_ref().expect("a toast says why");
    assert!(toast.error && toast.text.starts_with("No file dialog"), "{}", toast.text);

    // the dialog was cancelled
    s.shell.app.save_with(false, &ScriptedDialogs::answering(DialogOutcome::Cancelled));
    wait(&mut s, SETTLED);
    assert!(dot(&s), "nothing was written");

    // the write failed: the picked path sits under a file, which no save can create
    std::fs::write(dir.0.join("blocker"), b"x").unwrap();
    let blocked = dir.0.join("blocker").join("x.designos");
    s.shell.app.save_with(false, &ScriptedDialogs::answering(DialogOutcome::Picked(blocked.clone())));
    wait(&mut s, SETTLED);
    assert!(!blocked.exists());
    assert!(s.app().toast.as_ref().is_some_and(|t| t.error && t.text.starts_with("Cannot write")), "the failure is shown");
    assert!(dot(&s), "the write failed");
}

#[test]
fn editing_a_saved_document_again_brings_the_dot_back() {
    let (mut s, _d, _path) = external_sim();
    edit(&mut s);
    s.cmd_key(Key::S);
    wait(&mut s, SETTLED);
    assert!(!dot(&s));
    edit(&mut s);
    assert!(dot(&s), "an edit after the save");
}

#[test]
fn undoing_back_past_the_save_brings_the_dot_back() {
    let (mut s, _d, _path) = external_sim();
    edit(&mut s);
    s.cmd_key(Key::S);
    wait(&mut s, SETTLED);
    assert!(!dot(&s));
    s.shell.app.undo();
    wait(&mut s, SETTLED);
    assert!(dot(&s), "the document is no longer the file on disk");
}

#[cfg(unix)]
#[test]
fn a_failed_library_save_keeps_the_dot() {
    use std::os::unix::fs::PermissionsExt;
    let (mut s, d) = library_sim();
    let project = s.app().lib_file.as_ref().unwrap().project_id.clone();
    let files = d.0.join("lib").join("projects").join(&project).join("files");
    edit(&mut s);
    std::fs::set_permissions(&files, std::fs::Permissions::from_mode(0o555)).unwrap();
    // a process that may write anyway (root) cannot show a failed save
    let writable = std::fs::File::create(files.join("probe")).is_ok();
    s.cmd_key(Key::S);
    wait(&mut s, SETTLED);
    wait(&mut s, SETTLED);
    let still_dotted = dot(&s);
    std::fs::set_permissions(&files, std::fs::Permissions::from_mode(0o755)).unwrap();
    if writable {
        return;
    }
    assert!(s.app().toast.as_ref().is_some_and(|t| t.error), "the failure is shown");
    assert!(still_dotted, "the edit is not on disk");
}

#[test]
fn undoing_an_unsaved_edit_back_to_the_opened_document_clears_the_dot() {
    let (mut s, _d, _path) = external_sim();
    edit(&mut s);
    assert!(dot(&s));
    s.shell.app.undo();
    wait(&mut s, SETTLED);
    assert!(!dot(&s), "the document is what was opened");
}

#[test]
fn viewing_the_canvas_never_lights_the_dot() {
    let (mut s, _d, _path) = external_sim();
    edit(&mut s);
    s.cmd_key(Key::S);
    wait(&mut s, SETTLED);
    s.shell.app.zoom_to(2.0);
    wait(&mut s, SETTLED);
    assert!(!dot(&s), "a zoom changes no layer");

    let (mut s, _d) = library_sim();
    s.shell.app.zoom_to(2.0);
    wait(&mut s, SETTLED);
    assert!(!dot(&s), "a zoom changes no layer");
}

#[test]
fn ctrl_s_in_a_frame_that_also_carries_its_release_still_saves() {
    let (mut s, _d, path) = external_sim();
    edit(&mut s);
    s.mods = Modifiers::COMMAND;
    s.one_frame().key(Key::S).send();
    s.mods = Modifiers::NONE;
    wait(&mut s, SETTLED);
    let on_disk = Document::from_json(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(on_disk.nodes.len(), s.app().doc.nodes.len(), "the edit is on disk");
    assert!(!dot(&s));
}

/// The dot painted on the tab that holds the file `pick` selects.
fn dot_on_tab(s: &Sim, pick: impl Fn(&crate::tabs::ProjectTab) -> bool) -> bool {
    let tab = s.app().tabs.entries.iter().find(|t| pick(t)).expect("a tab for the file");
    s.rect_of(&format!("tab-unsaved-dot-{}", tab.id)).is_finite()
}

#[test]
fn a_library_tab_left_for_another_file_shows_no_dot_once_its_edit_is_written() {
    let (mut s, _d) = library_sim();
    let a = s.app().lib_file.as_ref().unwrap().clone();
    let b = lib(&s).create_file(&a.project_id, None, "Second", &two_rects()).unwrap();
    edit(&mut s);
    assert!(dot(&s));
    s.shell.app.go_home();
    wait(&mut s, SETTLED);
    crate::home::apply(&mut s.shell.app, Act::Open(b.id.clone()));
    wait(&mut s, SETTLED);
    assert_eq!(s.app().tabs.entries.len(), 2);
    assert!(!dot_on_tab(&s, |t| t.lib_file.as_ref().is_some_and(|f| f.id == a.id)), "leaving the file wrote the edit");
    assert!(!dot(&s), "the second file was not edited");
}

#[test]
fn a_tab_outside_the_library_keeps_its_own_state_while_another_tab_is_shown() {
    let (mut s, _d, path) = external_sim();
    let on = |t: &crate::tabs::ProjectTab| t.file_path.as_deref() == Some(path.as_path());
    edit(&mut s);
    // a second document replaces it on screen; the first one stays in its tab, unsaved
    let mut untitled = Document::new("Untitled");
    untitled.add_page("Page 1");
    s.shell.app.open_new_tab("Untitled", untitled, crate::tabs::ProjectTabKind::Design);
    wait(&mut s, SETTLED);
    assert_eq!(s.app().tabs.entries.len(), 2);
    assert!(dot_on_tab(&s, on), "the parked tab has an edit no save has written");
    assert!(!dot(&s), "the new document has none");

    // back to it: still unsaved. Save: clean, and it stays clean after another round trip
    let first = s.app().tabs.entries.iter().position(on).unwrap();
    switch_to(&mut s, first);
    assert!(dot(&s), "its edit is still unsaved");
    s.cmd_key(Key::S);
    wait(&mut s, SETTLED);
    assert!(!dot(&s));
    let second = 1 - first;
    switch_to(&mut s, second);
    assert!(!dot_on_tab(&s, on), "saved while shown, so clean while parked");
    switch_to(&mut s, first);
    assert!(!dot(&s), "a saved tab does not light up after a round trip");
}

fn switch_to(s: &mut Sim, idx: usize) {
    let mut tabs = std::mem::take(&mut s.shell.app.tabs);
    tabs.switch_to(idx, &mut s.shell.app);
    s.shell.app.tabs = tabs;
    wait(s, SETTLED);
}

#[test]
fn opening_a_library_file_never_flashes_the_dot() {
    let dir = TmpDir::new();
    let mut s = Sim::with(Options { home: true, library: Some(dir.0.join("lib")), ..Default::default() });
    s.shell.app.tabs.always_visible = true;
    let id = s.app().home.as_ref().unwrap().snap.files[0].id.clone();
    crate::home::apply(&mut s.shell.app, Act::Open(id));
    for _ in 0..30 {
        s.frame(vec![]);
        if !s.app().tabs.entries.is_empty() {
            assert!(!dot(&s), "at {:.2}s after the open", s.t);
        }
    }
    assert_eq!(s.app().autosave.saves, 0, "nothing was rewritten");
}

#[test]
fn the_dot_stays_lit_while_a_drag_holds_the_next_save_back() {
    let (mut s, _d) = library_sim();
    edit(&mut s);
    assert!(dot(&s), "the edit is waiting for autosave");
    // the pointer goes down on the canvas and stays there: autosave waits for it to come up
    let p = s.screen(300.0, 800.0);
    s.one_frame().move_to(p).press().send();
    for _ in 0..40 {
        s.frame(vec![]);
        assert!(dot(&s), "still unsaved at {:.2}s", s.t);
    }
    s.one_frame().move_to(p).release().send();
    wait(&mut s, 3.0);
    assert!(!dot(&s), "autosave wrote it once the pointer was up");
}
