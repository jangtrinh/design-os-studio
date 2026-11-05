//! Grid cell placement for the one auto-layout solver (see `layout_solver_grid.rs`).
//!
//! Children are placed in three passes, as CSS Grid does, so an auto child never lands on a cell
//! another child asked for, whatever the child order:
//!
//! 1. children pinned to both a row and a column;
//! 2. children locked to a row (with auto column), in child order, each past the child this pass
//!    placed before it in the same row;
//! 3. column-locked and auto children together in child order, advancing a sparse `(row, col)` cursor.
//!
//! A column pin past the last column (a deleted column, a narrower grid) is no pin at all: the
//! child keeps any row lock and falls back to the free cells.

use std::collections::HashMap;

use crate::layout_solver_types::ChildIn;

/// Most columns and spanned rows one grid solves.
pub(crate) use crate::MAX_GRID_TRACKS;

/// A child's resolved cell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Placed {
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

/// First cell, scanning rows from `start_row` (and columns from `start_col` on `start_row`), where
/// a `col_span` x `row_span` block fits; `column` pins the scan to one column.
fn first_free_cell(
    occupied: &mut Occupancy,
    start_row: usize,
    start_col: usize,
    column: Option<usize>,
    cols: usize,
    col_span: usize,
    row_span: usize,
) -> (usize, usize) {
    let mut r = start_row;
    loop {
        ensure_rows(occupied, r + row_span);
        let first_col = if r == start_row { start_col } else { 0 };
        let candidates = match column {
            Some(c) => c..=c,
            None => first_col..=(cols - col_span),
        };
        for c in candidates {
            if (r..r + row_span).all(|rr| (c..c + col_span).all(|cc| !occupied[rr][cc])) {
                return (r, c);
            }
        }
        r += 1;
    }
}

/// Places every child of a `cols`-column grid exactly as the solver does; the result is in child order.
pub fn place_children(kids: &[ChildIn], cols: usize) -> Vec<Placed> {
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
    // pass 2: locked to a row only, in child order. A child starts past the one this pass placed
    // before it in the same row, so it never takes a hole that child left in front of itself.
    let mut row_cursor: HashMap<usize, usize> = HashMap::new();
    for (i, w) in wants.iter().enumerate() {
        if let (Some(locked_row), None) = (w.row, w.column) {
            let start_col = row_cursor.get(&locked_row).copied().unwrap_or(0);
            let (row, col) = first_free_cell(&mut occupied, locked_row, start_col, None, cols, w.col_span, w.row_span);
            put(i, row, col, &mut occupied);
            // a child with no room past the cursor moves down; it leaves the cursor of its locked row alone
            if row == locked_row {
                row_cursor.insert(locked_row, col + w.col_span);
            }
        }
    }
    // pass 3: column-locked and auto children together in child order, using a sparse cursor.
    let (mut cursor_row, mut cursor_col) = (0usize, 0usize);
    for (i, w) in wants.iter().enumerate() {
        if w.row.is_some() {
            continue;
        }
        if let Some(col) = w.column {
            if col < cursor_col {
                cursor_row += 1;
            }
            let (row, col) = first_free_cell(&mut occupied, cursor_row, col, Some(col), cols, w.col_span, w.row_span);
            put(i, row, col, &mut occupied);
            (cursor_row, cursor_col) = (row, col);
        } else {
            let (row, col) = first_free_cell(&mut occupied, cursor_row, cursor_col, None, cols, w.col_span, w.row_span);
            put(i, row, col, &mut occupied);
            (cursor_row, cursor_col) = (row, col);
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

    fn span(row: Option<u16>, column: Option<u16>, col_span: u16, row_span: u16) -> ChildIn {
        ChildIn { cell: GridCell { row, column, col_span, row_span, ..Default::default() }, ..Default::default() }
    }

    fn cells(kids: &[ChildIn], cols: usize) -> Vec<(usize, usize)> {
        place_children(kids, cols).iter().map(|p| (p.row, p.col)).collect()
    }

    #[test]
    fn auto_children_never_take_a_cell_a_later_child_pinned() {
        assert_eq!(cells(&[kid(None, None), kid(Some(0), Some(0)), kid(None, None)], 2), [(0, 1), (0, 0), (1, 0)]);
    }

    #[test]
    fn a_auto_children_before_and_after_column_locked_child_match_css_grid() {
        assert_eq!(cells(&[kid(None, None), kid(None, Some(0)), kid(None, None)], 2), [(0, 0), (1, 0), (1, 1)]);
        let c5 = [kid(None, None), kid(None, None), kid(None, None), kid(None, None), span(None, Some(1), 2, 1), kid(None, None)];
        assert_eq!(cells(&c5, 3), [(0, 0), (0, 1), (0, 2), (1, 0), (1, 1), (2, 0)]);
    }

    #[test]
    fn b_row_locked_child_is_still_placed_before_auto_children() {
        let kids = [kid(None, None), kid(None, Some(0)), kid(Some(0), None), kid(Some(0), Some(0))];
        assert_eq!(cells(&kids, 2), [(1, 0), (2, 0), (0, 1), (0, 0)]);
        assert_eq!(cells(&[kid(None, None), kid(Some(0), None)], 2), [(0, 1), (0, 0)]);
    }

    #[test]
    fn c_column_locked_child_after_cursor_passed_its_column_goes_to_next_row_sparse() {
        assert_eq!(cells(&[kid(None, Some(2)), kid(None, Some(0)), kid(None, None)], 3), [(0, 2), (1, 0), (1, 1)]);
        assert_eq!(cells(&[kid(None, None), kid(None, Some(2)), kid(None, Some(1))], 3), [(0, 0), (0, 2), (1, 1)]);
    }

    #[test]
    fn d_column_locked_child_with_column_span_places_in_child_order() {
        let kids = [kid(None, None), kid(None, None), span(None, Some(1), 2, 1), kid(None, None)];
        assert_eq!(cells(&kids, 3), [(0, 0), (0, 1), (1, 1), (2, 0)]);
        assert_eq!(cells(&[span(None, Some(1), 2, 1), kid(None, None)], 3), [(0, 1), (1, 0)]);
    }

    #[test]
    fn null_control_all_auto_and_all_pinned_grids_match_base() {
        let all_auto = [kid(None, None), span(None, None, 2, 1), kid(None, None), span(None, None, 1, 2), kid(None, None)];
        assert_eq!(cells(&all_auto, 3), [(0, 0), (0, 1), (1, 0), (1, 1), (1, 2)]);
        let all_pinned = [kid(Some(1), Some(2)), kid(Some(0), Some(0)), span(Some(0), Some(1), 2, 1)];
        assert_eq!(cells(&all_pinned, 3), [(1, 2), (0, 0), (0, 1)]);
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
    fn a_row_locked_child_starts_past_the_row_locked_children_placed_before_it() {
        // rows 0 and 1 of five columns: the rowspan-2 child blocks (1, 1), so the colspan-2 child
        // starts at column 2 and leaves (1, 0) empty; the next child of row 1 goes after it, not into the hole
        let kids = [span(Some(0), None, 1, 1), span(Some(0), None, 1, 2), span(Some(1), None, 2, 1), span(Some(1), None, 1, 1)];
        assert_eq!(cells(&kids, 5), [(0, 0), (0, 1), (1, 2), (1, 4)]);
    }

    #[test]
    fn a_row_locked_child_with_no_room_past_the_previous_one_moves_down() {
        // a pin in front of a wide child leaves a hole at (0, 0); the wide child sits at column 2 and
        // the grid has no implicit columns, so the next row-locked child takes the first free cell below
        let kids = [kid(Some(0), Some(1)), span(Some(0), None, 2, 1), span(Some(0), None, 1, 1)];
        assert_eq!(cells(&kids, 4), [(0, 1), (0, 2), (1, 0)]);
    }

    #[test]
    fn a_row_locked_child_that_moves_down_does_not_close_its_own_row() {
        // the wide child cannot start past column 0 of row 0 and drops to row 1; the next narrow
        // child still takes the free cell next to the first one in row 0, as before
        let kids = [kid(Some(0), None), span(Some(0), None, 3, 1), kid(Some(0), None)];
        assert_eq!(cells(&kids, 3), [(0, 0), (1, 0), (0, 1)]);
    }

    #[test]
    fn explicit_cells_may_overlap_each_other_as_in_css() {
        assert_eq!(cells(&[kid(Some(1), Some(1)), kid(Some(1), Some(1))], 2), [(1, 1), (1, 1)]);
    }
}
