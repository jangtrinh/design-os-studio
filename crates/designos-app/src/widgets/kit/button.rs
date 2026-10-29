use super::focus::{keyboard_focus, paint_ring, ring_color, RingTone};
use super::{disabled, look, paint_frame, selected_hover, state_of, Look, Size, State};
use crate::icons::Icon;
use crate::theme;
use crate::widgets::{self, Key};
use egui::{Color32, CornerRadius, Painter, Rect, Response, Sense, Ui};

/// Button families. Destructive and its secondary form keep the error ramp.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hierarchy {
    #[cfg_attr(not(test), allow(dead_code))]
    Primary,
    #[cfg_attr(not(test), allow(dead_code))]
    Secondary,
    #[cfg_attr(not(test), allow(dead_code))]
    Tertiary,
    #[cfg_attr(not(test), allow(dead_code))]
    Destructive,
    #[cfg_attr(not(test), allow(dead_code))]
    SecondaryDestructive,
    Icon,
    #[cfg_attr(not(test), allow(dead_code))]
    Tool,
    #[cfg_attr(not(test), allow(dead_code))]
    Close,
}

const W: f32 = theme::BORDER_WIDTH;
const CLEAR: Color32 = Color32::TRANSPARENT;

pub const fn button_look(kind: Hierarchy, state: State) -> Look {
    let state = match state {
        State::Loading => State::Hover,
        other => other,
    };
    match kind {
        Hierarchy::Primary => solid(state, theme::BG_BRAND, theme::BG_BRAND_HOVER, theme::BG_BRAND_PRESSED, theme::TEXT_ON_BRAND),
        Hierarchy::Secondary => secondary(state),
        Hierarchy::Tertiary => tertiary(state),
        Hierarchy::Destructive => solid(state, theme::BG_ERROR_SOLID, theme::BG_ERROR_SOLID_HOVER, theme::BG_ERROR_SOLID_PRESSED, theme::TEXT_WHITE),
        Hierarchy::SecondaryDestructive => secondary_destructive(state),
        Hierarchy::Icon => ghost(state, theme::ICON_QUATERNARY, theme::ICON_QUATERNARY_HOVER, theme::ICON_SECONDARY_HOVER, true),
        Hierarchy::Tool => ghost(state, theme::ICON_QUATERNARY_HOVER, theme::ICON_SECONDARY_HOVER, theme::ICON, true),
        Hierarchy::Close => ghost(state, theme::ICON_QUATERNARY, theme::ICON_QUATERNARY_HOVER, theme::ICON_QUATERNARY, false),
    }
}

pub const fn button_ring(kind: Hierarchy, focused: bool) -> Color32 {
    if !focused {
        return CLEAR;
    }
    let tone = match kind {
        Hierarchy::Destructive | Hierarchy::SecondaryDestructive => RingTone::Error,
        _ => RingTone::Brand,
    };
    ring_color(tone)
}

const fn solid(state: State, fill: Color32, hover: Color32, pressed: Color32, label: Color32) -> Look {
    match state {
        State::Hover => look(hover, CLEAR, W, label, label),
        State::Pressed => look(pressed, CLEAR, W, label, label),
        State::Disabled => disabled(look(fill, CLEAR, W, label, label)),
        _ => look(fill, CLEAR, W, label, label),
    }
}

const fn secondary(state: State) -> Look {
    let border = theme::BORDER_STRONG;
    match state {
        State::Hover => look(theme::BG_HOVER, border, W, theme::TEXT_SECONDARY_HOVER, theme::ICON_QUATERNARY_HOVER),
        State::Pressed => look(theme::BG_PRESSED, border, W, theme::TEXT_SECONDARY_HOVER, theme::ICON_QUATERNARY_HOVER),
        State::Disabled => disabled(look(theme::BG, border, W, theme::TEXT_SECONDARY, theme::ICON_QUATERNARY)),
        _ => look(theme::BG, border, W, theme::TEXT_SECONDARY, theme::ICON_QUATERNARY),
    }
}

const fn tertiary(state: State) -> Look {
    match state {
        State::Hover => look(theme::BG_HOVER, CLEAR, W, theme::TEXT_TERTIARY_HOVER, theme::ICON_QUATERNARY_HOVER),
        State::Pressed => look(theme::BG_PRESSED, CLEAR, W, theme::TEXT_TERTIARY_HOVER, theme::ICON_QUATERNARY_HOVER),
        State::Focused => look(theme::BG, CLEAR, W, theme::TEXT_TERTIARY, theme::ICON_QUATERNARY),
        State::Disabled => disabled(look(CLEAR, CLEAR, W, theme::TEXT_TERTIARY, theme::ICON_QUATERNARY)),
        _ => look(CLEAR, CLEAR, W, theme::TEXT_TERTIARY, theme::ICON_QUATERNARY),
    }
}

const fn secondary_destructive(state: State) -> Look {
    match state {
        State::Hover => look(theme::BG_ERROR_PRIMARY, theme::BORDER_ERROR_SUBTLE, W, theme::TEXT_ERROR_HOVER, theme::TEXT_ERROR_HOVER),
        State::Pressed => look(theme::BG_ERROR_PRIMARY, theme::BORDER_ERROR, W, theme::TEXT_ERROR_HOVER, theme::TEXT_ERROR_HOVER),
        State::Disabled => disabled(look(theme::BG, theme::BORDER_ERROR_SUBTLE, W, theme::TEXT_ERROR, theme::TEXT_ERROR)),
        _ => look(theme::BG, theme::BORDER_ERROR_SUBTLE, W, theme::TEXT_ERROR, theme::TEXT_ERROR),
    }
}

/// Plate behind a close control. The glyph and the hover fill change with that plate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClosePlate {
    Primary,
    Hover,
    Selected,
}

/// Close glyph over a resting, hovered or selected tab. Pressed and hovered share a fill on a selected tab.
pub const fn close_look_over(plate: ClosePlate, state: State) -> Look {
    let state = match state {
        State::Loading => State::Hover,
        other => other,
    };
    match plate {
        ClosePlate::Primary => look(CLEAR, CLEAR, W, CLEAR, theme::ICON_QUATERNARY),
        ClosePlate::Hover => match state {
            State::Hover => look(theme::BG_SECONDARY_HOVER, CLEAR, W, CLEAR, theme::ICON_QUATERNARY_HOVER),
            State::Pressed => look(theme::BG_PRESSED, CLEAR, W, CLEAR, theme::ICON_QUATERNARY_HOVER),
            _ => look(CLEAR, CLEAR, W, CLEAR, theme::ICON_QUATERNARY),
        },
        ClosePlate::Selected => match state {
            State::Hover | State::Pressed => look(theme::BG_PRESSED, CLEAR, W, CLEAR, theme::ICON_QUATERNARY_HOVER),
            _ => look(CLEAR, CLEAR, W, CLEAR, theme::ICON_QUATERNARY),
        },
    }
}

/// Toast action: secondary text, a visible hover fill, pressed fill. Not the tertiary button ramp.
pub const fn toast_action_look(state: State) -> Look {
    let state = match state {
        State::Loading => State::Hover,
        other => other,
    };
    match state {
        State::Hover => look(theme::BG_SECONDARY_HOVER, CLEAR, W, theme::TEXT_SECONDARY_HOVER, theme::TEXT_SECONDARY_HOVER),
        State::Pressed => look(theme::BG_PRESSED, CLEAR, W, theme::TEXT_SECONDARY, theme::TEXT_SECONDARY),
        State::Disabled => disabled(look(CLEAR, CLEAR, W, theme::TEXT_SECONDARY, theme::TEXT_SECONDARY)),
        _ => look(CLEAR, CLEAR, W, theme::TEXT_SECONDARY, theme::TEXT_SECONDARY),
    }
}

const fn ghost(state: State, rest: Color32, hot: Color32, selected: Color32, full: bool) -> Look {
    match state {
        State::Hover => look(theme::BG_HOVER, CLEAR, W, CLEAR, hot),
        State::Pressed => look(theme::BG_PRESSED, CLEAR, W, CLEAR, hot),
        State::Focused => look(theme::BG, CLEAR, W, CLEAR, rest),
        State::Disabled if full => disabled(look(CLEAR, CLEAR, W, CLEAR, rest)),
        State::Selected if full => look(theme::BG_SELECTED, CLEAR, W, CLEAR, selected),
        _ => look(CLEAR, CLEAR, W, CLEAR, rest),
    }
}

fn sense(enabled: bool) -> Sense {
    if enabled {
        Sense::click()
    } else {
        Sense::hover()
    }
}

fn chrome(kind: Hierarchy, response: &Response, selected: bool, enabled: bool) -> Look {
    if selected && enabled && response.hovered() && !response.is_pointer_button_down_on() {
        return selected_hover(button_look(kind, State::Selected), button_look(kind, State::Hover));
    }
    button_look(kind, state_of(response, selected, enabled))
}

fn icon_px(size: Size) -> f32 {
    match size {
        Size::Xs => theme::LAYER_ICON_SIZE,
        Size::Sm | Size::Md => theme::LAYER_ICON_SIZE + theme::SPACE_FACT,
    }
}

fn draw_ring(ui: &Ui, rect: Rect, kind: Hierarchy, response: &Response) {
    let color = button_ring(kind, keyboard_focus(ui, response.id));
    if color == ring_color(RingTone::Brand) {
        super::focus::focus_ring(ui.painter(), rect, theme::RADIUS_CONTROL as f32);
    } else {
        paint_ring(ui.painter(), rect, theme::RADIUS_CONTROL as f32, color);
    }
}

/// Labelled button. Every size uses text-sm semibold. The caller owns the rect.
#[cfg_attr(not(test), allow(dead_code))]
pub fn button(ui: &mut Ui, rect: Rect, key: impl Key, label: &str, kind: Hierarchy, size: Size, enabled: bool) -> Response {
    let response = ui.interact(rect, ui.id().with(key), sense(enabled && ui.is_enabled()));
    let paint = chrome(kind, &response, false, enabled || !ui.is_enabled());
    paint_frame(ui.painter(), rect, theme::RADIUS_CONTROL, &paint);
    let font = match size {
        Size::Xs | Size::Sm | Size::Md => theme::font_text(theme::FONT_SIZE_TEXT_SM, theme::TextWeight::Semibold),
    };
    ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, label, font, paint.text);
    draw_ring(ui, rect, kind, &response);
    response
}

/// Icon-only button. `Hierarchy::Primary` is the send control; panels use `Hierarchy::Icon`.
#[allow(clippy::too_many_arguments)]
pub fn button_icon(ui: &mut Ui, rect: Rect, key: impl Key, icon: Icon, kind: Hierarchy, size: Size, enabled: bool, selected: bool) -> Response {
    let response = ui.interact(rect, ui.id().with(key), sense(enabled && ui.is_enabled()));
    let paint = chrome(kind, &response, selected, enabled || !ui.is_enabled());
    paint_frame(ui.painter(), rect, theme::RADIUS_CONTROL, &paint);
    widgets::paint_icon_in(ui.painter(), rect, icon, icon_px(size), paint.icon);
    draw_ring(ui, rect, kind, &response);
    response
}

/// Icon-button fill for callers that paint their own glyph. Uses the caller's corner radius.
pub fn paint_icon_bg(p: &Painter, rect: Rect, response: &Response, selected: bool, radius: CornerRadius) {
    let paint = chrome(Hierarchy::Icon, response, selected, true);
    if paint.fill != Color32::TRANSPARENT {
        p.rect_filled(rect, radius, paint.fill);
    }
    paint_ring(p, rect, radius.nw as f32, button_ring(Hierarchy::Icon, response.has_focus()));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn close_plates_follow_the_tab_fill() {
        let rest = close_look_over(ClosePlate::Primary, State::Default);
        assert_eq!(rest.fill, Color32::TRANSPARENT);
        assert_eq!(rest.icon, theme::ICON_QUATERNARY);
        let hot = close_look_over(ClosePlate::Hover, State::Hover);
        assert_eq!(hot.fill, theme::BG_SECONDARY_HOVER);
        assert_eq!(hot.icon, theme::ICON_QUATERNARY_HOVER);
        let pressed = close_look_over(ClosePlate::Selected, State::Pressed);
        assert_eq!(pressed.fill, theme::BG_PRESSED);
        assert_eq!(pressed.icon, theme::ICON_QUATERNARY_HOVER);
        let active = close_look_over(ClosePlate::Selected, State::Default);
        assert_eq!(active.fill, Color32::TRANSPARENT);
        assert_eq!(active.icon, theme::ICON_QUATERNARY);
    }

    #[test]
    fn toast_action_uses_the_secondary_text_ramp() {
        let rest = toast_action_look(State::Default);
        assert_eq!(rest.fill, Color32::TRANSPARENT);
        assert_eq!(rest.text, theme::TEXT_SECONDARY);
        let hot = toast_action_look(State::Hover);
        assert_eq!(hot.fill, theme::BG_SECONDARY_HOVER);
        assert_eq!(hot.text, theme::TEXT_SECONDARY_HOVER);
        let down = toast_action_look(State::Pressed);
        assert_eq!(down.fill, theme::BG_PRESSED);
        assert_eq!(down.text, theme::TEXT_SECONDARY);
    }
}
