//! Tells the user why an edit that the component cycle guard refuses did nothing, or landed elsewhere.
//!
//! The guard itself lives in `designos_doc::components` and answers with an empty command, so the edit paths in the
//! app are silent unless they ask the same core helpers (`held_main_cycle_path_at`, `reparent_held_main_cycle_path`)
//! for the loop. The sentences come from `designos_doc::cycle_refusal_message` and its siblings, the same ones that
//! the agent tools return, so one refused edit reads the same in the editor and over MCP.

use crate::app::App;
use crate::clipboard::Clip;
use designos_doc::{
    cycle_added_outside_message, cycle_kept_out_message, cycle_moved_outside_message, cycle_node_label, cycle_pasted_outside_message, cycle_refusal_message,
    held_main_at, held_main_cycle_path_at, instance_component, mains_held_through, reparent_held_main_cycle_path, reparent_would_hold_itself, CycleAction,
    Document, NodeId,
};

fn refusal(doc: &Document, action: CycleAction, held_main: NodeId, path: &[String]) -> String {
    cycle_refusal_message(action, &cycle_node_label(doc, held_main), path)
}

/// Why `instance` cannot show `component`, when the cause is a main that would hold itself.
pub(crate) fn swap_refusal(doc: &Document, instance: NodeId, component: NodeId) -> Option<String> {
    if instance_component(doc.get(instance)?) == Some(component) {
        return None;
    }
    let (held, path) = held_main_cycle_path_at(doc, component, instance, &[])?;
    Some(refusal(doc, CycleAction::Swap, held, &path))
}

/// Why an instance of `component` cannot go under `parent`.
pub(crate) fn insert_refusal(doc: &Document, component: NodeId, parent: NodeId) -> Option<String> {
    let (held, path) = held_main_cycle_path_at(doc, component, parent, &[])?;
    Some(refusal(doc, CycleAction::Add, held, &path))
}

/// What to say about the first of `ids` that cannot go under `new_parent`, by what became of the move.
pub(crate) struct MoveRefusal {
    /// Nothing moved.
    pub refused: String,
    /// Other layers moved, this one stayed out.
    pub kept_out: String,
    /// The layers went to another container instead.
    pub moved_outside: String,
}

pub(crate) fn move_refusal(doc: &Document, ids: &[NodeId], new_parent: NodeId) -> Option<MoveRefusal> {
    let id = ids.iter().copied().find(|id| *id != new_parent && !doc.is_ancestor(*id, new_parent) && reparent_would_hold_itself(doc, &[*id], new_parent))?;
    let (held, path) = reparent_held_main_cycle_path(doc, &[id], new_parent)?;
    let main = cycle_node_label(doc, held);
    Some(MoveRefusal {
        refused: refusal(doc, CycleAction::Move, held, &path),
        kept_out: cycle_kept_out_message(&cycle_node_label(doc, id), &main),
        moved_outside: cycle_moved_outside_message(&main),
    })
}

impl App {
    /// A move that the guard held back, told by what became of it: nothing moved (error), some layers moved and this
    /// one stayed out, or the layers went to another container.
    pub(crate) fn explain_move(&mut self, refusal: Option<MoveRefusal>, moved_any: bool) {
        match refusal {
            Some(r) if moved_any => self.show_toast(r.kept_out, false),
            Some(r) => self.show_toast(r.refused, true),
            None => {}
        }
    }

    /// A refused edit: one error toast with the reason, when there is one.
    pub(crate) fn explain_refusal(&mut self, reason: Option<String>) {
        if let Some(reason) = reason {
            self.show_toast(reason, true);
        }
    }

    /// An instance of `component` that the guard moved from `parent` to the page: says which main it stays out of.
    pub(crate) fn explain_instance_placed_outside(&mut self, component: NodeId, parent: NodeId) {
        if let Some(held) = held_main_at(&self.doc, component, parent) {
            let text = cycle_added_outside_message(&cycle_node_label(&self.doc, held));
            self.show_toast(text, false);
        }
    }

    /// What to tell the user about a paste that goes in `landed` instead of `container`, because `container` sits in a
    /// main that the pasted layers hold an instance of: which main it went outside of.
    pub(crate) fn paste_outside_notice(&self, clip: &Clip, container: NodeId, landed: NodeId) -> Option<String> {
        // Only a layer that holds an instance of that main is a loop. A main copied beside itself becomes an instance
        // of it, as always: that is no refusal to report.
        let shut = mains_held_through(&self.doc, clip.nodes.iter().filter_map(instance_component));
        if container == landed || !shut.iter().any(|m| container == *m || self.doc.is_ancestor(*m, container)) {
            return None;
        }
        let mut main = container;
        while let Some(up) = self.doc.get(main).and_then(|n| n.parent) {
            if up == landed {
                return Some(cycle_pasted_outside_message(&cycle_node_label(&self.doc, main)));
            }
            main = up;
        }
        None
    }
}

/// A frame named `name` on `parent`, made into a main component when `main` is set.
fn preset_frame(app: &mut App, parent: NodeId, rect: designos_doc::Rect, name: &str, main: bool) -> Option<NodeId> {
    let id = app.doc.alloc_id();
    let frame = app.doc.insert(parent, designos_doc::Node::frame(id, rect)).ok()?;
    app.doc.get_mut(frame)?.name = name.into();
    if main {
        designos_doc::create_component(&mut app.doc, frame).apply(&mut app.doc).ok()?;
    }
    Some(frame)
}

/// `--panel cycle-refusal-toast`: Row holds an instance of Card, and an icon inside Card is swapped to Row, which the
/// guard refuses. The toast is the real one that the swap raises.
pub fn apply_toast_preset(app: &mut App) {
    use designos_doc::{create_instance, Command, Rect, Vec2};
    let page = app.page;
    let (Some(card), Some(row), Some(icon)) = (
        preset_frame(app, page, Rect::new(0.0, 0.0, 240.0, 160.0), "Card", true),
        preset_frame(app, page, Rect::new(320.0, 0.0, 240.0, 160.0), "Row", true),
        preset_frame(app, page, Rect::new(0.0, 240.0, 48.0, 48.0), "Icon", true),
    ) else {
        return;
    };
    let place = |app: &mut App, component: NodeId, parent: NodeId, at: Vec2| -> Option<NodeId> {
        let Command::Insert { nodes, .. } = create_instance(&mut app.doc, component, parent, at) else { return None };
        let id = nodes.first()?.id;
        let index = app.doc.get(parent)?.children.len();
        Command::Insert { parent, index, nodes }.apply(&mut app.doc).ok()?;
        Some(id)
    };
    let Some(inside_card) = place(app, icon, card, Vec2::new(16.0, 16.0)) else { return };
    place(app, card, row, Vec2::new(16.0, 16.0));
    app.selection = vec![inside_card];
    app.swap_instance(inside_card, row);
}
