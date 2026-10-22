//! The version history panel: the current version and every saved one, grouped by day.

use super::day_label;
use crate::app::App;
use crate::comments::ui;
use crate::home::time::relative;
use crate::icons::Icon;
use crate::theme;
use crate::widgets::menu::{popup_menu, MenuItem, MenuResult};
use crate::widgets::{self, hline};
use egui::{Align2, Id, Pos2, Rect, Sense, Ui, Vec2};

const SIDE: f32 = 8.0;

enum Row<'a> {
    Day(String),
    Current,
    Version(&'a designos_library::FileVersion),
}

pub fn show(app: &mut App, ui: &mut Ui, rect: Rect) {
    let p = ui.painter().clone();
    p.rect_filled(rect, 0.0, theme::BG);
    widgets::vline(&p, rect.left(), rect.top(), rect.bottom(), theme::BORDER);
    widgets::record(ui, "history-panel", rect);
    p.text(
        Pos2::new(rect.left() + 16.0, rect.top() + theme::COMMENT_PANEL_HEADER_H / 2.0),
        Align2::LEFT_CENTER,
        "Version history",
        theme::font_large_strong(),
        theme::TEXT,
    );
    let close = Rect::from_min_size(Pos2::new(rect.right() - 8.0 - 24.0, rect.top() + theme::COMMENT_PANEL_HEADER_H / 2.0 - 12.0), Vec2::splat(24.0));
    if widgets::icon_button(ui, close, "history-close", Icon::Close, false, "Close").clicked() {
        app.toggle_version_history();
        return;
    }
    let mut y = rect.top() + theme::COMMENT_PANEL_HEADER_H;
    hline(&p, rect.left() + 1.0, rect.right(), y, theme::BORDER);
    y += 1.0;
    let x = rect.left() + 1.0 + SIDE;
    let w = rect.width() - SIDE * 2.0 - 1.0;

    let run_groups = app.hist.undo_groups_by_run();
    if !run_groups.is_empty() {
        let mut undo_target: Option<String> = None;
        y += theme::SPACE_FACT;
        for group in &run_groups {
            let Some(run_id) = group.run_id.as_deref() else { continue };
            let row_r = Rect::from_min_size(Pos2::new(x, y), Vec2::new(w, theme::CONTROL_TOOL));
            y += theme::CONTROL_TOOL;
            let dot_col = theme::owner_color_for_run(run_id);
            let dot_pos = Pos2::new(row_r.left() + theme::SPACE_INLINE, row_r.center().y);
            p.circle_filled(dot_pos, theme::AGENT_RUN_DOT_RADIUS, dot_col);
            let btn_x = row_r.right() - theme::HISTORY_RUN_TEXT_RIGHT_PAD - theme::SPACE_FACT;
            let btn_y = row_r.top() + theme::SPACE_FACT;
            let btn_r = Rect::from_min_size(Pos2::new(btn_x, btn_y), Vec2::new(theme::HISTORY_RUN_TEXT_RIGHT_PAD, theme::TARGET_MIN));
            let text_x = row_r.left() + theme::SPACE_BLOCK;
            let avail_w = (btn_r.left() - text_x - theme::SPACE_INLINE).max(theme::SPACE_BLOCK);
            let latest_label = group.labels.last().map(|s| s.as_str()).unwrap_or("");
            let step_count = group.step_count;
            let summary = if latest_label.trim().is_empty() {
                format!("{run_id} \u{00b7} {step_count} steps")
            } else {
                format!("{run_id} \u{00b7} {step_count} steps \u{00b7} {latest_label}")
            };
            let elided = widgets::elide(&p, &summary, &theme::font_caption_medium(), avail_w);
            p.text(Pos2::new(text_x, row_r.center().y), Align2::LEFT_CENTER, elided, theme::font_caption_medium(), theme::TEXT);
            let btn_id = format!("history-undo-run-{run_id}");
            if ui::button(ui, btn_r, &btn_id, "Undo run", false, true).clicked() {
                undo_target = Some(run_id.to_string());
            }
            widgets::record(ui, &format!("history-run-group-{run_id}"), row_r);
        }
        y += theme::SPACE_FACT;
        hline(&p, rect.left() + theme::BORDER_WIDTH, rect.right(), y, theme::BORDER);
        y += theme::BORDER_WIDTH;
        if let Some(target_run) = undo_target {
            crate::live_patch_apply::rollback_agent_run(app, &target_run, "undone from version history");
        }
    }

    if !app.has_history() {
        let g = p.layout("Save to library to keep version history".to_string(), theme::font_body(), theme::TEXT_SECONDARY, w - 8.0);
        p.galley(Pos2::new(x + 4.0, y + 16.0), g, theme::TEXT_SECONDARY);
        widgets::record(ui, "history-hint", Rect::from_min_size(Pos2::new(x, y + 16.0), Vec2::new(w, 20.0)));
        return;
    }
    app.refresh_versions();

    // save button
    y += 8.0;
    let viewing = app.history.viewing.is_some();
    let br = Rect::from_min_size(Pos2::new(x, y), Vec2::new(w, 28.0));
    if ui::button(ui, br, "history-save", "Save to version history\u{2026}", false, !viewing).clicked() {
        app.begin_save_version();
    }
    p.rect_stroke(br, theme::cr(theme::RADIUS_CONTROL), egui::Stroke::new(1.0, theme::BORDER), egui::StrokeKind::Inside);
    widgets::record(ui, "history-save-button", br);
    y += 28.0 + 8.0;
    hline(&p, rect.left() + 1.0, rect.right(), y, theme::BORDER);
    y += 1.0;

    // rows
    let now = crate::comments::time::now_ms();
    let versions = app.history.list.clone();
    let mut rows: Vec<Row> = vec![Row::Current];
    let mut last_day: Option<String> = None;
    for v in &versions {
        let d = day_label(now, v.created);
        if last_day.as_deref() != Some(d.as_str()) {
            rows.push(Row::Day(d.clone()));
            last_day = Some(d);
        }
        rows.push(Row::Version(v));
    }
    let row_h = |r: &Row| match r {
        Row::Day(_) => theme::HISTORY_DAY_H,
        _ => theme::HISTORY_ROW_H,
    };
    let list = Rect::from_min_max(Pos2::new(rect.left() + 1.0, y), rect.max);
    let content_h: f32 = rows.iter().map(row_h).sum::<f32>() + 8.0;
    crate::panels::wheel_scroll(ui, list, &mut app.history.scroll, content_h);
    let scroll = app.history.scroll;
    let viewing_n = app.history.viewing.as_ref().map(|v| v.n);
    let mut view: Option<u32> = None;
    let mut back = false;
    let mut menu: Option<(u32, Pos2)> = None;
    crate::panels::clipped(ui, list, |ui| {
        let p = ui.painter().clone();
        let mut ry = list.top() + 4.0 - scroll;
        for row in &rows {
            let h = row_h(row);
            let r = Rect::from_min_size(Pos2::new(x, ry), Vec2::new(w, h));
            ry += h;
            if r.bottom() < list.top() || r.top() > list.bottom() {
                continue;
            }
            match row {
                Row::Day(label) => {
                    p.text(Pos2::new(r.left() + 8.0, r.center().y + 2.0), Align2::LEFT_CENTER, label, theme::font_caption_strong(), theme::TEXT_TERTIARY);
                }
                Row::Current => {
                    let resp = ui.interact(r, ui.id().with("hist-current"), Sense::click());
                    let sel = viewing_n.is_none();
                    paint_row_bg(&p, r, sel, resp.hovered());
                    p.circle_filled(Pos2::new(r.left() + 16.0, r.center().y), 4.0, if sel { theme::BG_BRAND } else { theme::ICON_TERTIARY });
                    p.text(Pos2::new(r.left() + 32.0, r.center().y), Align2::LEFT_CENTER, "Current version", theme::font_body_strong(), theme::TEXT);
                    widgets::record(ui, "history-current", r);
                    if resp.clicked() && viewing_n.is_some() {
                        back = true;
                    }
                }
                Row::Version(v) => {
                    let resp = ui.interact(r, ui.id().with(("hist-version", v.n)), Sense::click());
                    let sel = viewing_n == Some(v.n);
                    paint_row_bg(&p, r, sel, resp.hovered());
                    p.circle_filled(Pos2::new(r.left() + 16.0, r.center().y), 4.0, if v.is_named() { theme::TEXT_BRAND } else { theme::ICON_TERTIARY });
                    let tx = r.left() + 32.0;
                    let avail = r.right() - tx - 8.0;
                    let ago = relative(now, v.created);
                    let (l1, l2) = match v.label.as_deref().filter(|l| !l.is_empty()) {
                        Some(l) => (l.to_string(), v.description.clone().filter(|d| !d.is_empty()).unwrap_or_else(|| ago.clone())),
                        None => {
                            // today: how long ago; older groups already say the day, so show the time
                            let when = if now.saturating_sub(v.created) < 86_400_000 { ago } else { super::stamp(v.created)[11..].to_string() };
                            (when, if v.auto { "Auto-saved".to_string() } else { "Saved".to_string() })
                        }
                    };
                    let l1 = widgets::elide(&p, &l1, &theme::font_body_strong(), avail);
                    let l2 = widgets::elide(&p, &l2.replace('\n', " "), &theme::font_caption(), avail);
                    p.text(Pos2::new(tx, r.top() + 14.0), Align2::LEFT_CENTER, l1, theme::font_body_strong(), theme::TEXT);
                    p.text(Pos2::new(tx, r.top() + 30.0), Align2::LEFT_CENTER, l2, theme::font_caption(), theme::TEXT_SECONDARY);
                    widgets::record(ui, &format!("history-version-{}", v.n), r);
                    if resp.clicked() {
                        view = Some(v.n);
                    }
                    if resp.secondary_clicked() {
                        menu = Some((v.n, resp.interact_pointer_pos().unwrap_or(r.center())));
                    }
                }
            }
        }
    });
    crate::panels::scrollbar(ui, list, scroll, content_h);
    if versions.is_empty() {
        let g = p.layout(
            "No saved versions yet. They are added while you edit, and when you save one yourself.".to_string(),
            theme::font_caption(),
            theme::TEXT_TERTIARY,
            w - 16.0,
        );
        let top = list.top() + theme::HISTORY_ROW_H + 8.0;
        p.galley(Pos2::new(x + 8.0, top), g, theme::TEXT_TERTIARY);
    }
    if back {
        app.exit_version_view();
    }
    if let Some(n) = view {
        app.view_version(n);
    }
    if menu.is_some() {
        app.history.menu = menu;
    }
    context_menu(app, ui.ctx());
}

fn paint_row_bg(p: &egui::Painter, r: Rect, selected: bool, hovered: bool) {
    if selected {
        p.rect_filled(r, theme::cr(theme::RADIUS_CONTROL), theme::BG_SELECTED);
    } else if hovered {
        p.rect_filled(r, theme::cr(theme::RADIUS_CONTROL), theme::BG_HOVER);
    }
}

/// Right-click menu of a version.
fn context_menu(app: &mut App, ctx: &egui::Context) {
    let Some((n, pos)) = app.history.menu else { return };
    let named = app.history.list.iter().find(|v| v.n == n).is_some_and(|v| v.label.is_some());
    let items = [
        MenuItem::new("Restore this version"),
        MenuItem::new(if named { "Rename this version" } else { "Name this version" }),
        MenuItem::new("Duplicate as new file").sep(),
        MenuItem::new("Delete"),
    ];
    match popup_menu(ctx, Id::new("history-menu"), pos, 190.0, &items, None) {
        MenuResult::Open => {}
        MenuResult::Closed => app.history.menu = None,
        MenuResult::Picked(i) => {
            app.history.menu = None;
            match i {
                0 => app.restore_version(n),
                1 => app.ask_version_name(n),
                2 => app.duplicate_version(n),
                _ => app.delete_version(n),
            }
        }
    }
}
