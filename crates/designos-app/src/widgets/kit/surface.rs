use super::{look, paint_frame, Look, Size};
use crate::theme;
use egui::{Color32, Painter, Rect};

const W: f32 = theme::BORDER_WIDTH;
const CLEAR: Color32 = Color32::TRANSPARENT;

#[cfg_attr(not(test), allow(dead_code))]
pub const MENU_INSET: f32 = theme::MENU_PILL_INSET;
#[cfg_attr(not(test), allow(dead_code))]
pub const PILL_RADIUS: u8 = theme::RADIUS_CONTROL;
pub const MENU_RADIUS: u8 = theme::RADIUS_FRAME;

#[cfg_attr(not(test), allow(dead_code))]
pub const fn menu_item_height(size: Size) -> f32 {
    match size {
        Size::Xs => theme::CONTROL_XS,
        Size::Sm => theme::CONTROL_SM,
        Size::Md => theme::CONTROL_MD,
    }
}

pub const fn menu_panel_look() -> Look {
    look(theme::BG, theme::BORDER_MENU, W, theme::TEXT_SECONDARY, theme::ICON_QUATERNARY)
}

#[cfg_attr(not(test), allow(dead_code))]
pub const fn menu_context_look() -> Look {
    menu_panel_look()
}

pub const fn popover_look() -> Look {
    look(theme::BG, theme::BORDER_MENU, W, theme::TEXT, theme::ICON_QUATERNARY)
}

#[cfg_attr(not(test), allow(dead_code))]
pub const fn modal_look() -> Look {
    look(theme::BG, CLEAR, W, theme::TEXT, theme::ICON_QUATERNARY)
}

#[cfg_attr(not(test), allow(dead_code))]
pub const fn command_look() -> Look {
    look(theme::BG, theme::BORDER_MENU, W, theme::TEXT, theme::ICON_QUATERNARY)
}

#[cfg_attr(not(test), allow(dead_code))]
pub const fn toast_look() -> Look {
    look(theme::BG_PRIMARY_ALT, theme::BORDER_MENU, W, theme::TEXT, theme::ICON_BRAND_PRIMARY)
}

#[cfg_attr(not(test), allow(dead_code))]
pub const fn tooltip_look() -> Look {
    look(theme::BG_PRIMARY_SOLID, CLEAR, W, theme::TEXT, CLEAR)
}

pub fn menu_surface(p: &Painter, rect: Rect) {
    paint_frame(p, rect, MENU_RADIUS, &menu_panel_look());
}

pub fn popover_surface(p: &Painter, rect: Rect) {
    paint_frame(p, rect, MENU_RADIUS, &popover_look());
}

/// Scrim over `screen`, then the radius-16 frame. No border and no shadow.
#[allow(dead_code)]
pub fn modal_surface(p: &Painter, screen: Rect, frame: Rect) {
    p.rect_filled(screen, 0.0, theme::BG_OVERLAY);
    paint_frame(p, frame, MENU_RADIUS, &modal_look());
}

#[allow(dead_code)]
pub fn command_bar_surface(p: &Painter, rect: Rect) {
    paint_frame(p, rect, theme::RADIUS_XL, &command_look());
}

#[allow(dead_code)]
pub fn toast_surface(p: &Painter, rect: Rect) {
    paint_frame(p, rect, theme::RADIUS_XL, &toast_look());
}

/// Frame for a caller-drawn tooltip. Stock `on_hover_text` stays on the window style.
pub fn tooltip_frame() -> egui::Frame {
    let paint = tooltip_look();
    egui::Frame::new()
        .fill(paint.fill)
        .corner_radius(theme::cr(theme::RADIUS_CONTROL))
        .inner_margin(egui::Margin::symmetric(theme::INSET_BUTTON as i8, theme::INSET_FRAME as i8))
        .stroke(egui::Stroke::NONE)
}
