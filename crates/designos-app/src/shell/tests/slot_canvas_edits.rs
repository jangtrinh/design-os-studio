//! Every canvas edit that parents into or out of an instance slot obeys the slot's limits.

use super::*;
use designos_doc::{Command, Node, OverrideField, SlotPropConfig};

struct Rig {
    s: Sim,
    inst: NodeId,
    slot: NodeId,
    badge: NodeId,
}

/// "Get started" instance with a slot frame of its own, `fill` badge instances inside it.
fn rig(min: Option<u32>, max: Option<u32>, preferred_only: bool, fill: usize) -> Rig {
    let mut s = Sim::with(Options { select: Some("Get started".into()), ..Default::default() });
    let main = s.by_name("Primary button");
    let inst = s.by_name("Get started");
    let page = s.app().page;
    let run = |app: &mut App, cmd: Command| {
        let mut tx = crate::edit::Tx::new();
        let _ = tx.cmd(&mut app.doc, cmd);
        app.run_tx(tx);
    };
    let badge = s.shell.app.doc.alloc_id();
    let mut node = Node::frame(badge, designos_doc::Rect::new(0.0, 0.0, 20.0, 20.0));
    node.name = "Badge".into();
    run(&mut s.shell.app, Command::Insert { parent: page, index: 0, nodes: vec![node] });
    let cmd = designos_doc::create_component(&mut s.shell.app.doc, badge);
    run(&mut s.shell.app, cmd);

    let slot_id = s.shell.app.doc.alloc_id();
    let mut frame = Node::frame(slot_id, designos_doc::Rect::new(8.0, 8.0, 80.0, 40.0));
    frame.name = "Extra".into();
    // topmost in the button, so a pointer over the slot reaches the slot and its content
    run(&mut s.shell.app, Command::Insert { parent: main, index: 3, nodes: vec![frame] });
    let cmd = designos_doc::convert_to_slot(&mut s.shell.app.doc, slot_id);
    run(&mut s.shell.app, cmd);
    let config = SlotPropConfig {
        min_children: min,
        max_children: max,
        allow_preferred_values_only: preferred_only.then_some(true),
        preferred: if preferred_only { vec![badge] } else { Vec::new() },
        ..Default::default()
    };
    let cmd = designos_doc::set_slot_config(&mut s.shell.app.doc, main, "Extra", config);
    run(&mut s.shell.app, cmd);

    let slot = child_of(&s.app().doc, inst, "Extra");
    let at = center(s.app(), slot);
    for _ in 0..fill {
        s.shell.app.selection = vec![slot];
        assert!(s.shell.app.insert_instance(badge, at).is_some());
    }
    s.shell.app.selection = Vec::new();
    s.shell.app.toast = None;
    assert_eq!(s.app().doc.get(slot).unwrap().children.len(), fill);
    assert_eq!(crate::canvas::create::container_at(&s.app().doc, s.app().page, at, &[]), slot, "the slot is the container under its own centre");
    Rig { s, inst, slot, badge }
}

fn center(app: &App, id: NodeId) -> DV {
    crate::util::rect_center(&crate::util::world_aabb(&app.doc, id))
}

fn child_of(d: &designos_doc::Document, parent: NodeId, name: &str) -> NodeId {
    *d.get(parent).unwrap().children.iter().find(|c| d.get(**c).unwrap().name == name).unwrap_or_else(|| panic!("no child {name}"))
}

/// Structure and placement of every node, so "unchanged" means unchanged.
fn snapshot(app: &App) -> Vec<String> {
    let mut v: Vec<String> = app
        .doc
        .nodes
        .values()
        .map(|n| format!("{:?} {:?} {:?} {} {} {:?}", n.id, n.parent, n.children, n.transform.tx, n.transform.ty, n.component.overrides))
        .collect();
    v.sort();
    v
}

/// Draws a rectangle on empty canvas with the rectangle tool; returns it.
fn draw_rect(s: &mut Sim, from: (f32, f32), to: (f32, f32)) -> NodeId {
    s.key(Key::R);
    let (a, b) = (s.screen(from.0, from.1), s.screen(to.0, to.1));
    s.mods = Modifiers::COMMAND;
    s.drag(a, b);
    s.mods = Modifiers::NONE;
    let id = s.app().selection[0];
    assert_eq!(s.app().doc.get(id).unwrap().parent, Some(s.app().page));
    id
}

fn assert_rejected(s: &Sim, before: &[String], text: &str) {
    let after = snapshot(s.app());
    let changed: Vec<&String> = after.iter().filter(|l| !before.contains(l)).chain(before.iter().filter(|l| !after.contains(l))).collect();
    assert!(changed.is_empty(), "a rejected edit leaves the document untouched, but these nodes differ: {changed:?}");
    let toast = s.app().toast.as_ref().expect("a rejected edit explains itself");
    assert!(toast.error && toast.text.contains(text), "toast: {:?}", toast.text);
}

fn has_slot_override(app: &App, slot: NodeId) -> bool {
    app.doc.get(slot).unwrap().component.overrides.contains(&OverrideField::SlotContent)
}

#[test]
fn dragging_a_node_into_a_full_slot_is_rejected() {
    let mut r = rig(None, Some(2), false, 2);
    let rect = draw_rect(&mut r.s, (300.0, 800.0), (380.0, 860.0));
    r.s.shell.app.selection = Vec::new();
    r.s.shell.app.toast = None;
    let before = snapshot(r.s.app());
    let (a, b) = (center(r.s.app(), rect), center(r.s.app(), r.slot));
    r.s.drag(r.s.screen(a.x, a.y), r.s.screen(b.x, b.y));
    assert_rejected(&r.s, &before, "maximum");
}

#[test]
fn dragging_a_plain_node_into_a_preferred_only_slot_is_rejected() {
    let mut r = rig(None, Some(2), true, 0);
    let rect = draw_rect(&mut r.s, (300.0, 800.0), (380.0, 860.0));
    r.s.shell.app.selection = Vec::new();
    r.s.shell.app.toast = None;
    let before = snapshot(r.s.app());
    let (a, b) = (center(r.s.app(), rect), center(r.s.app(), r.slot));
    r.s.drag(r.s.screen(a.x, a.y), r.s.screen(b.x, b.y));
    assert_rejected(&r.s, &before, "preferred");
    assert!(!has_slot_override(r.s.app(), r.slot), "a rejected drop does not mark the slot as changed");
}

#[test]
fn dragging_a_node_into_a_slot_with_room_adds_it_marks_the_slot_and_undoes_in_one_step() {
    let mut r = rig(None, Some(3), false, 1);
    let rect = draw_rect(&mut r.s, (300.0, 800.0), (380.0, 860.0));
    r.s.shell.app.selection = Vec::new();
    let (a, b) = (center(r.s.app(), rect), center(r.s.app(), r.slot));
    r.s.shell.app.doc.get_mut(r.slot).unwrap().component.overrides.remove(&OverrideField::SlotContent);
    r.s.drag(r.s.screen(a.x, a.y), r.s.screen(b.x, b.y));
    assert_eq!(r.s.app().doc.get(rect).unwrap().parent, Some(r.slot));
    assert!(has_slot_override(r.s.app(), r.slot));
    assert!(matches!(r.s.app().doc.get(r.inst).unwrap().kind, NodeKind::Instance { .. }));
    r.s.cmd_key(Key::Z);
    assert_eq!(r.s.app().doc.get(rect).unwrap().parent, Some(r.s.app().page), "one undo step moves it back");
}

#[test]
fn dragging_the_last_child_out_of_a_min_one_slot_is_rejected() {
    let mut r = rig(Some(1), None, false, 1);
    let child = r.s.app().doc.get(r.slot).unwrap().children[0];
    r.s.shell.app.selection = vec![child];
    let before = snapshot(r.s.app());
    let a = center(r.s.app(), child);
    let b = DV::new(340.0, 830.0);
    r.s.drag(r.s.screen(a.x, a.y), r.s.screen(b.x, b.y));
    assert_rejected(&r.s, &before, "at least");
}

#[test]
fn alt_dragging_a_slot_child_in_a_full_slot_is_rejected() {
    let mut r = rig(None, Some(2), false, 2);
    let child = r.s.app().doc.get(r.slot).unwrap().children[0];
    r.s.shell.app.selection = vec![child];
    let before = snapshot(r.s.app());
    let a = center(r.s.app(), child);
    r.s.mods = Modifiers::ALT;
    // released inside the slot, so the copy stays in it
    r.s.drag(r.s.screen(a.x, a.y), r.s.screen(a.x + 6.0, a.y + 4.0));
    r.s.mods = Modifiers::NONE;
    assert_rejected(&r.s, &before, "maximum");
}

#[test]
fn alt_dragging_a_slot_child_out_of_a_full_slot_copies_it_out_and_leaves_the_slot_alone() {
    let mut r = rig(None, Some(2), false, 2);
    let slot_kids = r.s.app().doc.get(r.slot).unwrap().children.clone();
    r.s.shell.app.selection = vec![slot_kids[0]];
    let a = center(r.s.app(), slot_kids[0]);
    r.s.mods = Modifiers::ALT;
    r.s.drag(r.s.screen(a.x, a.y), r.s.screen(340.0, 830.0));
    r.s.mods = Modifiers::NONE;
    assert_eq!(r.s.app().doc.get(r.slot).unwrap().children, slot_kids, "the slot keeps exactly its own content");
    let copy = r.s.app().selection[0];
    assert!(!slot_kids.contains(&copy));
    assert_eq!(r.s.app().doc.get(copy).unwrap().parent, Some(r.s.app().page));
    r.s.cmd_key(Key::Z);
    assert!(r.s.app().doc.get(copy).is_none(), "one undo step removes the copy");
    assert_eq!(r.s.app().doc.get(r.slot).unwrap().children, slot_kids);
}

#[test]
fn drawing_a_shape_in_a_full_slot_is_rejected() {
    let mut r = rig(None, Some(2), false, 2);
    let o = center(r.s.app(), r.slot);
    r.s.key(Key::R);
    let before = snapshot(r.s.app());
    r.s.mods = Modifiers::COMMAND;
    r.s.drag(r.s.screen(o.x - 36.0, o.y - 16.0), r.s.screen(o.x - 20.0, o.y - 6.0));
    r.s.mods = Modifiers::NONE;
    assert_rejected(&r.s, &before, "maximum");
}

#[test]
fn drawing_a_shape_in_a_preferred_only_slot_is_rejected() {
    let mut r = rig(None, Some(2), true, 0);
    let o = center(r.s.app(), r.slot);
    r.s.key(Key::R);
    let before = snapshot(r.s.app());
    r.s.mods = Modifiers::COMMAND;
    r.s.drag(r.s.screen(o.x - 36.0, o.y - 16.0), r.s.screen(o.x - 20.0, o.y - 6.0));
    r.s.mods = Modifiers::NONE;
    assert_rejected(&r.s, &before, "preferred");
    assert!(!has_slot_override(r.s.app(), r.slot));
}

#[test]
fn a_pen_path_in_a_full_slot_is_rejected() {
    let mut r = rig(None, Some(2), false, 2);
    let o = center(r.s.app(), r.slot);
    r.s.key(Key::P);
    let before = snapshot(r.s.app());
    for (dx, dy) in [(-36.0, -16.0), (-14.0, -16.0), (-14.0, -4.0)] {
        let p = r.s.screen(o.x + dx, o.y + dy);
        r.s.click(p);
    }
    r.s.key(Key::Enter);
    assert_rejected(&r.s, &before, "maximum");
}

/// Two copied rectangles on the clipboard; selection and toast cleared.
fn copy_rects(s: &mut Sim, count: usize) {
    let mut ids = Vec::new();
    for k in 0..count {
        let x = 300.0 + 120.0 * k as f32;
        ids.push(draw_rect(s, (x, 800.0), (x + 80.0, 860.0)));
    }
    s.shell.app.selection = ids;
    assert!(s.shell.app.copy_selection().is_some());
}

#[test]
fn paste_to_replace_that_overfills_a_slot_is_rejected() {
    let mut r = rig(None, Some(2), false, 2);
    copy_rects(&mut r.s, 2);
    let child = r.s.app().doc.get(r.slot).unwrap().children[0];
    r.s.shell.app.selection = vec![child];
    r.s.shell.app.toast = None;
    let before = snapshot(r.s.app());
    r.s.shell.app.paste_to_replace();
    assert_rejected(&r.s, &before, "maximum");
}

#[test]
fn paste_to_replace_with_a_plain_node_in_a_preferred_only_slot_is_rejected() {
    let mut r = rig(None, Some(2), true, 1);
    copy_rects(&mut r.s, 1);
    let child = r.s.app().doc.get(r.slot).unwrap().children[0];
    r.s.shell.app.selection = vec![child];
    r.s.shell.app.toast = None;
    let before = snapshot(r.s.app());
    r.s.shell.app.paste_to_replace();
    assert_rejected(&r.s, &before, "preferred");
}

#[test]
fn paste_to_replace_one_for_one_in_a_full_slot_is_allowed_and_marks_the_slot() {
    let mut r = rig(None, Some(2), false, 2);
    copy_rects(&mut r.s, 1);
    let child = r.s.app().doc.get(r.slot).unwrap().children[0];
    r.s.shell.app.selection = vec![child];
    r.s.shell.app.doc.get_mut(r.slot).unwrap().component.overrides.remove(&OverrideField::SlotContent);
    r.s.shell.app.paste_to_replace();
    let kids = r.s.app().doc.get(r.slot).unwrap().children.clone();
    assert_eq!(kids.len(), 2);
    assert!(r.s.app().doc.get(child).is_none(), "the target was replaced");
    assert!(has_slot_override(r.s.app(), r.slot));
}

#[test]
fn an_instance_goes_into_the_selected_slot_only_when_dropped_on_it() {
    let mut r = rig(None, None, false, 0);
    let page = r.s.app().page;
    r.s.shell.app.selection = vec![r.slot];
    // Dropped on empty canvas far from the slot: it lands where the pointer is.
    let far = r.s.shell.app.insert_instance(r.badge, DV::new(340.0, 830.0)).expect("inserted");
    assert_eq!(r.s.app().doc.get(far).unwrap().parent, Some(page));
    assert!(r.s.app().doc.get(r.slot).unwrap().children.is_empty());
    // Dropped on the slot itself: it joins the slot.
    r.s.shell.app.selection = vec![r.slot];
    let at = center(r.s.app(), r.slot);
    let near = r.s.shell.app.insert_instance(r.badge, at).expect("inserted");
    assert_eq!(r.s.app().doc.get(near).unwrap().parent, Some(r.slot));
}

#[test]
fn a_refused_move_to_parent_does_not_mark_the_slot_as_changed() {
    let mut r = rig(None, Some(2), true, 0);
    let rect = draw_rect(&mut r.s, (300.0, 800.0), (380.0, 860.0));
    r.s.shell.app.toast = None;
    let before = snapshot(r.s.app());
    r.s.shell.app.move_to_parent(&[rect], r.slot, 0);
    assert_rejected(&r.s, &before, "preferred");
    assert!(!has_slot_override(r.s.app(), r.slot));
}

fn tiny_png() -> Vec<u8> {
    let mut out = std::io::Cursor::new(Vec::new());
    image::RgbaImage::new(4, 4).write_to(&mut out, image::ImageFormat::Png).expect("encode png");
    out.into_inner()
}

const TINY_SVG: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><rect width="10" height="10"/></svg>"#;

#[test]
fn a_placed_image_in_a_slot_is_marked_so_a_main_component_edit_keeps_it() {
    let mut r = rig(None, None, false, 0);
    let at = center(r.s.app(), r.slot);
    let photo = crate::canvas::interact::place_image_bytes(&mut r.s.shell.app, at, "Photo", tiny_png()).expect("placed");
    assert_eq!(r.s.app().doc.get(photo).unwrap().parent, Some(r.slot));
    assert!(has_slot_override(r.s.app(), r.slot));
    let main = r.s.by_name("Primary button");
    let bg = child_of(&r.s.app().doc, main, "Bg");
    r.s.shell.app.edit_nodes(&[bg], |n| n.opacity = 0.45);
    assert_eq!(r.s.app().doc.get(child_of(&r.s.app().doc, r.inst, "Bg")).unwrap().opacity, 0.45, "the rest of the instance still follows the main component");
    assert_eq!(r.s.app().doc.get(r.slot).unwrap().children, vec![photo], "the user's image is still in the slot");
}

#[test]
fn a_placed_svg_in_a_slot_is_marked_so_a_main_component_edit_keeps_it() {
    let mut r = rig(None, None, false, 0);
    let at = center(r.s.app(), r.slot);
    let icon = crate::clipboard::place_svg(&mut r.s.shell.app, TINY_SVG, "Icon", at, r.slot).expect("placed");
    assert!(has_slot_override(r.s.app(), r.slot));
    let main = r.s.by_name("Primary button");
    let bg = child_of(&r.s.app().doc, main, "Bg");
    r.s.shell.app.edit_nodes(&[bg], |n| n.opacity = 0.45);
    assert_eq!(r.s.app().doc.get(r.slot).unwrap().children, vec![icon]);
}

#[test]
fn placing_an_image_or_svg_in_a_preferred_only_slot_is_rejected() {
    let mut r = rig(None, Some(2), true, 0);
    let at = center(r.s.app(), r.slot);
    let before = snapshot(r.s.app());
    assert!(crate::canvas::interact::place_image_bytes(&mut r.s.shell.app, at, "Photo", tiny_png()).is_none());
    assert_rejected(&r.s, &before, "preferred");
    let err = crate::clipboard::place_svg(&mut r.s.shell.app, TINY_SVG, "Icon", at, r.slot).unwrap_err();
    assert!(err.contains("preferred"), "{err}");
    assert_eq!(snapshot(r.s.app()), before);
    assert!(!has_slot_override(r.s.app(), r.slot));
}

#[test]
fn the_text_tool_in_a_full_slot_is_rejected() {
    let mut r = rig(None, Some(2), false, 2);
    let o = center(r.s.app(), r.slot);
    r.s.key(Key::T);
    let before = snapshot(r.s.app());
    let p = r.s.screen(o.x - 36.0, o.y - 16.0);
    r.s.click(p);
    assert!(r.s.app().canvas.text.is_none(), "no text session starts in a full slot");
    assert_rejected(&r.s, &before, "maximum");
}

#[test]
fn the_inspector_reset_of_slot_content_restores_the_main_slots_children() {
    let mut r = rig(None, None, false, 0);
    let main = r.s.by_name("Primary button");
    let main_slot = child_of(&r.s.app().doc, main, "Extra");
    // The main component's slot gets a default child; the untouched instance slot follows it.
    let id = r.s.shell.app.doc.alloc_id();
    let mut default_child = Node::frame(id, designos_doc::Rect::new(0.0, 0.0, 10.0, 10.0));
    default_child.name = "Default".into();
    let mut tx = crate::edit::Tx::new();
    tx.cmd(&mut r.s.shell.app.doc, Command::Insert { parent: main_slot, index: 0, nodes: vec![default_child] }).unwrap();
    r.s.shell.app.run_tx(tx);
    let mirrored = r.s.app().doc.get(r.slot).unwrap().children.clone();
    assert_eq!(mirrored.len(), 1, "an unedited slot follows the main component");
    assert_eq!(r.s.app().doc.get(mirrored[0]).unwrap().component.source, Some(id));

    // The user adds a child in the slot, then presses Reset in the inspector (it passes the slot node).
    r.s.shell.app.selection = vec![r.slot];
    let at = center(r.s.app(), r.slot);
    let added = r.s.shell.app.insert_instance(r.badge, at).expect("added");
    assert_eq!(r.s.app().doc.get(r.slot).unwrap().children.len(), 2);
    let fields: Vec<OverrideField> = r.s.app().doc.get(r.slot).unwrap().component.overrides.iter().copied().collect();
    assert_eq!(fields, vec![OverrideField::SlotContent]);
    r.s.shell.app.reset_node_overrides(r.slot, &fields);

    let kids = r.s.app().doc.get(r.slot).unwrap().children.clone();
    assert!(r.s.app().doc.get(added).is_none(), "the added child is gone");
    assert_eq!(kids.len(), 1);
    assert_eq!(r.s.app().doc.get(kids[0]).unwrap().component.source, Some(id), "the main's child is back");
    assert!(!has_slot_override(r.s.app(), r.slot));
    r.s.cmd_key(Key::Z);
    assert!(r.s.app().doc.get(added).is_some() && has_slot_override(r.s.app(), r.slot), "Reset is one undo step");
}

fn mjpeg_mp4() -> Vec<u8> {
    let jpeg = |c: u8| {
        let mut out = Vec::new();
        let img = image::RgbImage::from_pixel(32, 16, image::Rgb([c, 0, 0]));
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 95).encode_image(&img).unwrap();
        out
    };
    crate::video::build_mjpeg_mp4(&[jpeg(250), jpeg(10)], 32, 16, 10)
}

#[test]
fn a_refused_image_or_video_drop_stores_nothing_in_the_file() {
    let mut r = rig(None, Some(2), true, 0);
    let at = center(r.s.app(), r.slot);
    let stored = r.s.app().doc.images.len();
    assert!(crate::canvas::interact::place_image_bytes(&mut r.s.shell.app, at, "Photo", tiny_png()).is_none());
    assert_eq!(r.s.app().doc.images.len(), stored, "no image bytes for a refused image");
    assert!(crate::canvas::interact::place_video_bytes(&mut r.s.shell.app, at, "Clip", mjpeg_mp4()).is_none());
    assert_eq!(r.s.app().doc.images.len(), stored, "no poster or video bytes for a refused video");
    assert!(r.s.app().toast.as_ref().is_some_and(|t| t.error && t.text.contains("preferred")));
}

/// A group of `count` rectangles inside the slot (built directly, as a drop that predates the limits would).
fn group_in_slot(r: &mut Rig, count: usize) -> NodeId {
    let app = &mut r.s.shell.app;
    let gid = app.doc.alloc_id();
    let mut group = Node::new(gid, NodeKind::Group);
    group.size = DV::new(40.0, 20.0);
    let mut tx = crate::edit::Tx::new();
    tx.cmd(&mut app.doc, Command::Insert { parent: r.slot, index: 0, nodes: vec![group] }).unwrap();
    for k in 0..count {
        let id = app.doc.alloc_id();
        let rect = Node::rectangle(id, designos_doc::Rect::new(4.0 * k as f32, 0.0, 4.0, 4.0));
        tx.cmd(&mut app.doc, Command::Insert { parent: gid, index: k, nodes: vec![rect] }).unwrap();
    }
    app.run_tx(tx);
    app.toast = None;
    gid
}

#[test]
fn ungrouping_into_a_slot_past_its_max_is_rejected() {
    let mut r = rig(None, Some(2), false, 0);
    let group = group_in_slot(&mut r, 3);
    r.s.shell.app.selection = vec![group];
    let before = snapshot(r.s.app());
    r.s.shell.app.ungroup_selection();
    assert_rejected(&r.s, &before, "maximum");
}

#[test]
fn ungrouping_into_a_slot_within_its_max_works_and_marks_the_slot() {
    let mut r = rig(None, Some(2), false, 0);
    let group = group_in_slot(&mut r, 2);
    r.s.shell.app.selection = vec![group];
    r.s.shell.app.ungroup_selection();
    assert!(r.s.app().doc.get(group).is_none());
    assert_eq!(r.s.app().doc.get(r.slot).unwrap().children.len(), 2);
    assert!(has_slot_override(r.s.app(), r.slot));
}

#[test]
fn grouping_the_children_of_a_slot_below_its_min_is_rejected() {
    let mut r = rig(Some(2), None, false, 2);
    let kids = r.s.app().doc.get(r.slot).unwrap().children.clone();
    r.s.shell.app.selection = kids;
    let before = snapshot(r.s.app());
    r.s.shell.app.group_selection(false);
    assert_rejected(&r.s, &before, "at least");
}

#[test]
fn grouping_slot_children_when_the_min_still_holds_works_and_marks_the_slot() {
    let mut r = rig(Some(1), None, false, 2);
    let kids = r.s.app().doc.get(r.slot).unwrap().children.clone();
    r.s.shell.app.selection = kids.clone();
    r.s.shell.app.doc.get_mut(r.slot).unwrap().component.overrides.remove(&OverrideField::SlotContent);
    r.s.shell.app.group_selection(false);
    let slot_kids = r.s.app().doc.get(r.slot).unwrap().children.clone();
    assert_eq!(slot_kids.len(), 1);
    assert!(!kids.contains(&slot_kids[0]), "the group replaced its members");
    assert!(has_slot_override(r.s.app(), r.slot));
}
