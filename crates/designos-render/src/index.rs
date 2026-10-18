//! Spatial index of world-space bounds per page, plus a node snapshot used to
//! work out which page regions changed between document revisions.
//!
//! The index is hierarchical: every node stores the AABB of its whole drawn
//! subtree (including stroke / shadow / blur overflow, clipped by clipping
//! frames). The renderer walks the normal tree but skips any child whose
//! subtree box misses the window being painted.

use crate::geom::{to_sk_transform, transformed_bounds};
use designos_doc::{Document, Effect, Node, NodeId, NodeKind};
use std::collections::{HashMap, HashSet};
use std::hash::{BuildHasherDefault, Hasher};
use tiny_skia as sk;

/// Multiplicative hasher for dense integer ids (SipHash is needlessly slow
/// for the per-node maps rebuilt on every revision).
#[derive(Default, Clone, Copy)]
pub(crate) struct IdHasher(u64);

impl Hasher for IdHasher {
    fn write(&mut self, bytes: &[u8]) {
        for b in bytes {
            self.write_u64(*b as u64);
        }
    }
    fn write_u64(&mut self, n: u64) {
        self.0 = (self.0.rotate_left(5) ^ n).wrapping_mul(0x517c_c1b7_2722_0a95);
    }
    fn finish(&self) -> u64 {
        // fold the high bits down: hashbrown uses both ends
        self.0 ^ (self.0 >> 32)
    }
}

pub(crate) type IdMap<V> = HashMap<NodeId, V, BuildHasherDefault<IdHasher>>;

/// Axis-aligned box in page space: (x0, y0, x1, y1).
pub(crate) type B = (f32, f32, f32, f32);

pub(crate) const EMPTY: B = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);

const GRID_CELL: f32 = 2048.0;

fn grid_range(b: B) -> (i32, i32, i32, i32) {
    let cx0 = (b.0 / GRID_CELL).floor() as i32;
    let cy0 = (b.1 / GRID_CELL).floor() as i32;
    let cx1 = (b.2 / GRID_CELL).floor() as i32;
    let cy1 = (b.3 / GRID_CELL).floor() as i32;
    (cx0, cy0, cx1, cy1)
}

pub(crate) fn is_empty(b: B) -> bool {
    b.0 > b.2 || b.1 > b.3
}

pub(crate) fn union(a: B, b: B) -> B {
    (a.0.min(b.0), a.1.min(b.1), a.2.max(b.2), a.3.max(b.3))
}

pub(crate) fn intersect(a: B, b: B) -> B {
    (a.0.max(b.0), a.1.max(b.1), a.2.min(b.2), a.3.min(b.3))
}

pub(crate) fn overlaps(a: B, b: B) -> bool {
    a.0 <= b.2 && b.0 <= a.2 && a.1 <= b.3 && b.1 <= a.3 && !is_empty(a) && !is_empty(b)
}

/// Extra extent (local units) around the node bounds touched by strokes and
/// effects. Shared with the renderer's own culling so both agree exactly.
pub(crate) fn node_margin(n: &Node) -> f32 {
    let mut m = 0.0f32;
    let has_stroke_paint = n.stroke.paints.iter().any(|p| p.visible) && (n.stroke.weight > 0.0 || n.stroke.side_weights.is_some());
    if has_stroke_paint {
        let sw = n.stroke.side_weights.map(|s| s.iter().cloned().fold(0.0, f32::max)).unwrap_or(0.0);
        m = m.max(n.stroke.weight.max(sw) * 2.0);
    }
    for e in &n.effects {
        match e {
            Effect::DropShadow { offset, radius, spread, visible: true, .. } => {
                m = m.max(offset.x.abs().max(offset.y.abs()) + spread.max(0.0) + radius * 1.6 + 2.0)
            }
            Effect::LayerBlur { radius, visible: true } => m = m.max(radius * 1.6 + 2.0),
            Effect::Texture(t) if t.visible => m = m.max(t.radius.max(0.0) + 2.0),
            _ => {}
        }
    }
    if let NodeKind::Text(t) = &n.kind {
        m = m.max(t.style.font_size * 2.0);
    }
    if matches!(n.kind, NodeKind::Sticky(_)) {
        m = m.max(crate::board::sticky_shadow_reach(n.size));
    }
    m
}

/// How far (local units) pixels of this node's effects depend on content
/// outside the node's own footprint. A tile must be rendered with at least
/// this much padding for blurs to match a full-page render.
fn blur_reach(n: &Node) -> f32 {
    let mut r = 0.0f32;
    for e in &n.effects {
        match e {
            Effect::DropShadow { offset, radius, spread, visible: true, .. } => {
                r = r.max(offset.x.abs().max(offset.y.abs()) + spread.max(0.0) + radius * 1.6 + 2.0)
            }
            Effect::InnerShadow { offset, radius, spread, visible: true, .. } => {
                r = r.max(offset.x.abs().max(offset.y.abs()) + spread.abs() + radius * 1.6 + 2.0)
            }
            Effect::LayerBlur { radius, visible: true } | Effect::BackgroundBlur { radius, visible: true } if *radius > 0.0 => r = r.max(radius * 1.6 + 2.0),
            Effect::Texture(t) if t.visible => r = r.max(t.radius.max(0.0) + 2.0),
            Effect::Glass(g) if g.visible => r = r.max(g.frost.max(0.0) * 1.6 + g.depth.max(0.0) * 2.0 + 6.0),
            _ => {}
        }
    }
    r
}

fn is_chrome_node(doc: &Document, n: &Node, id: NodeId) -> bool {
    if matches!(n.kind, NodeKind::ComponentSet(_)) {
        return true;
    }
    if matches!(&n.kind, NodeKind::Unknown { type_name } if type_name.eq_ignore_ascii_case("SLICE"))
        || (matches!(n.kind, NodeKind::Rectangle) && n.fills.is_empty() && n.stroke.paints.is_empty() && n.name.starts_with("Slice"))
    {
        return true;
    }
    let has_own = n.component.is_slot || n.component.overrides.contains(&designos_doc::OverrideField::SlotContent) || !n.component.bindings.is_empty();
    let has_src = n.component.source.is_some_and(|s| {
        doc.get(s).is_some_and(|sn| {
            sn.component.is_slot || sn.component.overrides.contains(&designos_doc::OverrideField::SlotContent) || !sn.component.bindings.is_empty()
        })
    });
    (has_own || has_src) && designos_doc::is_slot_container(doc, id)
}

fn is_grid_node(n: &Node) -> bool {
    n.kind.frame_props().is_some_and(|fp| !fp.grids.is_empty())
}

/// Child positions from the page down to `id`. Comparing two paths gives paint order:
/// a parent sorts before its descendants and a lower sibling before the ones above it.
fn paint_path(doc: &Document, id: NodeId) -> Vec<usize> {
    let mut path = Vec::new();
    let mut cur = id;
    while let Some(parent) = doc.get(cur).and_then(|n| n.parent) {
        path.push(doc.get(parent).and_then(|p| p.paint_children().iter().position(|&c| c == cur)).unwrap_or(usize::MAX));
        cur = parent;
    }
    path.reverse();
    path
}

#[derive(Clone, Debug)]
pub(crate) struct Entry {
    /// Page-space bounds of the node and everything it draws (empty if hidden).
    pub bounds: B,
    pub parent: Option<NodeId>,
    /// Has a visible layer blur: any change below it spreads over its bounds.
    pub blur_container: bool,
    pub mask: bool,
    pub boolean: bool,
    pub chrome: bool,
    pub has_grids: bool,
    pub children_len: u32,
    pub top_order: u32,
    pub world_ts: [f32; 6],
    pub size: (f32, f32),
    pub rev: u64,
    pub children: Box<[NodeId]>,
}

/// Cached world bounds for one page, rebuilt when the revision changes.
#[derive(Default)]
pub struct SceneIndex {
    page: Option<NodeId>,
    rev: u64,
    doc_id: u64,
    doc_epoch: u64,
    nodes_len: usize,
    built: bool,
    pub(crate) entries: IdMap<Entry>,
    /// Nodes with blur-like effects: (subtree bounds, reach in page units).
    pub(crate) blurry: Vec<(B, f32)>,
    /// Bounds of nodes with a background blur (they read what is beneath).
    pub(crate) bg_blur: Vec<B>,
    pub(crate) top_grid: HashMap<(i32, i32), Vec<NodeId>, BuildHasherDefault<IdHasher>>,
    pub(crate) top_large: Vec<NodeId>,
    pub(crate) chrome_nodes: Vec<NodeId>,
    /// Visible nodes with layout grids, in paint order (see `restore_grid_order`).
    pub(crate) grid_nodes: Vec<NodeId>,
    /// An edit of a built index may have broken the paint order of `grid_nodes`.
    grid_order_stale: bool,
    /// Frames that paint their children in reverse document order (`reverse_z`), as of the last
    /// build or edit. Their `grid_nodes` are not in document order, and toggling the flag moves them.
    reverse_z_frames: HashSet<NodeId>,
    /// Child ids of the page as of the last build; the page has no `Entry` to remember them in.
    page_children: Box<[NodeId]>,
    top_has_mask: bool,
}

impl SceneIndex {
    pub fn new() -> Self {
        Self::default()
    }

    /// Rebuilds if `rev` or `page` differ from the cached build. Returns the
    /// previous entries when a rebuild happened.
    pub(crate) fn ensure_prev(&mut self, doc: &Document, page: NodeId, rev: u64) -> Option<IdMap<Entry>> {
        if self.built && self.page == Some(page) && self.rev == rev {
            return None;
        }
        let prev = std::mem::take(&mut self.entries);
        self.entries.reserve(prev.len().max(64));
        self.blurry.clear();
        self.bg_blur.clear();
        self.top_grid.clear();
        self.top_large.clear();
        self.chrome_nodes.clear();
        self.grid_nodes.clear();
        self.reverse_z_frames.clear();
        self.top_has_mask = false;
        self.page = Some(page);
        self.rev = rev;
        self.doc_id = doc.runtime_id();
        self.doc_epoch = doc.mutation_epoch();
        self.nodes_len = doc.nodes.len();
        self.built = true;
        if let Some(p) = doc.get(page) {
            let id = sk::Transform::identity();
            for c in &p.children {
                self.build(doc, *c, id, Some(page));
            }
        }
        // The build lists the grid nodes in document order, which is paint order unless a
        // `reverse_z` frame reverses its children.
        self.grid_order_stale = !self.reverse_z_frames.is_empty();
        self.restore_grid_order(doc);
        self.rebuild_top_grid(doc, page);
        Some(prev)
    }

    fn rebuild_top_grid(&mut self, doc: &Document, page: NodeId) {
        self.top_grid.clear();
        self.top_large.clear();
        self.top_has_mask = false;
        let Some(p) = doc.get(page) else { return };
        self.page_children = p.children.as_slice().into();
        for (order, &cid) in p.children.iter().enumerate() {
            let Some(n) = doc.get(cid) else { continue };
            if !n.visible {
                continue;
            }
            if n.is_mask {
                self.top_has_mask = true;
            }
            let Some(entry) = self.entries.get_mut(&cid) else { continue };
            entry.top_order = order as u32;
            let bounds = entry.bounds;
            if is_empty(bounds) {
                continue;
            }
            let (cx0, cy0, cx1, cy1) = grid_range(bounds);
            let cells = (cx1 as i64 - cx0 as i64 + 1).max(1).saturating_mul((cy1 as i64 - cy0 as i64 + 1).max(1));
            if cells > 64 {
                self.top_large.push(cid);
            } else {
                for cy in cy0..=cy1 {
                    for cx in cx0..=cx1 {
                        self.top_grid.entry((cx, cy)).or_default().push(cid);
                    }
                }
            }
        }
    }

    pub fn ensure(&mut self, doc: &Document, page: NodeId, rev: u64) {
        if self.built && self.page == Some(page) {
            if self.rev == rev {
                return;
            }
            if self.doc_id == doc.runtime_id() {
                let mut dirty = Vec::new();
                let mut removed = Vec::new();
                if doc.dirty_changes_since(self.doc_epoch, &mut dirty, &mut removed) {
                    if dirty.is_empty() && removed.is_empty() {
                        self.nodes_len = doc.nodes.len();
                        self.rev = rev;
                        self.doc_epoch = doc.mutation_epoch();
                        return;
                    }
                    self.note_child_order_changes(doc, page, &dirty);
                    let mut top_dirty = false;
                    for &rid in &removed {
                        if let Some(old_e) = self.entries.remove(&rid) {
                            if old_e.chrome {
                                self.chrome_nodes.retain(|&x| x != rid);
                            }
                            if old_e.has_grids {
                                self.grid_nodes.retain(|&x| x != rid);
                            }
                            if old_e.parent == Some(page) {
                                top_dirty = true;
                            }
                        }
                    }
                    let mut ok = true;
                    for &id in &dirty {
                        if id == page {
                            top_dirty = true;
                            if let Some(p) = doc.get(page) {
                                let id_ts = sk::Transform::identity();
                                for &cid in &p.children {
                                    if self.entries.get(&cid).is_none_or(|ce| ce.parent != Some(page)) {
                                        self.build(doc, cid, id_ts, Some(page));
                                    }
                                }
                            }
                        } else if doc.get(id).is_some() {
                            if doc.page_of(id) == Some(page) {
                                if !self.apply_subtree_update(doc, id, rev) {
                                    ok = false;
                                    break;
                                }
                            } else {
                                // Moved to another page: nothing of it is drawn here any more.
                                self.forget_grid_nodes_under(doc, id);
                            }
                        }
                    }
                    if ok {
                        self.restore_grid_order(doc);
                        if top_dirty {
                            self.rebuild_top_grid(doc, page);
                        }
                        self.nodes_len = doc.nodes.len();
                        self.rev = rev;
                        self.doc_epoch = doc.mutation_epoch();
                        return;
                    }
                }
            }
        }
        self.entries.clear();
        self.blurry.clear();
        self.bg_blur.clear();
        self.top_grid.clear();
        self.top_large.clear();
        self.chrome_nodes.clear();
        self.grid_nodes.clear();
        self.reverse_z_frames.clear();
        self.top_has_mask = false;
        self.page = Some(page);
        self.rev = rev;
        self.doc_id = doc.runtime_id();
        self.doc_epoch = doc.mutation_epoch();
        self.nodes_len = doc.nodes.len();
        self.built = true;
        if let Some(p) = doc.get(page) {
            let id = sk::Transform::identity();
            for c in &p.children {
                self.build(doc, *c, id, Some(page));
            }
        }
        self.grid_order_stale = !self.reverse_z_frames.is_empty();
        self.restore_grid_order(doc);
        self.rebuild_top_grid(doc, page);
    }

    /// Forgets the cached build so the next `ensure` rebuilds.
    pub fn clear(&mut self) {
        self.built = false;
        self.entries.clear();
        self.top_grid.clear();
        self.top_large.clear();
        self.chrome_nodes.clear();
        self.grid_nodes.clear();
        self.grid_order_stale = false;
        self.reverse_z_frames.clear();
        self.top_has_mask = false;
    }

    /// True unless the subtree of `id` is known to miss `win`.
    pub(crate) fn may_touch(&self, id: NodeId, win: B) -> bool {
        match self.entries.get(&id) {
            Some(e) => overlaps(e.bounds, win),
            None => true,
        }
    }

    /// Page-space bounds (x0, y0, x1, y1) of a node's drawn subtree, if indexed and visible.
    pub fn bounds_of(&self, id: NodeId) -> Option<(f32, f32, f32, f32)> {
        self.entries.get(&id).map(|e| e.bounds).filter(|b| !is_empty(*b))
    }

    /// Returns `(bounds, is_mask)` for a visible indexed node in a single hash lookup,
    /// or `None` if not indexed or hidden (`is_empty(bounds)`).
    #[inline]
    pub fn cull_info(&self, id: NodeId) -> Option<((f32, f32, f32, f32), bool)> {
        let e = self.entries.get(&id)?;
        if is_empty(e.bounds) {
            return None;
        }
        Some((e.bounds, e.mask))
    }

    /// Nodes on the active page that need ComponentSet / Slot / Slice overlay chrome.
    #[inline]
    pub fn chrome_nodes(&self) -> &[NodeId] {
        &self.chrome_nodes
    }

    /// Drawn nodes on the active page that have non-empty `FrameProps::grids`, in paint order:
    /// a node with a hidden ancestor is not drawn, so it is not listed.
    #[inline]
    pub fn grid_nodes(&self) -> &[NodeId] {
        &self.grid_nodes
    }

    fn note_grid_node(&mut self, id: NodeId) {
        if !self.grid_nodes.contains(&id) {
            self.grid_nodes.push(id);
            self.grid_order_stale = true;
        }
    }

    /// Stops listing `root` and every node beneath it, for a subtree that is no longer drawn.
    fn forget_grid_nodes_under(&mut self, doc: &Document, root: NodeId) {
        if !self.grid_nodes.is_empty() {
            self.grid_nodes.retain(|&id| id != root && !doc.is_ancestor(root, id));
        }
    }

    /// Flags `grid_nodes` for re-sorting when a dirty container lists its children in another order
    /// than the build saw: every grid node beneath it then paints at a different position.
    /// Must run before the dirty nodes are re-indexed, which overwrite the remembered child lists.
    fn note_child_order_changes(&mut self, doc: &Document, page: NodeId, dirty: &[NodeId]) {
        // Flipping `reverse_z` on a frame reverses the paint order of everything beneath it.
        // Track the flag first: it must stay current even when no re-sort is needed yet.
        let mut flipped = false;
        for &id in dirty {
            let now = doc.get(id).is_some_and(|n| n.paints_children_reversed());
            flipped |= if now { self.reverse_z_frames.insert(id) } else { self.reverse_z_frames.remove(&id) };
        }
        if self.grid_order_stale || self.grid_nodes.len() < 2 {
            return;
        }
        self.grid_order_stale = flipped
            || dirty.iter().any(|&id| {
                let Some(n) = doc.get(id) else { return false };
                if id == page {
                    return *self.page_children != *n.children;
                }
                n.visible && self.entries.get(&id).is_some_and(|e| *e.children != *n.children)
            });
    }

    /// Re-sorts `grid_nodes` into paint order after an edit listed nodes out of order.
    fn restore_grid_order(&mut self, doc: &Document) {
        if std::mem::take(&mut self.grid_order_stale) && self.grid_nodes.len() > 1 {
            self.grid_nodes.sort_by_cached_key(|&id| paint_path(doc, id));
        }
    }

    /// Cached page-space affine matrix `[sx, ky, kx, sy, tx, ty]` (`a, b, c, d, tx, ty`) for `id`.
    #[inline]
    pub fn world_transform_of(&self, id: NodeId) -> Option<[f32; 6]> {
        self.entries.get(&id).map(|e| e.world_ts)
    }

    /// True if any visible top-level child of the indexed page is a mask node.
    #[inline]
    pub fn has_top_mask(&self) -> bool {
        self.top_has_mask
    }

    /// Appends all indexed `NodeId`s whose `bounds` overlap `rect`.
    pub fn query_rect(&self, rect: (f32, f32, f32, f32), out: &mut Vec<NodeId>) {
        if is_empty(rect) {
            return;
        }
        for (&id, entry) in &self.entries {
            if overlaps(entry.bounds, rect) {
                out.push(id);
            }
        }
    }

    /// Appends top-level page children whose `Entry.bounds` overlap `rect`,
    /// querying `top_grid` cells overlapping `rect` plus `top_large`, deduplicating,
    /// and preserving back-to-front paint order.
    pub fn query_top_level(&self, rect: (f32, f32, f32, f32), out: &mut Vec<NodeId>) {
        if is_empty(rect) {
            return;
        }
        let start_len = out.len();
        let (cx0, cy0, cx1, cy1) = grid_range(rect);
        let mut seen: IdMap<()> = IdMap::default();
        let cells = (cx1 as i64 - cx0 as i64 + 1).max(1).saturating_mul((cy1 as i64 - cy0 as i64 + 1).max(1));
        if cells <= 4096 {
            for cy in cy0..=cy1 {
                for cx in cx0..=cx1 {
                    if let Some(bucket) = self.top_grid.get(&(cx, cy)) {
                        for &id in bucket {
                            if seen.insert(id, ()).is_none() && self.entries.get(&id).is_some_and(|e| e.parent == self.page && overlaps(e.bounds, rect)) {
                                out.push(id);
                            }
                        }
                    }
                }
            }
        } else {
            for (&(cx, cy), bucket) in &self.top_grid {
                if cx >= cx0 && cx <= cx1 && cy >= cy0 && cy <= cy1 {
                    for &id in bucket {
                        if seen.insert(id, ()).is_none() && self.entries.get(&id).is_some_and(|e| e.parent == self.page && overlaps(e.bounds, rect)) {
                            out.push(id);
                        }
                    }
                }
            }
        }
        for &id in &self.top_large {
            if seen.insert(id, ()).is_none() && self.entries.get(&id).is_some_and(|e| e.parent == self.page && overlaps(e.bounds, rect)) {
                out.push(id);
            }
        }
        if out.len() - start_len > 1 {
            out[start_len..].sort_unstable_by_key(|id| self.entries.get(id).map_or(u32::MAX, |e| e.top_order));
        }
    }

    fn translate_descendants(&mut self, root: NodeId, dx: f32, dy: f32, rev: u64) {
        let mut stack: Vec<NodeId> = Vec::new();
        if let Some(re) = self.entries.get(&root) {
            if !re.children.is_empty() {
                stack.extend_from_slice(&re.children);
            }
        }
        while let Some(id) = stack.pop() {
            if let Some(e) = self.entries.get_mut(&id) {
                if e.rev == rev {
                    continue;
                }
                e.world_ts[4] += dx;
                e.world_ts[5] += dy;
                if !is_empty(e.bounds) {
                    e.bounds = (e.bounds.0 + dx, e.bounds.1 + dy, e.bounds.2 + dx, e.bounds.3 + dy);
                    if !e.children.is_empty() {
                        stack.extend_from_slice(&e.children);
                    }
                }
            }
        }
    }

    /// Incrementally rebuilds `dirty_id`'s subtree and updates ancestor `Entry.bounds`
    /// up to `self.page`, stopping early when an ancestor's `bounds` does not change.
    pub fn update_subtree(&mut self, doc: &Document, dirty_id: NodeId, rev: u64) -> bool {
        if let Some(page) = self.page {
            self.note_child_order_changes(doc, page, &[page, dirty_id]);
        }
        let updated = self.apply_subtree_update(doc, dirty_id, rev);
        self.restore_grid_order(doc);
        updated
    }

    /// `update_subtree` without re-sorting `grid_nodes`, so a batch of updates sorts once.
    fn apply_subtree_update(&mut self, doc: &Document, dirty_id: NodeId, rev: u64) -> bool {
        if !self.built {
            return false;
        }
        let Some(page) = self.page else {
            return false;
        };
        if dirty_id == page || doc.get(dirty_id).is_none() {
            return false;
        }
        let mut ancestors: Vec<NodeId> = Vec::new();
        let mut cur = dirty_id;
        loop {
            let Some(p) = doc.get(cur).and_then(|n| n.parent).or_else(|| self.entries.get(&cur).and_then(|e| e.parent)) else {
                return false;
            };
            if p == page {
                break;
            }
            if ancestors.len() >= 1024 || p == dirty_id || ancestors.contains(&p) {
                return false;
            }
            ancestors.push(p);
            cur = p;
        }
        let top_id = ancestors.last().copied().unwrap_or(dirty_id);
        if !doc.get(page).is_some_and(|p| p.children.contains(&top_id)) {
            return false;
        }

        let mut ancestor_transforms: Vec<(NodeId, sk::Transform)> = Vec::with_capacity(ancestors.len());
        let mut parent_ts = sk::Transform::identity();
        for &aid in ancestors.iter().rev() {
            if !self.entries.contains_key(&aid) {
                return self.apply_subtree_update(doc, aid, rev);
            }
            let Some(an) = doc.get(aid) else {
                return false;
            };
            let an = &*designos_doc::resolve_node(doc, an);
            let drawn = an.visible && (!matches!(an.kind, NodeKind::Unknown { .. }) || !an.children.is_empty()) && !matches!(an.kind, NodeKind::Page { .. });
            if !drawn {
                // Under a hidden ancestor nothing of this subtree is drawn, whatever its own flags say.
                self.forget_grid_nodes_under(doc, dirty_id);
                self.rev = rev;
                return true;
            }
            parent_ts = parent_ts.pre_concat(to_sk_transform(&an.transform));
            ancestor_transforms.push((aid, parent_ts));
        }

        let old_top = self.entries.get(&top_id).map(|e| (e.bounds, e.mask, e.top_order, e.parent));
        let parent = Some(ancestors.first().copied().unwrap_or(page));

        let mut fast_done = false;
        if let Some(raw_n) = doc.get(dirty_id) {
            let n = &*designos_doc::resolve_node(doc, raw_n);
            let unknown = matches!(n.kind, NodeKind::Unknown { .. });
            let drawn = n.visible && (!unknown || !n.children.is_empty()) && !matches!(n.kind, NodeKind::Page { .. });
            let boolean = matches!(n.kind, NodeKind::BooleanOp(_));
            if drawn && !boolean {
                if let Some(old_e) = self.entries.get(&dirty_id).cloned() {
                    if !is_empty(old_e.bounds) && !old_e.boolean && n.is_mask == old_e.mask {
                        let ts = parent_ts.pre_concat(to_sk_transform(&n.transform));
                        let new_wts = [ts.sx, ts.ky, ts.kx, ts.sy, ts.tx, ts.ty];
                        let same_linear = (new_wts[0] - old_e.world_ts[0]).abs() < 1e-5
                            && (new_wts[1] - old_e.world_ts[1]).abs() < 1e-5
                            && (new_wts[2] - old_e.world_ts[2]).abs() < 1e-5
                            && (new_wts[3] - old_e.world_ts[3]).abs() < 1e-5;
                        if same_linear {
                            let dx = new_wts[4] - old_e.world_ts[4];
                            let dy = new_wts[5] - old_e.world_ts[5];
                            let stationary = dx.abs() <= 1e-5 && dy.abs() <= 1e-5 && n.variables.modes.is_empty() && n.component.values.is_empty();
                            let pure_translate = (dx.abs() > 1e-5 || dy.abs() > 1e-5)
                                && n.children.len() as u32 == old_e.children_len
                                && (n.size.x - old_e.size.0).abs() < 1e-4
                                && (n.size.y - old_e.size.1).abs() < 1e-4
                                && n.children.iter().all(|c| self.entries.get(c).is_some_and(|ce| ce.parent == Some(dirty_id)));
                            if stationary || pure_translate {
                                let children_box = if pure_translate {
                                    self.translate_descendants(dirty_id, dx, dy, rev);
                                    // Same children can still have been put in another order.
                                    if *old_e.children == *n.children {
                                        old_e.children
                                    } else {
                                        n.children.as_slice().into()
                                    }
                                } else {
                                    for &c in &n.children {
                                        if self.entries.get(&c).is_none_or(|ce| ce.parent != Some(dirty_id)) {
                                            self.build(doc, c, ts, Some(dirty_id));
                                        }
                                    }
                                    n.children.as_slice().into()
                                };
                                let mut sub = if unknown {
                                    EMPTY
                                } else {
                                    let m = node_margin(n);
                                    transformed_bounds(&ts, -m, -m, n.size.x + 2.0 * m, n.size.y + 2.0 * m)
                                };
                                if !n.children.is_empty() {
                                    let mut kids = EMPTY;
                                    for c in &n.children {
                                        let cb = self.entries.get(c).map(|e| e.bounds).unwrap_or(EMPTY);
                                        kids = union(kids, cb);
                                    }
                                    if !is_empty(kids) {
                                        if n.kind.frame_props().map(|f| f.clips_content).unwrap_or(false) {
                                            kids = intersect(kids, transformed_bounds(&ts, 0.0, 0.0, n.size.x, n.size.y));
                                        }
                                        if !is_empty(kids) {
                                            sub = union(sub, kids);
                                        }
                                    }
                                }
                                let layer_blur =
                                    !unknown && n.effects.iter().any(|e| matches!(e, Effect::LayerBlur { visible: true, radius } if *radius > 0.0));
                                let chrome = is_chrome_node(doc, n, dirty_id);
                                if chrome {
                                    if !self.chrome_nodes.contains(&dirty_id) {
                                        self.chrome_nodes.push(dirty_id);
                                    }
                                } else if old_e.chrome {
                                    self.chrome_nodes.retain(|&x| x != dirty_id);
                                }
                                let has_grids = is_grid_node(n);
                                if has_grids {
                                    self.note_grid_node(dirty_id);
                                } else if old_e.has_grids {
                                    self.grid_nodes.retain(|&x| x != dirty_id);
                                }
                                self.entries.insert(
                                    dirty_id,
                                    Entry {
                                        bounds: sub,
                                        parent,
                                        blur_container: layer_blur,
                                        mask: n.is_mask,
                                        boolean: false,
                                        chrome,
                                        has_grids,
                                        children_len: n.children.len() as u32,
                                        top_order: old_e.top_order,
                                        world_ts: new_wts,
                                        size: (n.size.x, n.size.y),
                                        rev,
                                        children: children_box,
                                    },
                                );
                                fast_done = true;
                            }
                        }
                    }
                }
            }
        }
        if !fast_done {
            self.build(doc, dirty_id, parent_ts, parent);
            if let Some(e) = self.entries.get_mut(&dirty_id) {
                e.rev = rev;
            }
        }

        for &(aid, ts) in ancestor_transforms.iter().rev() {
            let Some(an) = doc.get(aid) else {
                break;
            };
            let an = &*designos_doc::resolve_node(doc, an);
            let unknown = matches!(an.kind, NodeKind::Unknown { .. });
            let boolean = matches!(an.kind, NodeKind::BooleanOp(_));
            let mut sub = if unknown {
                EMPTY
            } else {
                let m = node_margin(an);
                transformed_bounds(&ts, -m, -m, an.size.x + 2.0 * m, an.size.y + 2.0 * m)
            };
            let draws_kids = !an.children.is_empty() && !boolean;
            let mut kids = EMPTY;
            if draws_kids {
                for c in &an.children {
                    let cb = self.entries.get(c).map(|e| e.bounds).unwrap_or(EMPTY);
                    kids = union(kids, cb);
                }
            }
            if !is_empty(kids) {
                if an.kind.frame_props().map(|f| f.clips_content).unwrap_or(false) {
                    kids = intersect(kids, transformed_bounds(&ts, 0.0, 0.0, an.size.x, an.size.y));
                }
                if !is_empty(kids) {
                    sub = union(sub, kids);
                }
            }
            let Some(entry) = self.entries.get_mut(&aid) else {
                break;
            };
            if entry.children_len != an.children.len() as u32 {
                entry.children_len = an.children.len() as u32;
                entry.children = an.children.as_slice().into();
            }
            if entry.bounds == sub {
                break;
            }
            entry.bounds = sub;
        }

        if let Some((_, _, top_order, _)) = old_top {
            if let Some(e) = self.entries.get_mut(&top_id) {
                e.top_order = top_order;
            }
        }
        let top_cell = |b: Option<B>| b.filter(|b| !is_empty(*b)).map(grid_range);
        let new_top = self.entries.get(&top_id).map(|e| (e.bounds, e.mask, e.parent));
        if top_cell(new_top.map(|t| t.0)) != top_cell(old_top.map(|t| t.0))
            || new_top.map(|t| t.1) != old_top.map(|t| t.1)
            || new_top.map(|t| t.2) != old_top.map(|t| t.3)
        {
            self.rebuild_top_grid(doc, page);
        }
        self.rev = rev;
        true
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    fn build(&mut self, doc: &Document, id: NodeId, parent_ts: sk::Transform, parent: Option<NodeId>) -> B {
        let Some(n) = doc.get(id) else { return EMPTY };
        let n = &*designos_doc::resolve_node(doc, n);
        let unknown = matches!(n.kind, NodeKind::Unknown { .. });
        let drawn = n.visible && (!unknown || !n.children.is_empty()) && !matches!(n.kind, NodeKind::Page { .. });
        let chrome = n.visible && is_chrome_node(doc, n, id);
        if chrome {
            if !self.chrome_nodes.contains(&id) {
                self.chrome_nodes.push(id);
            }
        } else if !self.chrome_nodes.is_empty() && self.entries.get(&id).is_some_and(|e| e.chrome) {
            self.chrome_nodes.retain(|&x| x != id);
        }
        let has_grids = n.visible && is_grid_node(n);
        if has_grids {
            self.note_grid_node(id);
        } else if !self.grid_nodes.is_empty() && self.entries.get(&id).is_some_and(|e| e.has_grids) {
            self.grid_nodes.retain(|&x| x != id);
        }
        let boolean = matches!(n.kind, NodeKind::BooleanOp(_));
        let children_len = n.children.len() as u32;
        if drawn && n.paints_children_reversed() {
            self.reverse_z_frames.insert(id);
        }
        let ts = parent_ts.pre_concat(to_sk_transform(&n.transform));
        let world_ts = [ts.sx, ts.ky, ts.kx, ts.sy, ts.tx, ts.ty];
        let size = (n.size.x, n.size.y);
        if !drawn {
            // The subtree below is no longer walked, so the grids in it stop being drawn.
            if self.entries.get(&id).is_some_and(|e| !e.children.is_empty()) {
                self.forget_grid_nodes_under(doc, id);
            }
            self.entries.insert(
                id,
                Entry {
                    bounds: EMPTY,
                    parent,
                    blur_container: false,
                    mask: n.is_mask,
                    boolean,
                    chrome,
                    has_grids,
                    children_len,
                    top_order: 0,
                    world_ts,
                    size,
                    rev: self.rev,
                    children: Box::new([]),
                },
            );
            return EMPTY;
        }
        let mut sub = if unknown {
            EMPTY
        } else {
            let m = node_margin(n);
            transformed_bounds(&ts, -m, -m, n.size.x + 2.0 * m, n.size.y + 2.0 * m)
        };
        let draws_kids = !n.children.is_empty() && !boolean;
        let mut kids = EMPTY;
        for c in &n.children {
            let cb = self.build(doc, *c, ts, Some(id));
            if draws_kids {
                kids = union(kids, cb);
            }
        }
        if !is_empty(kids) {
            if n.kind.frame_props().map(|f| f.clips_content).unwrap_or(false) {
                kids = intersect(kids, transformed_bounds(&ts, 0.0, 0.0, n.size.x, n.size.y));
            }
            if !is_empty(kids) {
                sub = union(sub, kids);
            }
        }
        let layer_blur = !unknown && n.effects.iter().any(|e| matches!(e, Effect::LayerBlur { visible: true, radius } if *radius > 0.0));
        if !unknown && !is_empty(sub) {
            let reach = blur_reach(n);
            if reach > 0.0 {
                // `ts` is a pure scale/rotation here; take its uniform scale
                let sc = (ts.sx * ts.sy - ts.kx * ts.ky).abs().sqrt().max(1e-6);
                self.blurry.push((sub, reach * sc));
            }
            if n.effects
                .iter()
                .any(|e| matches!(e, Effect::BackgroundBlur { visible: true, radius } if *radius > 0.0) || matches!(e, Effect::Glass(g) if g.visible))
            {
                self.bg_blur.push(sub);
            }
        }
        let children: Box<[NodeId]> = if n.children.is_empty() { Box::new([]) } else { n.children.as_slice().into() };
        self.entries.insert(
            id,
            Entry {
                bounds: sub,
                parent,
                blur_container: layer_blur,
                mask: n.is_mask,
                boolean,
                chrome,
                has_grids,
                children_len,
                top_order: 0,
                world_ts,
                size,
                rev: self.rev,
                children,
            },
        );
        sub
    }
}

/// Snapshot of every node of the page as of the last sync; compared against
/// the live document to find what changed.
#[derive(Default)]
pub(crate) struct Differ {
    snap: IdMap<Node>,
}

/// Result of comparing a revision against the previous one.
pub(crate) struct Changes {
    /// Nothing is known about the previous state: drop everything.
    pub all: bool,
    /// Page-space regions whose pixels may differ.
    pub rects: Vec<B>,
}

fn same_ignoring_structure(a: &Node, b: &Node) -> bool {
    if a == b {
        return true;
    }
    let mut t = b.clone();
    t.children = a.children.clone();
    t.parent = a.parent;
    t == *a
}

impl Differ {
    pub fn reset(&mut self) {
        self.snap.clear();
    }

    /// Compares the live page (indexed in `new`, previously indexed in `old`)
    /// with the snapshot and refreshes the snapshot.
    pub fn diff(&mut self, doc: &Document, page: NodeId, old: &IdMap<Entry>, new: &SceneIndex) -> Changes {
        let first = self.snap.is_empty();
        let mut rects: Vec<B> = Vec::new();
        // nodes whose change can spread to ancestors (blur layers, masks, boolean ops)
        let mut spread: Vec<NodeId> = Vec::new();
        let mut all = first;

        let obounds = |id: &NodeId| old.get(id).map(|e| e.bounds).unwrap_or(EMPTY);
        let nbounds = |id: &NodeId| new.entries.get(id).map(|e| e.bounds).unwrap_or(EMPTY);
        let is_mask = |id: &NodeId| new.entries.get(id).is_some_and(|e| e.mask) || old.get(id).is_some_and(|e| e.mask);

        // page node itself (background colour etc.)
        if let (Some(p), Some(s)) = (doc.get(page), self.snap.get(&page)) {
            if !same_ignoring_structure(s, p) {
                all = true;
            }
        }
        if let Some(p) = doc.get(page) {
            let changed_children = self.snap.get(&page).map(|s| s.children != p.children).unwrap_or(true);
            if changed_children {
                self.children_delta(
                    self.snap.get(&page).map(|s| s.children.clone()).unwrap_or_default(),
                    &p.children,
                    &obounds,
                    &nbounds,
                    &is_mask,
                    &mut rects,
                    &mut spread,
                );
            }
            self.snap.insert(page, p.clone());
        }

        for (id, ne) in new.entries.iter() {
            let Some(live) = doc.get(*id) else { continue };
            // Compare with variable bindings applied, so editing a variable or a
            // mode dirties exactly the nodes whose drawn values changed.
            let live = &*designos_doc::resolve_node(doc, live);
            match self.snap.get(id) {
                None => {
                    rects.push(ne.bounds);
                    spread.push(*id);
                    self.snap.insert(*id, live.clone());
                }
                Some(s) if s == live => {}
                Some(s) => {
                    let props_changed = !same_ignoring_structure(s, live);
                    if props_changed {
                        rects.push(obounds(id));
                        rects.push(ne.bounds);
                        spread.push(*id);
                        if s.is_mask != live.is_mask {
                            spread.push(*id);
                        }
                    }
                    if s.children != live.children {
                        self.children_delta(s.children.clone(), &live.children, &obounds, &nbounds, &is_mask, &mut rects, &mut spread);
                        spread.push(*id);
                    }
                    self.snap.insert(*id, live.clone());
                }
            }
        }
        // removed nodes
        let gone: Vec<NodeId> = self.snap.keys().filter(|k| **k != page && !new.entries.contains_key(k)).copied().collect();
        for g in gone {
            rects.push(obounds(&g));
            self.snap.remove(&g);
        }

        if !all {
            // ancestors that re-blur / re-mask / re-union what changed below them
            let mut extra: Vec<B> = Vec::new();
            for id in spread {
                let Some(e) = new.entries.get(&id).or_else(|| old.get(&id)) else { continue };
                if e.mask {
                    // a mask clips every sibling that follows it
                    if let Some(pnode) = e.parent.and_then(|p| doc.get(p)) {
                        let sibs = pnode.paint_children();
                        if let Some(pos) = sibs.iter().position(|c| *c == id) {
                            for c in &sibs[pos + 1..] {
                                extra.push(obounds(c));
                                extra.push(nbounds(c));
                            }
                        }
                    }
                }
                let mut parent = e.parent;
                while let Some(p) = parent {
                    let Some(pe) = new.entries.get(&p) else { break };
                    if pe.blur_container || pe.boolean {
                        extra.push(pe.bounds);
                        extra.push(obounds(&p));
                    }
                    parent = pe.parent;
                }
            }
            rects.extend(extra);
            rects.retain(|r| !is_empty(*r));
            // background blurs resample whatever lies beneath them
            if !new.bg_blur.is_empty() && !rects.is_empty() {
                let hits: Vec<B> = new.bg_blur.iter().copied().filter(|b| rects.iter().any(|r| overlaps(*r, *b))).collect();
                rects.extend(hits);
            }
        }
        Changes { all, rects }
    }

    /// Dirty regions for an edit of a child list: everything between the first
    /// and last position that differs (inserts, removals, z-order moves).
    #[allow(clippy::too_many_arguments)]
    fn children_delta(
        &self,
        old: Vec<NodeId>,
        new: &[NodeId],
        obounds: &dyn Fn(&NodeId) -> B,
        nbounds: &dyn Fn(&NodeId) -> B,
        is_mask: &dyn Fn(&NodeId) -> bool,
        rects: &mut Vec<B>,
        spread: &mut Vec<NodeId>,
    ) {
        let pre = old.iter().zip(new).take_while(|(a, b)| a == b).count();
        let suf = old[pre..].iter().rev().zip(new[pre..].iter().rev()).take_while(|(a, b)| a == b).count();
        let mut masks = false;
        for id in &old[pre..old.len() - suf] {
            rects.push(obounds(id));
            spread.push(*id);
            masks |= is_mask(id);
        }
        for id in &new[pre..new.len() - suf] {
            rects.push(nbounds(id));
            spread.push(*id);
            masks |= is_mask(id);
        }
        if masks {
            // a mask added / removed / moved changes every sibling after it
            for id in &new[pre..] {
                rects.push(nbounds(id));
                rects.push(obounds(id));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use designos_doc::{Affine, Rect, Vec2};

    #[test]
    fn query_top_level_and_update_subtree_match_full_ensure() {
        let mut doc = Document::new("test");
        let page = doc.add_page("page-1");
        let other_page = doc.add_page("page-2");

        // Top-level frame 1 (unclipped) in cell (0, 0)
        let f1_id = doc.alloc_id();
        let mut f1 = Node::frame(f1_id, Rect::new(100.0, 100.0, 400.0, 400.0));
        if let NodeKind::Frame(fp) = &mut f1.kind {
            fp.clips_content = false;
        }
        let f1_id = doc.insert(page, f1).unwrap();

        let g1_id = doc.alloc_id();
        let mut g1 = Node::new(g1_id, NodeKind::Group);
        g1.size = Vec2 { x: 100.0, y: 100.0 };
        g1.transform = Affine::translate(50.0, 50.0);
        let g1_id = doc.insert(f1_id, g1).unwrap();

        let child1_id = doc.alloc_id();
        let child1 = doc.insert(g1_id, Node::rectangle(child1_id, Rect::new(10.0, 10.0, 80.0, 80.0))).unwrap();

        // Top-level frame 2 (clipped) spanning into cell (1, 0)
        let f2_id = doc.alloc_id();
        let mut f2 = Node::frame(f2_id, Rect::new(1900.0, 200.0, 500.0, 500.0));
        if let NodeKind::Frame(fp) = &mut f2.kind {
            fp.clips_content = true;
        }
        let f2_id = doc.insert(page, f2).unwrap();

        let child2_id = doc.alloc_id();
        let child2 = doc.insert(f2_id, Node::rectangle(child2_id, Rect::new(20.0, 20.0, 100.0, 100.0))).unwrap();

        // Top-level node in negative grid cell (-2, -2)
        let neg_id = doc.alloc_id();
        let neg_id = doc.insert(page, Node::rectangle(neg_id, Rect::new(-3500.0, -3500.0, 300.0, 300.0))).unwrap();

        // Large top-level node spanning > 64 grid cells (stored in `top_large`)
        let large_id = doc.alloc_id();
        let large_id = doc.insert(page, Node::rectangle(large_id, Rect::new(-10000.0, -10000.0, 25000.0, 25000.0))).unwrap();

        // Node on a different page
        let other_node_id = doc.alloc_id();
        let other_node_id = doc.insert(other_page, Node::rectangle(other_node_id, Rect::new(0.0, 0.0, 50.0, 50.0))).unwrap();

        let mut inc_idx = SceneIndex::new();
        assert!(!inc_idx.update_subtree(&doc, child1, 1));

        inc_idx.ensure(&doc, page, 1);
        assert!(inc_idx.top_large.contains(&large_id));
        assert!(!inc_idx.update_subtree(&doc, page, 2));
        assert!(!inc_idx.update_subtree(&doc, other_node_id, 2));

        let query_rects = [(0.0, 0.0, 600.0, 600.0), (1800.0, 100.0, 2500.0, 800.0), (-4000.0, -4000.0, -3000.0, -3000.0), (5000.0, 5000.0, 5500.0, 5500.0)];

        // 1. Move deep child `child1` far outside unclipped `f1` so `g1` and `f1` bounds expand across cells
        doc.get_mut(child1).unwrap().transform = Affine::translate(2600.0, 2600.0);
        assert!(inc_idx.update_subtree(&doc, child1, 2));

        // 2. Move `child2` inside clipped `f2` (stopped early by clipping / unchanged ancestor bounds)
        doc.get_mut(child2).unwrap().transform = Affine::translate(60.0, 60.0);
        assert!(inc_idx.update_subtree(&doc, child2, 3));

        // 3. Move top-level `neg_id` to a new grid cell
        doc.get_mut(neg_id).unwrap().transform = Affine::translate(5100.0, 5100.0);
        assert!(inc_idx.update_subtree(&doc, neg_id, 4));

        let mut full_idx = SceneIndex::new();
        full_idx.ensure(&doc, page, 4);

        for id in [f1_id, g1_id, child1, f2_id, child2, neg_id, large_id] {
            assert_eq!(inc_idx.bounds_of(id), full_idx.bounds_of(id), "bounds mismatch for {:?}", id);
        }

        for r in query_rects {
            let mut inc_out = Vec::new();
            let mut full_out = Vec::new();
            inc_idx.query_top_level(r, &mut inc_out);
            full_idx.query_top_level(r, &mut full_out);
            inc_out.sort_by_key(|n| n.0);
            full_out.sort_by_key(|n| n.0);
            assert_eq!(inc_out, full_out, "query_top_level mismatch for rect {:?}", r);

            // Also verify against brute-force scan of top-level page children
            let mut expected: Vec<NodeId> =
                doc.get(page).unwrap().children.iter().copied().filter(|&cid| full_idx.bounds_of(cid).is_some_and(|b| overlaps(b, r))).collect();
            expected.sort_by_key(|n| n.0);
            assert_eq!(inc_out, expected);
        }
    }

    #[test]
    fn incremental_insert_remove_reparent_and_translate_match_full_ensure() {
        let mut doc = Document::new("inc-test");
        let page = doc.add_page("page-1");

        let f1_id = doc.alloc_id();
        let mut f1 = Node::frame(f1_id, Rect::new(100.0, 100.0, 500.0, 400.0));
        if let NodeKind::Frame(fp) = &mut f1.kind {
            fp.clips_content = false;
        }
        doc.insert(page, f1).unwrap();

        let g1_id = doc.alloc_id();
        let mut g1 = Node::new(g1_id, NodeKind::Group);
        g1.size = Vec2::new(200.0, 200.0);
        g1.transform = Affine::translate(20.0, 30.0);
        doc.insert(f1_id, g1).unwrap();

        let r1_id = doc.alloc_id();
        doc.insert(g1_id, Node::rectangle(r1_id, Rect::new(10.0, 15.0, 60.0, 40.0))).unwrap();

        let mut inc = SceneIndex::new();
        inc.ensure(&doc, page, 1);

        // 1. Insert new child inside g1_id and a new top-level node on page
        let r2_id = doc.alloc_id();
        doc.insert(g1_id, Node::rectangle(r2_id, Rect::new(300.0, 250.0, 80.0, 90.0))).unwrap();
        let top_rect_id = doc.alloc_id();
        doc.insert(page, Node::rectangle(top_rect_id, Rect::new(2200.0, 300.0, 120.0, 120.0))).unwrap();
        inc.ensure(&doc, page, 2);

        // 2. Pure translation of top-level frame f1_id (translates g1_id, r1_id, r2_id in place)
        doc.get_mut(f1_id).unwrap().transform.tx += 350.0;
        doc.get_mut(f1_id).unwrap().transform.ty += 180.0;
        inc.ensure(&doc, page, 3);

        // 3. Reparent r2_id from g1_id to page, and top_rect_id into f1_id
        doc.reparent(r2_id, page, 1).unwrap();
        doc.reparent(top_rect_id, f1_id, 0).unwrap();
        inc.ensure(&doc, page, 4);

        // 4. Remove r1_id
        doc.remove(r1_id).unwrap();
        inc.ensure(&doc, page, 5);

        let mut full = SceneIndex::new();
        full.ensure(&doc, page, 5);

        assert_eq!(inc.len(), full.len());
        for id in [f1_id, g1_id, r1_id, r2_id, top_rect_id] {
            assert_eq!(inc.bounds_of(id), full.bounds_of(id), "bounds mismatch for {:?}", id);
        }
        let mut inc_top = Vec::new();
        let mut full_top = Vec::new();
        inc.query_top_level((-1000.0, -1000.0, 5000.0, 5000.0), &mut inc_top);
        full.query_top_level((-1000.0, -1000.0, 5000.0, 5000.0), &mut full_top);
        assert_eq!(inc_top, full_top);
    }

    #[test]
    fn grid_nodes_tracks_only_visible_frames_with_layout_grids() {
        let mut doc = Document::new("grid-index-test");
        let page = doc.add_page("page-1");

        let sample_grid = designos_doc::LayoutGrid::grid(8.0, designos_doc::Color::rgba(255, 0, 0, 26));

        let plain_id = doc.alloc_id();
        doc.insert(page, Node::frame(plain_id, Rect::new(0.0, 0.0, 200.0, 200.0))).unwrap();

        let gridded_id = doc.alloc_id();
        let mut gridded = Node::frame(gridded_id, Rect::new(300.0, 0.0, 200.0, 200.0));
        if let NodeKind::Frame(fp) = &mut gridded.kind {
            fp.grids.push(sample_grid.clone());
        }
        doc.insert(page, gridded).unwrap();

        let mut idx = SceneIndex::new();
        idx.ensure(&doc, page, 1);
        assert_eq!(idx.grid_nodes(), &[gridded_id]);

        if let Some(NodeKind::Frame(fp)) = doc.get_mut(plain_id).map(|n| &mut n.kind) {
            fp.grids.push(sample_grid);
        }
        assert!(idx.update_subtree(&doc, plain_id, 2));
        assert!(idx.grid_nodes().contains(&gridded_id));
        assert!(idx.grid_nodes().contains(&plain_id));

        if let Some(NodeKind::Frame(fp)) = doc.get_mut(gridded_id).map(|n| &mut n.kind) {
            fp.grids.clear();
        }
        assert!(idx.update_subtree(&doc, gridded_id, 3));
        assert_eq!(idx.grid_nodes(), &[plain_id]);
    }

    fn column_grid() -> designos_doc::LayoutGrid {
        designos_doc::LayoutGrid::columns(4, 8.0, 0.0, designos_doc::Color::rgba(255, 0, 0, 26))
    }

    fn insert_frame(doc: &mut Document, parent: NodeId, rect: Rect, grid: bool) -> NodeId {
        let id = doc.alloc_id();
        let mut frame = Node::frame(id, rect);
        if let (true, NodeKind::Frame(fp)) = (grid, &mut frame.kind) {
            fp.grids.push(column_grid());
        }
        doc.insert(parent, frame).unwrap()
    }

    fn insert_group(doc: &mut Document, parent: NodeId, visible: bool) -> NodeId {
        let id = doc.alloc_id();
        let mut group = Node::new(id, NodeKind::Group);
        group.size = Vec2 { x: 400.0, y: 400.0 };
        group.visible = visible;
        doc.insert(parent, group).unwrap()
    }

    /// The layout-grid list a from-scratch build produces for the current document.
    fn grid_nodes_of_full_build(doc: &Document, page: NodeId, rev: u64) -> Vec<NodeId> {
        let mut full = SceneIndex::new();
        full.ensure(doc, page, rev);
        full.grid_nodes().to_vec()
    }

    #[test]
    fn hiding_an_ancestor_drops_the_layout_grids_beneath_it() {
        let mut doc = Document::new("grid-hidden-ancestor");
        let page = doc.add_page("page-1");
        let group = insert_group(&mut doc, page, true);
        let framed = insert_frame(&mut doc, group, Rect::new(10.0, 10.0, 200.0, 200.0), true);

        let mut idx = SceneIndex::new();
        idx.ensure(&doc, page, 1);
        assert_eq!(idx.grid_nodes(), &[framed]);

        doc.get_mut(group).unwrap().visible = false;
        idx.ensure(&doc, page, 2);
        assert!(idx.grid_nodes().is_empty(), "a grid under a hidden group is not drawn, so it must not be listed");
        assert_eq!(idx.grid_nodes(), grid_nodes_of_full_build(&doc, page, 2));

        doc.get_mut(group).unwrap().visible = true;
        idx.ensure(&doc, page, 3);
        assert_eq!(idx.grid_nodes(), &[framed], "showing the group again brings the grid back");
    }

    #[test]
    fn hiding_a_deep_ancestor_drops_every_nested_layout_grid() {
        let mut doc = Document::new("grid-hidden-deep-ancestor");
        let page = doc.add_page("page-1");
        let outer = insert_group(&mut doc, page, true);
        let inner = insert_group(&mut doc, outer, true);
        let a = insert_frame(&mut doc, inner, Rect::new(0.0, 0.0, 100.0, 100.0), true);
        let b = insert_frame(&mut doc, outer, Rect::new(120.0, 0.0, 100.0, 100.0), true);
        let kept = insert_frame(&mut doc, page, Rect::new(600.0, 0.0, 100.0, 100.0), true);

        let mut idx = SceneIndex::new();
        idx.ensure(&doc, page, 1);
        assert_eq!(idx.grid_nodes(), &[a, b, kept]);

        doc.get_mut(outer).unwrap().visible = false;
        idx.ensure(&doc, page, 2);
        assert_eq!(idx.grid_nodes(), &[kept]);
        assert_eq!(idx.grid_nodes(), grid_nodes_of_full_build(&doc, page, 2));
    }

    #[test]
    fn moving_a_gridded_frame_under_a_hidden_group_drops_its_layout_grid() {
        let mut doc = Document::new("grid-moved-under-hidden");
        let page = doc.add_page("page-1");
        let hidden = insert_group(&mut doc, page, false);
        let framed = insert_frame(&mut doc, page, Rect::new(10.0, 10.0, 200.0, 200.0), true);

        let mut idx = SceneIndex::new();
        idx.ensure(&doc, page, 1);
        assert_eq!(idx.grid_nodes(), &[framed]);

        doc.reparent(framed, hidden, 0).unwrap();
        idx.ensure(&doc, page, 2);
        assert!(idx.grid_nodes().is_empty());
        assert_eq!(idx.grid_nodes(), grid_nodes_of_full_build(&doc, page, 2));
    }

    #[test]
    fn moving_a_gridded_frame_to_another_page_drops_its_layout_grid() {
        let mut doc = Document::new("grid-moved-to-other-page");
        let page = doc.add_page("page-1");
        let other = doc.add_page("page-2");
        let framed = insert_frame(&mut doc, page, Rect::new(10.0, 10.0, 200.0, 200.0), true);

        let mut idx = SceneIndex::new();
        idx.ensure(&doc, page, 1);
        assert_eq!(idx.grid_nodes(), &[framed]);

        doc.reparent(framed, other, 0).unwrap();
        idx.ensure(&doc, page, 2);
        assert!(idx.grid_nodes().is_empty());
        assert_eq!(idx.grid_nodes(), grid_nodes_of_full_build(&doc, page, 2));
    }

    #[test]
    fn grid_nodes_stay_in_paint_order_after_incremental_edits() {
        let mut doc = Document::new("grid-paint-order");
        let page = doc.add_page("page-1");
        let outer = insert_frame(&mut doc, page, Rect::new(0.0, 0.0, 400.0, 400.0), false);
        let inner = insert_frame(&mut doc, outer, Rect::new(20.0, 20.0, 200.0, 200.0), true);
        let below = insert_frame(&mut doc, page, Rect::new(500.0, 0.0, 200.0, 200.0), false);
        let above = insert_frame(&mut doc, page, Rect::new(500.0, 0.0, 200.0, 200.0), true);

        let mut idx = SceneIndex::new();
        idx.ensure(&doc, page, 1);
        assert_eq!(idx.grid_nodes(), &[inner, above]);

        // A parent that gains a grid after its child already has one still paints beneath the child.
        doc.get_mut(outer).unwrap().kind.frame_props_mut().unwrap().grids.push(column_grid());
        // A lower sibling that gains a grid late still paints beneath the sibling above it.
        doc.get_mut(below).unwrap().kind.frame_props_mut().unwrap().grids.push(column_grid());
        idx.ensure(&doc, page, 2);

        let expected = [outer, inner, below, above];
        assert_eq!(idx.grid_nodes(), &expected);
        assert_eq!(idx.grid_nodes(), grid_nodes_of_full_build(&doc, page, 2));
    }

    #[test]
    fn reordering_siblings_on_the_page_re_sorts_the_layout_grid_list() {
        let mut doc = Document::new("grid-page-reorder");
        let page = doc.add_page("page-1");
        let a = insert_frame(&mut doc, page, Rect::new(0.0, 0.0, 200.0, 200.0), true);
        let b = insert_frame(&mut doc, page, Rect::new(100.0, 0.0, 200.0, 200.0), true);

        let mut idx = SceneIndex::new();
        idx.ensure(&doc, page, 1);
        assert_eq!(idx.grid_nodes(), &[a, b]);

        doc.reparent(a, page, 1).unwrap();
        idx.ensure(&doc, page, 2);
        assert_eq!(idx.grid_nodes(), &[b, a], "the frame that moved up now paints last");
        assert_eq!(idx.grid_nodes(), grid_nodes_of_full_build(&doc, page, 2));
    }

    #[test]
    fn reordering_siblings_inside_a_group_re_sorts_the_layout_grid_list() {
        let mut doc = Document::new("grid-group-reorder");
        let page = doc.add_page("page-1");
        let group = insert_group(&mut doc, page, true);
        let a = insert_frame(&mut doc, group, Rect::new(0.0, 0.0, 200.0, 200.0), true);
        let b = insert_frame(&mut doc, group, Rect::new(100.0, 0.0, 200.0, 200.0), true);

        let mut idx = SceneIndex::new();
        idx.ensure(&doc, page, 1);
        assert_eq!(idx.grid_nodes(), &[a, b]);

        doc.reparent(a, group, 1).unwrap();
        idx.ensure(&doc, page, 2);
        assert_eq!(idx.grid_nodes(), &[b, a]);
        assert_eq!(idx.grid_nodes(), grid_nodes_of_full_build(&doc, page, 2));
    }

    #[test]
    fn swapping_two_artboards_re_sorts_the_layout_grids_inside_them() {
        let mut doc = Document::new("grid-artboard-swap");
        let page = doc.add_page("page-1");
        let first = insert_frame(&mut doc, page, Rect::new(0.0, 0.0, 400.0, 400.0), false);
        let first_child = insert_frame(&mut doc, first, Rect::new(10.0, 10.0, 200.0, 200.0), true);
        let second = insert_frame(&mut doc, page, Rect::new(100.0, 0.0, 400.0, 400.0), false);
        let second_child = insert_frame(&mut doc, second, Rect::new(10.0, 10.0, 200.0, 200.0), true);

        let mut idx = SceneIndex::new();
        idx.ensure(&doc, page, 1);
        assert_eq!(idx.grid_nodes(), &[first_child, second_child]);

        doc.reparent(first, page, 1).unwrap();
        idx.ensure(&doc, page, 2);
        assert_eq!(idx.grid_nodes(), &[second_child, first_child]);
        assert_eq!(idx.grid_nodes(), grid_nodes_of_full_build(&doc, page, 2));
    }

    #[test]
    fn moving_a_listed_frame_into_another_group_re_sorts_the_layout_grid_list() {
        let mut doc = Document::new("grid-move-between-groups");
        let page = doc.add_page("page-1");
        let group = insert_group(&mut doc, page, true);
        let inside = insert_frame(&mut doc, group, Rect::new(0.0, 0.0, 200.0, 200.0), true);
        let moving = insert_frame(&mut doc, page, Rect::new(100.0, 0.0, 200.0, 200.0), true);

        let mut idx = SceneIndex::new();
        idx.ensure(&doc, page, 1);
        assert_eq!(idx.grid_nodes(), &[inside, moving]);

        doc.reparent(moving, group, 0).unwrap();
        idx.ensure(&doc, page, 2);
        assert_eq!(idx.grid_nodes(), &[moving, inside]);
        assert_eq!(idx.grid_nodes(), grid_nodes_of_full_build(&doc, page, 2));
    }

    #[test]
    fn reordering_while_the_group_is_translated_still_re_sorts_later_reorders() {
        let mut doc = Document::new("grid-translate-and-reorder");
        let page = doc.add_page("page-1");
        let group = insert_frame(&mut doc, page, Rect::new(0.0, 0.0, 600.0, 400.0), false);
        let a = insert_frame(&mut doc, group, Rect::new(0.0, 0.0, 200.0, 200.0), true);
        let b = insert_frame(&mut doc, group, Rect::new(100.0, 0.0, 200.0, 200.0), true);

        let mut idx = SceneIndex::new();
        idx.ensure(&doc, page, 1);
        assert_eq!(idx.grid_nodes(), &[a, b]);

        // One batch: the group moves and its children swap places.
        doc.get_mut(group).unwrap().transform.tx += 50.0;
        doc.reparent(a, group, 1).unwrap();
        idx.ensure(&doc, page, 2);
        assert_eq!(idx.grid_nodes(), &[b, a]);

        // Swapping back must be noticed too: the index has to remember the new child order.
        doc.reparent(a, group, 0).unwrap();
        idx.ensure(&doc, page, 3);
        assert_eq!(idx.grid_nodes(), &[a, b]);
        assert_eq!(idx.grid_nodes(), grid_nodes_of_full_build(&doc, page, 3));
    }

    fn set_reverse_z(doc: &mut Document, frame: NodeId, on: bool) {
        if let NodeKind::Frame(fp) = &mut doc.get_mut(frame).unwrap().kind {
            fp.auto_layout = on.then(|| designos_doc::AutoLayout { reverse_z: true, ..Default::default() });
        }
    }

    #[test]
    fn grid_nodes_follow_paint_order_in_reverse_z_frames_on_open_and_after_a_toggle() {
        let mut doc = Document::new("grid-reverse-z");
        let page = doc.add_page("page-1");
        let host = insert_frame(&mut doc, page, Rect::new(0.0, 0.0, 400.0, 400.0), false);
        let first = insert_frame(&mut doc, host, Rect::new(10.0, 10.0, 200.0, 200.0), true);
        let second = insert_frame(&mut doc, host, Rect::new(100.0, 10.0, 200.0, 200.0), true);
        set_reverse_z(&mut doc, host, true);

        // Opened with first-on-top: the first child paints last, so it is listed last.
        let mut idx = SceneIndex::new();
        idx.ensure(&doc, page, 1);
        assert_eq!(idx.grid_nodes(), &[second, first]);
        assert_eq!(idx.grid_nodes(), grid_nodes_of_full_build(&doc, page, 1));

        // Toggling reverse_z off on an open index restores document order.
        set_reverse_z(&mut doc, host, false);
        idx.ensure(&doc, page, 2);
        assert_eq!(idx.grid_nodes(), &[first, second]);
        assert_eq!(idx.grid_nodes(), grid_nodes_of_full_build(&doc, page, 2));

        // And toggling it on again re-sorts.
        set_reverse_z(&mut doc, host, true);
        idx.ensure(&doc, page, 3);
        assert_eq!(idx.grid_nodes(), &[second, first]);
        assert_eq!(idx.grid_nodes(), grid_nodes_of_full_build(&doc, page, 3));
    }
}
