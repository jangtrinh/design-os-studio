use super::{disabled, look, Look, State};
use crate::theme;
use egui::Color32;

const W: f32 = theme::BORDER_WIDTH;
const CLEAR: Color32 = Color32::TRANSPARENT;

const fn focus(focused: bool) -> Color32 {
    if focused {
        theme::FOCUS_RING
    } else {
        CLEAR
    }
}

pub const fn toggle_ring(focused: bool) -> Color32 {
    focus(focused)
}
pub const fn checkbox_ring(focused: bool) -> Color32 {
    focus(focused)
}
pub const fn radio_ring(focused: bool) -> Color32 {
    focus(focused)
}
pub const fn slider_ring(focused: bool) -> Color32 {
    focus(focused)
}
pub const fn swatch_ring(focused: bool) -> Color32 {
    focus(focused)
}

pub const fn toggle_look(state: State) -> Look {
    let track = look(theme::BG_TERTIARY, theme::BORDER, W * 0.5, CLEAR, theme::ICON_WHITE);
    match state {
        State::Disabled => disabled(track),
        State::Selected => look(theme::BG_BRAND, CLEAR, W, CLEAR, theme::ICON_WHITE),
        _ => track,
    }
}

pub const fn checkbox_look(state: State) -> Look {
    match state {
        State::Focused => look(theme::BG, theme::BORDER_STRONG, W, CLEAR, CLEAR),
        State::Disabled => disabled(look(theme::BG_TERTIARY, theme::BORDER_STRONG, W, CLEAR, CLEAR)),
        State::Selected => look(theme::BG_BRAND, CLEAR, W, CLEAR, theme::TEXT_ON_BRAND),
        _ => look(CLEAR, theme::BORDER_STRONG, W, CLEAR, CLEAR),
    }
}

pub const fn radio_card_look(state: State) -> Look {
    let resting = look(theme::BG, theme::BORDER, W, theme::TEXT_SECONDARY, theme::BORDER_STRONG);
    match state {
        State::Hover => look(theme::BG_HOVER, theme::BORDER, W, theme::TEXT_SECONDARY, theme::BORDER_STRONG),
        State::Disabled => disabled(resting),
        State::Selected => {
            let mut paint = look(theme::BG, theme::BORDER_SELECTED, W * 2.0, theme::TEXT, theme::BG_BRAND);
            paint.mark = theme::TEXT_ON_BRAND;
            paint
        }
        _ => resting,
    }
}

pub const fn slider_handle_look(state: State) -> Look {
    let resting = look(theme::BG_PRESSED, theme::BG, W * 2.0, CLEAR, theme::ICON_BRAND_PRIMARY);
    match state {
        State::Disabled => disabled(resting),
        _ => resting,
    }
}

pub const fn swatch_look(state: State) -> Look {
    match state {
        State::Selected => look(CLEAR, theme::FOCUS_RING, W * 2.0, CLEAR, CLEAR),
        _ => look(CLEAR, theme::BORDER_TRANSLUCENT, W, CLEAR, CLEAR),
    }
}
