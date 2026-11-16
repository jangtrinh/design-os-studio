//! A component property bound to a variable: `componentPropAssignments[].varValue` holds the variable alias.
//! The import keeps it as the instance's property binding, and a `.fig` save writes it back.
//! Hand-written kiwi-shaped JSON; nothing here comes from a real file.

mod common;

use common::fig_fixture::*;
use designos_doc::*;
use designos_fig::{export_fig, import_fig, import_fig_node_changes};
use serde_json::{json, Value};

const SET: (u64, u64) = (1, 5);
const LIGHT: (u64, u64) = (1, 6);
const DARK: (u64, u64) = (1, 10);
const SHOW: (u64, u64) = (1, 7);
const TITLE: (u64, u64) = (1, 8);

fn variable(id: (u64, u64), name: &str, ty: &str, light: Value, dark: Value) -> Value {
    let entry = |mode: (u64, u64), v: &Value| json!({"modeID": guid(mode.0, mode.1), "variableData": {"dataType": ty, "value": v}});
    fig_node(
        id,
        (0, 1),
        "VARIABLE",
        name,
        json!({"key": format!("key-{name}"), "variableSetID": {"guid": guid(SET.0, SET.1)}, "variableResolvedType": ty,
               "variableDataValues": {"entries": [entry(LIGHT, &light), entry(DARK, &dark)]}}),
    )
}

fn alias_assignment(def: (u64, u64), resolved: &str, var: (u64, u64), literal: Value) -> Value {
    json!({"defID": guid(def.0, def.1), "value": literal,
           "varValue": {"dataType": "ALIAS", "resolvedDataType": resolved, "value": {"alias": {"guid": guid(var.0, var.1)}}}})
}

/// A `Button` with an icon bound to `Show icon` and a label bound to `Label`, and one instance whose two properties
/// are bound to variables. `literal` is the plain value each assignment carries next to the alias.
fn fixture(literal: bool) -> Vec<Value> {
    let mut nodes = document_and_page();
    let set = fig_node(
        SET,
        (0, 1),
        "VARIABLE_SET",
        "Theme",
        json!({"key": "theme", "variableSetModes": [
            {"id": guid(LIGHT.0, LIGHT.1), "name": "Light", "sortPosition": "!"}, {"id": guid(DARK.0, DARK.1), "name": "Dark", "sortPosition": "\""}]}),
    );
    let defs = json!([
        {"id": guid(7, 1), "name": "Show icon", "type": "BOOL", "initialValue": {"boolValue": true}},
        {"id": guid(7, 2), "name": "Label", "type": "TEXT", "initialValue": {"textValue": {"characters": "Label"}}}
    ]);
    let (show_lit, title_lit) = if literal { (json!({"boolValue": false}), json!({"textValue": {"characters": "Pay"}})) } else { (json!({}), json!({})) };
    nodes.extend([
        set,
        variable(SHOW, "ShowIcon", "BOOLEAN", json!({"boolValue": true}), json!({"boolValue": false})),
        variable(TITLE, "Title", "STRING", json!({"textValue": "Pay"}), json!({"textValue": "Pay later"})),
        fig_node((5, 3), (0, 1), "SYMBOL", "Button", json!({"size": {"x": 100.0, "y": 40.0}, "componentPropDefs": defs})),
        fig_node(
            (5, 4),
            (5, 3),
            "TEXT",
            "label",
            json!({"textData": {"characters": "Label"}, "componentPropRefs": [{"componentPropNodeField": "TEXT_DATA", "defID": guid(7, 2)}]}),
        ),
        fig_node(
            (5, 5),
            (5, 3),
            "ELLIPSE",
            "icon",
            json!({"size": {"x": 20.0, "y": 20.0}, "componentPropRefs": [{"componentPropNodeField": "VISIBLE", "defID": guid(7, 1)}]}),
        ),
        fig_node(
            (1, 1),
            (0, 1),
            "INSTANCE",
            "Buy",
            json!({"size": {"x": 100.0, "y": 40.0}, "symbolData": {"symbolID": guid(5, 3), "uniformScaleFactor": 1.0, "symbolOverrides": []},
                   "componentPropAssignments": [
                       alias_assignment((7, 1), "BOOLEAN", SHOW, show_lit),
                       alias_assignment((7, 2), "STRING", TITLE, title_lit)]}),
        ),
    ]);
    nodes
}

fn import(literal: bool) -> Document {
    import_fig_node_changes(&Value::Array(fixture(literal)), "prop variable binding").expect("import")
}

fn buy(doc: &Document) -> &Node {
    by_name(doc, "Buy")
}

fn text_of(n: &Node) -> String {
    match &n.kind {
        NodeKind::Text(t) => t.content.clone(),
        _ => panic!("not text"),
    }
}

fn bound_name(doc: &Document, inst: &Node, prop: &str) -> Option<String> {
    let id = inst.variables.bindings.get(&prop_binding_key(prop))?;
    doc.variables.get(id).map(|v| v.name.clone())
}

#[test]
fn import_keeps_the_variable_an_assignment_is_bound_to() {
    let doc = import(true);
    assert_eq!(bound_name(&doc, buy(&doc), "Show icon").as_deref(), Some("ShowIcon"));
    assert_eq!(bound_name(&doc, buy(&doc), "Label").as_deref(), Some("Title"));
}

#[test]
fn an_assignment_with_only_the_alias_takes_the_value_of_its_variable() {
    let doc = import(false);
    let inst = buy(&doc);
    assert_eq!(bound_name(&doc, inst, "Show icon").as_deref(), Some("ShowIcon"));
    assert!(kid(&doc, inst, "icon").visible);
    assert_eq!(inst.component.values.get("Label"), Some(&PropValue::Text("Pay".into())), "the variable's light value, not the property default");
    assert_eq!(text_of(kid(&doc, inst, "label")), "Pay", "and the layer shows it");
}

#[test]
fn a_fig_save_and_reopen_keeps_the_binding() {
    let doc = import(true);
    let saved = export_fig(&doc).expect("export");
    let reopened = import_fig(&saved).expect("reopen");
    assert_eq!(bound_name(&reopened, buy(&reopened), "Show icon").as_deref(), Some("ShowIcon"));
    assert_eq!(bound_name(&reopened, buy(&reopened), "Label").as_deref(), Some("Title"));
    // and it is still live: the dark mode hides the icon
    let mut d = reopened;
    let id = buy(&d).id;
    let mut host = d.get(id).unwrap().clone();
    host.variables.modes.insert(d.variable_collections.keys().next().unwrap().clone(), d.variable_collections.values().next().unwrap().modes[1].id.clone());
    let mut h = History::default();
    h.exec(&mut d, Command::Update { node: Box::new(host) }).unwrap();
    assert!(!kid(&d, buy(&d), "icon").visible, "the reopened binding follows the mode");
}

#[test]
fn the_saved_assignment_carries_the_alias_of_the_variable() {
    let doc = import(true);
    let nodes = node_changes(&export_fig(&doc).expect("export"));
    let inst = nodes.iter().find(|n| n["name"] == "Buy").expect("instance saved");
    let assigns = inst["componentPropAssignments"].as_array().expect("assignments");
    let aliased: Vec<&Value> = assigns.iter().filter(|a| a["varValue"]["dataType"] == "ALIAS").collect();
    assert_eq!(aliased.len(), 2, "{assigns:?}");
    let show_var = nodes.iter().find(|n| n["name"] == "ShowIcon").expect("variable saved");
    assert!(aliased.iter().any(|a| a["varValue"]["value"]["alias"]["guid"] == show_var["guid"]), "the alias points at the saved variable");
    let resolved: Vec<&str> = aliased.iter().filter_map(|a| a["varValue"]["resolvedDataType"].as_str()).collect();
    assert!(resolved.contains(&"BOOLEAN") && resolved.contains(&"STRING"), "{resolved:?}");
}

#[test]
fn an_instance_without_bindings_writes_no_alias() {
    let mut doc = import(true);
    let id = buy(&doc).id;
    let mut n = doc.get(id).unwrap().clone();
    n.variables.bindings.clear();
    Command::Update { node: Box::new(n) }.apply(&mut doc).unwrap();
    let nodes = node_changes(&export_fig(&doc).expect("export"));
    let inst = nodes.iter().find(|n| n["name"] == "Buy").expect("instance saved");
    let assigns = inst["componentPropAssignments"].as_array().cloned().unwrap_or_default();
    assert!(assigns.iter().all(|a| a["varValue"]["dataType"] != "ALIAS"), "{assigns:?}");
}

#[test]
fn a_variant_assignment_bound_to_a_string_variable_swaps_the_variant_and_round_trips() {
    let mut nodes = document_and_page();
    let set = fig_node(
        SET,
        (0, 1),
        "VARIABLE_SET",
        "Theme",
        json!({"key": "theme", "variableSetModes": [
            {"id": guid(LIGHT.0, LIGHT.1), "name": "Light", "sortPosition": "!"}, {"id": guid(DARK.0, DARK.1), "name": "Dark", "sortPosition": "\""}]}),
    );
    let size_var = (1, 9);
    let comp_set_defs = json!([
        {"id": guid(7, 3), "name": "Size", "type": "VARIANT", "initialValue": {"textValue": {"characters": "Small"}}}
    ]);
    let orders = json!([{"property": "Size", "values": ["Small", "Large"]}]);
    nodes.extend([
        set,
        variable(size_var, "BtnSize", "STRING", json!({"textValue": "Large"}), json!({"textValue": "Small"})),
        fig_node(
            (5, 10),
            (0, 1),
            "FRAME",
            "Btn",
            json!({"isStateGroup": true, "size": {"x": 240.0, "y": 60.0}, "componentPropDefs": comp_set_defs, "stateGroupPropertyValueOrders": orders}),
        ),
        fig_node((5, 11), (5, 10), "SYMBOL", "Size=Small", json!({"size": {"x": 80.0, "y": 32.0}})),
        fig_node((5, 12), (5, 10), "SYMBOL", "Size=Large", json!({"size": {"x": 120.0, "y": 48.0}})),
        fig_node(
            (1, 2),
            (0, 1),
            "INSTANCE",
            "Cta",
            json!({"size": {"x": 80.0, "y": 32.0}, "symbolData": {"symbolID": guid(5, 11), "uniformScaleFactor": 1.0, "symbolOverrides": []},
                   "componentPropAssignments": [
                       alias_assignment((7, 3), "STRING", size_var, json!({"textValue": {"characters": "Large"}}))]}),
        ),
    ]);
    let doc = import_fig_node_changes(&Value::Array(nodes), "variant prop variable binding").expect("import");
    let cta = by_name(&doc, "Cta");
    let large = by_name(&doc, "Size=Large").id;
    let small = by_name(&doc, "Size=Small").id;
    assert_eq!(bound_name(&doc, cta, "Size").as_deref(), Some("BtnSize"));
    assert_eq!(instance_component(cta), Some(large), "light mode variable selects Size=Large even when symbolID was Size=Small");

    let saved = export_fig(&doc).expect("export");
    let mut reopened = import_fig(&saved).expect("reopen");
    let cta_re = by_name(&reopened, "Cta");
    assert_eq!(bound_name(&reopened, cta_re, "Size").as_deref(), Some("BtnSize"));
    assert_eq!(instance_component(cta_re), Some(by_name(&reopened, "Size=Large").id));

    let cta_id = cta_re.id;
    let small_re = by_name(&reopened, "Size=Small").id;
    let mut host = reopened.get(cta_id).unwrap().clone();
    host.variables
        .modes
        .insert(reopened.variable_collections.keys().next().unwrap().clone(), reopened.variable_collections.values().next().unwrap().modes[1].id.clone());
    let mut h = History::default();
    h.exec(&mut reopened, Command::Update { node: Box::new(host) }).unwrap();
    assert_eq!(instance_component(reopened.get(cta_id).unwrap()), Some(small_re), "dark mode swaps reopened instance to Size=Small");
    let _ = small;
}

fn bound_default_name(doc: &Document, comp: &Node, prop: &str) -> Option<String> {
    let id = comp.component.prop_default_variables.get(prop)?;
    doc.variables.get(id).map(|v| v.name.clone())
}

fn fig_with_bound_prop_defaults() -> Vec<Value> {
    let mut nodes = document_and_page();
    let set = fig_node(
        SET,
        (0, 1),
        "VARIABLE_SET",
        "Theme",
        json!({"key": "theme", "variableSetModes": [
            {"id": guid(LIGHT.0, LIGHT.1), "name": "Light", "sortPosition": "!"},
            {"id": guid(DARK.0, DARK.1), "name": "Dark", "sortPosition": "\""}]}),
    );
    let show_var = (1, 7);
    let label_var = (1, 8);
    let size_var = (1, 9);
    let defs = json!([
        {
            "id": guid(7, 1),
            "name": "Show icon",
            "type": "BOOL",
            "initialValue": {"boolValue": false},
            "varValue": {
                "dataType": "ALIAS",
                "resolvedDataType": "BOOLEAN",
                "value": {"alias": {"guid": guid(show_var.0, show_var.1)}}
            }
        },
        {
            "id": guid(7, 2),
            "name": "Label",
            "type": "TEXT",
            "initialValue": {"textValue": {"characters": "Submit"}},
            "varValue": {
                "dataType": "ALIAS",
                "resolvedDataType": "STRING",
                "value": {"alias": {"guid": guid(label_var.0, label_var.1)}}
            }
        }
    ]);
    let set_defs = json!([
        {
            "id": guid(7, 3),
            "name": "Size",
            "type": "VARIANT",
            "initialValue": {"textValue": {"characters": "Small"}},
            "varValue": {
                "dataType": "ALIAS",
                "resolvedDataType": "STRING",
                "value": {"alias": {"guid": guid(size_var.0, size_var.1)}}
            }
        }
    ]);
    let orders = json!([{"property": "Size", "values": ["Small", "Large"]}]);
    nodes.extend([
        set,
        variable(show_var, "ShowIcon", "BOOLEAN", json!({"boolValue": true}), json!({"boolValue": false})),
        variable(label_var, "Title", "STRING", json!({"textValue": "Pay"}), json!({"textValue": "Buy now"})),
        variable(size_var, "BtnSize", "STRING", json!({"textValue": "Large"}), json!({"textValue": "Small"})),
        fig_node((1, 1), (0, 1), "SYMBOL", "Button", json!({"size": {"x": 120.0, "y": 40.0}, "componentPropDefs": defs})),
        fig_node(
            (5, 10),
            (0, 1),
            "FRAME",
            "Chip",
            json!({"isStateGroup": true, "size": {"x": 240.0, "y": 60.0}, "componentPropDefs": set_defs, "stateGroupPropertyValueOrders": orders}),
        ),
        fig_node((5, 11), (5, 10), "SYMBOL", "Size=Small", json!({"size": {"x": 80.0, "y": 32.0}})),
        fig_node((5, 12), (5, 10), "SYMBOL", "Size=Large", json!({"size": {"x": 120.0, "y": 48.0}})),
    ]);
    nodes
}

#[test]
fn both_fig_and_rest_readers_keep_a_bound_property_default() {
    let fig_doc = import_fig_node_changes(&Value::Array(fig_with_bound_prop_defaults()), "bound prop defaults").expect("import fig");
    let fig_comp = by_name(&fig_doc, "Button");
    assert_eq!(fig_comp.component.prop_default_variables.get("Show icon").map(String::as_str), Some("VariableID:1:7"));
    assert_eq!(fig_comp.component.prop_default_variables.get("Label").map(String::as_str), Some("VariableID:1:8"));
    assert_eq!(bound_default_name(&fig_doc, fig_comp, "Show icon").as_deref(), Some("ShowIcon"));
    assert_eq!(bound_default_name(&fig_doc, fig_comp, "Label").as_deref(), Some("Title"));
    let fig_set = by_name(&fig_doc, "Chip");
    assert_eq!(fig_set.component.prop_default_variables.get("Size").map(String::as_str), Some("VariableID:1:9"));
    assert_eq!(bound_default_name(&fig_doc, fig_set, "Size").as_deref(), Some("BtnSize"));

    let rest = json!({
        "name": "rest bound prop defaults",
        "document": {
            "id": "0:0",
            "type": "DOCUMENT",
            "children": [{
                "id": "0:1",
                "type": "CANVAS",
                "name": "Page 1",
                "children": [{
                    "id": "1:1",
                    "type": "COMPONENT",
                    "name": "Button",
                    "absoluteBoundingBox": {"x": 0.0, "y": 0.0, "width": 120.0, "height": 40.0},
                    "componentPropertyDefinitions": {
                        "Show icon#7:1": {
                            "type": "BOOLEAN",
                            "defaultValue": false,
                            "boundVariables": {
                                "defaultValue": {"type": "VARIABLE_ALIAS", "id": "VariableID:1:7"}
                            }
                        },
                        "Label#7:2": {
                            "type": "TEXT",
                            "defaultValue": "Submit",
                            "boundVariables": {
                                "defaultValue": {"type": "VARIABLE_ALIAS", "id": "VariableID:1:8"}
                            }
                        }
                    },
                    "children": []
                }, {
                    "id": "5:10",
                    "type": "COMPONENT_SET",
                    "name": "Chip",
                    "absoluteBoundingBox": {"x": 0.0, "y": 60.0, "width": 240.0, "height": 60.0},
                    "componentPropertyDefinitions": {
                        "Size": {
                            "type": "VARIANT",
                            "defaultValue": "Small",
                            "variantOptions": ["Small", "Large"],
                            "boundVariables": {
                                "defaultValue": {"type": "VARIABLE_ALIAS", "id": "VariableID:1:9"}
                            }
                        }
                    },
                    "children": []
                }]
            }]
        }
    });
    let rest_doc = designos_fig::import_rest_json(&rest.to_string()).expect("import rest");
    let rest_comp = by_name(&rest_doc, "Button");
    assert_eq!(rest_comp.component.prop_default_variables.get("Show icon").map(String::as_str), Some("VariableID:1:7"));
    assert_eq!(rest_comp.component.prop_default_variables.get("Label").map(String::as_str), Some("VariableID:1:8"));
    let rest_set = by_name(&rest_doc, "Chip");
    assert_eq!(rest_set.component.prop_default_variables.get("Size").map(String::as_str), Some("VariableID:1:9"));
}

#[test]
fn native_save_and_load_keep_a_bound_property_default() {
    let mut doc = Document::new("native bound prop defaults");
    let page = doc.add_page("Page 1");
    let cid = doc.alloc_id();
    let mut comp = Node::new(cid, NodeKind::Component(FrameProps::default()));
    comp.name = "Button".into();
    comp.component.props = vec![
        ComponentPropDef { name: "Show icon".into(), kind: PropKind::Boolean, default: PropValue::Bool(false) },
        ComponentPropDef { name: "Label".into(), kind: PropKind::Text, default: PropValue::Text("Submit".into()) },
    ];
    comp.component.prop_default_variables.insert("Show icon".into(), "VariableID:1:7".into());
    comp.component.prop_default_variables.insert("Label".into(), "VariableID:1:8".into());
    doc.insert(page, comp).unwrap();

    let json = doc.to_json().expect("serialize");
    let loaded = Document::from_json(&json).expect("deserialize");
    assert_eq!(
        loaded.get(cid).unwrap().component.prop_default_variables,
        doc.get(cid).unwrap().component.prop_default_variables,
        "native save and load must preserve bound property defaults"
    );
}

#[test]
fn a_fig_round_trip_keeps_a_bound_property_default() {
    let mut doc = import(true);
    let comp_id = by_name(&doc, "Button").id;
    let show_var_id = doc.variables.values().find(|v| v.name == "ShowIcon").unwrap().id.clone();
    let label_var_id = doc.variables.values().find(|v| v.name == "Title").unwrap().id.clone();
    let comp_mut = doc.get_mut(comp_id).unwrap();
    comp_mut.component.prop_default_variables.insert("Show icon".into(), show_var_id);
    comp_mut.component.prop_default_variables.insert("Label".into(), label_var_id);

    let saved = export_fig(&doc).expect("export small .fig through Studio writer");
    let nodes = node_changes(&saved);
    let saved_comp = nodes.iter().find(|n| n["name"] == "Button").expect("component saved");
    let defs = saved_comp["componentPropDefs"].as_array().expect("componentPropDefs");
    let aliased: Vec<&Value> = defs.iter().filter(|d| d["varValue"]["dataType"] == "ALIAS").collect();
    assert_eq!(aliased.len(), 2, "both bound property defaults must write an ALIAS varValue: {defs:?}");

    let reopened = import_fig(&saved).expect("reopen .fig");
    let reopened_comp = by_name(&reopened, "Button");
    assert_eq!(bound_default_name(&reopened, reopened_comp, "Show icon").as_deref(), Some("ShowIcon"));
    assert_eq!(bound_default_name(&reopened, reopened_comp, "Label").as_deref(), Some("Title"));
}

#[test]
fn the_scan_file_lists_a_bound_property_default_in_component_bound_variables() {
    let doc = import_fig_node_changes(&Value::Array(fig_with_bound_prop_defaults()), "scan bound prop defaults").expect("import fig");
    let scan = designos_fig::scan_design_system(&doc).expect("scan design system");
    let button = scan.components.iter().find(|c| c.name == "Button").expect("Button component in scan");
    assert_eq!(
        button.bound_variables,
        Some(vec!["VariableID:1:7".to_string(), "VariableID:1:8".to_string()]),
        "components[].boundVariables must list variable ids bound to property defaults"
    );
    let chip = scan.components.iter().find(|c| c.name == "Chip").expect("Chip component set in scan");
    assert_eq!(
        chip.bound_variables,
        Some(vec!["VariableID:1:9".to_string()]),
        "components[].boundVariables on a component set must list variable ids bound to variant property defaults"
    );
}

#[test]
fn a_property_definition_without_a_binding_serializes_as_before() {
    let doc = import(true);
    let json = doc.to_json().expect("serialize");
    assert!(!json.contains("prop_default_variables"), "unbound property definitions must not write prop_default_variables in native JSON");

    let nodes = node_changes(&export_fig(&doc).expect("export"));
    let saved_comp = nodes.iter().find(|n| n["name"] == "Button").expect("component saved");
    let defs = saved_comp["componentPropDefs"].as_array().expect("componentPropDefs");
    assert!(defs.iter().all(|d| d.get("varValue").is_none()), "unbound property definitions must not write varValue in .fig: {defs:?}");
}

#[test]
fn deleting_a_variable_removes_its_bound_property_default_from_the_scan_file() {
    let mut doc = import_fig_node_changes(&Value::Array(fig_with_bound_prop_defaults()), "delete var scan").expect("import fig");
    doc.variables.remove("VariableID:1:7");
    for updated in designos_doc::dangling_binding_cleanup(&doc, &["VariableID:1:7".to_string()], &[]) {
        doc.nodes.insert(updated.id, updated);
    }

    let scan = designos_fig::scan_design_system(&doc).expect("scan design system");
    let button = scan.components.iter().find(|c| c.name == "Button").expect("Button component in scan");
    assert_eq!(
        button.bound_variables,
        Some(vec!["VariableID:1:8".to_string()]),
        "deleted variable must be removed from prop_default_variables and scan output"
    );
}

#[test]
fn remote_key_bound_property_default_omits_empty_version_and_round_trips() {
    let mut changes = fig_with_bound_prop_defaults();
    let comp_change = changes.iter_mut().find(|n| n["name"] == "Button").unwrap();
    let defs = comp_change["componentPropDefs"].as_array_mut().unwrap();
    defs[0]["varValue"] = json!({
        "dataType": "ALIAS",
        "resolvedDataType": "BOOLEAN",
        "value": { "alias": { "assetRef": { "key": "remote_bool_var_key" } } }
    });

    let doc = import_fig_node_changes(&Value::Array(changes), "remote key prop default").expect("import fig");
    let comp = by_name(&doc, "Button");
    assert_eq!(comp.component.prop_default_variables.get("Show icon").map(String::as_str), Some("key:remote_bool_var_key"));

    let saved = export_fig(&doc).expect("export .fig");
    let nodes = node_changes(&saved);
    let saved_comp = nodes.iter().find(|n| n["name"] == "Button").expect("Button saved");
    let saved_def = saved_comp["componentPropDefs"].as_array().unwrap().iter().find(|d| d["name"] == "Show icon").expect("Show icon def saved");
    let asset_ref = &saved_def["varValue"]["value"]["alias"]["assetRef"];
    assert_eq!(asset_ref["key"], "remote_bool_var_key");
    assert!(asset_ref.get("version").is_none(), "remote key assetRef must omit empty version: {asset_ref:?}");

    let reopened = import_fig(&saved).expect("reopen .fig");
    let reopened_comp = by_name(&reopened, "Button");
    assert_eq!(reopened_comp.component.prop_default_variables.get("Show icon").map(String::as_str), Some("key:remote_bool_var_key"));
}

#[test]
fn rest_import_clears_prop_default_variables_on_expanded_instances_and_requires_default_value_id_object() {
    let rest = json!({
        "name": "REST instance and strict prop defs",
        "document": {
            "id": "0:0",
            "name": "Document",
            "type": "DOCUMENT",
            "children": [{
                "id": "0:1",
                "name": "Page 1",
                "type": "CANVAS",
                "children": [
                    {
                        "id": "1:10",
                        "name": "Outer",
                        "type": "COMPONENT",
                        "absoluteBoundingBox": { "x": 0.0, "y": 0.0, "width": 120.0, "height": 40.0 },
                        "componentPropertyDefinitions": {
                            "Show icon#4:12": {
                                "type": "BOOLEAN",
                                "defaultValue": true,
                                "boundVariables": {
                                    "defaultValue": { "type": "VARIABLE_ALIAS", "id": "VariableID:1:7" }
                                }
                            },
                            "BareString#4:13": {
                                "type": "TEXT",
                                "defaultValue": "Ignored",
                                "boundVariables": {
                                    "defaultValue": "VariableID:1:8"
                                }
                            },
                            "ValueField#4:14": {
                                "type": "TEXT",
                                "defaultValue": "Ignored",
                                "boundVariables": {
                                    "value": { "type": "VARIABLE_ALIAS", "id": "VariableID:1:9" }
                                }
                            }
                        },
                        "children": [{
                            "id": "1:11",
                            "name": "InnerChild",
                            "type": "COMPONENT",
                            "absoluteBoundingBox": { "x": 0.0, "y": 0.0, "width": 40.0, "height": 20.0 },
                            "componentPropertyDefinitions": {
                                "ChildProp#4:15": {
                                    "type": "BOOLEAN",
                                    "defaultValue": false,
                                    "boundVariables": {
                                        "defaultValue": { "type": "VARIABLE_ALIAS", "id": "VariableID:1:7" }
                                    }
                                }
                            },
                            "children": []
                        }]
                    },
                    {
                        "id": "1:20",
                        "name": "OuterInstance",
                        "type": "INSTANCE",
                        "componentId": "1:10",
                        "absoluteBoundingBox": { "x": 160.0, "y": 0.0, "width": 120.0, "height": 40.0 },
                        "children": []
                    }
                ]
            }]
        }
    });

    let doc = designos_fig::import_rest_json(&rest.to_string()).expect("import REST JSON");
    let outer = by_name(&doc, "Outer");
    assert_eq!(outer.component.prop_default_variables.get("Show icon").map(String::as_str), Some("VariableID:1:7"));
    assert!(!outer.component.prop_default_variables.contains_key("BareString"), "bare string without id object must not be accepted");
    assert!(!outer.component.prop_default_variables.contains_key("ValueField"), "boundVariables.value on a definition must not be accepted");

    let inst = by_name(&doc, "OuterInstance");
    assert!(inst.component.prop_default_variables.is_empty(), "instance root must not store prop_default_variables");
    for &kid in &inst.children {
        assert!(doc.get(kid).unwrap().component.prop_default_variables.is_empty(), "expanded instance child must not inherit prop_default_variables");
    }
}

#[test]
fn migrating_a_boolean_property_to_a_slot_removes_its_bound_default_from_the_scan_and_undo_restores_it() {
    let mut doc = import_fig_node_changes(&Value::Array(fig_with_bound_prop_defaults()), "slot migration scan").expect("import fig");
    let comp_id = by_name(&doc, "Button").id;
    let icon_id = doc.alloc_id();
    let mut icon = Node::new(icon_id, NodeKind::Frame(FrameProps::default()));
    icon.name = "icon".into();
    icon.component.bindings.insert(designos_doc::OverrideField::Visible, "Show icon".into());
    doc.insert(comp_id, icon).unwrap();

    let mut h = designos_doc::History::default();
    let cmd = designos_doc::migrate_region_to_slot(&mut doc, icon_id).expect("migrate boolean container to slot");
    h.exec(&mut doc, cmd).unwrap();

    assert!(
        !doc.get(comp_id).unwrap().component.prop_default_variables.contains_key("Show icon"),
        "slot migration must remove the migrated property's bound default"
    );
    let scan = designos_fig::scan_design_system(&doc).expect("scan after slot migration");
    let button = scan.components.iter().find(|c| c.name == "Button").expect("Button in scan");
    assert_eq!(button.bound_variables, Some(vec!["VariableID:1:8".to_string()]), "scan must no longer list the variable of a property migrated to a slot");

    h.undo(&mut doc).unwrap();
    assert_eq!(
        doc.get(comp_id).unwrap().component.prop_default_variables.get("Show icon").map(String::as_str),
        Some("VariableID:1:7"),
        "undo must restore the bound property default"
    );
}
