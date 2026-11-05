use designos_doc::*;
use designos_fig::{export_fig, export_rest_json, export_variables_json, import_fig, import_rest_json, import_variables_json};

const LOCAL: &str = include_str!("fixtures/variables_local.json");
const FILE: &str = include_str!("fixtures/variables_file.json");

fn find<'a>(doc: &'a Document, name: &str) -> &'a Node {
    doc.nodes.values().find(|n| n.name == name).unwrap_or_else(|| panic!("no node named {name}"))
}

fn doc_with_variables() -> Document {
    let mut doc = import_rest_json(FILE).unwrap();
    assert_eq!(import_variables_json(&mut doc, LOCAL).unwrap(), 6);
    doc
}

#[test]
fn imports_collections_variables_and_values() {
    let doc = doc_with_variables();
    let theme = &doc.variable_collections["VariableCollectionId:1:1"];
    assert_eq!(theme.name, "Theme");
    assert_eq!(theme.key.as_deref(), Some("ck1"));
    assert_eq!(theme.modes.iter().map(|m| m.name.as_str()).collect::<Vec<_>>(), ["Light", "Dark"]);
    assert_eq!(theme.default_mode, "1:0");
    // display order follows variableIds
    assert_eq!(theme.variables[0], "VariableID:1:3");
    assert_eq!(theme.variables.len(), 5);
    let card = &doc.variables["VariableID:1:2"];
    assert_eq!(card.name, "Surface/Card");
    assert_eq!(card.kind, VariableKind::Color);
    assert_eq!(card.scopes, ["FRAME_FILL", "SHAPE_FILL"]);
    assert_eq!(card.description, "Card background");
    assert_eq!(card.values["1:0"], VariableValue::Color(Color::WHITE));
    assert_eq!(card.values["1:1"], VariableValue::Alias("VariableID:7:1".into()));
    assert_eq!(doc.variables["VariableID:1:4"].values["1:1"], VariableValue::Number(16.0));
    assert_eq!(doc.variables["VariableID:1:5"].values["1:0"], VariableValue::String("Hello".into()));
    assert_eq!(doc.variables["VariableID:1:6"].values["1:1"], VariableValue::Boolean(false));
    assert!(doc.variables["VariableID:7:1"].hidden_from_publishing);
}

#[test]
fn imports_node_bindings_and_modes() {
    let doc = doc_with_variables();
    let card = find(&doc, "Card");
    assert_eq!(card.variables.bindings["fills/0"], "VariableID:1:2");
    assert_eq!(card.variables.bindings["itemSpacing"], "VariableID:1:4");
    assert_eq!(card.variables.bindings["width"], "VariableID:1:4");
    assert_eq!(card.variables.modes["VariableCollectionId:1:1"], "1:1");
    let title = find(&doc, "Title");
    assert_eq!(title.variables.bindings["characters"], "VariableID:1:5");
    // resolved through the Dark mode inherited by the text: alias -> Primitives
    assert_eq!(doc.bound_value(card.id, "fills/0"), Some(VariableValue::Color(Color { r: 0.172, g: 0.172, b: 0.172, a: 1.0 })));
    assert_eq!(doc.bound_value(title.id, "characters"), Some(VariableValue::String("Hallo".into())));
    let r = resolved_node(&doc, title.id).unwrap();
    assert!(matches!(&r.kind, NodeKind::Text(t) if t.content == "Hallo" && t.style.font_size == 16.0));
}

#[test]
fn top_level_variables_key_is_accepted() {
    let mut file: serde_json::Value = serde_json::from_str(FILE).unwrap();
    let local: serde_json::Value = serde_json::from_str(LOCAL).unwrap();
    file["variables"] = local.clone();
    let doc = import_rest_json(&file.to_string()).unwrap();
    assert_eq!(doc.variables.len(), 6);
    // the bare meta object works too
    file["variables"] = local["meta"].clone();
    assert_eq!(import_rest_json(&file.to_string()).unwrap().variables.len(), 6);
    assert!(import_variables_json(&mut Document::new("x"), "{\"nope\": 1}").is_err());
}

#[test]
fn round_trips_through_export() {
    let doc = doc_with_variables();
    let json = export_rest_json(&doc);
    assert!(json["variables"]["variables"].is_object());
    let card_json = &json["document"]["children"][0]["children"][0];
    assert_eq!(card_json["boundVariables"]["fills"][0]["id"], "VariableID:1:2");
    assert_eq!(card_json["boundVariables"]["size"]["x"]["id"], "VariableID:1:4");
    assert_eq!(card_json["explicitVariableModes"]["VariableCollectionId:1:1"], "1:1");
    let back = import_rest_json(&json.to_string()).unwrap();
    assert_eq!(back.variables, doc.variables);
    assert_eq!(back.variable_collections, doc.variable_collections);
    assert_eq!(find(&back, "Card").variables, find(&doc, "Card").variables);
    assert_eq!(find(&back, "Title").variables, find(&doc, "Title").variables);

    // the standalone endpoint shape also round-trips
    let mut other = Document::new("o");
    let resp = export_variables_json(&doc);
    assert_eq!(resp["status"], 200);
    import_variables_json(&mut other, &resp.to_string()).unwrap();
    assert_eq!(other.variables, doc.variables);
    assert_eq!(other.variable_collections, doc.variable_collections);
}

#[test]
fn files_without_variables_export_without_the_key() {
    let doc = import_rest_json(FILE).unwrap();
    assert!(export_rest_json(&doc).get("variables").is_none());
    assert!(doc.variables.is_empty());
}

#[test]
fn fig_round_trip_preserves_variables_used_by_nodes() {
    let doc = doc_with_variables();
    let bytes = export_fig(&doc).expect("export .fig");
    let back = import_fig(&bytes).expect("re-import .fig");

    assert_eq!(back.variable_collections.len(), doc.variable_collections.len());
    assert_eq!(back.variables.len(), doc.variables.len());
    for (id, collection) in &doc.variable_collections {
        let actual = back.variable_collections.get(id).unwrap_or_else(|| panic!("collection {id} disappeared"));
        assert_eq!(actual.modes, collection.modes, "collection {id} modes changed");
        assert_eq!(actual.default_mode, collection.default_mode, "collection {id} default mode changed");
        assert_eq!(actual.variables.len(), collection.variables.len(), "collection {id} variable count changed");
        assert_eq!(
            actual.variables.iter().collect::<std::collections::BTreeSet<_>>(),
            collection.variables.iter().collect::<std::collections::BTreeSet<_>>(),
            "collection {id} variable membership changed"
        );
    }
    for (id, variable) in &doc.variables {
        let actual = back.variables.get(id).unwrap_or_else(|| panic!("variable {id} disappeared"));
        assert_eq!(actual.id, variable.id, "variable {id} id changed");
        assert_eq!(actual.collection, variable.collection, "variable {id} collection changed");
        assert_eq!(actual.kind, variable.kind, "variable {id} type changed");
        assert_eq!(actual.values, variable.values, "variable {id} values changed");
        assert_eq!(actual.key, variable.key, "variable {id} key changed");
    }
    for node in doc.nodes.values() {
        if node.variables.bindings.is_empty() {
            continue;
        }
        let reimported = back.nodes.values().find(|candidate| candidate.name == node.name).unwrap_or_else(|| panic!("node {} disappeared", node.name));
        assert_eq!(reimported.variables.bindings, node.variables.bindings, "bindings changed for {}", node.name);
    }
}
