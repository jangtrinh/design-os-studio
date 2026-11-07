//! Node geometry: converts node kinds into tiny-skia paths in node-local space.

use crate::boolean;
use designos_doc as d;
use designos_doc::{Document, Node, NodeKind};
use std::f32::consts::{PI, TAU};
use tiny_skia as sk;

const KAPPA: f32 = 0.552_284_8;

/// Fill/stroke geometry of a node in local space.
#[derive(Clone, Debug)]
pub struct Shape {
    pub paths: Vec<(sk::Path, sk::FillRule)>,
    /// All sub-paths are closed (inside/outside stroke alignment is meaningful).
    pub closed: bool,
    /// Whether fills apply (lines have no fill).
    pub fillable: bool,
    /// Plain rectangle (used by per-side stroke weights).
    pub is_rect: bool,
    /// Precomputed stroke outline (`Node::stroke_geometry`): filled with the
    /// stroke paints instead of stroking `paths`.
    pub stroke_paths: Option<Vec<(sk::Path, sk::FillRule)>>,
    /// Geometry to fill when it differs from `paths`: a vector network whose
    /// stroke-only segments must not be filled.
    pub fill_only: Option<Vec<(sk::Path, sk::FillRule)>>,
}

impl Shape {
    /// The paths the fill paints (and insides/outsides are measured against).
    pub fn fill_paths(&self) -> &[(sk::Path, sk::FillRule)] {
        self.fill_only.as_deref().unwrap_or(&self.paths)
    }

    fn single(path: sk::Path, rule: sk::FillRule, closed: bool) -> Self {
        Self { paths: vec![(path, rule)], closed, fillable: true, is_rect: false, stroke_paths: None, fill_only: None }
    }
}

pub fn to_sk_transform(a: &d::Affine) -> sk::Transform {
    sk::Transform::from_row(a.a, a.b, a.c, a.d, a.tx, a.ty)
}

pub fn fill_rule(r: d::FillRule) -> sk::FillRule {
    match r {
        d::FillRule::NonZero => sk::FillRule::Winding,
        d::FillRule::EvenOdd => sk::FillRule::EvenOdd,
    }
}

/// Appends an elliptical arc as cubic segments; assumes the current point is
/// already at the arc start. Angles are in radians (y-down, clockwise positive).
fn arc_to(pb: &mut sk::PathBuilder, cx: f32, cy: f32, rx: f32, ry: f32, a0: f32, sweep: f32) {
    let n = ((sweep.abs() / (PI / 2.0)).ceil() as usize).max(1);
    let step = sweep / n as f32;
    let k = 4.0 / 3.0 * (step / 4.0).tan();
    let mut a = a0;
    for _ in 0..n {
        let b = a + step;
        let (sa, ca) = a.sin_cos();
        let (sb, cb) = b.sin_cos();
        pb.cubic_to(cx + rx * (ca - k * sa), cy + ry * (sa + k * ca), cx + rx * (cb + k * sb), cy + ry * (sb - k * cb), cx + rx * cb, cy + ry * sb);
        a = b;
    }
}

fn oval(pb: &mut sk::PathBuilder, cx: f32, cy: f32, rx: f32, ry: f32, reverse: bool) {
    pb.move_to(cx + rx, cy);
    arc_to(pb, cx, cy, rx, ry, 0.0, if reverse { -TAU } else { TAU });
    pb.close();
}

/// Rounded rectangle with independent corner radii (tl, tr, br, bl), each
/// clamped to half of the smaller side.
pub fn rounded_rect(w: f32, h: f32, radii: [f32; 4]) -> Option<sk::Path> {
    if w <= 0.0 || h <= 0.0 {
        return None;
    }
    let lim = w.min(h) / 2.0;
    let r = radii.map(|v| v.max(0.0).min(lim));
    let mut pb = sk::PathBuilder::new();
    if r.iter().all(|v| *v <= 0.0) {
        pb.move_to(0.0, 0.0);
        pb.line_to(w, 0.0);
        pb.line_to(w, h);
        pb.line_to(0.0, h);
        pb.close();
        return pb.finish();
    }
    let [tl, tr, br, bl] = r;
    let k = KAPPA;
    pb.move_to(tl, 0.0);
    pb.line_to(w - tr, 0.0);
    pb.cubic_to(w - tr + tr * k, 0.0, w, tr - tr * k, w, tr);
    pb.line_to(w, h - br);
    pb.cubic_to(w, h - br + br * k, w - br + br * k, h, w - br, h);
    pb.line_to(bl, h);
    pb.cubic_to(bl - bl * k, h, 0.0, h - bl + bl * k, 0.0, h - bl);
    pb.line_to(0.0, tl);
    pb.cubic_to(0.0, tl - tl * k, tl - tl * k, 0.0, tl, 0.0);
    pb.close();
    pb.finish()
}

/// Per-corner constants of Figma's corner smoothing (the construction from
/// Figma's "Desperately seeking squircles" article, as implemented by
/// figma-squircle). Template space: the corner starts at the origin on the
/// edge, the edge direction is +x and the turn is toward +y.
struct SmoothCorner {
    a: f32,
    b: f32,
    c: f32,
    d: f32,
    /// How far from the corner point the curve starts along each edge.
    p: f32,
    arc_len: f32,
    arc_measure: f32,
    r: f32,
}

impl SmoothCorner {
    fn new(r: f32, smoothing: f32, budget: f32) -> Self {
        if r <= 0.0 {
            return Self { a: 0.0, b: 0.0, c: 0.0, d: 0.0, p: 0.0, arc_len: 0.0, arc_measure: 0.0, r: 0.0 };
        }
        // p = (1 + smoothing) * r, limited by the room left on the side; when
        // it is limited the smoothing is reduced rather than the radius.
        let s = smoothing.clamp(0.0, 1.0).min((budget / r - 1.0).max(0.0));
        let p = ((1.0 + s) * r).min(budget);
        let arc_measure = (90.0 * (1.0 - s)).to_radians();
        let arc_len = (arc_measure / 2.0).sin() * r * std::f32::consts::SQRT_2;
        let alpha = (PI / 2.0 - arc_measure) / 2.0;
        let p3_p4 = r * (alpha / 2.0).tan();
        let beta = (PI / 4.0) * s;
        let c = p3_p4 * beta.cos();
        let d = c * beta.tan();
        let b = (p - arc_len - c - d) / 3.0;
        Self { a: 2.0 * b, b, c, d, p, arc_len, arc_measure, r }
    }

    /// Appends the corner (cubic, arc, cubic) starting at `start`, with edge
    /// direction `u` and turn direction `v`.
    fn emit(&self, pb: &mut sk::PathBuilder, start: (f32, f32), u: (f32, f32), v: (f32, f32)) {
        if self.p <= 0.0 {
            return;
        }
        let m = |x: f32, y: f32| (start.0 + u.0 * x + v.0 * y, start.1 + u.1 * x + v.1 * y);
        let Self { a, b, c, d, arc_len, arc_measure, r, .. } = *self;
        let cubic = |pb: &mut sk::PathBuilder, p1: (f32, f32), p2: (f32, f32), p3: (f32, f32)| {
            let (p1, p2, p3) = (m(p1.0, p1.1), m(p2.0, p2.1), m(p3.0, p3.1));
            pb.cubic_to(p1.0, p1.1, p2.0, p2.1, p3.0, p3.1);
        };
        cubic(pb, (a, 0.0), (a + b, 0.0), (a + b + c, d));
        let p1 = (a + b + c, d);
        let p2 = (p1.0 + arc_len, p1.1 + arc_len);
        if arc_measure > 1e-4 {
            // circular arc between p1 and p2, clockwise (y down)
            let phi0 = PI / 4.0 - arc_measure / 2.0 - PI / 2.0;
            let (cx, cy) = (p1.0 - r * phi0.cos(), p1.1 - r * phi0.sin());
            let n = ((arc_measure / (PI / 2.0)).ceil() as usize).max(1);
            let step = arc_measure / n as f32;
            let k = 4.0 / 3.0 * (step / 4.0).tan();
            let mut ang = phi0;
            for _ in 0..n {
                let nb = ang + step;
                let (sa, ca) = ang.sin_cos();
                let (sb, cb) = nb.sin_cos();
                cubic(pb, (cx + r * (ca - k * sa), cy + r * (sa + k * ca)), (cx + r * (cb + k * sb), cy + r * (sb - k * cb)), (cx + r * cb, cy + r * sb));
                ang = nb;
            }
        }
        cubic(pb, (p2.0 + d, p2.1 + c), (p2.0 + d, p2.1 + b + c), (p2.0 + d, p2.1 + a + b + c));
    }
}

/// Rounded rectangle with Figma corner smoothing (0..1). With smoothing 0 this
/// is exactly [`rounded_rect`].
pub fn smooth_rounded_rect(w: f32, h: f32, radii: [f32; 4], smoothing: f32) -> Option<sk::Path> {
    if w <= 0.0 || h <= 0.0 {
        return None;
    }
    if smoothing <= 0.0 || radii.iter().all(|v| *v <= 0.0) {
        return rounded_rect(w, h, radii);
    }
    let lim = w.min(h) / 2.0;
    let r = radii.map(|v| v.max(0.0).min(lim));
    // corner order: tl, tr, br, bl; neighbours with the side length shared with them
    let neighbours = [[(1, w), (3, h)], [(0, w), (2, h)], [(1, h), (3, w)], [(2, w), (0, h)]];
    let mut order = [0usize, 1, 2, 3];
    order.sort_by(|x, y| r[*y].partial_cmp(&r[*x]).unwrap_or(std::cmp::Ordering::Equal));
    let mut used = [f32::NAN; 4];
    let mut corners: [Option<SmoothCorner>; 4] = [None, None, None, None];
    for i in order {
        let budget = neighbours[i]
            .iter()
            .map(|&(j, side)| {
                if r[j] <= 0.0 {
                    side
                } else if (r[i] - r[j]).abs() < 1e-6 {
                    side / 2.0
                } else if !used[j].is_nan() {
                    side - used[j]
                } else {
                    side * r[i] / (r[i] + r[j])
                }
            })
            .fold(f32::MAX, f32::min);
        let c = SmoothCorner::new(r[i], smoothing, budget);
        used[i] = c.p;
        corners[i] = Some(c);
    }
    let [tl, tr, br, bl] = corners.map(|c| c.unwrap());
    let mut pb = sk::PathBuilder::new();
    pb.move_to(w - tr.p, 0.0);
    tr.emit(&mut pb, (w - tr.p, 0.0), (1.0, 0.0), (0.0, 1.0));
    pb.line_to(w, h - br.p);
    br.emit(&mut pb, (w, h - br.p), (0.0, 1.0), (-1.0, 0.0));
    pb.line_to(bl.p, h);
    bl.emit(&mut pb, (bl.p, h), (-1.0, 0.0), (0.0, -1.0));
    pb.line_to(0.0, tl.p);
    tl.emit(&mut pb, (0.0, tl.p), (0.0, -1.0), (1.0, 0.0));
    pb.close();
    pb.finish()
}

/// An ellipse whose inner radius reaches the outer one: it has a path but no area.
pub(crate) fn is_hairline_ring(arc: &d::ArcData) -> bool {
    arc.inner_radius >= 0.999
}

fn ellipse_path(w: f32, h: f32, arc: &d::ArcData) -> Option<sk::Path> {
    if w <= 0.0 || h <= 0.0 {
        return None;
    }
    let (cx, cy, rx, ry) = (w / 2.0, h / 2.0, w / 2.0, h / 2.0);
    // a ring whose inner radius reaches the outer one has no width: it is a single circle, not two
    let hairline = is_hairline_ring(arc);
    let ir = arc.inner_radius.clamp(0.0, 0.999);
    let raw = arc.end - arc.start;
    let full = raw.abs() >= TAU - 1e-4;
    let mut pb = sk::PathBuilder::new();
    if full {
        oval(&mut pb, cx, cy, rx, ry, false);
        if ir > 0.0 && !hairline {
            oval(&mut pb, cx, cy, rx * ir, ry * ir, true);
        }
        return pb.finish();
    }
    let mut sweep = raw;
    if sweep < 0.0 {
        sweep += TAU;
    }
    if sweep <= 1e-6 {
        return None;
    }
    let a0 = arc.start;
    let a1 = a0 + sweep;
    let pt = |a: f32, r: f32| (cx + rx * r * a.cos(), cy + ry * r * a.sin());
    let (sx, sy) = pt(a0, 1.0);
    if hairline {
        pb.move_to(sx, sy);
        arc_to(&mut pb, cx, cy, rx, ry, a0, sweep);
    } else if ir > 0.0 {
        pb.move_to(sx, sy);
        arc_to(&mut pb, cx, cy, rx, ry, a0, sweep);
        let (ex, ey) = pt(a1, ir);
        pb.line_to(ex, ey);
        arc_to(&mut pb, cx, cy, rx * ir, ry * ir, a1, -sweep);
        pb.close();
    } else {
        pb.move_to(cx, cy);
        pb.line_to(sx, sy);
        arc_to(&mut pb, cx, cy, rx, ry, a0, sweep);
        pb.close();
    }
    pb.finish()
}

/// Maps a point of the unit circle onto the ellipse inscribed in a `w` x `h` box, where Figma places
/// polygon and star vertices. Shared with the exporters so they match the canvas.
pub fn inscribed_point(w: f32, h: f32, (x, y): (f32, f32)) -> (f32, f32) {
    ((x + 1.0) / 2.0 * w, (y + 1.0) / 2.0 * h)
}

/// Polygon/star outline with `pts` given on the unit circle: they land on the ellipse inscribed in the
/// node box, as Figma places them, so a box that is not the points' own bounds does not stretch the shape. Every vertex is
/// rounded with `radius` (node units, reduced where the adjacent edges are too
/// short to hold it).
fn poly_path(w: f32, h: f32, pts: &[(f32, f32)], radius: f32) -> Option<sk::Path> {
    if pts.len() < 3 || w <= 0.0 || h <= 0.0 {
        return None;
    }
    let pts: Vec<(f32, f32)> = pts.iter().map(|p| inscribed_point(w, h, *p)).collect();
    let mut pb = sk::PathBuilder::new();
    if radius <= 0.0 {
        for (i, (px, py)) in pts.iter().enumerate() {
            if i == 0 {
                pb.move_to(*px, *py);
            } else {
                pb.line_to(*px, *py);
            }
        }
        pb.close();
        return pb.finish();
    }
    let n = pts.len();
    let unit = |from: (f32, f32), to: (f32, f32)| {
        let (dx, dy) = (to.0 - from.0, to.1 - from.1);
        let l = (dx * dx + dy * dy).sqrt().max(1e-9);
        ((dx / l, dy / l), l)
    };
    // How far each vertex's tangent points sit from it. An edge is shared by two
    // vertices, so each may use at most half of it (equal-radius case) which keeps
    // the rounded corners from overlapping.
    let mut corners = Vec::with_capacity(n);
    for i in 0..n {
        let (prev, cur, next) = (pts[(i + n - 1) % n], pts[i], pts[(i + 1) % n]);
        let (d1, l1) = unit(cur, prev);
        let (d2, l2) = unit(cur, next);
        let cos = (d1.0 * d2.0 + d1.1 * d2.1).clamp(-1.0, 1.0);
        let phi = cos.acos(); // interior angle
        let half_tan = (phi / 2.0).tan();
        if !(1e-3..=PI - 1e-3).contains(&phi) || half_tan < 1e-6 {
            corners.push((cur, cur, cur, d1, d2, 0.0));
            continue;
        }
        let t = (radius / half_tan).min(l1 / 2.0).min(l2 / 2.0);
        let r = t * half_tan;
        // cubic handle for a circular arc spanning the exterior angle
        let k = 4.0 / 3.0 * ((PI - phi) / 4.0).tan() * r;
        corners.push(((cur.0 + d1.0 * t, cur.1 + d1.1 * t), (cur.0 + d2.0 * t, cur.1 + d2.1 * t), cur, d1, d2, k));
    }
    for (i, (a, b, _, d1, d2, k)) in corners.iter().enumerate() {
        if i == 0 {
            pb.move_to(a.0, a.1);
        } else {
            pb.line_to(a.0, a.1);
        }
        pb.cubic_to(a.0 - d1.0 * k, a.1 - d1.1 * k, b.0 - d2.0 * k, b.1 - d2.1 * k, b.0, b.1);
    }
    pb.close();
    pb.finish()
}

fn polygon_points(count: u32) -> Vec<(f32, f32)> {
    let n = count.max(3);
    (0..n)
        .map(|i| {
            let a = -PI / 2.0 + TAU * i as f32 / n as f32;
            (a.cos(), a.sin())
        })
        .collect()
}

fn star_points(count: u32, inner: f32) -> Vec<(f32, f32)> {
    let n = count.max(3);
    (0..n * 2)
        .map(|i| {
            let a = -PI / 2.0 + PI * i as f32 / n as f32;
            let r = if i % 2 == 0 { 1.0 } else { inner.clamp(0.0, 1.0) };
            (a.cos() * r, a.sin() * r)
        })
        .collect()
}

/// Converts document vector paths to tiny-skia paths.
pub fn vector_paths(paths: &[d::Path]) -> Shape {
    let mut out = Vec::new();
    let mut closed = true;
    for p in paths {
        let mut pb = sk::PathBuilder::new();
        let mut open = false;
        let mut started = false;
        for c in &p.cmds {
            match *c {
                d::PathCmd::MoveTo(v) => {
                    if open {
                        closed = false;
                    }
                    pb.move_to(v.x, v.y);
                    started = true;
                    open = true;
                }
                d::PathCmd::LineTo(v) => {
                    if !started {
                        pb.move_to(v.x, v.y);
                        started = true;
                    } else {
                        pb.line_to(v.x, v.y);
                    }
                    open = true;
                }
                d::PathCmd::QuadTo(c1, v) => {
                    if started {
                        pb.quad_to(c1.x, c1.y, v.x, v.y);
                    }
                    open = true;
                }
                d::PathCmd::CubicTo(c1, c2, v) => {
                    if started {
                        pb.cubic_to(c1.x, c1.y, c2.x, c2.y, v.x, v.y);
                    }
                    open = true;
                }
                d::PathCmd::Close => {
                    pb.close();
                    open = false;
                }
            }
        }
        if open {
            closed = false;
        }
        if let Some(path) = pb.finish() {
            out.push((path, fill_rule(p.fill_rule)));
        }
    }
    Shape { paths: out, closed, fillable: true, is_rect: false, stroke_paths: None, fill_only: None }
}

/// Geometry for a node, or `None` when it has no own geometry (groups, pages,
/// text, unknown). `tol` is the flattening tolerance in local units used by
/// boolean operations.
///
/// Precomputed `fill_geometry` / `stroke_geometry` (imported from Figma) take
/// precedence over geometry derived from the node kind.
pub fn node_shape(doc: &Document, n: &Node, tol: f32) -> Option<Shape> {
    let given = n.fill_geometry.as_deref().map(vector_paths).filter(|s| !s.paths.is_empty());
    let has_fill_geometry = given.is_some();
    let mut shape = match given {
        Some(s) => Some(s),
        None => derived_shape(doc, n, tol),
    }?;
    if matches!(
        n.kind,
        NodeKind::Frame(_) | NodeKind::Component(_) | NodeKind::ComponentSet(_) | NodeKind::Instance { .. } | NodeKind::Section | NodeKind::Rectangle
    ) {
        shape.is_rect = true;
    }
    if let Some(sg) = n.stroke_geometry.as_deref().map(vector_paths).filter(|s| !s.paths.is_empty()) {
        shape.stroke_paths = Some(sg.paths);
    } else if has_fill_geometry {
        stroke_network_not_fill_outline(n, &mut shape);
    }
    Some(shape)
}

/// A vector that keeps Figma's fill outline but whose own `paths` are no longer that outline (a resize
/// rebuilt them from the centerline network and dropped the baked stroke) strokes those paths: the
/// stroke follows the network, chains that are only stroked included, while the fill keeps the
/// outline with its winding, regions and open-path fills.
fn stroke_network_not_fill_outline(n: &Node, shape: &mut Shape) {
    let NodeKind::Vector { paths, network: Some(net) } = &n.kind else { return };
    let Some(fill) = n.fill_geometry.as_deref() else { return };
    if paths.is_empty() || paths == fill || net.is_empty() || !net.is_valid() {
        return;
    }
    let stroke = vector_paths(paths);
    if stroke.paths.is_empty() {
        return;
    }
    shape.fill_only = Some(std::mem::replace(&mut shape.paths, stroke.paths));
    shape.closed = stroke.closed;
}

fn derived_shape(doc: &Document, n: &Node, tol: f32) -> Option<Shape> {
    let (w, h) = (n.size.x, n.size.y);
    match &n.kind {
        NodeKind::Frame(_) | NodeKind::Component(_) | NodeKind::ComponentSet(_) | NodeKind::Instance { .. } | NodeKind::Section | NodeKind::Rectangle => {
            let radii = if matches!(n.kind, NodeKind::Section) { [0.0; 4] } else { n.corner_radii };
            let mut s = Shape::single(smooth_rounded_rect(w, h, radii, n.corner_smoothing)?, sk::FillRule::Winding, true);
            s.is_rect = true;
            Some(s)
        }
        NodeKind::Ellipse(arc) => {
            let mut s = Shape::single(ellipse_path(w, h, arc)?, sk::FillRule::Winding, true);
            // a ring as wide as its disc has no area to fill; its stroke still follows the path
            s.fillable = !is_hairline_ring(arc);
            Some(s)
        }
        NodeKind::Line => {
            let mut pb = sk::PathBuilder::new();
            pb.move_to(0.0, 0.0);
            pb.line_to(w, 0.0);
            Some(Shape {
                paths: vec![(pb.finish()?, sk::FillRule::Winding)],
                closed: false,
                fillable: false,
                is_rect: false,
                stroke_paths: None,
                fill_only: None,
            })
        }
        NodeKind::Polygon { count } => Some(Shape::single(poly_path(w, h, &polygon_points(*count), n.corner_radii[0])?, sk::FillRule::Winding, true)),
        NodeKind::Star { count, inner_ratio } => {
            Some(Shape::single(poly_path(w, h, &star_points(*count, *inner_ratio), n.corner_radii[0])?, sk::FillRule::Winding, true))
        }
        NodeKind::Vector { paths, network } => {
            let mut s = vector_paths(paths);
            if s.paths.is_empty() {
                return None;
            }
            if let Some(net) = network.as_ref().filter(|n| n.is_valid()) {
                if net.has_stroke_only_segments() {
                    s.fill_only = Some(vector_paths(&net.fill_paths()).paths);
                }
            }
            Some(s)
        }
        NodeKind::BooleanOp(op) => boolean::boolean_shape(doc, n, *op, tol),
        // board items have the geometry of their lowered body
        NodeKind::Sticky(_) | NodeKind::Connector(_) | NodeKind::ShapeWithText(_) | NodeKind::Stamp { .. } => {
            let body = crate::board::lower(doc, n)?;
            derived_shape(doc, &body, tol)
        }
        NodeKind::Page { .. } | NodeKind::Group | NodeKind::Text(_) | NodeKind::Unknown { .. } | NodeKind::Table(_) | NodeKind::TableCell(_) => None,
    }
}

/// Axis-aligned bounds of a rect transformed by `ts`: (x0, y0, x1, y1).
pub fn transformed_bounds(ts: &sk::Transform, x: f32, y: f32, w: f32, h: f32) -> (f32, f32, f32, f32) {
    let pts = [(x, y), (x + w, y), (x + w, y + h), (x, y + h)];
    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for (px, py) in pts {
        let tx = ts.sx * px + ts.kx * py + ts.tx;
        let ty = ts.ky * px + ts.sy * py + ts.ty;
        x0 = x0.min(tx);
        y0 = y0.min(ty);
        x1 = x1.max(tx);
        y1 = y1.max(ty);
    }
    (x0, y0, x1, y1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rounded_rect_bounds() {
        let p = rounded_rect(100.0, 50.0, [30.0, 0.0, 0.0, 0.0]).unwrap();
        let b = p.bounds();
        assert_eq!((b.left(), b.top(), b.right(), b.bottom()), (0.0, 0.0, 100.0, 50.0));
    }
}
