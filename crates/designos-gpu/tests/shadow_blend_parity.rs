//! #161: CPU and GPU composite drop and inner shadows with the same blend mode. Needs a hardware GPU
//! adapter and skips without one.

mod common;

use designos_doc::*;
use designos_render::Viewport;

const SIZE: (u32, u32) = (320, 240);

const ALL: [BlendMode; 19] = [
    BlendMode::Normal,
    BlendMode::PassThrough,
    BlendMode::Darken,
    BlendMode::Multiply,
    BlendMode::ColorBurn,
    BlendMode::LinearBurn,
    BlendMode::Lighten,
    BlendMode::Screen,
    BlendMode::ColorDodge,
    BlendMode::LinearDodge,
    BlendMode::Overlay,
    BlendMode::SoftLight,
    BlendMode::HardLight,
    BlendMode::Difference,
    BlendMode::Exclusion,
    BlendMode::Hue,
    BlendMode::Saturation,
    BlendMode::Color,
    BlendMode::Luminosity,
];

/// Three backdrop stripes, a layer with a blurred drop shadow, and a layer with an inner shadow.
fn scene(blend: BlendMode) -> (Document, NodeId) {
    let mut t = common::T::new();
    let stripes = [Color::rgb(200, 100, 50), Color::rgb(40, 160, 220), Color::rgb(230, 230, 40)];
    for (i, c) in stripes.into_iter().enumerate() {
        t.rect(None, Rect::new(i as f32 * 107.0, 0.0, 107.0, 240.0), c);
    }
    let shadow = Color { r: 0.5, g: 0.2, b: 0.7, a: 0.8 };
    t.add(None, |id| {
        let mut n = Node::rectangle(id, Rect::new(40.0, 30.0, 80.0, 60.0));
        n.fills = vec![Paint::solid(Color::WHITE)];
        n.effects =
            vec![Effect::DropShadow { color: shadow, offset: Vec2::new(30.0, 60.0), radius: 10.0, spread: 2.0, visible: true, show_behind: false, blend }];
        n
    });
    t.add(None, |id| {
        let mut n = Node::rectangle(id, Rect::new(180.0, 30.0, 100.0, 80.0));
        n.fills = vec![Paint::solid(Color::rgb(240, 240, 240))];
        n.effects = vec![Effect::InnerShadow { color: shadow, offset: Vec2::new(8.0, 8.0), radius: 12.0, spread: 0.0, visible: true, blend }];
        n
    });
    (t.doc, t.page)
}

#[test]
fn cpu_and_gpu_match_for_every_shadow_blend_mode() {
    let Some(mut g) = common::gpu() else { return };
    let vp = Viewport { offset: Vec2::new(0.0, 0.0), zoom: 1.0, dpr: 1.0 };
    for mode in ALL {
        let (doc, page) = scene(mode);
        let c = common::compare(&mut g, &format!("shadow-blend-{mode:?}"), &doc, page, vp, SIZE, 8);
        assert!(c.report.ratio < 0.02, "{mode:?}: strict mismatch {:.3}%", c.report.ratio * 100.0);
    }
}

#[test]
fn a_multiply_drop_shadow_darkens_the_backdrop_on_the_gpu() {
    let Some(mut g) = common::gpu() else { return };
    let vp = Viewport { offset: Vec2::new(0.0, 0.0), zoom: 1.0, dpr: 1.0 };
    let (normal, page) = scene(BlendMode::Normal);
    let (multiply, _) = scene(BlendMode::Multiply);
    let a = common::compare(&mut g, "shadow-blend-probe-normal", &normal, page, vp, SIZE, 8);
    let b = common::compare(&mut g, "shadow-blend-probe-multiply", &multiply, page, vp, SIZE, 8);
    // (100, 130) lies inside the drop shadow, clear of the layer, over the first stripe
    let (n, m) = (a.gpu.pixel(100, 130).unwrap(), b.gpu.pixel(100, 130).unwrap());
    assert!(m.red() < n.red() && m.green() < n.green(), "multiply must be darker than normal: {n:?} vs {m:?}");
}
