//! A variable of a subscribed library collection is exported as `remote: true`, one of a local collection as
//! `remote: false`, whichever way the file reaches the export: a `.fig` read, a `.fig` save and reopen, or the
//! variables export the command line writes.

mod common;

use common::fig_fixture::*;
use designos_doc::Document;
use designos_fig::{
    export_fig, export_variables, export_variables_json, import_fig_node_changes, import_fig_with_report, import_variables_value, VariablesFormat,
};
use serde_json::{json, Value};

const LOCAL_SET: (u64, u64) = (1, 5);
const LIBRARY_SET: (u64, u64) = (1, 20);
const LOCAL_MODE: (u64, u64) = (1, 6);
const LIBRARY_MODE: (u64, u64) = (1, 21);
const LIBRARY_KEY: &str = "lk-83c0";

/// A local collection with one variable, and a subscribed library collection with two: one that names its set by
/// guid and one that names it by the library asset key, the way a kit file refers to its library variables.
fn kit_like_file() -> Document {
    let by_asset_key = color_var(
        (1, 32),
        LIBRARY_SET,
        LIBRARY_MODE,
        "Brand/Secondary",
        &blue(),
        json!({"variableSetID": {"assetRef": {"key": format!("set-{}-{}", LIBRARY_SET.0, LIBRARY_SET.1)}}}),
    );
    let mut nodes = document_and_page();
    nodes.extend([
        variable_set(LOCAL_SET, "Theme", LOCAL_MODE, json!({})),
        variable_set(LIBRARY_SET, "Brand", LIBRARY_MODE, json!({"sourceLibraryKey": LIBRARY_KEY})),
        color_var((1, 30), LOCAL_SET, LOCAL_MODE, "Surface", &white(), json!({})),
        color_var((1, 31), LIBRARY_SET, LIBRARY_MODE, "Brand/Primary", &blue(), json!({})),
        by_asset_key,
    ]);
    import_fig_node_changes(&Value::Array(nodes), "KitLike").unwrap()
}

/// `(variable name, flag on the variable, flag on its collection)` for every variable of a REST-shaped export.
fn flags(payload: &Value) -> Vec<(String, bool, bool)> {
    let meta = &payload["meta"];
    let mut out: Vec<(String, bool, bool)> = meta["variables"]
        .as_object()
        .expect("variables")
        .values()
        .map(|v| {
            let collection = &meta["variableCollections"][v["variableCollectionId"].as_str().expect("collection id")];
            (v["name"].as_str().unwrap().to_string(), v["remote"].as_bool().expect("variable flag"), collection["remote"].as_bool().expect("collection flag"))
        })
        .collect();
    out.sort();
    out
}

fn expected() -> Vec<(String, bool, bool)> {
    vec![("Brand/Primary".into(), true, true), ("Brand/Secondary".into(), true, true), ("Surface".into(), false, false)]
}

#[test]
fn a_fig_read_keeps_the_library_collection_the_flag_is_derived_from() {
    let doc = kit_like_file();
    let library = doc.variable_collections.values().find(|c| c.name == "Brand").expect("library collection");
    assert!(library.is_library(), "the read keeps the subscribed collection marked");
    for name in ["Brand/Primary", "Brand/Secondary"] {
        let v = doc.variables.values().find(|v| v.name == name).unwrap();
        assert_eq!(v.collection, library.id, "{name} belongs to the library collection, however the file named its set");
    }
    let local = doc.variables.values().find(|v| v.name == "Surface").unwrap();
    assert!(!doc.variable_collections[&local.collection].is_library());
}

#[test]
fn rest_export_flags_library_variables_remote_and_local_ones_not() {
    let payload = export_variables_json(&kit_like_file());
    assert_eq!(flags(&payload), expected());
}

#[test]
fn rest_export_flag_survives_a_rest_round_trip() {
    let first = export_variables_json(&kit_like_file());
    let mut doc = Document::new("again");
    import_variables_value(&mut doc, &first);
    assert_eq!(flags(&export_variables_json(&doc)), expected());
}

#[test]
fn the_command_line_export_bundle_carries_the_flag() {
    // `designos FILE.fig --export-variables OUT.json --format figma-rest` writes the one file of this bundle.
    let doc = kit_like_file();
    for format in [VariablesFormat::FigmaRest, VariablesFormat::Auto] {
        let bundle = export_variables(&doc, format).unwrap();
        assert_eq!(bundle.files.len(), 1, "{format:?}");
        let written: Value = serde_json::from_slice(&bundle.files[0].1).unwrap();
        assert_eq!(flags(&written), expected(), "{format:?}");
    }
}

#[test]
fn a_fig_save_and_reopen_keeps_the_flag_on_the_variable_and_on_its_collection() {
    let mut doc = kit_like_file();
    for round in 1..=2 {
        let (reopened, _) = import_fig_with_report(&export_fig(&doc).unwrap()).unwrap();
        let library = reopened.variable_collections.values().find(|c| c.name == "Brand").expect("library collection");
        assert!(library.is_library(), "round {round}: the collection is still subscribed");
        assert_eq!(library.source_library_key.as_deref(), Some(LIBRARY_KEY), "round {round}");
        assert_eq!(flags(&export_variables_json(&reopened)), expected(), "round {round}");
        doc = reopened;
    }
}

#[test]
fn a_variable_in_a_local_collection_alone_exports_not_remote() {
    let mut nodes = document_and_page();
    nodes.extend([variable_set(LOCAL_SET, "Theme", LOCAL_MODE, json!({})), color_var((1, 30), LOCAL_SET, LOCAL_MODE, "Surface", &white(), json!({}))]);
    let doc = import_fig_node_changes(&Value::Array(nodes), "LocalOnly").unwrap();
    assert_eq!(flags(&export_variables_json(&doc)), vec![("Surface".to_string(), false, false)]);
}

#[test]
fn the_figma_native_export_has_no_remote_field_to_carry() {
    // The native token files hold `$value`, `$type` and `com.figma.*` extensions, and none of them names a library:
    // there is no field to flag, so a subscribed collection is written like a local one.
    let mut nodes = document_and_page();
    nodes.extend([
        variable_set(LIBRARY_SET, "Brand", LIBRARY_MODE, json!({"sourceLibraryKey": LIBRARY_KEY})),
        color_var((1, 31), LIBRARY_SET, LIBRARY_MODE, "Primary", &blue(), json!({})),
    ]);
    let doc = import_fig_node_changes(&Value::Array(nodes), "NativeLibrary").unwrap();
    assert!(doc.variable_collections.values().all(|c| c.is_library()));
    let bundle = export_variables(&doc, VariablesFormat::FigmaNative).unwrap();
    assert_eq!(bundle.files.len(), 1);
    let text = String::from_utf8(bundle.files[0].1.clone()).unwrap();
    assert!(text.contains("\"$value\""), "the library token is exported: {text}");
    for word in ["remote", "sourceLibraryKey", "source_library_key", LIBRARY_KEY] {
        assert!(!text.contains(word), "figma-native has no place for {word}: {text}");
    }
}
