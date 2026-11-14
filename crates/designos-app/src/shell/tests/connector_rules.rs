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

fn try_rect_of(s: &Sim, name: &str) -> Option<Rect> {
    s.ctx.data(|d| d.get_temp::<Rect>(egui::Id::new(("test-rect", name.to_string()))))
}

fn collect_texts_in(shapes: &[egui::epaint::ClippedShape], rect: Rect, out: &mut Vec<String>) {
    for cs in shapes {
        match &cs.shape {
            egui::epaint::Shape::Text(t) => {
                if rect.contains(t.pos) || rect.intersects(t.visual_bounding_rect()) {
                    out.push(t.galley.text().to_string());
                }
            }
            egui::epaint::Shape::Vec(nested) => {
                let wrapped: Vec<egui::epaint::ClippedShape> =
                    nested.iter().cloned().map(|shape| egui::epaint::ClippedShape { clip_rect: cs.clip_rect, shape }).collect();
                collect_texts_in(&wrapped, rect, out);
            }
            _ => {}
        }
    }
}

fn texts_in(s: &Sim, rect: Rect) -> Vec<String> {
    let mut out = Vec::new();
    collect_texts_in(&s.shapes, rect, &mut out);
    out
}

fn design_connector_pair(s: &mut Sim) -> (NodeId, NodeId, NodeId) {
    let page = s.app().page;
    let a = rectangle(s, page, "A", 120.0, 180.0);
    let b = rectangle(s, page, "B", 440.0, 320.0);
    s.shell.app.set_tool(Tool::Connector);
    s.drag(center(s, a), center(s, b));
    let c = connectors(s)[0];
    s.shell.app.select_only(c);
    s.shell.app.changed();
    s.frame(vec![]);
    s.frame(vec![]);
    (a, b, c)
}

#[test]
fn bar_shows_for_a_connector_selection_in_a_design_file() {
    let mut s = design_sim();
    let (_a, _b, c) = design_connector_pair(&mut s);
    assert_eq!(s.app().selection, vec![c]);
    let bar = try_rect_of(&s, "board-bar").expect("floating bar appears for a selected connector in a design file");
    let canvas = s.app().ui.regions.canvas.expect("canvas region populated");
    let right = s.app().ui.regions.right_panel.expect("inspector region populated");
    assert!(canvas.contains_rect(bar), "floating bar stays inside the canvas region: bar={bar:?} canvas={canvas:?}");
    assert!(bar.right() <= right.left(), "floating bar stays clear of the Inspector: bar={bar:?} right={right:?}");
    for key in ["board-line-Elbowed", "board-line-Straight", "board-line-Curved", "board-cap-start", "board-cap-end", "board-swatch-0"] {
        assert!(try_rect_of(&s, key).is_some(), "floating bar control {key} is present");
    }
}

#[test]
fn bar_stays_hidden_for_a_rectangle_in_a_design_file() {
    let mut s = design_sim();
    let (a, _b, c) = design_connector_pair(&mut s);
    assert!(try_rect_of(&s, "board-bar").is_some(), "floating bar appears when only the connector is selected");

    s.shell.app.select_only(a);
    s.shell.app.changed();
    s.frame(vec![]);
    assert!(try_rect_of(&s, "board-bar").is_none(), "floating bar is hidden when a rectangle is selected in a design file");

    s.shell.app.selection = vec![a, c];
    s.shell.app.changed();
    s.frame(vec![]);
    assert!(try_rect_of(&s, "board-bar").is_none(), "floating bar is hidden when selection mixes a rectangle and a connector");
}

#[test]
fn bar_changes_line_style_caps_and_ink_in_one_undo_step_each() {
    use designos_doc::{ConnectorCap, ConnectorLine};
    let mut s = design_sim();
    let (_a, _b, c) = design_connector_pair(&mut s);
    let d0 = s.app().hist.undo_depth();

    let curved_btn = s.rect_of("board-line-Curved");
    s.click(curved_btn.center());
    let props = match &s.app().doc.get(c).unwrap().kind {
        NodeKind::Connector(p) => p.clone(),
        other => panic!("expected connector, got {other:?}"),
    };
    assert_eq!(props.line, ConnectorLine::Curved);
    assert_eq!(s.app().hist.undo_depth(), d0 + 1, "changing line style from the bar is one undo step");

    let start_btn = s.rect_of("board-cap-start");
    s.click(start_btn.center());
    let start_circle = s.rect_of("board-cap-start-Circle");
    s.click(start_circle.center());
    let props = match &s.app().doc.get(c).unwrap().kind {
        NodeKind::Connector(p) => p.clone(),
        other => panic!("expected connector, got {other:?}"),
    };
    assert_eq!(props.start_cap, ConnectorCap::Circle);
    assert_eq!(s.app().hist.undo_depth(), d0 + 2, "changing start cap from the bar is one undo step");

    let end_btn = s.rect_of("board-cap-end");
    s.click(end_btn.center());
    let end_diamond = s.rect_of("board-cap-end-Diamond");
    s.click(end_diamond.center());
    let props = match &s.app().doc.get(c).unwrap().kind {
        NodeKind::Connector(p) => p.clone(),
        other => panic!("expected connector, got {other:?}"),
    };
    assert_eq!(props.end_cap, ConnectorCap::Diamond);
    assert_eq!(s.app().hist.undo_depth(), d0 + 3, "changing end cap from the bar is one undo step");

    let swatch_btn = s.rect_of("board-swatch-2");
    s.click(swatch_btn.center());
    let stroke_rgb = crate::board::create::solid_rgb(&s.app().doc.get(c).unwrap().stroke.paints[0]).unwrap();
    assert_eq!(stroke_rgb, crate::theme::BOARD_INK[2]);
    assert_eq!(s.app().hist.undo_depth(), d0 + 4, "changing ink from the bar is one undo step");

    s.shell.app.undo();
    let stroke_rgb = crate::board::create::solid_rgb(&s.app().doc.get(c).unwrap().stroke.paints[0]).unwrap();
    assert_eq!(stroke_rgb, crate::theme::BOARD_INK_ON_DARK);
    s.shell.app.undo();
    let NodeKind::Connector(p) = &s.app().doc.get(c).unwrap().kind else { unreachable!() };
    assert_eq!(p.end_cap, ConnectorCap::Arrow);
    s.shell.app.undo();
    let NodeKind::Connector(p) = &s.app().doc.get(c).unwrap().kind else { unreachable!() };
    assert_eq!(p.start_cap, ConnectorCap::None);
    s.shell.app.undo();
    let NodeKind::Connector(p) = &s.app().doc.get(c).unwrap().kind else { unreachable!() };
    assert_eq!(p.line, ConnectorLine::Elbowed);
}

#[test]
fn inspector_shows_connector_style_in_the_stroke_section() {
    let mut s = design_sim();
    let (_a, _b, _c) = design_connector_pair(&mut s);

    let weight_r = s.field_rect("stroke-w");
    let line_r = try_rect_of(&s, "connector-line").expect("connector line dropdown in Stroke section");
    let start_r = try_rect_of(&s, "connector-start-cap").expect("connector start cap dropdown in Stroke section");
    let end_r = try_rect_of(&s, "connector-end-cap").expect("connector end cap dropdown in Stroke section");

    assert!(try_rect_of(&s, "stroke-align").is_none(), "stroke align is hidden for connectors because connector lowering ignores it");
    assert!(line_r.top() >= weight_r.bottom(), "line style comes after stroke weight: weight={weight_r:?} line={line_r:?}");
    assert!(start_r.top() >= line_r.bottom(), "cap pickers come after line style: line={line_r:?} start={start_r:?}");
    assert!(
        (start_r.top() - end_r.top()).abs() < 1.0 && end_r.left() > start_r.right(),
        "start and end cap pickers share the row in order: start={start_r:?} end={end_r:?}"
    );

    assert!(texts_in(&s, line_r).iter().any(|t| t == "Elbowed"));
    assert!(texts_in(&s, start_r).iter().any(|t| t == "None"));
    assert!(texts_in(&s, end_r).iter().any(|t| t == "Arrow"));
}

#[test]
fn inspector_and_bar_write_the_same_connector_style() {
    use designos_doc::{ConnectorCap, ConnectorLine};
    let mut s = design_sim();
    let page = s.app().page;
    let a1 = rectangle(&mut s, page, "A1", 100.0, 120.0);
    let b1 = rectangle(&mut s, page, "B1", 420.0, 260.0);
    s.shell.app.set_tool(Tool::Connector);
    s.drag(center(&s, a1), center(&s, b1));
    let c_bar = connectors(&s)[0];

    let a2 = rectangle(&mut s, page, "A2", 100.0, 120.0);
    let b2 = rectangle(&mut s, page, "B2", 420.0, 260.0);
    s.shell.app.set_tool(Tool::Connector);
    s.drag(center(&s, a2), center(&s, b2));
    let c_insp = *connectors(&s).iter().find(|&&id| id != c_bar).unwrap();

    // Style c_bar through the floating bar: Straight, Start=Triangle, End=Diamond
    s.shell.app.select_only(c_bar);
    s.shell.app.changed();
    s.frame(vec![]);
    let d_bar_0 = s.app().hist.undo_depth();
    let straight_btn = s.rect_of("board-line-Straight");
    s.click(straight_btn.center());
    let start_btn = s.rect_of("board-cap-start");
    s.click(start_btn.center());
    let start_tri = s.rect_of("board-cap-start-Triangle");
    s.click(start_tri.center());
    let end_btn = s.rect_of("board-cap-end");
    s.click(end_btn.center());
    let end_dia = s.rect_of("board-cap-end-Diamond");
    s.click(end_dia.center());
    assert_eq!(s.app().hist.undo_depth(), d_bar_0 + 3);
    s.key(Key::Enter);
    s.cmd_key(Key::A);
    s.type_text("OK");
    s.key(Key::Escape);
    assert_eq!(s.app().hist.undo_depth(), d_bar_0 + 5);

    // Style c_insp through the Inspector Stroke section: Straight (idx 1), Start=Triangle (idx 2), End=Diamond (idx 4)
    s.shell.app.select_only(c_insp);
    s.shell.app.changed();
    s.frame(vec![]);
    let d_insp_0 = s.app().hist.undo_depth();
    let line_dd = s.rect_of("connector-line");
    s.click(line_dd.center());
    let row_straight = s.rect_of("menu-row-0-1");
    s.click(row_straight.center());

    let start_dd = s.rect_of("connector-start-cap");
    s.click(start_dd.center());
    let row_tri = s.rect_of("menu-row-0-2");
    s.click(row_tri.center());

    s.key(Key::Enter);
    s.cmd_key(Key::A);
    s.type_text("OK");
    s.frame(vec![]);
    let end_dd = s.rect_of("connector-end-cap");
    s.click(end_dd.center());
    let row_dia = s.rect_of("menu-row-0-4");
    s.click(row_dia.center());
    s.key(Key::Escape);
    assert_eq!(s.app().hist.undo_depth(), d_insp_0 + 4);

    let n_bar = s.app().doc.get(c_bar).unwrap();
    let n_insp = s.app().doc.get(c_insp).unwrap();
    let NodeKind::Connector(p_bar) = &n_bar.kind else { unreachable!() };
    let NodeKind::Connector(p_insp) = &n_insp.kind else { unreachable!() };
    assert_eq!(p_insp.line, ConnectorLine::Straight);
    assert_eq!(p_insp.start_cap, ConnectorCap::Triangle);
    assert_eq!(p_insp.end_cap, ConnectorCap::Diamond);
    assert_eq!((p_insp.line, p_insp.start_cap, p_insp.end_cap), (p_bar.line, p_bar.start_cap, p_bar.end_cap));
    assert_eq!(n_insp.transform, n_bar.transform, "both setters sync connector transform identically");
    assert_eq!(n_insp.size, n_bar.size, "both setters sync connector size identically");
}

#[test]
fn mixed_connector_selection_shows_mixed_and_changes_all_in_one_step() {
    use designos_doc::{ConnectorCap, ConnectorLine};
    let mut s = design_sim();
    let page = s.app().page;
    let a1 = rectangle(&mut s, page, "A1", 100.0, 120.0);
    let b1 = rectangle(&mut s, page, "B1", 400.0, 120.0);
    s.shell.app.set_tool(Tool::Connector);
    s.drag(center(&s, a1), center(&s, b1));
    let c1 = connectors(&s)[0];

    let a2 = rectangle(&mut s, page, "A2", 100.0, 300.0);
    let b2 = rectangle(&mut s, page, "B2", 400.0, 420.0);
    s.shell.app.set_tool(Tool::Connector);
    s.drag(center(&s, a2), center(&s, b2));
    let c2 = *connectors(&s).iter().find(|&&id| id != c1).unwrap();

    // Give c2 different line and caps from c1
    s.shell.app.edit_nodes(&[c2], |n| {
        if let NodeKind::Connector(p) = &mut n.kind {
            p.line = ConnectorLine::Curved;
            p.start_cap = ConnectorCap::Circle;
            p.end_cap = ConnectorCap::Triangle;
        }
    });
    designos_doc::sync_connector(&mut s.shell.app.doc, c2);

    s.shell.app.selection = vec![c1, c2];
    s.shell.app.changed();
    s.frame(vec![]);
    s.frame(vec![]);

    let line_r = s.rect_of("connector-line");
    let start_r = s.rect_of("connector-start-cap");
    let end_r = s.rect_of("connector-end-cap");
    assert!(texts_in(&s, line_r).iter().any(|t| t == "Mixed"), "mixed line styles show Mixed");
    assert!(texts_in(&s, start_r).iter().any(|t| t == "Mixed"), "mixed start caps show Mixed");
    assert!(texts_in(&s, end_r).iter().any(|t| t == "Mixed"), "mixed end caps show Mixed");

    let d0 = s.app().hist.undo_depth();
    s.click(line_r.center());
    let row_straight = s.rect_of("menu-row-0-1");
    s.click(row_straight.center());
    assert_eq!(s.app().hist.undo_depth(), d0 + 1, "changing mixed line style applies to all in one undo step");
    for id in [c1, c2] {
        let NodeKind::Connector(p) = &s.app().doc.get(id).unwrap().kind else { unreachable!() };
        assert_eq!(p.line, ConnectorLine::Straight);
    }

    let end_r = s.rect_of("connector-end-cap");
    s.click(end_r.center());
    let row_diamond = s.rect_of("menu-row-0-4");
    s.click(row_diamond.center());
    assert_eq!(s.app().hist.undo_depth(), d0 + 2, "changing mixed end cap applies to all in one undo step");
    for id in [c1, c2] {
        let NodeKind::Connector(p) = &s.app().doc.get(id).unwrap().kind else { unreachable!() };
        assert_eq!(p.end_cap, ConnectorCap::Diamond);
    }

    s.shell.app.undo();
    let NodeKind::Connector(p1) = &s.app().doc.get(c1).unwrap().kind else { unreachable!() };
    let NodeKind::Connector(p2) = &s.app().doc.get(c2).unwrap().kind else { unreachable!() };
    assert_eq!((p1.end_cap, p2.end_cap), (ConnectorCap::Arrow, ConnectorCap::Triangle));

    s.shell.app.undo();
    let NodeKind::Connector(p1) = &s.app().doc.get(c1).unwrap().kind else { unreachable!() };
    let NodeKind::Connector(p2) = &s.app().doc.get(c2).unwrap().kind else { unreachable!() };
    assert_eq!((p1.line, p2.line), (ConnectorLine::Elbowed, ConnectorLine::Curved));
}

#[test]
fn changing_line_style_reroutes_the_connector() {
    use designos_doc::ConnectorLine;
    let mut s = design_sim();
    let page = s.app().page;
    // Vertically aligned left anchor on A and a left-facing end at (100, 330):
    // Elbowed steps left to x = 76 (transform.tx = 76 - pad, w = 24 + 2*pad),
    // Straight stays at x = 100 (transform.tx = 100 - pad, w = 2*pad),
    // Curved arches further left (transform.tx < 76 - pad, w > 60).
    let a = add(&mut s, page, |id| Node::rectangle(id, DRect::new(100.0, 100.0, 80.0, 60.0)), "A");

    let cid = s.shell.app.doc.alloc_id();
    let props = designos_doc::ConnectorProps {
        start: designos_doc::ConnectorEnd::attached(a, designos_doc::Magnet::Left),
        end: designos_doc::ConnectorEnd { node: None, magnet: designos_doc::Magnet::Left, point: DV::new(100.0, 330.0) },
        line: ConnectorLine::Elbowed,
        ..Default::default()
    };
    let c = s.shell.app.doc.insert(page, Node::connector(cid, props)).unwrap();
    designos_doc::sync_connector(&mut s.shell.app.doc, c);
    s.shell.app.select_only(c);
    s.shell.app.changed();
    s.frame(vec![]);
    s.frame(vec![]);

    let elbow_route = designos_doc::connector_route(&s.app().doc, c).unwrap();
    assert!(elbow_route.curve.is_none() && elbow_route.pts.len() >= 4, "elbow route steps out from the left side: {elbow_route:?}");
    let elbow_size = s.app().doc.get(c).unwrap().size;

    // Switch to Straight via the Inspector dropdown: must re-route without shifting the free end and shrink stored node width
    let line_dd = s.rect_of("connector-line");
    s.click(line_dd.center());
    let row_straight = s.rect_of("menu-row-0-1");
    s.click(row_straight.center());

    let straight_route = designos_doc::connector_route(&s.app().doc, c).unwrap();
    assert_eq!(straight_route.pts.len(), 2, "straight route has two endpoints");
    assert_eq!(straight_route.end(), DV::new(100.0, 330.0), "changing line style does not shift the endpoint");
    let expected_straight_geom = designos_doc::connector_geometry(&s.app().doc, c).unwrap();
    let n_after_straight = s.app().doc.get(c).unwrap();
    assert_eq!(n_after_straight.transform, expected_straight_geom.transform, "stored transform updated by sync_connector on line change");
    assert_eq!(n_after_straight.size, expected_straight_geom.size, "stored size updated by sync_connector on line change");
    assert!(n_after_straight.size.x < elbow_size.x, "straight extent is narrower horizontally than the left-looped elbow extent");

    // Switch to Curved via the floating bar: must re-route to a cubic curve and widen stored node geometry
    let curved_btn = s.rect_of("board-line-Curved");
    s.click(curved_btn.center());
    let curved_route = designos_doc::connector_route(&s.app().doc, c).unwrap();
    assert!(curved_route.curve.is_some(), "curved route carries cubic control points");
    assert_eq!(curved_route.end(), DV::new(100.0, 330.0), "changing line style to Curved does not shift the endpoint");
    let expected_curved_geom = designos_doc::connector_geometry(&s.app().doc, c).unwrap();
    let n_after_curved = s.app().doc.get(c).unwrap();
    assert_eq!(n_after_curved.transform, expected_curved_geom.transform);
    assert_eq!(n_after_curved.size, expected_curved_geom.size);
    assert!(n_after_curved.size.x > elbow_size.x, "curved extent arches wider than the elbow stub");
}

#[test]
fn enter_opens_the_label_editor_on_a_connector_in_a_design_file() {
    let mut s = design_sim();
    let (_a, _b, c) = design_connector_pair(&mut s);
    s.key(Key::Enter);
    assert_eq!(s.app().canvas.text.as_ref().map(|t| t.id), Some(c), "Enter opens the text editor on the selected connector");
    s.cmd_key(Key::A);
    s.type_text("Approved");
    s.key(Key::Escape);
    assert!(s.app().canvas.text.is_none(), "Escape commits and closes the connector label editor");
    let NodeKind::Connector(p) = &s.app().doc.get(c).unwrap().kind else { unreachable!() };
    assert_eq!(p.label.as_ref().map(|l| l.content.as_str()), Some("Approved"));
}

#[test]
fn clicking_a_connector_label_selects_the_connector() {
    let mut s = design_sim();
    let (_a, _b, c) = design_connector_pair(&mut s);
    s.key(Key::Enter);
    s.cmd_key(Key::A);
    s.type_text("Approved");
    s.key(Key::Escape);

    s.shell.app.selection.clear();
    s.shell.app.changed();
    s.frame(vec![]);
    assert!(s.app().selection.is_empty());

    let n = s.app().doc.get(c).unwrap();
    let tf = designos_render::board::text_frame(&s.app().doc, n).expect("connector has a rendered label frame");
    let label_world = s.app().doc.world_transform(c).apply(DV::new(tf.rect.x + tf.rect.w / 2.0, tf.rect.y + tf.rect.h / 2.0));
    s.click(s.screen(label_world.x, label_world.y));
    assert_eq!(s.app().selection, vec![c], "clicking the connector label chip selects the connector");
}

#[test]
fn connector_label_is_inside_its_export_bounds() {
    use designos_doc::{board_text, sync_connector, ConnectorEnd, ConnectorProps, Document, Magnet, TextAlignH, TextAlignV};
    use designos_export::{export_svg, render_bounds, render_export, SvgOptions};

    let mut doc = Document::new("connector export bounds");
    let page = doc.add_page("Page 1");
    let frame = doc.alloc_id();
    doc.insert(page, Node::frame(frame, DRect::new(40.0, 40.0, 500.0, 420.0))).unwrap();
    let top_box = doc.alloc_id();
    doc.insert(frame, Node::rectangle(top_box, DRect::new(180.0, 20.0, 80.0, 60.0))).unwrap();
    let bottom_box = doc.alloc_id();
    doc.insert(frame, Node::rectangle(bottom_box, DRect::new(180.0, 280.0, 80.0, 60.0))).unwrap();

    // Vertical connector with a wide multi-line label that extends well past the vertical stroke
    let cid = doc.alloc_id();
    let mut label = board_text(14.0, TextAlignH::Center, TextAlignV::Center);
    label.content = "MMMMMM\nWWWWWW".into();
    let props = ConnectorProps {
        start: ConnectorEnd::attached(top_box, Magnet::Bottom),
        end: ConnectorEnd::attached(bottom_box, Magnet::Top),
        label: Some(label),
        ..Default::default()
    };
    let c = doc.insert(frame, Node::connector(cid, props)).unwrap();
    assert!(sync_connector(&mut doc, c));

    let conn_node = doc.get(c).unwrap();
    let local_extent = DRect::new(0.0, 0.0, conn_node.size.x, conn_node.size.y);
    let export_rect = render_bounds(&doc, c);
    let lowered = designos_render::board::lower(&doc, conn_node).expect("connector lowers");
    for child_id in &lowered.children {
        let child = designos_render::board::get(&doc, *child_id).expect("synthetic child exists");
        let child_rect = DRect::new(child.transform.tx, child.transform.ty, child.size.x, child.size.y);
        assert!(
            local_extent.x <= child_rect.x
                && local_extent.y <= child_rect.y
                && local_extent.x + local_extent.w >= child_rect.x + child_rect.w
                && local_extent.y + local_extent.h >= child_rect.y + child_rect.h,
            "connector_extent local box {local_extent:?} must contain lowered child {child_rect:?}"
        );
        assert!(
            export_rect.x <= child_rect.x
                && export_rect.y <= child_rect.y
                && export_rect.x + export_rect.w >= child_rect.x + child_rect.w
                && export_rect.y + export_rect.h >= child_rect.y + child_rect.h,
            "export render_bounds {export_rect:?} must contain lowered child {child_rect:?}"
        );
    }

    let svg_conn = export_svg(&doc, c, &SvgOptions { outline_text: false });
    assert!(svg_conn.contains("MMMMMM") && svg_conn.contains("WWWWWW"), "connector SVG includes label text: {svg_conn}");
    assert!(svg_conn.contains("fill=\"#FFFFFF\""), "connector SVG includes label chip background: {svg_conn}");

    let svg_frame = export_svg(&doc, frame, &SvgOptions { outline_text: false });
    assert!(svg_frame.contains("MMMMMM") && svg_frame.contains("WWWWWW"), "frame SVG includes connector label text: {svg_frame}");
    assert!(svg_frame.contains("fill=\"#FFFFFF\""), "frame SVG includes connector label chip background: {svg_frame}");

    let pm_conn = render_export(&doc, c, 1.0).expect("connector PNG renders");
    assert!(pm_conn.width() as f32 >= local_extent.w && pm_conn.height() as f32 >= local_extent.h);
    let pm_frame = render_export(&doc, frame, 1.0).expect("frame PNG renders");
    assert!(pm_frame.width() >= 500 && pm_frame.height() >= 420);
}
