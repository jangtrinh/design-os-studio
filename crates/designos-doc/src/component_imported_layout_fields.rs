//! The layout fields of an imported instance that `crate::component_imported_layout` hashes, compares and writes back.

use crate::branch_diff::StableHasher;
use crate::{Affine, Document, FillRule, Node, NodeId, NodeKind, Path, PathCmd, StretchedAxes, StrokeGeometryStamp, Vec2, VectorNetwork};

/// Shape layers whose baked outline is drawn for the shape parameters their `kind` holds.
fn is_parametric_shape(kind: &NodeKind) -> bool {
    matches!(kind, NodeKind::Rectangle | NodeKind::Ellipse(_) | NodeKind::Star { .. } | NodeKind::Polygon { .. } | NodeKind::Line | NodeKind::BooleanOp(_))
}

/// Both coordinates in one word.
fn add_vec2(h: &mut StableHasher, v: Vec2) {
    h.word(u64::from(v.x.to_bits()) << 32 | u64::from(v.y.to_bits()));
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

/// Hashes an optional outline list, see [`add_path_list`].
fn add_paths(h: &mut StableHasher, paths: &Option<Vec<Path>>) {
    match paths {
        None => h.word(0),
        Some(paths) => add_path_list(h, paths),
    }
}

/// Hashes a vector's paths and its network directly.
fn add_vector(h: &mut StableHasher, paths: &[Path], network: &Option<VectorNetwork>) {
    add_path_list(h, paths);
    let Some(net) = network else {
        h.word(0);
        return;
    };
    h.word(1 + net.vertices.len() as u64);
    for v in &net.vertices {
        add_vec2(h, v.pos);
        h.float(v.corner_radius);
        h.add(&(&v.cap, &v.join, &v.mirror));
    }
    h.word(net.segments.len() as u64);
    for s in &net.segments {
        h.word(s.a as u64);
        h.word(s.b as u64);
        add_vec2(h, s.ta);
        add_vec2(h, s.tb);
    }
    h.word(net.regions.len() as u64);
    for r in &net.regions {
        if !r.filled {
            h.word(2);
        }
        h.word(match r.rule {
            FillRule::NonZero => 0,
            FillRule::EvenOdd => 1,
        });
        h.word(r.loops.len() as u64);
        for l in &r.loops {
            h.word(l.len() as u64);
            for &i in l {
                h.word(i as u64);
            }
        }
    }
}

/// Four mixing chains for a long run of words (an outline's points), folded into the hasher at the end. One chain waits
/// on a multiply per word; four independent ones keep the CPU busy.
struct Lanes {
    state: [u64; 4],
    count: u64,
}

impl Lanes {
    const SEED: u64 = 0x9e37_79b9_7f4a_7c15;

    fn new() -> Self {
        Self { state: [0x243f_6a88_85a3_08d3, 0x1319_8a2e_0370_7344, 0xa409_3822_299f_31d0, 0x082e_fa98_ec4e_6c89], count: 0 }
    }

    fn push(&mut self, word: u64) {
        let lane = &mut self.state[(self.count & 3) as usize];
        *lane = (lane.rotate_left(5) ^ word).wrapping_mul(Self::SEED);
        self.count += 1;
    }

    fn point(&mut self, v: Vec2) {
        self.push(u64::from(v.x.to_bits()) << 32 | u64::from(v.y.to_bits()));
    }

    fn fold_into(self, h: &mut StableHasher) {
        h.word(self.count);
        for s in self.state {
            h.word(s);
        }
    }
}

/// Hashes an outline list directly (the serde path mixes a field or variant name into every point).
fn add_path_list(h: &mut StableHasher, paths: &[Path]) {
    let mut l = Lanes::new();
    l.push(1 + paths.len() as u64);
    for p in paths {
        l.push(match p.fill_rule {
            FillRule::NonZero => 0,
            FillRule::EvenOdd => 1,
        });
        l.push(p.cmds.len() as u64);
        for cmd in &p.cmds {
            match *cmd {
                PathCmd::MoveTo(a) => {
                    l.push(1);
                    l.point(a);
                }
                PathCmd::LineTo(a) => {
                    l.push(2);
                    l.point(a);
                }
                PathCmd::QuadTo(a, b) => {
                    l.push(3);
                    l.point(a);
                    l.point(b);
                }
                PathCmd::CubicTo(a, b, c) => {
                    l.push(4);
                    l.point(a);
                    l.point(b);
                    l.point(c);
                }
                PathCmd::Close => l.push(5),
            }
        }
    }
    l.fold_into(h);
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
    if let NodeKind::Vector { paths, network } = &n.kind {
        add_vector(h, paths, network);
    }
    for r in n.corner_radii {
        h.float(r);
    }
    h.float(n.corner_smoothing);
    if is_parametric_shape(&n.kind) {
        h.add(&n.kind);
    }
}

/// What the baked outlines of a layer were drawn for, apart from the layout fields themselves: corner radii and the
/// parameters of a shape. (A vector's paths are a layout field: Figma stores them resized with the instance.)
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Meaning {
    radii: [f32; 4],
    smoothing: f32,
    shape: Option<NodeKind>,
}

impl Meaning {
    fn of(n: &Node) -> Self {
        Self { radii: n.corner_radii, smoothing: n.corner_smoothing, shape: is_parametric_shape(&n.kind).then(|| n.kind.clone()) }
    }

    /// Whether layer `n` still has the corner radii and shape parameters this outline was drawn for.
    pub(crate) fn held_by(&self, n: &Node) -> bool {
        n.corner_radii == self.radii
            && n.corner_smoothing == self.smoothing
            && self.shape.as_ref().is_none_or(|k| *k == n.kind)
            && (self.shape.is_some() || !is_parametric_shape(&n.kind))
    }
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
    /// What the layer's baked fill or stroke outline was drawn for: an outline put back must be the one drawn for the
    /// shape parameters the layer has after the sync. `None` for a layer without one.
    pub(crate) outline_meaning: Option<Meaning>,
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
            outline_meaning: (n.fill_geometry.is_some() || n.stroke_geometry.is_some()).then(|| Meaning::of(n)),
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
