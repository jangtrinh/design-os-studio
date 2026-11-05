//! The bar over the canvas while an old version is shown.

use crate::app::App;
use crate::comments::ui;
use crate::theme;
use crate::widgets::{self, kit};
use egui::{Align2, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, Vec2};

pub fn show(app: &mut App, ui: &mut Ui, canvas: Rect) {
    let Some(v) = app.history.viewing.as_ref() else { return };
    let (n, title) = (v.n, v.title.clone());
    let bar = Rect::from_min_size(canvas.min, Vec2::new(canvas.width(), theme::HISTORY_BANNER_H));
    // keep clicks on the bar away from the canvas underneath
    ui.interact(bar, ui.id().with("history-banner"), Sense::click_and_drag());
    let p = ui.painter().clone();
    p.rect_filled(bar, 0.0, theme::BG_SECONDARY);
    p.rect_stroke(bar, 0.0, Stroke::new(theme::BORDER_WIDTH, theme::BORDER), StrokeKind::Inside);
    widgets::record(ui, "history-banner", bar);

    let pad = 12.0;
    let h = theme::CONTROL_XS;
    let y = bar.center().y - h / 2.0;
    let back_w = ui::kit_button_width(&p, "Back to current");
    let restore_w = ui::kit_button_width(&p, "Restore this version");
    let back = Rect::from_min_size(Pos2::new(bar.right() - pad - back_w, y), Vec2::new(back_w, h));
    let restore = Rect::from_min_size(Pos2::new(back.left() - 8.0 - restore_w, y), Vec2::new(restore_w, h));
    let font = theme::font_text(theme::FONT_SIZE_TEXT_SM, theme::TextWeight::Semibold);
    let label = widgets::elide(&p, &format!("Viewing {title}"), &font, (restore.left() - 16.0 - bar.left() - pad).max(40.0));
    p.text(Pos2::new(bar.left() + pad, bar.center().y), Align2::LEFT_CENTER, label, font, theme::TEXT);
    widgets::record(ui, "history-restore", restore);
    widgets::record(ui, "history-back", back);
    // Both buttons are painted and interacted every frame, whichever one is clicked.
    let restore_clicked = kit::button(ui, restore, "banner-restore", "Restore this version", kit::Hierarchy::Primary, kit::Size::Xs, true).clicked();
    let back_clicked = kit::button(ui, back, "banner-back", "Back to current", kit::Hierarchy::Secondary, kit::Size::Xs, true).clicked();
    if restore_clicked {
        app.restore_version(n);
    } else if back_clicked {
        app.exit_version_view();
    }
}
