//! Pure widget looks. Each function returns colours and a border width for one state.
//! Drawing a frame, a focus band or a control is a later step.
#![cfg_attr(not(test), allow(dead_code, unused_imports))]

use egui::Color32;

#[cfg(test)]
mod contrast_table;
#[cfg(test)]
mod contrast_tests;
#[cfg(test)]
mod focus_tests;
#[cfg(test)]
mod guard_tests;
#[cfg(test)]
mod row_tests;
#[cfg(test)]
mod state_tests;
#[cfg(test)]
mod table_tests;
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

pub use badge::{count_badge, status_dot, tag_look, tag_ring, DotTone, Tone};
pub use button::{button_look, button_ring, Hierarchy};
pub use choice::{
    checkbox_look, checkbox_ring, radio_card_look, radio_ring, slider_handle_look, slider_ring, swatch_look, swatch_ring, toggle_look, toggle_ring,
};
pub use feedback::{empty_state_look, loading_look, progress_look, tile_icon, TileKind};
pub use field::{field_look, property_field_look, search_look, select_look};
pub use rows::{divider_look, list_row_ring, menu_item_look, panel_header_look, row_look, section_header_look, separator_look, shortcut_look, tree_row_look};
pub use surface::{
    command_look, menu_context_look, menu_item_height, menu_panel_look, modal_look, popover_look, toast_look, tooltip_look, MENU_INSET, MENU_RADIUS,
    PILL_RADIUS,
};
pub use tabs::{segment_tile_h, segmented_look, segmented_ring, tab_look, tab_ring, FILE_TAB_BAR_H, HOME_TAB_H, SEGMENTED_STRIP_H, SESSION_TAB_H};

/// Editor step, Home and dialog body, dialog footer action.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Size {
    Xs,
    Sm,
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
    Loading,
}

/// Page colour behind a row. Secondary retargets hover, selected and pressed fills.
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
