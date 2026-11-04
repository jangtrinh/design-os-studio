//! Adjusted image fills on the CPU.
//!
//! The picture is sampled first (same texture, UV map, filter and spread as an unadjusted fill) into a scratch the
//! size of the fill's device bounds, clipped to the target. The adjustments then run on those sampled pixels, as the
//! GPU does per fragment, so a blended pixel at a hard edge is adjusted as one colour and the cost follows the pixels
//! on screen, never the size of the picture.

use super::apply_pixmap_adjustments;
use crate::tiny_skia as sk;
use designos_doc::ImageAdjust;
use std::sync::atomic::{AtomicU64, Ordering};

/// Largest scratch, in pixels, an adjusted fill may allocate: the bound the shadow and blur regions use.
pub(crate) const MAX_ADJUSTED_FILL_PIXELS: u64 = 48_000_000;

#[cfg(test)]
thread_local! {
    /// Lets a test shrink the scratch limit on this thread.
    pub(crate) static TEST_MAX_PIXELS: std::cell::Cell<Option<u64>> = const { std::cell::Cell::new(None) };
}

/// The scratch limit in effect: [`MAX_ADJUSTED_FILL_PIXELS`] outside tests.
pub(crate) fn max_adjusted_fill_pixels() -> u64 {
    #[cfg(test)]
    if let Some(limit) = TEST_MAX_PIXELS.with(|l| l.get()) {
        return limit;
    }
    MAX_ADJUSTED_FILL_PIXELS
}

/// Device-space pixel rectangle `(x0, y0, x1, y1)`, `x1 > x0` and `y1 > y0`.
pub(crate) type DeviceRegion = (i32, i32, i32, i32);

static UNADJUSTED_FALLBACKS: AtomicU64 = AtomicU64::new(0);

/// Number of adjusted image fills drawn without their adjustments because the scratch they needed was larger than
/// [`MAX_ADJUSTED_FILL_PIXELS`] or could not be allocated, since the process started.
pub fn unadjusted_fill_fallbacks() -> u64 {
    UNADJUSTED_FALLBACKS.load(Ordering::Relaxed)
}

pub(crate) fn note_unadjusted_fallback() {
    UNADJUSTED_FALLBACKS.fetch_add(1, Ordering::Relaxed);
}

/// Samples `src` through `pattern_ts` (picture pixels to node-local space) and `ts` (node-local space to target
/// pixels) over every pixel of `region`, then adjusts the sampled colours. `None` when the scratch would exceed
/// `max_pixels` or cannot be allocated.
///
/// The scratch holds the picture at opacity 1 and normal blending; paint opacity and blend mode belong to the
/// final composite. It is filled as one rectangle with an identity transform so no pixel is covered twice by the
/// anti-aliased edge of the fill.
pub(crate) fn adjusted_fill_region(
    src: sk::PixmapRef<'_>,
    spread: sk::SpreadMode,
    quality: sk::FilterQuality,
    pattern_ts: sk::Transform,
    ts: &sk::Transform,
    region: DeviceRegion,
    adjust: &ImageAdjust,
) -> Option<sk::Pixmap> {
    let (x0, y0, x1, y1) = region;
    let (w, h) = (u32::try_from(x1.checked_sub(x0)?).ok()?, u32::try_from(y1.checked_sub(y0)?).ok()?);
    if w == 0 || h == 0 || u64::from(w) * u64::from(h) > max_adjusted_fill_pixels() {
        return None;
    }
    let mut scratch = sk::Pixmap::new(w, h)?;
    let to_scratch = pattern_ts.post_concat(*ts).post_translate(-(x0 as f32), -(y0 as f32));
    let paint = sk::Paint { shader: sk::Pattern::new(src, spread, quality, 1.0, to_scratch), anti_alias: false, ..Default::default() };
    scratch.fill_rect(sk::Rect::from_xywh(0.0, 0.0, w as f32, h as f32)?, &paint, sk::Transform::identity(), None);
    apply_pixmap_adjustments(&mut scratch.as_mut(), adjust);
    Some(scratch)
}
