//! A `.pen` file has no deleted or hidden flag for a variable, so a soft-deleted variable is written as an ordinary one
//! while something still needs it, so the binding survives, and is left out otherwise.

use designos_doc::*;
use designos_fig::{export_pen_json, import_pen_json};
use serde_json::{json, Value};

const PEN: &str = r##"{
  "version": "2.21",
  "variables": {
    "Text/Muted": {"type": "color", "value": "#999999"},
    "Text/Gone": {"type": "color", "value": "#777777"},
    "Text/Strong": {"type": "color", "value": "#111111"},
    "Base/Slate": {"type": "color", "value": "#334455"},
    "Text/Alias": {"type": "color", "value": "$Base/Slate"}
  },
  "children": [
    {"id": "card", "type": "frame", "x": 0, "y": 0, "width": 100, "height": 50, "fill": "$Text/Muted"}
  ]
}"##;

fn mark(doc: &mut Document, id: &str) {
    doc.variables.get_mut(id).unwrap_or_else(|| panic!("variable {id}")).extensions.insert(SOFT_DELETED_EXTENSION_KEY.into(), true.into());
}

fn written_variables(doc: &Document) -> serde_json::Map<String, Value> {
    let exported: Value = serde_json::from_str(&export_pen_json(doc).unwrap()).unwrap();
    exported.get("variables").and_then(Value::as_object).cloned().unwrap_or_default()
}

fn card_fill(doc: &Document) -> Option<Color> {
    let card = doc.nodes.values().find(|n| n.source_id.as_deref() == Some("card")).expect("card");
    match &resolve_node(doc, card).fills.first()?.kind {
        PaintKind::Solid(c) => Some(*c),
        _ => None,
    }
}

#[test]
fn a_soft_deleted_variable_a_layer_still_binds_is_written_so_the_binding_survives() {
    let mut doc = import_pen_json(PEN).unwrap();
    assert_eq!(card_fill(&doc).map(|c| c.to_hex()), Some("999999".into()), "precondition: the card is bound to Text/Muted");
    for id in ["Text/Muted", "Text/Gone", "Base/Slate"] {
        mark(&mut doc, id);
    }

    let written = written_variables(&doc);
    let mut names: Vec<&str> = written.keys().map(String::as_str).collect();
    names.sort_unstable();
    assert_eq!(
        names,
        ["Base/Slate", "Text/Alias", "Text/Muted", "Text/Strong"],
        "the bound deleted variable and the deleted target of a live alias are written; the unreferenced deleted one is not"
    );
    assert_eq!(written["Text/Muted"], json!({"type": "color", "value": "#999999"}), "an ordinary variable object: the format has no flag to set");
    for (name, object) in &written {
        let mut keys: Vec<&str> = object.as_object().unwrap().keys().map(String::as_str).collect();
        keys.sort_unstable();
        assert!(keys.iter().all(|k| ["type", "value", "description"].contains(k)), "{name} carries only the fields the format defines: {keys:?}");
    }

    let reopened = import_pen_json(&export_pen_json(&doc).unwrap()).unwrap();
    assert_eq!(card_fill(&reopened).map(|c| c.to_hex()), Some("999999".into()), "the binding survives the save");
    assert!(reopened.variables.contains_key("Text/Muted") && !reopened.variables.contains_key("Text/Gone"));
    assert_eq!(reopened.variables["Text/Alias"].values.values().next(), Some(&VariableValue::Alias("Base/Slate".into())), "the alias still has its target");
}

#[test]
fn a_soft_deleted_variable_does_not_overwrite_the_live_variable_that_holds_its_name() {
    let mut doc = import_pen_json(PEN).unwrap();
    mark(&mut doc, "Text/Muted");
    // A live variable of the same name, under another id: the file keys variables by name.
    let mut live = doc.variables["Text/Strong"].clone();
    live.id = "live-muted".into();
    live.name = "Text/Muted".into();
    live.values = [(doc.variable_collections.values().next().unwrap().default_mode.clone(), VariableValue::Color(Color::from_hex("#666666").unwrap()))].into();
    let collection = live.collection.clone();
    // Listed before the deleted variable: written in that order, the later one would replace the earlier.
    doc.variable_collections.get_mut(&collection).unwrap().variables.insert(0, "live-muted".into());
    doc.variables.insert("live-muted".into(), live);

    let written = written_variables(&doc);
    assert_eq!(written["Text/Muted"]["value"], json!("#666666"), "the live variable's definition is the one written");
    assert_eq!(written.keys().filter(|k| k.as_str() == "Text/Muted").count(), 1);
}

#[test]
fn nothing_is_written_for_variables_and_collections_that_are_deleted_and_unreferenced() {
    let raw = r##"{
      "version": "2.21",
      "themes": {"mode": ["Light", "Dark"]},
      "variables": {"Old/Bg": {"type": "color", "value": [{"theme": {"mode": "Light"}, "value": "#FFFFFF"}, {"theme": {"mode": "Dark"}, "value": "#000000"}]}},
      "children": [{"id": "card", "type": "frame", "x": 0, "y": 0, "width": 10, "height": 10}]
    }"##;
    let mut doc = import_pen_json(raw).unwrap();
    mark(&mut doc, "Old/Bg");
    for c in doc.variable_collections.values_mut() {
        c.extensions.insert(SOFT_DELETED_EXTENSION_KEY.into(), true.into());
    }
    let exported: Value = serde_json::from_str(&export_pen_json(&doc).unwrap()).unwrap();
    assert!(exported.get("variables").is_none() && exported.get("themes").is_none(), "{exported}");

    // Bound again, the deleted variable and the modes it needs come back.
    let card = doc.nodes.values().find(|n| n.source_id.as_deref() == Some("card")).unwrap().id;
    doc.get_mut(card).unwrap().variables.bindings.insert("fills/0".into(), "Old/Bg".into());
    let exported: Value = serde_json::from_str(&export_pen_json(&doc).unwrap()).unwrap();
    assert!(exported["variables"].get("Old/Bg").is_some(), "{exported}");
    assert_eq!(exported["themes"]["mode"], json!(["Light", "Dark"]));
}
