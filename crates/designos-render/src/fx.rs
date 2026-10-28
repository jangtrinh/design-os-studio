//! Noise, Texture and Glass effects.
//!
//! Everything random is a pure function of the seed and of the *absolute* device
//! pixel (`effects::grid_pos`, the same grid the blurs anchor to), never of a tile
//! or pixmap origin. Tiles stitched together therefore equal a full-page render byte
//! for byte, and the pattern does not depend on how the target is cropped.

use crate::blur::{gaussian_blur_at, Edges};
use crate::effects::{composite, grid_pos, region, BBox, Sil};
use crate::paint::{blend_mode, sk_color};
use designos_doc as d;
use tiny_skia as sk;

// ---------------------------------------------------------------- hashing
fn mix(mut h: u32) -> u32 {
    h ^= h >> 16;
    h = h.wrapping_mul(0x7feb_352d);
    h ^= h >> 15;
    h = h.wrapping_mul(0x846c_a68b);
    h ^ (h >> 16)
}

/// Hash of a lattice point under `seed`.
pub(crate) fn hash2(seed: u32, x: i32, y: i32) -> u32 {
    mix(seed.wrapping_mul(0x9e37_79b1) ^ mix((x as u32).wrapping_mul(0x85eb_ca6b) ^ (y as u32).wrapping_mul(0xc2b2_ae35).rotate_left(13)))
}

/// Hash mapped to 0..1.
fn unit(h: u32) -> f32 {
    (h >> 8) as f32 / (1u32 << 24) as f32
}

/// Smooth value noise in 0..1 at lattice coordinates `(u, v)`.
pub(crate) fn value_noise(seed: u32, u: f32, v: f32) -> f32 {
    let (iu, iv) = (u.floor(), v.floor());
    let (fu, fv) = (u - iu, v - iv);
    let (iu, iv) = (iu as i32, iv as i32);
    let s = |t: f32| t * t * (3.0 - 2.0 * t);
    let (su, sv) = (s(fu), s(fv));
    let c = |dx: i32, dy: i32| unit(hash2(seed, iu.wrapping_add(dx), iv.wrapping_add(dy)));
    let top = c(0, 0) + (c(1, 0) - c(0, 0)) * su;
    let bot = c(0, 1) + (c(1, 1) - c(0, 1)) * su;
    top + (bot - top) * sv
}

fn hsv_to_rgb(h: f32, s: f32, v: f32) -> [f32; 3] {
    let h6 = h.rem_euclid(1.0) * 6.0;
    let (i, f) = (h6.floor() as i32, h6 - h6.floor());
    let (p, q, t) = (v * (1.0 - s), v * (1.0 - s * f), v * (1.0 - s * (1.0 - f)));
    match i {
        0 => [v, t, p],
        1 => [q, v, p],
        2 => [p, v, t],
        3 => [p, q, v],
        4 => [t, p, v],
        _ => [v, p, q],
    }
}

fn premul(r: f32, g: f32, b: f32, a: f32) -> sk::PremultipliedColorU8 {
    let q = |v: f32| (v * 255.0).round().clamp(0.0, 255.0) as u8;
    let a8 = q(a);
    sk::PremultipliedColorU8::from_rgba(q(r * a).min(a8), q(g * a).min(a8), q(b * a).min(a8), a8).unwrap_or(sk::PremultipliedColorU8::TRANSPARENT)
}

// ---------------------------------------------------------------- noise
/// Seeded grain over the shape: one grain is `size` page units wide; `density` is the
/// share of grains that are drawn.
#[allow(clippy::too_many_arguments)]
pub fn noise(target: &mut sk::Pixmap, clip: Option<&sk::Mask>, bbox: BBox, scale: f32, e: &d::NoiseEffect, alpha: f32, sil: Sil) {
    let Some((rx, ry, rw, rh)) = region(bbox, 0.0, target.width(), target.height()) else { return };
    let Some(mut pm) = sk::Pixmap::new(rw, rh) else { return };
    let cell = (e.size.max(0.05) * scale).max(1.0);
    let density = e.density.clamp(0.0, 1.0);
    let a = (e.opacity * alpha).clamp(0.0, 1.0);
    let rgb = |c: &d::Color| {
        let s = sk_color(c, 1.0);
        (s.red(), s.green(), s.blue(), s.alpha())
    };
    for y in 0..rh {
        for x in 0..rw {
            let (gx, gy) = grid_pos(rx + x as i32, ry + y as i32);
            let (cx, cy) = ((gx as f32 / cell).floor() as i32, (gy as f32 / cell).floor() as i32);
            let h = hash2(e.seed, cx, cy);
            if unit(h) >= density {
                continue;
            }
            let h2 = mix(h ^ 0x6a09_e667);
            let (r, g, b, ca) = match &e.mode {
                d::NoiseMode::Mono { color } => rgb(color),
                d::NoiseMode::Duo { color, secondary } => {
                    if h2 & 1 == 0 {
                        rgb(color)
                    } else {
                        rgb(secondary)
                    }
                }
                d::NoiseMode::Multi => {
                    let c = hsv_to_rgb(unit(h2), 1.0, 1.0);
                    (c[0], c[1], c[2], 1.0)
                }
            };
            let idx = (y * rw + x) as usize;
            pm.pixels_mut()[idx] = premul(r, g, b, ca * a);
        }
    }
    if let Some(mut shape) = sk::Pixmap::new(rw, rh) {
        sil(&mut shape, -(rx as f32), -(ry as f32), 0.0, d::SilhouetteMode::Opaque);
        composite(&mut pm, 0, 0, &shape, sk::BlendMode::DestinationIn, None);
    }
    composite(target, rx, ry, &pm, blend_mode(e.blend), clip);
}

// ---------------------------------------------------------------- texture
/// Bilinear sample of a premultiplied RGBA8 pixmap at pixel-centre coordinates `(x, y)`;
/// outside the pixmap is transparent.
struct Sampler<'a> {
    data: &'a [u8],
    w: i32,
    h: i32,
}

impl<'a> Sampler<'a> {
    fn new(pm: &'a sk::Pixmap) -> Self {
        Self { data: pm.data(), w: pm.width() as i32, h: pm.height() as i32 }
    }

    fn texel(&self, x: i32, y: i32) -> [f32; 4] {
        if x < 0 || y < 0 || x >= self.w || y >= self.h {
            return [0.0; 4];
        }
        let i = ((y * self.w + x) * 4) as usize;
        [self.data[i] as f32, self.data[i + 1] as f32, self.data[i + 2] as f32, self.data[i + 3] as f32]
    }

    /// Like `texel` but edge pixels extend outwards (for backdrops).
    fn clamped(&self, x: i32, y: i32) -> [f32; 4] {
        self.texel(x.clamp(0, self.w - 1), y.clamp(0, self.h - 1))
    }

    fn sample(&self, x: f32, y: f32, clamp: bool) -> [f32; 4] {
        let (fx, fy) = (x - 0.5, y - 0.5);
        let (x0, y0) = (fx.floor(), fy.floor());
        let (tx, ty) = (fx - x0, fy - y0);
        let (x0, y0) = (x0 as i32, y0 as i32);
        let get = |dx: i32, dy: i32| if clamp { self.clamped(x0 + dx, y0 + dy) } else { self.texel(x0 + dx, y0 + dy) };
        let (a, b, c, d) = (get(0, 0), get(1, 0), get(0, 1), get(1, 1));
        let mut out = [0.0; 4];
        for k in 0..4 {
            let top = a[k] + (b[k] - a[k]) * tx;
            let bot = c[k] + (d[k] - c[k]) * tx;
            out[k] = top + (bot - top) * ty;
        }
        out
    }
}

fn store(px: &mut sk::PremultipliedColorU8, c: [f32; 4]) {
    let q = |v: f32| v.round().clamp(0.0, 255.0) as u8;
    let a = q(c[3]);
    *px = sk::PremultipliedColorU8::from_rgba(q(c[0]).min(a), q(c[1]).min(a), q(c[2]).min(a), a).unwrap_or(sk::PremultipliedColorU8::TRANSPARENT);
}

/// Displaces the pixels of `layer` (the node rendered in isolation) with seeded value noise,
/// roughening its edges by up to `radius`; `clip_to_shape` then removes everything outside
/// the original silhouette.
pub fn texture(layer: &mut sk::Pixmap, bbox: BBox, scale: f32, e: &d::TextureEffect, sil: Sil) {
    let amp = e.radius.max(0.0) * scale;
    if amp <= 0.0 {
        return;
    }
    let Some((rx, ry, rw, rh)) = region(bbox, amp + 2.0, layer.width(), layer.height()) else { return };
    // Clamp the region to the layer without shifting its far edge.
    let (x1, y1) = ((rx + rw as i32).min(layer.width() as i32), (ry + rh as i32).min(layer.height() as i32));
    let (rx, ry) = (rx.max(0), ry.max(0));
    if x1 <= rx || y1 <= ry {
        return;
    }
    let (rw, rh) = ((x1 - rx) as u32, (y1 - ry) as u32);
    let Some(r) = sk::IntRect::from_xywh(rx, ry, rw, rh) else { return };
    let Some(src) = layer.clone_rect(r) else { return };
    let Some(mut out) = sk::Pixmap::new(rw, rh) else { return };
    let sampler = Sampler::new(&src);
    let cell = (e.size.max(0.1) * scale).max(1.0);
    for y in 0..rh as i32 {
        for x in 0..rw as i32 {
            let (gx, gy) = grid_pos(rx + x, ry + y);
            let (u, v) = (gx as f32 / cell, gy as f32 / cell);
            // Two octaves: broad waviness plus fine roughness.
            let n = |s: u32, k: f32| value_noise(e.seed ^ s, u * k + 17.3 * k, v * k + 5.1 * k);
            let dx = ((n(0, 1.0) - 0.5) + 0.5 * (n(0x51ed, 2.3) - 0.5)) * 1.33 * amp;
            let dy = ((n(0x9bd1, 1.0) - 0.5) + 0.5 * (n(0x2f7c, 2.3) - 0.5)) * 1.33 * amp;
            let c = sampler.sample(x as f32 + 0.5 + dx, y as f32 + 0.5 + dy, false);
            store(&mut out.pixels_mut()[(y as u32 * rw + x as u32) as usize], c);
        }
    }
    if e.clip_to_shape {
        if let Some(mut shape) = sk::Pixmap::new(rw, rh) {
            sil(&mut shape, -(rx as f32), -(ry as f32), 0.0, d::SilhouetteMode::Opaque);
            composite(&mut out, 0, 0, &shape, sk::BlendMode::DestinationIn, None);
        }
    }
    composite(layer, rx, ry, &out, sk::BlendMode::Source, None);
}

// ---------------------------------------------------------------- glass
/// Frosted, refractive glass: the backdrop under the shape is blurred (`frost`), bent near the
/// edges along the edge normal (`refraction`, within `depth` of the edge), split into red and
/// blue fringes (`dispersion`) and lit by a highlight from `light_angle`.
#[allow(clippy::too_many_arguments)]
pub fn glass(target: &mut sk::Pixmap, backdrop: Option<&sk::Pixmap>, clip: Option<&sk::Mask>, bbox: BBox, scale: f32, e: &d::GlassEffect, sil: Sil) {
    let (tw, th) = (target.width() as i32, target.height() as i32);
    let x0 = (bbox.0.floor() as i32).max(0);
    let y0 = (bbox.1.floor() as i32).max(0);
    let x1 = (bbox.2.ceil() as i32).min(tw);
    let y1 = (bbox.3.ceil() as i32).min(th);
    if x1 <= x0 || y1 <= y0 {
        return;
    }
    let depth = e.depth.max(0.0) * scale;
    let sigma_f = e.frost.max(0.0) * scale / 2.0;
    let sigma_m = (depth / 3.0).max(0.6);
    let pad = (3.0 * sigma_f.max(sigma_m)).ceil() as i32 + (depth * 1.5).ceil() as i32 + 2;
    let (px0, py0) = ((x0 - pad).max(0), (y0 - pad).max(0));
    let (px1, py1) = ((x1 + pad).min(tw), (y1 + pad).min(th));
    let Some(prect) = sk::IntRect::from_xywh(px0, py0, (px1 - px0) as u32, (py1 - py0) as u32) else { return };
    let Some(mut sub) = backdrop.unwrap_or(target).clone_rect(prect) else { return };
    if backdrop.is_some() {
        if let Some(own) = target.clone_rect(prect) {
            composite(&mut sub, 0, 0, &own, sk::BlendMode::SourceOver, None);
        }
    }
    gaussian_blur_at(&mut sub, sigma_f, grid_pos(px0, py0), Edges::Exact);
    // Coverage of the shape, and a softened copy whose gradient gives the edge normal.
    let Some(mut mask) = sk::Pixmap::new(prect.width(), prect.height()) else { return };
    sil(&mut mask, -(px0 as f32), -(py0 as f32), 0.0, d::SilhouetteMode::Opaque);
    let mut soft = mask.clone();
    gaussian_blur_at(&mut soft, sigma_m, grid_pos(px0, py0), Edges::Exact);
    let (cw, ch) = ((x1 - x0) as u32, (y1 - y0) as u32);
    let Some(mut out) = sk::Pixmap::new(cw, ch) else { return };
    let Some(mut cover) = sk::Pixmap::new(cw, ch) else { return };
    let back = Sampler::new(&sub);
    let softs = Sampler::new(&soft);
    let masks = Sampler::new(&mask);
    let (lx, ly) = (e.light_angle.to_radians().cos(), e.light_angle.to_radians().sin());
    let (refr, disp, inten) = (e.refraction.clamp(0.0, 1.0), e.dispersion.clamp(0.0, 1.0), e.light_intensity.clamp(0.0, 1.0));
    for y in y0..y1 {
        for x in x0..x1 {
            let (mx, my) = (x - px0, y - py0);
            let cov = masks.texel(mx, my)[3] / 255.0;
            if cov <= 0.0 {
                continue;
            }
            let m = |dx: i32, dy: i32| softs.clamped(mx + dx, my + dy)[3] / 255.0;
            let (gx, gy) = ((m(1, 0) - m(-1, 0)) * 0.5, (m(0, 1) - m(0, -1)) * 0.5);
            let len = (gx * gx + gy * gy).sqrt();
            let (nx, ny) = if len > 1e-4 { (gx / len, gy / len) } else { (0.0, 0.0) };
            // 1 at the very edge, 0 deeper than `depth` inside.
            let t = ((1.0 - m(0, 0)) * 2.0).clamp(0.0, 1.0);
            let (fx, fy) = (mx as f32 + 0.5, my as f32 + 0.5);
            let bend = refr * depth * t;
            let split = disp * depth * 0.5 * t;
            let at = |off: f32| back.sample(fx + nx * (bend + off), fy + ny * (bend + off), true);
            let (cr, cg, cb) = (at(split), at(0.0), at(-split));
            let mut c = [cr[0], cg[1], cb[2], cg[3]];
            // Highlight on edges facing the light, a faint shade on the far side.
            let facing = -(nx * lx + ny * ly);
            let hl = (inten * t * facing.max(0.0).powf(1.5)).clamp(0.0, 1.0);
            let shade = (inten * 0.3 * t * (-facing).max(0.0)).clamp(0.0, 1.0);
            for ch in c.iter_mut().take(3) {
                *ch = *ch * (1.0 - shade) * (1.0 - hl) + 255.0 * hl;
            }
            c[3] = c[3] * (1.0 - hl) + 255.0 * hl;
            for ch in &mut c {
                *ch *= cov;
            }
            let i = ((y - y0) as u32 * cw + (x - x0) as u32) as usize;
            store(&mut out.pixels_mut()[i], c);
            let q = (cov * 255.0).round() as u8;
            cover.pixels_mut()[i] = sk::PremultipliedColorU8::from_rgba(q, q, q, q).unwrap_or(sk::PremultipliedColorU8::TRANSPARENT);
        }
    }
    composite(target, x0, y0, &cover, sk::BlendMode::DestinationOut, clip);
    composite(target, x0, y0, &out, sk::BlendMode::SourceOver, clip);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashing_is_stable_and_spread() {
        assert_eq!(hash2(1, 5, 7), hash2(1, 5, 7));
        assert_ne!(hash2(1, 5, 7), hash2(2, 5, 7));
        assert_ne!(hash2(1, 5, 7), hash2(1, 7, 5));
        let mean: f32 = (0..4096).map(|i| unit(hash2(3, i % 64, i / 64))).sum::<f32>() / 4096.0;
        assert!((mean - 0.5).abs() < 0.03, "{mean}");
    }

    #[test]
    fn value_noise_is_continuous_and_bounded() {
        let mut prev = value_noise(9, 0.0, 0.3);
        for i in 1..400 {
            let v = value_noise(9, i as f32 * 0.01, 0.3);
            assert!((0.0..=1.0).contains(&v));
            assert!((v - prev).abs() < 0.08, "jump at {i}");
            prev = v;
        }
    }
}
