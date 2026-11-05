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

    fn painted_shapes(rect: Rect, label: &str, tone: Tone) -> Vec<egui::epaint::Shape> {
        let ctx = egui::Context::default();
        theme::install_fonts(&ctx);
        let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(400.0, 300.0))), ..Default::default() };
        let out = ctx.run_ui(raw, |ui| status_tag(ui.painter(), rect, label, tone));
        out.shapes.into_iter().map(|clipped| clipped.shape).collect()
    }

    #[test]
    fn status_tag_paints_the_tinted_triple_with_a_control_radius() {
        let rect = Rect::from_min_size(egui::pos2(20.0, 20.0), egui::vec2(64.0, theme::BADGE_MD));
        for tone in [Tone::Neutral, Tone::Success, Tone::Warning, Tone::Error] {
            let want = count_badge_look(tone);
            let shapes = painted_shapes(rect, "New", tone);
            let rects: Vec<&egui::epaint::RectShape> = shapes
                .iter()
                .filter_map(|shape| match shape {
                    egui::epaint::Shape::Rect(r) if r.rect == rect => Some(r),
                    _ => None,
                })
                .collect();
            let fill = rects.iter().find(|r| r.fill != Color32::TRANSPARENT).unwrap_or_else(|| panic!("{tone:?}: a fill at the chip rect"));
            assert_eq!(fill.fill, want.fill, "{tone:?} fill");
            assert_eq!(fill.corner_radius, theme::cr(theme::RADIUS_CONTROL), "{tone:?}: the chip uses the control radius, not a pill");
            let stroke = rects.iter().find(|r| r.stroke.width > 0.0).unwrap_or_else(|| panic!("{tone:?}: a border at the chip rect"));
            assert_eq!((stroke.stroke.color, stroke.stroke.width), (want.border, W), "{tone:?} border");
            let text = shapes
                .iter()
                .find_map(|shape| match shape {
                    egui::epaint::Shape::Text(t) if t.galley.text() == "New" => Some(t),
                    _ => None,
                })
                .unwrap_or_else(|| panic!("{tone:?}: the label"));
            let section = &text.galley.job.sections[0];
            assert_eq!(section.format.color, want.text, "{tone:?} label colour");
            assert_eq!(section.format.font_id, theme::font_text(theme::FONT_SIZE_TEXT_XS, theme::TextWeight::Medium), "{tone:?} label font");
            assert!(rect.contains(Rect::from_min_size(text.pos, text.galley.size()).center()), "{tone:?}: the label is centred in the chip");
        }
    }
}
