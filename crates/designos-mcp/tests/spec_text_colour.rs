//! Text a `NodeSpec` leaves without a colour is black, like text made any other way, and the shipped
//! console spec still draws its light labels because it names their colour.
#![allow(clippy::result_large_err)]

use designos_doc::{Color, NodeId, NodeKind, Paint, PaintKind};
use designos_mcp::engine::Engine;
use serde_json::{json, Value};

fn page(e: &Engine) -> u64 {
    e.resolve_default_page().unwrap().0
}

fn drawn_text_fills(e: &Engine, id: NodeId) -> Vec<Paint> {
    let resolved = designos_doc::resolved_node(&e.doc, id).expect("node");
    match &resolved.kind {
        NodeKind::Text(t) => t.style.fills.clone(),
        other => panic!("not a text node: {other:?}"),
    }
}

fn solid(p: &Paint) -> Color {
    match p.kind {
        PaintKind::Solid(c) => c,
        ref other => panic!("not solid: {other:?}"),
    }
}

#[test]
fn apply_spec_text_without_a_colour_is_black() {
    let mut e = Engine::default();
    let reply = e
        .call(
            "designos_apply_spec",
            json!({"parent_id": page(&e), "spec": {"kind": "frame", "name": "Card", "fill": "#FFFFFF", "children": [
                {"kind": "text", "name": "Heading", "text": "Hello", "font_size": 18},
                {"preset": "easeui-button-primary", "name": "Button", "text": "Go"}
            ]}}),
        )
        .unwrap();

    let heading = NodeId(reply["name_to_id"]["Card/Heading"].as_u64().unwrap());
    assert_eq!(drawn_text_fills(&e, heading), vec![Paint::solid(Color::BLACK)]);
    let button = e.doc.get(NodeId(reply["name_to_id"]["Card/Button"].as_u64().unwrap())).unwrap();
    assert_eq!(drawn_text_fills(&e, button.children[0]), vec![Paint::solid(Color::BLACK)], "the label made from `text`");
}

#[test]
fn the_shipped_agent_console_spec_draws_every_label_light() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/specs/easeui-agent-console.spec.json");
    let spec: Value = serde_json::from_str(&std::fs::read_to_string(path).expect("example spec")).expect("valid json");
    let mut e = Engine::default();
    let reply = e.call("designos_apply_spec", json!({"parent_id": page(&e), "spec": spec})).expect("the example applies");
    let root = NodeId(reply["root_node_id"].as_u64().unwrap());

    let mut texts = Vec::new();
    e.doc.walk(root, |n, _| {
        if let NodeKind::Text(t) = &n.kind {
            texts.push((n.id, t.content.clone()));
        }
    });
    assert!(texts.len() > 25, "the example holds its text nodes: {}", texts.len());
    for (id, content) in &texts {
        let fills = drawn_text_fills(&e, *id);
        assert!(!fills.is_empty(), "{content:?} has a colour");
        assert_ne!(solid(&fills[0]), Color::BLACK, "{content:?} would be black on a dark panel");
    }
    for label in ["designos --comments-json", "Apply Spec to Canvas", "1 \u{b7} SPEC-DRIVEN ENGINE"] {
        let (id, _) = texts.iter().find(|(_, c)| c == label).unwrap_or_else(|| panic!("label {label:?} in the example"));
        assert_eq!(drawn_text_fills(&e, *id), vec![Paint::solid(Color::rgb(0xF4, 0xF4, 0xF5))], "{label:?} keeps its light colour");
    }
}
