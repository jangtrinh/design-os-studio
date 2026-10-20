//! `App::apply_node_spec` (Cmd+K, spec files, terminal spec JSON) commits the variables a spec
//! creates together with its nodes, in one undo step.

use crate::app::App;
use designos_doc::{Document, NodeSpec};

fn app_with_empty_page() -> App {
    let mut doc = Document::new("Spec apply");
    doc.add_page("Page 1");
    App::new(doc)
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
