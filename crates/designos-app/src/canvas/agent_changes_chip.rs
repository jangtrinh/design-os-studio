//! Off-screen agent changes chip (`#695`): `"N changes -> Show"`.
//!
//! When an agent turn changes layers that lie outside the current viewport, shows a pill at the
//! canvas edge nearest to those changes instead of moving the user's view automatically. Clicking
//! the pill pans and zooms (`View::fit_with_margin(..., 0.10)`) to show the turn's changed layers
//! without modifying `app.selection`, calling `App::changed()`, or bumping `TileView::rev`.

use crate::app::{App, View};
use crate::icons::{self, Icon};
use crate::theme;
use crate::widgets;
use designos_doc::live_patch::{LivePatchReport, TouchKind};
use designos_doc::{Document, NodeId, NodeKind, Rect, Vec2 as DV};
use egui::{Color32, Id, Pos2, Rect as ERect, Sense, Stroke, StrokeKind, Ui, Vec2};

/// Maximum number of layers tracked per turn for the union bounds (`§8.8`).
pub const MAX_TURN_LAYERS: usize = 10_000;

/// Ten-percent margin on each side when fitting a turn's changed layers into the viewport (`§3.5`).
pub const SHOW_MARGIN_FRACTION: f32 = 0.10;

/// Which edge of the canvas a chip is anchored to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChipEdge {
    Left,
    Right,
    Top,
    Bottom,
}

/// One off-screen changes chip for a finished (or settled) agent turn.
#[derive(Clone, Debug, PartialEq)]
pub struct Chip {
    pub session_id: String,
    pub colour_index: usize,
    pub turn_id: u64,
    /// Distinct non-deleted layers inserted, updated, or moved during this turn (capped at `MAX_TURN_LAYERS`).
    pub layers: Vec<NodeId>,
    /// Cached number of `layers` currently outside the viewport.
    pub off_screen_count: usize,
    /// Cached page-space center of the union of off-screen layers.
    pub off_screen_center_page: DV,
}

impl Chip {
    pub fn color(&self) -> Color32 {
        theme::AGENT_COLORS[self.colour_index % theme::AGENT_COLORS.len()]
    }
}

/// Per-document state holding active off-screen change chips across agent sessions.
#[derive(Clone, Debug, Default)]
pub struct AgentChangesChips {
    pub chips: Vec<Chip>,
    /// Tracks each session's current turn number so a new turn clears the previous chip.
    session_turns: Vec<(String, u64)>,
    /// Tracks each session's accumulated layers for its active turn.
    turn_layers: Vec<(String, u64, usize, Vec<NodeId>)>,
    /// Cached `(offset_x_bits, offset_y_bits, zoom_bits, rect_w_bits, rect_h_bits)` to detect pan/zoom changes without per-frame work.
    last_view_key: Option<(u32, u32, u32, u32, u32)>,
}

/// Formats the count pill text (`"1"`, `"999+"`, `"9,999+"`).
pub fn format_count_badge(count: usize) -> String {
    if count >= MAX_TURN_LAYERS {
        "9,999+".to_string()
    } else if count > 999 {
        "999+".to_string()
    } else {
        count.to_string()
    }
}

/// True when `layer_bounds` (in page space) intersects `viewport_page` (in page space).
/// A partly visible layer counts as visible (`§3.2`).
pub fn is_layer_visible_in_page_rect(layer_bounds: Rect, viewport_page: Rect) -> bool {
    let lx1 = layer_bounds.x + layer_bounds.w.max(0.0);
    let ly1 = layer_bounds.y + layer_bounds.h.max(0.0);
    let vx1 = viewport_page.x + viewport_page.w.max(0.0);
    let vy1 = viewport_page.y + viewport_page.h.max(0.0);
    lx1 >= viewport_page.x && layer_bounds.x <= vx1 && ly1 >= viewport_page.y && layer_bounds.y <= vy1
}

/// Computes the page-space rectangle currently visible in `canvas_rect` under `view`.
pub fn viewport_page_rect(view: View, canvas_rect: ERect) -> Rect {
    let min_p = view.to_page(canvas_rect.min, canvas_rect.min);
    let max_p = view.to_page(canvas_rect.min, canvas_rect.max);
    Rect::new(min_p.x, min_p.y, (max_p.x - min_p.x).max(0.0), (max_p.y - min_p.y).max(0.0))
}

/// Unions two page-space rectangles.
fn union_rect(a: Rect, b: Rect) -> Rect {
    let x0 = a.x.min(b.x);
    let y0 = a.y.min(b.y);
    let x1 = (a.x + a.w).max(b.x + b.w);
    let y1 = (a.y + a.h).max(b.y + b.h);
    Rect::new(x0, y0, x1 - x0, y1 - y0)
}

/// Chooses the canvas edge nearest to `target_screen` and places a chip of `chip_size` inside
/// `canvas_rect` inset by `theme::INSET_PANEL`, stacked by `stack_index` with `theme::SPACE_INLINE`.
pub fn place_chip_rect(canvas_rect: ERect, target_screen: Pos2, chip_size: Vec2, stack_index: usize) -> (ChipEdge, ERect) {
    let inner = canvas_rect.shrink(theme::INSET_PANEL);
    let center = canvas_rect.center();
    let hw = (canvas_rect.width() * 0.5).max(1.0);
    let hh = (canvas_rect.height() * 0.5).max(1.0);
    let nx = (target_screen.x - center.x) / hw;
    let ny = (target_screen.y - center.y) / hh;

    let edge = if nx.abs() >= ny.abs() {
        if nx >= 0.0 {
            ChipEdge::Right
        } else {
            ChipEdge::Left
        }
    } else if ny >= 0.0 {
        ChipEdge::Bottom
    } else {
        ChipEdge::Top
    };

    let stack_step_y = (chip_size.y + theme::SPACE_INLINE) * (stack_index as f32);
    let stack_step_x = (chip_size.x + theme::SPACE_INLINE) * (stack_index as f32);

    let min_pos = match edge {
        ChipEdge::Right => {
            let x = (inner.max.x - chip_size.x).max(inner.min.x);
            let max_y = (inner.max.y - chip_size.y).max(inner.min.y);
            let base_y = (target_screen.y - chip_size.y * 0.5).clamp(inner.min.y, max_y);
            let y = (base_y + stack_step_y).min(max_y);
            Pos2::new(x, y)
        }
        ChipEdge::Left => {
            let x = inner.min.x;
            let max_y = (inner.max.y - chip_size.y).max(inner.min.y);
            let base_y = (target_screen.y - chip_size.y * 0.5).clamp(inner.min.y, max_y);
            let y = (base_y + stack_step_y).min(max_y);
            Pos2::new(x, y)
        }
        ChipEdge::Top => {
            let y = inner.min.y;
            let max_x = (inner.max.x - chip_size.x).max(inner.min.x);
            let base_x = (target_screen.x - chip_size.x * 0.5).clamp(inner.min.x, max_x);
            let x = (base_x + stack_step_x).min(max_x);
            Pos2::new(x, y)
        }
        ChipEdge::Bottom => {
            let y = (inner.max.y - chip_size.y).max(inner.min.y);
            let max_x = (inner.max.x - chip_size.x).max(inner.min.x);
            let base_x = (target_screen.x - chip_size.x * 0.5).clamp(inner.min.x, max_x);
            let x = (base_x + stack_step_x).min(max_x);
            Pos2::new(x, y)
        }
    };

    (edge, ERect::from_min_size(min_pos, chip_size))
}

fn compute_off_screen_stats(doc: &Document, layers: &[NodeId], view: View, canvas_rect: ERect) -> (Vec<NodeId>, usize, Option<DV>) {
    let vp = viewport_page_rect(view, canvas_rect);
    let mut existing = Vec::with_capacity(layers.len().min(MAX_TURN_LAYERS));
    let mut off_count = 0usize;
    let mut off_union: Option<Rect> = None;

    for &id in layers.iter().take(MAX_TURN_LAYERS) {
        let Some(node) = doc.get(id) else {
            continue;
        };
        if matches!(node.kind, NodeKind::Page { .. }) {
            continue;
        }
        existing.push(id);
        let bb = super::fast_world_aabb(doc, None, id);
        if !is_layer_visible_in_page_rect(bb, vp) {
            off_count += 1;
            off_union = Some(match off_union {
                Some(u) => union_rect(u, bb),
                None => bb,
            });
        }
    }

    let center = off_union.map(|u| DV::new(u.x + u.w * 0.5, u.y + u.h * 0.5));
    (existing, off_count, center)
}

impl AgentChangesChips {
    /// Clears all chips and turn state (called on document switch `DocEpoch` or file open).
    pub fn clear(&mut self) {
        self.chips.clear();
        self.session_turns.clear();
        self.turn_layers.clear();
        self.last_view_key = None;
    }

    /// Clears any chip and state belonging to `session_id` (called when a tab closes or next turn starts).
    #[allow(dead_code)]
    pub fn clear_session(&mut self, session_id: &str) {
        let clean = if session_id.trim().is_empty() { "default" } else { session_id.trim() };
        self.chips.retain(|c| c.session_id != clean);
        self.turn_layers.retain(|(sid, _, _, _)| sid != clean);
    }

    /// Starts a new turn for `session_id`, clearing any chip from its previous turn (`§3.6`).
    #[allow(dead_code)]
    pub fn begin_turn(&mut self, session_id: &str, turn_id: u64) {
        let clean = if session_id.trim().is_empty() { "default" } else { session_id.trim() };
        self.chips.retain(|c| c.session_id != clean);
        self.turn_layers.retain(|(sid, t, _, _)| !(sid == clean && *t != turn_id));
        if let Some(entry) = self.session_turns.iter_mut().find(|(s, _)| s == clean) {
            entry.1 = turn_id;
        } else {
            self.session_turns.push((clean.to_string(), turn_id));
        }
    }

    fn current_turn_for(&mut self, clean_id: &str, explicit_turn: Option<u64>) -> u64 {
        if let Some(t) = explicit_turn {
            if let Some(entry) = self.session_turns.iter_mut().find(|(s, _)| s == clean_id) {
                if entry.1 != t {
                    self.chips.retain(|c| c.session_id != clean_id);
                    self.turn_layers.retain(|(sid, old_t, _, _)| !(sid == clean_id && *old_t != t));
                    entry.1 = t;
                }
            } else {
                self.session_turns.push((clean_id.to_string(), t));
            }
            return t;
        }
        self.session_turns.iter().find(|(s, _)| s == clean_id).map(|(_, t)| *t).unwrap_or_else(|| {
            self.session_turns.push((clean_id.to_string(), 1));
            1
        })
    }

    /// Records the touched layers of a committed [`LivePatchReport`] for `(session_id, turn_id)`
    /// and updates the session's off-screen chip if any changed layer lies outside the viewport.
    #[allow(clippy::too_many_arguments)]
    pub fn record_patch(
        &mut self,
        session_id: &str,
        colour_index: usize,
        explicit_turn: Option<u64>,
        doc: &Document,
        report: &LivePatchReport,
        view: View,
        canvas_rect: ERect,
    ) {
        let clean_id = if session_id.trim().is_empty() { "default" } else { session_id.trim() };
        let turn_id = self.current_turn_for(clean_id, explicit_turn);

        let entry_idx = if let Some(idx) = self.turn_layers.iter().position(|(sid, t, _, _)| sid == clean_id && *t == turn_id) {
            self.turn_layers[idx].2 = colour_index;
            idx
        } else {
            self.turn_layers.push((clean_id.to_string(), turn_id, colour_index, Vec::new()));
            self.turn_layers.len() - 1
        };

        let layers = &mut self.turn_layers[entry_idx].3;
        for &(id, kind) in &report.touched {
            match kind {
                TouchKind::Inserted | TouchKind::Updated | TouchKind::Moved => {
                    if doc.get(id).is_some_and(|n| !matches!(n.kind, NodeKind::Page { .. })) && !layers.contains(&id) && layers.len() < MAX_TURN_LAYERS {
                        layers.push(id);
                    }
                }
                TouchKind::Removed => {
                    // Rule 1 & Edge case 5: removed layers are not counted; if an earlier layer in
                    // the turn was removed, drop it from the turn's layer list.
                    layers.retain(|&x| x != id);
                }
            }
        }

        self.finish_turn(clean_id, turn_id, doc, view, canvas_rect);
    }

    /// Evaluates the turn's changed layers at turn end and creates/updates or removes the chip for `(session_id, turn_id)`.
    pub fn finish_turn(&mut self, session_id: &str, turn_id: u64, doc: &Document, view: View, canvas_rect: ERect) {
        let clean_id = if session_id.trim().is_empty() { "default" } else { session_id.trim() };
        let Some(idx) = self.turn_layers.iter().position(|(sid, t, _, _)| sid == clean_id && *t == turn_id) else {
            self.chips.retain(|c| !(c.session_id == clean_id && c.turn_id == turn_id));
            return;
        };
        let colour_index = self.turn_layers[idx].2;
        let (existing, off_count, off_center) = compute_off_screen_stats(doc, &self.turn_layers[idx].3, view, canvas_rect);
        self.turn_layers[idx].3 = existing.clone();

        // Edge case 10: a second end for the same turn replaces the chip's layers; it never adds a second chip.
        self.chips.retain(|c| c.session_id != clean_id);
        if off_count > 0 {
            if let Some(center) = off_center {
                self.chips.push(Chip {
                    session_id: clean_id.to_string(),
                    colour_index,
                    turn_id,
                    layers: existing,
                    off_screen_count: off_count,
                    off_screen_center_page: center,
                });
            }
        }
    }

    /// Re-checks the active chips' layer lists when `view` or `canvas_rect` changes (`§3.3`, `§3.6`, `§11`).
    pub fn refresh_for_view(&mut self, doc: &Document, view: View, canvas_rect: ERect) {
        if self.chips.is_empty() {
            return;
        }
        let key = (view.offset.x.to_bits(), view.offset.y.to_bits(), view.zoom.to_bits(), canvas_rect.width().to_bits(), canvas_rect.height().to_bits());
        if self.last_view_key == Some(key) {
            return;
        }
        self.last_view_key = Some(key);
        self.recompute_all(doc, view, canvas_rect);
    }

    pub fn recompute_all(&mut self, doc: &Document, view: View, canvas_rect: ERect) {
        let mut kept = Vec::with_capacity(self.chips.len());
        for mut chip in self.chips.drain(..) {
            let (existing, off_count, off_center) = compute_off_screen_stats(doc, &chip.layers, view, canvas_rect);
            if off_count > 0 {
                if let Some(center) = off_center {
                    chip.layers = existing;
                    chip.off_screen_count = off_count;
                    chip.off_screen_center_page = center;
                    kept.push(chip);
                }
            }
        }
        self.chips = kept;
    }

    /// Fits `app.view` to the union of all layers changed in `chip_index`'s turn with a 10% margin,
    /// without altering `app.selection`, calling `app.changed()`, or bumping `TileView::rev`.
    pub fn activate_chip(&mut self, chip_index: usize, doc: &Document, view: &mut View, canvas_rect: ERect) -> bool {
        if chip_index >= self.chips.len() {
            return false;
        }
        let chip = self.chips.remove(chip_index);
        let mut union_all_layers: Option<Rect> = None;
        for &id in chip.layers.iter().take(MAX_TURN_LAYERS) {
            if doc.get(id).is_none() {
                continue;
            }
            let bb = super::fast_world_aabb(doc, None, id);
            union_all_layers = Some(match union_all_layers {
                Some(u) => union_rect(u, bb),
                None => bb,
            });
        }
        let Some(bounds) = union_all_layers else {
            return false;
        };
        *view = View::fit_with_margin(bounds, canvas_rect.size(), SHOW_MARGIN_FRACTION);
        self.last_view_key = None;
        self.recompute_all(doc, *view, canvas_rect);
        true
    }
}

fn hover_tone(c: Color32) -> Color32 {
    let [r, g, b, a] = c.to_srgba_unmultiplied();
    Color32::from_rgba_unmultiplied(r.saturating_add(16), g.saturating_add(16), b.saturating_add(16), a)
}

/// Paints any active off-screen agent change chips and handles click activation.
pub fn paint(app: &mut App, ui: &mut Ui, canvas_rect: ERect) {
    if app.canvas.tiles.unlimited_budget || app.agent_changes_chip.chips.is_empty() {
        return;
    }
    app.agent_changes_chip.refresh_for_view(&app.doc, app.view, canvas_rect);
    if app.agent_changes_chip.chips.is_empty() {
        return;
    }

    let origin = canvas_rect.min;
    let view = app.view;
    let mut edge_counts = [0usize; 4];
    let mut clicked_index: Option<usize> = None;

    let chip_snapshot = app.agent_changes_chip.chips.clone();
    for (idx, chip) in chip_snapshot.iter().enumerate() {
        let count_str = format_count_badge(chip.off_screen_count);
        let noun = if chip.off_screen_count == 1 { "change" } else { "changes" };
        let show_str = "Show";

        let painter = ui.painter_at(canvas_rect);
        let mono_font = theme::font_mono();
        let body_font = theme::font_caption_medium();

        let count_galley = painter.layout_no_wrap(count_str.clone(), mono_font.clone(), theme::TEXT);
        let show_galley = painter.layout_no_wrap(show_str.to_string(), body_font.clone(), theme::AGENT_LABEL_TEXT);

        let badge_h = theme::LAYER_ICON_SIZE;
        let badge_w = (count_galley.size().x + theme::INSET_FRAME).max(badge_h);
        let icon_w = theme::SPACE_GROUP;

        // Edge case 9: if the window is smaller than the full chip, elide "changes" first.
        let max_chip_w = (canvas_rect.width() - theme::INSET_PANEL * 2.0).max(theme::TARGET_MIN);
        let fixed_w = theme::INSET_FRAME * 2.0 + badge_w + theme::SPACE_FACT * 3.0 + show_galley.size().x + icon_w;
        let avail_noun_w = (max_chip_w - fixed_w).max(0.0);
        let elided_noun = if avail_noun_w <= 0.0 { String::new() } else { widgets::elide(&painter, noun, &body_font, avail_noun_w) };
        let noun_galley = painter.layout_no_wrap(elided_noun.clone(), body_font, theme::AGENT_LABEL_TEXT);
        let noun_part_w = if elided_noun.is_empty() { 0.0 } else { noun_galley.size().x + theme::SPACE_FACT };

        let pill_h = theme::CONTROL_STANDARD.max(theme::TARGET_MIN);
        let pill_w = (theme::INSET_FRAME * 2.0 + badge_w + theme::SPACE_FACT + noun_part_w + show_galley.size().x + theme::SPACE_FACT + icon_w)
            .max(theme::TARGET_MIN)
            .min(max_chip_w);

        let target_screen = view.to_screen(origin, chip.off_screen_center_page);
        let (prelim_edge, _) = place_chip_rect(canvas_rect, target_screen, Vec2::new(pill_w, pill_h), 0);
        let edge_slot = match prelim_edge {
            ChipEdge::Left => 0,
            ChipEdge::Right => 1,
            ChipEdge::Top => 2,
            ChipEdge::Bottom => 3,
        };
        let stack_idx = edge_counts[edge_slot];
        edge_counts[edge_slot] += 1;
        let (edge, pill_rect) = place_chip_rect(canvas_rect, target_screen, Vec2::new(pill_w, pill_h), stack_idx);

        let chip_id = Id::new(("agent-changes-chip", &chip.session_id, chip.turn_id));
        let resp = ui.interact(pill_rect, chip_id, Sense::click());
        if resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }

        let painter = ui.painter_at(canvas_rect);
        let fill = if resp.hovered() { hover_tone(chip.color()) } else { chip.color() };
        painter.rect_filled(pill_rect, theme::cr_pill(pill_h), fill);
        if resp.has_focus() {
            painter.rect_stroke(pill_rect, theme::cr_pill(pill_h), Stroke::new(theme::BORDER_WIDTH, theme::BORDER_SELECTED), StrokeKind::Outside);
        }

        // 1. Separate count pill (`BG_TERTIARY` fill, `font_mono`, D85 compliance: count is never inside a label).
        let mut cursor_x = pill_rect.min.x + theme::INSET_FRAME;
        let center_y = pill_rect.center().y;
        let badge_rect = ERect::from_min_size(Pos2::new(cursor_x, center_y - badge_h * 0.5), Vec2::new(badge_w, badge_h));
        painter.rect_filled(badge_rect, theme::cr_pill(badge_h), theme::BG_TERTIARY);
        painter.galley(badge_rect.center() - count_galley.size() * 0.5, count_galley, theme::TEXT);
        cursor_x = badge_rect.max.x + theme::SPACE_FACT;

        // 2. "change" / "changes"
        if !elided_noun.is_empty() {
            let noun_pos = Pos2::new(cursor_x, center_y - noun_galley.size().y * 0.5);
            cursor_x += noun_galley.size().x + theme::SPACE_FACT;
            painter.galley(noun_pos, noun_galley, theme::AGENT_LABEL_TEXT);
        }

        // 3. "Show" + directional arrow icon
        let show_pos = Pos2::new(cursor_x, center_y - show_galley.size().y * 0.5);
        cursor_x += show_galley.size().x + theme::SPACE_FACT;
        painter.galley(show_pos, show_galley, theme::AGENT_LABEL_TEXT);

        let arrow_icon = match edge {
            ChipEdge::Left => Icon::ChevronDown,
            ChipEdge::Right => Icon::ChevronRight,
            ChipEdge::Top => Icon::ChevronDown,
            ChipEdge::Bottom => Icon::ChevronDown,
        };
        let icon_center = Pos2::new(cursor_x + icon_w * 0.5, center_y);
        icons::paint(&painter, arrow_icon, icon_center, icon_w, theme::AGENT_LABEL_TEXT);

        widgets::record(ui, &format!("agent-changes-chip-{idx}"), pill_rect);
        widgets::record(ui, &format!("agent-changes-chip-{}", chip.session_id), pill_rect);

        if resp.clicked() {
            clicked_index = Some(idx);
        }
    }

    if let Some(idx) = clicked_index {
        let mut new_view = app.view;
        if app.agent_changes_chip.activate_chip(idx, &app.doc, &mut new_view, canvas_rect) {
            app.view = new_view;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::live_patch_apply::apply_live_patches;
    use designos_doc::live_patch::{DocEpoch, LiveOp, LivePatch, SiblingHint};
    use designos_doc::{Affine, Color, FrameProps, Node, Paint};
    use egui::RawInput;
    use serde_json::{Map, Value};

    fn update_name_op(id: NodeId, new_name: &str) -> LiveOp {
        let mut set = Map::new();
        set.insert("name".into(), Value::String(new_name.into()));
        LiveOp::Update { id, set, unset: Vec::new() }
    }

    fn make_doc_with_on_and_off_screen_layers() -> (Document, [NodeId; 2], [NodeId; 3]) {
        let mut doc = Document::new("Chip Test");
        let page = doc.add_page("Page 1");

        // On-screen frame at (100, 100) with 2 children
        let on_frame_id = doc.alloc_id();
        let mut on_frame = Node::new(on_frame_id, NodeKind::Frame(FrameProps::default()));
        on_frame.name = "OnScreen".into();
        on_frame.size = DV::new(300.0, 200.0);
        on_frame.transform = Affine::translate(100.0, 100.0);
        doc.insert(page, on_frame).unwrap();

        let mut on_children = [NodeId(0); 2];
        for (i, slot) in on_children.iter_mut().enumerate() {
            let cid = doc.alloc_id();
            let mut c = Node::new(cid, NodeKind::Rectangle);
            c.name = format!("On {i}");
            c.size = DV::new(60.0, 40.0);
            c.transform = Affine::translate(20.0 + (i as f32) * 80.0, 20.0);
            c.fills.push(Paint::solid(Color::rgb(100, 180, 240)));
            doc.insert(on_frame_id, c).unwrap();
            *slot = cid;
        }

        // Off-screen frame far to the right at (5000, 300) with 3 children
        let off_frame_id = doc.alloc_id();
        let mut off_frame = Node::new(off_frame_id, NodeKind::Frame(FrameProps::default()));
        off_frame.name = "OffScreen".into();
        off_frame.size = DV::new(400.0, 300.0);
        off_frame.transform = Affine::translate(5000.0, 300.0);
        doc.insert(page, off_frame).unwrap();

        let mut off_children = [NodeId(0); 3];
        for (i, slot) in off_children.iter_mut().enumerate() {
            let cid = doc.alloc_id();
            let mut c = Node::new(cid, NodeKind::Rectangle);
            c.name = format!("Off {i}");
            c.size = DV::new(80.0, 50.0);
            c.transform = Affine::translate(20.0 + (i as f32) * 100.0, 30.0);
            c.fills.push(Paint::solid(Color::rgb(240, 120, 90)));
            doc.insert(off_frame_id, c).unwrap();
            *slot = cid;
        }

        (doc, on_children, off_children)
    }

    #[test]
    fn a_turn_that_edits_an_off_screen_frame_shows_the_chip_with_the_count() {
        let (doc, _on_children, off_children) = make_doc_with_on_and_off_screen_layers();
        let mut app = App::new(doc);
        app.canvas_rect = ERect::from_min_size(Pos2::ZERO, Vec2::new(1440.0, 900.0));
        app.view = View { offset: DV::ZERO, zoom: 1.0 };

        let patches: Vec<LivePatch> =
            off_children.iter().enumerate().map(|(i, &cid)| LivePatch::new(vec![update_name_op(cid, &format!("Edited {i}"))])).collect();
        apply_live_patches(&mut app, &patches);

        assert_eq!(app.agent_changes_chip.chips.len(), 1);
        assert_eq!(app.agent_changes_chip.chips[0].off_screen_count, 3);
    }

    #[test]
    fn click_brings_the_frame_into_view_with_a_ten_percent_margin() {
        let (doc, _on_children, off_children) = make_doc_with_on_and_off_screen_layers();
        let mut app = App::new(doc);
        let canvas_rect = ERect::from_min_size(Pos2::ZERO, Vec2::new(1440.0, 900.0));
        app.canvas_rect = canvas_rect;
        app.view = View { offset: DV::ZERO, zoom: 1.0 };

        let patches: Vec<LivePatch> =
            off_children.iter().enumerate().map(|(i, &cid)| LivePatch::new(vec![update_name_op(cid, &format!("Edited {i}"))])).collect();
        apply_live_patches(&mut app, &patches);
        assert_eq!(app.agent_changes_chip.chips.len(), 1);

        let mut new_view = app.view;
        assert!(app.agent_changes_chip.activate_chip(0, &app.doc, &mut new_view, canvas_rect));
        app.view = new_view;

        // Every edited off-screen child plus 10% margin on each side must now lie inside the viewport.
        let vp = viewport_page_rect(app.view, canvas_rect);
        let mut union_bb: Option<Rect> = None;
        for &cid in &off_children {
            let bb = crate::canvas::fast_world_aabb(&app.doc, None, cid);
            assert!(is_layer_visible_in_page_rect(bb, vp), "child {cid:?} must be visible after click");
            union_bb = Some(match union_bb {
                Some(u) => union_rect(u, bb),
                None => bb,
            });
        }
        let u = union_bb.unwrap();
        let mx = u.w * SHOW_MARGIN_FRACTION;
        let my = u.h * SHOW_MARGIN_FRACTION;
        assert!(u.x - mx >= vp.x - 1e-2 && (u.x + u.w + mx) <= (vp.x + vp.w) + 1e-2);
        assert!(u.y - my >= vp.y - 1e-2 && (u.y + u.h + my) <= (vp.y + vp.h) + 1e-2);
        assert!(app.agent_changes_chip.chips.is_empty(), "chip must clear on click");
    }

    #[test]
    fn the_view_never_moves_without_a_click() {
        let (doc, _on_children, off_children) = make_doc_with_on_and_off_screen_layers();
        let mut app = App::new(doc);
        let canvas_rect = ERect::from_min_size(Pos2::ZERO, Vec2::new(1440.0, 900.0));
        app.canvas_rect = canvas_rect;
        let initial_view = View { offset: DV::new(15.0, 25.0), zoom: 1.25 };
        app.view = initial_view;

        for (i, &cid) in off_children.iter().enumerate() {
            apply_live_patches(&mut app, &[LivePatch::new(vec![update_name_op(cid, &format!("Turn {i}"))])]);
            assert_eq!(app.view.offset, initial_view.offset);
            assert_eq!(app.view.zoom, initial_view.zoom);
        }
    }

    #[test]
    fn an_on_screen_only_turn_shows_no_chip() {
        let (doc, on_children, _off_children) = make_doc_with_on_and_off_screen_layers();
        let mut app = App::new(doc);
        app.canvas_rect = ERect::from_min_size(Pos2::ZERO, Vec2::new(1440.0, 900.0));
        app.view = View { offset: DV::ZERO, zoom: 1.0 };

        let patches: Vec<LivePatch> =
            on_children.iter().enumerate().map(|(i, &cid)| LivePatch::new(vec![update_name_op(cid, &format!("Visible {i}"))])).collect();
        apply_live_patches(&mut app, &patches);

        assert!(app.agent_changes_chip.chips.is_empty(), "on-screen-only turn must show no chip");
    }

    #[test]
    fn click_changes_no_selection() {
        let (doc, on_children, off_children) = make_doc_with_on_and_off_screen_layers();
        let mut app = App::new(doc);
        let canvas_rect = ERect::from_min_size(Pos2::ZERO, Vec2::new(1440.0, 900.0));
        app.canvas_rect = canvas_rect;
        app.view = View { offset: DV::ZERO, zoom: 1.0 };

        apply_live_patches(&mut app, &[LivePatch::new(vec![update_name_op(off_children[0], "Off edit")])]);
        app.selection = vec![on_children[0]];
        let sel_before = app.selection.clone();

        let mut new_view = app.view;
        assert!(app.agent_changes_chip.activate_chip(0, &app.doc, &mut new_view, canvas_rect));
        app.view = new_view;

        assert_eq!(app.selection, sel_before, "clicking the chip must not alter selection");
    }

    #[test]
    fn chip_clears_on_next_turn_start_pan_to_visible_and_document_switch() {
        let (doc, _on_children, off_children) = make_doc_with_on_and_off_screen_layers();
        let mut app = App::new(doc);
        let canvas_rect = ERect::from_min_size(Pos2::ZERO, Vec2::new(1440.0, 900.0));
        app.canvas_rect = canvas_rect;
        app.view = View { offset: DV::ZERO, zoom: 1.0 };

        // 1. Next turn start clears the chip.
        apply_live_patches(&mut app, &[LivePatch::new(vec![update_name_op(off_children[0], "T1")]).with_agent("s1", "Claude")]);
        assert_eq!(app.agent_changes_chip.chips.len(), 1);
        app.agent_changes_chip.begin_turn("s1", 2);
        assert!(app.agent_changes_chip.chips.is_empty(), "chip must clear on next turn start");

        // 2. Panning so the layer is visible clears the chip.
        apply_live_patches(&mut app, &[LivePatch::new(vec![update_name_op(off_children[0], "T2")]).with_agent("s1", "Claude")]);
        assert_eq!(app.agent_changes_chip.chips.len(), 1);
        app.view = View { offset: DV::new(4800.0, 0.0), zoom: 1.0 };
        app.agent_changes_chip.refresh_for_view(&app.doc, app.view, canvas_rect);
        assert!(app.agent_changes_chip.chips.is_empty(), "chip must clear when user pans so changed layers are visible");

        // 3. Document switch clears the chip.
        app.view = View { offset: DV::ZERO, zoom: 1.0 };
        apply_live_patches(&mut app, &[LivePatch::new(vec![update_name_op(off_children[1], "T3")]).with_agent("s1", "Claude")]);
        assert_eq!(app.agent_changes_chip.chips.len(), 1);
        app.agent_changes_chip.clear();
        assert!(app.agent_changes_chip.chips.is_empty(), "chip must clear on document switch");
        let _ = DocEpoch::of(1, b"{}");
        let _ = SiblingHint::default();
    }

    #[test]
    fn chip_never_bumps_the_tile_revision_or_calls_changed() {
        let (doc, _on_children, off_children) = make_doc_with_on_and_off_screen_layers();
        let mut app = App::new(doc);
        let canvas_rect = ERect::from_min_size(Pos2::ZERO, Vec2::new(1440.0, 900.0));
        app.canvas_rect = canvas_rect;
        app.view = View { offset: DV::ZERO, zoom: 1.0 };

        apply_live_patches(&mut app, &[LivePatch::new(vec![update_name_op(off_children[0], "Off 0 Edited")])]);

        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        crate::theme::install_fonts(&ctx);
        let _ = ctx.run_ui(RawInput { screen_rect: Some(canvas_rect), time: Some(1.0), ..Default::default() }, |ui| {
            crate::canvas::tiles::paint(&mut app, ui, canvas_rect);
        });
        let rev_before = app.canvas.tiles.rev();
        assert!(!app.canvas.dirty);

        // Paint the chip and then activate it: neither may bump `tiles.rev()` or set `canvas.dirty`.
        let _ = ctx.run_ui(RawInput { screen_rect: Some(canvas_rect), time: Some(1.1), ..Default::default() }, |ui| {
            paint(&mut app, ui, canvas_rect);
        });
        assert_eq!(app.canvas.tiles.rev(), rev_before);
        assert!(!app.canvas.dirty);

        let mut new_view = app.view;
        assert!(app.agent_changes_chip.activate_chip(0, &app.doc, &mut new_view, canvas_rect));
        app.view = new_view;
        assert_eq!(app.canvas.tiles.rev(), rev_before);
        assert!(!app.canvas.dirty);
    }

    #[test]
    fn placement_uses_the_edge_nearest_the_changes() {
        let canvas_rect = ERect::from_min_size(Pos2::ZERO, Vec2::new(1440.0, 900.0));
        let chip_size = Vec2::new(140.0, theme::CONTROL_STANDARD);

        let (e_right, r_right) = place_chip_rect(canvas_rect, Pos2::new(3000.0, 450.0), chip_size, 0);
        assert_eq!(e_right, ChipEdge::Right);
        assert!((r_right.max.x - (canvas_rect.max.x - theme::INSET_PANEL)).abs() < 1e-4);

        let (e_left, r_left) = place_chip_rect(canvas_rect, Pos2::new(-1200.0, 450.0), chip_size, 0);
        assert_eq!(e_left, ChipEdge::Left);
        assert!((r_left.min.x - (canvas_rect.min.x + theme::INSET_PANEL)).abs() < 1e-4);

        let (e_top, r_top) = place_chip_rect(canvas_rect, Pos2::new(720.0, -900.0), chip_size, 0);
        assert_eq!(e_top, ChipEdge::Top);
        assert!((r_top.min.y - (canvas_rect.min.y + theme::INSET_PANEL)).abs() < 1e-4);

        let (e_bottom, r_bottom) = place_chip_rect(canvas_rect, Pos2::new(720.0, 2200.0), chip_size, 0);
        assert_eq!(e_bottom, ChipEdge::Bottom);
        assert!((r_bottom.max.y - (canvas_rect.max.y - theme::INSET_PANEL)).abs() < 1e-4);

        // Corner case: far bottom-right with stronger horizontal displacement goes to Right edge near bottom corner.
        let (e_corner, r_corner) = place_chip_rect(canvas_rect, Pos2::new(4000.0, 2000.0), chip_size, 0);
        assert_eq!(e_corner, ChipEdge::Right);
        assert!((r_corner.max.y - (canvas_rect.max.y - theme::INSET_PANEL)).abs() < 1e-4);
    }

    #[test]
    fn fit_with_margin_clamps_zoom_and_handles_a_point() {
        let size = Vec2::new(1440.0, 900.0);
        // Zero-size point union (0 x 0) must not produce NaN or exceed ZOOM_MAX.
        let v_point = View::fit_with_margin(Rect::new(250.0, 400.0, 0.0, 0.0), size, SHOW_MARGIN_FRACTION);
        assert!(v_point.zoom.is_finite());
        assert!(v_point.offset.x.is_finite() && v_point.offset.y.is_finite());
        assert!((theme::ZOOM_MIN..=theme::ZOOM_MAX).contains(&v_point.zoom));

        // Huge bounds must clamp at ZOOM_MIN.
        let v_huge = View::fit_with_margin(Rect::new(0.0, 0.0, 1e9, 1e9), size, SHOW_MARGIN_FRACTION);
        assert_eq!(v_huge.zoom, theme::ZOOM_MIN);
    }

    #[test]
    fn two_agents_two_chips_stack() {
        let (doc, _on_children, off_children) = make_doc_with_on_and_off_screen_layers();
        let mut app = App::new(doc);
        let canvas_rect = ERect::from_min_size(Pos2::ZERO, Vec2::new(1440.0, 900.0));
        app.canvas_rect = canvas_rect;
        app.view = View { offset: DV::ZERO, zoom: 1.0 };

        let p1 = LivePatch::new(vec![update_name_op(off_children[0], "By Claude")]).with_agent("session-claude", "Claude");
        let p2 = LivePatch::new(vec![update_name_op(off_children[1], "By Codex")]).with_agent("session-codex", "Codex");
        apply_live_patches(&mut app, &[p1, p2]);

        assert_eq!(app.agent_changes_chip.chips.len(), 2);
        assert_ne!(app.agent_changes_chip.chips[0].color(), app.agent_changes_chip.chips[1].color(), "two agents must have two distinct chip colours");

        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        crate::theme::install_fonts(&ctx);
        let _ = ctx.run_ui(RawInput { screen_rect: Some(canvas_rect), time: Some(1.0), ..Default::default() }, |ui| {
            paint(&mut app, ui, canvas_rect);
        });

        let r0 = ctx.data_mut(|d| d.get_temp::<ERect>(Id::new(("test-rect", "agent-changes-chip-0".to_string())))).expect("chip 0 rect");
        let r1 = ctx.data_mut(|d| d.get_temp::<ERect>(Id::new(("test-rect", "agent-changes-chip-1".to_string())))).expect("chip 1 rect");
        assert!(!r0.intersects(r1), "two chips at the same edge must stack without overlapping: {r0:?} vs {r1:?}");
    }
}
