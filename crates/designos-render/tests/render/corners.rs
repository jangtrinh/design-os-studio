use crate::common;
use common::*;
use designos_doc::*;
use designos_render::tiny_skia::Pixmap;

fn rounded(kind: fn(NodeId, Rect) -> Node, smoothing: f32, radius: f32) -> Pixmap {
    let mut t = T::new();
    let r = t.add(None, |id| {
        let mut n = kind(id, Rect::new(10.0, 10.0, 100.0, 100.0));
        n.fills = vec![Paint::solid(RED)];
        n
    });
    t.node(r).corner_radii = [radius; 4];
    t.node(r).corner_smoothing = smoothing;
    t.render(120, 120)
}

fn diff_pixels(a: &Pixmap, b: &Pixmap, x0: u32, y0: u32, x1: u32, y1: u32) -> usize {
    let mut n = 0;
    for y in y0..y1 {
        for x in x0..x1 {
            if !near(px(a, x, y), px(b, x, y), 2) {
                n += 1;
            }
        }
    }
    n
}

#[test]
fn zero_smoothing_matches_plain_rounding() {
    for kind in [Node::rectangle as fn(NodeId, Rect) -> Node, Node::frame] {
        let a = rounded(kind, 0.0, 24.0);
        let mut t = T::new();
        // reference: the same shape built from an explicit cubic-rounded path
        let k = 0.552_284_8 * 24.0;
        let v = Vec2::new;
        let path = Path {
            cmds: vec![
                PathCmd::MoveTo(v(24.0, 0.0)),
                PathCmd::LineTo(v(76.0, 0.0)),
                PathCmd::CubicTo(v(76.0 + k, 0.0), v(100.0, 24.0 - k), v(100.0, 24.0)),
                PathCmd::LineTo(v(100.0, 76.0)),
                PathCmd::CubicTo(v(100.0, 76.0 + k), v(76.0 + k, 100.0), v(76.0, 100.0)),
                PathCmd::LineTo(v(24.0, 100.0)),
                PathCmd::CubicTo(v(24.0 - k, 100.0), v(0.0, 76.0 + k), v(0.0, 76.0)),
                PathCmd::LineTo(v(0.0, 24.0)),
                PathCmd::CubicTo(v(0.0, 24.0 - k), v(24.0 - k, 0.0), v(24.0, 0.0)),
                PathCmd::Close,
            ],
            fill_rule: FillRule::NonZero,
        };
        t.add(None, |id| {
            let mut n = Node::new(id, NodeKind::Vector { paths: vec![path], network: None });
            n.transform = Affine::translate(10.0, 10.0);
            n.size = Vec2::new(100.0, 100.0);
            n.fills = vec![Paint::solid(RED)];
            n
        });
        let b = t.render(120, 120);
        assert_eq!(diff_pixels(&a, &b, 0, 0, 120, 120), 0);
    }
}

#[test]
fn smoothing_changes_corner_pixels_only() {
    let plain = rounded(Node::rectangle, 0.0, 24.0);
    let smooth = rounded(Node::rectangle, 0.6, 24.0);
    // the squircle curve leaves the straight edge earlier and cuts the corner more
    let corner = diff_pixels(&plain, &smooth, 10, 10, 50, 50);
    assert!(corner > 20, "corner region differs: {corner} pixels");
    // along the top edge the squircle curve starts 1.6 * r = 38px from the corner,
    // while a plain arc is already straight 24px from it
    assert!(!near(px(&plain, 30, 10), px(&smooth, 30, 10), 2), "{:?} {:?}", px(&plain, 30, 10), px(&smooth, 30, 10));
    // edge centres and the middle are untouched
    assert_eq!(diff_pixels(&plain, &smooth, 55, 10, 65, 20), 0);
    assert_eq!(diff_pixels(&plain, &smooth, 40, 40, 80, 80), 0);
    // the shape still reaches the box on all sides
    assert_eq!(px(&smooth, 60, 10), [255, 0, 0, 255]);
    assert_eq!(px(&smooth, 109, 60), [255, 0, 0, 255]);
}

#[test]
fn smoothing_is_symmetric_across_corners() {
    let pm = rounded(Node::rectangle, 0.6, 24.0);
    // 4-fold symmetry of a square: compare mirrored sample points
    for (dx, dy) in [(2u32, 2u32), (6, 2), (2, 9), (14, 4)] {
        let tl = px(&pm, 10 + dx, 10 + dy);
        let tr = px(&pm, 109 - dx, 10 + dy);
        let bl = px(&pm, 10 + dx, 109 - dy);
        let br = px(&pm, 109 - dx, 109 - dy);
        assert!(near(tl, tr, 3) && near(tl, bl, 3) && near(tl, br, 3), "{dx},{dy}: {tl:?} {tr:?} {bl:?} {br:?}");
    }
}

#[test]
fn smoothing_applies_to_frames_and_clips_children() {
    let mut t = T::new();
    let f = t.add(None, |id| Node::frame(id, Rect::new(10.0, 10.0, 100.0, 100.0)));
    t.node(f).corner_radii = [24.0; 4];
    t.node(f).corner_smoothing = 0.6;
    t.node(f).fills = vec![Paint::solid(BLUE)];
    if let NodeKind::Frame(p) = &mut t.node(f).kind {
        p.clips_content = true;
    }
    t.rect(Some(f), Rect::new(0.0, 0.0, 100.0, 100.0), RED);
    let pm = t.render(120, 120);
    assert_eq!(px(&pm, 11, 11), WHITE, "corner clipped");
    assert_eq!(px(&pm, 60, 60), [255, 0, 0, 255]);
    let plain = rounded(Node::frame, 0.0, 24.0);
    let smooth = rounded(Node::frame, 0.6, 24.0);
    assert!(diff_pixels(&plain, &smooth, 10, 10, 50, 50) > 20);
}

#[test]
fn smoothing_is_limited_by_side_length() {
    // radius = half the side: no room for smoothing, the shape stays a circle-ish pill
    let a = rounded(Node::rectangle, 0.0, 50.0);
    let b = rounded(Node::rectangle, 1.0, 50.0);
    assert!(diff_pixels(&a, &b, 0, 0, 120, 120) < 8);
}

#[test]
fn polygon_corner_radius_rounds_vertices() {
    let hex = |radius: f32| {
        let mut t = T::new();
        let id = t.add(None, |id| {
            let mut n = Node::new(id, NodeKind::Polygon { count: 6 });
            n.transform = Affine::translate(5.0, 5.0);
            n.size = Vec2::new(70.0, 70.0);
            n.fills = vec![Paint::solid(RED)];
            n.corner_radii = [radius, 0.0, 0.0, 0.0];
            n
        });
        let _ = id;
        t.render(80, 80)
    };
    let sharp = hex(0.0);
    let round = hex(30.0);
    // top vertex at (40, 5): the sharp tip is filled just below it; a 30px radius
    // flattens it by ~4.6px
    assert!(px(&sharp, 40, 7)[1] < 80, "sharp tip filled: {:?}", px(&sharp, 40, 7));
    assert!(px(&round, 40, 7)[1] > 200, "rounded tip is empty: {:?}", px(&round, 40, 7));
    assert_eq!(px(&round, 40, 40), [255, 0, 0, 255]);
    // edge midpoints stay put
    assert!(near(px(&sharp, 40, 20), px(&round, 40, 20), 2));
}

#[test]
fn star_corner_radius_rounds_tips() {
    let star = |radius: f32| {
        let mut t = T::new();
        t.add(None, |id| {
            let mut n = Node::new(id, NodeKind::Star { count: 5, inner_ratio: 0.45 });
            n.transform = Affine::translate(5.0, 5.0);
            n.size = Vec2::new(70.0, 70.0);
            n.fills = vec![Paint::solid(RED)];
            n.corner_radii = [radius, 0.0, 0.0, 0.0];
            n
        });
        t.render(80, 80)
    };
    let sharp = star(0.0);
    let round = star(6.0);
    // count red-ish pixels: rounding removes area at the five tips
    let area = |pm: &Pixmap| (0..80).flat_map(|y| (0..80).map(move |x| (x, y))).filter(|(x, y)| px(pm, *x, *y)[1] < 128).count();
    assert!(area(&round) + 20 < area(&sharp), "{} vs {}", area(&round), area(&sharp));
}
