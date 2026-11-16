//! Where the live (not stored-outline) stroke of a `Line` layer is centred.
//!
//! Measured on the Untitled UI kit's stored geometry: a LINE with CENTER alignment has its stroke
//! band at local y in [-w, 0] (centre -w/2, thickness w), also when the line is rotated, because the
//! rule holds in local space before the node transform. A ROUND cap is a stadium that stays inside
//! x in [0, L]; its semicircles of radius w/2 are centred w/2 in from each end.
//!
//! Every other case is unmeasured and keeps the plain centreline `(0, 0)-(L, 0)`: INSIDE and OUTSIDE
//! alignment, SQUARE caps, arrow and marker caps, and any dash pattern. Weights other than 1 and 2
//! take the rule linearly in w.
//!
//! The CPU renderer, the GPU tessellator, SVG and PDF export all take the line from here so they
//! cannot disagree.

use crate::{FillRule, Node, NodeKind, Path, PathCmd, Stroke, StrokeAlign, StrokeCap, Vec2};

/// The straight segment a `Line` is stroked along, in the layer's local space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LineCenterline {
    pub x0: f32,
    pub x1: f32,
    pub y: f32,
}

impl Node {
    /// The centreline the live stroke of this `Line` follows, or `None` when the node is not a line.
    ///
    /// Only what the Untitled UI kit measured moves: a solid CENTER line with a NONE or ROUND cap (7,219
    /// stored bands, weights 1 and 2, rotated ones included) is stroked at y = -w/2, and a ROUND cap is
    /// also inset by w/2 at each end. Every other case is unmeasured, so it keeps the plain centred
    /// stroke `(0, 0)-(L, 0)` exactly as before: INSIDE and OUTSIDE alignment, SQUARE caps, arrow and
    /// marker caps, and any dash pattern.
    pub fn line_centerline(&self) -> Option<LineCenterline> {
        if !matches!(self.kind, NodeKind::Line) {
            return None;
        }
        let len = self.size.x;
        let w = self.stroke.weight;
        let measured = self.stroke.align == StrokeAlign::Center
            && matches!(self.stroke.cap, StrokeCap::None | StrokeCap::Round)
            && !draws_dashes(&self.stroke)
            && w.is_finite()
            && w > 0.0;
        if !measured {
            return Some(LineCenterline { x0: 0.0, x1: len, y: 0.0 });
        }
        let inset = if self.stroke.cap == StrokeCap::Round { (w / 2.0).min(len.max(0.0) / 2.0) } else { 0.0 };
        Some(LineCenterline { x0: inset, x1: len - inset, y: -w / 2.0 })
    }
}

impl LineCenterline {
    /// The segment as a document path, `(x0, y)` to `(x1, y)`.
    pub fn to_path(self) -> Path {
        Path { cmds: vec![PathCmd::MoveTo(Vec2::new(self.x0, self.y)), PathCmd::LineTo(Vec2::new(self.x1, self.y))], fill_rule: FillRule::NonZero }
    }
}

impl Node {
    /// The path a `Line` becomes when it turns into a vector (point editing, Flatten, the Start point,
    /// End point and Swap controls): the centreline it is drawn on, so nothing moves on conversion.
    /// `None` when the node is not a line. This is the one place that decides it.
    pub fn line_vector_path(&self) -> Option<Path> {
        self.line_centerline().map(LineCenterline::to_path)
    }
}

/// Same test the renderers use: a pattern of non-negative lengths with a positive sum dashes the stroke.
fn draws_dashes(s: &Stroke) -> bool {
    !s.dashes.is_empty() && s.dashes.iter().all(|d| *d >= 0.0) && s.dashes.iter().sum::<f32>() > 0.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::NodeId;

    fn line(len: f32, w: f32, align: StrokeAlign, cap: StrokeCap) -> Node {
        let mut n = Node::new(NodeId(1), NodeKind::Line);
        n.size = Vec2::new(len, 0.0);
        n.stroke.weight = w;
        n.stroke.align = align;
        n.stroke.cap = cap;
        n
    }

    #[test]
    fn center_none_cap_shifts_up_by_half_the_weight() {
        let c = line(100.0, 2.0, StrokeAlign::Center, StrokeCap::None).line_centerline().unwrap();
        assert_eq!(c, LineCenterline { x0: 0.0, x1: 100.0, y: -1.0 });
    }

    #[test]
    fn round_cap_is_inset_by_half_the_weight_at_each_end() {
        let c = line(100.0, 10.0, StrokeAlign::Center, StrokeCap::Round).line_centerline().unwrap();
        assert_eq!(c, LineCenterline { x0: 5.0, x1: 95.0, y: -5.0 });
    }

    #[test]
    fn round_cap_on_a_line_shorter_than_the_weight_collapses_to_a_point() {
        let c = line(4.0, 10.0, StrokeAlign::Center, StrokeCap::Round).line_centerline().unwrap();
        assert_eq!((c.x0, c.x1), (2.0, 2.0));
    }

    #[test]
    fn square_arrow_and_dashed_lines_stay_on_the_axis() {
        for cap in [StrokeCap::Square, StrokeCap::ArrowLines, StrokeCap::TriangleFilled] {
            let c = line(100.0, 4.0, StrokeAlign::Center, cap).line_centerline().unwrap();
            assert_eq!(c, LineCenterline { x0: 0.0, x1: 100.0, y: 0.0 }, "{cap:?}");
        }
        for cap in [StrokeCap::None, StrokeCap::Round] {
            let mut n = line(100.0, 4.0, StrokeAlign::Center, cap);
            n.stroke.dashes = vec![10.0, 6.0];
            assert_eq!(n.line_centerline().unwrap(), LineCenterline { x0: 0.0, x1: 100.0, y: 0.0 }, "dashed {cap:?}");
            // a pattern that draws nothing different from solid is still solid
            n.stroke.dashes = vec![0.0, 0.0];
            assert_ne!(n.line_centerline().unwrap().y, 0.0, "a [0, 0] pattern is solid, {cap:?}");
        }
    }

    #[test]
    fn inside_and_outside_alignment_are_unchanged() {
        for align in [StrokeAlign::Inside, StrokeAlign::Outside] {
            let c = line(100.0, 4.0, align, StrokeCap::Round).line_centerline().unwrap();
            assert_eq!(c, LineCenterline { x0: 0.0, x1: 100.0, y: 0.0 });
        }
    }

    #[test]
    fn a_zero_weight_line_and_a_non_line_have_no_shift() {
        let c = line(100.0, 0.0, StrokeAlign::Center, StrokeCap::Round).line_centerline().unwrap();
        assert_eq!(c, LineCenterline { x0: 0.0, x1: 100.0, y: 0.0 });
        assert!(Node::new(NodeId(2), NodeKind::Rectangle).line_centerline().is_none());
    }

    #[test]
    fn the_vector_a_line_becomes_starts_on_the_drawn_centreline() {
        let path = |n: &Node| match n.line_vector_path().unwrap().cmds.as_slice() {
            [PathCmd::MoveTo(a), PathCmd::LineTo(b)] => ((a.x, a.y), (b.x, b.y)),
            other => panic!("one segment, got {other:?}"),
        };
        assert_eq!(path(&line(100.0, 10.0, StrokeAlign::Center, StrokeCap::Round)), ((5.0, -5.0), (95.0, -5.0)));
        assert_eq!(path(&line(100.0, 10.0, StrokeAlign::Center, StrokeCap::None)), ((0.0, -5.0), (100.0, -5.0)));
        assert_eq!(path(&line(100.0, 10.0, StrokeAlign::Center, StrokeCap::Square)), ((0.0, 0.0), (100.0, 0.0)));
        assert!(Node::new(NodeId(2), NodeKind::Rectangle).line_vector_path().is_none());
    }
}
