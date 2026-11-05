//! #1462: a narrow window with the Agents or Tools panel open must not panic, and its columns must still fit.
//!
//! `f32::clamp(260, max)` panicked once the room left for the panel fell below the AI panel's minimum: below 710 px
//! with the inspector, below 469 px on a board. Every case is its own `Sim`, because egui keeps the widget rects of
//! earlier frames and a shared window would prove nothing. The window is first laid out wide, with the panel open, and
//! then shrunk: the panic fires on the first frame at the new width.
//!
//! On `main` 64f2dd07 the doc cases at 400, 469 and 709 px and the board cases at 400 and 468 px panic. 710 px (doc)
//! and 469 px (board) are the first widths that do not.

use super::*;
use crate::app::RailItem;

const HEIGHT: f32 = 800.0;
const WIDE: f32 = 1600.0;

#[derive(Clone, Copy, PartialEq)]
enum Layout {
    Doc,
    Board,
}

/// One frame in a window `width` points wide, like `Sim::frame`.
fn frame_at(s: &mut Sim, width: f32) {
    s.t += 0.05;
    let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(width, HEIGHT));
    let mut raw = egui::RawInput { screen_rect: Some(screen), time: Some(s.t), modifiers: s.mods, ..Default::default() };
    let ctx = s.ctx.clone();
    let shell = &mut s.shell;
    eframe::App::raw_input_hook(shell, &ctx, &mut raw);
    let _ = ctx.run_ui(raw, |ui| shell.draw(ui));
}

fn frames_at(s: &mut Sim, width: f32, n: usize) {
    for _ in 0..n {
        frame_at(s, width);
    }
}

/// A window with `rail` open at the default width, `ai_panel_width` 400 (the app default).
fn open_wide(layout: Layout, rail: RailItem) -> Sim {
    let fixture = match layout {
        Layout::Doc => "demo",
        Layout::Board => "board-empty",
    };
    let mut s = Sim::with(Options { fixture: Some(fixture.into()), ..Default::default() });
    s.use_stub_agent_clis();
    s.shell.app.ui.rail = Some(rail);
    s.shell.app.ui.left_collapsed = false;
    s.shell.app.ui.ai_panel_width = 400.0;
    frames_at(&mut s, WIDE, 3);
    s
}

/// Closes the agent processes a test started.
fn finish(mut s: Sim) {
    for mut tab in s.shell.app.terminal.tabs.drain(..) {
        let _ = tab.child.kill();
    }
}

fn overlap(a: Rect, b: Rect) -> bool {
    a.intersect(b).width() > 0.5 && a.intersect(b).height() > 0.5
}

/// The columns fit the window and do not overlap, and the thresholds are the ones the product decided.
fn assert_columns_fit(s: &Sim, layout: Layout, width: f32) {
    let regions = s.app().ui.regions;
    let window = Rect::from_min_size(Pos2::ZERO, Vec2::new(width, HEIGHT));
    let canvas = regions.canvas.expect("the canvas region");
    assert!(canvas.width() >= 0.0 && canvas.height() >= 0.0, "w {width}: the canvas has a negative size: {canvas:?}");
    let mut shown = vec![("rail", regions.rail), ("left panel", regions.left_panel), ("canvas", regions.canvas), ("inspector", regions.right_panel)];
    shown.retain(|(_, r)| r.is_some());
    for (name, r) in &shown {
        let r = r.unwrap();
        assert!(r.left() >= -0.01 && r.right() <= width + 0.01, "w {width}: the {name} {r:?} leaves the window {window:?}");
    }
    for (i, (a_name, a)) in shown.iter().enumerate() {
        for (b_name, b) in &shown[i + 1..] {
            assert!(!overlap(a.unwrap(), b.unwrap()), "w {width}: the {a_name} {a:?} overlaps the {b_name} {b:?}");
        }
    }
    // The inspector goes below 670 px, the left panel below 429 px. A board has no inspector.
    let inspector_expected = layout == Layout::Doc && width >= 670.0;
    assert_eq!(regions.right_panel.is_some(), inspector_expected, "w {width}: the inspector is shown from 670 px up, on a document only");
    let panel_expected = width >= 429.0;
    assert_eq!(regions.left_panel.is_some(), panel_expected, "w {width}: the left panel is shown from 429 px up");
    if let Some(panel) = regions.left_panel {
        assert!(panel.width() >= 220.0, "w {width}: the panel {panel:?} is narrower than its floor");
    }
    assert_eq!(s.app().terminal.surface_visible, panel_expected, "w {width}: the agent terminal is visible exactly when its panel is");
    assert!(!s.app().ui.left_collapsed, "w {width}: the window width never overwrites the user's collapse choice");
}

fn run(layout: Layout, rail: RailItem, width: f32) {
    let mut s = open_wide(layout, rail);
    let wide = s.app().ui.regions;
    frames_at(&mut s, width, 3);
    assert_columns_fit(&s, layout, width);
    // Widening again brings every column back: nothing was stored.
    frames_at(&mut s, WIDE, 3);
    assert_eq!(s.app().ui.regions, wide, "w {width}: back at {WIDE} px the layout is the one it was");
    finish(s);
}

macro_rules! narrow {
    ($($name:ident: $layout:ident, $rail:ident, $width:expr;)*) => {
        $(
            #[test]
            fn $name() {
                run(Layout::$layout, RailItem::$rail, $width);
            }
        )*
    };
}

narrow! {
    agents_panel_doc_w400: Doc, Agents, 400.0;
    agents_panel_doc_w469: Doc, Agents, 469.0;
    agents_panel_doc_w709: Doc, Agents, 709.0;
    agents_panel_doc_w710: Doc, Agents, 710.0;
    agents_panel_board_w400: Board, Agents, 400.0;
    agents_panel_board_w468: Board, Agents, 468.0;
    agents_panel_board_w469: Board, Agents, 469.0;
    agents_panel_board_w709: Board, Agents, 709.0;
    agents_panel_board_w710: Board, Agents, 710.0;
    tools_panel_doc_w400: Doc, Tools, 400.0;
    tools_panel_doc_w469: Doc, Tools, 469.0;
    tools_panel_doc_w709: Doc, Tools, 709.0;
    tools_panel_doc_w710: Doc, Tools, 710.0;
    tools_panel_board_w400: Board, Tools, 400.0;
    tools_panel_board_w468: Board, Tools, 468.0;
    tools_panel_board_w469: Board, Tools, 469.0;
    tools_panel_board_w709: Board, Tools, 709.0;
    tools_panel_board_w710: Board, Tools, 710.0;
}

/// The board's two popovers sit above their anchor and are kept 8 px inside the window. Their bounds
/// `left + 8` and `right - width - 8` cross once the window is narrower than the popover plus 16 px, which `clamp`
/// answers with a panic. Screenshot mode and tiling window managers go below the 900 px minimum window.
mod board_popovers {
    use super::*;
    use crate::board::{BarMenu, ToolMenu};
    use designos_doc::{Node, Rect as DRect, ShapeKind};

    /// The narrowest window, 200 px: narrower than the shape popover (216 + 16) and the brush popover (224 + 16).
    const NARROW: f32 = 200.0;

    fn board_sim() -> Sim {
        let mut s = Sim::with(Options { fixture: Some("board-empty".into()), ..Default::default() });
        s.use_stub_agent_clis();
        s
    }

    #[test]
    fn the_shape_popover_of_the_selection_bar_survives_a_window_narrower_than_itself() {
        let mut s = board_sim();
        let id = s.shell.app.doc.alloc_id();
        let page = s.app().page;
        // big enough to cover the canvas wherever it is panned, so the selection bar is on screen
        let node = Node::shape_with_text(id, DRect::new(-5000.0, -5000.0, 10000.0, 10000.0), ShapeKind::RoundedRect, "box");
        let id = s.shell.app.doc.insert(page, node).unwrap();
        s.shell.app.changed();
        s.shell.app.selection = vec![id];
        frames_at(&mut s, WIDE, 2);
        let anchor = Rect::from_min_size(Pos2::new(300.0, 300.0), Vec2::new(40.0, 28.0));
        s.shell.app.board.bar_menu = Some((BarMenu::Shape, anchor));
        frames_at(&mut s, NARROW, 3);
        assert!(s.app().board.bar_menu.is_some(), "premise: the popover was open while the narrow frames ran");
        finish(s);
    }

    #[test]
    fn the_brush_popover_of_the_toolbar_survives_a_window_narrower_than_itself() {
        let mut s = board_sim();
        frames_at(&mut s, WIDE, 2);
        let anchor = Rect::from_min_size(Pos2::new(300.0, 700.0), Vec2::new(40.0, 28.0));
        s.shell.app.board.tool_menu = Some((ToolMenu::Brush, anchor));
        frames_at(&mut s, NARROW, 3);
        assert!(s.app().board.tool_menu.is_some(), "premise: the popover was open while the narrow frames ran");
        finish(s);
    }
}
