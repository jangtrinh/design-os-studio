//! Dragging on a selected table: track lines, grips and the corner handle. Each drag is one undo step.

use super::table_edit::{assert_same_doc, at, select_table, snapshot, table_sim};
use super::*;
use crate::board::table::TableGeo;
use designos_doc::NodeKind;

fn drag_steps(s: &mut Sim, from: (f32, f32), to: (f32, f32)) {
    let p = at(s, from.0, from.1);
    s.mv(p);
    s.button(true);
    for i in 1..=6 {
        let t = i as f32 / 6.0;
        let q = at(s, from.0 + (to.0 - from.0) * t, from.1 + (to.1 - from.1) * t);
        s.mv(q);
    }
    s.button(false);
    s.frame(vec![]);
}

#[test]
fn a_column_line_drag_resizes_the_column_in_one_step() {
    let (mut s, table) = table_sim(false);
    select_table(&mut s, table);
    let (depth, before) = (s.app().hist.undo_depth(), snapshot(&s));
    drag_steps(&mut s, (160.0, 22.0), (200.0, 22.0));
    assert_eq!(s.app().hist.undo_depth(), depth + 1);
    let geo = TableGeo::of(&s.app().doc, table).unwrap();
    assert!((geo.props.cols[0].size - 200.0).abs() < 1.0, "column 1 is {}", geo.props.cols[0].size);
    s.cmd_key(Key::Z);
    assert_same_doc(&s, &before);
}

#[test]
fn a_grip_drag_reorders_a_row_and_a_column_each_in_one_step() {
    for rows in [true, false] {
        let (mut s, table) = table_sim(false);
        select_table(&mut s, table);
        let ids0: Vec<_> = {
            let g = TableGeo::of(&s.app().doc, table).unwrap();
            (if rows { &g.props.rows } else { &g.props.cols }).iter().map(|t| t.id).collect()
        };
        let (depth, before) = (s.app().hist.undo_depth(), snapshot(&s));
        // the grip of row / column 2, dropped over row / column 3
        if rows {
            drag_steps(&mut s, (-16.0, 66.0), (-16.0, 110.0));
        } else {
            drag_steps(&mut s, (240.0, -16.0), (400.0, -16.0));
        }
        assert_eq!(s.app().hist.undo_depth(), depth + 1, "one step (rows: {rows})");
        let g = TableGeo::of(&s.app().doc, table).unwrap();
        let ids1: Vec<_> = (if rows { &g.props.rows } else { &g.props.cols }).iter().map(|t| t.id).collect();
        assert_eq!(ids1, vec![ids0[0], ids0[2], ids0[1]], "the track moved (rows: {rows})");
        s.cmd_key(Key::Z);
        assert_same_doc(&s, &before);
    }
}

#[test]
fn the_corner_handle_scales_every_track_and_keeps_the_text_size() {
    let (mut s, table) = table_sim(false);
    let cell = s.app().doc.get(table).unwrap().children[0];
    let font = |s: &Sim| match &s.app().doc.get(cell).unwrap().kind {
        NodeKind::TableCell(c) => c.text.style.font_size,
        _ => 0.0,
    };
    let (f0, depth, before) = (font(&s), s.app().hist.undo_depth(), snapshot(&s));
    let origin = s.app().doc.get(table).unwrap().transform;
    select_table(&mut s, table);
    drag_steps(&mut s, (480.0, 132.0), (600.0, 176.0));
    assert_eq!(s.app().hist.undo_depth(), depth + 1, "one step");
    let n = s.app().doc.get(table).unwrap();
    assert_eq!((n.size.x.round(), n.size.y.round()), (600.0, 176.0));
    assert_eq!(n.transform, origin, "the opposite corner stays put");
    let g = TableGeo::of(&s.app().doc, table).unwrap();
    assert!(g.props.cols.iter().all(|c| (c.size - 200.0).abs() < 1.0));
    assert!(g.props.rows.iter().all(|r| (r.size - 176.0 / 3.0).abs() < 1.0));
    assert_eq!(font(&s), f0, "text keeps its size");
    s.cmd_key(Key::Z);
    assert_same_doc(&s, &before);
}

#[test]
fn a_big_table_adds_one_layer_row_and_takes_only_before_and_after_drops() {
    use crate::panels::layers::{drop_for, flatten, DropKind};
    let mut s = Sim::new();
    let page = s.app().page;
    let count = |s: &Sim| flatten(&s.app().doc, s.app().page, &s.app().layers.expanded).len();
    let before = count(&s);
    let cmd = designos_doc::table_ops::create_table(&s.app().doc, page, designos_doc::Vec2::new(5000.0, 5000.0), 50, 50, &designos_doc::TableStyle::default())
        .unwrap();
    s.shell.app.hist.exec_user(&mut s.shell.app.doc, cmd).unwrap();
    assert_eq!(count(&s), before + 1, "2,500 cells add exactly one row");
    let rows = flatten(&s.app().doc, s.app().page, &s.app().layers.expanded);
    let i = rows.iter().position(|r| matches!(s.app().doc.get(r.id).map(|n| &n.kind), Some(NodeKind::Table(_)))).unwrap();
    let h = crate::theme::LAYER_ROW_HEIGHT;
    for frac in [0.1_f32, 0.3, 0.5, 0.7, 0.9] {
        let d = drop_for((i as f32 + frac) * h, &rows, &s.app().doc, &[]).unwrap();
        assert_ne!(d.kind, DropKind::Inside, "a layer dropped on the table row goes before or after it (at {frac})");
    }
}
