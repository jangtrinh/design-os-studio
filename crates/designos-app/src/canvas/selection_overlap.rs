//! Keeps selection chrome from covering what it marks: a selected connector's outline breaks
//! at its label (the renderer paints the label on a white box over the line), and the size
//! badge drops below the bottom magnet dot when a layer shows its magnets.

use crate::theme;
use egui::{Pos2, Rect};

/// Top edge (screen y) of the size badge under a selection whose lowest point is at `bottom`.
/// With the selection's magnet dots showing, the badge sits under the bottom dot, at its
/// hovered size, so the dot stays visible and can still be grabbed.
pub(crate) fn size_badge_top(bottom: f32, below_magnet: bool) -> f32 {
    if below_magnet {
        bottom + theme::BOARD_MAGNET_GAP + theme::BOARD_MAGNET_RADIUS * 1.3 + theme::CANVAS_BADGE_OFFSET
    } else {
        bottom + theme::CANVAS_BADGE_OFFSET + 3.0
    }
}

/// The runs of polyline `pts` that lie outside `r`, in order. A segment crossing `r` is cut at
/// its edges; runs shorter than two points are dropped.
pub(crate) fn polyline_outside_rect(pts: &[Pos2], r: Rect) -> Vec<Vec<Pos2>> {
    let mut runs: Vec<Vec<Pos2>> = Vec::new();
    let mut cur: Vec<Pos2> = Vec::new();
    for w in pts.windows(2) {
        let (a, b) = (w[0], w[1]);
        match inside_span(a, b, r) {
            None => {
                if cur.is_empty() {
                    cur.push(a);
                }
                cur.push(b);
            }
            Some((t0, t1)) => {
                if t0 > 0.0 {
                    if cur.is_empty() {
                        cur.push(a);
                    }
                    cur.push(a.lerp(b, t0));
                }
                if cur.len() >= 2 {
                    runs.push(std::mem::take(&mut cur));
                }
                cur.clear();
                if t1 < 1.0 {
                    cur.push(a.lerp(b, t1));
                    cur.push(b);
                }
            }
        }
    }
    if cur.len() >= 2 {
        runs.push(cur);
    }
    runs
}

/// Parameter range `[t0, t1]` of segment `a`–`b` inside `r` (Liang–Barsky), or `None` when the
/// segment misses `r` or only grazes it at one point.
fn inside_span(a: Pos2, b: Pos2, r: Rect) -> Option<(f32, f32)> {
    let d = b - a;
    let (mut t0, mut t1) = (0.0f32, 1.0f32);
    for (p, q) in [(-d.x, a.x - r.min.x), (d.x, r.max.x - a.x), (-d.y, a.y - r.min.y), (d.y, r.max.y - a.y)] {
        if p == 0.0 {
            if q < 0.0 {
                return None;
            }
        } else {
            let t = q / p;
            if p < 0.0 {
                t0 = t0.max(t);
            } else {
                t1 = t1.min(t);
            }
        }
    }
    (t1 - t0 > 1e-4).then_some((t0, t1))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: f32, y: f32) -> Pos2 {
        Pos2::new(x, y)
    }

    fn assert_runs(actual: Vec<Vec<Pos2>>, expected: Vec<Vec<Pos2>>) {
        assert_eq!(actual.len(), expected.len(), "runs: {actual:?}");
        for (a, e) in actual.iter().zip(&expected) {
            assert_eq!(a.len(), e.len(), "run {a:?} vs {e:?}");
            for (pa, pe) in a.iter().zip(e) {
                assert!((*pa - *pe).length() < 1e-3, "point {pa:?} vs {pe:?} in {actual:?}");
            }
        }
    }

    #[test]
    fn a_line_through_the_label_box_is_cut_at_its_edges() {
        let label = Rect::from_min_max(p(40.0, -10.0), p(60.0, 10.0));
        let runs = polyline_outside_rect(&[p(0.0, 0.0), p(100.0, 0.0)], label);
        assert_runs(runs, vec![vec![p(0.0, 0.0), p(40.0, 0.0)], vec![p(60.0, 0.0), p(100.0, 0.0)]]);
    }

    #[test]
    fn an_elbow_keeps_its_corner_and_loses_only_the_label_part() {
        // label centred on the middle (vertical) leg of an elbow route
        let label = Rect::from_min_max(p(45.0, 20.0), p(55.0, 30.0));
        let runs = polyline_outside_rect(&[p(0.0, 0.0), p(50.0, 0.0), p(50.0, 50.0), p(100.0, 50.0)], label);
        assert_runs(runs, vec![vec![p(0.0, 0.0), p(50.0, 0.0), p(50.0, 20.0)], vec![p(50.0, 30.0), p(50.0, 50.0), p(100.0, 50.0)]]);
    }

    #[test]
    fn a_line_away_from_the_label_box_stays_whole() {
        let label = Rect::from_min_max(p(40.0, 20.0), p(60.0, 30.0));
        let pts = vec![p(0.0, 0.0), p(100.0, 0.0), p(100.0, 80.0)];
        assert_runs(polyline_outside_rect(&pts, label), vec![pts]);
    }

    #[test]
    fn a_line_starting_on_the_box_edge_keeps_its_first_point() {
        let label = Rect::from_min_max(p(40.0, -10.0), p(60.0, 10.0));
        let runs = polyline_outside_rect(&[p(60.0, 0.0), p(100.0, 0.0)], label);
        assert_runs(runs, vec![vec![p(60.0, 0.0), p(100.0, 0.0)]]);
    }

    #[test]
    fn a_line_inside_the_label_box_draws_nothing() {
        let label = Rect::from_min_max(p(0.0, -5.0), p(100.0, 5.0));
        assert!(polyline_outside_rect(&[p(10.0, 0.0), p(90.0, 0.0)], label).is_empty());
    }

    #[test]
    fn the_size_badge_clears_the_bottom_magnet_dot_at_its_hovered_size() {
        let bottom = 300.0;
        let dot_bottom = bottom + theme::BOARD_MAGNET_GAP + theme::BOARD_MAGNET_RADIUS * 1.3;
        assert!(size_badge_top(bottom, true) > dot_bottom, "badge must start below the dot");
        // without magnets the badge keeps its usual place right under the selection
        assert_eq!(size_badge_top(bottom, false), bottom + theme::CANVAS_BADGE_OFFSET + 3.0);
        assert!(size_badge_top(bottom, false) < dot_bottom, "the old place overlapped the dot");
    }
}
