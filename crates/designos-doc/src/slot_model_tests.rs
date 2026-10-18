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
    assert_eq!(d.get(inst).unwrap().component.slot_content_id.as_deref(), Some("7:1"), "the node's own id is not touched");
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
