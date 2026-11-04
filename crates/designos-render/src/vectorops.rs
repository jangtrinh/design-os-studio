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
use rayon::prelude::*;
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

/// Outlined geometry of one paint set (for Flatten) or one glyph (for Outline stroke)
/// of a text node, in node-local coordinates.
#[derive(Clone, Debug)]
pub struct TextVectorRun {
    pub name: String,
    pub path: d::Path,
    pub fills: Vec<d::Paint>,
    /// Whether this run uses the text node's base fill (so node-level `fills/*` bindings apply).
    pub uses_base_fill: bool,
}

/// Outlines a text node for Flatten: one [`TextVectorRun`] per distinct fill paint set across the
/// shaped runs, in node-local coordinates (`FillRule::NonZero`).
pub fn text_flatten_runs(doc: &Document, n: &Node) -> Vec<TextVectorRun> {
    let resolved = d::resolve_node(doc, n);
    let NodeKind::Text(tp) = &resolved.kind else {
        return Vec::new();
    };
    let tl = crate::text::layout(tp, resolved.size);
    let mut out: Vec<TextVectorRun> = Vec::new();
    for (style_idx, (opt_path, style)) in tl.paths.iter().zip(tl.styles.iter()).enumerate() {
        let Some(sk_path) = opt_path.as_ref() else { continue };
        let d_path = sk_to_doc(sk_path, d::FillRule::NonZero);
        if d_path.cmds.is_empty() {
            continue;
        }
        let uses_base = style_idx == 0 || style.fills == tp.style.fills;
        if let Some(existing) = out.iter_mut().find(|r| r.fills == style.fills) {
            existing.path.cmds.extend(d_path.cmds);
            existing.uses_base_fill |= uses_base;
        } else {
            out.push(TextVectorRun { name: n.name.clone(), path: d_path, fills: style.fills.clone(), uses_base_fill: uses_base });
        }
    }
    out
}

/// Outlines a text node for Flatten when its stroke is `Inside` or `Outside`: one [`TextVectorRun`] per
/// style of the shaped text (the base style and one per text run), never merged by fill. The canvas
/// clips such a stroke to the outline of each style on its own, so a vector per style keeps that clip.
pub fn text_style_runs(doc: &Document, n: &Node) -> Vec<TextVectorRun> {
    let resolved = d::resolve_node(doc, n);
    let NodeKind::Text(tp) = &resolved.kind else {
        return Vec::new();
    };
    let tl = crate::text::layout(tp, resolved.size);
    let mut out = Vec::new();
    for (style_idx, (opt_path, style)) in tl.paths.iter().zip(tl.styles.iter()).enumerate() {
        let Some(sk_path) = opt_path.as_ref() else { continue };
        let d_path = sk_to_doc(sk_path, d::FillRule::NonZero);
        if d_path.cmds.is_empty() {
            continue;
        }
        let uses_base = style_idx == 0 || style.fills == tp.style.fills;
        out.push(TextVectorRun { name: n.name.clone(), path: d_path, fills: style.fills.clone(), uses_base_fill: uses_base });
    }
    out
}

/// A shaped glyph (or list marker, underline or strikethrough bar) with its outline, its style index and its run.
struct GlyphOutline {
    style: usize,
    path: sk::Path,
    run: TextVectorRun,
}

fn glyph_outlines_of(tp: &d::TextProps, size: Vec2) -> Vec<GlyphOutline> {
    let go = crate::text::glyph_outlines(tp, size);
    let mut out = Vec::with_capacity(go.glyphs.len());
    for g in go.glyphs {
        let d_path = sk_to_doc(&g.path, d::FillRule::NonZero);
        if d_path.cmds.is_empty() {
            continue;
        }
        let fills = go.styles.get(g.style).map(|s| s.fills.clone()).unwrap_or_default();
        let uses_base = g.style == 0 || fills == tp.style.fills;
        let name = if g.text.trim().is_empty() { "Vector".to_string() } else { g.text };
        out.push(GlyphOutline { style: g.style, path: g.path, run: TextVectorRun { name, path: d_path, fills, uses_base_fill: uses_base } });
    }
    out
}

/// Outlines a text node for Outline stroke: one [`TextVectorRun`] per shaped glyph (plus any list
/// markers and underline / strikethrough bars), in node-local coordinates (`FillRule::NonZero`).
pub fn text_glyph_runs(doc: &Document, n: &Node) -> Vec<TextVectorRun> {
    let resolved = d::resolve_node(doc, n);
    let NodeKind::Text(tp) = &resolved.kind else {
        return Vec::new();
    };
    glyph_outlines_of(tp, resolved.size).into_iter().map(|g| g.run).collect()
}

/// One layer of a text node whose `Inside` or `Outside` stroke is baked, in paint order.
#[derive(Clone, Debug)]
pub enum AlignedTextLayer {
    /// The fill geometry of one glyph (or list marker, underline or strikethrough bar).
    Fill(TextVectorRun),
    /// The stroke of one glyph or of several whose strokes touch, as fill geometry, in node-local coordinates (`FillRule::EvenOdd`).
    Stroke { name: String, paths: Vec<d::Path> },
}

/// Stroker precision, flattening tolerance and curve-fit tolerance of a baked text stroke, the longest polygon
/// edge handed to the fit, and the mean width under which a hole stays a line path. All in node units.
const TEXT_STROKE_RES_SCALE: f32 = 4.0;
const TEXT_STROKE_FLAT_TOL: f32 = 0.02;
const TEXT_STROKE_FIT_TOL: f32 = 0.04;
const TEXT_STROKE_FIT_GAP: f64 = 0.25;
const TEXT_STROKE_THIN_HOLE: f64 = 1.0;
/// The most glyphs one stroke vector spans. A thick stroke makes the rings of a whole paragraph touch, and one
/// vector for all of it would cost what a single baked run did.
const TEXT_STROKE_MAX_GLYPHS: usize = 128;

type Bounds = [f64; 4];

fn contours_bounds(shapes: &Shapes) -> Option<Bounds> {
    let mut pts = shapes.iter().flatten().flatten();
    let first = pts.next()?;
    let mut b = [first[0], first[1], first[0], first[1]];
    for p in pts {
        b = [b[0].min(p[0]), b[1].min(p[1]), b[2].max(p[0]), b[3].max(p[1])];
    }
    Some(b)
}

fn bounds_overlap(a: &Bounds, b: &Bounds) -> bool {
    a[0] <= b[2] && b[0] <= a[2] && a[1] <= b[3] && b[1] <= a[3]
}

/// Uniform grid over glyph bounds: finds the glyphs near a stroke ring without a pass over all of them.
struct BoundsGrid {
    cell: f64,
    cells: std::collections::HashMap<(i32, i32), Vec<u32>>,
}

impl BoundsGrid {
    fn new(bounds: &[Option<Bounds>]) -> Self {
        let mut extents: Vec<f64> = bounds.iter().flatten().map(|b| (b[2] - b[0]).max(b[3] - b[1])).collect();
        extents.sort_by(f64::total_cmp);
        let cell = extents.get(extents.len() / 2).copied().unwrap_or(1.0).max(1.0);
        let mut grid = BoundsGrid { cell, cells: Default::default() };
        for (i, b) in bounds.iter().enumerate() {
            let Some(b) = b else { continue };
            let (x0, y0, x1, y1) = grid.span(b);
            for cy in y0..=y1 {
                for cx in x0..=x1 {
                    grid.cells.entry((cx, cy)).or_default().push(i as u32);
                }
            }
        }
        grid
    }

    fn span(&self, b: &Bounds) -> (i32, i32, i32, i32) {
        let c = |v: f64| (v / self.cell).floor() as i32;
        (c(b[0]), c(b[1]), c(b[2]), c(b[3]))
    }

    /// Indices whose bounds share a cell with `b`, each once.
    fn near(&self, b: &Bounds) -> Vec<u32> {
        let (x0, y0, x1, y1) = self.span(b);
        let mut out = Vec::new();
        for cy in y0..=y1 {
            for cx in x0..=x1 {
                if let Some(v) = self.cells.get(&(cx, cy)) {
                    out.extend_from_slice(v);
                }
            }
        }
        out.sort_unstable();
        out.dedup();
        out
    }
}

fn find_root(parent: &mut [u32], mut i: usize) -> usize {
    while parent[i] as usize != i {
        parent[i] = parent[parent[i] as usize];
        i = parent[i] as usize;
    }
    i
}

/// A name for the stroke vector of several glyphs: the text they spell, without decoration labels.
fn stroke_layer_name(glyphs: &[GlyphOutline], members: &[usize]) -> String {
    const MAX_CHARS: usize = 24;
    let mut spelled = String::new();
    for &i in members {
        let name = glyphs[i].run.name.as_str();
        if !matches!(name, "Vector" | "Underline" | "Strikethrough") {
            spelled.push_str(name);
        }
    }
    let spelled = spelled.trim();
    let label = if spelled.is_empty() {
        "Vector".to_string()
    } else if spelled.chars().count() > MAX_CHARS {
        format!("{}...", spelled.chars().take(MAX_CHARS).collect::<String>())
    } else {
        spelled.to_string()
    };
    format!("{label} stroke")
}

/// Outlines a text node for Outline stroke when its stroke is `Inside` or `Outside`: the fill of every glyph (and
/// list marker, underline or strikethrough bar) and the stroke as fill geometry, in the order the canvas paints
/// them: the fills of one style, then the strokes of that style, style by style.
///
/// The canvas cuts the stroke of a style against the union of the outlines of that style. The baked stroke is cut
/// the same way, so it renders as the canvas does. Glyphs whose stroke rings touch share one stroke vector (a word,
/// as a rule): two vectors over the same edge would both be anti-aliased there and show a stronger edge than the
/// canvas draws. Glyphs apart from the others get a vector of their own. A thick stroke can chain the rings of a
/// whole paragraph together; such a chain is cut into vectors of at most [`TEXT_STROKE_MAX_GLYPHS`] glyphs, each
/// still cut against the glyphs of its neighbours, so every vector stays small. The rings of the two glyphs on
/// either side of a cut overlap outside every fill and both vectors paint that overlap, so a translucent
/// `Outside` stroke shows a darker band there. Leaving the overlap to the earlier vector only would trade the band
/// for an anti-aliased seam through the stroke, which an opaque paint shows too. A style without a fill emits no
/// fill layer; its strokes remain. Empty for a `Center` stroke, no weight, or a non-text node.
pub fn text_aligned_stroke_layers(doc: &Document, n: &Node) -> Vec<AlignedTextLayer> {
    let resolved = d::resolve_node(doc, n);
    let NodeKind::Text(tp) = &resolved.kind else {
        return Vec::new();
    };
    let stroke = &resolved.stroke;
    if stroke.weight <= 0.0 || stroke.align == d::StrokeAlign::Center {
        return Vec::new();
    }
    let glyphs = glyph_outlines_of(tp, resolved.size);
    let st = crate::outline::node_stroke_style(&resolved, stroke.weight * 2.0);
    let rule = if stroke.align == d::StrokeAlign::Inside { OverlayRule::Intersect } else { OverlayRule::Difference };
    // the filled polygon and the stroke ring of every glyph
    let geometry: Vec<(Shapes, Shapes)> = glyphs
        .par_iter()
        .map(|g| {
            let fill = flatten(&g.path, TEXT_STROKE_FLAT_TOL).simplify_shape(FillRule::NonZero);
            let ring = match crate::outline::stroke_outline(&g.path, &st, TEXT_STROKE_RES_SCALE) {
                Some(o) => flatten(&o, TEXT_STROKE_FLAT_TOL).simplify_shape(FillRule::NonZero),
                None => Vec::new(),
            };
            (fill, ring)
        })
        .collect();
    let ring_bounds: Vec<Option<Bounds>> = geometry.iter().map(|(_, ring)| contours_bounds(ring)).collect();
    let fill_bounds: Vec<Option<Bounds>> = geometry.iter().map(|(fill, _)| contours_bounds(fill)).collect();
    // glyphs of one style whose rings touch go together
    let ring_grid = BoundsGrid::new(&ring_bounds);
    let mut parent: Vec<u32> = (0..glyphs.len() as u32).collect();
    for (i, b) in ring_bounds.iter().enumerate() {
        let Some(b) = b else { continue };
        for j in ring_grid.near(b) {
            let j = j as usize;
            if j > i && glyphs[j].style == glyphs[i].style && ring_bounds[j].is_some_and(|o| bounds_overlap(b, &o)) {
                let (ri, rj) = (find_root(&mut parent, i), find_root(&mut parent, j));
                parent[ri.max(rj)] = ri.min(rj) as u32;
            }
        }
    }
    // the glyphs of every group in glyph order; a group too large for one vector is cut into runs of glyphs
    let mut groups: std::collections::BTreeMap<usize, Vec<usize>> = Default::default();
    for (i, b) in ring_bounds.iter().enumerate() {
        if b.is_some() {
            groups.entry(find_root(&mut parent, i)).or_default().push(i);
        }
    }
    let clusters: Vec<(&[usize], bool)> = groups.values().flat_map(|g| g.chunks(TEXT_STROKE_MAX_GLYPHS).map(move |c| (c, c.len() < g.len()))).collect();
    let fill_grid = BoundsGrid::new(&fill_bounds);
    let union_bounds = |members: &[usize], of: &[Option<Bounds>]| -> Bounds {
        members
            .iter()
            .filter_map(|&i| of[i])
            .fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |u, b| [u[0].min(b[0]), u[1].min(b[1]), u[2].max(b[2]), u[3].max(b[3])])
    };
    // the glyphs of `members`' style, not among them, whose bounds in `of` overlap `reach`
    let others = |members: &[usize], grid: &BoundsGrid, of: &[Option<Bounds>], reach: &Bounds| -> Vec<usize> {
        let style = glyphs[members[0]].style;
        let near = grid.near(reach).into_iter().map(|j| j as usize);
        near.filter(|j| glyphs[*j].style == style && members.binary_search(j).is_err() && of[*j].is_some_and(|b| bounds_overlap(&b, reach))).collect()
    };
    // the stroke of a cluster: its glyphs' rings joined, cut against the union of their outlines. Glyphs beyond the
    // cluster matter only when it is a run cut from a larger group: an Inside stroke takes in the rings of those that
    // reach into the cluster's outlines, an Outside stroke is cut against the outlines of those that its rings reach.
    let cluster_strokes: Vec<Option<(String, Vec<d::Path>)>> = clusters
        .par_iter()
        .map(|&(members, partial)| {
            let mut rings: Vec<&Shapes> = members.iter().map(|&i| &geometry[i].1).collect();
            let mut fills: Vec<&Shapes> = members.iter().map(|&i| &geometry[i].0).collect();
            if partial && stroke.align == d::StrokeAlign::Inside {
                let reach = union_bounds(members, &fill_bounds);
                rings.extend(others(members, &ring_grid, &ring_bounds, &reach).into_iter().map(|j| &geometry[j].1));
            } else if partial {
                let reach = union_bounds(members, &ring_bounds);
                fills.extend(others(members, &fill_grid, &fill_bounds, &reach).into_iter().map(|j| &geometry[j].0));
            }
            let joined: Shapes;
            let ring: &Shapes = if let [only] = rings.as_slice() {
                only
            } else {
                joined = rings.iter().flat_map(|r| r.iter().flatten().cloned()).collect::<Vec<_>>().simplify_shape(FillRule::NonZero);
                &joined
            };
            let clip: Shapes = fills.iter().flat_map(|f| f.iter().cloned()).collect();
            let clipped = ring.overlay(&clip, rule, FillRule::NonZero);
            let paths = text_stroke_shapes_to_paths(&clipped, TEXT_STROKE_FIT_TOL, TEXT_STROKE_FIT_GAP, TEXT_STROKE_THIN_HOLE);
            (!paths.is_empty()).then(|| (stroke_layer_name(&glyphs, members), paths))
        })
        .collect();
    let mut strokes: Vec<Option<(String, Vec<d::Path>)>> = vec![None; glyphs.len()];
    for ((members, _), baked) in clusters.iter().zip(cluster_strokes) {
        strokes[members[0]] = baked;
    }
    let mut order: Vec<usize> = (0..glyphs.len()).collect();
    order.sort_by_key(|&i| glyphs[i].style);
    let mut layers = Vec::with_capacity(glyphs.len() * 2);
    let mut start = 0;
    while start < order.len() {
        let style = glyphs[order[start]].style;
        let end = start + order[start..].iter().take_while(|&&i| glyphs[i].style == style).count();
        for &i in &order[start..end] {
            if !glyphs[i].run.fills.is_empty() {
                layers.push(AlignedTextLayer::Fill(glyphs[i].run.clone()));
            }
        }
        for &i in &order[start..end] {
            if let Some((name, paths)) = strokes[i].take() {
                layers.push(AlignedTextLayer::Stroke { name, paths });
            }
        }
        start = end;
    }
    layers
}

/// Geometry of a shape or text node in its own coordinate space as document paths
/// (rectangles, ellipses, polygons, stars, lines, vectors, boolean groups, text).
/// Containers have none.
pub fn node_paths(doc: &Document, n: &Node) -> Option<Vec<d::Path>> {
    if matches!(n.kind, NodeKind::Text(_)) {
        let runs = text_flatten_runs(doc, n);
        let mut cmds = Vec::new();
        for r in runs {
            cmds.extend(r.path.cmds);
        }
        return (!cmds.is_empty()).then(|| vec![d::Path { cmds, fill_rule: d::FillRule::NonZero }]);
    }
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

/// One contour as a closed line path.
fn contour_to_lines(c: &[Pt], cmds: &mut Vec<d::PathCmd>) {
    let Some(first) = c.first() else { return };
    cmds.push(d::PathCmd::MoveTo(Vec2::new(first[0] as f32, first[1] as f32)));
    for p in &c[1..] {
        cmds.push(d::PathCmd::LineTo(Vec2::new(p[0] as f32, p[1] as f32)));
    }
    cmds.push(d::PathCmd::Close);
}

/// [`shapes_to_paths`] for the polygons of a baked text stroke. A hole narrower than `thin_hole` on average (a
/// hairline slit left between two strokes) stays a line path: a curve fit moves its two sides by a share of its
/// width that shows. Every other contour has its edges cut into pieces of at most `max_gap` first. The fit gauges
/// its error at the polygon's vertices, so a long straight edge between two far vertices can come out bent (0.2
/// units on a glyph stem, measured).
fn text_stroke_shapes_to_paths(shapes: &Shapes, fit_tol: f32, max_gap: f64, thin_hole: f64) -> Vec<d::Path> {
    let mut paths = Vec::new();
    for sh in shapes {
        let mut cmds: Vec<d::PathCmd> = Vec::new();
        let mut fitted: Vec<Vec<Vec2>> = Vec::new();
        for (ci, c) in sh.iter().enumerate() {
            let perimeter: f64 = (0..c.len()).map(|i| (c[(i + 1) % c.len()][0] - c[i][0]).hypot(c[(i + 1) % c.len()][1] - c[i][1])).sum();
            if perimeter <= 0.0 {
                continue;
            }
            if ci > 0 && 2.0 * contour_area(c).abs() / perimeter < thin_hole {
                contour_to_lines(c, &mut cmds);
                continue;
            }
            let mut dense = Vec::with_capacity(c.len());
            for i in 0..c.len() {
                let (p, q) = (c[i], c[(i + 1) % c.len()]);
                dense.push(Vec2::new(p[0] as f32, p[1] as f32));
                let pieces = ((q[0] - p[0]).hypot(q[1] - p[1]) / max_gap).ceil() as usize;
                for k in 1..pieces {
                    let t = k as f64 / pieces as f64;
                    dense.push(Vec2::new((p[0] + (q[0] - p[0]) * t) as f32, (p[1] + (q[1] - p[1]) * t) as f32));
                }
            }
            fitted.push(dense);
        }
        cmds.extend(fit_polygons(&fitted, d::FillRule::EvenOdd, fit_tol).cmds);
        if !cmds.is_empty() {
            paths.push(d::Path { cmds, fill_rule: d::FillRule::EvenOdd });
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
    sk_stroke_with_join(s, width, cap, s.join)
}

fn sk_stroke_with_join(s: &d::Stroke, width: f32, cap: d::StrokeCap, join: d::StrokeJoin) -> sk::Stroke {
    let mut st = sk::Stroke {
        width,
        miter_limit: s.miter_limit.max(1.0),
        line_cap: match cap {
            d::StrokeCap::None
            | d::StrokeCap::ArrowLines
            | d::StrokeCap::ArrowEquilateral
            | d::StrokeCap::TriangleFilled
            | d::StrokeCap::DiamondFilled
            | d::StrokeCap::CircleFilled => sk::LineCap::Butt,
            d::StrokeCap::Round => sk::LineCap::Round,
            d::StrokeCap::Square => sk::LineCap::Square,
        },
        line_join: match join {
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
    crate::outline::stroke_outline(path, st, 1.0)
}

/// A straight segment is stored as the cubic `[p, p, q, q]` (see `path_curves`).
fn is_straight(c: &[Vec2; 4]) -> bool {
    c[1] == c[0] && c[2] == c[3]
}

/// Splits cubic `[c[0], c[1], c[2], c[3]]` at `t` in `0.0..=1.0`, preserving straight segments. On a curve `t` is the
/// Bézier parameter; on a straight segment it is the fraction of the segment's length (see [`cubic_arc_len_and_t`]).
fn split_cubic_at(c: &[Vec2; 4], t: f32) -> ([Vec2; 4], [Vec2; 4]) {
    let lerp = |a: Vec2, b: Vec2| Vec2::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t);
    if is_straight(c) {
        let m = lerp(c[0], c[3]);
        return ([c[0], c[0], m, m], [m, m, c[3], c[3]]);
    }
    let m01 = lerp(c[0], c[1]);
    let m12 = lerp(c[1], c[2]);
    let m23 = lerp(c[2], c[3]);
    let m012 = lerp(m01, m12);
    let m123 = lerp(m12, m23);
    let m0123 = lerp(m012, m123);
    ([c[0], m01, m012, m0123], [m0123, m123, m23, c[3]])
}

/// The two halves of `c`. The halves of a straight segment are the straight segments `[a, a, m, m]` and `[m, m, b, b]`.
/// Split like a curve they would be `[a, a, a + (b - a) / 4, m]`, a collinear cubic that the stroker can take for a
/// degenerate curve (when the corner lies on an axis) and join to the previous piece with a round join, whatever join
/// was asked for.
fn split_cubic_half(c: &[Vec2; 4]) -> ([Vec2; 4], [Vec2; 4]) {
    split_cubic_at(c, 0.5)
}

fn cubic_eval(c: &[Vec2; 4], t: f32) -> Vec2 {
    let u = 1.0 - t;
    let b0 = u * u * u;
    let b1 = 3.0 * u * u * t;
    let b2 = 3.0 * u * t * t;
    let b3 = t * t * t;
    Vec2::new(b0 * c[0].x + b1 * c[1].x + b2 * c[2].x + b3 * c[3].x, b0 * c[0].y + b1 * c[1].y + b2 * c[2].y + b3 * c[3].y)
}

/// Length of `c` and, when `target_dist` is given, the `t` for [`split_cubic_at`] that cuts `c` that far from its start.
/// A straight segment `[p, p, q, q]` is measured as the distance `|q - p|` and cut at the fraction of that length: its
/// Bézier parameter is not proportional to distance (the point at parameter `t` lies at `3t² - 2t³` of the way). A curve
/// is measured by sampling its points.
fn cubic_arc_len_and_t(c: &[Vec2; 4], target_dist: Option<f32>) -> (f32, f32) {
    if is_straight(c) {
        let len = (c[3].x - c[0].x).hypot(c[3].y - c[0].y);
        let t = match target_dist {
            Some(d) if len > 1e-6 => (d / len).clamp(0.0, 1.0),
            _ => 1.0,
        };
        return (len, t);
    }
    const N: usize = 16;
    let mut prev = c[0];
    let mut total = 0.0f32;
    let mut found_t = 1.0f32;
    let mut found = false;
    for i in 1..=N {
        let t = i as f32 / N as f32;
        let cur = cubic_eval(c, t);
        let seg = (cur.x - prev.x).hypot(cur.y - prev.y);
        if !found {
            if let Some(d) = target_dist {
                if total + seg >= d && seg > 1e-6 {
                    let frac = ((d - total) / seg).clamp(0.0, 1.0);
                    found_t = ((i - 1) as f32 + frac) / N as f32;
                    found = true;
                }
            }
        }
        total += seg;
        prev = cur;
    }
    (total, found_t)
}

fn trim_open_cubics(cs: &[[Vec2; 4]], start_trim: f32, end_trim: f32) -> Vec<[Vec2; 4]> {
    if start_trim <= 0.0 && end_trim <= 0.0 {
        return cs.to_vec();
    }
    let total_len: f32 = cs.iter().map(|c| cubic_arc_len_and_t(c, None).0).sum();
    if start_trim + end_trim >= total_len - 1e-4 {
        return Vec::new();
    }
    let mut cur = cs.to_vec();
    if start_trim > 0.0 {
        let mut rem = start_trim;
        let mut next = Vec::with_capacity(cur.len());
        let mut trimmed = false;
        for c in cur {
            if trimmed {
                next.push(c);
                continue;
            }
            let (len, _) = cubic_arc_len_and_t(&c, None);
            if rem >= len - 1e-5 {
                rem -= len;
            } else {
                let (_, t) = cubic_arc_len_and_t(&c, Some(rem));
                next.push(split_cubic_at(&c, t.clamp(0.0, 1.0)).1);
                trimmed = true;
            }
        }
        cur = next;
    }
    if end_trim > 0.0 {
        let mut rem = end_trim;
        let mut next = Vec::with_capacity(cur.len());
        let mut trimmed = false;
        for c in cur.into_iter().rev() {
            if trimmed {
                next.push(c);
                continue;
            }
            let (len, _) = cubic_arc_len_and_t(&c, None);
            if rem >= len - 1e-5 {
                rem -= len;
            } else {
                let (_, t) = cubic_arc_len_and_t(&c, Some((len - rem).max(0.0)));
                next.push(split_cubic_at(&c, t.clamp(0.0, 1.0)).0);
                trimmed = true;
            }
        }
        next.reverse();
        cur = next;
    }
    cur
}

fn cap_setback(cap: d::StrokeCap, w: f32) -> f32 {
    match cap {
        d::StrokeCap::ArrowEquilateral | d::StrokeCap::TriangleFilled => 4.5 * w,
        _ => 0.0,
    }
}

type Pt2 = (f32, f32);

/// The arc that rounds the corner `at` between `prev` and `next` with a circle of `radius` touching both edges, as
/// `(where it leaves the incoming edge, its two control points, where it joins the outgoing edge)`. The arc takes at most
/// half of each edge (the circle shrinks to fit). `None` for a radius of `0.0` or a corner that is not one.
fn corner_arc(prev: Pt2, at: Pt2, next: Pt2, radius: f32) -> Option<(Pt2, Pt2, Pt2, Pt2)> {
    let (dux, duy, dvx, dvy) = (prev.0 - at.0, prev.1 - at.1, next.0 - at.0, next.1 - at.1);
    let (lu, lv) = (dux.hypot(duy), dvx.hypot(dvy));
    if radius <= 0.0 || lu < 1e-6 || lv < 1e-6 {
        return None;
    }
    let (ux, uy, vx, vy) = (dux / lu, duy / lu, dvx / lv, dvy / lv);
    let angle = (ux * vx + uy * vy).clamp(-1.0, 1.0).acos();
    if !(1e-3..=std::f32::consts::PI - 1e-3).contains(&angle) {
        return None;
    }
    let half_tan = (angle / 2.0).tan();
    let reach = (radius / half_tan).min(0.5 * lu).min(0.5 * lv);
    // a cubic stands in for the arc of the turn `PI - angle`
    let handle = 4.0 / 3.0 * ((std::f32::consts::PI - angle) / 4.0).tan() * reach * half_tan;
    let (arrive, leave) = ((at.0 + ux * reach, at.1 + uy * reach), (at.0 + vx * reach, at.1 + vy * reach));
    Some((arrive, (arrive.0 - ux * handle, arrive.1 - uy * handle), (leave.0 - vx * handle, leave.1 - vy * handle), leave))
}

/// Appends the closed polygon `pts` to `pb`, corner `i` rounded with a circle of radius `radii[i]` (`0.0` keeps it sharp).
fn append_polygon_with_rounded_corners(pb: &mut sk::PathBuilder, pts: &[Pt2], radii: &[f32]) {
    let n = pts.len();
    let arcs: Vec<_> = (0..n).map(|i| corner_arc(pts[(i + n - 1) % n], pts[i], pts[(i + 1) % n], radii[i])).collect();
    let start = arcs[0].map_or(pts[0], |(_, _, _, leave)| leave);
    pb.move_to(start.0, start.1);
    for k in (1..n).chain(std::iter::once(0)) {
        match arcs[k] {
            Some((arrive, c1, c2, leave)) => {
                pb.line_to(arrive.0, arrive.1);
                pb.cubic_to(c1.0, c1.1, c2.0, c2.1, leave.0, leave.1);
            }
            None if k != 0 => pb.line_to(pts[k].0, pts[k].1),
            None => {}
        }
    }
    pb.close();
}

/// Appends the head of `cap` at `p`, pointing along `outward`, for a stroke of weight `w`. A `Round` `join` rounds the
/// corners of the filled polygon heads with a radius of a third of the weight.
fn append_cap_outline(pb: &mut sk::PathBuilder, p: Vec2, outward: Vec2, w: f32, cap: d::StrokeCap, join: d::StrokeJoin) {
    let corner = if join == d::StrokeJoin::Round { w / 3.0 } else { 0.0 };
    let f = outward;
    let n = Vec2::new(-f.y, f.x);
    let pt = |a: f32, b: f32| -> (f32, f32) { (p.x + n.x * a + f.x * b, p.y + n.y * a + f.y * b) };
    match cap {
        d::StrokeCap::None => {}
        d::StrokeCap::Square => {
            let r = 0.5 * w;
            let (p0, p1, p2, p3) = (pt(-r, 0.0), pt(-r, r), pt(r, r), pt(r, 0.0));
            pb.move_to(p0.0, p0.1);
            pb.line_to(p1.0, p1.1);
            pb.line_to(p2.0, p2.1);
            pb.line_to(p3.0, p3.1);
            pb.close();
        }
        d::StrokeCap::Round => {
            let r = 0.5 * w;
            let k = 0.552_284_8 * r;
            let (a, c1, c2, m, c3, c4, b) = (pt(-r, 0.0), pt(-r, k), pt(-k, r), pt(0.0, r), pt(k, r), pt(r, k), pt(r, 0.0));
            pb.move_to(a.0, a.1);
            pb.cubic_to(c1.0, c1.1, c2.0, c2.1, m.0, m.1);
            pb.cubic_to(c3.0, c3.1, c4.0, c4.1, b.0, b.1);
            pb.close();
        }
        d::StrokeCap::ArrowLines => {
            let s = 0.5 * std::f32::consts::FRAC_1_SQRT_2 * w;
            let l = 4.5 * std::f32::consts::FRAC_1_SQRT_2 * w;
            let k = 0.552_284_8 * s;
            let p0 = pt(-s, s);
            let (c01, c02, p1) = (pt(-s + k, s + k), pt(s - k, s + k), pt(s, s));
            let p2 = pt(l + s, -l + s);
            let (c21, c22, p3) = (pt(l + s + k, -l + s - k), pt(l + s + k, -l - s + k), pt(l + s, -l - s));
            let (c31, c32, p4) = (pt(l + s - k, -l - s - k), pt(l - s + k, -l - s - k), pt(l - s, -l - s));
            let p5 = pt(0.0, -2.0 * s);
            let p6 = pt(-l + s, -l - s);
            let (c61, c62, p7) = (pt(-l + s - k, -l - s - k), pt(-l - s + k, -l - s - k), pt(-l - s, -l - s));
            let (c71, c72, p8) = (pt(-l - s - k, -l - s + k), pt(-l - s - k, -l + s - k), pt(-l - s, -l + s));
            pb.move_to(p0.0, p0.1);
            pb.cubic_to(c01.0, c01.1, c02.0, c02.1, p1.0, p1.1);
            pb.line_to(p2.0, p2.1);
            pb.cubic_to(c21.0, c21.1, c22.0, c22.1, p3.0, p3.1);
            pb.cubic_to(c31.0, c31.1, c32.0, c32.1, p4.0, p4.1);
            pb.line_to(p5.0, p5.1);
            pb.line_to(p6.0, p6.1);
            pb.cubic_to(c61.0, c61.1, c62.0, c62.1, p7.0, p7.1);
            pb.cubic_to(c71.0, c71.1, c72.0, c72.1, p8.0, p8.1);
            pb.line_to(p0.0, p0.1);
            pb.close();
        }
        d::StrokeCap::ArrowEquilateral => {
            let len = 5.0 * w;
            let hw = (5.0 / 3.0_f32.sqrt()) * w;
            append_polygon_with_rounded_corners(pb, &[pt(-hw, -len), pt(0.0, 0.0), pt(hw, -len)], &[corner; 3]);
        }
        d::StrokeCap::TriangleFilled => {
            let len = 5.0 * w;
            let hw = (5.0 / 3.0_f32.sqrt()) * w;
            // only the two outer corners are rounded; the others meet the shaft
            append_polygon_with_rounded_corners(pb, &[pt(-0.5 * w, -len), pt(-hw, 0.0), pt(hw, 0.0), pt(0.5 * w, -len)], &[0.0, corner, corner, 0.0]);
        }
        d::StrokeCap::DiamondFilled => {
            let d = (5.0 / 3.0_f32.sqrt()) * w;
            append_polygon_with_rounded_corners(pb, &[pt(-d, 0.0), pt(0.0, d), pt(d, 0.0), pt(0.0, -d)], &[corner; 4]);
        }
        d::StrokeCap::CircleFilled => {
            let r = (8.0 / 3.0) * w;
            let k = 0.552_284_8 * r;
            let (p0, c01, c02, p1, c11, c12, p2, c21, c22, p3, c31, c32) =
                (pt(-r, 0.0), pt(-r, k), pt(-k, r), pt(0.0, r), pt(k, r), pt(r, k), pt(r, 0.0), pt(r, -k), pt(k, -r), pt(0.0, -r), pt(-k, -r), pt(-r, -k));
            pb.move_to(p0.0, p0.1);
            pb.cubic_to(c01.0, c01.1, c02.0, c02.1, p1.0, p1.1);
            pb.cubic_to(c11.0, c11.1, c12.0, c12.1, p2.0, p2.1);
            pb.cubic_to(c21.0, c21.1, c22.0, c22.1, p3.0, p3.1);
            pb.cubic_to(c31.0, c31.1, c32.0, c32.1, p0.0, p0.1);
            pb.close();
        }
    }
}

/// A cap that leaves the stroke butt: the head of an arrow is drawn on top of the shaft, not by the stroker.
fn shaft_cap(cap: d::StrokeCap) -> d::StrokeCap {
    if cap.is_arrow() {
        d::StrokeCap::None
    } else {
        cap
    }
}

/// The cap that the sub-paths of `stroke` take when the shaft is stroked without the caps of its ends: the dashes of a
/// dashed stroke keep `Stroke::cap`, a solid stroke has no cap to keep.
fn shaft_base_cap(stroke: &d::Stroke) -> d::StrokeCap {
    if crate::outline::has_dashes(stroke) {
        shaft_cap(stroke.cap)
    } else {
        d::StrokeCap::None
    }
}

/// Directions that point out of the two ends of the open curve `cs`, away from the path: `(start, end)`.
fn outward_directions(cs: &[[Vec2; 4]]) -> (Vec2, Vec2) {
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
    let (first, last) = (&cs[0], &cs[cs.len() - 1]);
    let s_in = dir(first[0], [first[1], first[2], first[3]]);
    let e_in = dir(last[3], [last[2], last[1], last[0]]);
    (Vec2::new(-s_in.x, -s_in.y), Vec2::new(-e_in.x, -e_in.y))
}

/// `st` without its dashes, capped with `cap`.
fn solid_stroke(st: &sk::Stroke, cap: sk::LineCap) -> sk::Stroke {
    sk::Stroke { width: st.width, miter_limit: st.miter_limit, line_cap: cap, line_join: st.line_join, dash: None }
}

/// Strokes the dashed open path `shaft`. Every dash is capped with `Stroke::cap`, except where the first dash starts
/// and where the last dash ends, if the path runs into them: those terminals take `start_cap` and `end_cap`. A path
/// that ends in a gap shows no cap at that end, and an arrow cap leaves its terminal butt (the head sits on top).
/// The dashes that keep `Stroke::cap` at both ends are stroked as a plain dashed line is.
fn expand_dashed_shaft(
    shaft: &d::Path,
    stroke: &d::Stroke,
    width: f32,
    (start_cap, end_cap): (d::StrokeCap, d::StrokeCap),
    res_scale: f32,
) -> Option<sk::Path> {
    let skp = vector_paths(std::slice::from_ref(shaft)).paths.into_iter().next().map(|x| x.0)?;
    let inner = shaft_cap(stroke.cap);
    let st = sk_stroke(stroke, width, inner);
    let Some(dash) = st.dash.as_ref() else { return skp.stroke(&st, res_scale) };
    let dashed = skp.dash(dash, res_scale)?;
    let (terminal_start, terminal_end) = (skp.points().first().copied()?, skp.points().last().copied()?);
    let at = |a: sk::Point, b: sk::Point| (a.x - b.x).hypot(a.y - b.y) <= 1e-2;

    // the dashes as contours, with the points they start and end at
    let mut contours: Vec<(sk::Path, sk::Point, sk::Point)> = Vec::new();
    let (mut pb, mut first, mut last) = (sk::PathBuilder::new(), None::<sk::Point>, sk::Point::zero());
    let mut flush = |pb: &mut sk::PathBuilder, first: Option<sk::Point>, last: sk::Point| {
        if let (Some(path), Some(first)) = (std::mem::replace(pb, sk::PathBuilder::new()).finish(), first) {
            contours.push((path, first, last));
        }
    };
    for seg in dashed.segments() {
        match seg {
            PathSegment::MoveTo(p) => {
                flush(&mut pb, first.take(), last);
                pb.move_to(p.x, p.y);
                (first, last) = (Some(p), p);
            }
            PathSegment::LineTo(p) => {
                pb.line_to(p.x, p.y);
                last = p;
            }
            PathSegment::QuadTo(c, p) => {
                pb.quad_to(c.x, c.y, p.x, p.y);
                last = p;
            }
            PathSegment::CubicTo(c1, c2, p) => {
                pb.cubic_to(c1.x, c1.y, c2.x, c2.y, p.x, p.y);
                last = p;
            }
            PathSegment::Close => pb.close(),
        }
    }
    flush(&mut pb, first.take(), last);

    let count = contours.len();
    let mut out = sk::PathBuilder::new();
    let mut plain = sk::PathBuilder::new();
    for (i, (contour, from, to)) in contours.into_iter().enumerate() {
        let start = if i == 0 && at(from, terminal_start) { shaft_cap(start_cap) } else { inner };
        let end = if i + 1 == count && at(to, terminal_end) { shaft_cap(end_cap) } else { inner };
        if (start, end) == (inner, inner) {
            plain.push_path(&contour);
            continue;
        }
        // a terminal with a cap of its own: stroke the dash butt and add the cap of each of its ends
        if let Some(body) = contour.stroke(&solid_stroke(&st, sk::LineCap::Butt), res_scale) {
            out.push_path(&body);
        }
        let doc = sk_to_doc(&contour, d::FillRule::NonZero);
        if let Some((cs, false)) = path_curves(&doc).into_iter().next().filter(|(cs, _)| !cs.is_empty()) {
            let (start_out, end_out) = outward_directions(&cs);
            append_cap_outline(&mut out, cs[0][0], start_out, width, start, stroke.join);
            append_cap_outline(&mut out, cs[cs.len() - 1][3], end_out, width, end, stroke.join);
        }
    }
    if let Some(body) = plain.finish().and_then(|p| p.stroke(&solid_stroke(&st, st.line_cap), res_scale)) {
        out.push_path(&body);
    }
    out.finish()
}

fn expand_open_subpath_with_caps(
    sub: &d::Path,
    stroke: &d::Stroke,
    width: f32,
    (start_cap, end_cap): (d::StrokeCap, d::StrokeCap),
    network: Option<&VectorNetwork>,
    res_scale: f32,
) -> Option<sk::Path> {
    let Some((cs, is_closed)) = path_curves(sub).into_iter().next() else {
        return expand_subpath(sub, stroke, width, shaft_base_cap(stroke), network, res_scale);
    };
    if is_closed || cs.is_empty() {
        return expand_subpath(sub, stroke, width, shaft_base_cap(stroke), network, res_scale);
    }
    let (start_out, end_out) = outward_directions(&cs);
    let (first, last) = (&cs[0], &cs[cs.len() - 1]);

    let start_trim = cap_setback(start_cap, width);
    let end_trim = cap_setback(end_cap, width);
    let dashed = crate::outline::has_dashes(stroke);
    let mut pb = sk::PathBuilder::new();
    let trimmed = trim_open_cubics(&cs, start_trim, end_trim);
    if !trimmed.is_empty() {
        let mut cmds = Vec::with_capacity(trimmed.len() + 1);
        cmds.push(d::PathCmd::MoveTo(trimmed[0][0]));
        for c in &trimmed {
            if is_straight(c) {
                cmds.push(d::PathCmd::LineTo(c[3]));
            } else {
                cmds.push(d::PathCmd::CubicTo(c[1], c[2], c[3]));
            }
        }
        let trimmed_sub = d::Path { cmds, fill_rule: sub.fill_rule };
        let shaft = if dashed {
            expand_dashed_shaft(&trimmed_sub, stroke, width, (start_cap, end_cap), res_scale)
        } else {
            expand_subpath(&trimmed_sub, stroke, width, d::StrokeCap::None, network, res_scale)
        };
        if let Some(shaft) = shaft {
            pb.push_path(&shaft);
        }
    }
    // the caps of a dashed shaft that are not arrow heads sit on its first and last dash, if the path runs into them
    let head = |cap: d::StrokeCap| if dashed && !cap.is_arrow() { d::StrokeCap::None } else { cap };
    append_cap_outline(&mut pb, first[0], start_out, width, head(start_cap), stroke.join);
    append_cap_outline(&mut pb, last[3], end_out, width, head(end_cap), stroke.join);
    pb.finish()
}

/// The caps at the start and end of the open sub-path `sub`: the cap of the vertex it ends in, `default_cap` for a vertex
/// without one, and `junction_cap` for a vertex that other segments run through.
fn open_subpath_caps(
    sub: &d::Path,
    net: Option<&VectorNetwork>,
    chain_end: Option<(usize, usize)>,
    default_cap: d::StrokeCap,
    junction_cap: d::StrokeCap,
) -> (d::StrokeCap, d::StrokeCap) {
    let Some(net) = net else { return (default_cap, default_cap) };
    let Some((cs, false)) = path_curves(sub).into_iter().next() else { return (default_cap, default_cap) };
    if cs.is_empty() {
        return (default_cap, default_cap);
    }
    let (start_pt, end_pt) = (cs[0][0], cs[cs.len() - 1][3]);
    let resolve = |hint: Option<usize>, pt: Vec2| -> d::StrokeCap {
        if let Some(vi) = hint.filter(|&i| i < net.vertices.len() && (net.vertices[i].pos.x - pt.x).hypot(net.vertices[i].pos.y - pt.y) <= 1e-2) {
            return if net.is_endpoint(vi) { net.vertices[vi].cap.unwrap_or(default_cap) } else { junction_cap };
        }
        if let Some((vi, v)) = net.vertices.iter().enumerate().find(|(_, v)| (v.pos.x - pt.x).hypot(v.pos.y - pt.y) <= 1e-2) {
            return if net.is_endpoint(vi) { v.cap.unwrap_or(default_cap) } else { junction_cap };
        }
        default_cap
    };
    (resolve(chain_end.map(|x| x.0), start_pt), resolve(chain_end.map(|x| x.1), end_pt))
}

/// Expands one sub-path `sub` by `stroke` at `width` and `res_scale`, honouring per-vertex
/// `StrokeJoin` overrides from `network` when present.
fn expand_subpath(sub: &d::Path, stroke: &d::Stroke, width: f32, base_cap: d::StrokeCap, network: Option<&VectorNetwork>, res_scale: f32) -> Option<sk::Path> {
    let skp = vector_paths(std::slice::from_ref(sub)).paths.into_iter().next().map(|x| x.0)?;
    let Some(net) = network.filter(|n| !crate::outline::has_dashes(stroke) && n.vertices.iter().any(|v| v.join.is_some_and(|j| j != stroke.join))) else {
        return crate::outline::stroke_outline(&skp, &sk_stroke(stroke, width, base_cap), res_scale);
    };
    let Some((cs, is_closed)) = path_curves(sub).into_iter().next() else {
        return crate::outline::stroke_outline(&skp, &sk_stroke(stroke, width, base_cap), res_scale);
    };
    let n = cs.len();
    if n == 0 || (!is_closed && n == 1) {
        return crate::outline::stroke_outline(&skp, &sk_stroke(stroke, width, base_cap), res_scale);
    }
    let join_at =
        |p: Vec2| -> d::StrokeJoin { net.vertices.iter().find(|v| (v.pos.x - p.x).hypot(v.pos.y - p.y) <= 1e-2).and_then(|v| v.join).unwrap_or(stroke.join) };
    let joins: Vec<d::StrokeJoin> = if is_closed { (0..n).map(|k| join_at(cs[k][0])).collect() } else { (1..n).map(|k| join_at(cs[k][0])).collect() };
    if joins.iter().all(|&j| j == joins[0]) {
        return crate::outline::stroke_outline(&skp, &sk_stroke_with_join(stroke, width, base_cap, joins[0]), res_scale);
    }
    let mut all_pb = sk::PathBuilder::new();
    let corner_indices: std::ops::Range<usize> = if is_closed { 0..n } else { 1..n };
    for k in corner_indices {
        let j = if is_closed { joins[k] } else { joins[k - 1] };
        let c_in = if is_closed {
            split_cubic_half(&cs[(k + n - 1) % n]).1
        } else if k == 1 {
            cs[0]
        } else {
            split_cubic_half(&cs[k - 1]).1
        };
        let c_out = if is_closed {
            split_cubic_half(&cs[k]).0
        } else if k + 1 == n {
            cs[n - 1]
        } else {
            split_cubic_half(&cs[k]).0
        };
        let mut pb = sk::PathBuilder::new();
        pb.move_to(c_in[0].x, c_in[0].y);
        pb.cubic_to(c_in[1].x, c_in[1].y, c_in[2].x, c_in[2].y, c_in[3].x, c_in[3].y);
        pb.cubic_to(c_out[1].x, c_out[1].y, c_out[2].x, c_out[2].y, c_out[3].x, c_out[3].y);
        if let Some(piece) = pb.finish() {
            let st = sk_stroke_with_join(stroke, width, d::StrokeCap::None, j);
            if let Some(o) = crate::outline::stroke_outline(&piece, &st, res_scale) {
                all_pb.push_path(&o);
            }
        }
    }
    if !is_closed && base_cap != d::StrokeCap::None {
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
        let (first, last) = (&cs[0], &cs[n - 1]);
        let s_in = dir(first[0], [first[1], first[2], first[3]]);
        let e_in = dir(last[3], [last[2], last[1], last[0]]);
        append_cap_outline(&mut all_pb, first[0], Vec2::new(-s_in.x, -s_in.y), width, base_cap, stroke.join);
        append_cap_outline(&mut all_pb, last[3], Vec2::new(-e_in.x, -e_in.y), width, base_cap, stroke.join);
    }
    all_pb.finish()
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

/// Cap shapes at the ends of open sub-paths for caps that differ from what the butt stroker draws.
fn cap_polygons(sub: &d::Path, w: f32, join: d::StrokeJoin, start_cap: d::StrokeCap, end_cap: d::StrokeCap) -> Vec<Vec<Pt>> {
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
        _ => {
            let mut pb = sk::PathBuilder::new();
            append_cap_outline(&mut pb, p, outward, w, cap, join);
            if let Some(path) = pb.finish() {
                out.extend(flatten(&path, 0.05));
            }
        }
    };
    // both directions run from an end point into the path; a cap points the opposite way
    let s_in = dir(first[0], [first[1], first[2], first[3]]);
    let e_in = dir(last[3], [last[2], last[1], last[0]]);
    cap_at(first[0], Vec2::new(-s_in.x, -s_in.y), start_cap);
    cap_at(last[3], Vec2::new(-e_in.x, -e_in.y), end_cap);
    out
}

/// True when `stroke` is dashed and some open chain of `net` ends in an end point (a vertex that no other segment runs
/// through) whose cap is not the cap of the dashes. A dashed stroke caps every dash with `Stroke::cap`; the cap of an
/// end point shows only where the first dash starts or the last dash ends.
fn dashed_end_caps(stroke: &d::Stroke, net: &VectorNetwork) -> bool {
    stroke.weight > 0.0
        && crate::outline::has_dashes(stroke)
        && net.vertices.iter().any(|v| v.cap.is_some_and(|c| c != stroke.cap))
        && net.is_valid()
        && net.open_chain_ends().into_iter().flat_map(|(a, b)| [a, b]).any(|v| net.is_endpoint(v) && net.vertices[v].cap.is_some_and(|c| c != stroke.cap))
}

/// Cap shapes to draw over a butt-capped stroke of a vector whose vertices carry their own stroke caps; `None` when
/// no vertex overrides the cap. A dashed stroke caps every dash with `Stroke::cap`, so there is nothing to draw over
/// it: the list is empty (not `None`) when an end point of a dashed stroke has a cap of its own, which tells callers
/// to take the stroke from `expand_node_subpaths` instead of the plain stroker.
pub fn vertex_caps(n: &Node) -> Option<Vec<d::Path>> {
    let NodeKind::Vector { network: Some(net), .. } = &n.kind else { return None };
    if crate::outline::has_dashes(&n.stroke) {
        return dashed_end_caps(&n.stroke, net).then(Vec::new);
    }
    if !net.is_valid() || !net.vertices.iter().any(|v| v.cap.is_some_and(|c| c != n.stroke.cap)) || n.stroke.weight <= 0.0 {
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
                for poly in cap_polygons(&sub, n.stroke.weight, n.stroke.join, sc, ec) {
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

/// True when `n` is a valid, non-dashed vector network whose vertices override `Stroke::join`.
pub fn has_per_vertex_joins(n: &Node) -> bool {
    let NodeKind::Vector { network: Some(net), .. } = &n.kind else { return false };
    !crate::outline::has_dashes(&n.stroke) && net.is_valid() && net.vertices.iter().any(|v| v.join.is_some_and(|j| j != n.stroke.join))
}

/// True when `n` draws one or more arrow caps on its open endpoints (either via `stroke.cap`
/// or via an endpoint vertex override on a valid `VectorNetwork`).
pub fn has_arrow_caps(n: &Node) -> bool {
    if n.stroke.weight <= 0.0 {
        return false;
    }
    if n.stroke.cap.is_arrow() {
        return true;
    }
    match &n.kind {
        NodeKind::Vector { network: Some(net), .. } => {
            net.is_valid() && net.vertices.iter().enumerate().any(|(i, v)| net.is_endpoint(i) && v.cap.is_some_and(d::StrokeCap::is_arrow))
        }
        _ => false,
    }
}

/// True when `n` has an `Inside` or `Outside` stroke on a shape that mixes closed and open sub-paths.
pub fn has_mixed_subpath_align(n: &Node, sh: &crate::geom::Shape) -> bool {
    if n.stroke.align == d::StrokeAlign::Center || sh.closed {
        return false;
    }
    sh.paths.iter().any(|(p, _)| p.segments().any(|s| matches!(s, PathSegment::Close)))
}

/// Expands every sub-path of `sh` at `width` and `res_scale`, honouring per-vertex joins and caps
/// (including arrowhead caps on the node or on open-chain endpoint vertices, and the end point caps of a dashed stroke).
pub(crate) fn expand_node_subpaths(n: &Node, sh: &crate::geom::Shape, width: f32, res_scale: f32) -> Vec<(sk::Path, sk::FillRule)> {
    let network = match &n.kind {
        NodeKind::Vector { network: Some(net), .. } if net.is_valid() => Some(net),
        _ => None,
    };
    if has_arrow_caps(n) || network.is_some_and(|net| dashed_end_caps(&n.stroke, net)) {
        let chain_ends = network.map(VectorNetwork::open_chain_ends).unwrap_or_default();
        // a dashed stroke keeps its own cap on the dashes that reach no end point, and on closed sub-paths
        let base_cap = shaft_base_cap(&n.stroke);
        let mut open_idx = 0usize;
        let mut out = Vec::new();
        for (p, r) in &sh.paths {
            let doc_p = sk_to_doc(p, rule_of(*r));
            for (sub, closed) in subpaths(&doc_p) {
                let expanded = if closed {
                    expand_subpath(&sub, &n.stroke, width, base_cap, network, res_scale)
                } else {
                    let hint = chain_ends.get(open_idx).copied();
                    open_idx += 1;
                    let caps = open_subpath_caps(&sub, network, hint, n.stroke.cap, base_cap);
                    expand_open_subpath_with_caps(&sub, &n.stroke, width, caps, network, res_scale)
                };
                if let Some(o) = expanded {
                    out.push((o, sk::FillRule::Winding));
                }
            }
        }
        return out;
    }
    let base_cap = if vertex_caps(n).is_some() { d::StrokeCap::None } else { n.stroke.cap };
    let mut out = Vec::new();
    for (p, r) in &sh.paths {
        let doc_p = sk_to_doc(p, rule_of(*r));
        for (sub, _) in subpaths(&doc_p) {
            if let Some(o) = expand_subpath(&sub, &n.stroke, width, base_cap, network, res_scale) {
                out.push((o, sk::FillRule::Winding));
            }
        }
    }
    if let Some(caps) = vertex_caps(n) {
        out.extend(vector_paths(&caps).paths);
    }
    out
}

/// Stroke outline for a shape mixing closed sub-paths (`Inside`/`Outside`) and open sub-paths (`Center`).
pub(crate) fn mixed_subpath_stroke_outline(n: &Node, sh: &crate::geom::Shape) -> Vec<(sk::Path, sk::FillRule)> {
    let network = match &n.kind {
        NodeKind::Vector { network: Some(net), .. } if net.is_valid() => Some(net),
        _ => None,
    };
    let doc_paths: Vec<d::Path> = sh.paths.iter().map(|(p, r)| sk_to_doc(p, rule_of(*r))).collect();
    let outlined = outline_stroke_along(&doc_paths, &n.stroke, network, None);
    vector_paths(&outlined).paths
}

/// Converts the stroke of `paths` into filled geometry: weight, alignment,
/// caps, joins, miter limit and dashes are honoured. `network` supplies
/// per-vertex stroke caps and joins (`paths` must then be `network.to_paths()`). The
/// result is filled with the non-zero (or, after clipping, even-odd) rule.
///
/// Centered strokes without per-vertex caps keep their curves exactly (the
/// stroker's own offset curves); inside/outside alignment and mixed caps go
/// through polygon clipping and are re-fitted to curves.
pub fn outline_stroke(paths: &[d::Path], stroke: &d::Stroke, network: Option<&VectorNetwork>) -> Vec<d::Path> {
    outline_stroke_along(paths, stroke, network, None)
}

/// [`outline_stroke`] of the geometry of node `n`, expanding a stroke the way the canvas strokes it.
/// A rectangle-like shape with a dashed `Inside` or `Outside` stroke is stroked along its inset or
/// outset outline at the stroke's own weight (so each dash cap is `w` wide) and clipped to its side
/// of the boundary; every other node goes through [`outline_stroke`] unchanged. Which shapes take
/// the inset or outset outline is decided by the node's own geometry, exactly as for the canvas, so
/// a vector that merely looks like a rectangle keeps the doubled boundary stroke.
/// Empty when the node has no geometry or no stroke weight.
pub fn outline_node_stroke(doc: &Document, n: &Node) -> Vec<d::Path> {
    let Some(paths) = node_paths(doc, n) else { return vec![] };
    let network = match &n.kind {
        NodeKind::Vector { network: Some(net), .. } => Some(net),
        _ => None,
    };
    let aligned_outline = node_shape(doc, n, 0.05).and_then(|sh| crate::outline::dashed_rect_stroke_path(n, &sh));
    outline_stroke_along(&paths, &n.stroke, network, aligned_outline.as_ref())
}

/// `aligned_outline` replaces the closed sub-paths as the line that an `Inside` or `Outside`
/// stroke follows, at the stroke's own weight instead of twice that; the clip to the shape stays.
fn outline_stroke_along(paths: &[d::Path], stroke: &d::Stroke, network: Option<&VectorNetwork>, aligned_outline: Option<&sk::Path>) -> Vec<d::Path> {
    let w = stroke.weight;
    if w <= 0.0 || paths.is_empty() {
        return vec![];
    }
    let per_vertex_caps = !crate::outline::has_dashes(stroke) && network.is_some_and(|n| n.vertices.iter().any(|v| v.cap.is_some_and(|c| c != stroke.cap)));
    let chain_caps: Vec<(d::StrokeCap, d::StrokeCap)> = match network {
        Some(n) if per_vertex_caps => {
            n.open_chain_ends().iter().map(|(a, b)| (n.vertices[*a].cap.unwrap_or(stroke.cap), n.vertices[*b].cap.unwrap_or(stroke.cap))).collect()
        }
        _ => vec![],
    };
    // the end points of a dashed stroke cap only the first and the last dash, the other dashes keep `Stroke::cap`
    let dashed_caps = network.is_some_and(|n| dashed_end_caps(stroke, n));
    let chain_ends = if dashed_caps { network.map(VectorNetwork::open_chain_ends).unwrap_or_default() } else { vec![] };
    let base_cap = if per_vertex_caps { d::StrokeCap::None } else { stroke.cap };
    let aligned = stroke.align != d::StrokeAlign::Center;
    let st_center = sk_stroke(stroke, w, base_cap);

    let mut out: Vec<d::Path> = vec![];
    let mut open_index = 0;
    for p in paths {
        let mut closed_subs: Vec<d::Path> = vec![];
        for (sub, closed) in subpaths(p) {
            if closed {
                if aligned {
                    closed_subs.push(sub);
                } else if let Some(o) = expand_subpath(&sub, stroke, w, base_cap, network, 1.0) {
                    out.push(sk_to_doc(&o, d::FillRule::NonZero));
                }
                continue;
            }
            let caps = if per_vertex_caps { chain_caps.get(open_index).copied() } else { None };
            if dashed_caps {
                let resolved = open_subpath_caps(&sub, network, chain_ends.get(open_index).copied(), stroke.cap, shaft_base_cap(stroke));
                open_index += 1;
                if let Some(o) = expand_open_subpath_with_caps(&sub, stroke, w, resolved, network, 1.0) {
                    out.push(sk_to_doc(&o, d::FillRule::NonZero));
                }
                continue;
            }
            open_index += 1;
            let resolved_caps = caps.unwrap_or((stroke.cap, stroke.cap));
            if resolved_caps.0.is_arrow() || resolved_caps.1.is_arrow() {
                if let Some(o) = expand_open_subpath_with_caps(&sub, stroke, w, resolved_caps, network, 1.0) {
                    out.push(sk_to_doc(&o, d::FillRule::NonZero));
                }
                continue;
            }
            let Some(o) = expand_subpath(&sub, stroke, w, base_cap, network, 1.0) else { continue };
            match caps {
                Some((sc, ec)) => {
                    let mut shapes: Shapes = flatten(&o, 0.05).simplify_shape(FillRule::NonZero);
                    let extra: Vec<Vec<Pt>> = cap_polygons(&sub, w, stroke.join, sc, ec);
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
            if let Some(outline) = aligned_outline {
                if let Some(o) = expand(outline, &st_center) {
                    ring = flatten(&o, 0.05).simplify_shape(FillRule::NonZero);
                    if !leaves_its_side(&ring, &fill, &closed_subs, stroke.align) {
                        // already on its side of the shape: keep the stroker's exact curves
                        out.push(sk_to_doc(&o, d::FillRule::NonZero));
                        continue;
                    }
                }
            } else {
                for c in &closed_subs {
                    let Some(o) = expand_subpath(c, stroke, w * 2.0, base_cap, network, 1.0) else { continue };
                    let polys = flatten(&o, 0.05).simplify_shape(FillRule::NonZero);
                    ring = if ring.is_empty() { polys } else { ring.overlay(&polys, OverlayRule::Union, FillRule::NonZero) };
                }
            }
            let rule = if stroke.align == d::StrokeAlign::Inside { OverlayRule::Intersect } else { OverlayRule::Difference };
            let clipped = ring.overlay(&fill, rule, FillRule::NonZero);
            out.extend(shapes_to_paths(&clipped, 0.08));
        }
    }
    out
}

/// How far (document units) a stroke may reach across the boundary of its shape before it counts as
/// crossing it: flattening the stroke and the shape leaves differences well below this.
const SIDE_SLACK: f32 = 0.2;

/// Whether the stroke `ring` reaches onto the wrong side of the shape `fill` (outside it for an
/// `Inside` stroke, into it for an `Outside` one) by more than [`SIDE_SLACK`]. `boundary` holds the
/// closed outlines of the shape.
fn leaves_its_side(ring: &Shapes, fill: &Shapes, boundary: &[d::Path], align: d::StrokeAlign) -> bool {
    let st = sk::Stroke { width: SIDE_SLACK * 2.0, ..Default::default() };
    let mut band: Shapes = vec![];
    for c in boundary {
        let Some(skp) = vector_paths(std::slice::from_ref(c)).paths.into_iter().next().map(|x| x.0) else { continue };
        let Some(o) = expand(&skp, &st) else { continue };
        let polys = flatten(&o, 0.05).simplify_shape(FillRule::NonZero);
        band = if band.is_empty() { polys } else { band.overlay(&polys, OverlayRule::Union, FillRule::NonZero) };
    }
    let wrong = if align == d::StrokeAlign::Inside {
        let allowed = fill.overlay(&band, OverlayRule::Union, FillRule::NonZero);
        ring.overlay(&allowed, OverlayRule::Difference, FillRule::NonZero)
    } else {
        let forbidden = fill.overlay(&band, OverlayRule::Difference, FillRule::NonZero);
        ring.overlay(&forbidden, OverlayRule::Intersect, FillRule::NonZero)
    };
    !wrong.is_empty()
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
        NodeKind::Page { .. } | NodeKind::Unknown { .. } => {}
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
