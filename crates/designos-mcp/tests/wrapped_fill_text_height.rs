//! MCP end-to-end for #856: QA's card built through apply_spec / apply_batch must store
//! Body 51 / Frame 104 (and the rest of QA's width table) on get_node.
#![allow(clippy::result_large_err)]

use designos_mcp::engine::Engine;
use serde_json::{json, Value};

const BODY: &str = "The quick brown fox jumps over the lazy dog and keeps running across the field until nightfall.";

fn card_spec(width: f32) -> Value {
    json!({
        "kind": "frame",
        "name": "Card",
        "width": width,
        "height": 40,
        "width_sizing": "fixed",
        "height_sizing": "hug",
        "layout": {"direction": "column", "padding": [10, 10, 10, 10], "gap": 8},
        "children": [
            {"kind": "text", "name": "Title", "text": "Title", "font_size": 20, "font_family": "Inter"},
            {"kind": "text", "name": "Body", "text": BODY, "font_size": 14, "font_family": "Inter", "width_sizing": "fill"}
        ]
    })
}

fn page(e: &Engine) -> u64 {
    e.resolve_default_page().unwrap().0
}

fn node_size(reply: &Value) -> (f32, f32) {
    let size = &reply["node"]["size"];
    (size["x"].as_f64().unwrap() as f32, size["y"].as_f64().unwrap() as f32)
}

fn get_size(e: &mut Engine, id: u64) -> (f32, f32) {
    node_size(&e.call("designos_get_node", json!({"node_id": id})).expect("get_node"))
}

fn assert_near(got: f32, want: f32, label: &str) {
    assert!((got - want).abs() < 0.51, "{label}: got {got} want {want}");
}

fn assert_card(e: &mut Engine, card: u64, body: u64, width: f32, body_h: f32, frame_h: f32) {
    let (bw, bh) = get_size(e, body);
    let (fw, fh) = get_size(e, card);
    assert_near(bw, (width - 20.0).max(1.0), &format!("W={width} body width"));
    assert_near(bh, body_h, &format!("W={width} body height"));
    assert_near(fw, width, &format!("W={width} frame width"));
    assert_near(fh, frame_h, &format!("W={width} frame height"));
}

#[test]
fn apply_batch_qa_card_stores_body_51_frame_104() {
    let mut e = Engine::new();
    let page = page(&e);
    let r = e.call("designos_apply_batch", json!({"ops": [{"op": "apply_spec", "as": "f", "parent": page, "spec": card_spec(300.0)}]})).expect("apply_batch");
    let card = r["name_to_id"]["f"].as_u64().unwrap();
    let body = r["name_to_id"]["f/Body"].as_u64().or_else(|| r["name_to_id"]["Body"].as_u64()).expect("Body id");
    assert_card(&mut e, card, body, 300.0, 51.0, 104.0);
}

#[test]
fn apply_spec_then_set_width_matches_qa_table() {
    let mut e = Engine::new();
    let page = page(&e);
    let r = e.call("designos_apply_spec", json!({"parent_id": page, "spec": card_spec(300.0)})).expect("apply_spec");
    let card = r["root_node_id"].as_u64().unwrap();
    let body = r["name_to_id"]["Card/Body"].as_u64().or_else(|| r["name_to_id"]["Body"].as_u64()).expect("Body id");
    assert_card(&mut e, card, body, 300.0, 51.0, 104.0);
    for (width, body_h, frame_h) in [(140.0, 102.0, 155.0), (420.0, 34.0, 87.0), (60.0, 322.0, 375.0), (300.0, 51.0, 104.0)] {
        e.call("designos_update_node", json!({"node_id": card, "width": width})).expect("set width");
        assert_card(&mut e, card, body, width, body_h, frame_h);
    }
}
