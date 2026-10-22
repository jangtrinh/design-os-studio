//! Copy and paste, Ctrl+D and Alt-drag of a main component (or a component set) in the file it lives in make an
//! instance of it, so the file keeps one main and Assets one entry. Pasting into another file copies for real.

use super::*;
use designos_doc::{instance_component, Node, Rect as DRect};

fn mains(s: &Sim) -> Vec<NodeId> {
    s.app().doc.nodes.values().filter(|n| matches!(n.kind, NodeKind::Component(_))).map(|n| n.id).collect()
}

fn center(s: &Sim, id: NodeId) -> DV {
    crate::util::rect_center(&crate::util::world_aabb(&s.app().doc, id))
}

/// The one new layer a gesture selected, which must be an instance of `main`.
fn the_new_instance_of(s: &Sim, main: NodeId) -> NodeId {
    let [new] = s.app().selection[..] else { panic!("one layer selected, got {:?}", s.app().selection) };
    let n = s.app().doc.get(new).expect("selected layer exists");
    assert!(matches!(n.kind, NodeKind::Instance { .. }), "the copy is an {:?}, not an instance", n.kind);
    assert_eq!(instance_component(n), Some(main), "the instance points at the original main");
    assert_eq!(n.component.source, Some(main));
    new
}

/// Everything a gesture must leave alone: the mains, and the Assets rows.
struct Before {
    mains: Vec<NodeId>,
    assets: usize,
    nodes: usize,
}

fn before(s: &Sim) -> Before {
    Before { mains: mains(s), assets: s.app().local_components().len(), nodes: s.app().doc.nodes.len() }
}

fn assert_still_one_of_each(s: &Sim, b: &Before) {
    assert_eq!(mains(s), b.mains, "no main was added");
    assert_eq!(s.app().local_components().len(), b.assets, "Assets lists each main once");
}

fn assert_one_undo_removes_it(s: &mut Sim, b: &Before) {
    s.cmd_key(Key::Z);
    assert_eq!(s.app().doc.nodes.len(), b.nodes, "one undo takes the whole copy away");
    assert_still_one_of_each(s, b);
}

fn main_sim() -> (Sim, NodeId) {
    let s = Sim::with(Options { select: Some("Primary button".into()), ..Default::default() });
    let main = s.by_name("Primary button");
    (s, main)
}

/// A main with no layers inside, so a press on it lands on the main itself: a press inside a top-level frame
/// picks the layer under the pointer, and "Primary button" is covered by its own layers.
fn bare_main(s: &mut Sim) -> NodeId {
    let comps = s.app().doc.get(s.by_name("Primary button")).unwrap().parent.unwrap();
    let id = s.shell.app.doc.alloc_id();
    let frame = s.shell.app.doc.insert(comps, Node::frame(id, DRect::new(300.0, 300.0, 120.0, 60.0))).unwrap();
    s.shell.app.doc.get_mut(frame).unwrap().name = "Bare main".into();
    s.shell.app.selection = vec![frame];
    s.shell.app.create_component();
    assert_eq!(s.app().selection, vec![frame], "the frame became the main in place");
    // the page was fitted around "Primary button" alone: bring the new main into view so a press can reach it
    s.shell.app.fit_selection();
    s.frame(vec![]);
    frame
}

/// "Primary button" and a plain frame next to it, combined into a set; returns the set and its first variant.
fn set_sim() -> (Sim, NodeId, NodeId) {
    let (mut s, main) = main_sim();
    let comps = s.app().doc.get(main).unwrap().parent.unwrap();
    let id = s.shell.app.doc.alloc_id();
    let other = s.shell.app.doc.insert(comps, Node::frame(id, DRect::new(400.0, 120.0, 168.0, 44.0))).unwrap();
    s.shell.app.selection = vec![main, other];
    s.shell.app.combine_selection_as_variants();
    let [set] = s.app().selection[..] else { panic!("the set is selected") };
    assert!(matches!(s.app().doc.get(set).unwrap().kind, NodeKind::ComponentSet(_)));
    let first = s.app().doc.get(set).unwrap().children[0];
    s.shell.app.toast = None;
    (s, set, first)
}

fn copy_then_paste(s: &mut Sim) {
    let json = s.shell.app.copy_selection().expect("something to copy");
    s.shell.app.paste(Some(&json));
}

fn alt_drag(s: &mut Sim, at: DV) {
    s.mods = Modifiers::ALT;
    let a = s.screen(at.x, at.y);
    assert!(s.app().canvas_rect.contains(a), "the press lands on the canvas: {a:?} in {:?}", s.app().canvas_rect);
    s.drag(a, a + Vec2::new(0.0, 90.0));
    s.mods = Modifiers::NONE;
}

// ---- a main component

#[test]
fn pasting_a_main_with_the_main_still_selected_makes_an_instance_beside_it() {
    let (mut s, main) = main_sim();
    let b = before(&s);
    assert_eq!(s.app().selection, vec![main]);
    copy_then_paste(&mut s);
    let inst = the_new_instance_of(&s, main);
    assert_eq!(s.app().doc.get(inst).unwrap().parent, s.app().doc.get(main).unwrap().parent, "beside the main, not inside it");
    assert_still_one_of_each(&s, &b);
    assert_one_undo_removes_it(&mut s, &b);
}

#[test]
fn pasting_a_main_with_nothing_selected_makes_an_instance() {
    let (mut s, main) = main_sim();
    let b = before(&s);
    let json = s.shell.app.copy_selection().unwrap();
    s.shell.app.selection.clear();
    s.shell.app.paste(Some(&json));
    the_new_instance_of(&s, main);
    assert_still_one_of_each(&s, &b);
}

#[test]
fn pasting_the_in_app_copy_of_a_main_makes_an_instance() {
    let (mut s, main) = main_sim();
    let b = before(&s);
    s.shell.app.copy_selection().unwrap();
    s.shell.app.paste(None);
    the_new_instance_of(&s, main);
    assert_still_one_of_each(&s, &b);
}

#[test]
fn ctrl_d_on_a_main_makes_an_instance() {
    let (mut s, main) = main_sim();
    let b = before(&s);
    s.cmd_key(Key::D);
    the_new_instance_of(&s, main);
    assert_still_one_of_each(&s, &b);
    assert_one_undo_removes_it(&mut s, &b);
}

#[test]
fn alt_drag_of_a_main_makes_an_instance() {
    let (mut s, _) = main_sim();
    let main = bare_main(&mut s);
    let b = before(&s);
    let c = center(&s, main);
    alt_drag(&mut s, c);
    let inst = the_new_instance_of(&s, main);
    assert_ne!(s.app().doc.get(inst).unwrap().transform, s.app().doc.get(main).unwrap().transform, "the instance followed the drag");
    assert_eq!(s.app().doc.get(main).unwrap().transform.ty, 300.0, "the main stayed put");
    assert_eq!(s.app().doc.get(inst).unwrap().parent, s.app().doc.get(main).unwrap().parent, "dropped on its main, it lands beside it");
    assert_still_one_of_each(&s, &b);
    assert_one_undo_removes_it(&mut s, &b);
}

#[test]
fn dropping_an_instance_on_its_own_main_leaves_it_beside_the_main() {
    let (mut s, main) = main_sim();
    let comps = s.app().doc.get(main).unwrap().parent.unwrap();
    let inst = s.shell.app.insert_instance(main, DV::new(600.0, 140.0)).expect("instance placed");
    assert_eq!(s.app().doc.get(inst).unwrap().parent, Some(comps));
    s.shell.app.fit_all();
    s.frame(vec![]);
    let (from, to) = (center(&s, inst), center(&s, main));
    s.drag(s.screen(from.x, from.y), s.screen(to.x, to.y));
    assert_eq!(s.app().doc.get(inst).unwrap().parent, Some(comps), "not inside the main it links to");
    assert!(designos_doc::instances_of(&s.app().doc, main).iter().all(|i| !s.app().doc.is_ancestor(main, *i)), "no instance inside the main");
}

// ---- a component set

#[test]
fn pasting_a_set_makes_an_instance_of_its_default_variant() {
    let (mut s, set, first) = set_sim();
    let b = before(&s);
    assert_eq!(s.app().selection, vec![set]);
    copy_then_paste(&mut s);
    let inst = the_new_instance_of(&s, first);
    assert_eq!(s.app().doc.get(inst).unwrap().parent, s.app().doc.get(set).unwrap().parent, "beside the set, not inside it");
    assert_still_one_of_each(&s, &b);
    assert_one_undo_removes_it(&mut s, &b);
}

#[test]
fn ctrl_d_on_a_set_makes_an_instance_of_its_default_variant() {
    let (mut s, _set, first) = set_sim();
    let b = before(&s);
    s.cmd_key(Key::D);
    the_new_instance_of(&s, first);
    assert_still_one_of_each(&s, &b);
    assert_one_undo_removes_it(&mut s, &b);
}

#[test]
fn alt_drag_of_a_set_makes_an_instance_of_its_default_variant() {
    let (mut s, set, first) = set_sim();
    let b = before(&s);
    let r = crate::util::world_aabb(&s.app().doc, set);
    // the set's own padding, where no variant sits under the pointer
    alt_drag(&mut s, DV::new(r.x + 3.0, r.y + 3.0));
    the_new_instance_of(&s, first);
    assert_still_one_of_each(&s, &b);
    assert_one_undo_removes_it(&mut s, &b);
}

#[test]
fn ctrl_d_on_a_variant_still_adds_a_variant_to_its_set() {
    let (mut s, set, first) = set_sim();
    let variants = s.app().doc.get(set).unwrap().children.len();
    s.shell.app.selection = vec![first];
    s.cmd_key(Key::D);
    assert_eq!(s.app().doc.get(set).unwrap().children.len(), variants + 1, "the copy is a new variant");
    let [new] = s.app().selection[..] else { panic!("one layer selected") };
    assert!(matches!(s.app().doc.get(new).unwrap().kind, NodeKind::Component(_)));
    assert_eq!(s.app().doc.get(new).unwrap().parent, Some(set));
}

// ---- mixed selections

#[test]
fn ctrl_d_on_a_main_and_a_plain_frame_instantiates_the_main_and_copies_the_frame() {
    let (mut s, main) = main_sim();
    let comps = s.app().doc.get(main).unwrap().parent.unwrap();
    let id = s.shell.app.doc.alloc_id();
    let frame = s.shell.app.doc.insert(comps, Node::frame(id, DRect::new(400.0, 400.0, 50.0, 50.0))).unwrap();
    s.shell.app.doc.get_mut(frame).unwrap().name = "Plain".into();
    s.shell.app.selection = vec![main, frame];
    let b = before(&s);
    s.cmd_key(Key::D);
    let [a, c] = s.app().selection[..] else { panic!("two copies selected, got {:?}", s.app().selection) };
    let (inst, plain) = if matches!(s.app().doc.get(a).unwrap().kind, NodeKind::Instance { .. }) { (a, c) } else { (c, a) };
    assert_eq!(instance_component(s.app().doc.get(inst).unwrap()), Some(main));
    assert!(matches!(s.app().doc.get(plain).unwrap().kind, NodeKind::Frame(_)), "the frame is copied as a frame");
    assert_eq!(s.app().doc.get(plain).unwrap().name, "Plain");
    assert_still_one_of_each(&s, &b);
}

#[test]
fn pasting_a_main_and_a_plain_frame_instantiates_the_main_and_copies_the_frame() {
    let (mut s, main) = main_sim();
    let comps = s.app().doc.get(main).unwrap().parent.unwrap();
    let id = s.shell.app.doc.alloc_id();
    let frame = s.shell.app.doc.insert(comps, Node::frame(id, DRect::new(400.0, 400.0, 50.0, 50.0))).unwrap();
    s.shell.app.selection = vec![main, frame];
    let b = before(&s);
    copy_then_paste(&mut s);
    let [a, c] = s.app().selection[..] else { panic!("two layers selected, got {:?}", s.app().selection) };
    let kinds = [&s.app().doc.get(a).unwrap().kind, &s.app().doc.get(c).unwrap().kind];
    assert_eq!(kinds.iter().filter(|k| matches!(k, NodeKind::Instance { .. })).count(), 1, "{kinds:?}");
    assert_eq!(kinds.iter().filter(|k| matches!(k, NodeKind::Frame(_))).count(), 1, "{kinds:?}");
    assert_still_one_of_each(&s, &b);
}

// ---- another file, and a main that is gone

#[test]
fn pasting_into_another_tab_copies_the_main_for_real() {
    let (mut s, main) = main_sim();
    let json = s.shell.app.copy_selection().unwrap();
    // The duplicated tab holds a document with the same node ids, a main at `main` included: only the
    // document identity tells it from the file the copy came from.
    s.shell.app.duplicate_tab(s.app().tabs.active_idx).expect("duplicated tab");
    assert!(matches!(s.app().doc.get(main).map(|n| &n.kind), Some(NodeKind::Component(_))));
    let b = before(&s);
    s.shell.app.selection.clear();
    s.shell.app.paste(Some(&json));
    let [new] = s.app().selection[..] else { panic!("one layer selected") };
    assert_ne!(new, main);
    assert!(matches!(s.app().doc.get(new).unwrap().kind, NodeKind::Component(_)), "a real copy of the main");
    assert_eq!(mains(&s).len(), b.mains.len() + 1);
}

#[test]
fn the_in_app_copy_pastes_for_real_into_another_tab_too() {
    let (mut s, _main) = main_sim();
    s.shell.app.copy_selection().unwrap();
    s.shell.app.duplicate_tab(s.app().tabs.active_idx).expect("duplicated tab");
    let b = before(&s);
    s.shell.app.selection.clear();
    s.shell.app.paste(None);
    let [new] = s.app().selection[..] else { panic!("one layer selected") };
    assert!(matches!(s.app().doc.get(new).unwrap().kind, NodeKind::Component(_)));
    assert_eq!(mains(&s).len(), b.mains.len() + 1);
}

#[test]
fn cut_and_paste_moves_a_main_instead_of_linking_to_the_deleted_one() {
    let (mut s, main) = main_sim();
    let json = s.shell.app.cut_selection().unwrap();
    assert!(s.app().doc.get(main).is_none());
    let before_paste = mains(&s).len();
    s.shell.app.paste(Some(&json));
    let [new] = s.app().selection[..] else { panic!("one layer selected") };
    assert!(matches!(s.app().doc.get(new).unwrap().kind, NodeKind::Component(_)), "the cut main comes back as a main");
    assert_eq!(mains(&s).len(), before_paste + 1);
}

// ---- what stays as it was

#[test]
fn a_plain_layer_and_an_instance_copy_as_before() {
    let (mut s, main) = main_sim();
    let inst = s.by_name("Get started");
    s.shell.app.selection = vec![inst];
    let b = before(&s);
    s.cmd_key(Key::D);
    the_new_instance_of(&s, main);
    assert_still_one_of_each(&s, &b);
    let comps = s.app().doc.get(main).unwrap().parent.unwrap();
    let id = s.shell.app.doc.alloc_id();
    let plain = s.shell.app.doc.insert(comps, Node::frame(id, DRect::new(500.0, 500.0, 40.0, 40.0))).unwrap();
    s.shell.app.selection = vec![plain];
    copy_then_paste(&mut s);
    let [new] = s.app().selection[..] else { panic!("one layer selected") };
    assert!(matches!(s.app().doc.get(new).unwrap().kind, NodeKind::Frame(_)));
}
