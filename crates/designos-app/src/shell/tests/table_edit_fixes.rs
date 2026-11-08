//! Fix round for table editing: typing from a selected cell, Space-pan, read-only cell geometry, the 24 px
//! grips at every zoom, rotated tables, silent no-ops, a table in a Hug frame, and the commit-time check.

use super::table_edit::{at, select_table, snapshot, table_sim, table_sim_in, text_of_cell};
use super::*;
use crate::board::table::{grips, TableAction, TableGeo};
use designos_doc::{Affine, Command, NodeKind};

fn first_cell(s: &Sim, table: NodeId) -> NodeId {
    s.app().doc.get(table).unwrap().children[0]
}

fn set_text(s: &mut Sim, cell: NodeId, text: &str) {
    s.shell.app.edit_nodes(&[cell], |n| {
        if let NodeKind::TableCell(c) = &mut n.kind {
            c.text.content = text.into();
        }
    });
}

fn select_cell(s: &mut Sim, cell: NodeId) {
    s.shell.app.selection = vec![cell];
    s.frame(vec![]);
    s.frame(vec![]);
}

#[test]
fn typing_on_a_selected_cell_inserts_each_character_once() {
    let (mut s, table) = table_sim(false);
    let cell = first_cell(&s, table);
    select_cell(&mut s, cell);
    s.type_text("a");
    s.key(Key::Escape);
    assert_eq!(text_of_cell(&s, cell), "a");
}

#[test]
fn space_while_panning_never_edits_a_selected_cell() {
    let (mut s, table) = table_sim(false);
    let cell = first_cell(&s, table);
    set_text(&mut s, cell, "x");
    select_cell(&mut s, cell);
    let depth = s.app().hist.undo_depth();
    let space = Event::Key { key: Key::Space, physical_key: Some(Key::Space), pressed: true, repeat: false, modifiers: Modifiers::NONE };
    s.frame(vec![space, Event::Text(" ".into())]);
    s.frame(vec![Event::Text(" ".into())]);
    s.key(Key::Escape);
    assert!(s.app().canvas.text.is_none());
    assert_eq!(text_of_cell(&s, cell), "x", "Space pans, it does not replace the text");
    assert_eq!(s.app().hist.undo_depth(), depth);
}

fn type_into_field(s: &mut Sim, key: &str, text: &str) {
    let r = s.field_rect(key);
    s.click(Pos2::new(r.left() + 40.0, r.center().y));
    s.type_text(text);
    s.key(Key::Enter);
}

#[test]
fn a_selected_cells_geometry_fields_are_read_only() {
    let (mut s, table) = table_sim(false);
    let cell = first_cell(&s, table);
    select_cell(&mut s, cell);
    let before = snapshot(&s);
    for key in ["x", "y", "rot", "w", "h"] {
        type_into_field(&mut s, key, "999");
    }
    assert!(designos_doc::table::relayout(&s.app().doc, table).is_empty(), "the table is still laid out as its tracks say");
    let now = s.app().doc.get(cell).unwrap();
    let was = &before.iter().find(|(id, _)| *id == cell).unwrap().1;
    // (typing digits after a click on a disabled field is typing on the selected cell, which may grow its row)
    assert_eq!(now.transform, was.transform, "the cell did not move");
}

#[test]
fn the_table_width_field_scales_the_tracks() {
    let (mut s, table) = table_sim(false);
    select_table(&mut s, table);
    let (depth, before) = (s.app().hist.undo_depth(), snapshot(&s));
    type_into_field(&mut s, "w", "600");
    assert_eq!(s.app().hist.undo_depth(), depth + 1, "one step");
    let n = s.app().doc.get(table).unwrap();
    assert_eq!(n.size.x.round(), 600.0);
    let g = TableGeo::of(&s.app().doc, table).unwrap();
    assert!(g.props.cols.iter().all(|c| (c.size - 200.0).abs() < 1.0), "every column scaled");
    assert!(designos_doc::table::relayout(&s.app().doc, table).is_empty(), "cells match the tracks");
    s.cmd_key(Key::Z);
    super::table_edit::assert_same_doc(&s, &before);
}

fn set_zoom(s: &mut Sim, zoom: f32) {
    let app = &mut s.shell.app;
    app.view.zoom = zoom;
    app.view.offset = designos_doc::Vec2::new(5000.0 - 200.0 / zoom, 5000.0 - 200.0 / zoom);
    app.canvas.dirty = true;
    app.changed();
    s.frame(vec![]);
    s.frame(vec![]);
}

#[test]
fn grips_keep_a_24_px_area_and_never_insert_a_row_at_any_zoom() {
    for zoom in [1.0_f32, 0.5, 0.25] {
        let (mut s, table) = table_sim(false);
        set_zoom(&mut s, zoom);
        select_table(&mut s, table);
        let geo = TableGeo::of(&s.app().doc, table).unwrap();
        let tl = grips::screen(s.app(), &geo, 0.0, 0.0);
        // the visible grip of row 2, drawn at the left edge centred on the row
        let row_mid = (geo.row_y[1] + geo.row_y[2]) / 2.0;
        let grip = Pos2::new(tl.x - grips::GRIP_GAP, grips::screen(s.app(), &geo, 0.0, row_mid).y);
        for dy in [-4.0_f32, 0.0, 4.0] {
            let p = Pos2::new(grip.x, grip.y + dy);
            let areas = grips::areas(s.app(), &geo, p);
            let hit = grips::hover_at(s.app(), p).expect("something is under the grip").1;
            assert!(matches!(hit, grips::Hover::Grip { rows: true, index: 1 }), "zoom {zoom}, dy {dy}: {hit:?}");
            let (_, area) = areas.iter().find(|(h, _)| matches!(h, grips::Hover::Grip { rows: true, .. })).unwrap();
            assert!(area.width() >= 24.0 && area.height() >= 24.0, "zoom {zoom}: grip area {area:?}");
        }
        let (depth, rows) = (s.app().hist.undo_depth(), geo.props.rows.len());
        s.mv(grip);
        s.button(true);
        s.button(false);
        s.frame(vec![]);
        let g = TableGeo::of(&s.app().doc, table).unwrap();
        assert_eq!(g.props.rows.len(), rows, "zoom {zoom}: a press on the grip inserts nothing");
        assert_eq!(s.app().hist.undo_depth(), depth);
    }
}

#[test]
fn a_press_that_ends_a_text_edit_never_inserts_a_row() {
    let (mut s, table) = table_sim(false);
    let geo = TableGeo::of(&s.app().doc, table).unwrap();
    let cell = first_cell(&s, table);
    crate::canvas::interact::begin_text_edit(&mut s.shell.app, cell);
    s.frame(vec![]);
    let rows = geo.props.rows.len();
    let depth = s.app().hist.undo_depth();
    // just left of the table, on the line between rows 1 and 2, where the hidden "+" would be
    let p = at(&s, -16.0, 44.0);
    s.click(p);
    let g = TableGeo::of(&s.app().doc, table).unwrap();
    assert_eq!(g.props.rows.len(), rows, "the invisible plus did not act");
    assert_eq!(s.app().hist.undo_depth(), depth);
}

#[test]
fn a_rotated_table_shows_no_grips_and_no_lines() {
    let (mut s, table) = table_sim(false);
    s.shell.app.edit_nodes(&[table], |n| {
        let c = designos_doc::Vec2::new(n.size.x / 2.0, n.size.y / 2.0);
        n.transform = n.transform * Affine::translate(c.x, c.y) * Affine::rotate(0.5) * Affine::translate(-c.x, -c.y);
    });
    select_table(&mut s, table);
    let geo = TableGeo::of(&s.app().doc, table).unwrap();
    let mid = grips::screen(s.app(), &geo, 240.0, 66.0);
    assert!(grips::areas(s.app(), &geo, mid).is_empty(), "no chrome on a rotated table");
    assert!(grips::hover_at(s.app(), mid).is_none());
}

#[test]
fn delete_on_an_empty_cell_is_a_silent_no_op() {
    let (mut s, table) = table_sim(false);
    let cell = first_cell(&s, table);
    select_cell(&mut s, cell);
    s.shell.app.toast = None;
    s.shell.app.toast_queue.clear();
    let depth = s.app().hist.undo_depth();
    s.key(Key::Delete);
    assert_eq!(s.app().hist.undo_depth(), depth, "no undo step");
    assert!(s.app().toast.is_none() && s.app().toast_queue.is_empty(), "no toast");
    // a merge of one cell and a resize to the same size are no-ops too
    assert!(!crate::board::table::run(&mut s.shell.app, TableAction::MergeCells));
    assert!(s.app().toast.is_none());
}

#[test]
fn a_table_in_a_hug_frame_grows_its_parent_in_the_same_step() {
    let (mut s, table) = table_sim(false);
    select_table(&mut s, table);
    let ctx = s.ctx.clone();
    crate::panels::menus::run(&mut s.shell.app, &ctx, crate::panels::menus::Action::AutoLayout);
    s.frame(vec![]);
    let frame = s.app().doc.get(table).unwrap().parent.unwrap();
    assert!(matches!(s.app().doc.get(frame).map(|n| &n.kind), Some(NodeKind::Frame(_))), "the table has a frame around it");
    let gap = |s: &Sim| s.app().doc.get(frame).unwrap().size.y - s.app().doc.get(table).unwrap().size.y;
    let gap0 = gap(&s);
    let (depth, h0) = (s.app().hist.undo_depth(), s.app().doc.get(frame).unwrap().size.y);
    let geo = TableGeo::of(&s.app().doc, table).unwrap();
    s.shell.app.selection = vec![geo.cell_covering(2, 0).unwrap().id];
    assert!(crate::board::table::run(&mut s.shell.app, TableAction::InsertRowBelow));
    assert_eq!(s.app().hist.undo_depth(), depth + 1);
    assert!((gap(&s) - gap0).abs() < 0.5, "the frame hugs the taller table");
    assert!(s.app().doc.get(frame).unwrap().size.y > h0 + 40.0);
    s.cmd_key(Key::Z);
    assert_eq!(s.app().doc.get(frame).unwrap().size.y, h0, "one undo gives the frame back");
}

#[test]
fn a_table_op_applied_one_command_at_a_time_is_checked_at_the_end() {
    let (mut s, table) = table_sim(false);
    let geo = TableGeo::of(&s.app().doc, table).unwrap();
    let cmd = designos_doc::table_ops::delete_rows(&s.app().doc, table, &[geo.row_id(1)]).unwrap();
    let Command::Batch(parts) = cmd else { panic!("a batch") };
    let mut tx = crate::edit::Tx::new();
    for p in parts {
        tx.cmd(&mut s.shell.app.doc, p).expect("the in-between states are not refused");
    }
    let depth = s.app().hist.undo_depth();
    assert!(s.shell.app.run_tx(tx));
    assert_eq!(s.app().hist.undo_depth(), depth + 1);
    assert_eq!(TableGeo::of(&s.app().doc, table).unwrap().props.rows.len(), 2);
    // a transaction that ends broken is refused whole
    let before = snapshot(&s);
    let cells = s.app().doc.get(table).unwrap().children.clone();
    let mut tx = crate::edit::Tx::new();
    tx.cmd(&mut s.shell.app.doc, Command::Remove { id: cells[0] }).unwrap();
    let depth = s.app().hist.undo_depth();
    assert!(!s.shell.app.run_tx(tx), "the commit refuses a table left without a cell");
    assert_eq!(s.app().hist.undo_depth(), depth);
    super::table_edit::assert_same_doc(&s, &before);
}

fn tab_through_a_table_in(label: &str, opts: Options) {
    let (mut s, table) = table_sim_in(opts, false);
    let stickies = |s: &Sim| s.app().doc.nodes.values().filter(|n| matches!(n.kind, NodeKind::Sticky(_))).count();
    let before = stickies(&s);
    let order: Vec<NodeId> = s.app().doc.get(table).unwrap().children.clone();
    // a click table -> cell, as a person does (the click also takes the keyboard from whatever held it)
    let first = at(&s, 80.0, 22.0);
    s.click(first);
    s.t += 1.0;
    s.click(first);
    assert_eq!(s.app().selection, vec![order[0]]);
    s.key(Key::Tab);
    assert_eq!(s.app().selection, vec![order[1]], "{label}: Tab on a selected cell selects the next cell");
    assert_eq!(stickies(&s), before, "and starts no sticky note");
    s.key(Key::Enter);
    assert_eq!(s.app().canvas.text.as_ref().map(|t| t.id), Some(order[1]));
    s.type_text("a");
    s.key(Key::Tab);
    assert_eq!(s.app().canvas.text.as_ref().map(|t| t.id), Some(order[2]), "{label}: Tab in a cell edit moves to the next cell");
    assert_eq!(stickies(&s), before);
}

#[test]
fn tab_in_a_table_cell_creates_no_sticky_and_moves_on_in_a_board_and_a_design() {
    tab_through_a_table_in("design", Options::default());
    tab_through_a_table_in("board", Options { fixture: Some("board-empty".into()), ..Default::default() });
}
