//! Node outlines as plain path commands (local space), for the SVG writer.

use crate::util::num;
use designos_doc::{Affine, ArcData, FillRule, Node, NodeKind, Path, PathCmd, Vec2};
use std::f32::consts::{PI, TAU};

const KAPPA: f32 = 0.552_284_8;

#[derive(Clone, Debug, PartialEq)]
pub enum GeoKind {
    /// Plain rectangle with one corner radius (0 = square).
    Rect {
        w: f32,
        h: f32,
        r: f32,
    },
    /// Full ellipse filling the box.
    Ellipse {
        w: f32,
        h: f32,
    },
    Path,
}

#[derive(Clone, Debug)]
pub struct Geo {
    pub kind: GeoKind,
    pub paths: Vec<(Vec<PathCmd>, FillRule)>,
    /// Every sub-path is closed (inside/outside strokes make sense).
    pub closed: bool,
    pub fillable: bool,
    /// Axis-aligned rectangle (per-side stroke weights apply).
    pub is_rect: bool,
    /// Extra attribute (a `mask="url(#..)"`) added to every element drawn with this geometry.
    pub mask: Option<String>,
}

fn v(x: f32, y: f32) -> Vec2 {
    Vec2::new(x, y)
}

pub fn rounded_rect(w: f32, h: f32, radii: [f32; 4]) -> Vec<PathCmd> {
    let lim = (w.min(h) / 2.0).max(0.0);
    let [tl, tr, br, bl] = radii.map(|r| r.max(0.0).min(lim));
    let k = KAPPA;
    if tl + tr + br + bl <= 0.0 {
        return vec![PathCmd::MoveTo(v(0.0, 0.0)), PathCmd::LineTo(v(w, 0.0)), PathCmd::LineTo(v(w, h)), PathCmd::LineTo(v(0.0, h)), PathCmd::Close];
    }
    let mut out = vec![PathCmd::MoveTo(v(tl, 0.0)), PathCmd::LineTo(v(w - tr, 0.0))];
    // square corners need no curve
    if tr > 0.0 {
        out.push(PathCmd::CubicTo(v(w - tr + tr * k, 0.0), v(w, tr - tr * k), v(w, tr)));
    }
    out.push(PathCmd::LineTo(v(w, h - br)));
    if br > 0.0 {
        out.push(PathCmd::CubicTo(v(w, h - br + br * k), v(w - br + br * k, h), v(w - br, h)));
    }
    out.push(PathCmd::LineTo(v(bl, h)));
    if bl > 0.0 {
        out.push(PathCmd::CubicTo(v(bl - bl * k, h), v(0.0, h - bl + bl * k), v(0.0, h - bl)));
    }
    out.push(PathCmd::LineTo(v(0.0, tl)));
    if tl > 0.0 {
        out.push(PathCmd::CubicTo(v(0.0, tl - tl * k), v(tl - tl * k, 0.0), v(tl, 0.0)));
    }
    out.push(PathCmd::Close);
    out
}

fn arc_cubics(out: &mut Vec<PathCmd>, cx: f32, cy: f32, rx: f32, ry: f32, a0: f32, sweep: f32) {
    let n = ((sweep.abs() / (PI / 2.0)).ceil() as usize).max(1);
    let step = sweep / n as f32;
    let k = 4.0 / 3.0 * (step / 4.0).tan();
    let mut a = a0;
    for _ in 0..n {
        let b = a + step;
        let (sa, ca) = a.sin_cos();
        let (sb, cb) = b.sin_cos();
        out.push(PathCmd::CubicTo(
            v(cx + rx * (ca - k * sa), cy + ry * (sa + k * ca)),
            v(cx + rx * (cb + k * sb), cy + ry * (sb - k * cb)),
            v(cx + rx * cb, cy + ry * sb),
        ));
        a = b;
    }
}

fn oval(out: &mut Vec<PathCmd>, cx: f32, cy: f32, rx: f32, ry: f32, reverse: bool) {
    out.push(PathCmd::MoveTo(v(cx + rx, cy)));
    arc_cubics(out, cx, cy, rx, ry, 0.0, if reverse { -TAU } else { TAU });
    out.push(PathCmd::Close);
}

/// Ellipse / arc / ring outline, same construction as the renderer.
fn ellipse_cmds(w: f32, h: f32, arc: &ArcData) -> Option<Vec<PathCmd>> {
    if w <= 0.0 || h <= 0.0 {
        return None;
    }
    let (cx, cy, rx, ry) = (w / 2.0, h / 2.0, w / 2.0, h / 2.0);
    let ir = arc.inner_radius.clamp(0.0, 0.999);
    let raw = arc.end - arc.start;
    let mut out = Vec::new();
    if raw.abs() >= TAU - 1e-4 {
        oval(&mut out, cx, cy, rx, ry, false);
        if ir > 0.0 {
            oval(&mut out, cx, cy, rx * ir, ry * ir, true);
        }
        return Some(out);
    }
    let sweep = if raw < 0.0 { raw + TAU } else { raw };
    if sweep <= 1e-6 {
        return None;
    }
    let (a0, a1) = (arc.start, arc.start + sweep);
    let pt = |a: f32, r: f32| v(cx + rx * r * a.cos(), cy + ry * r * a.sin());
    out.push(PathCmd::MoveTo(if ir > 0.0 { pt(a0, 1.0) } else { v(cx, cy) }));
    if ir <= 0.0 {
        out.push(PathCmd::LineTo(pt(a0, 1.0)));
    }
    arc_cubics(&mut out, cx, cy, rx, ry, a0, sweep);
    if ir > 0.0 {
        out.push(PathCmd::LineTo(pt(a1, ir)));
        arc_cubics(&mut out, cx, cy, rx * ir, ry * ir, a1, -sweep);
    }
    out.push(PathCmd::Close);
    Some(out)
}

/// Points are given on the unit circle and land on the ellipse inscribed in the box, as the canvas draws them.
fn poly_cmds(w: f32, h: f32, pts: &[(f32, f32)]) -> Option<Vec<PathCmd>> {
    if pts.len() < 3 || w <= 0.0 || h <= 0.0 {
        return None;
    }
    let mut out: Vec<PathCmd> = pts
        .iter()
        .enumerate()
        .map(|(i, (x, y))| {
            let (px, py) = designos_render::inscribed_point(w, h, (*x, *y));
            let p = v(px, py);
            if i == 0 {
                PathCmd::MoveTo(p)
            } else {
                PathCmd::LineTo(p)
            }
        })
        .collect();
    out.push(PathCmd::Close);
    Some(out)
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

/// Whether every sub-path of `cmds` ends with `Close`.
fn all_closed(cmds: &[PathCmd]) -> bool {
    let mut open = false;
    for c in cmds {
        match c {
            PathCmd::MoveTo(_) => {
                if open {
                    return false;
                }
                open = true;
            }
            PathCmd::Close => open = false,
            _ => open = true,
        }
    }
    !open
}

fn from_doc_paths(paths: &[Path]) -> Geo {
    Geo {
        kind: GeoKind::Path,
        closed: paths.iter().all(|p| all_closed(&p.cmds)),
        paths: paths.iter().map(|p| (p.cmds.clone(), p.fill_rule)).collect(),
        fillable: true,
        is_rect: false,
        mask: None,
    }
}

/// Outline for a node, `None` when it has none of its own (groups, text,
/// pages, boolean operations — the writer handles those separately).
pub fn node_geo(n: &Node) -> Option<Geo> {
    if let Some(fg) = n.fill_geometry.as_deref().filter(|g| !g.is_empty()) {
        return Some(from_doc_paths(fg));
    }
    let (w, h) = (n.size.x, n.size.y);
    let single = |cmds: Vec<PathCmd>, kind: GeoKind, closed: bool, fillable: bool, is_rect: bool| Geo {
        kind,
        paths: vec![(cmds, FillRule::NonZero)],
        closed,
        fillable,
        is_rect,
        mask: None,
    };
    match &n.kind {
        NodeKind::Frame(_) | NodeKind::Component(_) | NodeKind::ComponentSet(_) | NodeKind::Instance { .. } | NodeKind::Section | NodeKind::Rectangle => {
            if w <= 0.0 || h <= 0.0 {
                return None;
            }
            let radii = if matches!(n.kind, NodeKind::Section) { [0.0; 4] } else { n.corner_radii };
            let lim = w.min(h) / 2.0;
            let r = radii.map(|v| v.max(0.0).min(lim));
            let uniform = r.iter().all(|x| (x - r[0]).abs() < 1e-4) && n.corner_smoothing <= 0.0;
            let kind = if uniform { GeoKind::Rect { w, h, r: r[0] } } else { GeoKind::Path };
            Some(single(rounded_rect(w, h, radii), kind, true, true, true))
        }
        NodeKind::Ellipse(arc) => {
            let full = (arc.end - arc.start).abs() >= TAU - 1e-4 && arc.inner_radius <= 0.0;
            let kind = if full { GeoKind::Ellipse { w, h } } else { GeoKind::Path };
            Some(single(ellipse_cmds(w, h, arc)?, kind, true, true, false))
        }
        NodeKind::Line => {
            let cmds = vec![PathCmd::MoveTo(v(0.0, 0.0)), PathCmd::LineTo(v(w, 0.0))];
            Some(single(cmds, GeoKind::Path, false, false, false))
        }
        NodeKind::Polygon { count } => Some(single(poly_cmds(w, h, &polygon_points(*count))?, GeoKind::Path, true, true, false)),
        NodeKind::Star { count, inner_ratio } => Some(single(poly_cmds(w, h, &star_points(*count, *inner_ratio))?, GeoKind::Path, true, true, false)),
        NodeKind::Vector { paths, .. } => {
            let g = from_doc_paths(paths);
            (!g.paths.is_empty()).then_some(g)
        }
        _ => None,
    }
}

/// Applies `t` to every point.
pub fn transform_cmds(cmds: &[PathCmd], t: &Affine) -> Vec<PathCmd> {
    cmds.iter()
        .map(|c| match *c {
            PathCmd::MoveTo(p) => PathCmd::MoveTo(t.apply(p)),
            PathCmd::LineTo(p) => PathCmd::LineTo(t.apply(p)),
            PathCmd::QuadTo(a, p) => PathCmd::QuadTo(t.apply(a), t.apply(p)),
            PathCmd::CubicTo(a, b, p) => PathCmd::CubicTo(t.apply(a), t.apply(b), t.apply(p)),
            PathCmd::Close => PathCmd::Close,
        })
        .collect()
}

/// SVG path data (`M..L..C..Z`).
pub fn path_d(cmds: &[PathCmd]) -> String {
    let mut s = String::new();
    let pt = |p: &Vec2| format!("{} {}", num(p.x), num(p.y));
    for c in cmds {
        if !s.is_empty() {
            s.push(' ');
        }
        match c {
            PathCmd::MoveTo(p) => s.push_str(&format!("M{}", pt(p))),
            PathCmd::LineTo(p) => s.push_str(&format!("L{}", pt(p))),
            PathCmd::QuadTo(a, p) => s.push_str(&format!("Q{} {}", pt(a), pt(p))),
            PathCmd::CubicTo(a, b, p) => s.push_str(&format!("C{} {} {}", pt(a), pt(b), pt(p))),
            PathCmd::Close => s.push('Z'),
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn square_rect_path() {
        let d = path_d(&rounded_rect(10.0, 5.0, [0.0; 4]));
        assert_eq!(d, "M0 0 L10 0 L10 5 L0 5 Z");
    }

    #[test]
    fn closed_detection() {
        assert!(all_closed(&rounded_rect(4.0, 4.0, [1.0; 4])));
        assert!(!all_closed(&[PathCmd::MoveTo(v(0.0, 0.0)), PathCmd::LineTo(v(1.0, 1.0))]));
    }
}
