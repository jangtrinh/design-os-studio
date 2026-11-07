//! Basic shapes drawn from their primitive follow Figma: polygon and star points sit on the ellipse
//! inscribed in the box, and a ring with no width is one circle.

mod common;

use common::*;
use designos_doc::*;
use designos_render::tiny_skia::Pixmap;

fn shape(kind: NodeKind, size: Vec2) -> T {
    let mut t = T::new();
    t.add(None, |id| {
        let mut n = Node::ellipse(id, Rect::new(10.0, 10.0, size.x, size.y));
        n.kind = kind;
        n.fills = vec![Paint::solid(Color::BLACK)];
        n
    });
    t
}

fn is_ink(pm: &Pixmap, x: u32, y: u32) -> bool {
    px(pm, x, y)[0] < 128
}

/// Lowest inked row.
fn bottom(pm: &Pixmap) -> u32 {
    (0..pm.height()).rev().find(|y| (0..pm.width()).any(|x| is_ink(pm, x, *y))).unwrap()
}

#[test]
fn a_polygon_in_a_square_box_sits_on_the_inscribed_circle() {
    // a heptagon in a 100x100 box: its lowest vertices are at cy + r cos(pi / 7) = 50 + 50 * 0.901, not at the box edge
    let t = shape(NodeKind::Polygon { count: 7 }, Vec2::new(100.0, 100.0));
    let pm = t.render(120, 120);
    let low = bottom(&pm) as f32;
    assert!((low - (10.0 + 50.0 + 50.0 * (std::f32::consts::PI / 7.0).cos())).abs() <= 1.5, "bottom row {low}");
}

#[test]
fn a_star_in_a_square_box_has_its_tip_on_top_and_its_feet_above_the_bottom() {
    let t = shape(NodeKind::Star { count: 5, inner_ratio: 0.382 }, Vec2::new(100.0, 100.0));
    let pm = t.render(120, 120);
    assert!(is_ink(&pm, 60, 12), "the top tip is on the box's top edge");
    let low = bottom(&pm) as f32;
    // outer points at 54 degrees below the horizontal: cy + r sin(54deg)
    assert!((low - (60.0 + 50.0 * 54f32.to_radians().sin())).abs() <= 1.5, "bottom row {low}");
}

#[test]
fn a_stretched_pentagon_keeps_its_points_on_the_ellipse() {
    // five points on a 100 x 40 ellipse: the tip at the top edge, the lower corners at cy + ry sin(54deg) = 36.2, not at the box bottom
    let t = shape(NodeKind::Polygon { count: 5 }, Vec2::new(100.0, 40.0));
    let pm = t.render(120, 80);
    assert!(is_ink(&pm, 60, 11), "tip on the top edge");
    let low = bottom(&pm) as f32;
    assert!((low - (10.0 + 20.0 + 20.0 * 54f32.to_radians().sin())).abs() <= 1.5, "bottom row {low}");
}

fn hairline_ring(end: f32, fill: bool, stroke: Option<(f32, StrokeAlign)>) -> T {
    let mut t = T::new();
    t.add(None, |id| {
        let mut n = Node::ellipse(id, Rect::new(10.0, 10.0, 100.0, 100.0));
        n.kind = NodeKind::Ellipse(ArcData { start: 0.0, end, inner_radius: 1.0 });
        n.fills = if fill { vec![Paint::solid(Color::BLACK)] } else { vec![] };
        if let Some((w, align)) = stroke {
            n.stroke.paints = vec![Paint::solid(Color::BLACK)];
            n.stroke.weight = w;
            n.stroke.align = align;
        }
        n
    });
    t
}

fn ink_count(pm: &Pixmap) -> usize {
    pm.pixels().iter().filter(|p| p.red() < 128).count()
}

#[test]
fn a_filled_ring_without_width_draws_no_fill() {
    let pm = hairline_ring(std::f32::consts::TAU, true, None).render(120, 120);
    assert_eq!(ink_count(&pm), 0, "no area to fill, so no disc");
}

#[test]
fn a_filled_half_arc_without_width_draws_no_fill() {
    let pm = hairline_ring(std::f32::consts::PI, true, None).render(120, 120);
    assert_eq!(ink_count(&pm), 0);
}

#[test]
fn an_inside_stroke_on_a_ring_without_width_still_draws() {
    let pm = hairline_ring(std::f32::consts::TAU, false, Some((6.0, StrokeAlign::Inside))).render(120, 120);
    assert!(is_ink(&pm, 60 + 47, 60), "the stroke band inside the circle is inked");
    assert!(ink_count(&pm) > 500);
}

/// A closed loop with no fill region and no stored fill geometry, carrying the stored double-weight inside band.
fn regionless_loop_with_inside_stroke() -> T {
    use designos_doc::vector::{Segment, Vertex};
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
    t
}

#[test]
fn an_inside_stroke_survives_a_shape_with_no_fill_area() {
    let pm = regionless_loop_with_inside_stroke().render(120, 120);
    assert!(ink_count(&pm) > 1000, "{} inked pixels", ink_count(&pm));
}

#[test]
fn a_ring_without_width_is_one_circle_when_dashed() {
    let mut t = T::new();
    t.add(None, |id| {
        let mut n = Node::ellipse(id, Rect::new(10.0, 10.0, 100.0, 100.0));
        n.kind = NodeKind::Ellipse(ArcData { start: 0.0, end: std::f32::consts::TAU, inner_radius: 1.0 });
        n.fills.clear();
        n.stroke.paints = vec![Paint::solid(Color::BLACK)];
        n.stroke.weight = 6.0;
        n.stroke.align = StrokeAlign::Center;
        n.stroke.dashes = vec![0.1, 30.0];
        n.stroke.cap = StrokeCap::Round;
        n
    });
    let pm = t.render(120, 120);
    // walk the circle the dots sit on and count the separate dots: one per dash, not two
    let steps = 4000;
    let ink: Vec<bool> = (0..steps)
        .map(|i| {
            let a = std::f32::consts::TAU * i as f32 / steps as f32;
            is_ink(&pm, (60.0 + 50.0 * a.cos()).round() as u32, (60.0 + 50.0 * a.sin()).round() as u32)
        })
        .collect();
    let dots = (0..steps).filter(|i| ink[*i] && !ink[(*i + steps - 1) % steps]).count();
    // circumference 314.2 over a 30.1 dash cycle is 10.4 dots
    assert!((9..=12).contains(&dots), "{dots} dots");
}
