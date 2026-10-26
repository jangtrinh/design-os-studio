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
        StyleMeta {
            name: "Shadow/Card".into(),
            key: Some("key-fx".into()),
            description: "Shadow/Card description".into(),
            property: StyleProperty::Effect,
            bindings: Default::default(),
            ..Default::default()
        }
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
    d1.style_meta.insert(
        "Shadow/Unused".into(),
        StyleMeta {
            name: "Shadow/Unused".into(),
            key: None,
            description: String::new(),
            property: StyleProperty::Effect,
            bindings: Default::default(),
            ..Default::default()
        },
    );
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

fn add_two_mode_collection(doc: &mut Document) {
    doc.variable_collections.insert(
        "VariableCollectionId:3:1".into(),
        VariableCollection {
            id: "VariableCollectionId:3:1".into(),
            name: "Tokens".into(),
            modes: vec![VariableMode { id: "3:10".into(), name: "Light".into() }, VariableMode { id: "3:11".into(), name: "Dark".into() }],
            default_mode: "3:10".into(),
            variables: vec![],
            key: None,
            source_library_key: None,
            extensions: Default::default(),
        },
    );
}

fn insert_var(doc: &mut Document, id: &str, name: &str, kind: VariableKind, light: VariableValue, dark: VariableValue) {
    doc.variable_collections.get_mut("VariableCollectionId:3:1").unwrap().variables.push(id.into());
    doc.variables.insert(
        id.into(),
        Variable {
            id: id.into(),
            name: name.into(),
            collection: "VariableCollectionId:3:1".into(),
            kind,
            values: [("3:10".to_string(), light), ("3:11".to_string(), dark)].into_iter().collect(),
            description: String::new(),
            scopes: vec![],
            hidden_from_publishing: false,
            key: None,
            code_syntax: Default::default(),
            extensions: Default::default(),
            mode_extensions: Default::default(),
        },
    );
}

#[test]
fn fig_style_colorvar_round_trips() {
    let mut d = Document::new("StyleColorVar");
    d.add_page("Page");
    add_two_mode_collection(&mut d);
    let var_id = "VariableID:3:20".to_string();
    insert_var(
        &mut d,
        &var_id,
        "surface",
        VariableKind::Color,
        VariableValue::Color(Color::rgb(0xFF, 0xFF, 0xFF)),
        VariableValue::Color(Color::rgb(0x11, 0x11, 0x11)),
    );
    d.styles.insert("2:500".into(), Style::Fill(vec![Paint::solid(Color::WHITE)]));
    d.style_meta.insert(
        "2:500".into(),
        StyleMeta {
            name: "Card".into(),
            key: Some("k-card".into()),
            description: String::new(),
            property: StyleProperty::Fill,
            bindings: [("fills/0".to_string(), var_id.clone())].into_iter().collect(),
            ..Default::default()
        },
    );

    let bytes = export_fig(&d).unwrap();
    let d2 = import_fig(&bytes).unwrap();
    let (key, meta) = d2.style_meta.iter().find(|(_, m)| m.name == "Card").expect("Card style imported");
    assert_eq!(meta.bindings.get("fills/0"), Some(&var_id), "style bindings on {key}: {:?}", meta.bindings);
}

#[test]
fn text_style_paragraph_indent_and_weight_round_trip() {
    let mut d = Document::new("TextStyleVars");
    d.add_page("Page");
    add_two_mode_collection(&mut d);
    let indent_var = "VariableID:3:21".to_string();
    let weight_var = "VariableID:3:22".to_string();
    let style_var = "VariableID:3:23".to_string();
    insert_var(&mut d, &indent_var, "indent", VariableKind::Number, VariableValue::Number(12.0), VariableValue::Number(24.0));
    insert_var(&mut d, &weight_var, "weight", VariableKind::Number, VariableValue::Number(400.0), VariableValue::Number(700.0));
    insert_var(&mut d, &style_var, "fontStyle", VariableKind::String, VariableValue::String("Regular".into()), VariableValue::String("Bold Italic".into()));

    d.styles.insert("2:501".into(), Style::Text(TextStyle { font_weight: 400, ..TextStyle::default() }));
    d.style_meta.insert(
        "2:501".into(),
        StyleMeta {
            name: "Body".into(),
            key: Some("k-body".into()),
            description: String::new(),
            property: StyleProperty::Text,
            bindings: [
                ("paragraphIndent".to_string(), indent_var.clone()),
                ("fontWeight".to_string(), weight_var.clone()),
                ("fontStyle".to_string(), style_var.clone()),
            ]
            .into_iter()
            .collect(),
            ..Default::default()
        },
    );

    let bytes = export_fig(&d).unwrap();
    let d2 = import_fig(&bytes).unwrap();
    let (_, meta) = d2.style_meta.iter().find(|(_, m)| m.name == "Body").expect("Body style imported");
    assert_eq!(meta.bindings.get("paragraphIndent"), Some(&indent_var));
    assert_eq!(meta.bindings.get("fontWeight"), Some(&weight_var));
    assert_eq!(meta.bindings.get("fontStyle"), Some(&style_var));
}

#[test]
fn fill_style_gradient_stop_binding_round_trips() {
    let mut d = Document::new("GradStopVar");
    let page = d.add_page("Page");
    add_two_mode_collection(&mut d);
    let stop_var = "VariableID:3:24".to_string();
    let light_c = Color::rgb(0xFF, 0x00, 0x00);
    let dark_c = Color::rgb(0x00, 0xFF, 0x00);
    insert_var(&mut d, &stop_var, "grad-start", VariableKind::Color, VariableValue::Color(light_c), VariableValue::Color(dark_c));

    let grad = Paint {
        kind: PaintKind::LinearGradient {
            handles: [Vec2::new(0.0, 0.5), Vec2::new(1.0, 0.5), Vec2::new(0.0, 1.0)],
            stops: vec![GradientStop { pos: 0.0, color: light_c }, GradientStop { pos: 1.0, color: Color::BLACK }],
        },
        opacity: 1.0,
        visible: true,
        blend: BlendMode::Normal,
    };
    d.styles.insert("2:502".into(), Style::Fill(vec![grad.clone()]));
    d.style_meta.insert(
        "2:502".into(),
        StyleMeta {
            name: "HeroGrad".into(),
            key: Some("k-grad".into()),
            description: String::new(),
            property: StyleProperty::Fill,
            bindings: [("fills/0/stops/0".to_string(), stop_var.clone())].into_iter().collect(),
            ..Default::default()
        },
    );
    let fid = d.alloc_id();
    let mut frame = Node::frame(fid, Rect::new(0.0, 0.0, 100.0, 100.0));
    frame.name = "DarkFrame".into();
    frame.variables.modes.insert("VariableCollectionId:3:1".into(), "3:11".into());
    let fid = d.insert(page, frame).unwrap();
    let rid = d.alloc_id();
    let mut rect = Node::rectangle(rid, Rect::new(0.0, 0.0, 50.0, 50.0));
    rect.name = "GradRect".into();
    rect.fills = vec![grad];
    rect.style_refs.insert("fill".into(), "2:502".into());
    d.insert(fid, rect).unwrap();

    let bytes = export_fig(&d).unwrap();
    let d2 = import_fig(&bytes).unwrap();
    let (_, meta) = d2.style_meta.iter().find(|(_, m)| m.name == "HeroGrad").expect("HeroGrad style imported");
    assert_eq!(meta.bindings.get("fills/0/stops/0"), Some(&stop_var));

    let r2 = find(&d2, "GradRect");
    let resolved = resolve_node(&d2, r2);
    let PaintKind::LinearGradient { stops, .. } = &resolved.fills[0].kind else { panic!("expected linear gradient") };
    assert_eq!(stops[0].color, dark_c);
}

#[test]
fn effect_style_shadow_vars_round_trip() {
    let mut d = Document::new("EffectVars");
    let page = d.add_page("Page");
    add_two_mode_collection(&mut d);
    let c_var = "VariableID:3:25".to_string();
    let r_var = "VariableID:3:26".to_string();
    let s_var = "VariableID:3:27".to_string();
    let x_var = "VariableID:3:28".to_string();
    let y_var = "VariableID:3:29".to_string();
    let light_c = Color::rgb(0x22, 0x22, 0x22);
    let dark_c = Color::rgb(0xEE, 0xEE, 0xEE);
    insert_var(&mut d, &c_var, "sh-color", VariableKind::Color, VariableValue::Color(light_c), VariableValue::Color(dark_c));
    insert_var(&mut d, &r_var, "sh-radius", VariableKind::Number, VariableValue::Number(4.0), VariableValue::Number(16.0));
    insert_var(&mut d, &s_var, "sh-spread", VariableKind::Number, VariableValue::Number(1.0), VariableValue::Number(6.0));
    insert_var(&mut d, &x_var, "sh-x", VariableKind::Number, VariableValue::Number(2.0), VariableValue::Number(8.0));
    insert_var(&mut d, &y_var, "sh-y", VariableKind::Number, VariableValue::Number(3.0), VariableValue::Number(12.0));

    let fx = Effect::DropShadow { color: light_c, offset: Vec2::new(2.0, 3.0), radius: 4.0, spread: 1.0, visible: true, show_behind: false };
    d.styles.insert("2:503".into(), Style::Effect(vec![fx.clone()]));
    d.style_meta.insert(
        "2:503".into(),
        StyleMeta {
            name: "Elevation".into(),
            key: Some("k-elev".into()),
            description: String::new(),
            property: StyleProperty::Effect,
            bindings: [
                ("effects/0/color".to_string(), c_var.clone()),
                ("effects/0/radius".to_string(), r_var.clone()),
                ("effects/0/spread".to_string(), s_var.clone()),
                ("effects/0/offsetX".to_string(), x_var.clone()),
                ("effects/0/offsetY".to_string(), y_var.clone()),
            ]
            .into_iter()
            .collect(),
            ..Default::default()
        },
    );
    let fid = d.alloc_id();
    let mut frame = Node::frame(fid, Rect::new(0.0, 0.0, 100.0, 100.0));
    frame.name = "DarkFrame".into();
    frame.variables.modes.insert("VariableCollectionId:3:1".into(), "3:11".into());
    let fid = d.insert(page, frame).unwrap();
    let rid = d.alloc_id();
    let mut rect = Node::rectangle(rid, Rect::new(0.0, 0.0, 50.0, 50.0));
    rect.name = "Elevated".into();
    rect.effects = vec![fx];
    rect.style_refs.insert("effect".into(), "2:503".into());
    d.insert(fid, rect).unwrap();

    let bytes = export_fig(&d).unwrap();
    let d2 = import_fig(&bytes).unwrap();
    let (_, meta) = d2.style_meta.iter().find(|(_, m)| m.name == "Elevation").expect("Elevation style imported");
    assert_eq!(meta.bindings.get("effects/0/color"), Some(&c_var));
    assert_eq!(meta.bindings.get("effects/0/radius"), Some(&r_var));
    assert_eq!(meta.bindings.get("effects/0/spread"), Some(&s_var));
    assert_eq!(meta.bindings.get("effects/0/offsetX"), Some(&x_var));
    assert_eq!(meta.bindings.get("effects/0/offsetY"), Some(&y_var));

    let r2 = find(&d2, "Elevated");
    let resolved = resolve_node(&d2, r2);
    let Effect::DropShadow { color, offset, radius, spread, .. } = resolved.effects[0] else { panic!("expected drop shadow") };
    assert_eq!(color, dark_c);
    assert_eq!(radius, 16.0);
    assert_eq!(spread, 6.0);
    assert_eq!(offset, Vec2::new(8.0, 12.0));
}

fn exported_style_node_names(bytes: &[u8]) -> Vec<String> {
    let arch = container::open(bytes).expect("open exported .fig archive");
    let (schema_bytes, data_bytes) = container::read_archive_chunks(&arch).expect("decode exported .fig chunks");
    let s = designos_fig::kiwi::Schema::decode(&schema_bytes).expect("decode exported .fig schema");
    let r = s.find("Message").expect("find Message in schema");
    let msg_val = s.decode_message(r, &data_bytes).expect("decode exported .fig message");
    msg_val
        .get("nodeChanges")
        .and_then(|v| v.as_array())
        .into_iter()
        .flatten()
        .filter(|nc| nc.get("styleType").is_some())
        .filter_map(|nc| match nc.get("name") {
            Some(V::String(s)) => Some(s.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn deleted_style_leaves_no_orphan_style_node_or_dangling_ref_on_fig_round_trip() {
    let mut d = import_fig(&fixture_bytes()).unwrap();
    assert!(
        exported_style_node_names(&export_fig(&d).unwrap()).iter().any(|n| n == "Shadow/Card"),
        "precondition: exported .fig before delete_style carries the Shadow/Card style node"
    );
    let mut h = History::default();
    let cmd = delete_style(&d, "2:100").expect("delete_style succeeds on local effect style");
    h.exec(&mut d, cmd).unwrap();

    let bytes = export_fig(&d).expect("export_fig succeeds after deleting style");
    let style_names = exported_style_node_names(&bytes);
    assert!(!style_names.iter().any(|n| n == "Shadow/Card" || n == "2:100"), "exported .fig still has a style node for deleted Shadow/Card: {style_names:?}");
    let d2 = import_fig(&bytes).expect("import_fig succeeds on exported bytes");

    assert!(!d2.styles.contains_key("2:100"), "deleted style key must not reappear in styles");
    assert!(!d2.style_meta.contains_key("2:100"), "deleted style key must not reappear in style_meta");
    assert!(d2.style_meta.values().all(|m| m.name != "Shadow/Card"), "no orphan style_meta entry named Shadow/Card");
    assert!(d2.nodes.values().all(|n| n.name != "Shadow/Card"), "no orphan style node named Shadow/Card");
    for n in d2.nodes.values() {
        for (slot, target) in &n.style_refs {
            assert_ne!(target, "2:100", "node {} still points at deleted style 2:100 via {slot}", n.name);
            assert!(d2.styles.contains_key(target), "node {} has dangling style_ref {slot} -> {target}", n.name);
        }
    }
    for name in ["Modern", "Legacy"] {
        let n = find(&d2, name);
        assert!(!n.style_refs.contains_key("effect"), "{name} has no effect style ref");
        assert_eq!(drop_shadow_radius(&n.effects[0]), 8.0, "{name} keeps its resolved effect values");
        assert_eq!(n.style_refs.get("grid").map(String::as_str), Some("2:101"), "{name} keeps its other style refs");
    }
}

/// A .fig whose internal canvas holds a remote component that uses the same style as a local layer.
fn remote_component_fixture() -> Vec<u8> {
    let nodes = vec![
        node((0, 0), (0, 0), "!", "DOCUMENT", "Document", vec![]),
        node((0, 1), (0, 0), "a", "CANVAS", "Page", vec![("backgroundColor", color(1.0, 1.0, 1.0, 1.0))]),
        node((0, 2), (0, 0), "b", "CANVAS", "Internal Only Canvas", vec![("internalOnly", V::Bool(true))]),
        style_node(100, "Shadow/Card", "EFFECT", "key-fx", vec![("effects", V::Array(vec![shadow(8.0)]))]),
        node((2, 300), (0, 2), "c", "SYMBOL", "Remote Card", vec![("size", vec2(100.0, 100.0)), ("key", V::String("comp-key".into()))]),
        node(
            (2, 301),
            (2, 300),
            "a",
            "FRAME",
            "Remote Card Body",
            vec![("size", vec2(50.0, 50.0)), ("styleIdForEffect", sref(100)), ("effects", V::Array(vec![shadow(8.0)]))],
        ),
        frame(1, "Local", vec![("styleIdForEffect", sref(100)), ("effects", V::Array(vec![shadow(8.0)]))]),
    ];
    let message = msg(vec![("nodeChanges", V::Array(nodes))]);
    let schema = figma_like_schema();
    let root = schema.find("Message").unwrap();
    let body = schema.encode_message(root, &message).unwrap();
    container::write_container(&schema.encode(), &body, Compression::Zstd, 101)
}

#[test]
fn a_style_used_by_a_remote_component_on_the_library_page_cannot_be_deleted() {
    let d = import_fig(&remote_component_fixture()).unwrap();
    let body = d.nodes.values().find(|n| n.name == "Remote Card Body").map(|n| n.id).expect("remote body imported");
    assert!(is_library_node(&d, body), "Remote Card Body sits on the hidden library page");

    let err = delete_style(&d, "2:100").unwrap_err();
    assert!(matches!(&err, StyleOpError::LibraryStyle(k) if k == "2:100"), "expected LibraryStyle error, got {err:?}");
    assert!(d.styles.contains_key("2:100"));
    for n in d.nodes.values() {
        for target in n.style_refs.values() {
            assert!(d.styles.contains_key(target), "no dangling style ref on {}", n.name);
        }
    }
}

#[test]
fn deleting_colour_and_text_styles_on_mixed_run_text_preserves_rendered_pixels() {
    let mut d = Document::new("t");
    let page = d.add_page("p");
    let black = vec![Paint::solid(Color::BLACK)];
    let red = vec![Paint::solid(Color::rgb(255, 0, 0))];
    let base_fill = TextStyle { font_size: 20.0, fills: black.clone(), ..Default::default() };
    let hi_fill = TextStyle { fills: red, ..base_fill.clone() };
    let t_fill_id = d.alloc_id();
    let mut t_fill = Node::text(t_fill_id, Rect::new(0.0, 0.0, 220.0, 40.0), "Hello world", base_fill.clone());
    if let NodeKind::Text(tp) = &mut t_fill.kind {
        tp.runs = vec![TextRun { start: 0, end: 5, style: base_fill }, TextRun { start: 5, end: 11, style: hi_fill }];
    }
    let t_fill = d.insert(page, t_fill).unwrap();
    d.get_mut(t_fill).unwrap().style_refs.insert("fill".into(), "Ink".into());
    d.styles.insert("Ink".into(), Style::Fill(black));
    d.style_meta.insert("Ink".into(), StyleMeta { name: "Ink".into(), property: StyleProperty::Fill, ..Default::default() });

    let base_text = TextStyle { font_size: 20.0, font_weight: 400, ..Default::default() };
    let bold_text = TextStyle { font_weight: 700, ..base_text.clone() };
    let t_text_id = d.alloc_id();
    let mut t_text = Node::text(t_text_id, Rect::new(0.0, 50.0, 220.0, 40.0), "Hello world", base_text.clone());
    if let NodeKind::Text(tp) = &mut t_text.kind {
        tp.runs = vec![TextRun { start: 0, end: 5, style: base_text.clone() }, TextRun { start: 5, end: 11, style: bold_text }];
    }
    let t_text = d.insert(page, t_text).unwrap();
    d.get_mut(t_text).unwrap().style_refs.insert("text".into(), "Body".into());
    d.styles.insert("Body".into(), Style::Text(base_text));
    d.style_meta.insert("Body".into(), StyleMeta { name: "Body".into(), property: StyleProperty::Text, ..Default::default() });

    let fill_px_before = designos_render::render_node(&d, t_fill, 1.0).expect("render t_fill before").data().to_vec();
    let text_px_before = designos_render::render_node(&d, t_text, 1.0).expect("render t_text before").data().to_vec();

    let mut h = History::default();
    let c1 = delete_style(&d, "Ink").unwrap();
    h.exec(&mut d, c1).unwrap();
    let c2 = delete_style(&d, "Body").unwrap();
    h.exec(&mut d, c2).unwrap();

    let fill_px_after = designos_render::render_node(&d, t_fill, 1.0).expect("render t_fill after").data().to_vec();
    let text_px_after = designos_render::render_node(&d, t_text, 1.0).expect("render t_text after").data().to_vec();
    let fill_diff = fill_px_after.iter().zip(&fill_px_before).filter(|(a, b)| a != b).count();
    let text_diff = text_px_after.iter().zip(&text_px_before).filter(|(a, b)| a != b).count();
    assert_eq!(fill_diff, 0, "rendered pixels of mixed-colour text must not change after deleting its fill style ({fill_diff} bytes differed)");
    assert_eq!(text_diff, 0, "rendered pixels of mixed-weight text must not change after deleting its text style ({text_diff} bytes differed)");
}

#[test]
fn own_font_style_binding_and_style_font_weight_binding_preserve_resolved_node_across_modes() {
    use designos_doc::{resolve_node, Variable, VariableCollection, VariableKind, VariableMode, VariableValue};

    let mut d = Document::new("t");
    let p = d.add_page("p");
    d.variable_collections.insert(
        "c".into(),
        VariableCollection {
            id: "c".into(),
            name: "C".into(),
            modes: vec![VariableMode { id: "m1".into(), name: "M1".into() }, VariableMode { id: "m2".into(), name: "M2".into() }],
            default_mode: "m1".into(),
            variables: vec!["v_style".into(), "v_weight".into()],
            key: None,
            source_library_key: None,
            extensions: Default::default(),
        },
    );
    d.variables.insert(
        "v_style".into(),
        Variable {
            id: "v_style".into(),
            name: "fontStyle".into(),
            collection: "c".into(),
            kind: VariableKind::String,
            values: [("m1".to_string(), VariableValue::String("Bold Italic".into())), ("m2".to_string(), VariableValue::String("SemiBold Italic".into()))]
                .into_iter()
                .collect(),
            description: String::new(),
            scopes: vec![],
            hidden_from_publishing: false,
            key: None,
            code_syntax: Default::default(),
            extensions: Default::default(),
            mode_extensions: Default::default(),
        },
    );
    d.variables.insert(
        "v_weight".into(),
        Variable {
            id: "v_weight".into(),
            name: "fontWeight".into(),
            collection: "c".into(),
            kind: VariableKind::Number,
            values: [("m1".to_string(), VariableValue::Number(300.0)), ("m2".to_string(), VariableValue::Number(400.0))].into_iter().collect(),
            description: String::new(),
            scopes: vec![],
            hidden_from_publishing: false,
            key: None,
            code_syntax: Default::default(),
            extensions: Default::default(),
            mode_extensions: Default::default(),
        },
    );

    let tid = d.alloc_id();
    let mut t = Node::text(tid, Rect::new(0.0, 0.0, 120.0, 30.0), "Sample", TextStyle::default());
    t.style_refs.insert("text".into(), "Heading".into());
    t.variables.bindings.insert("fontStyle".into(), "v_style".into());
    let tid = d.insert(p, t).unwrap();

    d.styles.insert("Heading".into(), Style::Text(TextStyle::default()));
    d.style_meta.insert(
        "Heading".into(),
        StyleMeta {
            name: "Heading".into(),
            property: StyleProperty::Text,
            bindings: [("fontWeight".to_string(), "v_weight".to_string())].into_iter().collect(),
            ..Default::default()
        },
    );

    let mut before_by_mode = Vec::new();
    for mode in ["m1", "m2"] {
        d.get_mut(tid).unwrap().variables.modes.insert("c".into(), mode.into());
        let mut r = resolve_node(&d, d.get(tid).unwrap()).into_owned();
        r.style_refs.clear();
        r.variables = Default::default();
        before_by_mode.push((mode, r));
    }
    d.get_mut(tid).unwrap().variables.modes.remove("c");

    let mut h = History::default();
    let cmd = delete_style(&d, "Heading").unwrap();
    h.exec(&mut d, cmd).unwrap();

    for (mode, before) in before_by_mode {
        d.get_mut(tid).unwrap().variables.modes.insert("c".into(), mode.into());
        let mut after = resolve_node(&d, d.get(tid).unwrap()).into_owned();
        after.variables = Default::default();
        assert_eq!(after, before, "resolve_node in mode {mode} must match before and after delete_style");
    }
}

#[test]
fn style_sort_position_and_folder_order_round_trip() {
    let nodes = vec![
        node((0, 0), (0, 0), "!", "DOCUMENT", "Document", vec![]),
        node((0, 1), (0, 0), "a", "CANVAS", "Page", vec![("backgroundColor", color(1.0, 1.0, 1.0, 1.0))]),
        node((0, 2), (0, 0), "b", "CANVAS", "Internal Only Canvas", vec![("internalOnly", V::Bool(true))]),
        style_node(
            201,
            "Shadows/shadow-lg",
            "EFFECT",
            "key-lg",
            vec![("effects", V::Array(vec![shadow(16.0)])), ("sortPosition", V::String("!~~p~oLx".into()))],
        ),
        style_node(202, "Shadows/shadow-xs", "EFFECT", "key-xs", vec![("effects", V::Array(vec![shadow(2.0)])), ("sortPosition", V::String("!~~p~oL".into()))]),
        style_node(
            203,
            "Focus rings/focus-ring",
            "EFFECT",
            "key-ring",
            vec![("effects", V::Array(vec![shadow(4.0)])), ("sortPosition", V::String("\"'#".into()))],
        ),
        style_node(
            204,
            "Shadows/shadow-sm",
            "EFFECT",
            "key-sm",
            vec![("effects", V::Array(vec![shadow(6.0)])), ("sortPosition", V::String("!~~p~oLf".into()))],
        ),
    ];
    let message = msg(vec![("nodeChanges", V::Array(nodes))]);
    let schema = figma_like_schema();
    let root = schema.find("Message").unwrap();
    let body = schema.encode_message(root, &message).unwrap();
    let fig_bytes = container::write_container(&schema.encode(), &body, Compression::Zstd, 101);

    let d1 = import_fig(&fig_bytes).unwrap();
    assert_eq!(d1.style_meta["2:202"].sort_position.as_deref(), Some("!~~p~oL"));
    assert_eq!(d1.style_meta["2:204"].sort_position.as_deref(), Some("!~~p~oLf"));
    assert_eq!(d1.style_meta["2:201"].sort_position.as_deref(), Some("!~~p~oLx"));
    assert_eq!(d1.style_meta["2:203"].sort_position.as_deref(), Some("\"'#"));

    let order1: Vec<(String, Option<String>)> = ordered_style_keys(&d1, Some(StyleProperty::Effect))
        .into_iter()
        .map(|k| (d1.style_name(&k).to_string(), d1.style_meta[&k].sort_position.clone()))
        .collect();
    assert_eq!(
        order1,
        vec![
            ("Shadows/shadow-xs".into(), Some("!~~p~oL".into())),
            ("Shadows/shadow-sm".into(), Some("!~~p~oLf".into())),
            ("Shadows/shadow-lg".into(), Some("!~~p~oLx".into())),
            ("Focus rings/focus-ring".into(), Some("\"'#".into())),
        ]
    );

    let exported = export_fig(&d1).unwrap();
    let d2 = import_fig(&exported).unwrap();
    let order2: Vec<(String, Option<String>)> = ordered_style_keys(&d2, Some(StyleProperty::Effect))
        .into_iter()
        .map(|k| (d2.style_name(&k).to_string(), d2.style_meta[&k].sort_position.clone()))
        .collect();
    assert_eq!(order2, order1, "import -> export -> import must keep exact order and sortPosition values");
}

#[test]
fn unpositioned_ten_fill_styles_keep_order_through_fig_round_trip() {
    let mut d = Document::new("TenStyles");
    d.add_page("Page 1");
    let expected_names: Vec<String> = (1..=10).map(|i| format!("Fill/S{i:02}")).collect();
    for i in 1..=10 {
        let key = format!("S{i:02}");
        let name = format!("Fill/S{i:02}");
        d.styles.insert(key.clone(), Style::Fill(vec![Paint::solid(Color::rgb(i as u8 * 10, 20, 30))]));
        d.style_meta.insert(key, StyleMeta { name, property: StyleProperty::Fill, sort_position: None, ..Default::default() });
    }
    assert!(d.style_meta.values().all(|m| m.sort_position.is_none()));

    let exported = export_fig(&d).unwrap();
    assert!(d.style_meta.values().all(|m| m.sort_position.is_none()), "export must not mutate in-memory doc sort_position");

    let reimported = import_fig(&exported).unwrap();
    let names_after: Vec<String> =
        ordered_style_keys(&reimported, Some(StyleProperty::Fill)).into_iter().map(|k| reimported.style_name(&k).to_string()).collect();
    assert_eq!(names_after, expected_names, "ten unpositioned fill styles S01..S10 must keep their order through .fig export and import");
}

#[test]
fn mixed_positioned_and_unpositioned_styles_keep_order_through_fig_round_trip() {
    let mut d = Document::new("MixedStyles");
    d.add_page("Page 1");
    for (key, name, pos, r) in [
        ("p_mid", "Shadows/sm", Some("!~~p~oLf"), 20),
        ("p_last", "Focus rings/ring", Some("\"'#"), 30),
        ("p_first", "Shadows/xs", Some("!~~p~oL"), 10),
        ("1_unpos_first", "Custom/First", None, 40),
        ("2:2", "Custom/Second", None, 50),
    ] {
        d.styles.insert(key.into(), Style::Fill(vec![Paint::solid(Color::rgb(r, 0, 0))]));
        d.style_meta
            .insert(key.into(), StyleMeta { name: name.into(), property: StyleProperty::Fill, sort_position: pos.map(str::to_string), ..Default::default() });
    }

    let exported = export_fig(&d).unwrap();
    assert_eq!(d.style_meta["1_unpos_first"].sort_position, None);
    assert_eq!(d.style_meta["2:2"].sort_position, None);

    let reimported = import_fig(&exported).unwrap();
    let after: Vec<(String, Option<String>)> = ordered_style_keys(&reimported, Some(StyleProperty::Fill))
        .into_iter()
        .map(|k| (reimported.style_name(&k).to_string(), reimported.style_meta[&k].sort_position.clone()))
        .collect();
    let names_after: Vec<&str> = after.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(
        names_after,
        vec!["Shadows/xs", "Shadows/sm", "Focus rings/ring", "Custom/First", "Custom/Second"],
        "positioned styles must come first in sortPosition order, followed by unpositioned styles in stored order"
    );
    assert_eq!(after[0].1.as_deref(), Some("!~~p~oL"));
    assert_eq!(after[1].1.as_deref(), Some("!~~p~oLf"));
    assert_eq!(after[2].1.as_deref(), Some("\"'#"));
    assert!(after[3].1.is_some() && after[4].1.is_some(), "unpositioned styles receive generated sortPosition on round trip");
}

#[test]
fn style_description_fig_round_trip_keeps_empty_and_non_empty_descriptions() {
    let mut d_no_desc = Document::new("NoDescriptions");
    d_no_desc.add_page("Page 1");
    for (key, name, prop, style) in [
        ("fill_1", "Brand/Primary", StyleProperty::Fill, Style::Fill(vec![Paint::solid(Color::rgb(10, 20, 30))])),
        (
            "effect_1",
            "Shadows/sm",
            StyleProperty::Effect,
            Style::Effect(vec![Effect::DropShadow {
                color: Color::rgba(0, 0, 0, 64),
                offset: Vec2::new(0.0, 2.0),
                radius: 4.0,
                spread: 0.0,
                visible: true,
                show_behind: false,
            }]),
        ),
    ] {
        d_no_desc.styles.insert(key.into(), style);
        d_no_desc.style_meta.insert(key.into(), StyleMeta { name: name.into(), property: prop, ..Default::default() });
    }
    let bytes_no_desc = export_fig(&d_no_desc).unwrap();
    let imported_no_desc = import_fig(&bytes_no_desc).unwrap();
    assert_eq!(imported_no_desc.style_meta.len(), 2);
    assert!(
        imported_no_desc.style_meta.values().all(|m| m.description.is_empty()),
        "a file whose styles have no description must round-trip with every description still empty: {:?}",
        imported_no_desc.style_meta
    );

    let mut d_with_desc = d_no_desc.clone();
    let mut h = History::default();
    let cmd = set_style_description(&d_with_desc, "fill_1", "Primary brand surface   ").unwrap();
    h.exec(&mut d_with_desc, cmd).unwrap();
    let bytes_with_desc = export_fig(&d_with_desc).unwrap();
    let imported_with_desc = import_fig(&bytes_with_desc).unwrap();
    let (_, fill_meta) = imported_with_desc.style_meta.iter().find(|(_, m)| m.name == "Brand/Primary").expect("Brand/Primary imported");
    let (_, fx_meta) = imported_with_desc.style_meta.iter().find(|(_, m)| m.name == "Shadows/sm").expect("Shadows/sm imported");
    assert_eq!(fill_meta.description, "Primary brand surface");
    assert_eq!(fx_meta.description, "");
}
