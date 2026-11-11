//! Tells the user why an edit that the component cycle guard refuses did nothing, or landed elsewhere.
//!
//! The guard itself lives in `designos_doc::components` and answers with an empty command, so the edit paths in the
//! app are silent unless they ask the same core helpers (`held_main_cycle_path_at`, `reparent_held_main_cycle_path`)
//! for the loop. The wording here is the user-facing form of the `INSTANCE_CYCLE` refusal that tool calls return:
//! the same facts (which main would hold itself, through which path) without the code.

use crate::app::App;
use crate::clipboard::Clip;
use designos_doc::{
    cycle_node_label, held_main_at, held_main_cycle_path_at, instance_component, mains_held_through, parse_variant_name, reparent_held_main_cycle_path,
    reparent_would_hold_itself, Document, NodeId, NodeKind, OverrideField, PropValue,
};
use std::collections::BTreeMap;

const ARROW: &str = " \u{2192} ";

fn refusal(doc: &Document, action: &str, held_main: NodeId, path: &[String]) -> String {
    format!("Can't {action}: {} would contain itself ({})", cycle_node_label(doc, held_main), path.join(ARROW))
}

/// Why `instance` cannot show `component`, when the cause is a main that would hold itself.
pub(crate) fn swap_refusal(doc: &Document, instance: NodeId, component: NodeId) -> Option<String> {
    let (held, path) = held_main_cycle_path_at(doc, component, instance, &[])?;
    Some(refusal(doc, "swap", held, &path))
}

/// Why an instance of `component` cannot go under `parent`.
pub(crate) fn insert_refusal(doc: &Document, component: NodeId, parent: NodeId) -> Option<String> {
    let (held, path) = held_main_cycle_path_at(doc, component, parent, &[])?;
    Some(refusal(doc, "add", held, &path))
}

/// Why the first of `ids` cannot move under `new_parent`.
pub(crate) fn move_refusal(doc: &Document, ids: &[NodeId], new_parent: NodeId) -> Option<String> {
    let id = ids.iter().copied().find(|id| *id != new_parent && !doc.is_ancestor(*id, new_parent) && reparent_would_hold_itself(doc, &[*id], new_parent))?;
    let (held, path) = reparent_held_main_cycle_path(doc, &[id], new_parent)?;
    Some(refusal(doc, "move", held, &path))
}

/// The sibling variant that choosing `choice` for the variant property `name` selects for `instance`, if any.
fn variant_choice_target(doc: &Document, instance: NodeId, name: &str, choice: &str) -> Option<NodeId> {
    let comp = doc.get(instance_component(doc.get(instance)?)?)?;
    let set = doc.get(comp.parent?).filter(|p| matches!(p.kind, NodeKind::ComponentSet(_)))?;
    let mut want: BTreeMap<String, String> = parse_variant_name(&comp.name).into_iter().collect();
    want.insert(name.to_string(), choice.to_string());
    set.children.iter().copied().find(|k| {
        doc.get(*k).is_some_and(|n| matches!(n.kind, NodeKind::Component(_)) && parse_variant_name(&n.name).into_iter().collect::<BTreeMap<_, _>>() == want)
    })
}

/// Why setting the property `name` of `instance` to `value` was not stored, when the cause is a main that would hold
/// itself: a variant choice, or an instance-swap property, that would swap an instance to such a main.
pub(crate) fn prop_refusal(doc: &Document, instance: NodeId, name: &str, value: &PropValue) -> Option<String> {
    match value {
        PropValue::Variant(choice) => {
            let target = variant_choice_target(doc, instance, name, choice)?;
            let (held, path) = held_main_cycle_path_at(doc, target, instance, &[])?;
            Some(refusal(doc, "switch variant", held, &path))
        }
        PropValue::Instance(component) => {
            let mut found = None;
            doc.walk(instance, |n, _| {
                if found.is_none() && n.component.bindings.get(&OverrideField::MainComponent).is_some_and(|p| p == name) {
                    found = held_main_cycle_path_at(doc, *component, n.id, &[]);
                }
            });
            let (held, path) = found?;
            Some(refusal(doc, "swap", held, &path))
        }
        _ => None,
    }
}

impl App {
    /// A refused edit: one error toast with the reason, when there is one.
    pub(crate) fn explain_refusal(&mut self, reason: Option<String>) {
        if let Some(reason) = reason {
            self.show_toast(reason, true);
        }
    }

    /// An instance of `component` that the guard moved from `parent` to the page: says which main it stays out of.
    pub(crate) fn explain_instance_placed_outside(&mut self, component: NodeId, parent: NodeId) {
        if let Some(held) = held_main_at(&self.doc, component, parent) {
            let text = format!("Added outside {}, which would contain itself", cycle_node_label(&self.doc, held));
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
                return Some(format!("Pasted outside {}, which would contain itself", cycle_node_label(&self.doc, main)));
            }
            main = up;
        }
        None
    }
}
