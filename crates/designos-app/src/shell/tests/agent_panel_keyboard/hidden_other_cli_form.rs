//! #1001: letters typed during a rename go into the rename field. The Agents panel's `Other CLI…` form has two text
//! fields; once the form is hidden it holds no keys, and the rename or field that comes next gets all of them.

use super::*;
use crate::agent_cli_catalog::PickerRowKind;
use crate::agent_picker::rows_for_state;
use crate::agent_sessions::{SessionEntry, SessionEntryStatus};
use egui::Id;

pub(super) const LABEL_FIELD: &str = "agents-other-cli-label-input";

pub(super) fn form_ids() -> [Id; 2] {
    [Id::new(LABEL_FIELD), Id::new("agents-other-cli-command-input")]
}

fn rename_field_id(tab_id: u64) -> Id {
    Id::new(("agents-session-rename-input", tab_id))
}

pub(super) fn frames(s: &mut Sim, n: usize) {
    for _ in 0..n {
        s.frame(vec![]);
    }
}

/// Waits for the background version probes of the CLIs, so the rows stop reordering under the test's clicks.
fn settle_probes(s: &mut Sim) {
    for _ in 0..400 {
        s.frame(vec![]);
        if !s.shell.app.terminal.picker.cache.is_probing() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
    frames(s, 3);
}

/// The Agents panel with no session yet (the new-session list on screen) and one earlier session recorded as tab 7.
/// The record comes after the panel opened: a panel that finds a session to resume opens it instead of the list.
pub(super) fn list_sim() -> Sim {
    let mut s = Sim::new();
    s.use_stub_agent_clis();
    s.shell.app.terminal.session_registry.last_cli = None;
    s.cmd_key(Key::Backtick);
    settle_probes(&mut s);
    assert!(s.app().terminal.picker_open && s.app().terminal.tabs.is_empty(), "precondition: the panel shows its new-session list");
    s.shell.app.terminal.session_registry.sessions.push(SessionEntry {
        tab_id: 7,
        title: "Old chat".into(),
        cli: "claude".into(),
        session_id: Some("99999999-9999-4999-8999-999999999999".into()),
        created_at: 100,
        last_active_at: 200,
        status: SessionEntryStatus::Exited,
        extra: Default::default(),
    });
    frames(&mut s, 2);
    s
}

/// The Agents panel on a stub Claude tab with its `+` menu open.
fn menu_sim() -> Sim {
    let mut s = agent_sim();
    s.shell.app.terminal.session_registry.last_cli = None;
    let plus = s.rect_of("agents-runtime-add-btn");
    s.click(plus.center());
    settle_probes(&mut s);
    s
}

/// Opens the `Other CLI…` form from its row (`row_key` names the rect the surface records the row under), puts the caret
/// in the label field and types "ab".
pub(super) fn open_form_and_type_in_it(s: &mut Sim, row_key: impl Fn(&str) -> String) {
    let other = rows_for_state(&mut s.shell.app.terminal).into_iter().find(|r| r.kind == PickerRowKind::OtherCli).expect("the Other CLI row");
    let row = s.rect_of(&row_key(&other.id));
    s.click(row.center());
    frames(s, 2);
    let label = s.rect_of(LABEL_FIELD);
    s.click(label.center());
    s.type_text("ab");
    assert_eq!(s.app().terminal.picker.other_label_input, "ab", "precondition: the form took the letters while it was on screen");
}

/// The form is hidden: no field of it has the caret.
fn assert_form_has_no_caret(s: &Sim, after: &str) {
    let focused = s.ctx.memory(|m| m.focused());
    assert!(!matches!(focused, Some(id) if form_ids().contains(&id)), "{after}: a hidden form must not hold the keyboard, focus is on {focused:?}");
}

/// Presses `Rename` on the first row of the Sessions popover, which is open.
pub(super) fn press_rename(s: &mut Sim) -> u64 {
    let tab_id = s.shell.app.terminal.sessions_popover_rows().first().map(|r| r.tab_id).expect("a session row");
    let rename = s.rect_of(&format!("agents-session-rename-{tab_id}"));
    s.click(rename.center());
    frames(s, 2);
    assert_eq!(s.app().terminal.renaming_session_tab_id, Some(tab_id), "precondition: the rename started");
    tab_id
}

pub(super) fn assert_letters_went_into_the_rename(s: &mut Sim, tab_id: u64, after: &str) {
    s.type_text("xyz");
    let (input, focus, tool) = (s.app().terminal.rename_session_input.clone(), s.ctx.memory(|m| m.focused()), s.app().tool);
    let on_field = focus == Some(rename_field_id(tab_id));
    assert!(
        input == "xyz" && on_field && tool == Tool::Move,
        "{after}: typed letters belong in the rename field (the old title is selected, so they replace it); field text {input:?}, caret on the field: {on_field}, tool {tool:?}"
    );
    assert!(!s.app().terminal.viewport_focused, "{after}: the terminal does not read the keys of a rename");
}

#[test]
fn a_hidden_form_in_the_new_session_list_gets_none_of_a_renames_letters() {
    let mut s = list_sim();
    open_form_and_type_in_it(&mut s, |id| format!("picker-row-{id}"));
    // The Sessions popover covers the list, and the form with it. The form stays open, with its text.
    let sessions = s.rect_of("agents-sessions-btn");
    s.click(sessions.center());
    frames(&mut s, 2);
    assert!(s.app().terminal.picker.other_form_open, "precondition: the form is open but not drawn");
    assert_form_has_no_caret(&s, "after the list was covered");
    let tab_id = press_rename(&mut s);
    assert_letters_went_into_the_rename(&mut s, tab_id, "after the form was hidden");
    let picker = &s.app().terminal.picker;
    assert_eq!(
        (picker.other_label_input.as_str(), picker.other_command_input.as_str()),
        ("ab", ""),
        "the hidden form kept its text and took none of the letters"
    );
    finish(s);
}

#[test]
fn a_hidden_form_in_the_plus_menu_gets_none_of_a_renames_letters() {
    let mut s = menu_sim();
    open_form_and_type_in_it(&mut s, |id| id.to_string());
    // The Sessions button closes the menu, and the form with it.
    let sessions = s.rect_of("agents-sessions-btn");
    s.click(sessions.center());
    frames(&mut s, 2);
    assert_form_has_no_caret(&s, "after the menu closed");
    let tab_id = press_rename(&mut s);
    assert_letters_went_into_the_rename(&mut s, tab_id, "after the menu closed");
    let picker = &s.app().terminal.picker;
    assert!(picker.other_label_input.is_empty() && picker.other_command_input.is_empty(), "the closed form took none of the letters");
    finish(s);
}

#[test]
fn closing_the_form_or_leaving_the_agents_panel_leaves_no_caret_in_it() {
    let mut s = list_sim();
    open_form_and_type_in_it(&mut s, |id| format!("picker-row-{id}"));
    s.key(Key::Escape);
    frames(&mut s, 2);
    assert!(!s.app().terminal.picker.other_form_open, "Esc closes the form");
    assert_form_has_no_caret(&s, "after Esc");

    open_form_and_type_in_it(&mut s, |id| format!("picker-row-{id}"));
    let file_rail = s.rect_of("rail-File");
    s.click(file_rail.center());
    frames(&mut s, 3);
    assert!(s.app().terminal.picker.other_form_open, "precondition: the form is still open, behind the File panel");
    assert_form_has_no_caret(&s, "after the Agents panel was left");
    s.type_text("cd");
    assert_eq!(s.app().terminal.picker.other_label_input, "ab", "the form behind the File panel took none of the letters");
    finish(s);
}

#[test]
fn another_text_field_gets_the_letters_after_the_form_is_hidden() {
    let mut s = menu_sim();
    open_form_and_type_in_it(&mut s, |id| id.to_string());
    s.key(Key::Escape);
    frames(&mut s, 2);
    let composer = s.rect_of("agents-composer-input");
    s.click(composer.center());
    s.type_text("hi");
    assert_eq!(s.app().terminal.composer_draft(), "hi", "the composer took the letters");
    let picker = &s.app().terminal.picker;
    assert!(picker.other_label_input.is_empty() && picker.other_command_input.is_empty(), "and the hidden form none");
    finish(s);
}
