//! Top-centre canvas banner shown when the user selects or edits inside a frame an agent is
//! working on (`#693` — `"<agent name> is working here"` with `"Make a copy"` and `"Edit anyway"`).
//!
//! Layout (`docs/specs/v0.6/693-user-edits-inside-a-frame-an-agent-is-working-on.md` §7):
//! - Floating pill at the top centre of the canvas, `RADIUS_FRAME`, fill `BG_SECONDARY`,
//!   `BORDER` hairline, inset `INSET_FRAME`, `SPACE_INLINE` (`8 px`) between parts, inside the
//!   canvas rect at `INSET_PANEL` below its top edge and clear of the ruler strip.
//! - Left: an 8 px dot (`SPACE_INLINE`) in the agent's colour (`#682` token), then the sentence
//!   `"<agent name> is working here"` in body type (`font_body_medium`), `TEXT`, elided to fit.
//! - Right: two buttons of `CONTROL_STANDARD` (`28 px`), `RADIUS_CONTROL`:
//!   - `"Make a copy"` (primary action): `BG_BRAND` fill with `AGENT_LABEL_TEXT` dark label.
//!   - `"Edit anyway"` (neutral action): `BG_TERTIARY` fill with `TEXT` label.
//! - No animation, no timer; repaints on events only.

use crate::app::App;
use crate::theme;
use crate::widgets;
use egui::{Id, Pos2, Rect as ERect, Sense, Stroke, StrokeKind, Ui, Vec2};

/// Paints the top-centre busy-frame banner when the user selects or edits inside a busy frame,
/// and handles `"Make a copy"` and `"Edit anyway"` clicks (`#693`).
pub fn paint(app: &mut App, ui: &mut Ui, canvas_rect: ERect) {
    if app.canvas.tiles.unlimited_budget {
        return;
    }

    app.sync_busy_frame_banner();
    let Some(info) = app.agent_busy_frames.active_banner.clone() else {
        return;
    };

    let painter = ui.painter_at(canvas_rect);
    let body_font = theme::font_body_medium();
    let btn_font = theme::font_body_strong();
    let copy_label = "Make a copy";
    let edit_label = "Edit anyway";

    let copy_galley = painter.layout_no_wrap(copy_label.to_string(), btn_font.clone(), theme::AGENT_LABEL_TEXT);
    let edit_galley = painter.layout_no_wrap(edit_label.to_string(), body_font.clone(), theme::TEXT);

    let btn_h = theme::CONTROL_STANDARD.max(theme::TARGET_MIN);
    let copy_btn_w = (copy_galley.size().x + theme::INSET_BUTTON * 2.0).max(theme::TARGET_MIN);
    let edit_btn_w = (edit_galley.size().x + theme::INSET_BUTTON * 2.0).max(theme::TARGET_MIN);

    let dot_d = theme::SPACE_INLINE;
    let banner_h = btn_h + theme::INSET_FRAME * 2.0;
    let max_banner_w = (canvas_rect.width() - theme::INSET_PANEL * 2.0).max(theme::TARGET_MIN);

    let chrome_w = theme::INSET_FRAME * 2.0 + dot_d + theme::SPACE_INLINE + theme::SPACE_INLINE + copy_btn_w + theme::SPACE_INLINE + edit_btn_w;
    let max_text_w = (max_banner_w - chrome_w).max(theme::TARGET_MIN);
    let raw_sentence = info.banner_text();
    let elided_sentence = widgets::elide(&painter, &raw_sentence, &body_font, max_text_w);
    let text_galley = painter.layout_no_wrap(elided_sentence, body_font, theme::TEXT);

    let banner_w = (chrome_w + text_galley.size().x).max(theme::TARGET_MIN).min(max_banner_w);

    let ruler_clear = if app.ui.rulers { theme::RULER_SIZE } else { 0.0 };
    let top_y = canvas_rect.min.y + ruler_clear + theme::INSET_PANEL;
    let left_x = canvas_rect.center().x - banner_w * 0.5;
    let banner_rect = ERect::from_min_size(Pos2::new(left_x, top_y), Vec2::new(banner_w, banner_h));

    // Swallow pointer clicks inside the banner frame so clicking buttons never deselects on canvas.
    let banner_id = Id::new(("agent-busy-frame-banner", info.frame_id.0));
    let _banner_resp = ui.interact(banner_rect, banner_id, Sense::click());

    let center_y = banner_rect.center().y;
    let mut cursor_x = banner_rect.min.x + theme::INSET_FRAME;
    let dot_center = Pos2::new(cursor_x + dot_d * 0.5, center_y);
    cursor_x += dot_d + theme::SPACE_INLINE;

    let text_pos = Pos2::new(cursor_x, center_y - text_galley.size().y * 0.5);
    cursor_x += text_galley.size().x + theme::SPACE_INLINE;

    let copy_rect = ERect::from_min_size(Pos2::new(cursor_x, center_y - btn_h * 0.5), Vec2::new(copy_btn_w, btn_h));
    cursor_x += copy_btn_w + theme::SPACE_INLINE;

    let edit_rect = ERect::from_min_size(Pos2::new(cursor_x, center_y - btn_h * 0.5), Vec2::new(edit_btn_w, btn_h));

    let copy_resp = ui.interact(copy_rect, Id::new(("agent-busy-make-copy", info.frame_id.0)), Sense::click());
    let edit_resp = ui.interact(edit_rect, Id::new(("agent-busy-edit-anyway", info.frame_id.0)), Sense::click());
    if copy_resp.hovered() || edit_resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }

    let painter = ui.painter_at(canvas_rect);
    painter.rect_filled(banner_rect, theme::cr(theme::RADIUS_FRAME), theme::BG_SECONDARY);
    painter.rect_stroke(banner_rect, theme::cr(theme::RADIUS_FRAME), Stroke::new(theme::BORDER_WIDTH, theme::BORDER), StrokeKind::Inside);

    // 8 px agent colour dot + "<agent name> is working here"
    painter.circle_filled(dot_center, dot_d * 0.5, info.color());
    painter.galley(text_pos, text_galley, theme::TEXT);

    // Primary button: "Make a copy" (`BG_BRAND` with dark label)
    let copy_fill = if copy_resp.is_pointer_button_down_on() {
        theme::BG_BRAND_PRESSED
    } else if copy_resp.hovered() {
        theme::BG_BRAND_HOVER
    } else {
        theme::BG_BRAND
    };
    painter.rect_filled(copy_rect, theme::cr(theme::RADIUS_CONTROL), copy_fill);
    painter.galley(copy_rect.center() - copy_galley.size() * 0.5, copy_galley, theme::AGENT_LABEL_TEXT);

    // Neutral button: "Edit anyway" (`BG_TERTIARY` with `TEXT` label)
    let edit_fill = if edit_resp.is_pointer_button_down_on() {
        theme::BG_PRESSED
    } else if edit_resp.hovered() {
        theme::BG_HOVER
    } else {
        theme::BG_TERTIARY
    };
    painter.rect_filled(edit_rect, theme::cr(theme::RADIUS_CONTROL), edit_fill);
    painter.galley(edit_rect.center() - edit_galley.size() * 0.5, edit_galley, theme::TEXT);

    widgets::record(ui, "agent-busy-frame-banner", banner_rect);
    widgets::record(ui, "agent-busy-make-copy", copy_rect);
    widgets::record(ui, "agent-busy-edit-anyway", edit_rect);

    if copy_resp.clicked() {
        app.duplicate_into_free_spot(info.frame_id);
        app.sync_busy_frame_banner();
    } else if edit_resp.clicked() {
        app.agent_busy_frames.dismiss_banner(info.frame_id, info.run_key);
    }
}
