//! Connector rules that depend on the kind of file: whiteboard files keep their own targeting,
//! design files follow the layer rules, and what each file kind offers in the toolbar.

use super::*;
use crate::canvas::tools::Tool;
use designos_doc::{Affine, Command, Node, NodeKind, Rect as DRect, ShapeKind};

fn board_sim() -> Sim {
    Sim::with(Options { fixture: Some("board-empty".into()), ..Default::default() })
}

/// A design file with an empty first page, at 100% zoom.
fn design_sim() -> Sim {
    let mut s = Sim::new();
    let page = s.app().page;
    let existing: Vec<NodeId> = s.app().doc.get(page).unwrap().children.clone();
    for id in existing {
        let _ = s.shell.app.doc.remove(id);
    }
    s.shell.app.selection.clear();
    s.shell.app.hover = None;
    s.shell.app.canvas.last_hover_query = None;
    s.shell.app.view.zoom = 1.0;
    s.shell.app.view.offset = DV::ZERO;
    s.shell.app.changed();
    s.frame(vec![]);
    s
}

fn add(s: &mut Sim, parent: NodeId, make: impl FnOnce(NodeId) -> Node, name: &str) -> NodeId {
    let fresh = s.shell.app.doc.alloc_id();
    let mut n = make(fresh);
    n.name = name.into();
    let id = s.shell.app.doc.insert(parent, n).unwrap();
    s.shell.app.changed();
    s.frame(vec![]);
    id
}

fn shape(s: &mut Sim, parent: NodeId, name: &str, x: f32, y: f32) -> NodeId {
    add(s, parent, |id| Node::shape_with_text(id, DRect::new(x, y, 160.0, 100.0), ShapeKind::RoundedRect, "box"), name)
}

fn group(s: &mut Sim, parent: NodeId, x: f32, y: f32, w: f32, h: f32) -> NodeId {
    let id = s.shell.app.doc.alloc_id();
    let mut n = Node::new(id, NodeKind::Group);
    n.transform = Affine::translate(x, y);
    n.size = DV::new(w, h);
    add(s, parent, move |_| n, "Group")
}

fn section(s: &mut Sim, parent: NodeId, x: f32, y: f32, w: f32, h: f32) -> NodeId {
    let id = s.shell.app.doc.alloc_id();
    let mut n = Node::new(id, NodeKind::Section);
    n.transform = Affine::translate(x, y);
    n.size = DV::new(w, h);
    add(s, parent, move |_| n, "Section")
}

fn center(s: &Sim, id: NodeId) -> Pos2 {
    let r = crate::util::world_aabb(&s.app().doc, id);
    s.screen(r.x + r.w / 2.0, r.y + r.h / 2.0)
}

fn connectors(s: &Sim) -> Vec<NodeId> {
    let mut v: Vec<NodeId> = s.app().doc.nodes.values().filter(|n| matches!(n.kind, NodeKind::Connector(_))).map(|n| n.id).collect();
    v.sort();
    v
}

/// `(start node, end node, parent)` of a connector.
fn ends(s: &Sim, c: NodeId) -> (Option<NodeId>, Option<NodeId>, Option<NodeId>) {
    let n = s.app().doc.get(c).unwrap();
    let NodeKind::Connector(p) = &n.kind else { unreachable!() };
    (p.start.node, p.end.node, n.parent)
}

#[test]
fn board_hover_dots_reach_a_shape_inside_a_group() {
    let mut s = board_sim();
    let page = s.app().page;
    let g = group(&mut s, page, 150.0, 150.0, 560.0, 100.0);
    let a = shape(&mut s, g, "A", 0.0, 0.0);
    let _b = shape(&mut s, g, "B", 400.0, 0.0);
    s.mv(center(&s, a));
    s.mv(center(&s, a) + egui::vec2(1.0, 0.0));
    assert_eq!(s.app().board.dots, Some(a), "the dots sit on the shape under the pointer, not on its group");
}

#[test]
fn board_connector_tool_attaches_to_the_shapes_inside_a_group_not_the_group() {
    let mut s = board_sim();
    let page = s.app().page;
    let g = group(&mut s, page, 150.0, 150.0, 560.0, 100.0);
    let a = shape(&mut s, g, "A", 0.0, 0.0);
    let b = shape(&mut s, g, "B", 400.0, 0.0);
    s.shell.app.set_tool(Tool::Connector);
    s.drag(center(&s, a), center(&s, b));
    let cs = connectors(&s);
    assert_eq!(cs.len(), 1);
    assert_eq!(ends(&s, cs[0]), (Some(a), Some(b), Some(page)), "both ends sit on the shapes and the connector stays on the page");
}

#[test]
fn board_connector_dropped_on_the_empty_area_of_a_section_stays_a_free_end() {
    let mut s = board_sim();
    let page = s.app().page;
    let a = shape(&mut s, page, "A", 80.0, 200.0);
    let _sec = section(&mut s, page, 420.0, 80.0, 500.0, 420.0);
    s.shell.app.set_tool(Tool::Connector);
    s.drag(center(&s, a), s.screen(700.0, 300.0));
    let cs = connectors(&s);
    assert_eq!(cs.len(), 1);
    assert_eq!(ends(&s, cs[0]), (Some(a), None, Some(page)), "a section is not a target: the end stays free");
}

#[test]
fn board_connector_between_two_shapes_in_a_section_keeps_the_page_as_its_parent() {
    let mut s = board_sim();
    let page = s.app().page;
    let sec = section(&mut s, page, 80.0, 80.0, 800.0, 400.0);
    let a = shape(&mut s, sec, "A", 40.0, 120.0);
    let b = shape(&mut s, sec, "B", 560.0, 120.0);
    s.shell.app.set_tool(Tool::Connector);
    s.drag(center(&s, a), center(&s, b));
    let cs = connectors(&s);
    assert_eq!(cs.len(), 1);
    assert_eq!(ends(&s, cs[0]), (Some(a), Some(b), Some(page)), "the connector is a child of the page, not of the section");
    assert!(!s.app().doc.get(cs[0]).unwrap().layout_child.absolute, "a board connector is stored as before");
}

#[test]
fn board_connector_stays_on_the_page_when_one_of_its_shapes_is_regrouped() {
    let mut s = board_sim();
    let page = s.app().page;
    let sec = section(&mut s, page, 80.0, 80.0, 800.0, 400.0);
    let a = shape(&mut s, sec, "A", 40.0, 120.0);
    let b = shape(&mut s, sec, "B", 560.0, 120.0);
    let g = group(&mut s, sec, 300.0, 300.0, 100.0, 50.0);
    s.shell.app.set_tool(Tool::Connector);
    s.drag(center(&s, a), center(&s, b));
    let c = connectors(&s)[0];
    let mut tx = crate::edit::Tx::new();
    tx.cmd(&mut s.shell.app.doc, Command::Reparent { id: b, parent: g, index: 0 }).unwrap();
    s.shell.app.run_tx(tx);
    s.frame(vec![]);
    assert_eq!(s.app().doc.get(b).unwrap().parent, Some(g));
    assert_eq!(s.app().doc.get(c).unwrap().parent, Some(page), "a commit does not move a board connector into the section");
}
