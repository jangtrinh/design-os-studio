//! Replaying the live-patch log of a table edit gives the table the agent side holds.

use super::{live_ops_for_command, replay_live_patch, LivePatch, LivePatchReport, LivePatchState};
use crate::table_ops::{
    create_table, delete_cols, delete_rows, insert_cols, insert_rows, merge, move_col, move_row, resize_col, resize_row, resize_table, set_cell_text, unmerge,
};
use crate::*;
use serde_json::Value;

type Built = Result<Command, TableOpError>;

fn table_doc(rows: usize, cols: usize) -> (Document, NodeId, NodeId) {
    let mut d = Document::new("Table replay");
    let page = d.add_page("P");
    let cmd = create_table(&d, page, Vec2::new(10.0, 20.0), rows, cols, &TableStyle::default()).unwrap();
    cmd.apply(&mut d).unwrap();
    let table = d.get(page).unwrap().children[0];
    for (i, id) in d.get(table).unwrap().children.clone().into_iter().enumerate() {
        if let NodeKind::TableCell(c) = &mut d.get_mut(id).unwrap().kind {
            c.text.content = format!("t{i}");
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

/// The table node and its cells in child order, as JSON, so ids, spans, sizes and text all compare.
fn subtree(d: &Document, table: NodeId) -> Vec<Value> {
    let t = d.get(table).unwrap_or_else(|| panic!("table {} is missing", table.0));
    std::iter::once(t.id).chain(t.children.iter().copied()).map(|id| serde_json::to_value(d.get(id).unwrap()).unwrap()).collect()
}

/// Runs `op` on the agent copy, records its patch, replays it (through the JSON log round trip) on
/// the editor copy with the table rule checked at the end, and returns the editor report.
fn parity(agent: &mut Document, editor: &mut Document, table: NodeId, op: impl FnOnce(&Document) -> Built) -> LivePatchReport {
    let cmd = op(agent).expect("the op is accepted");
    let ops = live_ops_for_command(agent, &cmd).expect("the op can be mirrored");
    let patch: LivePatch = serde_json::from_str(&serde_json::to_string(&LivePatch::new(ops)).unwrap()).unwrap();
    History::default().exec_agent(agent, cmd).expect("the agent applies it");
    let report = replay_live_patch(editor, &patch, &mut LivePatchState::default(), &mut |d, c| c.apply(d).map(|_| ())).unwrap();
    assert!(report.skipped.is_empty(), "nothing is skipped: {:?}", report.skipped);
    if editor.get(table).is_some() {
        check_table_positions(editor, table).expect("the editor table keeps one cell per position");
    }
    report
}

fn same(agent: &Document, editor: &Document, table: NodeId) {
    for page in &agent.pages {
        assert_eq!(editor.get(*page).unwrap().children, agent.get(*page).unwrap().children, "the page lists the same children in the same order");
    }
    if agent.get(table).is_none() {
        assert!(editor.get(table).is_none(), "the table is gone on both sides");
    } else {
        assert_eq!(subtree(editor, table), subtree(agent, table));
    }
    assert_eq!(agent.nodes.len(), editor.nodes.len());
}

fn pair(rows: usize, cols: usize) -> (Document, Document, NodeId) {
    let (agent, _, table) = table_doc(rows, cols);
    (agent.clone(), agent, table)
}

#[test]
fn created_table_reaches_the_editor() {
    let (mut agent, page, _) = table_doc(1, 1);
    let mut editor = agent.clone();
    let cmd = create_table(&agent, page, Vec2::new(5.0, 6.0), 2, 3, &TableStyle::default()).unwrap();
    let ops = live_ops_for_command(&agent, &cmd).unwrap();
    cmd.apply(&mut agent).unwrap();
    let report = replay_live_patch(&mut editor, &LivePatch::new(ops), &mut LivePatchState::default(), &mut |d, c| c.apply(d).map(|_| ())).unwrap();
    assert!(report.skipped.is_empty(), "{:?}", report.skipped);
    let table = report.inserted_roots[0];
    check_table_positions(&editor, table).unwrap();
    same(&agent, &editor, table);
}

#[test]
fn insert_rows_matches() {
    let (mut a, mut e, t) = pair(3, 3);
    parity(&mut a, &mut e, t, |d| insert_rows(d, t, 1, 2));
    same(&a, &e, t);
}

#[test]
fn insert_cols_matches() {
    let (mut a, mut e, t) = pair(3, 3);
    parity(&mut a, &mut e, t, |d| insert_cols(d, t, 3, 1));
    same(&a, &e, t);
}

#[test]
fn delete_rows_matches() {
    let (mut a, mut e, t) = pair(4, 3);
    let id = props(&a, t).rows[1].id;
    parity(&mut a, &mut e, t, |d| delete_rows(d, t, &[id]));
    same(&a, &e, t);
}

#[test]
fn delete_cols_matches() {
    let (mut a, mut e, t) = pair(3, 4);
    let ids: Vec<_> = props(&a, t).cols[..2].iter().map(|c| c.id).collect();
    parity(&mut a, &mut e, t, |d| delete_cols(d, t, &ids));
    same(&a, &e, t);
}

#[test]
fn delete_every_row_removes_the_table() {
    let (mut a, mut e, t) = pair(2, 2);
    let ids: Vec<_> = props(&a, t).rows.iter().map(|r| r.id).collect();
    parity(&mut a, &mut e, t, |d| delete_rows(d, t, &ids));
    same(&a, &e, t);
}

#[test]
fn move_row_matches() {
    let (mut a, mut e, t) = pair(4, 3);
    let id = props(&a, t).rows[0].id;
    parity(&mut a, &mut e, t, |d| move_row(d, t, id, 2));
    same(&a, &e, t);
}

#[test]
fn move_col_matches() {
    let (mut a, mut e, t) = pair(3, 4);
    let id = props(&a, t).cols[3].id;
    parity(&mut a, &mut e, t, |d| move_col(d, t, id, 0));
    same(&a, &e, t);
}

#[test]
fn merge_matches() {
    let (mut a, mut e, t) = pair(3, 3);
    parity(&mut a, &mut e, t, |d| merge(d, t, (0, 0, 1, 1)));
    same(&a, &e, t);
}

#[test]
fn unmerge_matches() {
    let (mut a, mut e, t) = pair(3, 3);
    parity(&mut a, &mut e, t, |d| merge(d, t, (0, 0, 1, 1)));
    let p = props(&a, t);
    let anchor = a
        .get(t)
        .unwrap()
        .children
        .iter()
        .copied()
        .find(|c| matches!(&a.get(*c).unwrap().kind, NodeKind::TableCell(x) if x.row == p.rows[0].id && x.col == p.cols[0].id))
        .unwrap();
    parity(&mut a, &mut e, t, |d| unmerge(d, t, anchor));
    same(&a, &e, t);
}

#[test]
fn resize_matches() {
    let (mut a, mut e, t) = pair(3, 3);
    parity(&mut a, &mut e, t, |d| resize_table(d, t, Vec2::new(500.0, 300.0)));
    same(&a, &e, t);
}

#[test]
fn resize_col_matches() {
    let (mut a, mut e, t) = pair(3, 3);
    let id = props(&a, t).cols[1].id;
    parity(&mut a, &mut e, t, |d| resize_col(d, t, id, 240.0));
    same(&a, &e, t);
}

#[test]
fn resize_row_matches() {
    let (mut a, mut e, t) = pair(3, 3);
    let id = props(&a, t).rows[2].id;
    parity(&mut a, &mut e, t, |d| resize_row(d, t, id, 90.0));
    same(&a, &e, t);
}

#[test]
fn set_cell_text_matches() {
    let (mut a, mut e, t) = pair(3, 3);
    parity(&mut a, &mut e, t, |d| set_cell_text(d, t, 1, 2, "changed\nin two lines"));
    same(&a, &e, t);
}

/// Replays the edit and, like the MCP session, the undo and the redo of it, recorded from the step's inverse.
fn parity_through_undo_redo(rows: usize, cols: usize, op: impl Fn(&Document, NodeId) -> Built) {
    let (mut a, mut e, t) = pair(rows, cols);
    let mut hist = History::default();
    let replay = |from: &Document, cmd: &Command, e: &mut Document| {
        let ops = live_ops_for_command(from, cmd).expect("the step can be mirrored");
        let patch: LivePatch = serde_json::from_str(&serde_json::to_string(&LivePatch::new(ops)).unwrap()).unwrap();
        let report = replay_live_patch(e, &patch, &mut LivePatchState::default(), &mut |d, c| c.apply(d).map(|_| ())).unwrap();
        assert!(report.skipped.is_empty(), "nothing is skipped: {:?}", report.skipped);
    };
    let cmd = op(&a, t).expect("the op is accepted");
    replay(&a, &cmd, &mut e);
    hist.exec_agent(&mut a, cmd).unwrap();
    same(&a, &e, t);
    let inverse = hist.peek_undo().cloned().expect("an undo step");
    replay(&a, &inverse, &mut e);
    assert!(hist.undo(&mut a).unwrap());
    same(&a, &e, t);
    let forward = hist.peek_redo().cloned().expect("a redo step");
    replay(&a, &forward, &mut e);
    assert!(hist.redo(&mut a).unwrap());
    same(&a, &e, t);
}

#[test]
fn undo_and_redo_of_insert_rows_match() {
    parity_through_undo_redo(3, 3, |d, t| insert_rows(d, t, 1, 2));
}

#[test]
fn undo_and_redo_of_delete_cols_match() {
    parity_through_undo_redo(3, 4, |d, t| delete_cols(d, t, &[props(d, t).cols[1].id]));
}

#[test]
fn undo_and_redo_of_merge_match() {
    parity_through_undo_redo(3, 3, |d, t| merge(d, t, (0, 0, 0, 1)));
}

#[test]
fn undo_and_redo_of_move_row_match() {
    parity_through_undo_redo(4, 3, |d, t| move_row(d, t, props(d, t).rows[3].id, 0));
}

#[test]
fn undo_and_redo_of_resize_col_match() {
    parity_through_undo_redo(3, 3, |d, t| resize_col(d, t, props(d, t).cols[0].id, 200.0));
}

#[test]
fn undo_and_redo_of_delete_every_row_match() {
    parity_through_undo_redo(2, 2, |d, t| delete_rows(d, t, &props(d, t).rows.iter().map(|r| r.id).collect::<Vec<_>>()));
}

#[test]
fn a_created_table_lands_in_the_same_place_among_the_page_children() {
    let (mut agent, page, _) = table_doc(1, 1);
    let rect = agent.alloc_id();
    agent.insert(page, Node::rectangle(rect, Rect::new(0.0, 0.0, 5.0, 5.0))).unwrap();
    let mut editor = agent.clone();
    let cmd = create_table(&agent, page, Vec2::new(5.0, 6.0), 2, 2, &TableStyle::default()).unwrap();
    let ops = live_ops_for_command(&agent, &cmd).unwrap();
    cmd.apply(&mut agent).unwrap();
    replay_live_patch(&mut editor, &LivePatch::new(ops), &mut LivePatchState::default(), &mut |d, c| c.apply(d).map(|_| ())).unwrap();
    let table = *agent.get(page).unwrap().children.last().unwrap();
    same(&agent, &editor, table);
}

/// Replays `cmd`, built on `agent`, onto `editor` the way the reconnect path does.
fn replay_onto(agent: &Document, editor: &mut Document, cmd: &Command) {
    let ops = live_ops_for_command(agent, cmd).expect("the op can be mirrored");
    replay_live_patch(editor, &LivePatch::new(ops), &mut LivePatchState::default(), &mut |d, c| c.apply(d).map(|_| ())).unwrap();
}

fn cell_extents(d: &Document, table: NodeId, row: TrackId, col: TrackId) -> Vec2 {
    d.get(table)
        .unwrap()
        .children
        .iter()
        .find_map(|id| match &d.get(*id).unwrap().kind {
            NodeKind::TableCell(c) if c.row == row && c.col == col => d.get(*id).map(|n| n.size),
            _ => None,
        })
        .expect("the cell exists")
}

#[test]
fn an_agent_column_insert_keeps_a_row_height_the_user_set() {
    let (agent, _, table) = table_doc(2, 2);
    let mut editor = agent.clone();
    let row = props(&editor, table).rows[0].id;
    let col = props(&editor, table).cols[0].id;
    resize_row(&editor, table, row, 90.0).unwrap().apply(&mut editor).unwrap();

    replay_onto(&agent, &mut editor, &insert_cols(&agent, table, 1, 1).unwrap());

    let after = props(&editor, table);
    assert_eq!(after.rows[0].size, 90.0, "the user's row height survives the agent's column insert");
    assert_eq!(after.cols.len(), 3, "the agent's column arrived");
    assert_eq!(cell_extents(&editor, table, row, col).y, 90.0, "the cells follow the user's row height");
    check_table_positions(&editor, table).expect("one cell per position");
}

#[test]
fn an_agent_cell_text_edit_keeps_a_column_width_the_user_set() {
    let (agent, _, table) = table_doc(2, 2);
    let mut editor = agent.clone();
    let col = props(&editor, table).cols[0].id;
    resize_col(&editor, table, col, 300.0).unwrap().apply(&mut editor).unwrap();

    replay_onto(&agent, &mut editor, &set_cell_text(&agent, table, 0, 1, "edited").unwrap());

    assert_eq!(props(&editor, table).cols[0].size, 300.0, "a text edit is not a structural change and leaves the user's width alone");
    check_table_positions(&editor, table).expect("one cell per position");
}
