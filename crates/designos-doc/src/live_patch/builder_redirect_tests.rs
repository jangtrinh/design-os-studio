//! A layer the editor replaced (the shape builder's remainder) is built from geometry in its own
//! coordinate frame, so only the agent's appearance edits follow it; its shape edits are refused.

use super::*;
use crate::{Color, CommentAnchor, Document, Node, NodeId, Paint, Rect, Vec2};
use serde_json::{json, Value};

const REFUSAL: &str = "was reshaped by the shape builder while this edit waited";

/// A document where the editor replaced layer `old` by `fresh`, as the shape builder does, and
/// the replacement sits somewhere else.
fn replaced_layer() -> (Document, LivePatchState, NodeId, NodeId, NodeId) {
    let mut doc = Document::new("builder");
    let page = doc.add_page("Page");
    let old = doc.alloc_id();
    doc.insert(page, Node::rectangle(old, Rect::new(0.0, 0.0, 100.0, 100.0))).unwrap();
    let fresh = doc.alloc_id();
    doc.insert(page, Node::rectangle(fresh, Rect::new(30.0, 40.0, 60.0, 60.0))).unwrap();
    doc.remove(old).unwrap();
    let mut state = LivePatchState::default();
    state.record_replacement(old, fresh);
    (doc, state, page, old, fresh)
}

fn update(id: NodeId, set: Value) -> LiveOp {
    let Value::Object(set) = set else { panic!("fields are an object") };
    LiveOp::Update { id, set, unset: Vec::new() }
}

fn apply(doc: &mut Document, state: &mut LivePatchState, op: LiveOp) -> LivePatchReport {
    replay_live_patch(doc, &LivePatch::new(vec![op]), state, &mut |d, c| c.apply(d).map(|_| ())).unwrap()
}

#[test]
fn a_move_of_a_replaced_layer_is_refused_and_reported() {
    let (mut doc, mut state, _, old, fresh) = replaced_layer();
    let before = doc.get(fresh).unwrap().transform;
    let moved = serde_json::to_value(crate::Affine::translate(500.0, 0.0)).unwrap();
    let report = apply(&mut doc, &mut state, update(old, json!({ "transform": moved })));
    assert_eq!(doc.get(fresh).unwrap().transform, before, "the move landed in the replacement's coordinate frame");
    assert_eq!(report.applied, 0);
    assert_eq!(report.skipped.len(), 1, "{:?}", report.skipped);
    assert!(report.skipped[0].contains(&format!("layer {}", old.0)) && report.skipped[0].contains(REFUSAL), "{:?}", report.skipped);
}

#[test]
fn size_and_kind_of_a_replaced_layer_are_refused_each_on_its_own() {
    let (mut doc, mut state, _, old, fresh) = replaced_layer();
    let before = doc.get(fresh).unwrap().clone();
    let kind = serde_json::to_value(&Node::frame(old, Rect::new(0.0, 0.0, 1.0, 1.0)).kind).unwrap();
    let report = apply(&mut doc, &mut state, update(old, json!({ "size": [999.0, 999.0], "kind": kind })));
    assert_eq!(*doc.get(fresh).unwrap(), before);
    assert_eq!(report.skipped.len(), 2, "{:?}", report.skipped);
    assert!(report.skipped.iter().all(|s| s.contains(REFUSAL)), "{:?}", report.skipped);
}

#[test]
fn appearance_edits_still_follow_the_replacement_while_its_shape_edits_are_refused() {
    let (mut doc, mut state, _, old, fresh) = replaced_layer();
    let before = doc.get(fresh).unwrap().transform;
    let fills = serde_json::to_value(vec![Paint::solid(Color::rgb(255, 0, 0))]).unwrap();
    let mut stroke = doc.get(fresh).unwrap().stroke.clone();
    stroke.weight = 4.0;
    let set = json!({
        "name": "Agent name", "fills": fills, "stroke": stroke, "opacity": 0.5, "visible": false, "locked": true,
        "transform": serde_json::to_value(crate::Affine::translate(500.0, 0.0)).unwrap(),
    });
    let report = apply(&mut doc, &mut state, update(old, set));
    let node = doc.get(fresh).unwrap();
    assert_eq!((node.name.as_str(), node.opacity, node.visible, node.locked, node.stroke.weight), ("Agent name", 0.5, false, true, 4.0));
    assert_eq!(node.fills, vec![Paint::solid(Color::rgb(255, 0, 0))]);
    assert_eq!(node.transform, before);
    assert_eq!((report.applied, report.skipped.len()), (1, 1), "{report:?}");
}

#[test]
fn a_layer_that_still_exists_is_moved_normally() {
    let (mut doc, mut state, page, old, _) = replaced_layer();
    // The editor reverted the swap, so the old layer is back.
    doc.insert(page, Node::rectangle(old, Rect::new(0.0, 0.0, 100.0, 100.0))).unwrap();
    let moved = serde_json::to_value(crate::Affine::translate(500.0, 0.0)).unwrap();
    let report = apply(&mut doc, &mut state, update(old, json!({ "transform": moved })));
    assert!(report.skipped.is_empty(), "{:?}", report.skipped);
    assert_eq!(doc.get(old).unwrap().transform.tx, 500.0);
}

#[test]
fn replacements_are_forgotten_once_the_agent_works_from_a_newer_canvas() {
    let (mut doc, mut state, _, old, fresh) = replaced_layer();
    let rename = |name: &str, base| LivePatch::new(vec![update(old, json!({ "name": name }))]).with_base(base);
    let run =
        |doc: &mut Document, state: &mut LivePatchState, patch: LivePatch| replay_live_patch(doc, &patch, state, &mut |d, c| c.apply(d).map(|_| ())).unwrap();
    // The first patch seen knows nothing the replacement could contradict.
    assert!(run(&mut doc, &mut state, rename("First", Some(1))).skipped.is_empty());
    assert_eq!(doc.get(fresh).unwrap().name, "First");
    assert!(run(&mut doc, &mut state, rename("Same canvas", Some(1))).skipped.is_empty());
    // The agent reloaded the editor's canvas: its ids are the editor's now, and `old` is gone.
    let report = run(&mut doc, &mut state, rename("Newer canvas", Some(2)));
    assert_eq!(doc.get(fresh).unwrap().name, "Same canvas");
    assert_eq!(report.skipped.len(), 1, "{:?}", report.skipped);
}

#[test]
fn a_comment_on_a_replaced_layer_is_anchored_on_its_replacement() {
    let (mut doc, mut state, page, old, fresh) = replaced_layer();
    let thread =
        CommentThread { id: "t-1".into(), anchor: CommentAnchor { page, node: Some(old), offset: Vec2::new(1.0, 1.0) }, messages: Vec::new(), resolved: false };
    apply(&mut doc, &mut state, LiveOp::Comments { added: vec![thread], changed: Vec::new(), removed: Vec::new() });
    assert_eq!(doc.comments.len(), 1);
    assert_eq!(doc.comments[0].anchor.node, Some(fresh));
}
