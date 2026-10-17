//! Detached instances rule: a frame that carries `detachedSymbolId` provenance (set by a `.fig`
//! import) and is no longer an instance. When the main component is in this file the fix swaps the
//! frame for a fresh instance of it.

use super::{Finding, Fix, Rule};
use designos_doc::{Document, Node, NodeId, NodeKind};

/// The main component in this file a `detachedSymbolId` points at. `guid:S:L` and bare `S:L`
/// match a component's imported `source_id`; `assetRef:` ids name a published library, which this
/// file may not contain.
pub(super) fn resolve(doc: &Document, symbol: &str) -> Option<NodeId> {
    if symbol.starts_with("assetRef:") {
        return None;
    }
    let id = symbol.strip_prefix("guid:").unwrap_or(symbol);
    doc.nodes.values().find(|c| matches!(c.kind, NodeKind::Component(_)) && c.source_id.as_deref() == Some(id)).map(|c| c.id)
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

pub(super) fn check(doc: &Document, n: &Node, out: &mut Vec<Finding>) {
    let Some(symbol) = symbol_of(n) else { return };
    let component = resolve(doc, symbol).filter(|c| !doc.is_ancestor(*c, n.id));
    let (message, fix, fix_label) = match component {
        Some(c) => {
            let name = instance_name(doc, c);
            (format!("Detached from component \"{name}\""), Some(Fix::Reconnect { component: c }), format!("Reconnect to {name}"))
        }
        None => ("Detached from a component that is not in this file".to_string(), None, String::new()),
    };
    out.push(Finding { node: n.id, node_name: n.name.clone(), rule: Rule::DetachedInstance, message, fix, fix_label });
}
