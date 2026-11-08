//! Boolean operations on vector geometry: curves are flattened to polygons and
//! combined with `i_overlay`; the result is an even-odd polygon path.

use crate::geom::{node_shape, Shape};
use designos_doc as d;
use designos_doc::{resolve_node, Document, Node, NodeKind};
use i_overlay::core::fill_rule::FillRule;
use i_overlay::core::overlay_rule::OverlayRule;
use i_overlay::float::simplify::SimplifyShape;
use i_overlay::float::single::SingleFloatOverlay;
use tiny_skia as sk;
use tiny_skia::PathSegment;

type Pt = [f64; 2];
type Contours = Vec<Vec<Pt>>;
type Shapes = Vec<Vec<Vec<Pt>>>;

fn flatten_quad(out: &mut Vec<Pt>, p0: sk::Point, p1: sk::Point, p2: sk::Point, tol: f32) {
    let len = (p1 - p0).length() + (p2 - p1).length();
    let n = (((len / tol).sqrt() * 0.35).ceil() as usize).clamp(3, 64);
    for i in 1..=n {
        let t = i as f32 / n as f32;
        let u = 1.0 - t;
        out.push([(u * u * p0.x + 2.0 * u * t * p1.x + t * t * p2.x) as f64, (u * u * p0.y + 2.0 * u * t * p1.y + t * t * p2.y) as f64]);
    }
}

fn flatten_cubic(out: &mut Vec<Pt>, p0: sk::Point, p1: sk::Point, p2: sk::Point, p3: sk::Point, tol: f32) {
    let len = (p1 - p0).length() + (p2 - p1).length() + (p3 - p2).length();
    let n = (((len / tol).sqrt() * 0.35).ceil() as usize).clamp(4, 96);
    for i in 1..=n {
        let t = i as f32 / n as f32;
        let u = 1.0 - t;
        let (a, b, c, e) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
        out.push([(a * p0.x + b * p1.x + c * p2.x + e * p3.x) as f64, (a * p0.y + b * p1.y + c * p2.y + e * p3.y) as f64]);
    }
}

/// Flattens a path into polygon contours.
pub fn flatten(path: &sk::Path, tol: f32) -> Contours {
    let mut out: Contours = Vec::new();
    let mut cur: Vec<Pt> = Vec::new();
    let mut last = sk::Point::from_xy(0.0, 0.0);
    for seg in path.segments() {
        match seg {
            PathSegment::MoveTo(p) => {
                if cur.len() >= 3 {
                    out.push(std::mem::take(&mut cur));
                } else {
                    cur.clear();
                }
                cur.push([p.x as f64, p.y as f64]);
                last = p;
            }
            PathSegment::LineTo(p) => {
                cur.push([p.x as f64, p.y as f64]);
                last = p;
            }
            PathSegment::QuadTo(c, p) => {
                flatten_quad(&mut cur, last, c, p, tol);
                last = p;
            }
            PathSegment::CubicTo(c1, c2, p) => {
                flatten_cubic(&mut cur, last, c1, c2, p, tol);
                last = p;
            }
            PathSegment::Close => {
                if cur.len() >= 3 {
                    out.push(std::mem::take(&mut cur));
                } else {
                    cur.clear();
                }
            }
        }
    }
    if cur.len() >= 3 {
        out.push(cur);
    }
    out
}

fn io_rule(r: sk::FillRule) -> FillRule {
    match r {
        sk::FillRule::Winding => FillRule::NonZero,
        sk::FillRule::EvenOdd => FillRule::EvenOdd,
    }
}

fn transform_shapes(shapes: &mut Shapes, a: &d::Affine) {
    for sh in shapes.iter_mut() {
        for c in sh.iter_mut() {
            for p in c.iter_mut() {
                let (x, y) = (p[0] as f32, p[1] as f32);
                p[0] = (a.a * x + a.c * y + a.tx) as f64;
                p[1] = (a.b * x + a.d * y + a.ty) as f64;
            }
        }
    }
}

/// Normalised geometry of `child` in its parent's coordinate space.
fn child_shapes(doc: &Document, child: &Node, tol: f32) -> Option<Shapes> {
    let resolved = resolve_node(doc, child);
    let child = &*resolved;
    if !child.visible {
        return None;
    }
    let mut shapes: Shapes = if matches!(child.kind, NodeKind::Group) {
        combine(doc, child, OverlayRule::Union, tol)?
    } else {
        // a ring as wide as its disc is an operand with no area, not a missing one
        if matches!(&child.kind, NodeKind::Ellipse(arc) if crate::geom::is_hairline_ring(arc)) {
            return Some(Shapes::new());
        }
        let shape: Shape = node_shape(doc, child, tol)?;
        if !shape.fillable {
            return None;
        }
        let mut acc: Option<Shapes> = None;
        for (path, rule) in &shape.paths {
            let contours = flatten(path, tol);
            if contours.is_empty() {
                continue;
            }
            let s = contours.simplify_shape(io_rule(*rule));
            acc = Some(match acc {
                None => s,
                Some(a) => a.overlay(&s, OverlayRule::Union, FillRule::NonZero),
            });
        }
        acc?
    };
    transform_shapes(&mut shapes, &child.transform);
    Some(shapes)
}

fn combine(doc: &Document, parent: &Node, rule: OverlayRule, tol: f32) -> Option<Shapes> {
    let mut acc: Option<Shapes> = None;
    for cid in &parent.children {
        let Some(child) = doc.get(*cid) else { continue };
        let Some(s) = child_shapes(doc, child, tol) else { continue };
        acc = Some(match acc {
            None => s,
            Some(a) => a.overlay(&s, rule, FillRule::NonZero),
        });
    }
    acc
}

pub fn boolean_shape(doc: &Document, node: &Node, op: d::BooleanOp, tol: f32) -> Option<Shape> {
    let rule = match op {
        d::BooleanOp::Union => OverlayRule::Union,
        d::BooleanOp::Subtract => OverlayRule::Difference,
        d::BooleanOp::Intersect => OverlayRule::Intersect,
        d::BooleanOp::Exclude => OverlayRule::Xor,
    };
    let shapes = combine(doc, node, rule, tol.max(0.01))?;
    let mut pb = sk::PathBuilder::new();
    for sh in &shapes {
        for c in sh {
            if c.len() < 3 {
                continue;
            }
            pb.move_to(c[0][0] as f32, c[0][1] as f32);
            for p in &c[1..] {
                pb.line_to(p[0] as f32, p[1] as f32);
            }
            pb.close();
        }
    }
    let path = pb.finish()?;
    Some(Shape { paths: vec![(path, sk::FillRule::EvenOdd)], closed: true, fillable: true, is_rect: false, stroke_paths: None, fill_only: None })
}
