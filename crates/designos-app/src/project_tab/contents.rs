//! "On this page": the H2 and H3 headings of the open page, with the one being read marked.

use super::reader_layout::HeadingPos;
use crate::theme;
use crate::widgets;
use egui::{Align2, Pos2, Rect, Sense, Ui, Vec2};

const ROW_H: f32 = 26.0;
const TITLE_H: f32 = 18.0;
const TITLE_GAP: f32 = theme::SPACE_INLINE;
const TEXT_X: f32 = 12.0;
const SUB_TEXT_X: f32 = 24.0;
const TEXT_RIGHT_PAD: f32 = theme::SPACE_INLINE;

/// Pages with fewer headings than this show no contents.
pub const MIN_HEADINGS: usize = 2;

/// Paints the list in `rect` and returns the heading the reader clicked.
pub fn show(ui: &mut Ui, rect: Rect, headings: &[HeadingPos], active: Option<usize>) -> Option<usize> {
    if headings.len() < MIN_HEADINGS {
        return None;
    }
    let p = ui.painter().clone();
    let title = Rect::from_min_size(rect.min, Vec2::new(rect.width(), TITLE_H));
    p.text(
        title.left_center(),
        Align2::LEFT_CENTER,
        "On this page",
        theme::font_text(theme::FONT_SIZE_TEXT_XS, theme::TextWeight::Semibold),
        theme::TEXT_SECONDARY,
    );
    widgets::record(ui, "project-contents-title", title);
    let font = theme::font_text(theme::FONT_SIZE_TEXT_XS, theme::TextWeight::Medium);
    let mut clicked = None;
    for (i, h) in headings.iter().enumerate() {
        let row = Rect::from_min_size(Pos2::new(rect.left(), rect.top() + TITLE_H + TITLE_GAP + i as f32 * ROW_H), Vec2::new(rect.width(), ROW_H));
        let resp = ui.interact(row, ui.id().with(("project-contents", i)), Sense::click());
        widgets::labelled(ui, resp.id, egui::accesskit::Role::Link, &h.text);
        let is_active = active == Some(i);
        let color = if is_active {
            theme::TEXT
        } else if resp.hovered() {
            theme::TEXT_SECONDARY
        } else {
            theme::TEXT_TERTIARY
        };
        let rule = if is_active { theme::TEXT } else { theme::BORDER };
        widgets::vline(&p, row.left(), row.top(), row.bottom(), rule);
        let x = row.left() + if h.level >= 3 { SUB_TEXT_X } else { TEXT_X };
        let shown = widgets::elide(&p, &h.text, &font, row.right() - TEXT_RIGHT_PAD - x);
        p.text(Pos2::new(x, row.center().y), Align2::LEFT_CENTER, shown, font.clone(), color);
        widgets::record(ui, &format!("project-contents-{i}"), row);
        if resp.clicked() {
            clicked = Some(i);
        }
    }
    clicked
}
