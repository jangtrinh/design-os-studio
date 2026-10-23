//! Shift+D is the canvas's Dev Mode shortcut. In the Agents panel the composer and the agent's terminal own the
//! keyboard, so a capital D typed there stays a letter and Dev Mode keeps its state.

use super::super::dev_mode_shortcut::type_capital_d;
use super::*;

/// What the stub agent has been typed so far.
fn typed_into_the_agent(s: &Sim) -> String {
    s.app().terminal.active_tab().expect("a tab").input_line_buffer.clone()
}

#[test]
fn a_capital_d_typed_into_the_composer_does_not_toggle_dev_mode() {
    let mut s = agent_sim();
    let input = s.rect_of("agents-composer-input");
    s.click(input.center());
    assert_eq!(s.ctx.memory(|m| m.focused()), Some(crate::agent_a11y::composer_id()), "precondition: the composer has the caret");
    type_capital_d(&mut s);
    assert!(!s.app().ui.dev_mode, "the D belongs to the composer");
    assert_eq!(s.app().terminal.composer_draft(), "D", "the capital D was typed into the composer");
    assert_eq!(s.ctx.memory(|m| m.focused()), Some(crate::agent_a11y::composer_id()), "and the caret stays there");
    finish(s);
}

#[test]
fn a_capital_d_typed_while_the_agent_terminal_has_focus_goes_to_the_terminal() {
    let mut s = agent_sim();
    assert!(s.app().terminal.viewport_focused, "precondition: the terminal holds the keyboard");
    assert_eq!(typed_into_the_agent(&s), "");
    type_capital_d(&mut s);
    assert!(!s.app().ui.dev_mode, "the D belongs to the terminal");
    assert_eq!(typed_into_the_agent(&s), "D", "the capital D was typed into the agent");
    finish(s);
}
