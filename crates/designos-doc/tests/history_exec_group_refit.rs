//! `History::exec` refits the groups whose children a command added, removed or moved, and writes what it solved
//! into the step's inverse: undo and redo put every node back exactly, also in a document that is not at the
//! solver's fixed point. `exec_laid_out` solves nothing, and `settle_layout_after` gives a trial copy what `exec`
//! gives the committed document.

use designos_doc::*;

fn column(doc: &mut Document, parent: NodeId, sizing: Sizing, size: (f32, f32)) -> NodeId {
    let id = doc.alloc_id();
    let mut frame = Node::frame(id, Rect::new(0.0, 0.0, size.0, size.1));
    if let NodeKind::Frame(props) = &mut frame.kind {
        props.auto_layout = Some(AutoLayout {
            direction: LayoutDirection::Vertical,
            primary_sizing: sizing,
            counter_sizing: sizing,
            layout_version: 1,
            ..AutoLayout::default()
        });
    }
    doc.insert(parent, frame).unwrap();
    id
}

fn rect(doc: &mut Document, parent: NodeId, at: (f32, f32), size: (f32, f32)) -> NodeId {
    let id = doc.alloc_id();
    doc.insert(parent, Node::rectangle(id, Rect::new(at.0, at.1, size.0, size.1))).unwrap();
    id
}

fn group(doc: &mut Document, parent: NodeId, at: (f32, f32), size: (f32, f32)) -> NodeId {
    let id = doc.alloc_id();
    let mut node = Node::frame(id, Rect::new(at.0, at.1, size.0, size.1));
    node.kind = NodeKind::Group;
    doc.insert(parent, node).unwrap();
    id
}

fn stored(doc: &Document) -> Vec<(NodeId, [f32; 4])> {
    doc.nodes.values().map(|n| (n.id, [n.transform.tx, n.transform.ty, n.size.x, n.size.y])).collect()
}

fn page_x(doc: &Document, id: NodeId) -> f32 {
    doc.world_transform(id).tx
}

/// page > group (80 x 20) > [a at 0, b at 40], both 40 x 20.
fn group_of_two(at: (f32, f32)) -> (Document, NodeId, NodeId, NodeId) {
    let mut doc = Document::new("history-exec-group-refit");
    let page = doc.add_page("P");
    let g = group(&mut doc, page, at, (80.0, 20.0));
    let a = rect(&mut doc, g, (0.0, 0.0), (40.0, 20.0));
    let b = rect(&mut doc, g, (40.0, 0.0), (40.0, 20.0));
    (doc, g, a, b)
}

#[test]
fn removing_a_layer_refits_its_group_and_undo_and_redo_restore_it_exactly() {
    let (mut doc, g, _a, b) = group_of_two((0.0, 0.0));
    let before = stored(&doc);
    let mut history = History::default();

    history.exec(&mut doc, Command::Remove { id: b }).unwrap();
    assert_eq!(doc.get(g).unwrap().size, Vec2::new(40.0, 20.0), "the group fits the layer that is left");
    let after = stored(&doc);

    assert!(history.undo(&mut doc).unwrap());
    assert_eq!(stored(&doc), before, "undo puts the group, the layer that stayed and the removed one back");
    assert!(history.redo(&mut doc).unwrap());
    assert_eq!(stored(&doc), after, "redo refits the group to the same bounds");
}

#[test]
fn removing_the_first_layer_keeps_the_other_where_it_is_on_the_page() {
    let (mut doc, g, a, b) = group_of_two((100.0, 0.0));
    assert_eq!(page_x(&doc, b), 140.0);
    let before = stored(&doc);
    let mut history = History::default();

    history.exec(&mut doc, Command::Remove { id: a }).unwrap();
    assert_eq!(doc.get(g).unwrap().size, Vec2::new(40.0, 20.0));
    assert_eq!(page_x(&doc, b), 140.0, "the group moved to the layer that is left, which stayed where it was");

    assert!(history.undo(&mut doc).unwrap());
    assert_eq!(stored(&doc), before);
}

#[test]
fn a_smaller_group_moves_the_flow_around_it_and_undo_brings_the_stored_places_back() {
    // screen (column, fixed) > [group, tail]. The tail is stored below the place the flow would give it, so the
    // document is not at the solver's fixed point: an undo that solved again could not bring the tail back.
    let mut doc = Document::new("history-exec-group-in-flow");
    let page = doc.add_page("P");
    let screen = column(&mut doc, page, Sizing::Fixed, (400.0, 600.0));
    let g = group(&mut doc, screen, (0.0, 0.0), (80.0, 20.0));
    let _a = rect(&mut doc, g, (0.0, 0.0), (40.0, 20.0));
    let b = rect(&mut doc, g, (40.0, 0.0), (40.0, 20.0));
    let tail = rect(&mut doc, screen, (0.0, 200.0), (100.0, 30.0));
    let before = stored(&doc);
    let mut history = History::default();

    history.exec(&mut doc, Command::Remove { id: b }).unwrap();
    assert_eq!(doc.get(tail).unwrap().transform.ty, 20.0, "the screen follows the group, which is as tall as before but narrower");
    assert!(history.peek_undo_layout_recorded(), "the step holds every layout write it caused");

    assert!(history.undo(&mut doc).unwrap());
    assert_eq!(stored(&doc), before, "the tail is back at 200, where the document stored it");
    assert!(history.redo(&mut doc).unwrap());
    assert_eq!(doc.get(tail).unwrap().transform.ty, 20.0);
}

#[test]
fn inserting_a_layer_far_from_the_group_origin_refits_the_group() {
    let (mut doc, g, a, _b) = group_of_two((0.0, 0.0));
    let before = stored(&doc);
    let extra = doc.alloc_id();
    let mut history = History::default();

    history.exec(&mut doc, Command::Insert { parent: g, index: 2, nodes: vec![Node::rectangle(extra, Rect::new(100.0, 0.0, 20.0, 20.0))] }).unwrap();
    assert_eq!(doc.get(g).unwrap().size, Vec2::new(120.0, 20.0));
    assert_eq!(page_x(&doc, a), 0.0);

    assert!(history.undo(&mut doc).unwrap());
    assert_eq!(stored(&doc), before);
}

#[test]
fn exec_laid_out_solves_and_refits_nothing() {
    let (mut doc, g, _a, b) = group_of_two((0.0, 0.0));
    let mut history = History::default();

    history.exec_laid_out(&mut doc, Command::Remove { id: b }, None).unwrap();
    assert_eq!(doc.get(g).unwrap().size, Vec2::new(80.0, 20.0), "the caller owns the layout of this step");
    assert!(!history.peek_undo_layout_recorded());
    assert!(history.undo(&mut doc).unwrap());
    assert!(doc.get(b).is_some());
}

#[test]
fn a_connector_is_not_a_layout_input_of_the_frame_that_holds_it() {
    let mut doc = Document::new("history-exec-connector");
    let page = doc.add_page("P");
    let screen = column(&mut doc, page, Sizing::Fixed, (400.0, 600.0));
    let tail = rect(&mut doc, screen, (0.0, 200.0), (100.0, 30.0));
    let mut history = History::default();

    let id = doc.alloc_id();
    history.exec(&mut doc, Command::Insert { parent: screen, index: 1, nodes: vec![Node::connector(id, ConnectorProps::default())] }).unwrap();
    assert_eq!(doc.get(tail).unwrap().transform.ty, 200.0, "routing owns a connector; the frame around it is not solved");
}

#[test]
fn an_insert_and_a_reparent_in_one_batch_solve_the_column_the_layer_ends_in() {
    let mut doc = Document::new("history-exec-insert-reparent");
    let page = doc.add_page("P");
    let col = column(&mut doc, page, Sizing::Hug, (200.0, 0.0));
    let first = rect(&mut doc, col, (0.0, 0.0), (40.0, 20.0));
    doc.relayout_subtree(col);
    let fresh = doc.alloc_id();
    let mut history = History::default();

    history
        .exec(
            &mut doc,
            Command::Batch(vec![
                Command::Insert { parent: page, index: 1, nodes: vec![Node::rectangle(fresh, Rect::new(0.0, 0.0, 40.0, 30.0))] },
                Command::Reparent { id: fresh, parent: col, index: 0 },
            ]),
        )
        .unwrap();
    assert_eq!(doc.get(col).unwrap().size.y, 50.0, "the column hugs both layers");
    assert_eq!(doc.get(first).unwrap().transform.ty, 30.0);
}

#[test]
fn settle_layout_after_gives_a_trial_copy_what_exec_gives_the_committed_document() {
    let mut doc = Document::new("history-exec-trial");
    let page = doc.add_page("P");
    let screen = column(&mut doc, page, Sizing::Fixed, (400.0, 600.0));
    let g = group(&mut doc, screen, (0.0, 0.0), (80.0, 20.0));
    let _a = rect(&mut doc, g, (0.0, 0.0), (40.0, 20.0));
    let b = rect(&mut doc, g, (40.0, 0.0), (40.0, 20.0));
    let _tail = rect(&mut doc, screen, (0.0, 200.0), (100.0, 30.0));
    let cmds = vec![Command::Remove { id: b }];

    let mut committed = doc.clone();
    History::default().exec(&mut committed, Command::Batch(cmds.clone())).unwrap();

    let mut trial = doc.clone();
    for cmd in cmds.iter().cloned() {
        cmd.apply(&mut trial).unwrap();
    }
    assert_ne!(stored(&trial), stored(&committed), "precondition: the commands alone do not lay anything out");
    settle_layout_after(&doc, &mut trial, &cmds);
    assert_eq!(stored(&trial), stored(&committed));
}

/// A hug frame the document stored at a size the solver would not give it (an imported file): an edit that makes it
/// solve, and the undo of that edit, leave the stored size and places exactly where they were.
#[test]
fn undo_of_a_resize_inside_a_stored_hug_frame_restores_its_stored_size_and_places() {
    let mut doc = Document::new("history-exec-stored-hug");
    let page = doc.add_page("P");
    let hug = column(&mut doc, page, Sizing::Hug, (200.0, 500.0));
    let a = rect(&mut doc, hug, (0.0, 0.0), (40.0, 20.0));
    let b = rect(&mut doc, hug, (0.0, 200.0), (40.0, 20.0));
    let before = stored(&doc);
    let mut history = History::default();

    let mut taller = doc.get(a).unwrap().clone();
    taller.size.y = 30.0;
    history.exec(&mut doc, Command::Update { node: Box::new(taller) }).unwrap();
    assert_eq!(doc.get(hug).unwrap().size.y, 50.0, "the frame hugs the taller layer");
    assert_eq!(doc.get(b).unwrap().transform.ty, 30.0);
    let after = stored(&doc);

    assert!(history.undo(&mut doc).unwrap());
    assert_eq!(stored(&doc), before, "undo brings back the stored 500 px frame and the layer at 200");
    assert!(history.redo(&mut doc).unwrap());
    assert_eq!(stored(&doc), after);
}
