//! Where each subtree paints, in page space, kept so that the scan for layers behind a text can
//! pass over an earlier sibling that cannot reach it instead of visiting everything inside it for
//! every text.

use super::extent::{clipped, grown, painted_box};
use crate::{Affine, Document, NodeId, Rect};
use std::cell::RefCell;
use std::collections::HashMap;
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
}

fn union(a: Option<Rect>, b: Option<Rect>) -> Option<Rect> {
    match (a, b) {
        (Some(a), Some(b)) => Some(a.union(&b)),
        (a, b) => a.or(b),
    }
}

impl SubtreeBounds {
    /// Warms the cached bounds of every earlier sibling of `id` and of its ancestors up to
    /// `deadline`, returning `true` when all of them are cached and `false` if the slice budget
    /// expired mid-walk (preserving progress in `self.cache` and `self.stack` for the next slice).
    pub fn warm_for_text(&self, doc: &Document, id: NodeId, deadline: Instant) -> bool {
        if !self.drain_stack(doc, deadline) {
            return false;
        }
        let mut cur = id;
        while let Some(parent) = doc.get(cur).and_then(|n| n.parent) {
            let Some(p) = doc.get(parent) else { break };
            let parent_world = doc.world_transform(parent);
            let before = p.children.iter().position(|s| *s == cur).unwrap_or(p.children.len());
            for &sibling in p.children[..before].iter().rev() {
                if !self.cache.borrow().contains_key(&sibling) {
                    self.push_root(doc, sibling, parent_world);
                    if !self.drain_stack(doc, deadline) {
                        return false;
                    }
                }
            }
            cur = parent;
        }
        true
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

    fn drain_stack(&self, doc: &Document, deadline: Instant) -> bool {
        let mut stack = self.stack.borrow_mut();
        let mut cache = self.cache.borrow_mut();
        let mut steps = 0usize;
        while let Some(top) = stack.last_mut() {
            if steps > 0 && (steps & 63 == 0) && Instant::now() >= deadline {
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
