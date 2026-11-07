//! A spec applied into an auto-layout parent re-solves the new subtree and the parent's chain, and
//! nothing else: frames beside it keep the sizes and places the document stored for them.

use crate::{apply_spec_command_with, AutoLayout, Command, Document, LayoutDirection, Node, NodeId, NodeKind, NodeSpec, Rect, Sizing};
use serde_json::json;
use std::collections::BTreeMap;

fn auto_layout_kind(direction: LayoutDirection, primary: Sizing, counter: Sizing, gap: f32) -> NodeKind {
    let mut kind = NodeKind::Frame(Default::default());
    if let Some(f) = kind.frame_props_mut() {
        f.auto_layout = Some(AutoLayout { direction, primary_sizing: primary, counter_sizing: counter, item_spacing: gap, ..AutoLayout::default() });
    }
    kind
}

fn add_frame(doc: &mut Document, parent: NodeId, r: Rect, kind: NodeKind, name: &str) -> NodeId {
    let id = doc.alloc_id();
    let mut n = Node::frame(id, r);
    n.kind = kind;
    n.name = name.into();
    doc.insert(parent, n).unwrap()
}

fn add_rect(doc: &mut Document, parent: NodeId, r: Rect, name: &str) -> NodeId {
    let id = doc.alloc_id();
    let mut n = Node::rectangle(id, r);
    n.name = name.into();
    doc.insert(parent, n).unwrap()
}

fn new_child_spec() -> NodeSpec {
    serde_json::from_value(json!({ "kind": "frame", "name": "New", "width": 50, "height": 30 })).expect("spec")
}

/// `(x, y, width, height)` of every layer under a page.
fn geometry(doc: &Document) -> BTreeMap<u64, [f32; 4]> {
    doc.nodes.values().filter(|n| n.parent.is_some()).map(|n| (n.id.0, [n.transform.tx, n.transform.ty, n.size.x, n.size.y])).collect()
}

/// page > column (vertical, gap 10) > [pill (hug row stored 10x10, holds a 100x20 bar)].
/// The pill is not what a solve would give: stored 10x10, solved 100x20.
fn column_with_stale_pill() -> (Document, NodeId, NodeId) {
    let mut doc = Document::new("stale-pill");
    let page = doc.add_page("Page");
    let column =
        add_frame(&mut doc, page, Rect::new(0.0, 0.0, 100.0, 10.0), auto_layout_kind(LayoutDirection::Vertical, Sizing::Hug, Sizing::Fixed, 10.0), "column");
    let pill =
        add_frame(&mut doc, column, Rect::new(0.0, 0.0, 10.0, 10.0), auto_layout_kind(LayoutDirection::Horizontal, Sizing::Hug, Sizing::Hug, 0.0), "pill");
    add_rect(&mut doc, pill, Rect::new(0.0, 0.0, 100.0, 20.0), "bar");
    (doc, column, pill)
}

#[test]
fn a_spec_applied_into_an_auto_layout_parent_leaves_the_stored_size_of_a_sibling_frame() {
    let (mut doc, column, pill) = column_with_stale_pill();

    let root = doc.apply_spec(column, &new_child_spec()).unwrap();

    let pill_node = doc.get(pill).unwrap();
    assert_eq!((pill_node.size.x, pill_node.size.y), (10.0, 10.0), "the sibling is not part of the edit, so it keeps the size the document stored");
    assert_eq!(doc.get(root).unwrap().transform.ty, 20.0, "the new child follows the sibling's stored 10 px plus the 10 px gap");
}

#[test]
fn a_spec_command_places_the_new_root_after_the_stored_size_of_a_sibling_frame() {
    let (mut doc, column, _pill) = column_with_stale_pill();

    let (cmd, _report) = apply_spec_command_with(&mut doc, column, &new_child_spec(), &[], None).unwrap();

    let Command::Insert { nodes, .. } = cmd else { panic!("a plain spec is one insert") };
    assert_eq!(nodes[0].transform.ty, 20.0, "the new child follows the sibling's stored 10 px plus the 10 px gap");
}

#[test]
fn a_spec_command_leaves_every_existing_layer_where_it_was_including_the_auto_layout_ancestors() {
    let mut doc = Document::new("hug-chain");
    let page = doc.add_page("Page");
    let screen =
        add_frame(&mut doc, page, Rect::new(0.0, 0.0, 100.0, 80.0), auto_layout_kind(LayoutDirection::Vertical, Sizing::Hug, Sizing::Fixed, 10.0), "screen");
    let column =
        add_frame(&mut doc, screen, Rect::new(0.0, 0.0, 100.0, 20.0), auto_layout_kind(LayoutDirection::Vertical, Sizing::Hug, Sizing::Fixed, 10.0), "column");
    add_rect(&mut doc, column, Rect::new(0.0, 0.0, 100.0, 20.0), "row");
    add_rect(&mut doc, screen, Rect::new(0.0, 30.0, 100.0, 50.0), "tail");
    let before = geometry(&doc);

    apply_spec_command_with(&mut doc, column, &new_child_spec(), &[], None).unwrap();

    assert_eq!(geometry(&doc), before, "the call only builds the command: the column grew and moved the tail while it ran, and the document gets that back");
}
