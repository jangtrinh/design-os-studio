use super::{disabled, look, Look, State};
use crate::theme;
use egui::Color32;

const W: f32 = theme::BORDER_WIDTH;

const fn body(state: State, icon: Color32, resting_text: Color32, focused_text: Color32) -> Look {
    let resting = look(theme::BG, theme::BORDER_STRONG, W, resting_text, icon);
    match state {
        State::Focused => look(theme::BG, theme::BORDER_SELECTED, W * 2.0, focused_text, icon),
        State::Disabled => disabled(resting),
        _ => resting,
    }
}

/// `invalid` replaces the focused stroke with the error ring colour. Resting keeps the strong border.
pub const fn field_look(state: State, invalid: bool) -> Look {
    let mut paint = body(state, theme::ICON_QUATERNARY, theme::TEXT, theme::TEXT);
    if let State::Focused = state {
        if invalid {
            paint.border = theme::FOCUS_RING_ERROR;
            paint.border_w = W * 2.0;
        }
    }
    paint
}

pub const fn property_field_look(state: State) -> Look {
    body(state, theme::TEXT_TERTIARY, theme::TEXT, theme::TEXT)
}

pub const fn search_look(state: State) -> Look {
    body(state, theme::ICON_QUATERNARY, theme::TEXT_PLACEHOLDER, theme::TEXT)
}

pub const fn select_look(state: State) -> Look {
    body(state, theme::ICON_QUATERNARY, theme::TEXT, theme::TEXT)
}
