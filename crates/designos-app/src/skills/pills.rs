//! Count pills of the Skills button and the picker header (`#573`): a number is a pill next to a label, not text in parentheses.

use crate::theme;
use crate::widgets;
use egui::{Align2, Color32, Painter, Pos2, Rect, Vec2};

/// Width of a pill holding `text`.
pub(crate) fn pill_width(p: &Painter, text: &str) -> f32 {
    widgets::text_width(p, text, &theme::font_caption_strong()) + theme::SPACE_INLINE
}

/// Paints a pill with `text` whose left edge is `left`, centred on `center_y`, and returns its rectangle.
pub(crate) fn paint_pill(p: &Painter, left: f32, center_y: f32, text: &str, fill: Color32, fg: Color32) -> Rect {
    let rect = Rect::from_min_size(Pos2::new(left, center_y - theme::INSET_PANEL * 0.5), Vec2::new(pill_width(p, text), theme::INSET_PANEL));
    p.rect_filled(rect, theme::cr_pill(rect.height()), fill);
    p.text(rect.center(), Align2::CENTER_CENTER, text, theme::font_caption_strong(), fg);
    rect
}
