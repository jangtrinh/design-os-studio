//! The dash of a FigJam board item (a connector, a shape with text, a sticky) runs on continuously from the start of
//! its path, 10 on and 10 off, as it always did: the kit has no dashed board item that fits the one-pattern-per-segment
//! rule of the design canvas, and every renderer draws a board item as its lowered body (a vector or a rectangle).

use crate::common;

use common::*;
use designos_doc::*;

fn dashed(n: &mut Node) {
    n.stroke.paints = vec![Paint::solid(RED)];
    n.stroke.weight = 4.0;
    n.stroke.align = StrokeAlign::Center;
    n.stroke.cap = StrokeCap::None;
    n.stroke.dashes = vec![10.0, 10.0];
}

fn is_red(p: [u8; 4]) -> bool {
    p[0] > 200 && p[1] < 80 && p[2] < 80
}

/// A square shape with text at (20, 20), 100 x 100: its top side runs from x = 20, the dash covers x = 20..30, the gap
/// 30..40. (The one-pattern-per-segment rule would put half a dash at 20..25 and a dash at 35..45.)
#[test]
fn a_dashed_square_shape_with_text_keeps_the_continuous_dash() {
    let mut t = T::new();
    t.add(None, |id| {
        let mut n = Node::shape_with_text(id, Rect::new(20.0, 20.0, 100.0, 100.0), ShapeKind::Square, "");
        dashed(&mut n);
        n
    });
    let pm = t.render(140, 140);
    assert!(is_red(px(&pm, 27, 19)), "the first dash runs to x = 30: {:?}", px(&pm, 27, 19));
    assert!(!is_red(px(&pm, 37, 19)), "the gap runs from 30 to 40: {:?}", px(&pm, 37, 19));
}

/// A dashed elbow connector between two notes: along its first leg the dash covers 0..10 from the start of the route,
/// the gap 10..20.
#[test]
fn a_dashed_elbow_connector_keeps_the_continuous_dash() {
    let mut t = T::new();
    let a = t.add(None, |id| Node::sticky(id, Rect::new(20.0, 20.0, 80.0, 80.0), ""));
    let b = t.add(None, |id| Node::sticky(id, Rect::new(260.0, 200.0, 80.0, 80.0), ""));
    let c = t.add(None, |id| {
        let p = ConnectorProps { start: ConnectorEnd::attached(a, Magnet::Auto), end: ConnectorEnd::attached(b, Magnet::Auto), ..Default::default() };
        let mut n = Node::connector(id, p);
        dashed(&mut n);
        n
    });
    sync_connector(&mut t.doc, c);
    let route = connector_route(&t.doc, c).unwrap();
    assert!(route.pts.len() >= 3, "an elbow");
    let (p0, p1) = (route.pts[0], route.pts[1]);
    let len = ((p1.x - p0.x).powi(2) + (p1.y - p0.y).powi(2)).sqrt();
    assert!(len > 40.0);
    let at = |d: f32| (((p0.x + (p1.x - p0.x) * d / len) as u32), ((p0.y + (p1.y - p0.y) * d / len) as u32));
    let pm = t.render(360, 300);
    let (x7, y7) = at(7.0);
    let (x17, y17) = at(17.0);
    assert!(is_red(px(&pm, x7, y7)), "the first dash runs to 10 along the first leg: {:?}", px(&pm, x7, y7));
    assert!(!is_red(px(&pm, x17, y17)), "the gap runs from 10 to 20: {:?}", px(&pm, x17, y17));
}
