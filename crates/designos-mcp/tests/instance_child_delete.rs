//! A layer mirrored inside a component instance cannot be deleted one by one, by `designos_delete_node` or by
//! the `delete` op of `designos_apply_batch`. The refusal is the non-retryable `INSTANCE_CHILD` and changes nothing.

use designos_doc::{NodeId, NodeKind};
use designos_mcp::engine::Engine;
use serde_json::{json, Value};

fn engine() -> Engine {
    Engine { doc: designos_doc::fixtures::card_with_slots(), ..Engine::default() }
}

fn page_id(e: &Engine) -> u64 {
    e.doc.pages[0].0
}

fn place_instance(e: &mut Engine, slots: Option<Value>) -> u64 {
    let mut spec = json!({"kind": "instance", "name": "CardInst", "component": "Card"});
    if let Some(slots) = slots {
        spec["slots"] = slots;
    }
    e.call("designos_apply_spec", json!({"parent_id": page_id(e), "spec": spec})).expect("instance")["root_node_id"].as_u64().unwrap()
}

fn slot_node(e: &Engine, instance: u64, name: &str) -> NodeId {
    designos_doc::resolve_override_path(&e.doc, NodeId(instance), name).expect("slot")
}

fn main_card(e: &Engine) -> NodeId {
    e.doc.nodes.values().find(|n| n.name == "Card" && matches!(n.kind, NodeKind::Component(_))).expect("Card component").id
}

fn snapshot(e: &Engine) -> String {
    e.doc.to_json().expect("document json")
}

fn free_rectangle(e: &mut Engine) -> u64 {
    let page = page_id(e);
    e.call("designos_create_node", json!({"parent_id": page, "kind": "rectangle", "name": "Free", "x": 900.0, "y": 0.0, "width": 10.0, "height": 10.0}))
        .expect("free rectangle")["node"]["id"]
        .as_u64()
        .expect("node id")
}

/// An instance with its `body` slot frame, which is a mirrored layer that is neither the instance root nor slot content.
fn instance_with_mirrored_layer() -> (Engine, u64, u64) {
    let mut e = engine();
    let inst = place_instance(&mut e, None);
    let mirrored = slot_node(&e, inst, "body").0;
    (e, inst, mirrored)
}

fn assert_instance_child(err: &Value) {
    assert_eq!(err["code"], "INSTANCE_CHILD", "{err}");
    assert_eq!(err["safe_to_retry"], false, "{err}");
    assert_eq!(err["error_kind"], "fatal", "{err}");
    let next = err["next_step"].as_str().unwrap_or("");
    assert!(next.contains("Hide the layer") && next.contains("override"), "the next step says what to do instead: {err}");
}

#[test]
fn delete_node_refuses_a_mirrored_layer_with_a_non_retryable_instance_child_error() {
    let (mut e, _, mirrored) = instance_with_mirrored_layer();
    let before = snapshot(&e);
    let depth_before = e.history.undo_depth();

    let err = e.call("designos_delete_node", json!({"node_id": mirrored})).expect_err("refused").to_json_value();

    assert_instance_child(&err);
    assert_eq!(err["path"], "/node_id");
    assert_eq!(snapshot(&e), before);
    assert_eq!(e.history.undo_depth(), depth_before, "a refused delete creates no history");
}

#[test]
fn batch_delete_refuses_a_mirrored_layer_with_a_non_retryable_instance_child_error() {
    let (mut e, _, mirrored) = instance_with_mirrored_layer();
    let before = snapshot(&e);
    let depth_before = e.history.undo_depth();

    let err = e.call("designos_apply_batch", json!({"ops": [{"op": "delete", "node": mirrored}]})).expect_err("refused").to_json_value();

    assert_eq!(err["code"], "BATCH_OP_FAILED", "{err}");
    assert_eq!(err["safe_to_retry"], false, "the envelope takes the cause's kind: {err}");
    assert_instance_child(&err["cause"]);
    assert_eq!(err["cause"]["path"], "/ops/0/node");
    assert_eq!(snapshot(&e), before);
    assert_eq!(e.history.undo_depth(), depth_before);
}

#[test]
fn a_batch_that_deletes_a_free_layer_and_then_a_mirrored_layer_changes_nothing() {
    let (mut e, _, mirrored) = instance_with_mirrored_layer();
    let free = free_rectangle(&mut e);
    let before = snapshot(&e);
    let depth_before = e.history.undo_depth();

    let err = e
        .call("designos_apply_batch", json!({"ops": [{"op": "delete", "node": free}, {"op": "delete", "node": mirrored}]}))
        .expect_err("the second delete is refused, so the whole batch is")
        .to_json_value();

    assert_instance_child(&err["cause"]);
    assert_eq!(err["cause"]["path"], "/ops/1/node");
    assert_eq!(snapshot(&e), before, "the free layer is still there");
    assert_eq!(e.history.undo_depth(), depth_before);
}

#[test]
fn a_dry_run_reports_the_same_refusal_and_changes_nothing() {
    let (mut e, _, mirrored) = instance_with_mirrored_layer();
    let before = snapshot(&e);

    let err = e
        .call("designos_apply_batch", json!({"dry_run": true, "ops": [{"op": "delete", "node": mirrored}]}))
        .expect_err("a dry run refuses it too")
        .to_json_value();

    assert_eq!(err["code"], "BATCH_OP_FAILED", "{err}");
    assert_instance_child(&err["cause"]);
    assert_eq!(snapshot(&e), before);
}

#[test]
fn slot_content_an_instance_root_and_a_main_components_own_layer_stay_deletable() {
    let fill = json!({"body": [{"kind": "text", "name": "Hello", "text": "Hello"}]});

    for through_batch in [false, true] {
        let delete = |e: &mut Engine, id: u64| {
            if through_batch {
                e.call("designos_apply_batch", json!({"ops": [{"op": "delete", "node": id}]}))
            } else {
                e.call("designos_delete_node", json!({"node_id": id}))
            }
        };

        // content placed in an instance slot
        let mut e = engine();
        let inst = place_instance(&mut e, Some(fill.clone()));
        let slot = slot_node(&e, inst, "body");
        let content = *e.doc.get(slot).unwrap().children.last().expect("slot content");
        delete(&mut e, content.0).unwrap_or_else(|err| panic!("slot content (through_batch = {through_batch}): {err}"));
        assert!(e.doc.get(content).is_none());

        // the instance root
        let mut e = engine();
        let inst = place_instance(&mut e, None);
        delete(&mut e, inst).unwrap_or_else(|err| panic!("instance root (through_batch = {through_batch}): {err}"));
        assert!(e.doc.get(NodeId(inst)).is_none());

        // a layer of the main component itself
        let mut e = engine();
        let main_layer = slot_node(&e, main_card(&e).0, "body");
        delete(&mut e, main_layer.0).unwrap_or_else(|err| panic!("main component layer (through_batch = {through_batch}): {err}"));
        assert!(e.doc.get(main_layer).is_none());
    }
}
