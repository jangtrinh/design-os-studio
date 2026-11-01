//! Canvas click selection and matching-layer selection tests on fixture T.

use super::*;
use crate::panels::menus::Action;
use designos_doc::{
    create_component, create_instance, Affine, AutoLayout, Color, Document, LayoutDirection, Node, NodeId, NodeKind, Paint, Rect as DRect, Sizing, TextStyle,
    Vec2 as DV,
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

#[test]
fn t1_click_nested_label_with_nothing_selected_selects_top_frame_child() {
    let mut t = fixture_t();
    let pt = center_screen(&t.sim, t.a.btn2_label);
    t.sim.t += 1.0;
    t.sim.click(pt);
    assert_eq!(t.sim.app().selection, vec![t.a.list]);
}

#[test]
fn t2_click_group_child_with_nothing_selected_selects_group() {
    let mut t = fixture_t();
    let pt = center_screen(&t.sim, t.a.dot);
    t.sim.t += 1.0;
    t.sim.click(pt);
    assert_eq!(t.sim.app().selection, vec![t.a.g]);
}

#[test]
fn t3_click_nested_layer_inside_section_frame_selects_direct_child_of_section_frame() {
    let mut t = fixture_t();
    assert_eq!(t.sim.app().doc.get(t.card).unwrap().parent, Some(t.c));
    let pt = center_screen(&t.sim, t.title);
    t.sim.t += 1.0;
    t.sim.click(pt);
    assert_eq!(t.sim.app().selection, vec![t.card]);
}

#[test]
fn t4_single_click_or_drag_inside_selected_frame_keeps_and_moves_selected_frame() {
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
fn t5_click_sibling_branch_when_deeply_selected_selects_direct_child_of_shared_ancestor() {
    let mut t = fixture_t();
    t.sim.shell.app.select_only(t.a.btn2_label);
    t.sim.frame(vec![]);

    let pt = center_screen(&t.sim, t.a.btn3_label);
    t.sim.t += 1.0;
    t.sim.click(pt);
    assert_eq!(t.sim.app().selection, vec![t.a.btn3]);
}

#[test]
fn t6_click_other_top_level_frame_when_deeply_selected_selects_outermost_child_of_other_frame() {
    let mut t = fixture_t();
    t.sim.shell.app.select_only(t.a.btn2_label);
    t.sim.frame(vec![]);

    let pt = center_screen(&t.sim, t.b.btn2_label);
    t.sim.t += 1.0;
    t.sim.click(pt);
    assert_eq!(t.sim.app().selection, vec![t.b.list]);
}

#[test]
fn t7_double_click_drills_one_level_per_double_click_and_starts_text_edit_on_text() {
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
fn t8_enter_selects_children_and_shift_enter_selects_parent() {
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
fn t9_cmd_click_deep_selects_innermost_layer() {
    let mut t = fixture_t();
    let pt = center_screen(&t.sim, t.a.btn2_label);
    t.sim.t += 1.0;
    t.sim.mods = Modifiers::COMMAND;
    t.sim.click(pt);
    t.sim.mods = Modifiers::NONE;
    assert_eq!(t.sim.app().selection, vec![t.a.btn2_label]);
}

#[test]
fn t10_cmd_marquee_over_nested_container_selects_leaf_layers_without_ancestors() {
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
fn t11_hover_outline_matches_plain_and_cmd_click_target() {
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
fn t12_shift_click_toggles_matching_nested_layer_across_frames() {
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
fn t13_shift_click_on_non_matching_layer_uses_normal_outside_in_hit() {
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
fn t14_shift_marquee_over_frame_with_nested_selection_adds_only_matching_layers() {
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
fn t15_select_matching_layers_selects_matching_in_other_frames_with_toast() {
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
fn t16_select_matching_layers_in_section_scopes_to_section() {
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
fn t17_right_click_select_layer_submenu_lists_parents_first_and_selects_picked() {
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
fn hover_highlight_preference_toggles_canvas_hover_outline() {
    let mut t = fixture_t();
    let pt = center_screen(&t.sim, t.a.btn2_label);
    t.sim.mv(pt);
    assert_eq!(t.sim.app().hover, Some(t.a.list));
    assert!(!t.sim.app().ui.hide_hover_highlight);

    let ctx = t.sim.ctx.clone();
    crate::panels::menus::run(&mut t.sim.shell.app, &ctx, Action::ToggleHoverHighlight);
    assert!(t.sim.app().ui.hide_hover_highlight);

    crate::panels::menus::run(&mut t.sim.shell.app, &ctx, Action::ToggleHoverHighlight);
    assert!(!t.sim.app().ui.hide_hover_highlight);
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
