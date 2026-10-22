//! #923: an open dialog, menu or palette gets `Esc` (and `Enter` and the arrows) before the agent's terminal does, and `Esc`
//! in the composer never stops a running turn.

use super::*;
use crate::terminal::AgentTabStatus;

const ESC_BYTE: u8 = 0x1b;

/// One dialog, menu or palette that reads `Esc`: its name, how a test opens it, and how it tells that it is open.
type Overlay = (&'static str, fn(&mut Sim), fn(&Sim) -> bool);

fn overlays() -> Vec<Overlay> {
    fn open_main_menu(s: &mut Sim) {
        s.click(Pos2::new(28.0, 24.0));
    }
    vec![
        ("Agent permissions modal", |s| s.shell.app.ui.permissions_modal_open = true, |s| s.app().ui.permissions_modal_open),
        ("command palette", |s| s.shell.app.cmdk.open = true, |s| s.app().cmdk.open),
        ("keyboard shortcuts dialog", |s| s.shell.app.show_shortcuts(), |s| s.app().dialog.is_some()),
        ("main menu", open_main_menu, |s| s.app().ui.menu.is_some()),
    ]
}

fn terminal_bytes(s: &Sim) -> Vec<u8> {
    s.app().terminal.tabs[0].written_bytes.clone()
}

#[test]
fn esc_closes_the_open_overlay_and_never_reaches_the_terminal() {
    for (name, open, is_open) in overlays() {
        let mut s = agent_sim();
        assert!(s.app().terminal.is_capturing_keyboard(true), "precondition: the terminal holds the keyboard");
        open(&mut s);
        s.frame(vec![]);
        s.frame(vec![]);
        assert!(is_open(&s), "{name}: precondition: it is open");
        s.shell.app.terminal.tabs[0].written_bytes.clear();

        s.key(Key::Escape);
        assert!(!is_open(&s), "{name}: Esc closes it");
        assert!(!terminal_bytes(&s).contains(&ESC_BYTE), "{name}: Esc went to the agent's terminal as well: {:?}", terminal_bytes(&s));

        // With nothing open the same key belongs to the terminal again.
        s.key(Key::Escape);
        assert!(terminal_bytes(&s).contains(&ESC_BYTE), "{name}: with nothing open Esc reaches the terminal");
        finish(s);
    }
}

#[test]
fn enter_and_arrows_stay_out_of_the_terminal_while_an_overlay_is_open() {
    let mut s = agent_sim();
    s.shell.app.show_shortcuts();
    s.frame(vec![]);
    s.frame(vec![]);
    s.shell.app.terminal.tabs[0].written_bytes.clear();
    for key in [Key::ArrowUp, Key::ArrowDown, Key::Enter] {
        s.key(key);
    }
    assert!(terminal_bytes(&s).is_empty(), "keys meant for the dialog reached the terminal: {:?}", terminal_bytes(&s));
    finish(s);
}

#[test]
fn esc_closes_the_overlay_without_also_closing_the_session_menu_below_it() {
    // No session yet: the panel offers the `+` menu of agents, which closes on Esc too.
    let mut s = Sim::new();
    s.use_stub_agent_clis();
    let rail = s.rect_of("rail-Agents");
    s.click(rail.center());
    s.shell.app.terminal.runtime_dropdown_open = true;
    s.shell.app.ui.permissions_modal_open = true;
    s.frame(vec![]);
    s.frame(vec![]);
    assert!(s.app().terminal.runtime_dropdown_open && s.app().ui.permissions_modal_open, "precondition: both are open");

    s.key(Key::Escape);
    assert!(!s.app().ui.permissions_modal_open, "Esc closes the topmost overlay");
    assert!(s.app().terminal.runtime_dropdown_open, "the same Esc must not also close the menu below it");

    s.key(Key::Escape);
    assert!(!s.app().terminal.runtime_dropdown_open, "the next Esc closes the menu");
}

fn mark_turn_running(s: &mut Sim) {
    *s.shell.app.terminal.tabs[0].agent_status.lock().expect("status lock") = AgentTabStatus::Working(Some("Thinking".into()));
}

#[test]
fn esc_for_a_dialog_never_stops_the_turn_after_a_click_in_the_panel_header() {
    let mut s = agent_sim();
    // A press in the header strip is panel chrome: there Esc is the documented Stop.
    s.shell.app.terminal.viewport_focused = false;
    s.shell.app.terminal.chrome_focused = true;
    s.shell.app.ui.permissions_modal_open = true;
    s.frame(vec![]);
    mark_turn_running(&mut s);

    s.key(Key::Escape);
    assert!(!s.app().ui.permissions_modal_open, "Esc closes the dialog");
    assert!(s.app().terminal.tabs[0].stop_state.is_none(), "the Esc that closed the dialog must not also stop the turn");
    assert!(s.app().terminal.tabs[0].stop_sent_keys.is_empty(), "no interrupt key went to the agent");

    mark_turn_running(&mut s);
    s.key(Key::Escape);
    assert!(s.app().terminal.tabs[0].stop_state.is_some(), "with no dialog open Esc in the chrome still stops the turn (#684)");
    finish(s);
}

#[test]
fn esc_in_the_composer_never_stops_a_running_turn() {
    let mut s = agent_sim();
    // The user clicked the header earlier, then moved the caret into the composer with the keyboard shortcut.
    s.shell.app.terminal.chrome_focused = true;
    s.ctx.memory_mut(|m| m.request_focus(crate::agent_a11y::composer_id()));
    s.frame(vec![]);
    s.frame(vec![]);
    assert_eq!(s.ctx.memory(|m| m.focused()), Some(crate::agent_a11y::composer_id()), "precondition: the composer has the caret");
    mark_turn_running(&mut s);

    s.key(Key::Escape);
    assert!(s.app().terminal.tabs[0].stop_state.is_none(), "Esc in the composer only leaves the field; it must not stop the turn");
    assert!(s.app().terminal.tabs[0].stop_sent_keys.is_empty(), "no interrupt key went to the agent");
    finish(s);
}
