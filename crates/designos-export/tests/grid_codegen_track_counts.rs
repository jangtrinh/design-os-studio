//! The tracks the export writes must be the tracks the layout solver lays out: the same column
//! count (unset means 3, never more than `MAX_GRID_TRACKS`), flex tracks that can shrink under
//! their content, and an export time that does not grow with a stored track count.

use designos_doc::*;
use designos_export::{css_for, react_for, react_with, tailwind_for, ReactOptions};
use std::collections::BTreeSet;
use std::time::{Duration, Instant};

const MAX: usize = MAX_GRID_TRACKS;

/// A fixed 2000 x 400 grid frame holding `children` 10 x 10 rectangles.
fn grid(al: AutoLayout, children: usize) -> (Document, NodeId) {
    let mut d = Document::new("t");
    let page = d.add_page("p");
    let gid = d.alloc_id();
    let mut g = Node::frame(gid, Rect::new(0.0, 0.0, 2000.0, 400.0));
    g.fills.clear();
    let al = AutoLayout { direction: LayoutDirection::Grid, primary_sizing: Sizing::Fixed, counter_sizing: Sizing::Fixed, ..al };
    g.kind = NodeKind::Frame(FrameProps { auto_layout: Some(al), ..Default::default() });
    let g = d.insert(page, g).unwrap();
    for _ in 0..children {
        let cid = d.alloc_id();
        let mut c = Node::rectangle(cid, Rect::new(0.0, 0.0, 10.0, 10.0));
        c.fills.clear();
        d.insert(g, c).unwrap();
    }
    (d, g)
}

fn with_columns(columns: u32) -> AutoLayout {
    AutoLayout { grid_columns: columns, ..Default::default() }
}

/// The value of declaration `prop` in a CSS block.
fn decl(css: &str, prop: &str) -> String {
    let pre = format!("{prop}: ");
    css.lines().find_map(|l| l.strip_prefix(&pre)).map(|v| v.trim_end_matches(';').to_string()).unwrap_or_else(|| format!("<no {prop} in {css}>"))
}

/// The class tokens of the grid frame (first line) in Tailwind markup.
fn grid_classes(html: &str) -> Vec<String> {
    let line = html.lines().next().unwrap_or_default();
    let start = line.find("class=\"").map(|i| i + 7).unwrap_or(0);
    let end = line[start..].find('"').map(|i| start + i).unwrap_or(start);
    line[start..end].split_whitespace().map(String::from).collect()
}

fn uniform(n: usize) -> String {
    format!("repeat({n}, minmax(0, 1fr))")
}

/// How many columns the layout solver lays `grid` out in: the distinct x positions of its children.
fn solver_columns(d: &mut Document, g: NodeId) -> usize {
    d.relayout_subtree(g);
    let xs: BTreeSet<i64> = d.get(g).unwrap().children.iter().map(|c| (d.get(*c).unwrap().transform.tx * 100.0).round() as i64).collect();
    xs.len()
}

/// Runs `f`, which must finish well inside `limit` (a walk over a stored track count would not).
fn quickly<T>(limit: Duration, f: impl FnOnce() -> T) -> T {
    let t = Instant::now();
    let out = f();
    assert!(t.elapsed() < limit, "took {:?}", t.elapsed());
    out
}

#[test]
fn a_default_grid_is_the_same_uniform_template_in_css_react_and_tailwind() {
    let (d, g) = grid(with_columns(3), 1);
    assert_eq!(decl(&css_for(&d, g), "grid-template-columns"), uniform(3));
    assert!(react_for(&d, g).contains("gridTemplateColumns: 'repeat(3, minmax(0, 1fr))',"));
    assert!(grid_classes(&tailwind_for(&d, g)).contains(&"grid-cols-3".to_string()));
}

#[test]
fn an_unset_column_count_is_three_as_in_the_solver() {
    let (mut d, g) = grid(with_columns(0), 6);
    assert_eq!(decl(&css_for(&d, g), "grid-template-columns"), uniform(3));
    assert_eq!(solver_columns(&mut d, g), 3);
}

#[test]
fn a_column_count_past_the_solver_limit_exports_the_columns_the_solver_lays_out() {
    let past = MAX as u32 + 4;
    let (mut d, g) = grid(with_columns(past), past as usize);
    assert_eq!(solver_columns(&mut d, g), MAX, "the solver stops at MAX_GRID_TRACKS columns");
    assert_eq!(decl(&css_for(&d, g), "grid-template-columns"), uniform(MAX));
    assert!(react_for(&d, g).contains(&format!("gridTemplateColumns: '{}',", uniform(MAX))));
    assert!(grid_classes(&tailwind_for(&d, g)).iter().any(|c| c.contains(&format!("repeat({MAX},minmax(0,1fr))")) || *c == format!("grid-cols-{MAX}")));
}

#[test]
fn a_huge_column_count_exports_in_constant_time() {
    for columns in [5_000_000, u32::MAX] {
        let (d, g) = grid(with_columns(columns), 1);
        let css = quickly(Duration::from_secs(2), || css_for(&d, g));
        assert_eq!(decl(&css, "grid-template-columns"), uniform(MAX), "columns={columns}");
        let tw = quickly(Duration::from_secs(2), || tailwind_for(&d, g));
        assert!(tw.len() < 400, "columns={columns}: {} chars", tw.len());
        let jsx = quickly(Duration::from_secs(2), || react_with(&d, g, &ReactOptions::default()));
        assert!(jsx.len() < 600, "columns={columns}: {} chars", jsx.len());
    }
}

#[test]
fn a_huge_row_count_exports_in_constant_time() {
    let al = AutoLayout { grid_columns: 3, grid_rows: 5_000_000, ..Default::default() };
    let (d, g) = grid(al, 1);
    let css = quickly(Duration::from_secs(2), || css_for(&d, g));
    assert_eq!(decl(&css, "grid-template-rows"), uniform(5_000_000));

    // a stored row track in front of the default ones is still written out
    let mut al = AutoLayout { grid_columns: 3, grid_rows: 5_000_000, ..Default::default() };
    al.grid_row_tracks[0] = GridTrack::Fixed(80.0);
    let (d, g) = grid(al, 1);
    let css = quickly(Duration::from_secs(2), || css_for(&d, g));
    assert_eq!(decl(&css, "grid-template-rows"), format!("80px {}", uniform(4_999_999)));
    let tw = quickly(Duration::from_secs(2), || tailwind_for(&d, g));
    assert!(tw.len() < 400, "{} chars", tw.len());

    let (d, g) = grid(AutoLayout { grid_columns: 3, grid_rows: u32::MAX, ..Default::default() }, 1);
    let css = quickly(Duration::from_secs(2), || css_for(&d, g));
    assert_eq!(decl(&css, "grid-template-rows"), uniform(u32::MAX as usize));
}

#[test]
fn runs_of_default_tracks_fold_wherever_they_sit() {
    let mut al = with_columns(5);
    al.grid_col_tracks[2] = GridTrack::Fixed(120.0);
    let (d, g) = grid(al, 1);
    assert_eq!(decl(&css_for(&d, g), "grid-template-columns"), "repeat(2, minmax(0, 1fr)) 120px repeat(2, minmax(0, 1fr))");
    assert!(grid_classes(&tailwind_for(&d, g)).contains(&"grid-cols-[repeat(2,minmax(0,1fr))_120px_repeat(2,minmax(0,1fr))]".to_string()));

    // more columns than the document stores tracks for: the text does not depend on how many it stores
    let mut al = with_columns(MAX as u32);
    al.grid_col_tracks[0] = GridTrack::Fixed(120.0);
    let (d, g) = grid(al, 1);
    assert_eq!(decl(&css_for(&d, g), "grid-template-columns"), format!("120px repeat({}, minmax(0, 1fr))", MAX - 1));
}

#[test]
fn flex_tracks_in_a_mixed_template_can_shrink_under_their_content() {
    let mut al = with_columns(3);
    al.grid_col_tracks[0] = GridTrack::Flex(2.0);
    al.grid_col_tracks[1] = GridTrack::Fixed(120.0);
    al.grid_col_tracks[2] = GridTrack::Hug;
    let (d, g) = grid(al, 1);
    assert_eq!(decl(&css_for(&d, g), "grid-template-columns"), "minmax(0, 2fr) 120px auto");
    assert!(react_for(&d, g).contains("gridTemplateColumns: 'minmax(0, 2fr) 120px auto',"));
    assert!(grid_classes(&tailwind_for(&d, g)).contains(&"grid-cols-[minmax(0,2fr)_120px_auto]".to_string()));

    let mut al = with_columns(3);
    al.grid_col_tracks[1] = GridTrack::Fixed(120.0);
    let (d, g) = grid(al, 1);
    assert_eq!(decl(&css_for(&d, g), "grid-template-columns"), "minmax(0, 1fr) 120px minmax(0, 1fr)");
    assert!(grid_classes(&tailwind_for(&d, g)).contains(&"grid-cols-[minmax(0,1fr)_120px_minmax(0,1fr)]".to_string()));
}

#[test]
fn row_tracks_follow_the_same_rules() {
    let mut al = AutoLayout { grid_columns: 2, grid_rows: 3, ..Default::default() };
    al.grid_row_tracks[0] = GridTrack::Fixed(80.0);
    let (d, g) = grid(al, 1);
    assert_eq!(decl(&css_for(&d, g), "grid-template-rows"), "80px repeat(2, minmax(0, 1fr))");

    let (d, g) = grid(AutoLayout { grid_columns: 2, grid_rows: 3, ..Default::default() }, 1);
    assert_eq!(decl(&css_for(&d, g), "grid-template-rows"), uniform(3));
    assert!(grid_classes(&tailwind_for(&d, g)).contains(&"grid-rows-3".to_string()));
}
