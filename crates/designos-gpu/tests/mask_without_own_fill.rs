//! A frame mask with no fill of its own takes its alpha from its children on the
//! GPU too (the same rule as the CPU renderer), in the headless render and in the
//! interactive compile once the idle refine lifts the program cap.
//! Needs a GPU adapter; each test skips without one.

mod common;

use designos_doc::*;
use designos_render::tiny_skia::Pixmap;
use designos_render::Viewport;
use std::time::Duration;

const SIZE: (u32, u32) = (80, 80);
const RED: Color = Color { r: 1.0, g: 0.0, b: 0.0, a: 1.0 };
const BLUE: Color = Color { r: 0.0, g: 0.0, b: 1.0, a: 1.0 };

/// A mask frame with no fills whose only child covers the top-left quarter,
/// followed by a red rectangle over the whole page.
fn doc() -> (Document, NodeId) {
    let mut b = common::T::new();
    let mask = b.add(None, |id| {
        let mut n = Node::frame(id, Rect::new(0.0, 0.0, 80.0, 80.0));
        n.fills.clear();
        n.is_mask = true;
        n
    });
    b.rect(Some(mask), Rect::new(0.0, 0.0, 40.0, 40.0), BLUE);
    b.rect(None, Rect::new(0.0, 0.0, 80.0, 80.0), RED);
    (b.doc, b.page)
}

fn px(pm: &Pixmap, x: u32, y: u32) -> [u8; 4] {
    let p = pm.pixel(x, y).unwrap();
    [p.red(), p.green(), p.blue(), p.alpha()]
}

fn assert_cut_by_the_child(pm: &Pixmap, what: &str) {
    assert_eq!(px(pm, 20, 20), [255, 0, 0, 255], "{what}: inside the child of the mask");
    assert_eq!(px(pm, 60, 60), [255, 255, 255, 255], "{what}: inside the mask frame but outside its child");
    assert_eq!(px(pm, 60, 20), [255, 255, 255, 255], "{what}: right of the child");
}

#[test]
fn headless_gpu_matches_the_cpu_for_a_frame_mask_without_fill() {
    let Some(mut g) = common::gpu() else { return };
    let (doc, page) = doc();
    let vp = Viewport { offset: Vec2::new(0.0, 0.0), zoom: 1.0, dpr: 1.0 };
    let c = common::compare(&mut g, "mask-frame-without-fill", &doc, page, vp, SIZE, 4);
    assert_cut_by_the_child(&c.cpu, "cpu");
    assert_cut_by_the_child(&c.gpu, "gpu");
    assert_eq!(c.report.mismatched, 0, "gpu and cpu agree pixel for pixel");
}

#[test]
fn refined_interactive_gpu_frame_cuts_by_the_children() {
    let Some(mut g) = common::gpu() else { return };
    let (doc, page) = doc();
    let vp = Viewport { offset: Vec2::new(0.0, 0.0), zoom: 1.0, dpr: 1.0 };
    g.compiler.geom.budget = Some(Duration::from_millis(8));
    g.compiler.opts.uncap_programs = true;
    let pm = g.render_page(&doc, page, &vp, SIZE);
    assert_cut_by_the_child(&pm, "refined");
}
