//! Dragging a layer row reorders it however the window delivers the pointer events.
//!
//! egui starts a drag only on a frame after the press, and a release inside a frame drops the drag that was pending.
//! A slow frame or a quick flick therefore lets the move and the release land in the frame after the press, or puts
//! press, move and release into one frame, and the Layers panel saw neither a drag nor a click.

use super::*;

/// Pages of the demo document: the Pages list above the layers is one row per page (up to five).
const DEMO_PAGES: usize = 3;
/// Top of the Layers list in a fresh window: header, Pages title and list, the gap, the Layers header.
const LAYERS_LIST_TOP: f32 =
    theme::LEFT_HEADER_HEIGHT + theme::PANEL_TITLE_ROW_HEIGHT + DEMO_PAGES as f32 * theme::PAGE_ROW_HEIGHT + theme::SPACE_INLINE + theme::LAYER_HEADER_HEIGHT;
/// Row centres of the demo page, top-most layer first: Dashboard, Card, Welcome.
const X: f32 = 180.0;
const DASHBOARD_Y: f32 = LAYERS_LIST_TOP + theme::LAYER_ROW_HEIGHT / 2.0;
const CARD_Y: f32 = DASHBOARD_Y + theme::LAYER_ROW_HEIGHT;
const WELCOME_Y: f32 = CARD_Y + theme::LAYER_ROW_HEIGHT;
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
    assert_eq!(s.rect_of("layers-list").top(), LAYERS_LIST_TOP, "the row centres below follow the real Layers list");
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

/// A drag already under way over earlier frames; the final frame holds the release and then the pointer moving on, as
/// on a slow frame where the hand keeps going after the button came up.
fn drag_under_way_then_release_and_move_on(then: Pos2) -> (Sim, Vec<NodeId>) {
    let (mut s, before) = start();
    s.mv(card());
    s.button(true);
    s.mv(Pos2::new(X, (CARD_Y + ABOVE_DASHBOARD_Y) / 2.0));
    s.mv(above_dashboard());
    assert!(s.app().layers.drag.is_some(), "the drag is under way before the release frame");
    s.one_frame().release().move_to(then).send();
    s.frame(vec![]);
    (s, before)
}

#[test]
fn a_drag_under_way_drops_where_the_button_came_up_when_the_pointer_moves_on_to_the_canvas() {
    let (s, before) = drag_under_way_then_release_and_move_on(Pos2::new(900.0, 500.0));
    assert_card_on_top(&s, &before);
}

#[test]
fn a_drag_under_way_drops_where_the_button_came_up_when_the_pointer_moves_on_to_another_row() {
    let (s, before) = drag_under_way_then_release_and_move_on(Pos2::new(X, WELCOME_Y));
    assert_card_on_top(&s, &before);
}

#[test]
fn a_one_frame_drag_whose_pointer_moves_on_after_the_release_drops_at_the_release() {
    let (mut s, before) = start();
    s.one_frame().move_to(card()).press().move_to(above_dashboard()).release().move_to(Pos2::new(900.0, 500.0)).send();
    s.frame(vec![]);
    assert_card_on_top(&s, &before);
}

/// A scene with enough layers that the list scrolls: 40 rectangles on top of the demo's five.
fn with_many_rows() -> Sim {
    use designos_doc::{Node, Rect as DRect};
    let mut s = Sim::new();
    let page = s.app().page;
    for i in 0..40 {
        let id = s.shell.app.doc.alloc_id();
        s.shell.app.doc.insert(page, Node::rectangle(id, DRect::new(i as f32 * 4.0, 0.0, 10.0, 10.0))).expect("the page takes the rectangle");
    }
    s.frame(vec![]);
    s.frame(vec![]);
    s
}

/// Centre of list row `i` while the list is scrolled by `scroll` px; the list starts 16 px above the first row's centre.
fn row_centre(i: usize, scroll: f32) -> Pos2 {
    Pos2::new(X, DASHBOARD_Y + i as f32 * theme::LAYER_ROW_HEIGHT - scroll)
}

#[test]
fn a_press_that_shares_a_frame_with_a_scroll_picks_the_row_the_user_saw() {
    // Six trackpad deltas of 6 pt scroll the list 36 px in the frame of the press. The user pressed the row drawn at
    // that spot before the scroll, and a click and a drag from the same press must both take it.
    let wheel = || {
        (0..6).map(|_| Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: Vec2::new(0.0, -6.0),
            phase: egui::TouchPhase::Move,
            modifiers: Modifiers::NONE,
        })
    };
    let press = row_centre(5, 0.0) + Vec2::new(0.0, 10.0);
    let button = |pos, pressed| Event::PointerButton { pos, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE };

    let mut click = with_many_rows();
    click.mv(press);
    let seen = click.app().layers.rows_cache[5].id;
    click.frame(wheel().chain([button(press, true), button(press, false)]).collect());
    click.frame(vec![]);

    let mut drag = with_many_rows();
    drag.mv(press);
    let to = press + Vec2::new(0.0, 100.0);
    drag.frame(wheel().chain([button(press, true), Event::PointerMoved(to), button(to, false)]).collect());
    drag.frame(vec![]);

    assert_eq!(click.app().selection, vec![seen], "a click selects the row that was under the pointer");
    assert_eq!(drag.app().selection, vec![seen], "a drag picks up that same row");
}

/// Drags row `from` of a list scrolled by `scroll` px to the top half of row `to`, in one frame and over several frames.
/// Row `to` is above row `from`, so the dragged layer must end up above it.
fn scrolled_drag(scroll: f32, from: usize, to: usize, fast: bool) {
    let mut s = with_many_rows();
    s.shell.app.layers.scroll = scroll;
    s.frame(vec![]);
    s.frame(vec![]);
    let (moved, over) = (s.app().layers.rows_cache[from].id, s.app().layers.rows_cache[to].id);
    let (a, b) = (row_centre(from, scroll), row_centre(to, scroll) - Vec2::new(0.0, theme::LAYER_ROW_HEIGHT / 2.0 - 3.0));
    if fast {
        s.drag_in_one_frame(a, b);
    } else {
        s.drag(a, b);
    }
    let order = page_order(&s);
    let at = |id| order.iter().position(|x| *x == id).expect("the layer is still on the page");
    assert!(at(moved) > at(over), "scroll {scroll}, {} drag: the layer from row {from} is now above row {to}", if fast { "fast" } else { "slow" });
}

#[test]
fn a_fast_drag_on_a_scrolled_list_moves_the_row_under_the_press_to_the_row_under_the_release() {
    scrolled_drag(160.0, 8, 6, true);
}

#[test]
fn a_slow_drag_on_a_scrolled_list_moves_the_row_under_the_press_to_the_row_under_the_release() {
    scrolled_drag(160.0, 8, 6, false);
}

#[test]
fn a_fast_multi_selection_drag_moves_the_same_layers_as_a_slow_one() {
    let gesture = |fast: bool| {
        let (mut s, before) = start();
        s.click(card());
        s.mods = Modifiers::COMMAND;
        s.click(Pos2::new(X, WELCOME_Y));
        s.mods = Modifiers::NONE;
        assert_eq!(s.app().selection.len(), 2, "Card and Welcome are selected");
        if fast {
            s.drag_in_one_frame(card(), above_dashboard());
        } else {
            s.drag(card(), above_dashboard());
        }
        assert_eq!(s.app().selection.len(), 2, "both stay selected");
        (page_order(&s), before)
    };
    let ((fast, before), (slow, _)) = (gesture(true), gesture(false));
    assert_ne!(fast, before, "the layers moved");
    assert_eq!(fast, slow, "the fast drag does what the slow one does");
}

#[test]
fn a_fast_drag_while_renaming_never_reorders() {
    let (mut s, before) = start();
    s.click(card());
    s.shell.app.layers.renaming = Some(s.by_name("Card"));
    s.shell.app.layers.rename_started = false;
    s.frame(vec![]);
    s.frame(vec![]);
    s.mv(card());
    s.button(true);
    assert!(s.app().layers.press.is_some(), "the press is on a row");
    s.one_frame().move_to(above_dashboard()).release().send();
    s.frame(vec![]);
    assert_eq!(page_order(&s), before);
    assert!(s.app().layers.drag.is_none());
}

#[test]
fn a_fast_drag_while_the_name_filter_is_open_never_reorders() {
    let (mut s, before) = start();
    s.shell.app.layers.filter_open = true;
    s.shell.app.layers.filter = "d".into();
    s.frame(vec![]);
    s.frame(vec![]);
    let list_top = s.rect_of("layers-filter").bottom();
    let (a, b) = (Pos2::new(X, list_top + 16.0), Pos2::new(X, list_top + 16.0 + 2.0 * theme::LAYER_ROW_HEIGHT));
    s.mv(a);
    s.button(true);
    assert!(s.app().layers.press.is_some(), "the press is on a filtered row");
    s.one_frame().move_to(b).release().send();
    s.frame(vec![]);
    assert_eq!(page_order(&s), before);
    assert!(s.app().layers.drag.is_none());
}

#[test]
fn a_press_the_list_never_saw_released_is_forgotten_once_the_button_is_up() {
    let mut s = Sim::new();
    let (card_id, dashboard) = (s.by_name("Card"), s.by_name("Dashboard"));
    s.shell.app.layers.press = Some((card_id, card()));
    s.frame(vec![]);
    assert!(s.app().layers.press.is_none(), "no button is down, so no row is pressed");
    s.click(Pos2::new(X, DASHBOARD_Y));
    assert_eq!(s.app().selection, vec![dashboard], "a later click selects what it clicks on, not the old row");
}

#[test]
fn a_layer_deleted_while_the_button_is_held_is_not_dragged() {
    let (mut s, _) = start();
    let card_id = s.by_name("Card");
    s.mv(card());
    s.button(true);
    s.shell.app.doc.remove(card_id).expect("Card is removed");
    s.frame(vec![]);
    s.one_frame().move_to(above_dashboard()).release().send();
    s.frame(vec![]);
    assert!(!s.app().selection.contains(&card_id), "a deleted layer is not selected");
    assert!(s.app().layers.drag.is_none());
    assert!(s.app().toast.is_none(), "no failed-move toast");
}

/// A long drag of list row `from` (list at scroll 0) to `to`, over several frames, button still held. Returns the
/// dragged layer.
fn long_drag(s: &mut Sim, from: usize, to: Pos2) -> NodeId {
    let moved = s.app().layers.rows_cache[from].id;
    let a = row_centre(from, 0.0);
    s.mv(a);
    s.button(true);
    s.mv(a + (to - a) * 0.5);
    s.mv(to);
    assert!(s.app().layers.drag.is_some(), "the drag is under way");
    moved
}

/// The (parent, index) the drop indicator drawn on the last frame points at, for the dragged layer.
fn indicator_target(s: &Sim, moved: NodeId) -> (NodeId, usize) {
    let drop = s.app().layers.drop.expect("the last drawn frame shows a drop indicator");
    crate::panels::layers::resolve_drop(&s.app().doc, &s.app().layers.rows_cache, drop, &[moved]).expect("the indicator resolves")
}

fn landed(s: &Sim, id: NodeId) -> (NodeId, usize) {
    let parent = s.app().doc.get(id).unwrap().parent.unwrap();
    (parent, s.app().doc.get(parent).unwrap().children.iter().position(|c| *c == id).unwrap())
}

fn release_at(pos: Pos2) -> Event {
    Event::PointerButton { pos, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE }
}

/// 36 px of trackpad scroll, unsmoothed.
fn trackpad_scroll() -> Vec<Event> {
    (0..6)
        .map(|_| Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: Vec2::new(0.0, -6.0),
            phase: egui::TouchPhase::Move,
            modifiers: Modifiers::NONE,
        })
        .collect()
}

/// Layers has no auto-scroll, so a long drag reaches a row off screen by wheel; the drop must land on the indicator
/// the user aimed with, which was drawn at the scroll of the frame before the release.
#[test]
fn a_long_drag_released_without_a_scroll_lands_on_the_indicator() {
    let mut s = with_many_rows();
    let at = row_centre(10, 0.0) + Vec2::new(0.0, 6.0);
    let moved = long_drag(&mut s, 2, at);
    let expected = indicator_target(&s, moved);
    s.frame(vec![release_at(at)]);
    s.frame(vec![]);
    assert_eq!(landed(&s, moved), expected);
}

#[test]
fn a_long_drag_released_in_a_frame_that_also_scrolls_lands_on_the_indicator_the_user_saw() {
    let mut s = with_many_rows();
    let at = row_centre(10, 0.0) + Vec2::new(0.0, 6.0);
    let moved = long_drag(&mut s, 2, at);
    let expected = indicator_target(&s, moved);
    let mut events = trackpad_scroll();
    events.push(release_at(at));
    s.frame(events);
    s.frame(vec![]);
    assert_eq!(landed(&s, moved), expected, "scroll before the release");
}

#[test]
fn a_long_drag_released_before_a_scroll_in_the_same_frame_lands_on_the_indicator_the_user_saw() {
    let mut s = with_many_rows();
    let at = row_centre(10, 0.0) + Vec2::new(0.0, 6.0);
    let moved = long_drag(&mut s, 2, at);
    let expected = indicator_target(&s, moved);
    let mut events = vec![release_at(at)];
    events.extend(trackpad_scroll());
    s.frame(events);
    s.frame(vec![]);
    assert_eq!(landed(&s, moved), expected, "scroll after the release");
}

#[test]
fn a_long_drag_released_one_frame_after_a_mouse_wheel_notch_lands_on_the_indicator_the_user_saw() {
    // egui smooths a wheel notch over several frames, so the list is still scrolling in the frame of the release.
    let mut s = with_many_rows();
    let at = row_centre(10, 0.0) + Vec2::new(0.0, 6.0);
    let moved = long_drag(&mut s, 2, at);
    s.frame(vec![Event::MouseWheel {
        unit: egui::MouseWheelUnit::Line,
        delta: Vec2::new(0.0, -2.0),
        phase: egui::TouchPhase::Move,
        modifiers: Modifiers::NONE,
    }]);
    let expected = indicator_target(&s, moved);
    s.frame(vec![release_at(at)]);
    s.frame(vec![]);
    assert_eq!(landed(&s, moved), expected);
}

#[test]
fn a_click_release_then_a_press_and_move_in_one_frame_moves_nothing_while_the_button_is_held() {
    // A quick "click a row, then grab another" on a slow frame: the click's release, the new press and a move past the
    // click reach share one frame. The release belongs to the click, so the new drag has not ended.
    let (mut s, before) = start();
    s.mv(Pos2::new(X, WELCOME_Y));
    s.button(true);
    s.one_frame().release().move_to(card()).press().move_to(above_dashboard()).send();
    assert_eq!(page_order(&s), before, "no layer moves while the button is still held");
    // Letting go over the canvas cancels the drag.
    s.mv(Pos2::new(900.0, 500.0));
    s.button(false);
    s.frame(vec![]);
    assert_eq!(page_order(&s), before, "releasing outside the list drops nothing");
    assert!(s.app().layers.drag.is_none());
}

#[test]
fn a_press_the_list_never_saw_released_is_forgotten_when_the_list_skipped_frames() {
    let mut s = Sim::new();
    let card_id = s.by_name("Card");
    let was = landed(&s, card_id);
    s.mv(card());
    s.button(true);
    assert!(s.app().layers.press.is_some(), "the list saw the press on Card");
    // The sidebar goes away while the button is held and the release is never seen.
    s.shell.app.ui.left_collapsed = true;
    s.frame(vec![]);
    s.button(false);
    // A later gesture is held while the sidebar comes back; its pointer is far from the old press.
    s.mv(Pos2::new(900.0, 500.0));
    s.button(true);
    s.shell.app.ui.left_collapsed = false;
    s.mv(above_dashboard());
    assert!(s.app().layers.press.is_none(), "the old press is forgotten");
    assert!(s.app().layers.drag.is_none(), "no layer drag starts from the old press");
    s.button(false);
    s.frame(vec![]);
    // (The canvas drag may create a node of its own, so only Card is compared.)
    assert_eq!(landed(&s, card_id), was, "Card stays where it was");
}

#[test]
fn a_layer_that_is_no_longer_on_the_list_is_not_dragged() {
    // The page changes while the button is held (a keyboard page switch): the pressed layer is on another page.
    let mut s = Sim::new();
    let card_id = s.by_name("Card");
    s.mv(card());
    s.button(true);
    assert!(s.app().layers.press.is_some());
    s.shell.app.add_page().expect("a page is added and shown");
    s.frame(vec![]);
    s.one_frame().move_to(Pos2::new(X, DASHBOARD_Y)).release().send();
    s.frame(vec![]);
    assert!(!s.app().selection.contains(&card_id), "a layer of another page is not selected");
    assert!(s.app().layers.drag.is_none());
}

#[test]
fn a_drag_the_list_lost_sight_of_is_forgotten() {
    // The sidebar goes away mid-drag and comes back while the button is still held; letting go over a row then drops
    // nothing, because the drag it would drop is from before the list was hidden.
    let mut s = with_many_rows();
    let before = page_order(&s);
    let at = row_centre(10, 0.0) + Vec2::new(0.0, 6.0);
    long_drag(&mut s, 2, at);
    s.shell.app.ui.left_collapsed = true;
    s.frame(vec![]);
    s.shell.app.ui.left_collapsed = false;
    s.frame(vec![]);
    assert!(s.app().layers.drag.is_none(), "the drag did not survive the pass the list skipped");
    s.frame(vec![release_at(at)]);
    s.frame(vec![]);
    assert_eq!(page_order(&s), before);
}
