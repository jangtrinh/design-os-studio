//! Dragging a layer row reorders it however the window delivers the pointer events.
//!
//! egui starts a drag only on a frame after the press, and a release inside a frame drops the drag that was pending.
//! A slow frame or a quick flick therefore lets the move and the release land in the frame after the press, or puts
//! press, move and release into one frame, and the Layers panel saw neither a drag nor a click.

use super::*;

/// Row centres of the demo page, top-most layer first: Dashboard, Card, Welcome.
const X: f32 = 180.0;
const DASHBOARD_Y: f32 = 241.0;
const CARD_Y: f32 = 273.0;
const WELCOME_Y: f32 = 305.0;
/// 2 px below Dashboard's top edge: the "above" zone of a frame row.
const ABOVE_DASHBOARD_Y: f32 = DASHBOARD_Y - theme::LAYER_ROW_HEIGHT / 2.0 + 2.0;

fn page_order(s: &Sim) -> Vec<NodeId> {
    s.app().doc.get(s.app().page).expect("the page exists").children.clone()
}

fn card() -> Pos2 {
    Pos2::new(X, CARD_Y)
}

fn above_dashboard() -> Pos2 {
    Pos2::new(X, ABOVE_DASHBOARD_Y)
}

/// Card starts in the middle of the stack; after the drop it is the top-most layer.
fn assert_card_on_top(s: &Sim, before: &[NodeId]) {
    let card = s.by_name("Card");
    assert_ne!(page_order(s), before, "the layer order changed");
    assert_eq!(page_order(s).last(), Some(&card), "Card is the top-most layer now");
    assert_eq!(s.app().doc.get(card).unwrap().parent, Some(s.app().page), "Card stays on the page");
}

/// A fresh window, and the page order it starts with.
fn start() -> (Sim, Vec<NodeId>) {
    let s = Sim::new();
    let before = page_order(&s);
    (s, before)
}

#[test]
fn a_slow_drag_across_several_frames_still_reorders() {
    let (mut s, before) = start();
    s.drag(card(), above_dashboard());
    assert_card_on_top(&s, &before);
}

#[test]
fn press_move_and_release_in_one_frame_reorder() {
    let (mut s, before) = start();
    s.drag_in_one_frame(card(), above_dashboard());
    assert_card_on_top(&s, &before);
    s.cmd_key(Key::Z);
    assert_eq!(page_order(&s), before, "undo puts Card back in one step");
}

#[test]
fn a_press_in_its_own_frame_then_move_and_release_in_the_next_reorder() {
    let (mut s, before) = start();
    s.mv(card());
    s.button(true);
    s.one_frame().move_to(above_dashboard()).release().send();
    s.frame(vec![]);
    assert_card_on_top(&s, &before);
}

#[test]
fn press_and_move_in_one_frame_then_the_release_reorder() {
    let (mut s, before) = start();
    s.mv(card());
    s.one_frame().press().move_to(above_dashboard()).send();
    s.button(false);
    s.frame(vec![]);
    assert_card_on_top(&s, &before);
}

#[test]
fn the_release_frame_carrying_the_last_move_reorders() {
    let (mut s, before) = start();
    s.mv(card());
    s.button(true);
    s.mv(Pos2::new(X, (CARD_Y + ABOVE_DASHBOARD_Y) / 2.0));
    s.one_frame().move_to(above_dashboard()).release().send();
    s.frame(vec![]);
    assert_card_on_top(&s, &before);
}

#[test]
fn the_press_row_is_the_one_dragged_when_the_pointer_moved_on_within_the_frame() {
    // The pointer rests on Welcome, then presses Card and moves on in the same frame: the layer that moves is Card.
    let (mut s, before) = start();
    s.mv(Pos2::new(X, WELCOME_Y));
    s.one_frame().move_to(card()).press().move_to(above_dashboard()).release().send();
    s.frame(vec![]);
    assert_card_on_top(&s, &before);
}

#[test]
fn a_fast_drag_onto_the_middle_of_a_frame_row_reparents_into_it() {
    let mut s = Sim::new();
    let (welcome, card_id) = (s.by_name("Welcome"), s.by_name("Card"));
    s.drag_in_one_frame(card(), Pos2::new(X, WELCOME_Y));
    assert_eq!(s.app().doc.get(card_id).unwrap().parent, Some(welcome));
}

#[test]
fn a_fast_drag_selects_the_row_it_drags() {
    let (mut s, _) = start();
    s.drag_in_one_frame(card(), above_dashboard());
    assert_eq!(s.app().selection, vec![s.by_name("Card")]);
}

#[test]
fn a_click_without_movement_only_selects() {
    let (mut s, before) = start();
    s.click(card());
    assert_eq!(s.app().selection, vec![s.by_name("Card")]);
    assert_eq!(page_order(&s), before);
}

#[test]
fn a_press_and_release_in_one_frame_without_movement_only_selects() {
    let (mut s, before) = start();
    s.mv(card());
    s.one_frame().press().release().send();
    s.frame(vec![]);
    assert_eq!(s.app().selection, vec![s.by_name("Card")]);
    assert_eq!(page_order(&s), before);
}

#[test]
fn a_jitter_inside_a_click_in_one_frame_only_selects() {
    let (mut s, before) = start();
    s.mv(card());
    s.one_frame().press().move_to(card() + Vec2::new(3.0, 2.0)).release().send();
    s.frame(vec![]);
    assert_eq!(s.app().selection, vec![s.by_name("Card")]);
    assert_eq!(page_order(&s), before, "3 px is a click, not a drag");
}

#[test]
fn a_fast_drag_released_outside_the_list_changes_nothing() {
    let (mut s, before) = start();
    // Over the canvas: no drop target in the list.
    s.drag_in_one_frame(card(), Pos2::new(900.0, 500.0));
    assert_eq!(page_order(&s), before);
    assert!(s.app().layers.drag.is_none(), "the drag ended with the release");
}

#[test]
fn a_press_on_the_canvas_is_not_a_layer_drag() {
    let (mut s, before) = start();
    s.drag_in_one_frame(Pos2::new(900.0, 500.0), above_dashboard());
    assert_eq!(page_order(&s), before, "a press outside the list never drags a row");
}

#[test]
fn a_press_on_the_scrollbar_is_not_a_layer_drag() {
    use designos_doc::{Node, Rect as DRect};
    let (mut s, before_few) = start();
    let page = s.app().page;
    // Enough layers that the list scrolls and draws its scrollbar.
    for i in 0..40 {
        let id = s.shell.app.doc.alloc_id();
        s.shell.app.doc.insert(page, Node::rectangle(id, DRect::new(i as f32 * 4.0, 0.0, 10.0, 10.0))).expect("page takes the rectangle");
    }
    s.frame(vec![]);
    s.frame(vec![]);
    let before = page_order(&s);
    assert!(before.len() > before_few.len());
    let track = s.rect_of("scrollbar-track");
    // Press on the track and move down it, all in one frame: that is a scroll, and no row is picked up.
    s.drag_in_one_frame(track.center_top() + Vec2::new(0.0, 20.0), track.center_top() + Vec2::new(0.0, 80.0));
    assert_eq!(page_order(&s), before, "no layer moved");
    assert!(s.app().layers.drag.is_none());
}
