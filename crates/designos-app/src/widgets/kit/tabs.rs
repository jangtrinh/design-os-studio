use super::focus::{keyboard_focus, paint_ring};
use super::{disabled, look, paint_frame, selected_hover, state_of, Look, Size, State};
use crate::theme;
use crate::widgets::Key;
use egui::{Color32, Rect, Response, Sense, Ui};

const W: f32 = theme::BORDER_WIDTH;
const CLEAR: Color32 = Color32::TRANSPARENT;

#[cfg_attr(not(test), allow(dead_code))]
pub const FILE_TAB_BAR_H: f32 = theme::CONTROL_XS;
#[cfg_attr(not(test), allow(dead_code))]
pub const SESSION_TAB_H: f32 = theme::RIGHT_TABS_H;
#[cfg_attr(not(test), allow(dead_code))]
pub const SEGMENTED_STRIP_H: f32 = theme::RIGHT_TABS_H;
#[cfg_attr(not(test), allow(dead_code))]
pub const HOME_TAB_H: f32 = theme::CONTROL_SM;

#[cfg_attr(not(test), allow(dead_code))]
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

#[cfg_attr(not(test), allow(dead_code))]
pub const fn segmented_ring(focused: bool) -> Color32 {
    focus(focused)
}
#[cfg_attr(not(test), allow(dead_code))]
pub const fn tab_ring(focused: bool) -> Color32 {
    focus(focused)
}

#[cfg_attr(not(test), allow(dead_code))]
pub const fn segmented_look(state: State) -> Look {
    match state {
        State::Hover => look(CLEAR, CLEAR, W, theme::TEXT_SECONDARY, theme::ICON_QUATERNARY_HOVER),
        State::Focused => look(theme::BG, CLEAR, W, theme::TEXT_QUATERNARY, theme::ICON_QUATERNARY),
        State::Disabled => disabled(look(CLEAR, CLEAR, W, theme::TEXT_QUATERNARY, theme::ICON_QUATERNARY)),
        State::Selected => look(theme::BG_SELECTED, theme::BORDER_STRONG, W, theme::TEXT_SECONDARY, theme::ICON_SECONDARY),
        _ => look(CLEAR, CLEAR, W, theme::TEXT_QUATERNARY, theme::ICON_QUATERNARY),
    }
}

#[cfg_attr(not(test), allow(dead_code))]
pub const fn tab_look(state: State) -> Look {
    match state {
        State::Hover => look(theme::BG_HOVER, CLEAR, W, theme::TEXT_SECONDARY, theme::ICON_QUATERNARY_HOVER),
        State::Focused => look(theme::BG, CLEAR, W, theme::TEXT_QUATERNARY, theme::ICON_QUATERNARY),
        State::Disabled => disabled(look(CLEAR, CLEAR, W, theme::TEXT_QUATERNARY, theme::ICON_QUATERNARY)),
        State::Selected => look(theme::BG_SELECTED, CLEAR, W, theme::TEXT_SECONDARY, theme::ICON_QUATERNARY_HOVER),
        _ => look(CLEAR, CLEAR, W, theme::TEXT_QUATERNARY, theme::ICON_QUATERNARY),
    }
}

#[allow(dead_code)]
fn label_font() -> egui::FontId {
    theme::font_text(theme::FONT_SIZE_TEXT_XS, theme::TextWeight::Semibold)
}

#[allow(dead_code)]
fn paint_cell(ui: &Ui, cell: Rect, paint: &Look, label: &str, font: egui::FontId, ring: Color32, radius: u8) {
    paint_frame(ui.painter(), cell, radius, paint);
    ui.painter().text(cell.center(), egui::Align2::CENTER_CENTER, label, font, paint.text);
    paint_ring(ui.painter(), cell, radius as f32, ring);
}

#[allow(dead_code)]
pub fn segmented(ui: &mut Ui, rect: Rect, key: impl Key, items: &[&str], selected: usize, _size: Size) -> Option<usize> {
    if items.is_empty() {
        return None;
    }
    let shell = look(theme::BG_SECONDARY, CLEAR, 0.0, CLEAR, CLEAR);
    paint_frame(ui.painter(), rect, theme::RADIUS_CONTROL, &shell);
    let font = label_font();
    let salt = ui.id().with(key);
    // The tile is always inset in the shell. A strip too short for a minimum target around the tile hits on the
    // full-height column instead, so the option stays easy to press without painting flush to the shell edge.
    let inner = rect.shrink(theme::INSET_SEGMENT);
    let full_height_hit = inner.height() < theme::TARGET_MIN;
    let count = items.len() as f32;
    let width = inner.width() / count;
    let hit_width = rect.width() / count;
    let mut picked = None;
    for (i, label) in items.iter().enumerate() {
        let cell = Rect::from_min_size(egui::pos2(inner.left() + width * i as f32, inner.top()), egui::vec2(width, inner.height()));
        let hit = if full_height_hit {
            Rect::from_min_size(egui::pos2(rect.left() + hit_width * i as f32, rect.top()), egui::vec2(hit_width, rect.height()))
        } else {
            cell
        };
        let response = ui.interact(hit, salt.with(i), Sense::click());
        let on = i == selected;
        let state = state_of(&response, on, true);
        let paint = if on && response.hovered() && !response.is_pointer_button_down_on() {
            selected_hover(segmented_look(State::Selected), segmented_look(State::Hover))
        } else {
            segmented_look(state)
        };
        paint_cell(ui, cell, &paint, label, font.clone(), segmented_ring(keyboard_focus(ui, response.id)), theme::RADIUS_SEGMENT);
        if response.clicked() {
            picked = Some(i);
        }
    }
    picked
}

#[allow(dead_code)]
pub fn tab(ui: &mut Ui, rect: Rect, key: impl Key, label: &str, selected: bool) -> Response {
    let response = ui.interact(rect, ui.id().with(key), Sense::click());
    let state = state_of(&response, selected, true);
    let paint = if selected && response.hovered() && !response.is_pointer_button_down_on() {
        selected_hover(tab_look(State::Selected), tab_look(State::Hover))
    } else {
        tab_look(state)
    };
    paint_cell(ui, rect, &paint, label, label_font(), tab_ring(keyboard_focus(ui, response.id)), theme::RADIUS_CONTROL);
    response
}
