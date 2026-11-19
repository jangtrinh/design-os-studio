//! Whether a main component still has the content it had when the file was opened.
//!
//! The derived data a `.fig` stores for an instance (sizes, transforms and outlines of the nodes inside it) is only true
//! for the main component it was computed from. The page loader fingerprints the mains of the instances it holds derived
//! data for when the file is opened; a save fingerprints them again and writes the derived data only where the two agree.
//!
//! The document has a dirty log, but it cannot answer this: it is a bounded ring made for the renderer, and loading a
//! page invalidates it as a whole, so a save after the user opened any page could not tell an edited main from an
//! untouched one. A fingerprint compares content, whatever was done to the document in between.
//!
//! The page loader writes into mains itself: opening a page expands the instances that sit inside a main of that page
//! and the nested instances inside the mains its instances use. That is not an edit, but it changes the content. So the
//! loader keeps the content each main held when the file was opened as [`MainTrust`], and moves it forward over its own
//! writes, and only where the main still held the trusted content just before it wrote. A main edited by anyone else
//! stays different from its trusted content for good, whatever the loader does to it afterwards.

use designos_doc::branch_diff::node_content_hash;
use designos_doc::{Document, NodeId, NodeKind};
use std::collections::{BTreeSet, HashMap, HashSet};
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
    /// The mains scanned so far, as the content to trust for them.
    pub(crate) fn into_trust(self) -> MainTrust {
        MainTrust { mains: self.subtrees }
    }

    fn subtree(&mut self, doc: &Document, root: NodeId) -> &(u64, Vec<NodeId>) {
        self.subtrees.entry(root).or_insert_with(|| scan(doc, root))
    }

    /// The fingerprint of a set of mains: their own content together with the content of every main component that
    /// instances inside any of them name, and those mains' in turn. It changes when the set or any of them is edited.
    pub(crate) fn of_mains(&mut self, doc: &Document, mains: &[NodeId]) -> u64 {
        let roots: BTreeSet<NodeId> = mains.iter().copied().collect();
        let reached = self.reach(doc, &roots);
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

    /// The mains in `roots` and every main that instances inside them name, and those mains' in turn.
    fn reach(&mut self, doc: &Document, roots: &BTreeSet<NodeId>) -> BTreeSet<NodeId> {
        let mut reached = BTreeSet::new();
        let mut queue: Vec<NodeId> = roots.iter().copied().collect();
        while let Some(m) = queue.pop() {
            if reached.insert(m) {
                queue.extend(self.subtree(doc, m).1.iter().copied());
            }
        }
        reached
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

/// The content of each main component an instance's derived data depends on: its subtree hash and the mains the
/// instances inside it name, as they were when the file was opened and as the page loader left them since.
#[derive(Default)]
pub(crate) struct MainTrust {
    mains: HashMap<NodeId, (u64, Vec<NodeId>)>,
}

impl MainTrust {
    /// Whether the roots an instance depends on are the ones it had (`stored`) and `doc` holds the trusted content for
    /// every main that `now` reaches.
    pub(crate) fn holds(&self, fps: &mut Fingerprints, doc: &Document, stored: &[NodeId], now: &[NodeId]) -> bool {
        if stored.iter().collect::<BTreeSet<_>>() != now.iter().collect::<BTreeSet<_>>() {
            return false;
        }
        let mut seen = BTreeSet::new();
        let mut queue: Vec<NodeId> = now.to_vec();
        while let Some(m) = queue.pop() {
            if !seen.insert(m) {
                continue;
            }
            let (hash, named) = fps.subtree(doc, m);
            if self.mains.get(&m).map(|t| t.0) != Some(*hash) {
                return false;
            }
            queue.extend(named.iter().copied());
        }
        true
    }

    /// The fingerprint [`Fingerprints::of_mains`] gives for `roots` on a document that holds exactly the trusted
    /// content. `None` when a main they reach has no trusted content.
    pub(crate) fn fingerprint(&self, roots: &[NodeId]) -> Option<u64> {
        let roots: BTreeSet<NodeId> = roots.iter().copied().collect();
        let mut reached = BTreeSet::new();
        let mut queue: Vec<NodeId> = roots.iter().copied().collect();
        while let Some(m) = queue.pop() {
            if reached.insert(m) {
                queue.extend(self.mains.get(&m)?.1.iter().copied());
            }
        }
        let mut h = DefaultHasher::new();
        for m in &roots {
            h.write_u64(m.0);
        }
        h.write_u8(0xff);
        for m in reached {
            h.write_u64(m.0);
            h.write_u64(self.mains.get(&m)?.0);
        }
        Some(h.finish())
    }

    /// Moves the trusted content of the mains in `writes` to what `doc` holds now, for each one that held the trusted
    /// content when the loader started writing into it.
    pub(crate) fn advance(&mut self, doc: &Document, writes: &LoaderWrites) {
        for (&main, &before) in &writes.before {
            if self.mains.get(&main).map(|t| t.0) == Some(before) {
                self.mains.insert(main, scan(doc, main));
            }
        }
    }
}

/// The mains the page loader is about to write into, each hashed just before its first write.
#[derive(Default)]
pub(crate) struct LoaderWrites {
    watched: HashSet<NodeId>,
    before: HashMap<NodeId, u64>,
}

impl LoaderWrites {
    pub(crate) fn new(trust: &MainTrust) -> Self {
        Self { watched: trust.mains.keys().copied().collect(), before: HashMap::new() }
    }

    /// Notes main component `main` before the loader writes into it.
    pub(crate) fn about_to_write(&mut self, doc: &Document, main: NodeId) {
        if self.watched.contains(&main) && !self.before.contains_key(&main) {
            self.before.insert(main, scan(doc, main).0);
        }
    }

    /// Notes the main components that contain node `id`, before the loader writes into it.
    pub(crate) fn about_to_write_inside(&mut self, doc: &Document, id: NodeId) {
        let mut cur = doc.get(id).and_then(|n| n.parent);
        while let Some(p) = cur {
            let Some(n) = doc.get(p) else { break };
            if matches!(n.kind, NodeKind::Component(_)) {
                self.about_to_write(doc, p);
            }
            cur = n.parent;
        }
    }
}
