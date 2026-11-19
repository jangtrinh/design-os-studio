//! An agent edit that reaches the open editor as a live patch leaves the editor's document exactly as it leaves the
//! agent's: every node compares equal, field by field, not only by place and size. The agent side runs through the real
//! tool path (`Engine::call` and the live ops it records), and the editor side through `apply_live_patches`.

use crate::app::App;
use crate::group_delete_layout_tests::{frame_of, rect_in};
use designos_doc::live_patch::LivePatch;
use designos_doc::{AutoLayout, Document, FrameProps, LayoutAlign, LayoutDirection, NodeId, NodeKind, Rect, Sizing};
use designos_mcp::engine::Engine;
use serde_json::{json, Value};

fn auto_layout(direction: LayoutDirection, sizing: Sizing) -> NodeKind {
    let layout = AutoLayout { direction, primary_sizing: sizing, counter_sizing: sizing, primary_align: LayoutAlign::Start, ..AutoLayout::default() };
    NodeKind::Frame(FrameProps { auto_layout: Some(layout), ..FrameProps::default() })
}

/// An agent engine and an open editor that start from one document, built by `build` under the default page.
pub(crate) fn pair<T>(build: impl FnOnce(&mut Document, NodeId) -> T) -> (Engine, App, T) {
    let mut engine = Engine::new();
    let page = engine.resolve_default_page().unwrap();
    let ids = build(&mut engine.doc, page);
    let app = App::new(engine.doc.clone());
    engine.live_ops = Some(Vec::new());
    (engine, app, ids)
}

/// The agent runs the tool call; the editor replays the patch it recorded, after a trip through the log's JSON.
fn run_agent_call(engine: &mut Engine, app: &mut App, tool: &str, args: Value) {
    engine.call(tool, args).unwrap();
    let ops = engine.take_live_ops();
    assert!(!ops.is_empty(), "the call recorded live ops");
    let layout = engine.take_live_layout();
    let patch: LivePatch = serde_json::from_str(&serde_json::to_string(&LivePatch::new(ops).with_layout(layout)).unwrap()).unwrap();
    crate::live_patch_apply::apply_live_patches(app, &[patch]);
}

pub(crate) fn assert_same_documents(engine: &Engine, app: &App) {
    assert_same_documents_with(engine, app, "");
}

pub(crate) fn assert_same_documents_with(engine: &Engine, app: &App, context: &str) {
    let agent: Vec<(NodeId, Value)> = engine.doc.nodes.values().map(|n| (n.id, serde_json::to_value(n).unwrap())).collect();
    let editor: Vec<(NodeId, Value)> = app.doc.nodes.values().map(|n| (n.id, serde_json::to_value(n).unwrap())).collect();
    assert!(editor == agent, "the editor's document is the agent's, node by node: {context}");
}

fn size(doc: &Document, id: NodeId) -> (f32, f32) {
    let s = doc.get(id).unwrap().size;
    (s.x, s.y)
}

/// page > group (stored 300 x 80, wider than its one child) > frame (50 x 30) > rectangle. A layer added to the frame
/// changes nothing a group reads, so the group keeps the bounds the document stored.
#[test]
fn an_agent_insert_inside_a_frame_in_a_group_leaves_a_group_with_stored_bounds_alone() {
    let (mut engine, mut app, (group, frame)) = pair(|doc, page| {
        let group = frame_of(doc, page, Rect::new(0.0, 0.0, 300.0, 80.0), NodeKind::Group);
        let frame = frame_of(doc, group, Rect::new(0.0, 0.0, 50.0, 30.0), NodeKind::Frame(FrameProps::default()));
        rect_in(doc, frame, Rect::new(0.0, 0.0, 10.0, 10.0));
        (group, frame)
    });

    run_agent_call(&mut engine, &mut app, "designos_create_node", json!({ "kind": "rectangle", "parent_id": frame.0, "width": 20, "height": 20 }));

    assert_eq!(size(&engine.doc, group), (300.0, 80.0), "precondition: the agent side leaves the group alone");
    assert_eq!(size(&app.doc, group), (300.0, 80.0), "the editor leaves it alone too");
    assert_same_documents(&engine, &app);
}

/// screen (column) > group > card (hug column) > two rows. A row added to the card grows the card, the group fits the
/// card, and the screen reads the group: the editor runs the same chain as the agent.
#[test]
fn an_agent_insert_into_a_hug_column_inside_a_group_refits_the_group_like_the_agent_side() {
    let (mut engine, mut app, (group, card)) = pair(|doc, page| {
        let screen = frame_of(doc, page, Rect::new(0.0, 0.0, 400.0, 600.0), auto_layout(LayoutDirection::Vertical, Sizing::Fixed));
        let group = frame_of(doc, screen, Rect::new(0.0, 0.0, 60.0, 40.0), NodeKind::Group);
        let card = frame_of(doc, group, Rect::new(0.0, 0.0, 60.0, 40.0), auto_layout(LayoutDirection::Vertical, Sizing::Hug));
        rect_in(doc, card, Rect::new(0.0, 0.0, 60.0, 20.0));
        rect_in(doc, card, Rect::new(0.0, 20.0, 60.0, 20.0));
        rect_in(doc, screen, Rect::new(0.0, 300.0, 100.0, 30.0));
        (group, card)
    });

    run_agent_call(&mut engine, &mut app, "designos_create_node", json!({ "kind": "rectangle", "parent_id": card.0, "width": 60, "height": 20 }));

    assert_eq!(size(&engine.doc, group).1, 60.0, "precondition: the agent side grew the group with the card");
    assert_eq!(size(&app.doc, group).1, 60.0, "the editor grows it too");
    assert_same_documents(&engine, &app);
}

/// outer group > hug row > inner group > two layers. Deleting the first layer refits the inner group, the row hugs it,
/// and the outer group fits the row.
#[test]
fn an_agent_delete_inside_nested_groups_around_a_hug_frame_matches_the_agent_side() {
    let (mut engine, mut app, (outer, first)) = pair(|doc, page| {
        let outer = frame_of(doc, page, Rect::new(0.0, 0.0, 120.0, 20.0), NodeKind::Group);
        let row = frame_of(doc, outer, Rect::new(0.0, 0.0, 120.0, 20.0), auto_layout(LayoutDirection::Horizontal, Sizing::Hug));
        let inner = frame_of(doc, row, Rect::new(0.0, 0.0, 120.0, 20.0), NodeKind::Group);
        let first = rect_in(doc, inner, Rect::new(0.0, 0.0, 60.0, 20.0));
        rect_in(doc, inner, Rect::new(60.0, 0.0, 60.0, 20.0));
        (outer, first)
    });

    run_agent_call(&mut engine, &mut app, "designos_delete_node", json!({ "node_id": first.0 }));

    assert_eq!(size(&engine.doc, outer).0, 60.0, "precondition: the agent side shrank the outer group");
    assert_same_documents(&engine, &app);
}

/// The editor's own delete and the agent's delete leave the group with the same fields, not only the same box: a
/// refit does not turn the group's sizing to Fixed or drop what is drawn from its size.
#[test]
fn the_agent_delete_and_the_editor_delete_leave_a_group_with_the_same_fields() {
    let (mut engine, mut editor_app, (group, first)) = pair(|doc, page| {
        let group = frame_of(doc, page, Rect::new(0.0, 0.0, 80.0, 20.0), NodeKind::Group);
        doc.get_mut(group).unwrap().layout_child.set_axis_sizing(true, Sizing::Hug, None);
        let first = rect_in(doc, group, Rect::new(0.0, 0.0, 40.0, 20.0));
        rect_in(doc, group, Rect::new(40.0, 0.0, 40.0, 20.0));
        (group, first)
    });
    engine.call("designos_delete_node", json!({ "node_id": first.0 })).unwrap();

    editor_app.select_only(first);
    editor_app.delete_selection();

    let agent = serde_json::to_value(engine.doc.get(group).unwrap()).unwrap();
    let editor = serde_json::to_value(editor_app.doc.get(group).unwrap()).unwrap();
    assert_eq!(agent, editor);
}

/// One batch that resizes a layer in a hug frame inside a group and adds a layer to the same group: the agent's step
/// solves the frame (seeded by the resized layer) and then fits the group to it. The editor solves what the agent
/// solved, so the group is fitted after the frame, as on the agent side.
#[test]
fn an_agent_batch_that_resizes_a_layer_in_a_hug_frame_and_adds_a_layer_to_its_group_matches_the_agent_side() {
    let (mut engine, mut app, (group, r1)) = pair(|doc, page| {
        let screen = frame_of(doc, page, Rect::new(0.0, 0.0, 400.0, 600.0), auto_layout(LayoutDirection::Vertical, Sizing::Fixed));
        let group = frame_of(doc, screen, Rect::new(0.0, 0.0, 60.0, 40.0), NodeKind::Group);
        let card = frame_of(doc, group, Rect::new(0.0, 0.0, 60.0, 40.0), auto_layout(LayoutDirection::Vertical, Sizing::Hug));
        let r1 = rect_in(doc, card, Rect::new(0.0, 0.0, 60.0, 20.0));
        rect_in(doc, card, Rect::new(0.0, 20.0, 60.0, 20.0));
        rect_in(doc, screen, Rect::new(0.0, 300.0, 100.0, 30.0));
        (group, r1)
    });

    run_agent_call(
        &mut engine,
        &mut app,
        "designos_apply_batch",
        json!({ "ops": [
            { "op": "update", "node": r1.0, "height": 40 },
            { "op": "create", "parent": group.0, "kind": "rectangle", "x": 0, "y": 0, "width": 10, "height": 10 }
        ]}),
    );

    assert_eq!(size(&engine.doc, group).1, 60.0, "precondition: the agent side refits the group to the grown card");
    assert_same_documents(&engine, &app);
}

/// A patch line from a build that sent no layout replays as it did before the field existed: the solve that follows
/// the commit grows the hug frame, and nothing fits the group around it.
#[test]
fn a_patch_without_the_layout_field_replays_with_the_solve_after_the_commit_only() {
    let (mut engine, mut app, (group, card)) = pair(|doc, page| {
        let screen = frame_of(doc, page, Rect::new(0.0, 0.0, 400.0, 600.0), auto_layout(LayoutDirection::Vertical, Sizing::Fixed));
        let group = frame_of(doc, screen, Rect::new(0.0, 0.0, 60.0, 40.0), NodeKind::Group);
        let card = frame_of(doc, group, Rect::new(0.0, 0.0, 60.0, 40.0), auto_layout(LayoutDirection::Vertical, Sizing::Hug));
        rect_in(doc, card, Rect::new(0.0, 0.0, 60.0, 20.0));
        rect_in(doc, card, Rect::new(0.0, 20.0, 60.0, 20.0));
        (group, card)
    });
    engine.call("designos_create_node", json!({ "kind": "rectangle", "parent_id": card.0, "width": 60, "height": 20 })).unwrap();
    let ops = engine.take_live_ops();
    assert!(engine.take_live_layout().is_some(), "precondition: the agent's step solved a layout");
    let line = serde_json::to_string(&LivePatch::new(ops)).unwrap();
    assert!(!line.contains("layout"), "the line carries no layout: {line}");
    let patch: LivePatch = serde_json::from_str(&line).unwrap();

    crate::live_patch_apply::apply_live_patches(&mut app, &[patch]);

    assert_eq!(size(&app.doc, card).1, 60.0, "the solve after the commit grows the hug frame");
    assert_eq!(size(&app.doc, group).1, 40.0, "and nothing fits the group, as before the field existed");
}

/// The agent's step named a container it created, and the editor refused that insert (its parent is gone). The id is not
/// the agent's container then, whatever layer the editor holds under it, so the carried layout leaves that layer alone.
#[test]
fn a_carried_container_the_editor_refused_to_create_does_not_reach_an_unrelated_layer() {
    use designos_doc::live_patch::{LiveOp, SiblingHint};
    use designos_doc::ExecLayout;
    let (_engine, mut app, group) = pair(|doc, page| {
        // A user's group stored wider than its one child: a refit would change it.
        let group = frame_of(doc, page, Rect::new(0.0, 0.0, 300.0, 80.0), NodeKind::Group);
        rect_in(doc, group, Rect::new(0.0, 0.0, 50.0, 30.0));
        group
    });
    // The agent's container carries the id the editor holds for that group.
    let clash = group;
    let stored = serde_json::to_value(app.doc.get(group).unwrap()).unwrap();
    let mut container = designos_doc::Node::frame(clash, Rect::new(0.0, 0.0, 10.0, 10.0));
    container.name = "agent container".into();
    let missing_parent = NodeId(9_999);
    let patch = LivePatch::new(vec![LiveOp::Insert { parent: missing_parent, at: SiblingHint::default(), nodes: vec![container] }])
        .with_layout(Some(ExecLayout { seeds: vec![clash], structural: vec![clash], scoped_solve_first: false }));

    crate::live_patch_apply::apply_live_patches(&mut app, &[patch]);

    assert_eq!(serde_json::to_value(app.doc.get(group).unwrap()).unwrap(), stored, "the editor's own group is untouched");
}

/// `designos_create_node` into the hug frame refits the group around it (see the test above). The batch tool solves the
/// frame with its own scoped solve before the climb, so the climb finds nothing moved and the group keeps its old box, on
/// the agent side and in the editor alike. Red until the batch tool's solve and the climb agree on that case.
#[test]
#[ignore = "a batch that adds a layer to a hug frame inside a group leaves the group at its old box"]
fn an_agent_batch_that_adds_a_layer_to_a_hug_frame_in_a_group_refits_the_group_like_create_node() {
    let (mut engine, mut app, (group, card)) = pair(|doc, page| {
        let screen = frame_of(doc, page, Rect::new(0.0, 0.0, 400.0, 600.0), auto_layout(LayoutDirection::Vertical, Sizing::Fixed));
        let group = frame_of(doc, screen, Rect::new(0.0, 0.0, 60.0, 40.0), NodeKind::Group);
        let card = frame_of(doc, group, Rect::new(0.0, 0.0, 60.0, 40.0), auto_layout(LayoutDirection::Vertical, Sizing::Hug));
        rect_in(doc, card, Rect::new(0.0, 0.0, 60.0, 20.0));
        rect_in(doc, card, Rect::new(0.0, 20.0, 60.0, 20.0));
        (group, card)
    });

    run_agent_call(
        &mut engine,
        &mut app,
        "designos_apply_batch",
        json!({ "ops": [{ "op": "create", "parent": card.0, "kind": "rectangle", "x": 0, "y": 0, "width": 60, "height": 20 }]}),
    );

    assert_eq!(size(&engine.doc, card).1, 60.0, "precondition: the frame grew");
    assert_eq!(size(&engine.doc, group).1, 60.0, "the group fits the grown frame");
    assert_same_documents(&engine, &app);
}
