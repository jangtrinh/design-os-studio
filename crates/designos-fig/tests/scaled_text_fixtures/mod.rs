//! Kiwi-shaped fixtures for text in instances made with Figma's scale tool, shared by the scaled-text tests.
//! Hand-written; nothing here comes from a real file.
// Each test crate that includes this module uses only part of it.
#![allow(dead_code)]

use designos_doc::*;
use serde_json::{json, Value};

pub fn g(s: u64, l: u64) -> Value {
    json!({"sessionID": s, "localID": l})
}

pub fn node(id: (u64, u64), parent: (u64, u64), pos: &str, ty: &str, name: &str, extra: Value) -> Value {
    let mut v = json!({
        "guid": g(id.0, id.1), "parentIndex": {"guid": g(parent.0, parent.1), "position": pos},
        "type": ty, "name": name, "phase": "CREATED", "visible": true, "opacity": 1.0,
        "size": {"x": 10.0, "y": 10.0},
        "transform": {"m00": 1.0, "m01": 0.0, "m02": 0.0, "m10": 0.0, "m11": 1.0, "m12": 0.0}
    });
    for (k, x) in extra.as_object().unwrap() {
        v[k] = x.clone();
    }
    v
}

pub fn at(x: f64, y: f64) -> Value {
    json!({"m00": 1.0, "m01": 0.0, "m02": x, "m10": 0.0, "m11": 1.0, "m12": y})
}

pub fn solid(r: f64, gr: f64, b: f64) -> Value {
    json!({"type": "SOLID", "color": {"r": r, "g": gr, "b": b, "a": 1.0}, "opacity": 1.0, "visible": true, "blendMode": "NORMAL"})
}

pub fn alias(key: &str) -> Value {
    json!({"dataType": "ALIAS", "resolvedDataType": "FLOAT", "value": {"alias": {"assetRef": {"key": key, "version": "1"}}}})
}

pub fn color_alias(key: &str) -> Value {
    json!({"dataType": "ALIAS", "resolvedDataType": "COLOR", "value": {"alias": {"assetRef": {"key": key, "version": "1"}}}})
}

pub const LIGHT: (u64, u64) = (6, 10);
pub const DARK: (u64, u64) = (6, 11);

/// A two-mode number variable on the internal canvas.
pub fn float_var(id: (u64, u64), name: &str, key: &str, light: f64, dark: f64) -> Value {
    let entry = |mode: (u64, u64), x: f64| json!({"modeID": g(mode.0, mode.1), "variableData": {"dataType": "FLOAT", "resolvedDataType": "FLOAT", "value": {"floatValue": x}}});
    node(
        id,
        (0, 2),
        "v",
        "VARIABLE",
        name,
        json!({
            "key": key, "variableResolvedType": "FLOAT",
            "variableSetID": {"assetRef": {"key": "SET_TYPE", "version": "1"}},
            "variableDataValues": {"entries": [entry(LIGHT, light), entry(DARK, dark)]}
        }),
    )
}

/// A two-mode colour variable on the internal canvas.
pub fn color_var(id: (u64, u64), name: &str, key: &str, light: (f64, f64, f64), dark: (f64, f64, f64)) -> Value {
    let entry = |mode: (u64, u64), c: (f64, f64, f64)| {
        json!({"modeID": g(mode.0, mode.1), "variableData": {"dataType": "COLOR", "resolvedDataType": "COLOR",
            "value": {"colorValue": {"r": c.0, "g": c.1, "b": c.2, "a": 1.0}}}})
    };
    node(
        id,
        (0, 2),
        "v",
        "VARIABLE",
        name,
        json!({
            "key": key, "variableResolvedType": "COLOR",
            "variableSetID": {"assetRef": {"key": "SET_TYPE", "version": "1"}},
            "variableDataValues": {"entries": [entry(LIGHT, light), entry(DARK, dark)]}
        }),
    )
}

/// The document, its pages, the variable set, a text style bound to variables ("Bound", 14 / 20 in light mode,
/// 20 / 24 in dark mode) and a static text style ("Static", 14 / 20).
pub fn base() -> Vec<Value> {
    vec![
        node((0, 0), (0, 0), "!", "DOCUMENT", "Document", json!({})),
        node((0, 1), (0, 0), "a", "CANVAS", "Page", json!({})),
        node((0, 2), (0, 0), "b", "CANVAS", "Internal Only Canvas", json!({"internalOnly": true})),
        node(
            (6, 1),
            (0, 2),
            "v",
            "VARIABLE_SET",
            "Type",
            json!({"key": "SET_TYPE", "variableSetModes": [
                {"id": g(LIGHT.0, LIGHT.1), "name": "Light", "sortPosition": "!"},
                {"id": g(DARK.0, DARK.1), "name": "Dark", "sortPosition": "\""}
            ]}),
        ),
        float_var((6, 2), "Font size/text-sm", "VAR_SIZE", 14.0, 20.0),
        float_var((6, 3), "Line height/text-sm", "VAR_LH", 20.0, 24.0),
        color_var((6, 4), "Text/primary", "VAR_INK", (1.0, 0.0, 0.0), (0.0, 0.0, 1.0)),
        node(
            (8, 2),
            (0, 2),
            "s",
            "TEXT",
            "Bound",
            json!({
                "styleType": "TEXT", "fontSize": 14.0, "fontName": {"family": "Inter", "style": "Regular"},
                "lineHeight": {"units": "PIXELS", "value": 20.0},
                "variableConsumptionMap": {"entries": [
                    {"variableField": "FONT_SIZE", "variableData": alias("VAR_SIZE")},
                    {"variableField": "LINE_HEIGHT", "variableData": alias("VAR_LH")}
                ]}
            }),
        ),
        node(
            (8, 3),
            (0, 2),
            "t",
            "TEXT",
            "Static",
            json!({
                "styleType": "TEXT", "fontSize": 14.0, "fontName": {"family": "Inter", "style": "Regular"},
                "lineHeight": {"units": "PIXELS", "value": 20.0}
            }),
        ),
    ]
}

/// A text record linked to the style with guid `style`.
pub fn text(id: (u64, u64), parent: (u64, u64), name: &str, style: (u64, u64), extra: Value) -> Value {
    let mut v = node(
        id,
        parent,
        "a",
        "TEXT",
        name,
        json!({
            "transform": at(6.0, 4.0), "size": {"x": 88.0, "y": 20.0},
            "fontSize": 14.0, "fontName": {"family": "Inter", "style": "Regular"},
            "lineHeight": {"units": "PIXELS", "value": 20.0},
            "styleIdForText": {"guid": g(style.0, style.1)},
            "fillPaints": [solid(0.0, 0.0, 0.0)],
            "textData": {"characters": name}
        }),
    );
    for (k, x) in extra.as_object().unwrap() {
        v[k] = x.clone();
    }
    v
}

/// A local component "Badge" (100 x 28) holding `children`, and an instance of it at `scale`.
pub fn badge_doc(children: Vec<Value>, scale: f64) -> Value {
    let mut all = base();
    all.push(node((5, 1), (0, 1), "c", "SYMBOL", "Badge", json!({"size": {"x": 100.0, "y": 28.0}, "transform": at(0.0, 200.0)})));
    all.extend(children);
    all.push(node(
        (1, 1),
        (0, 1),
        "a",
        "INSTANCE",
        "Scaled badge",
        json!({"size": {"x": 100.0 * scale, "y": 28.0 * scale}, "symbolData": {"symbolID": g(5, 1), "uniformScaleFactor": scale}}),
    ));
    Value::Array(all)
}

pub fn find<'a>(doc: &'a Document, name: &str) -> &'a Node {
    doc.nodes.values().find(|n| n.name == name).unwrap_or_else(|| panic!("no node {name}"))
}

pub fn child<'a>(doc: &'a Document, n: &Node, name: &str) -> &'a Node {
    n.children.iter().map(|c| doc.get(*c).unwrap()).find(|c| c.name == name).unwrap_or_else(|| panic!("no child {name}"))
}

pub fn text_of(n: &Node) -> &TextProps {
    match &n.kind {
        NodeKind::Text(t) => t,
        k => panic!("{} is not text: {k:?}", n.name),
    }
}

pub fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-3
}

pub fn px(lh: LineHeight) -> f32 {
    match lh {
        LineHeight::Pixels(v) => v,
        other => panic!("line height is not in pixels: {other:?}"),
    }
}

/// Size and pixel line height of `n` as drawn: bindings and linked style bindings applied.
pub fn drawn(doc: &Document, n: &Node) -> (f32, f32) {
    let r = resolve_node(doc, n);
    let t = text_of(&r);
    (t.style.font_size, px(t.style.line_height))
}

pub fn scaled_rating(doc: &Document) -> &Node {
    child(doc, find(doc, "Scaled badge"), "Rating")
}

pub fn set_page_mode(doc: &mut Document, dark: bool) {
    let coll = doc.variable_collections.values().next().expect("a collection");
    let (cid, mid) = (coll.id.clone(), coll.modes[usize::from(dark)].id.clone());
    let page = doc.pages[0];
    doc.get_mut(page).unwrap().variables.modes.insert(cid, mid);
}
