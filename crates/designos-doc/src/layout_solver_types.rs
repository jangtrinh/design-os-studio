//! Input and output types of the one auto-layout solver (see `layout_solver.rs`).

use crate::{LayoutChild, Rect, Sizing, Vec2};

/// Optional min/max size of a layout child, in px (`x` = width, `y` = height).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SizeLimits {
    pub min_w: Option<f32>,
    pub max_w: Option<f32>,
    pub min_h: Option<f32>,
    pub max_h: Option<f32>,
}

fn clamp_opt(v: f32, min: Option<f32>, max: Option<f32>) -> f32 {
    let v = min.map_or(v, |lo| v.max(lo));
    max.map_or(v, |hi| v.min(hi))
}

impl SizeLimits {
    pub fn clamp_w(&self, w: f32) -> f32 {
        clamp_opt(w, self.min_w, self.max_w)
    }

    pub fn clamp_h(&self, h: f32) -> f32 {
        clamp_opt(h, self.min_h, self.max_h)
    }

    pub fn clamp(&self, size: Vec2) -> Vec2 {
        Vec2::new(self.clamp_w(size.x), self.clamp_h(size.y))
    }
}

impl From<&LayoutChild> for SizeLimits {
    fn from(lc: &LayoutChild) -> Self {
        Self { min_w: lc.min_w, max_w: lc.max_w, min_h: lc.min_h, max_h: lc.max_h }
    }
}

/// Where a child asks to sit in a `Grid` frame; ignored by flow frames.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GridCell {
    /// Explicit zero-based column, otherwise auto-placed.
    pub column: Option<u16>,
    /// Explicit zero-based row, otherwise auto-placed.
    pub row: Option<u16>,
    pub col_span: u16,
    pub row_span: u16,
}

impl Default for GridCell {
    fn default() -> Self {
        Self { column: None, row: None, col_span: 1, row_span: 1 }
    }
}

/// One participating child, as seen by the solver.
#[derive(Clone, Copy, Debug, Default)]
pub struct ChildIn {
    pub size: Vec2,
    /// Stretches along the primary axis (Fill or grow > 0).
    pub main_fill: bool,
    /// Stretches along the counter axis.
    pub cross_fill: bool,
    pub grow: f32,
    pub limits: SizeLimits,
    pub cell: GridCell,
    /// Per-child margins `[top, right, bottom, left]`.
    pub margin: [f32; 4],
}

impl ChildIn {
    /// Reads a child's layout properties. `horiz` is the flow's primary axis (grids count as horizontal).
    pub fn from_layout_child(size: Vec2, lc: &LayoutChild, horiz: bool) -> Self {
        let (main_s, cross_s) = if horiz { (lc.horizontal, lc.vertical) } else { (lc.vertical, lc.horizontal) };
        Self {
            size,
            main_fill: main_s == Sizing::Fill || lc.grow > 0.0,
            cross_fill: cross_s == Sizing::Fill,
            grow: lc.grow,
            limits: SizeLimits::from(lc),
            cell: GridCell { column: lc.grid_column, row: lc.grid_row, col_span: lc.col_span.max(1), row_span: lc.row_span.max(1) },
            margin: lc.margin,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct LayoutOut {
    pub frame: Vec2,
    /// Child rectangles in frame-local coordinates, same order as the input.
    pub rects: Vec<Rect>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limits_apply_the_minimum_first_so_the_maximum_wins_a_conflict() {
        let l = SizeLimits { min_w: Some(50.0), max_w: Some(40.0), min_h: None, max_h: Some(10.0) };
        assert_eq!(l.clamp(Vec2::new(5.0, 30.0)), Vec2::new(40.0, 10.0));
        assert_eq!(SizeLimits::default().clamp(Vec2::new(7.0, 9.0)), Vec2::new(7.0, 9.0));
    }

    #[test]
    fn child_in_reads_the_primary_axis_and_grid_cell_from_the_layout_child() {
        let lc = LayoutChild { horizontal: Sizing::Fill, grid_column: Some(2), col_span: 0, row_span: 3, max_h: Some(9.0), ..Default::default() };
        let horiz = ChildIn::from_layout_child(Vec2::new(4.0, 5.0), &lc, true);
        assert!(horiz.main_fill && !horiz.cross_fill);
        assert_eq!(horiz.cell, GridCell { column: Some(2), row: None, col_span: 1, row_span: 3 });
        assert_eq!(horiz.limits.max_h, Some(9.0));
        let vert = ChildIn::from_layout_child(Vec2::new(4.0, 5.0), &lc, false);
        assert!(!vert.main_fill && vert.cross_fill);
        let grown = ChildIn::from_layout_child(Vec2::ZERO, &LayoutChild { grow: 2.0, ..Default::default() }, false);
        assert!(grown.main_fill);
        assert_eq!(grown.grow, 2.0);
    }
}
