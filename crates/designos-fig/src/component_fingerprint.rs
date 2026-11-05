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

    /// The fingerprint of `main`: its own content together with the content of every main component that instances
    /// inside it name, and those mains' in turn. It changes when any of them is edited.
    pub(crate) fn of_main(&mut self, doc: &Document, main: NodeId) -> u64 {
        let mut reached = BTreeSet::new();
        let mut queue = vec![main];
        while let Some(m) = queue.pop() {
            if reached.insert(m) {
                queue.extend(self.subtree(doc, m).1.iter().copied());
            }
        }
        let mut h = DefaultHasher::new();
        for m in reached {
            h.write_u64(m.0);
            h.write_u64(self.subtree(doc, m).0);
        }
        h.finish()
    }
}
