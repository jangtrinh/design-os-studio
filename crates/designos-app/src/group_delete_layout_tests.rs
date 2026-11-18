//! The editor's own delete of a layer inside a group: the group refits the layers that are left (`fit_group`) and the
//! auto-layout frame around it solves again, in the same undo step. The headless tools follow the editor, so these
//! tests are what the designos-mcp tests for the same delete are measured against.

use crate::app::App;
use designos_doc::{AutoLayout, Document, FrameProps, LayoutAlign, LayoutDirection, Node, NodeId, NodeKind, Rect, Sizing};

struct Fixture {
    app: App,
    group: NodeId,
    a: NodeId,
    b: NodeId,
    tail: NodeId,
}

fn frame_of(doc: &mut Document, parent: NodeId, rect: Rect, kind: NodeKind) -> NodeId {
    let id = doc.alloc_id();
    let mut node = Node::frame(id, rect);
    node.kind = kind;
    doc.insert(parent, node).unwrap()
}

fn rect_in(doc: &mut Document, parent: NodeId, rect: Rect) -> NodeId {
    let id = doc.alloc_id();
    doc.insert(parent, Node::rectangle(id, rect)).unwrap()
}

/// page > screen (column, fixed 400 x 600, Start) > [group (80 x 20) > [a at 0, b at 40], tail stored at y = 200], or the same
/// group directly on the page when `in_auto_layout` is false.
fn fixture(in_auto_layout: bool) -> Fixture {
    let mut doc = Document::new("group-delete");
    let page = doc.add_page("p");
    let (host, tail_at) = if in_auto_layout {
        let layout = AutoLayout {
            direction: LayoutDirection::Vertical,
            primary_sizing: Sizing::Fixed,
            counter_sizing: Sizing::Fixed,
            primary_align: LayoutAlign::Start,
            ..AutoLayout::default()
        };
        let screen =
            frame_of(&mut doc, page, Rect::new(0.0, 0.0, 400.0, 600.0), NodeKind::Frame(FrameProps { auto_layout: Some(layout), ..FrameProps::default() }));
        (screen, 200.0)
    } else {
        (page, 200.0)
    };
    let group = frame_of(&mut doc, host, Rect::new(0.0, 0.0, 80.0, 20.0), NodeKind::Group);
    let a = rect_in(&mut doc, group, Rect::new(0.0, 0.0, 40.0, 20.0));
    let b = rect_in(&mut doc, group, Rect::new(40.0, 0.0, 40.0, 20.0));
    let tail = rect_in(&mut doc, host, Rect::new(0.0, tail_at, 100.0, 30.0));
    Fixture { app: App::new(doc), group, a, b, tail }
}

fn page_x(app: &App, id: NodeId) -> f32 {
    app.doc.world_transform(id).tx
}

fn stored(app: &App) -> Vec<(NodeId, [f32; 4])> {
    app.doc.nodes.values().map(|n| (n.id, [n.transform.tx, n.transform.ty, n.size.x, n.size.y])).collect()
}

#[test]
fn deleting_the_first_layer_of_a_group_in_auto_layout_moves_the_layer_that_is_left_with_the_flow() {
    let Fixture { mut app, group, a, b, tail } = fixture(true);
    let before = stored(&app);
    assert_eq!(page_x(&app, b), 40.0);

    app.select_only(a);
    app.delete_selection();

    assert_eq!(app.doc.get(group).unwrap().size.x, 40.0, "the group fits the layer that is left");
    assert_eq!(page_x(&app, b), 0.0, "the column places the narrower group at its own left edge, and the layer inside goes with it");
    assert_eq!(app.doc.get(tail).unwrap().transform.ty, 20.0, "the flow below follows the group");

    app.undo();
    assert_eq!(stored(&app), before, "undo puts every layer back where the document stored it");
}

#[test]
fn deleting_the_first_layer_of_a_group_outside_auto_layout_keeps_the_other_layer_in_place() {
    let Fixture { mut app, group, a, b, .. } = fixture(false);
    let before = stored(&app);

    app.select_only(a);
    app.delete_selection();

    assert_eq!(app.doc.get(group).unwrap().size.x, 40.0);
    assert_eq!(page_x(&app, b), 40.0, "nothing lays the group out again, so the layer that is left stays where it was");

    app.undo();
    assert_eq!(stored(&app), before);
}

/// The agent side deletes the layer and the open editor replays the patch: both documents end with the same group
/// bounds and the same flow around it, because the editor's replay is a history step that refits the group too.
#[test]
fn an_agent_delete_inside_a_group_reaches_the_editor_with_the_same_layout() {
    use designos_doc::live_patch::{live_ops_for_command, LivePatch};
    use designos_doc::{Command, History};

    let Fixture { mut app, group, a, tail, .. } = fixture(true);
    let mut agent_doc = app.doc.clone();
    let cmd = Command::Remove { id: a };
    let ops = live_ops_for_command(&agent_doc, &cmd).expect("the delete can be mirrored");
    let patch: LivePatch = serde_json::from_str(&serde_json::to_string(&LivePatch::new(ops)).unwrap()).unwrap();
    History::default().exec_agent(&mut agent_doc, cmd).expect("the agent applies it");
    assert_eq!(agent_doc.get(group).unwrap().size.x, 40.0, "precondition: the agent side refit the group");

    crate::live_patch_apply::apply_live_patches(&mut app, &[patch]);

    let editor: Vec<_> = stored(&app);
    let agent: Vec<_> = agent_doc.nodes.values().map(|n| (n.id, [n.transform.tx, n.transform.ty, n.size.x, n.size.y])).collect();
    assert_eq!(editor, agent);
    assert_eq!(app.doc.get(tail).unwrap().transform.ty, 20.0);
}
