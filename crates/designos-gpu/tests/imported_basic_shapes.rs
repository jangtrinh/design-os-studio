//! Imported basic shapes (ellipses, polygons, stars, rings) drawn by the GPU renderer must match the CPU
//! reference. Tests print a note and return when the machine has no GPU adapter.

mod common;

use common::*;
use designos_doc::*;
use designos_render::Viewport;

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

/// The stored outline of an inside stroke: centered on the edge at double weight.
fn stored_ring(r: f32, w: f32) -> Vec<Path> {
    let mut cmds = circle_cmds(r, r, r + w);
    cmds.extend(circle_cmds(r, r, r - w));
    vec![Path { cmds, fill_rule: FillRule::EvenOdd }]
}

#[test]
fn inside_stroked_imported_ellipse_stays_inside_on_both_renderers() {
    let Some(mut g) = gpu() else { return };
    let mut t = T::new();
    t.add(None, |id| {
        let mut n = Node::ellipse(id, Rect::new(30.0, 30.0, 100.0, 100.0));
        n.fills.clear();
        n.stroke.paints = vec![Paint::solid(Color::rgb(255, 0, 0))];
        n.stroke.weight = 10.0;
        n.stroke.align = StrokeAlign::Inside;
        n.stroke_geometry = Some(stored_ring(50.0, 10.0));
        n
    });
    let c = compare(&mut g, "inside_stroke_stored_ellipse", &t.doc, t.page, Viewport::default(), (160, 160), 8);
    assert!(c.report.ratio < 0.02, "{:?}", c.report);
    for (name, pm) in [("cpu", &c.cpu), ("gpu", &c.gpu)] {
        let outside = pm.pixel(80 + 55, 80).unwrap();
        assert!(outside.red() > 245 && outside.green() > 245, "{name}: nothing spills outside the edge: {outside:?}");
        let band = pm.pixel(80 + 45, 80).unwrap();
        assert!(band.red() > 200 && band.green() < 60, "{name}: the band is inked: {band:?}");
    }
}
