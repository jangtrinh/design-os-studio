//! A `.fig` save of a document that still has pages the loader has not opened.
//!
//! A large multi-page file opens with the instances of its inactive pages unexpanded. A save must keep what the file
//! says about them (their `symbolOverrides`) without opening those pages, and it must leave the document it saves
//! exactly as it found it: the user can still open the pages afterwards, and a second save keeps the overrides too.

mod common;

use common::fig_fixture::*;
use designos_doc::*;
use designos_fig::container::{self, Compression};
use designos_fig::kiwi::{Field, FieldType, KiwiValue as V};
use designos_fig::testkit::{figma_like_schema, msg, st};
use designos_fig::{export_fig, import_fig};
use serde_json::{json, Value};

/// Bytes of a `.fig` holding `changes` (JSON node changes) and `blobs`.
fn fig_bytes(changes: Vec<Value>, blobs: Vec<Vec<u8>>) -> Vec<u8> {
    let schema = figma_like_schema();
    let nc = schema.find("NodeChange").expect("NodeChange in the schema");
    let field = Field { name: "nodeChanges".into(), ty: FieldType::Def(nc), is_array: true, value: 0 };
    let Some(V::Array(items)) = schema.field_from_json_strict(&field, &Value::Array(changes), 0) else { panic!("node changes fit the schema") };
    let blob_values = blobs.into_iter().map(|b| st(vec![("bytes", V::Bytes(b))])).collect();
    let root = schema.find("Message").expect("Message in the schema");
    let body = schema.encode_message(root, &msg(vec![("nodeChanges", V::Array(items)), ("blobs", V::Array(blob_values))])).expect("encode");
    container::write_container(&schema.encode(), &body, Compression::Zstd, 106)
}

const BUTTON: (u64, u64) = (1, 10);
const LABEL: (u64, u64) = (1, 11);
const ICON: (u64, u64) = (1, 12);
const VARIANT: (u64, u64) = (1, 20);
const INSTANCE: (u64, u64) = (1, 21);
const STROKE_VAR: (u64, u64) = (1, 3);

fn rect_nc(id: (u64, u64), parent: (u64, u64), name: &str) -> Value {
    fig_node(id, parent, "ROUNDED_RECTANGLE", name, json!({"size": {"x": 4.0, "y": 4.0}}))
}

/// Three pages and 25,000 filler rectangles on the first, so that the loader defers the third page ("404 pages"). That
/// page holds a main component `Buttons/Button` (a label and an icon), a main component `Type=Simple 03` and, inside
/// it, an instance of the button. `instance_overrides` is the instance's `symbolOverrides`; `extra` adds nodes.
fn fixture(instance_overrides: Value, extra: Vec<Value>, blobs: Vec<Vec<u8>>) -> Vec<u8> {
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
        fig_node(
            INSTANCE,
            VARIANT,
            "INSTANCE",
            "Buttons/Button",
            json!({"symbolData": {"symbolID": guid(BUTTON.0, BUTTON.1), "symbolOverrides": instance_overrides}}),
        ),
    ];
    changes.extend(extra);
    changes.extend((0..25_000u64).map(|i| rect_nc((9, i + 1), (0, 1), "F")));
    fig_bytes(changes, blobs)
}

fn text_and_stroke_overrides() -> Value {
    json!([
        {"guidPath": {"guids": [guid(LABEL.0, LABEL.1)]}, "textData": {"characters": "Go back"}},
        {
            "guidPath": {"guids": [guid(ICON.0, ICON.1)]},
            "strokePaints": [solid(&rgba(0.64, 0.66, 0.68), Some(STROKE_VAR))],
            "strokeWeight": 1.6666666
        }
    ])
}

fn deferred_fixture() -> Vec<u8> {
    fixture(text_and_stroke_overrides(), Vec::new(), Vec::new())
}

fn page_named(doc: &Document, name: &str) -> NodeId {
    doc.pages.iter().copied().find(|&p| doc.get(p).is_some_and(|n| n.name.trim() == name)).unwrap_or_else(|| panic!("no page {name}"))
}

fn page_hydrated(doc: &Document, page: NodeId) -> bool {
    doc.page_hydrator.as_ref().is_none_or(|h| h.lock().unwrap().is_hydrated(page))
}

fn variant_button(doc: &Document) -> NodeId {
    let variant = doc.nodes.values().find(|n| matches!(n.kind, NodeKind::Component(_)) && n.name == "Type=Simple 03, Breakpoint=Desktop").expect("variant");
    variant.children[0]
}

fn label_of(doc: &Document, instance: NodeId) -> String {
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

fn reopened_label(saved: &[u8]) -> String {
    let mut doc = import_fig(saved).expect("reopen");
    doc.hydrate_all_pages();
    label_of(&doc, variant_button(&doc))
}

/// The `symbolOverrides` entries the saved file holds for the node with guid `id`.
fn saved_entries(saved: &[u8], id: (u64, u64)) -> Vec<Value> {
    let want = guid(id.0, id.1);
    node_changes(saved).into_iter().find(|nc| nc["guid"] == want).unwrap_or_else(|| panic!("no node {id:?} in the saved file"))["symbolData"]["symbolOverrides"]
        .as_array()
        .cloned()
        .unwrap_or_default()
}

fn entry_at(entries: &[Value], leaf: (u64, u64)) -> &Value {
    let want = guid(leaf.0, leaf.1);
    entries.iter().find(|e| e["guidPath"]["guids"].as_array().and_then(|g| g.last()) == Some(&want)).unwrap_or_else(|| panic!("no entry for {leaf:?}"))
}

fn children_written_under(saved: &[u8], parent: (u64, u64)) -> usize {
    let want = guid(parent.0, parent.1);
    node_changes(saved).iter().filter(|nc| nc["parentIndex"]["guid"] == want).count()
}

#[test]
fn export_leaves_the_deferred_page_of_the_exported_document_loadable() {
    let mut live = import_fig(&deferred_fixture()).expect("import");
    let page = page_named(&live, "404 pages");
    let button = variant_button(&live);
    assert!(!page_hydrated(&live, page), "the importer defers the page");
    assert!(live.get(button).unwrap().children.is_empty(), "the instance is unexpanded");

    export_fig(&live).expect("export");

    assert!(!page_hydrated(&live, page), "an export does not load the page of the document it saves");
    assert!(live.ensure_page_hydrated(page), "the page still loads on demand");
    assert_eq!(live.get(button).unwrap().children.len(), 2, "the instance expands to the label and the icon");
    assert_eq!(label_of(&live, button), "Go back", "and carries its override");
}

#[test]
fn a_second_export_from_the_same_document_keeps_the_deferred_overrides() {
    let live = import_fig(&deferred_fixture()).expect("import");
    let first = export_fig(&live).expect("first export");
    let second = export_fig(&live).expect("second export");
    assert_eq!(reopened_label(&first), "Go back");
    assert_eq!(reopened_label(&second), "Go back");
}

#[test]
fn loading_every_page_after_an_export_gives_the_node_count_of_a_fresh_open() {
    let fig = deferred_fixture();
    let mut fresh = import_fig(&fig).expect("import");
    fresh.hydrate_all_pages();

    let mut live = import_fig(&fig).expect("import");
    export_fig(&live).expect("export");
    live.hydrate_all_pages();

    assert_eq!(live.nodes.len(), fresh.nodes.len());
}

#[test]
fn export_of_a_deferred_page_instance_writes_its_overrides_without_loading_the_page() {
    let fig = deferred_fixture();
    let original = saved_entries(&fig, INSTANCE);
    assert_eq!(original.len(), 2, "the fixture states two entries");

    let live = import_fig(&fig).expect("import");
    let page = page_named(&live, "404 pages");
    let saved = export_fig(&live).expect("export");

    assert!(!page_hydrated(&live, page), "the page stays unloaded");
    assert_eq!(children_written_under(&saved, INSTANCE), 0, "no children are written for the unexpanded instance");
    let entries = saved_entries(&saved, INSTANCE);
    assert_eq!(entries, original, "the instance's entries are the ones the file stated");
    assert_eq!(entry_at(&entries, LABEL)["textData"]["characters"], json!("Go back"));
    let stroke = entry_at(&entries, ICON);
    assert_eq!(stroke["strokePaints"][0]["colorVar"]["value"]["alias"]["guid"], guid(STROKE_VAR.0, STROKE_VAR.1), "the stroke keeps its variable");
    assert_eq!(reopened_label(&saved), "Go back");
}

#[test]
fn export_of_a_deferred_page_instance_keeps_a_component_property_it_sets_on_a_nested_instance() {
    // `Card` (1:40) holds an instance of `Badge` (1:30), which has a text property `Label`; the instance on the
    // deferred page sets that property on the nested badge (1:41).
    let label_def = (4, 31);
    let extra = vec![
        fig_node(
            (1, 30),
            (0, 3),
            "SYMBOL",
            "Badge",
            json!({"componentPropDefs": [{
                "id": guid(label_def.0, label_def.1), "name": "Label", "type": "TEXT",
                "initialValue": {"textValue": {"characters": "Badge"}}
            }]}),
        ),
        fig_node((1, 40), (0, 3), "SYMBOL", "Card", json!({})),
        fig_node((1, 41), (1, 40), "INSTANCE", "Badge", json!({"symbolData": {"symbolID": guid(1, 30)}})),
        fig_node(
            (1, 50),
            (1, 20),
            "INSTANCE",
            "Card",
            json!({"symbolData": {"symbolID": guid(1, 40), "symbolOverrides": [{
                "guidPath": {"guids": [guid(1, 41)]},
                "componentPropAssignments": [{"defID": guid(label_def.0, label_def.1), "value": {"textValue": {"characters": "New"}}}]
            }]}}),
        ),
    ];
    let live = import_fig(&fixture(text_and_stroke_overrides(), extra, Vec::new())).expect("import");
    let saved = export_fig(&live).expect("export");

    let entries = saved_entries(&saved, (1, 50));
    let assigned = entry_at(&entries, (1, 41))["componentPropAssignments"][0].clone();
    let saved_defs: Vec<Value> = node_changes(&saved)
        .into_iter()
        .find(|nc| nc["guid"] == guid(1, 30))
        .map(|nc| nc["componentPropDefs"].as_array().cloned().unwrap_or_default())
        .unwrap_or_default();
    assert_eq!(saved_defs.len(), 1, "the saved file defines the property once");
    assert_eq!(assigned["defID"], saved_defs[0]["id"], "the assignment names the definition the saved file writes");

    let mut reopened = import_fig(&saved).expect("reopen");
    reopened.hydrate_all_pages();
    let card = reopened.nodes.values().find(|n| n.name == "Card" && matches!(n.kind, NodeKind::Instance { .. })).expect("card instance");
    let badge = card.children.iter().filter_map(|&c| reopened.get(c)).find(|n| n.name == "Badge").expect("badge in the card instance");
    assert_eq!(badge.component.values.get("Label"), Some(&PropValue::Text("New".into())), "the property value survives the save");
}

/// A geometry blob: a move and a line, as Figma stores path commands.
fn line_blob(x: f32) -> Vec<u8> {
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

fn saved_blob(saved: &[u8], index: u64) -> Vec<u8> {
    let decoded = designos_fig::decode_fig_to_json(saved).expect("decode");
    let hex = decoded["message"]["blobs"][index as usize]["bytes"].as_str().expect("blob bytes").to_string();
    (0..hex.len()).step_by(2).map(|i| u8::from_str_radix(&hex[i..i + 2], 16).expect("hex")).collect()
}

#[test]
fn export_of_a_deferred_page_instance_points_its_geometry_at_the_blobs_of_the_saved_file() {
    // The entry names blob 3 of the file it came from; the saved file has a blob table of its own.
    let wanted = line_blob(7.0);
    let blobs = vec![line_blob(1.0), line_blob(2.0), line_blob(3.0), wanted.clone()];
    let overrides = json!([{
        "guidPath": {"guids": [guid(ICON.0, ICON.1)]},
        "strokePaints": [solid(&rgba(0.64, 0.66, 0.68), None)],
        "strokeGeometry": [{"windingRule": "NONZERO", "commandsBlob": 3}]
    }]);
    let live = import_fig(&fixture(overrides, Vec::new(), blobs)).expect("import");
    let saved = export_fig(&live).expect("export");

    let entries = saved_entries(&saved, INSTANCE);
    let geometry = entry_at(&entries, ICON)["strokeGeometry"][0].clone();
    let index = geometry["commandsBlob"].as_u64().expect("the entry names a blob");
    assert_eq!(saved_blob(&saved, index), wanted, "the entry's blob is the one the file stated, wherever the saved table keeps it");
}

#[test]
fn export_writes_a_deferred_instance_from_the_document_when_its_component_changed() {
    let live = import_fig(&deferred_fixture()).expect("import");
    let other = live.nodes.values().find(|n| n.name == "Type=Simple 03, Breakpoint=Desktop").map(|n| n.id).expect("another component");
    let mut live = live;
    let instance = variant_button(&live);
    let NodeKind::Instance { component, .. } = &mut live.get_mut(instance).expect("instance").kind else { panic!("instance") };
    *component = Some(other);

    let saved = export_fig(&live).expect("export");
    assert!(saved_entries(&saved, INSTANCE).is_empty(), "entries written for one component are not written under another");
}
