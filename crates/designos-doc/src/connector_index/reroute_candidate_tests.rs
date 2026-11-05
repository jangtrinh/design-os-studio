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
/// containers. Connector ends are random shapes or frames (a tenth are free, a tenth sit on one
/// node at both ends), so connectors cross frame boundaries, sit inside them, and end on frames.
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
        let a = rng.pick(&ends);
        let b = if rng.below(10) == 0 { a } else { rng.pick(&ends) };
        let end = |rng: &mut Rng, node: NodeId| if rng.below(10) == 0 { ConnectorEnd::free(Vec2::ZERO) } else { ConnectorEnd::attached(node, Magnet::Auto) };
        let props = ConnectorProps { start: end(rng, a), end: end(rng, b), ..Default::default() };
        doc.insert(parent, Node::connector(id, props)).unwrap();
        connector_ids.push(id);
    }
    Scene { doc, page, frames: frame_ids, shapes: shape_ids, connectors: connector_ids }
}

/// The rule, stated with parent pointers only (no index, no subtree walk).
fn expected_candidates(doc: &Document, touched: &[NodeId]) -> Vec<NodeId> {
    let touched: BTreeSet<NodeId> = touched.iter().copied().collect();
    let deepest = |id: NodeId| {
        let mut cur = Some(id);
        while let Some(node) = cur {
            if touched.contains(&node) {
                return Some(node);
            }
            cur = doc.get(node).and_then(|n| n.parent);
        }
        None
    };
    let mut out = Vec::new();
    for (&id, node) in &doc.nodes {
        let NodeKind::Connector(props) = &node.kind else { continue };
        let (start, end) = (props.start.node, props.end.node);
        if touched.contains(&id) {
            out.push(id);
            continue;
        }
        if ![start, end].into_iter().flatten().any(|n| deepest(n).is_some()) {
            continue;
        }
        if let (Some(parent), Some(start), Some(end)) = (node.parent, start, end) {
            if let Some(root) = deepest(parent) {
                if start != root && end != root && deepest(start) == Some(root) && deepest(end) == Some(root) {
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
        assert_eq!(candidates(&scene.doc, &touched), expected_candidates(&scene.doc, &touched), "{label}, round {round}, touched {touched:?}");
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
        assert_matches_rule(&scene, &mut rng, 6, &format!("misleading children lists, seed {seed}"));
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
