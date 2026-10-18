//! Effect and grid styles in `.fig`: values come from the style node, legacy
//! refs resolve, soft-deleted styles are ignored, and export -> import keeps them.
use designos_doc::*;
use designos_fig::container::{self, Compression};
use designos_fig::kiwi::KiwiValue as V;
use designos_fig::testkit::{color, en, figma_like_schema, fl, guid, msg, node, solid, vec2};
use designos_fig::{export_fig, import_fig};

fn find<'a>(doc: &'a Document, name: &str) -> &'a Node {
    doc.nodes.values().find(|n| n.name == name).unwrap_or_else(|| panic!("no node named {name}"))
}

fn shadow(radius: f32) -> V {
    msg(vec![
        ("type", en("DROP_SHADOW")),
        ("color", color(0.0, 0.0, 0.0, 0.25)),
        ("offset", vec2(0.0, 4.0)),
        ("radius", fl(radius)),
        ("spread", fl(0.0)),
        ("visible", V::Bool(true)),
    ])
}

fn blur(radius: f32) -> V {
    msg(vec![("type", en("FOREGROUND_BLUR")), ("radius", fl(radius)), ("visible", V::Bool(true))])
}

fn columns(count: u32) -> V {
    msg(vec![
        ("pattern", en("STRIPES")),
        ("axis", en("X")),
        ("numSections", V::Int(count as i32)),
        ("gutterSize", fl(20.0)),
        ("offset", fl(0.0)),
        ("visible", V::Bool(true)),
    ])
}

fn square_grid(size: f32) -> V {
    msg(vec![("pattern", en("GRID")), ("sectionSize", fl(size)), ("visible", V::Bool(true))])
}

fn sref(local: u32) -> V {
    msg(vec![("guid", guid(2, local))])
}

fn style_node(local: u32, name: &str, ty: &str, key: &str, mut extra: Vec<(&'static str, V)>) -> V {
    let mut f =
        vec![("styleType", en(ty)), ("styleDescription", V::String(format!("{name} description"))), ("key", V::String(key.into())), ("size", vec2(1.0, 1.0))];
    f.append(&mut extra);
    node((2, local), (0, 2), &format!("{local}"), "RECTANGLE", name, f)
}

fn frame(local: u32, name: &str, extra: Vec<(&'static str, V)>) -> V {
    let mut f = vec![("size", vec2(100.0, 100.0))];
    f.extend(extra);
    node((2, local), (0, 1), &format!("{local}"), "FRAME", name, f)
}

/// Two style nodes (effect 2:100, grid 2:101), a soft-deleted effect style (2:102) and four
/// consumers whose own copies of the values are stale.
fn fixture_bytes() -> Vec<u8> {
    let nodes = vec![
        node((0, 0), (0, 0), "!", "DOCUMENT", "Document", vec![]),
        node((0, 1), (0, 0), "a", "CANVAS", "Page", vec![("backgroundColor", color(1.0, 1.0, 1.0, 1.0))]),
        node((0, 2), (0, 0), "b", "CANVAS", "Internal Only Canvas", vec![("internalOnly", V::Bool(true))]),
        style_node(100, "Shadow/Card", "EFFECT", "key-fx", vec![("effects", V::Array(vec![shadow(8.0)]))]),
        style_node(101, "Layout/12 col", "GRID", "key-grid", vec![("layoutGrids", V::Array(vec![columns(12)]))]),
        style_node(102, "Shadow/Gone", "EFFECT", "key-gone", vec![("effects", V::Array(vec![blur(99.0)])), ("isSoftDeletedStyle", V::Bool(true))]),
        style_node(103, "Border/Strong", "STROKE", "key-stroke", vec![("fillPaints", V::Array(vec![solid(0.0, 1.0, 0.0)]))]),
        frame(5, "Bordered", vec![("styleIdForStrokeFill", sref(103)), ("strokePaints", V::Array(vec![solid(1.0, 0.0, 0.0)])), ("strokeWeight", fl(1.0))]),
        frame(
            1,
            "Modern",
            vec![
                ("styleIdForEffect", sref(100)),
                ("effects", V::Array(vec![shadow(1.0)])),
                ("styleIdForGrid", sref(101)),
                ("layoutGrids", V::Array(vec![square_grid(10.0)])),
            ],
        ),
        frame(
            2,
            "Legacy",
            vec![
                ("inheritEffectStyleID", guid(2, 100)),
                ("effects", V::Array(vec![shadow(2.0)])),
                ("inheritGridStyleID", guid(2, 101)),
                ("layoutGrids", V::Array(vec![square_grid(20.0)])),
            ],
        ),
        frame(3, "Deleted", vec![("styleIdForEffect", sref(102)), ("effects", V::Array(vec![blur(3.0)]))]),
        frame(4, "Plain", vec![("effects", V::Array(vec![shadow(5.0)]))]),
    ];
    let message = msg(vec![("nodeChanges", V::Array(nodes))]);
    let schema = figma_like_schema();
    let root = schema.find("Message").unwrap();
    let body = schema.encode_message(root, &message).unwrap();
    container::write_container(&schema.encode(), &body, Compression::Zstd, 101)
}

fn drop_shadow_radius(e: &Effect) -> f32 {
    match e {
        Effect::DropShadow { radius, offset, .. } => {
            assert_eq!((offset.x, offset.y), (0.0, 4.0));
            *radius
        }
        other => panic!("expected a drop shadow, got {other:?}"),
    }
}

fn grid_columns(doc: &Document, n: &Node) -> u32 {
    match n.kind.frame_props().expect("frame").grids.as_slice() {
        [LayoutGrid::Columns { count, gutter, .. }] => {
            assert_eq!(*gutter, 20.0);
            *count
        }
        other => panic!("expected one column grid on {}, got {other:?} ({} styles)", n.name, doc.styles.len()),
    }
}

#[test]
fn consumers_take_effect_and_grid_values_from_the_style_node() {
    let d = import_fig(&fixture_bytes()).unwrap();
    let modern = find(&d, "Modern");
    assert_eq!(modern.effects.len(), 1);
    assert_eq!(drop_shadow_radius(&modern.effects[0]), 8.0, "the style node wins over the stale copy of the consumer");
    assert_eq!(grid_columns(&d, modern), 12);
    assert_eq!(modern.style_refs["effect"], "2:100");
    assert_eq!(modern.style_refs["grid"], "2:101");
}

#[test]
fn legacy_inherit_refs_resolve_to_the_style_nodes() {
    let d = import_fig(&fixture_bytes()).unwrap();
    let legacy = find(&d, "Legacy");
    assert_eq!(drop_shadow_radius(&legacy.effects[0]), 8.0);
    assert_eq!(grid_columns(&d, legacy), 12);
    assert_eq!(legacy.style_refs["effect"], "2:100");
    assert_eq!(legacy.style_refs["grid"], "2:101");
}

#[test]
fn style_values_and_metadata_are_imported() {
    let d = import_fig(&fixture_bytes()).unwrap();
    match &d.styles["2:100"] {
        Style::Effect(fx) => assert_eq!(drop_shadow_radius(&fx[0]), 8.0),
        other => panic!("expected an effect style, got {other:?}"),
    }
    assert!(matches!(&d.styles["2:101"], Style::Grid(g) if matches!(g.as_slice(), [LayoutGrid::Columns { count: 12, .. }])));
    assert_eq!(
        d.style_meta["2:100"],
        StyleMeta { name: "Shadow/Card".into(), key: Some("key-fx".into()), description: "Shadow/Card description".into(), property: StyleProperty::Effect }
    );
    assert_eq!(d.style_meta["2:101"].property, StyleProperty::Grid);
    assert!(find(&d, "Plain").style_refs.is_empty());
}

#[test]
fn a_soft_deleted_style_is_ignored() {
    let d = import_fig(&fixture_bytes()).unwrap();
    let deleted = find(&d, "Deleted");
    assert_eq!(deleted.effects.len(), 1);
    assert!(matches!(deleted.effects[0], Effect::LayerBlur { radius, .. } if radius == 3.0), "keeps its own effects, not the deleted style's");
    assert!(!d.style_meta.contains_key("2:102"), "no metadata for a deleted style");
    assert!(!deleted.style_refs.contains_key("effect"), "the consumer is left unlinked");
}

#[test]
fn a_stroke_style_node_is_a_stroke_style() {
    let d = import_fig(&fixture_bytes()).unwrap();
    let bordered = find(&d, "Bordered");
    assert_eq!(bordered.style_refs["stroke"], "2:103");
    assert!(matches!(bordered.stroke.paints[0].kind, PaintKind::Solid(c) if c.g == 1.0 && c.r == 0.0), "the style's paint wins over the stale copy");
    assert_eq!(d.style_meta["2:103"].property, StyleProperty::Stroke);
    assert_eq!(d.style_name("2:103"), "Border/Strong");
}

#[test]
fn export_then_import_keeps_effect_and_grid_styles_and_their_links() {
    let d1 = import_fig(&fixture_bytes()).unwrap();
    // An effect style no node uses must survive as well.
    let mut d1 = d1;
    d1.styles.insert("Shadow/Unused".into(), Style::Effect(vec![Effect::LayerBlur { radius: 6.0, visible: true }]));
    d1.style_meta
        .insert("Shadow/Unused".into(), StyleMeta { name: "Shadow/Unused".into(), key: None, description: String::new(), property: StyleProperty::Effect });
    let d2 = import_fig(&export_fig(&d1).unwrap()).unwrap();

    let modern = find(&d2, "Modern");
    let fx_key = &modern.style_refs["effect"];
    let grid_key = &modern.style_refs["grid"];
    assert_eq!(drop_shadow_radius(&modern.effects[0]), 8.0);
    assert_eq!(grid_columns(&d2, modern), 12);
    assert!(matches!(&d2.styles[fx_key], Style::Effect(fx) if drop_shadow_radius(&fx[0]) == 8.0));
    assert!(matches!(&d2.styles[grid_key], Style::Grid(g) if matches!(g.as_slice(), [LayoutGrid::Columns { count: 12, .. }])));
    assert_eq!(d2.style_name(fx_key), "Shadow/Card");
    assert_eq!(d2.style_meta[fx_key].property, StyleProperty::Effect);
    assert_eq!(d2.style_name(grid_key), "Layout/12 col");
    assert_eq!(d2.style_meta[grid_key].property, StyleProperty::Grid);
    // Legacy consumer was normalised to the modern reference and shares the same style.
    assert_eq!(&find(&d2, "Legacy").style_refs["effect"], fx_key);

    let unused = d2.style_meta.iter().find(|(_, m)| m.name == "Shadow/Unused").map(|(k, _)| k.clone()).expect("unused style survives");
    assert!(matches!(&d2.styles[&unused], Style::Effect(fx) if matches!(fx.as_slice(), [Effect::LayerBlur { radius, .. }] if *radius == 6.0)));
    assert_eq!(d2.style_meta[&unused].property, StyleProperty::Effect);
}

#[test]
fn noise_texture_and_glass_effects_round_trip_through_fig_import_export_reimport() {
    let mono_noise = msg(vec![
        ("type", en("NOISE")),
        ("noiseType", en("MONOTONE")),
        ("color", color(0.25, 0.5, 0.75, 1.0)),
        ("noiseSize", vec2(3.5, 3.5)),
        ("density", fl(0.65)),
        ("opacity", fl(0.4)),
        ("blendMode", en("OVERLAY")),
        ("seed", V::Uint(42)),
        ("visible", V::Bool(true)),
    ]);
    let duo_noise = msg(vec![
        ("type", en("NOISE")),
        ("noiseType", en("DUOTONE")),
        ("color", color(1.0, 0.0, 0.0, 1.0)),
        ("secondaryColor", color(0.0, 0.0, 1.0, 0.5)),
        ("noiseSize", vec2(2.0, 2.0)),
        ("density", fl(0.35)),
        ("opacity", fl(0.8)),
        ("blendMode", en("MULTIPLY")),
        ("seed", V::Uint(9)),
        ("visible", V::Bool(false)),
    ]);
    let multi_noise = msg(vec![
        ("type", en("NOISE")),
        ("noiseType", en("MULTITONE")),
        ("noiseSize", vec2(1.5, 1.5)),
        ("density", fl(0.9)),
        ("opacity", fl(0.5)),
        ("blendMode", en("SCREEN")),
        ("seed", V::Uint(77)),
        ("visible", V::Bool(true)),
    ]);
    let grain = msg(vec![
        ("type", en("GRAIN")),
        ("noiseSize", vec2(4.0, 4.0)),
        ("radius", fl(6.5)),
        ("clipToShape", V::Bool(false)),
        ("seed", V::Uint(13)),
        ("visible", V::Bool(false)),
    ]);
    let glass = msg(vec![
        ("type", en("GLASS")),
        ("specularAngle", fl(135.0)),
        ("specularIntensity", fl(0.75)),
        ("refractionIntensity", fl(0.6)),
        ("refractionRadius", fl(18.0)),
        ("chromaticAberration", fl(0.3)),
        ("radius", fl(8.0)),
        ("visible", V::Bool(true)),
    ]);

    let nodes = vec![
        node((0, 0), (0, 0), "!", "DOCUMENT", "Document", vec![]),
        node((0, 1), (0, 0), "a", "CANVAS", "Page", vec![("backgroundColor", color(1.0, 1.0, 1.0, 1.0))]),
        node((0, 2), (0, 0), "b", "CANVAS", "Internal Only Canvas", vec![("internalOnly", V::Bool(true))]),
        style_node(200, "FX/Rich", "EFFECT", "key-fx-rich", vec![("effects", V::Array(vec![duo_noise.clone(), grain.clone(), glass.clone()]))]),
        frame(10, "DirectFX", vec![("effects", V::Array(vec![mono_noise, duo_noise, multi_noise, grain, glass]))]),
        frame(11, "StyledFX", vec![("styleIdForEffect", sref(200)), ("effects", V::Array(vec![shadow(1.0)]))]),
    ];
    let message = msg(vec![("nodeChanges", V::Array(nodes))]);
    let schema = figma_like_schema();
    let root = schema.find("Message").unwrap();
    let body = schema.encode_message(root, &message).unwrap();
    let fig_bytes = container::write_container(&schema.encode(), &body, Compression::Zstd, 106);

    let expected_mono = Effect::Noise(NoiseEffect {
        mode: NoiseMode::Mono { color: Color { r: 0.25, g: 0.5, b: 0.75, a: 1.0 } },
        size: 3.5,
        density: 0.65,
        opacity: 0.4,
        blend: BlendMode::Overlay,
        seed: 42,
        visible: true,
    });
    let expected_duo = Effect::Noise(NoiseEffect {
        mode: NoiseMode::Duo { color: Color { r: 1.0, g: 0.0, b: 0.0, a: 1.0 }, secondary: Color { r: 0.0, g: 0.0, b: 1.0, a: 0.5 } },
        size: 2.0,
        density: 0.35,
        opacity: 0.8,
        blend: BlendMode::Multiply,
        seed: 9,
        visible: false,
    });
    let expected_multi =
        Effect::Noise(NoiseEffect { mode: NoiseMode::Multi, size: 1.5, density: 0.9, opacity: 0.5, blend: BlendMode::Screen, seed: 77, visible: true });
    let expected_texture = Effect::Texture(TextureEffect { size: 4.0, radius: 6.5, clip_to_shape: false, seed: 13, visible: false });
    let expected_glass =
        Effect::Glass(GlassEffect { light_angle: 135.0, light_intensity: 0.75, refraction: 0.6, depth: 18.0, dispersion: 0.3, frost: 8.0, visible: true });

    // 1. First import from .fig
    let d1 = import_fig(&fig_bytes).unwrap();
    assert_eq!(
        find(&d1, "DirectFX").effects,
        vec![expected_mono.clone(), expected_duo.clone(), expected_multi.clone(), expected_texture.clone(), expected_glass.clone()]
    );
    let expected_style_fx = vec![expected_duo, expected_texture, expected_glass];
    assert_eq!(find(&d1, "StyledFX").effects, expected_style_fx);
    assert_eq!(d1.styles["2:200"], Style::Effect(expected_style_fx.clone()));

    // 2. Export to .fig and re-import
    let exported = export_fig(&d1).unwrap();
    let d2 = import_fig(&exported).unwrap();
    assert_eq!(
        find(&d2, "DirectFX").effects,
        vec![expected_mono, expected_style_fx[0].clone(), expected_multi, expected_style_fx[1].clone(), expected_style_fx[2].clone()]
    );
    let styled = find(&d2, "StyledFX");
    assert_eq!(styled.effects, expected_style_fx);
    let fx_key = &styled.style_refs["effect"];
    assert_eq!(d2.styles[fx_key], Style::Effect(expected_style_fx));
}
