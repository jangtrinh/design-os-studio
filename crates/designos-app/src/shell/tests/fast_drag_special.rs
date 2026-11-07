//! Drags that are not a move, resize or rotate (corner radius, auto layout spacing, smart selection, shape controls,
//! ruler guides, pencil strokes, connectors and vector edits) end where the pointer let go when a slow frame holds the
//! release AND the next press.
//!
//! The new press must commit the open drag as one undo step before it starts its own gesture. Each gesture is compared
//! with the same drag spread over many frames (`Sim::drag`), which ends where the pointer ended.

use super::*;
use crate::canvas::{handles, interact, Interaction};
use designos_doc::{AutoLayout, FrameProps, LayoutDirection, Node, Rect as DocRect, Sizing};

/// Bare canvas: nothing sits there, so a press lands on empty space.
const EMPTY: (f32, f32) = (700.0, 850.0);

/// Reads the values a drag changes.
type Reader = Box<dyn Fn(&Sim) -> Vec<f32>>;

/// One drag on a prepared window.
struct Gesture {
    name: &'static str,
    sim: Sim,
    from: Pos2,
    to: Pos2,
    /// The values the drag changes.
    read: Reader,
    /// True while the interaction this drag starts is open.
    open: fn(&Interaction) -> bool,
    /// A press on empty canvas starts a drag of the same kind (a marquee in vector edit mode), so the interaction is
    /// open again after the new press.
    press_reopens: bool,
}

impl Gesture {
    fn values(&self) -> Vec<f32> {
        (self.read)(&self.sim)
    }

    fn is_open(&self) -> bool {
        (self.open)(&self.sim.app().canvas.inter)
    }

    /// Presses at `from` and follows the pointer to `to`, leaving the drag open.
    fn open_at_the_end(&mut self) {
        let (a, b) = (self.from, self.to);
        self.sim.mv(a);
        self.sim.button(true);
        self.sim.mv(a + (b - a) * 0.5);
        self.sim.mv(b);
        assert!(self.is_open(), "{}: the drag is open while the button is held", self.name);
    }
}

fn same(a: &[f32], b: &[f32]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-3)
}

fn screen_of(sim: &Sim, p: DV) -> Pos2 {
    sim.app().view.to_screen(sim.app().canvas_rect.min, p)
}

fn dv_of(p: Pos2) -> DV {
    DV::new(p.x, p.y)
}

/// Values of the nodes created after `floor`, in id order: size and place.
fn fresh_nodes(sim: &Sim, floor: u64) -> Vec<f32> {
    let mut nodes: Vec<_> = sim.app().doc.nodes.values().filter(|n| n.id.0 > floor).collect();
    nodes.sort_by_key(|n| n.id.0);
    nodes.iter().flat_map(|n| [n.size.x, n.size.y, n.transform.tx, n.transform.ty]).collect()
}

fn newest_id(sim: &Sim) -> u64 {
    sim.app().doc.nodes.values().map(|n| n.id.0).max().unwrap_or(0)
}

// ---------------------------------------------------------------- the scenes

fn corner_radius() -> Gesture {
    let sim = Sim::with(Options { select: Some("Hero".into()), ..Default::default() });
    let hero = sim.app().selection[0];
    let (corners, _) = interact::selection_box(sim.app()).expect("the selection has a box");
    let screen = corners.map(|c| dv_of(screen_of(&sim, c)));
    let radii = sim.app().doc.get(hero).unwrap().corner_radii;
    let handle = handles::corner_radius_handle_positions(&screen, radii, sim.app().view.zoom).expect("the corner handles are shown")[0];
    let from = Pos2::new(handle.x, handle.y);
    Gesture {
        name: "corner radius",
        sim,
        from,
        to: from + Vec2::new(24.0, 24.0),
        read: Box::new(move |s| s.app().doc.get(hero).unwrap().corner_radii.to_vec()),
        open: |i| matches!(i, Interaction::CornerRadius(_)),
        press_reopens: false,
    }
}

fn auto_layout_spacing() -> Gesture {
    let mut sim = Sim::new();
    let page = sim.app().page;
    let fid = sim.shell.app.doc.alloc_id();
    let mut f = Node::frame(fid, DocRect::new(520.0, 650.0, 220.0, 120.0));
    f.kind = NodeKind::Frame(FrameProps {
        auto_layout: Some(AutoLayout {
            direction: LayoutDirection::Horizontal,
            item_spacing: 30.0,
            padding: [20.0; 4],
            primary_sizing: Sizing::Fixed,
            counter_sizing: Sizing::Fixed,
            ..AutoLayout::default()
        }),
        ..FrameProps::default()
    });
    let f = sim.shell.app.doc.insert(page, f).unwrap();
    for _ in 0..2 {
        let cid = sim.shell.app.doc.alloc_id();
        sim.shell.app.doc.insert(f, Node::rectangle(cid, DocRect::new(0.0, 0.0, 60.0, 40.0))).unwrap();
    }
    sim.shell.app.doc.relayout_subtree(f);
    sim.shell.app.selection = vec![f];
    sim.frame(vec![]);
    sim.frame(vec![]);

    let n = sim.app().doc.get(f).unwrap();
    let al = n.kind.frame_props().and_then(|fp| fp.auto_layout.as_ref()).unwrap();
    let child_rects: Vec<_> = n
        .children
        .iter()
        .map(|c| {
            let c = sim.app().doc.get(*c).unwrap();
            DocRect::new(c.transform.tx, c.transform.ty, c.size.x, c.size.y)
        })
        .collect();
    let hnds = handles::auto_layout_handles(al, n.size, &child_rects, sim.app().view.zoom);
    let gap = hnds.iter().find(|h| matches!(h.kind, handles::AutoLayoutHandleKind::Gap(_))).expect("a gap handle is shown");
    let from = screen_of(&sim, sim.app().doc.world_transform(f).apply(gap.local_center));
    Gesture {
        name: "auto layout spacing",
        sim,
        from,
        to: from + Vec2::new(30.0, 0.0),
        read: Box::new(move |s| {
            let al = s.app().doc.get(f).unwrap().kind.frame_props().and_then(|fp| fp.auto_layout.clone()).unwrap();
            let mut v = vec![al.item_spacing];
            v.extend(al.padding);
            v
        }),
        open: |i| matches!(i, Interaction::AutoLayoutDrag(_)),
        press_reopens: false,
    }
}

fn smart_selection() -> Gesture {
    let mut sim = Sim::new();
    let (a, b) = (sim.by_name("Tile A"), sim.by_name("Tile B"));
    sim.shell.app.selection = vec![a, b];
    sim.frame(vec![]);
    let items: Vec<_> = [a, b].iter().map(|id| (*id, crate::util::world_aabb(&sim.app().doc, *id))).collect();
    let hnds = handles::smart_selection_handles(&items);
    let (kind, center, _) = hnds.iter().find(|(k, _, _)| matches!(k, handles::SmartSelectionHandleKind::Gap { .. })).copied().expect("a gap handle is shown");
    let horiz = matches!(kind, handles::SmartSelectionHandleKind::Gap { horiz: true, .. });
    let from = screen_of(&sim, center);
    Gesture {
        name: "smart selection",
        sim,
        from,
        to: from + if horiz { Vec2::new(24.0, 0.0) } else { Vec2::new(0.0, 24.0) },
        read: Box::new(move |s| {
            [a, b]
                .iter()
                .flat_map(|id| {
                    let n = s.app().doc.get(*id).unwrap();
                    [n.transform.tx, n.transform.ty]
                })
                .collect()
        }),
        open: |i| matches!(i, Interaction::SmartSelectionDrag(_)),
        press_reopens: false,
    }
}

fn shape_control() -> Gesture {
    let mut sim = Sim::new();
    let page = sim.app().page;
    let id = sim.shell.app.doc.alloc_id();
    let mut star = Node::new(id, NodeKind::Star { count: 5, inner_ratio: 0.382 });
    star.size = DV::new(160.0, 160.0);
    star.transform.tx = 520.0;
    star.transform.ty = 620.0;
    let star = sim.shell.app.doc.insert(page, star).unwrap();
    sim.shell.app.selection = vec![star];
    sim.frame(vec![]);
    sim.frame(vec![]);
    let (corners, _) = interact::selection_box(sim.app()).expect("the selection has a box");
    let screen = corners.map(|c| dv_of(screen_of(&sim, c)));
    let kind = sim.app().doc.get(star).unwrap().kind.clone();
    let pts = handles::shape_control_handles(&kind, &screen, sim.app().view.zoom);
    // the inner-ratio handle
    let from = Pos2::new(pts[1].x, pts[1].y);
    Gesture {
        name: "shape control",
        sim,
        from,
        to: from + Vec2::new(30.0, 0.0),
        read: Box::new(move |s| match s.app().doc.get(star).unwrap().kind {
            NodeKind::Star { count, inner_ratio } => vec![count as f32, inner_ratio],
            _ => panic!("not a star"),
        }),
        open: |i| matches!(i, Interaction::ShapeControl(_)),
        press_reopens: false,
    }
}

fn guide() -> Gesture {
    let mut sim = Sim::new();
    shift_key(&mut sim, Key::R);
    let page = sim.app().page;
    let cr = sim.app().canvas_rect;
    let to = sim.screen(300.0, 700.0);
    Gesture {
        name: "ruler guide",
        sim,
        from: Pos2::new(cr.left() + 10.0, to.y),
        to,
        read: Box::new(move |s| s.app().doc.guides.get(&page).map(|g| g.iter().map(|g| g.offset).collect()).unwrap_or_default()),
        open: |i| matches!(i, Interaction::Guide(_)),
        press_reopens: false,
    }
}

fn pencil() -> Gesture {
    let mut sim = Sim::new();
    sim.shell.app.set_tool(Tool::Pencil);
    let floor = newest_id(&sim);
    let (from, to) = (sim.screen(520.0, 650.0), sim.screen(640.0, 760.0));
    Gesture {
        name: "pencil stroke",
        sim,
        from,
        to,
        read: Box::new(move |s| fresh_nodes(s, floor)),
        open: |i| matches!(i, Interaction::Pencil(_)),
        press_reopens: false,
    }
}

fn connector() -> Gesture {
    let mut sim = Sim::new();
    sim.shell.app.set_tool(Tool::Connector);
    let floor = newest_id(&sim);
    let (from, to) = (sim.screen(520.0, 650.0), sim.screen(640.0, 760.0));
    Gesture {
        name: "connector",
        sim,
        from,
        to,
        read: Box::new(move |s| fresh_nodes(s, floor)),
        open: |i| matches!(i, Interaction::Connect(_)),
        press_reopens: false,
    }
}

fn vector_edit() -> Gesture {
    let (mut sim, id) = sim_with_vector();
    sim.key(Key::Enter);
    assert!(sim.app().canvas.vedit.is_some(), "Enter on a selected vector enters point editing");
    let from = sim.screen(370.0, 840.0);
    Gesture {
        name: "vector edit",
        sim,
        from,
        to: from + Vec2::new(30.0, 0.0),
        read: Box::new(move |s| {
            let cmds = vector_cmds(s, id);
            cmds.iter()
                .flat_map(|c| match c {
                    designos_doc::PathCmd::MoveTo(p) | designos_doc::PathCmd::LineTo(p) => vec![p.x, p.y],
                    _ => Vec::new(),
                })
                .collect()
        }),
        open: |i| matches!(i, Interaction::VectorDrag),
        press_reopens: true,
    }
}

// ---------------------------------------------------------------- the tests

/// The values before the drag, and after the same drag spread over many frames.
fn reference(make: fn() -> Gesture) -> (Vec<f32>, Vec<f32>) {
    let mut g = make();
    let before = g.values();
    g.sim.drag(g.from, g.to);
    let after = g.values();
    assert!(!same(&before, &after), "{}: the drag changes something, or the comparison proves nothing", g.name);
    (before, after)
}

fn commits_where_released(make: fn() -> Gesture) {
    let (before, want) = reference(make);
    let mut g = make();
    g.open_at_the_end();
    let depth = g.sim.app().hist.undo_depth();
    // the pointer is back at the middle of the drag when the frame starts; the button goes up at `to` and the next
    // press lands somewhere else
    let (b, elsewhere) = (g.to, g.sim.screen(EMPTY.0, EMPTY.1));
    g.sim.mv(g.from + (b - g.from) * 0.5);
    g.sim.one_frame().move_to(b).release().move_to(elsewhere).press().send();
    if g.press_reopens {
        let v = g.sim.app().canvas.vedit.as_ref().expect("still editing");
        assert!(v.drag.is_none(), "{}: the new press does not continue the old drag", g.name);
    } else {
        assert!(!g.is_open(), "{}: the new press does not continue the old drag", g.name);
    }
    assert_eq!(g.sim.app().hist.undo_depth(), depth + 1, "{}: the drag is one step in the undo history", g.name);
    assert!(same(&g.values(), &want), "{}: ends where the button went up\n   got {:?}\n  want {want:?}", g.name, g.values());

    g.sim.one_frame().release().send();
    g.sim.frame(vec![]);
    g.sim.cmd_key(Key::Z);
    assert!(same(&g.values(), &before), "{}: one undo restores it, got {:?}", g.name, g.values());
    assert_eq!(g.sim.app().hist.undo_depth(), depth, "{}: the drag was one undo step", g.name);
}

fn commits_when_the_press_is_off_the_canvas(make: fn() -> Gesture) {
    let (_, want) = reference(make);
    let mut g = make();
    g.open_at_the_end();
    let depth = g.sim.app().hist.undo_depth();
    let b = g.to;
    let rect = g.sim.app().canvas_rect;
    let on_a_panel = Pos2::new(rect.min.x - 20.0, rect.center().y);
    g.sim.mv(g.from + (b - g.from) * 0.5);
    g.sim.one_frame().move_to(b).release().move_to(on_a_panel).press().send();
    assert!(!g.is_open(), "{}: the drag does not follow the held button over the panel", g.name);
    assert_eq!(g.sim.app().hist.undo_depth(), depth + 1, "{}: the drag is in the undo history", g.name);
    assert!(same(&g.values(), &want), "{}: ends where the button went up, got {:?}", g.name, g.values());
}

/// One pair of tests per gesture, so a failure names the drag that dropped its change.
macro_rules! gesture_tests {
    ($($scene:ident),* $(,)?) => {
        mod where_released {
            use super::*;
            $(#[test] fn $scene() { commits_where_released(super::$scene); })*
        }
        mod press_off_the_canvas {
            use super::*;
            $(#[test] fn $scene() { commits_when_the_press_is_off_the_canvas(super::$scene); })*
        }
    };
}

gesture_tests!(corner_radius, auto_layout_spacing, smart_selection, shape_control, guide, pencil, connector, vector_edit);

// ---------------------------------------------------------------- a synthesized end makes no edit

fn vector_state(sim: &Sim, id: NodeId) -> (usize, Vec<designos_doc::PathCmd>) {
    (sim.app().hist.undo_depth(), vector_cmds(sim, id))
}

/// A press on a vector segment or vertex that never moved, then something other than a release ends it: the point is
/// not inserted and the vertex does not toggle between sharp and smooth.
fn unmoved_vector_press_then(end: impl Fn(&mut Sim, Pos2)) {
    for spot in [(335.0, 820.0), (370.0, 840.0)] {
        let (mut sim, id) = sim_with_vector();
        sim.key(Key::Enter);
        assert!(sim.app().canvas.vedit.is_some());
        let before = vector_state(&sim, id);
        let at = sim.screen(spot.0, spot.1);
        sim.mv(at);
        sim.button(true);
        sim.frame(vec![]);
        assert!(matches!(sim.app().canvas.inter, Interaction::VectorDrag), "{spot:?}: the press opens a vector drag");
        let elsewhere = sim.screen(EMPTY.0, EMPTY.1);
        end(&mut sim, elsewhere);
        sim.one_frame().release().send();
        sim.frame(vec![]);
        assert_eq!(vector_state(&sim, id), before, "{spot:?}: an unmoved press that was ended without a release edits nothing");
    }
}

#[test]
fn a_middle_press_over_an_unmoved_vector_press_makes_no_edit() {
    unmoved_vector_press_then(|sim, _| sim.one_frame().button(PointerButton::Middle, true).send());
}

#[test]
fn a_press_after_a_lost_release_over_an_unmoved_vector_press_makes_no_edit() {
    unmoved_vector_press_then(|sim, elsewhere| sim.one_frame().move_to(elsewhere).press().send());
}

#[test]
fn a_guide_whose_release_is_lost_off_the_window_is_moved_not_removed() {
    let mut g = guide();
    let page = g.sim.app().page;
    g.sim.drag(g.from, g.to);
    // pick the guide up again, drag it off the canvas, and lose the release
    let at = g.to;
    let off = Pos2::new(g.sim.app().canvas_rect.min.x - 30.0, at.y);
    g.sim.mv(at);
    g.sim.button(true);
    g.sim.mv(at + Vec2::new(40.0, 0.0));
    g.sim.mv(off);
    assert!(matches!(g.sim.app().canvas.inter, Interaction::Guide(_)), "the guide is picked up");
    let elsewhere = g.sim.screen(EMPTY.0, EMPTY.1);
    g.sim.one_frame().move_to(elsewhere).press().send();
    assert_eq!(g.sim.app().doc.guides.get(&page).map_or(0, |l| l.len()), 1, "the guide is still there");
}

/// A pointer button event with the modifiers held when it happened.
fn button_event(at: Pos2, pressed: bool, modifiers: Modifiers) -> Event {
    Event::PointerButton { pos: at, button: PointerButton::Primary, pressed, modifiers }
}

#[test]
fn a_modifier_released_while_the_button_is_down_still_shapes_a_corner_radius_at_the_release() {
    // Alt rounds one corner. It is held through the drag and up again by the time the frame with the release is drawn.
    let drag_with_alt = |alt_up_early: bool| {
        let mut g = corner_radius();
        let (a, b) = (g.from, g.to);
        g.sim.mods = Modifiers::ALT;
        g.sim.mv(a);
        g.sim.button(true);
        for i in 1..=4 {
            g.sim.mv(a + (b - a) * (i as f32 / 4.0));
        }
        if alt_up_early {
            g.sim.mods = Modifiers::NONE;
            g.sim.frame(vec![button_event(b, false, Modifiers::ALT)]);
        } else {
            g.sim.button(false);
        }
        g.sim.mods = Modifiers::NONE;
        g.sim.frame(vec![]);
        (g.values(), g.sim.app().hist.undo_depth())
    };
    let (slow, depth) = drag_with_alt(false);
    let (early, early_depth) = drag_with_alt(true);
    let plain = {
        let mut g = corner_radius();
        g.sim.drag(g.from, g.to);
        g.values()
    };
    assert!(!same(&slow, &plain), "Alt changes the result, or the comparison proves nothing");
    assert!(same(&early, &slow), "the release keeps the Alt held at the release: got {early:?}, want {slow:?}");
    assert_eq!(early_depth, depth, "one undo step either way");
}

// ---------------------------------------------------------------- a new guide that ends without a release

/// A new guide dragged from the ruler and left (`out`) before something other than a release ends it: no guide is
/// placed and nothing is added to the undo history.
fn new_guide_on_the_ruler_then(out: bool, end: impl Fn(&mut Sim, Pos2)) {
    let mut g = guide();
    let page = g.sim.app().page;
    let depth = g.sim.app().hist.undo_depth();
    g.sim.mv(g.from);
    g.sim.button(true);
    g.sim.frame(vec![]);
    assert!(matches!(g.sim.app().canvas.inter, Interaction::Guide(_)), "the press on the ruler starts a guide");
    if out {
        g.sim.mv(g.to);
        g.sim.mv(g.from);
    }
    let elsewhere = g.sim.screen(EMPTY.0, EMPTY.1);
    end(&mut g.sim, elsewhere);
    g.sim.one_frame().release().send();
    g.sim.frame(vec![]);
    assert_eq!(g.sim.app().doc.guides.get(&page).map_or(0, |l| l.len()), 0, "no guide is placed");
    assert_eq!(g.sim.app().hist.undo_depth(), depth, "nothing is added to the undo history");
}

#[test]
fn a_middle_press_over_an_unmoved_ruler_press_places_no_guide() {
    new_guide_on_the_ruler_then(false, |sim, _| sim.one_frame().button(PointerButton::Middle, true).send());
}

#[test]
fn a_middle_press_over_a_guide_put_back_on_the_ruler_places_no_guide() {
    new_guide_on_the_ruler_then(true, |sim, _| sim.one_frame().button(PointerButton::Middle, true).send());
}

#[test]
fn a_press_after_a_lost_release_over_an_unmoved_ruler_press_places_no_guide() {
    new_guide_on_the_ruler_then(false, |sim, elsewhere| sim.one_frame().move_to(elsewhere).press().send());
}

#[test]
fn a_real_release_then_a_press_in_one_frame_still_inserts_a_point_on_a_vector_segment() {
    let (mut sim, id) = sim_with_vector();
    sim.key(Key::Enter);
    let before = vector_cmds(&sim, id).len();
    let at = sim.screen(335.0, 820.0);
    sim.mv(at);
    sim.button(true);
    sim.frame(vec![]);
    assert!(matches!(sim.app().canvas.inter, Interaction::VectorDrag));
    let elsewhere = sim.screen(EMPTY.0, EMPTY.1);
    sim.one_frame().release().move_to(elsewhere).press().send();
    sim.one_frame().release().send();
    sim.frame(vec![]);
    assert_eq!(vector_cmds(&sim, id).len(), before + 1, "a click on a segment adds a point");
}
