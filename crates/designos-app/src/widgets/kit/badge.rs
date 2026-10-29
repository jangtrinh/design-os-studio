use super::focus::{keyboard_focus, paint_ring};
use super::{look, paint_frame, Look, State};
use crate::icons::Icon;
use crate::theme;
use crate::widgets::{self, Key};
use egui::{Color32, Painter, Pos2, Rect, Response, Sense, Ui};

const W: f32 = theme::BORDER_WIDTH;
const CLEAR: Color32 = Color32::TRANSPARENT;

/// Count-badge tones. There is no brand tone.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(not(test), allow(dead_code))]
pub enum Tone {
    Neutral,
    Success,
    Warning,
    Error,
}

/// Status dot tones. Info, error and success only.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(not(test), allow(dead_code))]
pub enum DotTone {
    Info,
    Error,
    Success,
}

#[cfg_attr(not(test), allow(dead_code))]
pub const fn count_badge_look(tone: Tone) -> Look {
    match tone {
        Tone::Neutral => look(theme::BADGE_NEUTRAL_BG, theme::BADGE_NEUTRAL_BORDER, W, theme::BADGE_NEUTRAL_TEXT, CLEAR),
        Tone::Success => look(theme::BG_SUCCESS_PRIMARY, theme::BADGE_SUCCESS_BORDER, W, theme::BADGE_SUCCESS_TEXT, CLEAR),
        Tone::Warning => look(theme::BG_WARNING_PRIMARY, theme::BADGE_WARNING_BORDER, W, theme::BADGE_WARNING_TEXT, CLEAR),
        Tone::Error => look(theme::BG_ERROR_PRIMARY, theme::BADGE_ERROR_BORDER, W, theme::BADGE_ERROR_TEXT, CLEAR),
    }
}

#[cfg_attr(not(test), allow(dead_code))]
pub const fn tag_look(state: State) -> Look {
    match state {
        State::Hover => look(theme::BG, theme::BORDER_STRONG, W, theme::TEXT_SECONDARY, theme::ICON_QUATERNARY_HOVER),
        _ => look(theme::BG, theme::BORDER_STRONG, W, theme::TEXT_SECONDARY, theme::ICON_QUATERNARY),
    }
}

#[cfg_attr(not(test), allow(dead_code))]
pub const fn tag_ring(focused: bool) -> Color32 {
    if focused {
        theme::FOCUS_RING
    } else {
        CLEAR
    }
}

#[cfg_attr(not(test), allow(dead_code))]
pub const fn status_color(tone: DotTone) -> Color32 {
    match tone {
        DotTone::Info => theme::ICON_BRAND_PRIMARY,
        DotTone::Error => theme::DANGER,
        DotTone::Success => theme::SUCCESS,
    }
}

/// Status chip. Same tinted triple as a count badge, square control radius, no close mark.
#[allow(dead_code)]
pub fn status_tag(p: &Painter, rect: Rect, label: &str, tone: Tone) {
    let paint = count_badge_look(tone);
    paint_frame(p, rect, theme::RADIUS_CONTROL, &paint);
    let font = theme::font_text(theme::FONT_SIZE_TEXT_XS, theme::TextWeight::Medium);
    p.text(rect.center(), egui::Align2::CENTER_CENTER, label, font, paint.text);
}

#[allow(dead_code)]
pub fn count_badge(p: &Painter, rect: Rect, text: &str, tone: Tone) {
    let paint = count_badge_look(tone);
    paint_frame(p, rect, theme::radius_full(rect.height()).round() as u8, &paint);
    let font = theme::font_text(theme::FONT_SIZE_TEXT_XS, theme::TextWeight::Medium);
    p.text(rect.center(), egui::Align2::CENTER_CENTER, text, font, paint.text);
}

#[allow(dead_code)]
pub fn tag(ui: &mut Ui, rect: Rect, key: impl Key, label: &str) -> Response {
    let response = ui.interact(rect, ui.id().with(key), Sense::click());
    let hot = response.hovered() || response.is_pointer_button_down_on();
    let paint = tag_look(if hot { State::Hover } else { State::Default });
    paint_frame(ui.painter(), rect, theme::RADIUS_CONTROL, &paint);
    let font = theme::font_text(theme::FONT_SIZE_TEXT_XS, theme::TextWeight::Medium);
    let glyph = theme::FONT_SIZE_TEXT_SM;
    let right = theme::INSET_SEGMENT + theme::SPACE_FACT;
    let close = Rect::from_center_size(egui::pos2(rect.right() - right - glyph * 0.5, rect.center().y), egui::Vec2::splat(glyph));
    ui.painter().text(egui::pos2(rect.left() + theme::INSET_FRAME, rect.center().y), egui::Align2::LEFT_CENTER, label, font, paint.text);
    widgets::paint_icon_in(ui.painter(), close, Icon::Close, glyph, paint.icon);
    paint_ring(ui.painter(), rect, theme::RADIUS_CONTROL as f32, tag_ring(keyboard_focus(ui, response.id)));
    response
}

#[allow(dead_code)]
pub fn status_dot(p: &Painter, center: Pos2, tone: DotTone) {
    let paint = look(status_color(tone), Color32::TRANSPARENT, theme::BORDER_WIDTH, Color32::TRANSPARENT, Color32::TRANSPARENT);
    p.circle_filled(center, theme::SPACE_FACT, paint.fill);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_tones_use_the_tinted_triples() {
        let ok = count_badge_look(Tone::Success);
        assert_eq!(ok.fill, theme::BG_SUCCESS_PRIMARY);
        assert_eq!(ok.border, theme::BADGE_SUCCESS_BORDER);
        assert_eq!(ok.text, theme::BADGE_SUCCESS_TEXT);
        let warn = count_badge_look(Tone::Warning);
        assert_eq!(warn.fill, theme::BG_WARNING_PRIMARY);
        assert_eq!(warn.border, theme::BADGE_WARNING_BORDER);
        assert_eq!(warn.text, theme::BADGE_WARNING_TEXT);
        let err = count_badge_look(Tone::Error);
        assert_eq!(err.fill, theme::BG_ERROR_PRIMARY);
        assert_eq!(err.border, theme::BADGE_ERROR_BORDER);
        assert_eq!(err.text, theme::BADGE_ERROR_TEXT);
    }
}
