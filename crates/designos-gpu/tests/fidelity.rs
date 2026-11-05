//! GPU renderer vs the CPU reference. Tests print a note and return when the
//! machine has no GPU adapter.

mod common;

use common::*;
use designos_doc::*;
use designos_render::tiny_skia::Pixmap;
use designos_render::Viewport;
use std::sync::Arc;

#[test]
fn simple_shapes() {
    let Some(mut g) = gpu() else { return };
    let mut t = T::new();
    t.rect(None, Rect::new(10.0, 10.0, 60.0, 40.0), Color::rgb(0x18, 0xA0, 0xFB));
    let e = t.add(None, |id| Node::ellipse(id, Rect::new(90.0, 10.0, 50.0, 50.0)));
    t.node(e).fills = vec![Paint::solid(Color::rgb(0xF2, 0x4E, 0x1E))];
    let c = compare(&mut g, "simple", &t.doc, t.page, Viewport::default(), (160, 80), 8);
    // tiny scene: nearly all of its mismatches are edge pixels
    assert!(c.report.ratio < 0.03, "{:?}", c.report);
}

#[test]
fn background_blur_without_a_visible_fill_leaves_backdrop_sharp() {
    let Some(mut g) = gpu() else { return };
    let mut t = T::new();
    t.rect(None, Rect::new(10.0, 0.0, 10.0, 40.0), Color::BLACK);
    let pane = t.add(None, |id| {
        let mut n = Node::rectangle(id, Rect::new(0.0, 0.0, 40.0, 40.0));
        n.fills.clear();
        n.effects = vec![Effect::BackgroundBlur { radius: 8.0, visible: true }];
        n
    });
    let c = compare(&mut g, "background_blur_fill_less", &t.doc, t.page, Viewport::default(), (60, 40), 2);
    let stripe_cpu = c.cpu.pixel(15, 20).unwrap();
    let stripe_gpu = c.gpu.pixel(15, 20).unwrap();
    assert_eq!([stripe_cpu.red(), stripe_cpu.green(), stripe_cpu.blue(), stripe_cpu.alpha()], [0, 0, 0, 255]);
    assert_eq!([stripe_gpu.red(), stripe_gpu.green(), stripe_gpu.blue(), stripe_gpu.alpha()], [0, 0, 0, 255]);
    let clear_cpu = c.cpu.pixel(25, 20).unwrap();
    let clear_gpu = c.gpu.pixel(25, 20).unwrap();
    assert_eq!([clear_cpu.red(), clear_cpu.green(), clear_cpu.blue(), clear_cpu.alpha()], [255, 255, 255, 255]);
    assert_eq!([clear_gpu.red(), clear_gpu.green(), clear_gpu.blue(), clear_gpu.alpha()], [255, 255, 255, 255]);

    t.node(pane).fills = vec![Paint { opacity: 0.0, ..Paint::solid(Color::WHITE) }];
    g.compiler.geom.clear();
    let c_zero = compare(&mut g, "background_blur_zero_opacity_fill", &t.doc, t.page, Viewport::default(), (60, 40), 2);
    let zero_cpu = c_zero.cpu.pixel(15, 20).unwrap();
    let zero_gpu = c_zero.gpu.pixel(15, 20).unwrap();
    assert!(zero_cpu.red() > 40 && zero_cpu.red() < 200, "CPU zero-opacity fill blurs backdrop: {zero_cpu:?}");
    assert!(zero_gpu.red() > 40 && zero_gpu.red() < 200, "GPU zero-opacity fill blurs backdrop: {zero_gpu:?}");

    t.node(pane).fills.clear();
    t.node(pane).stroke.paints = vec![Paint::solid(Color::BLACK)];
    t.node(pane).stroke.weight = 2.0;
    g.compiler.geom.clear();
    let c_stroke = compare(&mut g, "background_blur_stroke_only", &t.doc, t.page, Viewport::default(), (60, 40), 2);
    let stroke_cpu = c_stroke.cpu.pixel(15, 20).unwrap();
    let stroke_gpu = c_stroke.gpu.pixel(15, 20).unwrap();
    assert!(stroke_cpu.red() > 40 && stroke_cpu.red() < 200, "CPU stroke-only layer blurs backdrop: {stroke_cpu:?}");
    assert!(stroke_gpu.red() > 40 && stroke_gpu.red() < 200, "GPU stroke-only layer blurs backdrop: {stroke_gpu:?}");
}

/// White page, black stripe at x 10..20, and a group without any paint of its own that holds a
/// 40x40 rectangle filled white at 0.4 opacity. `blur` puts a background blur on the group.
fn paintless_group_over_stripe(blur: bool) -> T {
    let mut t = T::new();
    t.rect(None, Rect::new(10.0, 0.0, 10.0, 40.0), Color::BLACK);
    let group = t.add(None, |id| {
        let mut n = Node::new(id, NodeKind::Group);
        n.size = Vec2::new(40.0, 40.0);
        if blur {
            n.effects = vec![Effect::BackgroundBlur { radius: 8.0, visible: true }];
        }
        n
    });
    t.add(Some(group), |id| {
        let mut n = Node::rectangle(id, Rect::new(0.0, 0.0, 40.0, 40.0));
        n.fills = vec![Paint { opacity: 0.4, ..Paint::solid(Color::WHITE) }];
        n
    });
    t
}

#[test]
fn background_blur_on_a_paintless_group_blurs_through_its_children() {
    let Some(mut g) = gpu() else { return };
    let sharp_scene = paintless_group_over_stripe(false);
    let blur_scene = paintless_group_over_stripe(true);
    g.compiler.geom.clear();
    let sharp = compare(&mut g, "background_blur_group_without_effect", &sharp_scene.doc, sharp_scene.page, Viewport::default(), (60, 40), 2);
    g.compiler.geom.clear();
    let blurred = compare(&mut g, "background_blur_paintless_group", &blur_scene.doc, blur_scene.page, Viewport::default(), (60, 40), 2);
    let stripe = |sharp: &Pixmap, blurred: &Pixmap| (sharp.pixel(15, 20).unwrap().red(), blurred.pixel(15, 20).unwrap().red());
    let (cpu_before, cpu_after) = stripe(&sharp.cpu, &blurred.cpu);
    let (gpu_before, gpu_after) = stripe(&sharp.gpu, &blurred.gpu);
    assert!(
        cpu_after >= cpu_before + 8 && gpu_after >= gpu_before + 8,
        "the stripe under the child must be blurred: CPU {cpu_before} -> {cpu_after}, GPU {gpu_before} -> {gpu_after}"
    );
    for (who, pm) in [("CPU", &blurred.cpu), ("GPU", &blurred.gpu)] {
        let under_child = pm.pixel(15, 20).unwrap();
        assert_eq!(under_child.alpha(), 255, "{who}: the translucent child over an opaque page must remain opaque: {under_child:?}");
        let outside = pm.pixel(55, 20).unwrap();
        assert_eq!([outside.red(), outside.green(), outside.blue(), outside.alpha()], [255, 255, 255, 255], "{who}: outside the group the page is untouched");
    }
}

/// White page, black stripe at x 10..20, and a text node with no fill on its base style. The
/// only fill sits on the run covering the whole string. `blur` puts a background blur on it.
fn text_with_run_fill_over_stripe(run_fill: Paint, blur: bool) -> T {
    let mut t = T::new();
    t.rect(None, Rect::new(10.0, 0.0, 10.0, 40.0), Color::BLACK);
    t.add(None, |id| {
        let base = TextStyle { font_size: 40.0, font_weight: 800, fills: Vec::new(), ..Default::default() };
        let mut n = Node::text(id, Rect::new(0.0, 0.0, 60.0, 40.0), "MM", base.clone());
        let NodeKind::Text(tp) = &mut n.kind else { unreachable!() };
        tp.runs = vec![TextRun { start: 0, end: 2, style: TextStyle { fills: vec![run_fill], ..base } }];
        if blur {
            n.effects = vec![Effect::BackgroundBlur { radius: 8.0, visible: true }];
        }
        n
    });
    t
}

/// The brightest red channel on the stripe columns, which are black unless something paints over them.
fn brightest_on_stripe(pm: &Pixmap) -> u8 {
    (10..20).flat_map(|x| (0..40).map(move |y| (x, y))).map(|(x, y)| pm.pixel(x, y).unwrap().red()).max().unwrap()
}

#[test]
fn background_blur_on_text_counts_a_visible_run_fill() {
    let Some(mut g) = gpu() else { return };
    let mut run = |name: &str, fill: &Paint, blur: bool| {
        let t = text_with_run_fill_over_stripe(fill.clone(), blur);
        g.compiler.geom.clear();
        let c = compare(&mut g, name, &t.doc, t.page, Viewport::default(), (60, 40), 2);
        (brightest_on_stripe(&c.cpu), brightest_on_stripe(&c.gpu))
    };
    let invisible_ink = Paint { opacity: 0.0, ..Paint::solid(Color::WHITE) };
    assert_eq!(run("background_blur_text_without_effect", &invisible_ink, false), (0, 0), "fixture: nothing paints on the stripe");
    let (cpu, gpu) = run("background_blur_text_run_fill", &invisible_ink, true);
    assert!(cpu > 40 && gpu > 40, "the stripe under the glyphs must be blurred: brightest CPU {cpu}, GPU {gpu}");

    let hidden = Paint { visible: false, ..Paint::solid(Color::WHITE) };
    assert_eq!(run("background_blur_text_hidden_run_fill", &hidden, true), (0, 0), "a hidden run fill leaves the text with no paint: the stripe stays sharp");
}

#[test]
fn render_demo() {
    let Some(mut g) = gpu() else { return };
    let (doc, page) = demo::demo_scene();
    let vp = Viewport { offset: Vec2::ZERO, zoom: 1.0, dpr: 2.0 };
    let c = compare(&mut g, "render_demo", &doc, page, vp, (1560, 2000), 8);
    // target: under 1% at tolerance 8 (strict: every edge pixel counts). The bounds are
    // a little looser than what the reference machine measures, since MSAA sample
    // patterns differ between GPUs.
    assert!(c.report.ratio < 0.02, "strict mismatch {:.3}%", c.report.ratio * 100.0);
    assert!(c.report_aa.ratio < 0.005, "mismatch with AA forgiven {:.3}%", c.report_aa.ratio * 100.0);
}

#[test]
fn render_demo_zoomed() {
    let Some(mut g) = gpu() else { return };
    let (doc, page) = demo::demo_scene();
    // zoomed in and panned: nothing is re-rasterised on the GPU, geometry is re-flattened by bucket
    let vp = Viewport { offset: Vec2::new(300.0, 340.0), zoom: 3.0, dpr: 1.0 };
    let c = compare(&mut g, "render_demo_zoom3", &doc, page, vp, (1200, 900), 8);
    assert!(c.report.ratio < 0.02, "mismatch {:.3}%", c.report.ratio * 100.0);
}

fn fx_scene() -> (Document, NodeId) {
    let mut t = T::new();
    t.doc.images.insert("checker".into(), demo::checker());
    // stripes behind glass
    for i in 0..10 {
        t.rect(None, Rect::new(10.0 + i as f32 * 14.0, 10.0, 7.0, 90.0), Color::rgb(0x22, 0x22, 0x22));
    }
    let glass = t.rect(None, Rect::new(30.0, 30.0, 90.0, 50.0), Color::rgba(255, 255, 255, 40));
    t.node(glass).corner_radii = [14.0; 4];
    t.node(glass).effects = vec![Effect::Glass(GlassEffect {
        light_angle: 45.0,
        light_intensity: 0.7,
        refraction: 0.5,
        depth: 10.0,
        dispersion: 0.3,
        frost: 4.0,
        splay: 0.0,
        bevel: 0.0,
        visible: true,
    })];
    // noise (shape fast path) and noise on a group (general path)
    let n = t.rect(None, Rect::new(180.0, 10.0, 80.0, 60.0), Color::rgb(0x44, 0x88, 0xDD));
    t.node(n).effects = vec![Effect::Noise(NoiseEffect {
        mode: NoiseMode::Duo { color: Color::rgb(0, 0, 0), secondary: Color::WHITE },
        size: 1.5,
        size_y: None,
        density: 0.5,
        opacity: 0.4,
        blend: BlendMode::Normal,
        seed: 7,
        visible: true,
    })];
    let tx = t.rect(None, Rect::new(180.0, 90.0, 80.0, 50.0), Color::rgb(0xE0, 0x50, 0x30));
    t.node(tx).effects = vec![Effect::Texture(TextureEffect { size: 6.0, size_y: None, radius: 4.0, clip_to_shape: false, seed: 3, visible: true })];
    // clipped rounded frame with rotated child, image + blend modes
    let fr = t.add(None, |id| Node::frame(id, Rect::new(10.0, 120.0, 100.0, 70.0)));
    if let NodeKind::Frame(p) = &mut t.node(fr).kind {
        p.clips_content = true;
    }
    t.node(fr).corner_radii = [18.0; 4];
    let ch = t.rect(Some(fr), Rect::new(30.0, 20.0, 90.0, 60.0), Color::rgb(0x22, 0xAA, 0x66));
    t.node(ch).transform = Affine::translate(30.0, 20.0) * Affine::rotate(0.5);
    let im = t.rect(None, Rect::new(130.0, 120.0, 60.0, 60.0), Color::WHITE);
    t.node(im).fills = vec![Paint {
        kind: PaintKind::Image { image_ref: "checker".into(), scale_mode: ImageScaleMode::Fill, params: Default::default() },
        opacity: 0.9,
        visible: true,
        blend: BlendMode::Multiply,
    }];
    let modes = [BlendMode::Screen, BlendMode::Overlay, BlendMode::Difference, BlendMode::Hue, BlendMode::LinearDodge, BlendMode::SoftLight];
    for (i, m) in modes.iter().enumerate() {
        let r = t.rect(None, Rect::new(200.0 + i as f32 * 10.0, 160.0, 40.0, 30.0), Color::rgb(0xC0, 0x40, 0x90));
        t.node(r).blend = *m;
    }
    (t.doc, t.page)
}

#[test]
fn effects_and_blends() {
    let Some(mut g) = gpu() else { return };
    let (doc, page) = fx_scene();
    let c = compare(&mut g, "fx", &doc, page, Viewport { offset: Vec2::ZERO, zoom: 1.0, dpr: 2.0 }, (600, 420), 8);
    assert!(c.report.ratio < 0.03, "mismatch {:.3}%", c.report.ratio * 100.0);
}

#[test]
fn cpu_fallback_matches_cpu() {
    let Some(mut g) = gpu() else { return };
    let (doc, page) = fx_scene();
    // every node with an effect goes through the CPU-into-texture fallback
    g.compiler.opts.force_fallback = Some(Arc::new(|n: &Node| !n.effects.is_empty()));
    let c = compare(&mut g, "fx_fallback", &doc, page, Viewport { offset: Vec2::ZERO, zoom: 1.0, dpr: 2.0 }, (600, 420), 8);
    let stats = g.compile(&doc, page, 1, &Viewport { offset: Vec2::ZERO, zoom: 1.0, dpr: 2.0 }, (600, 420)).stats();
    assert!(stats.fallback_layers >= 2, "{stats:?}");
    assert!(c.report.ratio < 0.02, "mismatch {:.3}%", c.report.ratio * 100.0);
}

#[test]
fn extreme_effects_render() {
    let Some(mut g) = gpu() else { return };
    let mut t = T::new();
    t.rect(None, Rect::new(0.0, 0.0, 200.0, 200.0), Color::rgb(0x20, 0x60, 0xC0));
    let a = t.rect(None, Rect::new(40.0, 40.0, 80.0, 80.0), Color::rgb(0xF0, 0x40, 0x20));
    t.node(a).effects = vec![
        Effect::LayerBlur { radius: 400.0, visible: true },
        Effect::DropShadow {
            color: Color::BLACK,
            offset: Vec2::new(30.0, 30.0),
            radius: 300.0,
            spread: 20.0,
            visible: true,
            show_behind: true,
            blend: Default::default(),
        },
    ];
    let b = t.rect(None, Rect::new(100.0, 100.0, 90.0, 90.0), Color::rgba(255, 255, 255, 80));
    t.node(b).effects = vec![Effect::BackgroundBlur { radius: 250.0, visible: true }];
    let c = compare(&mut g, "extreme", &t.doc, t.page, Viewport { offset: Vec2::ZERO, zoom: 2.0, dpr: 1.0 }, (400, 400), 8);
    // huge blurs are approximated at reduced resolution; the image must still be close
    assert!(c.report.ssim > 0.9, "{:?}", c.report);
}

fn board_scene() -> (Document, NodeId) {
    let mut t = T::new();
    t.doc.kind = DocumentKind::Board;
    t.doc.get_mut(t.page).unwrap().kind = NodeKind::Page { background: Color::rgb(0xF4, 0xF4, 0xF4) };
    let mut ids = Vec::new();
    let notes =
        [(0xFFE58F_u32, "Ship the board mode"), (0xB6D4FF, "A longer note with enough words to make the text shrink to fit the square"), (0xC4EBA6, "")];
    for (i, (c, text)) in notes.iter().enumerate() {
        let id = t.add(None, |id| Node::sticky(id, Rect::new(20.0 + i as f32 * 170.0, 20.0, 150.0, 150.0), *text));
        if let NodeKind::Sticky(s) = &mut t.node(id).kind {
            s.color = Color::rgb((c >> 16) as u8, (c >> 8) as u8, *c as u8);
        }
        ids.push(id);
    }
    let shape = t.add(None, |id| Node::shape_with_text(id, Rect::new(30.0, 230.0, 140.0, 90.0), ShapeKind::Diamond, "Decide"));
    let shape2 = t.add(None, |id| Node::shape_with_text(id, Rect::new(300.0, 230.0, 140.0, 90.0), ShapeKind::RoundedRect, "Do it"));
    let connect = |t: &mut T, a: NodeId, b: NodeId, line: ConnectorLine, label: &str| {
        let c = t.add(None, |id| {
            let mut p =
                ConnectorProps { start: ConnectorEnd::attached(a, Magnet::Auto), end: ConnectorEnd::attached(b, Magnet::Auto), line, ..Default::default() };
            p.end_cap = ConnectorCap::Triangle;
            p.start_cap = ConnectorCap::Circle;
            if !label.is_empty() {
                let mut l = board_text(13.0, TextAlignH::Center, TextAlignV::Center);
                l.content = label.into();
                p.label = Some(l);
            }
            Node::connector(id, p)
        });
        sync_connector(&mut t.doc, c);
    };
    connect(&mut t, ids[0], shape, ConnectorLine::Elbowed, "");
    connect(&mut t, shape, shape2, ConnectorLine::Straight, "yes");
    connect(&mut t, ids[2], shape2, ConnectorLine::Curved, "");
    for (i, k) in STAMP_KINDS.iter().enumerate() {
        t.add(None, |id| Node::stamp(id, Vec2::new(30.0 + i as f32 * 60.0, 350.0), k, 48.0));
    }
    (t.doc, t.page)
}

#[test]
fn board_kinds_match_cpu() {
    let Some(mut g) = gpu() else { return };
    let (doc, page) = board_scene();
    let c = compare(&mut g, "board", &doc, page, Viewport { offset: Vec2::ZERO, zoom: 1.0, dpr: 2.0 }, (1000, 800), 8);
    assert!(c.report.ratio < 0.03, "mismatch {:.3}%", c.report.ratio * 100.0);
    assert!(c.report_aa.ratio < 0.01, "mismatch with AA forgiven {:.3}%", c.report_aa.ratio * 100.0);
    // the notes really are drawn (not an empty page): the yellow sticky fill is on screen
    let p = c.gpu.pixel(2 * 90, 2 * 150).unwrap();
    assert!(p.red() > 240 && p.green() > 200 && p.blue() < 180, "{p:?}");
}

#[test]
fn board_kinds_zoomed_match_cpu() {
    let Some(mut g) = gpu() else { return };
    let (doc, page) = board_scene();
    let vp = Viewport { offset: Vec2::new(20.0, 10.0), zoom: 2.5, dpr: 1.0 };
    let c = compare(&mut g, "board_zoom", &doc, page, vp, (900, 700), 8);
    assert!(c.report.ratio < 0.03, "mismatch {:.3}%", c.report.ratio * 100.0);
}

#[test]
fn board_kinds_through_the_cpu_fallback_match_cpu() {
    let Some(mut g) = gpu() else { return };
    let (doc, page) = board_scene();
    // everything the stickies lower to (shadowed bodies, their text) takes the CPU-into-texture path
    g.compiler.opts.force_fallback = Some(Arc::new(|n: &Node| !n.effects.is_empty() || matches!(n.kind, NodeKind::Text(_))));
    let vp = Viewport { offset: Vec2::ZERO, zoom: 1.0, dpr: 2.0 };
    let c = compare(&mut g, "board_fallback", &doc, page, vp, (1000, 800), 8);
    assert!(c.report.ratio < 0.02, "mismatch {:.3}%", c.report.ratio * 100.0);
}

#[test]
fn translucent_color_variable_with_stored_paint_opacity_matches_cpu() {
    let Some(mut g) = gpu() else { return };
    let mut t = T::new();
    let page = t.page;
    t.node(page).kind = NodeKind::Page { background: Color::BLACK };
    t.add(None, |id| {
        let mut n = Node::frame(id, Rect::new(0.0, 0.0, 40.0, 40.0));
        n.fills = vec![Paint { opacity: 0.2, ..Paint::solid(Color::WHITE) }];
        n.variables.bindings.insert("fills/0".into(), "alpha-white-20".into());
        n
    });
    t.doc.variable_collections.insert(
        "c".into(),
        VariableCollection {
            id: "c".into(),
            name: "C".into(),
            modes: vec![VariableMode { id: "m".into(), name: "M".into() }],
            default_mode: "m".into(),
            variables: vec!["alpha-white-20".into()],
            key: None,
            source_library_key: None,
            extensions: Default::default(),
        },
    );
    let alpha_white_20 = Color { r: 1.0, g: 1.0, b: 1.0, a: 0.2 };
    t.doc.variables.insert(
        "alpha-white-20".into(),
        Variable {
            id: "alpha-white-20".into(),
            name: "Component colors/Alpha/alpha-white-20".into(),
            collection: "c".into(),
            kind: VariableKind::Color,
            values: [("m".to_string(), VariableValue::Color(alpha_white_20))].into_iter().collect(),
            description: String::new(),
            scopes: vec![],
            hidden_from_publishing: false,
            key: None,
            code_syntax: Default::default(),
            extensions: Default::default(),
            mode_extensions: Default::default(),
        },
    );

    let vp = Viewport { offset: Vec2::ZERO, zoom: 1.0, dpr: 1.0 };
    let c = compare(&mut g, "alpha_white_20", &t.doc, page, vp, (40, 40), 2);
    assert_eq!(c.report.mismatched, 0);
    let p = c.gpu.pixel(20, 20).unwrap();
    assert!((p.red() as i32 - 51).abs() <= 2 && (p.green() as i32 - 51).abs() <= 2 && (p.blue() as i32 - 51).abs() <= 2, "got {p:?}");
}

#[test]
fn text_drop_shadow_uses_the_bound_colour_alpha_like_cpu() {
    let Some(mut g) = gpu() else { return };
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| std::env::set_var("DESIGNOS_BUNDLED_FONTS_ONLY", "1"));
    const SHADOW_DX: u32 = 60;
    let mut t = T::new();
    let page = t.page;
    t.node(page).kind = NodeKind::Page { background: Color::rgb(128, 128, 128) };
    t.add(None, |id| {
        let style = TextStyle { font_size: 100.0, font_weight: 900, fills: vec![Paint { opacity: 0.2, ..Paint::solid(Color::WHITE) }], ..Default::default() };
        let mut n = Node::text(id, Rect::new(10.0, 10.0, 60.0, 120.0), "I", style);
        if let NodeKind::Text(tx) = &mut n.kind {
            tx.auto_resize = TextAutoResize::WidthAndHeight;
        }
        n.variables.bindings.insert("fills/0".into(), "alpha-white-20".into());
        n.effects = vec![Effect::DropShadow {
            color: Color::rgba(0, 0, 0, 255),
            offset: Vec2::new(SHADOW_DX as f32, 0.0),
            radius: 0.0,
            spread: 0.0,
            visible: true,
            show_behind: false,
            blend: Default::default(),
        }];
        n
    });
    t.doc.variable_collections.insert(
        "c".into(),
        VariableCollection {
            id: "c".into(),
            name: "C".into(),
            modes: vec![VariableMode { id: "m".into(), name: "M".into() }],
            default_mode: "m".into(),
            variables: vec!["alpha-white-20".into()],
            key: None,
            source_library_key: None,
            extensions: Default::default(),
        },
    );
    t.doc.variables.insert(
        "alpha-white-20".into(),
        Variable {
            id: "alpha-white-20".into(),
            name: "Component colors/Alpha/alpha-white-20".into(),
            collection: "c".into(),
            kind: VariableKind::Color,
            values: [("m".to_string(), VariableValue::Color(Color { r: 1.0, g: 1.0, b: 1.0, a: 0.2 }))].into_iter().collect(),
            description: String::new(),
            scopes: vec![],
            hidden_from_publishing: false,
            key: None,
            code_syntax: Default::default(),
            extensions: Default::default(),
            mode_extensions: Default::default(),
        },
    );

    let vp = Viewport { offset: Vec2::ZERO, zoom: 1.0, dpr: 1.0 };
    let c = compare(&mut g, "text_shadow_bound_alpha", &t.doc, page, vp, (220, 140), 8);
    assert!(c.report.ratio < 0.02, "mismatch {:.3}%", c.report.ratio * 100.0);
    // glyph stem from the CPU reference, then the shadow sample SHADOW_DX to the right
    let row = 70;
    let lit: Vec<u32> = (0..220).filter(|&x| c.cpu.pixel(x, row).unwrap().red() > 140).collect();
    assert!(!lit.is_empty(), "glyph not found");
    let stem = (lit[0] + lit[lit.len() - 1]) / 2;
    // 20% black over 128 grey is ~102; a fully inked shadow would be ~0
    for (who, pm) in [("cpu", &c.cpu), ("gpu", &c.gpu)] {
        let p = pm.pixel(stem + SHADOW_DX, row).unwrap();
        assert!([p.red(), p.green(), p.blue()].iter().all(|v| (*v as i32 - 102).abs() <= 3), "{who} shadow got {p:?}");
    }
}

#[test]
fn unknown_layer_children_match_cpu() {
    let Some(mut g) = gpu() else { return };
    let mut t = T::new();
    let blue = Color { r: 0.0, g: 0.0, b: 1.0, a: 1.0 };
    let red = Color { r: 1.0, g: 0.0, b: 0.0, a: 1.0 };
    let table = t.add(None, |id| {
        let mut n = Node::new(id, NodeKind::Unknown { type_name: "TABLE".into() });
        n.transform = Affine::translate(10.0, 20.0);
        n.size = Vec2::new(60.0, 60.0);
        n.fills = vec![Paint::solid(blue)];
        n
    });
    let cell = t.add(Some(table), |id| {
        let mut n = Node::new(id, NodeKind::Unknown { type_name: "TABLE_CELL".into() });
        n.transform = Affine::translate(5.0, 6.0);
        n.size = Vec2::new(40.0, 30.0);
        n.fills = vec![Paint::solid(blue)];
        n
    });
    t.rect(Some(cell), Rect::new(2.0, 4.0, 20.0, 10.0), red);
    let vp = Viewport { offset: Vec2::ZERO, zoom: 1.0, dpr: 1.0 };
    let c = compare(&mut g, "unknown_table_children", &t.doc, t.page, vp, (80, 80), 2);
    assert_eq!(c.report.mismatched, 0);
    let p_in = c.gpu.pixel(25, 35).unwrap();
    assert_eq!([p_in.red(), p_in.green(), p_in.blue(), p_in.alpha()], [255, 0, 0, 255]);
    let p_out = c.gpu.pixel(12, 22).unwrap();
    assert_eq!([p_out.red(), p_out.green(), p_out.blue(), p_out.alpha()], [255, 255, 255, 255]);
}

#[test]
fn per_side_stroke_rounded_corners_match_cpu() {
    let Some(mut g) = gpu() else { return };
    let mut t = T::new();
    t.doc.get_mut(t.page).unwrap().kind = NodeKind::Page { background: Color::WHITE };
    let r = t.rect(None, Rect::new(0.0, 0.0, 160.0, 156.0), Color::WHITE);
    let n = t.node(r);
    n.corner_radii = [12.0; 4];
    n.stroke.paints = vec![Paint::solid(Color::rgb(0xD9, 0x3B, 0x2A))];
    n.stroke.weight = 1.0;
    n.stroke.side_weights = Some([0.0, 1.0, 1.0, 1.0]);
    n.stroke.align = StrokeAlign::Inside;
    let c = compare(&mut g, "per_side_rounded", &t.doc, t.page, Viewport::default(), (160, 156), 8);
    let top_gpu = c.gpu.pixel(80, 0).unwrap();
    assert_eq!([top_gpu.red(), top_gpu.green(), top_gpu.blue()], [255, 255, 255], "top edge row must stay unpainted on GPU");
    let bl_gpu = c.gpu.pixel(4, 152).unwrap();
    assert!(bl_gpu.red() > 210 && bl_gpu.green() < 200, "bottom-left corner arc at 45 degrees must be painted on GPU, got {bl_gpu:?}");
    assert!(c.report.ratio < 0.01, "strict mismatch {:.3}%", c.report.ratio * 100.0);
    assert!(c.report_aa.ratio < 0.002, "mismatch with AA forgiven {:.3}%", c.report_aa.ratio * 100.0);
}

#[test]
fn outside_stroke_with_imported_outer_disc_keeps_fill_in_centre() {
    let Some(mut g) = gpu() else { return };
    let mut t = T::new();
    t.doc.get_mut(t.page).unwrap().kind = NodeKind::Page { background: Color::rgb(0x20, 0x20, 0x20) };
    let e = t.add(None, |id| Node::ellipse(id, Rect::new(6.0, 6.0, 8.0, 8.0)));
    let n = t.node(e);
    n.fills = vec![Paint::solid(Color::rgb(0xD9, 0x3B, 0x2A))];
    n.stroke.paints = vec![Paint::solid(Color::WHITE)];
    n.stroke.weight = 1.5;
    n.stroke.align = StrokeAlign::Outside;
    let v = Vec2::new;
    n.stroke_geometry = Some(vec![Path {
        cmds: vec![PathCmd::MoveTo(v(-1.5, -1.5)), PathCmd::LineTo(v(9.5, -1.5)), PathCmd::LineTo(v(9.5, 9.5)), PathCmd::LineTo(v(-1.5, 9.5)), PathCmd::Close],
        fill_rule: FillRule::NonZero,
    }]);
    let c = compare(&mut g, "outside_stroke_imported_disc", &t.doc, t.page, Viewport::default(), (20, 20), 8);
    let center_cpu = c.cpu.pixel(10, 10).unwrap();
    let center_gpu = c.gpu.pixel(10, 10).unwrap();
    assert_eq!([center_cpu.red(), center_cpu.green(), center_cpu.blue()], [0xD9, 0x3B, 0x2A], "CPU centre must keep red fill");
    assert!(center_gpu.red() > 200 && center_gpu.green() < 80 && center_gpu.blue() < 60, "GPU centre must keep red fill, got {center_gpu:?}");
    let ring_gpu = c.gpu.pixel(5, 10).unwrap();
    assert!(ring_gpu.red() > 240 && ring_gpu.green() > 240 && ring_gpu.blue() > 240, "GPU outside ring must be white, got {ring_gpu:?}");
    assert_eq!(c.report_aa.mismatched, 0, "{:?}", c.report_aa);
    assert!(c.report.ssim > 0.99, "{:?}", c.report);
}

#[test]
fn radial_gradient_non_square_matches_cpu() {
    let Some(mut g) = gpu() else { return };
    let mut t = T::new();
    let f = t.rect(None, Rect::new(0.0, 0.0, 200.0, 100.0), Color::WHITE);
    let handles = [Vec2::new(0.5, 0.5), Vec2::new(0.5, 1.0), Vec2::new(0.0, 0.5)];
    let stops = vec![GradientStop { pos: 0.0, color: Color::rgb(255, 0, 0) }, GradientStop { pos: 1.0, color: Color::rgb(0, 0, 255) }];
    t.node(f).fills = vec![Paint { kind: PaintKind::RadialGradient { handles, stops }, opacity: 1.0, visible: true, blend: BlendMode::Normal }];
    let vp = Viewport { offset: Vec2::ZERO, zoom: 1.0, dpr: 1.0 };
    let c = compare(&mut g, "radial_non_square", &t.doc, t.page, vp, (200, 100), 4);
    assert!(c.report.ratio < 0.01, "mismatch {:.3}%", c.report.ratio * 100.0);
    let centre = c.gpu.pixel(100, 50).unwrap();
    assert!(centre.red() > 245 && centre.blue() < 10, "centre (100, 50): {centre:?}");
    for (x, y) in [(0, 50), (199, 50), (100, 0), (100, 99)] {
        let p = c.gpu.pixel(x, y).unwrap();
        assert!(p.blue() > 245 && p.red() < 10, "edge ({x}, {y}): {p:?}");
    }
}

#[test]
fn soft_line_break_u2028_matches_cpu() {
    let Some(mut g) = gpu() else { return };
    let mut t = T::new();
    let mut st = TextStyle { font_size: 20.0, line_height: LineHeight::Pixels(24.0), paragraph_spacing: 40.0, ..Default::default() };
    st.fills = vec![Paint::solid(Color::BLACK)];
    t.add(None, |id| Node::text(id, Rect::new(10.0, 10.0, 300.0, 120.0), "Still thinking\u{2028}about it?", st));

    let vp = Viewport { offset: Vec2::ZERO, zoom: 1.0, dpr: 1.0 };
    let c = compare(&mut g, "soft_line_break_u2028", &t.doc, t.page, vp, (320, 140), 8);
    assert!(c.report.ratio < 0.02, "mismatch {:.3}%", c.report.ratio * 100.0);
    // Second line should have ink in y=34..58 (line 2 at top=34..58), not y=74..98 (which would include 40px paragraph spacing)
    let has_ink_line2 = (10..150).any(|x| (34..58).any(|y| c.gpu.pixel(x, y).unwrap().red() < 200));
    assert!(has_ink_line2, "expected ink on second line after U+2028 without paragraph spacing");
}

#[test]
fn reverse_z_auto_layout_draws_masked_pattern_behind_heading_matches_cpu() {
    let Some(mut g) = gpu() else { return };
    let mut t = T::new();
    let hero = t.add(None, |id| {
        let mut n = Node::frame(id, Rect::new(0.0, 0.0, 80.0, 80.0));
        if let NodeKind::Frame(fp) = &mut n.kind {
            fp.auto_layout = Some(AutoLayout { direction: LayoutDirection::Vertical, reverse_z: true, ..Default::default() });
        }
        n
    });
    t.rect(Some(hero), Rect::new(20.0, 20.0, 40.0, 40.0), Color::rgb(0, 0, 255));
    let pat = t.add(Some(hero), |id| {
        let mut n = Node::frame(id, Rect::new(0.0, 0.0, 80.0, 80.0));
        n.fills.clear();
        n.layout_child.absolute = true;
        n
    });
    let mask = t.add(Some(pat), |id| {
        let mut n = Node::frame(id, Rect::new(0.0, 0.0, 80.0, 80.0));
        n.fills.clear();
        n.is_mask = true;
        n
    });
    t.rect(Some(mask), Rect::new(0.0, 0.0, 80.0, 80.0), Color::rgba(0, 0, 255, 128));
    t.rect(Some(pat), Rect::new(0.0, 0.0, 80.0, 80.0), Color::rgb(255, 0, 0));

    let vp = Viewport { offset: Vec2::ZERO, zoom: 1.0, dpr: 1.0 };
    let c = compare(&mut g, "reverse_z_mask", &t.doc, t.page, vp, (80, 80), 4);
    assert!(c.report.ratio < 0.01, "mismatch {:.3}%", c.report.ratio * 100.0);
    let heading_px = c.gpu.pixel(30, 30).unwrap();
    assert_eq!([heading_px.red(), heading_px.green(), heading_px.blue(), heading_px.alpha()], [0, 0, 255, 255]);
    let bg_px = c.gpu.pixel(10, 10).unwrap();
    assert!(bg_px.red() == 255 && (bg_px.green() as i32 - 128).abs() <= 2 && (bg_px.blue() as i32 - 128).abs() <= 2, "{bg_px:?}");
}

#[test]
fn auto_layout_reverse_z_matches_cpu() {
    let Some(mut g) = gpu() else { return };
    let mut t = T::new();
    let red = Color { r: 1.0, g: 0.0, b: 0.0, a: 1.0 };
    let blue = Color { r: 0.0, g: 0.0, b: 1.0, a: 1.0 };
    let row = t.add(None, |id| {
        let mut n = Node::frame(id, Rect::new(0.0, 0.0, 40.0, 20.0));
        n.fills.clear();
        if let NodeKind::Frame(fp) = &mut n.kind {
            fp.clips_content = false;
            fp.auto_layout = Some(AutoLayout { direction: LayoutDirection::Horizontal, item_spacing: -12.0, reverse_z: true, ..Default::default() });
        }
        n
    });
    t.rect(Some(row), Rect::new(0.0, 0.0, 20.0, 20.0), red);
    t.rect(Some(row), Rect::new(0.0, 0.0, 20.0, 20.0), blue);
    t.doc.relayout_subtree(row);

    // Also test a clipped reverse_z frame (exercises the clipped unit-quad fast path on GPU).
    let clipped_swatch = t.add(None, |id| {
        let mut n = Node::frame(id, Rect::new(0.0, 30.0, 40.0, 40.0));
        n.fills = vec![Paint::solid(Color::WHITE)];
        if let NodeKind::Frame(fp) = &mut n.kind {
            fp.clips_content = true;
            fp.auto_layout = Some(AutoLayout { direction: LayoutDirection::Vertical, item_spacing: -12.0, reverse_z: true, ..Default::default() });
        }
        n
    });
    let swatch = t.rect(Some(clipped_swatch), Rect::new(0.0, 0.0, 40.0, 20.0), red);
    t.node(swatch).layout_child.absolute = true;
    t.rect(Some(clipped_swatch), Rect::new(0.0, 0.0, 40.0, 40.0), blue);
    t.doc.relayout_subtree(clipped_swatch);

    let vp = Viewport { offset: Vec2::ZERO, zoom: 1.0, dpr: 1.0 };
    let c = compare(&mut g, "auto_layout_reverse_z", &t.doc, t.page, vp, (60, 80), 2);
    assert_eq!(c.report.mismatched, 0);
    let p_overlap = c.gpu.pixel(14, 10).unwrap();
    assert_eq!([p_overlap.red(), p_overlap.green(), p_overlap.blue(), p_overlap.alpha()], [255, 0, 0, 255]);
    let p_swatch = c.gpu.pixel(20, 40).unwrap();
    assert_eq!([p_swatch.red(), p_swatch.green(), p_swatch.blue(), p_swatch.alpha()], [255, 0, 0, 255]);
}

#[test]
fn dashed_rect_round_dash_cap_renders_round_dash_ends_gpu() {
    let Some(mut g) = gpu() else { return };
    let mut t = T::new();
    let red = Color { r: 1.0, g: 0.0, b: 0.0, a: 1.0 };
    let r = t.add(None, |id| {
        let mut n = Node::rectangle(id, Rect::new(20.0, 20.0, 60.0, 40.0));
        n.fills.clear();
        n.stroke.paints = vec![Paint::solid(red)];
        n.stroke.weight = 8.0;
        n.stroke.align = StrokeAlign::Inside;
        n.stroke.dashes = vec![20.0, 20.0];
        n.stroke.cap = StrokeCap::Round;
        n
    });
    let vp = Viewport { offset: Vec2::ZERO, zoom: 1.0, dpr: 1.0 };
    let c = compare(&mut g, "dashed_rect_round_cap", &t.doc, t.page, vp, (100, 80), 8);
    assert!(c.report.ratio < 0.02, "CPU/GPU mismatch {:.3}%", c.report.ratio * 100.0);
    let mid = c.gpu.pixel(46, 24).unwrap();
    assert_eq!([mid.red(), mid.green(), mid.blue(), mid.alpha()], [255, 0, 0, 255], "GPU round dash cap painted on midline (46, 24)");
    let corner = c.gpu.pixel(47, 20).unwrap();
    assert_eq!([corner.red(), corner.green(), corner.blue(), corner.alpha()], [255, 255, 255, 255], "GPU round dash cap leaves outer corner (47, 20) white");

    t.node(r).stroke.cap = StrokeCap::None;
    g.compiler.geom.clear();
    let c_none = compare(&mut g, "dashed_rect_butt_cap", &t.doc, t.page, vp, (100, 80), 8);
    let mid_none = c_none.gpu.pixel(46, 24).unwrap();
    assert_eq!([mid_none.red(), mid_none.green(), mid_none.blue(), mid_none.alpha()], [255, 255, 255, 255]);

    t.node(r).stroke.align = StrokeAlign::Outside;
    t.node(r).stroke.cap = StrokeCap::Round;
    g.compiler.geom.clear();
    let c_outside = compare(&mut g, "dashed_rect_outside_round_cap", &t.doc, t.page, vp, (120, 100), 8);
    assert!(c_outside.report.ratio < 0.02, "CPU/GPU mismatch {:.3}%", c_outside.report.ratio * 100.0);
    let out_mid = c_outside.gpu.pixel(38, 16).unwrap();
    assert_eq!([out_mid.red(), out_mid.green(), out_mid.blue(), out_mid.alpha()], [255, 0, 0, 255], "GPU outside round dash cap painted at (38, 16)");
    let out_gap = c_outside.gpu.pixel(46, 16).unwrap();
    assert_eq!([out_gap.red(), out_gap.green(), out_gap.blue(), out_gap.alpha()], [255, 255, 255, 255], "GPU outside dash gap unpainted at (46, 16)");
}

/// An open vector line from (20, 20) to (100, 20) with dashes `[20, 20]` (dashes at x = 20..40 and 60..80, the path ends in the
/// gap after the second one), weight 8, `Stroke::cap` Round, whose end vertices carry `vertex_caps`.
fn dashed_round_line_with_end_caps(t: &mut T, vertex_caps: (Option<StrokeCap>, Option<StrokeCap>)) {
    let red = Color { r: 1.0, g: 0.0, b: 0.0, a: 1.0 };
    t.add(None, |id| {
        let p = Path { cmds: vec![PathCmd::MoveTo(Vec2::new(0.0, 0.0)), PathCmd::LineTo(Vec2::new(80.0, 0.0))], fill_rule: FillRule::NonZero };
        let mut v0 = vector::Vertex::new(Vec2::new(0.0, 0.0));
        v0.cap = vertex_caps.0;
        let mut v1 = vector::Vertex::new(Vec2::new(80.0, 0.0));
        v1.cap = vertex_caps.1;
        let mut n = Node::new(
            id,
            NodeKind::Vector {
                paths: vec![p],
                network: Some(VectorNetwork {
                    vertices: vec![v0, v1],
                    segments: vec![vector::Segment { a: 0, b: 1, ta: Vec2::ZERO, tb: Vec2::ZERO }],
                    regions: Vec::new(),
                }),
            },
        );
        n.transform = Affine::translate(20.0, 20.0);
        n.size = Vec2::new(80.0, 0.0);
        n.fills.clear();
        n.stroke.paints = vec![Paint::solid(red)];
        n.stroke.weight = 8.0;
        n.stroke.align = StrokeAlign::Center;
        n.stroke.dashes = vec![20.0, 20.0];
        n.stroke.cap = StrokeCap::Round;
        n
    });
}

#[test]
fn dashed_open_line_uses_stroke_cap_on_every_dash_that_reaches_no_end_point_gpu() {
    let Some(mut g) = gpu() else { return };
    let mut t = T::new();
    // end vertices without a cap of their own: every dash, the first one at the start of the path included, uses Stroke::cap (Round)
    dashed_round_line_with_end_caps(&mut t, (None, None));
    let vp = Viewport { offset: Vec2::ZERO, zoom: 1.0, dpr: 1.0 };
    let c = compare(&mut g, "dashed_open_line_round_cap", &t.doc, t.page, vp, (120, 40), 8);
    assert!(c.report.ratio < 0.02, "CPU/GPU mismatch {:.3}%", c.report.ratio * 100.0);
    assert_eq!(px(&c.gpu, 18, 20), [255, 0, 0, 255]);
    assert_eq!(px(&c.gpu, 42, 20), [255, 0, 0, 255]);
    assert_eq!(px(&c.gpu, 43, 16), [255, 255, 255, 255]);
}

#[test]
fn an_explicit_butt_end_vertex_cap_leaves_only_the_start_of_a_dashed_round_line_butt_gpu() {
    let Some(mut g) = gpu() else { return };
    let mut t = T::new();
    dashed_round_line_with_end_caps(&mut t, (Some(StrokeCap::None), Some(StrokeCap::None)));
    let vp = Viewport { offset: Vec2::ZERO, zoom: 1.0, dpr: 1.0 };
    let c = compare(&mut g, "dashed_open_line_butt_start", &t.doc, t.page, vp, (120, 40), 8);
    assert!(c.report.ratio < 0.02, "CPU/GPU mismatch {:.3}%", c.report.ratio * 100.0);
    assert_eq!(px(&c.gpu, 18, 20), [255, 255, 255, 255], "the start of the path takes the butt cap of its vertex");
    assert_eq!(px(&c.gpu, 22, 20), [255, 0, 0, 255], "the first dash is painted from the start of the path");
    assert_eq!(px(&c.gpu, 42, 20), [255, 0, 0, 255], "the inner end of the first dash keeps the round Stroke::cap");
    assert_eq!(px(&c.gpu, 43, 16), [255, 255, 255, 255]);
    assert_eq!(px(&c.gpu, 58, 20), [255, 0, 0, 255], "the inner start of the second dash keeps the round Stroke::cap");
}

#[test]
fn solid_open_line_round_vertex_cap_renders_round_gpu() {
    let Some(mut g) = gpu() else { return };
    let mut t = T::new();
    let red = Color { r: 1.0, g: 0.0, b: 0.0, a: 1.0 };
    t.add(None, |id| {
        let p = Path { cmds: vec![PathCmd::MoveTo(Vec2::new(0.0, 0.0)), PathCmd::LineTo(Vec2::new(80.0, 0.0))], fill_rule: FillRule::NonZero };
        let mut v0 = vector::Vertex::new(Vec2::new(0.0, 0.0));
        v0.cap = Some(StrokeCap::Round);
        let v1 = vector::Vertex::new(Vec2::new(80.0, 0.0));
        let mut n = Node::new(
            id,
            NodeKind::Vector {
                paths: vec![p],
                network: Some(VectorNetwork {
                    vertices: vec![v0, v1],
                    segments: vec![vector::Segment { a: 0, b: 1, ta: Vec2::ZERO, tb: Vec2::ZERO }],
                    regions: Vec::new(),
                }),
            },
        );
        n.transform = Affine::translate(20.0, 20.0);
        n.size = Vec2::new(80.0, 0.0);
        n.fills.clear();
        n.stroke.paints = vec![Paint::solid(red)];
        n.stroke.weight = 8.0;
        n.stroke.align = StrokeAlign::Center;
        n.stroke.cap = StrokeCap::Round;
        n
    });
    let vp = Viewport { offset: Vec2::ZERO, zoom: 1.0, dpr: 1.0 };
    let c = compare(&mut g, "solid_open_line_round_vertex_cap", &t.doc, t.page, vp, (120, 40), 8);
    assert!(c.report.ratio < 0.02, "CPU/GPU mismatch {:.3}%", c.report.ratio * 100.0);
    let start_cap = c.gpu.pixel(18, 20).unwrap();
    assert_eq!([start_cap.red(), start_cap.green(), start_cap.blue(), start_cap.alpha()], [255, 0, 0, 255], "GPU round cap painted at (18, 20)");
    let start_corner = c.gpu.pixel(16, 16).unwrap();
    assert_eq!(
        [start_corner.red(), start_corner.green(), start_corner.blue(), start_corner.alpha()],
        [255, 255, 255, 255],
        "GPU round cap leaves outer corner (16, 16) white"
    );
}

#[test]
fn thin_inside_dashed_rect_keeps_stroke_gpu() {
    let Some(mut g) = gpu() else { return };
    let mut t = T::new();
    let red = Color { r: 1.0, g: 0.0, b: 0.0, a: 1.0 };
    t.add(None, |id| {
        let mut n = Node::rectangle(id, Rect::new(10.0, 10.0, 200.0, 4.0));
        n.fills.clear();
        n.stroke.paints = vec![Paint::solid(red)];
        n.stroke.weight = 4.0;
        n.stroke.align = StrokeAlign::Inside;
        n.stroke.dashes = vec![8.0, 60.0];
        n.stroke.cap = StrokeCap::Round;
        n
    });
    let vp = Viewport { offset: Vec2::ZERO, zoom: 1.0, dpr: 1.0 };
    let c = compare(&mut g, "thin_inside_dashed_rect", &t.doc, t.page, vp, (220, 24), 8);
    assert!(c.report.ratio < 0.02, "CPU/GPU mismatch {:.3}%", c.report.ratio * 100.0);
    let dash_px = c.gpu.pixel(14, 12).unwrap();
    assert_eq!(
        [dash_px.red(), dash_px.green(), dash_px.blue(), dash_px.alpha()],
        [255, 0, 0, 255],
        "GPU thin inside dashed rectangle paints the first dash at (14, 12)"
    );
    let cap_px = c.gpu.pixel(19, 11).unwrap();
    assert_eq!(
        [cap_px.red(), cap_px.green(), cap_px.blue(), cap_px.alpha()],
        [255, 0, 0, 255],
        "GPU thin inside dashed rectangle paints the round cap just past the dash end at (19, 11)"
    );
    let gap_px = c.gpu.pixel(40, 12).unwrap();
    assert_eq!(
        [gap_px.red(), gap_px.green(), gap_px.blue(), gap_px.alpha()],
        [255, 255, 255, 255],
        "GPU thin inside dashed rectangle leaves the middle of the gap white at (40, 12)"
    );
}

#[test]
fn inside_dashed_ellipse_round_cap_extends_past_dash_end_gpu() {
    let Some(mut g) = gpu() else { return };
    let mut t = T::new();
    let red = Color { r: 1.0, g: 0.0, b: 0.0, a: 1.0 };
    let e = t.add(None, |id| {
        let mut n = Node::ellipse(id, Rect::new(10.0, 10.0, 80.0, 80.0));
        n.fills.clear();
        n.stroke.paints = vec![Paint::solid(red)];
        n.stroke.weight = 4.0;
        n.stroke.align = StrokeAlign::Inside;
        n.stroke.dashes = vec![40.0, 20.0];
        n.stroke.cap = StrokeCap::Round;
        n
    });
    let vp = Viewport { offset: Vec2::ZERO, zoom: 1.0, dpr: 1.0 };
    let c = compare(&mut g, "inside_dashed_ellipse_round_cap", &t.doc, t.page, vp, (100, 100), 8);
    assert!(c.report_aa.ratio < 0.005, "mismatch with AA forgiven {:.3}%", c.report_aa.ratio * 100.0);
    let body = c.gpu.pixel(83, 68).unwrap();
    assert_eq!([body.red(), body.green(), body.blue(), body.alpha()], [255, 0, 0, 255], "GPU inside dash body painted at (83, 68)");
    let cap = c.gpu.pixel(68, 83).unwrap();
    assert_eq!([cap.red(), cap.green(), cap.blue(), cap.alpha()], [255, 0, 0, 255], "GPU round cap painted 2.6px past the dash end at (68, 83)");
    let gap = c.gpu.pixel(61, 86).unwrap();
    assert_eq!([gap.red(), gap.green(), gap.blue(), gap.alpha()], [255, 255, 255, 255], "GPU middle of the gap white at (61, 86)");

    t.node(e).stroke.cap = StrokeCap::None;
    g.compiler.geom.clear();
    let c_butt = compare(&mut g, "inside_dashed_ellipse_butt_cap", &t.doc, t.page, vp, (100, 100), 8);
    let butt = c_butt.gpu.pixel(68, 83).unwrap();
    assert_eq!([butt.red(), butt.green(), butt.blue(), butt.alpha()], [255, 255, 255, 255], "GPU butt dash ends at the dash end so (68, 83) is white");
}

fn png_bytes(w: u32, h: u32, f: impl Fn(u32, u32) -> [u8; 4]) -> Vec<u8> {
    let mut pm = designos_render::tiny_skia::Pixmap::new(w, h).unwrap();
    for y in 0..h {
        for x in 0..w {
            let c = f(x, y);
            let i = ((y * w + x) * 4) as usize;
            pm.data_mut()[i..i + 4].copy_from_slice(&c);
        }
    }
    pm.encode_png().unwrap()
}

fn px(pm: &designos_render::tiny_skia::Pixmap, x: u32, y: u32) -> [u8; 4] {
    let p = pm.pixel(x, y).unwrap();
    [p.red(), p.green(), p.blue(), p.alpha()]
}

#[test]
fn rotation_90_is_exact_on_a_2x2_image() {
    let Some(mut g) = gpu() else { return };
    let mut t = T::new();
    t.doc.images.insert(
        "quad".into(),
        png_bytes(2, 2, |x, y| match (x, y) {
            (0, 0) => [255, 0, 0, 255],
            (1, 0) => [0, 255, 0, 255],
            (0, 1) => [0, 0, 255, 255],
            _ => [255, 255, 0, 255],
        }),
    );
    let r = t.rect(None, Rect::new(0.0, 0.0, 2.0, 2.0), Color::WHITE);
    t.node(r).fills = vec![Paint {
        kind: PaintKind::Image { image_ref: "quad".into(), scale_mode: ImageScaleMode::Fill, params: ImageParams { rotation: 90.0, ..Default::default() } },
        opacity: 1.0,
        visible: true,
        blend: BlendMode::Normal,
    }];
    let vp = Viewport { offset: Vec2::ZERO, zoom: 1.0, dpr: 1.0 };
    let c = compare(&mut g, "rotation_90_2x2", &t.doc, t.page, vp, (2, 2), 0);
    assert_eq!(c.cpu.data(), c.gpu.data(), "CPU and GPU match exactly on 90-degree rotated 2x2 image");
    assert_eq!(px(&c.gpu, 0, 0), [0, 0, 255, 255]);
    assert_eq!(px(&c.gpu, 1, 0), [255, 0, 0, 255]);
    assert_eq!(px(&c.gpu, 0, 1), [255, 255, 0, 255]);
    assert_eq!(px(&c.gpu, 1, 1), [0, 255, 0, 255]);
}

#[test]
fn tile_scale_2_doubles_the_tile_size() {
    let Some(mut g) = gpu() else { return };
    let mut t = T::new();
    t.doc.images.insert("tile_rb".into(), png_bytes(20, 10, |x, _| if (5..15).contains(&x) { [0, 0, 255, 255] } else { [255, 0, 0, 255] }));
    let r = t.rect(None, Rect::new(0.0, 0.0, 40.0, 20.0), Color::WHITE);
    t.node(r).fills = vec![Paint {
        kind: PaintKind::Image { image_ref: "tile_rb".into(), scale_mode: ImageScaleMode::Tile, params: ImageParams { tile_scale: 2.0, ..Default::default() } },
        opacity: 1.0,
        visible: true,
        blend: BlendMode::Normal,
    }];
    let vp = Viewport { offset: Vec2::ZERO, zoom: 1.0, dpr: 1.0 };
    let c = compare(&mut g, "tile_scale_2", &t.doc, t.page, vp, (40, 20), 8);
    assert!(c.report.ratio < 0.02, "CPU/GPU mismatch {:.3}%", c.report.ratio * 100.0);
    assert_eq!(px(&c.gpu, 7, 5), [255, 0, 0, 255]);
    assert_eq!(px(&c.gpu, 20, 5), [0, 0, 255, 255]);
}

#[test]
fn crop_with_identity_transform_stretches_the_image() {
    let Some(mut g) = gpu() else { return };
    let mut t = T::new();
    t.doc.images.insert(
        "bands".into(),
        png_bytes(40, 40, |x, _| {
            if x < 8 {
                [255, 0, 0, 255]
            } else if x < 32 {
                [0, 255, 0, 255]
            } else {
                [0, 0, 255, 255]
            }
        }),
    );
    let r = t.rect(None, Rect::new(0.0, 0.0, 20.0, 40.0), Color::WHITE);
    t.node(r).fills = vec![Paint {
        kind: PaintKind::Image { image_ref: "bands".into(), scale_mode: ImageScaleMode::Crop, params: ImageParams::default() },
        opacity: 1.0,
        visible: true,
        blend: BlendMode::Normal,
    }];
    let vp = Viewport { offset: Vec2::ZERO, zoom: 1.0, dpr: 1.0 };
    let c = compare(&mut g, "crop_identity_stretch", &t.doc, t.page, vp, (20, 40), 8);
    assert!(c.report.ratio < 0.02, "CPU/GPU mismatch {:.3}%", c.report.ratio * 100.0);
    assert_eq!(px(&c.gpu, 1, 20), [255, 0, 0, 255]);
    assert_eq!(px(&c.gpu, 19, 20), [0, 0, 255, 255]);
}

#[test]
fn crop_transform_maps_uv_through_the_affine() {
    let Some(mut g) = gpu() else { return };
    let mut t = T::new();
    t.doc.images.insert("half_rb".into(), png_bytes(20, 20, |x, _| if x < 10 { [255, 0, 0, 255] } else { [0, 0, 255, 255] }));
    let r = t.rect(None, Rect::new(0.0, 0.0, 40.0, 40.0), Color::TRANSPARENT);
    t.node(r).fills = vec![Paint {
        kind: PaintKind::Image {
            image_ref: "half_rb".into(),
            scale_mode: ImageScaleMode::Crop,
            params: ImageParams { transform: Some([2.0, 0.0, -0.5, 0.0, 2.0, -0.5]), ..Default::default() },
        },
        opacity: 1.0,
        visible: true,
        blend: BlendMode::Normal,
    }];
    let vp = Viewport { offset: Vec2::ZERO, zoom: 1.0, dpr: 1.0 };
    let c = compare(&mut g, "crop_affine_uv", &t.doc, t.page, vp, (40, 40), 8);
    assert!(c.report.ratio < 0.05, "CPU/GPU mismatch {:.3}%", c.report.ratio * 100.0);
    assert_eq!(px(&c.gpu, 3, 20), [255, 255, 255, 255]);
    assert_eq!(px(&c.gpu, 37, 20), [255, 255, 255, 255]);
    assert_eq!(px(&c.gpu, 15, 20), [255, 0, 0, 255]);
    assert_eq!(px(&c.gpu, 25, 20), [0, 0, 255, 255]);
}

#[test]
fn singular_crop_transform_draws_nothing() {
    let Some(mut g) = gpu() else { return };
    let mut t = T::new();
    t.doc.images.insert("solid_red".into(), png_bytes(20, 20, |_, _| [255, 0, 0, 255]));
    let r = t.rect(None, Rect::new(0.0, 0.0, 40.0, 40.0), Color::TRANSPARENT);
    t.node(r).fills = vec![Paint {
        kind: PaintKind::Image {
            image_ref: "solid_red".into(),
            scale_mode: ImageScaleMode::Crop,
            params: ImageParams { transform: Some([0.0, 0.0, 0.0, 0.0, 0.0, 0.0]), ..Default::default() },
        },
        opacity: 1.0,
        visible: true,
        blend: BlendMode::Normal,
    }];
    let vp = Viewport { offset: Vec2::ZERO, zoom: 1.0, dpr: 1.0 };
    let c = compare(&mut g, "singular_crop", &t.doc, t.page, vp, (40, 40), 8);
    assert_eq!(c.report.mismatched, 0);
    assert_eq!(px(&c.gpu, 20, 20), [255, 255, 255, 255]);
}

fn tiled_halves_scene(tile_scale: f32) -> T {
    let mut t = T::new();
    t.doc.images.insert("halves".into(), png_bytes(2000, 1000, |x, _| if x < 1000 { [255, 0, 0, 255] } else { [0, 0, 255, 255] }));
    let r = t.rect(None, Rect::new(0.0, 0.0, 40.0, 20.0), Color::WHITE);
    t.node(r).fills = vec![Paint {
        kind: PaintKind::Image { image_ref: "halves".into(), scale_mode: ImageScaleMode::Tile, params: ImageParams { tile_scale, ..Default::default() } },
        opacity: 1.0,
        visible: true,
        blend: BlendMode::Normal,
    }];
    t
}

#[test]
fn tile_scale_zero_and_negative_render_at_the_minimum_on_cpu_and_gpu() {
    let Some(mut g) = gpu() else { return };
    let vp = Viewport { offset: Vec2::ZERO, zoom: 1.0, dpr: 1.0 };
    let t = tiled_halves_scene(0.01);
    let minimum = compare(&mut g, "tile_scale_minimum", &t.doc, t.page, vp, (40, 20), 8);
    // The CPU averages the 2000x1000 picture down to the 20x10 tile and the GPU point-samples it, so the pixels
    // on the red/blue seams differ; with those forgiven the two renders agree.
    assert!(minimum.report_aa.ratio < 0.02, "CPU/GPU mismatch with seams forgiven {:.3}%", minimum.report_aa.ratio * 100.0);
    assert_eq!(px(&minimum.gpu, 5, 5), [255, 0, 0, 255], "first tile red half");
    assert_eq!(px(&minimum.gpu, 15, 5), [0, 0, 255, 255], "first tile blue half");
    assert_eq!(px(&minimum.gpu, 25, 5), [255, 0, 0, 255], "second tile red half");
    for raw in [0.0, -0.0, -3.0] {
        let t = tiled_halves_scene(raw);
        let c = compare(&mut g, "tile_scale_clamped", &t.doc, t.page, vp, (40, 20), 8);
        assert!(c.gpu.data() == minimum.gpu.data(), "GPU: tile scale {raw} renders as 0.01");
        assert!(c.cpu.data() == minimum.cpu.data(), "CPU: tile scale {raw} renders as 0.01");
    }
}

#[test]
fn non_finite_tile_scale_renders_at_native_size_on_cpu_and_gpu() {
    let Some(mut g) = gpu() else { return };
    let vp = Viewport { offset: Vec2::ZERO, zoom: 1.0, dpr: 1.0 };
    let t = tiled_halves_scene(1.0);
    let native = compare(&mut g, "tile_scale_native", &t.doc, t.page, vp, (40, 20), 8);
    for raw in [f32::NAN, f32::INFINITY] {
        let t = tiled_halves_scene(raw);
        let c = compare(&mut g, "tile_scale_non_finite", &t.doc, t.page, vp, (40, 20), 8);
        assert!(c.gpu.data() == native.gpu.data(), "GPU: tile scale {raw} renders at native size");
        assert!(c.cpu.data() == native.cpu.data(), "CPU: tile scale {raw} renders at native size");
    }
}

fn solid_image_scene(rgba: [u8; 4], adjust: ImageAdjust, bg: Color) -> T {
    let mut t = T::new();
    t.node(t.page).kind = NodeKind::Page { background: bg };
    let key = format!("solid_{:02x}_{:02x}_{:02x}_{:02x}", rgba[0], rgba[1], rgba[2], rgba[3]);
    t.doc.images.insert(key.clone(), png_bytes(16, 16, |_, _| rgba));
    let r = t.rect(None, Rect::new(0.0, 0.0, 16.0, 16.0), Color::BLACK);
    t.node(r).fills = vec![Paint {
        kind: PaintKind::Image { image_ref: key, scale_mode: ImageScaleMode::Fill, params: ImageParams { adjust, ..ImageParams::default() } },
        opacity: 1.0,
        visible: true,
        blend: BlendMode::Normal,
    }];
    t
}

fn spec_step_adjust_rgba(rgba: [u8; 4], adj: &ImageAdjust) -> [u8; 4] {
    if rgba[3] == 0 {
        return [0, 0, 0, 0];
    }
    let mut lin = [(rgba[0] as f64 / 255.0).powf(2.2), (rgba[1] as f64 / 255.0).powf(2.2), (rgba[2] as f64 / 255.0).powf(2.2)];
    let luma_w = [0.2126_f64, 0.7152, 0.0722];
    let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let clamp01 = |v: [f64; 3]| [v[0].clamp(0.0, 1.0), v[1].clamp(0.0, 1.0), v[2].clamp(0.0, 1.0)];

    // Step 2: shadows and highlights offset ai
    let h = dot(lin, luma_w) - 0.5_f64.powf(2.2);
    let ai = 0.75 * h / (1.0 + 4.0 * h * h).sqrt() * if h > 0.0 { adj.highlights as f64 } else { -0.999 * adj.shadows as f64 };

    // Step 3: Lab (D65, plain cube root)
    if adj.tint != 0.0 || adj.temperature != 0.0 || adj.contrast != 0.0 {
        let x_xyz = (0.412_456_4 * lin[0] + 0.357_576_1 * lin[1] + 0.180_437_5 * lin[2]) / 0.95047;
        let y_xyz = 0.212_672_9 * lin[0] + 0.715_152_2 * lin[1] + 0.072_175_0 * lin[2];
        let z_xyz = (0.019_333_9 * lin[0] + 0.119_192_0 * lin[1] + 0.950_304_1 * lin[2]) / 1.08883;

        let fx = x_xyz.max(0.0).powf(1.0 / 3.0);
        let fy = y_xyz.max(0.0).powf(1.0 / 3.0);
        let fz = z_xyz.max(0.0).powf(1.0 / 3.0);

        let mut l_lab = 116.0 * fy - 16.0;
        let a_lab = 500.0 * (fx - fy) + 30.0 * adj.tint as f64;
        let b_lab = 200.0 * (fy - fz) + 30.0 * adj.temperature as f64;

        let c = (adj.contrast as f64).clamp(-0.3, 0.3);
        let l_c = 50.0;
        l_lab = if c < 0.0 {
            let low = 50.0 + 0.2 * (l_lab - l_c);
            low + (l_lab - low) * (c + 1.0)
        } else {
            let t = ((l_lab - (l_c - 30.0)) / 60.0).clamp(0.0, 1.0);
            let s = t * t * t * (t * (t * 6.0 - 15.0) + 10.0) * 100.0;
            l_lab + (s - l_lab) * c
        };

        let fy2 = (l_lab + 16.0) / 116.0;
        let fx2 = a_lab / 500.0 + fy2;
        let fz2 = fy2 - b_lab / 200.0;

        let x2 = fx2 * fx2 * fx2 * 0.95047;
        let y2 = fy2 * fy2 * fy2;
        let z2 = fz2 * fz2 * fz2 * 1.08883;

        lin = clamp01([
            3.240_454_2 * x2 - 1.537_138_5 * y2 - 0.498_531_4 * z2,
            -0.969_266_0 * x2 + 1.876_010_8 * y2 + 0.041_556_0 * z2,
            0.055_643_4 * x2 - 0.204_025_9 * y2 + 1.057_225_2 * z2,
        ]);
    }

    // Step 4: exposure with j = exposure + ai
    let j = adj.exposure as f64 + ai;
    if j != 0.0 {
        let k = 10.0_f64.powf(2.0 * j) - 1.0;
        let l = 0.75_f64;
        let mut m = 1.0 / (1.0 - l) - 1.0;
        let mut n = dot(lin, luma_w);
        let j_neg = j.min(0.0);
        m *= 1.0 + n * n * j_neg / (0.2 - j_neg);
        lin = [lin[0] + (n - lin[0]) * l, lin[1] + (n - lin[1]) * l, lin[2] + (n - lin[2]) * l];
        if j < 0.0 {
            let s = 1.0 + j / 16.0;
            lin = [lin[0] * s, lin[1] * s, lin[2] * s];
            n *= s;
        }
        lin = [lin[0] * (k + 1.0) / (lin[0] * k + 1.0), lin[1] * (k + 1.0) / (lin[1] * k + 1.0), lin[2] * (k + 1.0) / (lin[2] * k + 1.0)];
        n = n * (k + 1.0) / (n * k + 1.0);
        lin = clamp01([lin[0] + (lin[0] - n) * m, lin[1] + (lin[1] - n) * m, lin[2] + (lin[2] - n) * m]);
    }

    // Step 5: saturation p (vibrance)
    let p = (adj.vibrance as f64).clamp(-1.0, 1.0);
    if p != 0.0 {
        let n = dot(lin, luma_w);
        if p <= 0.0 {
            lin = clamp01([lin[0] + (lin[0] - n) * p, lin[1] + (lin[1] - n) * p, lin[2] + (lin[2] - n) * p]);
        } else {
            let min_c = lin[0].min(lin[1]).min(lin[2]);
            let max_c = lin[0].max(lin[1]).max(lin[2]);
            let q = n / (n - min_c + 1e-6);
            let r = (1.0 - n) / (max_c - n + 1e-6);
            let scale = (1.0 + (q.min(r) - 1.0) * p).min(2.0);
            lin = clamp01([n + (lin[0] - n) * scale, n + (lin[1] - n) * scale, n + (lin[2] - n) * scale]);
        }
    }

    // Step 6: output = result^(1 / 2.2)
    let out =
        [lin[0].max(0.0).powf(1.0 / 2.2).clamp(0.0, 1.0), lin[1].max(0.0).powf(1.0 / 2.2).clamp(0.0, 1.0), lin[2].max(0.0).powf(1.0 / 2.2).clamp(0.0, 1.0)];
    [(out[0] * 255.0 + 0.5) as u8, (out[1] * 255.0 + 0.5) as u8, (out[2] * 255.0 + 0.5) as u8, rgba[3]]
}

#[test]
fn exposure_plus_one_brightens_a_literal_pixel_equally_on_cpu_and_gpu() {
    let Some(mut g) = gpu() else { return };
    let vp = Viewport { offset: Vec2::ZERO, zoom: 1.0, dpr: 1.0 };
    let adj = ImageAdjust { exposure: 1.0, ..ImageAdjust::default() };
    let expected = spec_step_adjust_rgba([0x80, 0x80, 0x80, 0xFF], &adj);
    assert_eq!(expected, [251, 251, 251, 255]);
    let t = solid_image_scene([0x80, 0x80, 0x80, 0xFF], adj, Color::BLACK);
    let c = compare(&mut g, "exposure_plus_one", &t.doc, t.page, vp, (16, 16), 2);
    let cpu_px = px(&c.cpu, 8, 8);
    let gpu_px = px(&c.gpu, 8, 8);
    for ch in 0..3 {
        assert!(cpu_px[ch] > 0x80, "CPU channel {ch} was not brightened above 0x80: {cpu_px:?}");
        assert!(gpu_px[ch] > 0x80, "GPU channel {ch} was not brightened above 0x80: {gpu_px:?}");
        assert!(cpu_px[ch].abs_diff(expected[ch]) <= 2, "CPU differed from f64 reference {expected:?} by >2 on channel {ch}: {cpu_px:?}");
        assert!(gpu_px[ch].abs_diff(expected[ch]) <= 2, "GPU differed from f64 reference {expected:?} by >2 on channel {ch}: {gpu_px:?}");
        assert!(cpu_px[ch].abs_diff(gpu_px[ch]) <= 2, "CPU and GPU differed by more than 2 on channel {ch}: cpu={cpu_px:?} gpu={gpu_px:?}");
    }
    assert_eq!(cpu_px[3], 0xFF);
    assert_eq!(gpu_px[3], 0xFF);
}

#[test]
fn adjustment_unit_tests_per_step() {
    let Some(mut g) = gpu() else { return };
    let vp = Viewport { offset: Vec2::ZERO, zoom: 1.0, dpr: 1.0 };

    let mid_grey = [0x80, 0x80, 0x80, 0xFF];
    let warm = [200, 120, 60, 255];
    let cool = [60, 120, 200, 255];
    let dark_quarter = [64, 64, 64, 255];
    let bright_three_quarter = [191, 191, 191, 255];

    let steps: Vec<(&str, [u8; 4], ImageAdjust)> = vec![
        ("exposure_+0.5_grey", mid_grey, ImageAdjust { exposure: 0.5, ..Default::default() }),
        ("exposure_-0.5_grey", mid_grey, ImageAdjust { exposure: -0.5, ..Default::default() }),
        ("contrast_+0.5_grey", mid_grey, ImageAdjust { contrast: 0.5, ..Default::default() }),
        ("contrast_-0.5_grey", mid_grey, ImageAdjust { contrast: -0.5, ..Default::default() }),
        ("contrast_+0.5_warm", warm, ImageAdjust { contrast: 0.5, ..Default::default() }),
        ("contrast_-0.5_warm", warm, ImageAdjust { contrast: -0.5, ..Default::default() }),
        ("saturation_+0.5_grey", mid_grey, ImageAdjust { vibrance: 0.5, ..Default::default() }),
        ("saturation_-0.5_grey", mid_grey, ImageAdjust { vibrance: -0.5, ..Default::default() }),
        ("saturation_+0.5_warm", warm, ImageAdjust { vibrance: 0.5, ..Default::default() }),
        ("saturation_-0.5_warm", warm, ImageAdjust { vibrance: -0.5, ..Default::default() }),
        ("saturation_+0.5_cool", cool, ImageAdjust { vibrance: 0.5, ..Default::default() }),
        ("saturation_-0.5_cool", cool, ImageAdjust { vibrance: -0.5, ..Default::default() }),
        ("temperature_+0.5_grey", mid_grey, ImageAdjust { temperature: 0.5, ..Default::default() }),
        ("temperature_-0.5_grey", mid_grey, ImageAdjust { temperature: -0.5, ..Default::default() }),
        ("temperature_+0.5_warm", warm, ImageAdjust { temperature: 0.5, ..Default::default() }),
        ("temperature_-0.5_cool", cool, ImageAdjust { temperature: -0.5, ..Default::default() }),
        ("tint_+0.5_grey", mid_grey, ImageAdjust { tint: 0.5, ..Default::default() }),
        ("tint_-0.5_grey", mid_grey, ImageAdjust { tint: -0.5, ..Default::default() }),
        ("tint_+0.5_warm", warm, ImageAdjust { tint: 0.5, ..Default::default() }),
        ("tint_-0.5_cool", cool, ImageAdjust { tint: -0.5, ..Default::default() }),
        ("highlights_+0.5_grey", mid_grey, ImageAdjust { highlights: 0.5, ..Default::default() }),
        ("highlights_-0.5_grey", mid_grey, ImageAdjust { highlights: -0.5, ..Default::default() }),
        ("highlights_+0.5_0.75", bright_three_quarter, ImageAdjust { highlights: 0.5, ..Default::default() }),
        ("highlights_-0.5_0.75", bright_three_quarter, ImageAdjust { highlights: -0.5, ..Default::default() }),
        ("highlights_+0.5_0.25", dark_quarter, ImageAdjust { highlights: 0.5, ..Default::default() }),
        ("shadows_+0.5_grey", mid_grey, ImageAdjust { shadows: 0.5, ..Default::default() }),
        ("shadows_-0.5_grey", mid_grey, ImageAdjust { shadows: -0.5, ..Default::default() }),
        ("shadows_+0.5_0.25", dark_quarter, ImageAdjust { shadows: 0.5, ..Default::default() }),
        ("shadows_-0.5_0.25", dark_quarter, ImageAdjust { shadows: -0.5, ..Default::default() }),
        ("shadows_+0.5_0.75", bright_three_quarter, ImageAdjust { shadows: 0.5, ..Default::default() }),
    ];

    // Transparent and semi-transparent alpha preservation: alpha is untouched by adjustments.
    let semi_adj = ImageAdjust { exposure: 1.0, vibrance: 0.5, ..Default::default() };
    assert_eq!(designos_render::image_adjust::adjust_rgba([200, 100, 50, 0], &semi_adj), [0, 0, 0, 0]);
    let semi_unit = designos_render::image_adjust::adjust_rgba([128, 128, 128, 128], &semi_adj);
    assert_eq!(semi_unit[3], 128, "semi-transparent alpha must remain 128");
    let t_semi = solid_image_scene([128, 128, 128, 128], semi_adj, Color::rgba(0, 0, 0, 0));
    let c_semi = compare(&mut g, "semi_transparent_alpha", &t_semi.doc, t_semi.page, vp, (16, 16), 2);
    let cpu_spx = px(&c_semi.cpu, 8, 8);
    let gpu_spx = px(&c_semi.gpu, 8, 8);
    assert_eq!(cpu_spx[3], 128, "CPU rendered semi-transparent alpha must stay 128, got {cpu_spx:?}");
    assert_eq!(gpu_spx[3], 128, "GPU rendered semi-transparent alpha must stay 128, got {gpu_spx:?}");

    for (label, base_rgba, adj) in &steps {
        let expected = spec_step_adjust_rgba(*base_rgba, adj);
        let unit_out = designos_render::image_adjust::adjust_rgba(*base_rgba, adj);
        for ch in 0..4 {
            assert!(unit_out[ch].abs_diff(expected[ch]) <= 2, "{label}: adjust_rgba {unit_out:?} != expected {expected:?} on channel {ch}");
        }

        let t = solid_image_scene(*base_rgba, *adj, Color::BLACK);
        let c = compare(&mut g, label, &t.doc, t.page, vp, (16, 16), 2);
        let cpu_px = px(&c.cpu, 8, 8);
        let gpu_px = px(&c.gpu, 8, 8);
        for ch in 0..4 {
            assert!(cpu_px[ch].abs_diff(expected[ch]) <= 2, "{label}: CPU pixel {cpu_px:?} != expected {expected:?} on channel {ch}");
            assert!(gpu_px[ch].abs_diff(expected[ch]) <= 2, "{label}: GPU pixel {gpu_px:?} != expected {expected:?} on channel {ch}");
        }
    }
}

#[test]
fn fill_to_crop_switch_reproduces_the_framing() {
    let Some(mut g) = gpu() else { return };
    let vp = Viewport { offset: Vec2::ZERO, zoom: 1.0, dpr: 1.0 };

    for (rot, check_exact_m) in [(0.0_f32, true), (90.0_f32, false)] {
        let mut t = T::new();
        t.node(t.page).kind = NodeKind::Page { background: Color::BLACK };
        t.doc.images.insert("checker".into(), png_bytes(100, 100, |x, y| if (x < 50) ^ (y < 50) { [220, 40, 40, 255] } else { [40, 120, 220, 255] }));
        let r = t.rect(None, Rect::new(0.0, 0.0, 200.0, 100.0), Color::BLACK);
        let fill_paint = Paint {
            kind: PaintKind::Image {
                image_ref: "checker".into(),
                scale_mode: ImageScaleMode::Fill,
                params: ImageParams { rotation: rot, ..ImageParams::default() },
            },
            opacity: 1.0,
            visible: true,
            blend: BlendMode::Normal,
        };
        t.node(r).fills = vec![fill_paint];

        let fill_cmp = compare(&mut g, "fill_before_crop", &t.doc, t.page, vp, (200, 100), 8);

        // Switch Fill -> Crop through the panel helper.
        {
            let paint = &mut t.node(r).fills[0];
            designos_render::image_adjust::switch_image_scale_mode(paint, ImageScaleMode::Crop, Vec2::new(200.0, 100.0), (100, 100));
            let PaintKind::Image { scale_mode, params, .. } = &paint.kind else { panic!("expected image paint") };
            assert_eq!(*scale_mode, ImageScaleMode::Crop);
            let m = params.transform.expect("Fill -> Crop must write params.transform");
            if check_exact_m {
                let expected = [1.0_f32, 0.0, 0.0, 0.0, 0.5, 0.25];
                for i in 0..6 {
                    assert!((m[i] - expected[i]).abs() < 1e-6, "expected transform {expected:?}, got {m:?}");
                }
            }
        }

        let crop_cmp = compare(&mut g, "crop_after_fill", &t.doc, t.page, vp, (200, 100), 8);
        assert_eq!(crop_cmp.cpu.data(), fill_cmp.cpu.data(), "CPU Crop framing at rot={rot} must match Fill before switch");
        assert_eq!(crop_cmp.gpu.data(), fill_cmp.gpu.data(), "GPU Crop framing at rot={rot} must match Fill before switch");

        // Customize the stored crop transform, switch Crop -> Fill (must keep transform stored and render Fill),
        // then switch Fill -> Crop (must restore the stored crop transform).
        let custom_crop = [0.5_f32, 0.0, 0.1, 0.0, 0.5, 0.2];
        if let PaintKind::Image { params, .. } = &mut t.node(r).fills[0].kind {
            params.transform = Some(custom_crop);
        }
        {
            let paint = &mut t.node(r).fills[0];
            designos_render::image_adjust::switch_image_scale_mode(paint, ImageScaleMode::Fill, Vec2::new(200.0, 100.0), (100, 100));
            let PaintKind::Image { scale_mode, params, .. } = &paint.kind else { panic!("expected image paint") };
            assert_eq!(*scale_mode, ImageScaleMode::Fill);
            assert_eq!(params.transform, Some(custom_crop), "switching Crop -> Fill must keep transform stored");
        }
        let back_fill_cmp = compare(&mut g, "back_to_fill", &t.doc, t.page, vp, (200, 100), 8);
        assert_eq!(back_fill_cmp.cpu.data(), fill_cmp.cpu.data(), "Fill with stored crop transform must still render Fill framing");

        {
            let paint = &mut t.node(r).fills[0];
            designos_render::image_adjust::switch_image_scale_mode(paint, ImageScaleMode::Crop, Vec2::new(200.0, 100.0), (100, 100));
            let PaintKind::Image { scale_mode, params, .. } = &paint.kind else { panic!("expected image paint") };
            assert_eq!(*scale_mode, ImageScaleMode::Crop);
            assert_eq!(params.transform, Some(custom_crop), "switching back to Crop must restore the stored crop transform");
        }
    }
}

#[test]
fn all_zero_adjustments_are_byte_identical_to_the_previous_render() {
    let Some(mut g) = gpu() else { return };
    let vp = Viewport { offset: Vec2::ZERO, zoom: 1.0, dpr: 1.0 };
    assert!(!designos_render::image_adjust::has_rendered_adjustments(&ImageAdjust::default()));

    let mut t = T::new();
    t.node(t.page).kind = NodeKind::Page { background: Color::BLACK };
    t.doc.images.insert("checker".into(), png_bytes(100, 100, |x, y| if (x < 50) ^ (y < 50) { [220, 40, 40, 255] } else { [40, 120, 220, 255] }));
    let r = t.rect(None, Rect::new(0.0, 0.0, 100.0, 100.0), Color::BLACK);
    t.node(r).fills = vec![Paint {
        kind: PaintKind::Image {
            image_ref: "checker".into(),
            scale_mode: ImageScaleMode::Fill,
            params: ImageParams { adjust: ImageAdjust::default(), ..ImageParams::default() },
        },
        opacity: 1.0,
        visible: true,
        blend: BlendMode::Normal,
    }];

    let cmp_a = compare(&mut g, "zero_adjust_a", &t.doc, t.page, vp, (100, 100), 2);

    // Non-rendered fields (detail, vignette, brightness) when the 7 rendered fields are 0.0 must also skip the pass.
    if let PaintKind::Image { params, .. } = &mut t.node(r).fills[0].kind {
        params.adjust.detail = 0.5;
        params.adjust.vignette = 0.5;
        params.adjust.brightness = 0.5;
    }
    let cmp_b = compare(&mut g, "zero_adjust_b", &t.doc, t.page, vp, (100, 100), 2);

    assert_eq!(cmp_a.cpu.data(), cmp_b.cpu.data(), "CPU render with zero rendered adjustments must be byte-identical");
    assert_eq!(cmp_a.gpu.data(), cmp_b.gpu.data(), "GPU render with zero rendered adjustments must be byte-identical");
    assert_eq!(px(&cmp_a.cpu, 25, 25), [40, 120, 220, 255]);
    assert_eq!(px(&cmp_a.gpu, 25, 25), [40, 120, 220, 255]);
}

#[test]
fn kit_nodes_with_adjustments_render_on_cpu_and_gpu() {
    let Some(mut g) = gpu() else { return };
    let tag_opt = std::env::var("DESIGNOS_KIT_EVIDENCE_TAG").ok();
    let tag = tag_opt.as_deref().unwrap_or("test");
    let prefix = std::env::var("DESIGNOS_KIT_EVIDENCE_PREFIX").unwrap_or_default();
    let ev_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plans/reports/agy-163b-image-adjustments-panel-evidence");
    if tag_opt.is_some() {
        std::fs::create_dir_all(&ev_dir).unwrap();
        g.compiler.geom.clear();
        let exp_t = solid_image_scene([0x80, 0x80, 0x80, 0xFF], ImageAdjust { exposure: 1.0, ..Default::default() }, Color::BLACK);
        let vp = Viewport { offset: Vec2::ZERO, zoom: 1.0, dpr: 1.0 };
        let exp_cmp = compare(&mut g, &format!("solid_808080_exp1_{tag}"), &exp_t.doc, exp_t.page, vp, (16, 16), 2);
        exp_cmp.cpu.save_png(ev_dir.join(format!("{prefix}solid-808080-exp1-{tag}-cpu.png"))).unwrap();
        exp_cmp.gpu.save_png(ev_dir.join(format!("{prefix}solid-808080-exp1-{tag}-gpu.png"))).unwrap();
        let base_t = solid_image_scene([0x80, 0x80, 0x80, 0xFF], ImageAdjust::default(), Color::BLACK);
        let base_cmp = compare(&mut g, "solid_808080_exp1_base", &base_t.doc, base_t.page, vp, (16, 16), 2);
        base_cmp.cpu.save_png(ev_dir.join(format!("{prefix}solid-808080-exp1-base-cpu.png"))).unwrap();
        base_cmp.gpu.save_png(ev_dir.join(format!("{prefix}solid-808080-exp1-base-gpu.png"))).unwrap();
    }
    let fig_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../out/kit-clone.fig");
    if !fig_path.exists() {
        return;
    }
    let bytes = std::fs::read(&fig_path).expect("read out/kit-clone.fig");
    let doc = designos_fig::import_fig_with_budget(&bytes, 0).expect("import out/kit-clone.fig").0;

    let mut found: Vec<(NodeId, String, Vec2, Paint)> = Vec::new();
    for (&id, node) in &doc.nodes {
        for fill in &node.fills {
            if let PaintKind::Image { params, .. } = &fill.kind {
                let a = &params.adjust;
                if a.exposure != 0.0
                    || a.contrast != 0.0
                    || a.vibrance != 0.0
                    || a.temperature != 0.0
                    || a.tint != 0.0
                    || a.highlights != 0.0
                    || a.shadows != 0.0
                {
                    found.push((id, node.name.clone(), node.size, fill.clone()));
                }
            }
        }
    }
    found.sort_by_key(|(id, _, _, _)| id.0);
    eprintln!("Found {} kit nodes with non-zero image adjustments:", found.len());
    for (idx, (id, name, size, fill)) in found.iter().enumerate() {
        let PaintKind::Image { image_ref, scale_mode, params } = &fill.kind else { continue };
        let has_img = doc.images.contains_key(image_ref);
        eprintln!(
            "  [{idx}] id={:?} name={:?} size={:?} scale_mode={:?} image_ref={} has_img={} adjust={:?}",
            id, name, size, scale_mode, image_ref, has_img, params.adjust
        );
        if !has_img {
            continue;
        }
        let w = (size.x.round() as u32).clamp(16, 400);
        let h = (size.y.round() as u32).clamp(16, 240);
        g.compiler.geom.clear();
        let mut t = T::new();
        t.node(t.page).kind = NodeKind::Page { background: Color::BLACK };
        t.doc.images.insert(image_ref.clone(), doc.images.get(image_ref).unwrap().clone());
        let r = t.rect(None, Rect::new(0.0, 0.0, w as f32, h as f32), Color::BLACK);
        t.node(r).fills = vec![fill.clone()];
        let vp = Viewport { offset: Vec2::ZERO, zoom: 1.0, dpr: 1.0 };
        let cmp = compare(&mut g, &format!("kit_{idx}_{tag}"), &t.doc, t.page, vp, (w, h), 4);
        assert!(cmp.report_aa.ratio < 0.10, "kit node {idx} ({name}) CPU vs GPU mismatch too high: {:?}", cmp.report_aa);
        if tag_opt.is_some() {
            cmp.cpu.save_png(ev_dir.join(format!("{prefix}kit-node-{idx}-{tag}-cpu.png"))).unwrap();
            cmp.gpu.save_png(ev_dir.join(format!("{prefix}kit-node-{idx}-{tag}-gpu.png"))).unwrap();
        }
        if tag != "base" {
            let mut unadj_fill = fill.clone();
            if let PaintKind::Image { params, .. } = &mut unadj_fill.kind {
                params.adjust = ImageAdjust::default();
            }
            t.node(r).fills = vec![unadj_fill];
            let unadj_cmp = compare(&mut g, &format!("kit_{idx}_unadj"), &t.doc, t.page, vp, (w, h), 4);
            assert_ne!(cmp.cpu.data(), unadj_cmp.cpu.data(), "kit node {idx} ({name}) CPU render must differ from unadjusted");
            assert_ne!(cmp.gpu.data(), unadj_cmp.gpu.data(), "kit node {idx} ({name}) GPU render must differ from unadjusted");
        }
    }
}

#[test]
fn image_adjustments_interactive_frame_budget_holds() {
    let mut t = T::new();
    t.doc.images.insert("checker".into(), demo::checker());
    let mut ids = Vec::with_capacity(64);
    for idx in 0..64u32 {
        let col = (idx % 8) as f32;
        let row = (idx / 8) as f32;
        let r = t.rect(None, Rect::new(col * 80.0, row * 80.0, 64.0, 64.0), Color::WHITE);
        t.node(r).fills = vec![Paint {
            kind: PaintKind::Image { image_ref: "checker".into(), scale_mode: ImageScaleMode::Fill, params: ImageParams::default() },
            opacity: 1.0,
            visible: true,
            blend: BlendMode::Normal,
        }];
        ids.push(r);
    }

    let mut compiler = designos_gpu::SceneCompiler::new();
    compiler.geom.budget = Some(std::time::Duration::from_millis(8));
    let vp = Viewport { offset: Vec2::ZERO, zoom: 1.0, dpr: 2.0 };
    let size = (1280, 800);
    let mut rev = 1u64;

    // Warm-up compile
    let _ = compiler.compile(&t.doc, t.page, rev, &vp, size);

    // 1. 20 interactive frames with all-zero adjustments (pan gesture)
    let mut base_times = Vec::with_capacity(20);
    let mut base_progs = 0usize;
    let mut base_draws = 0usize;
    for step in 0..20 {
        let vp_step = Viewport { offset: Vec2::new(step as f32 * 4.0, step as f32 * 2.0), zoom: 1.0, dpr: 2.0 };
        let t0 = std::time::Instant::now();
        let frame = compiler.compile(&t.doc, t.page, rev, &vp_step, size);
        base_times.push(t0.elapsed().as_secs_f64() * 1000.0);
        let st = frame.stats();
        base_progs = st.programs;
        base_draws = st.draws;
    }

    // 2. 20 interactive frames with live slider adjustment mutations (all 7 sliders active)
    let mut adj_times = Vec::with_capacity(20);
    let mut adj_progs = 0usize;
    let mut adj_draws = 0usize;
    for step in 0..20 {
        rev += 1;
        let v = ((step as f32) - 10.0) / 10.0;
        for &id in &ids {
            if let Some(n) = t.doc.get_mut(id) {
                if let Some(Paint { kind: PaintKind::Image { params, .. }, .. }) = n.fills.first_mut() {
                    params.adjust = ImageAdjust {
                        exposure: v * 0.5,
                        contrast: v * 0.3,
                        vibrance: v * 0.6,
                        temperature: -v * 0.4,
                        tint: v * 0.35,
                        highlights: v * 0.45,
                        shadows: -v * 0.45,
                        ..Default::default()
                    };
                }
            }
        }
        let t0 = std::time::Instant::now();
        let frame = compiler.compile(&t.doc, t.page, rev, &vp, size);
        adj_times.push(t0.elapsed().as_secs_f64() * 1000.0);
        let st = frame.stats();
        adj_progs = st.programs;
        adj_draws = st.draws;
    }

    // 3. Idle steady-state check (same rev + viewport -> zero stale geometry hits)
    let _ = compiler.compile(&t.doc, t.page, rev, &vp, size);
    assert_eq!(compiler.geom.stale_hits, 0, "idle frame must have 0 stale geometry hits (0.00% idle CPU invariant)");

    base_times.sort_by(|a, b| a.partial_cmp(b).unwrap());
    adj_times.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let base_p50 = base_times[10];
    let base_p95 = base_times[18];
    let adj_p50 = adj_times[10];
    let adj_p95 = adj_times[18];
    eprintln!(
        "IMAGE_ADJUST_PERF: base_pan(20f) p50={:.3}ms p95={:.3}ms max={:.3}ms draws={} progs={} | live_adjust_drag(20f) p50={:.3}ms p95={:.3}ms max={:.3}ms draws={} progs={} | idle_stale_hits={}",
        base_p50, base_p95, base_times[19], base_draws, base_progs,
        adj_p50, adj_p95, adj_times[19], adj_draws, adj_progs,
        compiler.geom.stale_hits
    );
    let budget_ms = if cfg!(debug_assertions) { 40.0 } else { 8.0 };
    assert!(base_p95 < budget_ms, "base p95 {:.3}ms exceeded {:.1}ms budget", base_p95, budget_ms);
    assert!(adj_p95 < budget_ms, "live adjust p95 {:.3}ms exceeded {:.1}ms budget", adj_p95, budget_ms);
    assert!(adj_progs < 64, "offscreen programs {} must remain < 64", adj_progs);
}

/// A 64x64 picture of one-pixel-wide vertical stripes in `a` and `b`, drawn on a 63x63 node: a little under one device
/// pixel per texel, so both renderers filter bilinearly and every pixel of a row blends the two stripes at another weight.
fn stripes_scene(a: [u8; 4], b: [u8; 4], adjust: ImageAdjust) -> T {
    let mut t = T::new();
    t.node(t.page).kind = NodeKind::Page { background: Color::BLACK };
    // the GPU keeps decoded pictures by name, so every colour pair gets its own
    let key = format!("stripes_{a:02x?}_{b:02x?}");
    t.doc.images.insert(key.clone(), png_bytes(64, 64, |x, _| if x % 2 == 0 { a } else { b }));
    let r = t.rect(None, Rect::new(0.0, 0.0, 63.0, 63.0), Color::BLACK);
    t.node(r).fills = vec![Paint {
        kind: PaintKind::Image { image_ref: key, scale_mode: ImageScaleMode::Fill, params: ImageParams { adjust, ..Default::default() } },
        opacity: 1.0,
        visible: true,
        blend: BlendMode::Normal,
    }];
    t
}

#[test]
fn adjustments_act_on_the_blended_colour_at_hard_edges_equally_on_cpu_and_gpu() {
    let Some(mut g) = gpu() else { return };
    let vp = Viewport { offset: Vec2::ZERO, zoom: 1.0, dpr: 1.0 };
    let cases: [(&str, [u8; 4], [u8; 4], ImageAdjust); 5] = [
        ("exposure_plus_1", [40, 70, 110, 255], [230, 200, 110, 255], ImageAdjust { exposure: 1.0, ..Default::default() }),
        ("saturation_plus_1", [128, 64, 64, 255], [64, 64, 128, 255], ImageAdjust { vibrance: 1.0, ..Default::default() }),
        ("contrast_temperature", [150, 80, 70, 255], [70, 80, 150, 255], ImageAdjust { contrast: 0.3, temperature: 0.6, ..Default::default() }),
        ("tint", [160, 90, 40, 255], [40, 90, 160, 255], ImageAdjust { tint: 0.8, ..Default::default() }),
        ("highlights_shadows", [56, 56, 56, 255], [168, 168, 168, 255], ImageAdjust { highlights: -0.8, shadows: 0.6, ..Default::default() }),
    ];
    for (label, a, b, adjust) in cases {
        let t = stripes_scene(a, b, adjust);
        g.compiler.geom.clear();
        let c = compare(&mut g, &format!("stripes_{label}"), &t.doc, t.page, vp, (63, 63), 2);
        let mut worst = 0u8;
        let mut at = (0, 0);
        for y in 0..63 {
            for x in 0..63 {
                let (p, q) = (px(&c.cpu, x, y), px(&c.gpu, x, y));
                let d = (0..3).map(|k| p[k].abs_diff(q[k])).max().unwrap();
                if d > worst {
                    worst = d;
                    at = (x, y);
                }
            }
        }
        assert!(worst <= 2, "{label}: CPU and GPU differ by {worst} levels at {at:?} (cpu {:?}, gpu {:?})", px(&c.cpu, at.0, at.1), px(&c.gpu, at.0, at.1));
        let plain = stripes_scene(a, b, ImageAdjust::default());
        g.compiler.geom.clear();
        let base = compare(&mut g, &format!("stripes_{label}_plain"), &plain.doc, plain.page, vp, (63, 63), 2);
        assert_ne!(c.cpu.data(), base.cpu.data(), "{label}: the CPU render must differ from the unadjusted one");
    }
}

/// The literal black|white 2x1 picture stretched over a wide node: the CPU magnifies with a bicubic filter and the GPU
/// with a bilinear one, so the two only agree on what is adjusted: each backend adjusts its own sampled colour.
#[test]
fn each_backend_adjusts_the_colour_it_sampled_from_a_stretched_hard_edge() {
    let Some(mut g) = gpu() else { return };
    let vp = Viewport { offset: Vec2::ZERO, zoom: 1.0, dpr: 1.0 };
    let adjust = ImageAdjust { exposure: 1.0, ..Default::default() };
    let build = |adjust: ImageAdjust| {
        let mut t = T::new();
        t.node(t.page).kind = NodeKind::Page { background: Color::BLACK };
        t.doc.images.insert("edge".into(), png_bytes(2, 1, |x, _| if x == 0 { [0, 0, 0, 255] } else { [255, 255, 255, 255] }));
        let r = t.rect(None, Rect::new(0.0, 0.0, 64.0, 4.0), Color::BLACK);
        t.node(r).fills = vec![Paint {
            kind: PaintKind::Image { image_ref: "edge".into(), scale_mode: ImageScaleMode::Fill, params: ImageParams { adjust, ..Default::default() } },
            opacity: 1.0,
            visible: true,
            blend: BlendMode::Normal,
        }];
        t
    };
    let plain = build(ImageAdjust::default());
    let adj = build(adjust);
    g.compiler.geom.clear();
    let p = compare(&mut g, "edge_stretched_plain", &plain.doc, plain.page, vp, (64, 4), 2);
    g.compiler.geom.clear();
    let a = compare(&mut g, "edge_stretched_exposure", &adj.doc, adj.page, vp, (64, 4), 2);
    for x in 0..64 {
        for (name, plain_px, adj_px, tol) in [("CPU", px(&p.cpu, x, 2), px(&a.cpu, x, 2), 1u8), ("GPU", px(&p.gpu, x, 2), px(&a.gpu, x, 2), 2u8)] {
            let want = designos_render::image_adjust::adjust_rgba(plain_px, &adjust);
            for k in 0..3 {
                assert!(
                    want[k].abs_diff(adj_px[k]) <= tol,
                    "{name} pixel {x}: adjusted {adj_px:?} is not the adjustment of the sampled {plain_px:?} (want {want:?})"
                );
            }
        }
    }
}

/// The 64x64 stripe picture of `stripes_scene` in a 63x94 node, fitted (63x63 centred at y 15.5..78.5) or cropped with the
/// same framing turned a quarter, on a transparent page.
fn letterboxed_stripes(mode: ImageScaleMode, adjust: ImageAdjust) -> T {
    let (a, b) = ([128, 64, 64, 255], [64, 64, 128, 255]);
    let mut t = T::new();
    t.node(t.page).kind = NodeKind::Page { background: Color::TRANSPARENT };
    let key = format!("letterboxed_stripes_{a:02x?}_{b:02x?}");
    t.doc.images.insert(key.clone(), png_bytes(64, 64, |x, _| if x % 2 == 0 { a } else { b }));
    let size = Vec2::new(63.0, 94.0);
    let r = t.rect(None, Rect::new(0.0, 0.0, size.x, size.y), Color::TRANSPARENT);
    let transform = (mode == ImageScaleMode::Crop).then(|| {
        let framing = designos_render::image_adjust::crop_transform_from_framing(size, 64.0, 64.0, ImageScaleMode::Fit, &ImageParams::default()).unwrap();
        designos_render::image_adjust::rotate_crop_transform(framing, size, true).unwrap()
    });
    t.node(r).fills = vec![Paint {
        kind: PaintKind::Image { image_ref: key, scale_mode: mode, params: ImageParams { adjust, transform, ..Default::default() } },
        opacity: 1.0,
        visible: true,
        blend: BlendMode::Normal,
    }];
    t
}

#[test]
fn an_adjusted_fit_letterbox_and_turned_crop_stay_transparent_and_match_between_cpu_and_gpu() {
    let Some(mut g) = gpu() else { return };
    let vp = Viewport { offset: Vec2::ZERO, zoom: 1.0, dpr: 1.0 };
    let adjust = ImageAdjust { vibrance: 1.0, ..Default::default() };
    for (label, mode) in [("fit", ImageScaleMode::Fit), ("turned_crop", ImageScaleMode::Crop)] {
        let (plain, adjusted) = (letterboxed_stripes(mode, ImageAdjust::default()), letterboxed_stripes(mode, adjust));
        g.compiler.geom.clear();
        let p = compare(&mut g, &format!("letterbox_{label}_plain"), &plain.doc, plain.page, vp, (63, 94), 2);
        g.compiler.geom.clear();
        let a = compare(&mut g, &format!("letterbox_{label}_adjusted"), &adjusted.doc, adjusted.page, vp, (63, 94), 2);
        for y in (0..13).chain(81..94) {
            for x in 0..63 {
                assert_eq!(px(&a.cpu, x, y), px(&p.cpu, x, y), "{label}: CPU ({x}, {y}) outside the picture equals the unadjusted render");
                assert_eq!(px(&a.cpu, x, y)[3], 0, "{label}: CPU ({x}, {y}) outside the picture is transparent");
                assert_eq!(px(&a.gpu, x, y)[3], 0, "{label}: GPU ({x}, {y}) outside the picture is transparent");
            }
        }
        let mut worst = 0u8;
        for y in 18..76 {
            for x in 0..63 {
                let (c, q) = (px(&a.cpu, x, y), px(&a.gpu, x, y));
                worst = worst.max((0..4).map(|k| c[k].abs_diff(q[k])).max().unwrap());
            }
        }
        assert!(worst <= 2, "{label}: CPU and GPU differ by {worst} levels inside the picture");
        assert_ne!(a.cpu.data(), p.cpu.data(), "{label}: the adjustment changes the CPU render");
    }
}
