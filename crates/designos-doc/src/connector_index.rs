//! Per-document connector attachment index and shared incremental reroute planner.

use crate::*;
use std::cell::{Cell, RefCell};
use std::collections::hash_map::Entry;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::hash::{BuildHasherDefault, Hasher};
use std::ops::Deref;

#[cfg(test)]
mod reroute_candidate_tests;
#[cfg(test)]
mod scale_tests;

const MAX_INDEXED_DOCS: usize = 4;

/// Multiplicative hasher for dense integer ids; `NodeId` keys are trusted, so SipHash buys nothing.
#[derive(Default)]
struct IdHasher(u64);

impl Hasher for IdHasher {
    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 = (self.0 ^ u64::from(b)).wrapping_mul(0x9E37_79B9_7F4A_7C15);
        }
    }

    #[inline]
    fn write_u64(&mut self, i: u64) {
        let x = i.wrapping_mul(0x9E37_79B9_7F4A_7C15);
        self.0 = x ^ (x >> 32);
    }

    #[inline]
    fn finish(&self) -> u64 {
        self.0
    }
}

type IdSet = HashSet<NodeId, BuildHasherDefault<IdHasher>>;
type IdMap<V> = HashMap<NodeId, V, BuildHasherDefault<IdHasher>>;

/// Inline-first small vector holding up to 4 items on the stack before spilling to a heap [`Vec`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SmallVec<T: Copy + Default + PartialEq + Ord> {
    inline: [T; 4],
    len: usize,
    spill: Vec<T>,
}

impl<T: Copy + Default + PartialEq + Ord> Default for SmallVec<T> {
    fn default() -> Self {
        Self { inline: [T::default(); 4], len: 0, spill: Vec::new() }
    }
}

impl<T: Copy + Default + PartialEq + Ord> SmallVec<T> {
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn as_slice(&self) -> &[T] {
        if self.len <= 4 {
            &self.inline[..self.len]
        } else {
            &self.spill
        }
    }

    /// Inserts `value` unless present, keeping the list sorted.
    ///
    /// Appending a value above the current maximum (the order a rebuild visits ids in) is O(1);
    /// anything else costs a binary search plus one shift, never a scan or a re-sort.
    pub fn push_unique(&mut self, value: T) {
        let slice = self.as_slice();
        let pos = match slice.last() {
            Some(last) if *last < value => slice.len(),
            _ => match slice.binary_search(&value) {
                Ok(_) => return,
                Err(pos) => pos,
            },
        };
        if self.len < 4 {
            self.inline.copy_within(pos..self.len, pos + 1);
            self.inline[pos] = value;
        } else {
            if self.len == 4 {
                self.spill.extend_from_slice(&self.inline);
            }
            self.spill.insert(pos, value);
        }
        self.len += 1;
    }

    pub fn remove_value(&mut self, value: T) {
        let Ok(pos) = self.as_slice().binary_search(&value) else { return };
        if self.len <= 4 {
            self.inline.copy_within(pos + 1..self.len, pos);
            self.inline[self.len - 1] = T::default();
            self.len -= 1;
        } else {
            self.spill.remove(pos);
            self.len -= 1;
            if self.len == 4 {
                self.inline.copy_from_slice(&self.spill);
                self.spill.clear();
            }
        }
    }
}

impl<T: Copy + Default + PartialEq + Ord> Deref for SmallVec<T> {
    type Target = [T];

    fn deref(&self) -> &[T] {
        self.as_slice()
    }
}

/// Endpoint selector for a connector (`Start` or `End`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum End {
    Start,
    End,
}

/// Read-only result of [`plan_reroute`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ReroutePlan {
    pub updates: Vec<(NodeId, ConnectorGeom)>,
    pub rehomes: Vec<(NodeId, NodeId)>,
    pub detached: Vec<(NodeId, End)>,
    pub deferred: usize,
}

/// Diagnostic counters for [`ConnectorIndex`] operations on the current thread.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ConnectorIndexStats {
    pub rebuilds: u64,
    pub refreshes: u64,
    pub node_reads: u64,
    /// Connectors [`plan_reroute`] went on to read and route, i.e. not ruled out as lying inside a moved subtree.
    pub candidates: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ConnectorEntry {
    pub parent: Option<NodeId>,
    pub start: Option<NodeId>,
    pub end: Option<NodeId>,
}

/// Derived index mapping attached end nodes and parent containers to their connectors.
#[derive(Clone, Debug, Default)]
pub struct ConnectorIndex {
    derived_id: u64,
    epoch: u64,
    nodes_len: usize,
    pub connectors: BTreeMap<NodeId, ConnectorEntry>,
    pub by_end: HashMap<NodeId, SmallVec<NodeId>>,
    pub per_parent: HashMap<NodeId, SmallVec<NodeId>>,
}

#[derive(Default)]
struct IndexPool {
    entries: [Option<ConnectorIndex>; MAX_INDEXED_DOCS],
}

thread_local! {
    static POOL: RefCell<IndexPool> = RefCell::new(IndexPool::default());
    static STATS: Cell<ConnectorIndexStats> = const {
        Cell::new(ConnectorIndexStats { rebuilds: 0, refreshes: 0, node_reads: 0, candidates: 0 })
    };
}

#[inline]
fn record_node_read() {
    STATS.with(|s| {
        let mut cur = s.get();
        cur.node_reads += 1;
        s.set(cur);
    });
}

#[inline]
fn record_rebuild() {
    STATS.with(|s| {
        let mut cur = s.get();
        cur.rebuilds += 1;
        s.set(cur);
    });
}

#[inline]
fn record_candidate() {
    STATS.with(|s| {
        let mut cur = s.get();
        cur.candidates += 1;
        s.set(cur);
    });
}

#[inline]
fn record_refresh() {
    STATS.with(|s| {
        let mut cur = s.get();
        cur.refreshes += 1;
        s.set(cur);
    });
}

#[inline]
fn read_node(doc: &Document, id: NodeId) -> Option<&Node> {
    record_node_read();
    doc.get(id)
}

pub fn connector_index_stats() -> ConnectorIndexStats {
    STATS.with(|s| s.get())
}

pub fn reset_connector_index_stats() {
    STATS.with(|s| s.set(ConnectorIndexStats::default()));
}

pub fn clear_connector_index() {
    POOL.with(|p| *p.borrow_mut() = IndexPool::default());
    reset_connector_index_stats();
}

impl ConnectorIndex {
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.connectors.is_empty()
    }

    #[inline]
    pub fn contains_connector(&self, id: NodeId) -> bool {
        self.connectors.contains_key(&id)
    }

    #[inline]
    pub fn children_connectors(&self, parent: NodeId) -> &[NodeId] {
        self.per_parent.get(&parent).map(|v| v.as_slice()).unwrap_or(&[])
    }

    #[inline]
    pub fn connectors_for_end(&self, node: NodeId) -> &[NodeId] {
        self.by_end.get(&node).map(|v| v.as_slice()).unwrap_or(&[])
    }

    fn unindex_connector(&mut self, id: NodeId) {
        let Some(prev) = self.connectors.remove(&id) else { return };
        if let Some(p) = prev.parent {
            if let Some(list) = self.per_parent.get_mut(&p) {
                list.remove_value(id);
                if list.is_empty() {
                    self.per_parent.remove(&p);
                }
            }
        }
        for end_node in [prev.start, prev.end].into_iter().flatten() {
            if let Some(list) = self.by_end.get_mut(&end_node) {
                list.remove_value(id);
                if list.is_empty() {
                    self.by_end.remove(&end_node);
                }
            }
        }
    }

    fn insert_connector(&mut self, id: NodeId, parent: Option<NodeId>, start: Option<NodeId>, end: Option<NodeId>) {
        self.connectors.insert(id, ConnectorEntry { parent, start, end });
        if let Some(p) = parent {
            self.per_parent.entry(p).or_default().push_unique(id);
        }
        if let Some(s) = start {
            self.by_end.entry(s).or_default().push_unique(id);
        }
        if let Some(e) = end {
            self.by_end.entry(e).or_default().push_unique(id);
        }
    }

    /// Brings the lists in line with a connector's current parent and ends. A connector whose
    /// entry did not change (a geometry-only edit) is already listed, so it is left alone.
    fn reindex_connector(&mut self, id: NodeId, parent: Option<NodeId>, start: Option<NodeId>, end: Option<NodeId>) {
        if self.connectors.get(&id) == Some(&ConnectorEntry { parent, start, end }) {
            return;
        }
        self.unindex_connector(id);
        self.insert_connector(id, parent, start, end);
    }

    fn rebuild(&mut self, doc: &Document) {
        record_rebuild();
        self.derived_id = doc.derived_id();
        self.epoch = doc.mutation_epoch();
        self.nodes_len = doc.nodes.len();
        self.connectors.clear();
        self.by_end.clear();
        self.per_parent.clear();
        // `doc.nodes` yields ids in ascending order, so every list is built by appending.
        for (&id, n) in &doc.nodes {
            if let NodeKind::Connector(c) = &n.kind {
                self.insert_connector(id, n.parent, c.start.node, c.end.node);
            }
        }
    }

    fn refresh(&mut self, doc: &Document) {
        if self.derived_id != doc.derived_id() {
            self.rebuild(doc);
            return;
        }
        if self.epoch == doc.mutation_epoch() {
            if self.nodes_len != doc.nodes.len() {
                self.rebuild(doc);
            }
            return;
        }
        let mut dirty = Vec::new();
        let mut removed = Vec::new();
        if !doc.dirty_changes_since(self.epoch, &mut dirty, &mut removed) {
            self.rebuild(doc);
            return;
        }
        let delta = doc.nodes.len() as isize - self.nodes_len as isize;
        let min_delta = -(removed.len() as isize);
        let max_delta = dirty.len() as isize;
        if delta < min_delta || delta > max_delta || (delta > 0 && dirty.is_empty()) {
            self.rebuild(doc);
            return;
        }
        record_refresh();
        for id in removed {
            self.unindex_connector(id);
        }
        let mut extra = Vec::new();
        // Ids already queued: the dirty ones plus every descendant collected so far. Built on the
        // first subtree walk only, so a plain geometry edit never builds the set.
        let mut queued: Option<IdSet> = None;
        for &id in &dirty {
            match doc.get(id) {
                Some(n) => {
                    if let NodeKind::Connector(c) = &n.kind {
                        self.reindex_connector(id, n.parent, c.start.node, c.end.node);
                        continue;
                    }
                    self.unindex_connector(id);
                    if delta > 0 && !matches!(n.kind, NodeKind::Page { .. }) && !n.children.is_empty() {
                        let queued = queued.get_or_insert_with(|| dirty.iter().copied().collect());
                        let mut stack = n.children.clone();
                        while let Some(cid) = stack.pop() {
                            if queued.insert(cid) {
                                extra.push(cid);
                            }
                            if let Some(cn) = doc.get(cid) {
                                stack.extend(cn.children.iter().copied());
                            }
                        }
                    }
                }
                None => self.unindex_connector(id),
            }
        }
        for id in extra {
            match doc.get(id) {
                Some(Node { kind: NodeKind::Connector(c), parent, .. }) => self.reindex_connector(id, *parent, c.start.node, c.end.node),
                _ => self.unindex_connector(id),
            }
        }
        self.epoch = doc.mutation_epoch();
        self.nodes_len = doc.nodes.len();
    }

    /// Walks the touched nodes and their non-connector descendants. Connectors inside the subtree
    /// are end nodes like any other, so they are collected, but the walk never enters them.
    ///
    /// Returns `None` when the walk would visit more nodes than a scan of every connector costs;
    /// callers then scan the connectors instead. With `track`, the walk also records the deepest
    /// touched node above each walked node and connector, and the connector children of walked
    /// nodes, which is what lets the reroute planner rule out connectors without visiting them one
    /// by one.
    fn walk_touched(&self, doc: &Document, touched: &[NodeId], track: bool) -> Option<TouchedWalk> {
        let cap = self.connectors.len().saturating_mul(4).saturating_add(64);
        let mut walk = TouchedWalk { nodes: Vec::new(), root_of: IdMap::default(), inner: Vec::new(), children_agree: true };
        let mut stack: Vec<(NodeId, NodeId)> = Vec::with_capacity(touched.len().min(64));
        let mut seen = IdSet::default();
        for &t in touched {
            if seen.insert(t) {
                stack.push((t, t));
                if track && self.connectors.contains_key(&t) {
                    walk.root_of.insert(t, t);
                }
            }
        }
        let mut visited = 0usize;
        let mut parent_index_children = 0usize;
        let mut listed_touched_connectors = IdSet::default();
        while let Some((id, root)) = stack.pop() {
            walk.nodes.push(id);
            if self.connectors.contains_key(&id) {
                continue;
            }
            if track {
                walk.root_of.insert(id, root);
                parent_index_children += self.children_connectors(id).len();
            }
            visited += 1;
            if visited > cap {
                return None;
            }
            let Some(n) = read_node(doc, id) else { continue };
            if n.children.is_empty() {
                continue;
            }
            if visited + n.children.len() > cap {
                return None;
            }
            if track {
                walk.root_of.reserve(n.children.len());
                walk.inner.reserve(self.children_connectors(id).len());
            }
            for &child in &n.children {
                let Some(&entry) = self.connectors.get(&child) else {
                    if seen.insert(child) {
                        stack.push((child, root));
                    }
                    continue;
                };
                if !track {
                    if seen.insert(child) {
                        walk.nodes.push(child);
                    }
                    continue;
                }
                match walk.root_of.entry(child) {
                    Entry::Vacant(slot) => {
                        slot.insert(root);
                        walk.nodes.push(child);
                    }
                    // A touched connector is already a node and its own root; any other root means a repeat listing.
                    Entry::Occupied(slot) => walk.children_agree &= *slot.get() == child && listed_touched_connectors.insert(child),
                }
                walk.children_agree &= entry.parent == Some(id);
                walk.inner.push((child, entry, root));
            }
        }
        // With nothing misfiled or listed twice, the lists agree with `per_parent` when they have as many connectors.
        walk.children_agree &= walk.inner.len() == parent_index_children || !track;
        Some(walk)
    }

    /// Connectors with an end whose parent chain reaches a touched node, found by looking at every connector.
    fn scan_touching(&self, doc: &Document, touched: &[NodeId]) -> Vec<NodeId> {
        let touched_set: HashSet<NodeId> = touched.iter().copied().collect();
        let mut anc_cache: HashMap<NodeId, bool> = HashMap::new();
        let mut out = Vec::new();
        for (&cid, entry) in &self.connectors {
            let start_hit = entry.start.is_some_and(|s| has_touched_ancestor(doc, s, &touched_set, &mut anc_cache));
            let end_hit = !start_hit && entry.end.is_some_and(|e| has_touched_ancestor(doc, e, &touched_set, &mut anc_cache));
            if start_hit || end_hit {
                out.push(cid);
            }
        }
        out
    }

    /// Returns all connectors with an attached end on, or inside the subtree of, any node in `touched`.
    /// Connectors inside the subtree count as nodes of it.
    pub fn connectors_touching(&self, doc: &Document, touched: &[NodeId]) -> Vec<NodeId> {
        if touched.is_empty() || self.is_empty() {
            return Vec::new();
        }
        let mut out = Vec::new();
        if let Some(walk) = self.walk_touched(doc, touched, false) {
            for &id in &walk.nodes {
                out.extend_from_slice(self.connectors_for_end(id));
            }
        } else {
            out = self.scan_touching(doc, touched);
        }
        out.sort_unstable();
        out.dedup();
        out
    }

    /// Returns, in id order, the connectors a move of `touched` can change: the touched connectors
    /// themselves and those with an attached end on, or inside the subtree of, a touched node,
    /// except connectors lying entirely inside one moved subtree (parent and both ends strictly
    /// inside it), which move rigidly with it and keep their stored geometry. A connector that
    /// ends on another connector inside the subtree has an end that moved.
    ///
    /// Connectors that are children of a walked node are classified straight from the walk, one
    /// index entry each. The per-end-node lists are only read when some attached connector is not
    /// such a child, which the list lengths reveal without visiting any connector. When a
    /// container's children disagree with the parent index, or the subtree is too big to walk,
    /// every connector is looked at instead.
    fn reroute_candidates(&self, doc: &Document, touched: &[NodeId]) -> Vec<NodeId> {
        let mut out: Vec<NodeId> = Vec::new();
        let walk = self.walk_touched(doc, touched, true).filter(|walk| walk.children_agree);
        let mut inside = InsideCheck::new(self, doc, touched, walk.as_ref().map(|w| &w.root_of));
        match &walk {
            Some(walk) => {
                let mut accounted = 0usize;
                for &(cid, entry, parent_root) in &walk.inner {
                    let ends_on_walked = match (entry.start, entry.end) {
                        (Some(s), Some(e)) if s != e => usize::from(walk.root_of.contains_key(&s)) + usize::from(walk.root_of.contains_key(&e)),
                        (Some(x), _) | (_, Some(x)) => usize::from(walk.root_of.contains_key(&x)),
                        (None, None) => 0,
                    };
                    if ends_on_walked == 0 {
                        continue;
                    }
                    accounted += ends_on_walked;
                    if !inside.is_inside(entry, Some(parent_root)) {
                        out.push(cid);
                    }
                }
                let listed: usize = walk.nodes.iter().map(|&x| self.connectors_for_end(x).len()).sum();
                if listed != accounted {
                    let inner: IdSet = walk.inner.iter().map(|&(cid, ..)| cid).collect();
                    for &x in &walk.nodes {
                        for &cid in self.connectors_for_end(x) {
                            let Some(&entry) = self.connectors.get(&cid) else { continue };
                            if !inner.contains(&cid) && !inside.is_inside_resolving_parent(entry) {
                                out.push(cid);
                            }
                        }
                    }
                }
            }
            None => {
                for cid in self.scan_touching(doc, touched) {
                    let Some(&entry) = self.connectors.get(&cid) else { continue };
                    if !inside.is_inside_resolving_parent(entry) {
                        out.push(cid);
                    }
                }
            }
        }
        out.extend(touched.iter().copied().filter(|t| self.connectors.contains_key(t)));
        out.sort_unstable();
        out.dedup();
        out
    }
}

/// The touched nodes, their non-connector descendants and the connectors inside them, as found by
/// [`ConnectorIndex::walk_touched`].
struct TouchedWalk {
    /// Every node whose attached connectors count, once each: the walked nodes, the touched
    /// connectors and the connector children of walked nodes.
    nodes: Vec<NodeId>,
    /// The deepest touched node at or above each walked node and each connector inside one (only when tracked).
    root_of: IdMap<NodeId>,
    /// Connector children of walked nodes with their entries and the root above their parent (only when tracked).
    inner: Vec<(NodeId, ConnectorEntry, NodeId)>,
    /// Whether the connector children listed by the walked nodes are exactly the ones `per_parent` records
    /// for them: none listed twice, none under a parent they do not name, none missing (only when tracked).
    children_agree: bool,
}

/// Decides whether a connector lies entirely inside one moved subtree: its parent and both
/// attached ends sit strictly below the same touched node.
struct InsideCheck<'a> {
    index: &'a ConnectorIndex,
    doc: &'a Document,
    touched: &'a [NodeId],
    /// Walked-node roots; ends missing from it are resolved by climbing parents.
    roots: Option<&'a IdMap<NodeId>>,
    touched_set: Option<HashSet<NodeId>>,
    climbed: HashMap<NodeId, Option<NodeId>>,
}

impl<'a> InsideCheck<'a> {
    fn new(index: &'a ConnectorIndex, doc: &'a Document, touched: &'a [NodeId], roots: Option<&'a IdMap<NodeId>>) -> Self {
        Self { index, doc, touched, roots, touched_set: None, climbed: HashMap::new() }
    }

    /// Deepest touched node at or above `id`, found by climbing parents.
    fn climb(&mut self, id: NodeId) -> Option<NodeId> {
        let touched = self.touched;
        let touched_set = self.touched_set.get_or_insert_with(|| touched.iter().copied().collect());
        deepest_touched_ancestor(self.doc, id, touched_set, &mut self.climbed)
    }

    /// Deepest touched node at or above `id`. Nodes the walk did not reach have no touched ancestor,
    /// except connectors (the walk never enters them), which are climbed.
    fn root_of(&mut self, id: NodeId) -> Option<NodeId> {
        let Some(roots) = self.roots else { return self.climb(id) };
        match roots.get(&id) {
            Some(&root) => Some(root),
            None if self.index.connectors.contains_key(&id) => self.climb(id),
            None => None,
        }
    }

    /// `parent_root` is the deepest touched node at or above the connector's parent, if any.
    fn is_inside(&mut self, entry: ConnectorEntry, parent_root: Option<NodeId>) -> bool {
        let (Some(start), Some(end)) = (entry.start, entry.end) else { return false };
        let Some(root) = parent_root else { return false };
        if start == root || end == root {
            return false;
        }
        self.root_of(start) == Some(root) && self.root_of(end) == Some(root)
    }

    fn is_inside_resolving_parent(&mut self, entry: ConnectorEntry) -> bool {
        let parent_root = entry.parent.and_then(|p| self.root_of(p));
        self.is_inside(entry, parent_root)
    }
}

fn has_touched_ancestor(doc: &Document, id: NodeId, touched: &HashSet<NodeId>, cache: &mut HashMap<NodeId, bool>) -> bool {
    if touched.contains(&id) {
        return true;
    }
    if let Some(&res) = cache.get(&id) {
        return res;
    }
    let mut chain = Vec::new();
    let mut cur = Some(id);
    let mut hit = false;
    while let Some(nid) = cur {
        if touched.contains(&nid) {
            hit = true;
            break;
        }
        if let Some(&cached) = cache.get(&nid) {
            hit = cached;
            break;
        }
        chain.push(nid);
        cur = read_node(doc, nid).and_then(|n| n.parent);
    }
    for nid in chain {
        cache.insert(nid, hit);
    }
    hit
}

fn deepest_touched_ancestor(doc: &Document, id: NodeId, touched: &HashSet<NodeId>, cache: &mut HashMap<NodeId, Option<NodeId>>) -> Option<NodeId> {
    if touched.contains(&id) {
        return Some(id);
    }
    if let Some(&cached) = cache.get(&id) {
        return cached;
    }
    let mut chain = Vec::new();
    let mut cur = Some(id);
    let mut found = None;
    while let Some(nid) = cur {
        if touched.contains(&nid) {
            found = Some(nid);
            break;
        }
        if let Some(&cached) = cache.get(&nid) {
            found = cached;
            break;
        }
        chain.push(nid);
        cur = read_node(doc, nid).and_then(|n| n.parent);
    }
    for nid in chain {
        cache.insert(nid, found);
    }
    found
}

/// Calls `f` with the current thread's refreshed [`ConnectorIndex`] for `doc`.
pub fn with_index<R>(doc: &Document, f: impl FnOnce(&ConnectorIndex) -> R) -> R {
    let mut entry = POOL.with(|p| {
        let mut pool = p.borrow_mut();
        let did = doc.derived_id();
        if let Some(pos) = pool.entries.iter().position(|e| e.as_ref().is_some_and(|idx| idx.derived_id == did)) {
            let taken = pool.entries[pos].take().unwrap();
            pool.entries[..=pos].rotate_right(1);
            taken
        } else {
            pool.entries.rotate_right(1);
            pool.entries[0] = None;
            ConnectorIndex::default()
        }
    });
    entry.refresh(doc);
    let res = f(&entry);
    POOL.with(|p| {
        let mut pool = p.borrow_mut();
        if pool.entries[0].is_none() {
            pool.entries[0] = Some(entry);
        } else {
            pool.entries.rotate_right(1);
            pool.entries[0] = Some(entry);
        }
    });
    res
}

/// Returns all connectors with an attached end on, or inside the subtree of, any node in `touched`.
pub fn connectors_touching(doc: &Document, touched: &[NodeId]) -> Vec<NodeId> {
    with_index(doc, |idx| idx.connectors_touching(doc, touched))
}

fn transform_rect(m: Affine, r: Rect) -> Rect {
    let corners =
        [m.apply(Vec2::new(r.x, r.y)), m.apply(Vec2::new(r.x + r.w, r.y)), m.apply(Vec2::new(r.x + r.w, r.y + r.h)), m.apply(Vec2::new(r.x, r.y + r.h))];
    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for c in corners {
        x0 = x0.min(c.x);
        y0 = y0.min(c.y);
        x1 = x1.max(c.x);
        y1 = y1.max(c.y);
    }
    Rect::new(x0, y0, (x1 - x0).max(0.0), (y1 - y0).max(0.0))
}

fn anchor_envelope(a: &crate::board::Anchor, b: &crate::board::Anchor, pad: f32) -> Rect {
    let mut x0 = a.pos.x.min(b.pos.x);
    let mut y0 = a.pos.y.min(b.pos.y);
    let mut x1 = a.pos.x.max(b.pos.x);
    let mut y1 = a.pos.y.max(b.pos.y);
    for r in [a.rect, b.rect].into_iter().flatten() {
        x0 = x0.min(r.x);
        y0 = y0.min(r.y);
        x1 = x1.max(r.x + r.w);
        y1 = y1.max(r.y + r.h);
    }
    Rect::new(x0 - pad, y0 - pad, (x1 - x0 + 2.0 * pad).max(0.0), (y1 - y0 + 2.0 * pad).max(0.0))
}

/// Computes the read-only [`ReroutePlan`] for connectors affected by `touched`.
///
/// - When `doc` has no connectors (`idx.is_empty()`), returns an empty plan immediately without reading any nodes.
/// - Skips connectors whose parent and both attached ends sit strictly inside the same moved ancestor subtree.
/// - When `viewport` is `Some(vp)`, defers connectors whose old and new world extents both lie outside `vp`.
/// - Skips byte-equal updates so unchanged connectors are never written.
pub fn plan_reroute(doc: &Document, touched: &[NodeId], viewport: Option<Rect>) -> ReroutePlan {
    if touched.is_empty() {
        return ReroutePlan::default();
    }
    let candidates = with_index(doc, |idx| if idx.is_empty() { Vec::new() } else { idx.reroute_candidates(doc, touched) });
    if candidates.is_empty() {
        return ReroutePlan::default();
    }

    let mut plan = ReroutePlan::default();

    for cid in candidates {
        record_candidate();
        let Some(n) = read_node(doc, cid) else { continue };
        let NodeKind::Connector(c) = &n.kind else { continue };
        let mut has_cross_page = false;
        if viewport.is_none() {
            let conn_page = doc.page_of(cid);
            let start_cross_page = c.start.node.is_some_and(|s| s != cid && doc.get(s).is_some() && conn_page.is_some() && doc.page_of(s) != conn_page);
            let end_cross_page = c.end.node.is_some_and(|e| e != cid && doc.get(e).is_some() && conn_page.is_some() && doc.page_of(e) != conn_page);
            if start_cross_page {
                plan.detached.push((cid, End::Start));
            }
            if end_cross_page {
                plan.detached.push((cid, End::End));
            }
            has_cross_page = start_cross_page || end_cross_page;
            let start_deleted = c.start.node.is_some_and(|s| s != cid && doc.get(s).is_none());
            let end_deleted = c.end.node.is_some_and(|e| e != cid && doc.get(e).is_none());
            if !start_deleted && !end_deleted {
                let live_start = (!start_cross_page).then_some(c.start.node).flatten().filter(|&s| s != cid);
                let live_end = (!end_cross_page).then_some(c.end.node).flatten().filter(|&e| e != cid);
                if live_start.is_some() || live_end.is_some() {
                    if let Some(page) = conn_page {
                        let home = crate::connector_home(doc, live_start, live_end, page);
                        if n.parent != Some(home) {
                            plan.rehomes.push((cid, home));
                        }
                    }
                }
            }
        }
        if has_cross_page {
            continue;
        }
        let parent_world = n.parent.map(|p| doc.world_transform(p)).unwrap_or(Affine::IDENTITY);
        let old_world_rect = transform_rect(parent_world * n.transform, Rect::new(0.0, 0.0, n.size.x, n.size.y));

        if let Some(vp) = viewport {
            if !old_world_rect.intersects(&vp) {
                let Some((a, b)) = connector_anchors(doc, cid) else { continue };
                let label_pad = c
                    .label
                    .as_ref()
                    .filter(|l| !l.content.is_empty())
                    .map(|l| {
                        let s = label_size_estimate(l);
                        s.x.max(s.y)
                    })
                    .unwrap_or(0.0);
                let pad = 32.0 + cap_size(n.stroke.weight) * 1.3 + n.stroke.weight + label_pad;
                let approx_new_world = transform_rect(parent_world, anchor_envelope(&a, &b, pad));
                if !approx_new_world.intersects(&vp) {
                    plan.deferred += 1;
                    continue;
                }
            }
        }

        let Some(g) = connector_geometry(doc, cid) else { continue };
        if let Some(vp) = viewport {
            let new_world_rect = transform_rect(parent_world * g.transform, Rect::new(0.0, 0.0, g.size.x, g.size.y));
            if !old_world_rect.intersects(&vp) && !new_world_rect.intersects(&vp) {
                plan.deferred += 1;
                continue;
            }
        }

        let changed = n.transform != g.transform || n.size != g.size || c.start.point != g.start_point || c.end.point != g.end_point;
        if changed {
            plan.updates.push((cid, g));
        }
    }

    plan
}

#[cfg(test)]
mod tests {
    use super::*;

    fn add_rect(doc: &mut Document, parent: NodeId, x: f32, y: f32) -> NodeId {
        let id = doc.alloc_id();
        doc.insert(parent, Node::rectangle(id, Rect::new(x, y, 80.0, 60.0))).unwrap()
    }

    fn add_conn(doc: &mut Document, parent: NodeId, a: NodeId, b: NodeId) -> NodeId {
        let id = doc.alloc_id();
        let props = ConnectorProps { start: ConnectorEnd::attached(a, Magnet::Auto), end: ConnectorEnd::attached(b, Magnet::Auto), ..Default::default() };
        doc.insert(parent, Node::connector(id, props)).unwrap();
        sync_connector(doc, id);
        id
    }

    #[test]
    fn connector_index_finds_connectors_without_scanning_nodes() {
        clear_connector_index();
        let mut doc = Document::new("synthetic_112k");
        let page = doc.add_page("Page 1");
        let frame_id = doc.alloc_id();
        doc.insert(page, Node::frame(frame_id, Rect::new(0.0, 0.0, 400.0, 400.0))).unwrap();
        let a = add_rect(&mut doc, frame_id, 20.0, 20.0);
        let b = add_rect(&mut doc, frame_id, 220.0, 200.0);
        let c = add_conn(&mut doc, page, a, b);

        for i in 0..112_000usize {
            let rid = doc.alloc_id();
            let mut n = Node::rectangle(rid, Rect::new((i % 200) as f32 * 20.0, (i / 200) as f32 * 20.0, 10.0, 10.0));
            n.parent = Some(page);
            doc.nodes.insert(rid, n);
        }
        doc.mark_tree_full_dirty();

        with_index(&doc, |idx| assert!(!idx.is_empty()));
        reset_connector_index_stats();

        let found = connectors_touching(&doc, &[frame_id]);
        assert_eq!(found, vec![c]);
        let st = connector_index_stats();
        assert_eq!(st.rebuilds, 0);
        assert!(st.node_reads <= 16, "expected O(subtree) node reads, got {}", st.node_reads);
    }

    #[test]
    fn connector_index_refreshes_from_dirty_log_not_rebuild() {
        clear_connector_index();
        let mut doc = Document::new("refresh_test");
        let page = doc.add_page("Page 1");
        let a = add_rect(&mut doc, page, 0.0, 0.0);
        let b = add_rect(&mut doc, page, 200.0, 0.0);
        let d = add_rect(&mut doc, page, 0.0, 200.0);
        let c1 = add_conn(&mut doc, page, a, b);

        with_index(&doc, |idx| {
            assert_eq!(idx.connectors_for_end(a), &[c1]);
        });
        assert_eq!(connector_index_stats().rebuilds, 1);

        doc.get_mut(a).unwrap().transform.tx += 10.0;
        let c2 = add_conn(&mut doc, page, a, d);

        with_index(&doc, |idx| {
            assert_eq!(idx.connectors_for_end(a), &[c1, c2]);
            assert_eq!(idx.connectors_for_end(d), &[c2]);
        });
        let st = connector_index_stats();
        assert_eq!(st.rebuilds, 1);
        assert!(st.refreshes >= 1);

        doc.remove(c2).unwrap();
        with_index(&doc, |idx| {
            assert_eq!(idx.connectors_for_end(a), &[c1]);
            assert!(idx.connectors_for_end(d).is_empty());
        });
        assert_eq!(connector_index_stats().rebuilds, 1);
    }

    #[test]
    fn connector_index_rebuilds_on_ring_eviction_and_len_change() {
        clear_connector_index();
        let mut doc = Document::new("rebuild_test");
        let page = doc.add_page("Page 1");
        let mut rects = Vec::with_capacity(600);
        for i in 0..600usize {
            rects.push(add_rect(&mut doc, page, (i % 30) as f32 * 100.0, (i / 30) as f32 * 100.0));
        }
        let c1 = add_conn(&mut doc, page, rects[0], rects[1]);

        with_index(&doc, |idx| assert_eq!(idx.connectors_for_end(rects[0]), &[c1]));
        reset_connector_index_stats();

        for &r in &rects {
            doc.get_mut(r).unwrap().transform.tx += 1.0;
        }
        with_index(&doc, |idx| assert_eq!(idx.connectors_for_end(rects[0]), &[c1]));
        assert_eq!(connector_index_stats().rebuilds, 1);

        reset_connector_index_stats();
        let c2 = doc.alloc_id();
        let mut conn2 = Node::connector(
            c2,
            ConnectorProps { start: ConnectorEnd::attached(rects[2], Magnet::Auto), end: ConnectorEnd::attached(rects[3], Magnet::Auto), ..Default::default() },
        );
        conn2.parent = Some(page);
        doc.nodes.insert(c2, conn2);

        let direct_found = connectors_touching(&doc, &[rects[2]]);
        assert_eq!(direct_found, vec![c2]);
        assert_eq!(connector_index_stats().rebuilds, 1);
    }

    #[test]
    fn index_is_not_shared_between_a_document_and_its_clone() {
        clear_connector_index();
        let mut doc = Document::new("orig");
        let page = doc.add_page("Page 1");
        let a = add_rect(&mut doc, page, 0.0, 0.0);
        let b = add_rect(&mut doc, page, 200.0, 0.0);
        let d = add_rect(&mut doc, page, 0.0, 200.0);
        let c1 = add_conn(&mut doc, page, a, b);

        with_index(&doc, |idx| assert_eq!(idx.connectors_for_end(b), &[c1]));

        let mut cloned = doc.clone();
        assert_ne!(doc.derived_id(), cloned.derived_id());

        if let NodeKind::Connector(cp) = &mut doc.get_mut(c1).unwrap().kind {
            cp.end = ConnectorEnd::attached(d, Magnet::Auto);
        }
        cloned.get_mut(a).unwrap().transform.tx += 5.0;
        assert_eq!(doc.mutation_epoch(), cloned.mutation_epoch());

        assert_eq!(connectors_touching(&doc, &[d]), vec![c1]);
        assert!(connectors_touching(&doc, &[b]).is_empty());
        assert_eq!(connectors_touching(&cloned, &[b]), vec![c1]);
        assert!(connectors_touching(&cloned, &[d]).is_empty());
    }

    #[test]
    fn document_without_connectors_skips_all_reroute_work() {
        clear_connector_index();
        let mut doc = Document::new("no_connectors");
        let page = doc.add_page("Page 1");
        let mut rects = Vec::new();
        for i in 0..200usize {
            rects.push(add_rect(&mut doc, page, i as f32 * 20.0, 0.0));
        }
        with_index(&doc, |idx| assert!(idx.is_empty()));

        doc.get_mut(rects[0]).unwrap().transform.tx += 25.0;
        reset_connector_index_stats();

        let plan = plan_reroute(&doc, &[rects[0]], None);
        assert!(plan.updates.is_empty());
        assert_eq!(connector_index_stats().node_reads, 0);
    }

    #[test]
    fn moving_a_node_reroutes_only_its_connectors() {
        clear_connector_index();
        let mut doc = Document::new("selective_reroute");
        let page = doc.add_page("Page 1");
        let hub = add_rect(&mut doc, page, 0.0, 0.0);
        let mut targets = Vec::with_capacity(200);
        for i in 0..200usize {
            targets.push(add_rect(&mut doc, page, (i % 20) as f32 * 160.0 + 200.0, (i / 20) as f32 * 140.0));
        }
        let mut hub_conns = Vec::with_capacity(50);
        for i in 0..50usize {
            hub_conns.push(add_conn(&mut doc, page, hub, targets[i]));
        }
        for i in 0..1_950usize {
            let a = targets[50 + (i % 150)];
            let b = targets[50 + ((i + 1) % 150)];
            add_conn(&mut doc, page, a, b);
        }

        with_index(&doc, |_| {});
        doc.get_mut(hub).unwrap().transform.tx += 40.0;
        reset_connector_index_stats();

        let plan = plan_reroute(&doc, &[hub], None);
        assert_eq!(plan.updates.len(), 50);
        let updated_ids: Vec<NodeId> = plan.updates.iter().map(|(id, _)| *id).collect();
        assert_eq!(updated_ids, hub_conns);
        assert!(connector_index_stats().node_reads <= 64);
    }

    #[test]
    fn connector_inside_moved_frame_is_not_rerouted() {
        clear_connector_index();
        let mut doc = Document::new("internal_connectors");
        let page = doc.add_page("Page 1");
        let frame = doc.alloc_id();
        doc.insert(page, Node::frame(frame, Rect::new(0.0, 0.0, 2000.0, 2000.0))).unwrap();

        let a = add_rect(&mut doc, frame, 20.0, 20.0);
        let b = add_rect(&mut doc, frame, 300.0, 200.0);
        for _ in 0..1_000usize {
            add_conn(&mut doc, frame, a, b);
        }

        with_index(&doc, |_| {});
        doc.get_mut(frame).unwrap().transform.tx += 120.0;
        doc.get_mut(frame).unwrap().transform.ty += 80.0;

        let plan = plan_reroute(&doc, &[frame], None);
        assert!(plan.updates.is_empty(), "expected 0 updates for internal connectors, got {}", plan.updates.len());
    }

    #[test]
    fn reroute_defers_offscreen_connectors_until_commit() {
        clear_connector_index();
        let mut doc = Document::new("viewport_deferral");
        let page = doc.add_page("Page 1");
        let hub = add_rect(&mut doc, page, 240.0, 11_000.0);

        let mut visible_conns = Vec::new();
        let mut offscreen_conns = Vec::new();
        for i in 0..20usize {
            let t = add_rect(&mut doc, page, 9_000.0 + i as f32 * 120.0, 11_200.0);
            visible_conns.push(add_conn(&mut doc, page, hub, t));
        }
        for i in 0..30usize {
            let t = add_rect(&mut doc, page, 600.0 + i as f32 * 120.0, 11_200.0);
            offscreen_conns.push(add_conn(&mut doc, page, hub, t));
        }

        with_index(&doc, |_| {});
        doc.get_mut(hub).unwrap().transform.ty += 40.0;

        let vp = Rect::new(8_800.0, 10_900.0, 3_000.0, 800.0);
        let drag_plan = plan_reroute(&doc, &[hub], Some(vp));
        assert_eq!(drag_plan.updates.len(), 20);
        assert_eq!(drag_plan.deferred, 30);

        for (cid, g) in &drag_plan.updates {
            g.apply(doc.get_mut(*cid).unwrap());
        }

        let commit_plan = plan_reroute(&doc, &[hub], None);
        assert_eq!(commit_plan.updates.len(), 30);
        assert_eq!(commit_plan.deferred, 0);
        let commit_ids: Vec<NodeId> = commit_plan.updates.iter().map(|(id, _)| *id).collect();
        assert_eq!(commit_ids, offscreen_conns);
    }
}
