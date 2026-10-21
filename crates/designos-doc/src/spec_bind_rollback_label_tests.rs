//! `apply_spec` rollback of document data on error, and explicit text overrides on the label
//! of a frame that carries `text` (preset buttons, badges).

use crate::spec_bind::fixtures;
use crate::*;
use std::collections::BTreeMap;

/// Everything besides nodes that a spec apply may write.
type Data =
    (BTreeMap<String, Variable>, BTreeMap<String, VariableCollection>, BTreeMap<String, Style>, BTreeMap<String, StyleMeta>, LibraryState, Option<NodeId>);

fn data_of(doc: &Document) -> Data {
    (doc.variables.clone(), doc.variable_collections.clone(), doc.styles.clone(), doc.style_meta.clone(), doc.library.clone(), doc.library_page)
}

fn spec(v: serde_json::Value) -> NodeSpec {
    serde_json::from_value(v).unwrap()
}

/// A library document that publishes the colour variable `Brand/Accent`.
fn library_snapshot() -> LibrarySnapshot {
    let mut lib = Document::new("Brand Library");
    lib.add_page("Page 1");
    lib.variable_collections.insert(
        "coll_brand".into(),
        VariableCollection {
            extensions: Default::default(),
            id: "coll_brand".into(),
            name: "Brand".into(),
            modes: vec![VariableMode { id: "m1".into(), name: "Default".into() }],
            default_mode: "m1".into(),
            variables: vec!["var_accent".into()],
            key: None,
            source_library_key: None,
        },
    );
    lib.variables.insert(
        "var_accent".into(),
        Variable {
            extensions: Default::default(),
            mode_extensions: Default::default(),
            id: "var_accent".into(),
            name: "Accent".into(),
            collection: "coll_brand".into(),
            kind: VariableKind::Color,
            values: BTreeMap::from([("m1".to_string(), VariableValue::Color(Color::rgb(0x12, 0x34, 0x56)))]),
            description: String::new(),
            scopes: Vec::new(),
            hidden_from_publishing: false,
            key: None,
            code_syntax: BTreeMap::new(),
        },
    );
    snapshot(&lib, "brand", "Brand", 1)
}

fn consumer_with_library(snap: &LibrarySnapshot) -> Document {
    let mut doc = fixtures::tokens_doc();
    enable_library(&doc, snap).apply(&mut doc).unwrap();
    doc
}

#[test]
fn library_import_succeeds_when_every_child_resolves() {
    // Guards the fixture of the rollback test: the first child alone really imports.
    let snap = library_snapshot();
    let mut doc = consumer_with_library(&snap);
    let page = doc.pages[0];
    let before = data_of(&doc);
    let ok = spec(serde_json::json!({ "kind": "rectangle", "fill": "var:Brand/Accent" }));
    doc.apply_spec_with_libraries(page, &ok, std::slice::from_ref(&snap)).unwrap();
    assert_ne!(data_of(&doc), before);
    assert!(doc.variables.keys().any(|k| k.ends_with("var_accent")), "{:?}", doc.variables.keys().collect::<Vec<_>>());
    assert!(!doc.library.external.is_empty());
}

#[test]
fn library_import_is_rolled_back_when_a_later_child_fails() {
    let snap = library_snapshot();
    let mut doc = consumer_with_library(&snap);
    let page = doc.pages[0];
    let before = data_of(&doc);
    let nodes_before = doc.nodes.len();
    let next_before = doc.next_id;
    let bad = spec(serde_json::json!({
        "kind": "frame",
        "children": [
            { "kind": "rectangle", "fill": "var:Brand/Accent" },
            { "kind": "rectangle", "fill": "var:Color/Nope" }
        ]
    }));

    let err = doc.apply_spec_with_libraries(page, &bad, std::slice::from_ref(&snap)).unwrap_err();

    assert_eq!(err.code, "UNKNOWN_VARIABLE");
    assert!(err.safe_to_retry);
    assert_eq!(doc.nodes.len(), nodes_before, "inserted nodes are removed");
    assert_eq!(doc.next_id, next_before);
    assert_eq!(data_of(&doc), before, "variables, collections, styles, style meta and library state return to their pre-call state");
    assert!(doc.library.external.is_empty());
}

#[test]
fn easeui_collection_is_rolled_back_when_a_later_child_fails() {
    let mut doc = fixtures::tokens_doc();
    let page = doc.pages[0];
    let before = data_of(&doc);
    let bad = spec(serde_json::json!({
        "kind": "frame",
        "children": [
            { "kind": "rectangle", "fill": "$easeui.bg.panel" },
            { "kind": "rectangle", "fill": "var:Color/Nope" }
        ]
    }));
    assert!(doc.apply_spec_with_report(page, &bad).is_err());
    assert_eq!(data_of(&doc), before);
}

fn label_of(doc: &Document, root: NodeId) -> &Node {
    let id = doc.get(root).unwrap().children.iter().copied().find(|c| matches!(doc.get(*c).unwrap().kind, NodeKind::Text(_))).expect("label");
    doc.get(id).unwrap()
}

fn text_style_of(n: &Node) -> &TextStyle {
    let NodeKind::Text(tp) = &n.kind else { panic!("not a text node") };
    &tp.style
}

#[test]
fn explicit_font_size_overrides_and_detaches_the_style_on_a_frame_label() {
    let mut doc = fixtures::tokens_doc();
    let page = doc.pages[0];
    let s = spec(serde_json::json!({ "preset": "easeui-button-primary", "text": "Go", "text_style": "Heading L", "font_size": 18 }));
    let report = doc.apply_spec_with_report(page, &s).unwrap();
    let label = label_of(&doc, report.root_node_id);
    assert_eq!(text_style_of(label).font_size, 18.0);
    assert!(!label.style_refs.contains_key("text"), "{:?}", label.style_refs);
    let detached: Vec<_> = report.warnings.iter().filter(|w| w.code == "STYLE_DETACHED_BY_SPEC").collect();
    assert_eq!(detached.len(), 1, "{:?}", report.warnings);
    assert_eq!(detached[0].path, "spec.font_size");
}

#[test]
fn explicit_line_height_overrides_the_style_on_a_frame_label() {
    let mut doc = fixtures::tokens_doc();
    let page = doc.pages[0];
    let s = spec(serde_json::json!({ "preset": "easeui-badge", "text": "New", "text_style": "Heading L", "line_height": 40 }));
    let report = doc.apply_spec_with_report(page, &s).unwrap();
    let label = label_of(&doc, report.root_node_id);
    assert_eq!(text_style_of(label).line_height, LineHeight::Pixels(40.0));
    assert_eq!(text_style_of(label).font_size, 24.0, "the style still supplies the size");
    assert_eq!(label.style_refs.get("text").map(String::as_str), Some("style_heading_l"));
}

#[test]
fn font_size_variable_detaches_the_style_on_a_frame_label() {
    let mut doc = fixtures::tokens_doc();
    let page = doc.pages[0];
    let s = spec(serde_json::json!({ "preset": "easeui-button-primary", "text": "Go", "text_style": "Heading L", "font_size": "var:Spacing/4" }));
    let report = doc.apply_spec_with_report(page, &s).unwrap();
    let label = label_of(&doc, report.root_node_id);
    assert_eq!(label.variables.bindings.get("fontSize").map(String::as_str), Some("var_spacing_4"));
    assert_eq!(text_style_of(label).font_size, 16.0);
    assert!(!label.style_refs.contains_key("text"));
    assert_eq!(report.warnings.iter().filter(|w| w.code == "STYLE_DETACHED_BY_SPEC").count(), 1);
}

#[test]
fn text_node_keeps_the_same_override_behaviour() {
    let mut doc = fixtures::tokens_doc();
    let page = doc.pages[0];
    let s = spec(serde_json::json!({ "kind": "text", "text": "Go", "text_style": "Heading L", "font_size": 18 }));
    let report = doc.apply_spec_with_report(page, &s).unwrap();
    let n = doc.get(report.root_node_id).unwrap();
    assert_eq!(text_style_of(n).font_size, 18.0);
    assert!(!n.style_refs.contains_key("text"));
}
