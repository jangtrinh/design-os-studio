//! A variable the file has deleted while layers, styles or other variables still use it stays in the document so
//! those references resolve, but it is not live: it is not listed under its collection, and a `.fig` save writes
//! it back as deleted instead of bringing it back to life.

mod common;

use common::fig_fixture::*;
use designos_doc::*;
use designos_fig::{
    export_fig, export_variables, export_variables_json, import_fig, import_fig_node_changes, import_fig_with_report, import_variables, import_variables_value,
    ImportVariablesOptions, VariablesFormat,
};
use serde_json::{json, Value};

/// The stored key of the soft-deleted marker is part of the `.designos` file format, so it is spelled out here.
const MARKER: &str = "org.designos.softDeleted";

const SET: (u64, u64) = (1, 5);
const MODE: (u64, u64) = (1, 6);

fn is_marked(v: &Variable) -> bool {
    v.extensions.get(MARKER) == Some(&Value::Bool(true))
}

fn variable_named<'a>(doc: &'a Document, name: &str) -> Vec<&'a Variable> {
    doc.variables.values().filter(|v| v.name == name).collect()
}

fn deleted() -> Value {
    json!({"isSoftDeleted": true})
}

/// A live `Text/Primary`, a deleted `Text/Muted` that a layer binds and that aliases a deleted `Slate`, and a deleted
/// `Unused` nothing refers to.
fn deleted_but_referenced_fixture() -> Value {
    let (grey, slate) = (rgba(0.6, 0.6, 0.6), rgba(0.2, 0.25, 0.3));
    let mut nodes = document_and_page();
    nodes.extend([
        variable_set(SET, "Colors", MODE, json!({})),
        color_var((1, 20), SET, MODE, "Text/Primary", &white(), json!({})),
        color_var((1, 22), SET, MODE, "Slate", &slate, deleted()),
        fig_node(
            (1, 21),
            (0, 1),
            "VARIABLE",
            "Text/Muted",
            json!({
                "key": "key-1-21", "isSoftDeleted": true, "variableSetID": {"guid": guid(SET.0, SET.1)}, "variableResolvedType": "COLOR",
                "variableDataValues": {"entries": [{"modeID": guid(MODE.0, MODE.1), "variableData": var_alias(1, 22)}]}
            }),
        ),
        color_var((1, 23), SET, MODE, "Unused", &grey, deleted()),
        fig_node((3, 1), (0, 1), "FRAME", "Banner", json!({"fillPaints": [solid(&slate, Some((1, 21)))]})),
    ]);
    Value::Array(nodes)
}

#[test]
fn soft_deleted_referenced_variable_resolves_but_is_not_listed() {
    let doc = import_fig_node_changes(&deleted_but_referenced_fixture(), "DeletedButReferenced").unwrap();

    // The banner's binding and the alias behind it resolve through the deleted variables.
    let banner = by_name(&doc, "Banner");
    assert_eq!(banner.variables.bindings.get("fills/0").map(String::as_str), Some("VariableID:1:21"));
    assert_eq!(resolved_fill(&doc, banner.id), "#33404D", "the binding resolves through the deleted variable and its deleted alias target");

    // Both deleted variables the file still uses are kept and marked, the unused one is not kept, the live one is not marked.
    let muted = variable_named(&doc, "Text/Muted");
    let slate = variable_named(&doc, "Slate");
    assert_eq!((muted.len(), slate.len()), (1, 1));
    assert!(is_marked(muted[0]) && is_marked(slate[0]), "deleted variables carry the marker");
    assert!(variable_named(&doc, "Unused").is_empty(), "a deleted variable nothing uses is not kept");
    let primary = variable_named(&doc, "Text/Primary");
    assert!(!is_marked(primary[0]), "a variable that was live keeps no marker");

    // Only the live variable is listed under its collection.
    let coll = &doc.variable_collections["VariableCollectionId:1:5"];
    assert_eq!(coll.variables, ["VariableID:1:20"], "the collection lists live variables only");
    assert!(!coll.extensions.contains_key(MARKER), "the collection is live");
}

#[test]
fn deleted_and_live_variables_with_the_same_name_list_once() {
    let (live, deleted_grey) = (rgba(0.2, 0.2, 0.2), rgba(0.6, 0.6, 0.6));
    let mut nodes = document_and_page();
    nodes.extend([
        variable_set(SET, "Colors", MODE, json!({})),
        color_var((1, 20), SET, MODE, "Text/Muted", &live, json!({})),
        color_var((1, 21), SET, MODE, "Text/Muted", &deleted_grey, deleted()),
        fig_node((3, 1), (0, 1), "FRAME", "Banner", json!({"fillPaints": [solid(&deleted_grey, Some((1, 21)))]})),
    ]);
    let original = import_fig_node_changes(&Value::Array(nodes), "SameName").unwrap();
    let (reopened, report) = import_fig_with_report(&export_fig(&original).unwrap()).unwrap();

    for (stage, doc) in [("import", &original), ("reopen", &reopened)] {
        let coll = doc.variable_collections.values().next().expect("one collection");
        let listed: Vec<&Variable> = coll.variables.iter().map(|id| &doc.variables[id]).filter(|v| v.name == "Text/Muted").collect();
        assert_eq!(listed.len(), 1, "{stage}: the name is listed once");
        assert_eq!(listed[0].id, "VariableID:1:20", "{stage}: the live variable is the one listed");
        assert!(!is_marked(listed[0]), "{stage}: the listed variable is live");
        // The deleted twin stays, so the banner keeps its colour.
        assert_eq!(variable_named(doc, "Text/Muted").len(), 2, "{stage}: the deleted variable is still in the document");
        assert_eq!(resolved_fill(doc, by_name(doc, "Banner").id), "#999999", "{stage}: the banner resolves through the deleted variable");
    }
    assert_eq!(report.missing_variable_bindings, 0);
}

fn written_variable<'a>(changes: &'a [Value], name: &str) -> Vec<&'a Value> {
    changes.iter().filter(|c| c["type"] == "VARIABLE" && c["name"] == name).collect()
}

#[test]
fn soft_deleted_variable_is_written_back_as_soft_deleted() {
    let original = import_fig_node_changes(&deleted_but_referenced_fixture(), "WriteBack").unwrap();
    let saved = export_fig(&original).unwrap();
    let changes = node_changes(&saved);
    let flag = |name: &str| {
        let written = written_variable(&changes, name);
        assert_eq!(written.len(), 1, "{name} is written once");
        written[0].get("isSoftDeleted").and_then(Value::as_bool).unwrap_or(false)
    };
    assert!(flag("Text/Muted") && flag("Slate"), "deleted variables are written as deleted");
    assert!(!flag("Text/Primary"), "a live variable is not written as deleted");
    assert!(changes.iter().filter(|c| c["type"] == "VARIABLE_SET").all(|c| !c.get("isSoftDeleted").and_then(Value::as_bool).unwrap_or(false)));

    let (reopened, report) = import_fig_with_report(&saved).unwrap();
    assert_eq!(report.missing_variable_bindings, 0, "the layers still find their variables");
    assert!(variable_named(&reopened, "Text/Muted").iter().all(|v| is_marked(v)), "the reopened file keeps the variable deleted");
    assert!(variable_named(&reopened, "Text/Primary").iter().all(|v| !is_marked(v)));
    let coll = reopened.variable_collections.values().next().unwrap();
    assert_eq!(coll.variables.iter().map(|id| reopened.variables[id].name.as_str()).collect::<Vec<_>>(), ["Text/Primary"]);
    assert_eq!(resolved_fill(&reopened, by_name(&reopened, "Banner").id), "#33404D");

    // A variable the Studio deleted from the document is not in the file at all: nothing to write back.
    let again = node_changes(&export_fig(&reopened).unwrap());
    assert_eq!(written_variable(&again, "Text/Muted").len(), 1);
    assert_eq!(written_variable(&again, "Text/Muted")[0].get("isSoftDeleted").and_then(Value::as_bool), Some(true), "a second save keeps it deleted");
}

#[test]
fn soft_deleted_collection_is_written_back_as_soft_deleted() {
    let grey = rgba(0.6, 0.6, 0.6);
    let mut nodes = document_and_page();
    nodes.extend([
        variable_set(SET, "Colors", MODE, json!({})),
        color_var((1, 20), SET, MODE, "Text/Primary", &white(), json!({})),
        variable_set((1, 30), "Retired", (1, 31), deleted()),
        color_var((1, 32), (1, 30), (1, 31), "Old/Grey", &grey, deleted()),
        fig_node((3, 1), (0, 1), "FRAME", "Banner", json!({"fillPaints": [solid(&grey, Some((1, 32)))]})),
    ]);
    let original = import_fig_node_changes(&Value::Array(nodes), "DeletedSet").unwrap();
    let retired = original.variable_collections.values().find(|c| c.name == "Retired").expect("the set the deleted variable lives in is kept");
    assert_eq!(retired.extensions.get(MARKER), Some(&Value::Bool(true)));
    assert!(retired.variables.is_empty(), "a collection that is itself deleted lists nothing");

    let saved = export_fig(&original).unwrap();
    let sets: Vec<(String, bool)> = node_changes(&saved)
        .iter()
        .filter(|c| c["type"] == "VARIABLE_SET")
        .map(|c| (c["name"].as_str().unwrap_or("").to_string(), c.get("isSoftDeleted").and_then(Value::as_bool).unwrap_or(false)))
        .collect();
    assert!(sets.contains(&("Retired".into(), true)) && sets.contains(&("Colors".into(), false)), "set states are written as they were read: {sets:?}");
    let (reopened, report) = import_fig_with_report(&saved).unwrap();
    assert_eq!(report.missing_variable_bindings, 0);
    assert_eq!(reopened.variable_collections.values().filter(|c| c.extensions.get(MARKER) == Some(&Value::Bool(true))).count(), 1);
}

#[test]
fn rest_variables_flag_soft_deleted_variables_and_list_live_ones_only() {
    let original = import_fig_node_changes(&deleted_but_referenced_fixture(), "RestShape").unwrap();
    let meta = export_variables_json(&original)["meta"].clone();
    let muted = &meta["variables"]["VariableID:1:21"];
    assert_eq!(muted["deletedButReferenced"], json!(true), "Figma's own spelling for a deleted variable that is still referenced");
    assert!(meta["variables"]["VariableID:1:20"].get("deletedButReferenced").is_none_or(|v| v == &json!(false)));
    assert_eq!(meta["variableCollections"]["VariableCollectionId:1:5"]["variableIds"], json!(["VariableID:1:20"]));

    // The same payload read back marks the variable and keeps it out of the list, even if the list names it.
    let mut doc = Document::new("rest");
    let mut payload = meta.clone();
    payload["variableCollections"]["VariableCollectionId:1:5"]["variableIds"] = json!(["VariableID:1:20", "VariableID:1:21"]);
    assert_eq!(import_variables_value(&mut doc, &json!({ "meta": payload })), 3);
    assert!(is_marked(&doc.variables["VariableID:1:21"]));
    assert_eq!(doc.variable_collections["VariableCollectionId:1:5"].variables, ["VariableID:1:20"]);
}

#[test]
fn a_saved_file_without_soft_deleted_variables_writes_no_deleted_flag() {
    let mut nodes = document_and_page();
    nodes.extend([variable_set(SET, "Colors", MODE, json!({})), color_var((1, 20), SET, MODE, "Text/Primary", &white(), json!({}))]);
    let doc = import_fig_node_changes(&Value::Array(nodes), "AllLive").unwrap();
    let changes = node_changes(&export_fig(&doc).unwrap());
    assert!(changes.iter().all(|c| c.get("isSoftDeleted").is_none()), "no entry is written as deleted");
    assert!(import_fig(&export_fig(&doc).unwrap()).unwrap().variables.values().all(|v| !is_marked(v)));
}

fn bundle_text(doc: &Document, format: VariablesFormat) -> (String, usize) {
    let bundle = export_variables(doc, format).expect("export tokens");
    let text = bundle.files.iter().map(|(name, bytes)| format!("{name}\n{}", String::from_utf8_lossy(bytes))).collect::<Vec<_>>().join("\n");
    (text, bundle.omitted_extensions.len())
}

#[test]
fn token_export_leaves_out_soft_deleted_variables_and_collections() {
    let grey = rgba(0.6, 0.6, 0.6);
    let mut nodes = document_and_page();
    nodes.extend([
        variable_set(SET, "Colors", MODE, json!({})),
        color_var((1, 20), SET, MODE, "Text/Primary", &white(), json!({})),
        color_var((1, 21), SET, MODE, "Text/Faded", &grey, deleted()),
        variable_set((1, 30), "Retired", (1, 31), deleted()),
        color_var((1, 32), (1, 30), (1, 31), "Old/Grey", &grey, deleted()),
        fig_node((3, 1), (0, 1), "FRAME", "Banner", json!({"fillPaints": [solid(&grey, Some((1, 21))), solid(&grey, Some((1, 32)))]})),
    ]);
    let doc = import_fig_node_changes(&Value::Array(nodes), "TokenExport").unwrap();
    assert_eq!(doc.variables.len(), 3, "precondition: both deleted variables are kept for the banner");
    for format in [VariablesFormat::Dtcg, VariablesFormat::FigmaNative] {
        let (text, omitted) = bundle_text(&doc, format);
        assert!(text.contains("Primary"), "{format:?}: the live token is exported");
        for gone in ["Faded", "Old", "Grey", "Retired"] {
            assert!(!text.contains(gone), "{format:?}: the soft-deleted `{gone}` is not a token");
        }
        assert_eq!(omitted, 0, "{format:?}: the soft-deleted marker is not reported as an omitted extension");
    }
}

#[test]
fn token_import_merges_into_the_live_variable_not_the_deleted_one_of_the_same_name() {
    let (live, gone) = (rgba(0.2, 0.2, 0.2), rgba(0.6, 0.6, 0.6));
    let mut nodes = document_and_page();
    nodes.extend([
        variable_set(SET, "Colors", MODE, json!({})),
        // The deleted variable sorts first, so a name lookup that does not skip it would pick it.
        color_var((1, 19), SET, MODE, "Text/Muted", &gone, deleted()),
        color_var((1, 20), SET, MODE, "Text/Muted", &live, json!({})),
        fig_node((3, 1), (0, 1), "FRAME", "Banner", json!({"fillPaints": [solid(&gone, Some((1, 19)))]})),
    ]);
    let mut doc = import_fig_node_changes(&Value::Array(nodes), "TokenImport").unwrap();
    let tokens = r##"{"Text": {"Muted": {"$type": "color", "$value": "#112233"}}}"##;
    let options = ImportVariablesOptions { collection_name: Some("Colors".into()), mode_name: Some("Light".into()), ..Default::default() };

    let before = doc.variables.len();
    let report = import_variables(&mut doc, tokens, VariablesFormat::Dtcg, options).expect("import tokens");
    assert_eq!((report.added, report.updated), (0, 1), "the token updates a variable and adds none: {report:?}");
    assert_eq!(doc.variables.len(), before, "the token updates a variable; it adds none");
    let color_of = |id: &str| match doc.variables[id].values.values().next() {
        Some(VariableValue::Color(c)) => format!("#{:02X}{:02X}{:02X}", (c.r * 255.0).round() as u8, (c.g * 255.0).round() as u8, (c.b * 255.0).round() as u8),
        other => panic!("not a colour: {other:?}"),
    };
    assert_eq!(color_of("VariableID:1:20"), "#112233", "the live variable takes the new value");
    assert_eq!(color_of("VariableID:1:19"), "#999999", "the soft-deleted variable keeps the value its layers use");
}

#[test]
fn a_variable_hidden_from_publishing_stays_hidden_through_a_fig_round_trip() {
    let grey = rgba(0.6, 0.6, 0.6);
    let mut nodes = document_and_page();
    nodes.extend([
        variable_set(SET, "Colors", MODE, json!({})),
        color_var((1, 20), SET, MODE, "Text/Primary", &white(), json!({"isPublishable": true})),
        color_var((1, 21), SET, MODE, "Internal/Grey", &grey, json!({"isPublishable": false})),
        color_var((1, 22), SET, MODE, "Old/Grey", &grey, json!({"isPublishable": false, "isSoftDeleted": true})),
        color_var((1, 23), SET, MODE, "Old/White", &white(), json!({"isPublishable": true, "isSoftDeleted": true})),
        fig_node((3, 1), (0, 1), "FRAME", "Banner", json!({"fillPaints": [solid(&grey, Some((1, 22))), solid(&white(), Some((1, 23)))]})),
    ]);
    let original = import_fig_node_changes(&Value::Array(nodes), "Publish").unwrap();
    let hidden = |doc: &Document| -> Vec<(String, bool)> { doc.variables.values().map(|v| (v.name.clone(), v.hidden_from_publishing)).collect() };
    let expected =
        vec![("Internal/Grey".to_string(), true), ("Old/Grey".to_string(), true), ("Old/White".to_string(), false), ("Text/Primary".to_string(), false)];
    let mut before = hidden(&original);
    before.sort();
    assert_eq!(before, expected, "the file's publish switch is imported, deleted variables included");

    let saved = export_fig(&original).unwrap();
    let written: Vec<(String, Option<bool>)> = {
        let mut w: Vec<_> = node_changes(&saved)
            .iter()
            .filter(|c| c["type"] == "VARIABLE")
            .map(|c| (c["name"].as_str().unwrap_or("").to_string(), c.get("isPublishable").and_then(Value::as_bool)))
            .collect();
        w.sort();
        w
    };
    assert_eq!(
        written,
        vec![
            ("Internal/Grey".to_string(), Some(false)),
            ("Old/Grey".to_string(), Some(false)),
            ("Old/White".to_string(), None),
            ("Text/Primary".to_string(), None)
        ],
        "a hidden variable writes the switch off; the others write nothing"
    );
    let mut after = hidden(&import_fig(&saved).unwrap());
    after.sort();
    assert_eq!(after, expected, "the switch survives a save and reopen");
}
