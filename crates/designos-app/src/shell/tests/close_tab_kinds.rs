//! `designos_close_document` against the tabs a person really has open (AC-03, AC-10): an untitled tab, a `.fig` tab, a `.pen` tab and
//! a second dirty tab, and a tab that is already gone (closed by the person, or before this MCP process started).

use super::leave_prompt::{add_an_unsaved_untitled_tab, tab_id};
use super::live_agent_edits::finish;
use super::unsaved_dot::{edit, external_sim_with, library_sim, nodes_on_disk, two_rects, wait, TmpDir, SETTLED};
use super::*;
use crate::home::Leave;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

type Call = Result<Value, Value>;

fn fixture(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../designos-fig/tests/fixtures").join(rel)
}

/// An MCP session on the folder of the tab that is shown, with the library's autosave of agent changes off so the close is what writes.
fn mcp_on_the_shown_tab(s: &mut Sim, dir: &TmpDir) -> (designos_mcp::server::Session, PathBuf) {
    crate::comments::settings::save_autosave_ai_changes(s.app().lib.as_ref().expect("library").root(), false).unwrap();
    let workspace = dir.0.join("mcp-workspace");
    std::fs::create_dir_all(&workspace).unwrap();
    let mut session = designos_mcp::server::Session::with_session_dir(&workspace, &s.app().terminal.session_dir).expect("mcp session");
    session.set_close_ack_timeout(Duration::from_millis(800));
    // An agent that closes has edited or read the document before, so its log exists; a tab that was parked reads a close that is
    // already in a log it has not polled since it was shown as stale, so the editor must have polled the (empty) log once.
    let log = s.app().terminal.live_patch.path.clone();
    if !log.exists() {
        crate::agent_session_files::atomic_write_session_file(&log, b"").expect("create the log");
    }
    wait(s, SETTLED);
    (session, workspace)
}

fn close(s: &mut Sim, session: &mut designos_mcp::server::Session, args: Value) -> Call {
    std::thread::scope(|scope| {
        let h = scope.spawn(|| session.call("designos_close_document", args));
        let end = Instant::now() + Duration::from_secs(5);
        while !h.is_finished() && Instant::now() < end {
            wait(s, 0.1);
            std::thread::sleep(Duration::from_millis(5));
        }
        wait(s, SETTLED);
        h.join().expect("close thread").map_err(|e| e.to_json_value())
    })
}

fn switch_to(s: &mut Sim, idx: usize) {
    let mut tabs = std::mem::take(&mut s.shell.app.tabs);
    tabs.switch_to(idx, &mut s.shell.app);
    s.shell.app.tabs = tabs;
    wait(s, SETTLED);
}

fn next_step(v: &Value) -> &str {
    v["next_step"].as_str().unwrap_or("")
}

// ---- AC-03: untitled, .fig and .pen tabs -------------------------------------------------------------------------------------

fn untitled_tab() -> (Sim, TmpDir) {
    let dir = TmpDir::new();
    let mut s = Sim::with(Options { library: Some(dir.0.join("lib")), ..Default::default() });
    s.shell.app.tabs.always_visible = true;
    wait(&mut s, SETTLED);
    // A tab opened with "New" holds a document no file backs: its tab has no path and no library file.
    let mut doc = two_rects();
    doc.name = "Untitled".into();
    s.shell.app.open_new_tab("Untitled", doc, crate::tabs::ProjectTabKind::Design);
    wait(&mut s, SETTLED);
    assert!(s.app().file_path.is_none() && s.app().lib_file.is_none(), "a real untitled tab: no path, no library file");
    assert_eq!(s.app().tabs.entries.len(), 2, "the starting tab and the new one");
    edit(&mut s);
    assert!(s.app().has_unsaved_edits());
    (s, dir)
}

#[test]
fn a_real_untitled_tab_save_without_a_path_is_path_required_and_stays_open() {
    let (mut s, dir) = untitled_tab();
    let (mut session, workspace) = mcp_on_the_shown_tab(&mut s, &dir);
    let tabs = s.app().tabs.entries.len();
    let err = close(&mut s, &mut session, json!({"unsaved": "save"})).expect_err("an untitled tab needs a path");
    assert_eq!(err["code"], "PATH_REQUIRED", "{err}");
    assert_eq!(err["safe_to_retry"], false, "the same call fails again; the next step names the path: {err}");
    assert!(next_step(&err).contains("path"), "{err}");
    assert_eq!(s.app().tabs.entries.len(), tabs, "the tab stayed open");
    assert!(s.app().has_unsaved_edits(), "nothing was lost");
    finish(s, workspace);
}

#[test]
fn a_real_untitled_tab_save_with_a_path_writes_the_file_and_closes_only_that_tab() {
    let (mut s, dir) = untitled_tab();
    let (mut session, workspace) = mcp_on_the_shown_tab(&mut s, &dir);
    let nodes = s.app().doc.nodes.len();
    let target = dir.0.join("made.designos");
    let out = close(&mut s, &mut session, json!({"unsaved": "save", "path": target.to_string_lossy()})).expect("save to a path");
    assert_eq!(out["closed"], true, "{out}");
    assert_eq!(out["saved"], true, "{out}");
    assert_eq!(nodes_on_disk(&target), nodes, "the edit is in the file");
    assert_eq!(s.app().tabs.entries.len(), 1, "only the untitled tab closed");
    finish(s, workspace);
}

#[test]
fn a_real_untitled_tab_fail_refuses_and_discard_closes_without_a_file() {
    let (mut s, dir) = untitled_tab();
    let (mut session, workspace) = mcp_on_the_shown_tab(&mut s, &dir);
    let err = close(&mut s, &mut session, json!({})).expect_err("fail keeps a dirty tab");
    assert_eq!(err["code"], "UNSAVED_CHANGES", "{err}");
    assert_eq!(err["safe_to_retry"], false, "{err}");
    assert_eq!(s.app().tabs.entries.len(), 2, "the tab stayed open");
    let out = close(&mut s, &mut session, json!({"unsaved": "discard"})).expect("discard");
    assert_eq!(out["closed"], true, "{out}");
    assert_eq!(out["saved"], false, "{out}");
    assert_eq!(s.app().tabs.entries.len(), 1, "closed");
    assert!(!dir.0.join("made.designos").exists(), "discard wrote nothing");
    finish(s, workspace);
}

/// A sim whose first tab is `fixture` copied next to the library, edited and unsaved.
fn imported_tab(rel: &str) -> (Sim, TmpDir, PathBuf, Vec<u8>) {
    let dir = TmpDir::new();
    let name = Path::new(rel).file_name().unwrap().to_owned();
    let path = dir.0.join(name);
    std::fs::copy(fixture(rel), &path).expect("copy the fixture");
    let original = std::fs::read(&path).unwrap();
    let mut s = Sim::with(Options { file: Some(path.clone()), library: Some(dir.0.join("lib")), ..Default::default() });
    s.shell.app.tabs.always_visible = true;
    wait(&mut s, SETTLED);
    assert_eq!(s.app().file_path.as_deref(), Some(path.as_path()), "the tab holds the imported file's path");
    edit(&mut s);
    assert!(s.app().has_unsaved_edits(), "the edit is unsaved");
    (s, dir, path, original)
}

#[test]
fn a_fig_or_pen_tab_close_save_needs_a_native_path_and_never_rewrites_the_source() {
    for rel in ["legacy-0.4.0-rich.fig", "pen/v2_8.pen"] {
        let (mut s, dir, path, original) = imported_tab(rel);
        let (mut session, workspace) = mcp_on_the_shown_tab(&mut s, &dir);
        let nodes = s.app().doc.nodes.len();
        let err = close(&mut s, &mut session, json!({"unsaved": "save"})).expect_err(rel);
        assert_eq!(err["code"], "PATH_REQUIRED", "{rel}: the {rel} writer is not the native one: {err}");
        assert_eq!(s.app().tabs.entries.len(), 1, "{rel}: the tab stayed open");
        assert!(s.app().has_unsaved_edits(), "{rel}: still dirty");
        assert_eq!(std::fs::read(&path).unwrap(), original, "{rel}: the source file is untouched");
        let target = dir.0.join("copy.designos");
        let out = close(&mut s, &mut session, json!({"unsaved": "save", "path": target.to_string_lossy()})).unwrap_or_else(|e| panic!("{rel}: {e}"));
        assert_eq!(out["closed"], true, "{rel}: {out}");
        assert_eq!(out["saved"], true, "{rel}: {out}");
        assert_eq!(nodes_on_disk(&target), nodes, "{rel}: the native copy holds the edit");
        assert_eq!(std::fs::read(&path).unwrap(), original, "{rel}: the source file is still untouched");
        assert_eq!(s.app().tabs.entries.len(), 0, "{rel}: the tab closed");
        finish(s, workspace);
    }
}

#[test]
fn a_fig_or_pen_tab_fail_refuses_and_discard_closes_leaving_the_source() {
    for rel in ["legacy-0.4.0-rich.fig", "pen/v2_8.pen"] {
        let (mut s, dir, path, original) = imported_tab(rel);
        let (mut session, workspace) = mcp_on_the_shown_tab(&mut s, &dir);
        let err = close(&mut s, &mut session, json!({})).expect_err(rel);
        assert_eq!(err["code"], "UNSAVED_CHANGES", "{rel}: {err}");
        assert_eq!(err["safe_to_retry"], false, "{rel}: {err}");
        assert_eq!(s.app().tabs.entries.len(), 1, "{rel}: stayed open");
        let out = close(&mut s, &mut session, json!({"unsaved": "discard"})).unwrap_or_else(|e| panic!("{rel}: {e}"));
        assert_eq!(out["closed"], true, "{rel}: {out}");
        assert_eq!(s.app().tabs.entries.len(), 0, "{rel}: closed");
        assert_eq!(std::fs::read(&path).unwrap(), original, "{rel}: the source file is untouched");
        finish(s, workspace);
    }
}

// ---- AC-10: a second dirty tab is never asked about, saved or discarded -------------------------------------------------------

/// The file's tab (index 0, edited) and a second, unsaved untitled tab (edited); the file's tab is shown again.
fn file_tab_and_a_second_dirty_tab() -> (Sim, TmpDir, PathBuf, usize) {
    let (mut s, dir, path) = external_sim_with(&two_rects());
    edit(&mut s);
    add_an_unsaved_untitled_tab(&mut s);
    let second_nodes = s.app().doc.nodes.len();
    assert_eq!(s.app().tabs.entries.len(), 2);
    switch_to(&mut s, 0);
    assert!(s.app().has_unsaved_edits(), "the file tab is dirty");
    assert!(s.app().tabs.entries[1].dirty || second_nodes > 2, "the second tab holds edits");
    (s, dir, path, second_nodes)
}

fn the_second_tab_is_untouched(s: &Sim, second_nodes: usize) {
    assert_eq!(s.app().tabs.entries.len(), 1, "only the closed tab went");
    assert_eq!(s.app().tabs.entries[0].title, "Untitled", "the other tab is the untitled one");
    assert_eq!(s.app().doc.nodes.len(), second_nodes, "its edits are all there");
    assert!(s.app().has_unsaved_edits(), "and still unsaved: nothing saved or discarded it");
    assert!(s.app().dialog.is_none(), "nobody was asked about it: {:?}", s.app().dialog.as_ref().map(|d| d.title.clone()));
    assert!(!s.app().leave.is_asking() && !s.app().leave.quit_approved, "no leave flow started");
}

#[test]
fn closing_a_dirty_tab_with_save_leaves_a_second_dirty_tab_open_and_unsaved() {
    let (mut s, dir, path, second_nodes) = file_tab_and_a_second_dirty_tab();
    let (mut session, workspace) = mcp_on_the_shown_tab(&mut s, &dir);
    let file_nodes = s.app().doc.nodes.len();
    let out = close(&mut s, &mut session, json!({"unsaved": "save"})).expect("save close");
    assert_eq!(out["closed"], true, "{out}");
    assert_eq!(nodes_on_disk(&path), file_nodes, "the closed tab's edit was saved");
    the_second_tab_is_untouched(&s, second_nodes);
    finish(s, workspace);
}

#[test]
fn closing_a_dirty_tab_with_discard_leaves_a_second_dirty_tab_open_and_unsaved() {
    let (mut s, dir, path, second_nodes) = file_tab_and_a_second_dirty_tab();
    let (mut session, workspace) = mcp_on_the_shown_tab(&mut s, &dir);
    let on_disk = nodes_on_disk(&path);
    let out = close(&mut s, &mut session, json!({"unsaved": "discard"})).expect("discard close");
    assert_eq!(out["closed"], true, "{out}");
    assert_eq!(nodes_on_disk(&path), on_disk, "the file is as it was");
    the_second_tab_is_untouched(&s, second_nodes);
    finish(s, workspace);
}

#[test]
fn a_refused_close_keeps_both_dirty_tabs() {
    let (mut s, dir, _path, second_nodes) = file_tab_and_a_second_dirty_tab();
    let (mut session, workspace) = mcp_on_the_shown_tab(&mut s, &dir);
    let err = close(&mut s, &mut session, json!({})).expect_err("fail on a dirty tab");
    assert_eq!(err["code"], "UNSAVED_CHANGES", "{err}");
    assert_eq!(s.app().tabs.entries.len(), 2, "both tabs are open");
    assert!(s.app().dialog.is_none(), "no dialog");
    switch_to(&mut s, 1);
    assert_eq!(s.app().doc.nodes.len(), second_nodes, "the second tab's edits are there");
    assert!(s.app().has_unsaved_edits(), "and unsaved");
    finish(s, workspace);
}

// ---- A tab that is already gone -----------------------------------------------------------------------------------------------

/// What a new MCP process (or one whose tab the person closed) gets: an answer at once that says asking again can never work.
fn assert_tab_closed_at_once(s: &mut Sim, session: &mut designos_mcp::server::Session, args: Value, what: &str) {
    session.set_close_ack_timeout(Duration::from_secs(10));
    let started = Instant::now();
    let err = session.call("designos_close_document", args).map_err(|e| e.to_json_value()).expect_err(what);
    assert!(started.elapsed() < Duration::from_secs(2), "{what}: answered without waiting for an editor that is not there ({:?})", started.elapsed());
    assert_eq!(err["code"], "EDITOR_NOT_NOTIFIED", "{what}: {err}");
    assert_eq!(err["safe_to_retry"], false, "{what}: asking again can never work: {err}");
    assert!(next_step(&err).contains("already closed"), "{what}: {err}");
    wait(s, SETTLED);
}

#[test]
fn a_close_after_the_person_closed_an_outside_file_tab_is_tab_closed_not_a_timeout() {
    for args in [json!({}), json!({"unsaved": "save"}), json!({"unsaved": "discard"})] {
        let (mut s, dir, _path) = external_sim_with(&two_rects());
        let (mut session, workspace) = mcp_on_the_shown_tab(&mut s, &dir);
        let session_dir = s.app().terminal.session_dir.clone();
        let id = tab_id(&s, 0);
        s.shell.app.request_leave(None, Leave::CloseTab(id));
        wait(&mut s, SETTLED);
        assert_eq!(s.app().tabs.entries.len(), 0, "the person closed the tab");
        assert!(session_dir.join("tab-closed.json").is_file(), "the closed tab left its mark in its folder");
        assert_tab_closed_at_once(&mut s, &mut session, args.clone(), &format!("the same process, {args}"));
        let mut fresh = designos_mcp::server::Session::with_session_dir(&workspace, &session_dir).expect("a new process");
        assert_tab_closed_at_once(&mut s, &mut fresh, args.clone(), &format!("a new process, {args}"));
        finish(s, workspace);
    }
}

#[test]
fn a_close_after_the_person_closed_an_untitled_tab_is_tab_closed_not_a_timeout() {
    for args in [json!({}), json!({"unsaved": "save"}), json!({"unsaved": "discard"})] {
        let dir = TmpDir::new();
        let mut s = Sim::with(Options { library: Some(dir.0.join("lib")), ..Default::default() });
        s.shell.app.tabs.always_visible = true;
        let mut doc = two_rects();
        doc.name = "Untitled".into();
        s.shell.app.open_new_tab("Untitled", doc, crate::tabs::ProjectTabKind::Design);
        wait(&mut s, SETTLED);
        let (mut session, workspace) = mcp_on_the_shown_tab(&mut s, &dir);
        let session_dir = s.app().terminal.session_dir.clone();
        assert!(crate::agent_session_files::is_ephemeral_session_dir(&session_dir), "an untitled tab works in a scratch folder");
        let shown = s.app().tabs.active_idx;
        let id = tab_id(&s, shown);
        s.shell.app.request_leave(None, Leave::CloseTab(id));
        wait(&mut s, SETTLED);
        assert_eq!(s.app().tabs.entries.len(), 1, "the person closed the untitled tab");
        assert!(!session_dir.exists(), "a scratch folder goes with its tab");
        assert_tab_closed_at_once(&mut s, &mut session, args.clone(), &format!("the same process, {args}"));
        let mut fresh = designos_mcp::server::Session::with_session_dir(&workspace, &session_dir).expect("a new process");
        assert_tab_closed_at_once(&mut s, &mut fresh, args.clone(), &format!("a new process, {args}"));
        assert!(!session_dir.exists(), "the close did not bring the folder back");
        finish(s, workspace);
    }
}

#[test]
fn a_close_after_the_person_closed_a_library_file_tab_is_tab_closed_not_a_timeout() {
    for args in [json!({}), json!({"unsaved": "save"}), json!({"unsaved": "discard"})] {
        let (mut s, dir) = library_sim();
        let (mut session, workspace) = mcp_on_the_shown_tab(&mut s, &dir);
        let session_dir = s.app().terminal.session_dir.clone();
        assert!(session_dir.to_string_lossy().contains("agent-sessions"), "the folder of a library file stays: {}", session_dir.display());
        let id = tab_id(&s, 0);
        s.shell.app.request_leave(None, Leave::CloseTab(id));
        wait(&mut s, SETTLED);
        assert_eq!(s.app().tabs.entries.len(), 0, "the person closed the tab");
        assert!(session_dir.is_dir(), "a library file's folder outlives its tab");
        assert_tab_closed_at_once(&mut s, &mut session, args.clone(), &format!("the same process, {args}"));
        let mut fresh = designos_mcp::server::Session::with_session_dir(&workspace, &session_dir).expect("a new process");
        assert_tab_closed_at_once(&mut s, &mut fresh, args.clone(), &format!("a new process, {args}"));
        finish(s, workspace);
    }
}

#[test]
fn opening_the_library_file_again_makes_a_close_work_again() {
    let (mut s, dir) = library_sim();
    let (_old, workspace) = mcp_on_the_shown_tab(&mut s, &dir);
    let session_dir = s.app().terminal.session_dir.clone();
    let file_id = s.app().lib_file.as_ref().expect("library file").id.clone();
    let id = tab_id(&s, 0);
    s.shell.app.request_leave(None, Leave::CloseTab(id));
    wait(&mut s, SETTLED);
    assert!(session_dir.join("tab-closed.json").is_file(), "the closed tab left its mark");
    crate::home::apply(&mut s.shell.app, crate::home::Act::Open(file_id));
    wait(&mut s, SETTLED);
    assert_eq!(s.app().tabs.entries.len(), 1, "the file is open again");
    assert_eq!(s.app().terminal.session_dir, session_dir, "on the same folder");
    assert!(!session_dir.join("tab-closed.json").exists(), "the mark of the earlier tab is gone");
    let mut fresh = designos_mcp::server::Session::with_session_dir(&workspace, &session_dir).expect("a new process");
    fresh.set_close_ack_timeout(Duration::from_millis(800));
    let out = close(&mut s, &mut fresh, json!({"unsaved": "discard"})).expect("the reopened tab closes");
    assert_eq!(out["closed"], true, "{out}");
    assert_eq!(s.app().tabs.entries.len(), 0);
    finish(s, workspace);
}

#[test]
fn a_tab_that_is_open_but_not_answering_still_times_out_as_retryable() {
    let (mut s, dir, _path) = external_sim_with(&two_rects());
    let (mut session, workspace) = mcp_on_the_shown_tab(&mut s, &dir);
    session.set_close_ack_timeout(Duration::from_millis(300));
    // No frames run: the editor is slow, not gone.
    let err = session.call("designos_close_document", json!({"unsaved": "save"})).map_err(|e| e.to_json_value()).expect_err("no answer");
    assert_eq!(err["code"], "EDITOR_NOT_NOTIFIED", "{err}");
    assert_eq!(err["safe_to_retry"], true, "a slow editor may answer a retry: {err}");
    assert_eq!(s.app().tabs.entries.len(), 1, "the tab is open");
    finish(s, workspace);
}
