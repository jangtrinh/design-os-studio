//! Expose properties from nested instances: the main component's toggles, the outer instance's property rows, and
//! what edits, syncs and resets do to them.

use super::*;
use designos_doc::{ComponentPropDef, OverrideField, PropKind, PropValue};

fn child_of(d: &designos_doc::Document, parent: NodeId, name: &str) -> NodeId {
    *d.get(parent).unwrap().children.iter().find(|c| d.get(**c).unwrap().name == name).unwrap_or_else(|| panic!("no child {name}"))
}

fn recorded_rect(s: &Sim, name: &str) -> Option<Rect> {
    s.ctx.data(|d| d.get_temp::<Rect>(egui::Id::new(("test-rect", name.to_string()))))
}

fn clear_recorded_rect(s: &mut Sim, name: &str) {
    s.ctx.data_mut(|d| d.remove::<Rect>(egui::Id::new(("test-rect", name.to_string()))));
}

/// The toggle of one nested instance in a main component's "Nested instances" list. It is keyed by the node, so two
/// nested instances with one name have two toggles.
fn expose_toggle_key(nested: NodeId) -> String {
    format!("expose-nested:{}", nested.0)
}

/// Builds a scene with:
/// - `IconA` and `IconB` components;
/// - `Badge` component set (`Size=Sm` width 60, `Size=Lg` width 90) with properties registered in non-Figma order:
///   Text (`Caption`), InstanceSwap (`Glyph`), Boolean (`Show dot`), plus Variant (`Size`);
/// - `Card` main component with a background rect `CardBg` and two nested `Badge` instances:
///   `Leading badge` and `Trailing badge`;
/// - `CardInst` outer instance of `Card`.
fn setup_nested_expose_scene() -> (Sim, NodeId, NodeId, NodeId, NodeId, NodeId, NodeId) {
    let mut s = Sim::new();
    let page = s.app().page;

    let mut make_icon = |name: &str, x: f32| {
        let id = s.shell.app.doc.alloc_id();
        let mut f = designos_doc::Node::frame(id, designos_doc::Rect::new(x, 420.0, 12.0, 12.0));
        f.name = name.into();
        let comp = s.shell.app.doc.insert(page, f).unwrap();
        designos_doc::create_component(&mut s.shell.app.doc, comp).apply(&mut s.shell.app.doc).unwrap();
        comp
    };
    let icon_a = make_icon("IconA", 40.0);
    let icon_b = make_icon("IconB", 64.0);

    let mut make_badge_variant = |name: &str, x: f32, w: f32| {
        let id = s.shell.app.doc.alloc_id();
        let mut f = designos_doc::Node::frame(id, designos_doc::Rect::new(x, 460.0, w, 24.0));
        f.name = name.into();
        let comp = s.shell.app.doc.insert(page, f).unwrap();
        let dot_id = s.shell.app.doc.alloc_id();
        let mut dot = designos_doc::Node::ellipse(dot_id, designos_doc::Rect::new(4.0, 8.0, 8.0, 8.0));
        dot.name = "Dot".into();
        let dot = s.shell.app.doc.insert(comp, dot).unwrap();
        let txt_id = s.shell.app.doc.alloc_id();
        let mut txt = designos_doc::Node::text(txt_id, designos_doc::Rect::new(16.0, 4.0, 40.0, 16.0), "New", designos_doc::TextStyle::default());
        txt.name = "Text".into();
        let txt = s.shell.app.doc.insert(comp, txt).unwrap();
        designos_doc::create_component(&mut s.shell.app.doc, comp).apply(&mut s.shell.app.doc).unwrap();
        designos_doc::create_instance(&mut s.shell.app.doc, icon_a, comp, DV::new(40.0, 6.0)).apply(&mut s.shell.app.doc).unwrap();
        let ico = *s.app().doc.get(comp).unwrap().children.last().unwrap();
        s.shell.app.doc.get_mut(ico).unwrap().name = "IconSlot".into();
        (comp, dot, txt, ico)
    };
    let (v_sm, dot_sm, txt_sm, ico_sm) = make_badge_variant("Size=Sm", 40.0, 60.0);
    let (v_lg, dot_lg, txt_lg, ico_lg) = make_badge_variant("Size=Lg", 120.0, 90.0);
    designos_doc::combine_as_variants(&mut s.shell.app.doc, &[v_sm, v_lg]).apply(&mut s.shell.app.doc).unwrap();
    let badge_set = s.app().doc.get(v_sm).unwrap().parent.unwrap();
    s.shell.app.doc.get_mut(badge_set).unwrap().name = "Badge".into();

    // Intentionally add Text, InstanceSwap, then Boolean so the panel must sort them into Variant, Boolean, Text, InstanceSwap.
    designos_doc::add_prop(
        &mut s.shell.app.doc,
        badge_set,
        ComponentPropDef { name: "Caption".into(), kind: PropKind::Text, default: PropValue::Text("New".into()) },
    )
    .apply(&mut s.shell.app.doc)
    .unwrap();
    designos_doc::add_prop(
        &mut s.shell.app.doc,
        badge_set,
        ComponentPropDef { name: "Glyph".into(), kind: PropKind::InstanceSwap { preferred: vec![icon_a, icon_b] }, default: PropValue::Instance(icon_a) },
    )
    .apply(&mut s.shell.app.doc)
    .unwrap();
    designos_doc::add_prop(
        &mut s.shell.app.doc,
        badge_set,
        ComponentPropDef { name: "Show dot".into(), kind: PropKind::Boolean, default: PropValue::Bool(true) },
    )
    .apply(&mut s.shell.app.doc)
    .unwrap();
    for (dot, txt, ico) in [(dot_sm, txt_sm, ico_sm), (dot_lg, txt_lg, ico_lg)] {
        designos_doc::bind_prop(&mut s.shell.app.doc, dot, OverrideField::Visible, Some("Show dot")).apply(&mut s.shell.app.doc).unwrap();
        designos_doc::bind_prop(&mut s.shell.app.doc, txt, OverrideField::Characters, Some("Caption")).apply(&mut s.shell.app.doc).unwrap();
        designos_doc::bind_prop(&mut s.shell.app.doc, ico, OverrideField::MainComponent, Some("Glyph")).apply(&mut s.shell.app.doc).unwrap();
    }

    let card_id = s.shell.app.doc.alloc_id();
    let mut card = designos_doc::Node::frame(card_id, designos_doc::Rect::new(40.0, 540.0, 220.0, 56.0));
    card.name = "Card".into();
    let card = s.shell.app.doc.insert(page, card).unwrap();
    let bg_id = s.shell.app.doc.alloc_id();
    let mut bg = designos_doc::Node::rectangle(bg_id, designos_doc::Rect::new(0.0, 0.0, 220.0, 56.0));
    bg.name = "CardBg".into();
    s.shell.app.doc.insert(card, bg).unwrap();
    designos_doc::create_component(&mut s.shell.app.doc, card).apply(&mut s.shell.app.doc).unwrap();

    designos_doc::create_instance(&mut s.shell.app.doc, v_sm, card, DV::new(12.0, 16.0)).apply(&mut s.shell.app.doc).unwrap();
    let leading = *s.app().doc.get(card).unwrap().children.last().unwrap();
    s.shell.app.doc.get_mut(leading).unwrap().name = "Leading badge".into();

    designos_doc::create_instance(&mut s.shell.app.doc, v_sm, card, DV::new(112.0, 16.0)).apply(&mut s.shell.app.doc).unwrap();
    let trailing = *s.app().doc.get(card).unwrap().children.last().unwrap();
    s.shell.app.doc.get_mut(trailing).unwrap().name = "Trailing badge".into();

    designos_doc::create_instance(&mut s.shell.app.doc, card, page, DV::new(300.0, 540.0)).apply(&mut s.shell.app.doc).unwrap();
    let card_inst = *s.app().doc.get(page).unwrap().children.last().unwrap();
    s.shell.app.doc.get_mut(card_inst).unwrap().name = "CardInst".into();

    (s, card, leading, trailing, card_inst, v_sm, v_lg)
}

#[test]
fn expose_nested_instances_toggle_order_edit_isolation_sync_and_unexpose() {
    let (mut s, card, leading, trailing, card_inst, v_sm, v_lg) = setup_nested_expose_scene();

    // (1) Selecting the main component shows "Expose properties from" / "Nested instances" toggles.
    // Turning expose on sets `expose_nested_instances`, and Cmd+Z undoes it in one step.
    s.shell.app.selection = vec![card];
    s.frame(vec![]);
    s.frame(vec![]);
    assert!(s.rect_of("expose-nested-heading").width() > 0.0);
    assert!(s.rect_of("nested-instances-heading").width() > 0.0);
    let leading_tog = s.rect_of(&expose_toggle_key(leading));
    let trailing_tog = s.rect_of(&expose_toggle_key(trailing));
    assert!(leading_tog.width() > 0.0 && trailing_tog.width() > 0.0);
    assert!(!s.app().doc.get(leading).unwrap().component.expose_nested_instances);

    s.click(leading_tog.center());
    assert!(s.app().doc.get(leading).unwrap().component.expose_nested_instances, "clicking toggle exposes Leading badge");
    assert!(!s.app().doc.get(trailing).unwrap().component.expose_nested_instances, "Trailing badge stays unexposed");

    s.cmd_key(Key::Z);
    assert!(!s.app().doc.get(leading).unwrap().component.expose_nested_instances, "Cmd+Z undoes expose toggle in one step");

    // Re-enable expose on Leading badge.
    let leading_tog = s.rect_of(&expose_toggle_key(leading));
    s.click(leading_tog.center());
    assert!(s.app().doc.get(leading).unwrap().component.expose_nested_instances);

    // (2) Selecting the outer instance lists the exposed nested instance's properties under its name,
    // in Figma's order (Variant, Boolean, Text, InstanceSwap), and omits unexposed nested instances.
    for key in [
        "exposed-heading:Leading badge",
        "exposed-heading:Trailing badge",
        "prop:Leading badge/Size",
        "prop:Leading badge/Show dot",
        "prop:Leading badge/Caption",
        "prop:Leading badge/Glyph",
        "prop:Trailing badge/Size",
        "prop:Trailing badge/Show dot",
    ] {
        clear_recorded_rect(&mut s, key);
    }
    s.shell.app.selection = vec![card_inst];
    s.frame(vec![]);
    s.frame(vec![]);

    let hdr_r = s.rect_of("exposed-heading:Leading badge");
    let var_r = s.rect_of("prop:Leading badge/Size");
    let bool_r = s.rect_of("prop:Leading badge/Show dot");
    let text_r = s.rect_of("prop:Leading badge/Caption");
    let swap_r = s.rect_of("prop:Leading badge/Glyph");
    assert!(
        hdr_r.top() < var_r.top() && var_r.top() < bool_r.top() && bool_r.top() < text_r.top() && text_r.top() < swap_r.top(),
        "exposed properties must be listed under the nested instance heading in Figma order (Variant, Boolean, Text, InstanceSwap): hdr={} var={} bool={} text={} swap={}",
        hdr_r.top(),
        var_r.top(),
        bool_r.top(),
        text_r.top(),
        swap_r.top()
    );
    assert!(recorded_rect(&s, "exposed-heading:Trailing badge").is_none(), "unexposed nested instance heading must not be rendered");
    assert!(recorded_rect(&s, "prop:Trailing badge/Size").is_none(), "unexposed nested instance properties must not be rendered");
    assert!(recorded_rect(&s, "prop:Trailing badge/Show dot").is_none(), "unexposed nested instance boolean property must not be rendered");

    // (3) Editing an exposed boolean or variant property on the outer instance changes only the nested instance
    // inside that outer instance (not the main component), is one undo step, and survives editing the main component.
    let inst_leading = child_of(&s.app().doc, card_inst, "Leading badge");
    let inst_dot = child_of(&s.app().doc, inst_leading, "Dot");
    let main_dot = child_of(&s.app().doc, leading, "Dot");
    assert!(s.app().doc.get(inst_dot).unwrap().visible);
    assert!(s.app().doc.get(main_dot).unwrap().visible);

    let bool_tog = s.rect_of("toggle:prop:Leading badge/Show dot");
    s.click(bool_tog.left_center() + Vec2::new(10.0, 0.0));
    assert!(!s.app().doc.get(inst_dot).unwrap().visible, "outer instance's nested Dot is hidden");
    assert!(s.app().doc.get(main_dot).unwrap().visible, "main component's nested Dot remains visible");

    // One undo step restores the outer instance's nested boolean property.
    s.cmd_key(Key::Z);
    assert!(s.app().doc.get(inst_dot).unwrap().visible, "Cmd+Z undoes exposed boolean property edit in one step");

    // Toggle it off again and also switch the exposed Variant property from Sm to Lg via the dropdown.
    let bool_tog = s.rect_of("toggle:prop:Leading badge/Show dot");
    s.click(bool_tog.left_center() + Vec2::new(10.0, 0.0));
    assert!(!s.app().doc.get(inst_dot).unwrap().visible);

    let var_dd = s.rect_of("prop:Leading badge/Size");
    s.click(var_dd.center());
    s.click(s.rect_of("menu-row-0-1").center());
    assert_eq!(designos_doc::instance_component(s.app().doc.get(inst_leading).unwrap()), Some(v_lg), "outer instance's nested badge switched to Size=Lg");
    assert_eq!(designos_doc::instance_component(s.app().doc.get(leading).unwrap()), Some(v_sm), "main component's nested badge stays on Size=Sm");
    assert!(
        s.app().doc.get(inst_leading).unwrap().component.overrides.contains(&OverrideField::MainComponent),
        "the variant edit is recorded on the nested instance as a swap, the way a swap of a nested layer is"
    );
    let size_def = designos_doc::prop_defs(&s.app().doc, v_sm).into_iter().find(|d| d.name == "Size").expect("the Size property");
    assert_eq!(designos_doc::prop_value(&s.app().doc, inst_leading, &size_def), PropValue::Variant("Lg".into()), "the panel reads the new variant back");

    // One undo step restores the variant change.
    s.cmd_key(Key::Z);
    assert_eq!(designos_doc::instance_component(s.app().doc.get(inst_leading).unwrap()), Some(v_sm), "Cmd+Z undoes exposed variant property edit in one step");

    // Re-apply the variant change to Lg.
    let var_dd = s.rect_of("prop:Leading badge/Size");
    s.click(var_dd.center());
    s.click(s.rect_of("menu-row-0-1").center());
    assert_eq!(designos_doc::instance_component(s.app().doc.get(inst_leading).unwrap()), Some(v_lg));

    // Edit the main component `Card` (syncs all instances of `Card`): overrides on `inst_leading` must survive.
    let card_bg = child_of(&s.app().doc, card, "CardBg");
    s.shell.app.edit_nodes(&[card_bg], |n| {
        n.opacity = 0.85;
    });
    assert_eq!(
        designos_doc::instance_component(s.app().doc.get(inst_leading).unwrap()),
        Some(v_lg),
        "exposed variant override on nested instance survives editing/syncing the main component"
    );
    let inst_dot_after_sync = child_of(&s.app().doc, inst_leading, "Dot");
    assert!(!s.app().doc.get(inst_dot_after_sync).unwrap().visible, "exposed boolean override on nested instance survives editing/syncing the main component");

    // (4) Turning expose off on the main component hides the nested instance's properties from the outer instance panel
    // while keeping existing overrides on instances intact.
    s.shell.app.selection = vec![card];
    s.frame(vec![]);
    s.frame(vec![]);
    let leading_tog = s.rect_of(&expose_toggle_key(leading));
    s.click(leading_tog.center());
    assert!(!s.app().doc.get(leading).unwrap().component.expose_nested_instances);

    for key in ["exposed-heading:Leading badge", "prop:Leading badge/Size", "prop:Leading badge/Show dot", "toggle:prop:Leading badge/Show dot"] {
        clear_recorded_rect(&mut s, key);
    }
    s.shell.app.selection = vec![card_inst];
    s.frame(vec![]);
    s.frame(vec![]);

    assert!(recorded_rect(&s, "exposed-heading:Leading badge").is_none(), "turning expose off hides the nested instance heading from the outer instance panel");
    assert!(recorded_rect(&s, "prop:Leading badge/Size").is_none(), "turning expose off hides the nested instance properties from the outer instance panel");
    assert_eq!(designos_doc::instance_component(s.app().doc.get(inst_leading).unwrap()), Some(v_lg), "turning expose off keeps variant override intact");
    assert!(!s.app().doc.get(inst_dot_after_sync).unwrap().visible, "turning expose off keeps boolean override intact");
}

#[test]
fn same_named_nested_instances_have_their_own_expose_toggles_and_property_rows() {
    let (mut s, card, leading, trailing, card_inst, _, _) = setup_nested_expose_scene();
    // Both nested badges are called "Leading badge", in the main and in its instance.
    let inst_leading = child_of(&s.app().doc, card_inst, "Leading badge");
    let inst_trailing = child_of(&s.app().doc, card_inst, "Trailing badge");
    for id in [trailing, inst_trailing] {
        s.shell.app.doc.get_mut(id).unwrap().name = "Leading badge".into();
    }

    s.shell.app.selection = vec![card];
    s.frame(vec![]);
    s.frame(vec![]);
    let trailing_tog = s.rect_of(&expose_toggle_key(trailing));
    assert!(s.rect_of(&expose_toggle_key(leading)).width() > 0.0, "each of the two same-named nested instances has a toggle");

    s.click(trailing_tog.center());
    assert!(s.app().doc.get(trailing).unwrap().component.expose_nested_instances, "the toggle exposes the nested instance it sits on");
    assert!(!s.app().doc.get(leading).unwrap().component.expose_nested_instances, "the other nested instance is not exposed with it");

    let row_of = |nested: NodeId| format!("prop-node:{}/Show dot", nested.0);
    for id in [inst_leading, inst_trailing] {
        clear_recorded_rect(&mut s, &row_of(id));
    }
    s.shell.app.selection = vec![card_inst];
    s.frame(vec![]);
    s.frame(vec![]);
    assert!(recorded_rect(&s, &row_of(inst_trailing)).is_some(), "the exposed nested instance lists its properties");
    assert!(recorded_rect(&s, &row_of(inst_leading)).is_none(), "the unexposed one, with the same name, does not");

    // Exposing the other one too lists both; an edit on one row changes that nested instance only.
    s.shell.app.selection = vec![card];
    s.frame(vec![]);
    s.frame(vec![]);
    let leading_tog = s.rect_of(&expose_toggle_key(leading));
    s.click(leading_tog.center());
    s.shell.app.selection = vec![card_inst];
    s.frame(vec![]);
    s.frame(vec![]);
    let trailing_dot = child_of(&s.app().doc, inst_trailing, "Dot");
    let leading_dot = child_of(&s.app().doc, inst_leading, "Dot");
    let toggle = s.rect_of(&row_of(inst_trailing));
    s.click(toggle.left_center() + Vec2::new(10.0, 0.0));
    assert!(!s.app().doc.get(trailing_dot).unwrap().visible, "the edited nested instance hides its dot");
    assert!(s.app().doc.get(leading_dot).unwrap().visible, "the same-named one keeps its dot");
}

#[test]
fn reset_all_changes_counts_nested_values_only_while_the_nested_instance_is_exposed() {
    let (mut s, _, leading, _, card_inst, _, _) = setup_nested_expose_scene();
    let inst_leading = child_of(&s.app().doc, card_inst, "Leading badge");
    s.shell.app.set_instance_prop(inst_leading, "Show dot", Some(PropValue::Bool(false)));
    assert!(!crate::panels::components::has_changes(s.app(), card_inst), "a value on an unexposed nested instance is not a change of the outer instance");

    s.shell.app.set_expose_nested_instance(leading, true);
    assert!(crate::panels::components::has_changes(s.app(), card_inst), "once the nested instance is exposed the same value is a change");

    s.shell.app.selection = vec![card_inst];
    s.shell.app.reset_selection();
    assert!(s.app().doc.get(inst_leading).unwrap().component.values.is_empty(), "Reset all changes takes the exposed value back");
    assert!(!crate::panels::components::has_changes(s.app(), card_inst));
}

#[test]
fn reset_all_changes_takes_back_a_variant_edit_made_from_the_outer_instance() {
    let (mut s, card, leading, _, card_inst, v_sm, _) = setup_nested_expose_scene();
    s.shell.app.set_expose_nested_instance(leading, true);
    s.shell.app.selection = vec![card_inst];
    s.frame(vec![]);
    s.frame(vec![]);
    let inst_leading = child_of(&s.app().doc, card_inst, "Leading badge");

    let var_dd = s.rect_of("prop:Leading badge/Size");
    s.click(var_dd.center());
    s.click(s.rect_of("menu-row-0-1").center());
    assert_ne!(designos_doc::instance_component(s.app().doc.get(inst_leading).unwrap()), Some(v_sm), "the edit switched the nested badge");

    // Editing the main syncs every instance of it; the switch stays.
    let card_bg = child_of(&s.app().doc, card, "CardBg");
    s.shell.app.edit_nodes(&[card_bg], |n| n.opacity = 0.5);
    assert_ne!(designos_doc::instance_component(s.app().doc.get(inst_leading).unwrap()), Some(v_sm), "a sync of the main keeps the switch");
    assert!(crate::panels::components::has_changes(s.app(), card_inst), "the switch is a change of the outer instance");

    s.shell.app.selection = vec![card_inst];
    s.shell.app.reset_selection();
    assert_eq!(designos_doc::instance_component(s.app().doc.get(inst_leading).unwrap()), Some(v_sm), "Reset all changes puts the nested badge back");
    assert!(!crate::panels::components::has_changes(s.app(), card_inst));
}

#[test]
fn reset_all_changes_leaves_a_nested_value_the_outer_instance_inherits() {
    let (mut s, card, leading, _, card_inst, _, _) = setup_nested_expose_scene();
    s.shell.app.set_expose_nested_instance(leading, true);
    let inst_leading = child_of(&s.app().doc, card_inst, "Leading badge");
    let inst_dot = child_of(&s.app().doc, inst_leading, "Dot");

    // The main-side badge hides its dot after the outer instance exists, and the card syncs.
    s.shell.app.set_instance_prop(leading, "Show dot", Some(PropValue::Bool(false)));
    let card_bg = child_of(&s.app().doc, card, "CardBg");
    s.shell.app.edit_nodes(&[card_bg], |n| n.opacity = 0.9);
    assert!(!s.app().doc.get(inst_dot).unwrap().visible, "the outer instance's badge follows the main-side badge");
    assert!(s.app().doc.get(inst_leading).unwrap().component.values.is_empty(), "the badge in the outer instance holds no value of its own");
    assert!(!crate::panels::components::has_changes(s.app(), card_inst), "an inherited value is not a change of the outer instance");

    // Another change makes Reset available; it must not pin the inherited value.
    s.shell.app.edit_nodes(&[card_inst], |n| n.opacity = 0.5);
    assert!(crate::panels::components::has_changes(s.app(), card_inst), "the other change is offered for Reset");
    s.shell.app.selection = vec![card_inst];
    s.shell.app.reset_selection();
    assert!(s.app().doc.get(inst_leading).unwrap().component.values.is_empty(), "Reset all changes does not copy the main-side value onto the badge");

    s.shell.app.set_instance_prop(leading, "Show dot", Some(PropValue::Bool(true)));
    s.shell.app.edit_nodes(&[card_bg], |n| n.opacity = 0.8);
    assert!(s.app().doc.get(inst_dot).unwrap().visible, "the badge still follows the main-side badge after Reset");
}
