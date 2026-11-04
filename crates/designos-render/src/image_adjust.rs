//! Image fill adjustments (`paintFilter`) and scale-mode framing helpers.

use crate::tiny_skia as sk;
use designos_doc::{ImageAdjust, ImageParams, ImageScaleMode, Paint, PaintKind, Vec2};

mod adjusted_region;
pub use adjusted_region::unadjusted_fill_fallbacks;
#[cfg(test)]
pub(crate) use adjusted_region::TEST_MAX_PIXELS;
pub(crate) use adjusted_region::{
    adjusted_fill_region, note_unadjusted_fallback, sampled_fill_region, sampled_fill_region_unadjusted_fallback, DeviceRegion, FillSource,
};

mod crop_frame;
pub use crop_frame::{crop_paint_for_node, rotate_crop_transform, set_image_scale_mode, IDENTITY_CROP_TRANSFORM};

/// Gamma exponent used by Figma's fragment shader for exposure, contrast, and
/// saturation (`vec3(2.2)`).
pub const ADJUST_GAMMA: f32 = 2.2;

/// Inverse gamma exponent (`1.0 / 2.2 = 0.45454547`) used by Figma's fragment shader.
pub const INV_ADJUST_GAMMA: f32 = 0.45454547;

/// Rec.709 luminance weights used by Figma's fragment shader (`dot(c, vec3(0.2126, 0.7152, 0.0722))`).
pub const LUMA_WEIGHTS: [f32; 3] = [0.2126, 0.7152, 0.0722];

/// Threshold separating shadow polynomials (`< 0.5`) from highlight polynomials (`>= 0.5`).
pub const HIGHLIGHT_SHADOW_THRESHOLD: f32 = 0.5;

/// Symmetric clamp applied to `contrast` inside the tone-mapping step (`clamp(val, -0.3, 0.3)`).
pub const CONTRAST_LIMIT: f32 = 0.3;

/// Negative-exposure saturation preservation pivot (`0.2` in `1.0 + 0.2 * (-exposure)`).
pub const NEG_EXPOSURE_RESAT_PIVOT: f32 = 0.2;

/// Upper bound of the positive-saturation multiplier (`mix(1.0, 2.0, vibrance)`).
pub const POS_SAT_MAX_SCALE: f32 = 2.0;

#[cfg(test)]
thread_local! {
    /// Pixels run through [`adjust_rgb`] on this thread; tests read it to bound the work an adjusted fill does.
    pub(crate) static ADJUSTED_PIXELS: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
    /// Pixels of the largest pixmap an image fill allocated on this thread (its scratch or a padded copy of the picture).
    pub(crate) static LARGEST_FILL_ALLOCATION: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

/// Records a pixmap of `w` x `h` pixels allocated for an image fill, for the tests that bound the allocation.
#[cfg(test)]
pub(crate) fn note_fill_allocation(w: u32, h: u32) {
    LARGEST_FILL_ALLOCATION.with(|n| n.set(n.get().max(u64::from(w) * u64::from(h))));
}

#[inline]
fn non_zero(v: f32) -> bool {
    v.is_finite() && v != 0.0
}

/// Returns `true` when any of the seven rendered adjustment parameters is non-zero.
#[inline]
pub fn has_rendered_adjustments(adj: &ImageAdjust) -> bool {
    non_zero(adj.exposure)
        || non_zero(adj.contrast)
        || non_zero(adj.vibrance)
        || non_zero(adj.temperature)
        || non_zero(adj.tint)
        || non_zero(adj.highlights)
        || non_zero(adj.shadows)
}

#[inline]
fn dot3(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

#[inline]
fn clamp01(c: [f32; 3]) -> [f32; 3] {
    [c[0].clamp(0.0, 1.0), c[1].clamp(0.0, 1.0), c[2].clamp(0.0, 1.0)]
}

#[inline]
fn pow_gamma(c: [f32; 3], exp: f32) -> [f32; 3] {
    [c[0].max(0.0).powf(exp), c[1].max(0.0).powf(exp), c[2].max(0.0).powf(exp)]
}

#[inline]
fn smootherstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

#[inline]
fn shadows_highlights_offset(lin: [f32; 3], shadows: f32, highlights: f32) -> f32 {
    let h = dot3(lin, LUMA_WEIGHTS) - HIGHLIGHT_SHADOW_THRESHOLD.powf(ADJUST_GAMMA);
    let weight = if h > 0.0 { highlights } else { -0.999 * shadows };
    0.75 * h / (1.0 + 4.0 * h * h).sqrt() * weight
}

#[inline]
fn apply_lab_tint_temp_contrast(lin: [f32; 3], temperature: f32, tint: f32, contrast: f32) -> [f32; 3] {
    let x_xyz = (0.412_456_4 * lin[0] + 0.357_576_1 * lin[1] + 0.180_437_5 * lin[2]) / 0.95047;
    let y_xyz = 0.212_672_9 * lin[0] + 0.715_152_2 * lin[1] + 0.072_175_0 * lin[2];
    let z_xyz = (0.019_333_9 * lin[0] + 0.119_192 * lin[1] + 0.950_304_1 * lin[2]) / 1.08883;

    let fx = x_xyz.max(0.0).powf(1.0 / 3.0);
    let fy = y_xyz.max(0.0).powf(1.0 / 3.0);
    let fz = z_xyz.max(0.0).powf(1.0 / 3.0);

    let mut l_lab = 116.0 * fy - 16.0;
    let a_lab = 500.0 * (fx - fy) + 30.0 * tint;
    let b_lab = 200.0 * (fy - fz) + 30.0 * temperature;

    let c = contrast.clamp(-CONTRAST_LIMIT, CONTRAST_LIMIT);
    let l_c = HIGHLIGHT_SHADOW_THRESHOLD * 100.0;
    l_lab = if c < 0.0 {
        let low = 50.0 + 0.2 * (l_lab - l_c);
        low + (l_lab - low) * (c + 1.0)
    } else {
        let s = smootherstep(l_c - 30.0, l_c + 30.0, l_lab) * 100.0;
        l_lab + (s - l_lab) * c
    };

    let fy2 = (l_lab + 16.0) / 116.0;
    let fx2 = a_lab / 500.0 + fy2;
    let fz2 = fy2 - b_lab / 200.0;

    let x2 = fx2 * fx2 * fx2 * 0.95047;
    let y2 = fy2 * fy2 * fy2;
    let z2 = fz2 * fz2 * fz2 * 1.08883;

    clamp01([
        3.240_454_2 * x2 - 1.537_138_5 * y2 - 0.498_531_4 * z2,
        -0.969_266 * x2 + 1.876_010_8 * y2 + 0.041_556_0 * z2,
        0.055_643_4 * x2 - 0.204_025_9 * y2 + 1.057_225_2 * z2,
    ])
}

#[inline]
fn apply_exposure_linear(mut c: [f32; 3], j: f32) -> [f32; 3] {
    let k = 10.0_f32.powf(2.0 * j) - 1.0;
    let l = 0.75_f32;
    let mut m = 1.0 / (1.0 - l) - 1.0;
    let mut n = dot3(c, LUMA_WEIGHTS);
    let j_neg = j.min(0.0);
    m *= 1.0 + n * n * j_neg / (NEG_EXPOSURE_RESAT_PIVOT - j_neg);
    c = [c[0] + (n - c[0]) * l, c[1] + (n - c[1]) * l, c[2] + (n - c[2]) * l];
    if j < 0.0 {
        let s = 1.0 + j / 16.0;
        c = [c[0] * s, c[1] * s, c[2] * s];
        n *= s;
    }
    c = [c[0] * (k + 1.0) / (c[0] * k + 1.0), c[1] * (k + 1.0) / (c[1] * k + 1.0), c[2] * (k + 1.0) / (c[2] * k + 1.0)];
    n = n * (k + 1.0) / (n * k + 1.0);
    clamp01([c[0] + (c[0] - n) * m, c[1] + (c[1] - n) * m, c[2] + (c[2] - n) * m])
}

#[inline]
fn apply_saturation_linear(c: [f32; 3], p: f32) -> [f32; 3] {
    let n = dot3(c, LUMA_WEIGHTS);
    if p <= 0.0 {
        clamp01([c[0] + (c[0] - n) * p, c[1] + (c[1] - n) * p, c[2] + (c[2] - n) * p])
    } else {
        let min_c = c[0].min(c[1]).min(c[2]);
        let max_c = c[0].max(c[1]).max(c[2]);
        let q = n / (n - min_c + 1e-6);
        let r = (1.0 - n) / (max_c - n + 1e-6);
        let scale = (1.0 + (q.min(r) - 1.0) * p).min(POS_SAT_MAX_SCALE);
        clamp01([n + (c[0] - n) * scale, n + (c[1] - n) * scale, n + (c[2] - n) * scale])
    }
}

/// Applies the 6-step adjustment pipeline (spec section 3 rule 3) to a straight-alpha sRGB pixel `rgb` in `0.0..=1.0`.
#[inline]
pub fn adjust_rgb(c: [f32; 3], adj: &ImageAdjust) -> [f32; 3] {
    #[cfg(test)]
    ADJUSTED_PIXELS.with(|n| n.set(n.get() + 1));
    let exposure = if non_zero(adj.exposure) { adj.exposure.clamp(-1.0, 1.0) } else { 0.0 };
    let contrast = if non_zero(adj.contrast) { adj.contrast.clamp(-CONTRAST_LIMIT, CONTRAST_LIMIT) } else { 0.0 };
    let vibrance = if non_zero(adj.vibrance) { adj.vibrance.clamp(-1.0, 1.0) } else { 0.0 };
    let temperature = if non_zero(adj.temperature) { adj.temperature.clamp(-1.0, 1.0) } else { 0.0 };
    let tint = if non_zero(adj.tint) { adj.tint.clamp(-1.0, 1.0) } else { 0.0 };
    let highlights = if non_zero(adj.highlights) { adj.highlights.clamp(-1.0, 1.0) } else { 0.0 };
    let shadows = if non_zero(adj.shadows) { adj.shadows.clamp(-1.0, 1.0) } else { 0.0 };

    // Step 1: linear colour = rgb^2.2
    let mut lin = pow_gamma(c, ADJUST_GAMMA);
    // Step 2: shadows and highlights offset ai
    let ai = shadows_highlights_offset(lin, shadows, highlights);
    // Step 3: Lab (D65, plain cube root) tint, temperature, and contrast
    if tint != 0.0 || temperature != 0.0 || contrast != 0.0 {
        lin = apply_lab_tint_temp_contrast(lin, temperature, tint, contrast);
    }
    // Step 4: exposure with j = exposure + ai
    let j = exposure + ai;
    if j != 0.0 {
        lin = apply_exposure_linear(lin, j);
    }
    // Step 5: saturation p (kiwi vibrance)
    if vibrance != 0.0 {
        lin = apply_saturation_linear(lin, vibrance);
    }
    // Step 6: output = result^(1 / 2.2)
    clamp01(pow_gamma(lin, INV_ADJUST_GAMMA))
}

/// Applies the 7-parameter adjustment pipeline to a straight-alpha `RGBA8` pixel,
/// leaving `alpha` untouched and returning `[0, 0, 0, 0]` when `alpha == 0`.
#[inline]
pub fn adjust_rgba(rgba: [u8; 4], adj: &ImageAdjust) -> [u8; 4] {
    if rgba[3] == 0 {
        return [0, 0, 0, 0];
    }
    if !has_rendered_adjustments(adj) {
        return rgba;
    }
    let rgb = [rgba[0] as f32 / 255.0, rgba[1] as f32 / 255.0, rgba[2] as f32 / 255.0];
    let out = adjust_rgb(rgb, adj);
    [(out[0] * 255.0 + 0.5) as u8, (out[1] * 255.0 + 0.5) as u8, (out[2] * 255.0 + 0.5) as u8, rgba[3]]
}

/// Applies the 7-parameter adjustment pipeline in-place to a premultiplied `tiny-skia` pixmap.
pub fn apply_pixmap_adjustments(pix: &mut sk::PixmapMut<'_>, adj: &ImageAdjust) {
    if !has_rendered_adjustments(adj) {
        return;
    }
    for p in pix.pixels_mut() {
        let a = p.alpha();
        if a == 0 {
            continue;
        }
        if a == 255 {
            let [r, g, b, _] = adjust_rgba([p.red(), p.green(), p.blue(), 255], adj);
            if let Some(pm) = sk::PremultipliedColorU8::from_rgba(r, g, b, 255) {
                *p = pm;
            }
        } else {
            let af = a as f32;
            let inv_a = 1.0 / af;
            let rgb = [(p.red() as f32 * inv_a).clamp(0.0, 1.0), (p.green() as f32 * inv_a).clamp(0.0, 1.0), (p.blue() as f32 * inv_a).clamp(0.0, 1.0)];
            let out = adjust_rgb(rgb, adj);
            let r = (out[0] * af + 0.5).min(af) as u8;
            let g = (out[1] * af + 0.5).min(af) as u8;
            let b = (out[2] * af + 0.5).min(af) as u8;
            if let Some(pm) = sk::PremultipliedColorU8::from_rgba(r, g, b, a) {
                *p = pm;
            }
        }
    }
}

/// Computes the 2x3 image UV affine transform `[m00, m01, m02, m10, m11, m12]` that reproduces
/// the current `Fill` or `Fit` framing (including `params.rotation` quarter turns) in `Crop` mode.
pub fn crop_transform_from_framing(size: Vec2, iw: f32, ih: f32, scale_mode: ImageScaleMode, params: &ImageParams) -> Option<[f32; 6]> {
    if !(size.x.is_finite() && size.y.is_finite() && size.x > 0.0 && size.y > 0.0 && iw > 0.0 && ih > 0.0) {
        return None;
    }
    let mode = match scale_mode {
        ImageScaleMode::Fill | ImageScaleMode::Fit => scale_mode,
        ImageScaleMode::Crop | ImageScaleMode::Tile => ImageScaleMode::Fill,
    };
    let map = designos_doc::image_uv_map(size, iw, ih, mode, params)?;
    Some([map.u.0 * size.x, map.u.1 * size.y, map.u.2, map.v.0 * size.x, map.v.1 * size.y, map.v.2])
}

/// Returns the decoded `(width, height)` of `image_ref` from `doc` using the render cache.
pub fn image_dimensions(doc: &designos_doc::Document, image_ref: &str) -> Option<(u32, u32)> {
    let pm = crate::paint::load_image(doc, image_ref, None)?;
    Some((pm.width(), pm.height()))
}

/// Switches an image paint's `scale_mode` to `next`, preserving or synthesizing `params.transform`
/// when entering or leaving `ImageScaleMode::Crop`.
pub fn switch_image_scale_mode(paint: &mut Paint, next: ImageScaleMode, size: Vec2, image_size: (u32, u32)) {
    let PaintKind::Image { scale_mode, params, .. } = &mut paint.kind else { return };
    let prev = *scale_mode;
    if next == ImageScaleMode::Crop && prev != ImageScaleMode::Crop && params.transform.is_none() {
        let (iw, ih) = (image_size.0 as f32, image_size.1 as f32);
        params.transform = crop_transform_from_framing(size, iw, ih, prev, params);
    }
    *scale_mode = next;
}

#[cfg(test)]
mod tests;
