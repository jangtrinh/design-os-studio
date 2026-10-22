//! The Agents panel is the only terminal surface (#838): no dock under the canvas, no `>_` on the rail.
//!
//! Every test builds its own `Sim`: widget rects recorded in an earlier frame stay in the egui context, so a
//! shared window could not prove that something is gone.

use super::*;
use crate::agent_sessions::{save_registry, SessionEntry, SessionEntryStatus};
use crate::app::RailItem;
use crate::cmdk::CmdKAction;
use crate::terminal::AgentCliPreset;

fn recorded(s: &Sim, name: &str) -> Option<Rect> {
    s.ctx.data(|d| d.get_temp::<Rect>(egui::Id::new(("test-rect", name.to_string()))))
}

fn frames(s: &mut Sim, n: usize) {
    for _ in 0..n {
        s.frame(vec![]);
    }
}

/// A window that runs agent CLIs as stubs, so no test starts a real `claude`, `codex` or `agy`.
fn sim_with_stub_clis(opts: Options) -> Sim {
    let mut s = Sim::with(opts);
    s.use_stub_agent_clis();
    s
}

/// Closes the agent processes a test started.
fn finish(mut s: Sim) {
    for mut tab in s.shell.app.terminal.tabs.drain(..) {
        let _ = tab.child.kill();
    }
}

/// Records one resumable Claude session, so opening the Agents panel resumes it instead of showing the picker.
fn seed_resumable_claude_session(s: &mut Sim) {
    let terminal = &mut s.shell.app.terminal;
    std::fs::create_dir_all(&terminal.session_dir).expect("session dir");
    terminal.session_registry.sessions.push(SessionEntry {
        tab_id: 1,
        title: "Claude Code #1".into(),
        cli: "claude".into(),
        session_id: Some("66666666-6666-4666-8666-666666666666".into()),
        created_at: 100,
        last_active_at: 300,
        status: SessionEntryStatus::Exited,
        extra: Default::default(),
    });
    save_registry(&terminal.session_dir, &terminal.session_registry).expect("save registry");
}

/// Criterion 1: the canvas keeps the height it has with nothing open, and nothing of the dock is drawn.
fn assert_canvas_has_no_dock(s: &mut Sim, what: &str) {
    frames(s, 3);
    let full_bottom = Sim::new().app().canvas_rect.bottom();
    let bottom = s.app().canvas_rect.bottom();
    assert!((bottom - full_bottom).abs() < 0.5, "{what}: the canvas must keep its full height (bottom {bottom}, full {full_bottom})");
    for dock_part in ["dock-resize-handle", "dock-resize-grip", "agent-mention-btn-dock", "agent-selection-chip-dock"] {
        assert!(recorded(s, dock_part).is_none(), "{what}: {dock_part} is part of the removed dock");
    }
}

#[test]
fn cmd_backtick_opens_the_agents_panel_and_never_a_dock() {
    let mut s = sim_with_stub_clis(Options::default());
    assert_eq!(s.app().ui.rail, None);
    s.cmd_key(Key::Backtick);
    assert_eq!(s.app().ui.rail, Some(RailItem::Agents), "Cmd+` opens the Agents panel");
    assert!(!s.app().ui.left_collapsed);
    assert!(s.app().terminal.tabs.is_empty(), "with no earlier session the panel shows its new-session list and starts nothing");
    assert!(s.app().terminal.picker_open);
    assert_canvas_has_no_dock(&mut s, "Cmd+`");
    finish(s);
}

#[test]
fn cmd_backtick_closes_the_open_agents_panel() {
    let mut s = sim_with_stub_clis(Options::default());
    s.cmd_key(Key::Backtick);
    assert_eq!(s.app().ui.rail, Some(RailItem::Agents));
    // The terminal swallows keys while it has focus; the toggle is for the moment nothing in the panel does.
    s.shell.app.terminal.viewport_focused = false;
    s.cmd_key(Key::Backtick);
    assert_eq!(s.app().ui.rail, None, "a second Cmd+` closes the panel, like a second click on the Agents rail button");
    assert_canvas_has_no_dock(&mut s, "Cmd+` twice");
    finish(s);
}

#[test]
fn cmd_backtick_resumes_the_last_session_and_never_starts_a_shell() {
    let mut s = sim_with_stub_clis(Options::default());
    seed_resumable_claude_session(&mut s);
    s.cmd_key(Key::Backtick);
    assert_eq!(s.app().ui.rail, Some(RailItem::Agents));
    assert_eq!(s.app().terminal.tabs.len(), 1, "the panel resumes the last session");
    assert!(!s.app().terminal.tabs[0].is_shell_process(), "opening the panel never starts a plain shell");
    assert!(s.app().terminal.tabs[0].is_resumed);
    assert!(!s.app().terminal.picker_open);
    assert_canvas_has_no_dock(&mut s, "Cmd+` with a session");
    finish(s);
}

#[test]
fn cmd_backtick_still_types_into_a_focused_terminal() {
    let mut s = sim_with_stub_clis(Options::default());
    click_agents_rail_button(&mut s);
    assert_eq!(s.app().ui.rail, Some(RailItem::Agents));
    s.shell.app.terminal.spawn_tab(AgentCliPreset::Claude, None).expect("stub claude starts");
    s.shell.app.terminal.viewport_focused = true;
    assert!(s.app().terminal.is_capturing_keyboard(true), "the terminal captures the keyboard while it has focus");
    s.cmd_key(Key::Backtick);
    assert_eq!(s.app().ui.rail, Some(RailItem::Agents), "the key goes to the terminal, which keeps the panel open");
    assert_eq!(s.app().terminal.tabs.len(), 1);
    finish(s);
}

fn click_agents_rail_button(s: &mut Sim) {
    let r = s.rect_of("rail-Agents");
    s.click(r.center());
}

#[test]
fn the_rail_has_no_terminal_button_and_keeps_the_updates_button() {
    let s = Sim::new();
    assert!(recorded(&s, "rail-terminal").is_none(), "the `>_` button is removed: the Agents rail button opens the same panel");
    let updates = s.rect_of("rail-help");
    let canvas_bottom = s.app().canvas_rect.bottom();
    assert!(
        (updates.bottom() - (canvas_bottom - 8.0)).abs() < 0.5,
        "the Figma sync / updates button keeps its place at the bottom of the rail: {updates:?}, window bottom {canvas_bottom}"
    );
    finish(s);
}

#[test]
fn cmd_k_agent_preset_opens_the_agents_panel_and_never_a_dock() {
    let mut s = sim_with_stub_clis(Options::default());
    crate::shell::execute_cmdk_action(&mut s.shell.app, CmdKAction::OpenTerminal(AgentCliPreset::Claude));
    assert_eq!(s.app().ui.rail, Some(RailItem::Agents));
    assert!(!s.app().ui.left_collapsed);
    assert_eq!(s.app().terminal.tabs.len(), 1, "the preset starts its session");
    assert_canvas_has_no_dock(&mut s, "Cmd+K agent preset");
    finish(s);
}

#[test]
fn cmd_k_send_selection_opens_the_agents_panel_and_never_a_dock() {
    let mut s = sim_with_stub_clis(Options { select: Some("Card".into()), ..Default::default() });
    assert!(!s.app().selection.is_empty());
    crate::shell::execute_cmdk_action(&mut s.shell.app, CmdKAction::SendSelectionToTerminal);
    assert_eq!(s.app().ui.rail, Some(RailItem::Agents));
    assert!(!s.app().ui.left_collapsed);
    assert_canvas_has_no_dock(&mut s, "Cmd+K send selection");
    finish(s);
}

#[test]
fn inspect_send_button_opens_the_agents_panel_and_never_a_dock() {
    let mut s = sim_with_stub_clis(Options { select: Some("Card".into()), dev: Some("css".into()), ..Default::default() });
    s.scroll_inspector_down();
    let button = s.rect_of("btn-\"code-connect-terminal\"");
    s.click(button.center());
    assert_eq!(s.app().ui.rail, Some(RailItem::Agents), "the Inspect send button shows the Agents panel");
    assert!(!s.app().ui.left_collapsed);
    assert_eq!(s.app().toast.as_ref().map(|t| t.text.as_str()), Some("Sent selection to agent"), "the toast names the agent and carries no path");
    assert_canvas_has_no_dock(&mut s, "the Inspect send button");
    finish(s);
}

#[test]
fn panel_terminal_flag_opens_the_agents_panel_and_never_a_dock() {
    let s_opts = Options { panel: Some("terminal".into()), ..Default::default() };
    let mut s = sim_with_stub_clis(s_opts);
    assert_eq!(s.app().ui.rail, Some(RailItem::Agents));
    assert!(!s.app().ui.left_collapsed);
    assert!(s.app().terminal.tabs.is_empty(), "no earlier session: the panel shows the new-session list");
    assert_canvas_has_no_dock(&mut s, "--panel terminal");
    finish(s);
}

#[test]
fn tools_rail_item_opens_only_the_tools_panel() {
    let mut s = sim_with_stub_clis(Options::default());
    seed_resumable_claude_session(&mut s);
    let tools = s.rect_of("rail-Tools");
    s.click(tools.center());
    assert_eq!(s.app().ui.rail, Some(RailItem::Tools));
    assert!(!s.app().ui.left_collapsed);
    assert!(s.app().terminal.tabs.is_empty(), "Tools resumes no agent session: it only opens the Tools panel");
    assert_canvas_has_no_dock(&mut s, "the Tools rail item");
    s.click(tools.center());
    assert_eq!(s.app().ui.rail, None, "a second click closes the Tools panel");
    finish(s);
}
