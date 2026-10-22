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
use std::time::{Duration, Instant};

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
#[derive(Clone, Debug, Default)]
pub(super) struct Components {
    by_source_id: HashMap<String, NodeId>,
}

impl Components {
    #[cfg(test)]
    pub(super) fn new(doc: &Document) -> Self {
        let mut out = Self::default();
        for c in doc.nodes.values() {
            out.insert_node(c);
        }
        out
    }

    pub(super) fn insert_node(&mut self, c: &Node) {
        if !matches!(c.kind, NodeKind::Component(_)) {
            return;
        }
        let Some(source) = c.source_id.as_deref() else { return };
        // Two components can share a source id after a paste; the lowest id wins, deterministically.
        self.by_source_id.entry(source.to_string()).and_modify(|id| *id = (*id).min(c.id)).or_insert(c.id);
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

/// A comparison of a detached frame with its main component that can be paused: one pair of
/// layers per step, so a frame with tens of thousands of layers does not hold the UI thread.
pub(super) struct Comparison {
    pairs: Vec<(NodeId, NodeId, bool)>,
}

impl Comparison {
    pub(super) fn new(frame: NodeId, component: NodeId) -> Self {
        Self { pairs: vec![(frame, component, true)] }
    }

    /// Compares pairs of layers until `until` passes. `Some(equal)` once decided, `None` when it
    /// must be resumed. At least one pair is compared per call, so it always makes progress.
    pub(super) fn run(&mut self, doc: &Document, until: Instant) -> Option<bool> {
        while let Some((a, b, root)) = self.pairs.pop() {
            let (Some(x), Some(y)) = (doc.get(a), doc.get(b)) else { return Some(false) };
            if comparable(x, root) != comparable(y, root) || x.children.len() != y.children.len() {
                return Some(false);
            }
            self.pairs.extend(x.children.iter().zip(&y.children).map(|(p, q)| (*p, *q, false)));
            if !self.pairs.is_empty() && Instant::now() >= until {
                return None;
            }
        }
        Some(true)
    }
}

/// Whether the detached `frame` still equals `component`, layer for layer, apart from the frame's
/// name and placement. Only then is replacing it with an instance lossless.
pub(super) fn matches_component(doc: &Document, frame: NodeId, component: NodeId) -> bool {
    let mut comparison = Comparison::new(frame, component);
    loop {
        if let Some(equal) = comparison.run(doc, Instant::now() + Duration::from_secs(3600)) {
            return equal;
        }
    }
}

/// What the detached check decided for one layer.
pub(super) enum Detached {
    /// Nothing to report.
    Nothing,
    /// A finding that needs no comparison.
    Found(Finding),
    /// A plain detached frame of `component`: the finding depends on whether it still equals it,
    /// which [`Comparison`] decides (and [`compared`] turns into the finding).
    Compare { component: NodeId },
}

pub(super) fn plan(doc: &Document, components: &Components, n: &Node) -> Detached {
    let Some(symbol) = symbol_of(n) else { return Detached::Nothing };
    // A layer inside an instance (`component.source`) is a mirror kept by the sync from its main.
    let mirrored = n.component.source.is_some();
    if mirrored && n.component.source.and_then(|s| doc.get(s)).and_then(symbol_of) == Some(symbol) {
        return Detached::Nothing;
    }
    let component = components.resolve(symbol).filter(|c| !doc.is_ancestor(*c, n.id));
    let message = match component {
        Some(c) if mirrored => {
            let name = instance_name(doc, c);
            format!("Detached from \"{name}\" inside an instance · reconnect on the main component")
        }
        Some(c) => return Detached::Compare { component: c },
        None => "Detached from a component that is not in this file".to_string(),
    };
    Detached::Found(finding(n, message, None, String::new()))
}

/// The finding for a detached frame once it is known whether it still `equal`s its main component.
pub(super) fn compared(doc: &Document, n: &Node, component: NodeId, equal: bool) -> Finding {
    let name = instance_name(doc, component);
    if equal {
        finding(n, format!("Detached from component \"{name}\""), Some(Fix::Reconnect { component }), format!("Reconnect to {name}"))
    } else {
        finding(n, format!("Detached from \"{name}\" and edited since · no safe reconnect"), None, String::new())
    }
}

fn finding(n: &Node, message: String, fix: Option<Fix>, fix_label: String) -> Finding {
    Finding { node: n.id, node_name: n.name.clone(), rule: Rule::DetachedInstance, property: String::new(), message, fix, fix_label }
}
