//! "Delete page?": the kit confirm dialog. Cancel has the focus when it opens, so Enter never deletes.

use crate::theme;
use crate::widgets::{self, kit};
use egui::{Align2, Area, Context, Id, Key, Modifiers, Order, Pos2, Rect, Sense, Vec2};

const W: f32 = 440.0;
const PAD: f32 = theme::INSET_MODAL;
const BTN_H: f32 = theme::CONTROL_MD;
const LINE_H: f32 = theme::LINE_TEXT_SM;
/// Between lines of body text, and after each one (the same sizing `home/dialog.rs` gives its confirm dialogs).
const LINE_GAP: f32 = theme::SPACE_INLINE;
const TITLE_H: f32 = theme::LINE_TEXT_MD;
const TITLE_GAP: f32 = theme::SPACE_GROUP;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeletePage {
    pub rel: String,
    pub title: String,
    /// 0 = Cancel, 1 = Delete page.
    focus: usize,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Answer {
    Open,
    Cancel,
    Delete,
}

impl DeletePage {
    pub fn new(rel: String, title: String) -> Self {
        Self { rel, title, focus: 0 }
    }

    pub fn lines(&self) -> [String; 2] {
        [format!("\u{201c}{}\u{201d} moves to the trash.", self.title), "Links to it will break.".to_string()]
    }

    pub fn focused_is_cancel(&self) -> bool {
        self.focus == 0
    }
}

/// The dialog is 440 wide, centred 24 px above the window centre; 24 padding around a 24 title, 12 gap, two 20 px
/// lines each followed by 8, and the 40 px buttons: 180 tall.
pub fn dialog_rect(screen: Rect) -> Rect {
    let h = PAD + TITLE_H + TITLE_GAP + 2.0 * (LINE_H + LINE_GAP) + BTN_H + PAD;
    Rect::from_center_size(screen.center() - Vec2::new(0.0, 24.0), Vec2::new(W, h))
}

pub fn show(ctx: &Context, d: &mut DeletePage) -> Answer {
    let screen = ctx.content_rect();
    let rect = dialog_rect(screen);
    let (esc, enter, tab, shift_tab, left, right) = ctx.input_mut(|i| {
        (
            i.consume_key(Modifiers::NONE, Key::Escape),
            i.consume_key(Modifiers::NONE, Key::Enter),
            i.consume_key(Modifiers::NONE, Key::Tab),
            i.consume_key(Modifiers::SHIFT, Key::Tab),
            i.consume_key(Modifiers::NONE, Key::ArrowLeft),
            i.consume_key(Modifiers::NONE, Key::ArrowRight),
        )
    });
    if tab || shift_tab || left || right {
        d.focus = 1 - d.focus;
    }
    let mut answer = Answer::Open;
    if esc {
        answer = Answer::Cancel;
    }
    if enter {
        answer = if d.focus == 0 { Answer::Cancel } else { Answer::Delete };
    }
    let label_font = theme::font_text(theme::FONT_SIZE_TEXT_SM, theme::TextWeight::Semibold);
    Area::new(Id::new("project-delete-page")).order(Order::Foreground).fixed_pos(screen.min).interactable(true).fade_in(false).show(ctx, |ui| {
        let scrim = ui.allocate_rect(screen, Sense::click_and_drag());
        let p = ui.painter().clone();
        kit::modal_surface(&p, screen, rect);
        if scrim.clicked() && scrim.interact_pointer_pos().is_some_and(|pos| !rect.contains(pos)) {
            answer = Answer::Cancel;
        }
        let x0 = rect.left() + PAD;
        let mut y = rect.top() + PAD;
        p.text(
            Pos2::new(x0, y + TITLE_H / 2.0),
            Align2::LEFT_CENTER,
            "Delete page?",
            theme::font_text(theme::FONT_SIZE_TEXT_MD, theme::TextWeight::Semibold),
            theme::TEXT,
        );
        y += TITLE_H + TITLE_GAP;
        for line in d.lines() {
            let shown = widgets::elide(&p, &line, &theme::font_text(theme::FONT_SIZE_TEXT_SM, theme::TextWeight::Regular), W - PAD * 2.0);
            p.text(
                Pos2::new(x0, y + LINE_H / 2.0),
                Align2::LEFT_CENTER,
                shown,
                theme::font_text(theme::FONT_SIZE_TEXT_SM, theme::TextWeight::Regular),
                theme::TEXT_TERTIARY,
            );
            y += LINE_H + LINE_GAP;
        }
        let delete_w = widgets::text_width(&p, "Delete page", &label_font).max(48.0) + 32.0;
        let delete_r = Rect::from_min_size(Pos2::new(rect.right() - PAD - delete_w, y), Vec2::new(delete_w, BTN_H));
        let cancel_w = widgets::text_width(&p, "Cancel", &label_font).max(48.0) + 32.0;
        let cancel_r = Rect::from_min_size(Pos2::new(delete_r.left() - theme::SPACE_INLINE - cancel_w, y), Vec2::new(cancel_w, BTN_H));
        if kit::button(ui, cancel_r, "project-delete-cancel", "Cancel", kit::Hierarchy::Secondary, kit::Size::Md, true).clicked() {
            answer = Answer::Cancel;
        }
        if kit::button(ui, delete_r, "project-delete-confirm", "Delete page", kit::Hierarchy::Destructive, kit::Size::Md, true).clicked() {
            answer = Answer::Delete;
        }
        widgets::record(ui, "project-delete-cancel", cancel_r);
        widgets::record(ui, "project-delete-confirm", delete_r);
        let ring_rect = if d.focus == 0 { cancel_r } else { delete_r };
        kit::focus_ring(&p, ring_rect, theme::RADIUS_CONTROL as f32);
    });
    answer
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_dialog_names_the_page_and_says_where_it_goes() {
        let d = DeletePage::new("spec/cart-rules.md".into(), "Cart rules".into());
        assert_eq!(d.lines(), ["\u{201c}Cart rules\u{201d} moves to the trash.".to_string(), "Links to it will break.".to_string()]);
        assert!(d.focused_is_cancel(), "Cancel has the focus when the dialog opens");
    }

    #[test]
    fn the_dialog_is_440_by_180_and_sits_24_px_above_the_window_centre() {
        let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(1440.0, 900.0));
        let r = dialog_rect(screen);
        assert_eq!((r.width(), r.height()), (440.0, 180.0));
        assert_eq!(r.center(), Pos2::new(720.0, 426.0));
    }
}
