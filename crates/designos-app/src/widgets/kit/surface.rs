use super::{look, Look, Size};
use crate::theme;
use egui::Color32;

const W: f32 = theme::BORDER_WIDTH;
const CLEAR: Color32 = Color32::TRANSPARENT;

pub const MENU_INSET: f32 = theme::MENU_PILL_INSET;
pub const PILL_RADIUS: u8 = theme::RADIUS_CONTROL;
pub const MENU_RADIUS: u8 = theme::RADIUS_FRAME;

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

pub const fn menu_context_look() -> Look {
    menu_panel_look()
}

pub const fn popover_look() -> Look {
    look(theme::BG, theme::BORDER_MENU, W, theme::TEXT, theme::ICON_QUATERNARY)
}

pub const fn modal_look() -> Look {
    look(theme::BG, CLEAR, W, theme::TEXT, theme::ICON_QUATERNARY)
}

pub const fn command_look() -> Look {
    look(theme::BG, theme::BORDER_MENU, W, theme::TEXT, theme::ICON_QUATERNARY)
}

pub const fn toast_look() -> Look {
    look(theme::BG_PRIMARY_ALT, theme::BORDER_MENU, W, theme::TEXT, theme::ICON_BRAND_PRIMARY)
}

pub const fn tooltip_look() -> Look {
    look(theme::BG_PRIMARY_SOLID, CLEAR, W, theme::TEXT, CLEAR)
}
