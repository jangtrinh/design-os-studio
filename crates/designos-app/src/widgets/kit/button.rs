use super::focus::{ring_color, RingTone};
use super::{disabled, look, Look, State};
use crate::theme;
use egui::Color32;

/// Button families. Destructive and its secondary form keep the error ramp.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hierarchy {
    Primary,
    Secondary,
    Tertiary,
    Destructive,
    SecondaryDestructive,
    Icon,
    Tool,
    Close,
}

const W: f32 = theme::BORDER_WIDTH;
const CLEAR: Color32 = Color32::TRANSPARENT;

pub fn button_look(kind: Hierarchy, state: State) -> Look {
    let state = if state == State::Loading { State::Hover } else { state };
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

pub fn button_ring(kind: Hierarchy, state: State) -> Color32 {
    if state != State::Focused {
        return CLEAR;
    }
    let tone = match kind {
        Hierarchy::Destructive | Hierarchy::SecondaryDestructive => RingTone::Error,
        _ => RingTone::Brand,
    };
    ring_color(tone)
}

fn solid(state: State, fill: Color32, hover: Color32, pressed: Color32, label: Color32) -> Look {
    match state {
        State::Hover => look(hover, CLEAR, W, label, label),
        State::Pressed => look(pressed, CLEAR, W, label, label),
        State::Disabled => disabled(look(fill, CLEAR, W, label, label)),
        _ => look(fill, CLEAR, W, label, label),
    }
}

fn secondary(state: State) -> Look {
    let border = theme::BORDER_STRONG;
    match state {
        State::Hover => look(theme::BG_HOVER, border, W, theme::TEXT_SECONDARY_HOVER, theme::ICON_QUATERNARY_HOVER),
        State::Pressed => look(theme::BG_PRESSED, border, W, theme::TEXT_SECONDARY_HOVER, theme::ICON_QUATERNARY_HOVER),
        State::Disabled => disabled(look(theme::BG, border, W, theme::TEXT_SECONDARY, theme::ICON_QUATERNARY)),
        _ => look(theme::BG, border, W, theme::TEXT_SECONDARY, theme::ICON_QUATERNARY),
    }
}

fn tertiary(state: State) -> Look {
    match state {
        State::Hover => look(theme::BG_HOVER, CLEAR, W, theme::TEXT_TERTIARY_HOVER, theme::ICON_QUATERNARY_HOVER),
        State::Pressed => look(theme::BG_PRESSED, CLEAR, W, theme::TEXT_TERTIARY_HOVER, theme::ICON_QUATERNARY_HOVER),
        State::Focused => look(theme::BG, CLEAR, W, theme::TEXT_TERTIARY, theme::ICON_QUATERNARY),
        State::Disabled => disabled(look(CLEAR, CLEAR, W, theme::TEXT_TERTIARY, theme::ICON_QUATERNARY)),
        _ => look(CLEAR, CLEAR, W, theme::TEXT_TERTIARY, theme::ICON_QUATERNARY),
    }
}

fn secondary_destructive(state: State) -> Look {
    match state {
        State::Hover => look(theme::BG_ERROR_PRIMARY, theme::BORDER_ERROR_SUBTLE, W, theme::TEXT_ERROR_HOVER, theme::TEXT_ERROR_HOVER),
        State::Pressed => look(theme::BG_ERROR_PRIMARY, theme::BORDER_ERROR, W, theme::TEXT_ERROR_HOVER, theme::TEXT_ERROR_HOVER),
        State::Disabled => disabled(look(theme::BG, theme::BORDER_ERROR_SUBTLE, W, theme::TEXT_ERROR, theme::TEXT_ERROR)),
        _ => look(theme::BG, theme::BORDER_ERROR_SUBTLE, W, theme::TEXT_ERROR, theme::TEXT_ERROR),
    }
}

fn ghost(state: State, rest: Color32, hot: Color32, selected: Color32, full: bool) -> Look {
    match state {
        State::Hover => look(theme::BG_HOVER, CLEAR, W, CLEAR, hot),
        State::Pressed => look(theme::BG_PRESSED, CLEAR, W, CLEAR, hot),
        State::Focused => look(theme::BG, CLEAR, W, CLEAR, rest),
        State::Disabled if full => disabled(look(CLEAR, CLEAR, W, CLEAR, rest)),
        State::Selected if full => look(theme::BG_SELECTED, CLEAR, W, CLEAR, selected),
        _ => look(CLEAR, CLEAR, W, CLEAR, rest),
    }
}
