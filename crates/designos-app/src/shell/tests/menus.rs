//! UI tests for the main menu, toolbar dropdowns and the command palette.

use super::*;
use crate::canvas::tools::Tool;
use crate::panels::toolbar;

fn open_main(s: &mut Sim) {
    s.click(Pos2::new(28.0, 24.0));
    assert_eq!(s.app().ui.menu, Some(MenuKind::Main));
}

/// Row tops of the main menu (x=12, y=44, 8px padding, 17px separator blocks): Back to files,
/// Actions, File, Edit, View, Object, Text, Arrange, Vector.
fn main_row_center(i: usize) -> f32 {
    let seps_before = [0, 1, 2, 2, 2, 2, 2, 2, 2][i] as f32;
    44.0 + 8.0 + i as f32 * 24.0 + seps_before * 17.0 + 12.0
}

#[test]
fn main_menu_rows_sit_where_figma_puts_them() {
    // Figma reference (800x600): Back to files 64, Actions 105, File 146, Edit 170 ... Vector 290.
    assert_eq!(main_row_center(0), 64.0);
    assert_eq!(main_row_center(1), 105.0);
    assert_eq!(main_row_center(2), 146.0);
    assert_eq!(main_row_center(3), 170.0);
    assert_eq!(main_row_center(8), 290.0);
}

#[test]
fn hovering_opens_a_submenu_and_clicking_a_leaf_runs_it() {
    let mut s = Sim::new();
    open_main(&mut s);
    assert!(s.app().selection.is_empty());
    s.mv(Pos2::new(100.0, main_row_center(3)));
    s.frame(vec![]);
    // Edit submenu: top aligned with the hovered row minus the 8px padding; "Select all" is its
    // 7th row after two 17px separators.
    let top = main_row_center(3) - 12.0 - theme::MENU_PADDING;
    let y = top + theme::MENU_PADDING + 6.0 * 24.0 + 2.0 * crate::widgets::menu::SEP_H + 12.0;
    s.mv(Pos2::new(300.0, y));
    s.click(Pos2::new(300.0, y));
    assert_eq!(s.app().ui.menu, None);
    assert!(!s.app().selection.is_empty(), "Select all ran");
}

#[test]
fn keyboard_navigates_submenus() {
    let mut s = Sim::new();
    open_main(&mut s);
    for _ in 0..4 {
        s.key(Key::ArrowDown);
    }
    // Highlight is on Edit; Right opens it, Down lands on the first enabled row, Enter runs it.
    s.key(Key::ArrowRight);
    s.key(Key::Enter);
    assert_eq!(s.app().ui.menu, None);
    assert!(!s.app().selection.is_empty(), "first enabled Edit entry is Select all");
}

#[test]
fn escape_closes_submenu_then_menu() {
    let mut s = Sim::new();
    open_main(&mut s);
    for _ in 0..3 {
        s.key(Key::ArrowDown);
    }
    s.key(Key::ArrowRight);
    s.key(Key::Escape);
    assert_eq!(s.app().ui.menu, Some(MenuKind::Main), "first Escape only closes the submenu");
    s.key(Key::Escape);
    assert_eq!(s.app().ui.menu, None);
    // Left closes a submenu too.
    open_main(&mut s);
    for _ in 0..3 {
        s.key(Key::ArrowDown);
    }
    s.key(Key::ArrowRight);
    s.key(Key::ArrowLeft);
    assert_eq!(s.app().ui.menu, Some(MenuKind::Main));
}

#[test]
fn shape_dropdown_picks_a_tool_and_remembers_it() {
    let mut s = Sim::new();
    let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(1600.0, 1088.0));
    let chev = toolbar::chevron_rect(screen, 2);
    s.click(chev.center());
    assert_eq!(s.app().ui.menu, Some(MenuKind::ToolGroup(2)));
    // Rectangle, Line, Ellipse, Polygon, Star, Image: the Arrow tool is hidden.
    let h = theme::MENU_PADDING * 2.0 + 6.0 * theme::MENU_ITEM_HEIGHT;
    let pos = toolbar::tool_menu_pos(screen, 2, h);
    // 232px wide panel, opening above the bar.
    assert_eq!(pos.y + h, toolbar::toolbar_rect(screen).top() + theme::TOOL_MENU_OVERLAP);
    s.click(Pos2::new(pos.x + 100.0, pos.y + theme::MENU_PADDING + 2.0 * theme::MENU_ITEM_HEIGHT + 12.0));
    assert_eq!(s.app().tool, Tool::Ellipse);
    assert_eq!(s.app().group_pick[2], 2);
    assert_eq!(s.app().ui.menu, None);
}

#[test]
fn shift_l_picks_line_because_the_arrow_tool_is_hidden() {
    let mut s = Sim::new();
    shift_key(&mut s, Key::L);
    assert_eq!(s.app().tool, Tool::Line);
}

#[test]
fn ctrl_k_opens_the_palette_and_enter_runs_the_filtered_command() {
    let mut s = Sim::new();
    s.cmd_key(Key::K);
    assert_eq!(s.app().ui.menu, Some(MenuKind::Palette));
    s.frame(vec![]);
    s.type_text("ellip");
    s.frame(vec![]);
    s.key(Key::Enter);
    assert_eq!(s.app().ui.menu, None);
    assert_eq!(s.app().tool, Tool::Ellipse);
}

#[test]
fn escape_closes_the_palette() {
    let mut s = Sim::new();
    s.cmd_key(Key::K);
    s.frame(vec![]);
    s.key(Key::Escape);
    assert_eq!(s.app().ui.menu, None);
}
