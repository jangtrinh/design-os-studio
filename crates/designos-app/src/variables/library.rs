//! Collections subscribed from a shared library, as the Variables view shows them: grouped under a
//! "From library" heading and read-only, like library styles. They are edited in their library file.

use crate::icons::{self, Icon};
use crate::theme;
use crate::widgets;
use designos_doc::{Variable, VariableCollection, VariableStore};
use egui::{Align2, Pos2, Rect, Ui};
use std::collections::{BTreeMap, BTreeSet};

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
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VariablesImport {
    /// Variables of the payload that were merged into the file.
    pub imported: usize,
    /// Variables of the payload left out because a library record is involved: they belong to a
    /// library collection the file already has, which is edited in its library file, or they are
    /// library variables that would take the id of one of the file's own.
    pub skipped_library: usize,
}

#[allow(dead_code)]
impl VariablesImport {
    /// The toast text.
    pub fn message(&self) -> String {
        match self.skipped_library {
            0 => format!("Imported {} variables", self.imported),
            n => format!("Imported {} variables; {n} in library collections were skipped", self.imported),
        }
    }
}

/// Drops from an incoming payload every record the import must not apply to `store`, and returns
/// how many variables that was. A library record and a record of the file never replace each other:
/// nothing lands in a library collection the file already has, and a library collection or variable
/// is not imported under the id of one the file owns. Figma ids repeat across files, so both happen.
pub fn drop_refused_records(
    store: &VariableStore,
    collections: &mut BTreeMap<String, VariableCollection>,
    variables: &mut BTreeMap<String, Variable>,
) -> usize {
    let refused: BTreeSet<String> = collections
        .iter()
        .filter(|(id, c)| store.collections.get(*id).is_some_and(|known| known.is_library() || c.is_library()))
        .map(|(id, _)| id.clone())
        .collect();
    collections.retain(|id, _| !refused.contains(id));
    let before = variables.len();
    variables.retain(|id, v| {
        let incoming_library = collections.get(&v.collection).is_some_and(VariableCollection::is_library);
        let clash = store.variables.contains_key(id) && (incoming_library || store.is_library_variable(id));
        !clash && !refused.contains(&v.collection) && !store.is_library_collection(&v.collection)
    });
    // the collections that stay must not keep listing what was dropped
    for c in collections.values_mut() {
        c.variables.retain(|id| variables.contains_key(id));
    }
    before - variables.len()
}

/// The sidebar heading row.
pub fn paint_heading(ui: &Ui, row: Rect) {
    let p = ui.painter();
    p.text(Pos2::new(row.left() + 7.0, row.center().y), Align2::LEFT_CENTER, HEADING, theme::font_caption_medium(), theme::TEXT_TERTIARY);
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
            extensions: BTreeMap::new(),
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
        collections.get_mut(&own).unwrap().variables.push(lib_var);

        assert_eq!(drop_refused_records(&store, &mut collections, &mut variables), 2);
        assert_eq!(collections.keys().collect::<Vec<_>>(), [&own]);
        assert_eq!(variables.keys().collect::<Vec<_>>(), [&own_var]);
        assert_eq!(collections[&own].variables, vec![own_var.clone()], "the surviving collection lists only what survived");
    }

    #[test]
    fn a_library_record_never_takes_the_id_of_one_the_file_owns() {
        let mut store = VariableStore::default();
        let own = store.add_collection("Mine");
        let own_var = store.add_variable(&own, "Gap", VariableKind::Number).unwrap();
        let var_in = |id: &str, collection: &str| Variable { id: id.into(), collection: collection.into(), ..store.variables[&own_var].clone() };

        // a library collection under the id of the file's own collection, with a variable of its own;
        // and a new library collection whose variables are one that reuses the file's variable id and one that is new
        let mut same_id = coll(&own, Some("lk"));
        same_id.variables = vec!["inside".into()];
        let mut new_lib = coll("new", Some("lk"));
        new_lib.variables = vec![own_var.clone(), "fresh".into()];
        let mut collections = BTreeMap::from([(own.clone(), same_id), ("new".to_string(), new_lib)]);
        let mut variables = BTreeMap::from([
            ("inside".to_string(), var_in("inside", &own)),
            (own_var.clone(), var_in(&own_var, "new")),
            ("fresh".to_string(), var_in("fresh", "new")),
        ]);

        assert_eq!(drop_refused_records(&store, &mut collections, &mut variables), 2);
        assert_eq!(collections.keys().collect::<Vec<_>>(), ["new"], "the new library collection still comes in");
        assert_eq!(variables.keys().collect::<Vec<_>>(), ["fresh"]);
        assert_eq!(collections["new"].variables, vec!["fresh".to_string()]);
    }

    #[test]
    fn a_local_record_still_replaces_a_local_record_of_the_same_id() {
        let mut store = VariableStore::default();
        let own = store.add_collection("Mine");
        let own_var = store.add_variable(&own, "Gap", VariableKind::Number).unwrap();
        let (mut collections, mut variables) = (store.collections.clone(), store.variables.clone());
        variables.get_mut(&own_var).unwrap().name = "Spacing".into();
        assert_eq!(drop_refused_records(&store, &mut collections, &mut variables), 0);
        assert_eq!((collections.len(), variables[&own_var].name.as_str()), (1, "Spacing"));
    }
}
