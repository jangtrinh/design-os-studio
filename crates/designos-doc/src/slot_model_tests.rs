//! Slot data on [`ComponentLink`]: compact serialization, instance copies and garbage collection.

use crate::{
    create_component, create_instance, delete_prop, duplicate_subtree, instances_of, rename_prop, sync_instances, ComponentPropDef, Document, Node, NodeId,
    NodeKind, PropKind, PropValue, Rect, SlotPropConfig, Vec2,
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
