use super::focus::{keyboard_focus, paint_ring};
use super::{disabled, look, paint_frame, selected_hover, state_of, Look, State, Surface};
use crate::theme;
use crate::widgets::Key;
use egui::{Color32, Painter, Rect, Response, Sense, Ui};

const W: f32 = theme::BORDER_WIDTH;
const CLEAR: Color32 = Color32::TRANSPARENT;

#[cfg_attr(not(test), allow(dead_code))]
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

#[cfg_attr(not(test), allow(dead_code))]
pub const fn list_row_ring(focused: bool) -> Color32 {
    if focused {
        theme::FOCUS_RING
    } else {
        CLEAR
    }
}

#[cfg_attr(not(test), allow(dead_code))]
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

/// Command palette row. The selected row uses the selected surface; hover keeps the resting title.
pub const fn command_row_look(selected: bool, hot: bool) -> Look {
    if selected {
        look(theme::BG_SELECTED, CLEAR, W, theme::TEXT, theme::ICON)
    } else if hot {
        look(theme::BG_MENU_HOVER, CLEAR, W, theme::TEXT_SECONDARY, theme::ICON_TERTIARY)
    } else {
        look(CLEAR, CLEAR, W, theme::TEXT_SECONDARY, theme::ICON_TERTIARY)
    }
}

/// Menu row paint. Hover wins the fill; a check keeps the selected mark.
pub fn menu_row_look(disabled_row: bool, hot: bool, checked: bool) -> Look {
    if disabled_row {
        let mut paint = menu_item_look(State::Disabled);
        if checked {
            paint.mark = disabled(menu_item_look(State::Selected)).icon;
        }
        return paint;
    }
    if hot {
        let mut paint = menu_item_look(State::Hover);
        if checked {
            paint.mark = menu_item_look(State::Selected).icon;
        }
        return paint;
    }
    if checked {
        let mut paint = menu_item_look(State::Selected);
        paint.mark = paint.icon;
        return paint;
    }
    menu_item_look(State::Default)
}

#[allow(dead_code)]
pub fn tree_row_bg(p: &Painter, rect: Rect, state: State, surface: Surface) {
    paint_frame(p, rect, theme::RADIUS_CONTROL, &tree_row_look(state, surface));
}

#[allow(dead_code)]
pub fn list_row(ui: &mut Ui, rect: Rect, key: impl Key, label: &str, selected: bool, enabled: bool, surface: Surface) -> Response {
    let shown = enabled || !ui.is_enabled();
    let response = ui.interact(rect, ui.id().with(key), if enabled && ui.is_enabled() { Sense::click() } else { Sense::hover() });
    let state = state_of(&response, selected, shown);
    let paint = if selected && shown && response.hovered() && !response.is_pointer_button_down_on() {
        selected_hover(row_look(State::Selected, surface), row_look(State::Hover, surface))
    } else {
        row_look(state, surface)
    };
    paint_frame(ui.painter(), rect, theme::RADIUS_CONTROL, &paint);
    let font = theme::font_text(theme::FONT_SIZE_TEXT_SM, theme::TextWeight::Semibold);
    ui.painter().text(egui::pos2(rect.left() + theme::INSET_FRAME, rect.center().y), egui::Align2::LEFT_CENTER, label, font, paint.text);
    paint_ring(ui.painter(), rect, theme::RADIUS_CONTROL as f32, list_row_ring(keyboard_focus(ui, response.id)));
    response
}

#[allow(dead_code)]
pub fn panel_header(p: &Painter, rect: Rect, label: &str) {
    let paint = panel_header_look();
    let font = theme::font_text(theme::FONT_SIZE_TEXT_SM, theme::TextWeight::Semibold);
    p.text(egui::pos2(rect.left() + theme::INSET_PANEL, rect.center().y), egui::Align2::LEFT_CENTER, label, font, paint.text);
}

#[allow(dead_code)]
pub fn section_header(p: &Painter, rect: Rect, label: &str, enabled: bool) {
    let paint = section_header_look(if enabled { State::Default } else { State::Disabled });
    let font = theme::font_text(theme::FONT_SIZE_TEXT_SM, theme::TextWeight::Semibold);
    p.text(egui::pos2(rect.left(), rect.center().y), egui::Align2::LEFT_CENTER, label, font, paint.text);
    crate::widgets::hline(p, rect.left(), rect.right(), rect.bottom() - theme::BORDER_WIDTH, paint.border);
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

#[cfg_attr(not(test), allow(dead_code))]
pub const fn divider_look() -> Look {
    separator_look()
}

#[cfg_attr(not(test), allow(dead_code))]
pub const fn panel_header_look() -> Look {
    look(CLEAR, CLEAR, W, theme::TEXT, theme::ICON_QUATERNARY)
}

#[cfg_attr(not(test), allow(dead_code))]
pub const fn section_header_look(state: State) -> Look {
    let resting = look(CLEAR, theme::BORDER, W, theme::TEXT, theme::ICON_QUATERNARY);
    match state {
        State::Disabled => disabled(resting),
        _ => resting,
    }
}
