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

    // Undo removes the binding in one step; redo restores it.
    s.cmd_key(Key::Z);
    assert!(!s.app().doc.get(hero).unwrap().variables.bindings.contains_key("effects/0/radius"));
    s.mods = Modifiers::COMMAND | Modifiers::SHIFT;
    s.key(Key::Z);
    s.mods = Modifiers::NONE;
    assert_eq!(s.app().doc.get(hero).unwrap().variables.bindings.get("effects/0/radius"), Some(&fx_radius));

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
    assert!(
        s.app().props.pop.is_none(),
        "screenshot_chrome must not auto-open the effect popover inside effects::section"
    );

    let s_panel = Sim::with(Options {
        select: Some("Hero".into()),
        panel: Some("effect".into()),
        ..Default::default()
    });
    assert!(
        matches!(
            s_panel.app().props.pop.as_ref().map(|p| &p.kind),
            Some(crate::panels::effects::PopKind::Effect(0))
        ),
        "--panel effect must open PopKind::Effect(0) on the selected node"
    );
    let _ = fx_field_rect(&s_panel, "fxp-b", 0);
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
    s.shell.app.commit_store(store, &[], &[]);

    s.shell.app.edit_nodes(&[hero], |n| {
        n.fills = vec![Paint {
            kind: PaintKind::LinearGradient {
                handles: [
                    designos_doc::Vec2::new(0.0, 0.5),
                    designos_doc::Vec2::new(1.0, 0.5),
                    designos_doc::Vec2::new(0.0, 1.0),
                ],
                stops: vec![
                    GradientStop { pos: 0.0, color: Color::BLACK },
                    GradientStop { pos: 1.0, color: Color::WHITE },
                ],
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

    // 3. Eyedropper button click and canvas sampling path do nothing while the stop is bound.
    let eye_btn = s.rect_of("picker-eyedrop");
    s.click(eye_btn.center());
    s.frame(vec![]);
    assert!(!s.app().props.picker.as_ref().unwrap().eyedrop, "eyedropper button must be disabled when stop is bound");
    assert!(s.app().props.eyedropper.is_none());
    // Even if eyedropper state were armed before selecting a bound stop, sampling the canvas must not write.
    s.shell.app.props.picker.as_mut().unwrap().eyedrop = true;
    s.shell.app.props.eyedropper = Some(crate::panels::properties::PickTarget::Fill(0));
    s.click(Pos2::new(500.0, 400.0));
    s.frame(vec![]);
    assert_eq!(s.app().doc.get(hero).unwrap(), &before_node, "eyedropper canvas sample must not write to a bound stop");

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
                handles: [
                    designos_doc::Vec2::new(0.0, 0.5),
                    designos_doc::Vec2::new(1.0, 0.5),
                    designos_doc::Vec2::new(0.0, 1.0),
                ],
                stops: vec![
                    GradientStop { pos: 0.0, color: Color::BLACK },
                    GradientStop { pos: 1.0, color: Color::WHITE },
                ],
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

    assert_eq!(resolved_fill_stops(&s), vec![
        GradientStop { pos: 0.0, color: start_col },
        GradientStop { pos: 1.0, color: Color::WHITE },
    ]);

    // 1. Flip: stop 0 (bound to Grad/Start) moves to pos = 1.0 (index 1); unbound white stop moves to pos = 0.0 (index 0).
    s.click(Pos2::new(picker.left() + 184.0, picker.top() + 100.0));
    s.frame(vec![]);
    assert_eq!(s.app().doc.get(hero).unwrap().variables.bindings.get("fills/0/stops/0"), None);
    assert_eq!(s.app().doc.get(hero).unwrap().variables.bindings.get("fills/0/stops/1"), Some(&grad_start));
    assert_eq!(resolved_fill_stops(&s), vec![
        GradientStop { pos: 0.0, color: Color::WHITE },
        GradientStop { pos: 1.0, color: start_col },
    ]);

    // Undo restores both the stop order and the binding on `fills/0/stops/0` in one step.
    s.cmd_key(Key::Z);
    assert_eq!(s.app().doc.get(hero).unwrap().variables.bindings.get("fills/0/stops/0"), Some(&grad_start));
    assert_eq!(s.app().doc.get(hero).unwrap().variables.bindings.get("fills/0/stops/1"), None);
    assert_eq!(resolved_fill_stops(&s), vec![
        GradientStop { pos: 0.0, color: start_col },
        GradientStop { pos: 1.0, color: Color::WHITE },
    ]);

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
    assert_eq!(resolved_fill_stops(&s), vec![
        GradientStop { pos: 0.0, color: start_col },
        GradientStop { pos: 1.0, color: end_col },
    ]);

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
                handles: [
                    designos_doc::Vec2::new(0.0, 0.5),
                    designos_doc::Vec2::new(1.0, 0.5),
                    designos_doc::Vec2::new(0.0, 1.0),
                ],
                stops: vec![
                    GradientStop { pos: 0.0, color: Color::BLACK },
                    GradientStop { pos: 1.0, color: Color::WHITE },
                ],
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



