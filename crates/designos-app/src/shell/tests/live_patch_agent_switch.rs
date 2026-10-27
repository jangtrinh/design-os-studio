//! An agent that opens or creates another document (`designos_open_file`, `designos_new_document`) while the
//! document on screen has unsaved edits outside the library. Those edits stay in their own tab with their file;
//! the agent's document opens in a tab of its own, with the agent's terminal, and never takes the user's file.
//! A library file autosaves, and a document with nothing unsaved has nothing to protect.

use super::unsaved_dot::{dot, edit, external_sim_with, lib, library_sim, nodes_on_disk, two_rects, wait, SETTLED};
use super::*;
use crate::io::{DialogOutcome, ScriptedDialogs};
use designos_doc::{Document, Node, Rect as DRect};
use std::path::Path;

/// The document an agent opened: a page and one rectangle, so two nodes.
pub(super) fn agent_document() -> Document {
    let mut d = Document::new("Agent opened this");
    let page = d.add_page("P");
    let id = d.alloc_id();
    d.insert(page, Node::rectangle(id, DRect::new(0.0, 0.0, 10.0, 10.0))).unwrap();
    d
}

/// What the agent's switch leaves for the editor: the document it opened, waiting to be shown. (The log line that
/// announces it is not what is under test, so it is not written.)
pub(super) fn agent_switches(s: &mut Sim, doc: Document) {
    s.shell.app.terminal.live_patch.pending_doc = Some(doc);
    wait(s, SETTLED);
}

pub(super) fn tab_of_file(s: &Sim, path: &Path) -> Option<usize> {
    s.app().tabs.entries.iter().position(|t| t.file_path.as_deref() == Some(path))
}

fn tab_dot(s: &Sim, idx: usize) -> bool {
    let id = s.app().tabs.entries[idx].id;
    s.rect_of(&format!("tab-unsaved-dot-{id}")).is_finite()
}

/// The user's document, outside the library, has one edit no save has written: 4 nodes on screen, 3 in its file.
/// (Takes the sim by reference: a `Sim` is large, and a test thread's stack holds only a few copies of it.)
pub(super) fn edit_the_users_document(s: &mut Sim, path: &Path) {
    edit(s);
    assert_eq!(s.app().doc.nodes.len(), 4);
    assert_eq!(nodes_on_disk(path), 3);
    assert!(dot(s));
}

#[test]
fn an_agent_switch_keeps_the_unsaved_edits_in_their_own_tab_and_shows_the_agents_document_in_a_new_one() {
    let (mut s, _d, path) = external_sim_with(&two_rects());
    edit_the_users_document(&mut s, &path);
    agent_switches(&mut s, agent_document());

    assert_eq!(s.app().tabs.entries.len(), 2, "the user's document and the agent's, each in a tab");
    let user = tab_of_file(&s, &path).expect("the user's file is still open in a tab");
    assert_ne!(user, s.app().tabs.active_idx, "the agent's document is the one shown");
    let tab = &s.app().tabs.entries[user];
    assert_eq!(tab.doc.nodes.len(), 4, "the edit is still in the user's tab");
    assert!(crate::tabs::tab_is_unsaved(s.app(), user), "and it is still unsaved");
    assert!(tab_dot(&s, user), "the tab shows the dot");
    assert_eq!(s.app().doc.name, "Agent opened this");
    assert_eq!(s.app().doc.nodes.len(), 2);
    assert_eq!(s.app().tabs.entries[s.app().tabs.active_idx].title, "Agent opened this");

    // saving the user's tab writes the user's document to the user's file
    s.shell.app.switch_tab(user);
    wait(&mut s, SETTLED);
    assert_eq!(s.app().doc.nodes.len(), 4);
    assert_eq!(s.app().file_path.as_deref(), Some(path.as_path()));
    s.cmd_key(Key::S);
    wait(&mut s, SETTLED);
    assert_eq!(nodes_on_disk(&path), 4, "the user's edit reached the user's file");
    assert!(!dot(&s));
    assert_eq!(s.app().tabs.entries.len(), 2);
}

#[test]
fn the_agents_document_never_takes_the_users_file_path() {
    let (mut s, _d, path) = external_sim_with(&two_rects());
    edit_the_users_document(&mut s, &path);
    agent_switches(&mut s, agent_document());
    assert_eq!(s.app().file_path, None, "the agent's document has no file of its own yet");
    assert_eq!(s.app().watched_disk_path(), None, "and the user's file is not watched on its behalf");

    // Save on the agent's document asks where it goes; the user's file is not written
    let cancelled = ScriptedDialogs::answering(DialogOutcome::Cancelled);
    s.shell.app.save_with(false, &cancelled);
    wait(&mut s, SETTLED);
    assert_eq!(cancelled.requests.borrow().len(), 1, "Save asked where to put the agent's document");
    assert_eq!(nodes_on_disk(&path), 3, "the user's file is untouched");
    assert!(dot(&s), "the agent's document is not saved");

    // a new file of its own gets the agent's document, and the user's file is still untouched
    let elsewhere = path.with_file_name("agent-made.designos");
    s.shell.app.save_with(false, &ScriptedDialogs::answering(DialogOutcome::Picked(elsewhere.clone())));
    wait(&mut s, SETTLED);
    assert_eq!(nodes_on_disk(&elsewhere), 2);
    assert_eq!(nodes_on_disk(&path), 3, "the user's file is untouched");
    assert!(!dot(&s), "the agent's document is saved");
    let user = tab_of_file(&s, &path).expect("the user's tab");
    assert!(crate::tabs::tab_is_unsaved(s.app(), user), "the user's edit is still waiting for its own save");
}

#[test]
fn a_document_with_nothing_unsaved_is_replaced_in_its_tab_and_does_not_keep_its_file() {
    let (mut s, _d, path) = external_sim_with(&two_rects());
    assert!(!dot(&s));
    agent_switches(&mut s, agent_document());

    assert_eq!(s.app().tabs.entries.len(), 1, "nothing to protect, so no tab is added");
    assert_eq!(s.app().doc.name, "Agent opened this");
    assert_eq!(s.app().file_path, None, "the file the user had is not the agent's document's");
    assert!(dot(&s), "the agent's document is not in a file");
    let cancelled = ScriptedDialogs::answering(DialogOutcome::Cancelled);
    s.shell.app.save_with(false, &cancelled);
    assert_eq!(cancelled.requests.borrow().len(), 1, "Save asks where it goes");
    assert_eq!(nodes_on_disk(&path), 3, "the user's file is untouched");
}

/// The edit comes right before the switch, with no idle delay between them for autosave to run in.
#[test]
fn a_library_edit_made_just_before_the_agent_switch_is_written_first() {
    let (mut s, _d) = library_sim();
    let id = s.app().lib_file.as_ref().unwrap().id.clone();
    super::unsaved_dot::edit_now(&mut s);
    let edited = s.app().doc.nodes.len();
    assert_ne!(lib(&s).load_file(&id).unwrap().nodes.len(), edited, "the edit is not in the file yet");
    s.shell.app.terminal.live_patch.pending_doc = Some(agent_document());
    s.frame(vec![]);
    assert_eq!(s.app().doc.name, "Agent opened this", "the agent's document is on screen");
    assert_eq!(lib(&s).load_file(&id).unwrap().nodes.len(), edited, "the edit was written before the document went");
}

/// The agent's document is not the library file's: autosave, which writes the document on screen to its library
/// file, must never write it there.
#[test]
fn the_agents_document_does_not_overwrite_the_library_file() {
    let (mut s, _d) = library_sim();
    let id = s.app().lib_file.as_ref().unwrap().id.clone();
    edit(&mut s);
    let edited = s.app().doc.nodes.len();
    agent_switches(&mut s, agent_document());
    wait(&mut s, 2.5);
    assert_eq!(s.app().doc.name, "Agent opened this");
    assert_eq!(s.app().lib_file, None, "the agent's document is not a library file");
    assert_eq!(s.app().tabs.entries.len(), 1, "a library file autosaves: no tab is added to keep it");
    let on_disk = lib(&s).load_file(&id).unwrap();
    assert_eq!((on_disk.name.as_str(), on_disk.nodes.len()), ("DESIGN-OS Studio Starter", edited), "the user's file holds the user's document");
}

fn append_to_log(log: &Path, patch: &designos_doc::live_patch::LivePatch) {
    use std::io::Write;
    let line = serde_json::to_string(patch).unwrap() + "\n";
    std::fs::OpenOptions::new().create(true).append(true).open(log).unwrap().write_all(line.as_bytes()).unwrap();
}

/// A stand-in agent CLI that stays alive, so the terminal has a running agent tab, as it has while an agent works.
#[cfg(unix)]
pub(super) fn start_a_running_agent(s: &mut Sim, dir: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let bin = dir.join("agent-bin");
    std::fs::create_dir_all(&bin).unwrap();
    let script = bin.join("claude");
    std::fs::write(&script, "#!/bin/sh\nsleep 30\n").unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    s.shell.app.terminal.path_prepend.push(bin);
    s.shell.app.terminal.spawn_tab(crate::terminal::AgentCliPreset::Claude, None).expect("start the agent");
    assert_eq!(s.app().terminal.tabs.len(), 1);
}

/// The agent keeps working after the switch: its terminal, with its running agent, stays on screen and bound to the log
/// the agent writes to, and what the agent writes there lands on the document it opened, never on the user's parked
/// tab (whose layer ids it may share).
#[cfg(unix)]
#[test]
fn the_agents_terminal_follows_its_new_tab_and_later_patches_never_reach_the_users_tab() {
    use designos_doc::live_patch::{LiveOp, LivePatch};
    let (mut s, d, path) = external_sim_with(&two_rects());
    start_a_running_agent(&mut s, &d.0);
    edit_the_users_document(&mut s, &path);
    let doc = agent_document();
    let shared = doc.get(doc.pages[0]).unwrap().children[0];
    assert!(s.app().doc.get(shared).is_some(), "the user's document has a layer with the agent's layer id, so a misrouted edit shows");
    let (log, session) = (s.app().terminal.live_patch.path.clone(), s.app().terminal.session_id.clone());
    s.shell.app.terminal.tabs[0].turn_start_undo_depth = s.app().hist.undo_depth();
    assert!(s.app().terminal.tabs[0].turn_start_undo_depth > 0);

    agent_switches(&mut s, doc);
    assert_eq!(s.app().doc.name, "Agent opened this");
    assert_eq!(s.app().terminal.tabs.len(), 1, "the running agent is in the terminal on screen");
    assert_eq!((&s.app().terminal.live_patch.path, &s.app().terminal.session_id), (&log, &session), "and it is still bound to the log the agent writes");
    assert_eq!(s.app().terminal.tabs[0].turn_start_undo_depth, 0, "a turn that began above the new, empty history begins at its start");
    let user = tab_of_file(&s, &path).expect("the user's tab");
    let users_terminal = &s.app().tabs.entries[user].terminal;
    assert!(users_terminal.tabs.is_empty(), "no agent runs in the user's tab");
    assert_ne!(users_terminal.live_patch.path, log, "the user's tab has a log of its own");

    append_to_log(&log, &LivePatch::new(vec![LiveOp::Remove { id: shared }]));
    wait(&mut s, SETTLED);
    assert!(s.app().doc.get(shared).is_none(), "the agent's later patch applied to the agent's document");
    assert_eq!(s.app().tabs.entries[user].doc.nodes.len(), 4);
    assert!(s.app().tabs.entries[user].doc.get(shared).is_some(), "and not to the user's tab");

    // the user's tab, shown, still holds its document, and the agent's tab its own
    let agent = s.app().tabs.active_idx;
    s.shell.app.switch_tab(user);
    wait(&mut s, SETTLED);
    assert_eq!(s.app().doc.nodes.len(), 4);
    assert!(s.app().doc.get(shared).is_some());
    s.shell.app.switch_tab(agent);
    wait(&mut s, SETTLED);
    assert_eq!(s.app().doc.name, "Agent opened this");
    assert!(s.app().doc.get(shared).is_none());
    assert_eq!((&s.app().terminal.live_patch.path, &s.app().terminal.session_id), (&log, &session));
    s.shell.app.terminal.shutdown();
}

/// A question about the unsaved edits of a tab names the tab by its id. The agent's document changes what that tab
/// holds, so it waits for the answer; after Cancel nothing was dropped and the document opens as usual.
#[test]
fn an_agent_switch_waits_for_an_open_save_changes_question() {
    use crate::home::Leave;
    let (mut s, _d, path) = external_sim_with(&two_rects());
    edit_the_users_document(&mut s, &path);
    let id = s.app().tabs.entries[s.app().tabs.active_idx].id;
    s.shell.app.request_leave(None, Leave::CloseTab(id));
    assert_eq!(s.app().dialog.as_ref().map(|d| d.title.as_str()), Some("Save changes to Outside?"));

    agent_switches(&mut s, agent_document());
    assert_eq!(s.app().doc.name, "Outside", "the question is about this document: it is still on screen");
    assert_eq!(s.app().tabs.entries.len(), 1);
    assert!(s.app().terminal.live_patch.pending_doc.is_some(), "the agent's document is held, not dropped");

    // answered where the user answers it: the dialog closes with the answer, and only then does the document go
    let cancel = s.rect_of("dialog-cancel");
    s.click(cancel.center());
    wait(&mut s, SETTLED);
    assert_eq!(s.app().doc.name, "Agent opened this");
    assert_eq!(s.app().tabs.entries.len(), 2);
    assert_eq!(nodes_on_disk(&path), 3);
    assert!(tab_of_file(&s, &path).is_some_and(|i| crate::tabs::tab_is_unsaved(s.app(), i)), "the user's edit is still in its tab");
}
