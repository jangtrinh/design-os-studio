//! Exported `.fig` NodeChange records may only carry field tags that exist in Figma's own schema.
//!
//! A decoder that uses Figma's field table stops at the first tag it does not know, so one invented tag
//! hides every field written after it. These tests decode the exported message with a NodeChange
//! definition cut down to the real field map, then import from that decode, so any tag outside the map
//! fails the decode and every value below is read back through real fields only.

use designos_doc::*;
use designos_fig::kiwi::{self, KiwiValue, Schema};
use designos_fig::testkit::{figma_like_schema, real_node_change_tag};
use designos_fig::{container, export_fig, import_fig, import_fig_node_changes};
use serde_json::{json, Value};

mod common;
use common::{find, rich_document, STUDIO_PLUGIN_ID};

fn child<'a>(doc: &'a Document, parent: &Node, name: &str) -> &'a Node {
    parent.children.iter().filter_map(|id| doc.get(*id)).find(|n| n.name == name).unwrap_or_else(|| panic!("{} has no child {name}", parent.name))
}

fn padding_of(n: &Node) -> [f32; 4] {
    n.kind.frame_props().and_then(|f| f.auto_layout.as_ref()).unwrap_or_else(|| panic!("{} has no auto layout", n.name)).padding
}

/// The file's own schema and the encoded message, read back out of an exported archive.
fn canvas_of(bytes: &[u8]) -> (Schema, Vec<u8>) {
    let archive = container::open(bytes).expect("open archive");
    let (schema_bytes, message) = container::read_archive_chunks(&archive).expect("read chunks");
    (Schema::decode(&schema_bytes).expect("decode schema"), message)
}

/// The schema with every NodeChange field outside Figma's map (name and tag) removed.
fn real_fields_only(schema: &Schema) -> Schema {
    let mut cut = schema.clone();
    let nc = cut.find("NodeChange").expect("NodeChange def");
    cut.defs[nc].fields.retain(|f| real_node_change_tag(&f.name) == Some(f.value));
    // Round-trip through the wire form so the decoder's tag lookup is rebuilt.
    Schema::decode(&cut.encode()).expect("re-decode cut schema")
}

/// Decodes the export with real fields only (an unknown tag is a decode error) and imports from that decode.
fn import_through_real_fields(bytes: &[u8]) -> Document {
    let (schema, message) = canvas_of(bytes);
    let cut = real_fields_only(&schema);
    let root = cut.root_message().expect("root message");
    let decoded: KiwiValue = cut.decode_message(root, &message).unwrap_or_else(|e| panic!("export holds a NodeChange tag outside Figma's map: {e}"));
    import_fig_node_changes(&kiwi::to_json(&decoded), "real-fields").expect("import from real fields")
}

#[test]
fn a_name_figma_does_not_know_has_no_tag() {
    assert_eq!(real_node_change_tag("stackVerticalPadding"), Some(210));
    assert_eq!(real_node_change_tag("connectorEndCap"), Some(246));
    assert_eq!(real_node_change_tag("stackPaddingTop"), None);
    assert_eq!(real_node_change_tag("gridColumnAnchorIndex"), None);
}

#[test]
fn every_node_change_field_of_the_export_schema_is_in_figmas_map_with_its_tag() {
    let schema = figma_like_schema();
    let nc = schema.find("NodeChange").expect("NodeChange def");
    let bad: Vec<String> =
        schema.defs[nc].fields.iter().filter(|f| real_node_change_tag(&f.name) != Some(f.value)).map(|f| format!("{}={}", f.name, f.value)).collect();
    assert!(bad.is_empty(), "NodeChange fields outside Figma's map: {bad:?}");
}

#[test]
fn every_tag_of_an_exported_message_decodes_with_figmas_real_field_table() {
    let bytes = export_fig(&rich_document()).expect("export_fig");
    let (schema, message) = canvas_of(&bytes);
    let cut = real_fields_only(&schema);
    let root = cut.root_message().expect("root message");
    let decoded = cut.decode_message(root, &message).unwrap_or_else(|e| panic!("export holds a NodeChange tag outside Figma's map: {e}"));
    let changes = kiwi::to_json(&decoded);
    assert!(changes["nodeChanges"].as_array().is_some_and(|a| a.len() > 10), "the decode reached the node changes");
}

#[test]
fn connector_caps_are_written_on_connector_start_cap_and_connector_end_cap_and_round_trip() {
    let bytes = export_fig(&rich_document()).expect("export_fig");
    let (schema, message) = canvas_of(&bytes);
    let cut = real_fields_only(&schema);
    let decoded = cut.decode_message(cut.root_message().unwrap(), &message).expect("decode with real fields");
    let json = kiwi::to_json(&decoded);
    let connector = json["nodeChanges"].as_array().unwrap().iter().find(|n| n["type"] == "CONNECTOR").expect("connector node change");
    assert_eq!(connector["connectorStartCap"], "ARROW_LINES");
    assert_eq!(connector["connectorEndCap"], "ARROW_EQUILATERAL");
    assert!(connector.get("connectorStartStrokeCap").is_none() && connector.get("connectorEndStrokeCap").is_none());

    let rt = import_through_real_fields(&bytes);
    let NodeKind::Connector(p) = &find(&rt, "Connector").kind else { panic!("connector") };
    assert_eq!((p.start_cap, p.end_cap, p.line), (ConnectorCap::Arrow, ConnectorCap::Triangle, ConnectorLine::Straight));
    // The caps are typed data: they must not come back a second time as passthrough metadata.
    assert!(find(&rt, "Connector").fig_metadata.is_empty(), "{:?}", find(&rt, "Connector").fig_metadata);
    assert!(find(&import_fig(&bytes).expect("import_fig"), "Connector").fig_metadata.is_empty());
}

#[test]
fn padding_round_trips_on_frames_and_inside_instance_overrides_through_real_fields_only() {
    let bytes = export_fig(&rich_document()).expect("export_fig");
    let rt = import_through_real_fields(&bytes);
    assert_eq!(padding_of(find(&rt, "Card")), [10.0, 20.0, 30.0, 40.0], "component padding");
    assert_eq!(padding_of(find(&rt, "Grid")), [9.0, 8.0, 7.0, 6.0], "grid padding");
    let inst = find(&rt, "CardInstance");
    assert_eq!(padding_of(child(&rt, inst, "Inner")), [5.0, 6.0, 7.0, 8.0], "padding override on an instance child");
    let untouched = find(&rt, "SlotInstance");
    assert_eq!(padding_of(child(&rt, untouched, "Inner")), [1.0, 2.0, 3.0, 4.0], "an instance without the override keeps the main padding");
}

#[test]
fn grid_anchors_and_spans_round_trip_on_the_real_anchor_fields() {
    let rt = import_through_real_fields(&export_fig(&rich_document()).expect("export_fig"));
    let cell = find(&rt, "Cell");
    assert_eq!((cell.layout_child.grid_column, cell.layout_child.grid_row, cell.layout_child.col_span), (Some(2), Some(1), 2));
}

#[test]
fn sticky_author_and_slot_content_id_round_trip_through_a_reserved_plugin_data_carrier() {
    let bytes = export_fig(&rich_document()).expect("export_fig");
    let rt = import_through_real_fields(&bytes);
    let sticky_author = |name: &str| match &find(&rt, name).kind {
        NodeKind::Sticky(sp) => sp.show_author,
        other => panic!("{name} is {other:?}"),
    };
    assert!(sticky_author("AuthorShown"));
    assert!(!sticky_author("AuthorHidden"));
    assert_eq!(find(&rt, "SlotInstance").component.slot_content_id.as_deref(), Some("7:42"));
    assert!(find(&rt, "CardInstance").component.slot_content_id.is_none());

    // The carrier is reserved bookkeeping: it never shows up as a node's own plugin data.
    let slot_carrier = format!("{STUDIO_PLUGIN_ID}:slot-content-id");
    let author_carrier = format!("{STUDIO_PLUGIN_ID}:author-visible");
    for n in rt.nodes.values() {
        assert!(
            !n.plugin_data.keys().any(|k| k == &slot_carrier || k == &author_carrier),
            "{}: reserved carrier leaked into plugin_data: {:?}",
            n.name,
            n.plugin_data
        );
    }
}

#[test]
fn real_tags_check_rejects_a_longer_key() {
    let mut doc = rich_document();
    let card_id = find(&doc, "Card").id;
    let longer_slot_key = format!("{STUDIO_PLUGIN_ID}:slot-content-id-extra");
    let longer_author_key = format!("{STUDIO_PLUGIN_ID}:author-visible-extra");
    doc.get_mut(card_id).unwrap().plugin_data.insert(longer_slot_key.clone(), "user-slot-val".into());
    doc.get_mut(card_id).unwrap().plugin_data.insert(longer_author_key.clone(), "user-author-val".into());

    let rt = import_through_real_fields(&export_fig(&doc).expect("export_fig"));
    let rt_card = find(&rt, "Card");
    assert_eq!(rt_card.plugin_data.get(&longer_slot_key).map(String::as_str), Some("user-slot-val"));
    assert_eq!(rt_card.plugin_data.get(&longer_author_key).map(String::as_str), Some("user-author-val"));

    let slot_carrier = format!("{STUDIO_PLUGIN_ID}:slot-content-id");
    let author_carrier = format!("{STUDIO_PLUGIN_ID}:author-visible");
    for n in rt.nodes.values() {
        assert!(
            !n.plugin_data.keys().any(|k| k == &slot_carrier || k == &author_carrier),
            "{}: reserved carrier leaked into plugin_data: {:?}",
            n.name,
            n.plugin_data
        );
    }
}

#[test]
fn a_nodes_own_plugin_data_comes_back_unchanged_beside_the_carrier() {
    let doc = rich_document();
    let expected = find(&doc, "Card").plugin_data.clone();
    assert_eq!(expected.len(), 4);
    let rt = import_through_real_fields(&export_fig(&doc).expect("export_fig"));
    assert_eq!(find(&rt, "Card").plugin_data, expected);
    // The carrier sits on nodes with no plugin data of their own without adding any, and next to a node's
    // own entries (the instance copies the component's) without replacing them.
    assert!(find(&rt, "AuthorShown").plugin_data.is_empty());
    assert_eq!(find(&rt, "SlotInstance").plugin_data, find(&doc, "SlotInstance").plugin_data);
    assert_eq!(find(&rt, "SlotInstance").component.slot_content_id.as_deref(), Some("7:42"));
}

#[test]
fn the_embedded_schema_import_agrees_with_the_real_fields_import() {
    let bytes = export_fig(&rich_document()).expect("export_fig");
    let full = import_fig(&bytes).expect("import_fig");
    let real = import_through_real_fields(&bytes);
    assert_eq!(full.nodes.len(), real.nodes.len());
    for n in full.nodes.values() {
        let other = find(&real, &n.name);
        assert_eq!(n.plugin_data, other.plugin_data, "{}", n.name);
        assert_eq!(n.component.slot_content_id, other.component.slot_content_id, "{}", n.name);
        assert_eq!(n.layout_child, other.layout_child, "{}", n.name);
    }
}

/// Files written before the export used Figma's field table carry the old names; they must keep importing.
#[test]
fn files_written_with_the_old_field_names_still_import() {
    let g = |s: u64, l: u64| -> Value { json!({"sessionID": s, "localID": l}) };
    let node = |id: (u64, u64), parent: (u64, u64), pos: &str, ty: &str, name: &str, extra: Value| -> Value {
        let mut v = json!({
            "guid": g(id.0, id.1),
            "parentIndex": {"guid": g(parent.0, parent.1), "position": pos},
            "type": ty, "name": name, "phase": "CREATED", "visible": true, "opacity": 1.0,
            "size": {"x": 120.0, "y": 60.0},
            "transform": {"m00": 1.0, "m01": 0.0, "m02": 0.0, "m10": 0.0, "m11": 1.0, "m12": 0.0}
        });
        for (k, x) in extra.as_object().unwrap() {
            v[k] = x.clone();
        }
        v
    };
    let changes = Value::Array(vec![
        node((0, 0), (0, 0), "", "DOCUMENT", "Legacy", json!({})),
        node((0, 1), (0, 0), "!", "CANVAS", "Page 1", json!({})),
        node(
            (1, 1),
            (0, 1),
            "!",
            "FRAME",
            "OldGrid",
            json!({
                "stackMode": "GRID", "gridColumns": 2, "gridRows": 2,
                "stackPaddingTop": 11.0, "stackPaddingLeft": 22.0, "stackPaddingRight": 33.0, "stackPaddingBottom": 44.0
            }),
        ),
        node((1, 2), (1, 1), "!", "RECTANGLE", "OldCell", json!({"gridColumnAnchorIndex": 1, "gridRowAnchorIndex": 1})),
        node((1, 3), (0, 1), "\"", "STICKY", "OldSticky", json!({"authorVisible": true, "textData": {"characters": "x"}})),
        node((1, 4), (0, 1), "#", "CONNECTOR", "OldConnector", json!({"connectorStartStrokeCap": "ARROW_LINES", "connectorEndStrokeCap": "ARROW_EQUILATERAL"})),
        node((1, 5), (0, 1), "$", "FRAME", "OldSlot", json!({"isSlotContent": true, "jsonSerialized": {"slot-content-id": {"guid": g(9, 9)}}})),
    ]);
    let doc = import_fig_node_changes(&changes, "Legacy").expect("import legacy names");
    assert_eq!(padding_of(find(&doc, "OldGrid")), [11.0, 33.0, 44.0, 22.0]);
    let cell = find(&doc, "OldCell");
    assert_eq!((cell.layout_child.grid_column, cell.layout_child.grid_row), (Some(1), Some(1)));
    assert!(matches!(&find(&doc, "OldSticky").kind, NodeKind::Sticky(sp) if sp.show_author));
    let NodeKind::Connector(p) = &find(&doc, "OldConnector").kind else { panic!("connector") };
    assert_eq!((p.start_cap, p.end_cap), (ConnectorCap::Arrow, ConnectorCap::Triangle));
    assert_eq!(find(&doc, "OldSlot").component.slot_content_id.as_deref(), Some("9:9"));
}
