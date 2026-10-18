//! Newer `.fig` files keep a component property value in `varValue.value` and leave
//! `value` (assignments) or `initialValue` (definitions) empty. The importer reads
//! both, so an instance that hides an icon through a boolean property hides it.
//! Hand-written kiwi-shaped JSON; nothing here comes from a real file.

use designos_doc::*;
use designos_fig::import_fig_node_changes;
use serde_json::{json, Value};

fn g(s: u64, l: u64) -> Value {
    json!({"sessionID": s, "localID": l})
}

fn node(id: (u64, u64), parent: (u64, u64), pos: &str, ty: &str, name: &str, extra: Value) -> Value {
    let mut v = json!({
        "guid": g(id.0, id.1), "parentIndex": {"guid": g(parent.0, parent.1), "position": pos},
        "type": ty, "name": name, "phase": "CREATED", "visible": true, "opacity": 1.0,
        "size": {"x": 10.0, "y": 10.0},
        "transform": {"m00": 1.0, "m01": 0.0, "m02": 0.0, "m10": 0.0, "m11": 1.0, "m12": 0.0}
    });
    for (k, x) in extra.as_object().unwrap() {
        v[k] = x.clone();
    }
    v
}

/// A boolean value in the shape newer files use: `value` empty, the payload in `varValue`.
fn var_bool(b: bool) -> (Value, Value) {
    (json!({}), json!({"dataType": "BOOLEAN", "resolvedDataType": "BOOLEAN", "value": {"boolValue": b}}))
}

/// A page with a `Button` component (label, an icon instance whose visibility is bound
/// to the boolean property `Show icon`, and a text bound to `Label`) and one instance of it.
/// `def_default` and `assigned` give the property definition's default and the instance's
/// assignment in the new shape; `None` leaves them out.
fn fixture(def_default: Option<bool>, assigned: Option<bool>) -> Value {
    let (no_value, def_var) = def_default.map(var_bool).unwrap_or_else(|| (json!({"boolValue": true}), Value::Null));
    let mut def = json!({"id": g(7, 1), "name": "Show icon", "type": "BOOL", "initialValue": no_value});
    if !def_var.is_null() {
        def["varValue"] = def_var;
    }
    let mut instance = json!({
        "size": {"x": 100.0, "y": 40.0},
        "symbolData": {"symbolID": g(5, 3), "uniformScaleFactor": 1.0, "symbolOverrides": []}});
    if let Some(b) = assigned {
        let (value, var_value) = var_bool(b);
        instance["componentPropAssignments"] = json!([{"defID": g(7, 1), "value": value, "varValue": var_value}]);
    }
    json!([
        node((0, 0), (0, 0), "!", "DOCUMENT", "Document", json!({})),
        node((0, 1), (0, 0), "a", "CANVAS", "Page", json!({"backgroundColor": {"r": 1.0, "g": 1.0, "b": 1.0, "a": 1.0}})),
        node((5, 1), (0, 1), "a", "SYMBOL", "Icon", json!({"size": {"x": 20.0, "y": 20.0}})),
        node((5, 3), (0, 1), "b", "SYMBOL", "Button", json!({"size": {"x": 100.0, "y": 40.0}, "componentPropDefs": [def]})),
        node(
            (5, 5),
            (5, 3),
            "a",
            "INSTANCE",
            "icon",
            json!({
            "size": {"x": 20.0, "y": 20.0},
            "symbolData": {"symbolID": g(5, 1), "symbolOverrides": [], "uniformScaleFactor": 1.0},
            "componentPropRefs": [{"componentPropNodeField": "VISIBLE", "defID": g(7, 1)}]})
        ),
        node((1, 1), (0, 1), "c", "INSTANCE", "Buy", instance)
    ])
}

fn import(def_default: Option<bool>, assigned: Option<bool>) -> Document {
    import_fig_node_changes(&fixture(def_default, assigned), "prop var value").expect("import")
}

fn find<'a>(doc: &'a Document, name: &str) -> &'a Node {
    doc.nodes.values().find(|n| n.name == name).unwrap_or_else(|| panic!("no node {name}"))
}

fn icon_of_buy(doc: &Document) -> &Node {
    let buy = find(doc, "Buy");
    buy.children.iter().map(|c| doc.get(*c).unwrap()).find(|c| c.name == "icon").expect("the instance has its icon")
}

#[test]
fn assignment_in_var_value_hides_the_bound_icon() {
    let doc = import(None, Some(false));
    assert_eq!(find(&doc, "Buy").component.values.get("Show icon"), Some(&PropValue::Bool(false)), "the assignment is kept");
    assert!(!icon_of_buy(&doc).visible, "the icon is bound to a property the instance sets to false");
}

#[test]
fn assignment_in_var_value_can_also_show_the_icon() {
    let doc = import(None, Some(true));
    assert_eq!(find(&doc, "Buy").component.values.get("Show icon"), Some(&PropValue::Bool(true)));
    assert!(icon_of_buy(&doc).visible);
}

#[test]
fn no_assignment_keeps_the_main_components_visibility() {
    let doc = import(None, None);
    assert!(find(&doc, "Buy").component.values.is_empty());
    assert!(icon_of_buy(&doc).visible);
}

#[test]
fn definition_default_in_var_value_is_read() {
    let doc = import(Some(false), None);
    let button = find(&doc, "Button");
    let def = button.component.props.iter().find(|d| d.name == "Show icon").expect("definition imported");
    assert_eq!(def.default, PropValue::Bool(false), "default read from varValue when initialValue is empty");
}

#[test]
fn value_still_wins_over_var_value_when_both_are_present() {
    let mut changes = fixture(None, Some(true));
    // an old-shape assignment next to a stale varValue: the plain value is the one the file means
    let buy = changes.as_array_mut().unwrap().iter_mut().find(|n| n["name"] == "Buy").unwrap();
    buy["componentPropAssignments"] = json!([{"defID": g(7, 1), "value": {"boolValue": false}, "varValue": {"value": {"boolValue": true}}}]);
    let doc = import_fig_node_changes(&changes, "both").expect("import");
    assert_eq!(find(&doc, "Buy").component.values.get("Show icon"), Some(&PropValue::Bool(false)));
}
