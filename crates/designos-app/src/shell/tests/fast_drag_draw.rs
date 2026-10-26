//! A drag with a shape tool draws the size the user dragged, however the window delivers the pointer events.
//!
//! egui reports only the last pointer position of a frame and calls a button released in that frame "not down".
//! A slow frame or a quick flick therefore puts press, moves and release into one frame, or the final move and the
//! release into the same one. Those gestures must draw the dragged shape, not the 100 x 100 click default.

use super::*;
use crate::canvas::Interaction;

/// Page-space ends of the drag. 180 x 70 is far from the 100 x 100 click default on both axes.
const FROM: (f32, f32) = (300.0, 800.0);
const TO: (f32, f32) = (480.0, 870.0);
const DRAGGED: (f32, f32) = (TO.0 - FROM.0, TO.1 - FROM.1);
const DEFAULT: f32 = crate::theme::DEFAULT_SHAPE_SIZE;
/// Snapping and pixel rounding may move a corner by a few pixels.
const SLACK: f32 = 6.0;

fn sim_with(tool: Tool) -> Sim {
    let mut s = Sim::new();
    s.shell.app.set_tool(tool);
    s
}

/// The node the last gesture left selected, as (world origin, size, rotation in radians).
fn drawn(s: &Sim) -> (DV, DV, f32) {
    let id = *s.app().selection.first().expect("the drawn shape is selected");
    let n = s.app().doc.get(id).expect("the drawn shape exists");
    let w = s.app().doc.world_transform(id);
    (DV::new(w.tx, w.ty), n.size, w.rotation_rad())
}

fn near(got: f32, want: f32, what: &str) {
    assert!((got - want).abs() <= SLACK, "{what}: got {got}, want {want} +- {SLACK}");
}

/// A line is as long as the drag; every other shape is as wide and tall as the drag.
fn assert_dragged_shape(s: &Sim, tool: Tool, count_before: usize) {
    assert_eq!(s.count(), count_before + 1, "one new shape");
    assert_eq!(s.app().tool, Tool::Move, "the tool is spent");
    let (origin, size, rot) = drawn(s);
    near(origin.x, FROM.0, "origin x");
    near(origin.y, FROM.1, "origin y");
    if tool == Tool::Line {
        near(size.x, (DRAGGED.0.powi(2) + DRAGGED.1.powi(2)).sqrt(), "line length");
        near(rot, DRAGGED.1.atan2(DRAGGED.0), "line angle");
    } else {
        near(size.x, DRAGGED.0, "width");
        near(size.y, DRAGGED.1, "height");
    }
}

fn ends(s: &Sim) -> (Pos2, Pos2) {
    (s.screen(FROM.0, FROM.1), s.screen(TO.0, TO.1))
}

/// Press, moves and release all in one frame (a quick flick).
fn flick(tool: Tool) {
    let mut s = sim_with(tool);
    let before = s.count();
    let (a, b) = ends(&s);
    s.drag_in_one_frame(a, b);
    assert_dragged_shape(&s, tool, before);
    s.cmd_key(Key::Z);
    assert_eq!(s.count(), before, "undo removes the shape");
}

/// The press has its own frame; the last move shares the frame of the release.
fn release_frame_carries_last_move(tool: Tool) {
    let mut s = sim_with(tool);
    let before = s.count();
    let (a, b) = ends(&s);
    s.mv(a);
    s.button(true);
    s.mv(a + (b - a) * 0.5);
    s.one_frame().move_to(b).release().send();
    s.frame(vec![]);
    assert_dragged_shape(&s, tool, before);
    s.cmd_key(Key::Z);
    assert_eq!(s.count(), before, "undo removes the shape");
}

/// The pointer moves to the press position and the button goes down in the same frame as the rest of the drag,
/// so the pointer is already far from where the press happened when the frame is drawn.
fn press_position_is_the_origin(tool: Tool) {
    let mut s = sim_with(tool);
    let before = s.count();
    let (a, b) = ends(&s);
    s.mv(Pos2::new(700.0, 300.0));
    s.one_frame().move_to(a).press().move_to(b).release().send();
    s.frame(vec![]);
    assert_dragged_shape(&s, tool, before);
}

macro_rules! drag_tests {
    ($($tool:ident => $flick:ident, $release:ident, $origin:ident;)*) => {$(
        #[test]
        fn $flick() {
            flick(Tool::$tool);
        }
        #[test]
        fn $release() {
            release_frame_carries_last_move(Tool::$tool);
        }
        #[test]
        fn $origin() {
            press_position_is_the_origin(Tool::$tool);
        }
    )*};
}

drag_tests! {
    Rectangle => rectangle_flick_in_one_frame_draws_the_dragged_size, rectangle_release_frame_carries_the_last_move, rectangle_origin_is_the_press_position;
    Ellipse => ellipse_flick_in_one_frame_draws_the_dragged_size, ellipse_release_frame_carries_the_last_move, ellipse_origin_is_the_press_position;
    Frame => frame_flick_in_one_frame_draws_the_dragged_size, frame_release_frame_carries_the_last_move, frame_origin_is_the_press_position;
    Line => line_flick_in_one_frame_draws_the_dragged_size, line_release_frame_carries_the_last_move, line_origin_is_the_press_position;
}

#[test]
fn flick_draws_the_dragged_size_with_keyboard_shortcut_tool() {
    let mut s = Sim::new();
    let before = s.count();
    s.key(Key::R);
    assert_eq!(s.app().tool, Tool::Rectangle);
    let (a, b) = ends(&s);
    s.drag_in_one_frame(a, b);
    assert_dragged_shape(&s, Tool::Rectangle, before);
}

fn assert_default_click(s: &Sim, tool: Tool, count_before: usize, at: (f32, f32)) {
    assert_eq!(s.count(), count_before + 1, "one new shape");
    let (origin, size, _) = drawn(s);
    near(origin.x, at.0, "origin x");
    near(origin.y, at.1, "origin y");
    near(size.x, DEFAULT, "width");
    if tool != Tool::Line {
        near(size.y, DEFAULT, "height");
    }
}

#[test]
fn a_click_with_a_shape_tool_still_makes_the_default_size() {
    for tool in [Tool::Rectangle, Tool::Ellipse, Tool::Frame, Tool::Line] {
        // one event per frame
        let mut s = sim_with(tool);
        let before = s.count();
        let p = s.screen(FROM.0, FROM.1);
        s.click(p);
        assert_default_click(&s, tool, before, FROM);

        // press and release in one frame
        let mut s = sim_with(tool);
        let before = s.count();
        let p = s.screen(FROM.0, FROM.1);
        s.one_frame().move_to(p).press().release().send();
        s.frame(vec![]);
        assert_default_click(&s, tool, before, FROM);

        // a jitter below the drag threshold is still a click
        let mut s = sim_with(tool);
        let before = s.count();
        let p = s.screen(FROM.0, FROM.1);
        s.one_frame().move_to(p).press().move_to(p + Vec2::new(1.0, 1.0)).release().send();
        s.frame(vec![]);
        assert_default_click(&s, tool, before, FROM);
    }
}

#[test]
fn a_press_while_a_draw_is_open_ends_the_draw_and_undo_removes_the_whole_shape() {
    let mut s = sim_with(Tool::Rectangle);
    let before = s.count();
    let (a, b) = ends(&s);
    s.mv(a);
    s.button(true);
    s.mv(b);
    assert!(matches!(s.app().canvas.inter, Interaction::Draw(_)), "the draw is open while the button is held");

    // the middle button goes down (a pan habit) while the left one is still held
    s.one_frame().button(PointerButton::Middle, true).send();
    assert!(!matches!(s.app().canvas.inter, Interaction::Draw(_)), "the draw is not left open");
    s.one_frame().button(PointerButton::Middle, false).send();
    s.button(false);
    s.frame(vec![]);
    assert!(matches!(s.app().canvas.inter, Interaction::None), "the gestures are over");
    assert_eq!(s.count(), before + 1, "the shape drawn so far stays");
    let (_, size, _) = drawn(&s);
    near(size.x, DRAGGED.0, "width");
    near(size.y, DRAGGED.1, "height");

    // one undo step takes the whole shape away
    assert!(s.app().hist.can_undo(), "the shape is in the undo history");
    s.cmd_key(Key::Z);
    assert_eq!(s.count(), before, "undo removes the whole shape");
    assert!(!s.app().hist.can_undo(), "the shape was one undo step");
    s.cmd_key(Key::Z);
    assert_eq!(s.count(), before);
    assert_eq!(s.app().tool, Tool::Move, "the tool is spent");
}

#[test]
fn a_middle_press_in_the_frame_of_the_last_move_draws_to_that_move_and_ends_the_draw() {
    let mut s = sim_with(Tool::Ellipse);
    let before = s.count();
    let (a, b) = ends(&s);
    s.mv(a);
    s.button(true);
    s.mv(a + (b - a) * 0.5);
    s.one_frame().move_to(b).button(PointerButton::Middle, true).send();
    assert!(!matches!(s.app().canvas.inter, Interaction::Draw(_)), "the draw is not left open");
    s.one_frame().button(PointerButton::Middle, false).release().send();
    s.frame(vec![]);
    assert_eq!(s.count(), before + 1);
    let (_, size, _) = drawn(&s);
    near(size.x, DRAGGED.0, "width");
    near(size.y, DRAGGED.1, "height");
    s.cmd_key(Key::Z);
    assert_eq!(s.count(), before, "undo removes the whole shape");
    assert!(!s.app().hist.can_undo());
}

// ---------------------------------------------------------------- order of events inside a frame

fn new_nodes(s: &Sim, known: &std::collections::HashSet<NodeId>) -> Vec<NodeId> {
    let mut ids: Vec<NodeId> = s.app().doc.nodes.keys().copied().filter(|id| !known.contains(id)).collect();
    ids.sort_by_key(|id| id.0);
    ids
}

fn all_nodes(s: &Sim) -> std::collections::HashSet<NodeId> {
    s.app().doc.nodes.keys().copied().collect()
}

#[test]
fn press_release_press_in_one_frame_leaves_the_second_draw_open() {
    let mut s = sim_with(Tool::Rectangle);
    let before = s.count();
    let (a, b) = ends(&s);
    s.mv(a);
    // the release happened BEFORE the last press: it ends nothing that is open now
    s.one_frame().press().release().press().send();
    assert!(matches!(s.app().canvas.inter, Interaction::Draw(_)), "the draw of the last press is open");
    assert!(!s.app().hist.can_undo(), "no shape is committed yet");
    assert!(s.ctx.input(|i| i.pointer.primary_down()), "the button is still down");

    // the real release comes after a drag
    s.mv(b);
    s.one_frame().release().send();
    s.frame(vec![]);
    assert_dragged_shape(&s, Tool::Rectangle, before);
    s.cmd_key(Key::Z);
    assert_eq!(s.count(), before, "undo removes the shape");
}

/// Two empty-canvas points at `b.y`, horizontally centered and mirrored across `canvas_rect` so they stay clear of
/// the drawn shape `a..b`, the demo frames above `a`, and the floating toolbars at the bottom of the canvas.
fn empty_canvas_points(s: &Sim, b: Pos2) -> (Pos2, Pos2) {
    let cr = s.app().canvas_rect;
    (Pos2::new(cr.center().x, b.y), Pos2::new(cr.right() - (b.x - cr.left()), b.y))
}

#[test]
fn release_before_press_in_one_frame_ends_the_open_draw_where_it_was_released() {
    let mut s = sim_with(Tool::Rectangle);
    s.shell.app.keep_tool = true; // the tool stays armed, so the new press draws as well
    let known = all_nodes(&s);
    let (a, b) = ends(&s);
    let (_, elsewhere) = empty_canvas_points(&s, b);
    s.mv(a);
    s.button(true);
    s.mv(a + (b - a) * 0.5);
    // the open draw is released at `b`, then a new press happens at `elsewhere`, all in one frame
    s.one_frame().move_to(b).release().move_to(elsewhere).press().send();
    assert!(matches!(s.app().canvas.inter, Interaction::Draw(_)), "the draw of the new press is open");
    assert!(s.ctx.input(|i| i.pointer.primary_down()), "the button is down again");
    s.one_frame().release().send();
    s.frame(vec![]);

    let ids = new_nodes(&s, &known);
    assert_eq!(ids.len(), 2, "two shapes: the dragged one and the click");
    let first = s.app().doc.get(ids[0]).unwrap();
    near(first.size.x, DRAGGED.0, "first width");
    near(first.size.y, DRAGGED.1, "first height");
    let second = s.app().doc.get(ids[1]).unwrap();
    near(second.size.x, DEFAULT, "second width");
    near(second.size.y, DEFAULT, "second height");
}

#[test]
fn the_open_draw_ends_at_the_first_release_of_a_frame_that_holds_several() {
    let mut s = sim_with(Tool::Rectangle);
    s.shell.app.keep_tool = true; // the tool stays armed, so the last press draws as well
    let known = all_nodes(&s);
    let (a, b) = ends(&s);
    let (c, d) = empty_canvas_points(&s, b);
    s.mv(a);
    s.button(true);
    s.mv(a + (b - a) * 0.5);
    // the open draw is released at `b`; a click at `c` and a press at `d` follow in the same frame
    s.one_frame().move_to(b).release().move_to(c).press().release().move_to(d).press().send();
    assert!(matches!(s.app().canvas.inter, Interaction::Draw(_)), "the draw of the last press is open");
    assert!(s.ctx.input(|i| i.pointer.primary_down()), "the button is down at the end of the frame");
    s.one_frame().release().send();
    s.frame(vec![]);

    // the click at `c` falls inside the frame and is not drawn: one gesture per frame is all that is supported
    let ids = new_nodes(&s, &known);
    assert_eq!(ids.len(), 2, "the dragged shape and the draw of the last press");
    let first = s.app().doc.get(ids[0]).unwrap();
    near(first.size.x, DRAGGED.0, "first width");
    near(first.size.y, DRAGGED.1, "first height");
    let second = s.app().doc.get(ids[1]).unwrap();
    near(second.size.x, DEFAULT, "second width");
    near(second.size.y, DEFAULT, "second height");
}

// ---------------------------------------------------------------- drag versus click uses the raw pointer

/// A rectangle whose top-left corner is the snap point the tests aim at, in page space.
const GUIDE: (f32, f32) = (500.0, 800.0);

fn sim_with_guide(tool: Tool) -> Sim {
    let mut s = sim_with(tool);
    let page = s.app().page;
    let id = s.shell.app.doc.alloc_id();
    let node = designos_doc::Node::rectangle(id, designos_doc::Rect::new(GUIDE.0, GUIDE.1, 60.0, 60.0));
    let mut tx = crate::edit::Tx::new();
    let _ = tx.cmd(&mut s.shell.app.doc, designos_doc::Command::Insert { parent: page, index: 0, nodes: vec![node] });
    s.shell.app.run_tx(tx);
    s.frame(vec![]);
    s.frame(vec![]);
    s
}

#[test]
fn an_eight_pixel_drag_across_a_snap_guide_is_a_drag_not_a_click() {
    for tool in [Tool::Rectangle, Tool::Ellipse, Tool::Frame] {
        // in one frame
        let mut s = sim_with_guide(tool);
        let before = s.count();
        let c = s.screen(GUIDE.0, GUIDE.1);
        s.one_frame().move_to(c + Vec2::new(-4.0, -4.0)).press().move_to(c + Vec2::new(4.0, 4.0)).release().send();
        s.frame(vec![]);
        assert_eq!(s.count(), before + 1, "{tool:?}: one new shape");
        let (_, size, _) = drawn(&s);
        assert!(size.x < DEFAULT / 2.0 && size.y < DEFAULT / 2.0, "{tool:?} in one frame: a drag across the guide, not the default: {size:?}");

        // one event per frame
        let mut s = sim_with_guide(tool);
        let before = s.count();
        let c = s.screen(GUIDE.0, GUIDE.1);
        s.drag(c + Vec2::new(-4.0, -4.0), c + Vec2::new(4.0, 4.0));
        assert_eq!(s.count(), before + 1, "{tool:?}: one new shape");
        let (_, size, _) = drawn(&s);
        assert!(size.x < DEFAULT / 2.0 && size.y < DEFAULT / 2.0, "{tool:?} over several frames: a drag across the guide, not the default: {size:?}");
    }
}

#[test]
fn a_jitter_next_to_a_snap_guide_is_still_a_click() {
    let mut s = sim_with_guide(Tool::Rectangle);
    let before = s.count();
    let c = s.screen(GUIDE.0, GUIDE.1);
    let p = c + Vec2::new(-3.0, -3.0);
    s.one_frame().move_to(p).press().move_to(p + Vec2::new(1.0, 1.0)).release().send();
    s.frame(vec![]);
    assert_default_click(&s, Tool::Rectangle, before, GUIDE);
}

// ---------------------------------------------------------------- the one-frame helper itself

#[test]
fn one_frame_reaches_egui_as_a_single_frame() {
    let mut s = Sim::new();
    let (a, b) = ends(&s);
    let frames = s.ctx.cumulative_frame_nr();
    s.one_frame().move_to(a).press().move_to(b).release().send();
    assert_eq!(s.ctx.cumulative_frame_nr(), frames + 1, "exactly one frame ran");
    let (pressed, released, down, last) =
        s.ctx.input(|i| (i.pointer.primary_pressed(), i.pointer.primary_released(), i.pointer.primary_down(), i.pointer.latest_pos()));
    assert!(pressed && released, "the press and the release are both reported");
    assert!(!down, "the button is not down after a release in the same frame");
    assert_eq!(last, Some(b), "only the last pointer position is reported");
    assert_eq!(s.pos, b);
}

#[test]
fn one_frame_delivers_keys_and_text_together() {
    let mut s = Sim::new();
    let frames = s.ctx.cumulative_frame_nr();
    s.one_frame().key(Key::R).send();
    assert_eq!(s.ctx.cumulative_frame_nr(), frames + 1);
    assert_eq!(s.app().tool, Tool::Rectangle, "a key down and up in one frame still acts");

    s.key(Key::Escape);
    s.shell.app.set_tool(Tool::Text);
    let p = s.screen(FROM.0, FROM.1);
    s.click(p);
    assert!(s.app().canvas.text.is_some(), "the text tool starts a text session");
    s.one_frame().text("hi").send();
    s.key(Key::Escape);
    let id = *s.app().selection.first().expect("the text node is selected");
    match &s.app().doc.get(id).unwrap().kind {
        NodeKind::Text(t) => assert_eq!(t.content, "hi"),
        k => panic!("unexpected {k:?}"),
    }
}
