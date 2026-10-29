use super::{disabled, look, Look, State, Surface};
use crate::theme;
use egui::Color32;

const W: f32 = theme::BORDER_WIDTH;
const CLEAR: Color32 = Color32::TRANSPARENT;

pub const fn row_look(state: State, surface: Surface) -> Look {
    let (fill, text, icon) = match (state, surface) {
        (State::Hover, Surface::Primary) => (theme::BG_HOVER, theme::TEXT_SECONDARY_HOVER, theme::ICON_QUATERNARY_HOVER),
        (State::Hover, Surface::Secondary) => (theme::BG_SECONDARY_HOVER, theme::TEXT_SECONDARY_HOVER, theme::ICON_QUATERNARY_HOVER),
        (State::Focused, _) => (theme::BG, theme::TEXT_SECONDARY, theme::ICON_QUATERNARY),
        (State::Selected, Surface::Primary) => (theme::BG_SELECTED, theme::TEXT_SECONDARY_HOVER, theme::ICON_QUATERNARY_HOVER),
        (State::Selected, Surface::Secondary) | (State::Pressed, Surface::Secondary) => {
            (theme::BG_PRESSED, theme::TEXT_SECONDARY, theme::ICON_QUATERNARY_HOVER)
        }
        _ => (CLEAR, theme::TEXT_SECONDARY, theme::ICON_QUATERNARY),
    };
    look(fill, CLEAR, W, text, icon)
}

pub const fn list_row_ring(focused: bool) -> Color32 {
    if focused {
        theme::FOCUS_RING
    } else {
        CLEAR
    }
}

pub const fn tree_row_look(state: State, surface: Surface) -> Look {
    match (state, surface) {
        (State::Hover, Surface::Primary) => look(theme::BG_HOVER, CLEAR, W, theme::TEXT_TERTIARY, theme::ICON_QUATERNARY),
        (State::Hover, Surface::Secondary) => look(theme::BG_SECONDARY_HOVER, CLEAR, W, theme::TEXT_TERTIARY, theme::ICON_QUATERNARY),
        (State::Selected, Surface::Primary) => look(theme::BG_SELECTED, CLEAR, W, theme::TEXT_TERTIARY, theme::ICON_QUATERNARY),
        (State::Selected, Surface::Secondary) | (State::Pressed, Surface::Secondary) => {
            look(theme::BG_PRESSED, CLEAR, W, theme::TEXT_SECONDARY, theme::ICON_QUATERNARY_HOVER)
        }
        _ => look(CLEAR, CLEAR, W, theme::TEXT_TERTIARY, theme::ICON_QUATERNARY),
    }
}

pub const fn menu_item_look(state: State) -> Look {
    match state {
        State::Hover | State::Pressed | State::Focused => look(theme::BG_HOVER, CLEAR, W, theme::TEXT_SECONDARY_HOVER, theme::ICON_QUATERNARY_HOVER),
        State::Disabled => disabled(look(CLEAR, CLEAR, W, theme::TEXT_SECONDARY, theme::ICON_QUATERNARY)),
        State::Selected => look(CLEAR, CLEAR, W, theme::TEXT_SECONDARY, theme::ICON_BRAND_PRIMARY),
        _ => look(CLEAR, CLEAR, W, theme::TEXT_SECONDARY, theme::ICON_QUATERNARY),
    }
}

pub const fn shortcut_look(state: State) -> Look {
    let resting = look(CLEAR, CLEAR, W, theme::TEXT_QUATERNARY, CLEAR);
    match state {
        State::Disabled => disabled(resting),
        _ => resting,
    }
}

pub const fn separator_look() -> Look {
    look(theme::BORDER, CLEAR, W, CLEAR, CLEAR)
}

pub const fn divider_look() -> Look {
    separator_look()
}

pub const fn panel_header_look() -> Look {
    look(CLEAR, CLEAR, W, theme::TEXT, theme::ICON_QUATERNARY)
}

pub const fn section_header_look(state: State) -> Look {
    let resting = look(CLEAR, theme::BORDER, W, theme::TEXT, theme::ICON_QUATERNARY);
    match state {
        State::Disabled => disabled(resting),
        _ => resting,
    }
}
