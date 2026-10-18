//! The bar over the canvas while an old version is shown.

use crate::app::App;
use crate::comments::ui;
use crate::theme;
use crate::widgets;
use egui::{Align2, Pos2, Rect, Sense, Ui, Vec2};

pub fn show(app: &mut App, ui: &mut Ui, canvas: Rect) {
    let Some(v) = app.history.viewing.as_ref() else { return };
    let (n, title) = (v.n, v.title.clone());
    let bar = Rect::from_min_size(canvas.min, Vec2::new(canvas.width(), theme::HISTORY_BANNER_H));
    // keep clicks on the bar away from the canvas underneath
    ui.interact(bar, ui.id().with("history-banner"), Sense::click_and_drag());
    let p = ui.painter().clone();
    p.rect_filled(bar, 0.0, theme::BG_BRAND);
    widgets::record(ui, "history-banner", bar);

    let pad = 12.0;
    let h = 24.0;
    let y = bar.center().y - h / 2.0;
    let back_w = ui::button_width(&p, "Back to current");
    let restore_w = ui::button_width(&p, "Restore this version");
    let back = Rect::from_min_size(Pos2::new(bar.right() - pad - back_w, y), Vec2::new(back_w, h));
    let restore = Rect::from_min_size(Pos2::new(back.left() - 8.0 - restore_w, y), Vec2::new(restore_w, h));
    let font = theme::font_body_strong();
    let label = widgets::elide(&p, &format!("Viewing {title}"), &font, (restore.left() - 16.0 - bar.left() - pad).max(40.0));
    p.text(Pos2::new(bar.left() + pad, bar.center().y), Align2::LEFT_CENTER, label, font, theme::TEXT);
    // white-on-blue buttons: the restore button is solid, the other an outline
    p.rect_filled(restore, theme::cr(theme::RADIUS_CONTROL), theme::TEXT);
    let r = ui.interact(restore, ui.id().with("banner-restore"), Sense::click());
    let rcol = if r.hovered() { theme::BG_BRAND_HOVER } else { theme::BG_BRAND };
    p.text(restore.center(), Align2::CENTER_CENTER, "Restore this version", theme::font_body_strong(), rcol);
    widgets::record(ui, "history-restore", restore);
    let b = ui.interact(back, ui.id().with("banner-back"), Sense::click());
    if b.hovered() {
        p.rect_filled(back, theme::cr(theme::RADIUS_CONTROL), theme::BG_BRAND_HOVER);
    }
    p.text(back.center(), Align2::CENTER_CENTER, "Back to current", theme::font_body_strong(), theme::TEXT);
    widgets::record(ui, "history-back", back);
    if r.clicked() {
        app.restore_version(n);
    } else if b.clicked() {
        app.exit_version_view();
    }
}
