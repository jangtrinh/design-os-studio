//! Every way a parked tab is shown again takes the stale-close boundary of its edit log at that moment, so a close the agent writes
//! right after the tab is shown is never read as one written while it was hidden. A shown terminal that skipped it is counted by its
//! log at the next poll (`late_boundaries`), and each test here drives one of the ways a tab is shown.
//!
//! The log has to exist: a poll of a log that is not there returns before the boundary is looked at, so a test without one passes
//! whatever the switch does. Studio never makes the log (only the agent's MCP server appends to it), so each test makes it.
//!
//! Not covered here because nothing can be hidden there: a tab opened anew and a duplicated tab are shown with a terminal made for
//! them, whose log was never parked. Also not reached: the branches of the frame reconcile that show a parked tab holding the file
//! just opened from Home. Opening a file that a parked tab holds switches to that tab instead, which the Home tests below drive.

use super::leave_prompt::tab_id;
use super::unsaved_dot::{external_sim_with, library_sim, two_rects, wait, TmpDir, SETTLED};
use super::*;
use crate::home::{Act, Leave};

/// Makes the edit log of the shown tab, empty, so a poll reaches the boundary.
fn make_the_shown_tabs_log(s: &Sim) {
    let log = s.app().terminal.live_patch.path.clone();
    crate::agent_session_files::atomic_write_session_file(&log, b"").expect("the agent's edit log");
}

/// How often the shown tab's log took its boundary late, at a poll.
fn late(s: &Sim) -> u32 {
    s.app().terminal.live_patch.late_boundaries()
}

fn with_tabs(s: &mut Sim, f: impl FnOnce(&mut crate::tabs::TabsState, &mut App)) {
    let mut tabs = std::mem::take(&mut s.shell.app.tabs);
    f(&mut tabs, &mut s.shell.app);
    s.shell.app.tabs = tabs;
}

fn open_tab(s: &mut Sim, title: &str) {
    let mut doc = two_rects();
    doc.name = title.into();
    s.shell.app.open_new_tab(title, doc, crate::tabs::ProjectTabKind::Design);
    wait(s, SETTLED);
}

/// Two clean tabs, "First" and "Second", next to the starting one, each with its log; "Second" is shown and "First" is parked.
/// Made without showing a parked tab, so the step a test drives is the only one that shows one.
fn two_tabs_with_logs() -> (Sim, TmpDir) {
    let dir = TmpDir::new();
    let mut s = Sim::with(Options { library: Some(dir.0.join("lib")), ..Default::default() });
    s.shell.app.tabs.always_visible = true;
    wait(&mut s, SETTLED);
    open_tab(&mut s, "First");
    make_the_shown_tabs_log(&s);
    open_tab(&mut s, "Second");
    make_the_shown_tabs_log(&s);
    let first = s.app().tabs.entries.iter().find(|t| t.title == "First").expect("the first tab");
    assert!(first.terminal.live_patch.path.is_file(), "the parked tab keeps the log it was shown with");
    (s, dir)
}

fn shown_title(s: &Sim) -> String {
    s.app().tabs.entries[s.app().tabs.active_idx].title.clone()
}

#[test]
fn switching_to_a_parked_tab_takes_its_boundary() {
    let (mut s, _dir) = two_tabs_with_logs();
    for title in ["First", "Second", "First"] {
        let idx = s.app().tabs.entries.iter().position(|t| t.title == title).unwrap();
        with_tabs(&mut s, |tabs, app| tabs.switch_to(idx, app));
        wait(&mut s, SETTLED);
        assert_eq!(late(&s), 0, "switching to {title}");
    }
}

#[test]
fn closing_the_shown_tab_takes_the_boundary_of_the_tab_shown_next() {
    let (mut s, _dir) = two_tabs_with_logs();
    let id = tab_id(&s, s.app().tabs.active_idx);
    s.shell.app.request_leave(None, Leave::CloseTab(id));
    wait(&mut s, SETTLED);
    assert_eq!(shown_title(&s), "First", "the parked tab is shown");
    assert_eq!(late(&s), 0);
}

#[test]
fn closing_the_other_tabs_takes_the_boundary_of_the_tab_kept() {
    let (mut s, _dir) = two_tabs_with_logs();
    let first = s.app().tabs.entries.iter().position(|t| t.title == "First").unwrap();
    with_tabs(&mut s, |tabs, app| tabs.close_other_tabs(first, app));
    wait(&mut s, SETTLED);
    assert_eq!(shown_title(&s), "First", "the parked tab is shown");
    assert_eq!(late(&s), 0);
}

/// Home, then the library file that is already open in a parked tab: that tab is shown again, through the tab switch.
#[test]
fn opening_from_home_a_library_file_open_in_a_parked_tab_takes_its_boundary() {
    let (mut s, _dir) = library_sim();
    let file_id = s.app().lib_file.as_ref().expect("a library file").id.clone();
    make_the_shown_tabs_log(&s);
    open_tab(&mut s, "Second");
    with_tabs(&mut s, |tabs, app| tabs.enter_home(app));
    wait(&mut s, SETTLED);
    crate::home::apply(&mut s.shell.app, Act::Open(file_id.clone()));
    wait(&mut s, SETTLED);
    assert_eq!(s.app().lib_file.as_ref().map(|f| f.id.clone()), Some(file_id), "the file's tab is shown");
    assert_eq!(s.app().tabs.entries.len(), 2, "no tab was added for it");
    assert_eq!(late(&s), 0);
}

/// Home, then a file outside the library that is already open in a parked tab: that tab is shown again, through the tab switch.
#[test]
fn opening_from_home_an_outside_file_open_in_a_parked_tab_takes_its_boundary() {
    let (mut s, _dir, path) = external_sim_with(&two_rects());
    make_the_shown_tabs_log(&s);
    open_tab(&mut s, "Second");
    with_tabs(&mut s, |tabs, app| tabs.enter_home(app));
    wait(&mut s, SETTLED);
    s.shell.app.open_path(path.clone());
    wait(&mut s, SETTLED);
    assert_eq!(s.app().file_path.as_deref(), Some(path.as_path()), "the file's tab is shown");
    assert_eq!(s.app().tabs.entries.len(), 2, "no tab was added for it");
    assert_eq!(late(&s), 0);
}

/// Home from an untitled tab, then a panel that leaves Home without opening anything: that tab is shown again.
#[test]
fn leaving_home_without_opening_a_file_takes_the_boundary_of_the_tab_shown_again() {
    let (mut s, _dir) = two_tabs_with_logs();
    open_tab(&mut s, "Untitled");
    make_the_shown_tabs_log(&s);
    let tabs = s.app().tabs.entries.len();
    with_tabs(&mut s, |tabs, app| tabs.enter_home(app));
    wait(&mut s, SETTLED);
    crate::home::apply(&mut s.shell.app, Act::OpenSpecs);
    wait(&mut s, SETTLED);
    assert!(s.app().home.is_none(), "Home is left");
    assert_eq!(s.app().tabs.entries.len(), tabs, "no tab was added");
    assert_eq!(shown_title(&s), "Untitled", "the tab shown before Home is shown again");
    assert_eq!(late(&s), 0);
}
