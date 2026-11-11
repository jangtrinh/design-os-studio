//! Round-trip `.fig` export/import tests for node-level effect variable bindings
//! (`effects/{i}/color`, `effects/{i}/radius`, `effects/{i}/spread`, `effects/{i}/offsetX`,
//! `effects/{i}/offsetY`) and gradient stop colour variable bindings (`fills/{pi}/stops/{si}`,
//! `strokes/{pi}/stops/{si}`), including per-mode resolution across multiple variable modes.

use designos_doc::*;
use designos_fig::{export_fig, import_fig};

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

fn insert_var(doc: &mut Document, id: &str, name: &str, kind: VariableKind, scopes: &[&str], light: VariableValue, dark: VariableValue) {
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
            scopes: scopes.iter().map(|s| (*s).to_string()).collect(),
            hidden_from_publishing: false,
            key: None,
            code_syntax: Default::default(),
            extensions: Default::default(),
            mode_extensions: Default::default(),
        },
    );
}

fn find<'a>(doc: &'a Document, name: &str) -> &'a Node {
    doc.nodes.values().find(|n| n.name == name).unwrap_or_else(|| panic!("missing node {name}"))
}

#[test]
fn node_effect_and_gradient_stop_bindings_round_trip_and_resolve_per_mode() {
    let mut doc = Document::new("NodeEffectAndStopBindings");
    let page = doc.add_page("Page 1");
    add_two_mode_collection(&mut doc);

    let sh_color = "VariableID:3:30".to_string();
    let sh_radius = "VariableID:3:31".to_string();
    let sh_spread = "VariableID:3:32".to_string();
    let sh_ox = "VariableID:3:33".to_string();
    let sh_oy = "VariableID:3:34".to_string();
    let stop0_var = "VariableID:3:35".to_string();
    let stop1_var = "VariableID:3:36".to_string();
    let stroke_stop0_var = "VariableID:3:37".to_string();

    let sh_light = Color::rgba(0x10, 0x20, 0x30, 0x80);
    let sh_dark = Color::rgba(0xE0, 0xF0, 0xFF, 0xCC);
    let stop0_light = Color::rgb(0xFF, 0x22, 0x44);
    let stop0_dark = Color::rgb(0x22, 0xFF, 0x88);
    let stop1_light = Color::rgb(0x00, 0x44, 0xFF);
    let stop1_dark = Color::rgb(0xFF, 0xCC, 0x00);
    let stroke_light = Color::rgb(0xAA, 0x00, 0xFF);
    let stroke_dark = Color::rgb(0x00, 0xFF, 0xAA);

    insert_var(
        &mut doc,
        &sh_color,
        "effect/shadow-color",
        VariableKind::Color,
        &["EFFECT_COLOR"],
        VariableValue::Color(sh_light),
        VariableValue::Color(sh_dark),
    );
    insert_var(&mut doc, &sh_radius, "effect/shadow-radius", VariableKind::Number, &["EFFECT_FLOAT"], VariableValue::Number(6.0), VariableValue::Number(20.0));
    insert_var(&mut doc, &sh_spread, "effect/shadow-spread", VariableKind::Number, &["EFFECT_FLOAT"], VariableValue::Number(2.0), VariableValue::Number(8.0));
    insert_var(&mut doc, &sh_ox, "effect/shadow-x", VariableKind::Number, &["EFFECT_FLOAT"], VariableValue::Number(4.0), VariableValue::Number(12.0));
    insert_var(&mut doc, &sh_oy, "effect/shadow-y", VariableKind::Number, &["EFFECT_FLOAT"], VariableValue::Number(5.0), VariableValue::Number(15.0));
    insert_var(
        &mut doc,
        &stop0_var,
        "fill/grad-start",
        VariableKind::Color,
        &["ALL_FILLS"],
        VariableValue::Color(stop0_light),
        VariableValue::Color(stop0_dark),
    );
    insert_var(
        &mut doc,
        &stop1_var,
        "fill/grad-end",
        VariableKind::Color,
        &["SHAPE_FILL"],
        VariableValue::Color(stop1_light),
        VariableValue::Color(stop1_dark),
    );
    insert_var(
        &mut doc,
        &stroke_stop0_var,
        "stroke/grad-start",
        VariableKind::Color,
        &["STROKE_COLOR"],
        VariableValue::Color(stroke_light),
        VariableValue::Color(stroke_dark),
    );

    let frame_id = doc.alloc_id();
    let mut frame = Node::frame(frame_id, Rect::new(0.0, 0.0, 400.0, 300.0));
    frame.name = "ModeContainer".into();
    let frame_id = doc.insert(page, frame).unwrap();

    let card_id = doc.alloc_id();
    let mut card = Node::rectangle(card_id, Rect::new(20.0, 20.0, 200.0, 120.0));
    card.name = "BoundLayer".into();
    card.fills = vec![Paint {
        kind: PaintKind::LinearGradient {
            handles: [Vec2::new(0.0, 0.5), Vec2::new(1.0, 0.5), Vec2::new(0.0, 1.0)],
            stops: vec![GradientStop { pos: 0.0, color: stop0_light }, GradientStop { pos: 1.0, color: stop1_light }],
        },
        opacity: 1.0,
        visible: true,
        blend: BlendMode::Normal,
    }];
    card.stroke.paints = vec![Paint {
        kind: PaintKind::LinearGradient {
            handles: [Vec2::new(0.0, 0.5), Vec2::new(1.0, 0.5), Vec2::new(0.0, 1.0)],
            stops: vec![GradientStop { pos: 0.0, color: stroke_light }, GradientStop { pos: 1.0, color: Color::WHITE }],
        },
        opacity: 1.0,
        visible: true,
        blend: BlendMode::Normal,
    }];
    card.effects = vec![Effect::DropShadow {
        color: sh_light,
        offset: Vec2::new(4.0, 5.0),
        radius: 6.0,
        spread: 2.0,
        visible: true,
        show_behind: false,
        blend: BlendMode::Normal,
    }];
    card.variables.bindings.insert("effects/0/color".into(), sh_color.clone());
    card.variables.bindings.insert("effects/0/radius".into(), sh_radius.clone());
    card.variables.bindings.insert("effects/0/spread".into(), sh_spread.clone());
    card.variables.bindings.insert("effects/0/offsetX".into(), sh_ox.clone());
    card.variables.bindings.insert("effects/0/offsetY".into(), sh_oy.clone());
    card.variables.bindings.insert("fills/0/stops/0".into(), stop0_var.clone());
    card.variables.bindings.insert("fills/0/stops/1".into(), stop1_var.clone());
    card.variables.bindings.insert("strokes/0/stops/0".into(), stroke_stop0_var.clone());
    doc.insert(frame_id, card).unwrap();

    let bytes = export_fig(&doc).expect("export .fig");
    let mut doc2 = import_fig(&bytes).expect("import .fig");

    let imported = find(&doc2, "BoundLayer");
    assert_eq!(imported.variables.bindings.get("effects/0/color"), Some(&sh_color));
    assert_eq!(imported.variables.bindings.get("effects/0/radius"), Some(&sh_radius));
    assert_eq!(imported.variables.bindings.get("effects/0/spread"), Some(&sh_spread));
    assert_eq!(imported.variables.bindings.get("effects/0/offsetX"), Some(&sh_ox));
    assert_eq!(imported.variables.bindings.get("effects/0/offsetY"), Some(&sh_oy));
    assert_eq!(imported.variables.bindings.get("fills/0/stops/0"), Some(&stop0_var));
    assert_eq!(imported.variables.bindings.get("fills/0/stops/1"), Some(&stop1_var));
    assert_eq!(imported.variables.bindings.get("strokes/0/stops/0"), Some(&stroke_stop0_var));

    // Resolve in Light mode (default mode "3:10").
    let resolved_light = resolve_node(&doc2, imported);
    let Effect::DropShadow { color, offset, radius, spread, .. } = resolved_light.effects[0] else {
        panic!("expected DropShadow on resolved node");
    };
    assert_eq!(color, sh_light);
    assert_eq!(radius, 6.0);
    assert_eq!(spread, 2.0);
    assert_eq!(offset, Vec2::new(4.0, 5.0));
    let PaintKind::LinearGradient { stops: fill_stops_light, .. } = &resolved_light.fills[0].kind else {
        panic!("expected LinearGradient fill");
    };
    assert_eq!(fill_stops_light[0].color, stop0_light);
    assert_eq!(fill_stops_light[1].color, stop1_light);
    let PaintKind::LinearGradient { stops: stroke_stops_light, .. } = &resolved_light.stroke.paints[0].kind else {
        panic!("expected LinearGradient stroke");
    };
    assert_eq!(stroke_stops_light[0].color, stroke_light);

    // Switch container to Dark mode ("3:11") and verify every bound property resolves to Dark values.
    let container_id = find(&doc2, "ModeContainer").id;
    doc2.get_mut(container_id).unwrap().variables.modes.insert("VariableCollectionId:3:1".into(), "3:11".into());
    let imported = find(&doc2, "BoundLayer");
    let resolved_dark = resolve_node(&doc2, imported);
    let Effect::DropShadow { color, offset, radius, spread, .. } = resolved_dark.effects[0] else {
        panic!("expected DropShadow on resolved node");
    };
    assert_eq!(color, sh_dark);
    assert_eq!(radius, 20.0);
    assert_eq!(spread, 8.0);
    assert_eq!(offset, Vec2::new(12.0, 15.0));
    let PaintKind::LinearGradient { stops: fill_stops_dark, .. } = &resolved_dark.fills[0].kind else {
        panic!("expected LinearGradient fill");
    };
    assert_eq!(fill_stops_dark[0].color, stop0_dark);
    assert_eq!(fill_stops_dark[1].color, stop1_dark);
    let PaintKind::LinearGradient { stops: stroke_stops_dark, .. } = &resolved_dark.stroke.paints[0].kind else {
        panic!("expected LinearGradient stroke");
    };
    assert_eq!(stroke_stops_dark[0].color, stroke_dark);
}

#[test]
fn fig_roundtrip_preserves_glass_effect_variable_bindings_per_mode() {
    let mut doc = Document::new("GlassEffectBindings");
    let page = doc.add_page("Page 1");
    add_two_mode_collection(&mut doc);

    let v_depth = "VariableID:3:40".to_string();
    let v_angle = "VariableID:3:41".to_string();
    let v_intensity = "VariableID:3:42".to_string();
    let v_dispersion = "VariableID:3:43".to_string();
    let v_refraction = "VariableID:3:44".to_string();
    let v_frost = "VariableID:3:45".to_string();

    insert_var(&mut doc, &v_depth, "glass/depth", VariableKind::Number, &["EFFECT_FLOAT"], VariableValue::Number(4.0), VariableValue::Number(16.0));
    insert_var(&mut doc, &v_angle, "glass/angle", VariableKind::Number, &["EFFECT_FLOAT"], VariableValue::Number(30.0), VariableValue::Number(120.0));
    insert_var(&mut doc, &v_intensity, "glass/intensity", VariableKind::Number, &["EFFECT_FLOAT"], VariableValue::Number(40.0), VariableValue::Number(80.0));
    insert_var(&mut doc, &v_dispersion, "glass/dispersion", VariableKind::Number, &["EFFECT_FLOAT"], VariableValue::Number(15.0), VariableValue::Number(50.0));
    insert_var(&mut doc, &v_refraction, "glass/refraction", VariableKind::Number, &["EFFECT_FLOAT"], VariableValue::Number(20.0), VariableValue::Number(60.0));
    insert_var(&mut doc, &v_frost, "glass/frost", VariableKind::Number, &["EFFECT_FLOAT"], VariableValue::Number(10.0), VariableValue::Number(30.0));

    let frame_id = doc.alloc_id();
    let mut frame = Node::frame(frame_id, Rect::new(0.0, 0.0, 400.0, 300.0));
    frame.name = "ModeContainer".into();
    let frame_id = doc.insert(page, frame).unwrap();

    let card_id = doc.alloc_id();
    let mut card = Node::rectangle(card_id, Rect::new(20.0, 20.0, 200.0, 120.0));
    card.name = "GlassCard".into();
    card.effects = vec![Effect::Glass(GlassEffect {
        depth: 4.0,
        light_angle: 30.0,
        light_intensity: 0.4,
        dispersion: 0.15,
        refraction: 0.2,
        frost: 10.0,
        splay: 0.0,
        bevel: 0.0,
        visible: true,
    })];
    card.variables.bindings.insert("effects/0/depth".into(), v_depth.clone());
    card.variables.bindings.insert("effects/0/lightAngle".into(), v_angle.clone());
    card.variables.bindings.insert("effects/0/lightIntensity".into(), v_intensity.clone());
    card.variables.bindings.insert("effects/0/dispersion".into(), v_dispersion.clone());
    card.variables.bindings.insert("effects/0/refraction".into(), v_refraction.clone());
    card.variables.bindings.insert("effects/0/frost".into(), v_frost.clone());
    doc.insert(frame_id, card).unwrap();

    let bytes = export_fig(&doc).expect("export .fig");
    let mut doc2 = import_fig(&bytes).expect("import .fig");

    let imported = find(&doc2, "GlassCard");
    assert_eq!(imported.variables.bindings.get("effects/0/depth"), Some(&v_depth));
    assert_eq!(imported.variables.bindings.get("effects/0/lightAngle"), Some(&v_angle));
    assert_eq!(imported.variables.bindings.get("effects/0/lightIntensity"), Some(&v_intensity));
    assert_eq!(imported.variables.bindings.get("effects/0/dispersion"), Some(&v_dispersion));
    assert_eq!(imported.variables.bindings.get("effects/0/refraction"), Some(&v_refraction));
    assert_eq!(imported.variables.bindings.get("effects/0/radius"), Some(&v_frost), "the frost binding comes back as the glass blur radius");

    // Resolve in Light mode
    let resolved_light = resolve_node(&doc2, imported);
    let Effect::Glass(ref g_light) = resolved_light.effects[0] else { panic!("expected Glass") };
    assert_eq!(g_light.depth, 4.0);
    assert_eq!(g_light.light_angle, 30.0);
    assert!((g_light.light_intensity - 0.4).abs() < 1e-4);
    assert!((g_light.dispersion - 0.15).abs() < 1e-4);
    assert!((g_light.refraction - 0.2).abs() < 1e-4);
    assert_eq!(g_light.frost, 10.0);

    // Switch container to Dark mode
    let container_id = find(&doc2, "ModeContainer").id;
    doc2.get_mut(container_id).unwrap().variables.modes.insert("VariableCollectionId:3:1".into(), "3:11".into());
    let imported = find(&doc2, "GlassCard");
    let resolved_dark = resolve_node(&doc2, imported);
    let Effect::Glass(ref g_dark) = resolved_dark.effects[0] else { panic!("expected Glass") };
    assert_eq!(g_dark.depth, 16.0);
    assert_eq!(g_dark.light_angle, 120.0);
    assert!((g_dark.light_intensity - 0.8).abs() < 1e-4);
    assert!((g_dark.dispersion - 0.5).abs() < 1e-4);
    assert!((g_dark.refraction - 0.6).abs() < 1e-4);
    assert_eq!(g_dark.frost, 30.0);
}

fn test_glass_effect(frost: f32) -> GlassEffect {
    GlassEffect { frost, light_angle: 0.0, light_intensity: 0.0, refraction: 0.0, depth: 0.0, dispersion: 0.0, splay: 0.0, bevel: 0.0, visible: true }
}

#[test]
fn fig_roundtrip_preserves_overridden_instance_effect_and_gradient_stop_bindings() {
    let mut doc = Document::new("InstanceBindings");
    let page = doc.add_page("Page 1");
    add_two_mode_collection(&mut doc);

    let v_frost = "VariableID:3:50".to_string();
    let v_stop = "VariableID:3:51".to_string();
    insert_var(&mut doc, &v_frost, "glass/frost", VariableKind::Number, &["EFFECT_FLOAT"], VariableValue::Number(12.0), VariableValue::Number(24.0));
    insert_var(
        &mut doc,
        &v_stop,
        "fill/stop",
        VariableKind::Color,
        &["ALL_FILLS"],
        VariableValue::Color(Color::rgb(255, 0, 0)),
        VariableValue::Color(Color::rgb(0, 0, 255)),
    );

    // Create a component
    let comp_id = doc.alloc_id();
    let mut comp = Node::frame(comp_id, Rect::new(0.0, 0.0, 100.0, 100.0));
    comp.kind = NodeKind::Component(FrameProps::default());
    comp.name = "GlassComp".into();
    let comp_id = doc.insert(page, comp).unwrap();

    let inner_id = doc.alloc_id();
    let mut inner = Node::rectangle(inner_id, Rect::new(0.0, 0.0, 100.0, 100.0));
    inner.name = "InnerLayer".into();
    inner.effects = vec![Effect::Glass(test_glass_effect(5.0))];
    inner.fills = vec![Paint {
        kind: PaintKind::LinearGradient {
            handles: [Vec2::new(0.0, 0.5), Vec2::new(1.0, 0.5), Vec2::new(0.0, 1.0)],
            stops: vec![GradientStop { pos: 0.0, color: Color::WHITE }, GradientStop { pos: 1.0, color: Color::BLACK }],
        },
        opacity: 1.0,
        visible: true,
        blend: BlendMode::Normal,
    }];
    doc.insert(comp_id, inner).unwrap();

    // Create an instance of GlassComp
    let inst_id = doc.alloc_id();
    let mut inst = Node::frame(inst_id, Rect::new(120.0, 0.0, 100.0, 100.0));
    inst.kind = NodeKind::Instance { frame: FrameProps::default(), component: Some(comp_id), component_key: None };
    inst.name = "GlassInstance".into();
    let inst_id = doc.insert(page, inst).unwrap();

    // The expanded instance child layer
    let inst_child_id = doc.alloc_id();
    let mut inst_child = Node::rectangle(inst_child_id, Rect::new(0.0, 0.0, 100.0, 100.0));
    inst_child.name = "InnerLayer".into();
    inst_child.component.source = Some(inner_id);
    inst_child.effects = vec![Effect::Glass(test_glass_effect(12.0))];
    inst_child.fills = vec![Paint {
        kind: PaintKind::LinearGradient {
            handles: [Vec2::new(0.0, 0.5), Vec2::new(1.0, 0.5), Vec2::new(0.0, 1.0)],
            stops: vec![GradientStop { pos: 0.0, color: Color::rgb(255, 0, 0) }, GradientStop { pos: 1.0, color: Color::BLACK }],
        },
        opacity: 1.0,
        visible: true,
        blend: BlendMode::Normal,
    }];
    inst_child.variables.bindings.insert("effects/0/frost".into(), v_frost.clone());
    inst_child.variables.bindings.insert("fills/0/stops/0".into(), v_stop.clone());
    doc.insert(inst_id, inst_child).unwrap();

    let bytes = export_fig(&doc).expect("export .fig");
    let doc2 = import_fig(&bytes).expect("import .fig");

    // Check that the instance's inner layer preserved its effect and stop bindings
    let inst = find(&doc2, "GlassInstance");
    let child_id = inst.children.first().copied().expect("instance child");
    let child = doc2.get(child_id).expect("child exists");
    assert!(
        child.variables.bindings.get("effects/0/frost") == Some(&v_frost) || child.variables.bindings.get("effects/0/radius") == Some(&v_frost),
        "expected effect frost/radius variable binding on overridden instance child, got: {:?}",
        child.variables.bindings
    );
    assert_eq!(child.variables.bindings.get("fills/0/stops/0"), Some(&v_stop), "expected gradient stop variable binding on overridden instance child");
}
