//! Fixed row tracks of a grid that has no explicit row count survive a `.fig` round trip.
//!
//! With `grid_rows == 0` the solver still gives a Fixed row slot its height, and sizes the other
//! rows from the auto-track. A `.fig` has `gridRows` / `gridRowsSizing` as its only home for row
//! tracks, so the export has to write the Fixed slots as explicit rows.

use designos_doc::*;
use designos_fig::{export_fig, import_fig};

/// A one-column grid of three 20 px rows. Row 0 has a Fixed(30) slot and the auto-track is Fixed(50).
fn grid_document(grid_rows: u32, row_tracks: &[(usize, GridTrack)], auto: GridAutoTracks) -> Document {
    let mut d = Document::new("fixed-row-export");
    let page = d.add_page("p");
    let fid = d.alloc_id();
    let mut frame = Node::frame(fid, Rect::new(0.0, 0.0, 100.0, 10.0));
    frame.name = "RowGrid".into();
    let mut al = AutoLayout {
        direction: LayoutDirection::Grid,
        grid_columns: 1,
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
    for i in 0..3 {
        let id = d.alloc_id();
        let mut r = Node::rectangle(id, Rect::new(0.0, 0.0, 40.0, 20.0));
        r.name = format!("Item{i}");
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
