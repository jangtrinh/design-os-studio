//! `@`-mention picker popup UI for the Agent composer and terminal header (`#655`).
//!
//! Uses only [`crate::theme`] layout, spacing, typography, and color tokens.

use crate::agent_mention::{detect_at_query, Mention, MentionCandidate, MentionCategory, MentionSearchJob, DEFAULT_FRAME_BUDGET};
use crate::icons::{self, Icon};
use crate::theme;
use crate::widgets;
use designos_doc::{Document, NodeId};
use egui::{Align2, Context, Id, Key, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, Vec2};

/// Maximum visible candidate rows before scrolling (`#655` §4).
pub const MAX_VISIBLE_ROWS: usize = 8;

/// Copy shown when no layer, page, component, variable, or style matches the query (`#655` §4).
pub const EMPTY_MATCH_COPY: &str = "Nothing in this file matches";

/// Where the popup was opened from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum MentionPopupMode {
    /// Triggered by typing `@<query>` inside the composer input.
    #[default]
    Composer,
    /// Opened from the `Mention` button in the terminal header (`#655` §4).
    TerminalHeader,
}

/// Persistent state for the `@`-mention popup on [`crate::terminal::TerminalState`].
#[derive(Clone, Debug, Default)]
pub struct MentionPopupState {
    pub open: bool,
    pub mode: MentionPopupMode,
    /// Byte offset of `@` in `input_line` when `mode == MentionPopupMode::Composer`.
    pub at_byte: usize,
    /// Current query text after `@` (or typed into the terminal-mode filter).
    pub query: String,
    /// Currently highlighted candidate index (`0..candidates.len()`).
    pub active_index: usize,
    /// Vertical scroll offset in pixels.
    pub scroll_y: f32,
    /// Token `(at_byte, query)` dismissed with `Escape` so the popup stays closed until the user edits the token.
    pub dismissed_token: Option<(usize, String)>,
    /// Incremental budgeted search job for the current query.
    pub search_job: Option<MentionSearchJob>,
    /// Ranked candidates from `search_job`.
    pub candidates: Vec<MentionCandidate>,
    doc_epoch: Option<(u64, NodeId, String)>,
}

/// Outcome of one frame of [`MentionPopupState`] interaction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MentionPopupOutcome {
    None,
    /// User picked a candidate in composer mode (`input_line` updated and [`Mention`] returned).
    PickedComposer(Mention),
    /// User picked a candidate in terminal-header mode (paste reference into active PTY tab).
    PickedTerminal(Mention),
    /// User pressed `Esc` to close the popup without changing text.
    Dismissed,
}

impl MentionPopupState {
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Opens or toggles the popup in terminal-header mode (`Mention` button).
    pub fn toggle_terminal_mode(&mut self, doc: &Document, current_page: NodeId) {
        if self.open && self.mode == MentionPopupMode::TerminalHeader {
            self.close();
            return;
        }
        self.open = true;
        self.mode = MentionPopupMode::TerminalHeader;
        self.at_byte = 0;
        self.query.clear();
        self.active_index = 0;
        self.scroll_y = 0.0;
        self.dismissed_token = None;
        self.refresh_search(doc, current_page, true);
    }

    pub fn close(&mut self) {
        self.open = false;
        self.search_job = None;
    }

    /// Synchronizes the popup state with `input_line` in composer mode (`cursor_byte` defaults to `input_line.len()`).
    pub fn sync_from_composer_text(&mut self, input_line: &str, cursor_byte: usize, doc: &Document, current_page: NodeId) {
        if self.mode == MentionPopupMode::TerminalHeader && self.open {
            self.step_search_if_needed(doc, current_page);
            return;
        }
        match detect_at_query(input_line, cursor_byte) {
            Some((at_byte, q)) => {
                if let Some((d_at, ref d_q)) = self.dismissed_token {
                    if d_at == at_byte && d_q == q {
                        self.open = false;
                        return;
                    }
                }
                self.dismissed_token = None;
                let query_changed = !self.open || self.mode != MentionPopupMode::Composer || self.at_byte != at_byte || self.query != q;
                self.open = true;
                self.mode = MentionPopupMode::Composer;
                self.at_byte = at_byte;
                if query_changed {
                    self.query = q.to_string();
                    self.active_index = 0;
                    self.scroll_y = 0.0;
                    self.refresh_search(doc, current_page, true);
                } else {
                    self.step_search_if_needed(doc, current_page);
                }
            }
            None => {
                self.dismissed_token = None;
                if self.mode == MentionPopupMode::Composer {
                    self.close();
                }
            }
        }
    }

    fn refresh_search(&mut self, doc: &Document, current_page: NodeId, force_new: bool) {
        let key = (doc.nodes.len() as u64 ^ (doc.variables.len() as u64) << 16 ^ (doc.styles.len() as u64) << 32, current_page, self.query.clone());
        if !force_new && self.doc_epoch.as_ref() == Some(&key) {
            self.step_search_if_needed(doc, current_page);
            return;
        }
        self.doc_epoch = Some(key);
        let job = MentionSearchJob::start(doc, current_page, &self.query, DEFAULT_FRAME_BUDGET);
        self.candidates = job.candidates();
        if self.active_index >= self.candidates.len() {
            self.active_index = 0;
        }
        self.search_job = Some(job);
    }

    fn step_search_if_needed(&mut self, doc: &Document, current_page: NodeId) {
        if let Some(job) = &mut self.search_job {
            if !job.is_finished() {
                job.step(doc, current_page, DEFAULT_FRAME_BUDGET);
                self.candidates = job.candidates();
                if self.active_index >= self.candidates.len() {
                    self.active_index = self.candidates.len().saturating_sub(1);
                }
            }
        } else {
            self.refresh_search(doc, current_page, true);
        }
    }

    /// Consumes `ArrowUp`, `ArrowDown`, `Enter`, `Tab`, and `Escape` from `ctx` while the popup
    /// is open so the underlying `TextEdit` or PTY does not also act on them.
    pub fn handle_keyboard(
        &mut self,
        ctx: &Context,
        input_line: &mut String,
        active_mentions: &mut Vec<Mention>,
        doc: &Document,
        current_page: NodeId,
    ) -> MentionPopupOutcome {
        if !self.open {
            return MentionPopupOutcome::None;
        }

        let mut move_delta: isize = 0;
        let mut pick_active = false;
        let mut dismiss = false;
        let mut term_text_added = String::new();
        let mut term_backspace = false;
        let is_term_mode = self.mode == MentionPopupMode::TerminalHeader;

        ctx.input_mut(|i| {
            i.events.retain(|ev| match ev {
                egui::Event::Key { key: Key::ArrowDown, pressed: true, .. } => {
                    move_delta += 1;
                    false
                }
                egui::Event::Key { key: Key::ArrowUp, pressed: true, .. } => {
                    move_delta -= 1;
                    false
                }
                egui::Event::Key { key: Key::Enter | Key::Tab, pressed: true, .. } => {
                    pick_active = true;
                    false
                }
                egui::Event::Key { key: Key::Escape, pressed: true, .. } => {
                    dismiss = true;
                    false
                }
                egui::Event::Key { key: Key::Backspace, pressed: true, .. } if is_term_mode => {
                    term_backspace = true;
                    false
                }
                egui::Event::Text(t) if is_term_mode && !i.modifiers.command && !i.modifiers.ctrl => {
                    term_text_added.push_str(t);
                    false
                }
                _ => true,
            });
        });

        if dismiss {
            if self.mode == MentionPopupMode::Composer {
                self.dismissed_token = Some((self.at_byte, self.query.clone()));
            }
            self.close();
            return MentionPopupOutcome::Dismissed;
        }

        if is_term_mode && (term_backspace || !term_text_added.is_empty()) {
            if term_backspace {
                self.query.pop();
            }
            if !term_text_added.is_empty() {
                let clean: String = term_text_added.chars().filter(|c| !c.is_control()).collect();
                self.query.push_str(&clean);
            }
            self.active_index = 0;
            self.scroll_y = 0.0;
            self.refresh_search(doc, current_page, true);
        }

        if move_delta != 0 && !self.candidates.is_empty() {
            let len = self.candidates.len() as isize;
            let next = (self.active_index as isize + move_delta).rem_euclid(len) as usize;
            self.active_index = next;
        }

        if pick_active {
            if let Some(cand) = self.candidates.get(self.active_index).cloned() {
                return self.apply_pick(&cand, input_line, active_mentions);
            }
        }

        MentionPopupOutcome::None
    }

    /// Replaces `@<query>` in `input_line` with `@<label> ` (in composer mode), records the
    /// [`Mention`] in `active_mentions`, and closes the popup.
    pub fn apply_pick(&mut self, cand: &MentionCandidate, input_line: &mut String, active_mentions: &mut Vec<Mention>) -> MentionPopupOutcome {
        let mention = Mention::from_candidate(cand);
        let mode = self.mode;
        if mode == MentionPopupMode::Composer {
            let start = self.at_byte.min(input_line.len());
            let end = (start + 1 + self.query.len()).min(input_line.len());
            let replacement = format!("@{} ", mention.label);
            if input_line.is_char_boundary(start) && input_line.is_char_boundary(end) && start <= end {
                input_line.replace_range(start..end, &replacement);
            } else {
                input_line.push_str(&replacement);
            }
            active_mentions.push(mention.clone());
            self.close();
            MentionPopupOutcome::PickedComposer(mention)
        } else {
            self.close();
            MentionPopupOutcome::PickedTerminal(mention)
        }
    }

    /// Renders the `@`-mention popup anchored above `anchor_rect` (in composer mode) or below
    /// `anchor_rect` (in terminal-header mode).
    pub fn show(
        &mut self,
        ui: &mut Ui,
        panel_rect: Rect,
        anchor_rect: Rect,
        input_line: &mut String,
        active_mentions: &mut Vec<Mention>,
    ) -> MentionPopupOutcome {
        if !self.open {
            return MentionPopupOutcome::None;
        }

        if self.search_job.as_ref().is_some_and(|j| !j.is_finished()) {
            ui.ctx().request_repaint();
        }

        let inset = theme::INSET_POPOVER;
        let row_h = theme::CONTROL_ROW;
        let heading_h = theme::SPACE_GROUP + 14.0 + theme::SPACE_FACT;
        let filter_bar_h = if self.mode == MentionPopupMode::TerminalHeader { theme::CONTROL_STANDARD + theme::SPACE_ROW } else { 0.0 };

        // Compute content height across categories and rows.
        let mut total_content_h = 0.0f32;
        if self.candidates.is_empty() {
            total_content_h = row_h;
        } else {
            let mut last_cat: Option<MentionCategory> = None;
            for cand in &self.candidates {
                if last_cat != Some(cand.category) {
                    total_content_h += heading_h;
                    last_cat = Some(cand.category);
                }
                total_content_h += row_h;
            }
        }

        let max_list_h = MAX_VISIBLE_ROWS as f32 * row_h + heading_h * 2.0;
        let visible_list_h = total_content_h.min(max_list_h);
        let popup_h = inset * 2.0 + filter_bar_h + visible_list_h;
        let popup_w = anchor_rect.width().max(220.0).min((panel_rect.width() - theme::INSET_CONTROL * 2.0).max(180.0));
        let popup_x =
            anchor_rect.left().clamp(panel_rect.left() + theme::SPACE_FACT, (panel_rect.right() - popup_w - theme::SPACE_FACT).max(panel_rect.left()));
        let popup_y = match self.mode {
            MentionPopupMode::Composer => (anchor_rect.top() - theme::SPACE_FACT - popup_h).max(panel_rect.top() + theme::SPACE_FACT),
            MentionPopupMode::TerminalHeader => anchor_rect.bottom() + theme::SPACE_FACT,
        };
        let popup_rect = Rect::from_min_size(Pos2::new(popup_x, popup_y), Vec2::new(popup_w, popup_h));

        let p = ui.painter().clone();
        widgets::elevation_shadow(&p, popup_rect, theme::RADIUS_FRAME);
        p.rect_filled(popup_rect, theme::cr(theme::RADIUS_FRAME), theme::BG_MENU);
        p.rect_stroke(popup_rect, theme::cr(theme::RADIUS_FRAME), Stroke::new(theme::BORDER_WIDTH, theme::BORDER_MENU), StrokeKind::Inside);
        widgets::record(ui, "agent-mention-popup", popup_rect);

        let mut cur_y = popup_rect.top() + inset;
        if self.mode == MentionPopupMode::TerminalHeader {
            let filter_r =
                Rect::from_min_size(Pos2::new(popup_rect.left() + inset, cur_y), Vec2::new(popup_rect.width() - inset * 2.0, theme::CONTROL_STANDARD));
            p.rect_filled(filter_r, theme::cr(theme::RADIUS_CONTROL), theme::BG_SECONDARY);
            p.rect_stroke(filter_r, theme::cr(theme::RADIUS_CONTROL), Stroke::new(theme::BORDER_WIDTH, theme::BORDER_SELECTED), StrokeKind::Inside);
            let display_q = if self.query.is_empty() { "@ Filter layers, pages, components, variables, styles..." } else { &self.query };
            let q_col = if self.query.is_empty() { theme::TEXT_TERTIARY } else { theme::TEXT };
            p.text(Pos2::new(filter_r.left() + theme::INSET_CONTROL, filter_r.center().y), Align2::LEFT_CENTER, display_q, theme::font_body(), q_col);
            widgets::record(ui, "agent-mention-popup-filter", filter_r);
            cur_y += filter_bar_h;
        }

        let list_rect = Rect::from_min_size(Pos2::new(popup_rect.left() + inset, cur_y), Vec2::new(popup_rect.width() - inset * 2.0, visible_list_h));

        if self.candidates.is_empty() {
            let empty_r = Rect::from_min_size(list_rect.min, Vec2::new(list_rect.width(), row_h));
            p.text(
                Pos2::new(empty_r.left() + theme::INSET_CONTROL, empty_r.center().y),
                Align2::LEFT_CENTER,
                EMPTY_MATCH_COPY,
                theme::font_caption(),
                theme::TEXT_SECONDARY,
            );
            widgets::record(ui, "agent-mention-popup-empty", empty_r);
            return MentionPopupOutcome::None;
        }

        // Scroll wheel support inside the popup list.
        if ui.rect_contains_pointer(popup_rect) {
            let dy = ui.input(|i| i.smooth_scroll_delta.y);
            if dy.abs() > 0.1 {
                let max_scroll = (total_content_h - visible_list_h).max(0.0);
                self.scroll_y = (self.scroll_y - dy).clamp(0.0, max_scroll);
            }
        }

        let mut picked_idx: Option<usize> = None;
        let clip_painter = p.with_clip_rect(list_rect);
        let mut y = list_rect.top() - self.scroll_y;
        let mut last_cat: Option<MentionCategory> = None;

        for (idx, cand) in self.candidates.iter().enumerate() {
            if last_cat != Some(cand.category) {
                let head_r = Rect::from_min_size(Pos2::new(list_rect.left(), y), Vec2::new(list_rect.width(), heading_h));
                if head_r.bottom() >= list_rect.top() && head_r.top() <= list_rect.bottom() {
                    clip_painter.text(
                        Pos2::new(head_r.left() + theme::INSET_CONTROL, head_r.top() + theme::SPACE_GROUP + 7.0),
                        Align2::LEFT_CENTER,
                        cand.category.heading(),
                        theme::font_caption(),
                        theme::TEXT_SECONDARY,
                    );
                }
                y += heading_h;
                last_cat = Some(cand.category);
            }

            let row_r = Rect::from_min_size(Pos2::new(list_rect.left(), y), Vec2::new(list_rect.width(), row_h));
            if row_r.bottom() >= list_rect.top() && row_r.top() <= list_rect.bottom() {
                let resp = ui.interact(row_r, Id::new(("agent-mention-row", idx)), Sense::click());
                if resp.hovered() {
                    self.active_index = idx;
                }
                let is_active = idx == self.active_index;
                if is_active {
                    clip_painter.rect_filled(row_r, theme::cr(theme::RADIUS_CONTROL), theme::BG_SELECTED);
                } else if resp.hovered() {
                    clip_painter.rect_filled(row_r, theme::cr(theme::RADIUS_CONTROL), theme::BG_MENU_HOVER);
                }

                let name_color = if cand.hidden { theme::TEXT_TERTIARY } else { theme::TEXT };
                let top_row_y = row_r.top() + theme::SPACE_GROUP + theme::BORDER_WIDTH;
                let icon_pos = Pos2::new(row_r.left() + theme::INSET_CONTROL + theme::SPACE_INLINE - theme::BORDER_WIDTH, top_row_y);
                icons::paint(&clip_painter, cand.icon, icon_pos, 14.0, name_color);

                let text_x = row_r.left() + theme::INSET_CONTROL + 20.0;
                let max_text_w = (row_r.right() - theme::INSET_CONTROL - text_x).max(40.0);
                let name_font = theme::font_body_medium();
                let cap_font = theme::font_caption();
                let elided_name = widgets::elide(&clip_painter, &cand.name, &name_font, max_text_w);
                let elided_cap = widgets::elide(&clip_painter, &cand.caption, &cap_font, max_text_w);

                clip_painter.text(Pos2::new(text_x, top_row_y), Align2::LEFT_CENTER, elided_name, name_font, name_color);
                let bottom_row_y = row_r.top() + theme::CONTROL_STANDARD + theme::BORDER_WIDTH;
                clip_painter.text(Pos2::new(text_x, bottom_row_y), Align2::LEFT_CENTER, elided_cap, cap_font, theme::TEXT_SECONDARY);
                widgets::record(ui, &format!("agent-mention-row-{idx}"), row_r);

                if resp.clicked() {
                    picked_idx = Some(idx);
                }
            }
            y += row_h;
        }

        if let Some(idx) = picked_idx {
            if let Some(cand) = self.candidates.get(idx).cloned() {
                return self.apply_pick(&cand, input_line, active_mentions);
            }
        }

        MentionPopupOutcome::None
    }
}

/// Paints the compact `Mention` button (`@ Mention`) in the terminal header strip (`#655` §4).
/// Returns `(button_rect, clicked)`.
pub fn paint_terminal_mention_button(ui: &mut Ui, id_source: &str, right_edge: f32, strip_rect: Rect, active: bool) -> (Rect, bool) {
    let p = ui.painter().clone();
    let font = theme::font_caption_medium();
    let label = "@ Mention";
    let text_w = widgets::text_width(&p, label, &font);
    let btn_w = (text_w + theme::INSET_CONTROL * 2.0).max(theme::TARGET_MIN);
    let btn_h = theme::CONTROL_DENSE - 4.0;
    let btn_r = Rect::from_min_size(Pos2::new(right_edge - btn_w, strip_rect.center().y - btn_h * 0.5), Vec2::new(btn_w, btn_h));
    let hit_r = Rect::from_center_size(btn_r.center(), Vec2::new(btn_r.width().max(theme::TARGET_MIN), strip_rect.height().max(theme::TARGET_MIN)));
    let resp = ui.interact(hit_r, Id::new(id_source), Sense::click());
    let bg = if active {
        theme::BG_SELECTED
    } else if resp.hovered() {
        theme::BG_HOVER
    } else {
        theme::BG_SECONDARY
    };
    p.rect_filled(btn_r, theme::cr(theme::RADIUS_CONTROL), bg);
    p.rect_stroke(
        btn_r,
        theme::cr(theme::RADIUS_CONTROL),
        Stroke::new(theme::BORDER_WIDTH, if active { theme::BORDER_SELECTED } else { theme::BORDER }),
        StrokeKind::Inside,
    );
    let _ = Icon::Search;
    p.text(btn_r.center(), Align2::CENTER_CENTER, label, font, theme::TEXT);
    widgets::record(ui, id_source, btn_r);
    (btn_r, resp.clicked())
}

#[cfg(test)]
mod tests {
    use super::*;
    use designos_doc::{Node, Rect as DocRect};
    use egui::{Event, Modifiers, RawInput};

    #[test]
    fn popup_keyboard() {
        let mut doc = Document::new("Popup Sim Doc");
        let page = doc.add_page("Landing");
        let id1 = doc.alloc_id();
        let mut n1 = Node::frame(id1, DocRect::new(0.0, 0.0, 120.0, 40.0));
        n1.name = "Hero/Banner".to_string();
        doc.insert(page, n1).unwrap();

        let id2 = doc.alloc_id();
        let mut n2 = Node::frame(id2, DocRect::new(0.0, 0.0, 120.0, 40.0));
        n2.name = "Hero/Title".to_string();
        doc.insert(page, n2).unwrap();

        let ctx = Context::default();
        let mut popup = MentionPopupState::default();
        let mut input_line = "@he".to_string();
        let mut mentions: Vec<Mention> = Vec::new();

        // Step 1: Typing `@he` opens the popup with 2 candidates ("Hero/Banner", "Hero/Title").
        popup.sync_from_composer_text(&input_line, input_line.len(), &doc, page);
        assert!(popup.is_open());
        assert_eq!(popup.candidates.len(), 2);
        let second_name = popup.candidates[1].name.clone();
        let second_id = popup.candidates[1].id.clone();

        // Step 2: Press `Down` then `Enter` in an egui pass -> selects the second candidate and replaces `@he`.
        let mut raw = RawInput::default();
        raw.events.push(Event::Key { key: Key::ArrowDown, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE });
        raw.events.push(Event::Key { key: Key::Enter, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE });
        let mut outcome = MentionPopupOutcome::None;
        let _ = ctx.run_ui(raw, |ui| {
            let panel_r = Rect::from_min_size(Pos2::ZERO, Vec2::new(320.0, 480.0));
            let comp_r = Rect::from_min_size(Pos2::new(8.0, 360.0), Vec2::new(304.0, 96.0));
            outcome = popup.handle_keyboard(ui.ctx(), &mut input_line, &mut mentions, &doc, page);
            let _ = popup.show(ui, panel_r, comp_r, &mut input_line, &mut mentions);
        });

        assert!(!popup.is_open(), "Enter must close the popup after picking");
        assert_eq!(input_line, format!("@{second_name} "));
        assert_eq!(mentions.len(), 1);
        assert_eq!(mentions[0].label, second_name);
        assert_eq!(mentions[0].id, second_id);
        assert!(matches!(outcome, MentionPopupOutcome::PickedComposer(_)));

        // Step 3: Typing `@he` again and pressing `Esc` closes the popup without changing the text.
        input_line = "Inspect @he".to_string();
        popup.sync_from_composer_text(&input_line, input_line.len(), &doc, page);
        assert!(popup.is_open());
        let mentions_before = mentions.len();

        let mut esc_raw = RawInput::default();
        esc_raw.events.push(Event::Key { key: Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE });
        let _ = ctx.run_ui(esc_raw, |ui| {
            let res = popup.handle_keyboard(ui.ctx(), &mut input_line, &mut mentions, &doc, page);
            assert_eq!(res, MentionPopupOutcome::Dismissed);
        });
        assert!(!popup.is_open(), "Esc must close the popup");
        assert_eq!(input_line, "Inspect @he", "Esc must not change the composer text");
        assert_eq!(mentions.len(), mentions_before, "Esc must not add a mention");
    }
}
