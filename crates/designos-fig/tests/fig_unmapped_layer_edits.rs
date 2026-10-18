//! Layers the importer cannot map to a typed node (TABLE, SLIDE, ...) keep their children. Edits on
//! those children must stay undoable: the parent has to be accepted by every command whose inverse
//! puts a child back.

use designos_doc::*;
use designos_fig::import_fig_node_changes;
use serde_json::{json, Value};

fn find<'a>(doc: &'a Document, name: &str) -> &'a Node {
    doc.nodes.values().find(|n| n.name == name).unwrap_or_else(|| panic!("no node named {name}"))
}

fn child_names(doc: &Document, parent: NodeId) -> Vec<String> {
    doc.get(parent).expect("parent node").children.iter().map(|c| doc.get(*c).expect("child node").name.clone()).collect()
}

/// A TABLE with three cells, plus a plain frame to move a cell into.
fn table_doc() -> Document {
    let g = |s: u64, l: u64| -> Value { json!({"sessionID": s, "localID": l}) };
    let node = |id: (u64, u64), parent: (u64, u64), pos: &str, ty: &str, name: &str| -> Value {
        json!({
            "guid": g(id.0, id.1),
            "parentIndex": {"guid": g(parent.0, parent.1), "position": pos},
            "type": ty,
            "name": name,
            "phase": "CREATED",
            "visible": true,
            "opacity": 1.0,
            "size": {"x": 120.0, "y": 60.0},
            "transform": {"m00": 1.0, "m01": 0.0, "m02": 0.0, "m10": 0.0, "m11": 1.0, "m12": 0.0}
        })
    };
    let changes = Value::Array(vec![
        node((0, 0), (0, 0), "", "DOCUMENT", "TableDoc"),
        node((0, 1), (0, 0), "!", "CANVAS", "Page 1"),
        node((1, 1), (0, 1), "!", "TABLE", "Pricing"),
        node((1, 2), (1, 1), "!", "TABLE_CELL", "Cell A"),
        node((1, 3), (1, 1), "\"", "TABLE_CELL", "Cell B"),
        node((1, 4), (1, 1), "#", "TABLE_CELL", "Cell C"),
        node((1, 5), (0, 1), "\"", "FRAME", "Elsewhere"),
    ]);
    import_fig_node_changes(&changes, "TableDoc").expect("import node changes")
}

#[test]
fn deleting_a_table_cell_can_be_undone_at_the_same_index() {
    let mut doc = table_doc();
    let (table, cell) = (find(&doc, "Pricing").id, find(&doc, "Cell B").id);
    assert!(matches!(doc.get(table).unwrap().kind, NodeKind::Unknown { .. }), "fixture: the table is an unmapped layer");
    assert_eq!(child_names(&doc, table), ["Cell A", "Cell B", "Cell C"], "fixture: the importer keeps the cells under the table");

    let mut history = History::default();
    history.exec(&mut doc, Command::Remove { id: cell }).expect("delete the cell");
    assert_eq!(child_names(&doc, table), ["Cell A", "Cell C"]);

    assert!(history.undo(&mut doc).expect("undo of the delete"), "undo had nothing to undo");
    assert_eq!(child_names(&doc, table), ["Cell A", "Cell B", "Cell C"], "the cell is back at its old index");
    assert_eq!(doc.get(cell).expect("restored cell").parent, Some(table));

    assert!(history.redo(&mut doc).expect("redo of the delete"));
    assert_eq!(child_names(&doc, table), ["Cell A", "Cell C"]);
    assert!(history.undo(&mut doc).expect("second undo"));
    assert_eq!(child_names(&doc, table), ["Cell A", "Cell B", "Cell C"]);
}

#[test]
fn moving_a_table_cell_out_can_be_undone_at_the_same_index() {
    let mut doc = table_doc();
    let (table, cell, elsewhere) = (find(&doc, "Pricing").id, find(&doc, "Cell B").id, find(&doc, "Elsewhere").id);

    let mut history = History::default();
    history.exec(&mut doc, Command::Reparent { id: cell, parent: elsewhere, index: 0 }).expect("move the cell out");
    assert_eq!(child_names(&doc, table), ["Cell A", "Cell C"]);
    assert_eq!(doc.get(cell).unwrap().parent, Some(elsewhere));

    assert!(history.undo(&mut doc).expect("undo of the move"), "undo had nothing to undo");
    assert_eq!(child_names(&doc, table), ["Cell A", "Cell B", "Cell C"], "the cell is back at its old index");
    assert_eq!(doc.get(cell).unwrap().parent, Some(table));
    assert!(child_names(&doc, elsewhere).is_empty());

    assert!(history.redo(&mut doc).expect("redo of the move"));
    assert_eq!(doc.get(cell).unwrap().parent, Some(elsewhere));
}

#[test]
fn an_unmapped_layer_accepts_new_children_and_moved_in_layers() {
    let mut doc = table_doc();
    let (table, elsewhere) = (find(&doc, "Pricing").id, find(&doc, "Elsewhere").id);

    let id = doc.alloc_id();
    let mut added = Node::rectangle(id, Rect::new(0.0, 0.0, 10.0, 10.0));
    added.name = "Added".into();
    doc.insert(table, added).expect("insert under an unmapped layer");
    assert_eq!(child_names(&doc, table), ["Cell A", "Cell B", "Cell C", "Added"]);

    doc.reparent(id, elsewhere, 0).expect("move out");
    doc.reparent(id, table, 1).expect("move back in at an index");
    assert_eq!(child_names(&doc, table), ["Cell A", "Added", "Cell B", "Cell C"]);

    let mut history = History::default();
    let id2 = doc.alloc_id();
    let mut pasted = Node::rectangle(id2, Rect::new(0.0, 0.0, 10.0, 10.0));
    pasted.name = "Pasted".into();
    history.exec(&mut doc, Command::Insert { parent: table, index: 0, nodes: vec![pasted] }).expect("command insert under an unmapped layer");
    assert_eq!(child_names(&doc, table)[0], "Pasted");
    assert!(history.undo(&mut doc).expect("undo of the insert"));
    assert_eq!(child_names(&doc, table), ["Cell A", "Added", "Cell B", "Cell C"]);
}

#[test]
fn leaf_layers_still_refuse_children() {
    let mut doc = table_doc();
    let (cell, elsewhere) = (find(&doc, "Cell A").id, find(&doc, "Elsewhere").id);
    let rect_id = doc.alloc_id();
    let rect = doc.insert(elsewhere, Node::rectangle(rect_id, Rect::new(0.0, 0.0, 10.0, 10.0))).unwrap();

    let id = doc.alloc_id();
    assert!(matches!(doc.insert(rect, Node::rectangle(id, Rect::new(0.0, 0.0, 1.0, 1.0))), Err(DocError::NotAContainer(p)) if p == rect));
    assert!(matches!(doc.reparent(cell, rect, 0), Err(DocError::NotAContainer(p)) if p == rect));
}
