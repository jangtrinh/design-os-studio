//! Whiteboard capability checks on [`App`].
//!
//! Each method names the editor behaviour it gates rather than querying [`App::is_board`] at call
//! sites directly.

use crate::app::App;

impl App {
    /// Whether the whiteboard tool set and keyboard shortcuts are active.
    pub fn wb_tools_enabled(&self) -> bool {
        self.is_board()
    }

    /// Whether the floating whiteboard contextual bar is shown for a selection.
    pub fn wb_contextual_bar_enabled(&self) -> bool {
        self.is_board()
    }

    /// Whether whiteboard connector magnet dots and hover target tracking are active.
    pub fn wb_connectors_enabled(&self) -> bool {
        self.is_board()
    }

    /// Whether newly created sections use whiteboard fill and border defaults.
    pub fn wb_section_defaults(&self) -> bool {
        self.is_board()
    }

    /// Whether newly created text layers use the 24 px whiteboard default font size.
    pub fn wb_text_defaults(&self) -> bool {
        self.is_board()
    }

    /// Whether the canvas paints the dotted whiteboard background under page tiles.
    pub fn wb_dotted_background(&self) -> bool {
        self.is_board()
    }

    /// Whether Dev Mode (`Shift+D`) can be toggled in the active document.
    pub fn dev_mode_available(&self) -> bool {
        !self.is_board()
    }

    /// Whether `N` / `Shift+N` top-level frame stepping is available.
    pub fn frame_stepping_available(&self) -> bool {
        !self.is_board()
    }

    /// Whether canvas rulers capture right-clicks along the ruler strips.
    pub fn rulers_available(&self) -> bool {
        !self.is_board()
    }

    /// Whether the right-hand Inspector panel is shown instead of the board chrome.
    pub fn inspector_panel_visible(&self) -> bool {
        !self.is_board()
    }
}

#[cfg(test)]
mod tests {
    use crate::app::App;
    use designos_doc::{Document, DocumentKind};

    fn app_with_kind(kind: DocumentKind) -> App {
        let mut doc = Document::new("Cap test");
        doc.kind = kind;
        doc.add_page("Page 1");
        App::new(doc)
    }

    #[test]
    fn wb_capabilities_table_for_board_design_and_deck() {
        let table = [
            // (kind, wb_*, *_available / inspector_panel_visible)
            (DocumentKind::Board, true, false),
            (DocumentKind::Design, false, true),
            (DocumentKind::Deck, false, true),
        ];
        for (kind, wb_expected, design_expected) in table {
            let app = app_with_kind(kind);
            assert_eq!(app.wb_tools_enabled(), wb_expected, "{kind:?}: wb_tools_enabled");
            assert_eq!(app.wb_contextual_bar_enabled(), wb_expected, "{kind:?}: wb_contextual_bar_enabled");
            assert_eq!(app.wb_connectors_enabled(), wb_expected, "{kind:?}: wb_connectors_enabled");
            assert_eq!(app.wb_section_defaults(), wb_expected, "{kind:?}: wb_section_defaults");
            assert_eq!(app.wb_text_defaults(), wb_expected, "{kind:?}: wb_text_defaults");
            assert_eq!(app.wb_dotted_background(), wb_expected, "{kind:?}: wb_dotted_background");
            assert_eq!(app.dev_mode_available(), design_expected, "{kind:?}: dev_mode_available");
            assert_eq!(app.frame_stepping_available(), design_expected, "{kind:?}: frame_stepping_available");
            assert_eq!(app.rulers_available(), design_expected, "{kind:?}: rulers_available");
            assert_eq!(app.inspector_panel_visible(), design_expected, "{kind:?}: inspector_panel_visible");
        }
    }
}
