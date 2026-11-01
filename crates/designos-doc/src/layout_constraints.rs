//! Shared Figma constraint solver for container resize and instance expansion (#691).
//!
//! Implements the measured Figma constraint rules for `Min`, `Max`, `Center`,
//! `Stretch`, and `Scale` across both unrotated and rotated/skewed children,
//! including half-away-from-zero extent rounding and sub-1px sizes.

use crate::{Affine, Constraint, Constraints, Document, Node, NodeId, NodeKind, Path, PathCmd, Vec2};

/// Minimum axis extent (`2^-12`) below which per-axis scale falls back to `1.0`.
const MIN_EXTENT: f64 = 1.0 / 4096.0;

/// Rounds `x` to the nearest integer, ties rounding half away from zero.
#[inline]
pub fn round_half_away_from_zero(x: f64) -> f64 {
    x.round()
}

#[inline]
fn move_axis_edges(constraint: Constraint, e0: f64, e1: f64, old_p: f64, new_p: f64) -> (f64, f64) {
    match constraint {
        Constraint::Min => (e0, e1),
        Constraint::Max => {
            let delta = round_half_away_from_zero(new_p) - round_half_away_from_zero(old_p);
            (e0 + delta, e1 + delta)
        }
        Constraint::Center => {
            let delta = round_half_away_from_zero(new_p * 0.5) - round_half_away_from_zero(old_p * 0.5);
            (e0 + delta, e1 + delta)
        }
        Constraint::Stretch => {
            let delta = round_half_away_from_zero(new_p) - round_half_away_from_zero(old_p);
            (e0, (e1 + delta).max(e0))
        }
        Constraint::Scale => {
            if old_p.abs() < MIN_EXTENT {
                (e0, e1)
            } else {
                let r = new_p / old_p;
                (e0 * r, e1 * r)
            }
        }
    }
}

/// Computes the new `(transform, size)` of `child` when its parent container
/// resizes from `old` to `new`.
///
/// Returns `None` when the resulting matrix determinant is `0` or non-finite,
/// in which case the caller leaves `child` unchanged.
pub fn constrain_child(child: &Node, old: Vec2, new: Vec2) -> Option<(Affine, Vec2)> {
    constrain_with(child, child.constraints, old, new)
}

/// [`constrain_child`] under `constraints` instead of the ones `child` carries.
fn constrain_with(child: &Node, constraints: Constraints, old: Vec2, new: Vec2) -> Option<(Affine, Vec2)> {
    let w = child.size.x as f64;
    let h = child.size.y as f64;
    let a = child.transform.a as f64;
    let b = child.transform.b as f64;
    let c = child.transform.c as f64;
    let d = child.transform.d as f64;
    let tx = child.transform.tx as f64;
    let ty = child.transform.ty as f64;

    // 1. Axis-aligned bounds B of child's local (0, 0, w, h) in parent's axes.
    let aw = a * w;
    let ch = c * h;
    let bw = b * w;
    let dh = d * h;

    let bx0 = tx + aw.min(0.0) + ch.min(0.0);
    let bx1 = tx + aw.max(0.0) + ch.max(0.0);
    let by0 = ty + bw.min(0.0) + dh.min(0.0);
    let by1 = ty + bw.max(0.0) + dh.max(0.0);

    // 2. Move B's min and max edges per axis according to the child's constraints.
    let (nbx0, nbx1) = move_axis_edges(constraints.horizontal, bx0, bx1, old.x as f64, new.x as f64);
    let (nby0, nby1) = move_axis_edges(constraints.vertical, by0, by1, old.y as f64, new.y as f64);

    // 3. Per-axis scale s = new_extent / old_extent (1.0 when old_extent < 2^-12).
    let old_ext_x = bx1 - bx0;
    let new_ext_x = nbx1 - nbx0;
    let sx = if old_ext_x < MIN_EXTENT { 1.0 } else { new_ext_x / old_ext_x };

    let old_ext_y = by1 - by0;
    let new_ext_y = nby1 - nby0;
    let sy = if old_ext_y < MIN_EXTENT { 1.0 } else { new_ext_y / old_ext_y };

    let stretch_collapsed_x = matches!(constraints.horizontal, Constraint::Stretch) && old_ext_x >= MIN_EXTENT && new_ext_x <= 0.0;
    let stretch_collapsed_y = matches!(constraints.vertical, Constraint::Stretch) && old_ext_y >= MIN_EXTENT && new_ext_y <= 0.0;

    // 4. Apply map p -> B'.min + s * (p - B.min) after child's matrix.
    let ma = sx * a;
    let mc = sx * c;
    let mtx = nbx0 + sx * (tx - bx0);
    let mb = sy * b;
    let md = sy * d;
    let mty = nby0 + sy * (ty - by0);
    if !mtx.is_finite() || !mty.is_finite() {
        return None;
    }

    if stretch_collapsed_x || stretch_collapsed_y {
        // Decompose the rank-deficient map column by column so the transformed corners have
        // zero extent on the collapsed parent axis while keeping the original column signs.
        let col0_len = ma.hypot(mb);
        let (size_kx, na, nb) = if col0_len.is_finite() && col0_len > 1e-18 {
            (col0_len, ma / col0_len, mb / col0_len)
        } else {
            let orig0 = a.hypot(b);
            if orig0.is_finite() && orig0 > 1e-18 {
                (0.0, a / orig0, b / orig0)
            } else {
                (0.0, 1.0, 0.0)
            }
        };
        let col1_len = mc.hypot(md);
        let (size_ky, nc, nd) = if col1_len.is_finite() && col1_len > 1e-18 {
            (col1_len, mc / col1_len, md / col1_len)
        } else {
            let orig1 = c.hypot(d);
            if orig1.is_finite() && orig1 > 1e-18 {
                (0.0, c / orig1, d / orig1)
            } else {
                (0.0, 0.0, 1.0)
            }
        };
        let new_size = Vec2::new((w * size_kx) as f32, (h * size_ky) as f32);
        let new_transform = Affine { a: na as f32, b: nb as f32, c: nc as f32, d: nd as f32, tx: mtx as f32, ty: mty as f32 };
        return Some((new_transform, new_size));
    }

    let det = ma * md - mb * mc;
    if !det.is_finite() || det.abs() <= 1e-18 {
        return None;
    }

    // 5. Split M' into size and transform (keeping skew in the transform).
    let kx = ma.hypot(mb);
    if !kx.is_finite() || kx <= 1e-18 {
        return None;
    }
    let ky = det.abs() / kx;
    if !ky.is_finite() || ky <= 1e-18 {
        return None;
    }

    let new_size = Vec2::new((w * kx) as f32, (h * ky) as f32);
    let new_transform = Affine { a: (ma / kx) as f32, b: (mb / kx) as f32, c: (mc / ky) as f32, d: (md / ky) as f32, tx: mtx as f32, ty: mty as f32 };
    Some((new_transform, new_size))
}

#[inline]
fn scale_vec2(p: Vec2, kx: f32, ky: f32) -> Vec2 {
    Vec2::new(p.x * kx, p.y * ky)
}

fn scale_paths(paths: &mut [Path], kx: f32, ky: f32) {
    for path in paths {
        for cmd in &mut path.cmds {
            match cmd {
                PathCmd::MoveTo(p) | PathCmd::LineTo(p) => *p = scale_vec2(*p, kx, ky),
                PathCmd::QuadTo(c, p) => {
                    *c = scale_vec2(*c, kx, ky);
                    *p = scale_vec2(*p, kx, ky);
                }
                PathCmd::CubicTo(c1, c2, p) => {
                    *c1 = scale_vec2(*c1, kx, ky);
                    *c2 = scale_vec2(*c2, kx, ky);
                    *p = scale_vec2(*p, kx, ky);
                }
                PathCmd::Close => {}
            }
        }
    }
}

/// Applies `(new_transform, new_size)` from [`constrain_child`] / [`plan_constraint_pass`]
/// to `node`, scaling local vector/boolean path geometry or clearing stale analytical outlines
/// whenever `node.size` changes.
pub fn apply_constrained_geometry(node: &mut Node, new_transform: Affine, new_size: Vec2) {
    let old_size = node.size;
    let size_changed = (new_size.x - old_size.x).abs() > 1e-4 || (new_size.y - old_size.y).abs() > 1e-4;
    node.transform = new_transform;
    node.size = new_size;
    if !size_changed {
        return;
    }
    let kx = if old_size.x.abs() > 1e-6 { new_size.x / old_size.x } else { 1.0 };
    let ky = if old_size.y.abs() > 1e-6 { new_size.y / old_size.y } else { 1.0 };
    match &mut node.kind {
        NodeKind::Vector { paths, network } => {
            scale_paths(paths, kx, ky);
            if let Some(net) = network {
                for v in &mut net.vertices {
                    v.pos = scale_vec2(v.pos, kx, ky);
                }
                for s in &mut net.segments {
                    s.ta = scale_vec2(s.ta, kx, ky);
                    s.tb = scale_vec2(s.tb, kx, ky);
                }
                if net.vertices.iter().any(|v| v.corner_radius > 0.0) && net.is_valid() {
                    *paths = net.to_paths();
                }
            }
            if let Some(fg) = &mut node.fill_geometry {
                scale_paths(fg, kx, ky);
            }
            if let Some(sg) = &mut node.stroke_geometry {
                scale_paths(sg, kx, ky);
            }
        }
        NodeKind::BooleanOp(_) => {
            if let Some(fg) = &mut node.fill_geometry {
                scale_paths(fg, kx, ky);
            }
            if let Some(sg) = &mut node.stroke_geometry {
                scale_paths(sg, kx, ky);
            }
        }
        _ => {
            node.fill_geometry = None;
            node.stroke_geometry = None;
        }
    }
}

/// Plans a recursive constraint pass rooted at `parent` when `parent` resizes
/// from `old` to `new`, up to `depth` levels deep.
///
/// Parents are solved before their children; each child uses its immediate
/// parent's new size. In-flow children of auto-layout parents are skipped,
/// while absolute children are solved. Everything under a boolean group is part
/// of its shape, so it scales with the group whatever constraints it carries.
pub fn plan_constraint_pass(doc: &Document, parent: NodeId, old: Vec2, new: Vec2, depth: usize) -> Vec<(NodeId, Affine, Vec2)> {
    let mut out = Vec::new();
    plan_constraint_pass_inner(doc, parent, old, new, depth, false, &mut out);
    out
}

/// `in_shape`: an ancestor in this pass is a boolean group.
fn plan_constraint_pass_inner(doc: &Document, parent: NodeId, old: Vec2, new: Vec2, depth: usize, in_shape: bool, out: &mut Vec<(NodeId, Affine, Vec2)>) {
    if depth == 0 {
        return;
    }
    let Some(p) = doc.get(parent) else {
        return;
    };
    let has_al = p.kind.frame_props().and_then(|f| f.auto_layout.as_ref()).is_some();
    let in_shape = in_shape || matches!(p.kind, NodeKind::BooleanOp(_));
    for &c in &p.children {
        let Some(k) = doc.get(c) else { continue };
        if has_al && !k.layout_child.absolute {
            continue;
        }
        let constraints = if in_shape { Constraints { horizontal: Constraint::Scale, vertical: Constraint::Scale } } else { k.constraints };
        let Some((new_transform, new_size)) = constrain_with(k, constraints, old, new) else {
            continue;
        };
        let old_child = k.size;
        let size_changed = (new_size.x - old_child.x).abs() > 1e-4 || (new_size.y - old_child.y).abs() > 1e-4;
        let is_container = k.kind.is_container();
        out.push((c, new_transform, new_size));
        if size_changed && is_container {
            plan_constraint_pass_inner(doc, c, old_child, new_size, depth - 1, in_shape, out);
        }
    }
}

/// Whether `kind` is a container that supports "Resize to fit" (`Frame`, `Group`, `Component`, `ComponentSet`).
pub fn is_resize_to_fit_kind(kind: &NodeKind) -> bool {
    matches!(kind, NodeKind::Frame(_) | NodeKind::Group | NodeKind::Component(_) | NodeKind::ComponentSet(_))
}

/// Bounding box of `container_id`'s visible direct children in `container_id`'s own coordinate space.
/// Returns `None` when `container_id` is not a resize-to-fit container or has no visible children.
pub fn visible_children_bounds(doc: &Document, container_id: NodeId) -> Option<crate::Rect> {
    let container = doc.get(container_id)?;
    if !is_resize_to_fit_kind(&container.kind) {
        return None;
    }
    let mut min_x = f32::INFINITY;
    let mut min_y = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut max_y = f32::NEG_INFINITY;
    let mut any = false;
    for &cid in &container.children {
        let Some(raw) = doc.get(cid) else { continue };
        let k = crate::resolve_node(doc, raw);
        if !k.visible {
            continue;
        }
        any = true;
        let corners = [Vec2::new(0.0, 0.0), Vec2::new(k.size.x, 0.0), Vec2::new(k.size.x, k.size.y), Vec2::new(0.0, k.size.y)];
        for p in corners {
            let q = k.transform.apply(p);
            min_x = min_x.min(q.x);
            min_y = min_y.min(q.y);
            max_x = max_x.max(q.x);
            max_y = max_y.max(q.y);
        }
    }
    if !any || !min_x.is_finite() || !min_y.is_finite() || !max_x.is_finite() || !max_y.is_finite() {
        return None;
    }
    Some(crate::Rect::new(min_x, min_y, (max_x - min_x).max(0.0), (max_y - min_y).max(0.0)))
}

/// Whether at least one node in `ids` is a resize-to-fit container with at least one visible child.
pub fn can_resize_to_fit(doc: &Document, ids: &[NodeId]) -> bool {
    ids.iter().any(|&id| visible_children_bounds(doc, id).is_some())
}

/// Returns the eligible resize-to-fit targets from `ids`, deduplicated and ordered deepest-first so nested
/// selected containers fit before their enclosing ancestors.
pub fn ordered_resize_to_fit_targets(doc: &Document, ids: &[NodeId]) -> Vec<NodeId> {
    let depth = |id: NodeId| {
        let mut d = 0usize;
        let mut cur = doc.get(id).and_then(|n| n.parent);
        while let Some(p) = cur {
            d += 1;
            cur = doc.get(p).and_then(|n| n.parent);
        }
        d
    };
    let mut unique: Vec<(usize, usize, NodeId)> = Vec::new();
    for (idx, &id) in ids.iter().enumerate() {
        if visible_children_bounds(doc, id).is_some() && !unique.iter().any(|(_, _, existing)| *existing == id) {
            unique.push((depth(id), idx, id));
        }
    }
    unique.sort_by_key(|&(d, idx, _)| (std::cmp::Reverse(d), idx));
    unique.into_iter().map(|(_, _, id)| id).collect()
}

/// Applies "Resize to fit" to a single container `id` without running auto-layout relayout.
/// Calls `touch(doc, node_id)` before mutating any node so callers can snapshot `before` states.
/// Returns `Some(true)` if `id` is an auto-layout frame (updated to `Sizing::Hug` on both axes),
/// `Some(false)` if `id`'s bounds and children offsets were updated directly, or `None` if `id` was skipped.
pub fn apply_resize_to_fit_one(doc: &mut Document, id: NodeId, mut touch: impl FnMut(&Document, NodeId)) -> Option<bool> {
    let bb = visible_children_bounds(doc, id)?;
    let counter_horiz = doc.get(id).and_then(|n| n.parent).and_then(|p| doc.get(p)).and_then(crate::LayoutChild::counter_axis_is_horizontal);
    let is_al = doc.get(id)?.kind.frame_props().and_then(|f| f.auto_layout.as_ref()).is_some();
    if is_al {
        touch(doc, id);
        let n = doc.get_mut(id)?;
        n.layout_child.set_axis_sizing(true, crate::Sizing::Fixed, counter_horiz);
        n.layout_child.set_axis_sizing(false, crate::Sizing::Fixed, counter_horiz);
        let al = n.kind.frame_props_mut()?.auto_layout.as_mut()?;
        al.primary_sizing = crate::Sizing::Hug;
        al.counter_sizing = crate::Sizing::Hug;
        Some(true)
    } else {
        let children = doc.get(id)?.children.clone();
        touch(doc, id);
        for &cid in &children {
            touch(doc, cid);
        }
        if let Some(n) = doc.get_mut(id) {
            n.layout_child.set_axis_sizing(true, crate::Sizing::Fixed, counter_horiz);
            n.layout_child.set_axis_sizing(false, crate::Sizing::Fixed, counter_horiz);
            let new_size = Vec2::new(bb.w, bb.h);
            if (n.size.x - new_size.x).abs() > 1e-5 || (n.size.y - new_size.y).abs() > 1e-5 {
                n.fill_geometry = None;
                n.stroke_geometry = None;
            }
            n.transform = n.transform * Affine::translate(bb.x, bb.y);
            n.size = new_size;
        }
        for cid in children {
            if let Some(k) = doc.get_mut(cid) {
                k.transform.tx -= bb.x;
                k.transform.ty -= bb.y;
            }
        }
        Some(false)
    }
}

fn snapshot_subtree(doc: &Document, root: NodeId, before: &mut std::collections::BTreeMap<NodeId, Node>) {
    let Some(n) = doc.get(root) else { return };
    before.entry(root).or_insert_with(|| n.clone());
    let children = n.children.clone();
    for cid in children {
        snapshot_subtree(doc, cid, before);
    }
}

impl Document {
    /// Resizes each eligible container in `ids` (`Frame`, `Group`, `Component`, `ComponentSet`) to fit its
    /// visible children as a single undoable history step. Non-container layers (shapes, text, etc.) and
    /// containers without visible children are skipped.
    pub fn resize_to_fit(&mut self, hist: &mut crate::History, ids: &[NodeId]) -> bool {
        let targets = ordered_resize_to_fit_targets(self, ids);
        if targets.is_empty() {
            return false;
        }
        let epoch_before = self.layout_epoch;
        let mut before: std::collections::BTreeMap<NodeId, Node> = std::collections::BTreeMap::new();
        let mut al_seeds: Vec<NodeId> = Vec::new();

        for &id in &targets {
            let is_al = self.get(id).and_then(|n| n.kind.frame_props()).and_then(|f| f.auto_layout.as_ref()).is_some();
            if is_al {
                let mut root = id;
                while let Some(p) = self.get(root).and_then(|n| n.parent) {
                    if self.get(p).and_then(|pn| pn.kind.frame_props()).and_then(|f| f.auto_layout.as_ref()).is_some() {
                        root = p;
                    } else {
                        break;
                    }
                }
                snapshot_subtree(self, root, &mut before);
            } else if let Some(p) = self.get(id).and_then(|n| n.parent) {
                if self.get(p).and_then(|pn| pn.kind.frame_props()).and_then(|f| f.auto_layout.as_ref()).is_some() {
                    let mut root = p;
                    while let Some(gp) = self.get(root).and_then(|n| n.parent) {
                        if self.get(gp).and_then(|gpn| gpn.kind.frame_props()).and_then(|f| f.auto_layout.as_ref()).is_some() {
                            root = gp;
                        } else {
                            break;
                        }
                    }
                    snapshot_subtree(self, root, &mut before);
                    al_seeds.push(p);
                }
            }
            if let Some(true) = apply_resize_to_fit_one(self, id, |doc, nid| {
                if let Some(n) = doc.get(nid) {
                    before.entry(nid).or_insert_with(|| n.clone());
                }
            }) {
                al_seeds.push(id);
            }
        }

        if !al_seeds.is_empty() {
            self.relayout_seeds_upward(&al_seeds);
        }

        let mut cmds = Vec::new();
        for (nid, old_node) in before {
            if let Some(new_node) = self.get(nid).cloned() {
                if old_node != new_node {
                    if let Some(slot) = self.nodes.get_mut(&nid) {
                        *slot = old_node;
                    }
                    cmds.push(crate::Command::Update { node: Box::new(new_node) });
                }
            }
        }
        self.layout_epoch = epoch_before;
        if cmds.is_empty() {
            return true;
        }
        let cmd = if cmds.len() == 1 { cmds.pop().unwrap() } else { crate::Command::Batch(cmds) };
        let ok = hist.exec_user(self, cmd).is_ok();
        if ok {
            hist.mark_last_undo_as_layout_recorded(self, epoch_before);
        }
        ok
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Constraints, NodeKind, Rect};

    #[test]
    fn half_away_from_zero_rounding_table() {
        assert_eq!(round_half_away_from_zero(-0.5), -1.0);
        assert_eq!(round_half_away_from_zero(0.5), 1.0);
        assert_eq!(round_half_away_from_zero(2.5), 3.0);
        assert_eq!(round_half_away_from_zero(-2.5), -3.0);
        assert_eq!(round_half_away_from_zero(0.0), 0.0);
    }

    #[test]
    fn rotated_90_stretch_stretch_gives_100_by_140_without_skew() {
        let mut child = Node::rectangle(NodeId(2), Rect::new(0.0, 0.0, 100.0, 40.0));
        child.transform = Affine { a: 0.0, b: 1.0, c: -1.0, d: 0.0, tx: 60.0, ty: 20.0 };
        child.constraints = Constraints { horizontal: Constraint::Stretch, vertical: Constraint::Stretch };
        let (t, sz) = constrain_child(&child, Vec2::new(200.0, 200.0), Vec2::new(300.0, 200.0)).unwrap();
        assert!((sz.x - 100.0).abs() < 1e-4, "expected w=100, got {}", sz.x);
        assert!((sz.y - 140.0).abs() < 1e-4, "expected h=140, got {}", sz.y);
        // Column dot product a*c + b*d == 0 means no skew.
        let dot = t.a * t.c + t.b * t.d;
        assert!(dot.abs() < 1e-5, "expected no skew, dot={dot}");
        assert!((t.b * t.c - (-1.0)).abs() < 1e-5);
    }

    #[test]
    fn center_rounding_uses_rounded_extents() {
        let mut child = Node::rectangle(NodeId(2), Rect::new(10.0, 10.0, 20.0, 20.0));
        child.constraints = Constraints { horizontal: Constraint::Center, vertical: Constraint::Center };
        // Parent 101 -> 150 moves centred child by r(75) - r(50.5) = 75 - 51 = 24, not 24.5.
        let (t, sz) = constrain_child(&child, Vec2::new(101.0, 101.0), Vec2::new(150.0, 150.0)).unwrap();
        assert_eq!(sz, Vec2::new(20.0, 20.0));
        assert_eq!(t.tx, 34.0);
        assert_eq!(t.ty, 34.0);
    }

    #[test]
    fn sub_one_child_keeps_its_size() {
        // A 0.5 px horizontal divider with Stretch horizontal, Min vertical keeps 0.5 height.
        let mut half_line = Node::rectangle(NodeId(2), Rect::new(0.0, 10.0, 100.0, 0.5));
        half_line.constraints = Constraints { horizontal: Constraint::Stretch, vertical: Constraint::Min };
        let (_, sz) = constrain_child(&half_line, Vec2::new(100.0, 100.0), Vec2::new(200.0, 100.0)).unwrap();
        assert_eq!(sz, Vec2::new(200.0, 0.5));

        // A zero-thickness line stays zero.
        let mut zero_line = Node::new(NodeId(3), NodeKind::Line);
        zero_line.size = Vec2::new(100.0, 0.0);
        zero_line.constraints = Constraints { horizontal: Constraint::Stretch, vertical: Constraint::Min };
        let (_, sz0) = constrain_child(&zero_line, Vec2::new(100.0, 100.0), Vec2::new(250.0, 100.0)).unwrap();
        assert_eq!(sz0, Vec2::new(250.0, 0.0));
    }

    #[test]
    fn rotated_30_stretch_stretch_gives_175_130_by_44_266_with_skew() {
        let mut child = Node::rectangle(NodeId(2), Rect::new(0.0, 0.0, 100.0, 40.0));
        child.transform = Affine::rotate(30.0_f32.to_radians());
        child.constraints = Constraints { horizontal: Constraint::Stretch, vertical: Constraint::Stretch };
        let (t, sz) = constrain_child(&child, Vec2::new(200.0, 200.0), Vec2::new(300.0, 200.0)).unwrap();
        assert!((sz.x - 175.130).abs() < 1e-3, "expected w≈175.130, got {}", sz.x);
        assert!((sz.y - 44.266).abs() < 1e-3, "expected h≈44.266, got {}", sz.y);
        // Non-uniform stretch of a 30-degree rotated rectangle introduces skew (columns not orthogonal).
        let dot = t.a * t.c + t.b * t.d;
        assert!(dot.abs() > 0.1, "expected skew in transform, dot={dot}");
    }

    #[test]
    fn det_zero_leaves_the_child_unchanged() {
        let mut child = Node::rectangle(NodeId(2), Rect::new(10.0, 10.0, 50.0, 50.0));
        child.constraints = Constraints { horizontal: Constraint::Scale, vertical: Constraint::Scale };
        // Parent width collapses to 0 -> det(M') == 0 -> returns None.
        assert!(constrain_child(&child, Vec2::new(100.0, 100.0), Vec2::new(0.0, 100.0)).is_none());
    }

    #[test]
    fn old_extent_under_2_pow_minus_12_scales_by_one() {
        let mut child = Node::rectangle(NodeId(2), Rect::new(10.0, 20.0, 1e-5, 40.0));
        child.constraints = Constraints { horizontal: Constraint::Stretch, vertical: Constraint::Stretch };
        let (t, sz) = constrain_child(&child, Vec2::new(100.0, 100.0), Vec2::new(200.0, 150.0)).unwrap();
        // Horizontal old extent is 1e-5 < 2^-12, so sx = 1.0 (width unchanged), while vertical grows by 50.
        assert!((sz.x - 1e-5).abs() < 1e-9);
        assert!((sz.y - 90.0).abs() < 1e-4);
        assert_eq!(t.tx, 10.0);
        assert_eq!(t.ty, 20.0);
    }

    #[test]
    fn no_local_constraint_copies_remain() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let rest_import = std::fs::read_to_string(root.join("designos-fig/src/rest_import.rs")).unwrap();
        let app_layout = std::fs::read_to_string(root.join("designos-app/src/layout.rs")).unwrap();
        let prod_layout = app_layout.split("#[cfg(test)]").next().unwrap_or(&app_layout);
        assert!(!rest_import.contains("Constraint::Stretch =>"), "rest_import.rs must not hold a local per-axis match on Constraint");
        assert!(!prod_layout.contains("Constraint::Stretch =>"), "layout.rs production code must not hold a local per-axis match on Constraint");
        assert!(rest_import.contains("plan_constraint_pass"), "rest_import.rs must call plan_constraint_pass");
        assert!(prod_layout.contains("plan_constraint_pass"), "layout.rs must call plan_constraint_pass");
    }

    #[test]
    fn shrink_past_inset_stretch_child_collapses_without_mirroring_or_overflow() {
        let mut bar = Node::rectangle(NodeId(2), Rect::new(16.0, 10.0, 168.0, 20.0));
        bar.constraints = Constraints { horizontal: Constraint::Stretch, vertical: Constraint::Min };

        // Shrinking a 200px parent down to 20px (past the 32px total horizontal insets)
        // must collapse the bar width to 0 without flipping transform.a negative.
        let (t20, sz20) = constrain_child(&bar, Vec2::new(200.0, 100.0), Vec2::new(20.0, 100.0)).expect("stretch collapse at 20px");
        assert!(t20.a > 0.0, "transform.a must not flip negative, got {}", t20.a);
        assert_eq!(t20.tx, 16.0);
        assert_eq!(sz20, Vec2::new(0.0, 20.0));

        // Shrinking to 32px lands exactly at new_ext_x == 0; must collapse to 0 instead of returning None.
        let (t32, sz32) = constrain_child(&bar, Vec2::new(200.0, 100.0), Vec2::new(32.0, 100.0)).expect("stretch collapse at 32px");
        assert!(t32.a > 0.0, "transform.a must not flip negative, got {}", t32.a);
        assert_eq!(t32.tx, 16.0);
        assert_eq!(sz32, Vec2::new(0.0, 20.0));

        // Shrinking a 200px parent with a Stretch child at x=10, w=40 (right inset 150) to w=150:
        let mut pill = Node::rectangle(NodeId(3), Rect::new(10.0, 8.0, 40.0, 24.0));
        pill.constraints = Constraints { horizontal: Constraint::Stretch, vertical: Constraint::Stretch };
        let (tp, szp) = constrain_child(&pill, Vec2::new(200.0, 100.0), Vec2::new(150.0, 10.0)).expect("stretch collapse on both axes");
        assert!(tp.a > 0.0 && tp.d > 0.0, "basis must not mirror: {:?}", tp);
        assert_eq!(tp.tx, 10.0);
        assert_eq!(tp.ty, 8.0);
        assert_eq!(szp, Vec2::new(0.0, 0.0));

        // A 100x40 rectangle at 90 degrees (a=0, b=1, c=-1, d=0, tx=60, ty=20), Stretch/Stretch,
        // in a 200x200 frame resized to 150x200 has parent bounds x=20..60, y=20..120.
        // The 50px horizontal shrink collapses parent-X extent to 0 at x=20 while keeping y=20..120.
        let mut rot90 = Node::rectangle(NodeId(4), Rect::new(0.0, 0.0, 100.0, 40.0));
        rot90.transform = Affine { a: 0.0, b: 1.0, c: -1.0, d: 0.0, tx: 60.0, ty: 20.0 };
        rot90.constraints = Constraints { horizontal: Constraint::Stretch, vertical: Constraint::Stretch };
        let (tr90, sz90) = constrain_child(&rot90, Vec2::new(200.0, 200.0), Vec2::new(150.0, 200.0)).expect("90-deg stretch collapse");
        assert_eq!(sz90, Vec2::new(100.0, 0.0));
        assert!((tr90.a - 0.0).abs() < 1e-5 && (tr90.b - 1.0).abs() < 1e-5, "col0 must stay (0, 1): {:?}", tr90);
        assert!((tr90.c - (-1.0)).abs() < 1e-5 && (tr90.d - 0.0).abs() < 1e-5, "col1 must stay (-1, 0): {:?}", tr90);
        assert_eq!(tr90.tx, 20.0);
        assert_eq!(tr90.ty, 20.0);
        let corners90 =
            [tr90.apply(Vec2::new(0.0, 0.0)), tr90.apply(Vec2::new(sz90.x, 0.0)), tr90.apply(Vec2::new(sz90.x, sz90.y)), tr90.apply(Vec2::new(0.0, sz90.y))];
        let min_x = corners90.iter().map(|p| p.x).fold(f32::INFINITY, f32::min);
        let max_x = corners90.iter().map(|p| p.x).fold(f32::NEG_INFINITY, f32::max);
        let min_y = corners90.iter().map(|p| p.y).fold(f32::INFINITY, f32::min);
        let max_y = corners90.iter().map(|p| p.y).fold(f32::NEG_INFINITY, f32::max);
        assert!((max_x - min_x).abs() < 1e-5 && (min_x - 20.0).abs() < 1e-5, "parent-X extent must collapse at x=20: {:?}", corners90);
        assert!((min_y - 20.0).abs() < 1e-5 && (max_y - 120.0).abs() < 1e-5, "parent-Y extent must remain 20..120: {:?}", corners90);
    }
    /// A union at 100 x 80 holding a rectangle and a group with a rectangle in it, both with the default
    /// Min / Min constraints. Returns the document and the union, the rectangle, the group and the inner rectangle.
    fn union_with_operands() -> (Document, [NodeId; 4]) {
        let mut doc = Document::new("t");
        let page = doc.add_page("p");
        let union_id = doc.alloc_id();
        let mut union = Node::new(union_id, NodeKind::BooleanOp(crate::BooleanOp::Union));
        union.size = Vec2::new(100.0, 80.0);
        doc.insert(page, union).unwrap();
        let rect_id = doc.alloc_id();
        doc.insert(union_id, Node::rectangle(rect_id, Rect::new(10.0, 20.0, 40.0, 40.0))).unwrap();
        let group_id = doc.alloc_id();
        let mut group = Node::new(group_id, NodeKind::Group);
        group.transform = Affine::translate(50.0, 0.0);
        group.size = Vec2::new(50.0, 80.0);
        doc.insert(union_id, group).unwrap();
        let inner_id = doc.alloc_id();
        doc.insert(group_id, Node::rectangle(inner_id, Rect::new(10.0, 10.0, 30.0, 60.0))).unwrap();
        (doc, [union_id, rect_id, group_id, inner_id])
    }

    #[test]
    fn operands_of_a_boolean_group_scale_with_it_whatever_their_constraints() {
        let (doc, [union_id, rect_id, group_id, inner_id]) = union_with_operands();
        let plan = plan_constraint_pass(&doc, union_id, Vec2::new(100.0, 80.0), Vec2::new(200.0, 40.0), 64);
        let step = |id: NodeId| plan.iter().find(|(n, ..)| *n == id).map(|(_, t, s)| (*t, *s)).unwrap_or_else(|| panic!("{id:?} is in the plan"));
        let (t, sz) = step(rect_id);
        assert_eq!((t.tx, t.ty, sz), (20.0, 10.0, Vec2::new(80.0, 20.0)));
        let (t, sz) = step(group_id);
        assert_eq!((t.tx, t.ty, sz), (100.0, 0.0, Vec2::new(100.0, 40.0)));
        let (t, sz) = step(inner_id);
        assert_eq!((t.tx, t.ty, sz), (20.0, 5.0, Vec2::new(60.0, 30.0)), "a group inside the union scales its own layers too");
    }

    #[test]
    fn layers_of_a_frame_keep_following_their_own_constraints() {
        let (mut doc, [union_id, rect_id, ..]) = union_with_operands();
        doc.get_mut(union_id).unwrap().kind = NodeKind::Section;
        let plan = plan_constraint_pass(&doc, union_id, Vec2::new(100.0, 80.0), Vec2::new(200.0, 40.0), 64);
        let (_, t, sz) = plan.iter().find(|(n, ..)| *n == rect_id).copied().expect("the rectangle is in the plan");
        assert_eq!((t.tx, t.ty, sz), (10.0, 20.0, Vec2::new(40.0, 40.0)), "Min / Min leaves the layer where it was");
    }

    fn page_transform(doc: &Document, id: NodeId) -> Affine {
        let mut chain = Vec::new();
        let mut cur = Some(id);
        while let Some(nid) = cur {
            let Some(n) = doc.get(nid) else { break };
            if matches!(n.kind, NodeKind::Page { .. }) {
                break;
            }
            chain.push(n.transform);
            cur = n.parent;
        }
        let mut out = Affine::IDENTITY;
        for t in chain.into_iter().rev() {
            out = out * t;
        }
        out
    }

    fn assert_affine_close(a: Affine, b: Affine, eps: f32) {
        assert!(
            (a.a - b.a).abs() <= eps
                && (a.b - b.b).abs() <= eps
                && (a.c - b.c).abs() <= eps
                && (a.d - b.d).abs() <= eps
                && (a.tx - b.tx).abs() <= eps
                && (a.ty - b.ty).abs() <= eps,
            "expected affine {b:?}, got {a:?}"
        );
    }

    #[test]
    fn resize_to_fit_frame_larger_than_children_shrinks_and_keeps_child_page_positions() {
        let mut doc = Document::new("t");
        let page = doc.add_page("p");
        let fid = doc.alloc_id();
        doc.insert(page, Node::frame(fid, Rect::new(100.0, 200.0, 500.0, 400.0))).unwrap();
        let c1 = doc.alloc_id();
        doc.insert(fid, Node::rectangle(c1, Rect::new(40.0, 50.0, 60.0, 80.0))).unwrap();
        let c2 = doc.alloc_id();
        doc.insert(fid, Node::rectangle(c2, Rect::new(140.0, 110.0, 90.0, 70.0))).unwrap();

        let c1_page_before = page_transform(&doc, c1);
        let c2_page_before = page_transform(&doc, c2);

        let mut hist = crate::History::default();
        assert!(doc.resize_to_fit(&mut hist, &[fid]));

        let f = doc.get(fid).unwrap();
        assert_eq!((f.transform.tx, f.transform.ty), (140.0, 250.0));
        assert_eq!(f.size, Vec2::new(190.0, 130.0));
        assert_eq!((doc.get(c1).unwrap().transform.tx, doc.get(c1).unwrap().transform.ty), (0.0, 0.0));
        assert_eq!((doc.get(c2).unwrap().transform.tx, doc.get(c2).unwrap().transform.ty), (100.0, 60.0));
        assert_affine_close(page_transform(&doc, c1), c1_page_before, 1e-4);
        assert_affine_close(page_transform(&doc, c2), c2_page_before, 1e-4);
    }

    #[test]
    fn resize_to_fit_frame_smaller_than_children_grows_and_keeps_child_page_positions() {
        let mut doc = Document::new("t");
        let page = doc.add_page("p");
        let fid = doc.alloc_id();
        doc.insert(page, Node::frame(fid, Rect::new(50.0, 60.0, 40.0, 30.0))).unwrap();
        let c1 = doc.alloc_id();
        doc.insert(fid, Node::rectangle(c1, Rect::new(-20.0, -15.0, 50.0, 40.0))).unwrap();
        let c2 = doc.alloc_id();
        doc.insert(fid, Node::rectangle(c2, Rect::new(80.0, 95.0, 60.0, 50.0))).unwrap();

        let c1_page_before = page_transform(&doc, c1);
        let c2_page_before = page_transform(&doc, c2);

        let mut hist = crate::History::default();
        assert!(doc.resize_to_fit(&mut hist, &[fid]));

        let f = doc.get(fid).unwrap();
        assert_eq!((f.transform.tx, f.transform.ty), (30.0, 45.0));
        assert_eq!(f.size, Vec2::new(160.0, 160.0));
        assert_affine_close(page_transform(&doc, c1), c1_page_before, 1e-4);
        assert_affine_close(page_transform(&doc, c2), c2_page_before, 1e-4);
    }

    #[test]
    fn resize_to_fit_group_fits_visible_children() {
        let mut doc = Document::new("t");
        let page = doc.add_page("p");
        let gid = doc.alloc_id();
        let mut g = Node::new(gid, NodeKind::Group);
        g.transform = Affine::translate(10.0, 20.0);
        g.size = Vec2::new(300.0, 300.0);
        doc.insert(page, g).unwrap();
        let c1 = doc.alloc_id();
        doc.insert(gid, Node::rectangle(c1, Rect::new(25.0, 35.0, 80.0, 45.0))).unwrap();
        let c1_page_before = page_transform(&doc, c1);

        let mut hist = crate::History::default();
        assert!(doc.resize_to_fit(&mut hist, &[gid]));

        let g_after = doc.get(gid).unwrap();
        assert_eq!((g_after.transform.tx, g_after.transform.ty), (35.0, 55.0));
        assert_eq!(g_after.size, Vec2::new(80.0, 45.0));
        assert_affine_close(page_transform(&doc, c1), c1_page_before, 1e-4);
    }

    #[test]
    fn resize_to_fit_component_and_component_set_fit_visible_children() {
        let mut doc = Document::new("t");
        let page = doc.add_page("p");

        let comp_id = doc.alloc_id();
        let mut comp = Node::new(comp_id, NodeKind::Component(crate::FrameProps::default()));
        comp.transform = Affine::translate(10.0, 15.0);
        comp.size = Vec2::new(200.0, 180.0);
        doc.insert(page, comp).unwrap();
        let comp_child = doc.alloc_id();
        doc.insert(comp_id, Node::rectangle(comp_child, Rect::new(12.0, 18.0, 64.0, 32.0))).unwrap();

        let set_id = doc.alloc_id();
        let mut set_node = Node::new(set_id, NodeKind::ComponentSet(crate::FrameProps::default()));
        set_node.transform = Affine::translate(300.0, 40.0);
        set_node.size = Vec2::new(500.0, 400.0);
        doc.insert(page, set_node).unwrap();
        let var1 = doc.alloc_id();
        let mut v1 = Node::new(var1, NodeKind::Component(crate::FrameProps::default()));
        v1.transform = Affine::translate(24.0, 30.0);
        v1.size = Vec2::new(80.0, 36.0);
        doc.insert(set_id, v1).unwrap();
        let var2 = doc.alloc_id();
        let mut v2 = Node::new(var2, NodeKind::Component(crate::FrameProps::default()));
        v2.transform = Affine::translate(124.0, 30.0);
        v2.size = Vec2::new(96.0, 44.0);
        doc.insert(set_id, v2).unwrap();

        let comp_child_page = page_transform(&doc, comp_child);
        let var1_page = page_transform(&doc, var1);
        let var2_page = page_transform(&doc, var2);

        let mut hist = crate::History::default();
        assert!(doc.resize_to_fit(&mut hist, &[comp_id, set_id]));

        let comp_after = doc.get(comp_id).unwrap();
        assert_eq!((comp_after.transform.tx, comp_after.transform.ty), (22.0, 33.0));
        assert_eq!(comp_after.size, Vec2::new(64.0, 32.0));
        assert_affine_close(page_transform(&doc, comp_child), comp_child_page, 1e-4);

        let set_after = doc.get(set_id).unwrap();
        assert_eq!((set_after.transform.tx, set_after.transform.ty), (324.0, 70.0));
        assert_eq!(set_after.size, Vec2::new(196.0, 44.0));
        assert_affine_close(page_transform(&doc, var1), var1_page, 1e-4);
        assert_affine_close(page_transform(&doc, var2), var2_page, 1e-4);
    }

    #[test]
    fn resize_to_fit_auto_layout_frame_turns_hug_on_both_axes() {
        let mut doc = Document::new("t");
        let page = doc.add_page("p");
        let fid = doc.alloc_id();
        let mut frame = Node::frame(fid, Rect::new(20.0, 30.0, 400.0, 300.0));
        frame.kind = NodeKind::Frame(crate::FrameProps {
            auto_layout: Some(crate::AutoLayout {
                direction: crate::LayoutDirection::Horizontal,
                primary_sizing: crate::Sizing::Fixed,
                counter_sizing: crate::Sizing::Fixed,
                item_spacing: 10.0,
                padding: [8.0, 12.0, 8.0, 12.0],
                ..Default::default()
            }),
            ..Default::default()
        });
        doc.insert(page, frame).unwrap();
        let c1 = doc.alloc_id();
        doc.insert(fid, Node::rectangle(c1, Rect::new(0.0, 0.0, 50.0, 24.0))).unwrap();
        let c2 = doc.alloc_id();
        doc.insert(fid, Node::rectangle(c2, Rect::new(0.0, 0.0, 70.0, 34.0))).unwrap();
        doc.relayout_subtree(fid);
        assert_eq!(doc.get(fid).unwrap().size, Vec2::new(400.0, 300.0));

        let mut hist = crate::History::default();
        assert!(doc.resize_to_fit(&mut hist, &[fid]));

        let f = doc.get(fid).unwrap();
        let al = f.kind.frame_props().unwrap().auto_layout.as_ref().unwrap();
        assert_eq!(al.primary_sizing, crate::Sizing::Hug);
        assert_eq!(al.counter_sizing, crate::Sizing::Hug);
        // 12 + 50 + 10 + 70 + 12 = 154 width; 8 + max(24, 34) + 8 = 50 height.
        assert_eq!(f.size, Vec2::new(154.0, 50.0));
        assert_eq!((f.transform.tx, f.transform.ty), (20.0, 30.0));
    }

    #[test]
    fn resize_to_fit_ignores_hidden_children_and_disables_when_none_visible() {
        let mut doc = Document::new("t");
        let page = doc.add_page("p");
        let fid = doc.alloc_id();
        doc.insert(page, Node::frame(fid, Rect::new(0.0, 0.0, 300.0, 300.0))).unwrap();
        let vis = doc.alloc_id();
        doc.insert(fid, Node::rectangle(vis, Rect::new(30.0, 40.0, 50.0, 60.0))).unwrap();
        let hid = doc.alloc_id();
        let mut hid_node = Node::rectangle(hid, Rect::new(200.0, 220.0, 80.0, 70.0));
        hid_node.visible = false;
        doc.insert(fid, hid_node).unwrap();

        let vis_page_before = page_transform(&doc, vis);
        let hid_page_before = page_transform(&doc, hid);

        let mut hist = crate::History::default();
        assert!(can_resize_to_fit(&doc, &[fid]));
        assert!(doc.resize_to_fit(&mut hist, &[fid]));

        let f = doc.get(fid).unwrap();
        assert_eq!((f.transform.tx, f.transform.ty), (30.0, 40.0));
        assert_eq!(f.size, Vec2::new(50.0, 60.0));
        assert_affine_close(page_transform(&doc, vis), vis_page_before, 1e-4);
        assert_affine_close(page_transform(&doc, hid), hid_page_before, 1e-4);

        // Hide the remaining visible child: resize_to_fit must now be disabled and a no-op.
        doc.get_mut(vis).unwrap().visible = false;
        assert!(!can_resize_to_fit(&doc, &[fid]));
        assert!(!doc.resize_to_fit(&mut hist, &[fid]));
    }

    #[test]
    fn resize_to_fit_rotated_child_and_rotated_container_use_container_space() {
        let mut doc = Document::new("t");
        let page = doc.add_page("p");
        let fid = doc.alloc_id();
        let mut frame = Node::frame(fid, Rect::new(0.0, 0.0, 400.0, 400.0));
        // Rotate the container itself by 30 degrees and translate to (100, 150).
        frame.transform = Affine::translate(100.0, 150.0) * Affine::rotate(30.0_f32.to_radians());
        doc.insert(page, frame).unwrap();

        let cid = doc.alloc_id();
        let mut child = Node::rectangle(cid, Rect::new(0.0, 0.0, 100.0, 40.0));
        // Rotate the child 90 degrees inside the container: local corners span x in [20, 60], y in [30, 130].
        child.transform = Affine { a: 0.0, b: 1.0, c: -1.0, d: 0.0, tx: 60.0, ty: 30.0 };
        doc.insert(fid, child).unwrap();

        let child_page_before = page_transform(&doc, cid);

        let mut hist = crate::History::default();
        assert!(doc.resize_to_fit(&mut hist, &[fid]));

        let f = doc.get(fid).unwrap();
        assert!((f.size.x - 40.0).abs() < 1e-4 && (f.size.y - 100.0).abs() < 1e-4, "got size {:?}", f.size);
        let c = doc.get(cid).unwrap();
        // Child's rotated bounds in container now start at (0, 0), so tx became 60 - 20 = 40, ty became 30 - 30 = 0.
        assert!((c.transform.tx - 40.0).abs() < 1e-4 && c.transform.ty.abs() < 1e-4, "got child transform {:?}", c.transform);
        assert_affine_close(page_transform(&doc, cid), child_page_before, 1e-4);
    }

    #[test]
    fn resize_to_fit_does_not_apply_child_constraints() {
        let mut doc = Document::new("t");
        let page = doc.add_page("p");
        let fid = doc.alloc_id();
        doc.insert(page, Node::frame(fid, Rect::new(10.0, 20.0, 400.0, 300.0))).unwrap();

        let stretch_child = doc.alloc_id();
        let mut sc = Node::rectangle(stretch_child, Rect::new(50.0, 60.0, 120.0, 80.0));
        sc.constraints = Constraints { horizontal: Constraint::Stretch, vertical: Constraint::Scale };
        doc.insert(fid, sc).unwrap();

        let max_child = doc.alloc_id();
        let mut mc = Node::rectangle(max_child, Rect::new(90.0, 100.0, 100.0, 70.0));
        mc.constraints = Constraints { horizontal: Constraint::Max, vertical: Constraint::Center };
        doc.insert(fid, mc).unwrap();

        let sc_page_before = page_transform(&doc, stretch_child);
        let mc_page_before = page_transform(&doc, max_child);

        let mut hist = crate::History::default();
        assert!(doc.resize_to_fit(&mut hist, &[fid]));

        // Neither child's size nor page position may change despite Stretch/Scale/Max/Center constraints.
        assert_eq!(doc.get(stretch_child).unwrap().size, Vec2::new(120.0, 80.0));
        assert_eq!(doc.get(max_child).unwrap().size, Vec2::new(100.0, 70.0));
        assert_affine_close(page_transform(&doc, stretch_child), sc_page_before, 1e-4);
        assert_affine_close(page_transform(&doc, max_child), mc_page_before, 1e-4);
    }

    #[test]
    fn resize_to_fit_one_undo_restores_entire_selection_and_skips_shapes_and_text() {
        let mut doc = Document::new("t");
        let page = doc.add_page("p");

        let f1 = doc.alloc_id();
        doc.insert(page, Node::frame(f1, Rect::new(0.0, 0.0, 300.0, 200.0))).unwrap();
        let f1_c = doc.alloc_id();
        doc.insert(f1, Node::rectangle(f1_c, Rect::new(20.0, 30.0, 70.0, 50.0))).unwrap();

        let f2 = doc.alloc_id();
        let mut al_frame = Node::frame(f2, Rect::new(400.0, 0.0, 250.0, 180.0));
        al_frame.kind = NodeKind::Frame(crate::FrameProps {
            auto_layout: Some(crate::AutoLayout {
                direction: crate::LayoutDirection::Vertical,
                primary_sizing: crate::Sizing::Fixed,
                counter_sizing: crate::Sizing::Fixed,
                padding: [10.0; 4],
                ..Default::default()
            }),
            ..Default::default()
        });
        doc.insert(page, al_frame).unwrap();
        let f2_c = doc.alloc_id();
        doc.insert(f2, Node::rectangle(f2_c, Rect::new(0.0, 0.0, 60.0, 40.0))).unwrap();
        doc.relayout_subtree(f2);

        let shape = doc.alloc_id();
        doc.insert(page, Node::rectangle(shape, Rect::new(700.0, 10.0, 45.0, 35.0))).unwrap();
        let text = doc.alloc_id();
        doc.insert(page, Node::text(text, Rect::new(760.0, 10.0, 80.0, 24.0), "Hello", crate::TextStyle::default())).unwrap();

        let mut hist = crate::History::default();
        // Selecting only a shape and text does nothing.
        assert!(!can_resize_to_fit(&doc, &[shape, text]));
        assert!(!doc.resize_to_fit(&mut hist, &[shape, text]));

        // Selecting both frames + shape + text fits both frames and skips the shape and text in one undo step.
        assert!(doc.resize_to_fit(&mut hist, &[f1, f2, shape, text]));
        assert_eq!(doc.get(f1).unwrap().size, Vec2::new(70.0, 50.0));
        assert_eq!((doc.get(f1).unwrap().transform.tx, doc.get(f1).unwrap().transform.ty), (20.0, 30.0));
        assert_eq!(doc.get(f2).unwrap().size, Vec2::new(80.0, 60.0));
        assert_eq!(doc.get(shape).unwrap().size, Vec2::new(45.0, 35.0));
        assert_eq!(doc.get(text).unwrap().size, Vec2::new(80.0, 24.0));

        // Single undo restores both frames and their children.
        assert!(hist.undo(&mut doc).unwrap());
        assert_eq!(doc.get(f1).unwrap().size, Vec2::new(300.0, 200.0));
        assert_eq!((doc.get(f1).unwrap().transform.tx, doc.get(f1).unwrap().transform.ty), (0.0, 0.0));
        assert_eq!((doc.get(f1_c).unwrap().transform.tx, doc.get(f1_c).unwrap().transform.ty), (20.0, 30.0));
        assert_eq!(doc.get(f2).unwrap().size, Vec2::new(250.0, 180.0));
        let al_restored = doc.get(f2).unwrap().kind.frame_props().unwrap().auto_layout.as_ref().unwrap();
        assert_eq!(al_restored.primary_sizing, crate::Sizing::Fixed);
        assert_eq!(al_restored.counter_sizing, crate::Sizing::Fixed);
    }
}
