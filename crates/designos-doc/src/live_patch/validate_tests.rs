//! Patch lines come from a file any local process can write, so malformed edits must be refused
//! before they touch the document or its id counter.

use super::*;
use crate::{Command, DocError, Document, Node, NodeId, Rect, MAX_TREE_DEPTH, MIN_ID_HEADROOM};

fn doc_with_frame() -> (Document, NodeId, NodeId) {
    let mut d = Document::new("validate");
    let page = d.add_page("Page");
    let frame = d.alloc_id();
    d.insert(page, Node::frame(frame, Rect::new(0.0, 0.0, 100.0, 100.0))).unwrap();
    let kid = d.alloc_id();
    d.insert(frame, Node::rectangle(kid, Rect::new(0.0, 0.0, 10.0, 10.0))).unwrap();
    (d, page, frame)
}

/// A frame `id` whose children are `kids`, under `parent`.
fn frame(id: u64, parent: Option<u64>, kids: &[u64]) -> Node {
    let mut n = Node::frame(NodeId(id), Rect::new(0.0, 0.0, 10.0, 10.0));
    n.parent = parent.map(NodeId);
    n.children = kids.iter().map(|k| NodeId(*k)).collect();
    n
}

/// A rectangle (which cannot hold layers) that lists `kid` as its child.
fn leaf_with_child(id: u64, kid: u64) -> Node {
    let mut n = Node::rectangle(NodeId(id), Rect::new(0.0, 0.0, 5.0, 5.0));
    n.children = vec![NodeId(kid)];
    n
}

fn apply(doc: &mut Document, state: &mut LivePatchState, op: LiveOp) -> LivePatchReport {
    replay_live_patch(doc, &LivePatch::new(vec![op]), state, &mut |d, c| c.apply(d).map(|_| ())).unwrap()
}

#[test]
fn malformed_inserts_are_refused_without_touching_the_document() {
    let (base, page, _) = doc_with_frame();
    let deep: Vec<Node> = (0..=MAX_TREE_DEPTH as u64).map(|i| frame(1000 + i, (i > 0).then(|| 999 + i), &[1001 + i])).collect();
    let cases: Vec<(&str, Vec<Node>)> = vec![
        ("repeated id", vec![frame(100, None, &[101]), frame(101, Some(100), &[]), frame(101, Some(100), &[])]),
        ("second root", vec![frame(100, None, &[]), frame(101, None, &[])]),
        ("parent outside the subtree", vec![frame(100, None, &[]), frame(101, Some(page.0), &[])]),
        ("parent does not list its child", vec![frame(100, None, &[]), frame(101, Some(100), &[])]),
        ("child listed twice", vec![frame(100, None, &[101, 101]), frame(101, Some(100), &[])]),
        ("unreachable cycle", vec![frame(100, None, &[]), frame(101, Some(102), &[102]), frame(102, Some(101), &[101])]),
        ("too deep", deep),
        ("id near the end of the id space", vec![frame(u64::MAX - MIN_ID_HEADROOM + 1, None, &[])]),
        ("leaf with children", vec![leaf_with_child(100, 101), frame(101, Some(100), &[])]),
    ];
    for (what, nodes) in cases {
        let mut doc = base.clone();
        let next_id = doc.clone().alloc_id();
        let report = apply(&mut doc, &mut LivePatchState::default(), LiveOp::Insert { parent: page, at: SiblingHint::default(), nodes });
        assert!(!report.skipped.is_empty(), "{what}: must be reported as skipped");
        assert_eq!(report.applied, 0, "{what}");
        assert_eq!(doc.nodes.len(), base.nodes.len(), "{what}: the document changed");
        assert_eq!(doc.clone().alloc_id(), next_id, "{what}: the id counter moved");
    }
}

#[test]
fn an_insert_may_list_existing_layers_that_later_ops_move_in() {
    // Wrapping existing layers in a new frame: the frame lists them, and Reparent ops move them.
    let (mut doc, page, frame_id) = doc_with_frame();
    let wrapper = frame(500, None, &[frame_id.0]);
    let ops = vec![
        LiveOp::Insert { parent: page, at: SiblingHint::default(), nodes: vec![wrapper] },
        LiveOp::Reparent { id: frame_id, parent: NodeId(500), at: SiblingHint::default() },
    ];
    let report = replay_live_patch(&mut doc, &LivePatch::new(ops), &mut LivePatchState::default(), &mut |d, c| c.apply(d).map(|_| ())).unwrap();
    assert!(report.skipped.is_empty(), "{:?}", report.skipped);
    assert_eq!(doc.get(frame_id).unwrap().parent, Some(NodeId(500)));
}

#[test]
fn an_update_cannot_turn_a_frame_with_layers_into_a_leaf() {
    let (mut doc, _, frame_id) = doc_with_frame();
    let set = [("kind".to_string(), serde_json::json!("Rectangle"))].into_iter().collect();
    let report = apply(&mut doc, &mut LivePatchState::default(), LiveOp::Update { id: frame_id, set, unset: Vec::new() });
    assert!(!report.skipped.is_empty());
    assert!(doc.get(frame_id).unwrap().kind.is_container());
}

#[test]
fn a_failed_replay_leaves_no_id_remaps_behind() {
    let (mut doc, page, frame_id) = doc_with_frame();
    // The agent inserts a layer under an id the editor already uses (it gets a fresh id), then
    // the editor fails to apply the next command, so nothing of the patch lands.
    let colliding = Node::rectangle(frame_id, Rect::new(0.0, 0.0, 5.0, 5.0));
    let rename = |name: &str| LiveOp::Update { id: frame_id, set: [("name".to_string(), serde_json::json!(name))].into_iter().collect(), unset: Vec::new() };
    let ops = vec![LiveOp::Insert { parent: page, at: SiblingHint::default(), nodes: vec![colliding] }, rename("agent layer")];
    let before = doc.clone();
    let mut state = LivePatchState::default();
    let mut calls = 0;
    let failed = replay_live_patch(&mut doc, &LivePatch::new(ops), &mut state, &mut |d, c: Command| {
        calls += 1;
        if calls == 1 {
            c.apply(d).map(|_| ())
        } else {
            Err(DocError::NotFound(frame_id))
        }
    });
    assert!(failed.is_err());
    // The editor cancels the transaction; here the document is put back by hand.
    doc = before;
    // The agent's next edit of that id must reach the editor's node, not a remap to a layer
    // that never landed.
    let report = apply(&mut doc, &mut state, rename("renamed"));
    assert!(report.skipped.is_empty(), "{:?}", report.skipped);
    assert_eq!(doc.get(frame_id).unwrap().name, "renamed");
}
