//! The layout fields of an imported instance that `crate::component_imported_layout` hashes, compares and writes back.

use crate::branch_diff::StableHasher;
use crate::{Affine, Document, FillRule, Node, NodeId, NodeKind, Path, PathCmd, StretchedAxes, StrokeGeometryStamp, Vec2};

/// Shape layers whose baked outline is drawn for the shape parameters their `kind` holds.
fn is_parametric_shape(kind: &NodeKind) -> bool {
    matches!(kind, NodeKind::Rectangle | NodeKind::Ellipse(_) | NodeKind::Star { .. } | NodeKind::Polygon { .. } | NodeKind::Line | NodeKind::BooleanOp(_))
}

fn add_vec2(h: &mut StableHasher, v: Vec2) {
    h.float(v.x);
    h.float(v.y);
}

fn add_affine(h: &mut StableHasher, t: &Affine) {
    for v in [t.a, t.b, t.c, t.d, t.tx, t.ty] {
        h.float(v);
    }
}

fn add_pair(h: &mut StableHasher, pair: Option<(f32, f32)>) {
    match pair {
        None => h.word(0),
        Some((a, b)) => {
            h.word(1);
            h.float(a);
            h.float(b);
        }
    }
}

fn add_stretched(h: &mut StableHasher, s: &StretchedAxes) {
    add_pair(h, s.w);
    add_pair(h, s.h);
    h.word(match s.last_parent_horiz {
        None => 0,
        Some(false) => 1,
        Some(true) => 2,
    });
}

/// Hashes an outline list directly (the serde path mixes a field or variant name into every point).
fn add_paths(h: &mut StableHasher, paths: &Option<Vec<Path>>) {
    let Some(paths) = paths else {
        h.word(0);
        return;
    };
    h.word(1 + paths.len() as u64);
    for p in paths {
        h.word(match p.fill_rule {
            FillRule::NonZero => 0,
            FillRule::EvenOdd => 1,
        });
        h.word(p.cmds.len() as u64);
        for cmd in &p.cmds {
            match *cmd {
                PathCmd::MoveTo(a) => {
                    h.word(1);
                    add_vec2(h, a);
                }
                PathCmd::LineTo(a) => {
                    h.word(2);
                    add_vec2(h, a);
                }
                PathCmd::QuadTo(a, b) => {
                    h.word(3);
                    add_vec2(h, a);
                    add_vec2(h, b);
                }
                PathCmd::CubicTo(a, b, c) => {
                    h.word(4);
                    add_vec2(h, a);
                    add_vec2(h, b);
                    add_vec2(h, c);
                }
                PathCmd::Close => h.word(5),
            }
        }
    }
}

/// Hashes what decides whether the stored geometry of one layer is still right.
///
/// The layout fields (the ones [`LayoutFields`] puts back): size, position (not on the instance root, whose position is
/// its own and never synced), the auto-layout stretch record, the baked fill and stroke outlines, and a vector's paths.
/// Then what those fields mean: corner radii and a shape's parameters, for which a baked outline was drawn. The layer's
/// identity is the main layer it mirrors and its child count, so a copy of an instance hashes like the original.
pub(crate) fn hash_layer(h: &mut StableHasher, n: &Node, is_root: bool) {
    match n.component.source {
        None => h.word(0),
        Some(s) => {
            h.word(1);
            h.word(s.0);
        }
    }
    h.word(n.children.len() as u64);
    add_vec2(h, n.size);
    if !is_root {
        add_affine(h, &n.transform);
    }
    add_stretched(h, &n.layout_child.stretched);
    add_paths(h, &n.fill_geometry);
    add_paths(h, &n.stroke_geometry);
    h.add(&n.stroke_geometry_for);
    if matches!(n.kind, NodeKind::Vector { .. }) {
        h.add(&n.kind);
    }
    h.word(meaning_hash(n));
}

/// What the baked outlines of `n` were drawn for, apart from the layout fields themselves: corner radii and the
/// parameters of a shape. (A vector's paths are a layout field: Figma stores them resized with the instance.)
pub(crate) fn meaning_hash(n: &Node) -> u64 {
    let mut h = StableHasher::new();
    for r in n.corner_radii {
        h.float(r);
    }
    h.float(n.corner_smoothing);
    if is_parametric_shape(&n.kind) {
        h.add(&n.kind);
    }
    h.finish()
}

/// Visits the layers of the subtree under `root`, parents before children, in child order, looking each one up once.
/// `f` gets the layer's position in that order and returns whether to go on. `Some(true)` when every layer was
/// visited, `Some(false)` when `f` stopped the walk, `None` when a listed child is missing.
pub(crate) fn walk(doc: &Document, root: NodeId, mut f: impl FnMut(usize, &Node) -> bool) -> Option<bool> {
    let mut stack = vec![root];
    let mut k = 0;
    while let Some(id) = stack.pop() {
        let n = doc.get(id)?;
        if !f(k, n) {
            return Some(false);
        }
        k += 1;
        stack.extend(n.children.iter().rev().copied());
    }
    Some(true)
}

/// Hash of the layout of the subtree under `root`; `None` when a listed child is missing.
pub(crate) fn layout_hash(doc: &Document, root: NodeId) -> Option<u64> {
    let mut h = StableHasher::new();
    walk(doc, root, |_, n| {
        hash_layer(&mut h, n, n.id == root);
        true
    })?;
    Some(h.finish())
}

/// The layout fields of every layer under `root` in [`walk`] order, when their [`layout_hash`] is `stored`.
pub(crate) fn noted_if_stored(doc: &Document, root: NodeId, stored: u64) -> Option<Vec<LayoutFields>> {
    let mut h = StableHasher::new();
    let mut fields = Vec::new();
    walk(doc, root, |_, n| {
        hash_layer(&mut h, n, n.id == root);
        fields.push(LayoutFields::of(n));
        true
    })?;
    (h.finish() == stored).then_some(fields)
}

/// Whether the layers under `root` are the ones in `before`, in [`walk`] order, and `same` holds for each pair.
pub(crate) fn same_layers(doc: &Document, root: NodeId, before: &[LayoutFields], mut same: impl FnMut(&LayoutFields, &Node) -> bool) -> bool {
    let mut count = 0;
    let all = walk(doc, root, |k, n| {
        count = k + 1;
        before.get(k).is_some_and(|b| b.id == n.id && same(b, n))
    });
    all == Some(true) && count == before.len()
}

/// The layout fields of one layer, as written back when its stored geometry is kept.
pub(crate) struct LayoutFields {
    pub(crate) id: NodeId,
    size: Vec2,
    transform: Affine,
    stretched: StretchedAxes,
    fill_geometry: Option<Vec<Path>>,
    stroke_geometry: Option<Vec<Path>>,
    stroke_geometry_for: Option<StrokeGeometryStamp>,
    vector: Option<NodeKind>,
    /// [`meaning_hash`] of the layer when it carries a baked fill or stroke outline: an outline put back must be the
    /// one drawn for the shape parameters the layer has after the sync. `None` for a layer without one.
    pub(crate) outline_meaning: Option<u64>,
}

impl LayoutFields {
    pub(crate) fn of(n: &Node) -> Self {
        Self {
            id: n.id,
            size: n.size,
            transform: n.transform,
            stretched: n.layout_child.stretched,
            fill_geometry: n.fill_geometry.clone(),
            stroke_geometry: n.stroke_geometry.clone(),
            stroke_geometry_for: n.stroke_geometry_for.clone(),
            vector: matches!(n.kind, NodeKind::Vector { .. }).then(|| n.kind.clone()),
            outline_meaning: (n.fill_geometry.is_some() || n.stroke_geometry.is_some()).then(|| meaning_hash(n)),
        }
    }

    /// Whether layer `n` holds exactly these layout fields (so writing them back would change nothing).
    pub(crate) fn held_by(&self, n: &Node, is_root: bool) -> bool {
        n.id == self.id
            && n.size == self.size
            && (is_root || n.transform == self.transform)
            && n.layout_child.stretched == self.stretched
            && n.fill_geometry == self.fill_geometry
            && n.stroke_geometry == self.stroke_geometry
            && n.stroke_geometry_for == self.stroke_geometry_for
            && self.vector.as_ref().is_none_or(|v| *v == n.kind)
    }

    pub(crate) fn write_to(&self, n: &mut Node, is_root: bool) {
        n.size = self.size;
        if !is_root {
            n.transform = self.transform;
        }
        n.layout_child.stretched = self.stretched;
        n.fill_geometry.clone_from(&self.fill_geometry);
        n.stroke_geometry.clone_from(&self.stroke_geometry);
        n.stroke_geometry_for.clone_from(&self.stroke_geometry_for);
        if let Some(kind) = &self.vector {
            n.kind = kind.clone();
        }
    }
}
