//! Which layer a copy layer inside a nested component copy follows.
//!
//! A component copy stores its own layers, and each copy layer links (`ComponentLink::source`) to the layer one level
//! up: the layer of the copy it was taken from, or of the main when the copy sits directly in an instance of it. A page
//! copy of `O`, which holds a copy of `I`, which holds a copy of `A`, therefore links its innermost layer to the layer in
//! `O`'s copy of `I`, that one to the layer in `I`'s copy of `A`, and that one to `A`'s own layer. A sync compares the
//! children of a copy with the children of the layer it links to; a child whose link is not among them is taken for a
//! leftover, removed, and rebuilt from the main without the change a person made on it.
//!
//! A `.fig` file written by Studio names each copy layer by the authored layer in the innermost main, and an importer
//! that looks that name up across the whole file links a nested copy's layers past the levels in between. Figma resolves
//! a name inside the level it belongs to, one step per instance level; [`relink_copy_layers`] does the same for the links
//! of a document that already has them wrong, whether it was just imported or was saved natively by an earlier release.

use crate::{instance_component, Document, Node, NodeId, NodeKind};
use std::collections::{BTreeMap, BTreeSet};

/// Longest chain of links followed to an authored layer; a document that links past it is not followed any further.
const MAX_LINK_CHAIN: usize = 256;

/// The authored layer `id` stands for: a main component's own layer stands for itself, and a copy layer follows its links
/// one level up at a time, stopping before a step would reach a main component's root (an instance root links to the main
/// it shows, which is not a layer of anything). Two copies of one main inside a wrapper are two instance layers and so
/// two authored layers; the layers inside them share the main's layers but are only ever compared among siblings.
pub fn authored_layer(d: &Document, id: NodeId) -> NodeId {
    let mut cur = id;
    for _ in 0..MAX_LINK_CHAIN {
        let Some(s) = d.get(cur).and_then(|n| n.component.source) else { break };
        if s == cur || d.get(s).is_none_or(|sn| matches!(sn.kind, NodeKind::Component(_))) {
            break;
        }
        cur = s;
    }
    cur
}

/// The node whose children the children of `p` mirror: the layer `p` links to (the main for an instance root), or the
/// main `p` shows when that is another one than the layer it links to shows (a swapped nested instance mirrors its own
/// main). This is the node a sync compares the children of `p` with.
fn children_reference(d: &Document, p: &Node) -> Option<NodeId> {
    let linked = p.component.source.filter(|s| d.get(*s).is_some());
    let reference = linked.or_else(|| instance_component(p).filter(|c| d.get(*c).is_some()))?;
    let reference_shows = d.get(reference).and_then(instance_component);
    match instance_component(p) {
        Some(mine) if Some(mine) != reference_shows && d.get(mine).is_some() => Some(mine),
        _ => Some(reference),
    }
}

/// True when a child of `p` links to a layer that is not a child of `reference`.
fn has_child_off_the_reference(d: &Document, p: &Node, reference: NodeId) -> bool {
    p.children.iter().any(|k| {
        let source = d.get(*k).and_then(|kn| kn.component.source).and_then(|s| d.get(s));
        source.is_some_and(|sn| sn.parent != Some(reference))
    })
}

/// For each child of `ikids` that links outside `rkids` and stands for the same authored layer as exactly one child of
/// `rkids` that no other child links to, the child of `rkids` to link it to. A child with no such single match is left
/// out: two candidates, two children that want one candidate, or a candidate another child already links to.
fn matches_by_authored_layer(d: &Document, rkids: &[NodeId], ikids: &[NodeId]) -> Vec<(NodeId, NodeId)> {
    let reference_children: BTreeSet<NodeId> = rkids.iter().copied().collect();
    let taken: BTreeSet<NodeId> = ikids.iter().filter_map(|k| d.get(*k).and_then(|n| n.component.source)).filter(|s| reference_children.contains(s)).collect();
    let mut candidate_of: BTreeMap<NodeId, NodeId> = BTreeMap::new();
    let mut ambiguous: BTreeSet<NodeId> = BTreeSet::new();
    for rc in rkids.iter().filter(|rc| !taken.contains(*rc)) {
        let authored = authored_layer(d, *rc);
        if candidate_of.insert(authored, *rc).is_some() {
            ambiguous.insert(authored);
        }
    }
    let mut wanting: BTreeMap<NodeId, Vec<NodeId>> = BTreeMap::new();
    for k in ikids {
        let Some(s) = d.get(*k).and_then(|n| n.component.source) else { continue };
        if reference_children.contains(&s) || d.get(s).is_none() {
            continue;
        }
        wanting.entry(authored_layer(d, s)).or_default().push(*k);
    }
    wanting
        .into_iter()
        .filter(|(authored, ks)| ks.len() == 1 && !ambiguous.contains(authored))
        .filter_map(|(authored, ks)| candidate_of.get(&authored).map(|rc| (ks[0], *rc)))
        .collect()
}

/// Number of ancestors of `id`.
fn depth_of(d: &Document, id: NodeId) -> usize {
    std::iter::successors(d.get(id).and_then(|n| n.parent), |p| d.get(*p).and_then(|n| n.parent)).count()
}

/// Relinks every copy layer whose link skips a level (see the module docs) to the layer one level up that stands for the
/// same authored layer, and returns how many links changed. Values and override flags are not touched; a layer with no
/// single clear match keeps the link it has.
///
/// A parent is examined only when one of its children links outside what a sync compares them with, so a document whose
/// links are right (every file Figma wrote, and every file this release saved) is read once and left untouched. A parent
/// is relinked before its own children, because what the children are compared with is read from the parent's link.
pub fn relink_copy_layers(doc: &mut Document) -> usize {
    let mut queue: BTreeSet<(usize, NodeId)> = BTreeSet::new();
    for n in doc.nodes.values() {
        let may_mirror = n.component.source.is_some() || matches!(n.kind, NodeKind::Instance { .. });
        if !may_mirror || n.children.is_empty() {
            continue;
        }
        if children_reference(doc, n).is_some_and(|r| has_child_off_the_reference(doc, n, r)) {
            queue.insert((depth_of(doc, n.id), n.id));
        }
    }
    let mut relinked = 0;
    // A relinked child is queued below its parent, so the queue only ever moves down the tree and ends.
    while let Some((depth, p)) = queue.pop_first() {
        let Some(pn) = doc.get(p) else { continue };
        let Some(r) = children_reference(doc, pn) else { continue };
        let rkids = doc.get(r).map(|n| n.children.as_slice()).unwrap_or_default();
        let moves = matches_by_authored_layer(doc, rkids, &pn.children);
        for (k, rc) in moves {
            let Some(kn) = doc.get_mut(k) else { continue };
            kn.component.source = Some(rc);
            relinked += 1;
            if !kn.children.is_empty() {
                queue.insert((depth + 1, k));
            }
        }
    }
    relinked
}
