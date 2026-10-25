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
}
