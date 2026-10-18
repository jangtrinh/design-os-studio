//! Fixed row tracks of a grid that has no explicit row count survive a `.fig` round trip.
//!
//! With `grid_rows == 0` the solver still gives a Fixed row slot its height, and sizes the other
//! rows from the auto-track. A `.fig` has `gridRows` / `gridRowsSizing` as its only home for row
//! tracks, so the export has to write the Fixed slots as explicit rows.

use designos_doc::*;
use designos_fig::{export_fig, export_rest_json, import_fig, import_rest_json};

/// A one-column grid of three 20 px rows. Row 0 has a Fixed(30) slot and the auto-track is Fixed(50).
fn grid_document(grid_rows: u32, row_tracks: &[(usize, GridTrack)], auto: GridAutoTracks) -> Document {
    grid_document_of(1, &[None; 3], grid_rows, row_tracks, auto)
}

/// A `columns`-wide grid with one 40 x 20 child per entry of `row_anchors` (its pinned row, if any).
fn grid_document_of(columns: u32, row_anchors: &[Option<u16>], grid_rows: u32, row_tracks: &[(usize, GridTrack)], auto: GridAutoTracks) -> Document {
    let mut d = Document::new("fixed-row-export");
    let page = d.add_page("p");
    let fid = d.alloc_id();
    let mut frame = Node::frame(fid, Rect::new(0.0, 0.0, 100.0, 10.0));
    frame.name = "RowGrid".into();
    let mut al = AutoLayout {
        direction: LayoutDirection::Grid,
        grid_columns: columns,
        grid_rows,
        item_spacing: 10.0,
        counter_spacing: 10.0,
        counter_sizing: Sizing::Hug,
        grid_auto_tracks: auto,
        ..Default::default()
    };
    for &(i, t) in row_tracks {
        al.grid_row_tracks[i] = t;
    }
    frame.kind = NodeKind::Frame(FrameProps { auto_layout: Some(al), ..Default::default() });
    let fid = d.insert(page, frame).unwrap();
    for (i, row) in row_anchors.iter().enumerate() {
        let id = d.alloc_id();
        let mut r = Node::rectangle(id, Rect::new(0.0, 0.0, 40.0, 20.0));
        r.name = format!("Item{i}");
        r.layout_child.grid_row = *row;
        d.insert(fid, r).unwrap();
    }
    d.relayout_subtree(fid);
    d
}

/// Row origins (children in order) and the frame height after a fresh solve.
fn solved(doc: &mut Document) -> (Vec<f32>, f32) {
    let fid = doc.nodes.values().find(|n| n.name == "RowGrid").expect("RowGrid").id;
    doc.relayout_subtree(fid);
    let ys = doc.get(fid).unwrap().children.iter().map(|c| doc.get(*c).unwrap().transform.ty).collect();
    (ys, doc.get(fid).unwrap().size.y)
}

fn round_trip(doc: &Document) -> Document {
    import_fig(&export_fig(doc).expect("export_fig")).expect("import_fig")
}

#[test]
fn fixed_row_slot_without_explicit_rows_keeps_its_height_through_fig() {
    let mut before = grid_document(0, &[(0, GridTrack::Fixed(30.0))], GridAutoTracks::Track(GridTrack::Fixed(50.0)));
    assert_eq!(solved(&mut before), (vec![0.0, 40.0, 100.0], 150.0), "row 0 is the Fixed(30) slot, the others take the 50 px auto-track");

    let mut after = round_trip(&before);
    assert_eq!(solved(&mut after), (vec![0.0, 40.0, 100.0], 150.0), "the Fixed(30) row survives export and import");
}

fn rest_round_trip(doc: &Document) -> Document {
    import_rest_json(&export_rest_json(doc).to_string()).expect("import_rest_json")
}

#[test]
fn fixed_row_slot_without_explicit_rows_keeps_its_height_through_rest_json() {
    let mut before = grid_document(0, &[(0, GridTrack::Fixed(30.0))], GridAutoTracks::Track(GridTrack::Fixed(50.0)));
    let expect = solved(&mut before);
    assert_eq!(expect, (vec![0.0, 40.0, 100.0], 150.0));
    assert_eq!(solved(&mut rest_round_trip(&before)), expect, "the Fixed(30) row and the 50 px auto-track survive the REST export and import");
}

#[test]
fn auto_track_of_explicit_rows_survives_the_rest_export() {
    // one explicit Fixed(30) row, seven children in three columns: the two overflow rows take the 50 px auto-track
    let mut before = grid_document_of(3, &[None; 7], 1, &[(0, GridTrack::Fixed(30.0))], GridAutoTracks::Track(GridTrack::Fixed(50.0)));
    let expect = solved(&mut before);
    assert_eq!(expect, (vec![0.0, 0.0, 0.0, 40.0, 40.0, 40.0, 100.0], 150.0));
    assert_eq!(solved(&mut rest_round_trip(&before)), expect);
    assert_eq!(solved(&mut round_trip(&before)), expect, "the .fig export already did");
}

#[test]
fn auto_track_modes_survive_the_rest_export() {
    for auto in [GridAutoTracks::Rows, GridAutoTracks::Track(GridTrack::Hug), GridAutoTracks::Track(GridTrack::Fixed(64.0)), GridAutoTracks::None] {
        let before = grid_document_of(3, &[None; 7], 1, &[], auto);
        let back = rest_round_trip(&before);
        let al = back.nodes.values().find(|n| n.name == "RowGrid").unwrap().kind.frame_props().unwrap().auto_layout.clone().unwrap();
        assert_eq!(al.grid_auto_tracks, auto);
    }
}

#[test]
fn fixed_row_slot_after_a_flex_slot_keeps_the_auto_track_height_for_the_slot_before_it() {
    // slot 0 is not Fixed, so with no explicit rows it takes the 50 px auto-track; slot 1 is Fixed(30)
    let mut before = grid_document(0, &[(1, GridTrack::Fixed(30.0))], GridAutoTracks::Track(GridTrack::Fixed(50.0)));
    assert_eq!(solved(&mut before), (vec![0.0, 60.0, 100.0], 150.0));

    let mut after = round_trip(&before);
    assert_eq!(solved(&mut after), (vec![0.0, 60.0, 100.0], 150.0));
}

#[test]
fn explicit_rows_and_plain_grids_are_unchanged() {
    let mut explicit = grid_document(1, &[(0, GridTrack::Fixed(30.0))], GridAutoTracks::Track(GridTrack::Fixed(50.0)));
    let expect = solved(&mut explicit);
    assert_eq!(expect, (vec![0.0, 40.0, 100.0], 150.0));
    assert_eq!(solved(&mut round_trip(&explicit)), expect);

    // no Fixed slot and no explicit rows: nothing to write, rows stay content-sized
    let mut plain = grid_document(0, &[], GridAutoTracks::None);
    let expect = solved(&mut plain);
    assert_eq!(expect, (vec![0.0, 30.0, 60.0], 80.0));
    let back = round_trip(&plain);
    let al = back.nodes.values().find(|n| n.name == "RowGrid").unwrap().kind.frame_props().unwrap().auto_layout.clone().unwrap();
    assert_eq!(al.grid_rows, 0, "a grid with no row tracks to keep stays without explicit rows");
    assert_eq!(solved(&mut round_trip(&plain)), expect);
}

/// Both exporters, each read back through its own importer.
fn both_round_trips(doc: &Document) -> [(&'static str, Document); 2] {
    [(".fig", round_trip(doc)), ("REST JSON", rest_round_trip(doc))]
}

fn explicit_rows(doc: &Document) -> u32 {
    doc.nodes.values().find(|n| n.name == "RowGrid").unwrap().kind.frame_props().unwrap().auto_layout.as_ref().unwrap().grid_rows
}

#[test]
fn a_fixed_slot_past_the_placed_rows_is_not_written_as_an_empty_row() {
    // three rows are placed; slot 3 is unused today and must not become a real row of 40 px plus a gap
    let mut before = grid_document(0, &[(3, GridTrack::Fixed(40.0))], GridAutoTracks::None);
    let expect = solved(&mut before);
    assert_eq!(expect, (vec![0.0, 30.0, 60.0], 80.0));
    for (format, mut after) in both_round_trips(&before) {
        assert_eq!(solved(&mut after), expect, "{format}");
        assert_eq!(explicit_rows(&after), 0, "{format}: no explicit rows are invented");
    }
}

#[test]
fn only_the_fixed_slots_inside_the_placed_rows_are_written() {
    // slot 1 is placed (3 rows), slot 5 is not: the export keeps rows 0..=1 and drops the rest
    let mut before = grid_document(0, &[(1, GridTrack::Fixed(30.0)), (5, GridTrack::Fixed(40.0))], GridAutoTracks::None);
    let expect = solved(&mut before);
    assert_eq!(expect, (vec![0.0, 30.0, 70.0], 90.0));
    for (format, mut after) in both_round_trips(&before) {
        assert_eq!(solved(&mut after), expect, "{format}");
        assert_eq!(explicit_rows(&after), 2, "{format}: rows 0 and 1 carry the Fixed slot");
    }
}

#[test]
fn a_fixed_slot_inside_the_placed_rows_keeps_working() {
    let mut before = grid_document(0, &[(2, GridTrack::Fixed(40.0))], GridAutoTracks::None);
    let expect = solved(&mut before);
    assert_eq!(expect, (vec![0.0, 30.0, 60.0], 100.0));
    for (format, mut after) in both_round_trips(&before) {
        assert_eq!(solved(&mut after), expect, "{format}");
    }
}

#[test]
fn rows_a_pinned_child_places_count_as_placed_rows() {
    // the last child is pinned to row 4, so five rows are placed even with three children
    let mut before = grid_document_of(1, &[None, None, Some(4)], 0, &[(3, GridTrack::Fixed(40.0))], GridAutoTracks::None);
    let expect = solved(&mut before);
    assert_eq!(expect, (vec![0.0, 30.0, 120.0], 140.0));
    for (format, mut after) in both_round_trips(&before) {
        assert_eq!(solved(&mut after), expect, "{format}");
    }
}

#[test]
fn an_absolute_child_does_not_place_a_row() {
    let mut doc = grid_document_of(1, &[None, None, None], 0, &[(2, GridTrack::Fixed(40.0))], GridAutoTracks::None);
    let fid = doc.nodes.values().find(|n| n.name == "RowGrid").unwrap().id;
    let last = *doc.get(fid).unwrap().children.last().unwrap();
    doc.get_mut(last).unwrap().layout_child.absolute = true;
    doc.relayout_subtree(fid);
    // two rows are placed: slot 2 is past them
    let expect = solved(&mut doc);
    for (format, mut after) in both_round_trips(&doc) {
        assert_eq!(solved(&mut after), expect, "{format}");
        assert_eq!(explicit_rows(&after), 0, "{format}");
    }
}

#[test]
fn a_flow_frame_never_gets_explicit_rows_from_stale_slots() {
    let mut doc = grid_document(0, &[(0, GridTrack::Fixed(30.0))], GridAutoTracks::None);
    let fid = doc.nodes.values().find(|n| n.name == "RowGrid").unwrap().id;
    if let Some(al) = doc.get_mut(fid).unwrap().kind.frame_props_mut().and_then(|f| f.auto_layout.as_mut()) {
        al.direction = LayoutDirection::Vertical;
    }
    for (format, after) in both_round_trips(&doc) {
        assert_eq!(explicit_rows(&after), 0, "{format}: a vertical frame has no grid rows to keep");
    }
}
