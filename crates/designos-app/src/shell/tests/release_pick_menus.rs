//! The board toolbar popovers (brush, shape, stamp), the Zoom menu (board pill and right panel) and the Cmd+right-click
//! layer picker pick on the release of a press the menu saw, wherever the pointer travelled and however long it took.
//! Menus built on `popup_tree` already do; these tests hold the three hand-built ones to the same rule.

use super::*;
use crate::app::MenuKind;
use crate::board::ToolMenu;
use crate::canvas::tools::Tool;
use designos_doc::{ShapeKind, STAMP_KINDS};

#[derive(Clone, Copy, Debug)]
enum Menu {
    Brush,
    Shape,
    Stamp,
    ZoomPill,
    ZoomPanel,
    Layers,
}

/// Records a failed check and carries on, so one run names every menu that breaks.
macro_rules! soft {
    ($bad:ident, $cond:expr, $($msg:tt)+) => {
        if !$cond {
            $bad.push(format!($($msg)+));
        }
    };
}

const ALL: [Menu; 6] = [Menu::Brush, Menu::Shape, Menu::Stamp, Menu::ZoomPill, Menu::ZoomPanel, Menu::Layers];
/// Menus a press on a button opens (the layer picker opens from a Cmd+right-click instead).
const OPENED_BY_PRESS: [Menu; 5] = [Menu::Brush, Menu::Shape, Menu::Stamp, Menu::ZoomPill, Menu::ZoomPanel];

fn board_sim() -> Sim {
    Sim::with(Options { fixture: Some("board-empty".into()), ..Default::default() })
}

impl Menu {
    fn sim(self) -> Sim {
        match self {
            Menu::Brush | Menu::Shape | Menu::Stamp | Menu::ZoomPill => board_sim(),
            Menu::ZoomPanel | Menu::Layers => Sim::new(),
        }
    }

    /// Where a press opens the menu. The right panel's zoom control records no rect (that would change its layout
    /// snapshot), so it is found by its painted percentage.
    fn opener_at(self, s: &Sim) -> Pos2 {
        match self {
            Menu::ZoomPanel => {
                let label = format!("{}%", (s.app().view.zoom * 100.0).round());
                fn find(shape: &egui::Shape, label: &str) -> Option<Pos2> {
                    match shape {
                        egui::Shape::Text(t) if t.galley.text() == label && t.pos.x > 1300.0 => Some(t.pos + t.galley.size() / 2.0),
                        egui::Shape::Vec(v) => v.iter().find_map(|c| find(c, label)),
                        _ => None,
                    }
                }
                s.shapes.iter().find_map(|c| find(&c.shape, &label)).expect("the right panel paints the zoom percentage")
            }
            _ => center(s, self.opener()),
        }
    }

    fn opener(self) -> &'static str {
        match self {
            Menu::Brush => "board-chevron-Brush",
            Menu::Shape => "board-chevron-Shape",
            Menu::Stamp => "board-chevron-Stamp",
            Menu::ZoomPill => "board-zoom",
            Menu::ZoomPanel => unreachable!("found by its label, see opener_at"),
            Menu::Layers => unreachable!("the layer picker has no button"),
        }
    }

    /// Names of the row the test presses on and the row it expects to be picked.
    fn rows(self) -> (String, String) {
        let shape = |k: ShapeKind| format!("board-shape-{k:?}");
        match self {
            Menu::Brush => ("board-brush-Marker".into(), "board-brush-Highlighter".into()),
            Menu::Shape => (shape(ShapeKind::ALL[0]), shape(ShapeKind::Diamond)),
            Menu::Stamp => (format!("board-stamp-{}", STAMP_KINDS[0]), format!("board-stamp-{}", STAMP_KINDS[2])),
            // "Snap to pixel grid" is pressed on, "Pixel grid" is picked.
            Menu::ZoomPill | Menu::ZoomPanel => ("label:Snap to pixel grid".into(), "label:Pixel grid".into()),
            Menu::Layers => ("layer-row-0".into(), "layer-row-2".into()),
        }
    }

    /// Widget id of the expected row, to put keyboard focus on it.
    fn expected_row_id(self, s: &Sim) -> egui::Id {
        let area = egui::Id::new("board-tool-menu");
        match self {
            Menu::Brush => area.with(("board-brush", 1)),
            Menu::Shape => area.with(("board-shape", ShapeKind::ALL.iter().position(|k| *k == ShapeKind::Diamond).expect("Diamond is a shape"))),
            Menu::Stamp => area.with(("board-stamp", 2)),
            Menu::ZoomPill | Menu::ZoomPanel => crate::panels::menus::menu_id(MenuKind::Zoom).with(("zrow", 6)),
            Menu::Layers => egui::Id::new(("layer-picker-row", s.by_name("Card"))),
        }
    }

    fn is_open(self, s: &Sim) -> bool {
        match self {
            Menu::Brush | Menu::Shape | Menu::Stamp => s.app().board.tool_menu.is_some(),
            Menu::ZoomPill | Menu::ZoomPanel => s.app().ui.menu == Some(MenuKind::Zoom),
            Menu::Layers => s.app().ui.layer_picker.is_some(),
        }
    }

    /// Opens the menu without caring how: a click on its button, or the picker's state set directly.
    fn open(self, s: &mut Sim) {
        match self {
            Menu::Layers => {
                let ids = vec![s.by_name("Welcome"), s.by_name("Hero"), s.by_name("Card")];
                s.shell.app.selection.clear();
                s.shell.app.ui.layer_picker = Some((Pos2::new(700.0, 400.0), ids));
                settle(s);
            }
            _ => {
                let at = self.opener_at(s);
                s.click(at);
                settle(s);
            }
        }
        assert!(self.is_open(s), "{self:?} is open");
    }

    /// What picking the expected row (and only that row) leaves behind.
    fn picked_expected(self, s: &Sim) -> bool {
        match self {
            Menu::Brush => s.app().board.brush == Tool::Highlighter && s.app().tool == Tool::Highlighter,
            Menu::Shape => s.app().board.shape == ShapeKind::Diamond && s.app().tool == Tool::BoardShape,
            Menu::Stamp => s.app().board.stamp == STAMP_KINDS[2] && s.app().tool == Tool::Stamp,
            Menu::ZoomPill | Menu::ZoomPanel => !s.app().ui.pixel_grid,
            Menu::Layers => s.app().selection == vec![s.by_name("Card")],
        }
    }

    /// Whether the row that was only pressed on fired: the Zoom menu's other toggle moved, or another layer was picked.
    fn picked_the_pressed_row(self, s: &Sim) -> bool {
        match self {
            Menu::Brush => s.app().board.brush == Tool::Marker && s.app().tool == Tool::Marker,
            Menu::Shape => s.app().board.shape == ShapeKind::ALL[0] && s.app().tool == Tool::BoardShape,
            Menu::Stamp => s.app().board.stamp == STAMP_KINDS[0] && s.app().tool == Tool::Stamp,
            Menu::ZoomPill | Menu::ZoomPanel => s.app().ui.no_pixel_snap,
            Menu::Layers => s.app().selection == vec![s.by_name("Welcome")],
        }
    }

    /// Everything a pick in any of the menus can change: tool, brush, ink, shape, stamp, zoom, the Zoom menu's toggles
    /// and the selection. Two equal snapshots mean no row ran in between.
    fn state(s: &Sim) -> String {
        let a = s.app();
        format!(
            "{:?} {:?} {:?} {:?} {} {} {} {} {} {} {:?}",
            a.tool,
            a.board.brush,
            a.board.ink,
            a.board.shape,
            a.board.stamp,
            a.view.zoom,
            a.ui.pixel_grid,
            a.ui.no_pixel_snap,
            a.ui.hide_layout_grids,
            a.ui.rulers,
            a.selection
        )
    }

    fn nothing_picked(self, s: &Sim) -> bool {
        match self {
            Menu::Brush | Menu::Shape | Menu::Stamp => s.app().tool != Tool::Highlighter && s.app().tool != Tool::BoardShape && s.app().tool != Tool::Stamp,
            Menu::ZoomPill | Menu::ZoomPanel => s.app().ui.pixel_grid && !s.app().ui.no_pixel_snap,
            Menu::Layers => s.app().selection.is_empty(),
        }
    }
}

fn settle(s: &mut Sim) {
    for _ in 0..6 {
        s.frame(vec![]);
    }
}

/// The Zoom menu records no row rects (that would change its layout snapshot), so its rows are found by the label
/// painted on them: a name `label:<text>` stands for the row that carries it.
const ZOOM_LABELS: [&str; 11] = [
    "Zoom in",
    "Zoom out",
    "Zoom to fit",
    "Zoom to 50%",
    "Zoom to 100%",
    "Zoom to 200%",
    "Pixel grid",
    "Snap to pixel grid",
    "Layout guides",
    "Rulers",
    "Comments",
];

fn rect_named(s: &Sim, name: &str) -> Rect {
    let Some(label) = name.strip_prefix("label:") else { return s.rect_of(name) };
    fn find(shape: &egui::Shape, label: &str) -> Option<Rect> {
        match shape {
            egui::Shape::Text(t) if t.galley.text() == label => Some(Rect::from_min_size(t.pos, t.galley.size())),
            egui::Shape::Vec(v) => v.iter().find_map(|c| find(c, label)),
            _ => None,
        }
    }
    let text = s.shapes.iter().rev().find_map(|c| find(&c.shape, label)).unwrap_or_else(|| panic!("no row labelled {label}"));
    // The label starts 32 px into a 212 px wide row.
    Rect::from_min_size(Pos2::new(text.left() - 32.0, text.center().y - crate::theme::MENU_ITEM_HEIGHT / 2.0), Vec2::new(212.0, crate::theme::MENU_ITEM_HEIGHT))
}

fn center(s: &Sim, name: &str) -> Pos2 {
    rect_named(s, name).center()
}

/// Press on `from`, drag at least 40 px, hold for `hold_s`, release on `to`: one gesture, spread over many frames.
fn slide(s: &mut Sim, from: Pos2, to: Pos2, hold_s: f32) {
    s.mv(from);
    s.one_frame().press().send();
    let away = from + (to - from).normalized() * 40.0;
    s.mv(away);
    for _ in 0..(hold_s / 0.05).round() as usize {
        s.frame(vec![]);
    }
    s.one_frame().move_to(to).release().send();
    settle(s);
}

#[test]
fn a_release_on_another_row_picks_it_after_a_drag_and_a_long_hold() {
    let mut bad = Vec::new();
    for menu in ALL {
        let mut s = menu.sim();
        menu.open(&mut s);
        let (a, b) = menu.rows();
        let (from, to) = (center(&s, &a), center(&s, &b));
        slide(&mut s, from, to, 1.2);
        soft!(bad, menu.picked_expected(&s), "{menu:?}: the row under the release is not picked");
        soft!(bad, !menu.picked_the_pressed_row(&s), "{menu:?}: the row the press started on fired");
    }
    assert!(bad.is_empty(), "{bad:#?}");
}

#[test]
fn the_press_that_opens_a_menu_can_slide_onto_a_row_and_pick_it_on_release() {
    let mut bad = Vec::new();
    for menu in OPENED_BY_PRESS {
        let mut s = menu.sim();
        let opener = menu.opener_at(&s);
        s.mv(opener);
        s.one_frame().press().send();
        s.frame(vec![]);
        if !menu.is_open(&s) {
            bad.push(format!("{menu:?}: the press does not open the menu"));
            continue;
        }
        let to = center(&s, &menu.rows().1);
        s.mv(to);
        for _ in 0..12 {
            s.frame(vec![]);
        }
        s.one_frame().move_to(to).release().send();
        settle(&mut s);
        soft!(bad, menu.picked_expected(&s), "{menu:?}: the release over the row does not pick it");
    }
    assert!(bad.is_empty(), "{bad:#?}");
}

#[test]
fn a_press_slide_and_release_inside_one_frame_picks_the_row_under_the_release() {
    let mut bad = Vec::new();
    for menu in ALL {
        let mut s = menu.sim();
        menu.open(&mut s);
        let (a, b) = menu.rows();
        let (from, to) = (center(&s, &a), center(&s, &b));
        s.mv(from);
        s.one_frame().press().move_to(from + (to - from) * 0.5).move_to(to).release().send();
        settle(&mut s);
        soft!(bad, menu.picked_expected(&s), "{menu:?}: nothing picked");
    }
    assert!(bad.is_empty(), "{bad:#?}");
}

#[test]
fn a_release_whose_press_the_menu_never_saw_picks_nothing() {
    let mut bad = Vec::new();
    for menu in ALL {
        let mut s = menu.sim();
        menu.open(&mut s);
        let to = center(&s, &menu.rows().1);
        s.mv(to);
        s.one_frame().release().send();
        settle(&mut s);
        soft!(bad, menu.nothing_picked(&s), "{menu:?}: a bare release picked something");
    }
    assert!(bad.is_empty(), "{bad:#?}");
}

#[test]
fn a_release_over_a_toast_above_the_menu_picks_nothing() {
    let mut bad = Vec::new();
    // The layer picker shares the toast's layer order, and a press on it raises it above the toast: its rows are never
    // covered by one, so there is nothing to hold it to.
    for menu in OPENED_BY_PRESS {
        let mut s = menu.sim();
        let raise_toast = |s: &mut Sim| {
            s.shell.app.show_toast_with_reveal("Exported the file", std::path::PathBuf::from("/tmp/out.designos"));
            settle(s);
        };
        raise_toast(&mut s);
        let toast = s.rect_of("toast-reveal").center();
        // The toast comes up after the menu, so it is the higher layer.
        s.shell.app.toast = None;
        settle(&mut s);
        place_menu_over(menu, &mut s, toast);
        raise_toast(&mut s);
        if !menu.rows_under(&s, toast) {
            bad.push(format!("{menu:?}: test setup, no row lies under the toast at {toast:?}"));
            continue;
        }
        let before = Menu::state(&s);
        let from = center(&s, &menu.rows().0);
        s.mv(from);
        s.one_frame().press().send();
        s.mv(toast);
        s.one_frame().move_to(toast).release().send();
        settle(&mut s);
        soft!(bad, Menu::state(&s) == before, "{menu:?}: the release over the toast ran a row");
        soft!(bad, menu.is_open(&s), "{menu:?}: the release over the toast closed the menu, so a row ran");
    }
    assert!(bad.is_empty(), "{bad:#?}");
}

impl Menu {
    fn rows_under(self, s: &Sim, p: Pos2) -> bool {
        let names: Vec<String> = match self {
            Menu::Brush => vec!["board-brush-Marker".into(), "board-brush-Highlighter".into()],
            Menu::Shape => ShapeKind::ALL.iter().map(|k| format!("board-shape-{k:?}")).collect(),
            Menu::Stamp => STAMP_KINDS.iter().map(|k| format!("board-stamp-{k}")).collect(),
            Menu::ZoomPill | Menu::ZoomPanel => ZOOM_LABELS.iter().map(|l| format!("label:{l}")).collect(),
            Menu::Layers => (0..3).map(|i| format!("layer-row-{i}")).collect(),
        };
        names.iter().any(|n| rect_named(s, n).contains(p))
    }
}

/// Opens `menu` where one of its rows lies under `target`.
fn place_menu_over(menu: Menu, s: &mut Sim, target: Pos2) {
    match menu {
        Menu::Brush | Menu::Shape | Menu::Stamp => {
            let which = match menu {
                Menu::Brush => ToolMenu::Brush,
                Menu::Shape => ToolMenu::Shape,
                _ => ToolMenu::Stamp,
            };
            // The popover sits 8 px above its anchor and centres on it; the target lands a little above its bottom edge.
            let dy = if matches!(menu, Menu::Brush) { 98.0 } else { 30.0 };
            let anchor = Rect::from_center_size(Pos2::new(target.x, target.y + dy), Vec2::new(16.0, 24.0));
            s.shell.app.board.tool_menu = Some((which, anchor));
        }
        Menu::ZoomPill | Menu::ZoomPanel => {
            s.shell.app.ui.menu = Some(MenuKind::Zoom);
            s.shell.app.ui.menu_anchor = Some(Rect::from_center_size(Pos2::new(target.x + 90.0, s.ctx.content_rect().bottom() - 20.0), Vec2::new(56.0, 28.0)));
        }
        Menu::Layers => {
            let ids = vec![s.by_name("Welcome"), s.by_name("Hero"), s.by_name("Card")];
            let row_h = crate::theme::MENU_ITEM_HEIGHT;
            s.shell.app.selection.clear();
            s.shell.app.ui.layer_picker = Some((Pos2::new(target.x - 100.0, target.y - row_h - 1.5 * row_h), ids));
        }
    }
    settle(s);
}

#[test]
fn escape_and_a_press_outside_close_the_menu_without_a_pick() {
    let mut bad = Vec::new();
    for menu in ALL {
        let mut s = menu.sim();
        menu.open(&mut s);
        s.key(Key::Escape);
        settle(&mut s);
        soft!(bad, !menu.is_open(&s), "{menu:?}: Escape does not close it");
        soft!(bad, menu.nothing_picked(&s), "{menu:?}: Escape picked something");

        let mut s = menu.sim();
        menu.open(&mut s);
        let to = center(&s, &menu.rows().1);
        let outside = Pos2::new(60.0, 700.0);
        s.mv(outside);
        s.one_frame().press().send();
        settle(&mut s);
        soft!(bad, !menu.is_open(&s), "{menu:?}: a press outside does not close it");
        soft!(bad, menu.nothing_picked(&s), "{menu:?}: a press outside picked something");

        // A press outside that is released over a row of the menu that was open picks nothing either.
        let mut s = menu.sim();
        menu.open(&mut s);
        s.mv(outside);
        s.one_frame().press().move_to(to).release().send();
        settle(&mut s);
        soft!(bad, menu.nothing_picked(&s), "{menu:?}: the gesture that closed it picked a row");
    }
    assert!(bad.is_empty(), "{bad:#?}");
}

#[test]
fn enter_on_a_keyboard_focused_row_picks_it() {
    let mut bad = Vec::new();
    for menu in ALL {
        let mut s = menu.sim();
        menu.open(&mut s);
        let id = menu.expected_row_id(&s);
        s.ctx.memory_mut(|m| m.request_focus(id));
        settle(&mut s);
        if !s.ctx.memory(|m| m.has_focus(id)) {
            bad.push(format!("{menu:?}: test setup, the row {id:?} did not take focus"));
            continue;
        }
        s.key(Key::Enter);
        settle(&mut s);
        soft!(bad, menu.picked_expected(&s), "{menu:?}: Enter on the focused row picked nothing");
    }
    assert!(bad.is_empty(), "{bad:#?}");
}

#[test]
fn a_toolbar_popover_closes_after_a_pick_and_does_not_reopen_from_the_same_release() {
    let mut bad = Vec::new();
    for menu in [Menu::Shape, Menu::Stamp] {
        let mut s = menu.sim();
        let opener = menu.opener_at(&s);
        s.mv(opener);
        s.one_frame().press().send();
        s.frame(vec![]);
        if !menu.is_open(&s) {
            bad.push(format!("{menu:?}: the press does not open the popover"));
            continue;
        }
        let to = center(&s, &menu.rows().1);
        s.mv(to);
        s.one_frame().move_to(to).release().send();
        settle(&mut s);
        soft!(bad, menu.picked_expected(&s), "{menu:?}: nothing picked");
        soft!(bad, !menu.is_open(&s), "{menu:?}: the popover is open after the pick");
    }
    assert!(bad.is_empty(), "{bad:#?}");
}

#[test]
fn the_zoom_menus_checked_rows_toggle_once_per_release() {
    let mut bad = Vec::new();
    for menu in [Menu::ZoomPill, Menu::ZoomPanel] {
        let mut s = menu.sim();
        let before = s.app().ui.pixel_grid;
        for (n, want) in [(1, !before), (2, before)] {
            let opener = menu.opener_at(&s);
            s.mv(opener);
            s.one_frame().press().send();
            s.frame(vec![]);
            if !menu.is_open(&s) {
                bad.push(format!("{menu:?}: the press does not open the menu (release {n})"));
                break;
            }
            let to = center(&s, "label:Pixel grid");
            s.mv(to);
            s.one_frame().move_to(to).release().send();
            settle(&mut s);
            soft!(bad, s.app().ui.pixel_grid == want, "{menu:?}: after release {n} Pixel grid is {}, want {want}", s.app().ui.pixel_grid);
            soft!(bad, !menu.is_open(&s), "{menu:?}: the menu is still open after release {n}");
        }
    }
    assert!(bad.is_empty(), "{bad:#?}");
}

/// The middle of the Zoom menu's text field, found from the first row below it.
fn zoom_field_at(s: &Sim) -> Pos2 {
    let first = rect_named(s, "label:Zoom in");
    Pos2::new(first.center().x, first.top() - 34.0)
}

#[test]
fn a_text_gesture_that_starts_in_the_zoom_field_picks_no_row() {
    let mut bad = Vec::new();
    for menu in [Menu::ZoomPill, Menu::ZoomPanel] {
        let mut s = menu.sim();
        menu.open(&mut s);
        let before = Menu::state(&s);
        let field = zoom_field_at(&s);
        let zoom_in = center(&s, "label:Zoom in");
        s.mv(field);
        s.one_frame().press().send();
        s.mv(field + Vec2::new(10.0, 0.0));
        for _ in 0..4 {
            s.frame(vec![]);
        }
        s.mv(zoom_in);
        s.one_frame().move_to(zoom_in).release().send();
        settle(&mut s);
        soft!(bad, Menu::state(&s) == before, "{menu:?}: a press in the field ran the row it was released on");
        soft!(bad, menu.is_open(&s), "{menu:?}: the menu closed");
    }
    assert!(bad.is_empty(), "{bad:#?}");
}

#[test]
fn a_press_and_release_in_the_zoom_field_still_edit_the_field() {
    let mut bad = Vec::new();
    for menu in [Menu::ZoomPill, Menu::ZoomPanel] {
        let mut s = menu.sim();
        menu.open(&mut s);
        let before = Menu::state(&s);
        let field = zoom_field_at(&s);
        s.click(field);
        settle(&mut s);
        let field_id = crate::panels::menus::menu_id(MenuKind::Zoom).with("zoom-input");
        soft!(bad, s.ctx.memory(|m| m.has_focus(field_id)), "{menu:?}: the field did not take focus");
        s.type_text("7");
        settle(&mut s);
        let buf = s.ctx.data(|d| d.get_temp::<String>(crate::panels::menus::menu_id(MenuKind::Zoom).with("zoom-input-buf"))).unwrap_or_default();
        soft!(bad, buf.contains('7'), "{menu:?}: typing did not edit the field, it holds {buf:?}");
        soft!(bad, Menu::state(&s) == before && menu.is_open(&s), "{menu:?}: editing the field ran a row or closed the menu");
    }
    assert!(bad.is_empty(), "{bad:#?}");
}

/// Fills of the filled rects that lie inside `area`.
fn fills_inside(s: &Sim, area: Rect) -> Vec<egui::Color32> {
    fn walk(shape: &egui::Shape, area: Rect, out: &mut Vec<egui::Color32>) {
        match shape {
            egui::Shape::Rect(r) if r.fill != egui::Color32::TRANSPARENT && area.contains_rect(r.rect) => out.push(r.fill),
            egui::Shape::Vec(v) => v.iter().for_each(|c| walk(c, area, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    s.shapes.iter().for_each(|c| walk(&c.shape, area, &mut out));
    out
}

#[test]
fn the_row_under_a_held_pointer_shows_its_hover_fill() {
    let mut bad = Vec::new();
    for menu in ALL {
        let mut s = menu.sim();
        menu.open(&mut s);
        let (a, b) = menu.rows();
        let (from, to) = (center(&s, &a), center(&s, &b));
        let area = rect_named(&s, &b);
        // The look of a plain hover: fills that appear on the row when the pointer rests on it and nothing is down.
        s.mv(Pos2::new(60.0, 700.0));
        settle(&mut s);
        let idle = fills_inside(&s, area);
        s.mv(to);
        settle(&mut s);
        let hover: Vec<_> = fills_inside(&s, area).into_iter().filter(|f| !idle.contains(f)).collect();
        if hover.is_empty() {
            bad.push(format!("{menu:?}: test setup, resting the pointer on the row paints no hover fill"));
            continue;
        }
        let mut s = menu.sim();
        menu.open(&mut s);
        s.mv(from);
        s.one_frame().press().send();
        s.mv(from + (to - from).normalized() * 40.0);
        s.mv(to);
        settle(&mut s);
        let held = fills_inside(&s, area);
        soft!(bad, hover.iter().all(|f| held.contains(f)), "{menu:?}: no hover fill under the held pointer, {held:?} lacks {hover:?}");
    }
    assert!(bad.is_empty(), "{bad:#?}");
}

#[test]
fn a_layer_deleted_while_the_picker_is_open_picks_nothing_and_closes_it() {
    let mut s = Menu::Layers.sim();
    Menu::Layers.open(&mut s);
    let from = center(&s, "layer-row-0");
    let gone = center(&s, "layer-row-2");
    let card = s.by_name("Card");
    s.shell.app.doc.remove(card).expect("Card is removable");
    s.mv(from);
    s.one_frame().press().send();
    s.one_frame().move_to(gone).release().send();
    settle(&mut s);
    assert!(s.app().selection.is_empty(), "nothing was picked");
    assert!(s.app().ui.layer_picker.is_none(), "the picker closed");
}
