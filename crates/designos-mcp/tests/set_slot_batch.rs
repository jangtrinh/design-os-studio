//! `designos_apply_batch` `set_slot` fills the real instance slot.

use designos_doc::{ComponentPropDef, NodeId, NodeKind, OverrideField, PropKind, PropValue};
use designos_mcp::engine::Engine;
use serde_json::{json, Value};

fn engine() -> Engine {
    Engine { doc: designos_doc::fixtures::card_with_slots(), ..Engine::default() }
}

fn page_id(e: &Engine) -> u64 {
    e.doc.pages[0].0
}

fn place_instance(e: &mut Engine, name: &str, slots: Option<Value>) -> u64 {
    let mut spec = json!({"kind": "instance", "name": name, "component": "Card"});
    if let Some(slots) = slots {
        spec["slots"] = slots;
    }
    e.call("designos_apply_spec", json!({"parent_id": page_id(e), "spec": spec})).expect("instance")["root_node_id"].as_u64().unwrap()
}

fn slot_node(e: &Engine, instance: u64, name: &str) -> NodeId {
    designos_doc::resolve_override_path(&e.doc, NodeId(instance), name).expect("slot")
}

fn child_names(e: &Engine, id: NodeId) -> Vec<String> {
    e.doc.get(id).expect("node").children.iter().map(|c| e.doc.get(*c).map(|n| n.name.clone()).unwrap_or_default()).collect()
}

fn child_named(e: &Engine, parent: NodeId, name: &str) -> Option<NodeId> {
    e.doc.get(parent)?.children.iter().copied().find(|id| e.doc.get(*id).is_some_and(|n| n.name == name))
}

fn main_card(e: &Engine) -> NodeId {
    e.doc.nodes.values().find(|n| n.name == "Card" && matches!(n.kind, NodeKind::Component(_))).expect("Card component").id
}

fn hello_fill() -> Value {
    json!([{"kind": "text", "name": "Hello", "text": "Hello"}])
}

fn batch(e: &mut Engine, ops: Value) -> Result<Value, Value> {
    e.call("designos_apply_batch", json!({ "ops": ops })).map_err(|err| err.to_json_value())
}

fn get_node(e: &mut Engine, id: u64) -> Value {
    e.call("designos_get_node", json!({ "node_id": id })).expect("get_node")
}

fn slot_text(e: &mut Engine, slot: NodeId, name: &str) -> Option<String> {
    let children = get_node(e, slot.0)["node"]["children"].as_array()?.clone();
    for id in children.iter().filter_map(|v| v.as_u64()) {
        let child = get_node(e, id);
        if child["node"]["name"] == name {
            return child["node"]["kind"]["Text"]["content"].as_str().map(str::to_string);
        }
    }
    None
}

fn subtree(doc: &designos_doc::Document, id: NodeId) -> Value {
    let n = doc.get(id).expect("node");
    let kind = serde_json::to_value(&n.kind).expect("kind");
    let tag = kind.as_object().and_then(|o| o.keys().next().cloned()).unwrap_or_else(|| kind.to_string());
    let text = kind.get("Text").and_then(|t| t.get("content")).cloned();
    let comp = serde_json::to_value(&n.component).expect("component");
    json!({
        "name": n.name,
        "kind": tag,
        "text": text,
        "w": n.size.x,
        "h": n.size.y,
        "x": n.transform.tx,
        "y": n.transform.ty,
        "visible": n.visible,
        "opacity": n.opacity,
        "overrides": comp.get("overrides"),
        "is_slot_content": comp.get("is_slot_content"),
        "layout_child": serde_json::to_value(n.layout_child).expect("layout"),
        "children": n.children.iter().map(|c| subtree(doc, *c)).collect::<Vec<_>>(),
    })
}

fn slot_values(doc: &designos_doc::Document, id: NodeId) -> Value {
    let n = doc.get(id).expect("node");
    let mut map = serde_json::Map::new();
    for (key, value) in &n.component.values {
        let encoded = match value {
            PropValue::Slot(ids) => json!({"slot": ids.iter().map(|i| doc.get(*i).map(|n| n.name.clone())).collect::<Vec<_>>()}),
            other => serde_json::to_value(other).expect("prop"),
        };
        map.insert(key.clone(), encoded);
    }
    Value::Object(map)
}

#[test]
fn set_slot_on_top_level_instance_creates_child_visible_via_get_node() {
    let mut e = engine();
    let page = page_id(&e);
    let inst = place_instance(&mut e, "CardInst", None);
    let slot = slot_node(&e, inst, "body");
    let page_children = get_node(&mut e, page)["node"]["children"].clone();

    batch(&mut e, json!([{"op": "set_slot", "node": inst, "slot": "body", "fill": hello_fill()}])).expect("set_slot");

    assert_eq!(slot_text(&mut e, slot, "Hello").as_deref(), Some("Hello"), "top-level slot must hold the fill, visible via get_node");
    assert_eq!(get_node(&mut e, page)["node"]["children"], page_children, "set_slot must not add a sibling of the instance");
    assert_eq!(get_node(&mut e, inst)["node"]["name"], "CardInst");
}

#[test]
fn set_slot_on_nested_instance_creates_child_visible_via_get_node() {
    let mut e = engine();
    let page = page_id(&e);
    let card = main_card(&e);
    let main_slot = slot_node(&e, card.0, "body");
    let main_before = child_names(&e, main_slot);
    let shell = e
        .call(
            "designos_apply_spec",
            json!({
                "parent_id": page,
                "spec": {
                    "kind": "component",
                    "name": "Shell",
                    "width": 400.0,
                    "height": 400.0,
                    "children": [{ "kind": "instance", "name": "Inner", "component": format!("node:{}", card.0) }]
                }
            }),
        )
        .expect("shell");
    let shell_id = shell["root_node_id"].as_u64().unwrap();
    let shell_inst = e
        .call(
            "designos_apply_spec",
            json!({
                "parent_id": page,
                "spec": { "kind": "instance", "name": "ShellInst", "component": format!("node:{shell_id}") }
            }),
        )
        .expect("shell instance");
    let shell_inst_id = shell_inst["root_node_id"].as_u64().unwrap();
    let inner = child_named(&e, NodeId(shell_inst_id), "Inner").expect("nested instance");
    let slot = slot_node(&e, inner.0, "body");

    batch(&mut e, json!([{"op": "set_slot", "node": inner.0, "slot": "body", "fill": hello_fill()}])).expect("set_slot on nested instance");

    assert_eq!(slot_text(&mut e, slot, "Hello").as_deref(), Some("Hello"), "nested instance slot must hold the fill, visible via get_node");
    assert_eq!(child_names(&e, main_slot), main_before, "the main component slot must stay unchanged");
}

#[test]
fn set_slot_one_undo_removes_the_child() {
    let mut e = engine();
    let page = page_id(&e);
    let inst = place_instance(&mut e, "CardInst", None);
    let slot = slot_node(&e, inst, "body");
    let depth = e.history.undo_depth();

    batch(
        &mut e,
        json!([
            { "op": "insert", "parent": page, "kind": "rectangle", "name": "Marker", "width": 10.0, "height": 10.0 },
            { "op": "set_slot", "node": inst, "slot": "body", "fill": hello_fill() }
        ]),
    )
    .expect("batch");

    assert_eq!(slot_text(&mut e, slot, "Hello").as_deref(), Some("Hello"));
    assert!(child_named(&e, NodeId(page), "Marker").is_some());
    assert_eq!(e.history.undo_depth(), depth + 1, "set_slot shares one undo group with the rest of the batch");

    let undone = e.call("designos_undo", json!({})).unwrap();
    assert_eq!(undone["changed"], true);
    assert!(slot_text(&mut e, slot, "Hello").is_none(), "one undo removes the slot child");
    assert!(child_named(&e, NodeId(page), "Marker").is_none(), "one undo removes the rest of the batch");
    assert_eq!(e.history.undo_depth(), depth);
}

#[test]
fn set_slot_body_fill_matches_apply_spec_and_records_slot_value() {
    let fill = json!([
        {"kind": "text", "name": "Hello", "text": "Hello"},
        {"kind": "frame", "name": "Box", "height": 20.0}
    ]);
    let mut e = engine();
    let spec_inst = place_instance(&mut e, "A", Some(json!({ "body": fill.clone() })));
    let inst = place_instance(&mut e, "B", None);
    let body = slot_node(&e, inst, "body");
    let placeholder = child_named(&e, body, "DefaultBodyPlaceholder").expect("default body content");

    let reply = batch(&mut e, json!([{"op": "set_slot", "node": inst, "slot": "body", "fill": fill}])).expect("set_slot");

    assert_eq!(child_names(&e, body), vec!["Hello".to_string(), "Box".to_string()]);
    assert!(e.doc.get(placeholder).is_none(), "DefaultBodyPlaceholder is gone from the doc");
    assert!(e.doc.get(body).unwrap().component.overrides.contains(&OverrideField::SlotContent));
    let hello_id = e.doc.get(body).unwrap().children[0];
    assert_eq!(reply["name_to_id"]["Hello"].as_u64(), Some(hello_id.0), "name_to_id maps Hello to the created text");
    let box_id = e.doc.get(body).unwrap().children[1];
    assert_eq!(e.doc.get(NodeId(inst)).unwrap().component.values.get("body"), Some(&PropValue::Slot(vec![hello_id, box_id])));
    assert_eq!(subtree(&e.doc, slot_node(&e, spec_inst, "body")), subtree(&e.doc, body), "set_slot body must equal apply_spec slots body");
    assert_eq!(slot_values(&e.doc, NodeId(spec_inst)), slot_values(&e.doc, NodeId(inst)));
    let spec_size = e.doc.get(NodeId(spec_inst)).unwrap().size;
    let inst_size = e.doc.get(NodeId(inst)).unwrap().size;
    assert_eq!((spec_size.x, spec_size.y), (inst_size.x, inst_size.y));
}

#[test]
fn set_slot_token_fill_commits_variables_and_one_undo_removes_them() {
    let mut e = engine();
    let inst = place_instance(&mut e, "B", None);
    let vars_before = e.doc.variables.len();
    let reply = batch(
        &mut e,
        json!([{
            "op": "set_slot",
            "node": inst,
            "slot": "body",
            "fill": [{"kind": "frame", "name": "Tok", "height": 20.0, "fill": "$easeui.text.primary"}]
        }]),
    )
    .expect("token fill");
    let deprecated: Vec<&Value> = reply["warnings"].as_array().into_iter().flatten().filter(|w| w["code"] == "DEPRECATED_EASEUI_TOKEN").collect();
    assert_eq!(deprecated.len(), 1, "exactly one deprecated-token warning: {}", reply["warnings"]);
    assert_eq!(deprecated[0]["token"], "$easeui.text.primary");

    let body = slot_node(&e, inst, "body");
    let tok = e.doc.get(body).unwrap().children[0];
    let tok_json = serde_json::to_string(e.doc.get(tok).unwrap()).unwrap();
    let bound: Vec<&String> = e.doc.variables.keys().filter(|key| tok_json.contains(key.as_str())).collect();
    assert!(e.doc.variables.len() > vars_before, "the token's variable is committed");
    assert!(!bound.is_empty(), "the slot child is bound to a variable that exists");

    e.call("designos_undo", json!({})).unwrap();
    assert_eq!(e.doc.variables.len(), vars_before, "one undo removes the variables too");
}

#[test]
fn set_slot_false_hides_footer_keeps_children_and_undo_shows_it() {
    let mut e = engine();
    let inst = place_instance(&mut e, "B", None);
    let footer = slot_node(&e, inst, "footer");
    let kids = child_names(&e, footer);

    batch(&mut e, json!([{"op": "set_slot", "node": inst, "slot": "footer", "fill": false}])).expect("disable footer");

    assert!(!e.doc.get(footer).unwrap().visible);
    assert!(e.doc.get(footer).unwrap().component.overrides.contains(&OverrideField::Visible));
    assert_eq!(child_names(&e, footer), kids);
    e.call("designos_undo", json!({})).unwrap();
    assert!(e.doc.get(footer).unwrap().visible, "undo shows the slot again");
}

#[test]
fn set_slot_content_survives_main_component_change() {
    let mut e = engine();
    let card = main_card(&e);
    let inst = place_instance(&mut e, "B", None);
    batch(&mut e, json!([{"op": "set_slot", "node": inst, "slot": "body", "fill": hello_fill()}])).expect("fill");

    let main_body = slot_node(&e, card.0, "body");
    let extra_id = e.doc.alloc_id();
    let mut extra = designos_doc::Node::frame(extra_id, designos_doc::Rect::new(0.0, 0.0, 30.0, 10.0));
    extra.name = "MainExtra".into();
    e.doc.insert(main_body, extra).unwrap();
    let cmd = designos_doc::sync_instances(&mut e.doc, card);
    cmd.apply(&mut e.doc).unwrap();

    let body = slot_node(&e, inst, "body");
    assert_eq!(child_names(&e, body), vec!["Hello".to_string()], "slot content survives a main change");
}

#[test]
fn set_slot_dry_run_matches_commit() {
    let setup = || {
        let mut e = engine();
        let inst = place_instance(&mut e, "B", None);
        (e, inst)
    };
    let (mut dry_engine, dry_inst) = setup();
    let (mut commit_engine, commit_inst) = setup();
    assert_eq!(dry_inst, commit_inst);
    let page = page_id(&dry_engine);
    let ops = json!([
        {"op": "set_slot", "node": dry_inst, "slot": "body", "fill": hello_fill()},
        {"op": "insert", "parent": page, "kind": "rectangle", "name": "After", "width": 10.0, "height": 10.0}
    ]);
    let before = dry_engine.doc.to_json().unwrap();
    let depth = dry_engine.history.undo_depth();
    let dry = dry_engine.call("designos_apply_batch", json!({"ops": ops.clone(), "dry_run": true})).expect("dry_run");
    assert_eq!(dry_engine.doc.to_json().unwrap(), before, "dry_run must not mutate");
    assert_eq!(dry_engine.history.undo_depth(), depth);
    let committed = commit_engine.call("designos_apply_batch", json!({"ops": ops})).expect("commit");
    for key in ["created", "name_to_id", "changed", "touched_roots", "created_count", "touched_bounds"] {
        assert_eq!(dry[key], committed[key], "dry_run and commit differ on {key}");
    }
    assert_eq!(child_names(&commit_engine, slot_node(&commit_engine, commit_inst, "body")), vec!["Hello".to_string()]);
    for id in committed["created"].as_array().unwrap() {
        let id = id.as_u64().unwrap();
        assert!(commit_engine.doc.get(NodeId(id)).is_some(), "created id {id} must exist");
    }
}

#[test]
fn set_slot_fill_after_false_shows_the_slot() {
    let mut e = engine();
    let inst = place_instance(&mut e, "B", None);
    let header = slot_node(&e, inst, "header");

    batch(&mut e, json!([{"op": "set_slot", "node": inst, "slot": "header", "fill": false}])).expect("hide header");
    assert!(!e.doc.get(header).unwrap().visible);
    assert!(e.doc.get(header).unwrap().component.overrides.contains(&OverrideField::Visible));

    let fill = json!([{"kind": "instance", "name": "FilledHeader", "component": "CardHeader"}]);
    batch(&mut e, json!([{"op": "set_slot", "node": inst, "slot": "header", "fill": fill}])).expect("fill header");

    assert!(e.doc.get(header).unwrap().visible, "a filled header is visible");
    assert!(!e.doc.get(header).unwrap().component.overrides.contains(&OverrideField::Visible), "a fill removes the Visible override this instance set");
    assert_eq!(child_names(&e, header), vec!["FilledHeader".to_string()]);
}

#[test]
fn set_slot_keeps_a_shown_header_visible_when_the_main_slot_is_hidden() {
    let mut e = engine();
    let card = main_card(&e);
    let main_header = slot_node(&e, card.0, "header");
    e.call("designos_update_node", json!({"node_id": main_header.0, "visible": false})).expect("hide main header");
    assert!(!e.doc.get(main_header).unwrap().visible);

    let inst = place_instance(&mut e, "B", None);
    let header = slot_node(&e, inst, "header");
    assert!(!e.doc.get(header).unwrap().visible, "a new instance inherits the hidden main slot");
    e.call("designos_update_node", json!({"node_id": header.0, "visible": true})).expect("show instance header");
    assert!(e.doc.get(header).unwrap().visible);

    let fill = json!([{"kind": "instance", "name": "H", "component": "CardHeader"}]);
    batch(&mut e, json!([{"op": "set_slot", "node": inst, "slot": "header", "fill": fill}])).expect("fill header");

    assert!(e.doc.get(header).unwrap().visible, "a slot shown on this instance stays shown after a fill");
    assert_eq!(child_names(&e, header), vec!["H".to_string()]);
}

#[test]
fn set_slot_keeps_a_prop_hidden_header_hidden() {
    let mut e = engine();
    let card = main_card(&e);
    let main_header = slot_node(&e, card.0, "header");
    let added =
        designos_doc::add_prop(&mut e.doc, card, ComponentPropDef { name: "Show header".into(), kind: PropKind::Boolean, default: PropValue::Bool(true) });
    added.apply(&mut e.doc).expect("add prop");
    let bound = designos_doc::bind_prop(&mut e.doc, main_header, OverrideField::Visible, Some("Show header"));
    bound.apply(&mut e.doc).expect("bind visibility");

    let inst = place_instance(&mut e, "B", None);
    let header = slot_node(&e, inst, "header");
    let hidden = designos_doc::set_instance_prop(&mut e.doc, NodeId(inst), "Show header", Some(PropValue::Bool(false)));
    hidden.apply(&mut e.doc).expect("set prop");
    let before = e.doc.get(header).unwrap().clone();
    assert!(!before.visible, "the Boolean property hides the header");

    let fill = json!([{"kind": "instance", "name": "H", "component": "CardHeader"}]);
    batch(&mut e, json!([{"op": "set_slot", "node": inst, "slot": "header", "fill": fill.clone()}])).expect("fill header");
    assert_eq!(e.doc.get(header).unwrap().visible, before.visible, "a fill keeps the visibility its Boolean property gives");

    let reference = e
        .call(
            "designos_apply_spec",
            json!({
                "parent_id": page_id(&e),
                "spec": {
                    "kind": "instance",
                    "component": "Card",
                    "props": {"Show header": false},
                    "slots": {"header": fill}
                }
            }),
        )
        .expect("apply_spec")["root_node_id"]
        .as_u64()
        .unwrap();
    assert_eq!(subtree(&e.doc, header), subtree(&e.doc, slot_node(&e, reference, "header")), "set_slot matches apply_spec for the same property and fill");
}

#[test]
fn set_slot_unknown_slot_errors_and_changes_nothing() {
    let mut e = engine();
    let page = page_id(&e);
    let inst = place_instance(&mut e, "CardInst", None);
    let before = e.doc.to_json().unwrap();
    let depth = e.history.undo_depth();

    let err = batch(
        &mut e,
        json!([
            { "op": "insert", "parent": page, "kind": "rectangle", "name": "ShouldNotLand", "width": 8.0, "height": 8.0 },
            { "op": "set_slot", "node": inst, "slot": "no-such-slot", "fill": hello_fill() }
        ]),
    )
    .expect_err("unknown slot must fail the batch");
    assert_eq!(err["code"], "BATCH_OP_FAILED", "{err}");
    assert_eq!(err["cause"]["code"], "UNKNOWN_SLOT", "{err}");
    assert_eq!(e.doc.to_json().unwrap(), before, "a failed set_slot leaves the batch unchanged");
    assert_eq!(e.history.undo_depth(), depth);

    let frame = e.call("designos_create_node", json!({ "parent_id": page, "kind": "frame", "name": "NotAnInstance", "width": 20.0, "height": 20.0 })).unwrap();
    let frame_id = frame["node"]["id"].as_u64().unwrap();
    let before_frame = e.doc.to_json().unwrap();
    let depth_frame = e.history.undo_depth();
    let kind_err = batch(&mut e, json!([{"op": "set_slot", "node": frame_id, "slot": "body", "fill": hello_fill()}])).expect_err("a frame is not an instance");
    assert_eq!(kind_err["code"], "BATCH_OP_FAILED", "{kind_err}");
    assert!(kind_err["cause"]["message"].as_str().unwrap_or("").contains("not a component instance"), "{kind_err}");
    assert_eq!(e.doc.to_json().unwrap(), before_frame);
    assert_eq!(e.history.undo_depth(), depth_frame);
}

#[test]
fn set_slot_missing_header_frame_errors_and_changes_nothing() {
    let mut e = engine();
    let inst = place_instance(&mut e, "B", None);
    let header = slot_node(&e, inst, "header");
    e.doc.remove(header).expect("remove header frame");
    let before = e.doc.to_json().unwrap();
    let depth = e.history.undo_depth();

    let err = batch(&mut e, json!([{"op": "set_slot", "node": inst, "slot": "header", "fill": [{"kind": "instance", "component": "CardHeader"}]}]))
        .expect_err("a deleted slot frame must fail the batch");

    assert_eq!(err["code"], "BATCH_OP_FAILED", "{err}");
    assert_eq!(err["cause"]["code"], "SLOT_CONTAINER_NOT_FOUND", "{err}");
    assert_eq!(err["cause"]["path"], "/ops/0/slot", "{err}");
    assert_eq!(e.doc.to_json().unwrap(), before, "a missing slot frame leaves the document unchanged");
    assert_eq!(e.history.undo_depth(), depth);
}

#[test]
fn set_slot_body_name_is_case_insensitive() {
    let mut e = engine();
    let inst = place_instance(&mut e, "B", None);
    batch(&mut e, json!([{"op": "set_slot", "node": inst, "slot": "BODY", "fill": hello_fill()}])).expect("set_slot");

    let body = slot_node(&e, inst, "body");
    assert_eq!(child_names(&e, body), vec!["Hello".to_string()]);
    let values = &e.doc.get(NodeId(inst)).unwrap().component.values;
    assert!(values.contains_key("body"), "the instance value key is the slot's own name, got {:?}", values.keys().collect::<Vec<_>>());
    assert!(!values.contains_key("BODY"));
}
