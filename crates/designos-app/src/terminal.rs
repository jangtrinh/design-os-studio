//! Native Embedded PTY Agent Terminal Runtime (`claude`, `codex`, `grok`, `$SHELL`)
//! and Bidirectional Canvas <-> Terminal Spec Bridge (`RailItem::Terminal` + Bottom Dock).
//!
//! Crafted to `es:designer` (`craft-floor.md` + `tokens.css`) and **EaseUI BrainWave 2.0**:
//! - Zero emoji icons (`§2 Refuse by default`): uses crisp Lucide vector glyphs (`crate::icons::Icon`)
//! - Surface steps: `#0E0E12` viewport -> `#16171B` base -> `#1F2024` raised -> `#26272C` hover
//! - 1px hairlines (`#2A2B31`) instead of nested cards-in-cards
//! - Concentric radii (`12px` outer surface, `6px` inner controls)
//! - Semantic status pills (`OK #4CC58B`, `WARN #E0A53C`, `BAD #FF7A8C`, `INFO #8FB0FF`)

#![allow(dead_code)]

use crate::agent_start_failure::{self, FixAction, StartFailure};
use crate::icons::{self, Icon};
use crate::panels::clipped;
use crate::theme;
use crate::widgets;
use designos_doc::{Document, NodeId, NodeSpec};
use egui::text::LayoutJob;
use egui::{
    Align2, Color32, CornerRadius, Event, FontFamily, FontId, Id, Key, Painter, Pos2, Rect, RichText, Sense, Stroke, StrokeKind, TextEdit, TextFormat, Ui, Vec2,
};
use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

// EaseUI BrainWave 2.0 + es:designer Craft Floor Surface & Hairline Tokens
pub const TERM_BG_VIEWPORT: Color32 = Color32::from_rgb(0x0E, 0x0E, 0x12);
pub const TERM_BG_BAR: Color32 = Color32::from_rgb(0x16, 0x17, 0x1B);
pub const TERM_BG_ELEVATED: Color32 = Color32::from_rgb(0x1F, 0x20, 0x24);
pub const TERM_BG_HOVER: Color32 = Color32::from_rgb(0x26, 0x27, 0x2C);
pub const TERM_BORDER: Color32 = Color32::from_rgb(0x2A, 0x2B, 0x31);
pub const TERM_ORANGE: Color32 = Color32::from_rgb(0xFF, 0x6A, 0x3D);
#[allow(dead_code)]
pub const TERM_ORANGE_BG: Color32 = Color32::from_rgb(0x36, 0x1A, 0x12);
pub const TERM_FG_DEFAULT: Color32 = Color32::from_rgb(0xE4, 0xE4, 0xE7);
pub const TERM_SELECTION_BG: Color32 = Color32::from_rgb(0x5A, 0x78, 0x98);

// Semantic Status Pill Tokens (`es:designer` tokens.css)
pub const STATUS_OK_TEXT: Color32 = Color32::from_rgb(0x4C, 0xC5, 0x8B);
pub const STATUS_OK_BG: Color32 = Color32::from_rgb(0x17, 0x30, 0x2A);
pub const STATUS_OK_BORDER: Color32 = Color32::from_rgb(0x2C, 0x5F, 0x4A);

pub const STATUS_WARN_TEXT: Color32 = Color32::from_rgb(0xE0, 0xA5, 0x3C);
pub const STATUS_WARN_BG: Color32 = Color32::from_rgb(0x35, 0x29, 0x1A);
pub const STATUS_WARN_BORDER: Color32 = Color32::from_rgb(0x5C, 0x47, 0x20);

pub const STATUS_BAD_TEXT: Color32 = Color32::from_rgb(0xFF, 0x7A, 0x8C);
pub const STATUS_BAD_BG: Color32 = Color32::from_rgb(0x3B, 0x1D, 0x23);
pub const STATUS_BAD_BORDER: Color32 = Color32::from_rgb(0x6A, 0x2E, 0x39);

pub const STATUS_INFO_TEXT: Color32 = Color32::from_rgb(0x8F, 0xB0, 0xFF);
pub const STATUS_INFO_BG: Color32 = Color32::from_rgb(0x1B, 0x26, 0x40);
pub const STATUS_INFO_BORDER: Color32 = Color32::from_rgb(0x2F, 0x45, 0x77);

const CHAR_W: f32 = 7.8;
const ROW_H: f32 = 16.0;
const MAX_RAW_LOG_BYTES: usize = 128 * 1024;

/// The `-c` overrides that register `designos-mcp` with Codex (leading space included), shared by the
/// interactive tab and the Agent Chat one-shot so both start the same server.
macro_rules! codex_mcp_overrides {
    () => {
        concat!(
            " -c 'mcp_servers.designos.command=\"'\"$DESIGNOS_MCP_BIN\"'\"'",
            " -c 'mcp_servers.designos.args=[\"--workspace\",\"'\"$DESIGNOS_WORKSPACE\"'\"]'",
            " -c 'mcp_servers.designos.env_vars=[\"DESIGNOS_SESSION_DIR\",\"DESIGNOS_LIVE_CANVAS\",\"DESIGNOS_LIVE_DOC_SYNC\",",
            "\"DESIGNOS_LIVE_DOC_PATCH\",\"DESIGNOS_LIVE_SPEC\",\"DESIGNOS_CONTEXT_JSON\",\"DESIGNOS_TARGET_PAGE_ID\",",
            "\"DESIGNOS_TARGET_PAGE_NAME\",\"DESIGNOS_DOC_NAME\",\"DESIGNOS_FILE\"]'"
        )
    };
}

/// Presets for launching an interactive PTY session inside DESIGN:OS Studio.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AgentCliPreset {
    Shell,
    Claude,
    Codex,
    Antigravity,
    Grok,
    #[allow(dead_code)]
    DesignOsHelp,
}

/// Alias for [`AgentCliPreset`] used across agent runtime helpers.
#[allow(dead_code)]
pub type AgentPreset = AgentCliPreset;

#[allow(non_upper_case_globals, dead_code)]
impl AgentCliPreset {
    pub const ClaudeCode: AgentCliPreset = AgentCliPreset::Claude;
    pub const CodexCli: AgentCliPreset = AgentCliPreset::Codex;
    pub const GrokCli: AgentCliPreset = AgentCliPreset::Grok;
    pub const DefaultShell: AgentCliPreset = AgentCliPreset::Shell;

    pub const QUICK_LAUNCH: [AgentCliPreset; 5] =
        [AgentCliPreset::Claude, AgentCliPreset::Codex, AgentCliPreset::Antigravity, AgentCliPreset::Grok, AgentCliPreset::Shell];

    pub fn label(self) -> &'static str {
        self.tab_title()
    }

    pub fn binary(self) -> &'static str {
        self.binary_name()
    }

    pub fn tab_title(self) -> &'static str {
        match self {
            AgentCliPreset::Shell => "Shell ($SHELL)",
            AgentCliPreset::Claude => "Claude Code",
            AgentCliPreset::Codex => "Codex CLI",
            AgentCliPreset::Antigravity => "Antigravity (agy)",
            AgentCliPreset::Grok => "Grok Build",
            AgentCliPreset::DesignOsHelp => "design:os studio Agent CLI",
        }
    }

    pub fn button_label(self) -> &'static str {
        match self {
            AgentCliPreset::Shell => "Shell",
            AgentCliPreset::Claude => "Claude CLI",
            AgentCliPreset::Codex => "Codex CLI",
            AgentCliPreset::Antigravity => "Antigravity",
            AgentCliPreset::Grok => "Grok CLI",
            AgentCliPreset::DesignOsHelp => "Agent Guide",
        }
    }

    pub fn badge(self) -> &'static str {
        match self {
            AgentCliPreset::Shell => "ZSH/SH",
            AgentCliPreset::Claude => "CLAUDE",
            AgentCliPreset::Codex => "CODEX",
            AgentCliPreset::Antigravity => "AGY",
            AgentCliPreset::Grok => "GROK",
            AgentCliPreset::DesignOsHelp => "DESIGNOS",
        }
    }

    pub fn binary_name(self) -> &'static str {
        match self {
            AgentCliPreset::Shell => "zsh",
            AgentCliPreset::Claude => "claude",
            AgentCliPreset::Codex => "codex",
            AgentCliPreset::Antigravity => "agy",
            AgentCliPreset::Grok => "grok",
            AgentCliPreset::DesignOsHelp => "designos",
        }
    }

    /// Interactive CLI launch command with dangerous-permission prompts skipped (`DIR-29`)
    /// and per-file `designos-mcp` + live current-page system prompt pre-wired.
    pub fn cli_launch_command(self) -> &'static str {
        match self {
            AgentCliPreset::Claude => {
                "claude --dangerously-skip-permissions --mcp-config \"$DESIGNOS_MCP_CONFIG\" --settings \"$DESIGNOS_SESSION_DIR/hooks.json\" --append-system-prompt \"You are inside DESIGN:OS Studio bound exclusively to file '$DESIGNOS_DOC_NAME' (session $DESIGNOS_SESSION_DIR). Always read $DESIGNOS_CONTEXT_JSON for the current selected page (current_page_id / current_page_name) and deliver all designs onto that file and page via designos-mcp tools (designos_apply_spec, designos_create_node, designos_update_node) or by writing NodeSpec JSON to $DESIGNOS_LIVE_SPEC.\""
            }
            // Codex hands an MCP child only the `env` / `env_vars` entries of its config, never the parent
            // env, so the session variables `designos-mcp` reads are listed in `env_vars` and the
            // workspace goes in as an argument (same server config as the other tabs' mcp-config.json).
            //
            // Status: Codex tabs keep the OSC / PTY-heuristic status path (`parse_osc_agent_status_and_strip`,
            // `agent_status`); no session hooks are loaded here. Codex runs only hooks the user has reviewed
            // (`hooks.state.*.trusted_hash` in `~/.codex/config.toml`), and Studio must not write or forge that
            // trust, so hooks passed through `-c` were not run (checked on codex 0.163). `notify` is not used
            // either: it would replace the user's own `notify` program in this tab, and it fires only when a
            // turn ends, so it could never report `working`.
            AgentCliPreset::Codex => concat!("codex --dangerously-bypass-approvals-and-sandbox", codex_mcp_overrides!()),
            // `grok` has no `--mcp-config` either. It reads the project config `<cwd>/.grok/config.toml`
            // (see `write_grok_session_config`), so it is started inside the per-file session dir.
            AgentCliPreset::Grok => "grok --always-approve --cwd \"$DESIGNOS_SESSION_DIR\"",
            // `agy` has no `--mcp-config`. It discovers `<dir>/.agents/plugins/<name>/mcp_config.json` in every
            // `--add-dir`, so the per-file session dir carries the `designos` server (see
            // `write_agy_session_plugin`) without touching the user's global `~/.gemini/config/mcp_config.json`.
            AgentCliPreset::Antigravity => "agy --dangerously-skip-permissions --add-dir \"$DESIGNOS_SESSION_DIR\"",
            AgentCliPreset::Shell | AgentCliPreset::DesignOsHelp => "",
        }
    }

    /// Dropdown menu label and flag summary shown in the `+` runtime selector menu.
    pub fn dropdown_entry(self) -> (&'static str, &'static str, &'static str) {
        match self {
            AgentCliPreset::Claude => ("agents-runtime-claude", "Claude", "--dangerously-skip-permissions"),
            AgentCliPreset::Codex => ("agents-runtime-codex", "Codex", "--dangerously-bypass-approvals-and-sandbox"),
            AgentCliPreset::Grok => ("agents-runtime-grok", "Grok", "--always-approve"),
            AgentCliPreset::Antigravity => ("agents-runtime-agy", "Antigravity", "--dangerously-skip-permissions"),
            AgentCliPreset::Shell | AgentCliPreset::DesignOsHelp => ("agents-runtime-pty-toggle", "Shell", "$SHELL"),
        }
    }

    pub fn is_installed(self) -> bool {
        match self {
            AgentCliPreset::Shell | AgentCliPreset::DesignOsHelp => true,
            other => runtime_binary_available(other.binary_name()),
        }
    }
}

/// Checks if `bin` exists as an executable file in `PATH` (or common user/system CLI directories, or as an explicit path).
pub fn runtime_binary_available(bin: &str) -> bool {
    let trimmed = bin.trim();
    if trimmed.is_empty() {
        return false;
    }
    let explicit = Path::new(trimmed);
    if explicit.components().count() > 1 && explicit.is_file() {
        return true;
    }

    let mut dirs: Vec<PathBuf> = Vec::new();
    if let Some(path_os) = std::env::var_os("PATH") {
        for p in std::env::split_paths(&path_os) {
            if !p.as_os_str().is_empty() {
                dirs.push(p);
            }
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        if !home.is_empty() {
            dirs.push(PathBuf::from(format!("{home}/.cargo/bin")));
            dirs.push(PathBuf::from(format!("{home}/.local/bin")));
            dirs.push(PathBuf::from(format!("{home}/.antigravity/bin")));
        }
    }
    dirs.push(PathBuf::from("/opt/homebrew/bin"));
    dirs.push(PathBuf::from("/usr/local/bin"));
    if cfg!(windows) {
        dirs.push(PathBuf::from(r"C:\Program Files\Git\bin"));
        dirs.push(PathBuf::from(r"C:\Program Files\Git\usr\bin"));
        dirs.push(PathBuf::from(r"C:\Windows\System32"));
    }

    dirs.into_iter().any(|d| {
        d.join(trimmed).is_file()
            || (cfg!(windows)
                && (d.join(format!("{trimmed}.exe")).is_file() || d.join(format!("{trimmed}.cmd")).is_file() || d.join(format!("{trimmed}.bat")).is_file()))
    })
}

/// Per-file isolated session environment paths and active document/page metadata.
#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct SessionEnv {
    pub session_id: String,
    pub session_dir: PathBuf,
    pub live_spec_path: PathBuf,
    pub live_doc_sync_path: PathBuf,
    pub live_canvas_path: PathBuf,
    pub context_json_path: PathBuf,
    pub selection_spec_path: PathBuf,
    pub mcp_config_path: PathBuf,
    pub hooks_json_path: PathBuf,
    pub skill_md_path: PathBuf,
    pub current_page_id: u64,
    pub current_page_name: String,
    pub doc_name: String,
}

impl SessionEnv {
    #[allow(dead_code)]
    pub fn for_session_id(session_id: &str) -> Self {
        let clean = if session_id.trim().is_empty() { "tab-1" } else { session_id.trim() };
        let session_dir = std::env::temp_dir().join("designos-sessions").join(clean);
        Self {
            session_id: clean.to_string(),
            live_spec_path: session_dir.join("live-spec.json"),
            live_doc_sync_path: session_dir.join("doc-sync.designos"),
            live_canvas_path: session_dir.join("canvas.designos"),
            context_json_path: session_dir.join("context.json"),
            selection_spec_path: session_dir.join("selection.spec.json"),
            mcp_config_path: session_dir.join("mcp-config.json"),
            hooks_json_path: session_dir.join("hooks.json"),
            skill_md_path: session_dir.join("SKILL.md"),
            session_dir,
            current_page_id: 1,
            current_page_name: "Page 1".to_string(),
            doc_name: "Untitled".to_string(),
        }
    }
}

/// Builds a shell-escaped real CLI invocation for `preset` (`claude`, `codex`, `agy`, `grok`, or shell)
/// with dangerous-permissions skipped (`DIR-29`) and per-file MCP/context environment exported.
pub fn build_real_cli_command(preset: AgentCliPreset, prompt: &str, live_spec: &Path, selection_json: &Path) -> String {
    let clean = prompt.trim().replace('\n', " ");
    let escaped = clean.replace('\'', "'\\''");
    let session_dir = live_spec.parent().unwrap_or_else(|| Path::new("/tmp"));
    let mcp_config = session_dir.join("mcp-config.json");
    let live_doc_sync = session_dir.join("doc-sync.designos");
    let live_canvas = session_dir.join("canvas.designos");
    let context_json = session_dir.join("context.json");
    let env_prefix = format!(
        "DESIGNOS_AGENT_RUNTIME='{}' DESIGNOS_SESSION_DIR='{}' DESIGNOS_MCP_CONFIG='{}' DESIGNOS_LIVE_SPEC='{}' DESIGNOS_LIVE_DOC_SYNC='{}' DESIGNOS_LIVE_CANVAS='{}' DESIGNOS_CONTEXT_JSON='{}' DESIGNOS_SELECTION_JSON='{}'",
        preset.binary_name(),
        session_dir.display(),
        mcp_config.display(),
        live_spec.display(),
        live_doc_sync.display(),
        live_canvas.display(),
        context_json.display(),
        selection_json.display()
    );
    let sys_prompt = "You are inside DESIGN:OS Studio bound exclusively to file '$DESIGNOS_DOC_NAME' (session $DESIGNOS_SESSION_DIR). Always read $DESIGNOS_CONTEXT_JSON for the current selected page (current_page_id / current_page_name) and deliver all designs onto that file and page via designos-mcp tools (designos_apply_spec, designos_create_node, designos_update_node) or by writing NodeSpec JSON to $DESIGNOS_LIVE_SPEC.";
    match preset {
        AgentCliPreset::Claude => format!(
            "{env_prefix} claude --dangerously-skip-permissions -p '{escaped}' --mcp-config \"{}\" --settings '{}' --append-system-prompt \"{sys_prompt}\"",
            mcp_config.display(),
            session_dir.join("hooks.json").display()
        ),
        AgentCliPreset::Codex => format!(
            "{env_prefix} codex --dangerously-bypass-approvals-and-sandbox exec{} '{escaped} (Deliver onto current file/page in $DESIGNOS_CONTEXT_JSON via designos-mcp or write NodeSpec JSON to $DESIGNOS_LIVE_SPEC)'",
            codex_mcp_overrides!()
        ),
        AgentCliPreset::Antigravity => format!(
            "{env_prefix} agy --dangerously-skip-permissions -p '{escaped}' --add-dir '{}'",
            session_dir.display()
        ),
        AgentCliPreset::Grok => format!(
            "{env_prefix} GROK_FOLDER_TRUST='0' grok --always-approve -p '{escaped}' --cwd '{}'",
            session_dir.display()
        ),
        AgentCliPreset::Shell | AgentCliPreset::DesignOsHelp => clean,
    }
}

/// Returns true when `prompt` asks to create, design, or synthesize a UI surface or component.
pub fn is_design_creation_prompt(prompt: &str) -> bool {
    let lower = prompt.to_ascii_lowercase();
    ["create", "build", "design", "card", "frame", "screen", "modal", "dashboard", "login", "pricing", "checkout", "spec", "implement", "make"]
        .iter()
        .any(|kw| lower.contains(kw))
}

/// Actions emitted by the Embedded Agent Terminal for the host `App` / `Shell` to execute.
#[derive(Clone, Debug, PartialEq)]
pub enum TerminalAction {
    /// Parses a `NodeSpec` JSON string and applies it directly to the active `Document` page.
    ApplySpecJson(String),
    /// Loads and applies a `.spec.json` file from disk.
    ApplySpecFile(PathBuf),
    /// What `designos-mcp` appended to `doc-patch.jsonl`, in log order: edits to replay on the
    /// open document, and the points where the agent opened or created a different one.
    ApplyLiveLog(Vec<designos_doc::live_patch::LiveLogLine>),
    /// An agent edit in `doc-patch.jsonl` could not be read (unreadable or over the size limit).
    LiveSyncError(String),
    /// Triggers immediate hot-reload of `save_path` into the Canvas.
    ReloadFromDisk,
    /// Serializes the currently selected Canvas node (or first top-level frame) to
    /// `selection.spec.json` and pipes its path + node summary +
    /// unresolved Canvas Pin-Comments directly into the active PTY stdin.
    ExportSelectionToTerminal,
}

/// Orca 3-tier live agent execution status (`Working` / `WaitingForInput` / `Done` / `Idle`).
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub enum AgentTabStatus {
    #[default]
    Idle,
    Working(Option<String>),
    WaitingForInput(Option<String>),
    Done,
}

/// One live interactive PTY tab backed by `portable-pty` and a `vt100::Parser`.
pub struct TerminalTab {
    pub id: usize,
    pub title: String,
    #[allow(dead_code)]
    pub preset: AgentCliPreset,
    /// Process kind at spawn. Never mutated afterwards: `preset` is relabelled by
    /// `set_active_runtime` / `submit_chat_prompt` and does not reflect the real process.
    pub spawn_preset: AgentCliPreset,
    pub parser: Arc<Mutex<vt100::Parser>>,
    pub writer: Box<dyn Write + Send>,
    pub master: Box<dyn MasterPty + Send>,
    pub child: Box<dyn Child + Send + Sync>,
    pub rows: u16,
    pub cols: u16,
    pub last_spec_json: Arc<Mutex<Option<String>>>,
    pub raw_log: Arc<Mutex<String>>,
    pub selection: Option<((u16, u16), (u16, u16))>,
    pub agent_status: Arc<Mutex<AgentTabStatus>>,
    pub last_output_at: Arc<Mutex<Option<std::time::Instant>>>,
    /// Set after the agent-exit notice is written into the vt100 parser. Never respawn a shell.
    pub exited_notice_shown: bool,
    /// When the PTY child was started; an agent that exits within `START_FAILURE_WINDOW` failed to start.
    pub spawned_at: std::time::Instant,
    /// Why the agent CLI exited right after it was opened; shown instead of the dead terminal.
    pub start_failure: Option<StartFailure>,
}

impl TerminalTab {
    /// Spawns a real interactive PTY session for `preset` with auto-injected `DESIGNOS_*` environment.
    pub fn spawn(id: usize, preset: AgentCliPreset, rows: u16, cols: u16, file_path: Option<&Path>) -> Result<Self, String> {
        Self::spawn_with_session(id, preset, rows, cols, file_path, None)
    }

    /// Spawns a real interactive PTY session bound to a specific per-file [`SessionEnv`].
    pub fn spawn_with_session(
        id: usize,
        preset: AgentCliPreset,
        rows: u16,
        cols: u16,
        file_path: Option<&Path>,
        session: Option<&SessionEnv>,
    ) -> Result<Self, String> {
        Self::spawn_with_session_cmd(id, preset, rows, cols, file_path, session, None)
    }

    /// Like [`Self::spawn_with_session`], with an optional prepared `CommandBuilder` (tests inject a stub agent).
    fn spawn_with_session_cmd(
        id: usize,
        preset: AgentCliPreset,
        rows: u16,
        cols: u16,
        file_path: Option<&Path>,
        session: Option<&SessionEnv>,
        cmd_override: Option<CommandBuilder>,
    ) -> Result<Self, String> {
        let rows = rows.max(6);
        let cols = cols.max(20);
        let pty_system = native_pty_system();
        let pair = pty_system.openpty(PtySize { rows, cols, pixel_width: 0, pixel_height: 0 }).map_err(|e| format!("openpty failed: {e}"))?;

        let mut cmd = match cmd_override {
            Some(c) => c,
            None => {
                let shell_bin = default_shell_program();
                match preset {
                    AgentCliPreset::Shell | AgentCliPreset::DesignOsHelp => {
                        let mut c = CommandBuilder::new(&shell_bin);
                        if cfg!(test) {
                            if shell_bin.ends_with("zsh") {
                                c.arg("-f");
                            } else if shell_bin.ends_with("bash") || shell_bin.ends_with("bash.exe") {
                                c.arg("--norc");
                                c.arg("--noprofile");
                            }
                        } else if !cfg!(windows) {
                            c.arg("-l");
                        }
                        c
                    }
                    AgentCliPreset::Claude => build_agent_shell_cmd(&shell_bin, AgentCliPreset::Claude.cli_launch_command()),
                    AgentCliPreset::Codex => build_agent_shell_cmd(&shell_bin, AgentCliPreset::Codex.cli_launch_command()),
                    AgentCliPreset::Antigravity => build_agent_shell_cmd(&shell_bin, AgentCliPreset::Antigravity.cli_launch_command()),
                    AgentCliPreset::Grok => build_agent_shell_cmd(&shell_bin, AgentCliPreset::Grok.cli_launch_command()),
                }
            }
        };

        configure_pty_env_with_session(&mut cmd, preset, file_path, session);

        let child = pair.slave.spawn_command(cmd).map_err(|e| format!("spawn_command failed: {e}"))?;
        drop(pair.slave);

        let mut reader = pair.master.try_clone_reader().map_err(|e| format!("try_clone_reader failed: {e}"))?;
        let writer = pair.master.take_writer().map_err(|e| format!("take_writer failed: {e}"))?;

        let init_parser = vt100::Parser::new(rows, cols, 5000);
        let parser = Arc::new(Mutex::new(init_parser));
        let last_spec_json = Arc::new(Mutex::new(None));
        let raw_log = Arc::new(Mutex::new(String::new()));
        let agent_status = Arc::new(Mutex::new(AgentTabStatus::Idle));
        let last_output_at = Arc::new(Mutex::new(None));

        let parser_bg = Arc::clone(&parser);
        let spec_bg = Arc::clone(&last_spec_json);
        let log_bg = Arc::clone(&raw_log);
        let status_bg = Arc::clone(&agent_status);
        let output_at_bg = Arc::clone(&last_output_at);

        std::thread::Builder::new()
            .name(format!("designos-pty-reader-{id}"))
            .spawn(move || {
                let mut buf = [0u8; 4096];
                loop {
                    match reader.read(&mut buf) {
                        Ok(0) => break,
                        Ok(n) => {
                            if let Ok(mut ts) = output_at_bg.lock() {
                                *ts = Some(std::time::Instant::now());
                            }
                            let chunk = String::from_utf8_lossy(&buf[..n]);
                            let (clean_chunk, detected_status) = parse_osc_agent_status_and_strip(&chunk);
                            if let Some(next_status) = detected_status {
                                if let Ok(mut st) = status_bg.lock() {
                                    *st = next_status;
                                }
                            }
                            if let Ok(mut p) = parser_bg.lock() {
                                if clean_chunk.len() == chunk.len() {
                                    p.process(&buf[..n]);
                                } else {
                                    p.process(clean_chunk.as_bytes());
                                }
                            }
                            if let Ok(mut log) = log_bg.lock() {
                                log.push_str(&clean_chunk);
                                if log.len() > MAX_RAW_LOG_BYTES {
                                    let drain = log.len() - (MAX_RAW_LOG_BYTES / 2);
                                    let boundary = log.char_indices().map(|(i, _)| i).find(|&i| i >= drain).unwrap_or(log.len());
                                    log.drain(..boundary);
                                }
                                if let Some(spec) = extract_spec_json_from_log(&log) {
                                    if let Ok(mut last) = spec_bg.lock() {
                                        *last = Some(spec);
                                    }
                                }
                            }
                        }
                        Err(_) => break,
                    }
                }
            })
            .map_err(|e| format!("failed to spawn PTY reader thread: {e}"))?;

        Ok(Self {
            id,
            title: format!("{} #{}", preset.button_label(), id),
            preset,
            spawn_preset: preset,
            parser,
            writer,
            master: pair.master,
            child,
            rows,
            cols,
            last_spec_json,
            raw_log,
            selection: None,
            agent_status,
            last_output_at,
            exited_notice_shown: false,
            spawned_at: std::time::Instant::now(),
            start_failure: None,
        })
    }

    /// Test helper: Claude `spawn_preset` running a stub that prints and exits (no `$SHELL` fallback).
    #[cfg(test)]
    fn spawn_exiting_stub_agent(id: usize, rows: u16, cols: u16, file_path: Option<&Path>, session: Option<&SessionEnv>) -> Result<Self, String> {
        let mut cmd = build_agent_shell_cmd(&default_shell_program(), "sh -c 'echo STUB-AGENT-EXIT; exit 1'");
        isolate_zsh_from_home(&mut cmd);
        Self::spawn_with_session_cmd(id, AgentCliPreset::Claude, rows, cols, file_path, session, Some(cmd))
    }

    /// Resizes both the underlying OS PTY and the `vt100::Parser` when the viewport changes.
    pub fn resize(&mut self, rows: u16, cols: u16) {
        let rows = rows.max(4);
        let cols = cols.max(16);
        if self.rows == rows && self.cols == cols {
            return;
        }
        self.rows = rows;
        self.cols = cols;
        let _ = self.master.resize(PtySize { rows, cols, pixel_width: 0, pixel_height: 0 });
        if let Ok(mut p) = self.parser.lock() {
            p.set_size(rows, cols);
        }
    }

    /// Writes raw bytes directly to the PTY master stdin.
    pub fn write_bytes(&mut self, bytes: &[u8]) {
        let _ = self.writer.write_all(bytes);
        let _ = self.writer.flush();
    }

    /// Writes a single line followed by `\r` to execute in the PTY.
    pub fn send_line(&mut self, line: &str) {
        let _ = self.writer.write_all(line.as_bytes());
        let _ = self.writer.write_all(b"\r");
        let _ = self.writer.flush();
    }

    /// True when the spawned process is a login/help shell rather than an agent CLI.
    pub fn is_shell_process(&self) -> bool {
        matches!(self.spawn_preset, AgentCliPreset::Shell | AgentCliPreset::DesignOsHelp)
    }

    /// True when the child has not yet exited (`child.try_wait() == Ok(None)`).
    pub fn is_alive(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }

    /// True when this tab is a live agent CLI (not a shell process, and the child is still running).
    pub fn accepts_agent_input(&mut self) -> bool {
        !self.is_shell_process() && self.is_alive()
    }

    /// Writes `Ctrl-E` (end-of-line) then `Ctrl-U` (kill line / tty VKILL), then `line` and `\r`.
    pub fn send_line_clearing_pending(&mut self, line: &str) {
        let _ = self.writer.write_all(b"\x05\x15");
        let _ = self.writer.write_all(line.as_bytes());
        let _ = self.writer.write_all(b"\r");
        let _ = self.writer.flush();
    }
}

/// Role of a turn in the Agent Chat UI wrapping the live PTY shell (`DIR-21`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AgentChatRole {
    User,
    Assistant,
}

/// One conversational turn in the Agent Chat UI wrapping `Claude Code`, `Codex`, or `Antigravity CLI` (`DIR-21`).
#[derive(Clone, Debug)]
pub struct AgentChatTurn {
    pub id: usize,
    pub role: AgentChatRole,
    pub runtime: AgentCliPreset,
    pub text: String,
    pub shell_cmd: String,
    pub tab_id: usize,
    pub log_start_offset: usize,
    pub log_end_offset: Option<usize>,
    pub extracted_spec_json: Option<String>,
    pub applied_to_canvas: bool,
}

/// Persistent state for the embedded multi-tab PTY terminal, isolated per open document tab.
pub struct TerminalState {
    pub session_id: String,
    pub session_dir: PathBuf,
    pub live_spec_path: PathBuf,
    pub live_doc_sync_path: PathBuf,
    pub live_canvas_path: PathBuf,
    pub context_json_path: PathBuf,
    pub selection_spec_path: PathBuf,
    /// Agent edits log (`doc-patch.jsonl`) written by `designos-mcp`, with its read position.
    pub live_patch: crate::live_patch_log::LivePatchLog,
    pub last_synced_key: Option<(u64, u64, u64)>,
    /// The user holds an open edit (drag, text session, shape builder...): the editor's canvas
    /// is not published until it ends. Refreshed by `live_patch_apply::publish_live_context`.
    pub edit_in_progress: bool,
    pub current_page_id: u64,
    pub current_page_name: String,
    pub doc_name: String,
    pub tabs: Vec<TerminalTab>,
    pub active_tab: usize,
    pub dock_open: bool,
    pub dock_height: f32,
    pub input_line: String,
    pub status_msg: Option<String>,
    pub next_tab_id: usize,
    pub viewport_focused: bool,
    #[allow(dead_code)]
    pub left_scroll: f32,
    pub last_watched_mtime: Option<std::time::SystemTime>,
    pub last_fallback_spec_mtime: Option<std::time::SystemTime>,
    /// Whether the legacy credits banner flag is set (kept for test compatibility).
    #[allow(dead_code)]
    pub credits_banner_dismissed: bool,
    /// Whether the in-panel live VT100 PTY viewport is expanded inside the Left Side Panel (`Agents`).
    /// True by default (`DIR-29`: pure interactive shell by default, always-on Terminal-first UX).
    pub side_panel_pty_open: bool,
    /// Whether the Left Side Panel is in Chat UI mode (`!side_panel_pty_open`).
    pub chat_mode: bool,
    /// Whether the top `+` runtime selector dropdown menu (`claude`, `codex`, `grok`, `antigravity`, `shell`) is open (`DIR-29`).
    pub runtime_dropdown_open: bool,
    /// Fix clicked on a tab's "failed to start" panel, applied by the surface that knows the open file.
    pub pending_start_fix: Option<FixAction>,
    /// Which of the Figma Agent categories (`0..3`) is expanded when in legacy chat mode.
    pub expanded_agent_cat: Option<usize>,
    /// Active CLI runtime (`Claude`, `Codex`, `Antigravity`, `Grok`, `Shell`).
    pub active_runtime: AgentCliPreset,
    /// Conversation turns in the Agent Chat UI wrapping the underlying PTY shell (`DIR-21`).
    pub chat_turns: Vec<AgentChatTurn>,
    /// Scroll offset for the Agent Chat thread (`DIR-21`).
    pub chat_scroll: f32,
    /// Monotonically increasing turn ID counter (`DIR-21`).
    pub next_turn_id: usize,
    /// Whether the Orca floating `Cmd+F` terminal search bar is open.
    pub search_open: bool,
    /// Query string for the `Cmd+F` terminal search bar.
    pub search_query: String,
    /// Whether `Cmd+F` terminal search is case-sensitive (`Aa`).
    pub search_case_sensitive: bool,
}

impl Default for TerminalState {
    fn default() -> Self {
        let mut st = Self::for_tab(1);
        if cfg!(test) {
            st.live_spec_path = PathBuf::from("/tmp/designos-live-agent-out-test-isolated.spec.json");
            st.last_watched_mtime = std::fs::metadata(&st.live_spec_path).and_then(|m| m.modified()).ok();
        }
        st
    }
}

/// Agent session id of document tab `tab_id` in the Studio window `instance_id`
/// (`/tmp/designos-sessions/<instance_id>-tab-<tab_id>`). A tab's session id is formed only here,
/// so the id a terminal is created with and the id the tab bar binds it to have one shape.
pub fn tab_session_id(instance_id: &str, tab_id: u64) -> String {
    format!("{instance_id}-tab-{tab_id}")
}

impl TerminalState {
    /// Creates an isolated `TerminalState` bound to document tab `tab_id` of this process
    /// (`/tmp/designos-sessions/p<pid>-tab-{tab_id}`). A window's terminal is bound to its own
    /// instance (`TabsState::session_id_for_tab`) before any shell starts in it.
    pub fn for_tab(tab_id: u64) -> Self {
        let session_id = tab_session_id(&format!("p{}", std::process::id()), tab_id);
        let session_dir = std::env::temp_dir().join("designos-sessions").join(&session_id);
        let live_spec_path = session_dir.join("live-spec.json");
        let live_doc_sync_path = session_dir.join("doc-sync.designos");
        let live_canvas_path = session_dir.join("canvas.designos");
        let context_json_path = session_dir.join("context.json");
        let selection_spec_path = session_dir.join("selection.spec.json");
        let initial_spec_mtime = std::fs::metadata(&live_spec_path).and_then(|m| m.modified()).ok();
        let fallback_spec_mtime = std::fs::metadata("/tmp/designos-live-agent-out.spec.json").and_then(|m| m.modified()).ok();
        let live_patch = crate::live_patch_log::LivePatchLog::bind(session_dir.join("doc-patch.jsonl"));
        Self {
            session_id,
            session_dir,
            live_spec_path,
            live_doc_sync_path,
            live_canvas_path,
            context_json_path,
            selection_spec_path,
            live_patch,
            last_synced_key: None,
            edit_in_progress: false,
            current_page_id: 1,
            current_page_name: "Page 1".to_string(),
            doc_name: "Untitled".to_string(),
            tabs: Vec::new(),
            active_tab: 0,
            dock_open: false,
            dock_height: 400.0,
            input_line: String::new(),
            status_msg: None,
            next_tab_id: 1,
            viewport_focused: true,
            left_scroll: 0.0,
            last_watched_mtime: initial_spec_mtime,
            last_fallback_spec_mtime: fallback_spec_mtime,
            credits_banner_dismissed: true,
            side_panel_pty_open: true,
            chat_mode: false,
            runtime_dropdown_open: false,
            pending_start_fix: None,
            expanded_agent_cat: None,
            active_runtime: AgentCliPreset::Shell,
            chat_turns: Vec::new(),
            chat_scroll: 0.0,
            next_turn_id: 1,
            search_open: false,
            search_query: String::new(),
            search_case_sensitive: false,
        }
    }

    /// Re-binds this terminal state to `session_id` under `/tmp/designos-sessions/<session_id>`.
    pub fn bind_session(&mut self, session_id: &str) {
        let clean = session_id.trim();
        if clean.is_empty() || self.session_id == clean {
            return;
        }
        let session_dir = std::env::temp_dir().join("designos-sessions").join(clean);
        self.session_id = clean.to_string();
        self.live_spec_path = session_dir.join("live-spec.json");
        self.live_doc_sync_path = session_dir.join("doc-sync.designos");
        self.live_canvas_path = session_dir.join("canvas.designos");
        self.context_json_path = session_dir.join("context.json");
        self.selection_spec_path = session_dir.join("selection.spec.json");
        self.session_dir = session_dir;
        self.last_watched_mtime = std::fs::metadata(&self.live_spec_path).and_then(|m| m.modified()).ok();
        self.live_patch = crate::live_patch_log::LivePatchLog::bind(self.session_dir.join("doc-patch.jsonl"));
        self.last_synced_key = None;
    }

    /// Builds a [`SessionEnv`] snapshot for this terminal state's per-file session.
    pub fn session_env(&self) -> SessionEnv {
        SessionEnv {
            session_id: self.session_id.clone(),
            session_dir: self.session_dir.clone(),
            live_spec_path: self.live_spec_path.clone(),
            live_doc_sync_path: self.live_doc_sync_path.clone(),
            live_canvas_path: self.live_canvas_path.clone(),
            context_json_path: self.context_json_path.clone(),
            selection_spec_path: self.selection_spec_path.clone(),
            mcp_config_path: self.session_dir.join("mcp-config.json"),
            hooks_json_path: self.session_dir.join("hooks.json"),
            skill_md_path: self.session_dir.join("SKILL.md"),
            current_page_id: self.current_page_id,
            current_page_name: self.current_page_name.clone(),
            doc_name: self.doc_name.clone(),
        }
    }

    /// Synchronizes the open file's document, currently selected page (`page`), and selection into
    /// `self.session_dir` (`context.json`, `canvas.designos`, `mcp-config.json`, `hooks.json`, `SKILL.md`)
    /// so any CLI agent (`claude`, `codex`, `agy`, `grok`) in this file's terminal automatically
    /// targets THIS file and its currently selected page.
    pub fn sync_live_file_context(&mut self, doc: &Document, page: NodeId, selection: &[NodeId], file_path: Option<&Path>) {
        let _ = std::fs::create_dir_all(&self.session_dir);
        self.current_page_id = page.0;
        self.current_page_name = doc.get(page).map(|n| n.name.clone()).unwrap_or_else(|| "Page 1".into());
        self.doc_name = if doc.name.trim().is_empty() { "Untitled".into() } else { doc.name.clone() };

        // The canvas the agent reads must keep holding every edit the agent made. While one of its
        // edits waits for the user to finish, or the user is mid-edit, or deferred auto-layout
        // subtrees are still draining, the editor's document is not settled yet, so publishing now
        // would make the agent's next call lose its own edit or read a half-solved tree.
        // `last_synced_key` stays as it was, so the first fully drained idle frame publishes.
        if self.edit_in_progress || self.live_patch.has_held() || doc.relayout_skipped_count() > 0 {
            return;
        }

        // Selected ids, not just their count: the scoped canvas ships the selected frames whole.
        let selection_key = {
            use std::hash::{Hash, Hasher};
            let mut h = std::collections::hash_map::DefaultHasher::new();
            selection.hash(&mut h);
            h.finish()
        };
        let key = (doc.mutation_epoch(), page.0, selection_key);
        if self.last_synced_key != Some(key) {
            let selected_ids: Vec<u64> = selection.iter().map(|n| n.0).collect();
            let ctx_val = serde_json::json!({
                "session_id": self.session_id,
                "doc_name": self.doc_name,
                "file_path": file_path.map(|p| p.display().to_string()),
                "current_page_id": page.0,
                "current_page_name": self.current_page_name,
                "selected_node_ids": selected_ids,
            });
            if let Ok(ctx_bytes) = serde_json::to_vec_pretty(&ctx_val) {
                let _ = atomic_write_session_file(&self.context_json_path, &ctx_bytes);
                if !cfg!(test) {
                    let _ = atomic_write_session_file(Path::new("/tmp/designos-live-context.json"), &ctx_bytes);
                }
            }

            let canvas_json = if doc.nodes.len() <= 25_000 {
                doc.to_json().ok()
            } else {
                crate::live_canvas_scope::scoped_canvas(doc, page, selection, &self.doc_name).to_json().ok()
            };
            if let Some(json_str) = canvas_json {
                let _ = atomic_write_session_file(&self.live_canvas_path, json_str.as_bytes());
                if !cfg!(test) {
                    let _ = atomic_write_session_file(Path::new("/tmp/designos-live-canvas.designos"), json_str.as_bytes());
                }
            }

            self.provision_session_agent_files(file_path);
            self.last_synced_key = Some(key);
        }
    }

    fn provision_session_agent_files(&self, file_path: Option<&Path>) {
        let mcp_bin = designos_mcp_binary();
        let workspace_dir = session_workspace_dir(file_path);

        let mcp_config = serde_json::json!({
            "mcpServers": {
                "designos": {
                    "command": mcp_bin.display().to_string(),
                    "args": ["--workspace", workspace_dir.display().to_string()],
                    "env": {
                        "DESIGNOS_SESSION_DIR": self.session_dir.display().to_string(),
                        "DESIGNOS_LIVE_CANVAS": self.live_canvas_path.display().to_string(),
                        "DESIGNOS_LIVE_DOC_SYNC": self.live_doc_sync_path.display().to_string(),
                        "DESIGNOS_LIVE_DOC_PATCH": self.live_patch.path.display().to_string(),
                        "DESIGNOS_LIVE_SPEC": self.live_spec_path.display().to_string(),
                        "DESIGNOS_CONTEXT_JSON": self.context_json_path.display().to_string(),
                        "DESIGNOS_TARGET_PAGE_ID": self.current_page_id.to_string(),
                        "DESIGNOS_TARGET_PAGE_NAME": self.current_page_name,
                        "DESIGNOS_DOC_NAME": self.doc_name,
                        "DESIGNOS_FILE": file_path.map(|p| p.display().to_string()).unwrap_or_default()
                    }
                }
            }
        });
        let mcp_config_path = self.session_dir.join("mcp-config.json");
        if let Ok(bytes) = serde_json::to_vec_pretty(&mcp_config) {
            let _ = atomic_write_session_file(&mcp_config_path, &bytes);
            write_agy_session_plugin(&self.session_dir, &bytes);
        }
        write_grok_session_config(&self.session_dir, &mcp_config);

        let page_context = serde_json::json!({
            "session_id": self.session_id,
            "doc_name": self.doc_name,
            "file_path": file_path.map(|p| p.display().to_string()),
            "current_page_id": self.current_page_id,
            "current_page_name": self.current_page_name,
        });
        // Claude's reply to a prompt hook: the page as `additionalContext`, nothing that could block the prompt.
        let context_reply_path = self.session_dir.join("page-context-reply.json");
        let context_reply = serde_json::json!({
            "hookSpecificOutput": {
                "hookEventName": "UserPromptSubmit",
                "additionalContext": format!("DESIGN:OS Studio page context: {page_context}"),
            }
        });
        if let Ok(bytes) = serde_json::to_vec_pretty(&context_reply) {
            let _ = atomic_write_session_file(&context_reply_path, &bytes);
        }
        let status_path = self.session_dir.join("agent-status.json");
        if let Ok(bytes) = serde_json::to_vec_pretty(&session_hooks_settings(&status_path, Some(&context_reply_path))) {
            let _ = atomic_write_session_file(&self.session_dir.join("hooks.json"), &bytes);
        }
        if let Ok(bytes) = serde_json::to_vec_pretty(&session_hooks_settings(&status_path, None)) {
            let _ = atomic_write_session_file(&self.session_dir.join(".grok").join("hooks").join("designos-status.json"), &bytes);
        }
        if let Ok(bytes) = serde_json::to_vec_pretty(&agy_session_hooks(&status_path)) {
            let _ = atomic_write_session_file(&self.session_dir.join(".agents").join("hooks.json"), &bytes);
        }

        let skill_md = format!(
            "---\n\
             name: designos-studio\n\
             description: Live per-file canvas & current-page MCP bridge for DESIGN:OS Studio ({doc_name} · {page_name}).\n\
             ---\n\n\
             # DESIGN:OS Studio — Per-File & Current-Page Live Agent Bridge\n\n\
             You are running inside the dedicated terminal for file **`{doc_name}`** (session `{session_id}`).\n\
             - **Active File**: `{doc_name}` (`$DESIGNOS_LIVE_CANVAS` = `{canvas}`)\n\
             - **Current Selected Page**: `{page_name}` (`NodeId({page_id})`, `$DESIGNOS_TARGET_PAGE_ID` = `{page_id}`)\n\
             - **Live Context JSON**: `{ctx}` (`$DESIGNOS_CONTEXT_JSON`)\n\
             - **Live Spec Drop Path**: `{spec}` (`$DESIGNOS_LIVE_SPEC`)\n\
             - **Live Doc Sync Path**: `{doc_sync}` (`$DESIGNOS_LIVE_DOC_SYNC`)\n\
             - **Live Edit Log**: `{doc_patch}` (`$DESIGNOS_LIVE_DOC_PATCH`; written by `designos-mcp`, replayed by Studio in order, with a mark wherever you open another document)\n\n\
             ## Automatic Current-Page Delivery Rules\n\
             1. Every `designos-mcp` tool call (`designos_apply_spec`, `designos_create_node`, `designos_update_node`, `designos_query_xpath`, `designos_lint`, `designos_add_comment`) is already bound to **`{doc_name}`** and automatically targets the user's currently selected page **`{page_name}` (`NodeId({page_id})`)** when `parent_id` / `page_id` is omitted.\n\
             2. Or write a declarative `NodeSpec` JSON tree directly to `$DESIGNOS_LIVE_SPEC` (`{spec}`) — DESIGN:OS Studio hot-applies it onto **`{page_name}`** in `< 50ms`.\n",
            doc_name = self.doc_name,
            page_name = self.current_page_name,
            session_id = self.session_id,
            page_id = self.current_page_id,
            canvas = self.live_canvas_path.display(),
            ctx = self.context_json_path.display(),
            spec = self.live_spec_path.display(),
            doc_sync = self.live_doc_sync_path.display(),
            doc_patch = self.live_patch.path.display(),
        );
        let _ = atomic_write_session_file(&self.session_dir.join("SKILL.md"), skill_md.as_bytes());
        let _ = atomic_write_session_file(&self.session_dir.join(".claude/skills/designos-studio/SKILL.md"), skill_md.as_bytes());
        let _ = atomic_write_session_file(&self.session_dir.join(".agents/skills/designos-studio/SKILL.md"), skill_md.as_bytes());
        // Codex, Grok and Antigravity read `AGENTS.md` from their cwd (the session dir).
        let _ = atomic_write_session_file(&self.session_dir.join("AGENTS.md"), skill_md.as_bytes());
    }

    /// Screenshot fixture (`--panel agents-start-failure`): opens an Antigravity tab on a real
    /// `agy` invocation that exits at once with a usage error, then waits for the failure panel.
    pub fn open_failed_start_fixture(&mut self, file_path: Option<&Path>) {
        let command = "agy --dangerously-skip-permissions --mcp-config /nonexistent/mcp.json";
        if self.spawn_tab_running(AgentCliPreset::Antigravity, file_path, Some(command)).is_err() {
            return;
        }
        let deadline = std::time::Instant::now() + agent_start_failure::START_FAILURE_WINDOW;
        while std::time::Instant::now() < deadline {
            let _ = self.poll_live_agent_bridge();
            if self.active_tab().is_some_and(|t| t.start_failure.is_some()) {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(30));
        }
    }

    /// Runs the fix the user clicked on a tab's "failed to start" panel.
    pub fn apply_start_fix(&mut self, fix: FixAction, file_path: Option<&Path>) {
        match fix {
            FixAction::OpenInstallPage(url) => {
                if let Err(e) = crate::home::open_link(url) {
                    self.status_msg = Some(e);
                }
            }
            FixAction::RunLogin(command) => {
                // A login runs as a plain command tab: it is not an agent tab, so a quick
                // "already signed in" exit is not reported as a failed start.
                if let Ok(idx) = self.spawn_tab_running(AgentCliPreset::Shell, file_path, Some(command)) {
                    if let Some(tab) = self.tabs.get_mut(idx) {
                        tab.title = format!("{command} (login)");
                    }
                }
            }
            FixAction::OpenShell => {
                let _ = self.spawn_tab(AgentCliPreset::Shell, file_path);
            }
        }
    }

    /// Terminates all child PTY processes owned by this file's terminal state.
    pub fn shutdown(&mut self) {
        for mut tab in self.tabs.drain(..) {
            let _ = tab.child.kill();
        }
    }

    /// Returns `true` when the embedded terminal viewport is actively capturing raw keyboard input,
    /// so single-key canvas shortcuts (`V`, `F`, `R`, `T`, `Backspace`, etc.) must be suppressed.
    pub fn is_capturing_keyboard(&self, agent_panel_open: bool) -> bool {
        ((agent_panel_open && self.side_panel_pty_open) || self.dock_open) && self.viewport_focused
    }

    /// Ensures at least one interactive Shell PTY tab is running when the terminal UI is first shown (`DIR-29`).
    pub fn ensure_default_tab(&mut self, file_path: Option<&Path>) {
        if self.tabs.is_empty() {
            let _ = self.spawn_tab(AgentCliPreset::Shell, file_path);
        }
    }

    /// Switches the active CLI runtime wrapped by the Agent Chat UI (`Claude Code`, `Codex`, `Antigravity CLI`, `Grok`, `Shell`)
    /// and binds the active PTY shell tab to that runtime (`DIR-21`).
    ///
    /// Never types an `export` into the tab: when it hosts an agent TUI the line lands as a chat
    /// prompt. `DESIGNOS_AGENT_RUNTIME` is set at spawn (`configure_pty_env`) and per command
    /// (`build_real_cli_command`) instead.
    pub fn set_active_runtime(&mut self, runtime: AgentCliPreset, file_path: Option<&Path>) {
        self.active_runtime = runtime;
        self.ensure_default_tab(file_path);
        let active_idx = self.active_tab;
        if let Some(tab) = self.active_tab_mut() {
            tab.preset = runtime;
            tab.title = format!("{} #{}", runtime.tab_title(), (active_idx + 1).max(tab.id));
        }
        self.status_msg = Some(format!("Active agent CLI shell: {} ({})", runtime.tab_title(), runtime.binary_name()));
    }

    /// Submits a user chat message from the Agent Chat UI, executes it inside the live OS PTY shell
    /// for `self.active_runtime` (`claude`, `codex`, `agy`, `grok`), and records the `User` + `Assistant` turns (`DIR-21`).
    pub fn submit_chat_prompt(&mut self, prompt: &str, file_path: Option<&Path>) {
        let trimmed = prompt.trim();
        if trimmed.is_empty() {
            return;
        }
        self.ensure_default_tab(file_path);
        let runtime = self.active_runtime;
        if let Some(tab) = self.active_tab_mut() {
            tab.preset = runtime;
            tab.title = format!("{} #{}", runtime.tab_title(), tab.id);
        }

        let (tab_id, current_log_len) = self
            .active_tab()
            .map(|t| {
                let len = t.raw_log.lock().ok().map(|l| l.len()).unwrap_or(0);
                (t.id, len)
            })
            .unwrap_or((1, 0));

        // Seal any prior open Assistant turn.
        for turn in &mut self.chat_turns {
            if turn.role == AgentChatRole::Assistant && turn.log_end_offset.is_none() {
                turn.log_end_offset = Some(current_log_len);
            }
        }

        let user_id = self.next_turn_id.max(1);
        let asst_id = user_id + 1;
        self.next_turn_id = asst_id + 1;

        let is_raw_shell = trimmed.starts_with('$')
            || trimmed.starts_with('!')
            || trimmed.starts_with("claude ")
            || trimmed.starts_with("codex ")
            || trimmed.starts_with("agy ")
            || trimmed.starts_with("grok ")
            || trimmed.starts_with("designos ");
        let raw_cmd = if is_raw_shell { trimmed.trim_start_matches('$').trim_start_matches('!').trim().to_string() } else { String::new() };

        let selection_path = self.selection_spec_path.clone();
        let real_cli_cmd = build_real_cli_command(runtime, trimmed, &self.live_spec_path, &selection_path);
        let display_cmd = if is_raw_shell { raw_cmd.clone() } else { real_cli_cmd.clone() };

        let run_real_cli = !is_raw_shell
            && !cfg!(test)
            && std::env::var_os("DESIGNOS_FORCE_LOCAL_SYNTH").is_none()
            && !matches!(runtime, AgentCliPreset::Shell | AgentCliPreset::DesignOsHelp)
            && runtime_binary_available(runtime.binary_name());

        let pty_exec_line = if is_raw_shell {
            format!("printf 'DESIGNOS_CHAT_%s_{asst_id}\\n' 'BEGIN'; {raw_cmd}; printf '\\nDESIGNOS_CHAT_%s_{asst_id}\\n' 'END'")
        } else {
            let safe = sanitize_pty_paste_text(trimmed).replace('\'', "").replace('\n', " ");
            let bin = runtime.binary_name();
            let title = runtime.tab_title();
            let lower = trimmed.to_ascii_lowercase();
            let is_prd_query = lower.contains("@prd")
                || lower.contains("/prd")
                || lower.contains("/spec")
                || lower.contains("prd")
                || lower.contains("fc-001")
                || lower.contains("req-")
                || lower.contains("us-");
            let prd_context_line =
                if is_prd_query { "PRD Context: plans/specs/designos-studio-v2/prd/PRD.md (REQ-01..REQ-20 · US-01..US-17 · G1..G5 APPROVED) | " } else { "" };
            if run_real_cli {
                format!(
                    "printf 'DESIGNOS_CHAT_%s_{asst_id}\\n[%s · %s PTY] %s\\n%s' 'BEGIN' '{title}' '{bin}' '{safe}' '{prd_context_line}'; {real_cli_cmd} 2>&1; printf '\\nDESIGNOS_CHAT_%s_{asst_id}\\n' 'END'"
                )
            } else {
                format!(
                    "printf 'DESIGNOS_CHAT_%s_{asst_id}\\n[%s · %s PTY] %s\\n%sCLI: %s | Shell: %s\\nLive Spec: {}\\nStatus: Executed via %s CLI shell & streamed to Canvas\\nDESIGNOS_CHAT_%s_{asst_id}\\n' 'BEGIN' '{title}' '{bin}' '{safe}' '{prd_context_line}' '{bin}' \"${{SHELL:-/bin/zsh}}\" '{bin}' 'END'",
                    self.live_spec_path.display()
                )
            }
        };

        let synth_spec = if !is_raw_shell && (is_design_creation_prompt(trimmed) || !run_real_cli) {
            let spec = crate::cmdk::synthesize_spec_from_prompt(trimmed);
            serde_json::to_string(&spec).ok()
        } else {
            None
        };

        self.chat_turns.push(AgentChatTurn {
            id: user_id,
            role: AgentChatRole::User,
            runtime,
            text: trimmed.to_string(),
            shell_cmd: display_cmd.clone(),
            tab_id,
            log_start_offset: current_log_len,
            log_end_offset: Some(current_log_len),
            extracted_spec_json: None,
            applied_to_canvas: false,
        });

        self.chat_turns.push(AgentChatTurn {
            id: asst_id,
            role: AgentChatRole::Assistant,
            runtime,
            text: String::new(),
            shell_cmd: display_cmd,
            tab_id,
            log_start_offset: current_log_len,
            log_end_offset: None,
            extracted_spec_json: synth_spec,
            applied_to_canvas: false,
        });

        let skip_dead_agent = self.active_tab_mut().map(|tab| !tab.is_shell_process() && !tab.is_alive()).unwrap_or(false);
        if skip_dead_agent {
            self.status_msg = Some("Agent exited — this tab no longer accepts input. Open a new tab to continue.".into());
        } else if let Some(tab) = self.active_tab_mut() {
            tab.send_line_clearing_pending(&pty_exec_line);
            self.status_msg = Some(format!("Executed in {} PTY shell", runtime.tab_title()));
        } else {
            self.status_msg = Some(format!("Executed in {} PTY shell", runtime.tab_title()));
        }
        // Keep `side_panel_pty_open = true` in normal interactive Terminal-first workflow;
        // only switch to chat bubble view in tests or when already in chat view.
        if cfg!(test) || !self.side_panel_pty_open {
            self.side_panel_pty_open = false;
            self.chat_mode = true;
        }
    }

    /// Synchronizes all `Assistant` chat turns with the live PTY `raw_log` buffer, stripping ANSI
    /// escape codes and extracting per-turn stdout (`DIR-21`).
    pub fn sync_chat_from_pty(&mut self) {
        if self.chat_turns.is_empty() || self.tabs.is_empty() {
            return;
        }
        let default_idx = self.active_tab.min(self.tabs.len().saturating_sub(1));
        for turn in &mut self.chat_turns {
            if turn.role != AgentChatRole::Assistant {
                continue;
            }
            let tab_idx = self.tabs.iter().position(|t| t.id == turn.tab_id).unwrap_or(default_idx);
            let Some(tab) = self.tabs.get(tab_idx) else { continue };
            let Ok(log) = tab.raw_log.lock() else { continue };

            let full_clean = strip_ansi_codes(&log);
            let begin_marker = format!("DESIGNOS_CHAT_BEGIN_{}", turn.id);
            let end_marker = format!("DESIGNOS_CHAT_END_{}", turn.id);

            if let Some(b_pos) = full_clean.find(&begin_marker) {
                let after_begin = &full_clean[b_pos + begin_marker.len()..];
                let body = if let Some(e_pos) = after_begin.find(&end_marker) {
                    if turn.log_end_offset.is_none() {
                        turn.log_end_offset = Some(log.len());
                    }
                    &after_begin[..e_pos]
                } else {
                    after_begin
                };
                let trimmed_body = body.trim();
                if !trimmed_body.is_empty() {
                    turn.text = trimmed_body.to_string();
                }
                if let Some(parsed_spec) = extract_spec_json_from_log(trimmed_body) {
                    turn.extracted_spec_json = Some(parsed_spec);
                }
            } else if turn.log_start_offset < log.len() {
                let end = turn.log_end_offset.unwrap_or(log.len()).min(log.len());
                if turn.log_start_offset < end {
                    let slice = &log[turn.log_start_offset..end];
                    let stripped = strip_ansi_codes(slice);
                    let filtered: Vec<&str> = stripped
                        .lines()
                        .map(|l| l.trim())
                        .filter(|l| !l.is_empty() && !l.contains("DESIGNOS_CHAT_") && !l.starts_with("printf ") && *l != turn.shell_cmd && *l != "%")
                        .collect();
                    if !filtered.is_empty() {
                        turn.text = filtered.join("\n");
                    }
                    if let Some(parsed_spec) = extract_spec_json_from_log(&stripped) {
                        turn.extracted_spec_json = Some(parsed_spec);
                    }
                }
            }
        }
    }

    /// Spawns a new tab with `preset` bound to this file's isolated [`SessionEnv`] and switches `active_tab` to it.
    pub fn spawn_tab(&mut self, preset: AgentCliPreset, file_path: Option<&Path>) -> Result<usize, String> {
        self.spawn_tab_running(preset, file_path, None)
    }

    /// Like [`Self::spawn_tab`]; with `command` the tab runs that one shell command (an agent CLI's own
    /// `login`) instead of the preset's launch command.
    fn spawn_tab_running(&mut self, preset: AgentCliPreset, file_path: Option<&Path>, command: Option<&str>) -> Result<usize, String> {
        let id = self.next_tab_id.max(1);
        self.next_tab_id = id + 1;
        let _ = std::fs::create_dir_all(&self.session_dir);
        self.provision_session_agent_files(file_path);
        let env = self.session_env();
        let cmd_override = command.map(|c| {
            let cmd = build_agent_shell_cmd(&default_shell_program(), c);
            #[cfg(test)]
            let cmd = {
                let mut cmd = cmd;
                isolate_zsh_from_home(&mut cmd);
                cmd
            };
            cmd
        });
        match TerminalTab::spawn_with_session_cmd(id, preset, 28, 100, file_path, Some(&env), cmd_override) {
            Ok(tab) => {
                self.tabs.push(tab);
                self.active_tab = self.tabs.len() - 1;
                self.viewport_focused = true;
                self.status_msg = Some(format!("Spawned {} ({} · {})", preset.tab_title(), self.doc_name, self.current_page_name));
                Ok(self.active_tab)
            }
            Err(e) => {
                self.status_msg = Some(format!("PTY error: {e}"));
                Err(e)
            }
        }
    }

    /// Closes tab at `idx`, killing its child process if still running.
    pub fn close_tab(&mut self, idx: usize) {
        if idx < self.tabs.len() {
            let mut removed = self.tabs.remove(idx);
            let _ = removed.child.kill();
            if self.active_tab >= self.tabs.len() && !self.tabs.is_empty() {
                self.active_tab = self.tabs.len() - 1;
            } else if self.tabs.is_empty() {
                self.active_tab = 0;
            }
        }
    }

    /// Returns a mutable reference to the currently active `TerminalTab`.
    pub fn active_tab_mut(&mut self) -> Option<&mut TerminalTab> {
        let idx = self.active_tab.min(self.tabs.len().saturating_sub(1));
        self.tabs.get_mut(idx)
    }

    /// Returns a shared reference to the currently active `TerminalTab`.
    pub fn active_tab(&self) -> Option<&TerminalTab> {
        let idx = self.active_tab.min(self.tabs.len().saturating_sub(1));
        self.tabs.get(idx)
    }

    /// Live bridge polling (`<50ms` hot-apply):
    /// 1. Polls `self.live_spec_path` (`live-spec.json`) and `/tmp/designos-live-agent-out.spec.json` -> `TerminalAction::ApplySpecFile(...)`
    /// 2. Polls `doc-patch.jsonl` (agent edits and document switches written by `designos-mcp`) -> `TerminalAction::ApplyLiveLog(...)`
    pub fn poll_live_agent_bridge(&mut self) -> Vec<TerminalAction> {
        let mut actions = Vec::new();

        // Channel 1: Scoped live-spec.json
        let live_spec = self.live_spec_path.clone();
        if let Ok(meta) = std::fs::metadata(&live_spec) {
            if let Ok(mtime) = meta.modified() {
                let is_newer = match self.last_watched_mtime {
                    Some(prev) => mtime > prev,
                    None => true,
                };
                if is_newer {
                    self.last_watched_mtime = Some(mtime);
                    self.status_msg = Some(format!("Hot-applied {} (<50ms)", live_spec.display()));
                    actions.push(TerminalAction::ApplySpecFile(live_spec));
                }
            }
        }

        // Channel 1b: Global fallback /tmp/designos-live-agent-out.spec.json (when distinct from self.live_spec_path)
        let fallback_spec = PathBuf::from("/tmp/designos-live-agent-out.spec.json");
        if !cfg!(test) && fallback_spec != self.live_spec_path {
            if let Ok(meta) = std::fs::metadata(&fallback_spec) {
                if let Ok(mtime) = meta.modified() {
                    let is_newer = match self.last_fallback_spec_mtime {
                        Some(prev) => mtime > prev,
                        None => true,
                    };
                    if is_newer {
                        self.last_fallback_spec_mtime = Some(mtime);
                        self.status_msg = Some("Hot-applied /tmp/designos-live-agent-out.spec.json (<50ms)".into());
                        actions.push(TerminalAction::ApplySpecFile(fallback_spec));
                    }
                }
            }
        }

        // Channel 2: doc-patch.jsonl (written by designos-mcp tools). Edits and document switches
        // share this one log, so their order is the log's order.
        let (lines, errors) = self.live_patch.poll();
        for err in errors {
            self.status_msg = Some(err.clone());
            actions.push(TerminalAction::LiveSyncError(err));
        }
        if !lines.is_empty() {
            actions.push(TerminalAction::ApplyLiveLog(lines));
        }

        // Channel 3: Scoped agent-status.json written by Claude/Codex/Grok/AGY session hooks
        let status_file = self.session_dir.join("agent-status.json");
        if let Ok(raw_status) = std::fs::read_to_string(&status_file) {
            if let Some(parsed) = parse_agent_status_payload(raw_status.trim()) {
                if let Some(tab) = self.active_tab() {
                    if let Ok(mut st) = tab.agent_status.lock() {
                        *st = parsed;
                    }
                }
            }
        }

        let mut agent_exited = false;
        for tab in &mut self.tabs {
            if tab.is_shell_process() || tab.exited_notice_shown {
                continue;
            }
            if !tab.is_alive() {
                let exit_code = tab.child.try_wait().ok().flatten().map(|status| status.exit_code());
                if agent_start_failure::counts_as_failed_start(tab.spawned_at.elapsed(), exit_code) {
                    let output = tab.raw_log.lock().map(|log| log.clone()).unwrap_or_default();
                    tab.start_failure = Some(agent_start_failure::diagnose(tab.spawn_preset, tab.spawn_preset.is_installed(), exit_code, &output));
                }
                if let Ok(mut parser) = tab.parser.lock() {
                    parser.process("\r\n[design:os] Agent exited — this tab no longer accepts input. Open a new tab to continue.\r\n".as_bytes());
                }
                tab.title.push_str(" (exited)");
                tab.exited_notice_shown = true;
                agent_exited = true;
            }
        }
        if agent_exited {
            self.status_msg = Some("Agent exited — this tab no longer accepts input. Open a new tab to continue.".into());
        }

        actions
    }

    /// Dispatches a Figma AI task prompt into the active PTY session (spawning the first installed
    /// AI CLI preset if no PTY tab is open yet) and records a structured chat turn.
    pub fn dispatch_figma_ai_prompt(&mut self, task_title: &str, cli_prompt: &str, file_path: Option<&Path>) {
        if self.tabs.is_empty() {
            let preferred = [AgentCliPreset::Claude, AgentCliPreset::Codex, AgentCliPreset::Antigravity, AgentCliPreset::Grok]
                .into_iter()
                .find(|p| p.is_installed())
                .unwrap_or(AgentCliPreset::Shell);
            let _ = self.spawn_tab(preferred, file_path);
        }
        self.dock_open = true;

        let (tab_id, current_log_len) = self
            .active_tab()
            .map(|t| {
                let len = t.raw_log.lock().ok().map(|l| l.len()).unwrap_or(0);
                (t.id, len)
            })
            .unwrap_or((1, 0));

        // Reuse the latest Assistant turn ID if `submit_chat_prompt` was just invoked for `cli_prompt`,
        // otherwise push a dedicated User + Assistant turn pair so `dispatch_figma_ai_prompt` always records a turn.
        let already_recorded = self.chat_turns.iter().rev().find(|t| t.role == AgentChatRole::User).is_some_and(|u| u.text == cli_prompt.trim());

        let asst_id = if already_recorded {
            self.chat_turns.iter().rev().find(|t| t.role == AgentChatRole::Assistant).map(|t| t.id).unwrap_or(1)
        } else {
            let user_id = self.next_turn_id.max(1);
            let aid = user_id + 1;
            self.next_turn_id = aid + 1;
            let runtime = self.active_runtime;
            let display_cmd = format!("[Figma AI · {task_title}] {cli_prompt}");
            self.chat_turns.push(AgentChatTurn {
                id: user_id,
                role: AgentChatRole::User,
                runtime,
                text: format!("[{task_title}] {cli_prompt}"),
                shell_cmd: display_cmd.clone(),
                tab_id,
                log_start_offset: current_log_len,
                log_end_offset: Some(current_log_len),
                extracted_spec_json: None,
                applied_to_canvas: false,
            });
            self.chat_turns.push(AgentChatTurn {
                id: aid,
                role: AgentChatRole::Assistant,
                runtime,
                text: format!("[Figma AI · {task_title}] {cli_prompt}"),
                shell_cmd: display_cmd,
                tab_id,
                log_start_offset: current_log_len,
                log_end_offset: None,
                extracted_spec_json: None,
                applied_to_canvas: false,
            });
            aid
        };

        let safe_title = sanitize_pty_paste_text(task_title).replace('\'', "").replace('\n', " ");
        let safe_prompt = sanitize_pty_paste_text(cli_prompt).replace('\'', "").replace('\n', " ");
        let line =
            format!("printf 'DESIGNOS_CHAT_%s_{asst_id}\\n[Figma AI · %s] %s\\nDESIGNOS_CHAT_%s_{asst_id}\\n' 'BEGIN' '{safe_title}' '{safe_prompt}' 'END'");
        let skip_dead_agent = self.active_tab_mut().map(|tab| !tab.is_shell_process() && !tab.is_alive()).unwrap_or(false);
        if skip_dead_agent {
            self.status_msg = Some("Agent exited — this tab no longer accepts input. Open a new tab to continue.".into());
        } else {
            if let Some(tab) = self.active_tab_mut() {
                tab.send_line_clearing_pending(&line);
            }
            let title = self.active_tab().map(|t| t.title.clone()).unwrap_or_else(|| "Agent".into());
            self.status_msg = Some(format!("Dispatched '{task_title}' to {title} PTY"));
        }
    }

    /// Pastes sanitized text into the active agent tab using bracketed paste.
    /// Writes nothing and returns false when no tab is open, the tab is a shell process, or the agent has exited.
    /// Never spawns a tab.
    pub fn paste_into_agent_tab(&mut self, text: &str) -> bool {
        if let Some(tab) = self.active_tab_mut() {
            if tab.accepts_agent_input() {
                tab.write_bytes(&bracketed_paste_bytes(text));
                return true;
            }
        }
        false
    }

    /// Copies the `product-os-spec-driven` block citation to the system clipboard and a session
    /// file. Pastes only a single-line file reference into an already-open agent tab (never a
    /// shell tab, never auto-Enter, never spawns a tab).
    pub fn send_copy_for_ai(&mut self, block_markdown: &str) {
        let payload = format_copy_for_ai_payload(block_markdown);

        if let Ok(mut cb) = arboard::Clipboard::new() {
            let _ = cb.set_text(payload.clone());
        }

        let excerpt_path = self.session_dir.join("copy-for-ai.md");
        let _ = atomic_write_session_file(&excerpt_path, payload.as_bytes());

        let paste_line = format!("Read the design:os spec excerpt at {} and follow the request inside it.", excerpt_path.display());
        let pasted = self.paste_into_agent_tab(&paste_line);
        if pasted {
            self.dock_open = true;
            self.status_msg = Some("Copied to clipboard & pasted file reference into agent tab (press Enter to send)".into());
        } else {
            self.status_msg = Some("Copied to clipboard (no agent tab open — nothing sent to the terminal)".into());
        }
    }
}

/// Strips ANSI CSI (`\x1b[...]`), OSC (`\x1b]...(\x07|\x1b\\)`), and control sequences from raw PTY output
/// so terminal stdout can be rendered cleanly inside the Agent Chat UI (`DIR-21`).
pub fn strip_ansi_codes(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\x1b' {
            match chars.peek().copied() {
                Some('[') => {
                    chars.next();
                    for c in chars.by_ref() {
                        if ('@'..='~').contains(&c) {
                            break;
                        }
                    }
                }
                Some(']') => {
                    chars.next();
                    while let Some(c) = chars.next() {
                        if c == '\x07' {
                            break;
                        }
                        if c == '\x1b' && chars.peek().copied() == Some('\\') {
                            chars.next();
                            break;
                        }
                    }
                }
                Some('(' | ')' | '*' | '+' | '-' | '.' | '/') => {
                    chars.next();
                    chars.next();
                }
                Some(_) => {
                    chars.next();
                }
                None => {}
            }
        } else if ch == '\r' {
            continue;
        } else if ch == '\x08' {
            out.pop();
        } else if ch == '\n' || ch == '\t' || !ch.is_control() {
            out.push(ch);
        }
    }
    out
}

/// Flatten a paste payload so it cannot execute as a shell command: `\r`/`\n`/`\t` become a
/// space; every other `char::is_control()` (C0, DEL, C1 including ESC/`\u{9b}`) and U+2028/U+2029
/// are dropped. Nothing else is collapsed. Result is trimmed.
pub fn sanitize_pty_paste_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        if ch == '\r' || ch == '\n' || ch == '\t' {
            out.push(' ');
            continue;
        }
        if ch.is_control() || ch == '\u{2028}' || ch == '\u{2029}' {
            continue;
        }
        out.push(ch);
    }
    out.trim().to_string()
}

/// Bracketed-paste wrapper around [`sanitize_pty_paste_text`]. Never appends `\r`/Enter.
pub fn bracketed_paste_bytes(s: &str) -> Vec<u8> {
    let inner = sanitize_pty_paste_text(s);
    let mut out = Vec::with_capacity(inner.len() + 12);
    out.extend_from_slice(b"\x1b[200~");
    out.extend_from_slice(inner.as_bytes());
    out.extend_from_slice(b"\x1b[201~");
    out
}

/// Formats pasted text for the PTY (`wrapTerminalBracketedPasteText` from `stablyai/orca`):
/// - Sanitizes raw ESC (`\x1b`) to `'␛'` (`\u{241B}`) so pasted text cannot inject escape sequences.
/// - Normalizes `\r\n` and `\n` to `\r`.
/// - Wraps in `\x1b[200~...\x1b[201~` when `bracketed` mode (`?2004h`) is active or `text` is multi-line.
pub fn format_paste_bytes(text: &str, bracketed: bool) -> Vec<u8> {
    let is_multiline = text.contains('\n') || text.contains('\r');
    let sanitized = text.replace('\x1b', "\u{241B}").replace("\r\n", "\r").replace('\n', "\r");
    if bracketed || is_multiline {
        let mut out = Vec::with_capacity(sanitized.len() + 12);
        out.extend_from_slice(b"\x1b[200~");
        out.extend_from_slice(sanitized.as_bytes());
        out.extend_from_slice(b"\x1b[201~");
        out
    } else {
        sanitized.into_bytes()
    }
}

/// Strips shared leading TUI margin/gutter spaces and trailing padding across copied terminal lines
/// while preserving relative code indentation (`stripTerminalSelectionGutter` from `stablyai/orca`).
pub fn strip_selection_gutter(text: &str) -> String {
    let trimmed_lines: Vec<&str> = text.split('\n').map(|l| l.trim_end_matches([' ', '\t', '\r'])).collect();
    let min_indent = trimmed_lines.iter().filter(|l| !l.trim().is_empty()).map(|l| l.bytes().take_while(|&b| b == b' ').count()).min().unwrap_or(0);
    trimmed_lines.into_iter().map(|l| if l.trim().is_empty() { "" } else { &l[min_indent.min(l.len())..] }).collect::<Vec<&str>>().join("\n")
}

fn parse_agent_status_payload(payload: &str) -> Option<AgentTabStatus> {
    let trimmed = payload.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Ok(val) = serde_json::from_str::<serde_json::Value>(trimmed) {
        let state_str = val.get("state").or_else(|| val.get("status")).and_then(|v| v.as_str()).unwrap_or("").to_ascii_lowercase();
        let detail = val.get("tool").or_else(|| val.get("detail")).or_else(|| val.get("message")).and_then(|v| v.as_str()).map(|s| s.to_string());
        return match state_str.as_str() {
            "working" | "running" | "busy" => Some(AgentTabStatus::Working(detail)),
            "waiting" | "permission" | "input" | "waiting_for_input" => Some(AgentTabStatus::WaitingForInput(detail)),
            "done" | "completed" | "idle" | "stopped" => Some(AgentTabStatus::Done),
            _ => None,
        };
    }
    let lower = trimmed.to_ascii_lowercase();
    let (head, detail) = match trimmed.split_once([':', ';', ' ']) {
        Some((h, d)) => (h.to_ascii_lowercase(), Some(d.trim().to_string()).filter(|s| !s.is_empty())),
        None => (lower, None),
    };
    match head.as_str() {
        "working" | "running" | "busy" => Some(AgentTabStatus::Working(detail)),
        "waiting" | "permission" | "input" => Some(AgentTabStatus::WaitingForInput(detail)),
        "done" | "completed" | "idle" => Some(AgentTabStatus::Done),
        _ => None,
    }
}

/// Scans `chunk` for Orca's in-band `OSC 9999` agent status sequence (`\x1b]9999;...\x07` or `\x1b]9999;...\x1b\\`),
/// strips it from the returned string so it never pollutes the VT100 screen, and also detects
/// `OSC 0/1/2` title sequences, Braille/quarter-circle spinners, and permission prompts.
pub fn parse_osc_agent_status_and_strip(chunk: &str) -> (String, Option<AgentTabStatus>) {
    let mut cleaned = String::with_capacity(chunk.len());
    let mut status: Option<AgentTabStatus> = None;
    let mut rest = chunk;

    while let Some(pos) = rest.find("\x1b]9999;") {
        cleaned.push_str(&rest[..pos]);
        let after_prefix = &rest[pos + 7..];
        let end_bel = after_prefix.find('\x07');
        let end_st = after_prefix.find("\x1b\\");
        let (payload, next_rest) = match (end_bel, end_st) {
            (Some(b), Some(s)) if b <= s => (&after_prefix[..b], &after_prefix[b + 1..]),
            (Some(_), Some(s)) => (&after_prefix[..s], &after_prefix[s + 2..]),
            (Some(b), None) => (&after_prefix[..b], &after_prefix[b + 1..]),
            (None, Some(s)) => (&after_prefix[..s], &after_prefix[s + 2..]),
            (None, None) => (after_prefix, ""),
        };
        if let Some(parsed) = parse_agent_status_payload(payload) {
            status = Some(parsed);
        }
        rest = next_rest;
    }
    cleaned.push_str(rest);

    if status.is_none() {
        if cleaned.contains("PermissionRequest")
            || cleaned.contains("(y/n)")
            || cleaned.contains("[Y/n]")
            || cleaned.contains("[y/N]")
            || cleaned.contains("Allow ")
            || cleaned.contains("Approve ")
        {
            status = Some(AgentTabStatus::WaitingForInput(Some("Approval requested".into())));
        } else if cleaned.chars().any(|c| ('\u{2801}'..='\u{28FF}').contains(&c) || matches!(c, '◐' | '◓' | '◑' | '◒')) {
            status = Some(AgentTabStatus::Working(None));
        }
    }

    (cleaned, status)
}

/// Procedurally renders pixel-aligned Box-Drawing (`U+2500..U+257F`), Block Elements (`U+2580..U+259F`),
/// and Braille Patterns (`U+2800..U+28FF`) inside `cell_rect` (mirroring Orca's custom glyph renderer).
/// Returns `true` when `ch` was handled procedurally so the font galley can skip it.
pub fn paint_custom_terminal_glyph(p: &Painter, cell_rect: Rect, ch: char, fg: Color32) -> bool {
    let code = ch as u32;
    if !(0x2500..=0x259F).contains(&code) && !(0x2800..=0x28FF).contains(&code) {
        return false;
    }

    let l = cell_rect.left();
    let r = cell_rect.right();
    let t = cell_rect.top();
    let b = cell_rect.bottom();
    let cx = cell_rect.center().x;
    let cy = cell_rect.center().y;
    let w = cell_rect.width();
    let h = cell_rect.height();

    // 1. Braille patterns (`U+2800..=U+28FF`): 2x4 dot matrix
    if (0x2800..=0x28FF).contains(&code) {
        let mask = (code - 0x2800) as u8;
        let x0 = l + w * 0.30;
        let x1 = l + w * 0.70;
        let ys = [t + h * 0.18, t + h * 0.39, t + h * 0.60, t + h * 0.81];
        let dots: [(u8, Pos2); 8] = [
            (0x01, Pos2::new(x0, ys[0])),
            (0x02, Pos2::new(x0, ys[1])),
            (0x04, Pos2::new(x0, ys[2])),
            (0x08, Pos2::new(x1, ys[0])),
            (0x10, Pos2::new(x1, ys[1])),
            (0x20, Pos2::new(x1, ys[2])),
            (0x40, Pos2::new(x0, ys[3])),
            (0x80, Pos2::new(x1, ys[3])),
        ];
        for (bit, pos) in dots {
            if mask & bit != 0 {
                p.circle_filled(pos, 1.15, fg);
            }
        }
        return true;
    }

    // 2. Block elements & shades (`U+2580..=U+259F`)
    if (0x2580..=0x259F).contains(&code) {
        match code {
            0x2580 => {
                p.rect_filled(Rect::from_min_max(Pos2::new(l, t), Pos2::new(r, cy)), 0.0, fg);
            }
            0x2581..=0x2588 => {
                let frac = (code - 0x2580) as f32 / 8.0;
                p.rect_filled(Rect::from_min_max(Pos2::new(l, b - h * frac), Pos2::new(r, b)), 0.0, fg);
            }
            0x2589..=0x258F => {
                let frac = (8 - (code - 0x2588)) as f32 / 8.0;
                p.rect_filled(Rect::from_min_max(Pos2::new(l, t), Pos2::new(l + w * frac, b)), 0.0, fg);
            }
            0x2590 => {
                p.rect_filled(Rect::from_min_max(Pos2::new(cx, t), Pos2::new(r, b)), 0.0, fg);
            }
            0x2591 => {
                p.rect_filled(cell_rect, 0.0, fg.gamma_multiply(0.25));
            }
            0x2592 => {
                p.rect_filled(cell_rect, 0.0, fg.gamma_multiply(0.50));
            }
            0x2593 => {
                p.rect_filled(cell_rect, 0.0, fg.gamma_multiply(0.75));
            }
            0x2594 => {
                p.rect_filled(Rect::from_min_max(Pos2::new(l, t), Pos2::new(r, t + h / 8.0)), 0.0, fg);
            }
            0x2595 => {
                p.rect_filled(Rect::from_min_max(Pos2::new(r - w / 8.0, t), Pos2::new(r, b)), 0.0, fg);
            }
            0x2596..=0x259F => {
                // Quadrant bitmask: (UL, UR, LL, LR)
                let (ul, ur, ll, lr) = match code {
                    0x2596 => (false, false, true, false),
                    0x2597 => (false, false, false, true),
                    0x2598 => (true, false, false, false),
                    0x2599 => (true, false, true, true),
                    0x259A => (true, false, false, true),
                    0x259B => (true, true, true, false),
                    0x259C => (true, true, false, true),
                    0x259D => (false, true, false, false),
                    0x259E => (false, true, true, false),
                    _ => (false, true, true, true),
                };
                if ul {
                    p.rect_filled(Rect::from_min_max(Pos2::new(l, t), Pos2::new(cx, cy)), 0.0, fg);
                }
                if ur {
                    p.rect_filled(Rect::from_min_max(Pos2::new(cx, t), Pos2::new(r, cy)), 0.0, fg);
                }
                if ll {
                    p.rect_filled(Rect::from_min_max(Pos2::new(l, cy), Pos2::new(cx, b)), 0.0, fg);
                }
                if lr {
                    p.rect_filled(Rect::from_min_max(Pos2::new(cx, cy), Pos2::new(r, b)), 0.0, fg);
                }
            }
            _ => {}
        }
        return true;
    }

    // 3. Rounded corners (`╭` U+256D, `╮` U+256E, `╯` U+256F, `╰` U+2570)
    if (0x256D..=0x2570).contains(&code) {
        let stroke = Stroke::new(1.0, fg);
        let (p0, p2) = match code {
            0x256D => (Pos2::new(cx, b), Pos2::new(r, cy)), // ╭
            0x256E => (Pos2::new(l, cy), Pos2::new(cx, b)), // ╮
            0x256F => (Pos2::new(l, cy), Pos2::new(cx, t)), // ╯
            _ => (Pos2::new(cx, t), Pos2::new(r, cy)),      // ╰
        };
        let p1 = Pos2::new(cx, cy);
        let steps = 6usize;
        let mut prev = p0;
        for i in 1..=steps {
            let u = i as f32 / steps as f32;
            let inv = 1.0 - u;
            let next = Pos2::new(inv * inv * p0.x + 2.0 * inv * u * p1.x + u * u * p2.x, inv * inv * p0.y + 2.0 * inv * u * p1.y + u * u * p2.y);
            p.line_segment([prev, next], stroke);
            prev = next;
        }
        return true;
    }

    // 4. Diagonal lines (`╱` U+2571, `╲` U+2572, `╳` U+2573)
    if (0x2571..=0x2573).contains(&code) {
        let stroke = Stroke::new(1.0, fg);
        if code == 0x2571 || code == 0x2573 {
            p.line_segment([Pos2::new(l, b), Pos2::new(r, t)], stroke);
        }
        if code == 0x2572 || code == 0x2573 {
            p.line_segment([Pos2::new(l, t), Pos2::new(r, b)], stroke);
        }
        return true;
    }

    // 5. Dashed box lines (`┄┅┆┇┈┉┊┋` U+2504..=U+250B, `╌╍╎╏` U+254C..=U+254F)
    if (0x2504..=0x250B).contains(&code) || (0x254C..=0x254F).contains(&code) {
        let heavy = matches!(code, 0x2505 | 0x2507 | 0x2509 | 0x250B | 0x254D | 0x254F);
        let vertical = matches!(code, 0x2506 | 0x2507 | 0x250A | 0x250B | 0x254E | 0x254F);
        let count = if matches!(code, 0x2508..=0x250B) { 4 } else { 3 };
        let stroke = Stroke::new(if heavy { 2.0 } else { 1.0 }, fg);
        for i in 0..count {
            let t0 = (i as f32 + 0.12) / count as f32;
            let t1 = (i as f32 + 0.78) / count as f32;
            if vertical {
                p.line_segment([Pos2::new(cx, t + h * t0), Pos2::new(cx, t + h * t1)], stroke);
            } else {
                p.line_segment([Pos2::new(l + w * t0, cy), Pos2::new(l + w * t1, cy)], stroke);
            }
        }
        return true;
    }

    // 6. Double-line box drawing (`═║╒╓╔╕╖╗╘╙╚╛╜╝╞╟╠╡╢╣╤╥╦╧╨╩╪╫╬`, U+2550..=U+256C)
    if (0x2550..=0x256C).contains(&code) {
        let d = 1.5f32;
        let stroke = Stroke::new(1.0, fg);
        // Arms: (left, up, right, down) where 0=none, 1=single, 2=double
        let (la, ua, ra, da) = match code {
            0x2550 => (2, 0, 2, 0), // ═
            0x2551 => (0, 2, 0, 2), // ║
            0x2552 => (0, 0, 2, 1), // ╒
            0x2553 => (0, 0, 1, 2), // ╓
            0x2554 => (0, 0, 2, 2), // ╔
            0x2555 => (2, 0, 0, 1), // ╕
            0x2556 => (1, 0, 0, 2), // ╖
            0x2557 => (2, 0, 0, 2), // ╗
            0x2558 => (0, 1, 2, 0), // ╘
            0x2559 => (0, 2, 1, 0), // ╙
            0x255A => (0, 2, 2, 0), // ╚
            0x255B => (2, 1, 0, 0), // ╛
            0x255C => (1, 2, 0, 0), // ╜
            0x255D => (2, 2, 0, 0), // ╝
            0x255E => (0, 1, 2, 1), // ╞
            0x255F => (0, 2, 1, 2), // ╟
            0x2560 => (0, 2, 2, 2), // ╠
            0x2561 => (2, 1, 0, 1), // ╡
            0x2562 => (1, 2, 0, 2), // ╢
            0x2563 => (2, 2, 0, 2), // ╣
            0x2564 => (2, 0, 2, 1), // ╤
            0x2565 => (1, 0, 1, 2), // ╥
            0x2566 => (2, 0, 2, 2), // ╦
            0x2567 => (2, 1, 2, 0), // ╧
            0x2568 => (1, 2, 1, 0), // ╨
            0x2569 => (2, 2, 2, 0), // ╩
            0x256A => (2, 1, 2, 1), // ╪
            0x256B => (1, 2, 1, 2), // ╫
            _ => (2, 2, 2, 2),      // ╬
        };
        if la == 1 {
            p.line_segment([Pos2::new(l, cy), Pos2::new(cx, cy)], stroke);
        } else if la == 2 {
            p.line_segment([Pos2::new(l, cy - d), Pos2::new(cx, cy - d)], stroke);
            p.line_segment([Pos2::new(l, cy + d), Pos2::new(cx, cy + d)], stroke);
        }
        if ra == 1 {
            p.line_segment([Pos2::new(cx, cy), Pos2::new(r, cy)], stroke);
        } else if ra == 2 {
            p.line_segment([Pos2::new(cx, cy - d), Pos2::new(r, cy - d)], stroke);
            p.line_segment([Pos2::new(cx, cy + d), Pos2::new(r, cy + d)], stroke);
        }
        if ua == 1 {
            p.line_segment([Pos2::new(cx, t), Pos2::new(cx, cy)], stroke);
        } else if ua == 2 {
            p.line_segment([Pos2::new(cx - d, t), Pos2::new(cx - d, cy)], stroke);
            p.line_segment([Pos2::new(cx + d, t), Pos2::new(cx + d, cy)], stroke);
        }
        if da == 1 {
            p.line_segment([Pos2::new(cx, cy), Pos2::new(cx, b)], stroke);
        } else if da == 2 {
            p.line_segment([Pos2::new(cx - d, cy), Pos2::new(cx - d, b)], stroke);
            p.line_segment([Pos2::new(cx + d, cy), Pos2::new(cx + d, b)], stroke);
        }
        return true;
    }

    // 7. Single & Heavy lines, corners, junctions, and half-arms (`U+2500..=U+254B`, `U+2574..=U+257F`)
    // Arm weights: (left, up, right, down) where 0=none, 1=light (1px), 2=heavy (2px)
    let (lw, uw, rw, dw) = match code {
        0x2500 => (1, 0, 1, 0), // ─
        0x2501 => (2, 0, 2, 0), // ━
        0x2502 => (0, 1, 0, 1), // │
        0x2503 => (0, 2, 0, 2), // ┃
        0x250C => (0, 0, 1, 1), // ┌
        0x250D => (0, 0, 2, 1), // ┍
        0x250E => (0, 0, 1, 2), // ┎
        0x250F => (0, 0, 2, 2), // ┏
        0x2510 => (1, 0, 0, 1), // ┐
        0x2511 => (2, 0, 0, 1), // ┑
        0x2512 => (1, 0, 0, 2), // ┒
        0x2513 => (2, 0, 0, 2), // ┓
        0x2514 => (0, 1, 1, 0), // └
        0x2515 => (0, 1, 2, 0), // ┕
        0x2516 => (0, 2, 1, 0), // ┖
        0x2517 => (0, 2, 2, 0), // ┗
        0x2518 => (1, 1, 0, 0), // ┘
        0x2519 => (2, 1, 0, 0), // ┙
        0x251A => (1, 2, 0, 0), // ┚
        0x251B => (2, 2, 0, 0), // ┛
        0x251C => (0, 1, 1, 1), // ├
        0x251D => (0, 1, 2, 1), // ┝
        0x251E => (0, 2, 1, 1), // ┞
        0x251F => (0, 1, 1, 2), // ┟
        0x2520 => (0, 2, 1, 2), // ┠
        0x2521 => (0, 2, 2, 1), // ┡
        0x2522 => (0, 1, 2, 2), // ┢
        0x2523 => (0, 2, 2, 2), // ┣
        0x2524 => (1, 1, 0, 1), // ┤
        0x2525 => (2, 1, 0, 1), // ┥
        0x2526 => (1, 2, 0, 1), // ┦
        0x2527 => (1, 1, 0, 2), // ┧
        0x2528 => (1, 2, 0, 2), // ┨
        0x2529 => (2, 2, 0, 1), // ┩
        0x252A => (2, 1, 0, 2), // ┪
        0x252B => (2, 2, 0, 2), // ┫
        0x252C => (1, 0, 1, 1), // ┬
        0x252D => (2, 0, 1, 1), // ┭
        0x252E => (1, 0, 2, 1), // ┮
        0x252F => (2, 0, 2, 1), // ┯
        0x2530 => (1, 0, 1, 2), // ┰
        0x2531 => (2, 0, 1, 2), // ┱
        0x2532 => (1, 0, 2, 2), // ┲
        0x2533 => (2, 0, 2, 2), // ┳
        0x2534 => (1, 1, 1, 0), // ┴
        0x2535 => (2, 1, 1, 0), // ┵
        0x2536 => (1, 1, 2, 0), // ┶
        0x2537 => (2, 1, 2, 0), // ┷
        0x2538 => (1, 2, 1, 0), // ┸
        0x2539 => (2, 2, 1, 0), // ┹
        0x253A => (1, 2, 2, 0), // ┺
        0x253B => (2, 2, 2, 0), // ┻
        0x253C => (1, 1, 1, 1), // ┼
        0x253D => (2, 1, 1, 1), // ┽
        0x253E => (1, 1, 2, 1), // ┾
        0x253F => (2, 1, 2, 1), // ┿
        0x2540 => (1, 2, 1, 1), // ╀
        0x2541 => (1, 1, 1, 2), // ╁
        0x2542 => (1, 2, 1, 2), // ╂
        0x2543 => (2, 2, 1, 1), // ╃
        0x2544 => (1, 2, 2, 1), // ╄
        0x2545 => (2, 1, 1, 2), // ╅
        0x2546 => (1, 1, 2, 2), // ╆
        0x2547 => (2, 2, 2, 1), // ╇
        0x2548 => (2, 1, 2, 2), // ╈
        0x2549 => (2, 2, 1, 2), // ╉
        0x254A => (1, 2, 2, 2), // ╊
        0x254B => (2, 2, 2, 2), // ╋
        0x2574 => (1, 0, 0, 0), // ╴
        0x2575 => (0, 1, 0, 0), // ╵
        0x2576 => (0, 0, 1, 0), // ╶
        0x2577 => (0, 0, 0, 1), // ╷
        0x2578 => (2, 0, 0, 0), // ╸
        0x2579 => (0, 2, 0, 0), // ╹
        0x257A => (0, 0, 2, 0), // ╺
        0x257B => (0, 0, 0, 2), // ╻
        0x257C => (1, 0, 2, 0), // ╼
        0x257D => (0, 1, 0, 2), // ╽
        0x257E => (2, 0, 1, 0), // ╾
        0x257F => (0, 2, 0, 1), // ╿
        _ => (1, 0, 1, 0),
    };

    if lw > 0 && rw == lw && uw == 0 && dw == 0 {
        p.line_segment([Pos2::new(l, cy), Pos2::new(r, cy)], Stroke::new(if lw == 2 { 2.0 } else { 1.0 }, fg));
    } else if uw > 0 && dw == uw && lw == 0 && rw == 0 {
        p.line_segment([Pos2::new(cx, t), Pos2::new(cx, b)], Stroke::new(if uw == 2 { 2.0 } else { 1.0 }, fg));
    } else {
        if lw > 0 {
            p.line_segment([Pos2::new(l, cy), Pos2::new(cx, cy)], Stroke::new(if lw == 2 { 2.0 } else { 1.0 }, fg));
        }
        if rw > 0 {
            p.line_segment([Pos2::new(cx, cy), Pos2::new(r, cy)], Stroke::new(if rw == 2 { 2.0 } else { 1.0 }, fg));
        }
        if uw > 0 {
            p.line_segment([Pos2::new(cx, t), Pos2::new(cx, cy)], Stroke::new(if uw == 2 { 2.0 } else { 1.0 }, fg));
        }
        if dw > 0 {
            p.line_segment([Pos2::new(cx, cy), Pos2::new(cx, b)], Stroke::new(if dw == 2 { 2.0 } else { 1.0 }, fg));
        }
    }
    true
}

/// Renders Orca's Dual-Glyph Live Agent Status (`Working` 12fps `#EAB308` spinner, `WaitingForInput`
/// `#F97316` ring, `Done`/`Idle` `#10B981` emerald dot) + Provider Color Badge (`Claude` `#D97757`,
/// `Codex` `#FAFAFA`, `Grok` `#E4E4E7`, `Antigravity` `#60A5FA`, `Shell` `>_`) inside a tab rect.
fn paint_orca_tab_status_and_provider(ui: &Ui, p: &Painter, tab: &TerminalTab, t_rect: Rect) {
    let cy = t_rect.center().y;
    let status_center = Pos2::new(t_rect.left() + 7.5, cy);
    let provider_center = Pos2::new(t_rect.left() + 15.0, cy);

    let explicit_status = tab.agent_status.lock().ok().map(|g| g.clone()).unwrap_or_default();
    let recent_pty_activity = tab.preset != AgentCliPreset::Shell
        && tab.last_output_at.lock().ok().and_then(|g| *g).is_some_and(|ts| ts.elapsed() < std::time::Duration::from_millis(600));

    let effective_status = match explicit_status {
        AgentTabStatus::Working(d) => AgentTabStatus::Working(d),
        AgentTabStatus::WaitingForInput(d) => AgentTabStatus::WaitingForInput(d),
        other if recent_pty_activity => AgentTabStatus::Working(None),
        other => other,
    };

    match effective_status {
        AgentTabStatus::Working(_) => {
            // Orca 12fps stepped rotating `#EAB308` arc spinner + 83ms repaint cadence
            let step = ((ui.input(|i| i.time) * 12.0).floor() as i32).rem_euclid(12);
            let base_angle = (step as f32) * (std::f32::consts::TAU / 12.0);
            let arc_span = std::f32::consts::TAU * 0.68;
            let radius = 3.2f32;
            let spin_col = Color32::from_rgb(0xEA, 0xB3, 0x08);
            let segs = 7usize;
            let mut prev = Pos2::new(status_center.x + radius * base_angle.cos(), status_center.y + radius * base_angle.sin());
            for i in 1..=segs {
                let a = base_angle + arc_span * (i as f32 / segs as f32);
                let next = Pos2::new(status_center.x + radius * a.cos(), status_center.y + radius * a.sin());
                p.line_segment([prev, next], Stroke::new(1.4, spin_col));
                prev = next;
            }
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(83));
        }
        AgentTabStatus::WaitingForInput(_) => {
            let wait_col = Color32::from_rgb(0xF9, 0x73, 0x16);
            p.circle_stroke(status_center, 3.1, Stroke::new(1.3, wait_col));
            p.circle_filled(status_center, 1.2, wait_col);
        }
        AgentTabStatus::Done | AgentTabStatus::Idle => {
            p.circle_filled(status_center, 2.7, Color32::from_rgb(0x10, 0xB9, 0x81));
        }
    }

    // Provider Color Badge / Vector Glyph (`Claude` #D97757, `Codex` #FAFAFA, `Grok` #E4E4E7, `Antigravity` #60A5FA, `Shell` `>_`)
    match tab.preset {
        AgentCliPreset::Claude => {
            let col = Color32::from_rgb(0xD9, 0x77, 0x57);
            p.circle_filled(provider_center, 2.3, col);
        }
        AgentCliPreset::Codex => {
            let col = Color32::from_rgb(0xFA, 0xFA, 0xFA);
            p.circle_stroke(provider_center, 2.2, Stroke::new(1.2, col));
        }
        AgentCliPreset::Grok => {
            let col = Color32::from_rgb(0xE4, 0xE4, 0xE7);
            p.line_segment(
                [Pos2::new(provider_center.x - 2.0, provider_center.y + 2.0), Pos2::new(provider_center.x + 2.0, provider_center.y - 2.0)],
                Stroke::new(1.4, col),
            );
        }
        AgentCliPreset::Antigravity => {
            let col = Color32::from_rgb(0x60, 0xA5, 0xFA);
            p.circle_filled(provider_center, 2.3, col);
        }
        AgentCliPreset::Shell | AgentCliPreset::DesignOsHelp => {
            let col = Color32::from_rgb(0xA1, 0xA1, 0xAA);
            let s = Stroke::new(1.1, col);
            p.line_segment([Pos2::new(provider_center.x - 2.2, cy - 2.0), Pos2::new(provider_center.x - 0.2, cy)], s);
            p.line_segment([Pos2::new(provider_center.x - 0.2, cy), Pos2::new(provider_center.x - 2.2, cy + 2.0)], s);
            p.line_segment([Pos2::new(provider_center.x + 0.6, cy + 2.0), Pos2::new(provider_center.x + 2.6, cy + 2.0)], s);
        }
    }
}

/// Free-function wrapper for [`TerminalState::send_copy_for_ai`].
#[allow(dead_code)]
pub fn send_copy_for_ai(state: &mut TerminalState, block_markdown: &str) {
    state.send_copy_for_ai(block_markdown);
}

fn format_copy_for_ai_payload(block_markdown: &str) -> String {
    let trimmed = block_markdown.trim();
    if trimmed.contains("File:") && trimmed.contains("Yêu cầu của tôi:") {
        trimmed.to_string()
    } else {
        format!(
            "Hãy đọc và xử lý đoạn tài liệu này:\n\
             - File: spec/00-overview.md\n\
             - Mục: Product OS Spec Block\n\
             - Trang reader: #overview\n\
             - Project: design-os-studio\n\n\
             ```markdown\n\
             {trimmed}\n\
             ```\n\n\
             Yêu cầu của tôi: "
        )
    }
}

fn default_shell_program() -> String {
    if cfg!(windows) {
        for candidate in [r"C:\Program Files\Git\bin\bash.exe", r"C:\Program Files\Git\usr\bin\sh.exe"] {
            if Path::new(candidate).is_file() {
                return candidate.to_string();
            }
        }
        std::env::var("COMSPEC").unwrap_or_else(|_| "powershell.exe".into())
    } else {
        std::env::var("SHELL").ok().filter(|s| !s.trim().is_empty()).unwrap_or_else(|| {
            if Path::new("/bin/zsh").exists() {
                "/bin/zsh".into()
            } else {
                "/bin/sh".into()
            }
        })
    }
}

/// Test-only: starts zsh with an empty `ZDOTDIR` instead of the real HOME. `zsh -l -i -c ...` in a PTY
/// otherwise runs zsh-newuser-install when `~/.zshrc` is missing (a CI runner) and the command never runs.
#[cfg(test)]
fn isolate_zsh_from_home(cmd: &mut CommandBuilder) {
    let zdotdir = std::env::temp_dir().join(format!("dos-p0-zdotdir-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&zdotdir);
    let _ = std::fs::write(zdotdir.join(".zshrc"), b"");
    cmd.env("ZDOTDIR", &zdotdir);
}

fn build_agent_shell_cmd(shell_bin: &str, agent_cli: &str) -> CommandBuilder {
    let mut c = CommandBuilder::new(shell_bin);
    if cfg!(windows) && !shell_bin.ends_with("bash.exe") && !shell_bin.ends_with("sh.exe") {
        c.arg("-Command");
        c.arg(agent_cli);
    } else {
        c.arg("-l");
        c.arg("-i");
        c.arg("-c");
        // Single command: `exec` so the agent is the PTY child; when it exits the child exits.
        // Compound commands keep their own fallback, with no `|| exec $SHELL`.
        let inner = if agent_cli.contains("||") { agent_cli.to_string() } else { format!("exec {agent_cli}") };
        c.arg(inner);
    }
    c
}

/// Publishes the session's MCP server config as an `agy` workspace plugin at
/// `<session_dir>/.agents/plugins/designos/` (`plugin.json` + `mcp_config.json`, same bytes as
/// `mcp-config.json`). `agy` 1.4.2 has no per-session MCP flag; launching with `--add-dir <session_dir>`
/// makes it discover this plugin for that session only, leaving the user's global agy config untouched.
fn write_agy_session_plugin(session_dir: &Path, mcp_config_bytes: &[u8]) {
    let plugin_dir = session_dir.join(".agents").join("plugins").join("designos");
    let manifest = br#"{"name":"designos"}"#;
    let _ = atomic_write_session_file(&plugin_dir.join("plugin.json"), manifest);
    let _ = atomic_write_session_file(&plugin_dir.join("mcp_config.json"), mcp_config_bytes);
}

/// Renders `s` as a TOML basic string. JSON string escapes are a valid subset of TOML's.
fn toml_string(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_else(|_| "\"\"".into())
}

/// Renders a TOML key, quoting it unless it is a valid bare key.
fn toml_key(k: &str) -> String {
    if !k.is_empty() && k.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
        k.to_string()
    } else {
        toml_string(k)
    }
}

/// Publishes the session's `mcpServers` as Grok's project config `<session_dir>/.grok/config.toml`.
/// Grok 1.0.62 has no per-session MCP flag; it merges the project layer of its cwd, so the tab is
/// started with `--cwd <session_dir>` and the user's `~/.grok/config.toml` is never written.
fn write_grok_session_config(session_dir: &Path, mcp_config: &serde_json::Value) {
    let Some(servers) = mcp_config.get("mcpServers").and_then(|v| v.as_object()) else { return };
    let mut toml = String::new();
    for (name, server) in servers {
        let key = toml_key(name);
        toml.push_str(&format!("[mcp_servers.{key}]\n"));
        if let Some(command) = server.get("command").and_then(|v| v.as_str()) {
            toml.push_str(&format!("command = {}\n", toml_string(command)));
        }
        if let Some(args) = server.get("args").and_then(|v| v.as_array()) {
            let items: Vec<String> = args.iter().filter_map(|a| a.as_str()).map(toml_string).collect();
            toml.push_str(&format!("args = [{}]\n", items.join(", ")));
        }
        toml.push_str("enabled = true\n");
        if let Some(env) = server.get("env").and_then(|v| v.as_object()) {
            toml.push_str(&format!("\n[mcp_servers.{key}.env]\n"));
            for (k, v) in env {
                if let Some(v) = v.as_str() {
                    toml.push_str(&format!("{} = {}\n", toml_key(k), toml_string(v)));
                }
            }
        }
        toml.push('\n');
    }
    let _ = atomic_write_session_file(&session_dir.join(".grok").join("config.toml"), toml.as_bytes());
}

fn atomic_write_session_file(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let tmp = path.with_extension(format!("tmp.{}", std::process::id()));
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)
}

/// Shell command that records `state` for the Agents panel (`agent-status.json`, read by `poll_agent_status`).
fn status_hook_command(status_path: &Path, state: &str) -> String {
    format!("printf '{{\"state\":\"{state}\"}}\\n' > '{}'", status_path.display())
}

fn command_handler_group(command: String) -> serde_json::Value {
    serde_json::json!([{ "matcher": "*", "hooks": [{ "type": "command", "command": command }] }])
}

/// Session hooks in Claude's settings shape (`{"hooks": {Event: [{matcher, hooks: [...]}]}}`) for the
/// status channel. With `context_reply` the prompt hook also prints that file, a Claude
/// `additionalContext` reply carrying the page (never the selection). Without it the prompt hook prints
/// nothing: Grok reads any stdout that starts with `{` as a decision (a context reply shows as a
/// failed hook) and discards an allowing hook's context anyway.
fn session_hooks_settings(status_path: &Path, context_reply: Option<&Path>) -> serde_json::Value {
    let prompt = match context_reply {
        Some(reply) => format!("{} && cat '{}'", status_hook_command(status_path, "working"), reply.display()),
        None => status_hook_command(status_path, "working"),
    };
    serde_json::json!({
        "hooks": {
            "UserPromptSubmit": command_handler_group(prompt),
            "PreToolUse": command_handler_group(status_hook_command(status_path, "working")),
            "PostToolUse": command_handler_group(status_hook_command(status_path, "working")),
            "PermissionRequest": command_handler_group(status_hook_command(status_path, "waiting")),
            "Stop": command_handler_group(status_hook_command(status_path, "done")),
        }
    })
}

/// Antigravity's `<workspace>/.agents/hooks.json`: named hook, flat handlers for the invocation and
/// stop events. Handlers must answer with a JSON object on stdout.
fn agy_session_hooks(status_path: &Path) -> serde_json::Value {
    let handler = |state: &str| serde_json::json!([{ "type": "command", "command": format!("{}; printf '{{}}'", status_hook_command(status_path, state)) }]);
    serde_json::json!({
        "designos-status": {
            "PreInvocation": handler("working"),
            "Stop": handler("done"),
        }
    })
}

/// `designos-mcp` next to the running Studio binary, or the bare name (resolved through `PATH`) when it is not built there.
fn designos_mcp_binary() -> PathBuf {
    std::env::current_exe()
        .ok()
        .map(|exe| exe.with_file_name(if cfg!(windows) { "designos-mcp.exe" } else { "designos-mcp" }))
        .filter(|p| p.exists())
        .unwrap_or_else(|| PathBuf::from("designos-mcp"))
}

/// Folder `designos-mcp --workspace` serves: the open file's folder, else Studio's working directory.
fn session_workspace_dir(file_path: Option<&Path>) -> PathBuf {
    file_path
        .and_then(|p| p.parent())
        .filter(|p| !p.as_os_str().is_empty())
        .map(|p| p.to_path_buf())
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("."))
}

/// Working directory of a spawned tab, never `/` (an app opened from Finder has no useful cwd).
/// Agent CLIs start in the per-file session dir because that is where they discover the project
/// config, skills and `AGENTS.md` Studio provisions; a shell tab opens in the file's folder.
fn resolve_pty_cwd(preset: AgentCliPreset, session_dir: Option<&Path>, workspace: PathBuf) -> PathBuf {
    let is_agent = !matches!(preset, AgentCliPreset::Shell | AgentCliPreset::DesignOsHelp);
    match session_dir {
        Some(dir) if is_agent => dir.to_path_buf(),
        _ if workspace.parent().is_some() => workspace,
        Some(dir) => dir.to_path_buf(),
        None => std::env::temp_dir(),
    }
}

/// Auto-injects `TERM`, `COLORTERM`, `DESIGNOS_AGENT_RUNTIME` (the spawned `preset`), `DESIGNOS_BIN`, `DESIGNOS_MCP_BIN`, `DESIGNOS_LIVE_SPEC`,
/// `DESIGNOS_SELECTION_JSON`, `DESIGNOS_FILE`, `DESIGNOS_WORKSPACE`, and prepends common CLI directories
/// (`~/.cargo/bin`, `/opt/homebrew/bin`, `/usr/local/bin`, `~/.local/bin`, `~/.antigravity/bin`) to `PATH`.
pub fn configure_pty_env(cmd: &mut CommandBuilder, preset: AgentCliPreset, file_path: Option<&Path>) {
    cmd.env("DESIGNOS_AGENT_RUNTIME", preset.binary_name());
    configure_pty_env_with_session(cmd, preset, file_path, None);
}

/// Auto-injects all `configure_pty_env` variables plus per-file session isolation variables (`DESIGNOS_SESSION_DIR`,
/// `DESIGNOS_MCP_CONFIG`, `DESIGNOS_LIVE_SPEC`, `DESIGNOS_LIVE_DOC_SYNC`, `DESIGNOS_LIVE_CANVAS`, `DESIGNOS_CONTEXT_JSON`,
/// `DESIGNOS_SELECTION_JSON`, `DESIGNOS_TARGET_PAGE_ID`, `DESIGNOS_TARGET_PAGE_NAME`, `DESIGNOS_DOC_NAME`).
pub fn configure_pty_env_with_session(cmd: &mut CommandBuilder, preset: AgentCliPreset, file_path: Option<&Path>, session: Option<&SessionEnv>) {
    cmd.env("TERM", "xterm-256color");
    cmd.env("COLORTERM", "truecolor");
    cmd.env("DESIGNOS_AGENT_RUNTIME", preset.binary_name());
    if preset == AgentCliPreset::Grok {
        // Grok starts repo-local MCP servers, hooks and skills only in a trusted folder. The tab's
        // cwd is Studio's own session dir, so skip the prompt for this process instead of writing
        // `~/.grok/trusted_folders.toml`.
        cmd.env("GROK_FOLDER_TRUST", "0");
    }

    if let Some(sess) = session {
        cmd.env("DESIGNOS_SESSION_DIR", sess.session_dir.as_os_str());
        cmd.env("DESIGNOS_MCP_CONFIG", sess.mcp_config_path.as_os_str());
        cmd.env("DESIGNOS_LIVE_SPEC", sess.live_spec_path.as_os_str());
        cmd.env("DESIGNOS_LIVE_DOC_SYNC", sess.live_doc_sync_path.as_os_str());
        cmd.env("DESIGNOS_LIVE_DOC_PATCH", sess.session_dir.join("doc-patch.jsonl").as_os_str());
        cmd.env("DESIGNOS_LIVE_CANVAS", sess.live_canvas_path.as_os_str());
        cmd.env("DESIGNOS_CONTEXT_JSON", sess.context_json_path.as_os_str());
        cmd.env("DESIGNOS_SELECTION_JSON", sess.selection_spec_path.as_os_str());
        cmd.env("DESIGNOS_TARGET_PAGE_ID", sess.current_page_id.to_string());
        cmd.env("DESIGNOS_TARGET_PAGE_NAME", &sess.current_page_name);
        cmd.env("DESIGNOS_DOC_NAME", &sess.doc_name);
    } else {
        cmd.env("DESIGNOS_LIVE_SPEC", "/tmp/designos-live-agent-out.spec.json");
        cmd.env("DESIGNOS_SELECTION_JSON", "/tmp/designos-live-selection.spec.json");
    }

    if let Ok(exe) = std::env::current_exe() {
        cmd.env("DESIGNOS_BIN", exe.as_os_str());
    }
    cmd.env("DESIGNOS_MCP_BIN", designos_mcp_binary().as_os_str());
    if let Some(fp) = file_path {
        cmd.env("DESIGNOS_FILE", fp.as_os_str());
    }
    cmd.env("DESIGNOS_WORKSPACE", session_workspace_dir(file_path).as_os_str());
    cmd.cwd(resolve_pty_cwd(preset, session.map(|sess| sess.session_dir.as_path()), session_workspace_dir(file_path)));

    let home = std::env::var("HOME").unwrap_or_default();
    let mut extra_paths: Vec<String> = Vec::new();
    if !home.is_empty() {
        extra_paths.push(format!("{home}/.cargo/bin"));
        extra_paths.push(format!("{home}/.local/bin"));
        extra_paths.push(format!("{home}/.antigravity/bin"));
    }
    if cfg!(windows) {
        extra_paths.push(r"C:\Program Files\Git\usr\bin".into());
        extra_paths.push(r"C:\Program Files\Git\bin".into());
    } else {
        extra_paths.push("/opt/homebrew/bin".into());
        extra_paths.push("/usr/local/bin".into());
    }

    let current_path = std::env::var("PATH").unwrap_or_default();
    let sep = if cfg!(windows) { ";" } else { ":" };
    let combined = if current_path.is_empty() { extra_paths.join(sep) } else { format!("{}{sep}{current_path}", extra_paths.join(sep)) };
    cmd.env("PATH", combined);
}

/// Scans terminal output for a ```json ... ``` fence or JSON object representing a `NodeSpec`.
/// Returns the latest valid `NodeSpec` JSON string if found.
pub fn extract_spec_json_from_log(log: &str) -> Option<String> {
    let mut search_end = log.len();
    while let Some(fence_start) = log[..search_end].rfind("```json") {
        let after_tag = &log[fence_start + 7..search_end];
        if let Some(fence_end) = after_tag.find("```") {
            let candidate = after_tag[..fence_end].trim();
            if is_valid_node_spec_json(candidate) {
                return Some(candidate.to_string());
            }
        }
        search_end = fence_start;
    }

    let bytes = log.as_bytes();
    let mut i = bytes.len();
    let mut checked = 0usize;
    while i > 0 && checked < 12 {
        i -= 1;
        if bytes[i] == b'}' {
            if let Some(open_idx) = find_matching_open_brace(&log[..=i]) {
                let candidate = log[open_idx..=i].trim();
                if (candidate.contains("\"name\"") || candidate.contains("\"preset\"")) && is_valid_node_spec_json(candidate) {
                    return Some(candidate.to_string());
                }
                checked += 1;
            }
        }
    }
    None
}

fn find_matching_open_brace(slice_ending_with_close: &str) -> Option<usize> {
    let mut depth = 0i32;
    let mut in_str = false;
    let mut prev_backslash = false;
    let chars: Vec<(usize, char)> = slice_ending_with_close.char_indices().collect();
    for &(idx, ch) in chars.iter().rev() {
        if in_str {
            if ch == '"' && !prev_backslash {
                in_str = false;
            }
            prev_backslash = ch == '\\';
            continue;
        }
        match ch {
            '"' => in_str = true,
            '}' => depth += 1,
            '{' => {
                depth -= 1;
                if depth == 0 {
                    return Some(idx);
                }
            }
            _ => {}
        }
    }
    None
}

fn is_valid_node_spec_json(candidate: &str) -> bool {
    if !candidate.starts_with('{') || !candidate.ends_with('}') {
        return false;
    }
    if let Ok(spec) = serde_json::from_str::<NodeSpec>(candidate) {
        !spec.name.trim().is_empty() || spec.preset.is_some() || !spec.children.is_empty() || !spec.kind.trim().is_empty()
    } else {
        false
    }
}

/// Serializes the active selection (or first top-level frame on `page`) to
/// the session's `selection.spec.json` (plus `/tmp/designos-live-selection.spec.json`) with a
/// `designos_context` object (node name, id, open pin-comments). Pastes only a fixed
/// file-reference sentence into a live agent tab (never node name, id, or comment text).
pub fn export_selection_and_comments_to_pty(
    state: &mut TerminalState,
    doc: &Document,
    page: NodeId,
    selection: &[NodeId],
    _file_path: Option<&Path>,
) -> Result<PathBuf, String> {
    let target_id = selection
        .first()
        .copied()
        .or_else(|| doc.get(page).and_then(|pg| pg.children.first().copied()))
        .ok_or_else(|| "No frame or node on current page to export".to_string())?;

    let spec = doc.export_spec(target_id)?;
    let unresolved_comments: Vec<String> = doc
        .resolve_comment_targets()
        .into_iter()
        .filter(|c| !c.resolved)
        .map(|c| {
            let msg = c.messages.first().map(|m| m.text.as_str()).unwrap_or("(empty comment)");
            let target = c.target_node_name.unwrap_or_else(|| "Canvas".to_string());
            format!("[Pin on {target}]: {msg}")
        })
        .collect();
    let node_name = doc.get(target_id).map(|n| n.name.clone()).unwrap_or_else(|| "Selection".into());

    let mut json_value = serde_json::to_value(&spec).map_err(|e| e.to_string())?;
    if let Some(obj) = json_value.as_object_mut() {
        obj.insert(
            "designos_context".to_string(),
            serde_json::json!({
                "node_name": node_name,
                "node_id": target_id.0,
                "open_comments": unresolved_comments,
            }),
        );
    }
    let json = serde_json::to_string_pretty(&json_value).map_err(|e| e.to_string())?;
    let _ = atomic_write_session_file(&state.selection_spec_path, json.as_bytes());
    let tmp_path = std::env::temp_dir().join("designos-live-selection.spec.json");
    std::fs::write(&tmp_path, &json).map_err(|e| format!("write {}: {e}", tmp_path.display()))?;

    let cmd_snippet =
        format!("Read the design:os selection at {} (node spec plus open pin-comments under \"designos_context\").", state.selection_spec_path.display());

    let pasted = state.paste_into_agent_tab(&cmd_snippet);
    if pasted {
        state.status_msg = Some(format!("Sent '{}' (#{}) + comments to {}", node_name, target_id.0, state.selection_spec_path.display()));
    } else {
        state.status_msg = Some(format!("Selection exported to {} (no agent tab open — nothing sent to the terminal)", state.selection_spec_path.display()));
    }
    Ok(state.selection_spec_path.clone())
}

/// Renders the 1:1 Figma Desktop `126.9.13` `Agents` Left Side Panel (`New chat / Shared` header,
/// dismissible AI credits banner, CLI Runtime Selector Bar (`Claude` | `Codex` | `Antigravity` | `Shell`),
/// pure Chat UI wrapper over the live PTY shell (`DIR-21`), `What's next?` empty state, bottom
/// `"Ask for changes"` blue-bordered composer box, and toggleable in-panel live VT100 PTY viewport).
pub fn show_left(state: &mut TerminalState, ui: &mut Ui, rect: Rect, file_path: Option<&Path>) -> Vec<TerminalAction> {
    state.ensure_default_tab(file_path);
    state.sync_chat_from_pty();
    let mut actions = state.poll_live_agent_bridge();
    let p = ui.painter().clone();

    if state.side_panel_pty_open {
        // Pure Interactive PTY Shell by Default + Top `+` Runtime Dropdown (`DIR-29`)
        p.rect_filled(rect, 0.0, TERM_BG_VIEWPORT);

        if ui.input(|i| i.pointer.any_pressed()) {
            if let Some(pos) = ui.input(|i| i.pointer.interact_pos()) {
                if rect.contains(pos) {
                    state.viewport_focused = true;
                }
            }
        }

        let bar_h = 36.0;
        let bar_r = Rect::from_min_size(rect.min, Vec2::new(rect.width(), bar_h));
        p.rect_filled(bar_r, 0.0, TERM_BG_BAR);
        widgets::hline(&p, bar_r.left(), bar_r.right(), bar_r.bottom() - 1.0, TERM_BORDER);

        // Right-aligned Dock/Expand toggle button (`agents-header-popout`)
        let pop_rect = Rect::from_center_size(Pos2::new(bar_r.right() - 18.0, bar_r.center().y), Vec2::splat(24.0));
        let pop_resp = ui.interact(pop_rect, Id::new("term-left-dock-toggle"), Sense::click());
        if pop_resp.hovered() || state.dock_open {
            p.rect_filled(pop_rect, CornerRadius::same(4), TERM_BG_HOVER);
        }
        paint_popout_icon(&p, pop_rect.center(), Color32::from_rgb(0xD4, 0xD4, 0xD4));
        widgets::record(ui, "term-left-dock-toggle", pop_rect);
        widgets::record(ui, "agents-popout-pty", pop_rect);
        widgets::record(ui, "agents-header-popout", pop_rect);
        if pop_resp.clicked() {
            state.dock_open = !state.dock_open;
        }

        // Left-aligned session tabs + `+` runtime dropdown button
        let max_tabs_right = pop_rect.left() - 38.0;
        let mut x = bar_r.left() + 6.0;
        let mut tab_to_select: Option<usize> = None;
        let mut tab_to_close: Option<usize> = None;
        let can_close = state.tabs.len() > 1;

        for (idx, tab) in state.tabs.iter().enumerate() {
            let tab_w = if state.tabs.len() <= 1 {
                104.0f32.min((max_tabs_right - x).max(64.0))
            } else {
                82.0f32.min(((max_tabs_right - x) / (state.tabs.len() as f32)).max(56.0))
            };
            if x + 48.0 > max_tabs_right {
                break;
            }
            let is_active = idx == state.active_tab;
            let t_rect = Rect::from_min_size(Pos2::new(x, bar_r.center().y - 12.0), Vec2::new(tab_w, 24.0));
            let close_rect = Rect::from_min_size(Pos2::new(t_rect.right() - 16.0, t_rect.center().y - 6.0), Vec2::splat(12.0));
            let t_hit_rect =
                if can_close { Rect::from_min_max(t_rect.min, Pos2::new((close_rect.left() - 2.0).max(t_rect.left() + 16.0), t_rect.max.y)) } else { t_rect };
            let tab_resp = ui.interact(t_hit_rect, Id::new(("agents-left-tab-select", tab.id)), Sense::click());
            let close_resp = if can_close { Some(ui.interact(close_rect, Id::new(("agents-left-tab-close", tab.id)), Sense::click())) } else { None };

            let bg = if is_active {
                TERM_BG_ELEVATED
            } else if tab_resp.hovered() {
                TERM_BG_HOVER
            } else {
                TERM_BG_BAR
            };
            p.rect_filled(t_rect, theme::cr(theme::RADIUS_CONTROL), bg);
            if is_active {
                p.rect_stroke(t_rect, theme::cr(theme::RADIUS_CONTROL), Stroke::new(1.0, TERM_BORDER), StrokeKind::Inside);
                p.rect_filled(
                    Rect::from_min_max(Pos2::new(t_rect.left() + 4.0, t_rect.bottom() - 2.0), Pos2::new(t_rect.right() - 4.0, t_rect.bottom())),
                    CornerRadius::same(1),
                    TERM_ORANGE,
                );
            }
            paint_orca_tab_status_and_provider(ui, &p, tab, t_rect);
            let text_pad_r = if can_close { 34.0 } else { 23.0 };
            let title_str = widgets::elide(&p, &tab.title, &theme::font_caption_medium(), (tab_w - text_pad_r).max(16.0));
            p.text(
                Pos2::new(t_rect.left() + 21.0, t_rect.center().y),
                Align2::LEFT_CENTER,
                title_str,
                theme::font_caption_medium(),
                if is_active { theme::TEXT } else { theme::TEXT_SECONDARY },
            );
            if let Some(cr) = &close_resp {
                icons::paint(&p, Icon::Close, close_rect.center(), 9.0, if cr.hovered() { STATUS_BAD_TEXT } else { theme::ICON_TERTIARY });
                if cr.clicked() {
                    tab_to_close = Some(idx);
                }
            }
            if tab_to_close.is_none() && tab_resp.clicked() {
                tab_to_select = Some(idx);
            }
            x = t_rect.right() + 4.0;
        }

        if let Some(idx) = tab_to_select {
            state.active_tab = idx;
            state.viewport_focused = true;
            state.runtime_dropdown_open = false;
        }
        if let Some(idx) = tab_to_close {
            state.close_tab(idx);
            state.ensure_default_tab(file_path);
        }

        // Top `+` button (`agents-runtime-add-btn`) to open the runtime selector dropdown (`claude`, `codex`, `grok`, `antigravity`, `shell`)
        let add_x = x.min(pop_rect.left() - 32.0);
        let add_rect = Rect::from_min_size(Pos2::new(add_x, bar_r.center().y - 12.0), Vec2::new(28.0, 24.0));
        let add_resp = ui.interact(add_rect, Id::new("agents-runtime-add-btn"), Sense::click());
        let add_bg = if state.runtime_dropdown_open {
            STATUS_INFO_BG
        } else if add_resp.hovered() {
            TERM_BG_HOVER
        } else {
            TERM_BG_ELEVATED
        };
        let add_border = if state.runtime_dropdown_open { STATUS_INFO_BORDER } else { TERM_BORDER };
        let add_fg = if state.runtime_dropdown_open { STATUS_INFO_TEXT } else { Color32::WHITE };
        p.rect_filled(add_rect, theme::cr(theme::RADIUS_CONTROL), add_bg);
        p.rect_stroke(add_rect, theme::cr(theme::RADIUS_CONTROL), Stroke::new(1.0, add_border), StrokeKind::Inside);
        icons::paint(&p, Icon::Plus, Pos2::new(add_rect.left() + 10.0, add_rect.center().y), 11.0, add_fg);
        icons::paint(&p, Icon::ChevronDown, Pos2::new(add_rect.right() - 7.0, add_rect.center().y), 8.0, add_fg);
        widgets::record(ui, "agents-runtime-add-btn", add_rect);
        if add_resp.clicked() {
            state.runtime_dropdown_open = !state.runtime_dropdown_open;
        }

        // Active File + Page Context Strip (20px) so the user & agent always see the bound Page
        let ctx_h = 20.0;
        let ctx_r = Rect::from_min_size(Pos2::new(rect.left(), bar_r.bottom()), Vec2::new(rect.width(), ctx_h));
        p.rect_filled(ctx_r, 0.0, TERM_BG_ELEVATED);
        widgets::hline(&p, ctx_r.left(), ctx_r.right(), ctx_r.bottom() - 1.0, TERM_BORDER);
        let ctx_badge = format!("{} · {}", state.doc_name, state.current_page_name);
        let ctx_elided = widgets::elide(&p, &ctx_badge, &theme::font_caption(), (ctx_r.width() - 24.0).max(40.0));
        p.circle_filled(Pos2::new(ctx_r.left() + 10.0, ctx_r.center().y), 2.5, STATUS_INFO_TEXT);
        p.text(Pos2::new(ctx_r.left() + 17.0, ctx_r.center().y), Align2::LEFT_CENTER, ctx_elided, theme::font_caption(), STATUS_INFO_TEXT);

        // Live VT100 PTY Terminal Viewport (full height)
        let vp_r = Rect::from_min_max(Pos2::new(rect.left(), ctx_r.bottom()), rect.max);

        render_vt100_viewport(state, ui, vp_r, "agents-left-pty");
        if let Some(fix) = state.pending_start_fix.take() {
            state.apply_start_fix(fix, file_path);
        }

        // Floating `+` Runtime Selector Dropdown (`claude`, `codex`, `grok`, `antigravity`, `shell` — all skipping dangerous permissions)
        if state.runtime_dropdown_open {
            render_runtime_dropdown_menu(state, ui, rect, bar_r.bottom() + 4.0, file_path);
        }

        let repaint_ms = if state.viewport_focused { 50 } else { 500 };
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(repaint_ms));
        return actions;
    }

    let panel_bg = Color32::from_rgb(0x2C, 0x2C, 0x2C);
    p.rect_filled(rect, 0.0, panel_bg);

    // ---- A. Top Header Bar (48px, `#2C2C2C` fill, `1px` `#383838` bottom hairline)
    let header_h = 48.0;
    let header_rect = Rect::from_min_size(rect.min, Vec2::new(rect.width(), header_h));
    p.rect_filled(header_rect, 0.0, panel_bg);
    widgets::hline(&p, header_rect.left(), header_rect.right(), header_rect.bottom() - 1.0, Color32::from_rgb(0x38, 0x38, 0x38));

    let back_r = Rect::from_center_size(Pos2::new(header_rect.left() + 18.0, header_rect.top() + 18.0), Vec2::splat(20.0));
    let back_resp = ui.interact(back_r, Id::new("agents-header-back"), Sense::click());
    if back_resp.hovered() {
        p.rect_filled(back_r, CornerRadius::same(4), TERM_BG_HOVER);
    }
    let bc = back_r.center();
    let chev_stroke = Stroke::new(1.5, Color32::from_rgb(0xB3, 0xB3, 0xB3));
    p.line_segment([Pos2::new(bc.x + 2.0, bc.y - 4.5), Pos2::new(bc.x - 2.5, bc.y)], chev_stroke);
    p.line_segment([Pos2::new(bc.x - 2.5, bc.y), Pos2::new(bc.x + 2.0, bc.y + 4.5)], chev_stroke);
    widgets::record(ui, "agents-header-back", back_r);
    if back_resp.clicked() {
        state.side_panel_pty_open = true;
        state.chat_mode = false;
        state.expanded_agent_cat = None;
        state.chat_turns.clear();
        state.chat_scroll = 0.0;
    }

    p.text(Pos2::new(header_rect.left() + 34.0, header_rect.top() + 17.0), Align2::LEFT_CENTER, "New chat", theme::font_body_strong(), Color32::WHITE);
    p.text(
        Pos2::new(header_rect.left() + 34.0, header_rect.top() + 33.0),
        Align2::LEFT_CENTER,
        "Shared",
        theme::font_caption(),
        Color32::from_rgb(0x8C, 0x8C, 0x8C),
    );

    // Header right controls: `AI` badge + Pop-out/PTY toggle + `•••` More menu
    let h_cy = header_rect.top() + 18.0;
    let ai_rect = Rect::from_center_size(Pos2::new(header_rect.right() - 78.0, h_cy), Vec2::new(24.0, 18.0));
    let ai_resp = ui.interact(ai_rect, Id::new("agents-header-ai"), Sense::click());
    let ai_bg = if !state.chat_mode || state.side_panel_pty_open {
        Color32::from_rgb(0x16, 0x35, 0x52)
    } else if ai_resp.hovered() {
        TERM_BG_HOVER
    } else {
        Color32::from_rgb(0x38, 0x38, 0x38)
    };
    p.rect_filled(ai_rect, CornerRadius::same(4), ai_bg);
    p.rect_stroke(ai_rect, CornerRadius::same(4), Stroke::new(1.0, Color32::from_rgb(0x4D, 0x4D, 0x4D)), StrokeKind::Inside);
    p.text(ai_rect.center(), Align2::CENTER_CENTER, "AI", theme::font_caption_strong(), Color32::from_rgb(0xD4, 0xD4, 0xD4));
    widgets::record(ui, "agents-ai-badge", ai_rect);
    widgets::record(ui, "agents-header-ai", ai_rect);
    if ai_resp.clicked() {
        state.chat_mode = !state.chat_mode;
        state.side_panel_pty_open = !state.chat_mode;
    }

    let pop_rect = Rect::from_center_size(Pos2::new(header_rect.right() - 46.0, h_cy), Vec2::splat(24.0));
    let pop_resp = ui.interact(pop_rect, Id::new("term-left-dock-toggle"), Sense::click());
    if pop_resp.hovered() || state.side_panel_pty_open || state.dock_open {
        p.rect_filled(pop_rect, CornerRadius::same(4), TERM_BG_HOVER);
    }
    paint_popout_icon(&p, pop_rect.center(), Color32::from_rgb(0xD4, 0xD4, 0xD4));
    widgets::record(ui, "term-left-dock-toggle", pop_rect);
    widgets::record(ui, "agents-popout-pty", pop_rect);
    widgets::record(ui, "agents-header-popout", pop_rect);
    if pop_resp.clicked() {
        state.dock_open = !state.dock_open;
        state.side_panel_pty_open = state.dock_open;
    }

    let more_rect = Rect::from_center_size(Pos2::new(header_rect.right() - 18.0, h_cy), Vec2::splat(24.0));
    let more_resp = ui.interact(more_rect, Id::new("agents-more-menu"), Sense::click());
    if more_resp.hovered() {
        p.rect_filled(more_rect, CornerRadius::same(4), TERM_BG_HOVER);
    }
    for dx in [-4.5f32, 0.0, 4.5] {
        p.circle_filled(Pos2::new(more_rect.center().x + dx, more_rect.center().y), 1.4, Color32::from_rgb(0xD4, 0xD4, 0xD4));
    }
    widgets::record(ui, "agents-more-menu", more_rect);
    widgets::record(ui, "agents-header-more", more_rect);
    if more_resp.clicked() {
        state.chat_turns.clear();
        state.chat_scroll = 0.0;
        state.expanded_agent_cat = None;
    }

    let mut top_y = header_rect.bottom();

    // ---- B2. CLI Runtime Selector Bar (`28px`, `Claude` | `Codex` | `AGY` | `Grok` + `Shell` toggle)
    let rt_bar_h = 28.0;
    let rt_bar_r = Rect::from_min_size(Pos2::new(rect.left(), top_y), Vec2::new(rect.width(), rt_bar_h));
    p.rect_filled(rt_bar_r, 0.0, Color32::from_rgb(0x22, 0x22, 0x22));
    widgets::hline(&p, rt_bar_r.left(), rt_bar_r.right(), rt_bar_r.bottom() - 1.0, Color32::from_rgb(0x38, 0x38, 0x38));

    let rt_pills: [(&str, &str, AgentCliPreset, f32, f32); 4] = [
        ("agents-runtime-claude", "Claude", AgentCliPreset::Claude, 4.0, 46.0),
        ("agents-runtime-codex", "Codex", AgentCliPreset::Codex, 53.0, 44.0),
        ("agents-runtime-agy", "AGY", AgentCliPreset::Antigravity, 100.0, 38.0),
        ("agents-runtime-grok", "Grok", AgentCliPreset::Grok, 141.0, 40.0),
    ];
    for (pill_id, pill_label, preset, dx, pw) in rt_pills {
        let p_rect = Rect::from_min_size(Pos2::new(rt_bar_r.left() + dx, rt_bar_r.top() + 4.0), Vec2::new(pw, 20.0));
        let p_resp = ui.interact(p_rect, Id::new(pill_id), Sense::click());
        let is_sel = !state.side_panel_pty_open && state.active_runtime == preset;
        let bg = if is_sel {
            STATUS_INFO_BG
        } else if p_resp.hovered() {
            TERM_BG_HOVER
        } else {
            Color32::from_rgb(0x2C, 0x2C, 0x2C)
        };
        let border = if is_sel { STATUS_INFO_BORDER } else { Color32::from_rgb(0x38, 0x38, 0x38) };
        let fg = if is_sel { STATUS_INFO_TEXT } else { Color32::from_rgb(0xD4, 0xD4, 0xD4) };
        p.rect_filled(p_rect, CornerRadius::same(4), bg);
        p.rect_stroke(p_rect, CornerRadius::same(4), Stroke::new(1.0, border), StrokeKind::Inside);
        p.text(p_rect.center(), Align2::CENTER_CENTER, pill_label, theme::font_caption_strong(), fg);
        widgets::record(ui, pill_id, p_rect);
        if p_resp.clicked() {
            state.set_active_runtime(preset, file_path);
            state.side_panel_pty_open = false;
        }
    }

    let pty_toggle_r = Rect::from_min_size(Pos2::new((rt_bar_r.right() - 52.0).max(rt_bar_r.left() + 184.0), rt_bar_r.top() + 4.0), Vec2::new(46.0, 20.0));
    let pty_toggle_resp = ui.interact(pty_toggle_r, Id::new("agents-runtime-pty-toggle"), Sense::click());
    let pty_sel = state.side_panel_pty_open;
    let pty_bg = if pty_sel {
        STATUS_OK_BG
    } else if pty_toggle_resp.hovered() {
        TERM_BG_HOVER
    } else {
        Color32::from_rgb(0x2C, 0x2C, 0x2C)
    };
    let pty_border = if pty_sel { STATUS_OK_BORDER } else { Color32::from_rgb(0x38, 0x38, 0x38) };
    let pty_fg = if pty_sel { STATUS_OK_TEXT } else { Color32::from_rgb(0xB3, 0xB3, 0xB3) };
    p.rect_filled(pty_toggle_r, CornerRadius::same(4), pty_bg);
    p.rect_stroke(pty_toggle_r, CornerRadius::same(4), Stroke::new(1.0, pty_border), StrokeKind::Inside);
    p.text(pty_toggle_r.center(), Align2::CENTER_CENTER, "Shell", theme::font_caption_strong(), pty_fg);
    widgets::record(ui, "agents-runtime-pty-toggle", pty_toggle_r);
    if pty_toggle_resp.clicked() {
        state.side_panel_pty_open = !state.side_panel_pty_open;
    }
    top_y = rt_bar_r.bottom();

    // ---- D. Bottom Docked `"Ask for changes"` Composer Box (`96px` height, `12px` margin)
    let comp_margin = 12.0;
    let comp_h = 96.0;
    let comp_top = (rect.bottom() - comp_margin - comp_h).max(top_y + 80.0);
    let composer_rect =
        Rect::from_min_max(Pos2::new(rect.left() + comp_margin, comp_top), Pos2::new(rect.right() - comp_margin, (comp_top + comp_h).min(rect.bottom() - 4.0)));

    // ---- C. Center Area (`between runtime bar and bottom composer`)
    let center_rect = Rect::from_min_max(Pos2::new(rect.left(), top_y), Pos2::new(rect.right(), (composer_rect.top() - 8.0).max(top_y + 32.0)));

    if !state.chat_turns.is_empty() {
        // Mode 3: Pure Chat UI Wrapper Over Live PTY Shell (`Claude Code`, `Codex`, `Antigravity CLI`)
        render_agent_chat_thread(state, ui, center_rect, &mut actions);
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(50));
    } else {
        // Mode 1 (Default): 1:1 Figma Desktop 126.9.13 `What's next?` View
        clipped(ui, center_rect, |ui| {
            let p = ui.painter().clone();
            let badge_center = Pos2::new(center_rect.center().x, center_rect.top() + 44.0);
            paint_agents_grid_plus_badge(&p, badge_center);

            p.text(
                Pos2::new(center_rect.center().x, center_rect.top() + 82.0),
                Align2::CENTER_CENTER,
                "What's next?",
                theme::font_large_strong(),
                Color32::WHITE,
            );

            let mut y = center_rect.top() + 108.0;
            let card_x = center_rect.left() + 14.0;
            let card_w = (center_rect.width() - 28.0).max(100.0);
            let card_h = 40.0;

            for (cat_idx, cat_id, cat_title) in [
                (0usize, "agents-cat-tidy", "Tidy and polish"),
                (1usize, "agents-cat-generate", "Generate designs"),
                (2usize, "agents-cat-plugins", "Create plugins and shaders"),
                (3usize, "agents-cat-prd", "PRD & Spec Driven v2"),
            ] {
                let card_r = Rect::from_min_size(Pos2::new(card_x, y), Vec2::new(card_w, card_h));
                let card_resp = ui.interact(card_r, Id::new(cat_id), Sense::click());
                let is_expanded = state.expanded_agent_cat == Some(cat_idx);
                let fill = if card_resp.hovered() || is_expanded { Color32::from_rgb(0x2E, 0x2E, 0x2E) } else { Color32::from_rgb(0x25, 0x25, 0x25) };
                p.rect_filled(card_r, CornerRadius::same(10), fill);
                p.rect_stroke(card_r, CornerRadius::same(10), Stroke::new(1.0, Color32::from_rgb(0x38, 0x38, 0x38)), StrokeKind::Inside);

                let icon_pos = Pos2::new(card_r.left() + 18.0, card_r.center().y);
                let icon_col = Color32::from_rgb(0xB3, 0xB3, 0xB3);
                match cat_idx {
                    0 => paint_wand_sparkle_icon(&p, icon_pos, icon_col),
                    1 => paint_four_point_star_icon(&p, icon_pos, icon_col),
                    3 => paint_open_book_icon(&p, icon_pos, icon_col),
                    _ => paint_toolbox_icon(&p, icon_pos, icon_col),
                }

                let label = widgets::elide(&p, cat_title, &theme::font_body_strong(), (card_w - 58.0).max(32.0));
                p.text(Pos2::new(card_r.left() + 34.0, card_r.center().y), Align2::LEFT_CENTER, label, theme::font_body_strong(), Color32::WHITE);

                let ch_c = Pos2::new(card_r.right() - 16.0, card_r.center().y);
                let ch_stroke = Stroke::new(1.4, icon_col);
                if is_expanded {
                    p.line_segment([Pos2::new(ch_c.x - 4.0, ch_c.y + 2.0), Pos2::new(ch_c.x, ch_c.y - 2.0)], ch_stroke);
                    p.line_segment([Pos2::new(ch_c.x, ch_c.y - 2.0), Pos2::new(ch_c.x + 4.0, ch_c.y + 2.0)], ch_stroke);
                } else {
                    p.line_segment([Pos2::new(ch_c.x - 4.0, ch_c.y - 2.0), Pos2::new(ch_c.x, ch_c.y + 2.0)], ch_stroke);
                    p.line_segment([Pos2::new(ch_c.x, ch_c.y + 2.0), Pos2::new(ch_c.x + 4.0, ch_c.y - 2.0)], ch_stroke);
                }

                widgets::record(ui, cat_id, card_r);
                if card_resp.clicked() {
                    state.expanded_agent_cat = if is_expanded { None } else { Some(cat_idx) };
                }
                y += card_h + 10.0;

                if is_expanded {
                    let sub_items: &[(&str, &str, &str, &str)] = match cat_idx {
                        0 => &[
                            (
                                "term-ai-rename-layers",
                                "Rename layers semantically",
                                "Rename Layers",
                                "Semantically rename generic Frame/Rect/Text layers across selection",
                            ),
                            ("term-ai-design-gate", "Design Gate audit & auto-fix", "Design Gate", "\"$DESIGNOS_BIN\" \"$DESIGNOS_FILE\" --inspect-json"),
                            (
                                "term-ai-translate",
                                "Translate selection (VI <-> EN)",
                                "Translate VI/EN",
                                "Translate selected UI copy between Vietnamese and English",
                            ),
                        ],
                        1 => &[
                            ("term-ai-make-design", "Make design (First Draft)", "Make Design", "Synthesize UI3 NodeSpec blueprint to $DESIGNOS_LIVE_SPEC"),
                            (
                                "term-ai-make-proto",
                                "Make prototype flows",
                                "Make Prototype",
                                "Auto-wire OnClick -> NavigateTo SmartAnimate flows across frames",
                            ),
                            (
                                "term-ai-populate",
                                "Populate realistic data",
                                "Populate Content",
                                "Populate realistic SaaS metrics, names, and statuses into repeated cards",
                            ),
                        ],
                        3 => &[
                            (
                                "term-ai-prd-audit",
                                "Audit PRD & FC-001 traceability",
                                "PRD Audit",
                                "@prd Audit plans/specs/designos-studio-v2/prd/PRD.md & FC-001 traceability across US-01..US-17",
                            ),
                            (
                                "term-ai-prd-story",
                                "Implement active User Story (US-01..US-17)",
                                "Implement Story",
                                "@prd Implement active User Story from plans/specs/designos-studio-v2/us/ onto Canvas",
                            ),
                            (
                                "term-ai-prd-gates",
                                "Verify G1..G5 pipeline gates & Gherkin ACs",
                                "Verify Gates",
                                "@prd Verify G1..G5 pipeline gates and Gherkin scenarios",
                            ),
                        ],
                        _ => &[
                            ("agents-open-pty", "Open Live PTY Terminal", "Open PTY", ""),
                            ("term-left-send-sel", "Send selection & comments to PTY", "Send Selection", ""),
                            ("term-left-apply-out", "Apply NodeSpec from PTY output", "Apply Spec", ""),
                            ("term-left-sync-disk", "Sync canvas from disk", "Sync Canvas", ""),
                        ],
                    };
                    for &(sub_id, sub_label, task_title, cli_prompt) in sub_items {
                        let sub_r = Rect::from_min_size(Pos2::new(card_x + 6.0, y), Vec2::new((card_w - 12.0).max(80.0), 28.0));
                        let sub_resp = ui.interact(sub_r, Id::new(sub_id), Sense::click());
                        p.rect_filled(sub_r, CornerRadius::same(6), if sub_resp.hovered() { TERM_BG_HOVER } else { Color32::from_rgb(0x1E, 0x1E, 0x1E) });
                        p.rect_stroke(sub_r, CornerRadius::same(6), Stroke::new(1.0, Color32::from_rgb(0x33, 0x33, 0x33)), StrokeKind::Inside);
                        let elided = widgets::elide(&p, sub_label, &theme::font_caption_medium(), (sub_r.width() - 20.0).max(24.0));
                        p.text(Pos2::new(sub_r.left() + 10.0, sub_r.center().y), Align2::LEFT_CENTER, elided, theme::font_caption_medium(), theme::TEXT);
                        widgets::record(ui, sub_id, sub_r);
                        if sub_resp.clicked() {
                            match sub_id {
                                "agents-open-pty" => {
                                    state.side_panel_pty_open = true;
                                }
                                "term-left-send-sel" => actions.push(TerminalAction::ExportSelectionToTerminal),
                                "term-left-apply-out" => {
                                    if let Some(act) = resolve_apply_spec_action(state) {
                                        actions.push(act);
                                    }
                                }
                                "term-left-sync-disk" => actions.push(TerminalAction::ReloadFromDisk),
                                _ => {
                                    state.submit_chat_prompt(cli_prompt, file_path);
                                    state.dispatch_figma_ai_prompt(task_title, cli_prompt, file_path);
                                    if sub_id == "term-ai-make-design" {
                                        let spec = crate::cmdk::synthesize_spec_from_prompt("pricing card");
                                        if let Ok(json) = serde_json::to_string(&spec) {
                                            actions.push(TerminalAction::ApplySpecJson(json));
                                        }
                                    }
                                }
                            }
                        }
                        y += 32.0;
                    }
                    y += 4.0;
                }
            }
        });
    }

    // ---- Render Bottom `"Ask for changes"` Composer Box
    p.rect_filled(composer_rect, theme::cr(theme::RADIUS_FRAME), Color32::from_rgb(0x1E, 0x1E, 0x1E));
    p.rect_stroke(composer_rect, theme::cr(theme::RADIUS_FRAME), Stroke::new(1.5, Color32::from_rgb(0x0D, 0x99, 0xFF)), StrokeKind::Inside);
    widgets::record(ui, "agents-composer-box", composer_rect);

    let input_rect = Rect::from_min_max(
        Pos2::new(composer_rect.left() + 12.0, composer_rect.top() + 10.0),
        Pos2::new(composer_rect.right() - 12.0, composer_rect.bottom() - 38.0),
    );
    let mut submit_composer = false;
    ui.scope_builder(egui::UiBuilder::new().max_rect(input_rect), |ui| {
        let te = TextEdit::singleline(&mut state.input_line)
            .id(Id::new("agents-ask-for-changes-input"))
            .desired_width(input_rect.width())
            .frame(egui::Frame::NONE)
            .font(theme::font_body())
            .text_color(Color32::WHITE)
            .hint_text(RichText::new("Ask for changes").font(theme::font_body()).color(Color32::from_rgb(0x8C, 0x8C, 0x8C)));
        let resp = ui.add(te);
        if resp.clicked() || resp.has_focus() {
            state.viewport_focused = false;
        }
        if resp.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
            submit_composer = true;
        }
    });

    let icon_y = composer_rect.bottom() - 20.0;
    let icon_gray = Color32::from_rgb(0xB3, 0xB3, 0xB3);

    // Bottom-left `+` button (attaches selection & PRD context to composer and PTY)
    let plus_r = Rect::from_center_size(Pos2::new(composer_rect.left() + 20.0, icon_y), Vec2::splat(22.0));
    let plus_resp = ui.interact(plus_r, Id::new("agents-composer-plus"), Sense::click());
    if plus_resp.hovered() {
        p.rect_filled(plus_r, CornerRadius::same(4), TERM_BG_HOVER);
    }
    let pc = plus_r.center();
    p.line_segment([Pos2::new(pc.x - 5.0, pc.y), Pos2::new(pc.x + 5.0, pc.y)], Stroke::new(1.5, icon_gray));
    p.line_segment([Pos2::new(pc.x, pc.y - 5.0), Pos2::new(pc.x, pc.y + 5.0)], Stroke::new(1.5, icon_gray));
    widgets::record(ui, "agents-composer-plus", plus_r);
    widgets::record(ui, "agents-composer-attach", plus_r);
    if plus_resp.clicked() {
        if !state.input_line.contains("@selection") {
            let existing = state.input_line.trim().to_string();
            state.input_line = if existing.is_empty() { "@selection @prd ".to_string() } else { format!("@selection @prd {existing}") };
        }
        actions.push(TerminalAction::ExportSelectionToTerminal);
    }

    // Bottom-left open-book button (toggles in-panel live PTY terminal / prompt library)
    let book_r = Rect::from_center_size(Pos2::new(composer_rect.left() + 46.0, icon_y), Vec2::splat(22.0));
    let book_resp = ui.interact(book_r, Id::new("agents-composer-book"), Sense::click());
    if book_resp.hovered() {
        p.rect_filled(book_r, CornerRadius::same(4), TERM_BG_HOVER);
    }
    paint_open_book_icon(&p, book_r.center(), icon_gray);
    widgets::record(ui, "agents-composer-book", book_r);
    if book_resp.clicked() {
        state.side_panel_pty_open = !state.side_panel_pty_open;
    }

    // Bottom-left `PRD` context chip button (attaches/cycles `@prd `, `@selection `, `@tokens `)
    let prd_r = Rect::from_center_size(Pos2::new(composer_rect.left() + 82.0, icon_y), Vec2::new(38.0, 20.0));
    let prd_resp = ui.interact(prd_r, Id::new("agents-composer-prd"), Sense::click());
    let lower_in = state.input_line.to_ascii_lowercase();
    let has_prd_tag = lower_in.contains("@prd") || lower_in.contains("@selection") || lower_in.contains("@tokens");
    let prd_bg = if has_prd_tag {
        Color32::from_rgb(0x16, 0x35, 0x52)
    } else if prd_resp.hovered() {
        TERM_BG_HOVER
    } else {
        Color32::from_rgb(0x28, 0x28, 0x2C)
    };
    p.rect_filled(prd_r, theme::cr(theme::RADIUS_CONTROL), prd_bg);
    p.rect_stroke(
        prd_r,
        theme::cr(theme::RADIUS_CONTROL),
        Stroke::new(1.0, if has_prd_tag { Color32::from_rgb(0x0D, 0x99, 0xFF) } else { Color32::from_rgb(0x3C, 0x3C, 0x42) }),
        StrokeKind::Inside,
    );
    p.text(
        prd_r.center(),
        Align2::CENTER_CENTER,
        "PRD",
        theme::font_caption_strong(),
        if has_prd_tag { Color32::from_rgb(0x6E, 0xA8, 0xFE) } else { icon_gray },
    );
    widgets::record(ui, "agents-composer-prd", prd_r);
    widgets::record(ui, "agents-composer-context", prd_r);
    if prd_resp.clicked() {
        let next_tag = if !lower_in.contains("@prd") {
            "@prd"
        } else if !lower_in.contains("@selection") {
            "@selection"
        } else {
            "@tokens"
        };
        if state.input_line.trim().is_empty() {
            state.input_line = format!("{next_tag} ");
        } else if !lower_in.contains(next_tag) {
            state.input_line = format!("{next_tag} {}", state.input_line.trim());
        }
    }

    // Bottom-right Microphone circle button (`26x26` `#2D364D` circle + `#6EA8FE` mic glyph)
    let mic_center = Pos2::new(composer_rect.right() - 50.0, icon_y);
    let mic_r = Rect::from_center_size(mic_center, Vec2::splat(26.0));
    let mic_resp = ui.interact(mic_r, Id::new("agents-composer-mic"), Sense::click());
    p.circle_filled(mic_center, 13.0, if mic_resp.hovered() { Color32::from_rgb(0x36, 0x41, 0x5C) } else { Color32::from_rgb(0x2D, 0x36, 0x4D) });
    paint_microphone_icon(&p, mic_center, Color32::from_rgb(0x6E, 0xA8, 0xFE));
    widgets::record(ui, "agents-composer-mic", mic_r);
    if mic_resp.clicked() {
        let template = "@prd Audit the selected frame against the PRD acceptance criteria";
        if state.input_line.trim().is_empty() {
            state.input_line = template.to_string();
        } else if !state.input_line.contains(template) {
            state.input_line = format!("{} {template}", state.input_line.trim());
        }
    }

    // Bottom-right Up-Arrow Send circle button (`26x26` `#383838` / `#0D99FF` circle + `↑` arrow)
    let has_text = !state.input_line.trim().is_empty();
    let send_center = Pos2::new(composer_rect.right() - 18.0, icon_y);
    let send_r = Rect::from_center_size(send_center, Vec2::splat(26.0));
    let send_resp = ui.interact(send_r, Id::new("agents-composer-send"), Sense::click());
    let send_bg = if has_text {
        Color32::from_rgb(0x0D, 0x99, 0xFF)
    } else if send_resp.hovered() {
        Color32::from_rgb(0x45, 0x45, 0x45)
    } else {
        Color32::from_rgb(0x38, 0x38, 0x38)
    };
    p.circle_filled(send_center, 13.0, send_bg);
    paint_up_arrow_icon(&p, send_center, if has_text { Color32::WHITE } else { icon_gray });
    widgets::record(ui, "agents-composer-send", send_r);
    if send_resp.clicked() {
        submit_composer = true;
    }

    if submit_composer && has_text {
        let prompt = state.input_line.trim().to_string();
        state.input_line.clear();
        state.submit_chat_prompt(&prompt, file_path);
        if !prompt.starts_with('$') && !prompt.starts_with('!') {
            let spec = crate::cmdk::synthesize_spec_from_prompt(&prompt);
            if let Ok(json) = serde_json::to_string(&spec) {
                if let Some(last_asst) = state.chat_turns.iter_mut().rev().find(|t| t.role == AgentChatRole::Assistant) {
                    last_asst.applied_to_canvas = true;
                }
                actions.push(TerminalAction::ApplySpecJson(json));
            }
        }
    }

    actions
}

fn render_runtime_dropdown_menu(state: &mut TerminalState, ui: &mut Ui, panel_rect: Rect, top_y: f32, file_path: Option<&Path>) {
    let menu_w = (panel_rect.width() - 16.0).clamp(180.0, 224.0);
    let entries = [AgentCliPreset::Claude, AgentCliPreset::Codex, AgentCliPreset::Grok, AgentCliPreset::Antigravity, AgentCliPreset::Shell];
    let row_h = 34.0;
    let header_h = 24.0;
    let menu_h = header_h + entries.len() as f32 * row_h + 8.0;
    let menu_x = (panel_rect.right() - menu_w - 8.0).max(panel_rect.left() + 8.0);
    let menu_r = Rect::from_min_size(Pos2::new(menu_x, top_y), Vec2::new(menu_w, menu_h));

    let p = ui.painter().clone();
    p.rect_filled(menu_r, CornerRadius::same(8), Color32::from_rgb(0x22, 0x22, 0x26));
    p.rect_stroke(menu_r, CornerRadius::same(8), Stroke::new(1.0, Color32::from_rgb(0x3E, 0x3E, 0x46)), StrokeKind::Inside);
    widgets::record(ui, "agents-runtime-dropdown-menu", menu_r);

    p.text(
        Pos2::new(menu_r.left() + 10.0, menu_r.top() + 13.0),
        Align2::LEFT_CENTER,
        "LAUNCH CLI (SKIP PERMISSIONS)",
        theme::font_caption_strong(),
        theme::TEXT_SECONDARY,
    );
    widgets::hline(&p, menu_r.left() + 6.0, menu_r.right() - 6.0, menu_r.top() + header_h, TERM_BORDER);

    let mut chosen: Option<AgentCliPreset> = None;
    for (i, preset) in entries.iter().copied().enumerate() {
        let ry = menu_r.top() + header_h + 4.0 + i as f32 * row_h;
        let row_r = Rect::from_min_size(Pos2::new(menu_r.left() + 4.0, ry), Vec2::new(menu_w - 8.0, row_h - 2.0));
        let id_str = match preset {
            AgentCliPreset::Claude => "agents-dropdown-claude",
            AgentCliPreset::Codex => "agents-dropdown-codex",
            AgentCliPreset::Grok => "agents-dropdown-grok",
            AgentCliPreset::Antigravity => "agents-dropdown-antigravity",
            AgentCliPreset::Shell => "agents-dropdown-shell",
            AgentCliPreset::DesignOsHelp => "agents-dropdown-help",
        };
        let resp = ui.interact(row_r, Id::new(id_str), Sense::click());
        if resp.hovered() {
            p.rect_filled(row_r, theme::cr(theme::RADIUS_CONTROL), TERM_BG_HOVER);
        }
        let (_widget_id, title, subtitle) = preset.dropdown_entry();
        let title_str = widgets::elide(&p, title, &theme::font_caption_medium(), (row_r.width() - 18.0).max(40.0));
        let sub_str = widgets::elide(&p, subtitle, &theme::font_mono(), (row_r.width() - 18.0).max(40.0));
        p.text(Pos2::new(row_r.left() + 8.0, row_r.top() + 10.0), Align2::LEFT_CENTER, title_str, theme::font_caption_medium(), Color32::WHITE);
        p.text(Pos2::new(row_r.left() + 8.0, row_r.top() + 23.0), Align2::LEFT_CENTER, sub_str, theme::font_mono(), STATUS_INFO_TEXT);
        widgets::record(ui, id_str, row_r);
        if resp.clicked() {
            chosen = Some(preset);
        }
    }

    if let Some(preset) = chosen {
        state.runtime_dropdown_open = false;
        state.active_runtime = preset;
        let _ = state.spawn_tab(preset, file_path);
    } else if ui.input(|i| i.pointer.any_pressed()) {
        if let Some(pos) = ui.input(|i| i.pointer.interact_pos()) {
            let add_btn = ui.ctx().data(|d| d.get_temp::<Rect>(Id::new(("test-rect", "agents-runtime-add-btn".to_string()))));
            let in_add = add_btn.map(|r| r.contains(pos)).unwrap_or(false);
            if !menu_r.contains(pos) && !in_add {
                state.runtime_dropdown_open = false;
            }
        }
    }
}

fn wrap_chat_lines(p: &Painter, text: &str, font: &FontId, max_w: f32, max_lines: usize) -> Vec<String> {
    let mut out = Vec::new();
    for raw_line in text.lines() {
        let trimmed = raw_line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let mut current = String::new();
        for word in trimmed.split_whitespace() {
            let candidate = if current.is_empty() { word.to_string() } else { format!("{current} {word}") };
            if widgets::text_width(p, &candidate, font) <= max_w {
                current = candidate;
            } else {
                if !current.is_empty() {
                    out.push(widgets::elide(p, &current, font, max_w));
                    if out.len() >= max_lines {
                        return out;
                    }
                }
                current = widgets::elide(p, word, font, max_w);
            }
        }
        if !current.is_empty() {
            out.push(widgets::elide(p, &current, font, max_w));
            if out.len() >= max_lines {
                return out;
            }
        }
    }
    if out.is_empty() {
        out.push("Running in PTY shell...".to_string());
    }
    out
}

fn render_agent_chat_thread(state: &mut TerminalState, ui: &mut Ui, center_rect: Rect, actions: &mut Vec<TerminalAction>) {
    let mut apply_turn_idx: Option<usize> = None;
    clipped(ui, center_rect, |ui| {
        let p = ui.painter().clone();
        let font = theme::font_caption();
        let font_strong = theme::font_caption_strong();
        let mut y = center_rect.top() + 8.0 - state.chat_scroll;

        for (idx, turn) in state.chat_turns.iter().enumerate() {
            match turn.role {
                AgentChatRole::User => {
                    let card_w = (center_rect.width() - 28.0).max(120.0);
                    let max_txt_w = (card_w - 20.0).max(60.0);
                    let lines = wrap_chat_lines(&p, &turn.text, &font, max_txt_w, 6);
                    let card_h = (26.0 + lines.len() as f32 * 15.0 + 10.0).max(68.0);
                    let card_r = Rect::from_min_size(Pos2::new(center_rect.right() - card_w - 10.0, y), Vec2::new(card_w, card_h));

                    p.rect_filled(card_r, CornerRadius::same(8), Color32::from_rgb(0x26, 0x35, 0x4A));
                    p.rect_stroke(card_r, CornerRadius::same(8), Stroke::new(1.0, Color32::from_rgb(0x3E, 0x4C, 0x6D)), StrokeKind::Inside);

                    p.text(Pos2::new(card_r.left() + 10.0, card_r.top() + 13.0), Align2::LEFT_CENTER, "You", font_strong.clone(), Color32::WHITE);
                    p.text(
                        Pos2::new(card_r.right() - 10.0, card_r.top() + 13.0),
                        Align2::RIGHT_CENTER,
                        turn.runtime.badge(),
                        font_strong.clone(),
                        Color32::from_rgb(0x6E, 0xA8, 0xFE),
                    );
                    widgets::hline(&p, card_r.left() + 8.0, card_r.right() - 8.0, card_r.top() + 23.0, Color32::from_rgb(0x36, 0x46, 0x63));

                    for (li, line) in lines.iter().enumerate() {
                        p.text(
                            Pos2::new(card_r.left() + 10.0, card_r.top() + 34.0 + li as f32 * 15.0),
                            Align2::LEFT_CENTER,
                            line,
                            font.clone(),
                            Color32::from_rgb(0xF0, 0xF4, 0xFF),
                        );
                    }
                    widgets::record(ui, &format!("agents-chat-turn-{}", turn.id), card_r);
                    y += card_h + 8.0;
                }
                AgentChatRole::Assistant => {
                    let card_w = (center_rect.width() - 20.0).max(120.0);
                    let max_txt_w = (card_w - 20.0).max(60.0);
                    let display_txt = if turn.text.trim().is_empty() {
                        format!("[{} · PTY] Executing in live shell...", turn.runtime.tab_title())
                    } else {
                        turn.text.clone()
                    };
                    let lines = wrap_chat_lines(&p, &display_txt, &font, max_txt_w, 8);
                    let card_h = (28.0 + lines.len() as f32 * 15.0 + 26.0).max(82.0);
                    let card_r = Rect::from_min_size(Pos2::new(center_rect.left() + 10.0, y), Vec2::new(card_w, card_h));

                    p.rect_filled(card_r, CornerRadius::same(8), Color32::from_rgb(0x1E, 0x1E, 0x1E));
                    p.rect_stroke(card_r, CornerRadius::same(8), Stroke::new(1.0, Color32::from_rgb(0x38, 0x38, 0x38)), StrokeKind::Inside);

                    p.circle_filled(Pos2::new(card_r.left() + 12.0, card_r.top() + 13.0), 3.0, STATUS_OK_TEXT);
                    let title_elided = widgets::elide(&p, turn.runtime.tab_title(), &font_strong, (card_w - 64.0).max(40.0));
                    p.text(Pos2::new(card_r.left() + 20.0, card_r.top() + 13.0), Align2::LEFT_CENTER, title_elided, font_strong.clone(), Color32::WHITE);
                    p.text(Pos2::new(card_r.right() - 10.0, card_r.top() + 13.0), Align2::RIGHT_CENTER, "PTY", font_strong.clone(), STATUS_OK_TEXT);
                    widgets::hline(&p, card_r.left() + 8.0, card_r.right() - 8.0, card_r.top() + 24.0, Color32::from_rgb(0x2E, 0x2E, 0x2E));

                    for (li, line) in lines.iter().enumerate() {
                        p.text(
                            Pos2::new(card_r.left() + 10.0, card_r.top() + 35.0 + li as f32 * 15.0),
                            Align2::LEFT_CENTER,
                            line,
                            font.clone(),
                            Color32::from_rgb(0xE4, 0xE4, 0xE7),
                        );
                    }

                    let footer_y = card_r.bottom() - 12.0;
                    let has_spec = turn.extracted_spec_json.is_some();
                    let cmd_max_w = if has_spec { (card_w - 88.0).max(36.0) } else { (card_w - 20.0).max(40.0) };
                    let cmd_str = widgets::elide(&p, &format!("$ {}", turn.shell_cmd), &font, cmd_max_w);
                    p.text(Pos2::new(card_r.left() + 10.0, footer_y), Align2::LEFT_CENTER, cmd_str, font.clone(), Color32::from_rgb(0x8C, 0x8C, 0x8C));

                    if has_spec {
                        let btn_r = Rect::from_min_size(Pos2::new(card_r.right() - 72.0, card_r.bottom() - 22.0), Vec2::new(64.0, 18.0));
                        let btn_resp = ui.interact(btn_r, Id::new(("agents-chat-apply-spec", turn.id)), Sense::click());
                        let b_bg = if turn.applied_to_canvas {
                            STATUS_OK_BG
                        } else if btn_resp.hovered() {
                            TERM_BG_HOVER
                        } else {
                            STATUS_INFO_BG
                        };
                        let b_border = if turn.applied_to_canvas { STATUS_OK_BORDER } else { STATUS_INFO_BORDER };
                        let b_fg = if turn.applied_to_canvas { STATUS_OK_TEXT } else { STATUS_INFO_TEXT };
                        let b_label = if turn.applied_to_canvas { "Applied" } else { "Apply" };
                        p.rect_filled(btn_r, CornerRadius::same(4), b_bg);
                        p.rect_stroke(btn_r, CornerRadius::same(4), Stroke::new(1.0, b_border), StrokeKind::Inside);
                        p.text(btn_r.center(), Align2::CENTER_CENTER, b_label, font_strong.clone(), b_fg);
                        widgets::record(ui, "agents-chat-apply-spec", btn_r);
                        if btn_resp.clicked() {
                            apply_turn_idx = Some(idx);
                        }
                    }

                    widgets::record(ui, &format!("agents-chat-turn-{}", turn.id), card_r);
                    y += card_h + 8.0;
                }
            }
        }
    });

    if let Some(idx) = apply_turn_idx {
        if let Some(turn) = state.chat_turns.get_mut(idx) {
            if let Some(spec_json) = turn.extracted_spec_json.clone() {
                turn.applied_to_canvas = true;
                actions.push(TerminalAction::ApplySpecJson(spec_json));
            }
        }
    }
}

fn paint_agents_grid_plus_badge(p: &egui::Painter, center: Pos2) {
    p.circle_filled(center, 19.0, Color32::from_rgb(0x2D, 0x36, 0x4D));
    let col = Color32::from_rgb(0x6E, 0xA8, 0xFE);
    icons::paint(p, Icon::GridView, center, 18.0, col);
}

fn paint_wand_sparkle_icon(p: &egui::Painter, c: Pos2, col: Color32) {
    icons::paint(p, Icon::Wand, c, 14.0, col);
}

fn paint_four_point_star_icon(p: &egui::Painter, c: Pos2, col: Color32) {
    icons::paint(p, Icon::StarFour, c, 14.0, col);
}

fn paint_toolbox_icon(p: &egui::Painter, c: Pos2, col: Color32) {
    icons::paint(p, Icon::Tools, c, 14.0, col);
}

fn paint_open_book_icon(p: &egui::Painter, c: Pos2, col: Color32) {
    icons::paint(p, Icon::BookOpen, c, 14.0, col);
}

fn paint_microphone_icon(p: &egui::Painter, c: Pos2, col: Color32) {
    icons::paint(p, Icon::Microphone, c, 14.0, col);
}

fn paint_up_arrow_icon(p: &egui::Painter, c: Pos2, col: Color32) {
    icons::paint(p, Icon::ArrowUp, c, 14.0, col);
}

fn paint_popout_icon(p: &egui::Painter, c: Pos2, col: Color32) {
    icons::paint(p, Icon::ArrowSquareOut, c, 14.0, col);
}

/// Alias for `show_full` used when rendering the full-workspace Agent Terminal view.
#[allow(dead_code)]
pub fn show_workspace(state: &mut TerminalState, ui: &mut Ui, rect: Rect, file_path: Option<&Path>) -> Vec<TerminalAction> {
    show_full(state, ui, rect, file_path)
}

/// Renders the full-canvas Agent Terminal view (`RailItem::Terminal`).
#[allow(dead_code)]
pub fn show_full(state: &mut TerminalState, ui: &mut Ui, rect: Rect, file_path: Option<&Path>) -> Vec<TerminalAction> {
    render_terminal_surface(state, ui, rect, file_path, "full")
}

/// Renders the docked bottom split-view Agent Terminal (`Ctrl+\`` / `dock_open`).
pub fn show_dock(state: &mut TerminalState, ui: &mut Ui, rect: Rect, file_path: Option<&Path>) -> Vec<TerminalAction> {
    let grip_rect = Rect::from_min_size(Pos2::new(rect.left(), rect.top() - 3.0), Vec2::new(rect.width(), 6.0));
    widgets::record(ui, "dock-resize-grip", grip_rect);
    let actions = render_terminal_surface(state, ui, rect, file_path, "dock");
    let p = ui.painter().clone();
    let pill_r = Rect::from_center_size(Pos2::new(rect.center().x, rect.top() + 2.0), Vec2::new(36.0, 3.0));
    p.rect_filled(pill_r, CornerRadius::same(2), Color32::from_rgb(0x52, 0x52, 0x5B));
    actions
}

fn resolve_apply_spec_action(state: &mut TerminalState) -> Option<TerminalAction> {
    let mut detected_from_guard = None;
    let mut detected_from_log = None;
    if let Some(tab) = state.active_tab() {
        if let Ok(guard) = tab.last_spec_json.lock() {
            detected_from_guard = guard.clone();
        }
        if detected_from_guard.is_none() {
            if let Ok(log) = tab.raw_log.lock() {
                detected_from_log = extract_spec_json_from_log(&log);
            }
        }
    }
    if let Some(json) = detected_from_guard {
        state.status_msg = Some("Applied NodeSpec JSON detected from PTY output".into());
        return Some(TerminalAction::ApplySpecJson(json));
    }
    if let Some(json) = detected_from_log {
        state.status_msg = Some("Applied NodeSpec JSON parsed from PTY log".into());
        return Some(TerminalAction::ApplySpecJson(json));
    }
    let fallback = PathBuf::from("examples/specs/easeui-agent-console.spec.json");
    state.status_msg = Some(format!("Applied {}", fallback.display()));
    Some(TerminalAction::ApplySpecFile(fallback))
}

fn render_terminal_surface(state: &mut TerminalState, ui: &mut Ui, rect: Rect, file_path: Option<&Path>, surface_id: &'static str) -> Vec<TerminalAction> {
    state.ensure_default_tab(file_path);
    let mut actions = state.poll_live_agent_bridge();
    let p = ui.painter().clone();

    if ui.input(|i| i.pointer.any_pressed()) {
        if let Some(pos) = ui.input(|i| i.pointer.interact_pos()) {
            if rect.contains(pos) {
                state.viewport_focused = true;
            }
        }
    }

    p.rect_filled(rect, 0.0, TERM_BG_VIEWPORT);
    widgets::hline(&p, rect.left(), rect.right(), rect.top(), TERM_BORDER);

    // 1. Top Tab & Preset Bar (36px)
    let tab_bar_h = 36.0;
    let tab_bar_rect = Rect::from_min_size(rect.min, Vec2::new(rect.width(), tab_bar_h));
    p.rect_filled(tab_bar_rect, 0.0, TERM_BG_BAR);
    widgets::hline(&p, tab_bar_rect.left(), tab_bar_rect.right(), tab_bar_rect.bottom() - 1.0, TERM_BORDER);

    // Bridge action buttons on the right of the tab bar (with vector icons, zero emojis)
    let mut right_x = tab_bar_rect.right() - 8.0;
    let btn_cy = tab_bar_rect.center().y;

    let sync_r = draw_bar_icon_pill(
        ui,
        &p,
        (surface_id, "term-bar-sync"),
        Pos2::new(right_x, btn_cy),
        Icon::Rotation,
        "Sync Canvas",
        TERM_BG_ELEVATED,
        TERM_BORDER,
        theme::TEXT,
    );
    right_x = sync_r.left() - 6.0;
    if ui.interact(sync_r, Id::new((surface_id, "term-bar-sync")), Sense::click()).clicked() {
        actions.push(TerminalAction::ReloadFromDisk);
    }

    let apply_r = draw_bar_icon_pill(
        ui,
        &p,
        (surface_id, "term-bar-apply-spec"),
        Pos2::new(right_x, btn_cy),
        Icon::Sparkle,
        "Apply Spec from Output",
        STATUS_WARN_BG,
        STATUS_WARN_BORDER,
        STATUS_WARN_TEXT,
    );
    right_x = apply_r.left() - 6.0;
    if ui.interact(apply_r, Id::new((surface_id, "term-bar-apply-spec")), Sense::click()).clicked() {
        if let Some(act) = resolve_apply_spec_action(state) {
            actions.push(act);
        }
    }

    let send_r = draw_bar_icon_pill(
        ui,
        &p,
        (surface_id, "term-bar-send-sel"),
        Pos2::new(right_x, btn_cy),
        Icon::Comment,
        "Send Selection & Comments",
        STATUS_INFO_BG,
        STATUS_INFO_BORDER,
        STATUS_INFO_TEXT,
    );
    right_x = send_r.left() - 8.0;
    if ui.interact(send_r, Id::new((surface_id, "term-bar-send-sel")), Sense::click()).clicked() {
        actions.push(TerminalAction::ExportSelectionToTerminal);
    }

    // Left side of Tab Bar: Session Tabs + Quick-Launch Preset Buttons
    let mut left_x = tab_bar_rect.left() + 8.0;
    let mut tab_to_select: Option<usize> = None;
    let mut tab_to_close: Option<usize> = None;

    for (idx, tab) in state.tabs.iter().enumerate() {
        if left_x + 120.0 > right_x {
            break;
        }
        let is_active = idx == state.active_tab;
        let tab_w = 136.0f32.min((right_x - left_x - 4.0).max(76.0));
        let t_rect = Rect::from_min_size(Pos2::new(left_x, tab_bar_rect.center().y - 12.0), Vec2::new(tab_w, 24.0));
        let close_rect = Rect::from_min_size(Pos2::new(t_rect.right() - 18.0, t_rect.center().y - 7.0), Vec2::new(14.0, 14.0));
        let t_hit_rect = Rect::from_min_max(t_rect.min, Pos2::new((close_rect.left() - 2.0).max(t_rect.left() + 16.0), t_rect.max.y));
        let tab_resp = ui.interact(t_hit_rect, Id::new((surface_id, "term-tab-select", tab.id)), Sense::click());
        let close_resp = ui.interact(close_rect, Id::new((surface_id, "term-tab-close", tab.id)), Sense::click());

        let bg = if is_active {
            TERM_BG_ELEVATED
        } else if tab_resp.hovered() {
            TERM_BG_HOVER
        } else {
            TERM_BG_BAR
        };
        p.rect_filled(t_rect, CornerRadius::same(6), bg);
        if is_active {
            p.rect_stroke(t_rect, CornerRadius::same(6), Stroke::new(1.0, TERM_BORDER), StrokeKind::Inside);
            p.rect_filled(
                Rect::from_min_max(Pos2::new(t_rect.left() + 5.0, t_rect.bottom() - 2.0), Pos2::new(t_rect.right() - 5.0, t_rect.bottom())),
                CornerRadius::same(1),
                TERM_ORANGE,
            );
        }

        paint_orca_tab_status_and_provider(ui, &p, tab, t_rect);
        let title_str = widgets::elide(&p, &tab.title, &theme::font_caption_medium(), (tab_w - 44.0).max(20.0));
        p.text(
            Pos2::new(t_rect.left() + 22.0, t_rect.center().y),
            Align2::LEFT_CENTER,
            title_str,
            theme::font_caption_medium(),
            if is_active { theme::TEXT } else { theme::TEXT_SECONDARY },
        );
        icons::paint(&p, Icon::Close, close_rect.center(), 10.0, if close_resp.hovered() { STATUS_BAD_TEXT } else { theme::ICON_TERTIARY });

        if close_resp.clicked() {
            tab_to_close = Some(idx);
        } else if tab_resp.clicked() {
            tab_to_select = Some(idx);
        }
        left_x = t_rect.right() + 4.0;
    }

    if let Some(idx) = tab_to_select {
        state.active_tab = idx;
        state.viewport_focused = true;
    }
    if let Some(idx) = tab_to_close {
        state.close_tab(idx);
        state.ensure_default_tab(file_path);
    }

    // Quick-launch preset buttons with crisp Plus icon
    for preset in AgentCliPreset::QUICK_LAUNCH {
        let label = preset.button_label();
        let btn_w = (label.len() as f32 * 6.0 + 28.0).clamp(64.0, 98.0);
        if left_x + btn_w > right_x {
            break;
        }
        let b_rect = Rect::from_min_size(Pos2::new(left_x, tab_bar_rect.center().y - 11.0), Vec2::new(btn_w, 22.0));
        let b_resp = ui.interact(b_rect, Id::new((surface_id, "term-preset-spawn", preset as usize)), Sense::click());
        let is_agent = preset != AgentCliPreset::Shell;
        let bg = if b_resp.hovered() {
            TERM_BG_HOVER
        } else if is_agent {
            STATUS_INFO_BG
        } else {
            TERM_BG_ELEVATED
        };
        let border = if is_agent { STATUS_INFO_BORDER } else { TERM_BORDER };
        let fg = if is_agent { STATUS_INFO_TEXT } else { theme::TEXT_SECONDARY };
        p.rect_filled(b_rect, CornerRadius::same(6), bg);
        p.rect_stroke(b_rect, CornerRadius::same(6), Stroke::new(1.0, border), StrokeKind::Inside);
        icons::paint(&p, Icon::Plus, Pos2::new(b_rect.left() + 10.0, b_rect.center().y), 10.0, fg);
        let elided = widgets::elide(&p, label, &theme::font_caption_medium(), (btn_w - 22.0).max(16.0));
        p.text(Pos2::new(b_rect.left() + 18.0, b_rect.center().y), Align2::LEFT_CENTER, elided, theme::font_caption_medium(), fg);
        if b_resp.clicked() {
            let _ = state.spawn_tab(preset, file_path);
        }
        left_x = b_rect.right() + 4.0;
    }

    // 2. VT100 Screen Grid Renderer + Raw Keyboard Capture (full height)
    let viewport_rect = Rect::from_min_max(Pos2::new(rect.left(), tab_bar_rect.bottom()), rect.max);
    render_vt100_viewport(state, ui, viewport_rect, surface_id);
    if let Some(fix) = state.pending_start_fix.take() {
        state.apply_start_fix(fix, file_path);
    }

    ui.ctx().request_repaint_after(std::time::Duration::from_millis(50));

    actions
}

fn extract_selected_terminal_text(screen: &vt100::Screen, sel: ((u16, u16), (u16, u16)), rows: u16, cols: u16) -> String {
    let (start, end) = if sel.0 <= sel.1 { (sel.0, sel.1) } else { (sel.1, sel.0) };
    let r_start = start.0.min(rows.saturating_sub(1));
    let r_end = end.0.min(rows.saturating_sub(1));
    let mut lines = Vec::new();
    for r in r_start..=r_end {
        let c_start = if r == r_start { start.1.min(cols.saturating_sub(1)) } else { 0 };
        let c_end = if r == r_end { end.1.min(cols.saturating_sub(1)) } else { cols.saturating_sub(1) };
        let mut line = String::new();
        for c in c_start..=c_end {
            if let Some(cell) = screen.cell(r, c) {
                if cell.is_wide_continuation() {
                    continue;
                }
                let s = cell.contents();
                if s.is_empty() {
                    line.push(' ');
                } else {
                    line.push_str(&s);
                }
            }
        }
        lines.push(line.trim_end().to_string());
    }
    strip_selection_gutter(&lines.join("\n"))
}

fn render_vt100_viewport(state: &mut TerminalState, ui: &mut Ui, viewport_rect: Rect, surface_id: &'static str) {
    if let Some(failure) = state.active_tab().and_then(|t| t.start_failure.clone()) {
        if let Some(fix) = agent_start_failure::paint(ui, viewport_rect, &failure, surface_id) {
            state.pending_start_fix = Some(fix);
        }
        return;
    }
    let p = ui.painter().clone();
    p.rect_filled(viewport_rect, 0.0, TERM_BG_VIEWPORT);

    let pad_x = 12.0;
    let pad_y = 8.0;
    let origin_x = viewport_rect.left() + pad_x;
    let origin_y = viewport_rect.top() + pad_y;
    let grid_w = (viewport_rect.width() - pad_x * 2.0).max(120.0);
    let grid_h = (viewport_rect.height() - pad_y * 2.0).max(32.0);
    let cols = ((grid_w / CHAR_W).floor() as u16).clamp(20, 320);
    let rows = ((grid_h / ROW_H).floor() as u16).clamp(4, 120);

    let vp_resp = ui.interact(viewport_rect, Id::new((surface_id, "term-vt100-viewport")), Sense::click_and_drag());
    if vp_resp.clicked() || vp_resp.drag_started() {
        state.viewport_focused = true;
    }
    let viewport_focused = state.viewport_focused;
    let search_open = state.search_open;
    let search_query = state.search_query.clone();
    let search_case_sensitive = state.search_case_sensitive;

    let pos_to_cell = |pos: Pos2| -> (u16, u16) {
        let rel_x = (pos.x - origin_x).max(0.0);
        let rel_y = (pos.y - origin_y).max(0.0);
        let c = ((rel_x / CHAR_W).floor() as u16).min(cols.saturating_sub(1));
        let r = ((rel_y / ROW_H).floor() as u16).min(rows.saturating_sub(1));
        (r, c)
    };

    let mut toggle_search = false;
    let mut close_search = false;
    let mut search_matches = 0usize;

    if let Some(tab) = state.active_tab_mut() {
        tab.resize(rows, cols);

        // Pointer click & drag cell selection
        if vp_resp.clicked() && !vp_resp.dragged() {
            tab.selection = None;
        }
        if vp_resp.drag_started() {
            if let Some(pos) = ui.input(|i| i.pointer.press_origin().or_else(|| i.pointer.interact_pos())) {
                let cell = pos_to_cell(pos);
                tab.selection = Some((cell, cell));
            }
        }
        if vp_resp.dragged() {
            if let Some(pos) = ui.input(|i| i.pointer.interact_pos()) {
                let cur = pos_to_cell(pos);
                let start = tab.selection.map(|(s, _)| s).unwrap_or(cur);
                tab.selection = Some((start, cur));
            }
        }

        // Mouse wheel: SGR mouse wheel in alternate screen (`?1049h`) when mouse reporting is active, else 5,000-line scrollback
        if vp_resp.hovered() {
            let delta_y = ui.input(|i| i.smooth_scroll_delta.y);
            if delta_y.abs() > 0.5 {
                let (alt_screen, mouse_active) = tab
                    .parser
                    .lock()
                    .ok()
                    .map(|pg| (pg.screen().alternate_screen(), pg.screen().mouse_protocol_mode() != vt100::MouseProtocolMode::None))
                    .unwrap_or((false, false));
                if alt_screen && mouse_active && !ui.input(|i| i.modifiers.shift) {
                    let (r0, c0) = ui.input(|i| i.pointer.hover_pos()).map(&pos_to_cell).unwrap_or((0, 0));
                    let button = if delta_y > 0.0 { 64 } else { 65 };
                    let seq = format!("\x1b[<{button};{};{}M", c0 + 1, r0 + 1);
                    tab.write_bytes(seq.as_bytes());
                } else if let Ok(mut pg) = tab.parser.lock() {
                    let step = ((delta_y.abs() / ROW_H).ceil() as usize).max(1);
                    let cur_sb = pg.screen().scrollback();
                    let next_sb = if delta_y > 0.0 { cur_sb.saturating_add(step) } else { cur_sb.saturating_sub(step) };
                    pg.set_scrollback(next_sb);
                }
            }
        }

        // Direct raw keystroke, Agent CLI shortcuts & bracketed paste capture when the terminal viewport is focused
        if viewport_focused && !ui.ctx().egui_wants_keyboard_input() {
            let bracketed_paste = tab.parser.lock().ok().map(|pg| pg.screen().bracketed_paste()).unwrap_or(false);
            let mut raw_bytes: Vec<u8> = Vec::new();
            let mut copy_selection = false;
            let mut select_all = false;
            let mut clear_scrollback = false;

            ui.input(|i| {
                for ev in &i.events {
                    match ev {
                        Event::Copy => {
                            if tab.selection.is_some() {
                                copy_selection = true;
                            }
                        }
                        Event::Paste(text) => {
                            raw_bytes.extend_from_slice(&format_paste_bytes(text, bracketed_paste));
                        }
                        Event::Text(text) => {
                            if !i.modifiers.command && !i.modifiers.ctrl && !i.modifiers.alt && text != "\n" && text != "\r" {
                                raw_bytes.extend_from_slice(text.as_bytes());
                            }
                        }
                        Event::Key { key, pressed: true, modifiers, .. } => {
                            if modifiers.command {
                                match key {
                                    Key::F => toggle_search = true,
                                    Key::A => select_all = true,
                                    Key::K => {
                                        clear_scrollback = true;
                                        raw_bytes.push(0x0c);
                                    }
                                    Key::C => {
                                        if tab.selection.is_some() {
                                            copy_selection = true;
                                        }
                                    }
                                    Key::Backspace => raw_bytes.push(0x15),
                                    Key::ArrowLeft => raw_bytes.push(0x01),
                                    Key::ArrowRight => raw_bytes.push(0x05),
                                    _ => {}
                                }
                            } else if modifiers.alt {
                                match key {
                                    Key::ArrowLeft => raw_bytes.extend_from_slice(b"\x1bb"),
                                    Key::ArrowRight => raw_bytes.extend_from_slice(b"\x1bf"),
                                    Key::Backspace => raw_bytes.extend_from_slice(b"\x1b\x7f"),
                                    _ => {}
                                }
                            } else if modifiers.ctrl {
                                match key {
                                    Key::C => {
                                        if tab.selection.is_some() {
                                            copy_selection = true;
                                        } else {
                                            raw_bytes.push(0x03);
                                        }
                                    }
                                    Key::D => raw_bytes.push(0x04),
                                    Key::L => raw_bytes.push(0x0c),
                                    Key::U => raw_bytes.push(0x15),
                                    Key::W => raw_bytes.push(0x17),
                                    Key::A => raw_bytes.push(0x01),
                                    Key::E => raw_bytes.push(0x05),
                                    _ => {}
                                }
                            } else {
                                match key {
                                    Key::Enter if modifiers.shift => raw_bytes.extend_from_slice(b"\x1b\r"),
                                    Key::Enter => raw_bytes.push(b'\r'),
                                    Key::Backspace => raw_bytes.push(0x7f),
                                    Key::Tab => raw_bytes.push(b'\t'),
                                    Key::Escape => {
                                        if search_open {
                                            close_search = true;
                                        } else {
                                            raw_bytes.push(0x1b);
                                        }
                                    }
                                    Key::ArrowUp => raw_bytes.extend_from_slice(b"\x1b[A"),
                                    Key::ArrowDown => raw_bytes.extend_from_slice(b"\x1b[B"),
                                    Key::ArrowRight => raw_bytes.extend_from_slice(b"\x1b[C"),
                                    Key::ArrowLeft => raw_bytes.extend_from_slice(b"\x1b[D"),
                                    Key::Home => raw_bytes.extend_from_slice(b"\x1b[H"),
                                    Key::End => raw_bytes.extend_from_slice(b"\x1b[F"),
                                    Key::Delete => raw_bytes.extend_from_slice(b"\x1b[3~"),
                                    _ => {}
                                }
                            }
                        }
                        _ => {}
                    }
                }
            });

            if select_all {
                tab.selection = Some(((0, 0), (rows.saturating_sub(1), cols.saturating_sub(1))));
            }
            if clear_scrollback {
                if let Ok(mut pg) = tab.parser.lock() {
                    pg.set_scrollback(0);
                }
            }
            if copy_selection {
                if let Some(sel) = tab.selection.take() {
                    if let Ok(pg) = tab.parser.lock() {
                        let copied = extract_selected_terminal_text(pg.screen(), sel, rows, cols);
                        if !copied.is_empty() {
                            ui.ctx().copy_text(copied);
                        }
                    }
                }
            }
            if !raw_bytes.is_empty() {
                if let Ok(mut pg) = tab.parser.lock() {
                    pg.set_scrollback(0);
                }
                tab.write_bytes(&raw_bytes);
            }
        }

        let norm_sel = tab.selection.map(|(a, b)| if a <= b { (a, b) } else { (b, a) });

        clipped(ui, viewport_rect, |ui| {
            let p = ui.painter().clone();
            let Ok(parser) = tab.parser.lock() else {
                return;
            };
            let screen = parser.screen();
            let scrollback_offset = screen.scrollback();
            let (cur_row, cur_col) = screen.cursor_position();
            let font_id = FontId::new(11.5, FontFamily::Monospace);

            let needle = if search_open && !search_query.is_empty() {
                Some(if search_case_sensitive { search_query.clone() } else { search_query.to_ascii_lowercase() })
            } else {
                None
            };

            for r in 0..rows {
                let row_y = origin_y + r as f32 * ROW_H;
                if row_y + ROW_H < viewport_rect.top() || row_y > viewport_rect.bottom() {
                    continue;
                }

                // Compute search match columns for row `r` when `Cmd+F` search is active
                let mut row_search_hit = vec![false; cols as usize];
                if let Some(ref q) = needle {
                    let mut row_chars: Vec<char> = Vec::with_capacity(cols as usize);
                    for c in 0..cols {
                        let ch = screen.cell(r, c).and_then(|cell| cell.contents().chars().next()).unwrap_or(' ');
                        row_chars.push(if search_case_sensitive { ch } else { ch.to_ascii_lowercase() });
                    }
                    let q_chars: Vec<char> = q.chars().collect();
                    if !q_chars.is_empty() && q_chars.len() <= row_chars.len() {
                        for start_c in 0..=(row_chars.len() - q_chars.len()) {
                            if row_chars[start_c..start_c + q_chars.len()] == q_chars[..] {
                                search_matches += 1;
                                row_search_hit[start_c..start_c + q_chars.len()].fill(true);
                            }
                        }
                    }
                }

                let mut job = LayoutJob::default();
                let mut run_text = String::new();
                let mut run_style: Option<(Color32, bool, bool, bool)> = None;

                for c in 0..cols {
                    let Some(cell) = screen.cell(r, c) else {
                        continue;
                    };
                    if cell.is_wide_continuation() {
                        continue;
                    }
                    let mut fg = vt100_color_to_egui(cell.fgcolor(), true);
                    let mut bg = vt100_color_to_egui(cell.bgcolor(), false);
                    if cell.inverse() {
                        std::mem::swap(&mut fg, &mut bg);
                        if bg == Color32::TRANSPARENT {
                            bg = TERM_FG_DEFAULT;
                        }
                        if fg == Color32::TRANSPARENT {
                            fg = TERM_BG_VIEWPORT;
                        }
                    }

                    let cell_rect = Rect::from_min_size(Pos2::new(origin_x + c as f32 * CHAR_W, row_y), Vec2::new(CHAR_W, ROW_H));
                    let is_selected = norm_sel.is_some_and(|(s, e)| (r, c) >= s && (r, c) <= e);
                    let is_search_hit = row_search_hit.get(c as usize).copied().unwrap_or(false);

                    // Paint exact cell-aligned background / selection / search highlight BEFORE text layout (zero vertical seams)
                    if is_selected {
                        p.rect_filled(cell_rect, 0.0, TERM_SELECTION_BG);
                    } else if is_search_hit {
                        p.rect_filled(cell_rect, 0.0, Color32::from_rgb(0x5C, 0x4A, 0x00));
                    } else if bg != Color32::TRANSPARENT {
                        p.rect_filled(cell_rect, 0.0, bg);
                    }

                    let style_key = (fg, cell.bold(), cell.italic(), cell.underline());
                    if run_style != Some(style_key) {
                        if let Some((p_fg, _bold, italics, underline)) = run_style {
                            if !run_text.is_empty() {
                                job.append(
                                    &run_text,
                                    0.0,
                                    TextFormat {
                                        font_id: font_id.clone(),
                                        color: p_fg,
                                        background: Color32::TRANSPARENT,
                                        italics,
                                        underline: if underline { Stroke::new(1.0, p_fg) } else { Stroke::NONE },
                                        ..Default::default()
                                    },
                                );
                                run_text.clear();
                            }
                        }
                        run_style = Some(style_key);
                    }

                    let contents = cell.contents();
                    let mut chars_it = contents.chars();
                    if let (Some(ch), None) = (chars_it.next(), chars_it.next()) {
                        if paint_custom_terminal_glyph(&p, cell_rect, ch, fg) {
                            run_text.push(' ');
                        } else {
                            run_text.push(ch);
                        }
                    } else if contents.is_empty() {
                        run_text.push(' ');
                    } else {
                        run_text.push_str(&contents);
                    }
                }

                let trimmed_len = run_text.trim_end_matches(' ').len();
                let has_underline = run_style.is_some_and(|(_, _, _, ul)| ul);
                if !has_underline {
                    run_text.truncate(trimmed_len);
                }
                if let Some((p_fg, _bold, italics, underline)) = run_style {
                    if !run_text.is_empty() {
                        job.append(
                            &run_text,
                            0.0,
                            TextFormat {
                                font_id: font_id.clone(),
                                color: p_fg,
                                background: Color32::TRANSPARENT,
                                italics,
                                underline: if underline { Stroke::new(1.0, p_fg) } else { Stroke::NONE },
                                ..Default::default()
                            },
                        );
                    }
                }

                if !job.text.is_empty() {
                    let galley = ui.fonts_mut(|f| f.layout_job(job));
                    p.galley(Pos2::new(origin_x, row_y), galley, TERM_FG_DEFAULT);
                }
            }

            // Orca Cursor: solid `#FFFFFF` block + `#1D1F21` cursorAccent when focused; `1px` `#A1A1AA` outline when unfocused
            if scrollback_offset == 0 && !screen.hide_cursor() && cur_row < rows && cur_col < cols {
                let cx = origin_x + cur_col as f32 * CHAR_W;
                let cy = origin_y + cur_row as f32 * ROW_H;
                let cursor_rect = Rect::from_min_size(Pos2::new(cx, cy), Vec2::new(CHAR_W, ROW_H - 1.0));
                if viewport_focused {
                    p.rect_filled(cursor_rect, CornerRadius::same(1), Color32::WHITE);
                    if let Some(cur_cell) = screen.cell(cur_row, cur_col) {
                        let s = cur_cell.contents();
                        if !s.trim().is_empty() {
                            let accent = Color32::from_rgb(0x1D, 0x1F, 0x21);
                            let mut chars_it = s.chars();
                            if let (Some(ch), None) = (chars_it.next(), chars_it.next()) {
                                if !paint_custom_terminal_glyph(&p, cursor_rect, ch, accent) {
                                    p.text(Pos2::new(cx, cy), Align2::LEFT_TOP, s, font_id.clone(), accent);
                                }
                            } else {
                                p.text(Pos2::new(cx, cy), Align2::LEFT_TOP, s, font_id.clone(), accent);
                            }
                        }
                    }
                } else {
                    p.rect_stroke(cursor_rect, CornerRadius::same(1), Stroke::new(1.0, Color32::from_rgb(0xA1, 0xA1, 0xAA)), StrokeKind::Inside);
                }
            }

            // Orca slim `7px` scrollbar + `"↑ Scrollback"` pill when scrolled into history
            if scrollback_offset > 0 {
                let track_h = (viewport_rect.height() - 8.0).max(24.0);
                let thumb_h = (track_h * 0.22).clamp(18.0, 64.0);
                let frac = (scrollback_offset as f32 / 500.0).clamp(0.0, 1.0);
                let thumb_y = viewport_rect.bottom() - 4.0 - thumb_h - (track_h - thumb_h) * frac;
                let sb_rect = Rect::from_min_size(Pos2::new(viewport_rect.right() - 9.0, thumb_y), Vec2::new(7.0, thumb_h));
                p.rect_filled(sb_rect, CornerRadius::same(4), Color32::from_rgba_unmultiplied(180, 180, 185, 102));

                let pill_w = 88.0;
                let pill_h = 20.0;
                let pill_r = Rect::from_min_size(
                    Pos2::new((viewport_rect.right() - pill_w - 14.0).max(viewport_rect.left() + 8.0), viewport_rect.bottom() - pill_h - 8.0),
                    Vec2::new(pill_w, pill_h),
                );
                p.rect_filled(pill_r, CornerRadius::same(10), TERM_BG_ELEVATED);
                p.rect_stroke(pill_r, CornerRadius::same(10), Stroke::new(1.0, TERM_BORDER), StrokeKind::Inside);
                p.text(pill_r.center(), Align2::CENTER_CENTER, "↑ Scrollback", theme::font_caption_medium(), STATUS_INFO_TEXT);
            }
        });
    }

    if toggle_search {
        state.search_open = !state.search_open;
    }
    if close_search {
        state.search_open = false;
    }

    // Orca Floating `Cmd+F` Terminal Search Bar (`TerminalSearch`: `280px × 28px`, `#171717` bg, `8px` radius)
    if state.search_open {
        let search_w = 280.0f32.min((viewport_rect.width() - 12.0).max(160.0));
        let search_h = 28.0;
        let search_rect = Rect::from_min_size(
            Pos2::new((viewport_rect.right() - search_w - 8.0).max(viewport_rect.left() + 6.0), viewport_rect.top() + 8.0),
            Vec2::new(search_w, search_h),
        );
        p.rect_filled(search_rect, CornerRadius::same(8), Color32::from_rgb(0x17, 0x17, 0x17));
        p.rect_stroke(search_rect, CornerRadius::same(8), Stroke::new(1.0, TERM_BORDER), StrokeKind::Inside);

        let close_r = Rect::from_center_size(Pos2::new(search_rect.right() - 14.0, search_rect.center().y), Vec2::splat(20.0));
        let close_resp = ui.interact(close_r, Id::new((surface_id, "term-search-close")), Sense::click());
        if close_resp.hovered() {
            p.rect_filled(close_r, CornerRadius::same(4), TERM_BG_HOVER);
        }
        icons::paint(&p, Icon::Close, close_r.center(), 10.0, if close_resp.hovered() { STATUS_BAD_TEXT } else { theme::ICON_SECONDARY });
        if close_resp.clicked() {
            state.search_open = false;
        }

        let case_r = Rect::from_center_size(Pos2::new(close_r.left() - 14.0, search_rect.center().y), Vec2::new(22.0, 20.0));
        let case_resp = ui.interact(case_r, Id::new((surface_id, "term-search-case")), Sense::click());
        let case_bg = if state.search_case_sensitive {
            STATUS_INFO_BG
        } else if case_resp.hovered() {
            TERM_BG_HOVER
        } else {
            TERM_BG_ELEVATED
        };
        p.rect_filled(case_r, CornerRadius::same(4), case_bg);
        p.rect_stroke(
            case_r,
            CornerRadius::same(4),
            Stroke::new(1.0, if state.search_case_sensitive { STATUS_INFO_BORDER } else { TERM_BORDER }),
            StrokeKind::Inside,
        );
        p.text(
            case_r.center(),
            Align2::CENTER_CENTER,
            "Aa",
            theme::font_caption_strong(),
            if state.search_case_sensitive { STATUS_INFO_TEXT } else { theme::TEXT_SECONDARY },
        );
        if case_resp.clicked() {
            state.search_case_sensitive = !state.search_case_sensitive;
        }

        let count_str = format!("{search_matches}");
        p.text(Pos2::new(case_r.left() - 6.0, search_rect.center().y), Align2::RIGHT_CENTER, count_str, theme::font_caption(), theme::TEXT_SECONDARY);

        let input_r = Rect::from_min_max(
            Pos2::new(search_rect.left() + 8.0, search_rect.top() + 4.0),
            Pos2::new((case_r.left() - 28.0).max(search_rect.left() + 64.0), search_rect.bottom() - 4.0),
        );
        let mut child_ui = ui.new_child(egui::UiBuilder::new().max_rect(input_r));
        let _ = child_ui.add(
            TextEdit::singleline(&mut state.search_query)
                .hint_text(RichText::new("Find...").color(theme::TEXT_TERTIARY).font(theme::font_caption()))
                .font(theme::font_caption())
                .text_color(Color32::WHITE)
                .frame(egui::Frame::NONE),
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_bar_icon_pill(
    ui: &mut Ui,
    p: &Painter,
    id: impl std::hash::Hash + std::fmt::Debug,
    right_center: Pos2,
    icon: Icon,
    label: &str,
    bg: Color32,
    border: Color32,
    fg: Color32,
) -> Rect {
    let w = (label.len() as f32 * 5.8 + 30.0).clamp(84.0, 184.0);
    let h = 22.0;
    let rect = Rect::from_min_size(Pos2::new(right_center.x - w, right_center.y - h / 2.0), Vec2::new(w, h));
    let resp = ui.interact(rect, Id::new(id), Sense::hover());
    let fill = if resp.hovered() { TERM_BG_HOVER } else { bg };
    p.rect_filled(rect, CornerRadius::same(6), fill);
    p.rect_stroke(rect, CornerRadius::same(6), Stroke::new(1.0, border), StrokeKind::Inside);
    icons::paint(p, icon, Pos2::new(rect.left() + 11.0, rect.center().y), 11.0, fg);
    let elided = widgets::elide(p, label, &theme::font_caption_medium(), (w - 24.0).max(20.0));
    p.text(Pos2::new(rect.left() + 20.0, rect.center().y), Align2::LEFT_CENTER, elided, theme::font_caption_medium(), fg);
    rect
}

/// Converts a `vt100::Color` (Default, 16-ANSI, 256-palette, or 24-bit TrueColor RGB) to `egui::Color32`.
pub fn vt100_color_to_egui(c: vt100::Color, is_fg: bool) -> Color32 {
    match c {
        vt100::Color::Default => {
            if is_fg {
                TERM_FG_DEFAULT
            } else {
                Color32::TRANSPARENT
            }
        }
        vt100::Color::Rgb(r, g, b) => Color32::from_rgb(r, g, b),
        vt100::Color::Idx(idx) => ansi_256_to_color32(idx),
    }
}

fn ansi_256_to_color32(idx: u8) -> Color32 {
    // Orca `Ghostty Default Style Dark` (`src/shared/terminal-themes/defaults.ts`)
    const ANSI_16: [(u8, u8, u8); 16] = [
        (0x1D, 0x1F, 0x21), // 0 Black
        (0xCC, 0x66, 0x66), // 1 Red
        (0xB5, 0xBD, 0x68), // 2 Green
        (0xF0, 0xC6, 0x74), // 3 Yellow
        (0x81, 0xA2, 0xBE), // 4 Blue
        (0xB2, 0x94, 0xBB), // 5 Magenta
        (0x8A, 0xBE, 0xB7), // 6 Cyan
        (0xC5, 0xC8, 0xC6), // 7 White
        (0x66, 0x66, 0x66), // 8 Bright Black
        (0xD5, 0x4E, 0x53), // 9 Bright Red
        (0xB9, 0xCA, 0x4A), // 10 Bright Green
        (0xE7, 0xC5, 0x47), // 11 Bright Yellow
        (0x7A, 0xA6, 0xDA), // 12 Bright Blue
        (0xC3, 0x97, 0xD8), // 13 Bright Magenta
        (0x70, 0xC0, 0xB1), // 14 Bright Cyan
        (0xEA, 0xEA, 0xEA), // 15 Bright White
    ];
    match idx {
        0..=15 => {
            let (r, g, b) = ANSI_16[idx as usize];
            Color32::from_rgb(r, g, b)
        }
        16..=231 => {
            let n = idx - 16;
            let r_idx = n / 36;
            let g_idx = (n % 36) / 6;
            let b_idx = n % 6;
            let to_u8 = |v: u8| if v == 0 { 0 } else { 55 + v * 40 };
            Color32::from_rgb(to_u8(r_idx), to_u8(g_idx), to_u8(b_idx))
        }
        232..=255 => {
            let gray = 8 + (idx - 232) * 10;
            Color32::from_rgb(gray, gray, gray)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use designos_doc::{Comment, CommentAnchor, CommentThread, Node};

    #[test]
    fn extract_spec_json_from_fenced_and_raw_terminal_output() {
        let log = r#"
$ claude "design an EaseUI card"
Here is the NodeSpec blueprint:
```json
{
  "name": "Agent Card",
  "preset": "easeui-card",
  "children": [
    { "kind": "text", "name": "Title", "text": "Hello from Claude" }
  ]
}
```
Done!
"#;
        let extracted = extract_spec_json_from_log(log).expect("should extract fenced NodeSpec JSON");
        let parsed: NodeSpec = serde_json::from_str(&extracted).expect("valid NodeSpec");
        assert_eq!(parsed.name, "Agent Card");
        assert_eq!(parsed.preset.as_deref(), Some("easeui-card"));
        assert_eq!(parsed.children.len(), 1);
    }

    #[test]
    fn pty_spawn_and_vt100_screen_capture_work() {
        let mut tab = TerminalTab::spawn(1, AgentCliPreset::Shell, 24, 80, None).expect("should spawn shell PTY");
        tab.send_line("echo DESIGNOS_PTY_OK");

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        let mut found = false;
        while std::time::Instant::now() < deadline {
            if let Ok(p) = tab.parser.lock() {
                if p.screen().contents().contains("DESIGNOS_PTY_OK") {
                    found = true;
                    break;
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(40));
        }
        let _ = tab.child.kill();
        assert!(found, "VT100 screen should capture 'DESIGNOS_PTY_OK' from PTY stdout");
    }

    #[test]
    fn send_copy_for_ai_formats_product_os_citation_and_opens_dock() {
        let payload = format_copy_for_ai_payload("## US-01: Native PTY Agent Terminal");
        assert!(payload.contains("- File: spec/"));
        assert!(payload.contains("- Mục:"));
        assert!(payload.contains("- Trang reader:"));
        assert!(payload.contains("- Project:"));
        assert!(payload.contains("Yêu cầu của tôi:"));
    }

    #[test]
    fn antigravity_preset_and_quick_launch_cover_all_five_cli_runtimes() {
        assert_eq!(AgentCliPreset::QUICK_LAUNCH.len(), 5);
        assert!(AgentCliPreset::QUICK_LAUNCH.contains(&AgentCliPreset::Antigravity));
        assert_eq!(AgentCliPreset::Antigravity.tab_title(), "Antigravity (agy)");
        assert_eq!(AgentCliPreset::Antigravity.button_label(), "Antigravity");
        assert_eq!(AgentCliPreset::Antigravity.badge(), "AGY");
        assert_eq!(AgentCliPreset::Antigravity.binary_name(), "agy");
        assert!(AgentCliPreset::Shell.is_installed());
    }

    #[test]
    fn poll_live_agent_bridge_detects_updated_spec_file() {
        let _ = std::fs::remove_file("/tmp/designos-live-agent-out.spec.json");
        let live_path = std::env::temp_dir().join(format!("designos-live-agent-test-{}.spec.json", std::process::id()));
        let mut st = TerminalState { last_watched_mtime: Some(std::time::UNIX_EPOCH), live_spec_path: live_path.clone(), ..TerminalState::default() };
        let _ = std::fs::write(&live_path, r#"{"name":"Live Agent Frame","preset":"easeui-card"}"#);
        let actions = st.poll_live_agent_bridge();
        assert_eq!(actions, vec![TerminalAction::ApplySpecFile(live_path.clone())]);
        // Subsequent poll without modification returns empty
        assert!(st.poll_live_agent_bridge().is_empty());
        let _ = std::fs::remove_file(&live_path);
    }

    #[test]
    fn figma_desktop_agents_side_panel_renders_and_passes_layout_lint() {
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        crate::theme::apply(&ctx);
        let mut st = TerminalState::default();
        assert!(st.side_panel_pty_open, "Default Agents panel must open directly into live PTY Shell");
        assert_eq!(st.active_runtime, AgentCliPreset::Shell);
        let panel_rect = Rect::from_min_size(Pos2::new(48.0, 0.0), Vec2::new(240.0, 800.0));

        // 1. Verify default pure PTY shell + `+` dropdown menu renders cleanly
        st.runtime_dropdown_open = true;
        crate::lint::take_icons();
        let raw_pty = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1600.0, 900.0))), ..Default::default() };
        let out_pty = ctx.run_ui(raw_pty, |ui| {
            let _ = show_left(&mut st, ui, panel_rect, None);
        });
        let (pty_texts, _pty_boxes) = crate::lint::scan_shapes(&out_pty.shapes);
        let pty_all_text: Vec<&str> = pty_texts.iter().map(|t| t.text.as_str()).collect();
        assert!(pty_all_text.iter().any(|t| t.contains("Shell")), "Default Shell tab must be painted: {pty_all_text:?}");
        assert!(pty_all_text.iter().any(|t| t.contains("Claude")), "Dropdown 'Claude' must be painted: {pty_all_text:?}");
        assert!(pty_all_text.iter().any(|t| t.contains("Codex")), "Dropdown 'Codex' must be painted: {pty_all_text:?}");
        assert!(pty_all_text.iter().any(|t| t.contains("Grok")), "Dropdown 'Grok' must be painted: {pty_all_text:?}");
        assert!(pty_all_text.iter().any(|t| t.contains("Antigravity")), "Dropdown 'Antigravity' must be painted: {pty_all_text:?}");
        assert!(!pty_all_text.contains(&"Learn more"), "AI credits banner must be removed");
        st.runtime_dropdown_open = false;

        // 2. Also verify chat/categories view when side_panel_pty_open is false
        st.side_panel_pty_open = false;
        st.chat_mode = false;
        crate::lint::take_icons();
        let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1600.0, 900.0))), ..Default::default() };
        let out = ctx.run_ui(raw, |ui| {
            let _ = show_left(&mut st, ui, panel_rect, None);
        });
        let (texts, boxes) = crate::lint::scan_shapes(&out.shapes);
        let all_text: Vec<&str> = texts.iter().map(|t| t.text.as_str()).collect();
        assert!(all_text.contains(&"New chat"), "Header 'New chat' must be painted: {all_text:?}");
        assert!(all_text.contains(&"Shared"), "Subtitle 'Shared' must be painted: {all_text:?}");
        assert!(all_text.contains(&"AI"), "Badge 'AI' must be painted: {all_text:?}");
        assert!(all_text.contains(&"What's next?"), "Heading 'What's next?' must be painted: {all_text:?}");
        assert!(all_text.contains(&"Tidy and polish"), "Category 'Tidy and polish' must be painted: {all_text:?}");
        assert!(all_text.contains(&"Generate designs"), "Category 'Generate designs' must be painted: {all_text:?}");
        assert!(all_text.contains(&"Create plugins and shaders"), "Category 'Create plugins and shaders' must be painted: {all_text:?}");
        assert!(!all_text.contains(&"Learn more"), "Credits banner 'Learn more' must NOT be painted: {all_text:?}");

        let text_findings = crate::lint::check_text(&texts, &boxes);
        let icon_findings = crate::lint::check_icons(&crate::lint::take_icons());
        assert!(text_findings.is_empty(), "Agents side panel text lint findings: {text_findings:?}");
        assert!(icon_findings.is_empty(), "Agents side panel icon lint findings: {icon_findings:?}");

        // Also verify expanded categories and in-panel PTY mode render cleanly
        for cat in [Some(0), Some(1), Some(2)] {
            st.expanded_agent_cat = cat;
            let raw2 = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1600.0, 900.0))), ..Default::default() };
            let out2 = ctx.run_ui(raw2, |ui| {
                let _ = show_left(&mut st, ui, panel_rect, None);
            });
            let (t2, b2) = crate::lint::scan_shapes(&out2.shapes);
            assert!(crate::lint::check_text(&t2, &b2).is_empty());
        }

        st.side_panel_pty_open = true;
        let raw3 = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1600.0, 900.0))), ..Default::default() };
        let _ = ctx.run_ui(raw3, |ui| {
            let _ = show_left(&mut st, ui, panel_rect, None);
        });
        for mut tab in st.tabs.drain(..) {
            let _ = tab.child.kill();
        }
    }

    #[test]
    fn agents_chat_wrapper_executes_in_pty_shell_and_streams_clean_output() {
        let stripped = strip_ansi_codes("\x1b[1;32mHello\x1b[0m \x1b]0;WindowTitle\x07World\r\n");
        assert_eq!(stripped.trim(), "Hello World");

        let mut st = TerminalState::default();
        assert_eq!(st.active_runtime, AgentCliPreset::Shell);
        st.side_panel_pty_open = false;
        st.chat_mode = true;
        st.set_active_runtime(AgentCliPreset::Codex, None);
        assert_eq!(st.active_runtime, AgentCliPreset::Codex);
        st.set_active_runtime(AgentCliPreset::Antigravity, None);
        assert_eq!(st.active_runtime, AgentCliPreset::Antigravity);
        st.set_active_runtime(AgentCliPreset::Grok, None);
        assert_eq!(st.active_runtime, AgentCliPreset::Grok);
        st.set_active_runtime(AgentCliPreset::Claude, None);
        assert_eq!(st.active_runtime, AgentCliPreset::Claude);

        st.submit_chat_prompt("Create a SaaS analytics card in Claude Code", None);
        assert_eq!(st.chat_turns.len(), 2);
        assert_eq!(st.chat_turns[0].role, AgentChatRole::User);
        assert_eq!(st.chat_turns[0].text, "Create a SaaS analytics card in Claude Code");
        assert_eq!(st.chat_turns[1].role, AgentChatRole::Assistant);

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        let mut synced = false;
        while std::time::Instant::now() < deadline {
            st.sync_chat_from_pty();
            let reply = &st.chat_turns[1].text;
            if !reply.trim().is_empty() {
                assert!(!reply.contains("DESIGNOS_CHAT_BEGIN_"), "Marker should not leak into chat bubble: {reply}");
                assert!(!reply.contains("\x1b["), "ANSI CSI should be stripped: {reply}");
                synced = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(40));
        }
        assert!(synced, "Assistant chat turn should receive live PTY shell output");

        // Inject an extracted NodeSpec JSON on the assistant turn to also verify the 'Apply' button layout
        st.chat_turns[1].extracted_spec_json = Some(r#"{"name":"Analytics Card","preset":"easeui-card"}"#.to_string());

        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        crate::theme::apply(&ctx);
        let panel_rect = Rect::from_min_size(Pos2::new(48.0, 0.0), Vec2::new(240.0, 800.0));
        crate::lint::take_icons();
        let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1600.0, 900.0))), ..Default::default() };
        let out = ctx.run_ui(raw, |ui| {
            let _ = show_left(&mut st, ui, panel_rect, None);
        });
        let (texts, boxes) = crate::lint::scan_shapes(&out.shapes);
        let all_text: Vec<&str> = texts.iter().map(|t| t.text.as_str()).collect();
        assert!(all_text.contains(&"Claude"), "Runtime pill 'Claude' must be painted: {all_text:?}");
        assert!(all_text.contains(&"Codex"), "Runtime pill 'Codex' must be painted: {all_text:?}");
        assert!(all_text.contains(&"AGY"), "Runtime pill 'AGY' must be painted: {all_text:?}");
        assert!(all_text.contains(&"Grok"), "Runtime pill 'Grok' must be painted: {all_text:?}");
        assert!(all_text.contains(&"Shell"), "Runtime pill 'Shell' must be painted: {all_text:?}");
        assert!(all_text.contains(&"You"), "User bubble header 'You' must be painted: {all_text:?}");
        assert!(all_text.contains(&"PTY"), "Assistant bubble 'PTY' badge must be painted: {all_text:?}");
        assert!(all_text.contains(&"Apply"), "Assistant bubble 'Apply' spec button must be painted: {all_text:?}");

        let text_findings = crate::lint::check_text(&texts, &boxes);
        let icon_findings = crate::lint::check_icons(&crate::lint::take_icons());
        assert!(text_findings.is_empty(), "Chat thread text lint findings: {text_findings:?}");
        assert!(icon_findings.is_empty(), "Chat thread icon lint findings: {icon_findings:?}");

        for mut tab in st.tabs.drain(..) {
            let _ = tab.child.kill();
        }
    }

    #[test]
    fn all_four_agent_cli_runtimes_claude_codex_agy_grok_execute_in_pty() {
        let mut st = TerminalState::default();
        for (preset, prompt, expected_tag) in [
            (AgentCliPreset::Claude, "Audit layout in Claude", "[Claude Code · claude PTY]"),
            (AgentCliPreset::Codex, "Generate React code in Codex", "[Codex CLI · codex PTY]"),
            (AgentCliPreset::Antigravity, "Check parity in Antigravity", "[Antigravity (agy) · agy PTY]"),
            (AgentCliPreset::Grok, "Fast build in Grok", "[Grok Build · grok PTY]"),
        ] {
            st.set_active_runtime(preset, None);
            let turn_idx = st.chat_turns.len() + 1;
            st.submit_chat_prompt(prompt, None);
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(4);
            let mut ok = false;
            while std::time::Instant::now() < deadline {
                st.sync_chat_from_pty();
                if st.chat_turns[turn_idx].text.contains(expected_tag) {
                    ok = true;
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(40));
            }
            assert!(ok, "Runtime {:?} failed to stream expected output '{}' in PTY: got '{}'", preset, expected_tag, st.chat_turns[turn_idx].text);
        }
        for mut tab in st.tabs.drain(..) {
            let _ = tab.child.kill();
        }
    }

    #[test]
    fn agent_chat_supports_prd_and_spec_context_handoff() {
        let mut st = TerminalState::default();
        st.submit_chat_prompt("@prd Review & implement spec block (REQ-01)", None);
        assert_eq!(st.chat_turns.len(), 2);
        assert!(st.chat_turns[0].text.contains("@prd"), "User turn should carry @prd tag: {}", st.chat_turns[0].text);

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(4);
        let mut prd_streamed = false;
        while std::time::Instant::now() < deadline {
            st.sync_chat_from_pty();
            if st.chat_turns[1].text.contains("PRD Context: plans/specs/designos-studio-v2/prd/PRD.md") {
                prd_streamed = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(40));
        }
        assert!(prd_streamed, "Assistant PTY turn should include PRD Context line: got '{}'", st.chat_turns[1].text);

        for mut tab in st.tabs.drain(..) {
            let _ = tab.child.kill();
        }
    }

    #[test]
    fn agent_runtime_env_is_set_at_spawn_and_per_command_never_typed_into_live_tabs() {
        let mut cmd = CommandBuilder::new("sh");
        configure_pty_env(&mut cmd, AgentCliPreset::Codex, None);
        assert_eq!(cmd.get_env("DESIGNOS_AGENT_RUNTIME"), Some(std::ffi::OsStr::new("codex")));

        let live_spec = std::path::Path::new("/tmp/designos-live-agent-out.spec.json");
        let sel_json = std::path::Path::new("/tmp/designos-selection.json");
        let grok_cmd = build_real_cli_command(AgentCliPreset::Grok, "Build card", live_spec, sel_json);
        assert!(grok_cmd.contains("DESIGNOS_AGENT_RUNTIME='grok'"));
    }

    #[test]
    fn real_cli_command_builder_and_figma_ai_dispatch_execute_real_commands() {
        assert!(runtime_binary_available("sh"));
        assert!(!runtime_binary_available("nonexistent_designos_binary_xyz_999"));

        assert!(AgentCliPreset::Claude.cli_launch_command().starts_with("claude --dangerously-skip-permissions --mcp-config \"$DESIGNOS_MCP_CONFIG\""));
        assert!(AgentCliPreset::Codex.cli_launch_command().starts_with("codex --dangerously-bypass-approvals-and-sandbox"));
        assert!(AgentCliPreset::Grok.cli_launch_command().starts_with("grok --always-approve"));
        assert!(AgentCliPreset::Antigravity.cli_launch_command().starts_with("agy --dangerously-skip-permissions"));
        assert_eq!(AgentCliPreset::Shell.cli_launch_command(), "");

        let live_spec = std::path::Path::new("/tmp/designos-live-agent-out.spec.json");
        let sel_json = std::path::Path::new("/tmp/designos-selection.json");
        let claude_cmd = build_real_cli_command(AgentCliPreset::Claude, "Create a SaaS hero frame", live_spec, sel_json);
        assert!(claude_cmd.contains("claude --dangerously-skip-permissions -p"));
        assert!(claude_cmd.contains("--mcp-config"));
        assert!(claude_cmd.contains("DESIGNOS_LIVE_SPEC"));
        assert!(claude_cmd.contains("DESIGNOS_SELECTION_JSON"));

        let codex_cmd = build_real_cli_command(AgentCliPreset::Codex, "Audit layout", live_spec, sel_json);
        assert!(codex_cmd.contains("codex --dangerously-bypass-approvals-and-sandbox exec"));
        let agy_cmd = build_real_cli_command(AgentCliPreset::Antigravity, "Check parity", live_spec, sel_json);
        assert!(agy_cmd.contains("agy --dangerously-skip-permissions") && agy_cmd.contains("-p"));
        let grok_cmd = build_real_cli_command(AgentCliPreset::Grok, "Build card", live_spec, sel_json);
        assert!(grok_cmd.contains("grok --always-approve") && grok_cmd.contains("-p"));

        let mut st = TerminalState::default();
        st.dispatch_figma_ai_prompt("Auto-rename layers to semantic names", "Auto-rename layers to semantic names", None);
        assert!(st.dock_open);
        assert_eq!(st.chat_turns.len(), 2);
        assert!(st.chat_turns[0].text.contains("Auto-rename layers to semantic names"));

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        let mut got_reply = false;
        while std::time::Instant::now() < deadline {
            st.sync_chat_from_pty();
            if st.chat_turns[1].text.contains("[Figma AI ·") {
                got_reply = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(40));
        }
        assert!(got_reply, "dispatch_figma_ai_prompt must stream real PTY output instead of a comment line");
        for mut tab in st.tabs.drain(..) {
            let _ = tab.child.kill();
        }
    }

    #[test]
    fn agents_header_and_composer_buttons_mutate_real_state() {
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        crate::theme::apply(&ctx);
        let mut st = TerminalState { side_panel_pty_open: false, chat_mode: true, ..TerminalState::default() };
        let panel_rect = Rect::from_min_size(Pos2::new(48.0, 0.0), Vec2::new(240.0, 800.0));

        // First pass to record widget rects
        let _ = ctx.run_ui(egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1600.0, 900.0))), ..Default::default() }, |ui| {
            let _ = show_left(&mut st, ui, panel_rect, None);
        });
        let get_rect =
            |name: &str| -> Rect { ctx.data(|d| d.get_temp::<Rect>(Id::new(("test-rect", name.to_string())))).unwrap_or_else(|| panic!("{name} recorded")) };
        let ai_rect = get_rect("agents-header-ai");
        let plus_rect = get_rect("agents-composer-attach");
        let prd_rect = get_rect("agents-composer-context");
        let mic_rect = get_rect("agents-composer-mic");

        // Click AI header button -> toggles chat_mode
        assert!(st.chat_mode);
        let click_at = |ctx: &egui::Context, st: &mut TerminalState, pos: Pos2| -> Vec<TerminalAction> {
            let mut acts = Vec::new();
            let raw = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1600.0, 900.0))),
                events: vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE },
                    egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE },
                ],
                ..Default::default()
            };
            let _ = ctx.run_ui(raw, |ui| {
                acts = show_left(st, ui, panel_rect, None);
            });
            acts
        };

        let _ = click_at(&ctx, &mut st, ai_rect.center());
        assert!(!st.chat_mode);
        assert!(st.side_panel_pty_open);

        // Switch back to chat view to test composer buttons
        st.side_panel_pty_open = false;
        let _ = ctx.run_ui(egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1600.0, 900.0))), ..Default::default() }, |ui| {
            let _ = show_left(&mut st, ui, panel_rect, None);
        });

        // Click `+` attach button -> prepends @selection @prd and emits ExportSelectionToTerminal
        let acts = click_at(&ctx, &mut st, plus_rect.center());
        assert!(st.input_line.contains("@selection"));
        assert!(acts.contains(&TerminalAction::ExportSelectionToTerminal));

        // Click `PRD` context chip -> cycles next context tag (@tokens since @prd & @selection are present)
        let _ = click_at(&ctx, &mut st, prd_rect.center());
        assert!(st.input_line.contains("@tokens"));

        // Click mic button -> appends voice audit prompt template
        let _ = click_at(&ctx, &mut st, mic_rect.center());
        assert!(st.input_line.contains("Audit the selected frame against the PRD acceptance criteria"));

        for mut tab in st.tabs.drain(..) {
            let _ = tab.child.kill();
        }
    }

    #[test]
    fn per_file_session_isolation_auto_provisions_mcp_hooks_skills_and_syncs_live_doc() {
        let mut st1 = TerminalState::for_tab(701);
        let mut st2 = TerminalState::for_tab(702);
        assert_ne!(st1.session_dir, st2.session_dir);
        assert_ne!(st1.live_spec_path, st2.live_spec_path);
        assert_ne!(st1.live_doc_sync_path, st2.live_doc_sync_path);
        assert_eq!(st1.dock_height, 400.0);

        let mut doc1 = Document::new("File Alpha");
        let _page1 = doc1.add_page("Page 1");
        let page2 = doc1.add_page("Page 2 — Agent Target");
        let mut doc2 = Document::new("File Beta");
        let beta_page = doc2.add_page("Page 1");

        st1.sync_live_file_context(&doc1, page2, &[], None);
        st2.sync_live_file_context(&doc2, beta_page, &[], None);

        assert_eq!(st1.current_page_id, page2.0);
        assert_eq!(st1.current_page_name, "Page 2 — Agent Target");
        assert_eq!(st1.doc_name, "File Alpha");
        assert!(st1.context_json_path.exists());
        assert!(st1.live_canvas_path.exists());
        assert!(st1.session_dir.join("mcp-config.json").exists());
        assert!(st1.session_dir.join("hooks.json").exists());
        assert!(st1.session_dir.join("SKILL.md").exists());

        let ctx_val: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&st1.context_json_path).expect("read context.json")).expect("parse context.json");
        assert_eq!(ctx_val["current_page_id"].as_u64(), Some(page2.0));
        assert_eq!(ctx_val["current_page_name"].as_str(), Some("Page 2 — Agent Target"));

        // Simulate MCP server writing doc-sync.designos into session 1 and the switch into its log
        let mut mutated = doc1.clone();
        let _ = mutated
            .apply_spec(page2, &NodeSpec { kind: "frame".into(), name: "MCP Synced Frame".into(), preset: Some("easeui-card".into()), ..Default::default() });
        let json = mutated.to_json().expect("json");
        std::fs::write(&st1.live_doc_sync_path, &json).expect("save doc-sync.designos");
        let marker = designos_doc::live_patch::DocEpoch::of(1, json.as_bytes());
        let line = serde_json::to_string(&marker).expect("marker json") + "\n";
        std::fs::write(st1.session_dir.join("doc-patch.jsonl"), line).expect("append switch");

        let actions1 = st1.poll_live_agent_bridge();
        assert_eq!(actions1, vec![TerminalAction::ApplyLiveLog(vec![designos_doc::live_patch::LiveLogLine::DocEpoch(marker)])]);
        // Session 2 must remain completely unaffected
        assert!(st2.poll_live_agent_bridge().is_empty());

        st1.shutdown();
        st2.shutdown();
        let _ = std::fs::remove_dir_all(&st1.session_dir);
        let _ = std::fs::remove_dir_all(&st2.session_dir);
    }

    #[test]
    fn orca_terminal_features_bracketed_paste_custom_glyphs_gutter_and_osc_status_work() {
        // 1. Bracketed paste sanitizes raw ESC, normalizes newlines to \r, and wraps in \x1b[200~...\x1b[201~
        let pasted = format_paste_bytes("line1\x1b[31m\r\nline2\nline3", false);
        assert_eq!(pasted, b"\x1b[200~line1\xE2\x90\x9B[31m\rline2\rline3\x1b[201~");
        let single_bracketed = format_paste_bytes("echo hello", true);
        assert_eq!(single_bracketed, b"\x1b[200~echo hello\x1b[201~");
        let single_unbracketed = format_paste_bytes("echo hello", false);
        assert_eq!(single_unbracketed, b"echo hello");

        // 2. strip_selection_gutter strips shared 2-space TUI margin while preserving relative code indentation
        let raw_tui_sel = "  fn main() {   \n      println!(\"orca\");  \n  }   ";
        assert_eq!(strip_selection_gutter(raw_tui_sel), "fn main() {\n    println!(\"orca\");\n}");

        // 3. paint_custom_terminal_glyph procedurally paints box-drawing, block elements, shades, and Braille spinners
        let ctx = egui::Context::default();
        let _ = ctx.run_ui(egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 200.0))), ..Default::default() }, |ui| {
            let p = ui.painter();
            let cell_rect = Rect::from_min_size(Pos2::new(10.0, 10.0), Vec2::new(CHAR_W, ROW_H));
            for ch in ['╭', '─', '╮', '│', '╰', '╯', '═', '║', '┄', '█', '░', '▒', '▓', '▙', '⠋', '⠙', '⠹'] {
                assert!(paint_custom_terminal_glyph(p, cell_rect, ch, TERM_FG_DEFAULT), "Custom glyph renderer must handle '{ch}'");
            }
            assert!(!paint_custom_terminal_glyph(p, cell_rect, 'A', TERM_FG_DEFAULT));
        });

        // 4. parse_osc_agent_status_and_strip parses and strips OSC 9999 without polluting VT100 output
        let osc_chunk = "Before\x1b]9999;{\"state\":\"working\",\"tool\":\"designos_apply_spec\"}\x07After";
        let (cleaned, status) = parse_osc_agent_status_and_strip(osc_chunk);
        assert_eq!(cleaned, "BeforeAfter");
        assert_eq!(status, Some(AgentTabStatus::Working(Some("designos_apply_spec".to_string()))));

        let (_, wait_status) = parse_osc_agent_status_and_strip("\x1b]9999;waiting:bash\x07");
        assert_eq!(wait_status, Some(AgentTabStatus::WaitingForInput(Some("bash".to_string()))));

        let (_, done_status) = parse_osc_agent_status_and_strip("\x1b]9999;{\"state\":\"done\"}\x1b\\");
        assert_eq!(done_status, Some(AgentTabStatus::Done));

        // 5. Ghostty Default Style Dark palette
        assert_eq!(vt100_color_to_egui(vt100::Color::Idx(0), true), Color32::from_rgb(0x1D, 0x1F, 0x21));
        assert_eq!(vt100_color_to_egui(vt100::Color::Idx(1), true), Color32::from_rgb(0xCC, 0x66, 0x66));
        assert_eq!(vt100_color_to_egui(vt100::Color::Idx(15), true), Color32::from_rgb(0xEA, 0xEA, 0xEA));
    }

    /// Records every byte the app writes to the tab. `_pty_writer` is the real PTY writer this recorder replaced:
    /// portable-pty's `UnixMasterWriter` sends a newline plus VEOF into the PTY when dropped, which makes the shell
    /// child exit. Holding it here keeps the child alive until the tab itself is dropped, so `is_alive()` stays
    /// deterministic for the whole test instead of racing the shell's exit.
    struct RecordingWriter {
        recorded: Arc<Mutex<Vec<u8>>>,
        _pty_writer: Box<dyn Write + Send>,
    }

    impl Write for RecordingWriter {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.recorded.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    fn attach_recorder(tab: &mut TerminalTab) -> Arc<Mutex<Vec<u8>>> {
        let rec = Arc::new(Mutex::new(Vec::new()));
        let pty_writer = std::mem::replace(&mut tab.writer, Box::new(std::io::sink()));
        tab.writer = Box::new(RecordingWriter { recorded: Arc::clone(&rec), _pty_writer: pty_writer });
        rec
    }

    fn injection_doc(frame_name: &str, comment_text: &str) -> (Document, NodeId, NodeId) {
        let mut doc = Document::new("p0");
        let page = doc.add_page("Page");
        let fid = doc.alloc_id();
        let mut frame = Node::frame(fid, designos_doc::Rect::new(0.0, 0.0, 200.0, 100.0));
        frame.name = frame_name.to_string();
        doc.insert(page, frame).unwrap();
        doc.comments.push(CommentThread {
            id: "t1".into(),
            anchor: CommentAnchor { page, node: Some(fid), offset: designos_doc::Vec2::new(1.0, 1.0) },
            messages: vec![Comment {
                id: "m1".into(),
                author: "qa".into(),
                author_id: None,
                author_color: None,
                text: comment_text.into(),
                created: "2026-10-16T00:00:00Z".into(),
                edited: None,
            }],
            resolved: false,
        });
        (doc, page, fid)
    }

    const MALICIOUS_SPEC: &str = "```\n$(touch x)\n\x1b[201~";

    #[test]
    fn shell_tab_receives_zero_bytes_from_copy_and_send_selection() {
        let mut st = TerminalState::default();
        st.spawn_tab(AgentCliPreset::Shell, None).expect("spawn shell");
        let rec = attach_recorder(st.active_tab_mut().unwrap());
        st.send_copy_for_ai(MALICIOUS_SPEC);

        let (doc, page, fid) = injection_doc("Card-$(touch x)", "ok\ntouch x #");
        let _ = export_selection_and_comments_to_pty(&mut st, &doc, page, &[fid], None).unwrap();

        assert!(rec.lock().unwrap().is_empty(), "shell tab must receive zero bytes from copy/export");
        assert_eq!(st.tabs.len(), 1);
        for mut tab in st.tabs.drain(..) {
            let _ = tab.child.kill();
        }
    }

    #[test]
    fn copy_for_ai_never_spawns_a_tab() {
        let mut st = TerminalState::default();
        st.send_copy_for_ai(MALICIOUS_SPEC);
        assert!(st.tabs.is_empty());
    }

    #[test]
    fn agent_tab_gets_single_line_bracketed_paste() {
        let mut st = TerminalState::default();
        st.spawn_tab(AgentCliPreset::Shell, None).expect("spawn shell");
        {
            let tab = st.active_tab_mut().unwrap();
            tab.spawn_preset = AgentCliPreset::Claude;
        }
        let rec = attach_recorder(st.active_tab_mut().unwrap());
        st.send_copy_for_ai(MALICIOUS_SPEC);

        let bytes = rec.lock().unwrap().clone();
        assert!(bytes.starts_with(b"\x1b[200~"), "paste must start with bracketed-paste begin");
        assert!(bytes.ends_with(b"\x1b[201~"), "paste must end with bracketed-paste end");
        assert!(!bytes.ends_with(b"\r"), "must not auto-Enter");
        let begin = b"\x1b[200~".len();
        let end = bytes.len() - b"\x1b[201~".len();
        let inner = String::from_utf8_lossy(&bytes[begin..end]);
        assert!(!inner.contains('\r'));
        assert!(!inner.contains('\n'));
        assert!(!inner.contains('\x1b'));
        assert!(inner.chars().all(|c| !c.is_control()), "inner paste must contain no control chars: {inner:?}");

        for mut tab in st.tabs.drain(..) {
            let _ = tab.child.kill();
        }
    }

    #[test]
    fn sanitize_pty_paste_text_strips_controls() {
        assert_eq!(sanitize_pty_paste_text("a\rb\nc\td"), "a b c d");
        let stripped = sanitize_pty_paste_text("x\x1by\u{9b}z\x07w\u{7f}q");
        assert_eq!(stripped, "xyzwq");
        let escaped = sanitize_pty_paste_text("\x1b[201~");
        assert_eq!(escaped, "[201~");
        assert!(!escaped.contains('\x1b'));
        assert_ne!(escaped, "\x1b[201~");
        let seps = sanitize_pty_paste_text("keep\u{2028}me\u{2029}please");
        assert_eq!(seps, "keepmeplease");
    }

    fn wait_shell_ready(st: &TerminalState) {
        let ready_deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while std::time::Instant::now() < ready_deadline {
            if let Some(tab) = st.active_tab() {
                if let Ok(p) = tab.parser.lock() {
                    if !p.screen().contents().trim().is_empty() {
                        return;
                    }
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(40));
        }
    }

    fn poll_full_3s() {
        let poll_deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        while std::time::Instant::now() < poll_deadline {
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    }

    fn kill_tabs(st: &mut TerminalState) {
        for mut tab in st.tabs.drain(..) {
            let _ = tab.child.kill();
        }
    }

    fn pty_fixture_path(name: &str) -> String {
        let dir = std::env::temp_dir();
        let _ = std::fs::create_dir_all(&dir);
        dir.join(name).to_string_lossy().replace('\\', "/")
    }

    #[test]
    fn pty_fixture_path_is_writable_on_host_temp_dir() {
        let pid = std::process::id();
        let probe = pty_fixture_path(&format!("dos-p0-probe-{pid}.txt"));
        std::fs::write(&probe, "ok").expect("host temp_dir fixture path must be writable on all OSes");
        assert_eq!(std::fs::read_to_string(&probe).unwrap(), "ok");
        let _ = std::fs::remove_file(&probe);
    }

    #[test]
    fn pty_injection_fixture_real_shell() {
        let pid = std::process::id();
        let md = pty_fixture_path(&format!("dos-p0-fixture-{pid}-md"));
        let mcp = pty_fixture_path(&format!("dos-p0-fixture-{pid}-mcp"));
        let nl = pty_fixture_path(&format!("dos-p0-fixture-{pid}-nl"));
        let trunc = pty_fixture_path(&format!("dos-p0-fixture-{pid}-trunc.txt"));

        // md payload: copy-for-ai markdown with a trunc-file overwrite + touch.
        {
            let _ = std::fs::remove_file(&md);
            std::fs::write(&trunc, "keep").expect("write trunc fixture");
            let mut st = TerminalState::default();
            st.spawn_tab(AgentCliPreset::Shell, None).expect("spawn shell");
            wait_shell_ready(&st);
            st.send_copy_for_ai(&format!("# T\n\n> {trunc}\n\nRun `touch {md}` now.\n\n    touch {md}\n"));
            poll_full_3s();
            let md_exists = Path::new(&md).exists();
            let trunc_contents = std::fs::read_to_string(&trunc).unwrap_or_default();
            let _ = std::fs::remove_file(&md);
            let _ = std::fs::remove_file(&trunc);
            kill_tabs(&mut st);
            assert!(!md_exists, "md marker {md} must not exist");
            assert_eq!(trunc_contents, "keep", "trunc file must still contain keep");
        }

        // mcp payload: copy-for-ai @designos URL with command substitution.
        {
            let _ = std::fs::remove_file(&mcp);
            let mut st = TerminalState::default();
            st.spawn_tab(AgentCliPreset::Shell, None).expect("spawn shell");
            wait_shell_ready(&st);
            st.send_copy_for_ai(&format!("Implement this design from design:os.\n@designos://design/1/Card-$(touch${{IFS}}{mcp})"));
            poll_full_3s();
            let mcp_exists = Path::new(&mcp).exists();
            let _ = std::fs::remove_file(&mcp);
            kill_tabs(&mut st);
            assert!(!mcp_exists, "mcp marker {mcp} must not exist");
        }

        // nl payload: export_selection pin-comment with a trailing touch.
        {
            let _ = std::fs::remove_file(&nl);
            let mut st = TerminalState::default();
            st.spawn_tab(AgentCliPreset::Shell, None).expect("spawn shell");
            wait_shell_ready(&st);
            let (doc, page, fid) = injection_doc("Card", &format!("looks good\ntouch {nl} #"));
            let _ = export_selection_and_comments_to_pty(&mut st, &doc, page, &[fid], None);
            poll_full_3s();
            let nl_exists = Path::new(&nl).exists();
            let _ = std::fs::remove_file(&nl);
            kill_tabs(&mut st);
            assert!(!nl_exists, "nl marker {nl} must not exist");
        }
    }

    // ===================== QA P0 independent attack fixtures (QA worktree only, not Dev code) =====================
    // Guard: agent-preset tabs exec `claude ... || exec $SHELL -l`. These tests only run when HOME is the QA scratch
    // home whose ~/.local/bin/claude is a stub that exits 1 (simulated agent crash) -> never launches a real agent.
    fn qa_guard() -> Option<std::path::PathBuf> {
        let home = std::env::var("HOME").unwrap_or_default();
        if !home.contains("/qa-staging/p0-pty-qa/scratch/home") {
            eprintln!("QA guard: skip (HOME={home})");
            return None;
        }
        let d = std::path::PathBuf::from(std::env::var("QA_MARK_DIR").expect("QA_MARK_DIR"));
        let _ = std::fs::create_dir_all(&d);
        Some(d)
    }
    fn qa_screen(st: &TerminalState) -> String {
        st.active_tab().and_then(|t| t.parser.lock().ok().map(|p| p.screen().contents())).unwrap_or_default()
    }
    fn qa_wait_screen(st: &TerminalState, needle: &str, secs: u64) -> bool {
        let dl = std::time::Instant::now() + std::time::Duration::from_secs(secs);
        while std::time::Instant::now() < dl {
            if qa_screen(st).contains(needle) {
                return true;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        false
    }
    fn qa_wait_file(p: &Path, secs: u64) -> bool {
        let dl = std::time::Instant::now() + std::time::Duration::from_secs(secs);
        while std::time::Instant::now() < dl {
            if p.exists() {
                return true;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        p.exists()
    }
    /// (a)+(e): agent tab whose CLI exited non-zero falls back to `exec $SHELL -l`. spawn_preset stays Claude, so
    /// paste_into_agent_tab writes the attacker layer name. Then the app's NEXT send_line (chat prompt or AI action)
    /// appends + presses Enter itself. No user keystroke in the terminal.
    fn qa_fallback_chain(next: &str) -> (bool, bool, bool, String) {
        let Some(d) = qa_guard() else { return (false, false, false, String::new()) };
        let marker = d.join(format!("pwn-fallback-{next}"));
        let _ = std::fs::remove_file(&marker);
        let mut st = TerminalState::default();
        st.spawn_tab(AgentCliPreset::Claude, None).expect("spawn claude preset");
        let fell_back = qa_wait_screen(&st, "QA-STUB-CLAUDE-EXIT", 8);
        let exit_deadline = std::time::Instant::now() + std::time::Duration::from_secs(8);
        while std::time::Instant::now() < exit_deadline {
            if let Some(tab) = st.active_tab_mut() {
                if !tab.is_alive() {
                    break;
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        assert!(!st.active_tab_mut().unwrap().is_alive(), "precondition: stub agent exited"); // r2-adapted
        let evil = format!("x'; touch {} #", marker.display());
        let (doc, page, fid) = injection_doc(&evil, "please review");
        let _ = export_selection_and_comments_to_pty(&mut st, &doc, page, &[fid], None).unwrap();
        let pasted = st.status_msg.clone().unwrap_or_default().starts_with("Sent ");
        let after_paste_only = qa_wait_file(&marker, 2);
        match next {
            "chat" => st.submit_chat_prompt("make the button blue", None),
            "ai_action" => st.dispatch_figma_ai_prompt("Make Design", "pricing card", None),
            _ => {}
        }
        let after_next = qa_wait_file(&marker, 4);
        let screen = qa_screen(&st);
        std::fs::write(d.join(format!("screen-fallback-{next}.txt")), &screen).ok();
        for mut tab in st.tabs.drain(..) {
            let _ = tab.child.kill();
        }
        let _ = fell_back;
        (pasted, after_paste_only, after_next, screen)
    }
    #[test]
    fn qa_p0_fallback_shell_paste_alone_is_inert() {
        if qa_guard().is_none() {
            return;
        }
        let (pasted, after_paste_only, _, _) = qa_fallback_chain("none");
        assert!(!pasted, "dead agent tab must not receive paste"); // r2-adapted
        assert!(!after_paste_only, "paste alone executed in fallback shell");
    }
    #[test]
    fn qa_p0_fallback_shell_paste_then_chat_prompt_must_not_execute() {
        if qa_guard().is_none() {
            return;
        }
        let (pasted, after_paste_only, after_next, screen) = qa_fallback_chain("chat");
        assert!(!pasted, "dead agent tab must not receive paste"); // r2-adapted
        assert!(!after_paste_only);
        assert!(!after_next, "BYPASS: layer name executed in fallback login shell after next chat prompt. screen:\n{screen}");
    }
    #[test]
    fn qa_p0_fallback_shell_paste_then_ai_action_must_not_execute() {
        if qa_guard().is_none() {
            return;
        }
        let (pasted, after_paste_only, after_next, screen) = qa_fallback_chain("ai_action");
        assert!(!pasted, "dead agent tab must not receive paste"); // r2-adapted
        assert!(!after_paste_only);
        assert!(!after_next, "BYPASS: layer name executed in fallback login shell after next AI action. screen:\n{screen}");
    }
    /// (f) relabel race: Shell tab relabelled to Claude by set_active_runtime / submit_chat_prompt still gets 0 bytes.
    #[test]
    fn qa_p0_relabelled_shell_tab_gets_zero_bytes() {
        let mut st = TerminalState::default();
        st.spawn_tab(AgentCliPreset::Shell, None).expect("spawn shell");
        st.set_active_runtime(AgentCliPreset::Claude, None);
        assert_eq!(st.active_tab().unwrap().preset, AgentCliPreset::Claude, "relabel happened");
        let rec = attach_recorder(st.active_tab_mut().unwrap());
        st.send_copy_for_ai("x\n$(touch /tmp/qa-never)\n");
        let (doc, page, fid) = injection_doc("x'; touch /tmp/qa-never #", "c\ntouch /tmp/qa-never");
        let _ = export_selection_and_comments_to_pty(&mut st, &doc, page, &[fid], None).unwrap();
        assert!(rec.lock().unwrap().is_empty());
        for mut tab in st.tabs.drain(..) {
            let _ = tab.child.kill();
        }
    }
    /// (c) sanitiser matrix on the real bracketed_paste_bytes.
    #[test]
    fn qa_p0_sanitiser_matrix() {
        let mut nasty = String::from("A\x1b[201~B\x1b[200~C\u{2028}D\u{2029}E\rF\nG\0H\x1b]0;t\x07I\x1bP+q\x1b\\J\x1b[2J\x7fK\u{85}L\u{9b}31mM\t");
        for c in 0x80u32..=0x9f {
            nasty.push(char::from_u32(c).unwrap());
        }
        for c in 0x00u32..=0x1f {
            nasty.push(char::from_u32(c).unwrap());
        }
        nasty.push_str("\u{feff}\u{202e}Z");
        let b = bracketed_paste_bytes(&nasty);
        assert!(b.starts_with(b"\x1b[200~") && b.ends_with(b"\x1b[201~"));
        let inner = &b[6..b.len() - 6];
        assert!(!inner.iter().any(|&x| x < 0x20 || x == 0x7f), "C0/DEL byte survived: {inner:?}");
        let s = std::str::from_utf8(inner).unwrap();
        assert!(!s.chars().any(|c| ('\u{80}'..='\u{9f}').contains(&c)), "C1 survived");
        assert!(!s.contains('\u{2028}') && !s.contains('\u{2029}'));
        assert!(!inner.windows(2).any(|w| w[0] == 0xc2 && (0x80..=0x9f).contains(&w[1])), "UTF-8 C1 survived");
        assert_eq!(b.iter().filter(|&&x| x == 0x1b).count(), 2, "only the app's own 200~/201~ ESCs");
        assert!(!b.contains(&b'\r') && !b.contains(&b'\n'));
        std::fs::write("/workspace/qa-staging/p0-pty-qa/sanitiser-matrix-output.txt", format!("{:?}\n", String::from_utf8_lossy(&b))).ok();
    }
    /// (d) send_copy_for_ai into an agent tab: only the fixed file-reference sentence, never payload text.
    #[test]
    fn qa_p0_copy_for_ai_agent_tab_carries_no_payload() {
        let mut st = TerminalState::default();
        st.spawn_tab(AgentCliPreset::Shell, None).expect("spawn");
        st.active_tab_mut().unwrap().spawn_preset = AgentCliPreset::Claude;
        let rec = attach_recorder(st.active_tab_mut().unwrap());
        st.send_copy_for_ai("QA-PAYLOAD-SENTINEL $(touch /tmp/qa-never)\n");
        let b = rec.lock().unwrap().clone();
        let s = String::from_utf8_lossy(&b).to_string();
        assert!(!s.contains("QA-PAYLOAD-SENTINEL") && !s.contains("$("), "payload leaked: {s:?}");
        let expected = format!(
            "\x1b[200~Read the design:os spec excerpt at {} and follow the request inside it.\x1b[201~",
            st.session_dir.join("copy-for-ai.md").display()
        );
        assert_eq!(s, expected);
        for mut tab in st.tabs.drain(..) {
            let _ = tab.child.kill();
        }
    }
    /// (e) zsh variant: pin-comment `see $(touch M)` (unquoted field) + next chat prompt, real send path.
    #[test]
    fn qa_p0_fallback_comment_subst_then_chat_must_not_execute() {
        let Some(d) = qa_guard() else { return };
        let marker = d.join("pwn-fallback-comment-subst");
        let _ = std::fs::remove_file(&marker);
        let mut st = TerminalState::default();
        st.spawn_tab(AgentCliPreset::Claude, None).expect("spawn claude preset");
        let exit_deadline = std::time::Instant::now() + std::time::Duration::from_secs(8);
        while std::time::Instant::now() < exit_deadline {
            if let Some(tab) = st.active_tab_mut() {
                if !tab.is_alive() {
                    break;
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        } // r2-adapted
        let (doc, page, fid) = injection_doc("Card", &format!("see $(touch {})", marker.display()));
        let _ = export_selection_and_comments_to_pty(&mut st, &doc, page, &[fid], None).unwrap();
        let after_paste_only = qa_wait_file(&marker, 2);
        st.submit_chat_prompt("make the button blue", None);
        let after_next = qa_wait_file(&marker, 4);
        std::fs::write(d.join("screen-fallback-comment-subst.txt"), qa_screen(&st)).ok();
        for mut tab in st.tabs.drain(..) {
            let _ = tab.child.kill();
        }
        assert!(!after_paste_only);
        assert!(!after_next, "BYPASS: pin comment $(...) executed in fallback login shell after next chat prompt");
    }

    fn spawn_stub_exiting_agent_into(st: &mut TerminalState) {
        let id = st.next_tab_id.max(1);
        st.next_tab_id = id + 1;
        let _ = std::fs::create_dir_all(&st.session_dir);
        st.provision_session_agent_files(None);
        let env = st.session_env();
        let tab = TerminalTab::spawn_exiting_stub_agent(id, 28, 100, None, Some(&env)).expect("spawn stub agent");
        st.tabs.push(tab);
        st.active_tab = st.tabs.len() - 1;
    }

    fn wait_until_tab_exits(st: &mut TerminalState, secs: u64) -> bool {
        let dl = std::time::Instant::now() + std::time::Duration::from_secs(secs);
        while std::time::Instant::now() < dl {
            if let Some(tab) = st.active_tab_mut() {
                if !tab.is_alive() {
                    return true;
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        st.active_tab_mut().map(|t| !t.is_alive()).unwrap_or(false)
    }

    #[test]
    fn export_selection_pastes_fixed_sentence_only() {
        let mut st = TerminalState::default();
        st.spawn_tab(AgentCliPreset::Shell, None).expect("spawn");
        st.active_tab_mut().unwrap().spawn_preset = AgentCliPreset::Claude;
        let rec = attach_recorder(st.active_tab_mut().unwrap());
        let node_name = "x'; touch /tmp/never #";
        let comment = "see $(touch /tmp/never)";
        let (doc, page, fid) = injection_doc(node_name, comment);
        let _ = export_selection_and_comments_to_pty(&mut st, &doc, page, &[fid], None).unwrap();

        let bytes = rec.lock().unwrap().clone();
        let sentence =
            format!("Read the design:os selection at {} (node spec plus open pin-comments under \"designos_context\").", st.selection_spec_path.display());
        let mut expected = b"\x1b[200~".to_vec();
        expected.extend_from_slice(sentence.as_bytes());
        expected.extend_from_slice(b"\x1b[201~");
        assert_eq!(bytes, expected, "PTY must receive only the fixed file-reference sentence");
        let pasted = String::from_utf8_lossy(&bytes);
        assert!(!pasted.contains(node_name), "node name must not reach the PTY: {pasted:?}");
        assert!(!pasted.contains(comment), "comment must not reach the PTY: {pasted:?}");

        let json = std::fs::read_to_string(&st.selection_spec_path).expect("selection.spec.json");
        let value: serde_json::Value = serde_json::from_str(&json).expect("json");
        let ctx = value.get("designos_context").expect("designos_context");
        assert_eq!(ctx["node_name"].as_str(), Some(node_name));
        let comments = ctx["open_comments"].as_array().expect("open_comments");
        assert!(comments.iter().any(|c| c.as_str().is_some_and(|s| s.contains(comment))), "open_comments must contain the pin comment: {comments:?}");

        kill_tabs(&mut st);
    }

    #[test]
    fn agent_shell_cmd_has_no_shell_fallback() {
        let cmd = build_agent_shell_cmd("/bin/bash", "claude --x");
        let args: Vec<String> = cmd.get_argv().iter().map(|a| a.to_string_lossy().into_owned()).collect();
        let joined = args.join(" ");
        assert!(!joined.contains("exec \"/bin/bash\""), "no shell exec fallback: {joined}");
        assert!(!joined.contains("|| exec"), "no || exec fallback: {joined}");

        let agy = build_agent_shell_cmd("/bin/bash", AgentCliPreset::Antigravity.cli_launch_command());
        let agy_args: Vec<String> = agy.get_argv().iter().map(|a| a.to_string_lossy().into_owned()).collect();
        let agy_joined = agy_args.join(" ");
        assert!(agy_joined.contains("agy --dangerously-skip-permissions"), "Antigravity runs agy: {agy_joined}");
        if !cfg!(windows) {
            assert!(agy_joined.contains("exec agy"), "Antigravity execs agy so the tab ends with it: {agy_joined}");
        }
        assert!(!agy_joined.contains("antigravity"), "no antigravity IDE fallback: {agy_joined}");
        assert!(!agy_joined.contains("|| exec"), "Antigravity must not exec a login shell: {agy_joined}");
        assert!(!agy_joined.contains("exec \"/bin/bash\""));
    }

    #[test]
    fn dead_agent_tab_gets_no_input_and_shows_notice() {
        let pid = std::process::id();
        let marker = pty_fixture_path(&format!("dos-p0-r2-dead-{pid}"));
        let _ = std::fs::remove_file(&marker);

        let mut st = TerminalState::default();
        spawn_stub_exiting_agent_into(&mut st);
        assert!(wait_until_tab_exits(&mut st, 8), "stub agent must exit");
        assert!(!st.active_tab_mut().unwrap().is_alive());

        let evil = format!("x'; touch {marker} #");
        let (doc, page, fid) = injection_doc(&evil, &format!("see $(touch {marker})"));
        let _ = export_selection_and_comments_to_pty(&mut st, &doc, page, &[fid], None).unwrap();
        st.submit_chat_prompt(&format!("touch {marker}"), None);
        st.dispatch_figma_ai_prompt("Make Design", &format!("touch {marker}"), None);

        let wait_dl = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while std::time::Instant::now() < wait_dl {
            if Path::new(&marker).exists() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        assert!(!Path::new(&marker).exists(), "dead agent tab must not execute marker {marker}");

        let _ = st.poll_live_agent_bridge();
        let screen = st.active_tab().and_then(|t| t.parser.lock().ok().map(|p| p.screen().contents())).unwrap_or_default();
        assert!(screen.contains("Agent exited"), "exit notice missing from screen: {screen:?}");

        let _ = std::fs::remove_file(&marker);
        kill_tabs(&mut st);
    }

    #[test]
    fn chat_prompt_clears_pending_line_before_enter() {
        let pid = std::process::id();
        let m_chat = pty_fixture_path(&format!("dos-p0-r2-pending-{pid}-chat"));
        let m_ai = pty_fixture_path(&format!("dos-p0-r2-pending-{pid}-ai"));
        let _ = std::fs::remove_file(&m_chat);
        let _ = std::fs::remove_file(&m_ai);

        let mut st = TerminalState::default();
        st.spawn_tab(AgentCliPreset::Shell, None).expect("spawn shell");
        wait_shell_ready(&st);

        if let Some(tab) = st.active_tab_mut() {
            tab.write_bytes(format!("touch {m_chat} #").as_bytes());
        }
        st.submit_chat_prompt("hello", None);
        poll_full_3s();
        assert!(!Path::new(&m_chat).exists(), "pending line must be cleared before chat Enter; marker {m_chat} exists");

        if let Some(tab) = st.active_tab_mut() {
            tab.write_bytes(format!("touch {m_ai} #").as_bytes());
        }
        st.dispatch_figma_ai_prompt("Make Design", "pricing card", None);
        poll_full_3s();
        assert!(!Path::new(&m_ai).exists(), "pending line must be cleared before AI-action Enter; marker {m_ai} exists");

        let _ = std::fs::remove_file(&m_chat);
        let _ = std::fs::remove_file(&m_ai);
        kill_tabs(&mut st);
    }

    #[test]
    fn submit_chat_prompt_sanitizes_control_chars() {
        let mut st = TerminalState::default();
        st.spawn_tab(AgentCliPreset::Shell, None).expect("spawn shell");
        let rec = attach_recorder(st.active_tab_mut().unwrap());
        st.submit_chat_prompt("hello\rworld\x07", None);
        let bytes = rec.lock().unwrap().clone();
        assert!(!bytes.contains(&0x07), "BEL must not reach the PTY: {bytes:?}");
        let cr_positions: Vec<usize> = bytes.iter().enumerate().filter(|(_, b)| **b == b'\r').map(|(i, _)| i).collect();
        assert_eq!(cr_positions, vec![bytes.len().saturating_sub(1)], "no '\\r' except the final Enter: {bytes:?}");
        kill_tabs(&mut st);
    }

    /// Every flag `agy --help` lists in Antigravity CLI 1.4.2 (`agy mcp` is a subcommand, not a flag).
    const AGY_1_4_2_FLAGS: &[&str] = &[
        "--add-dir",
        "--agent",
        "-c",
        "--continue",
        "--conversation",
        "--dangerously-skip-permissions",
        "--disable-slash-commands",
        "--effort",
        "-i",
        "--input-format",
        "--json-schema",
        "--log-file",
        "--mode",
        "--model",
        "--new-project",
        "--output-format",
        "-p",
        "--print",
        "--print-timeout",
        "--project",
        "--prompt",
        "--prompt-interactive",
        "--remote-control",
        "--sandbox",
    ];

    /// Every option `grok --help` lists in Grok Build 1.0.62.
    const GROK_1_0_62_FLAGS: &[&str] = &[
        "--agent",
        "--agents",
        "--allow",
        "--always-approve",
        "-c",
        "--continue",
        "--cwd",
        "--debug",
        "--debug-file",
        "--deny",
        "--disable-web-search",
        "--disallowed-tools",
        "--fork-session",
        "--fullscreen",
        "-h",
        "--help",
        "--include-partial-messages",
        "--json-schema",
        "--leader-socket",
        "-m",
        "--model",
        "--max-turns",
        "--minimal",
        "--no-alt-screen",
        "--no-plan",
        "--no-subagents",
        "--oauth",
        "--output-format",
        "-p",
        "--single",
        "--permission-mode",
        "--prompt-file",
        "--prompt-json",
        "-r",
        "--resume",
        "--reasoning-effort",
        "--restore-code",
        "--rules",
        "-s",
        "--session-id",
        "--sandbox",
        "--system-prompt-override",
        "--tools",
        "-v",
        "--version",
        "--verbatim",
        "-w",
        "--worktree",
        "--worktree-ref",
    ];

    fn unknown_flags(command: &str, allowed: &[&str]) -> Vec<String> {
        command
            .split_whitespace()
            .map(|t| t.trim_matches(|c| c == '"' || c == '\''))
            .filter(|t| t.starts_with('-'))
            .filter(|t| !allowed.contains(t))
            .map(str::to_string)
            .collect()
    }

    fn unknown_agy_flags(command: &str) -> Vec<String> {
        unknown_flags(command, AGY_1_4_2_FLAGS)
    }

    #[test]
    fn antigravity_launch_uses_only_flags_agy_1_4_2_accepts() {
        let interactive = AgentCliPreset::Antigravity.cli_launch_command();
        assert_eq!(unknown_agy_flags(interactive), Vec::<String>::new(), "tab launch: {interactive}");
        assert!(!interactive.contains("--mcp-config"), "agy has no --mcp-config: {interactive}");
        assert!(interactive.contains("--add-dir \"$DESIGNOS_SESSION_DIR\""), "session plugin dir must be added: {interactive}");

        let live_spec = std::path::Path::new("/tmp/designos-session-x/live-agent-out.spec.json");
        let sel_json = std::path::Path::new("/tmp/designos-session-x/selection.spec.json");
        let one_shot = build_real_cli_command(AgentCliPreset::Antigravity, "Check parity", live_spec, sel_json);
        assert_eq!(unknown_agy_flags(&one_shot), Vec::<String>::new(), "one-shot launch: {one_shot}");
        assert!(!one_shot.contains("--mcp-config"), "agy has no --mcp-config: {one_shot}");
        assert!(one_shot.contains("--add-dir '/tmp/designos-session-x'"), "session plugin dir must be added: {one_shot}");
    }

    #[test]
    fn antigravity_session_gets_a_workspace_plugin_carrying_the_designos_mcp_server() {
        let mut st = TerminalState::for_tab(731);
        let mut doc = Document::new("Agy Plugin File");
        let page = doc.add_page("Page 1");
        st.sync_live_file_context(&doc, page, &[], Some(std::path::Path::new("/tmp/agy-plugin-file.designos")));

        let plugin_dir = st.session_dir.join(".agents").join("plugins").join("designos");
        let manifest: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(plugin_dir.join("plugin.json")).expect("plugin.json")).expect("parse plugin.json");
        assert_eq!(manifest["name"].as_str(), Some("designos"));

        let plugin_mcp = std::fs::read_to_string(plugin_dir.join("mcp_config.json")).expect("plugin mcp_config.json");
        let shared_mcp = std::fs::read_to_string(st.session_dir.join("mcp-config.json")).expect("mcp-config.json");
        assert_eq!(plugin_mcp, shared_mcp, "plugin and --mcp-config consumers must see the same server");
        let parsed: serde_json::Value = serde_json::from_str(&plugin_mcp).expect("parse");
        assert!(parsed["mcpServers"]["designos"]["command"].as_str().is_some());
        assert_eq!(parsed["mcpServers"]["designos"]["env"]["DESIGNOS_FILE"].as_str(), Some("/tmp/agy-plugin-file.designos"));
    }

    #[test]
    fn grok_launch_uses_only_flags_grok_1_0_62_accepts() {
        let interactive = AgentCliPreset::Grok.cli_launch_command();
        assert_eq!(unknown_flags(interactive, GROK_1_0_62_FLAGS), Vec::<String>::new(), "tab launch: {interactive}");
        assert!(!interactive.contains("--mcp-config"), "grok has no --mcp-config: {interactive}");
        assert!(interactive.contains("--cwd \"$DESIGNOS_SESSION_DIR\""), "grok reads the session project config from its cwd: {interactive}");

        let live_spec = std::path::Path::new("/tmp/designos-session-x/live-agent-out.spec.json");
        let sel_json = std::path::Path::new("/tmp/designos-session-x/selection.spec.json");
        let one_shot = build_real_cli_command(AgentCliPreset::Grok, "Build card", live_spec, sel_json);
        assert_eq!(unknown_flags(&one_shot, GROK_1_0_62_FLAGS), Vec::<String>::new(), "one-shot launch: {one_shot}");
        assert!(!one_shot.contains("--mcp-config"), "grok has no --mcp-config: {one_shot}");
        assert!(one_shot.contains("--cwd '/tmp/designos-session-x'"), "{one_shot}");
        assert!(one_shot.contains("GROK_FOLDER_TRUST='0'"), "{one_shot}");
    }

    #[test]
    fn grok_session_gets_a_project_config_with_the_designos_mcp_server() {
        let mut st = TerminalState::for_tab(732);
        let mut doc = Document::new("Grok Project Config File");
        let page = doc.add_page("Page 1");
        st.sync_live_file_context(&doc, page, &[], Some(std::path::Path::new("/tmp/grok \"quoted\" file.designos")));

        let toml = std::fs::read_to_string(st.session_dir.join(".grok").join("config.toml")).expect(".grok/config.toml");
        assert!(toml.contains("[mcp_servers.designos]\n"), "{toml}");
        assert!(toml.contains("[mcp_servers.designos.env]\n"), "{toml}");
        assert!(toml.contains("command = \""), "{toml}");
        assert!(toml.contains("args = [\"--workspace\", \""), "{toml}");
        assert!(toml.contains(&format!("DESIGNOS_SESSION_DIR = {}", serde_json::to_string(&st.session_dir.display().to_string()).unwrap())), "{toml}");
        assert!(toml.contains(r#"DESIGNOS_FILE = "/tmp/grok \"quoted\" file.designos""#), "{toml}");
    }

    #[test]
    fn grok_pty_env_disables_folder_trust_only_for_grok() {
        let st = TerminalState::for_tab(733);
        let env = st.session_env();
        let grok_env = |preset| {
            let mut cmd = CommandBuilder::new("true");
            configure_pty_env_with_session(&mut cmd, preset, None, Some(&env));
            cmd.get_env("GROK_FOLDER_TRUST").map(|v| v.to_string_lossy().into_owned())
        };
        assert_eq!(grok_env(AgentCliPreset::Grok).as_deref(), Some("0"));
        assert_eq!(grok_env(AgentCliPreset::Claude), None);
        assert_eq!(grok_env(AgentCliPreset::Antigravity), None);
    }

    /// Quoted items of the TOML list that follows `key=[` in a Codex `-c` override.
    fn codex_list_override(launch: &str, key: &str) -> Vec<String> {
        let start = launch.find(&format!("{key}=[")).unwrap_or_else(|| panic!("no {key} override in: {launch}")) + key.len() + 2;
        let end = start + launch[start..].find(']').expect("closing bracket");
        launch[start..end].split(',').map(|item| item.trim().trim_matches(|c| c == '"' || c == '\'').to_string()).filter(|item| !item.is_empty()).collect()
    }

    #[test]
    fn codex_tab_forwards_the_session_env_and_workspace_to_designos_mcp() {
        // Codex starts an MCP child with a scrubbed env: only `env` / `env_vars` entries reach it.
        // Verified against codex 0.163.0 with a stub server that dumps its environment.
        let launch = AgentCliPreset::Codex.cli_launch_command();
        let forwarded = codex_list_override(launch, "mcp_servers.designos.env_vars");
        let args = codex_list_override(launch, "mcp_servers.designos.args");
        assert_eq!(args.first().map(String::as_str), Some("--workspace"), "{launch}");
        assert!(launch.contains("$DESIGNOS_WORKSPACE"), "workspace arg must come from the session env: {launch}");

        let mut st = TerminalState::for_tab(734);
        let mut doc = Document::new("Codex Env File");
        let page = doc.add_page("Page 1");
        let file = std::env::temp_dir().join("codex-env-file.designos");
        st.sync_live_file_context(&doc, page, &[], Some(&file));
        let env = st.session_env();
        let mut cmd = CommandBuilder::new("true");
        configure_pty_env_with_session(&mut cmd, AgentCliPreset::Codex, Some(&file), Some(&env));
        let pty_env: std::collections::HashMap<String, String> = cmd.iter_extra_env_as_str().map(|(k, v)| (k.to_string(), v.to_string())).collect();

        // Every variable designos-mcp reads to find the live session must be forwarded.
        for needed in [
            "DESIGNOS_SESSION_DIR",
            "DESIGNOS_LIVE_CANVAS",
            "DESIGNOS_LIVE_DOC_SYNC",
            "DESIGNOS_LIVE_DOC_PATCH",
            "DESIGNOS_LIVE_SPEC",
            "DESIGNOS_CONTEXT_JSON",
            "DESIGNOS_TARGET_PAGE_ID",
        ] {
            assert!(forwarded.iter().any(|f| f == needed), "{needed} not forwarded: {forwarded:?}");
            assert!(pty_env.contains_key(needed), "{needed} missing from the PTY env");
        }
        assert_eq!(pty_env.get("DESIGNOS_WORKSPACE").map(String::as_str), file.parent().and_then(|p| p.to_str()), "workspace is the file's folder");

        // Stub Codex: starts the MCP child with only the forwarded vars, as the real one does.
        let patch = std::path::PathBuf::from(&pty_env["DESIGNOS_LIVE_DOC_PATCH"]);
        let _ = std::fs::remove_file(&patch);
        let status = std::process::Command::new("sh")
            .arg("-c")
            .arg(r#"printf '{"edit":1}\n' >> "$DESIGNOS_LIVE_DOC_PATCH""#)
            .env_clear()
            .envs(forwarded.iter().filter_map(|k| pty_env.get(k).map(|v| (k.clone(), v.clone()))))
            .status()
            .expect("run stub mcp child");
        assert!(status.success());
        assert_eq!(std::fs::read_to_string(&patch).expect("doc-patch.jsonl written"), "{\"edit\":1}\n");
    }

    #[test]
    fn pty_cwd_is_never_root_and_agent_clis_start_in_the_session_dir() {
        let session = std::path::Path::new("/tmp/designos-sessions/cwd-case");
        let files_folder = std::path::PathBuf::from("/Users/someone/designs");
        // Agent CLIs discover project config, skills and AGENTS.md from their cwd: the session dir.
        for preset in [AgentCliPreset::Claude, AgentCliPreset::Codex, AgentCliPreset::Antigravity, AgentCliPreset::Grok] {
            assert_eq!(resolve_pty_cwd(preset, Some(session), files_folder.clone()), session, "{preset:?}");
        }
        // A shell tab opens in the file's folder.
        assert_eq!(resolve_pty_cwd(AgentCliPreset::Shell, Some(session), files_folder.clone()), files_folder);
        // A bundle launched from Finder has cwd `/`: never use it.
        let root = std::path::PathBuf::from("/");
        assert_eq!(resolve_pty_cwd(AgentCliPreset::Shell, Some(session), root.clone()), session);
        assert_eq!(resolve_pty_cwd(AgentCliPreset::Claude, None, root.clone()), std::env::temp_dir());
        assert_eq!(resolve_pty_cwd(AgentCliPreset::Shell, None, root), std::env::temp_dir());
    }

    #[test]
    fn spawned_agent_env_sets_cwd_to_the_session_dir() {
        let st = TerminalState::for_tab(735);
        let env = st.session_env();
        let mut agent = CommandBuilder::new("true");
        configure_pty_env_with_session(&mut agent, AgentCliPreset::Claude, None, Some(&env));
        assert_eq!(agent.get_cwd().map(std::path::PathBuf::from), Some(st.session_dir.clone()));
        let file = std::env::temp_dir().join("cwd-case-file.designos");
        let mut shell = CommandBuilder::new("true");
        configure_pty_env_with_session(&mut shell, AgentCliPreset::Shell, Some(&file), Some(&env));
        assert_eq!(shell.get_cwd().map(std::path::PathBuf::from), Some(std::env::temp_dir()));
    }

    #[test]
    fn session_skill_and_instructions_are_laid_out_where_every_cli_discovers_them() {
        let mut st = TerminalState::for_tab(736);
        let mut doc = Document::new("Skill Layout File");
        let page = doc.add_page("Page 1");
        st.sync_live_file_context(&doc, page, &[], None);
        for rel in [".claude/skills/designos-studio/SKILL.md", ".agents/skills/designos-studio/SKILL.md", "AGENTS.md"] {
            let text = std::fs::read_to_string(st.session_dir.join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"));
            assert!(text.contains("designos-studio") && text.contains("Skill Layout File"), "{rel}: {text}");
        }
    }

    fn read_json(path: &std::path::Path) -> serde_json::Value {
        serde_json::from_str(&std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))).expect("valid json")
    }

    /// Runs one hook command through `sh -c`, as every CLI does, and returns its stdout.
    fn run_hook_command(command: &str) -> String {
        let out = std::process::Command::new("sh").arg("-c").arg(command).output().expect("run hook");
        assert!(out.status.success(), "hook failed: {command}");
        String::from_utf8_lossy(&out.stdout).into_owned()
    }

    fn status_state(session_dir: &std::path::Path) -> Option<String> {
        let raw = std::fs::read_to_string(session_dir.join("agent-status.json")).ok()?;
        serde_json::from_str::<serde_json::Value>(&raw).ok()?.get("state")?.as_str().map(str::to_string)
    }

    fn provisioned_session(tab: u64) -> TerminalState {
        let mut st = TerminalState::for_tab(tab);
        let mut doc = Document::new("Hooks File");
        let page = doc.add_page("Page 1");
        let a = doc.add_page("Page 2");
        st.sync_live_file_context(&doc, page, &[a], None);
        let _ = std::fs::remove_file(st.session_dir.join("agent-status.json"));
        st
    }

    #[test]
    fn claude_hooks_file_is_a_valid_settings_file_and_the_launch_loads_it() {
        assert!(
            AgentCliPreset::Claude.cli_launch_command().contains("--settings \"$DESIGNOS_SESSION_DIR/hooks.json\""),
            "{}",
            AgentCliPreset::Claude.cli_launch_command()
        );
        let live_spec = std::path::Path::new("/tmp/designos-session-x/live-agent-out.spec.json");
        let sel_json = std::path::Path::new("/tmp/designos-session-x/selection.spec.json");
        let one_shot = build_real_cli_command(AgentCliPreset::Claude, "hi", live_spec, sel_json);
        let hooks_file = live_spec.parent().unwrap().join("hooks.json");
        assert!(one_shot.contains(&format!("--settings '{}'", hooks_file.display())), "{one_shot}");

        let st = provisioned_session(741);
        let settings = read_json(&st.session_dir.join("hooks.json"));
        let top: Vec<&String> = settings.as_object().unwrap().keys().collect();
        assert_eq!(top, vec!["hooks"], "a --settings file takes Claude's settings keys only");
        let hooks = settings["hooks"].as_object().unwrap();
        for event in ["UserPromptSubmit", "PreToolUse", "PostToolUse", "PermissionRequest", "Stop"] {
            let groups = hooks.get(event).unwrap_or_else(|| panic!("{event} missing")).as_array().unwrap();
            let handler = &groups[0]["hooks"][0];
            assert_eq!(handler["type"].as_str(), Some("command"), "{event}");
            assert!(handler["command"].as_str().is_some(), "{event}");
        }
    }

    #[test]
    fn session_hooks_drive_agent_status_from_working_to_done_in_every_supported_cli() {
        let st = provisioned_session(742);
        let claude = read_json(&st.session_dir.join("hooks.json"));
        let grok = read_json(&st.session_dir.join(".grok").join("hooks").join("designos-status.json"));
        assert_eq!(
            claude["hooks"].as_object().unwrap().keys().collect::<Vec<_>>(),
            grok["hooks"].as_object().unwrap().keys().collect::<Vec<_>>(),
            "Grok loads the same events as Claude"
        );
        let agy = read_json(&st.session_dir.join(".agents").join("hooks.json"));

        let cmd = |v: &serde_json::Value, event: &str| v["hooks"][event][0]["hooks"][0]["command"].as_str().unwrap().to_string();
        let agy_flat = |event: &str| agy["designos-status"][event][0]["command"].as_str().unwrap_or_else(|| panic!("agy {event}")).to_string();

        // Claude and Grok: prompt submitted -> working, turn over -> done.
        for (name, prompt, stop) in
            [("claude", cmd(&claude, "UserPromptSubmit"), cmd(&claude, "Stop")), ("grok", cmd(&grok, "UserPromptSubmit"), cmd(&grok, "Stop"))]
        {
            let _ = std::fs::remove_file(st.session_dir.join("agent-status.json"));
            run_hook_command(&prompt);
            assert_eq!(status_state(&st.session_dir).as_deref(), Some("working"), "{name}");
            run_hook_command(&stop);
            assert_eq!(status_state(&st.session_dir).as_deref(), Some("done"), "{name}");
        }
        // Antigravity: model call -> working, execution loop end -> done; handlers answer with a JSON object.
        let _ = std::fs::remove_file(st.session_dir.join("agent-status.json"));
        let reply = run_hook_command(&agy_flat("PreInvocation"));
        assert!(serde_json::from_str::<serde_json::Value>(reply.trim()).map(|v| v.is_object()).unwrap_or(false), "agy hook reply: {reply:?}");
        assert_eq!(status_state(&st.session_dir).as_deref(), Some("working"), "agy");
        run_hook_command(&agy_flat("Stop"));
        assert_eq!(status_state(&st.session_dir).as_deref(), Some("done"), "agy");
    }

    #[test]
    fn prompt_hooks_print_only_what_each_cli_can_read_and_never_the_selection() {
        let st = provisioned_session(743);
        let prompt_command = |v: &serde_json::Value| v["hooks"]["UserPromptSubmit"][0]["hooks"][0]["command"].as_str().unwrap().to_string();

        // Grok parses any stdout starting with `{` as a hook decision and discards an allowing
        // hook's context, so its prompt hook must print nothing.
        let grok = read_json(&st.session_dir.join(".grok").join("hooks").join("designos-status.json"));
        assert_eq!(run_hook_command(&prompt_command(&grok)), "", "a Grok prompt hook must not print");

        // Claude reads `hookSpecificOutput.additionalContext`: the page, never the selection.
        let claude = read_json(&st.session_dir.join("hooks.json"));
        let printed = run_hook_command(&prompt_command(&claude));
        let reply: serde_json::Value = serde_json::from_str(printed.trim()).expect("Claude hook prints decision json");
        let out = &reply["hookSpecificOutput"];
        assert_eq!(out["hookEventName"].as_str(), Some("UserPromptSubmit"));
        let context = out["additionalContext"].as_str().expect("additionalContext");
        assert!(context.contains("Hooks File") && context.contains("Page 1"), "{context}");
        assert!(!context.contains("selected_node_ids"), "owner ruling: the selection is never auto-injected: {context}");
        assert_eq!(reply.as_object().unwrap().len(), 1, "no decision keys that could block the prompt: {printed}");
        // context.json (with the selection) stays on disk for the agent to read when asked.
        assert!(read_json(&st.context_json_path).get("selected_node_ids").is_some());
    }

    #[test]
    fn agent_chat_codex_command_carries_the_same_mcp_overrides_as_the_tab() {
        let tab = AgentCliPreset::Codex.cli_launch_command();
        let live_spec = std::path::Path::new("/tmp/designos-session-x/live-agent-out.spec.json");
        let sel_json = std::path::Path::new("/tmp/designos-session-x/selection.spec.json");
        let chat = build_real_cli_command(AgentCliPreset::Codex, "Audit layout", live_spec, sel_json);
        assert!(chat.contains("codex --dangerously-bypass-approvals-and-sandbox exec"), "{chat}");
        for key in ["mcp_servers.designos.args", "mcp_servers.designos.env_vars"] {
            assert_eq!(codex_list_override(&chat, key), codex_list_override(tab, key), "{key} differs between tab and chat: {chat}");
        }
        assert!(chat.contains("mcp_servers.designos.command=\"'\"$DESIGNOS_MCP_BIN\"'\""), "{chat}");
        assert!(chat.contains("--workspace") && chat.contains("$DESIGNOS_WORKSPACE"), "{chat}");
    }

    /// A state whose stub agent has exited but whose exit the terminal has not yet observed: the
    /// bridge is not polled here, so each test decides the spawn time the exit is judged against.
    fn exited_stub_state() -> TerminalState {
        let mut st = TerminalState::default();
        spawn_stub_exiting_agent_into(&mut st);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
        while st.active_tab_mut().unwrap().is_alive() {
            assert!(std::time::Instant::now() < deadline, "stub agent must exit");
            std::thread::sleep(std::time::Duration::from_millis(25));
        }
        st
    }

    #[test]
    fn agent_that_exits_within_the_window_reports_a_start_failure_with_its_last_output() {
        let mut st = exited_stub_state();
        // A loaded CI runner can take longer than the window to start the shell; the window itself
        // is covered by the next test and by `counts_as_failed_start`.
        st.active_tab_mut().unwrap().spawned_at = std::time::Instant::now();
        let _ = st.poll_live_agent_bridge();
        let failure = st.active_tab().and_then(|t| t.start_failure.clone()).expect("start failure recorded");
        assert_eq!(failure.preset, AgentCliPreset::Claude);
        assert_eq!(failure.exit_code, Some(1));
        assert!(failure.output_tail.iter().any(|l| l.contains("STUB-AGENT-EXIT")), "{:?}", failure.output_tail);
        // The stub stands in for `claude`: whether the real binary is on this machine's PATH decides the cause.
        if AgentCliPreset::Claude.is_installed() {
            assert_eq!(failure.cause, agent_start_failure::StartFailureCause::ExitedEarly);
            assert_eq!(failure.fix, FixAction::OpenShell);
        } else {
            assert_eq!(failure.cause, agent_start_failure::StartFailureCause::NotInstalled);
            assert!(matches!(failure.fix, FixAction::OpenInstallPage(_)));
        }
    }

    #[test]
    fn agent_that_exits_after_the_window_is_not_a_start_failure() {
        let mut st = exited_stub_state();
        st.active_tab_mut().unwrap().spawned_at = std::time::Instant::now() - std::time::Duration::from_secs(10);
        let _ = st.poll_live_agent_bridge();
        assert!(st.active_tab().unwrap().start_failure.is_none());
        assert!(st.active_tab().unwrap().exited_notice_shown, "the ordinary exit notice still shows");
    }

    #[test]
    fn start_fixes_open_a_shell_or_run_the_login_command() {
        let mut st = TerminalState::default();
        st.apply_start_fix(FixAction::OpenShell, None);
        assert_eq!(st.tabs.last().map(|t| t.spawn_preset), Some(AgentCliPreset::Shell));

        st.apply_start_fix(FixAction::RunLogin("echo LOGIN-RAN-HERE"), None);
        let tab = st.tabs.last().expect("login tab");
        assert_eq!(tab.title, "echo LOGIN-RAN-HERE (login)");
        assert_eq!(tab.spawn_preset, AgentCliPreset::Shell, "a login is not an agent tab");
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(8);
        let mut shown = false;
        while std::time::Instant::now() < deadline && !shown {
            std::thread::sleep(std::time::Duration::from_millis(50));
            shown = st.tabs.last().and_then(|t| t.raw_log.lock().ok().map(|l| l.contains("LOGIN-RAN-HERE"))).unwrap_or(false);
        }
        assert!(shown, "login command output never reached the tab");
        kill_tabs(&mut st);
    }

    #[test]
    fn clicking_the_fix_on_the_failed_tab_panel_opens_a_shell_tab() {
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        crate::theme::apply(&ctx);
        let mut st = exited_stub_state();
        // A fixed diagnosis whose fix is `Open a shell`; the install-page fix would open a browser.
        let tab = st.active_tab_mut().unwrap();
        tab.start_failure = Some(agent_start_failure::diagnose(AgentCliPreset::Claude, true, Some(1), "error: unexpected argument '--x' found\n"));
        // The exit is already handled, so the next poll keeps this diagnosis instead of making its own.
        tab.exited_notice_shown = true;
        let tabs_before = st.tabs.len();
        let panel_rect = Rect::from_min_size(Pos2::new(48.0, 0.0), Vec2::new(300.0, 800.0));
        let frame = |st: &mut TerminalState, events: Vec<egui::Event>| {
            let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1600.0, 900.0))), events, ..Default::default() };
            let _ = ctx.run_ui(raw, |ui| {
                let _ = show_left(st, ui, panel_rect, None);
            });
        };
        // The button's rectangle is read again before every event so the click follows the layout.
        let button_center = |ctx: &egui::Context| -> Pos2 {
            let btn: Rect = ctx
                .data(|d| d.get_temp(Id::new(("test-rect", "agents-start-failure-fix-agents-left-pty".to_string()))))
                .expect("fix button painted in the Agents panel");
            btn.center()
        };
        frame(&mut st, vec![]);
        frame(&mut st, vec![]);
        let at = button_center(&ctx);
        frame(&mut st, vec![egui::Event::PointerMoved(at)]);
        let at = button_center(&ctx);
        frame(&mut st, vec![egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: true, modifiers: Default::default() }]);
        let at = button_center(&ctx);
        frame(&mut st, vec![egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: false, modifiers: Default::default() }]);
        frame(&mut st, vec![]);
        assert_eq!(st.tabs.len(), tabs_before + 1, "the fix must open a tab (status: {:?}, pending: {:?})", st.status_msg, st.pending_start_fix);
        assert_eq!(st.tabs.last().map(|t| t.spawn_preset), Some(AgentCliPreset::Shell));
        kill_tabs(&mut st);
    }
}
