//! Detached instances rule: a frame that carries `detachedSymbolId` provenance (set by a `.fig`
//! import) and is no longer an instance. When the main component is in this file and the frame
//! still equals it (apart from its name and where it sits), the fix swaps the frame for a fresh
//! instance of it. A frame that was edited after detaching is reported without a fix, because an
//! instance would silently throw those edits away. A copy of such a frame inside an instance is not
//! reported: it mirrors the detached frame of the main component, which is where it is reported
//! and reconnected once. A reconnect on the copy would be undone by the next sync of the instance.

use super::{Finding, Fix, Rule};
use designos_doc::{Affine, ComponentLink, Constraints, Document, LayoutChild, Node, NodeId, NodeKind};
use std::collections::HashMap;

/// The id part of a `detachedSymbolId` that can name a main component in this file: `guid:S:L` and
/// bare `S:L` match a component's imported `source_id`; `assetRef:` ids name a published library,
/// which this file may not contain.
fn local_source_id(symbol: &str) -> Option<&str> {
    if symbol.starts_with("assetRef:") {
        return None;
    }
    Some(symbol.strip_prefix("guid:").unwrap_or(symbol))
}

/// Main components of the file by imported `source_id`, built once per audit so resolving a
/// detached frame is a lookup instead of a scan of the whole document.
pub(super) struct Components {
    by_source_id: HashMap<String, NodeId>,
}

impl Components {
    pub(super) fn new(doc: &Document) -> Self {
        let mut by_source_id: HashMap<String, NodeId> = HashMap::new();
        for c in doc.nodes.values().filter(|c| matches!(c.kind, NodeKind::Component(_))) {
            let Some(source) = c.source_id.as_deref() else { continue };
            // Two components can share a source id after a paste; the lowest id wins, deterministically.
            by_source_id.entry(source.to_string()).and_modify(|id| *id = (*id).min(c.id)).or_insert(c.id);
        }
        Components { by_source_id }
    }

    /// The main component in this file that `symbol` points at.
    pub(super) fn resolve(&self, symbol: &str) -> Option<NodeId> {
        self.by_source_id.get(local_source_id(symbol)?).copied()
    }
}

/// Whether `component` is a main component of this file that `symbol` points at. Constant time, so
/// a fix can re-check its target without rebuilding [`Components`].
pub(super) fn is_main_component(doc: &Document, symbol: &str, component: NodeId) -> bool {
    let (Some(source), Some(c)) = (local_source_id(symbol), doc.get(component)) else { return false };
    matches!(c.kind, NodeKind::Component(_)) && c.source_id.as_deref() == Some(source)
}

/// The detached symbol of `n`, when it is a plain layer that still remembers one.
pub(super) fn symbol_of(n: &Node) -> Option<&str> {
    if matches!(n.kind, NodeKind::Instance { .. } | NodeKind::Component(_) | NodeKind::ComponentSet(_)) {
        return None;
    }
    n.component.detached_symbol_id.as_deref().or(n.detached_symbol_id.as_deref()).filter(|s| !s.is_empty())
}

/// Name an instance of `component` gets: its set's name for a variant, else its own.
pub(super) fn instance_name(doc: &Document, component: NodeId) -> String {
    let Some(c) = doc.get(component) else { return String::new() };
    c.parent.and_then(|p| doc.get(p)).filter(|p| matches!(p.kind, NodeKind::ComponentSet(_))).map_or_else(|| c.name.clone(), |p| p.name.clone())
}

/// A copy of `n` with everything that legitimately differs between a component and a detached copy
/// of it blanked out: identity, tree links, component bookkeeping and the Component/Frame kind.
/// For the root, also its name and placement, which a reconnect keeps from the frame.
fn comparable(n: &Node, root: bool) -> Node {
    let mut c = n.clone();
    c.id = NodeId(0);
    c.parent = None;
    c.children.clear();
    c.source_id = None;
    c.component = ComponentLink::default();
    c.detached_symbol_id = None;
    if let NodeKind::Component(props) = c.kind.clone() {
        c.kind = NodeKind::Frame(props);
    }
    if root {
        c.name.clear();
        c.transform = Affine::IDENTITY;
        c.constraints = Constraints::default();
        c.layout_child = LayoutChild::default();
        c.visible = true;
    }
    c
}

fn same_subtree(doc: &Document, a: NodeId, b: NodeId, root: bool) -> bool {
    let (Some(x), Some(y)) = (doc.get(a), doc.get(b)) else { return false };
    comparable(x, root) == comparable(y, root)
        && x.children.len() == y.children.len()
        && x.children.iter().zip(&y.children).all(|(p, q)| same_subtree(doc, *p, *q, false))
}

/// Whether the detached `frame` still equals `component`, layer for layer, apart from the frame's
/// name and placement. Only then is replacing it with an instance lossless.
pub(super) fn matches_component(doc: &Document, frame: NodeId, component: NodeId) -> bool {
    same_subtree(doc, frame, component, true)
}

pub(super) fn check(doc: &Document, components: &Components, n: &Node, out: &mut Vec<Finding>) {
    let Some(symbol) = symbol_of(n) else { return };
    // A layer inside an instance (`component.source`) is a mirror kept by the sync from its main.
    let mirrored = n.component.source.is_some();
    if mirrored && n.component.source.and_then(|s| doc.get(s)).and_then(symbol_of) == Some(symbol) {
        return;
    }
    let component = components.resolve(symbol).filter(|c| !doc.is_ancestor(*c, n.id));
    let (message, fix, fix_label) = match component {
        Some(c) => {
            let name = instance_name(doc, c);
            if mirrored {
                (format!("Detached from \"{name}\" inside an instance · reconnect on the main component"), None, String::new())
            } else if matches_component(doc, n.id, c) {
                (format!("Detached from component \"{name}\""), Some(Fix::Reconnect { component: c }), format!("Reconnect to {name}"))
            } else {
                (format!("Detached from \"{name}\" and edited since · no safe reconnect"), None, String::new())
            }
        }
        None => ("Detached from a component that is not in this file".to_string(), None, String::new()),
    };
    out.push(Finding { node: n.id, node_name: n.name.clone(), rule: Rule::DetachedInstance, message, fix, fix_label });
}
