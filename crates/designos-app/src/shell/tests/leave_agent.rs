//! Agent-originated leave never opens the unsaved-changes dialog (AC-04, AC-10).

use super::leave_prompt::{add_an_unsaved_untitled_tab, asks_about, dialog_title, make_two_tabs, tab_id};
use super::unsaved_dot::{dot, external_sim_with, nodes_on_disk, two_rects, wait, SETTLED};
use crate::home::{Leave, LeaveFrom};

#[test]
fn agent_close_of_a_dirty_tab_never_asks() {
    let (mut s, _d, path) = external_sim_with(&two_rects());
    make_two_tabs(&mut s);
    let on_disk = nodes_on_disk(&path);
    let id = tab_id(&s, 0);
    assert_eq!(LeaveFrom::Agent, LeaveFrom::Agent);
    s.shell.app.request_leave_from_agent(None, Leave::CloseTab(id));
    wait(&mut s, SETTLED);
    assert!(s.app().dialog.is_none(), "agent close opened a dialog: {:?}", dialog_title(&s));
    assert!(!s.app().leave.is_asking(), "agent close set the asking state");
    assert_eq!(s.app().tabs.entries.len(), 1, "the tab closed");
    assert_eq!(nodes_on_disk(&path), on_disk, "discard leaves the file");
}

#[test]
fn human_close_of_a_dirty_tab_still_asks() {
    let (mut s, _d, _path) = external_sim_with(&two_rects());
    make_two_tabs(&mut s);
    let id = tab_id(&s, 0);
    s.shell.app.request_leave(None, Leave::CloseTab(id));
    assert!(asks_about(&s, "Outside"), "{:?}", dialog_title(&s));
    assert!(s.app().leave.is_asking());
    assert_eq!(s.app().tabs.entries.len(), 2, "nothing closes until answered");
}

#[test]
fn agent_close_of_one_tab_never_starts_quit_or_asks_about_other_dirty_tabs() {
    let (mut s, _d, path) = external_sim_with(&two_rects());
    make_two_tabs(&mut s);
    add_an_unsaved_untitled_tab(&mut s);
    assert_eq!(s.app().tabs.entries.len(), 3);
    let first = tab_id(&s, 0);
    s.shell.app.request_leave_from_agent(None, Leave::CloseTab(first));
    wait(&mut s, SETTLED);
    assert!(s.app().dialog.is_none(), "{:?}", dialog_title(&s));
    assert!(!s.app().leave.is_asking());
    assert!(!s.app().leave.quit_approved, "quit did not start");
    assert_eq!(s.app().tabs.entries.len(), 2, "only the one tab closed");
    assert!(s.app().tabs.entries.iter().any(|t| t.title == "Untitled"), "the other dirty tab stayed");
    let _ = path;
}

#[test]
fn agent_quit_is_ignored_so_other_dirty_tabs_are_not_asked() {
    let (mut s, _d, _path) = external_sim_with(&two_rects());
    make_two_tabs(&mut s);
    add_an_unsaved_untitled_tab(&mut s);
    let n = s.app().tabs.entries.len();
    s.shell.app.request_leave_from_agent(None, Leave::Quit);
    assert!(!s.app().leave.quit_approved);
    assert!(!s.app().leave.is_asking());
    assert!(s.app().dialog.is_none());
    assert_eq!(s.app().tabs.entries.len(), n);
    let _ = dot(&s);
}
