//! Empty-slot placeholders, the `+` picker and Reset keep working together in the flows a user takes.

use super::*;
use designos_doc::{Command, Node, OverrideField, Rect as DRect, Sizing, SlotPropConfig};

struct Rig {
    s: Sim,
    main: NodeId,
    inst: NodeId,
    badge: NodeId,
    slot: NodeId,
}

fn run(app: &mut App, cmd: Command) {
    let mut tx = crate::edit::Tx::new();
    let _ = tx.cmd(&mut app.doc, cmd);
    app.run_tx(tx);
}

fn child_of(d: &designos_doc::Document, parent: NodeId, name: &str) -> NodeId {
    *d.get(parent).unwrap().children.iter().find(|c| d.get(**c).unwrap().name == name).unwrap_or_else(|| panic!("no child {name}"))
}

fn center(app: &App, id: NodeId) -> DV {
    crate::util::rect_center(&crate::util::world_aabb(&app.doc, id))
}

fn has_slot_override(app: &App, slot: NodeId) -> bool {
    app.doc.get(slot).unwrap().component.overrides.contains(&OverrideField::SlotContent)
}

/// "Get started" instance whose button component gained an empty slot "Extra" (stretch on insert, `max` limit).
fn rig(max: Option<u32>) -> Rig {
    let mut s = Sim::with(Options { select: Some("Get started".into()), ..Default::default() });
    let main = s.by_name("Primary button");
    let inst = s.by_name("Get started");
    let page = s.app().page;
    let badge = s.shell.app.doc.alloc_id();
    let mut node = Node::frame(badge, DRect::new(0.0, 0.0, 20.0, 20.0));
    node.name = "Badge".into();
    run(&mut s.shell.app, Command::Insert { parent: page, index: 0, nodes: vec![node] });
    let cmd = designos_doc::create_component(&mut s.shell.app.doc, badge);
    run(&mut s.shell.app, cmd);

    let slot_id = s.shell.app.doc.alloc_id();
    let mut frame = Node::frame(slot_id, DRect::new(8.0, 8.0, 80.0, 40.0));
    frame.name = "Extra".into();
    run(&mut s.shell.app, Command::Insert { parent: main, index: 3, nodes: vec![frame] });
    let cmd = designos_doc::convert_to_slot(&mut s.shell.app.doc, slot_id);
    run(&mut s.shell.app, cmd);
    let config = SlotPropConfig { max_children: max, stretch_child_on_insert: Some(true), preferred: vec![badge], ..Default::default() };
    let cmd = designos_doc::set_slot_config(&mut s.shell.app.doc, main, "Extra", config);
    run(&mut s.shell.app, cmd);

    let slot = child_of(&s.app().doc, inst, "Extra");
    s.shell.app.selection = Vec::new();
    s.shell.app.toast = None;
    Rig { s, main, inst, badge, slot }
}

#[test]
fn a_drop_elsewhere_after_the_plus_picker_does_not_go_into_the_still_selected_slot() {
    let mut r = rig(Some(1));
    let page = r.s.app().page;
    let _ = (r.main, r.inst);

    let first = r.s.shell.app.insert_instance_into_slot(r.slot, r.badge).expect("the picker fills the slot");
    assert_eq!(r.s.app().selection, vec![r.slot], "the slot stays selected after the picker");
    assert_eq!(r.s.app().doc.get(first).unwrap().parent, Some(r.slot));
    assert!(has_slot_override(r.s.app(), r.slot));

    // An Assets drag released on empty canvas lands on the page, not in the (now full) slot.
    r.s.shell.app.toast = None;
    let dropped = r.s.shell.app.insert_instance(r.badge, DV::new(340.0, 830.0)).expect("the drop is not refused");
    assert_eq!(r.s.app().doc.get(dropped).unwrap().parent, Some(page));
    assert_eq!(r.s.app().doc.get(r.slot).unwrap().children, vec![first], "the slot is untouched");
    assert!(r.s.app().toast.is_none(), "no slot-limit toast for a drop that never reached the slot");
}

#[test]
fn reset_after_filling_an_empty_slot_with_the_picker_brings_the_placeholder_back() {
    let mut r = rig(None);
    s_frames(&mut r.s);
    assert!(designos_doc::should_show_empty_slot_placeholder(&r.s.app().doc, r.slot));

    let id = r.s.shell.app.insert_instance_into_slot(r.slot, r.badge).expect("filled");
    assert_eq!(r.s.app().doc.get(id).unwrap().layout_child.horizontal, Sizing::Fill, "stretch applies on insert");
    assert!(!designos_doc::should_show_empty_slot_placeholder(&r.s.app().doc, r.slot));
    let fields: Vec<OverrideField> = r.s.app().doc.get(r.slot).unwrap().component.overrides.iter().copied().collect();
    assert_eq!(fields, vec![OverrideField::SlotContent], "the inspector offers Reset for slot content");

    r.s.shell.app.reset_node_overrides(r.slot, &fields);

    assert!(r.s.app().doc.get(id).is_none(), "the picked instance is gone");
    assert!(r.s.app().doc.get(r.slot).unwrap().children.is_empty());
    assert!(!has_slot_override(r.s.app(), r.slot));
    assert!(designos_doc::should_show_empty_slot_placeholder(&r.s.app().doc, r.slot), "the empty slot shows its placeholder again");
    s_frames(&mut r.s);
    assert!(crate::canvas::draw::empty_slot_plus_button_rect(r.s.app(), r.s.app().canvas_rect, r.slot).is_some());
}

fn s_frames(s: &mut Sim) {
    s.frame(vec![]);
    s.frame(vec![]);
}

#[test]
fn an_empty_slot_deep_in_a_huge_document_still_gets_a_placeholder_and_a_working_plus() {
    let mut r = rig(None);
    let at = center(r.s.app(), r.inst);
    let page = r.s.app().page;
    let alloc_frame = |app: &mut App, name: &str, rect: DRect| {
        let id = app.doc.alloc_id();
        let mut n = Node::frame(id, rect);
        n.name = name.into();
        (id, n)
    };

    // 10,000+ nodes far away from the view put the document in the capped regime.
    let (filler, node) = alloc_frame(&mut r.s.shell.app, "Filler", DRect::new(80_000.0, 80_000.0, 10.0, 10.0));
    run(&mut r.s.shell.app, Command::Insert { parent: page, index: 0, nodes: vec![node] });
    for _ in 0..10_050 {
        let (_, n) = alloc_frame(&mut r.s.shell.app, "f", DRect::new(0.0, 0.0, 1.0, 1.0));
        r.s.shell.app.doc.insert(filler, n).unwrap();
    }
    assert!(r.s.app().doc.nodes.len() > 10_000);

    // page > section > frame > instance > slot: the slot is four levels below the page.
    let (section, node) = alloc_frame(&mut r.s.shell.app, "Section", DRect::new(at.x - 150.0, at.y + 120.0, 400.0, 300.0));
    run(&mut r.s.shell.app, Command::Insert { parent: page, index: 1, nodes: vec![node] });
    let (inner, node) = alloc_frame(&mut r.s.shell.app, "Inner", DRect::new(10.0, 10.0, 360.0, 260.0));
    run(&mut r.s.shell.app, Command::Insert { parent: section, index: 0, nodes: vec![node] });
    let cmd = designos_doc::create_instance(&mut r.s.shell.app.doc, r.main, inner, DV::new(20.0, 20.0));
    run(&mut r.s.shell.app, cmd);
    let deep_inst = *r.s.app().doc.get(inner).unwrap().children.last().unwrap();
    let deep_slot = child_of(&r.s.app().doc, deep_inst, "Extra");
    s_frames(&mut r.s);

    assert!(crate::canvas::interact::active_index(r.s.app()).is_some(), "the headless canvas builds a scene index");
    assert!(designos_doc::should_show_empty_slot_placeholder(&r.s.app().doc, deep_slot));
    let plus = r.s.rect_of(&format!("canvas-slot-plus:{}", deep_slot.0));
    assert!(plus.width() > 0.0, "the overlay registers a hit target for the deep slot");

    r.s.click(plus.center());
    assert_eq!(r.s.app().ui.slot_picker.map(|(slot, _)| slot), Some(deep_slot), "the + opens the picker for that slot");
}

#[test]
fn reset_after_duplicating_a_mirrored_slot_child_leaves_exactly_the_main_slots_children() {
    let mut r = rig(None);
    let main_slot = child_of(&r.s.app().doc, r.main, "Extra");
    let id = r.s.shell.app.doc.alloc_id();
    let mut default_child = Node::frame(id, DRect::new(0.0, 0.0, 10.0, 10.0));
    default_child.name = "Default".into();
    run(&mut r.s.shell.app, Command::Insert { parent: main_slot, index: 0, nodes: vec![default_child] });
    let mirrored = r.s.app().doc.get(r.slot).unwrap().children.clone();
    assert_eq!(mirrored.len(), 1, "an untouched slot follows the main component");

    // Duplicate the mirrored child in the instance slot (Ctrl+D), then press Reset.
    r.s.shell.app.selection = mirrored.clone();
    r.s.shell.app.duplicate_selection();
    assert_eq!(r.s.app().doc.get(r.slot).unwrap().children.len(), 2);
    assert!(has_slot_override(r.s.app(), r.slot));
    r.s.shell.app.reset_node_overrides(r.slot, &[OverrideField::SlotContent]);

    let kids = r.s.app().doc.get(r.slot).unwrap().children.clone();
    let sources: Vec<_> = kids.iter().map(|k| r.s.app().doc.get(*k).unwrap().component.source).collect();
    assert_eq!(sources, vec![Some(id)], "the slot holds the main slot's child once");
    assert!(!has_slot_override(r.s.app(), r.slot));

    // A later edit of the main component keeps it that way.
    let cmd = designos_doc::sync_instances(&mut r.s.shell.app.doc, r.main);
    run(&mut r.s.shell.app, cmd);
    assert_eq!(r.s.app().doc.get(r.slot).unwrap().children.len(), 1);
}

fn draw_rect_on_page(s: &mut Sim, from: (f32, f32), to: (f32, f32)) -> NodeId {
    s.key(Key::R);
    let (a, b) = (s.screen(from.0, from.1), s.screen(to.0, to.1));
    s.drag(a, b);
    let id = s.app().selection[0];
    assert_eq!(s.app().doc.get(id).unwrap().parent, Some(s.app().page));
    id
}

fn png_of_size(w: u32, h: u32) -> Vec<u8> {
    let mut out = std::io::Cursor::new(Vec::new());
    image::RgbaImage::from_pixel(w, h, image::Rgba([220, 80, 40, 255])).write_to(&mut out, image::ImageFormat::Png).expect("encode png");
    out.into_inner()
}

fn tiny_mjpeg_mp4() -> Vec<u8> {
    let jpeg = |c: u8| {
        let mut out = Vec::new();
        let img = image::RgbImage::from_pixel(32, 16, image::Rgb([c, 0, 0]));
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 95).encode_image(&img).unwrap();
        out
    };
    crate::video::build_mjpeg_mp4(&[jpeg(250), jpeg(10)], 32, 16, 10)
}

const SAMPLE_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="18"><rect width="24" height="18" fill="#3b82f6"/></svg>"##;

fn setup_library_for_rig(r: &mut Rig, tag: &str) -> (std::path::PathBuf, String, String) {
    let root = std::env::temp_dir().join(format!("designos-slot-stretch-lib-{tag}-{}", designos_library::new_id()));
    let _ = std::fs::remove_dir_all(&root);

    let mut a = Sim::new();
    a.shell.app.libs.set_root(root.clone());
    a.shell.app.open_libraries();
    s_frames(&mut a);
    a.frame(vec![]);
    a.shell.app.libs.description = "Initial publish".into();
    let pub_btn = a.rect_of("lib-publish");
    a.click(pub_btn.center());
    s_frames(&mut a);

    r.s.shell.app.libs.set_root(root.clone());
    r.s.shell.app.open_libraries();
    s_frames(&mut r.s);
    r.s.frame(vec![]);
    let toggle = r.s.rect_of("lib-toggle:Untitled");
    r.s.click(toggle.center());
    s_frames(&mut r.s);
    r.s.shell.app.libs.open = false;
    s_frames(&mut r.s);

    let lib_id = r.s.app().doc.library.enabled[0].id.clone();
    let key = r.s.shell.app.latest_snapshot(&lib_id).unwrap().assets.iter().find(|x| x.name == "Primary button").unwrap().key.clone();
    (root, lib_id, key)
}

#[test]
fn drag_into_stretch_slot_fills_the_slot() {
    let mut r = rig(None);
    // Allow any child in the slot so we can test rectangles, multi-selection, and text layers.
    let cfg = SlotPropConfig { stretch_child_on_insert: Some(true), ..Default::default() };
    let cmd = designos_doc::set_slot_config(&mut r.s.shell.app.doc, r.main, "Extra", cfg);
    run(&mut r.s.shell.app, cmd);

    // 1. Single rectangle dragged from the page into the non-auto-layout slot (width 80):
    // gets Fill, width = 80, and transform.tx = 0 (left padding) after drag's world-to-local transform.
    let rect = draw_rect_on_page(&mut r.s, (300.0, 800.0), (330.0, 820.0));
    assert_eq!(r.s.app().doc.get(rect).unwrap().size.x, 30.0);
    r.s.shell.app.selection = Vec::new();
    let (a, b) = (center(r.s.app(), rect), center(r.s.app(), r.slot));
    r.s.drag(r.s.screen(a.x, a.y), r.s.screen(b.x, b.y));
    let n = r.s.app().doc.get(rect).unwrap();
    assert_eq!(n.parent, Some(r.slot));
    assert_eq!(n.layout_child.horizontal, Sizing::Fill);
    assert_eq!(n.size.x, 80.0);
    assert_eq!(n.transform.tx, 0.0, "non-auto-layout slot sets tx = left padding after drag transform fix-up");

    // 2. Auto-layout slot with left/right padding 12 -> width = 80 - 24 = 56.
    let main_slot = child_of(&r.s.app().doc, r.main, "Extra");
    r.s.shell.app.edit_nodes(&[main_slot], |sn| {
        if let Some(fp) = sn.kind.frame_props_mut() {
            let mut al = designos_doc::AutoLayout::v1();
            al.direction = designos_doc::LayoutDirection::Vertical;
            al.primary_sizing = Sizing::Fixed;
            al.counter_sizing = Sizing::Fixed;
            al.padding = [4.0, 12.0, 4.0, 12.0];
            fp.auto_layout = Some(al);
        }
    });
    s_frames(&mut r.s);
    let r1 = draw_rect_on_page(&mut r.s, (300.0, 780.0), (360.0, 820.0));
    let r2 = draw_rect_on_page(&mut r.s, (380.0, 780.0), (430.0, 820.0));
    r.s.shell.app.selection = vec![r1, r2];
    let (a, b) = (center(r.s.app(), r1), center(r.s.app(), r.slot));
    r.s.drag(r.s.screen(a.x - 12.0, a.y), r.s.screen(b.x, b.y));
    for id in [r1, r2] {
        let child = r.s.app().doc.get(id).unwrap();
        assert_eq!(child.parent, Some(r.slot));
        assert_eq!(child.layout_child.horizontal, Sizing::Fill);
        assert_eq!(child.size.x, 56.0, "slot width 80 minus left/right padding 12+12 = 56");
    }

    // 3. Alt-drag copy from the page into the slot also stretches.
    let src = draw_rect_on_page(&mut r.s, (300.0, 850.0), (325.0, 865.0));
    r.s.shell.app.selection = vec![src];
    let (a, b) = (center(r.s.app(), src), center(r.s.app(), r.slot));
    r.s.mods = Modifiers::ALT;
    r.s.drag(r.s.screen(a.x, a.y), r.s.screen(b.x, b.y));
    r.s.mods = Modifiers::NONE;
    let copy = r.s.app().selection[0];
    assert_ne!(copy, src);
    assert_eq!(r.s.app().doc.get(copy).unwrap().parent, Some(r.slot));
    assert_eq!(r.s.app().doc.get(copy).unwrap().layout_child.horizontal, Sizing::Fill);
    assert_eq!(r.s.app().doc.get(copy).unwrap().size.x, 56.0);
}

#[test]
fn drag_between_slots_stretches_again() {
    let mut r = rig(None);
    let cfg = SlotPropConfig { stretch_child_on_insert: Some(true), ..Default::default() };
    let cmd = designos_doc::set_slot_config(&mut r.s.shell.app.doc, r.main, "Extra", cfg.clone());
    run(&mut r.s.shell.app, cmd);

    // Add a second wider slot "WideSlot" (width 130) on the main button component.
    let wide_main_id = r.s.shell.app.doc.alloc_id();
    let mut wide_frame = Node::frame(wide_main_id, DRect::new(100.0, 8.0, 130.0, 40.0));
    wide_frame.name = "WideSlot".into();
    run(&mut r.s.shell.app, Command::Insert { parent: r.main, index: 4, nodes: vec![wide_frame] });
    let cmd = designos_doc::convert_to_slot(&mut r.s.shell.app.doc, wide_main_id);
    run(&mut r.s.shell.app, cmd);
    let cmd = designos_doc::set_slot_config(&mut r.s.shell.app.doc, r.main, "WideSlot", cfg);
    run(&mut r.s.shell.app, cmd);
    let slot_b = child_of(&r.s.app().doc, r.inst, "WideSlot");
    s_frames(&mut r.s);

    // Drag a 30px rectangle into slot A (80px), then drag it from slot A into slot B (130px).
    let rect = draw_rect_on_page(&mut r.s, (300.0, 800.0), (330.0, 820.0));
    r.s.shell.app.selection = vec![rect];
    let (a, b) = (center(r.s.app(), rect), center(r.s.app(), r.slot));
    r.s.drag(r.s.screen(a.x, a.y), r.s.screen(b.x, b.y));
    assert_eq!(r.s.app().doc.get(rect).unwrap().parent, Some(r.slot));
    assert_eq!(r.s.app().doc.get(rect).unwrap().size.x, 80.0);

    let (a2, b2) = (center(r.s.app(), rect), center(r.s.app(), slot_b));
    r.s.drag(r.s.screen(a2.x, a2.y), r.s.screen(b2.x, b2.y));
    assert_eq!(r.s.app().doc.get(rect).unwrap().parent, Some(slot_b));
    assert_eq!(r.s.app().doc.get(rect).unwrap().layout_child.horizontal, Sizing::Fill);
    assert_eq!(r.s.app().doc.get(rect).unwrap().size.x, 130.0, "moving from slot A to slot B re-stretches to slot B's width");
}

#[test]
fn reorder_inside_same_slot_does_not_restretch() {
    let mut r = rig(None);
    let cfg = SlotPropConfig { stretch_child_on_insert: Some(true), ..Default::default() };
    let cmd = designos_doc::set_slot_config(&mut r.s.shell.app.doc, r.main, "Extra", cfg);
    run(&mut r.s.shell.app, cmd);

    // Make the slot a horizontal auto-layout frame (width 120, height 40) so two children sit side-by-side.
    let main_slot = child_of(&r.s.app().doc, r.main, "Extra");
    r.s.shell.app.edit_nodes(&[main_slot], |sn| {
        sn.size = DV::new(120.0, 40.0);
        if let Some(fp) = sn.kind.frame_props_mut() {
            let mut al = designos_doc::AutoLayout::v1();
            al.direction = designos_doc::LayoutDirection::Horizontal;
            al.primary_sizing = Sizing::Fixed;
            al.counter_sizing = Sizing::Fixed;
            al.item_spacing = 8.0;
            fp.auto_layout = Some(al);
        }
    });

    // Insert two 25x20 children directly into the instance slot with Fixed horizontal sizing.
    let c1 = r.s.shell.app.doc.alloc_id();
    let mut n1 = Node::rectangle(c1, DRect::new(0.0, 0.0, 25.0, 20.0));
    n1.name = "First".into();
    run(&mut r.s.shell.app, Command::Insert { parent: r.slot, index: 0, nodes: vec![n1] });
    let c2 = r.s.shell.app.doc.alloc_id();
    let mut n2 = Node::rectangle(c2, DRect::new(33.0, 0.0, 25.0, 20.0));
    n2.name = "Second".into();
    run(&mut r.s.shell.app, Command::Insert { parent: r.slot, index: 1, nodes: vec![n2] });
    {
        let mut ed = crate::edit::Edit::new();
        App::mark_slot_content_override(&mut r.s.shell.app.doc, &mut ed, r.slot);
        crate::layout::relayout_around(&mut r.s.shell.app.doc, &mut ed, r.slot);
    }
    s_frames(&mut r.s);
    assert_eq!(r.s.app().doc.get(r.slot).unwrap().children, vec![c1, c2]);
    assert_eq!(r.s.app().doc.get(c1).unwrap().size.x, 25.0);
    assert_eq!(r.s.app().doc.get(c1).unwrap().layout_child.horizontal, Sizing::Fixed);

    // Drag c1 past c2 inside the same auto-layout slot to reorder [c2, c1].
    r.s.shell.app.selection = vec![c1];
    let a = center(r.s.app(), c1);
    let b = DV::new(center(r.s.app(), c2).x + 18.0, a.y);
    r.s.drag(r.s.screen(a.x, a.y), r.s.screen(b.x, b.y));

    assert_eq!(r.s.app().doc.get(r.slot).unwrap().children, vec![c2, c1], "c1 reordered after c2 inside the same slot");
    assert_eq!(r.s.app().doc.get(c1).unwrap().layout_child.horizontal, Sizing::Fixed, "reorder inside the same slot must not set Fill");
    assert_eq!(r.s.app().doc.get(c1).unwrap().size.x, 25.0, "reorder inside the same slot must not re-stretch width");
    assert_eq!(r.s.app().doc.get(c2).unwrap().size.x, 25.0);
}

#[test]
fn library_drop_into_stretch_slot_fills_the_slot() {
    let mut r = rig(None);
    let cfg = SlotPropConfig { stretch_child_on_insert: Some(true), ..Default::default() };
    let cmd = designos_doc::set_slot_config(&mut r.s.shell.app.doc, r.main, "Extra", cfg);
    run(&mut r.s.shell.app, cmd);

    let (root, lib_id, key) = setup_library_for_rig(&mut r, "drop-stretch");
    let at = center(r.s.app(), r.slot);
    let dropped = r.s.shell.app.drop_library_asset(&lib_id, &key, at).expect("library asset dropped into slot");

    let node = r.s.app().doc.get(dropped).unwrap();
    assert_eq!(node.parent, Some(r.slot));
    assert_eq!(node.layout_child.horizontal, Sizing::Fill);
    assert_eq!(node.size.x, 80.0);
    assert!(node.component.overrides.contains(&OverrideField::Size));
    assert!(node.component.overrides.contains(&OverrideField::Layout));
    assert!(has_slot_override(r.s.app(), r.slot));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn image_place_into_stretch_slot() {
    let mut r = rig(None);
    let cfg = SlotPropConfig { stretch_child_on_insert: Some(true), ..Default::default() };
    let cmd = designos_doc::set_slot_config(&mut r.s.shell.app.doc, r.main, "Extra", cfg);
    run(&mut r.s.shell.app, cmd);

    let at = center(r.s.app(), r.slot);
    // 160x36 image placed into an 80px-wide slot: width shrinks to 80, height stays 36.
    let id = crate::canvas::interact::place_image_bytes(&mut r.s.shell.app, at, "BannerPhoto", png_of_size(160, 36)).expect("image placed");
    let node = r.s.app().doc.get(id).unwrap();
    assert_eq!(node.parent, Some(r.slot));
    assert_eq!(node.layout_child.horizontal, Sizing::Fill);
    assert_eq!(node.size.x, 80.0);
    assert_eq!(node.size.y, 36.0);
    assert!(has_slot_override(r.s.app(), r.slot));
}

#[test]
fn video_place_into_stretch_slot() {
    let mut r = rig(None);
    let cfg = SlotPropConfig { stretch_child_on_insert: Some(true), ..Default::default() };
    let cmd = designos_doc::set_slot_config(&mut r.s.shell.app.doc, r.main, "Extra", cfg);
    run(&mut r.s.shell.app, cmd);

    let at = center(r.s.app(), r.slot);
    let id = crate::canvas::interact::place_video_bytes(&mut r.s.shell.app, at, "DemoClip", tiny_mjpeg_mp4()).expect("video placed");
    let node = r.s.app().doc.get(id).unwrap();
    assert_eq!(node.parent, Some(r.slot));
    assert_eq!(node.layout_child.horizontal, Sizing::Fill);
    assert_eq!(node.size.x, 80.0);
    assert_eq!(node.size.y, 16.0);
    assert!(has_slot_override(r.s.app(), r.slot));
}

#[test]
fn svg_place_into_stretch_slot() {
    let mut r = rig(None);
    let cfg = SlotPropConfig { stretch_child_on_insert: Some(true), ..Default::default() };
    let cmd = designos_doc::set_slot_config(&mut r.s.shell.app.doc, r.main, "Extra", cfg);
    run(&mut r.s.shell.app, cmd);

    let at = center(r.s.app(), r.slot);
    let id = crate::clipboard::place_svg(&mut r.s.shell.app, SAMPLE_SVG, "Glyph", at, r.slot).expect("svg placed");
    let node = r.s.app().doc.get(id).unwrap();
    assert_eq!(node.parent, Some(r.slot));
    assert_eq!(node.layout_child.horizontal, Sizing::Fill);
    assert_eq!(node.size.x, 80.0);
    assert_eq!(node.size.y, 18.0);
    assert!(has_slot_override(r.s.app(), r.slot));
}

#[test]
fn drawn_layers_keep_their_size() {
    let mut r = rig(None);
    let cfg = SlotPropConfig { stretch_child_on_insert: Some(true), ..Default::default() };
    let cmd = designos_doc::set_slot_config(&mut r.s.shell.app.doc, r.main, "Extra", cfg);
    run(&mut r.s.shell.app, cmd);

    let o = center(r.s.app(), r.slot);
    // 1. Draw a small rectangle inside the 80px-wide stretch slot.
    r.s.key(Key::R);
    r.s.drag(r.s.screen(o.x - 15.0, o.y - 6.0), r.s.screen(o.x + 5.0, o.y + 6.0));
    let drawn_rect = r.s.app().selection[0];
    let rn = r.s.app().doc.get(drawn_rect).unwrap();
    assert_eq!(rn.parent, Some(r.slot));
    assert_eq!(rn.layout_child.horizontal, Sizing::Fixed, "drawn rectangle must not get Fill sizing");
    assert!(rn.size.x > 0.0 && rn.size.x < 80.0, "drawn rectangle keeps its drawn width (< 80), got {}", rn.size.x);

    // 2. Click with the Text tool inside the stretch slot.
    r.s.key(Key::T);
    r.s.click(r.s.screen(o.x + 10.0, o.y));
    r.s.type_text("Hi");
    r.s.key(Key::Escape);
    let drawn_text = *r.s.app().doc.get(r.slot).unwrap().children.last().unwrap();
    let tn = r.s.app().doc.get(drawn_text).unwrap();
    assert!(matches!(tn.kind, NodeKind::Text(_)));
    assert_ne!(tn.layout_child.horizontal, Sizing::Fill, "drawn text must not get Fill sizing");
    assert!(tn.size.x < 80.0, "drawn text keeps its natural width, got {}", tn.size.x);
}

#[test]
fn no_stretch_when_setting_off() {
    let mut r = rig(None);
    // Turn off stretch_child_on_insert (`None`).
    let cfg = SlotPropConfig { stretch_child_on_insert: None, ..Default::default() };
    let cmd = designos_doc::set_slot_config(&mut r.s.shell.app.doc, r.main, "Extra", cfg);
    run(&mut r.s.shell.app, cmd);

    let at = center(r.s.app(), r.slot);

    // 1. Canvas drag into slot
    let rect = draw_rect_on_page(&mut r.s, (300.0, 800.0), (330.0, 820.0));
    r.s.shell.app.selection = vec![rect];
    let a = center(r.s.app(), rect);
    r.s.drag(r.s.screen(a.x, a.y), r.s.screen(at.x, at.y));
    assert_eq!(r.s.app().doc.get(rect).unwrap().parent, Some(r.slot));
    assert_eq!(r.s.app().doc.get(rect).unwrap().layout_child.horizontal, Sizing::Fixed);
    assert_eq!(r.s.app().doc.get(rect).unwrap().size.x, 30.0);

    // 2. Image, video, SVG placement into slot
    let img_id = crate::canvas::interact::place_image_bytes(&mut r.s.shell.app, at, "Img", png_of_size(28, 14)).unwrap();
    assert_eq!(r.s.app().doc.get(img_id).unwrap().layout_child.horizontal, Sizing::Fixed);
    assert_eq!(r.s.app().doc.get(img_id).unwrap().size.x, 28.0);

    let vid_id = crate::canvas::interact::place_video_bytes(&mut r.s.shell.app, at, "Vid", tiny_mjpeg_mp4()).unwrap();
    assert_eq!(r.s.app().doc.get(vid_id).unwrap().layout_child.horizontal, Sizing::Fixed);
    assert_eq!(r.s.app().doc.get(vid_id).unwrap().size.x, 32.0);

    let svg_id = crate::clipboard::place_svg(&mut r.s.shell.app, SAMPLE_SVG, "Svg", at, r.slot).unwrap();
    assert_eq!(r.s.app().doc.get(svg_id).unwrap().layout_child.horizontal, Sizing::Fixed);
    assert_eq!(r.s.app().doc.get(svg_id).unwrap().size.x, 24.0);
}

#[test]
fn one_undo_step_per_action() {
    let mut r = rig(None);
    let cfg = SlotPropConfig { stretch_child_on_insert: Some(true), ..Default::default() };
    let cmd = designos_doc::set_slot_config(&mut r.s.shell.app.doc, r.main, "Extra", cfg);
    run(&mut r.s.shell.app, cmd);

    let (root, lib_id, key) = setup_library_for_rig(&mut r, "undo-one-step");
    let at = center(r.s.app(), r.slot);

    // 1. Drag into stretch slot -> 1 undo restores page parent, original transform, 30px width, Fixed sizing, and clears slot override.
    let rect = draw_rect_on_page(&mut r.s, (300.0, 800.0), (330.0, 820.0));
    let orig_tx = r.s.app().doc.get(rect).unwrap().transform;
    r.s.shell.app.selection = vec![rect];
    let a = center(r.s.app(), rect);
    r.s.drag(r.s.screen(a.x, a.y), r.s.screen(at.x, at.y));
    assert_eq!(r.s.app().doc.get(rect).unwrap().size.x, 80.0);
    assert_eq!(r.s.app().doc.get(rect).unwrap().layout_child.horizontal, Sizing::Fill);
    r.s.cmd_key(Key::Z);
    let restored = r.s.app().doc.get(rect).unwrap();
    assert_eq!(restored.parent, Some(r.s.app().page));
    assert_eq!(restored.size.x, 30.0);
    assert_eq!(restored.layout_child.horizontal, Sizing::Fixed);
    assert_eq!(restored.transform, orig_tx);
    assert!(!has_slot_override(r.s.app(), r.slot));

    // 2. Library drop into stretch slot -> 1 undo removes the dropped instance and clears slot override.
    let dropped = r.s.shell.app.drop_library_asset(&lib_id, &key, at).unwrap();
    assert_eq!(r.s.app().doc.get(dropped).unwrap().size.x, 80.0);
    r.s.cmd_key(Key::Z);
    assert!(r.s.app().doc.get(dropped).is_none());
    assert!(r.s.app().doc.get(r.slot).unwrap().children.is_empty());
    assert!(!has_slot_override(r.s.app(), r.slot));

    // 3. Image place -> 1 undo removes the image node and clears slot override.
    let img_id = crate::canvas::interact::place_image_bytes(&mut r.s.shell.app, at, "Img", png_of_size(28, 14)).unwrap();
    assert_eq!(r.s.app().doc.get(img_id).unwrap().size.x, 80.0);
    r.s.cmd_key(Key::Z);
    assert!(r.s.app().doc.get(img_id).is_none());
    assert!(!has_slot_override(r.s.app(), r.slot));

    // 4. Video place -> 1 undo removes the video node and clears slot override.
    let vid_id = crate::canvas::interact::place_video_bytes(&mut r.s.shell.app, at, "Vid", tiny_mjpeg_mp4()).unwrap();
    assert_eq!(r.s.app().doc.get(vid_id).unwrap().size.x, 80.0);
    r.s.cmd_key(Key::Z);
    assert!(r.s.app().doc.get(vid_id).is_none());
    assert!(!has_slot_override(r.s.app(), r.slot));

    // 5. SVG place -> 1 undo removes the SVG subtree and clears slot override.
    let svg_id = crate::clipboard::place_svg(&mut r.s.shell.app, SAMPLE_SVG, "Svg", at, r.slot).unwrap();
    assert_eq!(r.s.app().doc.get(svg_id).unwrap().size.x, 80.0);
    r.s.cmd_key(Key::Z);
    assert!(r.s.app().doc.get(svg_id).is_none());
    assert!(!has_slot_override(r.s.app(), r.slot));

    let _ = std::fs::remove_dir_all(root);
}
