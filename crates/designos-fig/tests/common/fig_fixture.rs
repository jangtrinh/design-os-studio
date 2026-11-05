//! Hand-written `.fig` node changes for save and reopen tests, plus helpers to read the reopened document.
#![allow(dead_code)]

use designos_doc::*;
use designos_fig::decode_fig_to_json;
use serde_json::{json, Value};

pub fn guid(s: u64, l: u64) -> Value {
    json!({"sessionID": s, "localID": l})
}

pub fn var_alias(s: u64, l: u64) -> Value {
    json!({"dataType": "ALIAS", "resolvedDataType": "COLOR", "value": {"alias": {"guid": guid(s, l)}}})
}

pub fn fig_node(id: (u64, u64), parent: (u64, u64), ty: &str, name: &str, extra: Value) -> Value {
    let mut v = json!({
        "guid": guid(id.0, id.1),
        "parentIndex": {"guid": guid(parent.0, parent.1), "position": "!"},
        "type": ty, "name": name, "phase": "CREATED", "visible": true, "opacity": 1.0,
        "size": {"x": 120.0, "y": 48.0},
        "transform": {"m00": 1.0, "m01": 0.0, "m02": 0.0, "m10": 0.0, "m11": 1.0, "m12": 0.0}
    });
    for (k, x) in extra.as_object().expect("extra object") {
        v[k] = x.clone();
    }
    v
}

pub fn document_and_page() -> Vec<Value> {
    vec![fig_node((0, 0), (0, 0), "DOCUMENT", "Document", json!({})), fig_node((0, 1), (0, 0), "CANVAS", "Page 1", json!({}))]
}

pub fn rgba(r: f64, g: f64, b: f64) -> Value {
    json!({"r": r, "g": g, "b": b, "a": 1.0})
}

pub fn white() -> Value {
    rgba(1.0, 1.0, 1.0)
}

pub fn blue() -> Value {
    rgba(f64::from(0x1E) / 255.0, f64::from(0x40) / 255.0, f64::from(0xAF) / 255.0)
}

pub const BLUE_HEX: &str = "#1E40AF";

/// A solid paint, bound to the colour variable `var` when given.
pub fn solid(c: &Value, var: Option<(u64, u64)>) -> Value {
    let mut p = json!({"type": "SOLID", "color": c, "opacity": 1.0, "visible": true, "blendMode": "NORMAL"});
    if let Some((s, l)) = var {
        p["colorVar"] = var_alias(s, l);
    }
    p
}

/// A `VARIABLE_SET` with a single mode.
pub fn variable_set(id: (u64, u64), name: &str, mode: (u64, u64), extra: Value) -> Value {
    let mut base = json!({"key": format!("set-{}-{}", id.0, id.1), "variableSetModes": [{"id": guid(mode.0, mode.1), "name": "Light", "sortPosition": "!"}]});
    for (k, x) in extra.as_object().expect("extra object") {
        base[k] = x.clone();
    }
    fig_node(id, (0, 1), "VARIABLE_SET", name, base)
}

/// A colour `VARIABLE` holding one value in `mode`.
pub fn color_var(id: (u64, u64), set: (u64, u64), mode: (u64, u64), name: &str, c: &Value, extra: Value) -> Value {
    let mut base = json!({
        "key": format!("key-{}-{}", id.0, id.1),
        "variableSetID": {"guid": guid(set.0, set.1)},
        "variableResolvedType": "COLOR",
        "variableDataValues": {"entries": [{"modeID": guid(mode.0, mode.1), "variableData": {"dataType": "COLOR", "value": {"colorValue": c}}}]}
    });
    for (k, x) in extra.as_object().expect("extra object") {
        base[k] = x.clone();
    }
    fig_node(id, (0, 1), "VARIABLE", name, base)
}

/// The decoded node changes of a saved `.fig`.
pub fn node_changes(bytes: &[u8]) -> Vec<Value> {
    decode_fig_to_json(bytes).expect("decode saved .fig")["message"]["nodeChanges"].as_array().cloned().unwrap_or_default()
}

pub fn guid_text(v: &Value) -> String {
    format!("{}:{}", v["sessionID"], v["localID"])
}

pub fn by_name<'a>(doc: &'a Document, name: &str) -> &'a Node {
    doc.nodes.values().find(|n| n.name == name).unwrap_or_else(|| panic!("no node named {name}"))
}

pub fn kid<'a>(doc: &'a Document, parent: &Node, name: &str) -> &'a Node {
    parent.children.iter().filter_map(|&c| doc.get(c)).find(|n| n.name == name).unwrap_or_else(|| panic!("no child named {name}"))
}

fn hex(c: Option<Color>) -> String {
    c.map(|c| format!("#{:02X}{:02X}{:02X}", (c.r * 255.0).round() as u8, (c.g * 255.0).round() as u8, (c.b * 255.0).round() as u8)).unwrap_or("-".into())
}

fn solid_hex(paints: &[Paint]) -> String {
    hex(paints.first().and_then(|p| match &p.kind {
        PaintKind::Solid(c) => Some(*c),
        _ => None,
    }))
}

/// The first fill of `id` after variable bindings resolve, as `#RRGGBB`.
pub fn resolved_fill(doc: &Document, id: NodeId) -> String {
    solid_hex(&resolve_node(doc, doc.get(id).expect("node")).fills)
}

/// The first stroke of `id` after variable bindings resolve, as `#RRGGBB`.
pub fn resolved_stroke(doc: &Document, id: NodeId) -> String {
    solid_hex(&resolve_node(doc, doc.get(id).expect("node")).stroke.paints)
}
