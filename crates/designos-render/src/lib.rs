//! Renders a [`designos_doc::Document`] page into a `tiny_skia::Pixmap`.
//! CONTRACT: keep `Viewport` and `render_page` signatures stable; the app and
//! the diff harness depend on them.
//!
//! The renderer is a CPU painter built on tiny-skia (deterministic output).
//! Every node is drawn in back-to-front order with Figma semantics: fills,
//! inner shadows, strokes, then children; drop shadows and background blur
//! come first. Group effects (opacity, blend, layer blur) use full-size
//! offscreen layers.

mod blur;
pub mod board;
mod boolean;
pub mod diff_image;
mod effects;
mod fx;
mod geom;
mod index;
mod mask;
mod outline;
mod paint;
mod shadow;
#[cfg(test)]
mod shadow_tests;
mod text;
mod tiles;
pub mod vectorops;

use designos_doc::{resolve_node, Document, Effect, Node, NodeId, NodeKind, Vec2};
pub use diff_image::{render_diff_image, DiffImageResult};
use geom::{node_shape, to_sk_transform, transformed_bounds, Shape};
pub use index::SceneIndex;
pub use mask::{mask_frame_box, mask_stencil};
pub use outline::{
    decoded_image, miter_outline, node_margin, node_stroke_style, outline_of, render_node_into, stroke_outline, stroke_plan, text_outlines, NodeShape,
    StrokeClip, StrokePlan, TextOutline,
};
use paint::{blend_mode, fill_paths, stroke_paths, PaintEnv};
pub use text::{
    available_families, font_features, font_generation, load_font_data, register_font, resolve_font, text_geometry, text_runs, GlyphRun, GlyphSpan, LineGeom,
    ResolvedFont, RunLine, TextGeometry, TextRuns,
};
pub use tiles::{Tile, TileBudget, TileCache, TileFrame, TileKey, TILE_SIZE};
pub use tiny_skia;
use tiny_skia as sk;

/// Benchmark and test hook: `false` switches blurs and shadows back to the
/// original unoptimised implementation (same API, slower).
#[doc(hidden)]
pub fn set_fast_effects(on: bool) {
    blur::set_reference_mode(!on);
}

/// Benchmark hook: blurs `pm` with the optimised or the original blur.
#[doc(hidden)]
pub fn bench_blur(pm: &mut tiny_skia::Pixmap, sigma: f32, reference: bool, margined: bool) {
    if reference {
        blur::gaussian_blur_reference(pm, sigma);
    } else {
        blur::gaussian_blur_at(pm, sigma, (0, 0), if margined { blur::Edges::Margined } else { blur::Edges::Exact });
    }
}

/// Number of blurred shadow masks currently cached (diagnostics).
#[doc(hidden)]
pub fn shadow_cache_len() -> usize {
    shadow::cached_patches()
}

/// Empties the shadow mask cache (diagnostics).
#[doc(hidden)]
pub fn clear_shadow_cache() {
    shadow::clear_cache();
}

/// Maps page space to device pixels: device = (page - offset) * zoom * dpr.
#[derive(Clone, Copy, Debug)]
pub struct Viewport {
    /// Page-space point shown at the top-left of the pixmap.
    pub offset: Vec2,
    pub zoom: f32,
    /// Device pixel ratio (2.0 on hi-dpi).
    pub dpr: f32,
}

impl Default for Viewport {
    fn default() -> Self {
        Self { offset: Vec2::ZERO, zoom: 1.0, dpr: 1.0 }
    }
}

/// Draws `page` (background + all visible descendants) into `target`.
pub fn render_page(doc: &Document, page: NodeId, vp: &Viewport, target: &mut tiny_skia::Pixmap) {
    let bg = match doc.get(page).map(|n| &n.kind) {
        Some(NodeKind::Page { background }) => paint::sk_color(background, 1.0),
        _ => sk::Color::from_rgba8(0x5A, 0x5A, 0x5A, 0xFF),
    };
    target.fill(bg);
    let Some(p) = doc.get(page) else { return };
    let s = vp.zoom * vp.dpr;
    let ts = sk::Transform::from_row(s, 0.0, 0.0, s, -vp.offset.x * s, -vp.offset.y * s);
    effects::set_grid_origin(&ts);
    let r = Renderer { doc, env: PaintEnv { doc }, cull: None };
    r.draw_children(&p.children, ts, target, None, None);
}

/// Like [`render_page`], but skips subtrees that do not intersect `target`
/// using a cached spatial index. `rev` is a document revision counter: the
/// index is rebuilt only when it (or `page`) changes since the last call, so
/// callers must bump it on every document edit. Output is identical to
/// [`render_page`].
pub fn render_page_indexed(doc: &Document, page: NodeId, vp: &Viewport, target: &mut tiny_skia::Pixmap, idx: &mut SceneIndex, rev: u64) {
    idx.ensure(doc, page, rev);
    let s = vp.zoom * vp.dpr;
    let ts = sk::Transform::from_row(s, 0.0, 0.0, s, -vp.offset.x * s, -vp.offset.y * s);
    render_with_transform(doc, page, ts, target, Some(&*idx));
}

/// Paints `page` through `ts` (page space -> target pixels). `index`, if given,
/// must have been built for this document revision and `ts` must be a pure
/// scale + translate.
pub(crate) fn render_with_transform(doc: &Document, page: NodeId, ts: sk::Transform, target: &mut sk::Pixmap, index: Option<&SceneIndex>) {
    let bg = match doc.get(page).map(|n| &n.kind) {
        Some(NodeKind::Page { background }) => paint::sk_color(background, 1.0),
        _ => sk::Color::from_rgba8(0x5A, 0x5A, 0x5A, 0xFF),
    };
    target.fill(bg);
    let Some(p) = doc.get(page) else { return };
    effects::set_grid_origin(&ts);
    let cull = index.map(|index| {
        // page-space window covered by the target (1 device px of slack)
        let (w, h) = (target.width() as f32, target.height() as f32);
        let win = ((-1.0 - ts.tx) / ts.sx, (-1.0 - ts.ty) / ts.sy, (w + 1.0 - ts.tx) / ts.sx, (h + 1.0 - ts.ty) / ts.sy);
        Cull { index, win }
    });
    let r = Renderer { doc, env: PaintEnv { doc }, cull };
    r.draw_children(&p.children, ts, target, None, None);
}

/// Longest side, in device pixels, of any off-screen image the renderer will allocate.
pub const MAX_PIXMAP_SIDE: u32 = 16_384;
/// Most device pixels (width x height) of any off-screen image the renderer will allocate: 16384 x 16384
/// (1 GiB RGBA), equal to [`MAX_PIXMAP_SIDE`] squared and to the largest GPU texture. Every size the side
/// cap admits therefore fits the budget, and exports such as 4000x3000 at 4x (192 Mpx) are allowed.
pub const MAX_PIXMAP_PIXELS: u64 = 1 << 28;

/// Pixel size of a `w` x `h` (already scaled) image, at least 1x1; `None` if it is not finite or
/// exceeds [`MAX_PIXMAP_SIDE`] / [`MAX_PIXMAP_PIXELS`], so an oversized request fails instead of aborting on allocation.
pub fn checked_pixmap_size(w: f32, h: f32) -> Option<(u32, u32)> {
    if !w.is_finite() || !h.is_finite() {
        return None;
    }
    let (w, h) = (w.ceil().max(1.0), h.ceil().max(1.0));
    let side = MAX_PIXMAP_SIDE as f32;
    if w > side || h > side || (w as u64) * (h as u64) > MAX_PIXMAP_PIXELS {
        return None;
    }
    Some((w as u32, h as u32))
}

/// Draws a single node subtree with its own top-left at (0,0); used for
/// exports and per-frame diffs. `None` if the node is missing or the image would exceed
/// [`MAX_PIXMAP_SIDE`] / [`MAX_PIXMAP_PIXELS`].
pub fn render_node(doc: &Document, node: NodeId, scale: f32) -> Option<tiny_skia::Pixmap> {
    let n = &*resolve_node(doc, doc.get(node)?);
    let (w, h) = checked_pixmap_size(n.size.x * scale, n.size.y * scale)?;
    let mut pm = tiny_skia::Pixmap::new(w, h)?;
    let r = Renderer { doc, env: PaintEnv { doc }, cull: None };
    let ts = sk::Transform::from_scale(scale, scale);
    effects::set_grid_origin(&ts);
    if let NodeKind::Page { background } = &n.kind {
        pm.fill(paint::sk_color(background, 1.0));
        r.draw_children(&n.children, ts, &mut pm, None, None);
    } else {
        r.draw_node(n, ts, &mut pm, None, None);
    }
    Some(pm)
}

// ---------------------------------------------------------------------------

struct Renderer<'a> {
    doc: &'a Document,
    env: PaintEnv<'a>,
    /// Skips children whose cached page-space bounds miss the drawn window.
    cull: Option<Cull<'a>>,
}

struct Cull<'a> {
    index: &'a SceneIndex,
    /// Visible window in page space (x0, y0, x1, y1).
    win: index::B,
}

/// What is visible underneath the layer being drawn: `None` means the draw
/// target itself is the backdrop.
type Back<'a> = Option<&'a sk::Pixmap>;

/// Composite of the parent backdrop (if any) and everything already in `target`.
fn composite_backdrop(back: Back, target: &sk::Pixmap) -> sk::Pixmap {
    let mut out = match back {
        Some(b) => b.clone(),
        None => return target.clone(),
    };
    out.draw_pixmap(0, 0, target.as_ref(), &sk::PixmapPaint::default(), sk::Transform::identity(), None);
    out
}

fn ts_scale(ts: &sk::Transform) -> f32 {
    (ts.sx * ts.sy - ts.kx * ts.ky).abs().sqrt().max(1e-6)
}

/// Mask covering `paths` (union) transformed by `ts`.
fn shape_mask(w: u32, h: u32, paths: &[(sk::Path, sk::FillRule)], ts: sk::Transform) -> Option<sk::Mask> {
    if paths.len() == 1 {
        let mut m = sk::Mask::new(w, h)?;
        m.fill_path(&paths[0].0, paths[0].1, true, ts);
        return Some(m);
    }
    let mut pm = sk::Pixmap::new(w, h)?;
    let mut p = sk::Paint::default();
    p.set_color(sk::Color::BLACK);
    for (path, rule) in paths {
        pm.fill_path(path, &p, *rule, ts, None);
    }
    Some(sk::Mask::from_pixmap(pm.as_ref(), sk::MaskType::Alpha))
}

fn mask_mul(m: &mut sk::Mask, other: &sk::Mask) {
    for (a, b) in m.data_mut().iter_mut().zip(other.data()) {
        *a = ((*a as u32 * *b as u32 + 127) / 255) as u8;
    }
}

fn black_paint(alpha: f32) -> sk::Paint<'static> {
    let mut p = sk::Paint { anti_alias: true, ..Default::default() };
    p.set_color(sk::Color::from_rgba(0.0, 0.0, 0.0, alpha.clamp(0.0, 1.0)).unwrap_or(sk::Color::BLACK));
    p
}

impl Renderer<'_> {
    // ----- tree -----------------------------------------------------------

    fn draw_children(&self, children: &[NodeId], parent_ts: sk::Transform, target: &mut sk::Pixmap, clip: Option<&sk::Mask>, back: Back) {
        for (i, cid) in children.iter().enumerate() {
            let Some(child) = board::get(self.doc, *cid) else { continue };
            // Variable bindings (and the mode in effect) are applied per node at draw time.
            let child = &*resolve_node(self.doc, &child);
            if let Some(c) = &self.cull {
                if !child.is_mask && !c.index.may_touch(*cid, c.win) {
                    continue;
                }
            }
            let ts = parent_ts.pre_concat(to_sk_transform(&child.transform));
            if child.is_mask {
                if !child.visible {
                    continue;
                }
                // Mask applies to this node's following siblings.
                let (w, h) = (target.width(), target.height());
                let (Some(mut layer), Some(mut mask_layer)) = (sk::Pixmap::new(w, h), sk::Pixmap::new(w, h)) else {
                    return;
                };
                let needs_back = children[i + 1..].iter().any(|c| self.doc.get(*c).is_some_and(|c| self.has_bg_blur(c)));
                let backdrop = needs_back.then(|| composite_backdrop(back, target));
                self.draw_children(&children[i + 1..], parent_ts, &mut layer, clip, backdrop.as_ref());
                // Masks use the shape's alpha regardless of its paints.
                let mask_node = mask::mask_stencil(child);
                self.draw_node(&mask_node, ts, &mut mask_layer, None, None);
                let pp = sk::PixmapPaint { blend_mode: sk::BlendMode::DestinationIn, ..Default::default() };
                layer.draw_pixmap(0, 0, mask_layer.as_ref(), &pp, sk::Transform::identity(), None);
                target.draw_pixmap(0, 0, layer.as_ref(), &sk::PixmapPaint::default(), sk::Transform::identity(), None);
                return;
            }
            self.draw_node(child, ts, target, clip, back);
        }
    }

    fn visible_fills<'n>(&self, n: &'n Node) -> Vec<&'n designos_doc::Paint> {
        match &n.kind {
            NodeKind::Text(t) => t.style.fills.iter().filter(|p| p.visible).collect(),
            _ => n.fills.iter().filter(|p| p.visible).collect(),
        }
    }

    fn visible_strokes<'n>(&self, n: &'n Node) -> Vec<&'n designos_doc::Paint> {
        if n.stroke.weight <= 0.0 && n.stroke.side_weights.is_none() {
            return Vec::new();
        }
        n.stroke.paints.iter().filter(|p| p.visible).collect()
    }

    /// Whether `n` or any drawn descendant has a visible background blur.
    fn has_bg_blur(&self, n: &Node) -> bool {
        if !n.visible {
            return false;
        }
        (!matches!(n.kind, NodeKind::Unknown { .. })
            && n.effects
                .iter()
                .any(|e| matches!(e, Effect::BackgroundBlur { visible: true, radius } if *radius > 0.0) || matches!(e, Effect::Glass(g) if g.visible)))
            || (Self::draws_children(n) && n.children.iter().any(|c| self.doc.get(*c).is_some_and(|c| self.has_bg_blur(c))))
    }

    fn draws_children(n: &Node) -> bool {
        !n.children.is_empty() && !matches!(n.kind, NodeKind::BooleanOp(_))
    }

    /// Extra extent (local units) around the node bounds touched by strokes/effects.
    fn margin(&self, n: &Node) -> f32 {
        index::node_margin(n)
    }

    fn bbox(&self, n: &Node, ts: &sk::Transform) -> effects::BBox {
        let m = self.margin(n);
        transformed_bounds(ts, -m, -m, n.size.x + 2.0 * m, n.size.y + 2.0 * m)
    }

    fn draw_node(&self, n: &Node, ts: sk::Transform, target: &mut sk::Pixmap, clip: Option<&sk::Mask>, back: Back) {
        if !n.visible || matches!(n.kind, NodeKind::Page { .. }) {
            return;
        }
        if matches!(n.kind, NodeKind::Unknown { .. }) {
            if !n.children.is_empty() {
                self.draw_children(&n.children, ts, target, clip, back);
            }
            return;
        }
        if n.kind.is_board_item() {
            // stickies, connectors, shapes with text and stamps draw as their lowered form
            if let Some(body) = board::lower(self.doc, n) {
                self.draw_node(&body, ts, target, clip, back);
            }
            return;
        }
        let clips = n.kind.frame_props().map(|f| f.clips_content).unwrap_or(false);
        let bb = self.bbox(n, &ts);
        let has_kids = Self::draws_children(n);
        if (!has_kids || clips) && (bb.2 < 0.0 || bb.3 < 0.0 || bb.0 > target.width() as f32 || bb.1 > target.height() as f32) {
            return;
        }
        let paints = self.visible_fills(n).len() + self.visible_strokes(n).len();
        let has_effects = n.effects.iter().any(|e| {
            matches!(e, Effect::DropShadow { visible: true, .. } | Effect::InnerShadow { visible: true, .. } | Effect::BackgroundBlur { visible: true, .. })
                || matches!(e, Effect::Glass(g) if g.visible)
                || matches!(e, Effect::Noise(n) if n.visible)
        });
        // Texture warps the node's own pixels, so it needs them in an isolated layer.
        let textured = n.effects.iter().any(|e| matches!(e, Effect::Texture(t) if t.visible && t.radius > 0.0));
        let layer_blur = n
            .effects
            .iter()
            .filter_map(|e| match e {
                Effect::LayerBlur { radius, visible: true } if *radius > 0.0 => Some(*radius),
                _ => None,
            })
            .fold(0.0f32, f32::max);
        let special_blend = !matches!(n.blend, designos_doc::BlendMode::Normal | designos_doc::BlendMode::PassThrough);
        let opacity = n.opacity.clamp(0.0, 1.0);
        if opacity <= 0.0 && !special_blend {
            return;
        }
        let need_layer = special_blend || layer_blur > 0.0 || textured || (opacity < 1.0 && (has_kids || paints > 1 || has_effects));
        if !need_layer {
            self.draw_body(n, ts, target, clip, opacity, back);
            return;
        }
        let Some(mut layer) = sk::Pixmap::new(target.width(), target.height()) else { return };
        // background blur inside an isolated layer samples what is already
        // painted underneath: the parent backdrop plus the current target
        let backdrop = self.has_bg_blur(n).then(|| composite_backdrop(back, target));
        self.draw_body(n, ts, &mut layer, clip, 1.0, backdrop.as_ref());
        if layer_blur > 0.0 {
            let sigma = layer_blur * ts_scale(&ts) / 2.0;
            let full = (0.0, 0.0, layer.width() as f32, layer.height() as f32);
            effects::blur_rect(&mut layer, if has_kids && !clips { full } else { bb }, sigma);
        }
        if let Some(mode) = paint::custom_blend(n.blend) {
            paint::blend_layer(target, &layer, opacity, mode);
            return;
        }
        let pp = sk::PixmapPaint { opacity, blend_mode: blend_mode(n.blend), ..Default::default() };
        target.draw_pixmap(0, 0, layer.as_ref(), &pp, sk::Transform::identity(), None);
    }

    // ----- node body ------------------------------------------------------

    /// Device rectangle, corner radius and fill alpha when the node's drop
    /// silhouette is a plain axis-aligned rounded rectangle with a single solid
    /// fill (the case the analytic shadow path handles).
    fn solid_rrect(&self, n: &Node, ts: &sk::Transform, shape: Option<&Shape>) -> Option<(shadow::RRect, f32)> {
        let sh = shape?;
        if !sh.is_rect || !sh.fillable || n.fill_geometry.is_some() || !self.visible_strokes(n).is_empty() {
            return None;
        }
        let fills = self.visible_fills(n);
        let [fill] = fills[..] else { return None };
        let designos_doc::PaintKind::Solid(c) = &fill.kind else { return None };
        let radii = if matches!(n.kind, NodeKind::Section) { [0.0; 4] } else { n.corner_radii };
        if radii.iter().any(|r| (r - radii[0]).abs() > 1e-4) || (radii[0] > 0.0 && n.corner_smoothing > 0.0) {
            return None;
        }
        let (sx, sy) = (ts.sx, ts.sy);
        if ts.kx.abs() > 1e-5 || ts.ky.abs() > 1e-5 || sx.is_nan() || sx <= 0.0 || sy.is_nan() || sy <= 0.0 || (sx - sy).abs() > 1e-4 * sx {
            return None;
        }
        let r = radii[0].max(0.0).min(n.size.x.min(n.size.y) / 2.0);
        Some((shadow::RRect { x: ts.tx, y: ts.ty, w: n.size.x * sx, h: n.size.y * sy, r: r * sx }, (c.a * fill.opacity).clamp(0.0, 1.0)))
    }

    fn draw_body(&self, n: &Node, ts: sk::Transform, target: &mut sk::Pixmap, clip: Option<&sk::Mask>, alpha: f32, back: Back) {
        self.draw_body_inner(n, ts, target, clip, alpha, back);
        for e in &n.effects {
            let Effect::Texture(t) = e else { continue };
            if !t.visible || t.radius <= 0.0 {
                continue;
            }
            let scale = ts_scale(&ts);
            let shape = node_shape(self.doc, n, 0.15 / scale);
            let text_layout = match &n.kind {
                NodeKind::Text(tp) => Some(text::layout(tp, n.size)),
                _ => None,
            };
            let sil = |pm: &mut sk::Pixmap, dx: f32, dy: f32, grow: f32, opaque: bool| {
                self.draw_silhouette(n, ts, shape.as_ref(), text_layout.as_deref(), pm, dx, dy, grow, opaque);
            };
            fx::texture(target, self.bbox(n, &ts), scale, t, &sil);
        }
    }

    fn draw_body_inner(&self, n: &Node, ts: sk::Transform, target: &mut sk::Pixmap, clip: Option<&sk::Mask>, alpha: f32, back: Back) {
        let scale = ts_scale(&ts);
        let shape = node_shape(self.doc, n, 0.15 / scale);
        let text_layout = match &n.kind {
            NodeKind::Text(t) => Some(text::layout(t, n.size)),
            _ => None,
        };
        let bb = self.bbox(n, &ts);
        let inner_bb = transformed_bounds(&ts, 0.0, 0.0, n.size.x, n.size.y);
        let sil = |pm: &mut sk::Pixmap, dx: f32, dy: f32, grow: f32, opaque: bool| {
            self.draw_silhouette(n, ts, shape.as_ref(), text_layout.as_deref(), pm, dx, dy, grow, opaque);
        };

        // 1. drop shadows + background blur (behind the node)
        let fast_rrect = if blur::reference_mode() { None } else { self.solid_rrect(n, &ts, shape.as_ref()) };
        for e in &n.effects {
            match e {
                Effect::DropShadow { color, offset, radius, spread, visible: true, show_behind } => {
                    if let Some((rr, fill_alpha)) = fast_rrect {
                        if shadow::drop_shadow_rrect(target, clip, rr, scale, color, *offset, *radius, *spread, *show_behind, alpha, fill_alpha, &sil) {
                            continue;
                        }
                    }
                    effects::drop_shadow(target, clip, bb, scale, color, *offset, *radius, *spread, *show_behind, alpha, &sil)
                }
                Effect::BackgroundBlur { radius, visible: true } if *radius > 0.0 => {
                    effects::background_blur(target, back, clip, inner_bb, scale, *radius, &sil)
                }
                Effect::Glass(g) if g.visible => fx::glass(target, back, clip, inner_bb, scale, g, &sil),
                _ => {}
            }
        }

        // 2. fills
        if let Some(tl) = &text_layout {
            self.draw_text(n, tl, ts, target, clip, alpha);
        } else if let Some(sh) = &shape {
            if sh.fillable {
                for p in self.visible_fills(n) {
                    fill_paths(&self.env, target, sh.fill_paths(), ts, n.size, p, alpha, clip);
                }
            }
        }

        // 3. inner shadows
        for e in &n.effects {
            if let Effect::InnerShadow { color, offset, radius, spread, visible: true } = e {
                effects::inner_shadow(target, clip, bb, scale, color, *offset, *radius, *spread, alpha, &sil);
            }
        }

        // 3b. noise grain over the fill
        for e in &n.effects {
            if let Effect::Noise(nz) = e {
                if nz.visible {
                    fx::noise(target, clip, bb, scale, nz, alpha, &sil);
                }
            }
        }

        // 4. strokes
        if let Some(sh) = &shape {
            self.draw_strokes(n, sh, ts, target, clip, alpha);
        }

        // 5. children
        if Self::draws_children(n) {
            let kids = n.paint_children();
            let clips = n.kind.frame_props().map(|f| f.clips_content).unwrap_or(false);
            if clips {
                let (w, h) = (target.width(), target.height());
                let Some(sh) = &shape else { return };
                let Some(mut m) = shape_mask(w, h, &sh.paths, ts) else { return };
                if let Some(c) = clip {
                    mask_mul(&mut m, c);
                }
                self.draw_children(&kids, ts, target, Some(&m), back);
            } else {
                self.draw_children(&kids, ts, target, clip, back);
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_silhouette(
        &self,
        n: &Node,
        ts: sk::Transform,
        shape: Option<&Shape>,
        tl: Option<&text::TextLayout>,
        pm: &mut sk::Pixmap,
        dx: f32,
        dy: f32,
        grow: f32,
        opaque: bool,
    ) {
        let ts2 = ts.post_translate(dx, dy);
        let fills = self.visible_fills(n);
        let strokes = self.visible_strokes(n);
        if let Some(tl) = tl {
            for (i, path) in tl.paths.iter().enumerate() {
                if let Some(path) = path {
                    let a = if opaque {
                        1.0
                    } else {
                        tl.styles[i].fills.iter().filter(|p| p.visible).map(designos_doc::Paint::silhouette_alpha).fold(0.0, f32::max)
                    };
                    pm.fill_path(path, &black_paint(a), sk::FillRule::Winding, ts2, None);
                }
            }
            return;
        }
        if let Some(sh) = shape {
            if (sh.fillable && !fills.is_empty()) || !strokes.is_empty() || (opaque && sh.fillable) {
                if sh.fillable && opaque && fills.is_empty() {
                    for (path, rule) in sh.fill_paths() {
                        pm.fill_path(path, &black_paint(1.0), *rule, ts2, None);
                    }
                } else if sh.fillable {
                    for p in &fills {
                        let a = if opaque { 1.0 } else { p.silhouette_alpha() };
                        let paint = black_paint(a);
                        for (path, rule) in sh.fill_paths() {
                            pm.fill_path(path, &paint, *rule, ts2, None);
                        }
                    }
                }
                // stroke footprint outside the fill
                let outset = match n.stroke.align {
                    designos_doc::StrokeAlign::Inside if sh.closed && sh.fillable && !fills.is_empty() => 0.0,
                    designos_doc::StrokeAlign::Outside if sh.closed => n.stroke.weight * 2.0,
                    _ => n.stroke.weight,
                };
                if let (Some(sp), false) = (&sh.stroke_paths, strokes.is_empty()) {
                    for (path, rule) in sp {
                        pm.fill_path(path, &black_paint(1.0), *rule, ts2, None);
                    }
                } else if !strokes.is_empty() && outset > 0.0 {
                    let mut st = sk::Stroke { width: outset, line_join: sk::LineJoin::Miter, ..Default::default() };
                    st.miter_limit = 4.0;
                    for (path, _) in &sh.paths {
                        pm.stroke_path(path, &black_paint(1.0), &st, ts2, None);
                    }
                }
                if grow != 0.0 {
                    let st = sk::Stroke { width: grow.abs() * 2.0, line_join: sk::LineJoin::Miter, miter_limit: 4.0, ..Default::default() };
                    let mut p = black_paint(1.0);
                    if grow < 0.0 {
                        p.blend_mode = sk::BlendMode::Clear;
                    }
                    for (path, _) in &sh.paths {
                        pm.stroke_path(path, &p, &st, ts2, None);
                    }
                }
                return;
            }
        }
        if Self::draws_children(n) {
            let kids = n.paint_children();
            // silhouettes are drawn offset from the page grid, so no culling
            Renderer { doc: self.doc, env: PaintEnv { doc: self.doc }, cull: None }.draw_children(&kids, ts2, pm, None, None);
        }
    }

    fn draw_text(&self, n: &Node, tl: &text::TextLayout, ts: sk::Transform, target: &mut sk::Pixmap, clip: Option<&sk::Mask>, alpha: f32) {
        for (i, path) in tl.paths.iter().enumerate() {
            let Some(path) = path else { continue };
            let paths = [(path.clone(), sk::FillRule::Winding)];
            for p in tl.styles[i].fills.iter().filter(|p| p.visible) {
                fill_paths(&self.env, target, &paths, ts, n.size, p, alpha, clip);
            }
            let strokes = self.visible_strokes(n);
            if !strokes.is_empty() {
                let st = self.make_stroke(n, n.stroke.weight);
                for p in strokes {
                    stroke_paths(&self.env, target, &paths, &st, ts, n.size, p, alpha, clip);
                }
            }
        }
    }

    fn make_stroke(&self, n: &Node, width: f32) -> sk::Stroke {
        let s = &n.stroke;
        // per-vertex caps are drawn as decorations over a butt-capped stroke
        let cap = if vectorops::vertex_caps(n).is_some() { designos_doc::StrokeCap::None } else { s.cap };
        let mut st = sk::Stroke {
            width,
            miter_limit: s.miter_limit.max(1.0),
            line_cap: match cap {
                designos_doc::StrokeCap::None => sk::LineCap::Butt,
                designos_doc::StrokeCap::Round => sk::LineCap::Round,
                designos_doc::StrokeCap::Square => sk::LineCap::Square,
            },
            line_join: match s.join {
                designos_doc::StrokeJoin::Miter => sk::LineJoin::Miter,
                designos_doc::StrokeJoin::Bevel => sk::LineJoin::Bevel,
                designos_doc::StrokeJoin::Round => sk::LineJoin::Round,
            },
            dash: None,
        };
        if !s.dashes.is_empty() && s.dashes.iter().all(|d| *d >= 0.0) && s.dashes.iter().sum::<f32>() > 0.0 {
            let mut v = s.dashes.clone();
            if v.len() % 2 == 1 {
                v.extend_from_within(..);
            }
            st.dash = sk::StrokeDash::new(v, 0.0);
        }
        st
    }

    fn draw_strokes(&self, n: &Node, sh: &Shape, ts: sk::Transform, target: &mut sk::Pixmap, clip: Option<&sk::Mask>, alpha: f32) {
        let paints = self.visible_strokes(n);
        if paints.is_empty() {
            return;
        }
        let (tw, th) = (target.width(), target.height());
        let align = if sh.closed { n.stroke.align } else { designos_doc::StrokeAlign::Center };
        // precomputed outline: already expanded by weight and alignment
        if let Some(sp) = &sh.stroke_paths {
            let mask = if align == designos_doc::StrokeAlign::Outside {
                shape_mask(tw, th, sh.fill_paths(), ts).map(|mut m| {
                    m.invert();
                    if let Some(c) = clip {
                        mask_mul(&mut m, c);
                    }
                    m
                })
            } else {
                None
            };
            let cl = mask.as_ref().or(clip);
            for p in paints {
                fill_paths(&self.env, target, sp, ts, n.size, p, alpha, cl);
            }
            return;
        }

        // per-side weights on rectangles
        if sh.is_rect && n.stroke.side_weights.is_some() {
            if let Some(path) = outline::per_side_stroke_path(n, align) {
                let rounded = n.corner_radii.iter().any(|v| *v > 0.0);
                let paths = [(path, sk::FillRule::Winding)];
                let mask = if rounded && n.corner_smoothing > 0.0 && align == designos_doc::StrokeAlign::Inside {
                    shape_mask(tw, th, sh.fill_paths(), ts).map(|mut m| {
                        if let Some(c) = clip {
                            mask_mul(&mut m, c);
                        }
                        m
                    })
                } else {
                    None
                };
                let cl = mask.as_ref().or(clip);
                for p in paints {
                    fill_paths(&self.env, target, &paths, ts, n.size, p, alpha, cl);
                }
            }
            return;
        }

        let w = n.stroke.weight;
        if w <= 0.0 {
            return;
        }
        if let Some(caps) = vectorops::vertex_caps(n) {
            let caps = geom::vector_paths(&caps).paths;
            for p in &paints {
                fill_paths(&self.env, target, &caps, ts, n.size, p, alpha, clip);
            }
        }
        match align {
            designos_doc::StrokeAlign::Center => {
                let st = self.make_stroke(n, w);
                for p in paints {
                    stroke_paths(&self.env, target, &sh.paths, &st, ts, n.size, p, alpha, clip);
                }
            }
            a => {
                let Some(mut m) = shape_mask(tw, th, sh.fill_paths(), ts) else { return };
                if a == designos_doc::StrokeAlign::Outside {
                    m.invert();
                }
                if let Some(c) = clip {
                    mask_mul(&mut m, c);
                }
                let st = self.make_stroke(n, w * 2.0);
                for p in paints {
                    stroke_paths(&self.env, target, &sh.paths, &st, ts, n.size, p, alpha, Some(&m));
                }
            }
        }
    }
}
