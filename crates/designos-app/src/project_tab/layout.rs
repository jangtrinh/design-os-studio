//! Where the Project tab puts its regions: rail, doc tree, reading column and "On this page".
//!
//! At 1440 px wide the tree is x48 w240, the reading column x399 w690 and the contents x1137 w192: the 930 px block
//! (690 + 48 gap + 192) is centred in the main region. Narrow windows follow the spec: the contents hide under 1200,
//! the column shrinks under 1000, and the tree becomes an overlay under 800.

use crate::theme;
use egui::{Pos2, Rect, Vec2};

pub const COLUMN_W: f32 = 690.0;
pub const CONTENTS_W: f32 = 192.0;
/// Gap between the reading column and "On this page" (`SPACE_REGION`).
pub const COLUMN_GAP: f32 = theme::SPACE_REGION;
/// Below this window width "On this page" is hidden and the column recentres.
pub const CONTENTS_MIN_WINDOW: f32 = 1200.0;
/// Below this window width the column fills the main region.
pub const FLUID_BELOW: f32 = 1000.0;
/// Below this window width the tree is an overlay.
pub const TREE_OVERLAY_BELOW: f32 = 800.0;
const MIN_COLUMN_W: f32 = 480.0;
const SIDE_PAD: f32 = theme::SPACE_SECTION;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProjectLayout {
    pub rail: Rect,
    /// The tree panel; under 800 px it is the overlay's rect, drawn only while the overlay is open.
    pub tree: Rect,
    pub tree_is_overlay: bool,
    /// Everything right of the rail (and of the tree when it is docked).
    pub main: Rect,
    pub column: Rect,
    pub contents: Option<Rect>,
}

/// `full` is the window area under the tab strip, rail included.
pub fn compute(full: Rect) -> ProjectLayout {
    let rail = Rect::from_min_max(full.min, Pos2::new(full.left() + theme::RAIL_WIDTH, full.bottom()));
    let tree_is_overlay = full.width() < TREE_OVERLAY_BELOW;
    let tree = Rect::from_min_size(Pos2::new(rail.right(), full.top()), Vec2::new(theme::LEFT_PANEL_WIDTH, full.height()));
    let main_left = if tree_is_overlay { rail.right() } else { tree.right() };
    let main = Rect::from_min_max(Pos2::new(main_left, full.top()), full.max);
    let top = full.top() + theme::SPACE_REGION;
    let show_contents = full.width() >= CONTENTS_MIN_WINDOW;
    let (left, width) = if full.width() < FLUID_BELOW {
        let w = (main.width() - SIDE_PAD * 2.0).max(MIN_COLUMN_W.min(main.width()));
        (main.left() + ((main.width() - w) / 2.0).max(0.0), w)
    } else if show_contents {
        let block = COLUMN_W + COLUMN_GAP + CONTENTS_W;
        (main.left() + ((main.width() - block) / 2.0).max(SIDE_PAD), COLUMN_W)
    } else {
        (main.left() + ((main.width() - COLUMN_W) / 2.0).max(SIDE_PAD), COLUMN_W)
    };
    let column = Rect::from_min_size(Pos2::new(left, top), Vec2::new(width, (full.bottom() - top).max(0.0)));
    let contents =
        show_contents.then(|| Rect::from_min_size(Pos2::new(column.right() + COLUMN_GAP, top), Vec2::new(CONTENTS_W, (full.bottom() - top).max(0.0))));
    ProjectLayout { rail, tree, tree_is_overlay, main, column, contents }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(width: f32) -> ProjectLayout {
        compute(Rect::from_min_size(Pos2::new(0.0, 32.0), Vec2::new(width, 868.0)))
    }

    #[test]
    fn at_1440_the_regions_sit_where_the_spec_puts_them() {
        let l = at(1440.0);
        assert_eq!((l.rail.left(), l.rail.width()), (0.0, 48.0));
        assert_eq!((l.tree.left(), l.tree.width()), (48.0, 240.0));
        assert_eq!((l.column.left(), l.column.width()), (399.0, 690.0));
        let c = l.contents.expect("contents at 1440");
        assert_eq!((c.left(), c.width()), (1137.0, 192.0));
        assert_eq!(l.column.top(), 80.0, "32 tab strip + 48 region padding");
        assert_eq!(c.top(), 80.0);
        assert!(!l.tree_is_overlay);
    }

    #[test]
    fn contents_hide_under_1200_and_the_column_recentres() {
        let l = at(1199.0);
        assert!(l.contents.is_none());
        assert_eq!(l.column.width(), 690.0);
        let main = l.main;
        assert!((l.column.center().x - main.center().x).abs() <= 1.0);
        assert!(at(1200.0).contents.is_some());
    }

    #[test]
    fn under_1000_the_column_fills_the_main_region_with_24_px_sides() {
        let l = at(900.0);
        assert_eq!(l.column.left() - l.main.left(), 24.0);
        assert_eq!(l.main.right() - l.column.right(), 24.0);
        let narrow = at(780.0);
        assert!(narrow.column.width() >= 480.0, "{:?}", narrow.column);
    }

    #[test]
    fn under_800_the_tree_is_an_overlay_and_the_column_starts_after_the_rail() {
        let l = at(700.0);
        assert!(l.tree_is_overlay);
        assert_eq!(l.main.left(), 48.0);
        assert!(l.column.left() >= 48.0);
    }
}
