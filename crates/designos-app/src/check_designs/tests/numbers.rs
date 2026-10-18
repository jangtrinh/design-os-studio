//! Literal fixtures for hard-coded number variables in Check designs (#154): gap, padding, corner
//! radius, stroke weight, font size and line height, plus scope filtering, ambiguity and undo.

use super::*;
use designos_doc::{AutoLayout, LineHeight, Paint, TextRun};

impl Fx {
    /// A number variable with one value per mode of the shared "Theme" collection (light default, dark)
    /// and the given Figma `scopes` (empty = all scopes).
    fn num_var(&mut self, name: &str, light: f32, dark: f32, scopes: &[&str]) -> String {
        let coll = self.doc.variable_collections.entry("theme".into()).or_insert_with(|| VariableCollection {
            id: "theme".into(),
            name: "Theme".into(),
            modes: vec![VariableMode { id: "light".into(), name: "Light".into() }, VariableMode { id: "dark".into(), name: "Dark".into() }],
            default_mode: "light".into(),
            variables: vec![],
            key: None,
            source_library_key: None,
        });
        let id = format!("var:{name}");
        if !coll.variables.contains(&id) {
            coll.variables.push(id.clone());
        }
        let values = [("light".to_string(), VariableValue::Number(light)), ("dark".to_string(), VariableValue::Number(dark))].into_iter().collect();
        self.doc.variables.insert(
            id.clone(),
            Variable {
                id: id.clone(),
                name: name.into(),
                collection: "theme".into(),
                kind: VariableKind::Number,
                values,
                description: String::new(),
                scopes: scopes.iter().map(|s| (*s).to_string()).collect(),
                hidden_from_publishing: false,
                key: None,
                code_syntax: Default::default(),
            },
        );
        id
    }
}

#[test]
fn gap_16_matching_space_4_is_flagged_with_bind_fix() {
    let mut f = fx();
    let row = f.frame(f.page, "Row", None);
    f.doc.get_mut(row).unwrap().kind =
        NodeKind::Frame(FrameProps { auto_layout: Some(AutoLayout { item_spacing: 16.0, ..AutoLayout::v1() }), ..FrameProps::default() });
    let var = f.num_var("space/4", 16.0, 16.0, &["GAP"]);
    let found = only(&f.report(), Rule::HardCodedValue);
    assert_eq!(found.node, row);
    assert_eq!(found.fix, Some(Fix::BindVariable { key: "itemSpacing".into(), variable: var.clone() }));
    assert_eq!(found.message, "Gap 16 matches variable \"space/4\"");
    assert_eq!(found.fix_label, "Bind to variable space/4");

    let mut app = f.app();
    let before = snapshot(&app, &[row]);
    assert!(app.check_designs_fix(&found));
    assert_eq!(app.doc.get(row).unwrap().variables.bindings.get("itemSpacing"), Some(&var));
    assert!(audit(&app.doc, &Scope::Page(app.page)).findings.is_empty(), "the finding is gone after the fix");
    app.undo();
    assert_eq!(snapshot(&app, &[row]), before);
    assert!(!app.hist.can_undo(), "exactly one undo step");
}

#[test]
fn each_number_property_is_covered() {
    let mut f = fx();
    let white = f.frame(f.page, "Surface", Some(Color::WHITE));
    let card = f.frame(white, "Card", None);
    {
        let n = f.doc.get_mut(card).unwrap();
        n.corner_radii = [8.0, 12.0, 0.0, 0.0];
        n.stroke.weight = 2.0;
        n.stroke.paints = vec![Paint::solid(BLUE)];
        n.kind = NodeKind::Frame(FrameProps {
            auto_layout: Some(AutoLayout { item_spacing: 16.0, counter_spacing: 10.0, padding: [24.0, 6.0, 0.0, 0.0], ..AutoLayout::v1() }),
            ..FrameProps::default()
        });
    }
    let t = f.text(white, "Heading", 20.0, 600, gray(0x11));
    if let NodeKind::Text(tp) = &mut f.doc.get_mut(t).unwrap().kind {
        tp.style.line_height = LineHeight::Pixels(28.0);
    }

    let v_r8 = f.num_var("radius/md", 8.0, 8.0, &["CORNER_RADIUS"]);
    let v_r12 = f.num_var("radius/lg", 12.0, 12.0, &["CORNER_RADIUS"]);
    let v_p24 = f.num_var("space/6", 24.0, 24.0, &["GAP"]);
    let v_p6 = f.num_var("space/1_5", 6.0, 6.0, &["GAP"]);
    let v_gap = f.num_var("space/4", 16.0, 16.0, &["GAP"]);
    let v_rgap = f.num_var("space/2_5", 10.0, 10.0, &["GAP"]);
    let v_stroke = f.num_var("border/thick", 2.0, 2.0, &["STROKE_FLOAT"]);
    let v_fs = f.num_var("font/size/lg", 20.0, 20.0, &["FONT_SIZE"]);
    let v_lh = f.num_var("font/line/lg", 28.0, 28.0, &["LINE_HEIGHT"]);

    let report = f.report();
    let messages: Vec<&str> = report.of(Rule::HardCodedValue).map(|x| x.message.as_str()).collect();
    assert_eq!(
        messages,
        vec![
            "Gap 16 matches variable \"space/4\"",
            "Row gap 10 matches variable \"space/2_5\"",
            "Top padding 24 matches variable \"space/6\"",
            "Right padding 6 matches variable \"space/1_5\"",
            "Top-left radius 8 matches variable \"radius/md\"",
            "Top-right radius 12 matches variable \"radius/lg\"",
            "Stroke weight 2 matches variable \"border/thick\"",
            "Font size 20 matches variable \"font/size/lg\"",
            "Line height 28 matches variable \"font/line/lg\"",
        ]
    );

    let mut app = f.app();
    assert_eq!(app.check_designs_fix_all(&report.findings), 9);
    let cn = app.doc.get(card).unwrap();
    assert_eq!(cn.variables.bindings.get("itemSpacing"), Some(&v_gap));
    assert_eq!(cn.variables.bindings.get("counterAxisSpacing"), Some(&v_rgap));
    assert_eq!(cn.variables.bindings.get("paddingTop"), Some(&v_p24));
    assert_eq!(cn.variables.bindings.get("paddingRight"), Some(&v_p6));
    assert_eq!(cn.variables.bindings.get("topLeftRadius"), Some(&v_r8));
    assert_eq!(cn.variables.bindings.get("topRightRadius"), Some(&v_r12));
    assert_eq!(cn.variables.bindings.get("strokeWeight"), Some(&v_stroke));
    let tn = app.doc.get(t).unwrap();
    assert_eq!(tn.variables.bindings.get("fontSize"), Some(&v_fs));
    assert_eq!(tn.variables.bindings.get("lineHeight"), Some(&v_lh));
    assert!(audit(&app.doc, &Scope::Page(app.page)).findings.is_empty());
}

#[test]
fn grouped_padding_fix_binds_four_keys_in_one_undo() {
    let mut f = fx();
    let card = f.frame(f.page, "Card", None);
    f.doc.get_mut(card).unwrap().kind =
        NodeKind::Frame(FrameProps { auto_layout: Some(AutoLayout { padding: [24.0; 4], ..AutoLayout::v1() }), ..FrameProps::default() });
    let var24 = f.num_var("space/6", 24.0, 24.0, &["GAP"]);
    let found = only(&f.report(), Rule::HardCodedValue);
    assert_eq!(found.node, card);
    assert_eq!(found.message, "Padding 24 matches variable \"space/6\"");
    assert_eq!(
        found.fix,
        Some(Fix::BindVariables {
            keys: vec!["paddingTop".into(), "paddingRight".into(), "paddingBottom".into(), "paddingLeft".into()],
            variable: var24.clone(),
        })
    );

    let mut app = f.app();
    let before = snapshot(&app, &[card]);
    assert!(app.check_designs_fix(&found));
    let n = app.doc.get(card).unwrap();
    for k in ["paddingTop", "paddingRight", "paddingBottom", "paddingLeft"] {
        assert_eq!(n.variables.bindings.get(k), Some(&var24), "{k} should be bound");
    }
    assert!(audit(&app.doc, &Scope::Page(app.page)).findings.is_empty());
    app.undo();
    assert_eq!(snapshot(&app, &[card]), before);
    assert!(!app.hist.can_undo(), "exactly one undo step");
}

#[test]
fn uniform_and_per_corner_radius_match_number_variables_and_bind_cleanly() {
    let mut f = fx();
    let r = f.rect(f.page, "Chip", BLUE);
    f.doc.get_mut(r).unwrap().corner_radii = [12.0; 4];
    let var12 = f.num_var("radius/lg", 12.0, 12.0, &["CORNER_RADIUS"]);
    let found = only(&f.report(), Rule::HardCodedValue);
    assert_eq!(found.node, r);
    assert_eq!(found.message, "Corner radius 12 matches variable \"radius/lg\"");
    assert_eq!(
        found.fix,
        Some(Fix::BindVariables {
            keys: vec!["topLeftRadius".into(), "topRightRadius".into(), "bottomRightRadius".into(), "bottomLeftRadius".into()],
            variable: var12.clone(),
        })
    );

    let mut app = f.app();
    let before = snapshot(&app, &[r]);
    assert!(app.check_designs_fix(&found));
    let n = app.doc.get(r).unwrap();
    for k in ["topLeftRadius", "topRightRadius", "bottomRightRadius", "bottomLeftRadius"] {
        assert_eq!(n.variables.bindings.get(k), Some(&var12), "{k} should be bound");
    }
    assert!(audit(&app.doc, &Scope::Page(app.page)).findings.is_empty());
    app.undo();
    assert_eq!(snapshot(&app, &[r]), before);
}

#[test]
fn percent_line_height_is_skipped() {
    let mut f = fx();
    let white = f.frame(f.page, "Surface", Some(Color::WHITE));
    let t = f.text(white, "Body", 16.0, 400, gray(0x11));
    if let NodeKind::Text(tp) = &mut f.doc.get_mut(t).unwrap().kind {
        tp.style.line_height = LineHeight::Percent(150.0);
    }
    f.num_var("line/percent", 150.0, 150.0, &["LINE_HEIGHT"]);
    assert!(f.report().findings.is_empty(), "{:#?}", f.report().findings);
}

#[test]
fn text_linked_to_a_style_or_with_mixed_run_sizes_is_not_flagged_for_number_variables() {
    let mut f = fx();
    let white = f.frame(f.page, "Surface", Some(Color::WHITE));
    let linked = f.text(white, "Linked", 20.0, 600, gray(0x11));
    f.style("H3", "Heading/H3", Style::Text(TextStyle { font_size: 20.0, font_weight: 600, ..TextStyle::default() }), StyleProperty::Text);
    f.doc.get_mut(linked).unwrap().style_refs.insert("text".into(), "H3".into());

    let mixed = f.text(white, "Mixed", 20.0, 600, gray(0x11));
    if let NodeKind::Text(tp) = &mut f.doc.get_mut(mixed).unwrap().kind {
        tp.runs = vec![TextRun { start: 0, end: 2, style: TextStyle { font_size: 14.0, ..tp.style.clone() } }];
    }
    f.num_var("font/size/lg", 20.0, 20.0, &["FONT_SIZE"]);
    assert!(f.report().findings.is_empty(), "{:#?}", f.report().findings);
}

#[test]
fn scope_excludes_wrong_property() {
    let mut f = fx();
    let row = f.frame(f.page, "Row", None);
    f.doc.get_mut(row).unwrap().kind =
        NodeKind::Frame(FrameProps { auto_layout: Some(AutoLayout { item_spacing: 16.0, ..AutoLayout::v1() }), ..FrameProps::default() });
    f.doc.get_mut(row).unwrap().corner_radii = [15.0; 4];
    // 16.0 variable scoped to CORNER_RADIUS must not suggest on itemSpacing, and 15.0 radius does not match 16.0.
    f.num_var("radius/xl", 16.0, 16.0, &["CORNER_RADIUS"]);
    assert!(f.report().findings.is_empty(), "{:#?}", f.report().findings);
}

#[test]
fn ambiguous_number_match_has_no_fix() {
    let mut f = fx();
    let row = f.frame(f.page, "Row", None);
    f.doc.get_mut(row).unwrap().kind =
        NodeKind::Frame(FrameProps { auto_layout: Some(AutoLayout { item_spacing: 16.0, ..AutoLayout::v1() }), ..FrameProps::default() });
    f.num_var("space/4", 16.0, 16.0, &["GAP"]);
    f.num_var("space/md", 16.0, 16.0, &["GAP"]);
    let found = only(&f.report(), Rule::HardCodedValue);
    assert_eq!(found.fix, None);
    assert!(found.fix_label.is_empty());
    assert_eq!(found.message, "Gap 16 matches 2 variables: \"space/4\", \"space/md\"");

    // When one variable holds 16 in every mode and the other changes in dark mode, the constant one wins.
    let mut f = fx();
    let row = f.frame(f.page, "Row", None);
    f.doc.get_mut(row).unwrap().kind =
        NodeKind::Frame(FrameProps { auto_layout: Some(AutoLayout { item_spacing: 16.0, ..AutoLayout::v1() }), ..FrameProps::default() });
    f.num_var("space/responsive", 16.0, 24.0, &["GAP"]);
    let steady = f.num_var("space/4", 16.0, 16.0, &["GAP"]);
    let found = only(&f.report(), Rule::HardCodedValue);
    assert_eq!(found.fix, Some(Fix::BindVariable { key: "itemSpacing".into(), variable: steady }));
}

#[test]
fn stale_number_fix_is_refused() {
    let mut f = fx();
    let card = f.frame(f.page, "Card", None);
    f.doc.get_mut(card).unwrap().kind =
        NodeKind::Frame(FrameProps { auto_layout: Some(AutoLayout { item_spacing: 16.0, padding: [24.0; 4], ..AutoLayout::v1() }), ..FrameProps::default() });
    f.doc.get_mut(card).unwrap().corner_radii = [8.0; 4];
    f.num_var("space/4", 16.0, 16.0, &["GAP"]);
    f.num_var("space/6", 24.0, 24.0, &["GAP"]);
    f.num_var("radius/md", 8.0, 8.0, &["CORNER_RADIUS"]);
    let mut app = f.app();
    let findings = audit(&app.doc, &Scope::Page(app.page)).findings;
    assert_eq!(findings.len(), 3, "{findings:#?}");

    // Edit gap, one padding side, and one corner radius after the audit: all three fixes must refuse.
    let n = app.doc.get_mut(card).unwrap();
    n.corner_radii[2] = 4.0;
    if let Some(al) = n.kind.frame_props_mut().and_then(|fp| fp.auto_layout.as_mut()) {
        al.item_spacing = 20.0;
        al.padding[1] = 12.0;
    }
    for finding in &findings {
        assert!(!app.check_designs_fix(finding), "stale finding must be refused: {finding:?}");
    }
    assert!(app.doc.get(card).unwrap().variables.bindings.is_empty());
    assert!(!app.hist.can_undo());
}
