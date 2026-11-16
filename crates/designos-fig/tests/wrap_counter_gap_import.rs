//! A wrap frame whose record carries no counter-axis spacing lays its lines out with the item spacing
//! (Figma's "Auto" line gap, the plugin API's `counterAxisSpacing = null`). An explicit counter gap,
//! zero included, is kept, and frames that do not wrap are untouched.

use designos_doc::*;
use designos_fig::{decode_fig_to_json, export_fig, export_rest_json, import_fig, import_fig_node_changes, import_rest_json};
use serde_json::{json, Map, Value};

fn stack_of(doc: &Document) -> AutoLayout {
    let n = doc.nodes.values().find(|n| n.name == "Stack").expect("Stack node");
    n.kind.frame_props().and_then(|f| f.auto_layout.clone()).expect("auto layout")
}

fn rest_stack(extra: Value) -> AutoLayout {
    let mut frame = json!({
        "id": "1:2", "name": "Stack", "type": "FRAME", "layoutMode": "HORIZONTAL", "itemSpacing": 32,
        "absoluteBoundingBox": {"x": 0, "y": 0, "width": 300, "height": 200}, "children": []
    });
    frame.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
    let doc =
        json!({"name": "f", "document": {"id": "0:0", "type": "DOCUMENT", "children": [{"id": "0:1", "name": "p", "type": "CANVAS", "children": [frame]}]}});
    stack_of(&import_rest_json(&doc.to_string()).unwrap())
}

fn kiwi_stack(extra: Value) -> AutoLayout {
    let mut stack: Map<String, Value> = json!({
        "guid": {"sessionID": 1, "localID": 2},
        "parentIndex": {"guid": {"sessionID": 0, "localID": 1}, "position": "!"},
        "type": "FRAME", "name": "Stack", "size": {"x": 300.0, "y": 200.0},
        "stackMode": "HORIZONTAL", "stackSpacing": 32.0
    })
    .as_object()
    .unwrap()
    .clone();
    stack.extend(extra.as_object().unwrap().clone());
    let changes = json!([
        {"guid": {"sessionID": 0, "localID": 0}, "type": "DOCUMENT", "name": "d"},
        {"guid": {"sessionID": 0, "localID": 1}, "parentIndex": {"guid": {"sessionID": 0, "localID": 0}, "position": "!"}, "type": "CANVAS", "name": "p"},
        Value::Object(stack)
    ]);
    stack_of(&import_fig_node_changes(&changes, "d").unwrap())
}

#[test]
fn rest_wrap_without_counter_gap_takes_the_item_spacing() {
    let al = rest_stack(json!({"layoutWrap": "WRAP"}));
    assert_eq!((al.item_spacing, al.counter_spacing, al.line_gap()), (32.0, None, 32.0));
}

#[test]
fn rest_wrap_keeps_an_explicit_counter_gap_including_zero() {
    let zero = rest_stack(json!({"layoutWrap": "WRAP", "counterAxisSpacing": 0}));
    assert_eq!((zero.counter_spacing, zero.line_gap()), (Some(0.0), 0.0));
    let twelve = rest_stack(json!({"layoutWrap": "WRAP", "counterAxisSpacing": 12}));
    assert_eq!((twelve.counter_spacing, twelve.line_gap()), (Some(12.0), 12.0));
    let equal = rest_stack(json!({"layoutWrap": "WRAP", "counterAxisSpacing": 32}));
    assert_eq!((equal.counter_spacing, equal.line_gap()), (Some(32.0), 32.0));
}

#[test]
fn rest_frames_that_do_not_wrap_keep_a_zero_counter_gap() {
    let absent = rest_stack(json!({}));
    assert_eq!((absent.counter_spacing, absent.line_gap()), (None, 0.0));
    let no_wrap = rest_stack(json!({"layoutWrap": "NO_WRAP"}));
    assert_eq!((no_wrap.counter_spacing, no_wrap.line_gap()), (None, 0.0));
}

#[test]
fn kiwi_wrap_without_counter_gap_takes_the_item_spacing() {
    let al = kiwi_stack(json!({"stackWrap": "WRAP"}));
    assert_eq!((al.item_spacing, al.counter_spacing, al.line_gap()), (32.0, None, 32.0));
}

#[test]
fn kiwi_wrap_keeps_an_explicit_counter_gap_including_zero() {
    let zero = kiwi_stack(json!({"stackWrap": "WRAP", "stackCounterSpacing": 0.0}));
    assert_eq!((zero.counter_spacing, zero.line_gap()), (Some(0.0), 0.0));
    let twelve = kiwi_stack(json!({"stackWrap": "WRAP", "stackCounterSpacing": 12.0}));
    assert_eq!((twelve.counter_spacing, twelve.line_gap()), (Some(12.0), 12.0));
    let equal = kiwi_stack(json!({"stackWrap": "WRAP", "stackCounterSpacing": 32.0}));
    assert_eq!((equal.counter_spacing, equal.line_gap()), (Some(32.0), 32.0));
}

#[test]
fn kiwi_frames_that_do_not_wrap_keep_a_zero_counter_gap() {
    let absent = kiwi_stack(json!({}));
    assert_eq!((absent.counter_spacing, absent.line_gap()), (None, 0.0));
}

/// Export writes the counter gap only when `Some` and omits it when `None`, so round-trip preserves both.
#[test]
fn counter_gap_round_trips_through_rest_json_and_fig_binary() {
    for (item, counter, expect_line_gap) in [(32.0, None, 32.0), (32.0, Some(32.0), 32.0), (32.0, Some(0.0), 0.0), (8.0, Some(12.0), 12.0)] {
        let mut d = Document::new("t");
        let page = d.add_page("p");
        let fid = d.alloc_id();
        let mut f = Node::frame(fid, Rect::new(0.0, 0.0, 300.0, 200.0));
        f.name = "Stack".into();
        let al = AutoLayout { direction: LayoutDirection::Horizontal, wrap: true, item_spacing: item, counter_spacing: counter, ..Default::default() };
        f.kind = NodeKind::Frame(FrameProps { auto_layout: Some(al), ..Default::default() });
        d.insert(page, f).unwrap();

        let rest_val = export_rest_json(&d);
        let rest_frame = &rest_val["document"]["children"][0]["children"][0];
        assert_eq!(rest_frame.get("counterAxisSpacing").and_then(Value::as_f64), counter.map(f64::from), "rest wire {counter:?}");
        let rest = import_rest_json(&rest_val.to_string()).unwrap();
        let rest_al = stack_of(&rest);
        assert_eq!((rest_al.item_spacing, rest_al.counter_spacing, rest_al.line_gap()), (item, counter, expect_line_gap), "rest round-trip");

        let fig_bytes = export_fig(&d).unwrap();
        let fig_json = decode_fig_to_json(&fig_bytes).unwrap();
        let fig_node = fig_json["message"]["nodeChanges"].as_array().unwrap().iter().find(|n| n["name"] == "Stack").unwrap();
        assert_eq!(fig_node.get("stackCounterSpacing").and_then(Value::as_f64), counter.map(f64::from), "fig wire {counter:?}");
        let fig = import_fig(&fig_bytes).unwrap();
        let fig_al = stack_of(&fig);
        assert_eq!((fig_al.item_spacing, fig_al.counter_spacing, fig_al.line_gap()), (item, counter, expect_line_gap), "fig round-trip");
    }
}

#[test]
fn instance_override_exports_counter_gap_only_when_some() {
    for (override_counter, expect_line_gap) in [(None, 16.0), (Some(24.0), 24.0)] {
        let mut d = Document::new("inst-ov");
        let page = d.add_page("p");
        let comp_id = d.alloc_id();
        let mut comp = Node::new(comp_id, NodeKind::Component(FrameProps::default()));
        comp.name = "Master".into();
        comp.size = Vec2::new(200.0, 200.0);
        d.insert(page, comp).unwrap();

        let inner_id = d.alloc_id();
        let mut inner = Node::frame(inner_id, Rect::new(0.0, 0.0, 200.0, 200.0));
        inner.name = "InnerStack".into();
        inner.kind = NodeKind::Frame(FrameProps {
            auto_layout: Some(AutoLayout {
                direction: LayoutDirection::Horizontal,
                wrap: true,
                item_spacing: 32.0,
                counter_spacing: None,
                ..Default::default()
            }),
            ..Default::default()
        });
        d.insert(comp_id, inner).unwrap();

        create_instance(&mut d, comp_id, page, Vec2::new(250.0, 0.0)).apply(&mut d).unwrap();
        let inst_id = d.nodes.values().find(|n| n.name == "Master" && matches!(n.kind, NodeKind::Instance { .. })).unwrap().id;
        let inst_inner_id = d.get(inst_id).unwrap().children[0];
        if let Some(n) = d.get_mut(inst_inner_id) {
            if let Some(al) = n.kind.frame_props_mut().and_then(|f| f.auto_layout.as_mut()) {
                al.item_spacing = 16.0;
                al.counter_spacing = override_counter;
            }
            n.component.overrides.insert(OverrideField::Layout);
        }

        let fig_bytes = export_fig(&d).unwrap();
        let fig_json = decode_fig_to_json(&fig_bytes).unwrap();
        let inst_rec = fig_json["message"]["nodeChanges"].as_array().unwrap().iter().find(|n| n["type"] == "INSTANCE").unwrap();
        let ovs = inst_rec["symbolData"]["symbolOverrides"].as_array().unwrap();
        let layout_ov = ovs.iter().find(|o| o.get("stackSpacing").is_some()).expect("layout override entry");
        assert_eq!(layout_ov.get("stackCounterSpacing").and_then(Value::as_f64), override_counter.map(f64::from));

        let reimp = import_fig(&fig_bytes).unwrap();
        let re_inst = reimp.nodes.values().find(|n| matches!(n.kind, NodeKind::Instance { .. })).unwrap();
        let re_inner = reimp.get(re_inst.children[0]).unwrap();
        let re_al = re_inner.kind.frame_props().unwrap().auto_layout.as_ref().unwrap();
        assert_eq!((re_al.item_spacing, re_al.counter_spacing, re_al.line_gap()), (16.0, override_counter, expect_line_gap));
    }
}
