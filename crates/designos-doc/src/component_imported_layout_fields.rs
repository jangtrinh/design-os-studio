//! The layout fields of an imported instance that `crate::component_imported_layout` hashes, compares and writes back.

use crate::branch_diff::StableHasher;
use crate::{Affine, Document, Node, NodeId, NodeKind, Path, StretchedAxes, StrokeGeometryStamp, Vec2};

/// Shape layers whose baked outline is drawn for the shape parameters their `kind` holds.
fn is_parametric_shape(kind: &NodeKind) -> bool {
    matches!(kind, NodeKind::Rectangle | NodeKind::Ellipse(_) | NodeKind::Star { .. } | NodeKind::Polygon { .. } | NodeKind::Line | NodeKind::BooleanOp(_))
}

/// Hashes what decides whether the stored geometry of one layer is still right.
///
/// The layout fields (the ones [`LayoutFields`] puts back): size, position (not on the instance root, whose position is
/// its own and never synced), the auto-layout stretch record, the baked fill and stroke outlines, and a vector's paths.
/// Then what those fields mean: corner radii and a shape's parameters, for which a baked outline was drawn. The layer's
/// identity is the main layer it mirrors and its child count, so a copy of an instance hashes like the original.
fn hash_layer(h: &mut StableHasher, n: &Node, is_root: bool) {
    h.add(&n.component.source);
    h.add(&(n.children.len() as u64));
    h.add(&n.size);
    if !is_root {
        h.add(&n.transform);
    }
    h.add(&n.layout_child.stretched);
    h.add(&n.fill_geometry);
    h.add(&n.stroke_geometry);
    h.add(&n.stroke_geometry_for);
    if matches!(n.kind, NodeKind::Vector { .. }) {
        h.add(&n.kind);
    }
    h.add(&meaning_hash(n));
}

/// What the baked outlines of `n` were drawn for, apart from the layout fields themselves: corner radii and the
/// parameters of a shape. (A vector's paths are a layout field: Figma stores them resized with the instance.)
pub(crate) fn meaning_hash(n: &Node) -> u64 {
    let mut h = StableHasher::new();
    h.add(&n.corner_radii);
    h.add(&n.corner_smoothing);
    if is_parametric_shape(&n.kind) {
        h.add(&n.kind);
    }
    h.finish()
}

/// The layers of the subtree under `root`, parents before children, in child order.
pub(crate) fn subtree(doc: &Document, root: NodeId) -> Option<Vec<NodeId>> {
    let mut out = Vec::new();
    let mut stack = vec![root];
    while let Some(id) = stack.pop() {
        let n = doc.get(id)?;
        out.push(id);
        stack.extend(n.children.iter().rev().copied());
    }
    Some(out)
}

/// Hash of the layout of the subtree under `root`; `None` when a listed child is missing.
pub(crate) fn layout_hash(doc: &Document, root: NodeId) -> Option<u64> {
    let mut h = StableHasher::new();
    for id in subtree(doc, root)? {
        hash_layer(&mut h, doc.get(id)?, id == root);
    }
    Some(h.finish())
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
