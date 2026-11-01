//! The shell's columns (rail, left panel, canvas, inspector) for any window width.
//!
//! One pure function, so no width can panic or give the canvas a negative size (#1462).

use crate::theme;
use crate::widgets::clamp::clamp_within;

/// Rail column with its border.
pub const RAIL_W: f32 = theme::RAIL_WIDTH + theme::BORDER_WIDTH;
/// Inspector column with its border.
pub const INSPECTOR_W: f32 = theme::RIGHT_PANEL_WIDTH + theme::BORDER_WIDTH;
/// What the canvas keeps before the left panel may take more.
pub const CANVAS_MIN: f32 = 160.0;
/// The narrowest a shown left panel gets, whatever it normally asks for.
pub const LEFT_FLOOR: f32 = 220.0;
/// The narrowest the Agents and Tools panel is dragged to when the window has room.
pub const AI_MIN: f32 = 260.0;
/// The narrowest the pages and layers panel is dragged to when the window has room.
pub const TREE_MIN: f32 = 200.0;

/// Which left panel is open.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LeftKind {
    /// Agents or Tools.
    Ai,
    /// Pages and layers.
    Tree,
}

impl LeftKind {
    /// The narrowest the panel is dragged to when the window has room.
    pub fn min_w(self) -> f32 {
        match self {
            LeftKind::Ai => AI_MIN,
            LeftKind::Tree => TREE_MIN,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Columns {
    /// The rail, at most the window.
    pub rail_w: f32,
    /// The left panel without its border; 0 while it is not shown.
    pub left_w: f32,
    /// The widest the left panel is dragged to.
    pub left_max_w: f32,
    /// True when the window is too narrow for any left panel: derived every frame, never stored as the user's choice.
    pub left_auto_collapsed: bool,
    /// The inspector with its border; 0 on a board or while it is hidden.
    pub right_w: f32,
    /// True when the inspector is dropped to make room.
    pub right_hidden: bool,
    /// What is left for the canvas, never negative.
    pub canvas_w: f32,
}

/// The columns of a `window_w` wide window. Each step runs only when the one before is not enough:
///
/// 1. The left panel takes `requested_left_w`, kept within `kind.min_w()` and what the canvas leaves it.
/// 2. Below its own minimum the panel shrinks, down to [`LEFT_FLOOR`] (the Agents panel at 260 px asked for 260).
/// 3. Without room for [`LEFT_FLOOR`] the inspector is hidden (a document below 670 px).
/// 4. Without room for [`LEFT_FLOOR`] after that, the left panel is auto-collapsed (below 429 px).
///
/// `left_collapsed` is the user's choice: a panel they hid needs no room, so it never hides the inspector for it.
/// No input, NaN and negative widths included, panics or makes a column negative.
pub fn columns(window_w: f32, board: bool, kind: LeftKind, requested_left_w: f32, left_collapsed: bool) -> Columns {
    let w = if window_w.is_finite() { window_w.max(0.0) } else { 0.0 };
    let rail_w = RAIL_W.min(w);
    let requested = if requested_left_w.is_finite() { requested_left_w } else { kind.min_w() };
    // What the left panel may take once the rail, the inspector `right` and the canvas minimum are paid.
    let room = |right: f32| w - rail_w - right - CANVAS_MIN;
    let needs = if left_collapsed { 0.0 } else { LEFT_FLOOR };
    let mut right_w = if board { 0.0 } else { INSPECTOR_W };
    let right_hidden = !board && room(right_w) < needs;
    if right_hidden {
        right_w = 0.0;
    }
    let room = room(right_w);
    let left_auto_collapsed = !left_collapsed && room < LEFT_FLOOR;
    let shown = !left_collapsed && !left_auto_collapsed;
    let left_w = if shown { clamp_within(requested, kind.min_w().min(room), room) } else { 0.0 };
    let panel_w = if shown { left_w + theme::BORDER_WIDTH } else { 0.0 };
    Columns {
        rail_w,
        left_w,
        left_max_w: room.max(LEFT_FLOOR),
        left_auto_collapsed,
        right_w,
        right_hidden,
        canvas_w: (w - rail_w - panel_w - right_w).max(0.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const KINDS: [LeftKind; 2] = [LeftKind::Ai, LeftKind::Tree];
    const REQUESTED: [f32; 7] = [f32::NAN, 0.0, 200.0, 260.0, 400.0, 1e9, f32::INFINITY];

    #[test]
    fn columns_never_panic_or_go_negative() {
        for w in -10..=3000 {
            for board in [false, true] {
                for kind in KINDS {
                    for requested in REQUESTED {
                        for collapsed in [false, true] {
                            let c = columns(w as f32, board, kind, requested, collapsed);
                            let all = [c.rail_w, c.left_w, c.left_max_w, c.right_w, c.canvas_w];
                            assert!(
                                all.iter().all(|v| v.is_finite() && *v >= 0.0),
                                "w {w} board {board} {kind:?} req {requested} collapsed {collapsed}: {c:?}"
                            );
                            let border = if c.left_w > 0.0 { theme::BORDER_WIDTH } else { 0.0 };
                            let used = c.rail_w + c.left_w + border + c.right_w + c.canvas_w;
                            assert!(
                                used <= (w as f32).max(0.0) + 0.01,
                                "w {w} board {board} {kind:?} req {requested}: the columns {used} overflow the window: {c:?}"
                            );
                            assert_eq!(
                                c.left_w == 0.0,
                                collapsed || c.left_auto_collapsed,
                                "w {w}: the panel is shown exactly when nothing collapsed it: {c:?}"
                            );
                            if c.left_w > 0.0 {
                                assert!(c.left_w >= kind.min_w().min(c.left_max_w), "w {w} {kind:?} req {requested}: a shown panel keeps its minimum: {c:?}");
                                assert!(c.left_w <= c.left_max_w, "w {w} {kind:?} req {requested}: a shown panel stays within its maximum: {c:?}");
                            }
                        }
                    }
                }
            }
        }
    }

    /// The thresholds the product decided: the inspector goes below 670 px, the left panel below 429 px.
    #[test]
    fn columns_degrade_in_priority_order() {
        let at = |w: f32, board: bool, collapsed: bool| columns(w, board, LeftKind::Ai, 400.0, collapsed);
        // Document with the Agents panel, 400 px asked for. Hiding the inspector frees 241 px, so the panel is wide again.
        for (w, left, inspector) in [(1010.0, 400.0, true), (900.0, 400.0, true), (710.0, 260.0, true), (709.0, 259.0, true), (670.0, 220.0, true)] {
            let c = at(w, false, false);
            assert!(c.right_hidden != inspector && c.right_w == if inspector { INSPECTOR_W } else { 0.0 }, "doc w {w}: {c:?}");
            assert_eq!(c.left_w, left, "doc w {w}: {c:?}");
            assert!(!c.left_auto_collapsed, "doc w {w}: {c:?}");
        }
        for (w, left) in [(669.0, 400.0), (600.0, 391.0), (500.0, 291.0), (429.0, 220.0)] {
            let c = at(w, false, false);
            assert!(c.right_hidden && c.right_w == 0.0, "doc w {w}: the inspector is hidden: {c:?}");
            assert_eq!(c.left_w, left, "doc w {w}: {c:?}");
            assert!(!c.left_auto_collapsed, "doc w {w}: {c:?}");
        }
        assert_eq!(at(429.0, false, false).left_w, 220.0);
        assert!(!at(429.0, false, false).left_auto_collapsed);
        assert!(at(428.0, false, false).left_auto_collapsed);
        assert_eq!(at(428.0, false, false).left_w, 0.0);
        assert_eq!(at(428.0, false, false).canvas_w, 428.0 - RAIL_W);
        // Board: no inspector, the panel gives way from 469 px.
        for w in [468.0, 469.0, 470.0, 709.0, 710.0] {
            let c = at(w, true, false);
            assert!(!c.right_hidden && c.right_w == 0.0, "board w {w}: {c:?}");
            assert_eq!(c.left_w, (w - RAIL_W - CANVAS_MIN).min(400.0), "board w {w}: {c:?}");
        }
        assert!(!at(429.0, true, false).left_auto_collapsed);
        assert!(at(428.0, true, false).left_auto_collapsed);
        // A panel the user hid never hides the inspector for room it does not take: from 450 px up.
        assert!(!at(450.0, false, true).right_hidden);
        assert!(at(449.0, false, true).right_hidden);
        assert!(!at(449.0, false, true).left_auto_collapsed);
    }

    /// With room the answer is the layout the window had before: pixel for pixel.
    #[test]
    fn columns_are_unchanged_when_there_is_room() {
        for w in [900.0, 1000.0, 1200.0, 1600.0, 2560.0] {
            for board in [false, true] {
                for kind in KINDS {
                    for requested in [200.0, 240.0, 260.0, 400.0, 900.0] {
                        let c = columns(w, board, kind, requested, false);
                        let right_w = if board { 0.0 } else { INSPECTOR_W };
                        let max_left_w = (w - right_w - RAIL_W - 160.0).max(220.0);
                        let old = requested.clamp(kind.min_w(), max_left_w);
                        assert_eq!(c.left_w, old, "w {w} board {board} {kind:?} req {requested}");
                        assert_eq!(c.right_w, right_w);
                        assert_eq!(c.left_max_w, max_left_w);
                        assert!(!c.left_auto_collapsed && !c.right_hidden);
                        assert_eq!(c.canvas_w, w - RAIL_W - old - theme::BORDER_WIDTH - right_w);
                    }
                }
            }
        }
    }

    #[test]
    fn a_nan_window_or_request_gives_a_usable_layout() {
        let c = columns(f32::NAN, false, LeftKind::Ai, f32::NAN, false);
        assert!(c.left_auto_collapsed && c.canvas_w == 0.0 && c.rail_w == 0.0, "{c:?}");
        let c = columns(1600.0, false, LeftKind::Ai, f32::NAN, false);
        assert_eq!(c.left_w, AI_MIN, "{c:?}");
    }
}
