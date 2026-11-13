//! A main that holds an instance of itself, through any number of mains, and the agent tools that could make or meet one.

use crate::engine::Engine;
use crate::tools::errors::ErrorCode;
use designos_doc::{
    add_prop, bind_prop, combine_as_variants, create_component, create_instance, instance_component, mains_held_by, sync_instances, Command, ComponentPropDef,
    Document, Node, NodeId, NodeKind, OverrideField, PropKind, PropValue, Rect, Variable, VariableCollection, VariableKind, VariableMode, VariableValue, Vec2,
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
    for (target, loop_text) in [(b, "B \u{2192} A \u{2192} B"), (layer_of(&e.doc, b), "B \u{2192} Layer \u{2192} A \u{2192} B")] {
        let err = e.call("designos_apply_batch", json!({"ops": [{"op": "move", "node": inst.0, "parent": target.0}]})).unwrap_err();
        assert_eq!(err.code, ErrorCode::BatchOpFailed, "{err}");
        assert_eq!(err.cause.as_ref().map(|c| c.code), Some(ErrorCode::MoveCycle), "{err}");
        assert_eq!(err.cause.as_ref().map(|c| c.message.clone()), Some(format!("Can't move: B would contain itself ({loop_text})")), "{err}");
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
fn a_spec_instance_swap_property_that_would_make_a_main_hold_itself_is_refused_with_the_editors_sentence() {
    let (mut e, page, _a, b) = a_holds_b();
    card_with_icon_prop(&mut e, page);
    let before = e.doc.to_json().unwrap();
    let err = e
        .call("designos_apply_spec", json!({"parent_id": b.0, "spec": {"kind": "instance", "component": "Card", "name": "K", "props": {"Icon": "A"}}}))
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::InstanceCycle, "{err}");
    assert_eq!(err.message, "Can't swap: B would contain itself (B \u{2192} Card \u{2192} Z \u{2192} A \u{2192} B)");
    assert_eq!(e.doc.to_json().unwrap(), before, "a refused call changes nothing");
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

/// The sentence the editor shows when the same edit is refused, with the cycle path and no code.
#[test]
fn a_refused_spec_instance_says_what_the_editor_says() {
    let (mut e, _page, _a, b) = a_holds_b();
    let before = e.doc.to_json().unwrap();
    // an instance of A inside B: A holds B, so B would contain itself
    let err = e.call("designos_apply_spec", json!({"parent_id": b.0, "spec": {"kind": "instance", "component": "A"}})).unwrap_err();
    assert_eq!(err.code, ErrorCode::InstanceCycle, "{err}");
    assert_eq!(err.message, "Can't add: B would contain itself (B \u{2192} A \u{2192} B)");
    // the same spec inside a batch
    let err =
        e.call("designos_apply_batch", json!({"ops": [{"op": "apply_spec", "parent_id": b.0, "spec": {"kind": "instance", "component": "A"}}]})).unwrap_err();
    let cause = err.cause.as_ref().expect("batch cause");
    assert_eq!(cause.code, ErrorCode::InstanceCycle, "{err}");
    assert_eq!(cause.message, "Can't add: B would contain itself (B \u{2192} A \u{2192} B)");
    assert_eq!(e.doc.to_json().unwrap(), before, "a refused call changes nothing");
}

#[test]
fn a_refused_spec_swap_says_what_the_editor_says() {
    let mut e = Engine::default();
    let page = e.active_page_id.unwrap();
    let row = main_on(&mut e, page, "Row", 0.0);
    let card = main_on(&mut e, page, "Card", 300.0);
    place(&mut e, row, card); // Card holds an instance of Row
    let dot = main_on(&mut e, page, "Dot", 600.0);
    let chip = main_on(&mut e, page, "Chip", 900.0);
    place(&mut e, dot, chip);
    let before = e.doc.to_json().unwrap();
    let spec = json!({"kind": "instance", "component": "Chip", "overrides": [{"path": "Dot", "swap": "Card"}]});
    // the nested Dot swapped to Card inside Row: Row would hold Card, which holds Row
    let err = e.call("designos_apply_spec", json!({"parent_id": layer_of(&e.doc, row).0, "spec": spec})).unwrap_err();
    assert_eq!(err.code, ErrorCode::InstanceCycle, "{err}");
    assert_eq!(err.message, "Can't swap: Row would contain itself (Row \u{2192} Layer \u{2192} \u{2026} \u{2192} Row)");
    let err = e.call("designos_apply_batch", json!({"ops": [{"op": "apply_spec", "parent_id": layer_of(&e.doc, row).0, "spec": spec}]})).unwrap_err();
    assert_eq!(
        err.cause.as_ref().map(|c| c.message.as_str()),
        Some("Can't swap: Row would contain itself (Row \u{2192} Layer \u{2192} \u{2026} \u{2192} Row)"),
        "{err}"
    );
    assert_eq!(e.doc.to_json().unwrap(), before, "a refused call changes nothing");
}

/// A text variable in a Theme collection that reads the given value in its only mode.
fn text_variable(e: &mut Engine, id: &str, value: &str) {
    let collection = e.doc.variable_collections.entry("theme".into()).or_insert_with(|| VariableCollection {
        id: "theme".into(),
        name: "Theme".into(),
        modes: vec![VariableMode { id: "light".into(), name: "Light".into() }],
        default_mode: "light".into(),
        variables: vec![],
        key: None,
        source_library_key: None,
        extensions: Default::default(),
    });
    collection.variables.push(id.into());
    e.doc.variables.insert(
        id.into(),
        Variable {
            id: id.into(),
            name: id.into(),
            collection: "theme".into(),
            kind: VariableKind::String,
            values: [("light".to_string(), VariableValue::String(value.into()))].into_iter().collect(),
            description: String::new(),
            scopes: vec![],
            hidden_from_publishing: false,
            key: None,
            code_syntax: Default::default(),
            extensions: Default::default(),
            mode_extensions: Default::default(),
        },
    );
}

/// Binding a variant property to a variable that names a variant which would make a main hold itself is refused with
/// the editor's sentence, and nothing changes.
#[test]
fn a_variable_bound_variant_that_would_make_a_main_hold_itself_is_refused_with_the_editors_sentence() {
    let mut e = Engine::default();
    let page = e.active_page_id.unwrap();
    let b = main_on(&mut e, page, "B", 0.0);
    let one = main_on(&mut e, page, "State=One", 300.0);
    let two = main_on(&mut e, page, "State=Two", 600.0);
    combine_as_variants(&mut e.doc, &[one, two]).apply(&mut e.doc).unwrap();
    let i = place(&mut e, one, b);
    place(&mut e, b, two); // State=Two holds B, so the instance of State=One inside B must not become State=Two
    text_variable(&mut e, "state", "Two");
    let before = e.doc.to_json().unwrap();
    let res = e.call("designos_update_node", json!({"node_id": i.0, "prop_vars": {"State": "var:Theme/state"}}));
    let err = res.unwrap_err();
    assert_eq!(err.code, ErrorCode::InstanceCycle, "{err}");
    assert_eq!(err.message, "Can't switch variant: B would contain itself (B \u{2192} Component set \u{2192} State=Two \u{2192} B)");
    assert_eq!(e.doc.to_json().unwrap(), before, "a refused call changes nothing");
    assert_eq!(instance_component(e.doc.get(i).unwrap()), Some(one), "the instance stays on State=One");
}

/// The same variable bound through the spec of a new instance in B.
#[test]
fn a_spec_that_binds_a_variant_to_a_variable_that_would_make_a_main_hold_itself_is_refused_with_the_editors_sentence() {
    let mut e = Engine::default();
    let page = e.active_page_id.unwrap();
    let b = main_on(&mut e, page, "B", 0.0);
    let one = main_on(&mut e, page, "State=One", 300.0);
    let two = main_on(&mut e, page, "State=Two", 600.0);
    combine_as_variants(&mut e.doc, &[one, two]).apply(&mut e.doc).unwrap();
    place(&mut e, b, two);
    text_variable(&mut e, "state", "Two");
    let before = e.doc.to_json().unwrap();
    let spec = json!({"kind": "instance", "component": "Component set", "props": {"State": "var:Theme/state"}});
    let res = e.call("designos_apply_spec", json!({"parent_id": b.0, "spec": spec}));
    let err = res.unwrap_err();
    assert_eq!(err.code, ErrorCode::InstanceCycle, "{err}");
    assert_eq!(err.message, "Can't switch variant: B would contain itself (B \u{2192} Component set \u{2192} State=Two \u{2192} B)");
    assert_eq!(e.doc.to_json().unwrap(), before, "a refused call changes nothing");
}

/// A set with two variant properties (Size S or L, State One or Two), an instance of Size=S, State=One inside main B,
/// and the variant `held` (a Size and State pair) holding B. Returns (engine, B, the instance, the variants by name).
fn size_state_set(held: (&str, &str)) -> (Engine, NodeId, NodeId, std::collections::BTreeMap<String, NodeId>) {
    let mut e = Engine::default();
    let page = e.active_page_id.unwrap();
    let b = main_on(&mut e, page, "B", 0.0);
    let mut mains = Vec::new();
    for (x, (size, state)) in [("S", "One"), ("S", "Two"), ("L", "One"), ("L", "Two")].into_iter().enumerate() {
        mains.push((format!("Size={size}, State={state}"), main_on(&mut e, page, &format!("Size={size}, State={state}"), 300.0 + 300.0 * x as f32)));
    }
    let ids: Vec<NodeId> = mains.iter().map(|(_, id)| *id).collect();
    combine_as_variants(&mut e.doc, &ids).apply(&mut e.doc).unwrap();
    let variants: std::collections::BTreeMap<String, NodeId> = mains.into_iter().collect();
    let i = place(&mut e, variants["Size=S, State=One"], b);
    place(&mut e, b, variants[&format!("Size={}, State={}", held.0, held.1)]);
    text_variable(&mut e, "size", "L");
    text_variable(&mut e, "state", "Two");
    (e, b, i, variants)
}

/// What the instance shows, by name.
fn shown(e: &Engine, instance: NodeId) -> String {
    e.doc.get(instance_component(e.doc.get(instance).unwrap()).unwrap()).unwrap().name.clone()
}

/// Binding both variant properties in one call selects one variant, Size=L, State=Two. That variant holds nothing, so
/// the call is legal, even though Size=S, State=Two (the State change alone) holds B.
#[test]
fn binding_two_variant_properties_whose_combined_variant_is_legal_is_applied() {
    let (mut e, _b, i, _variants) = size_state_set(("S", "Two"));
    let both = json!({"Size": "var:Theme/size", "State": "var:Theme/state"});
    e.call("designos_update_node", json!({"node_id": i.0, "prop_vars": both})).unwrap();
    assert_eq!(shown(&e, i), "Size=L, State=Two");

    let (mut e, b, _i, _variants) = size_state_set(("S", "Two"));
    let spec = json!({"kind": "instance", "component": "Component set", "props": both});
    let reply = e.call("designos_apply_spec", json!({"parent_id": b.0, "spec": spec})).unwrap();
    assert_eq!(shown(&e, NodeId(reply["root_node_id"].as_u64().unwrap())), "Size=L, State=Two");
}

/// The combined variant holds B, although each property alone selects a legal one: the call is refused with the
/// sentence that names the variant that the call asked for, and nothing changes.
#[test]
fn binding_two_variant_properties_whose_combined_variant_would_make_a_main_hold_itself_is_refused() {
    let (mut e, b, i, _variants) = size_state_set(("L", "Two"));
    let before = e.doc.to_json().unwrap();
    let both = json!({"Size": "var:Theme/size", "State": "var:Theme/state"});
    let sentence = "Can't switch variant: B would contain itself (B \u{2192} Component set \u{2192} Size=L, State=Two \u{2192} B)";
    let err = e.call("designos_update_node", json!({"node_id": i.0, "prop_vars": both})).unwrap_err();
    assert_eq!(err.code, ErrorCode::InstanceCycle, "{err}");
    assert_eq!(err.message, sentence);
    assert_eq!(e.doc.to_json().unwrap(), before, "a refused call changes nothing");
    assert_eq!(shown(&e, i), "Size=S, State=One");

    let spec = json!({"kind": "instance", "component": "Component set", "props": both});
    let err = e.call("designos_apply_spec", json!({"parent_id": b.0, "spec": spec})).unwrap_err();
    assert_eq!(err.code, ErrorCode::InstanceCycle, "{err}");
    assert_eq!(err.message, sentence);
    assert_eq!(e.doc.to_json().unwrap(), before, "a refused call changes nothing");
}
