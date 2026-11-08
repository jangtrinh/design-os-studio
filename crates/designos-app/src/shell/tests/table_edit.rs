//! Editing a table on the canvas: cells, Tab, rows and columns, merge, undo, IME, Layers.

use super::*;
use crate::board::table::{self, TableAction, TableGeo};
use designos_doc::table_ops;
use designos_doc::{NodeKind, TableStyle, Vec2 as DVec};

const ORIGIN: f32 = 5000.0;

pub(super) fn text_of_cell(s: &Sim, id: NodeId) -> String {
    match &s.app().doc.get(id).unwrap().kind {
        NodeKind::TableCell(c) => c.text.content.clone(),
        _ => panic!("not a cell"),
    }
}

/// A 3x3 table on the page of the demo file, far from its content, with the top-left 2x2 merged.
pub(super) fn table_sim(merged: bool) -> (Sim, NodeId) {
    table_sim_in(Options::default(), merged)
}

/// [`table_sim`] in a file opened with `opts` (a board, for instance).
pub(super) fn table_sim_in(opts: Options, merged: bool) -> (Sim, NodeId) {
    let mut s = Sim::with(opts);
    {
        let app = &mut s.shell.app;
        let page = app.page;
        let cmd = table_ops::create_table(&app.doc, page, DVec::new(ORIGIN, ORIGIN), 3, 3, &TableStyle::default()).unwrap();
        app.hist.exec_user(&mut app.doc, cmd).unwrap();
    }
    let table = *s.app().doc.get(s.app().page).unwrap().children.last().unwrap();
    if merged {
        let app = &mut s.shell.app;
        let cmd = table_ops::merge(&app.doc, table, (0, 0, 1, 1)).unwrap();
        app.hist.exec_user(&mut app.doc, cmd).unwrap();
    }
    {
        let app = &mut s.shell.app;
        app.view.zoom = 1.0;
        app.view.offset = DVec::new(ORIGIN - 200.0, ORIGIN - 200.0);
        app.canvas.dirty = true;
        app.changed();
    }
    s.frame(vec![]);
    s.frame(vec![]);
    (s, table)
}

pub(super) fn at(s: &Sim, x: f32, y: f32) -> Pos2 {
    s.screen(ORIGIN + x, ORIGIN + y)
}

fn double_click_at(s: &mut Sim, x: f32, y: f32) {
    let p = at(s, x, y);
    s.t += 1.0;
    s.click(p);
    s.click(p);
}

fn cells_in_tab_order(s: &Sim, table: NodeId) -> Vec<NodeId> {
    let mut v = Vec::new();
    let mut id = s.app().doc.get(table).unwrap().children[0];
    v.push(id);
    while let Some(n) = table_ops::next_cell(&s.app().doc, table, id, false) {
        v.push(n);
        id = n;
    }
    v
}

fn tab(s: &mut Sim, shift: bool) {
    s.mods = if shift { Modifiers::SHIFT } else { Modifiers::NONE };
    s.key(Key::Tab);
    s.mods = Modifiers::NONE;
}

pub(super) fn snapshot(s: &Sim) -> Vec<(NodeId, designos_doc::Node)> {
    s.app().doc.nodes.iter().map(|(k, v)| (*k, v.clone())).collect()
}

pub(super) fn assert_same_doc(s: &Sim, before: &[(NodeId, designos_doc::Node)]) {
    let now = snapshot(s);
    assert_eq!(now.len(), before.len(), "node count");
    for ((ka, na), (kb, nb)) in now.iter().zip(before) {
        assert_eq!(ka, kb);
        assert_eq!(na.kind, nb.kind, "kind of {ka:?}");
        assert_eq!((na.transform, na.size, &na.children), (nb.transform, nb.size, &nb.children), "geometry of {ka:?}");
    }
}

#[test]
fn tab_walks_the_cells_in_order_and_shift_tab_goes_back() {
    let (mut s, table) = table_sim(true);
    let order = cells_in_tab_order(&s, table);
    assert_eq!(order.len(), 6, "a 3x3 table with a 2x2 merge has six live cells");
    double_click_at(&mut s, 80.0, 44.0);
    assert_eq!(s.app().canvas.text.as_ref().map(|t| t.id), Some(order[0]), "double-click edits the merged anchor");
    s.type_text("a");
    for letter in ["b", "c", "d", "e", "f"] {
        tab(&mut s, false);
        s.type_text(letter);
    }
    s.key(Key::Escape);
    for (i, want) in ["a", "b", "c", "d", "e", "f"].iter().enumerate() {
        assert_eq!(text_of_cell(&s, order[i]), *want, "cell {i}");
    }
    double_click_at(&mut s, 80.0, 44.0 * 2.0 + 22.0);
    assert_eq!(s.app().canvas.text.as_ref().map(|t| t.id), Some(order[3]), "double-click edits the first cell of the last row");
    tab(&mut s, true);
    assert_eq!(s.app().canvas.text.as_ref().map(|t| t.id), Some(order[2]), "Shift+Tab returns to the cell before");
}

#[test]
fn tab_at_the_last_cell_adds_a_row_in_one_undo_step() {
    let (mut s, table) = table_sim(false);
    let order = cells_in_tab_order(&s, table);
    let last = *order.last().unwrap();
    double_click_at(&mut s, 160.0 * 2.0 + 80.0, 44.0 * 2.0 + 22.0);
    assert_eq!(s.app().canvas.text.as_ref().map(|t| t.id), Some(last));
    s.type_text("z");
    let depth = s.app().hist.undo_depth();
    tab(&mut s, false);
    assert_eq!(s.app().hist.undo_depth(), depth + 1, "the text and the new row are one step");
    let geo = TableGeo::of(&s.app().doc, table).unwrap();
    assert_eq!(geo.props.rows.len(), 4);
    let first_new = geo.cell_covering(3, 0).unwrap().id;
    assert_eq!(s.app().canvas.text.as_ref().map(|t| t.id), Some(first_new), "the first cell of the new row is being edited");
    s.key(Key::Escape);
    s.cmd_key(Key::Z);
    let geo = TableGeo::of(&s.app().doc, table).unwrap();
    assert_eq!(geo.props.rows.len(), 3);
    assert_eq!(text_of_cell(&s, last), "", "one undo takes back the row and the text");
}

#[test]
fn text_that_grows_a_row_undoes_with_the_row_height() {
    let (mut s, table) = table_sim(false);
    let cell = s.app().doc.get(table).unwrap().children[0];
    let h0 = s.app().doc.get(cell).unwrap().size.y;
    double_click_at(&mut s, 80.0, 22.0);
    s.type_text("one two three four five six seven eight nine ten eleven twelve");
    s.key(Key::Escape);
    let h1 = s.app().doc.get(cell).unwrap().size.y;
    assert!(h1 > h0, "a row grows to fit its text: {h0} -> {h1}");
    s.cmd_key(Key::Z);
    assert_eq!(s.app().doc.get(cell).unwrap().size.y, h0, "the height comes back with the text");
    assert_eq!(text_of_cell(&s, cell), "");
}

#[test]
fn ime_composition_in_a_cell_commits_and_tab_waits_for_it() {
    let (mut s, table) = table_sim(false);
    let order = cells_in_tab_order(&s, table);
    double_click_at(&mut s, 80.0, 22.0);
    s.frame(vec![Event::Ime(egui::ImeEvent::Preedit { text: "việ".into(), active_range_chars: None })]);
    tab(&mut s, false);
    assert_eq!(s.app().canvas.text.as_ref().map(|t| t.id), Some(order[0]), "Tab does not move cells during a composition");
    s.frame(vec![Event::Ime(egui::ImeEvent::Commit("việt".into()))]);
    s.key(Key::Escape);
    assert_eq!(text_of_cell(&s, order[0]), "việt");
}

#[test]
fn click_levels_table_then_cell_then_block_and_escape_steps_up() {
    let (mut s, table) = table_sim(false);
    s.click(at(&s, 240.0, 66.0));
    assert_eq!(s.app().selection, vec![table], "the first click selects the table");
    s.t += 1.0;
    s.click(at(&s, 240.0, 66.0));
    let geo = TableGeo::of(&s.app().doc, table).unwrap();
    assert_eq!(s.app().selection, vec![geo.cell_covering(1, 1).unwrap().id], "a click on the selected table selects the cell");
    s.mods = Modifiers::SHIFT;
    s.click(at(&s, 400.0, 110.0));
    s.mods = Modifiers::NONE;
    assert_eq!(s.app().selection.len(), 4, "Shift+click selects the block");
    s.key(Key::Escape);
    assert_eq!(s.app().selection, vec![table], "Esc goes up to the table");
}

#[test]
fn every_structural_action_is_one_undo_step_and_one_undo_restores() {
    let actions = [
        TableAction::InsertRowAbove,
        TableAction::InsertRowBelow,
        TableAction::InsertColLeft,
        TableAction::InsertColRight,
        TableAction::DeleteRow,
        TableAction::DeleteCol,
        TableAction::MergeCells,
    ];
    for a in actions {
        let (mut s, table) = table_sim(false);
        let geo = TableGeo::of(&s.app().doc, table).unwrap();
        let (c1, c2) = (geo.cell_covering(1, 1).unwrap().id, geo.cell_covering(2, 2).unwrap().id);
        s.shell.app.selection = if a == TableAction::MergeCells { geo.cells_in((1, 1, 2, 2)) } else { vec![c1] };
        let _ = c2;
        let before = snapshot(&s);
        let depth = s.app().hist.undo_depth();
        assert!(table::run(&mut s.shell.app, a), "{a:?} acts");
        assert_eq!(s.app().hist.undo_depth(), depth + 1, "{a:?} is one step");
        assert!(s.app().hist.undo_label().is_some(), "{a:?} names its step");
        s.cmd_key(Key::Z);
        assert_same_doc(&s, &before);
    }
}

#[test]
fn unmerge_and_a_refused_merge() {
    let (mut s, table) = table_sim(true);
    let geo = TableGeo::of(&s.app().doc, table).unwrap();
    let anchor = geo.cell_covering(0, 0).unwrap().id;
    s.shell.app.selection = vec![anchor];
    let before = snapshot(&s);
    let depth = s.app().hist.undo_depth();
    assert!(table::run(&mut s.shell.app, TableAction::UnmergeCells));
    assert_eq!(s.app().hist.undo_depth(), depth + 1);
    s.cmd_key(Key::Z);
    assert_same_doc(&s, &before);
    // a block that cuts into the merge is refused, and the menu entry says why
    let geo = TableGeo::of(&s.app().doc, table).unwrap();
    s.shell.app.selection = vec![geo.cell_covering(1, 2).unwrap().id, geo.cell_covering(2, 2).unwrap().id, geo.cell_covering(2, 1).unwrap().id];
    let entries = table::menu::entries(s.app());
    assert_eq!(entries.len(), 9);
    let (merge_item, _) = entries.iter().find(|(_, a)| *a == TableAction::MergeCells).unwrap();
    assert!(table::plan(s.app(), TableAction::MergeCells).is_err(), "a block that cuts a merge is refused");
    assert!(merge_item.disabled, "the entry is disabled, not dead");
    assert!(!merge_item.tooltip.is_empty(), "and says why");
}

#[test]
fn deleting_a_row_with_text_through_the_menu_undoes_and_redoes() {
    let (mut s, table) = table_sim(false);
    let geo = TableGeo::of(&s.app().doc, table).unwrap();
    let row2: Vec<NodeId> = geo.cells_in((1, 0, 1, 2));
    for (i, id) in row2.iter().enumerate() {
        s.shell.app.edit_nodes(&[*id], |n| {
            if let NodeKind::TableCell(c) = &mut n.kind {
                c.text.content = format!("t{i}");
            }
        });
    }
    let texts: Vec<String> = row2.iter().map(|id| text_of_cell(&s, *id)).collect();
    s.shell.app.selection = row2.clone();
    s.right_click(at(&s, 240.0, 66.0));
    assert!(s.app().ui.ctx_menu.is_some(), "the menu opens on a cell selection");
    assert_eq!(crate::panels::context::labels(s.app()).len(), 9);
    let before_nodes = s.app().doc.nodes.len();
    // the entry the menu runs
    let ctx = s.ctx.clone();
    crate::panels::menus::run(&mut s.shell.app, &ctx, crate::panels::menus::Action::Table(TableAction::DeleteRow));
    s.shell.app.ui.ctx_menu = None;
    s.frame(vec![]);
    assert_eq!(TableGeo::of(&s.app().doc, table).unwrap().props.rows.len(), 2);
    s.cmd_key(Key::Z);
    assert_eq!(s.app().doc.nodes.len(), before_nodes);
    assert_eq!(row2.iter().map(|id| text_of_cell(&s, *id)).collect::<Vec<_>>(), texts, "texts and ids are back");
    let is_mac = cfg!(target_os = "macos");
    s.mods = Modifiers { alt: false, ctrl: !is_mac, shift: true, mac_cmd: is_mac, command: true };
    s.key(Key::Z);
    s.mods = Modifiers::NONE;
    assert_eq!(TableGeo::of(&s.app().doc, table).unwrap().props.rows.len(), 2, "redo deletes it again");
}

#[test]
fn a_row_resize_drag_is_one_undo_step_with_many_moves() {
    let (mut s, table) = table_sim(false);
    s.click(at(&s, 240.0, 66.0));
    let depth = s.app().hist.undo_depth();
    let before = snapshot(&s);
    s.mv(at(&s, 240.0, 44.0));
    s.button(true);
    for i in 1..=8 {
        s.mv(at(&s, 240.0, 44.0 + 4.0 * i as f32));
        assert_eq!(s.app().hist.undo_depth(), depth, "nothing is committed while dragging");
    }
    s.button(false);
    s.frame(vec![]);
    assert_eq!(s.app().hist.undo_depth(), depth + 1, "one step on release");
    let geo = TableGeo::of(&s.app().doc, table).unwrap();
    assert!((geo.props.rows[0].size - 76.0).abs() < 1.0, "row 1 is {}", geo.props.rows[0].size);
    s.cmd_key(Key::Z);
    assert_same_doc(&s, &before);
}

#[test]
fn layers_shows_a_table_as_one_row_without_an_arrow() {
    let (mut s, table) = table_sim(false);
    let rows = crate::panels::layers::flatten(&s.app().doc, s.app().page, &s.app().layers.expanded);
    let row = rows.iter().find(|r| r.id == table).unwrap();
    assert!(!row.has_children, "no expand arrow");
    let mut expanded = s.app().layers.expanded.clone();
    expanded.insert(table);
    let rows2 = crate::panels::layers::flatten(&s.app().doc, s.app().page, &expanded);
    assert_eq!(rows.len(), rows2.len(), "an expanded table adds no cell rows");
    let geo = TableGeo::of(&s.app().doc, table).unwrap();
    s.shell.app.selection = vec![geo.cell_covering(1, 1).unwrap().id];
    s.frame(vec![]);
    s.frame(vec![]);
    assert!(!s.app().layers.expanded.contains(&table), "selecting a cell does not expand the table");
}

#[test]
fn layer_commands_with_a_cell_selected_leave_the_table_whole() {
    let (mut s, table) = table_sim(false);
    let geo = TableGeo::of(&s.app().doc, table).unwrap();
    let cell = geo.cell_covering(1, 1).unwrap().id;
    s.shell.app.selection = vec![cell];
    s.frame(vec![]);
    let before = snapshot(&s);
    for key in [Key::D, Key::G] {
        s.cmd_key(key);
        s.shell.app.selection = vec![cell];
    }
    s.mods = Modifiers { alt: true, ..Modifiers::NONE };
    s.cmd_key(Key::G);
    s.mods = Modifiers::NONE;
    s.shell.app.selection = vec![cell];
    for k in [Key::ArrowRight, Key::ArrowDown] {
        s.key(k);
    }
    let after = snapshot(&s);
    let t_before = before.iter().find(|(id, _)| *id == table).unwrap();
    let t_after = after.iter().find(|(id, _)| *id == table).unwrap();
    assert_eq!(t_before.1.kind, t_after.1.kind, "tracks unchanged");
    assert_eq!(t_before.1.children, t_after.1.children, "cells unchanged");
    assert!(selection_box_none(&s), "a cell selection shows no handles");
    s.shell.app.selection = vec![cell];
    s.frame(vec![]);
    s.frame(vec![]);
    assert!(s.app().board.dots.is_none(), "no connector dots on a cell");
}

fn selection_box_none(s: &Sim) -> bool {
    crate::canvas::interact::selection_box(s.app()).is_none()
}

pub(super) fn select_table(s: &mut Sim, table: NodeId) {
    s.shell.app.select_only(table);
    s.frame(vec![]);
    s.frame(vec![]);
}
