//! Regression tests for #531: exported CSS, Tailwind, and React grid placement must normalise
//! out-of-range column pins and wide spans identically to the grid layout solver (`Want::of`).

use designos_doc::*;
use designos_export::{css_for, react_for, react_with, tailwind_for, ReactOptions};

#[test]
fn out_of_range_column_pin_is_dropped_while_keeping_row_pin_and_matching_relayout_cell() {
    let mut d = Document::new("grid");
    let page = d.add_page("p");
    let gid = d.alloc_id();
    let mut g = Node::frame(gid, Rect::new(0.0, 0.0, 220.0, 100.0));
    g.fills.clear();
    let mut al = AutoLayout {
        direction: LayoutDirection::Grid,
        primary_sizing: Sizing::Fixed,
        counter_sizing: Sizing::Fixed,
        grid_columns: 2,
        grid_rows: 1,
        item_spacing: 20.0,
        ..Default::default()
    };
    al.grid_col_tracks[0] = GridTrack::Fixed(100.0);
    al.grid_col_tracks[1] = GridTrack::Fixed(100.0);
    al.grid_row_tracks[0] = GridTrack::Fixed(100.0);
    g.kind = NodeKind::Frame(FrameProps { auto_layout: Some(al), ..Default::default() });
    let g = d.insert(page, g).unwrap();

    let aid = d.alloc_id();
    let mut a = Node::rectangle(aid, Rect::new(0.0, 0.0, 40.0, 40.0));
    a.fills.clear();
    a.layout_child.grid_column = Some(0);
    a.layout_child.grid_row = Some(0);
    let a = d.insert(g, a).unwrap();

    let bid = d.alloc_id();
    let mut b = Node::rectangle(bid, Rect::new(0.0, 0.0, 40.0, 40.0));
    b.fills.clear();
    b.layout_child.grid_column = Some(5);
    b.layout_child.grid_row = Some(0);
    b.layout_child.col_span = 1;
    b.layout_child.row_span = 1;
    let b = d.insert(g, b).unwrap();

    d.relayout_subtree(g);
    assert_eq!(d.get(a).unwrap().transform.tx, 0.0);
    // Child B's out-of-range column 5 pin is dropped by the solver, so B lands on the free cell in row 0 (column 1, x = 120).
    assert_eq!((d.get(b).unwrap().transform.tx, d.get(b).unwrap().transform.ty), (120.0, 0.0));

    let css_b = css_for(&d, b);
    assert!(!css_b.contains("grid-column:"), "dropped column pin must not emit grid-column: {css_b}");
    assert!(css_b.contains("grid-row: 1 / span 1;"), "row pin must be preserved: {css_b}");

    let tw = tailwind_for(&d, g);
    let b_line = tw.lines().nth(2).expect("child B line");
    assert!(!b_line.contains("col-start") && !b_line.contains("col-span"), "dropped column pin must not emit col-start/col-span: {b_line}");
    assert!(b_line.contains("row-start-1"), "row pin must be preserved in Tailwind: {b_line}");

    let jsx = react_for(&d, g);
    assert!(!jsx.contains("gridColumn: '6 / span 1'"), "dropped column pin must not emit implicit column 6: {jsx}");
    assert!(jsx.contains("gridRow: '1 / span 1'"), "{jsx}");
}

#[test]
fn wide_column_span_pulls_start_column_back_to_match_solver_cell() {
    let mut d = Document::new("grid");
    let page = d.add_page("p");
    let gid = d.alloc_id();
    let mut g = Node::frame(gid, Rect::new(0.0, 0.0, 300.0, 100.0));
    g.fills.clear();
    let mut al = AutoLayout {
        direction: LayoutDirection::Grid,
        primary_sizing: Sizing::Fixed,
        counter_sizing: Sizing::Fixed,
        grid_columns: 3,
        grid_rows: 1,
        ..Default::default()
    };
    al.grid_col_tracks[0] = GridTrack::Fixed(100.0);
    al.grid_col_tracks[1] = GridTrack::Fixed(100.0);
    al.grid_col_tracks[2] = GridTrack::Fixed(100.0);
    al.grid_row_tracks[0] = GridTrack::Fixed(100.0);
    g.kind = NodeKind::Frame(FrameProps { auto_layout: Some(al), ..Default::default() });
    let g = d.insert(page, g).unwrap();

    let cid = d.alloc_id();
    let mut c = Node::rectangle(cid, Rect::new(0.0, 0.0, 40.0, 40.0));
    c.fills.clear();
    c.layout_child.grid_column = Some(2);
    c.layout_child.col_span = 2;
    c.layout_child.horizontal = Sizing::Fill;
    let c = d.insert(g, c).unwrap();

    d.relayout_subtree(g);
    // Solver pulls start column from index 2 back to index 1 (x = 100, width = 200) so span 2 fits in the 3-column grid.
    assert_eq!((d.get(c).unwrap().transform.tx, d.get(c).unwrap().size.x), (100.0, 200.0));

    let css_c = css_for(&d, c);
    assert!(css_c.contains("grid-column: 2 / span 2;"), "expected pulled-back start 2 / span 2, got:\n{css_c}");

    let tw = tailwind_for(&d, g);
    let c_line = tw.lines().nth(1).expect("child line");
    assert!(c_line.contains("col-start-2") && c_line.contains("col-span-2"), "expected col-start-2 col-span-2, got: {c_line}");

    let jsx = react_for(&d, g);
    assert!(jsx.contains("gridColumn: '2 / span 2'"), "{jsx}");
}

#[test]
fn span_exceeding_grid_columns_clamps_to_column_count_and_span_clamping_to_one_is_omitted() {
    let mut d = Document::new("grid");
    let page = d.add_page("p");
    let gid = d.alloc_id();
    let mut g = Node::frame(gid, Rect::new(0.0, 0.0, 400.0, 100.0));
    g.fills.clear();
    let al = AutoLayout {
        direction: LayoutDirection::Grid,
        primary_sizing: Sizing::Fixed,
        counter_sizing: Sizing::Fixed,
        grid_columns: 4,
        grid_rows: 1,
        ..Default::default()
    };
    g.kind = NodeKind::Frame(FrameProps { auto_layout: Some(al), ..Default::default() });
    let g = d.insert(page, g).unwrap();

    let cid = d.alloc_id();
    let mut c = Node::rectangle(cid, Rect::new(0.0, 0.0, 40.0, 40.0));
    c.fills.clear();
    c.layout_child.col_span = 13;
    c.layout_child.row_span = (MAX_GRID_TRACKS as u16) + 10;
    c.layout_child.horizontal = Sizing::Fill;
    let c = d.insert(g, c).unwrap();

    d.relayout_subtree(g);
    assert_eq!(d.get(c).unwrap().size.x, 400.0);

    let css_c = css_for(&d, c);
    assert!(css_c.contains("grid-column: span 4;"), "expected col_span clamped to 4, got:\n{css_c}");
    assert!(css_c.contains(&format!("grid-row: span {MAX_GRID_TRACKS};")), "expected row_span clamped to MAX_GRID_TRACKS ({MAX_GRID_TRACKS}), got:\n{css_c}");

    let tw = tailwind_for(&d, g);
    let c_line = tw.lines().nth(1).expect("child line");
    assert!(c_line.contains("col-span-4"), "expected col-span-4, got: {c_line}");
    assert!(!c_line.contains("span_13"), "must not overflow 4-column grid with span 13: {c_line}");
    assert!(
        c_line.contains(&format!("row-[span_{MAX_GRID_TRACKS}_/_span_{MAX_GRID_TRACKS}]")),
        "expected row span clamped to {MAX_GRID_TRACKS}, got: {c_line}"
    );

    let jsx_tw = react_with(&d, g, &ReactOptions { tailwind: true });
    assert!(jsx_tw.contains("col-span-4") && !jsx_tw.contains("span_13"), "{jsx_tw}");

    // When a 1-column grid clamps col_span = 3 down to 1 with no column pin, no grid-column or col-span is emitted.
    d.get_mut(g).unwrap().kind = NodeKind::Frame(FrameProps {
        auto_layout: Some(AutoLayout {
            direction: LayoutDirection::Grid,
            primary_sizing: Sizing::Fixed,
            counter_sizing: Sizing::Fixed,
            grid_columns: 1,
            grid_rows: 1,
            ..Default::default()
        }),
        ..Default::default()
    });
    d.get_mut(c).unwrap().layout_child.col_span = 3;
    d.get_mut(c).unwrap().layout_child.row_span = 1;
    let css_one = css_for(&d, c);
    assert!(!css_one.contains("grid-column:"), "span clamped to 1 without pin must omit grid-column:\n{css_one}");
    let tw_one = tailwind_for(&d, g);
    let one_line = tw_one.lines().nth(1).expect("child line");
    assert!(!one_line.contains("col-span") && !one_line.contains("col-start"), "span clamped to 1 must omit col-span: {one_line}");
}
