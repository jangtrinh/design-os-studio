//! The live stroke of a Line (no stored outline) lands where Figma's stored band does: a CENTER line's
//! band sits at local y in [-weight, 0], and a ROUND cap stays inside x in [0, length]. A flat Vector
//! and an INSIDE or OUTSIDE line keep their centred stroke.

mod common;

use common::*;
use designos_doc::*;
use designos_render::tiny_skia::Pixmap;

const ORIGIN: Vec2 = Vec2 { x: 20.0, y: 40.0 };
const VIEW: (u32, u32) = (200, 100);

fn ink_paint() -> Vec<Paint> {
    vec![Paint::solid(Color::rgb(0x18, 0x1D, 0x27))]
}

/// A Line as Figma imports it (CENTER), with no stored outline so it draws live.
fn live_line(t: &mut T, len: f32, w: f32, cap: StrokeCap) -> NodeId {
    t.add(None, |id| {
        let mut n = Node::new(id, NodeKind::Line);
        n.transform = Affine::translate(ORIGIN.x, ORIGIN.y);
        n.size = Vec2::new(len, 0.0);
        n.fills.clear();
        n.stroke.paints = ink_paint();
        n.stroke.weight = w;
        n.stroke.align = StrokeAlign::Center;
        n.stroke.cap = cap;
        n
    })
}

/// The same line carrying Figma's stored band: a rectangle at y in [-w, 0], x in [0, len].
fn stored_band_line(len: f32, w: f32) -> T {
    let mut t = T::new();
    let id = live_line(&mut t, len, w, StrokeCap::None);
    let cmds = vec![
        PathCmd::MoveTo(Vec2::new(0.0, -w)),
        PathCmd::LineTo(Vec2::new(len, -w)),
        PathCmd::LineTo(Vec2::new(len, 0.0)),
        PathCmd::LineTo(Vec2::new(0.0, 0.0)),
        PathCmd::Close,
    ];
    t.node(id).stroke_geometry = Some(vec![Path { cmds, fill_rule: FillRule::NonZero }]);
    t
}

fn dark(pm: &Pixmap, x: u32, y: u32) -> bool {
    pm.pixel(x, y).unwrap().red() < 128
}

/// (rows min, rows max, cols min, cols max) of dark pixels.
fn ink(pm: &Pixmap) -> (u32, u32, u32, u32) {
    let (mut y0, mut y1, mut x0, mut x1) = (u32::MAX, 0, u32::MAX, 0);
    for y in 0..pm.height() {
        for x in 0..pm.width() {
            if dark(pm, x, y) {
                (y0, y1, x0, x1) = (y0.min(y), y1.max(y), x0.min(x), x1.max(x));
            }
        }
    }
    (y0, y1, x0, x1)
}

#[test]
fn a_live_center_line_has_the_rows_of_figmas_stored_band_at_every_weight() {
    for w in [1.0f32, 2.0, 10.0] {
        let mut live = T::new();
        live_line(&mut live, 100.0, w, StrokeCap::None);
        let stored = stored_band_line(100.0, w);
        let (a, b) = (live.render(VIEW.0, VIEW.1), stored.render(VIEW.0, VIEW.1));
        let top = ORIGIN.y as u32 - w as u32;
        assert_eq!(ink(&a), (top, ORIGIN.y as u32 - 1, 20, 119), "w {w}: live rows are [-w, 0]");
        assert_eq!(a.data(), b.data(), "w {w}: live equals the stored band pixel for pixel");
    }
}

#[test]
fn square_and_dashed_center_lines_keep_the_centred_stroke() {
    // only a solid CENTER line with a butt or round cap was measured in the kit; the rest stays as on main
    let mut t = T::new();
    live_line(&mut t, 100.0, 10.0, StrokeCap::Square);
    assert_eq!(ink(&t.render(VIEW.0, VIEW.1)), (35, 44, 15, 124), "square cap");
    for cap in [StrokeCap::None, StrokeCap::Round] {
        let mut t = T::new();
        let id = live_line(&mut t, 100.0, 10.0, cap);
        t.node(id).stroke.dashes = vec![20.0, 10.0];
        let (y0, y1, ..) = ink(&t.render(VIEW.0, VIEW.1));
        assert_eq!((y0, y1), (35, 44), "dashed {cap:?}");
    }
}

#[test]
fn a_round_cap_stays_inside_zero_to_length() {
    let mut t = T::new();
    live_line(&mut t, 100.0, 10.0, StrokeCap::Round);
    let pm = t.render(VIEW.0, VIEW.1);
    assert_eq!(ink(&pm), (30, 39, 20, 119), "the stadium spans exactly [0, L]");
    // the middle of each end cap is solid; just past each end is empty
    assert!(dark(&pm, 21, 35) && dark(&pm, 118, 35));
    assert!(!dark(&pm, 19, 35) && !dark(&pm, 120, 35));
    // a semicircle of radius 5 is rounded: the end's corner pixel is empty
    assert!(!dark(&pm, 20, 30) && !dark(&pm, 119, 39));
}

#[test]
fn a_thin_round_cap_line_at_weight_one_spans_its_length() {
    let mut t = T::new();
    live_line(&mut t, 100.0, 1.0, StrokeCap::Round);
    let pm = t.render(VIEW.0, VIEW.1);
    let (y0, y1, x0, x1) = ink(&pm);
    assert_eq!((y0, y1), (39, 39));
    assert!((20..=21).contains(&x0) && (118..=119).contains(&x1), "x {x0}..{x1}");
}

#[test]
fn a_round_cap_line_shorter_than_its_weight_still_draws() {
    let mut t = T::new();
    live_line(&mut t, 4.0, 10.0, StrokeCap::Round);
    let (y0, y1, ..) = ink(&t.render(VIEW.0, VIEW.1));
    assert!(y0 <= y1, "a dot is drawn");
}

#[test]
fn a_rotated_line_keeps_the_same_local_band() {
    let (len, w) = (100.0f32, 10.0f32);
    let theta = 30.0f32.to_radians();
    let mut t = T::new();
    let id = live_line(&mut t, len, w, StrokeCap::None);
    t.node(id).transform = Affine { tx: 60.0, ty: 20.0, ..Affine::rotate(theta) };
    let pm = t.render(240, 200);
    let m = t.doc.get(id).unwrap().transform;
    for x in [20.0f32, 50.0, 80.0] {
        let at = |y: f32| {
            let p = m.apply(Vec2::new(x, y));
            dark(&pm, p.x.round() as u32, p.y.round() as u32)
        };
        assert!(at(-w / 2.0) && at(-w + 1.5) && at(-1.5), "x {x}: the band covers local y -w..0");
        assert!(!at(w / 2.0) && !at(2.0) && !at(-w - 2.0), "x {x}: nothing below the axis or above the band");
    }
}

#[test]
fn inside_and_outside_alignment_on_a_line_keep_the_centred_stroke() {
    for align in [StrokeAlign::Inside, StrokeAlign::Outside] {
        let mut t = T::new();
        let id = live_line(&mut t, 100.0, 10.0, StrokeCap::None);
        t.node(id).stroke.align = align;
        assert_eq!(ink(&t.render(VIEW.0, VIEW.1)), (35, 44, 20, 119), "{align:?}");
    }
}

#[test]
fn a_flat_vector_keeps_its_centred_stroke_and_overhanging_round_caps() {
    let mut t = T::new();
    t.add(None, |id| {
        let path = Path { cmds: vec![PathCmd::MoveTo(Vec2::ZERO), PathCmd::LineTo(Vec2::new(100.0, 0.0))], fill_rule: FillRule::NonZero };
        let mut n = Node::new(id, NodeKind::Vector { paths: vec![path], network: None });
        n.transform = Affine::translate(ORIGIN.x, ORIGIN.y);
        n.size = Vec2::new(100.0, 0.0);
        n.fills.clear();
        n.stroke.paints = ink_paint();
        n.stroke.weight = 10.0;
        n.stroke.align = StrokeAlign::Center;
        n.stroke.cap = StrokeCap::Round;
        n
    });
    assert_eq!(ink(&t.render(VIEW.0, VIEW.1)), (35, 44, 15, 124));
}

#[test]
fn a_stored_outline_still_wins_over_the_live_stroke() {
    let mut t = stored_band_line(100.0, 10.0);
    let id = t.doc.get(t.page).unwrap().children[0];
    // re-stamp for the round cap so the stored band is current; a live round cap would clear its corner
    t.node(id).stroke.cap = StrokeCap::Round;
    t.node(id).stroke_geometry_for = Some(StrokeGeometryStamp::of_node(t.doc.get(id).unwrap()));
    let pm = t.render(VIEW.0, VIEW.1);
    assert_eq!(ink(&pm), (30, 39, 20, 119));
    assert!(dark(&pm, 20, 30), "the stored rectangle has square corners, a live round cap would not");
}
