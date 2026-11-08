//! Third fix round: the canvas-keyboard gate built on the shared overlay gate, Delete and Tab with the gate
//! closed, the "+" next to the rulers, a line press that barely moves, and a reason on the greyed rows.

use super::table_edit::{at, select_table, table_sim, text_of_cell};
use super::*;
use crate::board::table::{grips, TableGeo};
use designos_doc::NodeKind;

fn select_first_cell(s: &mut Sim, table: NodeId) -> NodeId {
    let c0 = s.app().doc.get(table).unwrap().children[0];
    let p = at(s, 80.0, 22.0);
    s.t += 1.0;
    s.click(p);
    s.t += 1.0;
    s.click(p);
    assert_eq!(s.app().selection, vec![c0]);
    c0
}

fn set_text(s: &mut Sim, cell: NodeId, text: &str) {
    s.shell.app.edit_nodes(&[cell], |n| {
        if let NodeKind::TableCell(c) = &mut n.kind {
            c.text.content = text.into();
        }
    });
    s.frame(vec![]);
}

fn cmd_alt_j(s: &mut Sim) {
    let m = Modifiers { command: true, mac_cmd: true, alt: true, ..Default::default() };
    s.mods = m;
    s.frame(vec![Event::Key { key: Key::J, physical_key: Some(Key::J), pressed: true, repeat: false, modifiers: m }]);
    s.mods = Modifiers::NONE;
    s.frame(vec![]);
}

fn click_greyed_width_field(s: &mut Sim) {
    let r = s.field_rect("w");
    s.click(Pos2::new(r.left() + 40.0, r.center().y));
}

#[test]
fn delete_and_tab_with_the_gate_closed_do_nothing_at_all() {
    let (mut s, table) = table_sim(true);
    let c0 = select_first_cell(&mut s, table);
    set_text(&mut s, c0, "Price");
    let depth = s.app().hist.undo_depth();
    click_greyed_width_field(&mut s);
    s.shell.app.toast = None;
    s.shell.app.toast_queue.clear();
    s.key(Key::Delete);
    s.key(Key::Backspace);
    assert!(s.app().toast.is_none(), "no error toast: {:?}", s.app().toast.as_ref().map(|t| &t.text));
    assert_eq!(s.app().selection, vec![c0], "the selection stays");
    assert_eq!(text_of_cell(&s, c0), "Price");
    s.key(Key::Tab);
    assert_eq!(s.app().selection, vec![c0], "Tab stays with egui focus, it does not walk siblings");
    assert_eq!(s.app().hist.undo_depth(), depth);
}

#[test]
fn a_cells_keys_work_after_the_canvas_takes_focus_back() {
    for key in ["type", "enter", "delete"] {
        let (mut s, table) = table_sim(false);
        crate::agent_a11y::enable_accesskit(&s.ctx);
        let c0 = select_first_cell(&mut s, table);
        set_text(&mut s, c0, "Price");
        cmd_alt_j(&mut s);
        cmd_alt_j(&mut s);
        assert_eq!(s.ctx.memory(|m| m.focused()), Some(crate::agent_a11y::canvas_id()), "the canvas holds egui focus");
        let depth = s.app().hist.undo_depth();
        match key {
            "type" => {
                s.type_text("z");
                assert!(s.app().canvas.text.is_some(), "typing opens the edit");
                s.key(Key::Escape);
                assert_eq!(text_of_cell(&s, c0), "z");
            }
            "enter" => {
                s.key(Key::Enter);
                assert_eq!(s.app().canvas.text.as_ref().map(|t| t.id), Some(c0), "Enter opens the edit");
            }
            _ => {
                s.shell.app.toast = None;
                s.key(Key::Delete);
                assert_eq!(text_of_cell(&s, c0), "", "Delete clears the cell");
                assert_eq!(s.app().hist.undo_depth(), depth + 1);
                assert!(s.app().toast.is_none());
            }
        }
    }
}

#[test]
fn typing_after_a_click_in_the_fill_picker_never_reaches_the_cell() {
    let (mut s, table) = table_sim(false);
    let c0 = s.app().doc.get(table).unwrap().children[0];
    set_text(&mut s, c0, "Price");
    s.shell.app.edit_nodes(&[c0], |n| n.fills = vec![designos_doc::Paint::solid(designos_doc::Color::BLACK)]);
    select_first_cell(&mut s, table);
    s.click(s.rect_of("sw:false:0").center());
    s.frame(vec![]);
    let picker = s.rect_of("picker");
    assert!(s.app().props.picker.is_some(), "the picker is open");
    let p = Pos2::new(picker.left() + 30.0, picker.top() + 90.0);
    assert!(s.app().canvas_rect.contains(p), "the picker floats over the canvas rect");
    s.click(p);
    let depth = s.app().hist.undo_depth();
    s.type_text("7");
    s.key(Key::Escape);
    assert_eq!(text_of_cell(&s, c0), "Price", "a key typed with the picker in use never reaches the cell");
    assert_eq!(s.app().hist.undo_depth(), depth);
}

#[test]
fn tab_and_typing_with_the_libraries_modal_open_do_nothing_to_the_cell() {
    let (mut s, table) = table_sim(false);
    let c0 = select_first_cell(&mut s, table);
    set_text(&mut s, c0, "Price");
    s.shell.app.libs.open = true;
    s.frame(vec![]);
    s.frame(vec![]);
    s.key(Key::Tab);
    assert_eq!(s.app().selection, vec![c0], "Tab did not move the cell behind the modal");
    s.type_text("q");
    assert!(s.app().canvas.text.is_none(), "typing opened no edit behind the modal");
    assert_eq!(text_of_cell(&s, c0), "Price");
}

#[test]
fn a_plus_next_to_the_ruler_is_live_or_not_drawn() {
    for gap in [65.0_f32, 50.0, 40.0] {
        let (mut s, table) = table_sim(false);
        {
            let app = &mut s.shell.app;
            app.ui.rulers = true;
            app.view.offset = designos_doc::Vec2::new(5000.0 - gap, 5000.0 - 200.0);
            app.canvas.dirty = true;
            app.changed();
        }
        s.frame(vec![]);
        select_table(&mut s, table);
        let geo = TableGeo::of(&s.app().doc, table).unwrap();
        let tl = grips::screen(s.app(), &geo, 0.0, 0.0);
        let line_y = grips::screen(s.app(), &geo, 0.0, geo.row_y[1]).y;
        let aim = Pos2::new(tl.x - grips::GRIP_GAP - grips::GRIP_AREA, line_y);
        let painted = grips::areas(s.app(), &geo, aim).into_iter().find(|(h, _)| matches!(h, grips::Hover::Plus { rows: true, index: 1 }));
        let rows = geo.props.rows.len();
        if let Some((_, r)) = painted {
            s.mv(r.center());
            s.button(true);
            s.button(false);
            s.frame(vec![]);
            assert_eq!(TableGeo::of(&s.app().doc, table).unwrap().props.rows.len(), rows + 1, "gap {gap}: a drawn plus inserts a row");
        }
    }
}

#[test]
fn a_line_press_with_one_pixel_of_movement_stores_nothing() {
    for zoom in [1.0_f32, 0.25] {
        let (mut s, table) = table_sim(false);
        let cell = s.app().doc.get(table).unwrap().children[0];
        set_text(&mut s, cell, "one two three four five six seven eight nine ten eleven twelve thirteen");
        for fix in designos_doc::table::relayout(&s.app().doc, table) {
            fix.apply(&mut s.shell.app.doc).unwrap();
        }
        {
            let app = &mut s.shell.app;
            app.view.zoom = zoom;
            app.view.offset = designos_doc::Vec2::new(5000.0 - 200.0 / zoom, 5000.0 - 200.0 / zoom);
            app.canvas.dirty = true;
            app.changed();
        }
        s.frame(vec![]);
        select_table(&mut s, table);
        let geo = TableGeo::of(&s.app().doc, table).unwrap();
        let p = grips::screen(s.app(), &geo, 80.0, geo.row_y[1]);
        assert!(matches!(grips::hover_at(s.app(), p).map(|h| h.1), Some(grips::Hover::Line { rows: true, index: 0 })));
        let (depth, stored) = (s.app().hist.undo_depth(), geo.props.rows[0].size);
        s.mv(p);
        s.button(true);
        s.mv(p + egui::vec2(0.0, 1.0));
        s.button(false);
        s.frame(vec![]);
        assert_eq!(s.app().hist.undo_depth(), depth, "zoom {zoom}: a click is not a resize");
        assert_eq!(TableGeo::of(&s.app().doc, table).unwrap().props.rows[0].size, stored);
    }
}

#[test]
fn the_greyed_geometry_rows_say_why() {
    let (mut s, table) = table_sim(false);
    let cell = s.app().doc.get(table).unwrap().children[0];
    s.shell.app.selection = vec![cell];
    s.frame(vec![]);
    s.frame(vec![]);
    let r = s.field_rect("w");
    s.mv(Pos2::new(r.left() + 40.0, r.center().y));
    for _ in 0..40 {
        s.frame(vec![]);
    }
    fn walk(shape: &egui::epaint::Shape, out: &mut String) {
        match shape {
            egui::epaint::Shape::Text(t) => out.push_str(t.galley.text()),
            egui::epaint::Shape::Vec(v) => v.iter().for_each(|x| walk(x, out)),
            _ => {}
        }
    }
    let mut text = String::new();
    s.shapes.iter().for_each(|c| walk(&c.shape, &mut text));
    assert!(text.contains("Resize rows or columns"), "the hover reason is shown: {text:?}");
}
