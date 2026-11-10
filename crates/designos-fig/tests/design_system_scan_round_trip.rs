//! A scan through a real `.fig`: a small synthetic design system is built in memory, written with the `.fig`
//! writer, read back and scanned. The result is a checked-in file (`tests/data/design-system-scan-expected.json`),
//! so a change to what the scan says, or to the ids the writer gives, is seen in a diff.
//!
//! To regenerate the file after an intended change: `BLESS_SCAN_EXPECTED=1 cargo test -p designos-fig --test design_system_scan_round_trip`.

use designos_doc::*;
use designos_fig::{export_fig, import_fig, scan_design_system};

fn add(doc: &mut Document, parent: NodeId, kind: NodeKind, name: &str, size: (f32, f32)) -> NodeId {
    let id = doc.alloc_id();
    let mut node = Node::new(id, kind);
    node.name = name.into();
    node.size = Vec2::new(size.0, size.1);
    doc.insert(parent, node).unwrap()
}

fn solid(doc: &mut Document, id: NodeId) {
    doc.get_mut(id).unwrap().fills = vec![Paint::solid(Color::WHITE)];
}

/// Two pages: a design-system page with a section holding an icon and a two-variant button set, and a screens page with a
/// frame and, in a section, another frame, each using the button and the icon. The button's large variant binds a colour
/// variable of a local collection.
fn design_system() -> Document {
    let mut doc = Document::new("kit");
    let ds = doc.add_page("Design system");
    let screens = doc.add_page("Screens");

    doc.variable_collections.insert(
        "c-theme".into(),
        VariableCollection {
            id: "c-theme".into(),
            name: "Theme".into(),
            modes: vec![VariableMode { id: "m-light".into(), name: "Light".into() }, VariableMode { id: "m-dark".into(), name: "Dark".into() }],
            default_mode: "m-light".into(),
            variables: vec!["v-brand".into(), "v-gap".into()],
            key: None,
            source_library_key: None,
            extensions: Default::default(),
        },
    );
    let var = |id: &str, name: &str, kind: VariableKind, light: VariableValue, dark: VariableValue| Variable {
        id: id.into(),
        name: name.into(),
        collection: "c-theme".into(),
        kind,
        values: [("m-light".to_string(), light), ("m-dark".to_string(), dark)].into_iter().collect(),
        description: String::new(),
        scopes: Vec::new(),
        hidden_from_publishing: false,
        key: None,
        code_syntax: Default::default(),
        extensions: Default::default(),
        mode_extensions: Default::default(),
    };
    doc.variables.insert(
        "v-brand".into(),
        var(
            "v-brand",
            "color/brand",
            VariableKind::Color,
            VariableValue::Color(Color::rgb(0x33, 0x66, 0xFF)),
            VariableValue::Color(Color::rgb(0x99, 0xBB, 0xFF)),
        ),
    );
    doc.variables.insert("v-gap".into(), var("v-gap", "space/gap", VariableKind::Number, VariableValue::Number(8.0), VariableValue::Number(12.0)));

    let atoms = add(&mut doc, ds, NodeKind::Section, "Atoms", (800.0, 400.0));
    let icon = add(&mut doc, atoms, NodeKind::Component(FrameProps::default()), "Icon", (24.0, 24.0));
    let small = add(&mut doc, atoms, NodeKind::Component(FrameProps::default()), "Size=S", (80.0, 32.0));
    let large = add(&mut doc, atoms, NodeKind::Component(FrameProps::default()), "Size=L", (120.0, 48.0));
    for v in [small, large] {
        solid(&mut doc, v);
    }
    doc.get_mut(large).unwrap().variables.bindings.insert("fills/0".into(), "v-brand".into());
    combine_as_variants(&mut doc, &[small, large]).apply(&mut doc).expect("combine as variants");
    let set =
        doc.get(atoms).unwrap().children.iter().copied().find(|c| matches!(doc.get(*c).map(|n| &n.kind), Some(NodeKind::ComponentSet(_)))).expect("a set");
    doc.get_mut(set).unwrap().name = "Button".into();
    // The icon is used inside the large variant: a component using a component.
    create_instance(&mut doc, icon, large, Vec2::new(4.0, 4.0)).apply(&mut doc).expect("instance in a variant");

    let home = add(&mut doc, screens, NodeKind::Frame(FrameProps::default()), "Home", (375.0, 812.0));
    for at in [(10.0, 10.0), (50.0, 10.0)] {
        create_instance(&mut doc, icon, home, Vec2::new(at.0, at.1)).apply(&mut doc).expect("icon instance");
    }
    create_instance(&mut doc, small, home, Vec2::new(10.0, 100.0)).apply(&mut doc).expect("button instance");
    let flows = add(&mut doc, screens, NodeKind::Section, "Flows", (900.0, 900.0));
    let login = add(&mut doc, flows, NodeKind::Frame(FrameProps::default()), "Login", (390.0, 844.0));
    create_instance(&mut doc, large, login, Vec2::new(10.0, 10.0)).apply(&mut doc).expect("button instance");
    doc
}

#[test]
fn a_design_system_written_as_fig_and_read_back_scans_to_the_checked_in_file() {
    let reopened = import_fig(&export_fig(&design_system()).unwrap()).unwrap();
    let text = scan_design_system(&reopened).expect("the scan succeeds").to_json_text();
    let expected_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/design-system-scan-expected.json");
    if std::env::var_os("BLESS_SCAN_EXPECTED").is_some() {
        std::fs::write(&expected_path, &text).unwrap();
    }
    let expected = std::fs::read_to_string(&expected_path).expect("tests/data/design-system-scan-expected.json is checked in");
    assert_eq!(text, expected, "the scan of the round-tripped design system changed; if intended, regenerate the file (see the module comment)");
}

#[test]
fn the_round_tripped_scan_ties_each_instance_to_a_figma_id_of_the_file_itself() {
    let reopened = import_fig(&export_fig(&design_system()).unwrap()).unwrap();
    let scan: serde_json::Value = serde_json::from_str(&scan_design_system(&reopened).unwrap().to_json_text()).unwrap();
    let ids: std::collections::BTreeSet<&str> = scan["components"].as_array().unwrap().iter().map(|c| c["id"].as_str().unwrap()).collect();
    for frame in scan["frames"].as_array().unwrap() {
        for inst in frame["instances"].as_array().unwrap() {
            assert!(ids.contains(inst["component"].as_str().unwrap()), "{inst} names a component entry of the scan");
        }
    }
    assert_eq!(scan["sensing"], serde_json::json!({"status": "complete"}));
}

fn encode_fig(changes: Vec<designos_fig::kiwi::KiwiValue>) -> Vec<u8> {
    use designos_fig::container::{self, Compression};
    use designos_fig::kiwi::KiwiValue as V;
    use designos_fig::testkit::{figma_like_schema, msg};
    let schema = figma_like_schema();
    let root = schema.find("Message").expect("Message in schema");
    let body = schema.encode_message(root, &msg(vec![("nodeChanges", V::Array(changes))])).unwrap();
    container::write_container(&schema.encode(), &body, Compression::Zstd, 106)
}

#[test]
fn an_unpublished_symbol_on_the_internal_canvas_of_a_fig_file_resolves_to_its_set_or_own_figma_id() {
    use designos_fig::kiwi::KiwiValue as V;
    use designos_fig::testkit::{guid, msg, node, vec2};

    let bytes = encode_fig(vec![
        node((0, 0), (0, 0), "!", "DOCUMENT", "Document", vec![]),
        node((0, 1), (0, 0), "a", "CANVAS", "Screens", vec![]),
        node((0, 2), (0, 0), "b", "CANVAS", "Internal Only Canvas", vec![("internalOnly", V::Bool(true))]),
        node((10, 50), (0, 2), "a", "SYMBOL", "InternalIcon", vec![("size", vec2(24.0, 24.0))]),
        node((10, 60), (0, 2), "b", "FRAME", "InternalButton", vec![("isStateGroup", V::Bool(true)), ("size", vec2(120.0, 48.0))]),
        node((10, 61), (10, 60), "a", "SYMBOL", "Size=M", vec![("size", vec2(120.0, 48.0))]),
        node((1, 1), (0, 1), "a", "FRAME", "Home", vec![("size", vec2(375.0, 812.0))]),
        node((1, 2), (1, 1), "a", "INSTANCE", "Icon copy", vec![("size", vec2(24.0, 24.0)), ("symbolData", msg(vec![("symbolID", guid(10, 50))]))]),
        node((1, 3), (1, 1), "b", "INSTANCE", "Button copy", vec![("size", vec2(120.0, 48.0)), ("symbolData", msg(vec![("symbolID", guid(10, 61))]))]),
    ]);
    let reopened = import_fig(&bytes).unwrap();
    let scan: serde_json::Value = serde_json::from_str(&scan_design_system(&reopened).unwrap().to_json_text()).unwrap();
    let home = &scan["frames"][0];
    assert_eq!(home["instances"], serde_json::json!([{"component": "10:50", "count": 1}, {"component": "10:60", "count": 1}]));
    assert!(home.get("unresolvedInstances").is_none(), "{home}");
}

#[test]
fn an_instance_whose_symbol_is_absent_from_a_fig_file_counts_in_unresolved_instances_while_a_real_library_key_resolves() {
    use designos_fig::kiwi::KiwiValue as V;
    use designos_fig::testkit::{guid, msg, node, vec2};

    let bytes = encode_fig(vec![
        node((0, 0), (0, 0), "!", "DOCUMENT", "Document", vec![]),
        node((0, 1), (0, 0), "a", "CANVAS", "Screens", vec![]),
        node((0, 2), (0, 0), "b", "CANVAS", "Internal Only Canvas", vec![("internalOnly", V::Bool(true))]),
        node(
            (10, 70),
            (0, 2),
            "a",
            "SYMBOL",
            "PublishedIcon",
            vec![("size", vec2(24.0, 24.0)), ("componentKey", V::String("lib-key-real".into())), ("sourceLibraryKey", V::String("remote-lib".into()))],
        ),
        node((1, 1), (0, 1), "a", "FRAME", "Home", vec![("size", vec2(375.0, 812.0))]),
        node((1, 2), (1, 1), "a", "INSTANCE", "Published copy", vec![("size", vec2(24.0, 24.0)), ("symbolData", msg(vec![("symbolID", guid(10, 70))]))]),
        node((1, 3), (1, 1), "b", "INSTANCE", "Missing symbol copy", vec![("size", vec2(24.0, 24.0)), ("symbolData", msg(vec![("symbolID", guid(999, 999))]))]),
    ]);
    let reopened = import_fig(&bytes).unwrap();
    assert!(
        reopened.library.external.keys().any(|k| k.ends_with("/999:999")),
        "precondition: link_remote_components built a synthetic stand-in keyed by the missing guid"
    );
    let scan: serde_json::Value = serde_json::from_str(&scan_design_system(&reopened).unwrap().to_json_text()).unwrap();
    let home = &scan["frames"][0];
    assert_eq!(home["instances"], serde_json::json!([{"component": "lib-key-real", "count": 1}]));
    assert_eq!(home["unresolvedInstances"], serde_json::json!(1));
}
