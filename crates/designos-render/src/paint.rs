//! Paint (solid, gradient, image) to tiny-skia shader conversion and helpers
//! for filling / stroking paths with document paints.

use crate::geom::transformed_bounds;
use designos_doc as d;
use designos_doc::{Document, PaintKind, Vec2};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use tiny_skia as sk;

pub fn sk_color(c: &d::Color, alpha: f32) -> sk::Color {
    sk::Color::from_rgba(c.r.clamp(0.0, 1.0), c.g.clamp(0.0, 1.0), c.b.clamp(0.0, 1.0), (c.a * alpha).clamp(0.0, 1.0)).unwrap_or(sk::Color::TRANSPARENT)
}

pub fn blend_mode(b: d::BlendMode) -> sk::BlendMode {
    use d::BlendMode as B;
    match b {
        B::Normal | B::PassThrough => sk::BlendMode::SourceOver,
        B::Darken => sk::BlendMode::Darken,
        B::Multiply => sk::BlendMode::Multiply,
        B::ColorBurn => sk::BlendMode::ColorBurn,
        // Linear burn / dodge have no exact tiny-skia equivalent: callers detect them
        // with `custom_blend` and composite the layer with `blend_layer`. Until then
        // the content is drawn normally.
        B::LinearBurn | B::LinearDodge => sk::BlendMode::SourceOver,
        B::Lighten => sk::BlendMode::Lighten,
        B::Screen => sk::BlendMode::Screen,
        B::ColorDodge => sk::BlendMode::ColorDodge,
        B::Overlay => sk::BlendMode::Overlay,
        B::SoftLight => sk::BlendMode::SoftLight,
        B::HardLight => sk::BlendMode::HardLight,
        B::Difference => sk::BlendMode::Difference,
        B::Exclusion => sk::BlendMode::Exclusion,
        B::Hue => sk::BlendMode::Hue,
        B::Saturation => sk::BlendMode::Saturation,
        B::Color => sk::BlendMode::Color,
        B::Luminosity => sk::BlendMode::Luminosity,
    }
}

/// Blend modes composited per pixel by [`blend_layer`] instead of by tiny-skia.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CustomBlend {
    /// `max(0, cb + cs - 1)` per channel.
    LinearBurn,
    /// `min(1, cb + cs)` per channel (the separable blend function; unlike a
    /// plain premultiplied add it stays exact for translucent sources).
    LinearDodge,
}

pub fn custom_blend(b: d::BlendMode) -> Option<CustomBlend> {
    match b {
        d::BlendMode::LinearBurn => Some(CustomBlend::LinearBurn),
        d::BlendMode::LinearDodge => Some(CustomBlend::LinearDodge),
        _ => None,
    }
}

/// Composites `layer` (premultiplied, drawn with normal blending) over
/// `target` using the W3C compositing formula
/// `co = cs(1-ab) + cb(1-as) + as*ab*B(Cb, Cs)` with a custom blend function B.
pub fn blend_layer(target: &mut sk::Pixmap, layer: &sk::Pixmap, opacity: f32, mode: CustomBlend) {
    let opacity = opacity.clamp(0.0, 1.0);
    for (dst, src) in target.data_mut().as_chunks_mut::<4>().0.iter_mut().zip(layer.data().as_chunks::<4>().0.iter()) {
        if src[3] == 0 {
            continue;
        }
        let sa = src[3] as f32 / 255.0 * opacity;
        let ab = dst[3] as f32 / 255.0;
        let mut out = [0u8; 4];
        for k in 0..3 {
            let cs_pre = src[k] as f32 / 255.0 * opacity;
            let cb_pre = dst[k] as f32 / 255.0;
            let cs = if sa > 0.0 { (cs_pre / sa).min(1.0) } else { 0.0 };
            let cb = if ab > 0.0 { (cb_pre / ab).min(1.0) } else { 0.0 };
            let b = match mode {
                CustomBlend::LinearBurn => (cb + cs - 1.0).max(0.0),
                CustomBlend::LinearDodge => (cb + cs).min(1.0),
            };
            let co = cs_pre * (1.0 - ab) + cb_pre * (1.0 - sa) + sa * ab * b;
            out[k] = (co * 255.0 + 0.5).clamp(0.0, 255.0) as u8;
        }
        let ao = sa + ab - sa * ab;
        out[3] = (ao * 255.0 + 0.5).clamp(0.0, 255.0) as u8;
        // keep premultiplied invariants (colour <= alpha)
        let a = out[3];
        for c in out.iter_mut().take(3) {
            *c = (*c).min(a);
        }
        dst.copy_from_slice(&out);
    }
}

/// Runs `draw` against a scratch copy of `target`'s size and composites the
/// result with a custom blend mode.
fn draw_custom(target: &mut sk::Pixmap, mode: CustomBlend, draw: impl FnOnce(&mut sk::Pixmap)) {
    let Some(mut layer) = sk::Pixmap::new(target.width(), target.height()) else { return };
    draw(&mut layer);
    blend_layer(target, &layer, 1.0, mode);
}

/// Environment needed to resolve paints.
pub struct PaintEnv<'a> {
    pub doc: &'a Document,
}

type ImgKey = (u64, u32, u32);

thread_local! {
    static IMAGES: RefCell<HashMap<ImgKey, Option<Rc<sk::Pixmap>>>> = RefCell::new(HashMap::new());
}

fn hash_bytes(key: &str, data: &[u8]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    key.hash(&mut h);
    data.len().hash(&mut h);
    data[..data.len().min(512)].hash(&mut h);
    data[data.len().saturating_sub(512)..].hash(&mut h);
    h.finish()
}

fn premultiplied_pixmap(img: &image::RgbaImage) -> Option<sk::Pixmap> {
    let mut pm = sk::Pixmap::new(img.width(), img.height())?;
    for (dst, src) in pm.data_mut().as_chunks_mut::<4>().0.iter_mut().zip(img.as_raw().as_chunks::<4>().0.iter()) {
        let a = src[3] as u32;
        dst[0] = ((src[0] as u32 * a + 127) / 255) as u8;
        dst[1] = ((src[1] as u32 * a + 127) / 255) as u8;
        dst[2] = ((src[2] as u32 * a + 127) / 255) as u8;
        dst[3] = a as u8;
    }
    Some(pm)
}

/// Decoded image, optionally reduced to `want` size (cached).
pub(crate) fn load_image(doc: &Document, image_ref: &str, want: Option<(u32, u32)>) -> Option<Rc<sk::Pixmap>> {
    let data = doc.images.get(image_ref)?;
    let id = hash_bytes(image_ref, data);
    let (kw, kh) = want.unwrap_or((0, 0));
    if let Some(hit) = IMAGES.with(|c| c.borrow().get(&(id, kw, kh)).cloned()) {
        return hit;
    }
    let full = IMAGES.with(|c| c.borrow().get(&(id, 0, 0)).cloned());
    let full = match full {
        Some(f) => f,
        None => {
            let decoded = image::load_from_memory(data).ok().and_then(|i| premultiplied_pixmap(&i.to_rgba8())).map(Rc::new);
            IMAGES.with(|c| {
                let mut c = c.borrow_mut();
                if c.len() > 1024 {
                    c.clear();
                }
                c.insert((id, 0, 0), decoded.clone());
            });
            decoded
        }
    };
    let full = full?;
    let result = match want {
        Some((w, h)) if w < full.width() || h < full.height() => {
            let src = image::RgbaImage::from_raw(full.width(), full.height(), full.data().to_vec())?;
            let small = image::imageops::resize(&src, w.max(1), h.max(1), image::imageops::FilterType::Triangle);
            let mut pm = sk::Pixmap::new(small.width(), small.height())?;
            pm.data_mut().copy_from_slice(small.as_raw());
            Some(Rc::new(pm))
        }
        _ => Some(full),
    };
    IMAGES.with(|c| {
        let mut c = c.borrow_mut();
        if c.len() > 2048 {
            c.clear();
        }
        c.insert((id, kw, kh), result.clone());
    });
    result
}

fn gradient_stops(stops: &[d::GradientStop], alpha: f32) -> Vec<sk::GradientStop> {
    let mut s: Vec<&d::GradientStop> = stops.iter().collect();
    s.sort_by(|a, b| a.pos.partial_cmp(&b.pos).unwrap_or(std::cmp::Ordering::Equal));
    s.iter().map(|g| sk::GradientStop::new(g.pos.clamp(0.0, 1.0), sk_color(&g.color, alpha))).collect()
}

/// Unit gradient space -> node-local transform.
fn gradient_to_local(handles: &[Vec2; 3], size: Vec2) -> sk::Transform {
    let [h0, h1, h2] = *handles;
    let g = sk::Transform::from_row(h1.x - h0.x, h1.y - h0.y, h2.x - h0.x, h2.y - h0.y, h0.x, h0.y);
    sk::Transform::from_scale(size.x, size.y).pre_concat(g)
}

fn eval_stops(stops: &[d::GradientStop], t: f32, alpha: f32) -> [f32; 4] {
    // returns premultiplied rgba 0..1
    if stops.is_empty() {
        return [0.0; 4];
    }
    let mut lo = &stops[0];
    let mut hi = &stops[stops.len() - 1];
    if t <= lo.pos {
        hi = lo;
    } else if t >= hi.pos {
        lo = hi;
    } else {
        for w in stops.windows(2) {
            if t >= w[0].pos && t <= w[1].pos {
                lo = &w[0];
                hi = &w[1];
                break;
            }
        }
    }
    let f = if hi.pos > lo.pos { ((t - lo.pos) / (hi.pos - lo.pos)).clamp(0.0, 1.0) } else { 0.0 };
    let l = |a: f32, b: f32| a + (b - a) * f;
    let a = l(lo.color.a, hi.color.a) * alpha;
    [l(lo.color.r, hi.color.r) * a, l(lo.color.g, hi.color.g) * a, l(lo.color.b, hi.color.b) * a, a]
}

/// Rasterises an angular or diamond gradient into a device-space pixmap.
fn raster_gradient(kind: &PaintKind, size: Vec2, ts: &sk::Transform, alpha: f32, region: (i32, i32, i32, i32)) -> Option<sk::Pixmap> {
    let (x0, y0, x1, y1) = region;
    let (w, h) = ((x1 - x0).max(0) as u32, (y1 - y0).max(0) as u32);
    let mut pm = sk::Pixmap::new(w.max(1), h.max(1))?;
    let (handles, stops, angular) = match kind {
        PaintKind::AngularGradient { handles, stops } => (handles, stops, true),
        PaintKind::DiamondGradient { handles, stops } => (handles, stops, false),
        _ => return None,
    };
    let mut sorted = stops.clone();
    sorted.sort_by(|a, b| a.pos.partial_cmp(&b.pos).unwrap_or(std::cmp::Ordering::Equal));
    let m = ts.pre_concat(gradient_to_local(handles, size));
    let inv = m.invert()?;
    for yy in 0..pm.height() {
        for xx in 0..pm.width() {
            let (dx, dy) = ((x0 + xx as i32) as f32 + 0.5, (y0 + yy as i32) as f32 + 0.5);
            let ux = inv.sx * dx + inv.kx * dy + inv.tx;
            let uy = inv.ky * dx + inv.sy * dy + inv.ty;
            let t = if angular {
                let a = uy.atan2(ux) / std::f32::consts::TAU;
                if a < 0.0 {
                    a + 1.0
                } else {
                    a
                }
            } else {
                (ux.abs() + uy.abs()).min(1.0)
            };
            let c = eval_stops(&sorted, t, alpha);
            let i = ((yy * pm.width() + xx) * 4) as usize;
            let data = pm.data_mut();
            for k in 0..4 {
                data[i + k] = (c[k] * 255.0 + 0.5).clamp(0.0, 255.0) as u8;
            }
        }
    }
    Some(pm)
}

/// Builds a tiny-skia paint for `p` and runs `f` with it. The shader lives in
/// node-local space and is combined with `ts` when drawing, so callers must
/// draw with `ts` as the fill/stroke transform.
#[allow(clippy::too_many_arguments)]
pub fn with_paint<R>(
    env: &PaintEnv,
    p: &d::Paint,
    size: Vec2,
    ts: &sk::Transform,
    alpha: f32,
    margin: f32,
    target_dims: (u32, u32),
    f: impl FnOnce(&sk::Paint) -> R,
) -> Option<R> {
    if !p.visible || p.opacity <= 0.0 {
        return None;
    }
    let a = alpha * p.opacity;
    let mut paint = sk::Paint { anti_alias: true, blend_mode: blend_mode(p.blend), ..Default::default() };
    match &p.kind {
        PaintKind::Solid(c) => {
            paint.set_color(sk_color(c, a));
            Some(f(&paint))
        }
        PaintKind::LinearGradient { handles, stops } | PaintKind::RadialGradient { handles, stops } => {
            let radial = matches!(p.kind, PaintKind::RadialGradient { .. });
            let st = gradient_stops(stops, a);
            if st.is_empty() {
                return None;
            }
            if st.len() == 1 {
                let c = stops[0].color;
                paint.set_color(sk_color(&c, a));
                return Some(f(&paint));
            }
            let tr = gradient_to_local(handles, size);
            let shader = if radial {
                sk::RadialGradient::new(sk::Point::from_xy(0.0, 0.0), sk::Point::from_xy(0.0, 0.0), 1.0, st, sk::SpreadMode::Pad, tr)
            } else {
                sk::LinearGradient::new(sk::Point::from_xy(0.0, 0.0), sk::Point::from_xy(1.0, 0.0), st, sk::SpreadMode::Pad, tr)
            }?;
            paint.shader = shader;
            Some(f(&paint))
        }
        PaintKind::AngularGradient { .. } | PaintKind::DiamondGradient { .. } => {
            let (x0, y0, x1, y1) = transformed_bounds(ts, -margin, -margin, size.x + 2.0 * margin, size.y + 2.0 * margin);
            let region = (
                (x0.floor() as i32).max(0),
                (y0.floor() as i32).max(0),
                (x1.ceil() as i32).min(target_dims.0 as i32),
                (y1.ceil() as i32).min(target_dims.1 as i32),
            );
            if region.2 <= region.0 || region.3 <= region.1 {
                return None;
            }
            let pm = raster_gradient(&p.kind, size, ts, a, region)?;
            let inv = ts.invert()?;
            let shader_ts = sk::Transform::from_translate(region.0 as f32, region.1 as f32).post_concat(inv);
            paint.shader = sk::Pattern::new(pm.as_ref(), sk::SpreadMode::Pad, sk::FilterQuality::Nearest, 1.0, shader_ts);
            Some(f(&paint))
        }
        PaintKind::Image { image_ref, scale_mode } => {
            // first decode at full size to learn dimensions
            let full = load_image(env.doc, image_ref, None)?;
            let (iw, ih) = (full.width() as f32, full.height() as f32);
            let (s, tx, ty) = match scale_mode {
                d::ImageScaleMode::Fill | d::ImageScaleMode::Crop => {
                    let s = (size.x / iw).max(size.y / ih);
                    (s, (size.x - iw * s) / 2.0, (size.y - ih * s) / 2.0)
                }
                d::ImageScaleMode::Fit => {
                    let s = (size.x / iw).min(size.y / ih);
                    (s, (size.x - iw * s) / 2.0, (size.y - ih * s) / 2.0)
                }
                d::ImageScaleMode::Tile => (1.0, 0.0, 0.0),
            };
            if !s.is_finite() || s <= 0.0 {
                return None;
            }
            let dev_scale = (ts.sx * ts.sy - ts.kx * ts.ky).abs().sqrt().max(1e-6);
            let eff = s * dev_scale;
            let (img, mult) = if eff < 0.6 {
                let w = (iw * eff).ceil().max(1.0) as u32;
                let h = (ih * eff).ceil().max(1.0) as u32;
                let im = load_image(env.doc, image_ref, Some((w, h)))?;
                let m = iw / im.width() as f32;
                (im, m)
            } else {
                (full, 1.0)
            };
            let (spread, quality) = match scale_mode {
                d::ImageScaleMode::Tile => (sk::SpreadMode::Repeat, sk::FilterQuality::Bilinear),
                _ => (sk::SpreadMode::Pad, if eff > 1.0 { sk::FilterQuality::Bicubic } else { sk::FilterQuality::Bilinear }),
            };
            let shader_ts = sk::Transform::from_scale(s * mult, s * mult).post_translate(tx, ty);
            if matches!(scale_mode, d::ImageScaleMode::Fit) {
                // 1px transparent border so Pad leaves the letterbox transparent
                let (w, h) = (img.width(), img.height());
                let mut padded = sk::Pixmap::new(w + 2, h + 2)?;
                padded.draw_pixmap(1, 1, img.as_ref().as_ref(), &sk::PixmapPaint::default(), sk::Transform::identity(), None);
                let ts2 = sk::Transform::from_translate(-1.0, -1.0).post_concat(shader_ts);
                paint.shader = sk::Pattern::new(padded.as_ref(), spread, quality, a, ts2);
                return Some(f(&paint));
            }
            paint.shader = sk::Pattern::new(img.as_ref().as_ref(), spread, quality, a, shader_ts);
            Some(f(&paint))
        }
    }
}

/// Fills `paths` with `paint`.
#[allow(clippy::too_many_arguments)]
pub fn fill_paths(
    env: &PaintEnv,
    target: &mut sk::Pixmap,
    paths: &[(sk::Path, sk::FillRule)],
    ts: sk::Transform,
    size: Vec2,
    paint: &d::Paint,
    alpha: f32,
    clip: Option<&sk::Mask>,
) {
    if let Some(mode) = custom_blend(paint.blend) {
        let normal = d::Paint { blend: d::BlendMode::Normal, ..paint.clone() };
        draw_custom(target, mode, |l| fill_paths(env, l, paths, ts, size, &normal, alpha, clip));
        return;
    }
    let dims = (target.width(), target.height());
    with_paint(env, paint, size, &ts, alpha, 0.0, dims, |sp| {
        for (path, rule) in paths {
            target.fill_path(path, sp, *rule, ts, clip);
        }
    });
}

/// Strokes `paths` with `paint`.
#[allow(clippy::too_many_arguments)]
pub fn stroke_paths(
    env: &PaintEnv,
    target: &mut sk::Pixmap,
    paths: &[(sk::Path, sk::FillRule)],
    stroke: &sk::Stroke,
    ts: sk::Transform,
    size: Vec2,
    paint: &d::Paint,
    alpha: f32,
    clip: Option<&sk::Mask>,
) {
    if let Some(mode) = custom_blend(paint.blend) {
        let normal = d::Paint { blend: d::BlendMode::Normal, ..paint.clone() };
        draw_custom(target, mode, |l| stroke_paths(env, l, paths, stroke, ts, size, &normal, alpha, clip));
        return;
    }
    let dims = (target.width(), target.height());
    with_paint(env, paint, size, &ts, alpha, stroke.width, dims, |sp| {
        for (path, _) in paths {
            target.stroke_path(path, sp, stroke, ts, clip);
        }
    });
}
