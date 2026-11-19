//! `History::exec` keeps the auto-layout parent of an edited child in step with the edit, in the
//! same undo step, and solves nothing else: undo and redo restore the stored layout, and a frame
//! the edit did not reach keeps the positions it has.

use designos_doc::*;

fn hug_column(doc: &mut Document, parent: NodeId) -> NodeId {
    let id = doc.alloc_id();
    let mut frame = Node::frame(id, Rect::new(0.0, 0.0, 200.0, 0.0));
    if let NodeKind::Frame(props) = &mut frame.kind {
        props.auto_layout = Some(AutoLayout {
            direction: LayoutDirection::Vertical,
            primary_sizing: Sizing::Hug,
            counter_sizing: Sizing::Hug,
            layout_version: 1,
            ..AutoLayout::default()
        });
    }
    doc.insert(parent, frame).unwrap();
    id
}

fn rect(doc: &mut Document, parent: NodeId, height: f32) -> NodeId {
    let id = doc.alloc_id();
    doc.insert(parent, Node::rectangle(id, Rect::new(0.0, 0.0, 40.0, height))).unwrap();
    id
}

fn ty(doc: &Document, id: NodeId) -> f32 {
    doc.get(id).unwrap().transform.ty
}

fn height(doc: &Document, id: NodeId) -> f32 {
    doc.get(id).unwrap().size.y
}

/// A page with one Hug column holding three 20-high rectangles, laid out.
fn column_of_three() -> (Document, NodeId, [NodeId; 3]) {
    let mut doc = Document::new("history-exec-auto-layout");
    let page = doc.add_page("P");
    let column = hug_column(&mut doc, page);
    let rows = [rect(&mut doc, column, 20.0), rect(&mut doc, column, 20.0), rect(&mut doc, column, 20.0)];
    doc.relayout_subtree(column);
    assert_eq!((ty(&doc, rows[1]), ty(&doc, rows[2]), height(&doc, column)), (20.0, 40.0, 60.0), "precondition: the column is laid out");
    (doc, column, rows)
}

#[test]
fn removing_a_child_closes_the_gap_and_undo_and_redo_restore_it() {
    let (mut doc, column, rows) = column_of_three();
    let mut history = History::default();

    history.exec(&mut doc, Command::Remove { id: rows[0] }).unwrap();
    assert_eq!((ty(&doc, rows[1]), ty(&doc, rows[2]), height(&doc, column)), (0.0, 20.0, 40.0));

    assert!(history.undo(&mut doc).unwrap());
    assert_eq!((ty(&doc, rows[0]), ty(&doc, rows[1]), ty(&doc, rows[2]), height(&doc, column)), (0.0, 20.0, 40.0, 60.0));

    assert!(history.redo(&mut doc).unwrap());
    assert_eq!((ty(&doc, rows[1]), ty(&doc, rows[2]), height(&doc, column)), (0.0, 20.0, 40.0));
}

#[test]
fn inserting_a_child_grows_the_hug_parent_and_undo_and_redo_follow() {
    let (mut doc, column, rows) = column_of_three();
    let mut history = History::default();
    let fresh = doc.alloc_id();

    history.exec(&mut doc, Command::Insert { parent: column, index: 0, nodes: vec![Node::rectangle(fresh, Rect::new(0.0, 0.0, 40.0, 30.0))] }).unwrap();
    assert_eq!((ty(&doc, fresh), ty(&doc, rows[0]), ty(&doc, rows[2]), height(&doc, column)), (0.0, 30.0, 70.0, 90.0));

    assert!(history.undo(&mut doc).unwrap());
    assert!(doc.get(fresh).is_none());
    assert_eq!((ty(&doc, rows[0]), ty(&doc, rows[2]), height(&doc, column)), (0.0, 40.0, 60.0));

    assert!(history.redo(&mut doc).unwrap());
    assert_eq!((ty(&doc, fresh), ty(&doc, rows[0]), ty(&doc, rows[2]), height(&doc, column)), (0.0, 30.0, 70.0, 90.0));
}

#[test]
fn reparenting_a_child_re_solves_both_the_old_and_the_new_column() {
    let (mut doc, column, rows) = column_of_three();
    let page = doc.get(column).unwrap().parent.unwrap();
    let other = hug_column(&mut doc, page);
    let resident = rect(&mut doc, other, 10.0);
    doc.relayout_subtree(other);
    let mut history = History::default();

    history.exec(&mut doc, Command::Reparent { id: rows[0], parent: other, index: 0 }).unwrap();
    assert_eq!((ty(&doc, rows[1]), height(&doc, column)), (0.0, 40.0), "the old column closed the gap");
    assert_eq!((ty(&doc, rows[0]), ty(&doc, resident), height(&doc, other)), (0.0, 20.0, 30.0), "the new column made room");

    assert!(history.undo(&mut doc).unwrap());
    assert_eq!((ty(&doc, rows[1]), height(&doc, column)), (20.0, 60.0));
    assert_eq!((ty(&doc, resident), height(&doc, other)), (0.0, 10.0));

    assert!(history.redo(&mut doc).unwrap());
    assert_eq!((ty(&doc, rows[1]), height(&doc, column)), (0.0, 40.0));
    assert_eq!((ty(&doc, resident), height(&doc, other)), (20.0, 30.0));
}

#[test]
fn a_resize_bubbles_through_nested_hug_columns() {
    let mut doc = Document::new("history-exec-nested");
    let page = doc.add_page("P");
    let outer = hug_column(&mut doc, page);
    let inner = hug_column(&mut doc, outer);
    let leaf = rect(&mut doc, inner, 20.0);
    let below = rect(&mut doc, outer, 20.0);
    doc.relayout_subtree(outer);
    assert_eq!((ty(&doc, below), height(&doc, outer)), (20.0, 40.0), "precondition");
    let mut history = History::default();

    let mut edited = doc.get(leaf).unwrap().clone();
    edited.size.y = 70.0;
    history.exec(&mut doc, Command::Update { node: Box::new(edited) }).unwrap();

    assert_eq!(height(&doc, inner), 70.0);
    assert_eq!((ty(&doc, below), height(&doc, outer)), (70.0, 90.0));

    assert!(history.undo(&mut doc).unwrap());
    assert_eq!((ty(&doc, below), height(&doc, outer)), (20.0, 40.0));
    assert!(history.redo(&mut doc).unwrap());
    assert_eq!((ty(&doc, below), height(&doc, outer)), (70.0, 90.0));
}

#[test]
fn a_frame_the_edit_did_not_reach_is_not_re_solved() {
    let (mut doc, _column, rows) = column_of_three();
    let page = doc.get(rows[0]).and_then(|n| n.parent).and_then(|c| doc.get(c)).and_then(|n| n.parent).unwrap();
    let bystander = hug_column(&mut doc, page);
    let bystander_rows = [rect(&mut doc, bystander, 20.0), rect(&mut doc, bystander, 20.0)];
    doc.relayout_subtree(bystander);
    // Move a child off its flow slot by hand. A solve of this frame would put it back at 20.
    doc.get_mut(bystander_rows[1]).unwrap().transform.ty = 555.0;
    let mut history = History::default();

    let mut edited = doc.get(rows[0]).unwrap().clone();
    edited.size.y = 80.0;
    history.exec(&mut doc, Command::Update { node: Box::new(edited) }).unwrap();
    assert_eq!(ty(&doc, bystander_rows[1]), 555.0, "an edit in another frame must not re-solve this one");

    assert!(history.undo(&mut doc).unwrap());
    assert!(history.redo(&mut doc).unwrap());
    assert_eq!(ty(&doc, bystander_rows[1]), 555.0, "undo and redo of that edit leave it alone as well");
}

#[test]
fn an_edit_that_changes_no_layout_input_solves_nothing() {
    let (mut doc, column, rows) = column_of_three();
    // Same hand-placed marker as above: only a solve of the column would move it.
    doc.get_mut(rows[2]).unwrap().transform.ty = 555.0;
    let mut history = History::default();

    let mut renamed = doc.get(rows[0]).unwrap().clone();
    renamed.name = "Renamed".into();
    history.exec(&mut doc, Command::Update { node: Box::new(renamed) }).unwrap();

    assert_eq!(ty(&doc, rows[2]), 555.0, "a rename is not a layout input");
    assert_eq!(height(&doc, column), 60.0);
}
