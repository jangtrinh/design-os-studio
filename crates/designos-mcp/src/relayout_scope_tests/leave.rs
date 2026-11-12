//! A layer that leaves a frame (deleted, moved out, replaced in a slot) re-solves that frame and
//! climbs only while a frame's own size changes. The fixtures are not a fixed point of the solver,
//! so a solve that reaches a layer outside the frame shows up as a layer that moved.

use super::{add_frame, add_rect, auto_layout, fully_solved, geometry, moved, solve_everything, Geometry};
use crate::engine::Engine;
use designos_doc::{LayoutAlign, LayoutDirection, NodeId, NodeKind, Rect, Sizing};
use serde_json::{json, Value};

/// screen (column, fixed, gap 0) > [holder > [a, b], tail]. The tail is stored at y = 200; a solve of
/// the screen would put it right below the holder.
struct Holder {
    e: Engine,
    screen: NodeId,
    holder: NodeId,
    b: NodeId,
    tail: NodeId,
}

fn holder_with(kind: NodeKind, rect: Rect) -> Holder {
    let mut e = Engine::new();
    let page = e.resolve_default_page().unwrap();
    let screen = add_frame(
        &mut e,
        page,
        Rect::new(0.0, 0.0, 400.0, 600.0),
        auto_layout(LayoutDirection::Vertical, Sizing::Fixed, Sizing::Fixed, LayoutAlign::Start, 0.0),
        "screen",
    );
    let holder = add_frame(&mut e, screen, rect, kind, "holder");
    add_rect(&mut e, holder, Rect::new(0.0, 0.0, 40.0, 20.0), "a");
    let b = add_rect(&mut e, holder, Rect::new(0.0, 0.0, 40.0, 20.0), "b");
    let tail = add_rect(&mut e, screen, Rect::new(0.0, 200.0, 100.0, 30.0), "tail");
    assert_ne!(fully_solved(&e.doc), geometry(&e.doc), "the fixture must not be a fixed point of the solver");
    Holder { e, screen, holder, b, tail }
}

fn fixed_holder() -> Holder {
    holder_with(auto_layout(LayoutDirection::Horizontal, Sizing::Fixed, Sizing::Fixed, LayoutAlign::Start, 0.0), Rect::new(0.0, 0.0, 120.0, 56.0))
}

fn subtree(e: &Engine, root: NodeId) -> Vec<u64> {
    let mut ids = Vec::new();
    e.doc.walk(root, |n, _| ids.push(n.id.0));
    ids
}

/// Layers that moved and sit outside the subtree of `frame`.
fn moved_outside(e: &Engine, before: &Geometry, frame: NodeId) -> Vec<u64> {
    let inside = subtree(e, frame);
    moved(before, &geometry(&e.doc)).into_iter().filter(|id| !inside.contains(id)).collect()
}

fn ty(e: &Engine, id: NodeId) -> f32 {
    e.doc.get(id).unwrap().transform.ty
}

#[test]
fn deleting_a_layer_inside_a_fixed_size_frame_moves_nothing_outside_it() {
    let Holder { mut e, holder, b, tail, .. } = fixed_holder();
    let before = geometry(&e.doc);

    e.call("designos_delete_node", json!({ "node_id": b.0 })).unwrap();

    assert!(e.doc.get(b).is_none());
    assert_eq!(moved_outside(&e, &before, holder), Vec::<u64>::new());
    assert_eq!(ty(&e, tail), 200.0, "the tail keeps the place the document stored");
}

#[test]
fn changing_bound_padding_on_a_fixed_size_frame_moves_nothing_outside_it() {
    let Holder { mut e, holder, tail, .. } = fixed_holder();
    e.call(
        "designos_apply_batch",
        json!({ "ops": [{ "op": "set_variables",
            "upsert_collections": [{ "id": "space", "name": "Space", "modes": [{ "id": "m1", "name": "One" }], "default_mode": "m1" }],
            "upsert_variables": [{ "id": "pad", "name": "pad", "collection": "space", "kind": "number", "values": { "m1": 0 } }] }] }),
    )
    .unwrap();
    e.doc.get_mut(holder).unwrap().variables.bindings.insert("paddingLeft".into(), "pad".into());
    let before = geometry(&e.doc);

    e.call("designos_apply_batch", json!({ "ops": [{ "op": "set_variables", "upsert_variables": [{ "id": "pad", "values": { "m1": 20 } }] }] })).unwrap();

    assert_eq!(moved_outside(&e, &before, holder), Vec::<u64>::new());
    assert_eq!(ty(&e, tail), 200.0, "the tail keeps the place the document stored");
}

#[test]
fn changing_bound_inset_stroke_on_a_fixed_size_frame_moves_nothing_outside_it() {
    let Holder { mut e, holder, tail, .. } = fixed_holder();
    {
        let node = e.doc.get_mut(holder).unwrap();
        node.stroke.paints = vec![designos_doc::Paint::solid(designos_doc::Color::BLACK)];
        node.stroke.weight = 2.0;
        node.stroke.align = designos_doc::StrokeAlign::Inside;
        node.variables.bindings.insert("strokeWeight".into(), "border".into());
        if let Some(al) = node.kind.frame_props_mut().and_then(|f| f.auto_layout.as_mut()) {
            al.layout_version = 0;
            al.stroke_included = true;
        }
    }
    e.call(
        "designos_apply_batch",
        json!({ "ops": [{ "op": "set_variables",
            "upsert_collections": [{ "id": "paint", "name": "Paint", "modes": [{ "id": "m1", "name": "One" }], "default_mode": "m1" }],
            "upsert_variables": [{ "id": "border", "name": "border", "collection": "paint", "kind": "number", "values": { "m1": 2 } }] }] }),
    )
    .unwrap();
    let before = geometry(&e.doc);

    e.call("designos_apply_batch", json!({ "ops": [{ "op": "set_variables", "upsert_variables": [{ "id": "border", "values": { "m1": 10 } }] }] })).unwrap();

    assert_eq!(moved_outside(&e, &before, holder), Vec::<u64>::new());
    assert_eq!(ty(&e, tail), 200.0, "the tail keeps the place the document stored");
}

#[test]
fn deleting_a_layer_inside_a_hug_frame_shrinks_it_and_re_solves_the_frame_above() {
    let Holder { mut e, holder, b, tail, .. } =
        holder_with(auto_layout(LayoutDirection::Horizontal, Sizing::Hug, Sizing::Hug, LayoutAlign::Start, 0.0), Rect::new(0.0, 0.0, 120.0, 56.0));

    e.call("designos_delete_node", json!({ "node_id": b.0 })).unwrap();

    let size = e.doc.get(holder).unwrap().size;
    assert_eq!((size.x, size.y), (40.0, 20.0), "the frame hugs what is left");
    assert_eq!(ty(&e, tail), 20.0, "the screen follows the smaller frame");
}

#[test]
fn deleting_a_layer_inside_a_group_moves_nothing_outside_it() {
    let Holder { mut e, holder, b, tail, .. } = holder_with(NodeKind::Group, Rect::new(0.0, 0.0, 80.0, 20.0));
    e.doc.get_mut(b).unwrap().transform.tx = 40.0;
    let before = geometry(&e.doc);

    e.call("designos_delete_node", json!({ "node_id": b.0 })).unwrap();

    let size = e.doc.get(holder).unwrap().size;
    assert_eq!((size.x, size.y), (80.0, 20.0), "a group keeps the bounds it was stored with; no edit here refits it, so there is no new size to pass up");
    assert_eq!(moved_outside(&e, &before, holder), Vec::<u64>::new());
    assert_eq!(ty(&e, tail), 200.0);
}

#[test]
fn a_batch_delete_inside_a_fixed_size_frame_moves_nothing_outside_it() {
    let Holder { mut e, holder, b, tail, .. } = fixed_holder();
    let before = geometry(&e.doc);

    e.call("designos_apply_batch", json!({ "ops": [{ "op": "delete", "node": b.0 }] })).unwrap();

    assert!(e.doc.get(b).is_none());
    assert_eq!(moved_outside(&e, &before, holder), Vec::<u64>::new());
    assert_eq!(ty(&e, tail), 200.0);
}

#[test]
fn a_batch_apply_spec_inside_a_fixed_size_frame_moves_nothing_outside_it() {
    let Holder { mut e, holder, tail, .. } = fixed_holder();
    let before = geometry(&e.doc);
    e.call("designos_apply_batch", json!({"ops":[{"op":"apply_spec","parent":holder.0,"spec":{"kind":"rectangle","name":"new","width":20,"height":20}}]}))
        .unwrap();
    assert_eq!(moved_outside(&e, &before, holder), Vec::<u64>::new());
    assert_eq!(ty(&e, tail), 200.0);
}

#[test]
fn a_batch_move_out_of_a_fixed_size_frame_re_solves_the_frame_it_joins_and_not_the_one_it_left_above() {
    let Holder { mut e, screen, holder, b, tail, .. } = fixed_holder();
    let sink = add_frame(
        &mut e,
        screen,
        Rect::new(0.0, 150.0, 200.0, 40.0),
        auto_layout(LayoutDirection::Horizontal, Sizing::Fixed, Sizing::Fixed, LayoutAlign::Start, 0.0),
        "sink",
    );
    let before = geometry(&e.doc);

    e.call("designos_apply_batch", json!({ "ops": [{ "op": "move", "node": b.0, "parent": sink.0 }] })).unwrap();

    assert_eq!(e.doc.get(b).unwrap().parent, Some(sink));
    let outside_both: Vec<u64> = moved_outside(&e, &before, holder).into_iter().filter(|id| !subtree(&e, sink).contains(id)).collect();
    assert_eq!(outside_both, Vec::<u64>::new());
    assert_eq!(ty(&e, tail), 200.0);
}

#[test]
fn lint_fix_removing_an_empty_frame_inside_a_fixed_size_frame_moves_nothing_outside_it() {
    let Holder { mut e, holder, tail, .. } = fixed_holder();
    let spacer = add_frame(&mut e, holder, Rect::new(0.0, 0.0, 30.0, 30.0), NodeKind::Frame(Default::default()), "spacer");
    e.doc.get_mut(spacer).unwrap().fills.clear();
    let before = geometry(&e.doc);

    let reply = e.call("designos_lint_fix", json!({ "rules": ["empty-container"] })).unwrap();

    assert_eq!(reply["fixed_count"], 1);
    assert!(e.doc.get(spacer).is_none());
    assert_eq!(moved_outside(&e, &before, holder), Vec::<u64>::new());
    assert_eq!(ty(&e, tail), 200.0);
}

/// Two columns in a screen, each holding a layer and an empty frame; the screen stores its tail far below.
fn two_columns_with_empty_frames() -> (Engine, Vec<NodeId>) {
    let mut e = Engine::new();
    let page = e.resolve_default_page().unwrap();
    let screen = add_frame(
        &mut e,
        page,
        Rect::new(0.0, 0.0, 400.0, 600.0),
        auto_layout(LayoutDirection::Vertical, Sizing::Fixed, Sizing::Fixed, LayoutAlign::Start, 0.0),
        "screen",
    );
    let mut spacers = Vec::new();
    for name in ["left", "right"] {
        let column = add_frame(
            &mut e,
            screen,
            Rect::new(0.0, 0.0, 200.0, 50.0),
            auto_layout(LayoutDirection::Vertical, Sizing::Hug, Sizing::Fixed, LayoutAlign::Start, 0.0),
            name,
        );
        add_rect(&mut e, column, Rect::new(0.0, 0.0, 100.0, 20.0), "content");
        let spacer = add_frame(&mut e, column, Rect::new(0.0, 0.0, 30.0, 30.0), NodeKind::Frame(Default::default()), "spacer");
        e.doc.get_mut(spacer).unwrap().fills.clear();
        spacers.push(spacer);
    }
    (e, spacers)
}

#[test]
fn lint_fix_runs_one_solve_pass_for_every_fix_it_applies() {
    let (mut e, spacers) = two_columns_with_empty_frames();
    crate::tools::batch::take_relayout_passes();

    let reply: Value = e.call("designos_lint_fix", json!({ "rules": ["empty-container"] })).unwrap();

    assert_eq!(reply["fixed_count"], 2);
    assert!(spacers.iter().all(|s| e.doc.get(*s).is_none()), "both empty frames are removed");
    assert_eq!(crate::tools::batch::take_relayout_passes(), 1, "the two columns are solved together, each once");
}

#[test]
fn replacing_slot_content_in_a_fixed_size_instance_moves_nothing_outside_it() {
    let mut e = Engine { doc: designos_doc::fixtures::card_with_slots(), ..Engine::default() };
    let page = e.doc.pages[0];
    let holder = add_frame(
        &mut e,
        page,
        Rect::new(500.0, 0.0, 600.0, 400.0),
        auto_layout(LayoutDirection::Horizontal, Sizing::Fixed, Sizing::Fixed, LayoutAlign::Start, 0.0),
        "holder",
    );
    let spec = json!({ "kind": "instance", "name": "card", "component": "Card" });
    let reply = e.call("designos_apply_spec", json!({ "parent_id": holder.0, "spec": spec })).unwrap();
    let card = NodeId(reply["root_node_id"].as_u64().unwrap());
    let side = add_rect(&mut e, holder, Rect::new(0.0, 300.0, 40.0, 40.0), "side");
    if let Some(al) = e.doc.get_mut(card).and_then(|n| n.kind.frame_props_mut()).and_then(|f| f.auto_layout.as_mut()) {
        al.primary_sizing = Sizing::Fixed;
    }
    let before = geometry(&e.doc);

    e.call(
        "designos_apply_batch",
        json!({ "ops": [{ "op": "set_slot", "node": card.0, "slot": "body", "fill": [{ "kind": "text", "name": "Hello", "text": "Hello" }] }] }),
    )
    .unwrap();

    let after = geometry(&e.doc);
    assert_eq!(after[&card.0][3], before[&card.0][3], "the instance keeps its fixed height");
    assert_eq!(moved(&before, &after).into_iter().filter(|id| *id == side.0).collect::<Vec<_>>(), Vec::<u64>::new());
}

#[test]
fn filling_a_slot_of_a_hug_instance_moves_the_frame_below_it_up_the_page() {
    let mut e = Engine { doc: designos_doc::fixtures::card_with_slots(), ..Engine::default() };
    let page = e.doc.pages[0];
    let screen = add_frame(
        &mut e,
        page,
        Rect::new(500.0, 0.0, 400.0, 900.0),
        auto_layout(LayoutDirection::Vertical, Sizing::Fixed, Sizing::Fixed, LayoutAlign::Start, 0.0),
        "screen",
    );
    let spec = json!({ "kind": "instance", "name": "card", "component": "Card" });
    let reply = e.call("designos_apply_spec", json!({ "parent_id": screen.0, "spec": spec })).unwrap();
    let card = NodeId(reply["root_node_id"].as_u64().unwrap());
    let tail = add_rect(&mut e, screen, Rect::new(0.0, 700.0, 50.0, 30.0), "tail");
    let fill: Vec<Value> = (0..3).map(|i| json!({ "kind": "text", "name": format!("line {i}"), "text": "A line of body text that needs room" })).collect();

    e.call("designos_apply_batch", json!({ "ops": [{ "op": "set_slot", "node": card.0, "slot": "body", "fill": fill }] })).unwrap();

    let card_node = e.doc.get(card).unwrap();
    assert_eq!(ty(&e, tail), card_node.transform.ty + card_node.size.y, "the hug card grew, so the frame below follows its new bottom edge");
}

/// screen (column, fixed) > [holder (row, fixed 120x56) > [a, b, c], tail] with a connector from c to the
/// tail, drawn and then put back to the stored geometry so the next edit starts from an unsolved document.
fn fixed_holder_with_connector() -> (Holder, NodeId, NodeId) {
    let mut h = fixed_holder();
    let a = h.e.doc.get(h.holder).unwrap().children[0];
    let c = add_rect(&mut h.e, h.holder, Rect::new(80.0, 0.0, 40.0, 20.0), "c");
    let stored = geometry(&h.e.doc);
    h.e.call("designos_apply_batch", json!({ "ops": [{ "op": "connect", "from": c.0, "to": h.tail.0 }] })).unwrap();
    super::restore_geometry(&mut h.e, &stored);
    (h, a, c)
}

#[test]
fn deleting_a_layer_inside_a_fixed_size_frame_with_a_connector_moves_nothing_outside_it() {
    let (Holder { mut e, holder, tail, .. }, a, _) = fixed_holder_with_connector();
    let before = geometry(&e.doc);

    e.call("designos_delete_node", json!({ "node_id": a.0 })).unwrap();

    assert!(e.doc.get(a).is_none());
    let outside: Vec<u64> =
        moved_outside(&e, &before, holder).into_iter().filter(|id| !matches!(e.doc.get(NodeId(*id)).map(|n| &n.kind), Some(NodeKind::Connector(_)))).collect();
    assert_eq!(outside, Vec::<u64>::new());
    assert_eq!(ty(&e, tail), 200.0, "the tail keeps the place the document stored");
}

#[test]
fn a_connector_that_is_a_flow_child_is_laid_out_again_after_its_ends_move() {
    let mut e = Engine::new();
    let page = e.resolve_default_page().unwrap();
    let row = add_frame(
        &mut e,
        page,
        Rect::new(0.0, 0.0, 300.0, 60.0),
        auto_layout(LayoutDirection::Horizontal, Sizing::Hug, Sizing::Fixed, LayoutAlign::Start, 0.0),
        "row",
    );
    let a = add_rect(&mut e, row, Rect::new(0.0, 0.0, 40.0, 20.0), "a");
    let b = add_rect(&mut e, row, Rect::new(40.0, 0.0, 40.0, 20.0), "b");
    let d = add_rect(&mut e, row, Rect::new(80.0, 0.0, 40.0, 20.0), "d");
    let reply = e.call("designos_apply_batch", json!({ "ops": [{ "op": "connect", "from": a.0, "to": d.0 }] })).unwrap();
    let connector = NodeId(reply["created"][0].as_u64().unwrap());
    e.doc.get_mut(connector).unwrap().layout_child.absolute = false;
    solve_everything(&mut e);

    e.call("designos_update_node", json!({ "node_id": b.0, "width": 90.0 })).unwrap();

    let after = geometry(&e.doc);
    assert_eq!(after.get(&d.0).map(|g| g[0]), Some(130.0), "the end moved");
    assert_eq!(after, fully_solved(&e.doc), "the connector sits in its flow slot and the hug row wraps it");
}

#[test]
fn a_batch_delete_inside_a_fixed_size_frame_with_a_connector_moves_nothing_outside_it() {
    let (Holder { mut e, holder, tail, .. }, a, _) = fixed_holder_with_connector();
    let before = geometry(&e.doc);

    e.call("designos_apply_batch", json!({ "ops": [{ "op": "delete", "node": a.0 }] })).unwrap();

    assert!(e.doc.get(a).is_none());
    let outside: Vec<u64> =
        moved_outside(&e, &before, holder).into_iter().filter(|id| !matches!(e.doc.get(NodeId(*id)).map(|n| &n.kind), Some(NodeKind::Connector(_)))).collect();
    assert_eq!(outside, Vec::<u64>::new());
    assert_eq!(ty(&e, tail), 200.0, "the tail keeps the place the document stored");
}

#[test]
fn a_batch_apply_spec_inside_a_hug_frame_grows_it_and_re_solves_the_frame_above() {
    let Holder { mut e, holder, tail, .. } =
        holder_with(auto_layout(LayoutDirection::Horizontal, Sizing::Hug, Sizing::Hug, LayoutAlign::Start, 0.0), Rect::new(0.0, 0.0, 120.0, 56.0));

    e.call("designos_apply_batch", json!({"ops":[{"op":"apply_spec","parent":holder.0,"spec":{"kind":"rectangle","name":"new","width":20,"height":50}}]}))
        .unwrap();

    let size = e.doc.get(holder).unwrap().size;
    assert_eq!((size.x, size.y), (100.0, 50.0), "the frame hugs the layer that was added");
    assert_eq!(ty(&e, tail), 50.0, "the screen follows the larger frame");
}

/// Binding a fixed frame's padding to a variable with the single-layer tool changes what the frame lays out inside itself and
/// nothing outside it.
#[test]
fn binding_padding_on_a_fixed_size_frame_with_update_node_moves_nothing_outside_it() {
    let Holder { mut e, holder, tail, .. } = fixed_holder();
    e.call(
        "designos_apply_batch",
        json!({ "ops": [{ "op": "set_variables",
            "upsert_collections": [{ "id": "space", "name": "Space", "modes": [{ "id": "m1", "name": "One" }], "default_mode": "m1" }],
            "upsert_variables": [{ "id": "pad", "name": "pad", "collection": "space", "kind": "number", "values": { "m1": 20 } }] }] }),
    )
    .unwrap();
    let before = geometry(&e.doc);

    e.call("designos_update_node", json!({ "node_id": holder.0, "bindings": { "paddingLeft": "pad" } })).unwrap();

    assert_eq!(moved_outside(&e, &before, holder), Vec::<u64>::new());
    assert_eq!(ty(&e, tail), 200.0, "the tail keeps the place the document stored");
}
