//! Where each subtree paints, in page space, kept so that the scan for layers behind a text can
//! pass over an earlier sibling that cannot reach it instead of visiting everything inside it for
//! every text. The bounds are a superset of what the exact scan would find: they only ever rule a
//! subtree out.

use super::extent::{clipped, grown, painted_box};
use designos_doc::{Affine, Document, NodeId, Rect};
use std::cell::RefCell;
use std::collections::HashMap;

/// Subtrees nested deeper than this are not bounded: they count as reaching everywhere.
const MAX_DEPTH: usize = 128;

const UNBOUNDED: Rect = Rect::new(-1.0e9, -1.0e9, 2.0e9, 2.0e9);

/// Bounds per layer, computed on first use. Valid for the document they were computed from.
#[derive(Default)]
pub(in crate::check_designs) struct SubtreeBounds {
    cache: RefCell<HashMap<NodeId, Option<Rect>>>,
}

fn union(a: Option<Rect>, b: Option<Rect>) -> Option<Rect> {
    match (a, b) {
        (Some(a), Some(b)) => Some(a.union(&b)),
        (a, b) => a.or(b),
    }
}

impl SubtreeBounds {
    /// The box that everything painted by layer `id` and its descendants fits in, or `None` when
    /// the layer and everything in it is hidden. `world` is the transform of `id` to the page.
    ///
    /// The box holds each layer's painted box (its stroke and effects included, whether or not it
    /// paints anything), and the effects of a layer grow the bounds of its content. A frame that
    /// clips cuts the bounds of its children to its own box. The growth that effects on ancestors
    /// of `id` add is not in the box: the caller grows it by that.
    pub(super) fn of(&self, doc: &Document, id: NodeId, world: Affine) -> Option<Rect> {
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
pub(super) fn may_reach(bounds: Option<Rect>, inherited: f32, text_box: &Rect) -> bool {
    bounds.is_some_and(|b| grown(&b, inherited).intersects(text_box))
}
