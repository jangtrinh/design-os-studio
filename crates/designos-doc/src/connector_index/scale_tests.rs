//! Scale and ordering tests for the connector index: building it cold, refreshing it after a
//! big edit, and the sorted small-vector it stores its lists in.

use super::*;
use std::collections::BTreeSet;
use std::time::{Duration, Instant};

/// Attach targets the generated connectors are spread over.
const TARGETS: usize = 400;

/// Upper bound for one cold index build over [`REBUILD_CONNECTORS`] connectors under one parent.
///
/// Measured in a debug build: the build is linear and takes a few tens of milliseconds, while
/// a build that does a linear duplicate scan per insert needs several seconds. The bound sits
/// well clear of both, so it flags the complexity class, not a tight time budget.
const REBUILD_BOUND: Duration = Duration::from_millis(300);
const REBUILD_CONNECTORS: usize = 20_000;

/// Upper bound for refreshing the index after one insert into a frame that already holds
/// [`REFRESH_CONNECTORS`] connectors (same reasoning as [`REBUILD_BOUND`]).
const REFRESH_BOUND: Duration = Duration::from_millis(300);
const REFRESH_CONNECTORS: usize = 4_000;

/// Connectors edited in place between two index refreshes (a drag rewrites the geometry of every
/// rerouted connector), kept below the dirty log's capacity so the refresh stays incremental.
const EDITED_CONNECTORS: usize = 400;

/// Connectors inside the frame moved by the internal-connector checks.
const INTERNAL_CONNECTORS: usize = 1_000;

struct Fixture {
    doc: Document,
    /// Container that holds every connector.
    parent: NodeId,
    /// `(connector, start node, end node)` in creation (ascending id) order.
    connectors: Vec<(NodeId, Option<NodeId>, Option<NodeId>)>,
}

fn add_rect(doc: &mut Document, parent: NodeId, i: usize) -> NodeId {
    let id = doc.alloc_id();
    doc.insert(parent, Node::rectangle(id, Rect::new((i % 40) as f32 * 100.0, (i / 40) as f32 * 100.0, 80.0, 60.0))).unwrap()
}

/// Adds `count` connectors under `parent`, spread over `targets`, every one attached at both
/// ends. Every 11th connector starts and ends on the same node. With `free_ends`, every 13th
/// connector also has a free start and every 17th a free end.
fn add_connectors(doc: &mut Document, parent: NodeId, targets: &[NodeId], count: usize, free_ends: bool) -> Vec<(NodeId, Option<NodeId>, Option<NodeId>)> {
    let mut out = Vec::with_capacity(count);
    for i in 0..count {
        let a = targets[i % targets.len()];
        let b = if i % 11 == 0 { a } else { targets[(i * 7 + 3) % targets.len()] };
        let start = (!free_ends || i % 13 != 5).then_some(a);
        let end = (!free_ends || i % 17 != 9).then_some(b);
        let props = ConnectorProps {
            start: start.map_or_else(|| ConnectorEnd::free(Vec2::new(i as f32, 0.0)), |n| ConnectorEnd::attached(n, Magnet::Auto)),
            end: end.map_or_else(|| ConnectorEnd::free(Vec2::new(0.0, i as f32)), |n| ConnectorEnd::attached(n, Magnet::Auto)),
            ..Default::default()
        };
        let id = doc.alloc_id();
        doc.insert(parent, Node::connector(id, props)).unwrap();
        out.push((id, start, end));
    }
    out
}

fn fixture(count: usize) -> Fixture {
    let mut doc = Document::new("connector_index_scale");
    let page = doc.add_page("Page 1");
    let targets: Vec<NodeId> = (0..TARGETS).map(|i| add_rect(&mut doc, page, i)).collect();
    let connectors = add_connectors(&mut doc, page, &targets, count, false);
    Fixture { doc, parent: page, connectors }
}

/// Asserts the index lists every connector of `fx` once per attached end node and once under its parent.
fn assert_index_matches(idx: &ConnectorIndex, fx: &Fixture) {
    assert_eq!(idx.connectors.len(), fx.connectors.len());
    for &(id, start, end) in &fx.connectors {
        assert_eq!(idx.connectors.get(&id), Some(&ConnectorEntry { parent: Some(fx.parent), start, end }));
    }

    let mut want_by_end: BTreeMap<NodeId, BTreeSet<NodeId>> = BTreeMap::new();
    for &(id, start, end) in &fx.connectors {
        for node in [start, end].into_iter().flatten() {
            want_by_end.entry(node).or_default().insert(id);
        }
    }
    assert_eq!(idx.by_end.len(), want_by_end.len());
    for (node, want) in &want_by_end {
        let want: Vec<NodeId> = want.iter().copied().collect();
        assert_eq!(idx.connectors_for_end(*node), want.as_slice(), "connectors attached to {node:?}");
    }

    for &(id, start, end) in &fx.connectors {
        for node in [start, end].into_iter().flatten() {
            assert!(idx.connectors_for_end(node).contains(&id), "connector {id:?} missing from the list of {node:?}");
        }
    }

    let all: Vec<NodeId> = fx.connectors.iter().map(|c| c.0).collect();
    assert_eq!(idx.per_parent.len(), 1);
    assert_eq!(idx.children_connectors(fx.parent).len(), all.len());
    assert_eq!(idx.children_connectors(fx.parent), all.as_slice());

    let same_node = fx.connectors.iter().find(|c| c.1.is_some() && c.1 == c.2).expect("fixture has a connector with one node at both ends");
    let node = same_node.1.unwrap();
    assert_eq!(idx.connectors_for_end(node).iter().filter(|c| **c == same_node.0).count(), 1, "a connector on one node at both ends is listed once");
}

/// Asserts the index equals what a scan of every node of `doc` yields.
fn assert_index_equals_scan(idx: &ConnectorIndex, doc: &Document) {
    let mut by_end: BTreeMap<NodeId, BTreeSet<NodeId>> = BTreeMap::new();
    let mut per_parent: BTreeMap<NodeId, BTreeSet<NodeId>> = BTreeMap::new();
    let mut count = 0;
    for (&id, node) in &doc.nodes {
        let NodeKind::Connector(props) = &node.kind else { continue };
        count += 1;
        assert_eq!(idx.connectors.get(&id), Some(&ConnectorEntry { parent: node.parent, start: props.start.node, end: props.end.node }));
        for end in [props.start.node, props.end.node].into_iter().flatten() {
            by_end.entry(end).or_default().insert(id);
        }
        per_parent.entry(node.parent.unwrap()).or_default().insert(id);
    }
    assert_eq!(idx.connectors.len(), count);
    assert_eq!(idx.by_end.len(), by_end.len());
    assert_eq!(idx.per_parent.len(), per_parent.len());
    for (node, want) in by_end {
        assert_eq!(idx.connectors_for_end(node), want.into_iter().collect::<Vec<_>>().as_slice(), "connectors attached to {node:?}");
    }
    for (parent, want) in per_parent {
        assert_eq!(idx.children_connectors(parent), want.into_iter().collect::<Vec<_>>().as_slice(), "connectors under {parent:?}");
    }
}

#[test]
fn cold_build_with_many_connectors_under_one_parent_is_linear() {
    clear_connector_index();
    let fx = fixture(REBUILD_CONNECTORS);

    let started = Instant::now();
    let indexed = with_index(&fx.doc, |idx| idx.connectors.len());
    let elapsed = started.elapsed();

    eprintln!("cold build of {REBUILD_CONNECTORS} connectors under one parent: {elapsed:?}");
    assert_eq!(indexed, REBUILD_CONNECTORS);
    assert!(elapsed < REBUILD_BOUND, "cold build of {REBUILD_CONNECTORS} connectors under one parent took {elapsed:?} (bound {REBUILD_BOUND:?})");
    with_index(&fx.doc, |idx| assert_index_matches(idx, &fx));
    assert_eq!(connector_index_stats().rebuilds, 1, "checking the index must not rebuild it");
}

#[test]
fn inserting_into_a_frame_with_many_connectors_refreshes_incrementally_and_fast() {
    clear_connector_index();
    let mut doc = Document::new("connector_index_refresh_scale");
    let page = doc.add_page("Page 1");
    let frame = doc.alloc_id();
    doc.insert(page, Node::frame(frame, Rect::new(0.0, 0.0, 4_000.0, 4_000.0))).unwrap();
    let targets: Vec<NodeId> = (0..TARGETS).map(|i| add_rect(&mut doc, frame, i)).collect();
    let connectors = add_connectors(&mut doc, frame, &targets, REFRESH_CONNECTORS, false);
    let mut fx = Fixture { doc, parent: frame, connectors };

    with_index(&fx.doc, |idx| assert_eq!(idx.connectors.len(), REFRESH_CONNECTORS));
    reset_connector_index_stats();

    add_rect(&mut fx.doc, frame, TARGETS);

    let started = Instant::now();
    let indexed = with_index(&fx.doc, |idx| idx.connectors.len());
    let elapsed = started.elapsed();

    eprintln!("refresh after one insert into a frame of {REFRESH_CONNECTORS} connectors: {elapsed:?}");
    assert_eq!(indexed, REFRESH_CONNECTORS);
    let stats = connector_index_stats();
    assert_eq!((stats.rebuilds, stats.refreshes), (0, 1), "one insert must refresh the index, not rebuild it");
    assert!(elapsed < REFRESH_BOUND, "refresh after one insert into a frame of {REFRESH_CONNECTORS} connectors took {elapsed:?} (bound {REFRESH_BOUND:?})");
    with_index(&fx.doc, |idx| assert_index_matches(idx, &fx));
}

#[test]
fn refresh_after_editing_connector_geometry_in_a_big_parent_stays_incremental_and_fast() {
    clear_connector_index();
    let mut fx = fixture(REBUILD_CONNECTORS);
    with_index(&fx.doc, |idx| assert_eq!(idx.connectors.len(), REBUILD_CONNECTORS));
    reset_connector_index_stats();

    for &(id, ..) in fx.connectors.iter().step_by(REBUILD_CONNECTORS / EDITED_CONNECTORS) {
        fx.doc.get_mut(id).unwrap().transform.tx += 1.0;
    }

    let started = Instant::now();
    let indexed = with_index(&fx.doc, |idx| idx.connectors.len());
    let elapsed = started.elapsed();

    eprintln!("refresh after editing {EDITED_CONNECTORS} of {REBUILD_CONNECTORS} connectors under one parent: {elapsed:?}");
    assert_eq!(indexed, REBUILD_CONNECTORS);
    let stats = connector_index_stats();
    assert_eq!((stats.rebuilds, stats.refreshes), (0, 1), "a geometry edit must refresh the index, not rebuild it");
    assert!(elapsed < REFRESH_BOUND, "refresh after editing {EDITED_CONNECTORS} connectors under one parent took {elapsed:?} (bound {REFRESH_BOUND:?})");
    with_index(&fx.doc, |idx| assert_index_matches(idx, &fx));
}

#[test]
fn moving_a_frame_leaves_connectors_inside_it_unexamined() {
    clear_connector_index();
    let mut doc = Document::new("connector_index_internal_connectors");
    let page = doc.add_page("Page 1");
    let frame = doc.alloc_id();
    doc.insert(page, Node::frame(frame, Rect::new(0.0, 0.0, 2_000.0, 2_000.0))).unwrap();
    let inner = [add_rect(&mut doc, frame, 0), add_rect(&mut doc, frame, 1)];
    add_connectors(&mut doc, frame, &inner, INTERNAL_CONNECTORS, false);
    let outside = add_rect(&mut doc, page, 2);
    let crossing = doc.alloc_id();
    let props =
        ConnectorProps { start: ConnectorEnd::attached(inner[0], Magnet::Auto), end: ConnectorEnd::attached(outside, Magnet::Auto), ..Default::default() };
    doc.insert(page, Node::connector(crossing, props)).unwrap();
    with_index(&doc, |idx| assert_eq!(idx.connectors.len(), INTERNAL_CONNECTORS + 1));

    // Moving a node that is one end of many connectors examines every one of them.
    reset_connector_index_stats();
    plan_reroute(&doc, &[inner[0]], None);
    let attached = connectors_touching(&doc, &[inner[0]]).len();
    assert!(attached > INTERNAL_CONNECTORS / 2, "fixture must attach many connectors to one node, got {attached}");
    assert_eq!(connector_index_stats().candidates, attached as u64);

    // Moving the frame carries all of its inner connectors along; only the one crossing out is examined.
    reset_connector_index_stats();
    doc.get_mut(frame).unwrap().transform.tx += 5.0;
    let plan = plan_reroute(&doc, &[frame], None);
    assert_eq!(connector_index_stats().candidates, 1, "only the connector leaving the frame needs a look");
    assert!(plan.updates.iter().all(|(id, _)| *id == crossing));
}

#[test]
fn refresh_after_rewiring_and_geometry_edits_equals_a_full_scan() {
    clear_connector_index();
    let mut doc = Document::new("connector_index_rewire");
    let page = doc.add_page("Page 1");
    let nodes: Vec<NodeId> = (0..8).map(|i| add_rect(&mut doc, page, i)).collect();
    let connectors = add_connectors(&mut doc, page, &nodes, 40, true);
    with_index(&doc, |idx| assert_index_equals_scan(idx, &doc));
    reset_connector_index_stats();

    // One connector moves both ends onto the same node, another only changes geometry, a
    // third loses its node, and new connectors arrive after the index was built.
    let (rewired, ..) = connectors[3];
    if let NodeKind::Connector(props) = &mut doc.get_mut(rewired).unwrap().kind {
        props.start = ConnectorEnd::attached(nodes[7], Magnet::Auto);
        props.end = ConnectorEnd::attached(nodes[7], Magnet::Auto);
    }
    doc.get_mut(connectors[10].0).unwrap().transform.tx += 1.0;
    if let NodeKind::Connector(props) = &mut doc.get_mut(connectors[20].0).unwrap().kind {
        props.start = ConnectorEnd::free(Vec2::new(5.0, 5.0));
    }
    let late = add_connectors(&mut doc, page, &nodes, 3, true);

    with_index(&doc, |idx| {
        let stats = connector_index_stats();
        assert_eq!((stats.rebuilds, stats.refreshes), (0, 1));
        assert_index_equals_scan(idx, &doc);
        assert!(late.iter().all(|(id, ..)| idx.contains_connector(*id)));
        assert_eq!(idx.connectors_for_end(nodes[7]).iter().filter(|c| **c == rewired).count(), 1);
    });

    doc.remove(rewired).unwrap();
    with_index(&doc, |idx| {
        assert_index_equals_scan(idx, &doc);
        assert!(!idx.contains_connector(rewired));
    });
}

#[test]
fn small_vec_matches_a_sorted_set_across_the_inline_boundary() {
    // Deterministic pseudo-random walk over a small value range so the list repeatedly crosses
    // the 4-item inline limit in both directions and sees duplicates and missing removals.
    let mut state = 0x2545_F491_4F6C_DD1Du64;
    let mut next = move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    for range in [3u64, 6, 12, 40] {
        let mut list: SmallVec<NodeId> = SmallVec::default();
        let mut model: BTreeSet<NodeId> = BTreeSet::new();
        for step in 0..4_000 {
            let value = NodeId(next() % range);
            if next() % 5 < 3 {
                list.push_unique(value);
                model.insert(value);
            } else {
                list.remove_value(value);
                model.remove(&value);
            }
            let want: Vec<NodeId> = model.iter().copied().collect();
            assert_eq!(list.as_slice(), want.as_slice(), "range {range}, step {step}");
            assert_eq!(list.len(), want.len());
            assert_eq!(list.is_empty(), want.is_empty());
        }
    }
}

#[test]
fn small_vec_appends_in_ascending_order_without_reordering() {
    let mut list: SmallVec<NodeId> = SmallVec::default();
    for id in 1..=9u64 {
        list.push_unique(NodeId(id));
        list.push_unique(NodeId(id));
    }
    assert_eq!(list.as_slice(), (1..=9).map(NodeId).collect::<Vec<_>>().as_slice());
    list.push_unique(NodeId(0));
    list.push_unique(NodeId(5));
    assert_eq!(list.as_slice(), (0..=9).map(NodeId).collect::<Vec<_>>().as_slice());
}
