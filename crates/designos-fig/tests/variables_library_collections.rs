//! A collection subscribed from a shared library keeps its library marker through a `.fig` export and
//! re-import, and a collection the file owns stays unmarked.

use designos_doc::*;
use designos_fig::{export_fig, import_fig};

fn collection(id: &str, mode: &str, key: &str, source_library_key: Option<&str>) -> VariableCollection {
    VariableCollection {
        id: id.into(),
        name: "Theme".into(),
        modes: vec![VariableMode { id: mode.into(), name: "Light".into() }],
        default_mode: mode.into(),
        variables: Vec::new(),
        key: Some(key.into()),
        source_library_key: source_library_key.map(String::from),
        extensions: Default::default(),
    }
}

#[test]
fn fig_export_then_import_keeps_the_library_marker() {
    let mut doc = Document::new("Consumer");
    doc.add_page("Page 1");
    doc.variable_collections.insert("VariableCollectionId:10:1".into(), collection("VariableCollectionId:10:1", "10:0", "K_LOCAL", None));
    doc.variable_collections.insert("VariableCollectionId:10:2".into(), collection("VariableCollectionId:10:2", "10:1", "K_LIB", Some("lk-83c0")));

    let back = import_fig(&export_fig(&doc).expect("export")).expect("re-import");

    let named =
        |key: &str| back.variable_collections.values().find(|c| c.key.as_deref() == Some(key)).unwrap_or_else(|| panic!("no collection with key {key}"));
    assert!(!named("K_LOCAL").is_library(), "an own collection stays local");
    assert_eq!(named("K_LIB").source_library_key.as_deref(), Some("lk-83c0"), "a library collection keeps its source library");
}
