//! Component properties bound to variables through `update_node`, `apply_spec`, `get_node` and `export_spec`.
#![allow(clippy::result_large_err)]

use designos_doc::*;
use designos_mcp::engine::Engine;
use serde_json::{json, Value};

struct Setup {
    e: Engine,
    comp: NodeId,
    inst: NodeId,
}

fn variable(id: &str, kind: VariableKind, light: VariableValue, dark: VariableValue) -> Variable {
    Variable {
        id: id.into(),
        name: id.into(),
        collection: "theme".into(),
        kind,
        values: [("light".to_string(), light), ("dark".to_string(), dark)].into_iter().collect(),
        description: String::new(),
        scopes: vec![],
        hidden_from_publishing: false,
        key: None,
        code_syntax: Default::default(),
        extensions: Default::default(),
        mode_extensions: Default::default(),
    }
}

/// A component with a `label` text and an `icon` bound to the properties `Label` and `Show icon`, one instance of it in a
/// host frame, and a Theme collection with a boolean and a text variable.
fn setup() -> Setup {
    let mut e = Engine::default();
    let page = e.resolve_default_page().unwrap();
    let d = &mut e.doc;
    d.variable_collections.insert(
        "theme".into(),
        VariableCollection {
            id: "theme".into(),
            name: "Theme".into(),
            modes: vec![VariableMode { id: "light".into(), name: "Light".into() }, VariableMode { id: "dark".into(), name: "Dark".into() }],
            default_mode: "light".into(),
            variables: vec!["show".into(), "title".into()],
            key: None,
            source_library_key: None,
            extensions: Default::default(),
        },
    );
    d.variables.insert("show".into(), variable("show", VariableKind::Boolean, VariableValue::Boolean(true), VariableValue::Boolean(false)));
    let s = |v: &str| VariableValue::String(v.into());
    d.variables.insert("title".into(), variable("title", VariableKind::String, s("Pay"), s("Pay later")));
    let id = d.alloc_id();
    let comp = d.insert(page, Node::frame(id, Rect::new(0.0, 0.0, 100.0, 40.0))).unwrap();
    let id = d.alloc_id();
    let mut t = Node::text(id, Rect::new(10.0, 10.0, 60.0, 20.0), "Label", TextStyle::default());
    t.name = "label".into();
    let label = d.insert(comp, t).unwrap();
    let id = d.alloc_id();
    let mut ic = Node::ellipse(id, Rect::new(75.0, 10.0, 20.0, 20.0));
    ic.name = "icon".into();
    let icon = d.insert(comp, ic).unwrap();
    create_component(d, comp).apply(d).unwrap();
    for (name, kind, default) in [("Show icon", PropKind::Boolean, PropValue::Bool(true)), ("Label", PropKind::Text, PropValue::Text("Label".into()))] {
        add_prop(d, comp, ComponentPropDef { name: name.into(), kind, default }).apply(d).unwrap();
    }
    bind_prop(d, icon, OverrideField::Visible, Some("Show icon")).apply(d).unwrap();
    bind_prop(d, label, OverrideField::Characters, Some("Label")).apply(d).unwrap();
    let id = d.alloc_id();
    let host = d.insert(page, Node::frame(id, Rect::new(300.0, 0.0, 400.0, 200.0))).unwrap();
    create_instance(d, comp, host, Vec2::new(10.0, 10.0)).apply(d).unwrap();
    let inst = *d.get(host).unwrap().children.last().unwrap();
    Setup { e, comp, inst }
}

fn child_named(d: &Document, parent: NodeId, name: &str) -> NodeId {
    *d.get(parent).unwrap().children.iter().find(|c| d.get(**c).unwrap().name == name).unwrap_or_else(|| panic!("no child {name}"))
}

fn label_text(d: &Document, inst: NodeId) -> String {
    match &d.get(child_named(d, inst, "label")).unwrap().kind {
        NodeKind::Text(t) => t.content.clone(),
        _ => panic!("not text"),
    }
}

fn key(name: &str) -> String {
    prop_binding_key(name)
}

#[test]
fn update_node_binds_properties_in_one_undo_step_and_get_node_shows_them() {
    let mut s = setup();
    let reply =
        s.e.call("designos_update_node", json!({"node_id": s.inst.0, "prop_vars": {"Show icon": "var:Theme/show", "Label": "var:Theme/title"}})).unwrap();
    assert_eq!(reply["changed"], true, "{reply}");
    let got = s.e.call("designos_get_node", json!({"node_id": s.inst.0})).unwrap();
    assert_eq!(got["node"]["variables"]["bindings"][key("Show icon")], "show", "{got}");
    assert_eq!(got["node"]["variables"]["bindings"][key("Label")], "title");
    assert_eq!(label_text(&s.e.doc, s.inst), "Pay");

    // one undo takes both bindings back
    s.e.call("designos_undo", json!({})).unwrap();
    assert!(
        !s.e.doc.get(s.inst).unwrap().variables.bindings.keys().any(|k| prop_binding_name(k).is_some()),
        "{:?}",
        s.e.doc.get(s.inst).unwrap().variables.bindings
    );
    assert_eq!(label_text(&s.e.doc, s.inst), "Label");
}

#[test]
fn update_node_unbinds_with_null_and_refuses_a_wrong_variable() {
    let mut s = setup();
    s.e.call("designos_update_node", json!({"node_id": s.inst.0, "prop_vars": {"Label": "var:Theme/title"}})).unwrap();
    s.e.call("designos_update_node", json!({"node_id": s.inst.0, "prop_vars": {"Label": null}})).unwrap();
    assert!(s.e.doc.get(s.inst).unwrap().variables.bindings.is_empty());

    let wrong = s.e.call("designos_update_node", json!({"node_id": s.inst.0, "prop_vars": {"Show icon": "var:Theme/title"}})).unwrap_err();
    let wrong: Value = wrong.to_json_value();
    assert!(wrong["error"]["code"].as_str().is_some_and(|c| c.contains("VARIABLE") || c.contains("TYPE")), "{wrong}");
    let missing = s.e.call("designos_update_node", json!({"node_id": s.inst.0, "prop_vars": {"Nope": "var:Theme/show"}})).unwrap_err();
    assert_eq!(missing.to_json_value()["error"]["code"], "UNKNOWN_COMPONENT_PROPERTY");
    let not_instance = s.e.call("designos_update_node", json!({"node_id": s.comp.0, "prop_vars": {"Label": "var:Theme/title"}})).unwrap_err();
    assert_eq!(not_instance.to_json_value()["error"]["code"], "OVERRIDE_FIELD_NOT_APPLICABLE");
    let documented = designos_mcp::rules::tool_error_codes("designos_update_node");
    for returned in [&missing, &not_instance] {
        let code = returned.to_json_value()["error"]["code"].as_str().unwrap().to_string();
        assert!(documented.contains(&format!("`{code}`").as_str()), "designos_update_node error list must include {code}: {documented:?}");
    }
    assert!(s.e.doc.get(s.inst).unwrap().variables.bindings.is_empty(), "a refused call changes nothing");
}

#[test]
fn apply_spec_binds_and_export_spec_round_trips_the_binding() {
    let mut s = setup();
    let page = s.e.resolve_default_page().unwrap();
    let applied = s
        .e
        .call(
            "designos_apply_spec",
            json!({"parent_id": page.0, "spec": {"kind": "instance", "component": format!("node:{}", s.comp.0), "props": {"Show icon": "var:Theme/show", "Label": "var:Theme/title"}}}),
        )
        .unwrap();
    let root = applied["root_node_id"].as_u64().unwrap();
    assert_eq!(s.e.doc.get(NodeId(root)).unwrap().variables.bindings.get(&key("Label")).map(String::as_str), Some("title"), "{applied}");
    assert_eq!(label_text(&s.e.doc, NodeId(root)), "Pay");

    let exported = s.e.call("designos_export_spec", json!({"node_id": root})).unwrap();
    assert_eq!(exported["spec"]["props"]["Label"], "var:Theme/title", "{exported}");
    assert_eq!(exported["spec"]["props"]["Show icon"], "var:Theme/show");
    let again = s.e.call("designos_apply_spec", json!({"parent_id": page.0, "spec": exported["spec"]})).unwrap();
    let copy = NodeId(again["root_node_id"].as_u64().unwrap());
    assert_eq!(s.e.doc.get(copy).unwrap().variables.bindings, s.e.doc.get(NodeId(root)).unwrap().variables.bindings);
}

#[test]
fn a_variant_value_that_is_no_option_is_reported_and_keeps_the_variant() {
    let mut s = setup();
    // a Size variant set next to the label component
    let page = s.e.resolve_default_page().unwrap();
    let d = &mut s.e.doc;
    let mut mains = Vec::new();
    for name in ["Size=Small", "Size=Large"] {
        let id = d.alloc_id();
        let mut v = Node::frame(id, Rect::new(0.0, 100.0, 100.0, 40.0));
        v.name = name.into();
        let v = d.insert(page, v).unwrap();
        create_component(d, v).apply(d).unwrap();
        mains.push(v);
    }
    combine_as_variants(d, &mains).apply(d).unwrap();
    d.variables.insert("size".into(), variable("size", VariableKind::String, VariableValue::String("Huge".into()), VariableValue::String("Large".into())));
    d.variable_collections.get_mut("theme").unwrap().variables.push("size".into());
    let id = d.alloc_id();
    let host = d.insert(page, Node::frame(id, Rect::new(300.0, 300.0, 400.0, 200.0))).unwrap();
    create_instance(d, mains[0], host, Vec2::new(0.0, 0.0)).apply(d).unwrap();
    let inst = *d.get(host).unwrap().children.last().unwrap();

    let reply = s.e.call("designos_update_node", json!({"node_id": inst.0, "prop_vars": {"Size": "var:Theme/size"}})).unwrap();
    assert_eq!(reply["property_binding_issues"][0]["property"], "Size", "{reply}");
    assert!(reply["property_binding_issues"][0]["problem"].as_str().unwrap().contains("Huge"));
    assert_eq!(s.e.doc.get(inst).and_then(instance_component), Some(mains[0]), "the instance keeps its variant");
}

#[test]
fn update_node_with_name_and_invalid_prop_vars_leaves_document_and_undo_untouched() {
    let mut s = setup();
    let orig_name = s.e.doc.get(s.inst).unwrap().name.clone();
    let orig_depth = s.e.history.undo_depth();
    let err = s.e.call("designos_update_node", json!({"node_id": s.inst.0, "name": "Renamed", "prop_vars": {"Show icon": "var:Theme/title"}})).unwrap_err();
    let err_json: Value = err.to_json_value();
    assert!(err_json["error"]["code"].as_str().is_some_and(|c| c.contains("VARIABLE") || c.contains("TYPE")), "{err_json}");
    assert_eq!(s.e.doc.get(s.inst).unwrap().name, orig_name, "node name must remain unchanged on prop_vars error");
    assert!(s.e.doc.get(s.inst).unwrap().variables.bindings.is_empty(), "bindings must remain empty");
    assert_eq!(s.e.history.undo_depth(), orig_depth, "undo stack must remain untouched");
}

#[test]
fn update_node_with_name_and_valid_prop_vars_undoes_both_in_one_step() {
    let mut s = setup();
    let orig_name = s.e.doc.get(s.inst).unwrap().name.clone();
    let orig_depth = s.e.history.undo_depth();
    let reply =
        s.e.call("designos_update_node", json!({"node_id": s.inst.0, "name": "CTA", "prop_vars": {"Show icon": "var:Theme/show", "Label": "var:Theme/title"}}))
            .unwrap();
    assert_eq!(reply["changed"], true, "{reply}");
    assert_eq!(s.e.doc.get(s.inst).unwrap().name, "CTA");
    assert_eq!(s.e.doc.get(s.inst).unwrap().variables.bindings.get(&key("Show icon")).map(String::as_str), Some("show"));
    assert_eq!(s.e.doc.get(s.inst).unwrap().variables.bindings.get(&key("Label")).map(String::as_str), Some("title"));
    assert_eq!(label_text(&s.e.doc, s.inst), "Pay");
    assert_eq!(s.e.history.undo_depth(), orig_depth + 1, "mixed update must be one undo step");

    s.e.call("designos_undo", json!({})).unwrap();
    assert_eq!(s.e.doc.get(s.inst).unwrap().name, orig_name, "one undo must restore the node name");
    assert!(!s.e.doc.get(s.inst).unwrap().variables.bindings.keys().any(|k| prop_binding_name(k).is_some()), "one undo must remove the property bindings");
    assert_eq!(label_text(&s.e.doc, s.inst), "Label", "one undo must restore the bound child text");
    assert_eq!(s.e.history.undo_depth(), orig_depth);
}

#[test]
fn apply_spec_warns_when_bound_variant_variable_is_not_an_option() {
    let mut s = setup();
    let page = s.e.resolve_default_page().unwrap();
    let d = &mut s.e.doc;
    let mut mains = Vec::new();
    for name in ["Size=Small", "Size=Large"] {
        let id = d.alloc_id();
        let mut v = Node::frame(id, Rect::new(0.0, 100.0, 100.0, 40.0));
        v.name = name.into();
        let v = d.insert(page, v).unwrap();
        create_component(d, v).apply(d).unwrap();
        mains.push(v);
    }
    combine_as_variants(d, &mains).apply(d).unwrap();
    d.variables.insert("size".into(), variable("size", VariableKind::String, VariableValue::String("Huge".into()), VariableValue::String("Large".into())));
    d.variable_collections.get_mut("theme").unwrap().variables.push("size".into());

    let applied =
        s.e.call(
            "designos_apply_spec",
            json!({"parent_id": page.0, "spec": {"kind": "instance", "component": format!("node:{}", mains[0].0), "props": {"Size": "var:Theme/size"}}}),
        )
        .unwrap();
    let inst = NodeId(applied["root_node_id"].as_u64().unwrap());
    assert_eq!(s.e.doc.get(inst).unwrap().variables.bindings.get(&key("Size")).map(String::as_str), Some("size"));
    assert_eq!(s.e.doc.get(inst).and_then(instance_component), Some(mains[0]), "the instance stays on Small");
    let warnings = applied["warnings"].as_array().expect("warnings array");
    assert!(
        warnings.iter().any(|w| { w["path"] == "props.Size" && w["message"].as_str().is_some_and(|m| m.contains("Size") && m.contains("Huge")) }),
        "expected a spec warning for the invalid variant value, got: {applied}"
    );
}

#[test]
fn update_node_refuses_mismatched_library_prop_var_without_importing_then_binds_and_undoes_with_import() {
    let dir = std::env::temp_dir().join(format!("designos_mcp_prop_var_lib_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut lib_doc = Document::new("Theme Library");
    lib_doc.variable_collections.insert(
        "theme".into(),
        VariableCollection {
            id: "theme".into(),
            name: "Theme".into(),
            modes: vec![VariableMode { id: "light".into(), name: "Light".into() }, VariableMode { id: "dark".into(), name: "Dark".into() }],
            default_mode: "light".into(),
            variables: vec!["title".into()],
            key: None,
            source_library_key: None,
            extensions: Default::default(),
        },
    );
    lib_doc
        .variables
        .insert("title".into(), variable("title", VariableKind::String, VariableValue::String("Pay".into()), VariableValue::String("Pay later".into())));
    designos_library::Library::open(&dir).unwrap().publish_library("lib-theme", &lib_doc, "v1").unwrap();

    let mut s = setup();
    s.e.doc.variables.clear();
    s.e.doc.variable_collections.clear();
    s.e.doc.library.enabled.push(LibraryRef { id: "lib-theme".into(), name: "Theme".into(), version: 1 });
    let loaded = designos_mcp::tools::library_source::load_enabled_snapshots_from(&s.e.doc, &dir);
    assert_eq!(loaded.len(), 1);

    let orig_library = s.e.doc.library.clone();
    let orig_depth = s.e.history.undo_depth();

    let wrong = s.e.call("designos_update_node", json!({"node_id": s.inst.0, "prop_vars": {"Show icon": "var:Theme/title"}})).unwrap_err();
    let wrong_json: Value = wrong.to_json_value();
    assert_eq!(wrong_json["error"]["code"], "VARIABLE_TYPE_MISMATCH", "{wrong_json}");
    assert!(s.e.doc.variables.is_empty(), "refused library variable bind must leave doc.variables empty");
    assert!(s.e.doc.variable_collections.is_empty(), "refused library variable bind must leave doc.variable_collections empty");
    assert_eq!(s.e.doc.library, orig_library, "refused library variable bind must leave doc.library unchanged");
    assert_eq!(s.e.history.undo_depth(), orig_depth, "refused library variable bind must leave undo stack untouched");

    let partial =
        s.e.call("designos_update_node", json!({"node_id": s.inst.0, "prop_vars": {"Label": "var:Theme/title", "Nope": "var:Theme/title"}})).unwrap_err();
    assert_eq!(partial.to_json_value()["error"]["code"], "UNKNOWN_COMPONENT_PROPERTY");
    assert!(s.e.doc.variables.is_empty(), "partial prop_vars failure must roll back imported variables");
    assert!(s.e.doc.variable_collections.is_empty(), "partial prop_vars failure must roll back imported collections");
    assert_eq!(s.e.doc.library, orig_library, "partial prop_vars failure must roll back doc.library");
    assert_eq!(s.e.history.undo_depth(), orig_depth);

    let reply = s.e.call("designos_update_node", json!({"node_id": s.inst.0, "prop_vars": {"Label": "var:Theme/title"}})).unwrap();
    assert_eq!(reply["changed"], true, "{reply}");
    assert_eq!(s.e.doc.get(s.inst).unwrap().variables.bindings.get(&key("Label")).map(String::as_str), Some("lib-theme/title"));
    assert_eq!(label_text(&s.e.doc, s.inst), "Pay");
    assert!(s.e.doc.variables.contains_key("lib-theme/title"));
    assert!(!s.e.doc.library.external.is_empty());
    assert_eq!(s.e.history.undo_depth(), orig_depth + 1);

    s.e.call("designos_undo", json!({})).unwrap();
    assert!(s.e.doc.get(s.inst).unwrap().variables.bindings.is_empty(), "one undo removes the property binding");
    assert_eq!(label_text(&s.e.doc, s.inst), "Label", "one undo restores the bound text child");
    assert!(s.e.doc.variables.is_empty(), "one undo removes the imported library variable");
    assert!(s.e.doc.variable_collections.is_empty(), "one undo removes the imported library collection");
    assert_eq!(s.e.doc.library, orig_library, "one undo restores doc.library (removing imported external registrations)");
    assert_eq!(s.e.history.undo_depth(), orig_depth);

    let _ = std::fs::remove_dir_all(&dir);
}
