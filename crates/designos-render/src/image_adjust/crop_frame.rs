//! Crop frames: the node-UV to picture-UV transform of an image fill in Crop mode, kept per node while the
//! paint picker edits one paint for a whole selection.

use super::crop_transform_from_framing;
use designos_doc::{ImageScaleMode, Paint, PaintKind, Vec2};

/// The crop transform that stretches the picture over the node box (the Crop default).
pub const IDENTITY_CROP_TRANSFORM: [f32; 6] = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0];

/// Turns the picture shown by crop transform `m` a quarter turn about the centre of a node of `size`, in node
/// pixels, so the picture rotates rigidly and whatever sits under the node centre stays there. `clockwise`
/// picks the direction (the one [`designos_doc::image_uv_map`] gives a rotation of 90 degrees in Fill).
/// `None` for a degenerate `size` or a non-finite `m`.
pub fn rotate_crop_transform(m: [f32; 6], size: Vec2, clockwise: bool) -> Option<[f32; 6]> {
    if !(size.x.is_finite() && size.y.is_finite() && size.x > 0.0 && size.y > 0.0 && m.iter().all(|v| v.is_finite())) {
        return None;
    }
    // A node point (u, v) shows what the turned-back point (u_old, v_old) showed: the picture moved rigidly.
    let (k, l) = (size.y / size.x, size.x / size.y);
    let [a0, a1, a2, b0, b1, b2] = m;
    let turned = if clockwise {
        // u_old = 0.5 - 0.5 k + k v, v_old = 0.5 + 0.5 l - l u
        [-a1 * l, a0 * k, a2 + a0 * (0.5 - 0.5 * k) + a1 * (0.5 + 0.5 * l), -b1 * l, b0 * k, b2 + b0 * (0.5 - 0.5 * k) + b1 * (0.5 + 0.5 * l)]
    } else {
        // u_old = 0.5 + 0.5 k - k v, v_old = 0.5 - 0.5 l + l u
        [a1 * l, -a0 * k, a2 + a0 * (0.5 + 0.5 * k) + a1 * (0.5 - 0.5 * l), b1 * l, -b0 * k, b2 + b0 * (0.5 + 0.5 * k) + b1 * (0.5 - 0.5 * l)]
    };
    turned.iter().all(|v| v.is_finite()).then_some(turned)
}

/// Sets a paint's scale mode without touching its transform (the framing is unknown, so none is invented).
pub fn set_image_scale_mode(paint: &mut Paint, next: ImageScaleMode) {
    if let PaintKind::Image { scale_mode, .. } = &mut paint.kind {
        *scale_mode = next;
    }
}

/// Settles the image paint the picker writes onto one node.
///
/// `old` is that node's paint before the picker's current edit, `next` the picker's paint with the edit, and
/// `quarter_turns` the rotate clicks of this edit (clockwise positive). Only an image paint that stays on the same
/// picture is touched; any other edit is written as it is. A Crop frame is always the node's own:
/// - entering Crop from Fill, Fit or Tile keeps the frame the node stored, else frames the picture as the node
///   showed it, from the node's own size (`image_size` is asked only then). When the picture size is unknown the
///   node keeps its stored frame, or none (the identity): no frame is invented;
/// - a node that is already in Crop keeps its frame while a slider or another control moves, and turns it about its
///   own centre when the edit carries rotate clicks;
/// - the angle is always the node's own, plus the turns of this edit;
/// - in any other mode each node keeps the frame it stored, so a trip through Fill, Fit or Tile and back to Crop
///   does not hand one node's frame to the rest; a rotate click there drops it.
pub fn crop_paint_for_node(old: &Paint, next: &mut Paint, size: Vec2, image_size: impl FnOnce() -> Option<(u32, u32)>, quarter_turns: i32) {
    let (PaintKind::Image { image_ref: old_ref, scale_mode: old_mode, params: old_params }, PaintKind::Image { image_ref, scale_mode, params }) =
        (&old.kind, &mut next.kind)
    else {
        return;
    };
    if old_ref != image_ref {
        return;
    }
    params.rotation = turned_rotation(old_params.rotation, quarter_turns);
    if *scale_mode != ImageScaleMode::Crop {
        // Outside Crop the frame is only kept for a later return: every node keeps its own, and a rotate click
        // drops it, because the framing it was made for no longer applies. The angle is the node's own too:
        // Fill, Fit and Tile draw from it, and Crop restores its label from it on the way back.
        params.transform = if quarter_turns == 0 { old_params.transform } else { None };
        return;
    }
    let base = if *old_mode == ImageScaleMode::Crop {
        old_params.transform
    } else {
        old_params.transform.or_else(|| image_size().and_then(|(iw, ih)| crop_transform_from_framing(size, iw as f32, ih as f32, *old_mode, old_params)))
    };
    // Three clockwise turns are one counter-clockwise turn, which keeps the arithmetic to a single step.
    let (steps, clockwise) = match quarter_turns.rem_euclid(4) {
        3 => (1, false),
        n => (n, true),
    };
    params.transform = if steps == 0 {
        base
    } else {
        let turned = (0..steps).try_fold(base.unwrap_or(IDENTITY_CROP_TRANSFORM), |m, _| rotate_crop_transform(m, size, clockwise));
        turned.or(base)
    };
}

/// A node's own angle after `quarter_turns` rotate clicks (clockwise positive); an angle that was not turned is
/// kept verbatim, whatever its value.
fn turned_rotation(own: f32, quarter_turns: i32) -> f32 {
    if quarter_turns == 0 {
        own
    } else {
        (own + 90.0 * quarter_turns as f32).rem_euclid(360.0)
    }
}
