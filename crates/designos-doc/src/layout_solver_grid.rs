//! The 2D CSS Grid half of the one auto-layout solver (see `layout_solver.rs`).

use crate::layout_solver_grid_placement::{place_children, Placed, MAX_GRID_TRACKS};
use crate::layout_solver_types::{ChildIn, LayoutOut, SizeLimits};
use crate::{AutoLayout, GridTrack, Rect, Sizing, Vec2};

/// Widest natural child (single-column ones only) in column `c`.
fn widest_in_column(kids: &[ChildIn], placements: &[Placed], c: usize) -> f32 {
    placements.iter().zip(kids).filter(|(p, _)| p.col == c && p.col_span == 1).map(|(_, k)| k.size.x).fold(0.0, f32::max)
}

/// Solves a 2D CSS Grid auto-layout frame (`LayoutDirection::Grid`). Columns are the primary axis
/// (the width, as in Figma), rows the counter axis (the height). A child keeps its own size unless
/// it is Fill: a main Fill stretches across its cell's width, a cross Fill across the cell's height.
/// `own` is the frame's own min/max.
pub fn compute_grid(al: &AutoLayout, frame: Vec2, own: SizeLimits, kids: &[ChildIn]) -> LayoutOut {
    let frame = own.clamp(frame);
    let [pt, pr, pb, pl] = al.padding;
    let cols = if al.grid_columns > 0 { al.grid_columns as usize } else { 3 };
    let cols = cols.clamp(1, MAX_GRID_TRACKS);
    let col_gap = al.item_spacing.max(0.0);
    let row_gap = if al.counter_spacing > 0.0 { al.counter_spacing } else { col_gap };

    if kids.is_empty() {
        let fw = if al.primary_sizing == Sizing::Hug { own.clamp_w(pl + pr) } else { frame.x };
        let fh = if al.counter_sizing == Sizing::Hug { own.clamp_h(pt + pb) } else { frame.y };
        return LayoutOut { frame: Vec2::new(fw, fh), rects: vec![] };
    }

    let placements = place_children(kids, cols);
    let max_row = placements.iter().map(|p| p.row + p.row_span).max().unwrap_or(0);
    let rows = max_row.max(al.grid_rows as usize).max(1);

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

    // Row heights: a Fixed row track wins, every other row takes its tallest child.
    let fixed_row = |r: usize| match al.grid_row_tracks.get(r) {
        Some(GridTrack::Fixed(h)) => Some(h.max(0.0)),
        _ => None,
    };
    let mut row_heights = vec![0.0f32; rows];
    for (r, h) in row_heights.iter_mut().enumerate() {
        if let Some(fixed) = fixed_row(r) {
            *h = fixed;
        }
    }
    for (p, k) in placements.iter().zip(kids) {
        let per_row = (k.size.y - row_gap * (p.row_span.saturating_sub(1) as f32)).max(0.0) / (p.row_span as f32);
        for r in p.row..(p.row + p.row_span).min(rows) {
            if fixed_row(r).is_none() {
                row_heights[r] = row_heights[r].max(per_row);
            }
        }
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
        let cell_w: f32 = (0..p.col_span).map(|dc| col_widths[p.col + dc]).sum::<f32>() + col_gap * (p.col_span.saturating_sub(1) as f32);
        let cell_h: f32 = (0..p.row_span).map(|dr| row_heights[p.row + dr]).sum::<f32>() + row_gap * (p.row_span.saturating_sub(1) as f32);
        let w = k.limits.clamp_w(if k.main_fill { cell_w } else { k.size.x });
        let h = k.limits.clamp_h(if k.cross_fill { cell_h } else { k.size.y });
        rects.push(Rect::new(col_x[p.col], row_y[p.row], w, h));
    }

    let total_w = col_widths.iter().sum::<f32>() + col_gap * (cols.saturating_sub(1) as f32) + pl + pr;
    let total_h = row_heights.iter().sum::<f32>() + row_gap * (rows.saturating_sub(1) as f32) + pt + pb;
    let out_w = if al.primary_sizing == Sizing::Hug { own.clamp_w(total_w) } else { frame.x };
    let out_h = if al.counter_sizing == Sizing::Hug { own.clamp_h(total_h) } else { frame.y };

    LayoutOut { frame: Vec2::new(out_w, out_h), rects }
}
