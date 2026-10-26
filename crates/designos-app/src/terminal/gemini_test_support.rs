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

/// Stub `gemini` that prints the options of a confirmation dialog through its terminal, as the CLI would.
pub(super) const STUB_DIALOG: &str = r#"#!/bin/sh
trap '' INT
printf '%s' "${GEMINI_CLI_SYSTEM_DEFAULTS_PATH-unset}" > "$DESIGNOS_SESSION_DIR/defaults-env.txt"
sleep 0.4
printf '  1. Allow once\r\n  2. No, suggest changes (esc)\r\n'
printf 'ready' > "$DESIGNOS_SESSION_DIR/printed.txt"
sleep 30
"#;

/// Stub `gemini` that prints model prose containing the words an output check might take for a prompt, and no dialog.
pub(super) const STUB_PROSE: &str = r#"#!/bin/sh
trap '' INT
printf '%s' "${GEMINI_CLI_SYSTEM_DEFAULTS_PATH-unset}" > "$DESIGNOS_SESSION_DIR/defaults-env.txt"
sleep 0.4
printf 'Allow me to explain: the (y/n) prompt in your script asks about files.\r\n'
printf 'ready' > "$DESIGNOS_SESSION_DIR/printed.txt"
sleep 30
"#;

/// Stub `gemini` that records the `PATH` it was started with and the `python3` it finds.
pub(super) const STUB_PATH: &str = r#"#!/bin/sh
printf '%s' "$PATH" > "$DESIGNOS_SESSION_DIR/path-seen.txt"
command -v python3 > "$DESIGNOS_SESSION_DIR/python-seen.txt"
printf 'x' > "$DESIGNOS_SESSION_DIR/defaults-env.txt"
sleep 30
"#;

/// Clears a screen and homes the cursor.
pub(super) const CLEAR: &str = "\x1b[2J\x1b[H";

/// What Gemini 0.35.3 draws for an exec confirmation (`ToolConfirmationQueue.tsx`, `ToolConfirmationMessage.tsx`): a boxed header
/// with `index of total` when several are queued, the command, the question and the options. Under 80 columns wide.
pub(super) fn dialog_screen(index: usize, total: usize) -> String {
    let queue = if total > 1 { format!("{index} of {total}") } else { String::new() };
    let lines = [
        format!("Action Required{queue:>41}"),
        "? Shell rm -rf build".to_string(),
        String::new(),
        "Allow execution of: 'rm'?".to_string(),
        "● 1. Allow once".to_string(),
        "  2. Allow for this session".to_string(),
        "  3. No, suggest changes (esc)".to_string(),
    ];
    let rows: Vec<String> = lines.iter().map(|line| format!("│ {line:<56} │")).collect();
    format!("{CLEAR}╭{}╮\r\n{}\r\n╰{}╯", "─".repeat(58), rows.join("\r\n"), "─".repeat(58))
}

/// Draws `text` on tab `tab`'s screen, as if the CLI had printed it.
pub(super) fn show(st: &mut TerminalState, tab: usize, text: &str) {
    st.tabs[tab].parser.lock().unwrap().process(text.as_bytes());
    // What the reader thread does after it has fed a chunk to the screen.
    st.tabs[tab].screen_rev.fetch_add(1, std::sync::atomic::Ordering::Release);
}

pub(super) const ASK: AgentPermissionMode = AgentPermissionMode::Ask;

/// Longer than the time a dialog must stay off the screen before a wait ends.
pub(super) const QUIET: std::time::Duration = std::time::Duration::from_millis(320);

pub(super) const REFUSED: crate::agent_composer::SendOutcome = crate::agent_composer::SendOutcome::Refused(crate::agent_composer::RefuseReason::Permission);

/// True when tab `tab` shows a wait on the user.
pub(super) fn waiting(st: &TerminalState, tab: usize) -> bool {
    matches!(st.tabs[tab].effective_status(), AgentTabStatus::WaitingForInput(_))
}

/// One poll of the bridge: hook records, then every Gemini tab's wait against its screen.
pub(super) fn poll(st: &mut TerminalState) {
    let _ = st.poll_live_agent_bridge();
}

/// What the composer does with a prompt now.
pub(super) fn send(st: &mut TerminalState) -> crate::agent_composer::SendOutcome {
    st.send_to_agent("make it blue", None)
}

/// The dialog goes off tab `tab`'s screen and stays off for longer than the debounce, with a poll on each side.
pub(super) fn dialog_goes(st: &mut TerminalState, tab: usize) {
    show(st, tab, CLEAR);
    poll(st);
    std::thread::sleep(QUIET);
    poll(st);
}

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
    let mut log = std::fs::OpenOptions::new().create(true).append(true).open(st.prepared_hook_paths().events()).unwrap();
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
