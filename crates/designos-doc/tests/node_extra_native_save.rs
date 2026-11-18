//! `Node::extra` holds the keys an imported REST file carried that the model has no field for.
//! A native save keeps them in one named `extra` object: never merged into the node's own keys, so a
//! key named like a real field cannot shadow it, and a node without extras writes nothing for them.

use designos_doc::*;
use serde_json::json;

fn doc_with_rect() -> (Document, NodeId) {
    let mut doc = Document::new("keys");
    let page = doc.add_page("Page 1");
    let id = doc.alloc_id();
    doc.insert(page, Node::rectangle(id, Rect::new(0.0, 0.0, 40.0, 20.0))).unwrap();
    (doc, id)
}

#[test]
fn extra_survives_a_native_save_and_load() {
    let (mut doc, id) = doc_with_rect();
    doc.get_mut(id).unwrap().extra.insert("futureNodeKey".into(), json!({"n": [1, 2, 3]}));
    let loaded = Document::from_json(&doc.to_json().unwrap()).unwrap();
    assert_eq!(loaded.get(id).unwrap().extra.get("futureNodeKey"), Some(&json!({"n": [1, 2, 3]})));
}

#[test]
fn a_node_without_extras_writes_no_extra_key() {
    let (doc, _) = doc_with_rect();
    let saved = doc.to_json().unwrap();
    assert!(!saved.contains("\"extra\""), "an empty extra must leave the saved bytes as they were: {saved}");
}

#[test]
fn a_key_named_like_a_real_field_cannot_shadow_it() {
    let (mut doc, id) = doc_with_rect();
    let n = doc.get_mut(id).unwrap();
    n.name = "Real name".into();
    n.extra.insert("name".into(), json!("shadow"));
    n.extra.insert("opacity".into(), json!(0.25));
    n.extra.insert("visible".into(), json!(false));
    let loaded = Document::from_json(&doc.to_json().unwrap()).unwrap();
    let n = loaded.get(id).unwrap();
    assert_eq!(n.name, "Real name");
    assert_eq!(n.opacity, 1.0);
    assert!(n.visible);
    assert_eq!(n.extra.len(), 3, "all three keys stay in extra");
    assert_eq!(n.extra.get("opacity"), Some(&json!(0.25)));
}

#[test]
fn a_null_opacity_still_opens_as_fully_opaque() {
    let (doc, id) = doc_with_rect();
    let mut saved: serde_json::Value = serde_json::from_str(&doc.to_json().unwrap()).unwrap();
    saved["nodes"][id.0.to_string()]["opacity"] = serde_json::Value::Null;
    let loaded = Document::from_json(&saved.to_string()).unwrap();
    assert_eq!(loaded.get(id).unwrap().opacity, 1.0);
}

#[test]
fn a_duplicated_subtree_keeps_extra_on_every_copy() {
    let mut doc = Document::new("keys");
    let page = doc.add_page("Page 1");
    let id = doc.alloc_id();
    doc.insert(page, Node::frame(id, Rect::new(0.0, 0.0, 40.0, 20.0))).unwrap();
    let child = doc.alloc_id();
    doc.insert(id, Node::rectangle(child, Rect::new(0.0, 0.0, 4.0, 4.0))).unwrap();
    doc.get_mut(id).unwrap().extra.insert("outer".into(), json!(1));
    doc.get_mut(child).unwrap().extra.insert("inner".into(), json!("x"));
    let copies = duplicate_subtree(&mut doc, id);
    assert_eq!(copies.len(), 2);
    assert_ne!(copies[0].id, id);
    assert_eq!(copies[0].extra.get("outer"), Some(&json!(1)));
    assert_eq!(copies[1].extra.get("inner"), Some(&json!("x")));
    assert_eq!(doc.get(page).unwrap().children.len(), 1, "duplicate_subtree only builds the copies");
}
