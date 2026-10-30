//! UI tests for the main menu, toolbar dropdowns and the command palette.

use super::*;
use crate::canvas::tools::Tool;
use crate::panels::toolbar;

fn open_main(s: &mut Sim) {
    s.click(Pos2::new(28.0, 24.0));
    assert_eq!(s.app().ui.menu, Some(MenuKind::Main));
}

/// Item height of the Figma reference menu the row positions below were measured on (800x600 UI3 capture).
const FIGMA_MENU_ITEM_HEIGHT: f32 = 24.0;

/// Row centres of the main menu (x=12, y=44, 8px padding, 17px separator blocks) for items `item_h` tall:
/// Back to files, Actions, File, Edit, View, Object, Text, Arrange, Vector.
fn main_row_center_for(item_h: f32, i: usize) -> f32 {
    let seps_before = [0, 1, 2, 2, 2, 2, 2, 2, 2][i] as f32;
    44.0 + theme::MENU_PADDING + i as f32 * item_h + seps_before * crate::widgets::menu::SEP_H + item_h / 2.0
}

/// Row centres of the main menu as the app lays it out.
fn main_row_center(i: usize) -> f32 {
    main_row_center_for(theme::MENU_ITEM_HEIGHT, i)
}

/// Checks the reference numbers only, not the app: they were read off a menu of 24 px items, while the app's items are
/// `MENU_ITEM_HEIGHT` (32, the editor row size). What they still pin is the shared top padding and the separator block.
#[test]
fn figma_reference_menu_rows_use_the_shared_padding_and_separator() {
    // Figma reference (800x600): Back to files 64, Actions 105, File 146, Edit 170 ... Vector 290.
    let figma = |i| main_row_center_for(FIGMA_MENU_ITEM_HEIGHT, i);
    assert_eq!(figma(0), 64.0);
    assert_eq!(figma(1), 105.0);
    assert_eq!(figma(2), 146.0);
    assert_eq!(figma(3), 170.0);
    assert_eq!(figma(8), 290.0);
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
    let top = main_row_center(3) - theme::MENU_ITEM_HEIGHT / 2.0 - theme::MENU_PADDING;
    let y = top + theme::MENU_PADDING + 6.0 * theme::MENU_ITEM_HEIGHT + 2.0 * crate::widgets::menu::SEP_H + theme::MENU_ITEM_HEIGHT / 2.0;
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
    // Highlight is on Edit; Right opens it on its first enabled row (Paste, which is always on). Down skips the
    // disabled rows to Select all, and Enter runs it.
    s.key(Key::ArrowRight);
    s.key(Key::ArrowDown);
    s.key(Key::Enter);
    assert_eq!(s.app().ui.menu, None);
    assert!(!s.app().selection.is_empty(), "the next enabled Edit entry after Paste is Select all");
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
    let pos = toolbar::tool_menu_pos(chev, h);
    // 232px wide panel, opening above the bar.
    assert_eq!(pos.y + h, toolbar::toolbar_rect(screen).top() + theme::TOOL_MENU_OVERLAP);
    s.click(Pos2::new(pos.x + 100.0, pos.y + theme::MENU_PADDING + 2.0 * theme::MENU_ITEM_HEIGHT + 12.0));
    assert_eq!(s.app().tool, Tool::Ellipse);
    assert_eq!(s.app().group_pick[2], 2);
    assert_eq!(s.app().ui.menu, None);
}

/// Group of the Rectangle / Line / Ellipse / Polygon / Star / Image dropdown.
const SHAPE_GROUP: usize = 2;
/// Rows of that dropdown (the Arrow tool is hidden).
const ROW_RECTANGLE: usize = 0;
const ROW_ELLIPSE: usize = 2;

fn chevron_center(s: &Sim) -> Pos2 {
    s.rect_of(&format!("tool-chevron-{SHAPE_GROUP}")).center()
}

fn row_center(s: &Sim, row: usize) -> Pos2 {
    s.rect_of(&format!("menu-row-0-{row}")).center()
}

/// A Sim with the shape dropdown open, opened by a plain click on its chevron.
fn shape_menu_open() -> Sim {
    let mut s = Sim::new();
    s.click(chevron_center(&s));
    assert_eq!(s.app().ui.menu, Some(MenuKind::ToolGroup(SHAPE_GROUP)));
    s.frame(vec![]);
    s
}

fn assert_picked_ellipse(s: &Sim) {
    assert_eq!(s.app().tool, Tool::Ellipse, "the row under the release was picked");
    assert_eq!(s.app().group_pick[SHAPE_GROUP], ROW_ELLIPSE);
    assert_eq!(s.app().ui.menu, None, "picking closes the menu");
}

#[test]
fn a_row_is_picked_when_the_pointer_moves_before_the_release() {
    let mut s = shape_menu_open();
    let row = row_center(&s, ROW_ELLIPSE);
    s.mv(row);
    s.button(true);
    // 20 px along the row: far beyond the 6 px an egui click tolerates.
    s.mv(row + Vec2::new(20.0, 0.0));
    s.button(false);
    s.frame(vec![]);
    assert_picked_ellipse(&s);
}

#[test]
fn a_row_is_picked_after_a_long_press() {
    let mut s = shape_menu_open();
    let row = row_center(&s, ROW_ELLIPSE);
    s.mv(row);
    s.button(true);
    // 1.25 s at 50 ms a frame: past the 0.8 s an egui click tolerates.
    for _ in 0..25 {
        s.frame(vec![]);
    }
    s.button(false);
    s.frame(vec![]);
    assert_picked_ellipse(&s);
}

#[test]
fn press_on_the_chevron_slide_to_a_row_and_release_picks_it() {
    let mut s = Sim::new();
    s.mv(chevron_center(&s));
    s.button(true);
    assert_eq!(s.app().ui.menu, Some(MenuKind::ToolGroup(SHAPE_GROUP)), "the menu opens on the press");
    s.mv(row_center(&s, ROW_ELLIPSE));
    s.button(false);
    s.frame(vec![]);
    assert_picked_ellipse(&s);
}

#[test]
fn a_slide_from_one_row_to_another_in_one_frame_picks_the_row_under_the_release() {
    let mut s = shape_menu_open();
    let (from, to) = (row_center(&s, ROW_RECTANGLE), row_center(&s, ROW_ELLIPSE));
    s.mv(from);
    s.one_frame().press().move_to(to).release().send();
    s.frame(vec![]);
    assert_picked_ellipse(&s);
}

#[test]
fn press_move_and_release_in_one_frame_start_on_the_chevron_and_end_on_a_row() {
    // The row geometry exists only once the menu has been open: read it, then close the menu again.
    let mut s = shape_menu_open();
    let (chev, row) = (chevron_center(&s), row_center(&s, ROW_ELLIPSE));
    s.click(chev);
    assert_eq!(s.app().ui.menu, None, "a second press on the chevron closes the menu");
    s.mv(chev);
    s.one_frame().press().move_to(row).release().send();
    s.frame(vec![]);
    assert_picked_ellipse(&s);
}

#[test]
fn a_click_on_the_chevron_opens_the_menu_and_leaves_it_open() {
    let mut s = Sim::new();
    s.click(chevron_center(&s));
    s.frame(vec![]);
    assert_eq!(s.app().ui.menu, Some(MenuKind::ToolGroup(SHAPE_GROUP)), "the release on the chevron does not close it");
    assert_eq!(s.app().tool, Tool::Move);
}

#[test]
fn a_press_and_release_on_the_chevron_in_one_frame_opens_the_menu_once() {
    let mut s = Sim::new();
    s.mv(chevron_center(&s));
    s.one_frame().press().release().send();
    s.frame(vec![]);
    assert_eq!(s.app().ui.menu, Some(MenuKind::ToolGroup(SHAPE_GROUP)), "the press opened it and the release did not close it");
    assert_eq!(s.app().tool, Tool::Move);
}

#[test]
fn releasing_outside_every_row_picks_nothing() {
    let mut s = shape_menu_open();
    let row = row_center(&s, ROW_ELLIPSE);
    s.mv(row);
    s.button(true);
    s.mv(Pos2::new(row.x, row.y - 400.0));
    s.button(false);
    s.frame(vec![]);
    assert_eq!(s.app().tool, Tool::Move);
    assert_eq!(s.app().ui.menu, Some(MenuKind::ToolGroup(SHAPE_GROUP)), "the press began inside the menu, so it stays open");
}

#[test]
fn a_release_whose_press_came_before_the_menu_picks_nothing() {
    // The main menu opens on the release of a click. A later release the menu never saw the press of, here over the
    // leaf row "Back to files", must not pick it.
    let mut s = Sim::new();
    open_main(&mut s);
    s.mv(Pos2::new(100.0, main_row_center(0)));
    s.button(false);
    s.frame(vec![]);
    assert_eq!(s.app().ui.menu, Some(MenuKind::Main), "a release without a press in the menu picks nothing");
}

/// Hands the chevron of the shape group the keyboard focus, as Tab would.
fn focus_chevron(s: &mut Sim) {
    let id = s.ctx.data(|d| d.get_temp::<egui::Id>(egui::Id::new(("test-chevron-id", SHAPE_GROUP)))).expect("the toolbar recorded its chevron id");
    s.ctx.memory_mut(|m| m.request_focus(id));
    s.frame(vec![]);
    assert!(s.ctx.memory(|m| m.has_focus(id)), "the chevron holds the keyboard focus");
}

#[test]
fn space_and_enter_on_the_focused_chevron_open_and_close_the_menu() {
    for key in [Key::Space, Key::Enter] {
        let mut s = Sim::new();
        focus_chevron(&mut s);
        s.key(key);
        assert_eq!(s.app().ui.menu, Some(MenuKind::ToolGroup(SHAPE_GROUP)), "{key:?} opens the dropdown");
        s.frame(vec![]);
        assert_eq!(s.app().ui.menu, Some(MenuKind::ToolGroup(SHAPE_GROUP)), "the release of {key:?} does not close it");
        s.key(key);
        assert_eq!(s.app().ui.menu, None, "{key:?} again closes it");
    }
}

#[test]
fn a_screen_reader_click_on_the_chevron_opens_the_menu_once() {
    let mut s = Sim::new();
    let id = s.ctx.data(|d| d.get_temp::<egui::Id>(egui::Id::new(("test-chevron-id", SHAPE_GROUP)))).expect("the toolbar recorded its chevron id");
    let request = egui::accesskit::ActionRequest {
        action: egui::accesskit::Action::Click,
        target_tree: egui::accesskit::TreeId::ROOT,
        target_node: id.accesskit_id(),
        data: None,
    };
    s.frame(vec![egui::Event::AccessKitActionRequest(request)]);
    assert_eq!(s.app().ui.menu, Some(MenuKind::ToolGroup(SHAPE_GROUP)));
    s.frame(vec![]);
    assert_eq!(s.app().ui.menu, Some(MenuKind::ToolGroup(SHAPE_GROUP)), "one activation toggles once");
}

/// The open dropdown hangs off its chevron: its left edge starts at the group's button (the chevron's left edge minus
/// the gap) plus the dx, and its bottom edge sits above the chevron by the bar's inner padding minus the overlap.
fn assert_menu_hangs_off_the_chevron(s: &Sim) {
    let (chev, menu) = (s.rect_of(&format!("tool-chevron-{SHAPE_GROUP}")), s.rect_of("menu-level-0"));
    let want_left = chev.left() - theme::TOOL_CHEVRON_GAP + theme::TOOL_MENU_DX;
    let want_bottom = chev.top() - (theme::TOOLBELT_HEIGHT - theme::TOOL_BUTTON) / 2.0 + theme::TOOL_MENU_OVERLAP;
    assert!((menu.left() - want_left).abs() < 0.5, "menu left {} want {want_left} (chevron {chev:?})", menu.left());
    assert!((menu.bottom() - want_bottom).abs() < 0.5, "menu bottom {} want {want_bottom} (chevron {chev:?})", menu.bottom());
}

#[test]
fn the_dropdown_opens_against_its_chevron() {
    let s = shape_menu_open();
    assert_menu_hangs_off_the_chevron(&s);
}

#[test]
fn the_dropdown_opens_against_its_chevron_in_a_deck() {
    let mut s = Sim::new();
    let ctx = s.ctx.clone();
    crate::panels::menus::run(&mut s.shell.app, &ctx, crate::panels::menus::Action::NewDeck);
    s.frame(vec![]);
    s.frame(vec![]);
    assert!(s.app().is_deck());
    s.click(chevron_center(&s));
    assert_eq!(s.app().ui.menu, Some(MenuKind::ToolGroup(SHAPE_GROUP)));
    s.frame(vec![]);
    assert_menu_hangs_off_the_chevron(&s);
    // And a row of it can be picked.
    s.click(row_center(&s, ROW_ELLIPSE));
    assert_picked_ellipse(&s);
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
