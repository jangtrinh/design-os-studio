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
            " -c 'mcp_servers.designos.env_vars=[\"DESIGNOS_SESSION_DIR\",\"DESIGNOS_MCP_CONFIG\",\"DESIGNOS_LIVE_CANVAS\",\"DESIGNOS_LIVE_DOC_SYNC\",",
            "\"DESIGNOS_LIVE_DOC_PATCH\",\"DESIGNOS_LIVE_SPEC\",\"DESIGNOS_CONTEXT_JSON\",\"DESIGNOS_TARGET_PAGE_ID\",",
            "\"DESIGNOS_TARGET_PAGE_NAME\",\"DESIGNOS_DOC_NAME\",\"DESIGNOS_FILE\",\"DESIGNOS_AGENT_RUN_ID\",",
            "\"DESIGNOS_AGENT_RUNTIME\",\"DESIGNOS_IN_PROGRESS_STALE_MINUTES\"]'"
        )
    };
}

/// How Studio launches an agent CLI with respect to tool-approval prompts (`#645`, `#679`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum PermissionMode {
    /// Skip approval prompts using the CLI's documented bypass flag (default in Studio).
    #[default]
    Skip,
    /// Ask for approval using the CLI's default interactive mode.
    Ask,
}

/// Presets for launching an interactive PTY session inside DESIGN:OS Studio.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AgentCliPreset {
    Shell,
    Claude,
    Codex,
    Antigravity,
    Grok,
    Gemini,
    OpenCode,
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
    pub const GeminiCli: AgentCliPreset = AgentCliPreset::Gemini;
    pub const OpenCodeCli: AgentCliPreset = AgentCliPreset::OpenCode;
    pub const DefaultShell: AgentCliPreset = AgentCliPreset::Shell;

    pub const QUICK_LAUNCH: [AgentCliPreset; 5] =
        [AgentCliPreset::Claude, AgentCliPreset::Codex, AgentCliPreset::Antigravity, AgentCliPreset::Grok, AgentCliPreset::Shell];

    pub fn label(self) -> &'static str {
        self.tab_title()
    }

    pub fn binary(self) -> &'static str {
        self.binary_name()
    }

    pub fn display_name(self) -> &'static str {
        match self {
            AgentCliPreset::Shell => "Shell",
            AgentCliPreset::Claude => "Claude Code",
            AgentCliPreset::Codex => "Codex",
            AgentCliPreset::Antigravity => "Antigravity",
            AgentCliPreset::Grok => "Grok",
            AgentCliPreset::Gemini => "Gemini CLI",
            AgentCliPreset::OpenCode => "OpenCode",
            AgentCliPreset::DesignOsHelp => "Agent Guide",
        }
    }

    pub fn tab_title(self) -> &'static str {
        match self {
            AgentCliPreset::Shell => "Shell ($SHELL)",
            AgentCliPreset::Claude => "Claude Code",
            AgentCliPreset::Codex => "Codex CLI",
            AgentCliPreset::Antigravity => "Antigravity (agy)",
            AgentCliPreset::Grok => "Grok Build",
            AgentCliPreset::Gemini => "Gemini CLI",
            AgentCliPreset::OpenCode => "OpenCode",
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
            AgentCliPreset::Gemini => "Gemini CLI",
            AgentCliPreset::OpenCode => "OpenCode",
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
            AgentCliPreset::Gemini => "GEMINI",
            AgentCliPreset::OpenCode => "OPENCODE",
            AgentCliPreset::DesignOsHelp => "DESIGNOS",
        }
    }

    pub fn short_agent_name(self) -> &'static str {
        match self {
            AgentCliPreset::Claude => "Claude",
            AgentCliPreset::Codex => "Codex",
            AgentCliPreset::Antigravity => "Antigravity",
            AgentCliPreset::Grok => "Grok",
            AgentCliPreset::Gemini => "Gemini",
            AgentCliPreset::OpenCode => "OpenCode",
            AgentCliPreset::Shell | AgentCliPreset::DesignOsHelp => "Claude",
        }
    }

    pub fn binary_name(self) -> &'static str {
        match self {
            AgentCliPreset::Shell => "zsh",
            AgentCliPreset::Claude => "claude",
            AgentCliPreset::Codex => "codex",
            AgentCliPreset::Antigravity => "agy",
            AgentCliPreset::Grok => "grok",
            AgentCliPreset::Gemini => "gemini",
            AgentCliPreset::OpenCode => "opencode",
            AgentCliPreset::DesignOsHelp => "designos",
        }
    }

    /// True when this tab preset is an agent CLI tab (`Claude`, `Codex`, `Antigravity`, `Grok`, `Gemini`, `OpenCode`)
    /// rather than an interactive `Shell` or `DesignOsHelp` tab (`#685`).
    pub fn is_agent(self) -> bool {
        !matches!(self, AgentCliPreset::Shell | AgentCliPreset::DesignOsHelp)
    }

    /// True when this preset reserves `Ctrl+Z` inside the terminal on Windows/Linux (`#685`).
    pub fn uses_ctrl_z(self) -> bool {
        crate::terminal_undo_chord::preset_ctrl_z_policy(self).1
    }

    /// Documented permission / approval flags for `mode` (`#679` §3).
    /// Returns `&[]` when the CLI has no permission flag (e.g. `OpenCode`, `Shell`).
    pub fn permission_flags(self, mode: PermissionMode) -> &'static [&'static str] {
        match (self, mode) {
            (AgentCliPreset::Claude, PermissionMode::Skip) => &["--dangerously-skip-permissions"],
            (AgentCliPreset::Claude, PermissionMode::Ask) => &[],
            (AgentCliPreset::Codex, PermissionMode::Skip) => &["--dangerously-bypass-approvals-and-sandbox"],
            (AgentCliPreset::Codex, PermissionMode::Ask) => &[],
            (AgentCliPreset::Grok, PermissionMode::Skip) => &["--always-approve"],
            (AgentCliPreset::Grok, PermissionMode::Ask) => &[],
            (AgentCliPreset::Antigravity, PermissionMode::Skip) => &["--dangerously-skip-permissions"],
            (AgentCliPreset::Antigravity, PermissionMode::Ask) => &[],
            (AgentCliPreset::Gemini, PermissionMode::Skip) => &["--approval-mode", "yolo"],
            (AgentCliPreset::Gemini, PermissionMode::Ask) => &["--approval-mode", "default"],
            (AgentCliPreset::OpenCode | AgentCliPreset::Shell | AgentCliPreset::DesignOsHelp, _) => &[],
        }
    }

    /// Interactive CLI launch command for `mode` (`PermissionMode::Skip` vs `PermissionMode::Ask`).
    pub fn cli_launch_command_for_mode(self, mode: PermissionMode) -> &'static str {
        match (self, mode) {
            (AgentCliPreset::Claude, PermissionMode::Skip) => {
                "claude --dangerously-skip-permissions --mcp-config \"$DESIGNOS_MCP_CONFIG\" --settings \"$DESIGNOS_SESSION_DIR/hooks.json\" --append-system-prompt \"You are inside DESIGN:OS Studio bound exclusively to file '$DESIGNOS_DOC_NAME' (session $DESIGNOS_SESSION_DIR). Always read $DESIGNOS_CONTEXT_JSON for the current selected page (current_page_id / current_page_name) and deliver all designs onto that file and page via designos-mcp tools (designos_apply_spec, designos_create_node, designos_update_node) or by writing NodeSpec JSON to $DESIGNOS_LIVE_SPEC.\""
            }
            (AgentCliPreset::Claude, PermissionMode::Ask) => {
                "claude --mcp-config \"$DESIGNOS_MCP_CONFIG\" --settings \"$DESIGNOS_SESSION_DIR/hooks.json\" --append-system-prompt \"You are inside DESIGN:OS Studio bound exclusively to file '$DESIGNOS_DOC_NAME' (session $DESIGNOS_SESSION_DIR). Always read $DESIGNOS_CONTEXT_JSON for the current selected page (current_page_id / current_page_name) and deliver all designs onto that file and page via designos-mcp tools (designos_apply_spec, designos_create_node, designos_update_node) or by writing NodeSpec JSON to $DESIGNOS_LIVE_SPEC.\""
            }
            (AgentCliPreset::Codex, PermissionMode::Skip) => concat!("codex --dangerously-bypass-approvals-and-sandbox", codex_mcp_overrides!()),
            (AgentCliPreset::Codex, PermissionMode::Ask) => concat!("codex", codex_mcp_overrides!()),
            (AgentCliPreset::Grok, PermissionMode::Skip) => "grok --always-approve --cwd \"$DESIGNOS_SESSION_DIR\"",
            (AgentCliPreset::Grok, PermissionMode::Ask) => "grok --cwd \"$DESIGNOS_SESSION_DIR\"",
            (AgentCliPreset::Antigravity, PermissionMode::Skip) => "agy --dangerously-skip-permissions --add-dir \"$DESIGNOS_SESSION_DIR\"",
            (AgentCliPreset::Antigravity, PermissionMode::Ask) => "agy --add-dir \"$DESIGNOS_SESSION_DIR\"",
            // `gemini` reads `<session_dir>/.gemini/settings.json` from its cwd (written by
            // `write_gemini_session_config`) and never touches `~/.gemini/settings.json`.
            (AgentCliPreset::Gemini, PermissionMode::Skip) => "gemini --approval-mode yolo --include-directories \"$DESIGNOS_SESSION_DIR\"",
            (AgentCliPreset::Gemini, PermissionMode::Ask) => "gemini --approval-mode default --include-directories \"$DESIGNOS_SESSION_DIR\"",
            // `opencode` starts in `session_dir` (`opencode <project>`).
            (AgentCliPreset::OpenCode, _) => "opencode \"$DESIGNOS_SESSION_DIR\"",
            (AgentCliPreset::Shell | AgentCliPreset::DesignOsHelp, _) => "",
        }
    }

    /// Interactive CLI launch command with dangerous-permission prompts skipped (`DIR-29`)
    /// and per-file `designos-mcp` + live current-page system prompt pre-wired.
    pub fn cli_launch_command(self) -> &'static str {
        self.cli_launch_command_for_mode(PermissionMode::Skip)
    }

    /// Dropdown menu label and flag summary shown in the `+` runtime selector menu.
    pub fn dropdown_entry(self) -> (&'static str, &'static str, &'static str) {
        match self {
            AgentCliPreset::Claude => ("agents-runtime-claude", "Claude", "--dangerously-skip-permissions"),
            AgentCliPreset::Codex => ("agents-runtime-codex", "Codex", "--dangerously-bypass-approvals-and-sandbox"),
            AgentCliPreset::Grok => ("agents-runtime-grok", "Grok", "--always-approve"),
            AgentCliPreset::Antigravity => ("agents-runtime-agy", "Antigravity", "--dangerously-skip-permissions"),
            AgentCliPreset::Gemini => ("agents-runtime-gemini", "Gemini CLI", "--approval-mode yolo"),
            AgentCliPreset::OpenCode => ("agents-runtime-opencode", "OpenCode", "opencode"),
            AgentCliPreset::Shell | AgentCliPreset::DesignOsHelp => ("agents-runtime-pty-toggle", "Shell", "$SHELL"),
        }
    }

    pub fn is_installed(self) -> bool {
        match self {
            AgentCliPreset::Shell | AgentCliPreset::DesignOsHelp => true,
            other => runtime_binary_available(other.binary_name()),
        }
    }

    /// Verified installation directories for this CLI preset and shared user/system folders.
    pub fn install_dirs(self) -> Vec<PathBuf> {
        let _ = self;
        extra_cli_path_dirs()
    }
}

/// Shared list of user and system CLI installation directories probed by [`runtime_binary_available`]
/// and prepended to the child PTY `PATH` in [`configure_pty_env_with_session`].
pub fn extra_cli_path_dirs() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = Vec::new();
    if let Ok(home) = std::env::var("HOME") {
        if !home.is_empty() {
            dirs.push(PathBuf::from(format!("{home}/.cargo/bin")));
            dirs.push(PathBuf::from(format!("{home}/.local/bin")));
            dirs.push(PathBuf::from(format!("{home}/.antigravity/bin")));
            dirs.push(PathBuf::from(format!("{home}/.antigravity/antigravity/bin")));
            dirs.push(PathBuf::from(format!("{home}/.grok/bin")));
            dirs.push(PathBuf::from(format!("{home}/.npm-global/bin")));
            dirs.push(PathBuf::from(format!("{home}/.opencode/bin")));
        }
    }
    if cfg!(windows) {
        dirs.push(PathBuf::from(r"C:\Program Files\Git\usr\bin"));
        dirs.push(PathBuf::from(r"C:\Program Files\Git\bin"));
        dirs.push(PathBuf::from(r"C:\Windows\System32"));
    } else {
        dirs.push(PathBuf::from("/opt/homebrew/bin"));
        dirs.push(PathBuf::from("/usr/local/bin"));
    }
    dirs
}

/// Prompt sent by the `"Make design (First Draft)"` action (`#681` §3 rule 5, §13).
pub const MAKE_DESIGN_FIRST_DRAFT_PROMPT: &str =
    "Design a first draft for the selected frame, or for this page if nothing is selected, using this file's variables and styles.";

/// Checks if `bin` exists as an executable file in `PATH` (or common user/system CLI directories, or as an explicit path).
pub fn runtime_binary_available(bin: &str) -> bool {
    let path_os = std::env::var_os("PATH");
    let include_default_dirs = !path_os.as_ref().is_some_and(|p| p.is_empty());
    runtime_binary_available_in(bin, path_os.as_deref(), include_default_dirs)
}

fn runtime_binary_available_in(bin: &str, path_os: Option<&std::ffi::OsStr>, include_default_dirs: bool) -> bool {
    let trimmed = bin.trim();
    if trimmed.is_empty() {
        return false;
    }
    let explicit = Path::new(trimmed);
    if explicit.components().count() > 1 {
        return explicit.is_file();
    }

    let mut dirs: Vec<PathBuf> = Vec::new();
    if let Some(p_os) = path_os {
        for p in std::env::split_paths(p_os) {
            if !p.as_os_str().is_empty() {
                dirs.push(p);
            }
        }
    }
    if include_default_dirs {
        dirs.extend(extra_cli_path_dirs());
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
    pub run_id: String,
    pub agent_runtime: String,
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
    pub path_prepend: Vec<PathBuf>,
    #[cfg(test)]
    pub path_override: Option<std::ffi::OsString>,
}

impl SessionEnv {
    #[allow(dead_code)]
    pub fn for_session_id(session_id: &str) -> Self {
        let clean = if session_id.trim().is_empty() { "tab-1" } else { session_id.trim() };
        let session_dir = std::env::temp_dir().join("designos-sessions").join(clean);
        Self {
            session_id: clean.to_string(),
            run_id: format!("{clean}-1-1"),
            agent_runtime: AgentCliPreset::Claude.binary_name().to_string(),
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
            path_prepend: Vec::new(),
            #[cfg(test)]
            path_override: None,
        }
    }
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
    /// Writes the whole selection (`nodes` array, first 10 inline specs, <= 256 KB) to
    /// `selection.spec.json` and pastes one fixed sentence into the active agent tab without Enter.
    SendSelectionToAgent,
    /// Opens or toggles the per-turn change summary for `tab_id` when its receipt line is clicked (`#683` §3.5, §7).
    OpenTurnReceipt(usize),
    /// Focuses the Studio window and selects agent `tab_id` when its OS notification is clicked (`#683` §3.6).
    FocusAgentTab(usize),
    /// Closes a stopped agent turn: applies held patches, folds contiguous agent undo steps since
    /// `turn_start_depth` into one `"Agent turn (stopped)"` batch, and clears in-progress flags (`#684`).
    CloseStoppedTurn { tab_id: usize, turn_start_depth: usize, run_id: Option<String> },
    /// Clears `Document.in_progress` entries owned by an agent run that finished (`agent-status.json` -> `done`) or whose PTY exited.
    ClearInProgress { run_id: Option<String>, agent: Option<String> },
}

/// Orca live agent execution status (`Working` / `WaitingForInput` / `Done` / `Error` / `Stopped` / `Idle`).
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub enum AgentTabStatus {
    #[default]
    Idle,
    Working(Option<String>),
    WaitingForInput(Option<String>),
    Done,
    Error(Option<String>),
    Stopped,
}

/// One live interactive PTY tab backed by `portable-pty` and a `vt100::Parser`.
pub struct TerminalTab {
    pub id: usize,
    pub run_id: String,
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
    /// Last status observed by `TerminalState::drain_status_changes` (`#687`).
    pub reported_status: AgentTabStatus,
    pub last_output_at: Arc<Mutex<Option<std::time::Instant>>>,
    /// Set after the agent-exit notice is written into the vt100 parser. Never respawn a shell.
    pub exited_notice_shown: bool,
    /// When the PTY child was started; an agent that exits within `START_FAILURE_WINDOW` failed to start.
    pub spawned_at: std::time::Instant,
    /// Why the agent CLI exited right after it was opened; shown instead of the dead terminal.
    pub start_failure: Option<StartFailure>,
    /// Pre-assigned UUID v4 passed via `--session-id` (for Claude and Grok), or `None`.
    pub session_uuid: Option<String>,
    /// True when this tab was opened by resuming an entry from `sessions.json`.
    pub is_resumed: bool,
    /// Raw bytes written via [`Self::write_bytes`] during tests (`#685`).
    #[cfg(test)]
    pub written_bytes: Vec<u8>,
    /// Per-tab multi-line composer state (`#689` §4).
    pub composer: crate::agent_composer::ComposerState,
    /// Explicit shell command passed when opening this tab (`spawn_tab_running` / `spawn_custom_cli_tab`), if any.
    pub spawned_command: Option<String>,
    /// Slave PTY handle retained until the reader drains the first chunk (or the child exits silently)
    /// so macOS XNU `ptsclose`/`ttyflush` cannot discard unread output on immediate child exit (`#782`).
    slave: Arc<Mutex<Option<Box<dyn portable_pty::SlavePty + Send>>>>,
    /// Set to `true` once the background PTY reader thread has reached EOF or exited.
    pub reader_eof: Arc<std::sync::atomic::AtomicBool>,
    /// When set, the PTY reader also reports an approval prompt in the output as `WaitingForInput` for a tab whose
    /// status otherwise comes from hook events: Antigravity in `Ask` mode has no permission hook (`#656`).
    pub pty_waiting_heuristic: Arc<std::sync::atomic::AtomicBool>,
    /// Timestamp when the child process was first observed dead while waiting for `reader_eof`.
    pub exited_at: Option<std::time::Instant>,
    /// Per-tab ledger of agent canvas turns (`#677`).
    pub turn_ledger: crate::agent_turn_ledger::AgentTurnLedger,
    /// Set when a turn ends while this tab is not visible; cleared when the user shows the tab (`#683` §3.3).
    pub unread: bool,
    /// Name of the document this tab belongs to (`#683` §8.6).
    pub doc_name: String,
    /// Per-tab turn state machine and receipt ledger (`#683` §3.3, §3.5).
    pub turn_tracker: crate::agent_turn_status::TabTurnTracker,
    /// Permission mode active when this tab was spawned (`#656`).
    pub permission_mode: crate::agent_permissions::AgentPermissionMode,
    /// Per-tab approvals directory `<session_dir>/approvals/tab-<id>` (`#656`).
    pub approvals_dir: PathBuf,
    /// Pending approval requests queued for this tab (`#656`).
    pub pending_approvals: Vec<crate::agent_permissions::ApprovalRequest>,
    /// Active stop sequence state (`#684`), or `None` when not stopping.
    pub stop_state: Option<crate::agent_stop::StopState>,
    /// Shared flag so the PTY reader thread ignores OSC status updates while a stop sequence is in flight (`#684`).
    pub stop_in_flight: Arc<std::sync::atomic::AtomicBool>,
    /// When the first interrupt key of the latest stop went out; kept after the stop closes so the ledger can tell the stopped turn from the next one.
    pub stop_sent_instant: Option<std::time::Instant>,
    /// Keys sent by the stop sequence or Esc forwarding on this tab (for deterministic test inspection, `#684`).
    pub stop_sent_keys: Vec<u8>,
    /// Timestamp (`ms`) when the current turn started (`#684`).
    pub turn_started_at_ms: Option<u64>,
    /// `History::undo_depth` recorded at the start of the current turn (`#684`).
    pub turn_start_undo_depth: usize,
    /// True while the tab is inside an active `Working` turn (`#684`).
    pub was_working: bool,
    /// Notice shown when the CLI ignored both interrupt keys (`#684 §8.1`).
    pub still_running_notice: Option<String>,
    /// Last stopped-turn receipt line (e.g. `"Stopped by you after 42s"`, `#684 §3.4`); the only receipt for CLIs without a hook stream.
    pub last_receipt: Option<String>,
    /// Set on a restart-resumed tab (`#653`) that was running without a known turn (`#684 §8.6`).
    pub resumed_without_known_turn: bool,
}

impl TerminalTab {
    /// True when this tab has a queued approval request file or is a non-Claude tab in `Ask` mode waiting for input (`#656`).
    pub fn needs_approval(&self) -> bool {
        if !self.pending_approvals.is_empty() {
            return true;
        }
        self.permission_mode == crate::agent_permissions::AgentPermissionMode::Ask
            && matches!(self.preset, AgentCliPreset::Codex | AgentCliPreset::Grok | AgentCliPreset::Antigravity)
            && self.agent_status.lock().ok().is_some_and(|g| matches!(*g, AgentTabStatus::WaitingForInput(_)))
    }

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
    pub(crate) fn spawn_with_session_cmd(
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
                                c.arg("-d");
                                c.arg("-f");
                                #[cfg(test)]
                                isolate_zsh_from_home(&mut c);
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
                    AgentCliPreset::Gemini => build_agent_shell_cmd(&shell_bin, AgentCliPreset::Gemini.cli_launch_command()),
                    AgentCliPreset::OpenCode => build_agent_shell_cmd(&shell_bin, AgentCliPreset::OpenCode.cli_launch_command()),
                }
            }
        };

        if let Some(sess) = session {
            for lock_name in ["agent-events.jsonl.lock", "agent-status.jsonl.lock", "agent-status.json.lock"] {
                let _ = std::fs::remove_dir_all(sess.session_dir.join(lock_name));
            }
        }
        let approvals_dir = session
            .map(|s| crate::agent_permissions::tab_approvals_dir(&s.session_dir, id))
            .unwrap_or_else(|| std::env::temp_dir().join(format!("designos-approvals-tab-{id}")));
        configure_pty_env_with_session(&mut cmd, preset, file_path, session);
        if session.is_some() {
            cmd.env("DESIGNOS_APPROVALS_DIR", approvals_dir.as_os_str());
            cmd.env("DESIGNOS_TAB_ID", id.to_string());
        }

        let mut reader = pair.master.try_clone_reader().map_err(|e| format!("try_clone_reader failed: {e}"))?;
        let writer = pair.master.take_writer().map_err(|e| format!("take_writer failed: {e}"))?;

        let child = pair.slave.spawn_command(cmd).map_err(|e| format!("spawn_command failed: {e}"))?;
        let slave = Arc::new(Mutex::new(Some(pair.slave)));

        let init_parser = vt100::Parser::new(rows, cols, 5000);
        let parser = Arc::new(Mutex::new(init_parser));
        let last_spec_json = Arc::new(Mutex::new(None));
        let raw_log = Arc::new(Mutex::new(String::new()));
        let agent_status = Arc::new(Mutex::new(AgentTabStatus::Idle));
        let last_output_at = Arc::new(Mutex::new(None));
        let reader_eof = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let pty_waiting_heuristic = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let stop_in_flight = Arc::new(std::sync::atomic::AtomicBool::new(false));

        let parser_bg = Arc::clone(&parser);
        let spec_bg = Arc::clone(&last_spec_json);
        let log_bg = Arc::clone(&raw_log);
        let status_bg = Arc::clone(&agent_status);
        let output_at_bg = Arc::clone(&last_output_at);
        let eof_bg = Arc::clone(&reader_eof);
        let waiting_heuristic_bg = Arc::clone(&pty_waiting_heuristic);
        let slave_bg = Arc::downgrade(&slave);
        let stop_bg = Arc::clone(&stop_in_flight);

        let uses_pty_heuristic = preset == AgentCliPreset::Codex;
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
                            let has_osc = chunk.contains("\x1b]9999;");
                            let (clean_chunk, detected_status) = parse_osc_agent_status_and_strip(&chunk);
                            // While a stop sequence is in flight the status is owned by it, and a stopped tab only
                            // leaves `Stopped` when new work starts (`#684`).
                            let stopping = stop_bg.load(std::sync::atomic::Ordering::Relaxed);
                            let set_status = |next_status: AgentTabStatus| {
                                if stopping {
                                    return;
                                }
                                if let Ok(mut st) = status_bg.lock() {
                                    if !matches!(*st, AgentTabStatus::Stopped) || matches!(next_status, AgentTabStatus::Working(_)) {
                                        *st = next_status;
                                    }
                                }
                            };
                            if uses_pty_heuristic || has_osc {
                                if let Some(next_status) = detected_status {
                                    set_status(next_status);
                                }
                            } else if waiting_heuristic_bg.load(std::sync::atomic::Ordering::Acquire) {
                                if let Some(next_status @ AgentTabStatus::WaitingForInput(_)) = detected_status {
                                    set_status(next_status);
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
                            if let Some(s_arc) = slave_bg.upgrade() {
                                if let Ok(mut s) = s_arc.lock() {
                                    *s = None;
                                }
                            }
                        }
                        Err(_) => break,
                    }
                }
                eof_bg.store(true, std::sync::atomic::Ordering::Release);
            })
            .map_err(|e| format!("failed to spawn PTY reader thread: {e}"))?;

        let doc_name = session.map(|s| s.doc_name.clone()).unwrap_or_else(|| "Untitled".to_string());
        let run_id = session.map(|s| s.run_id.clone()).filter(|s| !s.is_empty()).unwrap_or_else(|| format!("p{}-tab-1-{id}-1", std::process::id()));

        Ok(Self {
            id,
            run_id,
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
            reported_status: AgentTabStatus::Idle,
            last_output_at,
            exited_notice_shown: false,
            spawned_at: std::time::Instant::now(),
            start_failure: None,
            session_uuid: None,
            is_resumed: false,
            #[cfg(test)]
            written_bytes: Vec::new(),
            composer: crate::agent_composer::ComposerState::default(),
            spawned_command: None,
            slave,
            reader_eof,
            pty_waiting_heuristic,
            exited_at: None,
            turn_ledger: crate::agent_turn_ledger::AgentTurnLedger::default(),
            unread: false,
            doc_name,
            turn_tracker: crate::agent_turn_status::TabTurnTracker::default(),
            permission_mode: crate::agent_permissions::AgentPermissionMode::Skip,
            approvals_dir,
            pending_approvals: Vec::new(),
            stop_state: None,
            stop_in_flight,
            stop_sent_instant: None,
            stop_sent_keys: Vec::new(),
            turn_started_at_ms: None,
            turn_start_undo_depth: 0,
            was_working: false,
            still_running_notice: None,
            last_receipt: None,
            resumed_without_known_turn: false,
        })
    }

    /// Test helper: spawns a stub `CommandBuilder` under `preset` (`#684`).
    #[cfg(test)]
    pub fn spawn_test_stub(id: usize, preset: AgentCliPreset, mut cmd: CommandBuilder) -> Result<Self, String> {
        isolate_zsh_from_home(&mut cmd);
        Self::spawn_with_session_cmd(id, preset, 24, 80, None, None, Some(cmd))
    }

    /// Effective status of this tab (`#683` §3.3, §4).
    /// Codex uses the PTY output-burst heuristic (`terminal.rs:510`, `2270`); Claude, Grok, and Antigravity
    /// are driven by `agent-events.jsonl` hook records.
    pub fn effective_status(&self) -> AgentTabStatus {
        let explicit_status = self.agent_status.lock().ok().map(|g| g.clone()).unwrap_or_default();
        let recent_pty_activity = self.spawn_preset == AgentCliPreset::Codex
            && self.last_output_at.lock().ok().and_then(|g| *g).is_some_and(|ts| ts.elapsed() < std::time::Duration::from_millis(600));
        match explicit_status {
            AgentTabStatus::Working(d) => AgentTabStatus::Working(d),
            AgentTabStatus::WaitingForInput(d) => AgentTabStatus::WaitingForInput(d),
            // A stopped Codex tab keeps showing `Stopped` while the CLI is still repainting its prompt (`#684`).
            AgentTabStatus::Stopped => AgentTabStatus::Stopped,
            other if recent_pty_activity => AgentTabStatus::Working(None),
            other => other,
        }
    }

    /// True for a tool, notification or failure hook event that was already in flight when the user stopped this tab. Such a
    /// straggler must not revive the stopped turn; only a new prompt does (`#684`).
    pub fn is_stale_event_after_stop(&self, event: &str) -> bool {
        matches!(event, "PreToolUse" | "PostToolUse" | "PermissionRequest" | "Notification" | "StopFailure")
            && matches!(self.agent_status.lock().ok().as_deref(), Some(AgentTabStatus::Stopped))
    }

    /// Last completed turn receipt for this tab (`None` for Codex and Shell, which have no hook stream).
    pub fn last_turn_receipt(&self) -> Option<&crate::agent_turn_status::TurnReceipt> {
        if matches!(self.spawn_preset, AgentCliPreset::Codex | AgentCliPreset::Shell | AgentCliPreset::DesignOsHelp) {
            return None;
        }
        self.turn_tracker.last_turn_receipt.as_ref()
    }

    /// Header status indicator (`(dot_color, text)`) when this tab is running, waiting for input, or in error (`#683` §5).
    pub fn header_status_indicator(&self, now_s: u64) -> Option<(Color32, String)> {
        match self.effective_status() {
            AgentTabStatus::Working(detail) => {
                let text = if self.turn_tracker.in_turn {
                    self.turn_tracker.running_header_text(Some(now_s as f64))
                } else {
                    detail.unwrap_or_else(|| "Working…".to_string())
                };
                Some((theme::INFO, text))
            }
            AgentTabStatus::WaitingForInput(detail) => {
                let text = detail.unwrap_or_else(|| "Waiting for approval".to_string());
                Some((theme::WARNING, text))
            }
            AgentTabStatus::Error(reason) => {
                let text = reason.unwrap_or_else(|| "Ended without a stop event".to_string());
                Some((theme::DANGER, text))
            }
            AgentTabStatus::Idle | AgentTabStatus::Done | AgentTabStatus::Stopped => None,
        }
    }

    /// Test helper: Claude `spawn_preset` running a stub that prints and exits (no `$SHELL` fallback).
    #[cfg(test)]
    pub(crate) fn spawn_exiting_stub_agent(id: usize, rows: u16, cols: u16, file_path: Option<&Path>, session: Option<&SessionEnv>) -> Result<Self, String> {
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

    /// Writes raw bytes directly to the PTY master stdin, returning any I/O error.
    pub fn try_write_bytes(&mut self, bytes: &[u8]) -> std::io::Result<()> {
        #[cfg(test)]
        self.written_bytes.extend_from_slice(bytes);
        self.writer.write_all(bytes)?;
        self.writer.flush()
    }

    /// Writes raw bytes directly to the PTY master stdin.
    pub fn write_bytes(&mut self, bytes: &[u8]) {
        let _ = self.try_write_bytes(bytes);
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
        let alive = matches!(self.child.try_wait(), Ok(None));
        if !alive {
            let first_dead = *self.exited_at.get_or_insert_with(std::time::Instant::now);
            let has_output = self.raw_log.lock().map(|l| !l.is_empty()).unwrap_or(false);
            if has_output || first_dead.elapsed() >= std::time::Duration::from_millis(50) {
                if let Ok(mut s) = self.slave.lock() {
                    *s = None;
                }
            }
        }
        alive
    }

    /// True when this tab is a live agent CLI (not a shell process, and the child is still running).
    pub fn accepts_agent_input(&mut self) -> bool {
        !self.is_shell_process() && self.is_alive()
    }

    /// Writes `Ctrl-E` (end-of-line) then `Ctrl-U` (kill line / tty VKILL), then `line` and `\r`.
    #[allow(dead_code)]
    pub fn send_line_clearing_pending(&mut self, line: &str) {
        let _ = self.writer.write_all(b"\x05\x15");
        let _ = self.writer.write_all(line.as_bytes());
        let _ = self.writer.write_all(b"\r");
        let _ = self.writer.flush();
    }
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
    /// Unattached composer state held while `tabs` is empty; moved into the first spawned tab (`#689` §4).
    pub unattached_composer: crate::agent_composer::ComposerState,
    pub status_msg: Option<String>,
    pub next_tab_id: usize,
    pub next_run_seq: usize,
    pub active_run_id: String,
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
    /// Whether the Orca floating `Cmd+F` terminal search bar is open.
    pub search_open: bool,
    /// Query string for the `Cmd+F` terminal search bar.
    pub search_query: String,
    /// Whether `Cmd+F` terminal search is case-sensitive (`Aa`).
    pub search_case_sensitive: bool,
    /// True when the last pointer press was in the panel's non-terminal chrome (header, tab strip, composer) (`#684 §3.1`).
    pub chrome_focused: bool,
    /// Deferred undo depth to fold once a user edit in progress finishes (`#684 §3.4`).
    pub pending_stop_fold_depth: Option<usize>,
    /// Deferred stop close metadata (`depth`, `pre_close_patch_count`, `run_id`, `agent`) (`#684 §3.4`, `§3.5`).
    pub pending_stop_close: Option<crate::agent_stop::PendingStopClose>,
    /// Current `History::undo_depth` of the open document, refreshed by `publish_live_context` (`#684`).
    pub current_undo_depth: usize,
    /// Test-only override for `cfg!(target_os = "macos")` when routing terminal undo/redo chords (`#685`).
    #[cfg(test)]
    pub platform_is_mac_override: Option<bool>,
    /// Test-only override for `preset.uses_ctrl_z()` when routing terminal undo/redo chords (`#685`).
    #[cfg(test)]
    pub cli_uses_ctrl_z_override: Option<bool>,
    /// Test-only `PATH` override for binary availability checks and spawned agent tabs.
    #[cfg(test)]
    pub path_override: Option<std::ffi::OsString>,
    /// How this terminal's session directory is keyed (`LibraryFile` vs `Fallback`).
    pub key: crate::agent_sessions::SessionKey,
    /// Active Studio library root (`#652` / `#653`), retained even when the session falls back to `/tmp/designos-sessions/`.
    pub library_root: Option<PathBuf>,
    /// Persistent `sessions.json` registry for this session folder (`#653`).
    pub session_registry: crate::agent_sessions::SessionRegistry,
    /// Window identifier holding `<session_dir>/session.lock` when bound to a library file (`None` on fallback).
    pub lock_owner: Option<String>,
    /// Inputs and resulting key of the last [`Self::bind_open_file`], so a steady frame does no IO.
    pub open_file_binding: Option<crate::agent_sessions::OpenFileBinding>,
    /// Whether `codex resume --last` has passed the runtime cwd-scoping check (`#653` §6).
    pub codex_cwd_scoped: bool,
    /// Whether `agy --continue` has passed the runtime cwd-scoping check (`#653` §6).
    pub agy_cwd_scoped: bool,
    /// Additional directories prepended at the front of `PATH` when spawning child PTYs.
    pub path_prepend: Vec<PathBuf>,
    /// State for the new-session CLI picker, compact `+` menu, and inline `"Other CLI..."` form (`#652`).
    pub picker: crate::agent_picker::AgentPickerState,
    /// True when the center of the Agent panel / `>_` dock shows the new-session picker (`#651`).
    pub picker_open: bool,
    /// True when the `"Sessions"` popover is open (`#651` §3 rule 5).
    pub sessions_popover_open: bool,
    /// `tab_id` of the session entry currently being renamed in place inside the `"Sessions"` popover (`#651` §3 rule 6).
    pub renaming_session_tab_id: Option<u64>,
    /// Draft text for the active inline session rename (`#651` §3 rule 6).
    pub rename_session_input: String,
    /// Prompt held by `dispatch_figma_ai_prompt` when no live tab and no resumable session exist (`#651` §3 rule 10).
    pub pending_prompt: Option<(String, String)>,
    /// Hook-written turn boundary marks reader (`<session_dir>/turn-marks.jsonl`, `#677`).
    pub turn_marks: crate::agent_turn_marks::TurnMarkLog,
    /// Fallback turn ledger used when `tabs` is empty (`#677`).
    pub turn_ledger: crate::agent_turn_ledger::AgentTurnLedger,
    /// Deferred `(tab_idx, turn_id)` revert waiting for an in-progress user edit to finish (`#677`).
    pub pending_turn_revert: Option<(usize, u64)>,
    /// Append-only `agent-events.jsonl` reader (`#683` §3.1).
    pub event_stream: crate::agent_turn_status::AgentEventStream,
    /// Background desktop notification dispatcher (`#683` §3.7).
    pub notifier: crate::agent_notify::AgentNotifier,
    /// True when the Studio OS window currently has focus (`#683` §3.7).
    pub window_focused: bool,
    /// True when the Agents panel or Terminal dock is currently visible (`#683` §3.7).
    pub surface_visible: bool,
    /// Observed per-tab status transitions `(tab_id, new_status)` for tests and diagnostics (`#683` §10).
    pub status_changes: Vec<(usize, AgentTabStatus)>,
    /// Permission mode for newly spawned agent tabs (`Skip` vs `Ask`, `#656`).
    pub permission_mode: crate::agent_permissions::AgentPermissionMode,
    /// Queued in-panel approval requests read from `<session_dir>/approvals/` (`#656`).
    pub pending_approvals: Vec<crate::agent_permissions::ApprovalRequest>,
    /// Latch ensuring at most one OS notification per waiting episode (`#656`).
    pub notification_latch: crate::agent_permissions::NotificationLatch,
    /// Pluggable OS notification sender (`#656`).
    pub approval_notifier: Arc<dyn crate::agent_permissions::ApprovalNotifier>,
    /// Tracks whether `agent-status.json` was already observed in the `Done` state so transitions fire once.
    pub last_agent_status_done: bool,
    /// Active `@`-mentions recorded for the current composer draft (`#655`).
    pub active_mentions: Vec<crate::agent_mention::Mention>,
    /// `@`-mention popup state (`#655`).
    pub mention_popup: crate::agent_mention_popup::MentionPopupState,
    /// Last observed byte offset of the caret inside the composer `TextEdit` (`#655`).
    pub composer_cursor_byte: Option<usize>,
}

impl Default for TerminalState {
    fn default() -> Self {
        #[cfg(test)]
        let tab_id = {
            use std::sync::atomic::{AtomicU64, Ordering};
            static NEXT_TEST_TAB_ID: AtomicU64 = AtomicU64::new(100_000);
            NEXT_TEST_TAB_ID.fetch_add(1, Ordering::Relaxed)
        };
        #[cfg(not(test))]
        let tab_id = 1;
        let mut st = Self::for_tab(tab_id);
        if cfg!(test) {
            st.live_spec_path = st.session_dir.join("live-agent-out-test-isolated.spec.json");
            st.last_watched_mtime = std::fs::metadata(&st.live_spec_path).and_then(|m| m.modified()).ok();
        }
        st
    }
}

impl Drop for TerminalState {
    fn drop(&mut self) {
        self.release_lock();
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
        let active_run_id = format!("{session_id}-1-1");
        let session_dir = std::env::temp_dir().join("designos-sessions").join(&session_id);
        let live_spec_path = session_dir.join("live-spec.json");
        let live_doc_sync_path = session_dir.join("doc-sync.designos");
        let live_canvas_path = session_dir.join("canvas.designos");
        let context_json_path = session_dir.join("context.json");
        let selection_spec_path = session_dir.join("selection.spec.json");
        let initial_spec_mtime = std::fs::metadata(&live_spec_path).and_then(|m| m.modified()).ok();
        let fallback_spec_mtime = std::fs::metadata("/tmp/designos-live-agent-out.spec.json").and_then(|m| m.modified()).ok();
        let live_patch = crate::live_patch_log::LivePatchLog::bind(session_dir.join("doc-patch.jsonl"));
        let turn_marks = crate::agent_turn_marks::TurnMarkLog::bind(session_dir.join("turn-marks.jsonl"));
        let event_stream = crate::agent_turn_status::AgentEventStream::bind(session_dir.join("agent-events.jsonl"));
        #[cfg(test)]
        let notifier = crate::agent_notify::AgentNotifier::with_sink(Arc::new(crate::agent_notify::StubNotificationSink::new()));
        #[cfg(not(test))]
        let notifier = crate::agent_notify::AgentNotifier::default();
        let approval_notifier: Arc<dyn crate::agent_permissions::ApprovalNotifier> =
            if cfg!(test) { Arc::new(crate::agent_permissions::RecordingNotifier::default()) } else { Arc::new(crate::agent_permissions::OsApprovalNotifier) };
        Self {
            key: crate::agent_sessions::SessionKey::Fallback(session_id.clone()),
            library_root: None,
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
            unattached_composer: crate::agent_composer::ComposerState::default(),
            status_msg: None,
            next_tab_id: 1,
            next_run_seq: 1,
            active_run_id,
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
            search_open: false,
            search_query: String::new(),
            search_case_sensitive: false,
            chrome_focused: false,
            pending_stop_fold_depth: None,
            pending_stop_close: None,
            current_undo_depth: 0,
            #[cfg(test)]
            platform_is_mac_override: None,
            #[cfg(test)]
            cli_uses_ctrl_z_override: None,
            #[cfg(test)]
            path_override: None,
            session_registry: crate::agent_sessions::SessionRegistry::default(),
            lock_owner: None,
            open_file_binding: None,
            codex_cwd_scoped: false,
            agy_cwd_scoped: false,
            path_prepend: Vec::new(),
            picker: crate::agent_picker::AgentPickerState::default(),
            picker_open: false,
            sessions_popover_open: false,
            renaming_session_tab_id: None,
            rename_session_input: String::new(),
            pending_prompt: None,
            turn_marks,
            turn_ledger: crate::agent_turn_ledger::AgentTurnLedger::default(),
            pending_turn_revert: None,
            event_stream,
            notifier,
            window_focused: true,
            surface_visible: true,
            status_changes: Vec::new(),
            permission_mode: crate::agent_permissions::AgentPermissionMode::Skip,
            pending_approvals: Vec::new(),
            notification_latch: crate::agent_permissions::NotificationLatch::default(),
            approval_notifier,
            last_agent_status_done: false,
            active_mentions: Vec::new(),
            mention_popup: crate::agent_mention_popup::MentionPopupState::default(),
            composer_cursor_byte: None,
        }
    }

    pub fn set_window_focused(&mut self, focused: bool) {
        self.window_focused = focused;
    }

    pub fn set_approval_notifier(&mut self, notifier: Arc<dyn crate::agent_permissions::ApprovalNotifier>) {
        self.approval_notifier = notifier;
    }

    /// True when the active tab has a pending approval request or is waiting for approval (`#656`).
    pub fn needs_approval(&self) -> bool {
        if !self.pending_approvals.is_empty() {
            return true;
        }
        self.active_tab().is_some_and(|t| t.needs_approval())
    }

    fn withdraw_tab_approvals(session_dir: &Path, tab: &mut TerminalTab) {
        crate::agent_permissions::withdraw_pending_requests_in_dir(&tab.approvals_dir);
        for req in tab.pending_approvals.drain(..) {
            let root_req = crate::agent_permissions::approvals_dir(session_dir).join(format!("{}.request.json", req.id));
            if root_req.exists() {
                let _ = crate::agent_permissions::write_approval_answer(session_dir, &req.id, crate::agent_permissions::ApprovalDecision::Withdrawn);
            }
        }
    }

    fn sync_active_tab_approvals(&mut self) {
        self.pending_approvals = self.active_tab().map(|t| t.pending_approvals.clone()).unwrap_or_default();
    }

    /// Writes `decision` (`Allow` or `Deny`) for `request_id` into the active tab's approvals folder
    /// (and `<session_dir>/approvals/<id>.answer` if present) and removes the answered entry (`#656`).
    pub fn answer_pending_approval(&mut self, request_id: &str, decision: crate::agent_permissions::ApprovalDecision) {
        let mut wrote_tab = false;
        if let Some(tab) = self.active_tab_mut() {
            if tab.approvals_dir.join(format!("{request_id}.request.json")).exists() {
                let _ = crate::agent_permissions::write_approval_answer_in_dir(&tab.approvals_dir, request_id, decision);
                wrote_tab = true;
            }
            tab.pending_approvals.retain(|r| r.id != request_id);
        }
        let root_req = crate::agent_permissions::approvals_dir(&self.session_dir).join(format!("{request_id}.request.json"));
        if !wrote_tab || root_req.exists() {
            let _ = crate::agent_permissions::write_approval_answer(&self.session_dir, request_id, decision);
        }
        self.pending_approvals.retain(|r| r.id != request_id);
        if self.pending_approvals.is_empty() {
            if let Some(tab) = self.active_tab() {
                if let Ok(mut st) = tab.agent_status.lock() {
                    if matches!(*st, AgentTabStatus::WaitingForInput(_)) {
                        *st = AgentTabStatus::Idle;
                    }
                }
            }
            self.notification_latch.reset();
        }
    }

    /// Handles a click on the in-panel approval card (`Approve`, `Deny`, or `Open tab`).
    pub fn handle_approval_card_action(&mut self, action: crate::agent_approval_card::CardAction) {
        match action {
            crate::agent_approval_card::CardAction::Approve(id) => {
                self.answer_pending_approval(&id, crate::agent_permissions::ApprovalDecision::Allow);
            }
            crate::agent_approval_card::CardAction::Deny(id) => {
                self.answer_pending_approval(&id, crate::agent_permissions::ApprovalDecision::Deny);
            }
            crate::agent_approval_card::CardAction::OpenTab => {
                self.side_panel_pty_open = true;
                self.chat_mode = false;
                self.viewport_focused = true;
            }
        }
    }

    /// Populates the approval card state for the `--panel agents-approval-*` screenshot fixtures (`#656`).
    pub fn open_approval_fixture(&mut self, preset_slug: &str, file_path: Option<&Path>) {
        self.permission_mode = crate::agent_permissions::AgentPermissionMode::Ask;
        self.ensure_default_tab(file_path);
        if let Some(tab) = self.active_tab_mut() {
            tab.permission_mode = crate::agent_permissions::AgentPermissionMode::Ask;
        }
        let now = crate::agent_sessions::unix_timestamp_secs();
        let approvals_dir = crate::agent_permissions::approvals_dir(&self.session_dir);
        let _ = std::fs::create_dir_all(&approvals_dir);
        match preset_slug {
            "agents-approval-command" => {
                if let Some(tab) = self.active_tab_mut() {
                    tab.preset = AgentCliPreset::Claude;
                    tab.title = "Claude CLI #1".into();
                }
                let req = serde_json::json!({
                    "id": "1001-Bash",
                    "tool_name": "Bash",
                    "summary": "cargo test -p designos-app --bin designos",
                    "timestamp": now,
                });
                let _ = std::fs::write(approvals_dir.join("1001-Bash.request.json"), req.to_string());
                let _ = self.poll_live_agent_bridge();
            }
            "agents-approval-queued" => {
                if let Some(tab) = self.active_tab_mut() {
                    tab.preset = AgentCliPreset::Claude;
                    tab.title = "Claude CLI #1".into();
                }
                for (id, tool, summary, ts) in [
                    ("1001-Bash", "Bash", "git status --short", now),
                    ("1002-Edit", "Edit", "crates/designos-app/src/terminal.rs", now + 1),
                    ("1003-Write", "Write", "docs/specs/v0.6/656-agents-ask-permission-mode-with-in-panel-approvals.md", now + 2),
                ] {
                    let req = serde_json::json!({
                        "id": id,
                        "tool_name": tool,
                        "summary": summary,
                        "timestamp": ts,
                    });
                    let _ = std::fs::write(approvals_dir.join(format!("{id}.request.json")), req.to_string());
                }
                let _ = self.poll_live_agent_bridge();
            }
            "agents-approval-open-tab" => {
                if let Some(tab) = self.active_tab_mut() {
                    tab.preset = AgentCliPreset::Grok;
                    tab.title = "Grok CLI #1".into();
                    if let Ok(mut st) = tab.agent_status.lock() {
                        *st = AgentTabStatus::WaitingForInput(Some("Needs approval".into()));
                    }
                }
            }
            _ => {}
        }
    }

    /// Returns the turn ledger for `tab_idx` (or the fallback ledger when no tabs are spawned).
    pub fn ledger_for_tab(&self, tab_idx: usize) -> &crate::agent_turn_ledger::AgentTurnLedger {
        self.tabs.get(tab_idx).map_or(&self.turn_ledger, |t| &t.turn_ledger)
    }

    /// Mutable access to the turn ledger for `tab_idx` (or the fallback ledger when no tabs are spawned).
    pub fn ledger_for_tab_mut(&mut self, tab_idx: usize) -> &mut crate::agent_turn_ledger::AgentTurnLedger {
        if let Some(tab) = self.tabs.get_mut(tab_idx) {
            &mut tab.turn_ledger
        } else {
            &mut self.turn_ledger
        }
    }

    /// Returns the active tab's turn ledger (or the fallback ledger when no tabs are spawned).
    pub fn active_turn_ledger(&self) -> &crate::agent_turn_ledger::AgentTurnLedger {
        let idx = self.active_tab.min(self.tabs.len().saturating_sub(1));
        self.ledger_for_tab(idx)
    }

    /// Mutable access to the active tab's turn ledger (or the fallback ledger when no tabs are spawned).
    pub fn active_turn_ledger_mut(&mut self) -> &mut crate::agent_turn_ledger::AgentTurnLedger {
        let idx = self.active_tab.min(self.tabs.len().saturating_sub(1));
        self.ledger_for_tab_mut(idx)
    }

    /// True when `tab_idx` is currently in [`AgentTabStatus::Working`].
    pub fn is_tab_working(&self, tab_idx: usize) -> bool {
        self.tabs.get(tab_idx).is_some_and(|t| matches!(t.agent_status.lock().ok().as_deref(), Some(AgentTabStatus::Working(_))))
    }

    /// Creates a `TerminalState` bound to `file_id` (and `lib_root` when provided) with `doc_name`.
    pub fn new_for_session(file_id: &str, doc_name: &str, lib_root: Option<&Path>) -> Self {
        let mut st = Self::for_tab(1);
        st.doc_name = doc_name.to_string();
        if let Some(root) = lib_root {
            let win_id = format!("p{}-{}", std::process::id(), file_id);
            st.bind_library_file(root, file_id, &win_id);
        } else {
            st.bind_session(file_id);
        }
        st
    }

    /// Returns true when `run_id` matches a live agent PTY tab in this terminal state.
    pub fn is_live_agent_run(&mut self, run_id: &str) -> bool {
        self.tabs.iter_mut().any(|t| !t.is_shell_process() && (t.run_id == run_id || self.session_id == run_id) && t.is_alive())
    }

    fn active_mcp_runtime_name(&self) -> &'static str {
        self.active_tab()
            .filter(|t| !t.is_shell_process())
            .map(|t| t.spawn_preset.binary_name())
            .or_else(|| (!matches!(self.active_runtime, AgentCliPreset::Shell | AgentCliPreset::DesignOsHelp)).then(|| self.active_runtime.binary_name()))
            .unwrap_or("claude")
    }

    fn update_session_dir_paths(&mut self, session_dir: PathBuf) {
        self.active_run_id = format!("{}-1-{}", self.session_id, self.next_run_seq.max(1));
        self.live_spec_path = session_dir.join("live-spec.json");
        self.live_doc_sync_path = session_dir.join("doc-sync.designos");
        self.live_canvas_path = session_dir.join("canvas.designos");
        self.context_json_path = session_dir.join("context.json");
        self.selection_spec_path = session_dir.join("selection.spec.json");
        self.session_dir = session_dir;
        self.last_watched_mtime = std::fs::metadata(&self.live_spec_path).and_then(|m| m.modified()).ok();
        self.live_patch = crate::live_patch_log::LivePatchLog::bind(self.session_dir.join("doc-patch.jsonl"));
        self.turn_marks = crate::agent_turn_marks::TurnMarkLog::bind(self.session_dir.join("turn-marks.jsonl"));
        self.event_stream = crate::agent_turn_status::AgentEventStream::bind(self.session_dir.join("agent-events.jsonl"));
        self.last_synced_key = None;
        self.pending_approvals.clear();
        for tab in &mut self.tabs {
            tab.approvals_dir = crate::agent_permissions::tab_approvals_dir(&self.session_dir, tab.id);
            tab.pending_approvals.clear();
        }
        crate::agent_permissions::cleanup_stale_approvals(&self.session_dir, crate::agent_sessions::unix_timestamp_secs());
        self.last_agent_status_done = false;
    }

    /// Re-binds this terminal state to `session_id` under `/tmp/designos-sessions/<session_id>`.
    /// When already bound to a library file and holding its `session.lock`, updates `self.session_id`
    /// while preserving `<lib_root>/agent-sessions/<file_id>` as `self.session_dir`.
    pub fn bind_session(&mut self, session_id: &str) {
        let clean = session_id.trim();
        if clean.is_empty() {
            return;
        }
        if matches!(self.key, crate::agent_sessions::SessionKey::LibraryFile { .. }) {
            self.session_id = clean.to_string();
            self.active_run_id = format!("{clean}-1-{}", self.next_run_seq.max(1));
            if self.lock_owner.is_none() {
                let fallback_dir = std::env::temp_dir().join("designos-sessions").join(clean);
                if self.session_dir != fallback_dir {
                    self.update_session_dir_paths(fallback_dir);
                }
            }
            return;
        }
        if self.session_id == clean {
            return;
        }
        let session_dir = std::env::temp_dir().join("designos-sessions").join(clean);
        self.session_id = clean.to_string();
        self.key = crate::agent_sessions::SessionKey::Fallback(clean.to_string());
        self.update_session_dir_paths(session_dir);
    }

    /// Binds this terminal state to `<lib_root>/agent-sessions/<file_id>/` (`#653`), acquiring
    /// `session.lock` for `window_id` or falling back to `/tmp/designos-sessions/<window_id>` if
    /// another live Studio window already holds the lock.
    pub fn bind_library_file(&mut self, lib_root: &Path, file_id: &str, window_id: &str) {
        self.bind_library_file_with_liveness(lib_root, file_id, window_id, crate::agent_sessions::is_pid_alive);
    }

    /// Like [`Self::bind_library_file`], with a custom `pid_alive` predicate for unit tests.
    pub fn bind_library_file_with_liveness<F>(&mut self, lib_root: &Path, file_id: &str, window_id: &str, pid_alive: F)
    where
        F: Fn(u32) -> bool,
    {
        self.library_root = Some(lib_root.to_path_buf());
        let clean_file_id = file_id.trim();
        if clean_file_id.is_empty() {
            return;
        }
        let win = if window_id.trim().is_empty() { self.session_id.clone() } else { window_id.trim().to_string() };
        let target_key = crate::agent_sessions::SessionKey::LibraryFile { lib_root: lib_root.to_path_buf(), file_id: clean_file_id.to_string() };
        if self.key == target_key && self.lock_owner.as_deref() == Some(win.as_str()) {
            return;
        }
        // Read after the already-bound early return so a steady frame never touches settings.json;
        // a mode another window saves later is picked up when an agent starts (`refresh_permission_mode_from_disk`).
        self.permission_mode = crate::agent_permissions::load_permission_mode(lib_root);
        if self.key != target_key {
            self.shutdown();
            self.next_tab_id = 1;
        } else {
            self.release_lock();
        }
        self.session_id = win.clone();
        match crate::agent_sessions::acquire_stable_session_dir_with(lib_root, clean_file_id, &win, &win, pid_alive) {
            crate::agent_sessions::LockOutcome::Acquired(dir) => {
                self.key = target_key;
                self.lock_owner = Some(win);
                self.update_session_dir_paths(dir.clone());
                self.session_registry = crate::agent_sessions::load_or_init_registry(&dir);
                if let Some(max_id) = self.session_registry.sessions.iter().map(|s| s.tab_id as usize).max() {
                    self.next_tab_id = self.next_tab_id.max(max_id + 1);
                }
            }
            crate::agent_sessions::LockOutcome::Fallback { dir, message } => {
                self.key = target_key;
                self.lock_owner = None;
                self.update_session_dir_paths(dir);
                self.session_registry = crate::agent_sessions::SessionRegistry::default();
                self.status_msg = Some(message);
            }
        }
    }

    /// Re-reads the saved permission mode of the bound library so a mode another window (a separate
    /// process) saved applies to the agent about to start. Called per spawn or prompt, never per frame.
    /// Without a library, or without a saved mode, the in-memory mode stays.
    fn refresh_permission_mode_from_disk(&mut self) {
        if let Some(mode) = self.library_root.as_deref().and_then(crate::agent_permissions::load_saved_permission_mode) {
            self.permission_mode = mode;
        }
    }

    /// Binds to the stable folder of the open file (`#653` rules 1 and 2): the library file id when
    /// there is one, else the hash key of its disk path ([`crate::agent_sessions::disk_file_session_key`]).
    /// A document with neither (an unsaved scratch, or an `.svg` opened over a file) gets the
    /// per-process temp folder again, so it never inherits the previous file's session.
    ///
    /// Runs every painted frame while no agent tab is open, so it does no filesystem work when the
    /// inputs are the ones it last bound: the path key is computed once per path, and a transient
    /// `canonicalize` failure (an external unlink + rename save) cannot flip an already-bound key.
    pub fn bind_open_file(&mut self, lib_root: &Path, lib_file_id: Option<&str>, disk_path: Option<&Path>, window_id: &str) {
        let cached = self.open_file_binding.as_ref().filter(|b| b.same_inputs(lib_root, lib_file_id, disk_path, window_id));
        if let Some(b) = cached {
            let still_bound =
                matches!(&self.key, crate::agent_sessions::SessionKey::LibraryFile { lib_root: r, file_id } if r == lib_root && *file_id == b.file_id);
            if still_bound && self.lock_owner.is_some() {
                return;
            }
        }
        let file_id = match (lib_file_id, disk_path) {
            (Some(id), _) => id.to_string(),
            (None, Some(path)) => match cached {
                Some(b) => b.file_id.clone(),
                None => crate::agent_sessions::disk_file_session_key(path),
            },
            (None, None) => {
                self.open_file_binding = None;
                if matches!(self.key, crate::agent_sessions::SessionKey::LibraryFile { .. }) {
                    self.unbind_library_file(window_id);
                }
                return;
            }
        };
        self.bind_library_file(lib_root, &file_id, window_id);
        self.open_file_binding = Some(crate::agent_sessions::OpenFileBinding {
            lib_root: lib_root.to_path_buf(),
            lib_file_id: lib_file_id.map(str::to_string),
            disk_path: disk_path.map(Path::to_path_buf),
            window_id: window_id.to_string(),
            file_id,
        });
    }

    /// Releases `<session_dir>/session.lock` if owned by this `TerminalState`.
    pub fn release_lock(&mut self) {
        if let Some(owner) = self.lock_owner.take() {
            crate::agent_sessions::release_session_lock(&self.session_dir, &owner);
        }
    }

    /// Detaches from any library file and binds to an ephemeral fallback `/tmp/designos-sessions/<session_id>`.
    pub fn unbind_library_file(&mut self, session_id: &str) {
        let clean = if session_id.trim().is_empty() { self.session_id.clone() } else { session_id.trim().to_string() };
        let target_key = crate::agent_sessions::SessionKey::Fallback(clean.clone());
        if self.key != target_key {
            self.shutdown();
            self.next_tab_id = 1;
        } else {
            self.release_lock();
        }
        self.key = target_key;
        self.session_id = String::new();
        self.session_registry = crate::agent_sessions::SessionRegistry::default();
        self.bind_session(&clean);
    }

    /// Builds a [`SessionEnv`] snapshot for this terminal state's per-file session.
    pub fn session_env(&self) -> SessionEnv {
        SessionEnv {
            session_id: self.session_id.clone(),
            run_id: self.active_run_id.clone(),
            agent_runtime: self.active_mcp_runtime_name().to_string(),
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
            path_prepend: self.path_prepend.clone(),
            #[cfg(test)]
            path_override: self.path_override.clone(),
        }
    }

    /// Returns `(session_id, agent_label)` for the active agent tab (or `active_runtime` when no agent tab is open).
    pub fn active_agent_session_identity(&self) -> (String, String) {
        let id = if self.session_id.trim().is_empty() { "default".to_string() } else { self.session_id.clone() };
        let label = self
            .tabs
            .get(self.active_tab)
            .filter(|t| !matches!(t.spawn_preset, AgentCliPreset::Shell | AgentCliPreset::DesignOsHelp))
            .map(|t| t.spawn_preset.short_agent_name())
            .unwrap_or_else(|| self.active_runtime.short_agent_name());
        (id, label.to_string())
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
        for tab in &mut self.tabs {
            tab.doc_name = self.doc_name.clone();
        }

        // The canvas the agent reads must keep holding every edit the agent made. While one of its
        // edits waits for the user to finish, or the user is mid-edit, or deferred auto-layout
        // subtrees are still draining, the editor's document is not settled yet, so publishing now
        // would make the agent's next call lose its own edit or read a half-solved tree.
        // `last_synced_key` stays as it was, so the first fully drained idle frame publishes.
        if self.edit_in_progress || self.live_patch.has_held() || doc.relayout_skipped_count() > 0 {
            return;
        }

        // Selected ids and active in_progress flags: flag changes through `Document::set_in_progress` and the clear
        // helpers bump `mutation_epoch`, but `doc.in_progress` is a public map. Folding it into `selection_key` keeps
        // a flag written without a bump from going unpublished to `canvas.designos`.
        let selection_key = {
            use std::hash::{Hash, Hasher};
            let mut h = std::collections::hash_map::DefaultHasher::new();
            selection.hash(&mut h);
            doc.in_progress.len().hash(&mut h);
            for (nid, ip) in &doc.in_progress {
                nid.hash(&mut h);
                ip.agent.hash(&mut h);
                ip.run_id.hash(&mut h);
                ip.status.hash(&mut h);
                ip.since_ms.hash(&mut h);
                ip.stale_after_s.hash(&mut h);
            }
            h.finish()
        };
        let key = (doc.mutation_epoch(), page.0, selection_key);
        if self.last_synced_key != Some(key) {
            // The agent may have appended an edit the editor has not read yet (the log is polled
            // after this runs in a frame). A canvas written now would lack it, and the agent's
            // session would take that for the user undoing it. Ask before the slow serialization
            // (so a held publish throws nothing away), again after it, and a last time between
            // writing the canvas file and renaming it into place (a file's modification time is its
            // last write, so that is the moment that orders it against the agent's append).
            // `last_synced_key` stays as it was: the same frame polls the log, applies the edit and
            // publishes again.
            if self.live_patch.has_unread_complete_line() {
                return;
            }
            let selected_ids: Vec<u64> = selection.iter().map(|n| n.0).collect();
            let is_scoped = doc.nodes.len() > 25_000;
            let ctx_val = serde_json::json!({
                "session_id": self.session_id,
                "run_id": self.active_run_id,
                "agent_runtime": self.active_mcp_runtime_name(),
                "doc_name": self.doc_name,
                "file_path": file_path.map(|p| p.display().to_string()),
                "current_page_id": page.0,
                "current_page_name": self.current_page_name,
                "selected_node_ids": selected_ids,
                "scoped": is_scoped,
            });
            let canvas_json =
                if !is_scoped { doc.to_json().ok() } else { crate::live_canvas_scope::scoped_canvas(doc, page, selection, &self.doc_name).to_json().ok() };

            // Checked again: the serialization above takes time.
            if self.live_patch.has_unread_complete_line() {
                return;
            }

            if let Ok(ctx_bytes) = serde_json::to_vec_pretty(&ctx_val) {
                let _ = atomic_write_session_file(&self.context_json_path, &ctx_bytes);
                if !cfg!(test) {
                    let _ = atomic_write_session_file(Path::new("/tmp/designos-live-context.json"), &ctx_bytes);
                }
            }

            if let Some(json_str) = canvas_json {
                let live_patch = &self.live_patch;
                if let Ok(true) = write_session_file_unless(&self.live_canvas_path, json_str.as_bytes(), || {
                    run_before_rename_test_hook();
                    live_patch.has_unread_complete_line()
                }) {
                    return;
                }
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
        let mcp_config_path = self.session_dir.join("mcp-config.json");

        // The default (every tool) is never written down: an absent `designos` key means "all", so a tool a later
        // build adds is on for sessions that survive an update. A user-edited `enabled_tools` list is kept across
        // each page or selection sync, together with the `known_tools` snapshot that lets new tools be switched on.
        let existing_config = std::fs::read_to_string(&mcp_config_path).ok();
        let custom_tools = designos_mcp::rules::reconcile_designos_block(existing_config.as_deref());

        let mut mcp_config = serde_json::json!({
            "mcpServers": {
                "designos": {
                    "command": mcp_bin.display().to_string(),
                    "args": ["--workspace", workspace_dir.display().to_string()],
                    "env": {
                        "DESIGNOS_SESSION_DIR": self.session_dir.display().to_string(),
                        "DESIGNOS_MCP_CONFIG": mcp_config_path.display().to_string(),
                        "DESIGNOS_AGENT_RUN_ID": self.active_run_id,
                        "DESIGNOS_AGENT_RUNTIME": self.active_mcp_runtime_name(),
                        "DESIGNOS_IN_PROGRESS_STALE_MINUTES": designos_doc::in_progress_stale_minutes().to_string(),
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
        if let (Some(custom), Some(root)) = (&custom_tools, mcp_config.as_object_mut()) {
            root.insert("designos".into(), custom.block.clone());
        }
        let mcp_config_bytes = serde_json::to_vec_pretty(&mcp_config).ok();
        if let Some(bytes) = &mcp_config_bytes {
            let _ = atomic_write_session_file(&mcp_config_path, bytes);
            write_agy_session_plugin(&self.session_dir, bytes);
        }
        write_gemini_session_config(&self.session_dir, &mcp_config);
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
        let approve_script_path = (self.permission_mode == crate::agent_permissions::AgentPermissionMode::Ask)
            .then(|| crate::agent_permissions::write_approval_hook_script(&self.session_dir));
        let claude_hooks = session_hooks_settings_with_approval(&status_path, Some(&context_reply_path), approve_script_path.as_deref());
        if let Ok(bytes) = serde_json::to_vec_pretty(&claude_hooks) {
            let _ = atomic_write_session_file(&self.session_dir.join("hooks.json"), &bytes);
        }
        if let Ok(bytes) = serde_json::to_vec_pretty(&session_hooks_settings(&status_path, None)) {
            let _ = atomic_write_session_file(&self.session_dir.join(".grok").join("hooks").join("designos-status.json"), &bytes);
        }
        if let Ok(bytes) = serde_json::to_vec_pretty(&agy_session_hooks(&status_path)) {
            let _ = atomic_write_session_file(&self.session_dir.join(".agents").join("hooks.json"), &bytes);
        }

        // The session `SKILL.md` carries the rules pack for the tools this session enables.
        let rules_body: std::borrow::Cow<'static, str> = match &custom_tools {
            Some(custom) => designos_mcp::rules::generate_rules_pack(Some(&custom.enabled))
                .map(std::borrow::Cow::Owned)
                .unwrap_or(std::borrow::Cow::Borrowed(designos_mcp::rules::GENERATED_RULES_MD)),
            None => std::borrow::Cow::Borrowed(designos_mcp::rules::GENERATED_RULES_MD),
        };
        let skill_md = designos_mcp::rules::format_session_skill_md(
            &self.doc_name,
            &self.current_page_name,
            &self.session_id,
            self.current_page_id,
            &self.live_canvas_path,
            &self.context_json_path,
            &self.live_spec_path,
            &self.live_doc_sync_path,
            &self.live_patch.path,
            &self.selection_spec_path,
            &rules_body,
        );
        let _ = atomic_write_session_file(&self.session_dir.join("SKILL.md"), skill_md.as_bytes());
        let _ = atomic_write_session_file(&self.session_dir.join(".claude/skills/designos-studio/SKILL.md"), skill_md.as_bytes());
        let _ = atomic_write_session_file(&self.session_dir.join(".agents/skills/designos-studio/SKILL.md"), skill_md.as_bytes());
        // Codex, Grok, Antigravity, Gemini CLI, and OpenCode read `AGENTS.md` / `GEMINI.md` from their cwd (the session dir).
        let _ = atomic_write_session_file(&self.session_dir.join("AGENTS.md"), skill_md.as_bytes());
        let _ = atomic_write_session_file(&self.session_dir.join("GEMINI.md"), skill_md.as_bytes());
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

    /// Test helper: polls the bridge until the active tab records a start failure or `budget` passes.
    ///
    /// A start only counts as failed when the CLI exits within `START_FAILURE_WINDOW` of its spawn. A
    /// loaded parallel run can take longer than that window just to start the stub's shell, so each
    /// poll restarts the clock of every tab that is still running: the exit is then judged on how long
    /// ago the tab was last seen alive, which is what these tests mean by "exits at once".
    #[cfg(test)]
    pub(crate) fn poll_until_start_failure(&mut self, budget: std::time::Duration) -> bool {
        crate::test_timing::poll_until(budget, || {
            for tab in &mut self.tabs {
                if matches!(tab.child.try_wait(), Ok(None)) {
                    tab.spawned_at = std::time::Instant::now();
                }
            }
            let _ = self.poll_live_agent_bridge();
            self.active_tab().is_some_and(|t| t.start_failure.is_some())
        })
    }

    /// Runs the fix the user clicked on a tab's "failed to start" panel.
    pub fn apply_start_fix(&mut self, fix: FixAction, file_path: Option<&Path>) {
        match fix {
            FixAction::OpenInstallPage(url) => {
                if cfg!(test) {
                    self.status_msg = Some(format!("Open {url} in your browser"));
                } else if let Err(e) = crate::home::open_link(url) {
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
            FixAction::StartNewSession(_preset) => {
                if self.active_tab().is_some_and(|t| t.start_failure.is_some()) {
                    let idx = self.active_tab.min(self.tabs.len().saturating_sub(1));
                    self.close_tab(idx);
                }
                self.picker_open = true;
            }
        }
    }

    /// Terminates all child PTY processes owned by this file's terminal state.
    pub fn shutdown(&mut self) {
        crate::agent_permissions::withdraw_pending_requests(&self.session_dir);
        self.pending_approvals.clear();
        let mut reg_dirty = false;
        let now = crate::agent_sessions::unix_timestamp_secs();
        for mut tab in self.tabs.drain(..) {
            let _ = tab.child.kill();
            if let Some(entry) = self.session_registry.find_entry_mut(tab.id as u64, tab.session_uuid.as_deref()) {
                if entry.status == crate::agent_sessions::SessionEntryStatus::Active {
                    entry.status = crate::agent_sessions::SessionEntryStatus::Exited;
                    entry.last_active_at = now;
                    reg_dirty = true;
                }
            }
        }
        self.active_tab = 0;
        if reg_dirty {
            let _ = crate::agent_sessions::save_registry(&self.session_dir, &self.session_registry);
        }
        self.release_lock();
    }

    /// Returns `true` when the embedded terminal viewport is actively capturing raw keyboard input,
    /// so single-key canvas shortcuts (`V`, `F`, `R`, `T`, `Backspace`, etc.) must be suppressed.
    pub fn is_capturing_keyboard(&self, agent_panel_open: bool) -> bool {
        self.mention_popup.is_open() || (((agent_panel_open && self.side_panel_pty_open) || self.dock_open) && self.viewport_focused)
    }

    /// Returns the currently active terminal tab when one exists.
    pub fn focused_tab(&self) -> Option<&TerminalTab> {
        self.tabs.get(self.active_tab.min(self.tabs.len().saturating_sub(1)))
    }

    /// Returns `(is_mac, tab_is_agent, cli_uses_ctrl_z)` for [`crate::terminal_undo_chord::terminal_undo_chord`].
    pub fn undo_chord_context(&self) -> (bool, bool, bool) {
        let is_mac = {
            #[cfg(test)]
            {
                self.platform_is_mac_override.unwrap_or(cfg!(target_os = "macos"))
            }
            #[cfg(not(test))]
            {
                cfg!(target_os = "macos")
            }
        };
        let preset = self.focused_tab().map(|t| t.preset).unwrap_or(self.active_runtime);
        let tab_is_agent = preset.is_agent();
        let cli_uses_ctrl_z = {
            #[cfg(test)]
            {
                self.cli_uses_ctrl_z_override.unwrap_or_else(|| preset.uses_ctrl_z())
            }
            #[cfg(not(test))]
            {
                preset.uses_ctrl_z()
            }
        };
        (is_mac, tab_is_agent, cli_uses_ctrl_z)
    }

    /// Ensures at least one interactive Shell PTY tab is running (`DIR-29`). Never resumes a recorded
    /// session: that happens only on open via `open_for_agent_use` (`#651` §3 rules 2 and 9).
    pub fn ensure_default_tab(&mut self, file_path: Option<&Path>) {
        if self.tabs.is_empty() {
            let _ = self.spawn_tab(AgentCliPreset::Shell, file_path);
        }
    }

    /// Switches the active CLI runtime (`Claude Code`, `Codex`, `Antigravity CLI`, `Grok`, `Shell`)
    /// and binds the active PTY tab label to that runtime (`DIR-21`).
    ///
    /// Never types an `export` into the tab: when it hosts an agent TUI the line lands as a chat
    /// prompt. `DESIGNOS_AGENT_RUNTIME` is set at spawn (`configure_pty_env`) instead.
    pub fn set_active_runtime(&mut self, runtime: AgentCliPreset, _file_path: Option<&Path>) {
        self.active_runtime = runtime;
        let active_idx = self.active_tab;
        if let Some(tab) = self.active_tab_mut() {
            tab.preset = runtime;
            tab.title = format!("{} #{}", runtime.tab_title(), (active_idx + 1).max(tab.id));
        }
        self.status_msg = Some(format!("Active agent CLI shell: {} ({})", runtime.tab_title(), runtime.binary_name()));
    }

    /// Submits a user message from the Agent composer into the active tab's live CLI session (`#689` §3 rules 1, 8).
    pub fn submit_chat_prompt(&mut self, prompt: &str, file_path: Option<&Path>) {
        let _ = self.send_to_agent(prompt, file_path);
    }

    /// Submits a system-generated image task prompt to the target agent runtime (`#667`).
    ///
    /// Picks a live tab that was started as `preset`; otherwise starts a new tab running that agent CLI. An
    /// existing tab is never relabelled, so a user's Shell or another CLI keeps its identity and never receives
    /// the prompt. The prompt is queued while a fresh tab is still starting.
    pub fn submit_system_image_prompt(&mut self, preset: AgentCliPreset, prompt: &str, file_path: Option<&Path>) {
        let reusable =
            self.tabs.iter_mut().position(|t| t.spawn_preset == preset && t.start_failure.is_none() && !t.exited_notice_shown && t.accepts_agent_input());
        self.active_runtime = preset;
        match reusable {
            Some(idx) => self.active_tab = idx,
            None => {
                if self.spawn_tab(preset, file_path).is_err() {
                    return;
                }
            }
        }
        if let Some(tab) = self.active_tab() {
            if let Ok(mut st) = tab.agent_status.lock() {
                *st = AgentTabStatus::Working(Some(prompt.chars().take(64).collect()));
            }
        }
        let _ = self.send_system_prompt(prompt, file_path);
    }

    /// Records `preset` as the last chosen agent CLI in this file's `sessions.json` (`#652` §3 rule 4).
    /// Only built-in agent CLIs (`claude`, `codex`, `grok`, `agy`, `gemini`, `opencode`) are recorded; `Shell` and custom
    /// commands never become the default.
    pub fn record_last_cli(&mut self, preset: AgentCliPreset) {
        if crate::agent_sessions::is_agent_cli_preset(preset) {
            self.session_registry.last_cli = Some(preset.binary_name().to_string());
            let _ = crate::agent_sessions::save_registry(&self.session_dir, &self.session_registry);
        }
    }

    /// Spawns a custom CLI command tab (`"Other CLI..."` / `custom_clis`) in this file's session folder
    /// with the `DESIGNOS_*` environment, without registering a resumable session (`#652` §3 rule 7).
    pub fn spawn_custom_cli_tab(&mut self, label: &str, command: &str, file_path: Option<&Path>) -> Result<usize, String> {
        let id = self.next_tab_id.max(1);
        self.next_tab_id = id + 1;
        let _ = std::fs::create_dir_all(&self.session_dir);
        self.provision_session_agent_files(file_path);
        let env = self.session_env();
        let mut cmd = build_agent_shell_cmd(&default_shell_program(), command);
        #[cfg(test)]
        isolate_zsh_from_home(&mut cmd);
        cmd.cwd(&self.session_dir);

        let clean_title = if label.trim().is_empty() { command.trim() } else { label.trim() };
        match TerminalTab::spawn_with_session_cmd(id, AgentCliPreset::Claude, 28, 100, file_path, Some(&env), Some(cmd)) {
            Ok(mut tab) => {
                tab.preset = AgentCliPreset::Shell;
                tab.title = clean_title.to_string();
                if self.tabs.is_empty() {
                    tab.composer = std::mem::take(&mut self.unattached_composer);
                }
                let has_queued = tab.composer.pending_send.is_some();
                tab.spawned_command = Some(command.to_string());
                self.tabs.push(tab);
                self.active_tab = self.tabs.len() - 1;
                self.picker_open = false;
                self.sessions_popover_open = false;
                self.viewport_focused = true;
                self.status_msg = Some(format!("Spawned {} ({} · {})", clean_title, self.doc_name, self.current_page_name));
                if let Some((held_title, held_prompt)) = self.pending_prompt.take() {
                    if !has_queued {
                        self.dispatch_figma_ai_prompt(&held_title, &held_prompt, file_path);
                    }
                }
                Ok(self.active_tab)
            }
            Err(e) => {
                self.status_msg = Some(format!("PTY error: {e}"));
                Err(e)
            }
        }
    }

    /// Spawns a new tab with `preset` bound to this file's isolated [`SessionEnv`] and switches `active_tab` to it.
    /// For registrable agent CLIs, records a new entry in `sessions.json` (`#653`) and updates `last_cli` (`#652`).
    pub fn spawn_tab(&mut self, preset: AgentCliPreset, file_path: Option<&Path>) -> Result<usize, String> {
        self.refresh_permission_mode_from_disk();
        if crate::agent_sessions::is_agent_cli_preset(preset) {
            let session_uuid =
                if crate::agent_sessions::uses_preassigned_session_id(preset) { Some(crate::agent_sessions::generate_session_uuid()) } else { None };
            let launch_cmd = crate::agent_permissions::build_new_session_command_for_mode(preset, self.permission_mode, session_uuid.as_deref());
            self.spawn_tab_with_registration(preset, file_path, Some(&launch_cmd), session_uuid, false, None, true)
        } else {
            self.spawn_tab_with_registration(preset, file_path, None, None, false, None, false)
        }
    }

    /// Like [`Self::spawn_tab`]; with `command` the tab runs that one shell command (an agent CLI's own
    /// `login`) instead of the preset's launch command and does not register in `sessions.json`.
    fn spawn_tab_running(&mut self, preset: AgentCliPreset, file_path: Option<&Path>, command: Option<&str>) -> Result<usize, String> {
        match command {
            Some(cmd) => self.spawn_tab_with_registration(preset, file_path, Some(cmd), None, false, None, false),
            None => self.spawn_tab(preset, file_path),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn spawn_tab_with_registration(
        &mut self,
        preset: AgentCliPreset,
        file_path: Option<&Path>,
        command: Option<&str>,
        session_uuid: Option<String>,
        is_resumed: bool,
        custom_title: Option<String>,
        register_new_entry: bool,
    ) -> Result<usize, String> {
        self.refresh_permission_mode_from_disk();
        let id = self.next_tab_id.max(1);
        self.next_tab_id = id + 1;
        let run_seq = self.next_run_seq.max(1);
        self.next_run_seq = run_seq + 1;
        self.active_run_id = format!("{}-{}-{}", self.session_id, id, run_seq);
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
            Ok(mut tab) => {
                #[cfg(test)]
                if preset.is_agent()
                    && self.path_override.as_ref().is_some_and(|p| !runtime_binary_available_in(preset.binary_name(), Some(p.as_os_str()), false))
                {
                    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(2000);
                    while std::time::Instant::now() < deadline && tab.is_alive() {
                        std::thread::sleep(std::time::Duration::from_millis(15));
                    }
                    tab.spawned_at = std::time::Instant::now();
                }
                tab.session_uuid = session_uuid.clone();
                tab.is_resumed = is_resumed;
                tab.spawned_command = command.map(str::to_string);
                tab.permission_mode = self.permission_mode;
                tab.pty_waiting_heuristic.store(
                    self.permission_mode == crate::agent_permissions::AgentPermissionMode::Ask && preset == AgentCliPreset::Antigravity,
                    std::sync::atomic::Ordering::Release,
                );
                if let Some(title) = custom_title {
                    tab.title = title;
                }
                if self.tabs.is_empty() {
                    tab.composer = std::mem::take(&mut self.unattached_composer);
                }
                let has_queued = tab.composer.pending_send.is_some();
                if register_new_entry && crate::agent_sessions::is_agent_cli_preset(preset) {
                    self.session_registry.last_cli = Some(preset.binary_name().to_string());
                    if crate::agent_sessions::is_registrable_cli_scoped(preset, self.codex_cwd_scoped, self.agy_cwd_scoped) {
                        let now = crate::agent_sessions::unix_timestamp_secs();
                        let entry = crate::agent_sessions::SessionEntry {
                            tab_id: id as u64,
                            title: tab.title.clone(),
                            cli: preset.binary_name().to_string(),
                            session_id: session_uuid,
                            created_at: now,
                            last_active_at: now,
                            status: crate::agent_sessions::SessionEntryStatus::Active,
                            extra: std::collections::BTreeMap::new(),
                        };
                        if let Some(ref uuid) = entry.session_id {
                            self.session_registry.sessions.retain(|s| s.session_id.as_deref() != Some(uuid.as_str()));
                        } else {
                            self.session_registry.sessions.retain(|s| !(s.session_id.is_none() && s.tab_id == id as u64));
                        }
                        self.session_registry.sessions.push(entry);
                    }
                    let _ = crate::agent_sessions::save_registry(&self.session_dir, &self.session_registry);
                }
                self.tabs.push(tab);
                self.active_tab = self.tabs.len() - 1;
                self.picker_open = false;
                self.sessions_popover_open = false;
                self.sync_active_tab_approvals();
                self.viewport_focused = true;
                self.status_msg = Some(format!("Spawned {} ({} · {})", preset.tab_title(), self.doc_name, self.current_page_name));
                if let Some((held_title, held_prompt)) = self.pending_prompt.take() {
                    if !has_queued {
                        self.dispatch_figma_ai_prompt(&held_title, &held_prompt, file_path);
                    }
                }
                Ok(self.active_tab)
            }
            Err(e) => {
                self.status_msg = Some(format!("PTY error: {e}"));
                Err(e)
            }
        }
    }

    /// Returns the single most recently active non-`Missing` session in `self.session_registry`,
    /// regardless of whether its CLI currently supports safe resume.
    pub fn newest_recorded_session(&self) -> Option<&crate::agent_sessions::SessionEntry> {
        self.session_registry
            .sessions
            .iter()
            .filter(|e| e.status != crate::agent_sessions::SessionEntryStatus::Missing)
            .max_by(|a, b| (a.last_active_at, a.tab_id).cmp(&(b.last_active_at, b.tab_id)))
    }

    /// Returns the newest session in `self.session_registry` if and only if that newest entry
    /// can be safely resumed (`status != Missing` and a valid resume command exists for the CLI).
    /// Never skips an unresumable newest entry to return an older session.
    pub fn resumable_sessions(&self) -> Vec<&crate::agent_sessions::SessionEntry> {
        self.newest_recorded_session()
            .into_iter()
            .filter(|e| crate::agent_sessions::build_resume_command_scoped(e, self.codex_cwd_scoped, self.agy_cwd_scoped).is_some())
            .collect()
    }

    fn resume_session_entry(&mut self, entry_clone: &crate::agent_sessions::SessionEntry, file_path: Option<&Path>) -> Result<usize, String> {
        self.refresh_permission_mode_from_disk();
        let preset =
            crate::agent_sessions::preset_from_cli_name(&entry_clone.cli).ok_or_else(|| format!("Unsupported CLI '{}' in sessions.json", entry_clone.cli))?;
        let cmd_str = crate::agent_permissions::build_resume_command_for_mode(entry_clone, self.codex_cwd_scoped, self.agy_cwd_scoped, self.permission_mode)
            .ok_or_else(|| format!("CLI '{}' does not support safe session resume", entry_clone.cli))?;

        let tab_idx =
            self.spawn_tab_with_registration(preset, file_path, Some(&cmd_str), entry_clone.session_id.clone(), true, Some(entry_clone.title.clone()), false)?;
        let new_tab_id = self.tabs[tab_idx].id as u64;
        if let Some(entry) = self.session_registry.find_entry_mut(entry_clone.tab_id, entry_clone.session_id.as_deref()) {
            entry.tab_id = new_tab_id;
            entry.status = crate::agent_sessions::SessionEntryStatus::Active;
            entry.last_active_at = crate::agent_sessions::unix_timestamp_secs();
            let _ = crate::agent_sessions::save_registry(&self.session_dir, &self.session_registry);
        }
        Ok(tab_idx)
    }

    /// Resumes the recorded session with `entry_tab_id` from `self.session_registry`.
    pub fn resume_session_by_tab_id(&mut self, entry_tab_id: u64, file_path: Option<&Path>) -> Result<usize, String> {
        let entry_clone = self
            .session_registry
            .sessions
            .iter()
            .find(|e| e.tab_id == entry_tab_id && e.status != crate::agent_sessions::SessionEntryStatus::Missing)
            .cloned()
            .ok_or_else(|| format!("No resumable session with tab_id {entry_tab_id}"))?;
        self.resume_session_entry(&entry_clone, file_path)
    }

    /// Resumes only the single newest non-`Missing` session in `self.session_registry`.
    /// If the newest entry cannot be resumed, opens the new-session picker (`self.runtime_dropdown_open = true`)
    /// rather than skipping to an older entry.
    pub fn resume_last_session(&mut self, file_path: Option<&Path>) -> Result<usize, String> {
        let newest = self.newest_recorded_session().cloned().ok_or_else(|| "No previous agent session to resume".to_string())?;
        if crate::agent_sessions::build_resume_command_scoped(&newest, self.codex_cwd_scoped, self.agy_cwd_scoped).is_none() {
            self.runtime_dropdown_open = true;
            return Err(format!("CLI '{}' does not support safe session resume", newest.cli));
        }
        self.resume_session_entry(&newest, file_path)
    }

    /// Renames tab `idx` and updates its `title` in `sessions.json` if registered.
    pub fn rename_tab(&mut self, idx: usize, new_title: &str) {
        let clean = new_title.trim();
        if clean.is_empty() || idx >= self.tabs.len() {
            return;
        }
        self.tabs[idx].title = clean.to_string();
        let tab_id = self.tabs[idx].id as u64;
        let session_uuid = self.tabs[idx].session_uuid.clone();
        if let Some(entry) = self.session_registry.find_entry_mut(tab_id, session_uuid.as_deref()) {
            entry.title = clean.to_string();
            entry.last_active_at = crate::agent_sessions::unix_timestamp_secs();
            let _ = crate::agent_sessions::save_registry(&self.session_dir, &self.session_registry);
        }
    }

    /// Closes tab at `idx`, killing its child process if still running.
    /// Closing the last tab shows the new-session picker (`#651` §3 rule 9).
    pub fn close_tab(&mut self, idx: usize) {
        if idx < self.tabs.len() {
            let mut removed = self.tabs.remove(idx);
            let _ = removed.child.kill();
            Self::withdraw_tab_approvals(&self.session_dir, &mut removed);
            if let Some(entry) = self.session_registry.find_entry_mut(removed.id as u64, removed.session_uuid.as_deref()) {
                if entry.status == crate::agent_sessions::SessionEntryStatus::Active {
                    entry.status = crate::agent_sessions::SessionEntryStatus::Exited;
                    entry.last_active_at = crate::agent_sessions::unix_timestamp_secs();
                    let _ = crate::agent_sessions::save_registry(&self.session_dir, &self.session_registry);
                }
            }
            if self.active_tab >= self.tabs.len() && !self.tabs.is_empty() {
                self.active_tab = self.tabs.len() - 1;
            } else if self.tabs.is_empty() {
                self.active_tab = 0;
                self.picker_open = true;
                crate::agent_permissions::withdraw_pending_requests(&self.session_dir);
            }
            self.sync_active_tab_approvals();
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

        // Channel 2a: turn-marks.jsonl (written by CLI session hooks before/after agent turns).
        let expected_marks_path = self.live_patch.path.with_file_name("turn-marks.jsonl");
        if self.turn_marks.path() != expected_marks_path {
            self.turn_marks = crate::agent_turn_marks::TurnMarkLog::bind(expected_marks_path);
        }
        let marks = self.turn_marks.poll();
        if !marks.is_empty() {
            self.active_turn_ledger_mut().ingest_marks(marks, std::time::Instant::now());
        }

        // Channel 2b: doc-patch.jsonl (written by designos-mcp tools). Edits and document switches
        // share this one log, so their order is the log's order.
        let (lines, errors) = self.live_patch.poll();
        for err in errors {
            self.status_msg = Some(err.clone());
            actions.push(TerminalAction::LiveSyncError(err));
        }
        if !lines.is_empty() {
            actions.push(TerminalAction::ApplyLiveLog(lines));
        }

        // Channel 3: Append-only agent-events.jsonl written by Claude, Grok, and Antigravity hooks (`#683`).
        // Each routed status change also drives that tab's turn ledger (`#677`) for CLIs without turn marks.
        let has_eligible_tabs = self.tabs.iter().any(|t| matches!(t.spawn_preset, AgentCliPreset::Claude | AgentCliPreset::Grok | AgentCliPreset::Antigravity));
        let records = self.event_stream.poll(has_eligible_tabs);
        // Tabs whose status left `WaitingForInput` for `Working`/`Done` in this poll: an approval answered in the
        // CLI itself (not the in-panel card) withdraws that tab's stale approval requests below (`#656`).
        let mut tabs_left_waiting: Vec<usize> = Vec::new();
        for record in records {
            let tabs_info: Vec<crate::agent_turn_status::TabAttributionInfo> = self
                .tabs
                .iter_mut()
                .enumerate()
                .map(|(idx, t)| crate::agent_turn_status::TabAttributionInfo {
                    idx,
                    preset: t.spawn_preset,
                    is_alive: t.is_alive(),
                    session_uuid: t.session_uuid.clone(),
                })
                .collect();
            if let Some(tab_idx) = crate::agent_turn_status::route_record_to_tab(&record, &tabs_info) {
                if self.tabs[tab_idx].is_stale_event_after_stop(&record.e) {
                    continue;
                }
                // A new prompt ends a stop that never saw a closing hook (Claude Code skips Stop on an interrupt). The
                // stopped turn closes first, with its own start depth, so the new turn never folds with it.
                if matches!(record.e.as_str(), "UserPromptSubmit" | "PreInvocation") {
                    if let Some(action) = crate::agent_stop::close_pending_stop(&mut self.tabs[tab_idx], designos_doc::now_ms()) {
                        actions.push(action);
                    }
                }
                let tab_visible = self.surface_visible && tab_idx == self.active_tab;
                let effect = self.tabs[tab_idx].turn_tracker.on_event(&record);
                if let Some(new_status) = effect.new_status {
                    if let Ok(mut st) = self.tabs[tab_idx].agent_status.lock() {
                        if matches!(*st, AgentTabStatus::WaitingForInput(_)) && matches!(new_status, AgentTabStatus::Working(_) | AgentTabStatus::Done) {
                            tabs_left_waiting.push(tab_idx);
                        }
                        *st = new_status.clone();
                    }
                    let patch_off = self.live_patch.current_offset();
                    self.ledger_for_tab_mut(tab_idx).on_status_change(matches!(new_status, AgentTabStatus::Working(_)), patch_off, std::time::Instant::now());
                    let tab_id = self.tabs[tab_idx].id;
                    self.status_changes.push((tab_id, new_status.clone()));
                    if effect.mark_unread_if_hidden {
                        if !tab_visible {
                            self.tabs[tab_idx].unread = true;
                        }
                    } else if matches!(new_status, AgentTabStatus::Working(_)) {
                        self.tabs[tab_idx].unread = false;
                    }
                }
                // While the window is unfocused the active Ask-mode tab raises its own "needs approval" notification
                // (once per request, `#656`), so the turn-status banner for the same permission request would notify twice.
                let covered_by_approval_latch = matches!(effect.notify, Some(crate::agent_notify::NotifyKind::NeedsInput))
                    && tab_idx == self.active_tab
                    && !self.window_focused
                    && self.tabs[tab_idx].permission_mode == crate::agent_permissions::AgentPermissionMode::Ask;
                if let Some(notify_kind) = effect.notify.filter(|_| !covered_by_approval_latch) {
                    let tab_id = self.tabs[tab_idx].id;
                    let tab_title = self.tabs[tab_idx].title.clone();
                    let doc_name = if self.tabs[tab_idx].doc_name.trim().is_empty() { self.doc_name.clone() } else { self.tabs[tab_idx].doc_name.clone() };
                    let notif = crate::agent_notify::AgentNotification::new(tab_id, tab_title, doc_name, notify_kind);
                    if let Some(denied_msg) = self.notifier.notify_if_background(self.window_focused, tab_visible, notif) {
                        self.status_msg = Some(denied_msg);
                    }
                }
            }
        }

        // Channel 4: Per-tab approvals/ directories written by Claude PreToolUse approval hook (#656)
        let now_secs = crate::agent_sessions::unix_timestamp_secs();
        let root_approvals = crate::agent_permissions::approvals_dir(&self.session_dir);
        let root_polled = if root_approvals.is_dir() {
            crate::agent_permissions::poll_approvals_in_dir(&root_approvals, now_secs, crate::agent_permissions::APPROVAL_HOOK_TIMEOUT_SECS)
        } else {
            Vec::new()
        };

        if self.tabs.is_empty() {
            self.pending_approvals = root_polled;
        } else {
            let active_idx = self.active_tab.min(self.tabs.len().saturating_sub(1));
            let mut next_per_tab: Vec<Vec<crate::agent_permissions::ApprovalRequest>> = self
                .tabs
                .iter_mut()
                .map(|tab| {
                    tab.approvals_dir = crate::agent_permissions::tab_approvals_dir(&self.session_dir, tab.id);
                    if tab.approvals_dir.is_dir() {
                        crate::agent_permissions::poll_approvals_in_dir(&tab.approvals_dir, now_secs, crate::agent_permissions::APPROVAL_HOOK_TIMEOUT_SECS)
                    } else {
                        Vec::new()
                    }
                })
                .collect();

            for req in root_polled {
                let owner_idx = self.tabs.iter().position(|t| t.pending_approvals.iter().any(|r| r.id == req.id)).unwrap_or(active_idx);
                if !next_per_tab[owner_idx].iter().any(|r| r.id == req.id) {
                    next_per_tab[owner_idx].push(req);
                }
            }

            for (idx, tab) in self.tabs.iter_mut().enumerate() {
                let had_pending = !tab.pending_approvals.is_empty();
                let mut polled = std::mem::take(&mut next_per_tab[idx]);
                polled.sort_by(|a, b| (a.timestamp, &a.id).cmp(&(b.timestamp, &b.id)));
                if tabs_left_waiting.contains(&idx) && polled.is_empty() {
                    Self::withdraw_tab_approvals(&self.session_dir, tab);
                } else {
                    tab.pending_approvals = polled;
                }
                if !tab.pending_approvals.is_empty() {
                    if let Ok(mut st) = tab.agent_status.lock() {
                        *st = AgentTabStatus::WaitingForInput(Some("Needs approval".into()));
                    }
                } else if had_pending {
                    if let Ok(mut st) = tab.agent_status.lock() {
                        if matches!(*st, AgentTabStatus::WaitingForInput(_)) {
                            *st = AgentTabStatus::Idle;
                        }
                    }
                }
            }
            self.sync_active_tab_approvals();
        }

        // Track where each running turn started in the undo stack so Stop can fold exactly that turn (`#684`).
        let current_undo_depth = self.current_undo_depth;
        for tab in &mut self.tabs {
            let is_running = crate::agent_stop::is_tab_turn_running(tab);
            if is_running && !tab.was_working {
                tab.was_working = true;
                tab.turn_started_at_ms = Some(designos_doc::now_ms());
                tab.turn_start_undo_depth = current_undo_depth;
                tab.last_receipt = None;
                tab.still_running_notice = None;
            } else if !is_running && tab.stop_state.is_none() {
                tab.was_working = false;
                tab.turn_started_at_ms = None;
                tab.turn_start_undo_depth = current_undo_depth;
            }
        }
        actions.extend(crate::agent_stop::poll_stop_sequences(self, designos_doc::now_ms()));
        // Channel 5: `agent-status.json` -> `done` releases the frames this run flagged `in_progress` (`#592`).
        let status_file = self.session_dir.join("agent-status.json");
        if let Ok(raw_status) = std::fs::read_to_string(&status_file) {
            if let Some(parsed) = parse_agent_status_payload(raw_status.trim()) {
                let is_done = matches!(parsed, AgentTabStatus::Done);
                if is_done && !self.last_agent_status_done {
                    self.last_agent_status_done = true;
                    let json_val = serde_json::from_str::<serde_json::Value>(raw_status.trim()).ok();
                    let json_agent = json_val.as_ref().and_then(|v| v.get("agent")).and_then(|v| v.as_str()).map(|s| s.to_string());
                    let fallback_agent = self.active_tab().filter(|t| !t.is_shell_process()).map(|t| t.spawn_preset.binary_name().to_string()).or_else(|| {
                        (!matches!(self.active_runtime, AgentCliPreset::Shell | AgentCliPreset::DesignOsHelp))
                            .then(|| self.active_runtime.binary_name().to_string())
                    });
                    let agent = json_agent.or(fallback_agent);
                    if let Some(explicit_run_id) =
                        json_val.as_ref().and_then(|v| v.get("run_id").or_else(|| v.get("session_id"))).and_then(|v| v.as_str()).filter(|r| !r.is_empty())
                    {
                        actions.push(TerminalAction::ClearInProgress { run_id: Some(explicit_run_id.to_string()), agent });
                    } else {
                        let tab_run_id =
                            self.active_tab().filter(|t| !t.is_shell_process()).map(|t| t.run_id.clone()).unwrap_or_else(|| self.active_run_id.clone());
                        actions.push(TerminalAction::ClearInProgress { run_id: Some(tab_run_id.clone()), agent: agent.clone() });
                        if self.tabs.iter().all(|t| t.is_shell_process()) && self.session_id != tab_run_id {
                            actions.push(TerminalAction::ClearInProgress { run_id: Some(self.session_id.clone()), agent });
                        }
                    }
                } else if !is_done {
                    self.last_agent_status_done = false;
                }
            }
        }

        let mut agent_exited = false;
        let mut stopped_exited = false;
        #[cfg(test)]
        let path_override = self.path_override.clone();
        let mut registry_dirty = false;
        let patch_off = self.live_patch.current_offset();
        let now_instant = std::time::Instant::now();
        let active_idx = self.active_tab;
        let surface_vis = self.surface_visible;
        for (idx, tab) in self.tabs.iter_mut().enumerate() {
            if tab.is_shell_process() || tab.exited_notice_shown {
                continue;
            }
            if !tab.is_alive() {
                tab.turn_ledger.on_tab_exited(patch_off, now_instant);
                let was_stopped = matches!(tab.agent_status.lock().ok().as_deref(), Some(AgentTabStatus::Stopped));
                let exit_code = tab.child.try_wait().ok().flatten().map(|status| status.exit_code());
                let failed_start = !was_stopped && agent_start_failure::counts_as_failed_start(tab.spawned_at.elapsed(), exit_code);
                if failed_start {
                    let log_empty = tab.raw_log.lock().map(|log| log.is_empty()).unwrap_or(false);
                    let eof = tab.reader_eof.load(std::sync::atomic::Ordering::Acquire);
                    if log_empty && !eof {
                        let first_seen = *tab.exited_at.get_or_insert_with(std::time::Instant::now);
                        if first_seen.elapsed() < std::time::Duration::from_millis(100) {
                            continue;
                        }
                    }
                    let output = tab.raw_log.lock().map(|log| log.clone()).unwrap_or_default();
                    let installed = {
                        #[cfg(test)]
                        {
                            if let Some(ref p) = path_override {
                                runtime_binary_available_in(tab.spawn_preset.binary_name(), Some(p.as_os_str()), false)
                            } else {
                                tab.spawn_preset.is_installed()
                            }
                        }
                        #[cfg(not(test))]
                        {
                            tab.spawn_preset.is_installed()
                        }
                    } || self.path_prepend.iter().any(|d| d.join(tab.spawn_preset.binary_name()).is_file());
                    let mut diag = agent_start_failure::diagnose(tab.spawn_preset, installed, exit_code, &output);
                    if tab.is_resumed && !matches!(diag.fix, FixAction::OpenInstallPage(_) | FixAction::RunLogin(_)) {
                        diag.fix = FixAction::StartNewSession(tab.spawn_preset);
                    }
                    tab.start_failure = Some(diag);
                }
                let prev_status = tab.effective_status();
                if let Some((next_status, _receipt)) = tab.turn_tracker.on_process_exit(now_secs as f64, &prev_status) {
                    if let Ok(mut st) = tab.agent_status.lock() {
                        *st = next_status.clone();
                    }
                    self.status_changes.push((tab.id, next_status));
                    let tab_visible = surface_vis && idx == active_idx;
                    if !tab_visible {
                        tab.unread = true;
                    }
                    let tab_title = tab.title.clone();
                    let doc_name = if tab.doc_name.trim().is_empty() { self.doc_name.clone() } else { tab.doc_name.clone() };
                    let notif = crate::agent_notify::AgentNotification::new(tab.id, tab_title, doc_name, crate::agent_notify::NotifyKind::Finished);
                    if let Some(denied_msg) = self.notifier.notify_if_background(self.window_focused, tab_visible, notif) {
                        self.status_msg = Some(denied_msg);
                    }
                }
                if let Some(entry) = self.session_registry.find_entry_mut(tab.id as u64, tab.session_uuid.as_deref()) {
                    entry.last_active_at = now_secs;
                    entry.status =
                        if failed_start && tab.is_resumed && matches!(tab.start_failure.as_ref().map(|d| &d.fix), Some(FixAction::StartNewSession(_))) {
                            crate::agent_sessions::SessionEntryStatus::Missing
                        } else {
                            crate::agent_sessions::SessionEntryStatus::Exited
                        };
                    registry_dirty = true;
                }
                if let Ok(mut parser) = tab.parser.lock() {
                    parser.process("\r\n[design:os] Agent exited — this tab no longer accepts input. Open a new tab to continue.\r\n".as_bytes());
                }
                Self::withdraw_tab_approvals(&self.session_dir, tab);
                if let Ok(mut st) = tab.agent_status.lock() {
                    if matches!(*st, AgentTabStatus::WaitingForInput(_)) {
                        *st = AgentTabStatus::Idle;
                    }
                }
                tab.title.push_str(" (exited)");
                tab.exited_notice_shown = true;
                agent_exited = true;
                stopped_exited |= was_stopped;
                actions.push(TerminalAction::ClearInProgress { run_id: Some(tab.run_id.clone()), agent: Some(tab.spawn_preset.binary_name().to_string()) });
            }
        }
        if registry_dirty {
            let _ = crate::agent_sessions::save_registry(&self.session_dir, &self.session_registry);
        }
        if agent_exited {
            self.sync_active_tab_approvals();
        }
        if let Some(denied_msg) = self.notifier.poll_denied_status_msg() {
            self.status_msg = Some(denied_msg);
        }
        if agent_exited && !stopped_exited && self.status_msg.as_deref() != Some(crate::agent_notify::NOTIFICATIONS_OFF_STATUS_MSG) {
            self.status_msg = Some("Agent exited — this tab no longer accepts input. Open a new tab to continue.".into());
        }

        self.tick_composer();
        while let Some(clicked_tab_id) = self.notifier.poll_activated_tab() {
            if self.focus_tab_by_id(clicked_tab_id) {
                actions.push(TerminalAction::FocusAgentTab(clicked_tab_id));
            }
        }

        let cli_label = self.active_tab().map(|t| t.preset.button_label()).unwrap_or_else(|| self.active_runtime.button_label());
        let request_key: Option<String> = if let Some(first) = self.pending_approvals.first() {
            Some(first.id.clone())
        } else if self.needs_approval() {
            Some("waiting".to_string())
        } else {
            None
        };
        self.notification_latch.update(self.approval_notifier.as_ref(), Some(self.window_focused), cli_label, &self.doc_name, request_key.as_deref());

        actions
    }

    /// Drains any tab status transitions since the last drain (`#687`).
    pub fn drain_status_changes(&mut self) -> Vec<crate::agent_a11y::StatusChange> {
        let mut out = Vec::new();
        for tab in &mut self.tabs {
            let current = tab.agent_status.lock().ok().map(|g| g.clone()).unwrap_or_default();
            if current != tab.reported_status {
                let prev = std::mem::replace(&mut tab.reported_status, current.clone());
                out.push(crate::agent_a11y::StatusChange { tab_id: tab.id, preset: tab.preset, title: tab.title.clone(), prev, next: current });
            }
        }
        out
    }

    /// Selects and reveals the agent tab with `tab_id` when a notification is clicked (`#683` §3.6).
    pub fn focus_tab_by_id(&mut self, tab_id: usize) -> bool {
        let Some(idx) = self.tabs.iter().position(|t| t.id == tab_id) else {
            return false;
        };
        self.active_tab = idx;
        self.tabs[idx].unread = false;
        self.surface_visible = true;
        self.window_focused = true;
        true
    }

    /// Records distinct `NodeId`s touched by `doc-patch.jsonl` entries applied during the active turn (`#683` §3.4).
    ///
    /// Attribution follows rule 2 & rule 5: only the single tab that is unambiguously in an active
    /// turn (when it is the only open tab, or when its current tool is a layer-mutation tool)
    /// receives the count. When multiple tabs are open and no single active-turn tab is running a
    /// disambiguating layer-mutation tool, Studio omits the count rather than misattributing a
    /// non-hooked tab's edit (e.g. Codex) to a hooked tab running `Read`.
    pub fn record_touched_layers(&mut self, node_ids: &[u64]) {
        if node_ids.is_empty() {
            return;
        }
        let in_turn_indices: Vec<usize> = self.tabs.iter().enumerate().filter_map(|(i, t)| t.turn_tracker.in_turn.then_some(i)).collect();
        let target_idx = match in_turn_indices.as_slice() {
            [only] if self.tabs.len() == 1 || crate::agent_turn_status::is_layer_mutation_tool(self.tabs[*only].turn_tracker.current_tool_name.as_deref()) => {
                Some(*only)
            }
            many if many.len() > 1 => {
                let mut layer_tool_tabs =
                    many.iter().copied().filter(|&i| crate::agent_turn_status::is_layer_mutation_tool(self.tabs[i].turn_tracker.current_tool_name.as_deref()));
                match (layer_tool_tabs.next(), layer_tool_tabs.next()) {
                    (Some(only), None) => Some(only),
                    _ => None,
                }
            }
            _ => None,
        };
        if let Some(idx) = target_idx {
            self.tabs[idx].turn_tracker.record_touched_layers(node_ids.iter().copied());
        }
    }

    /// Dispatches a Figma AI task prompt into the active PTY session through [`Self::send_to_agent`]
    /// (`#651` §3 rule 10 & `#689` §3 rule 8).
    pub fn dispatch_figma_ai_prompt(&mut self, task_title: &str, cli_prompt: &str, file_path: Option<&Path>) {
        self.dock_open = true;
        let outcome = self.send_to_agent(cli_prompt, file_path);
        if self.tabs.is_empty() && self.picker_open {
            self.pending_prompt = Some((task_title.to_string(), cli_prompt.to_string()));
        } else if matches!(outcome, crate::agent_composer::SendOutcome::Written | crate::agent_composer::SendOutcome::Queued) {
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

    /// Pastes a terminal-header `@`-mention pick into the active agent tab (`#655`).
    /// Closes the popup and reports the pasted label when accepted; keeps the popup open and sets
    /// `"No agent tab open - nothing sent"` when no agent tab accepted the paste.
    pub fn handle_terminal_mention_pick(&mut self, mention: &crate::agent_mention::Mention) -> bool {
        let snippet = crate::agent_mention::format_terminal_paste_mention(mention);
        if self.paste_into_agent_tab(&snippet) {
            self.mention_popup.close();
            self.status_msg = Some(format!("Pasted @{} into agent tab (press Enter to send)", mention.label));
            true
        } else {
            self.mention_popup.open = true;
            self.status_msg = Some("No agent tab open - nothing sent".to_string());
            false
        }
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

pub fn parse_agent_status_payload(payload: &str) -> Option<AgentTabStatus> {
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
            "done" | "completed" | "idle" => Some(AgentTabStatus::Done),
            "stopped" => Some(AgentTabStatus::Stopped),
            "error" | "failed" => Some(AgentTabStatus::Error(detail)),
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
        "stopped" => Some(AgentTabStatus::Stopped),
        "error" | "failed" => Some(AgentTabStatus::Error(detail)),
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

/// Renders Orca's Dual-Glyph Live Agent Status (`Working` 12fps `theme::INFO` spinner, `WaitingForInput`
/// `theme::WARNING` ring, `Done` unread `theme::SUCCESS` dot, `Idle`/`Stopped`/read `theme::TEXT_SECONDARY`,
/// `Error` `theme::DANGER`) + Provider Color Badge (`Claude` `#D97757`, `Codex` `#FAFAFA`, `Grok` `#E4E4E7`,
/// `Antigravity` `#60A5FA`, `Shell` `>_`) inside a tab rect (`#683` §5).
fn paint_orca_tab_status_and_provider(ui: &Ui, p: &Painter, tab: &TerminalTab, t_rect: Rect) {
    let cy = t_rect.center().y;
    let status_center = Pos2::new(t_rect.left() + 7.5, cy);
    let provider_center = Pos2::new(t_rect.left() + 15.0, cy);

    let effective_status = tab.effective_status();

    match effective_status {
        AgentTabStatus::Working(_) => {
            // Orca 12fps stepped rotating `theme::INFO` arc spinner + 83ms repaint cadence
            let step = ((ui.input(|i| i.time) * 12.0).floor() as i32).rem_euclid(12);
            let base_angle = (step as f32) * (std::f32::consts::TAU / 12.0);
            let arc_span = std::f32::consts::TAU * 0.68;
            let radius = 3.2f32;
            let spin_col = theme::INFO;
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
            let wait_col = theme::WARNING;
            p.circle_stroke(status_center, 3.1, Stroke::new(1.3, wait_col));
            p.circle_filled(status_center, 1.2, wait_col);
        }
        AgentTabStatus::Done if tab.unread => {
            p.circle_filled(status_center, 2.7, theme::SUCCESS);
        }
        AgentTabStatus::Error(_) => {
            p.circle_filled(status_center, 2.7, theme::DANGER);
        }
        AgentTabStatus::Done | AgentTabStatus::Idle => {
            p.circle_filled(status_center, 2.7, theme::TEXT_SECONDARY);
        }
        AgentTabStatus::Stopped => {
            p.circle_filled(status_center, 2.7, theme::ICON_TERTIARY);
        }
    }

    // Provider Color Badge / Vector Glyph (`Claude` #D97757, `Codex`/`OpenCode` #FAFAFA, `Grok` #E4E4E7, `Antigravity`/`Gemini` #60A5FA, `Shell` `>_`)
    match tab.preset {
        AgentCliPreset::Claude => {
            let col = Color32::from_rgb(0xD9, 0x77, 0x57);
            p.circle_filled(provider_center, 2.3, col);
        }
        AgentCliPreset::Codex | AgentCliPreset::OpenCode => {
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
        AgentCliPreset::Antigravity | AgentCliPreset::Gemini => {
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
        if cfg!(test) {
            if shell_bin.ends_with("zsh") {
                c.arg("-d");
                c.arg("-f");
            } else if shell_bin.ends_with("bash") || shell_bin.ends_with("bash.exe") {
                c.arg("--norc");
                c.arg("--noprofile");
            }
        } else {
            c.arg("-l");
            c.arg("-i");
        }
        c.arg("-c");
        // Single command: `exec` so the agent is the PTY child; when it exits the child exits.
        // Compound commands keep their own fallback, with no `|| exec $SHELL`.
        let trimmed = agent_cli.trim_start();
        let inner = if agent_cli.contains("||") || trimmed.starts_with("exec ") { agent_cli.to_string() } else { format!("exec {agent_cli}") };
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

/// Publishes the session's MCP server config as Gemini CLI's project settings at
/// `<session_dir>/.gemini/settings.json` (`"trust": true`, identical to the output of
/// `gemini mcp add --scope project --trust`) plus `<session_dir>/.gemini/trustedFolders.json`
/// (`GEMINI_CLI_TRUSTED_FOLDERS_PATH`), leaving `~/.gemini/` untouched (`#679` §3 rule 4).
pub fn write_gemini_session_config(session_dir: &Path, mcp_config: &serde_json::Value) {
    let _argv = build_gemini_mcp_add_argv(mcp_config);
    let mut gemini_cfg = mcp_config.clone();
    // `designos.enabled_tools` is read by `designos-mcp`; Gemini's settings file only takes the server entry.
    if let Some(obj) = gemini_cfg.as_object_mut() {
        obj.remove("designos");
    }
    if let Some(designos) = gemini_cfg.get_mut("mcpServers").and_then(|s| s.get_mut("designos")).and_then(|d| d.as_object_mut()) {
        designos.insert("trust".to_string(), serde_json::Value::Bool(true));
    }
    if let Ok(bytes) = serde_json::to_vec_pretty(&gemini_cfg) {
        let settings_path = session_dir.join(".gemini").join("settings.json");
        let _ = atomic_write_session_file(&settings_path, &bytes);
    }

    let mut trusted = serde_json::Map::new();
    trusted.insert(session_dir.display().to_string(), serde_json::Value::String("TRUST_FOLDER".to_string()));
    if let Ok(canon) = session_dir.canonicalize() {
        trusted.insert(canon.display().to_string(), serde_json::Value::String("TRUST_FOLDER".to_string()));
    }
    if let Ok(bytes) = serde_json::to_vec_pretty(&serde_json::Value::Object(trusted)) {
        let trusted_path = session_dir.join(".gemini").join("trustedFolders.json");
        let _ = atomic_write_session_file(&trusted_path, &bytes);
    }
}

/// Builds the project-scoped `gemini mcp add` argv (`#679` §3 rule 4) using only flags
/// documented in `gemini mcp add --help` (`--scope project`, `--trust`, `-e KEY=value`),
/// separating trailing server flags (`--workspace <dir>`) with `--`.
pub fn build_gemini_mcp_add_argv(mcp_config: &serde_json::Value) -> Vec<String> {
    let Some(server) = mcp_config.get("mcpServers").and_then(|s| s.get("designos")) else {
        return Vec::new();
    };
    let command = server.get("command").and_then(|v| v.as_str()).unwrap_or("designos-mcp");
    let mut argv = vec!["gemini".to_string(), "mcp".to_string(), "add".to_string(), "--scope".to_string(), "project".to_string(), "--trust".to_string()];
    if let Some(env_obj) = server.get("env").and_then(|v| v.as_object()) {
        for (k, v) in env_obj {
            if let Some(val) = v.as_str() {
                argv.push("-e".to_string());
                argv.push(format!("{k}={val}"));
            }
        }
    }
    argv.push("designos".to_string());
    argv.push(command.to_string());
    if let Some(args) = server.get("args").and_then(|v| v.as_array()) {
        let trailing: Vec<&str> = args.iter().filter_map(|a| a.as_str()).collect();
        if !trailing.is_empty() {
            if trailing.iter().any(|a| a.starts_with('-')) {
                argv.push("--".to_string());
            }
            for arg in trailing {
                argv.push(arg.to_string());
            }
        }
    }
    argv
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

#[cfg(test)]
thread_local! {
    /// Runs once between the temporary canvas file being complete and the rename, so a test can
    /// place an agent append in that window.
    static BEFORE_RENAME_TEST_HOOK: std::cell::RefCell<Option<Box<dyn FnOnce()>>> = const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
pub(crate) fn set_before_rename_test_hook(hook: Box<dyn FnOnce()>) {
    BEFORE_RENAME_TEST_HOOK.with(|h| *h.borrow_mut() = Some(hook));
}

#[cfg(test)]
fn run_before_rename_test_hook() {
    if let Some(hook) = BEFORE_RENAME_TEST_HOOK.with(|h| h.borrow_mut().take()) {
        hook();
    }
}

#[cfg(not(test))]
fn run_before_rename_test_hook() {}

fn atomic_write_session_file(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    write_session_file_unless(path, bytes, || false).map(|_| ())
}

/// Like [`atomic_write_session_file`], but `hold` is asked after the temporary file is complete
/// and just before it replaces `path`. When it says yes the temporary file is removed, `path`
/// is left as it was and `Ok(true)` comes back.
fn write_session_file_unless(path: &Path, bytes: &[u8], hold: impl FnOnce() -> bool) -> std::io::Result<bool> {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let tmp = path.with_extension(format!("tmp.{}", std::process::id()));
    std::fs::write(&tmp, bytes)?;
    if hold() {
        let _ = std::fs::remove_file(&tmp);
        return Ok(true);
    }
    std::fs::rename(&tmp, path).map(|()| false)
}

/// Shell command that appends one wrapped hook record to `<session_dir>/agent-events.jsonl` (`#683` §3.1)
/// and updates `agent-status.json` for legacy status readers.
fn status_hook_command(status_path: &Path, event: &str, state: &str) -> String {
    let events_path = status_path.parent().map(|p| p.join("agent-events.jsonl")).unwrap_or_else(|| status_path.with_file_name("agent-events.jsonl"));
    let append_cmd = crate::agent_turn_status::event_append_command(&events_path, event);
    format!("{append_cmd}; printf '{{\"state\":\"{state}\",\"run_id\":\"%s\"}}\\n' \"${{DESIGNOS_AGENT_RUN_ID:-}}\" > '{}'", status_path.display())
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
    session_hooks_settings_with_approval(status_path, context_reply, None)
}

/// [`session_hooks_settings`] plus, in `Ask` permission mode (`approval_script` is `Some`), `approve-hook.sh` as a second
/// handler of the `PermissionRequest` group so one request both records the turn status and raises the in-panel approval (`#656`).
fn session_hooks_settings_with_approval(status_path: &Path, context_reply: Option<&Path>, approval_script: Option<&Path>) -> serde_json::Value {
    let session_dir = status_path.parent().unwrap_or_else(|| Path::new("/tmp"));
    let mark_path = session_dir.join("turn-marks.jsonl");
    let patch_path = session_dir.join("doc-patch.jsonl");
    let cli = if context_reply.is_some() { "claude" } else { "grok" };
    let start_mark = crate::agent_turn_marks::turn_mark_hook_command(&mark_path, &patch_path, "start", cli);
    let end_mark = crate::agent_turn_marks::turn_mark_hook_command(&mark_path, &patch_path, "end", cli);
    let prompt = match context_reply {
        Some(reply) => format!("{}; {}; cat '{}'", status_hook_command(status_path, "UserPromptSubmit", "working"), start_mark, reply.display()),
        None => format!("{}; {}", status_hook_command(status_path, "UserPromptSubmit", "working"), start_mark),
    };
    let stop_cmd = |event: &str, state: &str| format!("{}; {}", status_hook_command(status_path, event, state), end_mark);
    let mut permission_request = command_handler_group(status_hook_command(status_path, "PermissionRequest", "waiting"));
    if let (Some(script), Some(handlers)) = (approval_script, permission_request[0]["hooks"].as_array_mut()) {
        handlers.push(crate::agent_permissions::approval_hook_handler(script));
    }
    serde_json::json!({
        "hooks": {
            "UserPromptSubmit": command_handler_group(prompt),
            "PreToolUse": command_handler_group(status_hook_command(status_path, "PreToolUse", "working")),
            "PostToolUse": command_handler_group(status_hook_command(status_path, "PostToolUse", "working")),
            "PostToolUseFailure": command_handler_group(status_hook_command(status_path, "PostToolUseFailure", "working")),
            "PermissionRequest": permission_request,
            "Notification": command_handler_group(status_hook_command(status_path, "Notification", "waiting")),
            "Stop": command_handler_group(stop_cmd("Stop", "done")),
            "StopFailure": command_handler_group(stop_cmd("StopFailure", "error")),
            "StopCancelled": command_handler_group(stop_cmd("StopCancelled", "stopped")),
            "SessionStart": command_handler_group(status_hook_command(status_path, "SessionStart", "idle")),
        }
    })
}

/// Antigravity's `<workspace>/.agents/hooks.json`: named hook, flat handlers for the invocation, tool,
/// and stop events (`#683` §3.1). Handlers must answer with a JSON object on stdout.
fn agy_session_hooks(status_path: &Path) -> serde_json::Value {
    let session_dir = status_path.parent().unwrap_or_else(|| Path::new("/tmp"));
    let mark_path = session_dir.join("turn-marks.jsonl");
    let patch_path = session_dir.join("doc-patch.jsonl");
    let start_mark = crate::agent_turn_marks::turn_mark_hook_command(&mark_path, &patch_path, "start", "agy");
    let end_mark = crate::agent_turn_marks::turn_mark_hook_command(&mark_path, &patch_path, "end", "agy");
    let handler = |event: &str, state: &str, mark: Option<&str>| {
        let mark = mark.map(|m| format!("{m}; ")).unwrap_or_default();
        serde_json::json!([{ "type": "command", "command": format!("{}; {mark}printf '{{}}'", status_hook_command(status_path, event, state)) }])
    };
    serde_json::json!({
        "designos-status": {
            "PreInvocation": handler("PreInvocation", "working", Some(&start_mark)),
            "PreToolUse": handler("PreToolUse", "working", None),
            "PostToolUse": handler("PostToolUse", "working", None),
            "PostToolUseFailure": handler("PostToolUseFailure", "working", None),
            "Stop": handler("Stop", "done", Some(&end_mark)),
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
    cmd.env("DESIGNOS_IN_PROGRESS_STALE_MINUTES", designos_doc::in_progress_stale_minutes().to_string());
    if preset == AgentCliPreset::Grok {
        // Grok starts repo-local MCP servers, hooks and skills only in a trusted folder. The tab's
        // cwd is Studio's own session dir, so skip the prompt for this process instead of writing
        // `~/.grok/trusted_folders.toml`.
        cmd.env("GROK_FOLDER_TRUST", "0");
    }

    if let Some(sess) = session {
        cmd.env("DESIGNOS_SESSION_DIR", sess.session_dir.as_os_str());
        cmd.env("DESIGNOS_AGENT_RUN_ID", &sess.run_id);
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
        if preset == AgentCliPreset::Gemini {
            cmd.env("GEMINI_CLI_TRUSTED_FOLDERS_PATH", sess.session_dir.join(".gemini").join("trustedFolders.json").as_os_str());
        }
    } else {
        cmd.env("DESIGNOS_AGENT_RUN_ID", format!("p{}-tab-1-1-1", std::process::id()));
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
    if cmd.get_cwd().is_none() {
        cmd.cwd(resolve_pty_cwd(preset, session.map(|sess| sess.session_dir.as_path()), session_workspace_dir(file_path)));
    }

    #[cfg(test)]
    if let Some(overrides) = session.and_then(|s| s.path_override.as_ref()) {
        cmd.env("PATH", overrides);
        return;
    }
    if std::env::var_os("PATH").is_some_and(|p| p.is_empty()) {
        cmd.env("PATH", "");
        return;
    }

    let mut extra_paths: Vec<String> = Vec::new();
    if let Some(sess) = session {
        for p in &sess.path_prepend {
            extra_paths.push(p.to_string_lossy().into_owned());
        }
    }
    extra_paths.extend(extra_cli_path_dirs().into_iter().map(|p| p.to_string_lossy().into_owned()));

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

/// Sends the current selection from the live selection chip (`#654`):
/// writes `selection.spec.json` (`{ page, count, nodes }`, inline `spec` for the first 10 layers
/// under 256 KB) and pastes one fixed sentence into the active agent tab without pressing Enter.
/// When no live agent tab is open, writes nothing to the PTY and sets `status_msg` to
/// `"No agent tab open - nothing sent"`.
pub fn send_selection_to_agent_pty(state: &mut TerminalState, doc: &Document, page: NodeId, selection: &[NodeId]) -> Result<Option<PathBuf>, String> {
    let Some((json, count)) = crate::agent_selection_chip::build_selection_spec_json(doc, page, selection)? else {
        return Ok(None);
    };
    let has_live_agent = state.active_tab_mut().is_some_and(|t| t.accepts_agent_input());
    if !has_live_agent {
        state.status_msg = Some("No agent tab open - nothing sent".to_string());
        let _ = atomic_write_session_file(&state.selection_spec_path, json.as_bytes());
        return Ok(Some(state.selection_spec_path.clone()));
    }
    let _ = atomic_write_session_file(&state.selection_spec_path, json.as_bytes());
    let sentence =
        format!("Read the design:os selection at {} (use designos_export_spec for layers without an inline spec).", state.selection_spec_path.display());
    if state.paste_into_agent_tab(&sentence) {
        state.status_msg = Some(format!("Sent {count} selected to {}", state.selection_spec_path.display()));
    } else {
        state.status_msg = Some("No agent tab open - nothing sent".to_string());
    }
    Ok(Some(state.selection_spec_path.clone()))
}

/// Renders the 1:1 Figma Desktop `126.9.13` `Agents` Left Side Panel (`New chat / Shared` header,
/// dismissible AI credits banner, CLI Runtime Selector Bar (`Claude` | `Codex` | `Antigravity` | `Shell`),
/// pure Chat UI wrapper over the live PTY shell (`DIR-21`), `What's next?` empty state, bottom
/// `"Ask for changes"` blue-bordered composer box, and toggleable in-panel live VT100 PTY viewport).
pub fn show_left(state: &mut TerminalState, ui: &mut Ui, rect: Rect, file_path: Option<&Path>) -> Vec<TerminalAction> {
    let summary = crate::agent_selection_chip::SelectionSummary::empty(&state.current_page_name);
    show_left_with_selection(state, ui, rect, file_path, &summary)
}

/// Renders the `Agents` Left Side Panel with the live [`crate::agent_selection_chip::SelectionSummary`]
/// computed in the current frame from `app`.
pub fn show_left_with_selection(
    state: &mut TerminalState,
    ui: &mut Ui,
    rect: Rect,
    file_path: Option<&Path>,
    selection_summary: &crate::agent_selection_chip::SelectionSummary,
) -> Vec<TerminalAction> {
    let empty_doc = Document::new(&state.doc_name);
    let page = NodeId(state.current_page_id.max(1));
    show_left_with_doc(state, ui, rect, file_path, selection_summary, &empty_doc, page)
}

/// Renders the `Agents` Left Side Panel with live [`Document`] access for the `@`-mention picker (`#655`).
pub fn show_left_with_doc(
    state: &mut TerminalState,
    ui: &mut Ui,
    rect: Rect,
    file_path: Option<&Path>,
    selection_summary: &crate::agent_selection_chip::SelectionSummary,
    doc: &Document,
    current_page: NodeId,
) -> Vec<TerminalAction> {
    let mut actions = state.poll_live_agent_bridge();
    crate::agent_a11y::label_agents_panel(state, ui);
    let p = ui.painter().clone();
    let composer_input_id = Id::new("agents-ask-for-changes-input");
    if ui.memory(|m| m.has_focus(composer_input_id)) {
        state.viewport_focused = false;
    }

    if state.side_panel_pty_open {
        // Pure Interactive PTY Shell by Default + Top `+` Runtime Dropdown (`DIR-29`)
        p.rect_filled(rect, 0.0, TERM_BG_VIEWPORT);

        let bar_h = 36.0;
        if ui.input(|i| i.pointer.any_pressed()) {
            if let Some(pos) = ui.input(|i| i.pointer.interact_pos()) {
                if rect.contains(pos) {
                    // A press in the header or the strip under it is panel chrome: `Esc` stops the turn there, and is forwarded to the CLI in the terminal (`#684 §3.1`).
                    let in_chrome = pos.y < rect.top() + bar_h + theme::CONTROL_DENSE;
                    state.chrome_focused = in_chrome;
                    state.viewport_focused = !in_chrome;
                } else {
                    state.chrome_focused = false;
                }
            }
        }

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

        let stop_left = crate::agent_stop::paint_header_stop_controls(ui, &p, state, bar_r, pop_rect.left());

        // Left-aligned session tabs + `"Sessions"` ghost button + `+` runtime dropdown button
        let max_tabs_right = crate::agent_open_flow::max_tabs_right_for_bar(stop_left);
        let mut x = bar_r.left() + 6.0;
        let mut tab_to_select: Option<usize> = None;
        let mut tab_to_close: Option<usize> = None;
        let can_close = !state.tabs.is_empty();

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
            let tab_waiting = tab.needs_approval() || (is_active && !state.pending_approvals.is_empty());
            let tab_resp = ui.interact(t_hit_rect, Id::new(("agents-left-tab-select", tab.id)), Sense::click());
            let tab_resp = if tab_waiting { tab_resp.on_hover_text("Needs approval") } else { tab_resp };
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
            if let Some(tab) = state.tabs.get_mut(idx) {
                tab.unread = false;
            }
            state.sync_active_tab_approvals();
            state.viewport_focused = true;
            state.runtime_dropdown_open = false;
            state.sessions_popover_open = false;
            state.picker.on_close();
            state.sessions_popover_open = false;
        }
        if state.surface_visible {
            if let Some(tab) = state.active_tab_mut() {
                tab.unread = false;
            }
        }
        if let Some(idx) = tab_to_close {
            state.close_tab(idx);
        }

        // `"Sessions"` ghost button (`#651` §3 rule 5 & §7) + top `+` button (`agents-runtime-add-btn`)
        let add_x = crate::agent_open_flow::render_sessions_button_in_bar(state, ui, bar_r, x, stop_left);
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
            if state.runtime_dropdown_open {
                state.sessions_popover_open = false;
            } else {
                state.picker.on_close();
            }
        }

        // Header strip (`CONTROL_DENSE` = 24px): the live selection chip (#654) stays on the left; the live turn status (`#683` §7,
        // running dot + text) is right-aligned at a capped width. The chip shrinks to the space left of the status.
        let ctx_h = theme::CONTROL_DENSE;
        let ctx_r = Rect::from_min_size(Pos2::new(rect.left(), bar_r.bottom()), Vec2::new(rect.width(), ctx_h));
        p.rect_filled(ctx_r, 0.0, theme::BG_PRIMARY);
        widgets::hline(&p, ctx_r.left(), ctx_r.right(), ctx_r.bottom() - theme::BORDER_WIDTH, theme::BORDER);
        let now_secs = crate::agent_sessions::unix_timestamp_secs();
        let strip_left = ctx_r.left() + theme::INSET_CONTROL;
        let (mention_btn_r, mention_clicked) = crate::agent_mention_popup::paint_terminal_mention_button(
            ui,
            "agent-mention-btn-left",
            ctx_r.right() - theme::INSET_CONTROL,
            ctx_r,
            state.mention_popup.is_open_for_surface("agents-left-pty"),
        );
        if mention_clicked {
            state.mention_popup.toggle_terminal_mode_for("agents-left-pty", doc, current_page);
        }
        let strip_right = (mention_btn_r.left() - theme::SPACE_INLINE).max(strip_left + theme::TARGET_MIN);
        let mut chip_right = strip_right;
        // One right-hand slot: a pending approval (`#656`) takes it over from the live turn status (`#683`).
        let needs_approval = state.needs_approval();
        let strip_status = if needs_approval {
            Some((STATUS_WARN_TEXT, "Needs approval".to_string()))
        } else if let Some(notice) = state.active_tab().and_then(|t| t.still_running_notice.clone()) {
            // The CLI ignored both interrupt keys (`#684 §8.1`).
            Some((STATUS_WARN_TEXT, notice))
        } else {
            state.active_tab().and_then(|t| t.header_status_indicator(now_secs))
        };
        if let Some((dot_col, status_txt)) = strip_status {
            const DOT_DIAMETER: f32 = 6.0;
            let text_x_off = DOT_DIAMETER + theme::INSET_CONTROL;
            let cap_w = (((ctx_r.right() - theme::INSET_CONTROL) - strip_left) * 0.4).max(text_x_off + theme::TARGET_MIN).min(strip_right - strip_left);
            let font = theme::font_caption();
            let text_w = widgets::text_width(&p, &status_txt, &font);
            let status_w = (text_x_off + text_w).min(cap_w);
            let status_r = Rect::from_min_max(Pos2::new(strip_right - status_w, ctx_r.top()), Pos2::new(strip_right, ctx_r.bottom()));
            let status_elided = widgets::elide(&p, &status_txt, &font, (status_w - text_x_off).max(0.0));
            p.circle_filled(Pos2::new(status_r.left() + DOT_DIAMETER * 0.5, status_r.center().y), DOT_DIAMETER * 0.5, dot_col);
            let text_col = if needs_approval { STATUS_WARN_TEXT } else { theme::TEXT_SECONDARY };
            p.text(Pos2::new(status_r.left() + text_x_off, status_r.center().y), Align2::LEFT_CENTER, status_elided, font, text_col);
            widgets::record(ui, if needs_approval { "agents-needs-approval-pill" } else { "agents-header-turn-status" }, status_r);
            chip_right = (status_r.left() - theme::INSET_CONTROL).max(strip_left + theme::TARGET_MIN);
        }
        let chip_bounds = Rect::from_min_max(Pos2::new(strip_left, ctx_r.top()), Pos2::new(chip_right, ctx_r.bottom()));
        if let Some((_chip_rect, true)) =
            crate::agent_selection_chip::paint_selection_chip(ui, "agent-selection-chip-left", selection_summary, chip_bounds, false)
        {
            actions.push(TerminalAction::SendSelectionToAgent);
        }

        // Terminal-header `@`-mention picker keys are consumed before the composer and the VT100 viewport read them (`#655`).
        if state.mention_popup.is_open_for_surface("agents-left-pty") {
            state.mention_popup.sync_from_composer_text("", 0, doc, current_page);
            let mut unused_text = String::new();
            let kb_outcome = state.mention_popup.handle_keyboard(ui.ctx(), &mut unused_text, &mut state.active_mentions, doc, current_page);
            if let crate::agent_mention_popup::MentionPopupOutcome::PickedTerminal(mention) = kb_outcome {
                state.handle_terminal_mention_pick(&mention);
            }
        }

        // Render bottom composer (`#689` §3 rule 9 & §7) first so its focused `TextEdit` consumes keys
        // before `render_vt100_viewport` checks keyboard input.
        let composer_rect = crate::agent_composer::show_composer(
            state,
            ui,
            rect,
            ctx_r.bottom(),
            file_path,
            crate::agent_composer::MentionScope { doc, page: current_page },
            &mut actions,
        );

        // Live VT100 PTY Terminal Viewport above the composer; the turn receipt row sits between them (`#683`).
        let active_receipt =
            state.active_tab().filter(|t| !matches!(t.effective_status(), AgentTabStatus::Working(_))).and_then(|t| t.last_turn_receipt().cloned());
        let receipt_h = if active_receipt.is_some() { 24.0 } else { 0.0 };
        let vp_bottom = (composer_rect.top() - theme::SPACE_INLINE - receipt_h).max(ctx_r.bottom() + theme::CONTROL_ROW);
        let vp_r = Rect::from_min_max(Pos2::new(rect.left(), ctx_r.bottom()), Pos2::new(rect.right(), vp_bottom));

        if ui.input(|i| i.pointer.any_pressed()) {
            if let Some(pos) = ui.input(|i| i.pointer.interact_pos()) {
                if vp_r.contains(pos) {
                    state.viewport_focused = true;
                    ui.memory_mut(|m| m.surrender_focus(composer_input_id));
                }
            }
        }

        render_vt100_viewport(state, ui, vp_r, file_path, "agents-left-pty");
        if !state.viewport_focused && ui.input(|i| i.key_pressed(Key::Escape)) {
            let composer_focused = ui.ctx().memory(|m| m.has_focus(Id::new("agents-ask-for-changes-input")));
            crate::agent_stop::handle_panel_escape(state, composer_focused, designos_doc::now_ms());
        }
        if let Some(receipt) = &active_receipt {
            let receipt_r = Rect::from_min_max(Pos2::new(rect.left(), vp_bottom), Pos2::new(rect.right(), vp_bottom + receipt_h));
            paint_turn_receipt_row(state, &mut actions, ui, &p, receipt_r, receipt);
        }
        if let Some(fix) = state.pending_start_fix.take() {
            state.apply_start_fix(fix, file_path);
        }

        if state.mention_popup.is_open_for_surface("agents-left-pty") {
            let mut unused_text = String::new();
            let show_outcome = state.mention_popup.show(ui, rect, ctx_r, &mut unused_text, &mut state.active_mentions);
            if let crate::agent_mention_popup::MentionPopupOutcome::PickedTerminal(mention) = show_outcome {
                state.handle_terminal_mention_pick(&mention);
            }
        }

        // Floating `+` Runtime Selector Dropdown (`claude`, `codex`, `grok`, `antigravity`, `shell` — all skipping dangerous permissions)
        if state.runtime_dropdown_open {
            render_runtime_dropdown_menu(state, ui, rect, bar_r.bottom() + 4.0, file_path);
        }
        if state.sessions_popover_open {
            crate::agent_open_flow::render_sessions_popover(state, ui, rect, bar_r.bottom() + theme::INSET_SEGMENT, file_path);
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
    }

    p.text(Pos2::new(header_rect.left() + 34.0, header_rect.top() + 17.0), Align2::LEFT_CENTER, "New chat", theme::font_body_strong(), Color32::WHITE);
    p.text(
        Pos2::new(header_rect.left() + 34.0, header_rect.top() + 33.0),
        Align2::LEFT_CENTER,
        "Shared",
        theme::font_caption(),
        Color32::from_rgb(0x8C, 0x8C, 0x8C),
    );
    if state.needs_approval() {
        let pill_w = widgets::text_width(&p, "Needs approval", &theme::font_caption_medium()) + 16.0;
        let pill_r = Rect::from_min_size(Pos2::new(header_rect.left() + 78.0, header_rect.top() + 24.0), Vec2::new(pill_w, 18.0));
        p.circle_filled(Pos2::new(pill_r.left() + 4.0, pill_r.center().y), 2.5, STATUS_WARN_TEXT);
        p.text(Pos2::new(pill_r.left() + 10.0, pill_r.center().y), Align2::LEFT_CENTER, "Needs approval", theme::font_caption_medium(), STATUS_WARN_TEXT);
        widgets::record(ui, "agents-needs-approval-pill", pill_r);
    }

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

    // ---- D. Bottom Docked Composer Box (`#689` §7)
    let composer_rect =
        crate::agent_composer::show_composer(state, ui, rect, top_y, file_path, crate::agent_composer::MentionScope { doc, page: current_page }, &mut actions);

    // ---- C. Center Area (`between runtime bar and bottom composer`)
    let full_center_rect = Rect::from_min_max(Pos2::new(rect.left(), top_y), Pos2::new(rect.right(), (composer_rect.top() - 8.0).max(top_y + 32.0)));
    let active_preset = state.active_tab().map(|t| t.preset).unwrap_or(state.active_runtime);
    let center_rect = if state.needs_approval() {
        let reqs = state.pending_approvals.clone();
        if let Some(act) = crate::agent_approval_card::paint_approval_card(ui, full_center_rect, active_preset, &reqs, "agents-left-chat") {
            state.handle_approval_card_action(act);
        }
        let card_h = crate::agent_approval_card::approval_card_outer_height(active_preset);
        Rect::from_min_max(Pos2::new(full_center_rect.left(), (full_center_rect.top() + card_h).min(full_center_rect.bottom())), full_center_rect.max)
    } else {
        full_center_rect
    };

    clipped(ui, center_rect, |ui| {
        let p = ui.painter().clone();
        let badge_center = Pos2::new(center_rect.center().x, center_rect.top() + 44.0);
        paint_agents_grid_plus_badge(&p, badge_center);

        p.text(Pos2::new(center_rect.center().x, center_rect.top() + 82.0), Align2::CENTER_CENTER, "What's next?", theme::font_large_strong(), Color32::WHITE);

        let mut y = center_rect.top() + 108.0;
        let card_x = center_rect.left() + 14.0;
        let card_w = (center_rect.width() - 28.0).max(100.0);
        let card_h = 40.0;

        for (cat_idx, cat_id, cat_title) in [
            (0usize, "agents-cat-tidy", "Tidy and polish"),
            (1usize, "agents-cat-generate", "Generate designs"),
            (2usize, "agents-cat-plugins", "Create plugins and shaders"),
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
                        ("term-ai-make-design", "Make design (First Draft)", "Make Design", MAKE_DESIGN_FIRST_DRAFT_PROMPT),
                        ("term-ai-make-proto", "Make prototype flows", "Make Prototype", "Auto-wire OnClick -> NavigateTo SmartAnimate flows across frames"),
                        (
                            "term-ai-populate",
                            "Populate realistic data",
                            "Populate Content",
                            "Populate realistic SaaS metrics, names, and statuses into repeated cards",
                        ),
                    ],
                    _ => &[
                        ("agents-open-pty", "Open Live PTY Terminal", "Open PTY", ""),
                        ("term-left-send-sel", "Send selection & comments to PTY", "Send Selection", ""),
                        ("term-left-apply-out", "Apply NodeSpec from PTY output", "Apply Spec", ""),
                        ("term-left-sync-disk", "Sync canvas from disk", "Sync Canvas", ""),
                    ],
                };
                for &(sub_id, sub_label, _task_title, cli_prompt) in sub_items {
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
                                let _ = state.send_to_agent(cli_prompt, file_path);
                            }
                        }
                    }
                    y += 32.0;
                }
                y += 4.0;
            }
        }
    });

    actions
}

fn render_runtime_dropdown_menu(state: &mut TerminalState, ui: &mut Ui, panel_rect: Rect, top_y: f32, file_path: Option<&Path>) {
    crate::agent_picker::render_runtime_dropdown_menu(state, ui, panel_rect, top_y, file_path);
}

/// Renders the one-line "Worked for" receipt row (`#683` §3.4, §5):
/// 24px row height, 8px horizontal padding, `theme::font_caption()` in `theme::TEXT_SECONDARY` (`#A1A1AA`),
/// numbers in `theme::font_mono()`. Clicking the receipt row toggles the per-turn summary (`#677`).
fn paint_turn_receipt_row(
    state: &mut TerminalState,
    actions: &mut Vec<TerminalAction>,
    ui: &Ui,
    p: &Painter,
    row_rect: Rect,
    receipt: &crate::agent_turn_status::TurnReceipt,
) {
    let resp = ui.interact(row_rect, Id::new("agents-turn-receipt-line"), Sense::click());
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    let mut job = egui::text::LayoutJob::default();
    for (piece, is_mono) in receipt.styled_runs() {
        let font_id = if is_mono { theme::font_mono() } else { theme::font_caption() };
        job.append(&piece, 0.0, egui::TextFormat { font_id, color: theme::TEXT_SECONDARY, ..Default::default() });
    }
    let galley = p.layout_job(job);
    let text_pos = Pos2::new(row_rect.left() + 8.0, row_rect.center().y - galley.size().y * 0.5);
    p.galley(text_pos, galley, theme::TEXT_SECONDARY);
    widgets::record(ui, "agents-turn-receipt-line", row_rect);
    if resp.clicked() {
        if let Some(tab) = state.active_tab_mut() {
            tab.turn_tracker.summary_open = !tab.turn_tracker.summary_open;
            let tab_id = tab.id;
            actions.push(TerminalAction::OpenTurnReceipt(tab_id));
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
    let summary = crate::agent_selection_chip::SelectionSummary::empty(&state.current_page_name);
    let empty_doc = Document::new(&state.doc_name);
    let page = NodeId(state.current_page_id.max(1));
    render_terminal_surface(state, ui, rect, file_path, "full", &summary, &empty_doc, page)
}

/// Renders the docked bottom split-view Agent Terminal (`Ctrl+\`` / `dock_open`).
pub fn show_dock(state: &mut TerminalState, ui: &mut Ui, rect: Rect, file_path: Option<&Path>) -> Vec<TerminalAction> {
    let summary = crate::agent_selection_chip::SelectionSummary::empty(&state.current_page_name);
    show_dock_with_selection(state, ui, rect, file_path, &summary)
}

/// Renders the docked bottom split-view Agent Terminal (`Ctrl+\`` / `dock_open`) with the live
/// [`crate::agent_selection_chip::SelectionSummary`] computed in the current frame from `app`.
pub fn show_dock_with_selection(
    state: &mut TerminalState,
    ui: &mut Ui,
    rect: Rect,
    file_path: Option<&Path>,
    selection_summary: &crate::agent_selection_chip::SelectionSummary,
) -> Vec<TerminalAction> {
    let empty_doc = Document::new(&state.doc_name);
    let page = NodeId(state.current_page_id.max(1));
    show_dock_with_doc(state, ui, rect, file_path, selection_summary, &empty_doc, page)
}

/// Renders the docked bottom split-view Agent Terminal with live [`Document`] access for the `@`-mention picker (`#655`).
pub fn show_dock_with_doc(
    state: &mut TerminalState,
    ui: &mut Ui,
    rect: Rect,
    file_path: Option<&Path>,
    selection_summary: &crate::agent_selection_chip::SelectionSummary,
    doc: &Document,
    current_page: NodeId,
) -> Vec<TerminalAction> {
    let grip_rect = Rect::from_min_size(Pos2::new(rect.left(), rect.top() - 3.0), Vec2::new(rect.width(), 6.0));
    widgets::record(ui, "dock-resize-grip", grip_rect);
    let actions = render_terminal_surface(state, ui, rect, file_path, "dock", selection_summary, doc, current_page);
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

#[allow(clippy::too_many_arguments)]
fn render_terminal_surface(
    state: &mut TerminalState,
    ui: &mut Ui,
    rect: Rect,
    file_path: Option<&Path>,
    surface_id: &'static str,
    selection_summary: &crate::agent_selection_chip::SelectionSummary,
    doc: &Document,
    current_page: NodeId,
) -> Vec<TerminalAction> {
    let mut actions = state.poll_live_agent_bridge();
    crate::agent_a11y::label_terminal_surface(state, ui, surface_id);
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
    right_x = send_r.left() - theme::SPACE_INLINE;
    if ui.interact(send_r, Id::new((surface_id, "term-bar-send-sel")), Sense::click()).clicked() {
        actions.push(TerminalAction::ExportSelectionToTerminal);
    }

    // `@ Mention` button in the terminal header (#655)
    let min_tabs_left = tab_bar_rect.left() + 148.0;
    if right_x - min_tabs_left >= theme::TARGET_MIN {
        let (mention_btn_r, mention_clicked) = crate::agent_mention_popup::paint_terminal_mention_button(
            ui,
            "agent-mention-btn-dock",
            right_x,
            tab_bar_rect,
            state.mention_popup.is_open_for_surface(surface_id),
        );
        right_x = mention_btn_r.left() - theme::SPACE_INLINE;
        if mention_clicked {
            state.mention_popup.toggle_terminal_mode_for(surface_id, doc, current_page);
        }
    }

    // Live selection chip at the left of the bar's right-hand button group (#654)
    if right_x - min_tabs_left >= theme::TARGET_MIN {
        let chip_max_w = (right_x - min_tabs_left).min(260.0);
        let chip_bounds = Rect::from_min_max(Pos2::new(right_x - chip_max_w, tab_bar_rect.top()), Pos2::new(right_x, tab_bar_rect.bottom()));
        if let Some((chip_rect, chip_clicked)) =
            crate::agent_selection_chip::paint_selection_chip(ui, "agent-selection-chip-dock", selection_summary, chip_bounds, true)
        {
            right_x = chip_rect.left() - theme::SPACE_INLINE;
            if chip_clicked {
                actions.push(TerminalAction::SendSelectionToAgent);
            }
        }
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
        let tab_waiting = tab.needs_approval() || (is_active && !state.pending_approvals.is_empty());
        let tab_resp = ui.interact(t_hit_rect, Id::new((surface_id, "term-tab-select", tab.id)), Sense::click());
        let tab_resp = if tab_waiting { tab_resp.on_hover_text("Needs approval") } else { tab_resp };
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
        if let Some(tab) = state.tabs.get_mut(idx) {
            tab.unread = false;
        }
        state.sync_active_tab_approvals();
        state.viewport_focused = true;
    }
    if state.surface_visible {
        if let Some(tab) = state.active_tab_mut() {
            tab.unread = false;
        }
    }
    if let Some(idx) = tab_to_close {
        state.close_tab(idx);
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

    let now_secs = crate::agent_sessions::unix_timestamp_secs();
    if let Some((dot_col, status_txt)) = state.active_tab().and_then(|t| t.header_status_indicator(now_secs)) {
        let avail_w = (right_x - left_x - 16.0).max(0.0);
        if avail_w >= 48.0 {
            let status_elided = widgets::elide(&p, &status_txt, &theme::font_caption(), avail_w - 14.0);
            p.circle_filled(Pos2::new(left_x + 7.0, tab_bar_rect.center().y), 3.0, dot_col);
            p.text(Pos2::new(left_x + 16.0, tab_bar_rect.center().y), Align2::LEFT_CENTER, status_elided, theme::font_caption(), theme::TEXT_SECONDARY);
        }
    }

    if state.mention_popup.is_open_for_surface(surface_id) {
        state.mention_popup.sync_from_composer_text("", 0, doc, current_page);
        let mut unused_text = String::new();
        let kb_outcome = state.mention_popup.handle_keyboard(ui.ctx(), &mut unused_text, &mut state.active_mentions, doc, current_page);
        if let crate::agent_mention_popup::MentionPopupOutcome::PickedTerminal(mention) = kb_outcome {
            state.handle_terminal_mention_pick(&mention);
        }
    }

    // 2. VT100 Screen Grid Renderer + Raw Keyboard Capture (full height)
    let active_receipt =
        state.active_tab().filter(|t| !matches!(t.effective_status(), AgentTabStatus::Working(_))).and_then(|t| t.last_turn_receipt().cloned());
    let vp_bottom = if active_receipt.is_some() { (rect.bottom() - 24.0).max(tab_bar_rect.bottom()) } else { rect.bottom() };
    let viewport_rect = Rect::from_min_max(Pos2::new(rect.left(), tab_bar_rect.bottom()), Pos2::new(rect.right(), vp_bottom));
    render_vt100_viewport(state, ui, viewport_rect, file_path, surface_id);
    if let Some(receipt) = &active_receipt {
        let receipt_r = Rect::from_min_max(Pos2::new(rect.left(), vp_bottom), rect.max);
        paint_turn_receipt_row(state, &mut actions, ui, &p, receipt_r, receipt);
    }
    if let Some(fix) = state.pending_start_fix.take() {
        state.apply_start_fix(fix, file_path);
    }

    if state.mention_popup.is_open_for_surface(surface_id) {
        let mut unused_text = String::new();
        let show_outcome = state.mention_popup.show(ui, rect, tab_bar_rect, &mut unused_text, &mut state.active_mentions);
        if let crate::agent_mention_popup::MentionPopupOutcome::PickedTerminal(mention) = show_outcome {
            state.handle_terminal_mention_pick(&mention);
        }
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

fn render_vt100_viewport(state: &mut TerminalState, ui: &mut Ui, viewport_rect: Rect, file_path: Option<&Path>, surface_id: &'static str) {
    if let Some(failure) = state.active_tab().and_then(|t| t.start_failure.clone()) {
        if let Some(fix) = agent_start_failure::paint(ui, viewport_rect, &failure, surface_id) {
            state.pending_start_fix = Some(fix);
        }
        return;
    }
    let p = ui.painter().clone();
    p.rect_filled(viewport_rect, 0.0, TERM_BG_VIEWPORT);

    let active_preset = state.active_tab().map(|t| t.preset).unwrap_or(state.active_runtime);
    let viewport_rect = if state.needs_approval() {
        let reqs = state.pending_approvals.clone();
        if let Some(act) = crate::agent_approval_card::paint_approval_card(ui, viewport_rect, active_preset, &reqs, surface_id) {
            state.handle_approval_card_action(act);
        }
        let card_h = crate::agent_approval_card::approval_card_outer_height(active_preset);
        Rect::from_min_max(Pos2::new(viewport_rect.left(), (viewport_rect.top() + card_h).min(viewport_rect.bottom())), viewport_rect.max)
    } else {
        viewport_rect
    };
    if state.picker_open || state.tabs.is_empty() {
        if !state.runtime_dropdown_open && !state.sessions_popover_open {
            crate::agent_picker::show_picker_panel(state, ui, viewport_rect, file_path);
        }
        return;
    }

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
    let viewport_focused = state.viewport_focused && !state.mention_popup.is_open();
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
        if viewport_focused
            && (!crate::agent_a11y::non_canvas_has_focus(ui.ctx())
                || ui.ctx().memory(|m| m.has_focus(vp_resp.id) || m.has_focus(crate::agent_a11y::composer_id())))
        {
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
                                    Key::Z if modifiers.ctrl && !modifiers.mac_cmd && !modifiers.shift => {
                                        raw_bytes.push(0x1a);
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
                                    Key::Z if !modifiers.shift => raw_bytes.push(0x1a),
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
                .id(Id::new((surface_id, "term-search-input")))
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
        let mut st = TerminalState::default();
        st.last_watched_mtime = Some(std::time::UNIX_EPOCH);
        st.live_spec_path = live_path.clone();
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

        st.spawn_tab(AgentCliPreset::Shell, None).expect("spawn");
        {
            let tab = st.active_tab_mut().unwrap();
            tab.spawn_preset = AgentCliPreset::Claude;
            tab.preset = AgentCliPreset::Claude;
        }
        let rec = attach_recorder(st.active_tab_mut().unwrap());
        let outcome = st.send_to_agent("Create a SaaS analytics card in Claude Code", None);
        assert_eq!(outcome, crate::agent_composer::SendOutcome::Written);
        st.flush_pending_enter_now();
        let written = String::from_utf8_lossy(&rec.lock().unwrap()).into_owned();
        assert!(written.contains("Create a SaaS analytics card in Claude Code"));
        assert!(written.ends_with('\r'));

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
        let text_findings = crate::lint::check_text(&texts, &boxes);
        let icon_findings = crate::lint::check_icons(&crate::lint::take_icons());
        assert!(text_findings.is_empty(), "Agents panel text lint findings: {text_findings:?}");
        assert!(icon_findings.is_empty(), "Agents panel icon lint findings: {icon_findings:?}");

        for mut tab in st.tabs.drain(..) {
            let _ = tab.child.kill();
        }
    }

    #[test]
    fn all_four_agent_cli_runtimes_claude_codex_agy_grok_execute_in_pty() {
        let mut st = TerminalState::default();
        st.spawn_tab(AgentCliPreset::Shell, None).expect("spawn");
        for (preset, prompt) in [
            (AgentCliPreset::Claude, "Audit layout in Claude"),
            (AgentCliPreset::Codex, "Generate React code in Codex"),
            (AgentCliPreset::Antigravity, "Check parity in Antigravity"),
            (AgentCliPreset::Grok, "Fast build in Grok"),
        ] {
            {
                let tab = st.active_tab_mut().unwrap();
                tab.spawn_preset = preset;
                tab.preset = preset;
            }
            let rec = attach_recorder(st.active_tab_mut().unwrap());
            st.submit_chat_prompt(prompt, None);
            st.flush_pending_enter_now();
            let written = String::from_utf8_lossy(&rec.lock().unwrap()).into_owned();
            assert!(written.contains(prompt), "Runtime {preset:?} must write prompt to live PTY: got {written:?}");
            assert!(written.ends_with('\r'));
        }
        for mut tab in st.tabs.drain(..) {
            let _ = tab.child.kill();
        }
    }

    #[test]
    fn agent_chat_supports_prd_and_spec_context_handoff() {
        let mut st = TerminalState::default();
        st.spawn_tab(AgentCliPreset::Shell, None).expect("spawn");
        {
            let tab = st.active_tab_mut().unwrap();
            tab.spawn_preset = AgentCliPreset::Claude;
            tab.preset = AgentCliPreset::Claude;
        }
        let rec = attach_recorder(st.active_tab_mut().unwrap());
        st.submit_chat_prompt("@prd Review & implement spec block (REQ-01)", None);
        st.flush_pending_enter_now();
        let written = String::from_utf8_lossy(&rec.lock().unwrap()).into_owned();
        assert!(written.contains("@prd Review & implement spec block (REQ-01)"), "Live PTY should receive @prd prompt: {written}");

        for mut tab in st.tabs.drain(..) {
            let _ = tab.child.kill();
        }
    }

    #[test]
    fn agent_runtime_env_is_set_at_spawn_and_per_command_never_typed_into_live_tabs() {
        let mut cmd = CommandBuilder::new("sh");
        configure_pty_env(&mut cmd, AgentCliPreset::Codex, None);
        assert_eq!(cmd.get_env("DESIGNOS_AGENT_RUNTIME"), Some(std::ffi::OsStr::new("codex")));
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

        let mut st = TerminalState::default();
        st.spawn_tab(AgentCliPreset::Shell, None).expect("spawn");
        {
            let tab = st.active_tab_mut().unwrap();
            tab.spawn_preset = AgentCliPreset::Claude;
            tab.preset = AgentCliPreset::Claude;
        }
        let rec = attach_recorder(st.active_tab_mut().unwrap());
        st.dispatch_figma_ai_prompt("Auto-rename layers to semantic names", "Auto-rename layers to semantic names", None);
        st.flush_pending_enter_now();
        assert!(st.dock_open);
        let written = String::from_utf8_lossy(&rec.lock().unwrap()).into_owned();
        assert!(written.contains("Auto-rename layers to semantic names"));
        assert!(!written.trim_start().starts_with('#'));
        for mut tab in st.tabs.drain(..) {
            let _ = tab.child.kill();
        }
    }

    #[test]
    fn agents_header_and_composer_buttons_mutate_real_state() {
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        crate::theme::apply(&ctx);
        let mut st = TerminalState::default();
        st.side_panel_pty_open = false;
        st.chat_mode = true;
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
        assert!(
            ctx.data(|d| d.get_temp::<Rect>(Id::new(("test-rect", "agents-composer-mic".to_string())))).is_none(),
            "agents-composer-mic must not be rendered"
        );

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
        assert!(st.composer_draft().contains("@selection"));
        assert!(acts.contains(&TerminalAction::ExportSelectionToTerminal));

        // Click `PRD` context chip -> cycles next context tag (@tokens since @prd & @selection are present)
        let _ = click_at(&ctx, &mut st, prd_rect.center());
        assert!(st.composer_draft().contains("@tokens"));

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

    #[test]
    fn failed_start_drain_in_the_bridge_poll_never_blocks_the_ui_thread() {
        let src = include_str!("terminal.rs");
        let start = src.find("    pub fn poll_live_agent_bridge(").expect("poll_live_agent_bridge exists");
        let rest = &src[start + 1..];
        let end = rest.find("\n    pub fn ").expect("a following method");
        let body = &rest[..end];
        assert!(!body.contains("thread::sleep"), "the bridge poll runs on the UI thread and must not sleep");
        assert!(body.contains("exited_at"), "a fast-exiting CLI is given its output window by deferring across frames (`exited_at`)");
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
        let run_seq = st.next_run_seq.max(1);
        st.next_run_seq = run_seq + 1;
        st.active_run_id = format!("{}-{}-{}", st.session_id, id, run_seq);
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
        let _ = std::fs::remove_dir_all(&st.session_dir);
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
        let _ = std::fs::remove_dir_all(&st.session_dir);
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
        {
            let tab = st.active_tab_mut().unwrap();
            tab.spawn_preset = AgentCliPreset::Claude;
            tab.preset = AgentCliPreset::Claude;
        }
        let rec = attach_recorder(st.active_tab_mut().unwrap());
        st.submit_chat_prompt("hello world\x07", None);
        st.flush_pending_enter_now();
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
        for rel in ["SKILL.md", ".claude/skills/designos-studio/SKILL.md", ".agents/skills/designos-studio/SKILL.md", "AGENTS.md"] {
            let text = std::fs::read_to_string(st.session_dir.join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"));
            assert!(text.contains("designos-studio") && text.contains("Skill Layout File"), "{rel}: {text}");
            assert!(text.ends_with(designos_mcp::rules::GENERATED_RULES_MD.trim_start()), "{rel} must include the generated rules pack");
        }
    }

    #[test]
    fn session_agent_files_preserve_enabled_tools_and_filter_skill_md() {
        let mut st = TerminalState::for_tab(799);
        let mut doc = Document::new("Enabled Tools Session File");
        let page1 = doc.add_page("Page 1");
        let page2 = doc.add_page("Page 2");
        st.sync_live_file_context(&doc, page1, &[], None);

        let mcp_cfg_path = st.session_dir.join("mcp-config.json");
        let initial_cfg = read_json(&mcp_cfg_path);
        assert!(initial_cfg.get("designos").is_none(), "the default (every tool) is never written down: {initial_cfg}");
        assert_eq!(initial_cfg["mcpServers"]["designos"]["env"]["DESIGNOS_MCP_CONFIG"].as_str(), Some(mcp_cfg_path.display().to_string().as_str()));

        // A restricted tool list written into mcp-config.json survives the next page sync.
        let mut custom_cfg = initial_cfg.clone();
        custom_cfg["designos"] = serde_json::json!({"enabled_tools": ["designos_get_document", "designos_list_nodes"]});
        std::fs::write(&mcp_cfg_path, serde_json::to_vec_pretty(&custom_cfg).unwrap()).unwrap();

        st.sync_live_file_context(&doc, page2, &[], None);

        let after_cfg = read_json(&mcp_cfg_path);
        assert_eq!(
            after_cfg["designos"]["enabled_tools"],
            serde_json::json!(["designos_get_document", "designos_list_nodes"]),
            "provision_session_agent_files must preserve configured designos.enabled_tools"
        );
        let snapshot = after_cfg["designos"]["known_tools"].as_array().expect("known_tools snapshot recorded next to a custom list");
        assert_eq!(snapshot.len(), designos_mcp::server::tools().len());

        for rel in ["SKILL.md", "AGENTS.md", ".claude/skills/designos-studio/SKILL.md", ".agents/skills/designos-studio/SKILL.md"] {
            let content = std::fs::read_to_string(st.session_dir.join(rel)).unwrap_or_else(|e| panic!("read {rel}: {e}"));
            assert!(content.contains("Mode: read-only"), "{rel} must reflect read-only enabled_tools filter");
            assert!(content.contains("### `designos_get_document`"), "{rel} must include enabled tool");
            assert!(!content.contains("### `designos_create_node`"), "{rel} must omit disabled tool section");
            assert!(!content.contains("### `designos_render_png`"), "{rel} must omit disabled render_png section");
        }
        let _ = std::fs::remove_dir_all(&st.session_dir);
    }

    #[test]
    fn session_written_before_a_tool_existed_gets_the_new_tool_on_the_next_sync() {
        let mut st = TerminalState::for_tab(798);
        let mut doc = Document::new("Persistent Session File");
        let page = doc.add_page("Page 1");
        let mcp_cfg_path = st.session_dir.join("mcp-config.json");

        // An older build wrote a restricted list against a tool set without `designos_lint`.
        let older: Vec<String> = designos_mcp::server::tools().into_iter().map(|t| t.name.to_string()).filter(|n| n != "designos_lint").collect();
        let cfg = serde_json::json!({"designos": {"enabled_tools": ["designos_get_document"], "known_tools": older}});
        std::fs::create_dir_all(&st.session_dir).unwrap();
        std::fs::write(&mcp_cfg_path, serde_json::to_vec_pretty(&cfg).unwrap()).unwrap();

        st.sync_live_file_context(&doc, page, &[], None);

        let after = read_json(&mcp_cfg_path);
        assert_eq!(after["designos"]["enabled_tools"], serde_json::json!(["designos_get_document", "designos_lint"]));
        let reloaded = designos_mcp::rules::EnabledToolsConfig::from_mcp_config_json(&after.to_string());
        assert!(reloaded.is_tool_enabled("designos_lint") && !reloaded.is_tool_enabled("designos_create_node"));

        // A session on the default never gets a frozen copy of the tool list.
        let mut default_st = TerminalState::for_tab(797);
        default_st.sync_live_file_context(&doc, page, &[], None);
        assert!(read_json(&default_st.session_dir.join("mcp-config.json")).get("designos").is_none());
        let _ = std::fs::remove_dir_all(&st.session_dir);
        let _ = std::fs::remove_dir_all(&default_st.session_dir);
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
        assert!(tab.contains("codex --dangerously-bypass-approvals-and-sandbox"), "{tab}");
        assert!(!codex_list_override(tab, "mcp_servers.designos.args").is_empty(), "{tab}");
        assert!(!codex_list_override(tab, "mcp_servers.designos.env_vars").is_empty(), "{tab}");
        assert!(tab.contains("--workspace") && tab.contains("$DESIGNOS_WORKSPACE"), "{tab}");
    }

    /// A state whose stub agent has exited but whose exit the terminal has not yet observed: the
    /// bridge is not polled here, so each test decides the spawn time the exit is judged against.
    fn exited_stub_state() -> TerminalState {
        let mut st = TerminalState::default();
        spawn_stub_exiting_agent_into(&mut st);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
        while st.active_tab_mut().unwrap().is_alive() || !st.active_tab().unwrap().reader_eof.load(std::sync::atomic::Ordering::Acquire) {
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
        let mut st = TerminalState::for_tab(782);
        st.apply_start_fix(FixAction::OpenShell, None);
        assert_eq!(st.tabs.last().map(|t| t.spawn_preset), Some(AgentCliPreset::Shell));
        assert_eq!(st.tabs.last().and_then(|t| t.spawned_command.as_deref()), None);

        st.apply_start_fix(FixAction::RunLogin("echo LOGIN-RAN-HERE"), None);
        let tab = st.tabs.last().expect("login tab");
        assert_eq!(tab.title, "echo LOGIN-RAN-HERE (login)");
        assert_eq!(tab.spawn_preset, AgentCliPreset::Shell, "a login is not an agent tab");
        assert_eq!(tab.spawned_command.as_deref(), Some("echo LOGIN-RAN-HERE"));
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
        let mut shown = false;
        while std::time::Instant::now() < deadline && !shown {
            std::thread::sleep(std::time::Duration::from_millis(25));
            let _ = st.tabs.last_mut().map(|t| t.is_alive());
            shown = st.tabs.last().and_then(|t| t.raw_log.lock().ok().map(|l| l.contains("LOGIN-RAN-HERE"))).unwrap_or(false);
        }
        assert!(shown, "login command output never reached the tab");
        kill_tabs(&mut st);
        let _ = std::fs::remove_dir_all(&st.session_dir);
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

    #[test]
    fn install_dirs_include_each_clis_own_folder() {
        let home = std::env::var("HOME").unwrap_or_default();
        if !home.is_empty() {
            let grok_dirs = AgentCliPreset::Grok.install_dirs();
            assert!(grok_dirs.contains(&PathBuf::from(format!("{home}/.grok/bin"))), "{grok_dirs:?}");
            let agy_dirs = AgentCliPreset::Antigravity.install_dirs();
            assert!(agy_dirs.contains(&PathBuf::from(format!("{home}/.antigravity/bin"))), "{agy_dirs:?}");
            assert!(agy_dirs.contains(&PathBuf::from(format!("{home}/.antigravity/antigravity/bin"))), "{agy_dirs:?}");
            let gemini_dirs = AgentCliPreset::Gemini.install_dirs();
            assert!(gemini_dirs.contains(&PathBuf::from(format!("{home}/.npm-global/bin"))), "{gemini_dirs:?}");
            let opencode_dirs = AgentCliPreset::OpenCode.install_dirs();
            assert!(opencode_dirs.contains(&PathBuf::from(format!("{home}/.opencode/bin"))), "{opencode_dirs:?}");
        }
    }

    #[test]
    fn probe_and_child_path_use_one_list() {
        let probe_list = AgentCliPreset::Grok.install_dirs();
        let child_list = extra_cli_path_dirs();
        assert_eq!(probe_list, child_list);
    }

    #[test]
    fn stub_cli_off_the_probed_path_that_prints_unauthorized() {
        let tmp = std::env::temp_dir().join(format!("designos-673-stub-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        let stub_path = tmp.join("stub-agent-673");
        std::fs::write(&stub_path, "#!/bin/sh\necho 'Error: unauthorized'\nexit 1\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&stub_path).unwrap().permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&stub_path, perms).unwrap();
        }
        assert!(!runtime_binary_available("stub-agent-673"), "stub name must not be on PATH or in install_dirs()");

        let mut cmd = build_agent_shell_cmd(&default_shell_program(), &format!("'{}'", stub_path.display()));
        isolate_zsh_from_home(&mut cmd);
        let tab = TerminalTab::spawn_with_session_cmd(1, AgentCliPreset::Grok, 24, 80, None, None, Some(cmd)).expect("spawn stub tab");
        let mut st = TerminalState::default();
        st.tabs = vec![tab];
        st.active_tab = 0;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
        while st.active_tab_mut().unwrap().is_alive() || !st.active_tab().unwrap().raw_log.lock().map(|l| l.contains("unauthorized")).unwrap_or(false) {
            assert!(std::time::Instant::now() < deadline, "stub agent must print unauthorized and exit");
            std::thread::sleep(std::time::Duration::from_millis(25));
        }
        st.active_tab_mut().unwrap().spawned_at = std::time::Instant::now();
        let _ = st.poll_live_agent_bridge();
        let failure = st.active_tab().and_then(|t| t.start_failure.clone()).expect("start failure recorded");
        assert_eq!(failure.cause, agent_start_failure::StartFailureCause::NotLoggedIn);
        assert_eq!(failure.fix, FixAction::RunLogin("grok login"));
        let _ = std::fs::remove_dir_all(&tmp);
    }

    fn spawn_live_claude_stub_tab(st: &mut TerminalState) {
        st.spawn_tab(AgentCliPreset::Shell, None).expect("spawn stub tab");
        if let Some(tab) = st.active_tab_mut() {
            tab.spawn_preset = AgentCliPreset::Claude;
            tab.preset = AgentCliPreset::Claude;
        }
    }

    #[test]
    fn prompt_with_a_stub_cli_that_writes_nothing_leaves_the_document_hash_unchanged() {
        let mut doc = Document::new("681 Noop Stub Doc");
        let page = doc.add_page("Page 1");
        let doc_hash = |d: &Document| -> u64 {
            use std::hash::{Hash, Hasher};
            let mut h = std::collections::hash_map::DefaultHasher::new();
            d.to_json().expect("doc.to_json").hash(&mut h);
            h.finish()
        };
        let hash_before = doc_hash(&doc);

        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        crate::theme::apply(&ctx);
        let mut st = TerminalState::for_tab(68101);
        st.side_panel_pty_open = false;
        st.chat_mode = true;
        st.active_runtime = AgentCliPreset::Claude;
        spawn_live_claude_stub_tab(&mut st);
        st.set_composer_draft("Design a pricing card");
        st.sync_live_file_context(&doc, page, &[], None);
        let panel_rect = Rect::from_min_size(Pos2::new(48.0, 0.0), Vec2::new(260.0, 800.0));

        let _ = ctx.run_ui(egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1600.0, 900.0))), ..Default::default() }, |ui| {
            let _ = show_left(&mut st, ui, panel_rect, None);
        });
        let send_rect: Rect = ctx.data(|d| d.get_temp(Id::new(("test-rect", "agents-composer-send".to_string())))).expect("agents-composer-send recorded");
        let send_pos = send_rect.center();

        let mut actions = Vec::new();
        let raw = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1600.0, 900.0))),
            events: vec![
                egui::Event::PointerMoved(send_pos),
                egui::Event::PointerButton { pos: send_pos, button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE },
                egui::Event::PointerButton { pos: send_pos, button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE },
            ],
            ..Default::default()
        };
        let _ = ctx.run_ui(raw, |ui| {
            actions = show_left(&mut st, ui, panel_rect, None);
        });

        assert!(
            !actions.iter().any(|a| matches!(a, TerminalAction::ApplySpecJson(_) | TerminalAction::ApplySpecFile(_))),
            "submitting prompt with noop stub must emit no ApplySpecJson or ApplySpecFile: {actions:?}"
        );
        assert_eq!(doc_hash(&doc), hash_before, "document hash must remain unchanged");
        kill_tabs(&mut st);
    }

    #[test]
    fn design_words_do_not_apply_a_template() {
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        crate::theme::apply(&ctx);
        let panel_rect = Rect::from_min_size(Pos2::new(48.0, 0.0), Vec2::new(260.0, 800.0));

        let mut st = TerminalState::for_tab(68102);
        st.side_panel_pty_open = false;
        st.chat_mode = true;
        st.active_runtime = AgentCliPreset::Claude;
        spawn_live_claude_stub_tab(&mut st);

        for prompt in
            ["create a card", "build a dashboard", "make a hero", "generate a form", "landing page", " template ", "ui for login", "screen for checkout"]
        {
            st.set_composer_draft(prompt);
            let _ = ctx.run_ui(egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1600.0, 900.0))), ..Default::default() }, |ui| {
                let _ = show_left(&mut st, ui, panel_rect, None);
            });
            let send_pos = ctx.data(|d| d.get_temp::<Rect>(Id::new(("test-rect", "agents-composer-send".to_string())))).expect("agents-composer-send").center();
            let mut actions = Vec::new();
            let raw = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1600.0, 900.0))),
                events: vec![
                    egui::Event::PointerMoved(send_pos),
                    egui::Event::PointerButton { pos: send_pos, button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE },
                    egui::Event::PointerButton { pos: send_pos, button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE },
                ],
                ..Default::default()
            };
            let _ = ctx.run_ui(raw, |ui| {
                actions = show_left(&mut st, ui, panel_rect, None);
            });
            assert!(!actions.iter().any(|a| matches!(a, TerminalAction::ApplySpecJson(_))), "prompt {prompt:?} must not produce ApplySpecJson: {actions:?}");
        }

        let terminal_src = include_str!("terminal.rs");
        let shipped_part = terminal_src.split("#[cfg(test)]\nmod tests").next().unwrap_or(terminal_src);
        assert!(!shipped_part.contains("fn is_design_creation_prompt"), "is_design_creation_prompt must not exist in terminal.rs");
        kill_tabs(&mut st);
    }

    #[test]
    fn missing_cli_shows_the_start_failure_surface_and_no_executed_text() {
        assert!(!runtime_binary_available_in("claude", Some(std::ffi::OsStr::new("")), false), "empty PATH must report claude as unavailable");
        let mut st = TerminalState::for_tab(68103);
        st.active_runtime = AgentCliPreset::Claude;
        st.path_override = Some(std::ffi::OsString::new());
        let _ = st.spawn_tab(AgentCliPreset::Claude, None);
        let deadline = std::time::Instant::now() + std::time::Duration::from_millis(2000);
        while std::time::Instant::now() < deadline && st.active_tab_mut().is_some_and(|t| t.is_alive()) {
            std::thread::sleep(std::time::Duration::from_millis(15));
        }
        let _ = st.poll_live_agent_bridge();

        let failure = st.active_tab().and_then(|t| t.start_failure.as_ref()).expect("missing CLI must populate start_failure on the active tab");
        assert_eq!(failure.preset, AgentCliPreset::Claude);
        assert_eq!(failure.cause, agent_start_failure::StartFailureCause::NotInstalled);
        kill_tabs(&mut st);
    }

    #[test]
    fn no_planning_path_reaches_the_transcript() {
        let mut st = TerminalState::for_tab(68104);
        st.active_runtime = AgentCliPreset::Claude;
        spawn_live_claude_stub_tab(&mut st);
        st.submit_chat_prompt("@prd check req-01 and us-02 in fc-001", None);
        st.flush_pending_enter_now();

        let written = String::from_utf8_lossy(&st.active_tab().unwrap().written_bytes).into_owned();
        assert!(written.contains("@prd check req-01 and us-02 in fc-001"), "written={written:?}");
        for forbidden in ["plans/specs/", "PRD.md", "FC-001", "REQ-01..REQ-20", "US-01..US-17", "G1..G5"] {
            assert!(!written.contains(forbidden), "written bytes must not contain {forbidden:?}: {written:?}");
        }
        kill_tabs(&mut st);
    }

    #[test]
    fn make_design_sends_a_prompt_and_applies_nothing() {
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        crate::theme::apply(&ctx);
        let panel_rect = Rect::from_min_size(Pos2::new(48.0, 0.0), Vec2::new(260.0, 800.0));
        let mut st = TerminalState::for_tab(68105);
        st.side_panel_pty_open = false;
        st.chat_mode = true;
        st.expanded_agent_cat = Some(1);
        spawn_live_claude_stub_tab(&mut st);

        let _ = ctx.run_ui(egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1600.0, 900.0))), ..Default::default() }, |ui| {
            let _ = show_left(&mut st, ui, panel_rect, None);
        });
        assert!(ctx.data(|d| d.get_temp::<Rect>(Id::new(("test-rect", "agents-cat-prd".to_string())))).is_none(), "agents-cat-prd category must be removed");
        let make_design_rect: Rect = ctx.data(|d| d.get_temp(Id::new(("test-rect", "term-ai-make-design".to_string())))).expect("term-ai-make-design recorded");
        let pos = make_design_rect.center();

        let mut actions = Vec::new();
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
            actions = show_left(&mut st, ui, panel_rect, None);
        });

        let written = String::from_utf8_lossy(&st.active_tab().unwrap().written_bytes).into_owned();
        assert!(written.contains(MAKE_DESIGN_FIRST_DRAFT_PROMPT), "clicking term-ai-make-design must send MAKE_DESIGN_FIRST_DRAFT_PROMPT to PTY: {written:?}");
        assert!(
            !actions.iter().any(|a| matches!(a, TerminalAction::ApplySpecJson(_) | TerminalAction::ApplySpecFile(_))),
            "clicking term-ai-make-design must not return ApplySpecJson or ApplySpecFile: {actions:?}"
        );
        kill_tabs(&mut st);
    }

    #[test]
    fn composer_has_no_microphone_control() {
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        crate::theme::apply(&ctx);
        let panel_rect = Rect::from_min_size(Pos2::new(48.0, 0.0), Vec2::new(260.0, 800.0));
        let mut st = TerminalState::for_tab(68106);
        st.side_panel_pty_open = false;
        st.chat_mode = true;

        let out = ctx.run_ui(egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1600.0, 900.0))), ..Default::default() }, |ui| {
            let _ = show_left(&mut st, ui, panel_rect, None);
        });
        assert!(
            ctx.data(|d| d.get_temp::<Rect>(Id::new(("test-rect", "agents-composer-mic".to_string())))).is_none(),
            "agents-composer-mic must not be recorded"
        );
        let (texts, _) = crate::lint::scan_shapes(&out.shapes);
        for t in &texts {
            assert!(!t.text.contains("Audit the selected frame against the PRD"), "painted text must not contain fake voice prompt: {:?}", t.text);
        }
        kill_tabs(&mut st);
    }

    fn make_three_layer_doc() -> (Document, NodeId, [NodeId; 3]) {
        let mut doc = Document::new("Chip Test Doc");
        let page = doc.add_page("Page 1");
        let mut ids = [NodeId(0); 3];
        for (idx, name) in ["A", "B", "C"].into_iter().enumerate() {
            let id = doc.alloc_id();
            let mut f = Node::frame(id, designos_doc::Rect::new((idx as f32) * 100.0, 0.0, 80.0, 60.0));
            f.name = name.to_string();
            doc.insert(page, f).unwrap();
            ids[idx] = id;
        }
        (doc, page, ids)
    }

    #[test]
    fn chip_updates_in_the_same_frame() {
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        crate::theme::apply(&ctx);
        let (doc, page, ids) = make_three_layer_doc();
        let mut st = TerminalState::for_tab(65401);
        let panel_rect = Rect::from_min_size(Pos2::new(48.0, 0.0), Vec2::new(280.0, 600.0));

        let run_one_frame = |st: &mut TerminalState, sel: &[NodeId]| -> Option<String> {
            let text_id = Id::new(("test-text", "agent-selection-chip-left".to_string()));
            ctx.data_mut(|d| d.remove_temp::<String>(text_id));
            let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1440.0, 900.0))), ..Default::default() };
            let _ = ctx.run_ui(raw, |ui| {
                crate::agent_selection_chip::with_selection_summary(&doc, page, sel, |summary| {
                    let _ = show_left_with_selection(st, ui, panel_rect, None, summary);
                });
            });
            ctx.data(|d| d.get_temp::<String>(text_id))
        };

        assert_eq!(run_one_frame(&mut st, &[]), None, "nothing selected: no chip text is painted");
        assert_eq!(run_one_frame(&mut st, &[ids[0]]).as_deref(), Some("Page 1 · 1 selected · A"));
        assert_eq!(run_one_frame(&mut st, &[ids[0], ids[1], ids[2]]).as_deref(), Some("Page 1 · 3 selected · A, B, +1"));
        assert_eq!(run_one_frame(&mut st, &[]), None, "clearing the selection hides the chip again in the same frame");

        kill_tabs(&mut st);
        let _ = std::fs::remove_dir_all(&st.session_dir);
    }

    #[test]
    fn selection_change_writes_nothing_to_the_pty() {
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        crate::theme::apply(&ctx);
        let (doc, page, ids) = make_three_layer_doc();
        let mut st = TerminalState::for_tab(65402);
        st.spawn_tab(AgentCliPreset::Shell, None).expect("spawn stub tab");
        st.active_tab_mut().unwrap().spawn_preset = AgentCliPreset::Claude;
        let rec = attach_recorder(st.active_tab_mut().unwrap());
        let panel_rect = Rect::from_min_size(Pos2::new(48.0, 0.0), Vec2::new(280.0, 600.0));

        let selections: [&[NodeId]; 5] = [&[], &[ids[0]], &[ids[0], ids[1]], &[ids[0], ids[1], ids[2]], &[]];
        for sel in selections {
            st.sync_live_file_context(&doc, page, sel, None);
            let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1440.0, 900.0))), ..Default::default() };
            let _ = ctx.run_ui(raw, |ui| {
                crate::agent_selection_chip::with_selection_summary(&doc, page, sel, |summary| {
                    let actions = show_left_with_selection(&mut st, ui, panel_rect, None, summary);
                    assert!(actions.is_empty(), "merely changing selection must emit no send action: {actions:?}");
                });
            });
        }

        assert!(rec.lock().unwrap().is_empty(), "selection changes must write zero bytes to the PTY");
        kill_tabs(&mut st);
        let _ = std::fs::remove_dir_all(&st.session_dir);
    }

    #[test]
    fn click_pastes_one_sentence_without_enter() {
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        crate::theme::apply(&ctx);
        let (doc, page, ids) = make_three_layer_doc();
        let sel = [ids[0], ids[1]];
        let mut st = TerminalState::for_tab(65403);
        st.spawn_tab(AgentCliPreset::Shell, None).expect("spawn stub tab");
        st.active_tab_mut().unwrap().spawn_preset = AgentCliPreset::Claude;
        let rec = attach_recorder(st.active_tab_mut().unwrap());
        let panel_rect = Rect::from_min_size(Pos2::new(48.0, 0.0), Vec2::new(280.0, 600.0));

        // Frame 1: paint to record chip rect.
        let raw0 = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1440.0, 900.0))), ..Default::default() };
        let _ = ctx.run_ui(raw0, |ui| {
            crate::agent_selection_chip::with_selection_summary(&doc, page, &sel, |summary| {
                let _ = show_left_with_selection(&mut st, ui, panel_rect, None, summary);
            });
        });
        let chip_rect: Rect = ctx.data(|d| d.get_temp(Id::new(("test-rect", "agent-selection-chip-left".to_string())))).expect("chip rect recorded");
        let click_pos = chip_rect.center();

        // Frame 2: click the chip.
        let mut emitted = Vec::new();
        let raw_click = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1440.0, 900.0))),
            events: vec![
                egui::Event::PointerMoved(click_pos),
                egui::Event::PointerButton { pos: click_pos, button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE },
                egui::Event::PointerButton { pos: click_pos, button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE },
            ],
            ..Default::default()
        };
        let _ = ctx.run_ui(raw_click, |ui| {
            crate::agent_selection_chip::with_selection_summary(&doc, page, &sel, |summary| {
                emitted = show_left_with_selection(&mut st, ui, panel_rect, None, summary);
            });
        });
        assert_eq!(emitted, vec![TerminalAction::SendSelectionToAgent]);

        send_selection_to_agent_pty(&mut st, &doc, page, &sel).expect("send selection");
        let bytes = rec.lock().unwrap().clone();
        assert!(bytes.starts_with(b"\x1b[200~") && bytes.ends_with(b"\x1b[201~"), "expected bracketed paste: {bytes:?}");
        assert!(!bytes.contains(&b'\r') && !bytes.contains(&b'\n'), "click must not press Enter: {bytes:?}");
        let inner = String::from_utf8_lossy(&bytes[6..bytes.len() - 6]);
        assert!(inner.contains(&st.selection_spec_path.display().to_string()), "paste must reference selection.spec.json: {inner}");
        assert!(inner.contains("designos_export_spec"), "paste must mention designos_export_spec: {inner}");

        kill_tabs(&mut st);
        let _ = std::fs::remove_dir_all(&st.session_dir);
    }

    #[test]
    fn header_strip_shows_chip_and_turn_status_side_by_side() {
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        crate::theme::apply(&ctx);
        let (doc, page, ids) = make_three_layer_doc();
        let sel = [ids[0], ids[1]];
        let mut st = isolated_683_state(68302);
        let _ = spawn_mock_agent_tab(&mut st, AgentCliPreset::Claude, Some("sid-strip-1"));
        let events_path = st.session_dir.join("agent-events.jsonl");
        {
            use std::io::Write;
            let mut f = std::fs::OpenOptions::new().create(true).append(true).open(&events_path).unwrap();
            writeln!(f, r#"{{"t":100.0,"e":"UserPromptSubmit","p":{{"session_id":"sid-strip-1"}}}}"#).unwrap();
            writeln!(f, r#"{{"t":101.0,"e":"PreToolUse","p":{{"session_id":"sid-strip-1","tool_name":"mcp__designos__designos_create_node"}}}}"#).unwrap();
        }
        let panel_rect = Rect::from_min_size(Pos2::new(48.0, 0.0), Vec2::new(280.0, 600.0));
        let frame = |st: &mut TerminalState, events: Vec<egui::Event>| -> Vec<TerminalAction> {
            let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1440.0, 900.0))), events, ..Default::default() };
            let mut out = Vec::new();
            let _ = ctx.run_ui(raw, |ui| {
                crate::agent_selection_chip::with_selection_summary(&doc, page, &sel, |summary| {
                    out = show_left_with_selection(st, ui, panel_rect, None, summary);
                });
            });
            out
        };
        let rect_of = |name: &str| -> Option<Rect> { ctx.data(|d| d.get_temp(Id::new(("test-rect", name.to_string())))) };

        let _ = frame(&mut st, vec![]);
        assert!(matches!(st.tabs[0].effective_status(), AgentTabStatus::Working(_)), "tab must be Working");
        let _ = frame(&mut st, vec![]);
        let chip = rect_of("agent-selection-chip-left").expect("selection chip stays visible while the turn runs");
        let status = rect_of("agents-header-turn-status").expect("turn status is visible");
        assert!(!chip.intersects(status), "chip {chip:?} and status {status:?} must not overlap");
        assert!(chip.right() < status.left(), "chip on the left, status right-aligned");
        assert!(status.right() <= panel_rect.right(), "status stays inside the panel");
        assert!(chip.height() >= theme::TARGET_MIN - 0.01 && status.height() >= theme::TARGET_MIN - 0.01, "targets keep the minimum height");

        let pos = chip.center();
        let click = vec![
            egui::Event::PointerMoved(pos),
            egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE },
            egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE },
        ];
        let emitted = frame(&mut st, click);
        assert_eq!(emitted, vec![TerminalAction::SendSelectionToAgent], "chip is still clickable while Working");

        kill_tabs(&mut st);
        let _ = std::fs::remove_dir_all(&st.session_dir);
    }

    #[test]
    fn file_covers_the_whole_selection() {
        let (doc, page, ids) = make_three_layer_doc();
        let mut st = TerminalState::for_tab(65404);
        let path = send_selection_to_agent_pty(&mut st, &doc, page, &ids).expect("send selection").expect("selection path written");
        let val = read_json(&path);
        assert_eq!(val["page"]["id"].as_u64(), Some(page.0));
        assert_eq!(val["page"]["name"].as_str(), Some("Page 1"));
        assert_eq!(val["count"].as_u64(), Some(3));
        let nodes = val["nodes"].as_array().expect("nodes array");
        assert_eq!(nodes.len(), 3);
        for (i, expected_name) in ["A", "B", "C"].into_iter().enumerate() {
            assert_eq!(nodes[i]["node_id"].as_u64(), Some(ids[i].0));
            assert_eq!(nodes[i]["name"].as_str(), Some(expected_name));
            assert_eq!(nodes[i]["kind"].as_str(), Some("frame"));
            assert!(nodes[i].get("spec").is_some());
        }
        let _ = std::fs::remove_dir_all(&st.session_dir);
    }

    #[test]
    fn empty_selection_hides_the_chip() {
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        crate::theme::apply(&ctx);
        let (doc, page, ids) = make_three_layer_doc();
        let mut st = TerminalState::for_tab(65405);
        st.spawn_tab(AgentCliPreset::Shell, None).expect("spawn stub tab");
        st.active_tab_mut().unwrap().spawn_preset = AgentCliPreset::Claude;
        let rec = attach_recorder(st.active_tab_mut().unwrap());
        let panel_rect = Rect::from_min_size(Pos2::new(48.0, 0.0), Vec2::new(280.0, 600.0));
        let dock_rect = Rect::from_min_size(Pos2::new(0.0, 500.0), Vec2::new(1200.0, 300.0));
        let _ = std::fs::remove_file(&st.selection_spec_path);
        let recorded = |ctx: &egui::Context, name: &str| -> (Option<Rect>, Option<String>) {
            ctx.data(|d| (d.get_temp(Id::new(("test-rect", name.to_string()))), d.get_temp(Id::new(("test-text", name.to_string())))))
        };

        let raw0 = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1440.0, 900.0))), ..Default::default() };
        let mut emitted = Vec::new();
        let _ = ctx.run_ui(raw0.clone(), |ui| {
            crate::agent_selection_chip::with_selection_summary(&doc, page, &[], |summary| {
                emitted.extend(show_left_with_selection(&mut st, ui, panel_rect, None, summary));
                emitted.extend(show_dock_with_selection(&mut st, ui, dock_rect, None, summary));
            });
        });
        assert_eq!(recorded(&ctx, "agent-selection-chip-left"), (None, None), "no chip in the panel header strip when nothing is selected");
        assert_eq!(recorded(&ctx, "agent-selection-chip-dock"), (None, None), "no chip in the dock bar when nothing is selected");
        assert_eq!(recorded(&ctx, "agent-selection-chip"), (None, None));

        // Where the chip would have sat, a click raises nothing and writes nothing.
        let click_pos = Pos2::new(panel_rect.left() + 40.0, 80.0);
        let raw_click = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1440.0, 900.0))),
            events: vec![
                egui::Event::PointerMoved(click_pos),
                egui::Event::PointerButton { pos: click_pos, button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE },
                egui::Event::PointerButton { pos: click_pos, button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE },
            ],
            ..Default::default()
        };
        let _ = ctx.run_ui(raw_click, |ui| {
            crate::agent_selection_chip::with_selection_summary(&doc, page, &[], |summary| {
                emitted.extend(show_left_with_selection(&mut st, ui, panel_rect, None, summary));
            });
        });
        assert!(!emitted.contains(&TerminalAction::SendSelectionToAgent), "no chip, no send action: {emitted:?}");
        assert!(rec.lock().unwrap().is_empty(), "an empty selection writes nothing to the PTY");
        assert!(!st.selection_spec_path.exists(), "an empty selection writes no selection.spec.json");

        // A selection brings the chip back in both places.
        let sel = [ids[0]];
        let _ = ctx.run_ui(raw0, |ui| {
            crate::agent_selection_chip::with_selection_summary(&doc, page, &sel, |summary| {
                let _ = show_left_with_selection(&mut st, ui, panel_rect, None, summary);
                let _ = show_dock_with_selection(&mut st, ui, dock_rect, None, summary);
            });
        });
        let (left_rect, left_text) = recorded(&ctx, "agent-selection-chip-left");
        let (dock_rect_chip, dock_text) = recorded(&ctx, "agent-selection-chip-dock");
        assert!(left_rect.is_some() && dock_rect_chip.is_some(), "a selected layer shows the chip in the strip and in the dock");
        assert_eq!(left_text.as_deref(), Some("Page 1 · 1 selected · A"));
        assert_eq!(dock_text.as_deref(), Some("Page 1 · 1 selected · A"));

        kill_tabs(&mut st);
        let _ = std::fs::remove_dir_all(&st.session_dir);
    }

    #[test]
    fn no_agent_tab_sends_nothing() {
        let (doc, page, ids) = make_three_layer_doc();
        let mut st = TerminalState::for_tab(65406);
        // 1. No tabs open at all:
        send_selection_to_agent_pty(&mut st, &doc, page, &[ids[0]]).expect("send with no tab");
        assert_eq!(st.status_msg.as_deref(), Some("No agent tab open - nothing sent"));

        // 2. Only a Shell tab open:
        st.spawn_tab(AgentCliPreset::Shell, None).expect("spawn shell");
        let rec = attach_recorder(st.active_tab_mut().unwrap());
        st.status_msg = None;
        send_selection_to_agent_pty(&mut st, &doc, page, &[ids[0]]).expect("send with shell tab");
        assert_eq!(st.status_msg.as_deref(), Some("No agent tab open - nothing sent"));
        assert!(rec.lock().unwrap().is_empty(), "shell tab must receive zero bytes");

        kill_tabs(&mut st);
        let _ = std::fs::remove_dir_all(&st.session_dir);
    }

    #[test]
    fn context_reply_still_excludes_the_selection() {
        prompt_hooks_print_only_what_each_cli_can_read_and_never_the_selection();
    }

    /// Every option `gemini --help` lists in Gemini CLI 0.38.2 (`plans/reports/release-060-evidence/cli-help-261019/gemini-help.txt`).
    const GEMINI_0_38_2_FLAGS: &[&str] = &[
        "-d",
        "--debug",
        "-m",
        "--model",
        "-p",
        "--prompt",
        "-i",
        "--prompt-interactive",
        "-s",
        "--sandbox",
        "-y",
        "--yolo",
        "--approval-mode",
        "--experimental-acp",
        "--allowed-mcp-server-names",
        "--allowed-tools",
        "-e",
        "--extensions",
        "-l",
        "--list-extensions",
        "-r",
        "--resume",
        "--list-sessions",
        "--delete-session",
        "--include-directories",
        "-o",
        "--output-format",
        "--raw-output",
        "--accept-raw-output-risk",
        "--screen-reader",
        "--worktree",
        "-v",
        "--version",
        "-h",
        "--help",
    ];

    /// Every option `gemini mcp add --help` lists in Gemini CLI 0.38.2 (`plans/reports/release-060-evidence/cli-help-261019/gemini-mcp-add-help.txt`).
    const GEMINI_MCP_ADD_0_38_2_FLAGS: &[&str] = &[
        "-d",
        "--debug",
        "-s",
        "--scope",
        "-t",
        "--transport",
        "--type",
        "-e",
        "--env",
        "-H",
        "--header",
        "--timeout",
        "--trust",
        "--description",
        "--include-tools",
        "--exclude-tools",
        "-h",
        "--help",
    ];

    /// Every option `opencode --help` lists in OpenCode 1.14.19 (`plans/reports/release-060-evidence/cli-help-261019/opencode-help.txt`).
    const OPENCODE_1_14_19_FLAGS: &[&str] = &[
        "-h",
        "--help",
        "-v",
        "--version",
        "--print-logs",
        "--log-level",
        "--port",
        "--hostname",
        "--mdns",
        "--mdns-domain",
        "--cors",
        "-m",
        "--model",
        "-c",
        "--continue",
        "-s",
        "--session",
        "--fork",
        "-p",
        "--prompt",
        "--agent",
    ];

    #[test]
    fn gemini_launch_uses_only_flags_its_help_lists() {
        for mode in [PermissionMode::Skip, PermissionMode::Ask] {
            let interactive = AgentCliPreset::Gemini.cli_launch_command_for_mode(mode);
            assert_eq!(unknown_flags(interactive, GEMINI_0_38_2_FLAGS), Vec::<String>::new(), "gemini tab launch ({mode:?}): {interactive}");
            assert!(!interactive.contains("--mcp-config"), "gemini has no --mcp-config: {interactive}");
            assert!(!interactive.contains("--dangerously-skip-permissions"), "gemini uses --approval-mode: {interactive}");
            assert!(interactive.contains("--include-directories \"$DESIGNOS_SESSION_DIR\""), "{interactive}");
        }

        let new_session = crate::agent_sessions::build_new_session_command(AgentCliPreset::Gemini, None);
        assert_eq!(unknown_flags(&new_session, GEMINI_0_38_2_FLAGS), Vec::<String>::new(), "gemini new session launch: {new_session}");

        let resume_entry = crate::agent_sessions::SessionEntry {
            tab_id: 1,
            title: "Gemini CLI #1".to_string(),
            cli: "gemini".to_string(),
            session_id: None,
            created_at: 100,
            last_active_at: 200,
            status: crate::agent_sessions::SessionEntryStatus::Exited,
            extra: std::collections::BTreeMap::new(),
        };
        let resume = crate::agent_sessions::build_resume_command(&resume_entry, false).expect("gemini resume");
        assert_eq!(unknown_flags(&resume, GEMINI_0_38_2_FLAGS), Vec::<String>::new(), "gemini resume launch: {resume}");

        let mcp_cfg = serde_json::json!({
            "mcpServers": {
                "designos": {
                    "command": "designos-mcp",
                    "args": ["--workspace", "/tmp/work"],
                    "env": { "DESIGNOS_SESSION_DIR": "/tmp/sess" }
                }
            }
        });
        let mcp_add_argv = build_gemini_mcp_add_argv(&mcp_cfg);
        // Only flags before the positional `<name> <commandOrUrl> [args...]` belong to `gemini mcp add`
        let name_pos = mcp_add_argv.iter().position(|t| t == "designos").expect("server name positional");
        let mcp_add_flags: Vec<String> =
            mcp_add_argv[..name_pos].iter().filter(|t| t.starts_with('-')).filter(|t| !GEMINI_MCP_ADD_0_38_2_FLAGS.contains(&t.as_str())).cloned().collect();
        assert_eq!(mcp_add_flags, Vec::<String>::new(), "gemini mcp add flags: {mcp_add_argv:?}");
        assert!(mcp_add_argv.windows(2).any(|w| w == ["--scope", "project"]), "must use project scope: {mcp_add_argv:?}");
    }

    #[test]
    fn opencode_launch_uses_only_flags_its_help_lists() {
        for mode in [PermissionMode::Skip, PermissionMode::Ask] {
            let interactive = AgentCliPreset::OpenCode.cli_launch_command_for_mode(mode);
            assert_eq!(unknown_flags(interactive, OPENCODE_1_14_19_FLAGS), Vec::<String>::new(), "opencode tab launch ({mode:?}): {interactive}");
            assert!(!interactive.contains("--mcp-config"), "opencode has no --mcp-config: {interactive}");
        }

        let new_session = crate::agent_sessions::build_new_session_command(AgentCliPreset::OpenCode, None);
        assert_eq!(unknown_flags(&new_session, OPENCODE_1_14_19_FLAGS), Vec::<String>::new(), "opencode new session launch: {new_session}");

        for sid in [None, Some("ses_abc123".to_string())] {
            let resume_entry = crate::agent_sessions::SessionEntry {
                tab_id: 1,
                title: "OpenCode #1".to_string(),
                cli: "opencode".to_string(),
                session_id: sid,
                created_at: 100,
                last_active_at: 200,
                status: crate::agent_sessions::SessionEntryStatus::Exited,
                extra: std::collections::BTreeMap::new(),
            };
            let resume = crate::agent_sessions::build_resume_command(&resume_entry, false).expect("opencode resume");
            assert_eq!(unknown_flags(&resume, OPENCODE_1_14_19_FLAGS), Vec::<String>::new(), "opencode resume launch: {resume}");
        }
    }

    #[test]
    fn gemini_project_settings_written_in_the_session_folder_only() {
        let home_settings = std::env::var("HOME").ok().filter(|h| !h.is_empty()).map(|h| PathBuf::from(h).join(".gemini").join("settings.json"));
        let home_before = home_settings.as_ref().and_then(|p| std::fs::read(p).ok());

        let mut st = TerminalState::for_tab(779);
        let mut doc = Document::new("Gemini Session Settings File");
        let page = doc.add_page("Page 1");
        st.sync_live_file_context(&doc, page, &[], Some(std::path::Path::new("/tmp/gemini-session-file.designos")));

        let session_settings_path = st.session_dir.join(".gemini").join("settings.json");
        let session_settings = std::fs::read_to_string(&session_settings_path).expect("<session_dir>/.gemini/settings.json");
        let parsed: serde_json::Value = serde_json::from_str(&session_settings).expect("parse .gemini/settings.json");
        assert!(parsed["mcpServers"]["designos"]["command"].as_str().is_some());
        assert_eq!(parsed["mcpServers"]["designos"]["trust"].as_bool(), Some(true));
        assert!(parsed.get("designos").is_none(), "Gemini settings must not carry the designos.enabled_tools block");
        assert_eq!(parsed["mcpServers"]["designos"]["env"]["DESIGNOS_FILE"].as_str(), Some("/tmp/gemini-session-file.designos"));
        assert!(st.session_dir.join(".gemini").join("trustedFolders.json").is_file(), "trustedFolders.json must be written in session_dir/.gemini/");
        assert!(st.session_dir.join("GEMINI.md").is_file(), "GEMINI.md must be written in session_dir");

        let home_after = home_settings.as_ref().and_then(|p| std::fs::read(p).ok());
        assert_eq!(home_before, home_after, "$HOME/.gemini/settings.json must never be modified by Studio");
    }

    #[test]
    fn gemini_live_check() {
        let mut st = TerminalState::for_tab(780);
        let mut doc = Document::new("Gemini Live Check File");
        let page = doc.add_page("Page 1");
        st.sync_live_file_context(&doc, page, &[], Some(std::path::Path::new("/tmp/gemini-live-check.designos")));

        let settings_path = st.session_dir.join(".gemini").join("settings.json");
        let trusted_path = st.session_dir.join(".gemini").join("trustedFolders.json");
        assert!(settings_path.is_file());
        assert!(trusted_path.is_file());

        // Verify the Gemini PTY session resolves cwd to `session_dir` and configures `GEMINI_CLI_TRUSTED_FOLDERS_PATH`.
        assert_eq!(resolve_pty_cwd(AgentCliPreset::Gemini, Some(&st.session_dir), PathBuf::from("/tmp")), st.session_dir);
        let mut pty_cmd = CommandBuilder::new("gemini");
        configure_pty_env_with_session(&mut pty_cmd, AgentCliPreset::Gemini, None, Some(&st.session_env()));
        assert_eq!(pty_cmd.get_env("GEMINI_CLI_TRUSTED_FOLDERS_PATH"), Some(trusted_path.as_os_str()));

        // Verify `build_gemini_mcp_add_argv` uses `--` before `--workspace` and when `gemini` is installed,
        // `gemini mcp list` discovers the project-scoped `designos` server from `session_dir`.
        let shared_mcp: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(st.session_dir.join("mcp-config.json")).unwrap()).unwrap();
        let argv = build_gemini_mcp_add_argv(&shared_mcp);
        assert!(argv.windows(2).any(|w| w == ["--", "--workspace"]), "must separate --workspace with --: {argv:?}");

        if runtime_binary_available("gemini") {
            let mut list_cmd = std::process::Command::new("gemini");
            list_cmd.args(["mcp", "list"]).current_dir(&st.session_dir).env("GEMINI_CLI_TRUSTED_FOLDERS_PATH", &trusted_path);
            if let Ok(out) = list_cmd.output() {
                let combined = format!("{}\n{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
                assert!(combined.contains("designos:"), "gemini mcp list must discover the session-scoped designos server: {combined}");
            }
        }

        // Verify an edit written to the session's `doc-patch.jsonl` (as `designos-mcp` does) is polled by the session bridge.
        let rect_id = doc.alloc_id();
        let mut rect = designos_doc::Node::new(rect_id, designos_doc::NodeKind::Rectangle);
        rect.name = "Gemini Card".into();
        let patch = designos_doc::live_patch::LivePatch::new(vec![designos_doc::live_patch::LiveOp::Insert {
            parent: page,
            at: designos_doc::live_patch::SiblingHint::default(),
            nodes: vec![rect],
        }]);
        let patch_line = format!("{}\n", serde_json::to_string(&patch).expect("serialize patch"));
        std::fs::write(&st.live_patch.path, patch_line).expect("write patch to doc-patch.jsonl");
        let actions = st.poll_live_agent_bridge();
        assert!(actions.iter().any(|a| matches!(a, TerminalAction::ApplyLiveLog(_))), "edit in doc-patch.jsonl must reach Studio live bridge");
    }

    #[test]
    fn permission_flags_per_mode() {
        assert_eq!(AgentCliPreset::Claude.permission_flags(PermissionMode::Skip), &["--dangerously-skip-permissions"]);
        assert_eq!(AgentCliPreset::Claude.permission_flags(PermissionMode::Ask), &[] as &[&str]);

        assert_eq!(AgentCliPreset::Codex.permission_flags(PermissionMode::Skip), &["--dangerously-bypass-approvals-and-sandbox"]);
        assert_eq!(AgentCliPreset::Codex.permission_flags(PermissionMode::Ask), &[] as &[&str]);

        assert_eq!(AgentCliPreset::Grok.permission_flags(PermissionMode::Skip), &["--always-approve"]);
        assert_eq!(AgentCliPreset::Grok.permission_flags(PermissionMode::Ask), &[] as &[&str]);

        assert_eq!(AgentCliPreset::Antigravity.permission_flags(PermissionMode::Skip), &["--dangerously-skip-permissions"]);
        assert_eq!(AgentCliPreset::Antigravity.permission_flags(PermissionMode::Ask), &[] as &[&str]);

        assert_eq!(AgentCliPreset::Gemini.permission_flags(PermissionMode::Skip), &["--approval-mode", "yolo"]);
        assert_eq!(AgentCliPreset::Gemini.permission_flags(PermissionMode::Ask), &["--approval-mode", "default"]);
        assert!(AgentCliPreset::Gemini.cli_launch_command_for_mode(PermissionMode::Skip).contains("--approval-mode yolo"));
        assert!(AgentCliPreset::Gemini.cli_launch_command_for_mode(PermissionMode::Ask).contains("--approval-mode default"));

        // OpenCode has no permission bypass flag in `opencode --help`, so both modes omit the flag and pass `$DESIGNOS_SESSION_DIR`
        assert_eq!(AgentCliPreset::OpenCode.permission_flags(PermissionMode::Skip), &[] as &[&str]);
        assert_eq!(AgentCliPreset::OpenCode.permission_flags(PermissionMode::Ask), &[] as &[&str]);
        assert_eq!(AgentCliPreset::OpenCode.cli_launch_command_for_mode(PermissionMode::Skip), "opencode \"$DESIGNOS_SESSION_DIR\"");
        assert_eq!(AgentCliPreset::OpenCode.cli_launch_command_for_mode(PermissionMode::Ask), "opencode \"$DESIGNOS_SESSION_DIR\"");
    }

    fn isolated_683_state(tab_id: u64) -> TerminalState {
        let dir = std::env::temp_dir().join("designos-sessions").join(tab_session_id(&format!("p{}", std::process::id()), tab_id));
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::create_dir_all(&dir);
        TerminalState::for_tab(tab_id)
    }

    fn spawn_mock_agent_tab(st: &mut TerminalState, preset: AgentCliPreset, session_uuid: Option<&str>) -> usize {
        let _ = std::fs::create_dir_all(&st.session_dir);
        st.spawn_tab(AgentCliPreset::Shell, None).expect("spawn shell backing tab");
        let idx = st.tabs.len() - 1;
        let tab = &mut st.tabs[idx];
        tab.spawn_preset = preset;
        tab.title = preset.label().to_string();
        tab.session_uuid = session_uuid.map(str::to_string);
        idx
    }

    #[test]
    fn hook_command_appends_one_wrapped_record() {
        let tmp = std::env::temp_dir().join(format!("designos-683-hook-cmd-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        let status_path = tmp.join("agent-status.json");
        let events_path = tmp.join("agent-events.jsonl");
        let cmd_str = status_hook_command(&status_path, "PreToolUse", "working");

        let mut child = std::process::Command::new("sh").arg("-c").arg(&cmd_str).stdin(std::process::Stdio::piped()).spawn().expect("spawn hook command");
        {
            use std::io::Write;
            let stdin = child.stdin.as_mut().unwrap();
            stdin.write_all(b"{\"session_id\":\"s-hook-1\",\"tool_name\":\"Read\"}").unwrap();
        }
        let status = child.wait().expect("wait hook command");
        assert!(status.success());

        let raw = std::fs::read_to_string(&events_path).expect("read agent-events.jsonl");
        let (records, consumed) = crate::agent_turn_status::parse_streaming_records(&raw);
        assert_eq!(consumed, raw.len());
        assert_eq!(records.len(), 1);
        assert!(records[0].t > 1_700_000_000.0);
        assert_eq!(records[0].e, "PreToolUse");
        assert_eq!(records[0].session_id(), Some("s-hook-1"));
        assert_eq!(records[0].tool_name(), Some("Read"));
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn stub_cli_drives_running_needs_input_done() {
        let mut st = isolated_683_state(68301);
        let _ = spawn_mock_agent_tab(&mut st, AgentCliPreset::Claude, Some("sid-drive-1"));
        let events_path = st.session_dir.join("agent-events.jsonl");

        let write_rec = |line: &str| {
            use std::io::Write;
            let mut f = std::fs::OpenOptions::new().create(true).append(true).open(&events_path).unwrap();
            writeln!(f, "{line}").unwrap();
        };

        write_rec(r#"{"t":100.0,"e":"UserPromptSubmit","p":{"session_id":"sid-drive-1"}}"#);
        write_rec(r#"{"t":101.0,"e":"PreToolUse","p":{"session_id":"sid-drive-1","tool_name":"mcp__designos__designos_create_node"}}"#);
        let _ = st.poll_live_agent_bridge();
        assert_eq!(st.tabs[0].effective_status(), AgentTabStatus::Working(Some("Editing layers…".to_string())));

        write_rec(r#"{"t":102.0,"e":"PermissionRequest","p":{"session_id":"sid-drive-1","tool_name":"Bash"}}"#);
        let _ = st.poll_live_agent_bridge();
        assert_eq!(st.tabs[0].effective_status(), AgentTabStatus::WaitingForInput(Some("Bash".to_string())));

        write_rec(r#"{"t":103.0,"e":"PostToolUse","p":{"session_id":"sid-drive-1","tool_name":"Bash"}}"#);
        let _ = st.poll_live_agent_bridge();
        assert_eq!(st.tabs[0].effective_status(), AgentTabStatus::Working(Some("Working…".to_string())));

        write_rec(r#"{"t":106.0,"e":"Stop","p":{"session_id":"sid-drive-1"}}"#);
        let _ = st.poll_live_agent_bridge();
        assert_eq!(st.tabs[0].effective_status(), AgentTabStatus::Done);
        kill_tabs(&mut st);
    }

    #[test]
    fn records_route_by_session_id_not_by_active_tab() {
        let mut st = isolated_683_state(68302);
        let active_idx = spawn_mock_agent_tab(&mut st, AgentCliPreset::Claude, Some("sid-active"));
        let bg_idx = spawn_mock_agent_tab(&mut st, AgentCliPreset::Claude, Some("sid-bg"));
        st.active_tab = active_idx;

        let events_path = st.session_dir.join("agent-events.jsonl");
        std::fs::write(
            &events_path,
            concat!(
                "{\"t\":200.0,\"e\":\"UserPromptSubmit\",\"p\":{\"session_id\":\"sid-bg\"}}\n",
                "{\"t\":201.0,\"e\":\"PreToolUse\",\"p\":{\"session_id\":\"sid-bg\",\"tool_name\":\"designos_get_document\"}}\n",
            ),
        )
        .unwrap();

        let _ = st.poll_live_agent_bridge();
        assert_eq!(st.tabs[active_idx].effective_status(), AgentTabStatus::Idle, "active tab must not receive background tab events");
        assert_eq!(
            st.tabs[bg_idx].effective_status(),
            AgentTabStatus::Working(Some("Reading the file…".to_string())),
            "background tab must receive its own session events"
        );
        kill_tabs(&mut st);
    }

    #[test]
    fn receipt_shows_exact_duration_and_counts() {
        let mut tracker = crate::agent_turn_status::TabTurnTracker::default();
        let submit = crate::agent_turn_status::AgentEventRecord { t: 100.0, e: "UserPromptSubmit".to_string(), p: serde_json::json!({"session_id": "s1"}) };
        let _ = tracker.on_event(&submit);
        for i in 0..12 {
            let pre = crate::agent_turn_status::AgentEventRecord {
                t: 101.0 + f64::from(i),
                e: "PreToolUse".to_string(),
                p: serde_json::json!({"session_id": "s1", "tool_name": "designos_update_node"}),
            };
            let _ = tracker.on_event(&pre);
        }
        tracker.record_touched_layers([1, 2, 3, 4, 5, 6, 7]);
        let stop = crate::agent_turn_status::AgentEventRecord { t: 180.0, e: "Stop".to_string(), p: serde_json::json!({"session_id": "s1"}) };
        let eff = tracker.on_event(&stop);
        let receipt = eff.receipt.expect("receipt emitted on Stop");
        assert_eq!(receipt.format_line(), "Worked for 1m 20s · 12 tool calls · 7 layers changed");
    }

    #[test]
    fn duration_rounds_up_with_minimum_one_second() {
        assert_eq!(crate::agent_turn_status::format_duration_secs(0.2), "1s");
        assert_eq!(crate::agent_turn_status::format_duration_secs(59.1), "1m 0s");
        assert_eq!(crate::agent_turn_status::format_duration_secs(60.0), "1m 0s");
        assert_eq!(crate::agent_turn_status::format_duration_secs(61.0), "1m 1s");
    }

    #[test]
    fn no_count_is_invented_without_data() {
        let mut tracker = crate::agent_turn_status::TabTurnTracker::default();
        let _ = tracker.on_event(&crate::agent_turn_status::AgentEventRecord { t: 10.0, e: "UserPromptSubmit".to_string(), p: serde_json::json!({}) });
        let _ =
            tracker.on_event(&crate::agent_turn_status::AgentEventRecord { t: 11.0, e: "PreToolUse".to_string(), p: serde_json::json!({"tool_name": "Read"}) });
        let eff = tracker.on_event(&crate::agent_turn_status::AgentEventRecord { t: 15.0, e: "Stop".to_string(), p: serde_json::json!({}) });
        let receipt = eff.receipt.expect("receipt emitted");
        assert_eq!(receipt.layers_changed, None);
        let line = receipt.format_line();
        assert_eq!(line, "Worked for 5s · 1 tool call");
        assert!(!line.contains("layer"), "must not invent a layers count: {line}");
    }

    #[test]
    fn one_notification_per_turn_when_unfocused() {
        let mut st = isolated_683_state(68303);
        let sink = std::sync::Arc::new(crate::agent_notify::StubNotificationSink::new());
        st.notifier = crate::agent_notify::AgentNotifier::with_sink(sink.clone());
        let idx = spawn_mock_agent_tab(&mut st, AgentCliPreset::Claude, Some("sid-notify-1"));
        st.active_tab = idx;
        st.window_focused = false;
        st.surface_visible = true;

        let events_path = st.session_dir.join("agent-events.jsonl");
        std::fs::write(
            &events_path,
            concat!(
                "{\"t\":10.0,\"e\":\"UserPromptSubmit\",\"p\":{\"session_id\":\"sid-notify-1\"}}\n",
                "{\"t\":14.0,\"e\":\"Stop\",\"p\":{\"session_id\":\"sid-notify-1\"}}\n",
                "{\"t\":15.0,\"e\":\"Stop\",\"p\":{\"session_id\":\"sid-notify-1\"}}\n",
            ),
        )
        .unwrap();
        let _ = st.poll_live_agent_bridge();
        assert_eq!(sink.count(), 1, "unfocused turn must send 1 notification even with duplicate Stop");

        // Now focused and visible: 0 new notifications.
        st.window_focused = true;
        st.surface_visible = true;
        {
            use std::io::Write;
            let mut f = std::fs::OpenOptions::new().append(true).open(&events_path).unwrap();
            writeln!(f, "{{\"t\":20.0,\"e\":\"UserPromptSubmit\",\"p\":{{\"session_id\":\"sid-notify-1\"}}}}").unwrap();
            writeln!(f, "{{\"t\":25.0,\"e\":\"Stop\",\"p\":{{\"session_id\":\"sid-notify-1\"}}}}").unwrap();
        }
        let _ = st.poll_live_agent_bridge();
        assert_eq!(sink.count(), 1, "focused and visible active tab must send 0 notifications");
        kill_tabs(&mut st);
    }

    #[test]
    fn notification_when_the_tab_is_hidden_but_window_focused() {
        let mut st = isolated_683_state(68304);
        let sink = std::sync::Arc::new(crate::agent_notify::StubNotificationSink::new());
        st.notifier = crate::agent_notify::AgentNotifier::with_sink(sink.clone());
        let active_idx = spawn_mock_agent_tab(&mut st, AgentCliPreset::Claude, Some("sid-fg"));
        let hidden_idx = spawn_mock_agent_tab(&mut st, AgentCliPreset::Grok, Some("sid-hidden"));
        st.active_tab = active_idx;
        st.window_focused = true;
        st.surface_visible = true;

        let events_path = st.session_dir.join("agent-events.jsonl");
        std::fs::write(
            &events_path,
            concat!(
                "{\"t\":50.0,\"e\":\"UserPromptSubmit\",\"p\":{\"sessionId\":\"sid-hidden\"}}\n",
                "{\"t\":55.0,\"e\":\"Stop\",\"p\":{\"sessionId\":\"sid-hidden\"}}\n",
            ),
        )
        .unwrap();
        let _ = st.poll_live_agent_bridge();
        assert_eq!(sink.count(), 1, "hidden tab finishing while window is focused must notify once");
        assert!(st.tabs[hidden_idx].unread, "hidden tab must be marked unread");
        kill_tabs(&mut st);
    }

    #[test]
    fn hook_files_carry_both_the_event_stream_and_the_turn_marks() {
        let status_path = std::path::Path::new("/tmp/designos-683-677-hooks/agent-status.json");
        let command_of = |v: &serde_json::Value, event: &str| v["hooks"][event][0]["hooks"][0]["command"].as_str().unwrap_or_default().to_string();
        let claude = session_hooks_settings(status_path, Some(std::path::Path::new("/tmp/designos-683-677-hooks/reply.json")));
        let grok = session_hooks_settings(status_path, None);
        for (cli, settings) in [("claude", &claude), ("grok", &grok)] {
            let start = command_of(settings, "UserPromptSubmit");
            assert!(start.contains("agent-events.jsonl") && start.contains("\"turn_mark\":\"start\"") && start.contains(cli), "{cli} prompt hook: {start}");
            for event in ["Stop", "StopFailure", "StopCancelled"] {
                let cmd = command_of(settings, event);
                assert!(cmd.contains("agent-events.jsonl") && cmd.contains(event), "{cli} {event} must append to the event stream: {cmd}");
                assert!(cmd.contains("\"turn_mark\":\"end\""), "{cli} {event} must write the end mark: {cmd}");
            }
            for event in ["PreToolUse", "PostToolUse", "PermissionRequest", "Notification"] {
                assert!(command_of(settings, event).contains("agent-events.jsonl"), "{cli} {event} must append to the event stream");
            }
        }
        assert!(
            command_of(&claude, "UserPromptSubmit").ends_with("cat '/tmp/designos-683-677-hooks/reply.json'"),
            "Claude still answers with the page context"
        );

        let agy = agy_session_hooks(status_path);
        let agy_cmd = |event: &str| agy["designos-status"][event][0]["command"].as_str().unwrap_or_default().to_string();
        assert!(agy_cmd("PreInvocation").contains("agent-events.jsonl") && agy_cmd("PreInvocation").contains("\"turn_mark\":\"start\""));
        assert!(agy_cmd("Stop").contains("agent-events.jsonl") && agy_cmd("Stop").contains("\"turn_mark\":\"end\""));
        assert!(agy_cmd("PreToolUse").contains("agent-events.jsonl") && !agy_cmd("PreToolUse").contains("turn_mark"));
    }

    /// Handler commands of one hook `event` in a generated Claude/Grok settings file (`{"hooks": {Event: [{hooks: [...]}]}}`).
    fn hook_handlers(settings: &serde_json::Value, event: &str) -> Vec<serde_json::Value> {
        let groups = settings["hooks"][event].as_array().unwrap_or_else(|| panic!("{event} hook group missing in {settings}"));
        assert_eq!(groups.len(), 1, "{event} must stay a single matcher group (no duplicate group): {settings}");
        groups[0]["hooks"].as_array().cloned().unwrap_or_default()
    }

    #[test]
    fn generated_claude_hooks_carry_status_marks_and_approval_in_ask_mode() {
        for mode in [crate::agent_permissions::AgentPermissionMode::Ask, crate::agent_permissions::AgentPermissionMode::Skip] {
            let mut st = isolated_683_state(68410);
            st.permission_mode = mode;
            st.provision_session_agent_files(None);
            let ask = mode == crate::agent_permissions::AgentPermissionMode::Ask;
            let read = |rel: &str| -> serde_json::Value {
                let raw = std::fs::read_to_string(st.session_dir.join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"));
                serde_json::from_str(&raw).unwrap_or_else(|e| panic!("{rel} is not JSON: {e}"))
            };
            let claude = read("hooks.json");
            let grok = read(".grok/hooks/designos-status.json");
            assert!(claude.get("permissions").is_none(), "hooks.json must never set permissions.allow: {claude}");

            for (cli, settings) in [("claude", &claude), ("grok", &grok)] {
                // #683 event stream: every status event keeps exactly one handler that appends to agent-events.jsonl.
                for event in [
                    "UserPromptSubmit",
                    "PreToolUse",
                    "PostToolUse",
                    "PostToolUseFailure",
                    "PermissionRequest",
                    "Notification",
                    "Stop",
                    "StopFailure",
                    "StopCancelled",
                    "SessionStart",
                ] {
                    let handlers = hook_handlers(settings, event);
                    let streams = handlers.iter().filter(|h| h["command"].as_str().unwrap_or_default().contains("agent-events.jsonl")).count();
                    assert_eq!(streams, 1, "{cli} {event} must append to the event stream exactly once: {handlers:?}");
                }
                // #677 turn marks: start on the prompt, end on every stop flavour.
                let prompt = hook_handlers(settings, "UserPromptSubmit");
                assert!(prompt[0]["command"].as_str().unwrap_or_default().contains("\"turn_mark\":\"start\""), "{cli} prompt hook lost the start mark");
                for event in ["Stop", "StopFailure", "StopCancelled"] {
                    let stop = hook_handlers(settings, event);
                    assert!(stop[0]["command"].as_str().unwrap_or_default().contains("\"turn_mark\":\"end\""), "{cli} {event} lost the end mark");
                }
            }

            // #656 approval: only Claude in Ask mode runs approve-hook.sh, next to (not instead of) the status handler.
            let claude_permission = hook_handlers(&claude, "PermissionRequest");
            let approve: Vec<_> = claude_permission.iter().filter(|h| h["command"].as_str().unwrap_or_default().contains("approve-hook.sh")).collect();
            if ask {
                assert_eq!(claude_permission.len(), 2, "Claude PermissionRequest = status handler + approval handler: {claude_permission:?}");
                assert_eq!(approve.len(), 1);
                assert_eq!(approve[0]["timeout"].as_u64(), Some(crate::agent_permissions::APPROVAL_HOOK_TIMEOUT_SECS));
                assert!(st.session_dir.join("approve-hook.sh").is_file(), "approve-hook.sh must be written for the hook to run");
            } else {
                assert_eq!(claude_permission.len(), 1, "Skip mode keeps only the status handler: {claude_permission:?}");
                assert!(approve.is_empty(), "Skip mode must not install the approval hook");
            }
            assert_eq!(hook_handlers(&grok, "PermissionRequest").len(), 1, "Grok never runs the Claude approval script");
            let _ = std::fs::remove_dir_all(&st.session_dir);
        }
    }

    #[test]
    fn one_permission_request_drives_turn_status_and_the_in_panel_approval() {
        use PermissionMode::{Ask, Skip};
        // (mode, window focused, surface visible, turn banners, approval notifications): one notification per request.
        for (mode, focused, visible, expected_banner, expected_approval) in [(Ask, false, true, 0, 1), (Skip, false, true, 1, 0), (Ask, true, false, 1, 0)] {
            let mut st = isolated_683_state(68411);
            st.permission_mode = mode;
            let sink = std::sync::Arc::new(crate::agent_notify::StubNotificationSink::new());
            st.notifier = crate::agent_notify::AgentNotifier::with_sink(sink.clone());
            let recorder = crate::agent_permissions::RecordingNotifier::default();
            st.set_approval_notifier(std::sync::Arc::new(recorder.clone()));
            let idx = spawn_mock_agent_tab(&mut st, AgentCliPreset::Claude, Some("sid-both"));
            st.active_tab = idx;
            st.window_focused = focused;
            st.surface_visible = visible;

            let approvals_dir = st.tabs[idx].approvals_dir.clone();
            if mode == Ask {
                std::fs::create_dir_all(&approvals_dir).unwrap();
                let request = serde_json::json!({
                    "id": "5001-Bash", "tool_name": "Bash", "summary": "cargo test", "timestamp": crate::agent_sessions::unix_timestamp_secs(),
                });
                std::fs::write(approvals_dir.join("5001-Bash.request.json"), request.to_string()).unwrap();
            }
            std::fs::write(
                st.session_dir.join("agent-events.jsonl"),
                concat!(
                    "{\"t\":400.0,\"e\":\"UserPromptSubmit\",\"p\":{\"session_id\":\"sid-both\"}}\n",
                    "{\"t\":401.0,\"e\":\"PermissionRequest\",\"p\":{\"session_id\":\"sid-both\",\"tool_name\":\"Bash\"}}\n",
                ),
            )
            .unwrap();
            let _ = st.poll_live_agent_bridge();

            // #683: the request reached the turn tracker and the status stream.
            assert!(
                matches!(st.status_changes.as_slice(), [(_, AgentTabStatus::Working(_)), (_, AgentTabStatus::WaitingForInput(_))]),
                "prompt then permission request must both reach the status stream: {:?}",
                st.status_changes
            );
            // Exactly one OS notification per request: the approval one while the window is unfocused in Ask mode,
            // the turn banner otherwise (including a hidden tab in a focused window).
            let case = format!("{mode:?} focused={focused} visible={visible}");
            assert_eq!(sink.count(), expected_banner, "turn-status banners for {case}");
            assert_eq!(recorder.recorded_events().len(), expected_approval, "approval notifications for {case}");
            let _ = st.poll_live_agent_bridge();
            assert_eq!(recorder.recorded_events().len(), expected_approval, "a second poll must not re-notify ({case})");
            assert_eq!(sink.count(), expected_banner, "a second poll must not re-notify ({case})");
            if mode == Skip {
                assert!(!st.needs_approval(), "Skip mode raises no in-panel approval");
                kill_tabs(&mut st);
                let _ = std::fs::remove_dir_all(&st.session_dir);
                continue;
            }
            // #656: the same request is an in-panel approval, and answering it writes the answer file the hook polls.
            assert!(st.needs_approval());
            assert_eq!(st.pending_approvals.len(), 1);
            assert_eq!(st.tabs[idx].effective_status(), AgentTabStatus::WaitingForInput(Some("Needs approval".into())));
            st.answer_pending_approval("5001-Bash", crate::agent_permissions::ApprovalDecision::Allow);
            assert_eq!(std::fs::read_to_string(approvals_dir.join("5001-Bash.answer")).unwrap().trim(), "allow");
            kill_tabs(&mut st);
            let _ = std::fs::remove_dir_all(&st.session_dir);
        }
    }

    #[test]
    fn header_strip_gives_its_right_slot_to_a_pending_approval() {
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        crate::theme::apply(&ctx);
        let (doc, page, ids) = make_three_layer_doc();
        let sel = [ids[0], ids[1]];
        let mut st = isolated_683_state(68412);
        st.permission_mode = crate::agent_permissions::AgentPermissionMode::Ask;
        let idx = spawn_mock_agent_tab(&mut st, AgentCliPreset::Claude, Some("sid-strip-approval"));
        let approvals_dir = st.tabs[idx].approvals_dir.clone();
        std::fs::create_dir_all(&approvals_dir).unwrap();
        let request = serde_json::json!({
            "id": "6001-Bash", "tool_name": "Bash", "summary": "cargo check", "timestamp": crate::agent_sessions::unix_timestamp_secs(),
        });
        std::fs::write(approvals_dir.join("6001-Bash.request.json"), request.to_string()).unwrap();
        std::fs::write(
            st.session_dir.join("agent-events.jsonl"),
            concat!(
                "{\"t\":500.0,\"e\":\"UserPromptSubmit\",\"p\":{\"session_id\":\"sid-strip-approval\"}}\n",
                "{\"t\":501.0,\"e\":\"PreToolUse\",\"p\":{\"session_id\":\"sid-strip-approval\",\"tool_name\":\"Bash\"}}\n",
            ),
        )
        .unwrap();
        let _ = st.poll_live_agent_bridge();

        let panel_rect = Rect::from_min_size(Pos2::new(48.0, 0.0), Vec2::new(280.0, 600.0));
        let _ = ctx.run_ui(egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1440.0, 900.0))), ..Default::default() }, |ui| {
            crate::agent_selection_chip::with_selection_summary(&doc, page, &sel, |summary| {
                let _ = show_left_with_selection(&mut st, ui, panel_rect, None, summary);
            });
        });
        let rect_of = |name: &str| -> Option<Rect> { ctx.data(|d| d.get_temp(Id::new(("test-rect", name.to_string())))) };
        let chip = rect_of("agent-selection-chip-left").expect("selection chip stays visible next to the approval");
        let pill = rect_of("agents-needs-approval-pill").expect("pending approval takes the right slot");
        assert!(rect_of("agents-header-turn-status").is_none(), "the turn status must not draw a second right-hand item");
        assert!(!chip.intersects(pill) && chip.right() < pill.left(), "chip {chip:?} left of approval {pill:?}");
        assert!(pill.right() <= panel_rect.right(), "approval stays inside the panel");
        let mention = rect_of("agent-mention-btn-left").expect("@ Mention button stays in the header strip");
        assert!(!mention.intersects(pill) && pill.right() <= mention.left(), "approval {pill:?} left of mention {mention:?}");
        assert!(!mention.intersects(chip) && mention.right() <= panel_rect.right(), "chip {chip:?} clear of mention {mention:?}");

        kill_tabs(&mut st);
        let _ = std::fs::remove_dir_all(&st.session_dir);
    }

    #[test]
    fn header_strip_keeps_still_running_notice_stop_and_mention_apart_at_narrow_widths() {
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        crate::theme::apply(&ctx);
        let (doc, page, ids) = make_three_layer_doc();
        let sel = [ids[0], ids[1]];
        for width in [280.0f32, 300.0] {
            let mut st = isolated_683_state(68413);
            let idx = spawn_mock_agent_tab(&mut st, AgentCliPreset::Claude, Some("sid-strip-still-running"));
            *st.tabs[idx].agent_status.lock().unwrap() = AgentTabStatus::Working(Some("Thinking".into()));
            st.tabs[idx].still_running_notice = Some(crate::agent_stop::STILL_RUNNING_NOTICE.to_string());

            let panel_rect = Rect::from_min_size(Pos2::new(48.0, 0.0), Vec2::new(width, 600.0));
            let _ = ctx.run_ui(egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1440.0, 900.0))), ..Default::default() }, |ui| {
                crate::agent_selection_chip::with_selection_summary(&doc, page, &sel, |summary| {
                    let _ = show_left_with_selection(&mut st, ui, panel_rect, None, summary);
                });
            });
            let rect_of = |name: &str| -> Option<Rect> { ctx.data(|d| d.get_temp(Id::new(("test-rect", name.to_string())))) };
            let chip = rect_of("agent-selection-chip-left").expect("chip");
            let notice = rect_of("agents-header-turn-status").expect("still-running notice takes the right slot");
            let mention = rect_of("agent-mention-btn-left").expect("@ Mention button");
            let stop = rect_of("agents-header-stop-btn").expect("Stop button");
            let popout = rect_of("agents-header-popout").expect("pop-out button");
            let strip = [("chip", chip), ("notice", notice), ("mention", mention)];
            for (i, (an, a)) in strip.iter().enumerate() {
                assert!(a.left() >= panel_rect.left() && a.right() <= panel_rect.right(), "{an} {a:?} inside {width}px panel");
                for (bn, b) in strip.iter().skip(i + 1) {
                    assert!(!a.intersects(*b), "{an} {a:?} overlaps {bn} {b:?} at {width}px");
                }
            }
            assert!(chip.right() <= notice.left() && notice.right() <= mention.left(), "chip | notice | mention order at {width}px");
            assert!(stop.right() <= popout.left() && !stop.intersects(popout), "Stop {stop:?} left of pop-out {popout:?} at {width}px");
            assert!(popout.right() <= panel_rect.right(), "pop-out inside panel");
            assert!(stop.bottom() <= chip.top().min(mention.top()), "Stop sits in the chrome bar above the strip");

            kill_tabs(&mut st);
            let _ = std::fs::remove_dir_all(&st.session_dir);
        }
    }

    #[test]
    fn esc_with_the_mention_popup_open_closes_the_popup_and_does_not_stop_the_turn() {
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        crate::theme::apply(&ctx);
        let (doc, page, _ids) = make_three_layer_doc();
        let mut st = isolated_683_state(68414);
        let idx = spawn_mock_agent_tab(&mut st, AgentCliPreset::Claude, Some("sid-esc-popup"));
        *st.tabs[idx].agent_status.lock().unwrap() = AgentTabStatus::Working(Some("Thinking".into()));
        st.viewport_focused = false;
        st.chrome_focused = true;
        st.mention_popup.toggle_terminal_mode_for("agents-left-pty", &doc, page);
        assert!(st.mention_popup.is_open());

        let panel_rect = Rect::from_min_size(Pos2::new(48.0, 0.0), Vec2::new(300.0, 600.0));
        let esc_frame = |st: &mut TerminalState| {
            let raw = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1440.0, 900.0))),
                events: vec![egui::Event::Key { key: Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::NONE }],
                ..Default::default()
            };
            let _ = ctx.run_ui(raw, |ui| {
                crate::agent_selection_chip::with_selection_summary(&doc, page, &[], |summary| {
                    let _ = show_left_with_doc(st, ui, panel_rect, None, summary, &doc, page);
                });
            });
        };
        esc_frame(&mut st);
        assert!(!st.mention_popup.is_open(), "Esc closes the popup");
        assert!(st.tabs[idx].stop_state.is_none(), "the same Esc must not stop the running turn");
        assert!(st.tabs[idx].stop_sent_keys.is_empty(), "no interrupt key was sent");

        esc_frame(&mut st);
        assert!(st.tabs[idx].stop_state.is_some(), "a second Esc with the popup closed stops the turn");

        kill_tabs(&mut st);
        let _ = std::fs::remove_dir_all(&st.session_dir);
    }

    #[test]
    fn skill_text_documents_the_grammar() {
        let st = TerminalState::for_tab(65501);
        let _ = std::fs::create_dir_all(&st.session_dir);
        st.provision_session_agent_files(None);
        let skill_text = std::fs::read_to_string(st.session_dir.join("SKILL.md")).expect("read generated SKILL.md");
        assert!(skill_text.contains("designos:node/"), "SKILL.md must document designos:node/: {skill_text}");
        assert!(skill_text.contains("designos:variable/"), "SKILL.md must document designos:variable/: {skill_text}");
        assert!(skill_text.contains("designos:style/"), "SKILL.md must document designos:style/: {skill_text}");
        let _ = std::fs::remove_dir_all(&st.session_dir);
    }

    #[test]
    fn antigravity_in_ask_mode_reports_an_approval_prompt_from_its_output() {
        let waiting_after = |ask: bool| -> bool {
            let mut cmd = build_agent_shell_cmd(&default_shell_program(), "sleep 0.5; printf 'Allow this edit? (y/n)\\r\\n'; sleep 4 || true");
            isolate_zsh_from_home(&mut cmd);
            let mut tab = TerminalTab::spawn_with_session_cmd(1, AgentCliPreset::Antigravity, 24, 80, None, None, Some(cmd)).expect("spawn stub agy");
            tab.pty_waiting_heuristic.store(ask, std::sync::atomic::Ordering::Release);
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
            let mut waiting = false;
            while std::time::Instant::now() < deadline && !waiting {
                std::thread::sleep(std::time::Duration::from_millis(50));
                waiting = matches!(tab.effective_status(), AgentTabStatus::WaitingForInput(_));
            }
            let _ = tab.child.kill();
            waiting
        };
        assert!(waiting_after(true), "Ask mode: the approval prompt in agy's output marks the tab as waiting");
        assert!(!waiting_after(false), "Skip mode: agy status stays hook-driven, output text must not flip it");
    }

    #[test]
    fn approval_card_stacks_between_the_strip_and_the_receipt_row_without_overlap() {
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        crate::theme::apply(&ctx);
        let (doc, page, ids) = make_three_layer_doc();
        let sel = [ids[0], ids[1]];
        let mut st = isolated_683_state(68413);
        st.permission_mode = crate::agent_permissions::AgentPermissionMode::Ask;
        let idx = spawn_mock_agent_tab(&mut st, AgentCliPreset::Claude, Some("sid-stack"));
        let approvals_dir = st.tabs[idx].approvals_dir.clone();
        std::fs::create_dir_all(&approvals_dir).unwrap();
        let events_path = st.session_dir.join("agent-events.jsonl");
        // Turn 1 finished (receipt row), turn 2 is waiting on a permission request (approval card).
        std::fs::write(
            &events_path,
            concat!(
                "{\"t\":600.0,\"e\":\"UserPromptSubmit\",\"p\":{\"session_id\":\"sid-stack\"}}\n",
                "{\"t\":601.0,\"e\":\"PreToolUse\",\"p\":{\"session_id\":\"sid-stack\",\"tool_name\":\"Read\"}}\n",
                "{\"t\":604.0,\"e\":\"Stop\",\"p\":{\"session_id\":\"sid-stack\"}}\n",
                "{\"t\":610.0,\"e\":\"UserPromptSubmit\",\"p\":{\"session_id\":\"sid-stack\"}}\n",
                "{\"t\":611.0,\"e\":\"PermissionRequest\",\"p\":{\"session_id\":\"sid-stack\",\"tool_name\":\"Bash\"}}\n",
            ),
        )
        .unwrap();
        let request = serde_json::json!({
            "id": "7001-Bash", "tool_name": "Bash", "summary": "cargo check", "timestamp": crate::agent_sessions::unix_timestamp_secs(),
        });
        std::fs::write(approvals_dir.join("7001-Bash.request.json"), request.to_string()).unwrap();
        let _ = st.poll_live_agent_bridge();
        assert!(st.tabs[idx].last_turn_receipt().is_some(), "turn 1 left a receipt");
        assert!(st.needs_approval());

        let rect_of = |name: &str| -> Option<Rect> { ctx.data(|d| d.get_temp(Id::new(("test-rect", name.to_string())))) };
        for height in [600.0_f32, 320.0] {
            let panel_rect = Rect::from_min_size(Pos2::new(48.0, 0.0), Vec2::new(300.0, height));
            let _ = ctx.run_ui(egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1440.0, 900.0))), ..Default::default() }, |ui| {
                crate::agent_selection_chip::with_selection_summary(&doc, page, &sel, |summary| {
                    let _ = show_left_with_selection(&mut st, ui, panel_rect, None, summary);
                });
            });
            let chip = rect_of("agent-selection-chip-left").expect("chip");
            let pill = rect_of("agents-needs-approval-pill").expect("approval status in the strip");
            let card = rect_of("agent-approval-card").expect("approval card");
            let receipt = rect_of("agents-turn-receipt-line").expect("receipt row");
            assert!(!chip.intersects(pill), "h={height}: chip {chip:?} vs approval status {pill:?}");
            assert!(pill.bottom() <= card.top() + 0.5, "h={height}: strip {pill:?} above the card {card:?}");
            assert!(card.bottom() <= receipt.top() + 0.5, "h={height}: card {card:?} above the receipt row {receipt:?}");
            assert!(receipt.bottom() <= panel_rect.bottom() + 0.5, "h={height}: receipt row inside the panel");
        }
        kill_tabs(&mut st);
        let _ = std::fs::remove_dir_all(&st.session_dir);
    }

    #[test]
    fn event_stream_status_changes_drive_the_tab_turn_ledger() {
        let mut st = isolated_683_state(68399);
        let idx = spawn_mock_agent_tab(&mut st, AgentCliPreset::Claude, Some("sid-ledger"));
        let events_path = st.session_dir.join("agent-events.jsonl");
        std::fs::write(&events_path, "{\"t\":300.0,\"e\":\"UserPromptSubmit\",\"p\":{\"session_id\":\"sid-ledger\"}}\n").unwrap();
        let _ = st.poll_live_agent_bridge();
        assert_eq!(st.ledger_for_tab_mut(idx).turns.len(), 1, "a Working status from the event stream opens a turn");

        let mut f = std::fs::OpenOptions::new().append(true).open(&events_path).unwrap();
        std::io::Write::write_all(&mut f, b"{\"t\":301.0,\"e\":\"Stop\",\"p\":{\"session_id\":\"sid-ledger\"}}\n").unwrap();
        let _ = st.poll_live_agent_bridge();
        assert_eq!(st.tabs[idx].effective_status(), AgentTabStatus::Done);
        assert!(st.ledger_for_tab_mut(idx).turns.is_empty(), "a turn without edits closes away on Stop");
        kill_tabs(&mut st);
    }

    #[test]
    fn works_for_each_cli_hook_file() {
        let status_path = std::path::Path::new("/tmp/designos-683-hooks-check/agent-status.json");
        let claude_grok = session_hooks_settings(status_path, None);
        for &(event, _) in crate::agent_turn_status::CLAUDE_GROK_HOOK_EVENTS {
            let cmd = claude_grok["hooks"][event][0]["hooks"][0]["command"].as_str().unwrap_or_default();
            assert!(cmd.contains("agent-events.jsonl"), "Claude/Grok hook for {event} must append to agent-events.jsonl: {cmd}");
            assert!(cmd.contains(event), "Claude/Grok hook for {event} must carry event name: {cmd}");
        }

        let agy = agy_session_hooks(status_path);
        for &(event, _) in crate::agent_turn_status::AGY_HOOK_EVENTS {
            let cmd = agy["designos-status"][event][0]["command"].as_str().unwrap_or_default();
            assert!(cmd.contains("agent-events.jsonl"), "Antigravity hook for {event} must append to agent-events.jsonl: {cmd}");
            assert!(cmd.contains(event), "Antigravity hook for {event} must carry event name: {cmd}");
        }
    }

    #[test]
    fn codex_uses_the_heuristic() {
        let mut st = isolated_683_state(68305);
        let codex_idx = spawn_mock_agent_tab(&mut st, AgentCliPreset::Codex, Some("codex-sid"));
        st.active_tab = codex_idx;

        // Write an event to agent-events.jsonl: Codex must ignore it.
        let events_path = st.session_dir.join("agent-events.jsonl");
        std::fs::write(&events_path, "{\"t\":10.0,\"e\":\"UserPromptSubmit\",\"p\":{\"session_id\":\"codex-sid\"}}\n").unwrap();
        let _ = st.poll_live_agent_bridge();
        // The backing shell prints its prompt on start; that output must not count as Codex activity here.
        *st.tabs[codex_idx].last_output_at.lock().unwrap() = None;
        assert_eq!(st.tabs[codex_idx].effective_status(), AgentTabStatus::Idle);

        // Heuristic / OSC status drives Codex Running -> Done.
        let (_, working) = parse_osc_agent_status_and_strip("\x1b]9999;working:Thinking\x07");
        if let Some(next) = working {
            *st.tabs[codex_idx].agent_status.lock().unwrap() = next;
        }
        assert!(matches!(st.tabs[codex_idx].effective_status(), AgentTabStatus::Working(_)));
        let (_, done) = parse_osc_agent_status_and_strip("\x1b]9999;done\x07");
        if let Some(next) = done {
            *st.tabs[codex_idx].agent_status.lock().unwrap() = next;
        }
        assert_eq!(st.tabs[codex_idx].effective_status(), AgentTabStatus::Done);
        kill_tabs(&mut st);
    }

    #[test]
    fn theme_has_info_success_warning_in_token_parity() {
        assert_eq!(theme::INFO, Color32::from_rgb(0x0D, 0x99, 0xFF));
        assert_eq!(theme::SUCCESS, Color32::from_rgb(0x14, 0xAE, 0x5C));
        assert_eq!(theme::WARNING, Color32::from_rgb(0xF5, 0x9E, 0x0B));

        let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let script = manifest_dir.join("../../scripts/design_os_self_evolve.py");
        let src = std::fs::read_to_string(&script).expect("read design_os_self_evolve.py");
        for name in ["\"INFO\"", "\"SUCCESS\"", "\"WARNING\""] {
            assert!(src.contains(name), "design_os_self_evolve.py missing {name}");
        }

        let tokens_path = manifest_dir.join("../../design/design.tokens.json");
        let tokens_raw = std::fs::read_to_string(&tokens_path).expect("read design.tokens.json");
        let tokens_json: serde_json::Value = serde_json::from_str(&tokens_raw).expect("parse design.tokens.json");
        assert_eq!(tokens_json["colors"]["info"]["$value"].as_str(), Some("#0d99ff"));
        assert_eq!(tokens_json["colors"]["success"]["$value"].as_str(), Some("#14ae5c"));
        assert_eq!(tokens_json["colors"]["warning"]["$value"].as_str(), Some("#f59e0b"));
    }

    #[test]
    fn touched_layers_do_not_bleed_into_other_active_tab_turn() {
        let mut st = isolated_683_state(68306);
        let tab_a = spawn_mock_agent_tab(&mut st, AgentCliPreset::Claude, Some("sid-a"));
        let tab_b = spawn_mock_agent_tab(&mut st, AgentCliPreset::Claude, Some("sid-b"));

        let events_path = st.session_dir.join("agent-events.jsonl");
        std::fs::write(
            &events_path,
            concat!(
                "{\"t\":10.0,\"e\":\"UserPromptSubmit\",\"p\":{\"session_id\":\"sid-a\"}}\n",
                "{\"t\":10.5,\"e\":\"UserPromptSubmit\",\"p\":{\"session_id\":\"sid-b\"}}\n",
                "{\"t\":11.0,\"e\":\"PreToolUse\",\"p\":{\"session_id\":\"sid-a\",\"tool_name\":\"mcp__designos__designos_update_node\"}}\n",
                "{\"t\":11.5,\"e\":\"PreToolUse\",\"p\":{\"session_id\":\"sid-b\",\"tool_name\":\"Read\"}}\n",
            ),
        )
        .unwrap();
        let _ = st.poll_live_agent_bridge();

        // Tab A is running a layer-mutation tool while Tab B is reading a file.
        st.record_touched_layers(&[101, 102, 103]);

        {
            use std::io::Write;
            let mut f = std::fs::OpenOptions::new().append(true).open(&events_path).unwrap();
            writeln!(f, "{{\"t\":15.0,\"e\":\"Stop\",\"p\":{{\"session_id\":\"sid-a\"}}}}").unwrap();
            writeln!(f, "{{\"t\":15.5,\"e\":\"Stop\",\"p\":{{\"session_id\":\"sid-b\"}}}}").unwrap();
        }
        let _ = st.poll_live_agent_bridge();

        let receipt_a = st.tabs[tab_a].last_turn_receipt().expect("receipt A");
        let receipt_b = st.tabs[tab_b].last_turn_receipt().expect("receipt B");
        assert_eq!(receipt_a.layers_changed, Some(3), "tab A performed the layer edit and must record 3 layers changed");
        assert_eq!(receipt_b.layers_changed, None, "tab B only ran Read and must not inherit tab A's touched layers");
        kill_tabs(&mut st);

        // MINOR 3: One Claude tab running `Read` alongside an open non-hooked Codex tab must not
        // inherit a Codex layer edit.
        let mut st_mixed = isolated_683_state(68311);
        let claude_idx = spawn_mock_agent_tab(&mut st_mixed, AgentCliPreset::Claude, Some("sid-claude-read"));
        let _codex_idx = spawn_mock_agent_tab(&mut st_mixed, AgentCliPreset::Codex, None);
        let mixed_events = st_mixed.session_dir.join("agent-events.jsonl");
        std::fs::write(
            &mixed_events,
            concat!(
                "{\"t\":20.0,\"e\":\"UserPromptSubmit\",\"p\":{\"session_id\":\"sid-claude-read\"}}\n",
                "{\"t\":21.0,\"e\":\"PreToolUse\",\"p\":{\"session_id\":\"sid-claude-read\",\"tool_name\":\"Read\"}}\n",
            ),
        )
        .unwrap();
        let _ = st_mixed.poll_live_agent_bridge();
        st_mixed.record_touched_layers(&[201, 202, 203, 204, 205]);
        {
            use std::io::Write;
            let mut f = std::fs::OpenOptions::new().append(true).open(&mixed_events).unwrap();
            writeln!(f, "{{\"t\":25.0,\"e\":\"Stop\",\"p\":{{\"session_id\":\"sid-claude-read\"}}}}").unwrap();
        }
        let _ = st_mixed.poll_live_agent_bridge();
        let claude_receipt = st_mixed.tabs[claude_idx].last_turn_receipt().expect("claude receipt");
        assert_eq!(claude_receipt.layers_changed, None, "Codex edit must not be attributed to a Claude tab running Read");
        kill_tabs(&mut st_mixed);
    }

    #[test]
    fn concurrent_hook_commands_do_not_interleave_records() {
        let tmp = std::env::temp_dir().join(format!("designos-683-concurrent-hooks-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        let events_path = tmp.join("agent-events.jsonl");
        let cmd_str = crate::agent_turn_status::event_append_command(&events_path, "PreToolUse");
        assert!(cmd_str.contains("got_lock=0") && cmd_str.contains("if [ \"$got_lock\" = 1 ]; then rmdir"), "hook must only rmdir lock when got_lock=1");

        let mut children = Vec::new();
        for i in 0..16 {
            let mut child = std::process::Command::new("sh").arg("-c").arg(&cmd_str).stdin(std::process::Stdio::piped()).spawn().expect("spawn parallel hook");
            let mut stdin = child.stdin.take().unwrap();
            std::thread::spawn(move || {
                use std::io::Write;
                let part1 = format!("{{\"session_id\":\"s-{i}\",\n");
                let part2 = format!("\"tool_name\":\"Tool{i}\"}}");
                stdin.write_all(part1.as_bytes()).unwrap();
                stdin.flush().unwrap();
                std::thread::sleep(std::time::Duration::from_millis(15));
                stdin.write_all(part2.as_bytes()).unwrap();
            });
            children.push(child);
        }
        for mut c in children {
            assert!(c.wait().unwrap().success());
        }

        let raw = std::fs::read_to_string(&events_path).expect("read agent-events.jsonl");
        let (records, consumed) = crate::agent_turn_status::parse_streaming_records(&raw);
        assert_eq!(consumed, raw.len(), "all bytes must parse without interleaved corruption: {raw}");
        assert_eq!(records.len(), 16, "all 16 concurrent hook records must parse cleanly");
        let _ = std::fs::remove_dir_all(&tmp);

        // Verify spawning a tab removes any stale `.lock` directories in `session_dir`.
        let mut st = isolated_683_state(68312);
        let stale_events_lock = st.session_dir.join("agent-events.jsonl.lock");
        let stale_status_lock = st.session_dir.join("agent-status.jsonl.lock");
        std::fs::create_dir_all(&stale_events_lock).unwrap();
        std::fs::create_dir_all(&stale_status_lock).unwrap();
        let _ = spawn_mock_agent_tab(&mut st, AgentCliPreset::Claude, Some("sid-lock-clean"));
        assert!(!stale_events_lock.exists(), "spawning a tab must remove stale agent-events.jsonl.lock");
        assert!(!stale_status_lock.exists(), "spawning a tab must remove stale agent-status.jsonl.lock");
        kill_tabs(&mut st);
    }

    #[test]
    fn process_exit_without_stop_when_unfocused_sends_notification() {
        let mut st = isolated_683_state(68307);
        let sink = std::sync::Arc::new(crate::agent_notify::StubNotificationSink::new());
        st.notifier = crate::agent_notify::AgentNotifier::with_sink(sink.clone());
        let tab = TerminalTab::spawn_exiting_stub_agent(1, 24, 80, None, None).expect("spawn exiting stub");
        st.tabs.push(tab);
        st.tabs[0].session_uuid = Some("sid-exit-1".to_string());
        st.active_tab = 0;
        st.window_focused = false;
        st.surface_visible = true;

        let events_path = st.session_dir.join("agent-events.jsonl");
        std::fs::write(&events_path, "{\"t\":10.0,\"e\":\"UserPromptSubmit\",\"p\":{\"session_id\":\"sid-exit-1\"}}\n").unwrap();
        let _ = st.poll_live_agent_bridge();

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        while st.tabs[0].is_alive() && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        // A quick exit with no output yet waits up to 100 ms for the reader to drain before it is handled.
        while !st.tabs[0].exited_notice_shown && std::time::Instant::now() < deadline {
            let _ = st.poll_live_agent_bridge();
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        assert_eq!(st.tabs[0].effective_status(), AgentTabStatus::Done);
        assert_eq!(st.tabs[0].last_turn_receipt().map(|r| r.format_line()), Some("Ended without a stop event".to_string()));
        assert_eq!(sink.count(), 1, "process exit during a running turn while unfocused must send a Finished notification");
    }

    #[test]
    fn clicking_turn_receipt_row_opens_turn_summary() {
        let mut st = isolated_683_state(68308);
        let tab_idx = spawn_mock_agent_tab(&mut st, AgentCliPreset::Claude, Some("sid-receipt-click"));
        st.active_tab = tab_idx;
        let tab_id = st.tabs[tab_idx].id;

        let events_path = st.session_dir.join("agent-events.jsonl");
        std::fs::write(
            &events_path,
            concat!(
                "{\"t\":10.0,\"e\":\"UserPromptSubmit\",\"p\":{\"session_id\":\"sid-receipt-click\"}}\n",
                "{\"t\":12.0,\"e\":\"PreToolUse\",\"p\":{\"session_id\":\"sid-receipt-click\",\"tool_name\":\"mcp__designos__designos_update_node\"}}\n",
                "{\"t\":15.0,\"e\":\"Stop\",\"p\":{\"session_id\":\"sid-receipt-click\"}}\n",
            ),
        )
        .unwrap();
        let _ = st.poll_live_agent_bridge();
        assert!(!st.tabs[tab_idx].turn_tracker.summary_open);
        st.status_msg = None;

        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        crate::theme::install_fonts(&ctx);
        let panel_rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(320.0, 480.0));
        let _ = ctx.run_ui(egui::RawInput { screen_rect: Some(panel_rect), ..Default::default() }, |ui| {
            let _ = show_left(&mut st, ui, panel_rect, None);
        });
        let receipt_rect: Rect = ctx
            .data_mut(|d| d.get_temp::<Rect>(Id::new(("test-rect", "agents-turn-receipt-line".to_string()))))
            .expect("agents-turn-receipt-line must be recorded");
        let pos = receipt_rect.center();

        let mut click_actions = Vec::new();
        for pressed in [true, false] {
            let mut raw = egui::RawInput { screen_rect: Some(panel_rect), ..Default::default() };
            raw.events.push(egui::Event::PointerMoved(pos));
            raw.events.push(egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE });
            let _ = ctx.run_ui(raw, |ui| {
                click_actions.extend(show_left(&mut st, ui, panel_rect, None));
            });
        }

        assert!(st.tabs[tab_idx].turn_tracker.summary_open, "clicking the receipt row must open the turn summary");
        assert!(click_actions.contains(&TerminalAction::OpenTurnReceipt(tab_id)), "clicking receipt must emit OpenTurnReceipt({tab_id})");
        assert!(st.status_msg.is_none(), "receipt click must not duplicate the toast text into terminal.status_msg");
        kill_tabs(&mut st);
    }

    #[test]
    fn os_notification_sink_reports_denied_immediately_and_click_focuses_tab() {
        use crate::agent_notify::{AgentNotification, NotificationOutcome, NotificationSink, NotifyKind, OsNotificationSink};

        let slow_failing_sink = std::sync::Arc::new(OsNotificationSink::with_runner(|_notif| {
            std::thread::sleep(std::time::Duration::from_millis(60));
            false
        }));
        let t0 = std::time::Instant::now();
        let outcome = slow_failing_sink.send(&AgentNotification::new(1, "Claude Code #1", "Untitled", NotifyKind::Finished));
        let send_elapsed = t0.elapsed();
        assert_eq!(outcome, NotificationOutcome::Sent, "OsNotificationSink::send must return Sent immediately without blocking on worker");
        assert!(send_elapsed < std::time::Duration::from_millis(30), "OsNotificationSink::send must not block the UI thread (took {send_elapsed:?})");

        let failing_sink = std::sync::Arc::new(OsNotificationSink::with_runner(|_notif| false));
        let mut st_denied = isolated_683_state(68309);
        st_denied.notifier = crate::agent_notify::AgentNotifier::with_sink(failing_sink);
        let _ = spawn_mock_agent_tab(&mut st_denied, AgentCliPreset::Claude, Some("sid-denied"));
        st_denied.window_focused = false;
        st_denied.surface_visible = true;
        st_denied.status_msg = None;

        let events_path = st_denied.session_dir.join("agent-events.jsonl");
        std::fs::write(
            &events_path,
            concat!(
                "{\"t\":1.0,\"e\":\"UserPromptSubmit\",\"p\":{\"session_id\":\"sid-denied\"}}\n",
                "{\"t\":3.0,\"e\":\"Stop\",\"p\":{\"session_id\":\"sid-denied\"}}\n",
            ),
        )
        .unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_millis(500);
        while st_denied.status_msg.as_deref() != Some(crate::agent_notify::NOTIFICATIONS_OFF_STATUS_MSG) && std::time::Instant::now() < deadline {
            let _ = st_denied.poll_live_agent_bridge();
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert_eq!(
            st_denied.status_msg.as_deref(),
            Some(crate::agent_notify::NOTIFICATIONS_OFF_STATUS_MSG),
            "poll_live_agent_bridge must report NOTIFICATIONS_OFF_STATUS_MSG once after the background worker detects a failed OS notification"
        );
        st_denied.status_msg = None;
        let _ = st_denied.poll_live_agent_bridge();
        assert!(st_denied.status_msg.is_none(), "NOTIFICATIONS_OFF_STATUS_MSG must only be emitted once");
        kill_tabs(&mut st_denied);

        let mut st = isolated_683_state(68310);
        let sink = std::sync::Arc::new(crate::agent_notify::StubNotificationSink::new());
        st.notifier = crate::agent_notify::AgentNotifier::with_sink(sink.clone());
        let fg_idx = spawn_mock_agent_tab(&mut st, AgentCliPreset::Claude, Some("sid-fg-click"));
        let bg_idx = spawn_mock_agent_tab(&mut st, AgentCliPreset::Grok, Some("sid-bg-click"));
        let bg_tab_id = st.tabs[bg_idx].id;
        st.active_tab = fg_idx;
        st.window_focused = false;
        st.surface_visible = false;

        let events_path2 = st.session_dir.join("agent-events.jsonl");
        std::fs::write(
            &events_path2,
            concat!(
                "{\"t\":10.0,\"e\":\"UserPromptSubmit\",\"p\":{\"sessionId\":\"sid-bg-click\"}}\n",
                "{\"t\":14.0,\"e\":\"Stop\",\"p\":{\"sessionId\":\"sid-bg-click\"}}\n",
            ),
        )
        .unwrap();
        let _ = st.poll_live_agent_bridge();
        assert!(st.tabs[bg_idx].unread);
        assert_eq!(st.active_tab, fg_idx);

        sink.click_last();
        let actions = st.poll_live_agent_bridge();
        assert_eq!(st.active_tab, bg_idx, "clicking notification must select the notified tab");
        assert!(!st.tabs[bg_idx].unread, "selecting tab via notification click must clear unread");
        assert!(st.window_focused && st.surface_visible, "clicking notification must focus window and reveal surface");
        assert!(actions.contains(&TerminalAction::FocusAgentTab(bg_tab_id)));
        kill_tabs(&mut st);

        let mut app = crate::app::App::new(designos_doc::Document::new("Untitled"));
        let ctx = egui::Context::default();
        let out = ctx.run_ui(egui::RawInput::default(), |ui| {
            crate::shell::execute_terminal_actions_with_ctx(&mut app, vec![TerminalAction::FocusAgentTab(bg_tab_id)], Some(ui.ctx()));
        });
        let vp_cmds = &out.viewport_output.get(&egui::ViewportId::ROOT).expect("root viewport").commands;
        assert!(vp_cmds.iter().any(|c| matches!(c, egui::ViewportCommand::Focus)), "FocusAgentTab must send ViewportCommand::Focus: {vp_cmds:?}");
        assert_eq!(app.ui.rail, Some(crate::app::RailItem::Agents));
        assert!(!app.ui.left_collapsed);
    }

    #[test]
    fn bridge_surfaces_a_request_and_writes_the_answer() {
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        crate::theme::apply(&ctx);

        let tmp = std::env::temp_dir().join(format!("designos-656-bridge-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        let mut st = TerminalState::default();
        st.bind_session(&format!("656-bridge-{}", std::process::id()));
        st.permission_mode = crate::agent_permissions::AgentPermissionMode::Ask;
        st.ensure_default_tab(None);
        if let Some(tab) = st.active_tab_mut() {
            tab.preset = AgentCliPreset::Claude;
            tab.title = "Claude CLI #1".into();
        }

        let approvals_dir = crate::agent_permissions::approvals_dir(&st.session_dir);
        std::fs::create_dir_all(&approvals_dir).unwrap();
        let now = crate::agent_sessions::unix_timestamp_secs();

        // 1. Write a request and verify the approval card surfaces and clicking Approve writes "allow"
        let req1 = serde_json::json!({
            "id": "2001-Bash",
            "tool_name": "Bash",
            "summary": "cargo test -p designos-app",
            "timestamp": now,
        });
        std::fs::write(approvals_dir.join("2001-Bash.request.json"), req1.to_string()).unwrap();

        let panel_rect = Rect::from_min_size(Pos2::new(48.0, 0.0), Vec2::new(300.0, 800.0));
        let run_frame = |st: &mut TerminalState, events: Vec<egui::Event>| -> egui::FullOutput {
            let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1600.0, 900.0))), events, ..Default::default() };
            ctx.run_ui(raw, |ui| {
                let _ = show_left(st, ui, panel_rect, None);
            })
        };

        let out = run_frame(&mut st, vec![]);
        let (texts, boxes) = crate::lint::scan_shapes(&out.shapes);
        assert!(crate::lint::check_text(&texts, &boxes).is_empty());
        let all_txt: Vec<&str> = texts.iter().map(|t| t.text.as_str()).collect();
        assert!(all_txt.iter().any(|t| t.contains("Bash")), "Tool name 'Bash' must be painted: {all_txt:?}");
        assert!(all_txt.iter().any(|t| t.contains("cargo test")), "Summary must be painted: {all_txt:?}");
        assert!(all_txt.contains(&"Approve"), "Approve button must be painted: {all_txt:?}");
        assert!(all_txt.contains(&"Deny"), "Deny button must be painted: {all_txt:?}");

        let approve_rect: Rect =
            ctx.data(|d| d.get_temp(Id::new(("test-rect", "agent-approval-approve".to_string())))).expect("agent-approval-approve rect recorded");
        let pos = approve_rect.center();
        let _ = run_frame(&mut st, vec![egui::Event::PointerMoved(pos)]);
        let _ = run_frame(
            &mut st,
            vec![
                egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE },
                egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE },
            ],
        );

        let ans1 = std::fs::read_to_string(approvals_dir.join("2001-Bash.answer")).expect("2001-Bash.answer written");
        assert_eq!(ans1.trim(), "allow");
        assert!(!st.needs_approval(), "Approval card must clear after clicking Approve");

        // 2. Write a second request and verify clicking Deny writes "deny"
        let req2 = serde_json::json!({
            "id": "2002-Write",
            "tool_name": "Write",
            "summary": "/tmp/dangerous.sh",
            "timestamp": now + 1,
        });
        std::fs::write(approvals_dir.join("2002-Write.request.json"), req2.to_string()).unwrap();

        let _ = run_frame(&mut st, vec![]);
        assert!(st.needs_approval());
        let deny_rect: Rect = ctx.data(|d| d.get_temp(Id::new(("test-rect", "agent-approval-deny".to_string())))).expect("agent-approval-deny rect recorded");
        let deny_pos = deny_rect.center();
        let _ = run_frame(&mut st, vec![egui::Event::PointerMoved(deny_pos)]);
        let _ = run_frame(
            &mut st,
            vec![
                egui::Event::PointerButton { pos: deny_pos, button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE },
                egui::Event::PointerButton { pos: deny_pos, button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE },
            ],
        );

        let ans2 = std::fs::read_to_string(approvals_dir.join("2002-Write.answer")).expect("2002-Write.answer written");
        assert_eq!(ans2.trim(), "deny");
        assert!(!st.needs_approval(), "Approval card must clear after clicking Deny");

        let _ = std::fs::remove_dir_all(&st.session_dir);
        let _ = std::fs::remove_dir_all(&tmp);
        kill_tabs(&mut st);
    }

    #[test]
    fn status_becomes_needs_approval() {
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        crate::theme::apply(&ctx);

        let mut st = TerminalState::default();
        st.bind_session(&format!("656-status-{}", std::process::id()));
        st.open_approval_fixture("agents-approval-command", None);
        assert!(st.needs_approval());
        let tab_status = st.active_tab().unwrap().agent_status.lock().unwrap().clone();
        assert_eq!(tab_status, AgentTabStatus::WaitingForInput(Some("Needs approval".into())));

        let panel_rect = Rect::from_min_size(Pos2::new(48.0, 0.0), Vec2::new(300.0, 800.0));
        let out = ctx.run_ui(egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1600.0, 900.0))), ..Default::default() }, |ui| {
            let _ = show_left(&mut st, ui, panel_rect, None);
        });
        let (texts, boxes) = crate::lint::scan_shapes(&out.shapes);
        assert!(crate::lint::check_text(&texts, &boxes).is_empty());
        let all_txt: Vec<&str> = texts.iter().map(|t| t.text.as_str()).collect();
        assert!(all_txt.contains(&"Needs approval"), "Status pill 'Needs approval' must be painted: {all_txt:?}");
        let pill_rect: Option<Rect> = ctx.data(|d| d.get_temp(Id::new(("test-rect", "agents-needs-approval-pill".to_string()))));
        assert!(pill_rect.is_some(), "agents-needs-approval-pill must be recorded");

        let _ = std::fs::remove_dir_all(&st.session_dir);
        kill_tabs(&mut st);
    }

    #[test]
    fn grok_and_agy_cards_have_no_buttons() {
        for preset in [AgentCliPreset::Grok, AgentCliPreset::Antigravity] {
            let ctx = egui::Context::default();
            crate::theme::install_fonts(&ctx);
            crate::theme::apply(&ctx);

            let mut st = TerminalState::default();
            st.bind_session(&format!("656-non-claude-{}-{:?}", std::process::id(), preset));
            st.permission_mode = crate::agent_permissions::AgentPermissionMode::Ask;
            st.ensure_default_tab(None);
            st.side_panel_pty_open = false;
            st.chat_mode = true;
            if let Some(tab) = st.active_tab_mut() {
                tab.preset = preset;
                tab.permission_mode = crate::agent_permissions::AgentPermissionMode::Ask;
                tab.title = format!("{} #1", preset.button_label());
                *tab.agent_status.lock().unwrap() = AgentTabStatus::WaitingForInput(Some("Needs approval".into()));
            }
            assert!(st.needs_approval());

            let panel_rect = Rect::from_min_size(Pos2::new(48.0, 0.0), Vec2::new(300.0, 800.0));
            let out = ctx.run_ui(egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1600.0, 900.0))), ..Default::default() }, |ui| {
                let _ = show_left(&mut st, ui, panel_rect, None);
            });
            let (texts, boxes) = crate::lint::scan_shapes(&out.shapes);
            assert!(crate::lint::check_text(&texts, &boxes).is_empty());
            let all_txt: Vec<&str> = texts.iter().map(|t| t.text.as_str()).collect();
            assert!(all_txt.contains(&"Open tab"), "Non-Claude card must show 'Open tab': {all_txt:?}");
            assert!(!all_txt.contains(&"Approve"), "Non-Claude card must NOT show 'Approve': {all_txt:?}");
            assert!(!all_txt.contains(&"Deny"), "Non-Claude card must NOT show 'Deny': {all_txt:?}");

            let approve_recorded: Option<Rect> = ctx.data(|d| d.get_temp(Id::new(("test-rect", "agent-approval-approve".to_string()))));
            let deny_recorded: Option<Rect> = ctx.data(|d| d.get_temp(Id::new(("test-rect", "agent-approval-deny".to_string()))));
            let open_tab_rect: Rect =
                ctx.data(|d| d.get_temp(Id::new(("test-rect", "agent-approval-open-tab".to_string())))).expect("agent-approval-open-tab recorded");
            assert!(approve_recorded.is_none());
            assert!(deny_recorded.is_none());

            // Clicking "Open tab" switches to the live PTY view and focuses the terminal viewport
            let pos = open_tab_rect.center();
            let _ = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1600.0, 900.0))),
                    events: vec![
                        egui::Event::PointerMoved(pos),
                        egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE },
                        egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE },
                    ],
                    ..Default::default()
                },
                |ui| {
                    let _ = show_left(&mut st, ui, panel_rect, None);
                },
            );
            assert!(st.side_panel_pty_open, "Clicking 'Open tab' must open the PTY tab view");
            assert!(st.viewport_focused, "Clicking 'Open tab' must focus the terminal viewport");

            let _ = std::fs::remove_dir_all(&st.session_dir);
            kill_tabs(&mut st);
        }
    }

    #[test]
    fn unfocused_window_sends_one_notification_without_the_command_text() {
        let recorder = crate::agent_permissions::RecordingNotifier::default();
        let mut st = TerminalState::default();
        st.bind_session(&format!("656-notify-{}", std::process::id()));
        st.doc_name = "Checkout Flow".into();
        st.set_approval_notifier(Arc::new(recorder.clone()));
        st.set_window_focused(false);
        st.ensure_default_tab(None);
        if let Some(tab) = st.active_tab_mut() {
            tab.preset = AgentCliPreset::Claude;
        }

        let approvals_dir = crate::agent_permissions::approvals_dir(&st.session_dir);
        std::fs::create_dir_all(&approvals_dir).unwrap();
        let req = serde_json::json!({
            "id": "3001-Bash",
            "tool_name": "Bash",
            "summary": "rm -rf /secret/command --token super-secret",
            "timestamp": crate::agent_sessions::unix_timestamp_secs(),
        });
        std::fs::write(approvals_dir.join("3001-Bash.request.json"), req.to_string()).unwrap();

        let _ = st.poll_live_agent_bridge();
        let _ = st.poll_live_agent_bridge();
        let events = recorder.recorded_events();
        assert_eq!(events.len(), 1, "Exactly one notification must be sent per wait episode");
        assert_eq!(events[0], "Claude CLI needs approval — Checkout Flow");
        assert!(!events[0].contains("rm -rf"), "Notification body must never contain the command text");
        assert!(!events[0].contains("super-secret"), "Notification body must never contain command arguments");

        let _ = std::fs::remove_dir_all(&st.session_dir);
        kill_tabs(&mut st);
    }

    #[test]
    fn sim_approval_card_and_permissions_modal_pass_layout_lint() {
        for preset_slug in ["agents-approval-command", "agents-approval-queued", "agents-approval-open-tab"] {
            let ctx = egui::Context::default();
            crate::theme::install_fonts(&ctx);
            crate::theme::apply(&ctx);
            let mut st = TerminalState::default();
            st.bind_session(&format!("656-lint-{preset_slug}-{}", std::process::id()));
            st.open_approval_fixture(preset_slug, None);

            crate::lint::take_icons();
            let panel_rect = Rect::from_min_size(Pos2::new(48.0, 0.0), Vec2::new(300.0, 800.0));
            let out = ctx.run_ui(egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1600.0, 900.0))), ..Default::default() }, |ui| {
                let _ = show_left(&mut st, ui, panel_rect, None);
            });
            let (texts, boxes) = crate::lint::scan_shapes(&out.shapes);
            let text_findings = crate::lint::check_text(&texts, &boxes);
            let icon_findings = crate::lint::check_icons(&crate::lint::take_icons());
            let purple_findings = crate::lint::check_no_purple_chrome(&out.shapes, None);
            assert!(text_findings.is_empty(), "{preset_slug} text lint findings: {text_findings:?}");
            assert!(icon_findings.is_empty(), "{preset_slug} icon lint findings: {icon_findings:?}");
            assert!(purple_findings.is_empty(), "{preset_slug} purple lint findings: {purple_findings:?}");

            let _ = std::fs::remove_dir_all(&st.session_dir);
            kill_tabs(&mut st);
        }

        // Also lint the Agent permissions modal
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        crate::theme::apply(&ctx);
        let mut open = true;
        let mut mode = crate::agent_permissions::AgentPermissionMode::Skip;
        crate::lint::take_icons();
        let out = ctx.run_ui(egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1600.0, 900.0))), ..Default::default() }, |_ui| {
            let _ = crate::agent_approval_card::paint_permissions_modal(&ctx, &mut open, &mut mode, None);
        });
        let (texts, boxes) = crate::lint::scan_shapes(&out.shapes);
        let text_findings = crate::lint::check_text(&texts, &boxes);
        let icon_findings = crate::lint::check_icons(&crate::lint::take_icons());
        let purple_findings = crate::lint::check_no_purple_chrome(&out.shapes, None);
        assert!(text_findings.is_empty(), "agent-permissions modal text lint findings: {text_findings:?}");
        assert!(icon_findings.is_empty(), "agent-permissions modal icon lint findings: {icon_findings:?}");
        assert!(purple_findings.is_empty(), "agent-permissions modal purple lint findings: {purple_findings:?}");
    }

    #[test]
    fn closing_or_exiting_one_tab_preserves_other_tab_pending_approvals() {
        let mut st = TerminalState::default();
        st.bind_session(&format!("656-tab-isolation-{}", std::process::id()));
        st.permission_mode = crate::agent_permissions::AgentPermissionMode::Ask;
        st.ensure_default_tab(None);
        let tab1_id = st
            .active_tab_mut()
            .map(|tab| {
                tab.preset = AgentCliPreset::Claude;
                tab.permission_mode = crate::agent_permissions::AgentPermissionMode::Ask;
                tab.id
            })
            .expect("tab 1 exists");
        let tab1_approvals_dir = crate::agent_permissions::tab_approvals_dir(&st.session_dir, tab1_id);
        std::fs::create_dir_all(&tab1_approvals_dir).unwrap();
        let req_path = tab1_approvals_dir.join("4001-Bash.request.json");
        let req = serde_json::json!({
            "id": "4001-Bash",
            "tool_name": "Bash",
            "summary": "cargo check",
            "timestamp": crate::agent_sessions::unix_timestamp_secs(),
        });
        std::fs::write(&req_path, req.to_string()).unwrap();
        let _ = st.poll_live_agent_bridge();
        assert!(st.needs_approval());
        assert_eq!(st.pending_approvals.len(), 1);

        // Open a second tab, give it its own per-tab pending approval, and close Tab 2:
        // Tab 2's request in approvals/tab-<tab2_id>/ must be withdrawn while Tab 1's request in approvals/tab-<tab1_id>/ survives.
        let _ = st.spawn_tab(AgentCliPreset::Shell, None);
        assert_eq!(st.tabs.len(), 2);
        let tab2_id = st.tabs[1].id;
        let tab2_approvals_dir = crate::agent_permissions::tab_approvals_dir(&st.session_dir, tab2_id);
        std::fs::create_dir_all(&tab2_approvals_dir).unwrap();
        let tab2_req_path = tab2_approvals_dir.join("4002-Bash.request.json");
        let tab2_ans_path = tab2_approvals_dir.join("4002-Bash.answer");
        let tab2_req = serde_json::json!({
            "id": "4002-Bash",
            "tool_name": "Bash",
            "summary": "rm -rf target",
            "timestamp": crate::agent_sessions::unix_timestamp_secs(),
        });
        std::fs::write(&tab2_req_path, tab2_req.to_string()).unwrap();
        let _ = st.poll_live_agent_bridge();
        st.close_tab(1);
        assert_eq!(st.tabs.len(), 1);
        assert!(tab2_ans_path.exists(), "Closing Tab 2 must withdraw Tab 2's per-tab request");
        assert!(!tab1_approvals_dir.join("4001-Bash.answer").exists(), "Closing Tab 2 must not withdraw Tab 1's per-tab request");
        assert!(req_path.exists(), "Closing Tab 2 must not delete Tab 1's .request.json");
        assert!(st.needs_approval(), "Tab 1 must still need approval after closing Tab 2");
        assert_eq!(st.pending_approvals.len(), 1);

        // Open an exiting stub agent in Tab 2 and let it exit: Tab 1's per-tab pending approval must STILL survive.
        let env = st.session_env();
        let stub = TerminalTab::spawn_exiting_stub_agent(2, 24, 80, None, Some(&env)).expect("spawn stub tab 2");
        st.tabs.push(stub);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        while st.tabs.get(1).is_some_and(|t| !t.exited_notice_shown) && std::time::Instant::now() < deadline {
            let _ = st.poll_live_agent_bridge();
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        assert!(!tab1_approvals_dir.join("4001-Bash.answer").exists(), "Tab 2 exiting must not withdraw Tab 1's per-tab request");
        assert!(req_path.exists(), "Tab 2 exiting must not delete Tab 1's .request.json");
        assert!(st.needs_approval(), "Tab 1 must still need approval after Tab 2 exits");
        assert_eq!(st.pending_approvals.len(), 1);

        let _ = std::fs::remove_dir_all(&st.session_dir);
        kill_tabs(&mut st);
    }

    #[test]
    fn skip_mode_and_empty_claude_requests_never_show_approval_card_or_notify() {
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        crate::theme::apply(&ctx);

        let recorder = crate::agent_permissions::RecordingNotifier::default();
        let mut st = TerminalState::default();
        st.bind_session(&format!("656-skip-guard-{}", std::process::id()));
        st.set_approval_notifier(Arc::new(recorder.clone()));
        st.set_window_focused(false);
        st.ensure_default_tab(None);
        if let Some(tab) = st.active_tab_mut() {
            tab.preset = AgentCliPreset::Claude;
            *tab.agent_status.lock().unwrap() = AgentTabStatus::WaitingForInput(Some("Approval requested".into()));
        }

        // In default Skip mode without any .request.json, needs_approval() must be false and no notification sent.
        assert!(!st.needs_approval(), "Skip mode without request file must not need approval");
        let _ = st.poll_live_agent_bridge();
        assert!(recorder.recorded_events().is_empty(), "Skip mode heuristic must not fire OS notifications");

        // Even if paint_approval_card is called on a Claude tab with empty requests, it must return None and paint nothing.
        let panel_rect = Rect::from_min_size(Pos2::new(48.0, 0.0), Vec2::new(300.0, 800.0));
        let mut card_act = None;
        let out = ctx.run_ui(egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1600.0, 900.0))), ..Default::default() }, |ui| {
            card_act = crate::agent_approval_card::paint_approval_card(ui, panel_rect, AgentCliPreset::Claude, &[], "test-empty-claude");
        });
        assert!(card_act.is_none());
        let (texts, _) = crate::lint::scan_shapes(&out.shapes);
        assert!(texts.is_empty(), "Empty-requests Claude card must not paint broken buttons: {texts:?}");

        let _ = std::fs::remove_dir_all(&st.session_dir);
        kill_tabs(&mut st);
    }

    #[test]
    fn resumed_tab_preserves_login_and_install_fix_actions() {
        let mut st = TerminalState::default();
        st.bind_session(&format!("656-resumed-fix-{}", std::process::id()));
        let env = st.session_env();
        let mut cmd = build_agent_shell_cmd(&default_shell_program(), "sh -c 'echo \"Not logged in · Please run /login\"; exit 1'");
        isolate_zsh_from_home(&mut cmd);
        let mut tab = TerminalTab::spawn_with_session_cmd(1, AgentCliPreset::Claude, 24, 80, None, Some(&env), Some(cmd)).expect("spawn resumed tab");
        tab.is_resumed = true;
        st.tabs.push(tab);

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        while st.tabs[0].start_failure.is_none() && std::time::Instant::now() < deadline {
            let _ = st.poll_live_agent_bridge();
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        let failure = st.tabs[0].start_failure.as_ref().expect("start_failure recorded");
        assert_eq!(
            failure.fix,
            FixAction::RunLogin("claude auth login"),
            "Resumed tab failing due to expired login must offer RunLogin, not overwrite with StartNewSession"
        );

        let _ = std::fs::remove_dir_all(&st.session_dir);
        kill_tabs(&mut st);
    }

    #[test]
    fn export_selection_paste_does_not_race_under_parallel_stress() {
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(8));
        let mut handles = Vec::new();
        for tid in 0..8usize {
            let barrier = std::sync::Arc::clone(&barrier);
            handles.push(std::thread::spawn(move || {
                let mut st = TerminalState::default();
                barrier.wait();
                for iter in 0..10usize {
                    let node_name = format!("x'; touch /tmp/never-t{tid}-i{iter} #");
                    let comment = format!("see $(touch /tmp/never-t{tid}-i{iter})");
                    let (doc, page, fid) = injection_doc(&node_name, &comment);
                    let written_path = export_selection_and_comments_to_pty(&mut st, &doc, page, &[fid], None).expect("export selection");
                    assert_eq!(written_path, st.selection_spec_path);
                    let json = std::fs::read_to_string(&st.selection_spec_path).expect("selection.spec.json");
                    let value: serde_json::Value = serde_json::from_str(&json).expect("json");
                    let ctx = value.get("designos_context").expect("designos_context");
                    assert_eq!(ctx["node_name"].as_str(), Some(node_name.as_str()));
                    let comments = ctx["open_comments"].as_array().expect("open_comments");
                    assert!(
                        comments.iter().any(|c| c.as_str().is_some_and(|s| s.contains(&comment))),
                        "open_comments must contain thread {tid} iter {iter} comment: {comments:?}"
                    );
                }
                let _ = std::fs::remove_dir_all(&st.session_dir);
            }));
        }
        for h in handles {
            h.join().expect("stress worker must not panic");
        }
    }

    #[test]
    fn a_late_stop_failure_does_not_overwrite_the_stopped_receipt() {
        let mut st = isolated_683_state(68462);
        let idx = spawn_mock_agent_tab(&mut st, AgentCliPreset::Claude, Some("sid-stop-failure"));
        let events_path = st.session_dir.join("agent-events.jsonl");
        std::fs::write(&events_path, "{\"t\":400.0,\"e\":\"UserPromptSubmit\",\"p\":{\"session_id\":\"sid-stop-failure\"}}\n").unwrap();
        let _ = st.poll_live_agent_bridge();

        *st.tabs[idx].agent_status.lock().unwrap() = AgentTabStatus::Stopped;
        st.tabs[idx].turn_tracker.stop_by_user(400.5);

        let mut f = std::fs::OpenOptions::new().append(true).open(&events_path).unwrap();
        std::io::Write::write_all(&mut f, b"{\"t\":401.0,\"e\":\"StopFailure\",\"p\":{\"session_id\":\"sid-stop-failure\",\"error\":\"rate_limit\"}}\n")
            .unwrap();
        let _ = st.poll_live_agent_bridge();

        assert_eq!(st.tabs[idx].effective_status(), AgentTabStatus::Stopped, "a late StopFailure must not turn the stopped tab into an error");
        assert_eq!(
            st.tabs[idx].turn_tracker.last_turn_receipt.as_ref().map(|r| r.outcome.clone()),
            Some(crate::agent_turn_status::TurnOutcome::StoppedByUser),
            "the stopped receipt stays"
        );
        kill_tabs(&mut st);
        let _ = std::fs::remove_dir_all(&st.session_dir);
    }

    #[test]
    fn tool_events_in_flight_after_a_stop_do_not_revive_the_tab_until_a_new_prompt() {
        let mut st = isolated_683_state(68461);
        let idx = spawn_mock_agent_tab(&mut st, AgentCliPreset::Claude, Some("sid-stopped"));
        let events_path = st.session_dir.join("agent-events.jsonl");
        std::fs::write(&events_path, "{\"t\":400.0,\"e\":\"UserPromptSubmit\",\"p\":{\"session_id\":\"sid-stopped\"}}\n").unwrap();
        let _ = st.poll_live_agent_bridge();
        assert!(matches!(st.tabs[idx].effective_status(), AgentTabStatus::Working(_)));

        // The user stops the turn from the panel: the tab is latched `Stopped`.
        *st.tabs[idx].agent_status.lock().unwrap() = AgentTabStatus::Stopped;
        st.tabs[idx].turn_tracker.stop_by_user(400.5);

        let append = |line: &str| {
            let mut f = std::fs::OpenOptions::new().append(true).open(&events_path).unwrap();
            std::io::Write::write_all(&mut f, line.as_bytes()).unwrap();
        };
        append("{\"t\":401.0,\"e\":\"PostToolUse\",\"p\":{\"session_id\":\"sid-stopped\",\"tool_name\":\"mcp__designos__designos_apply_spec\"}}\n");
        let _ = st.poll_live_agent_bridge();
        assert_eq!(st.tabs[idx].effective_status(), AgentTabStatus::Stopped, "a straggling tool event must not turn the stopped tab back to Working");
        assert!(!st.tabs[idx].turn_tracker.in_turn, "nor start a phantom turn");

        append("{\"t\":402.0,\"e\":\"UserPromptSubmit\",\"p\":{\"session_id\":\"sid-stopped\"}}\n");
        let _ = st.poll_live_agent_bridge();
        assert!(matches!(st.tabs[idx].effective_status(), AgentTabStatus::Working(_)), "a new prompt starts a new turn");
        kill_tabs(&mut st);
        let _ = std::fs::remove_dir_all(&st.session_dir);
    }

    #[test]
    fn submit_chat_prompt_appends_references_and_prunes_deleted_mentions() {
        let mut st = TerminalState::for_tab(65502);
        st.spawn_tab(AgentCliPreset::Shell, None).expect("spawn");
        {
            let tab = st.active_tab_mut().unwrap();
            tab.spawn_preset = AgentCliPreset::Claude;
            tab.preset = AgentCliPreset::Claude;
        }
        st.active_mentions.push(crate::agent_mention::Mention {
            label: "Hero/Title".to_string(),
            kind: crate::agent_mention::MentionKind::Node,
            id: "4521".to_string(),
            name: "Hero/Title".to_string(),
            page: Some("Landing".to_string()),
        });
        st.active_mentions.push(crate::agent_mention::Mention {
            label: "Deleted/Layer".to_string(),
            kind: crate::agent_mention::MentionKind::Node,
            id: "9999".to_string(),
            name: "Deleted/Layer".to_string(),
            page: Some("Landing".to_string()),
        });
        st.submit_chat_prompt("Check @Hero/Title and @Hero/Title spacing", None);
        let written = String::from_utf8_lossy(&st.active_tab().unwrap().written_bytes).into_owned();
        assert!(written.contains("References: @Hero/Title = designos:node/4521"), "the live session gets the reference footer: {written:?}");
        assert!(!written.contains("9999"), "deleted mention must not be included in sent prompt");
        let ref_count = written.lines().filter(|l| l.contains("References:")).count();
        assert_eq!(ref_count, 1, "duplicate @Hero/Title must be listed once");
        assert!(st.active_mentions.is_empty(), "a sent prompt consumes its mentions");

        kill_tabs(&mut st);
        let _ = std::fs::remove_dir_all(&st.session_dir);
    }

    #[test]
    fn mention_pick_in_shell_tab_sends_zero_bytes_keeps_popup_and_claude_tab_receives_reference() {
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        crate::theme::apply(&ctx);
        let (doc, page, ids) = make_three_layer_doc();
        let mut st = TerminalState::for_tab(65503);
        st.spawn_tab(AgentCliPreset::Shell, None).expect("spawn shell tab");
        let rec = attach_recorder(st.active_tab_mut().unwrap());
        st.sync_live_file_context(&doc, page, &[], None);
        st.mention_popup.toggle_terminal_mode_for("agents-left-pty", &doc, page);
        assert!(st.mention_popup.is_open());

        let panel_rect = Rect::from_min_size(Pos2::new(48.0, 0.0), Vec2::new(300.0, 600.0));
        let enter_raw = || egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1440.0, 900.0))),
            events: vec![egui::Event::Key { key: Key::Enter, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::NONE }],
            ..Default::default()
        };

        // 1) Active tab is a Shell tab: Enter must write zero bytes, keep popup open, and set status.
        let _ = ctx.run_ui(enter_raw(), |ui| {
            crate::agent_selection_chip::with_selection_summary(&doc, page, &[], |summary| {
                let _ = show_left_with_doc(&mut st, ui, panel_rect, None, summary, &doc, page);
            });
        });
        assert!(rec.lock().unwrap().is_empty(), "Shell tab must receive zero bytes when picking a mention");
        assert!(st.mention_popup.is_open(), "popup must stay open when no agent tab accepted the paste");
        assert_eq!(st.status_msg.as_deref(), Some("No agent tab open - nothing sent"));

        // 2) Active tab is a live Claude tab: Enter pastes `@A (designos:node/<id>)` and closes popup.
        st.active_tab_mut().unwrap().spawn_preset = AgentCliPreset::Claude;
        let _ = ctx.run_ui(enter_raw(), |ui| {
            crate::agent_selection_chip::with_selection_summary(&doc, page, &[], |summary| {
                let _ = show_left_with_doc(&mut st, ui, panel_rect, None, summary, &doc, page);
            });
        });
        assert!(!st.mention_popup.is_open(), "popup must close once live Claude tab accepts the paste");
        let bytes = rec.lock().unwrap().clone();
        let pasted = String::from_utf8_lossy(&bytes);
        let expected_ref = format!("@A (designos:node/{})", ids[0].0);
        assert!(pasted.contains(&expected_ref), "expected live Claude tab to receive {expected_ref}, got {pasted:?}");

        kill_tabs(&mut st);
        let _ = std::fs::remove_dir_all(&st.session_dir);
    }

    #[test]
    fn at_mention_typed_before_trailing_words_opens_popup_and_replaces_only_that_token() {
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        crate::theme::apply(&ctx);
        let (doc, page, _ids) = make_three_layer_doc();
        let mut st = TerminalState::for_tab(65504);
        st.side_panel_pty_open = false;
        st.sync_live_file_context(&doc, page, &[], None);
        st.set_composer_draft("make @a red");

        let input_id = Id::new("agents-ask-for-changes-input");
        let mut te_state = egui::widgets::text_edit::TextEditState::default();
        te_state.cursor.set_char_range(Some(egui::text::CCursorRange::one(egui::text::CCursor::new(7))));
        te_state.store(&ctx, input_id);
        ctx.memory_mut(|m| m.request_focus(input_id));

        let panel_rect = Rect::from_min_size(Pos2::new(48.0, 0.0), Vec2::new(320.0, 600.0));
        let raw0 = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1440.0, 900.0))), ..Default::default() };
        let _ = ctx.run_ui(raw0, |ui| {
            crate::agent_selection_chip::with_selection_summary(&doc, page, &[], |summary| {
                let _ = show_left_with_doc(&mut st, ui, panel_rect, None, summary, &doc, page);
            });
        });
        assert!(st.mention_popup.is_open(), "@a typed before trailing words must open the mention popup");
        assert_eq!(st.mention_popup.query, "a");

        // Press Enter in the next frame -> replaces only `@a` with `@A` and preserves trailing `red`.
        let raw_enter = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1440.0, 900.0))),
            events: vec![egui::Event::Key { key: Key::Enter, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::NONE }],
            ..Default::default()
        };
        let _ = ctx.run_ui(raw_enter, |ui| {
            crate::agent_selection_chip::with_selection_summary(&doc, page, &[], |summary| {
                let _ = show_left_with_doc(&mut st, ui, panel_rect, None, summary, &doc, page);
            });
        });
        assert!(
            !st.mention_popup.is_open(),
            "Enter must close the popup after replacing the token: input={:?}, cursor={:?}, popup={:?}",
            st.composer_draft(),
            st.composer_cursor_byte,
            st.mention_popup
        );
        assert_eq!(st.composer_draft(), "make @A red");
        assert_eq!(st.active_mentions.len(), 1);
        assert_eq!(st.active_mentions[0].label, "A");

        kill_tabs(&mut st);
        let _ = std::fs::remove_dir_all(&st.session_dir);
    }

    #[test]
    fn mention_popup_renders_only_on_the_surface_that_opened_it() {
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        crate::theme::apply(&ctx);
        let (doc, page, ids) = make_three_layer_doc();
        let mut st = TerminalState::for_tab(65505);
        st.spawn_tab(AgentCliPreset::Shell, None).expect("spawn tab");
        st.active_tab_mut().unwrap().spawn_preset = AgentCliPreset::Claude;
        let rec = attach_recorder(st.active_tab_mut().unwrap());
        st.sync_live_file_context(&doc, page, &[], None);

        let left_rect = Rect::from_min_size(Pos2::new(48.0, 0.0), Vec2::new(280.0, 600.0));
        let dock_rect = Rect::from_min_size(Pos2::new(340.0, 500.0), Vec2::new(900.0, 360.0));

        // Frame 1: paint both left PTY and dock surface to record `agent-mention-btn-dock`.
        let raw0 = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1440.0, 900.0))), ..Default::default() };
        let _ = ctx.run_ui(raw0, |ui| {
            crate::agent_selection_chip::with_selection_summary(&doc, page, &[], |summary| {
                let _ = show_left_with_doc(&mut st, ui, left_rect, None, summary, &doc, page);
                let _ = render_terminal_surface(&mut st, ui, dock_rect, None, "agents-dock", summary, &doc, page);
            });
        });
        let dock_btn: Rect = ctx.data(|d| d.get_temp(Id::new(("test-rect", "agent-mention-btn-dock".to_string())))).expect("dock mention button rect");

        // Frame 2: click `agent-mention-btn-dock` while both surfaces are rendered.
        let click_ev = |pos: Pos2| egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1440.0, 900.0))),
            events: vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE },
                egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE },
            ],
            ..Default::default()
        };
        let _ = ctx.run_ui(click_ev(dock_btn.center()), |ui| {
            crate::agent_selection_chip::with_selection_summary(&doc, page, &[], |summary| {
                let _ = show_left_with_doc(&mut st, ui, left_rect, None, summary, &doc, page);
                let _ = render_terminal_surface(&mut st, ui, dock_rect, None, "agents-dock", summary, &doc, page);
            });
        });
        assert!(st.mention_popup.is_open(), "clicking dock @ Mention must open popup");

        // Frame 3: paint so dock popup records `agent-mention-row-0` inside `dock_rect`, then click it.
        let raw1 = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1440.0, 900.0))), ..Default::default() };
        let _ = ctx.run_ui(raw1, |ui| {
            crate::agent_selection_chip::with_selection_summary(&doc, page, &[], |summary| {
                let _ = show_left_with_doc(&mut st, ui, left_rect, None, summary, &doc, page);
                let _ = render_terminal_surface(&mut st, ui, dock_rect, None, "agents-dock", summary, &doc, page);
            });
        });
        let row0: Rect = ctx.data(|d| d.get_temp(Id::new(("test-rect", "agent-mention-row-0".to_string())))).expect("row 0 rect");
        assert!(dock_rect.contains(row0.center()), "popup must render inside the dock surface that opened it, got {row0:?}");

        let _ = ctx.run_ui(click_ev(row0.center()), |ui| {
            crate::agent_selection_chip::with_selection_summary(&doc, page, &[], |summary| {
                let _ = show_left_with_doc(&mut st, ui, left_rect, None, summary, &doc, page);
                let _ = render_terminal_surface(&mut st, ui, dock_rect, None, "agents-dock", summary, &doc, page);
            });
        });
        assert!(!st.mention_popup.is_open(), "clicking a row in the dock popup must pick and close");
        let pasted = String::from_utf8_lossy(&rec.lock().unwrap()).to_string();
        assert!(pasted.contains(&format!("@A (designos:node/{})", ids[0].0)), "dock row click must paste mention into Claude tab, got {pasted:?}");

        kill_tabs(&mut st);
        let _ = std::fs::remove_dir_all(&st.session_dir);
    }

    fn permission_window_root(tag: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("dos-653-perm-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("create library root");
        root
    }

    /// Another window (its own process) switches the library to Ask: this window's next agent tab honours it.
    #[test]
    fn new_agent_tab_honours_the_permission_mode_another_window_saved() {
        use crate::agent_permissions::{save_permission_mode, AgentPermissionMode};
        let root = permission_window_root("other-window");
        let mut window_a = TerminalState::for_tab(1);
        window_a.bind_library_file(&root, "file-a", "win-a");
        let mut window_b = TerminalState::for_tab(2);
        window_b.bind_library_file(&root, "file-b", "win-b");
        assert_eq!(window_b.permission_mode, AgentPermissionMode::Skip, "window B cached the default when it bound");

        window_a.permission_mode = AgentPermissionMode::Ask;
        save_permission_mode(&root, AgentPermissionMode::Ask).expect("window A persists Ask");

        let idx = window_b.spawn_tab(AgentCliPreset::Shell, None).expect("spawn tab in window B");
        assert_eq!(window_b.permission_mode, AgentPermissionMode::Ask, "the spawn re-reads the saved mode");
        assert_eq!(window_b.tabs[idx].permission_mode, AgentPermissionMode::Ask, "the new tab runs in Ask mode");

        kill_tabs(&mut window_b);
        window_a.shutdown();
        window_b.shutdown();
        let _ = std::fs::remove_dir_all(&root);
    }

    /// No saved mode (or no library): a mode set in memory keeps applying to new tabs.
    #[test]
    fn new_agent_tab_keeps_the_in_memory_mode_when_nothing_is_saved() {
        use crate::agent_permissions::AgentPermissionMode;
        let root = permission_window_root("no-saved-mode");
        let mut bound = TerminalState::for_tab(3);
        bound.bind_library_file(&root, "file-c", "win-c");
        bound.permission_mode = AgentPermissionMode::Ask;
        assert!(!root.join("settings.json").exists());
        let idx = bound.spawn_tab(AgentCliPreset::Shell, None).expect("spawn bound tab");
        assert_eq!(bound.tabs[idx].permission_mode, AgentPermissionMode::Ask);
        kill_tabs(&mut bound);
        bound.shutdown();

        let mut detached = isolated_683_state(68490);
        detached.permission_mode = AgentPermissionMode::Ask;
        let idx = detached.spawn_tab(AgentCliPreset::Shell, None).expect("spawn detached tab");
        assert_eq!(detached.tabs[idx].permission_mode, AgentPermissionMode::Ask);
        kill_tabs(&mut detached);
        let _ = std::fs::remove_dir_all(&detached.session_dir);
        let _ = std::fs::remove_dir_all(&root);
    }

    /// Restores write access to a library root on drop, so a failed assertion cannot leave a folder the temp cleaner cannot delete.
    #[cfg(unix)]
    struct ReadOnlyRoot(PathBuf);

    #[cfg(unix)]
    impl ReadOnlyRoot {
        fn lock(root: &Path) -> Self {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(root, std::fs::Permissions::from_mode(0o555)).expect("make the root read-only");
            Self(root.to_path_buf())
        }
    }

    #[cfg(unix)]
    impl Drop for ReadOnlyRoot {
        fn drop(&mut self) {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&self.0, std::fs::Permissions::from_mode(0o755));
        }
    }

    /// The user picks Ask but `settings.json` cannot be written: the agent about to start must still run in Ask,
    /// not fall back to the Skip that is still on disk.
    #[cfg(unix)]
    #[test]
    fn a_failed_mode_save_keeps_ask_for_the_next_agent_tab() {
        use crate::agent_permissions::{save_permission_mode, AgentPermissionMode};
        let root = permission_window_root("failed-save");
        let mut st = TerminalState::for_tab(5);
        st.bind_library_file(&root, "file-e", "win-e");
        save_permission_mode(&root, AgentPermissionMode::Skip).expect("Skip is saved while the root is writable");
        {
            let _read_only = ReadOnlyRoot::lock(&root);
            st.permission_mode = AgentPermissionMode::Ask;
            if save_permission_mode(&root, AgentPermissionMode::Ask).is_ok() {
                eprintln!("the root stayed writable (running as root?): nothing to prove here");
                return;
            }
            let idx = st.spawn_tab(AgentCliPreset::Shell, None).expect("spawn tab");
            assert_eq!(st.permission_mode, AgentPermissionMode::Ask, "the unsaved Ask survives the spawn");
            assert_eq!(st.tabs[idx].permission_mode, AgentPermissionMode::Ask, "the new tab runs in Ask mode");
            kill_tabs(&mut st);
            st.shutdown();
        }
        assert_eq!(
            crate::agent_permissions::load_saved_permission_mode(&root),
            Some(AgentPermissionMode::Ask),
            "the unsaved choice is what the next window of this process reads"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    /// The picker path: the app setter tells the user the save failed, the unsaved Ask still reaches the next
    /// agent tab and a project tab opened afterwards, and a later successful save clears the memory of it.
    #[cfg(unix)]
    #[test]
    fn the_app_setter_reports_a_failed_save_and_keeps_ask_until_a_save_works() {
        use crate::agent_permissions::{save_permission_mode, AgentPermissionMode};
        let root = permission_window_root("app-setter");
        let mut app = crate::app::App::new(designos_doc::Document::new("Scratch"));
        app.lib = Some(designos_library::Library::open(&root).expect("open library"));
        app.terminal.bind_library_file(&root, "file-f", "win-f");
        save_permission_mode(&root, AgentPermissionMode::Skip).expect("Skip is saved while the root is writable");

        let read_only = ReadOnlyRoot::lock(&root);
        if std::fs::write(root.join("probe"), b"x").is_ok() {
            eprintln!("the root stayed writable (running as root?): nothing to prove here");
            return;
        }
        app.set_agent_permission_mode(AgentPermissionMode::Ask);
        assert_eq!(app.terminal.status_msg.as_deref(), Some("Mode not saved. Check folder permissions."));
        let idx = app.terminal.spawn_tab(AgentCliPreset::Shell, None).expect("spawn tab");
        assert_eq!(app.terminal.tabs[idx].permission_mode, AgentPermissionMode::Ask, "the active window keeps Ask");

        let mut opened_later = TerminalState::for_tab(6);
        opened_later.bind_library_file(&root, "file-g", "win-g");
        assert_eq!(opened_later.permission_mode, AgentPermissionMode::Ask, "a project tab opened later reads the unsaved Ask, not the Skip on disk");

        drop(read_only);
        app.set_agent_permission_mode(AgentPermissionMode::Ask);
        assert_eq!(crate::comments::settings::load_choice(&root, crate::agent_permissions::SETTING_KEY).as_deref(), Some("ask"), "the retry reaches the disk");
        save_permission_mode(&root, AgentPermissionMode::Skip).expect("Skip is saved again");
        let mut after = TerminalState::for_tab(7);
        after.bind_library_file(&root, "file-h", "win-h");
        assert_eq!(after.permission_mode, AgentPermissionMode::Skip, "a successful save clears the unsaved mode");

        kill_tabs(&mut app.terminal);
        app.terminal.shutdown();
        opened_later.shutdown();
        after.shutdown();
        let _ = std::fs::remove_dir_all(&root);
    }

    /// A mode set through the normal path (in memory and saved) wins on the next spawn.
    #[test]
    fn new_agent_tab_uses_the_mode_set_and_saved_in_this_window() {
        use crate::agent_permissions::{save_permission_mode, AgentPermissionMode};
        let root = permission_window_root("same-window");
        let mut st = TerminalState::for_tab(4);
        st.bind_library_file(&root, "file-d", "win-d");
        st.permission_mode = AgentPermissionMode::Ask;
        save_permission_mode(&root, AgentPermissionMode::Ask).expect("save Ask");
        let idx = st.spawn_tab(AgentCliPreset::Shell, None).expect("spawn tab");
        assert_eq!(st.tabs[idx].permission_mode, AgentPermissionMode::Ask);
        st.permission_mode = AgentPermissionMode::Skip;
        save_permission_mode(&root, AgentPermissionMode::Skip).expect("save Skip");
        let idx = st.spawn_tab(AgentCliPreset::Shell, None).expect("spawn second tab");
        assert_eq!(st.tabs[idx].permission_mode, AgentPermissionMode::Skip);
        kill_tabs(&mut st);
        st.shutdown();
        let _ = std::fs::remove_dir_all(&root);
    }
}
