//! UI tests for components and instances.

use super::*;
use crate::app::RailItem;
use designos_doc::{OverrideField, PropValue};

fn center(app: &App, id: NodeId) -> DV {
    crate::util::rect_center(&crate::util::world_aabb(&app.doc, id))
}

fn ctrl_alt() -> Modifiers {
    Modifiers { alt: true, ctrl: true, command: true, ..Default::default() }
}

fn child_of(d: &designos_doc::Document, parent: NodeId, name: &str) -> NodeId {
    *d.get(parent).unwrap().children.iter().find(|c| d.get(**c).unwrap().name == name).unwrap_or_else(|| panic!("no child {name}"))
}

#[test]
fn ctrl_alt_k_makes_a_component_and_wraps_shapes() {
    let mut s = Sim::with(Options { select: Some("Hero".into()), ..Default::default() });
    let hero = s.by_name("Hero");
    s.mods = ctrl_alt();
    s.key(Key::K);
    s.mods = Modifiers::NONE;
    let comp = s.app().selection[0];
    assert_ne!(comp, hero);
    assert!(matches!(s.app().doc.get(comp).unwrap().kind, NodeKind::Component(_)));
    assert_eq!(s.app().doc.get(hero).unwrap().parent, Some(comp));
    s.cmd_key(Key::Z);
    assert!(s.app().doc.get(comp).is_none());
    assert_eq!(s.app().doc.get(hero).unwrap().parent, Some(s.by_name("Welcome")));
}

#[test]
fn ctrl_alt_k_converts_a_frame_in_place() {
    let mut s = Sim::with(Options { select: Some("Card".into()), ..Default::default() });
    let card = s.by_name("Card");
    s.mods = ctrl_alt();
    s.key(Key::K);
    s.mods = Modifiers::NONE;
    assert!(matches!(s.app().doc.get(card).unwrap().kind, NodeKind::Component(_)));
    assert_eq!(s.app().selection, vec![card]);
}

#[test]
fn ctrl_alt_b_detaches_an_instance() {
    let mut s = Sim::with(Options { select: Some("Get started".into()), ..Default::default() });
    let inst = s.by_name("Get started");
    assert!(matches!(s.app().doc.get(inst).unwrap().kind, NodeKind::Instance { .. }));
    s.mods = ctrl_alt();
    s.key(Key::B);
    s.mods = Modifiers::NONE;
    let n = s.app().doc.get(inst).unwrap();
    assert!(matches!(n.kind, NodeKind::Frame(_)));
    assert!(n.component.is_empty());
    s.cmd_key(Key::Z);
    assert!(matches!(s.app().doc.get(inst).unwrap().kind, NodeKind::Instance { .. }));
}

#[test]
fn editing_the_main_component_updates_instances_in_one_undo_step() {
    let mut s = Sim::new();
    let main = s.by_name("Primary button");
    let inst = s.by_name("Learn more");
    let main_bg = child_of(&s.app().doc, main, "Bg");
    let inst_bg = child_of(&s.app().doc, inst, "Bg");
    let main_label = child_of(&s.app().doc, main, "Label");
    let inst_label = child_of(&s.app().doc, inst, "Label");

    // The second button overrides its Bg fill, so only the label opacity follows.
    let red = vec![designos_doc::Paint::solid(Color::rgb(255, 0, 0))];
    let inst_fill = s.app().doc.get(inst_bg).unwrap().fills.clone();
    s.shell.app.edit_nodes(&[main_bg, main_label], |n| {
        if n.name == "Bg" {
            n.fills = red.clone();
        } else {
            n.opacity = 0.5;
        }
    });
    assert_eq!(s.app().doc.get(inst_label).unwrap().opacity, 0.5);
    assert_eq!(s.app().doc.get(inst_bg).unwrap().fills, inst_fill, "overridden fill is kept");
    let first = s.by_name("Get started");
    assert_eq!(s.app().doc.get(child_of(&s.app().doc, first, "Bg")).unwrap().fills, red, "plain instance follows");
    s.cmd_key(Key::Z);
    assert_eq!(s.app().doc.get(inst_label).unwrap().opacity, 1.0);
    assert_ne!(s.app().doc.get(child_of(&s.app().doc, first, "Bg")).unwrap().fills, red);
    assert!(s.app().doc.get(main_label).unwrap().opacity == 1.0);
}

#[test]
fn editing_inside_an_instance_records_overrides() {
    let mut s = Sim::new();
    let inst = s.by_name("Get started");
    let icon = child_of(&s.app().doc, inst, "Icon");
    let bg = child_of(&s.app().doc, inst, "Bg");
    s.shell.app.edit_nodes(&[bg], |n| n.opacity = 0.4);
    let n = s.app().doc.get(bg).unwrap();
    assert_eq!(n.component.overrides, [OverrideField::Opacity].into_iter().collect());
    // Moving the instance itself is not an override.
    s.shell.app.edit_nodes(&[inst], |n| n.transform.tx += 10.0);
    assert!(!s.app().doc.get(inst).unwrap().component.overrides.contains(&OverrideField::Transform));
    // Main edits don't clobber the override.
    let main = s.by_name("Primary button");
    let main_bg = child_of(&s.app().doc, main, "Bg");
    s.shell.app.edit_nodes(&[main_bg], |n| n.opacity = 0.9);
    assert_eq!(s.app().doc.get(bg).unwrap().opacity, 0.4);
    let _ = icon;
}

#[test]
fn layers_inside_an_instance_are_hidden_not_deleted() {
    let mut s = Sim::new();
    let inst = s.by_name("Get started");
    let bg = child_of(&s.app().doc, inst, "Bg");
    s.shell.app.selection = vec![bg];
    let n0 = s.count();
    s.key(Key::Delete);
    assert_eq!(s.count(), n0);
    let n = s.app().doc.get(bg).unwrap();
    assert!(!n.visible);
    assert!(n.component.overrides.contains(&OverrideField::Visible));
}

#[test]
fn inspector_toggle_changes_the_boolean_property() {
    let mut s = Sim::with(Options { select: Some("Learn more".into()), ..Default::default() });
    let inst = s.by_name("Learn more");
    let icon = child_of(&s.app().doc, inst, "Icon");
    assert!(!s.app().doc.get(icon).unwrap().visible);
    let r = s.rect_of("toggle:prop:Show icon");
    s.click(r.left_center() + Vec2::new(10.0, 0.0));
    assert!(s.app().doc.get(icon).unwrap().visible, "toggle turns the icon on");
    assert_eq!(s.app().doc.get(inst).unwrap().component.values["Show icon"], PropValue::Bool(true));
    s.cmd_key(Key::Z);
    assert!(!s.app().doc.get(icon).unwrap().visible);
}

#[test]
fn inspector_text_property_relabels_the_instance() {
    let mut s = Sim::with(Options { select: Some("Get started".into()), ..Default::default() });
    let inst = s.by_name("Get started");
    let label = child_of(&s.app().doc, inst, "Label");
    let r = s.rect_of("prop:Label");
    s.click(r.center());
    s.frame(vec![Event::Key { key: Key::A, physical_key: Some(Key::A), pressed: true, repeat: false, modifiers: Modifiers::COMMAND }]);
    s.type_text("Go");
    s.key(Key::Enter);
    s.frame(vec![]);
    match &s.app().doc.get(label).unwrap().kind {
        NodeKind::Text(t) => assert_eq!(t.content, "Go"),
        k => panic!("{k:?}"),
    }
}

#[test]
fn instance_header_swaps_and_goes_to_main() {
    let mut s = Sim::with(Options { select: Some("Get started".into()), ..Default::default() });
    let inst = s.by_name("Get started");
    let main = s.by_name("Primary button");
    let go = s.rect_of("go-to-main");
    s.click(go.center());
    assert_eq!(s.app().selection, vec![main]);
    assert_eq!(s.app().doc.page_of(main), Some(s.app().page));
    assert_eq!(s.app().doc.get(s.app().page).unwrap().name, "Components");
    let _ = inst;
}

#[test]
fn component_selected_shows_property_definitions_and_adds_one() {
    let mut s = Sim::with(Options { select: Some("Primary button".into()), ..Default::default() });
    let main = s.by_name("Primary button");
    assert_eq!(s.app().doc.get(main).unwrap().component.props.len(), 2);
    let plus = s.rect_of("prop-add");
    s.click(plus.center());
    assert!(s.app().ui.prop_menu.is_some());
    // First entry of the popup is "Boolean".
    let at = plus.left_bottom() + Vec2::new(-150.0, 2.0);
    s.click(at + Vec2::new(40.0, theme::MENU_PADDING + theme::MENU_ITEM_HEIGHT / 2.0));
    let props = &s.app().doc.get(main).unwrap().component.props;
    assert_eq!(props.len(), 3);
    assert_eq!(props[2].kind, designos_doc::PropKind::Boolean);
    // Delete it again through its minus button.
    let name = props[2].name.clone();
    let del = s.rect_of(&format!("propdel:{name}"));
    s.click(del.center());
    assert_eq!(s.app().doc.get(main).unwrap().component.props.len(), 2);
}

#[test]
fn binding_a_layer_to_a_property_from_its_inspector() {
    let mut s = Sim::new();
    let main = s.by_name("Primary button");
    let bg = child_of(&s.app().doc, main, "Bg");
    s.shell.app.selection = vec![bg];
    s.frame(vec![]);
    s.frame(vec![]);
    let dd = s.rect_of("bind:Visibility");
    s.click(dd.center());
    // Options: None, Show icon.
    let at = dd.left_bottom() + Vec2::new(0.0, 2.0);
    s.click(at + Vec2::new(30.0, theme::MENU_PADDING + theme::MENU_ITEM_HEIGHT * 1.5));
    assert_eq!(s.app().doc.get(bg).unwrap().component.bindings.get(&OverrideField::Visible).map(String::as_str), Some("Show icon"));
}

#[test]
fn assets_panel_lists_components_and_drag_places_an_instance() {
    let mut s = Sim::new();
    s.shell.app.ui.rail = Some(RailItem::Assets);
    s.frame(vec![]);
    s.frame(vec![]);
    let row = s.rect_of("asset:Primary button");
    let main = s.by_name("Primary button");
    let before = designos_doc::instances_of(&s.app().doc, main).len();
    let target = s.screen(300.0, 820.0);
    s.drag(row.center(), target);
    let insts = designos_doc::instances_of(&s.app().doc, main);
    assert_eq!(insts.len(), before + 1);
    let new = *insts.iter().find(|i| s.app().selection == vec![**i]).expect("new instance selected");
    let n = s.app().doc.get(new).unwrap();
    assert_eq!(n.parent, Some(s.app().page));
    let c = center(s.app(), new);
    assert!((c.x - 300.0).abs() < 6.0 && (c.y - 820.0).abs() < 6.0, "centred on the drop point: {c:?}");
    assert_eq!(n.children.len(), 3);
    s.cmd_key(Key::Z);
    assert_eq!(designos_doc::instances_of(&s.app().doc, main).len(), before);
}

#[test]
fn assets_search_filters_rows() {
    let mut s = Sim::new();
    s.shell.app.ui.rail = Some(RailItem::Assets);
    s.shell.app.assets.query = "zzz".into();
    s.frame(vec![]);
    s.frame(vec![]);
    let entries = s.shell.app.local_components();
    assert!(!entries.is_empty());
    assert!(crate::panels::assets::filtered(&entries, "zzz").is_empty());
    assert_eq!(crate::panels::assets::filtered(&entries, "PRIMARY").len(), 1);
    assert_eq!(crate::panels::assets::filtered(&entries, "components").len(), 1, "matches the page name");
}

#[test]
fn alt_drag_duplicate_of_an_instance_stays_an_instance() {
    let mut s = Sim::with(Options { select: Some("Get started".into()), ..Default::default() });
    let inst = s.by_name("Get started");
    let main = s.by_name("Primary button");
    let c = center(s.app(), inst);
    s.mods = Modifiers::ALT;
    let a = s.screen(c.x, c.y);
    s.drag(a, a + Vec2::new(0.0, 90.0));
    s.mods = Modifiers::NONE;
    let insts = designos_doc::instances_of(&s.app().doc, main);
    assert_eq!(insts.len(), 3, "the copy is a third instance");
    let copy = *insts.iter().find(|i| **i != inst && s.app().selection == vec![**i]).expect("copy selected");
    let n = s.app().doc.get(copy).unwrap();
    assert_eq!(n.component.source, Some(main));
    assert!(n.children.iter().all(|k| s.app().doc.get(*k).unwrap().component.source.is_some()));
    // The copy keeps following the main component.
    let main_bg = child_of(&s.app().doc, main, "Bg");
    s.shell.app.edit_nodes(&[main_bg], |n| n.opacity = 0.3);
    assert_eq!(s.app().doc.get(child_of(&s.app().doc, copy, "Bg")).unwrap().opacity, 0.3);
}

#[test]
fn context_menu_detaches_and_combines() {
    let mut s = Sim::new();
    let inst = s.by_name("Get started");
    let c = center(s.app(), inst);
    let p = s.screen(c.x, c.y - 10.0);
    s.right_click(p);
    assert_eq!(s.app().selection, vec![inst]);
    let at = s.app().ui.ctx_menu.expect("menu opens at the pointer");
    let labels = crate::panels::context::labels(s.app());
    let idx = labels.iter().position(|l| l == "Detach instance").unwrap();
    assert!(labels.contains(&"Combine as variants".to_string()) && labels.contains(&"Create component".to_string()));
    // Items are 24px tall with a 17px separator block after "Add auto layout" (index 4).
    let sep = if idx > 4 { crate::widgets::menu::SEP_H } else { 0.0 };
    let y = at.y + theme::MENU_PADDING + idx as f32 * theme::MENU_ITEM_HEIGHT + sep + theme::MENU_ITEM_HEIGHT / 2.0;
    s.click(Pos2::new(at.x + 60.0, y));
    assert!(s.app().ui.ctx_menu.is_none());
    assert!(matches!(s.app().doc.get(inst).unwrap().kind, NodeKind::Frame(_)));
}

#[test]
fn combine_selection_builds_a_component_set() {
    let mut s = Sim::new();
    let main = s.by_name("Primary button");
    let first = s.app().doc.get(main).unwrap().clone();
    s.shell.app.selection = vec![main];
    s.shell.app.duplicate_selection();
    let copy = s.shell.app.selection[0];
    assert_ne!(copy, main);
    s.shell.app.selection = vec![main, copy];
    s.shell.app.combine_selection_as_variants();
    let set = s.app().selection[0];
    let n = s.app().doc.get(set).unwrap();
    assert!(matches!(n.kind, NodeKind::ComponentSet(_)));
    assert_eq!(n.children.len(), 2);
    assert_eq!(s.app().doc.get(main).unwrap().parent, Some(set));
    assert!(s.app().doc.get(main).unwrap().name.contains('='));
    let _ = first;
    s.cmd_key(Key::Z);
    assert!(s.app().doc.get(set).is_none());
}

#[test]
fn convert_frame_to_slot_and_edit_slot_settings_persists_and_undoes() {
    let mut s = Sim::with(Options { select: Some("Primary button".into()), ..Default::default() });
    let main = s.by_name("Primary button");
    // Insert a child Frame named "Leading slot" inside the main component.
    let slot_frame = {
        let id = s.shell.app.doc.alloc_id();
        let mut f = designos_doc::Node::frame(id, designos_doc::Rect::new(8.0, 8.0, 24.0, 24.0));
        f.name = "Leading slot".into();
        s.shell.app.doc.insert(main, f).unwrap()
    };
    s.shell.app.selection = vec![slot_frame];
    s.frame(vec![]);
    s.frame(vec![]);

    // Click "Convert to slot" in the inspector.
    let btn = s.rect_of("convert-to-slot");
    s.click(btn.center());
    s.frame(vec![]);
    s.frame(vec![]);

    assert!(s.app().doc.get(slot_frame).unwrap().component.is_slot, "frame is converted to a slot");
    assert_eq!(s.app().doc.get(slot_frame).unwrap().component.bindings.get(&OverrideField::SlotContent).map(String::as_str), Some("Leading slot"));
    assert!(
        s.app().doc.get(main).unwrap().component.props.iter().any(|p| p.name == "Leading slot" && p.kind == designos_doc::PropKind::Slot),
        "main component has the Slot property"
    );

    // Undo reverts the entire conversion in one undo step.
    s.cmd_key(Key::Z);
    s.frame(vec![]);
    s.frame(vec![]);
    assert!(!s.app().doc.get(slot_frame).unwrap().component.is_slot, "undo reverts is_slot in one step");
    assert!(!s.app().doc.get(main).unwrap().component.props.iter().any(|p| p.name == "Leading slot"));

    // Convert again and configure slot settings in the inspector.
    let btn = s.rect_of("convert-to-slot");
    s.click(btn.center());
    s.frame(vec![]);
    s.frame(vec![]);

    let min_field = s.rect_of("slot-min-children");
    s.click(min_field.center());
    s.frame(vec![Event::Key { key: Key::A, physical_key: Some(Key::A), pressed: true, repeat: false, modifiers: Modifiers::COMMAND }]);
    s.type_text("1");
    s.key(Key::Enter);
    s.frame(vec![]);

    let max_field = s.rect_of("slot-max-children");
    s.click(max_field.center());
    s.frame(vec![Event::Key { key: Key::A, physical_key: Some(Key::A), pressed: true, repeat: false, modifiers: Modifiers::COMMAND }]);
    s.type_text("3");
    s.key(Key::Enter);
    s.frame(vec![]);

    let pref_toggle = s.rect_of("toggle:slot-preferred-only:Leading slot");
    s.click(pref_toggle.left_center() + Vec2::new(10.0, 0.0));
    s.frame(vec![]);

    let stretch_toggle = s.rect_of("toggle:slot-stretch-on-insert:Leading slot");
    s.click(stretch_toggle.left_center() + Vec2::new(10.0, 0.0));
    s.frame(vec![]);

    let empty_toggle = s.rect_of("toggle:slot-show-empty:Leading slot");
    s.click(empty_toggle.left_center() + Vec2::new(10.0, 0.0));
    s.frame(vec![]);

    let cfg = s.app().doc.get(main).unwrap().component.slot_configs.get("Leading slot").expect("slot config persisted");
    assert_eq!(cfg.min_children, Some(1));
    assert_eq!(cfg.max_children, Some(3));
    assert_eq!(cfg.allow_preferred_values_only, Some(true));
    assert_eq!(cfg.stretch_child_on_insert, Some(true));
    assert_eq!(cfg.display_by_default, Some(true));
}

#[test]
fn slot_toggles_of_a_second_slot_change_only_that_slot() {
    let mut s = Sim::with(Options { select: Some("Primary button".into()), ..Default::default() });
    let main = s.by_name("Primary button");
    let mut slot_frames = Vec::new();
    for (i, name) in ["Leading slot", "Trailing slot"].into_iter().enumerate() {
        let id = s.shell.app.doc.alloc_id();
        let mut f = designos_doc::Node::frame(id, designos_doc::Rect::new(8.0 + 30.0 * i as f32, 8.0, 24.0, 24.0));
        f.name = name.into();
        slot_frames.push(s.shell.app.doc.insert(main, f).unwrap());
    }
    for frame in slot_frames {
        s.shell.app.selection = vec![frame];
        s.frame(vec![]);
        s.frame(vec![]);
        let btn = s.rect_of("convert-to-slot");
        s.click(btn.center());
        s.frame(vec![]);
        s.frame(vec![]);
    }

    // Select the main component: both slot properties are listed with their settings.
    s.shell.app.selection = vec![main];
    s.frame(vec![]);
    s.frame(vec![]);
    s.shell.app.props.scroll = 0.0;
    s.frame(vec![]);
    s.frame(vec![]);
    let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(1600.0, 1088.0));
    let cfg_of = |s: &Sim, name: &str| s.app().doc.get(main).unwrap().component.slot_configs.get(name).cloned().unwrap_or_default();

    for (rec, field) in [("slot-preferred-only", 0), ("slot-stretch-on-insert", 1), ("slot-show-empty", 2)] {
        let pick = |c: &designos_doc::SlotPropConfig| match field {
            0 => c.allow_preferred_values_only,
            1 => c.stretch_child_on_insert,
            _ => c.display_by_default,
        };
        let before_first = pick(&cfg_of(&s, "Leading slot"));
        let toggle = s.rect_of(&format!("toggle:{rec}:Trailing slot"));
        assert!(screen.contains_rect(toggle), "{rec} of the second slot is on screen: {toggle:?}");
        s.click(toggle.left_center() + Vec2::new(10.0, 0.0));
        s.frame(vec![]);
        s.frame(vec![]);
        assert_eq!(pick(&cfg_of(&s, "Trailing slot")), Some(true), "{rec}: second slot toggled");
        assert_eq!(pick(&cfg_of(&s, "Leading slot")), before_first, "{rec}: first slot untouched");
    }
}

/// Every text run painted last frame.
fn painted_strings(s: &Sim) -> Vec<String> {
    fn collect(shape: &egui::Shape, out: &mut Vec<String>) {
        match shape {
            egui::Shape::Text(t) => out.push(t.galley.job.text.clone()),
            egui::Shape::Vec(shapes) => shapes.iter().for_each(|x| collect(x, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    for clipped in &s.shapes {
        collect(&clipped.shape, &mut out);
    }
    out
}

/// A slot "Leading slot" in "Primary button" with preferred-only on, selected in the inspector, plus
/// extra components named `names`. Returns the sim, the main component, and the new component ids.
fn preferred_picker_sim(names: &[&str]) -> (Sim, NodeId, Vec<NodeId>) {
    let mut s = Sim::with(Options { select: Some("Primary button".into()), ..Default::default() });
    let main = s.by_name("Primary button");
    let slot_frame = {
        let id = s.shell.app.doc.alloc_id();
        let mut f = designos_doc::Node::frame(id, designos_doc::Rect::new(8.0, 8.0, 24.0, 24.0));
        f.name = "Leading slot".into();
        s.shell.app.doc.insert(main, f).unwrap()
    };
    let page = s.app().doc.pages[0];
    let mut comps = Vec::new();
    for name in names {
        let id = s.shell.app.doc.alloc_id();
        let mut c = designos_doc::Node::frame(id, designos_doc::Rect::new(0.0, 0.0, 16.0, 16.0));
        c.name = (*name).into();
        c.kind = NodeKind::Component(designos_doc::FrameProps::default());
        comps.push(s.shell.app.doc.insert(page, c).unwrap());
    }
    s.shell.app.selection = vec![slot_frame];
    s.frame(vec![]);
    s.frame(vec![]);
    let btn = s.rect_of("convert-to-slot");
    s.click(btn.center());
    s.frame(vec![]);
    s.frame(vec![]);
    let pref_toggle = s.rect_of("toggle:slot-preferred-only:Leading slot");
    s.click(pref_toggle.left_center() + Vec2::new(10.0, 0.0));
    s.frame(vec![]);
    s.frame(vec![]);
    (s, main, comps)
}

/// Row index of `id` in the picker, which lists every component but `main`, ordered by label.
fn picker_row(s: &Sim, main: NodeId, id: NodeId) -> usize {
    let mut all: Vec<NodeId> = s.app().doc.nodes.values().filter(|n| matches!(n.kind, NodeKind::Component(_))).map(|n| n.id).filter(|x| *x != main).collect();
    all.sort_by_key(|x| {
        let n = s.app().doc.get(*x).unwrap();
        match n.parent.and_then(|p| s.app().doc.get(p)).filter(|p| matches!(p.kind, NodeKind::ComponentSet(_))) {
            Some(set) => format!("{} / {}", set.name, n.name),
            None => n.name.clone(),
        }
    });
    all.iter().position(|x| *x == id).expect("component listed in the picker")
}

fn open_preferred_picker(s: &mut Sim) {
    let picker = s.rect_of("slot-preferred-picker");
    s.click(picker.center());
    s.frame(vec![]);
}

fn click_picker_row(s: &mut Sim, row: usize) {
    let r = s.rect_of(&format!("menu-row-0-{row}"));
    s.click(r.center());
    s.frame(vec![]);
    s.frame(vec![]);
}

fn preferred_of(s: &Sim, main: NodeId) -> Vec<NodeId> {
    s.app().doc.get(main).unwrap().component.slot_configs.get("Leading slot").map(|c| c.preferred.clone()).unwrap_or_default()
}

fn row_is_checked(s: &Sim, row: usize) -> bool {
    s.rect_of(&format!("menu-check-0-{row}")).is_positive()
}

#[test]
fn slot_preferred_picker_checks_and_shows_every_selected_component() {
    let (mut s, main, comps) = preferred_picker_sim(&["Icon", "Avatar"]);
    let (icon, avatar) = (comps[0], comps[1]);
    for id in [icon, avatar] {
        open_preferred_picker(&mut s);
        let row = picker_row(&s, main, id);
        click_picker_row(&mut s, row);
    }
    assert_eq!(preferred_of(&s, main), vec![icon, avatar]);
    assert!(painted_strings(&s).iter().any(|t| t == "Icon +1"), "the closed field names the first component and counts the rest: {:?}", painted_strings(&s));

    open_preferred_picker(&mut s);
    let (icon_row, avatar_row) = (picker_row(&s, main, icon), picker_row(&s, main, avatar));
    assert!(row_is_checked(&s, icon_row), "Icon is checked");
    assert!(row_is_checked(&s, avatar_row), "Avatar is checked although it is not the first preferred");
    let others = (0..).take_while(|i| s.ctx.data(|d| d.get_temp::<Rect>(egui::Id::new(("test-rect", format!("menu-check-0-{i}"))))).is_some());
    for i in others.filter(|i| *i != icon_row && *i != avatar_row) {
        assert!(!row_is_checked(&s, i), "row {i} is not preferred");
    }

    click_picker_row(&mut s, icon_row);
    assert_eq!(preferred_of(&s, main), vec![avatar], "clicking a checked row removes only that component");
    assert!(painted_strings(&s).iter().any(|t| t == "Avatar"), "one selection shows its name: {:?}", painted_strings(&s));
}

#[test]
fn slot_preferred_picker_tells_same_named_components_apart_by_id() {
    let (mut s, main, comps) = preferred_picker_sim(&["Icon", "Icon"]);
    let (first, second) = (comps[0], comps[1]);
    open_preferred_picker(&mut s);
    let row = picker_row(&s, main, first);
    click_picker_row(&mut s, row);
    assert_eq!(preferred_of(&s, main), vec![first]);

    open_preferred_picker(&mut s);
    let (first_row, second_row) = (picker_row(&s, main, first), picker_row(&s, main, second));
    assert_ne!(first_row, second_row);
    assert!(row_is_checked(&s, first_row), "the selected Icon is checked");
    assert!(!row_is_checked(&s, second_row), "the other Icon, same label, is not checked");

    click_picker_row(&mut s, second_row);
    assert_eq!(preferred_of(&s, main), vec![first, second], "the second Icon is added, not mistaken for the first");
    open_preferred_picker(&mut s);
    click_picker_row(&mut s, first_row);
    assert_eq!(preferred_of(&s, main), vec![second], "removing the first Icon keeps the second");
}

#[test]
fn instance_slot_add_reorder_remove_keeps_instance_linked_enforces_max_with_toast_and_syncs_non_slot_parts() {
    let mut s = Sim::with(Options { select: Some("Get started".into()), ..Default::default() });
    let main = s.by_name("Primary button");
    let inst = s.by_name("Get started");
    let page = s.app().page;

    let run_cmd = |app: &mut crate::app::App, cmd: designos_doc::Command| {
        let mut tx = crate::edit::Tx::new();
        let _ = tx.cmd(&mut app.doc, cmd);
        app.run_tx(tx);
    };
    let badge_id = s.shell.app.doc.alloc_id();
    let mut badge_node = designos_doc::Node::frame(badge_id, designos_doc::Rect::new(0.0, 0.0, 20.0, 20.0));
    badge_node.name = "Badge".into();
    run_cmd(&mut s.shell.app, designos_doc::Command::Insert { parent: page, index: 0, nodes: vec![badge_node] });
    let cmd = designos_doc::create_component(&mut s.shell.app.doc, badge_id);
    run_cmd(&mut s.shell.app, cmd);
    let badge = badge_id;

    // Add a Slot frame inside the main component with max_children = 2.
    let slot_id = s.shell.app.doc.alloc_id();
    let mut slot_frame = designos_doc::Node::frame(slot_id, designos_doc::Rect::new(8.0, 8.0, 80.0, 24.0));
    slot_frame.name = "Extra".into();
    run_cmd(&mut s.shell.app, designos_doc::Command::Insert { parent: main, index: 1, nodes: vec![slot_frame] });
    let cmd = designos_doc::convert_to_slot(&mut s.shell.app.doc, slot_id);
    run_cmd(&mut s.shell.app, cmd);
    let cmd = designos_doc::set_slot_config(&mut s.shell.app.doc, main, "Extra", designos_doc::SlotPropConfig { max_children: Some(2), ..Default::default() });
    run_cmd(&mut s.shell.app, cmd);

    let inst_slot = child_of(&s.app().doc, inst, "Extra");
    assert!(designos_doc::is_slot_container(&s.app().doc, inst_slot));

    // Insert first child instance into the instance's slot.
    s.shell.app.selection = vec![inst_slot];
    let slot_c = center(s.app(), inst_slot);
    s.shell.app.insert_instance(badge, slot_c);
    let c1 = s.app().doc.get(inst_slot).unwrap().children[0];
    assert_eq!(s.app().doc.get(c1).unwrap().parent, Some(inst_slot));

    // Duplicate c1 inside the slot -> 2 children in slot.
    s.shell.app.selection = vec![c1];
    s.shell.app.duplicate_selection();
    let children = s.app().doc.get(inst_slot).unwrap().children.clone();
    assert_eq!(children.len(), 2);
    let c2 = children[1];

    // Reorder c1 after c2 inside the instance slot, then undo/redo.
    s.shell.app.selection = vec![c1];
    s.shell.app.reorder(true, false);
    assert_eq!(s.app().doc.get(inst_slot).unwrap().children, vec![c2, c1]);
    s.shell.app.undo();
    assert_eq!(s.app().doc.get(inst_slot).unwrap().children, vec![c1, c2]);

    // Attempting to add a 3rd child is blocked by max_children = 2 and shows a toast.
    s.shell.app.toast = None;
    s.shell.app.selection = vec![c2];
    s.shell.app.duplicate_selection();
    assert_eq!(s.app().doc.get(inst_slot).unwrap().children.len(), 2, "max_children enforced");
    assert!(s.app().toast.is_some(), "toast shown when slot max_children is reached");

    // Removing a child inside the instance slot succeeds without detaching the instance.
    s.shell.app.selection = vec![c2];
    s.shell.app.delete_selection();
    assert_eq!(s.app().doc.get(inst_slot).unwrap().children, vec![c1]);
    assert!(matches!(s.app().doc.get(inst).unwrap().kind, NodeKind::Instance { .. }), "instance remains linked");

    // Main component updates to non-slot siblings still propagate to the instance without wiping slot content.
    let main_bg = child_of(&s.app().doc, main, "Bg");
    s.shell.app.edit_nodes(&[main_bg], |n| n.opacity = 0.45);
    assert_eq!(s.app().doc.get(child_of(&s.app().doc, inst, "Bg")).unwrap().opacity, 0.45);
    assert_eq!(s.app().doc.get(inst_slot).unwrap().children, vec![c1], "custom slot children preserved after main sync");

    // Select the instance's slot frame to verify the Slot content inspector section.
    s.shell.app.selection = vec![inst_slot];
    s.frame(vec![]);
    s.frame(vec![]);
    assert!(s.rect_of("slot-add-child").width() > 0.0, "Slot content add-instance dropdown rendered");
    assert!(s.rect_of(&format!("slot-child-del:{}", c1.0)).width() > 0.0, "Slot child remove button rendered");
}

#[test]
fn empty_slot_placeholder_preferred_picker_and_stretch_on_insert_ui() {
    let mut s = Sim::with(Options { select: Some("Get started".into()), ..Default::default() });
    let main = s.by_name("Primary button");
    let inst = s.by_name("Get started");
    let page = s.app().page;

    let run_cmd = |app: &mut crate::app::App, cmd: designos_doc::Command| {
        let mut tx = crate::edit::Tx::new();
        let _ = tx.cmd(&mut app.doc, cmd);
        app.run_tx(tx);
    };

    // Create two candidate components: PreferredBadge (20x20) and UnrelatedChip (30x20).
    let badge = s.shell.app.doc.alloc_id();
    let mut badge_node = designos_doc::Node::frame(badge, designos_doc::Rect::new(0.0, 0.0, 20.0, 20.0));
    badge_node.name = "PreferredBadge".into();
    run_cmd(&mut s.shell.app, designos_doc::Command::Insert { parent: page, index: 0, nodes: vec![badge_node] });
    let cmd = designos_doc::create_component(&mut s.shell.app.doc, badge);
    run_cmd(&mut s.shell.app, cmd);

    let chip = s.shell.app.doc.alloc_id();
    let mut chip_node = designos_doc::Node::frame(chip, designos_doc::Rect::new(0.0, 30.0, 30.0, 20.0));
    chip_node.name = "UnrelatedChip".into();
    run_cmd(&mut s.shell.app, designos_doc::Command::Insert { parent: page, index: 1, nodes: vec![chip_node] });
    let cmd = designos_doc::create_component(&mut s.shell.app.doc, chip);
    run_cmd(&mut s.shell.app, cmd);

    // Create an empty Slot frame (width = 96) inside the main component.
    let slot_id = s.shell.app.doc.alloc_id();
    let mut slot_frame = designos_doc::Node::frame(slot_id, designos_doc::Rect::new(8.0, 8.0, 96.0, 32.0));
    slot_frame.name = "SlotArea".into();
    run_cmd(&mut s.shell.app, designos_doc::Command::Insert { parent: main, index: 1, nodes: vec![slot_frame] });
    let cmd = designos_doc::convert_to_slot(&mut s.shell.app.doc, slot_id);
    run_cmd(&mut s.shell.app, cmd);

    // Configure preferred = [badge], stretch_child_on_insert = true, display_by_default = false first.
    let cmd = designos_doc::set_slot_config(
        &mut s.shell.app.doc,
        main,
        "SlotArea",
        designos_doc::SlotPropConfig { display_by_default: Some(false), stretch_child_on_insert: Some(true), preferred: vec![badge], ..Default::default() },
    );
    run_cmd(&mut s.shell.app, cmd);

    let inst_slot = child_of(&s.app().doc, inst, "SlotArea");
    s.frame(vec![]);
    s.frame(vec![]);

    // 1. With display_by_default = false, main empty slot shows placeholder (+ button), instance empty slot hides it.
    assert!(crate::canvas::draw::empty_slot_plus_button_rect(s.app(), s.app().canvas_rect, slot_id).is_some());
    assert!(crate::canvas::draw::empty_slot_plus_button_rect(s.app(), s.app().canvas_rect, inst_slot).is_none());

    // Enable display_by_default = true: both main and instance empty slots show placeholder (+ button).
    let cmd = designos_doc::set_slot_config(
        &mut s.shell.app.doc,
        main,
        "SlotArea",
        designos_doc::SlotPropConfig { display_by_default: Some(true), stretch_child_on_insert: Some(true), preferred: vec![badge], ..Default::default() },
    );
    run_cmd(&mut s.shell.app, cmd);
    s.shell.app.selection = vec![inst_slot];
    s.frame(vec![]);
    s.frame(vec![]);
    assert!(crate::canvas::draw::empty_slot_plus_button_rect(s.app(), s.app().canvas_rect, slot_id).is_some());
    assert!(crate::canvas::draw::empty_slot_plus_button_rect(s.app(), s.app().canvas_rect, inst_slot).is_some());
    assert!(s.rect_of(&format!("canvas-slot-plus:{}", inst_slot.0)).width() > 0.0);

    // 2. Preferred-instance picker lists only PreferredBadge (not UnrelatedChip) when preferred is set.
    assert_eq!(designos_doc::preferred_slot_components(&s.app().doc, inst_slot), vec![badge]);

    // Click the on-canvas `+` button on the empty instance slot to open the preferred-instances picker and pick PreferredBadge.
    let plus_center = s.rect_of(&format!("canvas-slot-plus:{}", inst_slot.0)).center();
    s.click(plus_center);
    assert!(s.app().ui.slot_picker.is_some());
    s.click(s.rect_of("menu-row-0-0").center());

    // 3. The inserted child is inside inst_slot, stretched to width 96.0 with Sizing::Fill, and the empty-slot placeholder is now gone.
    let children = s.app().doc.get(inst_slot).unwrap().children.clone();
    assert_eq!(children.len(), 1);
    let inserted = s.app().doc.get(children[0]).unwrap();
    assert_eq!(inserted.layout_child.horizontal, designos_doc::Sizing::Fill);
    assert_eq!(inserted.size.x, 96.0);
    assert!(crate::canvas::draw::empty_slot_plus_button_rect(s.app(), s.app().canvas_rect, inst_slot).is_none());
}

#[test]
fn migrate_to_slot_in_inspector_and_app_preserves_instance_content_and_undoes_in_one_step() {
    let mut s = Sim::new();
    let main = s.by_name("Primary button");
    let bg = child_of(&s.app().doc, main, "Bg");

    // Unsupported leaf shape is a no-op with an honest message and toast.
    s.shell.app.selection = vec![bg];
    let before_nodes = s.app().doc.nodes.clone();
    let err = s.shell.app.migrate_selection_to_slot().expect_err("rectangle leaf should be rejected");
    assert!(!err.is_empty());
    assert_eq!(s.app().doc.nodes, before_nodes);
    assert!(s.app().toast.as_ref().is_some_and(|t| t.error && t.text == err));

    // Create a Card component on the Welcome page with a nested instance of Primary button bound to an InstanceSwap prop.
    let page = s.app().page;
    let cid = s.shell.app.doc.alloc_id();
    let mut card = designos_doc::Node::frame(cid, designos_doc::Rect::new(40.0, 600.0, 240.0, 80.0));
    card.name = "Slot Card".into();
    let card = s.shell.app.doc.insert(page, card).unwrap();
    designos_doc::create_component(&mut s.shell.app.doc, card).apply(&mut s.shell.app.doc).unwrap();
    designos_doc::create_instance(&mut s.shell.app.doc, main, card, DV::new(16.0, 16.0)).apply(&mut s.shell.app.doc).unwrap();
    let region = *s.app().doc.get(card).unwrap().children.last().unwrap();
    designos_doc::add_prop(
        &mut s.shell.app.doc,
        card,
        designos_doc::ComponentPropDef {
            name: "Action".into(),
            kind: designos_doc::PropKind::InstanceSwap { preferred: vec![main] },
            default: PropValue::Instance(main),
        },
    )
    .apply(&mut s.shell.app.doc)
    .unwrap();
    designos_doc::bind_prop(&mut s.shell.app.doc, region, OverrideField::MainComponent, Some("Action")).apply(&mut s.shell.app.doc).unwrap();

    // Create an instance of Slot Card on the page.
    designos_doc::create_instance(&mut s.shell.app.doc, card, page, DV::new(340.0, 600.0)).apply(&mut s.shell.app.doc).unwrap();
    let card_inst = *s.app().doc.get(page).unwrap().children.last().unwrap();

    // Select `region` inside `Slot Card` and click the inspector's "Migrate to slot" button.
    s.shell.app.selection = vec![region];
    s.frame(vec![]);
    s.frame(vec![]);
    let pre_migrate_nodes = s.app().doc.nodes.clone();
    let btn = s.rect_of("migrate-to-slot");
    s.click(btn.center());

    assert!(s.app().doc.get(region).unwrap().component.is_slot);
    assert_eq!(s.app().doc.get(card).unwrap().component.props[0].kind, designos_doc::PropKind::Slot);
    let inst_slot = s.app().doc.get(card_inst).unwrap().children[0];
    assert!(s.app().doc.get(inst_slot).unwrap().component.is_slot);
    assert_eq!(s.app().doc.get(inst_slot).unwrap().children.len(), 1);
    let slot_child = s.app().doc.get(inst_slot).unwrap().children[0];
    assert!(s.app().doc.get(slot_child).unwrap().component.is_slot_content);

    // One Cmd+Z undoes the entire migration across the component and its instance.
    s.cmd_key(Key::Z);
    assert_eq!(s.app().doc.nodes, pre_migrate_nodes);
}
