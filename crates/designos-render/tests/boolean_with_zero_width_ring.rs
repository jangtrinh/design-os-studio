//! A ring whose inner radius is 100% has no area. Inside a boolean it is a zero-area operand, not a
//! missing one: intersecting with it is empty, subtracting with it first is empty, and union or exclude
//! leave the other operand as it is.

mod common;

use common::*;
use designos_doc::*;
use designos_render::tiny_skia::Pixmap;

fn scene(op: BooleanOp, ring_first: bool) -> Pixmap {
    let mut t = T::new();
    let group = t.add(None, |id| {
        let mut n = Node::new(id, NodeKind::BooleanOp(op));
        n.transform = Affine::translate(10.0, 10.0);
        n.size = Vec2::new(100.0, 100.0);
        n.fills = vec![Paint::solid(Color::BLACK)];
        n
    });
    let ring = |id| {
        let mut n = Node::ellipse(id, Rect::new(0.0, 0.0, 100.0, 100.0));
        n.kind = NodeKind::Ellipse(ArcData { start: 0.0, end: std::f32::consts::TAU, inner_radius: 1.0 });
        n
    };
    let rect = |id| Node::rectangle(id, Rect::new(0.0, 0.0, 100.0, 100.0));
    if ring_first {
        t.add(Some(group), ring);
        t.add(Some(group), rect);
    } else {
        t.add(Some(group), rect);
        t.add(Some(group), ring);
    }
    t.render(120, 120)
}

fn line_scene(op: BooleanOp, line_first: bool) -> Pixmap {
    let mut t = T::new();
    let group = t.add(None, |id| {
        let mut n = Node::new(id, NodeKind::BooleanOp(op));
        n.transform = Affine::translate(10.0, 10.0);
        n.size = Vec2::new(100.0, 100.0);
        n.fills = vec![Paint::solid(Color::BLACK)];
        n
    });
    let line = |id| {
        let mut n = Node::new(id, NodeKind::Line);
        n.size = Vec2::new(100.0, 0.0);
        n
    };
    let rect = |id| Node::rectangle(id, Rect::new(0.0, 0.0, 100.0, 100.0));
    if line_first {
        t.add(Some(group), line);
        t.add(Some(group), rect);
    } else {
        t.add(Some(group), rect);
        t.add(Some(group), line);
    }
    t.render(120, 120)
}

fn nested_empty_scene(op: BooleanOp) -> Pixmap {
    let mut t = T::new();
    let outer = t.add(None, |id| {
        let mut n = Node::new(id, NodeKind::BooleanOp(op));
        n.transform = Affine::translate(10.0, 10.0);
        n.size = Vec2::new(100.0, 100.0);
        n.fills = vec![Paint::solid(Color::BLACK)];
        n
    });
    t.add(Some(outer), |id| Node::rectangle(id, Rect::new(0.0, 0.0, 100.0, 100.0)));
    let inner = t.add(Some(outer), |id| {
        let mut n = Node::new(id, NodeKind::BooleanOp(BooleanOp::Union));
        n.size = Vec2::new(100.0, 100.0);
        n
    });
    let mut line = Node::new(t.doc.alloc_id(), NodeKind::Line);
    line.size = Vec2::new(100.0, 0.0);
    t.doc.insert(inner, line).unwrap();
    t.render(120, 120)
}

fn ink(pm: &Pixmap) -> usize {
    pm.pixels().iter().filter(|p| p.red() < 128).count()
}

#[test]
fn intersecting_with_a_ring_without_width_is_empty() {
    assert_eq!(ink(&scene(BooleanOp::Intersect, false)), 0);
    assert_eq!(ink(&scene(BooleanOp::Intersect, true)), 0);
}

#[test]
fn subtracting_from_a_ring_without_width_is_empty() {
    assert_eq!(ink(&scene(BooleanOp::Subtract, true)), 0);
}

#[test]
fn union_and_exclude_leave_the_other_operand() {
    for op in [BooleanOp::Union, BooleanOp::Exclude] {
        for ring_first in [false, true] {
            let n = ink(&scene(op, ring_first));
            assert!((9900..=10100).contains(&n), "{op:?} ring_first={ring_first}: {n} inked pixels, the square is 10000");
        }
    }
}

#[test]
fn subtracting_a_ring_without_width_keeps_the_first_operand() {
    let n = ink(&scene(BooleanOp::Subtract, false));
    assert!((9900..=10100).contains(&n), "{n}");
}

#[test]
fn an_open_line_is_a_zero_area_boolean_operand() {
    assert_eq!(ink(&line_scene(BooleanOp::Intersect, false)), 0);
    assert_eq!(ink(&line_scene(BooleanOp::Intersect, true)), 0);
    assert_eq!(ink(&line_scene(BooleanOp::Subtract, true)), 0);
    assert!((9900..=10100).contains(&ink(&line_scene(BooleanOp::Subtract, false))));
    for op in [BooleanOp::Union, BooleanOp::Exclude] {
        for line_first in [false, true] {
            let n = ink(&line_scene(op, line_first));
            assert!((9900..=10100).contains(&n), "{op:?} line_first={line_first}: {n} inked pixels, the square is 10000");
        }
    }
}

#[test]
fn an_empty_boolean_result_remains_an_operand_when_nested() {
    let intersect = ink(&nested_empty_scene(BooleanOp::Intersect));
    let subtract = ink(&nested_empty_scene(BooleanOp::Subtract));
    assert_eq!(intersect, 0);
    assert!((9900..=10100).contains(&subtract));
}
