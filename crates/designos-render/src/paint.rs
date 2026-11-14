//! Paint (solid, gradient, image) to tiny-skia shader conversion and helpers
//! for filling / stroking paths with document paints.

use crate::geom::transformed_bounds;
use designos_doc as d;
use designos_doc::{Document, PaintKind, Vec2};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet, VecDeque};
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
#[derive(Clone, Copy)]
pub struct PaintEnv<'a> {
    pub doc: &'a Document,
    /// Convert the document's colour profile to sRGB while drawing. Off for a render whose output is
    /// tagged with the document's own profile (a Display P3 export).
    pub to_srgb: bool,
}

impl<'a> PaintEnv<'a> {
    /// Draws for an sRGB surface: a Display P3 document's colours are converted.
    pub fn new(doc: &'a Document) -> Self {
        Self { doc, to_srgb: true }
    }

    /// Draws the document's colour numbers as they are (output tagged with the document profile).
    pub fn native(doc: &'a Document) -> Self {
        Self { doc, to_srgb: false }
    }

    /// `img` (decoded image bytes, which are sRGB) as it must be drawn. Colours are converted to sRGB
    /// when drawing for the screen; in a native render of a Display P3 document the pixels are
    /// converted from sRGB to Display P3 so the file's profile tag is true for photos too.
    ///
    /// The converted pixmap is cached per (image, size, profile): converting is a pass over every pixel,
    /// and the eyedropper loupe and an export with many layers on one photo ask for it again and again.
    pub(crate) fn image(&self, img: Rc<sk::Pixmap>, key: ImgKey) -> Rc<sk::Pixmap> {
        if self.to_srgb || self.doc.color_profile.is_srgb() {
            return img;
        }
        let profile = self.doc.color_profile;
        if let Some(hit) = CONVERTED_IMAGES.with(|c| c.borrow().get(&(key, profile))) {
            return hit;
        }
        CONVERSIONS.with(|c| c.set(c.get() + 1));
        let converter = d::SrgbPixelConverter::new(profile);
        let mut out = (*img).clone();
        for px in out.pixels_mut() {
            let c = px.demultiply();
            let [r, g, b] = converter.convert([c.red(), c.green(), c.blue()]);
            *px = sk::ColorU8::from_rgba(r, g, b, c.alpha()).premultiply();
        }
        let out = Rc::new(out);
        CONVERTED_IMAGES.with(|c| c.borrow_mut().insert((key, profile), out.clone()));
        out
    }

    /// `n` with its grain colours converted the way [`Self::col`] converts, `None` when unchanged.
    pub fn noise(&self, n: &d::NoiseEffect) -> Option<d::NoiseEffect> {
        if self.to_srgb {
            self.doc.color_profile.noise_to_srgb(n)
        } else {
            None
        }
    }

    /// `c` as it must be drawn: converted from the document profile to sRGB, or `c` itself for an
    /// sRGB document or a native render.
    pub fn col(&self, c: &d::Color) -> d::Color {
        if self.to_srgb {
            self.doc.color_profile.to_srgb(c)
        } else {
            *c
        }
    }
}

type ImgKey = (u64, u32, u32);
type ConvertedKey = (ImgKey, d::ColorProfile);

/// Byte budget for the per-thread decoded-image cache (64 MiB), exceeded only when a single image and its most recent reduced copy together exceed the bound.
pub const MAX_DECODED_IMAGE_BYTES: usize = 64 << 20;
const MAX_DECODED_IMAGE_ENTRIES: usize = 2048;

/// Byte budget for the per-thread converted-image cache (64 MiB).
pub const MAX_CONVERTED_IMAGE_BYTES: usize = 64 << 20;

#[derive(Default)]
struct ConvertedCache {
    entries: HashMap<ConvertedKey, Rc<sk::Pixmap>>,
    order: VecDeque<ConvertedKey>,
    bytes: usize,
}

impl ConvertedCache {
    fn get(&self, key: &ConvertedKey) -> Option<Rc<sk::Pixmap>> {
        self.entries.get(key).cloned()
    }

    fn insert(&mut self, key: ConvertedKey, pm: Rc<sk::Pixmap>) {
        let new_bytes = pm.data().len();
        if let Some(old) = self.entries.remove(&key) {
            self.bytes = self.bytes.saturating_sub(old.data().len());
            self.order.retain(|k| *k != key);
        }
        // An image over the budget is still kept (the eyedropper samples it every frame), but alone: it evicts
        // everything else, so peak memory is max(budget, that image) and the next insert evicts it.
        while self.bytes + new_bytes > MAX_CONVERTED_IMAGE_BYTES && !self.order.is_empty() {
            if let Some(oldest) = self.order.pop_front() {
                if let Some(old) = self.entries.remove(&oldest) {
                    self.bytes = self.bytes.saturating_sub(old.data().len());
                }
            }
        }
        self.bytes += new_bytes;
        self.entries.insert(key, pm);
        self.order.push_back(key);
    }

    fn retain_ids(&mut self, keep: &HashSet<u64>) {
        self.entries.retain(|((id, _, _), _), pm| {
            if keep.contains(id) {
                true
            } else {
                self.bytes = self.bytes.saturating_sub(pm.data().len());
                false
            }
        });
        self.order.retain(|((id, _, _), _)| keep.contains(id));
    }
}

#[derive(Default)]
struct DecodedCache {
    entries: HashMap<ImgKey, Option<Rc<sk::Pixmap>>>,
    order: VecDeque<ImgKey>,
    bytes: usize,
}

impl DecodedCache {
    fn get(&self, key: &ImgKey) -> Option<Option<Rc<sk::Pixmap>>> {
        self.entries.get(key).cloned()
    }

    fn insert(&mut self, key: ImgKey, image: Option<Rc<sk::Pixmap>>) {
        if let Some(old) = self.entries.remove(&key) {
            if let Some(old) = old {
                self.bytes = self.bytes.saturating_sub(old.data().len());
            }
            self.order.retain(|k| *k != key);
        }
        let new_bytes = image.as_ref().map_or(0, |pm| pm.data().len());
        let keep_full = (key.1 != 0 || key.2 != 0).then_some((key.0, 0, 0));
        let mut skipped_full = false;
        // An image over the budget may be held together with its own most recent reduced copy so
        // repeated reduced paints do not decode it again; everything else is evicted oldest first.
        while (self.bytes + new_bytes > MAX_DECODED_IMAGE_BYTES || self.entries.len() >= MAX_DECODED_IMAGE_ENTRIES) && !self.order.is_empty() {
            if let Some(oldest) = self.order.pop_front() {
                if Some(oldest) == keep_full {
                    skipped_full = true;
                    continue;
                }
                if let Some(old) = self.entries.remove(&oldest).flatten() {
                    self.bytes = self.bytes.saturating_sub(old.data().len());
                }
            }
        }
        self.bytes += new_bytes;
        self.entries.insert(key, image);
        self.order.push_back(key);
        if skipped_full {
            self.order.push_back((key.0, 0, 0));
        }
    }

    fn retain_ids(&mut self, keep: &HashSet<u64>) {
        self.entries.retain(|(id, _, _), image| {
            if keep.contains(id) {
                true
            } else {
                if let Some(image) = image {
                    self.bytes = self.bytes.saturating_sub(image.data().len());
                }
                false
            }
        });
        self.order.retain(|(id, _, _)| keep.contains(id));
    }
}

thread_local! {
    /// Images already converted to a document profile, by (image, size) and profile.
    static CONVERTED_IMAGES: RefCell<ConvertedCache> = RefCell::new(ConvertedCache::default());
    static CONVERSIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static IMAGES: RefCell<DecodedCache> = RefCell::new(DecodedCache::default());
    static DECODES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many image pixmaps this thread has converted to a document profile (a counter for tests: a
/// repeated native render of an unchanged image must not convert it again).
#[doc(hidden)]
pub fn image_conversions() -> usize {
    CONVERSIONS.with(std::cell::Cell::get)
}

/// How many encoded images this thread has decoded into RGBA pixmaps (a counter for tests: a
/// repeated render of an unchanged image must not decode it again).
#[doc(hidden)]
pub fn image_decodes() -> usize {
    DECODES.with(std::cell::Cell::get)
}

/// Bytes currently held by the thread-local converted-image cache.
#[doc(hidden)]
pub fn converted_image_bytes() -> usize {
    CONVERTED_IMAGES.with(|c| c.borrow().bytes)
}

/// Bytes currently held by the thread-local decoded-image cache.
#[doc(hidden)]
pub fn decoded_image_bytes() -> usize {
    IMAGES.with(|c| c.borrow().bytes)
}

/// Keeps only cached decoded and profile-converted pixmaps whose image hash is held by a surviving
/// document or its undo/redo history, dropping every other entry.
pub fn retain_converted_images<'a>(surviving: impl IntoIterator<Item = (&'a Document, Option<&'a d::History>)>) {
    let mut keep = HashSet::new();
    for (doc, hist) in surviving {
        for (k, v) in &doc.images {
            keep.insert(hash_bytes(k, v));
        }
        if let Some(h) = hist {
            h.for_each_image(|k, v| {
                keep.insert(hash_bytes(k, v));
            });
        }
    }
    CONVERTED_IMAGES.with(|c| c.borrow_mut().retain_ids(&keep));
    IMAGES.with(|c| c.borrow_mut().retain_ids(&keep));
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
    if let Some(hit) = IMAGES.with(|c| c.borrow().get(&(id, kw, kh))) {
        return hit;
    }
    let full = match IMAGES.with(|c| c.borrow().get(&(id, 0, 0))) {
        Some(f) => f,
        None => {
            DECODES.with(|n| n.set(n.get() + 1));
            let decoded = image::load_from_memory(data).ok().and_then(|i| premultiplied_pixmap(&i.to_rgba8())).map(Rc::new);
            IMAGES.with(|c| c.borrow_mut().insert((id, 0, 0), decoded.clone()));
            decoded
        }
    };
    let full = full?;
    let cache_resized = matches!(want, Some((w, h)) if w < full.width() || h < full.height());
    let result = match want {
        Some((w, h)) if cache_resized => {
            let src = image::RgbaImage::from_raw(full.width(), full.height(), full.data().to_vec())?;
            let small = image::imageops::resize(&src, w.max(1), h.max(1), image::imageops::FilterType::Triangle);
            let mut pm = sk::Pixmap::new(small.width(), small.height())?;
            pm.data_mut().copy_from_slice(small.as_raw());
            Some(Rc::new(pm))
        }
        _ => Some(full),
    };
    // A full-size request returns the same allocation cached under `(id, 0, 0)`. Do not add an
    // alias entry: the byte budget tracks allocations, so counting that Rc twice would evict
    // unrelated images before the cache actually reaches its limit.
    if cache_resized {
        IMAGES.with(|c| c.borrow_mut().insert((id, kw, kh), result.clone()));
    }
    result
}

fn gradient_stops(env: &PaintEnv, stops: &[d::GradientStop], alpha: f32) -> Vec<sk::GradientStop> {
    let mut s: Vec<&d::GradientStop> = stops.iter().collect();
    s.sort_by(|a, b| a.pos.partial_cmp(&b.pos).unwrap_or(std::cmp::Ordering::Equal));
    s.iter().map(|g| sk::GradientStop::new(g.pos.clamp(0.0, 1.0), sk_color(&env.col(&g.color), alpha))).collect()
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
fn raster_gradient(env: &PaintEnv, kind: &PaintKind, size: Vec2, ts: &sk::Transform, alpha: f32, region: (i32, i32, i32, i32)) -> Option<sk::Pixmap> {
    let (x0, y0, x1, y1) = region;
    let (w, h) = ((x1 - x0).max(0) as u32, (y1 - y0).max(0) as u32);
    let mut pm = sk::Pixmap::new(w.max(1), h.max(1))?;
    let (handles, stops, angular) = match kind {
        PaintKind::AngularGradient { handles, stops } => (handles, stops, true),
        PaintKind::DiamondGradient { handles, stops } => (handles, stops, false),
        _ => return None,
    };
    let mut sorted: Vec<d::GradientStop> = stops.iter().map(|g| d::GradientStop { color: env.col(&g.color), ..*g }).collect();
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
            paint.set_color(sk_color(&env.col(c), a));
            Some(f(&paint))
        }
        PaintKind::Shader(_) => None,
        PaintKind::LinearGradient { handles, stops } | PaintKind::RadialGradient { handles, stops } => {
            let radial = matches!(p.kind, PaintKind::RadialGradient { .. });
            let st = gradient_stops(env, stops, a);
            if st.is_empty() {
                return None;
            }
            if st.len() == 1 {
                let c = stops[0].color;
                paint.set_color(sk_color(&env.col(&c), a));
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
            let pm = raster_gradient(env, &p.kind, size, ts, a, region)?;
            let inv = ts.invert()?;
            let shader_ts = sk::Transform::from_translate(region.0 as f32, region.1 as f32).post_concat(inv);
            paint.shader = sk::Pattern::new(pm.as_ref(), sk::SpreadMode::Pad, sk::FilterQuality::Nearest, 1.0, shader_ts);
            Some(f(&paint))
        }
        PaintKind::Image { image_ref, scale_mode, params } => {
            // first decode at full size to learn dimensions
            let full = load_image(env.doc, image_ref, None)?;
            let image_id = env.doc.images.get(image_ref).map(|data| hash_bytes(image_ref, data))?;
            let (iw, ih) = (full.width() as f32, full.height() as f32);
            let map = d::image_uv_map(size, iw, ih, *scale_mode, params)?;
            let (u, v, s) = (map.u, map.v, map.scale);
            let dev_scale = (ts.sx * ts.sy - ts.kx * ts.ky).abs().sqrt().max(1e-6);
            let eff = s * dev_scale;
            let img = if eff < 0.6 {
                let w = (iw * eff).ceil().max(1.0) as u32;
                let h = (ih * eff).ceil().max(1.0) as u32;
                env.image(load_image(env.doc, image_ref, Some((w, h)))?, (image_id, w, h))
            } else {
                env.image(full, (image_id, 0, 0))
            };
            let (spread, quality) = match scale_mode {
                d::ImageScaleMode::Tile => (sk::SpreadMode::Repeat, sk::FilterQuality::Bilinear),
                _ => (sk::SpreadMode::Pad, if eff > 1.0 { sk::FilterQuality::Bicubic } else { sk::FilterQuality::Bilinear }),
            };
            let (w_f, h_f) = (img.width() as f32, img.height() as f32);
            let local_to_px = sk::Transform::from_row(u.0 * w_f, v.0 * h_f, u.1 * w_f, v.1 * h_f, u.2 * w_f, v.2 * h_f);
            let shader_ts = local_to_px.invert()?;
            let crop_leaves_unit_uv = matches!(scale_mode, d::ImageScaleMode::Crop)
                && [(0.0, 0.0), (size.x, 0.0), (0.0, size.y), (size.x, size.y)].into_iter().any(|(x, y)| {
                    let uu = u.0 * x + u.1 * y + u.2;
                    let vv = v.0 * x + v.1 * y + v.2;
                    let unit = -1e-5..=1.0 + 1e-5;
                    !unit.contains(&uu) || !unit.contains(&vv)
                });
            let needs_border = matches!(scale_mode, d::ImageScaleMode::Fit) || crop_leaves_unit_uv;
            let mut adjustment_fallback = false;
            if crate::image_adjust::has_rendered_adjustments(&params.adjust) {
                let (x0, y0, x1, y1) = transformed_bounds(ts, -margin, -margin, size.x + 2.0 * margin, size.y + 2.0 * margin);
                let region: crate::image_adjust::DeviceRegion = (
                    (x0.floor() as i32).max(0),
                    (y0.floor() as i32).max(0),
                    (x1.ceil() as i32).min(target_dims.0 as i32),
                    (y1.ceil() as i32).min(target_dims.1 as i32),
                );
                if region.2 <= region.0 || region.3 <= region.1 {
                    return None;
                }
                let inv = ts.invert()?;
                // The picture itself is sampled; the letterbox and out-of-bounds crop are cleared inside the scratch.
                let source =
                    crate::image_adjust::FillSource { src: (*img).as_ref(), spread, quality, pattern_ts: shader_ts, clear_outside_picture: needs_border };
                if let Some(scratch) = crate::image_adjust::adjusted_fill_region(&source, ts, region, &params.adjust) {
                    let from_scratch = sk::Transform::from_translate(region.0 as f32, region.1 as f32).post_concat(inv);
                    paint.shader = sk::Pattern::new(scratch.as_ref(), sk::SpreadMode::Pad, sk::FilterQuality::Nearest, a, from_scratch);
                    return Some(f(&paint));
                }
                // The scratch is over budget or could not be allocated: the fill is drawn without its adjustments and counted.
                crate::image_adjust::note_unadjusted_fallback();
                adjustment_fallback = true;
            }
            if needs_border {
                // Sample only the visible device region into a transparent scratch. This
                // replaces the old full-picture `(w + 2) x (h + 2)` padded copy while
                // preserving Pad semantics for Fit letterboxes and out-of-bounds Crop.
                // Include the caller's paint margin (for example, a centered or
                // outside stroke). The old padded source supplied a transparent
                // texel beyond the node, so the stroke footprint must sample the
                // same transparent border instead of padding the scratch edge.
                let (x0, y0, x1, y1) = transformed_bounds(ts, -margin, -margin, size.x + 2.0 * margin, size.y + 2.0 * margin);
                let region: crate::image_adjust::DeviceRegion = (
                    (x0.floor() as i32).max(0),
                    (y0.floor() as i32).max(0),
                    (x1.ceil() as i32).min(target_dims.0 as i32),
                    (y1.ceil() as i32).min(target_dims.1 as i32),
                );
                // The region follows the node's box. A caller that paints outside the box (an outline, a
                // stroke of text, an arrow head) can reach a target the box misses, and then there is
                // nothing to sample: the picture with a transparent border covers that case too.
                if region.2 > region.0 && region.3 > region.1 {
                    let source = crate::image_adjust::FillSource { src: (*img).as_ref(), spread, quality, pattern_ts: shader_ts, clear_outside_picture: true };
                    let sampled = if adjustment_fallback {
                        crate::image_adjust::sampled_fill_region_unadjusted_fallback(&source, ts, region)
                    } else {
                        crate::image_adjust::sampled_fill_region(&source, ts, region)
                    };
                    if let Some(scratch) = sampled {
                        let inv = ts.invert()?;
                        let from_scratch = sk::Transform::from_translate(region.0 as f32, region.1 as f32).post_concat(inv);
                        paint.shader = sk::Pattern::new(scratch.as_ref(), sk::SpreadMode::Pad, sk::FilterQuality::Nearest, a, from_scratch);
                        return Some(f(&paint));
                    }
                    // The visible region is over budget or its scratch could not be allocated.
                    crate::image_adjust::note_unadjusted_fallback();
                }
                // A Fit letterbox or an out-of-bounds crop must still come out transparent, so sample the
                // picture with a 1px transparent border: Pad then extends that border, not the picture's
                // edge texel.
                #[cfg(test)]
                crate::image_adjust::note_fill_allocation(img.width() + 2, img.height() + 2);
                let mut padded = sk::Pixmap::new(img.width() + 2, img.height() + 2)?;
                padded.draw_pixmap(1, 1, (*img).as_ref(), &sk::PixmapPaint::default(), sk::Transform::identity(), None);
                let pattern_ts = sk::Transform::from_translate(-1.0, -1.0).post_concat(shader_ts);
                paint.shader = sk::Pattern::new(padded.as_ref(), spread, quality, a, pattern_ts);
                return Some(f(&paint));
            }
            paint.shader = sk::Pattern::new((*img).as_ref(), spread, quality, a, shader_ts);
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

#[cfg(test)]
mod tests {
    use super::{decoded_image_bytes, load_image, retain_converted_images};
    use designos_doc::Document;
    use std::io::Cursor;
    use std::rc::Rc;

    #[test]
    fn full_size_decode_request_does_not_double_count_the_pixmap() {
        let side = 64;
        let image = image::RgbaImage::from_pixel(side, side, image::Rgba([10, 20, 30, 255]));
        let mut bytes = Cursor::new(Vec::new());
        image.write_to(&mut bytes, image::ImageFormat::Png).unwrap();
        let mut doc = Document::new("full-size-cache");
        doc.images.insert("image".into(), bytes.into_inner());
        retain_converted_images(std::iter::empty());

        let full = load_image(&doc, "image", None).unwrap();
        let requested = load_image(&doc, "image", Some((side, side))).unwrap();
        assert!(Rc::ptr_eq(&full, &requested));
        assert_eq!(decoded_image_bytes(), (side * side * 4) as usize);
    }

    #[test]
    fn decoded_cache_replaces_and_evicts_oversized_entry_with_exact_byte_count() {
        let mut cache = super::DecodedCache::default();
        let pm_a = Rc::new(super::sk::Pixmap::new(16, 16).unwrap());
        let pm_b = Rc::new(super::sk::Pixmap::new(32, 32).unwrap());
        cache.insert((1, 0, 0), Some(pm_a.clone()));
        assert_eq!(cache.bytes, pm_a.data().len());
        cache.insert((1, 0, 0), Some(pm_b.clone()));
        assert_eq!(cache.bytes, pm_b.data().len());
        assert_eq!(cache.entries.len(), 1);

        let full = Rc::new(super::sk::Pixmap::new(4100, 4100).unwrap());
        let red1 = Rc::new(super::sk::Pixmap::new(64, 64).unwrap());
        let red2 = Rc::new(super::sk::Pixmap::new(32, 32).unwrap());
        cache.insert((2, 0, 0), Some(full.clone()));
        cache.insert((2, 64, 64), Some(red1.clone()));
        assert!(cache.entries.contains_key(&(2, 0, 0)));
        assert!(cache.entries.contains_key(&(2, 64, 64)));
        assert_eq!(cache.bytes, full.data().len() + red1.data().len());

        cache.insert((2, 32, 32), Some(red2.clone()));
        assert!(cache.entries.contains_key(&(2, 0, 0)));
        assert!(!cache.entries.contains_key(&(2, 64, 64)));
        assert!(cache.entries.contains_key(&(2, 32, 32)));
        assert_eq!(cache.bytes, full.data().len() + red2.data().len());

        cache.insert((3, 0, 0), Some(pm_a.clone()));
        assert!(!cache.entries.contains_key(&(2, 0, 0)));
        assert!(!cache.entries.contains_key(&(2, 32, 32)));
        assert!(cache.entries.contains_key(&(3, 0, 0)));
        assert_eq!(cache.bytes, pm_a.data().len());
    }
}
