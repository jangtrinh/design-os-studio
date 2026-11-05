//! Selection commands: select all / inverse / children / parent and the
//! "select matching layers" family.

use crate::app::App;
use designos_doc::find::{self, MatchCriteria};
use designos_doc::NodeId;

impl App {
    /// Applies a computed selection; an empty result leaves the selection alone (with a hint).
    fn set_selection_from(&mut self, ids: Vec<NodeId>, empty_hint: &str) {
        if ids.is_empty() {
            if !empty_hint.is_empty() {
                self.show_toast(empty_hint, false);
            }
            return;
        }
        self.selection = ids;
    }

    /// Ctrl+A: all layers next to the selection (the page's top level when nothing is selected).
    pub fn select_all_layers(&mut self) {
        let ids = find::select_all(&self.doc, self.page, &self.selection);
        self.set_selection_from(ids, "");
    }

    /// Ctrl+Shift+A: the other layers inside the selection's parent.
    pub fn select_inverse(&mut self) {
        let ids = find::select_inverse(&self.doc, self.page, &self.selection);
        if ids.is_empty() {
            self.clear_selection();
        } else {
            self.selection = ids;
        }
    }

    /// Enter: the children of the selected layers.
    pub fn select_children(&mut self) {
        let ids = find::select_children(&self.doc, &self.selection);
        self.set_selection_from(ids, "");
    }

    /// Shift+Enter or `\`: the parents of the selected layers (never the page).
    pub fn select_parent(&mut self) {
        let ids = find::select_parent(&self.doc, &self.selection);
        self.set_selection_from(ids, "");
    }

    /// Tab / Shift+Tab: the next or previous sibling layer in the same parent.
    pub fn select_sibling(&mut self, forward: bool) {
        let ids = find::select_sibling(&self.doc, self.page, &self.selection, forward);
        self.set_selection_from(ids, "");
    }

    /// Selects every layer on the page matching the selected ones on `criteria`.
    pub fn select_matching(&mut self, criteria: MatchCriteria) {
        if self.selection.is_empty() {
            return;
        }
        let ids = find::select_matching_all(&self.doc, &self.selection, criteria);
        let unchanged = ids.len() == self.selection.len() && ids.iter().all(|id| self.selection.contains(id));
        if unchanged {
            let what = match criteria {
                MatchCriteria::Fill => "fill",
                MatchCriteria::Stroke => "stroke",
                MatchCriteria::Effect => "effect",
                MatchCriteria::TextProperties => "text properties",
                MatchCriteria::Font => "font",
                MatchCriteria::Instance => "main component",
                MatchCriteria::Variant => "variant",
                MatchCriteria::Type => "type",
                MatchCriteria::Name => "name",
                MatchCriteria::Properties => "properties",
            };
            self.show_toast(format!("No other layers with the same {what}"), false);
        }
        self.set_selection_from(ids, "");
    }
}
