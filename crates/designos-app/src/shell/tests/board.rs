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
