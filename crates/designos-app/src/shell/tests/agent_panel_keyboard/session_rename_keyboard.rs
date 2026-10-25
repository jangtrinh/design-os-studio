//! #1001: Rename in the Sessions popover takes the keys from the agent's terminal. Until then the terminal reads every
//! letter, and a letter that reaches it is typed into the agent.

use super::hidden_other_cli_form::{assert_letters_went_into_the_rename, frames, press_rename};
use super::*;

/// The Sessions popover open on the stub Claude tab, with the terminal holding the keyboard.
fn popover_sim() -> Sim {
    let mut s = agent_sim();
    assert!(s.app().terminal.viewport_focused, "precondition: the terminal holds the keyboard");
    let sessions = s.rect_of("agents-sessions-btn");
    s.click(sessions.center());
    frames(&mut s, 2);
    s
}

/// What the stub agent has been typed so far.
fn typed_into_the_agent(s: &Sim) -> String {
    s.app().terminal.active_tab().expect("a tab").input_line_buffer.clone()
}

#[test]
fn a_rename_takes_the_keyboard_from_the_agent_terminal() {
    let mut s = popover_sim();
    let tab_id = press_rename(&mut s);
    assert_letters_went_into_the_rename(&mut s, tab_id, "after the terminal had the keyboard");
    assert_eq!(typed_into_the_agent(&s), "", "none of the letters were typed into the agent");
    s.key(Key::Enter);
    assert_eq!(s.app().terminal.renaming_session_tab_id, None, "Enter ends the rename");
    assert_eq!(s.app().terminal.session_registry.sessions[0].title, "xyz", "and keeps what was typed");
    finish(s);
}
