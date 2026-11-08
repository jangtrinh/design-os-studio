//! Second fix round: one gate for "the canvas has the keyboard", the "+" at every zoom, a cell has no
//! stroke section, multi-selection and aspect-locked resizes keep the tracks in step, and a table
//! removed from a Hug frame shrinks it.

use super::table_edit::{assert_same_doc, at, select_table, snapshot, table_sim, text_of_cell};
use super::*;
use crate::app::RailItem;
use crate::board::table::{grips, TableAction, TableGeo};
use designos_doc::NodeKind;

fn click_cell(s: &mut Sim, table: NodeId, x: f32, y: f32) -> NodeId {
    let p = at(s, x, y);
    s.t += 1.0;
    s.click(p);
    s.t += 1.0;
    s.click(p);
    let cell = s.app().doc.get(table).unwrap().children[0];
    assert_eq!(s.app().selection, vec![cell], "the click selects the first cell");
    cell
}

#[test]
fn tab_in_the_agent_terminal_leaves_the_cell_selection() {
    let (mut s, table) = table_sim(false);
    let c0 = click_cell(&mut s, table, 80.0, 22.0);
    s.use_stub_agent_clis();
    s.shell.app.ui.rail = Some(RailItem::Agents);
    s.shell.app.ui.left_collapsed = false;
    s.shell.app.terminal.side_panel_pty_open = true;
    s.shell.app.terminal.viewport_focused = true;
    s.frame(vec![]);
    s.frame(vec![]);
    let open = !s.app().ui.left_hidden();
    assert!(s.app().terminal.is_capturing_keyboard(open), "the terminal holds the keyboard");
    s.key(Key::Tab);
    assert_eq!(s.app().selection, vec![c0], "Tab belongs to the terminal");
}

#[test]
fn tab_with_the_cell_menu_open_leaves_the_cell_selection() {
    let (mut s, table) = table_sim(false);
    let c0 = click_cell(&mut s, table, 80.0, 22.0);
    let p = at(&s, 80.0, 22.0);
    s.right_click(p);
    assert!(s.app().ui.ctx_menu.is_some());
    s.key(Key::Tab);
    assert_eq!(s.app().selection, vec![c0]);
}

#[test]
fn tab_with_the_variables_view_open_leaves_the_cell_selection() {
    let (mut s, table) = table_sim(false);
    let c0 = click_cell(&mut s, table, 80.0, 22.0);
    s.shell.app.vars.open = true;
    s.frame(vec![]);
    s.frame(vec![]);
    s.key(Key::Tab);
    assert_eq!(s.app().selection, vec![c0]);
}

#[test]
fn tab_during_the_eyedropper_cycles_the_format_and_never_replays() {
    let (mut s, table) = table_sim(false);
    let c0 = click_cell(&mut s, table, 80.0, 22.0);
    s.shell.app.props.eyedropper = Some(crate::panels::properties::EyedropTarget::Fill(0));
    s.frame(vec![]);
    s.frame(vec![]);
    let format = s.app().props.eyedropper_format.label();
    s.key(Key::Tab);
    s.key(Key::Tab);
    assert_ne!(s.app().props.eyedropper_format.label(), format, "the eyedropper's own Tab cycles its format");
    assert_eq!(s.app().selection, vec![c0], "the cell selection did not move");
    s.shell.app.props.eyedropper = None;
    s.frame(vec![]);
    s.frame(vec![]);
    s.frame(vec![]);
    assert_eq!(s.app().selection, vec![c0], "nothing replays once the eyedropper is closed");
}

fn set_text(s: &mut Sim, cell: NodeId, text: &str) {
    s.shell.app.edit_nodes(&[cell], |n| {
        if let NodeKind::TableCell(c) = &mut n.kind {
            c.text.content = text.into();
        }
    });
}

#[test]
fn typing_after_a_click_on_a_greyed_field_never_changes_the_cell() {
    let (mut s, table) = table_sim(false);
    let c0 = s.app().doc.get(table).unwrap().children[0];
    set_text(&mut s, c0, "Price");
    s.shell.app.selection = vec![c0];
    s.frame(vec![]);
    s.frame(vec![]);
    let depth = s.app().hist.undo_depth();
    let r = s.field_rect("w");
    s.click(Pos2::new(r.left() + 40.0, r.center().y));
    s.type_text("200");
    s.key(Key::Enter);
    s.key(Key::Delete);
    s.key(Key::Escape);
    assert_eq!(text_of_cell(&s, c0), "Price", "keys typed at the inspector never reach the cell");
    assert_eq!(s.app().hist.undo_depth(), depth, "and add no undo step");
    // a click back on the canvas gives the keyboard back
    click_cell(&mut s, table, 80.0, 22.0);
    s.type_text("z");
    s.key(Key::Escape);
    assert_eq!(text_of_cell(&s, c0), "z");
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
fn the_plus_is_live_wherever_it_is_drawn_at_every_zoom() {
    for zoom in [1.0_f32, 0.5, 0.25] {
        let (mut s, table) = table_sim(false);
        set_zoom(&mut s, zoom);
        select_table(&mut s, table);
        let geo = TableGeo::of(&s.app().doc, table).unwrap();
        let tl = grips::screen(s.app(), &geo, 0.0, 0.0);
        let line_y = grips::screen(s.app(), &geo, 0.0, geo.row_y[1]).y;
        let near = Pos2::new(tl.x - grips::GRIP_GAP, line_y);
        let plus = grips::areas(s.app(), &geo, near).into_iter().find(|(h, _)| matches!(h, grips::Hover::Plus { rows: true, index: 1 }));
        let rows = geo.props.rows.len();
        match plus {
            Some((_, area)) => {
                assert!(area.width() >= 24.0 && area.height() >= 24.0, "zoom {zoom}: plus target {area:?}");
                let c = area.center();
                assert!(
                    matches!(grips::hover_at(s.app(), c).map(|h| h.1), Some(grips::Hover::Plus { rows: true, index: 1 })),
                    "zoom {zoom}: drawn plus is live"
                );
                s.mv(c);
                s.button(true);
                s.button(false);
                s.frame(vec![]);
                assert_eq!(TableGeo::of(&s.app().doc, table).unwrap().props.rows.len(), rows + 1, "zoom {zoom}: the plus inserts a row");
            }
            None => {
                // not drawn: a press there selects the row through the grip
                s.mv(near);
                s.button(true);
                s.button(false);
                s.frame(vec![]);
                assert_eq!(TableGeo::of(&s.app().doc, table).unwrap().props.rows.len(), rows, "zoom {zoom}: nothing inserted");
            }
        }
        if zoom == 1.0 {
            assert!(plus.is_some(), "the plus is drawn at 100%");
        }
    }
}

/// Every string painted in the last frame.
fn painted_text(s: &Sim) -> String {
    fn walk(shape: &egui::epaint::Shape, out: &mut String) {
        match shape {
            egui::epaint::Shape::Text(t) => {
                out.push_str(t.galley.text());
                out.push('\n');
            }
            egui::epaint::Shape::Vec(v) => v.iter().for_each(|x| walk(x, out)),
            _ => {}
        }
    }
    let mut out = String::new();
    s.shapes.iter().for_each(|c| walk(&c.shape, &mut out));
    out
}

#[test]
fn a_cell_selection_hides_the_stroke_section_but_keeps_fill() {
    // the control: a selected table paints both section titles, so the words below are the right ones
    let (mut s, table) = table_sim(false);
    select_table(&mut s, table);
    let text = painted_text(&s);
    assert!(text.contains("Stroke") && text.contains("Fill"), "a table has Fill and Stroke");
    let (mut s, table) = table_sim(false);
    let cell = s.app().doc.get(table).unwrap().children[0];
    s.shell.app.selection = vec![cell];
    s.frame(vec![]);
    s.frame(vec![]);
    let text = painted_text(&s);
    assert!(text.contains("Fill"), "a cell keeps its Fill");
    assert!(!text.contains("Stroke"), "cell strokes are never drawn, so there is no Stroke section");
}

fn place_rectangle(s: &mut Sim, x: f32, y: f32, w: f32, h: f32) -> NodeId {
    let page = s.app().page;
    let id = s.shell.app.doc.alloc_id();
    let n = designos_doc::Node::rectangle(id, designos_doc::Rect::new(x, y, w, h));
    s.shell.app.hist.exec_user(&mut s.shell.app.doc, designos_doc::Command::Insert { parent: page, index: usize::MAX, nodes: vec![n] }).unwrap();
    s.shell.app.canvas.dirty = true;
    s.shell.app.changed();
    s.frame(vec![]);
    id
}

fn tracks_in_step(s: &Sim, table: NodeId) -> bool {
    let g = TableGeo::of(&s.app().doc, table).unwrap();
    let n = s.app().doc.get(table).unwrap();
    let (cols, rows): (f32, f32) = (g.props.cols.iter().map(|c| c.size).sum(), g.props.rows.iter().map(|r| r.size).sum());
    (n.size.x - cols).abs() < 1.0 && (n.size.y - rows).abs() < 1.0 && designos_doc::table::relayout(&s.app().doc, table).is_empty()
}

#[test]
fn the_inspector_width_of_a_table_and_another_layer_keeps_the_tracks_in_step() {
    let (mut s, table) = table_sim(false);
    let other = place_rectangle(&mut s, 5600.0, 5000.0, 100.0, 100.0);
    s.shell.app.selection = vec![table, other];
    s.frame(vec![]);
    s.frame(vec![]);
    let r = s.field_rect("w");
    s.click(Pos2::new(r.left() + 40.0, r.center().y));
    s.cmd_key(Key::A);
    s.type_text("600");
    s.key(Key::Enter);
    assert_eq!(s.app().doc.get(table).unwrap().size.x.round(), 600.0);
    assert!(tracks_in_step(&s, table), "the columns follow the table");
}

#[test]
fn a_corner_drag_over_a_table_and_another_layer_keeps_the_tracks_in_step() {
    let (mut s, table) = table_sim(false);
    let other = place_rectangle(&mut s, 5500.0, 5000.0, 100.0, 132.0);
    s.shell.app.selection = vec![table, other];
    s.frame(vec![]);
    s.frame(vec![]);
    let depth = s.app().hist.undo_depth();
    let before = snapshot(&s);
    let (from, to) = (at(&s, 600.0, 132.0), at(&s, 720.0, 132.0));
    s.mv(from);
    s.button(true);
    for i in 1..=6 {
        let t = i as f32 / 6.0;
        s.mv(from + (to - from) * t);
    }
    s.button(false);
    s.frame(vec![]);
    assert_eq!(s.app().hist.undo_depth(), depth + 1, "one step");
    assert!(s.app().doc.get(table).unwrap().size.x > 500.0, "the table was scaled");
    assert!(tracks_in_step(&s, table), "tracks and cells follow the table");
    s.cmd_key(Key::Z);
    assert_same_doc(&s, &before);
}

#[test]
fn the_tables_width_field_honours_lock_aspect_ratio() {
    let (mut s, table) = table_sim(false);
    s.shell.app.edit_nodes(&[table], |n| n.lock_aspect = true);
    select_table(&mut s, table);
    let r = s.field_rect("w");
    s.click(Pos2::new(r.left() + 40.0, r.center().y));
    s.cmd_key(Key::A);
    s.type_text("600");
    s.key(Key::Enter);
    let n = s.app().doc.get(table).unwrap();
    assert_eq!((n.size.x.round(), n.size.y.round()), (600.0, 165.0), "the height followed the width");
    assert!(tracks_in_step(&s, table));
}

fn hug_frame_around(s: &mut Sim, table: NodeId) -> NodeId {
    select_table(s, table);
    let ctx = s.ctx.clone();
    crate::panels::menus::run(&mut s.shell.app, &ctx, crate::panels::menus::Action::AutoLayout);
    s.frame(vec![]);
    s.app().doc.get(table).unwrap().parent.unwrap()
}

#[test]
fn deleting_every_row_of_a_table_in_a_hug_frame_shrinks_the_frame() {
    let (mut s, table) = table_sim(false);
    let frame = hug_frame_around(&mut s, table);
    let h0 = s.app().doc.get(frame).unwrap().size.y;
    let geo = TableGeo::of(&s.app().doc, table).unwrap();
    s.shell.app.selection = geo.cells_in((0, 0, 2, 2));
    let depth = s.app().hist.undo_depth();
    assert!(crate::board::table::run(&mut s.shell.app, TableAction::DeleteRow));
    assert!(s.app().doc.get(table).is_none(), "deleting every row removes the table");
    assert_eq!(s.app().hist.undo_depth(), depth + 1);
    assert!(s.app().doc.get(frame).unwrap().size.y < h0 - 40.0, "the frame hugs what is left");
    s.cmd_key(Key::Z);
    assert_eq!(s.app().doc.get(frame).unwrap().size.y, h0, "one undo gives the frame and the table back");
}

#[test]
fn a_row_resize_starts_from_the_grown_height() {
    let (mut s, table) = table_sim(false);
    let cell = s.app().doc.get(table).unwrap().children[0];
    set_text(&mut s, cell, "one two three four five six seven eight nine ten eleven twelve thirteen");
    s.shell.app.selection = vec![cell];
    for fix in designos_doc::table::relayout(&s.app().doc, table) {
        fix.apply(&mut s.shell.app.doc).unwrap();
    }
    s.shell.app.canvas.dirty = true;
    s.shell.app.changed();
    select_table(&mut s, table);
    let geo = TableGeo::of(&s.app().doc, table).unwrap();
    let grown = geo.row_y[1] - geo.row_y[0];
    assert!(grown > 60.0, "the row grew to {grown}");
    let (depth, line) = (s.app().hist.undo_depth(), geo.row_y[1]);
    let x = 80.0;
    let from = at(&s, x, line);
    s.mv(from);
    s.button(true);
    s.mv(from + egui::vec2(0.0, 20.0));
    s.mv(from + egui::vec2(0.0, 40.0));
    s.button(false);
    s.frame(vec![]);
    assert_eq!(s.app().hist.undo_depth(), depth + 1);
    let g = TableGeo::of(&s.app().doc, table).unwrap();
    assert!((g.props.rows[0].size - (grown + 40.0)).abs() < 1.5, "the drag started at {grown}, the row is {}", g.props.rows[0].size);
}
