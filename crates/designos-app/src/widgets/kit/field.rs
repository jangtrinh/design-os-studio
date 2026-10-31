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

#[cfg_attr(not(test), allow(dead_code))]
pub const fn select_look(state: State) -> Look {
    body(state, theme::ICON_QUATERNARY, theme::TEXT, theme::TEXT)
}

/// The dropdown trigger look (issue #1273), distinct from the kit `select_look`.
pub const fn dropdown_look(state: State) -> Look {
    let resting = look(theme::BG, Color32::TRANSPARENT, 0.0, theme::TEXT, theme::ICON_QUATERNARY);
    match state {
        State::Hover => look(theme::BG, theme::BORDER, W, theme::TEXT, theme::ICON_QUATERNARY),
        State::Focused => look(theme::BG, theme::BORDER_SELECTED, W, theme::TEXT, theme::ICON_QUATERNARY),
        State::Disabled => disabled(resting),
        _ => resting,
    }
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

#[cfg(test)]
mod dropdown_tests {
    use super::*;

    #[test]
    fn dropdown_trigger_states_are_distinct_from_kit_select() {
        let default = dropdown_look(State::Default);
        assert_eq!(default.border_w, 0.0, "the idle dropdown trigger must skip its stroke");
        assert_eq!(default.border, Color32::TRANSPARENT, "the idle dropdown border must be transparent");
        let hover = dropdown_look(State::Hover);
        assert_eq!(hover.border, theme::BORDER, "hover uses the hairline border token");
        assert_eq!(hover.border_w, W, "hover uses one border width");
        let focused = dropdown_look(State::Focused);
        assert_eq!(focused.border, theme::BORDER_SELECTED, "focus uses the selected border token");
        assert_eq!(focused.border_w, W, "focus uses a one width ring");
        assert_eq!(select_look(State::Default).border, theme::BORDER_STRONG, "kit select keeps its strong resting border");
        assert_eq!(select_look(State::Focused).border_w, W * 2.0, "kit select keeps its two width focus stroke");
    }
}
