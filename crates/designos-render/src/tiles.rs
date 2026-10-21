//! Tiled rendering: the page is painted as 256x256 device-pixel tiles keyed by
//! (scale, tile x, tile y), re-rendered in parallel only where the document
//! changed. A tile's pixels equal the same region of a full [`crate::render_page`].
//!
//! Tile space is "device pixels at a fixed scale": tile (i, j) at scale `s`
//! covers device pixels [i*256, (i+1)*256) x [j*256, (j+1)*256) of the image
//! `page * s`, independent of any viewport offset, so panning reuses tiles.
//! `s` is `zoom * dpr` of the viewport that requested them.

use crate::index::{self, Differ, B};
use crate::{render_with_transform_and_opacities, SceneIndex, Viewport};
use designos_doc::{Document, NodeId, Rect};
use rayon::prelude::*;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tiny_skia as sk;

/// Edge length of a tile in device pixels.
pub const TILE_SIZE: u32 = 256;

/// Minimum padding (device px) rendered around every tile and then cropped.
const EDGE_PAD: i32 = 2;

/// Worker pool for tile rendering. Half the cores (2..=8) so the UI thread
/// and the OS keep headroom; override with `DESIGNOS_TILE_THREADS`. Workers are
/// long lived, so their thread-local font systems and text layout caches
/// persist across frames.
fn pool() -> &'static rayon::ThreadPool {
    static POOL: std::sync::OnceLock<rayon::ThreadPool> = std::sync::OnceLock::new();
    POOL.get_or_init(|| {
        let cores = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
        let n = std::env::var("DESIGNOS_TILE_THREADS").ok().and_then(|v| v.parse().ok()).unwrap_or((cores / 2).clamp(2, 8));
        rayon::ThreadPoolBuilder::new().num_threads(n).thread_name(|i| format!("designos-tile-{i}")).build().expect("tile thread pool")
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TileKey {
    /// `f32::to_bits` of `zoom * dpr`.
    pub scale_bits: u32,
    pub x: i32,
    pub y: i32,
}

impl TileKey {
    pub fn scale(&self) -> f32 {
        f32::from_bits(self.scale_bits)
    }
    /// Top-left of the tile in device pixels of the `page * scale` image.
    pub fn device_origin(&self) -> (i32, i32) {
        (self.x * TILE_SIZE as i32, self.y * TILE_SIZE as i32)
    }
}

/// One tile ready to be drawn.
#[derive(Clone)]
pub struct Tile {
    pub key: TileKey,
    /// 256x256 premultiplied RGBA.
    pub pixmap: Arc<sk::Pixmap>,
    /// Changes whenever `pixmap` does; upload to the GPU only when it differs.
    pub version: u64,
    /// The document changed under this tile and a fresh render is pending.
    pub stale: bool,
}

/// Visible tiles for one frame.
#[derive(Default)]
pub struct TileFrame {
    /// `zoom * dpr` the tiles were rendered at.
    pub scale: f32,
    /// Tiles intersecting the viewport (stale ones keep their previous image).
    pub tiles: Vec<Tile>,
    /// Tiles of another scale covering the viewport, to draw (scaled) beneath
    /// `tiles` while some of them are still missing.
    pub fallback: Vec<Tile>,
    /// Visible tiles still showing out-of-date pixels.
    pub stale: usize,
    /// Visible tiles with no pixels at all yet.
    pub missing: usize,
    /// Tiles rendered by this call.
    pub rendered: usize,
    /// Off-screen tiles around the viewport still waiting to be prefetched.
    pub prefetch_pending: usize,
}

impl TileFrame {
    /// Tiles that still need rendering; the caller should repaint soon if non-zero.
    pub fn pending(&self) -> usize {
        self.stale + self.missing
    }
}

/// Limits how much rendering one `render_tiles` call may do. Work proceeds in
/// parallel batches and stops after the first batch that exceeds the budget.
#[derive(Clone, Copy, Debug)]
pub struct TileBudget {
    pub max_time: Option<Duration>,
    pub max_tiles: usize,
}

impl TileBudget {
    pub fn time(d: Duration) -> Self {
        Self { max_time: Some(d), max_tiles: usize::MAX }
    }
    pub fn tiles(n: usize) -> Self {
        Self { max_time: None, max_tiles: n }
    }
    pub fn unlimited() -> Self {
        Self { max_time: None, max_tiles: usize::MAX }
    }
}

struct Slot {
    pixmap: Option<Arc<sk::Pixmap>>,
    version: u64,
    dirty: bool,
    last_used: u64,
}

pub struct TileCache {
    slots: HashMap<TileKey, Slot>,
    index: SceneIndex,
    differ: Differ,
    page: Option<NodeId>,
    rev: Option<u64>,
    fonts: usize,
    next_version: u64,
    frame: u64,
    cur_scale: Option<u32>,
    prev_scale: Option<u32>,
    runtime_opacities: Vec<(NodeId, f32)>,
    /// Extra ring of tiles around the viewport rendered when time allows.
    pub prefetch_ring: i32,
    /// Soft cap on cached tiles (256 KiB each).
    pub max_tiles: usize,
}

impl Default for TileCache {
    fn default() -> Self {
        Self::new()
    }
}

impl TileCache {
    pub fn new() -> Self {
        Self {
            slots: HashMap::new(),
            index: SceneIndex::new(),
            differ: Differ::default(),
            page: None,
            rev: None,
            fonts: crate::font_generation(),
            next_version: 1,
            frame: 0,
            cur_scale: None,
            prev_scale: None,
            runtime_opacities: Vec::new(),
            prefetch_ring: 1,
            max_tiles: 600,
        }
    }

    /// Updates runtime-only per-node opacity multipliers (e.g. top-level insert fade-in)
    /// without mutating `Document` or bumping `rev`, invalidating only the tiles overlapping
    /// nodes whose runtime opacity changed.
    pub fn set_runtime_opacities(&mut self, doc: &Document, page: NodeId, opacities: Vec<(NodeId, f32)>) {
        if self.runtime_opacities == opacities {
            return;
        }
        self.index.ensure(doc, page, self.rev.unwrap_or(0));
        let mut changed_nodes = Vec::new();
        for &(id, new_a) in &opacities {
            let old_a = self.runtime_opacities.iter().find(|&&(oid, _)| oid == id).map(|&(_, a)| a);
            if old_a.is_none_or(|oa| (oa - new_a).abs() > 1e-4) {
                changed_nodes.push(id);
            }
        }
        for &(id, _) in &self.runtime_opacities {
            if !opacities.iter().any(|&(nid, _)| nid == id) {
                changed_nodes.push(id);
            }
        }
        self.runtime_opacities = opacities;
        for id in changed_nodes {
            if let Some(b) = self.index.bounds_of(id) {
                self.invalidate_b(b);
            }
        }
    }

    /// Marks every cached tile stale (they keep their old pixels until redrawn).
    pub fn invalidate_all(&mut self) {
        for s in self.slots.values_mut() {
            s.dirty = true;
        }
    }

    /// Marks tiles overlapping `page_rect` (page space) stale. Pass the union
    /// of the old and new bounds of every changed node.
    pub fn invalidate_rect(&mut self, page_rect: Rect) {
        self.invalidate_b((page_rect.x, page_rect.y, page_rect.x + page_rect.w, page_rect.y + page_rect.h));
    }

    fn invalidate_b(&mut self, b: B) {
        if index::is_empty(b) {
            return;
        }
        let t = TILE_SIZE as f32;
        for (k, s) in self.slots.iter_mut() {
            if s.dirty {
                continue;
            }
            let sc = k.scale();
            let (ox, oy) = k.device_origin();
            // a one pixel pad covers antialiasing at tile borders
            let tb = ((ox as f32 - 1.0) / sc, (oy as f32 - 1.0) / sc, (ox as f32 + t + 1.0) / sc, (oy as f32 + t + 1.0) / sc);
            if index::overlaps(tb, b) {
                s.dirty = true;
            }
        }
    }

    /// Drops all tiles and cached analysis.
    pub fn clear(&mut self) {
        self.slots.clear();
        self.index.clear();
        self.differ.reset();
        self.rev = None;
        self.cur_scale = None;
        self.prev_scale = None;
        self.runtime_opacities.clear();
    }

    /// Whether a tile (rendered or pending) is still cached.
    pub fn contains(&self, key: &TileKey) -> bool {
        self.slots.contains_key(key)
    }

    /// Number of cached tiles (all scales).
    pub fn cached_tiles(&self) -> usize {
        self.slots.len()
    }

    /// The scene index used for tile culling (valid after `render_tiles`).
    pub fn index(&self) -> &SceneIndex {
        &self.index
    }

    fn sync(&mut self, doc: &Document, page: NodeId, rev: u64) {
        if self.page != Some(page) {
            self.page = Some(page);
            self.differ.reset();
            self.index.clear();
            self.rev = None;
            for s in self.slots.values_mut() {
                s.dirty = true;
            }
        }
        let fonts = crate::font_generation();
        if fonts != self.fonts {
            self.fonts = fonts;
            self.invalidate_all();
        }
        if self.rev == Some(rev) {
            return;
        }
        self.rev = Some(rev);
        let Some(old) = self.index.ensure_prev(doc, page, rev) else { return };
        let ch = self.differ.diff(doc, page, &old, &self.index);
        if ch.all {
            self.invalidate_all();
        } else {
            for r in ch.rects {
                self.invalidate_b(r);
            }
        }
    }

    /// Returns the tiles covering `vp` (a viewport `size` device pixels wide
    /// and high), rendering missing and stale ones in parallel within
    /// `budget`. `rev` is the document revision counter: bump it on every edit.
    /// Changed regions are detected by diffing the page against the previous
    /// revision, so no explicit invalidation is needed (`invalidate_rect` is
    /// available for changes the document does not show, e.g. image bytes).
    pub fn render_tiles(&mut self, doc: &Document, page: NodeId, vp: &Viewport, size: (u32, u32), rev: u64, budget: TileBudget) -> TileFrame {
        self.sync(doc, page, rev);
        self.frame += 1;
        let scale = (vp.zoom * vp.dpr).max(1e-4);
        let skey = scale.to_bits();
        if self.cur_scale != Some(skey) {
            self.prev_scale = self.cur_scale;
            self.cur_scale = Some(skey);
        }
        let t = TILE_SIZE as f32;
        let (x0, y0) = (vp.offset.x * scale, vp.offset.y * scale);
        let (x1, y1) = (x0 + size.0 as f32, y0 + size.1 as f32);
        let (tx0, ty0) = ((x0 / t).floor() as i32, (y0 / t).floor() as i32);
        let (tx1, ty1) = (((x1 - 1.0).max(x0) / t).floor() as i32, ((y1 - 1.0).max(y0) / t).floor() as i32);
        let center = ((tx0 + tx1) as f32 / 2.0, (ty0 + ty1) as f32 / 2.0);
        let key = |x: i32, y: i32| TileKey { scale_bits: skey, x, y };

        let mut visible: Vec<TileKey> = Vec::new();
        for y in ty0..=ty1 {
            for x in tx0..=tx1 {
                visible.push(key(x, y));
            }
        }
        let dist = |k: &TileKey| (k.x as f32 - center.0).powi(2) + (k.y as f32 - center.1).powi(2);
        visible.sort_by(|a, b| dist(a).partial_cmp(&dist(b)).unwrap());
        let mut todo: Vec<TileKey> = visible.iter().copied().filter(|k| self.needs_render(k)).collect();
        let n_visible_todo = todo.len();
        let ring = self.prefetch_ring;
        if ring > 0 {
            let mut extra = Vec::new();
            for y in (ty0 - ring)..=(ty1 + ring) {
                for x in (tx0 - ring)..=(tx1 + ring) {
                    if x < tx0 || x > tx1 || y < ty0 || y > ty1 {
                        let k = key(x, y);
                        if self.needs_render(&k) {
                            extra.push(k);
                        }
                    }
                }
            }
            extra.sort_by(|a, b| dist(a).partial_cmp(&dist(b)).unwrap());
            todo.extend(extra);
        }

        // Render in parallel batches until the budget is spent. Visible tiles
        // come first; prefetch tiles run in one small batch per call, and only
        // when the visible ones are done and time remains, so they never
        // add a frame-sized spike.
        let started = Instant::now();
        let chunk = pool().current_num_threads().max(1);
        let ring_chunk = (chunk / 4).max(1);
        let mut rendered = 0usize;
        while rendered < todo.len() {
            if rendered >= budget.max_tiles {
                break;
            }
            let in_ring = rendered >= n_visible_todo;
            if rendered > 0 {
                if let Some(limit) = budget.max_time {
                    if started.elapsed() >= limit {
                        break;
                    }
                    if in_ring && started.elapsed() * 2 >= limit {
                        break;
                    }
                }
                if in_ring && rendered > n_visible_todo {
                    break;
                }
            }
            let step = if in_ring { ring_chunk } else { chunk.min(n_visible_todo - rendered) };
            let end = (rendered + step).min(todo.len()).min(budget.max_tiles.max(rendered + 1));
            let batch = &todo[rendered..end];
            let index = &self.index;
            let runtime_opacities = &self.runtime_opacities;
            let done: Vec<(TileKey, sk::Pixmap)> = if batch.len() == 1 {
                vec![(batch[0], render_tile(doc, page, index, batch[0], runtime_opacities))]
            } else {
                pool().install(|| batch.par_iter().map(|k| (*k, render_tile(doc, page, index, *k, runtime_opacities))).collect())
            };
            for (k, pm) in done {
                let version = self.next_version;
                self.next_version += 1;
                let frame = self.frame;
                let slot = self.slots.entry(k).or_insert(Slot { pixmap: None, version: 0, dirty: false, last_used: frame });
                slot.pixmap = Some(Arc::new(pm));
                slot.version = version;
                slot.dirty = false;
            }
            rendered = end;
        }

        let mut frame = TileFrame { scale, rendered, prefetch_pending: todo.len().saturating_sub(rendered.max(n_visible_todo)), ..Default::default() };
        for k in &visible {
            match self.slots.get_mut(k) {
                Some(s) if s.pixmap.is_some() => {
                    s.last_used = self.frame;
                    if s.dirty {
                        frame.stale += 1;
                    }
                    frame.tiles.push(Tile { key: *k, pixmap: s.pixmap.clone().unwrap(), version: s.version, stale: s.dirty });
                }
                _ => frame.missing += 1,
            }
        }
        if frame.pending() > 0 {
            if let Some(ps) = self.prev_scale.filter(|p| *p != skey) {
                let psc = f32::from_bits(ps);
                let r = psc / scale;
                let (px0, py0, px1, py1) = (x0 * r, y0 * r, x1 * r, y1 * r);
                let (a0, b0) = ((px0 / t).floor() as i32, (py0 / t).floor() as i32);
                let (a1, b1) = (((px1 - 1.0).max(px0) / t).floor() as i32, ((py1 - 1.0).max(py0) / t).floor() as i32);
                for y in b0..=b1 {
                    for x in a0..=a1 {
                        let k = TileKey { scale_bits: ps, x, y };
                        if let Some(s) = self.slots.get_mut(&k) {
                            if let Some(pm) = &s.pixmap {
                                s.last_used = self.frame;
                                frame.fallback.push(Tile { key: k, pixmap: pm.clone(), version: s.version, stale: true });
                            }
                        }
                    }
                }
            }
        }
        self.evict(skey);
        frame
    }

    fn needs_render(&self, k: &TileKey) -> bool {
        match self.slots.get(k) {
            Some(s) => s.dirty || s.pixmap.is_none(),
            None => true,
        }
    }

    fn evict(&mut self, keep_scale: u32) {
        if self.slots.len() <= self.max_tiles {
            return;
        }
        let mut order: Vec<(u64, TileKey)> = self
            .slots
            .iter()
            .filter(|(k, s)| s.last_used + 2 < self.frame || (k.scale_bits != keep_scale && Some(k.scale_bits) != self.prev_scale))
            .map(|(k, s)| (s.last_used, *k))
            .collect();
        order.sort();
        let excess = self.slots.len() - self.max_tiles;
        for (_, k) in order.into_iter().take(excess) {
            self.slots.remove(&k);
        }
    }
}

/// Paints one tile. Pixels equal the matching region of a full-page render:
/// the tile is drawn with padding for effects that read neighbouring pixels
/// (blurs, shadows) and cropped.
fn render_tile(doc: &Document, page: NodeId, idx: &SceneIndex, k: TileKey, runtime_opacities: &[(NodeId, f32)]) -> sk::Pixmap {
    let scale = k.scale();
    let ts_i = TILE_SIZE as i32;
    let (cx, cy) = k.device_origin();
    // page-space area of the tile
    let tile_b: B = (cx as f32 / scale, cy as f32 / scale, (cx + ts_i) as f32 / scale, (cy + ts_i) as f32 / scale);
    let mut reach = 0.0f32;
    for (b, r) in &idx.blurry {
        if index::overlaps(*b, tile_b) {
            reach = reach.max(*r);
        }
    }
    // The rasteriser clips paths at the pixmap edge, which changes coverage of
    // the outermost pixels, so even plain tiles are drawn a few pixels wider.
    let pad = ((reach * scale).ceil() as i32 + EDGE_PAD).clamp(EDGE_PAD, ts_i);
    let size = (ts_i + 2 * pad) as u32;
    let (ox, oy) = (cx - pad, cy - pad);
    let ts = sk::Transform::from_row(scale, 0.0, 0.0, scale, -(ox as f32), -(oy as f32));
    let mut pm = sk::Pixmap::new(size, size).expect("tile size");
    render_with_transform_and_opacities(doc, page, ts, &mut pm, Some(idx), runtime_opacities);
    let mut out = sk::Pixmap::new(TILE_SIZE, TILE_SIZE).expect("tile size");
    let (src, dst) = (pm.data(), out.data_mut());
    let row = TILE_SIZE as usize * 4;
    for y in 0..TILE_SIZE as usize {
        let s = ((y + pad as usize) * size as usize + pad as usize) * 4;
        dst[y * row..(y + 1) * row].copy_from_slice(&src[s..s + row]);
    }
    out
}
