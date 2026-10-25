//! Copy and paste, Ctrl+D and Alt-drag of a main component (or a component set) in the file it lives in make an
//! instance of it, so the file keeps one main and Assets one entry. Pasting into another file copies for real.

use super::*;
use designos_doc::{instance_component, sync_instances, ConnectorEnd, ConnectorProps, Magnet, Node, Rect as DRect};

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

// ---- what hangs on a main, and where a copy may go

fn add_frame(s: &mut Sim, parent: NodeId, rect: DRect, name: &str) -> NodeId {
    let id = s.shell.app.doc.alloc_id();
    let frame = s.shell.app.doc.insert(parent, Node::frame(id, rect)).unwrap();
    s.shell.app.doc.get_mut(frame).unwrap().name = name.into();
    frame
}

/// A main with one layer inside, on `parent`.
fn add_main(s: &mut Sim, parent: NodeId, rect: DRect, name: &str) -> NodeId {
    let main = add_frame(s, parent, rect, name);
    add_frame(s, main, DRect::new(10.0, 10.0, 40.0, 40.0), "Layer");
    designos_doc::create_component(&mut s.shell.app.doc, main).apply(&mut s.shell.app.doc).unwrap();
    main
}

fn connect(s: &mut Sim, parent: NodeId, from: NodeId, to: NodeId) -> NodeId {
    let id = s.shell.app.doc.alloc_id();
    let props = ConnectorProps { start: ConnectorEnd::attached(from, Magnet::Auto), end: ConnectorEnd::attached(to, Magnet::Auto), ..Default::default() };
    s.shell.app.doc.insert(parent, Node::connector(id, props)).unwrap()
}

fn ends(s: &Sim, connector: NodeId) -> (Option<NodeId>, Option<NodeId>) {
    let NodeKind::Connector(c) = &s.app().doc.get(connector).unwrap().kind else { panic!("{connector:?} is no connector") };
    (c.start.node, c.end.node)
}

/// The layer of `instance` that mirrors `source` (a layer of its main).
fn mirror_of(s: &Sim, instance: NodeId, source: NodeId) -> NodeId {
    *s.app().doc.get(instance).unwrap().children.iter().find(|c| s.app().doc.get(**c).unwrap().component.source == Some(source)).expect("a mirror of the layer")
}

/// "Primary button" and a plain frame, with a connector from the main to the frame and one from a layer in the main
/// to the frame, all selected. Returns (main, its layer, the two connectors).
fn main_with_two_connectors() -> (Sim, NodeId, NodeId, NodeId, NodeId) {
    let (mut s, main) = main_sim();
    let comps = s.app().doc.get(main).unwrap().parent.unwrap();
    let other = add_frame(&mut s, comps, DRect::new(600.0, 600.0, 40.0, 40.0), "Other");
    let layer = child_of(&s, main, "Bg");
    let (on_main, on_layer) = (connect(&mut s, comps, main, other), connect(&mut s, comps, layer, other));
    s.shell.app.selection = vec![main, other, on_main, on_layer];
    (s, main, layer, on_main, on_layer)
}

fn child_of(s: &Sim, parent: NodeId, name: &str) -> NodeId {
    *s.app().doc.get(parent).unwrap().children.iter().find(|c| s.app().doc.get(**c).unwrap().name == name).unwrap_or_else(|| panic!("no child {name}"))
}

/// The copied connectors (their starts) and the copy of the plain frame, from a selection of four new layers.
fn assert_connectors_follow_the_instance(s: &Sim, main: NodeId, layer: NodeId) {
    let sel = s.app().selection.clone();
    let doc = &s.app().doc;
    let instance = *sel.iter().find(|i| matches!(doc.get(**i).unwrap().kind, NodeKind::Instance { .. })).expect("an instance among the copies");
    assert_eq!(instance_component(doc.get(instance).unwrap()), Some(main));
    let frame_copy = *sel.iter().find(|i| doc.get(**i).unwrap().name == "Other").expect("the frame copy");
    let connectors: Vec<NodeId> = sel.iter().copied().filter(|i| matches!(doc.get(*i).unwrap().kind, NodeKind::Connector(_))).collect();
    assert_eq!(connectors.len(), 2);
    let mut starts: Vec<Option<NodeId>> = connectors.iter().map(|c| ends(s, *c).0).collect();
    starts.sort();
    let mut want = vec![Some(instance), Some(mirror_of(s, instance, layer))];
    want.sort();
    assert_eq!(starts, want, "one connector is attached to the instance, the other to the instance's copy of the layer");
    assert!(connectors.iter().all(|c| ends(s, *c).1 == Some(frame_copy)), "the other end follows the copied frame");
    assert!(connectors.iter().all(|c| ends(s, *c).0.is_some_and(|n| doc.get(n).is_some())), "no end names a layer that was never inserted");
}

#[test]
fn ctrl_d_keeps_a_connector_on_the_main_attached_to_the_new_instance() {
    let (mut s, main, layer, _, _) = main_with_two_connectors();
    s.cmd_key(Key::D);
    assert_connectors_follow_the_instance(&s, main, layer);
}

#[test]
fn pasting_keeps_a_connector_on_the_main_attached_to_the_new_instance() {
    let (mut s, main, layer, _, _) = main_with_two_connectors();
    copy_then_paste(&mut s);
    assert_connectors_follow_the_instance(&s, main, layer);
}

#[test]
fn ctrl_d_of_a_hidden_main_makes_a_hidden_instance() {
    let (mut s, main) = main_sim();
    s.shell.app.doc.get_mut(main).unwrap().visible = false;
    s.cmd_key(Key::D);
    let inst = the_new_instance_of(&s, main);
    assert!(!s.app().doc.get(inst).unwrap().visible, "the copy of a hidden layer stays hidden");
}

/// Main "M2" with a layer, an instance of it, and the instance's copy of the layer.
fn main_and_its_instance(s: &mut Sim) -> (NodeId, NodeId, NodeId) {
    let comps = s.app().doc.get(s.by_name("Primary button")).unwrap().parent.unwrap();
    let m2 = add_main(s, comps, DRect::new(700.0, 0.0, 200.0, 200.0), "M2");
    s.shell.app.selection.clear();
    let inst = s.shell.app.insert_instance(m2, DV::new(1200.0, 600.0)).expect("instance placed");
    let mirror = s.app().doc.get(inst).unwrap().children[0];
    (m2, inst, mirror)
}

/// What the next edit of the main does to its instances.
fn edit_main(s: &mut Sim, main: NodeId) {
    let cmd = sync_instances(&mut s.shell.app.doc, main);
    let _ = cmd.apply(&mut s.shell.app.doc);
}

#[test]
fn a_main_pasted_into_a_layer_of_its_own_instance_is_a_real_copy_that_an_edit_of_the_main_keeps() {
    let (mut s, _) = main_sim();
    let (m2, _inst, mirror) = main_and_its_instance(&mut s);
    s.shell.app.selection = vec![m2];
    let json = s.shell.app.copy_selection().unwrap();
    s.shell.app.selection = vec![mirror];
    s.shell.app.paste(Some(&json));
    let [new] = s.app().selection[..] else { panic!("one layer selected") };
    assert!(matches!(s.app().doc.get(new).unwrap().kind, NodeKind::Component(_)), "a real copy, not an instance that sits in an instance of its own main");
    edit_main(&mut s, m2);
    assert!(s.app().doc.get(new).is_some(), "the next edit of the main does not remove the pasted layer");
}

/// "A" holds an instance of "B"; both are mains on the page.
fn a_holds_an_instance_of_b(s: &mut Sim) -> (NodeId, NodeId) {
    let comps = s.app().doc.get(s.by_name("Primary button")).unwrap().parent.unwrap();
    let b = add_main(s, comps, DRect::new(700.0, 0.0, 120.0, 120.0), "B");
    let a = add_main(s, comps, DRect::new(900.0, 0.0, 120.0, 120.0), "A");
    let designos_doc::Command::Insert { nodes, .. } = designos_doc::create_instance(&mut s.shell.app.doc, b, a, DV::new(5.0, 5.0)) else {
        panic!("instance of B")
    };
    designos_doc::Command::Insert { parent: a, index: 0, nodes }.apply(&mut s.shell.app.doc).unwrap();
    (a, b)
}

fn assert_a_pasted_inside_b_is_valid(s: &mut Sim, b: NodeId) {
    for id in s.app().selection.clone() {
        assert!(!s.app().doc.is_ancestor(b, id) && id != b, "{id:?} would make B hold itself");
    }
    // the edit that overflowed the stack when B held itself
    edit_main(s, b);
    s.frame(vec![]);
}

#[test]
fn pasting_a_main_that_holds_an_instance_of_b_with_b_selected_does_not_make_b_hold_itself() {
    let (mut s, _) = main_sim();
    let (a, b) = a_holds_an_instance_of_b(&mut s);
    s.shell.app.selection = vec![a];
    let json = s.shell.app.copy_selection().unwrap();
    s.shell.app.selection = vec![b];
    s.shell.app.paste(Some(&json));
    assert_a_pasted_inside_b_is_valid(&mut s, b);
}

#[test]
fn pasting_a_main_that_holds_an_instance_of_b_with_a_layer_of_b_selected_does_not_make_b_hold_itself() {
    let (mut s, _) = main_sim();
    let (a, b) = a_holds_an_instance_of_b(&mut s);
    s.shell.app.selection = vec![a];
    let json = s.shell.app.copy_selection().unwrap();
    s.shell.app.selection = vec![child_of(&s, b, "Layer")];
    s.shell.app.paste(Some(&json));
    assert_a_pasted_inside_b_is_valid(&mut s, b);
}

#[test]
fn pasting_a_plain_frame_that_holds_an_instance_of_b_with_b_selected_does_not_make_b_hold_itself() {
    let (mut s, _) = main_sim();
    let (a, b) = a_holds_an_instance_of_b(&mut s);
    let comps = s.app().doc.get(a).unwrap().parent.unwrap();
    let holder = add_frame(&mut s, comps, DRect::new(0.0, 700.0, 100.0, 100.0), "Holder");
    let designos_doc::Command::Insert { nodes, .. } = designos_doc::create_instance(&mut s.shell.app.doc, b, holder, DV::new(5.0, 5.0)) else {
        panic!("instance of B")
    };
    designos_doc::Command::Insert { parent: holder, index: 0, nodes }.apply(&mut s.shell.app.doc).unwrap();
    s.shell.app.selection = vec![holder];
    let json = s.shell.app.copy_selection().unwrap();
    s.shell.app.selection = vec![b];
    s.shell.app.paste(Some(&json));
    assert_a_pasted_inside_b_is_valid(&mut s, b);
}

// ---- every other way to put a main holding B in B

/// "A" holds an instance of "B": the clipboard holds A, and the selection is B (or its one layer).
fn a_copied_with_b_selected(s: &mut Sim, a_layer_of_b: bool) -> (NodeId, NodeId) {
    let (a, b) = a_holds_an_instance_of_b(s);
    s.shell.app.selection = vec![a];
    s.shell.app.copy_selection().expect("A is copied");
    s.shell.app.selection = vec![if a_layer_of_b { child_of(s, b, "Layer") } else { b }];
    (a, b)
}

/// The one layer a command selected is a real copy of A, beside B: in B's parent, right above B, at B's place.
fn assert_a_copy_sits_beside(s: &mut Sim, b: NodeId) {
    let [new] = s.app().selection[..] else { panic!("one layer selected, got {:?}", s.app().selection) };
    let doc = &s.app().doc;
    let (b_node, copy) = (doc.get(b).unwrap(), doc.get(new).unwrap());
    assert!(matches!(copy.kind, NodeKind::Component(_)), "a real copy of A, got {:?}", copy.kind);
    assert_eq!(copy.parent, b_node.parent, "beside B, not in it");
    let siblings = &doc.get(b_node.parent.unwrap()).unwrap().children;
    let at = |id: NodeId| siblings.iter().position(|c| *c == id).unwrap();
    assert_eq!(at(new), at(b) + 1, "right above B");
    assert_eq!((copy.transform.tx, copy.transform.ty), (b_node.transform.tx, b_node.transform.ty), "at B's place");
    assert_a_pasted_inside_b_is_valid(s, b);
}

#[test]
fn paste_over_selection_with_b_selected_puts_a_main_that_holds_b_beside_b() {
    let (mut s, _) = main_sim();
    let (_, b) = a_copied_with_b_selected(&mut s, false);
    s.shell.app.paste_over_selection();
    assert_a_copy_sits_beside(&mut s, b);
}

#[test]
fn paste_over_selection_with_a_layer_of_b_selected_puts_a_main_that_holds_b_beside_b() {
    let (mut s, _) = main_sim();
    let (_, b) = a_copied_with_b_selected(&mut s, true);
    s.shell.app.paste_over_selection();
    assert_a_copy_sits_beside(&mut s, b);
}

#[test]
fn paste_to_replace_a_layer_of_b_puts_a_main_that_holds_b_beside_b() {
    let (mut s, _) = main_sim();
    let (_, b) = a_copied_with_b_selected(&mut s, true);
    let layer = s.app().selection[0];
    s.shell.app.paste_to_replace();
    assert!(s.app().doc.get(layer).is_none(), "the layer is replaced");
    assert_a_copy_sits_beside(&mut s, b);
}

#[test]
fn paste_to_replace_in_the_ordinary_place_still_replaces_in_place() {
    let (mut s, _) = main_sim();
    let (a, b) = a_holds_an_instance_of_b(&mut s);
    let comps = s.app().doc.get(a).unwrap().parent.unwrap();
    let plain = add_frame(&mut s, comps, DRect::new(0.0, 700.0, 100.0, 100.0), "Plain");
    s.shell.app.selection = vec![a];
    s.shell.app.copy_selection().expect("A is copied");
    s.shell.app.selection = vec![plain];
    let (index, tx, ty) = {
        let doc = &s.app().doc;
        (doc.get(comps).unwrap().children.iter().position(|c| *c == plain).unwrap(), doc.get(plain).unwrap().transform.tx, doc.get(plain).unwrap().transform.ty)
    };
    s.shell.app.paste_to_replace();
    let [new] = s.app().selection[..] else { panic!("one layer selected") };
    let n = s.app().doc.get(new).unwrap();
    assert!(s.app().doc.get(plain).is_none());
    assert_eq!(n.parent, Some(comps));
    assert_eq!(s.app().doc.get(comps).unwrap().children.iter().position(|c| *c == new), Some(index), "in the replaced layer's place");
    assert_eq!((n.transform.tx, n.transform.ty), (tx, ty));
    assert_a_pasted_inside_b_is_valid(&mut s, b);
}

#[test]
fn dropping_a_main_from_assets_onto_a_main_it_would_make_hold_itself_lands_on_the_page() {
    let (mut s, _) = main_sim();
    let (a, b) = a_holds_an_instance_of_b(&mut s);
    s.shell.app.selection.clear();
    let at = center(&s, b);
    let id = s.shell.app.insert_instance(a, at).expect("the instance lands somewhere");
    assert_eq!(s.app().doc.get(id).unwrap().parent, Some(s.app().page), "on the page, not inside B");
    assert_a_pasted_inside_b_is_valid(&mut s, b);
}

#[test]
fn swapping_an_instance_in_b_to_a_main_that_holds_b_changes_nothing() {
    let (mut s, _) = main_sim();
    let (a, b) = a_holds_an_instance_of_b(&mut s);
    let comps = s.app().doc.get(a).unwrap().parent.unwrap();
    let z = add_main(&mut s, comps, DRect::new(1100.0, 0.0, 60.0, 60.0), "Z");
    let designos_doc::Command::Insert { nodes, .. } = designos_doc::create_instance(&mut s.shell.app.doc, z, b, DV::new(60.0, 60.0)) else {
        panic!("instance of Z")
    };
    let inside_b = nodes[0].id;
    designos_doc::Command::Insert { parent: b, index: 0, nodes }.apply(&mut s.shell.app.doc).unwrap();
    let before = s.app().doc.nodes.len();
    s.shell.app.swap_instance(inside_b, a);
    assert_eq!(instance_component(s.app().doc.get(inside_b).unwrap()), Some(z), "still an instance of Z");
    assert_eq!(s.app().doc.nodes.len(), before);
    edit_main(&mut s, b);
    s.frame(vec![]);
}

#[test]
fn a_layers_panel_drop_of_an_instance_of_a_into_b_is_refused_when_a_holds_b() {
    let (mut s, _) = main_sim();
    let (a, b) = a_holds_an_instance_of_b(&mut s);
    let host = add_frame(&mut s, b, DRect::new(50.0, 50.0, 40.0, 40.0), "Host");
    s.shell.app.selection.clear();
    let inst = s.shell.app.insert_instance(a, DV::new(5000.0, 5000.0)).expect("an instance of A on the page");
    let page = s.app().doc.get(inst).unwrap().parent;
    // what the panel does for a drop in the middle of B's row, and for one in a layer of B
    s.shell.app.move_to_parent(&[inst], b, 0);
    s.shell.app.move_to_parent(&[inst], host, 0);
    assert_eq!(s.app().doc.get(inst).unwrap().parent, page, "the instance stays where it was");
    edit_main(&mut s, b);
    s.frame(vec![]);
    // an ordinary drop still moves it
    let comps = s.app().doc.get(a).unwrap().parent.unwrap();
    let frame = add_frame(&mut s, comps, DRect::new(0.0, 900.0, 200.0, 200.0), "Frame");
    s.shell.app.move_to_parent(&[inst], frame, 0);
    assert_eq!(s.app().doc.get(inst).unwrap().parent, Some(frame));
}
