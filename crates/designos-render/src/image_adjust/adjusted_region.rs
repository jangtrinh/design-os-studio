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

/// Largest image-fill scratch, in pixels, that a bounded fill may allocate.
/// The scratch follows the visible device region, so it is normally much
/// smaller than the decoded picture.
pub(crate) const MAX_FILL_PIXELS: u64 = 48_000_000;

#[cfg(test)]
thread_local! {
    /// Lets a test shrink the scratch limit on this thread.
    pub(crate) static TEST_MAX_PIXELS: std::cell::Cell<Option<u64>> = const { std::cell::Cell::new(None) };
}

/// The scratch limit in effect: [`MAX_FILL_PIXELS`] outside tests.
pub(crate) fn max_fill_pixels() -> u64 {
    #[cfg(test)]
    if let Some(limit) = TEST_MAX_PIXELS.with(|l| l.get()) {
        return limit;
    }
    MAX_FILL_PIXELS
}

/// Device-space pixel rectangle `(x0, y0, x1, y1)`, `x1 > x0` and `y1 > y0`.
pub(crate) type DeviceRegion = (i32, i32, i32, i32);

static UNADJUSTED_FALLBACKS: AtomicU64 = AtomicU64::new(0);

/// Number of image fills drawn without their optional adjustments or bounded
/// border sampling because the scratch they needed was too large or could not
/// be allocated, since the process started.
pub fn unadjusted_fill_fallbacks() -> u64 {
    UNADJUSTED_FALLBACKS.load(Ordering::Relaxed)
}

pub(crate) fn note_unadjusted_fallback() {
    UNADJUSTED_FALLBACKS.fetch_add(1, Ordering::Relaxed);
}

/// How the picture is sampled into the scratch.
pub(crate) struct FillSource<'a> {
    /// The picture as decoded (or its reduced copy), never a padded copy of it.
    pub src: sk::PixmapRef<'a>,
    pub spread: sk::SpreadMode,
    pub quality: sk::FilterQuality,
    /// Picture pixels to node-local space.
    pub pattern_ts: sk::Transform,
    /// Fit letterbox and out-of-bounds Crop: samples that fall outside the picture are transparent.
    pub clear_outside_picture: bool,
}

/// Samples the picture through `source.pattern_ts` and `ts` over every pixel
/// of `region`. `None` when the scratch would exceed [`max_fill_pixels`] or
/// cannot be allocated.
pub(crate) fn sampled_fill_region(source: &FillSource<'_>, ts: &sk::Transform, region: DeviceRegion) -> Option<sk::Pixmap> {
    sampled_fill_region_with_limit(source, ts, region, max_fill_pixels())
}

/// Samples an unadjusted fallback with the production bound even when tests
/// temporarily lower the adjustment scratch limit.
pub(crate) fn sampled_fill_region_unadjusted_fallback(source: &FillSource<'_>, ts: &sk::Transform, region: DeviceRegion) -> Option<sk::Pixmap> {
    sampled_fill_region_with_limit(source, ts, region, MAX_FILL_PIXELS)
}

fn sampled_fill_region_with_limit(source: &FillSource<'_>, ts: &sk::Transform, region: DeviceRegion, max_pixels: u64) -> Option<sk::Pixmap> {
    let (x0, y0, x1, y1) = region;
    let (w, h) = (u32::try_from(x1.checked_sub(x0)?).ok()?, u32::try_from(y1.checked_sub(y0)?).ok()?);
    if w == 0 || h == 0 || u64::from(w) * u64::from(h) > max_pixels {
        return None;
    }
    #[cfg(test)]
    super::note_fill_allocation(w, h);
    let mut scratch = sk::Pixmap::new(w, h)?;
    let to_scratch = source.pattern_ts.post_concat(*ts).post_translate(-(x0 as f32), -(y0 as f32));
    let paint = sk::Paint { shader: sk::Pattern::new(source.src, source.spread, source.quality, 1.0, to_scratch), anti_alias: false, ..Default::default() };
    scratch.fill_rect(sk::Rect::from_xywh(0.0, 0.0, w as f32, h as f32)?, &paint, sk::Transform::identity(), None);
    if source.clear_outside_picture {
        clear_outside_picture(&mut scratch, &to_scratch, source.src.width() as f32, source.src.height() as f32)?;
    }
    Some(scratch)
}

/// Samples the picture into a bounded scratch, then applies image
/// adjustments. The caller draws the fill without adjustments if this returns
/// `None`.
pub(crate) fn adjusted_fill_region(source: &FillSource<'_>, ts: &sk::Transform, region: DeviceRegion, adjust: &ImageAdjust) -> Option<sk::Pixmap> {
    let mut scratch = sampled_fill_region(source, ts, region)?;
    apply_pixmap_adjustments(&mut scratch.as_mut(), adjust);
    Some(scratch)
}

/// Clears the pixels of `scratch` whose centre maps, through the inverse of `to_scratch`, outside the picture of
/// `picture_w` x `picture_h` pixels. `None` when `to_scratch` cannot be inverted.
fn clear_outside_picture(scratch: &mut sk::Pixmap, to_scratch: &sk::Transform, picture_w: f32, picture_h: f32) -> Option<()> {
    let inv = to_scratch.invert()?;
    let width = scratch.width() as usize;
    for (row, pixels) in scratch.pixels_mut().chunks_mut(width).enumerate() {
        let cy = row as f32 + 0.5;
        for (col, pixel) in pixels.iter_mut().enumerate() {
            let cx = col as f32 + 0.5;
            let (u, v) = (inv.sx * cx + inv.kx * cy + inv.tx, inv.ky * cx + inv.sy * cy + inv.ty);
            if !(0.0..=picture_w).contains(&u) || !(0.0..=picture_h).contains(&v) {
                *pixel = sk::PremultipliedColorU8::TRANSPARENT;
            }
        }
    }
    Some(())
}
