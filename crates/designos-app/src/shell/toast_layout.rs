//! Where the toast and its action sit. The toast wraps a long message (an exported file's full path) inside the
//! window instead of running past its edge, and grows upward from a fixed bottom edge.

use crate::theme;
use egui::epaint::text::Galley;
use egui::{Color32, FontId, Painter, Pos2, Rect, Vec2};
use std::sync::Arc;

/// Distance between the toast and the window's side edges at the widest.
const SIDE_MARGIN: f32 = 24.0;
/// The widest the toast grows before it wraps.
const MAX_WIDTH: f32 = 720.0;
/// Left edge of the message: status dot, then a gap.
const PAD_LEFT: f32 = 28.0;
const PAD_RIGHT: f32 = 16.0;
const PAD_VERTICAL: f32 = 12.0;
/// Space between the message and the action.
const ACTION_GAP: f32 = 12.0;
/// The action's inner padding (its pointer target is `ACTION_HEIGHT` tall, at least 24 px).
const ACTION_PAD: f32 = 8.0;
const ACTION_HEIGHT: f32 = theme::CONTROL_XS;
/// Narrowest message column, so a tiny window still gets a readable toast.
const MIN_TEXT_WIDTH: f32 = 80.0;
/// Narrowest toast, before the window's side margins squeeze it further.
const MIN_WIDTH: f32 = 240.0;
/// Centre of the status dot, from the toast's left edge.
const DOT_INSET: f32 = 16.0;
/// Both side margins together.
const SIDE_MARGINS: f32 = 2.0 * SIDE_MARGIN;
/// The action's padding on both sides.
const ACTION_PADS: f32 = 2.0 * ACTION_PAD;
/// The padding above and below the message.
const PADS_VERTICAL: f32 = 2.0 * PAD_VERTICAL;

pub(super) struct ToastLayout {
    /// The whole toast.
    pub rect: Rect,
    pub text: Arc<Galley>,
    /// Top-left of the message.
    pub text_pos: Pos2,
    /// Centre of the status dot.
    pub dot: Pos2,
    /// The action's pointer target and its label, when the toast has an action.
    pub action: Option<(Rect, Arc<Galley>)>,
    /// A second action to the left of `action`, when the toast has two.
    pub extra: Option<(Rect, Arc<Galley>)>,
}

/// Lays the toast out inside `screen`. Its bottom edge is `TOAST_BOTTOM` above the window's bottom. The message uses
/// `font` and the actions use `action_font`.
pub(super) fn layout(
    p: &Painter,
    text: &str,
    font: &FontId,
    action_font: &FontId,
    screen: Rect,
    action_label: Option<&str>,
    extra_label: Option<&str>,
) -> ToastLayout {
    let max_w = (screen.width() - SIDE_MARGINS).clamp(MIN_WIDTH, MAX_WIDTH);
    let action_galley = action_label.map(|label| p.layout_no_wrap(label.to_string(), action_font.clone(), Color32::PLACEHOLDER));
    let extra_galley = extra_label.map(|label| p.layout_no_wrap(label.to_string(), action_font.clone(), Color32::PLACEHOLDER));
    let action_w = action_galley.as_ref().map_or(0.0, |g| g.size().x + ACTION_PADS);
    let extra_w = extra_galley.as_ref().map_or(0.0, |g| g.size().x + ACTION_PADS);
    let slots = (action_w > 0.0) as u8 + (extra_w > 0.0) as u8;
    let reserved = PAD_LEFT + PAD_RIGHT + action_w + extra_w + if slots > 0 { ACTION_GAP * slots as f32 } else { 0.0 };
    let wrap_w = (max_w - reserved).max(MIN_TEXT_WIDTH);
    let galley = p.layout(text.to_string(), font.clone(), Color32::PLACEHOLDER, wrap_w);

    let w = reserved + galley.size().x;
    let h = (galley.size().y + PADS_VERTICAL).max(theme::TOAST_HEIGHT);
    let rect = Rect::from_min_size(Pos2::new(screen.center().x - w / 2.0, screen.bottom() - theme::TOAST_BOTTOM - h), Vec2::new(w, h));

    let text_pos = Pos2::new(rect.left() + PAD_LEFT, rect.center().y - galley.size().y / 2.0);
    let first_row_h = galley.size().y / galley.rows.len().max(1) as f32;
    let dot = Pos2::new(rect.left() + DOT_INSET, text_pos.y + first_row_h / 2.0);
    let action = action_galley.map(|label| {
        let target = Rect::from_center_size(Pos2::new(rect.right() - PAD_RIGHT - action_w / 2.0, rect.center().y), Vec2::new(action_w, ACTION_HEIGHT));
        (target, label)
    });
    let extra = extra_galley.map(|label| {
        let right = action.as_ref().map(|(r, _)| r.left() - ACTION_GAP).unwrap_or(rect.right() - PAD_RIGHT);
        let target = Rect::from_center_size(Pos2::new(right - extra_w / 2.0, rect.center().y), Vec2::new(extra_w, ACTION_HEIGHT));
        (target, label)
    });
    ToastLayout { rect, text: galley, text_pos, dot, action, extra }
}
