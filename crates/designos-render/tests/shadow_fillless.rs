//! #520: a fill-less device-mockup frame keeps the shadow of its children inside its own bounds.
//! The fixture is the one in `520-spec.md` section 2: a 240x160 frame, a drop shadow of black at
//! 50% (offset 0,12, blur 24, show behind off), an opaque blue child A and a 50% red child B.
//! Rule F2 (`FILLLESS_KNOCKOUT = ChildrenOpaque`): the shadow is removed under the children
//! only, never under the whole frame rectangle.

mod common;

use common::*;
use designos_doc::*;
use designos_render::tiny_skia::Pixmap;
use designos_render::{render_page, TileBudget, TileCache, Viewport, TILE_SIZE};

const SIDE: u32 = 3 * TILE_SIZE;
const TOL: f32 = 0.02;

const BLUE_A: Color = Color { r: 0x25 as f32 / 255.0, g: 0x63 as f32 / 255.0, b: 0xEB as f32 / 255.0, a: 1.0 };
const RED_B: Color = Color { r: 0xDC as f32 / 255.0, g: 0x26 as f32 / 255.0, b: 0x26 as f32 / 255.0, a: 0.5 };

fn shadow(show_behind: bool) -> Effect {
    Effect::DropShadow { color: Color { r: 0.0, g: 0.0, b: 0.0, a: 0.5 }, offset: Vec2::new(0.0, 12.0), radius: 24.0, spread: 0.0, visible: true, show_behind }
}

/// Adds one fixture frame at (`x`, `y`) and returns it with its children (A, B).
fn frame(t: &mut T, x: f32, y: f32, white: bool, show_behind: bool) -> (NodeId, NodeId, NodeId) {
    let f = t.add(None, |id| {
        let mut n = Node::frame(id, Rect::new(x, y, 240.0, 160.0));
        n.fills = if white { vec![Paint::solid(Color::WHITE)] } else { vec![] };
        n.effects = vec![shadow(show_behind)];
        n
    });
    let a = t.rect(Some(f), Rect::new(60.0, 50.0, 120.0, 60.0), BLUE_A);
    let b = t.rect(Some(f), Rect::new(20.0, 110.0, 40.0, 30.0), RED_B);
    (f, a, b)
}

fn fixture(white: bool, show_behind: bool) -> (T, Pixmap) {
    let mut t = T::new();
    t.node(t.page).kind = NodeKind::Page { background: Color::TRANSPARENT };
    frame(&mut t, 96.0, 96.0, white, show_behind);
    let pm = t.render(SIDE, SIDE);
    (t, pm)
}

fn alpha(pm: &Pixmap, x: f32, y: f32) -> f32 {
    px(pm, x as u32, y as u32)[3] as f32 / 255.0
}

/// Shadow-layer alpha at the probes P1 (120,128), P2 (40,125), P3 (120,170) of a frame at
/// (`fx`, `fy`). P1 and P3 lie clear of both children, so the pixel alpha is the shadow alpha.
/// P2 is under B (a 50% layer over the shadow): alpha = 0.5 + 0.5 * shadow.
fn probes(pm: &Pixmap, fx: f32, fy: f32) -> [f32; 3] {
    let p2 = (alpha(pm, fx + 40.0, fy + 125.0) - 0.5) / 0.5;
    [alpha(pm, fx + 120.0, fy + 128.0), p2, alpha(pm, fx + 120.0, fy + 170.0)]
}

fn assert_probes(got: [f32; 3], want: [f32; 3], what: &str) {
    for (i, (g, w)) in got.iter().zip(want).enumerate() {
        assert!((g - w).abs() <= TOL, "{what}: P{} is {g:.3}, want {w:.3} +- {TOL}", i + 1);
    }
}

#[test]
fn filled_frame_shadow_comes_from_its_own_shape() {
    let (_, pm) = fixture(true, false);
    // P3 below the frame carries the shadow of the frame's own shape (0.275)
    assert_probes([0.0, 0.0, probes(&pm, 96.0, 96.0)[2]], [0.0, 0.0, 0.275], "frame 1");
    // inside the opaque fill nothing of the shadow shows: P1 is clean white, P2 is B over white
    assert!(near(px(&pm, 216, 224), WHITE, 1), "P1 {:?}", px(&pm, 216, 224));
    assert!(near(px(&pm, 136, 221), [237, 147, 147, 255], 3), "P2 {:?}", px(&pm, 136, 221));
}

#[test]
fn fillless_frame_shadow_is_not_erased_inside_the_frame() {
    let (_, pm) = fixture(false, false);
    let p = probes(&pm, 96.0, 96.0);
    assert!(p[0] > 0.1, "the shadow of child A must show inside the frame, P1 = {:.3}", p[0]);
}

#[test]
fn fillless_frame_knockout_follows_children_opaque_rule() {
    assert_eq!(FILLLESS_KNOCKOUT, FilllessKnockout::ChildrenOpaque);
    let (_, pm) = fixture(false, false);
    // F2: P1 keeps the shadow, P2 under child B is knocked out, P3 outside has none
    assert_probes(probes(&pm, 96.0, 96.0), [0.149, 0.0, 0.0], "frame 2, F2");
}

#[test]
fn show_behind_on_never_knocks_out() {
    let (_, pm) = fixture(false, true);
    let p = probes(&pm, 96.0, 96.0);
    assert!(p[0] > 0.1 && p[1] > 0.1, "shadow visible under both children: P1 {:.3}, P2 {:.3}", p[0], p[1]);
}

/// Child A sticks out of the frame at the bottom by 40; the frame clips content.
fn clipping(clips: bool) -> (T, NodeId, NodeId) {
    let mut t = T::new();
    t.node(t.page).kind = NodeKind::Page { background: Color::TRANSPARENT };
    let (f, a, _) = frame(&mut t, 96.0, 96.0, false, false);
    t.node(f).kind.frame_props_mut().unwrap().clips_content = clips;
    t.node(a).size = Vec2::new(120.0, 150.0); // reaches y = 96 + 50 + 150 = 296, frame ends at 256
    (t, f, a)
}

#[test]
fn clipping_frame_clips_children_in_silhouette_and_knockout() {
    let (open, ..) = clipping(false);
    let (clip, ..) = clipping(true);
    let (open, clip) = (open.render(SIDE, SIDE), clip.render(SIDE, SIDE));
    // below the frame (y 256), 20px under the clipped edge: P (216, 276)
    // unclipped: child A paints there; clipped: nothing paints, and its shadow is the clipped
    // shape's shadow, which stays inside the frame plus its blur falloff.
    assert!(alpha(&open, 216.0, 276.0) > 0.9, "unclipped child A paints below the frame");
    let a = alpha(&clip, 216.0, 276.0);
    assert!(a > 0.0 && a < 0.35, "clipped: only the shadow of the clipped part shows below the frame, got {a:.3}");
    // inside the frame the knockout still follows the child: A's body has no shadow of its own
    // under it, but the clipped shadow does not leak into where A is (alpha = A, fully opaque)
    assert!(alpha(&clip, 216.0, 200.0) > 0.99);
    // the unclipped child's shadow reaches farther below than the clipped one
    assert!(alpha(&open, 216.0, 300.0) > alpha(&clip, 216.0, 300.0) + 0.05);
}

#[test]
fn hidden_and_transparent_children_add_nothing() {
    let (mut t, ..) = {
        let mut t = T::new();
        t.node(t.page).kind = NodeKind::Page { background: Color::TRANSPARENT };
        let ids = frame(&mut t, 96.0, 96.0, false, false);
        (t, ids)
    };
    let kids = t.doc.get(t.doc.get(t.page).unwrap().children[0]).unwrap().children.clone();
    t.node(kids[0]).visible = false;
    t.node(kids[1]).opacity = 0.0;
    let pm = t.render(SIDE, SIDE);
    // no child draws or casts: not a pixel of shadow anywhere
    assert!(pm.data().iter().all(|b| *b == 0), "hidden and zero-opacity children leave nothing");
}

/// Both fixture frames side by side on one page, so tiles cross both shadows.
fn pair() -> T {
    let mut t = T::new();
    t.node(t.page).kind = NodeKind::Page { background: Color::TRANSPARENT };
    frame(&mut t, 96.0, 96.0, true, false);
    frame(&mut t, 432.0, 96.0, false, false);
    t
}

#[test]
fn fixture_pair_probes_on_one_page() {
    let t = pair();
    let pm = t.render(SIDE, SIDE);
    assert!((probes(&pm, 96.0, 96.0)[2] - 0.275).abs() <= TOL);
    assert_probes(probes(&pm, 432.0, 96.0), [0.149, 0.0, 0.0], "frame 2 next to frame 1");
}

#[test]
fn tile_render_equals_full_render_for_the_fixture_pair() {
    let t = pair();
    let vp = Viewport { offset: Vec2::ZERO, zoom: 1.0, dpr: 1.0 };
    // the reference is a plain render with a margin, cropped: blurs at a pixmap edge see
    // transparent pixels where a tile sees its neighbours
    const M: u32 = 128;

    let mut cache = TileCache::new();
    let f = cache.render_tiles(&t.doc, t.page, &vp, (SIDE, SIDE), 1, TileBudget::unlimited());
    assert_eq!(f.pending(), 0);
    let mut tiled = Pixmap::new(SIDE, SIDE).unwrap();
    for tile in &f.tiles {
        let (x, y) = tile.key.device_origin();
        tiled.draw_pixmap(x, y, tile.pixmap.as_ref().as_ref(), &Default::default(), Default::default(), None);
    }
    let mut big = Pixmap::new(SIDE + 2 * M, SIDE + 2 * M).unwrap();
    render_page(&t.doc, t.page, &Viewport { offset: Vec2::new(-(M as f32), -(M as f32)), ..vp }, &mut big);
    let mut reference = Pixmap::new(SIDE, SIDE).unwrap();
    reference.draw_pixmap(-(M as i32), -(M as i32), big.as_ref(), &Default::default(), Default::default(), None);
    let diff = tiled.data().iter().zip(reference.data()).filter(|(a, b)| a != b).count();
    assert_eq!(diff, 0, "tiles and the full render differ in {diff} bytes");
}
