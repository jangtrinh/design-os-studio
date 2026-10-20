//! Where each subtree paints, in page space, kept so that the scans for layers around a text can
//! pass over a sibling that cannot reach it instead of visiting everything inside it for every
//! text, and the index of those boxes per parent ([`ChildIndex`]) that answers "which siblings
//! reach this box" without looking at the others.

use super::children::{ChildIndex, ChildIndexParts};
use super::extent::{clipped, grown, painted_box};
use crate::lint::rect_tree::{RectEntry, RectTree, RectTreeBuild};
use crate::lint::PassCache;
use crate::{resolve_node, Affine, Document, NodeId, Rect};
use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

/// Subtrees nested deeper than this are not bounded: they count as reaching everywhere.
const MAX_DEPTH: usize = 128;

const UNBOUNDED: Rect = Rect::new(-1.0e9, -1.0e9, 2.0e9, 2.0e9);

struct Frame {
    id: NodeId,
    world: Affine,
    depth: usize,
    next_child: usize,
    inside: Option<Rect>,
}

/// Bounds per layer, computed on first use. Valid for the document they were computed from.
#[derive(Default)]
pub struct SubtreeBounds {
    cache: RefCell<HashMap<NodeId, Option<Rect>>>,
    stack: RefCell<Vec<Frame>>,
    /// The finished index of the children of each parent that has one.
    indexes: RefCell<HashMap<NodeId, Arc<ChildIndex>>>,
    /// Indexes interrupted by a time budget, to be resumed.
    pending: RefCell<HashMap<NodeId, Pending>>,
    /// What the measurement environment keeps for the whole pass (it is valid for the same document).
    pass: PassCache,
}

fn union(a: Option<Rect>, b: Option<Rect>) -> Option<Rect> {
    match (a, b) {
        (Some(a), Some(b)) => Some(a.union(&b)),
        (a, b) => a.or(b),
    }
}

/// The index over the children of one parent, while it is being built.
struct Pending {
    parent_world: Affine,
    next: usize,
    position: HashMap<NodeId, usize>,
    first_mask: Option<usize>,
    last_mask: Option<usize>,
    entries: Vec<RectEntry>,
    bound: Vec<usize>,
    max_scale: f32,
    max_coord: f32,
    tree: Option<RectTreeBuild>,
}

impl Pending {
    fn new(doc: &Document, parent: NodeId) -> Self {
        Pending {
            parent_world: doc.world_transform(parent),
            next: 0,
            position: HashMap::new(),
            first_mask: None,
            last_mask: None,
            entries: Vec::new(),
            bound: Vec::new(),
            max_scale: 0.0,
            max_coord: 0.0,
            tree: None,
        }
    }

    /// Takes in the child `id` at the next position, whose subtree paints in `rect`.
    fn add(&mut self, doc: &Document, id: NodeId, rect: Option<Rect>) {
        let pos = self.next;
        self.next += 1;
        self.position.entry(id).or_insert(pos);
        let Some(n) = doc.get(id) else { return };
        if n.is_mask {
            self.first_mask.get_or_insert(pos);
            self.last_mask = Some(pos);
        }
        if matches!(resolve_node(doc, n), Cow::Owned(_)) {
            self.bound.push(pos);
        }
        let Some(rect) = rect else { return };
        let world = self.parent_world * n.transform;
        self.max_scale = self.max_scale.max(world.a.hypot(world.b).max(world.c.hypot(world.d)));
        // Only the boxes the tree can bound count towards the rounding slack.
        let edges = [rect.x, rect.y, rect.x + rect.w, rect.y + rect.h];
        if edges.iter().all(|v| v.is_finite()) {
            self.max_coord = edges.iter().fold(self.max_coord, |m, v| m.max(v.abs()));
        }
        self.entries.push(RectEntry { pos, rect });
    }

    fn into_parts(self, len: usize) -> ChildIndexParts {
        ChildIndexParts {
            position: self.position,
            first_mask: self.first_mask,
            last_mask: self.last_mask,
            tree: self.tree.map(RectTreeBuild::finish).unwrap_or_else(|| RectTree::build(Vec::new())),
            bound: self.bound,
            max_scale: self.max_scale,
            max_coord: self.max_coord,
            len,
        }
    }
}

impl SubtreeBounds {
    /// Scratch space of the lint pass these bounds belong to.
    pub fn pass(&self) -> &PassCache {
        &self.pass
    }

    /// Warms what the scans for layers around the text `id` use: the bounds of every child of each
    /// of its ancestors, and the index over them. Returns `true` when all of it is ready and
    /// `false` if `deadline` passed first; the progress is kept for the next call.
    pub fn warm_for_text(&self, doc: &Document, id: NodeId, deadline: Option<Instant>) -> bool {
        let mut cur = id;
        while let Some(parent) = doc.get(cur).and_then(|n| n.parent) {
            if self.advance_index(doc, parent, deadline).is_none() {
                return false;
            }
            cur = parent;
        }
        true
    }

    /// The index over the children of `parent`, built now when it is not there yet.
    pub fn children_of(&self, doc: &Document, parent: NodeId) -> Arc<ChildIndex> {
        loop {
            if let Some(index) = self.advance_index(doc, parent, None) {
                return index;
            }
        }
    }

    /// Builds the index over the children of `parent` as far as `deadline` allows (never stopping
    /// before one step of progress). `None` when it is not finished.
    fn advance_index(&self, doc: &Document, parent: NodeId, deadline: Option<Instant>) -> Option<Arc<ChildIndex>> {
        if let Some(done) = self.indexes.borrow().get(&parent) {
            return Some(Arc::clone(done));
        }
        // A subtree whose walk was interrupted is finished first.
        if !self.drain_stack(doc, deadline) {
            return None;
        }
        let expired = || deadline.is_some_and(|d| Instant::now() >= d);
        let mut pend = self.pending.borrow_mut().remove(&parent).unwrap_or_else(|| Pending::new(doc, parent));
        let children: &[NodeId] = doc.get(parent).map_or(&[], |p| p.children.as_slice());
        if pend.tree.is_none() {
            let mut done = 0usize;
            while pend.next < children.len() {
                if done > 0 && done % 64 == 0 && expired() {
                    self.pending.borrow_mut().insert(parent, pend);
                    return None;
                }
                let child = children[pend.next];
                if !self.cache.borrow().contains_key(&child) {
                    self.push_root(doc, child, pend.parent_world);
                    if !self.drain_stack(doc, deadline) {
                        self.pending.borrow_mut().insert(parent, pend);
                        return None;
                    }
                }
                let rect = self.cache.borrow().get(&child).copied().flatten();
                pend.add(doc, child, rect);
                done += 1;
            }
            pend.tree = Some(RectTreeBuild::new(std::mem::take(&mut pend.entries)));
        }
        let Some(build) = pend.tree.as_mut() else { return None };
        while build.step() {
            if expired() {
                self.pending.borrow_mut().insert(parent, pend);
                return None;
            }
        }
        let index = Arc::new(ChildIndex::new(pend.into_parts(children.len())));
        self.indexes.borrow_mut().insert(parent, Arc::clone(&index));
        Some(index)
    }

    fn push_root(&self, doc: &Document, id: NodeId, parent_world: Affine) {
        let Some(n) = doc.get(id) else {
            self.cache.borrow_mut().insert(id, None);
            return;
        };
        if !n.visible || n.opacity <= 0.0 {
            self.cache.borrow_mut().insert(id, None);
            return;
        }
        let world = parent_world * n.transform;
        self.stack.borrow_mut().push(Frame { id, world, depth: 0, next_child: 0, inside: None });
    }

    fn drain_stack(&self, doc: &Document, deadline: Option<Instant>) -> bool {
        let mut stack = self.stack.borrow_mut();
        let mut cache = self.cache.borrow_mut();
        let mut steps = 0usize;
        while let Some(top) = stack.last_mut() {
            if steps > 0 && (steps & 63 == 0) && deadline.is_some_and(|d| Instant::now() >= d) {
                return false;
            }
            steps += 1;
            let children_len = doc.get(top.id).map_or(0, |n| n.children.len());
            if top.next_child < children_len {
                let child_id = doc.get(top.id).unwrap().children[top.next_child];
                top.next_child += 1;
                if let Some(&known) = cache.get(&child_id) {
                    top.inside = union(top.inside, known);
                } else if let Some(child) = doc.get(child_id) {
                    if !child.visible || child.opacity <= 0.0 {
                        cache.insert(child_id, None);
                    } else if top.depth + 1 >= MAX_DEPTH {
                        cache.insert(child_id, Some(UNBOUNDED));
                        top.inside = union(top.inside, Some(UNBOUNDED));
                    } else {
                        let world = top.world * child.transform;
                        let depth = top.depth + 1;
                        stack.push(Frame { id: child_id, world, depth, next_child: 0, inside: None });
                    }
                } else {
                    cache.insert(child_id, None);
                }
            } else {
                let top = stack.pop().unwrap();
                let bound = doc.get(top.id).map(|n| {
                    let painted = painted_box(&top.world, n, 0.0);
                    let mut inside = top.inside;
                    if n.kind.frame_props().is_some_and(|f| f.clips_content) {
                        inside = inside.and_then(|b| clipped(b, Some(painted.own)));
                    }
                    union(Some(painted.rect), inside.map(|b| grown(&b, painted.effects))).unwrap_or(painted.rect)
                });
                cache.insert(top.id, bound);
                if let Some(parent) = stack.last_mut() {
                    parent.inside = union(parent.inside, bound);
                }
            }
        }
        true
    }

    /// The box that everything painted by layer `id` and its descendants fits in, or `None` when
    /// the layer and everything in it is hidden. `world` is the transform of `id` to the page.
    pub fn of(&self, doc: &Document, id: NodeId, world: Affine) -> Option<Rect> {
        self.bounds(doc, id, world, 0)
    }

    fn bounds(&self, doc: &Document, id: NodeId, world: Affine, depth: usize) -> Option<Rect> {
        if let Some(known) = self.cache.borrow().get(&id) {
            return *known;
        }
        let found = self.compute(doc, id, world, depth);
        self.cache.borrow_mut().insert(id, found);
        found
    }

    fn compute(&self, doc: &Document, id: NodeId, world: Affine, depth: usize) -> Option<Rect> {
        let n = doc.get(id)?;
        if !n.visible || n.opacity <= 0.0 {
            return None;
        }
        if depth >= MAX_DEPTH {
            return Some(UNBOUNDED);
        }
        let painted = painted_box(&world, n, 0.0);
        let mut inside: Option<Rect> = None;
        for c in &n.children {
            let Some(child) = doc.get(*c) else { continue };
            inside = union(inside, self.bounds(doc, *c, world * child.transform, depth + 1));
        }
        if n.kind.frame_props().is_some_and(|f| f.clips_content) {
            inside = inside.and_then(|b| clipped(b, Some(painted.own)));
        }
        union(Some(painted.rect), inside.map(|b| grown(&b, painted.effects)))
    }
}

/// Whether the subtree of `id`, whose bounds are `bounds` and which sits under ancestors whose
/// effects add `inherited` page units of growth, can reach `text_box` at all.
pub fn may_reach(bounds: Option<Rect>, inherited: f32, text_box: &Rect) -> bool {
    bounds.is_some_and(|b| grown(&b, inherited).intersects(text_box))
}
