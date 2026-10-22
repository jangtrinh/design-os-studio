//! Canvas rendering through the renderer's tile cache: the page is painted as
//! 256 px tiles, each uploaded as its own texture only when its pixels
//! change, so panning reuses everything already drawn and an edit re-renders
//! just the tiles under the nodes it touched.
//!
//! Zoom gestures follow Figma: while the zoom keeps changing the tiles of the
//! last settled zoom are simply scaled by the GPU; once it has been still for
//! [`ZOOM_SETTLE`] seconds the page is re-rendered at the new scale (the old
//! tiles stay underneath until the new ones arrive).

use crate::app::App;
use designos_doc::NodeKind;
use designos_render::{Tile, TileBudget, TileCache, TileKey, Viewport, TILE_SIZE};
use egui::{Color32, Context, Rect as ERect, TextureHandle, TextureOptions, Ui};
use std::collections::HashMap;
use std::time::Duration;

/// Seconds without a zoom change before tiles are re-rendered at the new zoom.
pub const ZOOM_SETTLE: f64 = 0.15;

/// Time one frame may spend rendering tiles (a started batch always finishes).
const RENDER_BUDGET: Duration = Duration::from_millis(6);

struct TexEntry {
    handle: TextureHandle,
    version: u64,
    /// Sampled with linear filtering (tile is being scaled).
    linear: bool,
    seen: u64,
}

#[derive(Default)]
pub struct TileView {
    pub(crate) cache: TileCache,
    /// Document revision handed to the cache; bumped when `dirty` is consumed.
    rev: u64,
    textures: HashMap<TileKey, TexEntry>,
    frame_no: u64,
    /// Zoom the tiles are currently rendered at (lags `view.zoom` mid-gesture).
    settled_zoom: f32,
    last_zoom: f32,
    zoom_changed_at: f64,
    /// The document was replaced: the GPU geometry cache must be dropped.
    pub gpu_reset: bool,
    /// When true (during `--screenshot` / sequence capture), render all visible tiles synchronously at settled zoom.
    pub unlimited_budget: bool,
}

impl TileView {
    /// Document revision, bumped whenever the document changed (autosave watches it).
    pub fn rev(&self) -> u64 {
        self.rev
    }

    /// Cached spatial index of the current page's world bounds.
    pub fn scene_index(&self) -> &designos_render::SceneIndex {
        self.cache.index()
    }

    /// Re-renders everything (stale tiles stay visible until replaced).
    pub fn invalidate_all(&mut self) {
        self.cache.invalidate_all();
        self.gpu_reset = true;
    }
}

fn page_background(app: &App) -> Color32 {
    match app.doc.get(app.page).map(|n| &n.kind) {
        Some(NodeKind::Page { background }) => {
            let f = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
            Color32::from_rgb(f(background.r), f(background.g), f(background.b))
        }
        _ => Color32::from_rgb(0x5A, 0x5A, 0x5A),
    }
}

fn upload(ctx: &Context, tv: &mut TileView, tile: &Tile, linear: bool) -> egui::TextureId {
    let opts = if linear { TextureOptions::LINEAR } else { TextureOptions::NEAREST };
    let side = TILE_SIZE as usize;
    let frame_no = tv.frame_no;
    if let Some(e) = tv.textures.get_mut(&tile.key) {
        e.seen = frame_no;
        if e.version != tile.version || e.linear != linear {
            let img = egui::ColorImage::from_rgba_premultiplied([side, side], tile.pixmap.data());
            e.handle.set(img, opts);
            e.version = tile.version;
            e.linear = linear;
        }
        return e.handle.id();
    }
    let img = egui::ColorImage::from_rgba_premultiplied([side, side], tile.pixmap.data());
    let name = format!("tile {} {} {}", tile.key.scale_bits, tile.key.x, tile.key.y);
    let handle = ctx.load_texture(name, img, opts);
    let id = handle.id();
    tv.textures.insert(tile.key, TexEntry { handle, version: tile.version, linear, seen: frame_no });
    id
}

/// Document revision for this frame: bumped once when the document changed.
pub fn take_rev(app: &mut App) -> u64 {
    let c = &mut app.canvas;
    if c.dirty {
        c.dirty = false;
        c.tiles.rev += 1;
    }
    if std::mem::take(&mut c.tiles.gpu_reset) {
        c.gpu.reset();
    }
    c.tiles.rev
}

/// Renders (as far as the budget allows) and paints the page into `rect`.
pub fn paint(app: &mut App, ui: &Ui, rect: ERect) {
    if super::gpu::paint(app, ui, rect) {
        return;
    }
    let ctx = ui.ctx();
    let ppp = ctx.pixels_per_point();
    let (w, h) = ((rect.width() * ppp).round().max(1.0), (rect.height() * ppp).round().max(1.0));
    let start = std::time::Instant::now();
    let bg = page_background(app);
    let painter = ui.painter().with_clip_rect(rect);
    if !app.is_board() {
        painter.rect_filled(rect, 0.0, bg);
    }

    let (page, offset, zoom) = (app.page, app.view.offset, app.view.zoom);
    let now = ctx.input(|i| i.time);
    take_rev(app);
    let runtime_opacities =
        if app.canvas.tiles.unlimited_budget { Vec::new() } else { app.agent_presence.active_top_level_insert_opacities(&app.doc, page, app.now) };
    let c = &mut app.canvas;
    let tv = &mut c.tiles;
    tv.frame_no += 1;

    // Zoom gesture handling: keep rendering at the settled zoom until idle.
    if tv.unlimited_budget || tv.settled_zoom <= 0.0 {
        tv.settled_zoom = zoom;
        tv.last_zoom = zoom;
    }
    if zoom != tv.last_zoom {
        tv.last_zoom = zoom;
        tv.zoom_changed_at = now;
    }
    if zoom != tv.settled_zoom {
        let idle = now - tv.zoom_changed_at;
        if idle >= ZOOM_SETTLE {
            tv.settled_zoom = zoom;
        } else {
            ctx.request_repaint_after(Duration::from_secs_f64(ZOOM_SETTLE - idle + 0.005));
        }
    }
    let rz = tv.settled_zoom;
    // device size of the visible page area when drawn at the settled zoom
    let ratio = rz / zoom;
    let vp = Viewport { offset, zoom: rz, dpr: ppp };
    let size = ((w * ratio).ceil().max(1.0) as u32, (h * ratio).ceil().max(1.0) as u32);
    let budget = if tv.unlimited_budget { TileBudget::unlimited() } else { TileBudget::time(RENDER_BUDGET) };
    tv.cache.set_runtime_opacities(&app.doc, page, runtime_opacities);
    let frame = tv.cache.render_tiles(&app.doc, page, &vp, size, tv.rev, budget);
    let pending = frame.pending() > 0 || frame.prefetch_pending > 0;

    // Tiles are laid out in device pixels of the actual zoom, snapped so
    // neighbours abut exactly (and match the tile grid 1:1 when not scaling).
    let a = zoom * ppp;
    let to_pt = |dev: f32| dev.round() / ppp;
    for t in frame.fallback.iter().chain(frame.tiles.iter()) {
        let s = t.key.scale();
        let (ox, oy) = t.key.device_origin();
        let side = TILE_SIZE as f32;
        let x0 = (ox as f32 / s - offset.x) * a;
        let y0 = (oy as f32 / s - offset.y) * a;
        let x1 = ((ox as f32 + side) / s - offset.x) * a;
        let y1 = ((oy as f32 + side) / s - offset.y) * a;
        let r = ERect::from_min_max(egui::pos2(rect.min.x + to_pt(x0), rect.min.y + to_pt(y0)), egui::pos2(rect.min.x + to_pt(x1), rect.min.y + to_pt(y1)));
        if !r.intersects(rect) {
            continue;
        }
        let linear = (s - a).abs() > 1e-4 * a;
        let id = upload(ctx, tv, t, linear);
        painter.image(id, r, ERect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), Color32::WHITE);
    }

    if pending {
        ctx.request_repaint();
    }
    // drop textures of evicted tiles now and then
    if tv.frame_no.is_multiple_of(60) {
        let cache = &tv.cache;
        tv.textures.retain(|k, _| cache.contains(k));
    }
    c.last_render_ms = start.elapsed().as_secs_f32() * 1000.0;
}

/// What the window shows for the page, checked against the document (tests only). Empty when the picture is
/// right: no change waits for a paint, every visible tile is rendered from the current document (not stale,
/// not missing), has the pixels of a render from scratch, and is the version uploaded as its texture.
#[cfg(test)]
pub(crate) fn picture_errors(app: &mut App, ppp: f32) -> Vec<String> {
    let mut errors = Vec::new();
    if app.canvas.dirty {
        errors.push("the document changed after the last paint and nothing has painted it".to_string());
    }
    let (page, offset, view_zoom, rect) = (app.page, app.view.offset, app.view.zoom, app.canvas_rect);
    let tv = &mut app.canvas.tiles;
    let zoom = tv.settled_zoom;
    let ratio = zoom / view_zoom;
    let side = |len: f32| ((len * ppp).round().max(1.0) * ratio).ceil().max(1.0) as u32;
    let size = (side(rect.width()), side(rect.height()));
    let vp = Viewport { offset, zoom, dpr: ppp };
    // Zero tiles may render: the cache hands out what it holds, flagged stale where the document moved on.
    let shown = tv.cache.render_tiles(&app.doc, page, &vp, size, tv.rev, TileBudget::tiles(0));
    if shown.stale > 0 || shown.missing > 0 {
        errors.push(format!("{} stale and {} missing visible tiles", shown.stale, shown.missing));
    }
    let mut fresh = TileCache::new();
    fresh.prefetch_ring = 0;
    let want = fresh.render_tiles(&app.doc, page, &vp, size, 1, TileBudget::unlimited());
    for w in &want.tiles {
        let Some(s) = shown.tiles.iter().find(|t| t.key == w.key) else { continue };
        if s.pixmap.data() != w.pixmap.data() {
            errors.push(format!("tile {:?} does not show the document", s.key));
        }
        if tv.textures.get(&s.key).map(|e| e.version) != Some(s.version) {
            errors.push(format!("tile {:?} was not uploaded at version {}", s.key, s.version));
        }
    }
    errors
}
