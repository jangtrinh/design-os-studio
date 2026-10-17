//! Vector operations on document geometry: outline stroke, flatten, and the
//! region arrangement behind the shape builder. Everything here is pure
//! geometry (no pixmaps) so it can be tested by measuring areas.

use crate::boolean::flatten;
use crate::geom::{node_shape, vector_paths};
use designos_doc as d;
use designos_doc::vector::{fit_polygons, flatten_path, path_curves, point_in_polygon, polygon_area, VectorNetwork};
use designos_doc::{Affine, Document, Node, NodeId, NodeKind, Vec2};
use i_overlay::core::fill_rule::FillRule;
use i_overlay::core::overlay_rule::OverlayRule;
use i_overlay::float::simplify::SimplifyShape;
use i_overlay::float::single::SingleFloatOverlay;
use tiny_skia as sk;
use tiny_skia::PathSegment;

pub type Pt = [f64; 2];
/// Polygons with holes: shape -> contour -> point.
pub type Shapes = Vec<Vec<Vec<Pt>>>;

// ---------------------------------------------------------------- conversion
/// A tiny-skia path as a document path.
pub fn sk_to_doc(path: &sk::Path, rule: d::FillRule) -> d::Path {
    let mut cmds = Vec::new();
    let v = |p: sk::Point| Vec2::new(p.x, p.y);
    for seg in path.segments() {
        cmds.push(match seg {
            PathSegment::MoveTo(p) => d::PathCmd::MoveTo(v(p)),
            PathSegment::LineTo(p) => d::PathCmd::LineTo(v(p)),
            PathSegment::QuadTo(c, p) => d::PathCmd::QuadTo(v(c), v(p)),
            PathSegment::CubicTo(c1, c2, p) => d::PathCmd::CubicTo(v(c1), v(c2), v(p)),
            PathSegment::Close => d::PathCmd::Close,
        });
    }
    d::Path { cmds, fill_rule: rule }
}

fn rule_of(r: sk::FillRule) -> d::FillRule {
    match r {
        sk::FillRule::Winding => d::FillRule::NonZero,
        sk::FillRule::EvenOdd => d::FillRule::EvenOdd,
    }
}

/// Geometry of a shape node in its own coordinate space as document paths
/// (rectangles, ellipses, polygons, stars, lines, vectors, boolean groups).
/// Text and containers have none.
pub fn node_paths(doc: &Document, n: &Node) -> Option<Vec<d::Path>> {
    if let NodeKind::Vector { paths, network: Some(net) } = &n.kind {
        let p = net.to_paths();
        return Some(if p.is_empty() { paths.clone() } else { p });
    }
    if matches!(n.kind, NodeKind::BooleanOp(_)) {
        // polygons from the boolean engine, fitted back to curves
        let shape = node_shape(doc, n, 0.05)?;
        let mut contours = Vec::new();
        for (p, _) in &shape.paths {
            contours.extend(flatten(p, 0.05));
        }
        let pts: Vec<Vec<Vec2>> = contours.iter().map(|c| c.iter().map(|p| Vec2::new(p[0] as f32, p[1] as f32)).collect()).collect();
        let fitted = fit_polygons(&pts, d::FillRule::EvenOdd, 0.1);
        return (!fitted.cmds.is_empty()).then(|| vec![fitted]);
    }
    let shape = node_shape(doc, n, 0.05)?;
    Some(shape.paths.iter().map(|(p, r)| sk_to_doc(p, rule_of(*r))).collect())
}

fn contours_of(paths: &[d::Path], tol: f32) -> Vec<Vec<Pt>> {
    let mut out = Vec::new();
    for (p, _) in vector_paths(paths).paths {
        out.extend(flatten(&p, tol));
    }
    out
}

/// Normalised polygon shapes (outer contours with holes) of document paths.
pub fn paths_to_shapes(paths: &[d::Path], tol: f32) -> Shapes {
    let mut acc: Option<Shapes> = None;
    for p in paths {
        let contours = contours_of(std::slice::from_ref(p), tol);
        if contours.is_empty() {
            continue;
        }
        let rule = match p.fill_rule {
            d::FillRule::NonZero => FillRule::NonZero,
            d::FillRule::EvenOdd => FillRule::EvenOdd,
        };
        let s = contours.simplify_shape(rule);
        acc = Some(match acc {
            None => s,
            Some(a) => a.overlay(&s, OverlayRule::Union, FillRule::NonZero),
        });
    }
    acc.unwrap_or_default()
}

pub fn transform_shapes(shapes: &mut Shapes, a: &Affine) {
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

fn contour_area(c: &[Pt]) -> f64 {
    let mut a = 0.0;
    for i in 0..c.len() {
        let (p, q) = (c[i], c[(i + 1) % c.len()]);
        a += p[0] * q[1] - q[0] * p[1];
    }
    a * 0.5
}

/// Area of polygon shapes (holes subtract).
pub fn shapes_area(shapes: &Shapes) -> f64 {
    shapes.iter().map(|sh| sh.iter().enumerate().map(|(i, c)| if i == 0 { contour_area(c).abs() } else { -contour_area(c).abs() }).sum::<f64>()).sum()
}

/// Whether `p` is inside any of the shapes (holes excluded).
pub fn shapes_contain(shapes: &Shapes, p: Vec2) -> bool {
    shapes.iter().any(|sh| {
        let pts = |c: &Vec<Pt>| c.iter().map(|q| Vec2::new(q[0] as f32, q[1] as f32)).collect::<Vec<_>>();
        sh.iter().filter(|c| point_in_polygon(p, &pts(c))).count() % 2 == 1
    })
}

/// Document paths (curve-fitted) of polygon shapes.
pub fn shapes_to_paths(shapes: &Shapes, fit_tol: f32) -> Vec<d::Path> {
    let mut paths = Vec::new();
    for sh in shapes {
        let contours: Vec<Vec<Vec2>> = sh.iter().map(|c| c.iter().map(|p| Vec2::new(p[0] as f32, p[1] as f32)).collect()).collect();
        let p = fit_polygons(&contours, d::FillRule::EvenOdd, fit_tol);
        if !p.cmds.is_empty() {
            paths.push(p);
        }
    }
    paths
}

/// Area filled by document paths, by flattening and combining them (non-zero /
/// even-odd as each path says).
pub fn paths_area(paths: &[d::Path], tol: f32) -> f64 {
    shapes_area(&paths_to_shapes(paths, tol))
}

// ---------------------------------------------------------------- outline stroke
fn sk_stroke(s: &d::Stroke, width: f32, cap: d::StrokeCap) -> sk::Stroke {
    let mut st = sk::Stroke {
        width,
        miter_limit: s.miter_limit.max(1.0),
        line_cap: match cap {
            d::StrokeCap::None => sk::LineCap::Butt,
            d::StrokeCap::Round => sk::LineCap::Round,
            d::StrokeCap::Square => sk::LineCap::Square,
        },
        line_join: match s.join {
            d::StrokeJoin::Miter => sk::LineJoin::Miter,
            d::StrokeJoin::Bevel => sk::LineJoin::Bevel,
            d::StrokeJoin::Round => sk::LineJoin::Round,
        },
        dash: None,
    };
    if !s.dashes.is_empty() && s.dashes.iter().all(|x| *x >= 0.0) && s.dashes.iter().sum::<f32>() > 0.0 {
        let mut v = s.dashes.clone();
        if v.len() % 2 == 1 {
            v.extend_from_within(..);
        }
        st.dash = sk::StrokeDash::new(v, 0.0);
    }
    st
}

/// Expands one tiny-skia path by the stroke (dashes first).
fn expand(path: &sk::Path, st: &sk::Stroke) -> Option<sk::Path> {
    let p = match &st.dash {
        Some(dash) => path.dash(dash, 1.0)?,
        None => path.clone(),
    };
    p.stroke(st, 1.0)
}

/// Splits a document path into its sub-paths.
fn subpaths(p: &d::Path) -> Vec<(d::Path, bool)> {
    let mut out = vec![];
    let mut cur: Vec<d::PathCmd> = vec![];
    let mut closed = false;
    let flush = |cur: &mut Vec<d::PathCmd>, closed: &mut bool, out: &mut Vec<(d::Path, bool)>| {
        if cur.len() > 1 || (cur.len() == 1 && !matches!(cur[0], d::PathCmd::MoveTo(_))) {
            out.push((d::Path { cmds: std::mem::take(cur), fill_rule: p.fill_rule }, *closed));
        }
        cur.clear();
        *closed = false;
    };
    for c in &p.cmds {
        match c {
            d::PathCmd::MoveTo(_) => {
                flush(&mut cur, &mut closed, &mut out);
                cur.push(*c);
            }
            d::PathCmd::Close => {
                cur.push(*c);
                closed = true;
                flush(&mut cur, &mut closed, &mut out);
            }
            _ => cur.push(*c),
        }
    }
    flush(&mut cur, &mut closed, &mut out);
    out
}

fn circle_polygon(c: Vec2, r: f32) -> Vec<Pt> {
    let n = 48;
    (0..n)
        .map(|i| {
            let a = std::f32::consts::TAU * i as f32 / n as f32;
            [(c.x + r * a.cos()) as f64, (c.y + r * a.sin()) as f64]
        })
        .collect()
}

/// Cap shapes (circle or square) at the ends of open sub-paths for caps that
/// differ from what the stroker would draw.
fn cap_polygons(sub: &d::Path, w: f32, start_cap: d::StrokeCap, end_cap: d::StrokeCap) -> Vec<Vec<Pt>> {
    let curves = path_curves(sub);
    let Some((cs, _)) = curves.first() else { return vec![] };
    let (first, last) = (cs.first().unwrap(), cs.last().unwrap());
    let dir = |from: Vec2, cands: [Vec2; 3]| -> Vec2 {
        for c in cands {
            let v = Vec2::new(c.x - from.x, c.y - from.y);
            let l = (v.x * v.x + v.y * v.y).sqrt();
            if l > 1e-4 {
                return Vec2::new(v.x / l, v.y / l);
            }
        }
        Vec2::new(1.0, 0.0)
    };
    let mut out = vec![];
    let mut cap_at = |p: Vec2, outward: Vec2, cap: d::StrokeCap| match cap {
        d::StrokeCap::None => {}
        d::StrokeCap::Round => out.push(circle_polygon(p, w / 2.0)),
        d::StrokeCap::Square => {
            let n = Vec2::new(-outward.y, outward.x);
            let h = w / 2.0;
            let pt = |a: f32, b: f32| [(p.x + n.x * a + outward.x * b) as f64, (p.y + n.y * a + outward.y * b) as f64];
            out.push(vec![pt(-h, 0.0), pt(h, 0.0), pt(h, h), pt(-h, h)]);
        }
    };
    let s_out = dir(first[0], [first[1], first[2], first[3]]);
    let e_out = dir(last[3], [last[2], last[1], last[0]]);
    cap_at(first[0], Vec2::new(-s_out.x, -s_out.y), start_cap);
    cap_at(last[3], e_out, end_cap);
    out
}

/// Cap shapes to draw over a butt-capped stroke of a vector whose vertices
/// carry their own stroke caps; `None` when no vertex overrides the cap.
pub fn vertex_caps(n: &Node) -> Option<Vec<d::Path>> {
    let NodeKind::Vector { network: Some(net), .. } = &n.kind else { return None };
    if !net.is_valid() || !net.vertices.iter().any(|v| v.cap.is_some()) || n.stroke.weight <= 0.0 {
        return None;
    }
    let ends = net.open_chain_ends();
    let mut out = vec![];
    let mut idx = 0;
    for p in net.to_paths() {
        for (sub, closed) in subpaths(&p) {
            if closed {
                continue;
            }
            if let Some((a, b)) = ends.get(idx) {
                let (sc, ec) = (net.vertices[*a].cap.unwrap_or(n.stroke.cap), net.vertices[*b].cap.unwrap_or(n.stroke.cap));
                for poly in cap_polygons(&sub, n.stroke.weight, sc, ec) {
                    let pts: Vec<Vec2> = poly.iter().map(|q| Vec2::new(q[0] as f32, q[1] as f32)).collect();
                    let mut cmds = vec![d::PathCmd::MoveTo(pts[0])];
                    cmds.extend(pts[1..].iter().map(|q| d::PathCmd::LineTo(*q)));
                    cmds.push(d::PathCmd::Close);
                    out.push(d::Path { cmds, fill_rule: d::FillRule::NonZero });
                }
            }
            idx += 1;
        }
    }
    Some(out)
}

/// Converts the stroke of `paths` into filled geometry: weight, alignment,
/// caps, joins, miter limit and dashes are honoured. `network` supplies
/// per-vertex stroke caps (`paths` must then be `network.to_paths()`). The
/// result is filled with the non-zero (or, after clipping, even-odd) rule.
///
/// Centered strokes without per-vertex caps keep their curves exactly (the
/// stroker's own offset curves); inside/outside alignment and mixed caps go
/// through polygon clipping and are re-fitted to curves.
pub fn outline_stroke(paths: &[d::Path], stroke: &d::Stroke, network: Option<&VectorNetwork>) -> Vec<d::Path> {
    let w = stroke.weight;
    if w <= 0.0 || paths.is_empty() {
        return vec![];
    }
    let per_vertex_caps = network.is_some_and(|n| n.vertices.iter().any(|v| v.cap.is_some()));
    let chain_caps: Vec<(d::StrokeCap, d::StrokeCap)> = match network {
        Some(n) if per_vertex_caps => {
            n.open_chain_ends().iter().map(|(a, b)| (n.vertices[*a].cap.unwrap_or(stroke.cap), n.vertices[*b].cap.unwrap_or(stroke.cap))).collect()
        }
        _ => vec![],
    };
    let base_cap = if per_vertex_caps { d::StrokeCap::None } else { stroke.cap };
    let aligned = stroke.align != d::StrokeAlign::Center;
    let st_center = sk_stroke(stroke, w, base_cap);
    let st_double = sk_stroke(stroke, w * 2.0, base_cap);

    let mut out: Vec<d::Path> = vec![];
    let mut open_index = 0;
    for p in paths {
        let mut closed_subs: Vec<d::Path> = vec![];
        for (sub, closed) in subpaths(p) {
            let Some(skp) = vector_paths(std::slice::from_ref(&sub)).paths.into_iter().next().map(|x| x.0) else { continue };
            if closed {
                if aligned {
                    closed_subs.push(sub);
                } else if let Some(o) = expand(&skp, &st_center) {
                    out.push(sk_to_doc(&o, d::FillRule::NonZero));
                }
                continue;
            }
            let caps = if per_vertex_caps { chain_caps.get(open_index).copied() } else { None };
            open_index += 1;
            let Some(o) = expand(&skp, &st_center) else { continue };
            match caps {
                Some((sc, ec)) => {
                    let mut shapes: Shapes = flatten(&o, 0.05).simplify_shape(FillRule::NonZero);
                    let extra: Vec<Vec<Pt>> = cap_polygons(&sub, w, sc, ec);
                    if !extra.is_empty() {
                        shapes = shapes.overlay(&extra.simplify_shape(FillRule::NonZero), OverlayRule::Union, FillRule::NonZero);
                    }
                    out.extend(shapes_to_paths(&shapes, 0.08));
                }
                None => out.push(sk_to_doc(&o, d::FillRule::NonZero)),
            }
        }
        if !closed_subs.is_empty() {
            let fill = paths_to_shapes(&[d::Path { cmds: closed_subs.iter().flat_map(|c| c.cmds.clone()).collect(), fill_rule: p.fill_rule }], 0.05);
            let mut ring: Shapes = vec![];
            for c in &closed_subs {
                let Some(skp) = vector_paths(std::slice::from_ref(c)).paths.into_iter().next().map(|x| x.0) else { continue };
                let Some(o) = expand(&skp, &st_double) else { continue };
                let polys = flatten(&o, 0.05).simplify_shape(FillRule::NonZero);
                ring = if ring.is_empty() { polys } else { ring.overlay(&polys, OverlayRule::Union, FillRule::NonZero) };
            }
            let rule = if stroke.align == d::StrokeAlign::Inside { OverlayRule::Intersect } else { OverlayRule::Difference };
            let clipped = ring.overlay(&fill, rule, FillRule::NonZero);
            out.extend(shapes_to_paths(&clipped, 0.08));
        }
    }
    out
}

// ---------------------------------------------------------------- flatten
/// Result of flattening: geometry in the target space.
#[derive(Clone, Debug)]
pub struct Flattened {
    pub paths: Vec<d::Path>,
    /// Index (within the input ids) of the topmost node that contributed; its style is kept.
    pub style_from: usize,
}

fn collect_geometry(doc: &Document, id: NodeId, to_target: &Affine, out: &mut Vec<d::Path>) {
    let Some(n) = doc.get(id) else { return };
    if !n.visible {
        return;
    }
    let here = *to_target * n.transform;
    match &n.kind {
        NodeKind::Group => {
            for c in &n.children {
                collect_geometry(doc, *c, &here, out);
            }
        }
        NodeKind::Text(_) | NodeKind::Page { .. } | NodeKind::Unknown { .. } => {}
        k if k.is_container() && !matches!(k, NodeKind::BooleanOp(_)) => {
            // frames flatten their children, not their own box
            for c in &n.children {
                collect_geometry(doc, *c, &here, out);
            }
        }
        _ => {
            if let Some(paths) = node_paths(doc, n) {
                out.extend(paths.into_iter().map(|p| transform_path(&p, &here)));
            }
        }
    }
}

pub fn transform_path(p: &d::Path, a: &Affine) -> d::Path {
    let t = |v: Vec2| a.apply(v);
    d::Path {
        cmds: p
            .cmds
            .iter()
            .map(|c| match *c {
                d::PathCmd::MoveTo(v) => d::PathCmd::MoveTo(t(v)),
                d::PathCmd::LineTo(v) => d::PathCmd::LineTo(t(v)),
                d::PathCmd::QuadTo(c1, v) => d::PathCmd::QuadTo(t(c1), t(v)),
                d::PathCmd::CubicTo(c1, c2, v) => d::PathCmd::CubicTo(t(c1), t(c2), t(v)),
                d::PathCmd::Close => d::PathCmd::Close,
            })
            .collect(),
        fill_rule: p.fill_rule,
    }
}

/// Merges the geometry of `ids` into one list of paths in the space given by
/// `target_world` (the world transform of the parent the result will live in).
/// Boolean groups contribute their computed result. Returns `None` when
/// nothing has geometry.
pub fn flatten_nodes(doc: &Document, ids: &[NodeId], target_world: &Affine) -> Option<Flattened> {
    let inv = target_world.invert()?;
    let mut paths = vec![];
    let mut style_from = 0;
    for (i, id) in ids.iter().enumerate() {
        let Some(n) = doc.get(id.to_owned()) else { continue };
        let parent_world = n.parent.map(|p| doc.world_transform(p)).unwrap_or(Affine::IDENTITY);
        let before = paths.len();
        collect_geometry_rooted(doc, *id, &(inv * parent_world), &mut paths);
        if paths.len() > before {
            style_from = i;
        }
    }
    (!paths.is_empty()).then_some(Flattened { paths, style_from })
}

fn collect_geometry_rooted(doc: &Document, id: NodeId, to_target: &Affine, out: &mut Vec<d::Path>) {
    collect_geometry(doc, id, to_target, out);
}

// ---------------------------------------------------------------- shape builder
/// One region of the arrangement of overlapping shapes.
#[derive(Clone, Debug)]
pub struct Cell {
    /// The region as polygon shapes (a single polygon with its holes).
    pub shape: Shapes,
    /// Indices of the input shapes that cover this region.
    pub members: Vec<usize>,
    pub area: f64,
}

/// Splits overlapping shapes into the regions their outlines cut out: for two
/// overlapping circles, two crescents and the lens.
pub fn arrange(shapes: &[Shapes]) -> Vec<Cell> {
    let mut cells: Vec<(Shapes, Vec<usize>)> = vec![];
    for (i, s) in shapes.iter().enumerate() {
        if s.is_empty() {
            continue;
        }
        let mut next: Vec<(Shapes, Vec<usize>)> = vec![];
        let mut rest: Shapes = s.clone();
        for (g, mem) in &cells {
            let inter = g.overlay(s, OverlayRule::Intersect, FillRule::NonZero);
            let diff = g.overlay(s, OverlayRule::Difference, FillRule::NonZero);
            if shapes_area(&inter) > 1e-3 {
                let mut m = mem.clone();
                m.push(i);
                next.push((inter, m));
            }
            if shapes_area(&diff) > 1e-3 {
                next.push((diff, mem.clone()));
            }
            if !rest.is_empty() {
                rest = rest.overlay(g, OverlayRule::Difference, FillRule::NonZero);
            }
        }
        if shapes_area(&rest) > 1e-3 {
            next.push((rest, vec![i]));
        }
        cells = next;
    }
    let mut out = vec![];
    for (g, mem) in cells {
        for poly in g {
            let area = shapes_area(&vec![poly.clone()]);
            if area > 1e-3 {
                out.push(Cell { shape: vec![poly], members: mem.clone(), area });
            }
        }
    }
    out
}

/// What the shape builder does to the original shapes.
#[derive(Clone, Debug)]
pub struct BuilderResult {
    /// New shapes (merged groups of cells) with the index of the original whose style they take.
    pub pieces: Vec<(Shapes, usize)>,
    /// Per original: `None` when untouched, `Some(rest)` with what remains of it (maybe empty).
    pub remainders: Vec<Option<Shapes>>,
}

/// Resolves the builder state. `group[i]` is the merge group of cell `i`
/// (cells with the same group merge), `deleted[i]` removes the cell and
/// `touched[i]` marks cells the user acted on. Cells that are neither
/// merged nor deleted stay part of their original shapes.
pub fn resolve_builder(originals: &[Shapes], cells: &[Cell], group: &[Option<usize>], deleted: &[bool]) -> BuilderResult {
    let mut pieces: Vec<(Shapes, usize)> = vec![];
    let mut groups: Vec<usize> = group.iter().flatten().copied().collect();
    groups.sort_unstable();
    groups.dedup();
    let mut consumed: Vec<Shapes> = vec![vec![]; originals.len()];
    let consume = |consumed: &mut Vec<Shapes>, cell: &Cell| {
        for &m in &cell.members {
            consumed[m] = if consumed[m].is_empty() { cell.shape.clone() } else { consumed[m].overlay(&cell.shape, OverlayRule::Union, FillRule::NonZero) };
        }
    };
    let mut touched_orig = vec![false; originals.len()];
    for (i, c) in cells.iter().enumerate() {
        if deleted.get(i).copied().unwrap_or(false) || group.get(i).copied().flatten().is_some() {
            consume(&mut consumed, c);
            for &m in &c.members {
                touched_orig[m] = true;
            }
        }
    }
    for g in groups {
        let mut acc: Shapes = vec![];
        let mut style = 0;
        for (i, c) in cells.iter().enumerate() {
            if group[i] == Some(g) && !deleted[i] {
                acc = if acc.is_empty() { c.shape.clone() } else { acc.overlay(&c.shape, OverlayRule::Union, FillRule::NonZero) };
                style = style.max(c.members.iter().copied().max().unwrap_or(0));
            }
        }
        if !acc.is_empty() {
            pieces.push((acc, style));
        }
    }
    let remainders = originals
        .iter()
        .enumerate()
        .map(|(i, o)| {
            if !touched_orig[i] {
                None
            } else if consumed[i].is_empty() {
                Some(o.clone())
            } else {
                Some(o.overlay(&consumed[i], OverlayRule::Difference, FillRule::NonZero))
            }
        })
        .collect();
    BuilderResult { pieces, remainders }
}

/// Signed area of a flattened document path set; handy for tests.
pub fn flattened_area(paths: &[d::Path]) -> f32 {
    paths.iter().flat_map(|p| flatten_path(p, 0.01)).filter(|(_, c)| *c).map(|(pts, _)| polygon_area(&pts)).sum()
}

/// Offsets the closed contours of `paths` outward (`delta > 0.0`) or inward (`delta < 0.0`)
/// using Figma's default round join semantics. Open sub-paths pass through unchanged.
pub fn offset_path(paths: &[d::Path], delta: f32) -> Vec<d::Path> {
    offset_paths(paths, delta, d::StrokeJoin::Round, 4.0)
}

/// `(closed, open)` sub-path counts of `paths`.
pub fn count_subpaths(paths: &[d::Path]) -> (usize, usize) {
    paths.iter().flat_map(subpaths).fold((0, 0), |(closed, open), (_, is_closed)| if is_closed { (closed + 1, open) } else { (closed, open + 1) })
}

/// Offsets the closed contours of `paths` outward (`delta > 0.0`) or inward (`delta < 0.0`)
/// with explicit corner join (`Round`, `Miter`, `Bevel`) and miter limit.
///
/// An open sub-path has no inside to grow or shrink, and a parallel curve of it is not
/// what a centred stroke gives (that is a ribbon around it), so open sub-paths are carried
/// into the result exactly as they were. [`count_subpaths`] tells a caller whether there are any.
///
/// Each path is its own region and is taken as it paints: the closed contours inside one path
/// merge into one silhouette and only its outer boundary and its holes move, while separate paths
/// stay separate regions. When an inset removes all of the closed part the result is empty, open
/// sub-paths included, so a caller can leave the original as it was.
pub fn offset_paths(paths: &[d::Path], delta: f32, join: d::StrokeJoin, miter_limit: f32) -> Vec<d::Path> {
    if paths.is_empty() || !delta.is_finite() {
        return Vec::new();
    }
    if delta.abs() < 1e-5 {
        return paths.to_vec();
    }
    let stroke = d::Stroke {
        weight: delta.abs() * 2.0,
        align: d::StrokeAlign::Center,
        cap: d::StrokeCap::Round,
        join,
        miter_limit: miter_limit.max(1.0),
        ..Default::default()
    };
    let st = sk_stroke(&stroke, delta.abs() * 2.0, d::StrokeCap::Round);

    let mut open: Vec<d::Path> = Vec::new();
    let mut closed_paths: Vec<d::Path> = Vec::new();
    for p in paths {
        let mut closed_cmds: Vec<d::PathCmd> = Vec::new();
        for (sub, closed) in subpaths(p) {
            if closed {
                closed_cmds.extend(sub.cmds);
            } else {
                open.push(sub);
            }
        }
        if !closed_cmds.is_empty() {
            closed_paths.push(d::Path { cmds: closed_cmds, fill_rule: p.fill_rule });
        }
    }
    if closed_paths.is_empty() {
        return open;
    }

    // each path is a region of its own: its fill as it paints (overlaps inside the path merged,
    // holes kept), moved by a band along its boundary only, so an edge inside the fill is never
    // carved out of it
    let mut moved: Shapes = Vec::new();
    for path in &closed_paths {
        let fill = paths_to_shapes(std::slice::from_ref(path), 0.02);
        let band = boundary_band(&fill, &st);
        let region = match (delta > 0.0, band.is_empty()) {
            (true, true) => fill,
            (true, false) => fill.overlay(&band, OverlayRule::Union, FillRule::NonZero),
            (false, true) => Vec::new(),
            (false, false) => fill.overlay(&band, OverlayRule::Difference, FillRule::NonZero),
        };
        if !region.is_empty() && shapes_area(&region) > 1e-3 {
            moved = if moved.is_empty() { region } else { moved.overlay(&region, OverlayRule::Union, FillRule::NonZero) };
        }
    }
    if moved.is_empty() {
        // every closed part is gone: the caller keeps the original, with its open sub-paths
        return Vec::new();
    }
    open.extend(shapes_to_paths(&moved, 0.05));
    open
}

/// The stroke band `st` laid along every contour (outer boundaries and holes) of `shapes`.
fn boundary_band(shapes: &Shapes, st: &sk::Stroke) -> Shapes {
    let mut band: Shapes = Vec::new();
    for contour in shapes.iter().flatten() {
        let mut pb = sk::PathBuilder::new();
        for (i, p) in contour.iter().enumerate() {
            let (x, y) = (p[0] as f32, p[1] as f32);
            if i == 0 {
                pb.move_to(x, y);
            } else {
                pb.line_to(x, y);
            }
        }
        pb.close();
        let Some(outline) = pb.finish().and_then(|path| expand(&path, st)) else { continue };
        let polys = flatten(&outline, 0.02).simplify_shape(FillRule::NonZero);
        band = if band.is_empty() { polys } else { band.overlay(&polys, OverlayRule::Union, FillRule::NonZero) };
    }
    band
}
