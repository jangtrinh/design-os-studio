//! Replace all through the app's own path, on a table.

use crate::app::App;
use designos_doc::{apply_table_relayout, build_table, default_cell_text, Command, Document, NodeKind, Vec2};

/// The word is in the cell and, in the second round, in the table's layer name too: one command then
/// renames the table and rewrites the cell. The row grows to the new text either way.
#[test]
fn replace_all_grows_the_row_of_a_cell_even_when_the_table_name_matches_too() {
    for name in ["Table", "Revenue by month"] {
        let mut d = Document::new("t");
        let page = d.add_page("p");
        let mut nodes = build_table(&mut d, 3, 3, Vec2::ZERO);
        let (table, cell) = (nodes[0].id, nodes[1].id);
        nodes[0].name = name.into();
        if let NodeKind::TableCell(c) = &mut nodes[1].kind {
            c.text = default_cell_text("Revenue");
        }
        Command::Insert { parent: page, index: 0, nodes }.apply(&mut d).unwrap();
        let mut app = App::new(d);
        apply_table_relayout(&mut app.doc, table);
        let before = app.doc.get(cell).unwrap().size.y;
        app.find.query = "Revenue".into();
        app.find.replacement = "Revenue\nline 2\nline 3\nline 4\nline 5".into();
        assert_eq!(app.find_replace_all(), if name == "Table" { 1 } else { 2 }, "{name:?}");
        let after = app.doc.get(cell).unwrap().size.y;
        assert!(after > before + 40.0, "{name:?}: the row grew from {before} to {after}");
        assert!(designos_doc::table::relayout(&app.doc, table).is_empty(), "{name:?}: nothing is left to solve");
    }
}
