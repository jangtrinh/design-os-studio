use super::focus::{keyboard_focus, paint_ring};
use super::{disabled, look, paint_frame, selected_hover, state_of, Look, State};
use crate::icons::Icon;
use crate::theme;
use crate::widgets::{self, Key};
use egui::{Color32, Painter, Rect, Response, Sense, Ui, Vec2};

const W: f32 = theme::BORDER_WIDTH;
const CLEAR: Color32 = Color32::TRANSPARENT;

const fn focus(focused: bool) -> Color32 {
    if focused {
        theme::FOCUS_RING
    } else {
        CLEAR
    }
}

#[cfg_attr(not(test), allow(dead_code))]
pub const fn toggle_ring(focused: bool) -> Color32 {
    focus(focused)
}
pub const fn checkbox_ring(focused: bool) -> Color32 {
    focus(focused)
}
#[cfg_attr(not(test), allow(dead_code))]
pub const fn radio_ring(focused: bool) -> Color32 {
    focus(focused)
}
#[cfg_attr(not(test), allow(dead_code))]
pub const fn slider_ring(focused: bool) -> Color32 {
    focus(focused)
}
pub const fn swatch_ring_color(focused: bool) -> Color32 {
    focus(focused)
}

#[cfg_attr(not(test), allow(dead_code))]
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

#[cfg_attr(not(test), allow(dead_code))]
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

#[cfg_attr(not(test), allow(dead_code))]
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

fn click(ui: &mut Ui, rect: Rect, key: impl Key, enabled: bool) -> Response {
    ui.interact(rect, ui.id().with(key), if enabled { Sense::click() } else { Sense::hover() })
}

pub fn checkbox(ui: &mut Ui, rect: Rect, key: impl Key, value: bool, enabled: bool) -> Response {
    let response = click(ui, rect, key, enabled);
    let paint = checkbox_look(state_of(&response, value, enabled));
    let box_rect = Rect::from_center_size(rect.center(), Vec2::splat(theme::LAYER_ICON_SIZE));
    paint_frame(ui.painter(), box_rect, theme::SPACE_FACT as u8, &paint);
    if value {
        let on = checkbox_look(State::Selected);
        let mark = if enabled { on.icon } else { disabled(on).icon };
        widgets::paint_icon_in(ui.painter(), box_rect, Icon::Check, theme::FONT_SIZE_TEXT_XS, mark);
    }
    paint_ring(ui.painter(), box_rect, theme::SPACE_FACT, checkbox_ring(keyboard_focus(ui, response.id)));
    response
}

#[allow(dead_code)]
pub fn toggle(ui: &mut Ui, rect: Rect, key: impl Key, on: bool, enabled: bool) -> Response {
    let response = click(ui, rect, key, enabled);
    let paint = toggle_look(state_of(&response, on, enabled));
    paint_frame(ui.painter(), rect, theme::RADIUS_CONTROL, &paint);
    let knob = rect.height() * 0.5 - theme::SPACE_FACT * 0.5;
    let cx = if on { rect.right() - knob - theme::SPACE_FACT } else { rect.left() + knob + theme::SPACE_FACT };
    ui.painter().circle_filled(egui::pos2(cx, rect.center().y), knob.max(0.0), paint.icon);
    paint_ring(ui.painter(), rect, theme::RADIUS_CONTROL as f32, toggle_ring(keyboard_focus(ui, response.id)));
    response
}

#[allow(dead_code)]
pub fn radio_card(ui: &mut Ui, rect: Rect, key: impl Key, label: &str, selected: bool, enabled: bool) -> Response {
    let response = click(ui, rect, key, enabled);
    let state = state_of(&response, selected, enabled);
    let paint = if selected && enabled && response.hovered() && !response.is_pointer_button_down_on() {
        selected_hover(radio_card_look(State::Selected), radio_card_look(State::Hover))
    } else {
        radio_card_look(state)
    };
    paint_frame(ui.painter(), rect, theme::RADIUS_CONTROL, &paint);
    let center = egui::pos2(rect.left() + theme::LAYER_ICON_SIZE, rect.center().y);
    ui.painter().circle_stroke(center, theme::SPACE_FACT, egui::Stroke::new(theme::BORDER_WIDTH, paint.icon));
    if selected {
        ui.painter().circle_filled(center, theme::SPACE_FACT * 0.5, paint.mark);
    }
    let font = theme::font_text(theme::FONT_SIZE_TEXT_SM, theme::TextWeight::Medium);
    ui.painter().text(egui::pos2(center.x + theme::LAYER_ICON_SIZE, rect.center().y), egui::Align2::LEFT_CENTER, label, font, paint.text);
    paint_ring(ui.painter(), rect, theme::RADIUS_CONTROL as f32, radio_ring(keyboard_focus(ui, response.id)));
    response
}

#[allow(dead_code)]
pub fn slider_handle(ui: &mut Ui, rect: Rect, key: impl Key, enabled: bool) -> Response {
    let response = ui.interact(rect, ui.id().with(key), if enabled { Sense::click_and_drag() } else { Sense::hover() });
    let paint = slider_handle_look(state_of(&response, false, enabled));
    let radius = rect.width().min(rect.height()) * 0.5;
    ui.painter().circle_filled(rect.center(), radius, paint.fill);
    ui.painter().circle_stroke(rect.center(), radius, egui::Stroke::new(paint.border_w, paint.border));
    paint_ring(ui.painter(), rect, radius, slider_ring(keyboard_focus(ui, response.id)));
    response
}

/// Resting or selected swatch edge, plus the keyboard band when `focused`.
pub fn swatch_ring(p: &Painter, rect: Rect, selected: bool, focused: bool) {
    let paint = swatch_look(if selected { State::Selected } else { State::Default });
    paint_frame(p, rect, theme::SWATCH_RADIUS, &paint);
    paint_ring(p, rect, theme::SWATCH_RADIUS as f32, swatch_ring_color(focused));
}
