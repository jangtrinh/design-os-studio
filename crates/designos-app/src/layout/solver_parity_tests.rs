//! The editor solver (`layout::layout_node`), the headless spec solver (`Document::relayout_subtree`)
//! and the changelog solver (`designos_doc::relayout_subtree`) must put every child at the same
//! position and size for the same frame.

use super::*;
use crate::util::v2;
use designos_doc::{
    AutoLayout, CounterAlignContent, FrameProps, GridTrack, LayoutAlign, LayoutChild, LayoutDirection, Node, NodeId, NodeKind, Rect, Sizing, Vec2,
};

fn auto(direction: LayoutDirection, primary: LayoutAlign) -> AutoLayout {
    AutoLayout {
        direction,
        primary_align: primary,
        primary_sizing: Sizing::Fixed,
        counter_sizing: Sizing::Fixed,
        padding: [0.0; 4],
        item_spacing: 0.0,
        counter_spacing: 0.0,
        ..Default::default()
    }
}

type Origins = Vec<(f32, f32)>;

/// Child origins from the editor solver, then from the spec solver run on a real document.
fn positions_from_both(al: AutoLayout, frame: Vec2, kids: &[(f32, f32)]) -> (Origins, Origins) {
    let ins: Vec<ChildIn> = kids.iter().map(|&(w, h)| ChildIn { size: v2(w, h), ..Default::default() }).collect();
    let editor = compute(&al, frame, SizeLimits::default(), &ins).rects.iter().map(|r| (r.x, r.y)).collect();

    let mut d = Document::new("t");
    let page = d.add_page("p");
    let fid = d.alloc_id();
    let mut f = Node::frame(fid, Rect::new(0.0, 0.0, frame.x, frame.y));
    f.kind = NodeKind::Frame(FrameProps { auto_layout: Some(al), ..Default::default() });
    let f = d.insert(page, f).unwrap();
    let mut ids = vec![];
    for &(w, h) in kids {
        let id = d.alloc_id();
        ids.push(d.insert(f, Node::rectangle(id, Rect::new(0.0, 0.0, w, h))).unwrap());
    }
    d.relayout_subtree(f);
    let spec = ids.iter().map(|id| (d.get(*id).unwrap().transform.tx, d.get(*id).unwrap().transform.ty)).collect();
    (editor, spec)
}

fn frame_for(direction: LayoutDirection, main: f32, cross: f32) -> Vec2 {
    if direction == LayoutDirection::Horizontal {
        v2(main, cross)
    } else {
        v2(cross, main)
    }
}

/// `n` children of `main` x `cross` (main along the flow, cross across it).
fn kids_for(direction: LayoutDirection, n: usize, main: f32, cross: f32) -> Vec<(f32, f32)> {
    let size = if direction == LayoutDirection::Horizontal { (main, cross) } else { (cross, main) };
    vec![size; n]
}

const DIRECTIONS: [LayoutDirection; 2] = [LayoutDirection::Horizontal, LayoutDirection::Vertical];

#[test]
fn space_around_and_evenly_agree_with_free_space() {
    for direction in DIRECTIONS {
        for align in [LayoutAlign::SpaceAround, LayoutAlign::SpaceEvenly] {
            let mut al = auto(direction, align);
            al.item_spacing = 6.0;
            let kids = kids_for(direction, 4, 10.0, 10.0);
            let (editor, spec) = positions_from_both(al, frame_for(direction, 120.0, 20.0), &kids);
            assert_eq!(editor, spec, "{direction:?} {align:?}");
        }
    }
}

#[test]
fn space_around_and_evenly_overflow_pack_from_the_start_ignoring_item_spacing() {
    // four 40-long items in a 120 frame overflow: no free space, so no gap, not the 10 px item spacing
    for direction in DIRECTIONS {
        for align in [LayoutAlign::SpaceAround, LayoutAlign::SpaceEvenly] {
            let mut al = auto(direction, align);
            al.item_spacing = 10.0;
            let kids = kids_for(direction, 4, 40.0, 10.0);
            let frame = frame_for(direction, 120.0, 20.0);
            let (editor, spec) = positions_from_both(al, frame, &kids);
            let main = |p: &(f32, f32)| if direction == LayoutDirection::Horizontal { p.0 } else { p.1 };
            assert_eq!(editor.iter().map(main).collect::<Vec<_>>(), vec![0.0, 40.0, 80.0, 120.0], "editor {direction:?} {align:?}");
            assert_eq!(editor, spec, "{direction:?} {align:?}");
        }
    }
}

#[test]
fn wrapped_space_between_agrees_with_counter_align_content() {
    // 100-long main axis holds two 40-long items per line: three lines of 10, counter gap spread over a 100 frame
    for direction in DIRECTIONS {
        for content in [CounterAlignContent::Auto, CounterAlignContent::SpaceBetween] {
            let mut al = auto(direction, LayoutAlign::SpaceBetween);
            al.wrap = true;
            al.item_spacing = 4.0;
            al.counter_spacing = 2.0;
            al.counter_align_content = content;
            let kids = kids_for(direction, 6, 40.0, 10.0);
            let (editor, spec) = positions_from_both(al, frame_for(direction, 100.0, 100.0), &kids);
            assert_eq!(editor, spec, "{direction:?} {content:?}");
        }
    }
}

#[test]
fn wrapped_counter_space_between_spreads_lines_edge_to_edge() {
    // literal: lines of cross 10 in a 100 cross frame, gap = (100 - 30) / 2 = 35, so lines at 0, 45, 90
    let mut al = auto(LayoutDirection::Horizontal, LayoutAlign::Start);
    al.wrap = true;
    al.counter_align_content = CounterAlignContent::SpaceBetween;
    let (editor, spec) = positions_from_both(al, v2(100.0, 100.0), &[(40.0, 10.0); 6]);
    let ys = |p: &[(f32, f32)]| p.iter().map(|q| q.1).collect::<Vec<_>>();
    assert_eq!(ys(&editor), vec![0.0, 0.0, 45.0, 45.0, 90.0, 90.0]);
    assert_eq!(ys(&spec), ys(&editor));
}

// ---- the same frame through every entry point ------------------------------------------------

/// One flow child: its natural size plus the layout-child properties the solver reads.
#[derive(Clone, Copy)]
struct Kid {
    size: (f32, f32),
    lc: LayoutChild,
}

fn kid(w: f32, h: f32) -> Kid {
    Kid { size: (w, h), lc: LayoutChild::default() }
}

fn kid_with(w: f32, h: f32, edit: impl FnOnce(&mut LayoutChild)) -> Kid {
    let mut k = kid(w, h);
    edit(&mut k.lc);
    k
}

/// Frame size followed by `[x, y, w, h]` per child.
#[derive(Debug, PartialEq)]
struct Solved {
    frame: (f32, f32),
    kids: Vec<[f32; 4]>,
}

#[derive(Clone, Copy, Debug)]
enum Entry {
    /// `layout::layout_node` (the editor, through an `Edit` session).
    Editor,
    /// `Document::relayout_subtree` (specs, MCP).
    Spec,
    /// `designos_doc::relayout_subtree` (suggest auto layout, variable modes, component swaps).
    Changelog,
}

const ENTRIES: [Entry; 3] = [Entry::Editor, Entry::Spec, Entry::Changelog];

fn solve_through(entry: Entry, al: AutoLayout, frame: Vec2, frame_lc: LayoutChild, kids: &[Kid]) -> Solved {
    let mut d = Document::new("t");
    let page = d.add_page("p");
    let fid = d.alloc_id();
    let mut f = Node::frame(fid, Rect::new(0.0, 0.0, frame.x, frame.y));
    f.kind = NodeKind::Frame(FrameProps { auto_layout: Some(al), ..Default::default() });
    f.layout_child = frame_lc;
    let f = d.insert(page, f).unwrap();
    let mut ids: Vec<NodeId> = vec![];
    for k in kids {
        let id = d.alloc_id();
        let mut n = Node::rectangle(id, Rect::new(0.0, 0.0, k.size.0, k.size.1));
        n.layout_child = k.lc;
        ids.push(d.insert(f, n).unwrap());
    }
    match entry {
        Entry::Editor => layout_node(&mut d, &mut Edit::new(), f),
        Entry::Spec => d.relayout_subtree(f),
        Entry::Changelog => designos_doc::relayout_subtree(&mut d, f),
    }
    let fs = d.get(f).unwrap().size;
    let kids = ids
        .iter()
        .map(|id| {
            let n = d.get(*id).unwrap();
            [n.transform.tx, n.transform.ty, n.size.x, n.size.y]
        })
        .collect();
    Solved { frame: (fs.x, fs.y), kids }
}

/// Every entry point must agree; returns the shared result for literal checks.
fn solve_everywhere(al: AutoLayout, frame: Vec2, kids: &[Kid]) -> Solved {
    solve_everywhere_with_frame_limits(al, frame, LayoutChild::default(), kids)
}

fn solve_everywhere_with_frame_limits(al: AutoLayout, frame: Vec2, frame_lc: LayoutChild, kids: &[Kid]) -> Solved {
    let editor = solve_through(Entry::Editor, al, frame, frame_lc, kids);
    for entry in ENTRIES {
        assert_eq!(solve_through(entry, al, frame, frame_lc, kids), editor, "{entry:?} drifted from the editor solver");
    }
    editor
}

#[test]
fn fill_children_share_free_space_by_grow_weight() {
    // grow 1 : 3 over a 300 main axis -> 75 : 225, wherever the solver runs
    for direction in DIRECTIONS {
        let kids = [kid_with(10.0, 10.0, |lc| lc.grow = 1.0), kid_with(10.0, 10.0, |lc| lc.grow = 3.0)];
        let solved = solve_everywhere(auto(direction, LayoutAlign::Start), frame_for(direction, 300.0, 50.0), &kids);
        let (a, b) = (solved.kids[0], solved.kids[1]);
        if direction == LayoutDirection::Horizontal {
            assert_eq!((a, b), ([0.0, 0.0, 75.0, 10.0], [75.0, 0.0, 225.0, 10.0]), "{direction:?}");
        } else {
            assert_eq!((a, b), ([0.0, 0.0, 10.0, 75.0], [0.0, 75.0, 10.0, 225.0]), "{direction:?}");
        }
    }
}

#[test]
fn plain_fill_weighs_one_next_to_a_grow_child() {
    // a Fill child (weight 1) beside grow 3 -> 75 : 225
    let kids = [kid_with(10.0, 10.0, |lc| lc.horizontal = Sizing::Fill), kid_with(10.0, 10.0, |lc| lc.grow = 3.0)];
    let solved = solve_everywhere(auto(LayoutDirection::Horizontal, LayoutAlign::Start), v2(300.0, 50.0), &kids);
    assert_eq!(solved.kids[0], [0.0, 0.0, 75.0, 10.0]);
    assert_eq!(solved.kids[1], [75.0, 0.0, 225.0, 10.0]);
}

#[test]
fn wrapped_children_flow_into_lines_in_every_solver() {
    // two 40-long items per 100-long line, 4 apart; lines 2 apart (literal)
    let mut al = auto(LayoutDirection::Horizontal, LayoutAlign::Start);
    al.wrap = true;
    al.item_spacing = 4.0;
    al.counter_spacing = 2.0;
    let kids = vec![kid(40.0, 10.0); 6];
    let solved = solve_everywhere(al, v2(100.0, 100.0), &kids);
    let xy: Vec<(f32, f32)> = solved.kids.iter().map(|k| (k[0], k[1])).collect();
    assert_eq!(xy, vec![(0.0, 0.0), (44.0, 0.0), (0.0, 12.0), (44.0, 12.0), (0.0, 24.0), (44.0, 24.0)]);

    // vertical wrap fills columns top to bottom, then starts the next column
    let mut al = auto(LayoutDirection::Vertical, LayoutAlign::Start);
    al.wrap = true;
    al.item_spacing = 4.0;
    al.counter_spacing = 2.0;
    let kids = vec![kid(10.0, 40.0); 6];
    let solved = solve_everywhere(al, v2(100.0, 100.0), &kids);
    let xy: Vec<(f32, f32)> = solved.kids.iter().map(|k| (k[0], k[1])).collect();
    assert_eq!(xy, vec![(0.0, 0.0), (0.0, 44.0), (12.0, 0.0), (12.0, 44.0), (24.0, 0.0), (24.0, 44.0)]);
}

#[test]
fn wrapped_hug_cross_and_counter_align_agree() {
    let mut al = auto(LayoutDirection::Horizontal, LayoutAlign::Center);
    al.wrap = true;
    al.counter_align = LayoutAlign::End;
    al.counter_sizing = Sizing::Hug;
    al.item_spacing = 5.0;
    al.counter_spacing = 3.0;
    let kids = [kid(30.0, 10.0), kid(30.0, 20.0), kid(60.0, 8.0)];
    let solved = solve_everywhere(al, v2(70.0, 1.0), &kids);
    // line 1 = the two 30s (cross 20, end aligned), line 2 = the 60; hugged cross = 20 + 3 + 8
    assert_eq!(solved.frame, (70.0, 31.0));
    assert_eq!(solved.kids[0], [2.5, 10.0, 30.0, 10.0]);
    assert_eq!(solved.kids[1], [37.5, 0.0, 30.0, 20.0]);
}

#[test]
fn child_limits_clamp_the_distributed_size_and_siblings_follow() {
    // the Fill child would take 250 but is capped at 60; the next sibling packs right behind it
    let capped = kid_with(10.0, 10.0, |lc| {
        lc.horizontal = Sizing::Fill;
        lc.max_w = Some(60.0);
    });
    let solved = solve_everywhere(auto(LayoutDirection::Horizontal, LayoutAlign::Start), v2(300.0, 50.0), &[capped, kid(50.0, 10.0)]);
    assert_eq!(solved.kids[0], [0.0, 0.0, 60.0, 10.0]);
    assert_eq!(solved.kids[1], [60.0, 0.0, 50.0, 10.0]);
}

#[test]
fn fill_siblings_redistribute_space_after_min_or_max_clamping_freeze_loop() {
    // Width 300, Fill A capped at 60 and Fill B uncapped: B must be 240, not 150.
    let fill_a_capped = kid_with(10.0, 10.0, |lc| {
        lc.horizontal = Sizing::Fill;
        lc.max_w = Some(60.0);
    });
    let fill_b = kid_with(10.0, 10.0, |lc| lc.horizontal = Sizing::Fill);
    let solved = solve_everywhere(auto(LayoutDirection::Horizontal, LayoutAlign::Start), v2(300.0, 50.0), &[fill_a_capped, fill_b]);
    assert_eq!(solved.kids[0], [0.0, 0.0, 60.0, 10.0]);
    assert_eq!(solved.kids[1], [60.0, 0.0, 240.0, 10.0]);

    // Min-clamped Fill child freezes at 200 and leaves 100 for its Fill sibling.
    let fill_a_min = kid_with(10.0, 10.0, |lc| {
        lc.horizontal = Sizing::Fill;
        lc.min_w = Some(200.0);
    });
    let solved_min = solve_everywhere(auto(LayoutDirection::Horizontal, LayoutAlign::Start), v2(300.0, 50.0), &[fill_a_min, fill_b]);
    assert_eq!(solved_min.kids[0], [0.0, 0.0, 200.0, 10.0]);
    assert_eq!(solved_min.kids[1], [200.0, 0.0, 100.0, 10.0]);

    // Multi-pass cascade: A (grow 1, max 50) freezes first, then B (grow 1, max 80) exceeds 80 on pass 2 and freezes, leaving 170 for C (grow 2).
    let a = kid_with(10.0, 10.0, |lc| {
        lc.grow = 1.0;
        lc.max_w = Some(50.0);
    });
    let b = kid_with(10.0, 10.0, |lc| {
        lc.grow = 1.0;
        lc.max_w = Some(80.0);
    });
    let c = kid_with(10.0, 10.0, |lc| lc.grow = 2.0);
    let solved_cascade = solve_everywhere(auto(LayoutDirection::Horizontal, LayoutAlign::Start), v2(300.0, 50.0), &[a, b, c]);
    assert_eq!(solved_cascade.kids[0], [0.0, 0.0, 50.0, 10.0]);
    assert_eq!(solved_cascade.kids[1], [50.0, 0.0, 80.0, 10.0]);
    assert_eq!(solved_cascade.kids[2], [130.0, 0.0, 170.0, 10.0]);
}

#[test]
fn child_minimums_raise_natural_sizes_and_cross_fill_respects_maximums() {
    let raised = kid_with(10.0, 10.0, |lc| {
        lc.min_w = Some(30.0);
        lc.min_h = Some(20.0);
    });
    let stretched = kid_with(10.0, 10.0, |lc| {
        lc.vertical = Sizing::Fill;
        lc.max_h = Some(25.0);
    });
    let solved = solve_everywhere(auto(LayoutDirection::Horizontal, LayoutAlign::Start), v2(300.0, 50.0), &[raised, stretched]);
    assert_eq!(solved.kids[0], [0.0, 0.0, 30.0, 20.0]);
    assert_eq!(solved.kids[1], [30.0, 0.0, 10.0, 25.0]);
}

#[test]
fn a_hugging_frame_respects_its_own_limits() {
    let mut al = auto(LayoutDirection::Horizontal, LayoutAlign::Start);
    al.primary_sizing = Sizing::Hug;
    let frame_lc = LayoutChild { max_w: Some(150.0), ..Default::default() };
    let solved = solve_everywhere_with_frame_limits(al, v2(10.0, 40.0), frame_lc, &[kid(100.0, 10.0), kid(100.0, 10.0)]);
    assert_eq!(solved.frame, (150.0, 40.0));
}

fn grid(columns: u32) -> AutoLayout {
    AutoLayout { direction: LayoutDirection::Grid, grid_columns: columns, ..Default::default() }
}

#[test]
fn grid_honors_explicit_cells_and_fixed_children_keep_their_own_size() {
    let mut al = grid(2);
    al.item_spacing = 10.0;
    al.grid_col_tracks[0] = GridTrack::Fixed(100.0);
    al.grid_col_tracks[1] = GridTrack::Fixed(100.0);
    // the explicit child lands in column 1 / row 1; the auto child takes the first free cell
    let explicit = kid_with(50.0, 20.0, |lc| {
        lc.grid_column = Some(1);
        lc.grid_row = Some(1);
    });
    let solved = solve_everywhere(al, v2(300.0, 300.0), &[explicit, kid(60.0, 30.0)]);
    assert_eq!(solved.kids[0], [110.0, 40.0, 50.0, 20.0]);
    assert_eq!(solved.kids[1], [0.0, 0.0, 60.0, 30.0]);
}

#[test]
fn grid_fixed_row_tracks_set_the_row_height() {
    let mut al = grid(1);
    al.item_spacing = 10.0;
    al.grid_row_tracks[0] = GridTrack::Fixed(80.0);
    let solved = solve_everywhere(al, v2(200.0, 300.0), &[kid(50.0, 20.0), kid(50.0, 20.0)]);
    assert_eq!((solved.kids[0][1], solved.kids[1][1]), (0.0, 90.0));
}

#[test]
fn grid_spans_and_hugging_frame_agree() {
    let mut al = grid(3);
    al.item_spacing = 10.0;
    al.padding = [5.0; 4];
    al.primary_sizing = Sizing::Hug;
    al.counter_sizing = Sizing::Hug;
    let kids = [kid_with(60.0, 20.0, |lc| lc.col_span = 2), kid(50.0, 20.0), kid(50.0, 20.0), kid(50.0, 20.0), kid(50.0, 20.0)];
    let solved = solve_everywhere(al, v2(1.0, 1.0), &kids);
    // three 50-wide hugged columns + two 10 gaps + 10 padding; two 20-high rows + one gap + padding
    assert_eq!(solved.frame, (180.0, 60.0));
    // the spanning child is Fixed, so it keeps its own 60 px inside the 110 px cell
    assert_eq!(solved.kids[0], [5.0, 5.0, 60.0, 20.0]);
    assert_eq!(solved.kids[1], [125.0, 5.0, 50.0, 20.0]);
    assert_eq!(solved.kids[4], [125.0, 35.0, 50.0, 20.0]);
}

#[test]
fn grid_stretches_only_fill_children_across_their_cell() {
    // two 145 px flex columns (300 - 10 gap), row 0 as tall as its tallest child (20)
    let mut al = grid(2);
    al.item_spacing = 10.0;
    let fixed = kid(50.0, 20.0);
    let fill = kid_with(10.0, 10.0, |lc| {
        lc.horizontal = Sizing::Fill;
        lc.vertical = Sizing::Fill;
    });
    let hug = kid_with(30.0, 15.0, |lc| {
        lc.horizontal = Sizing::Hug;
        lc.vertical = Sizing::Hug;
    });
    let solved = solve_everywhere(al, v2(300.0, 300.0), &[fixed, fill, hug]);
    assert_eq!(solved.kids[0], [0.0, 0.0, 50.0, 20.0], "Fixed keeps its width and height");
    assert_eq!(solved.kids[1], [155.0, 0.0, 145.0, 20.0], "Fill takes the cell width and the row height");
    assert_eq!(solved.kids[2], [0.0, 30.0, 30.0, 15.0], "Hug keeps its content size");
}

#[test]
fn grid_fixed_child_spanning_rows_keeps_its_size() {
    // the spanning child's cell is 40 + 10 + 7.5 high (row 0 follows its taller neighbour), but it is Fixed 40 x 25
    let mut al = grid(2);
    al.item_spacing = 10.0;
    let spanning = kid_with(40.0, 25.0, |lc| lc.row_span = 2);
    let solved = solve_everywhere(al, v2(300.0, 300.0), &[spanning, kid(40.0, 40.0)]);
    assert_eq!(solved.kids[0], [0.0, 0.0, 40.0, 25.0]);
}

#[test]
fn grid_clamps_child_sizes_when_sizing_hug_columns_and_auto_rows() {
    let mut al = grid(2);
    al.item_spacing = 10.0;
    al.primary_sizing = Sizing::Hug;
    al.counter_sizing = Sizing::Hug;
    al.grid_col_tracks[0] = GridTrack::Hug;
    al.grid_col_tracks[1] = GridTrack::Hug;
    let capped = kid_with(120.0, 80.0, |lc| {
        lc.max_w = Some(50.0);
        lc.max_h = Some(30.0);
    });
    let plain = kid(40.0, 20.0);
    let solved = solve_everywhere(al, v2(300.0, 300.0), &[capped, plain]);
    assert_eq!(solved.frame, (100.0, 30.0));
    assert_eq!(solved.kids[0], [0.0, 0.0, 50.0, 30.0]);
    assert_eq!(solved.kids[1], [60.0, 0.0, 40.0, 20.0]);
}

#[test]
fn grid_row_span_does_not_inflate_rows_that_already_hold_it() {
    let mut al = grid(2);
    al.item_spacing = 10.0;
    al.counter_sizing = Sizing::Hug;
    // Row 0 is 40 high, row 1 is 15 high; with the 10 gap they already provide 65 >= 50.
    let spanning_fits = kid_with(40.0, 50.0, |lc| lc.row_span = 2);
    let solved = solve_everywhere(al, v2(300.0, 300.0), &[spanning_fits, kid(40.0, 40.0), kid(40.0, 15.0)]);
    assert_eq!(solved.frame, (300.0, 65.0));
    assert_eq!(solved.kids[2], [155.0, 50.0, 40.0, 15.0]);

    // When a spanning item (70) exceeds row 0 (40) + gap (10) + row 1 (10) = 60,
    // row 0 already exceeds an even 30 split, so only row 1 grows to 20 (total 70).
    let spanning_grows_short_row = kid_with(40.0, 70.0, |lc| lc.row_span = 2);
    let solved2 = solve_everywhere(al, v2(300.0, 300.0), &[spanning_grows_short_row, kid(40.0, 40.0), kid(40.0, 10.0)]);
    assert_eq!(solved2.frame, (300.0, 70.0));
    assert_eq!(solved2.kids[2], [155.0, 50.0, 40.0, 10.0]);
}

/// Solves `kids` placed in child order `order` (indices into `kids`); returns the frame and each
/// kid's rect keyed by its index in `kids`, so different orders can be compared kid by kid.
fn solve_in_child_order(al: AutoLayout, frame: Vec2, kids: &[Kid], order: &[usize]) -> ((f32, f32), Vec<[f32; 4]>) {
    let reordered: Vec<Kid> = order.iter().map(|&i| kids[i]).collect();
    let solved = solve_everywhere(al, frame, &reordered);
    let mut rects = vec![[0.0; 4]; kids.len()];
    for (pos, &i) in order.iter().enumerate() {
        rects[i] = solved.kids[pos];
    }
    (solved.frame, rects)
}

const ORDERS_OF_THREE: [[usize; 3]; 6] = [[0, 1, 2], [0, 2, 1], [1, 0, 2], [1, 2, 0], [2, 0, 1], [2, 1, 0]];

fn pinned_span(h: f32, column: u16, row: u16, row_span: u16) -> Kid {
    kid_with(40.0, h, |lc| {
        lc.grid_column = Some(column);
        lc.grid_row = Some(row);
        lc.row_span = row_span;
    })
}

fn hugging_grid(columns: u32) -> AutoLayout {
    let mut al = grid(columns);
    al.item_spacing = 10.0;
    al.counter_sizing = Sizing::Hug;
    al
}

#[test]
fn grid_overlapping_row_spans_size_rows_independent_of_child_order() {
    // The tall item holds rows 0-1 (100 high), the short one rows 1-2 (40 high). Rows 0 and 1 share the
    // tall item's 90 (45 each), which already holds the short item, so the empty row 2 stays flat:
    // 45 + 45 + 0 + two 10 gaps = 110 whichever of the two comes first.
    let pair = [pinned_span(100.0, 0, 0, 2), pinned_span(40.0, 1, 1, 2)];
    let forward = solve_in_child_order(hugging_grid(2), v2(300.0, 300.0), &pair, &[0, 1]);
    let reversed = solve_in_child_order(hugging_grid(2), v2(300.0, 300.0), &pair, &[1, 0]);
    assert_eq!(forward.0, (300.0, 110.0));
    assert_eq!(reversed, forward);
}

#[test]
fn grid_overlapping_row_spans_keep_content_below_in_place_for_every_child_order() {
    // Same spans plus a plain 5-high item in row 2: the rows stay 45, 45, 5 (frame 115) and the item sits at 110.
    let kids = [pinned_span(100.0, 0, 0, 2), pinned_span(40.0, 1, 1, 2), pinned_span(5.0, 0, 2, 1)];
    let first = solve_in_child_order(hugging_grid(2), v2(300.0, 300.0), &kids, &ORDERS_OF_THREE[0]);
    assert_eq!(first.0, (300.0, 115.0));
    assert_eq!(first.1[2], [0.0, 110.0, 40.0, 5.0]);
    for order in &ORDERS_OF_THREE[1..] {
        assert_eq!(solve_in_child_order(hugging_grid(2), v2(300.0, 300.0), &kids, order), first, "child order {order:?} changed the layout");
    }
}

#[test]
fn grid_row_spans_of_different_lengths_size_rows_independent_of_child_order() {
    // A 3-row span (110), a 2-row span over rows 0-1 (100) and a 2-row span over rows 1-2 (60).
    // The 2-row spans settle first: rows 45, 45, then row 2 gains the 5 the lower span still needs.
    // The 3-row span (90 without gaps) already fits in 95, so no row grows: 45 + 45 + 5 + two gaps.
    let kids = [pinned_span(110.0, 0, 0, 3), pinned_span(100.0, 1, 0, 2), pinned_span(60.0, 2, 1, 2)];
    let first = solve_in_child_order(hugging_grid(3), v2(300.0, 300.0), &kids, &ORDERS_OF_THREE[0]);
    assert_eq!(first.0, (300.0, 115.0));
    for order in &ORDERS_OF_THREE[1..] {
        assert_eq!(solve_in_child_order(hugging_grid(3), v2(300.0, 300.0), &kids, order), first, "child order {order:?} changed the layout");
    }
}

#[test]
fn grid_overlapping_row_spans_size_rows_the_same_when_the_tall_span_is_lower() {
    // Mirror image of the pair above: the tall item holds rows 1-2, the short one rows 0-1.
    let pair = [pinned_span(40.0, 1, 0, 2), pinned_span(100.0, 0, 1, 2)];
    let forward = solve_in_child_order(hugging_grid(2), v2(300.0, 300.0), &pair, &[0, 1]);
    let reversed = solve_in_child_order(hugging_grid(2), v2(300.0, 300.0), &pair, &[1, 0]);
    assert_eq!(forward.0, (300.0, 110.0));
    assert_eq!(reversed, forward);
}

// ---- grid placement: pinned first, then locked, then auto -------------------------------------

fn two_fixed_columns() -> AutoLayout {
    let mut al = grid(2);
    al.item_spacing = 10.0;
    al.grid_col_tracks[0] = GridTrack::Fixed(100.0);
    al.grid_col_tracks[1] = GridTrack::Fixed(100.0);
    al
}

#[test]
fn grid_auto_child_before_a_pinned_child_flows_around_it() {
    let pinned = kid_with(50.0, 20.0, |lc| {
        lc.grid_column = Some(0);
        lc.grid_row = Some(0);
    });
    // the auto child comes first in child order but must not take the pinned cell (0, 0)
    let solved = solve_everywhere(two_fixed_columns(), v2(300.0, 300.0), &[kid(50.0, 20.0), pinned]);
    assert_eq!(solved.kids[1], [0.0, 0.0, 50.0, 20.0], "pinned child keeps its cell");
    assert_eq!(solved.kids[0], [110.0, 0.0, 50.0, 20.0], "auto child takes the next free cell");
}

#[test]
fn grid_locked_children_place_after_pinned_ones_and_before_auto_ones() {
    let column_locked = kid_with(50.0, 20.0, |lc| lc.grid_column = Some(0));
    let row_locked = kid_with(50.0, 20.0, |lc| lc.grid_row = Some(0));
    let pinned = kid_with(50.0, 20.0, |lc| {
        lc.grid_column = Some(0);
        lc.grid_row = Some(0);
    });
    // child order: auto, column-locked, row-locked, pinned
    let solved = solve_everywhere(two_fixed_columns(), v2(300.0, 300.0), &[kid(50.0, 20.0), column_locked, row_locked, pinned]);
    assert_eq!(solved.kids[3][..2], [0.0, 0.0], "pinned (0, 0)");
    assert_eq!(solved.kids[1][..2], [0.0, 30.0], "column 0 locked: first free row below the pinned child");
    assert_eq!(solved.kids[2][..2], [110.0, 0.0], "row 0 locked: first free column in row 0");
    assert_eq!(solved.kids[0][..2], [110.0, 30.0], "auto: first free cell in reading order");
}

#[test]
fn grid_child_pinned_to_a_missing_column_falls_back_to_a_free_cell_in_its_row() {
    // column 2 does not exist in a 2-column grid; it must not clamp onto the child pinned at (0, 1)
    let a = kid_with(50.0, 20.0, |lc| {
        lc.grid_row = Some(0);
        lc.grid_column = Some(0);
    });
    let b = kid_with(50.0, 20.0, |lc| {
        lc.grid_row = Some(0);
        lc.grid_column = Some(1);
    });
    let orphan = kid_with(50.0, 20.0, |lc| {
        lc.grid_row = Some(0);
        lc.grid_column = Some(2);
    });
    let solved = solve_everywhere(two_fixed_columns(), v2(300.0, 300.0), &[a, b, orphan]);
    assert_eq!(solved.kids[0][..2], [0.0, 0.0]);
    assert_eq!(solved.kids[1][..2], [110.0, 0.0]);
    assert_eq!(solved.kids[2][..2], [0.0, 30.0]);
}

// ---- a frame's own min/max applies before alignment ------------------------------------------

#[test]
fn hugging_frame_raised_by_its_minimum_centers_children_in_the_raised_box() {
    // content 100, min_w 200: the frame grows to 200 and the child centers in it, at x = 50
    let mut al = auto(LayoutDirection::Horizontal, LayoutAlign::Center);
    al.primary_sizing = Sizing::Hug;
    let frame_lc = LayoutChild { min_w: Some(200.0), ..Default::default() };
    let solved = solve_everywhere_with_frame_limits(al, v2(10.0, 40.0), frame_lc, &[kid(100.0, 10.0)]);
    assert_eq!(solved.frame, (200.0, 40.0));
    assert_eq!(solved.kids[0], [50.0, 0.0, 100.0, 10.0]);
}

#[test]
fn hugging_cross_axis_raised_by_its_minimum_aligns_children_in_the_raised_box() {
    // content 10 high, min_h 50: counter Center puts the child at y = 20; End at y = 40
    for (align, y) in [(LayoutAlign::Center, 20.0), (LayoutAlign::End, 40.0)] {
        let mut al = auto(LayoutDirection::Horizontal, LayoutAlign::Start);
        al.counter_sizing = Sizing::Hug;
        al.counter_align = align;
        let frame_lc = LayoutChild { min_h: Some(50.0), ..Default::default() };
        let solved = solve_everywhere_with_frame_limits(al, v2(100.0, 1.0), frame_lc, &[kid(30.0, 10.0)]);
        assert_eq!(solved.frame, (100.0, 50.0), "{align:?}");
        assert_eq!(solved.kids[0], [0.0, y, 30.0, 10.0], "{align:?}");
    }
}

#[test]
fn vertical_frame_raised_by_its_minimum_centers_children_along_the_main_axis() {
    let mut al = auto(LayoutDirection::Vertical, LayoutAlign::Center);
    al.primary_sizing = Sizing::Hug;
    let frame_lc = LayoutChild { min_h: Some(100.0), ..Default::default() };
    let solved = solve_everywhere_with_frame_limits(al, v2(40.0, 1.0), frame_lc, &[kid(10.0, 20.0)]);
    assert_eq!(solved.frame, (40.0, 100.0));
    assert_eq!(solved.kids[0], [0.0, 40.0, 10.0, 20.0]);
}

#[test]
fn fixed_frame_below_its_minimum_lays_out_in_the_clamped_size() {
    // a Fixed 100 frame with min_w 200 becomes 200 wide, so a Fill child is 200 wide, not 100
    let al = auto(LayoutDirection::Horizontal, LayoutAlign::Start);
    let frame_lc = LayoutChild { min_w: Some(200.0), ..Default::default() };
    let fill = kid_with(10.0, 10.0, |lc| lc.horizontal = Sizing::Fill);
    let solved = solve_everywhere_with_frame_limits(al, v2(100.0, 40.0), frame_lc, &[fill]);
    assert_eq!(solved.frame, (200.0, 40.0));
    assert_eq!(solved.kids[0], [0.0, 0.0, 200.0, 10.0]);
}

#[test]
fn grid_frame_raised_by_its_minimum_shares_the_extra_width_between_flex_columns() {
    // hugging 2 columns of 50 + 10 gap = 110, min_w 300: both flex columns grow to 145
    let mut al = grid(2);
    al.item_spacing = 10.0;
    al.primary_sizing = Sizing::Hug;
    let frame_lc = LayoutChild { min_w: Some(300.0), ..Default::default() };
    let solved = solve_everywhere_with_frame_limits(al, v2(1.0, 1.0), frame_lc, &[kid(50.0, 20.0), kid(50.0, 20.0)]);
    assert_eq!(solved.frame.0, 300.0);
    assert_eq!(solved.kids[1], [155.0, 0.0, 50.0, 20.0]);

    // a Fixed grid frame below its minimum sizes its flex columns from the clamped width
    let mut al = grid(2);
    al.item_spacing = 10.0;
    let fill = kid_with(10.0, 10.0, |lc| lc.horizontal = Sizing::Fill);
    let solved = solve_everywhere_with_frame_limits(al, v2(100.0, 50.0), frame_lc, &[fill, fill]);
    assert_eq!(solved.kids[0], [0.0, 0.0, 145.0, 10.0]);
    assert_eq!(solved.kids[1], [155.0, 0.0, 145.0, 10.0]);
}

#[test]
fn nested_container_resized_by_parent_fill_resolves_its_own_children() {
    // Parent Fixed 300x100 (padding 10, gap 20) holds a Fixed 40x30 child and a nested auto-layout
    // child frame with horizontal Fill + vertical Hug. The nested frame hugs (98, 32) on the bottom-up
    // pass, then the parent stretches it to width 220, so the nested frame must re-solve its own
    // children inside 220 (stretching its Fill grandchild to 220 - 16 - 50 - 12 = 142).
    let run = |entry: Entry| {
        let mut d = Document::new("t");
        let page = d.add_page("p");
        let fid = d.alloc_id();
        let mut parent = Node::frame(fid, Rect::new(0.0, 0.0, 300.0, 100.0));
        let mut parent_al = auto(LayoutDirection::Horizontal, LayoutAlign::Start);
        parent_al.padding = [10.0, 10.0, 10.0, 10.0];
        parent_al.item_spacing = 20.0;
        parent.kind = NodeKind::Frame(FrameProps { auto_layout: Some(parent_al), ..Default::default() });
        let parent = d.insert(page, parent).unwrap();

        let lead_id = d.alloc_id();
        let lead = d.insert(parent, Node::rectangle(lead_id, Rect::new(0.0, 0.0, 40.0, 30.0))).unwrap();

        let nested_id = d.alloc_id();
        let mut nested = Node::frame(nested_id, Rect::new(0.0, 0.0, 80.0, 40.0));
        let mut nested_al = auto(LayoutDirection::Horizontal, LayoutAlign::Start);
        nested_al.primary_sizing = Sizing::Fixed;
        nested_al.counter_sizing = Sizing::Hug;
        nested_al.padding = [4.0, 8.0, 4.0, 8.0];
        nested_al.item_spacing = 12.0;
        nested.kind = NodeKind::Frame(FrameProps { auto_layout: Some(nested_al), ..Default::default() });
        nested.layout_child = LayoutChild { horizontal: Sizing::Fill, vertical: Sizing::Hug, ..Default::default() };
        let nested = d.insert(parent, nested).unwrap();

        let gc_fixed_id = d.alloc_id();
        let gc_fixed = d.insert(nested, Node::rectangle(gc_fixed_id, Rect::new(0.0, 0.0, 50.0, 24.0))).unwrap();

        let gc_fill_id = d.alloc_id();
        let mut gc_fill_node = Node::rectangle(gc_fill_id, Rect::new(0.0, 0.0, 20.0, 24.0));
        gc_fill_node.layout_child = LayoutChild { horizontal: Sizing::Fill, vertical: Sizing::Fixed, ..Default::default() };
        let gc_fill = d.insert(nested, gc_fill_node).unwrap();

        match entry {
            Entry::Editor => layout_node(&mut d, &mut Edit::new(), parent),
            Entry::Spec => d.relayout_subtree(parent),
            Entry::Changelog => designos_doc::relayout_subtree(&mut d, parent),
        }
        let rect_of = |id: NodeId| {
            let n = d.get(id).unwrap();
            [n.transform.tx, n.transform.ty, n.size.x, n.size.y]
        };
        (rect_of(parent), rect_of(lead), rect_of(nested), rect_of(gc_fixed), rect_of(gc_fill))
    };

    let editor = run(Entry::Editor);
    for entry in [Entry::Spec, Entry::Changelog] {
        assert_eq!(run(entry), editor, "{entry:?} disagrees with Editor");
    }
    assert_eq!(editor.0, [0.0, 0.0, 300.0, 100.0]);
    assert_eq!(editor.1, [10.0, 10.0, 40.0, 30.0]);
    assert_eq!(editor.2, [70.0, 10.0, 220.0, 32.0]);
    assert_eq!(editor.3, [8.0, 4.0, 50.0, 24.0]);
    assert_eq!(editor.4, [70.0, 4.0, 142.0, 24.0]);
}

/// Registers the two-mode number collection `col-space` (default `compact`, plus `spacious`) with one
/// variable per `(id, compact, spacious)`.
fn add_space_variables(d: &mut Document, vars: &[(&str, f32, f32)]) {
    use designos_doc::{Variable, VariableCollection, VariableKind, VariableMode, VariableValue};

    d.variable_collections.insert(
        "col-space".into(),
        VariableCollection {
            id: "col-space".into(),
            name: "Space".into(),
            modes: vec![VariableMode { id: "compact".into(), name: "Compact".into() }, VariableMode { id: "spacious".into(), name: "Spacious".into() }],
            default_mode: "compact".into(),
            variables: vars.iter().map(|(id, ..)| (*id).to_string()).collect(),
            key: None,
            source_library_key: None,
        },
    );
    for &(id, compact, spacious) in vars {
        d.variables.insert(
            id.into(),
            Variable {
                id: id.into(),
                name: id.into(),
                collection: "col-space".into(),
                kind: VariableKind::Number,
                values: [("compact".to_string(), VariableValue::Number(compact)), ("spacious".to_string(), VariableValue::Number(spacious))]
                    .into_iter()
                    .collect(),
                description: String::new(),
                scopes: Vec::new(),
                hidden_from_publishing: false,
                key: None,
                code_syntax: Default::default(),
            },
        );
    }
}

fn relayout_through(entry: Entry, d: &mut Document, root: NodeId) {
    match entry {
        Entry::Editor => layout_node(d, &mut Edit::new(), root),
        Entry::Spec => d.relayout_subtree(root),
        Entry::Changelog => designos_doc::relayout_subtree(d, root),
    }
}

fn box_of(d: &Document, id: NodeId) -> [f32; 4] {
    let n = d.get(id).unwrap();
    [n.transform.tx, n.transform.ty, n.size.x, n.size.y]
}

/// Runs `scene` through every entry point and requires the same result from each.
fn same_through_every_entry<T: PartialEq + std::fmt::Debug>(scene: impl Fn(Entry) -> T) -> T {
    let editor = scene(Entry::Editor);
    for entry in [Entry::Spec, Entry::Changelog] {
        assert_eq!(scene(entry), editor, "{entry:?} disagrees with Editor");
    }
    editor
}

#[test]
fn variable_bound_width_gap_and_padding_under_non_default_mode() {
    let run = |entry: Entry| {
        let mut d = Document::new("t");
        let page = d.add_page("p");
        add_space_variables(&mut d, &[("var-w", 40.0, 90.0), ("var-gap", 4.0, 16.0), ("var-px", 6.0, 20.0), ("var-py", 4.0, 12.0)]);

        let fid = d.alloc_id();
        let mut f = Node::frame(fid, Rect::new(0.0, 0.0, 10.0, 10.0));
        let mut al = auto(LayoutDirection::Horizontal, LayoutAlign::Start);
        al.primary_sizing = Sizing::Hug;
        al.counter_sizing = Sizing::Hug;
        al.item_spacing = 4.0;
        al.padding = [4.0, 6.0, 4.0, 6.0];
        f.kind = NodeKind::Frame(FrameProps { auto_layout: Some(al), ..Default::default() });
        f.variables.modes.insert("col-space".into(), "spacious".into());
        for (k, v) in [("itemSpacing", "var-gap"), ("paddingLeft", "var-px"), ("paddingRight", "var-px"), ("paddingTop", "var-py"), ("paddingBottom", "var-py")]
        {
            f.variables.bindings.insert(k.into(), v.into());
        }
        let f = d.insert(page, f).unwrap();

        let c0_id = d.alloc_id();
        let mut c0 = Node::rectangle(c0_id, Rect::new(0.0, 0.0, 40.0, 24.0));
        c0.variables.bindings.insert("width".into(), "var-w".into());
        let c0 = d.insert(f, c0).unwrap();

        let c1_id = d.alloc_id();
        let c1 = d.insert(f, Node::rectangle(c1_id, Rect::new(0.0, 0.0, 50.0, 24.0))).unwrap();

        relayout_through(entry, &mut d, f);
        let fs = d.get(f).unwrap().size;
        Solved { frame: (fs.x, fs.y), kids: vec![box_of(&d, c0), box_of(&d, c1)] }
    };

    let editor = same_through_every_entry(run);
    assert_eq!(editor.frame, (196.0, 48.0));
    assert_eq!(editor.kids[0], [20.0, 12.0, 90.0, 24.0]);
    assert_eq!(editor.kids[1], [126.0, 12.0, 50.0, 24.0]);
}

/// A fixed `parent_dir` flow frame holding a child frame (flow `child_dir`, the given sizing, one 10x10
/// rectangle inside) whose `width` and `height` are bound to variables, followed by a 30x10 rectangle.
/// Returns the child's and the sibling's `[x, y, w, h]`.
fn bound_child_frame_then_sibling(entry: Entry, parent_dir: LayoutDirection, child_dir: LayoutDirection, primary: Sizing, counter: Sizing) -> [[f32; 4]; 2] {
    let mut d = Document::new("t");
    let page = d.add_page("p");
    add_space_variables(&mut d, &[("var-w", 200.0, 300.0), ("var-h", 60.0, 90.0)]);

    let pid = d.alloc_id();
    let mut parent = Node::frame(pid, Rect::new(0.0, 0.0, 300.0, 300.0));
    parent.kind = NodeKind::Frame(FrameProps { auto_layout: Some(auto(parent_dir, LayoutAlign::Start)), ..Default::default() });
    let parent = d.insert(page, parent).unwrap();

    let cid = d.alloc_id();
    let mut child = Node::frame(cid, Rect::new(0.0, 0.0, 10.0, 10.0));
    let mut child_al = auto(child_dir, LayoutAlign::Start);
    child_al.primary_sizing = primary;
    child_al.counter_sizing = counter;
    child.kind = NodeKind::Frame(FrameProps { auto_layout: Some(child_al), ..Default::default() });
    child.variables.bindings.insert("width".into(), "var-w".into());
    child.variables.bindings.insert("height".into(), "var-h".into());
    let child = d.insert(parent, child).unwrap();
    let inner_id = d.alloc_id();
    d.insert(child, Node::rectangle(inner_id, Rect::new(0.0, 0.0, 10.0, 10.0))).unwrap();

    let sibling_id = d.alloc_id();
    let sibling = d.insert(parent, Node::rectangle(sibling_id, Rect::new(0.0, 0.0, 30.0, 10.0))).unwrap();

    relayout_through(entry, &mut d, parent);
    [box_of(&d, child), box_of(&d, sibling)]
}

#[test]
fn a_hugging_frame_ignores_width_and_height_variables_so_siblings_sit_behind_the_hugged_size() {
    // the frame hugs its 10x10 content on both axes: the bound 200 x 60 never reaches the parent, which
    // packs the next sibling right behind the hugged 10 (no hole), whichever way either flow runs
    for parent_dir in DIRECTIONS {
        for child_dir in DIRECTIONS {
            let got = same_through_every_entry(|entry| bound_child_frame_then_sibling(entry, parent_dir, child_dir, Sizing::Hug, Sizing::Hug));
            let sibling = if parent_dir == LayoutDirection::Horizontal { [10.0, 0.0, 30.0, 10.0] } else { [0.0, 10.0, 30.0, 10.0] };
            assert_eq!(got, [[0.0, 0.0, 10.0, 10.0], sibling], "parent {parent_dir:?} child {child_dir:?}");
        }
    }
}

#[test]
fn only_the_hugging_axis_of_a_frame_ignores_its_variable_the_fixed_axis_keeps_it() {
    // child hugs along its own flow (primary) and is fixed across it (counter)
    let cases = [
        // (parent flow, child flow, child box, sibling box)
        (LayoutDirection::Horizontal, LayoutDirection::Horizontal, [0.0, 0.0, 10.0, 60.0], [10.0, 0.0, 30.0, 10.0]),
        (LayoutDirection::Horizontal, LayoutDirection::Vertical, [0.0, 0.0, 200.0, 10.0], [200.0, 0.0, 30.0, 10.0]),
        (LayoutDirection::Vertical, LayoutDirection::Horizontal, [0.0, 0.0, 10.0, 60.0], [0.0, 60.0, 30.0, 10.0]),
        (LayoutDirection::Vertical, LayoutDirection::Vertical, [0.0, 0.0, 200.0, 10.0], [0.0, 10.0, 30.0, 10.0]),
    ];
    for (parent_dir, child_dir, child_box, sibling_box) in cases {
        let got = same_through_every_entry(|entry| bound_child_frame_then_sibling(entry, parent_dir, child_dir, Sizing::Hug, Sizing::Fixed));
        assert_eq!(got, [child_box, sibling_box], "parent {parent_dir:?} child {child_dir:?}");
    }
}

#[test]
fn a_fixed_frame_with_a_variable_bound_width_lays_its_children_out_in_the_bound_box() {
    // Fixed frame stored 10x10, width bound to 200: its Fill child follows the bound width in every entry
    let got = same_through_every_entry(|entry| {
        let mut d = Document::new("t");
        let page = d.add_page("p");
        add_space_variables(&mut d, &[("var-w", 200.0, 300.0)]);

        let pid = d.alloc_id();
        let mut parent = Node::frame(pid, Rect::new(0.0, 0.0, 300.0, 100.0));
        parent.kind = NodeKind::Frame(FrameProps { auto_layout: Some(auto(LayoutDirection::Horizontal, LayoutAlign::Start)), ..Default::default() });
        let parent = d.insert(page, parent).unwrap();

        let cid = d.alloc_id();
        let mut child = Node::frame(cid, Rect::new(0.0, 0.0, 10.0, 10.0));
        child.kind = NodeKind::Frame(FrameProps { auto_layout: Some(auto(LayoutDirection::Horizontal, LayoutAlign::Start)), ..Default::default() });
        child.variables.bindings.insert("width".into(), "var-w".into());
        let child = d.insert(parent, child).unwrap();
        let inner_id = d.alloc_id();
        let mut inner = Node::rectangle(inner_id, Rect::new(0.0, 0.0, 10.0, 10.0));
        inner.layout_child = LayoutChild { horizontal: Sizing::Fill, ..Default::default() };
        let inner = d.insert(child, inner).unwrap();

        let sibling_id = d.alloc_id();
        let sibling = d.insert(parent, Node::rectangle(sibling_id, Rect::new(0.0, 0.0, 30.0, 10.0))).unwrap();

        relayout_through(entry, &mut d, parent);
        [box_of(&d, child), box_of(&d, inner), box_of(&d, sibling)]
    });
    assert_eq!(got, [[0.0, 0.0, 200.0, 10.0], [0.0, 0.0, 200.0, 10.0], [200.0, 0.0, 30.0, 10.0]]);
}

#[test]
fn a_group_ignores_width_and_height_variables_because_it_always_fits_its_children() {
    // the group is stored 40x20 around its 40x20 rectangle; a bound 200 x 60 would only be refit away by
    // the editor, so no solver may let it push the sibling
    for parent_dir in DIRECTIONS {
        let got = same_through_every_entry(|entry| {
            let mut d = Document::new("t");
            let page = d.add_page("p");
            add_space_variables(&mut d, &[("var-w", 200.0, 300.0), ("var-h", 60.0, 90.0)]);

            let pid = d.alloc_id();
            let mut parent = Node::frame(pid, Rect::new(0.0, 0.0, 300.0, 300.0));
            parent.kind = NodeKind::Frame(FrameProps { auto_layout: Some(auto(parent_dir, LayoutAlign::Start)), ..Default::default() });
            let parent = d.insert(page, parent).unwrap();

            let gid = d.alloc_id();
            let mut group = Node::new(gid, NodeKind::Group);
            group.size = v2(40.0, 20.0);
            group.variables.bindings.insert("width".into(), "var-w".into());
            group.variables.bindings.insert("height".into(), "var-h".into());
            let group = d.insert(parent, group).unwrap();
            let inner_id = d.alloc_id();
            d.insert(group, Node::rectangle(inner_id, Rect::new(0.0, 0.0, 40.0, 20.0))).unwrap();

            let sibling_id = d.alloc_id();
            let sibling = d.insert(parent, Node::rectangle(sibling_id, Rect::new(0.0, 0.0, 30.0, 10.0))).unwrap();

            relayout_through(entry, &mut d, parent);
            [box_of(&d, group), box_of(&d, sibling)]
        });
        let sibling = if parent_dir == LayoutDirection::Horizontal { [40.0, 0.0, 30.0, 10.0] } else { [0.0, 20.0, 30.0, 10.0] };
        assert_eq!(got, [[0.0, 0.0, 40.0, 20.0], sibling], "parent {parent_dir:?}");
    }
}
