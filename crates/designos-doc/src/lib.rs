//! designos document model.
//!
//! This crate is the contract shared by every other crate: importers produce a
//! [`Document`], the renderer draws it, the app edits it through [`Command`]s.
//! Coordinates are in Figma units (1 unit = 1 CSS px at 100% zoom), y down.

#![allow(
    clippy::field_reassign_with_default,
    clippy::needless_range_loop,
    clippy::question_mark,
    clippy::manual_is_multiple_of,
    clippy::collapsible_if,
    clippy::match_like_matches_macro,
    clippy::needless_borrow,
    clippy::redundant_guards
)]

mod board;
pub mod changelog_features;
mod comments;
mod components;
mod dev;
pub mod find;
pub mod finite_json;
#[cfg(test)]
mod finite_json_tests;
mod geom;
mod history;
mod layout_apply;
pub mod layout_solver;
mod layout_solver_grid;
mod layout_solver_grid_placement;
mod layout_solver_types;
mod library;
mod library_ops;
pub mod live_patch;
mod load_repair;
#[cfg(test)]
mod load_repair_tests;
mod node;
pub mod openpencil;
mod paint;
mod prototype;
mod renumber_ids;
#[cfg(test)]
mod renumber_ids_tests;
mod settings;
#[cfg(test)]
mod slot_model_tests;
pub mod spec;
mod styles_ops;
mod textmodel;
mod variables;
mod variables_apply;
mod variables_edit;
pub mod vector;
mod vector_fit;

pub use board::*;
pub use changelog_features::*;
pub use comments::*;
pub use components::*;
pub use dev::*;
pub use geom::*;
pub use history::*;
pub use library::*;
pub use library_ops::*;
pub use load_repair::*;
pub use node::*;
pub use openpencil::*;
pub use paint::*;
pub use prototype::*;
pub use settings::*;
pub use spec::*;
pub use styles_ops::*;
pub use textmodel::*;
pub use variables::*;
pub use variables_apply::*;
pub use variables_edit::*;
pub use vector::{HandleMirror, VectorNetwork};
pub use vector_fit::{fit_polyline, Anchor};

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Stable node identifier. Imported Figma ids ("12:34") are kept in
/// [`Node::source_id`]; `NodeId` is our own dense key.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct NodeId(pub u64);

#[derive(Debug, thiserror::Error)]
pub enum DocError {
    #[error("node {0:?} not found")]
    NotFound(NodeId),
    #[error("cannot move node {0:?} into its own subtree")]
    Cycle(NodeId),
    #[error("node {0:?} cannot have children")]
    NotAContainer(NodeId),
}

const DIRTY_RING_CAP: usize = 512;
const REMOVED_RING_CAP: usize = 64;

static NEXT_DOC_RUNTIME_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

#[derive(Clone, Debug)]
struct DirtyLog {
    runtime_id: u64,
    epoch: u64,
    geom_full_epoch: u64,
    tree_full_epoch: u64,
    evicted_epoch: u64,
    write_pos: u64,
    ring: [(u64, NodeId); DIRTY_RING_CAP],
    removed_evicted_epoch: u64,
    removed_write_pos: u64,
    removed_ring: [(u64, NodeId); REMOVED_RING_CAP],
}

impl Default for DirtyLog {
    fn default() -> Self {
        Self {
            runtime_id: NEXT_DOC_RUNTIME_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            epoch: 1,
            geom_full_epoch: 1,
            tree_full_epoch: 1,
            evicted_epoch: 0,
            write_pos: 0,
            ring: [(0, NodeId(0)); DIRTY_RING_CAP],
            removed_evicted_epoch: 0,
            removed_write_pos: 0,
            removed_ring: [(0, NodeId(0)); REMOVED_RING_CAP],
        }
    }
}

/// A whole file: pages at the root, each page a tree of nodes.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Document {
    pub name: String,
    /// Design file or board (whiteboard) file; decides the toolbar and panels.
    #[serde(default)]
    pub kind: DocumentKind,
    pub nodes: BTreeMap<NodeId, Node>,
    /// Page node ids in display order. Each page is a node with [`NodeKind::Page`].
    pub pages: Vec<NodeId>,
    pub styles: BTreeMap<String, Style>,
    /// Display metadata for entries of `styles`, same keys.
    #[serde(default)]
    pub style_meta: BTreeMap<String, StyleMeta>,
    /// Prototype flows (starting points), in display order.
    #[serde(default)]
    pub flows: Vec<Flow>,
    /// Variables (design tokens) by id.
    #[serde(default)]
    pub variables: BTreeMap<String, Variable>,
    /// Variable collections (with their modes) by id.
    #[serde(default)]
    pub variable_collections: BTreeMap<String, VariableCollection>,
    /// Ruler guides per page.
    #[serde(default)]
    pub guides: BTreeMap<NodeId, Vec<Guide>>,
    /// Device used when presenting the prototype.
    #[serde(default)]
    pub prototype_device: PrototypeDevice,
    /// Comment threads on the canvas.
    #[serde(default)]
    pub comments: Vec<CommentThread>,
    /// Published / enabled libraries and local copies of library assets.
    #[serde(default)]
    pub library: LibraryState,
    /// Hidden page holding copies of library components; not in `pages`.
    #[serde(default)]
    pub library_page: Option<NodeId>,
    /// Embedded images keyed by content hash (Figma `imageRef`).
    #[serde(default, with = "serde_images", skip_serializing_if = "BTreeMap::is_empty")]
    pub images: BTreeMap<String, Vec<u8>>,
    next_id: u64,
    /// Where the exhausted-counter fallback resumes scanning, so one id is never handed out twice
    /// before its node is inserted. Runtime only.
    #[serde(skip)]
    gap_cursor: u64,
    #[serde(skip)]
    dirty: DirtyLog,
    /// Optional on-demand hydrator that expands deferred component instances on inactive pages when switched to or exported.
    #[serde(skip)]
    pub page_hydrator: Option<std::sync::Arc<std::sync::Mutex<dyn PageHydrator>>>,
}

/// On-demand hydrator for deferred component instances on inactive pages of large multi-page `.fig` documents.
pub trait PageHydrator: Send + Sync + std::fmt::Debug {
    fn hydrate_page(&mut self, doc: &mut Document, page: NodeId) -> bool;
    fn hydrate_all(&mut self, doc: &mut Document) -> bool;
    fn is_hydrated(&self, page: NodeId) -> bool;
}

mod serde_images {
    use serde::de::{self, MapAccess, SeqAccess, Visitor};
    use serde::ser::SerializeMap;
    use serde::{Deserializer, Serializer};
    use std::collections::BTreeMap;
    use std::fmt;

    const HEX: &[u8; 16] = b"0123456789abcdef";

    fn to_hex_prefixed(bytes: &[u8]) -> String {
        let mut out = Vec::with_capacity(4 + bytes.len() * 2);
        out.extend_from_slice(b"hex:");
        for &b in bytes {
            out.push(HEX[(b >> 4) as usize]);
            out.push(HEX[(b & 0x0f) as usize]);
        }
        // SAFETY: `out` only contains ASCII hex characters.
        unsafe { String::from_utf8_unchecked(out) }
    }

    fn from_hex_str(s: &str) -> Result<Vec<u8>, String> {
        let hex = s.strip_prefix("hex:").unwrap_or(s);
        if hex.len() % 2 != 0 {
            return Err("odd-length hex image payload".into());
        }
        let mut out = Vec::with_capacity(hex.len() / 2);
        let bytes = hex.as_bytes();
        let val = |c: u8| -> Option<u8> {
            match c {
                b'0'..=b'9' => Some(c - b'0'),
                b'a'..=b'f' => Some(c - b'a' + 10),
                b'A'..=b'F' => Some(c - b'A' + 10),
                _ => None,
            }
        };
        let mut i = 0;
        while i < bytes.len() {
            let hi = val(bytes[i]).ok_or_else(|| "invalid hex digit".to_string())?;
            let lo = val(bytes[i + 1]).ok_or_else(|| "invalid hex digit".to_string())?;
            out.push((hi << 4) | lo);
            i += 2;
        }
        Ok(out)
    }

    struct ImageBytes(Vec<u8>);

    impl<'de> serde::Deserialize<'de> for ImageBytes {
        fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
            struct BytesVisitor;
            impl<'de> Visitor<'de> for BytesVisitor {
                type Value = ImageBytes;
                fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                    f.write_str("a hex string or byte array")
                }
                fn visit_str<E: de::Error>(self, v: &str) -> Result<Self::Value, E> {
                    from_hex_str(v).map(ImageBytes).map_err(E::custom)
                }
                fn visit_bytes<E: de::Error>(self, v: &[u8]) -> Result<Self::Value, E> {
                    Ok(ImageBytes(v.to_vec()))
                }
                fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
                    let mut out = Vec::with_capacity(seq.size_hint().unwrap_or(0));
                    while let Some(b) = seq.next_element::<u8>()? {
                        out.push(b);
                    }
                    Ok(ImageBytes(out))
                }
            }
            deserializer.deserialize_any(BytesVisitor)
        }
    }

    pub fn serialize<S: Serializer>(map: &BTreeMap<String, Vec<u8>>, serializer: S) -> Result<S::Ok, S::Error> {
        let mut m = serializer.serialize_map(Some(map.len()))?;
        for (k, v) in map {
            let hex = to_hex_prefixed(v);
            m.serialize_entry(k, &hex)?;
        }
        m.end()
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<BTreeMap<String, Vec<u8>>, D::Error> {
        struct MapVisitor;
        impl<'de> Visitor<'de> for MapVisitor {
            type Value = BTreeMap<String, Vec<u8>>;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a map of image hash to hex string or byte array")
            }
            fn visit_map<M: MapAccess<'de>>(self, mut access: M) -> Result<Self::Value, M::Error> {
                let mut map = BTreeMap::new();
                while let Some((k, ImageBytes(v))) = access.next_entry::<String, ImageBytes>()? {
                    map.insert(k, v);
                }
                Ok(map)
            }
        }
        deserializer.deserialize_map(MapVisitor)
    }
}

impl Document {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into(), ..Default::default() }
    }

    /// Ensures deferred component instances on `page` are expanded on demand.
    /// Returns `true` if any deferred instances were expanded.
    pub fn ensure_page_hydrated(&mut self, page: NodeId) -> bool {
        let Some(h) = self.page_hydrator.clone() else { return false };
        let Ok(mut guard) = h.lock() else { return false };
        if guard.is_hydrated(page) {
            return false;
        }
        let changed = guard.hydrate_page(self, page);
        if changed {
            self.mark_tree_full_dirty();
        }
        changed
    }

    /// Expands all remaining deferred pages (e.g., before full-document export).
    pub fn hydrate_all_pages(&mut self) -> bool {
        let Some(h) = self.page_hydrator.take() else { return false };
        let Ok(mut guard) = h.lock() else { return false };
        let changed = guard.hydrate_all(self);
        if changed {
            self.mark_tree_full_dirty();
        }
        changed
    }

    /// Hands out a fresh id. The counter never wraps: once it is exhausted (a hostile or
    /// corrupted file can carry ids near `u64::MAX`) ids come from the gaps below the largest
    /// node id instead. Loaders that can renumber (`renumber_ids_for_headroom`) make this a last
    /// resort; the fallback still never returns an id it already returned, even when several ids
    /// are allocated before any of them is inserted.
    pub fn alloc_id(&mut self) -> NodeId {
        match self.next_id.checked_add(1) {
            Some(next) => {
                self.next_id = next;
                NodeId(next)
            }
            None => self.next_gap_id(),
        }
    }

    /// Smallest id (never 0) at or above the cursor that no node uses; advances the cursor past it.
    fn next_gap_id(&mut self) -> NodeId {
        let mut candidate = self.gap_cursor.max(1);
        for id in self.nodes.range(NodeId(candidate)..).map(|(id, _)| id) {
            if id.0 != candidate {
                break;
            }
            // saturating: all of 1..=u64::MAX in use would need more memory than exists.
            candidate = candidate.saturating_add(1);
        }
        self.gap_cursor = candidate.saturating_add(1);
        NodeId(candidate)
    }

    pub fn peek_next_id(&self) -> NodeId {
        NodeId(self.next_id + 1)
    }

    /// Allocate safely after importing a document whose stored counter may be
    /// stale, preserving previously allocated IDs and reporting exhaustion.
    pub fn try_alloc_id(&mut self) -> Option<NodeId> {
        let largest = self.nodes.keys().next_back().map_or(0, |id| id.0);
        self.next_id = self.next_id.max(largest).checked_add(1)?;
        Some(NodeId(self.next_id))
    }

    /// Ensures future [`Self::alloc_id`] calls return ids above `id`, so
    /// callers can assign deterministic ids themselves.
    pub fn reserve_ids(&mut self, id: NodeId) {
        self.next_id = self.next_id.max(id.0);
    }

    /// Display name of a shared style: its metadata name, else the key.
    pub fn style_name<'a>(&'a self, key: &'a str) -> &'a str {
        self.style_meta.get(key).map(|m| m.name.as_str()).filter(|n| !n.is_empty()).unwrap_or(key)
    }

    #[inline]
    pub fn runtime_id(&self) -> u64 {
        self.dirty.runtime_id
    }

    #[inline]
    pub fn mutation_epoch(&self) -> u64 {
        self.dirty.epoch
    }

    #[inline]
    pub fn bump_mutation_epoch(&mut self) {
        self.dirty.epoch += 1;
    }

    #[inline]
    pub fn mark_full_dirty(&mut self) {
        self.dirty.epoch += 1;
        self.dirty.geom_full_epoch = self.dirty.epoch;
        self.dirty.tree_full_epoch = self.dirty.epoch;
    }

    #[inline]
    pub fn mark_tree_full_dirty(&mut self) {
        self.dirty.epoch += 1;
        self.dirty.tree_full_epoch = self.dirty.epoch;
    }

    #[inline]
    pub fn mark_node_dirty(&mut self, id: NodeId) {
        self.dirty.epoch += 1;
        let ep = self.dirty.epoch;
        let full = self.dirty.geom_full_epoch.max(self.dirty.tree_full_epoch);
        if self.dirty.write_pos > 0 {
            let last = ((self.dirty.write_pos - 1) as usize) % DIRTY_RING_CAP;
            if self.dirty.ring[last].1 == id && self.dirty.ring[last].0 > full {
                self.dirty.ring[last].0 = ep;
                return;
            }
        }
        let idx = (self.dirty.write_pos as usize) % DIRTY_RING_CAP;
        self.dirty.evicted_epoch = self.dirty.evicted_epoch.max(self.dirty.ring[idx].0);
        self.dirty.ring[idx] = (ep, id);
        self.dirty.write_pos += 1;
    }

    #[inline]
    pub fn mark_node_removed(&mut self, id: NodeId) {
        self.dirty.epoch += 1;
        let ep = self.dirty.epoch;
        let idx = (self.dirty.removed_write_pos as usize) % REMOVED_RING_CAP;
        self.dirty.removed_evicted_epoch = self.dirty.removed_evicted_epoch.max(self.dirty.removed_ring[idx].0);
        self.dirty.removed_ring[idx] = (ep, id);
        self.dirty.removed_write_pos += 1;
    }

    /// Appends the unique [`NodeId`]s modified or inserted into `dirty_out` and
    /// removed [`NodeId`]s into `removed_out` since `since_epoch`, returning `true`
    /// if no full-tree/full-geom invalidation or ring eviction occurred since `since_epoch`.
    pub fn dirty_changes_since(&self, since_epoch: u64, dirty_out: &mut Vec<NodeId>, removed_out: &mut Vec<NodeId>) -> bool {
        dirty_out.clear();
        removed_out.clear();
        if since_epoch > self.dirty.epoch
            || since_epoch < self.dirty.geom_full_epoch
            || since_epoch < self.dirty.tree_full_epoch
            || since_epoch < self.dirty.evicted_epoch
            || since_epoch < self.dirty.removed_evicted_epoch
        {
            return false;
        }
        if since_epoch == self.dirty.epoch {
            return true;
        }
        let r_count = (self.dirty.removed_write_pos as usize).min(REMOVED_RING_CAP);
        for i in 0..r_count {
            let (ep, id) = self.dirty.removed_ring[i];
            if ep > since_epoch && !removed_out.contains(&id) {
                removed_out.push(id);
            }
        }
        let count = (self.dirty.write_pos as usize).min(DIRTY_RING_CAP);
        for i in 0..count {
            let (ep, id) = self.dirty.ring[i];
            if ep > since_epoch && !dirty_out.contains(&id) {
                dirty_out.push(id);
            }
        }
        true
    }

    /// Appends the unique [`NodeId`]s modified via [`Self::get_mut`] since
    /// `since_epoch` into `out` and returns `true` if no removals or
    /// ring-evicted mutations occurred since `since_epoch`.
    pub fn dirty_nodes_since(&self, since_epoch: u64, out: &mut Vec<NodeId>) -> bool {
        let mut removed = Vec::new();
        self.dirty_changes_since(since_epoch, out, &mut removed) && removed.is_empty()
    }

    /// Returns `true` if `id` is guaranteed not to have had its geometry or
    /// global styles/variables mutated since `since_epoch`.
    #[inline]
    pub fn is_node_clean_since(&self, id: NodeId, since_epoch: u64) -> bool {
        if since_epoch == self.dirty.epoch {
            return true;
        }
        if since_epoch > self.dirty.epoch || since_epoch < self.dirty.geom_full_epoch || since_epoch < self.dirty.evicted_epoch {
            return false;
        }
        let count = (self.dirty.write_pos as usize).min(DIRTY_RING_CAP);
        for i in 0..count {
            let (ep, dirty_id) = self.dirty.ring[i];
            if ep > since_epoch && dirty_id == id {
                return false;
            }
        }
        true
    }

    pub fn add_page(&mut self, name: impl Into<String>) -> NodeId {
        let id = self.alloc_id();
        let mut page = Node::new(id, NodeKind::Page { background: Color::rgb(0x5A, 0x5A, 0x5A) });
        page.name = name.into();
        self.nodes.insert(id, page);
        self.pages.push(id);
        self.mark_tree_full_dirty();
        id
    }

    /// Inserts `node` as the last (topmost) child of `parent`.
    pub fn insert(&mut self, parent: NodeId, mut node: Node) -> Result<NodeId, DocError> {
        let p = self.nodes.get_mut(&parent).ok_or(DocError::NotFound(parent))?;
        if !p.kind.can_hold_children() {
            return Err(DocError::NotAContainer(parent));
        }
        let id = node.id;
        p.children.push(id);
        node.parent = Some(parent);
        self.next_id = self.next_id.max(id.0);
        self.nodes.insert(id, node);
        self.mark_node_dirty(parent);
        self.mark_node_dirty(id);
        Ok(id)
    }

    pub fn get(&self, id: NodeId) -> Option<&Node> {
        self.nodes.get(&id)
    }

    pub fn get_mut(&mut self, id: NodeId) -> Option<&mut Node> {
        if self.nodes.contains_key(&id) {
            self.mark_node_dirty(id);
        }
        self.nodes.get_mut(&id)
    }

    /// Removes `id` and its subtree, returning the removed nodes (root first).
    pub fn remove(&mut self, id: NodeId) -> Result<Vec<Node>, DocError> {
        let parent = self.nodes.get(&id).ok_or(DocError::NotFound(id))?.parent;
        if let Some(p) = parent.and_then(|p| self.nodes.get_mut(&p)) {
            p.children.retain(|c| *c != id);
        }
        self.pages.retain(|p| *p != id);
        let mut out = Vec::new();
        let mut stack = vec![id];
        while let Some(n) = stack.pop() {
            if let Some(node) = self.nodes.remove(&n) {
                stack.extend(node.children.iter().rev().copied());
                out.push(node);
            }
        }
        if out.len() <= REMOVED_RING_CAP && parent.is_some() {
            if let Some(p) = parent {
                self.mark_node_dirty(p);
            }
            for r in &out {
                self.mark_node_removed(r.id);
            }
        } else {
            self.mark_tree_full_dirty();
        }
        Ok(out)
    }

    /// Moves `id` under `new_parent` at `index` (clamped).
    pub fn reparent(&mut self, id: NodeId, new_parent: NodeId, index: usize) -> Result<(), DocError> {
        if self.is_ancestor(id, new_parent) || id == new_parent {
            return Err(DocError::Cycle(id));
        }
        if !self.nodes.get(&new_parent).ok_or(DocError::NotFound(new_parent))?.kind.can_hold_children() {
            return Err(DocError::NotAContainer(new_parent));
        }
        let old = self.nodes.get(&id).ok_or(DocError::NotFound(id))?.parent;
        if let Some(p) = old.and_then(|p| self.nodes.get_mut(&p)) {
            p.children.retain(|c| *c != id);
        }
        let p = self.nodes.get_mut(&new_parent).unwrap();
        let index = index.min(p.children.len());
        p.children.insert(index, id);
        self.nodes.get_mut(&id).unwrap().parent = Some(new_parent);
        if let Some(old_p) = old {
            self.mark_node_dirty(old_p);
        }
        if old != Some(new_parent) {
            self.mark_node_dirty(new_parent);
        }
        self.mark_node_dirty(id);
        Ok(())
    }

    pub fn is_ancestor(&self, ancestor: NodeId, mut of: NodeId) -> bool {
        while let Some(p) = self.nodes.get(&of).and_then(|n| n.parent) {
            if p == ancestor {
                return true;
            }
            of = p;
        }
        false
    }

    /// Absolute transform (page space) of a node.
    pub fn world_transform(&self, id: NodeId) -> Affine {
        let mut chain = Vec::new();
        let mut cur = Some(id);
        while let Some(c) = cur {
            let Some(n) = self.nodes.get(&c) else { break };
            chain.push(n.transform);
            cur = n.parent;
        }
        chain.iter().rev().fold(Affine::IDENTITY, |acc, t| acc * *t)
    }

    /// Page that contains `id`.
    pub fn page_of(&self, mut id: NodeId) -> Option<NodeId> {
        loop {
            let n = self.nodes.get(&id)?;
            match n.parent {
                Some(p) => id = p,
                None => return matches!(n.kind, NodeKind::Page { .. }).then_some(id),
            }
        }
    }

    /// Pre-order traversal of a subtree, back-to-front paint order.
    pub fn walk(&self, root: NodeId, mut f: impl FnMut(&Node, usize)) {
        fn rec(doc: &Document, id: NodeId, depth: usize, f: &mut dyn FnMut(&Node, usize)) {
            if let Some(n) = doc.nodes.get(&id) {
                f(n, depth);
                for c in &n.children {
                    rec(doc, *c, depth + 1, f);
                }
            }
        }
        rec(self, root, 0, &mut f);
    }

    /// Counts the number of nodes in `page`'s subtree in $O(|T_{\text{page}}|)$ time.
    pub fn page_node_count(&self, page: NodeId) -> usize {
        let mut count = 0;
        self.walk(page, |_, _| count += 1);
        count
    }

    /// Collects all [`NodeId`]s reachable from `self.pages` (following
    /// `n.children`, `NodeKind::Instance { component: Some(cid), .. }`,
    /// [`ComponentLink::source`], and [`PropValue::Instance`] /
    /// [`PropKind::InstanceSwap`]) and removes any unreachable orphan nodes
    /// from `self.nodes`, returning the number of removed nodes.
    pub fn prune_unreachable(&mut self) -> usize {
        let mut reachable = std::collections::BTreeSet::new();
        let mut stack: Vec<NodeId> = self.pages.clone();
        if let Some(lp) = self.library_page {
            stack.push(lp);
        }
        while let Some(id) = stack.pop() {
            if !reachable.insert(id) {
                continue;
            }
            if let Some(n) = self.nodes.get(&id) {
                stack.extend(n.children.iter().copied());
                if let NodeKind::Instance { component: Some(cid), .. } = &n.kind {
                    stack.push(*cid);
                }
                if let Some(src) = n.component.source {
                    stack.push(src);
                }
                for prop in &n.component.props {
                    if let PropValue::Instance(cid) = prop.default {
                        stack.push(cid);
                    }
                    if let PropKind::InstanceSwap { preferred } = &prop.kind {
                        stack.extend(preferred.iter().copied());
                    }
                }
                for val in n.component.values.values() {
                    if let PropValue::Instance(cid) = val {
                        stack.push(*cid);
                    }
                }
                for cfg in n.component.slot_configs.values() {
                    stack.extend(cfg.preferred.iter().copied());
                }
            }
        }
        let before = self.nodes.len();
        self.nodes.retain(|id, _| reachable.contains(id));
        let removed = before - self.nodes.len();
        if removed > 0 {
            self.mark_tree_full_dirty();
        }
        removed
    }

    /// Compact JSON of the document. A number that is not finite (NaN, infinity) is written as its
    /// neutral value instead (see [`finite_json`]), so the text always parses back; a file used to
    /// fail to open with `invalid type: null, expected f32` after one such number was saved.
    pub fn to_json(&self) -> serde_json::Result<String> {
        let (json, replaced) = self.to_json_counted()?;
        if replaced > 0 {
            static WARNED: std::sync::Once = std::sync::Once::new();
            WARNED.call_once(|| eprintln!("designos-doc: wrote {replaced} non-finite numbers as their neutral value (reported once per run)"));
        }
        Ok(json)
    }

    /// Like [`Document::to_json`], also returning how many non-finite numbers were replaced.
    pub fn to_json_counted(&self) -> serde_json::Result<(String, usize)> {
        finite_json::to_string_finite(self)
    }

    /// How many non-finite numbers [`Document::to_json_counted`] would replace, counted without
    /// building the JSON text.
    pub fn count_non_finite(&self) -> serde_json::Result<usize> {
        finite_json::count_non_finite(self)
    }

    /// Parses a native document. Text written by older versions that emulated lists with
    /// literal bullet and "1. " prefixes is converted to real lists. A malformed node graph is
    /// repaired in place (see [`Document::validate_and_repair`]) and noted on stderr, and a `null`
    /// where a number belongs (a NaN saved by an older version) is read as the neutral value.
    pub fn from_json(s: &str) -> serde_json::Result<Self> {
        let (doc, repaired) = Self::from_json_with_report(s)?;
        if !repaired.is_clean() {
            eprintln!("designos-doc: repaired a damaged document on load: {repaired:?}");
        }
        Ok(doc)
    }

    /// Like [`Document::from_json`] but silent: returns what the load-time repair changed so a
    /// caller that must not accept a repaired document (for example the MCP server) can refuse it.
    pub fn from_json_with_report(s: &str) -> serde_json::Result<(Self, RepairReport)> {
        // A `null` in a number slot is how older versions saved a NaN or infinity; it is read as the
        // neutral value and counted (see [`finite_json`]).
        let (mut doc, non_finite_numbers): (Self, usize) = finite_json::from_str_repairing(s)?;
        let mut report = doc.validate_and_repair();
        report.non_finite_numbers = non_finite_numbers;
        doc.migrate_text();
        Ok((doc, report))
    }

    /// Converts literal list prefixes of every text node to real lists (see [`migrate_literal_lists`]).
    pub fn migrate_text(&mut self) {
        for node in self.nodes.values_mut() {
            if let NodeKind::Text(t) = &mut node.kind {
                migrate_literal_lists(t);
            }
        }
        self.mark_full_dirty();
    }
}

/// Which node property a shared style applies to.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum StyleProperty {
    #[default]
    Fill,
    Stroke,
    Text,
    Effect,
    Grid,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct StyleMeta {
    /// Name as shown in the UI, may contain "/" for grouping ("Brand/Primary").
    pub name: String,
    /// Library key (Figma style `key`), for linking to published libraries.
    pub key: Option<String>,
    pub description: String,
    /// Property the style is used for (a `Style::Fill` can be a stroke style).
    pub property: StyleProperty,
}

/// Shared (library) style referenced by nodes via `style_refs`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Style {
    Fill(Vec<Paint>),
    Text(TextStyle),
    Effect(Vec<Effect>),
    Grid(Vec<LayoutGrid>),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_reparent_remove() {
        let mut d = Document::new("t");
        let page = d.add_page("Page 1");
        let fid = d.alloc_id();
        let frame = d.insert(page, Node::frame(fid, Rect::new(0.0, 0.0, 100.0, 100.0))).unwrap();
        let rid = d.alloc_id();
        let rect = d.insert(page, Node::rectangle(rid, Rect::new(10.0, 10.0, 20.0, 20.0))).unwrap();
        d.reparent(rect, frame, 0).unwrap();
        assert_eq!(d.get(frame).unwrap().children, vec![rect]);
        assert!(d.reparent(frame, rect, 0).is_err());
        assert_eq!(d.page_of(rect), Some(page));
        let removed = d.remove(frame).unwrap();
        assert_eq!(removed.len(), 2);
        assert!(d.get(rect).is_none());
    }

    #[test]
    fn unknown_layer_holds_children_for_tree_edits_without_becoming_a_container() {
        let mut d = Document::new("t");
        let page = d.add_page("Page 1");
        let uid = d.alloc_id();
        let unknown = d.insert(page, Node::new(uid, NodeKind::Unknown { type_name: "SLIDE".into() })).unwrap();
        let kind = &d.get(unknown).unwrap().kind;
        assert!(kind.can_hold_children());
        assert!(!kind.is_container(), "an unmapped layer must not pick up clipping, layout or hit-testing as a container");
        assert!(kind.frame_props().is_none());

        let rid = d.alloc_id();
        let rect = d.insert(unknown, Node::rectangle(rid, Rect::new(0.0, 0.0, 10.0, 10.0))).unwrap();
        let fid = d.alloc_id();
        let frame = d.insert(page, Node::frame(fid, Rect::new(0.0, 0.0, 50.0, 50.0))).unwrap();
        d.reparent(rect, frame, 0).unwrap();
        d.reparent(rect, unknown, 0).unwrap();
        assert_eq!(d.get(unknown).unwrap().children, vec![rect]);

        let mut h = History::default();
        h.exec(&mut d, Command::Remove { id: rect }).unwrap();
        assert!(d.get(unknown).unwrap().children.is_empty());
        assert!(h.undo(&mut d).unwrap());
        assert_eq!(d.get(unknown).unwrap().children, vec![rect]);
        assert_eq!(d.get(rect).unwrap().parent, Some(unknown));

        // Leaves still refuse children.
        let lid = d.alloc_id();
        assert!(matches!(d.insert(rect, Node::rectangle(lid, Rect::new(0.0, 0.0, 1.0, 1.0))), Err(DocError::NotAContainer(p)) if p == rect));
        assert!(matches!(d.reparent(frame, rect, 0), Err(DocError::NotAContainer(p)) if p == rect));
    }

    #[test]
    fn json_roundtrip() {
        let mut d = Document::new("t");
        let page = d.add_page("P");
        let id = d.alloc_id();
        d.insert(page, Node::rectangle(id, Rect::new(0.0, 0.0, 5.0, 5.0))).unwrap();
        let back = Document::from_json(&d.to_json().unwrap()).unwrap();
        assert_eq!(back.nodes.len(), 2);
    }

    #[test]
    fn component_link_roundtrip_and_default() {
        let mut d = Document::new("t");
        let page = d.add_page("P");
        let id = d.alloc_id();
        let mut n = Node::rectangle(id, Rect::new(0.0, 0.0, 5.0, 5.0));
        n.component.source = Some(NodeId(99));
        n.component.overrides.insert(OverrideField::Fills);
        n.component.bindings.insert(OverrideField::Visible, "Show icon".into());
        d.insert(page, n).unwrap();
        let json = d.to_json().unwrap();
        let back = Document::from_json(&json).unwrap();
        assert_eq!(back.get(id).unwrap().component, d.get(id).unwrap().component);
        // Plain nodes omit the field, and files without it still load.
        assert!(!json.contains("\"component\": {}"));
        let page_json = serde_json::to_string(back.get(page).unwrap()).unwrap();
        assert!(!page_json.contains("\"component\""));
    }

    #[test]
    fn images_compact_and_legacy_roundtrip() {
        let mut d = Document::new("t");
        d.add_page("P");
        d.images.insert("hash1".into(), vec![0x89, 0x50, 0x4e, 0x47]);
        let json = d.to_json().unwrap();
        assert!(json.contains("\"hex:89504e47\""), "images serialize as compact hex: {json}");
        let back = Document::from_json(&json).unwrap();
        assert_eq!(back.images.get("hash1").unwrap(), &vec![0x89, 0x50, 0x4e, 0x47]);

        // Legacy array format still loads cleanly.
        let legacy = json.replace("\"hex:89504e47\"", "[137, 80, 78, 71]");
        let from_legacy = Document::from_json(&legacy).unwrap();
        assert_eq!(from_legacy.images.get("hash1").unwrap(), &vec![0x89, 0x50, 0x4e, 0x47]);
    }

    #[test]
    fn node_default_fields_omitted_and_roundtrip() {
        let mut d = Document::new("t");
        let page = d.add_page("P");
        let id = d.alloc_id();
        d.insert(page, Node::rectangle(id, Rect::new(1.0, 2.0, 30.0, 40.0))).unwrap();
        let rect_json = serde_json::to_string(d.get(id).unwrap()).unwrap();
        for omitted in
            ["\"source_id\"", "\"children\"", "\"effects\"", "\"style_refs\"", "\"fill_geometry\"", "\"stroke_geometry\"", "\"corner_radii\"", "\"stroke\""]
        {
            assert!(!rect_json.contains(omitted), "{omitted} should be omitted on default node: {rect_json}");
        }
        let back = Document::from_json(&d.to_json().unwrap()).unwrap();
        assert_eq!(back.get(id), d.get(id));
    }

    #[test]
    fn prune_unreachable_preserves_reachable_and_removes_orphans() {
        let mut d = Document::new("t");
        let page = d.add_page("Page 1");
        let fid = d.alloc_id();
        let frame = d.insert(page, Node::frame(fid, Rect::new(0.0, 0.0, 100.0, 100.0))).unwrap();
        let rid = d.alloc_id();
        let rect = d.insert(frame, Node::rectangle(rid, Rect::new(0.0, 0.0, 10.0, 10.0))).unwrap();

        // Detached component masters & referenced targets (e.g. from hidden/imported library nodes)
        let comp_id = d.alloc_id();
        let comp_child_id = d.alloc_id();
        let swap_default_id = d.alloc_id();
        let swap_pref_id = d.alloc_id();
        let swap_val_id = d.alloc_id();
        let source_target_id = d.alloc_id();

        let mut comp_node = Node::new(comp_id, NodeKind::Component(FrameProps::default()));
        comp_node.children.push(comp_child_id);
        comp_node.component.props.push(ComponentPropDef {
            name: "Icon".into(),
            kind: PropKind::InstanceSwap { preferred: vec![swap_pref_id] },
            default: PropValue::Instance(swap_default_id),
        });
        let mut comp_child = Node::rectangle(comp_child_id, Rect::new(0.0, 0.0, 8.0, 8.0));
        comp_child.parent = Some(comp_id);

        d.nodes.insert(comp_id, comp_node);
        d.nodes.insert(comp_child_id, comp_child);
        d.nodes.insert(swap_default_id, Node::new(swap_default_id, NodeKind::Component(FrameProps::default())));
        d.nodes.insert(swap_pref_id, Node::new(swap_pref_id, NodeKind::Component(FrameProps::default())));
        d.nodes.insert(swap_val_id, Node::new(swap_val_id, NodeKind::Component(FrameProps::default())));
        d.nodes.insert(source_target_id, Node::rectangle(source_target_id, Rect::new(0.0, 0.0, 4.0, 4.0)));

        let inst_id = d.alloc_id();
        let mut inst = Node::new(inst_id, NodeKind::Instance { frame: FrameProps::default(), component: Some(comp_id), component_key: None });
        inst.component.source = Some(source_target_id);
        inst.component.values.insert("Icon".into(), PropValue::Instance(swap_val_id));
        d.insert(page, inst).unwrap();

        // Unreachable orphan subtree
        let orphan_parent = d.alloc_id();
        let orphan_child = d.alloc_id();
        let mut op = Node::frame(orphan_parent, Rect::new(0.0, 0.0, 50.0, 50.0));
        op.children.push(orphan_child);
        let mut oc = Node::rectangle(orphan_child, Rect::new(0.0, 0.0, 10.0, 10.0));
        oc.parent = Some(orphan_parent);
        d.nodes.insert(orphan_parent, op);
        d.nodes.insert(orphan_child, oc);

        let removed = d.prune_unreachable();
        assert_eq!(removed, 2);

        for kept in [page, frame, rect, inst_id, comp_id, comp_child_id, swap_default_id, swap_pref_id, swap_val_id, source_target_id] {
            assert!(d.get(kept).is_some(), "expected node {kept:?} to be preserved");
        }
        assert!(d.get(orphan_parent).is_none());
        assert!(d.get(orphan_child).is_none());
    }

    #[test]
    fn page_node_count_measures_subtree() {
        let mut d = Document::new("t");
        let p1 = d.add_page("Page 1");
        let p2 = d.add_page("Page 2");
        assert_eq!(d.page_node_count(p1), 1);
        assert_eq!(d.page_node_count(p2), 1);
        assert_eq!(d.page_node_count(NodeId(9999)), 0);

        let fid = d.alloc_id();
        let frame = d.insert(p1, Node::frame(fid, Rect::new(0.0, 0.0, 100.0, 100.0))).unwrap();
        let r1 = d.alloc_id();
        d.insert(frame, Node::rectangle(r1, Rect::new(0.0, 0.0, 10.0, 10.0))).unwrap();
        let r2 = d.alloc_id();
        d.insert(frame, Node::rectangle(r2, Rect::new(20.0, 0.0, 10.0, 10.0))).unwrap();

        assert_eq!(d.page_node_count(p1), 4);
        assert_eq!(d.page_node_count(p2), 1);
    }
}
