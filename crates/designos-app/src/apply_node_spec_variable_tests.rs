//! `App::apply_node_spec` (Cmd+K, spec files, terminal spec JSON) commits the variables a spec
//! creates together with its nodes, in one undo step.

use crate::app::App;
use designos_doc::{create_component, create_instance, Command, Document, Node, NodeSpec, Rect, Vec2};

fn app_with_empty_page() -> App {
    let mut doc = Document::new("Spec apply");
    doc.add_page("Page 1");
    App::new(doc)
}

fn cycle_spec_app() -> App {
    let mut doc = Document::new("Spec cycle toast");
    let page = doc.add_page("Page 1");
    let b = doc.alloc_id();
    let b = doc.insert(page, Node::frame(b, Rect::new(0.0, 0.0, 100.0, 100.0))).unwrap();
    doc.get_mut(b).unwrap().name = "B".into();
    let b_layer = doc.alloc_id();
    doc.insert(b, Node::frame(b_layer, Rect::new(0.0, 0.0, 40.0, 40.0))).unwrap();
    create_component(&mut doc, b).apply(&mut doc).unwrap();
    let a = doc.alloc_id();
    let a = doc.insert(page, Node::frame(a, Rect::new(120.0, 0.0, 100.0, 100.0))).unwrap();
    doc.get_mut(a).unwrap().name = "A".into();
    let a_layer = doc.alloc_id();
    doc.insert(a, Node::frame(a_layer, Rect::new(0.0, 0.0, 40.0, 40.0))).unwrap();
    create_component(&mut doc, a).apply(&mut doc).unwrap();
    let Command::Insert { nodes, .. } = create_instance(&mut doc, b, a, Vec2::new(0.0, 0.0)) else { panic!("fixture instance") };
    Command::Insert { parent: a, index: 1, nodes }.apply(&mut doc).unwrap();
    let mut app = App::new(doc);
    app.page = b;
    app
}

#[test]
fn spec_cycle_refusal_toast_has_one_user_facing_sentence() {
    let mut app = cycle_spec_app();
    let spec: NodeSpec = serde_json::from_value(serde_json::json!({ "kind": "instance", "component": "A" })).unwrap();
    app.apply_node_spec(&spec, "test");
    let toast = app.toast.as_ref().expect("cycle refusal toast");
    assert_eq!(toast.text, "Can't add: B would contain itself (B → A → B)");
}

#[test]
fn easeui_spec_brings_its_variable_collection_and_one_undo_removes_both() {
    let mut app = app_with_empty_page();
    let spec: NodeSpec = serde_json::from_value(serde_json::json!({ "name": "Panel", "kind": "frame", "fill": "$easeui.bg.panel" })).unwrap();

    app.apply_node_spec(&spec, "test");

    let root = app.selection[0];
    let binding = app.doc.get(root).unwrap().variables.bindings.get("fills/0").cloned().expect("the fill is bound");
    assert!(app.doc.variables.contains_key(&binding), "the document lacks the variable {binding} the fill is bound to");
    assert!(app.doc.variable_collections.contains_key("easeui-deprecated"));

    assert!(app.hist.undo(&mut app.doc).expect("undo"));
    assert!(app.doc.get(root).is_none(), "the nodes are gone");
    assert!(app.doc.variable_collections.is_empty() && app.doc.variables.is_empty(), "the same undo step removes the variables");
    assert!(!app.hist.undo(&mut app.doc).expect("nothing left to undo"));
}
