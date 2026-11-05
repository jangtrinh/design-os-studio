//! Regression tests: a document deserialised from external bytes always satisfies the tree invariants.

use crate::{Document, Node, NodeId, Rect};
use std::collections::BTreeSet;

/// Builds page(1) -> frame(2) -> rect(3), returning the doc and ids.
fn sample() -> (Document, NodeId, NodeId, NodeId) {
    let mut d = Document::new("Repair");
    let page = d.add_page("P");
    let frame = d.alloc_id();
    d.insert(page, Node::frame(frame, Rect::new(0.0, 0.0, 100.0, 100.0))).unwrap();
    let rect = d.alloc_id();
    d.insert(frame, Node::rectangle(rect, Rect::new(1.0, 2.0, 3.0, 4.0))).unwrap();
    (d, page, frame, rect)
}

/// Asserts: every id in pages/children exists, parent <-> children agree, every parent chain
/// ends within `nodes.len()` hops, every child is claimed once, and `alloc_id` is fresh.
fn assert_consistent(d: &mut Document) {
    let mut claimed = BTreeSet::new();
    for (id, n) in &d.nodes {
        assert_eq!(n.id, *id, "node id matches its key");
        for c in &n.children {
            let child = d.nodes.get(c).unwrap_or_else(|| panic!("dangling child {c:?} under {id:?}"));
            assert_eq!(child.parent, Some(*id), "child {c:?} parent points back to {id:?}");
            assert!(claimed.insert(*c), "child {c:?} listed twice");
            assert_ne!(c, id, "self child");
        }
        if let Some(p) = n.parent {
            let parent = d.nodes.get(&p).unwrap_or_else(|| panic!("dangling parent {p:?} of {id:?}"));
            assert!(parent.children.contains(id), "parent {p:?} lists {id:?}");
        }
        let mut hops = 0;
        let mut cur = n.parent;
        while let Some(p) = cur {
            hops += 1;
            assert!(hops <= d.nodes.len(), "parent chain of {id:?} never ends");
            cur = d.nodes[&p].parent;
        }
    }
    for p in &d.pages {
        assert!(d.nodes.contains_key(p), "dangling page {p:?}");
    }
    let max = d.nodes.keys().next_back().map_or(0, |i| i.0);
    let fresh = d.alloc_id();
    assert!(fresh.0 > max, "alloc_id {fresh:?} must exceed the largest node id {max}");
}

fn reload(d: &Document) -> Document {
    Document::from_json(&d.to_json().unwrap()).unwrap()
}

#[test]
fn cyclic_parent_and_children_load_consistently() {
    let (mut d, _page, frame, rect) = sample();
    d.nodes.get_mut(&rect).unwrap().children.push(frame);
    d.nodes.get_mut(&frame).unwrap().parent = Some(rect);
    let mut back = reload(&d);
    assert_consistent(&mut back);
    let _ = back.world_transform(rect);
    let _ = back.page_of(rect);
    assert_eq!(back.nodes.len(), 3, "no node is deleted");
}

#[test]
fn detached_cycle_without_a_root_loads_consistently() {
    let (mut d, _page, _frame, _rect) = sample();
    let (a, b) = (NodeId(50), NodeId(51));
    let mut na = Node::frame(a, Rect::new(0.0, 0.0, 1.0, 1.0));
    let mut nb = Node::frame(b, Rect::new(0.0, 0.0, 1.0, 1.0));
    na.parent = Some(b);
    na.children.push(b);
    nb.parent = Some(a);
    nb.children.push(a);
    d.nodes.insert(a, na);
    d.nodes.insert(b, nb);
    let mut back = reload(&d);
    assert_consistent(&mut back);
    let _ = back.world_transform(a);
    assert_eq!(back.nodes.len(), 5, "no node is deleted");
}

#[test]
fn self_child_is_removed() {
    let (mut d, _page, frame, _rect) = sample();
    d.nodes.get_mut(&frame).unwrap().children.push(frame);
    let mut back = reload(&d);
    assert_consistent(&mut back);
    assert!(!back.nodes[&frame].children.contains(&frame));
}

#[test]
fn stale_next_id_is_raised_above_every_node() {
    let (d, ..) = sample();
    let mut v: serde_json::Value = serde_json::from_str(&d.to_json().unwrap()).unwrap();
    v["next_id"] = serde_json::json!(0);
    let mut back = Document::from_json(&v.to_string()).unwrap();
    assert_consistent(&mut back);
}

#[test]
fn dangling_child_and_page_ids_are_dropped() {
    let (mut d, page, frame, _rect) = sample();
    d.nodes.get_mut(&frame).unwrap().children.push(NodeId(9999));
    d.pages.push(NodeId(8888));
    d.nodes.get_mut(&page).unwrap().parent = Some(NodeId(7777));
    let mut back = reload(&d);
    assert_consistent(&mut back);
    assert_eq!(back.pages, vec![page]);
}

#[test]
fn child_listed_by_two_parents_and_twice_keeps_one_claim() {
    let (mut d, page, frame, rect) = sample();
    d.nodes.get_mut(&page).unwrap().children.push(rect);
    d.nodes.get_mut(&frame).unwrap().children.push(rect);
    let mut back = reload(&d);
    assert_consistent(&mut back);
    let claims = back.nodes.values().filter(|n| n.children.contains(&rect)).count();
    assert_eq!(claims, 1);
}

#[test]
fn stale_parent_not_listing_the_child_is_rebuilt_from_children() {
    let (mut d, _page, frame, rect) = sample();
    d.nodes.get_mut(&frame).unwrap().children.clear();
    let mut back = reload(&d);
    assert_consistent(&mut back);
    assert_eq!(back.nodes[&rect].parent, None);
}

#[test]
fn absurd_depth_is_capped_without_stack_overflow() {
    let mut d = Document::new("Deep");
    let page = d.add_page("P");
    let mut parent = page;
    for _ in 0..1500 {
        let id = d.alloc_id();
        d.insert(parent, Node::frame(id, Rect::new(0.0, 0.0, 1.0, 1.0))).unwrap();
        parent = id;
    }
    let mut back = reload(&d);
    assert_consistent(&mut back);
    assert_eq!(back.nodes.len(), 1501, "deep nodes are detached, not deleted");
    let mut depth = 0;
    back.walk(page, |_, dpt| depth = depth.max(dpt));
    assert!(depth < 1024, "walk depth {depth} must be bounded");
}

#[test]
fn valid_document_is_untouched() {
    let (d, ..) = sample();
    let before = d.to_json().unwrap();
    let back = reload(&d);
    assert_eq!(back.to_json().unwrap(), before);
}

#[test]
fn page_listed_under_another_pages_node_stays_a_root() {
    let mut d = Document::new("Pages");
    let p1 = d.add_page("P1");
    let p2 = d.add_page("P2");
    let x = d.alloc_id();
    d.insert(p1, Node::frame(x, Rect::new(0.0, 0.0, 10.0, 10.0))).unwrap();
    let y = d.alloc_id();
    d.insert(p2, Node::rectangle(y, Rect::new(0.0, 0.0, 10.0, 10.0))).unwrap();
    d.nodes.get_mut(&x).unwrap().children.push(p2);
    let (mut back, report) = Document::from_json_with_report(&d.to_json().unwrap()).unwrap();
    assert_consistent(&mut back);
    assert_eq!(back.nodes[&p2].parent, None, "a page never gains a parent");
    assert!(!back.nodes[&x].children.contains(&p2));
    assert_eq!(back.pages, vec![p1, p2]);
    assert_eq!(back.page_of(y), Some(p2), "the page keeps its own content");
    assert_eq!(report.duplicate_claims, 1);
}

#[test]
fn library_page_listed_under_a_page_node_stays_a_root() {
    let (mut d, _page, frame, _rect) = sample();
    let lib = d.alloc_id();
    d.nodes.insert(lib, Node::new(lib, crate::NodeKind::Page { background: crate::Color::rgb(0, 0, 0) }));
    d.library_page = Some(lib);
    d.nodes.get_mut(&frame).unwrap().children.push(lib);
    let mut back = reload(&d);
    assert_consistent(&mut back);
    assert_eq!(back.nodes[&lib].parent, None);
    assert!(!back.nodes[&frame].children.contains(&lib));
}

#[test]
fn max_node_id_loads_without_overflow_and_is_flagged() {
    let mut d = Document::new("Max");
    let page = d.add_page("P");
    d.insert(page, Node::rectangle(NodeId(u64::MAX), Rect::new(0.0, 0.0, 1.0, 1.0))).unwrap();
    let mut v = serde_json::to_value(&d).unwrap();
    v["next_id"] = serde_json::json!(5);
    let (mut back, report) = Document::from_json_with_report(&v.to_string()).unwrap();
    assert!(report.id_space_exhausted);
    assert!(report.structural_summary().unwrap().contains("too close to the maximum"));
    let fresh = back.alloc_id();
    assert!(!back.nodes.contains_key(&fresh), "alloc_id must not collide with {fresh:?}");
}

#[test]
fn alloc_id_never_wraps_when_the_counter_is_exhausted() {
    let (mut d, page, frame, rect) = sample();
    d.next_id = u64::MAX;
    let fresh = d.alloc_id();
    assert!(fresh.0 > 0 && ![page, frame, rect].contains(&fresh));
    assert_eq!(fresh, NodeId(4), "the lowest unused id is handed out");
    d.insert(page, Node::rectangle(fresh, Rect::new(0.0, 0.0, 1.0, 1.0))).unwrap();
    let next = d.alloc_id();
    assert_eq!(next, NodeId(5));
    assert!(!d.nodes.contains_key(&next));
}

#[test]
fn headroom_below_the_limit_is_not_flagged() {
    let (d, ..) = sample();
    let (_, report) = Document::from_json_with_report(&d.to_json().unwrap()).unwrap();
    assert!(!report.id_space_exhausted && report.is_clean());
}

/// A frame with an auto-layout `flow` that holds two rectangles and `connectors` connectors joining
/// them, each stored the way an older release wrote it: part of the flow, not absolute.
fn auto_layout_frame_with_flowing_connectors(connectors: usize) -> (Document, Vec<NodeId>) {
    let mut d = Document::new("Flowing connectors");
    let page = d.add_page("P");
    let frame = d.alloc_id();
    let mut f = Node::frame(frame, Rect::new(0.0, 0.0, 300.0, 120.0));
    if let Some(fp) = f.kind.frame_props_mut() {
        fp.auto_layout = Some(crate::AutoLayout::default());
    }
    d.insert(page, f).unwrap();
    let a = d.alloc_id();
    d.insert(frame, Node::rectangle(a, Rect::new(10.0, 10.0, 60.0, 40.0))).unwrap();
    let b = d.alloc_id();
    d.insert(frame, Node::rectangle(b, Rect::new(150.0, 10.0, 60.0, 40.0))).unwrap();
    let mut ids = Vec::new();
    for _ in 0..connectors {
        let c = d.alloc_id();
        let props = crate::ConnectorProps {
            start: crate::ConnectorEnd::attached(a, crate::Magnet::Auto),
            end: crate::ConnectorEnd::attached(b, crate::Magnet::Auto),
            ..Default::default()
        };
        let mut conn = Node::connector(c, props);
        conn.layout_child.absolute = false;
        d.insert(frame, conn).unwrap();
        ids.push(c);
    }
    (d, ids)
}

#[test]
fn a_connector_flowing_in_an_auto_layout_parent_is_repaired_on_load_and_reported() {
    let (d, ids) = auto_layout_frame_with_flowing_connectors(1);
    let (mut back, report) = Document::from_json_with_report(&d.to_json().unwrap()).unwrap();
    assert!(back.nodes[&ids[0]].layout_child.absolute, "the connector is taken out of the flow");
    assert!(!report.is_clean(), "the repair shows in the report: {report:?}");
    assert_eq!(report.structural_summary(), None, "nothing was guessed or lost, so a caller that refuses repaired graphs still opens the file");
    assert!(back.validate_and_repair().is_clean(), "a second pass over the repaired document finds nothing");
    let (_, resaved) = Document::from_json_with_report(&back.to_json().unwrap()).unwrap();
    assert!(resaved.is_clean(), "what a save writes needs no repair");
}

#[test]
fn load_repair_counts_each_flowing_connector_once() {
    let (d, ids) = auto_layout_frame_with_flowing_connectors(3);
    let (mut back, report) = Document::from_json_with_report(&d.to_json().unwrap()).unwrap();
    assert_eq!(report.connectors_made_absolute, 3);
    assert!(ids.iter().all(|id| back.nodes[id].layout_child.absolute));
    assert_eq!(back.validate_and_repair().connectors_made_absolute, 0, "a connector already out of the flow is not counted again");
}
