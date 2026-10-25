//! Regression tests for #985: `designos_delete_node` and `designos_apply_batch` (`delete` op)
//! refuse deleting a layer inside a component instance at any depth (including nested instances),
//! while keeping instance roots, main-component children, and agent-added slot content deletable.
#![allow(clippy::result_large_err)]

use designos_doc::{convert_to_slot, create_instance, sync_instances, FrameProps, Node, NodeId, NodeKind, OverrideField, Rect, TextStyle, Vec2};
use designos_mcp::{engine::Engine, tools::errors::ErrorCode};
use serde_json::json;

struct NestedInstanceFixture {
    icon_comp: NodeId,
    toolbar_comp: NodeId,
    toolbar_lead_icon: NodeId,
    toolbar_caption: NodeId,
    save_inst: NodeId,
    inst_caption: NodeId,
    inst_nested_icon: NodeId,
    inst_grandchild_glyph: NodeId,
}

fn build_nested_instance_fixture(e: &mut Engine) -> NestedInstanceFixture {
    let page = e.resolve_default_page().unwrap();

    // 1. Inner main component "Icon" with child "Glyph"
    let icon_comp = e.doc.alloc_id();
    let mut icon_node = Node::new(icon_comp, NodeKind::Component(FrameProps::default()));
    icon_node.name = "Icon".into();
    e.doc.insert(page, icon_node).unwrap();

    let glyph = e.doc.alloc_id();
    let mut glyph_node = Node::rectangle(glyph, Rect::new(0.0, 0.0, 16.0, 16.0));
    glyph_node.name = "Glyph".into();
    e.doc.insert(icon_comp, glyph_node).unwrap();

    // 2. Outer main component "ToolbarButton" containing an instance "LeadIcon" of "Icon" + text "Caption"
    let toolbar_comp = e.doc.alloc_id();
    let mut toolbar_node = Node::new(toolbar_comp, NodeKind::Component(FrameProps::default()));
    toolbar_node.name = "ToolbarButton".into();
    e.doc.insert(page, toolbar_node).unwrap();

    create_instance(&mut e.doc, icon_comp, toolbar_comp, Vec2::new(0.0, 0.0)).apply(&mut e.doc).unwrap();
    let toolbar_lead_icon = *e.doc.get(toolbar_comp).unwrap().children.last().unwrap();
    e.doc.get_mut(toolbar_lead_icon).unwrap().name = "LeadIcon".into();

    let toolbar_caption = e.doc.alloc_id();
    let mut caption_node = Node::text(toolbar_caption, Rect::new(20.0, 0.0, 60.0, 16.0), "Save", TextStyle::default());
    caption_node.name = "Caption".into();
    e.doc.insert(toolbar_comp, caption_node).unwrap();

    // 3. Instance "SaveButton" of "ToolbarButton" on the page
    create_instance(&mut e.doc, toolbar_comp, page, Vec2::new(100.0, 100.0)).apply(&mut e.doc).unwrap();
    let save_inst = *e.doc.get(page).unwrap().children.last().unwrap();
    e.doc.get_mut(save_inst).unwrap().name = "SaveButton".into();

    let inst_children = e.doc.get(save_inst).unwrap().children.clone();
    let inst_nested_icon = inst_children.iter().copied().find(|&id| e.doc.get(id).unwrap().name == "LeadIcon").expect("nested LeadIcon instance in SaveButton");
    let inst_caption = inst_children.iter().copied().find(|&id| e.doc.get(id).unwrap().name == "Caption").expect("direct Caption child in SaveButton");
    let inst_grandchild_glyph = e.doc.get(inst_nested_icon).unwrap().children.first().copied().expect("grandchild Glyph inside nested LeadIcon");

    NestedInstanceFixture { icon_comp, toolbar_comp, toolbar_lead_icon, toolbar_caption, save_inst, inst_caption, inst_nested_icon, inst_grandchild_glyph }
}

#[test]
fn delete_node_refuses_direct_child_of_instance() {
    let mut e = Engine::default();
    let f = build_nested_instance_fixture(&mut e);
    let rev_before = e.revision;

    let err = e.call("designos_delete_node", json!({ "node_id": f.inst_caption.0 })).expect_err("deleting a direct child of an instance must be refused");

    assert_eq!(err.code, ErrorCode::InvalidArguments);
    assert_eq!(err.path, "/node_id");
    assert_eq!(err.received, Some(json!(f.inst_caption.0)));
    assert_eq!(err.message, "Caption is inside the instance SaveButton. Hide it or set an override instead.");
    assert_eq!(err.next_step, "Hide it or set an override instead.");
    assert!(e.doc.get(f.inst_caption).is_some(), "direct child must remain in the document");
    assert!(e.doc.get(f.save_inst).unwrap().children.contains(&f.inst_caption), "direct child must remain attached to the instance");
    assert_eq!(e.revision, rev_before, "failed delete must not bump revision");
}

#[test]
fn delete_node_refuses_nested_instance_and_grandchild_in_nested_instance() {
    let mut e = Engine::default();
    let f = build_nested_instance_fixture(&mut e);

    // 1. Grandchild inside a nested instance
    let err_grandchild = e
        .call("designos_delete_node", json!({ "node_id": f.inst_grandchild_glyph.0 }))
        .expect_err("deleting a grandchild inside a nested instance must be refused");

    assert_eq!(err_grandchild.code, ErrorCode::InvalidArguments);
    assert_eq!(err_grandchild.path, "/node_id");
    assert_eq!(err_grandchild.received, Some(json!(f.inst_grandchild_glyph.0)));
    assert_eq!(err_grandchild.message, "Glyph is inside the instance SaveButton. Hide it or set an override instead.");
    assert_eq!(err_grandchild.next_step, "Hide it or set an override instead.");
    assert!(e.doc.get(f.inst_grandchild_glyph).is_some());

    // 2. The nested instance itself (child of outer instance)
    let err_nested = e
        .call("designos_delete_node", json!({ "node_id": f.inst_nested_icon.0 }))
        .expect_err("deleting a nested instance inside an outer instance must be refused");

    assert_eq!(err_nested.code, ErrorCode::InvalidArguments);
    assert_eq!(err_nested.path, "/node_id");
    assert_eq!(err_nested.received, Some(json!(f.inst_nested_icon.0)));
    assert_eq!(err_nested.message, "LeadIcon is inside the instance SaveButton. Hide it or set an override instead.");
    assert!(e.doc.get(f.inst_nested_icon).is_some());
}

#[test]
fn batch_delete_refuses_instance_child_and_nested_grandchild_atomically() {
    let mut e = Engine::default();
    let f = build_nested_instance_fixture(&mut e);
    let page = e.resolve_default_page().unwrap();
    let snapshot_before = e.doc.to_json().unwrap();
    let rev_before = e.revision;

    // 1. Batch with a valid create + valid update + invalid delete of direct instance child
    let batch_err = e
        .call(
            "designos_apply_batch",
            json!({
                "ops": [
                    { "op": "create", "as": "extra", "parent": page.0, "kind": "frame", "name": "ShouldRollBack" },
                    { "op": "update", "node": f.save_inst.0, "x": 320.0 },
                    { "op": "delete", "node": f.inst_caption.0 }
                ]
            }),
        )
        .expect_err("batch containing an instance-child delete must fail as a whole");

    assert_eq!(batch_err.code, ErrorCode::BatchOpFailed);
    assert_eq!(batch_err.failed_op, Some(2));
    assert_eq!(batch_err.op.as_deref(), Some("delete"));
    let cause = batch_err.cause.as_ref().expect("batch error must carry inner cause");
    assert_eq!(cause.code, ErrorCode::InvalidArguments);
    assert_eq!(cause.path, "/ops/2");
    assert_eq!(cause.received, Some(json!(f.inst_caption.0)));
    assert_eq!(cause.message, "Caption is inside the instance SaveButton. Hide it or set an override instead.");
    assert_eq!(e.doc.to_json().unwrap(), snapshot_before, "failed batch must leave the document completely unchanged");
    assert_eq!(e.revision, rev_before);

    // 2. Batch with a valid create + invalid delete of grandchild in nested instance
    let batch_grandchild_err = e
        .call(
            "designos_apply_batch",
            json!({
                "ops": [
                    { "op": "create", "parent": page.0, "kind": "rectangle", "name": "TransientRect" },
                    { "op": "delete", "node": f.inst_grandchild_glyph.0 }
                ]
            }),
        )
        .expect_err("batch containing a nested-instance grandchild delete must fail as a whole");

    assert_eq!(batch_grandchild_err.code, ErrorCode::BatchOpFailed);
    assert_eq!(batch_grandchild_err.failed_op, Some(1));
    let gc_cause = batch_grandchild_err.cause.as_ref().expect("batch cause");
    assert_eq!(gc_cause.code, ErrorCode::InvalidArguments);
    assert_eq!(gc_cause.message, "Glyph is inside the instance SaveButton. Hide it or set an override instead.");
    assert_eq!(e.doc.to_json().unwrap(), snapshot_before);
    assert_eq!(e.revision, rev_before);
}

#[test]
fn slot_added_content_stays_deletable_in_delete_node_and_batch() {
    let mut e = Engine { doc: designos_doc::fixtures::card_with_slots(), ..Engine::default() };
    let page = e.resolve_default_page().unwrap();

    // Create a Card instance with slot content populated via `slots`
    let applied = e
        .call(
            "designos_apply_spec",
            json!({
                "parent_id": page.0,
                "spec": {
                    "kind": "instance",
                    "component": "Card",
                    "name": "ProfileCard",
                    "slots": {
                        "header": [
                            { "kind": "instance", "component": "CardHeader", "name": "SlotHeaderInst" }
                        ],
                        "body": [
                            {
                                "kind": "frame",
                                "name": "SlotCustomGroup",
                                "children": [
                                    { "kind": "text", "name": "SlotDeepText", "text": "Nested inside slot group" }
                                ]
                            }
                        ]
                    }
                }
            }),
        )
        .expect("apply_spec with slots");
    let card_inst = NodeId(applied["root_node_id"].as_u64().unwrap());

    let header_slot =
        e.doc.get(card_inst).unwrap().children.iter().copied().find(|&id| e.doc.get(id).unwrap().name == "header").expect("header slot container");
    let body_slot = e.doc.get(card_inst).unwrap().children.iter().copied().find(|&id| e.doc.get(id).unwrap().name == "body").expect("body slot container");

    // Verify how slots mark added content:
    // 1) The slot containers carry OverrideField::SlotContent
    assert!(e.doc.get(header_slot).unwrap().component.overrides.contains(&OverrideField::SlotContent));
    assert!(e.doc.get(body_slot).unwrap().component.overrides.contains(&OverrideField::SlotContent));

    // 2) The root nodes inserted via `slots` carry `component.is_slot_content == true`
    let slot_header_inst = e.doc.get(header_slot).unwrap().children[0];
    let slot_custom_group = e.doc.get(body_slot).unwrap().children[0];
    let slot_deep_text = e.doc.get(slot_custom_group).unwrap().children[0];
    assert!(e.doc.get(slot_header_inst).unwrap().component.is_slot_content);
    assert!(e.doc.get(slot_custom_group).unwrap().component.is_slot_content);

    // The slot container itself ("body") is part of the Card instance structure and must NOT be deletable.
    let slot_container_err =
        e.call("designos_delete_node", json!({ "node_id": body_slot.0 })).expect_err("slot container itself is an instance child and cannot be deleted");
    assert_eq!(slot_container_err.code, ErrorCode::InvalidArguments);
    assert_eq!(slot_container_err.message, "body is inside the instance ProfileCard. Hide it or set an override instead.");

    // A child inside the slot-added component instance `SlotHeaderInst` belongs to `SlotHeaderInst`
    // and must NOT be deletable; only `SlotHeaderInst` itself (the slot item root) is deletable.
    let header_inner_label = e.doc.get(slot_header_inst).unwrap().children[0];
    let inner_err =
        e.call("designos_delete_node", json!({ "node_id": header_inner_label.0 })).expect_err("layer inside a slot-added component instance must be refused");
    assert_eq!(inner_err.code, ErrorCode::InvalidArguments);
    assert_eq!(inner_err.message, "Label is inside the instance SlotHeaderInst. Hide it or set an override instead.");

    // Deleting a nested child inside the agent-added non-instance slot frame succeeds.
    e.call("designos_delete_node", json!({ "node_id": slot_deep_text.0 })).expect("nested child inside slot-added frame stays deletable");
    assert!(e.doc.get(slot_deep_text).is_none());

    // Deleting the slot-added frame root via designos_delete_node succeeds.
    e.call("designos_delete_node", json!({ "node_id": slot_custom_group.0 })).expect("slot-added frame root stays deletable");
    assert!(e.doc.get(slot_custom_group).is_none());

    // Deleting the slot-added component instance root via designos_apply_batch delete op succeeds.
    e.call(
        "designos_apply_batch",
        json!({
            "ops": [
                { "op": "delete", "node": slot_header_inst.0 }
            ]
        }),
    )
    .expect("slot-added instance root stays deletable via batch delete");
    assert!(e.doc.get(slot_header_inst).is_none());

    // Adding a node to the slot via designos_create_node also marks SlotContent and stays deletable.
    let created = e
        .call(
            "designos_create_node",
            json!({
                "parent_id": body_slot.0,
                "kind": "text",
                "name": "CreatedSlotText",
                "text": "Added via create_node"
            }),
        )
        .expect("create_node in instance slot");
    let created_id = NodeId(created["node"]["id"].as_u64().unwrap());
    e.call("designos_delete_node", json!({ "node_id": created_id.0 })).expect("node added to slot via create_node stays deletable");
    assert!(e.doc.get(created_id).is_none());

    // Also verify a standalone component with a converted slot where content is added via create_node and deleted via batch.
    let comp_id = e.doc.alloc_id();
    let mut comp_node = Node::new(comp_id, NodeKind::Component(FrameProps::default()));
    comp_node.name = "Banner".into();
    e.doc.insert(page, comp_node).unwrap();
    let slot_id = e.doc.alloc_id();
    let mut slot_node = Node::frame(slot_id, Rect::new(0.0, 0.0, 100.0, 40.0));
    slot_node.name = "Actions".into();
    e.doc.insert(comp_id, slot_node).unwrap();
    convert_to_slot(&mut e.doc, slot_id).apply(&mut e.doc).unwrap();

    create_instance(&mut e.doc, comp_id, page, Vec2::new(0.0, 300.0)).apply(&mut e.doc).unwrap();
    let banner_inst = *e.doc.get(page).unwrap().children.last().unwrap();
    let inst_actions_slot = e.doc.get(banner_inst).unwrap().children[0];
    let added = e.call("designos_create_node", json!({ "parent_id": inst_actions_slot.0, "kind": "frame", "name": "ActionPill" })).unwrap();
    let action_pill_id = NodeId(added["node"]["id"].as_u64().unwrap());
    assert!(e.doc.get(inst_actions_slot).unwrap().component.overrides.contains(&OverrideField::SlotContent));
    e.call("designos_apply_batch", json!({ "ops": [{ "op": "delete", "node": action_pill_id.0 }] })).expect("batch delete of slot-added node must succeed");
    assert!(e.doc.get(action_pill_id).is_none());
    sync_instances(&mut e.doc, comp_id).apply(&mut e.doc).unwrap();
}

#[test]
fn instance_root_and_main_component_children_stay_deletable() {
    let mut e = Engine::default();
    let f = build_nested_instance_fixture(&mut e);
    let page = e.resolve_default_page().unwrap();

    // 1. A child inside a main component stays deletable via designos_delete_node.
    e.call("designos_delete_node", json!({ "node_id": f.toolbar_caption.0 })).expect("deleting a child of a main component must succeed");
    assert!(e.doc.get(f.toolbar_caption).is_none());

    // 2. A top-level instance placed inside a main component stays deletable via batch delete.
    e.call(
        "designos_apply_batch",
        json!({
            "ops": [
                { "op": "delete", "node": f.toolbar_lead_icon.0 }
            ]
        }),
    )
    .expect("deleting a top-level instance inside a main component must succeed");
    assert!(e.doc.get(f.toolbar_lead_icon).is_none());

    // 3. An instance root on the page stays deletable via designos_delete_node.
    e.call("designos_delete_node", json!({ "node_id": f.save_inst.0 })).expect("deleting an instance root must succeed");
    assert!(e.doc.get(f.save_inst).is_none());

    // 4. Another instance root on the page stays deletable via designos_apply_batch delete.
    create_instance(&mut e.doc, f.icon_comp, page, Vec2::new(200.0, 200.0)).apply(&mut e.doc).unwrap();
    let inst2 = *e.doc.get(page).unwrap().children.last().unwrap();
    e.call(
        "designos_apply_batch",
        json!({
            "ops": [
                { "op": "delete", "node": inst2.0 }
            ]
        }),
    )
    .expect("deleting an instance root via batch delete must succeed");
    assert!(e.doc.get(inst2).is_none());
    let _ = f.toolbar_comp;
}
