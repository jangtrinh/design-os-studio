//! CPU and GPU draw the same drop-shadow silhouette for every silhouette source chosen by
//! `designos_doc::silhouette_source`: own fill, children of a fill-less frame (clipped or not),
//! a hidden fill, and a stroke-only node. Pixel tests need a GPU adapter and skip without one.

mod common;

use designos_doc::*;
use designos_render::Viewport;

const SIZE: (u32, u32) = (120, 120);
const RED: Color = Color { r: 1.0, g: 0.0, b: 0.0, a: 1.0 };

fn shadow() -> Effect {
    Effect::DropShadow { color: Color::BLACK, offset: Vec2::new(8.0, 8.0), radius: 6.0, spread: 0.0, visible: true, show_behind: false }
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
    assert_eq!(c.report.mismatched, 0, "{name}: gpu and cpu agree pixel for pixel");
}

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
