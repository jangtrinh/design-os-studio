//! Figma's dash rule, measured on the outlines a `.fig` stores for dashed strokes.
//!
//! Every segment of a path (a side, a corner arc, a vector-network segment) is dashed on its own, the way a
//! continuous dash over the whole path would not do:
//!
//! - the pattern of the segment is stretched so that the segment holds a whole number of periods, the nearest
//!   `n = round(length / period)` and at least one, and every dash and gap grows or shrinks by
//!   `length / (n * period)`. A segment shorter than one dash is that one dash from end to end;
//! - the first dash is centred on the start of the segment, and so the last one on its end: a segment starts and
//!   ends with half a dash;
//! - the half dashes either side of a vertex are one dash. It runs round the vertex and takes the stroke's join
//!   there, which is the notch a mitred corner shows. A path that is not closed has half a dash at both its ends.
//!
//! A flattened path (a boolean result is nothing but short lines) has no segments in this sense; callers keep
//! tiny-skia's continuous dash for those.

use tiny_skia as sk;
use tiny_skia::PathSegment;

/// How near a dash must end to the end of its segment to run on into the next one.
const JOIN_TOLERANCE: f32 = 1e-2;
/// Segments shorter than this carry no dash.
const MIN_SEGMENT: f32 = 1e-3;
/// The dashes tiny-skia cuts in one call at most; more than this over a whole path draws nothing.
const MAX_DASHES: f64 = 1_000_000.0;
const CURVE_STEPS: usize = 48;
/// The resolution a segment is measured and cut at, whatever the device scale: dash ends sit in the same place on the
/// canvas, the GPU and the PDF.
const DASH_RES_SCALE: f32 = 16.0;

/// `dashes` as a dash array: an odd array repeats, and `None` when it dashes nothing (empty, a negative or
/// non-finite entry, or no length at all).
pub fn dash_array(dashes: &[f32]) -> Option<Vec<f32>> {
    if dashes.is_empty() || dashes.iter().any(|d| !d.is_finite() || *d < 0.0) || dashes.iter().sum::<f32>() <= 0.0 {
        return None;
    }
    let mut v = dashes.to_vec();
    if v.len() % 2 == 1 {
        v.extend_from_within(..);
    }
    Some(v)
}

#[derive(Clone, Copy)]
enum Seg {
    Line(sk::Point, sk::Point),
    Quad(sk::Point, sk::Point, sk::Point),
    Cubic(sk::Point, sk::Point, sk::Point, sk::Point),
}

impl Seg {
    fn start(self) -> sk::Point {
        match self {
            Seg::Line(a, _) | Seg::Quad(a, ..) | Seg::Cubic(a, ..) => a,
        }
    }

    fn end(self) -> sk::Point {
        match self {
            Seg::Line(_, b) | Seg::Quad(_, _, b) | Seg::Cubic(_, _, _, b) => b,
        }
    }

    fn at(self, t: f32) -> sk::Point {
        let u = 1.0 - t;
        match self {
            Seg::Line(a, b) => sk::Point::from_xy(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t),
            Seg::Quad(a, c, b) => sk::Point::from_xy(u * u * a.x + 2.0 * u * t * c.x + t * t * b.x, u * u * a.y + 2.0 * u * t * c.y + t * t * b.y),
            Seg::Cubic(a, c1, c2, b) => sk::Point::from_xy(
                u * u * u * a.x + 3.0 * u * u * t * c1.x + 3.0 * u * t * t * c2.x + t * t * t * b.x,
                u * u * u * a.y + 3.0 * u * u * t * c1.y + 3.0 * u * t * t * c2.y + t * t * t * b.y,
            ),
        }
    }

    fn length(self) -> f32 {
        let steps = if matches!(self, Seg::Line(..)) { 1 } else { CURVE_STEPS };
        let mut prev = self.start();
        let mut total = 0.0;
        for i in 1..=steps {
            let p = self.at(i as f32 / steps as f32);
            total += (p.x - prev.x).hypot(p.y - prev.y);
            prev = p;
        }
        total
    }

    fn path(self) -> Option<sk::Path> {
        let mut pb = sk::PathBuilder::new();
        let a = self.start();
        pb.move_to(a.x, a.y);
        match self {
            Seg::Line(_, b) => pb.line_to(b.x, b.y),
            Seg::Quad(_, c, b) => pb.quad_to(c.x, c.y, b.x, b.y),
            Seg::Cubic(_, c1, c2, b) => pb.cubic_to(c1.x, c1.y, c2.x, c2.y, b.x, b.y),
        }
        pb.finish()
    }
}

/// One dash: a move and the draw commands after it.
type Piece = Vec<PathSegment>;

fn near(a: sk::Point, b: sk::Point) -> bool {
    (a.x - b.x).hypot(a.y - b.y) <= JOIN_TOLERANCE
}

fn last_point(piece: &Piece) -> Option<sk::Point> {
    piece.iter().rev().find_map(|s| match *s {
        PathSegment::MoveTo(p) | PathSegment::LineTo(p) | PathSegment::QuadTo(_, p) | PathSegment::CubicTo(_, _, p) => Some(p),
        PathSegment::Close => None,
    })
}

/// The dashes of one segment, in order: the first starts at its start, the last ends at its end.
fn dash_segment(seg: Seg, array: &[f32], period: f32) -> Vec<Piece> {
    let length = seg.length();
    if length < MIN_SEGMENT {
        return Vec::new();
    }
    if length < array[0] {
        // too short for its first dash: the segment is that dash
        return seg.path().map(|p| p.segments().collect::<Piece>()).into_iter().collect();
    }
    let periods = (length / period).round().max(1.0);
    let scale = length / (periods * period);
    let stretched: Vec<f32> = array.iter().map(|a| a * scale).collect();
    let phase = stretched[0] / 2.0;
    let (Some(dash), Some(path)) = (sk::StrokeDash::new(stretched, phase), seg.path()) else { return Vec::new() };
    let Some(dashed) = path.dash(&dash, DASH_RES_SCALE) else { return Vec::new() };
    let mut pieces: Vec<Piece> = Vec::new();
    for s in dashed.segments() {
        match s {
            PathSegment::MoveTo(_) => pieces.push(vec![s]),
            _ => {
                if let Some(p) = pieces.last_mut() {
                    p.push(s);
                }
            }
        }
    }
    pieces
}

/// Appends the draw commands of `next` (its move dropped) to `piece`, which ends where `next` starts.
fn run_on(piece: &mut Piece, next: Piece) {
    piece.extend(next.into_iter().skip(1));
}

/// The dashes of one contour of segments; `closed` lets the last dash run on into the first.
fn dash_contour(segs: &[Seg], closed: bool, array: &[f32], period: f32) -> Vec<Piece> {
    let mut out: Vec<Piece> = Vec::new();
    // the dash that reaches the end of the segment last dashed, waiting to run on into the next one
    let mut tail: Option<Piece> = None;
    let mut first_is_head = false;
    for (i, seg) in segs.iter().enumerate() {
        let mut pieces = dash_segment(*seg, array, period);
        if pieces.is_empty() {
            out.extend(tail.take());
            continue;
        }
        if i == 0 {
            first_is_head = true;
        }
        if let Some(mut t) = tail.take() {
            let head = pieces.remove(0);
            run_on(&mut t, head);
            if pieces.is_empty() {
                // the dash covers the whole segment
                if last_point(&t).is_some_and(|p| near(p, seg.end())) {
                    tail = Some(t);
                } else {
                    out.push(t);
                }
                continue;
            }
            out.push(t);
        }
        if pieces.last().and_then(last_point).is_some_and(|p| near(p, seg.end())) {
            tail = pieces.pop();
        }
        out.extend(pieces);
    }
    if let Some(mut t) = tail {
        if closed && first_is_head && !out.is_empty() && out[0].first().is_some_and(|s| matches!(s, PathSegment::MoveTo(p) if near(*p, segs[0].start()))) {
            let head = out.remove(0);
            run_on(&mut t, head);
        }
        out.push(t);
    }
    out
}

thread_local! {
    static CONTINUOUS: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// While alive, [`dash_path`] on this thread dashes continuously along each contour, for geometry that is a
/// flattened result and so has no segments of Figma's kind. Guards nest: dropping one restores what was set before.
pub struct ContinuousDashes {
    before: bool,
}

impl ContinuousDashes {
    pub fn begin() -> Self {
        Self { before: CONTINUOUS.with(|c| c.replace(true)) }
    }
}

impl Drop for ContinuousDashes {
    fn drop(&mut self) {
        CONTINUOUS.with(|c| c.set(self.before));
    }
}

/// `path` dashed by `array`: one pattern per segment (see the module docs), or one pattern along each contour
/// while a [`ContinuousDashes`] guard is alive.
pub fn dash_path(path: &sk::Path, array: &[f32], res_scale: f32) -> Option<sk::Path> {
    if CONTINUOUS.with(|c| c.get()) {
        return path.dash(&sk::StrokeDash::new(array.to_vec(), 0.0)?, res_scale);
    }
    dash_each_segment(path, array)
}

/// `path` cut into dashes by Figma's rule (see the module docs): open contours, ready to be stroked with the
/// stroke's own cap and join but no dash. `array` comes from [`dash_array`].
pub fn dash_each_segment(path: &sk::Path, array: &[f32]) -> Option<sk::Path> {
    let period: f32 = array.iter().sum();
    if period <= 0.0 || array.len() < 2 {
        return None;
    }
    let contours = contours_of(path);
    // tiny-skia stops at a million dashes in one call, and dashing a path in one call bounds the path; cut segment by
    // segment the bound is per segment, so the dashes of the whole path are counted before any is cut
    let dashes: f64 = contours.iter().flat_map(|(segs, _)| segs).map(|seg| dash_count(*seg, array, period)).sum();
    if dashes.is_nan() || dashes > MAX_DASHES {
        return None;
    }
    let mut out = sk::PathBuilder::new();
    for (segs, closed) in &contours {
        for piece in dash_contour(segs, *closed, array, period) {
            for s in piece {
                match s {
                    PathSegment::MoveTo(p) => out.move_to(p.x, p.y),
                    PathSegment::LineTo(p) => out.line_to(p.x, p.y),
                    PathSegment::QuadTo(c, p) => out.quad_to(c.x, c.y, p.x, p.y),
                    PathSegment::CubicTo(c1, c2, p) => out.cubic_to(c1.x, c1.y, c2.x, c2.y, p.x, p.y),
                    PathSegment::Close => {}
                }
            }
        }
    }
    out.finish()
}

/// The segments of every contour of `path`, with whether the contour is closed. A closed contour ends with the line
/// back to its start when the path does not draw it.
fn contours_of(path: &sk::Path) -> Vec<(Vec<Seg>, bool)> {
    let mut out: Vec<(Vec<Seg>, bool)> = Vec::new();
    let mut segs: Vec<Seg> = Vec::new();
    let (mut start, mut cur) = (sk::Point::zero(), sk::Point::zero());
    for s in path.segments() {
        match s {
            PathSegment::MoveTo(p) => {
                out.push((std::mem::take(&mut segs), false));
                (start, cur) = (p, p);
            }
            PathSegment::LineTo(p) => {
                segs.push(Seg::Line(cur, p));
                cur = p;
            }
            PathSegment::QuadTo(c, p) => {
                segs.push(Seg::Quad(cur, c, p));
                cur = p;
            }
            PathSegment::CubicTo(c1, c2, p) => {
                segs.push(Seg::Cubic(cur, c1, c2, p));
                cur = p;
            }
            PathSegment::Close => {
                if !near(cur, start) {
                    segs.push(Seg::Line(cur, start));
                }
                out.push((std::mem::take(&mut segs), true));
                cur = start;
            }
        }
    }
    out.push((segs, false));
    out.retain(|(segs, _)| !segs.is_empty());
    out
}

/// How many dashes [`dash_segment`] cuts of `seg`.
fn dash_count(seg: Seg, array: &[f32], period: f32) -> f64 {
    let length = seg.length();
    if length < MIN_SEGMENT {
        return 0.0;
    }
    if length < array[0] {
        return 1.0;
    }
    f64::from((length / period).round().max(1.0)) * (array.len() / 2) as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(x0: f32, y0: f32, x1: f32, y1: f32) -> sk::Path {
        let mut pb = sk::PathBuilder::new();
        pb.move_to(x0, y0);
        pb.line_to(x1, y1);
        pb.finish().unwrap()
    }

    /// The `(start, end)` distance along a vertical line of each dash.
    fn spans(p: &sk::Path) -> Vec<(f32, f32)> {
        let mut out = Vec::new();
        let mut from = 0.0;
        for s in p.segments() {
            match s {
                PathSegment::MoveTo(q) => from = q.y,
                PathSegment::LineTo(q) => out.push((from, q.y)),
                _ => {}
            }
        }
        out
    }

    #[test]
    fn a_segment_holds_a_whole_number_of_stretched_periods_with_half_dashes_at_both_ends() {
        // 16 / 3.1 = 5.16 periods round to 5: period 3.2, the pattern stretched to 1.03
        let dashed = dash_each_segment(&line(0.0, 0.0, 0.0, 16.0), &[0.1, 3.0]).unwrap();
        let spans = spans(&dashed);
        assert_eq!(spans.len(), 6);
        let dash = 0.1 * (16.0 / 5.0) / 3.1;
        assert!((spans[0].0 - 0.0).abs() < 1e-4 && (spans[0].1 - dash / 2.0).abs() < 1e-3, "half a dash at the start: {:?}", spans[0]);
        for (k, s) in spans.iter().enumerate().take(5).skip(1) {
            let centre = (s.0 + s.1) / 2.0;
            assert!((centre - k as f32 * 16.0 / 5.0).abs() < 1e-3, "dash {k} is centred on a period: {s:?}");
            assert!((s.1 - s.0 - dash).abs() < 1e-3);
        }
        assert!((spans[5].1 - 16.0).abs() < 1e-4 && (spans[5].0 - (16.0 - dash / 2.0)).abs() < 1e-3, "half a dash at the end: {:?}", spans[5]);
    }

    #[test]
    fn the_count_is_the_nearest_whole_number_of_periods() {
        // 752 / 15 = 50.13 periods: 50 of 15.04, so the middle of the segment (50 is even) carries a dash of
        // 10 * 15.04 / 15 centred on 376
        let dashed = dash_each_segment(&line(0.0, 0.0, 0.0, 752.0), &[10.0, 5.0]).unwrap();
        let fifty_one = spans(&dashed);
        assert_eq!(fifty_one.len(), 51);
        let middle = fifty_one[25];
        assert!(((middle.0 + middle.1) / 2.0 - 376.0).abs() < 1e-2 && (middle.1 - middle.0 - 10.0 * 15.04 / 15.0).abs() < 1e-2, "{middle:?}");
        // 118 is 7.87 periods: 8 of 14.75
        let dashed = dash_each_segment(&line(0.0, 0.0, 0.0, 118.0), &[10.0, 5.0]).unwrap();
        let eight = spans(&dashed);
        assert_eq!(eight.len(), 9);
        assert!((eight[1].1 - eight[1].0 - 10.0 * 118.0 / 120.0).abs() < 1e-3);
    }

    #[test]
    fn a_length_of_whole_periods_is_not_stretched() {
        // 66 is 33 periods of 2: dashes of 1 centred on 0, 2, 4 ...
        let dashed = dash_each_segment(&line(0.0, 0.0, 0.0, 66.0), &[1.0, 1.0]).unwrap();
        let spans = spans(&dashed);
        assert_eq!(spans.len(), 34);
        assert!((spans[1].0 - 1.5).abs() < 1e-3 && (spans[1].1 - 2.5).abs() < 1e-3, "{:?}", spans[1]);
    }

    #[test]
    fn a_segment_shorter_than_a_dash_is_that_dash_and_one_a_little_longer_keeps_a_gap() {
        // 7.85 is shorter than the dash of 10: the whole segment is dash
        let dashed = dash_each_segment(&line(0.0, 0.0, 0.0, 7.85), &[10.0, 5.0]).unwrap();
        let solid = spans(&dashed);
        assert_eq!(solid.len(), 1);
        assert!((solid[0].0 - 0.0).abs() < 1e-4 && (solid[0].1 - 7.85).abs() < 1e-4);
        // 12.5 holds one period of 12.5: a dash of 8.33, half of it at each end, and a gap of 4.17 between
        let dashed = dash_each_segment(&line(0.0, 0.0, 0.0, 12.5), &[10.0, 5.0]).unwrap();
        let halves = spans(&dashed);
        assert_eq!(halves.len(), 2);
        assert!((halves[0].1 - 12.5 / 3.0).abs() < 1e-3 && (halves[1].0 - 12.5 * 2.0 / 3.0).abs() < 1e-3, "{halves:?}");
    }

    #[test]
    fn the_half_dashes_either_side_of_a_corner_are_one_dash() {
        // two sides of 30 (two periods of 15 each): the corner dash turns the corner in one piece
        let mut pb = sk::PathBuilder::new();
        pb.move_to(0.0, 0.0);
        pb.line_to(30.0, 0.0);
        pb.line_to(30.0, 30.0);
        let dashed = dash_each_segment(&pb.finish().unwrap(), &[10.0, 5.0]).unwrap();
        let corner = dashed.segments().collect::<Vec<_>>();
        let turns = corner.windows(3).any(|w| matches!((w[0], w[1], w[2]), (PathSegment::MoveTo(a), PathSegment::LineTo(b), PathSegment::LineTo(c)) if a.y == 0.0 && b.x == 30.0 && b.y == 0.0 && c.x == 30.0));
        assert!(turns, "the dash that reaches the corner carries on round it");
    }

    #[test]
    fn a_closed_path_has_no_open_ends() {
        let mut pb = sk::PathBuilder::new();
        pb.move_to(0.0, 0.0);
        pb.line_to(30.0, 0.0);
        pb.line_to(30.0, 30.0);
        pb.line_to(0.0, 30.0);
        pb.close();
        let dashed = dash_each_segment(&pb.finish().unwrap(), &[10.0, 5.0]).unwrap();
        let moves = dashed.segments().filter(|s| matches!(s, PathSegment::MoveTo(_))).count();
        // four sides of two periods: one dash per period plus the shared corner dashes: 4 corners + 4 mid dashes
        assert_eq!(moves, 8);
    }

    #[test]
    fn nested_guards_restore_the_flag_they_found() {
        let line = line(0.0, 0.0, 0.0, 66.0);
        let segments = |p: &sk::Path| p.segments().filter(|s| matches!(s, PathSegment::MoveTo(_))).count();
        let outer = ContinuousDashes::begin();
        {
            let _inner = ContinuousDashes::begin();
        }
        assert_eq!(segments(&dash_path(&line, &[1.0, 1.0], 1.0).unwrap()), 33, "still continuous: 66 / 2 dashes");
        drop(outer);
        assert_eq!(segments(&dash_path(&line, &[1.0, 1.0], 1.0).unwrap()), 34, "per segment again: half dashes at both ends");
    }

    #[test]
    fn a_path_of_many_segments_is_bounded_by_the_dashes_of_the_whole_path() {
        // 100 segments of 900 with 0.001 on, 0.001 off: 450,000 dashes each, 45 million in all. Each segment is under
        // tiny-skia's cap of a million dashes, the path is not: it draws nothing, as one continuous dash would.
        let mut pb = sk::PathBuilder::new();
        pb.move_to(0.0, 0.0);
        for i in 1..=100 {
            pb.line_to(if i % 2 == 1 { 900.0 } else { 0.0 }, i as f32);
        }
        let path = pb.finish().unwrap();
        let started = std::time::Instant::now();
        assert!(dash_each_segment(&path, &[0.001, 0.001]).is_none());
        assert!(started.elapsed() < std::time::Duration::from_secs(1), "the count is made before any dash is cut");
        // a path whose dashes are all counted but few still draws
        assert!(dash_each_segment(&path, &[100.0, 100.0]).is_some());
    }

    #[test]
    fn arrays_that_dash_nothing_are_refused() {
        assert!(dash_array(&[]).is_none());
        assert!(dash_array(&[0.0, 0.0]).is_none());
        assert!(dash_array(&[-1.0, 2.0]).is_none());
        assert_eq!(dash_array(&[4.0, 2.0, 1.0]).unwrap(), vec![4.0, 2.0, 1.0, 4.0, 2.0, 1.0]);
    }
}
