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
fn a_stretched_polygon_keeps_the_points_on_its_ellipse() {
    let t = shape(NodeKind::Polygon { count: 4 }, Vec2::new(100.0, 40.0));
    let pm = t.render(120, 80);
    // a diamond on a 100x40 ellipse: tips at the middle of each edge
    assert!(is_ink(&pm, 60, 11) && is_ink(&pm, 11, 30) && is_ink(&pm, 108, 30) && is_ink(&pm, 60, 48));
    assert!(!is_ink(&pm, 14, 13));
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
