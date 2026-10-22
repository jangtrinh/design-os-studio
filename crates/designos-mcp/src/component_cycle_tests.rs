//! A main that holds an instance of itself, through any number of mains, and the agent tools that could make or meet one.

use crate::engine::Engine;
use crate::tools::errors::ErrorCode;
use designos_doc::{
    add_prop, bind_prop, create_component, create_instance, instance_component, mains_held_by, sync_instances, Command, ComponentPropDef, Document, Node,
    NodeId, NodeKind, OverrideField, PropKind, PropValue, Rect, Vec2,
};
use serde_json::json;

/// A main on `parent` with one layer inside it.
fn main_on(e: &mut Engine, parent: NodeId, name: &str, x: f32) -> NodeId {
    let id = e.doc.alloc_id();
    let main = e.doc.insert(parent, Node::frame(id, Rect::new(x, 0.0, 100.0, 100.0))).unwrap();
    e.doc.get_mut(main).unwrap().name = name.into();
    let id = e.doc.alloc_id();
    let layer = e.doc.insert(main, Node::frame(id, Rect::new(5.0, 5.0, 20.0, 20.0))).unwrap();
    e.doc.get_mut(layer).unwrap().name = "Layer".into();
    create_component(&mut e.doc, main).apply(&mut e.doc).unwrap();
    main
}

fn place(e: &mut Engine, comp: NodeId, parent: NodeId) -> NodeId {
    let cmd = create_instance(&mut e.doc, comp, parent, Vec2::new(5.0, 5.0));
    assert!(matches!(cmd, Command::Insert { .. }), "instance of {comp:?} in {parent:?} was refused");
    cmd.apply(&mut e.doc).unwrap();
    *e.doc.get(parent).unwrap().children.last().unwrap()
}

fn layer_of(d: &Document, main: NodeId) -> NodeId {
    *d.get(main).unwrap().children.iter().find(|c| d.get(**c).unwrap().name == "Layer").unwrap()
}

fn self_holding(d: &Document) -> Vec<NodeId> {
    d.nodes.values().filter(|n| matches!(n.kind, NodeKind::Component(_))).map(|n| n.id).filter(|m| mains_held_by(d, &[*m]).contains(m)).collect()
}

/// Mains B and A, A holding an instance of B, on the page. Returns (engine, page, A, B).
fn a_holds_b() -> (Engine, NodeId, NodeId, NodeId) {
    let mut e = Engine::default();
    let page = e.active_page_id.unwrap();
    let b = main_on(&mut e, page, "B", 0.0);
    let a = main_on(&mut e, page, "A", 300.0);
    place(&mut e, b, a);
    (e, page, a, b)
}

/// The same pair, but B holds an instance of A too, made without any guard (an older build, or a hand-edited file).
fn cyclic() -> (Engine, NodeId, NodeId) {
    let (mut e, page, a, b) = a_holds_b();
    let inst = place(&mut e, a, page);
    Command::Reparent { id: inst, parent: b, index: 0 }.apply(&mut e.doc).unwrap();
    assert_eq!(self_holding(&e.doc).len(), 2);
    (e, a, b)
}

#[test]
fn a_batch_move_into_a_main_the_layer_holds_an_instance_of_is_refused() {
    let (mut e, page, a, b) = a_holds_b();
    let inst = place(&mut e, a, page); // an instance of A, which holds B
    let before = e.doc.to_json().unwrap();
    for target in [b, layer_of(&e.doc, b)] {
        let err = e.call("designos_apply_batch", json!({"ops": [{"op": "move", "node": inst.0, "parent": target.0}]})).unwrap_err();
        assert_eq!(err.code, ErrorCode::BatchOpFailed, "{err}");
        assert_eq!(err.cause.as_ref().map(|c| c.code), Some(ErrorCode::MoveCycle), "{err}");
    }
    // main A itself holds B too
    let err = e.call("designos_apply_batch", json!({"ops": [{"op": "move", "node": a.0, "parent": b.0}]})).unwrap_err();
    assert_eq!(err.cause.as_ref().map(|c| c.code), Some(ErrorCode::MoveCycle), "{err}");
    assert_eq!(e.doc.to_json().unwrap(), before, "a refused batch changes nothing");
    assert!(self_holding(&e.doc).is_empty());
}

#[test]
fn a_batch_move_that_holds_nothing_still_moves() {
    let (mut e, page, a, b) = a_holds_b();
    let inst = place(&mut e, a, page);
    let other = main_on(&mut e, page, "Other", 700.0);
    // an instance of A into an unrelated main, into the page again, and an instance of B into A (which holds B)
    e.call("designos_apply_batch", json!({"ops": [{"op": "move", "node": inst.0, "parent": other.0}]})).unwrap();
    assert_eq!(e.doc.get(inst).unwrap().parent, Some(other));
    e.call("designos_apply_batch", json!({"ops": [{"op": "move", "node": inst.0, "parent": page.0}]})).unwrap();
    let in_a = *e.doc.get(a).unwrap().children.last().unwrap();
    assert_eq!(instance_component(e.doc.get(in_a).unwrap()), Some(b));
    e.call("designos_apply_batch", json!({"ops": [{"op": "move", "node": in_a.0, "parent": a.0, "index": 0}]})).unwrap();
    assert!(self_holding(&e.doc).is_empty());
}

/// Card holds an instance of Z bound to the instance-swap property "Icon".
fn card_with_icon_prop(e: &mut Engine, page: NodeId) -> NodeId {
    let z = main_on(e, page, "Z", 600.0);
    let card = main_on(e, page, "Card", 900.0);
    let slot = place(e, z, card);
    let def = ComponentPropDef { name: "Icon".into(), kind: PropKind::InstanceSwap { preferred: vec![] }, default: PropValue::Instance(z) };
    add_prop(&mut e.doc, card, def).apply(&mut e.doc).unwrap();
    bind_prop(&mut e.doc, slot, OverrideField::MainComponent, Some("Icon")).apply(&mut e.doc).unwrap();
    card
}

#[test]
fn a_spec_instance_swap_property_that_would_make_a_main_hold_itself_makes_no_cycle() {
    let (mut e, page, _a, b) = a_holds_b();
    card_with_icon_prop(&mut e, page);
    let res = e.call("designos_apply_spec", json!({"parent_id": b.0, "spec": {"kind": "instance", "component": "Card", "name": "K", "props": {"Icon": "A"}}}));
    println!("apply_spec Card props Icon=A in B (A holds B): {:?}", res.as_ref().map(|_| "Ok").map_err(|e| e.to_string()));
    assert!(self_holding(&e.doc).is_empty(), "no main holds itself after the call");
    sync_instances(&mut e.doc, b).apply(&mut e.doc).unwrap();
    // the same props on the page, where nothing holds itself, still swap the icon
    let res =
        e.call("designos_apply_spec", json!({"parent_id": page.0, "spec": {"kind": "instance", "component": "Card", "name": "K2", "props": {"Icon": "A"}}}));
    assert!(res.is_ok(), "{res:?}");
    assert!(self_holding(&e.doc).is_empty());
}

#[test]
fn the_agent_tools_read_and_edit_a_document_that_already_holds_itself() {
    let (mut e, a, b) = cyclic();
    for node in [a, b, layer_of(&e.doc, a), layer_of(&e.doc, b)] {
        let got = e.call("designos_get_node", json!({"node_id": node.0})).unwrap();
        assert_eq!(got["node"]["id"], node.0);
    }
    let layer = layer_of(&e.doc, b);
    e.call("designos_update_node", json!({"node_id": layer.0, "opacity": 0.5, "name": "Edited"})).unwrap();
    e.call("designos_apply_batch", json!({"ops": [{"op": "update", "node": layer.0, "opacity": 0.25}]})).unwrap();
    assert_eq!(e.doc.get(layer).unwrap().opacity, 0.25);
    // later syncs of both mains, as Studio runs after each live edit, and a save and reopen
    for round in 0..24 {
        sync_instances(&mut e.doc, if round % 2 == 0 { b } else { a }).apply(&mut e.doc).unwrap();
    }
    let back = Document::from_json(&e.doc.to_json().unwrap()).unwrap();
    assert_eq!(back.nodes.len(), e.doc.nodes.len());
    e.call("designos_list_nodes", json!({"limit": 100})).unwrap();
}
