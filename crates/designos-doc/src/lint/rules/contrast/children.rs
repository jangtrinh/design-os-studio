//! The children of one layer, indexed by where their subtrees paint, so that the scans for layers
//! behind and in front of a text visit only the siblings that can reach it instead of every
//! sibling of every ancestor for every text.
//!
//! The index never decides anything itself: it returns a superset of the siblings that can reach
//! a box, in document order, and the caller applies the same exact test it applied to every
//! sibling before.

use crate::lint::rect_tree::RectTree;
use crate::{NodeId, Rect};
use std::collections::HashMap;

/// The children of one layer, in document order, by the box their subtrees paint in.
pub struct ChildIndex {
    position: HashMap<NodeId, usize>,
    /// Document positions of the first and last child that masks its following siblings.
    first_mask: Option<usize>,
    last_mask: Option<usize>,
    tree: RectTree,
    /// Children whose geometry may change when their variable bindings are applied: always candidates
    /// for a test on the resolved layer.
    bound: Vec<usize>,
    /// Largest page-space scale of a child, and largest coordinate in any child box, for rounding slack.
    max_scale: f32,
    max_coord: f32,
    len: usize,
}

/// The parts a [`ChildIndex`] is made of.
pub(super) struct ChildIndexParts {
    pub position: HashMap<NodeId, usize>,
    pub first_mask: Option<usize>,
    pub last_mask: Option<usize>,
    pub tree: RectTree,
    pub bound: Vec<usize>,
    pub max_scale: f32,
    pub max_coord: f32,
    pub len: usize,
}

impl ChildIndex {
    pub(super) fn new(p: ChildIndexParts) -> Self {
        ChildIndex {
            position: p.position,
            first_mask: p.first_mask,
            last_mask: p.last_mask,
            tree: p.tree,
            bound: p.bound,
            max_scale: p.max_scale,
            max_coord: p.max_coord,
            len: p.len,
        }
    }

    /// Number of children.
    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Position of the first occurrence of `id` among the children.
    pub fn position_of(&self, id: NodeId) -> Option<usize> {
        self.position.get(&id).copied()
    }

    /// Whether a child at a position `<= at` masks its following siblings.
    pub fn masked_through(&self, at: usize) -> bool {
        self.first_mask.is_some_and(|m| m <= at)
    }

    /// Whether a child at a position `>= at` masks its following siblings.
    pub fn masked_from(&self, at: usize) -> bool {
        self.last_mask.is_some_and(|m| m >= at)
    }

    /// Positions (ascending) of the children whose subtree may reach `t`: exactly those for which
    /// `may_reach(bounds, 0.0, t)` holds.
    pub fn reaching(&self, t: &Rect, out: &mut Vec<usize>) {
        out.clear();
        self.tree.query(t, out);
        out.sort_unstable();
    }

    /// A superset of [`ChildIndex::reaching`] for a test on the resolved layer around a point box
    /// `t`: it also lists the children whose resolved geometry may differ from their stored one, and
    /// pads `t` by the slack of a test that tolerates a small error in the layer's own space.
    pub fn near_point(&self, t: &Rect, out: &mut Vec<usize>) {
        let reach = (t.x.abs() + t.y.abs() + self.max_coord) * 1.0e-5;
        let pad = 0.05 * self.max_scale + 0.01 + reach;
        let padded = Rect::new(t.x - pad, t.y - pad, t.w + 2.0 * pad, t.h + 2.0 * pad);
        self.reaching(&padded, out);
        out.extend(self.bound.iter().copied());
        out.sort_unstable();
        out.dedup();
    }
}
