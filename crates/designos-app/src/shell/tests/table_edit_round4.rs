//! Fourth fix round: Tab in Present mode, Enter that opens an edit without typing a newline, and chrome
//! that the Layers panel's resize band never half covers.

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

#[test]
fn tab_in_present_mode_leaves_the_editors_cell_selection() {
    for via in ["shortcut", "start_present"] {
        let (mut s, table) = table_sim(false);
        let c0 = select_first_cell(&mut s, table);
        if via == "shortcut" {
            s.mods = Modifiers { command: true, mac_cmd: true, alt: true, ..Default::default() };
            s.key(Key::Enter);
            s.mods = Modifiers::NONE;
        } else {
            s.shell.app.start_present();
        }
        s.frame(vec![]);
        s.frame(vec![]);
        assert!(s.app().proto.present.is_some(), "{via}: the presentation is open");
        assert!(!crate::board::table::select::canvas_owns_keys(s.app(), &s.ctx), "{via}: the canvas does not own the keys while presenting");
        s.key(Key::Tab);
        s.key(Key::Tab);
        assert_eq!(s.app().selection, vec![c0], "{via}: Tab belongs to the prototype, not to the hidden cell selection");
    }
}

#[test]
fn enter_opens_the_edit_and_keeps_the_text_of_a_cell_and_of_a_text_layer() {
    // a cell
    let (mut s, table) = table_sim(false);
    let c0 = select_first_cell(&mut s, table);
    s.shell.app.edit_nodes(&[c0], |n| {
        if let NodeKind::TableCell(c) = &mut n.kind {
            c.text.content = "Price".into();
        }
    });
    s.frame(vec![]);
    let depth = s.app().hist.undo_depth();
    s.key(Key::Enter);
    assert_eq!(s.app().canvas.text.as_ref().map(|t| t.id), Some(c0), "Enter opens the edit");
    assert_eq!(text_of_cell(&s, c0), "Price", "and types nothing");
    s.key(Key::Escape);
    assert_eq!(text_of_cell(&s, c0), "Price");
    assert_eq!(s.app().hist.undo_depth(), depth, "Escape adds no undo step");

    // a text layer
    let (mut s, _table) = table_sim(false);
    let t = s.app().doc.nodes.values().find(|n| matches!(n.kind, NodeKind::Text(_))).map(|n| n.id).unwrap();
    s.shell.app.select_only(t);
    s.frame(vec![]);
    s.frame(vec![]);
    let content = |s: &Sim| s.app().doc.get(t).and_then(|n| n.kind.any_text().map(|x| x.content.clone())).unwrap();
    let (before, depth) = (content(&s), s.app().hist.undo_depth());
    s.key(Key::Enter);
    assert_eq!(s.app().canvas.text.as_ref().map(|x| x.id), Some(t), "Enter opens the edit");
    assert_eq!(content(&s), before, "and types nothing");
    s.key(Key::Escape);
    assert_eq!(content(&s), before);
    assert_eq!(s.app().hist.undo_depth(), depth, "Escape adds no undo step");
}

#[test]
fn with_rulers_off_no_drawn_control_reaches_under_the_panel_resize_band() {
    for gap in [28.0_f32, 32.0, 36.0, 52.0, 56.0, 60.0] {
        let (mut s, table) = table_sim(false);
        {
            let app = &mut s.shell.app;
            app.ui.rulers = false;
            app.view.offset = designos_doc::Vec2::new(5000.0 - gap, 5000.0 - 200.0);
            app.canvas.dirty = true;
            app.changed();
        }
        s.frame(vec![]);
        select_table(&mut s, table);
        assert!(!s.app().ui.left_hidden(), "the Layers panel is shown");
        let geo = TableGeo::of(&s.app().doc, table).unwrap();
        let tl = grips::screen(s.app(), &geo, 0.0, 0.0);
        let mid_y = grips::screen(s.app(), &geo, 0.0, geo.row_y[1]).y;
        let band = s.app().canvas_rect.left() + crate::theme::DRAG_DIVIDER_HIT * 0.5;
        for aim in [Pos2::new(tl.x - grips::GRIP_GAP, mid_y), Pos2::new(tl.x - grips::GRIP_GAP - grips::GRIP_AREA, mid_y)] {
            for (h, r) in grips::areas(s.app(), &geo, aim) {
                if matches!(h, grips::Hover::Grip { .. } | grips::Hover::Plus { .. }) {
                    assert!(r.left() >= band, "gap {gap}: drawn {h:?} box {r:?} reaches under the panel's resize band (to {band})");
                }
            }
        }
    }
}
