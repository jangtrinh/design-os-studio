//! Table edit operations: one undo step each, the guards at commit points, and a random-op fuzz.

use super::{
    apply_checked, create_table, delete_cols, delete_rows, insert_cols, insert_rows, merge, move_col, move_row, next_cell, resize_col, resize_row,
    resize_table, set_cell_text, unmerge, TableOpError, TableStyle, TABLE_MAX_SIZE, TABLE_MAX_TRACKS,
};
use crate::table::relayout;
use crate::*;
use std::collections::BTreeMap;

type OpResult = Result<Command, TableOpError>;

// ---------------------------------------------------------------- helpers

/// A page holding a `rows` x `cols` table whose cell (r, c) holds the text "r<r>c<c>".
fn doc_with_table(rows: usize, cols: usize) -> (Document, NodeId, NodeId) {
    let mut d = Document::new("Table ops");
    let page = d.add_page("P");
    let nodes = build_table(&mut d, rows, cols, Vec2::new(10.0, 20.0));
    let table = nodes[0].id;
    Command::Insert { parent: page, index: 0, nodes }.apply(&mut d).unwrap();
    for r in 0..rows {
        for c in 0..cols {
            let id = cell(&d, table, r, c);
            set_text(&mut d, id, &format!("r{r}c{c}"));
        }
    }
    (d, page, table)
}

fn props(d: &Document, table: NodeId) -> TableProps {
    match &d.get(table).unwrap().kind {
        NodeKind::Table(p) => p.clone(),
        _ => panic!("not a table"),
    }
}

/// The cell node at zero-based `(row, col)`; anchors and unmerged cells only.
fn cell(d: &Document, table: NodeId, row: usize, col: usize) -> NodeId {
    let p = props(d, table);
    cell_by_tracks(d, table, p.rows[row].id, p.cols[col].id).unwrap_or_else(|| panic!("no cell at ({row}, {col})"))
}

fn cell_by_tracks(d: &Document, table: NodeId, row: TrackId, col: TrackId) -> Option<NodeId> {
    d.get(table)?.children.iter().copied().find(|id| matches!(&d.get(*id).unwrap().kind, NodeKind::TableCell(c) if c.row == row && c.col == col))
}

fn set_text(d: &mut Document, id: NodeId, text: &str) {
    if let NodeKind::TableCell(c) = &mut d.get_mut(id).unwrap().kind {
        c.text.content = text.to_string();
    }
}

fn text_of(d: &Document, id: NodeId) -> String {
    d.get(id).unwrap().kind.board_text().unwrap().content.clone()
}

fn cell_count(d: &Document, table: NodeId) -> usize {
    d.get(table).unwrap().children.len()
}

fn geometry(d: &Document, id: NodeId) -> (f32, f32, f32, f32) {
    let n = d.get(id).unwrap();
    (n.transform.tx, n.transform.ty, n.size.x, n.size.y)
}

fn paint() -> Vec<Paint> {
    vec![Paint::solid(Color::rgb(200, 50, 50))]
}

fn track_ids(d: &Document, table: NodeId) -> (Vec<TrackId>, Vec<TrackId>) {
    let p = props(d, table);
    (p.rows.iter().map(|t| t.id).collect(), p.cols.iter().map(|t| t.id).collect())
}

/// Builds the op's command from the current document and runs it as one `exec_user` step.
fn run(d: &mut Document, h: &mut History, op: impl FnOnce(&Document) -> OpResult) {
    let cmd = op(d).expect("the op must be accepted");
    h.exec_user(d, cmd).expect("the op's command must run as one step");
}

/// Like [`run`] with a throw-away history, for set-up steps.
fn run_once(d: &mut Document, op: impl FnOnce(&Document) -> OpResult) {
    run(d, &mut History::default(), op);
}

/// Runs the op as one `exec_user` step and checks the step: one undo entry, the table whole after
/// it, `undo` restores every node exactly (same ids, same order), and `redo` restores the edit.
fn assert_one_undo_step(d: &mut Document, h: &mut History, table: NodeId, op: impl FnOnce(&Document) -> OpResult) {
    let before: BTreeMap<NodeId, Node> = d.nodes.clone();
    let depth = h.undo_depth();
    run(d, h, op);
    assert_eq!(h.undo_depth(), depth + 1, "exactly one undo step");
    let after = d.nodes.clone();
    assert!(after != before, "the op changed something");
    if d.get(table).is_some() {
        assert_eq!(check_table_positions(d, table), Ok(()));
        assert!(relayout(d, table).is_empty(), "the step already carries the layout");
    }
    assert!(h.undo(d).unwrap());
    assert_eq!(h.undo_depth(), depth, "one undo reverts the whole op");
    assert!(d.nodes == before, "undo restores the document node for node");
    assert!(h.redo(d).unwrap());
    assert!(d.nodes == after, "redo restores the edited state");
    assert_eq!(h.undo_depth(), depth + 1);
}

// ---------------------------------------------------------------- one undo step per op (AC-1)

#[test]
fn create_table_is_one_undo_step() {
    let (mut d, page, _) = doc_with_table(1, 1);
    let mut h = History::default();
    let before = d.nodes.len();
    run(&mut d, &mut h, |d| create_table(d, page, Vec2::new(300.0, 40.0), 3, 4, &TableStyle::default()));
    assert_eq!(d.nodes.len(), before + 1 + 12);
    assert_eq!(h.undo_depth(), 1);
    let table = *d.get(page).unwrap().children.last().unwrap();
    assert_eq!(check_table_positions(&d, table), Ok(()));
    assert_eq!(geometry(&d, table), (300.0, 40.0, 4.0 * TABLE_DEFAULT_COLUMN_WIDTH, 3.0 * TABLE_DEFAULT_ROW_HEIGHT));
    assert!(relayout(&d, table).is_empty());
    let after = d.nodes.clone();
    assert!(h.undo(&mut d).unwrap());
    assert_eq!(d.nodes.len(), before);
    assert!(!h.can_undo());
    assert!(h.redo(&mut d).unwrap());
    assert!(d.nodes == after);
}

#[test]
fn create_table_applies_the_style() {
    let (mut d, page, _) = doc_with_table(1, 1);
    let style = TableStyle { column_width: 100.0, row_height: 30.0, fills: paint(), stroke: None, cell_fills: paint() };
    run_once(&mut d, |d| create_table(d, page, Vec2::new(0.0, 0.0), 2, 2, &style));
    let table = *d.get(page).unwrap().children.last().unwrap();
    assert_eq!(geometry(&d, table), (0.0, 0.0, 200.0, 60.0));
    assert_eq!(d.get(table).unwrap().fills, paint());
    let last = cell(&d, table, 1, 1);
    assert_eq!(d.get(last).unwrap().fills, paint());
    assert_eq!(geometry(&d, last), (100.0, 30.0, 100.0, 30.0));
}

#[test]
fn insert_rows_and_cols_are_one_undo_step() {
    let (mut d, _, table) = doc_with_table(3, 3);
    let mut h = History::default();
    assert_one_undo_step(&mut d, &mut h, table, |d| insert_rows(d, table, 1, 2));
    assert_eq!((props(&d, table).rows.len(), cell_count(&d, table)), (5, 15));
    assert_one_undo_step(&mut d, &mut h, table, |d| insert_cols(d, table, 3, 1));
    assert_eq!((props(&d, table).cols.len(), cell_count(&d, table)), (4, 20));
    // the new row sits where it was asked for and has the default height
    assert_eq!(props(&d, table).rows[1].size, TABLE_DEFAULT_ROW_HEIGHT);
    assert_eq!(text_of(&d, cell(&d, table, 3, 0)), "r1c0");
    assert_eq!(text_of(&d, cell(&d, table, 1, 0)), "");
}

#[test]
fn delete_rows_and_cols_are_one_undo_step() {
    let (mut d, _, table) = doc_with_table(4, 4);
    let mut h = History::default();
    let (rows, cols) = track_ids(&d, table);
    assert_one_undo_step(&mut d, &mut h, table, |d| delete_rows(d, table, &[rows[1], rows[2]]));
    assert_eq!((props(&d, table).rows.len(), cell_count(&d, table)), (2, 8));
    assert_one_undo_step(&mut d, &mut h, table, |d| delete_cols(d, table, &[cols[0]]));
    assert_eq!((props(&d, table).cols.len(), cell_count(&d, table)), (3, 6));
    assert_eq!(text_of(&d, cell(&d, table, 1, 0)), "r3c1");
}

#[test]
fn move_row_and_col_are_one_undo_step() {
    let (mut d, _, table) = doc_with_table(3, 3);
    let mut h = History::default();
    let (rows, cols) = track_ids(&d, table);
    let moved = cell(&d, table, 0, 0);
    assert_one_undo_step(&mut d, &mut h, table, |d| move_row(d, table, rows[0], 2));
    assert_eq!(props(&d, table).rows.iter().map(|t| t.id).collect::<Vec<_>>(), vec![rows[1], rows[2], rows[0]]);
    assert_eq!(text_of(&d, cell(&d, table, 2, 0)), "r0c0");
    assert_eq!(cell(&d, table, 2, 0), moved, "a move keeps the cell nodes");
    assert_eq!(geometry(&d, moved).1, 2.0 * TABLE_DEFAULT_ROW_HEIGHT);
    assert_one_undo_step(&mut d, &mut h, table, |d| move_col(d, table, cols[2], 0));
    assert_eq!(text_of(&d, cell(&d, table, 0, 0)), "r1c2");
}

#[test]
fn resize_ops_are_one_undo_step_and_clamp() {
    let (mut d, _, table) = doc_with_table(3, 3);
    let mut h = History::default();
    let (rows, cols) = track_ids(&d, table);
    assert_one_undo_step(&mut d, &mut h, table, |d| resize_row(d, table, rows[1], 100.0));
    assert_one_undo_step(&mut d, &mut h, table, |d| resize_col(d, table, cols[2], 10.0));
    assert_eq!(props(&d, table).cols[2].size, TABLE_MIN_TRACK, "clamped to the minimum");
    assert_eq!(geometry(&d, table).2, 2.0 * TABLE_DEFAULT_COLUMN_WIDTH + TABLE_MIN_TRACK);
    assert_eq!(geometry(&d, cell(&d, table, 2, 1)).1, TABLE_DEFAULT_ROW_HEIGHT + 100.0);
    assert!(matches!(resize_row(&d, table, rows[1], 100.0), Err(TableOpError::NoChange(_))));
    assert_one_undo_step(&mut d, &mut h, table, |d| resize_table(d, table, Vec2::new(960.0, 440.0)));
    let p = props(&d, table);
    assert!((p.cols.iter().map(|t| t.size).sum::<f32>() - 960.0).abs() < 0.01);
    assert!((p.rows.iter().map(|t| t.size).sum::<f32>() - 440.0).abs() < 0.01);
    assert!(resize_table(&d, table, Vec2::new(f32::NAN, 5.0)).is_err());
}

#[test]
fn merge_unmerge_and_set_cell_text_are_one_undo_step() {
    let (mut d, _, table) = doc_with_table(3, 3);
    let mut h = History::default();
    assert_one_undo_step(&mut d, &mut h, table, |d| merge(d, table, (0, 0, 1, 1)));
    let anchor = cell(&d, table, 0, 0);
    assert_one_undo_step(&mut d, &mut h, table, |d| set_cell_text(d, table, 1, 1, "Xin chào"));
    assert_eq!(text_of(&d, anchor), "Xin chào", "a covered position edits the anchor");
    assert_one_undo_step(&mut d, &mut h, table, |d| unmerge(d, table, anchor));
}

// ---------------------------------------------------------------- specific cases (AC-2 .. AC-6)

#[test]
fn undo_of_a_row_delete_restores_texts_ids_order_and_sizes() {
    let (mut d, _, table) = doc_with_table(3, 3);
    let (rows, _) = track_ids(&d, table);
    let mut h = History::default();
    let ids: Vec<NodeId> = d.get(table).unwrap().children.clone();
    let tracks = props(&d, table);
    let texts: Vec<String> = ids.iter().map(|id| text_of(&d, *id)).collect();
    let sizes: Vec<(f32, f32, f32, f32)> = ids.iter().map(|id| geometry(&d, *id)).collect();

    run(&mut d, &mut h, |d| delete_rows(d, table, &[rows[1]]));
    assert_eq!(cell_count(&d, table), 6);
    assert!(h.undo(&mut d).unwrap());

    assert_eq!(d.get(table).unwrap().children, ids, "same NodeIds in the same order");
    assert_eq!(props(&d, table), tracks, "track ids, order and sizes");
    assert_eq!(ids.iter().map(|id| text_of(&d, *id)).collect::<Vec<_>>(), texts);
    assert_eq!(ids.iter().map(|id| geometry(&d, *id)).collect::<Vec<_>>(), sizes);
}

#[test]
fn merge_then_unmerge_on_a_3x3() {
    let (mut d, _, table) = doc_with_table(3, 3);
    run_once(&mut d, |d| merge(d, table, (0, 0, 1, 1)));
    assert_eq!(cell_count(&d, table), 6);
    let anchor = cell(&d, table, 0, 0);
    assert_eq!(text_of(&d, anchor), "r0c0\nr0c1\nr1c0\nr1c1", "covered text is appended, one line per cell");
    assert_eq!(geometry(&d, anchor), (0.0, 0.0, 2.0 * TABLE_DEFAULT_COLUMN_WIDTH, 2.0 * TABLE_DEFAULT_ROW_HEIGHT));
    assert_eq!(check_table_positions(&d, table), Ok(()));

    run_once(&mut d, |d| unmerge(d, table, anchor));
    assert_eq!(cell_count(&d, table), 9);
    assert!(props(&d, table).merges.is_empty());
    assert_eq!(text_of(&d, cell(&d, table, 0, 0)), "r0c0\nr0c1\nr1c0\nr1c1", "the anchor keeps the merged text");
    for pos in [(0, 1), (1, 0), (1, 1)] {
        assert_eq!(text_of(&d, cell(&d, table, pos.0, pos.1)), "", "the three new cells are empty");
    }
    let (w, h) = (TABLE_DEFAULT_COLUMN_WIDTH, TABLE_DEFAULT_ROW_HEIGHT);
    assert_eq!(geometry(&d, cell(&d, table, 1, 1)), (w, h, w, h));
}

#[test]
fn merge_skips_empty_covered_cells_and_does_not_start_with_a_blank_line() {
    let (mut d, _, table) = doc_with_table(2, 3);
    for c in [0, 1] {
        let id = cell(&d, table, 0, c);
        set_text(&mut d, id, "");
    }
    run_once(&mut d, |d| merge(d, table, (0, 0, 0, 2)));
    assert_eq!(text_of(&d, cell(&d, table, 0, 0)), "r0c2");
}

#[test]
fn move_row_across_a_merge_is_refused_and_insert_inside_one_grows_it() {
    let (mut d, _, table) = doc_with_table(3, 3);
    run_once(&mut d, |d| merge(d, table, (0, 0, 1, 1)));
    let (rows, cols) = track_ids(&d, table);
    let before = d.nodes.clone();
    for (id, to) in [(rows[0], 2), (rows[1], 0), (rows[2], 1)] {
        let err = move_row(&d, table, id, to).unwrap_err();
        assert!(matches!(&err, TableOpError::SplitsMerge(m) if m.contains("Unmerge")), "{err:?}");
    }
    assert!(matches!(move_col(&d, table, cols[0], 2), Err(TableOpError::SplitsMerge(_))));
    assert!(d.nodes == before, "a refused op leaves the document untouched");
    assert!(move_row(&d, table, rows[2], 0).is_ok(), "a free row may move to the other side of the block");

    run_once(&mut d, |d| insert_rows(d, table, 1, 2));
    let p = props(&d, table);
    assert_eq!(p.merges.len(), 1);
    assert_eq!((p.merges[0].row_span, p.merges[0].col_span), (4, 2), "row_span grows by the count");
    assert_eq!(check_table_positions(&d, table), Ok(()));
    // the block (1 anchor), column 2 over 5 rows, and the last old row's 2 cells
    assert_eq!(cell_count(&d, table), 1 + 5 + 2);
}

#[test]
fn insert_at_the_edge_of_a_merge_shifts_it_instead_of_growing_it() {
    let (mut d, _, table) = doc_with_table(3, 3);
    run_once(&mut d, |d| merge(d, table, (0, 0, 1, 1)));
    run_once(&mut d, |d| insert_rows(d, table, 0, 1));
    let m = props(&d, table).merges[0].clone();
    assert_eq!((m.row_span, m.col_span), (2, 2));
    assert_eq!(check_table_positions(&d, table), Ok(()));
    run_once(&mut d, |d| insert_rows(d, table, 3, 1));
    assert_eq!(props(&d, table).merges[0].row_span, 2, "a row after the block does not grow it");
}

#[test]
fn deleting_a_merge_anchor_row_moves_the_anchor_cell_to_the_next_row_of_the_block() {
    let (mut d, _, table) = doc_with_table(4, 2);
    run_once(&mut d, |d| merge(d, table, (0, 0, 2, 0)));
    let anchor = cell(&d, table, 0, 0);
    let (rows, _) = track_ids(&d, table);
    let mut h = History::default();
    assert_one_undo_step(&mut d, &mut h, table, |d| delete_rows(d, table, &[rows[0]]));
    assert_eq!(cell(&d, table, 0, 0), anchor, "the anchor keeps its NodeId");
    assert!(text_of(&d, anchor).contains("r0c0"), "and its text");
    let m = props(&d, table).merges[0].clone();
    assert_eq!((m.row, m.row_span), (rows[1], 2));
    // down to one row of the block: the merge is dropped, the cell stays
    let (rows, _) = track_ids(&d, table);
    run(&mut d, &mut h, |d| delete_rows(d, table, &[rows[1]]));
    assert!(props(&d, table).merges.is_empty());
    assert_eq!(check_table_positions(&d, table), Ok(()));
    assert_eq!(cell_count(&d, table), 2 * 2);
}

#[test]
fn deleting_columns_of_a_block_shrinks_it_and_deleting_every_row_removes_the_table() {
    let (mut d, page, table) = doc_with_table(2, 3);
    run_once(&mut d, |d| merge(d, table, (0, 0, 1, 1)));
    let (rows, cols) = track_ids(&d, table);
    let mut h = History::default();
    assert_one_undo_step(&mut d, &mut h, table, |d| delete_cols(d, table, &[cols[0]]));
    let m = props(&d, table).merges[0].clone();
    assert_eq!((m.col, m.row_span, m.col_span), (cols[1], 2, 1));
    assert_one_undo_step(&mut d, &mut h, table, |d| delete_rows(d, table, &rows));
    assert!(d.get(table).is_none());
    assert!(!d.get(page).unwrap().children.contains(&table));
}

#[test]
fn merge_absorbs_a_merge_it_contains_and_refuses_one_it_cuts() {
    let (mut d, _, table) = doc_with_table(3, 3);
    run_once(&mut d, |d| merge(d, table, (0, 0, 0, 1)));
    let before = d.nodes.clone();
    let err = merge(&d, table, (0, 1, 1, 2)).unwrap_err();
    assert!(matches!(&err, TableOpError::OverlapsMerge(m) if m.contains("row 1") && m.contains("Unmerge")), "{err:?}");
    assert!(matches!(merge(&d, table, (0, 0, 0, 1)), Err(TableOpError::NoChange(_))), "the same block again");
    assert!(d.nodes == before);

    run_once(&mut d, |d| merge(d, table, (0, 0, 1, 1)));
    assert_eq!(props(&d, table).merges.len(), 1, "the inner merge was absorbed");
    assert_eq!(text_of(&d, cell(&d, table, 0, 0)), "r0c0\nr0c1\nr1c0\nr1c1");
    assert_eq!(check_table_positions(&d, table), Ok(()));
}

#[test]
fn new_cells_inherit_the_neighbour_fill() {
    let (mut d, _, table) = doc_with_table(2, 2);
    for r in 0..2 {
        let id = cell(&d, table, r, 1);
        d.get_mut(id).unwrap().fills = paint();
    }
    run_once(&mut d, |d| insert_cols(d, table, 2, 1));
    let id = cell(&d, table, 1, 2);
    assert_eq!(d.get(id).unwrap().fills, paint(), "the column to the left decides");
    run_once(&mut d, |d| insert_cols(d, table, 0, 1));
    let id = cell(&d, table, 0, 0);
    assert!(d.get(id).unwrap().fills.is_empty(), "the first column copies the one that follows it");
    // unmerge keeps the block colour on the new cells
    run_once(&mut d, |d| merge(d, table, (0, 2, 1, 3)));
    let anchor = cell(&d, table, 0, 2);
    d.get_mut(anchor).unwrap().fills = paint();
    run_once(&mut d, |d| unmerge(d, table, anchor));
    let id = cell(&d, table, 1, 3);
    assert_eq!(d.get(id).unwrap().fills, paint());
}

#[test]
fn create_table_50_by_50_is_one_step_of_2501_nodes() {
    let (mut d, page, _) = doc_with_table(1, 1);
    let mut h = History::default();
    let before = d.nodes.len();
    run(&mut d, &mut h, |d| create_table(d, page, Vec2::new(0.0, 0.0), 50, 50, &TableStyle::default()));
    assert_eq!(d.nodes.len(), before + 2501);
    assert!(h.undo(&mut d).unwrap());
    assert_eq!(d.nodes.len(), before, "undo removes the table and all 2,500 cells");
    assert!(!h.can_undo());
}

/// Run: `cargo test -p designos-doc --release -- --ignored table_ops_perf --nocapture`.
#[test]
#[ignore = "timing test, run with --release"]
fn table_ops_perf_create_50x50_applies_in_under_150_ms() {
    let (mut d, page, _) = doc_with_table(1, 1);
    let mut h = History::default();
    let t = std::time::Instant::now();
    let cmd = create_table(&d, page, Vec2::new(0.0, 0.0), 50, 50, &TableStyle::default()).unwrap();
    let built = t.elapsed();
    let t = std::time::Instant::now();
    h.exec_user(&mut d, cmd).unwrap();
    let applied = t.elapsed();
    println!("table_ops_perf: create_table 50x50 (2,500 cells): build {built:?}, apply {applied:?} (budget 150 ms)");
    assert!(applied < std::time::Duration::from_millis(150), "apply took {applied:?}");
    let t = std::time::Instant::now();
    assert!(h.undo(&mut d).unwrap());
    println!("table_ops_perf: undo of the 2,501 nodes {:?}", t.elapsed());
}

#[test]
fn next_cell_walks_the_anchors_in_row_major_order() {
    let (mut d, _, table) = doc_with_table(3, 3);
    run_once(&mut d, |d| merge(d, table, (0, 0, 1, 1)));
    let order = [(0, 0), (0, 2), (1, 2), (2, 0), (2, 1), (2, 2)].map(|(r, c)| cell(&d, table, r, c));
    for w in order.windows(2) {
        assert_eq!(next_cell(&d, table, w[0], false), Some(w[1]));
        assert_eq!(next_cell(&d, table, w[1], true), Some(w[0]));
    }
    assert_eq!(next_cell(&d, table, order[5], false), None, "None after the last cell");
    assert_eq!(next_cell(&d, table, order[0], true), None, "None before the first");
    assert_eq!(next_cell(&d, table, table, false), None, "not a cell of the table");
}

#[test]
fn errors_name_the_next_action() {
    let (d, page, table) = doc_with_table(2, 2);
    let (rows, _) = track_ids(&d, table);
    let style = TableStyle::default();
    assert!(matches!(insert_rows(&d, page, 0, 1), Err(TableOpError::NotATable(_))));
    assert!(matches!(insert_rows(&d, table, 3, 1), Err(TableOpError::IndexOutOfRange(m)) if m.contains("1 to 3")));
    assert!(matches!(insert_rows(&d, table, 0, TABLE_MAX_TRACKS), Err(TableOpError::TooLarge(m)) if m.contains("Add fewer")));
    assert!(matches!(create_table(&d, page, Vec2::new(0.0, 0.0), 501, 1, &style), Err(TableOpError::TooLarge(_))));
    assert!(matches!(create_table(&d, page, Vec2::new(0.0, 0.0), 1, 501, &style), Err(TableOpError::TooLarge(_))));
    assert!(create_table(&d, page, Vec2::new(0.0, 0.0), 500, 500, &style).is_ok(), "500 x 500 is the limit, not over it");
    assert!(matches!(create_table(&d, page, Vec2::new(0.0, 0.0), 0, 1, &style), Err(TableOpError::IndexOutOfRange(_))));
    assert!(matches!(create_table(&d, table, Vec2::new(0.0, 0.0), 1, 1, &style), Err(TableOpError::BadParent(_))));
    assert!(matches!(delete_rows(&d, table, &[TrackId { session: 9, local: 9 }]), Err(TableOpError::IndexOutOfRange(_))));
    assert!(matches!(move_row(&d, table, rows[0], 5), Err(TableOpError::IndexOutOfRange(_))));
    assert!(matches!(merge(&d, table, (0, 0, 5, 5)), Err(TableOpError::IndexOutOfRange(_))));
    assert!(matches!(set_cell_text(&d, table, 0, 0, "r0c0"), Err(TableOpError::NoChange(_))));
    let loose = cell(&d, table, 0, 0);
    assert!(matches!(unmerge(&d, table, loose), Err(TableOpError::NoChange(m)) if m.contains("not merged")));
    assert!(matches!(unmerge(&d, table, page), Err(TableOpError::NotATable(_))));
    let mut broken = d.clone();
    broken.remove(loose).unwrap();
    assert!(matches!(insert_rows(&broken, table, 0, 1), Err(TableOpError::BrokenTable(_))));
}

// ---------------------------------------------------------------- guards at the commit points (R-T1-2, R-T1-9)

#[test]
fn a_command_with_a_missing_cell_is_refused_by_history_and_the_document_stays() {
    let (mut d, _, table) = doc_with_table(2, 2);
    let mut h = History::default();
    let Command::Batch(mut steps) = insert_rows(&d, table, 1, 1).unwrap() else { panic!("a batch") };
    let dropped = steps.pop().expect("the last step inserts the last new cell");
    assert!(matches!(dropped, Command::Insert { .. }));
    let before = d.nodes.clone();
    let err = h.exec_user(&mut d, Command::Batch(steps)).expect_err("the table would lack a cell");
    assert!(matches!(err, DocError::Table(_)), "{err:?}");
    assert!(d.nodes == before && !h.can_undo(), "the step was rolled back whole");
}

#[test]
fn apply_checked_keeps_the_table_rule_outside_history() {
    let (mut d, _, table) = doc_with_table(2, 2);
    let before = d.nodes.clone();

    // a whole op passes and returns an inverse that restores the document
    let cmd = insert_rows(&d, table, 1, 1).unwrap();
    let inverse = apply_checked(&mut d, cmd).unwrap();
    assert_eq!(check_table_positions(&d, table), Ok(()));
    assert_eq!(cell_count(&d, table), 6);
    inverse.apply(&mut d).unwrap();
    assert!(d.nodes == before);

    // a half op (the last cell insert missing) is undone, not committed
    let Command::Batch(mut steps) = insert_rows(&d, table, 1, 1).unwrap() else { panic!("a batch") };
    steps.pop();
    let err = apply_checked(&mut d, Command::Batch(steps)).expect_err("the position check refuses it");
    assert!(matches!(err, DocError::Table(m) if m.contains("table")));
    assert!(d.nodes == before, "rolled back to the exact start");

    // a batch whose later command is refused applies nothing, even though its earlier ones were valid
    let rect = d.alloc_id();
    let Command::Batch(mut steps) = insert_rows(&d, table, 1, 1).unwrap() else { panic!("a batch") };
    steps.push(Command::Insert { parent: table, index: 0, nodes: vec![Node::rectangle(rect, Rect::new(0.0, 0.0, 5.0, 5.0))] });
    assert!(apply_checked(&mut d, Command::Batch(steps)).is_err());
    assert!(d.nodes == before);
}

fn auto_layout_frame_with_table_and_rect() -> (Document, NodeId, NodeId, NodeId) {
    let mut d = Document::new("Layout");
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
    solve(&mut d, frame);
    (d, frame, table, rect)
}

fn solve(d: &mut Document, frame: NodeId) {
    d.relayout_subtree(frame);
    d.flush_pending_relayout();
}

fn layout_of(d: &Document, ids: &[NodeId]) -> Vec<(f32, f32, f32, f32)> {
    ids.iter().map(|id| geometry(d, *id)).collect()
}

#[test]
fn a_refused_undo_through_the_auto_layout_path_also_undoes_the_layout_pass() {
    let (mut d, frame, table, rect) = auto_layout_frame_with_table_and_rect();
    let mut h = History::default();
    run(&mut d, &mut h, |d| insert_rows(d, table, 2, 1));
    solve(&mut d, frame);
    // behind History's back a cell goes missing, so undoing the op would leave the table broken
    let victim = cell(&d, table, 0, 0);
    d.remove(victim).unwrap();
    solve(&mut d, frame);
    let ids = [frame, table, rect];
    let (before, depth, nodes) = (layout_of(&d, &ids), h.undo_depth(), d.nodes.clone());

    let err = h.undo(&mut d).expect_err("a table without one of its cells is refused");
    assert!(matches!(err, DocError::Table(_)), "{err:?}");
    assert_eq!(layout_of(&d, &ids), before, "the layout pass was undone with the step");
    assert!(d.nodes == nodes, "nothing else moved");
    assert_eq!((h.undo_depth(), h.can_redo()), (depth, false));
}

#[test]
fn a_refused_redo_through_the_auto_layout_path_also_undoes_the_layout_pass() {
    let (mut d, frame, table, rect) = auto_layout_frame_with_table_and_rect();
    let mut h = History::default();
    run(&mut d, &mut h, |d| insert_rows(d, table, 2, 1));
    assert!(h.undo(&mut d).unwrap());
    solve(&mut d, frame);
    let victim = cell(&d, table, 0, 0);
    d.remove(victim).unwrap();
    solve(&mut d, frame);
    let ids = [frame, table, rect];
    let (before, nodes) = (layout_of(&d, &ids), d.nodes.clone());

    let err = h.redo(&mut d).expect_err("the redo leaves the table without a cell");
    assert!(matches!(err, DocError::Table(_)), "{err:?}");
    assert_eq!(layout_of(&d, &ids), before);
    assert!(d.nodes == nodes);
    assert!(h.can_redo() && !h.can_undo());
}

// ---------------------------------------------------------------- random ops (risk: merge rules)

/// xorshift64*, so the run needs no dependency and replays from the seed.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
}

#[derive(Default, Debug)]
struct FuzzStats {
    applied: BTreeMap<&'static str, usize>,
    refused: BTreeMap<&'static str, usize>,
}

fn random_op(rng: &mut Rng, d: &Document, table: NodeId) -> (&'static str, OpResult) {
    let p = props(d, table);
    let (rows, cols) = (p.rows.len(), p.cols.len());
    let row = p.rows[rng.below(rows)].id;
    let col = p.cols[rng.below(cols)].id;
    match rng.below(18) {
        0 if rows < 9 => ("insert_rows", insert_rows(d, table, rng.below(rows + 1), 1 + rng.below(2))),
        1 if cols < 9 => ("insert_cols", insert_cols(d, table, rng.below(cols + 1), 1 + rng.below(2))),
        0 | 2 => ("delete_rows", delete_rows(d, table, &[row])),
        1 | 3 => ("delete_cols", delete_cols(d, table, &[col])),
        4 => ("move_row", move_row(d, table, row, rng.below(rows))),
        5 => ("move_col", move_col(d, table, col, rng.below(cols))),
        6 => ("resize_row", resize_row(d, table, row, 10.0 + rng.below(120) as f32)),
        7 => ("resize_col", resize_col(d, table, col, 10.0 + rng.below(300) as f32)),
        8 => ("resize_table", resize_table(d, table, Vec2::new(200.0 + rng.below(800) as f32, 100.0 + rng.below(500) as f32))),
        9..=12 => {
            let (r0, c0) = (rng.below(rows), rng.below(cols));
            let (r1, c1) = ((r0 + rng.below(3)).min(rows - 1), (c0 + rng.below(3)).min(cols - 1));
            ("merge", merge(d, table, (r0, c0, r1, c1)))
        }
        13 | 14 => {
            let anchors: Vec<NodeId> = p.merges.iter().filter_map(|m| cell_by_tracks(d, table, m.row, m.col)).collect();
            let target = if anchors.is_empty() { d.get(table).unwrap().children[0] } else { anchors[rng.below(anchors.len())] };
            ("unmerge", unmerge(d, table, target))
        }
        _ => ("set_cell_text", set_cell_text(d, table, rng.below(rows), rng.below(cols), &format!("t{}\nline", rng.below(1000)))),
    }
}

#[test]
fn a_thousand_random_ops_on_a_6x6_table_keep_every_invariant() {
    let (mut d, page, mut table) = doc_with_table(6, 6);
    assert!(d.clone().validate_and_repair().is_clean(), "the start is a well-formed document");
    let mut h = History::default();
    let mut rng = Rng(0x5EED_7AB1_E500_0001);
    let mut stats = FuzzStats::default();
    for i in 0..1000 {
        if d.get(table).is_none() {
            run(&mut d, &mut h, |d| create_table(d, page, Vec2::new(0.0, 0.0), 6, 6, &TableStyle::default()));
            table = *d.get(page).unwrap().children.last().unwrap();
        }
        let (name, result) = random_op(&mut rng, &d, table);
        let cmd = match result {
            Ok(cmd) => cmd,
            Err(e) => {
                let allowed = matches!(e, TableOpError::SplitsMerge(_) | TableOpError::OverlapsMerge(_) | TableOpError::NoChange(_));
                assert!(allowed, "op {i} {name}: an unexpected refusal: {e:?}");
                *stats.refused.entry(name).or_default() += 1;
                continue;
            }
        };
        let (before, depth) = (d.nodes.clone(), h.undo_depth());
        h.exec_user(&mut d, cmd).unwrap_or_else(|e| panic!("op {i} {name}: the op's own command was refused: {e}"));
        assert_eq!(h.undo_depth(), depth + 1, "op {i} {name}: one undo step");
        *stats.applied.entry(name).or_default() += 1;
        let after = d.nodes.clone();
        if d.get(table).is_some() {
            assert_eq!(check_table_positions(&d, table), Ok(()), "op {i} {name}");
            assert!(relayout(&d, table).is_empty(), "op {i} {name}: the step carries its own layout");
            let report = d.clone().validate_and_repair();
            assert!(report.is_clean(), "op {i} {name}: load repair would fix {report:?}");
        }
        assert!(h.undo(&mut d).unwrap() && d.nodes == before, "op {i} {name}: undo restores the document");
        assert!(h.redo(&mut d).unwrap() && d.nodes == after, "op {i} {name}: redo restores the edit");
    }
    let every_op = [
        "insert_rows",
        "insert_cols",
        "delete_rows",
        "delete_cols",
        "move_row",
        "move_col",
        "resize_row",
        "resize_col",
        "resize_table",
        "merge",
        "unmerge",
        "set_cell_text",
    ];
    for name in every_op {
        assert!(stats.applied.get(name).copied().unwrap_or(0) > 0, "the run never applied {name}: {stats:?}");
    }
    assert!(stats.refused.contains_key("move_row") || stats.refused.contains_key("move_col"), "a merge was never in the way of a move: {stats:?}");
    assert!(stats.refused.contains_key("merge"), "a merge was never refused: {stats:?}");
    println!("random ops: applied {:?}, refused {:?}", stats.applied, stats.refused);
}

// ---------------------------------------------------------------- caller-supplied counts and sizes

#[test]
fn a_huge_insert_count_is_too_large_not_an_overflow() {
    let (d, _, table) = doc_with_table(2, 2);
    for count in [usize::MAX, usize::MAX - 1, TABLE_MAX_TRACKS] {
        assert!(matches!(insert_rows(&d, table, 0, count), Err(TableOpError::TooLarge(_))), "rows, count {count}");
        assert!(matches!(insert_cols(&d, table, 2, count), Err(TableOpError::TooLarge(_))), "cols, count {count}");
    }
    assert!(insert_rows(&d, table, 0, TABLE_MAX_TRACKS - 2).is_ok(), "exactly the limit is allowed");
    assert!(matches!(insert_rows(&d, table, usize::MAX, 1), Err(TableOpError::IndexOutOfRange(_))));
    assert!(matches!(merge(&d, table, (0, 0, usize::MAX, usize::MAX)), Err(TableOpError::IndexOutOfRange(_))));
    assert!(matches!(set_cell_text(&d, table, usize::MAX, 0, "x"), Err(TableOpError::IndexOutOfRange(_))));
}

#[test]
fn track_and_table_sizes_are_capped_at_the_schema_limit() {
    let (mut d, _, table) = doc_with_table(2, 2);
    let (rows, _) = track_ids(&d, table);
    run_once(&mut d, |d| resize_row(d, table, rows[0], f32::MAX));
    // the second request lands on the same clamped size: nothing to do, and nothing overflows
    assert!(matches!(resize_row(&d, table, rows[0], f32::MAX), Err(TableOpError::NoChange(_))));
    let g = geometry(&d, table);
    assert!(g.2.is_finite() && g.3.is_finite(), "table geometry stays finite: {g:?}");
    assert_eq!(props(&d, table).rows[0].size, TABLE_MAX_SIZE);
    assert_eq!(check_table_positions(&d, table), Ok(()));
    run_once(&mut d, |d| resize_row(d, table, rows[1], f32::INFINITY));
    assert_eq!(props(&d, table).rows[1].size, TABLE_MAX_SIZE);
    // resize_table clamps a finite request to the limit; non-finite or non-positive input is refused
    run_once(&mut d, |d| resize_table(d, table, Vec2::new(3e38, 3e38)));
    let p = props(&d, table);
    assert!(p.cols.iter().chain(&p.rows).all(|t| t.size.is_finite() && t.size <= TABLE_MAX_SIZE && t.size >= TABLE_MIN_TRACK));
    let g = geometry(&d, table);
    assert!(g.2.is_finite() && g.3.is_finite());
    assert!((p.cols.iter().map(|t| t.size).sum::<f32>() - TABLE_MAX_SIZE).abs() < 1.0, "the width was clamped to the limit");
    for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, 0.0, -5.0] {
        assert!(resize_table(&d, table, Vec2::new(bad, 100.0)).is_err(), "{bad}");
        assert!(resize_table(&d, table, Vec2::new(100.0, bad)).is_err(), "{bad}");
    }
}
