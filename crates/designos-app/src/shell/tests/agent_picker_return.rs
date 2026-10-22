//! Return in the Agents panel acts only on a picker row the user moved a keyboard cursor to (arrows or Tab).
//! A bare Return, with the panel focused and nothing chosen, opens and starts nothing.

use super::*;
use crate::agent_cli_catalog::PickerRowKind;
use crate::agent_picker::rows_for_state;
use crate::app::RailItem;
use egui::epaint::Shape;
use std::path::PathBuf;

fn frames(s: &mut Sim, n: usize) {
    for _ in 0..n {
        s.frame(vec![]);
    }
}

/// The Agents panel open on its new-session list, over stub agent CLIs and an empty settings folder.
fn picker_sim(tag: &str) -> (Sim, PathBuf) {
    let dir = std::env::temp_dir().join(format!("designos_picker_return_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("settings dir");
    let mut s = Sim::new();
    s.use_stub_agent_clis();
    s.shell.app.terminal.picker.settings_root_override = Some(dir.clone());
    s.shell.app.terminal.session_registry.last_cli = None;
    s.cmd_key(Key::Backtick);
    assert_eq!(s.app().ui.rail, Some(RailItem::Agents));
    assert!(s.app().terminal.picker_open && s.app().terminal.tabs.is_empty(), "the panel shows its new-session list");
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

fn rows(s: &mut Sim) -> Vec<crate::agent_cli_catalog::PickerRow> {
    rows_for_state(&mut s.shell.app.terminal)
}

fn other_cli_index(s: &mut Sim) -> usize {
    rows(s).iter().position(|r| r.kind == PickerRowKind::OtherCli).expect("the Other CLI row")
}

fn is_other_cli_form_open(s: &Sim) -> bool {
    s.app().terminal.picker.other_form_open
}

/// True when the picker paints the row's selected fill over its own rectangle.
fn row_is_highlighted(s: &Sim, row_id: &str) -> bool {
    let rect = s.rect_of(&format!("picker-row-{row_id}"));
    s.shapes.iter().any(|c| matches!(&c.shape, Shape::Rect(r) if r.rect == rect && r.fill == crate::theme::BG_SELECTED))
}

#[test]
fn bare_return_in_the_agents_panel_starts_no_session() {
    let (mut s, dir) = picker_sim("bare_start");
    s.key(Key::Enter);
    frames(&mut s, 3);
    assert!(s.app().terminal.tabs.is_empty(), "Return must not start the preselected CLI");
    assert!(s.app().terminal.picker_open, "the list stays up");
    assert!(!is_other_cli_form_open(&s));
    finish(s, dir);
}

#[test]
fn bare_return_does_not_open_the_other_cli_form_when_it_is_the_first_row_the_picker_picks() {
    // With no agent CLI installed, as on the QA machine, the first selectable row is "Other CLI...".
    let (mut s, dir) = picker_sim("bare_other");
    let other = other_cli_index(&mut s);
    s.shell.app.terminal.picker.selected_index = other;
    frames(&mut s, 2);
    s.key(Key::Enter);
    frames(&mut s, 3);
    assert!(!is_other_cli_form_open(&s), "a stray Return must not open the Other CLI form");
    assert!(s.app().terminal.tabs.is_empty());
    finish(s, dir);
}

#[test]
fn return_on_a_row_the_user_moved_to_with_the_arrows_opens_it() {
    let (mut s, dir) = picker_sim("arrows");
    let other = other_cli_index(&mut s);
    let count = rows(&mut s).len();
    for _ in 0..count {
        s.key(Key::ArrowDown);
        if s.app().terminal.picker.selected_index == other {
            break;
        }
    }
    assert_eq!(s.app().terminal.picker.selected_index, other, "the arrows reach Other CLI");
    s.key(Key::Enter);
    frames(&mut s, 3);
    assert!(is_other_cli_form_open(&s), "Return opens the row the user moved to");
    assert!(s.app().terminal.tabs.is_empty());
    finish(s, dir);
}

#[test]
fn return_on_a_row_focused_with_tab_starts_only_that_row() {
    let (mut s, dir) = picker_sim("tab");
    // A row other than the one the picker parks its starting point on (Other CLI).
    let target = rows(&mut s).into_iter().find(|r| r.selectable && r.kind != PickerRowKind::OtherCli).expect("an installed CLI row").id;
    // Tab hands egui focus to a row; asking for the same focus is the same state.
    let id = egui::Id::new(format!("picker-row-{target}"));
    s.ctx.memory_mut(|m| m.request_focus(id));
    frames(&mut s, 2);
    s.key(Key::Enter);
    frames(&mut s, 3);
    assert_eq!(s.app().terminal.tabs.len(), 1, "Return starts the focused row");
    assert!(!is_other_cli_form_open(&s), "and no other row acts with it");
    finish(s, dir);
}

#[test]
fn the_first_arrow_down_lands_on_the_first_selectable_row() {
    let (mut s, dir) = picker_sim("first_down");
    let first = rows(&mut s).iter().position(|r| r.selectable).expect("a selectable row");
    // Park the starting point on the last row: the first arrow must still reveal the first row, not step past it.
    s.shell.app.terminal.picker.selected_index = other_cli_index(&mut s);
    s.key(Key::ArrowDown);
    assert_eq!(s.app().terminal.picker.selected_index, first);
    finish(s, dir);
}

#[test]
fn no_row_looks_focused_until_the_user_moves_the_cursor() {
    let (mut s, dir) = picker_sim("highlight");
    let picks = rows(&mut s);
    let first = picks.iter().find(|r| r.selectable).expect("a selectable row").id.clone();
    for r in &picks {
        assert!(!row_is_highlighted(&s, &r.id), "{} must not look focused before any arrow key", r.id);
    }
    s.key(Key::ArrowDown);
    frames(&mut s, 2);
    assert!(row_is_highlighted(&s, &first), "the arrow reveals the cursor on the first selectable row");
    finish(s, dir);
}
