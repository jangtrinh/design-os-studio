//! Table model tests: layout, merges, serde, load repair, hit lookup and the tree guards.

use super::*;
use crate::{Command, DocError, Document, History, Node, NodeId, NodeKind, Rect, Vec2};

/// A page holding a `rows` x `cols` table of default tracks. Returns the document, the page and the table.
fn doc_with_table(rows: usize, cols: usize) -> (Document, NodeId, NodeId) {
    let mut d = Document::new("Tables");
    let page = d.add_page("P");
    let nodes = build_table(&mut d, rows, cols, Vec2::new(10.0, 20.0));
    let table = nodes[0].id;
    Command::Insert { parent: page, index: 0, nodes }.apply(&mut d).unwrap();
    (d, page, table)
}

fn set_tracks(d: &mut Document, table: NodeId, rows: &[f32], cols: &[f32]) {
    let NodeKind::Table(p) = &mut d.get_mut(table).unwrap().kind else { panic!("not a table") };
    for (t, s) in p.rows.iter_mut().zip(rows) {
        t.size = *s;
    }
    for (t, s) in p.cols.iter_mut().zip(cols) {
        t.size = *s;
    }
}

fn props(d: &Document, table: NodeId) -> TableProps {
    match &d.get(table).unwrap().kind {
        NodeKind::Table(p) => p.clone(),
        _ => panic!("not a table"),
    }
}

/// The cell node at zero-based `(row, col)`.
fn cell(d: &Document, table: NodeId, row: usize, col: usize) -> Option<NodeId> {
    let p = props(d, table);
    let (r, c) = (p.rows[row].id, p.cols[col].id);
    d.get(table).unwrap().children.iter().copied().find(|id| matches!(&d.get(*id).unwrap().kind, NodeKind::TableCell(x) if x.row == r && x.col == c))
}

fn set_text(d: &mut Document, id: NodeId, text: &str) {
    if let NodeKind::TableCell(c) = &mut d.get_mut(id).unwrap().kind {
        c.text.content = text.to_string();
    }
}

fn set_text_at(d: &mut Document, table: NodeId, row: usize, col: usize, text: &str) {
    let id = cell(d, table, row, col).unwrap();
    set_text(d, id, text);
}

fn text_of(d: &Document, id: NodeId) -> String {
    d.get(id).unwrap().kind.board_text().unwrap().content.clone()
}

fn frame_of(d: &Document, id: NodeId) -> (f32, f32, f32, f32) {
    let n = d.get(id).unwrap();
    (n.transform.tx, n.transform.ty, n.size.x, n.size.y)
}

/// Makes a 2x2 block at the top-left of a 3x3 table: records the merge and removes the covered cells.
fn merge_top_left_2x2(d: &mut Document, table: NodeId) {
    let p = props(d, table);
    let covered: Vec<NodeId> = [(0, 1), (1, 0), (1, 1)].iter().map(|&(r, c)| cell(d, table, r, c).unwrap()).collect();
    for id in covered {
        d.remove(id).unwrap();
    }
    let NodeKind::Table(t) = &mut d.get_mut(table).unwrap().kind else { unreachable!() };
    t.merges.push(TableMerge { row: p.rows[0].id, col: p.cols[0].id, row_span: 2, col_span: 2 });
}

fn sized_table() -> (Document, NodeId, NodeId) {
    let (mut d, page, table) = doc_with_table(3, 3);
    set_tracks(&mut d, table, &[40.0, 40.0, 64.0], &[120.0, 240.0, 90.0]);
    apply_relayout(&mut d, table);
    (d, page, table)
}

#[test]
fn relayout_places_cells_by_track_sizes() {
    let (d, _, table) = sized_table();
    assert_eq!(frame_of(&d, cell(&d, table, 1, 1).unwrap()), (120.0, 40.0, 240.0, 40.0));
    assert_eq!(frame_of(&d, cell(&d, table, 2, 2).unwrap()), (360.0, 80.0, 90.0, 64.0));
    assert_eq!(d.get(table).unwrap().size, Vec2::new(450.0, 144.0));
    assert!(relayout(&d, table).is_empty(), "a laid out table has nothing left to change");
}

#[test]
fn relayout_merge_anchor_spans_its_tracks() {
    let (mut d, _, table) = sized_table();
    merge_top_left_2x2(&mut d, table);
    apply_relayout(&mut d, table);
    let anchor = cell(&d, table, 0, 0).unwrap();
    assert_eq!(frame_of(&d, anchor), (0.0, 0.0, 360.0, 80.0));
    for (r, c) in [(0, 1), (1, 0), (1, 1)] {
        assert!(cell(&d, table, r, c).is_none(), "covered cell ({r}, {c}) does not exist");
    }
    assert_eq!(d.get(table).unwrap().children.len(), 9 - 3);
    assert_eq!(check_table_positions(&d, table), Ok(()));
}

#[test]
fn relayout_keeps_stored_track_sizes_without_a_measurer() {
    let (mut d, _, table) = sized_table();
    set_text_at(&mut d, table, 0, 0, "a very long line of text that would wrap in a narrow cell and need more than forty points");
    assert!(relayout(&d, table).is_empty(), "no measurer installed: stored sizes decide");
    assert_eq!(props(&d, table).rows[0].size, 40.0);
}

#[test]
fn cell_at_finds_the_anchor_for_every_point_in_a_merged_block() {
    let (mut d, _, table) = sized_table();
    merge_top_left_2x2(&mut d, table);
    apply_relayout(&mut d, table);
    let anchor = cell(&d, table, 0, 0).unwrap();
    for p in [(0.0, 0.0), (10.0, 10.0), (359.9, 0.0), (0.0, 79.9), (359.9, 79.9), (200.0, 60.0)] {
        assert_eq!(cell_at(&d, table, Vec2::new(p.0, p.1)), Some(anchor), "point {p:?}");
    }
    assert_eq!(cell_at(&d, table, Vec2::new(360.0, 0.0)), cell(&d, table, 0, 2));
    assert_eq!(cell_at(&d, table, Vec2::new(449.9, 143.9)), cell(&d, table, 2, 2));
    for p in [(-0.1, 10.0), (10.0, -0.1), (450.0, 10.0), (10.0, 144.0), (900.0, 900.0)] {
        assert_eq!(cell_at(&d, table, Vec2::new(p.0, p.1)), None, "point {p:?} is outside");
    }
    let page = d.pages[0];
    assert_eq!(cell_at(&d, page, Vec2::new(1.0, 1.0)), None, "only a table has cells");
}

#[test]
fn kind_helpers_describe_table_and_cell() {
    let (d, _, table) = doc_with_table(1, 1);
    let cell_kind = &d.get(cell(&d, table, 0, 0).unwrap()).unwrap().kind;
    let table_kind = &d.get(table).unwrap().kind;
    assert_eq!(cell_kind.board_text().map(|t| t.content.as_str()), Some(""));
    assert!(cell_kind.any_text().is_some());
    assert!(cell_kind.is_board_item() && table_kind.is_board_item());
    assert!(table_kind.can_hold_children());
    assert!(!table_kind.is_container());
    assert_eq!((table_kind.default_name(), cell_kind.default_name()), ("Table", "Cell"));
    let cell_text = d.get(cell(&d, table, 0, 0).unwrap()).unwrap().kind.board_text().unwrap();
    assert_eq!((cell_text.style.font_family.as_str(), cell_text.style.font_weight, cell_text.style.font_size), ("Inter", 500, 16.0));
}

fn round_trip(d: &Document) -> Document {
    Document::from_json(&d.to_json().unwrap()).unwrap()
}

#[test]
fn serde_round_trip_keeps_table_merge_text_and_fill() {
    let (mut d, _, table) = sized_table();
    merge_top_left_2x2(&mut d, table);
    set_text_at(&mut d, table, 0, 0, "Bảng giá tháng 11");
    set_text_at(&mut d, table, 2, 1, "Đơn giá: 1.250.000 ₫");
    d.get_mut(cell(&d, table, 1, 2).unwrap()).unwrap().fills = vec![crate::Paint::solid(crate::Color::rgb(0xFF, 0xE5, 0x99))];
    apply_relayout(&mut d, table);
    let back = round_trip(&d);
    assert_eq!(back.nodes, d.nodes);
    assert_eq!(back.pages, d.pages);
    assert_eq!(props(&back, table).merges.len(), 1);
    assert_eq!(text_of(&back, cell(&back, table, 0, 0).unwrap()), "Bảng giá tháng 11");
}

#[test]
fn table_without_merges_does_not_write_the_merges_key() {
    let (d, _, _) = doc_with_table(2, 2);
    let json = d.to_json().unwrap();
    assert!(json.contains("\"Table\"") && json.contains("\"TableCell\""));
    assert!(!json.contains("merges"));
}

#[test]
fn document_without_tables_saves_the_same_bytes() {
    let mut d = Document::new("Plain");
    let page = d.add_page("P");
    let f = d.alloc_id();
    d.insert(page, Node::frame(f, Rect::new(0.0, 0.0, 100.0, 80.0))).unwrap();
    let t = d.alloc_id();
    d.insert(f, Node::text(t, Rect::new(4.0, 4.0, 60.0, 20.0), "Xin chào", crate::TextStyle::default())).unwrap();
    let json = d.to_json().unwrap();
    assert!(!json.contains("Table"), "no table tag appears in a table-free file");
    assert_eq!(json, include_str!("table_free_golden.json").trim_end(), "bytes match the file saved by the base commit");
}

// ---------------------------------------------------------------- load repair

fn strip_to_text(d: &mut Document, table: NodeId, row: usize, col: usize) -> NodeId {
    let id = cell(d, table, row, col).unwrap();
    d.remove(id).unwrap();
    id
}

#[test]
fn repair_turns_a_cell_outside_a_table_into_text() {
    let (mut d, page, table) = doc_with_table(2, 2);
    let stray = d.alloc_id();
    let mut n = Node::new(
        stray,
        NodeKind::TableCell(TableCellProps { row: props(&d, table).rows[0].id, col: props(&d, table).cols[0].id, text: default_cell_text("kept") }),
    );
    n.name = "Cell".into();
    d.insert(page, n).unwrap();
    let rep = d.validate_and_repair();
    assert_eq!(rep.tables, crate::TableRepairs { cells_to_text: 1, ..Default::default() });
    assert!(matches!(&d.get(stray).unwrap().kind, NodeKind::Text(t) if t.content == "kept"));
    assert!(d.validate_and_repair().is_clean());
}

#[test]
fn repair_gives_an_empty_table_one_row_and_one_column() {
    let (mut d, _, table) = doc_with_table(2, 2);
    for r in 0..2 {
        for c in 0..2 {
            strip_to_text(&mut d, table, r, c);
        }
    }
    let NodeKind::Table(p) = &mut d.get_mut(table).unwrap().kind else { unreachable!() };
    p.rows.clear();
    p.cols.clear();
    let rep = d.validate_and_repair();
    assert_eq!(rep.tables, crate::TableRepairs { tracks_added: 2, cells_added: 1, ..Default::default() });
    let p = props(&d, table);
    assert_eq!((p.rows.len(), p.cols.len()), (1, 1));
    assert_eq!(p.rows[0].size, TABLE_DEFAULT_ROW_HEIGHT);
    assert_eq!(p.cols[0].size, TABLE_DEFAULT_COLUMN_WIDTH);
    assert_eq!(d.get(table).unwrap().size, Vec2::new(TABLE_DEFAULT_COLUMN_WIDTH, TABLE_DEFAULT_ROW_HEIGHT));
    assert_eq!(check_table_positions(&d, table), Ok(()));
    assert!(d.validate_and_repair().is_clean());
}

#[test]
fn repair_folds_duplicate_cells_into_the_first_in_child_order() {
    let (mut d, _, table) = doc_with_table(2, 2);
    let first = cell(&d, table, 0, 0).unwrap();
    set_text(&mut d, first, "a");
    let p = props(&d, table);
    let dup = d.alloc_id();
    let kind = NodeKind::TableCell(TableCellProps { row: p.rows[0].id, col: p.cols[0].id, text: default_cell_text("dup") });
    d.insert(table, Node::new(dup, kind)).unwrap();
    let rep = d.validate_and_repair();
    assert_eq!(rep.tables, crate::TableRepairs { duplicate_cells_folded: 1, ..Default::default() });
    assert!(d.get(dup).is_none());
    assert_eq!(text_of(&d, first), "a\ndup");
    assert_eq!(d.get(table).unwrap().children.len(), 4);
    assert!(d.validate_and_repair().is_clean());
}

#[test]
fn repair_handles_cells_with_an_unknown_row_or_column() {
    let (mut d, _, table) = doc_with_table(2, 2);
    let missing = strip_to_text(&mut d, table, 1, 1);
    let _ = missing;
    let p = props(&d, table);
    let ghost = TrackId { session: 7, local: 7 };
    let empty = d.alloc_id();
    d.insert(table, Node::new(empty, NodeKind::TableCell(TableCellProps { row: ghost, col: p.cols[0].id, text: default_cell_text("") }))).unwrap();
    let wandering = d.alloc_id();
    d.insert(table, Node::new(wandering, NodeKind::TableCell(TableCellProps { row: p.rows[0].id, col: ghost, text: default_cell_text("lost words") })))
        .unwrap();
    let rep = d.validate_and_repair();
    assert_eq!(rep.tables, crate::TableRepairs { cells_removed_empty: 1, cells_relocated: 1, ..Default::default() });
    assert!(d.get(empty).is_none(), "an empty stray cell is removed");
    assert_eq!(cell(&d, table, 1, 1), Some(wandering), "a cell with text moves to the first free position");
    assert_eq!(text_of(&d, wandering), "lost words");
    assert_eq!(check_table_positions(&d, table), Ok(()));
    assert!(d.validate_and_repair().is_clean());
}

#[test]
fn repair_adds_an_empty_cell_where_a_position_has_none() {
    let (mut d, _, table) = doc_with_table(2, 2);
    strip_to_text(&mut d, table, 0, 1);
    let rep = d.validate_and_repair();
    assert_eq!(rep.tables, crate::TableRepairs { cells_added: 1, ..Default::default() });
    let added = cell(&d, table, 0, 1).unwrap();
    assert_eq!(text_of(&d, added), "");
    assert_eq!(frame_of(&d, added), (TABLE_DEFAULT_COLUMN_WIDTH, 0.0, TABLE_DEFAULT_COLUMN_WIDTH, TABLE_DEFAULT_ROW_HEIGHT));
    assert!(d.validate_and_repair().is_clean());
}

#[test]
fn repair_drops_overlapping_merges_and_folds_covered_cells_into_the_anchor() {
    let (mut d, _, table) = doc_with_table(3, 3);
    for (r, c, t) in [(0, 0, "a"), (0, 1, "b"), (1, 0, "c"), (1, 1, "d")] {
        set_text_at(&mut d, table, r, c, t);
    }
    let p = props(&d, table);
    let NodeKind::Table(tp) = &mut d.get_mut(table).unwrap().kind else { unreachable!() };
    tp.merges = vec![
        TableMerge { row: p.rows[0].id, col: p.cols[0].id, row_span: 2, col_span: 2 },
        TableMerge { row: p.rows[1].id, col: p.cols[1].id, row_span: 2, col_span: 2 },
    ];
    let rep = d.validate_and_repair();
    assert_eq!(rep.tables, crate::TableRepairs { merges_fixed: 1, covered_cells_folded: 3, ..Default::default() });
    assert_eq!(props(&d, table).merges.len(), 1);
    assert_eq!(text_of(&d, cell(&d, table, 0, 0).unwrap()), "a\nb\nc\nd");
    assert_eq!(d.get(table).unwrap().children.len(), 9 - 3);
    assert_eq!(frame_of(&d, cell(&d, table, 0, 0).unwrap()), (0.0, 0.0, 2.0 * TABLE_DEFAULT_COLUMN_WIDTH, 2.0 * TABLE_DEFAULT_ROW_HEIGHT));
    assert!(d.validate_and_repair().is_clean());
}

#[test]
fn repair_clamps_a_merge_that_runs_past_the_edge_and_drops_one_with_an_unknown_anchor() {
    let (mut d, _, table) = doc_with_table(3, 3);
    let p = props(&d, table);
    let NodeKind::Table(tp) = &mut d.get_mut(table).unwrap().kind else { unreachable!() };
    tp.merges = vec![
        TableMerge { row: p.rows[2].id, col: p.cols[2].id, row_span: 2, col_span: 3 },
        TableMerge { row: TrackId { session: 9, local: 9 }, col: p.cols[0].id, row_span: 2, col_span: 2 },
    ];
    let rep = d.validate_and_repair();
    assert_eq!(rep.tables, crate::TableRepairs { merges_fixed: 2, ..Default::default() });
    let merges = props(&d, table).merges;
    assert_eq!(merges.len(), 1);
    assert_eq!((merges[0].row_span, merges[0].col_span), (1, 1));
    assert!(d.validate_and_repair().is_clean());
}

#[test]
fn repair_clamps_track_sizes_that_are_not_positive_and_lays_the_table_out_again() {
    let (mut d, _, table) = doc_with_table(2, 2);
    set_tracks(&mut d, table, &[0.0, 50.0], &[f32::NAN, 70.0]);
    let rep = d.validate_and_repair();
    assert_eq!(rep.tables, crate::TableRepairs { track_sizes_clamped: 2, ..Default::default() });
    let p = props(&d, table);
    assert_eq!((p.rows[0].size, p.cols[0].size), (TABLE_MIN_TRACK, TABLE_MIN_TRACK));
    assert_eq!(frame_of(&d, cell(&d, table, 1, 1).unwrap()), (TABLE_MIN_TRACK, TABLE_MIN_TRACK, 70.0, 50.0));
    assert_eq!(d.get(table).unwrap().size, Vec2::new(TABLE_MIN_TRACK + 70.0, TABLE_MIN_TRACK + 50.0));
    assert!(d.validate_and_repair().is_clean());
}

#[test]
fn repair_gives_a_repeated_track_id_a_new_one() {
    let (mut d, _, table) = doc_with_table(2, 2);
    let NodeKind::Table(tp) = &mut d.get_mut(table).unwrap().kind else { unreachable!() };
    tp.cols[1].id = tp.cols[0].id;
    let rep = d.validate_and_repair();
    assert_eq!(rep.tables.tracks_reidentified, 1);
    assert_eq!(check_table_positions(&d, table), Ok(()));
    assert!(d.validate_and_repair().is_clean());
}

#[test]
fn a_well_formed_table_loads_clean() {
    let (mut d, _, table) = sized_table();
    merge_top_left_2x2(&mut d, table);
    apply_relayout(&mut d, table);
    let (back, report) = Document::from_json_with_report(&d.to_json().unwrap()).unwrap();
    assert!(report.is_clean(), "{report:?}");
    assert_eq!(back.nodes, d.nodes);
}

// ---------------------------------------------------------------- tree guards

fn snapshot(d: &Document) -> String {
    d.to_json().unwrap()
}

fn assert_refused_and_untouched(d: &mut Document, h: &mut History, cmd: Command) -> String {
    let before = snapshot(d);
    let err = h.exec_user(d, cmd).expect_err("the edit must be refused");
    assert!(matches!(err, DocError::Table(_)), "{err:?}");
    assert_eq!(snapshot(d), before, "the document is unchanged");
    assert!(!h.can_undo() && !h.can_redo(), "no step was recorded");
    err.to_string()
}

#[test]
fn kind_checks_refuse_foreign_children_and_loose_cells() {
    let (mut d, page, table) = doc_with_table(3, 3);
    let mut h = History::default();
    let rect = d.alloc_id();
    let msg = assert_refused_and_untouched(
        &mut d,
        &mut h,
        Command::Insert { parent: table, index: 0, nodes: vec![Node::rectangle(rect, Rect::new(0.0, 0.0, 5.0, 5.0))] },
    );
    assert!(msg.contains("table operation"), "{msg}");

    let frame = d.alloc_id();
    d.insert(page, Node::frame(frame, Rect::new(0.0, 0.0, 50.0, 50.0))).unwrap();
    let p = props(&d, table);
    let loose = d.alloc_id();
    let kind = NodeKind::TableCell(TableCellProps { row: p.rows[0].id, col: p.cols[0].id, text: default_cell_text("") });
    assert_refused_and_untouched(&mut d, &mut h, Command::Insert { parent: frame, index: 0, nodes: vec![Node::new(loose, kind)] });

    let any_cell = cell(&d, table, 1, 1).unwrap();
    assert_refused_and_untouched(&mut d, &mut h, Command::Reparent { id: any_cell, parent: page, index: 0 });
    let r2 = d.alloc_id();
    d.insert(page, Node::rectangle(r2, Rect::new(0.0, 0.0, 5.0, 5.0))).unwrap();
    assert_refused_and_untouched(&mut d, &mut h, Command::Reparent { id: r2, parent: table, index: 0 });
}

#[test]
fn removing_one_cell_is_refused_and_rolled_back() {
    let (mut d, _, table) = doc_with_table(3, 3);
    let mut h = History::default();
    let victim = cell(&d, table, 1, 1).unwrap();
    let msg = assert_refused_and_untouched(&mut d, &mut h, Command::Remove { id: victim });
    assert!(msg.contains("Use a table operation"), "{msg}");
    assert!(d.get(victim).is_some());
    assert_eq!(check_table_positions(&d, table), Ok(()));
}

fn new_cell(d: &mut Document, p: &TableProps, r: usize, c: usize) -> Node {
    let id = d.alloc_id();
    let kind = NodeKind::TableCell(TableCellProps { row: p.rows[r].id, col: p.cols[c].id, text: default_cell_text("") });
    Node::new(id, kind)
}

fn with_props(d: &Document, table: NodeId, p: TableProps) -> Command {
    let mut n = d.get(table).unwrap().clone();
    n.kind = NodeKind::Table(p);
    Command::Update { node: Box::new(n) }
}

fn undo_redo_cycle(d: &mut Document, h: &mut History, table: NodeId) {
    let after = snapshot(d);
    assert!(h.undo(d).unwrap());
    assert_eq!(check_table_positions(d, table), Ok(()));
    assert!(h.redo(d).unwrap());
    assert_eq!(check_table_positions(d, table), Ok(()));
    assert_eq!(snapshot(d), after, "redo restores the edited state");
}

#[test]
fn insert_rows_batch_succeeds_and_undoes() {
    let (mut d, _, table) = doc_with_table(3, 3);
    let mut h = History::default();
    let mut p = props(&d, table);
    let row = p.alloc_track();
    p.rows.push(TableTrack { id: row, size: 44.0 });
    let cells: Vec<Command> = (0..3).map(|c| Command::Insert { parent: table, index: 99, nodes: vec![new_cell(&mut d, &p, 3, c)] }).collect();
    let mut steps = vec![with_props(&d, table, p)];
    steps.extend(cells);
    h.exec_user(&mut d, Command::Batch(steps)).unwrap();
    assert_eq!(d.get(table).unwrap().children.len(), 12);
    assert_eq!(check_table_positions(&d, table), Ok(()));
    undo_redo_cycle(&mut d, &mut h, table);
    assert_eq!(d.get(table).unwrap().children.len(), 12);
    assert!(h.undo(&mut d).unwrap());
    assert_eq!(d.get(table).unwrap().children.len(), 9);
}

#[test]
fn delete_rows_batch_succeeds_in_either_order_and_undoes() {
    for cells_first in [true, false] {
        let (mut d, _, table) = doc_with_table(3, 3);
        let mut h = History::default();
        let mut p = props(&d, table);
        let doomed: Vec<NodeId> = (0..3).map(|c| cell(&d, table, 2, c).unwrap()).collect();
        p.rows.pop();
        let mut steps: Vec<Command> = doomed.iter().map(|&id| Command::Remove { id }).collect();
        let update = with_props(&d, table, p);
        if cells_first {
            steps.push(update);
        } else {
            steps.insert(0, update);
        }
        h.exec_user(&mut d, Command::Batch(steps)).unwrap();
        assert_eq!(d.get(table).unwrap().children.len(), 6, "cells_first = {cells_first}");
        assert_eq!(check_table_positions(&d, table), Ok(()));
        undo_redo_cycle(&mut d, &mut h, table);
        assert!(h.undo(&mut d).unwrap());
        assert_eq!(d.get(table).unwrap().children.len(), 9);
        assert_eq!(check_table_positions(&d, table), Ok(()));
    }
}

#[test]
fn a_batch_that_leaves_a_hole_is_rolled_back_whole() {
    let (mut d, _, table) = doc_with_table(3, 3);
    let mut h = History::default();
    let mut p = props(&d, table);
    p.rows.pop();
    let one_cell_short = vec![Command::Remove { id: cell(&d, table, 2, 0).unwrap() }, with_props(&d, table, p)];
    assert_refused_and_untouched(&mut d, &mut h, Command::Batch(one_cell_short));
}

#[test]
fn removing_the_table_removes_its_cells_and_undo_restores_them() {
    let (mut d, _, table) = doc_with_table(3, 3);
    let mut h = History::default();
    let ids = d.get(table).unwrap().children.clone();
    let before = snapshot(&d);
    h.exec_user(&mut d, Command::Remove { id: table }).unwrap();
    assert!(d.get(table).is_none() && ids.iter().all(|id| d.get(*id).is_none()));
    assert!(h.undo(&mut d).unwrap());
    assert_eq!(snapshot(&d), before);
    assert_eq!(check_table_positions(&d, table), Ok(()));
}

#[test]
fn a_text_edit_of_one_cell_is_a_plain_update() {
    let (mut d, _, table) = doc_with_table(3, 3);
    let mut h = History::default();
    let id = cell(&d, table, 0, 0).unwrap();
    let mut n = d.get(id).unwrap().clone();
    if let NodeKind::TableCell(c) = &mut n.kind {
        c.text.content = "hello".into();
    }
    h.exec_user(&mut d, Command::Update { node: Box::new(n) }).unwrap();
    assert_eq!(text_of(&d, id), "hello");
    assert!(h.undo(&mut d).unwrap());
    assert_eq!(text_of(&d, id), "");
}

#[test]
fn next_native_track_id_never_reuses_an_id_in_the_table() {
    let mut p = TableProps::grid(2, 2);
    p.next_track = 1;
    let fresh = p.alloc_track();
    assert!(p.rows.iter().chain(&p.cols).all(|t| t.id != fresh));
}

/// Every line is 20 px tall and 10 px per character wide; lines break at `\n` only.
struct LineMeasure;

impl text_measure::TextMeasure for LineMeasure {
    fn measure(&self, tp: &TextProps, _wrap_width: Option<f32>) -> Vec2 {
        let lines = tp.content.split('\n').collect::<Vec<_>>();
        Vec2::new(lines.iter().map(|l| l.chars().count()).max().unwrap_or(0) as f32 * 10.0, lines.len() as f32 * 20.0)
    }
}

#[test]
fn a_row_grows_to_its_tallest_cell_but_the_stored_height_stays() {
    let _guard = text_measure::ThreadLayoutMeasurerGuard::install(std::sync::Arc::new(LineMeasure));
    let (mut d, _, table) = sized_table();
    set_text_at(&mut d, table, 0, 1, "one\ntwo\nthree");
    set_text_at(&mut d, table, 2, 0, "short");
    apply_relayout(&mut d, table);
    let (_, y1, _, h1) = frame_of(&d, cell(&d, table, 1, 0).unwrap());
    let grown = 3.0 * 20.0 + 2.0 * TABLE_CELL_PADDING.1;
    assert_eq!(frame_of(&d, cell(&d, table, 0, 1).unwrap()), (120.0, 0.0, 240.0, grown));
    assert_eq!((y1, h1), (grown, 40.0), "the next row moves down");
    assert_eq!(frame_of(&d, cell(&d, table, 2, 0).unwrap()).3, 64.0, "a short text never shrinks a row");
    assert_eq!(d.get(table).unwrap().size, Vec2::new(450.0, grown + 40.0 + 64.0));
    assert_eq!(props(&d, table).rows[0].size, 40.0, "the user's height is kept for export");
    set_text_at(&mut d, table, 0, 1, "");
    apply_relayout(&mut d, table);
    assert_eq!(frame_of(&d, cell(&d, table, 0, 1).unwrap()).3, 40.0, "the row shrinks back to the stored height");
}

#[test]
fn a_merged_block_with_tall_text_grows_its_last_row_by_the_shortfall() {
    let _guard = text_measure::ThreadLayoutMeasurerGuard::install(std::sync::Arc::new(LineMeasure));
    let (mut d, _, table) = sized_table();
    merge_top_left_2x2(&mut d, table);
    set_text_at(&mut d, table, 0, 0, "1\n2\n3\n4\n5");
    apply_relayout(&mut d, table);
    let shortfall = 5.0 * 20.0 + 2.0 * TABLE_CELL_PADDING.1 - 80.0;
    assert!(shortfall > 0.0);
    let (_, _, _, h) = frame_of(&d, cell(&d, table, 0, 0).unwrap());
    assert_eq!(h, 80.0 + shortfall, "the block is exactly as tall as its text");
    let (_, y1, _, h1) = frame_of(&d, cell(&d, table, 1, 2).unwrap());
    assert_eq!((y1, h1), (40.0, 40.0 + shortfall), "only the last spanned row grows");
    assert_eq!(frame_of(&d, cell(&d, table, 2, 2).unwrap()).1, 80.0 + shortfall, "the next row moves down");
    assert_eq!(props(&d, table).rows[1].size, 40.0, "the user's height is kept");
    set_text_at(&mut d, table, 0, 0, "short");
    apply_relayout(&mut d, table);
    assert_eq!(frame_of(&d, cell(&d, table, 0, 0).unwrap()).3, 80.0, "the rows shrink back to the stored heights");
    assert_eq!(frame_of(&d, cell(&d, table, 1, 2).unwrap()).3, 40.0);
}

#[test]
fn a_merged_block_whose_text_fits_leaves_its_rows_alone() {
    let _guard = text_measure::ThreadLayoutMeasurerGuard::install(std::sync::Arc::new(LineMeasure));
    let (mut d, _, table) = sized_table();
    merge_top_left_2x2(&mut d, table);
    set_text_at(&mut d, table, 0, 0, "1\n2\n3");
    apply_relayout(&mut d, table);
    assert_eq!(frame_of(&d, cell(&d, table, 0, 0).unwrap()).3, 80.0);
    assert_eq!(frame_of(&d, cell(&d, table, 1, 2).unwrap()).3, 40.0);
}

/// Merges the block of `rs` x `cs` tracks at `(r0, c0)`: records the merge and removes the covered cells.
fn merge_block(d: &mut Document, table: NodeId, (r0, c0): (usize, usize), (rs, cs): (usize, usize)) {
    let p = props(d, table);
    for r in r0..r0 + rs {
        for c in c0..c0 + cs {
            if (r, c) != (r0, c0) {
                let id = cell(d, table, r, c).unwrap();
                d.remove(id).unwrap();
            }
        }
    }
    let NodeKind::Table(t) = &mut d.get_mut(table).unwrap().kind else { unreachable!() };
    t.merges.push(TableMerge { row: p.rows[r0].id, col: p.cols[c0].id, row_span: rs as u32, col_span: cs as u32 });
}

/// `n` lines of text, `20 * n` tall under [`LineMeasure`].
fn lines(n: usize) -> String {
    vec!["x"; n].join("\n")
}

fn lines_height(n: usize) -> f32 {
    20.0 * n as f32 + 2.0 * TABLE_CELL_PADDING.1
}

#[test]
fn merges_that_end_on_one_row_leave_that_row_as_tall_as_the_hungriest_block() {
    let _guard = text_measure::ThreadLayoutMeasurerGuard::install(std::sync::Arc::new(LineMeasure));
    let (mut d, _, table) = sized_table();
    merge_block(&mut d, table, (0, 0), (2, 1));
    merge_block(&mut d, table, (0, 1), (2, 1));
    set_text_at(&mut d, table, 0, 0, &lines(5));
    set_text_at(&mut d, table, 0, 1, &lines(7));
    apply_relayout(&mut d, table);
    let want = lines_height(7);
    assert_eq!(frame_of(&d, cell(&d, table, 0, 0).unwrap()).3, want, "both blocks are as tall as the hungriest");
    assert_eq!(frame_of(&d, cell(&d, table, 0, 1).unwrap()).3, want);
    assert_eq!(frame_of(&d, cell(&d, table, 1, 2).unwrap()).3, want - 40.0, "the shared last row took the larger shortfall once");
    assert_eq!(frame_of(&d, cell(&d, table, 2, 0).unwrap()).1, want, "the next row moves down");
}

#[test]
fn a_block_over_rows_and_columns_is_measured_at_the_summed_width_and_grows_its_last_row() {
    let _guard = text_measure::ThreadLayoutMeasurerGuard::install(std::sync::Arc::new(LineMeasure));
    let (mut d, _, table) = sized_table();
    merge_block(&mut d, table, (1, 1), (2, 2));
    set_text_at(&mut d, table, 1, 1, &lines(7));
    apply_relayout(&mut d, table);
    let h = lines_height(7);
    assert_eq!(frame_of(&d, cell(&d, table, 1, 1).unwrap()), (120.0, 40.0, 330.0, h), "spans both columns and exactly its text");
    assert_eq!(frame_of(&d, cell(&d, table, 1, 0).unwrap()).3, 40.0, "the first spanned row keeps its height");
    assert_eq!(d.get(table).unwrap().size, Vec2::new(450.0, 40.0 + h));
}

#[test]
fn a_tall_cell_in_the_last_row_of_a_block_is_not_grown_twice() {
    let _guard = text_measure::ThreadLayoutMeasurerGuard::install(std::sync::Arc::new(LineMeasure));
    let (mut d, _, table) = sized_table();
    merge_block(&mut d, table, (0, 0), (2, 1));
    set_text_at(&mut d, table, 0, 0, &lines(5));
    set_text_at(&mut d, table, 1, 1, &lines(5));
    apply_relayout(&mut d, table);
    let h = lines_height(5);
    assert_eq!(frame_of(&d, cell(&d, table, 1, 1).unwrap()).3, h, "the single cell sets the row");
    assert_eq!(frame_of(&d, cell(&d, table, 0, 0).unwrap()).3, 40.0 + h, "the block already fits: its text needs only {h}");
}

#[test]
fn an_edit_that_grows_a_merged_row_undoes_with_the_edit() {
    let _guard = text_measure::ThreadLayoutMeasurerGuard::install(std::sync::Arc::new(LineMeasure));
    let (mut d, _, table) = sized_table();
    merge_top_left_2x2(&mut d, table);
    apply_relayout(&mut d, table);
    let anchor = cell(&d, table, 0, 0).unwrap();
    let mut grown = d.get(anchor).unwrap().clone();
    if let NodeKind::TableCell(c) = &mut grown.kind {
        c.text.content = lines(5);
    }
    let mut h = History::default();
    h.exec(&mut d, Command::Update { node: Box::new(grown) }).unwrap();
    assert_eq!(frame_of(&d, anchor).3, lines_height(5), "the row grew with the edit");
    assert_eq!(frame_of(&d, cell(&d, table, 2, 2).unwrap()).1, lines_height(5));
    assert!(h.undo(&mut d).unwrap());
    assert_eq!(frame_of(&d, anchor).3, 80.0, "undo gives the rows back");
    assert_eq!(frame_of(&d, cell(&d, table, 2, 2).unwrap()).1, 80.0);
    assert_eq!(text_of(&d, anchor), "");
    assert!(h.redo(&mut d).unwrap());
    assert_eq!(frame_of(&d, anchor).3, lines_height(5), "redo grows them again");
}

#[test]
fn find_and_replace_in_a_cell_grows_its_row_and_no_text_is_hidden() {
    use crate::find::{find_text, replace, FindOptions, Scope};
    let _guard = text_measure::ThreadLayoutMeasurerGuard::install(std::sync::Arc::new(LineMeasure));
    let (mut d, page, table) = sized_table();
    set_text_at(&mut d, table, 0, 1, "Hi");
    apply_relayout(&mut d, table);
    let id = cell(&d, table, 0, 1).unwrap();
    assert_eq!(frame_of(&d, id).3, 40.0);
    let found = find_text(&d, &Scope::Page(page), "Hi", FindOptions::default());
    assert_eq!(found.len(), 1);
    let mut h = History::default();
    let cmd = replace(&d, &found, "1\n2\n3");
    h.exec(&mut d, cmd).unwrap();
    assert_eq!(frame_of(&d, id).3, lines_height(3), "the row grew to hold the replacement");
    assert_eq!(frame_of(&d, cell(&d, table, 1, 0).unwrap()).1, lines_height(3));
    assert!(h.undo(&mut d).unwrap());
    assert_eq!(frame_of(&d, id).3, 40.0);
    assert_eq!(text_of(&d, id), "Hi");
}

#[test]
fn an_edit_that_leaves_the_text_alone_does_not_touch_the_geometry() {
    let _guard = text_measure::ThreadLayoutMeasurerGuard::install(std::sync::Arc::new(LineMeasure));
    let (mut d, _, table) = sized_table();
    let id = cell(&d, table, 0, 0).unwrap();
    let mut n = d.get(id).unwrap().clone();
    n.name = "Renamed".into();
    let mut h = History::default();
    h.exec(&mut d, Command::Update { node: Box::new(n) }).unwrap();
    assert_eq!(h.undo_depth(), 1);
    assert!(relayout(&d, table).is_empty());
}

// ---------------------------------------------------------------- review round 2

/// `node` as a child of `parent` inside an `Insert` payload.
fn child_of(parent: &mut Node, mut node: Node) -> Node {
    node.parent = Some(parent.id);
    parent.children.push(node.id);
    node
}

#[test]
fn insert_checks_every_node_of_the_payload_not_only_the_root() {
    let (mut d, page, table) = doc_with_table(2, 2);
    let mut h = History::default();
    let p = props(&d, table);

    // a frame that carries a cell
    let fid = d.alloc_id();
    let mut frame = Node::frame(fid, Rect::new(0.0, 0.0, 50.0, 50.0));
    let loose = new_cell(&mut d, &p, 0, 0);
    let loose = child_of(&mut frame, loose);
    assert_refused_and_untouched(&mut d, &mut h, Command::Insert { parent: page, index: 0, nodes: vec![frame, loose] });

    // a table that carries a rectangle
    let mut nodes = build_table(&mut d, 2, 2, Vec2::ZERO);
    let rid = d.alloc_id();
    let rect = Node::rectangle(rid, Rect::new(0.0, 0.0, 5.0, 5.0));
    let rect = child_of(&mut nodes[0], rect);
    nodes.push(rect);
    assert_refused_and_untouched(&mut d, &mut h, Command::Insert { parent: page, index: 0, nodes });

    // a table that carries a table
    let mut outer = build_table(&mut d, 2, 2, Vec2::ZERO);
    let inner = build_table(&mut d, 1, 1, Vec2::ZERO);
    let inner_root = child_of(&mut outer[0], inner[0].clone());
    outer.push(inner_root);
    outer.extend(inner.into_iter().skip(1));
    assert_refused_and_untouched(&mut d, &mut h, Command::Insert { parent: page, index: 0, nodes: outer });

    // a whole well-formed table still goes in
    let good = build_table(&mut d, 2, 2, Vec2::ZERO);
    h.exec_user(&mut d, Command::Insert { parent: page, index: 0, nodes: good }).unwrap();
}

#[test]
fn changing_the_kind_of_a_table_or_cell_is_refused() {
    let (mut d, page, table) = doc_with_table(2, 2);
    let mut h = History::default();
    let id = cell(&d, table, 0, 0).unwrap();
    let mut to_text = d.get(id).unwrap().clone();
    to_text.kind = NodeKind::Text(default_cell_text("now text"));
    assert_refused_and_untouched(&mut d, &mut h, Command::Update { node: Box::new(to_text) });

    let mut to_frame = d.get(table).unwrap().clone();
    to_frame.kind = NodeKind::Frame(Default::default());
    assert_refused_and_untouched(&mut d, &mut h, Command::Update { node: Box::new(to_frame) });

    let rid = d.alloc_id();
    d.insert(page, Node::rectangle(rid, Rect::new(0.0, 0.0, 5.0, 5.0))).unwrap();
    let mut to_cell = d.get(rid).unwrap().clone();
    to_cell.kind = NodeKind::TableCell(TableCellProps { row: props(&d, table).rows[0].id, col: props(&d, table).cols[0].id, text: default_cell_text("") });
    assert_refused_and_untouched(&mut d, &mut h, Command::Update { node: Box::new(to_cell) });

    // an edit that keeps the kind is still a plain update
    let mut renamed = d.get(id).unwrap().clone();
    renamed.name = "Header".into();
    h.exec_user(&mut d, Command::Update { node: Box::new(renamed) }).unwrap();
}

/// A document where a rectangle sits in a frame, then the frame is turned into a table behind
/// the history's back (a legacy file), so the inverse of an earlier step is now refused.
fn history_with_a_step_that_can_no_longer_be_replayed() -> (Document, History, NodeId) {
    let mut d = Document::new("Replay");
    let page = d.add_page("P");
    let frame = d.alloc_id();
    d.insert(page, Node::frame(frame, Rect::new(0.0, 0.0, 100.0, 100.0))).unwrap();
    let rect = d.alloc_id();
    d.insert(page, Node::rectangle(rect, Rect::new(0.0, 0.0, 5.0, 5.0))).unwrap();
    let mut h = History::default();
    h.exec_user(&mut d, Command::Reparent { id: rect, parent: frame, index: 0 }).unwrap();
    (d, h, frame)
}

fn make_table(d: &mut Document, id: NodeId) {
    d.get_mut(id).unwrap().kind = NodeKind::Table(TableProps::grid(1, 1));
}

#[test]
fn a_refused_undo_keeps_the_step_and_both_stacks() {
    let (mut d, mut h, frame) = history_with_a_step_that_can_no_longer_be_replayed();
    let rect = d.get(frame).unwrap().children[0];
    let page = d.pages[0];
    h.exec_user(&mut d, Command::Reparent { id: rect, parent: page, index: 0 }).unwrap();
    make_table(&mut d, frame);
    let (before, depth) = (snapshot(&d), h.undo_depth());
    let err = h.undo(&mut d).expect_err("the inverse puts a rectangle back into a table");
    assert!(matches!(err, DocError::Table(_)), "{err:?}");
    assert_eq!((snapshot(&d), h.undo_depth(), h.can_redo()), (before, depth, false));
}

#[test]
fn a_refused_redo_keeps_the_step_and_both_stacks() {
    let (mut d, mut h, frame) = history_with_a_step_that_can_no_longer_be_replayed();
    assert!(h.undo(&mut d).unwrap());
    make_table(&mut d, frame);
    let before = snapshot(&d);
    let err = h.redo(&mut d).expect_err("the step puts a rectangle into a table");
    assert!(matches!(err, DocError::Table(_)), "{err:?}");
    assert_eq!((snapshot(&d), h.can_redo(), h.can_undo()), (before, true, false));
}

#[test]
fn a_table_cannot_lose_its_last_row_or_column() {
    let (mut d, _, table) = doc_with_table(1, 2);
    let mut h = History::default();
    let mut p = props(&d, table);
    p.rows.clear();
    let mut steps: Vec<Command> = d.get(table).unwrap().children.iter().map(|&id| Command::Remove { id }).collect();
    steps.push(with_props(&d, table, p));
    let msg = assert_refused_and_untouched(&mut d, &mut h, Command::Batch(steps));
    assert!(msg.contains("table operation"), "{msg}");
    assert_eq!(check_table_positions(&d, table), Ok(()));

    let mut empty = d.get(table).unwrap().clone();
    empty.kind = NodeKind::Table(TableProps { rows: vec![], cols: vec![], merges: vec![], next_track: 1 });
    empty.children.clear();
    d.nodes.insert(table, empty);
    assert!(check_table_positions(&d, table).is_err(), "the guard and the load repair agree that this table is broken");
}

#[test]
fn a_user_step_with_a_refused_command_applies_nothing() {
    let (mut d, page, table) = doc_with_table(2, 2);
    let mut h = History::default();
    let victim = cell(&d, table, 0, 0).unwrap();
    let group = d.alloc_id();
    // group a cell: Insert a group on the page, then move the cell into it
    let group_it = vec![
        Command::Insert { parent: page, index: 0, nodes: vec![Node::new(group, NodeKind::Group)] },
        Command::Reparent { id: victim, parent: group, index: 0 },
    ];
    assert_refused_and_untouched(&mut d, &mut h, Command::Batch(group_it));
    assert!(d.get(group).is_none(), "no stray group is left behind");

    // an earlier, valid command of the same step is rolled back too
    let mut renamed = d.get(table).unwrap().clone();
    renamed.name = "Renamed".into();
    let step = vec![Command::Update { node: Box::new(renamed) }, Command::Reparent { id: victim, parent: page, index: 0 }];
    assert_refused_and_untouched(&mut d, &mut h, Command::Batch(step));
    assert_eq!(d.get(table).unwrap().name, "Table");
}

#[test]
fn a_child_moved_out_of_a_table_keeps_its_place_on_the_canvas() {
    for rotation in [0.0_f32, 0.5] {
        let (mut d, _, table) = doc_with_table(2, 2);
        let t = d.get_mut(table).unwrap();
        t.transform = Affine::translate(10.0, 20.0) * Affine::rotate(rotation);
        let rect = d.alloc_id();
        let mut r = Node::rectangle(rect, Rect::new(5.0, 5.0, 8.0, 6.0));
        r.transform = Affine::translate(5.0, 5.0);
        d.insert(table, r).unwrap();
        let before = d.world_transform(rect);
        assert_eq!(d.validate_and_repair().tables.children_moved_out, 1);
        let after = d.world_transform(rect);
        for (a, b) in [(before.a, after.a), (before.b, after.b), (before.c, after.c), (before.d, after.d), (before.tx, after.tx), (before.ty, after.ty)] {
            assert!((a - b).abs() < 1e-4, "rotation {rotation}: {before:?} vs {after:?}");
        }
        assert!(rotation != 0.0 || (after.tx, after.ty) == (15.0, 25.0));
    }
}

#[test]
fn repair_moves_foreign_children_out_of_a_table_after_it() {
    let (mut d, page, table) = doc_with_table(2, 2);
    let rect = d.alloc_id();
    d.insert(table, Node::rectangle(rect, Rect::new(0.0, 0.0, 5.0, 5.0))).unwrap();
    let inner = build_table(&mut d, 1, 1, Vec2::ZERO);
    let inner_id = inner[0].id;
    for n in inner {
        let is_root = n.id == inner_id;
        d.nodes.insert(n.id, n);
        if is_root {
            d.nodes.get_mut(&inner_id).unwrap().parent = Some(table);
            d.nodes.get_mut(&table).unwrap().children.push(inner_id);
        }
    }
    let rep = d.validate_and_repair();
    assert_eq!(rep.tables.children_moved_out, 2);
    let order = d.get(page).unwrap().children.clone();
    let at = order.iter().position(|&id| id == table).unwrap();
    assert_eq!(&order[at + 1..], &[rect, inner_id], "both follow the table, in order");
    assert_eq!(d.get(rect).unwrap().parent, Some(page));
    assert_eq!(d.get(table).unwrap().children.len(), 4);
    assert_eq!(check_table_positions(&d, table), Ok(()));
    assert!(d.validate_and_repair().is_clean());
}
