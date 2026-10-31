//! Per-turn canvas edit review card rendered at the bottom of the agent panel (`#677`).

use crate::agent_turn_ledger::{RevertReport, TouchKind, TurnRecord, TurnState};
use crate::app::App;
use crate::theme;
use crate::widgets::{self, kit};
use designos_doc::NodeId;
use egui::{Align2, Id, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, Vec2};
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use std::time::Instant;

pub const MAX_PREVIEW_LAYERS: usize = 5;
const MAX_CALL_ROWS: usize = 8;
/// One call row in the expanded card. Sits on the pointer floor (24 px).
const CALL_ROW_H: f32 = theme::TARGET_MIN;
/// One changed-layer row, and the Show all / Show less toggle under the list: rows on the pointer floor.
const LAYER_ROW_H: f32 = theme::TARGET_MIN;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct TabScrollScope {
    instance_id: String,
    tab_idx: usize,
    tab_id: usize,
}

#[derive(Clone, Copy, Debug)]
struct TurnScrollEntry {
    turn_id: u64,
    created_at: Instant,
    scroll: f32,
    pin_bottom: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ActiveCardTarget {
    tab_idx: usize,
    tab_id: usize,
    turn_id: u64,
    created_at: Instant,
}

static TURN_SCROLL_STATE: LazyLock<Mutex<HashMap<TabScrollScope, TurnScrollEntry>>> = LazyLock::new(|| Mutex::new(HashMap::new()));
static ACTIVE_CARD_TARGET: LazyLock<Mutex<HashMap<String, ActiveCardTarget>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

fn tab_scroll_scope(app: &App, tab_idx: usize) -> TabScrollScope {
    let tab_id = app.terminal.tabs.get(tab_idx).map(|t| t.id).unwrap_or(usize::MAX);
    TabScrollScope { instance_id: app.tabs.instance_id.clone(), tab_idx, tab_id }
}

fn did_card_target_change(scope: &TabScrollScope, turn_id: u64, created_at: Instant) -> bool {
    let Ok(mut map) = ACTIVE_CARD_TARGET.lock() else {
        return false;
    };
    let next = ActiveCardTarget { tab_idx: scope.tab_idx, tab_id: scope.tab_id, turn_id, created_at };
    let prev = map.insert(scope.instance_id.clone(), next);
    prev.is_some_and(|p| p != next)
}

fn read_turn_scroll(scope: &TabScrollScope, turn_id: u64, created_at: Instant) -> (f32, bool) {
    let Ok(mut map) = TURN_SCROLL_STATE.lock() else {
        return (0.0, false);
    };
    match map.get(scope) {
        Some(entry) if entry.turn_id == turn_id && entry.created_at == created_at => (entry.scroll, entry.pin_bottom),
        _ => {
            map.insert(scope.clone(), TurnScrollEntry { turn_id, created_at, scroll: 0.0, pin_bottom: false });
            (0.0, false)
        }
    }
}

fn write_turn_scroll(scope: TabScrollScope, turn_id: u64, created_at: Instant, scroll: f32, pin_bottom: bool) {
    if let Ok(mut map) = TURN_SCROLL_STATE.lock() {
        map.insert(scope, TurnScrollEntry { turn_id, created_at, scroll, pin_bottom });
    }
}

fn reset_turn_scroll(scope: &TabScrollScope) {
    if let Ok(mut map) = TURN_SCROLL_STATE.lock() {
        if let Some(entry) = map.get_mut(scope) {
            entry.scroll = 0.0;
            entry.pin_bottom = false;
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TurnCardAction {
    Keep(u64),
    Revert(u64),
    SelectLayer(NodeId),
    Expand(u64),
    Collapse(u64),
}

pub fn touch_kind_label(kind: TouchKind) -> &'static str {
    match kind {
        TouchKind::Inserted => "added",
        TouchKind::Updated | TouchKind::Moved => "changed",
        TouchKind::Removed => "removed",
    }
}

pub fn touch_glyph_and_color(kind: TouchKind) -> (&'static str, egui::Color32) {
    match kind {
        TouchKind::Inserted => ("+", theme::TEXT_COMPONENT),
        TouchKind::Updated => ("~", theme::TEXT_BRAND),
        TouchKind::Moved => ("->", theme::TEXT_SECONDARY),
        TouchKind::Removed => ("-", theme::DANGER),
    }
}

/// Formats the post-revert status line shown on the card after `Revert` is clicked.
pub fn revert_status_text(report: &RevertReport) -> String {
    let mut parts = vec!["Reverted".to_string()];
    let kept = report.kept_user_changed.len();
    if kept > 0 {
        let noun = if kept == 1 { "layer" } else { "layers" };
        let names: Vec<&str> = report.kept_user_changed.iter().map(|(_, n)| n.as_str()).collect();
        parts.push(format!("Kept {kept} {noun} you changed afterwards ({})", names.join(", ")));
    }
    let extra = report.removed_with_frame.len();
    if extra > 0 {
        let noun = if extra == 1 { "child" } else { "children" };
        let names: Vec<&str> = report.removed_with_frame.iter().map(|(_, n)| n.as_str()).collect();
        parts.push(format!("{extra} {noun} removed with the frame ({})", names.join(", ")));
    }
    parts.join(" · ")
}

/// Polls any pending `turn-marks.jsonl` lines and advances heuristic turn state for `tab_idx`.
pub fn sync_tab_ledger(app: &mut App, tab_idx: usize) {
    app.terminal.poll_auto_allow_into_tab(tab_idx);
    let expected_marks_path = app.terminal.hook_paths().turn_marks();
    if app.terminal.turn_marks.path() != expected_marks_path {
        app.terminal.turn_marks = crate::agent_turn_marks::TurnMarkLog::bind(expected_marks_path);
    }
    let now = Instant::now();
    let marks = app.terminal.turn_marks.poll();
    app.terminal.ledger_for_tab_mut(tab_idx).ingest_marks(marks, now);
    let is_working = app.terminal.is_tab_working(tab_idx);
    app.terminal.ledger_for_tab_mut(tab_idx).tick(is_working, now);
}

/// Computes the height needed for the turn review card inside the agent panel, or `0.0` when
/// there is no visible turn for `tab_idx`.
pub fn desired_card_height(app: &mut App, tab_idx: usize) -> f32 {
    sync_tab_ledger(app, tab_idx);
    let now = Instant::now();
    let Some(turn) = app.terminal.ledger_for_tab(tab_idx).visible_turn(now) else {
        return 0.0;
    };
    compute_turn_card_height(turn)
}

fn call_row_plan(turn: &TurnRecord) -> (usize, bool) {
    let shown = turn.calls.len().min(MAX_CALL_ROWS);
    let more = turn.calls.len() > MAX_CALL_ROWS || turn.calls_dropped > 0;
    (shown, more)
}

fn call_block_height(turn: &TurnRecord) -> f32 {
    let (shown, more) = call_row_plan(turn);
    let total = shown + usize::from(more);
    if total == 0 {
        0.0
    } else {
        total as f32 * CALL_ROW_H + total.saturating_sub(1) as f32 * theme::SPACE_ROW + theme::SPACE_ROW
    }
}

fn compute_turn_card_height(turn: &TurnRecord) -> f32 {
    let pad = theme::INSET_FRAME;
    let calls_h = call_block_height(turn);
    match turn.state {
        TurnState::Reverted => pad * 2.0 + theme::CONTROL_XS + calls_h,
        TurnState::Active | TurnState::Completed => {
            let visible_rows = if turn.expanded { turn.touched_layers.len() } else { turn.touched_layers.len().min(MAX_PREVIEW_LAYERS) };
            let has_toggle = turn.touched_layers.len() > MAX_PREVIEW_LAYERS;
            let total_rows = visible_rows + usize::from(has_toggle);
            let rows_h = if total_rows > 0 { total_rows as f32 * LAYER_ROW_H + (total_rows.saturating_sub(1)) as f32 * theme::SPACE_ROW } else { 0.0 };
            let buttons_h = if turn.has_edits() { theme::SPACE_INLINE + theme::CONTROL_XS } else { 0.0 };
            pad * 2.0 + theme::LINE_BODY + theme::SPACE_FACT + theme::LINE_CAPTION + theme::SPACE_INLINE + rows_h + calls_h + buttons_h
        }
        TurnState::Kept => 0.0,
    }
}

/// Renders the per-turn review card inside `outer_rect` at the bottom of the agent panel.
pub fn show_in_rect(ui: &mut Ui, app: &mut App, tab_idx: usize, outer_rect: Rect) {
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(outer_rect));
    show(&mut child, app, tab_idx);
}

/// Renders the per-turn review card for `tab_idx` if it has an unreviewed or just-reverted
/// turn with ≥ 1 applied canvas edit.
pub fn show(ui: &mut Ui, app: &mut App, tab_idx: usize) {
    sync_tab_ledger(app, tab_idx);
    let now = Instant::now();
    let Some(turn_snapshot) = app.terminal.ledger_for_tab(tab_idx).visible_turn(now).cloned() else {
        return;
    };

    let desired_h = compute_turn_card_height(&turn_snapshot);
    let avail = ui.available_rect_before_wrap();
    let frame_pad = theme::INSET_FRAME * 2.0;
    let card_h = if avail.height().is_finite() {
        let max_card_h = (avail.height() - frame_pad).max(0.0);
        desired_h.min(max_card_h)
    } else {
        desired_h
    };
    let card_w = (avail.width() - frame_pad).max(120.0);
    let card_rect = Rect::from_min_size(Pos2::new(avail.left() + theme::INSET_FRAME, avail.top() + theme::INSET_FRAME), Vec2::new(card_w, card_h));
    ui.allocate_rect(Rect::from_min_size(avail.min, Vec2::new(avail.width(), card_h + frame_pad)), Sense::hover());

    let p = ui.painter().clone();
    p.rect_filled(card_rect, theme::cr(theme::RADIUS_FRAME), theme::BG_SECONDARY);
    p.rect_stroke(card_rect, theme::cr(theme::RADIUS_FRAME), Stroke::new(1.0, theme::BORDER), StrokeKind::Inside);
    widgets::record(ui, "agent-turn-card", card_rect);

    let inner = card_rect.shrink(theme::INSET_FRAME);
    let mut action: Option<TurnCardAction> = None;

    match turn_snapshot.state {
        TurnState::Reverted => {
            let report = turn_snapshot.last_revert_report.clone().unwrap_or_default();
            let row_rect = Rect::from_min_size(inner.min, Vec2::new(inner.width(), theme::CONTROL_XS));
            let dismiss_w = 60.0;
            let dismiss_rect =
                Rect::from_min_size(Pos2::new((row_rect.right() - dismiss_w).max(row_rect.left()), row_rect.top()), Vec2::new(dismiss_w, theme::CONTROL_XS));
            let dismiss_resp =
                kit::button(ui, dismiss_rect, ("agent-turn-dismiss", turn_snapshot.id), "Dismiss", kit::Hierarchy::Secondary, kit::Size::Xs, true);
            widgets::record(ui, "agent-turn-dismiss", dismiss_rect);
            widgets::labelled(ui, dismiss_resp.id, egui::accesskit::Role::Button, "Dismiss");
            if dismiss_resp.clicked() {
                action = Some(TurnCardAction::Keep(turn_snapshot.id));
            }

            let text_max_w = (dismiss_rect.left() - row_rect.left() - theme::SPACE_INLINE).max(40.0);
            draw_reverted_caption(&p, Pos2::new(row_rect.left(), row_rect.center().y), text_max_w, &report);
            let _ = draw_call_rows(ui, &p, &turn_snapshot, inner.left(), inner.width(), row_rect.bottom() + theme::SPACE_ROW, true);
        }
        TurnState::Active | TurnState::Completed => {
            draw_active_turn_card(ui, &p, app, tab_idx, inner, &turn_snapshot, &mut action);
        }
        TurnState::Kept => {}
    }

    if let Some(act) = action {
        // egui takes the caret out of the composer on a click anywhere else, and the card's buttons draw after the
        // composer: hand it back, or the next letter goes to the canvas tools (Shift+S picks Section) (`#922`).
        let caret_was_in_composer = app.terminal.composer_focused_last_frame && card_action_returns_to_composer(&act);
        if matches!(act, TurnCardAction::Expand(_) | TurnCardAction::Collapse(_) | TurnCardAction::Keep(_) | TurnCardAction::Revert(_)) {
            reset_turn_scroll(&tab_scroll_scope(app, tab_idx));
        }
        apply_card_action(app, tab_idx, act);
        if caret_was_in_composer {
            app.terminal.viewport_focused = false;
            ui.ctx().memory_mut(|m| m.request_focus(crate::agent_a11y::composer_id()));
        }
        ui.ctx().request_repaint();
    }
}

/// Revert, Keep, Dismiss and Show all are the panel's own buttons, so the user goes on typing in the composer after them.
/// Picking a layer row aims at the canvas (it selects and zooms to the layer), so the click keeps what egui did with focus.
fn card_action_returns_to_composer(action: &TurnCardAction) -> bool {
    !matches!(action, TurnCardAction::SelectLayer(_))
}

fn draw_call_rows(ui: &mut Ui, p: &egui::Painter, turn: &TurnRecord, left: f32, width: f32, mut y: f32, reverted: bool) -> f32 {
    let (shown, more) = call_row_plan(turn);
    if shown == 0 && !more {
        return y;
    }
    for (idx, row) in turn.calls.iter().take(shown).enumerate() {
        let rect = Rect::from_min_size(Pos2::new(left, y), Vec2::new(width, CALL_ROW_H));
        let label = row.card_label(reverted);
        let elided = widgets::elide(p, &label, &theme::font_caption(), (width - theme::INSET_CONTROL * 2.0).max(32.0));
        p.text(Pos2::new(rect.left() + theme::INSET_CONTROL, rect.center().y), Align2::LEFT_CENTER, elided, theme::font_caption(), theme::TEXT_SECONDARY);
        widgets::record(ui, &format!("agent-turn-call-{idx}"), rect);
        y += CALL_ROW_H + theme::SPACE_ROW;
    }
    if more {
        let hidden = turn.calls.len().saturating_sub(shown) as u64 + turn.calls_dropped;
        let rect = Rect::from_min_size(Pos2::new(left, y), Vec2::new(width, CALL_ROW_H));
        let text = format!("{hidden} more");
        p.text(Pos2::new(rect.left() + theme::INSET_CONTROL, rect.center().y), Align2::LEFT_CENTER, text, theme::font_caption(), theme::TEXT_SECONDARY);
        widgets::record(ui, "agent-turn-calls-more", rect);
        y += CALL_ROW_H + theme::SPACE_ROW;
    }
    y
}

fn draw_reverted_caption(p: &egui::Painter, pos: Pos2, max_w: f32, report: &RevertReport) {
    let kept = report.kept_user_changed.len();
    let extra = report.removed_with_frame.len();
    if kept == 0 && extra == 0 {
        let elided = widgets::elide(p, "Reverted", &theme::font_caption(), max_w);
        p.text(pos, Align2::LEFT_CENTER, elided, theme::font_caption(), theme::TEXT_SECONDARY);
        return;
    }
    let mut job = egui::text::LayoutJob::default();
    let cap_fmt = egui::TextFormat { font_id: theme::font_caption(), color: theme::TEXT_SECONDARY, ..Default::default() };
    let mono_fmt = egui::TextFormat { font_id: theme::font_mono(), color: theme::TEXT, ..Default::default() };
    job.append("Reverted", 0.0, cap_fmt.clone());
    if kept > 0 {
        job.append(" · Kept ", 0.0, cap_fmt.clone());
        job.append(&kept.to_string(), 0.0, mono_fmt.clone());
        let noun = if kept == 1 { " layer" } else { " layers" };
        let names: Vec<&str> = report.kept_user_changed.iter().map(|(_, n)| n.as_str()).collect();
        job.append(&format!("{noun} you changed afterwards ({})", names.join(", ")), 0.0, cap_fmt.clone());
    }
    if extra > 0 {
        job.append(" · ", 0.0, cap_fmt.clone());
        job.append(&extra.to_string(), 0.0, mono_fmt);
        let noun = if extra == 1 { " child" } else { " children" };
        let names: Vec<&str> = report.removed_with_frame.iter().map(|(_, n)| n.as_str()).collect();
        job.append(&format!("{noun} removed with the frame ({})", names.join(", ")), 0.0, cap_fmt);
    }
    let galley = p.layout_job(job);
    if galley.rect.width() <= max_w {
        p.galley(Pos2::new(pos.x, pos.y - galley.rect.height() * 0.5), galley, theme::TEXT_SECONDARY);
    } else {
        let full = revert_status_text(report);
        let elided = widgets::elide(p, &full, &theme::font_caption(), max_w);
        p.text(pos, Align2::LEFT_CENTER, elided, theme::font_caption(), theme::TEXT_SECONDARY);
    }
}

fn draw_active_turn_card(ui: &mut Ui, p: &egui::Painter, app: &App, tab_idx: usize, inner: Rect, turn: &TurnRecord, action: &mut Option<TurnCardAction>) {
    let fallback_cli = app.terminal.tabs.get(tab_idx).map(|t| t.preset.button_label()).unwrap_or_else(|| app.terminal.active_runtime.button_label());
    let mut y = inner.top();

    // 1. Headline: "<CLI> changed layers" in body-medium + separate count pill (`BG_TERTIARY`, `font_mono`).
    let headline = turn.headline(fallback_cli);
    let head_font = theme::font_body_medium();
    let count_str = turn.touched_layers.len().to_string();
    let pill_w = (widgets::text_width(p, &count_str, &theme::font_mono()) + 12.0).max(20.0);
    let head_max_w = (inner.width() - pill_w - theme::SPACE_INLINE).max(40.0);
    let head_elided = widgets::elide(p, &headline, &head_font, head_max_w);
    let head_cy = y + theme::LINE_BODY * 0.5;
    p.text(Pos2::new(inner.left(), head_cy), Align2::LEFT_CENTER, &head_elided, head_font.clone(), theme::TEXT);

    let head_w = widgets::text_width(p, &head_elided, &head_font);
    let pill_rect = Rect::from_center_size(
        Pos2::new((inner.left() + head_w + theme::SPACE_INLINE + pill_w * 0.5).min(inner.right() - pill_w * 0.5), head_cy),
        Vec2::new(pill_w, theme::LINE_BODY),
    );
    p.rect_filled(pill_rect, theme::cr(theme::RADIUS_FRAME), theme::BG_TERTIARY);
    p.text(pill_rect.center(), Align2::CENTER_CENTER, count_str, theme::font_mono(), theme::TEXT);
    widgets::record(ui, "agent-turn-count-pill", pill_rect);
    y += theme::LINE_BODY + theme::SPACE_FACT;

    // 2. Caption line with change summary (+ "approximate grouping" for heuristic turns).
    let sub_text = if turn.approximate { format!("{} · approximate grouping", turn.header_summary()) } else { turn.header_summary() };
    let sub_elided = widgets::elide(p, &sub_text, &theme::font_caption(), inner.width().max(40.0));
    p.text(Pos2::new(inner.left(), y + theme::LINE_CAPTION * 0.5), Align2::LEFT_CENTER, sub_elided, theme::font_caption(), theme::TEXT_SECONDARY);
    y += theme::LINE_CAPTION + theme::SPACE_INLINE;

    // 3. Scrollable list of changed layers above pinned Show all / Show less and Revert / Keep rows (`#1173`).
    let total_layers = turn.touched_layers.len();
    let layers_noun = if total_layers == 1 { "changed layer" } else { "changed layers" };
    let list_label = format!("{total_layers} {layers_noun}");
    widgets::labelled(ui, Id::new(("agent-turn-layers-region", turn.id)), egui::accesskit::Role::Region, &list_label);

    let limit = if turn.expanded { total_layers } else { total_layers.min(MAX_PREVIEW_LAYERS) };
    let has_toggle = total_layers > MAX_PREVIEW_LAYERS;
    let list_top = y;
    // No Keep/Revert row means the height omits that gap, so the anchor sits one SPACE_INLINE past the bottom and the call block lands on the bottom edge.
    let buttons_top = if turn.has_edits() { (inner.bottom() - theme::CONTROL_XS).max(list_top) } else { (inner.bottom() + theme::SPACE_INLINE).max(list_top) };
    // Height puts calls_h between the layer rows and the buttons, then SPACE_INLINE. With no calls
    // the anchor stays the button row, so the list matches the card from before auto-allow rows.
    let calls_h = call_block_height(turn);
    let rows_anchor = if calls_h > 0.0 { (buttons_top - theme::SPACE_INLINE - calls_h).max(list_top) } else { buttons_top };
    let gap_before_anchor = if calls_h > 0.0 { 0.0 } else { theme::SPACE_INLINE };
    let (list_bottom, toggle_top) = if has_toggle {
        let toggle_bottom = (rows_anchor - gap_before_anchor).max(list_top);
        let t_top = (toggle_bottom - LAYER_ROW_H).max(list_top);
        let l_bottom = (t_top - theme::SPACE_ROW).max(list_top);
        (l_bottom, t_top)
    } else {
        let l_bottom = if limit > 0 { (rows_anchor - gap_before_anchor).max(list_top) } else { list_top };
        (l_bottom, rows_anchor)
    };

    let list_rect = Rect::from_min_max(Pos2::new(inner.left(), list_top), Pos2::new(inner.right(), list_bottom));
    let row_step = LAYER_ROW_H + theme::SPACE_ROW;
    let content_h = if limit > 0 { limit as f32 * LAYER_ROW_H + (limit.saturating_sub(1)) as f32 * theme::SPACE_ROW } else { 0.0 };

    let scope = tab_scroll_scope(app, tab_idx);
    if did_card_target_change(&scope, turn.id, turn.created_at) {
        ui.ctx().input_mut(|i| i.smooth_scroll_delta = Vec2::ZERO);
    }
    let (mut scroll, mut pin_bottom) = read_turn_scroll(&scope, turn.id, turn.created_at);
    if pin_bottom {
        scroll = (content_h - list_rect.height()).max(0.0);
        if ui.rect_contains_pointer(list_rect) && ui.input(|i| i.smooth_scroll_delta.y.abs() > 0.0) {
            pin_bottom = false;
        }
    }
    crate::panels::wheel_scroll(ui, list_rect, &mut scroll, content_h);
    write_turn_scroll(scope, turn.id, turn.created_at, scroll, pin_bottom);

    crate::panels::clipped(ui, list_rect, |ui| {
        let cp = ui.painter().clone();
        for (idx, layer) in turn.touched_layers.iter().take(limit).enumerate() {
            let offset_y = idx as f32 * row_step - scroll;
            let row_y = list_rect.top() + offset_y;
            let row_rect = Rect::from_min_size(Pos2::new(list_rect.left(), row_y), Vec2::new(list_rect.width(), LAYER_ROW_H));
            if row_rect.bottom() <= list_rect.top() || row_rect.top() >= list_rect.bottom() {
                continue;
            }
            let hit_rect = row_rect.intersect(list_rect);
            let is_removed = layer.kind == TouchKind::Removed;
            let resp = ui.interact(hit_rect, Id::new(("agent-turn-layer", turn.id, layer.id.0)), Sense::click());
            let is_selected = !is_removed && app.is_selected(layer.id);
            if is_selected {
                cp.rect_filled(row_rect, theme::cr(theme::RADIUS_CONTROL), theme::BG_SELECTED);
            } else if resp.hovered() && !is_removed {
                cp.rect_filled(row_rect, theme::cr(theme::RADIUS_CONTROL), theme::BG_HOVER);
            }

            let kind_str = touch_kind_label(layer.kind);
            let kind_w = widgets::text_width(&cp, kind_str, &theme::font_caption()) + theme::SPACE_INLINE;
            let name_max_w = (row_rect.width() - theme::INSET_CONTROL * 2.0 - kind_w).max(32.0);
            let name_elided = widgets::elide(&cp, &layer.name, &theme::font_body(), name_max_w);
            cp.text(
                Pos2::new(row_rect.left() + theme::INSET_CONTROL, row_rect.center().y),
                Align2::LEFT_CENTER,
                name_elided,
                theme::font_body(),
                if is_removed { theme::TEXT_SECONDARY } else { theme::TEXT },
            );
            cp.text(
                Pos2::new(row_rect.right() - theme::INSET_CONTROL, row_rect.center().y),
                Align2::RIGHT_CENTER,
                kind_str,
                theme::font_caption(),
                theme::TEXT_SECONDARY,
            );

            widgets::record(ui, &format!("agent-turn-row-{idx}"), hit_rect);
            widgets::record(ui, &format!("agent-turn-layer-{}", layer.id.0), hit_rect);
            if !is_removed && resp.clicked() {
                *action = Some(TurnCardAction::SelectLayer(layer.id));
            }
        }
    });
    crate::panels::scrollbar(ui, list_rect, scroll, content_h);

    if has_toggle {
        let toggle_rect = Rect::from_min_size(Pos2::new(inner.left(), toggle_top), Vec2::new(inner.width(), LAYER_ROW_H));
        if turn.expanded {
            let less_id = Id::new(("agent-turn-show-less", turn.id));
            let less_resp = ui.interact(toggle_rect, less_id, Sense::click());
            if less_resp.hovered() {
                p.rect_filled(toggle_rect, theme::cr(theme::RADIUS_CONTROL), theme::BG_HOVER);
            }
            p.text(
                Pos2::new(toggle_rect.left() + theme::INSET_CONTROL, toggle_rect.center().y),
                Align2::LEFT_CENTER,
                "Show less",
                theme::font_caption_medium(),
                theme::TEXT_SECONDARY,
            );
            widgets::record(ui, "agent-turn-show-less", toggle_rect);
            widgets::labelled(ui, less_id, egui::accesskit::Role::Button, "Show less");
            if less_resp.clicked() {
                *action = Some(TurnCardAction::Collapse(turn.id));
            }
        } else {
            let more_id = Id::new(("agent-turn-show-all", turn.id));
            let more_resp = ui.interact(toggle_rect, more_id, Sense::click());
            if more_resp.hovered() {
                p.rect_filled(toggle_rect, theme::cr(theme::RADIUS_CONTROL), theme::BG_HOVER);
            }
            p.text(
                Pos2::new(toggle_rect.left() + theme::INSET_CONTROL, toggle_rect.center().y),
                Align2::LEFT_CENTER,
                "Show all",
                theme::font_caption_medium(),
                theme::TEXT_SECONDARY,
            );
            widgets::record(ui, "agent-turn-show-all", toggle_rect);
            widgets::labelled(ui, more_id, egui::accesskit::Role::Button, "Show all");
            if more_resp.clicked() {
                *action = Some(TurnCardAction::Expand(turn.id));
            }
        }
    }

    if calls_h > 0.0 {
        let _ = draw_call_rows(ui, p, turn, inner.left(), inner.width(), rows_anchor, false);
    }

    if !turn.has_edits() {
        return;
    }

    // 4. Two secondary buttons: `Revert` and `Keep`: `Button/Secondary` xs (`CONTROL_XS` high), `SPACE_INLINE` apart.
    let btn_w = ((inner.width() - theme::SPACE_INLINE) * 0.5).max(48.0);
    let revert_rect = Rect::from_min_size(Pos2::new(inner.left(), buttons_top), Vec2::new(btn_w, theme::CONTROL_XS));
    let keep_rect = Rect::from_min_size(Pos2::new(revert_rect.right() + theme::SPACE_INLINE, buttons_top), Vec2::new(btn_w, theme::CONTROL_XS));

    let revert_resp = kit::button(ui, revert_rect, ("agent-turn-revert", turn.id), "Revert", kit::Hierarchy::Secondary, kit::Size::Xs, true);
    widgets::record(ui, "agent-turn-revert", revert_rect);
    widgets::labelled(ui, revert_resp.id, egui::accesskit::Role::Button, "Revert");
    if revert_resp.clicked() {
        *action = Some(TurnCardAction::Revert(turn.id));
    }

    let keep_resp = kit::button(ui, keep_rect, ("agent-turn-keep", turn.id), "Keep", kit::Hierarchy::Secondary, kit::Size::Xs, true);
    widgets::record(ui, "agent-turn-keep", keep_rect);
    widgets::labelled(ui, keep_resp.id, egui::accesskit::Role::Button, "Keep");
    if keep_resp.clicked() {
        *action = Some(TurnCardAction::Keep(turn.id));
    }
}

pub fn apply_card_action(app: &mut App, tab_idx: usize, action: TurnCardAction) {
    match action {
        TurnCardAction::Keep(turn_id) => {
            app.terminal.ledger_for_tab_mut(tab_idx).keep_turn(turn_id);
        }
        TurnCardAction::Revert(turn_id) => {
            if crate::live_patch_apply::user_edit_in_progress(app) {
                app.terminal.pending_turn_revert = Some((tab_idx, turn_id));
                return;
            }
            execute_revert(app, tab_idx, turn_id);
        }
        TurnCardAction::SelectLayer(id) => {
            if app.doc.get(id).is_some() {
                if let Some(page) = app.doc.page_of(id) {
                    if page != app.page {
                        app.switch_page(page);
                    }
                }
                app.select_only(id);
                app.fit_selection();
            }
        }
        TurnCardAction::Expand(turn_id) => {
            if let Some(turn) = app.terminal.ledger_for_tab_mut(tab_idx).turns.iter_mut().find(|t| t.id == turn_id) {
                turn.expanded = true;
            }
        }
        TurnCardAction::Collapse(turn_id) => {
            if let Some(turn) = app.terminal.ledger_for_tab_mut(tab_idx).turns.iter_mut().find(|t| t.id == turn_id) {
                turn.expanded = false;
            }
        }
    }
}

pub fn execute_revert(app: &mut App, tab_idx: usize, turn_id: u64) {
    let res = if tab_idx < app.terminal.tabs.len() {
        app.terminal.tabs[tab_idx].turn_ledger.revert_turn(&mut app.doc, &mut app.hist, turn_id)
    } else {
        app.terminal.turn_ledger.revert_turn(&mut app.doc, &mut app.hist, turn_id)
    };
    match res {
        Ok(report) => {
            app.relayout_moved_geometry();
            app.sanitize();
            app.changed();
            app.show_toast(revert_status_text(&report), false);
        }
        Err(e) => {
            app.show_toast(format!("Turn not reverted: {e}"), true);
        }
    }
}

/// Seeds a deterministic turn-review card fixture for `--panel agents-turn-card`,
/// `agents-turn-card-collapsed`, and `agents-turn-card-small` screenshot captures (`#1173`).
pub fn seed_turn_card_fixture(ctx: &egui::Context, app: &mut App, preset: &str) {
    use designos_doc::live_patch::{LiveOp, LivePatch};
    use designos_doc::{Node, Rect as DRect};

    app.terminal.active_runtime = crate::terminal::AgentCliPreset::Claude;
    app.terminal.side_panel_pty_open = true;
    app.terminal.chat_mode = false;
    app.terminal.picker_open = false;
    let fp = app.file_path.clone();
    app.terminal.ensure_default_tab(fp.as_deref());
    let tab_idx = app.terminal.active_tab;
    if let Some(tab) = app.terminal.tabs.get_mut(tab_idx) {
        tab.preset = crate::terminal::AgentCliPreset::Claude;
        tab.title = "Claude Code #1".into();
        let mut history = String::new();
        for idx in 1..=80 {
            history.push_str(&format!("[{idx:02}] Updated layer tokens and spacing across Hero & Cards\r\n"));
        }
        if let Ok(mut parser) = tab.parser.lock() {
            parser.process(history.as_bytes());
            parser.set_scrollback(0);
        }
    }

    let row_count = if preset == "agents-turn-card-small" { 3usize } else { 82usize };
    let page = app.page;
    let mut ids = Vec::with_capacity(row_count);
    for idx in 0..row_count {
        let id = app.doc.alloc_id();
        let mut node = Node::rectangle(id, DRect::new(0.0, 0.0, 24.0, 16.0));
        node.name = format!("Layer {:02}", idx + 1);
        let _ = app.doc.insert(page, node);
        ids.push(id);
    }
    let ops: Vec<LiveOp> = ids
        .iter()
        .enumerate()
        .map(|(idx, &id)| {
            let mut set = serde_json::Map::new();
            set.insert("name".into(), format!("Row {:02} · Card Layer", idx + 1).into());
            LiveOp::Update { id, set, unset: Vec::new() }
        })
        .collect();
    crate::live_patch_apply::apply_live_patches_with_offsets(app, &[LivePatch::new(ops)], &[]);

    let expand = preset == "agents-turn-card";
    let scope = tab_scroll_scope(app, tab_idx);
    if let Some(turn) = app.terminal.ledger_for_tab_mut(tab_idx).turns.last_mut() {
        turn.cli = Some("claude".into());
        turn.expanded = expand;
        let initial_scroll = if expand { 1.0e6_f32 } else { 0.0 };
        write_turn_scroll(scope, turn.id, turn.created_at, initial_scroll, expand);
    }
    let _ = ctx;
}
