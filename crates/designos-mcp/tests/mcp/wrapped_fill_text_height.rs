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

#[test]
fn apply_spec_hug_text_with_max_width_wraps_and_without_max_width_stays_single_line() {
    let mut e = Engine::new();
    let page = page(&e);
    let long_copy = "A very long paragraph of body copy that should wrap once it hits the two hundred pixel maximum width limit instead of growing forever on a single line.";

    let wrapped = e
        .call(
            "designos_apply_spec",
            json!({
                "parent_id": page,
                "spec": {
                    "kind": "frame",
                    "name": "Wrap",
                    "width_sizing": "hug",
                    "height_sizing": "hug",
                    "layout": {"direction": "column"},
                    "children": [
                        {
                            "kind": "text",
                            "name": "Limited",
                            "text": long_copy,
                            "font_size": 14,
                            "width_sizing": "hug",
                            "max_width": 200
                        }
                    ]
                }
            }),
        )
        .expect("apply_spec with max_width");
    let wrap_frame = wrapped["root_node_id"].as_u64().unwrap();
    let limited_text = wrapped["name_to_id"]["Wrap/Limited"].as_u64().or_else(|| wrapped["name_to_id"]["Limited"].as_u64()).expect("Limited text id");

    let (tw, th) = get_size(&mut e, limited_text);
    let (fw, fh) = get_size(&mut e, wrap_frame);
    assert_near(tw, 200.0, "hug text with max_width width");
    assert!(th >= 85.0, "hug text with max_width must wrap and grow in height, got {th}");
    assert_near(fw, 200.0, "enclosing hug frame width");
    assert_near(fh, th, "enclosing hug frame height matches wrapped text height");

    let unclamped = e
        .call(
            "designos_apply_spec",
            json!({
                "parent_id": page,
                "spec": {
                    "kind": "frame",
                    "name": "UnclampedWrap",
                    "width_sizing": "hug",
                    "height_sizing": "hug",
                    "layout": {"direction": "column"},
                    "children": [
                        {
                            "kind": "text",
                            "name": "Unclamped",
                            "text": long_copy,
                            "font_size": 14,
                            "width_sizing": "hug"
                        }
                    ]
                }
            }),
        )
        .expect("apply_spec without max_width");
    let unclamped_text =
        unclamped["name_to_id"]["UnclampedWrap/Unclamped"].as_u64().or_else(|| unclamped["name_to_id"]["Unclamped"].as_u64()).expect("Unclamped text id");
    let (uw, uh) = get_size(&mut e, unclamped_text);
    assert!(uw > 600.0, "hug text without max_width stays single-line wide, got {uw}");
    assert_near(uh, 17.0, "hug text without max_width stays single-line height");
}

#[test]
fn hug_text_with_max_width_grows_when_the_text_gets_longer() {
    let mut e = Engine::new();
    let page = page(&e);
    let long_copy = "The agent read 14 layers, renamed the card titles, and moved the badge into the header row so the layout matches the kit. It did not change any colours.";

    let created = e
        .call(
            "designos_apply_spec",
            json!({
                "parent_id": page,
                "spec": {
                    "kind": "frame",
                    "name": "ShortThenLong",
                    "width_sizing": "hug",
                    "height_sizing": "hug",
                    "layout": {"direction": "column", "padding": [16, 16, 16, 16]},
                    "children": [
                        {
                            "kind": "text",
                            "name": "Label",
                            "text": "Hi",
                            "font_size": 14,
                            "width_sizing": "hug",
                            "max_width": 200
                        }
                    ]
                }
            }),
        )
        .expect("apply_spec short text with max_width");
    let frame_id = created["root_node_id"].as_u64().unwrap();
    let text_id = created["name_to_id"]["ShortThenLong/Label"].as_u64().or_else(|| created["name_to_id"]["Label"].as_u64()).expect("Label text id");

    let (w0, h0) = get_size(&mut e, text_id);
    assert!(w0 < 40.0, "short text starts at its natural width, got {w0}");
    assert_near(h0, 17.0, "short text starts on a single line");

    e.call("designos_update_node", json!({"node_id": text_id, "text": long_copy})).expect("update_node to long text");

    let (w1, h1) = get_size(&mut e, text_id);
    let (fw1, fh1) = get_size(&mut e, frame_id);
    assert_near(w1, 200.0, "long text widens to max_width");
    assert!((85.0..300.0).contains(&h1), "long text wraps at max_width rather than a narrow column, got {h1}");
    assert_near(fw1, 232.0, "enclosing hug frame widens with text + padding");
    assert_near(fh1, h1 + 32.0, "enclosing hug frame grows in height with wrapped text + padding");
    let updated_node = e.call("designos_get_node", json!({"node_id": text_id})).expect("get_node")["node"].clone();
    assert_eq!(updated_node["kind"]["Text"]["auto_resize"], "WidthAndHeight");
}

#[test]
fn hug_text_with_max_width_shrinks_when_the_text_gets_shorter() {
    let mut e = Engine::new();
    let page = page(&e);
    let long_copy = "The agent read 14 layers, renamed the card titles, and moved the badge into the header row so the layout matches the kit. It did not change any colours.";

    let created = e
        .call(
            "designos_apply_spec",
            json!({
                "parent_id": page,
                "spec": {
                    "kind": "frame",
                    "name": "LongThenShort",
                    "width_sizing": "hug",
                    "height_sizing": "hug",
                    "layout": {"direction": "column", "padding": [16, 16, 16, 16]},
                    "children": [
                        {
                            "kind": "text",
                            "name": "Label",
                            "text": long_copy,
                            "font_size": 14,
                            "width_sizing": "hug",
                            "max_width": 200
                        }
                    ]
                }
            }),
        )
        .expect("apply_spec long text with max_width");
    let frame_id = created["root_node_id"].as_u64().unwrap();
    let text_id = created["name_to_id"]["LongThenShort/Label"].as_u64().or_else(|| created["name_to_id"]["Label"].as_u64()).expect("Label text id");

    let (w0, h0) = get_size(&mut e, text_id);
    assert_near(w0, 200.0, "long text starts clamped at max_width");
    assert!(h0 >= 85.0, "long text starts wrapped across multiple lines, got {h0}");

    e.call("designos_update_node", json!({"node_id": text_id, "text": "Hi"})).expect("update_node to short text");

    let (w1, h1) = get_size(&mut e, text_id);
    let (fw1, fh1) = get_size(&mut e, frame_id);
    assert!(w1 < 40.0, "shortened text hugs back down to its natural width, got {w1}");
    assert_near(h1, 17.0, "shortened text returns to a single line");
    assert_near(fw1, w1 + 32.0, "enclosing hug frame shrinks to natural width + padding");
    assert_near(fh1, 49.0, "enclosing hug frame shrinks to single-line height + padding");
    let updated_node = e.call("designos_get_node", json!({"node_id": text_id})).expect("get_node")["node"].clone();
    assert_eq!(updated_node["kind"]["Text"]["auto_resize"], "WidthAndHeight");
}
