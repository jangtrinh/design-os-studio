//! Studio replays an agent's table edits to exactly the table the agent side holds.

use crate::app::App;
use crate::live_patch_apply::apply_live_patches;
use designos_doc::live_patch::{live_ops_for_command, LivePatch};
use designos_doc::table_ops::{create_table, delete_cols, delete_rows, insert_cols, insert_rows, merge, move_col, move_row, resize_col, resize_table, unmerge};
use designos_doc::*;
use serde_json::Value;

fn table_doc(rows: usize, cols: usize) -> (Document, NodeId) {
    let mut d = Document::new("Table live patch");
    let page = d.add_page("P");
    create_table(&d, page, Vec2::new(10.0, 20.0), rows, cols, &TableStyle::default()).unwrap().apply(&mut d).unwrap();
    let table = d.get(page).unwrap().children[0];
    for (i, id) in d.get(table).unwrap().children.clone().into_iter().enumerate() {
        if let NodeKind::TableCell(c) = &mut d.get_mut(id).unwrap().kind {
            c.text.content = format!("t{i}");
        }
    }
    (d, table)
}

fn props(d: &Document, table: NodeId) -> TableProps {
    match &d.get(table).unwrap().kind {
        NodeKind::Table(p) => p.clone(),
        _ => panic!("not a table"),
    }
}

/// The table and its cells in child order as JSON: ids, spans, sizes and text all compare.
fn subtree(d: &Document, table: NodeId) -> Vec<Value> {
    let t = d.get(table).unwrap_or_else(|| panic!("table {} is missing", table.0));
    std::iter::once(t.id).chain(t.children.iter().copied()).map(|id| serde_json::to_value(d.get(id).unwrap()).unwrap()).collect()
}

/// The agent runs `op`, its patch goes through the log's JSON, and Studio applies it.
fn run(mcp: &mut Document, app: &mut App, op: impl FnOnce(&Document) -> Result<Command, TableOpError>) {
    let cmd = op(mcp).expect("the op is accepted");
    let ops = live_ops_for_command(mcp, &cmd).expect("the op can be mirrored");
    let patch: LivePatch = serde_json::from_str(&serde_json::to_string(&LivePatch::new(ops)).unwrap()).unwrap();
    History::default().exec_agent(mcp, cmd).expect("the agent applies it");
    apply_live_patches(app, &[patch]);
}

fn same(mcp: &Document, app: &App, table: NodeId) {
    if mcp.get(table).is_none() {
        assert!(app.doc.get(table).is_none(), "the table is gone on both sides");
    } else {
        check_table_positions(&app.doc, table).expect("Studio's table keeps one cell per position");
        assert_eq!(subtree(&app.doc, table), subtree(mcp, table));
    }
    assert_eq!(app.doc.nodes.len(), mcp.nodes.len());
}

fn pair(rows: usize, cols: usize) -> (Document, App, NodeId) {
    let (mcp, table) = table_doc(rows, cols);
    (mcp.clone(), App::new(mcp), table)
}

#[test]
fn a_table_the_agent_creates_appears_in_studio() {
    let (mut mcp, mut app, _) = pair(1, 1);
    let page = mcp.pages[0];
    let cmd = create_table(&mcp, page, Vec2::new(5.0, 6.0), 2, 3, &TableStyle::default()).unwrap();
    let patch = LivePatch::new(live_ops_for_command(&mcp, &cmd).unwrap());
    cmd.apply(&mut mcp).unwrap();
    apply_live_patches(&mut app, &[patch]);
    let table = *mcp.get(page).unwrap().children.last().unwrap();
    assert_eq!(app.hist.undo_depth(), 1);
    same(&mcp, &app, table);
}

macro_rules! parity_tests {
    ($($name:ident: ($rows:expr, $cols:expr) |$d:ident, $t:ident| $op:expr;)*) => {$(
        #[test]
        fn $name() {
            let (mut mcp, mut app, $t) = pair($rows, $cols);
            run(&mut mcp, &mut app, |$d| $op);
            same(&mcp, &app, $t);
            assert_eq!(app.hist.undo_depth(), 1, "one tool call is one undo step");
        }
    )*};
}

parity_tests! {
    insert_rows_matches: (3, 3) |d, t| insert_rows(d, t, 1, 2);
    insert_cols_matches: (3, 3) |d, t| insert_cols(d, t, 3, 1);
    delete_rows_matches: (4, 3) |d, t| delete_rows(d, t, &[props(d, t).rows[1].id]);
    delete_cols_matches: (3, 4) |d, t| delete_cols(d, t, &props(d, t).cols[..2].iter().map(|c| c.id).collect::<Vec<_>>());
    delete_every_row_matches: (2, 2) |d, t| delete_rows(d, t, &props(d, t).rows.iter().map(|r| r.id).collect::<Vec<_>>());
    move_row_matches: (4, 3) |d, t| move_row(d, t, props(d, t).rows[0].id, 2);
    move_col_matches: (3, 4) |d, t| move_col(d, t, props(d, t).cols[3].id, 0);
    merge_matches: (3, 3) |d, t| merge(d, t, (0, 0, 1, 1));
    resize_matches: (3, 3) |d, t| resize_table(d, t, Vec2::new(500.0, 300.0));
    resize_col_matches: (3, 3) |d, t| resize_col(d, t, props(d, t).cols[1].id, 240.0);
}

#[test]
fn unmerge_matches() {
    let (mut mcp, mut app, t) = pair(3, 3);
    run(&mut mcp, &mut app, |d| merge(d, t, (0, 0, 1, 1)));
    let p = props(&mcp, t);
    let anchor = mcp
        .get(t)
        .unwrap()
        .children
        .iter()
        .copied()
        .find(|c| matches!(&mcp.get(*c).unwrap().kind, NodeKind::TableCell(x) if x.row == p.rows[0].id && x.col == p.cols[0].id))
        .unwrap();
    run(&mut mcp, &mut app, |d| unmerge(d, t, anchor));
    same(&mcp, &app, t);
}

/// An `Update` of the table alone that drops a row, leaving cells at a position that no longer exists.
fn dropping_a_track(mcp: &Document, table: NodeId, rows: bool) -> LivePatch {
    let mut broken = mcp.get(table).unwrap().clone();
    if let NodeKind::Table(p) = &mut broken.kind {
        if rows {
            p.rows.pop()
        } else {
            p.cols.pop()
        };
    }
    LivePatch::new(live_ops_for_command(mcp, &Command::Update { node: Box::new(broken) }).unwrap())
}

#[test]
fn a_patch_that_would_break_the_table_is_refused_and_the_next_one_applies() {
    let (mut mcp, mut app, t) = pair(3, 3);
    let before = app.doc.to_json().unwrap();
    apply_live_patches(&mut app, &[dropping_a_track(&mcp, t, true)]);
    assert_eq!(app.doc.to_json().unwrap(), before, "the refused patch changed nothing");
    assert_eq!(app.hist.undo_depth(), 0);

    let bad = dropping_a_track(&mcp, t, false);
    let cmd = resize_col(&mcp, t, props(&mcp, t).cols[1].id, 240.0).unwrap();
    let good = LivePatch::new(live_ops_for_command(&mcp, &cmd).unwrap());
    cmd.apply(&mut mcp).unwrap();
    apply_live_patches(&mut app, &[bad, good]);
    same(&mcp, &app, t);
}
