//! GPU renderer vs the CPU reference. Tests print a note and return when the
//! machine has no GPU adapter.

mod common;

use common::*;
use designos_doc::*;
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
    t.node(glass).effects =
        vec![Effect::Glass(GlassEffect { light_angle: 45.0, light_intensity: 0.7, refraction: 0.5, depth: 10.0, dispersion: 0.3, frost: 4.0, visible: true })];
    // noise (shape fast path) and noise on a group (general path)
    let n = t.rect(None, Rect::new(180.0, 10.0, 80.0, 60.0), Color::rgb(0x44, 0x88, 0xDD));
    t.node(n).effects = vec![Effect::Noise(NoiseEffect {
        mode: NoiseMode::Duo { color: Color::rgb(0, 0, 0), secondary: Color::WHITE },
        size: 1.5,
        density: 0.5,
        opacity: 0.4,
        blend: BlendMode::Normal,
        seed: 7,
        visible: true,
    })];
    let tx = t.rect(None, Rect::new(180.0, 90.0, 80.0, 50.0), Color::rgb(0xE0, 0x50, 0x30));
    t.node(tx).effects = vec![Effect::Texture(TextureEffect { size: 6.0, radius: 4.0, clip_to_shape: false, seed: 3, visible: true })];
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
        kind: PaintKind::Image { image_ref: "checker".into(), scale_mode: ImageScaleMode::Fill },
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
        Effect::DropShadow { color: Color::BLACK, offset: Vec2::new(30.0, 30.0), radius: 300.0, spread: 20.0, visible: true, show_behind: true },
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
