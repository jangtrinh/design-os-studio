//! Replaying the live-patch log of a table edit gives the table the agent side holds.

use super::{live_ops_for_command, replay_live_patch, LivePatch, LivePatchReport, LivePatchState};
use crate::table_ops::{create_table, delete_cols, delete_rows, insert_cols, insert_rows, merge, move_col, move_row, resize_col, resize_table, unmerge};
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
