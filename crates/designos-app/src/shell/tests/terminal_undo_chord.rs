//! End-to-end `Sim` tests for `#685`: undo and redo the document while an agent terminal has focus.

use super::*;
use crate::live_patch_apply::user_edit_in_progress;
use crate::terminal::AgentCliPreset;

const MAC_CMD: Modifiers = Modifiers { alt: false, ctrl: false, shift: false, mac_cmd: true, command: true };
const MAC_SHIFT_CMD: Modifiers = Modifiers { alt: false, ctrl: false, shift: true, mac_cmd: true, command: true };
const MAC_CTRL: Modifiers = Modifiers { alt: false, ctrl: true, shift: false, mac_cmd: false, command: false };
const WIN_CTRL: Modifiers = Modifiers { alt: false, ctrl: true, shift: false, mac_cmd: false, command: true };
const WIN_CTRL_SHIFT: Modifiers = Modifiers { alt: false, ctrl: true, shift: true, mac_cmd: false, command: true };

/// Creates a `Sim` window bound to a real `designos_mcp::server::Session` with the terminal dock
/// open and focused on `preset`.
fn live_terminal_sim(preset: AgentCliPreset) -> (Sim, designos_mcp::server::Session, std::path::PathBuf) {
    let mut s = Sim::new();
    let workspace = std::env::temp_dir().join(format!("designos-685-ws-{}", crate::live_sync_tests::unique_tab_id()));
    std::fs::create_dir_all(&workspace).unwrap();
    let session = designos_mcp::server::Session::with_session_dir(&workspace, &s.app().terminal.session_dir).expect("mcp session");
    s.shell.app.terminal.dock_open = true;
    s.shell.app.terminal.viewport_focused = true;
    s.frame(vec![]);
    if let Some(tab) = s.shell.app.terminal.active_tab_mut() {
        tab.preset = preset;
        tab.written_bytes.clear();
    }
    s.shell.app.terminal.active_runtime = preset;
    (s, session, workspace)
}

fn finish(mut s: Sim, workspace: std::path::PathBuf) {
    let dir = s.app().terminal.session_dir.clone();
    s.shell.app.terminal.shutdown();
    let _ = std::fs::remove_dir_all(dir);
    let _ = std::fs::remove_dir_all(workspace);
}

fn send_chord(s: &mut Sim, mods: Modifiers, key: Key) {
    s.mods = mods;
    s.key(key);
    s.mods = Modifiers::NONE;
}

fn node_name(s: &Sim, id: NodeId) -> String {
    s.app().doc.get(id).map(|n| n.name.clone()).unwrap_or_default()
}

/// Starts dragging Hero to the right and stops halfway, with the pointer button still down.
fn start_hero_drag(s: &mut Sim) -> (NodeId, f32, Pos2) {
    let hero = s.by_name("Hero");
    s.shell.app.selection = vec![hero];
    s.frame(vec![]);
    let a = s.screen(300.0, 150.0);
    let t0 = s.app().doc.get(hero).unwrap().transform.tx;
    let b = s.screen(330.0, 150.0);
    s.mv(a);
    s.button(true);
    s.mv(a + (b - a) * 0.5);
    assert!(matches!(s.app().canvas.inter, canvas::Interaction::Move(_)), "precondition: dragging");
    (hero, t0, b)
}

#[test]
fn cmd_z_while_terminal_focused_undoes_document() {
    let (mut s, mut session, workspace) = live_terminal_sim(AgentCliPreset::Claude);
    s.shell.app.terminal.platform_is_mac_override = Some(true);
    let hero = s.by_name("Hero");

    session.call("designos_update_node", serde_json::json!({ "node_id": hero.0, "name": "Hero (agent)" })).expect("agent rename");
    s.frame(vec![]);
    assert_eq!(node_name(&s, hero), "Hero (agent)");
    assert!(s.app().terminal.is_capturing_keyboard(false));

    if let Some(tab) = s.shell.app.terminal.active_tab_mut() {
        tab.written_bytes.clear();
    }
    let active_tab_before = s.app().terminal.active_tab;

    send_chord(&mut s, MAC_CMD, Key::Z);

    assert_eq!(node_name(&s, hero), "Hero", "Cmd+Z with terminal focused undid the document step");
    assert!(s.app().terminal.viewport_focused, "terminal kept keyboard focus after Cmd+Z");
    assert_eq!(s.app().terminal.active_tab, active_tab_before, "focused_tab unchanged");
    assert!(s.app().terminal.focused_tab().unwrap().written_bytes.is_empty(), "undo chord must write nothing to the PTY");

    finish(s, workspace);
}

#[test]
fn mac_cmd_z_undoes_with_the_terminal_focused() {
    cmd_z_while_terminal_focused_undoes_document();
}

#[test]
fn shift_cmd_z_and_cmd_y_while_terminal_focused_redo_document() {
    let (mut s, mut session, workspace) = live_terminal_sim(AgentCliPreset::Claude);
    s.shell.app.terminal.platform_is_mac_override = Some(true);
    let hero = s.by_name("Hero");

    session.call("designos_update_node", serde_json::json!({ "node_id": hero.0, "name": "Hero (redo)" })).expect("agent rename");
    s.frame(vec![]);
    assert_eq!(node_name(&s, hero), "Hero (redo)");

    // Undo first with Cmd+Z.
    send_chord(&mut s, MAC_CMD, Key::Z);
    assert_eq!(node_name(&s, hero), "Hero");

    // Redo with Shift+Cmd+Z.
    if let Some(tab) = s.shell.app.terminal.active_tab_mut() {
        tab.written_bytes.clear();
    }
    send_chord(&mut s, MAC_SHIFT_CMD, Key::Z);
    assert_eq!(node_name(&s, hero), "Hero (redo)", "Shift+Cmd+Z redid the document step");
    assert!(s.app().terminal.viewport_focused, "terminal kept focus after Shift+Cmd+Z");
    assert!(s.app().terminal.focused_tab().unwrap().written_bytes.is_empty());

    // Undo again and redo with Cmd+Y.
    send_chord(&mut s, MAC_CMD, Key::Z);
    assert_eq!(node_name(&s, hero), "Hero");

    if let Some(tab) = s.shell.app.terminal.active_tab_mut() {
        tab.written_bytes.clear();
    }
    send_chord(&mut s, MAC_CMD, Key::Y);
    assert_eq!(node_name(&s, hero), "Hero (redo)", "Cmd+Y redid the document step");
    assert!(s.app().terminal.viewport_focused, "terminal kept focus after Cmd+Y");
    assert!(s.app().terminal.focused_tab().unwrap().written_bytes.is_empty());

    finish(s, workspace);
}

#[test]
fn mac_shift_cmd_z_redoes() {
    shift_cmd_z_and_cmd_y_while_terminal_focused_redo_document();
}

#[test]
fn ctrl_z_on_mac_still_sends_sigtstp_to_pty() {
    let (mut s, mut session, workspace) = live_terminal_sim(AgentCliPreset::Claude);
    s.shell.app.terminal.platform_is_mac_override = Some(true);
    let hero = s.by_name("Hero");

    session.call("designos_update_node", serde_json::json!({ "node_id": hero.0, "name": "Hero (sigtstp)" })).expect("agent rename");
    s.frame(vec![]);
    assert_eq!(node_name(&s, hero), "Hero (sigtstp)");

    if let Some(tab) = s.shell.app.terminal.active_tab_mut() {
        tab.written_bytes.clear();
    }
    send_chord(&mut s, MAC_CTRL, Key::Z);

    assert_eq!(node_name(&s, hero), "Hero (sigtstp)", "Ctrl+Z on macOS must NOT undo the document");
    assert_eq!(s.app().terminal.focused_tab().unwrap().written_bytes, vec![0x1a], "Ctrl+Z on macOS must forward 0x1A (SIGTSTP) to the PTY");

    finish(s, workspace);
}

#[test]
fn ctrl_z_in_shell_tab_on_linux_goes_to_pty() {
    let (mut s, mut session, workspace) = live_terminal_sim(AgentCliPreset::Shell);
    s.shell.app.terminal.platform_is_mac_override = Some(false);
    let hero = s.by_name("Hero");

    session.call("designos_update_node", serde_json::json!({ "node_id": hero.0, "name": "Hero (shell)" })).expect("agent rename");
    s.frame(vec![]);
    assert_eq!(node_name(&s, hero), "Hero (shell)");

    if let Some(tab) = s.shell.app.terminal.active_tab_mut() {
        tab.written_bytes.clear();
    }
    send_chord(&mut s, WIN_CTRL, Key::Z);

    assert_eq!(node_name(&s, hero), "Hero (shell)", "Ctrl+Z in a Shell tab on Linux/Windows must not undo the document");
    assert_eq!(
        s.app().terminal.focused_tab().unwrap().written_bytes,
        vec![0x1a],
        "Ctrl+Z in a Shell tab on Linux/Windows must write 0x1A (SIGTSTP) to the PTY"
    );

    finish(s, workspace);
}

#[test]
fn windows_ctrl_z_is_left_to_a_shell_tab() {
    ctrl_z_in_shell_tab_on_linux_goes_to_pty();
}

#[test]
fn windows_ctrl_z_undoes_in_an_agent_tab_when_the_cli_does_not_use_it() {
    let (mut s, mut session, workspace) = live_terminal_sim(AgentCliPreset::Claude);
    s.shell.app.terminal.platform_is_mac_override = Some(false);
    s.shell.app.terminal.cli_uses_ctrl_z_override = Some(false);
    let hero = s.by_name("Hero");

    session.call("designos_update_node", serde_json::json!({ "node_id": hero.0, "name": "Hero (win-agent)" })).expect("agent rename");
    s.frame(vec![]);
    assert_eq!(node_name(&s, hero), "Hero (win-agent)");

    if let Some(tab) = s.shell.app.terminal.active_tab_mut() {
        tab.written_bytes.clear();
    }
    send_chord(&mut s, WIN_CTRL, Key::Z);
    assert_eq!(node_name(&s, hero), "Hero", "Ctrl+Z undid the document in an agent tab with cli_uses_ctrl_z=false");
    assert!(s.app().terminal.focused_tab().unwrap().written_bytes.is_empty());

    send_chord(&mut s, WIN_CTRL_SHIFT, Key::Z);
    assert_eq!(node_name(&s, hero), "Hero (win-agent)", "Ctrl+Shift+Z redid the document");
    assert!(s.app().terminal.focused_tab().unwrap().written_bytes.is_empty());

    send_chord(&mut s, WIN_CTRL, Key::Z);
    assert_eq!(node_name(&s, hero), "Hero");

    send_chord(&mut s, WIN_CTRL, Key::Y);
    assert_eq!(node_name(&s, hero), "Hero (win-agent)", "Ctrl+Y redid the document");
    assert!(s.app().terminal.focused_tab().unwrap().written_bytes.is_empty());

    finish(s, workspace);
}

#[test]
fn ctrl_z_is_left_to_a_cli_that_uses_it() {
    let (mut s, mut session, workspace) = live_terminal_sim(AgentCliPreset::Claude);
    s.shell.app.terminal.platform_is_mac_override = Some(false);
    s.shell.app.terminal.cli_uses_ctrl_z_override = Some(true);
    let hero = s.by_name("Hero");

    session.call("designos_update_node", serde_json::json!({ "node_id": hero.0, "name": "Hero (cli-ctrl-z)" })).expect("agent rename");
    s.frame(vec![]);
    assert_eq!(node_name(&s, hero), "Hero (cli-ctrl-z)");

    if let Some(tab) = s.shell.app.terminal.active_tab_mut() {
        tab.written_bytes.clear();
    }
    send_chord(&mut s, WIN_CTRL, Key::Z);
    assert_eq!(node_name(&s, hero), "Hero (cli-ctrl-z)", "Ctrl+Z must not undo the document when cli_uses_ctrl_z=true");
    assert_eq!(s.app().terminal.focused_tab().unwrap().written_bytes, vec![0x1a], "Ctrl+Z must be forwarded to the CLI PTY when cli_uses_ctrl_z=true");

    finish(s, workspace);
}

#[test]
fn undo_chord_writes_nothing_to_the_pty() {
    let (mut s, mut session, workspace) = live_terminal_sim(AgentCliPreset::Claude);
    s.shell.app.terminal.platform_is_mac_override = Some(true);
    let hero = s.by_name("Hero");

    session.call("designos_update_node", serde_json::json!({ "node_id": hero.0, "name": "Hero (pty-zero)" })).expect("agent rename");
    s.frame(vec![]);

    if let Some(tab) = s.shell.app.terminal.active_tab_mut() {
        tab.written_bytes.clear();
    }
    send_chord(&mut s, MAC_CMD, Key::Z);
    send_chord(&mut s, MAC_SHIFT_CMD, Key::Z);
    send_chord(&mut s, MAC_CMD, Key::Z);
    send_chord(&mut s, MAC_CMD, Key::Y);

    assert!(s.app().terminal.focused_tab().unwrap().written_bytes.is_empty(), "taken undo/redo chords must write 0 bytes to the PTY");

    finish(s, workspace);
}

#[test]
fn text_edit_keeps_native_text_undo() {
    let (mut s, mut session, workspace) = live_terminal_sim(AgentCliPreset::Claude);
    s.shell.app.terminal.platform_is_mac_override = Some(true);
    let hero = s.by_name("Hero");

    session.call("designos_update_node", serde_json::json!({ "node_id": hero.0, "name": "Hero (stay)" })).expect("agent rename");
    s.frame(vec![]);
    assert_eq!(node_name(&s, hero), "Hero (stay)");

    // Open the terminal search bar (`TextEdit`) while `viewport_focused` remains true and focus it.
    s.shell.app.terminal.search_open = true;
    s.frame(vec![]);
    let search_id = egui::Id::new(("dock", "term-search-input"));
    s.ctx.memory_mut(|m| m.request_focus(search_id));
    s.frame(vec![]);
    assert!(s.ctx.egui_wants_keyboard_input(), "terminal search TextEdit should want keyboard input");

    s.type_text("hello");
    assert_eq!(s.app().terminal.search_query, "hello");

    if let Some(tab) = s.shell.app.terminal.active_tab_mut() {
        tab.written_bytes.clear();
    }

    // Pressing Cmd+Z while the TextEdit has focus must NOT undo the document.
    send_chord(&mut s, MAC_CMD, Key::Z);
    assert_eq!(node_name(&s, hero), "Hero (stay)", "Cmd+Z while a TextEdit has focus must leave the document history untouched");
    assert!(s.app().terminal.focused_tab().unwrap().written_bytes.is_empty());

    finish(s, workspace);
}

#[test]
fn empty_undo_stack_is_silent_noop() {
    let (mut s, _session, workspace) = live_terminal_sim(AgentCliPreset::Claude);
    s.shell.app.terminal.platform_is_mac_override = Some(true);

    assert!(!s.app().hist.can_undo(), "fresh document has an empty undo stack");
    assert!(!s.app().hist.can_redo(), "fresh document has an empty redo stack");
    let before = s.count();

    if let Some(tab) = s.shell.app.terminal.active_tab_mut() {
        tab.written_bytes.clear();
    }

    send_chord(&mut s, MAC_CMD, Key::Z);
    send_chord(&mut s, MAC_SHIFT_CMD, Key::Z);
    send_chord(&mut s, MAC_CMD, Key::Y);

    assert_eq!(s.count(), before, "empty undo/redo stack is a silent no-op");
    assert!(s.app().terminal.viewport_focused, "terminal keeps focus on no-op undo/redo");
    assert!(s.app().terminal.focused_tab().unwrap().written_bytes.is_empty(), "no-op undo/redo must still consume the chord and write nothing to the PTY");

    finish(s, workspace);
}

#[test]
fn chord_is_ignored_during_a_user_edit() {
    let (mut s, mut session, workspace) = live_terminal_sim(AgentCliPreset::Claude);
    s.shell.app.terminal.platform_is_mac_override = Some(true);
    let card = s.by_name("Card");

    session.call("designos_update_node", serde_json::json!({ "node_id": card.0, "name": "Card (agent)" })).expect("agent rename");
    s.frame(vec![]);
    assert_eq!(node_name(&s, card), "Card (agent)");

    // Begin an in-progress canvas drag while the terminal remains focused.
    let (_hero, _t0, _b) = start_hero_drag(&mut s);
    s.shell.app.terminal.viewport_focused = true;
    assert!(user_edit_in_progress(s.app()));

    if let Some(tab) = s.shell.app.terminal.active_tab_mut() {
        tab.written_bytes.clear();
    }

    send_chord(&mut s, MAC_CMD, Key::Z);
    assert_eq!(node_name(&s, card), "Card (agent)", "undo chord must be ignored while a canvas drag / user edit is in progress");
    assert!(s.app().terminal.focused_tab().unwrap().written_bytes.is_empty(), "ignored chord during user edit must still not leak bytes into the PTY");

    // End the in-progress canvas interaction and verify Cmd+Z now undoes the document step.
    s.shell.app.canvas.inter = canvas::Interaction::None;
    s.button(false);
    s.frame(vec![]);
    assert!(!user_edit_in_progress(s.app()));
    send_chord(&mut s, MAC_CMD, Key::Z);
    assert_eq!(node_name(&s, card), "Card");

    finish(s, workspace);
}

#[test]
fn granularity_is_one_step_per_tool_call() {
    let (mut s, mut session, workspace) = live_terminal_sim(AgentCliPreset::Claude);
    s.shell.app.terminal.platform_is_mac_override = Some(true);
    let before = s.count();
    let page_id = s.app().page.0;

    // Tool call 1: `designos_apply_spec` creating 3 nodes in a single MCP tool call.
    session
        .call(
            "designos_apply_spec",
            serde_json::json!({
                "parent_id": page_id,
                "spec": {
                    "type": "frame",
                    "name": "BatchFrame",
                    "width": 200,
                    "height": 100,
                    "children": [
                        { "type": "rect", "name": "BatchChildA", "width": 80, "height": 40 },
                        { "type": "rect", "name": "BatchChildB", "width": 80, "height": 40 }
                    ]
                }
            }),
        )
        .expect("tool call 1 apply_spec");
    s.frame(vec![]);
    let after_call_1 = s.count();
    assert_eq!(after_call_1, before + 3, "tool call 1 created 3 nodes in one batch");

    // Tool call 2: `designos_create_node` creating 1 additional node.
    session
        .call(
            "designos_create_node",
            serde_json::json!({
                "parent_id": page_id,
                "kind": "rectangle",
                "name": "Step2Rect",
                "x": 240.0,
                "y": 20.0,
                "width": 80.0,
                "height": 60.0
            }),
        )
        .expect("tool call 2 create_node");
    s.frame(vec![]);
    assert_eq!(s.count(), before + 4);

    // First Cmd+Z undoes only tool call 2.
    send_chord(&mut s, MAC_CMD, Key::Z);
    assert_eq!(s.count(), after_call_1, "first Cmd+Z reverts only tool call 2");

    // Second Cmd+Z undoes all 3 nodes of tool call 1 in one step.
    send_chord(&mut s, MAC_CMD, Key::Z);
    assert_eq!(s.count(), before, "second Cmd+Z reverts all of tool call 1 in a single undo step");

    // Shift+Cmd+Z redoes tool call 1, then tool call 2.
    send_chord(&mut s, MAC_SHIFT_CMD, Key::Z);
    assert_eq!(s.count(), after_call_1);
    send_chord(&mut s, MAC_SHIFT_CMD, Key::Z);
    assert_eq!(s.count(), before + 4);

    finish(s, workspace);
}
