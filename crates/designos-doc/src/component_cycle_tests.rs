//! Documents in which a main holds an instance of itself, through any number of mains.
//!
//! Studio refuses to make one, but a file from an older build or from an agent can carry one, and the walks over
//! component links must stop at the repeat instead of overflowing the stack. Crash tests run alone with `--exact`:
//! a stack overflow aborts the whole test binary.

use crate::spec::OverrideSpec;
use crate::spec_instance::apply_overrides;
use crate::{
    add_prop, bind_prop, combine_as_variants, create_component, create_instance, instance_component, mains_held_by, reparent_would_hold_itself,
    set_instance_prop, swap_instance, sync_instances, Color, Command, ComponentPropDef, Document, Node, NodeId, NodeKind, OverrideField, Paint, PropKind,
    PropValue, Rect, Vec2,
};
use serde_json::json;

/// A main on `parent` with one layer inside it.
fn main_on(d: &mut Document, parent: NodeId, name: &str) -> NodeId {
    let id = d.alloc_id();
    let main = d.insert(parent, Node::frame(id, Rect::new(0.0, 0.0, 100.0, 100.0))).unwrap();
    d.get_mut(main).unwrap().name = name.into();
    let id = d.alloc_id();
    let layer = d.insert(main, Node::frame(id, Rect::new(10.0, 10.0, 30.0, 30.0))).unwrap();
    d.get_mut(layer).unwrap().name = "Layer".into();
    create_component(d, main).apply(d).unwrap();
    main
}

/// An instance of `comp` under `parent`, placed the way the app does.
fn place(d: &mut Document, comp: NodeId, parent: NodeId) -> NodeId {
    let cmd = create_instance(d, comp, parent, Vec2::new(5.0, 5.0));
    assert!(matches!(cmd, Command::Insert { .. }), "instance of {comp:?} in {parent:?} was refused");
    cmd.apply(d).unwrap();
    *d.get(parent).unwrap().children.last().unwrap()
}

fn is_nothing(c: &Command) -> bool {
    matches!(c, Command::Batch(v) if v.is_empty())
}

fn layer_of(d: &Document, main: NodeId) -> NodeId {
    *d.get(main).unwrap().children.iter().find(|c| d.get(**c).unwrap().name == "Layer").unwrap()
}

fn self_holding(d: &Document) -> Vec<NodeId> {
    d.nodes.values().filter(|n| matches!(n.kind, NodeKind::Component(_))).map(|n| n.id).filter(|m| mains_held_by(d, &[*m]).contains(m)).collect()
}

fn max_depth(d: &Document) -> usize {
    let mut deepest = 0;
    for page in d.pages.clone() {
        d.walk(page, |_, depth| deepest = deepest.max(depth));
    }
    deepest
}

/// Mains A and B on one page; A holds an instance of B, and B an instance of A. The second link bypasses the guard:
/// the instance of A is made on the page and then reparented into B, as a file from an older build or an agent can.
/// Returns (document, A, B).
fn cyclic_pair() -> (Document, NodeId, NodeId) {
    let mut d = Document::new("cycle");
    let page = d.add_page("P");
    let b = main_on(&mut d, page, "B");
    let a = main_on(&mut d, page, "A");
    place(&mut d, b, a);
    let inst_of_a = place(&mut d, a, page);
    Command::Reparent { id: inst_of_a, parent: b, index: 0 }.apply(&mut d).unwrap();
    assert_eq!(self_holding(&d).len(), 2, "the fixture holds itself");
    (d, a, b)
}

fn fill(d: &mut Document, id: NodeId, c: Color) {
    let mut n = d.get(id).unwrap().clone();
    n.fills = vec![Paint::solid(c)];
    Command::Update { node: Box::new(n) }.apply(d).unwrap();
}

// ---------- the primitives refuse to make a main hold itself

/// Set "State=One|Two" with an instance I of One in main B; Two holds an instance of B. Returns (document, B, Two, I).
fn variant_cycle() -> (Document, NodeId, NodeId, NodeId) {
    let mut d = Document::new("variants");
    let page = d.add_page("P");
    let b = main_on(&mut d, page, "B");
    let one = main_on(&mut d, page, "State=One");
    let two = main_on(&mut d, page, "State=Two");
    combine_as_variants(&mut d, &[one, two]).apply(&mut d).unwrap();
    let i = place(&mut d, one, b);
    place(&mut d, b, two);
    assert!(self_holding(&d).is_empty());
    (d, b, two, i)
}

#[test]
fn a_variant_switch_that_would_make_the_main_hold_itself_changes_nothing() {
    let (mut d, b, two, i) = variant_cycle();
    let before = d.get(i).unwrap().clone();
    let cmd = set_instance_prop(&mut d, i, "State", Some(PropValue::Variant("Two".into())));
    assert!(is_nothing(&cmd), "the switch is refused as a whole");
    cmd.apply(&mut d).unwrap();
    let after = d.get(i).unwrap();
    assert_eq!(after.component.values, before.component.values, "the stored value stays as it was");
    assert_ne!(instance_component(after), Some(two));
    assert_eq!(instance_component(after), instance_component(&before));
    assert!(self_holding(&d).is_empty());
    sync_instances(&mut d, b).apply(&mut d).unwrap();
}

#[test]
fn a_variant_switch_that_holds_nothing_still_switches() {
    let (mut d, _b, two, i) = variant_cycle();
    // the same switch on an instance outside B is fine: nothing holds itself there
    let page = d.pages[0];
    let one = instance_component(d.get(i).unwrap()).unwrap();
    let outside = place(&mut d, one, page);
    let cmd = set_instance_prop(&mut d, outside, "State", Some(PropValue::Variant("Two".into())));
    assert!(!is_nothing(&cmd));
    cmd.apply(&mut d).unwrap();
    assert_eq!(instance_component(d.get(outside).unwrap()), Some(two));
    assert!(self_holding(&d).is_empty());
}

/// Main Card holds an instance of Z bound to the instance-swap property "Icon". Returns (Card, Z).
fn card_with_icon_prop(d: &mut Document) -> (NodeId, NodeId) {
    let page = d.pages[0];
    let z = main_on(d, page, "Z");
    let card = main_on(d, page, "Card");
    let slot = place(d, z, card);
    let def = ComponentPropDef { name: "Icon".into(), kind: PropKind::InstanceSwap { preferred: vec![] }, default: PropValue::Instance(z) };
    add_prop(d, card, def).apply(d).unwrap();
    bind_prop(d, slot, OverrideField::MainComponent, Some("Icon")).apply(d).unwrap();
    (card, z)
}

#[test]
fn an_instance_swap_property_set_to_a_main_that_holds_the_main_it_sits_in_changes_nothing() {
    let mut d = Document::new("swap prop");
    let page = d.add_page("P");
    let b = main_on(&mut d, page, "B");
    let a = main_on(&mut d, page, "A");
    place(&mut d, b, a); // A holds B
    let (card, z) = card_with_icon_prop(&mut d);
    let k = place(&mut d, card, b); // a Card instance inside B
    let nested = *d.get(k).unwrap().children.iter().find(|c| matches!(d.get(**c).unwrap().kind, NodeKind::Instance { .. })).unwrap();
    assert_eq!(instance_component(d.get(nested).unwrap()), Some(z));
    let before = d.get(k).unwrap().component.values.clone();

    let cmd = set_instance_prop(&mut d, k, "Icon", Some(PropValue::Instance(a)));
    assert!(is_nothing(&cmd), "an Icon of A in B would make B hold itself");
    cmd.apply(&mut d).unwrap();
    assert_eq!(d.get(k).unwrap().component.values, before, "the stored value stays as it was");
    assert_eq!(instance_component(d.get(nested).unwrap()), Some(z), "the icon stays Z");
    assert!(self_holding(&d).is_empty());
    sync_instances(&mut d, b).apply(&mut d).unwrap();

    // outside B the same property takes the same value
    let outside = place(&mut d, card, page);
    let cmd = set_instance_prop(&mut d, outside, "Icon", Some(PropValue::Instance(a)));
    assert!(!is_nothing(&cmd));
    cmd.apply(&mut d).unwrap();
    assert!(self_holding(&d).is_empty());
}

#[test]
fn a_manual_swap_still_refuses_a_main_that_holds_the_main_it_sits_in() {
    let (mut d, _a, b) = cyclic_pair();
    let i = *d.get(b).unwrap().children.last().unwrap();
    assert!(is_nothing(&swap_instance(&mut d, i, b)), "no instance of B inside B");
}

#[test]
fn apply_spec_swap_refuses_a_held_main_without_mutating() {
    let mut d = Document::new("spec cycle");
    let page = d.add_page("P");
    let b = main_on(&mut d, page, "B");
    let a = main_on(&mut d, page, "A");
    place(&mut d, b, a);
    let c = main_on(&mut d, page, "C");
    place(&mut d, b, c);
    let root = place(&mut d, c, page);
    crate::Command::Reparent { id: root, parent: b, index: d.get(b).unwrap().children.len() }.apply(&mut d).unwrap();
    let before = d.to_json().unwrap();
    let ov: OverrideSpec = serde_json::from_value(json!({"path":"B", "swap":"A"})).unwrap();
    let err = apply_overrides(&mut d, root, &[ov], &[]).unwrap_err();
    assert!(err.to_string().contains("INSTANCE_CYCLE"), "{err}");
    assert_eq!(d.to_json().unwrap(), before);
}

#[test]
fn apply_spec_create_refuses_a_held_main_without_mutating() {
    let mut d = Document::new("spec create cycle");
    let page = d.add_page("P");
    let b = main_on(&mut d, page, "B");
    let a = main_on(&mut d, page, "A");
    place(&mut d, b, a);
    let parent = layer_of(&d, b);
    let before = d.to_json().unwrap();
    let spec: crate::NodeSpec = serde_json::from_value(json!({"kind":"instance", "component":"A"})).unwrap();
    let err = d.apply_spec(parent, &spec).unwrap_err();
    assert!(err.to_string().contains("INSTANCE_CYCLE"), "{err}");
    assert_eq!(d.to_json().unwrap(), before);
}

// ---------- the shared move check

#[test]
fn moving_a_layer_under_a_main_it_holds_an_instance_of_is_flagged_and_other_moves_are_not() {
    let mut d = Document::new("moves");
    let page = d.add_page("P");
    let b = main_on(&mut d, page, "B");
    let a = main_on(&mut d, page, "A");
    let in_a = place(&mut d, b, a); // A holds B
    let outside = place(&mut d, a, page); // an instance of A
    let plain = {
        let id = d.alloc_id();
        d.insert(page, Node::frame(id, Rect::new(500.0, 0.0, 10.0, 10.0))).unwrap()
    };
    // an instance of A holds B (through A): not into B, nor into a layer of B
    assert!(reparent_would_hold_itself(&d, &[outside], b));
    assert!(reparent_would_hold_itself(&d, &[outside], layer_of(&d, b)));
    // main A holds B: not into B
    assert!(reparent_would_hold_itself(&d, &[a], b));
    // an instance of B moves anywhere that is not inside B
    assert!(reparent_would_hold_itself(&d, &[in_a], b));
    assert!(!reparent_would_hold_itself(&d, &[in_a], page));
    assert!(!reparent_would_hold_itself(&d, &[outside], page));
    assert!(!reparent_would_hold_itself(&d, &[plain], b), "a plain frame holds nothing");
    assert!(!reparent_would_hold_itself(&d, &[in_a], a), "an instance of B stays fine in A, which holds it already");
}

// ---------- a document that already holds itself must not overflow

/// The walks stop at the repeat: syncing the two mains over and over leaves the same document after a few rounds.
#[test]
fn syncing_a_cyclic_pair_settles_instead_of_growing_without_end() {
    let (mut d, a, b) = cyclic_pair();
    let mut counts = Vec::new();
    for round in 0..40 {
        let owner = if round % 2 == 0 { b } else { a };
        sync_instances(&mut d, owner).apply(&mut d).unwrap();
        counts.push(d.nodes.len());
    }
    assert_eq!(counts[10], counts[39], "the tree stops growing: {counts:?}");
    assert!(max_depth(&d) < 24, "the nesting stops at the repeat, depth {}", max_depth(&d));
    assert!(is_nothing(&sync_instances(&mut d, a)) && is_nothing(&sync_instances(&mut d, b)), "and a settled document needs no sync");
}

#[test]
fn a_main_of_a_cyclic_pair_still_reaches_its_instances_outside_the_repeat() {
    let (mut d, a, b) = cyclic_pair();
    let page = d.pages[0];
    let on_page = place(&mut d, b, page);
    sync_instances(&mut d, b).apply(&mut d).unwrap();
    let red = Color::rgb(200, 10, 10);
    let layer = layer_of(&d, b);
    fill(&mut d, layer, red);
    sync_instances(&mut d, b).apply(&mut d).unwrap();
    sync_instances(&mut d, a).apply(&mut d).unwrap();
    let copy = *d.get(on_page).unwrap().children.iter().find(|c| d.get(**c).unwrap().name == "Layer").unwrap();
    assert_eq!(d.get(copy).unwrap().fills, d.get(layer_of(&d, b)).unwrap().fills, "an instance of B on the page follows B");
}

fn child_named(d: &Document, parent: NodeId, name: &str) -> NodeId {
    *d.get(parent).unwrap().children.iter().find(|c| d.get(**c).unwrap().name == name).unwrap_or_else(|| panic!("no child {name} in {parent:?}"))
}

/// A repeat is left as stored: B in A holds A, which holds B again. That innermost B is the repeat, and a sync
/// of the real B reaches everything up to it but not the layers inside it.
#[test]
fn a_main_met_again_inside_its_own_mirror_keeps_the_subtree_it_has() {
    let (mut d, a, b) = cyclic_pair();
    sync_instances(&mut d, b).apply(&mut d).unwrap();
    // A > B (instance) > A (instance) > B (instance, the repeat) > Layer
    let b_in_a = child_named(&d, a, "B");
    let a_in_b_in_a = child_named(&d, b_in_a, "A");
    let repeat = child_named(&d, a_in_b_in_a, "B");
    let repeat_layer = child_named(&d, repeat, "Layer");
    let red = Color::rgb(200, 10, 10);
    let layer = layer_of(&d, b);
    fill(&mut d, layer, red);
    sync_instances(&mut d, b).apply(&mut d).unwrap();
    let want = d.get(layer).unwrap().fills.clone();
    assert_eq!(d.get(child_named(&d, b_in_a, "Layer")).unwrap().fills, want, "the instance of B in A follows B");
    assert_ne!(d.get(repeat_layer).unwrap().fills, want, "the repeat keeps its layers");
    assert_eq!(self_holding(&d).len(), 2);
}

/// Not every main inside an instance of itself is a cycle: an instance of Card on the page, with its inner icon swapped
/// to Card, mirrors Card in Card. Card holds nothing of itself, so that is finite, and the inner Card follows its main.
#[test]
fn an_instance_swapped_to_the_main_it_sits_in_on_the_page_still_follows_that_main() {
    let mut d = Document::new("swap to card");
    let page = d.add_page("P");
    let z = main_on(&mut d, page, "Z");
    let card = main_on(&mut d, page, "Card");
    place(&mut d, z, card); // Card holds an icon, an instance of Z
    let on_page = place(&mut d, card, page);
    let inner = child_named(&d, on_page, "Z");
    let swap = swap_instance(&mut d, inner, card);
    assert!(!is_nothing(&swap), "nothing holds itself on the page, so the swap is allowed");
    swap.apply(&mut d).unwrap();
    assert!(self_holding(&d).is_empty());

    let layer = layer_of(&d, card);
    fill(&mut d, layer, Color::rgb(200, 10, 10));
    sync_instances(&mut d, card).apply(&mut d).unwrap();
    let want = d.get(layer).unwrap().fills.clone();
    assert_eq!(d.get(child_named(&d, on_page, "Layer")).unwrap().fills, want, "the instance on the page follows Card");
    assert_eq!(d.get(child_named(&d, inner, "Layer")).unwrap().fills, want, "so does the Card inside it");
}

/// The same through an instance-swap property: its value on the page is Card itself.
#[test]
fn an_instance_swap_property_set_to_the_main_it_sits_in_on_the_page_still_follows_that_main() {
    let mut d = Document::new("swap prop to card");
    let page = d.add_page("P");
    let (card, _z) = card_with_icon_prop(&mut d);
    let on_page = place(&mut d, card, page);
    let set = set_instance_prop(&mut d, on_page, "Icon", Some(PropValue::Instance(card)));
    assert!(!is_nothing(&set), "nothing holds itself on the page, so the value is taken");
    set.apply(&mut d).unwrap();
    let inner = *d.get(on_page).unwrap().children.iter().find(|c| matches!(d.get(**c).unwrap().kind, NodeKind::Instance { .. })).unwrap();
    assert_eq!(instance_component(d.get(inner).unwrap()), Some(card));

    let layer = layer_of(&d, card);
    fill(&mut d, layer, Color::rgb(10, 200, 10));
    sync_instances(&mut d, card).apply(&mut d).unwrap();
    let want = d.get(layer).unwrap().fills.clone();
    assert_eq!(d.get(child_named(&d, inner, "Layer")).unwrap().fills, want, "the Card inside the Card follows its main");
    assert!(self_holding(&d).is_empty());
}

/// A value from an older build asks for a main that would hold itself: a sync keeps the instance on the main it has.
#[test]
fn a_stored_variant_value_that_would_make_a_main_hold_itself_is_left_alone_by_a_sync() {
    let (mut d, b, two, i) = variant_cycle();
    let one = instance_component(d.get(i).unwrap()).unwrap();
    let mut n = d.get(i).unwrap().clone();
    n.component.values.insert("State".into(), PropValue::Variant("Two".into()));
    Command::Update { node: Box::new(n) }.apply(&mut d).unwrap();
    for _ in 0..3 {
        sync_instances(&mut d, b).apply(&mut d).unwrap();
        sync_instances(&mut d, two).apply(&mut d).unwrap();
    }
    assert_eq!(instance_component(d.get(i).unwrap()), Some(one));
    assert!(self_holding(&d).is_empty());
}

#[test]
fn a_settled_cyclic_pair_survives_the_json_round_trip() {
    let (mut d, a, b) = cyclic_pair();
    for round in 0..6 {
        sync_instances(&mut d, if round % 2 == 0 { b } else { a }).apply(&mut d).unwrap();
    }
    let json = d.to_json().unwrap();
    let mut back = Document::from_json(&json).unwrap();
    assert_eq!(back.nodes.len(), d.nodes.len());
    sync_instances(&mut back, b).apply(&mut back).unwrap();
    assert_eq!(self_holding(&back).len(), 2);
}

/// `n` mains in a ring: Mi holds an instance of M(i+1), and the last holds one of M0 (made without the guard), plus
/// the given extra links `(from, to)`: main `from` holds an instance of main `to`.
fn ring(n: usize, extra_links: &[(usize, usize)]) -> (Document, Vec<NodeId>) {
    let mut d = Document::new("ring");
    let page = d.add_page("P");
    let mains: Vec<NodeId> = (0..n).map(|i| main_on(&mut d, page, &format!("M{i}"))).collect();
    for i in 0..n - 1 {
        place(&mut d, mains[i + 1], mains[i]);
    }
    let mut links = vec![(n - 1, 0)];
    links.extend_from_slice(extra_links);
    for (from, to) in links {
        let inst = place(&mut d, mains[to], page);
        Command::Reparent { id: inst, parent: mains[from], index: 0 }.apply(&mut d).unwrap();
    }
    (d, mains)
}

/// Longer cycles, and cycles that share mains: whatever the order of the syncs, the document stops growing.
#[test]
fn syncing_rings_and_tangles_of_mains_settles() {
    let shapes: [(usize, Vec<(usize, usize)>); 6] =
        [(2, vec![]), (3, vec![]), (4, vec![]), (3, vec![(1, 0)]), (4, vec![(2, 0), (3, 1)]), (5, vec![(0, 3), (4, 2), (2, 0)])];
    for (n, links) in shapes {
        let (mut d, mains) = ring(n, &links);
        let mut counts = Vec::new();
        for round in 0..60 {
            sync_instances(&mut d, mains[(round * 7 + round / 3) % n]).apply(&mut d).unwrap();
            counts.push(d.nodes.len());
        }
        assert_eq!(counts[30], counts[59], "{n} mains, links {links:?}: the tree stops growing: {counts:?}");
        assert!(counts[59] < 5000 && max_depth(&d) < 40, "{n} mains, links {links:?}: {} nodes, depth {}", counts[59], max_depth(&d));
    }
}

// ---------- a stored variant value that a sync refuses must not swallow other edits

/// A set "State" with the given variants, a Boolean "Show" bound to the layer of every variant, and a main B that holds an
/// instance I of the first variant with State stored as that variant. Nothing holds itself yet.
/// Returns (document, B, the variant mains in order, I).
fn show_toggle_fixture(variants: &[&str]) -> (Document, NodeId, Vec<NodeId>, NodeId) {
    let mut d = Document::new("show toggle");
    let page = d.add_page("P");
    let b = main_on(&mut d, page, "B");
    let mains: Vec<NodeId> = variants.iter().map(|v| main_on(&mut d, page, &format!("State={v}"))).collect();
    combine_as_variants(&mut d, &mains).apply(&mut d).unwrap();
    let set = d.get(mains[0]).unwrap().parent.unwrap();
    let def = ComponentPropDef { name: "Show".into(), kind: PropKind::Boolean, default: PropValue::Bool(true) };
    add_prop(&mut d, set, def).apply(&mut d).unwrap();
    for m in &mains {
        let layer = layer_of(&d, *m);
        bind_prop(&mut d, layer, OverrideField::Visible, Some("Show")).apply(&mut d).unwrap();
    }
    let i = place(&mut d, mains[0], b);
    set_instance_prop(&mut d, i, "State", Some(PropValue::Variant(variants[0].into()))).apply(&mut d).unwrap();
    assert_eq!(d.get(i).unwrap().component.values.get("State"), Some(&PropValue::Variant(variants[0].into())));
    assert!(self_holding(&d).is_empty());
    (d, b, mains, i)
}

fn rename(d: &mut Document, id: NodeId, name: &str) {
    let mut n = d.get(id).unwrap().clone();
    n.name = name.into();
    Command::Update { node: Box::new(n) }.apply(d).unwrap();
}

/// The stored value an older build or an agent can leave: State set on I without the sync that would refuse it.
fn store_state(d: &mut Document, i: NodeId, variant: &str) {
    let mut n = d.get(i).unwrap().clone();
    n.component.values.insert("State".into(), PropValue::Variant(variant.into()));
    Command::Update { node: Box::new(n) }.apply(d).unwrap();
}

fn show_of(d: &Document, i: NodeId) -> Option<PropValue> {
    d.get(i).unwrap().component.values.get("Show").cloned()
}

/// I has State=One stored, and an instance of B sits in variant Two (allowed: B holds One, not Two). Then the two
/// variants swap their names by renaming the layers, so the stored "One" now selects the variant that holds B: a sync
/// keeps I where it is. Turning "Show" off on I must still work.
#[test]
fn a_property_edit_lands_when_the_stored_variant_value_selects_a_main_that_holds_itself() {
    let (mut d, b, v, i) = show_toggle_fixture(&["One", "Two"]);
    let (one, two) = (v[0], v[1]);
    let set = d.get(one).unwrap().parent.unwrap();
    place(&mut d, b, two);
    for (id, name) in [(two, "State=Tmp"), (one, "State=Two"), (two, "State=One")] {
        rename(&mut d, id, name);
        sync_instances(&mut d, set).apply(&mut d).unwrap();
    }
    assert_eq!(instance_component(d.get(i).unwrap()), Some(one), "I stays on the node it is on");
    assert_eq!(d.get(i).unwrap().component.values.get("State"), Some(&PropValue::Variant("One".into())));
    assert!(self_holding(&d).is_empty());
    let dot = child_named(&d, i, "Layer");
    assert!(d.get(dot).unwrap().visible);
    let before = d.nodes.clone();

    let cmd = set_instance_prop(&mut d, i, "Show", Some(PropValue::Bool(false)));
    assert!(!is_nothing(&cmd), "the edit is not dropped");
    assert_eq!(d.nodes, before, "building the edit writes nothing");

    let undo = cmd.clone().apply(&mut d).unwrap();
    assert!(!d.get(dot).unwrap().visible, "the bound layer hides");
    assert_eq!(show_of(&d, i), Some(PropValue::Bool(false)));
    assert_eq!(instance_component(d.get(i).unwrap()), Some(one), "I still stays on its main");
    assert!(self_holding(&d).is_empty());
    let after = d.nodes.clone();

    undo.apply(&mut d).unwrap();
    assert_eq!(d.nodes, before, "one undo takes the whole edit back");
    cmd.apply(&mut d).unwrap();
    assert_eq!(d.nodes, after, "and one redo puts it back");
}

/// While an older refused value stays stored, a value of the user's own that would make a main hold itself is still
/// refused as a whole.
#[test]
fn a_new_variant_value_that_would_make_a_main_hold_itself_is_still_refused_next_to_a_stored_one() {
    let (mut d, b, v, i) = show_toggle_fixture(&["One", "Two", "Three"]);
    let (one, two, three) = (v[0], v[1], v[2]);
    place(&mut d, b, two);
    place(&mut d, b, three);
    store_state(&mut d, i, "Two");
    let set = d.get(one).unwrap().parent.unwrap();
    sync_instances(&mut d, set).apply(&mut d).unwrap();
    assert_eq!(instance_component(d.get(i).unwrap()), Some(one), "the stored Two is refused by the sync");
    let before = d.nodes.clone();

    let cmd = set_instance_prop(&mut d, i, "State", Some(PropValue::Variant("Three".into())));
    assert!(is_nothing(&cmd), "Three would make B hold itself: refused as a whole");
    assert_eq!(d.nodes, before);
    cmd.apply(&mut d).unwrap();
    assert_eq!(d.get(i).unwrap().component.values.get("State"), Some(&PropValue::Variant("Two".into())), "the stored value stays as it was");
    assert_eq!(instance_component(d.get(i).unwrap()), Some(one));
    assert!(self_holding(&d).is_empty());
}

/// An edit around a stored refused value, refused or taken, builds its command without writing to the document, and the
/// command undoes and redoes exactly.
#[test]
fn edits_next_to_a_stored_refused_variant_value_leave_no_stray_writes() {
    let (mut d, b, v, i) = show_toggle_fixture(&["One", "Two", "Three"]);
    let (one, two, three) = (v[0], v[1], v[2]);
    place(&mut d, b, two);
    place(&mut d, b, three);
    store_state(&mut d, i, "Two");
    let set = d.get(one).unwrap().parent.unwrap();
    sync_instances(&mut d, set).apply(&mut d).unwrap();
    let dot = child_named(&d, i, "Layer");
    let (nodes0, next0) = (d.nodes.clone(), d.peek_next_id());

    let refused = set_instance_prop(&mut d, i, "State", Some(PropValue::Variant("Three".into())));
    assert!(is_nothing(&refused));
    assert_eq!((&d.nodes, d.peek_next_id()), (&nodes0, next0), "a refused edit writes nothing");

    let hide = set_instance_prop(&mut d, i, "Show", Some(PropValue::Bool(false)));
    assert!(!is_nothing(&hide), "the edit is taken");
    assert_eq!((&d.nodes, d.peek_next_id()), (&nodes0, next0), "a taken edit builds its command without writing");
    let undo = hide.clone().apply(&mut d).unwrap();
    assert!(!d.get(dot).unwrap().visible);
    let nodes1 = d.nodes.clone();
    undo.apply(&mut d).unwrap();
    assert_eq!(d.nodes, nodes0, "undo");
    hide.apply(&mut d).unwrap();
    assert_eq!(d.nodes, nodes1, "redo");

    // clearing the value is an edit too, and it lands the same way
    let show = set_instance_prop(&mut d, i, "Show", None);
    assert!(!is_nothing(&show));
    show.apply(&mut d).unwrap();
    assert!(d.get(dot).unwrap().visible);
    assert_eq!(show_of(&d, i), None);
    assert!(self_holding(&d).is_empty());
}
