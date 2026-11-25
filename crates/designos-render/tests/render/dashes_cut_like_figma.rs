//! A dashed stroke is cut the way Figma stores it: every segment of the path (a side, a corner arc, a network
//! segment) holds a whole number of stretched periods and starts and ends on half a dash, and the half dashes
//! either side of a vertex are one dash. The numbers are the outlines the Untitled UI kit stores for four dashed
//! layers (geometry only): a rounded frame, a dotted vertical line, a dotted horizontal line and a dotted chart.

use crate::common;

use common::*;
use designos_doc::*;
use designos_render::tiny_skia::PathSegment;
use designos_render::{outline_of, stroke_plan};

/// The `[left, top, right, bottom]` of every contour of the outline the stroke of `id` fills.
fn contour_boxes(t: &mut T, id: NodeId) -> Vec<[f32; 4]> {
    let node = t.node(id).clone();
    let shape = outline_of(&t.doc, &node, 0.05).expect("the node has a shape");
    let plan = stroke_plan(&node, &shape, 8.0).expect("the node has a stroke outline");
    let mut boxes: Vec<[f32; 4]> = Vec::new();
    for (path, _) in &plan.outline {
        let mut cur: Option<[f32; 4]> = None;
        let grow = |cur: &mut Option<[f32; 4]>, x: f32, y: f32| {
            let b = cur.get_or_insert([x, y, x, y]);
            *b = [b[0].min(x), b[1].min(y), b[2].max(x), b[3].max(y)];
        };
        for s in path.segments() {
            match s {
                PathSegment::MoveTo(p) => {
                    boxes.extend(cur.take());
                    grow(&mut cur, p.x, p.y);
                }
                PathSegment::LineTo(p) => grow(&mut cur, p.x, p.y),
                PathSegment::QuadTo(c, p) => {
                    grow(&mut cur, c.x, c.y);
                    grow(&mut cur, p.x, p.y);
                }
                PathSegment::CubicTo(c1, c2, p) => {
                    grow(&mut cur, c1.x, c1.y);
                    grow(&mut cur, c2.x, c2.y);
                    grow(&mut cur, p.x, p.y);
                }
                PathSegment::Close => {}
            }
        }
        boxes.extend(cur.take());
    }
    boxes
}

fn has_box(boxes: &[[f32; 4]], want: [f32; 4], tol: f32) -> bool {
    boxes.iter().any(|b| b.iter().zip(&want).all(|(a, w)| (a - w).abs() <= tol))
}

fn dashed(n: &mut Node, weight: f32, cap: StrokeCap, dashes: &[f32]) {
    n.fills.clear();
    n.stroke.paints = vec![Paint::solid(RED)];
    n.stroke.weight = weight;
    n.stroke.align = StrokeAlign::Center;
    n.stroke.cap = cap;
    n.stroke.dashes = dashes.to_vec();
}

fn line_vector(t: &mut T, points: &[(f32, f32)]) -> NodeId {
    let path = Path {
        cmds: points
            .iter()
            .enumerate()
            .map(|(i, (x, y))| if i == 0 { PathCmd::MoveTo(Vec2::new(*x, *y)) } else { PathCmd::LineTo(Vec2::new(*x, *y)) })
            .collect(),
        fill_rule: FillRule::NonZero,
    };
    let net = VectorNetwork::from_paths(std::slice::from_ref(&path));
    t.add(None, |id| Node::new(id, NodeKind::Vector { paths: net.to_paths(), network: Some(net) }))
}

/// A frame of 768 x 134 with corner radius 8 and a dashed `[10, 5]` stroke of weight 1: the top side is 752 long,
/// 50 periods of 15.04; the left side is 118 long, 8 periods of 14.75 (dash 9.833); the corner arc of 12.57 is one
/// period, a half dash at each end.
#[test]
fn a_dashed_rounded_frame_is_cut_side_by_side_and_corner_by_corner() {
    let mut t = T::new();
    let id = t.add(None, |id| {
        let mut n = Node::frame(id, Rect::new(0.0, 0.0, 768.0, 134.0));
        n.corner_radii = [8.0; 4];
        dashed(&mut n, 1.0, StrokeCap::None, &[10.0, 5.0]);
        n
    });
    let boxes = contour_boxes(&mut t, id);
    assert!(has_box(&boxes, [378.987, -0.5, 389.013, 0.5], 0.02), "the dash on the middle of the top side is 10.027 long and centred on 384");
    assert!(has_box(&boxes, [-0.5, 106.333, 0.5, 116.167], 0.02), "a dash of the left side is 9.833 long");
    assert!(has_box(&boxes, [-0.5, 121.083, 1.503, 130.251], 0.05), "half a dash of the left side runs on round the corner into the half dash of its arc");
}

/// A vertical line of two network segments of 8 with the dash `[0.1, 3]`, round caps, weight 1: each segment holds
/// 3 periods of 2.667 and a dot sits at every vertex, so the dots are 2.667 apart and the one at the middle vertex is
/// the two half dots of the segments run together.
#[test]
fn a_dotted_line_has_a_dot_on_every_vertex_of_its_network() {
    let mut t = T::new();
    let id = line_vector(&mut t, &[(0.0, 0.0), (0.0, 8.0), (0.0, 16.0)]);
    dashed(t.node(id), 1.0, StrokeCap::Round, &[0.1, 3.0]);
    let boxes = contour_boxes(&mut t, id);
    assert!(has_box(&boxes, [-0.5, 2.124, 0.5, 3.21], 0.02), "the dot at 2.667");
    assert!(has_box(&boxes, [-0.5, 7.457, 0.5, 8.543], 0.02), "the dot at the middle vertex");
    assert!(!boxes.iter().any(|b| (b[1] - 2.6).abs() < 0.2), "no dot of a pattern that runs on along the whole line, at 3.1");
}

/// A horizontal line of 30 with `[0.1, 6]`, round caps, weight 2: 5 periods of 6.0, dashes of 0.098 centred on 0, 6, 12 ...
#[test]
fn the_dots_of_a_heavier_line_sit_on_whole_periods() {
    let mut t = T::new();
    let id = line_vector(&mut t, &[(0.0, 0.0), (30.0, 0.0)]);
    dashed(t.node(id), 2.0, StrokeCap::Round, &[0.1, 6.0]);
    let boxes = contour_boxes(&mut t, id);
    assert!(has_box(&boxes, [4.951, -1.0, 7.049, 1.0], 0.02), "the dot at 6");
    assert!(has_box(&boxes, [28.951, -1.0, 31.0, 1.0], 0.02), "the half dot at the end of the line");
    assert_eq!(boxes.len(), 6, "a dot at 0, 6 ... 30");
}

/// A segment shorter than its first dash is that dash: the corner arc of a radius 5 corner (7.85 long) under the
/// dash `[10, 5]` has no gap.
#[test]
fn a_segment_shorter_than_the_dash_has_no_gap() {
    let mut t = T::new();
    let id = t.add(None, |id| {
        let mut n = Node::frame(id, Rect::new(0.0, 0.0, 328.0, 272.0));
        n.corner_radii = [5.0; 4];
        dashed(&mut n, 1.0, StrokeCap::None, &[10.0, 5.0]);
        n
    });
    let boxes = contour_boxes(&mut t, id);
    // the left side (262 long, 17 periods) ends on a half dash of 5.137 at y = 267, where the arc starts; the arc is
    // dash from end to end, and the bottom side (318 long, 21 periods) starts on a half dash of 5.048: all one dash
    assert!(has_box(&boxes, [-0.5, 261.863, 10.048, 272.5], 0.05), "the dash runs from the left side through the whole arc into the bottom side");
}
