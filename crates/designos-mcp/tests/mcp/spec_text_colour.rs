//! Text a `NodeSpec` leaves without a colour takes the colour that contrasts best with the solid fill
//! behind it, and the shipped console spec still draws its light labels because it names their colour.
#![allow(clippy::result_large_err)]

use designos_doc::lint::rules::contrast::contrast_ratio;
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

/// The colour behind a text node: the first visible paint that is solid at half opacity or more on its ancestors (a hidden
/// ancestor still counts, since it is shown later), else the page background.
fn backdrop_of(e: &Engine, text: NodeId) -> Color {
    let mut cur = e.doc.get(text).and_then(|n| n.parent);
    while let Some(id) = cur {
        let n = e.doc.get(id).expect("ancestor");
        if let NodeKind::Page { background } = &n.kind {
            return *background;
        }
        for p in n.fills.iter().rev() {
            if let (true, PaintKind::Solid(c)) = (p.visible, &p.kind) {
                if p.opacity * c.a >= 0.5 {
                    return *c;
                }
            }
        }
        cur = n.parent;
    }
    Color::WHITE
}

fn text_nodes_under(e: &Engine, root: NodeId) -> Vec<NodeId> {
    let mut out = Vec::new();
    e.doc.walk(root, |n, _| {
        if matches!(n.kind, NodeKind::Text(_)) {
            out.push(n.id);
        }
    });
    out
}

#[test]
fn apply_spec_text_without_a_colour_is_black_on_white_and_light_on_a_dark_card() {
    let mut e = Engine::default();
    let white = e
        .call(
            "designos_apply_spec",
            json!({"parent_id": page(&e), "spec": {"kind": "frame", "name": "Light", "fill": "#FFFFFF", "children": [
                {"kind": "text", "name": "Heading", "text": "Hello", "font_size": 18}
            ]}}),
        )
        .unwrap();
    let heading = NodeId(white["name_to_id"]["Light/Heading"].as_u64().unwrap());
    assert_eq!(drawn_text_fills(&e, heading), vec![Paint::solid(Color::BLACK)]);

    let dark = e
        .call(
            "designos_apply_spec",
            json!({"parent_id": page(&e), "spec": {"kind": "frame", "name": "Card", "preset": "easeui-card", "children": [
                {"kind": "text", "name": "Heading", "text": "Hello"},
                {"kind": "frame", "name": "Clear", "fill": "transparent", "children": [{"kind": "text", "name": "Nested", "text": "Nested"}]},
                {"preset": "easeui-button-secondary", "name": "Button", "text": "Cancel"}
            ]}}),
        )
        .unwrap();
    let light = Paint::solid(Color::rgb(0xF4, 0xF4, 0xF5));
    for path in ["Card/Heading", "Card/Clear/Nested"] {
        let id = NodeId(dark["name_to_id"][path].as_u64().unwrap());
        assert_eq!(drawn_text_fills(&e, id), vec![light.clone()], "{path}");
    }
    let button = e.doc.get(NodeId(dark["name_to_id"]["Card/Button"].as_u64().unwrap())).unwrap();
    assert_eq!(drawn_text_fills(&e, button.children[0]), vec![light], "the label made from `text` on a dark preset");
}

#[test]
fn text_in_a_frame_hidden_before_or_after_is_black_on_white_and_light_on_dark() {
    let mut e = Engine::default();
    let spec = e
        .call(
            "designos_apply_spec",
            json!({"parent_id": page(&e), "spec": {"kind": "frame", "name": "Menu", "fill": "#FFFFFF", "visible": false, "children": [
                {"kind": "text", "name": "Item", "text": "Item"}
            ]}}),
        )
        .unwrap();
    let item = NodeId(spec["name_to_id"]["Menu/Item"].as_u64().unwrap());
    assert_eq!(drawn_text_fills(&e, item), vec![Paint::solid(Color::BLACK)], "a frame built hidden");

    let frame = e.call("designos_create_node", json!({"parent_id": page(&e), "kind": "frame", "name": "Popover", "fill": "#FFFFFF"})).unwrap();
    let frame_id = frame["node"]["id"].as_u64().unwrap();
    e.call("designos_update_node", json!({"node_id": frame_id, "visible": false})).unwrap();
    let text = e.call("designos_apply_spec", json!({"parent_id": frame_id, "spec": {"kind": "text", "name": "Note", "text": "Note"}})).unwrap();
    let note = NodeId(text["root_node_id"].as_u64().unwrap());
    assert_eq!(drawn_text_fills(&e, note), vec![Paint::solid(Color::BLACK)], "text added to a frame that was hidden afterwards");
}

#[test]
fn an_explicit_colour_a_variable_or_a_style_beats_the_contrast_pick_through_apply_spec() {
    let mut e = Engine::default();
    e.call(
        "designos_apply_batch",
        json!({"ops": [
            {"op": "set_variables",
             "upsert_collections": [{"id": "brand", "name": "Brand", "modes": [{"id": "light", "name": "Light"}], "default_mode": "light"}],
             "upsert_variables": [{"id": "brand/ink", "name": "Ink", "collection": "brand", "kind": "color", "values": {"light": "#FF8800"}}]},
            {"op": "set_style", "upsert": [{"key": "accent", "name": "Accent", "property": "fill", "fill": "#00AAFF"}]}
        ]}),
    )
    .unwrap();
    let reply = e
        .call(
            "designos_apply_spec",
            json!({"parent_id": page(&e), "spec": {"kind": "frame", "name": "Card", "preset": "easeui-card", "children": [
                {"kind": "text", "name": "Red", "text": "Red", "text_color": "#FF0000"},
                {"kind": "text", "name": "Bound", "text": "Bound", "text_color": "var:Brand/Ink"},
                {"kind": "text", "name": "Styled", "text": "Styled", "text_color": "style:Accent"}
            ]}}),
        )
        .unwrap();
    let id = |name: &str| NodeId(reply["name_to_id"][format!("Card/{name}")].as_u64().unwrap());
    assert_eq!(drawn_text_fills(&e, id("Red")), vec![Paint::solid(Color::rgb(255, 0, 0))]);
    assert_eq!(drawn_text_fills(&e, id("Bound")), vec![Paint::solid(Color::rgb(0xFF, 0x88, 0x00))]);
    assert_eq!(e.doc.get(id("Bound")).unwrap().variables.bindings.get("fills/0").map(String::as_str), Some("brand/ink"));
    assert_eq!(drawn_text_fills(&e, id("Styled")), vec![Paint::solid(Color::rgb(0x00, 0xAA, 0xFF))]);
    assert!(e.doc.get(id("Styled")).unwrap().style_refs.contains_key("fill"));
}

/// The `designos_apply_spec` worked example in the rules pack: an `easeui-card` with text and no colour anywhere.
fn rules_pack_apply_spec_example() -> Value {
    let pack = designos_mcp::rules::generate_default_rules_pack().expect("rules pack");
    let line = pack.lines().find(|l| l.contains(r#""name":"Metric Card""#) && l.contains(r#""spec""#)).expect("the apply_spec example in the rules pack");
    let json = line.split('`').find(|part| part.starts_with('{')).expect("the example is in backticks");
    serde_json::from_str(json).expect("the example is valid JSON")
}

#[test]
fn the_rules_pack_example_draws_every_text_node_readable_on_the_fill_behind_it() {
    let example = rules_pack_apply_spec_example();
    let mut e = Engine::default();
    let reply = e.call("designos_apply_spec", json!({"parent_id": page(&e), "spec": example["spec"]})).expect("the example applies");
    let root = NodeId(reply["root_node_id"].as_u64().unwrap());

    let texts = text_nodes_under(&e, root);
    assert_eq!(texts.len(), 2, "Label and Value");
    for id in texts {
        let fills = drawn_text_fills(&e, id);
        let ratio = contrast_ratio(solid(&fills[0]), backdrop_of(&e, id));
        assert!(ratio >= 4.5, "{:?} contrast {ratio:.2}:1", e.doc.get(id).unwrap().name);
        assert_eq!(fills, vec![Paint::solid(Color::rgb(0xF4, 0xF4, 0xF5))], "text on the dark card is the light colour");
    }
}

#[test]
fn the_shipped_agent_console_spec_draws_every_label_light() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../designos-app/assets/specs/easeui-agent-console.spec.json");
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
