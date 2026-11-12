//! Picks through the cases that rebuild, rename, detach or re-resolve the instance a pick sits on: an ancestor switching
//! its variant, an instance-swap property, a text binding, renaming or deleting the property, a spec override, a detach.
//!
//! Fixture: the one of `component_variant_picks_tests`, plus a set `Card` (`Kind` A / B) whose variants each hold a
//! `Button` instance `btn` with its `Size` bound to the variable `size`.

use super::component_variant_picks_tests::*;
use crate::*;

struct Cx {
    f: Px,
    cards: [NodeId; 2],
    /// An instance of the first card variant in the host.
    top: NodeId,
}

fn card_fixture() -> Cx {
    let mut f = build(&[], &[]);
    f.d.variables.insert("kind".into(), string_variable("kind", "A", "B"));
    f.d.variable_collections.get_mut("theme").unwrap().variables.push("kind".into());
    let page = f.d.get(f.host).unwrap().parent.unwrap();
    let mut mains = Vec::new();
    let mut buttons = Vec::new();
    for (name, y) in [("Kind=A", 1000.0), ("Kind=B", 1200.0)] {
        let id = f.d.alloc_id();
        let mut fr = Node::frame(id, Rect::new(0.0, y, 300.0, 100.0));
        fr.name = name.into();
        let fr = f.d.insert(page, fr).unwrap();
        create_instance(&mut f.d, f.m[0], fr, Vec2::new(10.0, 10.0)).apply(&mut f.d).unwrap();
        let b = *f.d.get(fr).unwrap().children.last().unwrap();
        let mut bn = f.d.get(b).unwrap().clone();
        bn.name = "btn".into();
        Command::Update { node: Box::new(bn) }.apply(&mut f.d).unwrap();
        create_component(&mut f.d, fr).apply(&mut f.d).unwrap();
        mains.push(fr);
        buttons.push(b);
    }
    combine_as_variants(&mut f.d, &mains).apply(&mut f.d).unwrap();
    let set = f.d.get(mains[0]).unwrap().parent.unwrap();
    let mut sn = f.d.get(set).unwrap().clone();
    sn.name = "Card".into();
    Command::Update { node: Box::new(sn) }.apply(&mut f.d).unwrap();
    for b in buttons {
        let c = bind_prop_variable(&mut f.d, b, "Size", Some("size")).unwrap();
        f.h.exec(&mut f.d, c).unwrap();
    }
    create_instance(&mut f.d, mains[0], f.host, Vec2::new(10.0, 300.0)).apply(&mut f.d).unwrap();
    let top = *f.d.get(f.host).unwrap().children.last().unwrap();
    Cx { f, cards: [mains[0], mains[1]], top }
}

fn btn_of(f: &Px, inst: NodeId) -> NodeId {
    *f.d.get(inst).unwrap().children.iter().find(|c| f.d.get(**c).unwrap().name == "btn").expect("a btn child")
}

fn swap_to(f: &mut Px, inst: NodeId, to: NodeId) {
    let c = swap_instance(&mut f.d, inst, to);
    f.h.exec(&mut f.d, c).unwrap();
}

/// After each step the pick is still recorded on the button the swap rebuilt, and holds against both modes.
fn assert_pick_holds(f: &mut Px, card: NodeId) {
    let btn = btn_of(f, card);
    assert_eq!(picks(f, btn), ["Size"], "the rebuilt button still has its pick");
    assert_eq!(shown(f, btn), LARGE_DEFAULT);
    set_mode(f, "dark");
    set_mode(f, "light");
    let btn = btn_of(f, card);
    assert_eq!(picks(f, btn), ["Size"]);
    assert_eq!(shown(f, btn), LARGE_DEFAULT, "and a mode change leaves it on Large");
}

#[test]
fn a_pick_on_a_button_in_a_card_survives_the_card_switching_variant_there_and_back() {
    let mut x = card_fixture();
    let (top, btn) = (x.top, btn_of(&x.f, x.top));
    pick_nested(&mut x.f, btn, "Size", "Large");
    assert_eq!(picks(&x.f, btn), ["Size"]);
    for to in [x.cards[1], x.cards[0]] {
        swap_to(&mut x.f, top, to);
        assert_pick_holds(&mut x.f, top);
    }
}

#[test]
fn a_pick_on_a_button_in_a_card_survives_a_variable_driven_card_switch_there_and_back() {
    let mut x = card_fixture();
    let top = x.top;
    let c = bind_prop_variable(&mut x.f.d, top, "Kind", Some("kind")).unwrap();
    x.f.h.exec(&mut x.f.d, c).unwrap();
    let btn = btn_of(&x.f, top);
    pick_nested(&mut x.f, btn, "Size", "Large");
    set_mode(&mut x.f, "dark");
    assert_eq!(shown(&x.f, top), "Kind=B", "the variable moved the card");
    assert_eq!(picks(&x.f, btn_of(&x.f, top)), ["Size"], "the button in the new card variant keeps its pick");
    set_mode(&mut x.f, "light");
    assert_eq!(shown(&x.f, top), "Kind=A");
    assert_pick_holds(&mut x.f, top);
}

#[test]
fn a_pick_on_a_button_in_a_card_in_a_wrapper_survives_the_card_switching_variant_there_and_back() {
    let mut x = card_fixture();
    let page = x.f.d.get(x.f.host).unwrap().parent.unwrap();
    let id = x.f.d.alloc_id();
    let wrapper = x.f.d.insert(page, Node::frame(id, Rect::new(0.0, 1500.0, 400.0, 150.0))).unwrap();
    create_instance(&mut x.f.d, x.cards[0], wrapper, Vec2::new(10.0, 10.0)).apply(&mut x.f.d).unwrap();
    create_component(&mut x.f.d, wrapper).apply(&mut x.f.d).unwrap();
    create_instance(&mut x.f.d, wrapper, x.f.host, Vec2::new(10.0, 600.0)).apply(&mut x.f.d).unwrap();
    let w = *x.f.d.get(x.f.host).unwrap().children.last().unwrap();
    let card = *x.f.d.get(w).unwrap().children.first().unwrap();
    let btn = btn_of(&x.f, card);
    pick_nested(&mut x.f, btn, "Size", "Large");
    assert_eq!(picks(&x.f, btn), ["Size"]);
    for to in [x.cards[1], x.cards[0]] {
        swap_to(&mut x.f, card, to);
        assert_pick_holds(&mut x.f, card);
    }
}

#[test]
fn an_instance_swap_property_target_does_not_take_the_variant_of_a_binding_that_belongs_to_another_component() {
    let mut f = build(&["Size"], &[]);
    let page = f.d.get(f.host).unwrap().parent.unwrap();
    let mut badges = Vec::new();
    for (name, wd) in [("Size=Small", 60.0), ("Size=Large", 90.0)] {
        let id = f.d.alloc_id();
        let mut v = Node::frame(id, Rect::new(0.0, 2000.0, wd, 20.0));
        v.name = name.into();
        let v = f.d.insert(page, v).unwrap();
        create_component(&mut f.d, v).apply(&mut f.d).unwrap();
        badges.push(v);
    }
    combine_as_variants(&mut f.d, &badges).apply(&mut f.d).unwrap();
    let n_main = *f.d.get(f.wmain).unwrap().children.first().unwrap();
    let def = ComponentPropDef { name: "Icon".into(), kind: PropKind::InstanceSwap { preferred: vec![] }, default: PropValue::Instance(f.m[0]) };
    add_prop(&mut f.d, f.wmain, def).apply(&mut f.d).unwrap();
    bind_prop(&mut f.d, n_main, OverrideField::MainComponent, Some("Icon")).apply(&mut f.d).unwrap();
    let n = copy(&f);
    pick_nested(&mut f, n, "State", "Hover");
    assert!(picks(&f, n).is_empty(), "State is not bound: the shown variant is the pick");
    set_mode(&mut f, "dark");
    let c = set_instance_prop(&mut f.d, f.w, "Icon", Some(PropValue::Instance(badges[0])));
    f.h.exec(&mut f.d, c).unwrap();
    assert_eq!(shown(&f, copy(&f)), "Size=Small", "the property chose Badge Small; the Size variable belongs to Button");
}

#[test]
fn a_swap_of_an_instance_that_binds_only_a_text_property_stores_nothing_new() {
    let mut f = build(&[], &[]);
    let top = f.top;
    let set = f.d.get(f.m[0]).unwrap().parent.unwrap();
    add_prop(&mut f.d, set, ComponentPropDef { name: "Label".into(), kind: PropKind::Text, default: PropValue::Text("Buy".into()) }).apply(&mut f.d).unwrap();
    f.d.variables.insert("label".into(), string_variable("label", "Buy", "Pay"));
    f.d.variable_collections.get_mut("theme").unwrap().variables.push("label".into());
    let c = bind_prop_variable(&mut f.d, top, "Label", Some("label")).unwrap();
    f.h.exec(&mut f.d, c).unwrap();
    let before = f.d.get(top).unwrap().component.values.clone();
    assert!(before.contains_key("Label"), "the binding stores the text it resolved");
    let large = f.m[1];
    swap_to(&mut f, top, large);
    assert_eq!(shown(&f, top), LARGE_DEFAULT);
    assert_eq!(f.d.get(top).unwrap().component.values, before, "no variant value joins the stored values");
    assert!(picks(&f, top).is_empty());
}

#[test]
fn renaming_a_property_renames_its_pick() {
    let mut f = build(&[], &["Size"]);
    let top = f.top;
    let set = f.d.get(f.m[0]).unwrap().parent.unwrap();
    pick_top(&mut f, top, "Size", "Large");
    let c = rename_prop(&mut f.d, set, "Size", "Scale");
    f.h.exec(&mut f.d, c).unwrap();
    assert_eq!(picks(&f, top), ["Scale"], "the pick follows the property to its new name");
    set_mode(&mut f, "dark");
    set_mode(&mut f, "light");
    assert_eq!(f.d.get(top).unwrap().component.values.get("Scale"), Some(&PropValue::Variant("Large".into())), "and still holds against the variable");
}

#[test]
fn deleting_a_property_drops_its_pick_and_a_new_property_of_that_name_starts_free() {
    let mut f = build(&[], &["Size"]);
    let top = f.top;
    let set = f.d.get(f.m[0]).unwrap().parent.unwrap();
    pick_top(&mut f, top, "Size", "Large");
    let c = delete_prop(&mut f.d, set, "Size");
    f.h.exec(&mut f.d, c).unwrap();
    assert!(picks(&f, top).is_empty(), "no pick of a property that no longer exists");
    let def = ComponentPropDef {
        name: "Size".into(),
        kind: PropKind::Variant { options: vec!["Small".into(), "Large".into()] },
        default: PropValue::Variant("Small".into()),
    };
    add_prop(&mut f.d, set, def).apply(&mut f.d).unwrap();
    let c = bind_prop_variable(&mut f.d, top, "Size", Some("size")).unwrap();
    f.h.exec(&mut f.d, c).unwrap();
    set_mode(&mut f, "dark");
    assert_eq!(f.d.get(top).unwrap().component.values.get("Size"), Some(&PropValue::Variant("Large".into())), "the new property follows its variable");
}

/// A spec that pins a nested variant property: `Card` placed in the host with `btn` pinned to `Large`.
fn place_card_pinning_large(x: &mut Cx) -> NodeId {
    let spec: NodeSpec = serde_json::from_value(serde_json::json!({
        "kind": "instance",
        "component": "Card",
        "overrides": [{ "path": "btn", "variant": { "Size": "Large" } }]
    }))
    .unwrap();
    let (cmd, _) = apply_spec_command(&mut x.f.d, x.f.host, &spec, &[]).expect("spec applies");
    x.f.h.exec(&mut x.f.d, cmd).unwrap();
    *x.f.d.get(x.f.host).unwrap().children.last().unwrap()
}

#[test]
fn a_spec_pinning_a_bound_property_to_the_value_it_already_shows_records_the_pick() {
    let mut x = card_fixture();
    set_mode(&mut x.f, "dark");
    let card = place_card_pinning_large(&mut x);
    let btn = btn_of(&x.f, card);
    assert_eq!(shown(&x.f, btn), LARGE_DEFAULT, "the dark variable already gives Large");
    assert_eq!(picks(&x.f, btn), ["Size"], "pinning is a pick, whether or not the shown variant had to move");
    set_mode(&mut x.f, "light");
    assert_eq!(shown(&x.f, btn_of(&x.f, card)), LARGE_DEFAULT, "so the pin holds against light");
}

#[test]
fn picking_the_variant_a_bound_copy_already_shows_holds_it_against_the_next_mode_change() {
    let mut f = build(&["Size"], &[]);
    set_mode(&mut f, "dark");
    let n = copy(&f);
    assert_eq!(shown(&f, n), LARGE_DEFAULT, "the dark variable gives Large");
    pick_nested(&mut f, n, "Size", "Large");
    assert_eq!(picks(&f, n), ["Size"], "naming the variant it shows pins the bound property");
    assert!(!f.d.get(n).unwrap().component.values.contains_key("Size"));
    set_mode(&mut f, "light");
    assert_eq!(shown(&f, n), LARGE_DEFAULT, "so light does not move it");
    let before = f.d.get(n).unwrap().clone();
    pick_nested(&mut f, n, "Size", "Large");
    assert_eq!(*f.d.get(n).unwrap(), before, "pinning again changes nothing");
    pick_nested(&mut f, n, "State", "Default");
    assert_eq!(*f.d.get(n).unwrap(), before, "and naming what an unbound property shows records nothing");
}

#[test]
fn a_spec_pin_leaves_no_stale_stored_value_behind_a_reset() {
    let mut x = card_fixture();
    let card = place_card_pinning_large(&mut x);
    let btn = btn_of(&x.f, card);
    assert_eq!(shown(&x.f, btn), LARGE_DEFAULT);
    assert!(!x.f.d.get(btn).unwrap().component.values.contains_key("Size"), "the copy's variant is the main it shows, not a stored value");
    let c = reset_overrides(&mut x.f.d, btn, &[OverrideField::MainComponent]);
    x.f.h.exec(&mut x.f.d, c).unwrap();
    assert_eq!(shown(&x.f, btn), SMALL_DEFAULT, "reset gives the variable's variant (light)");
    let comp = instance_component(x.f.d.get(btn).unwrap()).unwrap();
    let def = prop_defs(&x.f.d, comp).into_iter().find(|d| d.name == "Size").unwrap();
    assert_eq!(prop_value(&x.f.d, btn, &def), PropValue::Variant("Small".into()), "the row value and the shown variant agree");
}

#[test]
fn detaching_a_nested_instance_clears_its_picks() {
    let mut f = build(&["Size"], &[]);
    let n = copy(&f);
    pick_nested(&mut f, n, "Size", "Large");
    assert_eq!(picks(&f, n), ["Size"]);
    let c = detach_instance(&mut f.d, n);
    f.h.exec(&mut f.d, c).unwrap();
    assert!(matches!(f.d.get(n).unwrap().kind, NodeKind::Frame(_)));
    assert!(picks(&f, n).is_empty());
    assert!(!f.d.to_json().unwrap().contains("variant_picks"), "a frame carries no pick record");
}

#[test]
fn undo_then_redo_of_a_pick_restores_the_variant_the_picks_and_the_stored_values() {
    let mut f = build(&["Size"], &["Size"]);
    let (n, top) = (copy(&f), f.top);
    for (inst, nested) in [(n, true), (top, false)] {
        let before = f.d.get(inst).unwrap().clone();
        if nested {
            pick_nested(&mut f, inst, "Size", "Large");
        } else {
            pick_top(&mut f, inst, "Size", "Large");
        }
        let after = f.d.get(inst).unwrap().clone();
        assert_eq!(picks(&f, inst), ["Size"]);
        f.h.undo(&mut f.d).unwrap();
        assert_eq!(*f.d.get(inst).unwrap(), before, "undo takes the pick, the variant and the values back");
        f.h.redo(&mut f.d).unwrap();
        assert_eq!(*f.d.get(inst).unwrap(), after, "redo puts all of them back");
    }
}
