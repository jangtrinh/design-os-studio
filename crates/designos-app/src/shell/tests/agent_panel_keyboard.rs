//! Who owns the keyboard in the Agents panel: the composer keeps it across the review card's buttons (#922), and an open
//! dialog or menu gets `Esc` before the agent's terminal does (#923).
//!
//! Every test builds its own `Sim` with stub agent CLIs, so no test starts a real `claude`.

use super::*;
use crate::terminal::AgentCliPreset;

mod overlay_escape;
mod review_card_focus;

/// The Agents panel open on a stub Claude tab, with the terminal viewport holding the keyboard.
fn agent_sim() -> Sim {
    let mut s = Sim::new();
    s.use_stub_agent_clis();
    let rail = s.rect_of("rail-Agents");
    s.click(rail.center());
    s.shell.app.terminal.spawn_tab(AgentCliPreset::Claude, None).expect("stub claude starts");
    s.shell.app.terminal.viewport_focused = true;
    s.frame(vec![]);
    s.frame(vec![]);
    s
}

/// Closes the agent processes a test started.
fn finish(mut s: Sim) {
    for mut tab in s.shell.app.terminal.tabs.drain(..) {
        let _ = tab.child.kill();
    }
}
