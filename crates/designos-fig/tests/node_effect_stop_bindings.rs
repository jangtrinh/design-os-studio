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
