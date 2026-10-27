//! #1195: Approval banner shows "Show prompt" on the active tab (AC-1), "Open tab" on a background tab
//! switching to that exact tab id (AC-2), handles multiple waiting tabs and terminal-answered prompts,
//! and exposes keyboard Tab/Enter navigation plus accessible labels (AC-4).

use super::*;
use crate::agent_permissions::AgentPermissionMode;
use crate::terminal::{AgentCliPreset, AgentTabStatus, NEEDS_APPROVAL_LABEL};
use egui::accesskit::Role;
use egui::Id;

fn frames(s: &mut Sim, n: usize) {
    for _ in 0..n {
        s.frame(vec![]);
    }
}

fn painted_texts(s: &Sim) -> Vec<String> {
    crate::lint::scan_shapes(&s.shapes).0.into_iter().map(|t| t.text).collect()
}

fn approval_sim() -> Sim {
    let mut s = Sim::new();
    s.use_stub_agent_clis();
    s.cmd_key(Key::Backtick);
    s.shell.app.terminal.permission_mode = AgentPermissionMode::Ask;
    frames(&mut s, 2);
    s
}

fn seed_waiting_approval(s: &mut Sim, tab_idx: usize) {
    let tab = &mut s.shell.app.terminal.tabs[tab_idx];
    tab.permission_mode = AgentPermissionMode::Ask;
    if let Ok(mut st) = tab.agent_status.lock() {
        *st = AgentTabStatus::WaitingForInput(Some(NEEDS_APPROVAL_LABEL.into()));
    }
}

fn finish(mut s: Sim) {
    for mut tab in s.shell.app.terminal.tabs.drain(..) {
        let _ = tab.child.kill();
    }
}

#[test]
fn active_tab_approval_shows_show_prompt_and_focuses_terminal() {
    let mut s = approval_sim();
    let codex_idx = s.shell.app.terminal.spawn_tab(AgentCliPreset::Codex, None).expect("stub codex starts");
    let tab_id = s.app().terminal.tabs[codex_idx].id;
    assert_eq!(s.app().terminal.active_tab().map(|t| t.id), Some(tab_id));
    seed_waiting_approval(&mut s, codex_idx);

    // Move focus away from the terminal viewport before clicking "Show prompt".
    s.shell.app.terminal.viewport_focused = false;
    s.ctx.memory_mut(|m| m.request_focus(crate::agent_a11y::composer_id()));
    frames(&mut s, 2);

    let texts = painted_texts(&s);
    assert!(texts.iter().any(|t| t == "Codex CLI needs approval in its terminal"), "banner title must be painted: {texts:?}");
    assert!(texts.iter().any(|t| t == "Show prompt"), "active-tab banner must show 'Show prompt': {texts:?}");
    assert!(!texts.iter().any(|t| t == "Open tab"), "active-tab banner must NOT show 'Open tab': {texts:?}");

    let show_prompt = s.rect_of("agent-approval-show-prompt");
    s.shell.app.terminal.tabs[0].written_bytes.clear();
    s.click(show_prompt.center());
    assert!(s.app().terminal.viewport_focused, "clicking 'Show prompt' must focus the terminal viewport");
    assert!(
        s.app().terminal.tabs[0].written_bytes.is_empty(),
        "clicking 'Show prompt' must not inject any keystrokes into the CLI: {:?}",
        s.app().terminal.tabs[0].written_bytes
    );

    // Enter and Esc reach the CLI at once.
    s.key(Key::Enter);
    assert!(
        s.app().terminal.tabs[0].written_bytes.contains(&b'\r'),
        "Enter must reach the CLI immediately after clicking 'Show prompt': {:?}",
        s.app().terminal.tabs[0].written_bytes
    );
    s.shell.app.terminal.tabs[0].written_bytes.clear();
    s.key(Key::Escape);
    assert!(
        s.app().terminal.tabs[0].written_bytes.contains(&0x1b),
        "Esc must reach the CLI immediately after clicking 'Show prompt': {:?}",
        s.app().terminal.tabs[0].written_bytes
    );

    finish(s);
}

#[test]
fn background_tab_approval_shows_banner_and_open_tab_switches_to_waiting_tab() {
    let mut s = approval_sim();
    let codex_idx = s.shell.app.terminal.spawn_tab(AgentCliPreset::Codex, None).expect("stub codex starts");
    let codex_tab_id = s.app().terminal.tabs[codex_idx].id;
    let claude_idx = s.shell.app.terminal.spawn_tab(AgentCliPreset::Claude, None).expect("stub claude starts");
    let claude_tab_id = s.app().terminal.tabs[claude_idx].id;
    assert_eq!(s.app().terminal.active_tab().map(|t| t.id), Some(claude_tab_id), "precondition: Claude tab is active");

    // Seed a pending approval on the background Codex tab (index 0).
    seed_waiting_approval(&mut s, codex_idx);
    s.shell.app.terminal.viewport_focused = false;
    frames(&mut s, 2);

    assert!(s.app().terminal.needs_approval(), "background tab approval must surface needs_approval() on active view");
    let texts = painted_texts(&s);
    assert!(texts.iter().any(|t| t == "Codex CLI needs approval in its terminal"), "background tab banner must name the waiting CLI: {texts:?}");
    assert!(texts.iter().any(|t| t == "Open tab"), "background tab banner must show 'Open tab': {texts:?}");
    assert!(!texts.iter().any(|t| t == "Show prompt"), "background tab banner must NOT show 'Show prompt': {texts:?}");

    let open_tab = s.rect_of("agent-approval-open-tab");
    s.click(open_tab.center());

    assert_eq!(s.app().terminal.active_tab().map(|t| t.id), Some(codex_tab_id), "clicking 'Open tab' must switch to the exact waiting tab id");
    assert!(s.app().terminal.viewport_focused, "clicking 'Open tab' must focus the switched tab's terminal viewport");

    let texts_after = painted_texts(&s);
    assert!(texts_after.iter().any(|t| t == "Show prompt"), "once switched to the waiting tab, banner must show 'Show prompt': {texts_after:?}");
    assert!(!texts_after.iter().any(|t| t == "Open tab"), "once on the waiting tab, 'Open tab' must no longer show: {texts_after:?}");

    finish(s);
}

#[test]
fn two_pending_approvals_in_two_tabs_shows_count_and_opens_oldest_first() {
    let mut s = approval_sim();
    let codex_idx = s.shell.app.terminal.spawn_tab(AgentCliPreset::Codex, None).expect("stub codex starts");
    let codex_tab_id = s.app().terminal.tabs[codex_idx].id;
    let grok_idx = s.shell.app.terminal.spawn_tab(AgentCliPreset::Grok, None).expect("stub grok starts");
    let grok_tab_id = s.app().terminal.tabs[grok_idx].id;
    let claude_idx = s.shell.app.terminal.spawn_tab(AgentCliPreset::Claude, None).expect("stub claude starts");
    let claude_tab_id = s.app().terminal.tabs[claude_idx].id;
    assert_eq!(s.app().terminal.active_tab().map(|t| t.id), Some(claude_tab_id));

    // Both background tabs (Codex = oldest, Grok = newer) enter waiting-for-approval.
    seed_waiting_approval(&mut s, codex_idx);
    seed_waiting_approval(&mut s, grok_idx);
    frames(&mut s, 2);

    let texts = painted_texts(&s);
    assert!(texts.iter().any(|t| t == "1 of 2"), "banner must show count '1 of 2' when two tabs have pending approvals: {texts:?}");
    assert!(texts.iter().any(|t| t == "Codex CLI needs approval in its terminal"), "banner must target the oldest waiting tab (Codex): {texts:?}");
    assert!(texts.iter().any(|t| t == "Open tab"), "banner must offer 'Open tab' for the oldest waiting background tab: {texts:?}");

    let open_tab = s.rect_of("agent-approval-open-tab");
    s.click(open_tab.center());
    assert_eq!(s.app().terminal.active_tab().map(|t| t.id), Some(codex_tab_id), "'Open tab' must switch to the oldest waiting tab first");

    // Once the first tab's approval is answered in the terminal, the banner switches to the second waiting tab (Grok).
    if let Ok(mut st) = s.shell.app.terminal.tabs[codex_idx].agent_status.lock() {
        *st = AgentTabStatus::Working(None);
    }
    frames(&mut s, 2);

    let texts_second = painted_texts(&s);
    assert!(!texts_second.iter().any(|t| t == "1 of 2"), "count '1 of 2' must clear when only one tab remains waiting: {texts_second:?}");
    assert!(
        texts_second.iter().any(|t| t == "Grok Build needs approval in its terminal"),
        "banner must now show the remaining waiting tab (Grok): {texts_second:?}"
    );
    assert!(texts_second.iter().any(|t| t == "Open tab"), "remaining background tab must show 'Open tab': {texts_second:?}");

    let open_second = s.rect_of("agent-approval-open-tab");
    s.click(open_second.center());
    assert_eq!(s.app().terminal.active_tab().map(|t| t.id), Some(grok_tab_id), "'Open tab' must switch to the second waiting tab");

    finish(s);
}

#[test]
fn approval_answered_in_terminal_clears_banner_and_sends_nothing_twice() {
    let mut s = approval_sim();
    let codex_idx = s.shell.app.terminal.spawn_tab(AgentCliPreset::Codex, None).expect("stub codex starts");
    seed_waiting_approval(&mut s, codex_idx);
    frames(&mut s, 2);

    assert!(s.app().terminal.needs_approval());
    assert!(painted_texts(&s).iter().any(|t| t == "Show prompt"));

    let show_prompt = s.rect_of("agent-approval-show-prompt");
    s.click(show_prompt.center());
    s.shell.app.terminal.tabs[0].written_bytes.clear();

    // User answers the prompt in the terminal with Enter.
    s.key(Key::Enter);
    assert_eq!(s.app().terminal.tabs[0].written_bytes, vec![b'\r'], "exactly one Enter byte is sent when the user answers in the terminal");

    // Next status update from the CLI clears the waiting state.
    if let Ok(mut st) = s.shell.app.terminal.tabs[0].agent_status.lock() {
        *st = AgentTabStatus::Working(None);
    }
    frames(&mut s, 2);

    assert!(!s.app().terminal.needs_approval(), "banner state must clear on the next status update");
    let texts = painted_texts(&s);
    assert!(!texts.iter().any(|t| t.contains("needs approval in its terminal")), "approval banner must disappear: {texts:?}");
    assert!(!texts.iter().any(|t| t == "Show prompt" || t == "Open tab"), "no approval button may remain after clearing: {texts:?}");
    assert_eq!(s.app().terminal.tabs[0].written_bytes, vec![b'\r'], "clearing the banner must never send a second keystroke to the terminal");

    finish(s);
}

#[test]
fn keyboard_tab_and_enter_reach_show_prompt_and_open_tab_with_accessible_labels() {
    let mut s = approval_sim();
    crate::agent_a11y::enable_accesskit(&s.ctx);
    let codex_idx = s.shell.app.terminal.spawn_tab(AgentCliPreset::Codex, None).expect("stub codex starts");
    let codex_tab_id = s.app().terminal.tabs[codex_idx].id;
    let claude_idx = s.shell.app.terminal.spawn_tab(AgentCliPreset::Claude, None).expect("stub claude starts");
    let claude_tab_id = s.app().terminal.tabs[claude_idx].id;
    assert_eq!(s.app().terminal.active_tab().map(|t| t.id), Some(claude_tab_id));

    // 1. Background tab waiting -> "Open tab" button gets accessible label and is reachable via Tab from panel chrome + Enter.
    seed_waiting_approval(&mut s, codex_idx);
    s.shell.app.terminal.viewport_focused = false;
    s.shell.app.terminal.chrome_focused = true;
    s.ctx.memory_mut(|m| m.request_focus(Id::new(("agents-left-tab-select", claude_tab_id))));
    let open_btn_id = Id::new(("agents-left-pty", "agent-approval-open-tab"));
    let mut open_a11y_label = None;
    let mut open_a11y_role = None;
    for _ in 0..3 {
        s.frame(vec![]);
        if let Some(update) = s.accesskit_update.as_ref() {
            if let Some((_, node)) = update.nodes.iter().find(|(id, _)| *id == open_btn_id.accesskit_id()) {
                open_a11y_role = Some(node.role());
                open_a11y_label = node.label().map(str::to_string);
            }
        }
    }
    assert_eq!(open_a11y_role, Some(Role::Button), "'Open tab' must have Role::Button in AccessKit");
    assert_eq!(
        open_a11y_label.as_deref(),
        Some("Open the Codex tab waiting for approval"),
        "'Open tab' must expose an accessible label naming the waiting agent tab"
    );

    let mut reached_open_tab = false;
    for _ in 0..80 {
        if s.ctx.memory(|m| m.focused()) == Some(open_btn_id) {
            reached_open_tab = true;
            break;
        }
        s.key(Key::Tab);
    }
    assert!(reached_open_tab, "Tab from panel chrome must reach the 'Open tab' button");

    s.shell.app.terminal.tabs[0].written_bytes.clear();
    s.key(Key::Enter);
    assert_eq!(s.app().terminal.active_tab().map(|t| t.id), Some(codex_tab_id), "pressing Enter on focused 'Open tab' must switch to the waiting Codex tab");
    assert!(s.app().terminal.viewport_focused, "pressing Enter on 'Open tab' must focus the terminal viewport");
    assert!(s.app().terminal.tabs[0].written_bytes.is_empty(), "activating 'Open tab' with Enter must not leak Enter into the CLI PTY");

    // 2. Active tab waiting -> "Show prompt" button gets accessible label and is reachable via Tab from panel chrome + Enter.
    let show_btn_id = Id::new(("agents-left-pty", "agent-approval-show-prompt"));
    let mut show_a11y_label = None;
    let mut show_a11y_role = None;
    for _ in 0..3 {
        if let Some(update) = s.accesskit_update.as_ref() {
            if let Some((_, node)) = update.nodes.iter().find(|(id, _)| *id == show_btn_id.accesskit_id()) {
                show_a11y_role = Some(node.role());
                show_a11y_label = node.label().map(str::to_string);
            }
        }
        if show_a11y_label.is_some() {
            break;
        }
        s.frame(vec![]);
    }
    assert_eq!(show_a11y_role, Some(Role::Button), "'Show prompt' must have Role::Button in AccessKit");
    assert_eq!(show_a11y_label.as_deref(), Some("Show the Codex prompt"), "'Show prompt' must expose an accessible label naming the active agent prompt");

    s.shell.app.terminal.viewport_focused = false;
    s.shell.app.terminal.chrome_focused = true;
    s.ctx.memory_mut(|m| m.request_focus(Id::new(("agents-left-tab-select", codex_tab_id))));
    s.frame(vec![]);
    let mut reached_show_prompt = false;
    for _ in 0..80 {
        if s.ctx.memory(|m| m.focused()) == Some(show_btn_id) {
            reached_show_prompt = true;
            break;
        }
        s.key(Key::Tab);
    }
    assert!(reached_show_prompt, "Tab from panel chrome must reach the 'Show prompt' button");

    s.shell.app.terminal.tabs[0].written_bytes.clear();
    s.key(Key::Enter);
    assert!(s.app().terminal.viewport_focused, "pressing Enter on focused 'Show prompt' must focus the terminal viewport");
    assert!(s.app().terminal.tabs[0].written_bytes.is_empty(), "activating 'Show prompt' with Enter must not leak Enter into the CLI PTY");

    // Subsequent Enter reaches the CLI immediately.
    s.key(Key::Enter);
    assert_eq!(s.app().terminal.tabs[0].written_bytes, vec![b'\r'], "next Enter after 'Show prompt' must reach the CLI");

    finish(s);
}

#[test]
fn focused_terminal_receives_tab_while_background_or_active_tab_waits_for_approval() {
    let mut s = approval_sim();
    let codex_idx = s.shell.app.terminal.spawn_tab(AgentCliPreset::Codex, None).expect("stub codex starts");
    let claude_idx = s.shell.app.terminal.spawn_tab(AgentCliPreset::Claude, None).expect("stub claude starts");
    let claude_tab_id = s.app().terminal.tabs[claude_idx].id;
    assert_eq!(s.app().terminal.active_tab().map(|t| t.id), Some(claude_tab_id));

    // Background Codex tab is waiting for approval while active Claude terminal viewport has focus.
    seed_waiting_approval(&mut s, codex_idx);
    s.shell.app.terminal.viewport_focused = true;
    frames(&mut s, 2);
    assert!(s.app().terminal.needs_approval());

    s.shell.app.terminal.tabs[claude_idx].written_bytes.clear();
    s.key(Key::Tab);
    assert_eq!(
        s.app().terminal.tabs[claude_idx].written_bytes,
        vec![b'\t'],
        "active focused terminal PTY must receive '\\t' even while a background tab waits for approval"
    );

    // Active waiting tab also receives Tab in its own focused terminal prompt.
    let open_tab = s.rect_of("agent-approval-open-tab");
    s.click(open_tab.center());
    assert!(s.app().terminal.viewport_focused);
    s.shell.app.terminal.tabs[codex_idx].written_bytes.clear();
    s.key(Key::Tab);
    assert_eq!(s.app().terminal.tabs[codex_idx].written_bytes, vec![b'\t'], "active waiting CLI must also receive '\\t' when its terminal viewport has focus");

    finish(s);
}

#[test]
fn canvas_single_key_shortcuts_work_when_canvas_clicked_while_background_tab_waits_for_approval() {
    let mut s = approval_sim();
    let codex_idx = s.shell.app.terminal.spawn_tab(AgentCliPreset::Codex, None).expect("stub codex starts");
    let claude_idx = s.shell.app.terminal.spawn_tab(AgentCliPreset::Claude, None).expect("stub claude starts");
    let claude_tab_id = s.app().terminal.tabs[claude_idx].id;
    assert_eq!(s.app().terminal.active_tab().map(|t| t.id), Some(claude_tab_id));

    // Background tab waits for approval while the Agents panel is open on the terminal view.
    seed_waiting_approval(&mut s, codex_idx);
    frames(&mut s, 2);
    assert!(s.app().terminal.needs_approval());
    assert!(s.app().terminal.side_panel_pty_open);

    // User clicks the canvas (terminal viewport is not focused) and presses R then V.
    s.click(s.screen(900.0, 500.0));
    assert!(!s.app().terminal.viewport_focused, "clicking the canvas must unfocus the terminal viewport");
    assert_eq!(s.app().tool, Tool::Move);

    s.key(Key::R);
    assert_eq!(s.app().tool, Tool::Rectangle, "pressing R after clicking the canvas must switch to the Rectangle tool while a background tab waits");

    s.key(Key::V);
    assert_eq!(s.app().tool, Tool::Move, "pressing V after clicking the canvas must switch back to the Move tool while a background tab waits");

    finish(s);
}
