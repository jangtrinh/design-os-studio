//! #161: drop and inner shadows composite with their own blend mode (after the #520 knockout).

mod common;

use common::*;
use designos_doc::*;

const BACKDROP: Color = Color { r: 200.0 / 255.0, g: 100.0 / 255.0, b: 50.0 / 255.0, a: 1.0 };
const GRAY: Color = Color { r: 128.0 / 255.0, g: 128.0 / 255.0, b: 128.0 / 255.0, a: 1.0 };

fn scene(effect: Effect) -> T {
    let mut t = T::new();
    t.node(t.page).kind = NodeKind::Page { background: Color::TRANSPARENT };
    t.rect(None, Rect::new(0.0, 0.0, 300.0, 300.0), BACKDROP);
    // the shadow-casting layer sits at (100,100); its shadow is shifted 60px down, clear of the layer
    t.add(None, |id| {
        let mut n = Node::rectangle(id, Rect::new(100.0, 100.0, 40.0, 40.0));
        n.fills = vec![Paint::solid(Color::WHITE)];
        n.effects = vec![effect];
        n
    });
    t
}

fn drop(blend: BlendMode) -> Effect {
    Effect::DropShadow { color: GRAY, offset: Vec2::new(0.0, 60.0), radius: 0.0, spread: 0.0, visible: true, show_behind: false, blend }
}

#[test]
fn multiply_drop_shadow_darkens_the_backdrop_at_a_literal_pixel() {
    let pm = scene(drop(BlendMode::Multiply)).render(300, 300);
    // the shadow covers (100,160)-(140,200): 200,100,50 x 128/255 = 100.4, 50.2, 25.1
    let p = px(&pm, 120, 180);
    assert!(near(p, [100, 50, 25, 255], 2), "multiply shadow over the backdrop: {p:?}");
    // outside the shadow the backdrop is untouched
    assert!(near(px(&pm, 20, 20), [200, 100, 50, 255], 1));
}

#[test]
fn normal_drop_shadow_still_covers_the_backdrop() {
    let pm = scene(drop(BlendMode::Normal)).render(300, 300);
    let p = px(&pm, 120, 180);
    assert!(near(p, [128, 128, 128, 255], 2), "normal shadow: {p:?}");
}

#[test]
fn screen_and_linear_dodge_shadows_lighten_the_backdrop() {
    // screen: 1 - (1 - cb)(1 - cs): 200 -> 228.0 (200 + 128 - 200*128/255 = 227.6), 100 -> 177.8, 50 -> 153.5
    let pm = scene(drop(BlendMode::Screen)).render(300, 300);
    assert!(near(px(&pm, 120, 180), [228, 178, 154, 255], 2), "screen: {:?}", px(&pm, 120, 180));
    // linear dodge: min(1, cb + cs): 200 + 128 -> 255, 100 + 128 = 228, 50 + 128 = 178
    let pm = scene(drop(BlendMode::LinearDodge)).render(300, 300);
    assert!(near(px(&pm, 120, 180), [255, 228, 178, 255], 2), "linear dodge: {:?}", px(&pm, 120, 180));
}

#[test]
fn blend_applies_after_the_show_behind_knockout() {
    // the shadow lies under its own layer: show_behind off removes it there, so the layer
    // stays white whatever the blend mode
    let e = Effect::DropShadow {
        color: GRAY,
        offset: Vec2::new(10.0, 10.0),
        radius: 0.0,
        spread: 0.0,
        visible: true,
        show_behind: false,
        blend: BlendMode::Multiply,
    };
    let pm = scene(e).render(300, 300);
    assert!(near(px(&pm, 120, 120), [255, 255, 255, 255], 1), "layer body: {:?}", px(&pm, 120, 120));
    // the uncovered corner of the shadow is multiplied
    assert!(near(px(&pm, 145, 145), [100, 50, 25, 255], 2), "shadow corner: {:?}", px(&pm, 145, 145));
}

#[test]
fn multiply_inner_shadow_darkens_what_is_beneath() {
    let e = Effect::InnerShadow { color: GRAY, offset: Vec2::new(20.0, 0.0), radius: 0.0, spread: 0.0, visible: true, blend: BlendMode::Multiply };
    // white layer fill: the inner shadow is a 20px band on the left of the layer
    let pm = scene(e).render(300, 300);
    let band = px(&pm, 110, 120);
    let rest = px(&pm, 130, 120);
    assert!(near(rest, [255, 255, 255, 255], 1), "outside the band: {rest:?}");
    // white x 128/255 = 128 (multiply by white is the shadow colour itself)
    assert!(near(band, [128, 128, 128, 255], 2), "inside the band: {band:?}");
}
