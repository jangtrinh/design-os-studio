//! Collections subscribed from a shared library, as the Variables view shows them: grouped under a
//! "From library" heading and read-only, like library styles. They are edited in their library file.

use crate::icons::{self, Icon};
use crate::theme;
use crate::widgets;
use designos_doc::VariableStore;
use egui::{Align2, Pos2, Rect, Ui};

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
    let (own, lib) = split_collections(view);
    own.into_iter().chain(lib).next()
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
    use designos_doc::VariableCollection;

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
}
