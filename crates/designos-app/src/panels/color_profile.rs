//! The document's colour profile in the Design panel: a dropdown on the title row of the Page
//! section (shown while nothing is selected), where the page colour it applies to is set.

use super::properties::Pane;
use crate::app::App;
use crate::theme;
use crate::widgets::menu::dropdown;
use crate::widgets::record;
use designos_doc::{ColorProfile, Command};
use egui::{Rect, Sense, Ui};

/// Left edge and width of the dropdown on the title row. It ends 8 px before the trailing action
/// button and clears the "Page" title at the left.
const DROPDOWN_W: f32 = 112.0;
const DROPDOWN_X: f32 = theme::SECTION_TRAILING_X - 8.0 - DROPDOWN_W;
/// Inset that centres the 32 px field in the 40 px title row.
const DROPDOWN_DY: f32 = (theme::SECTION_TITLE_HEIGHT - theme::FIELD_HEIGHT) / 2.0;

/// Draws the dropdown on the title row just above `pane.y` (call right after the title).
pub(super) fn control(app: &mut App, ui: &mut Ui, pane: &mut Pane) {
    let r: Rect = pane.rect(DROPDOWN_X, -theme::SECTION_TITLE_HEIGHT + DROPDOWN_DY, DROPDOWN_W, theme::FIELD_HEIGHT);
    record(ui, "color-profile", r);
    let names: Vec<String> = ColorProfile::ALL.iter().map(|p| p.label().to_string()).collect();
    if let Some(i) = dropdown(ui, r, "color-profile", app.doc.color_profile.label(), &names) {
        app.set_color_profile(ColorProfile::ALL[i]);
    }
    ui.interact(r, ui.id().with("color-profile-tip"), Sense::hover()).on_hover_text("Color profile");
}

impl App {
    /// Sets the colour profile of the document as one undo step. Every colour is redrawn.
    pub fn set_color_profile(&mut self, profile: ColorProfile) {
        if profile == self.doc.color_profile {
            return;
        }
        if let Err(e) = self.hist.exec(&mut self.doc, Command::SetColorProfile { profile }) {
            self.show_toast(format!("Edit failed: {e}"), true);
        }
        crate::widgets::set_document_profile(self.doc.color_profile);
        self.changed();
    }
}
