//! Where the composer's chips go: which are visible, how wide each is, and how they wrap into rows.

use egui::{Pos2, Rect, Vec2};

use crate::agent_selection_chip::{composer_chip_label, selection_fingerprint};
use crate::terminal::TerminalState;
use crate::theme;
use crate::widgets;

pub const CHIP_MAX_W: f32 = 148.0;
pub const CHIP_MIN_W: f32 = 72.0;

pub(crate) fn selection_visible(state: &TerminalState) -> bool {
    let summary = &state.composer_selection;
    summary.count > 0 && state.active_composer().selection_chip_dismissed != Some(selection_fingerprint(summary))
}

/// The width of every chip, in the order they are drawn: selection, PRD, skills, pasted layers. Empty without chips.
pub fn chip_widths(p: &egui::Painter, state: &TerminalState) -> Vec<f32> {
    let font = chip_font();
    let mut widths = Vec::new();
    if selection_visible(state) {
        widths.push(tag_w(p, &composer_chip_label(&state.composer_selection).unwrap_or_default(), &font));
    }
    if state.active_composer().attach_prd {
        widths.push(tag_w(p, "PRD", &font));
    }
    let n = state.skills.selected_names.len();
    if n > 0 {
        widths.push(tag_w(p, &format!("{n} skills"), &font));
    }
    for chip in &state.active_composer().chips {
        widths.push(tag_w(p, &chip.name, &font));
    }
    widths
}

fn tag_w(p: &egui::Painter, label: &str, font: &egui::FontId) -> f32 {
    (widgets::text_width(p, label, font) + theme::INSET_BUTTON * 2.0 + theme::TARGET_MIN).clamp(CHIP_MIN_W, CHIP_MAX_W)
}

pub fn chip_rows(widths: &[f32], inner_w: f32) -> usize {
    wrap_rows(widths, inner_w)
}

fn wrap_rows(widths: &[f32], inner_w: f32) -> usize {
    if widths.is_empty() {
        return 0;
    }
    let mut rows = 1usize;
    let mut x = 0.0;
    for w in widths {
        if x > 0.0 && x + w > inner_w {
            rows += 1;
            x = 0.0;
        }
        x += w + theme::SPACE_ROW;
    }
    rows
}

pub fn place(x: &mut f32, y: &mut f32, row: Rect, w: f32) -> Rect {
    if *x > row.left() && *x + w > row.right() {
        *x = row.left();
        *y += theme::BADGE_MD + theme::SPACE_ROW;
    }
    let rect = Rect::from_min_size(Pos2::new(*x, *y), Vec2::new(w, theme::BADGE_MD));
    *x = rect.right() + theme::SPACE_ROW;
    rect
}

pub fn chip_font() -> egui::FontId {
    theme::font_text(theme::FONT_SIZE_TEXT_XS, theme::TextWeight::Medium)
}
