use crate::common;
use common::*;
use designos_doc::*;

fn color(r: u8, g: u8, b: u8, a: u8) -> Color {
    Color::rgba(r, g, b, a)
}

/// Opaque backdrop (100, 200, 50) with an opaque source drawn with `mode`.
fn blend_px(mode: BlendMode, backdrop: Color, src: Color, src_opacity: f32) -> [u8; 4] {
    let mut t = T::new();
    t.rect(None, Rect::new(0.0, 0.0, 10.0, 10.0), backdrop);
    let s = t.rect(None, Rect::new(0.0, 0.0, 10.0, 10.0), src);
    t.node(s).blend = mode;
    t.node(s).opacity = src_opacity;
    px(&t.render(10, 10), 5, 5)
}

#[test]
fn linear_burn_is_exact() {
    // cb + cs - 1, per channel, clamped at 0
    let got = blend_px(BlendMode::LinearBurn, color(200, 100, 255, 255), color(255, 128, 64, 255), 1.0);
    // r: 200/255 + 1 - 1 -> 200; g: 100 + 128 - 255 < 0 -> 0; b: 255 + 64 - 255 -> 64
    assert!(near(got, [200, 0, 64, 255], 1), "{got:?}");
    // ColorBurn would differ: sanity that the old approximation is gone
    let got = blend_px(BlendMode::LinearBurn, color(128, 128, 128, 255), color(192, 192, 192, 255), 1.0);
    assert!(near(got, [65, 65, 65, 255], 1), "{got:?}");
}

#[test]
fn linear_burn_with_opacity_mixes_with_backdrop() {
    // 50% opacity: result = 0.5 * burned + 0.5 * backdrop
    let got = blend_px(BlendMode::LinearBurn, color(200, 200, 200, 255), color(128, 128, 128, 255), 0.5);
    let burned = 200.0 + 128.0 - 255.0; // 73
    let want = (burned + 200.0) / 2.0;
    assert!(near(got, [want as u8, want as u8, want as u8, 255], 2), "{got:?} want {want}");
}

#[test]
fn linear_burn_over_transparent_backdrop_is_source() {
    // inside a group with nothing underneath, the source simply shows through
    let mut t = T::new();
    let g = t.add(None, |id| Node::new(id, NodeKind::Group));
    let r = t.rect(Some(g), Rect::new(0.0, 0.0, 10.0, 10.0), color(10, 200, 30, 255));
    t.node(r).blend = BlendMode::LinearBurn;
    // page is white: burn of white backdrop leaves the source colour (cb=1 -> cs)
    let got = px(&t.render(10, 10), 5, 5);
    assert!(near(got, [10, 200, 30, 255], 1), "{got:?}");
}

#[test]
fn linear_burn_on_translucent_source_uses_proper_alpha_compositing() {
    // as = 0.5, Cs = 0.5 over opaque Cb = 0.8: B = 0.3
    // co = Cb(1-as) + as*B = 0.4 + 0.15 = 0.55
    let got = blend_px(BlendMode::LinearBurn, color(204, 204, 204, 255), color(128, 128, 128, 128), 1.0);
    assert!(near(got, [140, 140, 140, 255], 3), "{got:?}");
}

#[test]
fn linear_burn_on_fill_paint() {
    let mut t = T::new();
    t.rect(None, Rect::new(0.0, 0.0, 10.0, 10.0), color(200, 100, 255, 255));
    let s = t.rect(None, Rect::new(0.0, 0.0, 10.0, 10.0), color(255, 128, 64, 255));
    t.node(s).fills[0].blend = BlendMode::LinearBurn;
    let got = px(&t.render(10, 10), 5, 5);
    assert!(near(got, [200, 0, 64, 255], 1), "{got:?}");
}

#[test]
fn linear_dodge_adds_and_clamps() {
    let got = blend_px(BlendMode::LinearDodge, color(100, 200, 50, 255), color(100, 100, 100, 255), 1.0);
    assert!(near(got, [200, 255, 150, 255], 1), "{got:?}");
}

#[test]
fn linear_dodge_translucent_source_is_exact() {
    // as = 0.5, Cs = 1 over Cb = 0.8: B = min(1, 1.8) = 1
    // co = Cb(1-as) + as*B = 0.4 + 0.5 = 0.9  (a premultiplied "plus" would clip to 1.0)
    let got = blend_px(BlendMode::LinearDodge, color(204, 204, 204, 255), color(255, 255, 255, 128), 1.0);
    assert!(near(got, [230, 230, 230, 255], 3), "{got:?}");
}
