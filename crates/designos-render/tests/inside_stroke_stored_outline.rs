//! An imported shape carries its stroke as a stored outline. Figma stores an inside stroke at double
//! width (centered on the edge), so it has to be clipped to the shape, like an outside stroke is clipped
//! to the outside of it. Without that the stroke spills past the edge at twice the weight.

mod common;

use common::*;
use designos_doc::*;

fn circle_cmds(cx: f32, cy: f32, r: f32) -> Vec<PathCmd> {
    let n = 96;
    let mut cmds = Vec::new();
    for i in 0..n {
        let a = std::f32::consts::TAU * i as f32 / n as f32;
        let v = Vec2::new(cx + r * a.cos(), cy + r * a.sin());
        cmds.push(if i == 0 { PathCmd::MoveTo(v) } else { PathCmd::LineTo(v) });
    }
    cmds.push(PathCmd::Close);
    cmds
}

/// The stored outline of a centered stroke of `2 * w` around a circle of radius `r`.
fn ring(cx: f32, cy: f32, r: f32, w: f32) -> Vec<Path> {
    let mut cmds = circle_cmds(cx, cy, r + w);
    cmds.extend(circle_cmds(cx, cy, r - w));
    vec![Path { cmds, fill_rule: FillRule::EvenOdd }]
}

fn scene(kind: NodeKind, align: StrokeAlign, geometry: Vec<Path>) -> T {
    let mut t = T::new();
    t.add(None, |id| {
        let mut n = Node::ellipse(id, Rect::new(30.0, 30.0, 100.0, 100.0));
        n.kind = kind;
        n.fills.clear();
        n.stroke.paints = vec![Paint::solid(RED)];
        n.stroke.weight = 10.0;
        n.stroke.align = align;
        n.stroke_geometry = Some(geometry);
        n
    });
    t
}

fn is_red(p: [u8; 4]) -> bool {
    p[0] > 200 && p[1] < 60 && p[2] < 60
}

/// Pixel at `r` from the centre of the 100x100 shape placed at (30, 30), along the +x axis.
fn at_radius(pm: &designos_render::tiny_skia::Pixmap, r: f32) -> [u8; 4] {
    px(pm, (80.0 + r) as u32, 80)
}

#[test]
fn inside_stroked_ellipse_keeps_the_stroke_inside_at_the_stored_weight() {
    let t = scene(NodeKind::Ellipse(ArcData::default()), StrokeAlign::Inside, ring(50.0, 50.0, 50.0, 10.0));
    let pm = t.render(160, 160);
    assert!(is_red(at_radius(&pm, 45.0)), "the band 40..50 is inked");
    assert!(is_red(at_radius(&pm, 41.0)));
    assert_eq!(at_radius(&pm, 55.0), WHITE, "nothing spills outside the edge");
    assert_eq!(at_radius(&pm, 36.0), WHITE, "the band is the stored weight, not twice it");
}

#[test]
fn inside_stroked_polygon_does_not_spill_past_its_edges() {
    // a diamond: |x - 50| + |y - 50| <= 50 in the node; the stored band is a square ring around the box edge
    let sq = |lo: f32, hi: f32| {
        vec![
            PathCmd::MoveTo(Vec2::new(lo, lo)),
            PathCmd::LineTo(Vec2::new(hi, lo)),
            PathCmd::LineTo(Vec2::new(hi, hi)),
            PathCmd::LineTo(Vec2::new(lo, hi)),
            PathCmd::Close,
        ]
    };
    let mut cmds = sq(-10.0, 110.0);
    cmds.extend(sq(10.0, 90.0));
    let t = scene(NodeKind::Polygon { count: 4 }, StrokeAlign::Inside, vec![Path { cmds, fill_rule: FillRule::EvenOdd }]);
    let pm = t.render(160, 160);
    // local (4, 4) is in the stored band but outside the diamond
    assert_eq!(px(&pm, 34, 34), WHITE);
}

#[test]
fn outside_and_center_stored_outlines_are_unchanged() {
    let t = scene(NodeKind::Ellipse(ArcData::default()), StrokeAlign::Center, ring(50.0, 50.0, 50.0, 5.0));
    let pm = t.render(160, 160);
    assert!(is_red(at_radius(&pm, 52.0)), "a center stroke straddles the edge");
    assert!(is_red(at_radius(&pm, 48.0)));
    let t = scene(NodeKind::Ellipse(ArcData::default()), StrokeAlign::Outside, ring(50.0, 50.0, 55.0, 5.0));
    let pm = t.render(160, 160);
    assert!(is_red(at_radius(&pm, 55.0)));
    assert_eq!(at_radius(&pm, 45.0), WHITE);
}
