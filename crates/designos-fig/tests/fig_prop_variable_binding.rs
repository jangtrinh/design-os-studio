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
