//! Every paint a Display P3 document draws is converted: fills, strokes, text fills and strokes,
//! gradients and the page background.

mod common;

use common::*;
use designos_doc::*;

const MID: Color = Color { r: 0.5, g: 0.4, b: 0.8, a: 1.0 };
const GREEN: Color = Color { r: 0.1, g: 0.9, b: 0.2, a: 1.0 };

/// Fill, stroke, text (fill and stroke), angular gradient and page background, every colour written through `c`.
fn full_scene(profile: ColorProfile, c: impl Fn(Color) -> Color) -> T {
    let mut t = T::new();
    t.doc.color_profile = profile;
    t.node(t.page).kind = NodeKind::Page { background: c(Color { r: 0.9, g: 0.85, b: 0.95, a: 1.0 }) };
    t.add(None, |id| {
        let mut n = Node::rectangle(id, Rect::new(10.0, 10.0, 60.0, 40.0));
        n.fills = vec![Paint::solid(c(MID))];
        n.stroke.paints = vec![Paint::solid(c(GREEN))];
        n.stroke.weight = 6.0;
        n.stroke.align = StrokeAlign::Center;
        n
    });
    t.add(None, |id| {
        let style = TextStyle { font_size: 28.0, fills: vec![Paint::solid(c(Color { r: 0.9, g: 0.1, b: 0.4, a: 1.0 }))], ..Default::default() };
        let mut n = Node::text(id, Rect::new(90.0, 10.0, 120.0, 40.0), "Wide", style);
        n.stroke.paints = vec![Paint::solid(c(Color { r: 0.0, g: 0.6, b: 1.0, a: 1.0 }))];
        n.stroke.weight = 1.5;
        n
    });
    t.add(None, |id| {
        let mut n = Node::rectangle(id, Rect::new(10.0, 70.0, 80.0, 40.0));
        n.fills = vec![paint(PaintKind::AngularGradient {
            handles: [Vec2::new(0.5, 0.5), Vec2::new(1.0, 0.5), Vec2::new(0.5, 1.0)],
            stops: vec![stop(0.0, c(MID)), stop(1.0, c(Color { r: 1.0, g: 0.4, b: 0.0, a: 1.0 }))],
        })];
        n
    });
    t
}

#[test]
fn a_display_p3_document_draws_exactly_like_the_srgb_document_of_the_converted_colours() {
    let p3 = full_scene(ColorProfile::DisplayP3, |c| c).render(220, 120);
    let srgb = full_scene(ColorProfile::Srgb, |c| ColorProfile::DisplayP3.to_srgb(&c)).render(220, 120);
    assert_eq!(p3.data(), srgb.data());
    let unconverted = full_scene(ColorProfile::Srgb, |c| c).render(220, 120);
    assert_ne!(p3.data(), unconverted.data(), "the conversion must change the pixels");
}

#[test]
fn a_display_p3_stroke_pixel_is_the_converted_colour() {
    let pm = full_scene(ColorProfile::DisplayP3, |c| c).render(220, 120);
    // the left edge of the rectangle's centred 6px stroke, over the page: P3 (0.1, 0.9, 0.2)
    let p = px(&pm, 10, 30);
    let w = ColorProfile::DisplayP3.to_srgb(&GREEN);
    let want = [(w.r * 255.0).round() as u8, (w.g * 255.0).round() as u8, (w.b * 255.0).round() as u8, 255];
    assert!(near(p, want, 2), "{p:?} vs {want:?}");
}
