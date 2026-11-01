//! Canvas click selection and matching-layer selection tests on fixture T.

use super::*;
use crate::panels::menus::Action;
use designos_doc::{
    create_component, create_instance, Affine, AutoLayout, Color, Document, FrameProps, LayoutDirection, Node, NodeId, NodeKind, Paint, Rect as DRect, Sizing,
    TextStyle, Vec2 as DV,
};

struct SubtreeIds {
    root: NodeId,
    list: NodeId,
    btn1: NodeId,
    btn1_label: NodeId,
    btn2: NodeId,
    btn2_label: NodeId,
    btn3: NodeId,
    btn3_label: NodeId,
    g: NodeId,
    dot: NodeId,
}

struct FixtureT {
    sim: Sim,
    a: SubtreeIds,
    b: SubtreeIds,
    s: NodeId,
    c: NodeId,
    card: NodeId,
    title: NodeId,
}

fn build_button_component(doc: &mut Document, comps_page: NodeId) -> NodeId {
    let id = doc.alloc_id();
    let mut comp = Node::frame(id, DRect::new(0.0, 0.0, 300.0, 40.0));
    comp.name = "Button".into();
    comp.fills = vec![Paint::solid(Color::rgb(0x0D, 0x99, 0xFF))];
    let main = doc.insert(comps_page, comp).unwrap();

    let lbl_id = doc.alloc_id();
    let style = TextStyle { font_size: 16.0, font_weight: 600, fills: vec![Paint::solid(Color::WHITE)], ..Default::default() };
    let mut lbl = Node::text(lbl_id, DRect::new(16.0, 8.0, 120.0, 24.0), "OK", style);
    lbl.name = "Label".into();
    doc.insert(main, lbl).unwrap();

    create_component(doc, main).apply(doc).unwrap();
    main
}

fn find_child_named(doc: &Document, parent: NodeId, name: &str) -> NodeId {
    doc.get(parent)
        .unwrap()
        .children
        .iter()
        .copied()
        .find(|&cid| doc.get(cid).is_some_and(|n| n.name == name))
        .unwrap_or_else(|| panic!("missing child {name} under {parent:?}"))
}

fn build_frame_subtree(doc: &mut Document, page: NodeId, main_btn: NodeId, name: &str, x: f32, y: f32) -> SubtreeIds {
    let fid = doc.alloc_id();
    let mut frame = Node::frame(fid, DRect::new(x, y, 400.0, 400.0));
    frame.name = name.into();
    frame.fills = vec![Paint::solid(Color::rgb(0x1E, 0x1E, 0x24))];
    let root = doc.insert(page, frame).unwrap();

    let lid = doc.alloc_id();
    let mut list_node = Node::frame(lid, DRect::new(20.0, 20.0, 300.0, 160.0));
    list_node.name = "List".into();
    list_node.fills = vec![Paint::solid(Color::rgb(0x27, 0x27, 0x2A))];
    if let Some(fp) = list_node.kind.frame_props_mut() {
        fp.auto_layout = Some(AutoLayout {
            direction: LayoutDirection::Vertical,
            item_spacing: 20.0,
            padding: [0.0; 4],
            primary_sizing: Sizing::Fixed,
            counter_sizing: Sizing::Fixed,
            ..Default::default()
        });
    }
    let list = doc.insert(root, list_node).unwrap();

    let mut btns = Vec::new();
    for (idx, bname) in ["Btn1", "Btn2", "Btn3"].into_iter().enumerate() {
        let cmd = create_instance(doc, main_btn, list, DV::new(0.0, idx as f32 * 60.0));
        let designos_doc::Command::Insert { nodes, .. } = &cmd else {
            panic!("expected Insert command for instance");
        };
        let inst_id = nodes[0].id;
        cmd.apply(doc).unwrap();
        let inst = doc.get_mut(inst_id).unwrap();
        inst.name = bname.into();
        inst.transform = Affine::translate(0.0, idx as f32 * 60.0);
        inst.size = DV::new(300.0, 40.0);
        let lbl = find_child_named(doc, inst_id, "Label");
        btns.push((inst_id, lbl));
    }
    doc.relayout_subtree(list);

    let gid = doc.alloc_id();
    let mut grp = Node::new(gid, NodeKind::Group);
    grp.name = "G".into();
    grp.transform = Affine::translate(20.0, 300.0);
    grp.size = DV::new(40.0, 40.0);
    let g = doc.insert(root, grp).unwrap();

    let did = doc.alloc_id();
    let mut dot_node = Node::ellipse(did, DRect::new(0.0, 0.0, 40.0, 40.0));
    dot_node.name = "Dot".into();
    dot_node.fills = vec![Paint::solid(Color::rgb(0xE3, 0x63, 0x23))];
    let dot = doc.insert(g, dot_node).unwrap();

    SubtreeIds { root, list, btn1: btns[0].0, btn1_label: btns[0].1, btn2: btns[1].0, btn2_label: btns[1].1, btn3: btns[2].0, btn3_label: btns[2].1, g, dot }
}

fn fixture_t() -> FixtureT {
    let mut sim = Sim::new();
    let mut doc = Document::new("Fixture T");
    let page = doc.add_page("Page 1");
    let comps = doc.add_page("Components");

    let main_btn = build_button_component(&mut doc, comps);
    let a = build_frame_subtree(&mut doc, page, main_btn, "A", 0.0, 0.0);
    let b = build_frame_subtree(&mut doc, page, main_btn, "B", 500.0, 0.0);

    let sid = doc.alloc_id();
    let mut sec = Node::frame(sid, DRect::new(0.0, 600.0, 500.0, 300.0));
    sec.name = "S".into();
    sec.kind = NodeKind::Section;
    sec.fills = vec![Paint::solid(Color::rgb(0x18, 0x18, 0x1B))];
    let s = doc.insert(page, sec).unwrap();

    let cid = doc.alloc_id();
    let mut c_frame = Node::frame(cid, DRect::new(20.0, 20.0, 220.0, 220.0));
    c_frame.name = "C".into();
    c_frame.fills = vec![Paint::solid(Color::rgb(0x1E, 0x1E, 0x24))];
    let c = doc.insert(s, c_frame).unwrap();

    let card_id = doc.alloc_id();
    let mut card_frame = Node::frame(card_id, DRect::new(20.0, 20.0, 160.0, 120.0));
    card_frame.name = "Card".into();
    card_frame.fills = vec![Paint::solid(Color::rgb(0x27, 0x27, 0x2A))];
    let card = doc.insert(c, card_frame).unwrap();

    let tid = doc.alloc_id();
    let style = TextStyle { font_size: 16.0, font_weight: 600, fills: vec![Paint::solid(Color::WHITE)], ..Default::default() };
    let mut title_node = Node::text(tid, DRect::new(12.0, 12.0, 120.0, 24.0), "Title", style);
    title_node.name = "Title".into();
    let title = doc.insert(card, title_node).unwrap();

    sim.shell.app.doc = doc;
    sim.shell.app.page = page;
    sim.shell.app.selection.clear();
    sim.shell.app.view.zoom = 0.85;
    sim.shell.app.view.offset = DV::new(-60.0, -60.0);
    sim.shell.app.changed();
    sim.frame(vec![]);
    sim.frame(vec![]);

    FixtureT { sim, a, b, s, c, card, title }
}

fn center_screen(s: &Sim, id: NodeId) -> Pos2 {
    let bb = crate::util::world_aabb(&s.app().doc, id);
    s.screen(bb.x + bb.w * 0.5, bb.y + bb.h * 0.5)
}

fn add_rect(t: &mut FixtureT, parent: NodeId, name: &str, r: DRect) -> NodeId {
    let id = t.sim.shell.app.doc.alloc_id();
    let mut n = Node::rectangle(id, r);
    n.name = name.into();
    n.fills = vec![Paint::solid(Color::rgb(0x44, 0x44, 0x44))];
    t.sim.shell.app.doc.insert(parent, n).unwrap()
}

fn add_group(t: &mut FixtureT, parent: NodeId, name: &str, x: f32, y: f32) -> NodeId {
    let gid = t.sim.shell.app.doc.alloc_id();
    let mut g = Node::new(gid, NodeKind::Group);
    g.name = name.into();
    g.transform = Affine::translate(x, y);
    g.size = DV::new(30.0, 30.0);
    let g = t.sim.shell.app.doc.insert(parent, g).unwrap();
    add_rect(t, g, "Inner", DRect::new(0.0, 0.0, 30.0, 30.0));
    g
}

fn settle(t: &mut FixtureT) {
    t.sim.shell.app.changed();
    t.sim.frame(vec![]);
    t.sim.frame(vec![]);
}

fn shift_drag(t: &mut FixtureT, a: (f32, f32), b: (f32, f32)) {
    let from = t.sim.screen(a.0, a.1);
    let to = t.sim.screen(b.0, b.1);
    t.sim.t += 1.0;
    t.sim.mods = Modifiers::SHIFT;
    t.sim.drag(from, to);
    t.sim.mods = Modifiers::NONE;
}

fn has_canvas_hover_outline_for(s: &Sim, id: NodeId) -> bool {
    let rect = s.app().canvas_rect;
    let corners = crate::canvas::fast_world_corners(&s.app().doc, id).map(|c| s.app().view.to_screen(rect.min, c));
    s.shapes.iter().any(|cs| match &cs.shape {
        egui::Shape::Path(path) => {
            path.closed
                && (path.stroke.width - theme::CANVAS_HOVER_WIDTH).abs() < 1e-4
                && path.points.len() == 4
                && path.points.iter().zip(corners.iter()).all(|(a, b)| (*a - *b).length() < 0.5)
        }
        _ => false,
    })
}

#[test]
fn click_nested_label_with_nothing_selected_selects_top_frame_child() {
    let mut t = fixture_t();
    let pt = center_screen(&t.sim, t.a.btn2_label);
    t.sim.t += 1.0;
    t.sim.click(pt);
    assert_eq!(t.sim.app().selection, vec![t.a.list]);
}

#[test]
fn click_group_child_with_nothing_selected_selects_group() {
    let mut t = fixture_t();
    let pt = center_screen(&t.sim, t.a.dot);
    t.sim.t += 1.0;
    t.sim.click(pt);
    assert_eq!(t.sim.app().selection, vec![t.a.g]);
}

#[test]
fn click_nested_layer_inside_section_frame_selects_direct_child_of_section_frame() {
    let mut t = fixture_t();
    assert_eq!(t.sim.app().doc.get(t.card).unwrap().parent, Some(t.c));
    let pt = center_screen(&t.sim, t.title);
    t.sim.t += 1.0;
    t.sim.click(pt);
    assert_eq!(t.sim.app().selection, vec![t.card]);
}

#[test]
fn single_click_or_drag_inside_selected_frame_keeps_and_moves_selected_frame() {
    let mut t = fixture_t();
    t.sim.shell.app.select_only(t.a.list);
    t.sim.frame(vec![]);

    let pt = center_screen(&t.sim, t.a.btn3_label);
    t.sim.t += 1.0;
    t.sim.click(pt);
    assert_eq!(t.sim.app().selection, vec![t.a.list], "single click must not enter selected container");

    let tx0 = t.sim.app().doc.get(t.a.list).unwrap().transform.tx;
    let to =
        t.sim.screen(crate::util::world_aabb(&t.sim.app().doc, t.a.btn3_label).x + 90.0, crate::util::world_aabb(&t.sim.app().doc, t.a.btn3_label).y + 12.0);
    t.sim.t += 1.0;
    t.sim.drag(pt, to);
    let tx1 = t.sim.app().doc.get(t.a.list).unwrap().transform.tx;
    assert_eq!(t.sim.app().selection, vec![t.a.list]);
    assert!(tx1 > tx0 + 15.0, "dragging over child moves the selected container: {tx0} -> {tx1}");
}

#[test]
fn click_sibling_branch_when_deeply_selected_selects_direct_child_of_shared_ancestor() {
    let mut t = fixture_t();
    t.sim.shell.app.select_only(t.a.btn2_label);
    t.sim.frame(vec![]);

    let pt = center_screen(&t.sim, t.a.btn3_label);
    t.sim.t += 1.0;
    t.sim.click(pt);
    assert_eq!(t.sim.app().selection, vec![t.a.btn3]);
}

#[test]
fn click_other_top_level_frame_when_deeply_selected_selects_outermost_child_of_other_frame() {
    let mut t = fixture_t();
    t.sim.shell.app.select_only(t.a.btn2_label);
    t.sim.frame(vec![]);

    let pt = center_screen(&t.sim, t.b.btn2_label);
    t.sim.t += 1.0;
    t.sim.click(pt);
    assert_eq!(t.sim.app().selection, vec![t.b.list]);
}

#[test]
fn double_click_drills_one_level_per_double_click_and_starts_text_edit_on_text() {
    let mut t = fixture_t();
    let pt = center_screen(&t.sim, t.a.btn2_label);

    // First double-click from nothing selected: click 1 selects A/List, click 2 drills to A/List/Btn2.
    t.sim.t += 1.0;
    t.sim.click(pt);
    t.sim.click(pt);
    assert_eq!(t.sim.app().selection, vec![t.a.btn2]);
    assert!(t.sim.app().canvas.text.is_none());

    // Second double-click: drills into A/List/Btn2/Label and starts text editing immediately.
    t.sim.t += 1.0;
    t.sim.click(pt);
    t.sim.click(pt);
    assert_eq!(t.sim.app().selection, vec![t.a.btn2_label]);
    assert_eq!(t.sim.app().canvas.text.as_ref().map(|s| s.id), Some(t.a.btn2_label));
}

#[test]
fn enter_selects_children_and_shift_enter_selects_parent() {
    let mut t = fixture_t();
    t.sim.shell.app.select_only(t.a.list);
    t.sim.frame(vec![]);

    t.sim.key(Key::Enter);
    assert_eq!(t.sim.app().selection, vec![t.a.btn1, t.a.btn2, t.a.btn3]);

    // Directly after Enter, Shift+Enter climbs back to A/List.
    t.sim.mods = Modifiers::SHIFT;
    t.sim.key(Key::Enter);
    t.sim.mods = Modifiers::NONE;
    assert_eq!(t.sim.app().selection, vec![t.a.list]);

    // Starting from A/List, Shift+Enter climbs to A.
    t.sim.mods = Modifiers::SHIFT;
    t.sim.key(Key::Enter);
    t.sim.mods = Modifiers::NONE;
    assert_eq!(t.sim.app().selection, vec![t.a.root]);
}

#[test]
fn cmd_click_deep_selects_innermost_layer() {
    let mut t = fixture_t();
    let pt = center_screen(&t.sim, t.a.btn2_label);
    t.sim.t += 1.0;
    t.sim.mods = Modifiers::COMMAND;
    t.sim.click(pt);
    t.sim.mods = Modifiers::NONE;
    assert_eq!(t.sim.app().selection, vec![t.a.btn2_label]);
}

#[test]
fn cmd_marquee_over_nested_container_selects_leaf_layers_without_ancestors() {
    let mut t = fixture_t();
    let bb = crate::util::world_aabb(&t.sim.app().doc, t.a.btn1);
    let from = t.sim.screen(-10.0, bb.y - 4.0);
    let to = t.sim.screen(bb.x + bb.w + 4.0, bb.y + bb.h + 4.0);
    t.sim.t += 1.0;
    t.sim.mods = Modifiers::COMMAND;
    t.sim.drag(from, to);
    t.sim.mods = Modifiers::NONE;

    let sel = &t.sim.app().selection;
    assert!(sel.contains(&t.a.btn1_label), "deep marquee must select leaf layers inside Btn1: {sel:?}");
    assert!(!sel.contains(&t.a.root) && !sel.contains(&t.a.list) && !sel.contains(&t.a.btn1), "deep marquee must not select ancestor containers: {sel:?}");
}

#[test]
fn hover_outline_matches_plain_and_cmd_click_target() {
    let mut t = fixture_t();
    let pt = center_screen(&t.sim, t.a.btn2_label);

    t.sim.mods = Modifiers::NONE;
    t.sim.mv(pt);
    assert_eq!(t.sim.app().hover, Some(t.a.list));

    t.sim.mods = Modifiers::COMMAND;
    t.sim.mv(pt);
    assert_eq!(t.sim.app().hover, Some(t.a.btn2_label));
    t.sim.mods = Modifiers::NONE;
}

#[test]
fn shift_click_toggles_matching_nested_layer_across_frames() {
    let mut t = fixture_t();
    t.sim.shell.app.select_only(t.a.btn2_label);
    t.sim.frame(vec![]);

    let pt = center_screen(&t.sim, t.b.btn2_label);
    t.sim.t += 1.0;
    t.sim.mods = Modifiers::SHIFT;
    t.sim.click(pt);
    assert_eq!(t.sim.app().selection, vec![t.a.btn2_label, t.b.btn2_label]);

    // Shift-clicking the matching layer again removes it from the selection.
    t.sim.t += 1.0;
    t.sim.click(pt);
    t.sim.mods = Modifiers::NONE;
    assert_eq!(t.sim.app().selection, vec![t.a.btn2_label]);
}

#[test]
fn shift_click_on_non_matching_layer_uses_normal_outside_in_hit() {
    let mut t = fixture_t();
    t.sim.shell.app.select_only(t.a.btn2_label);
    t.sim.frame(vec![]);

    let pt = center_screen(&t.sim, t.b.dot);
    t.sim.t += 1.0;
    t.sim.mods = Modifiers::SHIFT;
    t.sim.click(pt);
    t.sim.mods = Modifiers::NONE;
    assert_eq!(t.sim.app().selection, vec![t.a.btn2_label, t.b.g]);
}

#[test]
fn shift_marquee_over_frame_with_nested_selection_adds_only_matching_layers() {
    let mut t = fixture_t();
    t.sim.shell.app.select_only(t.a.btn2_label);
    t.sim.frame(vec![]);

    let bb = crate::util::world_aabb(&t.sim.app().doc, t.b.root);
    let from = t.sim.screen(bb.x - 10.0, bb.y - 10.0);
    let to = t.sim.screen(bb.x + bb.w + 10.0, bb.y + bb.h + 10.0);
    t.sim.t += 1.0;
    t.sim.mods = Modifiers::SHIFT;
    t.sim.drag(from, to);
    t.sim.mods = Modifiers::NONE;

    assert_eq!(t.sim.app().selection, vec![t.a.btn2_label, t.b.btn2_label]);
}

#[test]
fn select_matching_layers_selects_matching_in_other_frames_with_toast() {
    let mut t = fixture_t();
    t.sim.shell.app.select_only(t.a.btn2_label);
    t.sim.frame(vec![]);

    t.sim.mods = Modifiers { alt: true, ctrl: true, command: true, ..Default::default() };
    t.sim.key(Key::A);
    t.sim.mods = Modifiers::NONE;

    assert_eq!(t.sim.app().selection, vec![t.a.btn2_label, t.b.btn2_label]);
    assert!(!t.sim.app().selection.contains(&t.a.btn1_label), "must never add Btn1/Label from the same frame");
    assert_eq!(t.sim.app().toast.as_ref().map(|toast| toast.text.as_str()), Some("1 matching layers found, 2 items selected"));
}

#[test]
fn select_matching_layers_in_section_scopes_to_section() {
    let mut t = fixture_t();
    // Add a top-level Card/Title outside section S so a page-wide search would wrongly find it.
    let page = t.sim.app().page;
    let out_card_id = t.sim.shell.app.doc.alloc_id();
    let mut out_card = Node::frame(out_card_id, DRect::new(560.0, 620.0, 160.0, 120.0));
    out_card.name = "Card".into();
    let out_card = t.sim.shell.app.doc.insert(page, out_card).unwrap();
    let out_title_id = t.sim.shell.app.doc.alloc_id();
    let style = TextStyle { font_size: 16.0, font_weight: 600, fills: vec![Paint::solid(Color::WHITE)], ..Default::default() };
    let mut out_title = Node::text(out_title_id, DRect::new(12.0, 12.0, 120.0, 24.0), "Title", style.clone());
    out_title.name = "Title".into();
    let _out_title = t.sim.shell.app.doc.insert(out_card, out_title).unwrap();
    t.sim.shell.app.changed();
    t.sim.frame(vec![]);

    // With only one frame C inside section S, nothing in S matches S/C/Card/Title.
    t.sim.shell.app.select_only(t.title);
    let ctx = t.sim.ctx.clone();
    crate::panels::menus::run(&mut t.sim.shell.app, &ctx, Action::SelectMatching);
    assert_eq!(t.sim.app().selection, vec![t.title]);
    assert_eq!(t.sim.app().toast.as_ref().map(|toast| toast.text.as_str()), Some("No matching layers to select in this section"));

    // Add a second frame C2 inside section S with Card/Title; now it matches inside S only.
    let c2_id = t.sim.shell.app.doc.alloc_id();
    let mut c2 = Node::frame(c2_id, DRect::new(260.0, 20.0, 220.0, 220.0));
    c2.name = "C2".into();
    let c2 = t.sim.shell.app.doc.insert(t.s, c2).unwrap();
    let card2_id = t.sim.shell.app.doc.alloc_id();
    let mut card2 = Node::frame(card2_id, DRect::new(20.0, 20.0, 160.0, 120.0));
    card2.name = "Card".into();
    let card2 = t.sim.shell.app.doc.insert(c2, card2).unwrap();
    let title2_id = t.sim.shell.app.doc.alloc_id();
    let mut title2 = Node::text(title2_id, DRect::new(12.0, 12.0, 120.0, 24.0), "Title", style);
    title2.name = "Title".into();
    let title2 = t.sim.shell.app.doc.insert(card2, title2).unwrap();
    t.sim.shell.app.changed();
    t.sim.frame(vec![]);

    crate::panels::menus::run(&mut t.sim.shell.app, &ctx, Action::SelectMatching);
    assert_eq!(t.sim.app().selection, vec![t.title, title2]);
}

#[test]
fn right_click_select_layer_submenu_lists_parents_first_and_selects_picked() {
    let mut t = fixture_t();
    let pt = center_screen(&t.sim, t.a.btn2_label);
    t.sim.right_click(pt);
    assert!(t.sim.app().ui.ctx_menu.is_some());

    let top_labels = crate::panels::context::labels(t.sim.app());
    let sub_idx = top_labels.iter().position(|l| l == "Select layer").expect("plain right-click menu must include 'Select layer' submenu");

    let sub_row = t.sim.rect_of(&format!("menu-row-0-{sub_idx}"));
    t.sim.mv(sub_row.center());
    t.sim.frame(vec![]);

    // Submenu must open with 4 rows in Layers-panel order (parents first: A, List, Btn2, Label).
    let mut row_rects = Vec::new();
    for i in 0..4 {
        row_rects.push(t.sim.rect_of(&format!("menu-row-1-{i}")));
    }
    let sub_labels = crate::panels::context::select_layer_labels(t.sim.app());
    assert_eq!(sub_labels, vec!["A", "List", "Btn2", "Label"]);

    // Clicking "Btn2" (index 2) selects A/List/Btn2.
    t.sim.click(row_rects[2].center());
    assert_eq!(t.sim.app().selection, vec![t.a.btn2]);
}

#[test]
fn comment_pin_inside_nested_frame_attaches_to_nested_layer() {
    let mut t = fixture_t();
    t.sim.key(Key::C);
    assert_eq!(t.sim.app().tool, Tool::Comment);

    let pt = center_screen(&t.sim, t.a.btn2_label);
    t.sim.t += 1.0;
    t.sim.click(pt);

    let composer = t.sim.app().comments.composer.as_ref().expect("clicking with Comment tool opens comment composer");
    assert_eq!(composer.anchor.node, Some(t.a.btn2));
}

#[test]
fn hover_highlight_preference_toggles_canvas_hover_outline_and_persists_across_shells() {
    let lib_dir = std::env::temp_dir().join(format!("designos-hover-pref-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&lib_dir);

    let mut t = fixture_t();
    t.sim.shell.app.attach_library(lib_dir.clone());
    let pt = center_screen(&t.sim, t.a.btn2_label);
    t.sim.mv(pt);
    assert_eq!(t.sim.app().hover, Some(t.a.list));
    assert!(!t.sim.app().ui.hide_hover_highlight);
    assert!(has_canvas_hover_outline_for(&t.sim, t.a.list), "hover outline stroke must be drawn when enabled");

    let ctx = t.sim.ctx.clone();
    crate::panels::menus::run(&mut t.sim.shell.app, &ctx, Action::ToggleHoverHighlight);
    t.sim.frame(vec![]);
    assert!(t.sim.app().ui.hide_hover_highlight);
    assert!(!has_canvas_hover_outline_for(&t.sim, t.a.list), "canvas hover outline stroke must be suppressed when disabled");

    // Hovering a row in the Layers panel still draws its outline on the canvas.
    t.sim.shell.app.hover_layer = Some(t.a.list);
    t.sim.frame(vec![]);
    assert!(has_canvas_hover_outline_for(&t.sim, t.a.list), "layers panel hover outline must still draw when canvas hover highlight is disabled");
    t.sim.shell.app.hover_layer = None;

    // A second shell built against the same library root loads the persisted preference.
    let second = Sim::built(Options { library: Some(lib_dir.clone()), ..Options::default() });
    assert!(second.app().ui.hide_hover_highlight, "disabled hover highlight preference must persist across shell launches");

    crate::panels::menus::run(&mut t.sim.shell.app, &ctx, Action::ToggleHoverHighlight);
    t.sim.frame(vec![]);
    assert!(!t.sim.app().ui.hide_hover_highlight);
    assert!(has_canvas_hover_outline_for(&t.sim, t.a.list));

    let third = Sim::built(Options { library: Some(lib_dir.clone()), ..Options::default() });
    assert!(!third.app().ui.hide_hover_highlight, "re-enabled hover highlight preference must persist across shell launches");
    let _ = std::fs::remove_dir_all(&lib_dir);
}

#[test]
fn select_all_with_same_text_properties_and_variant() {
    use designos_doc::find::MatchCriteria;

    let mut t = fixture_t();
    let ctx = t.sim.ctx.clone();

    t.sim.shell.app.select_only(t.a.btn2_label);
    crate::panels::menus::run(&mut t.sim.shell.app, &ctx, Action::SelectSame(MatchCriteria::TextProperties));
    assert!(t.sim.app().selection.contains(&t.a.btn1_label));
    assert!(t.sim.app().selection.contains(&t.a.btn2_label));
    assert!(t.sim.app().selection.contains(&t.b.btn2_label));
    assert!(t.sim.app().selection.contains(&t.title));

    // Select all with same instance from a layer inside an instance selects all Button instances.
    t.sim.shell.app.select_only(t.a.btn2_label);
    assert!(designos_doc::find::can_select_same_instance(&t.sim.app().doc, &t.sim.app().selection));
    assert!(!designos_doc::find::can_select_same_variant(&t.sim.app().doc, &t.sim.app().selection));
    crate::panels::menus::run(&mut t.sim.shell.app, &ctx, Action::SelectSame(MatchCriteria::Instance));
    assert_eq!(t.sim.app().selection.len(), 6, "all 6 Button instances across A and B should be selected: {:?}", t.sim.app().selection);
}

#[test]
fn select_layer_submenu_includes_locked_layers_and_can_select_them() {
    let mut t = fixture_t();
    t.sim.shell.app.doc.get_mut(t.a.btn2).unwrap().locked = true;
    t.sim.shell.app.changed();
    t.sim.frame(vec![]);

    let pt = center_screen(&t.sim, t.a.btn2_label);
    t.sim.right_click(pt);
    assert!(t.sim.app().ui.ctx_menu.is_some());

    let top_labels = crate::panels::context::labels(t.sim.app());
    let sub_idx = top_labels.iter().position(|l| l == "Select layer").unwrap();
    let sub_row = t.sim.rect_of(&format!("menu-row-0-{sub_idx}"));
    t.sim.mv(sub_row.center());
    t.sim.frame(vec![]);

    let btn2_row = t.sim.rect_of("menu-row-1-2");
    t.sim.click(btn2_row.center());
    assert_eq!(t.sim.app().selection, vec![t.a.btn2]);
}

#[test]
fn shift_marquee_with_direct_child_selected_extends_to_siblings_and_plain_marquee_hits_nested_frame() {
    let mut t = fixture_t();

    // Plain marquee partly over A/List selects A/List without descending into its children.
    let from = t.sim.screen(-30.0, 30.0);
    let to = t.sim.screen(100.0, 100.0);
    t.sim.t += 1.0;
    t.sim.drag(from, to);
    assert_eq!(t.sim.app().selection, vec![t.a.list]);

    // A/G is a direct child of top-level frame A (not nested), so Shift+drag partly over A/List adds A/List.
    t.sim.shell.app.select_only(t.a.g);
    t.sim.frame(vec![]);
    shift_drag(&mut t, (-30.0, 30.0), (100.0, 100.0));
    assert_eq!(t.sim.app().selection, vec![t.a.g, t.a.list]);

    // Shift+drag from A/G over a sibling group G2 or sibling rectangle R inside A adds it as well.
    let a = t.a.root;
    let g2 = add_group(&mut t, a, "G2", 100.0, 250.0);
    settle(&mut t);
    t.sim.shell.app.select_only(t.a.g);
    t.sim.frame(vec![]);
    shift_drag(&mut t, (-30.0, 240.0), (115.0, 270.0));
    assert_eq!(t.sim.app().selection, vec![t.a.g, g2]);

    let r = add_rect(&mut t, a, "R", DRect::new(100.0, 355.0, 30.0, 30.0));
    settle(&mut t);
    t.sim.shell.app.select_only(t.a.g);
    t.sim.frame(vec![]);
    shift_drag(&mut t, (-30.0, 355.0), (115.0, 375.0));
    assert_eq!(t.sim.app().selection, vec![t.a.g, r]);
}

#[test]
fn shift_marquee_with_nested_selection_toggles_only_matching_layers_and_caches_candidates() {
    let mut t = fixture_t();
    let a = t.a.root;
    let _r = add_rect(&mut t, a, "R", DRect::new(100.0, 250.0, 30.0, 30.0));
    settle(&mut t);

    // A/G/Dot is deeper than click depth (nested), so Shift+drag over non-matching sibling leaf R adds nothing.
    t.sim.shell.app.select_only(t.a.dot);
    t.sim.frame(vec![]);
    shift_drag(&mut t, (-30.0, 240.0), (115.0, 270.0));
    assert_eq!(t.sim.app().selection, vec![t.a.dot]);

    // When both matching labels are already selected, Shift+drag over frame B toggles B/List/Btn2/Label off and adds nothing else.
    t.sim.shell.app.selection = vec![t.a.btn2_label, t.b.btn2_label];
    t.sim.frame(vec![]);
    shift_drag(&mut t, (490.0, -10.0), (910.0, 410.0));
    assert_eq!(t.sim.app().selection, vec![t.a.btn2_label]);
}

#[test]
fn eyedropper_shift_click_samples_deep_nested_token_and_vector_edit_exits_on_nested_sibling() {
    let mut t = fixture_t();
    for (key, col) in [("S-List", Color::rgb(10, 20, 30)), ("S-Btn", Color::rgb(200, 100, 50))] {
        t.sim.shell.app.doc.styles.insert(key.into(), designos_doc::Style::Fill(vec![Paint::solid(col)]));
    }
    t.sim.shell.app.doc.get_mut(t.a.list).unwrap().style_refs.insert("fill".into(), "S-List".into());
    t.sim.shell.app.doc.get_mut(t.a.btn2).unwrap().style_refs.insert("fill".into(), "S-Btn".into());
    settle(&mut t);

    let dot = t.b.dot;
    t.sim.shell.app.select_only(dot);
    t.sim.frame(vec![]);
    t.sim.shell.app.props.eyedropper = Some(crate::panels::properties::EyedropTarget::Fill(0));
    t.sim.frame(vec![]);

    let pt = center_screen(&t.sim, t.a.btn2_label);
    t.sim.mods = Modifiers::SHIFT;
    t.sim.mv(pt);
    t.sim.t += 1.0;
    t.sim.click(pt);
    t.sim.mods = Modifiers::NONE;
    let sampled = t.sim.app().doc.get(dot).and_then(|n| n.style_refs.get("fill").cloned());
    assert_eq!(sampled.as_deref(), Some("S-Btn"), "eyedropper Shift+click over nested label must sample nearest styled ancestor Btn2, not outer List");

    // Vector edit mode exits when clicking a shape inside an unclipped sibling nested frame overlapping the edited vector's interior.
    let a_root = t.a.root;
    let vrect = add_rect(&mut t, a_root, "EditedVec", DRect::new(20.0, 200.0, 200.0, 120.0));
    let uf_id = t.sim.shell.app.doc.alloc_id();
    let mut uf = Node::frame(uf_id, DRect::new(20.0, 200.0, 10.0, 10.0));
    uf.name = "Unclipped".into();
    uf.fills.clear();
    if let Some(fp) = uf.kind.frame_props_mut() {
        fp.clips_content = false;
    }
    let uf = t.sim.shell.app.doc.insert(a_root, uf).unwrap();
    let overlay_child = add_rect(&mut t, uf, "OverlayChild", DRect::new(80.0, 40.0, 40.0, 40.0));
    settle(&mut t);

    crate::canvas::vector::enter(&mut t.sim.shell.app, vrect);
    t.sim.frame(vec![]);
    assert_eq!(t.sim.app().canvas.vedit.as_ref().map(|v| v.id), Some(vrect));

    let child_pt = center_screen(&t.sim, overlay_child);
    t.sim.t += 1.0;
    t.sim.click(child_pt);
    assert!(t.sim.app().canvas.vedit.is_none(), "clicking a nested sibling shape overlapping the edited vector must exit vector edit mode");
}

#[test]
fn shift_hover_and_selection_change_update_hover_target_to_match_press_target() {
    let mut t = fixture_t();
    t.sim.shell.app.select_only(t.a.btn2_label);
    t.sim.frame(vec![]);

    let pt = center_screen(&t.sim, t.b.btn2_label);
    t.sim.mods = Modifiers::SHIFT;
    t.sim.mv(pt);
    assert_eq!(t.sim.app().hover, Some(t.b.btn2_label), "Shift-hover over matching layer must preview the deep matching layer");
    t.sim.mods = Modifiers::NONE;
    t.sim.mv(pt);
    assert_eq!(t.sim.app().hover, Some(t.b.list), "releasing Shift must refresh hover back to the normal outside-in target");

    // Changing the selection at a stationary pointer refreshes the cached hover target to the new selection depth.
    let pt_a = center_screen(&t.sim, t.a.btn2_label);
    t.sim.shell.app.clear_selection();
    t.sim.mv(pt_a);
    assert_eq!(t.sim.app().hover, Some(t.a.list));

    t.sim.shell.app.select_only(t.a.btn1);
    t.sim.frame(vec![]);
    assert_eq!(t.sim.app().hover, Some(t.a.btn2), "selection change without pointer movement must refresh hover inside the newly entered container");
}

#[test]
fn select_layer_submenu_orders_frontmost_sibling_first_caps_deepest_and_skips_layers_panel_menu() {
    let mut t = fixture_t();
    let a = t.a.root;
    let _back = add_rect(&mut t, a, "Back", DRect::new(330.0, 300.0, 40.0, 40.0));
    let _front = add_rect(&mut t, a, "Front", DRect::new(340.0, 310.0, 40.0, 40.0));
    settle(&mut t);

    let overlap_pt = t.sim.screen(355.0, 325.0);
    t.sim.right_click(overlap_pt);
    let sub = crate::panels::context::select_layer_labels(t.sim.app());
    assert_eq!(sub, vec!["A", "Front", "Back"], "overlapping siblings must list front-most sibling first after shared parent");
    t.sim.shell.app.ui.ctx_menu = None;
    t.sim.frame(vec![]);

    // A context menu opened from the Layers panel (ctx_menu_page = None) never gains a canvas Select layer row when the pointer moves over the canvas.
    t.sim.shell.app.select_only(t.a.btn2);
    let panel_pt = Pos2::new(t.sim.app().canvas_rect.left() - 80.0, 300.0);
    t.sim.mv(panel_pt);
    t.sim.shell.app.ui.ctx_menu = Some(panel_pt);
    t.sim.shell.app.ui.ctx_menu_page = None;
    t.sim.frame(vec![]);
    let over_canvas = center_screen(&t.sim, t.a.btn2_label);
    t.sim.mv(over_canvas);
    t.sim.frame(vec![]);
    let top_labels = crate::panels::context::labels(t.sim.app());
    assert!(!top_labels.iter().any(|l| l == "Select layer"), "Layers-panel context menu must not include canvas Select layer submenu: {top_labels:?}");
    assert!(crate::panels::context::select_layer_labels(t.sim.app()).is_empty());
    t.sim.shell.app.ui.ctx_menu = None;
    t.sim.frame(vec![]);

    // A 35-deep container stack under Select layer keeps the outermost containers and trims the deepest layers at 32.
    let page = t.sim.app().page;
    let mut parent = page;
    let mut leaf = page;
    for depth in 0..35 {
        let id = t.sim.shell.app.doc.alloc_id();
        let mut f = Node::frame(id, DRect::new(if depth == 0 { 1100.0 } else { 0.0 }, 0.0, 100.0, 100.0));
        f.name = format!("Depth{depth}");
        f.fills = vec![Paint::solid(Color::rgb(0x30, 0x30, 0x36))];
        parent = t.sim.shell.app.doc.insert(parent, f).unwrap();
        leaf = parent;
    }
    settle(&mut t);
    let deep_pt = center_screen(&t.sim, leaf);
    t.sim.right_click(deep_pt);
    let capped = crate::panels::context::select_layer_labels(t.sim.app());
    assert_eq!(capped.len(), 32);
    assert_eq!(capped.first().map(String::as_str), Some("Depth0"), "outermost container must remain first when capped at 32");
    assert_eq!(capped.last().map(String::as_str), Some("Depth31"), "deepest layers past 32 must be dropped");
}

#[test]
fn select_matching_layers_enforces_symmetric_section_scope_sibling_index_and_text_override_rules() {
    let mut t = fixture_t();
    let page = t.sim.app().page;

    // Add a top-level frame C3 outside section S with the exact same relative path Card/Title.
    let c3_id = t.sim.shell.app.doc.alloc_id();
    let mut c3 = Node::frame(c3_id, DRect::new(1000.0, 600.0, 220.0, 220.0));
    c3.name = "C3".into();
    let c3 = t.sim.shell.app.doc.insert(page, c3).unwrap();
    let card_id = t.sim.shell.app.doc.alloc_id();
    let mut card = Node::frame(card_id, DRect::new(20.0, 20.0, 160.0, 120.0));
    card.name = "Card".into();
    let card = t.sim.shell.app.doc.insert(c3, card).unwrap();
    let tid = t.sim.shell.app.doc.alloc_id();
    let style = TextStyle { font_size: 16.0, font_weight: 600, fills: vec![Paint::solid(Color::WHITE)], ..Default::default() };
    let mut tn = Node::text(tid, DRect::new(12.0, 12.0, 120.0, 24.0), "Title", style.clone());
    tn.name = "Title".into();
    let outside_title = t.sim.shell.app.doc.insert(card, tn).unwrap();
    settle(&mut t);

    let ctx = t.sim.ctx.clone();
    // From inside section S, outside C3/Card/Title is ignored.
    t.sim.shell.app.select_only(t.title);
    crate::panels::menus::run(&mut t.sim.shell.app, &ctx, Action::SelectMatching);
    assert_eq!(t.sim.app().selection, vec![t.title]);

    // Symmetric: from outside section S on the page, S/C/Card/Title inside section S is also ignored.
    t.sim.shell.app.select_only(outside_title);
    t.sim.key(Key::Q);
    assert_eq!(t.sim.app().selection, vec![outside_title], "layer outside any section must not match a layer inside section S");

    // Same-parent duplicate sibling index tie-break: two identically named rectangles in A and B at identical coordinates.
    let a_root = t.a.root;
    let b_root = t.b.root;
    let a_dup0 = add_rect(&mut t, a_root, "DupBadge", DRect::new(340.0, 20.0, 20.0, 20.0));
    let _a_dup1 = add_rect(&mut t, a_root, "DupBadge", DRect::new(340.0, 20.0, 20.0, 20.0));
    let b_dup0 = add_rect(&mut t, b_root, "DupBadge", DRect::new(340.0, 20.0, 20.0, 20.0));
    let b_dup1 = add_rect(&mut t, b_root, "DupBadge", DRect::new(340.0, 20.0, 20.0, 20.0));
    settle(&mut t);
    t.sim.shell.app.select_only(a_dup0);
    t.sim.key(Key::Q);
    assert_eq!(t.sim.app().selection, vec![a_dup0, b_dup0], "same-parent duplicates at identical position must tie-break by sibling index");
    assert!(!t.sim.app().selection.contains(&b_dup1));

    // Never-renamed text layers whose contents match their names: tie-break by nearest relative position rather than identical text content.
    let add_auto_text = |t: &mut FixtureT, parent: NodeId, text: &str, y: f32| -> NodeId {
        let id = t.sim.shell.app.doc.alloc_id();
        let mut n = Node::text(id, DRect::new(340.0, y, 40.0, 20.0), text, style.clone());
        n.name = text.into();
        t.sim.shell.app.doc.insert(parent, n).unwrap()
    };
    let a_txt_top = add_auto_text(&mut t, a_root, "AlphaCopy", 60.0);
    let _a_txt_bot = add_auto_text(&mut t, a_root, "BetaCopy", 180.0);
    let b_txt_top = add_auto_text(&mut t, b_root, "GammaCopy", 60.0);
    let b_txt_bot = add_auto_text(&mut t, b_root, "AlphaCopy", 180.0);
    settle(&mut t);
    t.sim.shell.app.select_only(a_txt_top);
    t.sim.key(Key::Q);
    assert_eq!(t.sim.app().selection, vec![a_txt_top, b_txt_top], "never-renamed text layers must tie-break by nearest relative position");
    assert!(!t.sim.app().selection.contains(&b_txt_bot));

    // Instance text child whose text was overridden (changing its node name to the override text) still matches via the main component's layer name.
    if let Some(n) = t.sim.shell.app.doc.get_mut(t.a.btn2_label) {
        n.name = "CustomCTA".into();
        n.component.overrides.insert(designos_doc::OverrideField::Name);
        if let NodeKind::Text(tp) = &mut n.kind {
            tp.content = "CustomCTA".into();
        }
    }
    settle(&mut t);
    t.sim.shell.app.select_only(t.a.btn2_label);
    t.sim.key(Key::Q);
    assert_eq!(
        t.sim.app().selection,
        vec![t.a.btn2_label, t.b.btn2_label],
        "instance text child with overridden text must still match counterpart using main component layer name"
    );
}

#[test]
fn select_same_instance_matches_main_component_while_select_same_variant_matches_component_set() {
    use designos_doc::find::MatchCriteria;

    let mut t = fixture_t();
    let comps = t.sim.app().doc.pages[1];
    let page = t.sim.app().page;

    let set_id = t.sim.shell.app.doc.alloc_id();
    let mut set = Node::new(set_id, NodeKind::ComponentSet(FrameProps::default()));
    set.name = "Chip".into();
    set.transform = Affine::translate(0.0, 100.0);
    set.size = DV::new(300.0, 100.0);
    let set = t.sim.shell.app.doc.insert(comps, set).unwrap();

    let mut variants = Vec::new();
    for (idx, vname) in ["Size=sm", "Size=lg"].into_iter().enumerate() {
        let vid = t.sim.shell.app.doc.alloc_id();
        let mut v = Node::new(vid, NodeKind::Component(FrameProps::default()));
        v.name = vname.into();
        v.transform = Affine::translate(idx as f32 * 120.0, 0.0);
        v.size = DV::new(100.0, 40.0);
        let v = t.sim.shell.app.doc.insert(set, v).unwrap();
        add_rect(&mut t, v, "Bg", DRect::new(0.0, 0.0, 100.0, 40.0));
        variants.push(v);
    }

    let mut instances = Vec::new();
    for (idx, comp_id) in [variants[0], variants[0], variants[1]].into_iter().enumerate() {
        let cmd = create_instance(&mut t.sim.shell.app.doc, comp_id, page, DV::new(1000.0, idx as f32 * 60.0));
        let designos_doc::Command::Insert { nodes, .. } = &cmd else {
            panic!("expected Insert command");
        };
        let inst_id = nodes[0].id;
        cmd.apply(&mut t.sim.shell.app.doc).unwrap();
        instances.push(inst_id);
    }
    settle(&mut t);

    let ctx = t.sim.ctx.clone();
    t.sim.shell.app.select_only(instances[0]);
    assert!(designos_doc::find::can_select_same_instance(&t.sim.app().doc, &t.sim.app().selection));
    assert!(designos_doc::find::can_select_same_variant(&t.sim.app().doc, &t.sim.app().selection));

    crate::panels::menus::run(&mut t.sim.shell.app, &ctx, Action::SelectSame(MatchCriteria::Instance));
    assert_eq!(t.sim.app().selection, vec![instances[1], instances[0]], "SelectSame(Instance) must match only instances of the same main component");

    t.sim.shell.app.select_only(instances[0]);
    crate::panels::menus::run(&mut t.sim.shell.app, &ctx, Action::SelectSame(MatchCriteria::Variant));
    assert_eq!(t.sim.app().selection, vec![instances[2], instances[1], instances[0]], "SelectSame(Variant) must match all instances across the component set");
}
