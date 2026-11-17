//! The live stroke of a Line (no stored outline) lands where Figma's band does, in both renderers: a
//! CENTER line's band sits at local y in [-weight, 0] and a ROUND cap stays inside x in [0, length].
//! The GPU half is skipped when the machine has no GPU adapter; the CPU half always runs.

mod common;

use common::*;
use designos_doc::*;
use designos_render::tiny_skia::Pixmap;
use designos_render::{render_page, Viewport};

const VIEW: (u32, u32) = (240, 120);
const AT: Vec2 = Vec2 { x: 20.0, y: 50.0 };

fn line(t: &mut T, len: f32, w: f32, cap: StrokeCap, rot: f32) -> NodeId {
    t.add(None, |id| {
        let mut n = Node::new(id, NodeKind::Line);
        n.transform = Affine { tx: AT.x, ty: AT.y, ..Affine::rotate(rot) };
        n.size = Vec2::new(len, 0.0);
        n.fills.clear();
        n.stroke.paints = vec![Paint::solid(Color::rgb(0x18, 0x1D, 0x27))];
        n.stroke.weight = w;
        n.stroke.align = StrokeAlign::Center;
        n.stroke.cap = cap;
        n
    })
}

/// (rows min, rows max, cols min, cols max) of dark pixels.
fn ink(pm: &Pixmap) -> (u32, u32, u32, u32) {
    let (mut y0, mut y1, mut x0, mut x1) = (u32::MAX, 0, u32::MAX, 0);
    for y in 0..pm.height() {
        for x in 0..pm.width() {
            if pm.pixel(x, y).unwrap().red() < 128 {
                (y0, y1, x0, x1) = (y0.min(y), y1.max(y), x0.min(x), x1.max(x));
            }
        }
    }
    (y0, y1, x0, x1)
}

fn cpu(t: &T) -> Pixmap {
    let mut pm = Pixmap::new(VIEW.0, VIEW.1).unwrap();
    render_page(&t.doc, t.page, &Viewport::default(), &mut pm);
    pm
}

/// Both renderers draw the same ink box, and their pixels agree.
fn check(name: &str, t: &T, want: Option<(u32, u32, u32, u32)>) {
    let c = cpu(t);
    if let Some(w) = want {
        assert_eq!(ink(&c), w, "{name}: CPU ink box");
    }
    let Some(mut g) = gpu() else { return };
    let cmp = compare(&mut g, name, &t.doc, t.page, Viewport::default(), VIEW, 8);
    assert_eq!(ink(&cmp.cpu), ink(&c));
    if let Some(w) = want {
        assert_eq!(ink(&cmp.gpu), w, "{name}: GPU ink box");
    }
    assert!(cmp.report.ratio < 0.02, "{name}: mismatch {:.3}%", cmp.report.ratio * 100.0);
    assert!(cmp.report.ssim > 0.97, "{name}: {:?}", cmp.report);
}

#[test]
fn center_none_lines_land_on_figmas_band_at_weights_1_2_and_10() {
    for (w, top) in [(1.0f32, 49u32), (2.0, 48), (10.0, 40)] {
        let mut t = T::new();
        line(&mut t, 100.0, w, StrokeCap::None, 0.0);
        check(&format!("live_line_none_w{w}"), &t, Some((top, 49, 20, 119)));
    }
}

#[test]
fn a_round_cap_line_spans_zero_to_length_on_cpu_and_gpu() {
    let mut t = T::new();
    line(&mut t, 100.0, 10.0, StrokeCap::Round, 0.0);
    check("live_line_round_w10", &t, Some((40, 49, 20, 119)));
}

#[test]
fn a_rotated_line_matches_between_cpu_and_gpu() {
    for cap in [StrokeCap::None, StrokeCap::Round] {
        let mut t = T::new();
        line(&mut t, 100.0, 10.0, cap, 30.0f32.to_radians());
        check(&format!("live_line_rot30_{cap:?}"), &t, None);
    }
}

#[test]
fn changing_the_cap_of_a_drawn_line_redraws_it_on_the_gpu() {
    let mut t = T::new();
    let id = line(&mut t, 100.0, 10.0, StrokeCap::None, 0.0);
    let Some(mut g) = gpu() else { return };
    // the compiler reuses geometry within one document revision, so each state gets its own
    let vp = Viewport::default();
    let frame = g.compile(&t.doc, t.page, 1, &vp, VIEW);
    let first = g.render_frame(&frame, VIEW);
    assert_eq!(ink(&first), (40, 49, 20, 119));
    assert!(first.pixel(20, 40).unwrap().red() < 128, "a butt end keeps its corner");
    t.node(id).stroke.cap = StrokeCap::Round;
    let frame = g.compile(&t.doc, t.page, 2, &vp, VIEW);
    let second = g.render_frame(&frame, VIEW);
    assert_eq!(ink(&second), (40, 49, 20, 119), "the rounded band still spans [0, L]");
    assert!(second.pixel(20, 40).unwrap().red() > 200, "a round end clears the corner");
    let c = compare(&mut g, "live_line_cap_toggle", &t.doc, t.page, vp, VIEW, 8);
    assert!(c.report.ratio < 0.02, "mismatch {:.3}%", c.report.ratio * 100.0);
}
