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
    Effect::DropShadow {
        color: Color { r: 0.0, g: 0.0, b: 0.0, a: 0.5 },
        offset: Vec2::new(0.0, 12.0),
        radius: 24.0,
        spread: 0.0,
        visible: true,
        show_behind,
        blend: Default::default(),
    }
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

// ---------------------------------------------------------------------------
// Fill-less frame shadow knockout follows what the children paint
// ---------------------------------------------------------------------------

fn right_shadow() -> Effect {
    Effect::DropShadow {
        color: Color { r: 0.0, g: 0.0, b: 0.0, a: 0.65 },
        offset: Vec2::new(18.0, 0.0),
        radius: 8.0,
        spread: 0.0,
        visible: true,
        show_behind: false,
        blend: Default::default(),
    }
}

fn case_a_doc(with_paint_free_children: bool) -> T {
    let mut t = T::new();
    let f = t.add(None, |id| {
        let mut n = Node::frame(id, Rect::new(20.0, 20.0, 120.0, 80.0));
        n.fills.clear();
        n.effects = vec![right_shadow()];
        n
    });
    t.rect(Some(f), Rect::new(0.0, 0.0, 50.0, 80.0), BLUE_A);
    if with_paint_free_children {
        // Paint-free child frame (no fills) over top half of cast shadow
        t.add(Some(f), |id| {
            let mut n = Node::frame(id, Rect::new(50.0, 0.0, 70.0, 40.0));
            n.fills.clear();
            n
        });
        // 0%-opacity fill child frame over bottom half of cast shadow
        t.add(Some(f), |id| {
            let mut n = Node::frame(id, Rect::new(50.0, 40.0, 70.0, 40.0));
            let mut p = Paint::solid(Color::WHITE);
            p.opacity = 0.0;
            n.fills = vec![p];
            n
        });
    }
    t
}

#[test]
fn paint_free_and_zero_fill_children_keep_shadow() {
    let actual = case_a_doc(true);
    let expected = case_a_doc(false);
    let pm = actual.render(160, 120);
    // (78, 40) is under the paint-free child frame; (78, 80) is under the 0%-fill child frame
    assert!(px(&pm, 78, 40)[0] < 220, "shadow under paint-free child frame was knocked out: {:?}", px(&pm, 78, 40));
    assert!(px(&pm, 78, 80)[0] < 220, "shadow under 0%-fill child frame was knocked out: {:?}", px(&pm, 78, 80));
    let diff = designos_render::render_diff_image(&actual.doc, &expected.doc, actual.page, Rect::new(0.0, 0.0, 160.0, 120.0), 1.0).unwrap();
    assert_eq!(diff.changed_pixels, 0, "paint-free/0%-fill children changed {} pixels", diff.changed_pixels);
}

fn case_b_doc(nested_in_fillless_child_frame: bool) -> T {
    let mut t = T::new();
    let f = t.add(None, |id| {
        let mut n = Node::frame(id, Rect::new(20.0, 20.0, 120.0, 80.0));
        n.fills.clear();
        n.effects = vec![right_shadow()];
        n
    });
    t.rect(Some(f), Rect::new(0.0, 0.0, 40.0, 80.0), BLUE_A);
    if nested_in_fillless_child_frame {
        let child_frame = t.add(Some(f), |id| {
            let mut n = Node::frame(id, Rect::new(40.0, 0.0, 80.0, 80.0));
            n.fills = vec![Paint::solid(Color::TRANSPARENT)];
            n
        });
        // Paint-free spacer grandchild inside the child frame
        t.add(Some(child_frame), |id| {
            let mut n = Node::frame(id, Rect::new(0.0, 0.0, 40.0, 80.0));
            n.fills.clear();
            n
        });
        // Painting grandchild at (40, 10, 40, 60) inside child_frame -> (80, 10, 40, 60) in f
        t.rect(Some(child_frame), Rect::new(40.0, 10.0, 40.0, 60.0), RED_B);
    } else {
        t.rect(Some(f), Rect::new(80.0, 10.0, 40.0, 60.0), RED_B);
    }
    t
}

#[test]
fn fillless_child_frame_with_painting_grandchild_knocks_out_only_grandchild() {
    let actual = case_b_doc(true);
    let expected = case_b_doc(false);
    let pm = actual.render(160, 120);
    // (68, 60) is inside the fill-less child frame (outside the grandchild) in the cast shadow of BLUE_A
    assert!(px(&pm, 68, 60)[0] < 220, "fill-less child frame knocked out shadow outside grandchild: {:?}", px(&pm, 68, 60));
    // (115, 60) is inside the 50% red grandchild: its own shadow underneath is knocked out
    assert!(px(&pm, 115, 60)[1] >= 120, "grandchild area did not knock out its shadow: {:?}", px(&pm, 115, 60));
    let diff = designos_render::render_diff_image(&actual.doc, &expected.doc, actual.page, Rect::new(0.0, 0.0, 160.0, 120.0), 1.0).unwrap();
    assert_eq!(diff.changed_pixels, 0, "fill-less child frame wrapper changed {} pixels", diff.changed_pixels);
}

fn case_c_doc(use_mask: bool) -> T {
    let mut t = T::new();
    let f = t.add(None, |id| {
        let mut n = Node::frame(id, Rect::new(20.0, 20.0, 100.0, 100.0));
        n.fills.clear();
        n.effects = vec![Effect::DropShadow {
            color: Color { r: 0.0, g: 0.0, b: 0.0, a: 0.75 },
            offset: Vec2::ZERO,
            radius: 14.0,
            spread: 6.0,
            visible: true,
            show_behind: false,
            blend: Default::default(),
        }];
        n
    });
    if use_mask {
        t.add(Some(f), |id| {
            let mut m = Node::ellipse(id, Rect::new(10.0, 10.0, 80.0, 80.0));
            m.is_mask = true;
            m.fills = vec![Paint::solid(Color::WHITE)];
            m
        });
        t.rect(Some(f), Rect::new(10.0, 10.0, 80.0, 80.0), RED_B);
    } else {
        t.add(Some(f), |id| {
            let mut e = Node::ellipse(id, Rect::new(10.0, 10.0, 80.0, 80.0));
            e.fills = vec![Paint::solid(RED_B)];
            e
        });
    }
    t
}

#[test]
fn ellipse_mask_plus_rect_sibling_knocks_out_circle_not_rect() {
    let actual = case_c_doc(true);
    let expected = case_c_doc(false);
    let pm = actual.render(140, 140);
    // (40, 40) is inside the 80x80 rect (30..110, 30..110) but outside the circle mask centered at (70, 70)
    assert!(px(&pm, 40, 40)[0] < 230, "unmasked rect corner knocked out circular shadow at (40, 40): {:?}", px(&pm, 40, 40));
    // (70, 70) is the center of the masked circle: shadow underneath is knocked out
    assert!(px(&pm, 70, 70)[1] >= 120, "circle interior did not knock out shadow at (70, 70): {:?}", px(&pm, 70, 70));
    let exp_pm = expected.render(140, 140);
    for (a, b) in pm.data().chunks_exact(4).zip(exp_pm.data().chunks_exact(4)) {
        assert!(near([a[0], a[1], a[2], a[3]], [b[0], b[1], b[2], b[3]], 2), "pixel differed by >2 ULP: {a:?} vs {b:?}");
    }
    let diff = designos_render::render_diff_image(&actual.doc, &expected.doc, actual.page, Rect::new(0.0, 0.0, 140.0, 140.0), 1.0).unwrap();
    assert!(diff.changed_ratio < 0.01, "masked rect vs circle differed by {:.4}% ({} px)", diff.changed_ratio * 100.0, diff.changed_pixels);
}

fn case_d_doc(wrapped_in_group: bool) -> T {
    let mut t = T::new();
    let f = t.add(None, |id| {
        let mut n = Node::frame(id, Rect::new(20.0, 20.0, 120.0, 80.0));
        n.fills.clear();
        n.effects = vec![Effect::DropShadow {
            color: Color { r: 0.0, g: 0.0, b: 0.0, a: 0.75 },
            offset: Vec2::new(0.0, 8.0),
            radius: 12.0,
            spread: 0.0,
            visible: true,
            show_behind: false,
            blend: Default::default(),
        }];
        n
    });
    if wrapped_in_group {
        let g = t.add(Some(f), |id| Node::new(id, NodeKind::Group));
        t.rect(Some(g), Rect::new(10.0, 10.0, 100.0, 60.0), RED_B);
    } else {
        t.rect(Some(f), Rect::new(10.0, 10.0, 100.0, 60.0), RED_B);
    }
    t
}

#[test]
fn group_wrapped_50pct_child_removes_same_shadow_as_direct_child() {
    let actual = case_d_doc(true);
    let expected = case_d_doc(false);
    let pm = actual.render(160, 120);
    // (80, 60) is the center of the 50% red child over white page: shadow must be fully knocked out
    assert!(px(&pm, 80, 60)[1] >= 120, "group-wrapped 50% child left shadow under itself: {:?}", px(&pm, 80, 60));
    let diff = designos_render::render_diff_image(&actual.doc, &expected.doc, actual.page, Rect::new(0.0, 0.0, 160.0, 120.0), 1.0).unwrap();
    assert_eq!(diff.changed_pixels, 0, "group wrapper changed {} shadow pixels", diff.changed_pixels);
}

#[test]
fn runtime_opacity_zero_child_is_skipped_in_knockout() {
    let mut t = T::new();
    let f = t.add(None, |id| {
        let mut n = Node::frame(id, Rect::new(20.0, 20.0, 120.0, 80.0));
        n.fills.clear();
        n.effects = vec![right_shadow()];
        n
    });
    t.rect(Some(f), Rect::new(0.0, 0.0, 50.0, 80.0), BLUE_A);
    let c2 = t.rect(Some(f), Rect::new(50.0, 0.0, 70.0, 80.0), Color::WHITE);

    let vp = Viewport::default();
    let mut cache = TileCache::new();
    cache.set_runtime_opacities(&t.doc, t.page, vec![(c2, 0.0)]);
    let tf = cache.render_tiles(&t.doc, t.page, &vp, (160, 120), 1, TileBudget::unlimited());
    let mut pm = Pixmap::new(160, 120).unwrap();
    for tile in &tf.tiles {
        let (x, y) = tile.key.device_origin();
        pm.draw_pixmap(x, y, tile.pixmap.as_ref().as_ref(), &Default::default(), Default::default(), None);
    }
    assert!(px(&pm, 78, 60)[0] < 220, "runtime-opacity-0 child knocked out shadow on CPU: {:?}", px(&pm, 78, 60));
}

fn fnv1a(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

#[test]
fn null_control_filled_and_plain_fillless_frames_unchanged() {
    let (_, filled) = fixture(true, false);
    let (_, behind) = fixture(false, true);
    let (_, plain_fillless) = fixture(false, false);
    assert_probes([0.0, 0.0, probes(&filled, 96.0, 96.0)[2]], [0.0, 0.0, 0.275], "null control filled");
    assert!(near(px(&filled, 216, 224), WHITE, 1), "filled P1 {:?}", px(&filled, 216, 224));
    let p_behind = probes(&behind, 96.0, 96.0);
    assert!(p_behind[0] > 0.1 && p_behind[1] > 0.1, "null control show_behind {:?}", p_behind);
    assert_probes(probes(&plain_fillless, 96.0, 96.0), [0.149, 0.0, 0.0], "null control plain fill-less");
    // Exact deterministic pixel digests of the fill-less, filled and show-behind fixtures
    assert_eq!(fnv1a(filled.data()), 0x92c8d1f05c89c3be);
    assert_eq!(fnv1a(behind.data()), 0x67f982db879975e7);
    assert_eq!(fnv1a(plain_fillless.data()), 0x31ff0b365a24b628);
}

// ---------------------------------------------------------------------------
// Paint-free leaves, text children and luminance masks
// ---------------------------------------------------------------------------

fn differing_bytes(a: &Pixmap, b: &Pixmap) -> usize {
    a.data().iter().zip(b.data()).filter(|(x, y)| x != y).count()
}

fn zero_percent_paint() -> Paint {
    let mut p = Paint::solid(Color::WHITE);
    p.opacity = 0.0;
    p
}

fn offset_drop_shadow(radius: f32) -> Effect {
    Effect::DropShadow {
        color: Color { r: 0.0, g: 0.0, b: 0.0, a: 0.65 },
        offset: Vec2::new(16.0, 16.0),
        radius,
        spread: 0.0,
        visible: true,
        show_behind: false,
        blend: Default::default(),
    }
}

fn inner_shadow(radius: f32) -> Effect {
    Effect::InnerShadow {
        color: Color { r: 0.0, g: 0.0, b: 0.0, a: 0.65 },
        offset: Vec2::new(6.0, 6.0),
        radius,
        spread: 0.0,
        visible: true,
        blend: Default::default(),
    }
}

/// A frame whose only fill is visible but 0% opaque, holding one child painted `child`.
fn zero_fill_frame(effects: Vec<Effect>, child: Color) -> T {
    let mut t = T::new();
    let f = t.add(None, |id| {
        let mut n = Node::frame(id, Rect::new(30.0, 30.0, 100.0, 80.0));
        n.fills = vec![zero_percent_paint()];
        n.effects = effects;
        n
    });
    t.rect(Some(f), Rect::new(10.0, 10.0, 40.0, 40.0), child);
    t
}

#[test]
fn zero_fill_frame_with_children_keeps_an_empty_drop_shadow() {
    for radius in [0.0, 8.0] {
        let with = zero_fill_frame(vec![offset_drop_shadow(radius)], BLUE_A).render(160, 140);
        let without = zero_fill_frame(vec![], BLUE_A).render(160, 140);
        let diff = differing_bytes(&with, &without);
        assert_eq!(diff, 0, "radius {radius}: a 0% fill frame cast a drop shadow ({diff} bytes differ)");
    }
}

#[test]
fn zero_fill_frame_with_children_draws_no_inner_shadow() {
    for radius in [0.0, 4.0] {
        // a 50% child, so an inner shadow drawn under it would show through
        let with = zero_fill_frame(vec![inner_shadow(radius)], RED_B).render(160, 140);
        let without = zero_fill_frame(vec![], RED_B).render(160, 140);
        let diff = differing_bytes(&with, &without);
        assert_eq!(diff, 0, "radius {radius}: a 0% fill frame drew an inner shadow ({diff} bytes differ)");
    }
}

/// A fill-less frame whose stroke paint is 0% opaque.
fn zero_stroke_frame(effects: Vec<Effect>) -> T {
    let mut t = T::new();
    t.add(None, |id| {
        let mut n = Node::frame(id, Rect::new(30.0, 30.0, 100.0, 60.0));
        n.fills.clear();
        n.stroke.weight = 4.0;
        n.stroke.paints = vec![zero_percent_paint()];
        n.effects = effects;
        n
    });
    t
}

#[test]
fn zero_percent_stroke_frame_keeps_its_stroke_footprint_shadow() {
    let with = zero_stroke_frame(vec![offset_drop_shadow(8.0)]).render(160, 140);
    let without = zero_stroke_frame(vec![]).render(160, 140);
    // the ring's shadow, shifted by 16, shows below the frame's bottom edge (y 90 -> 106)
    let probe = px(&with, 80, 106);
    assert!(probe[0] < 235, "the stroke footprint cast no shadow: {probe:?}");
    assert!(differing_bytes(&with, &without) > 0);
}

const TEXT_CANVAS: (u32, u32) = (240, 230);

/// A fill-less frame (20, 20, 200 x 150): a blue band across the top whose shadow falls 70px
/// below it, and a text child standing in that shadow.
fn text_scene(text_fills: Vec<Paint>, text_stroke: bool, with_text: bool, with_shadow: bool) -> T {
    let mut t = T::new();
    let f = t.add(None, |id| {
        let mut n = Node::frame(id, Rect::new(20.0, 20.0, 200.0, 150.0));
        n.fills.clear();
        if with_shadow {
            n.effects = vec![Effect::DropShadow {
                color: Color { r: 0.0, g: 0.0, b: 0.0, a: 0.65 },
                offset: Vec2::new(0.0, 70.0),
                radius: 6.0,
                spread: 0.0,
                visible: true,
                show_behind: false,
                blend: Default::default(),
            }];
        }
        n
    });
    t.rect(Some(f), Rect::new(0.0, 0.0, 200.0, 60.0), BLUE_A);
    if with_text {
        t.add(Some(f), |id| {
            let style = TextStyle { font_size: 48.0, font_weight: 700, fills: text_fills, ..Default::default() };
            let mut n = Node::text(id, Rect::new(10.0, 72.0, 180.0, 60.0), "HHH", style);
            if text_stroke {
                n.stroke.weight = 1.0;
                n.stroke.paints = vec![Paint::solid(Color::BLACK)];
            }
            n
        });
    }
    t
}

fn black_at(opacity: f32) -> Paint {
    let mut p = Paint::solid(Color::BLACK);
    p.opacity = opacity;
    p
}

/// Pixel positions fully covered by a 50% black glyph: its interior, away from the rim.
fn glyph_interior(pm: &Pixmap) -> Vec<(u32, u32)> {
    let covered = |x: u32, y: u32| {
        let p = px(pm, x, y);
        y >= 90 && p[0] == p[1] && p[1] == p[2] && p[0] <= 128 && p[3] == 255
    };
    let mut out = Vec::new();
    for y in 1..pm.height() - 1 {
        for x in 1..pm.width() - 1 {
            if covered(x, y) && (-1i32..=1).all(|dy| (-1i32..=1).all(|dx| covered((x as i32 + dx) as u32, (y as i32 + dy) as u32))) {
                out.push((x, y));
            }
        }
    }
    out
}

#[test]
fn paint_free_text_children_add_nothing_to_the_knockout() {
    let (w, h) = TEXT_CANVAS;
    let mut hidden = black_at(1.0);
    hidden.visible = false;
    for (what, fills) in [("0% fill", vec![black_at(0.0)]), ("hidden fill", vec![hidden]), ("no fill", vec![])] {
        let actual = text_scene(fills, false, true, true).render(w, h);
        let expected = text_scene(vec![], false, false, true).render(w, h);
        let diff = differing_bytes(&actual, &expected);
        assert_eq!(diff, 0, "{what}: text that paints nothing changed {diff} bytes of the shadow");
    }
}

#[test]
fn half_opaque_text_knocks_out_the_shadow_at_full_coverage() {
    let (w, h) = TEXT_CANVAS;
    let actual = text_scene(vec![black_at(0.5)], false, true, true).render(w, h);
    let no_shadow = text_scene(vec![black_at(0.5)], false, true, false).render(w, h);
    let interior = glyph_interior(&no_shadow);
    assert!(interior.len() > 100, "the glyphs must have interior pixels, found {}", interior.len());
    for (x, y) in &interior {
        assert!(
            near(px(&actual, *x, *y), px(&no_shadow, *x, *y), 1),
            "({x}, {y}): shadow left under a 50% glyph: {:?} vs {:?}",
            px(&actual, *x, *y),
            px(&no_shadow, *x, *y)
        );
    }
    // the shadow does show beside the glyphs
    assert!(px(&actual, 24, 120)[0] < 235, "the fixture must cast a shadow: {:?}", px(&actual, 24, 120));
}

#[test]
fn stroke_only_text_still_knocks_out_its_glyph_interior() {
    let (w, h) = TEXT_CANVAS;
    let interior = glyph_interior(&text_scene(vec![black_at(0.5)], false, true, false).render(w, h));
    let actual = text_scene(vec![], true, true, true).render(w, h);
    assert!(interior.len() > 100);
    for (x, y) in &interior {
        assert!(near(px(&actual, *x, *y), WHITE, 1), "({x}, {y}): {:?}", px(&actual, *x, *y));
    }
    assert!(px(&actual, 24, 120)[0] < 235, "the fixture must cast a shadow: {:?}", px(&actual, 24, 120));
}

/// The ellipse mask of the mask case, but a mid-gray luminance mask (alpha 1, luminance 0.5).
fn luminance_scene(show_behind: Option<bool>) -> T {
    let mut t = T::new();
    let f = t.add(None, |id| {
        let mut n = Node::frame(id, Rect::new(20.0, 20.0, 100.0, 100.0));
        n.fills.clear();
        if let Some(show_behind) = show_behind {
            n.effects = vec![Effect::DropShadow {
                color: Color { r: 0.0, g: 0.0, b: 0.0, a: 0.75 },
                offset: Vec2::ZERO,
                radius: 14.0,
                spread: 6.0,
                visible: true,
                show_behind,
                blend: Default::default(),
            }];
        }
        n
    });
    t.add(Some(f), |id| {
        let mut m = Node::ellipse(id, Rect::new(10.0, 10.0, 80.0, 80.0));
        m.is_mask = true;
        m.mask_type = MaskType::Luminance;
        m.fills = vec![Paint::solid(Color { r: 0.5, g: 0.5, b: 0.5, a: 1.0 })];
        m
    });
    t.rect(Some(f), Rect::new(10.0, 10.0, 80.0, 80.0), RED_B);
    t
}

#[test]
fn luminance_mask_knocks_out_by_its_luminance() {
    let knocked = luminance_scene(Some(false)).render(140, 140);
    let behind = luminance_scene(Some(true)).render(140, 140);
    let none = luminance_scene(None).render(140, 140);
    // at the centre, green rises with the share of shadow knocked out: none = all, behind = nothing
    let (gk, gb, gn) = (px(&knocked, 70, 70)[1] as f32, px(&behind, 70, 70)[1] as f32, px(&none, 70, 70)[1] as f32);
    assert!(gn - gb >= 8.0, "the fixture must cast a visible shadow at the centre: shadowed {gb}, bare {gn}");
    let share = (gk - gb) / (gn - gb);
    assert!((0.3..=0.7).contains(&share), "a mid-gray luminance mask must knock out about half the shadow, got {share:.2} ({gb} {gk} {gn})");
    // outside the mask the rect's corner removes nothing
    assert!(near(px(&knocked, 40, 40), px(&behind, 40, 40), 2), "{:?} vs {:?}", px(&knocked, 40, 40), px(&behind, 40, 40));
}
