//! Whether a main component still has the content it had when the file was opened.
//!
//! The derived data a `.fig` stores for an instance (sizes, transforms and outlines of the nodes inside it) is only true
//! for the main component it was computed from. The page loader fingerprints the mains of the instances it holds derived
//! data for when the file is opened; a save fingerprints them again and writes the derived data only where the two agree.
//!
//! The document has a dirty log, but it cannot answer this: it is a bounded ring made for the renderer, and loading a
//! page invalidates it as a whole, so a save after the user opened any page could not tell an edited main from an
//! untouched one. A fingerprint compares content, whatever was done to the document in between.

use designos_doc::branch_diff::node_content_hash;
use designos_doc::{Document, NodeId, NodeKind};
use std::collections::{BTreeSet, HashMap};
use std::hash::{DefaultHasher, Hasher};

/// Fingerprints of main components of one document, each subtree hashed once.
#[derive(Default)]
pub(crate) struct Fingerprints {
    /// Root -> (hash of the subtree under it, the main components its instances name).
    subtrees: HashMap<NodeId, (u64, Vec<NodeId>)>,
}

/// Hashes the subtree under `root` (ids, content and order of every node) and lists the main components named by the
/// instances inside it.
fn scan(doc: &Document, root: NodeId) -> (u64, Vec<NodeId>) {
    let mut h = DefaultHasher::new();
    let mut named = BTreeSet::new();
    let mut stack = vec![root];
    while let Some(id) = stack.pop() {
        h.write_u64(id.0);
        let Some(n) = doc.get(id) else {
            h.write_u8(0);
            continue;
        };
        h.write_u64(node_content_hash(n));
        h.write_usize(n.children.len());
        if let NodeKind::Instance { component: Some(c), .. } = &n.kind {
            if *c != root {
                named.insert(*c);
            }
        }
        stack.extend(n.children.iter().rev().copied());
    }
    (h.finish(), named.into_iter().collect())
}

impl Fingerprints {
    fn subtree(&mut self, doc: &Document, root: NodeId) -> &(u64, Vec<NodeId>) {
        self.subtrees.entry(root).or_insert_with(|| scan(doc, root))
    }

    /// The fingerprint of a set of mains: their own content together with the content of every main component that
    /// instances inside any of them name, and those mains' in turn. It changes when the set or any of them is edited.
    pub(crate) fn of_mains(&mut self, doc: &Document, mains: &[NodeId]) -> u64 {
        let roots: BTreeSet<NodeId> = mains.iter().copied().collect();
        let mut reached = BTreeSet::new();
        let mut queue: Vec<NodeId> = roots.iter().copied().collect();
        while let Some(m) = queue.pop() {
            if reached.insert(m) {
                queue.extend(self.subtree(doc, m).1.iter().copied());
            }
        }
        let mut h = DefaultHasher::new();
        for m in &roots {
            h.write_u64(m.0);
        }
        h.write_u8(0xff);
        for m in reached {
            h.write_u64(m.0);
            h.write_u64(self.subtree(doc, m).0);
        }
        h.finish()
    }
}

/// The mains the derived data of the instance `inst` depends on: its main, the mains its file record swaps nested
/// instances to (`record_swaps`), and the mains its own instance-swap property values name.
pub(crate) fn instance_mains(doc: &Document, inst: NodeId, main: NodeId, record_swaps: &[NodeId]) -> Vec<NodeId> {
    let mut out = vec![main];
    out.extend(record_swaps.iter().copied());
    if let Some(n) = doc.get(inst) {
        out.extend(n.component.values.values().filter_map(|v| match v {
            designos_doc::PropValue::Instance(t) if t.0 != 0 && matches!(doc.get(*t).map(|m| &m.kind), Some(NodeKind::Component(_))) => Some(*t),
            _ => None,
        }));
    }
    out
}
