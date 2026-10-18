//! Slot data on [`ComponentLink`]: compact serialization, instance copies and garbage collection.

use crate::{
    create_component, create_instance, delete_prop, duplicate_subtree, instances_of, rename_prop, reset_instance, sync_instances, ComponentPropDef, Document,
    Node, NodeId, NodeKind, PropKind, PropValue, Rect, SlotPropConfig, Vec2,
};

/// A component frame holding a slot frame, with a slot property, its settings and a preferred component.
fn component_with_slot() -> (Document, NodeId, NodeId, NodeId) {
    let mut d = Document::new("slots");
    let page = d.add_page("P");
    let id = d.alloc_id();
    let icon = d.insert(page, Node::frame(id, Rect::new(0.0, 0.0, 16.0, 16.0))).unwrap();
    create_component(&mut d, icon).apply(&mut d).unwrap();

    let id = d.alloc_id();
    let comp = d.insert(page, Node::frame(id, Rect::new(0.0, 100.0, 200.0, 100.0))).unwrap();
    let id = d.alloc_id();
    let mut body = Node::frame(id, Rect::new(0.0, 0.0, 200.0, 80.0));
    body.name = "body".into();
    let body = d.insert(comp, body).unwrap();
    create_component(&mut d, comp).apply(&mut d).unwrap();

    let node = d.get_mut(comp).unwrap();
    node.component.props.push(ComponentPropDef { name: "Body".into(), kind: PropKind::Slot, default: PropValue::Slot(vec![]) });
    node.component.slot_configs.insert("Body".into(), SlotPropConfig { max_children: Some(2), preferred: vec![icon], ..Default::default() });
    (d, comp, body, icon)
}

#[test]
fn a_node_without_slot_data_serializes_none_of_it() {
    let (d, ..) = component_with_slot();
    let json = d.to_json().unwrap();
    assert_eq!(json.matches("slot_configs").count(), 1, "only the component that has settings writes the key");
    assert!(!json.contains("is_slot"), "unset flags are not written");
    assert!(!json.contains("slot_content_id"));
}

#[test]
fn slot_data_survives_the_native_json_round_trip() {
    let (mut d, comp, body, _) = component_with_slot();
    d.get_mut(body).unwrap().component.is_slot = true;
    d.get_mut(comp).unwrap().component.slot_content_id = Some("7:42".into());
    let loaded = Document::from_json(&d.to_json().unwrap()).unwrap();
    assert_eq!(loaded.get(comp).unwrap().component.slot_configs, d.get(comp).unwrap().component.slot_configs);
    assert!(loaded.get(body).unwrap().component.is_slot);
    assert_eq!(loaded.get(comp).unwrap().component.slot_content_id.as_deref(), Some("7:42"));
}

#[test]
fn an_instance_gets_its_own_slot_frame_flag_but_not_the_components_settings() {
    let (mut d, comp, body, _) = component_with_slot();
    d.get_mut(body).unwrap().component.is_slot = true;
    let page = d.pages[0];
    create_instance(&mut d, comp, page, Vec2::new(400.0, 0.0)).apply(&mut d).unwrap();
    let inst = instances_of(&d, comp)[0];
    let inst_body = d.get(inst).unwrap().children[0];
    assert!(d.get(inst_body).unwrap().component.is_slot, "the instance's copy of the slot frame is a slot");
    assert!(d.get(inst).unwrap().component.slot_configs.is_empty(), "settings belong to the definition, not to the instance");

    // Turning the flag off on the component reaches the instance on the next sync.
    d.get_mut(body).unwrap().component.is_slot = false;
    sync_instances(&mut d, comp).apply(&mut d).unwrap();
    assert!(!d.get(inst_body).unwrap().component.is_slot);
    assert!(matches!(d.get(inst).unwrap().kind, NodeKind::Instance { .. }));
}

#[test]
fn garbage_collection_keeps_a_component_that_only_a_slot_prefers() {
    let (mut d, comp, _, icon) = component_with_slot();
    // Detach the icon from the page: only the slot settings still name it.
    let page = d.pages[0];
    d.get_mut(page).unwrap().children.retain(|c| *c != icon);
    d.get_mut(icon).unwrap().parent = None;
    d.library_page = None;
    d.prune_unreachable();
    assert!(d.get(icon).is_some(), "preferred slot component is reachable through the slot settings");
    assert!(d.get(comp).is_some());
}

fn body_config() -> SlotPropConfig {
    SlotPropConfig { max_children: Some(2), ..Default::default() }
}

#[test]
fn renaming_a_slot_property_carries_its_settings_along() {
    let (mut d, comp, _, icon) = component_with_slot();
    rename_prop(&mut d, comp, "Body", "Content").apply(&mut d).unwrap();
    let n = d.get(comp).unwrap();
    assert!(n.component.props.iter().any(|p| p.name == "Content" && p.kind == PropKind::Slot));
    assert_eq!(n.component.slot_configs.get("Content"), Some(&SlotPropConfig { preferred: vec![icon], ..body_config() }));
    assert!(!n.component.slot_configs.contains_key("Body"), "no settings are left under the old name");
}

#[test]
fn renaming_a_slot_property_can_be_undone() {
    let (mut d, comp, _, _) = component_with_slot();
    let before = d.get(comp).unwrap().component.slot_configs.clone();
    let inverse = rename_prop(&mut d, comp, "Body", "Content").apply(&mut d).unwrap();
    assert_eq!(d.get(comp).unwrap().component.slot_configs.keys().collect::<Vec<_>>(), ["Content"]);
    inverse.apply(&mut d).unwrap();
    assert_eq!(d.get(comp).unwrap().component.slot_configs, before);
}

#[test]
fn deleting_a_slot_property_drops_its_settings() {
    let (mut d, comp, _, _) = component_with_slot();
    delete_prop(&mut d, comp, "Body").apply(&mut d).unwrap();
    assert!(d.get(comp).unwrap().component.props.is_empty());
    assert!(d.get(comp).unwrap().component.slot_configs.is_empty(), "settings of a deleted property are gone");
}

#[test]
fn a_deleted_slot_property_no_longer_keeps_its_preferred_component_alive() {
    let (mut d, comp, _, icon) = component_with_slot();
    delete_prop(&mut d, comp, "Body").apply(&mut d).unwrap();
    // Detach the icon from the page: the slot settings were the only thing still naming it.
    let page = d.pages[0];
    d.get_mut(page).unwrap().children.retain(|c| *c != icon);
    d.get_mut(icon).unwrap().parent = None;
    d.library_page = None;
    d.prune_unreachable();
    assert!(d.get(icon).is_none());
    assert!(d.get(comp).is_some());
}

#[test]
fn deleting_a_slot_property_can_be_undone() {
    let (mut d, comp, _, _) = component_with_slot();
    let before = d.get(comp).unwrap().component.slot_configs.clone();
    let inverse = delete_prop(&mut d, comp, "Body").apply(&mut d).unwrap();
    assert!(d.get(comp).unwrap().component.slot_configs.is_empty());
    inverse.apply(&mut d).unwrap();
    assert_eq!(d.get(comp).unwrap().component.slot_configs, before);
}

#[test]
fn settings_of_the_other_slot_properties_are_not_touched() {
    let (mut d, comp, _, icon) = component_with_slot();
    let node = d.get_mut(comp).unwrap();
    node.component.props.push(ComponentPropDef { name: "Footer".into(), kind: PropKind::Slot, default: PropValue::Slot(vec![]) });
    node.component.slot_configs.insert("Footer".into(), SlotPropConfig { min_children: Some(1), ..Default::default() });
    rename_prop(&mut d, comp, "Body", "Content").apply(&mut d).unwrap();
    delete_prop(&mut d, comp, "Content").apply(&mut d).unwrap();
    let n = d.get(comp).unwrap();
    assert_eq!(n.component.slot_configs.len(), 1);
    assert_eq!(n.component.slot_configs.get("Footer"), Some(&SlotPropConfig { min_children: Some(1), ..Default::default() }));
    assert!(n.component.slot_configs.values().all(|c| !c.preferred.contains(&icon)));
}

#[test]
fn duplicate_subtree_clears_slot_content_id_so_copies_do_not_share_a_guid() {
    let (mut d, comp, body, _) = component_with_slot();
    d.get_mut(comp).unwrap().component.slot_content_id = Some("9:9".into());
    d.get_mut(body).unwrap().component.slot_content_id = Some("9:10".into());

    let page = d.pages[0];
    create_instance(&mut d, comp, page, Vec2::new(300.0, 100.0)).apply(&mut d).unwrap();
    let inst = instances_of(&d, comp)[0];
    d.get_mut(inst).unwrap().component.slot_content_id = Some("12:34".into());
    let inst_body = d.get(inst).unwrap().children[0];
    d.get_mut(inst_body).unwrap().component.slot_content_id = Some("12:35".into());

    let copied = duplicate_subtree(&mut d, inst);
    assert_eq!(copied.len(), 2);
    assert_eq!(d.get(inst).unwrap().component.slot_content_id.as_deref(), Some("12:34"));
    assert_eq!(copied[0].component.slot_content_id, None, "duplicated instance must not share slot_content_id with original");
    assert_eq!(copied[1].component.slot_content_id, None, "duplicated child must not share slot_content_id with original");
}

#[test]
fn sync_instances_propagates_is_slot_content_flipped_on_the_component() {
    let (mut d, comp, body, _) = component_with_slot();
    let page = d.pages[0];
    create_instance(&mut d, comp, page, Vec2::new(300.0, 100.0)).apply(&mut d).unwrap();
    let inst = instances_of(&d, comp)[0];
    let inst_body = d.get(inst).unwrap().children[0];
    assert!(!d.get(inst_body).unwrap().component.is_slot_content);

    d.get_mut(body).unwrap().component.is_slot_content = true;
    sync_instances(&mut d, comp).apply(&mut d).unwrap();
    assert!(d.get(inst_body).unwrap().component.is_slot_content);
}

#[test]
fn two_slot_props_on_one_node_keep_separate_slot_content_ids_and_rename_or_delete_cleanly() {
    let (mut d, comp, _, _) = component_with_slot();
    let page = d.pages[0];
    let c1 = d.alloc_id();
    let c1 = d.insert(page, Node::frame(c1, Rect::new(0.0, 0.0, 40.0, 20.0))).unwrap();
    let c2 = d.alloc_id();
    let c2 = d.insert(page, Node::frame(c2, Rect::new(0.0, 30.0, 40.0, 20.0))).unwrap();

    {
        let node = d.get_mut(comp).unwrap();
        node.component.props.push(ComponentPropDef { name: "Footer".into(), kind: PropKind::Slot, default: PropValue::Slot(vec![]) });
        node.component.slot_content_ids.insert("Body".into(), "7:10".into());
        node.component.slot_content_ids.insert("Footer".into(), "7:20".into());
    }
    create_instance(&mut d, comp, page, Vec2::new(400.0, 0.0)).apply(&mut d).unwrap();
    let inst = instances_of(&d, comp)[0];
    {
        let inst_node = d.get_mut(inst).unwrap();
        inst_node.component.values.insert("Body".into(), PropValue::Slot(vec![c1]));
        inst_node.component.values.insert("Footer".into(), PropValue::Slot(vec![c2]));
        inst_node.component.slot_content_ids.insert("Body".into(), "9:10".into());
        inst_node.component.slot_content_ids.insert("Footer".into(), "9:20".into());
    }

    let loaded = Document::from_json(&d.to_json().unwrap()).unwrap();
    assert_eq!(loaded.get(comp).unwrap().component.slot_content_ids.get("Body").map(String::as_str), Some("7:10"));
    assert_eq!(loaded.get(comp).unwrap().component.slot_content_ids.get("Footer").map(String::as_str), Some("7:20"));
    assert_eq!(loaded.get(inst).unwrap().component.slot_content_ids.get("Body").map(String::as_str), Some("9:10"));
    assert_eq!(loaded.get(inst).unwrap().component.slot_content_ids.get("Footer").map(String::as_str), Some("9:20"));

    // Renaming "Body" -> "Header" moves its slot_content_id on both component and instance without disturbing "Footer".
    rename_prop(&mut d, comp, "Body", "Header").apply(&mut d).unwrap();
    assert_eq!(d.get(comp).unwrap().component.slot_content_ids.get("Header").map(String::as_str), Some("7:10"));
    assert_eq!(d.get(comp).unwrap().component.slot_content_ids.get("Footer").map(String::as_str), Some("7:20"));
    assert!(!d.get(comp).unwrap().component.slot_content_ids.contains_key("Body"));
    assert_eq!(d.get(inst).unwrap().component.slot_content_ids.get("Header").map(String::as_str), Some("9:10"));
    assert_eq!(d.get(inst).unwrap().component.slot_content_ids.get("Footer").map(String::as_str), Some("9:20"));
    assert!(!d.get(inst).unwrap().component.slot_content_ids.contains_key("Body"));

    // Deleting "Header" removes only "Header"'s slot_content_id and leaves "Footer" intact.
    delete_prop(&mut d, comp, "Header").apply(&mut d).unwrap();
    assert!(!d.get(comp).unwrap().component.slot_content_ids.contains_key("Header"));
    assert_eq!(d.get(comp).unwrap().component.slot_content_ids.get("Footer").map(String::as_str), Some("7:20"));
    assert!(!d.get(inst).unwrap().component.slot_content_ids.contains_key("Header"));
    assert_eq!(d.get(inst).unwrap().component.slot_content_ids.get("Footer").map(String::as_str), Some("9:20"));
}

/// The node's own `slot_content_id` (Figma `jsonSerialized.slot-content-id`) is a different field
/// from the per-property `slot_content_ids`. Nothing derives one from the other.
fn component_with_two_slots_and_a_node_id() -> (Document, NodeId) {
    let (mut d, comp, _, _) = component_with_slot();
    let node = d.get_mut(comp).unwrap();
    node.component.props = vec![
        ComponentPropDef { name: "Footer".into(), kind: PropKind::Slot, default: PropValue::Slot(vec![]) },
        ComponentPropDef { name: "Body".into(), kind: PropKind::Slot, default: PropValue::Slot(vec![]) },
    ];
    node.component.slot_content_id = Some("7:1".into());
    (d, comp)
}

#[test]
fn a_file_saved_with_only_the_node_slot_content_id_loads_it_untouched() {
    let (mut d, comp) = component_with_two_slots_and_a_node_id();
    let page = d.pages[0];
    create_instance(&mut d, comp, page, Vec2::new(400.0, 0.0)).apply(&mut d).unwrap();
    let inst = instances_of(&d, comp)[0];
    d.get_mut(inst).unwrap().component.slot_content_id = Some("9:88".into());

    let legacy_json = d.to_json().unwrap();
    assert!(legacy_json.contains("\"slot_content_id\""));
    assert!(!legacy_json.contains("\"slot_content_ids\""));

    let loaded = Document::from_json(&legacy_json).unwrap();
    for (id, want) in [(comp, "7:1"), (inst, "9:88")] {
        let c = &loaded.get(id).unwrap().component;
        assert_eq!(c.slot_content_id.as_deref(), Some(want), "the node's own id is kept");
        assert!(c.slot_content_ids.is_empty(), "no per-property id is invented: {:?}", c.slot_content_ids);
    }
}

#[test]
fn deleting_a_property_never_touches_the_nodes_own_slot_content_id() {
    // Even when a property id equals the node id, the node field is a separate fact.
    let (mut d, comp) = component_with_two_slots_and_a_node_id();
    {
        let n = d.get_mut(comp).unwrap();
        n.component.slot_content_ids.insert("Footer".into(), "7:1".into());
        n.component.slot_content_ids.insert("Body".into(), "8:2".into());
    }
    delete_prop(&mut d, comp, "Footer").apply(&mut d).unwrap();
    let n = &d.get(comp).unwrap().component;
    assert_eq!(n.slot_content_id.as_deref(), Some("7:1"));
    assert_eq!(n.slot_content_ids.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect::<Vec<_>>(), vec![("Body", "8:2")]);

    // A property holding a different id leaves it alone as well.
    let (mut d, comp) = component_with_two_slots_and_a_node_id();
    {
        let n = d.get_mut(comp).unwrap();
        n.component.slot_content_ids.insert("Body".into(), "8:2".into());
        n.component.slot_content_ids.insert("Footer".into(), "8:3".into());
    }
    delete_prop(&mut d, comp, "Body").apply(&mut d).unwrap();
    let n = &d.get(comp).unwrap().component;
    assert_eq!(n.slot_content_id.as_deref(), Some("7:1"));
    assert_eq!(n.slot_content_ids.get("Footer").map(String::as_str), Some("8:3"));
}

#[test]
fn deleting_the_only_or_an_idless_slot_keeps_the_nodes_own_slot_content_id() {
    let (mut d, comp, _, _) = component_with_slot();
    d.get_mut(comp).unwrap().component.slot_content_id = Some("7:42".into());
    delete_prop(&mut d, comp, "Body").apply(&mut d).unwrap();
    assert_eq!(d.get(comp).unwrap().component.slot_content_id.as_deref(), Some("7:42"));

    let (mut d, comp) = component_with_two_slots_and_a_node_id();
    delete_prop(&mut d, comp, "Body").apply(&mut d).unwrap();
    assert_eq!(d.get(comp).unwrap().component.slot_content_id.as_deref(), Some("7:1"));
}

#[test]
fn deleting_a_property_leaves_the_slot_content_id_on_instances_alone() {
    let (mut d, comp) = component_with_two_slots_and_a_node_id();
    let page = d.pages[0];
    create_instance(&mut d, comp, page, Vec2::new(400.0, 0.0)).apply(&mut d).unwrap();
    let inst = instances_of(&d, comp)[0];
    {
        let n = d.get_mut(inst).unwrap();
        n.component.values.clear();
        n.component.values.insert("Footer".into(), PropValue::Slot(vec![]));
        n.component.slot_content_id = Some("9:1".into());
        n.component.slot_content_ids.insert("Footer".into(), "9:1".into());
        n.component.slot_content_ids.insert("Body".into(), "9:2".into());
    }
    delete_prop(&mut d, comp, "Footer").apply(&mut d).unwrap();
    let n = &d.get(inst).unwrap().component;
    assert_eq!(n.slot_content_id.as_deref(), Some("9:1"));
    assert_eq!(n.slot_content_ids.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect::<Vec<_>>(), vec![("Body", "9:2")]);
}

fn component_with_slot_definition_ids() -> (Document, NodeId) {
    let (mut d, comp) = component_with_two_slots_and_a_node_id();
    let n = d.get_mut(comp).unwrap();
    n.component.slot_content_ids.insert("Header".into(), "8:1".into());
    n.component.slot_content_ids.insert("Body".into(), "8:2".into());
    (d, comp)
}

#[test]
fn a_new_instance_starts_with_no_slot_assignments() {
    let (mut d, comp) = component_with_slot_definition_ids();
    let page = d.pages[0];
    create_instance(&mut d, comp, page, Vec2::new(400.0, 0.0)).apply(&mut d).unwrap();
    let inst = instances_of(&d, comp)[0];
    assert!(d.get(inst).unwrap().component.slot_content_ids.is_empty(), "the master's definition ids are not assignments");
    assert_eq!(d.get(comp).unwrap().component.slot_content_ids.len(), 2, "the master keeps its own");
    // The node's own id is unique per node: a copy never inherits it, and clearing the per-property
    // assignments never derives or rewrites it.
    assert_eq!(d.get(comp).unwrap().component.slot_content_id.as_deref(), Some("7:1"), "the master keeps its own node id");
    assert_eq!(d.get(inst).unwrap().component.slot_content_id, None, "a new instance must not share the master's node id");
}

#[test]
fn resetting_an_instance_drops_its_slot_assignments_and_undo_brings_them_back() {
    let (mut d, comp) = component_with_slot_definition_ids();
    let page = d.pages[0];
    create_instance(&mut d, comp, page, Vec2::new(400.0, 0.0)).apply(&mut d).unwrap();
    let inst = instances_of(&d, comp)[0];
    {
        let n = d.get_mut(inst).unwrap();
        n.component.slot_content_ids.insert("Header".into(), "9:1".into());
        n.component.slot_content_id = Some("9:7".into());
    }
    let inverse = reset_instance(&mut d, inst).apply(&mut d).unwrap();
    let n = &d.get(inst).unwrap().component;
    assert!(n.slot_content_ids.is_empty());
    assert_eq!(n.slot_content_id.as_deref(), Some("9:7"), "the node's own id is not touched");
    inverse.apply(&mut d).unwrap();
    assert_eq!(d.get(inst).unwrap().component.slot_content_ids.get("Header").map(String::as_str), Some("9:1"));
}

#[test]
fn convert_to_slot_creates_prop_binds_frame_and_syncs_instances_in_one_undo_step() {
    let mut d = Document::new("slots");
    let page = d.add_page("P");
    let id = d.alloc_id();
    let comp = d.insert(page, Node::frame(id, Rect::new(0.0, 0.0, 240.0, 120.0))).unwrap();
    let id = d.alloc_id();
    let mut body = Node::frame(id, Rect::new(16.0, 16.0, 208.0, 88.0));
    body.name = "Body".into();
    let body = d.insert(comp, body).unwrap();
    create_component(&mut d, comp).apply(&mut d).unwrap();
    create_instance(&mut d, comp, page, Vec2::new(300.0, 0.0)).apply(&mut d).unwrap();
    let inst = instances_of(&d, comp)[0];
    let inst_body = d.get(inst).unwrap().children[0];

    let inv = crate::convert_to_slot(&mut d, body).apply(&mut d).unwrap();
    let body_node = d.get(body).unwrap();
    assert!(body_node.component.is_slot);
    assert_eq!(body_node.component.bindings.get(&crate::OverrideField::SlotContent).map(String::as_str), Some("Body"));
    let comp_node = d.get(comp).unwrap();
    assert!(comp_node.component.props.iter().any(|p| p.name == "Body" && p.kind == PropKind::Slot));
    assert!(d.get(inst_body).unwrap().component.is_slot, "instance child is synced as a slot in the same step");

    inv.apply(&mut d).unwrap();
    assert!(!d.get(body).unwrap().component.is_slot);
    assert!(!d.get(body).unwrap().component.bindings.contains_key(&crate::OverrideField::SlotContent));
    assert!(d.get(comp).unwrap().component.props.is_empty());
    assert!(!d.get(inst_body).unwrap().component.is_slot);
}

#[test]
fn set_slot_config_persists_all_settings_and_undoes_in_one_step() {
    let (mut d, comp, body, icon) = component_with_slot();
    d.get_mut(body).unwrap().component.is_slot = true;
    let updated = SlotPropConfig {
        min_children: Some(1),
        max_children: Some(4),
        allow_preferred_values_only: Some(true),
        stretch_child_on_insert: Some(true),
        display_by_default: Some(true),
        preferred: vec![icon],
        ..Default::default()
    };
    let inv = crate::set_slot_config(&mut d, comp, "Body", updated.clone()).apply(&mut d).unwrap();
    assert_eq!(d.get(comp).unwrap().component.slot_configs.get("Body"), Some(&updated));
    let roundtripped = Document::from_json(&d.to_json().unwrap()).unwrap();
    assert_eq!(roundtripped.get(comp).unwrap().component.slot_configs.get("Body"), Some(&updated));

    inv.apply(&mut d).unwrap();
    assert_eq!(
        d.get(comp).unwrap().component.slot_configs.get("Body"),
        Some(&SlotPropConfig { max_children: Some(2), preferred: vec![icon], ..Default::default() })
    );
}

#[test]
fn editing_instance_slot_adds_reorders_and_removes_without_detaching_and_enforces_limits() {
    let (mut d, comp, body, icon) = component_with_slot();
    let id = d.alloc_id();
    let mut header = Node::frame(id, Rect::new(0.0, 0.0, 200.0, 20.0));
    header.name = "Header".into();
    let header = d.insert(comp, header).unwrap();
    d.get_mut(body).unwrap().component.is_slot = true;
    d.get_mut(body).unwrap().component.bindings.insert(crate::OverrideField::SlotContent, "Body".into());
    d.get_mut(comp).unwrap().component.slot_configs.insert(
        "Body".into(),
        SlotPropConfig { min_children: Some(1), max_children: Some(2), allow_preferred_values_only: Some(true), preferred: vec![icon], ..Default::default() },
    );

    let page = d.pages[0];
    create_instance(&mut d, comp, page, Vec2::new(400.0, 0.0)).apply(&mut d).unwrap();
    let inst = instances_of(&d, comp)[0];
    let inst_body = *d.get(inst).unwrap().children.iter().find(|&&c| d.get(c).unwrap().name == "body").unwrap();
    let inst_header = *d.get(inst).unwrap().children.iter().find(|&&c| d.get(c).unwrap().name == "Header").unwrap();

    // 1. Non-preferred plain frame is rejected when allow_preferred_values_only is true.
    let bad_id = d.alloc_id();
    let bad_rect = Node::frame(bad_id, Rect::new(0.0, 0.0, 20.0, 20.0));
    assert_eq!(crate::insert_into_slot(&mut d, inst_body, 0, vec![bad_rect]).unwrap_err(), crate::SlotConstraintError::PreferredOnly);

    // 2. Add two preferred instances of `icon` into `inst_body`.
    let make_icon_inst = |d: &mut Document, label: &str| -> Vec<Node> {
        let cmd = create_instance(d, icon, page, Vec2::ZERO);
        let mut nodes = match cmd {
            crate::Command::Insert { nodes, .. } => nodes,
            _ => panic!("expected Insert"),
        };
        nodes[0].name = label.into();
        nodes
    };
    let n1 = make_icon_inst(&mut d, "Chip 1");
    let c1_id = n1[0].id;
    crate::insert_into_slot(&mut d, inst_body, 0, n1).unwrap().apply(&mut d).unwrap();
    assert!(crate::is_linked(d.get(inst).unwrap()), "instance stays linked after adding slot child");
    assert!(matches!(d.get(inst).unwrap().kind, NodeKind::Instance { .. }));
    assert!(d.get(inst_body).unwrap().component.overrides.contains(&crate::OverrideField::SlotContent));

    let n2 = make_icon_inst(&mut d, "Chip 2");
    let c2_id = n2[0].id;
    crate::insert_into_slot(&mut d, inst_body, 1, n2).unwrap().apply(&mut d).unwrap();
    assert_eq!(d.get(inst_body).unwrap().children, vec![c1_id, c2_id]);

    // 3. Adding a 3rd child exceeds max_children = 2.
    let n3 = make_icon_inst(&mut d, "Chip 3");
    assert_eq!(crate::insert_into_slot(&mut d, inst_body, 2, n3).unwrap_err(), crate::SlotConstraintError::MaxChildren(2));

    // 4. Reorder children inside the instance slot without detaching.
    crate::reorder_in_slot(&mut d, inst_body, c2_id, 0).unwrap().apply(&mut d).unwrap();
    assert_eq!(d.get(inst_body).unwrap().children, vec![c2_id, c1_id]);
    assert!(crate::is_linked(d.get(inst).unwrap()));

    // 5. Main component updates propagate to non-slot parts while slot content is preserved.
    d.get_mut(header).unwrap().opacity = 0.35;
    sync_instances(&mut d, comp).apply(&mut d).unwrap();
    assert_eq!(d.get(inst_header).unwrap().opacity, 0.35, "non-slot part updated from main");
    assert_eq!(d.get(inst_body).unwrap().children, vec![c2_id, c1_id], "slot children preserved across sync");

    // 6. Removing one child succeeds (leaves 1 >= min_children), removing the last is blocked by min_children = 1.
    crate::remove_from_slot(&mut d, c2_id).unwrap().apply(&mut d).unwrap();
    assert_eq!(d.get(inst_body).unwrap().children, vec![c1_id]);
    assert_eq!(crate::remove_from_slot(&mut d, c1_id).unwrap_err(), crate::SlotConstraintError::MinChildren(1));
}

#[test]
fn empty_slot_placeholder_preferred_picker_and_stretch_on_insert() {
    let (mut d, comp, body, icon) = component_with_slot();
    let page = d.pages[0];
    let other_id = d.alloc_id();
    let mut other = Node::frame(other_id, Rect::new(0.0, 40.0, 24.0, 24.0));
    other.name = "Other".into();
    let other = d.insert(page, other).unwrap();
    create_component(&mut d, other).apply(&mut d).unwrap();

    d.get_mut(body).unwrap().component.is_slot = true;
    d.get_mut(body).unwrap().component.bindings.insert(crate::OverrideField::SlotContent, "Body".into());
    d.get_mut(comp).unwrap().component.slot_configs.insert(
        "Body".into(),
        SlotPropConfig { display_by_default: Some(true), stretch_child_on_insert: Some(true), preferred: vec![icon], ..Default::default() },
    );

    create_instance(&mut d, comp, page, Vec2::new(400.0, 0.0)).apply(&mut d).unwrap();
    let inst = instances_of(&d, comp)[0];
    let inst_body = d.get(inst).unwrap().children[0];

    // 1. Empty slot shows placeholder in main component and in instance when display_by_default is true/None.
    assert!(crate::should_show_empty_slot_placeholder(&d, body));
    assert!(crate::should_show_empty_slot_placeholder(&d, inst_body));

    // When display_by_default is Some(false), the instance hides the empty-slot placeholder by default while main keeps it.
    d.get_mut(comp).unwrap().component.slot_configs.get_mut("Body").unwrap().display_by_default = Some(false);
    assert!(crate::should_show_empty_slot_placeholder(&d, body));
    assert!(!crate::should_show_empty_slot_placeholder(&d, inst_body));
    d.get_mut(comp).unwrap().component.slot_configs.get_mut("Body").unwrap().display_by_default = Some(true);

    // 2. Preferred-instance picker lists only preferred components when set, and all other components when empty.
    assert_eq!(crate::preferred_slot_components(&d, inst_body), vec![icon]);
    d.get_mut(comp).unwrap().component.slot_configs.get_mut("Body").unwrap().preferred.clear();
    let all_candidates = crate::preferred_slot_components(&d, inst_body);
    assert!(all_candidates.contains(&icon) && all_candidates.contains(&other) && !all_candidates.contains(&comp));
    d.get_mut(comp).unwrap().component.slot_configs.get_mut("Body").unwrap().preferred = vec![icon];

    // 3. Inserting a child when stretch_child_on_insert is true stretches the child to the slot's inner width and sets Sizing::Fill.
    let cmd = create_instance(&mut d, icon, page, Vec2::ZERO);
    let nodes = match cmd {
        crate::Command::Insert { nodes, .. } => nodes,
        _ => panic!("expected Insert"),
    };
    let child_id = nodes[0].id;
    crate::insert_into_slot(&mut d, inst_body, 0, nodes).unwrap().apply(&mut d).unwrap();
    let child = d.get(child_id).unwrap();
    assert_eq!(child.layout_child.horizontal, crate::Sizing::Fill);
    assert_eq!(child.size.x, 200.0);
    assert!(!crate::should_show_empty_slot_placeholder(&d, inst_body), "non-empty slot hides placeholder");

    // Syncing the main component preserves the stretched width and Fill sizing on the inserted instance.
    sync_instances(&mut d, comp).apply(&mut d).unwrap();
    let child = d.get(child_id).unwrap();
    assert_eq!(child.layout_child.horizontal, crate::Sizing::Fill);
    assert_eq!(child.size.x, 200.0);
}

/// An instance whose slot frame holds the main component's default child "Default".
fn instance_with_default_slot_child() -> (Document, NodeId, NodeId, NodeId, NodeId, NodeId) {
    let (mut d, comp, body, _icon) = component_with_slot();
    let id = d.alloc_id();
    let mut default_child = Node::frame(id, Rect::new(0.0, 0.0, 10.0, 10.0));
    default_child.name = "Default".into();
    let main_default = d.insert(body, default_child).unwrap();
    d.get_mut(body).unwrap().component.is_slot = true;
    d.get_mut(body).unwrap().component.bindings.insert(crate::OverrideField::SlotContent, "Body".into());
    let page = d.pages[0];
    create_instance(&mut d, comp, page, Vec2::new(400.0, 0.0)).apply(&mut d).unwrap();
    let inst = instances_of(&d, comp)[0];
    let inst_body = *d.get(inst).unwrap().children.iter().find(|&&c| d.get(c).unwrap().name == "body").unwrap();
    let inst_default = d.get(inst_body).unwrap().children[0];
    assert_eq!(d.get(inst_default).unwrap().component.source, Some(main_default));
    (d, comp, inst, inst_body, inst_default, main_default)
}

#[test]
fn reset_of_slot_content_rebuilds_the_slot_from_the_main_component() {
    let (mut d, comp, inst, inst_body, inst_default, main_default) = instance_with_default_slot_child();

    // The user adds a plain frame: it has no source, so only the override flag marks it as theirs.
    let id = d.alloc_id();
    let mut extra = Node::frame(id, Rect::new(0.0, 0.0, 10.0, 10.0));
    extra.name = "Extra".into();
    let extra_id = extra.id;
    crate::insert_into_slot(&mut d, inst_body, 1, vec![extra]).unwrap().apply(&mut d).unwrap();
    assert_eq!(d.get(inst_body).unwrap().children, vec![inst_default, extra_id]);
    assert!(d.get(inst_body).unwrap().component.overrides.contains(&crate::OverrideField::SlotContent));

    crate::reset_overrides(&mut d, inst, &[crate::OverrideField::SlotContent]).apply(&mut d).unwrap();

    let kids = d.get(inst_body).unwrap().children.clone();
    assert!(d.get(extra_id).is_none(), "the user's frame is gone after Reset");
    assert_eq!(kids.len(), 1);
    assert_eq!(d.get(kids[0]).unwrap().component.source, Some(main_default), "the slot mirrors the main's slot children");
    assert!(!d.get(inst_body).unwrap().component.overrides.contains(&crate::OverrideField::SlotContent));
    assert!(matches!(d.get(inst).unwrap().kind, NodeKind::Instance { .. }));

    // Once reset, the slot follows the main component again.
    d.get_mut(main_default).unwrap().opacity = 0.25;
    sync_instances(&mut d, comp).apply(&mut d).unwrap();
    assert_eq!(d.get(d.get(inst_body).unwrap().children[0]).unwrap().opacity, 0.25);
}

#[test]
fn reset_of_slot_content_brings_back_a_removed_main_slot_child() {
    let (mut d, _, inst, inst_body, inst_default, main_default) = instance_with_default_slot_child();
    let id = d.alloc_id();
    let extra = Node::frame(id, Rect::new(0.0, 0.0, 10.0, 10.0));
    crate::insert_into_slot(&mut d, inst_body, 1, vec![extra]).unwrap().apply(&mut d).unwrap();
    crate::remove_from_slot(&mut d, inst_default).unwrap().apply(&mut d).unwrap();
    assert!(d.get(inst_default).is_none());

    crate::reset_overrides(&mut d, inst, &[crate::OverrideField::SlotContent]).apply(&mut d).unwrap();
    let kids = d.get(inst_body).unwrap().children.clone();
    assert_eq!(kids.len(), 1);
    assert_eq!(d.get(kids[0]).unwrap().component.source, Some(main_default));
}

#[test]
fn a_slot_that_still_holds_the_users_content_survives_a_main_sync() {
    let (mut d, comp, _inst, inst_body, inst_default, main_default) = instance_with_default_slot_child();
    let id = d.alloc_id();
    let extra = Node::frame(id, Rect::new(0.0, 0.0, 10.0, 10.0));
    let extra_id = extra.id;
    crate::insert_into_slot(&mut d, inst_body, 1, vec![extra]).unwrap().apply(&mut d).unwrap();
    d.get_mut(main_default).unwrap().opacity = 0.5;
    sync_instances(&mut d, comp).apply(&mut d).unwrap();
    assert_eq!(d.get(inst_body).unwrap().children, vec![inst_default, extra_id], "overridden slot content is left alone");
}

#[test]
fn a_swap_is_judged_on_the_count_the_slot_ends_with() {
    let (mut d, comp, body, _) = component_with_slot();
    d.get_mut(body).unwrap().component.is_slot = true;
    d.get_mut(body).unwrap().component.bindings.insert(crate::OverrideField::SlotContent, "Body".into());
    d.get_mut(comp)
        .unwrap()
        .component
        .slot_configs
        .insert("Body".into(), SlotPropConfig { min_children: Some(2), max_children: Some(2), ..Default::default() });
    for _ in 0..2 {
        let id = d.alloc_id();
        d.insert(body, Node::frame(id, Rect::new(0.0, 0.0, 4.0, 4.0))).unwrap();
    }
    let ids: Vec<NodeId> = (0..3).map(|_| d.alloc_id()).collect();
    let nodes: Vec<Node> = ids.iter().map(|id| Node::frame(*id, Rect::new(0.0, 0.0, 4.0, 4.0))).collect();
    let refs: Vec<&Node> = nodes.iter().collect();
    // 2 - 1 + 1 = 2: one for one is fine at both limits.
    assert!(crate::check_slot_replace(&d, body, 1, &refs[..1]).is_ok());
    // 2 - 1 + 2 = 3 > max.
    assert_eq!(crate::check_slot_replace(&d, body, 1, &refs[..2]).unwrap_err(), crate::SlotConstraintError::MaxChildren(2));
    // 2 - 2 + 1 = 1 < min.
    assert_eq!(crate::check_slot_replace(&d, body, 2, &refs[..1]).unwrap_err(), crate::SlotConstraintError::MinChildren(2));
    // a plain insert keeps the old rule: 2 + 1 > max.
    assert_eq!(crate::check_slot_insert(&d, body, &refs[..1]).unwrap_err(), crate::SlotConstraintError::MaxChildren(2));
}

/// A Card component holding its own "Content" slot and a nested Button instance whose "Icon" slot
/// has different settings. Returns (doc, card, button, button_icon_preferred, nested_icon_slot).
fn card_with_nested_button_slot() -> (Document, NodeId, NodeId, NodeId, NodeId) {
    let mut d = Document::new("nested-slots");
    let page = d.add_page("P");
    let frame = |d: &mut Document, parent: NodeId, name: &str, r: Rect| {
        let id = d.alloc_id();
        let mut n = Node::frame(id, r);
        n.name = name.into();
        d.insert(parent, n).unwrap()
    };

    let preferred = frame(&mut d, page, "Glyph", Rect::new(0.0, 0.0, 8.0, 8.0));
    create_component(&mut d, preferred).apply(&mut d).unwrap();

    let button = frame(&mut d, page, "Button", Rect::new(0.0, 20.0, 120.0, 40.0));
    let icon = frame(&mut d, button, "Icon", Rect::new(0.0, 0.0, 120.0, 40.0));
    create_component(&mut d, button).apply(&mut d).unwrap();
    crate::convert_to_slot(&mut d, icon).apply(&mut d).unwrap();
    crate::set_slot_config(
        &mut d,
        button,
        "Icon",
        SlotPropConfig {
            display_by_default: Some(false),
            stretch_child_on_insert: Some(true),
            max_children: Some(1),
            preferred: vec![preferred],
            ..Default::default()
        },
    )
    .apply(&mut d)
    .unwrap();

    let card = frame(&mut d, page, "Card", Rect::new(0.0, 100.0, 300.0, 200.0));
    let content = frame(&mut d, card, "Content", Rect::new(0.0, 0.0, 300.0, 100.0));
    create_component(&mut d, card).apply(&mut d).unwrap();
    crate::convert_to_slot(&mut d, content).apply(&mut d).unwrap();
    crate::set_slot_config(&mut d, card, "Content", SlotPropConfig { max_children: Some(5), ..Default::default() }).apply(&mut d).unwrap();
    create_instance(&mut d, button, card, Vec2::new(0.0, 120.0)).apply(&mut d).unwrap();
    sync_instances(&mut d, card).apply(&mut d).unwrap();

    let nested_button = *d.get(card).unwrap().children.iter().find(|c| d.get(**c).unwrap().name == "Button").unwrap();
    let nested_icon = d.get(nested_button).unwrap().children[0];
    (d, card, button, preferred, nested_icon)
}

#[test]
fn a_slot_in_a_nested_instance_takes_its_settings_from_the_nested_component() {
    let (mut d, card, button, preferred, nested_icon) = card_with_nested_button_slot();
    let want = d.get(button).unwrap().component.slot_configs.get("Icon").cloned().unwrap();

    // Inside the Card component, the nested Button's Icon slot is governed by Button's settings,
    // not by the Card's single "Content" entry.
    assert_eq!(crate::slot_config_of(&d, nested_icon), Some(want.clone()));
    assert!(!crate::should_show_empty_slot_placeholder(&d, nested_icon), "Button hides the empty Icon slot in instances");
    assert_eq!(crate::preferred_slot_components(&d, nested_icon), vec![preferred]);

    // The same holds in an instance of the Card.
    let page = d.pages[0];
    create_instance(&mut d, card, page, Vec2::new(500.0, 0.0)).apply(&mut d).unwrap();
    let card_inst = instances_of(&d, card)[0];
    let inst_button = *d.get(card_inst).unwrap().children.iter().find(|c| d.get(**c).unwrap().name == "Button").unwrap();
    let inst_icon = d.get(inst_button).unwrap().children[0];
    assert_eq!(crate::slot_config_of(&d, inst_icon), Some(want));
    assert!(!crate::should_show_empty_slot_placeholder(&d, inst_icon));
    assert_eq!(crate::preferred_slot_components(&d, inst_icon), vec![preferred]);

    // Card's own slot still resolves to Card's settings.
    let card_content = *d.get(card).unwrap().children.iter().find(|c| d.get(**c).unwrap().name == "Content").unwrap();
    assert_eq!(crate::slot_config_of(&d, card_content).and_then(|c| c.max_children), Some(5));
}

#[test]
fn a_bound_slot_without_settings_resolves_to_none_instead_of_the_only_entry_of_another_slot() {
    let (mut d, card, _, _, nested_icon) = card_with_nested_button_slot();
    // Button keeps a single entry, but for another slot: it must not stand in for the bound "Icon" slot.
    let button = crate::enclosing_component(&d, d.get(nested_icon).unwrap().component.source.unwrap()).unwrap();
    let configs = &mut d.get_mut(button).unwrap().component.slot_configs;
    let only = configs.remove("Icon").unwrap();
    configs.insert("Other".into(), only);
    assert_eq!(crate::slot_config_of(&d, nested_icon), None);
    assert_eq!(d.get(card).unwrap().component.slot_configs.len(), 1);
}

#[test]
fn reset_drops_a_duplicate_of_a_mirrored_slot_child_and_a_main_sync_keeps_it_gone() {
    let (mut d, comp, inst, inst_body, inst_default, main_default) = instance_with_default_slot_child();

    // Duplicating a mirrored child keeps its `source`, so the copy points at the same main child.
    let copy = crate::duplicate_subtree(&mut d, inst_default);
    let copy_id = copy[0].id;
    assert_eq!(copy[0].component.source, Some(main_default));
    crate::insert_into_slot(&mut d, inst_body, 1, copy).unwrap().apply(&mut d).unwrap();
    assert_eq!(d.get(inst_body).unwrap().children, vec![inst_default, copy_id]);

    // While the slot overrides its content, a main-component sync leaves both children alone.
    sync_instances(&mut d, comp).apply(&mut d).unwrap();
    assert_eq!(d.get(inst_body).unwrap().children, vec![inst_default, copy_id], "edited slot content is kept");

    // Reset: the slot holds exactly the main slot's children, with no second node for the same source.
    crate::reset_overrides(&mut d, inst, &[crate::OverrideField::SlotContent]).apply(&mut d).unwrap();
    let kids = d.get(inst_body).unwrap().children.clone();
    let sources: Vec<Option<NodeId>> = kids.iter().map(|k| d.get(*k).unwrap().component.source).collect();
    assert_eq!(sources, vec![Some(main_default)], "one mirrored child per main child, nothing extra");
    assert!(d.get(copy_id).is_none(), "the duplicate is removed");
    assert!(!d.get(inst_body).unwrap().component.overrides.contains(&crate::OverrideField::SlotContent));

    // A later main-component sync keeps it that way.
    sync_instances(&mut d, comp).apply(&mut d).unwrap();
    let sources: Vec<Option<NodeId>> = d.get(inst_body).unwrap().children.iter().map(|k| d.get(*k).unwrap().component.source).collect();
    assert_eq!(sources, vec![Some(main_default)]);
}
