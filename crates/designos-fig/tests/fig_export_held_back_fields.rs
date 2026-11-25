//! The export schema holds Figma's whole `NodeChange`, but a field no writer gates on the layer being unedited is not
//! written yet. These tests pin both halves: the schema takes every field of Figma's tree whole, and the writers that read
//! from the schema (the metadata writer and the override writer of an unopened instance) leave those fields out, so a
//! save never puts an old copy of one beside an edit.
//!
//! The input files are written with Figma's own schema, so they hold the fields. A file written with Studio's schema
//! before this change would drop them while it was built, and a guard on it would pass for the wrong reason.

mod common;

use common::deferred_fig::{rect_nc, BUTTON, ICON, INSTANCE, LABEL, VARIANT};
use common::fig_fixture::*;
use designos_doc::*;
use designos_fig::container::{self, Compression};
use designos_fig::kiwi::{Field, FieldType, KiwiValue as V, Schema};
use designos_fig::testkit::{figma_like_schema, msg, parse_schema_text};
use designos_fig::{export_fig, import_fig, import_fig_node_changes};
use serde_json::{json, Value};

fn real() -> Schema {
    parse_schema_text(include_str!("data/figma-real-schema.txt"))
}

/// A `.fig` of `changes` (JSON node changes) written with Figma's real schema, so every field of every change is kept.
fn real_fig_bytes(changes: Vec<Value>) -> Vec<u8> {
    let schema = real();
    let nc = schema.find("NodeChange").expect("NodeChange in Figma's schema");
    let field = Field { name: "nodeChanges".into(), ty: FieldType::Def(nc), is_array: true, value: 0 };
    let Some(V::Array(items)) = schema.field_from_json_strict(&field, &Value::Array(changes), 0) else { panic!("node changes fit Figma's schema") };
    let root = schema.find("Message").expect("Message in Figma's schema");
    let body = schema.encode_message(root, &msg(vec![("nodeChanges", V::Array(items)), ("blobs", V::Array(Vec::new()))])).expect("encode");
    container::write_container(&schema.encode(), &body, Compression::Zstd, 106)
}

fn record<'a>(changes: &'a [Value], name: &str) -> &'a Value {
    changes.iter().find(|c| c["name"] == name).unwrap_or_else(|| panic!("no record named {name}"))
}

/// Fails when the saved `record` holds any of `keys`.
fn assert_lacks(record: &Value, keys: &[&str], what: &str) {
    for key in keys {
        assert!(record.get(*key).is_none(), "{what}: the saved record holds {key}: {}", record[*key]);
    }
}

#[test]
fn a_node_change_with_the_fields_figma_stores_encodes_in_the_export_schema() {
    let (ours, figma) = (figma_like_schema(), real());
    let nc = figma.find("NodeChange").unwrap();
    let from_json = |name: &str, v: Value| figma.def_field_from_json(nc, name, &v).unwrap_or_else(|| panic!("Figma's schema reads {name}"));
    let fields = vec![
        from_json("fontVersion", json!("1")),
        from_json("textBidiVersion", json!(2)),
        from_json("textUserLayoutVersion", json!(5)),
        from_json("prototypeInteractions", json!([{"id": guid(7, 1), "event": {"interactionType": "ON_CLICK"}, "actions": []}])),
        from_json("derivedSymbolDataLayoutVersion", json!(3)),
        from_json("scrollBehavior", json!("STICKY_SCROLLS")),
    ];
    let value = V::Message(fields);
    let bytes = ours.encode_message(ours.find("NodeChange").unwrap(), &value).expect("the export schema encodes every field");
    assert_eq!(figma.decode_message(nc, &bytes).expect("Figma's schema reads the bytes"), value);
}

#[test]
fn a_symbol_data_whose_override_carries_a_text_layout_version_conforms_whole() {
    let (ours, figma) = (figma_like_schema(), real());
    let nc = figma.find("NodeChange").unwrap();
    let json = json!({
        "symbolID": guid(1, 10),
        "uniformScaleFactor": 1.0,
        "symbolOverrides": [{"guidPath": {"guids": [guid(1, 11)]}, "textData": {"characters": "Go back"}, "fontVersion": "1", "textUserLayoutVersion": 5}]
    });
    let held = figma.def_field_from_json(nc, "symbolData", &json).expect("Figma's schema reads the symbol data").1;
    let own_def = ours.find("SymbolData").unwrap();
    assert_eq!(ours.conform_def(own_def, &held), Some(held.clone()), "the export schema keeps the override's layout version");
}

/// A value of `symbolData` read with the export schema (the way the writers do) holds the key whole.
#[test]
fn a_symbol_data_value_converted_with_the_export_schema_keeps_the_override_field() {
    let ours = figma_like_schema();
    let nc = ours.find("NodeChange").unwrap();
    let json = json!({"symbolID": guid(1, 10), "symbolOverrides": [{"guidPath": {"guids": [guid(1, 11)]}, "textUserLayoutVersion": 5}]});
    let (_, value) = ours.def_field_from_json(nc, "symbolData", &json).expect("symbol data converts");
    let entry = value.get("symbolOverrides").and_then(V::as_array).map(|a| a[0].clone()).expect("one override entry");
    assert_eq!(entry.get("textUserLayoutVersion"), Some(&V::Uint(5)));
}

const LARGE_FILE_FILLERS: u64 = 5_001;

/// A text carrying Figma's layout versions, in a file with more entries than the large-file threshold.
fn large_file_with_versioned_text() -> Vec<u8> {
    let mut changes = document_and_page();
    changes.push(fig_node(
        (1, 2),
        (0, 1),
        "TEXT",
        "Versioned text",
        json!({"fontSize": 14.0, "textData": {"characters": "Hello"}, "fontVersion": "1", "textUserLayoutVersion": 5, "textBidiVersion": 1}),
    ));
    changes.extend((1..=LARGE_FILE_FILLERS).map(|i| rect_nc((9, i), (0, 1), "Filler")));
    real_fig_bytes(changes)
}

#[test]
fn a_large_file_text_whose_characters_were_edited_saves_with_no_font_version_or_layout_versions() {
    let input = large_file_with_versioned_text();
    let in_file = node_changes(&input);
    assert_eq!(record(&in_file, "Versioned text")["fontVersion"], json!("1"), "the input holds the key");
    assert_eq!(record(&in_file, "Versioned text")["textUserLayoutVersion"], json!(5), "the input holds the key");

    let mut doc = import_fig(&input).expect("import");
    assert!(doc.nodes.len() > 5_000, "the fixture takes the large-file path");
    let id = by_name(&doc, "Versioned text").id;
    assert!(doc.get(id).unwrap().fig_metadata.contains_key("fontVersion"), "the import keeps the key in the metadata");
    match &mut doc.get_mut(id).unwrap().kind {
        NodeKind::Text(t) => t.content = "Hello, edited".into(),
        other => panic!("expected a text, found {other:?}"),
    }

    let saved = node_changes(&export_fig(&doc).expect("export"));
    let text = record(&saved, "Versioned text");
    assert_eq!(text["textData"]["characters"], json!("Hello, edited"), "the edit is saved");
    assert_lacks(text, &["fontVersion", "textUserLayoutVersion", "textBidiVersion"], "an edited large-file text");
}

#[test]
fn a_resized_instance_saves_with_no_derived_symbol_data_layout_version() {
    let mut changes = document_and_page();
    changes.push(fig_node((1, 10), (0, 1), "SYMBOL", "Main", json!({})));
    changes.push(fig_node((1, 11), (1, 10), "ROUNDED_RECTANGLE", "Box", json!({"size": {"x": 20.0, "y": 20.0}})));
    changes.push(fig_node((1, 20), (0, 1), "INSTANCE", "Main copy", json!({"symbolData": {"symbolID": guid(1, 10)}, "derivedSymbolDataLayoutVersion": 3})));
    let input = real_fig_bytes(changes);
    assert_eq!(record(&node_changes(&input), "Main copy")["derivedSymbolDataLayoutVersion"], json!(3), "the input holds the key");

    let mut doc = import_fig(&input).expect("import");
    let id = by_name(&doc, "Main copy").id;
    assert!(doc.get(id).unwrap().fig_metadata.contains_key("derivedSymbolDataLayoutVersion"), "the import keeps the key in the metadata");
    doc.get_mut(id).unwrap().size.x += 40.0;

    let saved = node_changes(&export_fig(&doc).expect("export"));
    assert_lacks(record(&saved, "Main copy"), &["derivedSymbolDataLayoutVersion"], "a resized instance");
}

/// Three pages and 25,000 filler rectangles, so that the loader defers the third page. That page holds a main component
/// `Buttons/Button` and, inside a variant, an instance of it whose `symbolOverrides` are `overrides`.
fn deferred_file(overrides: Value) -> Vec<u8> {
    let mut changes = vec![
        fig_node((0, 0), (0, 0), "DOCUMENT", "Document", json!({})),
        fig_node((0, 1), (0, 0), "CANVAS", "Cover", json!({})),
        fig_node((0, 2), (0, 0), "CANVAS", "Page 2", json!({})),
        fig_node((0, 3), (0, 0), "CANVAS", "404 pages", json!({})),
        fig_node(BUTTON, (0, 3), "SYMBOL", "Buttons/Button", json!({})),
        fig_node(LABEL, BUTTON, "TEXT", "Text", json!({"fontSize": 14.0, "textData": {"characters": "Button CTA"}})),
        fig_node(ICON, BUTTON, "ROUNDED_RECTANGLE", "Icon", json!({})),
        fig_node(VARIANT, (0, 3), "SYMBOL", "Type=Simple 03, Breakpoint=Desktop", json!({"size": {"x": 1440.0, "y": 960.0}})),
        fig_node(INSTANCE, VARIANT, "INSTANCE", "Buttons/Button", json!({"symbolData": {"symbolID": guid(BUTTON.0, BUTTON.1), "symbolOverrides": overrides}})),
    ];
    changes.extend((0..25_000u64).map(|i| rect_nc((9, i + 1), (0, 1), "F")));
    real_fig_bytes(changes)
}

fn instance_entries(bytes: &[u8]) -> Vec<Value> {
    node_changes(bytes).into_iter().find(|c| c["guid"] == guid(INSTANCE.0, INSTANCE.1)).expect("the instance")["symbolData"]["symbolOverrides"]
        .as_array()
        .cloned()
        .expect("the instance has override entries")
}

#[test]
fn an_unopened_instances_override_never_gains_prototype_interactions_or_text_layout_versions() {
    let held_fields = json!({
        "fontVersion": "1",
        "textUserLayoutVersion": 5,
        "textBidiVersion": 1,
        "prototypeInteractions": [{"id": guid(7, 1), "event": {"interactionType": "ON_CLICK"}, "actions": []}],
    });
    let plain = json!({"guidPath": {"guids": [guid(LABEL.0, LABEL.1)]}, "textData": {"characters": "Go back"}});
    let mut carrying = plain.clone();
    for (k, v) in held_fields.as_object().unwrap() {
        carrying[k] = v.clone();
    }
    let input = deferred_file(json!([carrying]));
    let original = instance_entries(&input);
    for key in held_fields.as_object().unwrap().keys() {
        assert!(original[0].get(key).is_some(), "the input entry holds {key}");
    }

    let doc = import_fig(&input).expect("import");
    let page = doc.pages.iter().copied().find(|&p| doc.get(p).is_some_and(|n| n.name.trim() == "404 pages")).expect("page");
    assert!(doc.page_hydrator.as_ref().is_some_and(|h| !h.lock().unwrap().is_hydrated(page)), "the instance's page is not opened");

    let entries = instance_entries(&export_fig(&doc).expect("export"));
    assert_eq!(entries, instance_entries(&deferred_file(json!([plain]))), "the entry is written as it was, without the fields held back");
}

#[test]
fn the_export_schema_still_reads_the_other_fields_of_an_entry_in_a_nested_override() {
    // a nested instance override sits in `symbolOverrides` of `symbolOverrides`; held-back keys go at every depth
    let nested = json!({
        "guidPath": {"guids": [guid(LABEL.0, LABEL.1)]},
        "textData": {"characters": "Go back"},
        "symbolData": {"symbolID": guid(BUTTON.0, BUTTON.1), "symbolOverrides": [{"guidPath": {"guids": [guid(ICON.0, ICON.1)]}, "visible": false, "scrollBehavior": "SCROLLS"}]},
    });
    let input = deferred_file(json!([nested.clone()]));
    assert_eq!(instance_entries(&input)[0]["symbolData"]["symbolOverrides"][0]["scrollBehavior"], json!("SCROLLS"), "the input holds the key");

    let doc = import_fig(&input).expect("import");
    let entries = instance_entries(&export_fig(&doc).expect("export"));
    let inner = &entries[0]["symbolData"]["symbolOverrides"][0];
    assert_eq!(inner["visible"], json!(false), "the rest of the nested entry is kept");
    assert!(inner.get("scrollBehavior").is_none(), "the held-back key is left out of the nested entry");
}

#[test]
fn the_schema_an_export_embeds_holds_figmas_whole_node_change_tree() {
    // the schema chunk of an export is the whole NodeChange tree, field for field and in Figma's order
    let doc = import_fig(&large_file_with_versioned_text()).expect("import");
    let bytes = export_fig(&doc).expect("export");
    let archive = container::open(&bytes).expect("archive");
    let (schema_chunk, _) = container::read_archive_chunks(&archive).expect("chunks");
    let embedded = Schema::decode(&schema_chunk).expect("schema");
    let figma = real();
    let (own_nc, figma_nc) = (embedded.find("NodeChange").unwrap(), figma.find("NodeChange").unwrap());
    assert!(embedded.same_definition(own_nc, &figma, figma_nc), "the embedded NodeChange tree equals Figma's");
}

#[test]
fn a_nested_override_two_levels_deep_never_gains_a_held_back_field() {
    // an override of an override of an override: each level states its own text and carries fields held back
    let held = |level: u64| {
        json!({
            "textUserLayoutVersion": 5 + level,
            "fontVersion": "1",
            "prototypeInteractions": [{"id": guid(7, level), "event": {"interactionType": "ON_CLICK"}, "actions": []}],
        })
    };
    let with_held = |mut entry: Value, level: u64| {
        for (k, v) in held(level).as_object().unwrap() {
            entry[k] = v.clone();
        }
        entry
    };
    let deepest = json!({"guidPath": {"guids": [guid(ICON.0, ICON.1)]}, "visible": false});
    let middle = json!({
        "guidPath": {"guids": [guid(LABEL.0, LABEL.1)]},
        "textData": {"characters": "Go back"},
        "symbolData": {"symbolID": guid(BUTTON.0, BUTTON.1), "symbolOverrides": [with_held(deepest.clone(), 3)]},
    });
    let outer = json!({
        "guidPath": {"guids": [guid(VARIANT.0, VARIANT.1)]},
        "symbolData": {"symbolID": guid(BUTTON.0, BUTTON.1), "symbolOverrides": [with_held(middle.clone(), 2)]},
    });
    let input = deferred_file(json!([with_held(outer.clone(), 1)]));
    let in_file = instance_entries(&input);
    let level2 = &in_file[0]["symbolData"]["symbolOverrides"][0];
    let level3 = &level2["symbolData"]["symbolOverrides"][0];
    for (what, entry) in [("outer", &in_file[0]), ("middle", level2), ("deepest", level3)] {
        assert!(entry.get("textUserLayoutVersion").is_some() && entry.get("prototypeInteractions").is_some(), "the input's {what} entry holds the fields");
    }

    let doc = import_fig(&input).expect("import");
    let saved = instance_entries(&export_fig(&doc).expect("export"));
    let (outer_out, middle_out) = (&saved[0], &saved[0]["symbolData"]["symbolOverrides"][0]);
    let deepest_out = &middle_out["symbolData"]["symbolOverrides"][0];
    for (what, entry) in [("outer", outer_out), ("middle", middle_out), ("deepest", deepest_out)] {
        assert_lacks(entry, &["textUserLayoutVersion", "fontVersion", "prototypeInteractions"], what);
    }
    assert_eq!(middle_out["textData"]["characters"], json!("Go back"), "the rest of the middle entry is kept");
    assert_eq!(deepest_out["visible"], json!(false), "the rest of the deepest entry is kept");
}

#[test]
fn a_keyframe_track_passthrough_child_is_saved_without_its_held_back_keys() {
    // children of a keyframe track the importer does not model are kept as whole records and written back; a record of
    // Figma's carries fields that no writer gates yet
    let held = json!({
        "fontVersion": "1",
        "textUserLayoutVersion": 5,
        "prototypeInteractions": [{"id": guid(7, 1), "event": {"interactionType": "ON_CLICK"}, "actions": []}],
    });
    let mut preset = fig_node(
        (1, 23),
        (1, 21),
        "ANIMATION_PRESET_INSTANCE",
        "TrackPresetChild",
        json!({"timelineDuration": 400000, "pluginData": [{"pluginID": "p2", "key": "k2", "value": "v2"}]}),
    );
    for (k, v) in held.as_object().unwrap() {
        preset[k] = v.clone();
    }
    let hero = fig_node(
        (1, 20),
        (0, 1),
        "FRAME",
        "Hero",
        json!({
            "timelineDefinitions": {"entries": [{"id": guid(3, 1), "data": {"durationUs": 1000000, "defaultTimeline": true}}]},
            "parameterConsumptionMap": {"entries": [{
                "nodeField": 18,
                "variableField": "OPACITY",
                "variableData": {
                    "value": {"keyframeTrackParameterValue": {"parameters": [{
                        "type": "MANUAL",
                        "value": {"manual": {"keyframeTrackId": {"guid": guid(1, 21)}, "timelineDefId": guid(3, 1)}}
                    }]}},
                    "dataType": "KEYFRAME_TRACK_PARAMETER_DATA",
                    "resolvedDataType": "KEYFRAME_TRACK_PARAMETER_DATA"
                }
            }]}
        }),
    );
    let mut changes = document_and_page();
    changes.push(hero);
    changes.push(fig_node((1, 21), (1, 20), "KEYFRAME_TRACK", "OpacityTrack", json!({"keyframeOperation": "SET", "trackLoopMode": "NONE"})));
    changes.push(preset);

    let doc = import_fig_node_changes(&json!({"nodeChanges": changes}), "Motion").expect("import");
    let hero = by_name(&doc, "Hero");
    let kept = hero.fig_metadata.get("_motionTrackMeta").expect("the import keeps the track's data");
    for key in ["fontVersion", "textUserLayoutVersion", "prototypeInteractions"] {
        assert!(kept.contains(key), "the kept passthrough child holds {key}");
    }

    let saved = node_changes(&export_fig(&doc).expect("export"));
    let child = record(&saved, "TrackPresetChild");
    assert_eq!(child["type"], json!("ANIMATION_PRESET_INSTANCE"));
    assert_eq!(child["timelineDuration"], json!(400000), "the other keys are kept");
    assert_eq!(child["pluginData"], json!([{"pluginID": "p2", "key": "k2", "value": "v2"}]), "the other keys are kept");
    assert_lacks(child, &["fontVersion", "textUserLayoutVersion", "prototypeInteractions"], "a keyframe track passthrough child");
}
