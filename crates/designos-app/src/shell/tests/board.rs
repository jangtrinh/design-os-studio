//! UI tests of board (whiteboard) mode: the toolbar, stickies and typing into them, Tab to the next
//! sticky, connectors dragged out of magnet dots and following their nodes, the contextual toolbar,
//! alt-drag, double-click editing, and undo for all of it.

use super::*;
use crate::board::connect;
use crate::canvas::tools::Tool;
use designos_doc::{connector_route, ConnectorCap, ConnectorLine, Magnet, Node, NodeKind, Rect as DRect, ShapeKind, StickyProps, STICKY_SIZE};

fn board_sim() -> Sim {
    Sim::with(Options { fixture: Some("board-empty".into()), ..Default::default() })
}

fn board_demo_sim() -> Sim {
    Sim::with(Options { fixture: Some("board".into()), ..Default::default() })
}

fn stickies(s: &Sim) -> Vec<NodeId> {
    let mut v: Vec<NodeId> = s.app().doc.nodes.values().filter(|n| matches!(n.kind, NodeKind::Sticky(_))).map(|n| n.id).collect();
    v.sort();
    v
}

fn connectors(s: &Sim) -> Vec<NodeId> {
    let mut v: Vec<NodeId> = s.app().doc.nodes.values().filter(|n| matches!(n.kind, NodeKind::Connector(_))).map(|n| n.id).collect();
    v.sort();
    v
}

fn sticky_text(s: &Sim, id: NodeId) -> String {
    match &s.app().doc.get(id).unwrap().kind {
        NodeKind::Sticky(p) => p.text.content.clone(),
        k => panic!("{k:?}"),
    }
}

/// Adds a shape with text on the page at page position `(x, y)`.
fn add_shape(s: &mut Sim, name: &str, x: f32, y: f32) -> NodeId {
    let id = s.shell.app.doc.alloc_id();
    let mut n = Node::shape_with_text(id, DRect::new(x, y, 160.0, 100.0), ShapeKind::RoundedRect, "box");
    n.name = name.into();
    let page = s.app().page;
    let id = s.shell.app.doc.insert(page, n).unwrap();
    s.shell.app.changed();
    s.frame(vec![]);
    id
}

fn node_center(s: &Sim, id: NodeId) -> Pos2 {
    let r = crate::util::world_aabb(&s.app().doc, id);
    s.screen(r.x + r.w / 2.0, r.y + r.h / 2.0)
}

fn dot(s: &Sim, id: NodeId, m: Magnet) -> Pos2 {
    let (_, p) = connect::dot_positions(s.app(), id).into_iter().find(|(k, _)| *k == m).unwrap();
    s.screen(p.x, p.y)
}

#[test]
fn board_files_get_the_board_ui() {
    let s = board_sim();
    assert!(s.app().is_board());
    assert_eq!(s.app().doc.kind, designos_doc::DocumentKind::Board);
    // no right panel: the canvas runs to the window edge
    assert!((s.app().canvas_rect.right() - 1600.0).abs() < 1.0, "canvas right {}", s.app().canvas_rect.right());
    // the board toolbar, with its own tools
    for t in [Tool::Move, Tool::Hand, Tool::Marker, Tool::Sticky, Tool::BoardShape, Tool::Connector, Tool::Text, Tool::Section, Tool::Stamp] {
        let _ = s.rect_of(&format!("board-tool-{t:?}"));
    }
    // a design file keeps its panels and has no board toolbar
    let d = Sim::new();
    assert!(!d.app().is_board());
    assert!(d.app().canvas_rect.right() < 1600.0 - 200.0);
    assert!(d.ctx.data(|dt| dt.get_temp::<Rect>(egui::Id::new(("test-rect", "board-tool-Sticky".to_string())))).is_none());
}

#[test]
fn toolbar_buttons_pick_tools_and_shortcuts_follow_the_board_keys() {
    let mut s = board_sim();
    let r = s.rect_of("board-tool-Sticky");
    s.click(r.center());
    assert_eq!(s.app().tool, Tool::Sticky);
    s.key(Key::Escape);
    assert_eq!(s.app().tool, Tool::Move);
    for (k, shift, t) in [
        (Key::S, false, Tool::Sticky),
        (Key::X, false, Tool::Connector),
        (Key::E, false, Tool::Stamp),
        (Key::R, false, Tool::BoardShape),
        (Key::T, false, Tool::Text),
        (Key::P, false, Tool::Marker),
        (Key::P, true, Tool::Highlighter),
        (Key::S, true, Tool::Section),
        (Key::V, false, Tool::Move),
    ] {
        s.mods = if shift { Modifiers::SHIFT } else { Modifiers::NONE };
        s.key(k);
        s.mods = Modifiers::NONE;
        assert_eq!(s.app().tool, t, "{k:?}");
    }
}

#[test]
fn sticky_tool_places_a_note_and_typing_goes_into_it() {
    let mut s = board_sim();
    s.key(Key::S);
    assert_eq!(s.app().tool, Tool::Sticky);
    let at = s.screen(300.0, 300.0);
    s.click(at);
    let notes = stickies(&s);
    assert_eq!(notes.len(), 1);
    let n = s.app().doc.get(notes[0]).unwrap();
    // centred on the click, default size, colour from the palette's first entry
    assert_eq!(n.size, designos_doc::Vec2::new(STICKY_SIZE, STICKY_SIZE));
    assert!((n.transform.tx + n.size.x / 2.0 - 300.0).abs() < 4.0 && (n.transform.ty + n.size.y / 2.0 - 300.0).abs() < 4.0, "{:?}", n.transform);
    assert!(matches!(&n.kind, NodeKind::Sticky(StickyProps { color, .. }) if *color == designos_doc::STICKY_YELLOW));
    // the tool is spent and typing starts right away
    assert_eq!(s.app().tool, Tool::Move);
    assert!(s.app().canvas.text.is_some(), "typing starts in the new sticky");
    s.type_text("Hello board");
    s.key(Key::Escape);
    assert!(s.app().canvas.text.is_none());
    assert_eq!(sticky_text(&s, notes[0]), "Hello board");
    assert_eq!(s.app().selection, notes);
}

#[test]
fn sticky_creation_and_text_undo() {
    let mut s = board_sim();
    s.key(Key::S);
    let at = s.screen(300.0, 300.0);
    s.click(at);
    s.type_text("abc");
    s.key(Key::Escape);
    let id = stickies(&s)[0];
    assert_eq!(sticky_text(&s, id), "abc");
    // creation and first text are separate steps: undo drops the text, then the note
    s.cmd_key(Key::Z);
    s.cmd_key(Key::Z);
    assert!(stickies(&s).is_empty() || sticky_text(&s, stickies(&s)[0]).is_empty());
    s.cmd_key(Key::Z);
    s.cmd_key(Key::Z);
    assert!(stickies(&s).is_empty(), "everything is undone");
    // and redo brings it all back
    for _ in 0..4 {
        s.mods = Modifiers::COMMAND | Modifiers::SHIFT;
        s.key(Key::Z);
        s.mods = Modifiers::NONE;
    }
    assert_eq!(stickies(&s).len(), 1);
}

#[test]
fn tab_makes_the_next_sticky_to_the_right() {
    let mut s = board_sim();
    s.key(Key::S);
    let at = s.screen(300.0, 300.0);
    s.click(at);
    s.type_text("first");
    s.key(Key::Tab);
    let notes = stickies(&s);
    assert_eq!(notes.len(), 2, "Tab adds a sticky");
    let (a, b) = (s.app().doc.get(notes[0]).unwrap(), s.app().doc.get(notes[1]).unwrap());
    assert_eq!(sticky_text(&s, notes[0]), "first", "the first note kept its text");
    assert!(
        (b.transform.tx - (a.transform.tx + a.size.x + crate::theme::BOARD_STICKY_GAP)).abs() < 1.0,
        "to the right: {} vs {}",
        b.transform.tx,
        a.transform.tx
    );
    assert!((b.transform.ty - a.transform.ty).abs() < 1.0, "same row");
    assert_eq!(a.size, b.size);
    // typing continues in the new note, and Tab again keeps going
    assert_eq!(s.app().canvas.text.as_ref().map(|t| t.id), Some(notes[1]));
    s.type_text("second");
    s.key(Key::Tab);
    s.type_text("third");
    s.key(Key::Escape);
    let notes = stickies(&s);
    assert_eq!(notes.len(), 3);
    assert_eq!(notes.iter().map(|n| sticky_text(&s, *n)).collect::<Vec<_>>(), vec!["first", "second", "third"]);
    let tx: Vec<f32> = notes.iter().map(|n| s.app().doc.get(*n).unwrap().transform.tx).collect();
    assert!(tx[0] < tx[1] && tx[1] < tx[2]);
    // the same colour, picked up from the first note
    let colors: Vec<_> = notes.iter().map(|n| if let NodeKind::Sticky(p) = &s.app().doc.get(*n).unwrap().kind { p.color } else { unreachable!() }).collect();
    assert!(colors.iter().all(|c| *c == colors[0]));
}

#[test]
fn tab_works_on_a_selected_note_too_and_undoes() {
    let mut s = board_demo_sim();
    let a = s.by_name("Sticky 1");
    s.shell.app.selection = vec![a];
    let before = stickies(&s).len();
    s.key(Key::Tab);
    assert_eq!(stickies(&s).len(), before + 1);
    s.key(Key::Escape);
    s.cmd_key(Key::Z);
    assert_eq!(stickies(&s).len(), before, "one undo removes the new note");
}

#[test]
fn double_click_edits_a_sticky() {
    let mut s = board_demo_sim();
    let id = s.by_name("Sticky 3");
    let c = node_center(&s, id);
    s.click(c);
    s.click(c);
    assert_eq!(s.app().canvas.text.as_ref().map(|t| t.id), Some(id), "double-click opens the text");
    s.key(Key::End);
    s.mods = Modifiers::COMMAND;
    s.key(Key::A);
    s.mods = Modifiers::NONE;
    s.type_text("Replaced");
    s.key(Key::Escape);
    assert_eq!(sticky_text(&s, id), "Replaced");
    s.cmd_key(Key::Z);
    assert_eq!(sticky_text(&s, id), "Text shrinks to fit the note");
}

#[test]
fn shapes_are_created_with_text_and_the_shape_picker_changes_what_is_placed() {
    let mut s = board_sim();
    // pick a diamond from the shape popover
    let chev = s.rect_of("board-chevron-Shape");
    s.click(chev.center());
    let cell = s.rect_of("board-shape-Diamond");
    s.click(cell.center());
    assert_eq!(s.app().tool, Tool::BoardShape);
    assert_eq!(s.app().board.shape, ShapeKind::Diamond);
    let at = s.screen(400.0, 300.0);
    s.click(at);
    let id = s.app().selection[0];
    assert!(matches!(&s.app().doc.get(id).unwrap().kind, NodeKind::ShapeWithText(p) if p.shape == ShapeKind::Diamond));
    s.type_text("Decide");
    s.key(Key::Escape);
    assert!(matches!(&s.app().doc.get(id).unwrap().kind, NodeKind::ShapeWithText(p) if p.text.content == "Decide"));
}

#[test]
fn stamp_tool_places_stamps() {
    let mut s = board_sim();
    let chev = s.rect_of("board-chevron-Stamp");
    s.click(chev.center());
    let cell = s.rect_of("board-stamp-heart");
    s.click(cell.center());
    assert_eq!(s.app().tool, Tool::Stamp);
    let at = s.screen(500.0, 400.0);
    s.click(at);
    let id = s.app().selection[0];
    let n = s.app().doc.get(id).unwrap();
    assert_eq!(n.kind, NodeKind::Stamp { kind: "heart".into() });
    assert_eq!(n.size, designos_doc::Vec2::new(64.0, 64.0));
    assert_eq!(s.app().tool, Tool::Move);
}

#[test]
fn hovering_a_shape_shows_four_magnet_dots() {
    let mut s = board_sim();
    let a = add_shape(&mut s, "A", 200.0, 200.0);
    assert_eq!(s.app().board.dots, None);
    let c = node_center(&s, a);
    s.mv(c);
    assert_eq!(s.app().board.dots, Some(a));
    let dots = connect::dot_positions(s.app(), a);
    assert_eq!(dots.len(), 4);
    let sides: Vec<Magnet> = dots.iter().map(|d| d.0).collect();
    assert_eq!(sides, vec![Magnet::Top, Magnet::Right, Magnet::Bottom, Magnet::Left]);
    // the dots sit just outside the sides, so moving onto one keeps them
    let right = dot(&s, a, Magnet::Right);
    s.mv(right);
    assert_eq!(s.app().board.dots, Some(a));
    // far away they go
    let away = s.screen(900.0, 600.0);
    s.mv(away);
    assert_eq!(s.app().board.dots, None);
}

#[test]
fn dragging_from_a_dot_to_another_shape_attaches_a_connector_and_moving_reroutes_it() {
    let mut s = board_sim();
    let a = add_shape(&mut s, "A", 200.0, 200.0);
    let b = add_shape(&mut s, "B", 600.0, 200.0);
    s.mv(node_center(&s, a));
    let from = dot(&s, a, Magnet::Right);
    // drop on the middle of B
    s.drag(from, node_center(&s, b));
    let conns = connectors(&s);
    assert_eq!(conns.len(), 1, "one connector was made");
    let c = conns[0];
    let NodeKind::Connector(p) = &s.app().doc.get(c).unwrap().kind else { unreachable!() };
    assert_eq!(p.start.node, Some(a));
    assert_eq!(p.start.magnet, Magnet::Right);
    assert_eq!(p.end.node, Some(b), "attached to the node it was dropped on");
    assert_eq!(s.app().selection, vec![c]);
    let r = connector_route(&s.app().doc, c).unwrap();
    assert_eq!(r.start(), designos_doc::Vec2::new(360.0, 250.0), "right side of A");
    assert_eq!(r.end(), designos_doc::Vec2::new(600.0, 250.0), "left side of B");

    // move B down by dragging it: the connector re-routes live and after the drop
    s.key(Key::Escape);
    let bc = node_center(&s, b);
    s.drag(bc, bc + Vec2::new(0.0, 160.0 * s.app().view.zoom));
    let nb = s.app().doc.get(b).unwrap();
    assert!(nb.transform.ty > 300.0, "B moved: {}", nb.transform.ty);
    let r2 = connector_route(&s.app().doc, c).unwrap();
    assert!((r2.end().y - (nb.transform.ty + nb.size.y / 2.0)).abs() < 1.0 || r2.end().y > 300.0, "the end follows B: {:?}", r2.end());
    assert_ne!(r2.pts, r.pts, "the route changed");
    let conn = s.app().doc.get(c).unwrap();
    // the connector node covers its new route
    let bb = r2.bounds();
    assert!(
        conn.transform.tx <= bb.x + 0.5
            && conn.transform.ty <= bb.y + 0.5
            && conn.transform.tx + conn.size.x >= bb.x + bb.w - 0.5
            && conn.transform.ty + conn.size.y >= bb.y + bb.h - 0.5
    );

    // undo restores B and the route in one step
    s.cmd_key(Key::Z);
    let r3 = connector_route(&s.app().doc, c).unwrap();
    assert_eq!(r3.pts, r.pts, "undo puts the connector back with its node");
    assert_eq!(s.app().doc.get(b).unwrap().transform.ty, 200.0);
    // undoing again removes the connector
    s.cmd_key(Key::Z);
    assert!(connectors(&s).is_empty());
}

#[test]
fn the_connector_tool_connects_two_notes_and_the_connector_survives_a_move_of_either_end() {
    let mut s = board_demo_sim();
    let a = s.by_name("Sticky 4");
    let b = s.by_name("Release");
    let n0 = connectors(&s).len();
    s.key(Key::X);
    assert_eq!(s.app().tool, Tool::Connector);
    s.drag(node_center(&s, a), node_center(&s, b));
    let conns = connectors(&s);
    assert_eq!(conns.len(), n0 + 1);
    let c = *conns.last().unwrap();
    let NodeKind::Connector(p) = &s.app().doc.get(c).unwrap().kind else { unreachable!() };
    assert_eq!((p.start.node, p.end.node), (Some(a), Some(b)));
    assert_eq!(s.app().tool, Tool::Move, "the tool is spent");
    // move the start note: the line follows it
    let before = connector_route(&s.app().doc, c).unwrap();
    s.key(Key::Escape);
    let ac = node_center(&s, a);
    s.drag(ac, ac + Vec2::new(30.0, 90.0));
    let after = connector_route(&s.app().doc, c).unwrap();
    assert_ne!(before.start(), after.start());
}

#[test]
fn a_click_on_a_dot_without_dragging_makes_no_connector() {
    let mut s = board_sim();
    let a = add_shape(&mut s, "A", 200.0, 200.0);
    s.mv(node_center(&s, a));
    let d = dot(&s, a, Magnet::Bottom);
    s.click(d);
    assert!(connectors(&s).is_empty());
    assert!(matches!(s.app().canvas.inter, crate::canvas::Interaction::None));
}

#[test]
fn dropping_a_connector_on_empty_canvas_leaves_a_free_end() {
    let mut s = board_sim();
    let a = add_shape(&mut s, "A", 200.0, 200.0);
    s.mv(node_center(&s, a));
    let from = dot(&s, a, Magnet::Right);
    s.drag(from, s.screen(700.0, 500.0));
    let c = connectors(&s)[0];
    let NodeKind::Connector(p) = &s.app().doc.get(c).unwrap().kind else { unreachable!() };
    assert_eq!(p.start.node, Some(a));
    assert_eq!(p.end.node, None);
    assert!((p.end.point.x - 700.0).abs() < 3.0 && (p.end.point.y - 500.0).abs() < 3.0, "{:?}", p.end.point);
}

#[test]
fn deleting_a_node_leaves_its_connector_with_a_free_end_and_undo_reattaches() {
    let mut s = board_demo_sim();
    let rel = s.by_name("Release");
    let c = s.by_name("Connector 2");
    let end_before = connector_route(&s.app().doc, c).unwrap().end();
    s.shell.app.selection = vec![rel];
    s.key(Key::Delete);
    assert!(s.app().doc.get(rel).is_none());
    let end_after = connector_route(&s.app().doc, c).unwrap().end();
    assert_eq!(end_after, end_before, "the free end stays where it was");
    s.cmd_key(Key::Z);
    assert!(s.app().doc.get(rel).is_some());
    let NodeKind::Connector(p) = &s.app().doc.get(c).unwrap().kind else { unreachable!() };
    assert_eq!(p.end.node, Some(rel), "undo reconnects it");
}

#[test]
fn alt_drag_duplicates_a_sticky_and_keeps_the_original() {
    let mut s = board_demo_sim();
    let a = s.by_name("Sticky 1");
    let tx = s.app().doc.get(a).unwrap().transform;
    let n0 = stickies(&s).len();
    s.mods = Modifiers::ALT;
    let c = node_center(&s, a);
    s.drag(c, c + Vec2::new(0.0, 300.0 * s.app().view.zoom));
    s.mods = Modifiers::NONE;
    assert_eq!(stickies(&s).len(), n0 + 1);
    assert_eq!(s.app().doc.get(a).unwrap().transform, tx, "the original stays");
    let dup = s.app().selection[0];
    assert_ne!(dup, a);
    let d = s.app().doc.get(dup).unwrap();
    assert!(d.transform.ty > tx.ty + 100.0, "the copy moved");
    assert_eq!(sticky_text(&s, dup), sticky_text(&s, a));
    s.cmd_key(Key::Z);
    assert_eq!(stickies(&s).len(), n0, "one undo takes the copy back");
}

#[test]
fn contextual_bar_recolors_notes_and_undoes() {
    let mut s = board_demo_sim();
    let a = s.by_name("Sticky 1");
    // no selection: no bar
    assert!(s.ctx.data(|d| d.get_temp::<Rect>(egui::Id::new(("test-rect", "board-bar".to_string())))).is_none() || s.app().selection.is_empty());
    s.shell.app.selection = vec![a];
    s.frame(vec![]);
    let bar = s.rect_of("board-bar");
    let sel = crate::util::world_aabb(&s.app().doc, a);
    let top = s.screen(sel.x, sel.y).y;
    assert!(bar.bottom() < top, "the bar floats above the selection");
    let sw = s.rect_of("board-swatch-5");
    s.click(sw.center());
    let color = match &s.app().doc.get(a).unwrap().kind {
        NodeKind::Sticky(p) => crate::board::rgb_of(p.color),
        _ => unreachable!(),
    };
    assert_eq!(color, crate::theme::BOARD_PALETTE[5]);
    assert_eq!(crate::board::rgb_of(s.app().board.color), crate::theme::BOARD_PALETTE[5], "new notes start in the picked colour");
    s.cmd_key(Key::Z);
    let color = match &s.app().doc.get(a).unwrap().kind {
        NodeKind::Sticky(p) => crate::board::rgb_of(p.color),
        _ => unreachable!(),
    };
    assert_eq!(color, crate::theme::BOARD_PALETTE[0]);
}

#[test]
fn contextual_bar_text_size_and_alignment() {
    let mut s = board_demo_sim();
    let a = s.by_name("Sticky 1");
    s.shell.app.selection = vec![a];
    s.frame(vec![]);
    let minus = s.rect_of("board-size-minus");
    s.click(minus.center());
    let size = |s: &Sim| match &s.app().doc.get(a).unwrap().kind {
        NodeKind::Sticky(p) => p.text.style.font_size,
        _ => unreachable!(),
    };
    assert_eq!(size(&s), 24.0, "32 steps down to 24");
    let plus = s.rect_of("board-size-plus");
    s.click(plus.center());
    s.click(plus.center());
    assert_eq!(size(&s), 40.0);
    let center = s.rect_of("board-align-center");
    s.click(center.center());
    assert!(matches!(&s.app().doc.get(a).unwrap().kind, NodeKind::Sticky(p) if p.text.style.align_h == designos_doc::TextAlignH::Center));
}

#[test]
fn contextual_bar_for_connectors_has_line_style_and_caps() {
    let mut s = board_demo_sim();
    let c = s.by_name("Connector 1");
    s.shell.app.selection = vec![c];
    s.frame(vec![]);
    let curved = s.rect_of("board-line-Curved");
    s.click(curved.center());
    assert!(matches!(&s.app().doc.get(c).unwrap().kind, NodeKind::Connector(p) if p.line == ConnectorLine::Curved));
    // the end cap popover
    let cap = s.rect_of("board-cap-end");
    s.click(cap.center());
    let diamond = s.rect_of("board-bar-cap-Diamond");
    s.click(diamond.center());
    assert!(matches!(&s.app().doc.get(c).unwrap().kind, NodeKind::Connector(p) if p.end_cap == ConnectorCap::Diamond));
    let start = s.rect_of("board-cap-start");
    s.click(start.center());
    let circle = s.rect_of("board-bar-cap-Circle");
    s.click(circle.center());
    assert!(matches!(&s.app().doc.get(c).unwrap().kind, NodeKind::Connector(p) if p.start_cap == ConnectorCap::Circle));
    // ink colour
    let sw = s.rect_of("board-swatch-2");
    s.click(sw.center());
    assert_eq!(crate::board::bar::current_color(s.app().doc.get(c).unwrap()), Some(crate::theme::BOARD_INK[2]));
    // each was its own undo step
    s.cmd_key(Key::Z);
    s.cmd_key(Key::Z);
    s.cmd_key(Key::Z);
    s.cmd_key(Key::Z);
    assert!(matches!(&s.app().doc.get(c).unwrap().kind, NodeKind::Connector(p) if p.line == ConnectorLine::Elbowed && p.end_cap == ConnectorCap::Triangle));
}

#[test]
fn contextual_bar_for_shapes_swaps_the_shape() {
    let mut s = board_demo_sim();
    let d = s.by_name("Decision");
    s.shell.app.selection = vec![d];
    s.frame(vec![]);
    let btn = s.rect_of("board-shape-btn");
    s.click(btn.center());
    let star = s.rect_of("board-bar-shape-Star");
    s.click(star.center());
    assert!(matches!(&s.app().doc.get(d).unwrap().kind, NodeKind::ShapeWithText(p) if p.shape == ShapeKind::Star));
    s.cmd_key(Key::Z);
    assert!(matches!(&s.app().doc.get(d).unwrap().kind, NodeKind::ShapeWithText(p) if p.shape == ShapeKind::Diamond));
}

#[test]
fn marker_draws_thick_round_strokes_and_stays_active() {
    let mut s = board_sim();
    s.key(Key::P);
    assert_eq!(s.app().tool, Tool::Marker);
    let (a, b) = (s.screen(300.0, 300.0), s.screen(500.0, 380.0));
    s.drag(a, b);
    assert_eq!(s.app().tool, Tool::Marker, "the marker stays selected for the next stroke");
    let page = s.app().page;
    let kids = s.app().doc.get(page).unwrap().children.clone();
    assert_eq!(kids.len(), 1);
    let n = s.app().doc.get(kids[0]).unwrap();
    assert!(matches!(n.kind, NodeKind::Vector { .. }));
    assert_eq!(n.stroke.weight, crate::theme::BOARD_MARKER_WIDTH);
    assert_eq!(n.stroke.cap, designos_doc::StrokeCap::Round);
    s.cmd_key(Key::Z);
    assert!(s.app().doc.get(page).unwrap().children.is_empty());
}

#[test]
fn highlighter_is_wide_and_translucent() {
    let mut s = board_sim();
    s.mods = Modifiers::SHIFT;
    s.key(Key::P);
    s.mods = Modifiers::NONE;
    assert_eq!(s.app().tool, Tool::Highlighter);
    let (a, b) = (s.screen(300.0, 300.0), s.screen(500.0, 300.0));
    s.drag(a, b);
    let page = s.app().page;
    let id = s.app().doc.get(page).unwrap().children[0];
    let n = s.app().doc.get(id).unwrap();
    assert_eq!(n.stroke.weight, crate::theme::BOARD_HIGHLIGHTER_WIDTH);
    assert!(matches!(&n.stroke.paints[0].kind, designos_doc::PaintKind::Solid(c) if c.a < 0.6));
}

#[test]
fn section_tool_makes_a_section_that_adopts_notes_dropped_into_it() {
    let mut s = board_sim();
    s.mods = Modifiers::SHIFT;
    s.key(Key::S);
    s.mods = Modifiers::NONE;
    assert_eq!(s.app().tool, Tool::Section);
    s.mods = Modifiers::COMMAND;
    s.drag(s.screen(100.0, 100.0), s.screen(800.0, 600.0));
    s.mods = Modifiers::NONE;
    let sec = s.app().selection[0];
    assert!(matches!(s.app().doc.get(sec).unwrap().kind, NodeKind::Section));
    // a sticky placed inside becomes its child
    s.key(Key::S);
    let at = s.screen(300.0, 300.0);
    s.click(at);
    let note = stickies(&s)[0];
    assert_eq!(s.app().doc.get(note).unwrap().parent, Some(sec));
}

#[test]
fn text_tool_in_a_board_makes_larger_text() {
    let mut s = board_sim();
    s.key(Key::T);
    let at = s.screen(300.0, 300.0);
    s.click(at);
    s.type_text("Title");
    s.key(Key::Escape);
    let id = s.app().selection[0];
    match &s.app().doc.get(id).unwrap().kind {
        NodeKind::Text(t) => {
            assert_eq!(t.content, "Title");
            assert_eq!(t.style.font_size, 24.0);
        }
        k => panic!("{k:?}"),
    }
}

#[test]
fn connector_labels_can_be_typed_by_double_clicking_the_line() {
    let mut s = board_demo_sim();
    let c = s.by_name("Connector 1");
    let route = connector_route(&s.app().doc, c).unwrap();
    let mid = route.midpoint();
    let at = s.screen(mid.x, mid.y);
    s.click(at);
    s.click(at);
    assert_eq!(s.app().canvas.text.as_ref().map(|t| t.id), Some(c), "double-click edits the label");
    s.type_text("maybe");
    s.key(Key::Escape);
    assert!(matches!(&s.app().doc.get(c).unwrap().kind, NodeKind::Connector(p) if p.label.as_ref().is_some_and(|l| l.content == "maybe")));
    // emptying a label removes it (a pause first, so these are not counted as more clicks of one multi-click)
    s.t += 1.0;
    s.frame(vec![]);
    s.click(at);
    s.click(at);
    s.mods = Modifiers::COMMAND;
    s.key(Key::A);
    s.mods = Modifiers::NONE;
    s.key(Key::Backspace);
    s.key(Key::Escape);
    assert!(matches!(&s.app().doc.get(c).unwrap().kind, NodeKind::Connector(p) if p.label.is_none()));
}

#[test]
fn a_typed_sticky_text_shrinks_instead_of_overflowing_the_note() {
    let mut s = board_sim();
    s.key(Key::S);
    let at = s.screen(300.0, 300.0);
    s.click(at);
    let id = stickies(&s)[0];
    s.type_text("Word word word word word word word word word word word word word word word word word word word word word word word word");
    let doc = &s.app().doc;
    let f = crate::board::create::text_frame(doc, doc.get(id).unwrap()).unwrap();
    assert!(f.props.style.font_size < designos_doc::STICKY_FONT_MAX, "the drawn size shrank to {}", f.props.style.font_size);
    let h = designos_render::text_geometry(&f.props, f.size).height;
    assert!(h <= f.size.y + 0.5, "the text fits its box: {h} <= {}", f.size.y);
    // while the stored size stays at the maximum, so deleting text grows it back
    assert!(matches!(&doc.get(id).unwrap().kind, NodeKind::Sticky(p) if p.text.style.font_size == designos_doc::STICKY_FONT_MAX));
    s.key(Key::Escape);
}

#[test]
fn board_items_survive_a_copy_paste_with_their_connectors() {
    let mut s = board_demo_sim();
    let a = s.by_name("Decision");
    let b = s.by_name("Release");
    let c = s.by_name("Connector 2");
    s.shell.app.selection = vec![a, b, c];
    let n0 = s.count();
    s.shell.app.duplicate_selection();
    assert_eq!(s.count(), n0 + 3);
    let copies = s.app().selection.clone();
    assert_eq!(copies.len(), 3);
    // the copied connector joins the copied shapes, not the originals
    let cc = copies.iter().copied().find(|i| matches!(s.app().doc.get(*i).unwrap().kind, NodeKind::Connector(_))).unwrap();
    let NodeKind::Connector(p) = &s.app().doc.get(cc).unwrap().kind else { unreachable!() };
    assert!(p.start.node.is_some_and(|n| copies.contains(&n)) && p.end.node.is_some_and(|n| copies.contains(&n)), "{p:?}");
}

#[test]
fn author_line_toggle_on_the_bar() {
    let mut s = board_demo_sim();
    let a = s.by_name("Sticky 1");
    s.shell.app.selection = vec![a];
    s.frame(vec![]);
    let show = |s: &Sim| matches!(&s.app().doc.get(a).unwrap().kind, NodeKind::Sticky(p) if p.show_author);
    assert!(!show(&s));
    let btn = s.rect_of("board-author");
    s.click(btn.center());
    assert!(show(&s));
    assert!(matches!(&s.app().doc.get(a).unwrap().kind, NodeKind::Sticky(p) if p.author.as_deref().is_some_and(|n| !n.is_empty())));
    s.cmd_key(Key::Z);
    assert!(!show(&s));
}

#[test]
fn undo_and_redo_cover_every_board_edit_in_one_history() {
    // creating, typing, moving, connecting, restyling: undo them all, in order, back to the empty board
    let mut s = board_sim();
    let a = add_shape(&mut s, "A", 200.0, 200.0);
    let b = add_shape(&mut s, "B", 600.0, 200.0);
    // shapes were added straight to the document, so history starts empty
    s.mv(node_center(&s, a));
    let from = dot(&s, a, Magnet::Right);
    s.drag(from, node_center(&s, b));
    let c = connectors(&s)[0];
    s.key(Key::Escape);
    let bc = node_center(&s, b);
    s.drag(bc, bc + Vec2::new(0.0, 100.0 * s.app().view.zoom));
    s.shell.app.selection = vec![c];
    s.frame(vec![]);
    let curved = s.rect_of("board-line-Curved");
    s.click(curved.center());
    // restyle, move, connect: three undo steps
    s.cmd_key(Key::Z);
    assert!(matches!(&s.app().doc.get(c).unwrap().kind, NodeKind::Connector(p) if p.line == ConnectorLine::Elbowed));
    s.cmd_key(Key::Z);
    assert_eq!(s.app().doc.get(b).unwrap().transform.ty, 200.0);
    s.cmd_key(Key::Z);
    assert!(connectors(&s).is_empty());
    assert!(!s.app().hist.can_undo());
    // and forward again
    for _ in 0..3 {
        s.mods = Modifiers::COMMAND | Modifiers::SHIFT;
        s.key(Key::Z);
        s.mods = Modifiers::NONE;
    }
    assert_eq!(connectors(&s).len(), 1);
    assert!(matches!(&s.app().doc.get(c).unwrap().kind, NodeKind::Connector(p) if p.line == ConnectorLine::Curved));
    assert!(s.app().doc.get(b).unwrap().transform.ty > 200.0);
}

#[test]
fn connectors_follow_nudges_resizes_and_moves_of_a_whole_section() {
    let mut s = board_demo_sim();
    let rel = s.by_name("Release");
    let c = s.by_name("Connector 2");
    let end0 = connector_route(&s.app().doc, c).unwrap().end();
    // arrow keys nudge the selection, one undo step each
    s.shell.app.selection = vec![rel];
    s.mods = Modifiers::SHIFT;
    s.key(Key::ArrowDown);
    s.mods = Modifiers::NONE;
    let end1 = connector_route(&s.app().doc, c).unwrap().end();
    assert!((end1.y - end0.y - 10.0).abs() < 0.01, "nudged 10 down: {end0:?} -> {end1:?}");
    s.cmd_key(Key::Z);
    assert_eq!(connector_route(&s.app().doc, c).unwrap().end(), end0);

    // resizing from the bottom right handle moves the middle of the left side
    let (corners, _) = crate::canvas::interact::selection_box(s.app()).unwrap();
    let br = s.app().view.to_screen(s.app().canvas_rect.min, corners[2]);
    let h0 = s.app().doc.get(rel).unwrap().size.y;
    s.drag(br, br + Vec2::new(0.0, 60.0 * s.app().view.zoom));
    assert!(s.app().doc.get(rel).unwrap().size.y > h0 + 20.0);
    let end2 = connector_route(&s.app().doc, c).unwrap().end();
    assert!(end2.y > end0.y + 10.0, "the end follows the taller node's middle: {end0:?} -> {end2:?}");
    s.cmd_key(Key::Z);
    assert_eq!(connector_route(&s.app().doc, c).unwrap().end(), end0);

    // moving the whole section carries its notes, and every connector attached to them follows
    let sec = s.by_name("Went well");
    let c1 = s.by_name("Connector 1");
    let start0 = connector_route(&s.app().doc, c1).unwrap().start();
    s.shell.app.selection = vec![sec];
    s.shell.app.nudge(0.0, 40.0);
    s.frame(vec![]);
    let start1 = connector_route(&s.app().doc, c1).unwrap().start();
    assert!((start1.y - start0.y - 40.0).abs() < 0.01, "{start0:?} -> {start1:?}");
    // all three connectors were re-synced to the moved nodes, in the same undo step
    s.cmd_key(Key::Z);
    assert_eq!(connector_route(&s.app().doc, c1).unwrap().start(), start0);
    assert_eq!(connector_route(&s.app().doc, c).unwrap().end(), end0);
}

#[test]
fn moving_a_free_connector_carries_its_loose_ends() {
    let mut s = board_sim();
    let a = add_shape(&mut s, "A", 200.0, 200.0);
    s.mv(node_center(&s, a));
    let from = dot(&s, a, Magnet::Right);
    s.drag(from, s.screen(700.0, 500.0));
    let c = connectors(&s)[0];
    s.key(Key::Escape);
    // a connector with one loose end moves by its loose end; the attached end stays on its node
    let r0 = connector_route(&s.app().doc, c).unwrap();
    s.shell.app.selection = vec![c];
    s.shell.app.nudge(0.0, 30.0);
    let r1 = connector_route(&s.app().doc, c).unwrap();
    assert_eq!(r1.start(), r0.start(), "the end on the shape stays put");
    assert!((r1.end().y - r0.end().y - 30.0).abs() < 0.01, "{:?} -> {:?}", r0.end(), r1.end());
    s.cmd_key(Key::Z);
    assert_eq!(connector_route(&s.app().doc, c).unwrap().end(), r0.end());
}

#[test]
fn facilitator_timer_runs_while_canvas_is_in_use() {
    let mut s = board_sim();
    assert!(!s.app().board.timer.running);
    assert_eq!(s.app().board.timer.format_mm_ss(), "05:00");

    // Open the facilitator timer HUD and start the timer
    let toggle = s.rect_of("board-timer-toggle");
    s.click(toggle.center());
    assert!(s.app().board.timer.open);

    let start_btn = s.rect_of("board-timer-start-pause");
    s.click(start_btn.center());
    assert!(s.app().board.timer.running);
    let rem_after_start = s.app().board.timer.remaining_secs;

    // Use the canvas while the timer is running: place a sticky note and type into it
    s.key(Key::S);
    s.click(s.screen(320.0, 320.0));
    s.type_text("Brainstorm item");
    s.key(Key::Escape);
    assert_eq!(stickies(&s).len(), 1);

    // The timer continued ticking down while the canvas was actively used
    assert!(s.app().board.timer.running);
    assert!(
        s.app().board.timer.remaining_secs < rem_after_start - 0.1,
        "expected timer to advance while canvas in use: before={rem_after_start}, after={}",
        s.app().board.timer.remaining_secs
    );

    // Add 1 minute (+60s) and verify remaining time increases while still running
    let rem_before_add = s.app().board.timer.remaining_secs;
    let add_min = s.rect_of("board-timer-add-min");
    s.click(add_min.center());
    assert!(s.app().board.timer.remaining_secs > rem_before_add + 50.0);

    // Reset stops the timer and restores the configured duration
    let reset_btn = s.rect_of("board-timer-reset");
    s.click(reset_btn.center());
    assert!(!s.app().board.timer.running);
    assert_eq!(s.app().board.timer.remaining_secs, s.app().board.timer.duration_secs);
}

/// Spacing steps of the 4 px grid used for padding and gaps.
const GRID_STEPS: [f32; 9] = [4.0, 8.0, 12.0, 16.0, 24.0, 28.0, 32.0, 40.0, 48.0];

#[test]
fn facilitator_timer_chrome_sits_on_the_four_pixel_grid() {
    let mut s = board_sim();
    let pill = s.rect_of("board-timer-toggle");
    s.click(pill.center());
    assert!(s.app().board.timer.open);
    let bar = crate::board::toolbar::toolbar_rect(s.ctx.content_rect());
    let hud = s.rect_of("board-timer-hud");
    let start = s.rect_of("board-timer-start-pause");
    let add = s.rect_of("board-timer-add-min");
    let reset = s.rect_of("board-timer-reset");

    let spacing = [
        ("toolbar to timer pill", pill.left() - bar.right()),
        ("timer pill to HUD", pill.top() - hud.bottom()),
        ("HUD left padding", start.left() - hud.left()),
        ("HUD top padding", start.top() - hud.top()),
        ("HUD bottom padding", hud.bottom() - start.bottom()),
        ("HUD right padding", hud.right() - reset.right()),
        ("gap between Start and +1 min", add.left() - start.right()),
        ("gap between +1 min and Reset", reset.left() - add.right()),
    ];
    let off_grid: Vec<String> =
        spacing.iter().filter(|(_, v)| !GRID_STEPS.iter().any(|g| (g - v).abs() < 0.01)).map(|(what, v)| format!("{what} is {v} px")).collect();
    assert!(off_grid.is_empty(), "off the 4 px grid {GRID_STEPS:?}: {off_grid:?}");

    for (what, r) in [("timer pill", pill), ("Start", start), ("+1 min", add), ("Reset", reset)] {
        assert!(r.width() >= 24.0 && r.height() >= 24.0, "{what} pointer target {}x{} is below 24 px", r.width(), r.height());
    }
    assert!((pill.height() - bar.height()).abs() < 0.01 && (pill.top() - bar.top()).abs() < 0.01, "pill follows the toolbar row");
}

#[test]
fn facilitator_timer_hud_stays_usable_over_a_selection_bar_pinned_to_the_bottom() {
    let mut s = board_sim();
    let canvas = s.app().canvas_rect;
    let pill = s.rect_of("board-timer-toggle");
    s.click(pill.center());
    assert!(s.app().board.timer.open);
    // a shape as tall as the canvas leaves no room above or below it, so the contextual bar is
    // pinned just over the toolbar row; centre it on the timer HUD
    let origin = s.screen(0.0, 0.0);
    let zoom = s.screen(1.0, 0.0).x - origin.x;
    let (cx, half_w) = (pill.right() - 112.0, 60.0);
    let page = |sx: f32, sy: f32| ((sx - origin.x) / zoom, (sy - origin.y) / zoom);
    let (x0, y0) = page(cx - half_w, canvas.top() + 16.0);
    let (x1, y1) = page(cx + half_w, canvas.bottom() - 16.0);
    let id = s.shell.app.doc.alloc_id();
    let mut n = Node::shape_with_text(id, DRect::new(x0, y0, x1 - x0, y1 - y0), ShapeKind::RoundedRect, "box");
    n.name = "tall".into();
    let page_id = s.app().page;
    s.shell.app.doc.insert(page_id, n).unwrap();
    s.shell.app.changed();
    s.frame(vec![]);
    s.click(Pos2::new(cx, canvas.top() + 200.0));
    let selected = s.app().selection.clone();
    assert_eq!(selected, vec![id], "the tall shape is selected");
    let bar = s.rect_of("board-bar");
    let start = s.rect_of("board-timer-start-pause");
    assert!(bar.contains_rect(start), "premise: the pinned contextual bar {bar:?} covers the HUD button {start:?}");

    // the HUD is drawn and hit above the bar's layer
    let under_bar = s.ctx.layer_id_at(Pos2::new(bar.left() + 10.0, bar.center().y)).expect("the bar's own layer");
    let at_start = s.ctx.layer_id_at(start.center()).expect("the HUD layer");
    assert!(at_start.order > under_bar.order, "HUD layer {at_start:?} must sit above the bar's layer {under_bar:?}");

    let doc_before = s.app().doc.to_json().unwrap();
    s.click(start.center());
    assert!(s.app().board.timer.running, "Start under the contextual bar must still reach the timer");
    assert_eq!(s.app().selection, selected, "the click did not touch the selection");
    assert_eq!(s.app().doc.to_json().unwrap(), doc_before, "the click did not reach the bar's swatches");
}

#[test]
fn tool_popovers_stay_above_the_timer_hud() {
    let mut s = board_sim();
    let pill = s.rect_of("board-timer-toggle");
    s.click(pill.center());
    let hud = s.rect_of("board-timer-hud");
    let chevron = s.rect_of("board-chevron-Stamp");
    s.click(chevron.center());
    assert!(s.app().board.tool_menu.is_some(), "the stamp popover is open");
    let kind = designos_doc::STAMP_KINDS[0];
    let cell = s.rect_of(&format!("board-stamp-{kind}"));
    assert!(hud.intersects(cell), "premise: the HUD {hud:?} sits where the stamp cell {cell:?} is");

    s.click(cell.center());
    assert_eq!(s.app().tool, Tool::Stamp, "the click reached the popover cell under the HUD");
    assert!(s.app().board.tool_menu.is_none());
}

/// How many repaints the board toolbar asked for in an idle frame (the context reports the causes
/// of the previous pass, so a second frame is run before reading them).
fn toolbar_repaint_requests(s: &mut Sim) -> usize {
    s.frame(vec![]);
    s.frame(vec![]);
    s.ctx.repaint_causes().iter().filter(|c| c.file.replace('\\', "/").ends_with("board/toolbar.rs")).count()
}

#[test]
fn facilitator_timer_keeps_requesting_frames_after_start_and_stops_when_paused() {
    let mut s = board_sim();
    let pill = s.rect_of("board-timer-toggle");
    s.click(pill.center());
    s.mv(s.screen(900.0, 400.0));
    assert_eq!(toolbar_repaint_requests(&mut s), 0, "an idle timer at 05:00 asks for no frames");

    // press and release on Start with no frame pumped afterwards, like a user who then holds still
    let start = s.rect_of("board-timer-start-pause");
    s.mv(start.center());
    s.button(true);
    s.button(false);
    assert!(s.app().board.timer.running);
    assert!(s.ctx.has_requested_repaint(), "the frame that handled the Start click schedules the next frame");
    let before = s.app().board.timer.remaining_secs;
    for i in 0..12 {
        assert!(toolbar_repaint_requests(&mut s) > 0, "idle frame {i} after Start asked for no further frame, so the label would freeze");
    }
    assert!(s.app().board.timer.remaining_secs < before - 1.0, "the label advances with no input at all");

    s.click(start.center());
    assert!(!s.app().board.timer.running);
    s.mv(s.screen(900.0, 400.0));
    assert_eq!(toolbar_repaint_requests(&mut s), 0, "a paused timer asks for no frames");
}

#[test]
fn facilitator_timer_stops_asking_for_frames_at_zero() {
    let mut s = board_sim();
    let pill = s.rect_of("board-timer-toggle");
    s.click(pill.center());
    s.shell.app.board.timer.remaining_secs = 0.2;
    let start = s.rect_of("board-timer-start-pause");
    s.mv(start.center());
    s.button(true);
    s.button(false);
    assert!(s.app().board.timer.running);
    for _ in 0..8 {
        s.frame(vec![]);
    }
    assert!(!s.app().board.timer.running);
    assert_eq!(s.app().board.timer.format_mm_ss(), "00:00");
    s.mv(s.screen(900.0, 400.0));
    assert_eq!(toolbar_repaint_requests(&mut s), 0, "a finished timer asks for no frames");
}

#[test]
fn plus_one_minute_extends_the_countdown_but_reset_returns_to_the_preset() {
    let mut s = board_sim();
    let pill = s.rect_of("board-timer-toggle");
    s.click(pill.center());
    let add = s.rect_of("board-timer-add-min");
    s.click(add.center());
    assert_eq!(s.app().board.timer.format_mm_ss(), "06:00");
    let reset = s.rect_of("board-timer-reset");
    s.click(reset.center());
    assert_eq!(s.app().board.timer.format_mm_ss(), "05:00", "Reset restores the preset, not the extended run");
}

#[test]
fn facilitator_timer_label_uses_tabular_digits() {
    fn walk(shape: &egui::Shape, out: &mut Vec<egui::FontFamily>) {
        match shape {
            egui::Shape::Text(t) if t.galley.job.text == "05:00" => {
                out.extend(t.galley.job.sections.iter().map(|sec| sec.format.font_id.family.clone()));
            }
            egui::Shape::Vec(v) => v.iter().for_each(|x| walk(x, out)),
            _ => {}
        }
    }
    let s = board_sim();
    let mut families = Vec::new();
    s.shapes.iter().for_each(|c| walk(&c.shape, &mut families));
    assert!(!families.is_empty(), "the 05:00 label is painted");
    assert!(families.iter().all(|f| *f == egui::FontFamily::Monospace), "digits must not jitter each second: {families:?}");
}

#[test]
fn section_and_text_creation_defaults_across_document_kinds() {
    use designos_doc::{Document, DocumentKind, StrokeAlign};

    for (kind, is_board) in [(DocumentKind::Board, true), (DocumentKind::Design, false), (DocumentKind::Deck, false)] {
        let mut doc = Document::new("Creation defaults");
        doc.kind = kind;
        let page = doc.add_page("Page 1");
        let mut app = App::new(doc);

        let sec = crate::canvas::create::new_shape(&mut app, Tool::Section);
        assert_eq!(sec.kind, NodeKind::Section);
        assert_eq!(sec.size, designos_doc::Vec2::ZERO);
        if is_board {
            assert_eq!(sec.fills, vec![Paint::solid(Color::WHITE)], "{kind:?}: section fill");
            assert_eq!(sec.stroke.paints, vec![Paint::solid(Color::rgb(0xCF, 0xCF, 0xCF))], "{kind:?}: section stroke");
            assert_eq!(sec.stroke.weight, 1.0, "{kind:?}: section stroke weight");
            assert_eq!(sec.stroke.align, StrokeAlign::Inside, "{kind:?}: section stroke align");
        } else {
            assert_eq!(sec.fills, vec![Paint::solid(Color::rgb(0xF5, 0xF5, 0xF5))], "{kind:?}: section fill");
            assert!(sec.stroke.paints.is_empty(), "{kind:?}: section stroke");
        }

        let (txt, _) = crate::canvas::create::new_text(&mut app, page, designos_doc::Vec2::ZERO).expect("new text");
        let NodeKind::Text(tp) = &txt.kind else { panic!("expected text node") };
        let expected_size = if is_board { 24.0 } else { crate::theme::DEFAULT_TEXT_SIZE };
        assert_eq!(tp.style.font_size, expected_size, "{kind:?}: text font size");
    }
}

#[test]
fn dotted_background_paints_on_boards_and_solid_page_fill_on_design_canvas() {
    fn page_bg(s: &Sim) -> egui::Color32 {
        match s.app().doc.get(s.app().page).map(|n| &n.kind) {
            Some(NodeKind::Page { background }) => {
                let f = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
                egui::Color32::from_rgb(f(background.r), f(background.g), f(background.b))
            }
            _ => egui::Color32::from_rgb(0x5A, 0x5A, 0x5A),
        }
    }
    fn page_fill_index(s: &Sim) -> Option<usize> {
        let canvas = s.app().canvas_rect;
        let bg = page_bg(s);
        assert_ne!(bg, crate::theme::BG, "page colour must differ from the window background");
        s.shapes.iter().rposition(|c| match &c.shape {
            egui::Shape::Rect(r) => r.fill == bg && r.rect.contains_rect(canvas.shrink(1.0)),
            _ => false,
        })
    }
    fn dots_index(s: &Sim) -> Option<usize> {
        fn has_board_dots(shape: &egui::Shape) -> bool {
            match shape {
                egui::Shape::Mesh(m) => m.vertices.iter().any(|v| v.color == crate::theme::BOARD_DOT),
                egui::Shape::Vec(v) => v.iter().any(has_board_dots),
                _ => false,
            }
        }
        s.shapes.iter().position(|c| has_board_dots(&c.shape))
    }

    let b = board_sim();
    let dots = dots_index(&b).expect("board canvas paints the dotted background mesh");
    let last_fill = page_fill_index(&b);
    assert!(
        last_fill.is_none_or(|i| i < dots),
        "no rect filled with the page colour and covering the canvas is painted after the board dots: fill {last_fill:?} dots {dots}"
    );

    let d = Sim::new();
    assert!(dots_index(&d).is_none(), "design canvas omits the dotted background mesh");
    assert!(page_fill_index(&d).is_some(), "design canvas paints a rect filled with the page colour covering the canvas");
}

#[test]
fn frame_stepping_shortcut_steps_top_level_frames_only_when_available() {
    let mut d = Sim::new();
    d.shell.app.selection.clear();
    d.frame(vec![]);
    d.key(Key::N);
    let d_page = d.app().page;
    let sel = d.app().selection.clone();
    assert_eq!(sel.len(), 1, "N selects exactly one node when frame stepping is available: {sel:?}");
    let n = d.app().doc.get(sel[0]).expect("selected node exists");
    assert!(matches!(n.kind, NodeKind::Frame(_) | NodeKind::Component(_) | NodeKind::ComponentSet(_)), "N selects a frame-like node: {:?}", n.kind);
    assert!(n.visible, "N selects a visible frame");
    let is_top_level_or_in_top_section = n.parent == Some(d_page)
        || n.parent.and_then(|pid| d.app().doc.get(pid)).is_some_and(|p| p.visible && matches!(p.kind, NodeKind::Section) && p.parent == Some(d_page));
    assert!(is_top_level_or_in_top_section, "selected frame parent must be the page or a top-level Section: {:?}", n.parent);

    let mut b = board_sim();
    let b_page = b.app().page;
    let b_frame_id = b.shell.app.doc.alloc_id();
    let mut b_frame = Node::frame(b_frame_id, DRect::new(100.0, 100.0, 200.0, 150.0));
    b_frame.name = "Board Frame".into();
    b.shell.app.doc.insert(b_page, b_frame).unwrap();
    b.shell.app.selection.clear();
    b.shell.app.changed();
    b.frame(vec![]);
    b.key(Key::N);
    assert!(b.app().selection.is_empty(), "N does not step frames on a board document");
}

#[test]
fn right_click_on_ruler_strip_skips_canvas_menu_only_when_enabled() {
    let mut d = Sim::new();
    d.shell.app.ui.rulers = true;
    d.frame(vec![]);
    let d_strip = Pos2::new(d.app().canvas_rect.left() + 60.0, d.app().canvas_rect.top() + 8.0);
    d.right_click(d_strip);
    assert!(d.app().ui.ctx_menu.is_none(), "Design ruler strip skips the canvas context menu on right-click");

    let mut b = board_sim();
    b.shell.app.ui.rulers = true;
    b.frame(vec![]);
    let b_strip = Pos2::new(b.app().canvas_rect.left() + 60.0, b.app().canvas_rect.top() + 8.0);
    b.right_click(b_strip);
    assert_eq!(b.app().ui.ctx_menu, Some(b_strip), "Board ruler strip does not suppress the canvas context menu on right-click");
}

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
    s.shell.app.view.offset = designos_doc::Vec2::ZERO;
    s.shell.app.changed();
    s.frame(vec![]);
    s
}

fn add_design_frame(s: &mut Sim, parent: NodeId, name: &str, x: f32, y: f32, w: f32, h: f32) -> NodeId {
    let id = s.shell.app.doc.alloc_id();
    let mut n = Node::frame(id, DRect::new(x, y, w, h));
    n.name = name.into();
    s.shell.app.doc.insert(parent, n).unwrap();
    s.shell.app.changed();
    s.frame(vec![]);
    id
}

fn add_design_rect(s: &mut Sim, parent: NodeId, name: &str, x: f32, y: f32, w: f32, h: f32) -> NodeId {
    let id = s.shell.app.doc.alloc_id();
    let mut n = Node::rectangle(id, DRect::new(x, y, w, h));
    n.name = name.into();
    s.shell.app.doc.insert(parent, n).unwrap();
    s.shell.app.changed();
    s.frame(vec![]);
    id
}

#[test]
fn connector_tool_attaches_frame_to_frame_in_a_design_file() {
    let mut s = design_sim();
    let page = s.app().page;
    let a = add_design_frame(&mut s, page, "Frame A", 120.0, 180.0, 160.0, 100.0);
    let b = add_design_frame(&mut s, page, "Frame B", 520.0, 180.0, 160.0, 100.0);
    s.shell.app.set_tool(Tool::Connector);
    s.drag(node_center(&s, a), node_center(&s, b));

    let conns = connectors(&s);
    assert_eq!(conns.len(), 1, "connector created between frames in a design file");
    let c = conns[0];
    let NodeKind::Connector(p) = &s.app().doc.get(c).unwrap().kind else { unreachable!() };
    assert_eq!(p.start.node, Some(a));
    assert_eq!(p.end.node, Some(b));

    let before = connector_route(&s.app().doc, c).unwrap();
    s.key(Key::Escape);
    let ac = node_center(&s, a);
    s.drag(ac, ac + Vec2::new(0.0, 120.0));
    let after = connector_route(&s.app().doc, c).unwrap();
    assert_ne!(before.start(), after.start(), "moving frame A reroutes the attached connector");
}

#[test]
fn dot_press_starts_connector_in_a_design_file() {
    let mut s = design_sim();
    let page = s.app().page;
    let a = add_design_frame(&mut s, page, "Frame A", 120.0, 180.0, 160.0, 100.0);
    let b = add_design_frame(&mut s, page, "Frame B", 520.0, 180.0, 160.0, 100.0);
    s.click(node_center(&s, a));
    assert_eq!(s.app().selection, vec![a]);
    assert_eq!(s.app().board.dots, Some(a), "selected design frame shows magnet dots under Move tool");

    let from = dot(&s, a, Magnet::Right);
    s.drag(from, node_center(&s, b));
    let conns = connectors(&s);
    assert_eq!(conns.len(), 1);
    let c = conns[0];
    let NodeKind::Connector(p) = &s.app().doc.get(c).unwrap().kind else { unreachable!() };
    assert_eq!((p.start.node, p.start.magnet), (Some(a), Magnet::Right));
    assert_eq!(p.end.node, Some(b));
}

#[test]
fn dots_show_on_selected_design_layer_and_on_whiteboard_items_only() {
    let mut s = design_sim();
    let page = s.app().page;
    let r = add_design_rect(&mut s, page, "Rect", 120.0, 180.0, 120.0, 80.0);
    let sticky_id = s.shell.app.doc.alloc_id();
    s.shell.app.doc.insert(page, Node::sticky(sticky_id, DRect::new(520.0, 180.0, 160.0, 160.0), "Note")).unwrap();
    s.shell.app.changed();
    s.frame(vec![]);

    // Move tool: hover unselected rectangle -> dots == None
    s.mv(node_center(&s, r));
    assert_eq!(s.app().board.dots, None, "Move tool must not show dots on an unselected design layer");

    // Select it -> dots == Some(r)
    s.click(node_center(&s, r));
    assert_eq!(s.app().board.dots, Some(r), "Move tool shows dots on the single selected design layer");

    // Hover sticky -> dots == Some(sticky)
    s.mv(node_center(&s, sticky_id));
    assert_eq!(s.app().board.dots, Some(sticky_id), "Move tool shows dots when hovering a whiteboard item");

    // Connector tool: hover unselected rectangle -> dots == Some(r)
    s.key(Key::Escape);
    s.shell.app.set_tool(Tool::Connector);
    s.mv(node_center(&s, r));
    assert_eq!(s.app().board.dots, Some(r), "Connector tool shows dots on any hovered connectable layer");
}

#[test]
fn dots_use_cached_hover_and_do_not_mark_document_changed() {
    let mut s = design_sim();
    let page = s.app().page;
    let a = add_design_rect(&mut s, page, "A", 120.0, 180.0, 120.0, 80.0);
    let b = add_design_rect(&mut s, page, "B", 520.0, 180.0, 120.0, 80.0);
    s.shell.app.set_tool(Tool::Connector);
    s.frame(vec![]);
    s.shell.app.canvas.dirty = false;
    let rev0 = s.app().canvas.tiles.rev();

    // Cached hover query holds `a` while the page point passed to `update_hover` sits over `b`.
    s.shell.app.hover = Some(a);
    connect::update_hover(&mut s.shell.app, Some(designos_doc::Vec2::new(580.0, 220.0)));
    assert_eq!(s.app().board.dots, Some(a), "dots must come from cached app.hover, not a second hit_under");
    assert!(!s.app().canvas.dirty, "update_hover must not mark the document dirty");
    assert_eq!(s.app().canvas.tiles.rev(), rev0, "update_hover must not bump TileView::rev");

    // Pointer hover across targets updates dots from app.hover without dirtying the canvas or bumping TileView::rev.
    s.mv(node_center(&s, b));
    s.shell.app.canvas.dirty = false;
    let rev1 = s.app().canvas.tiles.rev();
    s.mv(node_center(&s, a));
    assert_eq!(s.app().hover, Some(a));
    assert_eq!(s.app().board.dots, Some(a));
    assert!(!s.app().canvas.dirty);
    assert_eq!(s.app().canvas.tiles.rev(), rev1);
}

#[test]
fn cmd_picks_deepest_layer_for_drop_target() {
    let mut s = design_sim();
    let page = s.app().page;
    let src = add_design_frame(&mut s, page, "Source", 80.0, 180.0, 120.0, 80.0);
    let f = add_design_frame(&mut s, page, "F", 440.0, 120.0, 260.0, 220.0);
    let g = add_design_frame(&mut s, f, "G", 20.0, 20.0, 220.0, 180.0);
    let btn = add_design_rect(&mut s, g, "B", 30.0, 30.0, 120.0, 48.0);

    // Plain drop over B attaches to nested frame G (what a plain Move click selects).
    s.shell.app.set_tool(Tool::Connector);
    s.drag(node_center(&s, src), node_center(&s, btn));
    let c1 = *connectors(&s).last().expect("plain connector created");
    let NodeKind::Connector(p1) = &s.app().doc.get(c1).unwrap().kind else { unreachable!() };
    assert_eq!(p1.end.node, Some(g), "plain drop over B inside nested frame G attaches to G");

    // Cmd-drop over B attaches to deepest layer B.
    s.shell.app.set_tool(Tool::Connector);
    let a_pos = node_center(&s, src);
    let b_pos = node_center(&s, btn);
    s.mv(a_pos);
    s.button(true);
    let is_mac = cfg!(target_os = "macos");
    s.mods = Modifiers { alt: false, ctrl: !is_mac, shift: false, mac_cmd: is_mac, command: true };
    s.mv(b_pos);
    s.button(false);
    s.mods = Modifiers::NONE;
    s.frame(vec![]);

    let conns = connectors(&s);
    assert_eq!(conns.len(), 2);
    let c2 = conns[1];
    let NodeKind::Connector(p2) = &s.app().doc.get(c2).unwrap().kind else { unreachable!() };
    assert_eq!(p2.end.node, Some(btn), "Cmd-drop over B attaches to deepest layer B");
}

#[test]
fn alt_drag_ignores_magnets() {
    let mut s = design_sim();
    let page = s.app().page;
    let a = add_design_frame(&mut s, page, "A", 120.0, 180.0, 140.0, 90.0);
    let b = add_design_frame(&mut s, page, "B", 480.0, 180.0, 160.0, 100.0);
    s.shell.app.set_tool(Tool::Connector);

    let a_pos = node_center(&s, a);
    let b_pos = node_center(&s, b);
    let b_page = s.app().view.to_page(s.app().canvas_rect.min, b_pos);
    s.mv(a_pos);
    s.button(true);
    s.mods = Modifiers { alt: true, ..Modifiers::NONE };
    s.mv(b_pos);
    s.button(false);
    s.mods = Modifiers::NONE;
    s.frame(vec![]);

    let conns = connectors(&s);
    assert_eq!(conns.len(), 1);
    let c = conns[0];
    let NodeKind::Connector(p) = &s.app().doc.get(c).unwrap().kind else { unreachable!() };
    assert_eq!(p.start.node, Some(a));
    assert_eq!(p.end.node, None, "Alt-drag leaves a free end without snapping to B");
    assert!(
        (p.end.point.x - b_page.x).abs() < 1.0 && (p.end.point.y - b_page.y).abs() < 1.0,
        "end.point is the release point: {:?} vs {:?}",
        p.end.point,
        b_page
    );
}

#[test]
fn connector_in_auto_layout_frame_is_absolute_and_ignored_by_layout() {
    let mut s = design_sim();
    let page = s.app().page;
    let f = add_design_frame(&mut s, page, "AL Frame", 120.0, 140.0, 320.0, 120.0);
    if let Some(fp) = s.shell.app.doc.get_mut(f).unwrap().kind.frame_props_mut() {
        fp.auto_layout = Some(designos_doc::AutoLayout {
            direction: designos_doc::LayoutDirection::Horizontal,
            padding: [16.0, 16.0, 16.0, 16.0],
            item_spacing: 40.0,
            primary_sizing: designos_doc::Sizing::Hug,
            counter_sizing: designos_doc::Sizing::Hug,
            ..Default::default()
        });
    }
    let a = add_design_rect(&mut s, f, "Child A", 16.0, 16.0, 80.0, 48.0);
    let b = add_design_rect(&mut s, f, "Child B", 136.0, 16.0, 80.0, 48.0);
    let mut ed = crate::edit::Edit::default();
    crate::layout::layout_node(&mut s.shell.app.doc, &mut ed, f);
    s.shell.app.changed();
    s.frame(vec![]);

    let a_t0 = s.app().doc.get(a).unwrap().transform;
    let b_t0 = s.app().doc.get(b).unwrap().transform;
    let f_size0 = s.app().doc.get(f).unwrap().size;

    s.shell.app.set_tool(Tool::Connector);
    s.drag(node_center(&s, a), node_center(&s, b));

    let conns = connectors(&s);
    assert_eq!(conns.len(), 1);
    let c = conns[0];
    let cn = s.app().doc.get(c).unwrap();
    assert_eq!(cn.parent, Some(f), "connector between siblings inside F lives in F");
    assert!(cn.layout_child.absolute, "connector inside auto-layout frame has layout_child.absolute == true");

    let mut ed2 = crate::edit::Edit::default();
    crate::layout::layout_node(&mut s.shell.app.doc, &mut ed2, f);
    assert_eq!(s.app().doc.get(a).unwrap().transform, a_t0, "child A position unchanged by connector");
    assert_eq!(s.app().doc.get(b).unwrap().transform, b_t0, "child B position unchanged by connector");
    assert_eq!(s.app().doc.get(f).unwrap().size, f_size0, "hug auto-layout frame size unchanged by connector");
}

#[test]
fn load_repair_sets_absolute_on_connectors_in_auto_layout_parents() {
    let mut doc = designos_doc::Document::new("RepairALConnector");
    let page = doc.add_page("Page 1");
    let f = doc.alloc_id();
    let mut fn_node = Node::frame(f, DRect::new(0.0, 0.0, 300.0, 120.0));
    if let Some(fp) = fn_node.kind.frame_props_mut() {
        fp.auto_layout = Some(designos_doc::AutoLayout::default());
    }
    doc.insert(page, fn_node).unwrap();
    let a = doc.alloc_id();
    doc.insert(f, Node::rectangle(a, DRect::new(10.0, 10.0, 60.0, 40.0))).unwrap();
    let b = doc.alloc_id();
    doc.insert(f, Node::rectangle(b, DRect::new(150.0, 10.0, 60.0, 40.0))).unwrap();
    let c = doc.alloc_id();
    let mut conn = Node::connector(
        c,
        designos_doc::ConnectorProps {
            start: designos_doc::ConnectorEnd::attached(a, Magnet::Auto),
            end: designos_doc::ConnectorEnd::attached(b, Magnet::Auto),
            ..Default::default()
        },
    );
    conn.layout_child.absolute = false;
    doc.insert(f, conn).unwrap();

    let json = doc.to_json().unwrap();
    let loaded = designos_doc::Document::from_json(&json).unwrap();
    assert!(loaded.get(c).unwrap().layout_child.absolute, "load repair must set layout_child.absolute = true on connectors in auto-layout parents");
}

#[test]
fn reparenting_an_end_node_rehomes_the_connector_in_the_same_undo_step() {
    let mut s = design_sim();
    let page = s.app().page;
    let f = add_design_frame(&mut s, page, "F", 100.0, 120.0, 460.0, 220.0);
    let a = add_design_rect(&mut s, f, "A", 30.0, 50.0, 100.0, 60.0);
    let b = add_design_rect(&mut s, f, "B", 280.0, 50.0, 100.0, 60.0);

    s.shell.app.set_tool(Tool::Connector);
    s.drag(node_center(&s, a), node_center(&s, b));
    let c = connectors(&s)[0];
    assert_eq!(s.app().doc.get(c).unwrap().parent, Some(f));
    let end_points = |s: &Sim| match &s.app().doc.get(c).unwrap().kind {
        NodeKind::Connector(p) => (p.start.point, p.end.point),
        k => panic!("{k:?}"),
    };
    let points_in_f = end_points(&s);

    let page_len = s.app().doc.get(page).unwrap().children.len();
    let mut tx = crate::edit::Tx::new();
    tx.cmd(&mut s.shell.app.doc, designos_doc::Command::Reparent { id: b, parent: page, index: page_len }).unwrap();
    s.shell.app.run_tx(tx);
    s.frame(vec![]);
    assert_eq!(s.app().doc.get(b).unwrap().parent, Some(page));
    assert_eq!(s.app().doc.get(c).unwrap().parent, Some(page), "reparenting B to page rehomes connector C to page in the same commit");

    // The end points are in the page's space now: they match the route and each sits on the edge of its node.
    let geometry = designos_doc::connector_geometry(&s.app().doc, c).unwrap();
    let (start, end) = end_points(&s);
    assert_eq!((start, end), (geometry.start_point, geometry.end_point), "the stored end points are the route's, resolved in the new parent");
    for (point, node) in [(start, a), (end, b)] {
        let r = crate::util::world_aabb(&s.app().doc, node);
        let on_edge = ((point.x - r.x).abs() < 0.5 || (point.x - (r.x + r.w)).abs() < 0.5) && point.y >= r.y - 0.5 && point.y <= r.y + r.h + 0.5
            || ((point.y - r.y).abs() < 0.5 || (point.y - (r.y + r.h)).abs() < 0.5) && point.x >= r.x - 0.5 && point.x <= r.x + r.w + 0.5;
        assert!(on_edge, "end point {point:?} sits on the edge of its node {r:?}");
    }
    assert_ne!((start, end), points_in_f, "the points moved with the parent space");

    // One undo puts both B and C back in F, and the end points back where they were.
    s.shell.app.undo();
    s.frame(vec![]);
    assert_eq!(s.app().doc.get(b).unwrap().parent, Some(f), "one undo puts B back in F");
    assert_eq!(s.app().doc.get(c).unwrap().parent, Some(f), "one undo puts C back in F");
    assert_eq!(end_points(&s), points_in_f, "one undo restores the end points");
}

#[test]
fn moving_an_end_node_to_another_page_detaches_that_end_and_keeps_the_connector() {
    let mut s = design_sim();
    let page1 = s.app().page;
    let a = add_design_frame(&mut s, page1, "A", 120.0, 180.0, 140.0, 90.0);
    let b = add_design_frame(&mut s, page1, "B", 480.0, 180.0, 140.0, 90.0);

    s.shell.app.set_tool(Tool::Connector);
    s.drag(node_center(&s, a), node_center(&s, b));
    let c = connectors(&s)[0];
    let last_end_point = match &s.app().doc.get(c).unwrap().kind {
        NodeKind::Connector(p) => p.end.point,
        _ => unreachable!(),
    };

    let page2 = s.shell.app.doc.add_page("Page 2");
    s.shell.app.toast = None;
    let mut tx = crate::edit::Tx::new();
    tx.cmd(&mut s.shell.app.doc, designos_doc::Command::Reparent { id: b, parent: page2, index: 0 }).unwrap();
    s.shell.app.run_tx(tx);
    s.frame(vec![]);

    assert_eq!(s.app().doc.page_of(c), Some(page1), "connector stays on page 1");
    let NodeKind::Connector(p) = &s.app().doc.get(c).unwrap().kind else { unreachable!() };
    assert_eq!(p.start.node, Some(a));
    assert_eq!(p.end.node, None, "end attached to B detaches when B moves to another page");
    assert!((p.end.point.x - last_end_point.x).abs() < 0.5 && (p.end.point.y - last_end_point.y).abs() < 0.5, "end.point is B's last anchor point");
    assert!(s.app().toast.is_some(), "detaching an end on cross-page move shows a toast");
}

#[test]
fn hidden_target_keeps_connector_attached_and_visible() {
    let mut s = design_sim();
    let page = s.app().page;
    let a = add_design_frame(&mut s, page, "A", 120.0, 180.0, 140.0, 90.0);
    let b = add_design_frame(&mut s, page, "B", 480.0, 180.0, 140.0, 90.0);

    s.shell.app.set_tool(Tool::Connector);
    s.drag(node_center(&s, a), node_center(&s, b));
    let c = connectors(&s)[0];
    let route_before = connector_route(&s.app().doc, c).unwrap();

    s.shell.app.toggle_visible(b);
    s.frame(vec![]);
    assert!(!s.app().doc.get(b).unwrap().visible);
    let cn = s.app().doc.get(c).unwrap();
    assert!(cn.visible, "connector stays visible when target B is hidden");
    let NodeKind::Connector(p) = &cn.kind else { unreachable!() };
    assert_eq!(p.end.node, Some(b), "connector stays attached to hidden target B");
    let route_after = connector_route(&s.app().doc, c).unwrap();
    assert_eq!(route_after.end(), route_before.end(), "connector keeps routing to hidden B's geometry");
}

#[test]
fn magnet_hit_radius_is_a_24px_target() {
    assert!(std::hint::black_box(crate::theme::BOARD_MAGNET_HIT) >= 12.0, "BOARD_MAGNET_HIT must be at least 12.0 px (24 px target)");
    let mut s = design_sim();
    let page = s.app().page;
    let a = add_design_frame(&mut s, page, "A", 200.0, 200.0, 160.0, 100.0);
    s.click(node_center(&s, a));
    assert_eq!(s.app().board.dots, Some(a));

    let right = dot(&s, a, Magnet::Right);
    let rect = s.app().canvas_rect;
    assert_eq!(connect::dot_hit(s.app(), rect, right + Vec2::new(11.5, 0.0)), Some((a, Magnet::Right)), "hits at 11.5 px from dot centre");
    assert_eq!(connect::dot_hit(s.app(), rect, right + Vec2::new(12.5, 0.0)), None, "misses at 12.5 px from dot centre");
}

#[test]
fn x_key_selects_connector_in_a_design_file() {
    let mut s = design_sim();
    assert_eq!(s.app().tool, Tool::Move);
    s.key(Key::X);
    assert_eq!(s.app().tool, Tool::Connector, "pressing X on a design canvas selects Tool::Connector");
}

#[test]
fn x_over_align_row_toggles_spacing_not_tool() {
    let mut s = design_sim();
    let page = s.app().page;
    let f = add_design_frame(&mut s, page, "AL", 200.0, 200.0, 240.0, 120.0);
    if let Some(fp) = s.shell.app.doc.get_mut(f).unwrap().kind.frame_props_mut() {
        fp.auto_layout = Some(designos_doc::AutoLayout::default());
    }
    s.shell.app.select_only(f);
    s.shell.app.changed();
    s.frame(vec![]);
    s.frame(vec![]);

    let ph = s.field_rect("al-ph");
    let align_dy =
        (crate::theme::FIELD_HEIGHT + 4.0) + (crate::theme::SPACE_GROUP - crate::theme::SPACE_ROW) + crate::theme::LABEL_BOX_H + crate::theme::SPACE_FACT;
    let align_pos = Pos2::new(ph.center().x, ph.center().y + align_dy);
    assert_eq!(s.app().tool, Tool::Move);
    s.mv(align_pos);
    s.key(Key::X);
    let al = s.app().doc.get(f).unwrap().kind.frame_props().unwrap().auto_layout.as_ref().unwrap();
    assert_eq!(al.primary_align, designos_doc::LayoutAlign::SpaceBetween, "X over align row toggles auto-spacing");
    assert_eq!(s.app().tool, Tool::Move, "X over align row must not switch tool to Connector");

    // Moving pointer back to canvas and pressing X selects Tool::Connector.
    s.mv(s.app().canvas_rect.center());
    s.key(Key::X);
    assert_eq!(s.app().tool, Tool::Connector);
}

#[test]
fn shift_l_selects_arrow_and_x_selects_connector() {
    let mut s = design_sim();
    s.mods = Modifiers { shift: true, ..Modifiers::NONE };
    s.key(Key::L);
    s.mods = Modifiers::NONE;
    assert_eq!(s.app().tool, Tool::Arrow, "Shift+L selects Arrow");

    s.key(Key::X);
    assert_eq!(s.app().tool, Tool::Connector, "plain X selects Connector");
    assert_ne!(Tool::Arrow, Tool::Connector);
}

#[test]
fn toolbar_stays_512px_wide() {
    assert_eq!(crate::panels::toolbar::toolbar_width(), 512.0, "toolbelt width stays 512 px");
    assert!(crate::canvas::tools::GROUPS[2].contains(&Tool::Connector), "shape group contains Tool::Connector");
    assert_eq!(Tool::Connector.label(), "Connector");
    assert_eq!(Tool::Connector.shortcut(), "X");

    let mut s = design_sim();
    let screen = s.ctx.content_rect();
    assert!(crate::panels::menus::preset_menu(&s.ctx, &mut s.shell.app, "shape", screen));
    s.frame(vec![]);
    assert!(
        crate::panels::menus::palette_entries(s.app(), "connector").iter().any(|(label, sc)| label == "Connector" && sc == "X"),
        "palette/menu lists Connector with shortcut X"
    );
}

#[test]
fn default_ink_has_3_to_1_contrast_on_default_page() {
    let mut s = design_sim();
    let page = s.app().page;
    let a = add_design_frame(&mut s, page, "A", 120.0, 120.0, 120.0, 80.0);
    let b = add_design_frame(&mut s, page, "B", 380.0, 120.0, 120.0, 80.0);
    s.shell.app.set_tool(Tool::Connector);
    s.drag(node_center(&s, a), node_center(&s, b));
    let c_page = connectors(&s)[0];
    let col_page = match &s.app().doc.get(c_page).unwrap().stroke.paints[0].kind {
        PaintKind::Solid(c) => *c,
        _ => panic!("expected solid stroke"),
    };
    let page_bg = Color::rgb(0x5A, 0x5A, 0x5A);
    assert_eq!(col_page, Color::WHITE, "default #5A5A5A page picks white connector ink");
    assert!(designos_doc::lint::rules::contrast::contrast_ratio(col_page, page_bg) >= 3.0);

    let wf = add_design_frame(&mut s, page, "WhiteFrame", 120.0, 320.0, 420.0, 200.0);
    s.shell.app.doc.get_mut(wf).unwrap().fills = vec![Paint::solid(Color::WHITE)];
    let ca = add_design_rect(&mut s, wf, "CA", 20.0, 40.0, 100.0, 60.0);
    let cb = add_design_rect(&mut s, wf, "CB", 260.0, 40.0, 100.0, 60.0);
    s.shell.app.set_tool(Tool::Connector);
    s.drag(node_center(&s, ca), node_center(&s, cb));
    let c_white = *connectors(&s).last().unwrap();
    let col_white = match &s.app().doc.get(c_white).unwrap().stroke.paints[0].kind {
        PaintKind::Solid(c) => *c,
        _ => panic!("expected solid stroke"),
    };
    assert_eq!(col_white, Color::rgb(0x1E, 0x1E, 0x1E), "white frame picks #1E1E1E connector ink");
    assert!(designos_doc::lint::rules::contrast::contrast_ratio(col_white, Color::WHITE) >= 3.0);
}

#[test]
fn board_connector_keeps_the_toolbar_ink() {
    let mut b = board_sim();
    let picked = Color::rgb(0xF2, 0x48, 0x22);
    b.shell.app.board.ink = picked;
    let a = add_shape(&mut b, "A", 200.0, 200.0);
    let c_target = add_shape(&mut b, "B", 560.0, 200.0);
    b.shell.app.set_tool(Tool::Connector);
    b.drag(node_center(&b, a), node_center(&b, c_target));
    let cid = *connectors(&b).last().unwrap();
    let stroke_col = match &b.app().doc.get(cid).unwrap().stroke.paints[0].kind {
        PaintKind::Solid(col) => *col,
        _ => panic!("expected solid stroke"),
    };
    assert_eq!(stroke_col, picked, "board connector keeps the picked board toolbar ink #F24822");

    // In a design file on the default #5A5A5A page, default ink is contrast-checked white, not app.board.ink.
    let mut d = design_sim();
    let page = d.app().page;
    let da = add_design_frame(&mut d, page, "DA", 120.0, 180.0, 120.0, 80.0);
    let db = add_design_frame(&mut d, page, "DB", 420.0, 180.0, 120.0, 80.0);
    d.shell.app.set_tool(Tool::Connector);
    d.drag(node_center(&d, da), node_center(&d, db));
    let dcid = *connectors(&d).last().unwrap();
    let d_col = match &d.app().doc.get(dcid).unwrap().stroke.paints[0].kind {
        PaintKind::Solid(col) => *col,
        _ => panic!("expected solid stroke"),
    };
    assert_eq!(d_col, Color::WHITE);
}

#[test]
fn board_file_behaviour_unchanged_for_connectors() {
    let mut b = board_sim();
    assert!(b.app().is_board());
    assert!(b.app().wb_connectors_enabled());
    let a = add_shape(&mut b, "A", 200.0, 200.0);
    let target = add_shape(&mut b, "B", 600.0, 200.0);
    b.mv(node_center(&b, a));
    assert_eq!(b.app().board.dots, Some(a), "unselected whiteboard shape still shows magnet dots on hover");
    b.drag(dot(&b, a, Magnet::Right), node_center(&b, target));
    let cid = *connectors(&b).last().unwrap();
    let NodeKind::Connector(p) = &b.app().doc.get(cid).unwrap().kind else { unreachable!() };
    assert_eq!((p.start.node, p.start.magnet, p.end.node), (Some(a), Magnet::Right, Some(target)));

    let d = design_sim();
    assert!(!d.app().is_board());
    assert!(d.app().wb_connectors_enabled(), "Design documents also enable live connectors");
}

#[test]
fn svg_of_a_design_frame_includes_inner_connectors() {
    let mut s = design_sim();
    let page = s.app().page;
    let f = add_design_frame(&mut s, page, "Container", 100.0, 120.0, 500.0, 240.0);
    let a = add_design_rect(&mut s, f, "A", 30.0, 60.0, 100.0, 60.0);
    let b = add_design_rect(&mut s, f, "B", 340.0, 60.0, 100.0, 60.0);

    s.shell.app.set_tool(Tool::Connector);
    s.drag(node_center(&s, a), node_center(&s, b));
    let c = connectors(&s)[0];
    assert_eq!(s.app().doc.get(c).unwrap().parent, Some(f), "inner connector is parented to frame F");

    let svg = designos_export::export_svg(&s.app().doc, f, &designos_export::SvgOptions::default());
    assert!(svg.matches("<path ").count() >= 2, "exported SVG of frame F must include the connector path and arrowhead: {svg}");
}

#[test]
fn connector_label_edits_in_a_design_file() {
    let mut s = design_sim();
    let page = s.app().page;
    let a = add_design_frame(&mut s, page, "A", 120.0, 180.0, 140.0, 90.0);
    let b = add_design_frame(&mut s, page, "B", 520.0, 180.0, 140.0, 90.0);

    s.shell.app.set_tool(Tool::Connector);
    s.drag(node_center(&s, a), node_center(&s, b));
    let c = connectors(&s)[0];
    let NodeKind::Connector(p0) = &s.app().doc.get(c).unwrap().kind else { unreachable!() };
    assert_eq!((p0.start.node, p0.end.node), (Some(a), Some(b)), "connector attaches to design frames A and B");
    let mid = connector_route(&s.app().doc, c).unwrap().midpoint();
    let mid_screen = s.screen(mid.x, mid.y);
    s.click(mid_screen);
    s.click(mid_screen);
    assert_eq!(s.app().canvas.text.as_ref().map(|t| t.id), Some(c), "double-clicking connector opens label text session");
    s.type_text("Yes");
    s.key(Key::Escape);
    let NodeKind::Connector(p) = &s.app().doc.get(c).unwrap().kind else { unreachable!() };
    assert_eq!(p.label.as_ref().map(|l| l.content.as_str()), Some("Yes"));
}
