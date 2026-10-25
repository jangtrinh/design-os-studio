pub mod assets;
pub mod check_designs;
pub mod components;
pub mod context;
pub mod effects;
pub mod export;
pub mod inspect;
pub mod layers;
pub mod layoutguide;
pub mod left;
pub mod menus;
pub mod properties;
pub mod proto_actions;
pub mod prototype;
pub mod rail;
pub mod right;
pub mod stroke;
pub mod style_edit;
mod style_edit_grid_rows;
pub mod style_picker;
pub mod toolbar;
pub mod typography;
pub mod var_bind;
pub mod vertex;

use crate::theme;
use egui::{Rect, Ui};

/// Runs `f` in a child ui clipped to `rect` (so scrolled content never bleeds).
pub fn clipped<R>(ui: &mut Ui, rect: Rect, f: impl FnOnce(&mut Ui) -> R) -> R {
    ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
        ui.set_clip_rect(ui.clip_rect().intersect(rect));
        f(ui)
    })
    .inner
}

fn scrollbar_drag_id(rect: Rect) -> egui::Id {
    egui::Id::new(("scrollbar-drag", rect.min.x as i32, rect.min.y as i32, rect.width() as i32, rect.height() as i32))
}

/// Consumes any pending scrollbar drag value for `rect`.
pub fn take_scrollbar_drag(ui: &Ui, rect: Rect) -> Option<f32> {
    let key = scrollbar_drag_id(rect);
    ui.ctx().data_mut(|d| d.remove_temp::<f32>(key))
}

/// Draws an interactive thin scrollbar thumb on the right edge of `rect`.
pub fn scrollbar(ui: &mut Ui, rect: Rect, scroll: f32, content_h: f32) {
    if content_h <= rect.height() + 0.5 {
        return;
    }
    let track_len = rect.height();
    let thumb_h = (track_len * rect.height() / content_h).max(24.0);
    let max_scroll = content_h - rect.height();
    let track = Rect::from_min_size(egui::pos2(rect.right() - 8.0, rect.top()), egui::vec2(8.0, track_len));
    crate::widgets::record(ui, "scrollbar-track", track);
    let resp = ui.interact(track, ui.id().with(("scrollbar", track.min.x as i32, track.min.y as i32)), egui::Sense::click_and_drag());
    let mut cur_scroll = scroll;
    if (resp.clicked() || resp.dragged()) && max_scroll > 0.0 {
        if let Some(pos) = resp.interact_pointer_pos() {
            let denom = (track_len - thumb_h).max(1.0);
            let new_t = ((pos.y - track.min.y - thumb_h * 0.5) / denom).clamp(0.0, 1.0);
            cur_scroll = new_t * max_scroll;
            ui.ctx().data_mut(|d| d.insert_temp(scrollbar_drag_id(rect), cur_scroll));
        }
    }
    let t = if max_scroll > 0.0 { (cur_scroll / max_scroll).clamp(0.0, 1.0) } else { 0.0 };
    let y = rect.top() + (track_len - thumb_h) * t;
    let r = Rect::from_min_size(egui::pos2(rect.right() - 6.0, y), egui::vec2(4.0, thumb_h));
    let col = if resp.hovered() || resp.dragged() { theme::TEXT_SECONDARY } else { theme::TEXT_TERTIARY };
    ui.painter().rect_filled(r, theme::cr(2), col);
}

/// Applies wheel scrolling (and any scrollbar drag) when the pointer is over `rect`.
pub fn wheel_scroll(ui: &Ui, rect: Rect, scroll: &mut f32, content_h: f32) {
    let max_scroll = (content_h - rect.height()).max(0.0);
    if let Some(dragged) = take_scrollbar_drag(ui, rect) {
        *scroll = dragged;
    }
    if ui.rect_contains_pointer(rect) {
        let dy = ui.input(|i| i.smooth_scroll_delta.y);
        *scroll -= dy;
    }
    *scroll = scroll.clamp(0.0, max_scroll);
}
