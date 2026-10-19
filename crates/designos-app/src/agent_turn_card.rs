//! Per-turn canvas edit review card rendered at the bottom of the agent panel (`#677`).

use crate::agent_turn_ledger::{RevertReport, TouchKind, TurnRecord, TurnState};
use crate::app::App;
use crate::theme;
use crate::widgets;
use designos_doc::NodeId;
use egui::{Align2, Id, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, Vec2};
use std::time::Instant;

pub const MAX_PREVIEW_LAYERS: usize = 5;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TurnCardAction {
    Keep(u64),
    Revert(u64),
    SelectLayer(NodeId),
    Expand(u64),
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
    let expected_marks_path = app.terminal.live_patch.path.with_file_name("turn-marks.jsonl");
    if app.terminal.turn_marks.path() != expected_marks_path {
        app.terminal.turn_marks = crate::agent_turn_marks::TurnMarkLog::bind(expected_marks_path);
    }
    let now = Instant::now();
    let marks = app.terminal.turn_marks.poll();
    if !marks.is_empty() {
        app.terminal.ledger_for_tab_mut(tab_idx).ingest_marks(marks, now);
    }
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

fn compute_turn_card_height(turn: &TurnRecord) -> f32 {
    let pad = theme::INSET_FRAME;
    match turn.state {
        TurnState::Reverted => pad * 2.0 + theme::CONTROL_DENSE,
        TurnState::Active | TurnState::Completed => {
            let visible_rows = if turn.expanded { turn.touched_layers.len() } else { turn.touched_layers.len().min(MAX_PREVIEW_LAYERS) };
            let has_show_all = !turn.expanded && turn.touched_layers.len() > MAX_PREVIEW_LAYERS;
            let total_rows = visible_rows + usize::from(has_show_all);
            let rows_h = if total_rows > 0 { total_rows as f32 * theme::CONTROL_DENSE + (total_rows.saturating_sub(1)) as f32 * theme::SPACE_ROW } else { 0.0 };
            pad * 2.0
                + theme::LINE_BODY
                + theme::SPACE_FACT
                + theme::LINE_CAPTION
                + theme::SPACE_INLINE
                + rows_h
                + theme::SPACE_INLINE
                + theme::CONTROL_STANDARD
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

    let fallback_cli = app.terminal.tabs.get(tab_idx).map(|t| t.preset.button_label()).unwrap_or_else(|| app.terminal.active_runtime.button_label());

    let card_h = compute_turn_card_height(&turn_snapshot);
    let avail = ui.available_rect_before_wrap();
    let card_w = (avail.width() - theme::INSET_FRAME * 2.0).max(120.0);
    let card_rect = Rect::from_min_size(Pos2::new(avail.left() + theme::INSET_FRAME, avail.top() + theme::INSET_FRAME), Vec2::new(card_w, card_h));
    ui.allocate_rect(Rect::from_min_size(avail.min, Vec2::new(avail.width(), card_h + theme::INSET_FRAME * 2.0)), Sense::hover());

    let p = ui.painter().clone();
    p.rect_filled(card_rect, theme::cr(theme::RADIUS_FRAME), theme::BG_SECONDARY);
    p.rect_stroke(card_rect, theme::cr(theme::RADIUS_FRAME), Stroke::new(1.0, theme::BORDER), StrokeKind::Inside);
    widgets::record(ui, "agent-turn-card", card_rect);

    let inner = card_rect.shrink(theme::INSET_FRAME);
    let mut action: Option<TurnCardAction> = None;

    match turn_snapshot.state {
        TurnState::Reverted => {
            let report = turn_snapshot.last_revert_report.clone().unwrap_or_default();
            let row_rect = Rect::from_min_size(inner.min, Vec2::new(inner.width(), theme::CONTROL_DENSE));
            let dismiss_w = 60.0;
            let dismiss_rect =
                Rect::from_min_size(Pos2::new((row_rect.right() - dismiss_w).max(row_rect.left()), row_rect.top()), Vec2::new(dismiss_w, theme::CONTROL_DENSE));
            let dismiss_resp = ui.interact(dismiss_rect, Id::new(("agent-turn-dismiss", turn_snapshot.id)), Sense::click());
            p.rect_filled(dismiss_rect, theme::cr(theme::RADIUS_CONTROL), if dismiss_resp.hovered() { theme::BG_HOVER } else { theme::BG_TERTIARY });
            p.rect_stroke(dismiss_rect, theme::cr(theme::RADIUS_CONTROL), Stroke::new(1.0, theme::BORDER), StrokeKind::Inside);
            p.text(dismiss_rect.center(), Align2::CENTER_CENTER, "Dismiss", theme::font_caption_medium(), theme::TEXT_SECONDARY);
            widgets::record(ui, "agent-turn-dismiss", dismiss_rect);
            if dismiss_resp.clicked() {
                action = Some(TurnCardAction::Keep(turn_snapshot.id));
            }

            let text_max_w = (dismiss_rect.left() - row_rect.left() - theme::SPACE_INLINE).max(40.0);
            draw_reverted_caption(&p, Pos2::new(row_rect.left(), row_rect.center().y), text_max_w, &report);
        }
        TurnState::Active | TurnState::Completed => {
            draw_active_turn_card(ui, &p, app, inner, &turn_snapshot, fallback_cli, &mut action);
        }
        TurnState::Kept => {}
    }

    if let Some(act) = action {
        apply_card_action(app, tab_idx, act);
    }
}

fn draw_reverted_caption(p: &egui::Painter, pos: Pos2, max_w: f32, report: &RevertReport) {
    let kept = report.kept_user_changed.len();
    if kept == 0 && report.removed_with_frame.is_empty() {
        let elided = widgets::elide(p, "Reverted", &theme::font_caption(), max_w);
        p.text(pos, Align2::LEFT_CENTER, elided, theme::font_caption(), theme::TEXT_SECONDARY);
        return;
    }
    let mut job = egui::text::LayoutJob::default();
    job.append("Reverted · Kept ", 0.0, egui::TextFormat { font_id: theme::font_caption(), color: theme::TEXT_SECONDARY, ..Default::default() });
    job.append(&kept.to_string(), 0.0, egui::TextFormat { font_id: theme::font_mono(), color: theme::TEXT, ..Default::default() });
    let noun = if kept == 1 { " layer you changed afterwards" } else { " layers you changed afterwards" };
    job.append(noun, 0.0, egui::TextFormat { font_id: theme::font_caption(), color: theme::TEXT_SECONDARY, ..Default::default() });
    let galley = p.layout_job(job);
    if galley.rect.width() <= max_w {
        p.galley(Pos2::new(pos.x, pos.y - galley.rect.height() * 0.5), galley, theme::TEXT_SECONDARY);
    } else {
        let full = revert_status_text(report);
        let elided = widgets::elide(p, &full, &theme::font_caption(), max_w);
        p.text(pos, Align2::LEFT_CENTER, elided, theme::font_caption(), theme::TEXT_SECONDARY);
    }
}

fn draw_active_turn_card(ui: &mut Ui, p: &egui::Painter, app: &App, inner: Rect, turn: &TurnRecord, fallback_cli: &str, action: &mut Option<TurnCardAction>) {
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

    // 3. Collapsible list of changed layers (first 5 visible, then "Show all"), each row `CONTROL_DENSE` high.
    let limit = if turn.expanded { turn.touched_layers.len() } else { turn.touched_layers.len().min(MAX_PREVIEW_LAYERS) };
    for (idx, layer) in turn.touched_layers.iter().take(limit).enumerate() {
        let row_rect = Rect::from_min_size(Pos2::new(inner.left(), y), Vec2::new(inner.width(), theme::CONTROL_DENSE));
        let is_removed = layer.kind == TouchKind::Removed;
        let resp = ui.interact(row_rect, Id::new(("agent-turn-layer", turn.id, layer.id.0)), Sense::click());
        let is_selected = !is_removed && app.is_selected(layer.id);
        if is_selected {
            p.rect_filled(row_rect, theme::cr(theme::RADIUS_CONTROL), theme::BG_SELECTED);
        } else if resp.hovered() && !is_removed {
            p.rect_filled(row_rect, theme::cr(theme::RADIUS_CONTROL), theme::BG_HOVER);
        }

        let kind_str = touch_kind_label(layer.kind);
        let kind_w = widgets::text_width(p, kind_str, &theme::font_caption()) + theme::SPACE_INLINE;
        let name_max_w = (row_rect.width() - theme::INSET_CONTROL * 2.0 - kind_w).max(32.0);
        let name_elided = widgets::elide(p, &layer.name, &theme::font_body(), name_max_w);
        p.text(
            Pos2::new(row_rect.left() + theme::INSET_CONTROL, row_rect.center().y),
            Align2::LEFT_CENTER,
            name_elided,
            theme::font_body(),
            if is_removed { theme::TEXT_SECONDARY } else { theme::TEXT },
        );
        p.text(
            Pos2::new(row_rect.right() - theme::INSET_CONTROL, row_rect.center().y),
            Align2::RIGHT_CENTER,
            kind_str,
            theme::font_caption(),
            theme::TEXT_SECONDARY,
        );

        widgets::record(ui, &format!("agent-turn-row-{idx}"), row_rect);
        widgets::record(ui, &format!("agent-turn-layer-{}", layer.id.0), row_rect);
        if !is_removed && resp.clicked() {
            *action = Some(TurnCardAction::SelectLayer(layer.id));
        }
        y += theme::CONTROL_DENSE + theme::SPACE_ROW;
    }

    let remaining = turn.touched_layers.len().saturating_sub(limit);
    if remaining > 0 {
        let more_rect = Rect::from_min_size(Pos2::new(inner.left(), y), Vec2::new(inner.width(), theme::CONTROL_DENSE));
        let more_resp = ui.interact(more_rect, Id::new(("agent-turn-show-all", turn.id)), Sense::click());
        if more_resp.hovered() {
            p.rect_filled(more_rect, theme::cr(theme::RADIUS_CONTROL), theme::BG_HOVER);
        }
        p.text(
            Pos2::new(more_rect.left() + theme::INSET_CONTROL, more_rect.center().y),
            Align2::LEFT_CENTER,
            "Show all",
            theme::font_caption_medium(),
            theme::TEXT_SECONDARY,
        );
        widgets::record(ui, "agent-turn-show-all", more_rect);
        if more_resp.clicked() {
            *action = Some(TurnCardAction::Expand(turn.id));
        }
        y += theme::CONTROL_DENSE + theme::SPACE_ROW;
    }

    if limit > 0 || remaining > 0 {
        y = (y - theme::SPACE_ROW) + theme::SPACE_INLINE;
    }

    // 4. Two secondary buttons: `Revert` and `Keep`, `CONTROL_STANDARD` high, `RADIUS_CONTROL`, `SPACE_INLINE` apart.
    let btn_w = ((inner.width() - theme::SPACE_INLINE) * 0.5).max(48.0);
    let revert_rect = Rect::from_min_size(Pos2::new(inner.left(), y), Vec2::new(btn_w, theme::CONTROL_STANDARD));
    let keep_rect = Rect::from_min_size(Pos2::new(revert_rect.right() + theme::SPACE_INLINE, y), Vec2::new(btn_w, theme::CONTROL_STANDARD));

    let revert_resp = ui.interact(revert_rect, Id::new(("agent-turn-revert", turn.id)), Sense::click());
    p.rect_filled(revert_rect, theme::cr(theme::RADIUS_CONTROL), if revert_resp.hovered() { theme::BG_HOVER } else { theme::BG_TERTIARY });
    p.rect_stroke(revert_rect, theme::cr(theme::RADIUS_CONTROL), Stroke::new(1.0, theme::BORDER), StrokeKind::Inside);
    p.text(revert_rect.center(), Align2::CENTER_CENTER, "Revert", theme::font_body_medium(), theme::TEXT);
    widgets::record(ui, "agent-turn-revert", revert_rect);
    if revert_resp.clicked() {
        *action = Some(TurnCardAction::Revert(turn.id));
    }

    let keep_resp = ui.interact(keep_rect, Id::new(("agent-turn-keep", turn.id)), Sense::click());
    p.rect_filled(keep_rect, theme::cr(theme::RADIUS_CONTROL), if keep_resp.hovered() { theme::BG_HOVER } else { theme::BG_TERTIARY });
    p.rect_stroke(keep_rect, theme::cr(theme::RADIUS_CONTROL), Stroke::new(1.0, theme::BORDER), StrokeKind::Inside);
    p.text(keep_rect.center(), Align2::CENTER_CENTER, "Keep", theme::font_body_medium(), theme::TEXT);
    widgets::record(ui, "agent-turn-keep", keep_rect);
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
                app.changed();
            }
        }
        TurnCardAction::Expand(turn_id) => {
            if let Some(turn) = app.terminal.ledger_for_tab_mut(tab_idx).turns.iter_mut().find(|t| t.id == turn_id) {
                turn.expanded = true;
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
            app.show_toast(format!("Revert failed: {e}"), true);
        }
    }
}
