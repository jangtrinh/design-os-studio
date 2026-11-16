//! Where the live (not stored-outline) stroke of a `Line` layer is centred.
//!
//! Measured on the Untitled UI kit's stored geometry: a LINE with CENTER alignment has its stroke
//! band at local y in [-w, 0] (centre -w/2, thickness w), also when the line is rotated, because the
//! rule holds in local space before the node transform. A ROUND cap is a stadium that stays inside
//! x in [0, L]; its semicircles of radius w/2 are centred w/2 in from each end.
//!
//! Every other case is unmeasured and keeps the plain centreline `(0, 0)-(L, 0)`: INSIDE and OUTSIDE
//! alignment, SQUARE caps, arrow and marker caps (so an arrow on a LINE rides the shifted axis but
//! is not inset), and weights other than 1 and 2 (the rule is applied linearly in w).
//!
//! The CPU renderer, the GPU tessellator, SVG and PDF export all take the line from here so they
//! cannot disagree.

use crate::{Node, NodeKind, StrokeAlign, StrokeCap};

/// The straight segment a `Line` is stroked along, in the layer's local space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LineCenterline {
    pub x0: f32,
    pub x1: f32,
    pub y: f32,
}

impl Node {
    /// The centreline the live stroke of this `Line` follows, or `None` when the node is not a line.
    pub fn line_centerline(&self) -> Option<LineCenterline> {
        if !matches!(self.kind, NodeKind::Line) {
            return None;
        }
        let len = self.size.x;
        let w = self.stroke.weight;
        if self.stroke.align != StrokeAlign::Center || !w.is_finite() || w <= 0.0 {
            return Some(LineCenterline { x0: 0.0, x1: len, y: 0.0 });
        }
        let inset = if self.stroke.cap == StrokeCap::Round { (w / 2.0).min(len.max(0.0) / 2.0) } else { 0.0 };
        Some(LineCenterline { x0: inset, x1: len - inset, y: -w / 2.0 })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{NodeId, Vec2};

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
    fn square_and_arrow_caps_keep_their_x_extent() {
        for cap in [StrokeCap::Square, StrokeCap::ArrowLines, StrokeCap::TriangleFilled] {
            let c = line(100.0, 4.0, StrokeAlign::Center, cap).line_centerline().unwrap();
            assert_eq!(c, LineCenterline { x0: 0.0, x1: 100.0, y: -2.0 });
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
}
