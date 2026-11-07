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

fn basic_shape(kind: NodeKind, dashed_ring: bool) -> T {
    let mut t = T::new();
    t.add(None, |id| {
        let mut n = Node::ellipse(id, Rect::new(10.0, 10.0, 100.0, 100.0));
        n.kind = kind;
        if dashed_ring {
            n.fills.clear();
            n.stroke.paints = vec![Paint::solid(Color::BLACK)];
            n.stroke.weight = 6.0;
            n.stroke.dashes = vec![0.1, 30.0];
            n.stroke.align = StrokeAlign::Center;
            n.stroke.cap = StrokeCap::Round;
        } else {
            n.fills = vec![Paint::solid(Color::BLACK)];
        }
        n
    });
    t
}

#[test]
fn polygon_star_and_dotted_ring_match_the_cpu() {
    let Some(mut g) = gpu() else { return };
    for (name, kind, dashed) in [
        ("heptagon_inscribed", NodeKind::Polygon { count: 7 }, false),
        ("star_inscribed", NodeKind::Star { count: 5, inner_ratio: 0.382 }, false),
        ("dotted_ring_single_contour", NodeKind::Ellipse(ArcData { start: 0.0, end: std::f32::consts::TAU, inner_radius: 1.0 }), true),
    ] {
        let t = basic_shape(kind, dashed);
        g.compiler.geom.clear();
        let c = compare(&mut g, name, &t.doc, t.page, Viewport::default(), (120, 120), 8);
        assert!(c.report.ratio < 0.03, "{name}: {:?}", c.report);
        for (who, pm) in [("cpu", &c.cpu), ("gpu", &c.gpu)] {
            assert!(ink_count(pm) > 100, "{name}: {who} draws real ink, got {}", ink_count(pm));
        }
    }
}

fn ink_count(pm: &designos_render::tiny_skia::Pixmap) -> usize {
    pm.pixels().iter().filter(|p| p.red() < 128 && p.green() < 128).count()
}

fn hairline(end: f32, fill: bool, align: StrokeAlign) -> T {
    let mut t = T::new();
    t.add(None, |id| {
        let mut n = Node::ellipse(id, Rect::new(10.0, 10.0, 100.0, 100.0));
        n.kind = NodeKind::Ellipse(ArcData { start: 0.0, end, inner_radius: 1.0 });
        n.fills = if fill { vec![Paint::solid(Color::BLACK)] } else { vec![] };
        if !fill {
            n.stroke.paints = vec![Paint::solid(Color::BLACK)];
            n.stroke.weight = 6.0;
            n.stroke.align = align;
        }
        n
    });
    t
}

#[test]
fn a_ring_without_width_has_no_fill_and_keeps_its_inside_stroke_on_both_renderers() {
    let Some(mut g) = gpu() else { return };
    for (name, end, fill) in [("hairline_fill_full", std::f32::consts::TAU, true), ("hairline_fill_half", std::f32::consts::PI, true)] {
        g.compiler.geom.clear();
        let t = hairline(end, fill, StrokeAlign::Center);
        let c = compare(&mut g, name, &t.doc, t.page, Viewport::default(), (120, 120), 8);
        assert_eq!(ink_count(&c.cpu), 0, "{name}: cpu draws no disc");
        assert_eq!(ink_count(&c.gpu), 0, "{name}: gpu draws no disc");
    }
    g.compiler.geom.clear();
    let t = hairline(std::f32::consts::TAU, false, StrokeAlign::Inside);
    let c = compare(&mut g, "hairline_inside_stroke", &t.doc, t.page, Viewport::default(), (120, 120), 8);
    assert!(ink_count(&c.cpu) > 500 && ink_count(&c.gpu) > 500, "cpu {} gpu {}", ink_count(&c.cpu), ink_count(&c.gpu));
    // a thin curved band: nearly all mismatches are edge pixels
    assert!(c.report.ratio < 0.05 && c.report.ssim > 0.98, "{:?}", c.report);
}

#[test]
fn an_inside_stroke_survives_a_shape_with_no_fill_area_on_both_renderers() {
    use designos_doc::vector::{Segment, Vertex};
    let Some(mut g) = gpu() else { return };
    let sq = |lo: f32, hi: f32| {
        vec![
            PathCmd::MoveTo(Vec2::new(lo, lo)),
            PathCmd::LineTo(Vec2::new(hi, lo)),
            PathCmd::LineTo(Vec2::new(hi, hi)),
            PathCmd::LineTo(Vec2::new(lo, hi)),
            PathCmd::Close,
        ]
    };
    let mut t = T::new();
    t.add(None, |id| {
        let mut n = Node::rectangle(id, Rect::new(10.0, 10.0, 100.0, 100.0));
        let net = VectorNetwork {
            vertices: vec![
                Vertex::new(Vec2::new(0.0, 0.0)),
                Vertex::new(Vec2::new(100.0, 0.0)),
                Vertex::new(Vec2::new(100.0, 100.0)),
                Vertex::new(Vec2::new(0.0, 100.0)),
            ],
            segments: (0..4).map(|i| Segment { a: i, b: (i + 1) % 4, ta: Vec2::new(0.0, 0.0), tb: Vec2::new(0.0, 0.0) }).collect(),
            regions: vec![],
        };
        n.kind = NodeKind::Vector { paths: vec![Path { cmds: sq(0.0, 100.0), fill_rule: FillRule::NonZero }], network: Some(net) };
        n.fills.clear();
        n.stroke.paints = vec![Paint::solid(Color::BLACK)];
        n.stroke.weight = 5.0;
        n.stroke.align = StrokeAlign::Inside;
        let mut band = sq(-5.0, 105.0);
        band.extend(sq(5.0, 95.0));
        n.stroke_geometry = Some(vec![Path { cmds: band, fill_rule: FillRule::EvenOdd }]);
        n
    });
    let c = compare(&mut g, "regionless_loop_inside_stroke", &t.doc, t.page, Viewport::default(), (120, 120), 8);
    assert!(ink_count(&c.cpu) > 1000 && ink_count(&c.gpu) > 1000, "cpu {} gpu {}", ink_count(&c.cpu), ink_count(&c.gpu));
    assert!(c.report.ratio < 0.03, "{:?}", c.report);
}
