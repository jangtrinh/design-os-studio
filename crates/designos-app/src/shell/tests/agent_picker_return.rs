//! Arrows and Return in the Agents panel's new-session list and its `+` menu act only on a row that has egui focus
//! (reached with Tab). With no row focused the picker ignores both keys: a bare Return opens and starts nothing,
//! and a layer selected on the canvas keeps its own arrow and Return shortcuts.

use super::*;
use crate::agent_cli_catalog::{PickerRow, PickerRowKind};
use crate::agent_picker::rows_for_state;
use crate::app::RailItem;
use crate::terminal::AgentCliPreset;
use egui::epaint::Shape;
use egui::Id;
use std::path::PathBuf;

/// The two surfaces that list the rows, and the widget id each draws a row with.
#[derive(Clone, Copy)]
enum Surface {
    /// The Agents panel before any session exists.
    List,
    /// The `+` menu over a running session.
    Menu,
}

impl Surface {
    fn row_id(self, row_id: &str) -> Id {
        match self {
            Surface::List => Id::new(format!("picker-row-{row_id}")),
            Surface::Menu => Id::new(row_id),
        }
    }

    fn rect_key(self, row_id: &str) -> String {
        match self {
            Surface::List => format!("picker-row-{row_id}"),
            Surface::Menu => row_id.to_string(),
        }
    }
}

fn frames(s: &mut Sim, n: usize) {
    for _ in 0..n {
        s.frame(vec![]);
    }
}

/// A window over stub agent CLIs and an empty settings folder, showing `surface`.
fn picker_sim(tag: &str, surface: Surface, opts: Options) -> (Sim, PathBuf) {
    let dir = std::env::temp_dir().join(format!("designos_picker_return_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("settings dir");
    let mut s = Sim::with(opts);
    s.use_stub_agent_clis();
    s.shell.app.terminal.picker.settings_root_override = Some(dir.clone());
    s.shell.app.terminal.session_registry.last_cli = None;
    s.cmd_key(Key::Backtick);
    assert_eq!(s.app().ui.rail, Some(RailItem::Agents));
    if let Surface::Menu = surface {
        s.shell.app.terminal.spawn_tab(AgentCliPreset::Claude, None).expect("stub claude starts");
        s.shell.app.terminal.viewport_focused = false;
        s.shell.app.terminal.runtime_dropdown_open = true;
    } else {
        assert!(s.app().terminal.picker_open && s.app().terminal.tabs.is_empty(), "the panel shows its new-session list");
    }
    settle_probes(&mut s);
    (s, dir)
}

/// Waits for the background version probes of the installed CLIs: rows reorder once when the last one settles, and
/// a test that acts before that reads a list that is about to change.
fn settle_probes(s: &mut Sim) {
    for _ in 0..400 {
        s.frame(vec![]);
        if !s.shell.app.terminal.picker.cache.is_probing() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
    assert!(!s.shell.app.terminal.picker.cache.is_probing(), "the probes settle");
    frames(s, 3);
}

fn finish(mut s: Sim, dir: PathBuf) {
    for mut tab in s.shell.app.terminal.tabs.drain(..) {
        let _ = tab.child.kill();
    }
    let _ = std::fs::remove_dir_all(dir);
}

fn rows(s: &mut Sim) -> Vec<PickerRow> {
    rows_for_state(&mut s.shell.app.terminal)
}

/// Id of the row that has egui focus, if any.
fn focused_row(s: &mut Sim, surface: Surface) -> Option<String> {
    let focused = s.ctx.memory(|m| m.focused())?;
    rows(s).into_iter().find(|r| surface.row_id(&r.id) == focused).map(|r| r.id)
}

/// Presses Tab until a row of the list has focus; returns its id.
fn tab_to_a_row(s: &mut Sim, surface: Surface) -> String {
    for _ in 0..300 {
        if let Some(id) = focused_row(s, surface) {
            return id;
        }
        s.key(Key::Tab);
    }
    panic!("Tab never reached a picker row");
}

/// Gives focus to the first selectable row of the `+` menu without Tab: from the canvas, Tab never leaves the
/// canvas (40 presses stay on it), so a click-free way in is a direct focus request, as a screen reader makes.
fn focus_first_menu_row(s: &mut Sim) -> String {
    let first = ids_of_selectable(s).remove(0);
    s.ctx.memory_mut(|m| m.request_focus(Surface::Menu.row_id(&first)));
    frames(s, 2);
    assert_eq!(focused_row(s, Surface::Menu).as_deref(), Some(first.as_str()));
    first
}

fn ids_of_selectable(s: &mut Sim) -> Vec<String> {
    rows(s).into_iter().filter(|r| r.selectable).map(|r| r.id).collect()
}

fn is_other_cli_form_open(s: &Sim) -> bool {
    s.app().terminal.picker.other_form_open
}

/// True when the picker paints the row's selected fill over its own rectangle.
fn row_is_highlighted(s: &Sim, surface: Surface, row_id: &str) -> bool {
    let rect = s.rect_of(&surface.rect_key(row_id));
    s.shapes.iter().any(|c| matches!(&c.shape, Shape::Rect(r) if r.rect == rect && r.fill == crate::theme::BG_SELECTED))
}

/// What a Return on `row` does: opens the form, or starts a session.
fn expect_return_effect(s: &Sim, row: &PickerRow, tabs_before: usize) {
    if row.kind == PickerRowKind::OtherCli {
        assert!(is_other_cli_form_open(s), "Return on {} opens the form", row.id);
        assert_eq!(s.app().terminal.tabs.len(), tabs_before, "and starts nothing");
    } else {
        assert_eq!(s.app().terminal.tabs.len(), tabs_before + 1, "Return on {} starts that session", row.id);
        assert!(!is_other_cli_form_open(s), "and opens no form");
    }
}

#[test]
fn bare_return_in_the_agents_panel_starts_no_session() {
    let (mut s, dir) = picker_sim("bare_start", Surface::List, Options::default());
    s.key(Key::Enter);
    frames(&mut s, 3);
    assert!(s.app().terminal.tabs.is_empty(), "Return must not start a CLI");
    assert!(s.app().terminal.picker_open, "the list stays up");
    assert!(!is_other_cli_form_open(&s), "and opens no Other CLI form");
    finish(s, dir);
}

#[test]
fn arrows_and_return_with_no_row_focused_leave_the_picker_to_the_canvas() {
    let (mut s, dir) = picker_sim("canvas_keys", Surface::List, Options { select: Some("Avatar".into()), ..Default::default() });
    // The layer was picked on the canvas, so the canvas, not the panel's terminal viewport, holds the keyboard.
    s.shell.app.terminal.viewport_focused = false;
    let layer = s.app().selection[0];
    let y0 = s.app().doc.get(layer).unwrap().transform.ty;
    assert!(focused_row(&mut s, Surface::List).is_none());
    s.key(Key::ArrowDown);
    let y1 = s.app().doc.get(layer).unwrap().transform.ty;
    assert!(y1 > y0, "the arrow nudges the selected layer ({y0} -> {y1})");
    s.key(Key::Enter);
    frames(&mut s, 3);
    assert!(s.app().terminal.tabs.is_empty(), "Return after a nudge starts no session");
    assert!(!is_other_cli_form_open(&s), "and opens no form");
    assert!(s.app().terminal.picker_open);
    finish(s, dir);
}

#[test]
fn tab_reaches_a_row_and_return_acts_on_that_row_only() {
    let (mut s, dir) = picker_sim("tab", Surface::List, Options::default());
    let id = tab_to_a_row(&mut s, Surface::List);
    let row = rows(&mut s).into_iter().find(|r| r.id == id).unwrap();
    s.key(Key::Enter);
    frames(&mut s, 3);
    expect_return_effect(&s, &row, 0);
    finish(s, dir);
}

#[test]
fn arrows_move_focus_between_selectable_rows_and_wrap() {
    let (mut s, dir) = picker_sim("arrows", Surface::List, Options::default());
    let selectable = ids_of_selectable(&mut s);
    assert!(selectable.len() >= 3, "the stub CLIs, Other CLI and Shell are selectable: {selectable:?}");
    let start = tab_to_a_row(&mut s, Surface::List);
    // Walk to the top, then down through every row and once more around.
    while focused_row(&mut s, Surface::List).as_deref() != Some(selectable[0].as_str()) {
        s.key(Key::ArrowUp);
    }
    for expected in selectable.iter().skip(1).chain(std::iter::once(&selectable[0])) {
        s.key(Key::ArrowDown);
        assert_eq!(focused_row(&mut s, Surface::List).as_deref(), Some(expected.as_str()), "ArrowDown steps to the next selectable row (started at {start})");
    }
    s.key(Key::ArrowUp);
    assert_eq!(focused_row(&mut s, Surface::List).as_deref(), selectable.last().map(String::as_str), "ArrowUp wraps to the last row");
    finish(s, dir);
}

#[test]
fn return_on_the_row_the_arrows_reached_opens_it() {
    let (mut s, dir) = picker_sim("arrows_return", Surface::List, Options::default());
    tab_to_a_row(&mut s, Surface::List);
    let selectable = ids_of_selectable(&mut s);
    let other = selectable.iter().find(|id| id.contains("other-cli")).expect("Other CLI row").clone();
    for _ in 0..selectable.len() {
        if focused_row(&mut s, Surface::List).as_deref() == Some(other.as_str()) {
            break;
        }
        s.key(Key::ArrowDown);
    }
    assert_eq!(focused_row(&mut s, Surface::List).as_deref(), Some(other.as_str()), "the arrows reach Other CLI");
    s.key(Key::Enter);
    frames(&mut s, 3);
    assert!(is_other_cli_form_open(&s), "Return opens the row the arrows reached");
    assert!(s.app().terminal.tabs.is_empty());
    finish(s, dir);
}

#[test]
fn no_row_looks_focused_until_the_keyboard_is_on_one() {
    let (mut s, dir) = picker_sim("highlight", Surface::List, Options::default());
    let all = rows(&mut s);
    for r in &all {
        assert!(!row_is_highlighted(&s, Surface::List, &r.id), "{} must not look focused before the keyboard is on a row", r.id);
    }
    let id = tab_to_a_row(&mut s, Surface::List);
    frames(&mut s, 2);
    assert!(row_is_highlighted(&s, Surface::List, &id), "the focused row shows the selected look");
    finish(s, dir);
}

#[test]
fn the_plus_menu_ignores_arrows_and_return_until_a_row_has_focus() {
    let (mut s, dir) = picker_sim("menu_idle", Surface::Menu, Options::default());
    assert!(s.app().terminal.runtime_dropdown_open);
    assert!(focused_row(&mut s, Surface::Menu).is_none());
    for key in [Key::ArrowDown, Key::ArrowUp, Key::Enter] {
        s.key(key);
    }
    frames(&mut s, 3);
    assert_eq!(s.app().terminal.tabs.len(), 1, "no session was started");
    assert!(!is_other_cli_form_open(&s), "and no form opened");
    assert!(s.app().terminal.runtime_dropdown_open, "the menu stays open");
    finish(s, dir);
}

#[test]
fn the_plus_menu_moves_focus_with_arrows_and_returns_on_the_focused_row() {
    let (mut s, dir) = picker_sim("menu_keys", Surface::Menu, Options::default());
    let selectable = ids_of_selectable(&mut s);
    focus_first_menu_row(&mut s);
    while focused_row(&mut s, Surface::Menu).as_deref() != Some(selectable[0].as_str()) {
        s.key(Key::ArrowUp);
    }
    s.key(Key::ArrowDown);
    assert_eq!(focused_row(&mut s, Surface::Menu).as_deref(), Some(selectable[1].as_str()), "ArrowDown steps to the next row");
    s.key(Key::ArrowUp);
    s.key(Key::ArrowUp);
    assert_eq!(focused_row(&mut s, Surface::Menu).as_deref(), selectable.last().map(String::as_str), "ArrowUp wraps to the last row");
    let other = selectable.iter().find(|id| id.contains("other-cli")).expect("Other CLI row").clone();
    for _ in 0..selectable.len() {
        if focused_row(&mut s, Surface::Menu).as_deref() == Some(other.as_str()) {
            break;
        }
        s.key(Key::ArrowUp);
    }
    s.key(Key::Enter);
    frames(&mut s, 3);
    assert!(is_other_cli_form_open(&s), "Return opens the focused Other CLI row");
    assert_eq!(s.app().terminal.tabs.len(), 1, "and starts nothing");
    finish(s, dir);
}
