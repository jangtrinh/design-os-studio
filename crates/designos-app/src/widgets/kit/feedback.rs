use super::{look, Look};
use crate::theme;
use egui::Color32;

const W: f32 = theme::BORDER_WIDTH;
const CLEAR: Color32 = Color32::TRANSPARENT;

/// Home file tiles, plus the neutral empty-state tile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TileKind {
    Design,
    Board,
    Deck,
    Spec,
    Neutral,
}

pub const fn progress_look() -> Look {
    look(theme::BG_PRESSED, CLEAR, W, theme::TEXT_SECONDARY, theme::ICON_BRAND_PRIMARY)
}

pub const fn loading_look() -> Look {
    look(theme::BG_SECONDARY, theme::BORDER, W, theme::TEXT_SECONDARY, theme::ICON_BRAND_PRIMARY)
}

pub const fn empty_state_look() -> Look {
    look(CLEAR, theme::BORDER_STRONG, W, theme::TEXT, theme::ICON_QUATERNARY)
}

pub const fn tile_icon(kind: TileKind) -> Look {
    match kind {
        TileKind::Design => look(theme::TILE_DESIGN_BG, CLEAR, W, CLEAR, theme::TILE_DESIGN_GLYPH),
        TileKind::Board => look(theme::TILE_BOARD_BG, CLEAR, W, CLEAR, theme::TILE_BOARD_GLYPH),
        TileKind::Deck => look(theme::TILE_DECK_BG, CLEAR, W, CLEAR, theme::TILE_DECK_GLYPH),
        TileKind::Spec => look(theme::TILE_SPEC_BG, CLEAR, W, CLEAR, theme::TILE_SPEC_GLYPH),
        TileKind::Neutral => empty_state_look(),
    }
}
