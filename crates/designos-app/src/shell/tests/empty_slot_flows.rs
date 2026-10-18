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
