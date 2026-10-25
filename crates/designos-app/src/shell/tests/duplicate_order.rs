//! A duplicate lands directly above its source, in the source's own parent, and is selected.
//! Paint order is the `children` order of the parent: the last child is drawn on top.

use super::*;
use crate::app::App;
use designos_doc::{Document, Node, Rect as DRect};

fn rect(d: &mut Document, parent: NodeId, name: &str) -> NodeId {
    let id = d.alloc_id();
    let mut n = Node::rectangle(id, DRect::new(0.0, 0.0, 40.0, 40.0));
    n.name = name.into();
    d.insert(parent, n).unwrap()
}

fn names(app: &App, parent: NodeId) -> Vec<String> {
    app.doc.get(parent).unwrap().children.iter().map(|c| app.doc.get(*c).unwrap().name.clone()).collect()
}

/// A page of rectangles `a b c d` (bottom to top) and a frame `f` on top of them holding `x y z`.
fn scene() -> (App, NodeId, NodeId, Vec<NodeId>, Vec<NodeId>) {
    let mut d = Document::new("t");
    let page = d.add_page("p");
    let top: Vec<NodeId> = ["a", "b", "c", "d"].iter().map(|n| rect(&mut d, page, n)).collect();
    let id = d.alloc_id();
    let frame = d.insert(page, Node::frame(id, DRect::new(200.0, 0.0, 300.0, 300.0))).unwrap();
    let inner: Vec<NodeId> = ["x", "y", "z"].iter().map(|n| rect(&mut d, frame, n)).collect();
    (App::new(d), page, frame, top, inner)
}

#[test]
fn the_copy_goes_directly_above_its_source_and_is_selected() {
    let (mut app, page, _frame, top, _) = scene();
    app.selection = vec![top[1]];
    app.duplicate_selection();
    assert_eq!(names(&app, page), ["a", "b", "b", "c", "d", "Frame"], "the copy of b sits right after b");
    let kids = &app.doc.get(page).unwrap().children;
    assert_eq!(kids[1], top[1], "b keeps its place");
    assert_eq!(app.selection, vec![kids[2]], "the copy is selected");
    assert_ne!(kids[2], top[1]);
}

#[test]
fn the_copy_of_the_top_layer_stays_on_top() {
    let (mut app, _page, frame, _, inner) = scene();
    app.selection = vec![inner[2]];
    app.duplicate_selection();
    assert_eq!(names(&app, frame), ["x", "y", "z", "z"]);
    assert_eq!(app.doc.get(frame).unwrap().children[3], app.selection[0]);
}

#[test]
fn a_nested_layer_is_copied_inside_its_own_parent() {
    let (mut app, page, frame, top, inner) = scene();
    app.selection = vec![inner[0]];
    app.duplicate_selection();
    assert_eq!(names(&app, frame), ["x", "x", "y", "z"]);
    assert_eq!(app.doc.get(page).unwrap().children.len(), top.len() + 1, "the page gained nothing");
    let copy = app.selection[0];
    assert_eq!(app.doc.get(copy).unwrap().parent, Some(frame));
}

#[test]
fn several_layers_each_get_their_copy_above_their_own_source() {
    for order in [[0usize, 2], [2, 0]] {
        let (mut app, page, _frame, top, _) = scene();
        app.selection = vec![top[order[0]], top[order[1]]];
        app.duplicate_selection();
        assert_eq!(names(&app, page), ["a", "a", "b", "c", "c", "d", "Frame"], "selection order {order:?}");
        let kids = app.doc.get(page).unwrap().children.clone();
        assert_eq!((kids[0], kids[2], kids[3], kids[5]), (top[0], top[1], top[2], top[3]), "sources keep their places");
        assert!(app.selection.contains(&kids[1]) && app.selection.contains(&kids[4]), "both copies are selected");
        assert_eq!(app.selection.len(), 2);
    }
}

#[test]
fn layers_in_different_parents_are_each_copied_in_their_own_parent() {
    let (mut app, page, frame, top, inner) = scene();
    app.selection = vec![top[3], inner[1]];
    app.duplicate_selection();
    assert_eq!(names(&app, page), ["a", "b", "c", "d", "d", "Frame"]);
    assert_eq!(names(&app, frame), ["x", "y", "y", "z"]);
}

#[test]
fn duplicating_is_one_undo_step() {
    let (mut app, page, _frame, top, _) = scene();
    app.selection = vec![top[0], top[2]];
    let before = app.doc.get(page).unwrap().children.clone();
    app.duplicate_selection();
    assert_eq!(app.doc.get(page).unwrap().children.len(), before.len() + 2);
    app.undo();
    assert_eq!(app.doc.get(page).unwrap().children, before, "one undo removes every copy");
}

#[test]
fn ctrl_d_places_the_copy_directly_above_the_source() {
    let mut s = Sim::with(Options { select: Some("Hero".into()), ..Default::default() });
    let hero = s.app().selection[0];
    let parent = s.app().doc.get(hero).unwrap().parent.unwrap();
    let before = s.app().doc.get(parent).unwrap().children.clone();
    assert_eq!(before.first(), Some(&hero), "Hero is the bottom layer of its frame");
    s.cmd_key(Key::D);
    let after = s.app().doc.get(parent).unwrap().children.clone();
    assert_eq!(after.len(), before.len() + 1);
    assert_eq!(after[0], hero, "Hero keeps its place");
    assert_eq!(s.app().selection, vec![after[1]], "the copy is selected, right above Hero");
    assert_eq!(after[2..], before[1..], "the layers that were above Hero stay above the copy");
    s.cmd_key(Key::Z);
    assert_eq!(s.app().doc.get(parent).unwrap().children, before, "one undo removes the copy");
}

#[test]
fn ctrl_d_pressed_again_stacks_each_copy_on_the_one_before() {
    let mut s = Sim::with(Options { select: Some("Hero".into()), ..Default::default() });
    let hero = s.app().selection[0];
    let parent = s.app().doc.get(hero).unwrap().parent.unwrap();
    s.cmd_key(Key::D);
    s.cmd_key(Key::D);
    s.cmd_key(Key::D);
    let kids = s.app().doc.get(parent).unwrap().children.clone();
    assert_eq!(kids[0], hero);
    assert_eq!(s.app().selection, vec![kids[3]], "the last copy is selected and is the highest of the four");
}

#[test]
fn alt_drag_places_the_copy_directly_above_the_source() {
    let mut s = Sim::with(Options { select: Some("Hero".into()), ..Default::default() });
    let hero = s.app().selection[0];
    let parent = s.app().doc.get(hero).unwrap().parent.unwrap();
    let before = s.app().doc.get(parent).unwrap().children.clone();
    s.mods = Modifiers::ALT;
    let a = s.screen(300.0, 150.0);
    s.drag(a, a + Vec2::new(40.0, 40.0));
    s.mods = Modifiers::NONE;
    let after = s.app().doc.get(parent).unwrap().children.clone();
    assert_eq!(after.len(), before.len() + 1, "the drag copied Hero");
    assert_eq!(after[0], hero);
    assert_eq!(s.app().selection, vec![after[1]]);
    assert_eq!(after[2..], before[1..]);
}
