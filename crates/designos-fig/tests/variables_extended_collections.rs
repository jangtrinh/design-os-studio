//! Extended variable collections (a parent link, a mode mapping and per-variable overrides) survive a `.fig`
//! export and re-import, and the REST variables payload carries them in Figma's documented fields.

mod common;

use common::fig_fixture::{document_and_page, fig_node, variable_set};
use designos_doc::*;
use designos_fig::{export_fig, export_variables_json, import_fig, import_fig_node_changes_with_report, import_variables_value};
use serde_json::{json, Value};

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
    brand.set_extension(&ext);

    let back = import_fig(&export_fig(&doc).expect("export")).expect("re-import");
    let brand = named(&back, "Brand");
    let ext = brand.extension().expect("Brand still extends");
    assert_eq!(ext.parent.set_key.as_deref(), Some("SET_THEME"), "the library parent is kept by key");
    assert!(ext.parent.collection.is_empty());
    assert_eq!(ext.mode_map, [("10:4".to_string(), "10:0".to_string()), ("10:6".to_string(), "10:3".to_string())].into());
    assert_eq!(ext.overrides, [(BG.to_string(), [("10:6".to_string(), color(255, 0, 0))].into())].into());
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

#[test]
fn override_nodes_the_importer_does_not_read_are_counted_in_the_report() {
    let mut nodes = document_and_page();
    nodes.push(variable_set((1, 5), "Colors", (1, 6), json!({})));
    let (_, report) = import_fig_node_changes_with_report(&Value::Array(nodes.clone()), "None").unwrap();
    assert_eq!(report.dropped_variable_overrides, 0);

    nodes.push(fig_node((1, 20), (0, 1), "VARIABLE_OVERRIDE", "Override A", json!({})));
    nodes.push(fig_node((1, 21), (0, 1), "VARIABLE_OVERRIDE", "Override B", json!({})));
    let (_, report) = import_fig_node_changes_with_report(&Value::Array(nodes), "Two").unwrap();
    assert_eq!(report.dropped_variable_overrides, 2, "nothing vanishes silently: both nodes are counted");
}

#[test]
fn an_imported_extension_makes_its_parent_extendable_even_without_the_flag() {
    let mut payload = export_variables_json(&extended_doc(None));
    payload["meta"]["variableCollections"][PARENT].as_object_mut().unwrap().remove("isExtendable");
    let mut back = Document::new("d");
    import_variables_value(&mut back, &payload);
    assert!(back.variable_collections[PARENT].is_extendable(), "lookups rely on the parent's flag to skip the extension scan");
}

#[test]
fn publishing_an_extension_from_file_a_and_using_it_in_file_b_survives_edits_and_fig_round_trip() {
    // File A: Theme (Light = White, Dark = Black) and Brand extending Theme (Dark overridden to Red).
    let mut store_a = VariableStore::default();
    let theme_a = store_a.add_collection("Theme");
    let light_a = store_a.collections[&theme_a].modes[0].id.clone();
    store_a.rename_mode(&theme_a, &light_a, "Light");
    let dark_a = store_a.add_mode(&theme_a, "Dark").unwrap();
    let bg_a = store_a.add_variable(&theme_a, "bg", VariableKind::Color).unwrap();
    assert!(store_a.set_value(&bg_a, &light_a, color(255, 255, 255)));
    assert!(store_a.set_value(&bg_a, &dark_a, color(0, 0, 0)));
    let brand_a = store_a.extend_collection(&theme_a, "Brand").unwrap();
    let b_light_a = store_a.collections[&brand_a].modes[0].id.clone();
    let b_dark_a = store_a.collections[&brand_a].modes[1].id.clone();
    assert!(store_a.set_value(&bg_a, &b_dark_a, color(255, 0, 0)));

    let mut file_a = Document::new("File A");
    file_a.add_page("Page 1");
    store_a.command().apply(&mut file_a).unwrap();
    let published_keys: Vec<String> = publishable_assets(&file_a).into_iter().map(|(k, _, _)| k).collect();
    assert!(published_keys.contains(&format!("col:{brand_a}")), "the extension collection is published: {published_keys:?}");
    let snap = snapshot(&file_a, "L1", "Lib A", 1);

    // File B: has its own collection and extension (reusing Studio ids like VariableCollectionId:d1 / VariableModeId:d2),
    // and imports File A's extension collection.
    let mut store_b = VariableStore::default();
    let own_col = store_b.add_collection("Local");
    let own_mode = store_b.collections[&own_col].modes[0].id.clone();
    let own_var = store_b.add_variable(&own_col, "tint", VariableKind::Color).unwrap();
    assert!(store_b.set_value(&own_var, &own_mode, color(10, 20, 30)));
    let own_ext = store_b.extend_collection(&own_col, "LocalExt").unwrap();
    let own_ext_mode = store_b.collections[&own_ext].modes[0].id.clone();

    let mut file_b = Document::new("File B");
    let page_b = file_b.add_page("Page 1");
    store_b.command().apply(&mut file_b).unwrap();
    import_asset(&mut file_b, &snap, &format!("col:{brand_a}"), None).apply(&mut file_b).unwrap();

    let lib_brand = format!("L1/{brand_a}");
    let lib_bg = format!("L1/{bg_a}");
    let lib_b_light = format!("L1/{b_light_a}");
    let lib_b_dark = format!("L1/{b_dark_a}");

    let place_bound_rect = |doc: &mut Document, name: &str, coll: &str, mode: &str, var: &str| -> NodeId {
        let fid = doc.alloc_id();
        let mut frame = Node::frame(fid, Rect::new(0.0, 0.0, 20.0, 20.0));
        frame.name = name.into();
        frame.variables.modes.insert(coll.into(), mode.into());
        doc.insert(page_b, frame).unwrap();
        let rid = doc.alloc_id();
        let mut rect = Node::rectangle(rid, Rect::new(0.0, 0.0, 10.0, 10.0));
        rect.name = format!("{name}_rect");
        rect.variables.bindings.insert("fills/0".into(), var.into());
        doc.insert(fid, rect).unwrap();
        rid
    };

    let r_light = place_bound_rect(&mut file_b, "BrandLight", &lib_brand, &lib_b_light, &lib_bg);
    let r_dark = place_bound_rect(&mut file_b, "BrandDark", &lib_brand, &lib_b_dark, &lib_bg);
    let r_own = place_bound_rect(&mut file_b, "OwnBase", &own_col, &own_mode, &own_var);
    let r_own_ext = place_bound_rect(&mut file_b, "OwnExt", &own_ext, &own_ext_mode, &own_var);

    // 1. Resolved values from the imported extension collection.
    assert_eq!(file_b.bound_value(r_light, "fills/0"), Some(color(255, 255, 255)));
    assert_eq!(file_b.bound_value(r_dark, "fills/0"), Some(color(255, 0, 0)));

    // 2. Edit in B's own modes (both base mode and B's own extension mode).
    let mut edit_b = VariableStore::from_doc(&file_b);
    assert!(edit_b.set_value(&own_var, &own_mode, color(1, 2, 3)), "edit in B's own base mode is accepted");
    assert!(edit_b.set_value(&own_var, &own_ext_mode, color(4, 5, 6)), "edit in B's own extension mode is accepted");
    edit_b.command().apply(&mut file_b).unwrap();
    assert_eq!(file_b.bound_value(r_own, "fills/0"), Some(color(1, 2, 3)));
    assert_eq!(file_b.bound_value(r_own_ext, "fills/0"), Some(color(4, 5, 6)));

    // 3. `.fig` round-trip of File B keeps all resolved values.
    let back = import_fig(&export_fig(&file_b).expect("export")).expect("re-import");
    let rect_by_name = |name: &str| back.nodes.values().find(|n| n.name == name).unwrap_or_else(|| panic!("missing {name}")).id;
    assert_eq!(back.bound_value(rect_by_name("BrandLight_rect"), "fills/0"), Some(color(255, 255, 255)));
    assert_eq!(back.bound_value(rect_by_name("BrandDark_rect"), "fills/0"), Some(color(255, 0, 0)));
    assert_eq!(back.bound_value(rect_by_name("OwnBase_rect"), "fills/0"), Some(color(1, 2, 3)));
    assert_eq!(back.bound_value(rect_by_name("OwnExt_rect"), "fills/0"), Some(color(4, 5, 6)));
}
