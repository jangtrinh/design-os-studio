use super::{disabled, look, Look, State};
use crate::theme;
use egui::Color32;

const W: f32 = theme::BORDER_WIDTH;
const CLEAR: Color32 = Color32::TRANSPARENT;

pub const FILE_TAB_BAR_H: f32 = theme::CONTROL_XS;
pub const SESSION_TAB_H: f32 = theme::RIGHT_TABS_H;
pub const SEGMENTED_STRIP_H: f32 = theme::RIGHT_TABS_H;
pub const HOME_TAB_H: f32 = theme::CONTROL_SM;

pub const fn segment_tile_h() -> f32 {
    theme::RIGHT_TABS_H - theme::INSET_SEGMENT * 2.0
}

const fn focus(focused: bool) -> Color32 {
    if focused {
        theme::FOCUS_RING
    } else {
        CLEAR
    }
}

pub const fn segmented_ring(focused: bool) -> Color32 {
    focus(focused)
}
pub const fn tab_ring(focused: bool) -> Color32 {
    focus(focused)
}

pub const fn segmented_look(state: State) -> Look {
    match state {
        State::Hover => look(CLEAR, CLEAR, W, theme::TEXT_SECONDARY, theme::ICON_QUATERNARY_HOVER),
        State::Focused => look(theme::BG, CLEAR, W, theme::TEXT_QUATERNARY, theme::ICON_QUATERNARY),
        State::Disabled => disabled(look(CLEAR, CLEAR, W, theme::TEXT_QUATERNARY, theme::ICON_QUATERNARY)),
        State::Selected => look(theme::BG_SELECTED, theme::BORDER_STRONG, W, theme::TEXT_SECONDARY, theme::ICON_SECONDARY),
        _ => look(CLEAR, CLEAR, W, theme::TEXT_QUATERNARY, theme::ICON_QUATERNARY),
    }
}

pub const fn tab_look(state: State) -> Look {
    match state {
        State::Hover => look(theme::BG_HOVER, CLEAR, W, theme::TEXT_SECONDARY, theme::ICON_QUATERNARY_HOVER),
        State::Focused => look(theme::BG, CLEAR, W, theme::TEXT_QUATERNARY, theme::ICON_QUATERNARY),
        State::Disabled => disabled(look(CLEAR, CLEAR, W, theme::TEXT_QUATERNARY, theme::ICON_QUATERNARY)),
        State::Selected => look(theme::BG_SELECTED, CLEAR, W, theme::TEXT_SECONDARY, theme::ICON_QUATERNARY_HOVER),
        _ => look(CLEAR, CLEAR, W, theme::TEXT_QUATERNARY, theme::ICON_QUATERNARY),
    }
}
