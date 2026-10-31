//! Sim tests for #132 (inspector half): variable binding on node-level effect rows
//! (`effects/{i}/color`, `effects/{i}/radius`, `effects/{i}/spread`, `effects/{i}/offsetX`,
//! `effects/{i}/offsetY`) and on gradient stops (`fills/{pi}/stops/{si}`, `strokes/{pi}/stops/{si}`).

use super::*;
use designos_doc::{BlendMode, Effect, GradientStop, PaintKind, VariableKind, VariableValue};

fn theme_ids(app: &App) -> (String, String, String) {
    let c = app.doc.variable_collections.values().find(|c| c.name == "Theme").unwrap();
    let light = c.modes.iter().find(|m| m.name == "Light").unwrap().id.clone();
    let dark = c.modes.iter().find(|m| m.name == "Dark").unwrap().id.clone();
    (c.id.clone(), light, dark)
}

fn set_scopes(store: &mut designos_doc::VariableStore, id: &str, scopes: &[&str]) {
    store.variables.get_mut(id).unwrap().scopes = scopes.iter().map(|s| (*s).to_string()).collect();
}

fn has_test_rect(s: &Sim, name: &str) -> bool {
    s.ctx.data(|d| d.get_temp::<egui::Rect>(egui::Id::new(("test-rect", name.to_string())))).is_some()
}

fn fx_field_rect(s: &Sim, tag: &str, i: usize) -> Rect {
    let key = format!("{:?}", (tag, i));
    s.ctx.data(|d| d.get_temp::<Rect>(egui::Id::new(("field-rect", key.clone())))).unwrap_or_else(|| panic!("no field {key}"))
}

fn collect_texts_in_rect(shape: &egui::epaint::Shape, rect: Rect, out: &mut Vec<String>) {
    match shape {
        egui::epaint::Shape::Text(t) => {
            if rect.contains(t.pos) {
                out.push(t.galley.text().to_string());
            }
        }
        egui::epaint::Shape::Vec(shapes) => {
            for s in shapes {
                collect_texts_in_rect(s, rect, out);
            }
        }
        _ => {}
    }
}

fn texts_in_rect(s: &Sim, rect: Rect) -> Vec<String> {
    let mut out = Vec::new();
    for cs in &s.shapes {
        collect_texts_in_rect(&cs.shape, rect, &mut out);
    }
    out
}

#[test]
fn node_effect_rows_bind_filter_scopes_detach_and_undo() {
    let mut s = Sim::with(Options { select: Some("Hero".into()), ..Default::default() });
    let hero = s.by_name("Hero");

    let mut store = designos_doc::VariableStore::from_doc(&s.app().doc);
    let (coll, light, dark) = theme_ids(s.app());

    let fx_color = store.add_variable(&coll, "Effect/ShadowColor", VariableKind::Color).unwrap();
    store.set_value(&fx_color, &light, VariableValue::Color(Color::rgb(0x12, 0x34, 0x56)));
    store.set_value(&fx_color, &dark, VariableValue::Color(Color::rgb(0xAB, 0xCD, 0xEF)));
    set_scopes(&mut store, &fx_color, &["EFFECT_COLOR"]);

    let wrong_stroke_color = store.add_variable(&coll, "Stroke/OnlyColor", VariableKind::Color).unwrap();
    store.set_value(&wrong_stroke_color, &light, VariableValue::Color(Color::rgb(0x99, 0x00, 0x00)));
    set_scopes(&mut store, &wrong_stroke_color, &["STROKE_COLOR"]);

    let fx_radius = store.add_variable(&coll, "Effect/Radius", VariableKind::Number).unwrap();
    store.set_value(&fx_radius, &light, VariableValue::Number(18.0));
    store.set_value(&fx_radius, &dark, VariableValue::Number(28.0));
    set_scopes(&mut store, &fx_radius, &["EFFECT_FLOAT"]);

    let fx_spread = store.add_variable(&coll, "Effect/Spread", VariableKind::Number).unwrap();
    store.set_value(&fx_spread, &light, VariableValue::Number(6.0));
    set_scopes(&mut store, &fx_spread, &["EFFECT_FLOAT"]);

    let fx_ox = store.add_variable(&coll, "Effect/OffsetX", VariableKind::Number).unwrap();
    store.set_value(&fx_ox, &light, VariableValue::Number(7.0));
    set_scopes(&mut store, &fx_ox, &["EFFECT_FLOAT"]);

    let fx_oy = store.add_variable(&coll, "Effect/OffsetY", VariableKind::Number).unwrap();
    store.set_value(&fx_oy, &light, VariableValue::Number(11.0));
    set_scopes(&mut store, &fx_oy, &["EFFECT_FLOAT"]);

    let wrong_num = store.add_variable(&coll, "Layout/WidthOnly", VariableKind::Number).unwrap();
    store.set_value(&wrong_num, &light, VariableValue::Number(99.0));
    set_scopes(&mut store, &wrong_num, &["WIDTH_HEIGHT"]);

    s.shell.app.commit_store(store, &[], &[]);

    // Ensure Hero has a DropShadow at effects[0] and a LayerBlur at effects[1].
    s.shell.app.edit_nodes(&[hero], |n| {
        n.effects = vec![
            Effect::DropShadow {
                color: Color::rgba(0, 0, 0, 64),
                offset: designos_doc::Vec2::new(0.0, 4.0),
                radius: 8.0,
                spread: 0.0,
                visible: true,
                show_behind: false,
                blend: BlendMode::Normal,
            },
            Effect::LayerBlur { radius: 4.0, visible: true },
        ];
    });
    s.scroll_inspector_down();

    // Open the DropShadow popover (effects[0]).
    let minus0 = s.rect_of("fx-minus-0");
    s.click(Pos2::new(minus0.left() - 52.0, minus0.center().y));
    assert!(s.app().props.pop.is_some(), "effect settings popover should open");
    s.frame(vec![]);

    // 1. Bind shadow Blur radius (`effects/0/radius`).
    let blur_field = fx_field_rect(&s, "fxp-b", 0);
    s.mv(blur_field.center());
    s.frame(vec![]);
    s.click(s.rect_of("bind:effects/0/radius").center());
    s.frame(vec![]);
    assert!(!has_test_rect(&s, "pick:Layout/WidthOnly"), "WIDTH_HEIGHT variable must not be offered for EFFECT_FLOAT");
    s.click(s.rect_of("pick:Effect/Radius").center());
    s.frame(vec![]);
    assert_eq!(s.app().doc.get(hero).unwrap().variables.bindings.get("effects/0/radius"), Some(&fx_radius));
    let Effect::DropShadow { radius, .. } = s.app().doc.get(hero).unwrap().effects[0] else { panic!("expected DropShadow") };
    assert_eq!(radius, 18.0);
    s.rect_of("bound:effects/0/radius");

    // Undo removes the binding in one step and restores the original radius; redo restores it.
    s.cmd_key(Key::Z);
    assert!(!s.app().doc.get(hero).unwrap().variables.bindings.contains_key("effects/0/radius"));
    let Effect::DropShadow { radius, .. } = s.app().doc.get(hero).unwrap().effects[0] else { panic!("expected DropShadow") };
    assert_eq!(radius, 8.0, "undo must restore the original shadow radius in one step");
    s.mods = Modifiers::COMMAND | Modifiers::SHIFT;
    s.key(Key::Z);
    s.mods = Modifiers::NONE;
    assert_eq!(s.app().doc.get(hero).unwrap().variables.bindings.get("effects/0/radius"), Some(&fx_radius));
    let Effect::DropShadow { radius, .. } = s.app().doc.get(hero).unwrap().effects[0] else { panic!("expected DropShadow") };
    assert_eq!(radius, 18.0);

    // 2. Bind X offset (`effects/0/offsetX`), Y offset (`effects/0/offsetY`), and Spread (`effects/0/spread`).
    let x_field = fx_field_rect(&s, "fxp-x", 0);
    s.mv(x_field.center());
    s.frame(vec![]);
    s.click(s.rect_of("bind:effects/0/offsetX").center());
    s.frame(vec![]);
    assert!(!has_test_rect(&s, "pick:Layout/WidthOnly"));
    s.click(s.rect_of("pick:Effect/OffsetX").center());
    s.frame(vec![]);
    assert_eq!(s.app().doc.get(hero).unwrap().variables.bindings.get("effects/0/offsetX"), Some(&fx_ox));
    s.rect_of("bound:effects/0/offsetX");

    let y_field = fx_field_rect(&s, "fxp-y", 0);
    s.mv(y_field.center());
    s.frame(vec![]);
    s.click(s.rect_of("bind:effects/0/offsetY").center());
    s.frame(vec![]);
    s.click(s.rect_of("pick:Effect/OffsetY").center());
    s.frame(vec![]);
    assert_eq!(s.app().doc.get(hero).unwrap().variables.bindings.get("effects/0/offsetY"), Some(&fx_oy));
    s.rect_of("bound:effects/0/offsetY");

    let s_field = fx_field_rect(&s, "fxp-s", 0);
    s.mv(s_field.center());
    s.frame(vec![]);
    s.click(s.rect_of("bind:effects/0/spread").center());
    s.frame(vec![]);
    s.click(s.rect_of("pick:Effect/Spread").center());
    s.frame(vec![]);
    assert_eq!(s.app().doc.get(hero).unwrap().variables.bindings.get("effects/0/spread"), Some(&fx_spread));
    s.rect_of("bound:effects/0/spread");

    // 3. Bind shadow Color (`effects/0/color`), check wrong-scope exclusion, detach, and undo.
    let color_row_center = Pos2::new(s_field.center().x, s_field.center().y + 28.0);
    s.mv(color_row_center);
    s.frame(vec![]);
    s.click(s.rect_of("bind:effects/0/color").center());
    s.frame(vec![]);
    assert!(!has_test_rect(&s, "pick:Stroke/OnlyColor"), "STROKE_COLOR variable must not be offered for EFFECT_COLOR");
    s.click(s.rect_of("pick:Effect/ShadowColor").center());
    s.frame(vec![]);
    assert_eq!(s.app().doc.get(hero).unwrap().variables.bindings.get("effects/0/color"), Some(&fx_color));
    let Effect::DropShadow { color, offset, radius, spread, .. } = s.app().doc.get(hero).unwrap().effects[0] else {
        panic!("expected DropShadow");
    };
    assert_eq!(color, Color::rgb(0x12, 0x34, 0x56));
    assert_eq!(offset, designos_doc::Vec2::new(7.0, 11.0));
    assert_eq!(radius, 18.0);
    assert_eq!(spread, 6.0);

    let color_pill = s.rect_of("bound:effects/0/color");
    // Detach via the bound pill's detach button.
    s.click(Pos2::new(color_pill.right() - 13.0, color_pill.center().y));
    s.frame(vec![]);
    assert!(!s.app().doc.get(hero).unwrap().variables.bindings.contains_key("effects/0/color"));

    // Undo brings the colour binding back; another undo removes it again.
    s.cmd_key(Key::Z);
    assert_eq!(s.app().doc.get(hero).unwrap().variables.bindings.get("effects/0/color"), Some(&fx_color));
    s.cmd_key(Key::Z);
    assert!(!s.app().doc.get(hero).unwrap().variables.bindings.contains_key("effects/0/color"));

    // 4. Open LayerBlur popover (effects[1]) and bind `effects/1/radius`.
    s.shell.app.props.pop = None;
    s.frame(vec![]);
    let minus1 = s.rect_of("fx-minus-1");
    s.click(Pos2::new(minus1.left() - 52.0, minus1.center().y));
    s.frame(vec![]);
    let blur1_field = fx_field_rect(&s, "fxp-blur", 1);
    s.mv(blur1_field.center());
    s.frame(vec![]);
    s.click(s.rect_of("bind:effects/1/radius").center());
    s.frame(vec![]);
    s.click(s.rect_of("pick:Effect/Radius").center());
    s.frame(vec![]);
    assert_eq!(s.app().doc.get(hero).unwrap().variables.bindings.get("effects/1/radius"), Some(&fx_radius));
    s.rect_of("bound:effects/1/radius");
}

#[test]
fn gradient_stop_colors_bind_filter_scopes_detach_and_undo() {
    let mut s = Sim::with(Options { select: Some("Hero".into()), ..Default::default() });
    let hero = s.by_name("Hero");

    let mut store = designos_doc::VariableStore::from_doc(&s.app().doc);
    let (coll, light, _dark) = theme_ids(s.app());

    let fill_stop_var = store.add_variable(&coll, "Grad/StopStart", VariableKind::Color).unwrap();
    store.set_value(&fill_stop_var, &light, VariableValue::Color(Color::rgb(0xFF, 0x11, 0x22)));
    set_scopes(&mut store, &fill_stop_var, &["ALL_FILLS"]);

    let fill_stop1_var = store.add_variable(&coll, "Grad/StopEnd", VariableKind::Color).unwrap();
    store.set_value(&fill_stop1_var, &light, VariableValue::Color(Color::rgb(0x22, 0xAA, 0xFF)));
    set_scopes(&mut store, &fill_stop1_var, &["SHAPE_FILL"]);

    let wrong_frame_fill = store.add_variable(&coll, "Frame/OnlyStopColor", VariableKind::Color).unwrap();
    store.set_value(&wrong_frame_fill, &light, VariableValue::Color(Color::rgb(0x77, 0x88, 0x99)));
    set_scopes(&mut store, &wrong_frame_fill, &["FRAME_FILL"]);

    let wrong_fx_color = store.add_variable(&coll, "Effect/OnlyColor", VariableKind::Color).unwrap();
    store.set_value(&wrong_fx_color, &light, VariableValue::Color(Color::rgb(0x11, 0x22, 0x33)));
    set_scopes(&mut store, &wrong_fx_color, &["EFFECT_COLOR"]);

    let wrong_stroke_color = store.add_variable(&coll, "Stroke/OnlyStopColor", VariableKind::Color).unwrap();
    store.set_value(&wrong_stroke_color, &light, VariableValue::Color(Color::rgb(0x44, 0x55, 0x66)));
    set_scopes(&mut store, &wrong_stroke_color, &["STROKE_COLOR"]);

    s.shell.app.commit_store(store, &[], &[]);

    // Give Hero a two-stop linear gradient fill.
    s.shell.app.edit_nodes(&[hero], |n| {
        n.fills = vec![Paint {
            kind: PaintKind::LinearGradient {
                handles: [designos_doc::Vec2::new(0.0, 0.5), designos_doc::Vec2::new(1.0, 0.5), designos_doc::Vec2::new(0.0, 1.0)],
                stops: vec![GradientStop { pos: 0.0, color: Color::BLACK }, GradientStop { pos: 1.0, color: Color::WHITE }],
            },
            opacity: 1.0,
            visible: true,
            blend: BlendMode::Normal,
        }];
    });
    s.frame(vec![]);

    // Open the fill paint picker.
    s.click(s.rect_of("sw:false:0").center());
    s.frame(vec![]);
    let picker = s.rect_of("picker");

    // Hover the active gradient stop's colour row (`vy = y0 + 411.0`, height 24.0).
    let stop_color_row = Pos2::new(picker.left() + 100.0, picker.top() + 423.0);
    s.mv(stop_color_row);
    s.frame(vec![]);

    // Bind stop 0 (`fills/0/stops/0`).
    s.click(s.rect_of("bind:fills/0/stops/0").center());
    s.frame(vec![]);
    assert!(!has_test_rect(&s, "pick:Effect/OnlyColor"), "EFFECT_COLOR variable must not be offered for gradient fill stop");
    assert!(!has_test_rect(&s, "pick:Stroke/OnlyStopColor"), "STROKE_COLOR variable must not be offered for gradient fill stop");
    assert!(!has_test_rect(&s, "pick:Frame/OnlyStopColor"), "FRAME_FILL variable must not be offered for rectangle gradient fill stop");
    s.click(s.rect_of("pick:Grad/StopStart").center());
    s.frame(vec![]);

    assert_eq!(s.app().doc.get(hero).unwrap().variables.bindings.get("fills/0/stops/0"), Some(&fill_stop_var));
    let PaintKind::LinearGradient { stops, .. } = &s.app().doc.get(hero).unwrap().fills[0].kind else {
        panic!("expected LinearGradient");
    };
    assert_eq!(stops[0].color, Color::rgb(0xFF, 0x11, 0x22));
    s.rect_of("bound:fills/0/stops/0");

    // Undo removes the stop binding in one step and restores the original stop colour.
    s.cmd_key(Key::Z);
    assert!(!s.app().doc.get(hero).unwrap().variables.bindings.contains_key("fills/0/stops/0"));
    let PaintKind::LinearGradient { stops, .. } = &s.app().doc.get(hero).unwrap().fills[0].kind else {
        panic!("expected LinearGradient");
    };
    assert_eq!(stops[0].color, Color::BLACK);

    // Redo restores the stop 0 binding.
    s.mods = Modifiers::COMMAND | Modifiers::SHIFT;
    s.key(Key::Z);
    s.mods = Modifiers::NONE;
    assert_eq!(s.app().doc.get(hero).unwrap().variables.bindings.get("fills/0/stops/0"), Some(&fill_stop_var));
    let pill0 = s.rect_of("bound:fills/0/stops/0");

    // Detach stop 0 via the pill's detach button.
    s.click(Pos2::new(pill0.right() - 13.0, pill0.center().y));
    s.frame(vec![]);
    assert!(!s.app().doc.get(hero).unwrap().variables.bindings.contains_key("fills/0/stops/0"));

    // Select stop 1 on the gradient bar (`pos = 1.0` at `left + 168.0, top + 100.0`) and bind `fills/0/stops/1`.
    s.click(Pos2::new(picker.left() + 168.0, picker.top() + 100.0));
    s.frame(vec![]);
    s.mv(stop_color_row);
    s.frame(vec![]);
    s.click(s.rect_of("bind:fills/0/stops/1").center());
    s.frame(vec![]);
    s.click(s.rect_of("pick:Grad/StopEnd").center());
    s.frame(vec![]);
    assert_eq!(s.app().doc.get(hero).unwrap().variables.bindings.get("fills/0/stops/1"), Some(&fill_stop1_var));
    let PaintKind::LinearGradient { stops, .. } = &s.app().doc.get(hero).unwrap().fills[0].kind else {
        panic!("expected LinearGradient");
    };
    assert_eq!(stops[1].color, Color::rgb(0x22, 0xAA, 0xFF));
    s.rect_of("bound:fills/0/stops/1");

    // Also test a gradient stroke stop (`strokes/0/stops/0`) and STROKE_COLOR scope filtering.
    s.shell.app.props.picker = None;
    s.shell.app.edit_nodes(&[hero], |n| {
        n.stroke.paints = vec![Paint {
            kind: PaintKind::LinearGradient {
                handles: [designos_doc::Vec2::new(0.0, 0.5), designos_doc::Vec2::new(1.0, 0.5), designos_doc::Vec2::new(0.0, 1.0)],
                stops: vec![GradientStop { pos: 0.0, color: Color::BLACK }, GradientStop { pos: 1.0, color: Color::WHITE }],
            },
            opacity: 1.0,
            visible: true,
            blend: BlendMode::Normal,
        }];
        n.stroke.weight = 2.0;
    });
    s.frame(vec![]);
    s.click(s.rect_of("sw:true:0").center());
    s.frame(vec![]);
    let stroke_picker = s.rect_of("picker");
    let stroke_stop_row = Pos2::new(stroke_picker.left() + 100.0, stroke_picker.top() + 423.0);
    s.mv(stroke_stop_row);
    s.frame(vec![]);
    s.ctx.data_mut(|d| d.remove::<egui::Rect>(egui::Id::new(("test-rect", "pick:Grad/StopStart".to_string()))));
    s.click(s.rect_of("bind:strokes/0/stops/0").center());
    s.frame(vec![]);
    assert!(!has_test_rect(&s, "pick:Grad/StopStart"), "ALL_FILLS variable must not be offered for gradient stroke stop");
    assert!(!has_test_rect(&s, "pick:Effect/OnlyColor"), "EFFECT_COLOR variable must not be offered for gradient stroke stop");
    s.click(s.rect_of("pick:Stroke/OnlyStopColor").center());
    s.frame(vec![]);
    assert_eq!(s.app().doc.get(hero).unwrap().variables.bindings.get("strokes/0/stops/0"), Some(&wrong_stroke_color));
    let PaintKind::LinearGradient { stops, .. } = &s.app().doc.get(hero).unwrap().stroke.paints[0].kind else {
        panic!("expected LinearGradient");
    };
    assert_eq!(stops[0].color, Color::rgb(0x44, 0x55, 0x66));
    s.rect_of("bound:strokes/0/stops/0");
}

#[test]
fn effect_popover_opens_via_panel_flag_not_screenshot_chrome_side_effect() {
    let mut s = Sim::with(Options { select: Some("Hero".into()), ..Default::default() });
    let hero = s.by_name("Hero");
    let mut store = designos_doc::VariableStore::from_doc(&s.app().doc);
    let (coll, light, _) = theme_ids(s.app());
    let fx_radius = store.add_variable(&coll, "Effect/Radius", VariableKind::Number).unwrap();
    store.set_value(&fx_radius, &light, VariableValue::Number(18.0));
    s.shell.app.commit_store(store, &[], &[]);

    s.shell.app.edit_nodes(&[hero], |n| {
        n.effects = vec![Effect::DropShadow {
            color: Color::rgba(0, 0, 0, 64),
            offset: designos_doc::Vec2::new(0.0, 4.0),
            radius: 18.0,
            spread: 0.0,
            visible: true,
            show_behind: false,
            blend: BlendMode::Normal,
        }];
        n.variables.bindings.insert("effects/0/radius".into(), fx_radius.clone());
    });
    s.shell.app.ui.screenshot_chrome = true;
    s.shell.app.props.pop = None;
    s.scroll_inspector_down();
    assert!(s.app().props.pop.is_none(), "screenshot_chrome must not auto-open the effect popover inside effects::section");

    let opts = crate::cli::parse(["--select", "Hero", "--panel", "effect"].into_iter().map(String::from)).unwrap();
    let mut s_panel = Sim::with(opts);
    assert!(
        matches!(s_panel.app().props.pop.as_ref().map(|p| &p.kind), Some(crate::panels::effects::PopKind::Effect(0))),
        "--panel effect must open PopKind::Effect(0) on the selected node"
    );
    let _ = fx_field_rect(&s_panel, "fxp-b", 0);
    let panel_hero = s_panel.by_name("Hero");
    let mut store2 = designos_doc::VariableStore::from_doc(&s_panel.app().doc);
    let (coll2, light2, _) = theme_ids(s_panel.app());
    let fx_radius2 = store2.add_variable(&coll2, "Effect/Radius", VariableKind::Number).unwrap();
    store2.set_value(&fx_radius2, &light2, VariableValue::Number(18.0));
    s_panel.shell.app.commit_store(store2, &[], &[]);
    s_panel.shell.app.bind_variable(&[panel_hero], &["effects/0/radius"], &fx_radius2);
    s_panel.frame(vec![]);
    let _ = s_panel.rect_of("bound:effects/0/radius");
}

#[test]
fn bound_gradient_stop_locks_color_controls_until_detached() {
    let mut s = Sim::with(Options { select: Some("Hero".into()), ..Default::default() });
    let hero = s.by_name("Hero");

    let mut store = designos_doc::VariableStore::from_doc(&s.app().doc);
    let (coll, light, _) = theme_ids(s.app());
    let fill_stop_var = store.add_variable(&coll, "Grad/StopStart", VariableKind::Color).unwrap();
    store.set_value(&fill_stop_var, &light, VariableValue::Color(Color::rgb(0xFF, 0x11, 0x22)));
    set_scopes(&mut store, &fill_stop_var, &["ALL_FILLS"]);
    let donor_token_var = store.add_variable(&coll, "Token/Donor", VariableKind::Color).unwrap();
    store.set_value(&donor_token_var, &light, VariableValue::Color(Color::rgb(0x11, 0x88, 0xEE)));
    set_scopes(&mut store, &donor_token_var, &["ALL_FILLS"]);
    s.shell.app.commit_store(store, &[], &[]);

    let donor = crate::canvas::interact::hit_under(s.app(), s.app().view.to_page(s.app().canvas_rect.min, Pos2::new(500.0, 400.0)), true)
        .filter(|&id| id != hero)
        .unwrap_or_else(|| s.app().doc.nodes.values().find(|n| n.name == "Hero" && n.id != hero).unwrap().id);
    s.shell.app.bind_variable(&[donor], &["fills/0"], &donor_token_var);

    s.shell.app.edit_nodes(&[hero], |n| {
        n.fills = vec![Paint {
            kind: PaintKind::LinearGradient {
                handles: [designos_doc::Vec2::new(0.0, 0.5), designos_doc::Vec2::new(1.0, 0.5), designos_doc::Vec2::new(0.0, 1.0)],
                stops: vec![GradientStop { pos: 0.0, color: Color::BLACK }, GradientStop { pos: 1.0, color: Color::WHITE }],
            },
            opacity: 1.0,
            visible: true,
            blend: BlendMode::Normal,
        }];
    });
    s.frame(vec![]);

    // Open the fill paint picker and bind stop 0 (`fills/0/stops/0`).
    s.click(s.rect_of("sw:false:0").center());
    s.frame(vec![]);
    let picker = s.rect_of("picker");
    let stop_color_row = Pos2::new(picker.left() + 100.0, picker.top() + 423.0);
    s.mv(stop_color_row);
    s.frame(vec![]);
    s.click(s.rect_of("bind:fills/0/stops/0").center());
    s.frame(vec![]);
    s.click(s.rect_of("pick:Grad/StopStart").center());
    s.frame(vec![]);

    let bound_color = Color::rgb(0xFF, 0x11, 0x22);
    let before_node = s.app().doc.get(hero).unwrap().clone();
    assert_eq!(before_node.variables.bindings.get("fills/0/stops/0"), Some(&fill_stop_var));

    // 1. Clicking a swatch while stop 0 is bound leaves the document unchanged and the binding in place.
    let swatch1 = Pos2::new(picker.left() + 16.0 + 24.0 + 8.0, picker.top() + 496.0 + 8.0);
    s.click(swatch1);
    s.frame(vec![]);
    assert_eq!(s.app().doc.get(hero).unwrap(), &before_node, "clicking a swatch on a bound stop must not change the node");
    assert_eq!(s.app().doc.get(hero).unwrap().variables.bindings.get("fills/0/stops/0"), Some(&fill_stop_var));

    // 2. Clicking the SV area, hue slider, and alpha slider while bound also leaves the document unchanged.
    s.click(Pos2::new(picker.left() + 80.0, picker.top() + 200.0));
    s.click(Pos2::new(picker.left() + 140.0, picker.top() + 363.0));
    s.click(Pos2::new(picker.left() + 100.0, picker.top() + 391.0));
    s.frame(vec![]);
    assert_eq!(s.app().doc.get(hero).unwrap(), &before_node, "SV/hue/alpha controls must be locked while stop is bound");

    // 3. Eyedropper button click is disabled on a bound stop; if armed on an unbound stop before
    // switching to the bound stop, sampling the canvas closes the picker without writing to the bound stop.
    s.click(Pos2::new(picker.left() + 28.0, picker.top() + 377.0));
    s.frame(vec![]);
    assert!(!s.app().props.picker.as_ref().unwrap().eyedrop, "eyedropper button must be disabled when stop is bound");
    assert!(s.app().props.eyedropper.is_none());
    s.click(Pos2::new(picker.left() + 168.0, picker.top() + 100.0));
    s.frame(vec![]);
    s.click(Pos2::new(picker.left() + 28.0, picker.top() + 377.0));
    s.frame(vec![]);
    assert!(s.app().props.picker.as_ref().unwrap().eyedrop, "eyedropper button must arm on unbound stop 1");
    s.click(Pos2::new(picker.left() + 16.0, picker.top() + 100.0));
    s.frame(vec![]);
    assert_eq!(s.app().props.picker.as_ref().unwrap().state.sel, 0);
    assert!(s.app().props.picker.as_ref().unwrap().eyedrop);
    s.click(Pos2::new(500.0, 400.0));
    s.frame(vec![]);
    assert_eq!(s.app().doc.get(hero).unwrap(), &before_node, "eyedropper canvas sample must not write to a bound stop");
    assert!(s.app().props.picker.is_none(), "canvas click after eyedropper closes the picker as on base");

    // Re-open the fill picker and verify Shift+eyedropper token sampling on the canvas is also blocked on a bound stop.
    s.click(s.rect_of("sw:false:0").center());
    s.frame(vec![]);
    let picker = s.rect_of("picker");
    s.click(Pos2::new(picker.left() + 168.0, picker.top() + 100.0));
    s.frame(vec![]);
    s.click(Pos2::new(picker.left() + 28.0, picker.top() + 377.0));
    s.frame(vec![]);
    assert!(s.app().props.picker.as_ref().unwrap().eyedrop);
    s.click(Pos2::new(picker.left() + 16.0, picker.top() + 100.0));
    s.frame(vec![]);
    assert_eq!(s.app().props.picker.as_ref().unwrap().state.sel, 0);
    s.shell.app.hover = Some(donor);
    s.mods = Modifiers::SHIFT;
    s.click(Pos2::new(500.0, 400.0));
    s.mods = Modifiers::NONE;
    s.frame(vec![]);
    assert_eq!(s.app().doc.get(hero).unwrap(), &before_node, "Shift+eyedropper token sample must not write to or rebind a bound stop");
    assert!(s.app().props.picker.is_none());

    // Re-open the fill picker for step 4.
    s.click(s.rect_of("sw:false:0").center());
    s.frame(vec![]);
    let picker = s.rect_of("picker");

    // 4. The bound stop's position on the gradient bar stays editable.
    let stop0_handle = Pos2::new(picker.left() + 16.0, picker.top() + 100.0);
    let stop0_dragged = Pos2::new(picker.left() + 54.0, picker.top() + 100.0);
    s.drag(stop0_handle, stop0_dragged);
    s.frame(vec![]);
    let PaintKind::LinearGradient { stops, .. } = &s.app().doc.get(hero).unwrap().fills[0].kind else {
        panic!("expected LinearGradient");
    };
    assert!((stops[0].pos - 0.25).abs() < 0.05, "bound stop position must remain draggable, got {}", stops[0].pos);
    assert_eq!(stops[0].color, bound_color);
    assert_eq!(s.app().doc.get(hero).unwrap().variables.bindings.get("fills/0/stops/0"), Some(&fill_stop_var));

    // 5. After detaching via the pill's detach control, clicking a swatch writes to the stop.
    let pill0 = s.rect_of("bound:fills/0/stops/0");
    s.click(Pos2::new(pill0.right() - 13.0, pill0.center().y));
    s.frame(vec![]);
    assert!(!s.app().doc.get(hero).unwrap().variables.bindings.contains_key("fills/0/stops/0"));
    s.click(swatch1);
    s.frame(vec![]);
    let PaintKind::LinearGradient { stops, .. } = &s.app().doc.get(hero).unwrap().fills[0].kind else {
        panic!("expected LinearGradient");
    };
    assert_ne!(stops[0].color, bound_color, "swatch click must write to the stop after detaching");
}

#[test]
fn gradient_stop_bindings_follow_stop_across_flip_add_and_delete() {
    let mut s = Sim::with(Options { select: Some("Hero".into()), ..Default::default() });
    let hero = s.by_name("Hero");

    let mut store = designos_doc::VariableStore::from_doc(&s.app().doc);
    let (coll, light, _) = theme_ids(s.app());
    let grad_start = store.add_variable(&coll, "Grad/Start", VariableKind::Color).unwrap();
    store.set_value(&grad_start, &light, VariableValue::Color(Color::rgb(0xFF, 0x11, 0x22)));
    set_scopes(&mut store, &grad_start, &["ALL_FILLS"]);

    let grad_end = store.add_variable(&coll, "Grad/End", VariableKind::Color).unwrap();
    store.set_value(&grad_end, &light, VariableValue::Color(Color::rgb(0x22, 0xAA, 0xFF)));
    set_scopes(&mut store, &grad_end, &["ALL_FILLS"]);

    let stroke_var = store.add_variable(&coll, "Stroke/Stop", VariableKind::Color).unwrap();
    store.set_value(&stroke_var, &light, VariableValue::Color(Color::rgb(0x33, 0xCC, 0x66)));
    set_scopes(&mut store, &stroke_var, &["STROKE_COLOR"]);
    s.shell.app.commit_store(store, &[], &[]);

    let start_col = Color::rgb(0xFF, 0x11, 0x22);
    let end_col = Color::rgb(0x22, 0xAA, 0xFF);
    let stroke_col = Color::rgb(0x33, 0xCC, 0x66);

    s.shell.app.edit_nodes(&[hero], |n| {
        n.fills = vec![Paint {
            kind: PaintKind::LinearGradient {
                handles: [designos_doc::Vec2::new(0.0, 0.5), designos_doc::Vec2::new(1.0, 0.5), designos_doc::Vec2::new(0.0, 1.0)],
                stops: vec![GradientStop { pos: 0.0, color: Color::BLACK }, GradientStop { pos: 1.0, color: Color::WHITE }],
            },
            opacity: 1.0,
            visible: true,
            blend: BlendMode::Normal,
        }];
    });
    s.frame(vec![]);

    let resolved_fill_stops = |sim: &Sim| -> Vec<GradientStop> {
        let resolved = designos_doc::resolve_node(&sim.app().doc, sim.app().doc.get(hero).unwrap());
        let PaintKind::LinearGradient { stops, .. } = &resolved.fills[0].kind else {
            panic!("expected LinearGradient");
        };
        stops.clone()
    };

    // Open fill picker and bind stop 0 (`pos = 0.0`) to `Grad/Start`.
    s.click(s.rect_of("sw:false:0").center());
    s.frame(vec![]);
    let picker = s.rect_of("picker");
    let stop_color_row = Pos2::new(picker.left() + 100.0, picker.top() + 423.0);
    s.mv(stop_color_row);
    s.frame(vec![]);
    s.click(s.rect_of("bind:fills/0/stops/0").center());
    s.frame(vec![]);
    s.click(s.rect_of("pick:Grad/Start").center());
    s.frame(vec![]);

    assert_eq!(resolved_fill_stops(&s), vec![GradientStop { pos: 0.0, color: start_col }, GradientStop { pos: 1.0, color: Color::WHITE },]);

    // 1. Flip: stop 0 (bound to Grad/Start) moves to pos = 1.0 (index 1); unbound white stop moves to pos = 0.0 (index 0).
    s.click(Pos2::new(picker.left() + 184.0, picker.top() + 100.0));
    s.frame(vec![]);
    assert_eq!(s.app().doc.get(hero).unwrap().variables.bindings.get("fills/0/stops/0"), None);
    assert_eq!(s.app().doc.get(hero).unwrap().variables.bindings.get("fills/0/stops/1"), Some(&grad_start));
    assert_eq!(resolved_fill_stops(&s), vec![GradientStop { pos: 0.0, color: Color::WHITE }, GradientStop { pos: 1.0, color: start_col },]);

    // Undo restores both the stop order and the binding on `fills/0/stops/0` in one step.
    s.cmd_key(Key::Z);
    assert_eq!(s.app().doc.get(hero).unwrap().variables.bindings.get("fills/0/stops/0"), Some(&grad_start));
    assert_eq!(s.app().doc.get(hero).unwrap().variables.bindings.get("fills/0/stops/1"), None);
    assert_eq!(resolved_fill_stops(&s), vec![GradientStop { pos: 0.0, color: start_col }, GradientStop { pos: 1.0, color: Color::WHITE },]);

    // Bind stop 1 (`pos = 1.0`) to `Grad/End` as well.
    s.click(Pos2::new(picker.left() + 168.0, picker.top() + 100.0));
    s.frame(vec![]);
    s.mv(stop_color_row);
    s.frame(vec![]);
    s.click(s.rect_of("bind:fills/0/stops/1").center());
    s.frame(vec![]);
    s.click(s.rect_of("pick:Grad/End").center());
    s.frame(vec![]);
    assert_eq!(s.app().doc.get(hero).unwrap().variables.bindings.get("fills/0/stops/1"), Some(&grad_end));

    // 2. Add stop at pos = 0.5 (clicking midpoint of gradient bar): stop 1 shifts to index 2 (`fills/0/stops/2`).
    let bar_mid = Pos2::new(picker.left() + 16.0 + 76.0, picker.top() + 100.0);
    s.click(bar_mid);
    s.frame(vec![]);
    let stops_after_add = resolved_fill_stops(&s);
    assert_eq!(stops_after_add.len(), 3);
    assert_eq!(s.app().doc.get(hero).unwrap().variables.bindings.get("fills/0/stops/0"), Some(&grad_start));
    assert_eq!(s.app().doc.get(hero).unwrap().variables.bindings.get("fills/0/stops/1"), None);
    assert_eq!(s.app().doc.get(hero).unwrap().variables.bindings.get("fills/0/stops/2"), Some(&grad_end));
    assert_eq!(stops_after_add[0].color, start_col);
    assert_eq!(stops_after_add[2].color, end_col);

    // Undo removes the added stop and shifts `fills/0/stops/2` back to `fills/0/stops/1` in one step.
    s.cmd_key(Key::Z);
    assert_eq!(resolved_fill_stops(&s).len(), 2);
    assert_eq!(s.app().doc.get(hero).unwrap().variables.bindings.get("fills/0/stops/0"), Some(&grad_start));
    assert_eq!(s.app().doc.get(hero).unwrap().variables.bindings.get("fills/0/stops/1"), Some(&grad_end));
    assert_eq!(s.app().doc.get(hero).unwrap().variables.bindings.get("fills/0/stops/2"), None);
    assert_eq!(resolved_fill_stops(&s), vec![GradientStop { pos: 0.0, color: start_col }, GradientStop { pos: 1.0, color: end_col },]);

    // 3. Delete stop: add the midpoint stop back, detach stop 0, bind middle stop 1 to `Grad/Start`, then delete stop 1.
    s.click(bar_mid);
    s.frame(vec![]);
    s.click(Pos2::new(picker.left() + 16.0, picker.top() + 100.0));
    s.frame(vec![]);
    let pill0 = s.rect_of("bound:fills/0/stops/0");
    s.click(Pos2::new(pill0.right() - 13.0, pill0.center().y));
    s.frame(vec![]);
    s.click(bar_mid);
    s.frame(vec![]);
    s.mv(stop_color_row);
    s.frame(vec![]);
    s.click(s.rect_of("bind:fills/0/stops/1").center());
    s.frame(vec![]);
    s.click(s.rect_of("pick:Grad/Start").center());
    s.frame(vec![]);
    assert_eq!(s.app().doc.get(hero).unwrap().variables.bindings.get("fills/0/stops/1"), Some(&grad_start));
    assert_eq!(s.app().doc.get(hero).unwrap().variables.bindings.get("fills/0/stops/2"), Some(&grad_end));

    // Delete selected middle stop 1 by dragging it >30px away from the gradient bar:
    // stop 1's binding is removed and stop 2 shifts to `fills/0/stops/1`.
    s.drag(bar_mid, Pos2::new(bar_mid.x, bar_mid.y + 40.0));
    s.frame(vec![]);
    let stops_after_del = resolved_fill_stops(&s);
    assert_eq!(stops_after_del.len(), 2);
    assert_eq!(s.app().doc.get(hero).unwrap().variables.bindings.get("fills/0/stops/0"), None);
    assert_eq!(s.app().doc.get(hero).unwrap().variables.bindings.get("fills/0/stops/1"), Some(&grad_end));
    assert_eq!(s.app().doc.get(hero).unwrap().variables.bindings.get("fills/0/stops/2"), None);
    assert_eq!(stops_after_del[1].color, end_col);

    // Undo restores the deleted middle stop and both bindings (`fills/0/stops/1` and `fills/0/stops/2`) together.
    s.cmd_key(Key::Z);
    let stops_restored = resolved_fill_stops(&s);
    assert_eq!(stops_restored.len(), 3);
    assert_eq!(s.app().doc.get(hero).unwrap().variables.bindings.get("fills/0/stops/1"), Some(&grad_start));
    assert_eq!(s.app().doc.get(hero).unwrap().variables.bindings.get("fills/0/stops/2"), Some(&grad_end));
    assert_eq!(stops_restored[1].color, start_col);
    assert_eq!(stops_restored[2].color, end_col);

    // 4. Stroke gradient stop bindings also remap across flip and restore on undo.
    s.shell.app.props.picker = None;
    s.shell.app.edit_nodes(&[hero], |n| {
        n.stroke.paints = vec![Paint {
            kind: PaintKind::LinearGradient {
                handles: [designos_doc::Vec2::new(0.0, 0.5), designos_doc::Vec2::new(1.0, 0.5), designos_doc::Vec2::new(0.0, 1.0)],
                stops: vec![GradientStop { pos: 0.0, color: Color::BLACK }, GradientStop { pos: 1.0, color: Color::WHITE }],
            },
            opacity: 1.0,
            visible: true,
            blend: BlendMode::Normal,
        }];
        n.stroke.weight = 2.0;
    });
    s.frame(vec![]);
    s.click(s.rect_of("sw:true:0").center());
    s.frame(vec![]);
    let sp = s.rect_of("picker");
    s.mv(Pos2::new(sp.left() + 100.0, sp.top() + 423.0));
    s.frame(vec![]);
    s.click(s.rect_of("bind:strokes/0/stops/0").center());
    s.frame(vec![]);
    s.click(s.rect_of("pick:Stroke/Stop").center());
    s.frame(vec![]);
    s.click(Pos2::new(sp.left() + 184.0, sp.top() + 100.0));
    s.frame(vec![]);
    assert_eq!(s.app().doc.get(hero).unwrap().variables.bindings.get("strokes/0/stops/0"), None);
    assert_eq!(s.app().doc.get(hero).unwrap().variables.bindings.get("strokes/0/stops/1"), Some(&stroke_var));
    let resolved_stroke = designos_doc::resolve_node(&s.app().doc, s.app().doc.get(hero).unwrap());
    let PaintKind::LinearGradient { stops: s_stops, .. } = &resolved_stroke.stroke.paints[0].kind else {
        panic!("expected stroke LinearGradient");
    };
    assert_eq!(s_stops[0].color, Color::WHITE);
    assert_eq!(s_stops[1].color, stroke_col);
    s.cmd_key(Key::Z);
    assert_eq!(s.app().doc.get(hero).unwrap().variables.bindings.get("strokes/0/stops/0"), Some(&stroke_var));
    assert_eq!(s.app().doc.get(hero).unwrap().variables.bindings.get("strokes/0/stops/1"), None);
}

#[test]
fn picker_resync_runs_in_picker_popup_not_unrelated_sections() {
    let mut s = Sim::with(Options { select: Some("Hero".into()), ..Default::default() });
    let hero = s.by_name("Hero");

    s.shell.app.edit_nodes(&[hero], |n| {
        n.fills = vec![Paint {
            kind: PaintKind::LinearGradient {
                handles: [designos_doc::Vec2::new(0.0, 0.5), designos_doc::Vec2::new(1.0, 0.5), designos_doc::Vec2::new(0.0, 1.0)],
                stops: vec![GradientStop { pos: 0.0, color: Color::BLACK }, GradientStop { pos: 1.0, color: Color::WHITE }],
            },
            opacity: 1.0,
            visible: true,
            blend: BlendMode::Normal,
        }];
    });
    s.frame(vec![]);

    s.click(s.rect_of("sw:false:0").center());
    s.frame(vec![]);

    // Update the stop colour in the document while `pk.tx` is untouched.
    let updated = Color::rgb(0x12, 0x34, 0x56);
    if let Some(n) = s.shell.app.doc.get_mut(hero) {
        if let PaintKind::LinearGradient { stops, .. } = &mut n.fills[0].kind {
            stops[0].color = updated;
        }
    }

    // Calling `effects::section` or `var_bind::bind_picker` when no bind picker is open must not reach into the colour picker.
    crate::panels::effects::section_for_test(&mut s.shell.app, &s.ctx, &[hero], &[]);
    let PaintKind::LinearGradient { stops, .. } = &s.app().props.picker.as_ref().unwrap().paint.kind else {
        panic!("expected LinearGradient");
    };
    assert_eq!(stops[0].color, Color::BLACK, "effects::section must not resync the colour picker");

    crate::panels::var_bind::bind_picker(&mut s.shell.app, &s.ctx);
    let PaintKind::LinearGradient { stops, .. } = &s.app().props.picker.as_ref().unwrap().paint.kind else {
        panic!("expected LinearGradient");
    };
    assert_eq!(stops[0].color, Color::BLACK, "var_bind::bind_picker exit must not resync the colour picker");

    // Running a frame runs `properties::picker_popup`, which owns the colour picker resync.
    s.frame(vec![]);
    let PaintKind::LinearGradient { stops, .. } = &s.app().props.picker.as_ref().unwrap().paint.kind else {
        panic!("expected LinearGradient");
    };
    assert_eq!(stops[0].color, updated, "properties::picker_popup must resync the colour picker from the document");
}

#[test]
fn probe_stop_bind_after_a_picker_edit_survives_the_next_picker_edit() {
    let mut s = Sim::with(Options { select: Some("Hero".into()), ..Default::default() });
    let hero = s.by_name("Hero");
    let mut store = designos_doc::VariableStore::from_doc(&s.app().doc);
    let (coll, light, _dark) = theme_ids(s.app());
    let v = store.add_variable(&coll, "Probe/Red", VariableKind::Color).unwrap();
    store.set_value(&v, &light, VariableValue::Color(Color::rgb(0xFF, 0x11, 0x22)));
    s.shell.app.commit_store(store, &[], &[]);
    s.shell.app.edit_nodes(&[hero], |n| {
        n.fills = vec![Paint {
            kind: PaintKind::LinearGradient {
                handles: [designos_doc::Vec2::new(0.0, 0.5), designos_doc::Vec2::new(1.0, 0.5), designos_doc::Vec2::new(0.0, 1.0)],
                stops: vec![GradientStop { pos: 0.0, color: Color::BLACK }, GradientStop { pos: 1.0, color: Color::WHITE }],
            },
            opacity: 1.0,
            visible: true,
            blend: BlendMode::Normal,
        }];
    });
    s.frame(vec![]);
    s.click(s.rect_of("sw:false:0").center());
    s.frame(vec![]);
    let picker = s.rect_of("picker");
    let swatch0 = Pos2::new(picker.left() + 24.0, picker.top() + 504.0);
    // Stop 1: edit its colour through a swatch, so the picker transaction is live.
    s.click(Pos2::new(picker.left() + 168.0, picker.top() + 100.0));
    s.frame(vec![]);
    s.click(swatch0);
    s.frame(vec![]);
    let touched = !s.app().props.picker.as_ref().unwrap().tx.ed.touched().is_empty();
    // Stop 0: bind it.
    s.click(Pos2::new(picker.left() + 16.0, picker.top() + 100.0));
    s.frame(vec![]);
    s.mv(Pos2::new(picker.left() + 100.0, picker.top() + 423.0));
    s.frame(vec![]);
    s.click(s.rect_of("bind:fills/0/stops/0").center());
    s.frame(vec![]);
    s.click(s.rect_of("pick:Probe/Red").center());
    s.frame(vec![]);
    let bound_after_pick = s.app().doc.get(hero).unwrap().variables.bindings.get("fills/0/stops/0").cloned();
    // Stop 1 again: one more picker edit.
    s.click(Pos2::new(picker.left() + 168.0, picker.top() + 100.0));
    s.frame(vec![]);
    s.click(swatch0);
    s.frame(vec![]);
    let bound_after_edit = s.app().doc.get(hero).unwrap().variables.bindings.get("fills/0/stops/0").cloned();
    assert!(touched, "guard: the first swatch click opened a live picker transaction");
    assert_eq!(bound_after_pick.as_ref(), Some(&v));
    assert_eq!(bound_after_edit.as_ref(), Some(&v), "the binding survives the next picker edit");
}

#[test]
fn multiselect_bound_gradient_stop_locks_all_selected_layers_and_shows_mixed_pill() {
    let mut s = Sim::with(Options { select: Some("Hero".into()), ..Default::default() });
    let mut store = designos_doc::VariableStore::from_doc(&s.app().doc);
    let (coll, light, _dark) = theme_ids(s.app());
    let v_red = store.add_variable(&coll, "Probe/Red", VariableKind::Color).unwrap();
    store.set_value(&v_red, &light, VariableValue::Color(Color::rgb(0xFF, 0x11, 0x22)));
    set_scopes(&mut store, &v_red, &["ALL_FILLS"]);
    let v_blue = store.add_variable(&coll, "Probe/Blue", VariableKind::Color).unwrap();
    store.set_value(&v_blue, &light, VariableValue::Color(Color::rgb(0x22, 0x66, 0xFF)));
    set_scopes(&mut store, &v_blue, &["ALL_FILLS"]);
    s.shell.app.commit_store(store, &[], &[]);

    let red = Color::rgb(0xFF, 0x11, 0x22);
    let blue = Color::rgb(0x22, 0x66, 0xFF);
    let mut heroes: Vec<NodeId> = s.app().doc.nodes.values().filter(|n| n.name == "Hero").map(|n| n.id).collect();
    heroes.sort();
    assert!(heroes.len() >= 2, "fixture has two Hero layers");
    let (a, b) = (heroes[0], heroes[1]);

    let two_stop_grad = || Paint {
        kind: PaintKind::LinearGradient {
            handles: [designos_doc::Vec2::new(0.0, 0.5), designos_doc::Vec2::new(1.0, 0.5), designos_doc::Vec2::new(0.0, 1.0)],
            stops: vec![GradientStop { pos: 0.0, color: Color::BLACK }, GradientStop { pos: 1.0, color: Color::WHITE }],
        },
        opacity: 1.0,
        visible: true,
        blend: BlendMode::Normal,
    };
    let stops_of = |p: &Paint| -> Vec<(f32, Color)> {
        match &p.kind {
            PaintKind::LinearGradient { stops, .. } => stops.iter().map(|st| (st.pos, st.color)).collect(),
            _ => vec![],
        }
    };

    // 1. Layer `a` is unbound at stop 0; layer `b` binds `fills/0/stops/0` to `Probe/Red`.
    s.shell.app.edit_nodes(&[a, b], |n| n.fills = vec![two_stop_grad()]);
    s.shell.app.bind_variable(&[b], &["fills/0/stops/0"], &v_red);
    s.shell.app.selection = vec![a, b];
    s.frame(vec![]);
    s.frame(vec![]);

    s.click(s.rect_of("sw:false:0").center());
    s.frame(vec![]);
    let picker = s.rect_of("picker");
    s.click(Pos2::new(picker.left() + 16.0, picker.top() + 100.0));
    s.frame(vec![]);

    // Clicking a swatch while stop 0 is selected does not write a raw colour to `b` (or `a`).
    s.click(Pos2::new(picker.left() + 24.0, picker.top() + 504.0));
    s.frame(vec![]);
    let nb = s.app().doc.get(b).unwrap();
    let b_raw = stops_of(&nb.fills[0]);
    let b_res = stops_of(&designos_doc::resolve_node(&s.app().doc, nb).fills[0]);
    assert_eq!(nb.variables.bindings.get("fills/0/stops/0"), Some(&v_red));
    assert_eq!(b_raw[0].1, b_res[0].1, "swatch must not write a raw colour into b's bound stop");
    assert_eq!(b_raw[0].1, red);
    assert_eq!(stops_of(&s.app().doc.get(a).unwrap().fills[0])[0].1, Color::BLACK);

    // Because `a` is unbound and `b` is bound, the pill shows "Mixed".
    let pill_rect = s.rect_of("bound:fills/0/stops/0");
    assert_eq!(texts_in_rect(&s, pill_rect), vec!["Mixed".to_string()]);

    // Editing unbound stop 1 on the multi-selection also preserves `b`'s bound stop 0 raw colour.
    s.click(Pos2::new(picker.left() + 168.0, picker.top() + 100.0));
    s.frame(vec![]);
    s.click(Pos2::new(picker.left() + 24.0, picker.top() + 504.0));
    s.frame(vec![]);
    assert_eq!(stops_of(&s.app().doc.get(b).unwrap().fills[0])[0].1, red);
    s.click(Pos2::new(picker.left() + 16.0, picker.top() + 100.0));
    s.frame(vec![]);

    // 2. When `a` binds `Probe/Blue` and `b` binds `Probe/Red` at stop 0, the pill still shows "Mixed" and stays locked.
    s.shell.app.bind_variable(&[a], &["fills/0/stops/0"], &v_blue);
    s.frame(vec![]);
    let pill_rect = s.rect_of("bound:fills/0/stops/0");
    assert_eq!(texts_in_rect(&s, pill_rect), vec!["Mixed".to_string()]);
    s.click(Pos2::new(picker.left() + 24.0, picker.top() + 504.0));
    s.frame(vec![]);
    assert_eq!(stops_of(&s.app().doc.get(a).unwrap().fills[0])[0].1, blue);
    assert_eq!(stops_of(&s.app().doc.get(b).unwrap().fills[0])[0].1, red);

    // 3. When both `a` and `b` bind the same variable `Probe/Red`, the pill shows `Probe/Red`.
    s.shell.app.bind_variable(&[a], &["fills/0/stops/0"], &v_red);
    s.frame(vec![]);
    let pill_rect = s.rect_of("bound:fills/0/stops/0");
    assert_eq!(texts_in_rect(&s, pill_rect), vec!["Probe/Red".to_string()]);
}

#[test]
fn deleting_effect_drops_its_bindings_and_shifts_later_effect_bindings_down() {
    let mut s = Sim::with(Options { select: Some("Hero".into()), ..Default::default() });
    let hero = s.by_name("Hero");
    let mut store = designos_doc::VariableStore::from_doc(&s.app().doc);
    let (coll, light, _dark) = theme_ids(s.app());
    let v_blur0 = store.add_variable(&coll, "Probe/Blur", VariableKind::Number).unwrap();
    store.set_value(&v_blur0, &light, VariableValue::Number(18.0));
    let v_blur1 = store.add_variable(&coll, "Probe/BlurSecond", VariableKind::Number).unwrap();
    store.set_value(&v_blur1, &light, VariableValue::Number(12.0));
    s.shell.app.commit_store(store, &[], &[]);

    let shadow = |radius: f32| Effect::DropShadow {
        color: Color::rgba(0, 0, 0, 64),
        offset: designos_doc::Vec2::new(0.0, 4.0),
        radius,
        spread: 0.0,
        visible: true,
        show_behind: false,
        blend: BlendMode::Normal,
    };

    // 1. Review probe: two shadows (radius 8 and 4), `effects/0/radius` bound to 18; delete effect 0 -> remaining effect renders 4 with no binding.
    s.shell.app.edit_nodes(&[hero], |n| n.effects = vec![shadow(8.0), shadow(4.0)]);
    s.shell.app.bind_variable(&[hero], &["effects/0/radius"], &v_blur0);
    s.scroll_inspector_down();
    s.frame(vec![]);
    s.click(s.rect_of("fx-minus-0").center());
    s.frame(vec![]);

    let n = s.app().doc.get(hero).unwrap();
    let raw_radii: Vec<f32> = n.effects.iter().map(|e| if let Effect::DropShadow { radius, .. } = e { *radius } else { -1.0 }).collect();
    let res_radii: Vec<f32> =
        designos_doc::resolve_node(&s.app().doc, n).effects.iter().map(|e| if let Effect::DropShadow { radius, .. } = e { *radius } else { -1.0 }).collect();
    assert_eq!(raw_radii, vec![4.0], "guard: effect 0 was deleted");
    assert_eq!(res_radii, vec![4.0], "remaining effect must render 4.0 with no stale binding from deleted effect 0");
    assert!(!n.variables.bindings.contains_key("effects/0/radius"));

    // Undo restores both effects and `effects/0/radius` in one step.
    s.cmd_key(Key::Z);
    s.frame(vec![]);
    let n = s.app().doc.get(hero).unwrap();
    assert_eq!(n.effects.len(), 2);
    assert_eq!(n.variables.bindings.get("effects/0/radius"), Some(&v_blur0));

    // 2. When effect 1 also has a binding (`effects/1/radius`), deleting effect 0 shifts `effects/1/radius` down to `effects/0/radius` in one undo step.
    s.shell.app.bind_variable(&[hero], &["effects/1/radius"], &v_blur1);
    s.frame(vec![]);
    s.click(s.rect_of("fx-minus-0").center());
    s.frame(vec![]);
    let n = s.app().doc.get(hero).unwrap();
    let res_radii: Vec<f32> =
        designos_doc::resolve_node(&s.app().doc, n).effects.iter().map(|e| if let Effect::DropShadow { radius, .. } = e { *radius } else { -1.0 }).collect();
    assert_eq!(res_radii, vec![12.0]);
    assert_eq!(n.variables.bindings.get("effects/0/radius"), Some(&v_blur1));
    assert_eq!(n.variables.bindings.get("effects/1/radius"), None);

    s.cmd_key(Key::Z);
    s.frame(vec![]);
    let n = s.app().doc.get(hero).unwrap();
    assert_eq!(n.effects.len(), 2);
    assert_eq!(n.variables.bindings.get("effects/0/radius"), Some(&v_blur0));
    assert_eq!(n.variables.bindings.get("effects/1/radius"), Some(&v_blur1));
}

#[test]
fn eyedropper_sample_closes_picker_and_commits_one_undo_step() {
    let mut s = Sim::with(Options { select: Some("Hero".into()), ..Default::default() });
    let hero = s.by_name("Hero");
    s.shell.app.edit_nodes(&[hero], |n| {
        n.fills = vec![Paint {
            kind: PaintKind::LinearGradient {
                handles: [designos_doc::Vec2::new(0.0, 0.5), designos_doc::Vec2::new(1.0, 0.5), designos_doc::Vec2::new(0.0, 1.0)],
                stops: vec![GradientStop { pos: 0.0, color: Color::BLACK }, GradientStop { pos: 1.0, color: Color::WHITE }],
            },
            opacity: 1.0,
            visible: true,
            blend: BlendMode::Normal,
        }];
    });
    s.frame(vec![]);

    s.click(s.rect_of("sw:false:0").center());
    s.frame(vec![]);
    let picker = s.rect_of("picker");
    let depth0 = s.app().hist.undo_depth();
    s.click(Pos2::new(picker.left() + 28.0, picker.top() + 377.0));
    s.frame(vec![]);
    assert_eq!(s.app().props.picker.as_ref().map(|p| p.eyedrop), Some(true), "guard: eyedropper button armed");

    s.click(Pos2::new(500.0, 400.0));
    s.frame(vec![]);
    assert!(s.app().props.picker.is_none(), "picker must close after an eyedropper sample as on base");
    assert_eq!(s.app().hist.undo_depth() - depth0, 1, "eyedropper sample must commit one undo step");
    let PaintKind::LinearGradient { stops, .. } = &s.app().doc.get(hero).unwrap().fills[0].kind else {
        panic!("expected LinearGradient");
    };
    assert_ne!(stops[0].color, Color::BLACK, "eyedropper sample must update the stop colour");

    s.cmd_key(Key::Z);
    s.frame(vec![]);
    let PaintKind::LinearGradient { stops, .. } = &s.app().doc.get(hero).unwrap().fills[0].kind else {
        panic!("expected LinearGradient");
    };
    assert_eq!(stops[0].color, Color::BLACK, "one undo restores the pre-sample stop colour");
}

#[test]
fn unbound_gradient_picker_session_with_flip_stays_one_undo_step() {
    let mut s = Sim::with(Options { select: Some("Hero".into()), ..Default::default() });
    let hero = s.by_name("Hero");
    s.shell.app.edit_nodes(&[hero], |n| {
        n.fills = vec![Paint {
            kind: PaintKind::LinearGradient {
                handles: [designos_doc::Vec2::new(0.0, 0.5), designos_doc::Vec2::new(1.0, 0.5), designos_doc::Vec2::new(0.0, 1.0)],
                stops: vec![GradientStop { pos: 0.0, color: Color::BLACK }, GradientStop { pos: 1.0, color: Color::WHITE }],
            },
            opacity: 1.0,
            visible: true,
            blend: BlendMode::Normal,
        }];
    });
    s.frame(vec![]);
    let original = s.app().doc.get(hero).unwrap().fills.clone();

    s.click(s.rect_of("sw:false:0").center());
    s.frame(vec![]);
    let picker = s.rect_of("picker");
    let depth0 = s.app().hist.undo_depth();
    s.click(Pos2::new(picker.left() + 24.0, picker.top() + 504.0));
    s.frame(vec![]);
    s.click(Pos2::new(picker.left() + 184.0, picker.top() + 100.0));
    s.frame(vec![]);
    s.click(Pos2::new(picker.left() + 48.0, picker.top() + 504.0));
    s.frame(vec![]);
    s.key(Key::Escape);
    s.frame(vec![]);

    assert!(s.app().props.picker.is_none());
    assert_eq!(s.app().hist.undo_depth() - depth0, 1, "unbound picker session with flip must stay one undo step");
    s.cmd_key(Key::Z);
    s.frame(vec![]);
    assert_eq!(s.app().doc.get(hero).unwrap().fills, original, "one undo restores original fills after unbound flip session");
}

#[test]
fn canvas_gradient_line_insert_remaps_stop_binding_and_undoes_in_one_step() {
    let mut s = Sim::with(Options { select: Some("Hero".into()), ..Default::default() });
    let hero = s.by_name("Hero");
    let mut store = designos_doc::VariableStore::from_doc(&s.app().doc);
    let (coll, light, _dark) = theme_ids(s.app());
    let v = store.add_variable(&coll, "Probe/Red", VariableKind::Color).unwrap();
    let red = Color::rgb(0xFF, 0x11, 0x22);
    store.set_value(&v, &light, VariableValue::Color(red));
    set_scopes(&mut store, &v, &["ALL_FILLS"]);
    s.shell.app.commit_store(store, &[], &[]);

    s.shell.app.edit_nodes(&[hero], |n| {
        n.fills = vec![Paint {
            kind: PaintKind::LinearGradient {
                handles: [designos_doc::Vec2::new(0.0, 0.5), designos_doc::Vec2::new(1.0, 0.5), designos_doc::Vec2::new(0.0, 1.0)],
                stops: vec![GradientStop { pos: 0.0, color: Color::BLACK }, GradientStop { pos: 1.0, color: Color::WHITE }],
            },
            opacity: 1.0,
            visible: true,
            blend: BlendMode::Normal,
        }];
    });
    s.frame(vec![]);

    s.click(s.rect_of("sw:false:0").center());
    s.frame(vec![]);
    let picker = s.rect_of("picker");
    s.click(Pos2::new(picker.left() + 168.0, picker.top() + 100.0));
    s.frame(vec![]);
    s.mv(Pos2::new(picker.left() + 100.0, picker.top() + 423.0));
    s.frame(vec![]);
    s.click(s.rect_of("bind:fills/0/stops/1").center());
    s.frame(vec![]);
    s.click(s.rect_of("pick:Probe/Red").center());
    s.frame(vec![]);
    assert_eq!(s.app().doc.get(hero).unwrap().variables.bindings.get("fills/0/stops/1"), Some(&v));

    let depth0 = s.app().hist.undo_depth();
    let n = s.app().doc.get(hero).unwrap();
    let (w, h) = (n.size.x, n.size.y);
    let xf = s.app().doc.world_transform(hero);
    let hit = [0.5_f32, 0.4, 0.6, 0.3, 0.7, 0.25, 0.75]
        .into_iter()
        .map(|t| {
            let pg = xf.apply(designos_doc::Vec2::new(w * t, h * 0.5));
            s.screen(pg.x, pg.y)
        })
        .find(|&p| s.app().canvas_rect.contains(p) && !picker.expand(4.0).contains(p))
        .expect("a visible point on the gradient line");

    s.click(hit);
    s.frame(vec![]);

    let n = s.app().doc.get(hero).unwrap();
    let PaintKind::LinearGradient { stops: raw_stops, .. } = &n.fills[0].kind else { panic!("expected LinearGradient") };
    let resolved = designos_doc::resolve_node(&s.app().doc, n);
    let PaintKind::LinearGradient { stops: res_stops, .. } = &resolved.fills[0].kind else { panic!("expected LinearGradient") };
    assert_eq!(raw_stops.len(), 3, "guard: the canvas click inserted a stop");
    assert_eq!(n.variables.bindings.get("fills/0/stops/1"), None);
    assert_eq!(n.variables.bindings.get("fills/0/stops/2"), Some(&v), "binding follows the end stop to index 2");
    assert_eq!(res_stops[2].color, red);
    assert_ne!(res_stops[1].color, red, "the new middle stop is not painted by the binding");
    assert_eq!(s.app().hist.undo_depth() - depth0, 1);

    s.cmd_key(Key::Z);
    s.frame(vec![]);
    let n = s.app().doc.get(hero).unwrap();
    let PaintKind::LinearGradient { stops: raw_stops, .. } = &n.fills[0].kind else { panic!("expected LinearGradient") };
    assert_eq!(raw_stops.len(), 2, "one undo removes the inserted stop");
    assert_eq!(n.variables.bindings.get("fills/0/stops/1"), Some(&v), "and puts the binding back on index 1");
    assert_eq!(n.variables.bindings.get("fills/0/stops/2"), None);
}

#[test]
fn dragging_gradient_stop_off_bar_deletes_at_most_one_stop_per_drag() {
    let mut s = Sim::with(Options { select: Some("Hero".into()), ..Default::default() });
    let hero = s.by_name("Hero");
    let four = vec![
        GradientStop { pos: 0.0, color: Color::BLACK },
        GradientStop { pos: 0.33, color: Color::rgb(0x40, 0x40, 0x40) },
        GradientStop { pos: 0.66, color: Color::rgb(0x90, 0x90, 0x90) },
        GradientStop { pos: 1.0, color: Color::WHITE },
    ];
    s.shell.app.edit_nodes(&[hero], |n| {
        n.fills = vec![Paint {
            kind: PaintKind::LinearGradient {
                handles: [designos_doc::Vec2::new(0.0, 0.5), designos_doc::Vec2::new(1.0, 0.5), designos_doc::Vec2::new(0.0, 1.0)],
                stops: four.clone(),
            },
            opacity: 1.0,
            visible: true,
            blend: BlendMode::Normal,
        }];
    });
    s.frame(vec![]);
    s.click(s.rect_of("sw:false:0").center());
    s.frame(vec![]);
    let picker = s.rect_of("picker");
    let a = Pos2::new(picker.left() + 16.0 + 152.0 * 0.33, picker.top() + 100.0);
    s.mv(a);
    s.button(true);
    for dy in [10.0, 20.0, 35.0, 40.0, 45.0, 50.0] {
        s.mv(Pos2::new(a.x, a.y + dy));
    }
    s.button(false);
    s.frame(vec![]);
    let PaintKind::LinearGradient { stops, .. } = &s.app().doc.get(hero).unwrap().fills[0].kind else {
        panic!("expected LinearGradient");
    };
    assert_eq!(stops.len(), 3, "holding a drag off the gradient bar must delete only one stop (4 -> 3)");
}

#[test]
fn dangling_gradient_stop_binding_edit_is_kept() {
    let mut s = Sim::with(Options { select: Some("Hero".into()), ..Default::default() });
    let hero = s.by_name("Hero");
    s.shell.app.edit_nodes(&[hero], |n| {
        n.fills = vec![Paint {
            kind: PaintKind::LinearGradient {
                handles: [designos_doc::Vec2::new(0.0, 0.5), designos_doc::Vec2::new(1.0, 0.5), designos_doc::Vec2::new(0.0, 1.0)],
                stops: vec![GradientStop { pos: 0.0, color: Color::BLACK }, GradientStop { pos: 1.0, color: Color::WHITE }],
            },
            opacity: 1.0,
            visible: true,
            blend: BlendMode::Normal,
        }];
        n.variables.bindings.insert("fills/0/stops/0".into(), "VariableID:missing".into());
    });
    s.frame(vec![]);
    assert_eq!(
        s.app().doc.get(hero).unwrap().variables.bindings.get("fills/0/stops/0").map(String::as_str),
        Some("VariableID:missing"),
        "guard: the planted binding survived"
    );
    s.click(s.rect_of("sw:false:0").center());
    s.frame(vec![]);
    let picker = s.rect_of("picker");
    s.click(Pos2::new(picker.left() + 16.0, picker.top() + 100.0));
    s.frame(vec![]);
    assert!(!has_test_rect(&s, "bound:fills/0/stops/0"), "dangling stop binding must not render a bound pill");
    s.click(Pos2::new(picker.left() + 24.0, picker.top() + 504.0));
    s.frame(vec![]);
    let pk_col = s.app().props.picker.as_ref().and_then(|p| match &p.paint.kind {
        PaintKind::LinearGradient { stops, .. } => Some(stops[0].color),
        _ => None,
    });
    let n = s.app().doc.get(hero).unwrap();
    let PaintKind::LinearGradient { stops: raw_stops, .. } = &n.fills[0].kind else { panic!("expected LinearGradient") };
    let resolved = designos_doc::resolve_node(&s.app().doc, n);
    let PaintKind::LinearGradient { stops: res_stops, .. } = &resolved.fills[0].kind else { panic!("expected LinearGradient") };
    assert_ne!(pk_col, Some(Color::BLACK), "guard: swatch click updated the picker stop colour");
    assert_eq!(Some(raw_stops[0].color), pk_col, "document raw stop colour must keep the edit on a dangling binding");
    assert_eq!(Some(res_stops[0].color), pk_col, "the canvas shows the colour the picker shows");
}

#[test]
fn multiselect_stroke_gradient_bound_stop_preserves_bound_color_on_every_layer() {
    let mut s = Sim::with(Options { select: Some("Hero".into()), ..Default::default() });
    let mut store = designos_doc::VariableStore::from_doc(&s.app().doc);
    let (coll, light, _dark) = theme_ids(s.app());
    let v_red = store.add_variable(&coll, "Probe/StrokeRed", VariableKind::Color).unwrap();
    let red = Color::rgb(0xFF, 0x11, 0x22);
    store.set_value(&v_red, &light, VariableValue::Color(red));
    set_scopes(&mut store, &v_red, &["STROKE_COLOR"]);
    let v_blue = store.add_variable(&coll, "Probe/StrokeBlue", VariableKind::Color).unwrap();
    let blue = Color::rgb(0x22, 0x66, 0xFF);
    store.set_value(&v_blue, &light, VariableValue::Color(blue));
    set_scopes(&mut store, &v_blue, &["STROKE_COLOR"]);
    s.shell.app.commit_store(store, &[], &[]);

    let mut heroes: Vec<NodeId> = s.app().doc.nodes.values().filter(|n| n.name == "Hero").map(|n| n.id).collect();
    heroes.sort();
    assert!(heroes.len() >= 2, "fixture has two Hero layers");
    let (a, b) = (heroes[0], heroes[1]);

    let two_stop_grad = || Paint {
        kind: PaintKind::LinearGradient {
            handles: [designos_doc::Vec2::new(0.0, 0.5), designos_doc::Vec2::new(1.0, 0.5), designos_doc::Vec2::new(0.0, 1.0)],
            stops: vec![GradientStop { pos: 0.0, color: Color::BLACK }, GradientStop { pos: 1.0, color: Color::WHITE }],
        },
        opacity: 1.0,
        visible: true,
        blend: BlendMode::Normal,
    };
    let stroke_stops_of = |n: &designos_doc::Node| -> Vec<(f32, Color)> {
        match &n.stroke.paints[0].kind {
            PaintKind::LinearGradient { stops, .. } => stops.iter().map(|st| (st.pos, st.color)).collect(),
            _ => vec![],
        }
    };

    s.shell.app.edit_nodes(&[a, b], |n| {
        n.stroke.paints = vec![two_stop_grad()];
        n.stroke.weight = 2.0;
    });
    s.shell.app.bind_variable(&[a], &["strokes/0/stops/0"], &v_blue);
    s.shell.app.bind_variable(&[b], &["strokes/0/stops/0"], &v_red);
    s.shell.app.selection = vec![a, b];
    s.frame(vec![]);
    s.frame(vec![]);

    s.click(s.rect_of("sw:true:0").center());
    s.frame(vec![]);
    let picker = s.rect_of("picker");
    // Select unbound stop 1 and edit its colour with a swatch.
    s.click(Pos2::new(picker.left() + 168.0, picker.top() + 100.0));
    s.frame(vec![]);
    s.click(Pos2::new(picker.left() + 24.0, picker.top() + 504.0));
    s.frame(vec![]);

    let na = s.app().doc.get(a).unwrap();
    let nb = s.app().doc.get(b).unwrap();
    let a_raw = stroke_stops_of(na);
    let a_res = stroke_stops_of(&designos_doc::resolve_node(&s.app().doc, na));
    let b_raw = stroke_stops_of(nb);
    let b_res = stroke_stops_of(&designos_doc::resolve_node(&s.app().doc, nb));
    assert_ne!(a_raw[1].1, Color::WHITE, "guard: editing stop 1 updated layer a's stop 1");
    assert_eq!(a_raw[1].1, b_raw[1].1, "guard: editing stop 1 updated layer b's stop 1");
    assert_eq!(a_raw[0].1, blue, "layer a's bound stroke stop 0 raw colour stays blue");
    assert_eq!(a_raw[0].1, a_res[0].1, "layer a's bound stroke stop 0 raw colour equals its resolved colour");
    assert_eq!(b_raw[0].1, red, "layer b's bound stroke stop 0 raw colour stays red");
    assert_eq!(b_raw[0].1, b_res[0].1, "layer b's bound stroke stop 0 raw colour equals its resolved colour");
}

#[test]
fn dangling_only_gradient_stop_flip_session_stays_one_undo_step_and_remaps_key() {
    let mut s = Sim::with(Options { select: Some("Hero".into()), ..Default::default() });
    let hero = s.by_name("Hero");
    let green = Color { r: 0.0, g: 1.0, b: 0.0, a: 1.0 };
    s.shell.app.edit_nodes(&[hero], |n| {
        n.fills = vec![Paint {
            kind: PaintKind::LinearGradient {
                handles: [designos_doc::Vec2::new(0.0, 0.5), designos_doc::Vec2::new(1.0, 0.5), designos_doc::Vec2::new(0.0, 1.0)],
                stops: vec![GradientStop { pos: 0.0, color: green }, GradientStop { pos: 1.0, color: Color::WHITE }],
            },
            opacity: 1.0,
            visible: true,
            blend: BlendMode::Normal,
        }];
        n.variables.bindings.insert("fills/0/stops/0".into(), "VariableID:missing".into());
    });
    s.frame(vec![]);
    let stop_binds = |s: &Sim| -> Vec<(String, String)> {
        s.app().doc.get(hero).unwrap().variables.bindings.iter().filter(|(k, _)| k.contains("/stops/")).map(|(k, v)| (k.clone(), v.clone())).collect()
    };
    let stop_colors = |p: &Paint| -> Vec<Color> {
        match &p.kind {
            PaintKind::LinearGradient { stops, .. } => stops.iter().map(|st| st.color).collect(),
            _ => vec![],
        }
    };
    let original = (s.app().doc.get(hero).unwrap().fills.clone(), stop_binds(&s));

    s.click(s.rect_of("sw:false:0").center());
    s.frame(vec![]);
    let picker = s.rect_of("picker");
    let d0 = s.app().hist.undo_depth();
    // Edit stop 1, flip gradient, then edit the dangling stop (now at index 1).
    s.click(Pos2::new(picker.left() + 168.0, picker.top() + 100.0));
    s.frame(vec![]);
    s.click(Pos2::new(picker.left() + 24.0, picker.top() + 504.0));
    s.frame(vec![]);
    s.click(Pos2::new(picker.left() + 184.0, picker.top() + 100.0));
    s.frame(vec![]);
    let binds_after_flip = stop_binds(&s);
    s.click(Pos2::new(picker.left() + 168.0, picker.top() + 100.0));
    s.frame(vec![]);
    s.click(Pos2::new(picker.left() + 48.0, picker.top() + 504.0));
    s.frame(vec![]);
    let pk_stops = s.app().props.picker.as_ref().map(|p| stop_colors(&p.paint)).unwrap_or_default();
    let n = s.app().doc.get(hero).unwrap();
    let raw_after_edit = stop_colors(&n.fills[0]);
    let res_after_edit = stop_colors(&designos_doc::resolve_node(&s.app().doc, n).fills[0]);
    s.key(Key::Escape);
    s.frame(vec![]);
    let steps = s.app().hist.undo_depth() - d0;
    s.cmd_key(Key::Z);
    s.frame(vec![]);
    let back = (s.app().doc.get(hero).unwrap().fills.clone(), stop_binds(&s));

    assert_eq!(binds_after_flip, vec![("fills/0/stops/1".to_string(), "VariableID:missing".to_string())], "dangling key follows its stop on flip");
    assert_eq!(raw_after_edit[1], pk_stops[1], "edit on the dangling stop lands in the doc");
    assert_eq!(raw_after_edit, res_after_edit, "raw == resolved");
    assert_eq!(steps, 1, "an unbound (dangling-only) session is one undo step");
    assert_eq!(back, original, "one undo restores fills and the dangling key");
}
