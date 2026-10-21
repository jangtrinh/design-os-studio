//! Text colour sent through `fill` / `fills` on create, update and batch lands where every renderer
//! reads it: the text style (`style.fills`), not the node's own `fills`.
#![allow(clippy::result_large_err)]

use designos_doc::{Color, NodeId, NodeKind, Paint, PaintKind, TextRun, TextStyle};
use designos_mcp::engine::Engine;
use serde_json::{json, Value};

const RED: Color = Color { r: 1.0, g: 0.0, b: 0.0, a: 1.0 };

fn red_paint() -> Paint {
    Paint::solid(RED)
}

fn page_of(e: &Engine) -> u64 {
    e.resolve_default_page().unwrap().0
}

fn text_fills(e: &Engine, id: NodeId) -> Vec<Paint> {
    match &e.doc.get(id).expect("node").kind {
        NodeKind::Text(t) => t.style.fills.clone(),
        other => panic!("not a text node: {other:?}"),
    }
}

fn node_fills(e: &Engine, id: NodeId) -> Vec<Paint> {
    e.doc.get(id).expect("node").fills.clone()
}

fn create_text(e: &mut Engine, extra: Value) -> NodeId {
    let mut args = json!({"parent_id": page_of(e), "kind": "text", "text": "Hello red", "width": 200, "height": 40});
    args.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
    NodeId(e.call("designos_create_node", args).unwrap()["node"]["id"].as_u64().unwrap())
}

/// Count of opaque red-dominant pixels in the node's own render: black text yields none.
fn red_pixels(e: &Engine, id: NodeId) -> usize {
    let pm = designos_render::render_node(&e.doc, id, 2.0).expect("render");
    pm.pixels().iter().filter(|p| p.alpha() > 200 && p.red() > 200 && p.green() < 40 && p.blue() < 40).count()
}

fn is_solid_red(p: &Paint) -> bool {
    matches!(p.kind, PaintKind::Solid(c) if c == RED)
}

#[test]
fn create_text_with_fill_colours_the_text_style() {
    let mut e = Engine::default();
    let id = create_text(&mut e, json!({"fill": "#FF0000"}));

    let fills = text_fills(&e, id);
    assert!(fills.len() == 1 && is_solid_red(&fills[0]), "text style carries the red fill: {fills:?}");
    assert!(node_fills(&e, id).is_empty(), "a text node's own fills stay empty");
    assert!(red_pixels(&e, id) > 20, "the rendered text is red");
}

#[test]
fn create_text_with_fills_array_colours_the_text_style() {
    let mut e = Engine::default();
    let id = create_text(&mut e, json!({"fills": [red_paint()]}));

    let fills = text_fills(&e, id);
    assert!(fills.len() == 1 && is_solid_red(&fills[0]), "{fills:?}");
    assert!(node_fills(&e, id).is_empty());
}

#[test]
fn create_reply_serializes_the_red_text_style() {
    let mut e = Engine::default();
    let reply =
        e.call("designos_create_node", json!({"parent_id": page_of(&e), "kind": "text", "text": "Hi", "width": 120, "height": 30, "fill": "#FF0000"})).unwrap();
    let style_fills = &reply["node"]["kind"]["Text"]["style"]["fills"];
    assert_eq!(style_fills.as_array().map(Vec::len), Some(1), "reply: {reply}");
    assert_eq!(style_fills[0]["kind"]["Solid"]["r"], 1.0, "the serialized style fill is red: {reply}");
    assert_eq!(style_fills[0]["kind"]["Solid"]["g"], 0.0, "{reply}");
    assert!(reply["node"]["fills"].as_array().is_none_or(Vec::is_empty), "the node's own fills stay empty: {reply}");
}

#[test]
fn update_text_fill_recolours_the_style_and_its_runs() {
    let mut e = Engine::default();
    let id = create_text(&mut e, json!({}));
    if let Some(NodeKind::Text(t)) = e.doc.get_mut(id).map(|n| &mut n.kind) {
        t.runs.push(TextRun { start: 0, end: 5, style: TextStyle::default() });
    }

    e.call("designos_update_node", json!({"node_id": id.0, "fill": "#FF0000"})).unwrap();

    let fills = text_fills(&e, id);
    assert!(fills.len() == 1 && is_solid_red(&fills[0]), "{fills:?}");
    let NodeKind::Text(t) = &e.doc.get(id).unwrap().kind else { unreachable!() };
    assert!(t.runs.iter().all(|r| r.style.fills.len() == 1 && is_solid_red(&r.style.fills[0])), "every run takes the colour: {:?}", t.runs);
    assert!(node_fills(&e, id).is_empty());
    assert!(red_pixels(&e, id) > 20, "the rendered text is red");
}

#[test]
fn update_fills_array_on_text_colours_the_text_style() {
    let mut e = Engine::default();
    let id = create_text(&mut e, json!({}));

    e.call("designos_update_node", json!({"node_id": id.0, "fills": [red_paint()]})).unwrap();

    let fills = text_fills(&e, id);
    assert!(fills.len() == 1 && is_solid_red(&fills[0]), "{fills:?}");
    assert!(node_fills(&e, id).is_empty());
}

#[test]
fn batch_insert_and_update_colour_the_text_style() {
    let mut e = Engine::default();
    let page = page_of(&e);
    let reply = e
        .call(
            "designos_apply_batch",
            json!({"ops": [
                {"op": "insert", "as": "a", "parent": page, "kind": "text", "text": "One", "width": 100, "height": 30, "fill": "#FF0000"},
                {"op": "insert", "as": "b", "parent": page, "kind": "text", "text": "Two", "width": 100, "height": 30},
                {"op": "update", "node": "@b", "fill": "#FF0000"}
            ]}),
        )
        .unwrap();
    let ids: Vec<NodeId> = reply["created"].as_array().expect("created ids").iter().map(|v| NodeId(v.as_u64().unwrap())).collect();
    assert_eq!(ids.len(), 2, "{reply}");

    for id in ids {
        let fills = text_fills(&e, id);
        assert!(fills.len() == 1 && is_solid_red(&fills[0]), "node {id:?}: {fills:?}");
        assert!(node_fills(&e, id).is_empty(), "node {id:?}");
        assert!(red_pixels(&e, id) > 20, "node {id:?} renders red");
    }
}

#[test]
fn non_text_nodes_keep_writing_their_own_fills() {
    let mut e = Engine::default();
    let reply = e.call("designos_create_node", json!({"parent_id": page_of(&e), "kind": "rectangle", "width": 20, "height": 20, "fill": "#FF0000"})).unwrap();
    let id = NodeId(reply["node"]["id"].as_u64().unwrap());
    let fills = node_fills(&e, id);
    assert!(fills.len() == 1 && is_solid_red(&fills[0]), "{fills:?}");
}
