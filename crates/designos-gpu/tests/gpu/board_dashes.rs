//! The dash of a FigJam board item runs on continuously from the start of its path on the GPU as on the CPU: the kit has
//! no dashed board item that fits the one-pattern-per-segment rule of the design canvas, and the GPU draws a board item
//! as its lowered body (a vector or a rectangle). Tests print a note and skip the GPU half when the machine has no GPU.

use crate::common;

use common::*;
use designos_doc::*;
use designos_render::tiny_skia::Pixmap;
use designos_render::{render_page, Viewport};

fn dashed(n: &mut Node) {
    n.stroke.paints = vec![Paint::solid(Color { r: 1.0, g: 0.0, b: 0.0, a: 1.0 })];
    n.stroke.weight = 4.0;
    n.stroke.align = StrokeAlign::Center;
    n.stroke.cap = StrokeCap::None;
    n.stroke.dashes = vec![10.0, 10.0];
}

fn px(pm: &Pixmap, x: u32, y: u32) -> [u8; 4] {
    let p = pm.pixel(x, y).unwrap();
    [p.red(), p.green(), p.blue(), p.alpha()]
}

fn is_red(p: [u8; 4]) -> bool {
    p[0] > 200 && p[1] < 80 && p[2] < 80
}

/// Both renderers draw `t`, and `check` holds for each picture.
fn on_cpu_and_gpu(t: &T, name: &str, size: (u32, u32), check: impl Fn(&str, &Pixmap)) {
    let vp = Viewport::default();
    let mut cpu = Pixmap::new(size.0, size.1).unwrap();
    render_page(&t.doc, t.page, &vp, &mut cpu);
    check("CPU", &cpu);
    if let Some(mut g) = gpu() {
        let c = compare(&mut g, name, &t.doc, t.page, vp, size, 8);
        check("GPU", &c.gpu);
    }
}

#[test]
fn a_dashed_square_shape_with_text_keeps_the_continuous_dash_on_cpu_and_gpu() {
    let mut t = T::new();
    t.add(None, |id| {
        let mut n = Node::shape_with_text(id, Rect::new(20.0, 20.0, 100.0, 100.0), ShapeKind::Square, "");
        dashed(&mut n);
        n
    });
    on_cpu_and_gpu(&t, "board_dashed_square_shape", (140, 140), |who, pm| {
        assert!(is_red(px(pm, 27, 19)), "{who}: the first dash runs to x = 30: {:?}", px(pm, 27, 19));
        assert!(!is_red(px(pm, 37, 19)), "{who}: the gap runs from 30 to 40: {:?}", px(pm, 37, 19));
    });
}

#[test]
fn a_dashed_elbow_connector_keeps_the_continuous_dash_on_cpu_and_gpu() {
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
    let (p0, p1) = (route.pts[0], route.pts[1]);
    let len = ((p1.x - p0.x).powi(2) + (p1.y - p0.y).powi(2)).sqrt();
    assert!(route.pts.len() >= 3 && len > 40.0, "an elbow with a long first leg");
    let at = |d: f32| ((p0.x + (p1.x - p0.x) * d / len) as u32, (p0.y + (p1.y - p0.y) * d / len) as u32);
    on_cpu_and_gpu(&t, "board_dashed_elbow_connector", (360, 300), |who, pm| {
        let ((x7, y7), (x17, y17)) = (at(7.0), at(17.0));
        assert!(is_red(px(pm, x7, y7)), "{who}: the first dash runs to 10 along the first leg: {:?}", px(pm, x7, y7));
        assert!(!is_red(px(pm, x17, y17)), "{who}: the gap runs from 10 to 20: {:?}", px(pm, x17, y17));
    });
}
