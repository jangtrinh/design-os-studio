//! Document paints -> per-draw shader data (solid colours, gradient ramps,
//! decoded images).

use crate::frame::*;
use designos_doc as d;
use designos_doc::{Document, PaintKind, Vec2};
use designos_render::decoded_image;
use designos_render::tiny_skia as sk;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

static NEXT_IMAGE_ID: AtomicU64 = AtomicU64::new(1);

/// Shader data for one paint (the transform is filled in by the caller).
#[derive(Clone)]
pub struct PaintInst {
    pub inst: Instance,
    pub ramp: Option<Arc<RampData>>,
    pub image: Option<Arc<ImageData>>,
}

#[derive(Default)]
pub struct PaintCache {
    ramps: HashMap<u64, Arc<RampData>>,
    images: HashMap<String, (usize, Option<Arc<ImageData>>)>,
}

fn clamp01(v: f32) -> f32 {
    v.clamp(0.0, 1.0)
}

fn color4(c: &d::Color, alpha: f32) -> [f32; 4] {
    [clamp01(c.r), clamp01(c.g), clamp01(c.b), clamp01(c.a * alpha)]
}

/// Unit gradient space -> node-local transform (handles in node-normalised space).
fn gradient_to_local(handles: &[Vec2; 3], size: Vec2) -> sk::Transform {
    let [h0, h1, h2] = *handles;
    let g = sk::Transform::from_row(h1.x - h0.x, h1.y - h0.y, h2.x - h0.x, h2.y - h0.y, h0.x, h0.y);
    sk::Transform::from_scale(size.x, size.y).pre_concat(g)
}

/// Straight-interpolated, premultiplied colour of sorted `stops` at `t`.
fn eval_stops(stops: &[d::GradientStop], t: f32) -> [f32; 4] {
    let first = &stops[0];
    let last = &stops[stops.len() - 1];
    let (lo, hi) = if t <= first.pos {
        (first, first)
    } else if t >= last.pos {
        (last, last)
    } else {
        let mut pair = (first, last);
        for w in stops.windows(2) {
            if t >= w[0].pos && t <= w[1].pos {
                pair = (&w[0], &w[1]);
                break;
            }
        }
        pair
    };
    let f = if hi.pos > lo.pos { ((t - lo.pos) / (hi.pos - lo.pos)).clamp(0.0, 1.0) } else { 0.0 };
    let l = |a: f32, b: f32| a + (b - a) * f;
    let a = clamp01(l(lo.color.a, hi.color.a));
    [clamp01(l(lo.color.r, hi.color.r)) * a, clamp01(l(lo.color.g, hi.color.g)) * a, clamp01(l(lo.color.b, hi.color.b)) * a, a]
}

fn stops_key(stops: &[d::GradientStop]) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for s in stops {
        s.pos.to_bits().hash(&mut h);
        for v in [s.color.r, s.color.g, s.color.b, s.color.a] {
            v.to_bits().hash(&mut h);
        }
    }
    h.finish()
}

impl PaintCache {
    fn ramp(&mut self, profile: d::ColorProfile, stops: &[d::GradientStop]) -> Arc<RampData> {
        let mut sorted: Vec<d::GradientStop> = stops.iter().map(|s| d::GradientStop { pos: s.pos, color: profile.to_srgb(&s.color) }).collect();
        sorted.sort_by(|a, b| a.pos.partial_cmp(&b.pos).unwrap_or(std::cmp::Ordering::Equal));
        for s in &mut sorted {
            s.pos = clamp01(s.pos);
        }
        let key = stops_key(&sorted);
        if let Some(r) = self.ramps.get(&key) {
            return r.clone();
        }
        if self.ramps.len() > 4096 {
            self.ramps.clear();
        }
        let mut rgba = Vec::with_capacity(RAMP_W * 4);
        for i in 0..RAMP_W {
            let t = i as f32 / (RAMP_W - 1) as f32;
            let c = eval_stops(&sorted, t);
            for v in c {
                rgba.push((v * 255.0 + 0.5).clamp(0.0, 255.0) as u8);
            }
        }
        let r = Arc::new(RampData { key, rgba });
        self.ramps.insert(key, r.clone());
        r
    }

    fn image(&mut self, doc: &Document, image_ref: &str) -> Option<Arc<ImageData>> {
        let len = doc.images.get(image_ref)?.len();
        if let Some((l, img)) = self.images.get(image_ref) {
            if *l == len {
                return img.clone();
            }
        }
        let img = decoded_image(doc, image_ref)
            .map(|pm| Arc::new(ImageData { id: NEXT_IMAGE_ID.fetch_add(1, Ordering::Relaxed), w: pm.width(), h: pm.height(), rgba: pm.data().to_vec() }));
        if self.images.len() > 2048 {
            self.images.clear();
        }
        self.images.insert(image_ref.to_string(), (len, img.clone()));
        img
    }

    /// Shader data for paint `p` on a node of `size`. `alpha` is the opacity
    /// the caller applies on top of the paint's own. `None` draws nothing.
    pub fn convert(&mut self, doc: &Document, p: &d::Paint, size: Vec2, alpha: f32) -> Option<PaintInst> {
        if !p.visible || p.opacity <= 0.0 {
            return None;
        }
        let a = alpha * p.opacity;
        let mut inst = Instance { t: [0.0, 0.0, a, 0.0], ..Default::default() };
        match &p.kind {
            PaintKind::Solid(c) => {
                inst.q[0] = KIND_SOLID;
                inst.c0 = color4(&doc.color_profile.to_srgb(c), a);
                inst.t[2] = 1.0;
                Some(PaintInst { inst, ramp: None, image: None })
            }
            PaintKind::Shader(_) => None,
            PaintKind::LinearGradient { handles, stops }
            | PaintKind::RadialGradient { handles, stops }
            | PaintKind::AngularGradient { handles, stops }
            | PaintKind::DiamondGradient { handles, stops } => {
                if stops.is_empty() {
                    return None;
                }
                if stops.len() == 1 {
                    inst.q[0] = KIND_SOLID;
                    inst.c0 = color4(&doc.color_profile.to_srgb(&stops[0].color), a);
                    inst.t[2] = 1.0;
                    return Some(PaintInst { inst, ramp: None, image: None });
                }
                let inv = gradient_to_local(handles, size).invert()?;
                inst.q[0] = match &p.kind {
                    PaintKind::LinearGradient { .. } => KIND_LINEAR,
                    PaintKind::RadialGradient { .. } => KIND_RADIAL,
                    PaintKind::AngularGradient { .. } => KIND_ANGULAR,
                    _ => KIND_DIAMOND,
                };
                inst.c1 = [inv.sx, inv.ky, inv.kx, inv.sy];
                inst.c2 = [inv.tx, inv.ty, 0.0, 0.0];
                Some(PaintInst { inst, ramp: Some(self.ramp(doc.color_profile, stops)), image: None })
            }
            PaintKind::Image { image_ref, scale_mode, params } => {
                let img = self.image(doc, image_ref)?;
                let (iw, ih) = (img.w as f32, img.h as f32);
                let d::ImageUvMap { u, v, .. } = d::image_uv_map(size, iw, ih, *scale_mode, params)?;
                inst.q[0] = KIND_IMAGE;
                inst.q[2] = match scale_mode {
                    d::ImageScaleMode::Fit | d::ImageScaleMode::Crop => IMG_FIT,
                    d::ImageScaleMode::Tile => IMG_TILE,
                    d::ImageScaleMode::Fill => IMG_PAD,
                };
                // local -> uv (0..1 over the image)
                inst.c1 = [u.0, v.0, u.1, v.1];
                inst.c2 = [u.2, v.2, 0.0, 0.0];
                if designos_render::image_adjust::has_rendered_adjustments(&params.adjust) {
                    let adj = &params.adjust;
                    inst.q[1] = 1;
                    inst.c0 = [adj.exposure.clamp(-1.0, 1.0), adj.contrast.clamp(-1.0, 1.0), adj.vibrance.clamp(-1.0, 1.0), adj.temperature.clamp(-1.0, 1.0)];
                    inst.c2[2] = adj.tint.clamp(-1.0, 1.0);
                    inst.c2[3] = adj.highlights.clamp(-1.0, 1.0);
                    inst.t[3] = adj.shadows.clamp(-1.0, 1.0);
                }
                Some(PaintInst { inst, ramp: None, image: Some(img) })
            }
        }
    }
}
