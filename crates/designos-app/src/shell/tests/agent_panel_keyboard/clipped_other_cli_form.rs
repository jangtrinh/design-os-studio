//! #1001: a form that is out of view holds no keys. In a short window the new-session list ends above the composer, and the
//! `Other CLI…` form, which sits under the list's last rows, is clipped away: Tab must not walk into it.

use super::hidden_other_cli_form::{form_ids, frames, list_sim, LABEL_FIELD};
use super::*;
use crate::agent_cli_catalog::PickerRowKind;
use crate::agent_picker::rows_for_state;
use egui::Id;

/// The window height where the whole form lies below the end of the list's area.
const FORM_OUT_OF_VIEW: f32 = 480.0;
/// The window height where the label is in view and the command field below it is not.
const COMMAND_FIELD_OUT_OF_VIEW: f32 = 540.0;

/// One frame in a window `height` points tall, like `Sim::frame`.
fn frame_at(s: &mut Sim, height: f32, events: Vec<Event>) {
    s.t += 0.05;
    let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(1600.0, height));
    let mut raw = egui::RawInput { screen_rect: Some(screen), time: Some(s.t), events, modifiers: s.mods, ..Default::default() };
    let ctx = s.ctx.clone();
    let shell = &mut s.shell;
    eframe::App::raw_input_hook(shell, &ctx, &mut raw);
    let _ = ctx.run_ui(raw, |ui| shell.draw(ui));
}

fn frames_at(s: &mut Sim, height: f32, n: usize) {
    for _ in 0..n {
        frame_at(s, height, vec![]);
    }
}

fn click_at(s: &mut Sim, height: f32, p: Pos2) {
    let mods = s.mods;
    frame_at(s, height, vec![Event::PointerMoved(p)]);
    for pressed in [true, false] {
        frame_at(s, height, vec![Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed, modifiers: mods }]);
    }
    frame_at(s, height, vec![]);
}

fn press_tab(s: &mut Sim, height: f32) {
    let mods = s.mods;
    for pressed in [true, false] {
        frame_at(s, height, vec![Event::Key { key: Key::Tab, physical_key: Some(Key::Tab), pressed, repeat: false, modifiers: mods }]);
    }
}

/// The new-session list in a window `height` points tall, with the `Other CLI…` form opened from its row.
fn window_with_the_form_open(height: f32) -> Sim {
    let mut s = list_sim();
    frames_at(&mut s, height, 3);
    let other = rows_for_state(&mut s.shell.app.terminal).into_iter().find(|r| r.kind == PickerRowKind::OtherCli).expect("the Other CLI row");
    let row = s.rect_of(&format!("picker-row-{}", other.id));
    click_at(&mut s, height, row.center());
    frames_at(&mut s, height, 3);
    assert!(s.app().terminal.picker.other_form_open, "precondition: the form is open");
    s
}

/// Presses Tab 80 times: the caret never lands in a field of `ids`.
fn assert_tab_never_reaches(s: &mut Sim, height: f32, ids: &[Id]) {
    for presses in 1..=80 {
        press_tab(s, height);
        let focused = s.ctx.memory(|m| m.focused());
        assert!(!matches!(focused, Some(id) if ids.contains(&id)), "Tab {presses} put the caret in a field that cannot be seen");
    }
}

#[test]
fn tab_does_not_walk_into_a_form_that_is_out_of_view() {
    let height = FORM_OUT_OF_VIEW;
    let mut s = window_with_the_form_open(height);
    let (form, composer) = (s.rect_of("agents-other-cli-form"), s.rect_of("agents-composer-input"));
    assert!(form.top() > composer.top(), "precondition: the form lies below the end of the list, under the composer: form {form:?}, composer {composer:?}");
    assert_tab_never_reaches(&mut s, height, &form_ids());
    frame_at(&mut s, height, vec![Event::Text("zz".into())]);
    let picker = &s.app().terminal.picker;
    assert!(picker.other_label_input.is_empty() && picker.other_command_input.is_empty(), "the form took letters it cannot show");
    finish(s);
}

#[test]
fn tab_does_not_walk_into_the_field_of_a_form_that_is_out_of_view() {
    let height = COMMAND_FIELD_OUT_OF_VIEW;
    let mut s = window_with_the_form_open(height);
    let (label, command, composer) = (s.rect_of(LABEL_FIELD), s.rect_of("agents-other-cli-command-input"), s.rect_of("agents-composer-input"));
    assert!(label.top() < composer.top() && command.top() > composer.top(), "precondition: only the label is in view: {label:?}, {command:?}, {composer:?}");
    assert_tab_never_reaches(&mut s, height, &form_ids()[1..]);
    finish(s);
}

#[test]
fn the_form_takes_letters_once_it_is_scrolled_into_view() {
    let height = FORM_OUT_OF_VIEW;
    let mut s = window_with_the_form_open(height);
    s.shell.app.terminal.left_scroll = 300.0;
    frames_at(&mut s, height, 3);
    let label = s.rect_of(LABEL_FIELD);
    click_at(&mut s, height, label.center());
    frame_at(&mut s, height, vec![Event::Text("ab".into())]);
    assert_eq!(s.app().terminal.picker.other_label_input, "ab", "the form is usable where it can be seen");
    frames(&mut s, 1);
    finish(s);
}
