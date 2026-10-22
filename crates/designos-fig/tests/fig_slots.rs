//! Slots in `.fig`: the `isSlot` / `isSlotContent` flags, the SLOT component property with its
//! `SlotPropConfig` and preferred values, the `SLOT_CONTENT_ID` binding, and the instance
//! `slot-content-id`. Fixtures are hand-written and content-free; the last tests scan the owner's
//! private `VSF - PCP.fig` (never committed, skipped when absent) and assert only counts.

use designos_doc::*;
use designos_fig::{export_fig, import_fig, import_fig_node_changes, import_rest_json};
use serde_json::{json, Value};

fn find<'a>(doc: &'a Document, name: &str) -> &'a Node {
    doc.nodes.values().find(|n| n.name == name).unwrap_or_else(|| panic!("no node named {name}"))
}

fn full_config(preferred: Vec<NodeId>) -> SlotPropConfig {
    SlotPropConfig {
        stretch_child_on_insert: Some(true),
        display_by_default: Some(false),
        min_children: Some(1),
        max_children: Some(3),
        allow_preferred_values_only: Some(true),
        preferred,
        ..Default::default()
    }
}

/// A `Panel` component with a slot property, a slot frame bound to it, one preferred component,
/// content flagged as slot content, and an instance that fills the slot.
fn slot_document() -> Document {
    let mut doc = Document::new("slots");
    let page = doc.add_page("Page");

    let icon_id = doc.alloc_id();
    let mut icon = Node::new(icon_id, NodeKind::Component(FrameProps::default()));
    icon.name = "Icon".into();
    icon.size = Vec2::new(16.0, 16.0);
    doc.insert(page, icon).unwrap();

    let panel_id = doc.alloc_id();
    let mut panel = Node::new(panel_id, NodeKind::Component(FrameProps::default()));
    panel.name = "Panel".into();
    panel.size = Vec2::new(320.0, 200.0);
    panel.component.description = "A panel with a body slot".into();
    panel.component.expose_nested_instances = true;
    panel.fig_metadata.insert("userFacingVersion".into(), "126".into());
    panel.component.props = vec![
        ComponentPropDef { name: "Dense#1:0".into(), kind: PropKind::Boolean, default: PropValue::Bool(true) },
        ComponentPropDef { name: "Body#1:1".into(), kind: PropKind::Slot, default: PropValue::Slot(vec![]) },
    ];
    panel.component.slot_configs.insert("Body#1:1".into(), full_config(vec![icon_id]));
    doc.insert(page, panel).unwrap();

    let body_id = doc.alloc_id();
    let mut body = Node::new(body_id, NodeKind::Frame(FrameProps::default()));
    body.name = "BodySlot".into();
    body.size = Vec2::new(280.0, 120.0);
    body.component.bindings.insert(OverrideField::SlotContent, "Body#1:1".into());
    body.component.is_slot = true;
    doc.insert(panel_id, body).unwrap();

    let content_id = doc.alloc_id();
    let mut content = Node::new(content_id, NodeKind::Rectangle);
    content.name = "Banner".into();
    content.size = Vec2::new(260.0, 60.0);
    content.component.is_slot_content = true;
    doc.insert(page, content).unwrap();

    create_instance(&mut doc, panel_id, page, Vec2::new(400.0, 0.0)).apply(&mut doc).expect("create_instance");
    let inst_id = instances_of(&doc, panel_id)[0];
    let inst = doc.get_mut(inst_id).unwrap();
    inst.name = "PanelInstance".into();
    inst.component.values.insert("Body#1:1".into(), PropValue::Slot(vec![content_id]));
    inst.component.slot_content_id = Some("7:42".into());
    doc
}

#[test]
fn slot_flags_config_and_instance_content_survive_a_fig_round_trip() {
    let doc = slot_document();
    let rt = import_fig(&export_fig(&doc).expect("export_fig")).expect("import_fig");

    let icon = find(&rt, "Icon");
    let panel = find(&rt, "Panel");
    assert_eq!(panel.component.slot_configs.len(), 1);
    assert_eq!(panel.component.slot_configs.get("Body"), Some(&full_config(vec![icon.id])));
    assert!(panel.component.props.iter().any(|p| p.name == "Body" && p.kind == PropKind::Slot));

    let body = find(&rt, "BodySlot");
    assert!(body.component.is_slot, "isSlot");
    assert!(!body.component.is_slot_content);
    assert_eq!(body.component.bindings.get(&OverrideField::SlotContent).map(String::as_str), Some("Body"));

    let banner = find(&rt, "Banner");
    assert!(banner.component.is_slot_content, "isSlotContent");
    assert!(!banner.component.is_slot);

    let inst = find(&rt, "PanelInstance");
    assert_eq!(inst.component.values.get("Body"), Some(&PropValue::Slot(vec![banner.id])), "instance keeps its slot content");
    assert_eq!(inst.component.slot_content_id.as_deref(), Some("7:42"));

    assert!(!icon.component.is_slot && !icon.component.is_slot_content && icon.component.slot_content_id.is_none());
}

#[test]
fn only_the_slot_fields_the_file_wrote_come_back() {
    let mut doc = slot_document();
    let panel_id = find(&doc, "Panel").id;
    let panel = doc.get_mut(panel_id).unwrap();
    // minChildren written as 0 stays 0; the other four fields were never written.
    panel.component.slot_configs.insert("Body#1:1".into(), SlotPropConfig { min_children: Some(0), ..Default::default() });
    let rt = import_fig(&export_fig(&doc).unwrap()).unwrap();
    assert_eq!(find(&rt, "Panel").component.slot_configs.get("Body"), Some(&SlotPropConfig { min_children: Some(0), ..Default::default() }));

    // A slot property with no settings at all gets no settings entry.
    let panel = doc.get_mut(panel_id).unwrap();
    panel.component.slot_configs.clear();
    let rt = import_fig(&export_fig(&doc).unwrap()).unwrap();
    assert!(find(&rt, "Panel").component.slot_configs.is_empty());
}

#[test]
fn data_beside_the_slots_is_not_disturbed() {
    let doc = slot_document();
    let rt = import_fig(&export_fig(&doc).unwrap()).unwrap();
    let panel = find(&rt, "Panel");
    assert_eq!(panel.component.description, "A panel with a body slot");
    assert!(panel.component.expose_nested_instances);
    assert_eq!(panel.fig_metadata.get("userFacingVersion").map(String::as_str), Some("126"));
    let dense = panel.component.props.iter().find(|p| p.name == "Dense").expect("Dense prop");
    assert_eq!((dense.kind.clone(), dense.default.clone()), (PropKind::Boolean, PropValue::Bool(true)));
    assert_eq!(panel.component.props.len(), 2);
}

#[test]
fn a_second_round_trip_changes_nothing() {
    let once = import_fig(&export_fig(&slot_document()).unwrap()).unwrap();
    let twice = import_fig(&export_fig(&once).unwrap()).unwrap();
    for name in ["Panel", "BodySlot", "Banner", "PanelInstance"] {
        assert_eq!(find(&once, name).component.slot_configs, find(&twice, name).component.slot_configs, "{name}");
        assert_eq!(find(&once, name).component.is_slot, find(&twice, name).component.is_slot, "{name}");
        assert_eq!(find(&once, name).component.is_slot_content, find(&twice, name).component.is_slot_content, "{name}");
        assert_eq!(find(&once, name).component.slot_content_id, find(&twice, name).component.slot_content_id, "{name}");
    }
}

#[test]
fn a_renamed_slot_property_keeps_its_settings_in_the_exported_file() {
    let mut doc = slot_document();
    let panel_id = find(&doc, "Panel").id;
    rename_prop(&mut doc, panel_id, "Body#1:1", "Content#1:1").apply(&mut doc).unwrap();
    let rt = import_fig(&export_fig(&doc).unwrap()).unwrap();
    let icon = find(&rt, "Icon").id;
    let panel = find(&rt, "Panel");
    assert_eq!(panel.component.slot_configs.get("Content"), Some(&full_config(vec![icon])));
    assert!(!panel.component.slot_configs.contains_key("Body"));
    assert_eq!(find(&rt, "BodySlot").component.bindings.get(&OverrideField::SlotContent).map(String::as_str), Some("Content"));
}

fn g(s: u64, l: u64) -> Value {
    json!({"sessionID": s, "localID": l})
}

fn nc(id: (u64, u64), parent: (u64, u64), pos: &str, ty: &str, name: &str, extra: Value) -> Value {
    let mut v = json!({
        "guid": g(id.0, id.1), "parentIndex": {"guid": g(parent.0, parent.1), "position": pos},
        "type": ty, "name": name, "phase": "CREATED", "visible": true, "opacity": 1.0,
        "size": {"x": 10.0, "y": 10.0},
        "transform": {"m00": 1.0, "m01": 0.0, "m02": 0.0, "m10": 0.0, "m11": 1.0, "m12": 0.0}
    });
    for (k, x) in extra.as_object().unwrap() {
        v[k] = x.clone();
    }
    v
}

/// Kiwi-shaped node changes with the field names of the Figma 126 schema.
fn kiwi_fixture() -> Value {
    json!([
        nc((0, 0), (0, 0), "!", "DOCUMENT", "Document", json!({})),
        nc((0, 1), (0, 0), "a", "CANVAS", "Page", json!({})),
        nc(
            (5, 1),
            (0, 1),
            "a",
            "SYMBOL",
            "Panel",
            json!({
            "size": {"x": 200.0, "y": 100.0},
            "componentPropDefs": [{"id": g(7, 1), "name": "Body", "type": "SLOT",
                "slotPropConfig": {"stretchChildOnInsert": true, "displayByDefault": false,
                    "minChildren": 1, "maxChildren": 4, "allowPreferredValuesOnly": true}}]})
        ),
        nc(
            (5, 2),
            (5, 1),
            "a",
            "FRAME",
            "Body slot",
            json!({
            "isSlot": true,
            "componentPropRefs": [{"componentPropNodeField": "SLOT_CONTENT_ID", "defID": g(7, 1)}]})
        ),
        nc((5, 3), (5, 1), "b", "FRAME", "Header", json!({})),
        nc((1, 1), (0, 1), "b", "FRAME", "Card", json!({"isSlotContent": true})),
        nc(
            (1, 2),
            (0, 1),
            "c",
            "INSTANCE",
            "Panel copy",
            json!({
            "symbolData": {"symbolID": g(5, 1), "symbolOverrides": [], "uniformScaleFactor": 1.0},
            "jsonSerialized": {"slot-content-id": {"guid": g(9, 9)}}})
        )
    ])
}

#[test]
fn imports_slot_fields_from_a_kiwi_shaped_fixture() {
    let doc = import_fig_node_changes(&kiwi_fixture(), "t").unwrap();
    let panel = find(&doc, "Panel");
    assert_eq!(
        panel.component.slot_configs.get("Body"),
        Some(&SlotPropConfig {
            stretch_child_on_insert: Some(true),
            display_by_default: Some(false),
            min_children: Some(1),
            max_children: Some(4),
            allow_preferred_values_only: Some(true),
            preferred: vec![],
            ..Default::default()
        })
    );
    let slot = find(&doc, "Body slot");
    assert!(slot.component.is_slot);
    assert_eq!(slot.component.bindings.get(&OverrideField::SlotContent).map(String::as_str), Some("Body"));
    assert!(!find(&doc, "Header").component.is_slot);
    assert!(find(&doc, "Card").component.is_slot_content);
    assert_eq!(find(&doc, "Panel copy").component.slot_content_id.as_deref(), Some("9:9"));
    assert!(find(&doc, "Page").component.slot_content_id.is_none());
}

/// A slot whose file lists preferred components but never wrote a `slotPropConfig`.
fn preferred_without_config_fixture() -> Value {
    json!([
        nc((0, 0), (0, 0), "!", "DOCUMENT", "Document", json!({})),
        nc((0, 1), (0, 0), "a", "CANVAS", "Page", json!({})),
        nc((6, 1), (0, 1), "a", "SYMBOL", "Icon", json!({})),
        nc(
            (5, 1),
            (0, 1),
            "b",
            "SYMBOL",
            "Panel",
            json!({
            "componentPropDefs": [{"id": g(7, 1), "name": "Body", "type": "SLOT", "preferredValues": [g(6, 1)]}]})
        )
    ])
}

#[test]
fn preferred_components_are_kept_when_the_file_wrote_no_slot_config() {
    let doc = import_fig_node_changes(&preferred_without_config_fixture(), "t").unwrap();
    let icon = find(&doc, "Icon").id;
    let cfg = find(&doc, "Panel").component.slot_configs.get("Body").cloned().expect("preferred values are kept");
    assert_eq!(cfg, SlotPropConfig { preferred: vec![icon], ..Default::default() });
}

#[test]
fn preferred_components_without_a_slot_config_survive_a_fig_round_trip() {
    let doc = import_fig_node_changes(&preferred_without_config_fixture(), "t").unwrap();
    let rt = import_fig(&export_fig(&doc).unwrap()).unwrap();
    let icon = find(&rt, "Icon").id;
    assert_eq!(find(&rt, "Panel").component.slot_configs.get("Body"), Some(&SlotPropConfig { preferred: vec![icon], ..Default::default() }));
}

#[test]
fn raw_numeric_slot_entries_are_rejected_and_do_not_bind_unrelated_nodes() {
    let mut fixture = kiwi_fixture();
    let arr = fixture.as_array_mut().unwrap();
    // Instance (1, 2) assigns SLOT prop (7, 1) with raw numbers "1" (Page), "2" (Panel), "999" (non-existent),
    // alongside valid GUID "1:1" (Card).
    arr[6]["componentPropAssignments"] = json!([
        {
            "defID": g(7, 1),
            "value": { "textValue": { "characters": "1, 2, 1:1, 999" } }
        }
    ]);
    let doc = import_fig_node_changes(&fixture, "t").unwrap();
    let card_id = find(&doc, "Card").id;
    let inst = find(&doc, "Panel copy");
    assert_eq!(
        inst.component.values.get("Body"),
        Some(&PropValue::Slot(vec![card_id])),
        "raw numbers in a SLOT list must be rejected instead of aliasing internal NodeIds"
    );
}

#[test]
fn unknown_slot_prop_config_keys_are_preserved_in_native_format() {
    let fixture = json!([
        nc((0, 0), (0, 0), "!", "DOCUMENT", "Document", json!({})),
        nc((0, 1), (0, 0), "a", "CANVAS", "Page", json!({})),
        nc(
            (5, 1),
            (0, 1),
            "a",
            "SYMBOL",
            "Panel",
            json!({
                "componentPropDefs": [{
                    "id": g(7, 1),
                    "name": "Body",
                    "type": "SLOT",
                    "slotPropConfig": {
                        "stretchChildOnInsert": true,
                        "minChildren": 1,
                        "futureSlotMode": "WRAP",
                        "customLimit": 8
                    }
                }]
            })
        )
    ]);
    let doc = import_fig_node_changes(&fixture, "t").unwrap();
    let cfg = find(&doc, "Panel").component.slot_configs.get("Body").expect("Body slot config exists");
    assert_eq!(cfg.stretch_child_on_insert, Some(true));
    assert_eq!(cfg.min_children, Some(1));
    assert_eq!(cfg.extra.get("futureSlotMode"), Some(&json!("WRAP")));
    assert_eq!(cfg.extra.get("customLimit"), Some(&json!(8)));

    let native_json = doc.to_json().unwrap();
    let reloaded = Document::from_json(&native_json).unwrap();
    let reloaded_cfg = find(&reloaded, "Panel").component.slot_configs.get("Body").expect("Body slot config survives native round-trip");
    assert_eq!(reloaded_cfg, cfg);
    assert_eq!(reloaded_cfg.extra.get("futureSlotMode"), Some(&json!("WRAP")));
    assert_eq!(reloaded_cfg.extra.get("customLimit"), Some(&json!(8)));
}

#[test]
fn real_preferred_values_object_round_trips_for_slot_and_instance_swap_and_keeps_legacy_guid_list() {
    use designos_fig::container;
    use designos_fig::kiwi::{KiwiValue, Schema};

    let real_fixture = json!([
        nc((0, 0), (0, 0), "!", "DOCUMENT", "Document", json!({})),
        nc((0, 1), (0, 0), "a", "CANVAS", "Page", json!({})),
        nc((0, 2), (0, 0), "b", "CANVAS", "Internal Only Canvas", json!({"internalOnly": true})),
        nc((6, 1), (0, 2), "a", "SYMBOL", "LibIcon", json!({"componentKey": "K_LIB_ICON", "sourceLibraryKey": "lk-icons"})),
        nc((6, 2), (0, 1), "a", "FRAME", "BadgeSet", json!({"isStateGroup": true, "componentKey": "K_BADGE_SET"})),
        nc((6, 3), (6, 2), "a", "SYMBOL", "Variant=Default", json!({"componentKey": "K_BADGE_DEF"})),
        nc(
            (5, 1),
            (0, 1),
            "b",
            "SYMBOL",
            "Panel",
            json!({
                "componentKey": "K_PANEL",
                "componentPropDefs": [
                    {
                        "id": g(7, 1),
                        "name": "LeadIcon",
                        "type": "INSTANCE_SWAP",
                        "initialValue": {"guidValue": g(6, 1)},
                        "preferredValues": {
                            "stringValues": [],
                            "instanceSwapValues": [
                                {"type": "COMPONENT", "key": "K_LIB_ICON"},
                                {"type": "STATE_GROUP", "key": "K_BADGE_SET"}
                            ],
                            "parameterValues": []
                        }
                    },
                    {
                        "id": g(7, 2),
                        "name": "Body",
                        "type": "SLOT",
                        "preferredValues": {
                            "stringValues": [],
                            "instanceSwapValues": [
                                {"type": "COMPONENT", "key": "K_LIB_ICON"},
                                {"type": "STATE_GROUP", "key": "K_BADGE_SET"}
                            ],
                            "parameterValues": []
                        },
                        "slotPropConfig": {"allowPreferredValuesOnly": true}
                    }
                ]
            })
        )
    ]);

    // 1. Import the real preferredValues object shape and resolve keys via library-key / componentKey lookup.
    let doc = import_fig_node_changes(&real_fixture, "t").unwrap();
    let lib_icon = find(&doc, "LibIcon").id;
    let badge_set = find(&doc, "BadgeSet").id;
    let panel = find(&doc, "Panel");

    let swap_prop = panel.component.props.iter().find(|p| p.name == "LeadIcon").expect("LeadIcon prop");
    assert_eq!(swap_prop.kind, PropKind::InstanceSwap { preferred: vec![lib_icon, badge_set] });
    assert_eq!(
        panel.component.slot_configs.get("Body"),
        Some(&SlotPropConfig { allow_preferred_values_only: Some(true), preferred: vec![lib_icon, badge_set], ..Default::default() })
    );

    // 2. Export to .fig and verify the binary Kiwi schema + message uses tag 8 ComponentPropPreferredValues object.
    let exported_bytes = export_fig(&doc).expect("export_fig");
    let archive = container::open(&exported_bytes).expect("open container");
    let (schema_bin, msg_bin) = container::read_archive_chunks(&archive).expect("read archive chunks");
    let schema = Schema::decode(&schema_bin).expect("decode schema");
    let cpd_def = schema.defs.iter().find(|d| d.name == "ComponentPropDef").expect("ComponentPropDef in schema");
    let pv_field = cpd_def.fields.iter().find(|f| f.name.as_ref() == "preferredValues").expect("preferredValues field");
    assert_eq!(pv_field.value, 8, "preferredValues tag is 8 in ComponentPropDef");
    assert!(!pv_field.is_array, "preferredValues is a single ComponentPropPreferredValues message, not an array");

    let root = schema.root_message().unwrap();
    let decoded_msg = schema.decode_message(root, &msg_bin).unwrap();
    let changes = match decoded_msg.get("nodeChanges") {
        Some(KiwiValue::Array(arr)) => arr,
        other => panic!("expected nodeChanges array, got {other:?}"),
    };
    let panel_nc = changes.iter().find(|nc| nc.get("name") == Some(&KiwiValue::String("Panel".into()))).expect("Panel NodeChange");
    let defs = match panel_nc.get("componentPropDefs") {
        Some(KiwiValue::Array(arr)) => arr,
        other => panic!("expected componentPropDefs array, got {other:?}"),
    };
    for def_val in defs {
        let pv = def_val.get("preferredValues").expect("preferredValues object on exported prop def");
        let swap_vals = match pv.get("instanceSwapValues") {
            Some(KiwiValue::Array(arr)) => arr,
            other => panic!("expected instanceSwapValues array inside preferredValues, got {other:?}"),
        };
        assert_eq!(swap_vals.len(), 2);
        assert_eq!(swap_vals[0].get("type"), Some(&KiwiValue::Enum("COMPONENT".into())));
        assert_eq!(swap_vals[0].get("key"), Some(&KiwiValue::String("K_LIB_ICON".into())));
        assert_eq!(swap_vals[1].get("type"), Some(&KiwiValue::Enum("STATE_GROUP".into())));
        assert_eq!(swap_vals[1].get("key"), Some(&KiwiValue::String("K_BADGE_SET".into())));
    }

    // 3. Re-import the exported .fig and verify round-trip fidelity of preferred values for both prop types.
    let rt = import_fig(&exported_bytes).expect("import_fig");
    let rt_icon = find(&rt, "LibIcon").id;
    let rt_set = find(&rt, "BadgeSet").id;
    let rt_panel = find(&rt, "Panel");
    let rt_swap = rt_panel.component.props.iter().find(|p| p.name == "LeadIcon").expect("LeadIcon after round-trip");
    assert_eq!(rt_swap.kind, PropKind::InstanceSwap { preferred: vec![rt_icon, rt_set] });
    assert_eq!(
        rt_panel.component.slot_configs.get("Body"),
        Some(&SlotPropConfig { allow_preferred_values_only: Some(true), preferred: vec![rt_icon, rt_set], ..Default::default() })
    );

    // 4. Legacy guid-list form still imports for both INSTANCE_SWAP and SLOT.
    let legacy_fixture = json!([
        nc((0, 0), (0, 0), "!", "DOCUMENT", "Document", json!({})),
        nc((0, 1), (0, 0), "a", "CANVAS", "Page", json!({})),
        nc((6, 1), (0, 1), "a", "SYMBOL", "Icon", json!({})),
        nc(
            (5, 1),
            (0, 1),
            "b",
            "SYMBOL",
            "LegacyPanel",
            json!({
                "componentPropDefs": [
                    {"id": g(7, 1), "name": "Swap", "type": "INSTANCE_SWAP", "initialValue": {"guidValue": g(6, 1)}, "preferredValues": [g(6, 1)]},
                    {"id": g(7, 2), "name": "Body", "type": "SLOT", "preferredValues": [g(6, 1)]}
                ]
            })
        )
    ]);
    let legacy_doc = import_fig_node_changes(&legacy_fixture, "t").unwrap();
    let legacy_icon = find(&legacy_doc, "Icon").id;
    let legacy_panel = find(&legacy_doc, "LegacyPanel");
    let legacy_swap = legacy_panel.component.props.iter().find(|p| p.name == "Swap").unwrap();
    assert_eq!(legacy_swap.kind, PropKind::InstanceSwap { preferred: vec![legacy_icon] });
    assert_eq!(legacy_panel.component.slot_configs.get("Body"), Some(&SlotPropConfig { preferred: vec![legacy_icon], ..Default::default() }));
}

/// `Page` holds an icon component, a button component that nests an instance of the icon (the
/// instance repeats the icon's `componentKey`, as library imports and our own export do), and a
/// `Panel` whose slot and instance-swap props prefer the icon by key. The nested instance comes
/// after the icon in document order, and the panel after both.
fn preferred_key_with_later_instance_fixture() -> Value {
    json!([
        nc((0, 0), (0, 0), "!", "DOCUMENT", "Document", json!({})),
        nc((0, 1), (0, 0), "a", "CANVAS", "Page", json!({})),
        nc((6, 1), (0, 1), "a", "SYMBOL", "Icon", json!({"componentKey": "K_ICON"})),
        nc((6, 2), (0, 1), "b", "SYMBOL", "Button", json!({"componentKey": "K_BUTTON"})),
        nc((6, 3), (6, 2), "a", "INSTANCE", "IconInstance", json!({"componentKey": "K_ICON", "symbolData": {"symbolID": g(6, 1)}})),
        nc(
            (5, 1),
            (0, 1),
            "c",
            "SYMBOL",
            "Panel",
            json!({
                "componentKey": "K_PANEL",
                "componentPropDefs": [
                    {
                        "id": g(7, 1),
                        "name": "LeadIcon",
                        "type": "INSTANCE_SWAP",
                        "initialValue": {"guidValue": g(6, 1)},
                        "preferredValues": {"stringValues": [], "instanceSwapValues": [{"type": "COMPONENT", "key": "K_ICON"}]}
                    },
                    {
                        "id": g(7, 2),
                        "name": "Body",
                        "type": "SLOT",
                        "preferredValues": {"stringValues": [], "instanceSwapValues": [{"type": "COMPONENT", "key": "K_ICON"}]}
                    }
                ]
            })
        )
    ])
}

fn assert_preferred_is(doc: &Document, expected: NodeId) {
    assert!(matches!(doc.get(expected).unwrap().kind, NodeKind::Component(_)), "preferred entry must be a component");
    let panel = find(doc, "Panel");
    let swap = panel.component.props.iter().find(|p| p.name == "LeadIcon").expect("LeadIcon prop");
    assert_eq!(swap.kind, PropKind::InstanceSwap { preferred: vec![expected] });
    assert_eq!(panel.component.slot_configs.get("Body").map(|c| c.preferred.clone()), Some(vec![expected]));
}

#[test]
fn preferred_key_resolves_to_the_component_even_when_an_instance_of_it_comes_later() {
    let doc = import_fig_node_changes(&preferred_key_with_later_instance_fixture(), "t").unwrap();
    assert_preferred_is(&doc, find(&doc, "Icon").id);
}

#[test]
fn preferred_key_carried_only_by_an_instance_resolves_to_the_instanced_component() {
    let fixture = json!([
        nc((0, 0), (0, 0), "!", "DOCUMENT", "Document", json!({})),
        nc((0, 1), (0, 0), "a", "CANVAS", "Page", json!({})),
        nc((6, 1), (0, 1), "a", "SYMBOL", "Icon", json!({})),
        nc((6, 2), (0, 1), "b", "SYMBOL", "Button", json!({"componentKey": "K_BUTTON"})),
        nc((6, 3), (6, 2), "a", "INSTANCE", "IconInstance", json!({"componentKey": "K_VIA_INSTANCE", "symbolData": {"symbolID": g(6, 1)}})),
        nc(
            (5, 1),
            (0, 1),
            "c",
            "SYMBOL",
            "Panel",
            json!({
                "componentPropDefs": [{
                    "id": g(7, 1),
                    "name": "LeadIcon",
                    "type": "INSTANCE_SWAP",
                    "preferredValues": {"stringValues": [], "instanceSwapValues": [{"type": "COMPONENT", "key": "K_VIA_INSTANCE"}]}
                }]
            })
        )
    ]);
    let doc = import_fig_node_changes(&fixture, "t").unwrap();
    let icon = find(&doc, "Icon").id;
    let panel = find(&doc, "Panel");
    let swap = panel.component.props.iter().find(|p| p.name == "LeadIcon").expect("LeadIcon prop");
    assert_eq!(swap.kind, PropKind::InstanceSwap { preferred: vec![icon] });
}

#[test]
fn preferred_component_survives_import_export_import_when_an_instance_repeats_its_key() {
    let doc = import_fig_node_changes(&preferred_key_with_later_instance_fixture(), "t").unwrap();
    assert_preferred_is(&doc, find(&doc, "Icon").id);

    let rt = import_fig(&export_fig(&doc).expect("export_fig")).expect("import_fig");
    assert_preferred_is(&rt, find(&rt, "Icon").id);
}

#[test]
fn an_instance_never_overrides_the_component_that_owns_the_key() {
    // The instance repeats `K_ICON` but points at a different component (a local stand-in).
    let fixture = json!([
        nc((0, 0), (0, 0), "!", "DOCUMENT", "Document", json!({})),
        nc((0, 1), (0, 0), "a", "CANVAS", "Page", json!({})),
        nc((6, 1), (0, 1), "a", "SYMBOL", "Icon", json!({"componentKey": "K_ICON"})),
        nc((6, 2), (0, 1), "b", "SYMBOL", "StandIn", json!({})),
        nc((6, 3), (6, 2), "a", "INSTANCE", "Nested", json!({"componentKey": "K_ICON", "symbolData": {"symbolID": g(6, 2)}})),
        nc(
            (5, 1),
            (0, 1),
            "c",
            "SYMBOL",
            "Panel",
            json!({
                "componentPropDefs": [{
                    "id": g(7, 1),
                    "name": "LeadIcon",
                    "type": "INSTANCE_SWAP",
                    "preferredValues": {"stringValues": [], "instanceSwapValues": [{"type": "COMPONENT", "key": "K_ICON"}]}
                }]
            })
        )
    ]);
    let doc = import_fig_node_changes(&fixture, "t").unwrap();
    let swap = find(&doc, "Panel").component.props.iter().find(|p| p.name == "LeadIcon").cloned().expect("LeadIcon prop");
    assert_eq!(swap.kind, PropKind::InstanceSwap { preferred: vec![find(&doc, "Icon").id] });
}

fn component_key(doc: &Document, name: &str) -> Option<String> {
    find(doc, name).fig_metadata.get("componentKey").cloned()
}

/// Preferred entries of the `Panel` swap prop and of its slot, by node id.
fn panel_preferred(doc: &Document) -> (Vec<NodeId>, Vec<NodeId>) {
    let panel = find(doc, "Panel");
    let swap = match &panel.component.props.iter().find(|p| p.name == "LeadIcon").expect("LeadIcon prop").kind {
        PropKind::InstanceSwap { preferred } => preferred.clone(),
        other => panic!("LeadIcon is {other:?}"),
    };
    let slot = panel.component.slot_configs.get("Body").map(|c| c.preferred.clone()).unwrap_or_default();
    (swap, slot)
}

fn assert_keys_and_targets_survive(doc: &Document) {
    let (icon, set) = (find(doc, "Icon").id, find(doc, "BadgeSet").id);
    assert_eq!(component_key(doc, "Icon").as_deref(), Some("K_ICON"));
    assert_eq!(component_key(doc, "BadgeSet").as_deref(), Some("K_SET"));
    let (swap, slot) = panel_preferred(doc);
    assert_eq!(swap, vec![icon, set]);
    assert_eq!(slot, vec![icon, set]);
}

#[test]
fn a_key_carried_only_by_the_symbol_key_field_round_trips_through_fig_export() {
    let prefs = json!({"stringValues": [], "instanceSwapValues": [
        {"type": "COMPONENT", "key": "K_ICON"}, {"type": "STATE_GROUP", "key": "K_SET"}]});
    let fixture = json!([
        nc((0, 0), (0, 0), "!", "DOCUMENT", "Document", json!({})),
        nc((0, 1), (0, 0), "a", "CANVAS", "Page", json!({})),
        nc((6, 1), (0, 1), "a", "SYMBOL", "Icon", json!({"key": "K_ICON"})),
        nc((6, 2), (0, 1), "b", "FRAME", "BadgeSet", json!({"isStateGroup": true, "key": "K_SET"})),
        nc((6, 3), (6, 2), "a", "SYMBOL", "Variant=Default", json!({})),
        nc(
            (5, 1),
            (0, 1),
            "c",
            "SYMBOL",
            "Panel",
            json!({"componentPropDefs": [
                {"id": g(7, 1), "name": "LeadIcon", "type": "INSTANCE_SWAP", "preferredValues": prefs},
                {"id": g(7, 2), "name": "Body", "type": "SLOT", "preferredValues": prefs}
            ]})
        )
    ]);
    let doc = import_fig_node_changes(&fixture, "t").unwrap();
    assert_keys_and_targets_survive(&doc);

    let rt = import_fig(&export_fig(&doc).expect("export_fig")).expect("import_fig");
    assert_keys_and_targets_survive(&rt);
    let twice = import_fig(&export_fig(&rt).expect("export_fig")).expect("import_fig");
    assert_keys_and_targets_survive(&twice);
}

#[test]
fn a_key_from_the_rest_component_maps_round_trips_through_fig_export() {
    let prefs = json!({"stringValues": [], "instanceSwapValues": [
        {"type": "COMPONENT", "key": "K_ICON"}, {"type": "STATE_GROUP", "key": "K_SET"}]});
    let file = json!({
        "name": "rest",
        "document": {"id": "0:0", "type": "DOCUMENT", "children": [{"id": "0:1", "type": "CANVAS", "name": "Page", "children": [
            {"id": "1:1", "name": "Icon", "type": "COMPONENT", "size": {"x": 16, "y": 16}},
            {"id": "1:3", "name": "BadgeSet", "type": "COMPONENT_SET", "size": {"x": 40, "y": 16}, "children": [
                {"id": "1:4", "name": "Variant=Default", "type": "COMPONENT", "size": {"x": 40, "y": 16}}
            ]},
            {"id": "1:2", "name": "Panel", "type": "COMPONENT", "size": {"x": 100, "y": 100},
             "componentPropertyDefinitions": {
                "LeadIcon#7:1": {"type": "INSTANCE_SWAP", "defaultValue": "1:1", "preferredValues": prefs},
                "Body#7:2": {"type": "SLOT", "preferredValues": prefs}
             }}
        ]}]},
        "components": {"1:1": {"key": "K_ICON", "name": "Icon"}, "1:4": {"key": "K_DEF", "name": "Variant=Default"}},
        "componentSets": {"1:3": {"key": "K_SET", "name": "BadgeSet"}}
    });
    let doc = import_rest_json(&file.to_string()).unwrap();
    assert_keys_and_targets_survive(&doc);

    let rt = import_fig(&export_fig(&doc).expect("export_fig")).expect("import_fig");
    assert_keys_and_targets_survive(&rt);
}

#[test]
fn an_instance_key_is_not_written_onto_a_component_that_does_not_own_it() {
    // `Nested` repeats `K_ICON` but its symbolID is the stand-in; Icon owns the key.
    let fixture = json!([
        nc((0, 0), (0, 0), "!", "DOCUMENT", "Document", json!({})),
        nc((0, 1), (0, 0), "a", "CANVAS", "Page", json!({})),
        nc((6, 1), (0, 1), "a", "SYMBOL", "Icon", json!({"componentKey": "K_ICON"})),
        nc((6, 2), (0, 1), "b", "SYMBOL", "StandIn", json!({})),
        nc((6, 3), (6, 2), "a", "INSTANCE", "Nested", json!({"componentKey": "K_ICON", "symbolData": {"symbolID": g(6, 2)}})),
        nc(
            (5, 1),
            (0, 1),
            "c",
            "SYMBOL",
            "Panel",
            json!({"componentPropDefs": [{
                "id": g(7, 1), "name": "LeadIcon", "type": "INSTANCE_SWAP",
                "preferredValues": {"stringValues": [], "instanceSwapValues": [{"type": "COMPONENT", "key": "K_ICON"}]}
            }]})
        )
    ]);
    let doc = import_fig_node_changes(&fixture, "t").unwrap();
    let rt = import_fig(&export_fig(&doc).expect("export_fig")).expect("import_fig");
    let (swap, _) = panel_preferred(&rt);
    assert_eq!(swap, vec![find(&rt, "Icon").id], "preferred target stays the owner of the key");
    assert_eq!(component_key(&rt, "Icon").as_deref(), Some("K_ICON"));
    assert_ne!(component_key(&rt, "StandIn").as_deref(), Some("K_ICON"), "StandIn must not take Icon's key");
}

#[test]
fn an_instance_key_never_replaces_the_key_its_component_already_has() {
    // `Inst` instances Icon but carries a different key than the one Icon owns.
    let fixture = json!([
        nc((0, 0), (0, 0), "!", "DOCUMENT", "Document", json!({})),
        nc((0, 1), (0, 0), "a", "CANVAS", "Page", json!({})),
        nc((6, 1), (0, 1), "a", "SYMBOL", "Icon", json!({"componentKey": "K_ICON"})),
        nc((6, 2), (0, 1), "b", "INSTANCE", "Inst", json!({"componentKey": "K_OTHER", "symbolData": {"symbolID": g(6, 1)}})),
        nc(
            (5, 1),
            (0, 1),
            "c",
            "SYMBOL",
            "Panel",
            json!({"componentPropDefs": [{
                "id": g(7, 1), "name": "LeadIcon", "type": "INSTANCE_SWAP",
                "preferredValues": {"stringValues": [], "instanceSwapValues": [{"type": "COMPONENT", "key": "K_ICON"}]}
            }]})
        )
    ]);
    let doc = import_fig_node_changes(&fixture, "t").unwrap();
    let rt = import_fig(&export_fig(&doc).expect("export_fig")).expect("import_fig");
    assert_eq!(component_key(&rt, "Icon").as_deref(), Some("K_ICON"));
    assert_eq!(panel_preferred(&rt).0, vec![find(&rt, "Icon").id]);
}

#[test]
fn two_slot_props_on_one_node_round_trip_separate_slot_content_ids_through_fig() {
    let fixture = json!([
        nc((0, 0), (0, 0), "!", "DOCUMENT", "Document", json!({})),
        nc((0, 1), (0, 0), "a", "CANVAS", "Page", json!({})),
        nc(
            (5, 1),
            (0, 1),
            "a",
            "SYMBOL",
            "Panel",
            json!({
                "componentPropDefs": [
                    {
                        "id": g(7, 1),
                        "name": "Header",
                        "type": "SLOT",
                        "varValue": {
                            "dataType": "SLOT_CONTENT_ID",
                            "value": { "slotContentIdValue": { "guid": g(8, 1) } }
                        }
                    },
                    {
                        "id": g(7, 2),
                        "name": "Body",
                        "type": "SLOT",
                        "varValue": {
                            "dataType": "SLOT_CONTENT_ID",
                            "value": { "slotContentIdValue": { "guid": g(8, 2) } }
                        }
                    }
                ]
            })
        ),
        nc(
            (1, 2),
            (0, 1),
            "b",
            "INSTANCE",
            "Panel copy",
            json!({
                "symbolData": { "symbolID": g(5, 1), "symbolOverrides": [], "uniformScaleFactor": 1.0 },
                "componentPropAssignments": [
                    {
                        "defID": g(7, 1),
                        "varValue": {
                            "dataType": "SLOT_CONTENT_ID",
                            "value": { "slotContentIdValue": { "guid": g(9, 1) } }
                        }
                    },
                    {
                        "defID": g(7, 2),
                        "varValue": {
                            "dataType": "SLOT_CONTENT_ID",
                            "value": { "slotContentIdValue": { "guid": g(9, 2) } }
                        }
                    }
                ]
            })
        )
    ]);

    let doc = import_fig_node_changes(&fixture, "t").unwrap();
    let panel = find(&doc, "Panel");
    assert_eq!(panel.component.slot_content_ids.get("Header").map(String::as_str), Some("8:1"));
    assert_eq!(panel.component.slot_content_ids.get("Body").map(String::as_str), Some("8:2"));

    let inst = find(&doc, "Panel copy");
    assert_eq!(inst.component.slot_content_ids.get("Header").map(String::as_str), Some("9:1"));
    assert_eq!(inst.component.slot_content_ids.get("Body").map(String::as_str), Some("9:2"));

    let rt = import_fig(&export_fig(&doc).unwrap()).unwrap();
    let rt_panel = find(&rt, "Panel");
    assert_eq!(rt_panel.component.slot_content_ids.get("Header").map(String::as_str), Some("8:1"));
    assert_eq!(rt_panel.component.slot_content_ids.get("Body").map(String::as_str), Some("8:2"));

    let rt_inst = find(&rt, "Panel copy");
    assert_eq!(rt_inst.component.slot_content_ids.get("Header").map(String::as_str), Some("9:1"));
    assert_eq!(rt_inst.component.slot_content_ids.get("Body").map(String::as_str), Some("9:2"));
}

/// The node's own `jsonSerialized.slot-content-id` (field a) and the per-property
/// `SLOT_CONTENT_ID` assignment (field b) are different fields. These tests pin that a is never
/// turned into b, and that b is never folded into a.
fn panel_with_one_slot(panel_extra: Value, instance_extra: Value) -> Value {
    let mut panel = json!({"componentPropDefs": [{"id": g(7, 1), "name": "Body", "type": "SLOT"}]});
    for (k, v) in panel_extra.as_object().unwrap() {
        panel[k] = v.clone();
    }
    let mut inst = json!({"symbolData": {"symbolID": g(5, 1), "symbolOverrides": [], "uniformScaleFactor": 1.0}});
    for (k, v) in instance_extra.as_object().unwrap() {
        inst[k] = v.clone();
    }
    json!([
        nc((0, 0), (0, 0), "!", "DOCUMENT", "Document", json!({})),
        nc((0, 1), (0, 0), "a", "CANVAS", "Page", json!({})),
        nc((5, 1), (0, 1), "a", "SYMBOL", "Panel", panel),
        nc((1, 2), (0, 1), "b", "INSTANCE", "Panel copy", inst)
    ])
}

fn node_slot_id(id: (u64, u64)) -> Value {
    json!({"slot-content-id": {"guid": g(id.0, id.1)}})
}

fn assert_no_property_slot_ids(doc: &Document) {
    for name in ["Panel", "Panel copy"] {
        let ids = &find(doc, name).component.slot_content_ids;
        assert!(ids.is_empty(), "{name} gained per-property slot content ids it never had: {ids:?}");
    }
}

#[test]
fn a_node_slot_content_id_without_a_property_assignment_stays_the_nodes_own_field_through_fig() {
    let fixture = panel_with_one_slot(json!({"jsonSerialized": node_slot_id((9, 8))}), json!({"jsonSerialized": node_slot_id((9, 9))}));
    let doc = import_fig_node_changes(&fixture, "t").unwrap();
    assert_no_property_slot_ids(&doc);
    assert_eq!(find(&doc, "Panel").component.slot_content_id.as_deref(), Some("9:8"));
    assert_eq!(find(&doc, "Panel copy").component.slot_content_id.as_deref(), Some("9:9"));

    let rt = import_fig(&export_fig(&doc).expect("export_fig")).expect("import_fig");
    assert_no_property_slot_ids(&rt);
    assert_eq!(find(&rt, "Panel").component.slot_content_id.as_deref(), Some("9:8"), "node field round-trips");
    assert_eq!(find(&rt, "Panel copy").component.slot_content_id.as_deref(), Some("9:9"), "node field round-trips");
}

#[test]
fn a_node_slot_content_id_on_a_one_slot_component_is_not_written_as_a_property_assignment() {
    let mut doc = Document::new("one-slot");
    let page = doc.add_page("Page");
    let id = doc.alloc_id();
    let mut panel = Node::new(id, NodeKind::Component(FrameProps::default()));
    panel.name = "Panel".into();
    panel.component.props = vec![ComponentPropDef { name: "Body#1:1".into(), kind: PropKind::Slot, default: PropValue::Slot(vec![]) }];
    panel.component.slot_content_id = Some("7:7".into());
    doc.insert(page, panel).unwrap();
    let rt = import_fig(&export_fig(&doc).expect("export_fig")).expect("import_fig");
    assert!(find(&rt, "Panel").component.slot_content_ids.is_empty(), "{:?}", find(&rt, "Panel").component.slot_content_ids);
    assert_eq!(find(&rt, "Panel").component.slot_content_id.as_deref(), Some("7:7"));
}

#[test]
fn a_legacy_native_file_keeps_its_node_slot_content_id_and_gains_no_property_assignment() {
    let mut doc = Document::new("legacy");
    let page = doc.add_page("Page");
    let id = doc.alloc_id();
    let mut panel = Node::new(id, NodeKind::Component(FrameProps::default()));
    panel.name = "Panel".into();
    panel.component.props = vec![
        ComponentPropDef { name: "Body#1:1".into(), kind: PropKind::Slot, default: PropValue::Slot(vec![]) },
        ComponentPropDef { name: "Footer#1:2".into(), kind: PropKind::Slot, default: PropValue::Slot(vec![]) },
    ];
    panel.component.slot_content_id = Some("7:42".into());
    doc.insert(page, panel).unwrap();
    let json = doc.to_json().unwrap();
    assert!(!json.contains("slot_content_ids"), "file as saved before the per-property field existed");

    let loaded = Document::from_json(&json).unwrap();
    assert_eq!(find(&loaded, "Panel").component.slot_content_id.as_deref(), Some("7:42"));
    assert!(find(&loaded, "Panel").component.slot_content_ids.is_empty());
    let rt = import_fig(&export_fig(&loaded).expect("export_fig")).expect("import_fig");
    assert!(find(&rt, "Panel").component.slot_content_ids.is_empty(), "{:?}", find(&rt, "Panel").component.slot_content_ids);
    assert_eq!(find(&rt, "Panel").component.slot_content_id.as_deref(), Some("7:42"));
}

fn slot_assignment(def: (u64, u64), id: (u64, u64)) -> Value {
    json!({"defID": g(def.0, def.1), "varValue": {"dataType": "SLOT_CONTENT_ID", "value": {"slotContentIdValue": {"guid": g(id.0, id.1)}}}})
}

#[test]
fn per_property_slot_assignments_and_the_node_field_stay_separate_through_fig() {
    let fixture = json!([
        nc((0, 0), (0, 0), "!", "DOCUMENT", "Document", json!({})),
        nc((0, 1), (0, 0), "a", "CANVAS", "Page", json!({})),
        nc(
            (5, 1),
            (0, 1),
            "a",
            "SYMBOL",
            "Panel",
            json!({"componentPropDefs": [
                {"id": g(7, 1), "name": "Header", "type": "SLOT"},
                {"id": g(7, 2), "name": "Body", "type": "SLOT"}]})
        ),
        nc(
            (1, 2),
            (0, 1),
            "b",
            "INSTANCE",
            "Panel copy",
            json!({
                "symbolData": {"symbolID": g(5, 1), "symbolOverrides": [], "uniformScaleFactor": 1.0},
                "jsonSerialized": node_slot_id((9, 9)),
                "componentPropAssignments": [slot_assignment((7, 1), (9, 1)), slot_assignment((7, 2), (9, 2))]
            })
        )
    ]);
    let doc = import_fig_node_changes(&fixture, "t").unwrap();
    let rt = import_fig(&export_fig(&doc).expect("export_fig")).expect("import_fig");
    for d in [&doc, &rt] {
        let inst = find(d, "Panel copy");
        assert_eq!(inst.component.slot_content_id.as_deref(), Some("9:9"), "node field is not replaced by a property id");
        assert_eq!(inst.component.slot_content_ids.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect::<Vec<_>>(), vec![("Body", "9:2"), ("Header", "9:1")]);
        assert!(find(d, "Panel").component.slot_content_ids.is_empty());
        assert!(find(d, "Panel").component.slot_content_id.is_none());
    }
}

#[test]
fn a_fresh_instance_of_a_two_slot_component_adds_no_slot_assignments_through_fig() {
    let fixture = json!([
        nc((0, 0), (0, 0), "!", "DOCUMENT", "Document", json!({})),
        nc((0, 1), (0, 0), "a", "CANVAS", "Page", json!({})),
        nc(
            (5, 1),
            (0, 1),
            "a",
            "SYMBOL",
            "Panel",
            json!({
                "componentPropDefs": [
                    { "id": g(7, 1), "name": "Header", "type": "SLOT",
                      "varValue": { "dataType": "SLOT_CONTENT_ID", "value": { "slotContentIdValue": { "guid": g(8, 1) } } } },
                    { "id": g(7, 2), "name": "Body", "type": "SLOT",
                      "varValue": { "dataType": "SLOT_CONTENT_ID", "value": { "slotContentIdValue": { "guid": g(8, 2) } } } }
                ]
            })
        )
    ]);
    let mut doc = import_fig_node_changes(&fixture, "t").unwrap();
    let panel_id = find(&doc, "Panel").id;
    let page = doc.pages[0];
    create_instance(&mut doc, panel_id, page, Vec2::new(400.0, 0.0)).apply(&mut doc).unwrap();
    let inst_id = instances_of(&doc, panel_id)[0];
    doc.get_mut(inst_id).unwrap().name = "Fresh".into();

    let rt = import_fig(&export_fig(&doc).unwrap()).unwrap();
    let panel = find(&rt, "Panel");
    assert_eq!(panel.component.slot_content_ids.get("Header").map(String::as_str), Some("8:1"));
    assert_eq!(panel.component.slot_content_ids.get("Body").map(String::as_str), Some("8:2"));
    let fresh = find(&rt, "Fresh");
    assert!(fresh.component.slot_content_ids.is_empty(), "{:?}", fresh.component.slot_content_ids);
}

#[test]
fn exported_fig_uses_figmas_real_kiwi_numbering_for_component_prop_type_and_node_change_tags() {
    let doc = slot_document();
    let bytes = export_fig(&doc).unwrap();
    let archive = designos_fig::container::open(&bytes).unwrap();
    let (schema_bytes, _) = designos_fig::container::read_archive_chunks(&archive).unwrap();
    let schema = designos_fig::kiwi::Schema::decode(&schema_bytes).unwrap();

    let prop_type = schema.find("ComponentPropType").expect("ComponentPropType enum exists");
    let variant_val = |def_idx: usize, name: &str| -> Option<u32> { schema.defs[def_idx].fields.iter().find(|f| f.name.as_ref() == name).map(|f| f.value) };
    assert_eq!(variant_val(prop_type, "BOOL"), Some(0));
    assert_eq!(variant_val(prop_type, "TEXT"), Some(1));
    assert_eq!(variant_val(prop_type, "INSTANCE_SWAP"), Some(3));
    assert_eq!(variant_val(prop_type, "VARIANT"), Some(4));
    assert_eq!(variant_val(prop_type, "SLOT"), Some(7), "Figma ComponentPropType.SLOT must be 7");

    let node_field = schema.find("ComponentPropNodeField").expect("ComponentPropNodeField enum exists");
    assert_eq!(variant_val(node_field, "VISIBLE"), Some(0));
    assert_eq!(variant_val(node_field, "TEXT_DATA"), Some(1));
    assert_eq!(variant_val(node_field, "OVERRIDDEN_SYMBOL_ID"), Some(2));
    assert_eq!(variant_val(node_field, "SLOT_CONTENT_ID"), Some(4), "Figma ComponentPropNodeField.SLOT_CONTENT_ID must be 4");

    let nc_def = schema.find("NodeChange").expect("NodeChange message exists");
    for (field_name, expected_tag) in [
        ("guid", 1),
        ("phase", 2),
        ("parentIndex", 3),
        ("type", 4),
        ("name", 5),
        ("visible", 6),
        ("locked", 7),
        ("opacity", 8),
        ("blendMode", 9),
        ("size", 11),
        ("transform", 12),
        ("fillPaints", 38),
        ("strokePaints", 39),
        ("effects", 43),
        ("symbolData", 113),
        ("derivedSymbolData", 125),
        ("overriddenSymbolID", 143),
        ("symbolDescription", 144),
        ("overrideKey", 213),
        ("stateGroupPropertyValueOrders", 238),
        ("componentPropDefs", 266),
        ("componentPropRefs", 267),
        ("componentPropAssignments", 268),
        ("propsAreBubbled", 305),
        ("overrideLevel", 321),
        ("detachedSymbolId", 342),
        ("codeSyntax", 358),
        ("isSlot", 463),
        ("variantPropSpecs", 483),
        ("isSlotContent", 495),
    ] {
        assert_eq!(variant_val(nc_def, field_name), Some(expected_tag), "NodeChange.{field_name} must use Figma's real Kiwi tag {expected_tag}");
    }

    // Re-importing the exported .fig encoded with real Kiwi numbering must round-trip all slot data.
    let rt = import_fig(&bytes).unwrap();
    assert!(find(&rt, "BodySlot").component.is_slot);
    assert_eq!(find(&rt, "BodySlot").component.bindings.get(&OverrideField::SlotContent).map(String::as_str), Some("Body"));
    assert!(find(&rt, "Banner").component.is_slot_content);
}

/// Slot record counts of one decoded Kiwi message.
#[derive(Debug, Default, PartialEq, Eq)]
struct SlotCounts {
    node_changes: usize,
    /// `isSlot` written as `true` (field 463) / written at all.
    is_slot_true: usize,
    is_slot_present: usize,
    /// `isSlotContent` written as `true` (field 495) / written at all.
    is_slot_content_true: usize,
    is_slot_content_present: usize,
    /// Control: every `componentPropDefs` entry of any type, so a zero slot result is known to come
    /// from a scan that does read component properties.
    prop_defs: usize,
    /// `componentPropDefs` of type `SLOT`, and those carrying a `slotPropConfig`.
    slot_props: usize,
    slot_prop_configs: usize,
    /// `componentPropRefs` binding `SLOT_CONTENT_ID`.
    slot_content_id_refs: usize,
    /// `componentPropAssignments` whose `defID` names a `SLOT` property definition (the path
    /// `fig.rs` reads and `fig_export.rs` writes).
    slot_assignments: usize,
    /// Nodes holding `jsonSerialized` -> `slot-content-id` (the instance slot content guid).
    slot_content_id_bags: usize,
}

/// `sessionID` / `localID` pair of a Kiwi `GUID` struct.
fn guid_key(v: &designos_fig::kiwi::KiwiValue) -> Option<(u64, u64)> {
    use designos_fig::kiwi::KiwiValue::{Uint, Uint64};
    let n = |f: &str| match v.get(f)? {
        Uint(x) => Some(u64::from(*x)),
        Uint64(x) => Some(*x),
        _ => None,
    };
    Some((n("sessionID")?, n("localID")?))
}

fn scan_slots(msg: &designos_fig::kiwi::KiwiValue) -> SlotCounts {
    use designos_fig::kiwi::KiwiValue;
    use std::collections::HashSet;
    let slot_type = KiwiValue::Enum("SLOT".into());
    let changes = msg.get("nodeChanges").and_then(KiwiValue::as_array).unwrap_or(&[]);
    let mut c = SlotCounts { node_changes: changes.len(), ..Default::default() };
    let defs_of = |nc: &KiwiValue| nc.get("componentPropDefs").and_then(KiwiValue::as_array).unwrap_or(&[]).to_vec();
    // Definitions live on the component; assignments on its instances, so collect the ids first.
    let mut slot_def_ids = HashSet::new();
    for nc in changes {
        for d in defs_of(nc) {
            c.prop_defs += 1;
            if d.get("type") == Some(&slot_type) {
                c.slot_props += 1;
                c.slot_prop_configs += usize::from(d.get("slotPropConfig").is_some());
                slot_def_ids.extend(d.get("id").and_then(guid_key));
            }
        }
    }
    for nc in changes {
        if let Some(v) = nc.get("isSlot") {
            c.is_slot_present += 1;
            c.is_slot_true += usize::from(v == &KiwiValue::Bool(true));
        }
        if let Some(v) = nc.get("isSlotContent") {
            c.is_slot_content_present += 1;
            c.is_slot_content_true += usize::from(v == &KiwiValue::Bool(true));
        }
        for r in nc.get("componentPropRefs").and_then(KiwiValue::as_array).unwrap_or(&[]) {
            c.slot_content_id_refs += usize::from(r.get("componentPropNodeField") == Some(&KiwiValue::Enum("SLOT_CONTENT_ID".into())));
        }
        for a in nc.get("componentPropAssignments").and_then(KiwiValue::as_array).unwrap_or(&[]) {
            c.slot_assignments += usize::from(a.get("defID").and_then(guid_key).is_some_and(|id| slot_def_ids.contains(&id)));
        }
        c.slot_content_id_bags += usize::from(nc.get("jsonSerialized").and_then(|j| j.get("slot-content-id")).and_then(|s| s.get("guid")).is_some());
    }
    c
}

/// Opens a `.fig` and decodes its root message the way import does (drops the multi-gigabyte
/// derived fields on large files). Returns the embedded schema next to the message.
fn decode_fig_for_scan(bytes: &[u8]) -> (designos_fig::kiwi::Schema, designos_fig::kiwi::KiwiValue) {
    use designos_fig::{container, kiwi::Schema};
    let archive = container::open(bytes).expect("open .fig container");
    let (schema_bin, msg_bin) = container::read_archive_chunks(&archive).expect("read archive chunks");
    let schema = Schema::decode(&schema_bin).expect("decode embedded Kiwi schema");
    let root = schema.root_message().expect("root message");
    let msg = schema.decode_message_for_import(root, &msg_bin).expect("decode message");
    (schema, msg)
}

/// Field tag of `def.field` in a decoded schema.
fn schema_tag(schema: &designos_fig::kiwi::Schema, def: &str, field: &str) -> Option<u32> {
    let d = schema.defs.iter().find(|d| d.name == def)?;
    d.fields.iter().find(|f| f.name.as_ref() == field).map(|f| f.value)
}

#[test]
fn the_slot_scan_finds_every_slot_record_a_fig_export_writes() {
    let (schema, msg) = decode_fig_for_scan(&export_fig(&slot_document()).expect("export_fig"));
    // The testkit schema numbers its own fields; only the names are shared with a real file.
    assert!(schema_tag(&schema, "NodeChange", "isSlot").is_some() && schema_tag(&schema, "NodeChange", "isSlotContent").is_some());
    let c = scan_slots(&msg);
    // Two slot frames: the component's own and its instance's copy, both bound to the property.
    let expected = SlotCounts {
        node_changes: 8,
        is_slot_true: 2,
        is_slot_present: 2,
        is_slot_content_true: 1,
        is_slot_content_present: 1,
        prop_defs: 2,
        slot_props: 1,
        slot_prop_configs: 1,
        slot_content_id_refs: 2,
        slot_assignments: 1,
        // The export's schema is a subset of Figma's real one, which has no `jsonSerialized`: the
        // node's own slot content id travels in Studio's plugin data instead (counted below).
        slot_content_id_bags: 0,
    };
    assert_eq!(c, expected, "a scan that cannot see slots would make a zero result meaningless");
    assert_eq!(count_carried_slot_content_ids(&msg), 1, "the node's own slot content id is written once, in plugin data");
}

fn is_carried_slot_content_id_entry(e: &designos_fig::kiwi::KiwiValue) -> bool {
    let key = match e.get("key") {
        Some(designos_fig::kiwi::KiwiValue::String(s)) => s.as_str(),
        _ => return false,
    };
    let plugin_id = match e.get("pluginID") {
        Some(designos_fig::kiwi::KiwiValue::String(s)) => Some(s.as_str()),
        _ => None,
    };
    (key == "slot-content-id" && plugin_id.unwrap_or("designos-studio") == "designos-studio") || key == "designos-studio:slot-content-id"
}

fn count_carried_slot_content_ids(msg: &designos_fig::kiwi::KiwiValue) -> usize {
    msg.get("nodeChanges")
        .and_then(designos_fig::kiwi::KiwiValue::as_array)
        .unwrap_or(&[])
        .iter()
        .flat_map(|nc| nc.get("pluginData").and_then(designos_fig::kiwi::KiwiValue::as_array).unwrap_or(&[]).iter())
        .filter(|e| is_carried_slot_content_id_entry(e))
        .count()
}

#[test]
fn slot_scan_counts_only_the_exact_key() {
    use designos_fig::kiwi::KiwiValue as V;
    use designos_fig::testkit::msg;
    let entry = |plugin_id: &str, key: &str| -> V {
        msg(vec![("pluginID", V::String(plugin_id.into())), ("key", V::String(key.into())), ("value", V::String("7:42".into()))])
    };
    let key_only = |key: &str| -> V { msg(vec![("key", V::String(key.into())), ("value", V::String("7:42".into()))]) };
    assert!(!is_carried_slot_content_id_entry(&entry("designos-studio", "my-slot-content-id")));
    assert!(!is_carried_slot_content_id_entry(&entry("other", "slot-content-id")));
    assert!(!is_carried_slot_content_id_entry(&key_only("my-slot-content-id")));
    assert!(!is_carried_slot_content_id_entry(&key_only("other:slot-content-id")));
    assert!(!is_carried_slot_content_id_entry(&key_only("designos-studio:slot-content-id-extra")));
    assert!(is_carried_slot_content_id_entry(&entry("designos-studio", "slot-content-id")));
    assert!(is_carried_slot_content_id_entry(&key_only("designos-studio:slot-content-id")));

    let root = msg(vec![(
        "nodeChanges",
        V::Array(vec![msg(vec![(
            "pluginData",
            V::Array(vec![
                entry("designos-studio", "my-slot-content-id"),
                entry("other", "slot-content-id"),
                key_only("other:slot-content-id"),
                entry("designos-studio", "slot-content-id"),
            ]),
        )])]),
    )]);
    assert_eq!(count_carried_slot_content_ids(&root), 1);
}

/// Where the owner's private `VSF - PCP.fig` lives: `DESIGNOS_VSF_PCP`, else `$HOME/Downloads`.
fn vsf_pcp_path() -> Option<std::path::PathBuf> {
    if let Some(p) = std::env::var_os("DESIGNOS_VSF_PCP") {
        let p = std::path::PathBuf::from(p);
        assert!(p.is_file(), "DESIGNOS_VSF_PCP is set but {} is not a file", p.display());
        return Some(p);
    }
    let p = std::path::PathBuf::from(std::env::var_os("HOME")?).join("Downloads").join("VSF - PCP.fig");
    p.is_file().then_some(p)
}

/// #160 AC1/AC2 on the owner's `VSF - PCP.fig` (209 MB export of 2026-10-14; the 124 MB
/// `VSF - PCP (1).fig` measures the same slot counts). Measured: no slot record of any kind in
/// 155,297 node changes, although the file carries 4,481 other component property definitions.
/// Zero is the AC2 outcome: the file is not a slot reference and no fixture is derived from it
/// (the hand-written slot fixtures above stay the only slot data). If a newer export of the file
/// carries slots this fails, which is the cue to derive a content-free fixture of that shape.
/// Skipped when the file is absent (CI). Only counts are asserted; nothing from the file is committed.
#[test]
fn vsf_pcp_fig_slot_counts_are_the_measured_ones() {
    let Some(path) = vsf_pcp_path() else {
        eprintln!("SKIPPED: VSF - PCP.fig not found (set DESIGNOS_VSF_PCP or place it in ~/Downloads)");
        return;
    };
    let bytes = std::fs::read(&path).expect("read VSF - PCP.fig");
    let (schema, msg) = decode_fig_for_scan(&bytes);
    assert_eq!(schema_tag(&schema, "NodeChange", "isSlot"), Some(463));
    assert_eq!(schema_tag(&schema, "NodeChange", "isSlotContent"), Some(495));
    assert_eq!(schema_tag(&schema, "ComponentPropType", "SLOT"), Some(7));
    let counts = scan_slots(&msg);
    eprintln!("VSF - PCP.fig slot counts: {counts:?}");
    let measured = SlotCounts { node_changes: 155_297, prop_defs: 4_481, ..Default::default() };
    assert_eq!(counts, measured, "VSF - PCP.fig changed; re-measure and apply #160 AC2/AC3");
}
