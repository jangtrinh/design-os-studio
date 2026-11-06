//! An agent edit re-solves only what layout reads, and leaves every other layer where the document
//! stored it. The fixtures are not a fixed point of the solver (as an imported file is not), so a
//! solve that reaches a layer the edit did not touch shows up as a layer that moved.

mod data;
mod leave;
mod paint;

use crate::engine::Engine;
use designos_doc::{AutoLayout, Document, FrameProps, LayoutAlign, LayoutDirection, Node, NodeId, NodeKind, Rect, Sizing};
use serde_json::json;
use std::collections::BTreeMap;

pub(super) fn auto_layout(direction: LayoutDirection, primary: Sizing, counter: Sizing, align: LayoutAlign, gap: f32) -> NodeKind {
    NodeKind::Frame(FrameProps {
        auto_layout: Some(AutoLayout {
            direction,
            primary_sizing: primary,
            counter_sizing: counter,
            primary_align: align,
            item_spacing: gap,
            ..AutoLayout::default()
        }),
        ..FrameProps::default()
    })
}

pub(super) fn add_frame(e: &mut Engine, parent: NodeId, r: Rect, kind: NodeKind, name: &str) -> NodeId {
    let id = e.doc.alloc_id();
    let mut n = Node::frame(id, r);
    n.kind = kind;
    n.name = name.into();
    e.doc.insert(parent, n).unwrap()
}

pub(super) fn add_rect(e: &mut Engine, parent: NodeId, r: Rect, name: &str) -> NodeId {
    let id = e.doc.alloc_id();
    let mut n = Node::rectangle(id, r);
    n.name = name.into();
    e.doc.insert(parent, n).unwrap()
}

/// `(x, y, width, height)` of every layer under a page, by node id.
pub(super) type Geometry = BTreeMap<u64, [f32; 4]>;

pub(super) fn geometry(doc: &Document) -> Geometry {
    doc.nodes.values().filter(|n| n.parent.is_some()).map(|n| (n.id.0, [n.transform.tx, n.transform.ty, n.size.x, n.size.y])).collect()
}

/// Layers present in both snapshots whose geometry differs.
pub(super) fn moved(before: &Geometry, after: &Geometry) -> Vec<u64> {
    before.iter().filter(|(id, b)| after.get(id).is_some_and(|a| a.iter().zip(b.iter()).any(|(x, y)| (x - y).abs() > 0.01))).map(|(id, _)| *id).collect()
}

/// What a full solve of every top-level frame gives: where an edit that re-solved everything would leave the layers.
pub(super) fn fully_solved(doc: &Document) -> Geometry {
    let mut d = doc.clone();
    for page in d.pages.clone() {
        for top in d.get(page).map(|n| n.children.clone()).unwrap_or_default() {
            d.relayout_subtree(top);
        }
    }
    d.flush_pending_relayout();
    geometry(&d)
}

pub(super) fn solve_everything(e: &mut Engine) {
    for page in e.doc.pages.clone() {
        for top in e.doc.get(page).map(|n| n.children.clone()).unwrap_or_default() {
            e.doc.relayout_subtree(top);
        }
    }
    e.doc.flush_pending_relayout();
}

/// screen (column, hug, gap 20) > [wrap row (100 wide, hug tall, gap 10) > [a, b, c, d: 40x40], tail].
/// The four squares are stored on top of each other and the row at 40 px tall, as an import that did not
/// run the solver leaves them; a solve would wrap them to 90 px, move three squares and push the tail down.
struct WrapScreen {
    e: Engine,
    row: NodeId,
    a: NodeId,
    b: NodeId,
}

fn wrap_screen() -> WrapScreen {
    let mut e = Engine::new();
    let page = e.resolve_default_page().unwrap();
    let screen = add_frame(
        &mut e,
        page,
        Rect::new(0.0, 0.0, 100.0, 90.0),
        auto_layout(LayoutDirection::Vertical, Sizing::Hug, Sizing::Fixed, LayoutAlign::Start, 20.0),
        "screen",
    );
    let mut row_kind = auto_layout(LayoutDirection::Horizontal, Sizing::Fixed, Sizing::Hug, LayoutAlign::Start, 10.0);
    if let Some(al) = row_kind.frame_props_mut().and_then(|f| f.auto_layout.as_mut()) {
        al.wrap = true;
        al.counter_spacing = 10.0;
    }
    let row = add_frame(&mut e, screen, Rect::new(0.0, 0.0, 100.0, 40.0), row_kind, "wrap row");
    let a = add_rect(&mut e, row, Rect::new(0.0, 0.0, 40.0, 40.0), "a");
    let b = add_rect(&mut e, row, Rect::new(0.0, 0.0, 40.0, 40.0), "b");
    add_rect(&mut e, row, Rect::new(0.0, 0.0, 40.0, 40.0), "c");
    add_rect(&mut e, row, Rect::new(0.0, 0.0, 40.0, 40.0), "d");
    add_rect(&mut e, screen, Rect::new(0.0, 60.0, 100.0, 30.0), "tail");
    assert_ne!(fully_solved(&e.doc), geometry(&e.doc), "the fixture must not be a fixed point of the solver");
    WrapScreen { e, row, a, b }
}

#[test]
fn a_fill_edit_on_a_child_of_a_wrap_frame_moves_no_layer() {
    let WrapScreen { mut e, b, .. } = wrap_screen();
    let before = geometry(&e.doc);

    e.call("designos_update_node", json!({ "node_id": b.0, "fill": "#112233" })).unwrap();

    assert_eq!(moved(&before, &geometry(&e.doc)), Vec::<u64>::new());
}

#[test]
fn a_batch_fill_edit_on_a_child_of_a_wrap_frame_moves_no_layer() {
    let WrapScreen { mut e, b, .. } = wrap_screen();
    let before = geometry(&e.doc);

    e.call("designos_apply_batch", json!({ "ops": [{ "op": "update", "node": b.0, "fill": "#445566", "opacity": 0.5, "name": "renamed" }] })).unwrap();

    assert_eq!(moved(&before, &geometry(&e.doc)), Vec::<u64>::new());
}

#[test]
fn a_fill_style_applied_to_a_child_of_a_wrap_frame_moves_no_layer() {
    let WrapScreen { mut e, b, .. } = wrap_screen();
    let before = geometry(&e.doc);

    e.call(
        "designos_apply_batch",
        json!({ "ops": [{ "op": "set_style",
            "upsert": [{ "key": "accent", "name": "Accent", "property": "fill", "fill": "#123456" }],
            "apply": [{ "node": b.0, "style": "accent" }] }] }),
    )
    .unwrap();

    assert!(!e.doc.get(b).unwrap().fills.is_empty(), "the style must have reached the node");
    assert_eq!(moved(&before, &geometry(&e.doc)), Vec::<u64>::new());
}

#[test]
fn connecting_two_children_of_a_wrap_frame_moves_no_layer() {
    let WrapScreen { mut e, row, a, b } = wrap_screen();
    let before = geometry(&e.doc);

    let reply = e.call("designos_apply_batch", json!({ "ops": [{ "op": "connect", "as": "link", "from": a.0, "to": b.0 }] })).unwrap();

    let connector = NodeId(reply["created"][0].as_u64().expect("the connector id"));
    assert_eq!(e.doc.get(connector).unwrap().parent, Some(row), "the connector lives in the wrap frame");
    assert_eq!(moved(&before, &geometry(&e.doc)), Vec::<u64>::new());
}

#[test]
fn a_new_child_width_in_an_auto_layout_frame_re_solves_the_frame() {
    let mut e = Engine::new();
    let page = e.resolve_default_page().unwrap();
    let row = add_frame(
        &mut e,
        page,
        Rect::new(0.0, 0.0, 300.0, 40.0),
        auto_layout(LayoutDirection::Horizontal, Sizing::Fixed, Sizing::Fixed, LayoutAlign::Start, 10.0),
        "row",
    );
    let first = add_rect(&mut e, row, Rect::new(0.0, 0.0, 50.0, 40.0), "first");
    let second = add_rect(&mut e, row, Rect::new(60.0, 0.0, 50.0, 40.0), "second");

    e.call("designos_update_node", json!({ "node_id": first.0, "width": 120 })).unwrap();
    assert_eq!(e.doc.get(second).unwrap().transform.tx, 130.0, "the second child follows the wider first child");

    e.call("designos_apply_batch", json!({ "ops": [{ "op": "update", "node": first.0, "width": 80 }] })).unwrap();
    assert_eq!(e.doc.get(second).unwrap().transform.tx, 90.0, "and follows it back in a batch");
}

#[test]
fn moving_a_flow_child_by_coordinates_puts_it_back_in_the_flow() {
    let mut e = Engine::new();
    let page = e.resolve_default_page().unwrap();
    let column = add_frame(
        &mut e,
        page,
        Rect::new(0.0, 0.0, 100.0, 90.0),
        auto_layout(LayoutDirection::Vertical, Sizing::Fixed, Sizing::Fixed, LayoutAlign::Start, 10.0),
        "column",
    );
    let first = add_rect(&mut e, column, Rect::new(0.0, 0.0, 100.0, 40.0), "first");
    let second = add_rect(&mut e, column, Rect::new(0.0, 50.0, 100.0, 40.0), "second");

    e.call("designos_update_node", json!({ "node_id": second.0, "x": 33, "y": 70 })).unwrap();

    let at = e.doc.get(second).unwrap().transform;
    assert_eq!((at.tx, at.ty), (0.0, 50.0), "a flow child sits where the solver puts it");
    assert_eq!(e.doc.get(first).unwrap().transform.ty, 0.0);
}

#[test]
fn applying_a_spec_into_an_auto_layout_parent_leaves_a_sibling_subtree_where_it_was() {
    let WrapScreen { mut e, row, .. } = wrap_screen();
    let screen = e.doc.get(row).unwrap().parent.unwrap();
    let before = geometry(&e.doc);

    let reply =
        e.call("designos_apply_spec", json!({ "parent_id": screen.0, "spec": { "kind": "frame", "name": "Hero", "width": 60, "height": 20 } })).unwrap();

    let hero = NodeId(reply["root_node_id"].as_u64().unwrap());
    assert_eq!(e.doc.get(hero).unwrap().transform.ty, 110.0, "the new child follows the row's stored 40 px, a gap, the 30 px tail and a gap");
    let after = geometry(&e.doc);
    let mut sibling_subtree = vec![row];
    sibling_subtree.extend(e.doc.get(row).unwrap().children.iter().copied());
    let moved_in_sibling: Vec<u64> = moved(&before, &after).into_iter().filter(|id| sibling_subtree.iter().any(|n| n.0 == *id)).collect();
    assert_eq!(moved_in_sibling, Vec::<u64>::new(), "the wrap row and its squares keep the geometry the document stored");
}

/// screen (row, fixed) > [side (column, fixed), chat (row, fixed, end-aligned, 32 px side padding) > [bubble]].
/// The bubble is stored at (80, 12); a solve of `chat` would put it at (256, 0).
struct ChatScreen {
    e: Engine,
    side: NodeId,
    bubble: NodeId,
}

fn chat_screen() -> ChatScreen {
    let mut e = Engine::new();
    let page = e.resolve_default_page().unwrap();
    let screen = add_frame(
        &mut e,
        page,
        Rect::new(0.0, 0.0, 800.0, 600.0),
        auto_layout(LayoutDirection::Horizontal, Sizing::Fixed, Sizing::Fixed, LayoutAlign::Start, 0.0),
        "screen",
    );
    let side = add_frame(
        &mut e,
        screen,
        Rect::new(0.0, 0.0, 240.0, 600.0),
        auto_layout(LayoutDirection::Vertical, Sizing::Fixed, Sizing::Fixed, LayoutAlign::Start, 0.0),
        "side",
    );
    let mut chat_kind = auto_layout(LayoutDirection::Horizontal, Sizing::Fixed, Sizing::Fixed, LayoutAlign::End, 0.0);
    if let Some(al) = chat_kind.frame_props_mut().and_then(|f| f.auto_layout.as_mut()) {
        al.padding = [0.0, 32.0, 0.0, 32.0];
    }
    let chat = add_frame(&mut e, screen, Rect::new(240.0, 0.0, 560.0, 600.0), chat_kind, "chat");
    let bubble = add_rect(&mut e, chat, Rect::new(80.0, 12.0, 272.0, 44.0), "bubble");
    assert_eq!(fully_solved(&e.doc)[&bubble.0][..2], [256.0, 0.0], "the fixture must not be a fixed point of the solver");
    ChatScreen { e, side, bubble }
}

fn bubble_at(e: &Engine, bubble: NodeId) -> (f32, f32) {
    let at = e.doc.get(bubble).unwrap().transform;
    (at.tx, at.ty)
}

#[test]
fn a_width_edit_re_solves_its_own_row_and_leaves_the_children_of_a_sibling_frame_alone() {
    let ChatScreen { mut e, side, bubble } = chat_screen();

    e.call("designos_update_node", json!({ "node_id": side.0, "width": 200 })).unwrap();

    assert_eq!(e.doc.get(side).unwrap().size.x, 200.0);
    assert_eq!(bubble_at(&e, bubble), (80.0, 12.0));
}

#[test]
fn a_batch_width_edit_re_solves_its_own_row_and_leaves_the_children_of_a_sibling_frame_alone() {
    let ChatScreen { mut e, side, bubble } = chat_screen();

    e.call("designos_apply_batch", json!({ "ops": [{ "op": "update", "node": side.0, "width": 200 }] })).unwrap();

    assert_eq!(e.doc.get(side).unwrap().size.x, 200.0);
    assert_eq!(bubble_at(&e, bubble), (80.0, 12.0));
}

/// Puts every layer in `stored` back to the geometry it had. A step that solves the frames around it while
/// it runs (the first `connect` of a test, on code that re-solves) leaves the fixture as stored again for
/// the edit under test.
pub(super) fn restore_geometry(e: &mut Engine, stored: &Geometry) {
    for (id, [x, y, w, h]) in stored {
        if let Some(n) = e.doc.get_mut(NodeId(*id)) {
            n.transform.tx = *x;
            n.transform.ty = *y;
            n.size.x = *w;
            n.size.y = *h;
        }
    }
}

#[test]
fn re_attaching_a_connector_inside_a_wrap_frame_moves_no_layer() {
    let WrapScreen { mut e, row, a, b } = wrap_screen();
    let c = e.doc.get(row).unwrap().children[2];
    let stored = geometry(&e.doc);
    let reply = e.call("designos_apply_batch", json!({ "ops": [{ "op": "connect", "from": a.0, "to": b.0 }] })).unwrap();
    let connector = reply["created"][0].as_u64().unwrap();
    restore_geometry(&mut e, &stored);
    let before = geometry(&e.doc);

    e.call("designos_apply_batch", json!({ "ops": [{ "op": "connect", "node": connector, "to": c.0, "label": "onward" }] })).unwrap();

    assert_eq!(moved(&before, &geometry(&e.doc)).into_iter().filter(|id| *id != connector).collect::<Vec<_>>(), Vec::<u64>::new());
}

#[test]
fn moving_a_connector_to_the_screen_that_holds_its_frame_moves_no_layer() {
    let WrapScreen { mut e, row, a, b } = wrap_screen();
    let screen = e.doc.get(row).unwrap().parent.unwrap();
    let tail = *e.doc.get(screen).unwrap().children.last().unwrap();
    let stored = geometry(&e.doc);
    let reply = e.call("designos_apply_batch", json!({ "ops": [{ "op": "connect", "from": a.0, "to": b.0 }] })).unwrap();
    let connector = NodeId(reply["created"][0].as_u64().unwrap());
    assert_eq!(e.doc.get(connector).unwrap().parent, Some(row));
    restore_geometry(&mut e, &stored);
    let before = geometry(&e.doc);

    e.call("designos_apply_batch", json!({ "ops": [{ "op": "connect", "node": connector.0, "to": tail.0 }] })).unwrap();

    assert_eq!(e.doc.get(connector).unwrap().parent, Some(screen), "the connector now sits in the frame both ends share");
    assert_eq!(moved(&before, &geometry(&e.doc)).into_iter().filter(|id| *id != connector.0).collect::<Vec<_>>(), Vec::<u64>::new());
}
