//! What a `.fig` save writes as the id an override path uses for a node (`overrideKey`) and as each step of an override
//! path (`guidPath`), read from the saved file.

mod common;

use common::fig_fixture::*;
use designos_doc::*;
use designos_fig::{export_fig, import_fig_node_changes};
use serde_json::{json, Value};

fn component(doc: &mut Document, parent: NodeId, name: &str) -> NodeId {
    let id = doc.alloc_id();
    let mut n = Node::new(id, NodeKind::Component(FrameProps::default()));
    n.name = name.into();
    n.size = Vec2::new(100.0, 50.0);
    doc.insert(parent, n).expect("insert component")
}

fn rectangle(doc: &mut Document, parent: NodeId, name: &str) -> NodeId {
    let id = doc.alloc_id();
    let mut n = Node::rectangle(id, Rect::new(0.0, 0.0, 10.0, 10.0));
    n.name = name.into();
    doc.insert(parent, n).expect("insert rectangle")
}

fn instance_in(doc: &mut Document, main: NodeId, parent: NodeId, name: &str) -> NodeId {
    create_instance(doc, main, parent, Vec2::new(0.0, 0.0)).apply(doc).expect("create instance");
    let id = *doc.get(parent).expect("parent").children.last().expect("the new instance");
    doc.get_mut(id).expect("instance").name = name.into();
    id
}

fn saved(doc: &Document) -> Vec<Value> {
    node_changes(&export_fig(doc).expect("export"))
}

fn written<'a>(changes: &'a [Value], name: &str) -> &'a Value {
    changes.iter().find(|nc| nc["name"] == name).unwrap_or_else(|| panic!("no node {name} in the saved file"))
}

#[test]
fn instances_inside_a_component_get_their_own_override_key_and_an_instance_on_a_page_gets_none() {
    let mut doc = Document::new("override keys");
    let page = doc.add_page("Page");
    let leaf = component(&mut doc, page, "Leaf");
    rectangle(&mut doc, leaf, "Dot");
    let holder = component(&mut doc, page, "Holder");
    instance_in(&mut doc, leaf, holder, "First");
    instance_in(&mut doc, leaf, holder, "Second");
    instance_in(&mut doc, leaf, page, "OnPage");

    let changes = saved(&doc);
    let (first, second, on_page) = (written(&changes, "First"), written(&changes, "Second"), written(&changes, "OnPage"));
    assert_eq!(first["overrideKey"], first["guid"], "an instance inside a component is keyed by its own guid");
    assert_eq!(second["overrideKey"], second["guid"]);
    assert_ne!(first["overrideKey"], second["overrideKey"], "two instances of one component inside another do not share a key");
    assert!(on_page.get("overrideKey").is_none(), "an instance on a page has no key");
}

#[test]
fn a_node_that_names_itself_as_its_source_gets_no_override_key_on_a_page() {
    let mut doc = Document::new("self source");
    let page = doc.add_page("Page");
    let node = rectangle(&mut doc, page, "Loop");
    doc.get_mut(node).expect("node").component.source = Some(node);

    let changes = saved(&doc);
    assert!(written(&changes, "Loop").get("overrideKey").is_none());
}

#[test]
fn an_override_key_follows_the_guid_that_the_source_id_of_the_authored_node_names() {
    let mut doc = Document::new("source ids");
    let page = doc.add_page("Page");
    let first = component(&mut doc, page, "First");
    let authored = rectangle(&mut doc, first, "Authored");
    doc.get_mut(authored).expect("node").source_id = Some("I9:9;77:7777".into());
    let second = component(&mut doc, page, "Second");
    let copy = rectangle(&mut doc, second, "Copy");
    doc.get_mut(copy).expect("node").component.source = Some(authored);

    let changes = saved(&doc);
    assert_eq!(written(&changes, "Copy")["overrideKey"], guid(77, 7777), "the key is the guid that ends the authored node's source id");
}

fn note_fixture() -> Value {
    let mut nodes = document_and_page();
    nodes.extend([
        fig_node((10, 1), (0, 1), "SYMBOL", "Note", json!({})),
        fig_node(
            (10, 2),
            (10, 1),
            "TEXT",
            "Body",
            json!({"fontSize": 14.0, "fontName": {"family": "Inter", "style": "Regular", "postscript": ""}, "textData": {"characters": "Hello world"}}),
        ),
        fig_node((20, 1), (0, 1), "INSTANCE", "NoteInst", json!({"symbolData": {"symbolID": guid(10, 1), "symbolOverrides": []}})),
    ]);
    Value::Array(nodes)
}

#[test]
fn an_override_path_step_follows_the_guid_that_the_source_id_names_when_the_node_has_no_source() {
    let mut doc = import_fig_node_changes(&note_fixture(), "Path steps").expect("import");
    let inst = by_name(&doc, "NoteInst");
    let body = kid(&doc, inst, "Body").id;
    {
        let n = doc.get_mut(body).expect("node");
        n.component.source = None;
        n.source_id = Some("I9:9;77:7777".into());
        n.component.overrides.insert(OverrideField::Characters);
        let NodeKind::Text(tp) = &mut n.kind else { panic!("Body is a text layer") };
        tp.content = "Changed".into();
    }

    let changes = saved(&doc);
    let entries = written(&changes, "NoteInst")["symbolData"]["symbolOverrides"].as_array().cloned().unwrap_or_default();
    let entry = entries.iter().find(|e| e["textData"]["characters"] == "Changed").expect("the text override is written");
    assert_eq!(entry["guidPath"]["guids"], json!([guid(77, 7777)]), "the step is the guid that ends the node's source id");
}
