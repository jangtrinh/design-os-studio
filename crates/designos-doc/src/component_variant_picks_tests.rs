//! A hand pick on a bound variant property holds that property only, on a top-level instance and on a nested copy alike.
//!
//! Fixture: a set `Button` with the properties `Size` (Small / Large) and `State` (Default / Hover), all four variants.
//! The collection `theme` has the string variables `size` (light Small, dark Large) and `state` (light Default, dark
//! Hover). A wrapper main holds one nested instance of the set; `W` is an instance of the wrapper inside `host`, and `top`
//! is an instance of the set that sits in `host` itself.

use crate::*;

pub(super) struct Px {
    pub(super) d: Document,
    pub(super) host: NodeId,
    pub(super) w: NodeId,
    pub(super) wmain: NodeId,
    pub(super) top: NodeId,
    /// Small/Default, Large/Default, Small/Hover, Large/Hover.
    pub(super) m: [NodeId; 4],
    pub(super) h: History,
}

pub(super) fn string_variable(id: &str, light: &str, dark: &str) -> Variable {
    let s = |v: &str| VariableValue::String(v.into());
    Variable {
        id: id.into(),
        name: id.into(),
        collection: "theme".into(),
        kind: VariableKind::String,
        values: [("light".to_string(), s(light)), ("dark".to_string(), s(dark))].into_iter().collect(),
        description: String::new(),
        scopes: vec![],
        hidden_from_publishing: false,
        key: None,
        code_syntax: Default::default(),
        extensions: Default::default(),
        mode_extensions: Default::default(),
    }
}

/// `nested`: the properties bound inside the wrapper main (so `W`'s copy inherits them); `top`: the properties bound on
/// the top-level instance. Each property `Size` / `State` is bound to the variable `size` / `state`.
pub(super) fn build(nested: &[&str], top: &[&str]) -> Px {
    let mut d = Document::new("t");
    let page = d.add_page("P");
    d.variable_collections.insert(
        "theme".into(),
        VariableCollection {
            id: "theme".into(),
            name: "Theme".into(),
            modes: vec![VariableMode { id: "light".into(), name: "Light".into() }, VariableMode { id: "dark".into(), name: "Dark".into() }],
            default_mode: "light".into(),
            variables: vec!["size".into(), "state".into()],
            key: None,
            source_library_key: None,
            extensions: Default::default(),
        },
    );
    d.variables.insert("size".into(), string_variable("size", "Small", "Large"));
    d.variables.insert("state".into(), string_variable("state", "Default", "Hover"));
    let mut mains = Vec::new();
    for (name, wd) in
        [("Size=Small, State=Default", 100.0), ("Size=Large, State=Default", 200.0), ("Size=Small, State=Hover", 110.0), ("Size=Large, State=Hover", 210.0)]
    {
        let id = d.alloc_id();
        let mut v = Node::frame(id, Rect::new(0.0, 0.0, wd, 40.0));
        v.name = name.into();
        let v = d.insert(page, v).unwrap();
        let id = d.alloc_id();
        let mut t = Node::text(id, Rect::new(10.0, 10.0, 60.0, 20.0), "Label", TextStyle::default());
        t.name = "label".into();
        d.insert(v, t).unwrap();
        create_component(&mut d, v).apply(&mut d).unwrap();
        mains.push(v);
    }
    combine_as_variants(&mut d, &mains).apply(&mut d).unwrap();
    let id = d.alloc_id();
    let wmain = d.insert(page, Node::frame(id, Rect::new(0.0, 300.0, 400.0, 100.0))).unwrap();
    create_instance(&mut d, mains[0], wmain, Vec2::new(10.0, 10.0)).apply(&mut d).unwrap();
    let n_main = *d.get(wmain).unwrap().children.last().unwrap();
    create_component(&mut d, wmain).apply(&mut d).unwrap();
    let mut h = History::default();
    for prop in nested {
        let c = bind_prop_variable(&mut d, n_main, prop, Some(&prop.to_lowercase())).expect("bind inside the wrapper main");
        h.exec(&mut d, c).unwrap();
    }
    let id = d.alloc_id();
    let host = d.insert(page, Node::frame(id, Rect::new(500.0, 0.0, 400.0, 400.0))).unwrap();
    create_instance(&mut d, wmain, host, Vec2::new(10.0, 10.0)).apply(&mut d).unwrap();
    let w = *d.get(host).unwrap().children.last().unwrap();
    create_instance(&mut d, mains[0], host, Vec2::new(10.0, 200.0)).apply(&mut d).unwrap();
    let top_id = *d.get(host).unwrap().children.last().unwrap();
    for prop in top {
        let c = bind_prop_variable(&mut d, top_id, prop, Some(&prop.to_lowercase())).expect("bind on the top-level instance");
        h.exec(&mut d, c).unwrap();
    }
    Px { d, host, w, wmain, top: top_id, m: [mains[0], mains[1], mains[2], mains[3]], h }
}

pub(super) fn copy(f: &Px) -> NodeId {
    *f.d.get(f.w).unwrap().children.first().unwrap()
}

/// The variant the instance shows, as its name.
pub(super) fn shown(f: &Px, inst: NodeId) -> String {
    f.d.get(instance_component(f.d.get(inst).unwrap()).unwrap()).unwrap().name.clone()
}

pub(super) fn set_mode(f: &mut Px, mode: &str) {
    let mut n = f.d.get(f.host).unwrap().clone();
    n.variables.modes.insert("theme".into(), mode.into());
    f.h.exec(&mut f.d, Command::Update { node: Box::new(n) }).unwrap();
}

pub(super) fn picks(f: &Px, inst: NodeId) -> Vec<String> {
    f.d.get(inst).unwrap().component.variant_picks.iter().cloned().collect()
}

pub(super) fn pick_nested(f: &mut Px, inst: NodeId, prop: &str, value: &str) {
    let c = set_nested_instance_prop(&mut f.d, inst, prop, Some(PropValue::Variant(value.into())));
    f.h.exec(&mut f.d, c).unwrap();
}

pub(super) fn pick_top(f: &mut Px, inst: NodeId, prop: &str, value: &str) {
    let c = set_instance_prop(&mut f.d, inst, prop, Some(PropValue::Variant(value.into())));
    f.h.exec(&mut f.d, c).unwrap();
}

pub(super) const SMALL_DEFAULT: &str = "Size=Small, State=Default";
pub(super) const LARGE_DEFAULT: &str = "Size=Large, State=Default";
pub(super) const SMALL_HOVER: &str = "Size=Small, State=Hover";
pub(super) const LARGE_HOVER: &str = "Size=Large, State=Hover";

#[test]
fn a_pick_on_state_leaves_a_bound_size_following_the_mode() {
    let mut f = build(&["Size"], &[]);
    let n = copy(&f);
    let before = f.d.get(n).unwrap().clone();
    pick_nested(&mut f, n, "State", "Hover");
    assert_eq!(shown(&f, n), SMALL_HOVER);
    assert!(picks(&f, n).is_empty(), "State is not bound, so the shown variant is the pick and nothing else is recorded");
    set_mode(&mut f, "dark");
    assert_eq!(shown(&f, n), LARGE_HOVER, "Size still follows its variable in dark, State stays picked");
    set_mode(&mut f, "light");
    assert_eq!(shown(&f, n), SMALL_HOVER, "and back to Small in light");
    f.h.undo(&mut f.d).unwrap();
    f.h.undo(&mut f.d).unwrap();
    f.h.undo(&mut f.d).unwrap();
    assert_eq!(*f.d.get(n).unwrap(), before, "undoing the pick and both mode changes restores the copy exactly");
}

#[test]
fn a_pick_on_the_bound_property_holds_only_that_property() {
    let mut f = build(&["Size", "State"], &[]);
    let n = copy(&f);
    let before = f.d.get(n).unwrap().clone();
    assert_eq!(shown(&f, n), SMALL_DEFAULT);
    pick_nested(&mut f, n, "Size", "Large");
    assert_eq!(shown(&f, n), LARGE_DEFAULT);
    assert_eq!(picks(&f, n), ["Size"], "only the property that was picked is recorded");
    set_mode(&mut f, "dark");
    assert_eq!(shown(&f, n), LARGE_HOVER, "Size holds, State follows its variable");
    set_mode(&mut f, "light");
    assert_eq!(shown(&f, n), LARGE_DEFAULT, "Size still holds in light, State follows back");
    f.h.undo(&mut f.d).unwrap();
    f.h.undo(&mut f.d).unwrap();
    f.h.undo(&mut f.d).unwrap();
    assert_eq!(*f.d.get(n).unwrap(), before, "one undo per step takes the pick back with its variant");
}

#[test]
fn reset_gives_every_picked_property_back_to_its_variable() {
    let mut f = build(&["Size", "State"], &[]);
    let n = copy(&f);
    pick_nested(&mut f, n, "Size", "Large");
    set_mode(&mut f, "dark");
    set_mode(&mut f, "light");
    assert_eq!(shown(&f, n), LARGE_DEFAULT);
    let c = reset_overrides(&mut f.d, n, &[OverrideField::MainComponent]);
    f.h.exec(&mut f.d, c).unwrap();
    assert!(picks(&f, n).is_empty(), "reset empties the picks");
    assert_eq!(shown(&f, n), SMALL_DEFAULT, "light: both properties give their variable's value at once");
    set_mode(&mut f, "dark");
    assert_eq!(shown(&f, n), LARGE_HOVER, "and both follow the mode again");
}

#[test]
fn a_top_level_swap_to_a_sibling_variant_holds_the_properties_it_changed() {
    let mut f = build(&[], &["Size", "State"]);
    let top = f.top;
    let before = f.d.get(top).unwrap().clone();
    assert_eq!(shown(&f, top), SMALL_DEFAULT);
    let c = swap_instance(&mut f.d, top, f.m[1]);
    f.h.exec(&mut f.d, c).unwrap();
    assert_eq!(shown(&f, top), LARGE_DEFAULT);
    assert_eq!(f.d.get(top).unwrap().component.values.get("Size"), Some(&PropValue::Variant("Large".into())), "the swap stores the property it changed");
    assert_eq!(picks(&f, top), ["Size"], "only the bound property that differs is recorded");
    set_mode(&mut f, "dark");
    assert_eq!(shown(&f, top), LARGE_HOVER, "State follows, Size keeps the swap");
    set_mode(&mut f, "light");
    assert_eq!(shown(&f, top), LARGE_DEFAULT);
    f.h.undo(&mut f.d).unwrap();
    f.h.undo(&mut f.d).unwrap();
    f.h.undo(&mut f.d).unwrap();
    assert_eq!(*f.d.get(top).unwrap(), before, "the swap, its picks and its stored values undo together");
}

#[test]
fn a_swap_to_another_set_holds_every_bound_property_the_new_main_has() {
    let mut f = build(&[], &["Size"]);
    let top = f.top;
    let page = f.d.get(f.host).unwrap().parent.unwrap();
    let mut other = Vec::new();
    for (name, wd) in [("Size=Small", 60.0), ("Size=Large", 90.0)] {
        let id = f.d.alloc_id();
        let mut v = Node::frame(id, Rect::new(0.0, 900.0, wd, 20.0));
        v.name = name.into();
        let v = f.d.insert(page, v).unwrap();
        create_component(&mut f.d, v).apply(&mut f.d).unwrap();
        other.push(v);
    }
    combine_as_variants(&mut f.d, &other).apply(&mut f.d).unwrap();
    let c = swap_instance(&mut f.d, top, other[0]);
    f.h.exec(&mut f.d, c).unwrap();
    assert_eq!(f.d.get(instance_component(f.d.get(top).unwrap()).unwrap()).unwrap().name, "Size=Small");
    assert_eq!(picks(&f, top), ["Size"], "the other set has a Size too: it is held although the value is the same");
    set_mode(&mut f, "dark");
    assert_eq!(f.d.get(instance_component(f.d.get(top).unwrap()).unwrap()).unwrap().name, "Size=Small", "and a mode change does not move it");
}

#[test]
fn a_top_level_pick_on_a_bound_property_holds_until_reset() {
    let mut f = build(&[], &["Size"]);
    let top = f.top;
    let before = f.d.get(top).unwrap().clone();
    pick_top(&mut f, top, "Size", "Large");
    assert_eq!(shown(&f, top), LARGE_DEFAULT);
    assert_eq!(picks(&f, top), ["Size"]);
    set_mode(&mut f, "dark");
    set_mode(&mut f, "light");
    assert_eq!(shown(&f, top), LARGE_DEFAULT, "a mode change does not take the pick back");
    let c = reset_instance(&mut f.d, top);
    f.h.exec(&mut f.d, c).unwrap();
    assert_eq!(shown(&f, top), SMALL_DEFAULT, "reset gives the variable's value at once, with no mode change");
    assert!(picks(&f, top).is_empty());
    set_mode(&mut f, "dark");
    assert_eq!(shown(&f, top), LARGE_DEFAULT, "and it follows the mode again");
    for _ in 0..5 {
        f.h.undo(&mut f.d).unwrap();
    }
    assert_eq!(*f.d.get(top).unwrap(), before, "undo restores the variant, the picks and the stored values step by step");
}

#[test]
fn binding_a_picked_property_makes_it_follow_again() {
    let mut f = build(&["Size"], &["Size"]);
    let (n, top) = (copy(&f), f.top);
    pick_nested(&mut f, n, "Size", "Large");
    pick_top(&mut f, top, "Size", "Large");
    assert_eq!((picks(&f, n), picks(&f, top)), (vec!["Size".to_string()], vec!["Size".to_string()]));
    for inst in [n, top] {
        let c = bind_prop_variable(&mut f.d, inst, "Size", Some("size")).unwrap();
        f.h.exec(&mut f.d, c).unwrap();
        assert!(picks(&f, inst).is_empty(), "binding means follow from now on");
        assert_eq!(shown(&f, inst), SMALL_DEFAULT, "light: the variable's value shows at once");
    }
    set_mode(&mut f, "dark");
    assert_eq!((shown(&f, n), shown(&f, top)), (LARGE_DEFAULT.to_string(), LARGE_DEFAULT.to_string()));
}

#[test]
fn unbinding_a_picked_property_drops_its_pick() {
    let mut f = build(&[], &["Size"]);
    let top = f.top;
    pick_top(&mut f, top, "Size", "Large");
    let c = bind_prop_variable(&mut f.d, top, "Size", None).unwrap();
    f.h.exec(&mut f.d, c).unwrap();
    assert!(picks(&f, top).is_empty(), "an unbound property is never in the set");
    assert_eq!(shown(&f, top), LARGE_DEFAULT, "and keeps the variant it showed");
}

#[test]
fn variant_picks_survive_a_native_save_and_load() {
    let mut f = build(&["Size", "State"], &["Size"]);
    let (n, top) = (copy(&f), f.top);
    pick_nested(&mut f, n, "Size", "Large");
    pick_top(&mut f, top, "Size", "Large");
    let json = f.d.to_json().unwrap();
    let mut d2 = Document::from_json(&json).unwrap();
    assert_eq!(d2.get(n).unwrap().component.variant_picks, f.d.get(n).unwrap().component.variant_picks);
    assert_eq!(d2.get(top).unwrap().component.variant_picks, f.d.get(top).unwrap().component.variant_picks);
    assert!(!d2.get(n).unwrap().component.variant_picks.is_empty() && !d2.get(top).unwrap().component.variant_picks.is_empty());
    let mut h = History::default();
    let mut hn = d2.get(f.host).unwrap().clone();
    hn.variables.modes.insert("theme".into(), "dark".into());
    h.exec(&mut d2, Command::Update { node: Box::new(hn) }).unwrap();
    let name = |d: &Document, id: NodeId| d.get(instance_component(d.get(id).unwrap()).unwrap()).unwrap().name.clone();
    assert_eq!(name(&d2, n), LARGE_HOVER, "after the reload the copy still holds Size and follows State");
    assert_eq!(name(&d2, top), LARGE_DEFAULT, "and the top-level instance holds its pick");
}

#[test]
fn a_pick_on_an_unbound_property_records_no_variant_picks() {
    // nothing bound at all
    let mut f = build(&[], &[]);
    let (n, top) = (copy(&f), f.top);
    pick_nested(&mut f, n, "Size", "Large");
    pick_top(&mut f, top, "State", "Hover");
    let c = swap_instance(&mut f.d, n, f.m[3]);
    f.h.exec(&mut f.d, c).unwrap();
    let c = swap_instance(&mut f.d, top, f.m[1]);
    f.h.exec(&mut f.d, c).unwrap();
    assert!(picks(&f, n).is_empty() && picks(&f, top).is_empty());
    // one property bound, the pick goes to the other
    let mut f = build(&["Size"], &["Size"]);
    let (n, top) = (copy(&f), f.top);
    pick_nested(&mut f, n, "State", "Hover");
    pick_top(&mut f, top, "State", "Hover");
    assert!(picks(&f, n).is_empty() && picks(&f, top).is_empty(), "State is unbound on both");
    let json = f.d.to_json().unwrap();
    assert!(!json.contains("variant_picks"), "no key reaches the native file of an instance that holds no pick");
}

#[test]
fn a_swap_of_an_instance_with_no_bound_property_stores_nothing_new() {
    let mut f = build(&[], &[]);
    let top = f.top;
    let c = swap_instance(&mut f.d, top, f.m[1]);
    f.h.exec(&mut f.d, c).unwrap();
    assert!(f.d.get(top).unwrap().component.values.is_empty(), "an unbound instance keeps the stored values it had");
}

#[test]
fn a_new_instance_of_a_main_starts_its_copies_without_picks() {
    let mut f = build(&["Size"], &[]);
    let n_main = *f.d.get(f.wmain).unwrap().children.first().unwrap();
    let mut n = f.d.get(n_main).unwrap().clone();
    n.component.variant_picks.insert("Size".into());
    Command::Update { node: Box::new(n) }.apply(&mut f.d).unwrap();
    create_instance(&mut f.d, f.wmain, f.host, Vec2::new(10.0, 300.0)).apply(&mut f.d).unwrap();
    let w2 = *f.d.get(f.host).unwrap().children.last().unwrap();
    let c2 = *f.d.get(w2).unwrap().children.first().unwrap();
    assert!(picks(&f, c2).is_empty(), "a copy made from a main starts without a pick of its own");
}

/// An instance-swap property, not a variable, moves a copy that holds a hand swap: the switch drops the swap's record, as
/// it always did. Only a switch a variable binding makes keeps the record of a pick.
#[test]
fn an_instance_swap_property_replaces_a_hand_swap_and_drops_its_record() {
    let mut d = Document::new("t");
    let page = d.add_page("P");
    let mut comps = Vec::new();
    for (name, y) in [("A", 0.0), ("B", 100.0), ("C", 200.0)] {
        let id = d.alloc_id();
        let mut v = Node::frame(id, Rect::new(0.0, y, 20.0, 20.0));
        v.name = name.into();
        let v = d.insert(page, v).unwrap();
        create_component(&mut d, v).apply(&mut d).unwrap();
        comps.push(v);
    }
    let id = d.alloc_id();
    let card = d.insert(page, Node::frame(id, Rect::new(0.0, 400.0, 100.0, 50.0))).unwrap();
    create_instance(&mut d, comps[0], card, Vec2::ZERO).apply(&mut d).unwrap();
    let slot = *d.get(card).unwrap().children.last().unwrap();
    create_component(&mut d, card).apply(&mut d).unwrap();
    let def = ComponentPropDef { name: "Icon".into(), kind: PropKind::InstanceSwap { preferred: vec![] }, default: PropValue::Instance(comps[0]) };
    add_prop(&mut d, card, def).apply(&mut d).unwrap();
    bind_prop(&mut d, slot, OverrideField::MainComponent, Some("Icon")).apply(&mut d).unwrap();
    create_instance(&mut d, card, page, Vec2::new(300.0, 0.0)).apply(&mut d).unwrap();
    let outer = *d.get(page).unwrap().children.last().unwrap();
    let copy = *d.get(outer).unwrap().children.first().unwrap();
    swap_instance(&mut d, copy, comps[2]).apply(&mut d).unwrap();
    assert!(d.get(copy).unwrap().component.overrides.contains(&OverrideField::MainComponent), "the hand swap is recorded");
    set_instance_prop(&mut d, outer, "Icon", Some(PropValue::Instance(comps[1]))).apply(&mut d).unwrap();
    assert_eq!(instance_component(d.get(copy).unwrap()), Some(comps[1]), "the property moved the copy");
    assert!(!d.get(copy).unwrap().component.overrides.contains(&OverrideField::MainComponent), "and the record of the hand swap went with it");
}

// --- files written before the picks were recorded -------------------------------------------------------------

/// The state an import leaves for a copy the file shows on `shown` while a binding is on it: a recorded main swap with no
/// picks. (`settle_bound_props` is the import's last step.)
fn imported_copy(bind: &[&str], shown: &str, dark: bool) -> (Px, NodeId) {
    let mut f = build(bind, &[]);
    let n = copy(&f);
    if dark {
        let mut host = f.d.get(f.host).unwrap().clone();
        host.variables.modes.insert("theme".into(), "dark".into());
        Command::Update { node: Box::new(host) }.apply(&mut f.d).unwrap();
    }
    let target = f.m[[SMALL_DEFAULT, LARGE_DEFAULT, SMALL_HOVER, LARGE_HOVER].iter().position(|s| *s == shown).unwrap()];
    let c = swap_instance(&mut f.d, n, target);
    c.apply(&mut f.d).unwrap();
    let mut node = f.d.get(n).unwrap().clone();
    node.component.variant_picks.clear();
    Command::Update { node: Box::new(node) }.apply(&mut f.d).unwrap();
    (f, n)
}

#[test]
fn a_file_without_picks_whose_copy_shows_the_variables_variant_goes_on_following() {
    let (mut f, n) = imported_copy(&["Size", "State"], LARGE_HOVER, true);
    assert!(f.d.get(n).unwrap().component.overrides.contains(&OverrideField::MainComponent), "the import recorded the swap");
    settle_bound_props(&mut f.d);
    assert!(f.d.get(n).unwrap().component.variant_picks.is_empty());
    assert!(!f.d.get(n).unwrap().component.overrides.contains(&OverrideField::MainComponent), "the swap was the binding's doing: released");
    assert_eq!(shown(&f, n), LARGE_HOVER);
    set_mode(&mut f, "light");
    assert_eq!(shown(&f, n), SMALL_DEFAULT, "and the copy goes on following the mode");
}

#[test]
fn a_file_without_picks_holds_only_the_bound_properties_whose_shown_value_differs_from_the_variable() {
    // light: size gives Small, state gives Default. The file shows Size=Large (differs) and State=Default (the same).
    let (mut f, n) = imported_copy(&["Size", "State"], LARGE_DEFAULT, false);
    settle_bound_props(&mut f.d);
    assert_eq!(picks(&f, n), ["Size"], "Size differs from its variable: a pick; State shows its variable's value: it keeps following");
    assert_eq!(shown(&f, n), LARGE_DEFAULT, "opening does not change what the copy shows");
    set_mode(&mut f, "dark");
    assert_eq!(shown(&f, n), LARGE_HOVER);
}

#[test]
fn a_file_without_picks_holds_a_swap_to_another_set_that_has_the_bound_property() {
    let mut f = build(&["Size"], &[]);
    let n = copy(&f);
    let page = f.d.get(f.host).unwrap().parent.unwrap();
    let mut other = Vec::new();
    for (name, wd) in [("Size=Small", 60.0), ("Size=Large", 90.0)] {
        let id = f.d.alloc_id();
        let mut v = Node::frame(id, Rect::new(0.0, 900.0, wd, 20.0));
        v.name = name.into();
        let v = f.d.insert(page, v).unwrap();
        create_component(&mut f.d, v).apply(&mut f.d).unwrap();
        other.push(v);
    }
    combine_as_variants(&mut f.d, &other).apply(&mut f.d).unwrap();
    swap_instance(&mut f.d, n, other[0]).apply(&mut f.d).unwrap();
    let mut node = f.d.get(n).unwrap().clone();
    node.component.variant_picks.clear();
    Command::Update { node: Box::new(node) }.apply(&mut f.d).unwrap();
    settle_bound_props(&mut f.d);
    assert_eq!(picks(&f, n), ["Size"], "the value is the variable's, but the copy is on another set than its main: that is a swap");
    set_mode(&mut f, "dark");
    assert_eq!(f.d.get(instance_component(f.d.get(n).unwrap()).unwrap()).unwrap().name, "Size=Small", "and a mode change leaves it there");
}

#[test]
fn a_file_without_picks_keeps_an_unbound_property_pick_as_the_main_swap_it_stored() {
    let (mut f, n) = imported_copy(&["Size"], SMALL_HOVER, false);
    settle_bound_props(&mut f.d);
    assert!(picks(&f, n).is_empty(), "State is not bound and Size shows its variable's value");
    assert!(f.d.get(n).unwrap().component.overrides.contains(&OverrideField::MainComponent), "the State pick stays recorded as the swap");
    set_mode(&mut f, "dark");
    assert_eq!(shown(&f, n), LARGE_HOVER, "Size follows, State stays picked");
}

#[test]
fn a_native_file_from_before_the_picks_opens_with_the_variants_it_stored() {
    let mut f = build(&["Size", "State"], &["Size"]);
    let (n, top) = (copy(&f), f.top);
    pick_nested(&mut f, n, "Size", "Large");
    pick_top(&mut f, top, "Size", "Large");
    // the file an older build writes: the swap and the stored value are there, no picks
    let mut doc = f.d.clone();
    for id in [n, top] {
        let mut node = doc.get(id).unwrap().clone();
        node.component.variant_picks.clear();
        Command::Update { node: Box::new(node) }.apply(&mut doc).unwrap();
    }
    let json = doc.to_json().unwrap();
    assert!(!json.contains("variant_picks"));
    let d2 = Document::from_json(&json).unwrap();
    let name = |d: &Document, id: NodeId| d.get(instance_component(d.get(id).unwrap()).unwrap()).unwrap().name.clone();
    assert_eq!(name(&d2, n), name(&f.d, n), "the copy opens on the variant it stored");
    assert_eq!(name(&d2, top), name(&f.d, top), "and so does the top-level instance");
}

/// Known limit, pinned so it is read as a decision: a native file has no import step, so a pick an older build held on a
/// bound property (recorded then only as the swap or the stored value) is not turned into a pick on open. It shows what
/// it stored, and the property follows its variable from the first mode change on.
#[test]
fn a_pick_an_older_build_held_on_a_bound_property_follows_its_variable_after_the_first_mode_change() {
    let mut f = build(&["Size"], &["Size"]);
    let (n, top) = (copy(&f), f.top);
    pick_nested(&mut f, n, "Size", "Large");
    pick_top(&mut f, top, "Size", "Large");
    let mut doc = f.d.clone();
    for id in [n, top] {
        let mut node = doc.get(id).unwrap().clone();
        node.component.variant_picks.clear();
        Command::Update { node: Box::new(node) }.apply(&mut doc).unwrap();
    }
    let mut d2 = Document::from_json(&doc.to_json().unwrap()).unwrap();
    let name = |d: &Document, id: NodeId| d.get(instance_component(d.get(id).unwrap()).unwrap()).unwrap().name.clone();
    assert_eq!((name(&d2, n), name(&d2, top)), (LARGE_DEFAULT.to_string(), LARGE_DEFAULT.to_string()), "it opens as it was stored");
    let mut h = History::default();
    let mut host = d2.get(f.host).unwrap().clone();
    host.variables.modes.insert("theme".into(), "dark".into());
    h.exec(&mut d2, Command::Update { node: Box::new(host.clone()) }).unwrap();
    host.variables.modes.insert("theme".into(), "light".into());
    h.exec(&mut d2, Command::Update { node: Box::new(host) }).unwrap();
    assert_eq!((name(&d2, n), name(&d2, top)), (SMALL_DEFAULT.to_string(), SMALL_DEFAULT.to_string()));
}
