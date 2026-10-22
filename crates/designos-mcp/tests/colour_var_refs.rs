//! `var:` colours on `update_node`, `create_node` and `apply_batch` bind the variable the way
//! `apply_spec` does, and an unknown or mistyped variable is reported as such.
#![allow(clippy::result_large_err)]

mod support;

use designos_doc::{NodeId, Paint};
use designos_mcp::engine::Engine;
use serde_json::{json, Value};
use support::*;

// ---- var: references -----------------------------------------------------------------------------

#[test]
fn update_node_binds_a_variable_to_a_fill_and_to_text_colour() {
    let mut e = palette();
    let rect = apply(&mut e, json!({"kind": "rectangle", "name": "Box", "width": 60, "height": 30, "fill": "#CCCCCC"}));
    let text = apply(&mut e, json!({"kind": "text", "name": "Label", "text": "Hi", "width": 60, "height": 24}));

    let r = e.call("designos_update_node", json!({"node_id": rect.0, "fill": "var:Color/Danger"})).unwrap();
    let t = e.call("designos_update_node", json!({"node_id": text.0, "fill": "var:Color/Danger"})).unwrap();

    assert_eq!(r["changed"], true, "{r}");
    assert_eq!(t["changed"], true, "{t}");
    assert_bound_to(&e, rect, "color/danger", DANGER);
    assert_bound_to(&e, text, "color/danger", DANGER);
    assert!(!node(&e, rect).style_refs.contains_key("fill"));
}

#[test]
fn a_var_fill_replaces_an_earlier_binding_and_one_undo_restores_it() {
    let mut e = palette();
    let id = bound_frame(&mut e);

    e.call("designos_update_node", json!({"node_id": id.0, "fill": "var:Color/Danger"})).unwrap();
    assert_bound_to(&e, id, "color/danger", DANGER);

    e.call("designos_undo", json!({})).unwrap();
    assert_bound_to(&e, id, "color/primary", PRIMARY);
}

#[test]
fn a_var_fill_keeps_one_paint_even_when_the_node_had_several() {
    let mut e = palette();
    let id = apply(&mut e, json!({"kind": "rectangle", "name": "Two", "width": 60, "height": 30}));
    let paints = [Paint::solid(colour("#111111")), Paint::solid(colour("#222222"))];
    e.call("designos_update_node", json!({"node_id": id.0, "fills": paints})).unwrap();

    e.call("designos_update_node", json!({"node_id": id.0, "fill": "var:Color/Danger"})).unwrap();

    assert_bound_to(&e, id, "color/danger", DANGER);
}

#[test]
fn a_bound_fill_is_unchanged_when_the_same_variable_is_sent_again() {
    let mut e = palette();
    let id = bound_frame(&mut e);

    let reply = e.call("designos_update_node", json!({"node_id": id.0, "fill": "var:Color/Primary"})).unwrap();

    assert_eq!(reply["changed"], false, "{reply}");
    assert_bound_to(&e, id, "color/primary", PRIMARY);
}

#[test]
fn an_unknown_variable_is_reported_as_unknown_variable_with_the_valid_names() {
    let mut e = palette();
    let id = bound_frame(&mut e);
    let before = node(&e, id).clone();

    let err = err_json(e.call("designos_update_node", json!({"node_id": id.0, "fill": "var:Color/Dnager"})).unwrap_err());

    assert_eq!(err["code"], "UNKNOWN_VARIABLE", "{err}");
    assert_eq!(err["path"], "/fill", "{err}");
    let options: Vec<&str> = err["valid_options"].as_array().expect("valid_options").iter().filter_map(Value::as_str).collect();
    assert!(options.contains(&"Color/Danger"), "the closest names are offered: {options:?}");
    assert_eq!(node(&e, id), &before, "a refused write leaves the node alone");
}

#[test]
fn a_number_variable_is_refused_for_a_colour() {
    let mut e = palette();
    let id = bound_frame(&mut e);

    let err = err_json(e.call("designos_update_node", json!({"node_id": id.0, "fill": "var:Color/Gap"})).unwrap_err());

    assert_eq!(err["code"], "VARIABLE_TYPE_MISMATCH", "{err}");
    assert_bound_to(&e, id, "color/primary", PRIMARY);
}

#[test]
fn a_bad_colour_is_still_an_out_of_range_value() {
    let mut e = palette();
    let id = bound_frame(&mut e);

    let err = err_json(e.call("designos_update_node", json!({"node_id": id.0, "fill": "not-a-colour"})).unwrap_err());

    assert_eq!(err["code"], "VALUE_OUT_OF_RANGE", "{err}");
}

#[test]
fn batch_update_and_insert_bind_variables_in_one_undo_step() {
    let mut e = palette();
    let page = page(&e);
    let rect = apply(&mut e, json!({"kind": "rectangle", "name": "Box", "width": 60, "height": 30, "fill": "#CCCCCC"}));

    let reply = e
        .call(
            "designos_apply_batch",
            json!({"ops": [
                {"op": "update", "node": rect.0, "fill": "var:Color/Danger"},
                {"op": "insert", "as": "t", "parent": page, "kind": "text", "text": "Hi", "width": 60, "height": 24, "fill": "var:Color/Primary"}
            ]}),
        )
        .unwrap();

    let created = NodeId(reply["created"][0].as_u64().expect("created id"));
    assert_bound_to(&e, rect, "color/danger", DANGER);
    assert_bound_to(&e, created, "color/primary", PRIMARY);

    e.call("designos_undo", json!({})).unwrap();
    assert!(e.doc.get(created).is_none(), "the insert is undone");
    assert!(fill_bindings(&e, rect).is_empty(), "the update is undone in the same step");
    assert_eq!(drawn_paints(&e, rect).len(), 1);
    assert_eq!(solid_hex(&drawn_paints(&e, rect)[0]), "#CCCCCC");
}

#[test]
fn a_batch_with_an_unknown_variable_names_the_op_and_changes_nothing() {
    let mut e = palette();
    let rect = apply(&mut e, json!({"kind": "rectangle", "name": "Box", "width": 60, "height": 30, "fill": "#CCCCCC"}));
    let before = node(&e, rect).clone();

    let err = err_json(
        e.call(
            "designos_apply_batch",
            json!({"ops": [{"op": "update", "node": rect.0, "fill": "var:Color/Primary"}, {"op": "update", "node": rect.0, "fill": "var:Color/Nope"}]}),
        )
        .unwrap_err(),
    );

    assert_eq!(err["code"], "BATCH_OP_FAILED", "{err}");
    assert_eq!(err["failed_op"], 1, "{err}");
    assert_eq!(err["cause"]["code"], "UNKNOWN_VARIABLE", "{err}");
    assert_eq!(err["cause"]["path"], "/ops/1/fill", "{err}");
    assert!(err["cause"]["valid_options"].as_array().is_some_and(|o| !o.is_empty()), "{err}");
    assert_eq!(node(&e, rect), &before, "the batch is atomic");
}

#[test]
fn create_node_binds_a_variable_using_the_parents_mode() {
    let mut e = palette();
    let dark = apply(&mut e, json!({"kind": "frame", "name": "Dark panel", "width": 200, "height": 100}));
    e.doc.get_mut(dark).unwrap().variables.modes.insert("theme".into(), "dark".into());

    let reply =
        e.call("designos_create_node", json!({"parent_id": dark.0, "kind": "rectangle", "width": 40, "height": 40, "fill": "var:Theme/Surface"})).unwrap();

    let id = NodeId(reply["node"]["id"].as_u64().unwrap());
    assert_eq!(fill_bindings(&e, id), vec![("fills/0".to_string(), "theme/surface".to_string())]);
    let raw = raw_paints(&e, id);
    assert!(raw.len() == 1 && solid_hex(&raw[0]) == "#18181B", "the stored paint is the dark value: {raw:?}");
}

#[test]
fn create_node_with_an_unknown_variable_is_unknown_variable() {
    let mut e = palette();
    let err = err_json(e.call("designos_create_node", json!({"parent_id": page(&e), "kind": "rectangle", "fill": "var:Color/Nope"})).unwrap_err());
    assert_eq!(err["code"], "UNKNOWN_VARIABLE", "{err}");
    assert_eq!(err["path"], "/fill", "{err}");
}

#[test]
fn a_variable_made_earlier_in_the_same_batch_can_be_bound() {
    let mut e = Engine::default();
    let page = page(&e);

    e.call(
        "designos_apply_batch",
        json!({"ops": [
            {"op": "set_variables",
             "upsert_collections": [{"id": "brand", "name": "Brand", "modes": [{"id": "light", "name": "Light"}], "default_mode": "light"}],
             "upsert_variables": [{"id": "brand/accent", "name": "Accent", "collection": "brand", "kind": "color", "values": {"light": "#E36323"}}]},
            {"op": "insert", "as": "box", "parent": page, "kind": "rectangle", "width": 40, "height": 40, "fill": "var:Brand/Accent"},
            {"op": "insert", "parent": page, "kind": "text", "text": "Hi", "width": 40, "height": 20},
            {"op": "update", "node": "$prev", "fill": "var:Brand/Accent"}
        ]}),
    )
    .expect("the variable exists by the time the later ops run");

    let mut bound = Vec::new();
    e.doc.walk(NodeId(page), |n, _| {
        if n.variables.bindings.get("fills/0").map(String::as_str) == Some("brand/accent") {
            bound.push(n.id);
        }
    });
    assert_eq!(bound.len(), 2, "both nodes are bound: {bound:?}");
}

#[test]
fn a_variable_without_a_value_in_the_nodes_mode_is_bound_and_the_reply_says_so() {
    let mut e = palette();
    e.call(
        "designos_apply_batch",
        json!({"ops": [{"op": "set_variables", "upsert_variables": [
            {"id": "theme/accent", "name": "Accent", "collection": "theme", "kind": "color", "values": {"light": "#E36323"}}
        ]}]}),
    )
    .unwrap();
    let id = apply(&mut e, json!({"kind": "frame", "name": "Dark panel", "width": 100, "height": 40}));
    e.doc.get_mut(id).unwrap().variables.modes.insert("theme".into(), "dark".into());

    let reply = e.call("designos_update_node", json!({"node_id": id.0, "fill": "var:Theme/Accent"})).unwrap();

    assert_eq!(fill_bindings(&e, id), vec![("fills/0".to_string(), "theme/accent".to_string())]);
    let warnings = reply["warnings"].as_array().unwrap_or_else(|| panic!("the reply carries the warning: {reply}"));
    assert_eq!(warnings[0]["code"], "VARIABLE_MODE_FALLBACK", "{reply}");
    let plain = e.call("designos_update_node", json!({"node_id": id.0, "fill": "var:Color/Primary"})).unwrap();
    assert!(plain.get("warnings").is_none(), "no warnings key when there is nothing to say: {plain}");
}

#[test]
fn an_empty_variable_reference_is_an_unknown_variable_not_a_bad_colour() {
    let mut e = palette();
    let id = bound_frame(&mut e);

    let err = err_json(e.call("designos_update_node", json!({"node_id": id.0, "fill": "var:"})).unwrap_err());

    assert_eq!(err["code"], "UNKNOWN_VARIABLE", "{err}");
    assert_bound_to(&e, id, "color/primary", PRIMARY);
}
