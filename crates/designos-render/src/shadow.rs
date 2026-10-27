//! Fast drop shadows for solid-filled, axis-aligned rounded rectangles.
//!
//! The blurred mask of a rounded rectangle is a nine-patch: far from the
//! corners it only varies across the nearest edge, so it is the same along the
//! edge. The four corner regions plus one column and one row of the plateau
//! therefore describe the whole mask. That small patch is rendered and blurred
//! once with the regular pipeline, cached by *geometry* (size or corner extent,
//! corner radius, sigma and the sub-pixel position of the edges, never the node
//! id), and the full-size mask is assembled from it by index maps. Design
//! systems repeat the same card shadow thousands of times, so the patch is
//! usually a cache hit and the cost of a shadow is one pass over its pixels.
//!
//! Edge positions are truncated to 1/256 px, the precision tiny-skia rasterises
//! edges with (so the patch is anti-aliased exactly like the node itself would
//! be), and sigma and the corner radius are quantised to 1/64 px.

use crate::blur::{gaussian_blur_at, Edges};
use crate::effects::{composite, Sil};
use crate::geom::rounded_rect;
use crate::paint::sk_color;
use designos_doc as d;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use tiny_skia as sk;

/// Device-space rectangle of the node and its uniform corner radius.
#[derive(Clone, Copy, Debug)]
pub struct RRect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub r: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct AxisKey {
    /// 256ths of a pixel: leading edge fraction, patch extent of the shape.
    frac: i32,
    ext256: i32,
    len: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct Key {
    x: AxisKey,
    y: AxisKey,
    r64: i32,
    sigma64: i32,
}

struct Patch {
    w: usize,
    h: usize,
    alpha: Vec<u8>,
}

const MAX_CACHE_BYTES: usize = 48 << 20;

struct Cache {
    map: HashMap<Key, Arc<Patch>>,
    bytes: usize,
}

fn cache() -> &'static Mutex<Cache> {
    static C: OnceLock<Mutex<Cache>> = OnceLock::new();
    C.get_or_init(|| Mutex::new(Cache { map: HashMap::new(), bytes: 0 }))
}

/// Number of cached patches (diagnostics and tests).
#[doc(hidden)]
pub fn cached_patches() -> usize {
    cache().lock().map(|c| c.map.len()).unwrap_or(0)
}

thread_local! {
    static BUILDS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Patches built (cache misses) on this thread so far.
#[cfg(test)]
#[doc(hidden)]
pub fn thread_patch_builds() -> usize {
    BUILDS.with(|b| b.get())
}

/// Patch length and plateau-compressed extent (256ths) of one axis.
#[cfg(test)]
pub(crate) fn patch_dims(a: f32, b: f32, k: i32, e: i32) -> Option<(i32, i32)> {
    Axis::new(a, b, k, e).map(|ax| (ax.key.len, ax.key.ext256))
}

/// Empties the patch cache.
#[doc(hidden)]
pub fn clear_cache() {
    if let Ok(mut c) = cache().lock() {
        c.map.clear();
        c.bytes = 0;
    }
}

/// How one axis of the shadow maps onto the patch.
#[derive(Clone, Copy, Debug)]
struct Axis {
    /// Integer pixel containing the leading edge.
    i0: i32,
    /// Margin (px) of the patch outside the shape, and corner extent inwards.
    k: i32,
    e: i32,
    /// Plateau pixels removed from the patch.
    n: i32,
    key: AxisKey,
}

impl Axis {
    /// `a`, `b`: leading and trailing edge in device pixels.
    fn new(a: f32, b: f32, k: i32, e: i32) -> Option<Axis> {
        let i0 = a.floor() as i32;
        let fa = (((a - i0 as f32) * 256.0).floor() as i32).clamp(0, 255);
        let ib = b.floor() as i32;
        let fb = (((b - ib as f32) * 256.0).floor() as i32).clamp(0, 255);
        let l256 = (ib - i0) * 256 + fb - fa;
        if l256 < 1 {
            return None;
        }
        let n = (l256.div_euclid(256) - (2 * e + 3)).max(0);
        let ext256 = l256 - n * 256;
        let len = (k * 256 + fa + ext256 + 255) / 256 + k;
        Some(Axis { i0, k, e, n, key: AxisKey { frac: fa, ext256, len } })
    }

    /// Patch index for absolute pixel `x`, or `None` outside the shadow.
    fn map(&self, x: i32) -> Option<usize> {
        let o = x - self.i0;
        let p = if o < -self.k {
            return None;
        } else if o < self.e {
            o + self.k
        } else if self.n > 0 && o < self.n + self.e + 1 {
            self.k + self.e
        } else {
            o - self.n + self.k
        };
        (p < self.key.len).then_some(p as usize)
    }

    /// Absolute pixel range `[lo, hi)` the shadow can touch.
    fn span(&self) -> (i32, i32) {
        (self.i0 - self.k, self.i0 - self.k + self.key.len + self.n)
    }
}

fn build_patch(key: &Key, k: i32) -> Patch {
    let (w, h) = (key.x.len as u32, key.y.len as u32);
    let sigma = key.sigma64 as f32 / 64.0;
    let mut pm = sk::Pixmap::new(w.max(1), h.max(1)).expect("patch pixmap");
    let (ew, eh) = (key.x.ext256 as f32 / 256.0, key.y.ext256 as f32 / 256.0);
    let r = key.r64 as f32 / 64.0;
    if let Some(path) = rounded_rect(ew, eh, [r; 4]) {
        let mut p = sk::Paint { anti_alias: true, ..Default::default() };
        p.set_color(sk::Color::BLACK);
        let ts = sk::Transform::from_translate(k as f32 + key.x.frac as f32 / 256.0, k as f32 + key.y.frac as f32 / 256.0);
        pm.fill_path(&path, &p, sk::FillRule::Winding, ts, None);
    }
    gaussian_blur_at(&mut pm, sigma, (0, 0), Edges::Margined);
    Patch { w: w as usize, h: h as usize, alpha: pm.data().as_chunks::<4>().0.iter().map(|p| p[3]).collect() }
}

fn patch_for(key: Key, k: i32) -> Arc<Patch> {
    if let Ok(c) = cache().lock() {
        if let Some(p) = c.map.get(&key) {
            return p.clone();
        }
    }
    BUILDS.with(|b| b.set(b.get() + 1));
    let patch = Arc::new(build_patch(&key, k));
    if let Ok(mut c) = cache().lock() {
        if c.bytes + patch.alpha.len() > MAX_CACHE_BYTES {
            c.map.clear();
            c.bytes = 0;
        }
        c.bytes += patch.alpha.len();
        c.map.insert(key, patch.clone());
    }
    patch
}

/// Draws the drop shadow of a solid rounded rectangle. Returns `false` (having
/// drawn nothing) when the shape is not suitable and the caller must use the
/// generic path.
///
/// `fill_alpha` is the alpha of the node's single solid fill. `sil` is only
/// used to knock the node itself out of the shadow when `show_behind` is off.
#[allow(clippy::too_many_arguments)]
pub fn drop_shadow_rrect(
    target: &mut sk::Pixmap,
    clip: Option<&sk::Mask>,
    rr: RRect,
    scale: f32,
    color: &d::Color,
    offset: d::Vec2,
    radius: f32,
    spread: f32,
    show_behind: bool,
    blend: d::BlendMode,
    alpha: f32,
    fill_alpha: f32,
    sil: Sil,
) -> bool {
    let sigma = radius.max(0.0) * scale / 2.0;
    if sigma < 0.3 || rr.w.is_nan() || rr.w <= 0.0 || rr.h.is_nan() || rr.h <= 0.0 || !rr.x.is_finite() || !rr.y.is_finite() {
        return false;
    }
    // The generic path grows the silhouette by stroking its outline, whose
    // curve approximation differs from a true rounded rectangle by about a
    // tenth of a pixel; that only shows through a very small blur.
    if spread != 0.0 && sigma < 8.0 {
        return false;
    }
    let sigma64 = (sigma * 64.0).round() as i32;
    let sigma_q = sigma64 as f32 / 64.0;
    let sp = spread * scale;
    let (a, b) = (rr.x + offset.x * scale - sp, rr.x + rr.w + offset.x * scale + sp);
    let (c, dd) = (rr.y + offset.y * scale - sp, rr.y + rr.h + offset.y * scale + sp);
    if b - a < 1.0 / 32.0 || dd - c < 1.0 / 32.0 || (b - a) > 1.0e6 || (dd - c) > 1.0e6 {
        return false;
    }
    let mut r = if rr.r > 0.0 { (rr.r + sp).max(0.0) } else { 0.0 };
    r = r.min((b - a) / 2.0).min((dd - c) / 2.0);
    let r64 = (r * 64.0).round() as i32;
    let r_q = r64 as f32 / 64.0;
    let k = (3.0 * sigma_q).ceil() as i32 + 3;
    let e = r_q.ceil() as i32 + (3.0 * sigma_q).ceil() as i32 + 4;
    let (Some(ax), Some(ay)) = (Axis::new(a, b, k, e), Axis::new(c, dd, k, e)) else { return false };
    let (x_lo, x_hi) = ax.span();
    let (y_lo, y_hi) = ay.span();
    let (rx, ry) = (x_lo.max(0), y_lo.max(0));
    let (rx1, ry1) = (x_hi.min(target.width() as i32), y_hi.min(target.height() as i32));
    if rx1 <= rx || ry1 <= ry {
        return true;
    }
    let (rw, rh) = ((rx1 - rx) as usize, (ry1 - ry) as usize);
    let Some(mut pm) = sk::Pixmap::new(rw as u32, rh as u32) else { return false };

    let patch = patch_for(Key { x: ax.key, y: ay.key, r64, sigma64 }, k);
    debug_assert_eq!((patch.w, patch.h), (ax.key.len as usize, ay.key.len as usize));

    // shadow colour premultiplied per mask level
    let col = sk_color(color, alpha);
    let va = col.alpha() * fill_alpha.clamp(0.0, 1.0);
    let mut lut = [[0u8; 4]; 256];
    for (m, out) in lut.iter_mut().enumerate() {
        let v = va * m as f32 / 255.0;
        *out = [(col.red() * v * 255.0 + 0.5) as u8, (col.green() * v * 255.0 + 0.5) as u8, (col.blue() * v * 255.0 + 0.5) as u8, (v * 255.0 + 0.5) as u8];
    }
    let xmap: Vec<i32> = (rx..rx1).map(|x| ax.map(x).map_or(-1, |p| p as i32)).collect();
    let data = pm.data_mut();
    let mut prev: Option<(usize, usize)> = None; // (patch row, region row) of the last written row
    for ry_i in 0..rh {
        let Some(py) = ay.map(ry + ry_i as i32) else { continue };
        if let Some((ppy, prow)) = prev {
            if ppy == py {
                data.copy_within(prow * rw * 4..(prow + 1) * rw * 4, ry_i * rw * 4);
                prev = Some((py, ry_i));
                continue;
            }
        }
        let prow = &patch.alpha[py * patch.w..(py + 1) * patch.w];
        let out = &mut data[ry_i * rw * 4..(ry_i + 1) * rw * 4];
        for (o, &xm) in out.as_chunks_mut::<4>().0.iter_mut().zip(&xmap) {
            if xm >= 0 {
                let m = prow[xm as usize];
                if m != 0 {
                    o.copy_from_slice(&lut[m as usize]);
                }
            }
        }
        prev = Some((py, ry_i));
    }
    if !show_behind {
        if let Some(mut knock) = sk::Pixmap::new(rw as u32, rh as u32) {
            sil(&mut knock, -(rx as f32), -(ry as f32), 0.0, designos_doc::SilhouetteMode::Knockout);
            composite(&mut pm, 0, 0, &knock, sk::BlendMode::DestinationOut, None);
        }
    }
    crate::effects::composite_blended(target, rx, ry, &pm, blend, clip);
    true
}
