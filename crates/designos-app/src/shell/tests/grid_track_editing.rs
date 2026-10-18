//! Editing the tracks of a grid auto layout on the canvas and in the inspector: header pills,
//! `+` buttons, reorder, and what a track edit leaves untouched.

use super::*;
use crate::canvas::draw::{grid_track_add_button_rects, grid_track_pill_rects};
use crate::canvas::grids::{delete_grid_track, format_grid_track, insert_grid_track, move_grid_track, node_css_grid_overlay, resize_grid_track};
use crate::edit::Edit;
use designos_doc::{AutoLayout, GridTrack, LayoutDirection, Node, Rect as DRect, MAX_GRID_TRACKS};
use egui::{Event, Key, Modifiers, PointerButton};

/// A grid frame at page (320, 320) with one 60x40 rectangle per `(column, row)` pin, selected and drawn.
fn add_grid(s: &mut Sim, al: AutoLayout, pins: &[(Option<u16>, Option<u16>)]) -> (NodeId, Vec<NodeId>) {
    let page = s.app().page;
    let fid = s.shell.app.doc.alloc_id();
    let mut grid = Node::frame(fid, DRect::new(320.0, 320.0, 480.0, 240.0));
    grid.kind.frame_props_mut().unwrap().auto_layout = Some(AutoLayout { direction: LayoutDirection::Grid, ..al });
    let grid_id = s.shell.app.doc.insert(page, grid).unwrap();
    let mut kids = Vec::new();
    for &(column, row) in pins {
        let cid = s.shell.app.doc.alloc_id();
        let mut child = Node::rectangle(cid, DRect::new(0.0, 0.0, 60.0, 40.0));
        child.layout_child.grid_column = column;
        child.layout_child.grid_row = row;
        kids.push(s.shell.app.doc.insert(grid_id, child).unwrap());
    }
    let mut ed = Edit::new();
    crate::layout::relayout_around(&mut s.shell.app.doc, &mut ed, grid_id);
    s.shell.app.selection = vec![grid_id];
    s.frame(vec![]);
    s.frame(vec![]);
    (grid_id, kids)
}

fn layout_of(s: &Sim, id: NodeId) -> AutoLayout {
    s.app().doc.get(id).unwrap().kind.frame_props().unwrap().auto_layout.clone().unwrap()
}

/// The track labels the canvas pills show.
fn labels(s: &Sim, grid_id: NodeId, is_col: bool) -> Vec<String> {
    let ov = node_css_grid_overlay(&s.app().doc, grid_id).unwrap();
    let tracks = if is_col { ov.col_tracks } else { ov.row_tracks };
    tracks.into_iter().map(format_grid_track).collect()
}

fn column_pin(s: &Sim, id: NodeId) -> Option<u16> {
    s.app().doc.get(id).unwrap().layout_child.grid_column
}

fn row_pin(s: &Sim, id: NodeId) -> Option<u16> {
    s.app().doc.get(id).unwrap().layout_child.grid_row
}

/// The header pills of one axis, in track order.
fn pills(s: &Sim, grid_id: NodeId, is_col: bool) -> Vec<Rect> {
    let mut found: Vec<_> = grid_track_pill_rects(s.app(), s.app().canvas_rect).into_iter().filter(|&(g, c, _, _)| g == grid_id && c == is_col).collect();
    found.sort_by_key(|&(_, _, idx, _)| idx);
    found.into_iter().map(|(_, _, _, r)| r).collect()
}

fn add_button(s: &Sim, grid_id: NodeId, is_col: bool) -> Option<Rect> {
    grid_track_add_button_rects(s.app(), s.app().canvas_rect).into_iter().find(|&(g, c, _)| g == grid_id && c == is_col).map(|(_, _, r)| r)
}

fn three_columns(first: GridTrack, second: GridTrack, third: GridTrack) -> AutoLayout {
    let mut al = AutoLayout { grid_columns: 3, grid_rows: 1, ..AutoLayout::default() };
    al.grid_col_tracks[0] = first;
    al.grid_col_tracks[1] = second;
    al.grid_col_tracks[2] = third;
    al
}

/// 3 columns, `grid_rows` left at 0, four auto-placed children: the solver lays out 2 rows.
fn default_grid_with_two_rows(s: &mut Sim) -> NodeId {
    let al = AutoLayout { grid_columns: 3, ..AutoLayout::default() };
    let (grid_id, _) = add_grid(s, al, &[(None, None); 4]);
    assert_eq!(layout_of(s, grid_id).grid_rows, 0);
    assert_eq!(node_css_grid_overlay(&s.app().doc, grid_id).unwrap().rows, 2, "the canvas draws one pill per laid-out row");
    grid_id
}

fn set_row_track(s: &mut Sim, grid_id: NodeId, idx: usize, track: GridTrack) {
    let fp = s.shell.app.doc.get_mut(grid_id).unwrap().kind.frame_props_mut().unwrap();
    fp.auto_layout.as_mut().unwrap().grid_row_tracks[idx] = track;
}

#[test]
fn dragging_a_column_pill_reorders_it_without_cycling_its_sizing_and_undoes_in_one_step() {
    let mut s = Sim::new();
    let (grid_id, _) = add_grid(&mut s, three_columns(GridTrack::Fixed(80.0), GridTrack::Fixed(120.0), GridTrack::Flex(1.0)), &[(None, None); 3]);
    assert_eq!(labels(&s, grid_id, true), ["80px", "120px", "1fr"]);

    let p = pills(&s, grid_id, true);
    s.drag(p[0].center(), p[2].center());
    assert_eq!(labels(&s, grid_id, true), ["120px", "1fr", "80px"], "the 80px track moved and kept its size");

    s.shell.app.undo();
    assert_eq!(labels(&s, grid_id, true), ["80px", "120px", "1fr"], "one undo restores order and sizing");
}

#[test]
fn dragging_a_row_pill_of_a_default_grid_reorders_the_laid_out_rows() {
    let mut s = Sim::new();
    let grid_id = default_grid_with_two_rows(&mut s);
    set_row_track(&mut s, grid_id, 0, GridTrack::Fixed(90.0));
    s.frame(vec![]);
    assert_eq!(labels(&s, grid_id, false), ["90px", "1fr"]);

    let p = pills(&s, grid_id, false);
    s.drag(p[0].center(), p[1].center());
    assert_eq!(labels(&s, grid_id, false), ["1fr", "90px"], "the rows swapped; the drag did not cycle row 0 to Hug");

    s.shell.app.undo();
    assert_eq!(labels(&s, grid_id, false), ["90px", "1fr"]);
}

#[test]
fn clicking_a_track_pill_cycles_its_sizing_in_one_undo_step() {
    let mut s = Sim::new();
    let (grid_id, _) = add_grid(&mut s, three_columns(GridTrack::Flex(1.0), GridTrack::Flex(1.0), GridTrack::Flex(1.0)), &[(None, None); 3]);

    let p = pills(&s, grid_id, true);
    s.click(p[1].center());
    assert!(layout_of(&s, grid_id).grid_col_tracks[1] != GridTrack::Flex(1.0), "1fr cycled to a fixed size");
    assert_eq!(layout_of(&s, grid_id).grid_col_tracks[0], GridTrack::Flex(1.0));

    s.shell.app.undo();
    assert_eq!(labels(&s, grid_id, true), ["1fr", "1fr", "1fr"]);
}

#[test]
fn a_press_and_release_in_one_frame_is_a_click_and_cycles_once() {
    let mut s = Sim::new();
    let (grid_id, _) = add_grid(&mut s, three_columns(GridTrack::Flex(1.0), GridTrack::Flex(1.0), GridTrack::Flex(1.0)), &[(None, None); 3]);

    let at = pills(&s, grid_id, true)[2].center();
    s.pos = at;
    let button = |pressed| Event::PointerButton { pos: at, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE };
    s.frame(vec![Event::PointerMoved(at)]);
    s.frame(vec![button(true), button(false)]);
    assert_eq!(labels(&s, grid_id, true)[..2], ["1fr", "1fr"]);
    assert!(labels(&s, grid_id, true)[2].ends_with("px"), "1fr cycled to a fixed size once, not twice");
}

#[test]
fn space_held_while_dragging_across_pills_pans_instead_of_editing_tracks() {
    let mut s = Sim::new();
    let (grid_id, _) = add_grid(&mut s, three_columns(GridTrack::Fixed(80.0), GridTrack::Fixed(120.0), GridTrack::Flex(1.0)), &[(None, None); 3]);

    let key = |pressed| Event::Key { key: Key::Space, physical_key: Some(Key::Space), pressed, repeat: false, modifiers: Modifiers::NONE };
    s.frame(vec![key(true)]);
    let p = pills(&s, grid_id, true);
    s.drag(p[0].center(), p[2].center());
    s.frame(vec![key(false)]);
    assert_eq!(labels(&s, grid_id, true), ["80px", "120px", "1fr"]);
}

#[test]
fn row_plus_keeps_a_size_set_by_clicking_a_row_pill() {
    let mut s = Sim::new();
    let grid_id = default_grid_with_two_rows(&mut s);
    s.click(pills(&s, grid_id, false)[1].center());
    let sized = layout_of(&s, grid_id).grid_row_tracks[1];
    assert!(matches!(sized, GridTrack::Fixed(_)), "the row pill click fixed row 1: {sized:?}");

    s.click(add_button(&s, grid_id, false).expect("row + button").center());
    assert_eq!(layout_of(&s, grid_id).grid_row_tracks[1], sized);
    assert_eq!(node_css_grid_overlay(&s.app().doc, grid_id).unwrap().rows, 3);

    s.shell.app.undo();
    assert_eq!(node_css_grid_overlay(&s.app().doc, grid_id).unwrap().rows, 2, "one undo takes the row back");
    assert_eq!(layout_of(&s, grid_id).grid_row_tracks[1], sized, "and keeps the pill's size");
}

#[test]
fn track_edits_keep_the_slot_list_at_least_the_track_limit_and_never_lose_a_slot() {
    let mut s = Sim::new();
    let columns = 3;
    let mut al = AutoLayout { grid_columns: columns, ..AutoLayout::default() };
    al.grid_col_tracks[0] = GridTrack::Fixed(10.0);
    al.grid_col_tracks[1] = GridTrack::Fixed(20.0);
    al.grid_col_tracks[MAX_GRID_TRACKS - 1] = GridTrack::Fixed(77.0);
    let (grid_id, _) = add_grid(&mut s, al, &[(None, None)]);

    assert!(insert_grid_track(&mut s.shell.app, grid_id, true, 1, GridTrack::Fixed(15.0)));
    let after = layout_of(&s, grid_id);
    assert_eq!(after.grid_columns, 4);
    assert_eq!(&after.grid_col_tracks[..3], [GridTrack::Fixed(10.0), GridTrack::Fixed(15.0), GridTrack::Fixed(20.0)]);
    assert_eq!(after.grid_col_tracks[MAX_GRID_TRACKS], GridTrack::Fixed(77.0), "the slot past the limit shifted with the rest, it is not dropped");

    assert!(delete_grid_track(&mut s.shell.app, grid_id, true, 1));
    let back = layout_of(&s, grid_id);
    assert_eq!(back.grid_columns, 3);
    assert_eq!(back.grid_col_tracks[MAX_GRID_TRACKS - 1], GridTrack::Fixed(77.0));
    assert_eq!(back.grid_col_tracks.len(), MAX_GRID_TRACKS);

    // with nothing parked past the limit, an insert does not leave a growing tail of default slots
    let (plain, _) = add_grid(&mut s, AutoLayout { grid_columns: 3, ..AutoLayout::default() }, &[(None, None)]);
    assert!(insert_grid_track(&mut s.shell.app, plain, true, 0, GridTrack::Fixed(5.0)));
    assert_eq!(layout_of(&s, plain).grid_col_tracks.len(), MAX_GRID_TRACKS);
    assert!(delete_grid_track(&mut s.shell.app, plain, true, 0));
    assert_eq!(layout_of(&s, plain).grid_col_tracks.len(), MAX_GRID_TRACKS);
}

#[test]
fn only_the_add_buttons_a_click_can_reach_are_drawn() {
    let mut s = Sim::new();
    let (grid_id, _) = add_grid(&mut s, three_columns(GridTrack::Flex(1.0), GridTrack::Flex(1.0), GridTrack::Flex(1.0)), &[(None, None); 3]);
    let drawn_pluses = |s: &Sim| -> Vec<egui::Pos2> {
        s.shapes
            .iter()
            .filter_map(|clipped| match &clipped.shape {
                egui::Shape::Text(t) if t.galley.text() == "+" => Some(t.visual_bounding_rect().center()),
                _ => None,
            })
            .collect()
    };

    let reachable = grid_track_add_button_rects(s.app(), s.app().canvas_rect);
    assert_eq!(reachable.len(), 2);
    let pluses = drawn_pluses(&s);
    assert_eq!(pluses.len(), 2, "one + per axis is painted");
    for (_, _, r) in &reachable {
        assert!(pluses.iter().any(|p| r.contains(*p)), "a + is painted inside the clickable rect {r:?}");
    }

    // a selected child makes its parent grid draw its overlay, but a click cannot reach the parent's buttons
    let child = s.app().doc.get(grid_id).unwrap().children[0];
    s.shell.app.selection = vec![child];
    s.frame(vec![]);
    s.frame(vec![]);
    assert!(grid_track_add_button_rects(s.app(), s.app().canvas_rect).is_empty());
    assert!(drawn_pluses(&s).is_empty(), "no + is painted that a click would not reach");
}

#[test]
fn track_edits_leave_frames_that_are_not_grids_alone() {
    let mut s = Sim::new();
    let al = AutoLayout { direction: LayoutDirection::Vertical, grid_columns: 5, ..AutoLayout::default() };
    let (frame_id, _) = add_grid(&mut s, al.clone(), &[(None, None)]);
    s.shell.app.doc.get_mut(frame_id).unwrap().kind.frame_props_mut().unwrap().auto_layout = Some(al.clone());

    assert!(!insert_grid_track(&mut s.shell.app, frame_id, true, 0, GridTrack::Flex(1.0)));
    assert!(!delete_grid_track(&mut s.shell.app, frame_id, true, 0));
    assert!(!resize_grid_track(&mut s.shell.app, frame_id, true, 0, GridTrack::Hug));
    assert!(!move_grid_track(&mut s.shell.app, frame_id, true, 0, 1));
    assert_eq!(layout_of(&s, frame_id), al);
}

#[test]
fn a_pill_drag_that_ends_off_every_pill_changes_nothing() {
    let mut s = Sim::new();
    let (grid_id, _) = add_grid(&mut s, three_columns(GridTrack::Fixed(80.0), GridTrack::Fixed(120.0), GridTrack::Flex(1.0)), &[(None, None); 3]);

    let p = pills(&s, grid_id, true);
    let empty_canvas = s.screen(900.0, 700.0);
    s.drag(p[0].center(), empty_canvas);
    assert_eq!(labels(&s, grid_id, true), ["80px", "120px", "1fr"]);
}

#[test]
fn row_plus_on_a_default_grid_appends_a_row_and_keeps_the_sizes_set_on_other_rows() {
    let mut s = Sim::new();
    let grid_id = default_grid_with_two_rows(&mut s);
    set_row_track(&mut s, grid_id, 1, GridTrack::Fixed(80.0));
    s.frame(vec![]);

    let plus = add_button(&s, grid_id, false).expect("row + button");
    s.click(plus.center());

    let al = layout_of(&s, grid_id);
    assert_eq!(al.grid_rows, 3, "grid_rows follows the new laid-out row count");
    assert_eq!(node_css_grid_overlay(&s.app().doc, grid_id).unwrap().rows, 3, "a third row exists");
    assert_eq!(al.grid_row_tracks[1], GridTrack::Fixed(80.0), "the size set on row 1 survives");
    assert_eq!(labels(&s, grid_id, false), ["1fr", "80px", "1fr"]);
}

#[test]
fn moving_a_row_of_a_default_grid_uses_the_laid_out_rows() {
    let mut s = Sim::new();
    let grid_id = default_grid_with_two_rows(&mut s);
    set_row_track(&mut s, grid_id, 0, GridTrack::Fixed(90.0));

    assert!(move_grid_track(&mut s.shell.app, grid_id, false, 0, 1));
    assert_eq!(labels(&s, grid_id, false), ["1fr", "90px"]);
}

#[test]
fn resizing_a_row_keeps_the_fixed_height_of_a_row_past_the_explicit_rows() {
    let mut s = Sim::new();
    let al = AutoLayout { grid_columns: 3, grid_rows: 2, ..AutoLayout::default() };
    let (grid_id, _) = add_grid(&mut s, al, &[(Some(0), Some(3))]);
    set_row_track(&mut s, grid_id, 3, GridTrack::Fixed(80.0));

    assert!(resize_grid_track(&mut s.shell.app, grid_id, false, 0, GridTrack::Fixed(50.0)));
    let al = layout_of(&s, grid_id);
    assert_eq!(al.grid_row_tracks[0], GridTrack::Fixed(50.0));
    assert_eq!(al.grid_row_tracks[3], GridTrack::Fixed(80.0), "row 3 keeps its fixed height");
    assert_eq!(al.grid_rows, 2, "the explicit row count is not rewritten");
}

#[test]
fn resizing_a_column_keeps_the_column_count_and_every_other_slot() {
    let mut s = Sim::new();
    let mut al = AutoLayout { grid_columns: 12, ..AutoLayout::default() };
    for i in 0..12 {
        al.grid_col_tracks[i] = GridTrack::Fixed(10.0 + i as f32);
    }
    al.grid_col_tracks[20] = GridTrack::Fixed(33.0);
    let (grid_id, _) = add_grid(&mut s, al, &[(None, None)]);

    assert!(resize_grid_track(&mut s.shell.app, grid_id, true, 0, GridTrack::Fixed(99.0)));
    let after = layout_of(&s, grid_id);
    assert_eq!(after.grid_columns, 12);
    assert_eq!(after.grid_col_tracks[0], GridTrack::Fixed(99.0));
    for i in 1..12 {
        assert_eq!(after.grid_col_tracks[i], GridTrack::Fixed(10.0 + i as f32), "column {i}");
    }
    assert_eq!(after.grid_col_tracks[20], GridTrack::Fixed(33.0), "a size parked past the column count is not reset");
}

#[test]
fn resizing_a_column_of_a_grid_at_the_track_limit_keeps_every_column() {
    let mut s = Sim::new();
    let mut al = AutoLayout { grid_columns: MAX_GRID_TRACKS as u32, ..AutoLayout::default() };
    for i in 0..MAX_GRID_TRACKS {
        al.grid_col_tracks[i] = GridTrack::Fixed(10.0 + i as f32);
    }
    let (grid_id, _) = add_grid(&mut s, al, &[(None, None)]);

    assert!(resize_grid_track(&mut s.shell.app, grid_id, true, 0, GridTrack::Fixed(99.0)));
    let after = layout_of(&s, grid_id);
    assert_eq!(after.grid_columns, MAX_GRID_TRACKS as u32);
    for i in 1..MAX_GRID_TRACKS {
        assert_eq!(after.grid_col_tracks[i], GridTrack::Fixed(10.0 + i as f32), "column {i}");
    }
}

#[test]
fn resizing_a_track_does_not_write_the_default_column_count() {
    let mut s = Sim::new();
    let (grid_id, _) = add_grid(&mut s, AutoLayout::default(), &[(None, None)]);
    assert_eq!(layout_of(&s, grid_id).grid_columns, 0, "unset: the solver uses 3 columns");

    assert!(resize_grid_track(&mut s.shell.app, grid_id, true, 1, GridTrack::Fixed(70.0)));
    let after = layout_of(&s, grid_id);
    assert_eq!(after.grid_columns, 0);
    assert_eq!(after.grid_col_tracks[1], GridTrack::Fixed(70.0));
}

#[test]
fn inserting_a_row_shifts_every_pin_at_or_after_it_including_pins_past_the_track_limit() {
    let mut s = Sim::new();
    let al = AutoLayout { grid_columns: 3, grid_rows: 4, ..AutoLayout::default() };
    let last = MAX_GRID_TRACKS as u16 - 1;
    let (grid_id, kids) = add_grid(&mut s, al, &[(Some(0), Some(last - 1)), (Some(0), Some(last)), (Some(0), Some(100)), (Some(0), Some(1))]);

    assert!(insert_grid_track(&mut s.shell.app, grid_id, false, 0, GridTrack::Flex(1.0)));
    assert_eq!(row_pin(&s, kids[0]), Some(last), "pin just under the limit moves");
    assert_eq!(row_pin(&s, kids[1]), Some(last + 1), "pin at the limit moves, so it never shares a row with the one before");
    assert_eq!(row_pin(&s, kids[2]), Some(101), "pin past the limit moves");
    assert_eq!(row_pin(&s, kids[3]), Some(2));
    assert_eq!(layout_of(&s, grid_id).grid_rows, 5);
}

#[test]
fn appending_a_column_keeps_a_dormant_pin_dormant() {
    let mut s = Sim::new();
    let columns = MAX_GRID_TRACKS as u32 - 1;
    let al = AutoLayout { grid_columns: columns, ..AutoLayout::default() };
    // a pin on the first column that does not exist yet: it waits until the grid grows
    let (grid_id, kids) = add_grid(&mut s, al, &[(Some(columns as u16), None)]);

    assert!(insert_grid_track(&mut s.shell.app, grid_id, true, columns as usize, GridTrack::Flex(1.0)));
    assert_eq!(layout_of(&s, grid_id).grid_columns, MAX_GRID_TRACKS as u32);
    assert_eq!(column_pin(&s, kids[0]), Some(columns as u16 + 1), "the pin moved with the insert, so the child does not lock onto the new last column");
}

#[test]
fn deleting_and_moving_rows_remap_pins_past_the_track_limit() {
    let mut s = Sim::new();
    let al = AutoLayout { grid_columns: 3, grid_rows: 4, ..AutoLayout::default() };
    let (grid_id, kids) = add_grid(&mut s, al, &[(Some(0), Some(0)), (Some(0), Some(2)), (Some(0), Some(100))]);

    assert!(move_grid_track(&mut s.shell.app, grid_id, false, 0, 1));
    assert_eq!(row_pin(&s, kids[0]), Some(1));
    assert_eq!(row_pin(&s, kids[2]), Some(100), "a pin outside the moved range stays");

    assert!(delete_grid_track(&mut s.shell.app, grid_id, false, 0));
    assert_eq!(row_pin(&s, kids[0]), Some(0), "the pin that sat on row 1 follows its row up");
    assert_eq!(row_pin(&s, kids[1]), Some(1));
    assert_eq!(row_pin(&s, kids[2]), Some(99));
}

#[test]
fn canvas_add_buttons_meet_the_pointer_floor_and_vanish_at_the_track_limit() {
    let mut s = Sim::new();
    let (grid_id, _) = add_grid(&mut s, three_columns(GridTrack::Flex(1.0), GridTrack::Flex(1.0), GridTrack::Flex(1.0)), &[(None, None); 3]);
    for is_col in [true, false] {
        let r = add_button(&s, grid_id, is_col).expect("add button");
        assert!(r.width() >= 24.0 && r.height() >= 24.0, "add {} button is {}x{}", if is_col { "column" } else { "row" }, r.width(), r.height());
        assert!(s.app().canvas_rect.contains_rect(r), "the button sits inside the canvas");
        for pill in pills(&s, grid_id, true).into_iter().chain(pills(&s, grid_id, false)) {
            assert!(!r.intersects(pill), "the add button {r:?} overlaps the header pill {pill:?}");
        }
    }

    let full = AutoLayout { grid_columns: MAX_GRID_TRACKS as u32, grid_rows: MAX_GRID_TRACKS as u32, ..AutoLayout::default() };
    let (full_id, _) = add_grid(&mut s, full, &[]);
    assert!(add_button(&s, full_id, true).is_none());
    assert!(add_button(&s, full_id, false).is_none());
    assert!(!insert_grid_track(&mut s.shell.app, full_id, true, MAX_GRID_TRACKS, GridTrack::Flex(1.0)));
}

/// Scrolls the inspector until the field `key` is in the middle of the window and returns its rect.
fn inspector_field(s: &mut Sim, key: &str) -> Rect {
    s.frame(vec![]);
    let before = s.field_rect(key);
    let view = s.app().canvas_rect;
    s.shell.app.props.scroll += before.center().y - view.center().y;
    s.frame(vec![]);
    s.frame(vec![]);
    let r = s.field_rect(key);
    assert!(r.min.y > 80.0 && r.max.y < 1000.0, "field {key} is on screen at {r:?}");
    r
}

#[test]
fn the_inspector_value_field_does_not_pass_clicks_to_the_move_button() {
    let mut s = Sim::new();
    let (grid_id, _) = add_grid(&mut s, three_columns(GridTrack::Fixed(80.0), GridTrack::Fixed(120.0), GridTrack::Flex(1.0)), &[(None, None); 3]);
    let field = inspector_field(&mut s, "grid-col-px-0");

    s.click(Pos2::new(field.right() - 3.0, field.center().y));
    assert_eq!(labels(&s, grid_id, true), ["80px", "120px", "1fr"], "a click on the value field only edits the value");
}
