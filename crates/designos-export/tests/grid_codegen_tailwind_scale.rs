//! Tailwind only ships `grid-cols-{1..12}`, `grid-rows-{1..6}`, `col-start-{1..13}`, `row-start-{1..7}`,
//! `col-span-{1..12}` and `row-span-{1..6}`. Past those, a class would generate no rule at all, so
//! the export writes an arbitrary value instead.

use designos_doc::*;
use designos_export::{react_with, tailwind_for, ReactOptions};

/// A grid frame with `columns` x `rows` template tracks (all `1fr`) and one child set up by `child`.
fn grid(columns: u32, rows: u32, child: impl FnOnce(&mut LayoutChild)) -> (Document, NodeId) {
    let mut d = Document::new("t");
    let page = d.add_page("p");
    let gid = d.alloc_id();
    let mut g = Node::frame(gid, Rect::new(0.0, 0.0, 800.0, 400.0));
    g.fills.clear();
    let al = AutoLayout {
        direction: LayoutDirection::Grid,
        primary_sizing: Sizing::Fixed,
        counter_sizing: Sizing::Fixed,
        grid_columns: columns,
        grid_rows: rows,
        ..Default::default()
    };
    g.kind = NodeKind::Frame(FrameProps { auto_layout: Some(al), ..Default::default() });
    let g = d.insert(page, g).unwrap();
    let cid = d.alloc_id();
    let mut c = Node::rectangle(cid, Rect::new(0.0, 0.0, 40.0, 40.0));
    c.fills.clear();
    child(&mut c.layout_child);
    d.insert(g, c).unwrap();
    (d, g)
}

/// Class tokens of the element on line `n` of Tailwind markup.
fn classes(html: &str, n: usize) -> Vec<String> {
    let line = html.lines().nth(n).unwrap_or_default();
    let start = line.find("class").and_then(|i| line[i..].find('"').map(|j| i + j + 1)).unwrap_or(0);
    let end = line[start..].find('"').map(|i| start + i).unwrap_or(start);
    line[start..end].split_whitespace().map(String::from).collect()
}

fn has(classes: &[String], wanted: &str) -> bool {
    classes.iter().any(|c| c == wanted)
}

/// The classes of the grid frame (line 0) and of its only child (line 1).
fn grid_and_child(columns: u32, rows: u32, child: impl FnOnce(&mut LayoutChild)) -> (Vec<String>, Vec<String>) {
    let (d, g) = grid(columns, rows, child);
    let html = tailwind_for(&d, g);
    (classes(&html, 0), classes(&html, 1))
}

#[test]
fn uniform_columns_use_the_scale_up_to_twelve_and_an_arbitrary_value_past_it() {
    let (c12, _) = grid_and_child(12, 0, |_| {});
    assert!(has(&c12, "grid-cols-12"), "{c12:?}");
    let (c13, _) = grid_and_child(13, 0, |_| {});
    assert!(has(&c13, "grid-cols-[repeat(13,minmax(0,1fr))]"), "{c13:?}");
    let (c16, _) = grid_and_child(16, 0, |_| {});
    assert!(has(&c16, "grid-cols-[repeat(16,minmax(0,1fr))]"), "{c16:?}");
    assert!(!c16.iter().any(|c| c == "grid-cols-16"), "grid-cols-16 does not exist in Tailwind: {c16:?}");
}

#[test]
fn uniform_rows_use_the_scale_up_to_six_and_an_arbitrary_value_past_it() {
    let (c6, _) = grid_and_child(3, 6, |_| {});
    assert!(has(&c6, "grid-rows-6"), "{c6:?}");
    let (c7, _) = grid_and_child(3, 7, |_| {});
    assert!(has(&c7, "grid-rows-[repeat(7,minmax(0,1fr))]"), "{c7:?}");
}

#[test]
fn column_and_row_pins_use_the_scale_up_to_thirteen_and_seven() {
    let (_, in_scale) = grid_and_child(16, 8, |lc| {
        lc.grid_column = Some(12);
        lc.grid_row = Some(6);
    });
    assert!(has(&in_scale, "col-start-13") && has(&in_scale, "row-start-7"), "{in_scale:?}");

    let (_, past) = grid_and_child(16, 8, |lc| {
        lc.grid_column = Some(13);
        lc.grid_row = Some(7);
    });
    assert!(has(&past, "col-start-[14]") && has(&past, "row-start-[8]"), "{past:?}");
    assert!(!past.iter().any(|c| c == "col-start-14" || c == "row-start-8"), "col-start-14 and row-start-8 generate nothing: {past:?}");
}

#[test]
fn spans_use_the_scale_up_to_twelve_and_six() {
    let (_, in_scale) = grid_and_child(16, 8, |lc| {
        lc.col_span = 12;
        lc.row_span = 6;
    });
    assert!(has(&in_scale, "col-span-12") && has(&in_scale, "row-span-6"), "{in_scale:?}");

    let (_, past) = grid_and_child(16, 8, |lc| {
        lc.col_span = 13;
        lc.row_span = 7;
    });
    assert!(has(&past, "col-[span_13_/_span_13]") && has(&past, "row-[span_7_/_span_7]"), "{past:?}");
    assert!(!past.iter().any(|c| c == "col-span-13" || c == "row-span-7"), "col-span-13 and row-span-7 generate nothing: {past:?}");
}

#[test]
fn react_with_tailwind_writes_the_same_classes() {
    let (mut d, g) = grid(16, 0, |lc| {
        lc.grid_column = Some(13);
    });
    let cid = d.alloc_id();
    let mut c2 = Node::rectangle(cid, Rect::new(0.0, 0.0, 40.0, 40.0));
    c2.fills.clear();
    c2.layout_child.col_span = 13;
    d.insert(g, c2).unwrap();
    let jsx = react_with(&d, g, &ReactOptions { tailwind: true });
    assert!(jsx.contains("grid-cols-[repeat(16,minmax(0,1fr))]"), "{jsx}");
    assert!(jsx.contains("col-start-[14]") && jsx.contains("col-[span_13_/_span_13]"), "{jsx}");
}
