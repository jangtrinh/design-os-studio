use super::{look, paint_frame, Look};
use crate::theme;
use egui::{Color32, Painter, Pos2, Rect};

const W: f32 = theme::BORDER_WIDTH;
const CLEAR: Color32 = Color32::TRANSPARENT;

/// Home file tiles, plus the neutral empty-state tile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(not(test), allow(dead_code))]
pub enum TileKind {
    Design,
    Board,
    Deck,
    Spec,
    Neutral,
}

#[cfg_attr(not(test), allow(dead_code))]
pub const fn progress_look() -> Look {
    look(theme::BG_PRESSED, CLEAR, W, theme::TEXT_SECONDARY, theme::ICON_BRAND_PRIMARY)
}

#[cfg_attr(not(test), allow(dead_code))]
pub const fn loading_look() -> Look {
    look(theme::BG_SECONDARY, theme::BORDER, W, theme::TEXT_SECONDARY, theme::ICON_BRAND_PRIMARY)
}

#[cfg_attr(not(test), allow(dead_code))]
pub const fn empty_state_look() -> Look {
    look(CLEAR, theme::BORDER_STRONG, W, theme::TEXT, theme::ICON_QUATERNARY)
}

#[cfg_attr(not(test), allow(dead_code))]
pub const fn tile_look(kind: TileKind) -> Look {
    match kind {
        TileKind::Design => look(theme::TILE_DESIGN_BG, CLEAR, W, CLEAR, theme::TILE_DESIGN_GLYPH),
        TileKind::Board => look(theme::TILE_BOARD_BG, CLEAR, W, CLEAR, theme::TILE_BOARD_GLYPH),
        TileKind::Deck => look(theme::TILE_DECK_BG, CLEAR, W, CLEAR, theme::TILE_DECK_GLYPH),
        TileKind::Spec => look(theme::TILE_SPEC_BG, CLEAR, W, CLEAR, theme::TILE_SPEC_GLYPH),
        TileKind::Neutral => empty_state_look(),
    }
}

#[allow(dead_code)]
pub fn progress_bar(p: &Painter, rect: Rect, fraction: f32) {
    let paint = progress_look();
    p.rect_filled(rect, theme::cr(theme::RADIUS_MARK), paint.fill);
    let mut done = rect;
    done.max.x = rect.min.x + rect.width() * fraction.clamp(0.0, 1.0);
    if done.width() > 0.0 {
        p.rect_filled(done, theme::cr(theme::RADIUS_MARK), paint.icon);
    }
}

#[allow(dead_code)]
pub fn loading_ring(p: &Painter, center: Pos2, radius: f32) {
    let paint = loading_look();
    p.circle_filled(center, radius, paint.fill);
    p.circle_stroke(center, radius, egui::Stroke::new(paint.border_w.max(theme::BORDER_WIDTH), paint.icon));
}

#[allow(dead_code)]
pub fn empty_state(p: &Painter, rect: Rect, title: &str) {
    let paint = empty_state_look();
    paint_frame(p, rect, theme::RADIUS_XL, &paint);
    let font = theme::font_text(theme::FONT_SIZE_TEXT_SM, theme::TextWeight::Medium);
    p.text(rect.center(), egui::Align2::CENTER_CENTER, title, font, paint.text);
}

#[allow(dead_code)]
pub fn tile_icon(p: &Painter, rect: Rect, kind: TileKind) {
    paint_frame(p, rect, theme::RADIUS_CONTROL, &tile_look(kind));
}
