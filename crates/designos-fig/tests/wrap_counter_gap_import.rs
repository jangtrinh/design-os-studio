//! A wrap frame whose record carries no counter-axis spacing lays its lines out with the item spacing
//! (Figma's "Auto" line gap, the plugin API's `counterAxisSpacing = null`). An explicit counter gap,
//! zero included, is kept, and frames that do not wrap are untouched.

use designos_doc::*;
use designos_fig::{export_fig, export_rest_json, import_fig, import_fig_node_changes, import_rest_json};
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
    assert_eq!((al.item_spacing, al.counter_spacing), (32.0, 32.0));
}

#[test]
fn rest_wrap_keeps_an_explicit_counter_gap_including_zero() {
    assert_eq!(rest_stack(json!({"layoutWrap": "WRAP", "counterAxisSpacing": 0})).counter_spacing, 0.0);
    assert_eq!(rest_stack(json!({"layoutWrap": "WRAP", "counterAxisSpacing": 12})).counter_spacing, 12.0);
}

#[test]
fn rest_frames_that_do_not_wrap_keep_a_zero_counter_gap() {
    assert_eq!(rest_stack(json!({})).counter_spacing, 0.0);
    assert_eq!(rest_stack(json!({"layoutWrap": "NO_WRAP"})).counter_spacing, 0.0);
}

#[test]
fn kiwi_wrap_without_counter_gap_takes_the_item_spacing() {
    let al = kiwi_stack(json!({"stackWrap": "WRAP"}));
    assert_eq!((al.item_spacing, al.counter_spacing), (32.0, 32.0));
}

#[test]
fn kiwi_wrap_keeps_an_explicit_counter_gap_including_zero() {
    assert_eq!(kiwi_stack(json!({"stackWrap": "WRAP", "stackCounterSpacing": 0.0})).counter_spacing, 0.0);
    assert_eq!(kiwi_stack(json!({"stackWrap": "WRAP", "stackCounterSpacing": 12.0})).counter_spacing, 12.0);
}

#[test]
fn kiwi_frames_that_do_not_wrap_keep_a_zero_counter_gap() {
    assert_eq!(kiwi_stack(json!({})).counter_spacing, 0.0);
}

/// Export writes the counter gap explicitly, so a re-import neither changes nor re-derives it.
#[test]
fn counter_gap_round_trips_through_rest_json_and_fig_binary() {
    for (item, counter) in [(32.0, 32.0), (32.0, 0.0), (8.0, 12.0)] {
        let mut d = Document::new("t");
        let page = d.add_page("p");
        let fid = d.alloc_id();
        let mut f = Node::frame(fid, Rect::new(0.0, 0.0, 300.0, 200.0));
        f.name = "Stack".into();
        let al = AutoLayout { direction: LayoutDirection::Horizontal, wrap: true, item_spacing: item, counter_spacing: counter, ..Default::default() };
        f.kind = NodeKind::Frame(FrameProps { auto_layout: Some(al), ..Default::default() });
        d.insert(page, f).unwrap();

        let rest = import_rest_json(&export_rest_json(&d).to_string()).unwrap();
        assert_eq!((stack_of(&rest).item_spacing, stack_of(&rest).counter_spacing), (item, counter), "rest");
        let fig = import_fig(&export_fig(&d).unwrap()).unwrap();
        assert_eq!((stack_of(&fig).item_spacing, stack_of(&fig).counter_spacing), (item, counter), "fig");
    }
}
