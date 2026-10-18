//! "Library updates available" banner and the review list: a checkbox per
//! updated asset, then "Update selected" or "Update all".

use super::modal::{button, frame, header, kind_icon, kind_label, modal_rect};
use crate::app::App;
use crate::icons::{self, Icon};
use crate::panels::{clipped, scrollbar, wheel_scroll};
use crate::theme;
use crate::widgets;
use designos_doc::*;
use egui::{Align2, Color32, Context, Id, Key, Order, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, Vec2};

const PAD: f32 = 20.0;
const HEADER_H: f32 = 48.0;
const FOOT_H: f32 = 60.0;
const GROUP_H: f32 = 32.0;
const ROW_H: f32 = 32.0;

/// The banner: "Library updates available - Review".
pub fn show_banner(app: &mut App, ctx: &Context) {
    if !app.update_banner_visible() {
        return;
    }
    let screen = ctx.content_rect();
    let size = Vec2::new(380.0, 40.0);
    let r = Rect::from_center_size(Pos2::new(screen.center().x, screen.top() + 72.0), size);
    let mut review = false;
    let mut dismiss = false;
    egui::Area::new(Id::new("library-updates-banner")).order(Order::Tooltip).fixed_pos(r.min).constrain(false).show(ctx, |ui| {
        ui.allocate_rect(Rect::from_min_size(r.min, size), Sense::hover());
        let p = ui.painter().clone();
        widgets::elevation_400(&p, r, theme::RADIUS_FRAME);
        p.rect_filled(r, theme::cr(theme::RADIUS_FRAME), theme::BG_MENU);
        p.rect_stroke(r, theme::cr(theme::RADIUS_FRAME), Stroke::new(1.0, theme::ELEVATION_400_RING), StrokeKind::Inside);
        widgets::record(ui, "library-updates-banner", r);
        icons::paint(&p, Icon::Book, Pos2::new(r.left() + 22.0, r.center().y), 16.0, theme::ICON_SELECTED);
        p.text(Pos2::new(r.left() + 40.0, r.center().y), Align2::LEFT_CENTER, "Library updates available", theme::font_body(), Color32::WHITE);
        let rb = Rect::from_center_size(Pos2::new(r.right() - 76.0, r.center().y), Vec2::new(64.0, 26.0));
        review = button(ui, rb, "library-updates-review", "Review", true, true);
        let xb = Rect::from_center_size(Pos2::new(r.right() - 22.0, r.center().y), Vec2::splat(24.0));
        let xr = ui.interact(xb, Id::new("library-updates-dismiss"), Sense::click());
        widgets::button_bg(&p, xb, &xr, false, theme::cr(theme::RADIUS_CONTROL));
        widgets::record(ui, "library-updates-dismiss", xb);
        let c = xb.center();
        let st = Stroke::new(1.2, theme::ICON_SECONDARY);
        p.line_segment([c + Vec2::new(-3.5, -3.5), c + Vec2::new(3.5, 3.5)], st);
        p.line_segment([c + Vec2::new(-3.5, 3.5), c + Vec2::new(3.5, -3.5)], st);
        dismiss = xr.clicked();
    });
    if review {
        app.open_updates_review();
    } else if dismiss {
        app.dismiss_update_banner();
    }
}

fn checkbox(ui: &mut Ui, rect: Rect, key: &str, on: bool) -> bool {
    let r = ui.interact(rect, Id::new(("lib-check", key)), Sense::click());
    widgets::record(ui, &format!("lib-check:{key}"), rect);
    let b = Rect::from_center_size(rect.center(), Vec2::splat(14.0));
    let p = ui.painter();
    if on {
        p.rect_filled(b, theme::cr(3), theme::BG_BRAND);
        icons::paint(p, Icon::Check, b.center(), 12.0, Color32::WHITE);
    } else {
        p.rect_stroke(b, theme::cr(3), Stroke::new(1.0, if r.hovered() { theme::ICON_SECONDARY } else { theme::ICON_TERTIARY }), StrokeKind::Inside);
    }
    r.clicked()
}

pub fn show(app: &mut App, ctx: &Context) {
    if !app.libs.review {
        return;
    }
    let groups = app.pending_libraries();
    // Libraries whose only difference is a dropped asset are still shown, flagged.
    let mut all: Vec<super::PendingLib> = app.libs.pending_all();
    all.sort_by_key(|p| p.name.to_lowercase());
    let screen = ctx.content_rect();
    let rows: usize = all.iter().map(|g| g.updates.len()).sum();
    let h = HEADER_H + FOOT_H + 24.0 + all.len() as f32 * GROUP_H + rows as f32 * ROW_H + 24.0;
    let m = modal_rect(screen, 520.0, h.clamp(240.0, 560.0));
    let esc = !ctx.egui_wants_keyboard_input() && ctx.input(|i| i.key_pressed(Key::Escape));
    let mut act: Option<bool> = None;
    let mut close = frame(ctx, "library-updates-modal", m, |ui| {
        let closed = header(ui, m, "Library updates", "library-updates-close");
        let p = ui.painter().clone();
        let list = Rect::from_min_max(Pos2::new(m.left(), m.top() + HEADER_H), Pos2::new(m.right(), m.bottom() - FOOT_H));
        let content_h = all.iter().map(|g| GROUP_H + g.updates.len() as f32 * ROW_H).sum::<f32>() + 16.0;
        let mut scroll = app.libs.review_scroll;
        wheel_scroll(ui, list, &mut scroll, content_h);
        app.libs.review_scroll = scroll;
        if all.iter().all(|g| g.updates.is_empty()) {
            p.text(list.center(), Align2::CENTER_CENTER, "All libraries are up to date", theme::font_body(), theme::TEXT_TERTIARY);
        }
        let mut toggles: Vec<(String, String)> = Vec::new();
        clipped(ui, list, |ui| {
            let p = ui.painter().clone();
            let mut y = list.top() + 8.0 - scroll;
            for g in &all {
                if g.updates.is_empty() {
                    continue;
                }
                let applicable: Vec<&Update> = g.applicable().collect();
                let all_on = !applicable.is_empty() && applicable.iter().all(|u| app.libs.selected.contains(&(g.library_id.clone(), u.key.clone())));
                let hr = Rect::from_min_size(Pos2::new(list.left(), y), Vec2::new(list.width(), GROUP_H));
                icons::paint(&p, Icon::Book, Pos2::new(hr.left() + PAD + 8.0, hr.center().y), 14.0, theme::ICON_SECONDARY);
                p.text(Pos2::new(hr.left() + PAD + 24.0, hr.center().y), Align2::LEFT_CENTER, &g.name, theme::font_body_strong(), theme::TEXT);
                p.text(
                    Pos2::new(hr.right() - PAD - 28.0, hr.center().y),
                    Align2::RIGHT_CENTER,
                    format!("v{}", g.version),
                    theme::font_caption(),
                    theme::TEXT_TERTIARY,
                );
                if !applicable.is_empty() && hr.bottom() > list.top() && hr.top() < list.bottom() {
                    let cr = Rect::from_center_size(Pos2::new(hr.right() - PAD - 8.0, hr.center().y), Vec2::splat(24.0));
                    if checkbox(ui, cr, &format!("lib:{}", g.name), all_on) {
                        for u in &applicable {
                            let k = (g.library_id.clone(), u.key.clone());
                            if all_on == app.libs.selected.contains(&k) {
                                toggles.push(k);
                            }
                        }
                    }
                }
                y += GROUP_H;
                for u in &g.updates {
                    let row = Rect::from_min_size(Pos2::new(list.left(), y), Vec2::new(list.width(), ROW_H));
                    y += ROW_H;
                    if row.bottom() < list.top() || row.top() > list.bottom() {
                        continue;
                    }
                    let key = (g.library_id.clone(), u.key.clone());
                    let removed = u.change == UpdateKind::Removed;
                    let rr = ui.interact(row, Id::new(("lib-upd-row", &g.library_id, &u.key)), Sense::click());
                    widgets::record(ui, &format!("lib-upd:{}", u.name), row);
                    if rr.hovered() && !removed {
                        p.rect_filled(row.shrink2(Vec2::new(8.0, 1.0)), theme::cr(theme::RADIUS_CONTROL), theme::BG_HOVER);
                    }
                    if !removed {
                        let cr = Rect::from_center_size(Pos2::new(row.left() + PAD + 8.0, row.center().y), Vec2::splat(24.0));
                        let on = app.libs.selected.contains(&key);
                        if checkbox(ui, cr, &u.name, on) || (rr.clicked() && !cr.contains(rr.interact_pointer_pos().unwrap_or(Pos2::ZERO))) {
                            toggles.push(key.clone());
                        }
                    }
                    let col = if removed { theme::TEXT_TERTIARY } else { theme::TEXT };
                    icons::paint(
                        &p,
                        kind_icon(u.kind),
                        Pos2::new(row.left() + PAD + 36.0, row.center().y),
                        14.0,
                        if removed { theme::ICON_TERTIARY } else { theme::ICON_SECONDARY },
                    );
                    let name = widgets::elide(&p, &u.name, &theme::font_body(), row.width() - 250.0);
                    p.text(Pos2::new(row.left() + PAD + 54.0, row.center().y), Align2::LEFT_CENTER, name, theme::font_body(), col);
                    let (tag, tcol) = match u.change {
                        UpdateKind::Changed => ("Changed", theme::TEXT_SECONDARY),
                        UpdateKind::Added => ("New", theme::TEXT_BRAND),
                        UpdateKind::Removed => ("Missing from library", theme::DANGER),
                    };
                    p.text(Pos2::new(row.right() - PAD, row.center().y), Align2::RIGHT_CENTER, tag, theme::font_caption(), tcol);
                    if !removed {
                        p.text(
                            Pos2::new(row.right() - PAD - 70.0, row.center().y),
                            Align2::RIGHT_CENTER,
                            kind_label(u.kind),
                            theme::font_caption(),
                            theme::TEXT_TERTIARY,
                        );
                    }
                }
            }
        });
        for k in toggles {
            if !app.libs.selected.remove(&k) {
                app.libs.selected.insert(k);
            }
        }
        scrollbar(ui, list, scroll, content_h);

        // footer
        widgets::hline(&p, m.left(), m.right(), m.bottom() - FOOT_H, theme::BORDER);
        let n = app.libs.selected.len();
        let total: usize = groups.iter().map(|g| g.applicable().count()).sum();
        let note = if n > 0 {
            format!("{n} selected")
        } else if total > 0 {
            format!("{total} update{} available", if total == 1 { "" } else { "s" })
        } else {
            String::new()
        };
        p.text(Pos2::new(m.left() + PAD, m.bottom() - FOOT_H / 2.0), Align2::LEFT_CENTER, note, theme::font_body(), theme::TEXT_SECONDARY);
        let by = m.bottom() - FOOT_H / 2.0 - 14.0;
        let all_b = Rect::from_min_size(Pos2::new(m.right() - PAD - 84.0, by), Vec2::new(84.0, 28.0));
        let sel_b = Rect::from_min_size(Pos2::new(all_b.left() - 8.0 - 128.0, by), Vec2::new(128.0, 28.0));
        if button(ui, sel_b, "library-update-selected", "Update selected", false, n > 0) {
            act = Some(false);
        }
        if button(ui, all_b, "library-update-all", "Update all", true, total > 0) {
            act = Some(true);
        }
        if closed {
            act = None;
        }
        closed
    });
    close |= esc;
    if let Some(all_updates) = act {
        app.apply_review(all_updates);
    } else if close {
        app.close_updates_review();
    }
}
