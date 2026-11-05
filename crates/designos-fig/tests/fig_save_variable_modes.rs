//! A `.fig` save writes each collection with the modes it has, and gives every mode a GUID of its own.

mod common;

use common::fig_fixture::*;
use designos_doc::*;
use designos_fig::{export_fig, import_fig, import_fig_node_changes, import_fig_with_report};
use serde_json::{json, Value};
use std::collections::BTreeSet;

fn mode_names(doc: &Document) -> Vec<String> {
    doc.variable_collections.values().flat_map(|c| c.modes.iter().map(|m| m.name.clone())).collect()
}

fn variable_with_value_for_modes(modes: &[(u64, u64)]) -> Value {
    let entries: Vec<Value> =
        modes.iter().map(|m| json!({"modeID": guid(m.0, m.1), "variableData": {"dataType": "COLOR", "value": {"colorValue": white()}}})).collect();
    fig_node(
        (1, 7),
        (0, 1),
        "VARIABLE",
        "Bg",
        json!({"key": "bg", "variableSetID": {"guid": guid(1, 5)}, "variableResolvedType": "COLOR", "variableDataValues": {"entries": entries}}),
    )
}

/// A value that names a mode its collection never had, and one that names a mode removed from the collection.
#[test]
fn value_for_an_unknown_mode_adds_no_mode_on_fig_save() {
    value_for_a_mode_the_collection_never_had_adds_no_mode();
    value_for_a_removed_mode_adds_no_mode();
}

fn value_for_a_mode_the_collection_never_had_adds_no_mode() {
    // The value names a mode its collection does not have.
    let mut nodes = document_and_page();
    nodes.extend([
        variable_set((1, 5), "Colors", (1, 6), json!({})),
        variable_with_value_for_modes(&[(1, 6), (9, 9)]),
        fig_node((3, 1), (0, 1), "FRAME", "Banner", json!({"fillPaints": [solid(&white(), Some((1, 7)))]})),
    ]);
    let original = import_fig_node_changes(&Value::Array(nodes), "UnknownMode").unwrap();
    assert_eq!(mode_names(&original), ["Light"], "precondition: the collection has one mode");
    assert!(original.variables.values().any(|v| v.values.contains_key("9:9")), "precondition: a value names the unknown mode");

    let reopened = import_fig(&export_fig(&original).unwrap()).unwrap();
    assert_eq!(mode_names(&reopened), ["Light"], "the save adds no mode for the unknown id");
    let bg = reopened.variables.values().find(|v| v.name == "Bg").expect("Bg survives");
    let default_mode = &reopened.variable_collections[&bg.collection].default_mode;
    assert_eq!(bg.values.keys().collect::<Vec<_>>(), [default_mode], "only the value of a mode the collection has is written");
}

fn value_for_a_removed_mode_adds_no_mode() {
    let mut nodes = document_and_page();
    nodes.extend([
        fig_node(
            (1, 5),
            (0, 1),
            "VARIABLE_SET",
            "Colors",
            json!({"key": "colors", "variableSetModes": [
                {"id": guid(1, 6), "name": "Light", "sortPosition": "!"}, {"id": guid(1, 10), "name": "Dark", "sortPosition": "\""}]}),
        ),
        variable_with_value_for_modes(&[(1, 6), (1, 10)]),
    ]);
    let mut original = import_fig_node_changes(&Value::Array(nodes), "RemovedMode").unwrap();
    for c in original.variable_collections.values_mut() {
        c.modes.retain(|m| m.name != "Dark");
    }
    assert_eq!(mode_names(&original), ["Light"]);

    let reopened = import_fig(&export_fig(&original).unwrap()).unwrap();
    assert_eq!(mode_names(&reopened), ["Light"], "a save does not bring a removed mode back");
}

fn mode_guids_and_other_guids(saved: &[u8]) -> (Vec<String>, BTreeSet<String>) {
    let changes = node_changes(saved);
    let modes: Vec<String> =
        changes.iter().flat_map(|c| c.get("variableSetModes").and_then(Value::as_array).cloned().unwrap_or_default()).map(|m| guid_text(&m["id"])).collect();
    let others: BTreeSet<String> = changes.iter().map(|c| guid_text(&c["guid"])).collect();
    (modes, others)
}

#[test]
fn mode_guids_never_collide_with_other_guids_in_a_saved_fig() {
    // The file's only mode carries the same GUID as its collection.
    let mut nodes = document_and_page();
    nodes.extend([
        variable_set((1, 1), "Tokens", (1, 1), json!({})),
        color_var((1, 2), (1, 1), (1, 1), "Bg", &white(), json!({})),
        fig_node((3, 1), (0, 1), "FRAME", "Banner", json!({"fillPaints": [solid(&white(), Some((1, 2)))]})),
    ]);
    let original = import_fig_node_changes(&Value::Array(nodes), "ModeCollide").unwrap();
    let saved = export_fig(&original).unwrap();
    let (modes, others) = mode_guids_and_other_guids(&saved);
    assert_eq!(modes.len(), 1);
    assert!(modes.iter().all(|m| !others.contains(m)), "mode GUIDs {modes:?} collide with the file's other GUIDs {others:?}");
    let (reopened, report) = import_fig_with_report(&saved).unwrap();
    assert_eq!(report.missing_variable_bindings, 0, "the binding still resolves");
    assert_eq!(resolved_fill(&reopened, by_name(&reopened, "Banner").id), "#FFFFFF");

    // Studio-made modes have no GUID of their own: the ones allocated for them avoid every node's GUID.
    let mut nodes = document_and_page();
    nodes.extend([
        fig_node((3, 1), (0, 1), "FRAME", "Banner", json!({"fillPaints": [solid(&white(), None)]})),
        fig_node((3, 2), (0, 1), "FRAME", "Card", json!({"fillPaints": [solid(&white(), None)]})),
    ]);
    let mut studio = import_fig_node_changes(&Value::Array(nodes), "StudioModes").unwrap();
    studio.variable_collections.insert(
        "coll-theme".into(),
        VariableCollection {
            id: "coll-theme".into(),
            name: "Theme".into(),
            modes: vec![VariableMode { id: "light".into(), name: "Light".into() }, VariableMode { id: "dark".into(), name: "Dark".into() }],
            default_mode: "light".into(),
            variables: vec!["var-bg".into()],
            key: None,
            source_library_key: None,
            extensions: Default::default(),
        },
    );
    studio.variables.insert(
        "var-bg".into(),
        Variable {
            id: "var-bg".into(),
            name: "Bg".into(),
            key: None,
            collection: "coll-theme".into(),
            kind: VariableKind::Color,
            values: [
                ("light".into(), VariableValue::Color(Color::from_hex("#FFFFFF").unwrap())),
                ("dark".into(), VariableValue::Color(Color::from_hex("#111111").unwrap())),
            ]
            .into_iter()
            .collect(),
            description: String::new(),
            scopes: Vec::new(),
            hidden_from_publishing: false,
            code_syntax: Default::default(),
            extensions: Default::default(),
            mode_extensions: Default::default(),
        },
    );
    let (modes, others) = mode_guids_and_other_guids(&export_fig(&studio).unwrap());
    assert_eq!(modes.len(), 2);
    assert!(modes.iter().all(|m| !others.contains(m)), "mode GUIDs {modes:?} collide with the file's other GUIDs {others:?}");
    assert_eq!(modes.iter().collect::<BTreeSet<_>>().len(), 2, "each mode has its own GUID");
}
