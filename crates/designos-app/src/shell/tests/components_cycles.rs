//! A main that holds an instance of itself, through any number of mains. Studio does not make one, and a file that
//! already carries one (from an older build or an agent) opens and edits without overflowing the stack.
//!
//! Crash tests run alone with `--exact`: a stack overflow aborts the whole test binary.

use super::*;
use designos_doc::{
    add_prop, bind_prop, combine_as_variants, create_component, create_instance, instance_component, mains_held_by, Command, ComponentPropDef, Node,
    OverrideField, PropKind, PropValue, Rect as DRect,
};

fn main_sim() -> Sim {
    Sim::with(Options { select: Some("Primary button".into()), ..Default::default() })
}

fn comps(s: &Sim) -> NodeId {
    s.app().doc.get(s.by_name("Primary button")).unwrap().parent.unwrap()
}

fn add_frame(s: &mut Sim, parent: NodeId, r: DRect, name: &str) -> NodeId {
    let id = s.shell.app.doc.alloc_id();
    let f = s.shell.app.doc.insert(parent, Node::frame(id, r)).unwrap();
    s.shell.app.doc.get_mut(f).unwrap().name = name.into();
    f
}

/// A main with one layer "Layer".
fn add_main(s: &mut Sim, parent: NodeId, r: DRect, name: &str) -> NodeId {
    let m = add_frame(s, parent, r, name);
    add_frame(s, m, DRect::new(10.0, 10.0, 40.0, 40.0), "Layer");
    create_component(&mut s.shell.app.doc, m).apply(&mut s.shell.app.doc).unwrap();
    m
}

fn place(s: &mut Sim, comp: NodeId, parent: NodeId) -> NodeId {
    let Command::Insert { nodes, .. } = create_instance(&mut s.shell.app.doc, comp, parent, DV::new(5.0, 5.0)) else {
        panic!("instance of {comp:?} in {parent:?} refused")
    };
    let id = nodes[0].id;
    let n = s.app().doc.get(parent).unwrap().children.len();
    Command::Insert { parent, index: n, nodes }.apply(&mut s.shell.app.doc).unwrap();
    id
}

fn layer_of(s: &Sim, main: NodeId) -> NodeId {
    *s.app().doc.get(main).unwrap().children.iter().find(|c| s.app().doc.get(**c).unwrap().name == "Layer").unwrap()
}

fn self_holding(s: &Sim) -> Vec<NodeId> {
    let doc = &s.app().doc;
    doc.nodes.values().filter(|n| matches!(n.kind, NodeKind::Component(_))).map(|n| n.id).filter(|m| mains_held_by(doc, &[*m]).contains(m)).collect()
}

/// Mains B and A on the Components page; A holds an instance of B.
fn a_holds_b(s: &mut Sim) -> (NodeId, NodeId) {
    let c = comps(s);
    let b = add_main(s, c, DRect::new(700.0, 0.0, 120.0, 120.0), "B");
    let a = add_main(s, c, DRect::new(900.0, 0.0, 120.0, 120.0), "A");
    place(s, b, a);
    (a, b)
}

/// The same pair, but with an instance of A inside B as well, put there with no guard at all (what a file from an
/// older build or an agent can carry). Returns (A, B).
fn cyclic(s: &mut Sim) -> (NodeId, NodeId) {
    let (a, b) = a_holds_b(s);
    let page = s.app().page;
    let inst = place(s, a, page);
    Command::Reparent { id: inst, parent: b, index: 0 }.apply(&mut s.shell.app.doc).unwrap();
    assert_eq!(self_holding(s).len(), 2, "the fixture holds itself");
    (a, b)
}

// ---------- the Properties panel cannot make a main hold itself

#[test]
fn a_variant_switch_that_would_make_a_main_hold_itself_changes_nothing() {
    let mut s = main_sim();
    let c = comps(&s);
    let b = add_main(&mut s, c, DRect::new(700.0, 0.0, 120.0, 120.0), "B");
    let one = add_frame(&mut s, c, DRect::new(0.0, 700.0, 60.0, 60.0), "State=One");
    let two = add_frame(&mut s, c, DRect::new(100.0, 700.0, 60.0, 60.0), "State=Two");
    for m in [one, two] {
        add_frame(&mut s, m, DRect::new(5.0, 5.0, 20.0, 20.0), "Dot");
        create_component(&mut s.shell.app.doc, m).apply(&mut s.shell.app.doc).unwrap();
    }
    combine_as_variants(&mut s.shell.app.doc, &[one, two]).apply(&mut s.shell.app.doc).unwrap();
    let i = place(&mut s, one, b);
    place(&mut s, b, two); // Two holds B, so B's instance of One must not become Two
    let values = s.app().doc.get(i).unwrap().component.values.clone();

    s.shell.app.set_instance_prop(i, "State", Some(PropValue::Variant("Two".into())));

    let n = s.app().doc.get(i).unwrap();
    assert_eq!(instance_component(n), Some(one), "the instance stays on State=One");
    assert_eq!(n.component.values, values, "and the value it shows stays as it was");
    assert!(self_holding(&s).is_empty());
    s.shell.app.edit_nodes(&[layer_of(&s, b)], |n| n.opacity = 0.5);
    s.frame(vec![]);
}

#[test]
fn an_instance_swap_property_that_would_make_a_main_hold_itself_changes_nothing() {
    let mut s = main_sim();
    let (a, b) = a_holds_b(&mut s);
    let c = comps(&s);
    let z = add_main(&mut s, c, DRect::new(1100.0, 0.0, 60.0, 60.0), "Z");
    let card = add_main(&mut s, c, DRect::new(0.0, 900.0, 200.0, 100.0), "Card");
    let slot = place(&mut s, z, card);
    let def = ComponentPropDef { name: "Icon".into(), kind: PropKind::InstanceSwap { preferred: vec![] }, default: PropValue::Instance(z) };
    add_prop(&mut s.shell.app.doc, card, def).apply(&mut s.shell.app.doc).unwrap();
    bind_prop(&mut s.shell.app.doc, slot, OverrideField::MainComponent, Some("Icon")).apply(&mut s.shell.app.doc).unwrap();
    let k = place(&mut s, card, b); // a Card inside B; A holds B
    let nested = *s.app().doc.get(k).unwrap().children.iter().find(|c| matches!(s.app().doc.get(**c).unwrap().kind, NodeKind::Instance { .. })).unwrap();
    let values = s.app().doc.get(k).unwrap().component.values.clone();

    s.shell.app.set_instance_prop(k, "Icon", Some(PropValue::Instance(a)));

    assert_eq!(instance_component(s.app().doc.get(nested).unwrap()), Some(z), "the icon stays Z");
    assert_eq!(s.app().doc.get(k).unwrap().component.values, values, "and the property keeps its value");
    assert!(self_holding(&s).is_empty());
    s.shell.app.edit_nodes(&[layer_of(&s, b)], |n| n.opacity = 0.5);
    s.frame(vec![]);
}

// ---------- a document that already holds itself

#[test]
fn the_first_edit_of_a_layer_in_a_main_of_a_document_that_already_holds_itself_does_not_abort() {
    let mut s = main_sim();
    let (a, b) = cyclic(&mut s);
    s.frame(vec![]);
    s.shell.app.edit_nodes(&[layer_of(&s, b)], |n| n.opacity = 0.5);
    s.frame(vec![]);
    assert_eq!(s.app().doc.get(layer_of(&s, b)).unwrap().opacity, 0.5, "the edit lands");
    // a second edit, in the other main, then undo and redo of both
    s.shell.app.edit_nodes(&[layer_of(&s, a)], |n| n.opacity = 0.25);
    s.frame(vec![]);
    s.shell.app.undo();
    s.shell.app.undo();
    s.shell.app.redo();
    s.frame(vec![]);
    // every layer of the two mains, selected in turn, draws its panels
    let mut ids = Vec::new();
    for main in [a, b] {
        s.app().doc.walk(main, |n, _| ids.push(n.id));
    }
    for id in ids {
        s.shell.app.selection = vec![id];
        s.frame(vec![]);
    }
    assert_eq!(self_holding(&s).len(), 2, "the file holds itself as before: Studio does not rewrite it");
}

#[test]
fn a_document_that_already_holds_itself_saves_reopens_and_edits() {
    let mut s = main_sim();
    let (_a, b) = cyclic(&mut s);
    let path = std::env::temp_dir().join(format!("designos_cycle_{}.designos", std::process::id()));
    crate::io::save_path(&s.app().doc, &path).unwrap();
    s.shell.app.open_path(path.clone());
    std::fs::remove_file(&path).ok();
    s.frame(vec![]);
    let b = s.app().doc.nodes.values().find(|n| n.name == "B" && matches!(n.kind, NodeKind::Component(_))).map_or(b, |n| n.id);
    assert_eq!(self_holding(&s).len(), 2, "the reopened file still holds itself");
    s.shell.app.edit_nodes(&[layer_of(&s, b)], |n| n.opacity = 0.5);
    s.frame(vec![]);
    assert_eq!(s.app().doc.get(layer_of(&s, b)).unwrap().opacity, 0.5);
}
