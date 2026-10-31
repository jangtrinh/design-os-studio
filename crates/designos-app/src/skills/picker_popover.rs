//! The Skills popover: a checkbox, a name and one line of description per skill. Counts, sources and paths live in tooltips.

use super::{SkillsAttachmentState, MAX_COMBINED_TOKENS, MAX_SKILLS_PER_RUN};
use crate::icons::Icon;
use crate::theme;
use crate::widgets::{self, kit};
use egui::{Align2, Id, Key, Pos2, Rect, Sense, Ui, UiBuilder, Vec2};

/// A budget line shows once the run is within 10 % of a limit.
const BUDGET_NEAR_TOKENS: usize = MAX_COMBINED_TOKENS / 10 * 9;
const ROW_H: f32 = theme::CONTROL_MD + theme::SPACE_ROW;

/// Where the composer's skills chip sits, `None` without one. A press on the chip is not an outside press: the chip opens and closes the popover itself.
fn picker_anchor_id() -> Id {
    Id::new("agents-skills-picker-anchor-rect")
}

/// Records where the chip is drawn this frame, `None` when it is not. The value is overwritten in place, so no frame allocates.
pub(crate) fn remember_picker_anchor(ctx: &egui::Context, anchor: Option<Rect>) {
    ctx.data_mut(|d| *d.get_temp_mut_or_default::<Option<Rect>>(picker_anchor_id()) = anchor);
}

/// True when `Esc` or a press outside the popover and its chip asks it to close.
fn dismissed(ui: &Ui, popover: Rect) -> bool {
    if ui.input(|i| i.key_pressed(Key::Escape)) {
        return true;
    }
    if !ui.input(|i| i.pointer.any_pressed()) {
        return false;
    }
    let Some(pos) = ui.input(|i| i.pointer.interact_pos()) else {
        return false;
    };
    let on_chip = ui.ctx().data(|d| d.get_temp::<Option<Rect>>(picker_anchor_id())).flatten().is_some_and(|chip| chip.contains(pos));
    !popover.contains(pos) && !on_chip
}

/// `7300` as `7,300`.
pub(super) fn thousands(n: usize) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

impl SkillsAttachmentState {
    /// The budget line under the rows, only near or over a limit: the violation text over one, else the totals.
    pub(super) fn budget_line(&self) -> Option<(String, bool)> {
        if let Some(violation) = self.limit_violation() {
            return Some((violation, true));
        }
        let count = self.selected_entries().len();
        let tokens = self.combined_tokens();
        (tokens >= BUDGET_NEAR_TOKENS || count >= MAX_SKILLS_PER_RUN)
            .then(|| (format!("{} of {} tokens · {count} of {MAX_SKILLS_PER_RUN} skills", thousands(tokens), thousands(MAX_COMBINED_TOKENS)), false))
    }

    /// Renders the floating Skills popover inside `panel_rect`. Choosing a skill only changes the selection: skills reach an
    /// agent with its next send (`delivery`), not when they are ticked.
    pub fn show_picker_popover(&mut self, ui: &mut Ui, panel_rect: Rect, anchor_bottom_y: f32) {
        if !self.picker_open {
            self.picker_drawn = false;
            return;
        }
        self.poll_watcher(Some(ui.ctx()));

        let budget = self.budget_line();
        let header_h = theme::MENU_ITEM_HEIGHT;
        let footer_h = if budget.is_some() { theme::MENU_ITEM_HEIGHT } else { 0.0 };
        let pad = theme::INSET_FRAME;
        let pop_w = (panel_rect.width() - theme::INSET_PANEL).clamp(theme::CONTROL_MD * 5.0 + theme::SPACE_INLINE * 3.0, theme::CONTROL_MD * 8.0);
        let want_h = pad * 2.0 + header_h + self.entries.len() as f32 * ROW_H + footer_h;
        let pop_h = want_h.min((panel_rect.height() - theme::INSET_PANEL * 2.0).max(theme::CONTROL_MD * 4.0));
        let pop_x = (panel_rect.left() + theme::SPACE_INLINE).min(panel_rect.right() - pop_w - theme::SPACE_ROW);
        let pop_y = (anchor_bottom_y - pop_h - theme::SPACE_INLINE).max(panel_rect.top() + theme::CONTROL_MD);
        let pop_r = Rect::from_min_size(Pos2::new(pop_x, pop_y), Vec2::new(pop_w, pop_h));
        let inner = pop_r.shrink(pad);

        // It closes on `Esc` and on a press outside, which is what lets it count as an open popup that owns the keyboard. The pass
        // that opened it is skipped: the press that picked its item reaches it in that very pass.
        if self.picker_drawn && dismissed(ui, pop_r) {
            self.picker_open = false;
            self.picker_drawn = false;
            return;
        }
        self.picker_drawn = true;
        widgets::popup_open::mark_open(ui.ctx());

        let p = ui.painter().clone();
        kit::popover_surface(&p, pop_r);
        widgets::record(ui, "agents-skills-picker-popover", pop_r);

        let header = Rect::from_min_size(inner.min, Vec2::new(inner.width(), header_h));
        let close_r = Rect::from_center_size(Pos2::new(header.right() - theme::TARGET_MIN * 0.5, header.center().y), Vec2::splat(theme::TARGET_MIN));
        let title_r = Rect::from_min_max(Pos2::new(header.left() + theme::SPACE_INLINE, header.top()), Pos2::new(close_r.left(), header.bottom()));
        p.text(Pos2::new(title_r.left(), title_r.center().y), Align2::LEFT_CENTER, "Attach skills", theme::font_large_strong(), theme::TEXT);
        let library_root = &self.library_root;
        ui.interact(title_r, Id::new("agents-skills-picker-title"), Sense::hover()).on_hover_ui(|ui| {
            ui.label(format!("User skills: {}/skills/<name>/SKILL.md", library_root.display()));
        });
        let close = kit::button_icon(ui, close_r, "agents-skills-picker-close", Icon::Close, kit::Hierarchy::Icon, kit::Size::Xs, true, false);
        widgets::record(ui, "agents-skills-picker-close", close_r);
        widgets::labelled(ui, close.id, egui::accesskit::Role::Button, "Close skills");
        if close.clicked() {
            self.picker_open = false;
            return;
        }

        let rows_clip = Rect::from_min_max(Pos2::new(inner.left(), header.bottom()), Pos2::new(inner.right(), inner.bottom() - footer_h));
        let max_scroll = (self.entries.len() as f32 * ROW_H - rows_clip.height()).max(0.0);
        if ui.rect_contains_pointer(pop_r) {
            let delta_y = ui.input(|i| i.smooth_scroll_delta.y);
            if delta_y.abs() > 0.0 {
                self.scroll_offset = (self.scroll_offset - delta_y).clamp(0.0, max_scroll);
            }
        }
        self.scroll_offset = self.scroll_offset.clamp(0.0, max_scroll);

        let mut rows_ui = ui.new_child(UiBuilder::new().max_rect(rows_clip));
        rows_ui.set_clip_rect(rows_clip.intersect(ui.clip_rect()));
        let mut toggle_target: Option<String> = None;
        let mut ry = rows_clip.top() - self.scroll_offset;
        for entry in &self.entries {
            let row_r = Rect::from_min_size(Pos2::new(rows_clip.left(), ry), Vec2::new(rows_clip.width(), ROW_H - theme::SPACE_ROW));
            ry += ROW_H;
            if row_r.bottom() < rows_clip.top() || row_r.top() > rows_clip.bottom() {
                continue;
            }
            let valid = entry.status.is_valid();
            let sel_key = entry.selection_key();
            let row_id = format!("agents-skill-row-{}", sel_key.replace(':', "-"));
            let name = entry.display_name(&self.entries);
            let hot = rows_ui.rect_contains_pointer(row_r.intersect(rows_clip));
            if hot {
                rows_ui.painter().rect_filled(row_r, theme::cr(theme::RADIUS_CONTROL), theme::BG_MENU_HOVER);
            }
            let content = row_r.shrink2(Vec2::new(theme::SPACE_INLINE, 0.0));
            let check = kit::checkbox(&mut rows_ui, content, row_id.as_str(), self.is_entry_selected(entry), valid);
            widgets::record(ui, &row_id, row_r.intersect(rows_clip));
            widgets::labelled(ui, check.id, egui::accesskit::Role::CheckBox, &name);
            check.clone().on_hover_ui(|ui| {
                ui.label(format!("{name} @{} · {} · {} tokens", entry.version, entry.source.badge_label().to_lowercase(), entry.token_count));
            });
            if valid && check.clicked() {
                toggle_target = Some(sel_key);
            }
            let text_left = content.left() + theme::LAYER_ICON_SIZE + theme::SPACE_INLINE;
            let mut text_right = content.right();
            let rp = rows_ui.painter().clone();
            if let Some(label) = entry.status.error_badge() {
                let w = widgets::text_width(&rp, label, &theme::font_caption()) + theme::SPACE_INLINE * 2.0;
                let badge = Rect::from_center_size(
                    Pos2::new(content.right() - w * 0.5, row_r.top() + theme::SPACE_GROUP + theme::SPACE_ROW),
                    Vec2::new(w, theme::BADGE_SM),
                );
                kit::count_badge(&rp, badge, label, kit::Tone::Error);
                text_right = badge.left() - theme::SPACE_INLINE;
            }
            let name_font = theme::font_text(theme::FONT_SIZE_BODY, theme::TextWeight::Medium);
            let name_color = if valid { theme::TEXT } else { theme::TEXT_ERROR };
            let name_shown = widgets::elide(&rp, &name, &name_font, (text_right - text_left).max(theme::CONTROL_MD));
            rp.text(Pos2::new(text_left, row_r.top() + theme::SPACE_GROUP), Align2::LEFT_CENTER, name_shown, name_font, name_color);
            let desc = widgets::elide(&rp, &entry.description, &theme::font_caption(), (content.right() - text_left).max(theme::CONTROL_MD));
            rp.text(
                Pos2::new(text_left, row_r.top() + theme::SPACE_GROUP + theme::LINE_BODY),
                Align2::LEFT_CENTER,
                desc,
                theme::font_caption(),
                theme::TEXT_SECONDARY,
            );
        }
        if let Some(target) = toggle_target {
            self.toggle_skill(&target);
        }

        if let Some((text, over)) = budget {
            let line = Rect::from_min_max(Pos2::new(inner.left(), inner.bottom() - footer_h), inner.max);
            let shown = widgets::elide(&p, &text, &theme::font_caption(), line.width() - theme::SPACE_INLINE * 2.0);
            let color = if over { theme::TEXT_ERROR } else { theme::TEXT_WARNING };
            p.text(Pos2::new(line.left() + theme::SPACE_INLINE, line.center().y), Align2::LEFT_CENTER, shown, theme::font_caption(), color);
            widgets::record(ui, "agents-skills-limit-warning", line);
        }
    }
}
