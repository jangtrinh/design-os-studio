//! Checks a subtree from a patch line before it reaches the document. The log is a file any
//! local process can write, so it may hold cycles, orphans or ids that would exhaust the id space.

use crate::{Document, Node, NodeId, MAX_TREE_DEPTH, MIN_ID_HEADROOM};
use std::collections::{BTreeMap, BTreeSet};

/// Why `nodes` (root first) cannot be inserted under `parent`, or `None` when it can.
///
/// A node may list children outside the subtree: those are existing layers that later Reparent
/// ops of the same patch move in, and the insert drops them from its own lists.
pub(super) fn insert_problem(doc: &Document, parent: NodeId, nodes: &[Node]) -> Option<String> {
    let mut by_id: BTreeMap<NodeId, &Node> = BTreeMap::new();
    for n in nodes {
        if n.id.0 == 0 || n.id.0 > u64::MAX - MIN_ID_HEADROOM {
            return Some(format!("node id {} is outside the usable id range", n.id.0));
        }
        if by_id.insert(n.id, n).is_some() {
            return Some(format!("node {} appears twice", n.id.0));
        }
    }
    let root = &nodes[0];
    if root.parent.is_some_and(|p| by_id.contains_key(&p)) {
        return Some(format!("the first node {} is inside its own subtree", root.id.0));
    }
    for n in nodes {
        for c in n.children.iter().filter_map(|c| by_id.get(c)) {
            if c.parent != Some(n.id) || n.children.iter().filter(|x| **x == c.id).count() != 1 {
                return Some(format!("node {} is not listed exactly once by its own parent", c.id.0));
            }
        }
    }
    for n in &nodes[1..] {
        if !n.parent.is_some_and(|p| by_id.get(&p).is_some_and(|p| p.children.contains(&n.id))) {
            return Some(format!("node {} has no parent inside the inserted layers", n.id.0));
        }
    }
    let mut seen = BTreeSet::new();
    let mut stack = vec![(root.id, depth_of(doc, parent) + 1)];
    while let Some((id, depth)) = stack.pop() {
        if depth > MAX_TREE_DEPTH {
            return Some(format!("the inserted layers nest deeper than {MAX_TREE_DEPTH} levels"));
        }
        if !seen.insert(id) {
            return Some(format!("node {} is reached twice", id.0));
        }
        stack.extend(by_id[&id].children.iter().filter(|c| by_id.contains_key(c)).map(|c| (*c, depth + 1)));
    }
    (seen.len() != nodes.len()).then(|| "some inserted layers are not reachable from the first one".to_string())
}

/// Ancestors above `id` (a page is at depth 0), counted up to one past the limit.
fn depth_of(doc: &Document, id: NodeId) -> usize {
    let mut depth = 0;
    let mut cur = doc.get(id).and_then(|n| n.parent);
    while let Some(p) = cur.filter(|_| depth <= MAX_TREE_DEPTH) {
        depth += 1;
        cur = doc.get(p).and_then(|n| n.parent);
    }
    depth
}
