//! Grid cell placement for the one auto-layout solver (see `layout_solver_grid.rs`).
//!
//! Children are placed in three passes, as CSS Grid does, so an auto child never lands on a cell
//! another child asked for, whatever the child order:
//!
//! 1. children pinned to both a row and a column;
//! 2. children locked to a row or to a column, in child order;
//! 3. auto children, in child order, on the first free cell in reading order.
//!
//! A column pin past the last column (a deleted column, a narrower grid) is no pin at all: the
//! child keeps any row lock and falls back to the free cells.

use crate::layout_solver_types::ChildIn;

/// Most columns and spanned rows one grid solves.
pub(crate) const MAX_GRID_TRACKS: usize = 16;

/// A child's resolved cell.
#[derive(Clone, Copy)]
pub(crate) struct Placed {
    pub row: usize,
    pub col: usize,
    pub row_span: usize,
    pub col_span: usize,
}

type Occupancy = Vec<[bool; MAX_GRID_TRACKS]>;

/// What one child asks for, with its spans already limited to the grid.
struct Want {
    row: Option<usize>,
    column: Option<usize>,
    row_span: usize,
    col_span: usize,
}

impl Want {
    fn of(k: &ChildIn, cols: usize) -> Self {
        let col_span = (k.cell.col_span.max(1) as usize).min(cols);
        let row_span = (k.cell.row_span.max(1) as usize).min(MAX_GRID_TRACKS);
        // a column that no longer exists is dropped; one that exists but is too narrow for the span is pulled back in
        let column = k.cell.column.map(usize::from).filter(|c| *c < cols).map(|c| c.min(cols - col_span));
        Self { row: k.cell.row.map(usize::from), column, row_span, col_span }
    }
}

fn ensure_rows(occupied: &mut Occupancy, rows: usize) {
    if occupied.len() < rows {
        occupied.resize(rows, [false; MAX_GRID_TRACKS]);
    }
}

/// First cell, scanning rows from `start_row`, where a `col_span` x `row_span` block fits;
/// `column` pins the scan to one column.
fn first_free_cell(occupied: &mut Occupancy, start_row: usize, column: Option<usize>, cols: usize, col_span: usize, row_span: usize) -> (usize, usize) {
    let mut r = start_row;
    loop {
        ensure_rows(occupied, r + row_span);
        let candidates = match column {
            Some(c) => c..=c,
            None => 0..=(cols - col_span),
        };
        for c in candidates {
            if (r..r + row_span).all(|rr| (c..c + col_span).all(|cc| !occupied[rr][cc])) {
                return (r, c);
            }
        }
        r += 1;
    }
}

/// Places every child; the result is in child order.
pub(crate) fn place_children(kids: &[ChildIn], cols: usize) -> Vec<Placed> {
    let wants: Vec<Want> = kids.iter().map(|k| Want::of(k, cols)).collect();
    let mut occupied: Occupancy = Vec::new();
    // every slot is overwritten by exactly one of the three passes below
    let mut placed = vec![Placed { row: 0, col: 0, row_span: 1, col_span: 1 }; kids.len()];
    let mut put = |i: usize, row: usize, col: usize, occupied: &mut Occupancy| {
        let w = &wants[i];
        ensure_rows(occupied, row + w.row_span);
        for line in &mut occupied[row..row + w.row_span] {
            line[col..col + w.col_span].fill(true);
        }
        placed[i] = Placed { row, col, row_span: w.row_span, col_span: w.col_span };
    };
    // pass 1: both pinned. Explicit cells may overlap each other, as in CSS.
    for (i, w) in wants.iter().enumerate() {
        if let (Some(row), Some(col)) = (w.row, w.column) {
            put(i, row, col, &mut occupied);
        }
    }
    // pass 2: locked to a row or a column, pass 3: auto
    for locked_pass in [true, false] {
        for (i, w) in wants.iter().enumerate() {
            let pinned_both = w.row.is_some() && w.column.is_some();
            let locked = w.row.is_some() || w.column.is_some();
            if pinned_both || locked != locked_pass {
                continue;
            }
            let (row, col) = first_free_cell(&mut occupied, w.row.unwrap_or(0), w.column, cols, w.col_span, w.row_span);
            put(i, row, col, &mut occupied);
        }
    }
    placed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout_solver_types::GridCell;

    fn kid(row: Option<u16>, column: Option<u16>) -> ChildIn {
        ChildIn { cell: GridCell { row, column, ..Default::default() }, ..Default::default() }
    }

    fn cells(kids: &[ChildIn], cols: usize) -> Vec<(usize, usize)> {
        place_children(kids, cols).iter().map(|p| (p.row, p.col)).collect()
    }

    #[test]
    fn auto_children_never_take_a_cell_a_later_child_pinned() {
        assert_eq!(cells(&[kid(None, None), kid(Some(0), Some(0)), kid(None, None)], 2), [(0, 1), (0, 0), (1, 0)]);
    }

    #[test]
    fn locked_children_go_before_auto_ones_in_child_order() {
        let kids = [kid(None, None), kid(None, Some(0)), kid(Some(0), None), kid(Some(0), Some(0))];
        assert_eq!(cells(&kids, 2), [(1, 1), (1, 0), (0, 1), (0, 0)]);
    }

    #[test]
    fn a_missing_column_keeps_the_row_lock_and_a_wide_span_is_pulled_back_in() {
        // column 5 does not exist: the child keeps row 0 and takes its first free cell
        assert_eq!(cells(&[kid(Some(0), Some(0)), kid(Some(0), Some(5))], 2), [(0, 0), (0, 1)]);
        // with row 0 full it moves down instead of stacking on a pinned child
        assert_eq!(cells(&[kid(Some(0), Some(0)), kid(Some(0), Some(1)), kid(Some(0), Some(2))], 2), [(0, 0), (0, 1), (1, 0)]);
        let wide = ChildIn { cell: GridCell { column: Some(1), col_span: 2, ..Default::default() }, ..Default::default() };
        assert_eq!(cells(&[wide], 3), [(0, 1)]);
        let too_wide = ChildIn { cell: GridCell { column: Some(2), col_span: 2, ..Default::default() }, ..Default::default() };
        assert_eq!(cells(&[too_wide], 3), [(0, 1)]);
    }

    #[test]
    fn explicit_cells_may_overlap_each_other_as_in_css() {
        assert_eq!(cells(&[kid(Some(1), Some(1)), kid(Some(1), Some(1))], 2), [(1, 1), (1, 1)]);
    }
}
