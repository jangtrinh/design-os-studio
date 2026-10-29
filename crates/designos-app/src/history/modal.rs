//! "Save to version history…": a name and an optional description.

use crate::app::App;
use crate::comments::ui;
use crate::theme;
use crate::widgets::{self, kit};
use egui::{Align2, Context, Id, Key, Modifiers, Order, Pos2, Rect, Sense, TextEdit, Vec2};

const W: f32 = 440.0;
const PAD: f32 = theme::INSET_MODAL;
const FIELD_H: f32 = theme::CONTROL_SM;
const DESC_H: f32 = 88.0;
const BTN_H: f32 = theme::CONTROL_MD;
const LABEL_H: f32 = 18.0;

pub fn show(app: &mut App, ctx: &Context) {
    let Some(mut m) = app.history.save_modal.take() else { return };
    let screen = ctx.content_rect();
    let name_id = Id::new("version-name");
    let name_focused = ctx.memory(|mem| mem.has_focus(name_id));
    let (ctrl_enter, enter, esc) = ctx.input_mut(|i| {
        (i.consume_key(Modifiers::COMMAND, Key::Enter), name_focused && i.consume_key(Modifiers::NONE, Key::Enter), i.consume_key(Modifiers::NONE, Key::Escape))
    });
    let h = PAD + 24.0 + 12.0 + LABEL_H + 4.0 + FIELD_H + 12.0 + LABEL_H + 4.0 + DESC_H + 16.0 + BTN_H + PAD;
    let rect = Rect::from_center_size(screen.center() - Vec2::new(0.0, 24.0), Vec2::new(W, h));
    let mut submit = ctrl_enter || enter;
    let mut cancel = esc;
    let font = theme::font_text(theme::FONT_SIZE_TEXT_SM, theme::TextWeight::Regular);
    egui::Area::new(Id::new("version-save-modal")).order(Order::Foreground).fixed_pos(screen.min).interactable(true).show(ctx, |ui| {
        let scrim = ui.allocate_rect(screen, Sense::click_and_drag());
        let p = ui.painter().clone();
        kit::modal_surface(&p, screen, rect);
        if scrim.clicked() && !rect.contains(scrim.interact_pointer_pos().unwrap_or(Pos2::ZERO)) {
            cancel = true;
        }
        let x0 = rect.left() + PAD;
        let w = W - PAD * 2.0;
        let mut y = rect.top() + PAD;
        p.text(
            Pos2::new(x0, y + 12.0),
            Align2::LEFT_CENTER,
            "Save to version history",
            theme::font_text(theme::FONT_SIZE_TEXT_MD, theme::TextWeight::Semibold),
            theme::TEXT,
        );
        y += 24.0 + 12.0;
        p.text(Pos2::new(x0, y + LABEL_H / 2.0), Align2::LEFT_CENTER, "Version name", font.clone(), theme::TEXT_TERTIARY);
        y += LABEL_H + 4.0;
        let nr = Rect::from_min_size(Pos2::new(x0, y), Vec2::new(w, FIELD_H));
        kit::paint_field(ui, nr, name_id, kit::Size::Sm, false);
        let te = TextEdit::singleline(&mut m.name)
            .id(name_id)
            .font(font.clone())
            .text_color(theme::TEXT)
            .frame(egui::Frame::NONE.inner_margin(egui::Margin::symmetric(10, 0)))
            .vertical_align(egui::Align::Center)
            .hint_text(egui::RichText::new("e.g. Ready for review").font(font.clone()).color(theme::TEXT_PLACEHOLDER))
            .min_size(nr.size())
            .desired_width(nr.width());
        // The hint colour is egui's weak text colour; point it at the placeholder role for this field only.
        ui.visuals_mut().weak_text_color = Some(theme::TEXT_PLACEHOLDER);
        let out = ui.put(nr, te);
        if m.fresh {
            out.request_focus();
            m.fresh = false;
        }
        widgets::record(ui, "version-name", nr);
        y += FIELD_H + 12.0;
        p.text(Pos2::new(x0, y + LABEL_H / 2.0), Align2::LEFT_CENTER, "Description (optional)", font.clone(), theme::TEXT_TERTIARY);
        y += LABEL_H + 4.0;
        let dr = Rect::from_min_size(Pos2::new(x0, y), Vec2::new(w, DESC_H));
        ui::text_box(ui, dr, Id::new("version-description"), &mut m.description, "What changed in this version?", false);
        widgets::record(ui, "version-description", dr);
        y += DESC_H + 16.0;
        let enabled = !m.name.trim().is_empty();
        let sw = ui::kit_button_width(&p, "Save").max(64.0);
        let save_r = Rect::from_min_size(Pos2::new(rect.right() - PAD - sw, y), Vec2::new(sw, BTN_H));
        let cw = ui::kit_button_width(&p, "Cancel").max(64.0);
        let cancel_r = Rect::from_min_size(Pos2::new(save_r.left() - 8.0 - cw, y), Vec2::new(cw, BTN_H));
        if kit::button(ui, save_r, "version-save", "Save", kit::Hierarchy::Primary, kit::Size::Md, enabled).clicked() {
            submit = true;
        }
        if kit::button(ui, cancel_r, "version-cancel", "Cancel", kit::Hierarchy::Secondary, kit::Size::Md, true).clicked() {
            cancel = true;
        }
        widgets::record(ui, "version-save", save_r);
        widgets::record(ui, "version-cancel", cancel_r);
    });
    if cancel {
        return;
    }
    if submit && !m.name.trim().is_empty() {
        app.save_named_version(&m.name, &m.description);
        return;
    }
    app.history.save_modal = Some(m);
}
