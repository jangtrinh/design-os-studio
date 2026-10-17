//! Collections subscribed from a shared library, as the Variables view shows them: grouped under a
//! "From library" heading and read-only, like library styles. They are edited in their library file.

use crate::icons::{self, Icon};
use crate::theme;
use crate::widgets;
use designos_doc::{Variable, VariableCollection, VariableStore};
use egui::{Align2, Pos2, Rect, Ui};
use std::collections::BTreeMap;

/// Hover text and hint for anything that would edit a library collection.
pub const LIBRARY_TIP: &str = "Edit in the library file";

/// Heading above the library collections in the sidebar.
pub const HEADING: &str = "From library";

/// Collection ids in sidebar order: the file's own collections first, then the library ones.
pub fn split_collections(view: &VariableStore) -> (Vec<String>, Vec<String>) {
    view.collections.iter().map(|(id, c)| (id.clone(), c.is_library())).fold((vec![], vec![]), |(mut own, mut lib), (id, is_lib)| {
        if is_lib { &mut lib } else { &mut own }.push(id);
        (own, lib)
    })
}

/// The collection the view opens on: the first one the file owns, else the first library one.
pub fn first_collection(view: &VariableStore) -> Option<String> {
    first_collection_of(&view.collections)
}

/// [`first_collection`] for a bare collection map, e.g. the document's own. Map order is string
/// order, so it is not the order in which Figma created the collections: `…:1:10` precedes `…:1:2`.
pub fn first_collection_of(collections: &BTreeMap<String, VariableCollection>) -> Option<String> {
    collections.iter().find(|(_, c)| !c.is_library()).or_else(|| collections.iter().next()).map(|(id, _)| id.clone())
}

/// What an import of variables did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VariablesImport {
    /// Variables of the payload that were merged into the file.
    pub imported: usize,
    /// Variables of the payload left out because they belong to a library collection the file
    /// already has: such a collection is edited in its library file, so an import cannot change it.
    pub skipped_library: usize,
}

impl VariablesImport {
    /// The toast text.
    pub fn message(&self) -> String {
        match self.skipped_library {
            0 => format!("Imported {} variables", self.imported),
            n => format!("Imported {} variables; {n} in library collections were skipped", self.imported),
        }
    }
}

/// Drops from an incoming payload everything that would land in a library collection `store`
/// already has, and returns how many variables that was.
pub fn drop_existing_library(
    store: &VariableStore,
    collections: &mut BTreeMap<String, VariableCollection>,
    variables: &mut BTreeMap<String, Variable>,
) -> usize {
    collections.retain(|id, _| !store.is_library_collection(id));
    let before = variables.len();
    variables.retain(|id, v| !store.is_library_collection(&v.collection) && !store.is_library_variable(id));
    before - variables.len()
}

/// The sidebar heading row.
pub fn paint_heading(ui: &Ui, row: Rect) {
    let p = ui.painter();
    p.text(Pos2::new(row.left() + 7.0, row.center().y), Align2::LEFT_CENTER, HEADING, theme::font_small_medium(), theme::TEXT_TERTIARY);
    widgets::record(ui, &format!("vars-heading:{HEADING}"), row);
}

/// A padlock, centred on `c`, marking a read-only collection or note.
pub fn paint_lock(ui: &Ui, c: Pos2, size: f32) {
    icons::paint(ui.painter(), Icon::Lock, c, size, theme::ICON_TERTIARY);
}

/// Label of the note that takes the place of "Create variable" while a library collection is open.
pub const NOTE: &str = "Read-only (from library)";

/// The read-only note, laid out like the "Create variable" button it replaces.
pub fn paint_read_only_note(ui: &Ui, area: Rect) {
    let cy = area.center().y;
    paint_lock(ui, Pos2::new(area.left() + 14.0, cy), 14.0);
    ui.painter().text(Pos2::new(area.left() + 34.0, cy), Align2::LEFT_CENTER, NOTE, theme::font_body(), theme::TEXT_SECONDARY);
    widgets::record(ui, "vars-readonly", area);
    ui.interact(area, egui::Id::new("vars-readonly"), egui::Sense::hover()).on_hover_text(LIBRARY_TIP);
}

#[cfg(test)]
mod tests {
    use super::*;
    use designos_doc::VariableKind;

    fn coll(id: &str, lib: Option<&str>) -> VariableCollection {
        VariableCollection {
            id: id.into(),
            name: id.into(),
            modes: vec![],
            default_mode: String::new(),
            variables: vec![],
            key: None,
            source_library_key: lib.map(String::from),
        }
    }

    #[test]
    fn own_collections_come_before_library_ones_and_open_first() {
        let mut v = VariableStore::default();
        for (id, lib) in [("a", Some("lk")), ("b", None), ("c", Some("lk")), ("d", None)] {
            v.collections.insert(id.into(), coll(id, lib));
        }
        assert_eq!(split_collections(&v), (vec!["b".into(), "d".into()], vec!["a".into(), "c".into()]));
        assert_eq!(first_collection(&v).as_deref(), Some("b"));
        v.collections.retain(|_, c| c.is_library());
        assert_eq!(first_collection(&v).as_deref(), Some("a"));
        assert_eq!(first_collection(&VariableStore::default()), None);
    }

    #[test]
    fn an_incoming_payload_loses_whatever_would_land_in_an_existing_library_collection() {
        let mut store = VariableStore::default();
        let lib = store.add_collection("Shared");
        let own = store.add_collection("Mine");
        let lib_var = store.add_variable(&lib, "Brand/Sea", VariableKind::Color).unwrap();
        let own_var = store.add_variable(&own, "Gap", VariableKind::Number).unwrap();
        store.collections.get_mut(&lib).unwrap().source_library_key = Some("lk".into());

        let (mut collections, mut variables) = (store.collections.clone(), store.variables.clone());
        // a variable the payload files under the library collection, and one that claims a library
        // variable's id from inside a collection the file owns
        let mut stray = store.variables[&own_var].clone();
        stray.id = "VariableID:stray".into();
        stray.collection = lib.clone();
        variables.insert(stray.id.clone(), stray);
        variables.get_mut(&lib_var).unwrap().collection = own.clone();

        assert_eq!(drop_existing_library(&store, &mut collections, &mut variables), 2);
        assert_eq!(collections.keys().collect::<Vec<_>>(), [&own]);
        assert_eq!(variables.keys().collect::<Vec<_>>(), [&own_var]);
    }
}
