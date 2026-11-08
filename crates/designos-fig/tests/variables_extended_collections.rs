//! Extended variable collections (a parent link, a mode mapping and per-variable overrides) survive a `.fig`
//! export and re-import, and the REST variables payload carries them in Figma's documented fields.

use designos_doc::*;
use designos_fig::{export_fig, export_variables_json, import_fig, import_variables_value};
use serde_json::json;

const PARENT: &str = "VariableCollectionId:10:1";
const EXT: &str = "VariableCollectionId:10:2";
const BG: &str = "VariableID:10:5";

fn color(r: u8, g: u8, b: u8) -> VariableValue {
    VariableValue::Color(Color::rgb(r, g, b))
}

fn collection(id: &str, name: &str, modes: &[(&str, &str)]) -> VariableCollection {
    VariableCollection {
        id: id.into(),
        name: name.into(),
        modes: modes.iter().map(|(i, n)| VariableMode { id: (*i).into(), name: (*n).into() }).collect(),
        default_mode: modes[0].0.into(),
        variables: Vec::new(),
        key: None,
        source_library_key: None,
        extensions: Default::default(),
    }
}

/// Theme (Light/Dark) with one colour variable, and Brand extending it with its own Light/Dark and one override.
fn extended_doc(parent_library: Option<&str>) -> Document {
    let mut doc = Document::new("Consumer");
    doc.add_page("Page 1");
    let mut parent = collection(PARENT, "Theme", &[("10:0", "Light"), ("10:3", "Dark")]);
    parent.variables = vec![BG.into()];
    parent.set_extendable(true);
    if let Some(lib) = parent_library {
        parent.key = Some("SET_THEME".into());
        parent.source_library_key = Some(lib.into());
    }
    let mut brand = collection(EXT, "Brand", &[("10:4", "Light"), ("10:6", "Dark")]);
    brand.set_extension(&CollectionExtension {
        parent: ParentCollection { collection: PARENT.into(), set_key: parent_library.map(|_| "SET_THEME".to_string()) },
        mode_map: [("10:4".to_string(), "10:0".to_string()), ("10:6".to_string(), "10:3".to_string())].into(),
        overrides: [(BG.to_string(), [("10:6".to_string(), color(255, 0, 0))].into())].into(),
    });
    doc.variable_collections.insert(PARENT.into(), parent);
    doc.variable_collections.insert(EXT.into(), brand);
    doc.variables.insert(
        BG.into(),
        Variable {
            id: BG.into(),
            name: "bg".into(),
            collection: PARENT.into(),
            kind: VariableKind::Color,
            values: [("10:0".to_string(), color(255, 255, 255)), ("10:3".to_string(), color(0, 0, 0))].into(),
            description: String::new(),
            scopes: vec![],
            hidden_from_publishing: false,
            key: None,
            code_syntax: Default::default(),
            extensions: Default::default(),
            mode_extensions: Default::default(),
        },
    );
    doc
}

fn named<'a>(doc: &'a Document, name: &str) -> &'a VariableCollection {
    doc.variable_collections.values().find(|c| c.name == name).unwrap_or_else(|| panic!("no collection named {name}"))
}

fn assert_extension_survived(back: &Document) {
    let theme = named(back, "Theme");
    let brand = named(back, "Brand");
    assert!(theme.is_extendable(), "the parent keeps its permission to be extended");
    assert!(!theme.is_extension());
    let ext = brand.extension().expect("Brand still extends");
    assert_eq!(ext.parent.collection, theme.id, "the parent link points at the re-imported parent");
    assert_eq!(brand.modes.len(), 2);
    assert_eq!(ext.mode_map[&brand.modes[0].id], theme.modes[0].id);
    assert_eq!(ext.mode_map[&brand.modes[1].id], theme.modes[1].id);
    let bg = back.variables.values().find(|v| v.name == "bg").unwrap();
    assert_eq!(ext.overrides.len(), 1);
    assert_eq!(ext.overrides[&bg.id], [(brand.modes[1].id.clone(), color(255, 0, 0))].into(), "only the Dark override exists");
    assert_eq!(bg.values.len(), 2, "the parent variable keeps just its own modes' values");
    assert!(!bg.values.contains_key(&brand.modes[1].id));
    assert_eq!(back.extended_value(&brand.id, &bg.id, &brand.modes[0].id), Some(color(255, 255, 255)));
    assert_eq!(back.extended_value(&brand.id, &bg.id, &brand.modes[1].id), Some(color(255, 0, 0)));
}

#[test]
fn fig_round_trip_keeps_an_extended_collection_and_its_overrides() {
    let back = import_fig(&export_fig(&extended_doc(None)).expect("export")).expect("re-import");
    assert_extension_survived(&back);
}

#[test]
fn fig_round_trip_keeps_an_extension_of_a_library_collection() {
    let back = import_fig(&export_fig(&extended_doc(Some("lk-83c0"))).expect("export")).expect("re-import");
    assert_extension_survived(&back);
    assert_eq!(named(&back, "Theme").source_library_key.as_deref(), Some("lk-83c0"));
    assert_eq!(named(&back, "Brand").extension().unwrap().parent.set_key.as_deref(), Some("SET_THEME"));
    assert!(!named(&back, "Brand").is_library(), "the extension belongs to the consuming file");
}

#[test]
fn fig_round_trip_keeps_a_parent_the_file_does_not_hold() {
    let mut doc = extended_doc(Some("lk-83c0"));
    doc.variable_collections.remove(PARENT);
    doc.variables.clear();
    let brand = doc.variable_collections.get_mut(EXT).unwrap();
    let mut ext = brand.extension().unwrap();
    ext.parent.collection = String::new();
    ext.overrides.clear();
    ext.mode_map.clear();
    brand.set_extension(&ext);

    let back = import_fig(&export_fig(&doc).expect("export")).expect("re-import");
    let ext = named(&back, "Brand").extension().expect("Brand still extends");
    assert_eq!(ext.parent.set_key.as_deref(), Some("SET_THEME"), "the library parent is kept by key");
    assert!(ext.parent.collection.is_empty());
}

#[test]
fn fig_round_trip_keeps_the_mode_a_layer_picks_in_an_extension() {
    let mut doc = extended_doc(None);
    let page = doc.add_page("Layers");
    let id = doc.alloc_id();
    let frame = doc.insert(page, Node::frame(id, Rect::new(0.0, 0.0, 10.0, 10.0))).unwrap();
    doc.get_mut(frame).unwrap().variables.modes.insert(EXT.into(), "10:6".into());

    let back = import_fig(&export_fig(&doc).expect("export")).expect("re-import");
    let brand = named(&back, "Brand");
    let page = back.pages.last().copied().expect("a page");
    let child = back.get(page).unwrap().children[0];
    assert_eq!(back.get(child).unwrap().variables.modes.get(&brand.id), Some(&brand.modes[1].id));
}

#[test]
fn the_rest_payload_uses_figmas_extended_collection_fields() {
    let doc = extended_doc(None);
    let payload = export_variables_json(&doc);
    let brand = &payload["meta"]["variableCollections"][EXT];
    assert_eq!(brand["isExtension"], json!(true));
    assert_eq!(brand["parentVariableCollectionId"], json!(PARENT));
    assert_eq!(brand["rootVariableCollectionId"], json!(PARENT));
    assert_eq!(brand["modes"][1]["parentModeId"], json!("10:3"));
    assert_eq!(brand["variableOverrides"][BG]["10:6"], json!({"r": 1.0, "g": 0.0, "b": 0.0, "a": 1.0}));
    assert_eq!(brand["inheritedVariableIds"], json!([BG]));
    assert!(payload["meta"]["variableCollections"][PARENT].get("isExtension").is_none());
    assert_eq!(payload["meta"]["variables"][BG]["valuesByMode"].as_object().unwrap().len(), 2, "overrides stay out of the parent's values");

    let mut back = Document::new("d");
    assert_eq!(import_variables_value(&mut back, &payload), 1);
    let ext = back.variable_collections[EXT].extension().unwrap();
    assert_eq!(ext.parent.collection, PARENT);
    assert_eq!(ext.mode_map["10:6"], "10:3");
    assert_eq!(ext.overrides[BG]["10:6"], color(255, 0, 0));
    assert!(back.variable_collections[PARENT].is_extendable());
}
