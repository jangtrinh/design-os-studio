use super::{look, Look, State};
use crate::theme;
use egui::Color32;

const W: f32 = theme::BORDER_WIDTH;
const CLEAR: Color32 = Color32::TRANSPARENT;

/// Count-badge tones. There is no brand tone.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    Neutral,
    Success,
    Warning,
    Error,
}

/// Status dot tones. Info, error and success only.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DotTone {
    Info,
    Error,
    Success,
}

pub const fn count_badge(tone: Tone) -> Look {
    match tone {
        Tone::Neutral => look(theme::BADGE_NEUTRAL_BG, theme::BADGE_NEUTRAL_BORDER, W, theme::BADGE_NEUTRAL_TEXT, CLEAR),
        Tone::Success => look(theme::BG_SUCCESS_PRIMARY, theme::BADGE_SUCCESS_BORDER, W, theme::BADGE_SUCCESS_TEXT, CLEAR),
        Tone::Warning => look(theme::BG_WARNING_PRIMARY, theme::BADGE_WARNING_BORDER, W, theme::BADGE_WARNING_TEXT, CLEAR),
        Tone::Error => look(theme::BG_ERROR_PRIMARY, theme::BADGE_ERROR_BORDER, W, theme::BADGE_ERROR_TEXT, CLEAR),
    }
}

pub const fn tag_look(state: State) -> Look {
    match state {
        State::Hover => look(theme::BG, theme::BORDER_STRONG, W, theme::TEXT_SECONDARY, theme::ICON_QUATERNARY_HOVER),
        _ => look(theme::BG, theme::BORDER_STRONG, W, theme::TEXT_SECONDARY, theme::ICON_QUATERNARY),
    }
}

pub const fn tag_ring(focused: bool) -> Color32 {
    if focused {
        theme::FOCUS_RING
    } else {
        CLEAR
    }
}

pub const fn status_dot(tone: DotTone) -> Color32 {
    match tone {
        DotTone::Info => theme::ICON_BRAND_PRIMARY,
        DotTone::Error => theme::DANGER,
        DotTone::Success => theme::SUCCESS,
    }
}
