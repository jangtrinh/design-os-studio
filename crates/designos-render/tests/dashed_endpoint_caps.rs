//! A dashed open stroke caps every dash with the stroke's own cap, and the cap of an end vertex shows
//! only at that end of the path: where the first dash starts or the last dash ends. A dash that never
//! reaches an end of the path takes no end vertex cap, and arrowheads sit on top of the dashed shaft.
//!
//! The line runs at `y = 20` from `x = 10`, weight 4, dashes `[10, 6]`, so the stroke fills rows 18..22.

mod common;

use common::*;
use designos_doc::*;
use designos_render::tiny_skia::Pixmap;
use designos_render::vectorops::{outline_stroke, paths_area};

const INK: u8 = 0x18;
const VIEW: (u32, u32) = (120, 40);
const X0: f32 = 10.0;
const Y: f32 = 20.0;

fn stroke(cap: StrokeCap) -> Stroke {
    Stroke {
        paints: vec![Paint::solid(Color::rgb(INK, 0x1D, 0x27))],
        weight: 4.0,
        align: StrokeAlign::Center,
        cap,
        join: StrokeJoin::Miter,
        dashes: vec![10.0, 6.0],
        ..Stroke::default()
    }
}

/// A dashed line of `len` whose end vertices carry `caps`, or a plain `Line` node when `caps` is `None`.
fn dashed_line(t: &mut T, len: f32, stroke_cap: StrokeCap, caps: Option<(StrokeCap, StrokeCap)>) -> NodeId {
    t.add(None, |id| {
        let mut n = match caps {
            Some((start, end)) => {
                let path = Path { cmds: vec![PathCmd::MoveTo(Vec2::ZERO), PathCmd::LineTo(Vec2::new(len, 0.0))], fill_rule: FillRule::NonZero };
                let mut net = VectorNetwork::from_paths(std::slice::from_ref(&path));
                net.vertices[0].cap = Some(start);
                net.vertices[1].cap = Some(end);
                Node::new(id, NodeKind::Vector { paths: vec![path], network: Some(net) })
            }
            None => Node::new(id, NodeKind::Line),
        };
        n.transform = Affine::translate(X0, Y);
        n.size = Vec2::new(len, 0.0);
        n.fills.clear();
        n.stroke = stroke(stroke_cap);
        n
    })
}

fn render(len: f32, stroke_cap: StrokeCap, caps: Option<(StrokeCap, StrokeCap)>) -> Pixmap {
    let mut t = T::new();
    dashed_line(&mut t, len, stroke_cap, caps);
    t.render(VIEW.0, VIEW.1)
}

/// Coverage of the stroke colour in pixel `(x, y)`: 0 on the white page, 1 where the stroke is solid.
fn coverage(pm: &Pixmap, x: u32, y: u32) -> f32 {
    (255.0 - px(pm, x, y)[0] as f32) / (255.0 - INK as f32)
}

/// Total coverage in the square of pixels from `(x0, y0)` up to `(x1, y1)`.
fn ink(pm: &Pixmap, x0: u32, y0: u32, x1: u32, y1: u32) -> f32 {
    (y0..y1).flat_map(|y| (x0..x1).map(move |x| (x, y))).map(|(x, y)| coverage(pm, x, y)).sum()
}

/// Columns in `x_range` where the two renders differ at all.
fn differing_columns(a: &Pixmap, b: &Pixmap, x_range: std::ops::Range<u32>) -> Vec<u32> {
    x_range.filter(|x| (0..a.height()).any(|y| px(a, *x, y) != px(b, *x, y))).collect()
}

const ROUND: StrokeCap = StrokeCap::Round;
const SQUARE: StrokeCap = StrokeCap::Square;

#[test]
fn a_square_end_on_a_dashed_round_line_squares_only_the_end_dash() {
    // dashes cover x = 10..20, 26..36, 42..52, 58..68, 74..84 and 90..94: the path ends inside the last dash
    let plain = render(84.0, ROUND, None);
    let capped = render(84.0, ROUND, Some((ROUND, SQUARE)));

    let changed = differing_columns(&plain, &capped, 0..88);
    assert!(changed.is_empty(), "dashes before the last one are drawn as on a plain round dashed line, columns {changed:?} differ");
    for x in [21, 37, 53, 69, 85] {
        assert!(coverage(&capped, x, 19) > 0.5, "the inner dash end next to column {x} keeps its round cap, coverage {}", coverage(&capped, x, 19));
    }

    // a round cap leaves the corners of the 2px past the end bare, a square cap fills them
    for y in [18, 21] {
        assert!(coverage(&capped, 95, y) > 0.9, "the end terminal is square: pixel (95, {y}) has coverage {}", coverage(&capped, 95, y));
        assert!(coverage(&plain, 95, y) < 0.6, "the plain round end has a rounded corner: pixel (95, {y}) has coverage {}", coverage(&plain, 95, y));
    }
    assert!(coverage(&capped, 8, 18) < 0.6, "the start terminal stays round, coverage {}", coverage(&capped, 8, 18));
}

#[test]
fn a_square_start_on_a_dashed_round_line_squares_only_the_first_dash() {
    let plain = render(80.0, ROUND, None);
    let capped = render(80.0, ROUND, Some((SQUARE, ROUND)));

    for y in [18, 21] {
        assert!(coverage(&capped, 8, y) > 0.9, "the start terminal is square: pixel (8, {y}) has coverage {}", coverage(&capped, 8, y));
        assert!(coverage(&plain, 8, y) < 0.6, "the plain round start has a rounded corner: pixel (8, {y}) has coverage {}", coverage(&plain, 8, y));
    }
    assert!(coverage(&capped, 21, 19) > 0.5, "the first dash keeps a round cap at its inner end, coverage {}", coverage(&capped, 21, 19));
    let changed = differing_columns(&plain, &capped, 24..100);
    assert!(changed.is_empty(), "no dash but the first one changes, columns {changed:?} differ");
}

#[test]
fn an_end_cap_does_not_show_where_the_path_ends_in_a_gap() {
    // 80 is five whole periods: the last dash ends at x = 84 and the path ends at x = 90, inside the gap
    let plain = render(80.0, ROUND, None);
    let capped = render(80.0, ROUND, Some((ROUND, SQUARE)));
    let changed = differing_columns(&plain, &capped, 0..VIEW.0);
    assert!(changed.is_empty(), "a square end in a gap draws nothing, columns {changed:?} differ");
    assert_eq!(ink(&capped, 86, 0, VIEW.0, VIEW.1), 0.0, "nothing is painted past the last dash");
}

#[test]
fn a_none_end_on_a_dashed_round_line_leaves_the_end_dash_butt() {
    let capped = render(84.0, ROUND, Some((ROUND, StrokeCap::None)));
    assert!(coverage(&capped, 94, 19) < 0.05, "the end terminal is butt: pixel (94, 19) has coverage {}", coverage(&capped, 94, 19));
    assert!(coverage(&capped, 93, 19) > 0.95, "the end dash reaches the end of the path");
    assert!(coverage(&capped, 21, 19) > 0.5, "the inner dash ends stay round");
}

#[test]
fn a_line_arrow_end_on_a_dashed_round_line_draws_the_head_and_keeps_round_dashes() {
    let plain = render(84.0, ROUND, None);
    let arrow = render(84.0, ROUND, Some((ROUND, StrokeCap::ArrowLines)));

    // the wings of the head run back from the tip at x = 94 along the diagonals
    for (x, y) in [(87, 26), (87, 13)] {
        assert!(ink(&arrow, x, y, x + 1, y + 1) > 0.5, "a wing of the head covers pixel ({x}, {y}), coverage {}", ink(&arrow, x, y, x + 1, y + 1));
        assert!(ink(&plain, x, y, x + 1, y + 1) < 0.05, "the plain dashed line has no head at pixel ({x}, {y})");
    }
    let changed = differing_columns(&plain, &arrow, 0..78);
    assert!(changed.is_empty(), "the shaft keeps its round dashes before the head, columns {changed:?} differ");
    for x in [21, 37, 53, 69] {
        assert!(coverage(&arrow, x, 19) > 0.5, "the inner dash end next to column {x} keeps its round cap, coverage {}", coverage(&arrow, x, 19));
    }
}

#[test]
fn an_arrow_head_start_on_a_dashed_round_line_keeps_round_inner_dashes() {
    let plain = render(84.0, ROUND, None);
    let arrow = render(84.0, ROUND, Some((StrokeCap::ArrowEquilateral, ROUND)));
    // the head is five weights long and the shaft starts at x = 28, so its dashes cover 28..38, 44..54, 60..70, 76..86 and 92..94
    assert!(ink(&arrow, 28, 12, 29, 13) > 0.9, "the arrowhead is drawn at the start: coverage {}", ink(&arrow, 28, 12, 29, 13));
    assert!(ink(&plain, 28, 12, 29, 13) < 0.05, "the plain dashed line has no head there");
    for x in [39, 55, 71, 87] {
        assert!(coverage(&arrow, x, 19) > 0.5, "the inner dash end next to column {x} keeps its round cap, coverage {}", coverage(&arrow, x, 19));
    }
    assert!(coverage(&arrow, 95, 19) > 0.2, "the end terminal keeps its round cap, coverage {}", coverage(&arrow, 95, 19));
}

#[test]
fn a_dashed_closed_path_next_to_an_arrow_keeps_its_round_dashes() {
    let rect = Path {
        cmds: vec![
            PathCmd::MoveTo(Vec2::new(0.0, 0.0)),
            PathCmd::LineTo(Vec2::new(40.0, 0.0)),
            PathCmd::LineTo(Vec2::new(40.0, 20.0)),
            PathCmd::LineTo(Vec2::new(0.0, 20.0)),
            PathCmd::Close,
        ],
        fill_rule: FillRule::NonZero,
    };
    let line = Path { cmds: vec![PathCmd::MoveTo(Vec2::new(60.0, 10.0)), PathCmd::LineTo(Vec2::new(110.0, 10.0))], fill_rule: FillRule::NonZero };
    let render_vector = |paths: Vec<Path>, end_cap: Option<StrokeCap>| -> Pixmap {
        let mut t = T::new();
        t.add(None, |id| {
            let net = VectorNetwork::from_paths(&paths);
            let mut net = net;
            if let Some(cap) = end_cap {
                let last = net.vertices.len() - 1;
                net.vertices[last].cap = Some(cap);
            }
            let mut n = Node::new(id, NodeKind::Vector { paths: net.to_paths(), network: Some(net) });
            n.transform = Affine::translate(5.0, 10.0);
            n.size = Vec2::new(110.0, 20.0);
            n.fills.clear();
            n.stroke = stroke(ROUND);
            n
        });
        t.render(VIEW.0, VIEW.1)
    };
    let plain = render_vector(vec![rect.clone(), line.clone()], None);
    let arrow = render_vector(vec![rect, line], Some(StrokeCap::ArrowLines));
    let changed = differing_columns(&plain, &arrow, 0..55);
    assert!(changed.is_empty(), "the closed path is drawn as without the arrow, columns {changed:?} differ");
}

#[test]
fn outlining_a_dashed_stroke_keeps_the_dash_caps_and_the_end_caps_of_the_vertices() {
    let path = Path { cmds: vec![PathCmd::MoveTo(Vec2::new(0.0, 0.0)), PathCmd::LineTo(Vec2::new(84.0, 0.0))], fill_rule: FillRule::NonZero };
    let mut net = VectorNetwork::from_paths(std::slice::from_ref(&path));
    net.vertices[0].cap = Some(ROUND);
    net.vertices[1].cap = Some(SQUARE);
    let outline = outline_stroke(&net.to_paths(), &stroke(ROUND), Some(&net));
    // five whole dashes with a round cap at each end, and a last dash 4 long with a round start and a square end
    let disc = std::f64::consts::PI * 4.0;
    let expected = 5.0 * (40.0 + disc) + (16.0 + disc / 2.0 + 8.0);
    let got = paths_area(&outline, 0.02);
    assert!((got - expected).abs() < 0.6, "outline area {got}, expected {expected}");
}
