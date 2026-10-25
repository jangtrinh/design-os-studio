//! What an agent's document switch must not lose or touch: a close or quit while the switch waits behind a dialog asks
//! about the agent's document, and text typed for the user's layer never lands on the agent's layer that shares its id.

use super::leave_prompt::{asks_about, click};
use super::live_patch_agent_switch::agent_switches;
use super::live_patch_agent_switch_edges::agent_doc_n;
use super::unsaved_dot::{edit, external_sim_with, nodes_on_disk, two_rects, wait, SETTLED};
use super::*;
use designos_doc::NodeId;

/// The About dialog is open, which is no question about the document, when the agent opens another document: the switch
/// waits behind the dialog, so the agent's document is not a tab yet.
fn the_agent_opens_a_document_behind_the_about_dialog(s: &mut Sim) {
    s.shell.app.show_about();
    s.frame(vec![]);
    agent_switches(s, agent_doc_n("Agent opened this", 3));
    assert!(s.app().dialog.is_some(), "About is still open");
    assert_eq!(s.app().doc.name, "Outside", "the user's document is still on screen");
    assert!(s.app().terminal.live_patch.pending_doc.is_some(), "the agent's document is held");
}

/// The window's close is pressed while the switch is held: the agent's document is shown, then asked about like any other.
#[test]
fn a_close_while_the_agents_document_is_held_behind_a_dialog_asks_about_it() {
    let (mut s, _d, _path) = external_sim_with(&two_rects());
    the_agent_opens_a_document_behind_the_about_dialog(&mut s);
    s.close_frame();
    assert!(asks_about(&s, "Agent opened this"), "asked: {:?}", s.app().dialog.as_ref().map(|d| d.title.clone()));
    assert!(s.commands.contains(&egui::ViewportCommand::CancelClose), "the window stays open for the question");
    assert!(s.app().terminal.live_patch.pending_doc.is_none(), "the agent's document is not held any more");
    s.sent_commands.clear();
    click(&mut s, "dialog-alt");
    assert!(s.sent_commands.contains(&egui::ViewportCommand::Close), "answered: the close is asked for again");
}

/// The user's unsaved edit keeps its tab, in tab order before the agent's document: both are asked about.
#[test]
fn a_close_while_the_agents_document_is_held_asks_about_the_users_unsaved_edit_and_then_about_it() {
    let (mut s, _d, _path) = external_sim_with(&two_rects());
    edit(&mut s);
    the_agent_opens_a_document_behind_the_about_dialog(&mut s);
    s.close_frame();
    assert!(asks_about(&s, "Outside"), "the user's edit is asked about first");
    s.sent_commands.clear();
    click(&mut s, "dialog-alt");
    assert!(asks_about(&s, "Agent opened this"), "then the agent's document");
    assert!(!s.sent_commands.contains(&egui::ViewportCommand::Close), "the quit waits for the second answer");
    click(&mut s, "dialog-alt");
    assert!(s.sent_commands.contains(&egui::ViewportCommand::Close), "both answered: the close is asked for again");
}

/// A drag that is still going on holds the switch too (the document must not change under it). The close ends the drag
/// where it is, like it does for the questions, and the agent's document is then shown and asked about.
#[test]
fn a_close_during_a_drag_while_the_agents_document_is_held_still_asks_about_it() {
    let (mut s, _d, _path) = external_sim_with(&two_rects());
    let from = s.screen(25.0, 25.0);
    s.mv(from);
    s.button(true);
    s.mv(s.screen(60.0, 60.0));
    s.mv(s.screen(90.0, 90.0));
    assert!(matches!(s.app().canvas.inter, crate::canvas::Interaction::Move(_)), "a layer is being dragged");
    the_agent_opens_a_document_behind_the_about_dialog(&mut s);
    assert!(!matches!(s.app().canvas.inter, crate::canvas::Interaction::None), "the drag is still going on behind the dialog");
    s.close_frame();
    let mut asked = Vec::new();
    for _ in 0..4 {
        let Some(title) = s.app().dialog.as_ref().map(|d| d.title.clone()).filter(|t| t.starts_with("Save changes")) else { break };
        asked.push(title);
        click(&mut s, "dialog-alt");
    }
    assert_eq!(asked.last().map(String::as_str), Some("Save changes to Agent opened this?"), "asked: {asked:?}");
    assert!(s.app().terminal.live_patch.pending_doc.is_none(), "the agent's document is not held any more");
}

/// A value typed in a Properties field and not yet committed belongs to the user's layer. The agent's document replaces
/// a saved document in place and has a layer with the same id: the typed value must not end up on it.
#[test]
fn text_typed_for_the_users_layer_is_not_written_on_the_agents_layer_with_the_same_id() {
    let (mut s, _d, path) = external_sim_with(&two_rects());
    edit(&mut s);
    s.cmd_key(Key::S);
    wait(&mut s, SETTLED);
    let selected = s.app().selection.clone();
    assert!(!selected.is_empty(), "the edit left a layer selected");
    let on_disk = std::fs::read_to_string(&path).unwrap();

    let x = s.field_rect("x");
    s.click(egui::Pos2::new(x.left() + 40.0, x.center().y));
    s.type_text("4321");
    s.frame(vec![]);
    assert!(s.ctx.memory(|m| m.focused().is_some()), "the X field holds the typed text, not yet committed");

    let agent = agent_doc_n("Agent opened this", 8);
    assert!(selected.iter().all(|id| agent.get(*id).is_some()), "the agent's document has a layer with each selected id");
    let before: Vec<(NodeId, f32)> = agent.nodes.iter().map(|(id, n)| (*id, n.transform.tx)).collect();
    s.shell.app.terminal.live_patch.pending_doc = Some(agent);
    for _ in 0..40 {
        if s.app().doc.name == "Agent opened this" {
            break;
        }
        s.frame(vec![]);
    }
    assert_eq!(s.app().doc.name, "Agent opened this");
    assert_eq!(s.app().tabs.entries.len(), 1, "replaced in place: the document was saved");

    s.key(Key::Enter);
    wait(&mut s, SETTLED);
    let moved: Vec<NodeId> =
        before.iter().filter(|(id, tx)| s.app().doc.get(*id).is_none_or(|n| (n.transform.tx - tx).abs() > 1e-3)).map(|(id, _)| *id).collect();
    assert!(moved.is_empty(), "layers of the agent's document the typed value was written on: {moved:?}");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), on_disk, "and the user's file is untouched");
    assert_eq!(nodes_on_disk(&path), 4);
}
