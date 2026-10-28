//! #520: CPU and GPU agree on the fixture pair of `520-spec.md` section 2 (white frame 1, fill-less
//! frame 2 under rule F2). Needs a hardware GPU adapter and skips without one.

mod common;

use designos_doc::*;
use designos_render::Viewport;

const SIZE: (u32, u32) = (640, 360);
const BLUE_A: Color = Color { r: 0x25 as f32 / 255.0, g: 0x63 as f32 / 255.0, b: 0xEB as f32 / 255.0, a: 1.0 };
const RED_B: Color = Color { r: 0xDC as f32 / 255.0, g: 0x26 as f32 / 255.0, b: 0x26 as f32 / 255.0, a: 0.5 };

fn frame(t: &mut common::T, x: f32, white: bool, show_behind: bool) {
    let f = t.add(None, |id| {
        let mut n = Node::frame(id, Rect::new(x, 60.0, 240.0, 160.0));
        n.fills = if white { vec![Paint::solid(Color::WHITE)] } else { vec![] };
        n.effects = vec![Effect::DropShadow {
            color: Color { r: 0.0, g: 0.0, b: 0.0, a: 0.5 },
            offset: Vec2::new(0.0, 12.0),
            radius: 24.0,
            spread: 0.0,
            visible: true,
            show_behind,
            blend: Default::default(),
        }];
        n
    });
    t.rect(Some(f), Rect::new(60.0, 50.0, 120.0, 60.0), BLUE_A);
    t.rect(Some(f), Rect::new(20.0, 110.0, 40.0, 30.0), RED_B);
}

fn pair(show_behind: bool) -> (Document, NodeId) {
    let mut t = common::T::new();
    frame(&mut t, 40.0, true, show_behind);
    frame(&mut t, 360.0, false, show_behind);
    (t.doc, t.page)
}

#[test]
fn cpu_and_gpu_match_on_the_fixture_pair() {
    let Some(mut g) = common::gpu() else { return };
    let (doc, page) = pair(false);
    let vp = Viewport { offset: Vec2::new(0.0, 0.0), zoom: 1.0, dpr: 1.0 };
    let c = common::compare(&mut g, "fillless-fixture-pair", &doc, page, vp, SIZE, 8);
    assert!(c.report.ratio < 0.02, "strict mismatch {:.3}%", c.report.ratio * 100.0);
    // frame 2 probes P1 (inside, under child A's shadow), P2 (centre of child B), P3 (below): the
    // GPU pixel is the CPU pixel, so the GPU follows rule F2 wherever the CPU does
    for (dx, dy) in [(120, 128), (40, 125), (120, 170)] {
        let (x, y) = (360 + dx, 60 + dy);
        let (a, b) = (c.cpu.pixel(x, y).unwrap(), c.gpu.pixel(x, y).unwrap());
        let d = [a.red().abs_diff(b.red()), a.green().abs_diff(b.green()), a.blue().abs_diff(b.blue())];
        assert!(d.iter().all(|v| *v <= 8), "probe ({dx}, {dy}): cpu {a:?} gpu {b:?}");
    }
    // P1 carries shadow on the GPU too: it is darker than the white page
    assert!(c.gpu.pixel(480, 188).unwrap().red() < 245, "frame 2 P1 keeps the shadow on the GPU");
}

#[test]
fn cpu_and_gpu_match_on_the_fixture_pair_with_show_behind() {
    let Some(mut g) = common::gpu() else { return };
    let (doc, page) = pair(true);
    let vp = Viewport { offset: Vec2::new(0.0, 0.0), zoom: 1.0, dpr: 1.0 };
    let c = common::compare(&mut g, "fillless-fixture-pair-behind", &doc, page, vp, SIZE, 8);
    assert!(c.report.ratio < 0.02, "strict mismatch {:.3}%", c.report.ratio * 100.0);
}
