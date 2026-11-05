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

fn rectangle(s: &mut Sim, parent: NodeId, name: &str, x: f32, y: f32) -> NodeId {
    add(s, parent, |id| Node::rectangle(id, DRect::new(x, y, 120.0, 80.0)), name)
}

fn ink_of(s: &Sim, c: NodeId) -> Color {
    match &s.app().doc.get(c).unwrap().stroke.paints[0].kind {
        PaintKind::Solid(col) => *col,
        k => panic!("expected a solid stroke, got {k:?}"),
    }
}

/// Draws a connector with the Connector tool; Cmd picks the deepest layer under each end.
fn connect_deep(s: &mut Sim, a: NodeId, b: NodeId) -> NodeId {
    s.shell.app.set_tool(Tool::Connector);
    s.mods = Modifiers::COMMAND;
    s.drag(center(s, a), center(s, b));
    s.mods = Modifiers::NONE;
    *connectors(s).last().expect("the drag made a connector")
}

#[test]
fn default_ink_reads_the_nearest_opaque_ancestor_fill() {
    let mut s = design_sim();
    let page = s.app().page;
    let screen = add(&mut s, page, |id| Node::frame(id, DRect::new(80.0, 80.0, 640.0, 360.0)), "Screen");
    let wrapper = add(
        &mut s,
        screen,
        |id| {
            let mut n = Node::frame(id, DRect::new(20.0, 20.0, 600.0, 320.0));
            n.fills.clear();
            n
        },
        "Wrapper",
    );
    let a = rectangle(&mut s, wrapper, "Card A", 40.0, 100.0);
    let b = rectangle(&mut s, wrapper, "Card B", 420.0, 100.0);
    let c = connect_deep(&mut s, a, b);
    assert_eq!(s.app().doc.get(c).unwrap().parent, Some(wrapper), "the connector is homed in the fill-less wrapper");
    let ink = ink_of(&s, c);
    assert_eq!(ink, crate::board::color_of(crate::theme::BOARD_INK[0]), "the white screen above the wrapper is the surface");
    assert!(designos_doc::lint::rules::contrast::contrast_ratio(ink, Color::WHITE) >= 3.0);
}

#[test]
fn default_ink_on_a_light_page_is_dark() {
    let mut s = design_sim();
    let page = s.app().page;
    let light = Color::rgb(0xF5, 0xF5, 0xF5);
    s.shell.app.doc.get_mut(page).unwrap().kind = NodeKind::Page { background: light };
    // a band that is only a 10% tint is not a surface: the page behind it is
    let band = add(
        &mut s,
        page,
        |id| {
            let mut n = Node::frame(id, DRect::new(80.0, 80.0, 640.0, 240.0));
            let mut tint = Paint::solid(Color::BLACK);
            tint.opacity = 0.1;
            n.fills = vec![tint];
            n
        },
        "Band",
    );
    let a = rectangle(&mut s, band, "Card A", 40.0, 60.0);
    let b = rectangle(&mut s, band, "Card B", 420.0, 60.0);
    let c = connect_deep(&mut s, a, b);
    assert_eq!(s.app().doc.get(c).unwrap().parent, Some(band));
    let ink = ink_of(&s, c);
    assert_eq!(ink, crate::board::color_of(crate::theme::BOARD_INK[0]));
    assert!(designos_doc::lint::rules::contrast::contrast_ratio(ink, light) >= 3.0);
}

#[test]
fn default_ink_on_the_dark_default_page_has_3_to_1() {
    let mut s = design_sim();
    let page = s.app().page;
    let a = add(&mut s, page, |id| Node::frame(id, DRect::new(120.0, 180.0, 160.0, 100.0)), "Frame A");
    let b = add(&mut s, page, |id| Node::frame(id, DRect::new(520.0, 180.0, 160.0, 100.0)), "Frame B");
    s.shell.app.set_tool(Tool::Connector);
    s.drag(center(&s, a), center(&s, b));
    let c = connectors(&s)[0];
    assert_eq!(s.app().doc.get(c).unwrap().parent, Some(page));
    let ink = ink_of(&s, c);
    assert_eq!(ink, crate::board::color_of(crate::theme::BOARD_INK_ON_DARK));
    assert!(designos_doc::lint::rules::contrast::contrast_ratio(ink, Color::rgb(0x5A, 0x5A, 0x5A)) >= 3.0);
}

#[test]
fn connector_ink_tokens_match_the_document_defaults() {
    let mut doc = designos_doc::Document::new("Ink tokens");
    let page = doc.add_page("Page 1");
    let on_dark_page = designos_doc::default_connector_ink(&doc, page);
    assert_eq!(on_dark_page, crate::board::color_of(crate::theme::BOARD_INK_ON_DARK));
    if let Some(n) = doc.get_mut(page) {
        n.kind = NodeKind::Page { background: Color::WHITE };
    }
    assert_eq!(designos_doc::default_connector_ink(&doc, page), crate::board::color_of(crate::theme::BOARD_INK[0]));
}
