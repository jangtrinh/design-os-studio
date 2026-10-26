//! Reference forms `apply_spec` accepts beyond the E15-3 table: bound `width` / `height` / `visible` /
//! `text`, `grid_style`, style keys (`style:<key>`, `style:<library id>/<key>`), library-scoped variable
//! ids, and the id-qualified `valid_options` of an ambiguous variable path. These are the forms the
//! design-system search produces, so each one must apply as pasted.

use crate::spec_bind::fixtures;
use crate::*;
use std::collections::BTreeMap;

fn spec(v: serde_json::Value) -> NodeSpec {
    serde_json::from_value(v).unwrap()
}

fn add_variable(doc: &mut Document, coll_id: &str, coll_name: &str, var_id: &str, name: &str, kind: VariableKind, value: VariableValue) {
    let coll = doc.variable_collections.entry(coll_id.to_string()).or_insert_with(|| VariableCollection {
        extensions: Default::default(),
        id: coll_id.to_string(),
        name: coll_name.to_string(),
        modes: vec![VariableMode { id: "m1".into(), name: "Default".into() }],
        default_mode: "m1".into(),
        variables: Vec::new(),
        key: None,
        source_library_key: None,
    });
    coll.variables.push(var_id.to_string());
    doc.variables.insert(
        var_id.to_string(),
        Variable {
            extensions: Default::default(),
            mode_extensions: Default::default(),
            id: var_id.to_string(),
            name: name.to_string(),
            collection: coll_id.to_string(),
            kind,
            values: BTreeMap::from([("m1".to_string(), value)]),
            description: String::new(),
            scopes: Vec::new(),
            hidden_from_publishing: false,
            key: None,
            code_syntax: BTreeMap::new(),
        },
    );
}

fn add_style(doc: &mut Document, key: &str, name: &str, property: StyleProperty, style: Style) {
    doc.styles.insert(key.to_string(), style);
    doc.style_meta.insert(
        key.to_string(),
        StyleMeta { name: name.to_string(), description: String::new(), property, key: None, bindings: Default::default(), ..Default::default() },
    );
}

fn text_style(size: f32) -> Style {
    Style::Text(TextStyle { font_size: size, ..Default::default() })
}

fn solid() -> Style {
    Style::Fill(vec![Paint::solid(Color::rgb(0x10, 0x20, 0x30))])
}

fn tokens_with_copy_flags_and_sizes() -> Document {
    let mut doc = fixtures::tokens_doc();
    add_variable(&mut doc, "coll_copy", "Copy", "var_cta", "Cta", VariableKind::String, VariableValue::String("Get Started".into()));
    add_variable(&mut doc, "coll_flags", "Flags", "var_hidden", "Hidden", VariableKind::Boolean, VariableValue::Boolean(false));
    add_variable(&mut doc, "coll_size", "Size", "var_wide", "Wide", VariableKind::Number, VariableValue::Number(320.0));
    add_variable(&mut doc, "coll_size", "Size", "var_tall", "Tall", VariableKind::Number, VariableValue::Number(180.0));
    doc
}

fn library_with(id: &str, build: impl FnOnce(&mut Document)) -> LibrarySnapshot {
    let mut lib = Document::new(id);
    lib.add_page("Page 1");
    build(&mut lib);
    snapshot(&lib, id, id, 1)
}

fn enable(doc: &mut Document, snap: &LibrarySnapshot) {
    enable_library(doc, snap).apply(doc).unwrap();
}

#[test]
fn binds_width_and_height_and_fixes_auto_layout_sizing() {
    let mut doc = tokens_with_copy_flags_and_sizes();
    let page = doc.pages[0];
    let s = spec(serde_json::json!({
        "kind": "frame",
        "layout": { "direction": "row" },
        "width": "var:Size/Wide",
        "height": "var:Size/Tall"
    }));
    let report = doc.apply_spec_with_report(page, &s).unwrap();
    let node = doc.get(report.root_node_id).unwrap();
    assert_eq!(node.variables.bindings.get("width").map(String::as_str), Some("var_wide"));
    assert_eq!(node.variables.bindings.get("height").map(String::as_str), Some("var_tall"));
    assert_eq!((node.size.x, node.size.y), (320.0, 180.0), "bound size is applied and a hugging auto-layout does not shrink it");
    let al = node.kind.frame_props().unwrap().auto_layout.as_ref().unwrap();
    assert_eq!((al.primary_sizing, al.counter_sizing), (Sizing::Fixed, Sizing::Fixed));
    assert_eq!(report.bindings_created, 2);

    let exported = doc.export_spec(report.root_node_id).unwrap();
    assert_eq!(exported.width, Some(NumberOrVar::Ref("var:Size/Wide".into())));
    assert_eq!(exported.height, Some(NumberOrVar::Ref("var:Size/Tall".into())));
}

#[test]
fn width_variable_of_the_wrong_kind_is_a_type_mismatch() {
    let mut doc = tokens_with_copy_flags_and_sizes();
    let page = doc.pages[0];
    let err = doc.apply_spec_with_report(page, &spec(serde_json::json!({ "kind": "frame", "width": "var:Copy/Cta" }))).unwrap_err();
    assert_eq!(err.code, "VARIABLE_TYPE_MISMATCH");
    assert_eq!(err.path, "spec.width");
}

#[test]
fn text_var_binds_characters_and_names_from_value() {
    let mut doc = tokens_with_copy_flags_and_sizes();
    let page = doc.pages[0];

    let report = doc.apply_spec_with_report(page, &spec(serde_json::json!({ "kind": "text", "text": "var:Copy/Cta" }))).unwrap();
    let node = doc.get(report.root_node_id).unwrap();
    let NodeKind::Text(tp) = &node.kind else { panic!("expected a text node") };
    assert_eq!(tp.content, "Get Started");
    assert_eq!(node.name, "Get Started", "the name comes from the resolved value, never from the var: string");
    assert_eq!(node.variables.bindings.get("characters").map(String::as_str), Some("var_cta"));
    assert_eq!(node.size.x, fitted_width(tp), "the node is sized for the resolved text, not for an empty string");
    assert!(node.size.x > 20.0);

    let exported = doc.export_spec(report.root_node_id).unwrap();
    assert_eq!(exported.text.as_deref(), Some("var:Copy/Cta"));

    // The label a button preset generates takes the same binding.
    let report = doc.apply_spec_with_report(page, &spec(serde_json::json!({ "preset": "easeui-button-primary", "text": "var:Copy/Cta" }))).unwrap();
    let frame = doc.get(report.root_node_id).unwrap();
    let label = doc.get(frame.children[0]).unwrap();
    let NodeKind::Text(ltp) = &label.kind else { panic!("expected a text label") };
    assert_eq!(ltp.content, "Get Started");
    assert_eq!(label.variables.bindings.get("characters").map(String::as_str), Some("var_cta"));
    assert!(!label.name.contains("var:") && !frame.name.contains("var:"));
    assert_eq!(label.size.x, fitted_width(ltp));
    assert!(label.size.x > 20.0);
}

fn fitted_width(tp: &TextProps) -> f32 {
    crate::spec::estimate_intrinsic_text_size(tp).x
}

#[test]
fn visible_var_binds_boolean() {
    let mut doc = tokens_with_copy_flags_and_sizes();
    let page = doc.pages[0];

    let report = doc.apply_spec_with_report(page, &spec(serde_json::json!({ "kind": "rectangle", "visible": "var:Flags/Hidden" }))).unwrap();
    let node = doc.get(report.root_node_id).unwrap();
    assert_eq!(node.variables.bindings.get("visible").map(String::as_str), Some("var_hidden"));
    assert!(!node.visible, "the bound Boolean value (false) is applied");
    let exported = doc.export_spec(report.root_node_id).unwrap();
    assert_eq!(exported.visible, Some(BoolOrVar::Ref("var:Flags/Hidden".into())));

    let report = doc.apply_spec_with_report(page, &spec(serde_json::json!({ "kind": "rectangle", "visible": false }))).unwrap();
    assert!(!doc.get(report.root_node_id).unwrap().visible);
    assert_eq!(doc.export_spec(report.root_node_id).unwrap().visible, Some(BoolOrVar::Bool(false)));

    let err = doc.apply_spec_with_report(page, &spec(serde_json::json!({ "kind": "rectangle", "visible": "var:Size/Wide" }))).unwrap_err();
    assert_eq!(err.code, "VARIABLE_TYPE_MISMATCH");
}

#[test]
fn grid_style_links_by_name() {
    let mut doc = fixtures::tokens_doc();
    add_style(&mut doc, "grid_12", "Grid12Col", StyleProperty::Grid, Style::Grid(Vec::new()));
    let page = doc.pages[0];

    let report = doc.apply_spec_with_report(page, &spec(serde_json::json!({ "kind": "frame", "grid_style": "Grid12Col" }))).unwrap();
    assert_eq!(report.styles_linked, 1);
    let node = doc.get(report.root_node_id).unwrap();
    assert_eq!(node.style_refs.get("grid").map(String::as_str), Some("grid_12"));
    assert_eq!(doc.export_spec(report.root_node_id).unwrap().grid_style.as_deref(), Some("Grid12Col"));

    let err = doc.apply_spec_with_report(page, &spec(serde_json::json!({ "kind": "frame", "grid_style": "Heading L" }))).unwrap_err();
    assert_eq!(err.code, "STYLE_TYPE_MISMATCH");
}

#[test]
fn style_ref_by_local_key_and_library_scoped_key() {
    let brand = library_with("lib-brand", |lib| add_style(lib, "s_heading", "Heading L", StyleProperty::Text, text_style(36.0)));
    let mut doc = Document::new("Consumer");
    let page = doc.add_page("Page 1");
    add_style(&mut doc, "z_local_heading", "Heading L", StyleProperty::Text, text_style(24.0));
    enable(&mut doc, &brand);

    // The key of a local style that shares its name with a library style.
    let report = doc
        .apply_spec_with_libraries(
            page,
            &spec(serde_json::json!({ "kind": "text", "text": "Local", "text_style": "style:z_local_heading" })),
            std::slice::from_ref(&brand),
        )
        .unwrap();
    assert_eq!(doc.get(report.root_node_id).unwrap().style_refs.get("text").map(String::as_str), Some("z_local_heading"));

    // The library-scoped key before the style was ever imported.
    assert!(!doc.styles.contains_key("lib-brand/s_heading"));
    let report = doc
        .apply_spec_with_libraries(
            page,
            &spec(serde_json::json!({ "kind": "text", "text": "Brand", "text_style": "style:lib-brand/s_heading" })),
            std::slice::from_ref(&brand),
        )
        .unwrap();
    let node = doc.get(report.root_node_id).unwrap();
    assert_eq!(node.style_refs.get("text").map(String::as_str), Some("lib-brand/s_heading"));
    let NodeKind::Text(tp) = &node.kind else { panic!("expected a text node") };
    assert_eq!(tp.style.font_size, 36.0);

    // The same reference once the style is imported resolves by its local key.
    let report = doc
        .apply_spec_with_libraries(
            page,
            &spec(serde_json::json!({ "kind": "text", "text": "Again", "text_style": "style:lib-brand/s_heading" })),
            std::slice::from_ref(&brand),
        )
        .unwrap();
    assert_eq!(doc.get(report.root_node_id).unwrap().style_refs.get("text").map(String::as_str), Some("lib-brand/s_heading"));

    // A key that exists nowhere is still an UNKNOWN_STYLE with the closest names.
    let err = doc.apply_spec_with_libraries(page, &spec(serde_json::json!({ "kind": "text", "text_style": "style:lib-brand/nope" })), &[brand]).unwrap_err();
    assert_eq!(err.code, "UNKNOWN_STYLE");
    assert!(!err.valid_options.is_empty());
}

#[test]
fn stroke_style_ref_accepts_stroke_property_fill_style() {
    let mut doc = Document::new("Consumer");
    let page = doc.add_page("Page 1");
    add_style(&mut doc, "stroke_subtle", "Subtle", StyleProperty::Stroke, solid());
    add_style(&mut doc, "fill_brand", "Brand", StyleProperty::Fill, solid());

    let report =
        doc.apply_spec_with_report(page, &spec(serde_json::json!({ "kind": "rectangle", "stroke": "style:Subtle", "fill": "style:stroke_subtle" }))).unwrap();
    let node = doc.get(report.root_node_id).unwrap();
    assert_eq!(node.style_refs.get("stroke").map(String::as_str), Some("stroke_subtle"));
    assert_eq!(node.style_refs.get("fill").map(String::as_str), Some("stroke_subtle"), "a colour style serves either side");

    let report = doc.apply_spec_with_report(page, &spec(serde_json::json!({ "kind": "rectangle", "stroke": "style:Brand" }))).unwrap();
    assert_eq!(doc.get(report.root_node_id).unwrap().style_refs.get("stroke").map(String::as_str), Some("fill_brand"));

    let err = doc.apply_spec_with_report(page, &spec(serde_json::json!({ "kind": "rectangle", "stroke": "style:Nope" }))).unwrap_err();
    assert_eq!(err.code, "UNKNOWN_STYLE");
}

fn accent_library() -> LibrarySnapshot {
    library_with("brand", |lib| {
        add_variable(lib, "coll_brand", "Brand", "var_accent", "Accent", VariableKind::Color, VariableValue::Color(Color::rgb(0x12, 0x34, 0x56)));
    })
}

#[test]
fn library_scoped_var_id_imports_before_first_use() {
    let brand = accent_library();
    let mut doc = fixtures::tokens_doc();
    // A local variable with the same raw id: the library-scoped form must still reach the library.
    add_variable(&mut doc, "coll_local", "Local", "var_accent", "Accent", VariableKind::Color, VariableValue::Color(Color::rgb(0xFF, 0, 0)));
    enable(&mut doc, &brand);
    let page = doc.pages[0];
    assert!(!doc.variables.contains_key("brand/var_accent"));

    let report = doc
        .apply_spec_with_libraries(page, &spec(serde_json::json!({ "kind": "rectangle", "fill": "var:id:brand/var_accent" })), std::slice::from_ref(&brand))
        .unwrap();
    let node = doc.get(report.root_node_id).unwrap();
    assert_eq!(node.variables.bindings.get("fills/0").map(String::as_str), Some("brand/var_accent"));
    assert!(doc.variables.contains_key("brand/var_accent"), "the library variable was imported");

    let report = doc
        .apply_spec_with_libraries(page, &spec(serde_json::json!({ "kind": "rectangle", "fill": "var:id:var_accent" })), std::slice::from_ref(&brand))
        .unwrap();
    assert_eq!(
        doc.get(report.root_node_id).unwrap().variables.bindings.get("fills/0").map(String::as_str),
        Some("var_accent"),
        "the bare id keeps the local variable"
    );

    let err = doc.apply_spec_with_libraries(page, &spec(serde_json::json!({ "kind": "rectangle", "fill": "var:id:brand/nope" })), &[brand]).unwrap_err();
    assert_eq!(err.code, "UNKNOWN_VARIABLE");
}

#[test]
fn name_match_prefers_local_style_over_disabled_library_copy() {
    let icons = library_with("a-icons", |lib| add_style(lib, "s_heading", "Heading L", StyleProperty::Text, text_style(12.0)));
    let brand = library_with("z-brand", |lib| add_style(lib, "s_heading", "Heading L", StyleProperty::Text, text_style(36.0)));
    let mut doc = Document::new("Consumer");
    let page = doc.add_page("Page 1");
    for snap in [&icons, &brand] {
        enable(&mut doc, snap);
        import_asset(&mut doc, snap, "style:s_heading", None).apply(&mut doc).unwrap();
    }
    disable_library(&doc, "a-icons").apply(&mut doc).unwrap();
    let libs = std::slice::from_ref(&brand);
    let key_of = |doc: &mut Document, name: &str| {
        let r = doc.apply_spec_with_libraries(page, &spec(serde_json::json!({ "kind": "text", "text_style": name })), libs).unwrap();
        doc.get(r.root_node_id).unwrap().style_refs.get("text").cloned().unwrap()
    };

    // `a-icons/s_heading` sorts first, but its library is disabled: the enabled copy wins.
    assert_eq!(key_of(&mut doc, "Heading L"), "z-brand/s_heading");

    // A local style of the same name wins over both imported copies.
    add_style(&mut doc, "z_local_heading", "Heading L", StyleProperty::Text, text_style(24.0));
    assert_eq!(key_of(&mut doc, "Heading L"), "z_local_heading");

    // With no other copy left, the disabled library's style still resolves by its name.
    let mut only_disabled = Document::new("Consumer");
    let page2 = only_disabled.add_page("Page 1");
    enable(&mut only_disabled, &icons);
    import_asset(&mut only_disabled, &icons, "style:s_heading", None).apply(&mut only_disabled).unwrap();
    disable_library(&only_disabled, "a-icons").apply(&mut only_disabled).unwrap();
    let r = only_disabled.apply_spec_with_report(page2, &spec(serde_json::json!({ "kind": "text", "text_style": "Heading L" }))).unwrap();
    assert_eq!(only_disabled.get(r.root_node_id).unwrap().style_refs.get("text").map(String::as_str), Some("a-icons/s_heading"));
}

fn doc_with_two_accents(second_collection_name: &str) -> Document {
    let mut doc = Document::new("Ambiguous");
    doc.add_page("Page 1");
    add_variable(&mut doc, "c_a", "Alpha", "v_a", "Accent", VariableKind::Color, VariableValue::Color(Color::rgb(1, 2, 3)));
    add_variable(&mut doc, "c_b", second_collection_name, "v_b", "Accent", VariableKind::Color, VariableValue::Color(Color::rgb(4, 5, 6)));
    doc
}

#[test]
fn ambiguous_valid_options_resolve_back() {
    // Same variable name in two collections: the unqualified path is ambiguous.
    for (path, second_collection) in [("var:Accent", "Beta"), ("var:Dup/Accent", "Dup")] {
        let mut doc = doc_with_two_accents(second_collection);
        if second_collection == "Dup" {
            doc.variable_collections.get_mut("c_a").unwrap().name = "Dup".into();
        }
        let page = doc.pages[0];
        let err = doc.apply_spec_with_report(page, &spec(serde_json::json!({ "kind": "rectangle", "fill": path }))).unwrap_err();
        assert_eq!(err.code, "AMBIGUOUS_VARIABLE", "{path}");
        let mut options = err.valid_options.clone();
        options.sort();
        assert_eq!(options, ["var:c_a/Accent", "var:c_b/Accent"], "{path}: every option is a pasteable reference");
        for (option, expected_var) in [("var:c_a/Accent", "v_a"), ("var:c_b/Accent", "v_b")] {
            let r = doc.apply_spec_with_report(page, &spec(serde_json::json!({ "kind": "rectangle", "fill": option }))).unwrap();
            assert_eq!(doc.get(r.root_node_id).unwrap().variables.bindings.get("fills/0").map(String::as_str), Some(expected_var), "{option}");
        }
        assert!(err.corrected_example.is_some());
    }
}
