//! A Display P3 document converts its colours to sRGB the same way on the CPU and the GPU renderer, and
//! an sRGB document is untouched. Needs a hardware GPU adapter and skips without one.

mod common;

use designos_doc::*;
use designos_render::Viewport;

const SIZE: (u32, u32) = (320, 240);
const MID: Color = Color { r: 0.5, g: 0.4, b: 0.8, a: 1.0 };
const WIDE: Color = Color { r: 0.1, g: 0.9, b: 0.2, a: 1.0 };
const MID_AS_SRGB: [u8; 3] = [132, 101, 211];

type MakeGradient = fn([Vec2; 3], Vec<GradientStop>) -> PaintKind;

fn gradient(kind: MakeGradient, c: &impl Fn(Color) -> Color) -> Paint {
    let stops = vec![GradientStop { pos: 0.0, color: c(MID) }, GradientStop { pos: 1.0, color: c(WIDE) }];
    Paint { kind: kind([Vec2::new(0.0, 0.5), Vec2::new(1.0, 0.5), Vec2::new(0.5, 1.0)], stops), opacity: 1.0, visible: true, blend: BlendMode::Normal }
}

/// Page background, a solid, four gradients, a stroke, text, a drop shadow, an inner shadow and a noise grain, all in P3 numbers.
fn scene(profile: ColorProfile) -> (Document, NodeId) {
    scene_with(profile, |c| c)
}

/// [`scene`] with every colour written through `c`.
fn scene_with(profile: ColorProfile, c: impl Fn(Color) -> Color) -> (Document, NodeId) {
    let mut t = common::T::new();
    t.doc.color_profile = profile;
    t.node(t.page).kind = NodeKind::Page { background: c(Color { r: 0.9, g: 0.85, b: 0.95, a: 1.0 }) };
    t.rect(None, Rect::new(10.0, 10.0, 60.0, 60.0), c(MID));
    let kinds: [MakeGradient; 4] = [
        |handles, stops| PaintKind::LinearGradient { handles, stops },
        |handles, stops| PaintKind::RadialGradient { handles, stops },
        |handles, stops| PaintKind::AngularGradient { handles, stops },
        |handles, stops| PaintKind::DiamondGradient { handles, stops },
    ];
    for (i, k) in kinds.into_iter().enumerate() {
        t.add(None, |id| {
            let mut n = Node::rectangle(id, Rect::new(90.0 + i as f32 * 55.0, 10.0, 50.0, 60.0));
            n.fills = vec![gradient(k, &c)];
            n
        });
    }
    t.add(None, |id| {
        let mut n = Node::rectangle(id, Rect::new(20.0, 100.0, 60.0, 50.0));
        n.fills = vec![Paint::solid(Color::WHITE)];
        n.effects = vec![Effect::DropShadow {
            color: c(WIDE),
            offset: Vec2::new(60.0, 20.0),
            radius: 6.0,
            spread: 0.0,
            visible: true,
            show_behind: false,
            blend: BlendMode::Normal,
        }];
        n
    });
    t.add(None, |id| {
        let mut n = Node::rectangle(id, Rect::new(190.0, 100.0, 90.0, 80.0));
        n.fills = vec![Paint::solid(Color::rgb(240, 240, 240))];
        n.effects = vec![Effect::InnerShadow { color: c(MID), offset: Vec2::new(6.0, 6.0), radius: 8.0, spread: 0.0, visible: true, blend: BlendMode::Normal }];
        n
    });
    t.add(None, |id| {
        let mut n = Node::rectangle(id, Rect::new(20.0, 180.0, 120.0, 50.0));
        n.fills = vec![Paint::solid(Color::WHITE)];
        n.effects = vec![Effect::Noise(NoiseEffect {
            mode: NoiseMode::Mono { color: c(MID) },
            size: 2.0,
            size_y: None,
            density: 0.5,
            opacity: 1.0,
            blend: BlendMode::Normal,
            seed: 7,
            visible: true,
        })];
        n
    });
    t.add(None, |id| {
        let mut n = Node::rectangle(id, Rect::new(160.0, 196.0, 60.0, 30.0));
        n.fills = vec![Paint::solid(c(MID))];
        n.stroke.paints = vec![Paint::solid(c(WIDE))];
        n.stroke.weight = 5.0;
        n.stroke.align = StrokeAlign::Center;
        n
    });
    t.add(None, |id| {
        let style = TextStyle { font_size: 28.0, fills: vec![Paint::solid(c(Color { r: 0.9, g: 0.1, b: 0.4, a: 1.0 }))], ..Default::default() };
        Node::text(id, Rect::new(236.0, 194.0, 80.0, 34.0), "Wide", style)
    });
    (t.doc, t.page)
}

fn vp() -> Viewport {
    Viewport { offset: Vec2::new(0.0, 0.0), zoom: 1.0, dpr: 1.0 }
}

#[test]
fn cpu_and_gpu_match_for_a_display_p3_document() {
    let Some(mut g) = common::gpu() else { return };
    let (doc, page) = scene(ColorProfile::DisplayP3);
    let c = common::compare(&mut g, "display-p3", &doc, page, vp(), SIZE, 8);
    assert!(c.report.ratio < 0.02, "strict mismatch {:.3}%", c.report.ratio * 100.0);
}

#[test]
fn cpu_and_gpu_match_for_the_same_numbers_in_an_srgb_document() {
    let Some(mut g) = common::gpu() else { return };
    let (doc, page) = scene(ColorProfile::Srgb);
    let c = common::compare(&mut g, "display-p3-guard-srgb", &doc, page, vp(), SIZE, 8);
    assert!(c.report.ratio < 0.02, "strict mismatch {:.3}%", c.report.ratio * 100.0);
}

#[test]
fn the_gpu_draws_a_p3_solid_as_the_converted_srgb_colour_and_an_srgb_solid_as_stored() {
    let Some(mut g) = common::gpu() else { return };
    let (p3, page) = scene(ColorProfile::DisplayP3);
    let (srgb, _) = scene(ColorProfile::Srgb);
    let a = common::compare(&mut g, "display-p3-solid", &p3, page, vp(), SIZE, 8);
    let b = common::compare(&mut g, "display-p3-solid-srgb", &srgb, page, vp(), SIZE, 8);
    let probe = |c: &common::Cmp| {
        let p = c.gpu.pixel(40, 40).unwrap();
        [p.red(), p.green(), p.blue()]
    };
    let near = |x: [u8; 3], y: [u8; 3]| x.iter().zip(y).all(|(a, b)| (*a as i32 - b as i32).abs() <= 1);
    assert!(near(probe(&a), MID_AS_SRGB), "P3 solid on the GPU: {:?}", probe(&a));
    assert!(near(probe(&b), [128, 102, 204]), "sRGB solid on the GPU: {:?}", probe(&b));
}

#[test]
fn the_gpu_draws_a_p3_document_like_the_srgb_document_of_the_converted_colours() {
    // Strokes, text, gradients, shadows, grain and the page background all convert on the GPU.
    // One GPU per document: a headless compile caches geometry (text fills included) by node id at revision 0.
    let (Some(mut g1), Some(mut g2), Some(mut g3)) = (common::gpu(), common::gpu(), common::gpu()) else { return };
    let (p3, page) = scene(ColorProfile::DisplayP3);
    let (pre, _) = scene_with(ColorProfile::Srgb, |c| ColorProfile::DisplayP3.to_srgb(&c));
    let (raw, _) = scene(ColorProfile::Srgb);
    let a = g1.render_page(&p3, page, &vp(), SIZE);
    let b = g2.render_page(&pre, page, &vp(), SIZE);
    let c = g3.render_page(&raw, page, &vp(), SIZE);
    let differing = |x: &designos_render::tiny_skia::Pixmap, y: &designos_render::tiny_skia::Pixmap| {
        x.data().chunks_exact(4).zip(y.data().chunks_exact(4)).filter(|(p, q)| p.iter().zip(q.iter()).any(|(a, b)| (*a as i32 - *b as i32).abs() > 1)).count()
    };
    assert_eq!(differing(&a, &b), 0, "P3 numbers and the converted sRGB numbers must give the same GPU pixels");
    assert!(differing(&a, &c) > 1000, "the conversion must change the GPU pixels");
}
