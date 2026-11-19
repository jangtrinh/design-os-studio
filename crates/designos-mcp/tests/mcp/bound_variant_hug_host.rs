//! A `set_variables` batch that switches the variant of a bound instance solves the hugging frame around it in the same
//! step: the reply, the document and one undo all agree, and the trial the batch runs first matches what is committed.
#![allow(clippy::result_large_err)]

use designos_doc::*;
use designos_mcp::engine::Engine;
use serde_json::{json, Value};

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

/// A `Size` variant set (Small 100 wide, Large 200 wide) and a hugging row `host` with 4 px padding that holds `inst`, an
/// instance of the set with `Size` bound to `size`; beside it a second hugging row `quiet` that holds a fixed frame.
struct Setup {
    e: Engine,
    host: NodeId,
    inst: NodeId,
    small: NodeId,
    large: NodeId,
    quiet: NodeId,
}

fn hugging_row(d: &mut Document, parent: NodeId, x: f32) -> NodeId {
    let id = d.alloc_id();
    let mut f = Node::frame(id, Rect::new(x, 0.0, 10.0, 10.0));
    if let NodeKind::Frame(fp) = &mut f.kind {
        fp.auto_layout = Some(AutoLayout {
            direction: LayoutDirection::Horizontal,
            primary_sizing: Sizing::Hug,
            counter_sizing: Sizing::Hug,
            padding: [4.0; 4],
            layout_version: 1,
            ..AutoLayout::default()
        });
    }
    d.insert(parent, f).unwrap()
}

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
    let host = hugging_row(d, page, 500.0);
    create_instance(d, mains[0], host, Vec2::new(0.0, 0.0)).apply(d).unwrap();
    let inst = *d.get(host).unwrap().children.last().unwrap();
    bind_prop_variable(d, inst, "Size", Some("size")).unwrap().apply(d).unwrap();
    let quiet = hugging_row(d, page, 900.0);
    let id = d.alloc_id();
    d.insert(quiet, Node::frame(id, Rect::new(0.0, 0.0, 50.0, 30.0))).unwrap();
    d.relayout_subtree(host);
    d.relayout_subtree(quiet);
    Setup { e, host, inst, small: mains[0], large: mains[1], quiet }
}

fn shown(d: &Document, inst: NodeId) -> NodeId {
    instance_component(d.get(inst).unwrap()).unwrap()
}

fn width(d: &Document, id: NodeId) -> f32 {
    d.get(id).unwrap().size.x
}

fn reply_width(e: &mut Engine, id: NodeId) -> f64 {
    let got: Value = e.call("designos_get_node", json!({"node_id": id.0})).unwrap();
    got["node"]["size"]["x"].as_f64().unwrap_or_else(|| panic!("no size in {got}"))
}

fn set_light_value(e: &mut Engine, value: &str) {
    e.call(
        "designos_apply_batch",
        json!({"ops": [{"op": "set_variables", "upsert_variables": [{"id": "size", "collection": "theme", "kind": "String", "values": {"light": value}}]}]}),
    )
    .unwrap();
}

#[test]
fn a_set_variables_batch_that_switches_a_bound_instance_resizes_the_hug_host_in_the_reply_and_the_document() {
    let mut s = setup();
    assert_eq!((shown(&s.e.doc, s.inst), width(&s.e.doc, s.host)), (s.small, 108.0), "precondition");
    let quiet_before = (width(&s.e.doc, s.quiet), s.e.doc.get(s.quiet).unwrap().transform.tx);
    let depth = s.e.history.undo_depth();

    // The committed document must equal the trial the batch ran first: the batch asserts it in a debug build.
    set_light_value(&mut s.e, "Large");

    assert_eq!(shown(&s.e.doc, s.inst), s.large);
    assert_eq!(width(&s.e.doc, s.host), 208.0, "the document: the host hugs the switched instance");
    assert_eq!(reply_width(&mut s.e, s.host), 208.0, "the tool reply shows the same");
    assert_eq!(s.e.history.undo_depth(), depth + 1, "one undo step");
    assert_eq!((width(&s.e.doc, s.quiet), s.e.doc.get(s.quiet).unwrap().transform.tx), quiet_before, "the frame beside it is not touched");

    s.e.call("designos_undo", json!({})).unwrap();
    assert_eq!(shown(&s.e.doc, s.inst), s.small);
    assert_eq!(width(&s.e.doc, s.host), 108.0, "one undo gives the host its size back");
    assert_eq!(reply_width(&mut s.e, s.host), 108.0, "and the tool reply agrees");

    s.e.call("designos_redo", json!({})).unwrap();
    assert_eq!(shown(&s.e.doc, s.inst), s.large);
    assert_eq!(width(&s.e.doc, s.host), 208.0, "one redo brings the hugged size back");
}
