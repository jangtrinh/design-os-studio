//! Slot data on [`ComponentLink`]: compact serialization, instance copies and garbage collection.

use crate::{
    create_component, create_instance, instances_of, sync_instances, ComponentPropDef, Document, Node, NodeId, NodeKind, PropKind, PropValue, Rect,
    SlotPropConfig, Vec2,
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
