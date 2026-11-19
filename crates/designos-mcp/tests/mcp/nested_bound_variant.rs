//! A nested instance whose variant property is bound to a variable follows the variable through the agent tools:
//! `designos_apply_batch` with `set_variables`, and `designos_apply_spec` placing an instance in a frame with a mode.
#![allow(clippy::result_large_err)]

use designos_doc::*;
use designos_mcp::engine::Engine;
use serde_json::json;

struct Setup {
    e: Engine,
    page: NodeId,
    wrapper: NodeId,
    host: NodeId,
    w: NodeId,
    small: NodeId,
    large: NodeId,
}

fn size_variable() -> Variable {
    Variable {
        id: "size".into(),
        name: "size".into(),
        collection: "theme".into(),
        kind: VariableKind::String,
        values: [("light".to_string(), VariableValue::String("Small".into())), ("dark".to_string(), VariableValue::String("Large".into()))]
            .into_iter()
            .collect(),
        description: String::new(),
        scopes: vec![],
        hidden_from_publishing: false,
        key: None,
        code_syntax: Default::default(),
        extensions: Default::default(),
        mode_extensions: Default::default(),
    }
}

/// A `Size` variant set, a wrapper main holding one instance of it with `Size` bound to `size`, and an instance `w` of the
/// wrapper in `host`.
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
            variables: vec!["size".into()],
            key: None,
            source_library_key: None,
            extensions: Default::default(),
        },
    );
    d.variables.insert("size".into(), size_variable());
    let mut mains = Vec::new();
    for (name, wd) in [("Size=Small", 100.0), ("Size=Large", 200.0)] {
        let id = d.alloc_id();
        let mut v = Node::frame(id, Rect::new(0.0, 0.0, wd, 40.0));
        v.name = name.into();
        let v = d.insert(page, v).unwrap();
        create_component(d, v).apply(d).unwrap();
        mains.push(v);
    }
    combine_as_variants(d, &mains).apply(d).unwrap();
    let id = d.alloc_id();
    let wrapper = d.insert(page, Node::frame(id, Rect::new(0.0, 300.0, 400.0, 100.0))).unwrap();
    create_instance(d, mains[0], wrapper, Vec2::new(10.0, 10.0)).apply(d).unwrap();
    let nested = *d.get(wrapper).unwrap().children.last().unwrap();
    create_component(d, wrapper).apply(d).unwrap();
    let bind = bind_prop_variable(d, nested, "Size", Some("size")).unwrap();
    bind.apply(d).unwrap();
    let id = d.alloc_id();
    let host = d.insert(page, Node::frame(id, Rect::new(500.0, 0.0, 400.0, 200.0))).unwrap();
    create_instance(d, wrapper, host, Vec2::new(10.0, 10.0)).apply(d).unwrap();
    let w = *d.get(host).unwrap().children.last().unwrap();
    Setup { e, page, wrapper, host, w, small: mains[0], large: mains[1] }
}

fn shown(d: &Document, inst: NodeId) -> NodeId {
    instance_component(d.get(inst).unwrap()).unwrap()
}

fn copy_of(d: &Document, w: NodeId) -> NodeId {
    *d.get(w).unwrap().children.first().unwrap()
}

#[test]
fn a_set_variables_batch_that_changes_the_value_in_the_active_mode_flips_the_copy_and_one_undo_restores_it() {
    let mut s = setup();
    assert_eq!(shown(&s.e.doc, copy_of(&s.e.doc, s.w)), s.small);
    let depth = s.e.history.undo_depth();
    s.e.call(
        "designos_apply_batch",
        json!({"ops": [{"op": "set_variables", "upsert_variables": [{"id": "size", "collection": "theme", "kind": "String", "values": {"light": "Large"}}]}]}),
    )
    .unwrap();
    assert_eq!(shown(&s.e.doc, copy_of(&s.e.doc, s.w)), s.large, "the copy shows the variant the variable gives now");
    assert_eq!(s.e.history.undo_depth(), depth + 1, "one undo step");
    s.e.call("designos_undo", json!({})).unwrap();
    assert_eq!(shown(&s.e.doc, copy_of(&s.e.doc, s.w)), s.small, "one undo restores the variable and the variant");
}

/// Only a top-level bound instance: no wrapper, no nested copy.
fn plain_top_level_setup() -> (Engine, NodeId, NodeId, NodeId) {
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
            variables: vec!["size".into()],
            key: None,
            source_library_key: None,
            extensions: Default::default(),
        },
    );
    d.variables.insert("size".into(), size_variable());
    let mut mains = Vec::new();
    for (name, wd) in [("Size=Small", 100.0), ("Size=Large", 200.0)] {
        let id = d.alloc_id();
        let mut v = Node::frame(id, Rect::new(0.0, 0.0, wd, 40.0));
        v.name = name.into();
        let v = d.insert(page, v).unwrap();
        create_component(d, v).apply(d).unwrap();
        mains.push(v);
    }
    combine_as_variants(d, &mains).apply(d).unwrap();
    let id = d.alloc_id();
    let host = d.insert(page, Node::frame(id, Rect::new(500.0, 0.0, 400.0, 200.0))).unwrap();
    create_instance(d, mains[0], host, Vec2::new(10.0, 10.0)).apply(d).unwrap();
    let inst = *d.get(host).unwrap().children.last().unwrap();
    bind_prop_variable(d, inst, "Size", Some("size")).unwrap().apply(d).unwrap();
    (e, inst, mains[0], mains[1])
}

#[test]
fn a_set_variables_batch_that_moves_a_plain_top_level_bound_instance_commits_what_the_trial_showed() {
    let (mut e, inst, small, large) = plain_top_level_setup();
    assert_eq!(shown(&e.doc, inst), small);
    e.call(
        "designos_apply_batch",
        json!({"ops": [{"op": "set_variables", "upsert_variables": [{"id": "size", "collection": "theme", "kind": "String", "values": {"light": "Large"}}]}]}),
    )
    .unwrap();
    assert_eq!(shown(&e.doc, inst), large, "the committed instance shows the variant the new value gives");
    e.call("designos_undo", json!({})).unwrap();
    assert_eq!(shown(&e.doc, inst), small, "one undo restores the variable and the variant");
}

#[test]
fn update_node_binds_the_variant_of_a_nested_copy_and_a_sync_of_the_wrapper_main_leaves_it() {
    let mut s = setup();
    let s2 = |v: &str| VariableValue::String(v.into());
    let mut inverse = size_variable();
    inverse.id = "size2".into();
    inverse.name = "size2".into();
    inverse.values = [("light".to_string(), s2("Large")), ("dark".to_string(), s2("Small"))].into_iter().collect();
    s.e.doc.variables.insert("size2".into(), inverse);
    s.e.doc.variable_collections.get_mut("theme").unwrap().variables.push("size2".into());
    let copy = copy_of(&s.e.doc, s.w);
    assert_eq!(shown(&s.e.doc, copy), s.small, "the main's binding gives Small in light");
    s.e.call("designos_update_node", json!({"node_id": copy.0, "prop_vars": {"Size": "var:Theme/size2"}})).unwrap();
    assert_eq!(shown(&s.e.doc, copy), s.large, "the copy's own variable gives Large in light");
    sync_instances(&mut s.e.doc, s.wrapper).apply(&mut s.e.doc).unwrap();
    assert_eq!(s.e.doc.get(copy).unwrap().variables.bindings.get(&prop_binding_key("Size")).map(String::as_str), Some("size2"));
    assert_eq!(shown(&s.e.doc, copy), s.large, "a sync of the wrapper main leaves the copy's own binding");
}

fn set_host_mode(s: &mut Setup, mode: &str) {
    let mut host = s.e.doc.get(s.host).unwrap().clone();
    host.variables.modes.insert("theme".into(), mode.into());
    s.e.exec(Command::Update { node: Box::new(host) }).unwrap();
}

#[test]
fn update_node_unbinding_the_binding_a_copy_inherits_holds_the_copy_until_reset() {
    let mut s = setup();
    let copy = copy_of(&s.e.doc, s.w);
    set_host_mode(&mut s, "dark");
    assert_eq!(shown(&s.e.doc, copy), s.large, "the dark host gives Large");
    let reply = s.e.call("designos_update_node", json!({"node_id": copy.0, "prop_vars": {"Size": null}})).unwrap();
    assert_eq!(reply["changed"], true, "{reply}");
    assert!(!s.e.doc.get(copy).unwrap().variables.bindings.contains_key(&prop_binding_key("Size")), "the copy holds no binding");
    set_host_mode(&mut s, "light");
    assert_eq!(shown(&s.e.doc, copy), s.large, "a mode change no longer moves the copy");
    let reset = reset_instance(&mut s.e.doc, s.w);
    s.e.exec(reset).unwrap();
    assert_eq!(
        s.e.doc.get(copy).unwrap().variables.bindings.get(&prop_binding_key("Size")).map(String::as_str),
        Some("size"),
        "reset gives the main's binding back"
    );
    assert_eq!(shown(&s.e.doc, copy), s.small, "light gives Small");
    set_host_mode(&mut s, "dark");
    assert_eq!(shown(&s.e.doc, copy), s.large, "and it follows the mode again");
}

#[test]
fn a_batch_move_of_the_wrapper_instance_into_a_dark_frame_flips_the_copy_and_one_undo_restores_it() {
    let mut s = setup();
    let applied =
        s.e.call("designos_apply_spec", json!({"parent_id": s.page.0, "spec": {"kind": "frame", "name": "Dark frame", "mode": {"Theme": "Dark"}}})).unwrap();
    let dark = NodeId(applied["root_node_id"].as_u64().unwrap());
    let depth = s.e.history.undo_depth();
    s.e.call("designos_apply_batch", json!({"ops": [{"op": "move", "node": s.w.0, "parent": dark.0}]})).unwrap();
    assert_eq!(s.e.doc.get(s.w).unwrap().parent, Some(dark));
    assert_eq!(shown(&s.e.doc, copy_of(&s.e.doc, s.w)), s.large, "moved into the dark frame: the copy shows Large");
    assert_eq!(s.e.history.undo_depth(), depth + 1);
    s.e.call("designos_undo", json!({})).unwrap();
    assert_eq!(s.e.doc.get(s.w).unwrap().parent, Some(s.host));
    assert_eq!(shown(&s.e.doc, copy_of(&s.e.doc, s.w)), s.small, "one undo moves it back and restores the variant");
}

#[test]
fn an_instance_placed_through_apply_spec_in_a_dark_frame_shows_the_variant_that_mode_gives() {
    let mut s = setup();
    let applied =
        s.e.call(
            "designos_apply_spec",
            json!({"parent_id": s.page.0, "spec": {"kind": "frame", "name": "Dark host", "mode": {"Theme": "Dark"},
                "children": [{"kind": "instance", "component": format!("node:{}", s.wrapper.0)}]}}),
        )
        .unwrap();
    let frame = NodeId(applied["root_node_id"].as_u64().unwrap());
    let placed = *s.e.doc.get(frame).unwrap().children.first().unwrap();
    assert_eq!(shown(&s.e.doc, copy_of(&s.e.doc, placed)), s.large, "the copy resolves the dark frame's mode");
    assert_eq!(shown(&s.e.doc, copy_of(&s.e.doc, s.w)), s.small, "the instance in the light host is untouched");
    let _ = s.host;
}
