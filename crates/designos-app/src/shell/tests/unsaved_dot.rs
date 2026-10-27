//! The unsaved dot on a tab: it follows what the last save (or the open) wrote, not the undo stack.
//! A save that did not happen (cancelled dialog, no dialog, write error) keeps the dot.
//!
//! The dot is current a frame or two after the change; every test waits [`SETTLED`] after one.

use super::*;
use crate::home::Act;
use crate::io::{DialogOutcome, ScriptedDialogs};
use designos_doc::{Document, Node, PageHydrator, Rect as DRect};
use designos_library::Library;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub(super) struct TmpDir(pub(super) PathBuf);

impl TmpDir {
    pub(super) fn new() -> Self {
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
pub(super) fn two_rects() -> Document {
    let mut d = Document::new("Outside");
    let page = d.add_page("P");
    for i in 0..2 {
        let id = d.alloc_id();
        d.insert(page, Node::rectangle(id, DRect::new(i as f32 * 100.0, 0.0, 50.0, 50.0))).unwrap();
    }
    d
}

/// A sim on the library's seeded "DESIGN-OS Studio Starter" file, tab bar shown.
pub(super) fn library_sim() -> (Sim, TmpDir) {
    let dir = TmpDir::new();
    let mut s = Sim::with(Options { home: true, library: Some(dir.0.join("lib")), ..Default::default() });
    s.shell.app.tabs.always_visible = true;
    let id = {
        let home = s.app().home.as_ref().unwrap();
        home.snap.files.iter().find(|f| f.name == "DESIGN-OS Studio Starter").unwrap().id.clone()
    };
    crate::home::apply(&mut s.shell.app, Act::Open(id));
    wait(&mut s, SETTLED);
    (s, dir)
}

/// A sim on a `.designos` file outside the library.
fn external_sim() -> (Sim, TmpDir, PathBuf) {
    external_sim_with(&two_rects())
}

pub(super) fn external_sim_with(doc: &Document) -> (Sim, TmpDir, PathBuf) {
    let dir = TmpDir::new();
    let path = dir.0.join("outside.designos");
    std::fs::write(&path, doc.to_json().unwrap()).unwrap();
    let mut s = Sim::with(Options { file: Some(path.clone()), library: Some(dir.0.join("lib")), ..Default::default() });
    s.shell.app.tabs.always_visible = true;
    wait(&mut s, SETTLED);
    (s, dir, path)
}

/// Longer than the settle delay of a changed revision.
pub(super) const SETTLED: f64 = 0.5;

pub(super) fn wait(s: &mut Sim, secs: f64) {
    let end = s.t + secs;
    while s.t < end {
        s.frame(vec![]);
    }
}

/// Whether the active tab was painted with the unsaved dot in the last frame.
pub(super) fn dot(s: &Sim) -> bool {
    let id = s.app().tabs.entries[s.app().tabs.active_idx].id;
    s.rect_of(&format!("tab-unsaved-dot-{id}")).is_finite()
}

/// One undoable edit: duplicates the first layer on the page.
pub(super) fn edit_now(s: &mut Sim) {
    let app = &mut s.shell.app;
    let first = app.doc.get(app.page).unwrap().children[0];
    app.selection = vec![first];
    app.duplicate_selection();
}

/// [`edit_now`], then waits for the dot to settle.
pub(super) fn edit(s: &mut Sim) {
    edit_now(s);
    wait(s, SETTLED);
}

pub(super) fn nodes_on_disk(path: &std::path::Path) -> usize {
    Document::from_json(&std::fs::read_to_string(path).unwrap()).map(|d| d.nodes.len()).unwrap_or(0)
}

pub(super) fn lib(s: &Sim) -> Library {
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
    let rev = s.app().canvas.tiles.rev();
    s.shell.app.zoom_to(2.0);
    wait(&mut s, SETTLED);
    assert_eq!(s.app().canvas.tiles.rev(), rev, "a zoom is not a change of the document");
    assert!(!dot(&s), "a zoom changes no layer");

    let (mut s, _d) = library_sim();
    let rev = s.app().canvas.tiles.rev();
    s.shell.app.zoom_to(2.0);
    wait(&mut s, SETTLED);
    assert_eq!(s.app().canvas.tiles.rev(), rev, "a zoom is not a change of the document");
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
pub(super) fn dot_on_tab(s: &Sim, pick: impl Fn(&crate::tabs::ProjectTab) -> bool) -> bool {
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

pub(super) fn switch_to(s: &mut Sim, idx: usize) {
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
    let mut checked = 0;
    for _ in 0..30 {
        s.frame(vec![]);
        if !s.app().tabs.entries.is_empty() {
            assert!(!dot(&s), "at {:.2}s after the open", s.t);
            checked += 1;
        }
    }
    assert!(checked >= 20, "the dot was looked at in {checked} of 30 frames");
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

/// `n` rectangles on page 1 and an empty page 2, which is the page a hydrator fills on its first visit.
fn two_page_doc(n: usize) -> (Document, NodeId) {
    let mut d = Document::new("Pages");
    let p1 = d.add_page("P1");
    for i in 0..n {
        let id = d.alloc_id();
        d.insert(p1, Node::rectangle(id, DRect::new((i % 100) as f32 * 6.0, (i / 100) as f32 * 6.0, 5.0, 5.0))).unwrap();
    }
    let p2 = d.add_page("P2");
    (d, p2)
}

/// Models the deferred pages of a big `.fig`: the first visit to `page` adds a node to it.
#[derive(Debug)]
struct OneNodeHydrator {
    page: NodeId,
    done: bool,
}

impl PageHydrator for OneNodeHydrator {
    fn hydrate_page(&mut self, doc: &mut Document, page: NodeId) -> bool {
        if page != self.page || self.done {
            return false;
        }
        self.done = true;
        let id = doc.alloc_id();
        doc.insert(page, Node::rectangle(id, DRect::new(0.0, 0.0, 10.0, 10.0))).is_ok()
    }
    fn hydrate_all(&mut self, doc: &mut Document) -> bool {
        let page = self.page;
        self.hydrate_page(doc, page)
    }
    fn is_hydrated(&self, page: NodeId) -> bool {
        page != self.page || self.done
    }
}

#[test]
fn visiting_a_page_after_a_save_does_not_light_the_dot() {
    // 14 nodes take the full content hash, 5,100 the cheap epoch hash: neither may see the page visit as an edit
    for n in [10, 5100] {
        let (doc, page2) = two_page_doc(n);
        let (mut s, _d, _path) = external_sim_with(&doc);
        s.shell.app.doc.page_hydrator = Some(Arc::new(Mutex::new(OneNodeHydrator { page: page2, done: false })));
        edit(&mut s);
        s.cmd_key(Key::S);
        wait(&mut s, SETTLED);
        assert!(!dot(&s), "saved ({n} rects)");
        let (nodes, depth) = (s.app().doc.nodes.len(), s.app().hist.undo_depth());
        s.shell.app.switch_page(page2);
        wait(&mut s, SETTLED);
        assert!(s.app().doc.nodes.len() > nodes, "the page was expanded ({n} rects)");
        assert_eq!(s.app().hist.undo_depth(), depth, "no undo step: not an edit ({n} rects)");
        assert!(!dot(&s), "a page visit is not an edit ({n} rects)");
    }
}

#[test]
fn undoing_back_to_the_saved_state_clears_the_dot_on_a_big_document() {
    let (doc, _page2) = two_page_doc(5100);
    let (mut s, _d, _path) = external_sim_with(&doc);
    edit(&mut s);
    s.cmd_key(Key::S);
    wait(&mut s, SETTLED);
    edit(&mut s);
    assert!(dot(&s));
    s.shell.app.undo();
    wait(&mut s, SETTLED);
    assert!(!dot(&s), "back at the saved state, whatever the document size");
    s.shell.app.undo();
    wait(&mut s, SETTLED);
    assert!(dot(&s), "past it");
}

#[test]
fn undoing_to_the_saved_state_and_editing_again_is_not_the_saved_document() {
    let (mut s, _d, _path) = external_sim();
    edit(&mut s);
    edit(&mut s);
    s.cmd_key(Key::S);
    wait(&mut s, SETTLED);
    s.shell.app.undo();
    // a different edit takes the place of the saved step: the same depth, another document
    edit(&mut s);
    assert!(dot(&s));
    s.shell.app.undo();
    s.shell.app.redo();
    wait(&mut s, SETTLED);
    assert!(dot(&s), "the saved state cannot be reached again");
    s.cmd_key(Key::S);
    wait(&mut s, SETTLED);
    assert!(!dot(&s));
}

#[test]
fn an_agent_that_replaces_the_document_lights_the_dot_until_the_next_save() {
    let (mut s, d, path) = external_sim();
    let mut replaced = s.app().doc.clone();
    replaced.name = "Agent version".into();
    s.shell.app.terminal.live_patch.pending_doc = Some(replaced);
    wait(&mut s, SETTLED);
    assert_eq!(s.app().doc.name, "Agent version", "the agent's document is on screen");
    assert_eq!(s.app().hist.undo_depth(), 0, "no undo step came with it");
    assert!(dot(&s), "it is not in the file");
    // It has no file yet: Save asks where it goes, and the file the user had is not written.
    let elsewhere = d.0.join("agent-version.designos");
    s.shell.app.save_with(false, &ScriptedDialogs::answering(DialogOutcome::Picked(elsewhere.clone())));
    wait(&mut s, SETTLED);
    assert!(!dot(&s));
    assert_eq!(Document::from_json(&std::fs::read_to_string(&elsewhere).unwrap()).unwrap().name, "Agent version");
    assert_eq!(Document::from_json(&std::fs::read_to_string(&path).unwrap()).unwrap().name, "Outside", "the user's file is untouched");
}

/// Runs frames until `done` holds and returns the repaint requests of that last frame.
fn repaints_of_the_frame_where(s: &mut Sim, done: impl Fn(&Sim) -> bool) -> Vec<Duration> {
    let log = Arc::new(Mutex::new(Vec::new()));
    let log_in_callback = log.clone();
    s.ctx.set_request_repaint_callback(move |info| log_in_callback.lock().unwrap().push(info.delay));
    for _ in 0..200 {
        log.lock().unwrap().clear();
        s.frame(vec![]);
        if done(s) {
            return log.lock().unwrap().clone();
        }
    }
    panic!("the condition never held");
}

#[test]
fn autosave_clearing_the_dot_asks_for_the_repaint_that_shows_it() {
    // the tab bar is drawn before autosave runs: without a request of its own the old dot stays until an input arrives
    let (mut s, _d) = library_sim();
    edit(&mut s);
    assert!(dot(&s));
    let saves = s.app().autosave.saves;
    let requests = repaints_of_the_frame_where(&mut s, |s| s.app().autosave.saves > saves);
    assert!(!s.app().has_unsaved_edits(), "autosave wrote the edit");
    assert!(requests.contains(&Duration::ZERO), "an immediate repaint is asked for in the frame that clears the dot: {requests:?}");
    s.frame(vec![]);
    assert!(!dot(&s));
}

/// The repaint requests made while `act` runs its frames.
fn repaints_during(s: &mut Sim, act: impl FnOnce(&mut Sim)) -> Vec<Duration> {
    let log = Arc::new(Mutex::new(Vec::new()));
    let log_in_callback = log.clone();
    s.ctx.set_request_repaint_callback(move |info| log_in_callback.lock().unwrap().push(info.delay));
    act(s);
    let requests = log.lock().unwrap().clone();
    requests
}

#[test]
fn undoing_to_the_saved_state_asks_for_the_repaint_that_shows_it() {
    let (mut s, _d, _path) = external_sim();
    edit(&mut s);
    s.cmd_key(Key::S);
    wait(&mut s, SETTLED);
    edit(&mut s);
    assert!(dot(&s));
    // Ctrl+Z is handled after the tab bar was drawn: the frame that clears the dot has already painted it
    let requests = repaints_during(&mut s, |s| s.cmd_key(Key::Z));
    assert!(!s.app().has_unsaved_edits());
    assert!(requests.contains(&Duration::ZERO), "an immediate repaint is asked for: {requests:?}");
}

#[test]
fn saving_asks_for_the_repaint_that_shows_the_clean_tab() {
    let (mut s, _d, _path) = external_sim();
    edit(&mut s);
    assert!(dot(&s));
    let requests = repaints_during(&mut s, |s| s.cmd_key(Key::S));
    assert!(!s.app().has_unsaved_edits());
    assert!(requests.contains(&Duration::ZERO), "an immediate repaint is asked for: {requests:?}");
}

#[test]
fn a_held_dot_asks_for_no_repaint_so_an_idle_window_stays_idle() {
    let (mut s, _d) = library_sim();
    edit(&mut s);
    wait(&mut s, 3.0);
    let requests = repaints_of_the_frame_where(&mut s, |_| true);
    assert!(requests.is_empty(), "a saved library file left alone asks for nothing: {requests:?}");

    // a document outside the library has one timed wake-up of its own (the disk watcher's); the dot adds no repaint
    // Left alone means after the edit's own follow-up frames: on a slow machine the frame right after an edit can
    // still ask for one to finish deferred drawing, which is not the dot's doing.
    let (mut s, _d, _path) = external_sim();
    edit(&mut s);
    wait(&mut s, 3.0);
    assert!(dot(&s));
    let requests = repaints_of_the_frame_where(&mut s, |_| true);
    assert!(!requests.contains(&Duration::ZERO), "an unsaved document left alone asks for no immediate repaint: {requests:?}");
}

#[test]
fn switching_back_to_an_unedited_library_tab_never_lights_the_dot() {
    let (mut s, _d) = library_sim();
    let a = s.app().lib_file.as_ref().unwrap().clone();
    let b = lib(&s).create_file(&a.project_id, None, "Second", &two_rects()).unwrap();
    s.shell.app.go_home();
    wait(&mut s, SETTLED);
    crate::home::apply(&mut s.shell.app, Act::Open(b.id.clone()));
    wait(&mut s, 3.0);
    assert_eq!(s.app().tabs.entries.len(), 2);
    let first = s.app().tabs.entries.iter().position(|t| t.lib_file.as_ref().is_some_and(|f| f.id == a.id)).unwrap();
    let mut tabs = std::mem::take(&mut s.shell.app.tabs);
    tabs.switch_to(first, &mut s.shell.app);
    s.shell.app.tabs = tabs;
    let end = s.t + 3.0;
    while s.t < end {
        s.frame(vec![]);
        assert!(!dot(&s), "an unedited file shows no dot, at {:.2}s after the switch", s.t);
    }
}

#[test]
fn edits_that_net_out_on_a_library_file_leave_no_dot_and_no_rewrite() {
    let (mut s, _d) = library_sim();
    let id = s.app().lib_file.as_ref().unwrap().id.clone();
    let modified = lib(&s).file(&id).unwrap().modified;
    // rename a layer and rename it back by hand: two undo steps, the document as it was
    let app = &mut s.shell.app;
    let first = app.doc.get(app.page).unwrap().children[0];
    let original = app.doc.get(first).unwrap().clone();
    let mut renamed = original.clone();
    renamed.name = "Something else".into();
    app.hist.exec(&mut app.doc, designos_doc::Command::Update { node: Box::new(renamed) }).unwrap();
    app.changed();
    wait(&mut s, 0.1);
    assert!(dot(&s), "an edit");
    let app = &mut s.shell.app;
    app.hist.exec(&mut app.doc, designos_doc::Command::Update { node: Box::new(original) }).unwrap();
    app.changed();
    wait(&mut s, 3.0);
    assert_eq!(s.app().hist.undo_depth(), 2);
    assert!(!dot(&s), "the file holds exactly this document");
    assert_eq!(s.app().autosave.saves, 0, "nothing needed writing");
    assert_eq!(lib(&s).file(&id).unwrap().modified, modified);
}

#[test]
fn rewriting_the_file_on_disk_never_clears_the_dot_of_an_unsaved_edit() {
    // a sync client, `touch` or an agent writing the same bytes; a tool that left a partial file
    for (what, garbage) in [("the same bytes", false), ("an unreadable file", true)] {
        let (mut s, _d, path) = external_sim();
        let original = std::fs::read(&path).unwrap();
        edit(&mut s);
        assert!(dot(&s));
        let edited = s.app().doc.nodes.len();
        // a later mtime than the one the app recorded
        std::thread::sleep(Duration::from_millis(30));
        std::fs::write(&path, if garbage { b"not a document".to_vec() } else { original }).unwrap();
        wait(&mut s, 1.5);
        assert_eq!(s.app().doc.nodes.len(), edited, "the document keeps its edit ({what})");
        assert!(nodes_on_disk(&path) < edited, "the edit is not in the file ({what})");
        assert!(dot(&s), "the edit is still unsaved ({what})");
    }
}

#[test]
fn a_reload_that_replaces_the_document_with_the_file_leaves_no_dot() {
    let (mut s, _d, path) = external_sim();
    let (changed, _) = two_page_doc(7);
    std::thread::sleep(Duration::from_millis(30));
    std::fs::write(&path, changed.to_json().unwrap()).unwrap();
    wait(&mut s, 1.5);
    assert_eq!(s.app().doc.nodes.len(), changed.nodes.len(), "the file's document is on screen");
    assert!(!dot(&s), "it is the file");
    edit(&mut s);
    assert!(dot(&s), "and an edit on it is unsaved");
}

#[test]
fn an_edit_that_commits_right_after_a_save_is_not_saved() {
    // a native Save dialog blocks the UI thread while an agent's edit queues; both land before the next frame
    let (mut s, _d, path) = external_sim();
    edit(&mut s);
    s.shell.app.save_with(false, &ScriptedDialogs::answering(DialogOutcome::Cancelled));
    let saved = nodes_on_disk(&path);
    edit_now(&mut s);
    wait(&mut s, SETTLED);
    assert_eq!(s.app().doc.nodes.len(), saved + 1);
    assert_eq!(nodes_on_disk(&path), saved, "the second edit is not in the file");
    assert!(dot(&s), "so it is unsaved");
    s.cmd_key(Key::S);
    wait(&mut s, SETTLED);
    assert!(!dot(&s));
}

#[cfg(unix)]
#[test]
fn a_restore_whose_write_fails_keeps_the_dot() {
    use std::os::unix::fs::PermissionsExt;
    let (mut s, d) = library_sim();
    let lf = s.app().lib_file.as_ref().unwrap().clone();
    let version = lib(&s).save_version(&lf.id, &s.app().doc, Some("v".into()), false).unwrap();
    edit(&mut s);
    wait(&mut s, 3.0);
    let files = d.0.join("lib").join("projects").join(&lf.project_id).join("files");
    std::fs::set_permissions(&files, std::fs::Permissions::from_mode(0o555)).unwrap();
    // a process that may write anyway (root) cannot show a failed write
    let writable = std::fs::File::create(files.join("probe")).is_ok();
    s.shell.app.restore_version(version.n);
    wait(&mut s, SETTLED);
    let lit = dot(&s);
    std::fs::set_permissions(&files, std::fs::Permissions::from_mode(0o755)).unwrap();
    if writable {
        return;
    }
    assert!(lit, "the restored document is not in the file");
}

/// An unsaved comment on a fresh untitled document: the one thing the `.fig` and `.pen` writers do not keep.
fn untitled_with_a_comment() -> (Sim, TmpDir) {
    let dir = TmpDir::new();
    let mut s = Sim::with(Options { library: Some(dir.0.join("lib")), ..Default::default() });
    s.shell.app.tabs.always_visible = true;
    s.shell.app.replace_document(two_rects(), None);
    wait(&mut s, SETTLED);
    let page = s.app().page;
    s.shell.app.post_comment(designos_doc::CommentAnchor { page, node: None, offset: designos_doc::Vec2::new(10.0, 10.0) }, "keep me");
    wait(&mut s, SETTLED);
    assert_eq!(s.app().doc.comments.len(), 1);
    assert!(dot(&s), "the comment is unsaved");
    (s, dir)
}

#[test]
fn a_save_to_fig_or_pen_does_not_clear_the_dot_because_those_files_lack_what_the_document_holds() {
    for ext in ["fig", "pen", "FIG"] {
        let (mut s, d) = untitled_with_a_comment();
        let target = d.0.join(format!("copy.{ext}"));
        s.shell.app.save_with(false, &ScriptedDialogs::answering(DialogOutcome::Picked(target.clone())));
        wait(&mut s, SETTLED);
        assert!(target.is_file(), "the file was written ({ext})");
        let kept = crate::io::load_path(&target).map(|d| d.comments.len()).unwrap_or(0);
        assert_eq!(kept, 0, "the {ext} writer does not keep comments: the premise of the dot staying");
        assert!(dot(&s), "the comment is in the app and not in the file ({ext})");
    }
}

#[test]
fn a_save_to_the_native_format_clears_the_dot() {
    for ext in ["designos", "DESIGNOS"] {
        let (mut s, d) = untitled_with_a_comment();
        let target = d.0.join(format!("copy.{ext}"));
        s.shell.app.save_with(false, &ScriptedDialogs::answering(DialogOutcome::Picked(target.clone())));
        wait(&mut s, SETTLED);
        let kept = crate::io::load_path(&target).map(|d| d.comments.len()).unwrap_or(0);
        assert_eq!(kept, 1, "the file holds the comment ({ext})");
        assert!(!dot(&s), "everything the document holds is in the file ({ext})");
    }
}

#[test]
fn a_native_save_after_a_copy_to_fig_clears_the_dot_that_the_copy_left() {
    let (mut s, d) = untitled_with_a_comment();
    s.shell.app.save_with(false, &ScriptedDialogs::answering(DialogOutcome::Picked(d.0.join("copy.fig"))));
    wait(&mut s, SETTLED);
    assert!(dot(&s), "the copy lacks the comment");
    let target = d.0.join("whole.designos");
    s.shell.app.save_with(false, &ScriptedDialogs::answering(DialogOutcome::Picked(target.clone())));
    wait(&mut s, SETTLED);
    assert!(!dot(&s), "the native file has everything");
    assert_eq!(s.app().file_path.as_deref(), Some(target.as_path()), "the document now lives in the native file");
}
