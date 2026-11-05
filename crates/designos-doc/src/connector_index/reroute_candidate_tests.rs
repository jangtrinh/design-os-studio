//! The planner's candidate set against a direct statement of the rule, on generated documents.
//!
//! Rule: a connector is a candidate when it is touched itself or has an end on, or below, a
//! touched node; it is dropped when it is not touched and its parent and both ends sit strictly
//! below the same deepest touched node.

use super::*;
use std::collections::BTreeSet;

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }

    fn pick(&mut self, from: &[NodeId]) -> NodeId {
        from[self.below(from.len())]
    }
}

struct Scene {
    doc: Document,
    page: NodeId,
    frames: Vec<NodeId>,
    shapes: Vec<NodeId>,
    connectors: Vec<NodeId>,
}

/// Frames nested under the page or earlier frames, shapes and connectors under random
/// containers. Connector ends are random shapes, frames or earlier connectors (a tenth are free,
/// a tenth sit on one node at both ends), so connectors cross frame boundaries, sit inside them,
/// and end on frames and on other connectors.
fn random_scene(rng: &mut Rng, frames: usize, shapes: usize, connectors: usize) -> Scene {
    let mut doc = Document::new("reroute_candidates");
    let page = doc.add_page("Page 1");
    let mut containers = vec![page];
    let mut frame_ids = Vec::new();
    for _ in 0..frames {
        let id = doc.alloc_id();
        let parent = rng.pick(&containers);
        doc.insert(parent, Node::frame(id, Rect::new(0.0, 0.0, 500.0, 500.0))).unwrap();
        containers.push(id);
        frame_ids.push(id);
    }
    let mut shape_ids = Vec::new();
    for i in 0..shapes {
        let id = doc.alloc_id();
        let parent = rng.pick(&containers);
        doc.insert(parent, Node::rectangle(id, Rect::new(i as f32 * 10.0, 0.0, 8.0, 8.0))).unwrap();
        shape_ids.push(id);
    }
    let ends: Vec<NodeId> = shape_ids.iter().chain(&frame_ids).copied().collect();
    let mut connector_ids = Vec::new();
    for _ in 0..connectors {
        let id = doc.alloc_id();
        let parent = rng.pick(&containers);
        let target = |rng: &mut Rng| if !connector_ids.is_empty() && rng.below(5) == 0 { rng.pick(&connector_ids) } else { rng.pick(&ends) };
        let a = target(rng);
        let b = if rng.below(10) == 0 { a } else { target(rng) };
        let end = |rng: &mut Rng, node: NodeId| if rng.below(10) == 0 { ConnectorEnd::free(Vec2::ZERO) } else { ConnectorEnd::attached(node, Magnet::Auto) };
        let props = ConnectorProps { start: end(rng, a), end: end(rng, b), ..Default::default() };
        doc.insert(parent, Node::connector(id, props)).unwrap();
        connector_ids.push(id);
    }
    Scene { doc, page, frames: frame_ids, shapes: shape_ids, connectors: connector_ids }
}

/// Deepest touched node at or above `id`, climbing parent pointers only.
fn deepest(doc: &Document, touched: &BTreeSet<NodeId>, id: NodeId) -> Option<NodeId> {
    let mut cur = Some(id);
    while let Some(node) = cur {
        if touched.contains(&node) {
            return Some(node);
        }
        cur = doc.get(node).and_then(|n| n.parent);
    }
    None
}

/// Connectors with an end on, or below, a touched node, stated with parent pointers only.
fn expected_touching(doc: &Document, touched: &[NodeId]) -> Vec<NodeId> {
    let touched: BTreeSet<NodeId> = touched.iter().copied().collect();
    doc.nodes
        .iter()
        .filter_map(|(&id, node)| match &node.kind {
            NodeKind::Connector(props) => [props.start.node, props.end.node].into_iter().flatten().any(|n| deepest(doc, &touched, n).is_some()).then_some(id),
            _ => None,
        })
        .collect()
}

/// The candidate rule, stated with parent pointers only (no index, no subtree walk).
fn expected_candidates(doc: &Document, touched: &[NodeId]) -> Vec<NodeId> {
    let touched_set: BTreeSet<NodeId> = touched.iter().copied().collect();
    let attached: BTreeSet<NodeId> = expected_touching(doc, touched).into_iter().collect();
    let mut out = Vec::new();
    for (&id, node) in &doc.nodes {
        let NodeKind::Connector(props) = &node.kind else { continue };
        if touched_set.contains(&id) {
            out.push(id);
            continue;
        }
        if !attached.contains(&id) {
            continue;
        }
        if let (Some(parent), Some(start), Some(end)) = (node.parent, props.start.node, props.end.node) {
            if let Some(root) = deepest(doc, &touched_set, parent) {
                if start != root && end != root && deepest(doc, &touched_set, start) == Some(root) && deepest(doc, &touched_set, end) == Some(root) {
                    continue;
                }
            }
        }
        out.push(id);
    }
    out
}

fn candidates(doc: &Document, touched: &[NodeId]) -> Vec<NodeId> {
    with_index(doc, |idx| idx.reroute_candidates(doc, touched))
}

fn random_touched(rng: &mut Rng, scene: &Scene) -> Vec<NodeId> {
    let count = 1 + rng.below(3);
    (0..count)
        .map(|_| match rng.below(10) {
            0 => scene.page,
            1..=4 => rng.pick(&scene.frames),
            5..=7 => rng.pick(&scene.shapes),
            _ => rng.pick(&scene.connectors),
        })
        .collect()
}

fn assert_matches_rule(scene: &Scene, rng: &mut Rng, rounds: usize, label: &str) {
    for round in 0..rounds {
        let touched = random_touched(rng, scene);
        assert_eq!(candidates(&scene.doc, &touched), expected_candidates(&scene.doc, &touched), "candidates, {label}, round {round}, touched {touched:?}");
        assert_eq!(connectors_touching(&scene.doc, &touched), expected_touching(&scene.doc, &touched), "touching, {label}, round {round}, touched {touched:?}");
    }
}

#[test]
fn candidates_match_the_rule_on_generated_documents() {
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    // The last shape is large enough, against few connectors, that walks give up and scan instead.
    for (frames, shapes, connectors) in [(3, 6, 12), (5, 10, 60), (2, 4, 5), (4, 30, 120), (3, 90, 4)] {
        for seed in 0..40 {
            clear_connector_index();
            let scene = random_scene(&mut rng, frames, shapes, connectors);
            assert_matches_rule(&scene, &mut rng, 8, &format!("{frames} frames, {shapes} shapes, {connectors} connectors, seed {seed}"));
        }
    }
}

#[test]
fn candidates_match_the_rule_after_edits_to_the_document() {
    let mut rng = Rng(0x2545_F491_4F6C_DD1D);
    for seed in 0..30 {
        clear_connector_index();
        let mut scene = random_scene(&mut rng, 4, 12, 50);
        assert_matches_rule(&scene, &mut rng, 3, &format!("before edits, seed {seed}"));
        for _ in 0..6 {
            let id = rng.pick(&scene.connectors);
            let target = rng.pick(&scene.shapes);
            if let NodeKind::Connector(props) = &mut scene.doc.get_mut(id).unwrap().kind {
                props.end = ConnectorEnd::attached(target, Magnet::Auto);
            }
        }
        let victim = scene.connectors.swap_remove(rng.below(scene.connectors.len()));
        scene.doc.remove(victim).unwrap();
        assert_matches_rule(&scene, &mut rng, 5, &format!("after edits, seed {seed}"));
    }
}

/// Children lists that disagree with parent pointers (a connector listed twice, or under a
/// container that is not its parent) must not change the answer, which follows parent pointers.
#[test]
fn candidates_do_not_depend_on_how_container_children_list_their_connectors() {
    let mut rng = Rng(0xD1B5_4A32_D192_ED03);
    for seed in 0..40 {
        clear_connector_index();
        let mut scene = random_scene(&mut rng, 3, 8, 40);
        for _ in 0..4 {
            let container = rng.pick(&scene.frames);
            let connector = rng.pick(&scene.connectors);
            let listed_twice = scene.doc.get(container).unwrap().children.contains(&connector);
            let elsewhere = scene.doc.get(connector).unwrap().parent != Some(container);
            if listed_twice || elsewhere {
                scene.doc.get_mut(container).unwrap().children.push(connector);
            }
        }
        // `connectors_touching` follows children lists, so only the candidates are held to the rule here.
        for round in 0..6 {
            let touched = random_touched(&mut rng, &scene);
            assert_eq!(
                candidates(&scene.doc, &touched),
                expected_candidates(&scene.doc, &touched),
                "misleading children lists, seed {seed}, round {round}, touched {touched:?}"
            );
        }
    }
}

/// A connector listed twice under its frame adds as many attachments as a connector outside the
/// frame that is attached to the same two nodes would; the outside one must still be found.
#[test]
fn a_connector_outside_the_frame_is_found_when_an_inner_one_is_listed_twice() {
    clear_connector_index();
    let mut doc = Document::new("reroute_candidates_listed_twice");
    let page = doc.add_page("Page 1");
    let frame = doc.alloc_id();
    doc.insert(page, Node::frame(frame, Rect::new(0.0, 0.0, 500.0, 500.0))).unwrap();
    let nodes: Vec<NodeId> = (0..2)
        .map(|i| {
            let id = doc.alloc_id();
            doc.insert(frame, Node::rectangle(id, Rect::new(i as f32 * 100.0, 0.0, 80.0, 60.0))).unwrap()
        })
        .collect();
    let mut connect = |parent: NodeId| {
        let id = doc.alloc_id();
        let props =
            ConnectorProps { start: ConnectorEnd::attached(nodes[0], Magnet::Auto), end: ConnectorEnd::attached(nodes[1], Magnet::Auto), ..Default::default() };
        doc.insert(parent, Node::connector(id, props)).unwrap()
    };
    let inner = connect(frame);
    let outer = connect(page);
    doc.get_mut(frame).unwrap().children.push(inner);

    assert_eq!(candidates(&doc, &[frame]), vec![outer]);
    assert_eq!(expected_candidates(&doc, &[frame]), vec![outer]);
}

/// A frame holding two connectors (`inner`) between two rects inside it, and a rect outside it.
struct NestedConnectors {
    doc: Document,
    frame: NodeId,
    inner: [NodeId; 2],
    outside: NodeId,
}

fn nested_connectors() -> NestedConnectors {
    let mut doc = Document::new("reroute_candidates_connector_ends");
    let page = doc.add_page("Page 1");
    let frame = doc.alloc_id();
    doc.insert(page, Node::frame(frame, Rect::new(0.0, 0.0, 500.0, 500.0))).unwrap();
    let rects: Vec<NodeId> = (0..2)
        .map(|i| {
            let id = doc.alloc_id();
            doc.insert(frame, Node::rectangle(id, Rect::new(i as f32 * 100.0, 0.0, 80.0, 60.0))).unwrap()
        })
        .collect();
    let inner = [connect(&mut doc, frame, rects[0], rects[1]), connect(&mut doc, frame, rects[1], rects[0])];
    let outside = doc.alloc_id();
    doc.insert(page, Node::rectangle(outside, Rect::new(900.0, 0.0, 80.0, 60.0))).unwrap();
    NestedConnectors { doc, frame, inner, outside }
}

fn connect(doc: &mut Document, parent: NodeId, start: NodeId, end: NodeId) -> NodeId {
    let id = doc.alloc_id();
    let props = ConnectorProps { start: ConnectorEnd::attached(start, Magnet::Auto), end: ConnectorEnd::attached(end, Magnet::Auto), ..Default::default() };
    doc.insert(parent, Node::connector(id, props)).unwrap()
}

/// Both ways of finding the candidates (the subtree walk, and the scan the walk falls back to
/// on a big subtree) must give the same answer, and it must be the rule's. `build` returns the
/// document, the frame to move and the connectors that must be rerouted.
fn assert_walk_and_scan_agree(build: impl Fn() -> (Document, NodeId, Vec<NodeId>)) {
    let (small, frame, wanted) = build();
    let (mut big, ..) = build();
    // Enough rects after the connectors (so their ids are the same) to make the walk give up.
    for i in 0..200 {
        let id = big.alloc_id();
        big.insert(frame, Node::rectangle(id, Rect::new(i as f32, 200.0, 4.0, 4.0))).unwrap();
    }
    clear_connector_index();
    assert!(with_index(&small, |idx| idx.walk_touched(&small, &[frame], true).is_some()), "small subtree is walked");
    assert!(with_index(&big, |idx| idx.walk_touched(&big, &[frame], true).is_none()), "big subtree is scanned instead");
    assert_eq!(candidates(&small, &[frame]), wanted, "walk");
    assert_eq!(candidates(&big, &[frame]), wanted, "scan");
    assert_eq!(expected_candidates(&small, &[frame]), wanted, "rule");
    assert_eq!(connectors_touching(&small, &[frame]), connectors_touching(&big, &[frame]), "touching: walk vs scan");
    assert_eq!(connectors_touching(&small, &[frame]), expected_touching(&small, &[frame]), "touching: walk vs rule");
}

#[test]
fn a_connector_ending_on_a_connector_inside_a_moved_frame_is_rerouted() {
    // Parented to the frame, starting on an inner connector and ending outside: the end outside
    // does not move with the frame, the one on the inner connector does.
    assert_walk_and_scan_agree(|| {
        let mut n = nested_connectors();
        let cross = connect(&mut n.doc, n.frame, n.inner[0], n.outside);
        (n.doc, n.frame, vec![cross])
    });
}

#[test]
fn a_connector_outside_a_moved_frame_with_both_ends_on_inner_connectors_is_rerouted() {
    // Parented to the page: its parent stays put while both ends move.
    assert_walk_and_scan_agree(|| {
        let mut n = nested_connectors();
        let page = n.doc.get(n.frame).unwrap().parent.unwrap();
        let cross = connect(&mut n.doc, page, n.inner[0], n.inner[1]);
        (n.doc, n.frame, vec![cross])
    });
}

#[test]
fn a_connector_between_inner_connectors_stays_put_with_the_frame() {
    // Parent and both ends inside the frame: nothing to reroute.
    assert_walk_and_scan_agree(|| {
        let mut n = nested_connectors();
        connect(&mut n.doc, n.frame, n.inner[0], n.inner[1]);
        (n.doc, n.frame, Vec::new())
    });
}

/// Two mistakes in a container's children list that leave its count right: a connector listed
/// twice, and a connector whose parent is the container but which the list leaves out. Both
/// connectors have an end inside the frame, so both must be found.
#[test]
fn candidates_survive_a_children_list_that_repeats_one_connector_and_omits_another() {
    clear_connector_index();
    let mut n = nested_connectors();
    let inside = n.doc.get(n.frame).unwrap().children[0];
    let outside_far = {
        let page = n.doc.get(n.frame).unwrap().parent.unwrap();
        let id = n.doc.alloc_id();
        n.doc.insert(page, Node::rectangle(id, Rect::new(1200.0, 0.0, 80.0, 60.0))).unwrap()
    };
    let crossing = connect(&mut n.doc, n.frame, inside, n.outside);
    let ghost = connect(&mut n.doc, n.frame, inside, outside_far);
    let children = &mut n.doc.get_mut(n.frame).unwrap().children;
    children.retain(|c| *c != ghost);
    children.push(crossing);

    let want = vec![crossing, ghost];
    assert_eq!(expected_candidates(&n.doc, &[n.frame]), want, "rule");
    assert_eq!(candidates(&n.doc, &[n.frame]), want);
}

/// A connector the parent index records under the frame but the frame's children leave out is
/// still inside the frame: a connector ending on it moves with the frame.
#[test]
fn a_connector_ending_on_a_connector_that_its_frame_does_not_list_is_rerouted() {
    clear_connector_index();
    let mut n = nested_connectors();
    let inside = n.doc.get(n.frame).unwrap().children[0];
    let ghost = connect(&mut n.doc, n.frame, inside, n.outside);
    n.doc.get_mut(n.frame).unwrap().children.retain(|c| *c != ghost);
    let page = n.doc.get(n.frame).unwrap().parent.unwrap();
    let crossing = connect(&mut n.doc, page, ghost, n.outside);

    assert_eq!(expected_candidates(&n.doc, &[n.frame]), vec![ghost, crossing], "rule");
    assert_eq!(candidates(&n.doc, &[n.frame]), vec![ghost, crossing]);
}

/// A connector listed by a frame it does not belong to keeps its own parent's root: its parent
/// is a different touched frame, so its ends (in this frame) moved relative to it.
#[test]
fn a_connector_listed_under_a_frame_other_than_its_parent_is_judged_by_its_parent() {
    clear_connector_index();
    let mut n = nested_connectors();
    let page = n.doc.get(n.frame).unwrap().parent.unwrap();
    let other = n.doc.alloc_id();
    n.doc.insert(page, Node::frame(other, Rect::new(2000.0, 0.0, 500.0, 500.0))).unwrap();
    let rects: Vec<NodeId> = n.doc.get(n.frame).unwrap().children[..2].to_vec();
    let misfiled = connect(&mut n.doc, other, rects[0], rects[1]);
    n.doc.get_mut(other).unwrap().children.retain(|c| *c != misfiled);
    n.doc.get_mut(n.frame).unwrap().children.push(misfiled);

    let touched = [n.frame, other];
    assert_eq!(expected_candidates(&n.doc, &touched), vec![misfiled], "rule");
    assert_eq!(candidates(&n.doc, &touched), vec![misfiled]);
}

/// A touched connector listed twice by its frame, one connector missing from the frame's children
/// and a connector outside the frame: attachments add up as if the lists were right.
#[test]
fn repeat_listing_of_a_touched_connector_does_not_hide_connectors_from_the_scan() {
    clear_connector_index();
    let mut n = nested_connectors();
    let page = n.doc.get(n.frame).unwrap().parent.unwrap();
    let rects: Vec<NodeId> = n.doc.get(n.frame).unwrap().children[..2].to_vec();
    let touched_connector = n.inner[0];
    let ghost = connect(&mut n.doc, n.frame, rects[0], n.outside);
    let outer = connect(&mut n.doc, page, rects[1], n.outside);
    let children = &mut n.doc.get_mut(n.frame).unwrap().children;
    children.retain(|c| *c != ghost);
    children.push(touched_connector);

    let touched = [n.frame, touched_connector];
    let want = expected_candidates(&n.doc, &touched);
    assert!(want.contains(&ghost) && want.contains(&outer), "rule: {want:?}");
    assert_eq!(candidates(&n.doc, &touched), want);
}
