//! Table edit operations: one undo step each, the guards at commit points, and a random-op fuzz.

use super::*;
use crate::*;

fn auto_layout_frame_with_table_and_rect() -> (Document, NodeId, NodeId, NodeId) {
    let mut d = Document::new("N4");
    let page = d.add_page("P");
    let frame = d.alloc_id();
    let mut f = Node::frame(frame, Rect::new(0.0, 0.0, 300.0, 300.0));
    if let NodeKind::Frame(fp) = &mut f.kind {
        fp.auto_layout = Some(AutoLayout {
            direction: LayoutDirection::Vertical,
            primary_sizing: Sizing::Hug,
            counter_sizing: Sizing::Hug,
            item_spacing: 10.0,
            ..AutoLayout::default()
        });
    }
    d.insert(page, f).unwrap();
    let nodes = build_table(&mut d, 2, 2, Vec2::new(0.0, 0.0));
    let table = nodes[0].id;
    Command::Insert { parent: frame, index: 0, nodes }.apply(&mut d).unwrap();
    let rect = d.alloc_id();
    d.insert(frame, Node::rectangle(rect, Rect::new(0.0, 500.0, 40.0, 40.0))).unwrap();
    (d, frame, table, rect)
}

fn geometry(d: &Document, ids: &[NodeId]) -> Vec<(f32, f32, f32, f32)> {
    ids.iter().map(|id| d.get(*id).unwrap()).map(|n| (n.transform.tx, n.transform.ty, n.size.x, n.size.y)).collect()
}

#[test]
fn a_refused_undo_through_the_auto_layout_path_also_undoes_the_layout_pass() {
    let (mut d, frame, table, rect) = auto_layout_frame_with_table_and_rect();
    let mut h = History::default();
    let mut grown = d.get(table).unwrap().clone();
    let NodeKind::Table(p) = &mut grown.kind else { unreachable!() };
    let row = p.alloc_track();
    p.rows.push(TableTrack { id: row, size: 44.0 });
    let (row_id, col_ids) = (row, [p.cols[0].id, p.cols[1].id]);
    grown.size = Vec2::new(grown.size.x, grown.size.y + 44.0);
    let mut step = vec![Command::Update { node: Box::new(grown) }];
    for col in col_ids {
        let id = d.alloc_id();
        let kind = NodeKind::TableCell(TableCellProps { row: row_id, col, text: default_cell_text("") });
        step.push(Command::Insert { parent: table, index: 99, nodes: vec![Node::new(id, kind)] });
    }
    h.exec_user(&mut d, Command::Batch(step)).unwrap();
    d.relayout_subtree(frame);
    d.flush_pending_relayout();
    // behind History's back: a cell goes missing, so the undo leaves the table broken
    let victim = d.get(table).unwrap().children[0];
    d.remove(victim).unwrap();
    d.relayout_subtree(frame);
    d.flush_pending_relayout();
    let ids = [frame, table, rect];
    let before = geometry(&d, &ids);
    let depth = h.undo_depth();
    let err = h.undo(&mut d).expect_err("a table without one of its cells is refused");
    assert!(matches!(err, DocError::Table(_)), "{err:?}");
    assert_eq!((geometry(&d, &ids), h.undo_depth(), h.can_redo()), (before, depth, false), "layout pass was undone with the step");
}
