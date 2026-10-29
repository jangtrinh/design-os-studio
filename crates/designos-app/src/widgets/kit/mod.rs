//! Widget looks and the painters that read them.

use egui::{Color32, Painter, Rect, Stroke, StrokeKind};

#[cfg(test)]
mod contrast_table;
#[cfg(test)]
mod contrast_tests;
#[cfg(test)]
mod focus_tests;
#[cfg(test)]
mod guard_tests;
#[cfg(test)]
mod painter_form_tests;
#[cfg(test)]
mod painter_live_tests;
#[cfg(test)]
mod painter_menu_tests;
#[cfg(test)]
mod painter_minor_tests;
#[cfg(test)]
mod painter_place_tests;
#[cfg(test)]
mod painter_shape_tests;
#[cfg(test)]
mod painter_tip_tests;
#[cfg(test)]
mod row_tests;
#[cfg(test)]
mod state_tests;
#[cfg(test)]
mod table_tests;
#[cfg(test)]
mod theme_apply_tests;
#[cfg(test)]
mod token_tests;

mod badge;
mod button;
mod choice;
mod feedback;
mod field;
mod focus;
mod rows;
mod surface;
mod tabs;

#[cfg_attr(not(test), allow(unused_imports))]
pub use badge::{count_badge, count_badge_look, status_color, status_dot, status_tag, tag, tag_look, tag_ring, DotTone, Tone};
#[cfg_attr(not(test), allow(unused_imports))]
pub use button::{button, button_icon, button_look, button_ring, close_look_over, paint_icon_bg, toast_action_look, ClosePlate, Hierarchy};
#[cfg_attr(not(test), allow(unused_imports))]
pub use choice::{
    checkbox, checkbox_look, checkbox_ring, radio_card, radio_card_look, radio_ring, slider_handle, slider_handle_look, slider_ring, swatch_look, swatch_ring,
    swatch_ring_color, toggle, toggle_look, toggle_ring,
};
#[cfg_attr(not(test), allow(unused_imports))]
pub use feedback::{empty_state, empty_state_look, loading_look, loading_ring, progress_bar, progress_look, tile_icon, tile_look, TileKind};
#[cfg_attr(not(test), allow(unused_imports))]
pub use field::{field_look, paint_field, property_field_look, search_look, select_look};
pub use focus::{focus_ring, keyboard_focus};
#[cfg_attr(not(test), allow(unused_imports))]
pub use rows::{
    command_row_look, divider_look, list_row, list_row_ring, menu_item_look, menu_row_look, panel_header, panel_header_look, row_look, section_header,
    section_header_look, separator_look, shortcut_look, tree_row_bg, tree_row_look,
};
#[cfg_attr(not(test), allow(unused_imports))]
pub use surface::{
    command_bar_surface, command_look, menu_context_look, menu_item_height, menu_panel_look, menu_surface, modal_look, modal_surface, popover_look,
    popover_surface, toast_look, toast_surface, tooltip_frame, tooltip_look, MENU_INSET, MENU_RADIUS, PILL_RADIUS,
};
#[cfg_attr(not(test), allow(unused_imports))]
pub use tabs::{
    segment_tile_h, segmented, segmented_look, segmented_ring, tab, tab_look, tab_ring, FILE_TAB_BAR_H, HOME_TAB_H, SEGMENTED_STRIP_H, SESSION_TAB_H,
};

/// Editor step, Home and dialog body, dialog footer action.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Size {
    Xs,
    #[cfg_attr(not(test), allow(dead_code))]
    Sm,
    #[cfg_attr(not(test), allow(dead_code))]
    Md,
}

/// Interaction shown by a look. Loading is a button hover with a spinner, not a pointer state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Default,
    Hover,
    Pressed,
    Focused,
    Disabled,
    Selected,
    #[cfg_attr(not(test), allow(dead_code))]
    Loading,
}

/// Page colour behind a row. Secondary retargets hover, selected and pressed fills.
#[cfg_attr(not(test), allow(dead_code))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Surface {
    Primary,
    Secondary,
}

/// Resolved paint for one widget state. A null table cell is [`Color32::TRANSPARENT`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Look {
    pub fill: Color32,
    pub border: Color32,
    pub border_w: f32,
    pub text: Color32,
    pub icon: Color32,
    pub mark: Color32,
}

/// Fills and strokes `rect` from `paint`. A transparent fill or border is skipped.
pub fn paint_frame(p: &Painter, rect: Rect, radius: u8, paint: &Look) {
    let curve = crate::theme::cr(radius);
    if paint.fill != Color32::TRANSPARENT {
        p.rect_filled(rect, curve, paint.fill);
    }
    if paint.border != Color32::TRANSPARENT && paint.border_w > 0.0 {
        p.rect_stroke(rect, curve, Stroke::new(paint.border_w, paint.border), StrokeKind::Inside);
    }
}

pub(crate) const fn look(fill: Color32, border: Color32, border_w: f32, text: Color32, icon: Color32) -> Look {
    Look { fill, border, border_w, text, icon, mark: Color32::TRANSPARENT }
}

/// Half the premultiplied alpha. Opaque 255 lands on 128, matching `theme::rgba` integer math.
pub const fn half_color(c: Color32) -> Color32 {
    let a0 = c.a();
    if a0 == 0 {
        return c;
    }
    let a = (a0 as u16).div_ceil(2);
    let r = ((c.r() as u16 * a) / a0 as u16) as u8;
    let g = ((c.g() as u16 * a) / a0 as u16) as u8;
    let b = ((c.b() as u16 * a) / a0 as u16) as u8;
    Color32::from_rgba_premultiplied(r, g, b, a as u8)
}

/// Selected fill and border, with the hovered text and icon.
pub const fn selected_hover(selected: Look, hot: Look) -> Look {
    Look { text: hot.text, icon: hot.icon, ..selected }
}

/// Rule for a disabled control whose table cell is the resting cell at half alpha.
pub const fn disabled(look: Look) -> Look {
    Look {
        fill: half_color(look.fill),
        border: half_color(look.border),
        border_w: look.border_w,
        text: half_color(look.text),
        icon: half_color(look.icon),
        mark: half_color(look.mark),
    }
}

/// Disabled, then pressed, then selected, then hover, then keyboard focus, otherwise resting.
/// Loading is chosen by the caller: this signature has no async flag.
pub fn state_of(r: &egui::Response, selected: bool, enabled: bool) -> State {
    if !enabled {
        State::Disabled
    } else if r.is_pointer_button_down_on() {
        State::Pressed
    } else if selected {
        State::Selected
    } else if r.hovered() {
        State::Hover
    } else if r.has_focus() {
        State::Focused
    } else {
        State::Default
    }
}
