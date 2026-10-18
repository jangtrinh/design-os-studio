//! The 2D CSS Grid half of the one auto-layout solver (see `layout_solver.rs`).

use crate::layout_solver_grid_placement::{place_children, Placed, MAX_GRID_TRACKS};
use crate::layout_solver_types::{ChildIn, LayoutOut, SizeLimits};
use crate::{AutoLayout, GridAutoTracks, GridTrack, Rect, Sizing, Vec2};

/// The width `k` asks of an auto column: its clamped width and margins. A child that fills the width and has margins on it
/// is stretched to its column minus those margins, so its stored width is that stretched one and counting it would pin the
/// column to `(column - margin) + margin` for good; it asks for its minimum width and margins only, like a counter-axis
/// Fill child in a flow line. Without margins the stored width still counts, so a hugging grid keeps the width of its
/// Fill child.
fn column_need(k: &ChildIn) -> f32 {
    let margins = k.margin[1] + k.margin[3];
    k.limits.clamp_w(if k.main_fill && margins > 0.0 { 0.0 } else { k.size.x }) + margins
}

/// The height `k` asks of an auto row (see [`column_need`]).
fn row_need(k: &ChildIn) -> f32 {
    let margins = k.margin[0] + k.margin[2];
    k.limits.clamp_h(if k.cross_fill && margins > 0.0 { 0.0 } else { k.size.y }) + margins
}

/// Widest child (single-column ones only) in column `c`.
fn widest_in_column(kids: &[ChildIn], placements: &[Placed], c: usize) -> f32 {
    placements.iter().zip(kids).filter(|(p, _)| p.col == c && p.col_span == 1).map(|(_, k)| column_need(k)).fold(0.0, f32::max)
}

/// A child that spans several rows: the rows it covers, its first column and its clamped height.
struct RowSpan {
    start: usize,
    end: usize,
    col: usize,
    need: f32,
}

/// Raises the rows a span covers until they hold `rem` (its height without the row gaps). Rows
/// flagged in `frozen` (Fixed tracks) and rows already at least an even share keep their height; the
/// remaining rows are leveled up to one common height. A row is never lowered.
fn level_span_rows(heights: &mut [f32], mut frozen: Vec<bool>, mut rem: f32) {
    rem -= heights.iter().zip(&frozen).filter(|(_, &f)| f).map(|(h, _)| h).sum::<f32>();
    loop {
        let unfrozen = frozen.iter().filter(|&&f| !f).count();
        if unfrozen == 0 || rem <= 0.0 {
            return;
        }
        let share = rem / (unfrozen as f32);
        let mut newly_frozen = false;
        for (h, f) in heights.iter().zip(frozen.iter_mut()) {
            if !*f && *h >= share {
                *f = true;
                rem -= *h;
                newly_frozen = true;
            }
        }
        if !newly_frozen {
            for (h, &f) in heights.iter_mut().zip(&frozen) {
                if !f {
                    *h = share;
                }
            }
            return;
        }
    }
}

/// Columns a grid solves: the explicit count, three when none is set, at most [`MAX_GRID_TRACKS`].
fn grid_columns(al: &AutoLayout) -> usize {
    let cols = if al.grid_columns > 0 { al.grid_columns as usize } else { 3 };
    cols.clamp(1, MAX_GRID_TRACKS)
}

/// Rows the placed children cover.
fn rows_covered(placements: &[Placed]) -> usize {
    placements.iter().map(|p| p.row + p.row_span).max().unwrap_or(0)
}

/// Rows the children of a grid fill on their own, placed exactly as [`compute_grid`] places them:
/// the rows it creates before `grid_rows` adds empty ones. A row track past this count is unused.
pub fn placed_grid_rows(al: &AutoLayout, kids: &[ChildIn]) -> usize {
    rows_covered(&place_children(kids, grid_columns(al)))
}

/// Solves a 2D CSS Grid auto-layout frame (`LayoutDirection::Grid`). Columns are the primary axis
/// (the width, as in Figma), rows the counter axis (the height). A child keeps its own size unless
/// it is Fill: a main Fill stretches across its cell's width, a cross Fill across the cell's height.
/// `own` is the frame's own min/max.
pub fn compute_grid(al: &AutoLayout, frame: Vec2, own: SizeLimits, kids: &[ChildIn]) -> LayoutOut {
    let frame = own.clamp(frame);
    let [pt, pr, pb, pl] = al.padding;
    let cols = grid_columns(al);
    let col_gap = al.item_spacing.max(0.0);
    let row_gap = if al.counter_spacing > 0.0 { al.counter_spacing } else { col_gap };

    if kids.is_empty() {
        let fw = if al.primary_sizing == Sizing::Hug { own.clamp_w(pl + pr) } else { frame.x };
        let fh = if al.counter_sizing == Sizing::Hug { own.clamp_h(pt + pb) } else { frame.y };
        return LayoutOut { frame: Vec2::new(fw, fh), rects: vec![] };
    }

    let placements = place_children(kids, cols);
    let rows = rows_covered(&placements).max(al.grid_rows as usize).max(1);

    // Resolve column widths from `grid_col_tracks` + `frame.x`.
    let inner_w = (frame.x - pl - pr - col_gap * (cols.saturating_sub(1) as f32)).max(0.0);
    let col_track = |c: usize| al.grid_col_tracks.get(c).copied().unwrap_or(GridTrack::Flex(1.0));
    let mut col_widths = vec![0.0f32; cols];
    let mut flex_total = 0.0f32;
    let mut fixed_used = 0.0f32;

    for (c, col_w) in col_widths.iter_mut().enumerate() {
        match col_track(c) {
            GridTrack::Fixed(w) => {
                *col_w = w.max(1.0);
                fixed_used += *col_w;
            }
            GridTrack::Hug => {
                *col_w = widest_in_column(kids, &placements, c).max(1.0);
                fixed_used += *col_w;
            }
            GridTrack::Flex(fr) => {
                flex_total += fr.max(0.1);
            }
        }
    }
    let rem_w = (inner_w - fixed_used).max(0.0);
    for (c, col_w) in col_widths.iter_mut().enumerate() {
        if let GridTrack::Flex(fr) = col_track(c) {
            *col_w = if al.primary_sizing == Sizing::Hug {
                widest_in_column(kids, &placements, c).max(40.0)
            } else if flex_total > 0.0 {
                (rem_w * (fr.max(0.1) / flex_total)).max(1.0)
            } else {
                (rem_w / cols as f32).max(1.0)
            };
        }
    }

    // A hugging frame raised by its own minimum width hands the extra to its flex columns.
    if al.primary_sizing == Sizing::Hug && flex_total > 0.0 {
        let natural = col_widths.iter().sum::<f32>() + col_gap * (cols.saturating_sub(1) as f32) + pl + pr;
        let extra = own.clamp_w(natural) - natural;
        if extra > 0.0 {
            for (c, col_w) in col_widths.iter_mut().enumerate() {
                if let GridTrack::Flex(fr) = col_track(c) {
                    *col_w += extra * fr.max(0.1) / flex_total;
                }
            }
        }
    }

    // Row heights: an explicit Fixed row track wins (or an explicit Fixed auto-track size for
    // implicit overflow rows `r >= al.grid_rows`); single-row items size their row from clamped
    // height, then spanning items distribute only the remaining needed height across shorter auto rows.
    let explicit_rows = al.grid_rows as usize;
    let fixed_row = |r: usize| -> Option<f32> {
        if explicit_rows > 0 && r < explicit_rows {
            match al.grid_row_tracks.get(r) {
                Some(GridTrack::Fixed(h)) => Some(h.max(0.0)),
                _ => None,
            }
        } else if explicit_rows == 0 {
            if let Some(GridTrack::Fixed(h)) = al.grid_row_tracks.get(r) {
                Some(h.max(0.0))
            } else if let GridAutoTracks::Track(GridTrack::Fixed(h)) = al.grid_auto_tracks {
                Some(h.max(0.0))
            } else {
                None
            }
        } else if let GridAutoTracks::Track(GridTrack::Fixed(h)) = al.grid_auto_tracks {
            Some(h.max(0.0))
        } else {
            None
        }
    };
    let mut row_heights = vec![0.0f32; rows];
    for (r, h) in row_heights.iter_mut().enumerate() {
        if let Some(fixed) = fixed_row(r) {
            *h = fixed;
        }
    }
    for (p, k) in placements.iter().zip(kids) {
        if p.row_span == 1 && p.row < rows && fixed_row(p.row).is_none() {
            row_heights[p.row] = row_heights[p.row].max(row_need(k));
        }
    }
    // Spans are settled in an order that depends on the spans alone, never on child order (layer order
    // is only z-order): fewer rows first, then the tallest first so the rows it raises already hold the
    // shorter spans that overlap it, then by position. Spans that tie on all four settle identically.
    let mut spans: Vec<RowSpan> = placements
        .iter()
        .zip(kids)
        .filter(|(p, _)| p.row_span > 1)
        .map(|(p, k)| RowSpan { start: p.row, end: (p.row + p.row_span).min(rows), col: p.col, need: row_need(k) })
        .filter(|s| s.end > s.start)
        .collect();
    spans.sort_by(|a, b| (a.end - a.start).cmp(&(b.end - b.start)).then(b.need.total_cmp(&a.need)).then(a.start.cmp(&b.start)).then(a.col.cmp(&b.col)));
    for s in &spans {
        let gaps = row_gap * ((s.end - s.start - 1) as f32);
        let frozen = (s.start..s.end).map(|r| fixed_row(r).is_some()).collect();
        level_span_rows(&mut row_heights[s.start..s.end], frozen, (s.need - gaps).max(0.0));
    }

    let mut col_x = vec![0.0f32; cols];
    let mut cx = pl;
    for (x, &w) in col_x.iter_mut().zip(&col_widths) {
        *x = cx;
        cx += w + col_gap;
    }

    let mut row_y = vec![0.0f32; rows];
    let mut cy = pt;
    for (y, &h) in row_y.iter_mut().zip(&row_heights) {
        *y = cy;
        cy += h + row_gap;
    }

    let mut rects = Vec::with_capacity(kids.len());
    for (p, k) in placements.iter().zip(kids) {
        let [mt, mr, mb, ml] = k.margin;
        let cell_w: f32 = (0..p.col_span).map(|dc| col_widths[p.col + dc]).sum::<f32>() + col_gap * (p.col_span.saturating_sub(1) as f32);
        let cell_h: f32 = (0..p.row_span).map(|dr| row_heights[p.row + dr]).sum::<f32>() + row_gap * (p.row_span.saturating_sub(1) as f32);
        let w = k.limits.clamp_w(if k.main_fill { (cell_w - ml - mr).max(0.0) } else { k.size.x });
        let h = k.limits.clamp_h(if k.cross_fill { (cell_h - mt - mb).max(0.0) } else { k.size.y });
        rects.push(Rect::new(col_x[p.col] + ml, row_y[p.row] + mt, w, h));
    }

    let total_w = col_widths.iter().sum::<f32>() + col_gap * (cols.saturating_sub(1) as f32) + pl + pr;
    let total_h = row_heights.iter().sum::<f32>() + row_gap * (rows.saturating_sub(1) as f32) + pt + pb;
    let out_w = if al.primary_sizing == Sizing::Hug { own.clamp_w(total_w) } else { frame.x };
    let out_h = if al.counter_sizing == Sizing::Hug { own.clamp_h(total_h) } else { frame.y };

    LayoutOut { frame: Vec2::new(out_w, out_h), rects }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::LayoutDirection;

    fn grid(columns: u32, primary: Sizing, counter: Sizing) -> AutoLayout {
        AutoLayout { direction: LayoutDirection::Grid, grid_columns: columns, primary_sizing: primary, counter_sizing: counter, ..Default::default() }
    }

    fn child(w: f32, h: f32) -> ChildIn {
        ChildIn { size: Vec2::new(w, h), ..Default::default() }
    }

    /// Solves twice the way a document does: the second pass reads the sizes the first pass wrote, after `edit` changed the kids.
    fn solve_then_edit_and_solve(al: &AutoLayout, frame: Vec2, mut kids: Vec<ChildIn>, edit: impl FnOnce(&mut [ChildIn])) -> (LayoutOut, LayoutOut) {
        let first = compute_grid(al, frame, SizeLimits::default(), &kids);
        for (k, r) in kids.iter_mut().zip(&first.rects) {
            k.size = Vec2::new(r.w, r.h);
        }
        edit(&mut kids);
        let second = compute_grid(al, first.frame, SizeLimits::default(), &kids);
        (first, second)
    }

    #[test]
    fn placed_rows_follow_the_placement_the_solver_uses() {
        use crate::layout_solver_types::GridCell;
        let al = grid(2, Sizing::Fixed, Sizing::Hug);
        assert_eq!(placed_grid_rows(&al, &[]), 0);
        // five auto children in two columns fill three rows, however many rows the frame asks for
        let five: Vec<ChildIn> = (0..5).map(|_| child(10.0, 10.0)).collect();
        assert_eq!(placed_grid_rows(&al, &five), 3);
        assert_eq!(placed_grid_rows(&AutoLayout { grid_rows: 9, ..al.clone() }, &five), 3);
        // a pinned child and a spanning child reach their last row
        let pinned = ChildIn { cell: GridCell { row: Some(4), ..Default::default() }, ..child(10.0, 10.0) };
        let tall = ChildIn { cell: GridCell { row_span: 3, ..Default::default() }, ..child(10.0, 10.0) };
        assert_eq!(placed_grid_rows(&al, &[child(10.0, 10.0), pinned]), 5);
        assert_eq!(placed_grid_rows(&al, &[tall]), 3);
        // the same rows the solver lays out: the frame grows by one gap per row that `grid_rows` does not add
        let out = compute_grid(&AutoLayout { counter_spacing: 5.0, ..al.clone() }, Vec2::new(100.0, 10.0), SizeLimits::default(), &five);
        assert_eq!(out.frame.y, 3.0 * 10.0 + 2.0 * 5.0);
    }

    #[test]
    fn an_auto_row_shrinks_back_around_a_cross_fill_child_with_margins() {
        // A is 30 x 100, B is 20 x 10 and fills its row's height between 30 + 30 of margin: the row is A's 100 tall, B stretches to 40
        let a = child(30.0, 100.0);
        let b = ChildIn { cross_fill: true, margin: [30.0, 0.0, 30.0, 0.0], ..child(20.0, 10.0) };
        let al = grid(2, Sizing::Fixed, Sizing::Hug);
        let (first, second) = solve_then_edit_and_solve(&al, Vec2::new(200.0, 1.0), vec![a, b], |kids| kids[0].size.y = 40.0);
        assert_eq!(first.frame.y, 100.0);
        assert_eq!(first.rects[1], Rect::new(100.0, 30.0, 20.0, 40.0));
        // A shrinks to 40: the row follows down to what B's margins need, B's stretched 40 does not hold it open
        assert_eq!(second.frame.y, 60.0);
        assert_eq!(second.rects[0], Rect::new(0.0, 0.0, 30.0, 40.0));
        assert_eq!(second.rects[1], Rect::new(100.0, 30.0, 20.0, 0.0));
    }

    #[test]
    fn a_fill_child_without_margins_still_counts_at_its_own_size() {
        let al = grid(1, Sizing::Hug, Sizing::Hug);
        let kids = [ChildIn { cross_fill: true, main_fill: true, ..child(100.0, 20.0) }];
        let out = compute_grid(&al, Vec2::new(1.0, 1.0), SizeLimits::default(), &kids);
        assert_eq!(out.frame, Vec2::new(100.0, 20.0));
        assert_eq!(out.rects[0], Rect::new(0.0, 0.0, 100.0, 20.0));
    }

    #[test]
    fn a_fill_child_asks_of_its_track_its_minimum_and_margins_only() {
        let al = grid(1, Sizing::Hug, Sizing::Hug);
        let kids = [ChildIn {
            cross_fill: true,
            main_fill: true,
            margin: [2.0, 3.0, 4.0, 5.0],
            limits: SizeLimits { min_w: Some(50.0), min_h: Some(30.0), ..Default::default() },
            ..child(10.0, 10.0)
        }];
        let out = compute_grid(&al, Vec2::new(1.0, 1.0), SizeLimits::default(), &kids);
        assert_eq!(out.frame, Vec2::new(58.0, 36.0));
        assert_eq!(out.rects[0], Rect::new(5.0, 2.0, 50.0, 30.0));
    }

    #[test]
    fn a_hugging_column_shrinks_back_around_a_main_fill_child_with_margins() {
        // one column: A is 100 x 30, B is 10 x 20 and fills the column's width between 30 + 30 of margin
        let a = child(100.0, 30.0);
        let b = ChildIn { main_fill: true, margin: [0.0, 30.0, 0.0, 30.0], ..child(10.0, 20.0) };
        let al = grid(1, Sizing::Hug, Sizing::Fixed);
        let (first, second) = solve_then_edit_and_solve(&al, Vec2::new(1.0, 200.0), vec![a, b], |kids| kids[0].size.x = 40.0);
        assert_eq!(first.frame.x, 100.0);
        assert_eq!(first.rects[1], Rect::new(30.0, 30.0, 40.0, 20.0));
        assert_eq!(second.frame.x, 60.0);
        assert_eq!(second.rects[0], Rect::new(0.0, 0.0, 40.0, 30.0));
        assert_eq!(second.rects[1], Rect::new(30.0, 30.0, 0.0, 20.0));
    }
}
