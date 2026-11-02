//! THROWAWAY slash menu behaviour and popup, identical for both backends.

use crate::app::{App, Mode, SlashState};
use crate::history::Edit;
use crate::model::char_len;
use crate::slash::{self, ITEMS};
use crate::tedit::take_key;
use crate::theme;
use egui::{pos2, vec2, Align2, Context, CornerRadius, Key, Order, Rect, Sense, Stroke};

impl App {
    /// Opens the menu on an empty block with the slash typed by "+" (no characters in the text).
    pub fn slash_open_at_caret(&mut self) {
        let before = self.snap();
        self.doc.blocks[self.focus].replace(0..0, "/");
        self.record(before, Edit::Other);
        self.go(self.focus, 1);
    }

    /// Opens, updates or closes the menu from the text before the caret. Called once per frame.
    pub fn slash_refresh(&mut self) {
        if self.preedit_active || self.preedit.is_some() {
            return;
        }
        let blk = &self.doc.blocks[self.focus];
        let found = slash::trigger(&blk.text, self.head).filter(|(start, _)| self.slash_dismissed != Some((blk.id, *start)));
        match (found, self.slash.take()) {
            (Some((start, query)), prev) => {
                let sel = match prev {
                    Some(p) if p.query == query && p.start == start => p.sel,
                    _ => 0,
                };
                let n = slash::filter(&query).len();
                self.slash = Some(SlashState { start, query, sel: sel.min(n.saturating_sub(1)) });
            }
            (None, _) => self.slash = None,
        }
    }

    /// Arrow / Enter / Esc while the menu is open. Runs before either backend reads input.
    pub fn slash_keys(&mut self, ctx: &Context) {
        let Some(s) = self.slash.as_ref() else { return };
        let matches = slash::filter(&s.query);
        if take_key(ctx, Key::ArrowDown, false) {
            let n = matches.len().max(1);
            self.slash.as_mut().unwrap().sel = (s.sel + 1) % n;
        } else if take_key(ctx, Key::ArrowUp, false) {
            let n = matches.len().max(1);
            self.slash.as_mut().unwrap().sel = (s.sel + n - 1) % n;
        } else if take_key(ctx, Key::Enter, false) {
            if let Some(&i) = matches.get(s.sel) {
                self.slash_apply(i);
            }
        } else if take_key(ctx, Key::Escape, false) {
            self.slash_dismissed = Some((self.doc.blocks[self.focus].id, s.start));
            self.slash = None;
        }
    }

    pub fn slash_apply(&mut self, item: usize) {
        let (Some(s), Some(kind)) = (self.slash.take(), ITEMS[item].kind) else { return };
        let before = self.snap();
        let head = self.head.min(char_len(&self.doc.blocks[self.focus].text));
        let blk = &mut self.doc.blocks[self.focus];
        blk.replace(s.start..head, "");
        blk.kind = kind;
        self.record(before, Edit::Other);
        self.go(self.focus, s.start);
    }

    pub fn slash_show(&mut self, ctx: &Context) {
        let (Some(s), Some(at)) = (self.slash.as_ref(), self.caret_screen) else { return };
        let matches = slash::filter(&s.query);
        let (sel, mut clicked) = (s.sel, None);
        egui::Area::new(egui::Id::new("slash-menu")).order(Order::Foreground).fade_in(false).fixed_pos(at + vec2(0.0, 4.0)).show(ctx, |ui| {
            let (w, row_h) = (320.0, 32.0);
            egui::Frame::NONE.fill(theme::BG).stroke(Stroke::new(1.0, theme::BORDER)).corner_radius(CornerRadius::same(16)).inner_margin(8).show(ui, |ui| {
                ui.set_width(w - 16.0);
                if matches.is_empty() {
                    let (r, _) = ui.allocate_exact_size(vec2(w - 16.0, row_h), Sense::hover());
                    ui.painter().text(r.min + vec2(8.0, row_h / 2.0), Align2::LEFT_CENTER, "No blocks match", theme::medium(12.0), theme::TEXT_PLACEHOLDER);
                    return;
                }
                egui::ScrollArea::vertical().max_height(328.0).auto_shrink([false, true]).show(ui, |ui| {
                    let mut section = "";
                    for (pos, &i) in matches.iter().enumerate() {
                        let it = &ITEMS[i];
                        if it.section != section {
                            section = it.section;
                            let (r, _) = ui.allocate_exact_size(vec2(w - 16.0, 24.0), Sense::hover());
                            ui.painter().text(r.min + vec2(8.0, 12.0), Align2::LEFT_CENTER, section, theme::medium(12.0), theme::TEXT_PLACEHOLDER);
                        }
                        let (r, resp) = ui.allocate_exact_size(vec2(w - 16.0, row_h), Sense::click());
                        let on = pos == sel || resp.hovered();
                        let col = if it.kind.is_some() { theme::TEXT_SECONDARY } else { theme::TEXT_PLACEHOLDER };
                        if on {
                            ui.painter().rect_filled(Rect::from_min_max(r.min, pos2(r.max.x, r.max.y)), CornerRadius::same(8), theme::BG_SELECTED);
                        }
                        ui.painter().text(r.min + vec2(16.0, row_h / 2.0), Align2::LEFT_CENTER, it.label, theme::medium(12.0), col);
                        ui.painter().text(pos2(r.max.x - 16.0, r.center().y), Align2::RIGHT_CENTER, it.hint, theme::mono(12.0), theme::TEXT_PLACEHOLDER);
                        if resp.clicked() {
                            clicked = Some(i);
                        }
                    }
                });
            });
        });
        if let Some(i) = clicked {
            self.slash_apply(i);
        }
        let _ = Mode::Custom; // both modes share this popup
    }
}
