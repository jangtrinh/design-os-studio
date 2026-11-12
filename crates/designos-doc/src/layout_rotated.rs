//! How auto layout sees a child whose transform rotates or flips it.
//!
//! The solver works with axis-aligned slots, and a rotated or flipped child occupies its slot with
//! its bounding box (Figma lays out the same way). The solver is fed the box's extent; the answer
//! is written back through [`slot_write`], the one place that turns a solved slot into a local size
//! and a transform origin that puts the box's top-left corner on the slot. The headless solver, the
//! instance-constraint solver and the editor solver all write through it. Transforms are rotation
//! and flip only (no scale or shear).
//!
//! Three kinds of child, told apart by [`classify`]:
//! - plain (translation only): the slot is the local box, as before;
//! - quarter turn (90, 180, 270 degrees, any flip, within float-trigonometry error): Fill, Hug,
//!   min/max and stretch all act on the bounding box, and the slot maps back to an exact local
//!   size by swapping the axes when the turn does;
//! - any other angle: the child keeps its local size and acts as Fixed ([`fix_tilted_input`]):
//!   Fill, grow, stretch and min/max are ignored, and only its position follows the slot through
//!   its bounding box. A bounding box cannot be traced back to one local size (it is ill
//!   conditioned near 45 degrees and has no non-negative solution for many boxes), and solving it
//!   anyway compounds on every relayout. Figma's behaviour for Fill on a tilted layer is unverified.

use crate::layout_solver::{ChildIn, SizeLimits};
use crate::{Affine, AlignSelf, Rect, Vec2};

/// Component size below which a rotation matrix entry counts as zero.
const AXIS_EPS: f32 = 1e-4;

/// How a transform places a child's box in the flow.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Turn {
    /// Translation only: the local box is the slot.
    Plain,
    /// A multiple of 90 degrees or a flip; `swap` when the local axes land on the other page axes.
    Quarter { swap: bool },
    /// Any other rotation.
    Tilted,
}

pub(crate) fn classify(tf: &Affine) -> Turn {
    let axis_kept = tf.b.abs() <= AXIS_EPS && tf.c.abs() <= AXIS_EPS;
    let axis_swapped = tf.a.abs() <= AXIS_EPS && tf.d.abs() <= AXIS_EPS;
    if axis_kept {
        if tf.a < 0.0 || tf.d < 0.0 {
            Turn::Quarter { swap: false }
        } else {
            Turn::Plain
        }
    } else if axis_swapped {
        Turn::Quarter { swap: true }
    } else {
        Turn::Tilted
    }
}

/// Whether the local axes stay on the page axes, so a box width is still the local width.
pub(crate) fn keeps_axes(tf: &Affine) -> bool {
    matches!(classify(tf), Turn::Plain | Turn::Quarter { swap: false })
}

/// Whether the child's bounding box, not its local box, is what the flow positions.
pub(crate) fn bbox_applies(tf: &Affine) -> bool {
    classify(tf) != Turn::Plain
}

/// Width and height of the page-aligned box around a local `size` box under `tf`.
pub(crate) fn extent(tf: &Affine, size: Vec2) -> Vec2 {
    match classify(tf) {
        Turn::Plain | Turn::Quarter { swap: false } => size,
        Turn::Quarter { swap: true } => Vec2::new(size.y, size.x),
        Turn::Tilted => Vec2::new(tf.a.abs() * size.x + tf.c.abs() * size.y, tf.b.abs() * size.x + tf.d.abs() * size.y),
    }
}

/// The local size of a child under `tf` whose bounding box has extent `target`; `current` for a
/// tilted child, whose local size never follows its slot.
pub(crate) fn local_size(tf: &Affine, target: Vec2, current: Vec2) -> Vec2 {
    match classify(tf) {
        Turn::Plain | Turn::Quarter { swap: false } => target,
        Turn::Quarter { swap: true } => Vec2::new(target.y, target.x),
        Turn::Tilted => current,
    }
}

/// What the solver's slot for one child becomes in the document.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SlotWrite {
    /// The child's transform with its origin moved so its bounding box starts at the slot corner.
    pub transform: Affine,
    /// The child's local size.
    pub size: Vec2,
    /// The bounding-box extent of `size` under `transform`, what the solver reads back next time.
    pub extent: Vec2,
}

/// Maps the solved `slot` of a child with transform `tf` and local size `current` to what to store.
pub fn slot_write(tf: &Affine, current: Vec2, slot: Rect) -> SlotWrite {
    let size = local_size(tf, Vec2::new(slot.w, slot.h), current);
    let (off_x, off_y) =
        if bbox_applies(tf) { ((tf.a * size.x).min(0.0) + (tf.c * size.y).min(0.0), (tf.b * size.x).min(0.0) + (tf.d * size.y).min(0.0)) } else { (0.0, 0.0) };
    SlotWrite { transform: Affine { tx: slot.x - off_x, ty: slot.y - off_y, ..*tf }, size, extent: extent(tf, size) }
}

/// Whether the solved `slot` still differs from the box the child has, so another solve pass can
/// change something. A tilted child never takes the slot's size, so it never counts.
pub fn slot_drifted(tf: &Affine, size: Vec2, slot: Rect) -> bool {
    if classify(tf) == Turn::Tilted {
        return false;
    }
    let have = extent(tf, size);
    (have.x - slot.w).abs() > 1e-3 || (have.y - slot.h).abs() > 1e-3
}

/// Makes a tilted child behave as Fixed for the solver: no Fill, grow, stretch or limits.
pub(crate) fn fix_tilted_input(tf: &Affine, input: &mut ChildIn) {
    if classify(tf) != Turn::Tilted {
        return;
    }
    input.main_fill = false;
    input.cross_fill = false;
    input.grow = 0.0;
    if input.align_self == AlignSelf::Stretch {
        input.align_self = AlignSelf::Auto;
    }
    input.limits = SizeLimits::default();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn turn(deg: f32) -> Affine {
        Affine::rotate(deg.to_radians())
    }

    fn box_min(tf: Affine, size: Vec2) -> Vec2 {
        let b = crate::lint::rules::aabb_of_transformed_rect(tf, size);
        Vec2::new(b.x, b.y)
    }

    #[test]
    fn classify_tells_quarter_turns_from_float_trig_noise() {
        assert_eq!(classify(&Affine::translate(5.0, 6.0)), Turn::Plain);
        for deg in [90.0, 270.0, -90.0] {
            assert_eq!(classify(&turn(deg)), Turn::Quarter { swap: true }, "{deg}");
        }
        assert_eq!(classify(&turn(180.0)), Turn::Quarter { swap: false });
        assert_eq!(classify(&turn(0.0)), Turn::Plain);
        assert_eq!(classify(&Affine { a: -1.0, ..Affine::IDENTITY }), Turn::Quarter { swap: false });
        for deg in [10.0, 30.0, 44.0, 45.0, 60.0, 150.0] {
            assert_eq!(classify(&turn(deg)), Turn::Tilted, "{deg}");
        }
    }

    #[test]
    fn plain_transform_is_untouched() {
        let tf = Affine::translate(5.0, 6.0);
        assert_eq!(extent(&tf, Vec2::new(72.0, 18.0)), Vec2::new(72.0, 18.0));
        let w = slot_write(&tf, Vec2::new(72.0, 18.0), Rect::new(1.0, 2.0, 30.0, 40.0));
        assert_eq!((w.transform.tx, w.transform.ty, w.size), (1.0, 2.0, Vec2::new(30.0, 40.0)));
    }

    #[test]
    fn quarter_turn_slot_maps_to_an_exact_local_size_and_origin() {
        for deg in [90.0, 180.0, 270.0, -90.0] {
            let tf = turn(deg);
            let w = slot_write(&tf, Vec2::new(72.0, 18.0), Rect::new(10.0, 20.0, 18.0, 300.0));
            let swapped = deg != 180.0;
            let want = if swapped { Vec2::new(300.0, 18.0) } else { Vec2::new(18.0, 300.0) };
            assert_eq!(w.size, want, "{deg}");
            let min = box_min(w.transform, w.size);
            assert!((min.x - 10.0).abs() < 1e-3 && (min.y - 20.0).abs() < 1e-3, "{deg}: {min:?}");
            assert_eq!(w.extent, Vec2::new(18.0, 300.0), "{deg}");
        }
    }

    #[test]
    fn tilted_child_keeps_its_size_and_the_box_lands_on_the_slot() {
        for deg in [10.0, 30.0, 44.0, 45.0, 60.0, 150.0] {
            let tf = turn(deg);
            let size = Vec2::new(40.0, 10.0);
            let w = slot_write(&tf, size, Rect::new(7.0, 9.0, 999.0, 999.0));
            assert_eq!(w.size, size, "{deg}");
            let min = box_min(w.transform, w.size);
            assert!((min.x - 7.0).abs() < 1e-3 && (min.y - 9.0).abs() < 1e-3, "{deg}: {min:?}");
            assert!(!slot_drifted(&tf, size, Rect::new(0.0, 0.0, 999.0, 999.0)), "{deg}");
        }
    }

    #[test]
    fn tilted_input_loses_fill_grow_stretch_and_limits() {
        let mut input = ChildIn {
            main_fill: true,
            cross_fill: true,
            grow: 2.0,
            align_self: AlignSelf::Stretch,
            limits: SizeLimits { min_w: Some(5.0), ..Default::default() },
            ..Default::default()
        };
        fix_tilted_input(&turn(90.0), &mut input);
        assert!(input.main_fill && input.cross_fill && input.limits.min_w.is_some(), "a quarter turn keeps them");
        fix_tilted_input(&turn(30.0), &mut input);
        assert!(!input.main_fill && !input.cross_fill && input.grow == 0.0 && input.align_self == AlignSelf::Auto);
        assert_eq!(input.limits.min_w, None);
    }
}
