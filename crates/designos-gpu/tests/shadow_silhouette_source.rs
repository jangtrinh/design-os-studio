//! CPU and GPU draw the same drop-shadow silhouette for every silhouette source chosen by
//! `designos_doc::silhouette_source`: own fill, children of a fill-less frame (clipped or not),
//! a hidden fill, and a stroke-only node. Pixel tests need a GPU adapter and skip without one.

mod common;

use designos_doc::*;
use designos_render::Viewport;

const SIZE: (u32, u32) = (120, 120);
const RED: Color = Color { r: 1.0, g: 0.0, b: 0.0, a: 1.0 };

fn shadow() -> Effect {
    Effect::DropShadow {
        color: Color::BLACK,
        offset: Vec2::new(8.0, 8.0),
        radius: 6.0,
        spread: 0.0,
        visible: true,
        show_behind: false,
        blend: Default::default(),
    }
}

/// One shadowed 60x60 node at (20, 20) shaped by `f`, with a red child sticking out of the
/// node's box on the right.
fn doc(f: impl FnOnce(&mut Node)) -> (Document, NodeId) {
    let mut b = common::T::new();
    let frame = b.add(None, |id| {
        let mut n = Node::frame(id, Rect::new(20.0, 20.0, 60.0, 60.0));
        n.effects = vec![shadow()];
        f(&mut n);
        n
    });
    b.rect(Some(frame), Rect::new(10.0, 10.0, 70.0, 20.0), RED);
    (b.doc, b.page)
}

fn parity(name: &str, f: impl FnOnce(&mut Node)) {
    let Some(mut g) = common::gpu() else { return };
    let (doc, page) = doc(f);
    let vp = Viewport { offset: Vec2::new(0.0, 0.0), zoom: 1.0, dpr: 1.0 };
    let c = common::compare(&mut g, name, &doc, page, vp, SIZE, 4);
    // Same budget as the other GPU parity tests (fidelity.rs): anti-aliased edge pixels
    // differ between Metal/Vulkan and tiny-skia, so a pixel-exact rule fails on real GPUs
    // (12 px of 14400 at tolerance 4 on Apple Paravirtual, ssim 0.9994).
    assert!(c.report_aa.ratio < 0.005, "{name}: mismatch with AA forgiven {:.3}%", c.report_aa.ratio * 100.0);
    assert!(c.report.ratio < 0.02, "{name}: strict mismatch {:.3}%", c.report.ratio * 100.0);
    // A wrong silhouette or knockout is not an edge effect: it moves or erases a whole
    // shadow region (the red child alone is 70x20 = 1400 px, about 10% of the image, far
    // over both budgets). The probes pin that directly at interior pixels away from any edge.
    for (x, y) in PROBES {
        let (a, b) = (c.cpu.pixel(x, y).unwrap(), c.gpu.pixel(x, y).unwrap());
        let d = [a.red().abs_diff(b.red()), a.green().abs_diff(b.green()), a.blue().abs_diff(b.blue()), a.alpha().abs_diff(b.alpha())];
        assert!(d.iter().all(|v| *v <= 8), "{name}: probe ({x}, {y}): cpu {a:?} gpu {b:?}");
    }
}

/// Interior pixels: inside the node, inside the shadow band right of and below the node,
/// inside the red child, and in the child's shadow outside the node.
const PROBES: [(u32, u32); 5] = [(50, 50), (84, 60), (50, 84), (60, 40), (96, 46)];

#[test]
fn own_fill_silhouette_matches() {
    parity("sil-own-fill", |_| {});
}

#[test]
fn fill_less_frame_uses_children_on_both() {
    parity("sil-children", |n| n.fills.clear());
}

#[test]
fn fill_less_clipping_frame_uses_children_on_both() {
    parity("sil-children-clipped", |n| {
        n.fills.clear();
        n.kind.frame_props_mut().unwrap().clips_content = true;
    });
}

#[test]
fn hidden_fill_uses_children_on_both() {
    parity("sil-hidden-fill", |n| n.fills[0].visible = false);
}

#[test]
fn stroke_only_silhouette_matches() {
    parity("sil-stroke-only", |n| {
        n.fills.clear();
        n.stroke.weight = 4.0;
        n.stroke.paints = vec![Paint::solid(Color::BLACK)];
    });
}
