//! Helpers the Gemini CLI tab tests share: a library window that has started a Gemini tab on a stub `gemini`, hook records
//! written the way Gemini's handlers write them, and one frame of the Agents panel with chosen input.

use super::*;
use crate::agent_permissions::{save_permission_mode, AgentPermissionMode};
use crate::gemini_status_hooks::SYSTEM_DEFAULTS_ENV;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

/// Stub `gemini`: records the system-defaults variable and the file it names, then idles like a CLI would.
pub(super) const STUB_IDLE: &str = r#"#!/bin/sh
trap '' INT
printf '%s' "${GEMINI_CLI_SYSTEM_DEFAULTS_PATH-unset}" > "$DESIGNOS_SESSION_DIR/defaults-env.txt"
cat "${GEMINI_CLI_SYSTEM_DEFAULTS_PATH:-/dev/null}" > "$DESIGNOS_SESSION_DIR/defaults-seen.txt" 2>/dev/null || true
sleep 30
"#;

/// Stub `gemini` for a tab with no hooks: prints the options of Gemini's confirmation dialog, then a Braille spinner.
pub(super) const STUB_PROMPT_THEN_SPINNER: &str = r#"#!/bin/sh
trap '' INT
printf '%s' "${GEMINI_CLI_SYSTEM_DEFAULTS_PATH-unset}" > "$DESIGNOS_SESSION_DIR/defaults-env.txt"
sleep 0.4
printf 'Allow execution of: rm? 1. Allow once 2. No, suggest changes (esc)\r\n'
sleep 0.6
printf '\342\240\213 Thinking...\r\n'
printf 'ready' > "$DESIGNOS_SESSION_DIR/printed.txt"
sleep 30
"#;

/// What Studio's own environment holds for the defaults variable: the stub reports this when a tab's environment is left alone.
pub(super) fn inherited() -> String {
    std::env::var(SYSTEM_DEFAULTS_ENV).unwrap_or_else(|_| "unset".to_string())
}

/// A library window in `mode` that has started a Gemini tab on [`STUB_IDLE`]; `before_start` runs on the session folder first.
pub(super) fn start_gemini(tag: &str, mode: AgentPermissionMode, before_start: impl FnOnce(&Path)) -> (PathBuf, TerminalState) {
    start_stub(tag, mode, STUB_IDLE, before_start)
}

/// [`start_gemini`] on any stub; returns once the stub has started.
pub(super) fn start_stub(tag: &str, mode: AgentPermissionMode, stub: &str, before_start: impl FnOnce(&Path)) -> (PathBuf, TerminalState) {
    let dir = std::env::temp_dir().join(format!("designos-gemini-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("bin")).unwrap();
    std::fs::write(dir.join("bin/gemini"), stub).unwrap();
    std::fs::set_permissions(dir.join("bin/gemini"), std::fs::Permissions::from_mode(0o755)).unwrap();
    save_permission_mode(&dir, mode).expect("the library holds the mode");
    let mut st = TerminalState::for_tab(crate::live_sync_tests::unique_tab_id());
    st.bind_library_file(&dir, "file-gemini", &format!("p{}-w1-tab-1", std::process::id()));
    st.path_prepend.push(dir.join("bin"));
    std::fs::create_dir_all(&st.session_dir).unwrap();
    before_start(&st.session_dir);
    st.spawn_tab(AgentCliPreset::Gemini, None).expect("spawn gemini");
    let seen = st.session_dir.join("defaults-env.txt");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while !seen.is_file() {
        assert!(std::time::Instant::now() < deadline, "the stub gemini never started");
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
    std::thread::sleep(std::time::Duration::from_millis(100));
    (dir, st)
}

pub(super) fn finish(dir: PathBuf, mut st: TerminalState) {
    crate::agent_approval_channel::remove_channel(&st.session_dir);
    st.shutdown();
    let _ = std::fs::remove_dir_all(&dir);
}

/// The content of a file the stub wrote into the session folder.
pub(super) fn recorded(st: &TerminalState, file: &str) -> String {
    std::fs::read_to_string(st.session_dir.join(file)).unwrap_or_default()
}

/// The status-hooks files in the session's private folder.
pub(super) fn hook_files(st: &TerminalState) -> Vec<PathBuf> {
    let channel = crate::agent_approval_channel::channel_dir(&st.session_dir).unwrap();
    let names = std::fs::read_dir(channel).into_iter().flatten().flatten().map(|entry| entry.path());
    names.filter(|path| path.file_name().is_some_and(|name| name.to_string_lossy().starts_with("gemini-tab-"))).collect()
}

/// Appends one hook record that names tab `tab` to the session's event log, as Gemini's handler does, and polls the bridge once.
pub(super) fn send_record(st: &mut TerminalState, tab: usize, event: &str, payload: &str) {
    use std::io::Write;
    let mut log = std::fs::OpenOptions::new().create(true).append(true).open(st.session_dir.join("agent-events.jsonl")).unwrap();
    writeln!(log, r#"{{"t":10,"e":"{event}","tab":"{tab}","p":{payload}}}"#).unwrap();
    let _ = st.poll_live_agent_bridge();
}

/// The record Gemini's `Notification` hook writes when it shows a tool-confirmation dialog in tab `st.tabs[0]`.
pub(super) fn ask_permission(st: &mut TerminalState) {
    let id = st.tabs[0].id;
    send_record(st, id, "Notification", r#"{"session_id":"g","notification_type":"ToolPermission"}"#);
}

/// An egui context with Studio's fonts and theme.
pub(super) fn panel_ctx() -> egui::Context {
    let ctx = egui::Context::default();
    crate::theme::install_fonts(&ctx);
    crate::theme::apply(&ctx);
    ctx
}

/// Runs one frame of the Agents panel with `events` as input, with the terminal view open and focused when `terminal_view`
/// (else the chat view), and returns the text it painted.
pub(super) fn frame(st: &mut TerminalState, ctx: &egui::Context, terminal_view: bool, events: Vec<egui::Event>) -> Vec<String> {
    st.side_panel_pty_open = terminal_view;
    st.chat_mode = !terminal_view;
    st.viewport_focused = terminal_view;
    let panel_rect = Rect::from_min_size(Pos2::new(48.0, 0.0), Vec2::new(560.0, 800.0));
    let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1600.0, 900.0))), events, ..Default::default() };
    let out = ctx.run_ui(raw, |ui| {
        let _ = show_left(st, ui, panel_rect, None);
    });
    crate::lint::scan_shapes(&out.shapes).0.into_iter().map(|t| t.text).collect()
}

/// A key press.
pub(super) fn key(key: egui::Key, modifiers: egui::Modifiers) -> egui::Event {
    egui::Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers }
}
