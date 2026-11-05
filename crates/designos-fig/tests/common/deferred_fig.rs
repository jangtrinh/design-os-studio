//! A `.fig` with pages the loader defers: three pages and 25,000 filler rectangles, a main component `Buttons/Button`, a
//! variant main component and an instance of the button inside it, plus readers for the saved file.
#![allow(dead_code)]

use super::fig_fixture::*;
use designos_doc::*;
use designos_fig::container::{self, Compression};
use designos_fig::import_fig;
use designos_fig::kiwi::{Field, FieldType, KiwiValue as V};
use designos_fig::testkit::{figma_like_schema, msg, st};
use serde_json::{json, Value};

/// Bytes of a `.fig` holding `changes` (JSON node changes) and `blobs`.
pub fn fig_bytes(changes: Vec<Value>, blobs: Vec<Vec<u8>>) -> Vec<u8> {
    let schema = figma_like_schema();
    let nc = schema.find("NodeChange").expect("NodeChange in the schema");
    let field = Field { name: "nodeChanges".into(), ty: FieldType::Def(nc), is_array: true, value: 0 };
    let Some(V::Array(items)) = schema.field_from_json_strict(&field, &Value::Array(changes), 0) else { panic!("node changes fit the schema") };
    let blob_values = blobs.into_iter().map(|b| st(vec![("bytes", V::Bytes(b))])).collect();
    let root = schema.find("Message").expect("Message in the schema");
    let body = schema.encode_message(root, &msg(vec![("nodeChanges", V::Array(items)), ("blobs", V::Array(blob_values))])).expect("encode");
    container::write_container(&schema.encode(), &body, Compression::Zstd, 106)
}

pub const BUTTON: (u64, u64) = (1, 10);
pub const LABEL: (u64, u64) = (1, 11);
pub const ICON: (u64, u64) = (1, 12);
pub const VARIANT: (u64, u64) = (1, 20);
pub const INSTANCE: (u64, u64) = (1, 21);
pub const STROKE_VAR: (u64, u64) = (1, 3);

pub fn rect_nc(id: (u64, u64), parent: (u64, u64), name: &str) -> Value {
    fig_node(id, parent, "ROUNDED_RECTANGLE", name, json!({"size": {"x": 4.0, "y": 4.0}}))
}

/// Three pages and 25,000 filler rectangles on the first, so that the loader defers the third page ("404 pages"). That
/// page holds a main component `Buttons/Button` (a label and an icon), a main component `Type=Simple 03` and, inside
/// it, an instance of the button. `instance_overrides` is the instance's `symbolOverrides`; `extra` adds nodes.
pub fn fixture(instance_overrides: Value, extra: Vec<Value>, blobs: Vec<Vec<u8>>) -> Vec<u8> {
    fixture_with(json!({}), instance_overrides, extra, blobs)
}

/// [`fixture`] where `instance_fields` are further fields of the instance (for example its `derivedSymbolData`).
pub fn fixture_with(instance_fields: Value, instance_overrides: Value, extra: Vec<Value>, blobs: Vec<Vec<u8>>) -> Vec<u8> {
    let mut instance = json!({"symbolData": {"symbolID": guid(BUTTON.0, BUTTON.1), "symbolOverrides": instance_overrides}});
    for (k, x) in instance_fields.as_object().expect("instance fields object") {
        instance[k] = x.clone();
    }
    let mut changes = vec![
        fig_node((0, 0), (0, 0), "DOCUMENT", "Document", json!({})),
        fig_node((0, 1), (0, 0), "CANVAS", "Cover", json!({})),
        fig_node((0, 2), (0, 0), "CANVAS", "Page 2", json!({})),
        fig_node((0, 3), (0, 0), "CANVAS", "404 pages", json!({})),
        fig_node(
            (1, 2),
            (0, 0),
            "VARIABLE_SET",
            "Colors",
            json!({"defaultModeID": guid(1, 100), "variableSetModes": [{"id": guid(1, 100), "name": "Light", "sortPosition": "!"}]}),
        ),
        fig_node(
            STROKE_VAR,
            (1, 2),
            "VARIABLE",
            "fg-quaternary",
            json!({
                "variableSetID": {"guid": guid(1, 2)},
                "variableResolvedType": "COLOR",
                "variableDataValues": {"entries": [{
                    "modeID": guid(1, 100),
                    "variableData": {"dataType": "COLOR", "resolvedDataType": "COLOR", "value": {"colorValue": rgba(0.64, 0.66, 0.68)}}
                }]}
            }),
        ),
        fig_node(BUTTON, (0, 3), "SYMBOL", "Buttons/Button", json!({})),
        fig_node(LABEL, BUTTON, "TEXT", "Text", json!({"fontSize": 14.0, "textData": {"characters": "Button CTA"}})),
        fig_node(ICON, BUTTON, "ROUNDED_RECTANGLE", "Icon", json!({"strokePaints": [solid(&rgba(0.0, 0.0, 0.0), None)], "strokeWeight": 1.5})),
        fig_node(VARIANT, (0, 3), "SYMBOL", "Type=Simple 03, Breakpoint=Desktop", json!({"size": {"x": 1440.0, "y": 960.0}})),
        fig_node(INSTANCE, VARIANT, "INSTANCE", "Buttons/Button", instance),
    ];
    changes.extend(extra);
    changes.extend((0..25_000u64).map(|i| rect_nc((9, i + 1), (0, 1), "F")));
    fig_bytes(changes, blobs)
}

pub fn text_and_stroke_overrides() -> Value {
    json!([
        {"guidPath": {"guids": [guid(LABEL.0, LABEL.1)]}, "textData": {"characters": "Go back"}},
        {
            "guidPath": {"guids": [guid(ICON.0, ICON.1)]},
            "strokePaints": [solid(&rgba(0.64, 0.66, 0.68), Some(STROKE_VAR))],
            "strokeWeight": 1.6666666
        }
    ])
}

pub fn deferred_fixture() -> Vec<u8> {
    fixture(text_and_stroke_overrides(), Vec::new(), Vec::new())
}

pub fn page_named(doc: &Document, name: &str) -> NodeId {
    doc.pages.iter().copied().find(|&p| doc.get(p).is_some_and(|n| n.name.trim() == name)).unwrap_or_else(|| panic!("no page {name}"))
}

pub fn page_hydrated(doc: &Document, page: NodeId) -> bool {
    doc.page_hydrator.as_ref().is_none_or(|h| h.lock().unwrap().is_hydrated(page))
}

pub fn variant_button(doc: &Document) -> NodeId {
    let variant = doc.nodes.values().find(|n| matches!(n.kind, NodeKind::Component(_)) && n.name == "Type=Simple 03, Breakpoint=Desktop").expect("variant");
    variant.children[0]
}

pub fn label_of(doc: &Document, instance: NodeId) -> String {
    let n = doc.get(instance).expect("instance");
    n.children
        .iter()
        .filter_map(|&c| doc.get(c))
        .find_map(|c| match &c.kind {
            NodeKind::Text(tp) => Some(tp.content.clone()),
            _ => None,
        })
        .unwrap_or_else(|| "<no text child>".into())
}

pub fn reopened_label(saved: &[u8]) -> String {
    let mut doc = import_fig(saved).expect("reopen");
    doc.hydrate_all_pages();
    label_of(&doc, variant_button(&doc))
}

/// The `symbolOverrides` entries the saved file holds for the node with guid `id`.
pub fn saved_entries(saved: &[u8], id: (u64, u64)) -> Vec<Value> {
    let want = guid(id.0, id.1);
    node_changes(saved).into_iter().find(|nc| nc["guid"] == want).unwrap_or_else(|| panic!("no node {id:?} in the saved file"))["symbolData"]["symbolOverrides"]
        .as_array()
        .cloned()
        .unwrap_or_default()
}

pub fn entry_at(entries: &[Value], leaf: (u64, u64)) -> &Value {
    let want = guid(leaf.0, leaf.1);
    entries.iter().find(|e| e["guidPath"]["guids"].as_array().and_then(|g| g.last()) == Some(&want)).unwrap_or_else(|| panic!("no entry for {leaf:?}"))
}

pub fn children_written_under(saved: &[u8], parent: (u64, u64)) -> usize {
    let want = guid(parent.0, parent.1);
    node_changes(saved).iter().filter(|nc| nc["parentIndex"]["guid"] == want).count()
}

/// A geometry blob: a move and a line, as Figma stores path commands.
pub fn line_blob(x: f32) -> Vec<u8> {
    let mut b = vec![1u8];
    for v in [0.0f32, 0.0] {
        b.extend_from_slice(&v.to_le_bytes());
    }
    b.push(2);
    for v in [x, 5.0f32] {
        b.extend_from_slice(&v.to_le_bytes());
    }
    b
}

pub fn saved_blob(saved: &[u8], index: u64) -> Vec<u8> {
    let decoded = designos_fig::decode_fig_to_json(saved).expect("decode");
    let hex = decoded["message"]["blobs"][index as usize]["bytes"].as_str().expect("blob bytes").to_string();
    (0..hex.len()).step_by(2).map(|i| u8::from_str_radix(&hex[i..i + 2], 16).expect("hex")).collect()
}

/// The `derivedSymbolData` entries the saved file holds for the node with guid `id`.
pub fn saved_derived(saved: &[u8], id: (u64, u64)) -> Vec<Value> {
    let want = guid(id.0, id.1);
    node_changes(saved).into_iter().find(|nc| nc["guid"] == want).unwrap_or_else(|| panic!("no node {id:?} in the saved file"))["derivedSymbolData"]
        .as_array()
        .cloned()
        .unwrap_or_default()
}
