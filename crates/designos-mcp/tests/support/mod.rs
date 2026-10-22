//! Shared fixtures for the colour-write tests: a palette of variables and readers for what a node
//! stores and what the renderer draws.
#![allow(dead_code, clippy::result_large_err)]

use designos_doc::{Color, Node, NodeId, NodeKind, Paint, PaintKind};
use designos_mcp::engine::Engine;
use designos_mcp::tools::errors::ToolError;
use serde_json::{json, Value};

pub const RED: &str = "#FF0000";
pub const PRIMARY: &str = "#2563EB";
pub const DANGER: &str = "#DC2626";

pub fn colour(hex: &str) -> Color {
    Color::from_hex(hex).expect("valid hex")
}

pub fn solid_hex(p: &Paint) -> String {
    match p.kind {
        PaintKind::Solid(c) => format!("#{}", c.to_hex()).to_ascii_uppercase(),
        ref other => panic!("not a solid paint: {other:?}"),
    }
}

/// The batch that defines the palette: a light/dark `Theme` collection, a light-only `Color`
/// collection and one number variable.
pub fn palette_ops() -> Value {
    json!({"ops": [{
        "op": "set_variables",
        "upsert_collections": [
            {"id": "color", "name": "Color", "modes": [{"id": "light", "name": "Light"}], "default_mode": "light"},
            {"id": "theme", "name": "Theme", "modes": [{"id": "light", "name": "Light"}, {"id": "dark", "name": "Dark"}], "default_mode": "light"}
        ],
        "upsert_variables": [
            {"id": "color/primary", "name": "Primary", "collection": "color", "kind": "color", "values": {"light": PRIMARY}},
            {"id": "color/danger", "name": "Danger", "collection": "color", "kind": "color", "values": {"light": DANGER}},
            {"id": "color/gap", "name": "Gap", "collection": "color", "kind": "number", "values": {"light": 8}},
            {"id": "theme/surface", "name": "Surface", "collection": "theme", "kind": "color", "values": {"light": "#FFFFFF", "dark": "#18181B"}}
        ]
    }]})
}

pub fn palette() -> Engine {
    let mut e = Engine::default();
    e.call("designos_apply_batch", palette_ops()).expect("palette variables");
    e
}

pub fn page(e: &Engine) -> u64 {
    e.resolve_default_page().unwrap().0
}

pub fn apply(e: &mut Engine, spec: Value) -> NodeId {
    let page = page(e);
    let reply = e.call("designos_apply_spec", json!({"parent_id": page, "spec": spec})).expect("apply_spec");
    NodeId(reply["root_node_id"].as_u64().unwrap())
}

pub fn bound_frame(e: &mut Engine) -> NodeId {
    apply(e, json!({"kind": "frame", "name": "Button", "width": 120, "height": 40, "fill": "var:Color/Primary"}))
}

pub fn bound_text(e: &mut Engine) -> NodeId {
    apply(e, json!({"kind": "text", "name": "Label", "text": "Verify", "width": 160, "height": 40, "font_size": 28, "text_color": "var:Color/Primary"}))
}

pub fn node(e: &Engine, id: NodeId) -> &Node {
    e.doc.get(id).expect("node")
}

pub fn raw_paints(e: &Engine, id: NodeId) -> Vec<Paint> {
    match &node(e, id).kind {
        NodeKind::Text(t) => t.style.fills.clone(),
        _ => node(e, id).fills.clone(),
    }
}

/// The paints the renderer sees: the node's own with every variable binding substituted.
pub fn drawn_paints(e: &Engine, id: NodeId) -> Vec<Paint> {
    let resolved = designos_doc::resolved_node(&e.doc, id).expect("node");
    match &resolved.kind {
        NodeKind::Text(t) => t.style.fills.clone(),
        _ => resolved.fills.clone(),
    }
}

pub fn fill_bindings(e: &Engine, id: NodeId) -> Vec<(String, String)> {
    node(e, id).variables.bindings.iter().filter(|(k, _)| k.starts_with("fills/")).map(|(k, v)| (k.clone(), v.clone())).collect()
}

pub fn centre_pixel(e: &Engine, id: NodeId) -> (u8, u8, u8) {
    let pm = designos_render::render_node(&e.doc, id, 1.0).expect("render");
    let p = pm.pixel(pm.width() / 2, pm.height() / 2).expect("pixel");
    (p.red(), p.green(), p.blue())
}

/// Count of opaque red-dominant pixels in the node's own render.
pub fn red_pixels(e: &Engine, id: NodeId) -> usize {
    let pm = designos_render::render_node(&e.doc, id, 2.0).expect("render");
    pm.pixels().iter().filter(|p| p.alpha() > 200 && p.red() > 200 && p.green() < 40 && p.blue() < 40).count()
}

pub fn assert_bound_to(e: &Engine, id: NodeId, var: &str, hex: &str) {
    assert_eq!(fill_bindings(e, id), vec![("fills/0".to_string(), var.to_string())], "binding on {id:?}");
    let raw = raw_paints(e, id);
    assert!(raw.len() == 1 && solid_hex(&raw[0]) == hex, "raw paint follows the variable: {raw:?}");
    let drawn = drawn_paints(e, id);
    assert!(drawn.len() == 1 && solid_hex(&drawn[0]) == hex, "drawn paint follows the variable: {drawn:?}");
}

pub fn assert_plain(e: &Engine, id: NodeId, hex: &str) {
    assert_eq!(fill_bindings(e, id), Vec::<(String, String)>::new(), "no fill binding left on {id:?}");
    assert!(!node(e, id).style_refs.contains_key("fill"), "no fill style link left on {id:?}: {:?}", node(e, id).style_refs);
    let drawn = drawn_paints(e, id);
    assert!(drawn.len() == 1 && solid_hex(&drawn[0]) == hex, "drawn paint is {hex}: {drawn:?}");
}

pub fn err_json(err: ToolError) -> Value {
    err.to_json_value()
}
