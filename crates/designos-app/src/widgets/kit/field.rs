use super::{disabled, look, paint_frame, Look, Size, State};
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

#[cfg_attr(not(test), allow(dead_code))]
pub const fn property_field_look(state: State) -> Look {
    body(state, theme::TEXT_TERTIARY, theme::TEXT, theme::TEXT)
}

#[cfg_attr(not(test), allow(dead_code))]
pub const fn search_look(state: State) -> Look {
    body(state, theme::ICON_QUATERNARY, theme::TEXT_PLACEHOLDER, theme::TEXT)
}

pub const fn select_look(state: State) -> Look {
    body(state, theme::ICON_QUATERNARY, theme::TEXT, theme::TEXT)
}

/// Field chrome. Focus widens the border; there is no hover stroke and no focus ring.
pub fn paint_field(ui: &egui::Ui, rect: egui::Rect, id: egui::Id, size: Size, invalid: bool) {
    // A disabled `Ui` already multiplies painter opacity. Halving here as well would dim twice.
    let state = if ui.is_enabled() && ui.memory(|mem| mem.has_focus(id)) { State::Focused } else { State::Default };
    let radius = match size {
        Size::Xs | Size::Sm | Size::Md => theme::RADIUS_CONTROL,
    };
    paint_frame(ui.painter(), rect, radius, &field_look(state, invalid));
}
