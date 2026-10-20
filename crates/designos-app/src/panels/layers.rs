//! Layers tree: reflects the page's node tree; select, rename, toggle,
//! expand/collapse and drag to reorder/reparent.

use super::{clipped, scrollbar, wheel_scroll};
use crate::app::App;
use crate::icons::{self, Icon};
use crate::theme;
use crate::widgets::{self, field};
use designos_doc::{Document, NodeId, NodeKind};
use egui::{Align2, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, Vec2};
use std::collections::HashSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DropKind {
    Before,
    After,
    Inside,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Drop {
    pub row: usize,
    pub kind: DropKind,
}

#[derive(Default)]
pub struct LayersState {
    pub expanded: HashSet<NodeId>,
    pub expanded_gen: u64,
    pub rows_cache: Vec<Row>,
    pub cache_key: Option<(NodeId, u64, u64, bool, String)>,
    pub scroll: f32,
    pub page_scroll: f32,
    pub renaming: Option<NodeId>,
    pub renaming_page: Option<NodeId>,
    /// Right-clicked page and where its context menu opens.
    pub page_menu: Option<(NodeId, Pos2)>,
    pub page_drag: Option<NodeId>,
    /// Insertion slot (0..=pages) while dragging a page.
    pub page_drop: Option<usize>,
    pub rename_started: bool,
    pub rename_seen: bool,
    pub drag: Option<Vec<NodeId>>,
    pub drop: Option<Drop>,
    pub last_sel: Vec<NodeId>,
    pub anchor: Option<NodeId>,
    /// Name filter (the header's search icon): shown, its text, and a focus request.
    pub filter_open: bool,
    pub filter: String,
    pub filter_focus: bool,
}

impl LayersState {
    pub fn reset(&mut self) {
        *self = LayersState::default();
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Row {
    pub id: NodeId,
    pub depth: usize,
    pub has_children: bool,
}

const MAX_FLATTENED_ROWS: usize = 20_000;

/// Visible rows, top-most layer first (reverse z-order), honoring expansion.
pub fn flatten(doc: &Document, page: NodeId, expanded: &HashSet<NodeId>) -> Vec<Row> {
    fn rec(doc: &Document, id: NodeId, depth: usize, expanded: &HashSet<NodeId>, out: &mut Vec<Row>) {
        if out.len() >= MAX_FLATTENED_ROWS {
            return;
        }
        let Some(n) = doc.get(id) else { return };
        for c in n.children.iter().rev() {
            if out.len() >= MAX_FLATTENED_ROWS {
                return;
            }
            let Some(cn) = doc.get(*c) else { continue };
            let has = !cn.children.is_empty();
            out.push(Row { id: *c, depth, has_children: has });
            if has && expanded.contains(c) {
                rec(doc, *c, depth + 1, expanded, out);
            }
        }
    }
    let mut out = Vec::new();
    rec(doc, page, 0, expanded, &mut out);
    out
}

/// Computes the drop target for a pointer at `y` (relative to the first row).
pub fn drop_for(y: f32, rows: &[Row], doc: &Document, dragged: &[NodeId]) -> Option<Drop> {
    if rows.is_empty() {
        return None;
    }
    let idx = (y / theme::LAYER_ROW_HEIGHT).floor();
    if idx < 0.0 {
        return Some(Drop { row: 0, kind: DropKind::Before });
    }
    let idx = idx as usize;
    if idx >= rows.len() {
        return Some(Drop { row: rows.len() - 1, kind: DropKind::After });
    }
    let row = rows[idx];
    if dragged.iter().any(|d| *d == row.id || doc.is_ancestor(*d, row.id)) {
        return None;
    }
    let rel = (y - idx as f32 * theme::LAYER_ROW_HEIGHT) / theme::LAYER_ROW_HEIGHT;
    let container = doc.get(row.id).map(|n| n.kind.is_container() && !matches!(n.kind, NodeKind::Instance { .. })).unwrap_or(false);
    let kind = if container {
        if rel < 0.25 {
            DropKind::Before
        } else if rel > 0.75 {
            DropKind::After
        } else {
            DropKind::Inside
        }
    } else if rel < 0.5 {
        DropKind::Before
    } else {
        DropKind::After
    };
    Some(Drop { row: idx, kind })
}

/// Resolves a drop to (parent, index) for `App::move_to_parent` (index counted
/// without the dragged nodes).
pub fn resolve_drop(doc: &Document, rows: &[Row], drop: Drop, dragged: &[NodeId]) -> Option<(NodeId, usize)> {
    let row = rows.get(drop.row)?;
    let (parent, mut index) = match drop.kind {
        DropKind::Inside => (row.id, doc.get(row.id)?.children.len()),
        DropKind::Before | DropKind::After => {
            let parent = doc.get(row.id)?.parent?;
            let pos = doc.get(parent)?.children.iter().position(|c| *c == row.id)?;
            // Layers are listed top-first, so "before" (visually above) means a higher z index.
            (parent, if drop.kind == DropKind::Before { pos + 1 } else { pos })
        }
    };
    let siblings = &doc.get(parent)?.children;
    let removed_before = dragged.iter().filter(|d| siblings.iter().position(|c| c == *d).map(|p| p < index).unwrap_or(false)).count();
    index = index.saturating_sub(removed_before);
    Some((parent, index))
}

pub(crate) fn kind_icon(k: &NodeKind) -> Icon {
    match k {
        NodeKind::Page { .. } => Icon::Page,
        NodeKind::Frame(f) => {
            if let Some(al) = &f.auto_layout {
                if al.direction == designos_doc::LayoutDirection::Horizontal {
                    Icon::AutoLayoutH
                } else {
                    Icon::AutoLayoutV
                }
            } else {
                Icon::Frame
            }
        }
        NodeKind::Group => Icon::Group,
        NodeKind::Section => Icon::Section,
        NodeKind::Rectangle => Icon::Rectangle,
        NodeKind::Ellipse(_) => Icon::Ellipse,
        NodeKind::Line => Icon::Line,
        NodeKind::Polygon { .. } => Icon::Polygon,
        NodeKind::Star { .. } => Icon::Star,
        NodeKind::Vector { .. } => Icon::Vector,
        NodeKind::Text(_) => Icon::Text,
        NodeKind::BooleanOp(_) => Icon::Boolean,
        NodeKind::Component(_) => Icon::Component,
        NodeKind::ComponentSet(_) => Icon::ComponentSet,
        NodeKind::Instance { .. } => Icon::Instance,
        NodeKind::Sticky(_) => Icon::Sticky,
        NodeKind::Connector(_) => Icon::Connector,
        NodeKind::ShapeWithText(_) => Icon::Shapes,
        NodeKind::Stamp { .. } => Icon::Stamp,
        NodeKind::Unknown { .. } => Icon::Rectangle,
    }
}

pub fn kind_label(k: &NodeKind) -> &'static str {
    match k {
        NodeKind::Page { .. } => "Page",
        NodeKind::Frame(f) => match f.auto_layout.as_ref().map(|al| al.direction) {
            Some(designos_doc::LayoutDirection::Grid) => "Grid",
            Some(designos_doc::LayoutDirection::Horizontal | designos_doc::LayoutDirection::Vertical) => "Auto layout",
            None => "Frame",
        },
        NodeKind::Group => "Group",
        NodeKind::Section => "Section",
        NodeKind::Rectangle => "Rect",
        NodeKind::Ellipse(_) => "Ellipse",
        NodeKind::Line => "Line",
        NodeKind::Polygon { .. } => "Polygon",
        NodeKind::Star { .. } => "Star",
        NodeKind::Vector { .. } => "Vector",
        NodeKind::Text(_) => "Text",
        NodeKind::BooleanOp(_) => "Boolean",
        NodeKind::Component(_) => "Component",
        NodeKind::ComponentSet(_) => "Variant set",
        NodeKind::Instance { .. } => "Instance",
        NodeKind::Sticky(_) => "Sticky",
        NodeKind::Connector(_) => "Connector",
        NodeKind::ShapeWithText(_) => "Shape",
        NodeKind::Stamp { .. } => "Stamp",
        NodeKind::Unknown { .. } => "Layer",
    }
}

fn is_component_like(n: &designos_doc::Node) -> bool {
    // Components, instances and every layer inside an instance read as purple.
    matches!(n.kind, NodeKind::Component(_) | NodeKind::ComponentSet(_) | NodeKind::Instance { .. }) || n.component.source.is_some()
}

/// Gap between the name and the type label, plus the label's inset from the end of the text column.
const TYPE_LABEL_PAD: f32 = 8.0;
/// Narrowest name that still reads once the type label takes its share of the text column.
const MIN_NAME_WIDTH_WITH_LABEL: f32 = 16.0;

/// How a row's text column is shared between the layer name and the right-aligned type label.
#[derive(Clone, Copy, Debug, PartialEq)]
struct RowTextSplit {
    /// Right edge available to the name.
    name_right: f32,
    /// False when the column is too narrow for the label plus a readable name: the name keeps the whole column.
    show_label: bool,
}

/// `label_w` is the label's text width plus [`TYPE_LABEL_PAD`].
fn split_row_text(text_left: f32, text_right: f32, label_w: f32) -> RowTextSplit {
    if text_right - text_left >= label_w + MIN_NAME_WIDTH_WITH_LABEL {
        RowTextSplit { name_right: text_right - label_w, show_label: true }
    } else {
        RowTextSplit { name_right: text_right, show_label: false }
    }
}

/// The type label is dimmer than the name and greys out together with it when the layer is hidden.
fn type_label_color(node_visible: bool, component_like: bool) -> egui::Color32 {
    if node_visible && component_like {
        theme::TEXT_COMPONENT.gamma_multiply(0.75)
    } else {
        theme::TEXT_TERTIARY
    }
}

fn compute_visible_rows(app: &App) -> Vec<Row> {
    match crate::find::filter::matcher(&app.layers.filter).filter(|_| app.layers.filter_open) {
        Some(m) => crate::find::filter::rows(&app.doc, app.page, &m),
        None => flatten(&app.doc, app.page, &app.layers.expanded),
    }
}

fn structural_hash(app: &App) -> u64 {
    let mut h = (app.doc.pages.len() as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (app.doc.nodes.len() as u64).wrapping_mul(0xBF58_476D_1CE4_E5B9)
        ^ (app.layers.expanded.len() as u64).wrapping_mul(0x94D0_49BB_1331_11EB);
    if let Some(page) = app.doc.get(app.page) {
        h ^= (page.children.len() as u64).wrapping_mul(0x94D0_49BB_1331_11EB);
        for (i, &cid) in page.children.iter().take(64).enumerate() {
            let child_kids = app.doc.get(cid).map_or(0, |n| n.children.len()) as u64;
            h = h.rotate_left(7) ^ cid.0.wrapping_add((i as u64) << 32) ^ (child_kids << 16);
        }
        if let Some(&last_cid) = page.children.last() {
            h ^= last_cid.0.rotate_left(19);
        }
    }
    if app.layers.expanded.len() <= 64 {
        let mut exp_acc = 0u64;
        for &eid in &app.layers.expanded {
            if let Some(en) = app.doc.get(eid) {
                let first = en.children.first().map_or(0, |c| c.0);
                let last = en.children.last().map_or(0, |c| c.0);
                let item = eid.0.wrapping_mul(0x517C_C1B7_2722_0A95)
                    ^ (en.children.len() as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
                    ^ first.rotate_left(13)
                    ^ last.rotate_left(29);
                exp_acc = exp_acc.wrapping_add(item);
            }
        }
        h ^= exp_acc;
    }
    if app.layers.filter_open && !app.layers.filter.is_empty() {
        h ^= app.canvas.tiles.rev().wrapping_mul(0x94D0_49BB_1331_11EB);
    }
    h
}

fn current_cache_key(app: &App) -> (NodeId, u64, u64, bool, String) {
    (app.page, structural_hash(app), app.layers.expanded_gen, app.layers.filter_open, app.layers.filter.clone())
}

/// The rows to show: the whole tree, or only what matches the name filter.
#[allow(dead_code)]
pub(crate) fn visible_rows(app: &App) -> Vec<Row> {
    let key = current_cache_key(app);
    if app.layers.cache_key.as_ref() == Some(&key) {
        return app.layers.rows_cache.clone();
    }
    compute_visible_rows(app)
}

fn ensure_rows_cache(app: &mut App) {
    let key = current_cache_key(app);
    if app.layers.cache_key.as_ref() != Some(&key) {
        app.layers.rows_cache = compute_visible_rows(app);
        app.layers.cache_key = Some(key);
    }
}

#[allow(dead_code)]
fn visible_rows_cached(app: &mut App) -> Vec<Row> {
    ensure_rows_cache(app);
    app.layers.rows_cache.clone()
}

pub fn show(app: &mut App, ui: &mut Ui, region: Rect) {
    // Reveal newly selected layers once marquee interaction settles.
    let in_marquee = matches!(app.canvas.inter, crate::canvas::Interaction::Marquee { .. });
    if !in_marquee && app.selection != app.layers.last_sel {
        let sel = app.selection.clone();
        let huge_doc = app.doc.nodes.len() > 10_000;
        if !huge_doc || sel.len() <= 4 {
            for id in sel.iter().take(8) {
                let mut cur = app.doc.get(*id).and_then(|n| n.parent);
                while let Some(p) = cur {
                    if (!huge_doc || app.doc.get(p).is_some_and(|n| n.children.len() <= 500)) && app.layers.expanded.insert(p) {
                        app.layers.expanded_gen = app.layers.expanded_gen.wrapping_add(1);
                    }
                    cur = app.doc.get(p).and_then(|n| n.parent);
                }
            }
        }
        ensure_rows_cache(app);
        if let Some(i) = sel.first().and_then(|s| app.layers.rows_cache.iter().position(|r| r.id == *s)) {
            let top = i as f32 * theme::LAYER_ROW_HEIGHT;
            if top < app.layers.scroll {
                app.layers.scroll = top;
            } else if top + theme::LAYER_ROW_HEIGHT > app.layers.scroll + region.height() {
                app.layers.scroll = top + theme::LAYER_ROW_HEIGHT - region.height();
            }
        }
        app.layers.last_sel = sel;
    }
    ensure_rows_cache(app);
    let total_rows = app.layers.rows_cache.len();
    let filter = crate::find::filter::matcher(&app.layers.filter).filter(|_| app.layers.filter_open);
    let content_h = total_rows as f32 * theme::LAYER_ROW_HEIGHT + 8.0;
    let mut scroll = app.layers.scroll;
    wheel_scroll(ui, region, &mut scroll, content_h);
    app.layers.scroll = scroll;
    app.hover_layer = None;

    let first = (scroll / theme::LAYER_ROW_HEIGHT).floor().max(0.0) as usize;
    let last = (((scroll + region.height()) / theme::LAYER_ROW_HEIGHT).ceil() as usize + 1).min(total_rows);
    let visible_slice: Vec<Row> = if first < last { app.layers.rows_cache[first..last].to_vec() } else { Vec::new() };
    let selected: HashSet<NodeId> = app.selection.iter().copied().collect();

    // Drag bookkeeping.
    let pointer = ui.input(|i| i.pointer.interact_pos());
    if let (Some(ids), Some(pos)) = (app.layers.drag.clone(), pointer) {
        app.layers.drop = if region.contains(pos) { drop_for(pos.y - region.top() + scroll, &app.layers.rows_cache, &app.doc, &ids) } else { None };
    }

    clipped(ui, region, |ui| {
        let p = ui.painter().clone();
        for (offset, row) in visible_slice.iter().copied().enumerate() {
            let i = first + offset;
            let Some(node) = app.doc.get(row.id) else { continue };
            let (node_parent, comp, icon, type_label, node_name, node_visible, node_locked) =
                (node.parent, is_component_like(node), kind_icon(&node.kind), kind_label(&node.kind), node.name.clone(), node.visible, node.locked);
            let rr = Rect::from_min_size(
                Pos2::new(region.left(), region.top() + i as f32 * theme::LAYER_ROW_HEIGHT - scroll),
                Vec2::new(region.width(), theme::LAYER_ROW_HEIGHT),
            );
            let is_sel = selected.contains(&row.id);
            let under_sel = !is_sel && !selected.is_empty() && {
                let mut c = node_parent;
                let mut f = false;
                let mut steps = 0usize;
                while let Some(pp) = c {
                    if selected.contains(&pp) {
                        f = true;
                        break;
                    }
                    steps += 1;
                    if steps >= 16 {
                        break;
                    }
                    c = app.doc.get(pp).and_then(|n| n.parent);
                }
                f
            };
            let resp = ui.interact(rr, ui.id().with(("layer", row.id.0)), Sense::click_and_drag());
            let row_hot = ui.rect_contains_pointer(rr);
            let canvas_hov = app.hover == Some(row.id);
            if resp.hovered() {
                app.hover_layer = Some(row.id);
            }
            // One shape for every row state: a pill at the frame inset (selection, hover and the tint of
            // a selected layer's children all share it).
            let pill = rr.shrink2(Vec2::new(theme::INSET_FRAME, 0.0));
            if is_sel {
                p.rect_filled(pill, theme::cr(theme::RADIUS_CONTROL), if resp.hovered() || canvas_hov { theme::BG_SELECTED_HOVER } else { theme::BG_SELECTED });
            } else if under_sel {
                p.rect_filled(pill, theme::cr(theme::RADIUS_CONTROL), theme::BG_SELECTED_SECONDARY);
                if canvas_hov {
                    p.rect_stroke(pill, theme::cr(theme::RADIUS_CONTROL), Stroke::new(1.0, theme::BORDER_SELECTED), StrokeKind::Inside);
                }
            } else if canvas_hov {
                p.rect_stroke(pill, theme::cr(theme::RADIUS_CONTROL), Stroke::new(1.0, theme::BORDER_SELECTED), StrokeKind::Inside);
            } else if resp.hovered() && app.layers.drag.is_none() {
                p.rect_stroke(pill, theme::cr(theme::RADIUS_CONTROL), Stroke::new(1.0, theme::BORDER), StrokeKind::Inside);
            }

            let indent = row.depth as f32 * theme::LAYER_INDENT;
            let chev = Rect::from_min_size(Pos2::new(rr.left() + theme::LAYER_CHEVRON_PAD + indent, rr.top() + 4.0), Vec2::splat(theme::LAYER_CHEVRON_BOX));
            if row.has_children {
                let open = app.layers.expanded.contains(&row.id);
                let cr = ui.interact(chev, ui.id().with(("chev", row.id.0)), Sense::click());
                let col = if is_sel {
                    theme::ICON_SELECTED
                } else if cr.hovered() {
                    theme::ICON
                } else {
                    theme::ICON_TERTIARY
                };
                icons::paint(&p, if open { Icon::ChevronDown } else { Icon::ChevronRight }, chev.center(), 12.0, col);
                if cr.clicked() {
                    let alt = ui.input(|i| i.modifiers.alt);
                    if alt {
                        let mut descendants = Vec::new();
                        let mut stack = vec![row.id];
                        while let Some(cur) = stack.pop() {
                            if descendants.len() >= 2000 {
                                break;
                            }
                            if let Some(n) = app.doc.get(cur) {
                                if !n.children.is_empty() {
                                    descendants.push(cur);
                                    for &ch in &n.children {
                                        stack.push(ch);
                                    }
                                }
                            }
                        }
                        if open {
                            for d in descendants {
                                app.layers.expanded.remove(&d);
                            }
                        } else {
                            for d in descendants {
                                app.layers.expanded.insert(d);
                            }
                        }
                    } else if open {
                        app.layers.expanded.remove(&row.id);
                    } else {
                        app.layers.expanded.insert(row.id);
                    }
                    app.layers.expanded_gen = app.layers.expanded_gen.wrapping_add(1);
                }
            }
            let icon_c = Pos2::new(chev.right() + theme::LAYER_ICON_GAP + theme::LAYER_ICON_SIZE / 2.0, rr.center().y);
            let icon_col = if comp {
                theme::TEXT_COMPONENT
            } else if is_sel {
                theme::ICON_SELECTED
            } else {
                theme::ICON_TERTIARY
            };
            icons::paint(&p, icon, icon_c, theme::LAYER_ICON_SIZE, icon_col);

            let text_left = icon_c.x + theme::LAYER_ICON_SIZE / 2.0 + 6.0;
            let toggles_w = theme::LAYER_TOGGLE_SIZE * 2.0 + 8.0;
            let text_right = rr.right() - toggles_w;
            let tcol = if !node_visible {
                theme::TEXT_TERTIARY
            } else if comp {
                theme::TEXT_COMPONENT
            } else {
                theme::TEXT
            };
            if app.layers.renaming == Some(row.id) {
                let id = ui.id().with(("layer-rename", row.id.0));
                let tr = Rect::from_min_max(Pos2::new(text_left - 4.0, rr.top() + 4.0), Pos2::new(rr.right() - 8.0, rr.bottom() - 4.0));
                p.rect_filled(tr, theme::cr(theme::RADIUS_MARK), theme::BG);
                p.rect_stroke(tr, theme::cr(theme::RADIUS_MARK), Stroke::new(1.0, theme::BORDER_SELECTED), StrokeKind::Inside);
                if !app.layers.rename_started {
                    ui.memory_mut(|m| m.request_focus(id));
                    app.layers.rename_started = true;
                    app.layers.rename_seen = false;
                }
                let mut tab_dir: Option<bool> = None;
                ui.input_mut(|i| {
                    if let Some(idx) = i.events.iter().position(|ev| matches!(ev, egui::Event::Key { key: egui::Key::Tab, pressed: true, .. })) {
                        if let egui::Event::Key { modifiers, .. } = i.events.remove(idx) {
                            tab_dir = Some(!modifiers.shift);
                        }
                    }
                });
                if let Some(forward) = tab_dir {
                    let cur_buf = ui.data(|d| d.get_temp::<String>(id)).unwrap_or_else(|| node_name.clone());
                    ui.data_mut(|d| d.remove_temp::<String>(id));
                    if cur_buf != node_name {
                        app.rename(row.id, &cur_buf);
                    }
                    let next_idx = if forward { (i + 1 < app.layers.rows_cache.len()).then_some(i + 1) } else { i.checked_sub(1) };
                    if let Some(ni) = next_idx.and_then(|idx| app.layers.rows_cache.get(idx).map(|r| r.id)) {
                        app.layers.renaming = Some(ni);
                        app.layers.rename_started = false;
                        app.layers.rename_seen = false;
                        app.select_only(ni);
                    } else {
                        app.layers.renaming = None;
                        app.layers.rename_started = false;
                        app.layers.rename_seen = false;
                    }
                } else {
                    let res = field::edit_text(ui, tr, id, &node_name, theme::font_body(), theme::TEXT, 4.0);
                    let focused = ui.memory(|m| m.has_focus(id));
                    if focused {
                        app.layers.rename_seen = true;
                    }
                    if let Some(s) = res {
                        app.rename(row.id, &s);
                    }
                    if !focused && app.layers.rename_seen {
                        app.layers.renaming = None;
                        app.layers.rename_started = false;
                        app.layers.rename_seen = false;
                    }
                }
            } else {
                let chip_left = crate::image_jobs::layer_row_progress_chip(app, ui, &p, rr, text_right, row.id);
                let avail_right = chip_left.unwrap_or(text_right);
                let label_w = widgets::text_width(&p, type_label, &theme::font_caption()) + TYPE_LABEL_PAD;
                let split = if chip_left.is_some() {
                    RowTextSplit { name_right: avail_right, show_label: false }
                } else {
                    split_row_text(text_left, avail_right, label_w)
                };
                let shown = widgets::elide(&p, &node_name, &theme::font_body(), (split.name_right - text_left).max(10.0));
                if let Some(m) = &filter {
                    let ranges = crate::find::filter::highlights(m, &shown);
                    crate::find::filter::paint_highlights(&p, &shown, &ranges, text_left, rr, &theme::font_body(), split.name_right);
                }
                p.text(Pos2::new(text_left, rr.center().y), Align2::LEFT_CENTER, shown, theme::font_body(), tcol);
                if split.show_label {
                    let column = Rect::from_min_max(Pos2::new(text_left, rr.top()), Pos2::new(avail_right, rr.bottom()));
                    p.with_clip_rect(column).text(
                        Pos2::new(avail_right - 2.0, rr.center().y),
                        Align2::RIGHT_CENTER,
                        type_label,
                        theme::font_caption(),
                        type_label_color(node_visible, comp),
                    );
                }
            }

            // Visibility / lock toggles (visible on hover or when active).
            let eye = Rect::from_min_size(Pos2::new(rr.right() - 8.0 - theme::LAYER_TOGGLE_SIZE, rr.top() + 4.0), Vec2::splat(theme::LAYER_TOGGLE_SIZE));
            let lock = eye.translate(Vec2::new(-theme::LAYER_TOGGLE_SIZE, 0.0));
            if row_hot || !node_visible {
                let er = ui.interact(eye, ui.id().with(("eye", row.id.0)), Sense::click());
                icons::paint(
                    &p,
                    if node_visible { Icon::Eye } else { Icon::EyeOff },
                    eye.center(),
                    14.0,
                    if er.hovered() { theme::ICON } else { theme::ICON_SECONDARY },
                );
                if er.clicked() {
                    app.toggle_visible(row.id);
                }
            }
            if row_hot || node_locked {
                let lr = ui.interact(lock, ui.id().with(("lock", row.id.0)), Sense::click());
                icons::paint(
                    &p,
                    if node_locked { Icon::Lock } else { Icon::Unlock },
                    lock.center(),
                    14.0,
                    if lr.hovered() { theme::ICON } else { theme::ICON_SECONDARY },
                );
                if lr.clicked() {
                    app.toggle_lock(row.id);
                }
            }

            if resp.secondary_clicked() {
                if !is_sel {
                    app.select_only(row.id);
                }
                app.ui.ctx_menu = ui.input(|i| i.pointer.interact_pos());
            }

            // Selection clicks.
            let mods = ui.input(|i| i.modifiers);
            if resp.double_clicked() {
                let over_icon = ui.input(|i| i.pointer.interact_pos()).is_some_and(|pos| pos.x >= chev.right() && pos.x < text_left);
                if over_icon {
                    app.select_only(row.id);
                    app.layers.anchor = Some(row.id);
                    app.fit_selection();
                } else {
                    app.layers.renaming = Some(row.id);
                    app.layers.rename_started = false;
                }
            } else if resp.clicked() && app.layers.renaming != Some(row.id) {
                if mods.shift {
                    let anchor = app.layers.anchor.and_then(|a| app.layers.rows_cache.iter().position(|r| r.id == a)).unwrap_or(i);
                    let (a, b) = (anchor.min(i), anchor.max(i));
                    if b < app.layers.rows_cache.len() {
                        app.selection = app.layers.rows_cache[a..=b].iter().map(|r| r.id).collect();
                    }
                } else if mods.command {
                    if let Some(pos) = app.selection.iter().position(|s| *s == row.id) {
                        app.selection.remove(pos);
                    } else {
                        app.selection.push(row.id);
                    }
                    app.layers.anchor = Some(row.id);
                } else {
                    app.select_only(row.id);
                    app.layers.anchor = Some(row.id);
                }
            }
            if resp.drag_started() && app.layers.renaming.is_none() && filter.is_none() {
                if !is_sel {
                    app.select_only(row.id);
                }
                app.layers.drag = Some(app.top_selection());
            }
        }

        // Drop indicator.
        if let (Some(d), Some(_)) = (app.layers.drop, app.layers.drag.as_ref()) {
            if let Some(row) = app.layers.rows_cache.get(d.row) {
                let top = region.top() + d.row as f32 * theme::LAYER_ROW_HEIGHT - scroll;
                let indent = row.depth as f32 * theme::LAYER_INDENT + theme::LAYER_CHEVRON_PAD;
                match d.kind {
                    DropKind::Inside => {
                        let r = Rect::from_min_size(Pos2::new(region.left() + 4.0, top), Vec2::new(region.width() - 8.0, theme::LAYER_ROW_HEIGHT));
                        p.rect_stroke(r, theme::cr(theme::RADIUS_MARK), Stroke::new(theme::LAYER_DROP_THICKNESS, theme::DROP_INDICATOR), StrokeKind::Inside);
                    }
                    k => {
                        let y = if k == DropKind::Before { top } else { top + theme::LAYER_ROW_HEIGHT };
                        let r = Rect::from_min_max(Pos2::new(region.left() + indent, y - 1.0), Pos2::new(region.right() - 8.0, y + 1.0));
                        p.rect_filled(r, 0.0, theme::DROP_INDICATOR);
                    }
                }
            }
        }
    });
    scrollbar(ui, region, scroll, content_h);

    // Finish drag.
    if app.layers.drag.is_some() && ui.input(|i| i.pointer.primary_released()) {
        let ids = app.layers.drag.take().unwrap_or_default();
        if let Some(d) = app.layers.drop.take() {
            if let Some((parent, index)) = resolve_drop(&app.doc, &app.layers.rows_cache, d, &ids) {
                app.move_to_parent(&ids, parent, index);
                if app.layers.expanded.insert(parent) {
                    app.layers.expanded_gen = app.layers.expanded_gen.wrapping_add(1);
                }
            }
        }
        app.layers.drop = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use designos_doc::{Node, Rect as DRect};

    fn sample() -> (Document, NodeId, NodeId, NodeId, NodeId) {
        let mut d = Document::new("t");
        let page = d.add_page("p");
        let f = d.alloc_id();
        let frame = d.insert(page, Node::frame(f, DRect::new(0.0, 0.0, 100.0, 100.0))).unwrap();
        let a = d.alloc_id();
        let a = d.insert(frame, Node::rectangle(a, DRect::new(0.0, 0.0, 10.0, 10.0))).unwrap();
        let b = d.alloc_id();
        let b = d.insert(page, Node::rectangle(b, DRect::new(0.0, 0.0, 10.0, 10.0))).unwrap();
        (d, page, frame, a, b)
    }

    #[test]
    fn flatten_lists_top_first_and_honors_expansion() {
        let (d, page, frame, a, b) = sample();
        let mut ex = HashSet::new();
        let rows = flatten(&d, page, &ex);
        assert_eq!(rows.iter().map(|r| r.id).collect::<Vec<_>>(), vec![b, frame]);
        ex.insert(frame);
        let rows = flatten(&d, page, &ex);
        assert_eq!(rows.iter().map(|r| r.id).collect::<Vec<_>>(), vec![b, frame, a]);
        assert_eq!(rows[2].depth, 1);
    }

    #[test]
    fn drop_zones() {
        let (d, page, frame, _a, b) = sample();
        let rows = flatten(&d, page, &HashSet::new());
        // row 0 = b (leaf): top half before, bottom half after
        assert_eq!(drop_for(5.0, &rows, &d, &[]), Some(Drop { row: 0, kind: DropKind::Before }));
        assert_eq!(drop_for(20.0, &rows, &d, &[]), Some(Drop { row: 0, kind: DropKind::After }));
        // row 1 = frame: middle is "inside"
        assert_eq!(drop_for(48.0, &rows, &d, &[]), Some(Drop { row: 1, kind: DropKind::Inside }));
        // can't drop onto itself
        assert_eq!(drop_for(48.0, &rows, &d, &[frame]), None);
        assert_eq!(drop_for(1000.0, &rows, &d, &[b]), Some(Drop { row: 1, kind: DropKind::After }));
    }

    #[test]
    fn resolve_inside_and_ordering() {
        let (d, page, frame, a, b) = sample();
        let rows = flatten(&d, page, &HashSet::new());
        let (p, i) = resolve_drop(&d, &rows, Drop { row: 1, kind: DropKind::Inside }, &[b]).unwrap();
        assert_eq!((p, i), (frame, 1));
        // dropping "before" the top row b puts the node above it in z-order
        let (p, i) = resolve_drop(&d, &rows, Drop { row: 0, kind: DropKind::Before }, &[a]).unwrap();
        assert_eq!(p, page);
        assert_eq!(i, 2);
        // moving b above the frame: dragged sibling before index is discounted
        let (p, i) = resolve_drop(&d, &rows, Drop { row: 1, kind: DropKind::Before }, &[b]).unwrap();
        assert_eq!((p, i), (page, 1));
    }

    #[test]
    fn kind_icon_maps_auto_layout_and_component_set() {
        use designos_doc::{AutoLayout, FrameProps, LayoutDirection};
        let plain = NodeKind::Frame(FrameProps::default());
        assert_eq!(kind_icon(&plain), Icon::Frame);

        let h_frame = FrameProps { auto_layout: Some(AutoLayout { direction: LayoutDirection::Horizontal, ..AutoLayout::default() }), ..FrameProps::default() };
        assert_eq!(kind_icon(&NodeKind::Frame(h_frame)), Icon::AutoLayoutH);

        let v_frame = FrameProps { auto_layout: Some(AutoLayout { direction: LayoutDirection::Vertical, ..AutoLayout::default() }), ..FrameProps::default() };
        assert_eq!(kind_icon(&NodeKind::Frame(v_frame)), Icon::AutoLayoutV);

        assert_eq!(kind_icon(&NodeKind::Component(FrameProps::default())), Icon::Component);
        assert_eq!(kind_icon(&NodeKind::ComponentSet(FrameProps::default())), Icon::ComponentSet);
    }

    #[test]
    fn rows_cache_not_invalidated_by_canvas_dirty_or_transform_drag() {
        let (d, _page, frame, a, b) = sample();
        let mut app = App::new(d);
        app.layers.expanded.insert(frame);
        app.layers.expanded_gen = 1;

        let rows1 = visible_rows_cached(&mut app);
        assert_eq!(rows1.iter().map(|r| r.id).collect::<Vec<_>>(), vec![b, frame, a]);
        let key1 = app.layers.cache_key.clone();
        assert!(key1.is_some());

        // Simulate dragging a node on canvas: canvas.dirty becomes true and transform changes,
        // but hierarchy is unchanged.
        app.doc.get_mut(a).unwrap().transform.tx += 42.0;
        app.canvas.dirty = true;
        let rows2 = visible_rows_cached(&mut app);
        assert_eq!(rows2.len(), 3);
        assert_eq!(app.layers.cache_key, key1, "cache_key must remain valid during transform drags");
    }

    #[test]
    fn kind_label_covers_every_node_type() {
        use designos_doc::{ArcData, AutoLayout, BooleanOp, ConnectorProps, FrameProps, LayoutDirection, ShapeWithTextProps, StickyProps, TextProps};

        let cases: &[(NodeKind, &str)] = &[
            (NodeKind::Page { background: designos_doc::Color::BLACK }, "Page"),
            (NodeKind::Frame(FrameProps::default()), "Frame"),
            (
                NodeKind::Frame(FrameProps {
                    auto_layout: Some(AutoLayout { direction: LayoutDirection::Horizontal, ..AutoLayout::default() }),
                    ..FrameProps::default()
                }),
                "Auto layout",
            ),
            (
                NodeKind::Frame(FrameProps {
                    auto_layout: Some(AutoLayout { direction: LayoutDirection::Grid, ..AutoLayout::default() }),
                    ..FrameProps::default()
                }),
                "Grid",
            ),
            (NodeKind::Group, "Group"),
            (NodeKind::Section, "Section"),
            (NodeKind::Rectangle, "Rect"),
            (NodeKind::Ellipse(ArcData::default()), "Ellipse"),
            (NodeKind::Line, "Line"),
            (NodeKind::Polygon { count: 5 }, "Polygon"),
            (NodeKind::Star { count: 5, inner_ratio: 0.5 }, "Star"),
            (NodeKind::Vector { paths: vec![], network: None }, "Vector"),
            (NodeKind::Text(TextProps::default()), "Text"),
            (NodeKind::BooleanOp(BooleanOp::Union), "Boolean"),
            (NodeKind::Component(FrameProps::default()), "Component"),
            (NodeKind::ComponentSet(FrameProps::default()), "Variant set"),
            (NodeKind::Instance { component: Some(NodeId(1)), component_key: None, frame: FrameProps::default() }, "Instance"),
            (NodeKind::Sticky(StickyProps::default()), "Sticky"),
            (NodeKind::Connector(ConnectorProps::default()), "Connector"),
            (NodeKind::ShapeWithText(ShapeWithTextProps::default()), "Shape"),
            (NodeKind::Stamp { kind: "thumbs-up".into() }, "Stamp"),
            (NodeKind::Unknown { type_name: "SLICE".into() }, "Layer"),
        ];

        for (kind, expected) in cases {
            assert_eq!(kind_label(kind), *expected, "mismatch for {kind:?}");
        }
    }

    #[test]
    fn only_rows_inside_the_viewport_paint_a_type_label() {
        let mut d = Document::new("perf");
        let page = d.add_page("Page 1");
        for idx in 0..2_000 {
            let id = d.alloc_id();
            let mut n = Node::rectangle(id, DRect::new(0.0, (idx * 12) as f32, 40.0, 10.0));
            n.name = format!("Item {idx}");
            d.insert(page, n).unwrap();
        }
        let mut app = App::new(d);
        let region = Rect::from_min_size(Pos2::new(0.0, 0.0), Vec2::new(240.0, 600.0));
        let painted = paint_panel(&mut app, region);
        let painted_rows = (region.height() / theme::LAYER_ROW_HEIGHT).ceil() as usize + 1;
        let names = painted.iter().filter(|t| t.size == theme::FONT_SIZE_BODY).count();
        let labels: Vec<&Painted> = painted.iter().filter(|t| t.size == theme::FONT_SIZE_CAPTION).collect();
        assert_eq!(names, painted_rows, "names are painted for the viewport rows only, not all 2,000");
        assert_eq!(labels.len(), painted_rows, "type labels are painted for the viewport rows only, not all 2,000");
        assert!(labels.iter().all(|t| t.text == "Rect"), "every painted label names its node type");
    }

    #[test]
    fn row_text_split_keeps_a_readable_name_or_drops_the_label() {
        // Wide column: the label takes its width and the name gets the rest.
        assert_eq!(split_row_text(50.0, 180.0, 53.0), RowTextSplit { name_right: 127.0, show_label: true });
        // Exactly the label plus the smallest readable name: the label stays.
        let tight = split_row_text(50.0, 50.0 + 53.0 + MIN_NAME_WIDTH_WITH_LABEL, 53.0);
        assert!(tight.show_label);
        assert_eq!(tight.name_right - 50.0, MIN_NAME_WIDTH_WITH_LABEL);
        // One pixel narrower: the label is dropped and the name keeps the whole column.
        assert_eq!(split_row_text(50.0, 50.0 + 53.0 + MIN_NAME_WIDTH_WITH_LABEL - 1.0, 53.0), RowTextSplit { name_right: 118.0, show_label: false });
        // A column with no room at all.
        assert!(!split_row_text(50.0, 50.0, 30.0).show_label);
    }

    /// One text run the panel painted: string, font size, logical rect, colour and the clip it was drawn under.
    struct Painted {
        text: String,
        size: f32,
        rect: Rect,
        color: egui::Color32,
        clip: Rect,
    }

    fn paint_panel(app: &mut App, region: Rect) -> Vec<Painted> {
        fn collect(shape: &egui::Shape, clip: Rect, out: &mut Vec<Painted>) {
            match shape {
                egui::Shape::Text(t) => {
                    if let Some(section) = t.galley.job.sections.first() {
                        out.push(Painted {
                            text: t.galley.job.text.clone(),
                            size: section.format.font_id.size,
                            rect: Rect::from_min_size(t.pos, t.galley.size()),
                            color: section.format.color,
                            clip,
                        });
                    }
                }
                egui::Shape::Vec(shapes) => shapes.iter().for_each(|s| collect(s, clip, out)),
                _ => {}
            }
        }
        let ctx = egui::Context::default();
        let output = ctx.run_ui(egui::RawInput::default(), |ui| show(app, ui, region));
        let mut painted = Vec::new();
        for clipped in &output.shapes {
            collect(&clipped.shape, clipped.clip_rect, &mut painted);
        }
        painted
    }

    fn row_texts(painted: &[Painted], region: Rect, row: usize) -> Vec<&Painted> {
        let center_y = region.top() + (row as f32 + 0.5) * theme::LAYER_ROW_HEIGHT;
        painted.iter().filter(|t| (t.rect.center().y - center_y).abs() < 1.0).collect()
    }

    /// A page holding a chain of `depth` plain frames with one auto-layout frame named `name` at the bottom
    /// (row `depth`), every ancestor expanded.
    fn nested_auto_layout_app(depth: usize, direction: designos_doc::LayoutDirection, name: &str) -> App {
        use designos_doc::{AutoLayout, FrameProps};
        let mut d = Document::new("nested");
        let page = d.add_page("p");
        let mut parent = page;
        let mut ancestors = Vec::new();
        for level in 0..depth {
            let id = d.alloc_id();
            let mut frame = Node::frame(id, DRect::new(0.0, 0.0, 100.0, 100.0));
            frame.name = format!("L{level}");
            parent = d.insert(parent, frame).unwrap();
            ancestors.push(parent);
        }
        let id = d.alloc_id();
        let mut leaf = Node::frame(id, DRect::new(0.0, 0.0, 50.0, 50.0));
        leaf.name = name.to_string();
        leaf.kind = NodeKind::Frame(FrameProps { auto_layout: Some(AutoLayout { direction, ..AutoLayout::default() }), ..FrameProps::default() });
        d.insert(parent, leaf).unwrap();
        let mut app = App::new(d);
        app.layers.expanded.extend(ancestors);
        app.layers.expanded_gen = 1;
        app
    }

    /// Paints an auto-layout frame at every nesting depth the default sidebar can show and calls
    /// `check(description, name, label, text_right)` for each, where `label` is `None` when the row dropped it.
    fn for_each_nested_auto_layout_row(mut check: impl FnMut(&str, &Painted, Option<&Painted>, f32)) {
        use designos_doc::LayoutDirection::{Grid, Horizontal, Vertical};
        let region = Rect::from_min_size(Pos2::ZERO, Vec2::new(theme::LEFT_PANEL_WIDTH, 600.0));
        let text_right = region.right() - (theme::LAYER_TOGGLE_SIZE * 2.0 + 8.0);
        for direction in [Horizontal, Vertical, Grid] {
            for depth in 0..=7 {
                let mut app = nested_auto_layout_app(depth, direction, "Primary navigation bar with a very long layer name");
                let painted = paint_panel(&mut app, region);
                let row = row_texts(&painted, region, depth);
                let at = format!("{direction:?} frame at depth {depth}");
                let name = row.iter().find(|t| t.size == theme::FONT_SIZE_BODY).unwrap_or_else(|| panic!("{at}: the name is not painted"));
                check(&at, name, row.iter().find(|t| t.size == theme::FONT_SIZE_CAPTION).copied(), text_right);
            }
        }
    }

    #[test]
    fn type_label_never_overlaps_the_name_or_the_layer_icon_at_any_depth() {
        let mut violations = Vec::new();
        for_each_nested_auto_layout_row(|at, name, label, text_right| {
            if name.rect.right() > text_right + 0.5 {
                violations.push(format!("{at}: name ends at {} past the text column end {text_right}", name.rect.right()));
            }
            if let Some(label) = label {
                if name.rect.right() > label.rect.left() {
                    violations.push(format!("{at}: name {:?} overlaps label {:?}", name.rect, label.rect));
                }
                if label.rect.left() - name.rect.left() < 16.0 {
                    violations.push(format!("{at}: label {:?} leaves the name under 16 px or covers the layer icon", label.rect));
                }
            }
        });
        assert!(violations.is_empty(), "{}", violations.join("\n"));
    }

    #[test]
    fn type_label_is_clipped_to_the_text_column() {
        let mut violations = Vec::new();
        for_each_nested_auto_layout_row(|at, name, label, text_right| {
            if let Some(label) = label {
                if label.rect.right() > text_right || label.clip.left() < name.rect.left() || label.clip.right() > text_right {
                    violations.push(format!(
                        "{at}: label {:?} is clipped to {:?}, wider than the text column {}..{text_right}",
                        label.rect,
                        label.clip,
                        name.rect.left()
                    ));
                }
            }
        });
        assert!(violations.is_empty(), "{}", violations.join("\n"));
    }

    #[test]
    fn type_label_stays_on_wide_rows_and_gives_way_to_the_name_on_narrow_ones() {
        use designos_doc::LayoutDirection::{Grid, Horizontal};
        let region = Rect::from_min_size(Pos2::ZERO, Vec2::new(theme::LEFT_PANEL_WIDTH, 600.0));
        let long_name = "Primary navigation bar with a very long layer name";
        let row_at = |depth: usize, direction| {
            let mut app = nested_auto_layout_app(depth, direction, long_name);
            let painted = paint_panel(&mut app, region);
            let texts = row_texts(&painted, region, depth);
            let name = texts.iter().find(|t| t.size == theme::FONT_SIZE_BODY).map(|t| t.rect).expect("name painted");
            let label = texts.iter().find(|t| t.size == theme::FONT_SIZE_CAPTION).map(|t| t.text.clone());
            (name, label)
        };
        assert_eq!(row_at(0, Horizontal).1.as_deref(), Some("Auto layout"), "a top-level row shows its type");
        assert_eq!(row_at(3, Horizontal).1.as_deref(), Some("Auto layout"), "depth 3 (82 px) still fits the label and a readable name");
        // Depth 4 leaves 66 px: less than "Auto layout" plus a 16 px name, so the name keeps the whole column.
        let (name, label) = row_at(4, Horizontal);
        assert_eq!(label, None);
        assert!(name.width() > 50.0, "the name takes the whole 66 px column, got {}", name.width());
        // Depth 5 leaves 50 px: too narrow for "Auto layout", wide enough for the shorter "Grid".
        let (name, label) = row_at(5, Horizontal);
        assert_eq!(label, None);
        assert!(name.width() > 40.0, "the name takes the whole 50 px column, got {}", name.width());
        assert_eq!(row_at(5, Grid).1.as_deref(), Some("Grid"));
        // Depth 6 leaves 34 px: the name still gets all of it, not the old 16 px floor.
        let (name, label) = row_at(6, Horizontal);
        assert_eq!(label, None);
        assert!(name.width() > 24.0, "the name takes the whole 34 px column, got {}", name.width());
    }

    #[test]
    fn hidden_layers_grey_the_type_label_with_the_name() {
        use designos_doc::FrameProps;
        let region = Rect::from_min_size(Pos2::ZERO, Vec2::new(theme::LEFT_PANEL_WIDTH, 600.0));
        let mut d = Document::new("visibility");
        let page = d.add_page("p");
        let id = d.alloc_id();
        let mut hidden = Node::rectangle(id, DRect::new(0.0, 0.0, 10.0, 10.0));
        hidden.name = "Hidden".into();
        hidden.visible = false;
        d.insert(page, hidden).unwrap();
        let id = d.alloc_id();
        let mut hidden_instance = Node::rectangle(id, DRect::new(0.0, 0.0, 10.0, 10.0));
        hidden_instance.name = "Hidden instance".into();
        hidden_instance.kind = NodeKind::Instance { component: Some(NodeId(1)), component_key: None, frame: FrameProps::default() };
        hidden_instance.visible = false;
        d.insert(page, hidden_instance).unwrap();
        let id = d.alloc_id();
        let mut shown_instance = Node::rectangle(id, DRect::new(0.0, 0.0, 10.0, 10.0));
        shown_instance.name = "Shown instance".into();
        shown_instance.kind = NodeKind::Instance { component: Some(NodeId(1)), component_key: None, frame: FrameProps::default() };
        d.insert(page, shown_instance).unwrap();
        let mut app = App::new(d);
        let painted = paint_panel(&mut app, region);
        // Rows are listed top-first: shown instance, hidden instance, hidden rectangle.
        let colors = |row: usize| {
            let texts = row_texts(&painted, region, row);
            let name = texts.iter().find(|t| t.size == theme::FONT_SIZE_BODY).expect("name painted").color;
            let label = texts.iter().find(|t| t.size == theme::FONT_SIZE_CAPTION).expect("type label painted").color;
            (name, label)
        };
        assert_eq!(colors(0), (theme::TEXT_COMPONENT, theme::TEXT_COMPONENT.gamma_multiply(0.75)), "visible instance keeps the component colours");
        assert_eq!(colors(1), (theme::TEXT_TERTIARY, theme::TEXT_TERTIARY), "hidden instance greys the name and the label together");
        assert_eq!(colors(2), (theme::TEXT_TERTIARY, theme::TEXT_TERTIARY), "hidden rectangle greys the name and the label together");
    }
}
