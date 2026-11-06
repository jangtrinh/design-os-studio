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

    // and the other way round: a dark screen above a fill-less wrapper gives white ink
    let night_fill = Color::rgb(0x12, 0x12, 0x15);
    let night = add(
        &mut s,
        page,
        |id| {
            let mut n = Node::frame(id, DRect::new(80.0, 480.0, 640.0, 360.0));
            n.fills = vec![Paint::solid(night_fill)];
            n
        },
        "Night screen",
    );
    let night_wrapper = add(
        &mut s,
        night,
        |id| {
            let mut n = Node::frame(id, DRect::new(20.0, 20.0, 600.0, 320.0));
            n.fills.clear();
            n
        },
        "Night wrapper",
    );
    let a2 = rectangle(&mut s, night_wrapper, "Night card A", 40.0, 100.0);
    let b2 = rectangle(&mut s, night_wrapper, "Night card B", 420.0, 100.0);
    let c2 = connect_deep(&mut s, a2, b2);
    assert_eq!(s.app().doc.get(c2).unwrap().parent, Some(night_wrapper));
    let ink2 = ink_of(&s, c2);
    assert_eq!(ink2, crate::board::color_of(crate::theme::BOARD_INK_ON_DARK), "the dark screen above the wrapper is the surface");
    assert!(designos_doc::lint::rules::contrast::contrast_ratio(ink2, night_fill) >= 3.0);
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

fn dot_of(s: &Sim, id: NodeId, m: designos_doc::Magnet) -> Pos2 {
    let (_, p) = crate::board::connect::dot_positions(s.app(), id).into_iter().find(|(k, _)| *k == m).unwrap();
    s.screen(p.x, p.y)
}

#[test]
fn board_dots_leave_a_selected_item_when_the_pointer_does() {
    let mut s = board_sim();
    let page = s.app().page;
    let a = shape(&mut s, page, "A", 200.0, 200.0);
    s.click(center(&s, a));
    assert_eq!(s.app().selection, vec![a]);
    assert_eq!(s.app().board.dots, Some(a), "the dots show while the pointer is over the item");
    s.mv(s.screen(900.0, 600.0));
    s.mv(s.screen(905.0, 600.0));
    assert_eq!(s.app().board.dots, None, "a board shows the dots of the item under the pointer only, not of the selection");
}

#[test]
fn board_connector_tool_hover_leaves_no_highlight() {
    let mut s = board_sim();
    let page = s.app().page;
    let a = shape(&mut s, page, "A", 200.0, 200.0);
    s.shell.app.set_tool(Tool::Connector);
    s.mv(center(&s, a));
    s.mv(center(&s, a) + egui::vec2(1.0, 0.0));
    assert_eq!(s.app().hover, None, "the Connector tool in a board file draws no hover highlight");
    assert_eq!(s.app().board.dots, Some(a), "the dots still show on the item under the pointer");
}

#[test]
fn board_connector_drag_with_alt_still_attaches_both_ends() {
    let mut s = board_sim();
    let page = s.app().page;
    let a = shape(&mut s, page, "A", 120.0, 200.0);
    let b = shape(&mut s, page, "B", 560.0, 200.0);
    s.shell.app.set_tool(Tool::Connector);
    s.mods = Modifiers { alt: true, ..Modifiers::NONE };
    s.drag(center(&s, a), center(&s, b));
    s.mods = Modifiers::NONE;
    let cs = connectors(&s);
    assert_eq!(cs.len(), 1);
    assert_eq!(ends(&s, cs[0]), (Some(a), Some(b), Some(page)), "Alt does not free the ends in a board file");
}

#[test]
fn board_dot_press_with_alt_still_starts_a_connector() {
    let mut s = board_sim();
    let page = s.app().page;
    let a = shape(&mut s, page, "A", 120.0, 200.0);
    let b = shape(&mut s, page, "B", 560.0, 200.0);
    s.mv(center(&s, a));
    s.mv(center(&s, a) + egui::vec2(1.0, 0.0));
    s.mods = Modifiers { alt: true, ..Modifiers::NONE };
    s.drag(dot_of(&s, a, designos_doc::Magnet::Right), center(&s, b));
    s.mods = Modifiers::NONE;
    let cs = connectors(&s);
    assert_eq!(cs.len(), 1, "pressing a dot with Alt held still drags a connector out");
    assert_eq!(ends(&s, cs[0]).0, Some(a));
}

/// Contrast between the line a connector draws and the card fill next to it, read from the CPU render.
fn rendered_line_contrast(s: &Sim, c: NodeId) -> f64 {
    let doc = &s.app().doc;
    // the route is in the connector's parent space; the render is in page space
    let local = designos_doc::connector_route(doc, c).expect("route").midpoint();
    let parent = doc.get(c).and_then(|n| n.parent).expect("a connector has a parent");
    let mid = doc.world_transform(parent).apply(local);
    let mut pm = designos_render::tiny_skia::Pixmap::new(800, 520).unwrap();
    let vp = designos_render::Viewport { offset: DV::ZERO, zoom: 1.0, dpr: 1.0 };
    designos_render::render_page(doc, s.app().page, &vp, &mut pm);
    let at = |x: f32, y: f32| {
        let px = pm.pixel(x as u32, y as u32).expect("pixel inside the render").demultiply();
        Color::rgb(px.red(), px.green(), px.blue())
    };
    let fill = at(mid.x, mid.y + 12.0);
    (-3..=3).map(|dy| designos_doc::lint::rules::contrast::contrast_ratio(at(mid.x, mid.y + dy as f32), fill)).fold(1.0, f64::max)
}

#[test]
fn default_ink_stays_visible_in_a_card_faded_by_its_layer_opacity() {
    let mut s = design_sim();
    let page = s.app().page;
    // a white card at 40% layer opacity on the dark default page: the line and the fill fade together
    let card = add(
        &mut s,
        page,
        |id| {
            let mut n = Node::frame(id, DRect::new(80.0, 80.0, 640.0, 360.0));
            n.opacity = 0.4;
            n
        },
        "Faded card",
    );
    let a = rectangle(&mut s, card, "Card A", 40.0, 100.0);
    let b = rectangle(&mut s, card, "Card B", 420.0, 100.0);
    let c = connect_deep(&mut s, a, b);
    assert_eq!(s.app().doc.get(c).unwrap().parent, Some(card));
    let ratio = rendered_line_contrast(&s, c);
    assert!(ratio >= 3.0, "the rendered line reaches 3:1 against the card pixel next to it, got {ratio:.2}:1");
    assert_eq!(ink_of(&s, c), crate::board::color_of(crate::theme::BOARD_INK[0]), "the card's white fill is the surface, whatever its layer opacity");
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

fn deck_sim() -> Sim {
    let mut s = Sim::new();
    let ctx = s.ctx.clone();
    crate::panels::menus::run(&mut s.shell.app, &ctx, crate::panels::menus::Action::NewDeck);
    s.frame(vec![]);
    s.frame(vec![]);
    assert!(s.app().is_deck());
    s
}

#[test]
fn deck_files_offer_no_connector_and_x_does_nothing() {
    let mut s = deck_sim();
    assert!(crate::panels::menus::palette_entries(s.app(), "connector").is_empty(), "the command palette lists no Connector in a deck");

    let chevron = s.rect_of("tool-chevron-2");
    s.click(chevron.center());
    assert_eq!(s.app().ui.menu, Some(crate::app::MenuKind::ToolGroup(2)));
    // Rectangle, Line, Arrow, Ellipse, Polygon, Star, Image
    let rows = 7.0;
    let menu = s.rect_of("menu-level-0");
    assert_eq!(menu.height(), crate::theme::MENU_PADDING * 2.0 + rows * crate::theme::MENU_ITEM_HEIGHT, "the shape dropdown has no Connector row");

    s.key(Key::Escape);
    s.key(Key::X);
    assert_eq!(s.app().tool, Tool::Move, "X does nothing in a deck");
}

#[test]
fn a_remembered_connector_pick_does_not_follow_into_a_deck_file() {
    let mut s = deck_sim();
    let connector_row = crate::canvas::tools::GROUPS[2].iter().position(|t| *t == Tool::Connector).expect("the shape group lists Connector");
    s.shell.app.group_pick[2] = connector_row;
    s.frame(vec![]);
    // the button sits left of its chevron
    let chevron = s.rect_of("tool-chevron-2");
    s.click(Pos2::new(chevron.left() - crate::theme::TOOL_CHEVRON_GAP - crate::theme::TOOL_BUTTON / 2.0, chevron.center().y));
    assert_eq!(s.app().tool, Tool::Rectangle, "the shape button falls back to the first tool a deck offers");
}

#[test]
fn a_connector_tool_carried_into_a_deck_falls_back_to_move() {
    let mut s = deck_sim();
    // the tool of the file the user came from (a tab switch keeps it)
    s.shell.app.tool = Tool::Connector;
    s.frame(vec![]);
    assert_eq!(s.app().tool, Tool::Move, "a deck offers no Connector tool, so it is not left active");
    s.shell.app.set_tool(Tool::Connector);
    s.frame(vec![]);
    assert_eq!(s.app().tool, Tool::Move, "asking for it again does not leave a dead tool either");
}

#[test]
fn design_files_offer_connector_in_the_shape_dropdown_and_the_palette() {
    let mut s = design_sim();
    assert!(crate::panels::menus::palette_entries(s.app(), "connector").iter().any(|(label, shortcut)| label == "Connector" && shortcut == "X"));
    let chevron = s.rect_of("tool-chevron-2");
    s.click(chevron.center());
    let rows = 8.0;
    let menu = s.rect_of("menu-level-0");
    assert_eq!(menu.height(), crate::theme::MENU_PADDING * 2.0 + rows * crate::theme::MENU_ITEM_HEIGHT);
}

/// A page with frame F holding rectangles A and B and the connector C that joins them.
fn doc_with_connector_in_a_frame() -> (designos_doc::Document, NodeId, NodeId, NodeId, NodeId, NodeId) {
    use designos_doc::{ConnectorEnd, ConnectorProps, Document, Magnet};
    let mut doc = Document::new("live patch connector");
    let page = doc.add_page("Page 1");
    let f = doc.alloc_id();
    doc.insert(page, Node::frame(f, DRect::new(100.0, 100.0, 600.0, 300.0))).unwrap();
    let a = doc.alloc_id();
    doc.insert(f, Node::rectangle(a, DRect::new(40.0, 100.0, 100.0, 60.0))).unwrap();
    let b = doc.alloc_id();
    doc.insert(f, Node::rectangle(b, DRect::new(400.0, 100.0, 100.0, 60.0))).unwrap();
    let c = doc.alloc_id();
    let props = ConnectorProps { start: ConnectorEnd::attached(a, Magnet::Auto), end: ConnectorEnd::attached(b, Magnet::Auto), ..Default::default() };
    doc.insert(f, Node::connector(c, props)).unwrap();
    designos_doc::sync_connector(&mut doc, c);
    (doc, page, f, a, b, c)
}

fn live_patch_of(doc: &designos_doc::Document, cmd: Command, label: &str) -> designos_doc::live_patch::LivePatch {
    let ops = designos_doc::live_patch::live_ops_for_command(doc, &cmd).unwrap();
    let mut patch = designos_doc::live_patch::LivePatch::new(ops);
    patch.label = Some(label.into());
    patch
}

#[test]
fn an_agent_reparenting_an_end_node_rehomes_the_connector_in_the_same_undo_step() {
    let (doc, page, f, _a, b, c) = doc_with_connector_in_a_frame();
    let patch = live_patch_of(&doc, Command::Reparent { id: b, parent: page, index: usize::MAX }, "move B out of F");
    let mut app = crate::app::App::new(doc);
    let depth = app.hist.undo_depth();
    crate::live_patch_apply::apply_live_patches(&mut app, &[patch]);
    assert_eq!(app.doc.get(b).unwrap().parent, Some(page));
    assert_eq!(app.doc.get(c).unwrap().parent, Some(page), "the connector follows to the nearest common container");
    let g = designos_doc::connector_geometry(&app.doc, c).unwrap();
    let NodeKind::Connector(p) = &app.doc.get(c).unwrap().kind else { unreachable!() };
    assert_eq!((p.start.point, p.end.point), (g.start_point, g.end_point), "the end points are resolved in the new parent");
    assert_eq!(app.hist.undo_depth(), depth + 1, "the patch and the re-home are one undo step");
    app.undo();
    assert_eq!(app.doc.get(b).unwrap().parent, Some(f));
    assert_eq!(app.doc.get(c).unwrap().parent, Some(f), "one undo restores both");
}

#[test]
fn an_agent_moving_an_end_node_to_another_page_detaches_that_end_and_says_so() {
    let (mut doc, page1, _f, a, b, c) = doc_with_connector_in_a_frame();
    let page2 = doc.add_page("Page 2");
    let patch = live_patch_of(&doc, Command::Reparent { id: b, parent: page2, index: 0 }, "move B to page 2");
    let mut app = crate::app::App::new(doc);
    let _ = crate::board::connect::take_detach_toast();
    crate::live_patch_apply::apply_live_patches(&mut app, &[patch]);
    assert_eq!(app.doc.page_of(c), Some(page1), "the connector stays on its page");
    let NodeKind::Connector(p) = &app.doc.get(c).unwrap().kind else { unreachable!() };
    assert_eq!((p.start.node, p.end.node), (Some(a), None), "the end on the moved node is free");
    assert!(crate::board::connect::take_detach_toast().is_some(), "the user is told");
}

#[test]
fn a_connector_the_user_moved_by_hand_is_not_pulled_back_by_an_unrelated_reparent() {
    let mut s = design_sim();
    let page = s.app().page;
    let a = add(&mut s, page, |id| Node::frame(id, DRect::new(100.0, 100.0, 160.0, 100.0)), "A");
    let b = add(&mut s, page, |id| Node::frame(id, DRect::new(520.0, 100.0, 160.0, 100.0)), "B");
    let f = add(&mut s, page, |id| Node::frame(id, DRect::new(100.0, 400.0, 300.0, 200.0)), "F");
    let g = add(&mut s, page, |id| Node::frame(id, DRect::new(520.0, 400.0, 300.0, 200.0)), "G");
    let l = rectangle(&mut s, page, "L", 900.0, 100.0);
    s.shell.app.set_tool(Tool::Connector);
    s.drag(center(&s, a), center(&s, b));
    let c = connectors(&s)[0];

    // the user drops the connector into F in the Layers panel
    let mut tx = crate::edit::Tx::new();
    tx.cmd(&mut s.shell.app.doc, Command::Reparent { id: c, parent: f, index: usize::MAX }).unwrap();
    s.shell.app.run_tx(tx);
    s.frame(vec![]);
    assert_eq!(s.app().doc.get(c).unwrap().parent, Some(f), "a manual reparent sticks");

    // one step: A is nudged and a layer that has nothing to do with the connector goes into G
    let mut tx = crate::edit::Tx::new();
    tx.ed.node(&mut s.shell.app.doc, a).unwrap().transform = Affine::translate(100.0, 120.0);
    tx.cmd(&mut s.shell.app.doc, Command::Reparent { id: l, parent: g, index: usize::MAX }).unwrap();
    s.shell.app.run_tx(tx);
    s.frame(vec![]);
    assert_eq!(s.app().doc.get(l).unwrap().parent, Some(g));
    assert_eq!(s.app().doc.get(c).unwrap().parent, Some(f), "the connector stays where the user put it");
}

#[test]
fn x_over_the_align_row_leaves_a_pen_path_in_progress_alone() {
    let mut s = design_sim();
    let page = s.app().page;
    let f = add(&mut s, page, |id| Node::frame(id, DRect::new(200.0, 200.0, 240.0, 120.0)), "AL");
    if let Some(fp) = s.shell.app.doc.get_mut(f).unwrap().kind.frame_props_mut() {
        fp.auto_layout = Some(designos_doc::AutoLayout::default());
    }
    // a pen path with two points is in progress on empty canvas
    s.shell.app.set_tool(Tool::Pen);
    s.click(s.screen(600.0, 100.0));
    s.click(s.screen(700.0, 160.0));
    assert!(s.app().canvas.pen.is_some(), "the pen path is in progress");
    let nodes_before = s.app().doc.nodes.len();

    s.shell.app.select_only(f);
    s.shell.app.changed();
    s.frame(vec![]);
    s.frame(vec![]);
    let ph = s.field_rect("al-ph");
    let align_dy =
        (crate::theme::FIELD_HEIGHT + 4.0) + (crate::theme::SPACE_GROUP - crate::theme::SPACE_ROW) + crate::theme::LABEL_BOX_H + crate::theme::SPACE_FACT;
    s.mv(Pos2::new(ph.center().x, ph.center().y + align_dy));
    s.key(Key::X);

    let al = s.app().doc.get(f).unwrap().kind.frame_props().unwrap().auto_layout.as_ref().unwrap();
    assert_eq!(al.primary_align, designos_doc::LayoutAlign::SpaceBetween, "X over the align row toggles auto spacing");
    assert_eq!(s.app().tool, Tool::Pen, "X over the align row does not touch the tool");
    assert!(s.app().canvas.pen.is_some(), "the pen path is still in progress");
    assert_eq!(s.app().doc.nodes.len(), nodes_before, "no layer was made from the pen path");
}
