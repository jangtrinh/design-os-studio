//! Hug text built or restyled by agents stays consistent with the layout around it:
//! the measurer reaches every batch path, and a style or variable change that resizes a
//! label also re-lays out the hug frames that wrap it (issue #1321).
#![allow(clippy::result_large_err)]

use designos_doc::{resolve_node, NodeId, NodeKind, TextRun, Vec2};
use designos_mcp::engine::Engine;
use serde_json::{json, Value};

fn text_style(font_size: f32) -> Value {
    json!({
        "font_family": "Inter", "font_weight": 600, "italic": false, "font_size": font_size,
        "line_height": "Auto", "letter_spacing": 0.0, "paragraph_spacing": 0.0,
        "align_h": "Left", "align_v": "Top", "case": "Original", "decoration": "None", "fills": []
    })
}

fn default_page(e: &Engine) -> u64 {
    e.resolve_default_page().unwrap().0
}

fn hug_button_spec(name: &str, label: &str, font_size: Value) -> Value {
    json!({
        "kind": "frame", "name": name, "width_sizing": "hug", "height_sizing": "hug",
        "layout": {"direction": "row", "padding": [0, 12, 0, 12], "align": "center", "justify": "center"},
        "children": [{"kind": "text", "name": "Label", "text": label, "font_size": font_size, "font_weight": 600}]
    })
}

/// A hug row button (padding 12/12) with one hug label. Returns `(button, label)`.
fn hug_button(e: &mut Engine, parent: u64, name: &str, font_size: Value) -> (NodeId, NodeId) {
    let r = e.call("designos_apply_spec", json!({"parent_id": parent, "spec": hug_button_spec(name, "Delete", font_size)})).unwrap();
    let btn = NodeId(r["root_node_id"].as_u64().unwrap());
    let lbl = NodeId(r["name_to_id"][format!("{name}/Label")].as_u64().unwrap());
    (btn, lbl)
}

fn type_variables(e: &mut Engine, with_alias: bool) {
    let mut vars = vec![json!({"id": "type/body", "name": "Body", "collection": "type", "kind": "number", "values": {"m": 14.0}})];
    if with_alias {
        vars = vec![
            json!({"id": "type/base", "name": "Base", "collection": "type", "kind": "number", "values": {"m": 14.0}}),
            json!({"id": "type/body", "name": "Body", "collection": "type", "kind": "number", "values": {"m": {"alias": "type/base"}}}),
        ];
    }
    e.call(
        "designos_apply_batch",
        json!({"ops": [{"op": "set_variables",
            "upsert_collections": [{"id": "type", "name": "Type", "modes": [{"id": "m", "name": "Default"}], "default_mode": "m"}],
            "upsert_variables": vars}]}),
    )
    .unwrap();
}

fn set_number(e: &mut Engine, id: &str, v: f32) {
    e.call("designos_apply_batch", json!({"ops": [{"op": "set_variables", "upsert_variables": [{"id": id, "values": {"m": v}}]}]})).unwrap();
}

/// What the renderer measures for `id`: bindings and linked-style bindings applied.
fn drawn_measure(e: &Engine, id: NodeId) -> Vec2 {
    let n = e.doc.get(id).unwrap();
    let drawn = resolve_node(&e.doc, n);
    let NodeKind::Text(tp) = &drawn.kind else { panic!("not text") };
    designos_render::measure_text_box(tp, None)
}

fn size(e: &Engine, id: NodeId) -> Vec2 {
    e.doc.get(id).unwrap().size
}

fn root_id(r: &Value) -> NodeId {
    NodeId(r["root_node_id"].as_u64().unwrap())
}

/// The spec paths keep a 16 px minimum width for an empty label so it stays selectable. `create_node` and `update_node` give the editor's 1 px minimum.
#[test]
fn empty_label_keeps_16px_on_spec_paths_and_1px_on_create_and_update() {
    let mut e = Engine::new();
    let page = default_page(&e);

    let r = e.call("designos_apply_spec", json!({"parent_id": page, "spec": {"kind": "text", "name": "Empty", "text": ""}})).unwrap();
    assert_eq!(size(&e, root_id(&r)).x, 16.0, "empty NodeSpec text");

    let r = e
        .call(
            "designos_apply_spec",
            json!({"parent_id": page, "spec": {"kind": "frame", "name": "Gen", "width_sizing": "hug", "height_sizing": "hug", "text": ""}}),
        )
        .unwrap();
    let gen = e.doc.get(root_id(&r)).unwrap();
    let label = gen.children.iter().copied().find(|c| matches!(e.doc.get(*c).unwrap().kind, NodeKind::Text(_))).expect("generated label");
    assert_eq!(size(&e, label).x, 16.0, "generated empty label");

    e.call(
        "designos_apply_spec",
        json!({"parent_id": page, "spec": {
            "kind": "component", "name": "EmptyComp", "width_sizing": "hug", "height_sizing": "hug",
            "layout": {"direction": "row"},
            "children": [{"kind": "text", "name": "Label", "text": "OK", "font_size": 14.0, "font_weight": 600}]
        }}),
    )
    .unwrap();
    let inst = e
        .call(
            "designos_apply_spec",
            json!({"parent_id": page, "spec": {"kind": "instance", "name": "EmptyInst", "component": "EmptyComp",
                "overrides": [{"path": "Label", "text": ""}]}}),
        )
        .unwrap();
    let inst_label = NodeId(inst["name_to_id"]["EmptyInst/Label"].as_u64().unwrap());
    assert_eq!(size(&e, inst_label).x, 16.0, "instance override to empty text");

    let c = e.call("designos_create_node", json!({"parent_id": page, "kind": "text", "name": "C", "text": ""})).unwrap();
    assert_eq!(size(&e, NodeId(c["node"]["id"].as_u64().unwrap())).x, 1.0, "create_node empty text");

    let u = e.call("designos_create_node", json!({"parent_id": page, "kind": "text", "name": "U", "text": "Delete"})).unwrap();
    let uid = NodeId(u["node"]["id"].as_u64().unwrap());
    e.call("designos_update_node", json!({"node_id": uid.0, "text": ""})).unwrap();
    assert_eq!(size(&e, uid).x, 1.0, "update_node to empty text");
}

/// Upserting a text style that a label is linked to resizes the label, so the hug frame
/// around it must follow.
#[test]
fn set_style_upsert_relays_out_the_hug_parent() {
    let mut e = Engine::new();
    let page = default_page(&e);
    let (btn, lbl) = hug_button(&mut e, page, "Btn", json!(14.0));
    e.call(
        "designos_apply_batch",
        json!({"ops": [{"op": "set_style",
            "upsert": [{"key": "lbl", "name": "Lbl", "property": "text", "text_style": text_style(14.0)}],
            "apply": [{"node": lbl.0, "style": "lbl"}]}]}),
    )
    .unwrap();
    let (button_before, label_before) = (size(&e, btn), size(&e, lbl));
    e.call("designos_apply_batch", json!({"ops": [{"op": "set_style", "upsert": [{"key": "lbl", "text_style": text_style(36.0)}]}]})).unwrap();
    let (b, l) = (size(&e, btn), size(&e, lbl));
    assert!(l.x > label_before.x, "label must have grown: {label_before:?} -> {l:?}");
    assert_eq!(b.x, 24.0 + l.x, "hug button width {} must equal padding 24 + label {}", b.x, l.x);
    assert_eq!(b.y, l.y, "hug button height {} must equal label {}", b.y, l.y);

    // One undo step restores the label and the frame around it; redo brings both back.
    e.call("designos_undo", json!({})).unwrap();
    assert_eq!((size(&e, btn), size(&e, lbl)), (button_before, label_before), "undo must restore the label and its hug button");
    e.call("designos_redo", json!({})).unwrap();
    assert_eq!((size(&e, btn), size(&e, lbl)), (b, l), "redo must bring back the grown label and its hug button");
}

/// Control: `update_node` already re-measured its label before the batch paths did. Its undo
/// restores the hug button too, which is what the batch paths are held to.
#[test]
fn update_node_undo_restores_the_hug_parent() {
    let mut e = Engine::new();
    let page = default_page(&e);
    let (btn, lbl) = hug_button(&mut e, page, "Btn", json!(14.0));
    let (button_before, label_before) = (size(&e, btn), size(&e, lbl));
    e.call("designos_update_node", json!({"node_id": lbl.0, "text": "Delete everything now"})).unwrap();
    let (b, l) = (size(&e, btn), size(&e, lbl));
    assert!(l.x > label_before.x, "label must have grown: {label_before:?} -> {l:?}");
    assert_eq!(b.x, 24.0 + l.x, "update_node: hug button width {} must equal 24 + label {}", b.x, l.x);
    e.call("designos_undo", json!({})).unwrap();
    assert_eq!((size(&e, btn), size(&e, lbl)), (button_before, label_before), "undo of update_node must restore the label and its hug button");
}

/// A text linked to an upserted text style also receives a fill style in the same op:
/// the fill apply must not hide the new text style from it.
#[test]
fn set_style_upsert_reaches_a_text_that_also_gets_a_fill_style() {
    let mut e = Engine::new();
    let page = default_page(&e);
    let r = e
        .call("designos_apply_spec", json!({"parent_id": page, "spec": {"kind": "text", "name": "T", "text": "Delete", "font_size": 14.0, "font_weight": 600}}))
        .unwrap();
    let id = root_id(&r);
    e.call(
        "designos_apply_batch",
        json!({"ops": [{"op": "set_style",
            "upsert": [{"key": "k", "name": "K", "property": "text", "text_style": text_style(14.0)}],
            "apply": [{"node": id.0, "style": "k"}]}]}),
    )
    .unwrap();
    e.call(
        "designos_apply_batch",
        json!({"ops": [{"op": "set_style",
            "upsert": [{"key": "k", "text_style": text_style(36.0)}, {"key": "f", "name": "F", "property": "fill", "fill": "#ff0000"}],
            "apply": [{"node": id.0, "style": "f"}]}]}),
    )
    .unwrap();
    let n = e.doc.get(id).unwrap();
    let NodeKind::Text(tp) = &n.kind else { panic!("not text") };
    assert_eq!(tp.style.font_size, 36.0, "text linked to the upserted style keeps the old font size");
    assert_eq!(n.style_refs.get("fill").map(String::as_str), Some("f"), "the fill style of the same op must still be applied");
    assert_eq!(n.size, drawn_measure(&e, id), "the box follows the new font size");
}

/// A bound label on a second page is re-measured by `set_variables`, so its hug parent
/// there must follow too (the default page is not special).
#[test]
fn set_variables_relays_out_hug_parents_on_every_page() {
    let mut e = Engine::new();
    let page = default_page(&e);
    let second = e.doc.add_page("Second").0;
    type_variables(&mut e, false);
    let (btn1, lbl1) = hug_button(&mut e, page, "Btn1", json!("var:type/Body"));
    let (btn2, lbl2) = hug_button(&mut e, second, "Btn2", json!("var:type/Body"));
    let before = [size(&e, btn1), size(&e, lbl1), size(&e, btn2), size(&e, lbl2)];
    set_number(&mut e, "type/body", 28.0);
    assert!(size(&e, lbl2).x > before[3].x, "label on the second page must have grown");
    assert_eq!(size(&e, btn1).x, 24.0 + size(&e, lbl1).x, "default-page control");
    assert_eq!(size(&e, btn2).x, 24.0 + size(&e, lbl2).x, "second-page hug button {} must equal 24 + label {}", size(&e, btn2).x, size(&e, lbl2).x);
    let after = [size(&e, btn1), size(&e, lbl1), size(&e, btn2), size(&e, lbl2)];

    // One undo step restores every label and the frame around it, on both pages; redo brings them back.
    e.call("designos_undo", json!({})).unwrap();
    assert_eq!([size(&e, btn1), size(&e, lbl1), size(&e, btn2), size(&e, lbl2)], before, "undo must restore both pages");
    e.call("designos_redo", json!({})).unwrap();
    assert_eq!([size(&e, btn1), size(&e, lbl1), size(&e, btn2), size(&e, lbl2)], after, "redo must bring both pages back");
}

/// A bound text with a run: the renderer draws the run's size, which the binding rewrites.
#[test]
fn set_variables_measures_what_is_drawn_for_text_with_runs() {
    let mut e = Engine::new();
    let page = default_page(&e);
    type_variables(&mut e, false);
    let r = e
        .call(
            "designos_apply_spec",
            json!({"parent_id": page, "spec": {"kind": "text", "name": "Bound", "text": "Delete", "font_size": "var:type/Body", "font_weight": 600}}),
        )
        .unwrap();
    let id = root_id(&r);
    if let NodeKind::Text(tp) = &mut e.doc.get_mut(id).unwrap().kind {
        let style = tp.style.clone();
        tp.runs = vec![TextRun { start: 0, end: tp.content.len(), style }];
    }
    let before = size(&e, id);
    set_number(&mut e, "type/body", 28.0);
    let after = size(&e, id);
    assert!(after.x > before.x, "fontSize 14 -> 28 must widen the box ({before:?} -> {after:?})");
    assert_eq!(after, drawn_measure(&e, id), "stored box must equal the measure of what is drawn");
}

/// Bound properties other than the font size change the box too.
#[test]
fn set_variables_measures_a_bound_line_height() {
    let mut e = Engine::new();
    let page = default_page(&e);
    e.call(
        "designos_apply_batch",
        json!({"ops": [{"op": "set_variables",
            "upsert_collections": [{"id": "type", "name": "Type", "modes": [{"id": "m", "name": "Default"}], "default_mode": "m"}],
            "upsert_variables": [{"id": "type/lh", "name": "Lh", "collection": "type", "kind": "number", "values": {"m": 20.0}}]}]}),
    )
    .unwrap();
    let r = e
        .call(
            "designos_apply_spec",
            json!({"parent_id": page, "spec": {"kind": "text", "name": "Bound", "text": "Delete", "font_size": 14.0, "line_height": "var:type/Lh", "font_weight": 600}}),
        )
        .unwrap();
    let id = root_id(&r);
    let before = size(&e, id);
    set_number(&mut e, "type/lh", 60.0);
    let after = size(&e, id);
    assert!(after.y > before.y, "line height 20 -> 60 must grow the box ({before:?} -> {after:?})");
    assert_eq!(after, drawn_measure(&e, id));
}

/// A label bound to an alias is re-measured when the variable the alias points at changes.
#[test]
fn set_variables_follows_alias_chains() {
    let mut e = Engine::new();
    let page = default_page(&e);
    type_variables(&mut e, true);
    let r = e
        .call(
            "designos_apply_spec",
            json!({"parent_id": page, "spec": {"kind": "text", "name": "Bound", "text": "Delete", "font_size": "var:type/Body", "font_weight": 600}}),
        )
        .unwrap();
    let id = root_id(&r);
    let before = size(&e, id);
    set_number(&mut e, "type/base", 28.0);
    let after = size(&e, id);
    assert!(after.x > before.x, "base 14 -> 28 must widen a label bound to its alias ({before:?} -> {after:?})");
    assert_eq!(after, drawn_measure(&e, id));
}

/// The batch `apply_spec` op sizes hug text with the renderer, not the estimate.
#[test]
fn batch_apply_spec_op_measures_hug_text() {
    let mut e = Engine::new();
    let page = default_page(&e);
    let r = e
        .call(
            "designos_apply_batch",
            json!({"ops": [{"op": "apply_spec", "as": "btn", "parent": page, "spec": hug_button_spec("Btn", "Delete", json!(14.0))}]}),
        )
        .unwrap();
    let btn = NodeId(r["created"][0].as_u64().expect("created root id"));
    let lbl = e.doc.get(btn).unwrap().children[0];
    assert_eq!(size(&e, lbl), drawn_measure(&e, lbl), "batch apply_spec label must be the shaped box, not the estimate");
    assert_eq!(size(&e, btn).x, 24.0 + size(&e, lbl).x);
}

/// `set_slot` builds the filled instance with the renderer, so a slot text in a family
/// the renderer does not have is reported as measured with a fallback font. Without a
/// measurer the report is silent.
#[test]
fn batch_set_slot_reports_fallback_measure() {
    let mut e = Engine::new();
    let page = default_page(&e);
    let comp = e
        .call(
            "designos_apply_spec",
            json!({"parent_id": page, "spec": {"kind": "component", "name": "Card", "width_sizing": "hug", "height_sizing": "hug",
                "layout": {"direction": "column"},
                "children": [{"kind": "frame", "name": "body", "width_sizing": "hug", "height_sizing": "hug", "layout": {"direction": "column"}}]}}),
        )
        .unwrap();
    let body = e.doc.get(root_id(&comp)).unwrap().children[0];
    e.doc.get_mut(body).unwrap().component.is_slot = true;
    let inst = e.call("designos_apply_spec", json!({"parent_id": page, "spec": {"kind": "instance", "name": "CardInst", "component": "Card"}})).unwrap();
    let fill = |family: &str| json!([{"kind": "text", "name": "Filled", "text": "Delete", "font_family": family, "font_size": 14.0}]);
    let slot_op = |family: &str| json!({"ops": [{"op": "set_slot", "node": root_id(&inst).0, "slot": "body", "fill": fill(family)}]});

    let inter = e.call("designos_apply_batch", slot_op("Inter")).unwrap();
    assert!(inter.get("measured_with_fallback").is_none(), "an Inter label is not a fallback: {inter}");

    let missing = e.call("designos_apply_batch", slot_op("NoSuchFamily")).unwrap();
    let ids = missing["measured_with_fallback"].as_array().expect("measured_with_fallback in the set_slot reply");
    assert!(!ids.is_empty(), "the slot text in a missing family must be reported");
    let warnings = missing["warnings"].as_array().expect("warnings in the set_slot reply");
    assert!(warnings.iter().any(|w| w["code"] == "TEXT_MEASURED_WITH_FALLBACK_FONT"), "warning code missing: {warnings:?}");
}

/// Time of one batch of `ops` colour-variable `set_variables` ops on `e`.
fn time_colour_ops(e: &mut Engine, ops: usize) -> std::time::Duration {
    e.call(
        "designos_apply_batch",
        json!({"ops": [{"op": "set_variables",
            "upsert_collections": [{"id": "c", "name": "C", "modes": [{"id": "m", "name": "Default"}], "default_mode": "m"}],
            "upsert_variables": [{"id": "c/x", "name": "X", "collection": "c", "kind": "color", "values": {"m": "#000000"}}]}]}),
    )
    .unwrap();
    let ops: Vec<Value> =
        (0..ops).map(|i| json!({"op": "set_variables", "upsert_variables": [{"id": "c/x", "values": {"m": format!("#0000{i:02x}")}}]})).collect();
    let started = std::time::Instant::now();
    e.call("designos_apply_batch", json!({"ops": ops})).unwrap();
    started.elapsed()
}

fn engine_with_rectangles(count: u32) -> Engine {
    let mut e = Engine::new();
    let bulk = e.doc.add_page("Bulk");
    let holder = e.doc.alloc_id();
    let holder = e.doc.insert(bulk, designos_doc::Node::frame(holder, designos_doc::Rect::new(0.0, 0.0, 10.0, 10.0))).unwrap();
    for i in 0..count {
        let id = e.doc.alloc_id();
        e.doc.insert(holder, designos_doc::Node::rectangle(id, designos_doc::Rect::new(i as f32, 0.0, 1.0, 1.0))).unwrap();
    }
    e
}

/// Colour variables cannot be bound to a text size, so changing them must not walk every
/// node of the document. The cost of 100 such ops is compared with the same batch on a tiny
/// document, so the check does not depend on the speed of the machine. What stays is the
/// per-batch copy of the document (about 20x here); a scan per op is over 1000x.
#[test]
fn colour_variable_ops_do_not_scan_the_document() {
    let best_of = |count: u32| (0..3).map(|_| time_colour_ops(&mut engine_with_rectangles(count), 100)).min().unwrap();
    let small = best_of(10);
    let large = best_of(40_000);
    let ratio = large.as_secs_f64() / small.as_secs_f64().max(1e-6);
    eprintln!("colour ops: 10 nodes {small:?}, 40k nodes {large:?}, ratio {ratio:.1}");
    assert!(ratio < 150.0, "100 colour-variable ops took {ratio:.1}x longer on 40k nodes ({large:?}) than on 10 ({small:?}): they scan the document");
}
