//! How auto layout sees a child whose transform rotates or flips it.
//!
//! The solver works with axis-aligned slots. A rotated or flipped child occupies the slot with its
//! bounding box (Figma lays out the same way), so the solver is fed the box's extent and its answer
//! is written back as a local size plus a transform origin that lands the box's top-left corner on
//! the solved slot. A child whose transform has no rotation, flip or shear skips all of this
//! ([`bbox_applies`] is false) and keeps the plain size/translation round trip.
//!
//! Sizing flags (Fill, Hug, Fixed), min/max limits and `align_self` are read as stored and apply to
//! the bounding box along the parent's axes. A Fill that changes the box extent is traced back to
//! a local size ([`local_size_for_extent`]); where that is not possible (a 45 degree turn maps both
//! local axes onto one diagonal) the child keeps its size and only its position follows the slot.

use crate::{Affine, Vec2};

/// Below this the 2x2 extent system is treated as singular.
const SINGULAR_EPS: f32 = 1e-4;

/// Whether the child's bounding box, not its local box, is what the flow positions.
pub(crate) fn bbox_applies(tf: &Affine) -> bool {
    tf.b.abs() > 1e-4 || tf.c.abs() > 1e-4 || tf.a < 0.0 || tf.d < 0.0
}

/// Whether the transform keeps the local axes on the page axes (flips and half turns only), so a
/// bounding-box width is still the local width.
pub(crate) fn keeps_axes(tf: &Affine) -> bool {
    tf.b.abs() <= 1e-4 && tf.c.abs() <= 1e-4
}

/// Width and height of the page-aligned box around a local `size` box under `tf`.
pub(crate) fn extent(tf: &Affine, size: Vec2) -> Vec2 {
    if !bbox_applies(tf) {
        return size;
    }
    Vec2::new(tf.a.abs() * size.x + tf.c.abs() * size.y, tf.b.abs() * size.x + tf.d.abs() * size.y)
}

/// The local size whose bounding box has `target` extent. `current` is returned when it already
/// has that extent (so an untouched child never drifts) and when the system cannot be solved.
pub(crate) fn local_size_for_extent(tf: &Affine, target: Vec2, current: Vec2) -> Vec2 {
    if !bbox_applies(tf) {
        return target;
    }
    let have = extent(tf, current);
    if (have.x - target.x).abs() <= 1e-3 && (have.y - target.y).abs() <= 1e-3 {
        return current;
    }
    let (a, c, b, d) = (tf.a.abs(), tf.c.abs(), tf.b.abs(), tf.d.abs());
    let det = a * d - c * b;
    if det.abs() < SINGULAR_EPS {
        return current;
    }
    Vec2::new(((d * target.x - c * target.y) / det).max(0.0), ((a * target.y - b * target.x) / det).max(0.0))
}

/// `tf` moved so the bounding box of a local `size` box has its top-left corner at `slot`.
pub(crate) fn placed_at_slot(tf: Affine, size: Vec2, slot: Vec2) -> Affine {
    let (off_x, off_y) =
        if bbox_applies(&tf) { ((tf.a * size.x).min(0.0) + (tf.c * size.y).min(0.0), (tf.b * size.x).min(0.0) + (tf.d * size.y).min(0.0)) } else { (0.0, 0.0) };
    Affine { tx: slot.x - off_x, ty: slot.y - off_y, ..tf }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn quarter(turns: i32) -> Affine {
        let (s, c) = match turns.rem_euclid(4) {
            0 => (0.0, 1.0),
            1 => (1.0, 0.0),
            2 => (0.0, -1.0),
            _ => (-1.0, 0.0),
        };
        Affine { a: c, b: s, c: -s, d: c, tx: 0.0, ty: 0.0 }
    }

    #[test]
    fn plain_transform_is_untouched() {
        let tf = Affine::translate(5.0, 6.0);
        assert!(!bbox_applies(&tf));
        assert_eq!(extent(&tf, Vec2::new(72.0, 18.0)), Vec2::new(72.0, 18.0));
        assert_eq!(placed_at_slot(tf, Vec2::new(72.0, 18.0), Vec2::new(1.0, 2.0)).tx, 1.0);
    }

    #[test]
    fn quarter_turn_swaps_extent_and_lands_the_box_on_the_slot() {
        for turns in 1..=3 {
            let tf = quarter(turns);
            let size = Vec2::new(72.0, 18.0);
            let ext = extent(&tf, size);
            let want = if turns == 2 { size } else { Vec2::new(18.0, 72.0) };
            assert!((ext.x - want.x).abs() < 1e-3 && (ext.y - want.y).abs() < 1e-3, "turns {turns}: {ext:?}");
            let placed = placed_at_slot(tf, size, Vec2::new(10.0, 20.0));
            let p: Vec<Vec2> = [(0.0, 0.0), (72.0, 0.0), (0.0, 18.0), (72.0, 18.0)].iter().map(|&(x, y)| placed.apply(Vec2::new(x, y))).collect();
            let min_x = p.iter().map(|v| v.x).fold(f32::MAX, f32::min);
            let min_y = p.iter().map(|v| v.y).fold(f32::MAX, f32::min);
            assert!((min_x - 10.0).abs() < 1e-3 && (min_y - 20.0).abs() < 1e-3, "turns {turns}: {min_x},{min_y}");
        }
    }

    #[test]
    fn arbitrary_angle_box_lands_on_the_slot() {
        let tf = Affine::rotate(30f32.to_radians());
        let size = Vec2::new(40.0, 10.0);
        let placed = placed_at_slot(tf, size, Vec2::new(0.0, 0.0));
        let b = crate::lint::rules::aabb_of_transformed_rect(placed, size);
        assert!(b.x.abs() < 1e-3 && b.y.abs() < 1e-3);
        let ext = extent(&tf, size);
        assert!((b.w - ext.x).abs() < 1e-3 && (b.h - ext.y).abs() < 1e-3);
    }

    #[test]
    fn extent_round_trips_to_the_local_size() {
        let tf = quarter(1);
        let got = local_size_for_extent(&tf, Vec2::new(18.0, 300.0), Vec2::new(72.0, 18.0));
        assert_eq!(got, Vec2::new(300.0, 18.0));
        let tf = Affine::rotate(30f32.to_radians());
        let got = local_size_for_extent(&tf, extent(&tf, Vec2::new(60.0, 20.0)), Vec2::new(40.0, 10.0));
        assert!((got.x - 60.0).abs() < 1e-2 && (got.y - 20.0).abs() < 1e-2);
    }

    #[test]
    fn unchanged_or_unsolvable_extent_keeps_the_current_size() {
        let tf = Affine::rotate(45f32.to_radians());
        let cur = Vec2::new(40.0, 10.0);
        assert_eq!(local_size_for_extent(&tf, extent(&tf, cur), cur), cur);
        assert_eq!(local_size_for_extent(&tf, Vec2::new(500.0, 500.0), cur), cur);
    }
}
