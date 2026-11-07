//! A polygon or star exported to SVG puts its points where the canvas draws them: on the ellipse
//! inscribed in the node box, not stretched to fill the box.

use designos_doc::*;
use designos_export::{export_svg, SvgOptions};
use designos_render::vectorops::flatten_nodes;

fn extents_of_cmds(cmds: impl Iterator<Item = Vec2>) -> (f32, f32) {
    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for p in cmds {
        x0 = x0.min(p.x);
        y0 = y0.min(p.y);
        x1 = x1.max(p.x);
        y1 = y1.max(p.y);
    }
    (x1 - x0, y1 - y0)
}

/// Size of the box around the points of the first `d="..."` in the SVG.
fn svg_extents(svg: &str) -> (f32, f32) {
    let start = svg.find(" d=\"").expect("a path") + 4;
    let d = &svg[start..start + svg[start..].find('"').unwrap()];
    let nums: Vec<f32> = d.split(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-')).filter(|s| !s.is_empty()).map(|s| s.parse().unwrap()).collect();
    extents_of_cmds(nums.chunks_exact(2).map(|c| Vec2::new(c[0], c[1])))
}

fn canvas_extents(doc: &Document, id: NodeId) -> (f32, f32) {
    let flat = flatten_nodes(doc, &[id], &Affine::IDENTITY).expect("flattens");
    let pts = flat.paths.iter().flat_map(|p| p.cmds.iter()).filter_map(|c| match *c {
        PathCmd::MoveTo(v) | PathCmd::LineTo(v) | PathCmd::QuadTo(_, v) | PathCmd::CubicTo(_, _, v) => Some(v),
        PathCmd::Close => None,
    });
    extents_of_cmds(pts)
}

fn check(kind: NodeKind, size: Vec2) {
    let mut doc = Document::new("t");
    let page = doc.add_page("p");
    let id = doc.alloc_id();
    let mut n = Node::new(id, kind);
    n.size = size;
    n.fills = vec![Paint::solid(Color::BLACK)];
    let id = doc.insert(page, n).unwrap();
    let (cw, ch) = canvas_extents(&doc, id);
    let (sw, sh) = svg_extents(&export_svg(&doc, id, &SvgOptions::default()));
    assert!((cw - sw).abs() < 0.5 && (ch - sh).abs() < 0.5, "canvas {cw} x {ch}, svg {sw} x {sh}");
}

#[test]
fn a_resized_star_exports_the_points_the_canvas_draws() {
    check(NodeKind::Star { count: 5, inner_ratio: 0.382 }, Vec2::new(200.0, 70.0));
}

#[test]
fn a_polygon_exports_the_points_the_canvas_draws() {
    check(NodeKind::Polygon { count: 7 }, Vec2::new(100.0, 100.0));
}
