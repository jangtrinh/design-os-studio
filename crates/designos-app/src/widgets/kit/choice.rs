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

/// Pressed is not its own look: the control keeps the value it already shows.
fn held_value(response: &Response, value: bool, enabled: bool) -> State {
    match state_of(response, value, enabled) {
        State::Pressed if value => State::Selected,
        State::Pressed => State::Default,
        other => other,
    }
}

pub fn checkbox(ui: &mut Ui, rect: Rect, key: impl Key, value: bool, enabled: bool) -> Response {
    let shown = enabled || !ui.is_enabled();
    let response = click(ui, rect, key, enabled && ui.is_enabled());
    let paint = checkbox_look(held_value(&response, value, shown));
    let side = theme::LAYER_ICON_SIZE;
    let box_rect = Rect::from_min_size(egui::pos2(rect.left(), rect.center().y - side * 0.5), Vec2::splat(side));
    paint_frame(ui.painter(), box_rect, theme::SPACE_FACT as u8, &paint);
    if value {
        let on = checkbox_look(State::Selected);
        let mark = if shown { on.icon } else { disabled(on).icon };
        widgets::paint_icon_in(ui.painter(), box_rect, Icon::Check, theme::FONT_SIZE_TEXT_XS, mark);
    }
    paint_ring(ui.painter(), box_rect, theme::SPACE_FACT, checkbox_ring(keyboard_focus(ui, response.id)));
    response
}

pub fn toggle(ui: &mut Ui, rect: Rect, key: impl Key, on: bool, enabled: bool) -> Response {
    let shown = enabled || !ui.is_enabled();
    // The track keeps its own height; a track under the pointer floor hits on a taller band around it.
    let pad = ((theme::TARGET_MIN - rect.height()) * 0.5).max(0.0);
    let response = click(ui, rect.expand2(egui::vec2(0.0, pad)), key, enabled && ui.is_enabled());
    let paint = toggle_look(held_value(&response, on, shown));
    let radius = theme::radius_full(rect.height()).round();
    paint_frame(ui.painter(), rect, radius as u8, &paint);
    let knob = theme::radius_full(theme::LAYER_ICON_SIZE);
    let cx = if on { rect.right() - theme::INSET_SEGMENT - knob } else { rect.left() + theme::INSET_SEGMENT + knob };
    ui.painter().circle_filled(egui::pos2(cx, rect.center().y), knob.max(0.0), paint.icon);
    paint_ring(ui.painter(), rect, radius, toggle_ring(keyboard_focus(ui, response.id)));
    response
}

#[allow(dead_code)]
pub fn radio_card(ui: &mut Ui, rect: Rect, key: impl Key, label: &str, selected: bool, enabled: bool) -> Response {
    let shown = enabled || !ui.is_enabled();
    let response = click(ui, rect, key, enabled && ui.is_enabled());
    let state = state_of(&response, selected, shown);
    let paint = if selected && shown && response.hovered() && !response.is_pointer_button_down_on() {
        selected_hover(radio_card_look(State::Selected), radio_card_look(State::Hover))
    } else {
        radio_card_look(state)
    };
    paint_frame(ui.painter(), rect, theme::RADIUS_XL, &paint);
    let radio_r = theme::LAYER_ICON_SIZE * 0.5;
    let center = egui::pos2(rect.left() + theme::INSET_PANEL + radio_r, rect.center().y);
    if selected {
        ui.painter().circle_filled(center, radio_r, paint.icon);
        ui.painter().circle_filled(center, theme::INSET_SEGMENT + theme::BORDER_WIDTH, paint.mark);
    } else {
        ui.painter().circle_stroke(center, radio_r, egui::Stroke::new(theme::BORDER_WIDTH, paint.icon));
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
    ui.painter().circle_filled(rect.center(), radius, paint.icon);
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
