//! Auto layout child margins (`stackChildMargin*`) through component instances: a cleared margin
//! survives an export / import round trip, and a scaled instance scales its children's margins.

use designos_doc::*;
use designos_fig::{decode_fig_to_json, export_fig, import_fig, import_fig_node_changes};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};

fn find<'a>(doc: &'a Document, name: &str) -> &'a Node {
    doc.nodes.values().find(|n| n.name == name).unwrap_or_else(|| panic!("no node named {name}"))
}

fn child_named<'a>(doc: &'a Document, parent: &Node, name: &str) -> &'a Node {
    parent.children.iter().filter_map(|id| doc.get(*id)).find(|n| n.name == name).unwrap_or_else(|| panic!("no child named {name} in {}", parent.name))
}

/// A component `Row` whose two children carry margins, and an instance `RowInstance` of it that clears the top margin
/// of both children (keeping the right one of `TopAndRight`) through a `Layout` override.
fn component_with_cleared_instance_margins() -> Document {
    let mut doc = Document::new("ClearedMargins");
    let page = doc.add_page("Page 1");

    let comp_id = doc.alloc_id();
    let mut comp = Node::new(
        comp_id,
        NodeKind::Component(FrameProps {
            auto_layout: Some(AutoLayout { direction: LayoutDirection::Horizontal, padding: [4.0; 4], ..Default::default() }),
            ..Default::default()
        }),
    );
    comp.name = "Row".into();
    comp.size = Vec2::new(200.0, 80.0);
    doc.insert(page, comp).unwrap();

    for (name, margin) in [("OnlyTop", [10.0, 0.0, 0.0, 0.0]), ("TopAndRight", [10.0, 6.0, 0.0, 0.0])] {
        let id = doc.alloc_id();
        let mut child = Node::rectangle(id, Rect::new(0.0, 0.0, 40.0, 20.0));
        child.name = name.into();
        child.layout_child.margin = margin;
        doc.insert(comp_id, child).unwrap();
    }

    create_instance(&mut doc, comp_id, page, Vec2::new(300.0, 0.0)).apply(&mut doc).expect("create_instance");
    let inst_id = instances_of(&doc, comp_id)[0];
    doc.get_mut(inst_id).unwrap().name = "RowInstance".into();
    for cid in doc.get(inst_id).unwrap().children.clone() {
        let ch = doc.get_mut(cid).unwrap();
        match ch.name.as_str() {
            "OnlyTop" => ch.layout_child.margin = [0.0; 4],
            "TopAndRight" => ch.layout_child.margin = [0.0, 6.0, 0.0, 0.0],
            other => panic!("unexpected instance child {other}"),
        }
        ch.component.overrides.insert(OverrideField::Layout);
    }
    doc
}

fn assert_margins_cleared_on_the_instance_and_kept_on_the_master(rt: &Document) {
    let master = find(rt, "Row");
    assert_eq!(child_named(rt, master, "OnlyTop").layout_child.margin, [10.0, 0.0, 0.0, 0.0], "the master keeps its margins");
    assert_eq!(child_named(rt, master, "TopAndRight").layout_child.margin, [10.0, 6.0, 0.0, 0.0]);

    let inst = find(rt, "RowInstance");
    assert_eq!(child_named(rt, inst, "OnlyTop").layout_child.margin, [0.0; 4], "a cleared margin must not come back from the master");
    assert_eq!(child_named(rt, inst, "TopAndRight").layout_child.margin, [0.0, 6.0, 0.0, 0.0]);
}

#[test]
fn instance_override_clearing_a_master_margin_survives_the_fig_round_trip() {
    let bytes = export_fig(&component_with_cleared_instance_margins()).expect("export_fig");
    assert_margins_cleared_on_the_instance_and_kept_on_the_master(&import_fig(&bytes).expect("import_fig"));
}

/// The exported node changes as a reader that rebuilds an instance from its master plus `symbolOverrides` sees them:
/// Figma writes an instance without child nodes, while `export_fig` also writes the instance's resolved children.
fn node_changes_without_instance_descendants(fig: &[u8]) -> Value {
    let decoded = decode_fig_to_json(fig).expect("decode_fig_to_json");
    let changes = decoded["message"]["nodeChanges"].as_array().expect("nodeChanges");
    let key = |guid: &Value| format!("{}:{}", guid["sessionID"], guid["localID"]);
    let parent_of = |c: &Value| c.get("parentIndex").and_then(|p| p.get("guid")).map(key);
    let instances: HashSet<String> = changes.iter().filter(|c| c["type"] == "INSTANCE").map(|c| key(&c["guid"])).collect();
    let parents: HashMap<String, String> = changes.iter().filter_map(|c| Some((key(&c["guid"]), parent_of(c)?))).collect();
    let under_instance = |c: &Value| {
        let mut cur = parent_of(c);
        while let Some(p) = cur {
            if instances.contains(&p) {
                return true;
            }
            cur = parents.get(&p).cloned();
        }
        false
    };
    Value::Array(changes.iter().filter(|c| !under_instance(c)).cloned().collect())
}

#[test]
fn cleared_margin_override_is_written_for_a_reader_that_rebuilds_the_instance_from_its_master() {
    let bytes = export_fig(&component_with_cleared_instance_margins()).expect("export_fig");
    let rt = import_fig_node_changes(&node_changes_without_instance_descendants(&bytes), "t").expect("import node changes");
    assert_margins_cleared_on_the_instance_and_kept_on_the_master(&rt);
}

fn g(s: u64, l: u64) -> Value {
    json!({"sessionID": s, "localID": l})
}

fn node(id: (u64, u64), parent: (u64, u64), pos: &str, ty: &str, name: &str, extra: Value) -> Value {
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

#[test]
fn uniform_scale_scales_the_copied_children_margins_with_their_padding() {
    let input = json!([
        node((0, 0), (0, 0), "!", "DOCUMENT", "Document", json!({})),
        node((0, 1), (0, 0), "a", "CANVAS", "Page", json!({})),
        node((5, 1), (0, 1), "a", "SYMBOL", "Master", json!({"size": {"x": 100.0, "y": 60.0}})),
        node(
            (5, 2),
            (5, 1),
            "a",
            "FRAME",
            "Padded",
            json!({
                "overrideKey": g(9, 1), "stackMode": "HORIZONTAL", "stackPaddingTop": 10.0, "stackSpacing": 4.0,
                "stackChildMarginTop": 10.0, "size": {"x": 50.0, "y": 30.0}
            })
        ),
        node(
            (1, 1),
            (0, 1),
            "b",
            "INSTANCE",
            "Doubled",
            json!({"size": {"x": 200.0, "y": 120.0}, "symbolData": {"symbolID": g(5, 1), "uniformScaleFactor": 2.0, "symbolOverrides": []}})
        ),
        node(
            (1, 2),
            (0, 1),
            "c",
            "INSTANCE",
            "DoubledWithOverride",
            json!({
                "size": {"x": 200.0, "y": 120.0},
                "symbolData": {"symbolID": g(5, 1), "uniformScaleFactor": 2.0, "symbolOverrides": [
                    {"guidPath": {"guids": [g(9, 1)]}, "stackChildMarginTop": 3.0}]}
            })
        ),
    ]);
    let doc = import_fig_node_changes(&input, "t").unwrap();

    let master_child = child_named(&doc, find(&doc, "Master"), "Padded");
    assert_eq!(master_child.layout_child.margin, [10.0, 0.0, 0.0, 0.0]);

    let scaled = child_named(&doc, find(&doc, "Doubled"), "Padded");
    let al = scaled.kind.frame_props().and_then(|f| f.auto_layout.as_ref()).expect("scaled child keeps its auto layout");
    assert_eq!(al.padding[0], 20.0, "padding scales");
    assert_eq!(scaled.layout_child.margin, [20.0, 0.0, 0.0, 0.0], "the margin scales with the padding");

    let overridden = child_named(&doc, find(&doc, "DoubledWithOverride"), "Padded");
    assert_eq!(overridden.layout_child.margin, [3.0, 0.0, 0.0, 0.0], "an override applied after the scale replaces the scaled margin");
}
