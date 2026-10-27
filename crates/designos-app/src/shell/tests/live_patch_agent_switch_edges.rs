//! Edges of an agent's document switch (`live_patch_agent_switch`): any open question holds it, a document replaced in
//! place starts a fresh undo history, a Stop that is pending at the switch still folds into one step, and the user's
//! tab, left behind while the agent works in another tab of the same window, neither claims "another window" nor
//! retries the file's session lock.

use super::live_patch_agent_switch::{agent_document, agent_switches, edit_the_users_document, tab_of_file};
use super::unsaved_dot::{edit, external_sim_with, lib, library_sim, nodes_on_disk, two_rects, wait, SETTLED};
use super::*;
use crate::io::{DialogOutcome, ScriptedDialogs};
use designos_doc::live_patch::{LiveOp, LivePatch};
use designos_doc::{Document, Node, NodeId, Rect as DRect};
use std::path::Path;

/// A document an agent opened: a page and `n` rectangles.
pub(super) fn agent_doc_n(name: &str, n: usize) -> Document {
    let mut d = Document::new(name);
    let page = d.add_page("P");
    for i in 0..n {
        let id = d.alloc_id();
        d.insert(page, Node::rectangle(id, DRect::new(i as f32 * 20.0, 0.0, 10.0, 10.0))).unwrap();
    }
    d
}

fn dialog_title(s: &Sim) -> Option<String> {
    s.app().dialog.as_ref().map(|d| d.title.clone())
}

fn sorted_ids(doc: &Document) -> Vec<NodeId> {
    let mut ids: Vec<NodeId> = doc.nodes.keys().copied().collect();
    ids.sort();
    ids
}

/// Another program rewrites the user's file (8 nodes) while the user's edit is unsaved, and "File changed on disk" opens.
fn another_program_rewrites_the_file(s: &mut Sim, path: &Path) {
    std::thread::sleep(std::time::Duration::from_millis(30));
    std::fs::write(path, agent_doc_n("Written by another program", 7).to_json().unwrap()).unwrap();
    for _ in 0..80 {
        s.frame(vec![]);
        if s.app().dialog.is_some() {
            break;
        }
    }
    assert_eq!(dialog_title(s).as_deref(), Some("File changed on disk"));
    assert_eq!(nodes_on_disk(path), 8);
}

/// The agent's document waits behind the open question: the user's document is still on screen and the question is
/// still about it.
fn assert_the_switch_is_held(s: &Sim) {
    assert_eq!(dialog_title(s).as_deref(), Some("File changed on disk"), "the question is still open");
    assert_eq!(s.app().doc.name, "Outside", "the question is about this document: it is still on screen");
    assert_eq!(s.app().tabs.entries.len(), 1);
    assert!(s.app().terminal.live_patch.pending_doc.is_some(), "the agent's document is held, not dropped");
}

/// "Use file" loads the file into the user's document before the agent's document takes the screen, so the answer is
/// not lost: the file stays as the user chose it, and the agent's document never overwrites it.
#[test]
fn an_agent_switch_waits_for_the_file_changed_on_disk_question_and_use_file_loads_the_file_first() {
    let (mut s, _d, path) = external_sim_with(&two_rects());
    edit_the_users_document(&mut s, &path);
    another_program_rewrites_the_file(&mut s, &path);

    agent_switches(&mut s, agent_document());
    assert_the_switch_is_held(&s);

    let use_file = s.rect_of("dialog-ok");
    s.click(use_file.center());
    wait(&mut s, SETTLED);
    assert_eq!(dialog_title(&s), None);
    assert_eq!(s.app().doc.name, "Agent opened this", "the answer is given: the agent's document is shown");
    assert_eq!(s.app().tabs.entries.len(), 1, "the file's version was loaded, so nothing of the user's is left to keep in a tab");
    assert_eq!(s.app().tabs.entries.iter().filter(|t| t.doc.nodes.len() == 4).count(), 0, "and the replaced edit is not held anywhere");
    assert_eq!(nodes_on_disk(&path), 8, "the version the user chose is on disk");
    assert_eq!(s.app().file_path, None, "the agent's document has no file");

    // a save of the agent's document asks where it goes and leaves the chosen version alone
    let cancelled = ScriptedDialogs::answering(DialogOutcome::Cancelled);
    s.shell.app.save_with(false, &cancelled);
    wait(&mut s, SETTLED);
    assert_eq!(cancelled.requests.borrow().len(), 1);
    assert_eq!(nodes_on_disk(&path), 8);
}

/// "Keep mine" keeps the user's edit in its tab, with its file, so its next save writes the user's version.
#[test]
fn an_agent_switch_waits_for_the_file_changed_on_disk_question_and_keep_mine_saves_the_users_version() {
    let (mut s, _d, path) = external_sim_with(&two_rects());
    edit_the_users_document(&mut s, &path);
    another_program_rewrites_the_file(&mut s, &path);

    agent_switches(&mut s, agent_document());
    assert_the_switch_is_held(&s);

    let keep_mine = s.rect_of("dialog-cancel");
    s.click(keep_mine.center());
    wait(&mut s, SETTLED);
    assert_eq!(s.app().doc.name, "Agent opened this");
    assert_eq!(s.app().tabs.entries.len(), 2, "the user's edit has a tab of its own");
    assert_eq!(nodes_on_disk(&path), 8, "nothing was written by the switch");

    let user = tab_of_file(&s, &path).expect("the user's tab holds the user's file");
    s.shell.app.switch_tab(user);
    wait(&mut s, SETTLED);
    assert_eq!(s.app().doc.nodes.len(), 4);
    assert_eq!(dialog_title(&s), None, "the answered question does not come back");
    s.cmd_key(Key::S);
    wait(&mut s, SETTLED);
    let on_disk = Document::from_json(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!((on_disk.name.as_str(), on_disk.nodes.len()), ("Outside", 4), "Save wrote the version the user kept");
}

/// An agent document with a layer for every id the user's last step touched: an undo that replays the user's step on
/// the agent's document would delete an agent layer.
fn the_agents_layers_share_ids_with_the_users_last_step(s: &mut Sim, name: &str) -> Document {
    let before = sorted_ids(&s.app().doc);
    edit(s);
    let touched: Vec<NodeId> = sorted_ids(&s.app().doc).into_iter().filter(|id| !before.contains(id)).collect();
    assert!(!touched.is_empty(), "the edit added layers");
    let mut doc = Document::new(name);
    let page = doc.add_page("P");
    for (i, &id) in touched.iter().enumerate() {
        doc.insert(page, Node::rectangle(id, DRect::new(i as f32 * 20.0, 0.0, 10.0, 10.0))).unwrap();
    }
    assert!(touched.iter().all(|id| doc.get(*id).is_some()), "the agent's document has a layer with each of those ids, so a misrouted undo shows");
    doc
}

fn assert_undo_leaves_the_agents_document_alone(s: &mut Sim, agent_doc_ids: &[NodeId]) {
    assert_eq!(s.app().doc.name, "Agent opened this");
    assert_eq!(s.app().hist.undo_depth(), 0, "the agent's document starts a history of its own");
    s.cmd_key(Key::Z);
    wait(s, SETTLED);
    assert_eq!(s.app().doc.name, "Agent opened this");
    assert_eq!(sorted_ids(&s.app().doc), agent_doc_ids, "undo took nothing from the agent's document, and added nothing");
}

#[test]
fn undo_after_the_agent_replaces_a_saved_document_in_place_leaves_the_agents_document_alone() {
    let (mut s, _d, path) = external_sim_with(&two_rects());
    let doc = the_agents_layers_share_ids_with_the_users_last_step(&mut s, "Agent opened this");
    let ids = sorted_ids(&doc);
    s.cmd_key(Key::S);
    wait(&mut s, SETTLED);
    assert_eq!(nodes_on_disk(&path), 4, "saved, so nothing is unsaved and the document is replaced in its tab");
    assert!(s.app().hist.undo_depth() > 0, "the user's history is there before the switch");
    agent_switches(&mut s, doc);
    assert_eq!(s.app().tabs.entries.len(), 1);
    assert_undo_leaves_the_agents_document_alone(&mut s, &ids);
    assert_eq!(nodes_on_disk(&path), 4, "and the user's file is untouched");
}

#[test]
fn undo_after_the_agent_replaces_a_library_document_in_place_leaves_the_agents_document_alone() {
    let (mut s, _d) = library_sim();
    let id = s.app().lib_file.as_ref().unwrap().id.clone();
    let doc = the_agents_layers_share_ids_with_the_users_last_step(&mut s, "Agent opened this");
    let ids = sorted_ids(&doc);
    wait(&mut s, 2.5);
    let on_disk = lib(&s).load_file(&id).unwrap().nodes.len();
    assert_eq!(on_disk, s.app().doc.nodes.len(), "the library file is written, so the document is replaced in its tab");
    assert!(s.app().hist.undo_depth() > 0, "the user's history is there before the switch");
    agent_switches(&mut s, doc);
    assert_eq!(s.app().tabs.entries.len(), 1);
    assert_undo_leaves_the_agents_document_alone(&mut s, &ids);
    assert_eq!(lib(&s).load_file(&id).unwrap().nodes.len(), on_disk, "and the library file is untouched");
}

/// The turn the running agent began above the history that was left behind begins at the start of the new one, as it
/// does when the user's document is kept in a tab of its own.
#[cfg(unix)]
#[test]
fn a_running_turn_begins_at_the_start_of_the_history_of_a_document_replaced_in_place() {
    let (mut s, d, _path) = external_sim_with(&two_rects());
    super::live_patch_agent_switch::start_a_running_agent(&mut s, &d.0);
    edit(&mut s);
    s.cmd_key(Key::S);
    wait(&mut s, SETTLED);
    // The agent's turn starts: the bridge notes the depth of the history it starts on, as it does for a real turn.
    *s.shell.app.terminal.tabs[0].agent_status.lock().unwrap() = crate::terminal::AgentTabStatus::Working(None);
    s.frame(vec![]);
    assert!(crate::agent_stop::is_tab_turn_running(&s.app().terminal.tabs[0]), "the turn is running");
    assert!(s.app().terminal.tabs[0].turn_start_undo_depth > 0, "it began above the start of the user's history");
    agent_switches(&mut s, agent_doc_n("Agent opened this", 3));
    assert_eq!(s.app().tabs.entries.len(), 1, "replaced in place");
    assert!(crate::agent_stop::is_tab_turn_running(&s.app().terminal.tabs[0]), "the turn is still running");
    assert_eq!(s.app().terminal.tabs[0].turn_start_undo_depth, 0);
    s.shell.app.terminal.shutdown();
}

/// A Stop that is pending at the instant of the switch holds the undo depth the user's history had. The new history is
/// shorter, so the fold must start at its end of history: the agent's edits that follow fold into one step.
fn stop_pending_at_the_switch(s: &mut Sim) -> (usize, usize, Vec<Option<String>>) {
    let user_depth = s.app().hist.undo_depth();
    assert_eq!(user_depth, 3);
    let doc = agent_doc_n("Agent opened this", 3);
    let kids = doc.get(doc.pages[0]).unwrap().children.clone();
    s.shell.app.terminal.live_patch.pending_doc = Some(doc);
    s.shell.app.terminal.live_patch.pending = vec![LivePatch::new(vec![LiveOp::Remove { id: kids[0] }]), LivePatch::new(vec![LiveOp::Remove { id: kids[1] }])];
    s.shell.app.terminal.live_patch.pending_offsets = vec![0, 1];
    s.shell.app.terminal.pending_stop_fold_depth = Some(user_depth);
    s.frame(vec![]);
    assert_eq!(s.app().doc.name, "Agent opened this");
    assert_eq!(s.app().doc.nodes.len(), 2, "the agent's two edits were applied to its document");
    (user_depth, s.app().hist.undo_depth(), s.app().hist.undo_labels().map(|l| l.map(str::to_string)).collect())
}

#[test]
fn a_stop_pending_at_the_switch_folds_the_agents_edits_into_one_step_beside_the_users_tab() {
    let (mut s, _d, _path) = external_sim_with(&two_rects());
    edit(&mut s);
    edit(&mut s);
    edit(&mut s);
    let (_, depth, labels) = stop_pending_at_the_switch(&mut s);
    assert_eq!(s.app().tabs.entries.len(), 2, "the unsaved edits kept their own tab");
    assert_eq!((depth, labels), (1, vec![Some("Agent turn (stopped)".to_string())]));
}

#[test]
fn a_stop_pending_at_the_switch_folds_the_agents_edits_into_one_step_in_place() {
    let (mut s, _d, _path) = external_sim_with(&two_rects());
    edit(&mut s);
    edit(&mut s);
    edit(&mut s);
    s.cmd_key(Key::S);
    wait(&mut s, SETTLED);
    let (_, depth, labels) = stop_pending_at_the_switch(&mut s);
    assert_eq!(s.app().tabs.entries.len(), 1, "replaced in its tab");
    assert_eq!((depth, labels), (1, vec![Some("Agent turn (stopped)".to_string())]));
}

#[cfg(unix)]
mod users_tab_while_the_agent_runs {
    use super::super::live_patch_agent_switch::start_a_running_agent;
    use super::*;
    use crate::agent_sessions::{SessionKey, ANOTHER_WINDOW_OWNS_SESSION_MSG};
    use std::sync::{Arc, Mutex};

    /// The user's tab, with the unsaved edit and its file, is shown while the agent runs in its own tab of this window.
    fn the_users_tab_shown_while_the_agent_runs_in_another_tab(s: &mut Sim, dir: &Path, path: &Path) -> usize {
        start_a_running_agent(s, dir);
        edit_the_users_document(s, path);
        agent_switches(s, agent_document());
        assert_eq!(s.app().terminal.tabs.len(), 1, "the agent runs in the tab on screen");
        let agent = s.app().tabs.active_idx;
        let user = tab_of_file(s, path).expect("the user's tab");
        s.shell.app.switch_tab(user);
        wait(s, SETTLED);
        assert_eq!(s.app().doc.nodes.len(), 4, "the user's tab is shown");
        agent
    }

    fn shut_everything_down(s: &mut Sim) {
        s.shell.app.terminal.shutdown();
        for t in &mut s.shell.app.tabs.entries {
            t.terminal.shutdown();
        }
    }

    /// The agent is in this window, in another tab, so the window is not "another window"; and the lock that tab holds
    /// is not tried again on every painted frame. It is bound again once the agent's tab lets go.
    #[test]
    fn the_users_tab_does_not_claim_another_window_and_does_not_retry_the_lock_each_frame() {
        let (mut s, d, path) = external_sim_with(&two_rects());
        let agent = the_users_tab_shown_while_the_agent_runs_in_another_tab(&mut s, &d.0, &path);
        let mut other_window = 0;
        for _ in 0..10 {
            s.shell.app.terminal.status_msg = None;
            s.frame(vec![]);
            if s.app().terminal.status_msg.as_deref() == Some(ANOTHER_WINDOW_OWNS_SESSION_MSG) {
                other_window += 1;
            }
        }
        assert_eq!(other_window, 0, "frames on the user's tab that said another window owns the file");
        assert_ne!(s.app().terminal.status_msg.as_deref(), Some(ANOTHER_WINDOW_OWNS_SESSION_MSG));
        let t = &s.app().terminal;
        assert!(matches!(t.key, SessionKey::Fallback(_)), "the lock is not tried: the user's tab stays on its own folder");
        assert!(t.open_file_binding.is_none() && t.lock_owner.is_none());

        // the agent's tab lets go of the file: the user's tab takes it on the next frame
        s.shell.app.tabs.entries[agent].terminal.shutdown();
        s.frame(vec![]);
        assert!(s.app().terminal.lock_owner.is_some(), "the user's tab holds the file's session folder now");
        assert!(matches!(s.app().terminal.key, SessionKey::LibraryFile { .. }));
        shut_everything_down(&mut s);
    }

    /// Frames on the user's tab ask for no repaint while the agent works in the other tab.
    #[test]
    fn the_users_tab_asks_for_no_repaint_while_the_agent_runs_in_another_tab() {
        let (mut s, d, path) = external_sim_with(&two_rects());
        the_users_tab_shown_while_the_agent_runs_in_another_tab(&mut s, &d.0, &path);
        wait(&mut s, 5.0);
        let requests = Arc::new(Mutex::new(Vec::new()));
        let seen = requests.clone();
        s.ctx.set_request_repaint_callback(move |info| seen.lock().unwrap().push(info.delay));
        let mut per_frame = Vec::new();
        for _ in 0..5 {
            requests.lock().unwrap().clear();
            s.frame(vec![]);
            per_frame.push(requests.lock().unwrap().clone());
        }
        assert!(per_frame.iter().all(|r| r.is_empty()), "repaint requests per idle frame: {per_frame:?}");
        shut_everything_down(&mut s);
    }
}
