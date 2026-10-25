//! Assets panel (rail "Assets"): local components grouped by page and frame,
//! with search, a list/grid toggle and a rendered thumbnail per component.
//! Drag a row onto the canvas to place an instance.
//!
//! Thumbnails are `designos_render::render_node` renders scaled to fit, rendered
//! lazily (a couple per frame, only for rows that are on screen) and kept as
//! egui textures keyed by node id. Each texture remembers a fingerprint of the
//! component's subtree (plus the components its instances point at); the
//! fingerprint is re-checked every few tenths of a second for visible rows and
//! a changed one queues a re-render, so editing a component updates its
//! thumbnail without redrawing the others.

use super::{clipped, scrollbar, wheel_scroll};
use crate::app::App;
use crate::comp_ops::AssetEntry;
use crate::icons::{self, Icon};
use crate::libraries::LibEntry;
use crate::theme;
use crate::widgets;
use designos_doc::{Document, NodeId, NodeKind};
use egui::{Align2, Color32, Context, Id, LayerId, Order, Pos2, Rect, Sense, Stroke, StrokeKind, TextEdit, TextureHandle, TextureId, TextureOptions, Ui, Vec2};
use std::collections::{HashMap, HashSet};
use std::hash::Hasher;
use std::time::Instant;

#[derive(Default)]
pub struct AssetsState {
    pub query: String,
    pub scroll: f32,
    /// Component being dragged out of the panel.
    pub drag: Option<NodeId>,
    /// Grid of previews instead of the list.
    pub grid: bool,
    /// Library component being dragged out of the panel.
    pub lib_drag: Option<LibDrag>,
    thumbs: Thumbs,
    /// Thumbnails of library components, per library (their node ids are the library's own).
    lib_thumbs: HashMap<String, Thumbs>,
}

/// A component of an enabled library being dragged onto the canvas.
#[derive(Clone, Debug)]
pub struct LibDrag {
    pub library_id: String,
    pub key: String,
    pub name: String,
    pub size: designos_doc::Vec2,
}

impl AssetsState {
    /// Thumbnails rendered so far (tests and diagnostics).
    #[cfg(test)]
    pub fn thumbs_rendered(&self) -> usize {
        self.thumbs.renders
    }

    /// Whether a texture exists for `id`.
    #[cfg(test)]
    pub fn has_thumb(&self, id: NodeId) -> bool {
        self.thumbs.map.get(&id).is_some_and(|t| t.tex.is_some())
    }
}

const ROW_H: f32 = 48.0;
const GROUP_H: f32 = 28.0;
const SEARCH_H: f32 = 40.0;
/// Edge of the square thumbnail box in list view.
const THUMB: f32 = 40.0;
const GRID_GAP: f32 = 8.0;
const GRID_PREVIEW_H: f32 = 72.0;
const GRID_LABEL_H: f32 = 24.0;
const GRID_ROW_H: f32 = GRID_PREVIEW_H + GRID_LABEL_H + GRID_GAP;
const VIEW_BTN: f32 = 24.0;

#[derive(Clone)]
enum Line {
    Page(String),
    Container(String),
    /// One list row.
    Item(usize),
    /// One row of grid cells.
    Cells(Vec<usize>),
}

/// Entries matching `query` (case-insensitive on name, frame and page).
pub fn filtered(entries: &[AssetEntry], query: &str) -> Vec<usize> {
    let q = query.trim().to_lowercase();
    entries
        .iter()
        .enumerate()
        .filter(|(_, e)| q.is_empty() || e.name.to_lowercase().contains(&q) || e.container.to_lowercase().contains(&q) || e.page.to_lowercase().contains(&q))
        .map(|(i, _)| i)
        .collect()
}

/// Grid columns that fit `width`.
fn grid_cols(width: f32) -> usize {
    (((width - GRID_GAP) / 96.0).floor() as usize).clamp(1, 4)
}

fn lines(entries: &[AssetEntry], shown: &[usize], cols: Option<usize>) -> Vec<Line> {
    let mut out = Vec::new();
    let (mut page, mut container) = (String::new(), String::new());
    let mut pending: Vec<usize> = Vec::new();
    let flush = |out: &mut Vec<Line>, pending: &mut Vec<usize>| {
        if !pending.is_empty() {
            out.push(Line::Cells(std::mem::take(pending)));
        }
    };
    for &i in shown {
        let e = &entries[i];
        if (out.is_empty() && pending.is_empty()) || e.page != page {
            flush(&mut out, &mut pending);
            out.push(Line::Page(e.page.clone()));
            page = e.page.clone();
            container = String::new();
        }
        if e.container != container {
            flush(&mut out, &mut pending);
            container = e.container.clone();
            if !container.is_empty() {
                out.push(Line::Container(container.clone()));
            }
        }
        match cols {
            Some(c) => {
                pending.push(i);
                if pending.len() == c {
                    flush(&mut out, &mut pending);
                }
            }
            None => out.push(Line::Item(i)),
        }
    }
    flush(&mut out, &mut pending);
    out
}

fn line_h(l: &Line) -> f32 {
    match l {
        Line::Item(_) => ROW_H,
        Line::Cells(_) => GRID_ROW_H,
        _ => GROUP_H,
    }
}

// ---------------------------------------------------------------------------
// thumbnails

/// How often (seconds) a visible thumbnail's fingerprint is re-checked.
const RECHECK: f64 = 0.4;
/// Thumbnails rendered per frame, and the time after which no more start.
const RENDERS_PER_FRAME: usize = 2;
const RENDER_TIME_BUDGET_MS: u128 = 8;
/// Fingerprint checks per frame.
const CHECKS_PER_FRAME: usize = 8;
/// Textures kept; entries not drawn recently are dropped past this.
const MAX_TEXTURES: usize = 160;

struct Thumb {
    tex: Option<TextureHandle>,
    /// Fingerprint of the subtree the texture was rendered from.
    fp: u64,
    /// Pixel bounds the texture was rendered for.
    bounds: (u32, u32),
    checked: f64,
    used: u64,
    stale: bool,
}

#[derive(Default)]
struct Thumbs {
    map: HashMap<NodeId, Thumb>,
    frame: u64,
    renders: usize,
    renders_left: usize,
    checks_left: usize,
    started: Option<Instant>,
    /// Some visible thumbnail still waits for its render.
    pending: bool,
}

/// Hash of everything that decides how `id` renders: its subtree, and the
/// subtrees of the components its instances point at (a few levels deep).
fn fingerprint(doc: &Document, id: NodeId) -> u64 {
    fn rec(doc: &Document, id: NodeId, h: &mut std::collections::hash_map::DefaultHasher, seen: &mut HashSet<NodeId>, depth: usize) {
        if depth > 4 || !seen.insert(id) {
            return;
        }
        let mut targets = Vec::new();
        doc.walk(id, |n, _| {
            if let Ok(b) = serde_json::to_vec(n) {
                h.write(&b);
            }
            if let NodeKind::Instance { component: Some(c), .. } = &n.kind {
                targets.push(*c);
            }
        });
        for t in targets {
            rec(doc, t, h, seen, depth + 1);
        }
    }
    let mut h = std::collections::hash_map::DefaultHasher::new();
    rec(doc, id, &mut h, &mut HashSet::new(), 0);
    h.finish()
}

/// Renders `id` scaled to fit `bounds` pixels (never upscaled past 1:1 at
/// `max_scale`).
fn render_thumb(doc: &Document, id: NodeId, bounds: (u32, u32), max_scale: f32) -> Option<egui::ColorImage> {
    let n = doc.get(id)?;
    if !(n.size.x > 0.0 && n.size.y > 0.0) {
        return None;
    }
    let fit = (bounds.0 as f32 / n.size.x).min(bounds.1 as f32 / n.size.y);
    let scale = fit.min(max_scale).max(1e-3);
    let pm = designos_render::render_node(doc, id, scale)?;
    let (w, h) = (pm.width() as usize, pm.height() as usize);
    Some(egui::ColorImage::from_rgba_premultiplied([w, h], pm.data()))
}

impl Thumbs {
    fn begin_frame(&mut self) {
        self.frame += 1;
        self.renders_left = RENDERS_PER_FRAME;
        self.checks_left = CHECKS_PER_FRAME;
        self.started = Some(Instant::now());
        self.pending = false;
    }

    /// Drops textures that have not been drawn for a while.
    fn end_frame(&mut self) {
        if self.map.len() > MAX_TEXTURES {
            let frame = self.frame;
            self.map.retain(|_, t| t.used + 120 >= frame);
        }
    }

    /// Texture and its size in points for `id`, fitting a `max` points box.
    /// Renders (or re-renders) it when the budget allows; a stale texture keeps
    /// showing until its replacement is ready.
    fn get(&mut self, ctx: &Context, doc: &Document, id: NodeId, max: Vec2, now: f64) -> Option<(TextureId, Vec2)> {
        let ppp = ctx.pixels_per_point();
        let bounds = ((max.x * ppp).round().max(1.0) as u32, (max.y * ppp).round().max(1.0) as u32);
        let frame = self.frame;
        let t = self.map.entry(id).or_insert(Thumb { tex: None, fp: 0, bounds: (0, 0), checked: f64::NEG_INFINITY, used: 0, stale: true });
        t.used = frame;
        if t.bounds != bounds {
            t.stale = true;
        }
        if !t.stale && now - t.checked > RECHECK && self.checks_left > 0 {
            self.checks_left -= 1;
            t.checked = now;
            if fingerprint(doc, id) != t.fp {
                t.stale = true;
            }
        }
        if t.stale {
            let in_time = self.started.is_none_or(|s| s.elapsed().as_millis() < RENDER_TIME_BUDGET_MS);
            if self.renders_left > 0 && in_time {
                self.renders_left -= 1;
                self.renders += 1;
                t.fp = fingerprint(doc, id);
                t.bounds = bounds;
                t.checked = now;
                t.stale = false;
                match render_thumb(doc, id, bounds, ppp.max(1.0)) {
                    Some(img) => match &mut t.tex {
                        Some(h) => h.set(img, TextureOptions::LINEAR),
                        None => t.tex = Some(ctx.load_texture(format!("asset-thumb-{}", id.0), img, TextureOptions::LINEAR)),
                    },
                    None => t.tex = None,
                }
            } else {
                self.pending = true;
            }
        }
        t.tex.as_ref().map(|h| (h.id(), h.size_vec2() / ppp))
    }
}

// ---------------------------------------------------------------------------

/// What happened to the rows this frame.
#[derive(Default)]
struct Outcome {
    started: Option<NodeId>,
    /// Index into the library entries of a row drag that started.
    started_lib: Option<usize>,
    go_main: Option<NodeId>,
}

fn paint_thumb(p: &egui::Painter, thumbs: &mut Thumbs, ctx: &Context, doc: &Document, e: &AssetEntry, boxr: Rect, now: f64) {
    p.rect_filled(boxr, theme::cr(theme::RADIUS_CONTROL), theme::BG_SECONDARY);
    let inner = boxr.shrink(4.0);
    match thumbs.get(ctx, doc, e.id, inner.size(), now) {
        Some((tex, size)) => {
            let r = Rect::from_center_size(inner.center(), size);
            p.image(tex, r, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
        }
        None => icons::paint(p, Icon::Component, boxr.center(), 16.0, theme::CANVAS_COMPONENT),
    }
    p.rect_stroke(boxr, theme::cr(theme::RADIUS_CONTROL), Stroke::new(1.0, theme::BORDER_TRANSLUCENT), StrokeKind::Inside);
}

#[allow(clippy::too_many_arguments)]
fn list_item(
    ui: &mut Ui,
    p: &egui::Painter,
    thumbs: &mut Thumbs,
    doc: &Document,
    e: &AssetEntry,
    row: Rect,
    dragging: bool,
    src: Option<usize>,
    now: f64,
    out: &mut Outcome,
) {
    let r = ui.interact(row, ui.id().with(("asset", src, e.id.0)), Sense::click_and_drag());
    widgets::record(ui, &format!("asset:{}", e.name), row);
    let inset = row.shrink2(Vec2::new(theme::INSET_FRAME, 0.0));
    if r.hovered() || dragging {
        p.rect_filled(inset, theme::cr(theme::RADIUS_CONTROL), theme::BG_HOVER);
    }
    let boxr = Rect::from_min_size(Pos2::new(inset.left() + theme::INSET_CONTROL, row.center().y - THUMB / 2.0), Vec2::splat(THUMB));
    if row.intersects(ui.clip_rect()) {
        paint_thumb(p, thumbs, ui.ctx(), doc, e, boxr, now);
    }
    let label = if e.variants > 0 { format!("{} ({})", e.name, e.variants) } else { e.name.clone() };
    let s = widgets::elide(p, &label, &theme::font_body(), inset.right() - boxr.right() - 20.0);
    p.text(Pos2::new(boxr.right() + theme::SPACE_GROUP, row.center().y), Align2::LEFT_CENTER, s, theme::font_body(), theme::TEXT);
    row_events(&r, e, src, out);
}

/// Records drag starts and double clicks of a row.
fn row_events(r: &egui::Response, e: &AssetEntry, src: Option<usize>, out: &mut Outcome) {
    if r.drag_started() {
        match src {
            Some(k) => out.started_lib = Some(k),
            None => out.started = Some(e.id),
        }
    }
    if r.double_clicked() && src.is_none() {
        out.go_main = Some(e.id);
    }
}

#[allow(clippy::too_many_arguments)]
fn grid_cell(
    ui: &mut Ui,
    p: &egui::Painter,
    thumbs: &mut Thumbs,
    doc: &Document,
    e: &AssetEntry,
    cell: Rect,
    dragging: bool,
    src: Option<usize>,
    now: f64,
    out: &mut Outcome,
) {
    let r = ui.interact(cell, ui.id().with(("asset", src, e.id.0)), Sense::click_and_drag());
    let preview = Rect::from_min_size(cell.min, Vec2::new(cell.width(), GRID_PREVIEW_H));
    widgets::record(ui, &format!("asset:{}", e.name), preview);
    if r.hovered() || dragging {
        p.rect_filled(cell.expand(2.0), theme::cr(theme::RADIUS_CONTROL), theme::BG_HOVER);
    }
    if cell.intersects(ui.clip_rect()) {
        paint_thumb(p, thumbs, ui.ctx(), doc, e, preview, now);
        if e.variants > 0 {
            let badge_txt = format!("{}", e.variants);
            let bfont = theme::font_caption_strong();
            let bw = (widgets::text_width(p, &badge_txt, &bfont) + 20.0).max(26.0);
            let badge_r = Rect::from_min_size(Pos2::new(preview.right() - bw - 4.0, preview.top() + 4.0), Vec2::new(bw, 16.0));
            p.rect_filled(badge_r, theme::cr_pill(badge_r.height()), theme::CANVAS_COMPONENT);
            icons::paint(p, Icon::Component, Pos2::new(badge_r.left() + 8.0, badge_r.center().y), 10.0, Color32::WHITE);
            p.text(Pos2::new(badge_r.left() + 15.0, badge_r.center().y), Align2::LEFT_CENTER, badge_txt, bfont, Color32::WHITE);
        }
    }
    let label = if e.variants > 0 { format!("{} ({})", e.name, e.variants) } else { e.name.clone() };
    let s = widgets::elide(p, &label, &theme::font_caption(), cell.width() - 4.0);
    p.text(Pos2::new(cell.left() + 2.0, preview.bottom() + GRID_LABEL_H / 2.0), Align2::LEFT_CENTER, s, theme::font_caption(), theme::TEXT_SECONDARY);
    row_events(&r, e, src, out);
}

/// The list / grid switch next to the search box.
fn view_toggle(ui: &mut Ui, at: Pos2, grid: &mut bool) {
    for (i, is_grid) in [false, true].into_iter().enumerate() {
        let rect = Rect::from_min_size(Pos2::new(at.x + i as f32 * (VIEW_BTN + 2.0), at.y), Vec2::splat(VIEW_BTN));
        let r = ui.interact(rect, ui.id().with(("assets-view", is_grid)), Sense::click());
        let selected = *grid == is_grid;
        widgets::button_bg(ui.painter(), rect, &r, selected, theme::cr(theme::RADIUS_CONTROL));
        widgets::record(ui, if is_grid { "assets-view-grid" } else { "assets-view-list" }, rect);
        let col = if selected {
            theme::ICON_SELECTED
        } else if r.hovered() {
            theme::ICON
        } else {
            theme::ICON_SECONDARY
        };
        let p = ui.painter();
        let c = rect.center();
        icons::paint(p, if is_grid { Icon::GridView } else { Icon::ListView }, c, 15.0, col);
        if r.on_hover_text(if is_grid { "Grid view" } else { "List view" }).clicked() {
            *grid = is_grid;
        }
    }
}

pub fn show(app: &mut App, ui: &mut Ui, region: Rect) {
    let p = ui.painter().clone();
    // ---- 1. Figma UI3 40px Header Row: "Assets" + Libraries book icon button
    let hdr_h = SEARCH_H;
    p.text(
        Pos2::new(region.left() + theme::INSET_PANEL, region.top() + hdr_h * 0.5),
        Align2::LEFT_CENTER,
        "Assets",
        theme::font_large_strong(),
        Color32::WHITE,
    );
    let libs_btn = Rect::from_min_size(Pos2::new(region.right() - 8.0 - VIEW_BTN, region.top() + (hdr_h - VIEW_BTN) * 0.5), Vec2::splat(VIEW_BTN));
    widgets::record(ui, "assets-libraries", libs_btn);
    if widgets::icon_button(ui, libs_btn, "assets-libraries", Icon::BookOpen, app.libs.open, "Libraries").clicked() {
        app.open_libraries();
    }
    widgets::hline(&p, region.left(), region.right(), region.top() + hdr_h, theme::BORDER);

    // ---- 2. Search Bar Row: "Search all libraries" (#0D99FF 1px border) + Filter/Sliders + View Toggle
    let search_y = region.top() + hdr_h + theme::SPACE_INLINE;
    let toggle_w = VIEW_BTN * 2.0 + 2.0;
    let filter_btn_w = VIEW_BTN + 4.0;
    let sr = Rect::from_min_size(
        Pos2::new(region.left() + theme::INSET_PANEL, search_y),
        Vec2::new((region.width() - 2.0 * theme::INSET_PANEL - filter_btn_w - toggle_w - theme::SPACE_FACT).max(80.0), theme::CONTROL_STANDARD),
    );
    p.rect_filled(sr, theme::cr(theme::RADIUS_CONTROL), Color32::from_rgb(0x1E, 0x1E, 0x1E));
    p.rect_stroke(sr, theme::cr(theme::RADIUS_CONTROL), Stroke::new(1.0, Color32::from_rgb(0x0D, 0x99, 0xFF)), StrokeKind::Inside);
    icons::paint(&p, Icon::Search, Pos2::new(sr.left() + 14.0, sr.center().y), 14.0, theme::ICON_SECONDARY);
    let mut q = std::mem::take(&mut app.assets.query);
    let te = TextEdit::singleline(&mut q)
        .id(ui.id().with("assets-search"))
        .font(theme::font_body())
        .text_color(theme::TEXT)
        .frame(egui::Frame::NONE)
        .hint_text("Search all libraries")
        .margin(egui::Margin::symmetric(0, 0))
        .vertical_align(egui::Align::Center)
        .desired_width((sr.width() - 32.0).max(40.0));
    let inner = Rect::from_min_max(Pos2::new(sr.left() + 26.0, sr.top()), sr.max);
    ui.put(inner, te);
    widgets::record(ui, "assets-search", sr);
    app.assets.query = q;

    let cat_id = Id::new("assets-cat-filter");
    let mut cat = ui.ctx().data(|d| d.get_temp::<u8>(cat_id)).unwrap_or(0);

    let filter_btn = Rect::from_min_size(Pos2::new(sr.right() + 4.0, search_y + 2.0), Vec2::splat(VIEW_BTN));
    widgets::record(ui, "assets-filter-btn", filter_btn);
    if widgets::icon_button(ui, filter_btn, "assets-filter-btn", Icon::StrokeSettings, cat != 0, "Cycle asset category filter").clicked() {
        cat = (cat + 1) % 3;
        ui.ctx().data_mut(|d| d.insert_temp(cat_id, cat));
        app.assets.scroll = 0.0;
    }

    let mut grid = app.assets.grid;
    view_toggle(ui, Pos2::new(filter_btn.right() + 4.0, search_y + 2.0), &mut grid);
    if grid != app.assets.grid {
        app.assets.grid = grid;
        app.assets.scroll = 0.0;
    }

    // ---- 3. UI3 Category Filter Pills: All | Local | Used in file
    let pill_y = search_y + 34.0;
    let mut pill_x = region.left() + theme::INSET_PANEL;
    for (idx, label) in [(0u8, "All"), (1u8, "Local"), (2u8, "Used in file")] {
        let pfont = if cat == idx { theme::font_caption_strong() } else { theme::font_caption() };
        let pw = widgets::text_width(&p, label, &pfont) + 14.0;
        let pr = Rect::from_min_size(Pos2::new(pill_x, pill_y), Vec2::new(pw, 20.0));
        widgets::record(ui, &format!("assets-filter:{label}"), pr);
        let presp = ui.interact(pr, ui.id().with(("assets-cat", idx)), Sense::click());
        if cat == idx {
            p.rect_filled(pr, theme::cr_pill(pr.height()), theme::BG_SECONDARY);
        } else if presp.hovered() {
            p.rect_filled(pr, theme::cr_pill(pr.height()), theme::BG_HOVER);
        }
        p.text(pr.center(), Align2::CENTER_CENTER, label, pfont, if cat == idx { theme::TEXT } else { theme::TEXT_SECONDARY });
        if presp.clicked() && cat != idx {
            cat = idx;
            ui.ctx().data_mut(|d| d.insert_temp(cat_id, cat));
            app.assets.scroll = 0.0;
        }
        pill_x += pw + 4.0;
    }

    // ---- 4. "All libraries" section + vertical "Created in this file" card + "Add more libraries" button
    let sec_top = pill_y + 26.0;
    p.text(Pos2::new(region.left() + theme::INSET_PANEL, sec_top + 8.0), Align2::LEFT_CENTER, "All libraries", theme::font_body_strong(), Color32::WHITE);

    type AssetsDocCacheVal = (Vec<AssetEntry>, Vec<AssetEntry>, usize, std::collections::HashSet<NodeId>);
    type AssetsDocCacheEntry = ((u64, u64, usize, u8), AssetsDocCacheVal);
    thread_local! {
        static ASSETS_DOC_CACHE: std::cell::RefCell<Option<AssetsDocCacheEntry>> = const { std::cell::RefCell::new(None) };
    }
    let assets_key = (app.doc.runtime_id(), app.doc.mutation_epoch(), app.doc.nodes.len(), cat);
    let (local_comps, unavail_comps, real_count, used_components) = ASSETS_DOC_CACHE.with(|cache| {
        if let Some((k, v)) = cache.borrow().as_ref() {
            if *k == assets_key {
                return v.clone();
            }
        }
        let local_comps = app.local_components();
        let real_count = local_comps.len().max(app.doc.nodes.values().filter(|n| matches!(n.kind, NodeKind::Component(_) | NodeKind::ComponentSet(_))).count());
        let unavail_comps = app.unavailable_library_components();
        let used_components: std::collections::HashSet<NodeId> = if cat == 2 {
            let mut set = std::collections::HashSet::new();
            for n in app.doc.nodes.values() {
                if let NodeKind::Instance { component: Some(cid), .. } = n.kind {
                    set.insert(cid);
                    if let Some(parent_id) = app.doc.get(cid).and_then(|cn| cn.parent) {
                        set.insert(parent_id);
                    }
                }
            }
            set
        } else {
            std::collections::HashSet::new()
        };
        let val = (local_comps, unavail_comps, real_count, used_components);
        *cache.borrow_mut() = Some((assets_key, val.clone()));
        val
    });
    let card_r = Rect::from_min_size(
        Pos2::new(region.left() + theme::INSET_PANEL, sec_top + 22.0),
        Vec2::new((region.width() - 2.0 * theme::INSET_PANEL).max(80.0), 86.0),
    );
    widgets::record(ui, "assets-created-in-file", card_r);
    widgets::record(ui, "assets-created-in-file-card", card_r);
    let card_resp = ui.interact(card_r, ui.id().with("assets-created-in-file-card"), Sense::click());
    let preview_r = Rect::from_min_size(card_r.min, Vec2::new(card_r.width(), 52.0));
    p.rect_filled(
        preview_r,
        theme::cr(theme::RADIUS_CONTROL),
        if card_resp.hovered() { Color32::from_rgb(0x38, 0x38, 0x38) } else { Color32::from_rgb(0x2C, 0x2C, 0x2C) },
    );
    p.rect_stroke(preview_r, theme::cr(theme::RADIUS_CONTROL), Stroke::new(1.0, Color32::from_rgb(0x38, 0x38, 0x38)), StrokeKind::Inside);
    icons::paint(&p, Icon::Component, preview_r.center(), 18.0, Color32::from_rgb(0x8C, 0x8C, 0x8C));
    p.text(Pos2::new(card_r.left() + 2.0, preview_r.bottom() + 11.0), Align2::LEFT_CENTER, "Created in this file", theme::font_body_strong(), Color32::WHITE);
    p.text(
        Pos2::new(card_r.left() + 2.0, preview_r.bottom() + 26.0),
        Align2::LEFT_CENTER,
        format!("{real_count} component{}", if real_count == 1 { "" } else { "s" }),
        theme::font_caption(),
        Color32::from_rgb(0x8C, 0x8C, 0x8C),
    );
    if card_resp.on_hover_text("Filter to local components & focus first component").clicked() {
        cat = 1;
        ui.ctx().data_mut(|d| d.insert_temp(cat_id, cat));
        app.assets.scroll = 0.0;
        if let Some(first) = local_comps.first() {
            app.select_only(first.id);
            app.fit_selection();
        }
    }

    let add_libs_r = Rect::from_min_size(
        Pos2::new(region.left() + theme::INSET_PANEL, card_r.bottom() + 6.0),
        Vec2::new((region.width() - 2.0 * theme::INSET_PANEL).max(80.0), theme::CONTROL_STANDARD),
    );
    widgets::record(ui, "assets-add-more-libraries", add_libs_r);
    let add_libs_resp = ui.interact(add_libs_r, ui.id().with("assets-add-more-libs"), Sense::click());
    p.rect_filled(
        add_libs_r,
        theme::cr(theme::RADIUS_CONTROL),
        if add_libs_resp.hovered() { Color32::from_rgb(0x38, 0x38, 0x38) } else { Color32::from_rgb(0x2C, 0x2C, 0x2C) },
    );
    p.rect_stroke(add_libs_r, theme::cr(theme::RADIUS_CONTROL), Stroke::new(1.0, Color32::from_rgb(0x38, 0x38, 0x38)), StrokeKind::Inside);
    p.text(add_libs_r.center(), Align2::CENTER_CENTER, "Add more libraries", theme::font_body_strong(), Color32::WHITE);
    if add_libs_resp.clicked() {
        app.open_libraries();
    }

    // ---- list
    let list = Rect::from_min_max(Pos2::new(region.left(), add_libs_r.bottom() + 8.0), region.max);
    // Local components, then imported ones whose library is unavailable, then one
    // section per enabled library (entries with a `Some` source index).
    let mut entries = local_comps;
    entries.extend(unavail_comps);
    let lib_entries = app.library_entries();
    let mut srcs: Vec<Option<usize>> = vec![None; entries.len()];
    if cat != 1 {
        for (k, le) in lib_entries.iter().enumerate() {
            entries.push(AssetEntry { page: le.library_name.clone(), container: String::new(), name: le.name.clone(), id: le.node, variants: le.variants });
            srcs.push(Some(k));
        }
    }
    let mut lib_docs: HashMap<String, std::sync::Arc<Document>> = HashMap::new();
    for le in lib_entries.iter() {
        if !lib_docs.contains_key(&le.library_id) {
            if let Some(d) = app.library_render_doc(&le.library_id) {
                lib_docs.insert(le.library_id.clone(), d);
            }
        }
    }
    let shown: Vec<usize> =
        filtered(&entries, &app.assets.query).into_iter().filter(|&i| cat != 2 || (srcs[i].is_none() && used_components.contains(&entries[i].id))).collect();
    let cols = app.assets.grid.then(|| grid_cols(list.width() - 2.0 * theme::INSET_PANEL));
    let ls = lines(&entries, &shown, cols);
    let content_h: f32 = ls.iter().map(line_h).sum::<f32>() + 8.0;
    let mut scroll = app.assets.scroll;
    wheel_scroll(ui, list, &mut scroll, content_h);
    app.assets.scroll = scroll;
    if entries.is_empty() {
        p.text(Pos2::new(list.center().x, list.top() + 20.0), Align2::CENTER_CENTER, "No components yet", theme::font_body(), theme::TEXT_TERTIARY);
        p.text(
            Pos2::new(list.center().x, list.top() + 38.0),
            Align2::CENTER_CENTER,
            "Select a frame and press Ctrl+Alt+K",
            theme::font_caption(),
            theme::TEXT_TERTIARY,
        );
    } else if shown.is_empty() {
        p.text(Pos2::new(list.center().x, list.top() + 20.0), Align2::CENTER_CENTER, "No results", theme::font_body(), theme::TEXT_TERTIARY);
    }

    let mut thumbs = std::mem::take(&mut app.assets.thumbs);
    thumbs.begin_frame();
    let mut lib_thumbs = std::mem::take(&mut app.assets.lib_thumbs);
    for t in lib_thumbs.values_mut() {
        t.begin_frame();
    }
    let dragging = app.assets.drag;
    let lib_dragging = app.assets.lib_drag.as_ref().and_then(|d| lib_entries.iter().position(|e| e.library_id == d.library_id && e.key == d.key));
    let now = app.now;
    let mut out = Outcome::default();
    {
        let doc = &app.doc;
        clipped(ui, list, |ui| {
            let p = ui.painter().clone();
            let mut y = list.top() - scroll;
            for l in &ls {
                let h = line_h(l);
                let row = Rect::from_min_size(Pos2::new(list.left(), y), Vec2::new(list.width(), h));
                y += h;
                if row.bottom() < list.top() || row.top() > list.bottom() {
                    continue;
                }
                match l {
                    Line::Page(name) => {
                        p.text(
                            Pos2::new(row.left() + theme::PANEL_PADDING_X, row.center().y),
                            Align2::LEFT_CENTER,
                            name,
                            theme::font_body_strong(),
                            theme::TEXT,
                        );
                    }
                    Line::Container(name) => {
                        let s = widgets::elide(&p, name, &theme::font_body(), row.width() - 40.0);
                        p.text(
                            Pos2::new(row.left() + theme::PANEL_PADDING_X + 8.0, row.center().y),
                            Align2::LEFT_CENTER,
                            s,
                            theme::font_body(),
                            theme::TEXT_SECONDARY,
                        );
                    }
                    Line::Item(i) => {
                        let src = srcs[*i];
                        let is_drag = src.map(|k| lib_dragging == Some(k)).unwrap_or(dragging == Some(entries[*i].id));
                        match src.and_then(|k| lib_source(&lib_entries, &lib_docs, &mut lib_thumbs, k)) {
                            Some((d, t)) => list_item(ui, &p, t, d, &entries[*i], row, is_drag, src, now, &mut out),
                            None if src.is_none() => list_item(ui, &p, &mut thumbs, doc, &entries[*i], row, is_drag, None, now, &mut out),
                            None => {}
                        }
                    }
                    Line::Cells(ix) => {
                        let n = cols.unwrap_or(1) as f32;
                        let cw = (list.width() - 2.0 * theme::INSET_PANEL - GRID_GAP * (n - 1.0)) / n;
                        for (k, i) in ix.iter().enumerate() {
                            let cell = Rect::from_min_size(
                                Pos2::new(list.left() + theme::INSET_PANEL + k as f32 * (cw + GRID_GAP), row.top()),
                                Vec2::new(cw, GRID_PREVIEW_H + GRID_LABEL_H),
                            );
                            let src = srcs[*i];
                            let is_drag = src.map(|k| lib_dragging == Some(k)).unwrap_or(dragging == Some(entries[*i].id));
                            match src.and_then(|k| lib_source(&lib_entries, &lib_docs, &mut lib_thumbs, k)) {
                                Some((d, t)) => grid_cell(ui, &p, t, d, &entries[*i], cell, is_drag, src, now, &mut out),
                                None if src.is_none() => grid_cell(ui, &p, &mut thumbs, doc, &entries[*i], cell, is_drag, None, now, &mut out),
                                None => {}
                            }
                        }
                    }
                }
            }
        });
    }
    thumbs.end_frame();
    let mut pending = thumbs.pending;
    for t in lib_thumbs.values_mut() {
        t.end_frame();
        pending |= t.pending;
    }
    if pending {
        ui.ctx().request_repaint();
    }
    app.assets.thumbs = thumbs;
    app.assets.lib_thumbs = lib_thumbs;
    scrollbar(ui, list, scroll, content_h);
    if let Some(s) = super::take_scrollbar_drag(ui, list) {
        app.assets.scroll = s;
    }
    if out.started.is_some() {
        app.assets.drag = out.started;
    }
    if let Some(le) = out.started_lib.and_then(|k| lib_entries.get(k)) {
        app.assets.lib_drag = Some(LibDrag { library_id: le.library_id.clone(), key: le.key.clone(), name: le.name.clone(), size: le.size });
    }
    if let Some(c) = out.go_main {
        app.go_to_main_component(c);
    }
    drag(app, ui);
}

/// Library entry `k` with its render document and thumbnail cache.
fn lib_source<'a>(
    entries: &[LibEntry],
    docs: &'a HashMap<String, std::sync::Arc<Document>>,
    thumbs: &'a mut HashMap<String, Thumbs>,
    k: usize,
) -> Option<(&'a Document, &'a mut Thumbs)> {
    let le = entries.get(k)?;
    let d = docs.get(&le.library_id)?;
    Some((d.as_ref(), thumbs.entry(le.library_id.clone()).or_default()))
}

/// Ghost under the pointer while dragging; drops an instance on release.
fn drag(app: &mut App, ui: &mut Ui) {
    if app.assets.drag.is_none() && app.assets.lib_drag.is_none() {
        return;
    }
    let (ptr, released) = ui.input(|i| (i.pointer.interact_pos(), i.pointer.primary_released()));
    let over_canvas = ptr.is_some_and(|p| app.canvas_rect.contains(p));
    let ghost = match (app.assets.drag.and_then(|c| app.doc.get(c)), &app.assets.lib_drag) {
        (Some(c), _) => Some((c.name.clone(), c.size)),
        (None, Some(d)) => Some((d.name.clone(), d.size)),
        _ => None,
    };
    if let (Some(pos), Some((name, csize))) = (ptr, ghost) {
        let p = ui.ctx().layer_painter(LayerId::new(Order::Tooltip, Id::new("asset-drag")));
        if over_canvas {
            let size = Vec2::new(csize.x * app.view.zoom, csize.y * app.view.zoom);
            let r = Rect::from_center_size(pos, size);
            p.rect_filled(r, 0.0, theme::CANVAS_COMPONENT.gamma_multiply(0.18));
            p.rect_stroke(r, 0.0, Stroke::new(1.0, theme::CANVAS_COMPONENT), StrokeKind::Inside);
        } else {
            let font = theme::font_body();
            let w = widgets::text_width(&p, &name, &font) + 24.0;
            let r = Rect::from_min_size(pos + Vec2::new(12.0, 12.0), Vec2::new(w, 24.0));
            p.rect_filled(r, theme::cr(theme::RADIUS_CONTROL), theme::BG_MENU);
            p.text(r.center(), Align2::CENTER_CENTER, &name, font, Color32::WHITE);
        }
    }
    if released {
        let comp = app.assets.drag.take();
        let lib = app.assets.lib_drag.take();
        if let Some(pos) = ptr.filter(|_| over_canvas) {
            let at = app.view.to_page(app.canvas_rect.min, pos);
            if let Some(comp) = comp {
                app.insert_instance(comp, at);
            } else if let Some(d) = lib {
                app.drop_library_asset(&d.library_id, &d.key, at);
            }
        }
    }
}
