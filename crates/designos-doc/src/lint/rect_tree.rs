//! A static bounding-volume tree over boxes, for "which of these boxes meet this one" without
//! looking at the others.
//!
//! It never decides anything itself: [`RectTree::query`] returns exactly the entries for which
//! `Rect::intersects` holds, computed with the same four comparisons, so a caller that used to test
//! every box gets the same answer from the boxes the tree hands back.

use crate::Rect;

/// Entries in a leaf of the tree.
const LEAF: usize = 8;

/// One box and the caller's position for it.
#[derive(Clone, Copy)]
pub struct RectEntry {
    pub pos: usize,
    pub rect: Rect,
}

#[derive(Clone, Copy)]
struct Edges {
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
}

impl Edges {
    const NONE: Edges = Edges { x0: f32::MAX, y0: f32::MAX, x1: f32::MIN, y1: f32::MIN };

    /// The edges as `Rect::intersects` reads them: the right edge is `x + w` in `f32`.
    fn of(r: &Rect) -> Edges {
        Edges { x0: r.x, y0: r.y, x1: r.x + r.w, y1: r.y + r.h }
    }

    fn include(&mut self, o: &Edges) {
        self.x0 = self.x0.min(o.x0);
        self.y0 = self.y0.min(o.y0);
        self.x1 = self.x1.max(o.x1);
        self.y1 = self.y1.max(o.y1);
    }

    /// Whether some rect inside these edges can intersect `t` (`tx1`, `ty1` are its right and bottom edges).
    /// Mirrors the four comparisons of `Rect::intersects`, so it is true whenever an entry intersects.
    fn may_meet(&self, t: &Rect, tx1: f32, ty1: f32) -> bool {
        self.x0 < tx1 && t.x < self.x1 && self.y0 < ty1 && t.y < self.y1
    }
}

enum Kind {
    Leaf { lo: usize, hi: usize },
    Split { left: usize, right: usize },
}

struct TreeNode {
    edges: Edges,
    kind: Kind,
}

/// Boxes with a finite edge in a bounding-volume tree, split at the median of the longer side.
pub struct RectTree {
    entries: Vec<RectEntry>,
    nodes: Vec<TreeNode>,
    /// Boxes with an edge that is not finite: tested one by one.
    loose: Vec<RectEntry>,
}

/// A tree being built, a node range at a time so that a time budget can interrupt it.
pub struct RectTreeBuild {
    tree: RectTree,
    todo: Vec<(usize, usize, usize)>,
}

fn finite(r: &Rect) -> bool {
    let e = Edges::of(r);
    r.x.is_finite() && r.y.is_finite() && e.x1.is_finite() && e.y1.is_finite()
}

impl RectTreeBuild {
    pub fn new(all: Vec<RectEntry>) -> Self {
        let (entries, loose): (Vec<RectEntry>, Vec<RectEntry>) = all.into_iter().partition(|e| finite(&e.rect));
        let n = entries.len();
        let mut tree = RectTree { entries, nodes: Vec::new(), loose };
        let mut todo = Vec::new();
        if n > 0 {
            tree.nodes.push(TreeNode { edges: Edges::NONE, kind: Kind::Leaf { lo: 0, hi: 0 } });
            todo.push((0, n, 0));
        }
        RectTreeBuild { tree, todo }
    }

    /// Splits one pending range. `false` once the tree is complete.
    pub fn step(&mut self) -> bool {
        let Some((lo, hi, slot)) = self.todo.pop() else { return false };
        let tree = &mut self.tree;
        let mut edges = Edges::NONE;
        for e in &tree.entries[lo..hi] {
            edges.include(&Edges::of(&e.rect));
        }
        if hi - lo <= LEAF {
            tree.nodes[slot] = TreeNode { edges, kind: Kind::Leaf { lo, hi } };
            return true;
        }
        let mid = lo + (hi - lo) / 2;
        let wide = edges.x1 - edges.x0 >= edges.y1 - edges.y0;
        let centre = |e: &RectEntry| if wide { e.rect.x + e.rect.w * 0.5 } else { e.rect.y + e.rect.h * 0.5 };
        tree.entries[lo..hi].select_nth_unstable_by(mid - lo, |a, b| centre(a).total_cmp(&centre(b)));
        let left = tree.nodes.len();
        tree.nodes.push(TreeNode { edges: Edges::NONE, kind: Kind::Leaf { lo: 0, hi: 0 } });
        let right = tree.nodes.len();
        tree.nodes.push(TreeNode { edges: Edges::NONE, kind: Kind::Leaf { lo: 0, hi: 0 } });
        tree.nodes[slot] = TreeNode { edges, kind: Kind::Split { left, right } };
        self.todo.push((mid, hi, right));
        self.todo.push((lo, mid, left));
        true
    }

    pub fn finish(self) -> RectTree {
        self.tree
    }
}

impl RectTree {
    /// A tree over `entries`, built in one go.
    pub fn build(entries: Vec<RectEntry>) -> Self {
        let mut build = RectTreeBuild::new(entries);
        while build.step() {}
        build.finish()
    }

    /// Appends the positions (in no particular order) of the entries whose box intersects `t`.
    pub fn query(&self, t: &Rect, out: &mut Vec<usize>) {
        let hit = |e: &&RectEntry| e.rect.intersects(t);
        out.extend(self.loose.iter().filter(hit).map(|e| e.pos));
        if self.nodes.is_empty() {
            return;
        }
        let (tx1, ty1) = (t.x + t.w, t.y + t.h);
        let mut stack = vec![0usize];
        while let Some(at) = stack.pop() {
            let node = &self.nodes[at];
            if !node.edges.may_meet(t, tx1, ty1) {
                continue;
            }
            match node.kind {
                Kind::Leaf { lo, hi } => out.extend(self.entries[lo..hi].iter().filter(hit).map(|e| e.pos)),
                Kind::Split { left, right } => {
                    stack.push(right);
                    stack.push(left);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A small deterministic generator, so a failing seed can be replayed.
    struct Lcg(u64);

    impl Lcg {
        fn next(&mut self) -> f32 {
            self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            ((self.0 >> 33) % 2000) as f32 / 10.0 - 50.0
        }
    }

    #[test]
    fn query_equals_testing_every_box() {
        for seed in 0..40u64 {
            let mut rng = Lcg(seed);
            let mut entries: Vec<RectEntry> = (0..(seed as usize * 13 % 300))
                .map(|pos| RectEntry { pos, rect: Rect::new(rng.next(), rng.next(), rng.next().abs() / 4.0, rng.next().abs() / 4.0) })
                .collect();
            // Boxes the tree cannot bound, and ones that only touch the query box.
            entries.push(RectEntry { pos: 1000, rect: Rect::new(f32::NEG_INFINITY, 0.0, f32::INFINITY, 10.0) });
            entries.push(RectEntry { pos: 1001, rect: Rect::new(f32::NAN, 0.0, 5.0, 5.0) });
            entries.push(RectEntry { pos: 1002, rect: Rect::new(-1.0e9, -1.0e9, 2.0e9, 2.0e9) });
            entries.push(RectEntry { pos: 1003, rect: Rect::new(10.0, 10.0, 5.0, 5.0) });
            let tree = RectTree::build(entries.clone());
            for _ in 0..40 {
                let q = Rect::new(rng.next(), rng.next(), rng.next().abs() / 3.0, rng.next().abs() / 3.0);
                let mut got = Vec::new();
                tree.query(&q, &mut got);
                got.sort_unstable();
                let mut want: Vec<usize> = entries.iter().filter(|e| e.rect.intersects(&q)).map(|e| e.pos).collect();
                want.sort_unstable();
                assert_eq!(got, want, "seed {seed} query {q:?}");
            }
            // A box that only touches an entry does not intersect it.
            let mut got = Vec::new();
            tree.query(&Rect::new(15.0, 10.0, 5.0, 5.0), &mut got);
            assert!(!got.contains(&1003));
        }
    }

    #[test]
    fn empty_tree_answers_nothing() {
        let mut got = Vec::new();
        RectTree::build(Vec::new()).query(&Rect::new(0.0, 0.0, 10.0, 10.0), &mut got);
        assert!(got.is_empty());
    }
}
