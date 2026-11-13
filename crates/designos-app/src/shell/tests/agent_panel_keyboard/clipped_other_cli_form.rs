//! #1001: a form that is out of view holds no keys. In a short window the agent menu ends above the pill, and the
//! `Other CLI…` form, which sits under the menu's last rows, is clipped away: Tab must not walk into it.

use super::hidden_other_cli_form::{form_ids, list_sim, LABEL_FIELD};
use super::*;
use egui::Id;

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

fn press_tab(s: &mut Sim, height: f32) {
    let mods = s.mods;
    for pressed in [true, false] {
        frame_at(s, height, vec![Event::Key { key: Key::Tab, physical_key: Some(Key::Tab), pressed, repeat: false, modifiers: mods }]);
    }
}

/// The New chat state in a window `height` points tall, with the agent menu open, `More…` expanded and the `Other CLI…` form open.
fn window_with_the_form_open(height: f32) -> Sim {
    let mut s = list_sim();
    frames_at(&mut s, height, 3);
    s.shell.app.terminal.runtime_dropdown_open = true;
    s.shell.app.terminal.picker.menu_more_open = true;
    s.shell.app.terminal.picker.open_other_form();
    frames_at(&mut s, height, 4);
    s
}

/// [`window_with_the_form_open`] with the menu scrolled back to its top: opening the form reveals it, so a form that is out of view
/// has to be put there by scrolling away, as a later wheel turn would.
fn window_with_the_form_scrolled_out(height: f32) -> Sim {
    let mut s = window_with_the_form_open(height);
    s.shell.app.terminal.picker.menu_scroll = 0.0;
    frames_at(&mut s, height, 2);
    s
}

/// Window heights at which the opened form lies below the menu's first screenful (the menu ends above the pill).
const SHORT_WINDOWS: [f32; 7] = [240.0, 270.0, 300.0, 330.0, 360.0, 420.0, 480.0];

#[test]
fn opening_other_cli_brings_the_whole_form_into_view() {
    let mut revealed = 0;
    for height in SHORT_WINDOWS {
        let scrolled_out = window_with_the_form_scrolled_out(height);
        let (menu, form) = (scrolled_out.rect_of("agents-agent-menu"), scrolled_out.rect_of("agents-other-cli-form"));
        let clipped = form.bottom() > menu.bottom();
        finish(scrolled_out);
        if !clipped {
            continue;
        }
        revealed += 1;
        let s = window_with_the_form_open(height);
        let (menu, form) = (s.rect_of("agents-agent-menu"), s.rect_of("agents-other-cli-form"));
        // The reveal aligns the form's 2 px slot, so the card may end up to 3 px past the menu's end; a form taller than the menu shows its top.
        let fits = form.height() <= menu.height();
        assert!(
            form.top() >= menu.top() - 1.0 && (form.bottom() <= menu.bottom() + 3.0 || (!fits && form.top() <= menu.top() + 1.0)),
            "at {height} px the opened form must be revealed inside the menu: form {form:?}, menu {menu:?}"
        );
        finish(s);
    }
    assert!(revealed >= 1, "some window height clips the form until the menu is revealed: {revealed}");
}

/// The window chrome above the Sim's canvas: a live window is this much taller than the Sim window that gives its menu the same room
/// (QA r3 measured a live 520 px window clipping the form exactly where the Sim does at 460 px).
const LIVE_CHROME: f32 = 60.0;

fn assert_form_inside_menu(s: &Sim, what: &str) {
    let (menu, form) = (s.rect_of("agents-agent-menu"), s.rect_of("agents-other-cli-form"));
    assert!(form.top() >= menu.top() - 1.0 && form.bottom() <= menu.bottom() + 3.0, "{what}: form {form:?}, menu {menu:?}");
}

#[test]
fn opening_other_cli_in_a_520_px_live_window_shows_the_whole_form() {
    let height = 520.0 - LIVE_CHROME;
    let scrolled_out = window_with_the_form_scrolled_out(height);
    let (menu, form) = (scrolled_out.rect_of("agents-agent-menu"), scrolled_out.rect_of("agents-other-cli-form"));
    assert!(form.bottom() > menu.bottom() + 3.0, "a live 520 px window must clip the form until it is revealed: form {form:?}, menu {menu:?}");
    finish(scrolled_out);
    let s = window_with_the_form_open(height);
    assert_form_inside_menu(&s, "at a live 520 px window (Sim 460 px) opening the form reveals it");
    finish(s);
}

#[test]
fn resizing_the_window_with_the_form_open_reveals_it_again() {
    // Opened in a tall window the form fits with no scroll; every shrink after that has to bring it back into view.
    let mut s = window_with_the_form_open(700.0);
    assert_form_inside_menu(&s, "open at 700 px");
    for height in [460.0, 360.0, 300.0, 360.0, 460.0] {
        frames_at(&mut s, height, 3);
        assert_form_inside_menu(&s, &format!("after resizing to {height} px"));
    }
    // A wheel turn away from the form is the person's choice and stays put while the window keeps its size.
    s.shell.app.terminal.picker.menu_scroll = 0.0;
    frames_at(&mut s, 460.0, 3);
    let (menu, form) = (s.rect_of("agents-agent-menu"), s.rect_of("agents-other-cli-form"));
    assert!(form.bottom() > menu.bottom() + 3.0, "a manual scroll away is not undone without a resize: form {form:?}, menu {menu:?}");
    finish(s);
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
fn a_form_clipped_out_of_the_menu_holds_no_keys() {
    let mut clipped = 0;
    for height in [240.0, 270.0, 300.0, 330.0, 360.0, 420.0, 480.0] {
        let mut s = window_with_the_form_scrolled_out(height);
        let (menu, form) = (s.rect_of("agents-agent-menu"), s.rect_of("agents-other-cli-form"));
        // The label field starts one pad under the form's top, the command field one field and a gap below that.
        let label_top = form.top() + crate::theme::INSET_FRAME;
        let command_top = label_top + 22.0 + crate::theme::SPACE_ROW;
        let hidden: Vec<Id> = form_ids().into_iter().zip([label_top, command_top]).filter(|(_, top)| *top >= menu.bottom()).map(|(id, _)| id).collect();
        if !hidden.is_empty() {
            clipped += 1;
            assert_tab_never_reaches(&mut s, height, &hidden);
        }
        if hidden.len() == 2 {
            frame_at(&mut s, height, vec![Event::Text("zz".into())]);
            let picker = &s.app().terminal.picker;
            assert!(picker.other_label_input.is_empty() && picker.other_command_input.is_empty(), "at {height}: the form took letters it cannot show");
        }
        finish(s);
    }
    assert!(clipped >= 2, "some window heights clip a field or the whole form out of the menu: {clipped}");
}

#[test]
fn the_form_takes_letters_once_the_menu_is_scrolled_to_it() {
    let mut scrolled = 0;
    for height in [300.0, 330.0, 360.0, 420.0] {
        let mut s = window_with_the_form_scrolled_out(height);
        let (menu, form) = (s.rect_of("agents-agent-menu"), s.rect_of("agents-other-cli-form"));
        if form.top() + crate::theme::INSET_FRAME < menu.bottom() {
            finish(s);
            continue;
        }
        scrolled += 1;
        // The menu scrolls when it is taller than the room above the pill: scrolled by the form's distance below the menu's top
        // edge, the form comes into view. A field that is not drawn records no rect, so the form's own rect is the measure.
        s.shell.app.terminal.picker.menu_scroll = form.top() - menu.top();
        frames_at(&mut s, height, 3);
        let label = s.rect_of(LABEL_FIELD);
        let menu = s.rect_of("agents-agent-menu");
        assert!(menu.contains(label.center()), "at {height}: scrolling brought the label field into the menu: {label:?} in {menu:?}");
        let (pos, mods) = (label.center(), s.mods);
        frame_at(&mut s, height, vec![Event::PointerMoved(pos)]);
        for pressed in [true, false] {
            frame_at(&mut s, height, vec![Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: mods }]);
        }
        frame_at(&mut s, height, vec![]);
        frame_at(&mut s, height, vec![Event::Text("ab".into())]);
        assert_eq!(s.app().terminal.picker.other_label_input, "ab", "at {height}: the form is usable where it can be seen");
        finish(s);
    }
    assert!(scrolled >= 1, "some window height hides the form until the menu scrolls: {scrolled}");
}
