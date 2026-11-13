//! Noise and clipped texture on a group that paints nothing itself are masked by the group's
//! children at their own alpha, on the GPU as on the CPU; only a background blur widens the mask
//! to the children's full footprint. Both renderers take the mask from
//! `designos_doc::silhouette_source`, so a GPU that agrees with the CPU render proves nothing by
//! itself: each scene also pins the CPU pixels (the same digests as `designos-render`'s
//! `group_effect_masks` test). Glass is left out: on this scene the GPU glass differs from the
//! CPU glass in 1,854 of 8,000 pixels at tolerance 4, far over the budget below, so its mask is
//! covered by the unit tests of `silhouette_source`. Pixel tests need a GPU adapter and skip
//! without one.

mod common;

use designos_doc::*;
use designos_render::Viewport;

const BLACK: Color = Color { r: 0.0, g: 0.0, b: 0.0, a: 1.0 };
const SIZE: (u32, u32) = (100, 80);

fn fnv1a(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Six black stripes on the white page, then a 60x60 group at (10, 10) carrying `fx` over a
/// 60x60 ellipse filled with 40% white.
fn group_over_translucent_child(fx: Effect) -> common::T {
    let mut t = common::T::new();
    for i in 0..6 {
        t.rect(None, Rect::new(5.0 + 15.0 * i as f32, 0.0, 6.0, 80.0), BLACK);
    }
    let g = t.add(None, |id| {
        let mut n = Node::new(id, NodeKind::Group);
        n.size = Vec2::new(60.0, 60.0);
        n.transform = Affine::translate(10.0, 10.0);
        n.effects = vec![fx];
        n
    });
    t.add(Some(g), |id| {
        let mut n = Node::ellipse(id, Rect::new(0.0, 0.0, 60.0, 60.0));
        n.fills = vec![Paint { opacity: 0.4, ..Paint::solid(Color::WHITE) }];
        n
    });
    t
}

fn group_over_translucent_child_at_edge(fx: Effect) -> common::T {
    let mut t = common::T::new();
    for i in 0..6 {
        t.rect(None, Rect::new(15.0 * i as f32, 0.0, 6.0, 80.0), BLACK);
    }
    let g = t.add(None, |id| {
        let mut n = Node::new(id, NodeKind::Group);
        n.size = Vec2::new(60.0, 60.0);
        n.effects = vec![fx];
        n
    });
    t.add(Some(g), |id| {
        let mut n = Node::ellipse(id, Rect::new(0.0, 0.0, 60.0, 60.0));
        n.fills = vec![Paint { opacity: 0.4, ..Paint::solid(Color::WHITE) }];
        n
    });
    t
}

/// The CPU pixels equal `cpu_digest`, and the GPU draws the same scene within the budget of the
/// other GPU parity tests. The group's ellipse covers 2,827 of the 8,000 pixels, far over both
/// budgets, so a mask that differs between the renderers cannot hide as an edge effect.
fn parity(name: &str, fx: Effect, cpu_digest: u64) {
    let Some(mut g) = common::gpu() else { return };
    let t = group_over_translucent_child(fx);
    let c = common::compare(&mut g, name, &t.doc, t.page, Viewport::default(), SIZE, 4);
    assert_eq!(fnv1a(c.cpu.data()), cpu_digest, "{name}: pixels of the CPU scene changed");
    assert!(c.report_aa.ratio < 0.005, "{name}: mismatch with AA forgiven {:.3}%", c.report_aa.ratio * 100.0);
    assert!(c.report.ratio < 0.02, "{name}: strict mismatch {:.3}%", c.report.ratio * 100.0);
}

#[test]
fn noise_on_a_group_stays_masked_by_the_children_alpha() {
    let fx = Effect::Noise(NoiseEffect {
        mode: NoiseMode::Mono { color: BLACK },
        size: 1.0,
        size_y: None,
        density: 0.6,
        opacity: 1.0,
        blend: BlendMode::Normal,
        seed: 7,
        visible: true,
    });
    parity("group-noise-translucent-child", fx, 0x1c360c5c2e5fff46);
}

#[test]
fn clipped_texture_on_a_group_stays_masked_by_the_children_alpha() {
    let fx = Effect::Texture(TextureEffect { size: 4.0, size_y: None, radius: 3.0, clip_to_shape: true, seed: 3, visible: true });
    parity("group-texture-translucent-child", fx, 0x43cc2e696be2ddf5);
}

#[test]
fn background_blur_on_a_group_uses_the_child_footprint_mask() {
    let fx = Effect::BackgroundBlur { radius: 8.0, visible: true };
    // The digest is deliberately pinned alongside the GPU comparison: changing
    // the mask back to the opaque silhouette changes the CPU scene as well.
    parity("group-background-blur-translucent-child", fx, 0xc6117e40479ae03e);
}

#[test]
fn background_blur_descendant_uses_resolved_radius_in_gpu_compile() {
    for masked in [false, true] {
        let Some(mut g) = common::gpu() else { return };
        let Some(mut expected_gpu) = common::gpu() else { return };
        let mut t = common::T::new();
        t.rect(None, Rect::new(10.0, 0.0, 10.0, 40.0), BLACK);
        let group = t.add(None, |id| {
            let mut n = Node::new(id, NodeKind::Group);
            n.size = Vec2::new(40.0, 40.0);
            n.opacity = if masked { 1.0 } else { 0.9 };
            n
        });
        if masked {
            t.add(Some(group), |id| {
                let mut n = Node::rectangle(id, Rect::new(0.0, 0.0, 40.0, 40.0));
                n.fills = vec![Paint::solid(Color::WHITE)];
                n.is_mask = true;
                n
            });
        }
        let pane = t.add(Some(group), |id| {
            let mut n = Node::rectangle(id, Rect::new(0.0, 0.0, 40.0, 40.0));
            n.fills = vec![Paint::solid(Color::TRANSPARENT)];
            n.effects = vec![Effect::BackgroundBlur { radius: 0.0, visible: true }];
            n.visible = false;
            n.variables.bindings.insert("effects/0/radius".into(), "blur-radius".into());
            n.variables.bindings.insert("visible".into(), "blur-visible".into());
            n
        });
        t.doc.variable_collections.insert(
            "theme".into(),
            VariableCollection {
                id: "theme".into(),
                name: "Theme".into(),
                modes: vec![VariableMode { id: "light".into(), name: "Light".into() }, VariableMode { id: "dark".into(), name: "Dark".into() }],
                default_mode: "light".into(),
                variables: vec!["blur-radius".into(), "blur-visible".into()],
                key: None,
                source_library_key: None,
                extensions: Default::default(),
            },
        );
        t.doc.variables.insert(
            "blur-visible".into(),
            Variable {
                id: "blur-visible".into(),
                name: "blur-visible".into(),
                collection: "theme".into(),
                kind: VariableKind::Boolean,
                values: [("light".into(), VariableValue::Boolean(false)), ("dark".into(), VariableValue::Boolean(true))].into_iter().collect(),
                description: String::new(),
                scopes: vec![],
                hidden_from_publishing: false,
                key: None,
                extensions: Default::default(),
                code_syntax: Default::default(),
                mode_extensions: Default::default(),
            },
        );
        t.doc.variables.insert(
            "blur-radius".into(),
            Variable {
                id: "blur-radius".into(),
                name: "blur-radius".into(),
                collection: "theme".into(),
                kind: VariableKind::Number,
                values: [("light".into(), VariableValue::Number(0.0)), ("dark".into(), VariableValue::Number(8.0))].into_iter().collect(),
                description: String::new(),
                scopes: vec![],
                hidden_from_publishing: false,
                key: None,
                extensions: Default::default(),
                code_syntax: Default::default(),
                mode_extensions: Default::default(),
            },
        );
        t.node(group).variables.modes.insert("theme".into(), "dark".into());
        t.node(group).variables.modes.insert("theme".into(), "light".into());
        let light_frame = g.compile(&t.doc, t.page, 1, &Viewport::default(), (60, 40));
        let light = g.render_frame(&light_frame, (60, 40));
        assert_eq!(light.pixel(15, 20).unwrap().red(), 0, "light mode keeps the hidden pane sharp");
        t.node(group).variables.modes.insert("theme".into(), "dark".into());
        let resolved = designos_doc::apply_bound_variables(&t.doc);
        let mut cpu_expected = designos_render::tiny_skia::Pixmap::new(60, 40).unwrap();
        designos_render::render_page(&resolved, resolved.pages[0], &Viewport::default(), &mut cpu_expected);
        let frame = g.compile(&t.doc, t.page, 2, &Viewport::default(), (60, 40));
        let actual = g.render_frame(&frame, (60, 40));
        // Use a fresh compiler for the fully resolved document. HeadlessGpu caches its
        // SceneIndex by revision; both documents use rev=0 in the convenience API.
        let expected = expected_gpu.render_page(&resolved, resolved.pages[0], &Viewport::default(), (60, 40));
        if let Ok(dir) = std::env::var("DESIGNOS_BLUR_EVIDENCE_DIR") {
            std::fs::create_dir_all(&dir).unwrap();
            actual.save_png(std::path::Path::new(&dir).join(format!("gpu-actual-masked-{masked}.png"))).unwrap();
            expected.save_png(std::path::Path::new(&dir).join(format!("gpu-resolved-expected-masked-{masked}.png"))).unwrap();
        }
        let edge = ((20 * 60 + 9) * 4) as usize;
        let edge_value = cpu_expected.data()[edge];
        assert!(edge_value > 0 && edge_value < 255, "fixture must visibly blur at stripe edge, got {edge_value}");
        assert!(expected.data()[edge] < 255, "the GPU reference must visibly blur the stripe edge");
        assert!(
            actual.data() == expected.data(),
            "GPU descendant backdrop must use resolved radius and visibility (edge actual={}, expected={})",
            actual.data()[edge],
            expected.data()[edge]
        );

        t.node(pane).variables.bindings.clear();
        let unbound_frame = g.compile(&t.doc, t.page, 3, &Viewport::default(), (60, 40));
        let unbound = g.render_frame(&unbound_frame, (60, 40));
        assert_eq!(unbound.pixel(15, 20).unwrap().red(), 0, "unbound hidden pane retains its stored visibility");
    }
}

#[test]
fn background_blur_at_the_canvas_edge_matches_the_cpu_reference() {
    let Some(mut g) = common::gpu() else { return };
    let small = group_over_translucent_child_at_edge(Effect::BackgroundBlur { radius: 8.0, visible: true });
    let c_small = common::compare(&mut g, "group-background-blur-canvas-edge-r8", &small.doc, small.page, Viewport::default(), SIZE, 4);
    // The gaussian-vs-box difference leaves about 239 of 8,000 pixels strict-different and 71
    // once AA is forgiven. Repeating the edge texels instead of reading transparent zeros gives
    // 373 and 188, so the AA budget (120 pixels) separates the two; the strict budget (280
    // pixels) keeps real margin over the floor without letting the clamped edge pass either.
    assert!(c_small.report_aa.ratio < 0.015, "small-radius edge mismatch with AA forgiven {:.3}%", c_small.report_aa.ratio * 100.0);
    assert!(c_small.report.ratio < 0.035, "small-radius edge strict mismatch {:.3}%", c_small.report.ratio * 100.0);

    let large = group_over_translucent_child_at_edge(Effect::BackgroundBlur { radius: 40.0, visible: true });
    let c = common::compare(&mut g, "group-background-blur-canvas-edge", &large.doc, large.page, Viewport::default(), SIZE, 4);
    // At radius 40, the transparent-edge path leaves 18 of 8,000 pixels different after AA
    // forgiveness on the Apple M1 adapter. The base clamp path leaves 23 pixels, so the
    // threshold below catches the regression while retaining margin for the Gaussian-vs-box
    // approximation.
    assert!(c.report_aa.ratio < 0.0025, "edge mismatch with AA forgiven {:.3}%", c.report_aa.ratio * 100.0);
    assert!(c.report.ratio < 0.035, "edge strict mismatch {:.3}%", c.report.ratio * 100.0);
}

#[test]
fn background_blur_in_the_middle_of_the_canvas_at_large_radius_is_unchanged() {
    let Some(mut g) = common::gpu() else { return };
    let t = group_over_translucent_child(Effect::BackgroundBlur { radius: 40.0, visible: true });
    let c = common::compare(&mut g, "group-background-blur-mid-canvas-r40", &t.doc, t.page, Viewport::default(), SIZE, 4);
    assert_eq!(fnv1a(c.cpu.data()), 0xd95e2344f3be3855, "pixels of the CPU mid-canvas scene changed");
    assert_eq!(fnv1a(c.gpu.data()), 0x377ace298aa5a96e, "pixels of the GPU mid-canvas scene changed");
    assert!(c.report_aa.ratio < 0.005, "mid-canvas mismatch with AA forgiven {:.3}%", c.report_aa.ratio * 100.0);
    assert!(c.report.ratio < 0.02, "mid-canvas strict mismatch {:.3}%", c.report.ratio * 100.0);
}
