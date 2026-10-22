//! Move, resize and rotate end where the pointer let go, however the window delivers the pointer events.
//!
//! egui reports only the last pointer position of a frame and calls a button released in that frame "not down".
//! On a slow frame the press, the moves and the release land in one frame, or the last move and the release share
//! one. `on_drag` does not run on the frame of the release, so those gestures must follow the pointer to the position
//! of the release themselves, or the layer stops short (or a drag turns into a click).
//!
//! Every gesture is compared with the same gesture spread over many frames (`Sim::drag`), which ends where the
//! pointer ended and is the behaviour users get on a fast machine.

use super::*;
use crate::canvas::handles::{self, Handle};
use crate::canvas::Interaction;

#[derive(Clone, Copy, Debug)]
enum Kind {
    Move,
    /// The press selects the layer first, then the drag moves it.
    MoveUnselected,
    /// Lands a few pixels short of a snap target, which pulls it in flush.
    MoveToSnap,
    ResizeBottomRightCorner,
    ResizeTopLeftCorner,
    ResizeRightEdge,
    ResizeTopEdge,
    /// Lands a few pixels short of a snap target, which pulls the edge in flush.
    ResizeEdgeToSnap,
    Rotate,
    RotateGroup,
}

const ALL: [Kind; 10] = [
    Kind::Move,
    Kind::MoveUnselected,
    Kind::MoveToSnap,
    Kind::ResizeBottomRightCorner,
    Kind::ResizeTopLeftCorner,
    Kind::ResizeRightEdge,
    Kind::ResizeTopEdge,
    Kind::ResizeEdgeToSnap,
    Kind::Rotate,
    Kind::RotateGroup,
];

/// How the events of one gesture reach egui.
#[derive(Clone, Copy, Debug)]
enum Delivery {
    /// Press, moves and release in ONE frame (a quick flick).
    OneFrame,
    /// The press has its own frame and so does the first move; the last move shares the frame of the release.
    ReleaseFrame,
    /// The pointer is far away from the press position when the frame that holds the press is drawn.
    PointerElsewhere,
}

const DELIVERIES: [Delivery; 3] = [Delivery::OneFrame, Delivery::ReleaseFrame, Delivery::PointerElsewhere];

/// One gesture on a prepared window: the layers it works on and the screen positions of its press and release.
struct Case {
    kind: Kind,
    sim: Sim,
    ids: Vec<NodeId>,
    from: Pos2,
    to: Pos2,
}

/// A layer's transform and size: `[a, b, c, d, tx, ty, w, h]`.
type Shape = [f32; 8];

/// Page-space point inside Hero (360 x 240 at 120, 120), well clear of every handle.
const INSIDE_HERO: (f32, f32) = (300.0, 200.0);
/// Empty canvas: nothing is there to select, snap to, or drop into.
const EMPTY: (f32, f32) = (700.0, 950.0);

fn hero_selected() -> Sim {
    Sim::with(Options { select: Some("Hero".into()), ..Default::default() })
}

fn handle_at(sim: &Sim, handle: Handle) -> Pos2 {
    let (corners, _) = interact::selection_box(sim.app()).expect("the selection has a box");
    sim.app().view.to_screen(sim.app().canvas_rect.min, handles::handle_pos(&corners, handle))
}

/// A quarter turn about the middle of the selection, grabbed just outside the bottom-right corner.
fn quarter_turn(sim: &Sim) -> (Pos2, Pos2) {
    let (corners, _) = interact::selection_box(sim.app()).expect("the selection has a box");
    let to_screen = |p: DV| sim.app().view.to_screen(sim.app().canvas_rect.min, p);
    let start = to_screen(corners[2]) + Vec2::new(5.0, 5.0);
    let center = to_screen(DV::new((corners[0].x + corners[2].x) / 2.0, (corners[0].y + corners[2].y) / 2.0));
    let rel = start - center;
    (start, center + Vec2::new(-rel.y, rel.x))
}

impl Case {
    fn new(kind: Kind) -> Self {
        let mut sim = hero_selected();
        let hero = sim.app().selection[0];
        let page = |sim: &Sim, p: (f32, f32)| sim.screen(p.0, p.1);
        let (ids, from, to) = match kind {
            Kind::Move => (vec![hero], page(&sim, INSIDE_HERO), page(&sim, (350.0, 235.0))),
            Kind::MoveUnselected => {
                sim.shell.app.clear_selection();
                sim.frame(vec![]);
                (vec![hero], page(&sim, INSIDE_HERO), page(&sim, (350.0, 235.0)))
            }
            // the left edge of Hero would land at 142, two pixels from the left edge of the layers below it (144)
            Kind::MoveToSnap => (vec![hero], page(&sim, INSIDE_HERO), page(&sim, (INSIDE_HERO.0 + 22.0, INSIDE_HERO.1))),
            Kind::ResizeBottomRightCorner => {
                let a = handle_at(&sim, Handle::BR);
                (vec![hero], a, a + Vec2::new(-60.0, -40.0))
            }
            Kind::ResizeTopLeftCorner => {
                let a = handle_at(&sim, Handle::TL);
                (vec![hero], a, a + Vec2::new(40.0, 30.0))
            }
            Kind::ResizeRightEdge => {
                let a = handle_at(&sim, Handle::R);
                (vec![hero], a, a + Vec2::new(-70.0, 0.0))
            }
            Kind::ResizeTopEdge => {
                let a = handle_at(&sim, Handle::T);
                (vec![hero], a, a + Vec2::new(0.0, 45.0))
            }
            // the right edge of Hero (480) is dragged to 458, two pixels from the right edge of the Button (456)
            Kind::ResizeEdgeToSnap => (vec![hero], handle_at(&sim, Handle::R), page(&sim, (458.0, 240.0))),
            Kind::Rotate => {
                let (a, b) = quarter_turn(&sim);
                (vec![hero], a, b)
            }
            Kind::RotateGroup => {
                let (a, b) = (sim.by_name("Tile A"), sim.by_name("Tile B"));
                sim.shell.app.selection = vec![a, b];
                sim.frame(vec![]);
                let (from, to) = quarter_turn(&sim);
                (vec![a, b], from, to)
            }
        };
        Case { kind, sim, ids, from, to }
    }

    fn shape(&self) -> Vec<Shape> {
        self.ids
            .iter()
            .map(|id| {
                let n = self.sim.app().doc.get(*id).expect("the layer exists");
                let t = n.transform;
                [t.a, t.b, t.c, t.d, t.tx, t.ty, n.size.x, n.size.y]
            })
            .collect()
    }

    /// True while the interaction this case starts is open.
    fn is_open(&self) -> bool {
        let inter = &self.sim.app().canvas.inter;
        match self.kind {
            Kind::Move | Kind::MoveUnselected | Kind::MoveToSnap => matches!(inter, Interaction::Move(_)),
            Kind::Rotate | Kind::RotateGroup => matches!(inter, Interaction::Rotate(_)),
            _ => matches!(inter, Interaction::Resize(_)),
        }
    }

    fn undo_depth(&self) -> usize {
        self.sim.app().hist.undo_depth()
    }

    fn world_x(&self) -> f32 {
        self.sim.app().doc.world_transform(self.ids[0]).tx
    }

    fn width(&self) -> f32 {
        self.sim.app().doc.get(self.ids[0]).expect("the layer exists").size.x
    }

    /// Runs the gesture the way `how` says and lets the window settle.
    fn run(&mut self, how: Delivery) {
        let (a, b) = (self.from, self.to);
        let s = &mut self.sim;
        match how {
            Delivery::OneFrame => s.drag_in_one_frame(a, b),
            Delivery::ReleaseFrame => {
                s.mv(a);
                s.button(true);
                s.mv(a + (b - a) * 0.5);
                s.one_frame().move_to(b).release().send();
                s.frame(vec![]);
            }
            Delivery::PointerElsewhere => {
                s.mv(Pos2::new(900.0, 300.0));
                s.one_frame().move_to(a).press().move_to(b).release().send();
                s.frame(vec![]);
            }
        }
    }

    /// Presses at `from` and follows the pointer to `to`, leaving the gesture open.
    fn open_at_the_end(&mut self) {
        let (a, b) = (self.from, self.to);
        self.sim.mv(a);
        self.sim.button(true);
        self.sim.mv(a + (b - a) * 0.5);
        self.sim.mv(b);
        assert!(self.is_open(), "{:?}: the gesture is open while the button is held", self.kind);
    }
}

fn same(a: &[Shape], b: &[Shape]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.iter().zip(y).all(|(p, q)| (p - q).abs() < 1e-3))
}

fn assert_same(got: &[Shape], want: &[Shape], what: &str) {
    assert!(same(got, want), "{what}\n   got {got:?}\n  want {want:?}");
}

/// The layers before the gesture, and after the same gesture spread over many frames.
fn reference(kind: Kind) -> (Vec<Shape>, Vec<Shape>) {
    let mut c = Case::new(kind);
    let before = c.shape();
    c.sim.drag(c.from, c.to);
    let after = c.shape();
    assert!(!same(&before, &after), "{kind:?}: the gesture changes the layers, or the comparison proves nothing");
    (before, after)
}

#[test]
fn a_gesture_ends_at_the_release_position_however_the_frames_are_cut() {
    for kind in ALL {
        let (before, want) = reference(kind);
        for how in DELIVERIES {
            let mut c = Case::new(kind);
            c.run(how);
            assert_same(&c.shape(), &want, &format!("{kind:?} {how:?}: ends where the pointer let go"));
            assert!(!matches!(c.sim.app().canvas.inter, Interaction::Move(_) | Interaction::Resize(_) | Interaction::Rotate(_)));
            assert_eq!(c.undo_depth(), 1, "{kind:?} {how:?}: one undo step");
            c.sim.cmd_key(Key::Z);
            assert_same(&c.shape(), &before, &format!("{kind:?} {how:?}: undo restores the layers"));
            assert!(!c.sim.app().hist.can_undo(), "{kind:?} {how:?}: the gesture was one undo step");
        }
    }
}

#[test]
fn snapping_applies_to_a_gesture_in_one_frame_as_it_does_to_a_slow_drag() {
    // 22 px of drag, snapped to 24 (Hero's left edge flush with the layers below); a 22 px wide pull of the right
    // edge, snapped so it ends flush with the Button at 456.
    for (kind, flush) in [(Kind::MoveToSnap, 144.0), (Kind::ResizeEdgeToSnap, 336.0)] {
        let read = |c: &Case| if matches!(kind, Kind::MoveToSnap) { c.world_x() } else { c.width() };
        let mut slow = Case::new(kind);
        slow.sim.drag(slow.from, slow.to);
        assert!((read(&slow) - flush).abs() < 0.01, "{kind:?}: a slow drag snaps to {flush}, got {}", read(&slow));
        for how in DELIVERIES {
            let mut c = Case::new(kind);
            c.run(how);
            assert!((read(&c) - flush).abs() < 0.01, "{kind:?} {how:?}: snaps to {flush} like a slow drag, got {}", read(&c));
        }
    }
}

#[test]
fn a_move_drops_into_the_container_under_the_release_not_under_the_pointer_after_it() {
    let mut s = Sim::new();
    let tile = s.by_name("Tile A");
    let board = s.app().doc.get(tile).and_then(|n| n.parent).expect("the tile sits in the board");
    let welcome = s.app().doc.get(s.by_name("Hero")).and_then(|n| n.parent).expect("Hero sits in Welcome");
    s.click(s.screen(600.0, 400.0));
    assert_eq!(s.app().selection, vec![tile]);
    let (a, inside_welcome, elsewhere) = (s.screen(600.0, 400.0), s.screen(300.0, 500.0), s.screen(EMPTY.0, EMPTY.1));
    s.mv(a);
    s.button(true);
    s.mv(a + (inside_welcome - a) * 0.5);
    // the button goes up over Welcome, and the pointer moves on over the bare canvas within the same frame
    s.one_frame().move_to(inside_welcome).release().move_to(elsewhere).send();
    s.frame(vec![]);
    assert_eq!(s.app().doc.get(tile).unwrap().parent, Some(welcome), "dropped where the button went up");
    s.cmd_key(Key::Z);
    assert_eq!(s.app().doc.get(tile).unwrap().parent, Some(board), "undo puts it back");
}

// ---------------------------------------------------------------- a press ends the gesture that is open

#[test]
fn a_release_then_a_press_in_one_frame_commits_the_open_gesture_where_it_was_released() {
    for kind in ALL {
        let (before, want) = reference(kind);
        let mut c = Case::new(kind);
        c.open_at_the_end();
        // the pointer is back at the middle of the drag when the frame starts; the release happens at `to` and the
        // next press somewhere else
        let (b, elsewhere) = (c.to, c.sim.screen(EMPTY.0, EMPTY.1));
        c.sim.mv(c.from + (b - c.from) * 0.5);
        c.sim.one_frame().move_to(b).release().move_to(elsewhere).press().send();
        assert!(!c.is_open(), "{kind:?}: the new press does not continue the old gesture");
        assert_eq!(c.undo_depth(), 1, "{kind:?}: the first gesture is in the undo history");
        assert_same(&c.shape(), &want, &format!("{kind:?}: the gesture ended where the button went up"));

        c.sim.one_frame().release().send();
        c.sim.frame(vec![]);
        c.sim.cmd_key(Key::Z);
        assert_same(&c.shape(), &before, &format!("{kind:?}: one undo restores the layers"));
        assert!(!c.sim.app().hist.can_undo(), "{kind:?}: the gesture was one undo step");
    }
}

#[test]
fn a_press_at_the_spot_of_the_release_does_not_move_the_end_of_the_open_gesture() {
    for kind in ALL {
        let (_, want) = reference(kind);
        let mut c = Case::new(kind);
        c.open_at_the_end();
        let b = c.to;
        c.sim.mv(c.from + (b - c.from) * 0.5);
        // release and press at the very same position: the press is no click of the old gesture
        c.sim.one_frame().move_to(b).release().press().send();
        assert_eq!(c.undo_depth(), 1, "{kind:?}: the first gesture is in the undo history");
        assert_same(&c.shape(), &want, &format!("{kind:?}: the gesture ended at the release, not where it was last drawn"));
    }
}

#[test]
fn a_press_off_the_canvas_after_a_release_still_ends_the_open_gesture() {
    for kind in ALL {
        let (_, want) = reference(kind);
        let mut c = Case::new(kind);
        c.open_at_the_end();
        let b = c.to;
        let rect = c.sim.app().canvas_rect;
        let on_a_panel = Pos2::new(rect.min.x - 20.0, rect.center().y);
        c.sim.mv(c.from + (b - c.from) * 0.5);
        // the button went up at `b`; the next press lands on a panel, which never reaches the canvas
        c.sim.one_frame().move_to(b).release().move_to(on_a_panel).press().send();
        assert!(!c.is_open(), "{kind:?}: the gesture is not left open to follow the held button over the panel");
        assert_eq!(c.undo_depth(), 1, "{kind:?}: the gesture is in the undo history");
        assert_same(&c.shape(), &want, &format!("{kind:?}: the gesture ended where the button went up"));
    }
}

#[test]
fn a_press_after_a_lost_release_ends_the_gesture_as_it_was_last_drawn() {
    for kind in ALL {
        let (before, want) = reference(kind);
        let mut c = Case::new(kind);
        c.open_at_the_end();
        // no release ever arrived; the next press is somewhere that has nothing to do with the gesture
        let elsewhere = c.sim.screen(EMPTY.0, EMPTY.1);
        c.sim.one_frame().move_to(elsewhere).press().send();
        assert!(!c.is_open(), "{kind:?}: the new press does not continue the old gesture");
        assert_eq!(c.undo_depth(), 1, "{kind:?}: the gesture is in the undo history");
        assert_same(&c.shape(), &want, &format!("{kind:?}: the gesture stays where it was last drawn"));

        c.sim.one_frame().release().send();
        c.sim.frame(vec![]);
        c.sim.cmd_key(Key::Z);
        assert_same(&c.shape(), &before, &format!("{kind:?}: one undo restores the layers"));
        assert!(!c.sim.app().hist.can_undo(), "{kind:?}: the gesture was one undo step");
    }
}

#[test]
fn a_middle_press_ends_the_open_gesture_in_one_undo_step() {
    for kind in ALL {
        let (before, want) = reference(kind);
        let mut c = Case::new(kind);
        c.open_at_the_end();
        // a pan habit: the middle button goes down while the left one is still held
        c.sim.one_frame().button(PointerButton::Middle, true).send();
        assert!(!c.is_open(), "{kind:?}: the gesture is not left open");
        assert_eq!(c.undo_depth(), 1, "{kind:?}: the gesture is in the undo history");
        assert_same(&c.shape(), &want, &format!("{kind:?}: the gesture stays where the pointer was"));

        c.sim.one_frame().button(PointerButton::Middle, false).release().send();
        c.sim.frame(vec![]);
        assert!(matches!(c.sim.app().canvas.inter, Interaction::None), "{kind:?}: the gestures are over");
        // Not the chord: egui reads the middle click plus the release at one spot as a double click, and over a
        // text layer that opens a text session, which keeps the keyboard.
        c.sim.shell.app.undo();
        assert_same(&c.shape(), &before, &format!("{kind:?}: one undo restores the layers"));
        assert!(!c.sim.app().hist.can_undo(), "{kind:?}: the gesture was one undo step");
    }
}

// ---------------------------------------------------------------- a click stays a click

/// A spot that hits what the case grabs, 5.6 px from the exact handle: inside the 6 px hit radius, outside the 5 px
/// snap distance. A resize that followed the pointer to the click would visibly change the layer.
fn off_centre(c: &Case) -> Pos2 {
    match c.kind {
        Kind::ResizeTopEdge => c.from + Vec2::new(0.0, 5.6),
        Kind::ResizeBottomRightCorner | Kind::ResizeTopLeftCorner | Kind::ResizeRightEdge | Kind::ResizeEdgeToSnap => c.from + Vec2::new(5.6, 0.0),
        Kind::Rotate | Kind::RotateGroup => c.from + Vec2::new(2.0, 2.0),
        Kind::Move | Kind::MoveUnselected | Kind::MoveToSnap => c.from,
    }
}

#[test]
fn a_press_and_release_in_one_frame_without_movement_is_a_click() {
    for kind in ALL {
        let mut c = Case::new(kind);
        let before = c.shape();
        let p = off_centre(&c);
        c.sim.one_frame().move_to(p).press().release().send();
        c.sim.frame(vec![]);
        assert_same(&c.shape(), &before, &format!("{kind:?}: a click changes nothing"));
        assert_eq!(c.undo_depth(), 0, "{kind:?}: a click adds no undo step");
        assert!(matches!(c.sim.app().canvas.inter, Interaction::None), "{kind:?}: the click is over");
    }
}

#[test]
fn a_jitter_below_the_drag_threshold_is_still_a_click() {
    for kind in [Kind::Move, Kind::MoveUnselected] {
        let mut c = Case::new(kind);
        let before = c.shape();
        let p = c.from;
        c.sim.one_frame().move_to(p).press().move_to(p + Vec2::new(1.0, 1.0)).release().send();
        c.sim.frame(vec![]);
        assert_same(&c.shape(), &before, &format!("{kind:?}: a jitter moves nothing"));
        assert_eq!(c.undo_depth(), 0, "{kind:?}: a jitter adds no undo step");
        assert_eq!(c.sim.app().selection, c.ids, "{kind:?}: the click selects the layer");
    }
}
