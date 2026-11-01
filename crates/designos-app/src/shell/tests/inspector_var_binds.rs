//! Sim tests for #876 and #133 (inspector half): always-visible variable-bind icon
//! gated by matching variable availability, non-overlapping value/bind rects,
//! typography / wrap-gap / per-side-stroke bind entry points, and style-linked
//! text detach on variable bind.

use super::*;
use designos_doc::{
    BlendMode, Effect, GradientStop, LayoutDirection, LineHeight, NodeKind, Paint, PaintKind, Style, TextStyle, VariableKind, VariableStore, VariableValue,
};
use egui::Color32;
use std::path::PathBuf;

fn has_test_rect(s: &Sim, name: &str) -> bool {
    let _ = s;
    crate::widgets::last_pass_names().into_iter().any(|(n, _)| n == name)
}

fn sim_has_text(s: &Sim, text: &str) -> bool {
    fn walk(shape: &egui::epaint::Shape, text: &str) -> bool {
        match shape {
            egui::epaint::Shape::Text(t) => t.galley.job.text == text || t.galley.job.text.contains(text),
            egui::epaint::Shape::Vec(v) => v.iter().any(|sub| walk(sub, text)),
            _ => false,
        }
    }
    s.shapes.iter().any(|cs| walk(&cs.shape, text))
}

fn sim_redo(s: &mut Sim) {
    let is_mac = s.shell.app.terminal.platform_is_mac_override.unwrap_or(cfg!(target_os = "macos"));
    s.mods = Modifiers { alt: false, ctrl: !is_mac, shift: true, mac_cmd: is_mac, command: true };
    s.key(Key::Z);
    s.mods = Modifiers::NONE;
}

fn field_rect_any(s: &Sim, key: impl std::fmt::Debug) -> Rect {
    s.ctx.data(|d| d.get_temp::<Rect>(egui::Id::new(("field-rect", format!("{key:?}"))))).unwrap_or_else(|| panic!("no field {key:?}"))
}

fn painted_bg_rect_at(s: &Sim, left: f32, top: f32) -> Rect {
    fn walk(shape: &egui::epaint::Shape, left: f32, top: f32) -> Option<Rect> {
        match shape {
            egui::epaint::Shape::Rect(r)
                if (r.rect.left() - left).abs() < 1.5 && (r.rect.top() - top).abs() < 1.5 && r.rect.height() >= 20.0 && r.rect.width() >= 30.0 =>
            {
                Some(r.rect)
            }
            egui::epaint::Shape::Vec(v) => v.iter().find_map(|sub| walk(sub, left, top)),
            _ => None,
        }
    }
    s.shapes.iter().find_map(|cs| walk(&cs.shape, left, top)).unwrap_or_else(|| panic!("no painted field_bg rect at ({left}, {top})"))
}

fn weight_dropdown_rect(s: &Sim) -> Rect {
    painted_bg_rect_at(s, s.field_rect("font-family").left(), s.field_rect("font-size").top())
}

fn open_type_details(s: &mut Sim) {
    let style_btn = s.rect_of("text-style-button");
    let details_btn = Pos2::new(style_btn.right() + crate::theme::SPACE_FACT + crate::theme::PANEL_ACTION_BUTTON * 0.5, style_btn.center().y);
    s.click(details_btn);
}

fn has_stroke_color_in_rect(s: &Sim, target: Rect, color: Color32) -> bool {
    fn walk(shape: &egui::epaint::Shape, target: Rect, color: Color32) -> bool {
        match shape {
            egui::epaint::Shape::Path(p) => {
                p.stroke.color == egui::epaint::ColorMode::Solid(color) && !p.points.is_empty() && p.points.iter().all(|pt| target.expand(2.0).contains(*pt))
            }
            egui::epaint::Shape::Vec(v) => v.iter().any(|sub| walk(sub, target, color)),
            _ => false,
        }
    }
    s.shapes.iter().any(|cs| walk(&cs.shape, target, color))
}

fn clear_all_variables_and_bindings(app: &mut App) {
    app.doc.variables.clear();
    app.doc.variable_collections.clear();
    app.doc.library.enabled.clear();
    app.doc.library.external.clear();
    for n in app.doc.nodes.values_mut() {
        n.variables.bindings.clear();
        n.variables.modes.clear();
    }
    for m in app.doc.style_meta.values_mut() {
        m.bindings.clear();
    }
}

fn add_test_collection_with_all_kinds(app: &mut App) -> (String, String, String, String, String) {
    clear_all_variables_and_bindings(app);
    let mut store = VariableStore::default();
    let coll = store.add_collection("Tokens");
    let mode = store.collections[&coll].default_mode.clone();

    let color_id = store.add_variable(&coll, "Color/Brand", VariableKind::Color).unwrap();
    store.set_value(&color_id, &mode, VariableValue::Color(Color::rgb(0x22, 0x66, 0xDD)));

    let num_id = store.add_variable(&coll, "Number/Step", VariableKind::Number).unwrap();
    store.set_value(&num_id, &mode, VariableValue::Number(24.0));

    let str_fam_id = store.add_variable(&coll, "Type/Family", VariableKind::String).unwrap();
    store.set_value(&str_fam_id, &mode, VariableValue::String("Playfair Display".into()));

    let str_style_id = store.add_variable(&coll, "Type/Style", VariableKind::String).unwrap();
    store.set_value(&str_style_id, &mode, VariableValue::String("Bold Italic".into()));

    app.commit_store(store, &[], &[]);
    (coll, color_id, num_id, str_fam_id, str_style_id)
}

fn tmp_lib_dir(tag: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("designos-var-binds-{tag}-{}", designos_library::new_id()));
    let _ = std::fs::remove_dir_all(&p);
    p
}

fn has_filled_rect_at(s: &Sim, target: Rect, fill: Color32) -> bool {
    fn walk(shape: &egui::epaint::Shape, target: Rect, fill: Color32) -> bool {
        match shape {
            egui::epaint::Shape::Rect(r) => (r.rect.min - target.min).length() < 1.0 && (r.rect.max - target.max).length() < 1.0 && r.fill == fill,
            egui::epaint::Shape::Vec(v) => v.iter().any(|sub| walk(sub, target, fill)),
            _ => false,
        }
    }
    s.shapes.iter().any(|cs| walk(&cs.shape, target, fill))
}

#[test]
fn inspector_var_binds_always_visible_at_rest_and_opens_picker_on_single_click() {
    let mut s = Sim::with(Options { select: Some("Hero".into()), ..Default::default() });
    let (_coll, _color_id, _num_id, _fam_id, _style_id) = add_test_collection_with_all_kinds(&mut s.shell.app);
    s.frame(vec![]);

    // Without moving the pointer (`s.mv` not called), every bindable field on Hero records `bind:<key>`.
    for key in ["width", "height", "opacity", "topLeftRadius", "fills/0"] {
        let name = format!("bind:{key}");
        assert!(has_test_rect(&s, &name), "expected {name} to be recorded at rest without hovering");
        let b = s.rect_of(&name);
        assert!(b.width() >= crate::theme::TARGET_MIN && b.height() >= crate::theme::TARGET_MIN, "{name} hit box {b:?} smaller than TARGET_MIN");
    }

    // At rest, no BG_TERTIARY or BG_SECONDARY pill background is drawn under `bind:width`,
    // the icon is vertically centred on its 32 px row (MINOR-4), and its stroke uses ICON_QUATERNARY (MINOR-5 / M12).
    let bw = s.rect_of("bind:width");
    let fw = s.field_rect("w");
    assert!(
        (bw.center().y - fw.center().y).abs() < 0.5,
        "bind:width must be vertically centred on w row: bind cy={} vs field cy={}",
        bw.center().y,
        fw.center().y
    );
    assert!(!has_filled_rect_at(&s, bw, crate::theme::BG_TERTIARY), "at-rest bind button must not fill BG_TERTIARY");
    assert!(has_stroke_color_in_rect(&s, bw, crate::theme::ICON_QUATERNARY), "at-rest bind button must paint hexagon stroke in ICON_QUATERNARY");
    assert!(!has_stroke_color_in_rect(&s, bw, crate::theme::ICON_SECONDARY), "at-rest bind button must not paint hexagon stroke in ICON_SECONDARY");

    // Hovering `bind:width` draws the BG_TERTIARY rounded rect.
    s.mv(bw.center());
    s.frame(vec![]);
    assert!(has_filled_rect_at(&s, bw, crate::theme::BG_TERTIARY), "hovered bind button must fill BG_TERTIARY");

    // Move pointer away to (0, 0), settle, then click `bind:opacity` in a single frame without pre-hovering.
    s.mv(Pos2::ZERO);
    s.frame(vec![]);
    let bo = s.rect_of("bind:opacity");
    let fo = s.field_rect("opacity");
    assert!(
        (bo.center().y - fo.center().y).abs() < 0.5,
        "bind:opacity must be vertically centred on opacity row: bind cy={} vs field cy={}",
        bo.center().y,
        fo.center().y
    );
    s.click(bo.center());
    assert_eq!(
        s.app().vars.bind_picker.as_ref().and_then(|bp| bp.keys.first().map(String::as_str)),
        Some("opacity"),
        "clicking bind:opacity at rest must open the bind picker in one frame"
    );
    s.shell.app.vars.bind_picker = None;

    // M11: Open gradient picker on Hero and verify gradient-stop row records NO `bind:fills/0/stops/0` without hover,
    // and DOES record `bind:fills/0/stops/0` when hovered.
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
    s.mv(Pos2::ZERO);
    s.frame(vec![]);
    let sw = s.rect_of("sw:false:0");
    s.click(sw.center());
    s.mv(Pos2::ZERO);
    s.frame(vec![]);
    let picker_r = s.rect_of("picker");
    assert!(!has_test_rect(&s, "bind:fills/0/stops/0"), "gradient-stop row must NOT record bind:fills/0/stops/0 without pointer hover (M11)");
    s.mv(Pos2::new(picker_r.left() + 100.0, picker_r.top() + 423.0));
    s.frame(vec![]);
    assert!(has_test_rect(&s, "bind:fills/0/stops/0"), "gradient-stop row must record bind:fills/0/stops/0 when pointer hovers the stop row");
}

#[test]
fn inspector_var_binds_hidden_when_no_matching_local_or_library_variable_exists() {
    let mut s = Sim::with(Options { select: Some("Hero".into()), ..Default::default() });
    clear_all_variables_and_bindings(&mut s.shell.app);
    s.frame(vec![]);

    for key in ["width", "height", "opacity", "topLeftRadius", "fills/0", "fontSize", "fontFamily", "fontStyle"] {
        assert!(!has_test_rect(&s, &format!("bind:{key}")), "bind:{key} must be hidden when the document has no variables");
    }
    assert_eq!(s.field_rect("w").width(), 88.0, "when no bind icon is shown, field w must keep its full 88 px width (M5)");

    // Color-only document (M1 / M5): only `bind:fills/0` shows on Hero; numeric/string fields show no icon and keep 88 px width.
    let mut cstore = VariableStore::default();
    let ccoll = cstore.add_collection("ColorsOnly");
    let cmode = cstore.collections[&ccoll].default_mode.clone();
    let cvar = cstore.add_variable(&ccoll, "Color/Primary", VariableKind::Color).unwrap();
    cstore.set_value(&cvar, &cmode, VariableValue::Color(Color::rgb(0x11, 0x22, 0x33)));
    s.shell.app.commit_store(cstore, &[], &[]);
    s.frame(vec![]);
    assert!(has_test_rect(&s, "bind:fills/0"), "Color-only document must show bind:fills/0");
    for absent in ["width", "height", "opacity", "topLeftRadius"] {
        assert!(!has_test_rect(&s, &format!("bind:{absent}")), "Color-only document must not show bind:{absent} (M1)");
    }
    assert_eq!(s.field_rect("w").width(), 88.0, "Color-only document must not narrow numeric field w (M5)");
    clear_all_variables_and_bindings(&mut s.shell.app);

    // Add a single Number variable scoped only to CORNER_RADIUS.
    let mut store = VariableStore::default();
    let coll = store.add_collection("Radii");
    let mode = store.collections[&coll].default_mode.clone();
    let rad_var = store.add_variable(&coll, "Radius/Card", VariableKind::Number).unwrap();
    store.set_value(&rad_var, &mode, VariableValue::Number(12.0));
    store.variables.get_mut(&rad_var).unwrap().scopes = vec!["CORNER_RADIUS".into()];
    s.shell.app.commit_store(store, &[], &[]);
    s.frame(vec![]);

    assert!(has_test_rect(&s, "bind:topLeftRadius"), "CORNER_RADIUS variable must show bind:topLeftRadius");
    for absent in ["width", "height", "opacity", "fills/0", "fontSize"] {
        assert!(!has_test_rect(&s, &format!("bind:{absent}")), "CORNER_RADIUS-only variable must not show bind:{absent}");
    }
    assert_eq!(s.field_rect("w").width(), 88.0, "CORNER_RADIUS-only variable must leave w at full 88 px width");
    assert_eq!(s.field_rect("radius").width(), 63.0, "CORNER_RADIUS-only variable must narrow radius to 63 px");

    // Picker contents scope check: a FONT_SIZE-only variable is offered in the fontSize picker and NOT offered on lineHeight.
    let welcome = s.by_name("Welcome back");
    s.shell.app.selection = vec![welcome];
    clear_all_variables_and_bindings(&mut s.shell.app);
    let mut tstore = VariableStore::default();
    let tcoll = tstore.add_collection("TypeScopes");
    let tmode = tstore.collections[&tcoll].default_mode.clone();
    let fs_only = tstore.add_variable(&tcoll, "Scoped/FontSizeOnly", VariableKind::Number).unwrap();
    tstore.set_value(&fs_only, &tmode, VariableValue::Number(18.0));
    tstore.variables.get_mut(&fs_only).unwrap().scopes = vec!["FONT_SIZE".into()];
    let unscoped_num = tstore.add_variable(&tcoll, "Scoped/AnyNumber", VariableKind::Number).unwrap();
    tstore.set_value(&unscoped_num, &tmode, VariableValue::Number(22.0));
    s.shell.app.commit_store(tstore, &[], &[]);
    s.frame(vec![]);

    s.click(s.rect_of("bind:fontSize").center());
    s.frame(vec![]);
    assert!(has_test_rect(&s, "pick:Scoped/FontSizeOnly"), "FONT_SIZE-only variable must be offered in fontSize picker");
    assert!(has_test_rect(&s, "pick:Scoped/AnyNumber"), "unscoped Number variable must be offered in fontSize picker");
    s.shell.app.vars.bind_picker = None;
    s.frame(vec![]);

    s.click(s.rect_of("bind:lineHeight").center());
    s.frame(vec![]);
    assert!(!has_test_rect(&s, "pick:Scoped/FontSizeOnly"), "FONT_SIZE-only variable must NOT be offered in lineHeight picker");
    assert!(has_test_rect(&s, "pick:Scoped/AnyNumber"), "unscoped Number variable must be offered in lineHeight picker");
    s.shell.app.vars.bind_picker = None;

    // Library-only check: publish a library with only a CORNER_RADIUS Number variable, enable it in an empty-var doc.
    let root = tmp_lib_dir("scope-gate");
    let mut lib_sim = Sim::new();
    lib_sim.shell.app.libs.set_root(root.clone());
    clear_all_variables_and_bindings(&mut lib_sim.shell.app);
    let mut lstore = VariableStore::default();
    let lcoll = lstore.add_collection("SharedRadii");
    let lmode = lstore.collections[&lcoll].default_mode.clone();
    let lrad = lstore.add_variable(&lcoll, "Radius/Shared", VariableKind::Number).unwrap();
    lstore.set_value(&lrad, &lmode, VariableValue::Number(16.0));
    lstore.variables.get_mut(&lrad).unwrap().scopes = vec!["CORNER_RADIUS".into()];
    lib_sim.shell.app.commit_store(lstore, &[], &[]);
    lib_sim.shell.app.open_libraries();
    for _ in 0..3 {
        lib_sim.frame(vec![]);
    }
    lib_sim.shell.app.libs.description = "Radii only".into();
    let pub_btn = lib_sim.rect_of("lib-publish");
    lib_sim.click(pub_btn.center());
    for _ in 0..2 {
        lib_sim.frame(vec![]);
    }
    let lib_id = lib_sim.app().doc.library.published.as_ref().expect("published library").library_id.clone();

    let mut consumer = Sim::with(Options { select: Some("Hero".into()), ..Default::default() });
    consumer.shell.app.libs.set_root(root.clone());
    clear_all_variables_and_bindings(&mut consumer.shell.app);
    consumer.shell.app.set_library_enabled(&lib_id, true);
    consumer.shell.app.refresh_libraries();
    consumer.frame(vec![]);

    assert!(consumer.app().doc.variables.is_empty(), "consumer has no local variables");
    assert!(has_test_rect(&consumer, "bind:topLeftRadius"), "enabled library CORNER_RADIUS variable must show bind:topLeftRadius");
    assert!(!has_test_rect(&consumer, "bind:width"), "enabled library CORNER_RADIUS variable must not show bind:width");
    assert!(!has_test_rect(&consumer, "bind:fills/0"), "enabled library CORNER_RADIUS variable must not show bind:fills/0");

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn inspector_var_binds_typography_fields_bind_detach_legacy_weight_and_font_load() {
    let mut s = Sim::with(Options { select: Some("Welcome back".into()), ..Default::default() });
    let t_id = s.by_name("Welcome back");
    let (_coll, _color_id, num_id, _fam_id, _style_id) = add_test_collection_with_all_kinds(&mut s.shell.app);
    s.frame(vec![]);

    for key in ["fontFamily", "fontSize", "lineHeight", "letterSpacing"] {
        assert!(has_test_rect(&s, &format!("bind:{key}")), "expected bind:{key} in Typography section at rest");
    }
    // MAJOR-3 (owner ruling): Weight dropdown has NO bind icon and NO reserve, so weight names show in full.
    assert!(!has_test_rect(&s, "bind:fontStyle"), "weight dropdown must not draw a bind:fontStyle icon on the row");
    assert!(!has_test_rect(&s, "bind:fontWeight"), "weight dropdown must not draw a bind:fontWeight icon on the row");
    assert_eq!(weight_dropdown_rect(&s).width(), 88.0, "weight dropdown must keep its full 88 px width");
    for (w, label) in [(400u16, "Regular"), (500, "Medium"), (600, "Semi Bold"), (800, "Extra Bold")] {
        s.shell.app.edit_nodes(&[t_id], |n| {
            if let NodeKind::Text(tp) = &mut n.kind {
                tp.style.font_weight = w;
            }
        });
        s.frame(vec![]);
        assert!(sim_has_text(&s, label), "weight dropdown must paint full weight name '{label}' without elision");
    }

    // 1. Bind fontFamily -> updates family, requests font face, shows bound:fontFamily, undo removes it, detaches cleanly.
    s.click(s.rect_of("bind:fontFamily").center());
    s.frame(vec![]);
    s.click(s.rect_of("pick:Type/Family").center());
    s.frame(vec![]);
    let NodeKind::Text(tp) = &s.app().doc.get(t_id).unwrap().kind else { panic!("expected text") };
    assert_eq!(tp.style.font_family, "Playfair Display");
    assert!(
        s.app().fonts.requested().iter().any(|r| r.family.eq_ignore_ascii_case("Playfair Display")),
        "binding fontFamily must request the new font family via app.fonts"
    );
    s.cmd_key(Key::Z);
    s.frame(vec![]);
    assert!(!s.app().doc.get(t_id).unwrap().variables.bindings.contains_key("fontFamily"), "undo must remove fontFamily binding");
    sim_redo(&mut s);
    s.frame(vec![]);
    let fam_pill = s.rect_of("bound:fontFamily");
    s.click(Pos2::new(fam_pill.right() - 13.0, fam_pill.center().y));
    s.frame(vec![]);
    assert!(!s.app().doc.get(t_id).unwrap().variables.bindings.contains_key("fontFamily"));
    let NodeKind::Text(tp) = &s.app().doc.get(t_id).unwrap().kind else { panic!("expected text") };
    assert_eq!(tp.style.font_family, "Playfair Display", "detaching fontFamily keeps resolved family");

    // 2. Bind fontStyle through "Apply variable…" at the bottom of the weight dropdown menu -> updates weight + italic, shows bound:fontStyle, undo removes it, detaches cleanly.
    s.click(weight_dropdown_rect(&s).center());
    s.frame(vec![]);
    assert!(sim_has_text(&s, "Apply variable\u{2026}"), "weight dropdown menu must include 'Apply variable…' when String variables match fontStyle");
    s.click(s.rect_of("menu-row-0-9").center());
    s.frame(vec![]);
    assert_eq!(
        s.app().vars.bind_picker.as_ref().and_then(|bp| bp.keys.first().map(String::as_str)),
        Some("fontStyle"),
        "clicking 'Apply variable…' must open the variable picker for fontStyle"
    );
    s.click(s.rect_of("pick:Type/Style").center());
    s.frame(vec![]);
    let NodeKind::Text(tp) = &s.app().doc.get(t_id).unwrap().kind else { panic!("expected text") };
    assert_eq!(tp.style.font_weight, 700);
    assert!(tp.style.italic);
    s.cmd_key(Key::Z);
    s.frame(vec![]);
    assert!(!s.app().doc.get(t_id).unwrap().variables.bindings.contains_key("fontStyle"), "undo must remove fontStyle binding");
    sim_redo(&mut s);
    s.frame(vec![]);
    let style_pill = s.rect_of("bound:fontStyle");
    s.click(Pos2::new(style_pill.right() - 13.0, style_pill.center().y));
    s.frame(vec![]);
    assert!(!s.app().doc.get(t_id).unwrap().variables.bindings.contains_key("fontStyle"));

    // 3. Legacy/imported `fontWeight` binding on the weight row renders `bound:fontWeight` and detaches.
    s.shell.app.bind_variable(&[t_id], &["fontWeight"], &num_id);
    s.frame(vec![]);
    assert!(has_test_rect(&s, "bound:fontWeight"), "existing fontWeight binding must show bound:fontWeight pill");
    let wt_pill = s.rect_of("bound:fontWeight");
    s.click(Pos2::new(wt_pill.right() - 13.0, wt_pill.center().y));
    s.frame(vec![]);
    assert!(!s.app().doc.get(t_id).unwrap().variables.bindings.contains_key("fontWeight"));

    // 4. Bind lineHeight and letterSpacing (with undo check + pill detach).
    for (key, check) in [
        ("lineHeight", (&|tp: &designos_doc::TextProps| tp.style.line_height == LineHeight::Pixels(24.0)) as &dyn Fn(&designos_doc::TextProps) -> bool),
        ("letterSpacing", &|tp: &designos_doc::TextProps| (tp.style.letter_spacing - 24.0).abs() < 1e-4),
    ] {
        s.click(s.rect_of(&format!("bind:{key}")).center());
        s.frame(vec![]);
        s.click(s.rect_of("pick:Number/Step").center());
        s.frame(vec![]);
        let NodeKind::Text(tp) = &s.app().doc.get(t_id).unwrap().kind else { panic!("expected text") };
        assert!(check(tp), "{key} did not update TextStyle");
        s.cmd_key(Key::Z);
        s.frame(vec![]);
        assert!(!s.app().doc.get(t_id).unwrap().variables.bindings.contains_key(key), "undo must remove {key} binding");
        sim_redo(&mut s);
        s.frame(vec![]);
        let pill = s.rect_of(&format!("bound:{key}"));
        s.click(Pos2::new(pill.right() - 13.0, pill.center().y));
        s.frame(vec![]);
        assert!(!s.app().doc.get(t_id).unwrap().variables.bindings.contains_key(key));
    }

    // 5. Open Type details popover and bind paragraphIndent and paragraphSpacing (with undo check + pill detach).
    open_type_details(&mut s);
    s.frame(vec![]);
    assert!(has_test_rect(&s, "bind:paragraphIndent"), "expected bind:paragraphIndent in Type details popover");
    assert!(has_test_rect(&s, "bind:paragraphSpacing"), "expected bind:paragraphSpacing in Type details popover");

    for (key, check) in [
        ("paragraphIndent", (&|tp: &designos_doc::TextProps| (tp.paragraph.indent - 24.0).abs() < 1e-4) as &dyn Fn(&designos_doc::TextProps) -> bool),
        ("paragraphSpacing", &|tp: &designos_doc::TextProps| (tp.style.paragraph_spacing - 24.0).abs() < 1e-4),
    ] {
        s.click(s.rect_of(&format!("bind:{key}")).center());
        s.frame(vec![]);
        s.click(s.rect_of("pick:Number/Step").center());
        s.frame(vec![]);
        let NodeKind::Text(tp) = &s.app().doc.get(t_id).unwrap().kind else { panic!("expected text") };
        assert!(check(tp), "{key} did not update paragraph property");
        s.cmd_key(Key::Z);
        s.frame(vec![]);
        assert!(!s.app().doc.get(t_id).unwrap().variables.bindings.contains_key(key), "undo must remove {key} binding");
        sim_redo(&mut s);
        s.frame(vec![]);
        let pill = s.rect_of(&format!("bound:{key}"));
        s.click(Pos2::new(pill.right() - 13.0, pill.center().y));
        s.frame(vec![]);
        assert!(!s.app().doc.get(t_id).unwrap().variables.bindings.contains_key(key));
    }
}

#[test]
fn inspector_var_binds_style_linked_text_detaches_in_same_undo_step_local_and_library() {
    let mut s = Sim::with(Options { select: Some("Welcome back".into()), ..Default::default() });
    let t_id = s.by_name("Welcome back");
    let (_coll, _color_id, _num_id, _fam_id, _style_id) = add_test_collection_with_all_kinds(&mut s.shell.app);

    let style_key = "Body/Regular".to_string();
    s.shell.app.doc.styles.insert(style_key.clone(), Style::Text(TextStyle::default()));

    for (key, pick_name, in_details) in [
        ("fontFamily", "pick:Type/Family", false),
        ("fontStyle", "pick:Type/Style", false),
        ("fontSize", "pick:Number/Step", false),
        ("lineHeight", "pick:Number/Step", false),
        ("letterSpacing", "pick:Number/Step", false),
        ("paragraphIndent", "pick:Number/Step", true),
        ("paragraphSpacing", "pick:Number/Step", true),
    ] {
        s.shell.app.apply_style(&[t_id], &style_key);
        s.frame(vec![]);
        assert_eq!(s.app().doc.get(t_id).unwrap().style_refs.get("text").map(String::as_str), Some(style_key.as_str()));

        if in_details && s.app().props.pop.is_none() {
            open_type_details(&mut s);
            s.frame(vec![]);
        }

        if key == "fontStyle" {
            s.click(weight_dropdown_rect(&s).center());
            s.frame(vec![]);
            s.click(s.rect_of("menu-row-0-9").center());
            s.frame(vec![]);
        } else {
            s.click(s.rect_of(&format!("bind:{key}")).center());
            s.frame(vec![]);
        }
        s.click(s.rect_of(pick_name).center());
        s.frame(vec![]);

        let n = s.app().doc.get(t_id).unwrap();
        assert!(!n.style_refs.contains_key("text"), "binding {key} must detach the linked text style");
        assert!(n.variables.bindings.contains_key(key), "binding {key} must record the variable binding");

        // Single undo restores both the text style link and removes the variable binding.
        s.cmd_key(Key::Z);
        s.frame(vec![]);
        let n = s.app().doc.get(t_id).unwrap();
        assert_eq!(n.style_refs.get("text").map(String::as_str), Some(style_key.as_str()), "one undo after binding {key} must restore style_refs[\"text\"]");
        assert!(!n.variables.bindings.contains_key(key), "one undo after binding {key} must remove the binding");
    }

    // Library variable path: binding a library variable on a style-linked text layer detaches the style in the same undo step.
    let root = tmp_lib_dir("style-detach-lib");
    let mut lib_sim = Sim::new();
    lib_sim.shell.app.libs.set_root(root.clone());
    clear_all_variables_and_bindings(&mut lib_sim.shell.app);
    let mut lstore = VariableStore::default();
    let lcoll = lstore.add_collection("SharedType");
    let lmode = lstore.collections[&lcoll].default_mode.clone();
    let lsize = lstore.add_variable(&lcoll, "Type/SharedSize", VariableKind::Number).unwrap();
    lstore.set_value(&lsize, &lmode, VariableValue::Number(32.0));
    lib_sim.shell.app.commit_store(lstore, &[], &[]);
    lib_sim.shell.app.open_libraries();
    for _ in 0..3 {
        lib_sim.frame(vec![]);
    }
    lib_sim.shell.app.libs.description = "Type tokens".into();
    let pub_btn = lib_sim.rect_of("lib-publish");
    lib_sim.click(pub_btn.center());
    for _ in 0..2 {
        lib_sim.frame(vec![]);
    }
    let lib_id = lib_sim.app().doc.library.published.as_ref().unwrap().library_id.clone();

    let mut c = Sim::with(Options { select: Some("Welcome back".into()), ..Default::default() });
    let ct = c.by_name("Welcome back");
    c.shell.app.libs.set_root(root.clone());
    clear_all_variables_and_bindings(&mut c.shell.app);
    c.shell.app.doc.styles.insert(style_key.clone(), Style::Text(TextStyle::default()));
    c.shell.app.apply_style(&[ct], &style_key);
    c.shell.app.set_library_enabled(&lib_id, true);
    c.shell.app.refresh_libraries();
    c.frame(vec![]);

    assert!(c.app().doc.variables.is_empty());
    c.click(c.rect_of("bind:fontSize").center());
    c.frame(vec![]);
    c.click(c.rect_of("pick:Type/SharedSize").center());
    c.frame(vec![]);

    let cn = c.app().doc.get(ct).unwrap();
    assert!(!cn.style_refs.contains_key("text"), "library variable bind on fontSize must detach text style");
    assert!(cn.variables.bindings.contains_key("fontSize"));
    assert_eq!(c.app().doc.variables.len(), 1, "library variable must be imported");

    c.cmd_key(Key::Z);
    c.frame(vec![]);
    let cn = c.app().doc.get(ct).unwrap();
    assert_eq!(cn.style_refs.get("text").map(String::as_str), Some(style_key.as_str()), "single undo must restore text style link after library bind");
    assert!(!cn.variables.bindings.contains_key("fontSize"), "single undo must remove fontSize binding");
    assert!(c.app().doc.variables.is_empty(), "single undo must remove imported library variable");

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn inspector_var_binds_suppressed_during_active_text_edit_session() {
    let mut s = Sim::with(Options { select: Some("Welcome back".into()), ..Default::default() });
    let t_id = s.by_name("Welcome back");
    let (coll, color_id, num_id, fam_id, style_id) = add_test_collection_with_all_kinds(&mut s.shell.app);
    let mut store = VariableStore::from_doc(&s.shell.app.doc);
    let mode = store.collections[&coll].default_mode.clone();
    let size30_id = store.add_variable(&coll, "Number/Size30", VariableKind::Number).unwrap();
    store.set_value(&size30_id, &mode, VariableValue::Number(30.0));
    s.shell.app.commit_store(store, &[], &[]);

    crate::canvas::textsession::begin_text_edit(&mut s.shell.app, t_id);
    s.frame(vec![]);

    for key in ["fontFamily", "fontStyle", "fontSize", "lineHeight", "letterSpacing", "fills/0"] {
        assert!(!has_test_rect(&s, &format!("bind:{key}")), "bind:{key} must not be offered at rest during an active text editing session");
    }

    // Even when hovering the size or color fields, bind:fontSize and bind:fills/0 must remain suppressed.
    let size_center = s.field_rect("font-size").center();
    s.mv(size_center);
    assert!(!has_test_rect(&s, "bind:fontSize"), "bind:fontSize must not be offered on hover during an active text editing session");

    let hex_center = s.rect_of("text-hex").center();
    s.mv(hex_center);
    assert!(!has_test_rect(&s, "bind:fills/0"), "bind:fills/0 must not be offered on hover during an active text editing session");

    open_type_details(&mut s);
    s.frame(vec![]);
    for key in ["paragraphIndent", "paragraphSpacing"] {
        assert!(!has_test_rect(&s, &format!("bind:{key}")), "bind:{key} must not be offered in Type details during an active text editing session");
    }

    // BLOCKER-1 red test: when typography fields ARE bound, an active text-editing session must STILL draw
    // every bound pill (`var_bind::pill_field`) so the fields stay locked, while hiding only the bind icons.
    // Specifically: bind `fontSize` to 30, type 44 in a session -> stored value stays 30 and the pill is painted.
    crate::canvas::textsession::finish_text(&mut s.shell.app, true);
    s.shell.app.props.pop = None;
    s.shell.app.bind_variable(&[t_id], &["fontFamily"], &fam_id);
    s.shell.app.bind_variable(&[t_id], &["fontStyle"], &style_id);
    s.shell.app.bind_variable(&[t_id], &["fontSize"], &size30_id);
    s.shell.app.bind_variable(&[t_id], &["lineHeight"], &num_id);
    s.shell.app.bind_variable(&[t_id], &["letterSpacing"], &num_id);
    s.shell.app.bind_variable(&[t_id], &["fills/0"], &color_id);
    s.shell.app.bind_variable(&[t_id], &["paragraphIndent"], &num_id);
    s.shell.app.bind_variable(&[t_id], &["paragraphSpacing"], &num_id);
    s.frame(vec![]);

    let fs_pill_outside_session = s.rect_of("bound:fontSize");
    crate::canvas::textsession::begin_text_edit(&mut s.shell.app, t_id);
    s.frame(vec![]);

    for key in ["fontFamily", "fontStyle", "fontSize", "lineHeight", "letterSpacing", "fills/0"] {
        assert!(has_test_rect(&s, &format!("bound:{key}")), "bound:{key} pill must be painted during an active text editing session");
        assert!(!has_test_rect(&s, &format!("bind:{key}")), "bind:{key} icon must be hidden during an active text editing session");
    }

    // Attempt to click the font-size area and type "44" + Enter in the session: stored font_size must stay 30.0.
    s.click(Pos2::new(fs_pill_outside_session.left() + 16.0, fs_pill_outside_session.center().y));
    s.frame(vec![
        egui::Event::Text("44".into()),
        egui::Event::Key { key: Key::Enter, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE },
    ]);
    s.frame(vec![]);
    let NodeKind::Text(tp) = &s.app().doc.get(t_id).unwrap().kind else { panic!("expected text") };
    assert_eq!(tp.style.font_size, 30.0, "typing 44 on a bound fontSize field during a text session must keep stored font_size at 30.0");
    assert!(has_test_rect(&s, "bound:fontSize"), "bound:fontSize pill must still be painted");

    open_type_details(&mut s);
    s.frame(vec![]);
    for key in ["paragraphIndent", "paragraphSpacing"] {
        assert!(has_test_rect(&s, &format!("bound:{key}")), "bound:{key} pill must be painted in Type details during an active text editing session");
        assert!(!has_test_rect(&s, &format!("bind:{key}")), "bind:{key} icon must be hidden in Type details during an active text editing session");
    }
}

#[test]
fn inspector_var_binds_wrap_counter_axis_spacing_binds_and_detaches() {
    let mut s = Sim::with(Options { select: Some("Welcome".into()), ..Default::default() });
    let hero = s.by_name("Welcome");
    let _ = add_test_collection_with_all_kinds(&mut s.shell.app);
    s.shell.app.edit_nodes(&[hero], |n| {
        if let Some(f) = n.kind.frame_props_mut() {
            let al = f.auto_layout.get_or_insert_with(Default::default);
            al.direction = LayoutDirection::Horizontal;
            al.wrap = true;
            al.counter_spacing = 6.0;
        }
    });
    s.frame(vec![]);

    assert!(has_test_rect(&s, "bind:counterAxisSpacing"), "expected bind:counterAxisSpacing on wrap auto-layout frame");
    s.click(s.rect_of("bind:counterAxisSpacing").center());
    s.frame(vec![]);
    s.click(s.rect_of("pick:Number/Step").center());
    s.frame(vec![]);

    let n = s.app().doc.get(hero).unwrap();
    assert!(n.variables.bindings.contains_key("counterAxisSpacing"));
    assert_eq!(n.kind.frame_props().unwrap().auto_layout.as_ref().unwrap().counter_spacing, 24.0);
    // Undo removes the binding and restores 6.0.
    s.cmd_key(Key::Z);
    s.frame(vec![]);
    let n = s.app().doc.get(hero).unwrap();
    assert!(!n.variables.bindings.contains_key("counterAxisSpacing"), "undo must remove counterAxisSpacing binding");
    assert_eq!(n.kind.frame_props().unwrap().auto_layout.as_ref().unwrap().counter_spacing, 6.0);
    sim_redo(&mut s);
    s.frame(vec![]);
    let pill = s.rect_of("bound:counterAxisSpacing");
    s.click(Pos2::new(pill.right() - 13.0, pill.center().y));
    s.frame(vec![]);
    assert!(!s.app().doc.get(hero).unwrap().variables.bindings.contains_key("counterAxisSpacing"));
    assert_eq!(s.app().doc.get(hero).unwrap().kind.frame_props().unwrap().auto_layout.as_ref().unwrap().counter_spacing, 24.0);
}

#[test]
fn inspector_var_binds_per_side_stroke_weights_bind_and_detach() {
    let mut s = Sim::with(Options { select: Some("Hero".into()), ..Default::default() });
    let hero = s.by_name("Hero");
    let _ = add_test_collection_with_all_kinds(&mut s.shell.app);
    s.shell.app.edit_nodes(&[hero], |n| {
        n.stroke.paints = vec![Paint::solid(Color::BLACK)];
        n.stroke.weight = 2.0;
        n.stroke.side_weights = Some([1.0, 2.0, 3.0, 4.0]);
    });
    s.scroll_inspector_down();
    s.frame(vec![]);

    // M10: Verify 2x2 grid positions of T/R/B/L stroke side bind icons (T,R on top row; B,L on bottom row; T,B left; R,L right).
    let rt = s.rect_of("bind:strokeTopWeight");
    let rr = s.rect_of("bind:strokeRightWeight");
    let rb = s.rect_of("bind:strokeBottomWeight");
    let rl = s.rect_of("bind:strokeLeftWeight");
    assert!((rt.center().y - rr.center().y).abs() < 1.0, "Top and Right icons must share top row y: {rt:?} vs {rr:?}");
    assert!((rb.center().y - rl.center().y).abs() < 1.0, "Bottom and Left icons must share bottom row y: {rb:?} vs {rl:?}");
    assert!(rt.center().y < rb.center().y - 10.0, "Top row must sit above Bottom row: {rt:?} vs {rb:?}");
    assert!((rt.center().x - rb.center().x).abs() < 1.0, "Top and Bottom icons must share left column x: {rt:?} vs {rb:?}");
    assert!((rr.center().x - rl.center().x).abs() < 1.0, "Right and Left icons must share right column x: {rr:?} vs {rl:?}");
    assert!(rt.center().x < rr.center().x - 10.0, "Left column must sit left of Right column: {rt:?} vs {rr:?}");

    let side_keys = ["strokeTopWeight", "strokeRightWeight", "strokeBottomWeight", "strokeLeftWeight"];
    for (idx, key) in side_keys.into_iter().enumerate() {
        let orig = (idx + 1) as f32;
        let bind_name = format!("bind:{key}");
        assert!(has_test_rect(&s, &bind_name), "expected {bind_name} at rest when side_weights is active");
        s.click(s.rect_of(&bind_name).center());
        s.frame(vec![]);
        s.click(s.rect_of("pick:Number/Step").center());
        s.frame(vec![]);

        let n = s.app().doc.get(hero).unwrap();
        assert!(n.variables.bindings.contains_key(key));
        assert_eq!(n.stroke.side_weights.unwrap()[idx], 24.0);
        // Undo removes the binding and restores the original per-side weight.
        s.cmd_key(Key::Z);
        s.frame(vec![]);
        let n = s.app().doc.get(hero).unwrap();
        assert!(!n.variables.bindings.contains_key(key), "undo must remove {key} binding");
        assert_eq!(n.stroke.side_weights.unwrap()[idx], orig);
        sim_redo(&mut s);
        s.frame(vec![]);
        let pill = s.rect_of(&format!("bound:{key}"));
        s.click(Pos2::new(pill.right() - 13.0, pill.center().y));
        s.frame(vec![]);
        assert!(!s.app().doc.get(hero).unwrap().variables.bindings.contains_key(key));
        assert_eq!(s.app().doc.get(hero).unwrap().stroke.side_weights.unwrap()[idx], 24.0);
    }
}

#[test]
fn inspector_var_binds_no_overlap_between_value_field_and_bind_hitbox() {
    let mut s = Sim::with(Options { select: Some("Welcome".into()), ..Default::default() });
    let hero = s.by_name("Welcome");
    let _ = add_test_collection_with_all_kinds(&mut s.shell.app);
    s.shell.app.edit_nodes(&[hero], |n| {
        if let Some(f) = n.kind.frame_props_mut() {
            let al = f.auto_layout.get_or_insert_with(Default::default);
            al.direction = LayoutDirection::Horizontal;
            al.wrap = true;
        }
        n.stroke.paints = vec![Paint::solid(Color::BLACK)];
        n.stroke.side_weights = Some([1.0, 2.0, 3.0, 4.0]);
        n.effects = vec![Effect::DropShadow {
            color: Color::rgba(0, 0, 0, 64),
            offset: designos_doc::Vec2::new(0.0, 4.0),
            radius: 8.0,
            spread: 0.0,
            visible: true,
            show_behind: false,
            blend: Default::default(),
        }];
    });
    s.frame(vec![]);

    let assert_all_recorded_binds_min_size = |s: &Sim, context: &str| {
        let recorded = crate::widgets::last_pass_names();
        let binds: Vec<(String, Rect)> = recorded.into_iter().filter(|(n, _)| n.starts_with("bind:") || n.starts_with("style-bind:")).collect();
        assert!(!binds.is_empty(), "{context}: expected at least one bind:* widget");
        for (name, _) in &binds {
            let bind_r = s.rect_of(name);
            assert!(
                bind_r.width() >= crate::theme::TARGET_MIN && bind_r.height() >= crate::theme::TARGET_MIN,
                "{context}: {name} hit box {bind_r:?} is smaller than TARGET_MIN"
            );
        }
    };

    assert_all_recorded_binds_min_size(&s, "frame-top");
    for (field_key, bind_key) in [
        ("w", "bind:width"),
        ("h", "bind:height"),
        ("opacity", "bind:opacity"),
        ("radius", "bind:topLeftRadius"),
        ("al-gap", "bind:itemSpacing"),
        ("al-ph", "bind:paddingLeft"),
        ("al-pv", "bind:paddingTop"),
        ("al-counter-gap", "bind:counterAxisSpacing"),
    ] {
        let vr = s.field_rect(field_key);
        let br = s.rect_of(bind_key);
        assert!(!vr.intersects(br), "field {field_key} {vr:?} intersects {bind_key} {br:?}");
        assert!(
            (br.center().y - vr.center().y).abs() < 0.5 && (br.top() - (vr.top() + 4.0)).abs() < 0.5,
            "32 px inspector row {field_key} must vertically centre {bind_key} at top + 4.0 (MINOR-4): vr={vr:?}, br={br:?}"
        );
    }

    s.scroll_inspector_down();
    s.frame(vec![]);
    for (idx, bind_key) in
        [(0usize, "bind:strokeTopWeight"), (1usize, "bind:strokeRightWeight"), (2usize, "bind:strokeBottomWeight"), (3usize, "bind:strokeLeftWeight")]
    {
        let vr = field_rect_any(&s, ("stroke-side", idx));
        let br = s.rect_of(bind_key);
        assert!(!vr.intersects(br), "stroke side field {idx} {vr:?} intersects {bind_key} {br:?}");
    }

    let minus0 = s.rect_of("fx-minus-0");
    s.click(Pos2::new(minus0.left() - 52.0, minus0.center().y));
    s.frame(vec![]);
    assert_all_recorded_binds_min_size(&s, "frame-scrolled-with-fx-popover");
    // M14: Check painted field_rect against icon rect on all effect popover rows.
    for (fx_field, bind_key) in [
        (("fxp-x", 0usize), "bind:effects/0/offsetX"),
        (("fxp-y", 0usize), "bind:effects/0/offsetY"),
        (("fxp-b", 0usize), "bind:effects/0/radius"),
        (("fxp-s", 0usize), "bind:effects/0/spread"),
    ] {
        let vr = field_rect_any(&s, fx_field);
        let br = s.rect_of(bind_key);
        assert!(!vr.intersects(br), "effect field {fx_field:?} {vr:?} intersects {bind_key} {br:?} (M14)");
    }

    // Select a text layer and open Type details.
    let welcome = s.by_name("Welcome back");
    s.shell.app.selection = vec![welcome];
    s.shell.app.props.pop = None;
    s.shell.app.props.scroll = 0.0;
    s.frame(vec![]);
    open_type_details(&mut s);
    s.frame(vec![]);
    assert_all_recorded_binds_min_size(&s, "text-with-type-details");
    for (field_key, bind_key) in [
        ("font-family", "bind:fontFamily"),
        ("font-size", "bind:fontSize"),
        ("line-height", "bind:lineHeight"),
        ("letter-spacing", "bind:letterSpacing"),
        ("det-indent", "bind:paragraphIndent"),
        ("det-pspacing", "bind:paragraphSpacing"),
    ] {
        let vr = s.field_rect(field_key);
        let br = s.rect_of(bind_key);
        assert!(!vr.intersects(br), "text field {field_key} {vr:?} intersects {bind_key} {br:?}");
    }
    let text_hex = s.rect_of("text-hex");
    let text_fill_bind = s.rect_of("bind:fills/0");
    assert!(!text_hex.intersects(text_fill_bind), "text-hex {text_hex:?} intersects bind:fills/0 {text_fill_bind:?}");

    // M15 + MINOR-4: Style editor rows (32 px `ROW_HEIGHT`) — verify painted field_rect does not intersect
    // style-bind icon rect on text style editor (including font-family) and that the 24 px bind icon is
    // vertically centred on each 32 px row (`br.top() == vr.top() + 4.0`).
    s.shell.app.props.pop = None;
    s.shell.app.doc.styles.insert("Text/Heading".into(), Style::Text(TextStyle::default()));
    s.shell.app.props.style_edit = Some(crate::panels::style_edit::StyleEdit {
        key: "Text/Heading".into(),
        anchor: Rect::from_min_size(Pos2::new(1200.0, 200.0), Vec2::splat(24.0)),
        item: 0,
        pending: None,
    });
    s.frame(vec![]);
    assert_all_recorded_binds_min_size(&s, "style-edit-text");
    for (field_key, bind_key) in [
        ("font-family", "style-bind:Text/Heading:fontFamily"),
        ("se-weight", "style-bind:Text/Heading:fontWeight"),
        ("se-size", "style-bind:Text/Heading:fontSize"),
        ("se-line-height", "style-bind:Text/Heading:lineHeight"),
        ("se-letter-spacing", "style-bind:Text/Heading:letterSpacing"),
    ] {
        let vr = s.field_rect(field_key);
        let br = s.rect_of(bind_key);
        assert!(!vr.intersects(br), "style editor field {field_key} {vr:?} intersects {bind_key} {br:?} (M15)");
        assert!((br.center().y - vr.center().y).abs() < 0.5, "style editor row {field_key} must vertically centre {bind_key}: vr={vr:?}, br={br:?}");
    }
}

#[test]
fn inspector_var_binds_frame_allocations_with_700_library_variables_within_ten_percent() {
    let measure_median_allocs = |s: &mut Sim| -> usize {
        for _ in 0..3 {
            s.frame(vec![]);
        }
        let _guard = crate::widgets::RecordDisabled::begin();
        let mut counts = Vec::with_capacity(15);
        for _ in 0..15 {
            let ((), n_alloc) = crate::test_alloc_count::allocations_in(|| {
                s.frame(vec![]);
            });
            counts.push(n_alloc);
        }
        counts.sort_unstable();
        counts[counts.len() / 2]
    };

    // Baseline frame allocations on a document with no variables (matches main's fast path).
    let mut base_sim = Sim::with(Options { select: Some("Hero".into()), ..Default::default() });
    clear_all_variables_and_bindings(&mut base_sim.shell.app);
    let base_allocs = measure_median_allocs(&mut base_sim);

    // Publish a library with 700 Color variables and enable it on the same document.
    let root = tmp_lib_dir("perf-700-vars");
    let mut lib_sim = Sim::new();
    lib_sim.shell.app.libs.set_root(root.clone());
    clear_all_variables_and_bindings(&mut lib_sim.shell.app);
    let mut lstore = VariableStore::default();
    let lcoll = lstore.add_collection("PerfPalette");
    let lmode = lstore.collections[&lcoll].default_mode.clone();
    for i in 0..700 {
        let vid = lstore.add_variable(&lcoll, &format!("Color/Token{i:03}"), VariableKind::Color).unwrap();
        lstore.set_value(&vid, &lmode, VariableValue::Color(Color::rgb((i % 255) as u8, ((i * 7) % 255) as u8, 128)));
    }
    lib_sim.shell.app.commit_store(lstore, &[], &[]);
    assert!(lib_sim.shell.app.publish_file(), "publish 700-variable library");
    let lib_id = lib_sim.app().doc.library.published.as_ref().unwrap().library_id.clone();

    let mut lib_consumer = Sim::with(Options { select: Some("Hero".into()), ..Default::default() });
    lib_consumer.shell.app.libs.set_root(root.clone());
    clear_all_variables_and_bindings(&mut lib_consumer.shell.app);
    lib_consumer.shell.app.set_library_enabled(&lib_id, true);
    lib_consumer.shell.app.refresh_libraries();
    let lib_allocs = measure_median_allocs(&mut lib_consumer);

    let _ = std::fs::remove_dir_all(root);
    let budget = (base_allocs * 110) / 100;
    assert!(
        lib_allocs <= budget,
        "MAJOR-2: frame allocations with 700-variable enabled library ({lib_allocs}) exceeded +10% budget ({budget}) over baseline ({base_allocs})"
    );
}
