//! Stable per-file agent session folders, lockfiles, `sessions.json` registry, and CLI
//! new-session / resume command builders (`#653`).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::terminal::AgentCliPreset;

/// Status message shown when another live Studio window already holds `session.lock` for this file.
pub const ANOTHER_WINDOW_OWNS_SESSION_MSG: &str = "This file's agents run in another window. Switch to it.";

/// How a `TerminalState` is bound to disk.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SessionKey {
    /// Bound to `<lib_root>/agent-sessions/<file_id>/`: `file_id` is the library file id, or
    /// `path-<hex>` ([`disk_file_session_key`]) for a file opened from disk.
    LibraryFile { lib_root: PathBuf, file_id: String },
    /// Ephemeral fallback at `<data dir>/designos/sessions/<session_id>`.
    Fallback(String),
}

/// Lifecycle state of one recorded CLI session in `sessions.json`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionEntryStatus {
    Active,
    Exited,
    Missing,
}

/// One CLI session entry inside `<session_dir>/sessions.json`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionEntry {
    pub tab_id: u64,
    pub title: String,
    pub cli: String,
    pub session_id: Option<String>,
    pub created_at: u64,
    pub last_active_at: u64,
    pub status: SessionEntryStatus,
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

/// Persistent session registry stored at `<session_dir>/sessions.json`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionRegistry {
    pub version: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_cli: Option<String>,
    #[serde(default)]
    pub sessions: Vec<SessionEntry>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

impl Default for SessionRegistry {
    fn default() -> Self {
        Self { version: 1, last_cli: None, sessions: Vec::new(), extra: BTreeMap::new() }
    }
}

/// Default tab titles a CLI's sessions were recorded with before its tab title changed, as `(cli, old name)`.
/// Claude tabs were named `Claude CLI #<n>` until they were named `Claude Code #<n>` like the CLI is everywhere else.
const LEGACY_DEFAULT_TITLE_NAMES: &[(&str, &str)] = &[("claude", "Claude CLI")];

/// The current default title for a session recorded with its CLI's old default title, else `None`.
fn current_default_title(cli: &str, title: &str) -> Option<String> {
    let (name, number) = title.rsplit_once(" #")?;
    if number.is_empty() || !number.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let cli = cli.trim();
    LEGACY_DEFAULT_TITLE_NAMES.iter().find(|(legacy_cli, legacy_name)| *legacy_cli == cli && *legacy_name == name)?;
    Some(format!("{} #{number}", preset_from_cli_name(cli)?.button_label()))
}

impl SessionRegistry {
    /// Renames every entry that still has the old default title of its CLI (`Claude CLI #3`) to the current default
    /// (`Claude Code #3`), and reports whether any entry changed. A title the user typed never matches: only the
    /// exact `<old name> #<digits>` of the entry's own CLI does.
    pub fn rename_legacy_default_titles(&mut self) -> bool {
        let mut changed = false;
        for entry in &mut self.sessions {
            if let Some(title) = current_default_title(&entry.cli, &entry.title) {
                entry.title = title;
                changed = true;
            }
        }
        changed
    }

    /// Finds the mutable [`SessionEntry`] matching `session_uuid` when present, falling back to
    /// matching `tab_id` only for entries without a `session_id` (`None`).
    pub fn find_entry_mut(&mut self, tab_id: u64, session_uuid: Option<&str>) -> Option<&mut SessionEntry> {
        let clean_uuid = session_uuid.map(str::trim).filter(|s| !s.is_empty());
        if let Some(uuid) = clean_uuid {
            self.sessions.iter_mut().find(|e| e.session_id.as_deref() == Some(uuid))
        } else {
            self.sessions.iter_mut().find(|e| e.session_id.is_none() && e.tab_id == tab_id)
        }
    }
}

/// Single-owner lockfile stored at `<session_dir>/session.lock`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionLock {
    pub pid: u32,
    pub window: String,
    pub started_at: u64,
}

/// Outcome of attempting to acquire `<lib_root>/agent-sessions/<file_id>/session.lock`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LockOutcome {
    /// This window acquired (or already owned / took over a stale) `session.lock`.
    Acquired(PathBuf),
    /// Another live window owns `session.lock`; caller must use the fallback `<data dir>/designos/sessions/...` dir.
    Fallback { dir: PathBuf, message: String },
}

/// Current Unix epoch timestamp in seconds.
pub fn unix_timestamp_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// Generates a random RFC 4122 UUID v4 string (`xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx`) using OS entropy
/// (`/dev/urandom` on Unix, mixed with high-resolution clock + atomic counter fallback).
pub fn generate_session_uuid() -> String {
    let mut bytes = [0u8; 16];
    let mut filled = false;
    #[cfg(unix)]
    {
        use std::io::Read;
        if let Ok(mut f) = std::fs::File::open("/dev/urandom") {
            if f.read_exact(&mut bytes).is_ok() {
                filled = true;
            }
        }
    }
    if !filled {
        static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        let seq = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(0);
        let pid = std::process::id() as u64;
        let a = nanos ^ seq.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ pid.rotate_left(17);
        let b = seq.rotate_left(29) ^ nanos.wrapping_mul(0xBF58_476D_1CE4_E5B9) ^ pid.wrapping_mul(0x94D0_49BB_1331_11EB);
        bytes[..8].copy_from_slice(&a.to_le_bytes());
        bytes[8..].copy_from_slice(&b.to_le_bytes());
    }
    // RFC 4122 version 4 and variant 1 bits
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        bytes[0],
        bytes[1],
        bytes[2],
        bytes[3],
        bytes[4],
        bytes[5],
        bytes[6],
        bytes[7],
        bytes[8],
        bytes[9],
        bytes[10],
        bytes[11],
        bytes[12],
        bytes[13],
        bytes[14],
        bytes[15]
    )
}

/// Stable session folder path for a library file: `<lib_root>/agent-sessions/<file_id>`.
pub fn stable_session_dir(lib_root: &Path, file_id: &str) -> PathBuf {
    let safe_id: String = file_id.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_').collect();
    lib_root.join("agent-sessions").join(safe_id)
}

/// Session key for a file opened from disk without a library id: `path-<16 hex digits>`, the
/// FNV-1a 64-bit hash of its canonical path (the path itself when it cannot be canonicalised).
/// FNV-1a is spelled out here, not taken from `DefaultHasher`, because the key names a folder that
/// must keep its name across Studio and Rust upgrades. Renaming or moving the file changes the key
/// and so starts a fresh history (`#653` rule 2). The result only uses `[0-9a-z-]`, so
/// [`stable_session_dir`] keeps it as is.
pub fn disk_file_session_key(path: &Path) -> String {
    let canonical = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    format!("path-{:016x}", fnv1a_64(canonical.as_os_str().as_encoded_bytes()))
}

/// FNV-1a 64-bit hash of `bytes`. Spelled out, not taken from `DefaultHasher`, for names that must stay the same
/// across Rust upgrades.
pub fn fnv1a_64(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// What [`crate::terminal::TerminalState::bind_open_file`] last bound: its inputs and the session
/// key they produced (the library file id, or the `path-<hex>` key of the disk path).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpenFileBinding {
    pub lib_root: PathBuf,
    pub lib_file_id: Option<String>,
    pub disk_path: Option<PathBuf>,
    pub window_id: String,
    pub file_id: String,
}

impl OpenFileBinding {
    /// Whether a new `bind_open_file` call carries exactly the inputs of this binding.
    pub fn same_inputs(&self, lib_root: &Path, lib_file_id: Option<&str>, disk_path: Option<&Path>, window_id: &str) -> bool {
        self.same_file(lib_root, lib_file_id, disk_path) && self.window_id == window_id
    }

    /// Whether this binding is of the same file, whichever window it was made for.
    pub fn same_file(&self, lib_root: &Path, lib_file_id: Option<&str>, disk_path: Option<&Path>) -> bool {
        self.lib_root == lib_root && self.lib_file_id.as_deref() == lib_file_id && self.disk_path.as_deref() == disk_path
    }
}

/// Interprets `/bin/kill -0 <pid>` exit status and stderr: exit 0 or `EPERM` ("Operation not permitted")
/// means the target process is alive.
pub(crate) fn kill_zero_indicates_alive(success: bool, stderr: &[u8]) -> bool {
    if success {
        return true;
    }
    let msg = String::from_utf8_lossy(stderr).to_ascii_lowercase();
    msg.contains("operation not permitted") || msg.contains("eperm")
}

/// Checks whether OS process `pid` is currently alive.
pub fn is_pid_alive(pid: u32) -> bool {
    if pid == 0 {
        return false;
    }
    if pid == std::process::id() {
        return true;
    }
    #[cfg(unix)]
    {
        // `kill -0 <pid>` succeeds (exit 0) if the process exists and we can signal it;
        // if it fails with EPERM ("Operation not permitted") the process also exists.
        std::process::Command::new("/bin/kill")
            .arg("-0")
            .arg(pid.to_string())
            .output()
            .map(|out| kill_zero_indicates_alive(out.status.success(), &out.stderr))
            .unwrap_or(false)
    }
    #[cfg(not(unix))]
    {
        let _ = pid;
        false
    }
}

fn try_create_lock_exclusively(lock_path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new().write(true).create_new(true).open(lock_path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

fn fallback_lock_outcome(fallback_id: &str) -> LockOutcome {
    fallback_outcome(fallback_id, false, |_| ANOTHER_WINDOW_OWNS_SESSION_MSG.to_string())
}

/// The ephemeral folder Studio keeps for `fallback_id`, with the status line `message(works)` (`works`: the folder can be used).
/// When it cannot be made the line says so too ("either" only when `message` already reports another failure): the tabs that
/// follow stop with the same reason instead of starting in a folder Studio did not make.
fn fallback_outcome(fallback_id: &str, after_a_failure: bool, message: impl FnOnce(bool) -> String) -> LockOutcome {
    let dir = crate::agent_session_files::ephemeral_session_dir(fallback_id);
    let message = match crate::agent_session_files::ensure_session_dir(&dir) {
        Ok(()) => message(true),
        Err(e) => {
            let either = if after_a_failure { " either" } else { "" };
            format!("{} The temporary session folder can't be used{either}: {}.", message(false), crate::agent_session_files::with_next_step(&e))
        }
    };
    LockOutcome::Fallback { dir, message }
}

/// Acquires `<lib_root>/agent-sessions/<file_id>/session.lock` for `window`, taking over stale locks
/// whose `pid` is no longer alive (tested via `pid_alive`) or whose `started_at` does not match the
/// running process, or returns a fallback `<data dir>/designos/sessions/<fallback_id>` directory if another
/// live window already owns the folder.
pub fn acquire_stable_session_dir_with<F>(lib_root: &Path, file_id: &str, window: &str, fallback_id: &str, pid_alive: F) -> LockOutcome
where
    F: Fn(u32) -> bool,
{
    let dir = stable_session_dir(lib_root, file_id);
    // No lock or registry is written into a folder that is a link or belongs to someone else.
    if let Err(e) = crate::agent_session_files::ensure_session_dir(&dir) {
        return fallback_outcome(fallback_id, true, |works| crate::agent_session_files::library_fallback_message(&e, works));
    }
    let lock_path = dir.join("session.lock");
    let my_pid = std::process::id();
    let started_at = (crate::agent_session_files::current_process_started_ms() / 1000).max(1);
    let lock = SessionLock { pid: my_pid, window: window.to_string(), started_at };
    let bytes = serde_json::to_vec_pretty(&lock).unwrap_or_default();

    if try_create_lock_exclusively(&lock_path, &bytes).is_ok() {
        return LockOutcome::Acquired(dir);
    }

    if let Ok(raw) = std::fs::read_to_string(&lock_path) {
        if let Ok(existing) = serde_json::from_str::<SessionLock>(&raw) {
            if existing.pid == my_pid && existing.window == window {
                return LockOutcome::Acquired(dir);
            }
            if pid_alive(existing.pid) && crate::agent_session_files::is_pid_start_time_matching(existing.pid, existing.started_at) {
                return fallback_lock_outcome(fallback_id);
            }
        }
    }

    let _ = std::fs::remove_file(&lock_path);
    if try_create_lock_exclusively(&lock_path, &bytes).is_ok() {
        LockOutcome::Acquired(dir)
    } else {
        fallback_lock_outcome(fallback_id)
    }
}

/// Acquires `<lib_root>/agent-sessions/<file_id>/session.lock` using [`is_pid_alive`].
pub fn acquire_stable_session_dir(lib_root: &Path, file_id: &str, window: &str, fallback_id: &str) -> LockOutcome {
    acquire_stable_session_dir_with(lib_root, file_id, window, fallback_id, is_pid_alive)
}

/// Releases `<session_dir>/session.lock` if it is owned by `( std::process::id(), window )`.
pub fn release_session_lock(session_dir: &Path, window: &str) {
    let lock_path = session_dir.join("session.lock");
    if let Ok(raw) = std::fs::read_to_string(&lock_path) {
        if let Ok(existing) = serde_json::from_str::<SessionLock>(&raw) {
            if existing.pid == std::process::id() && existing.window == window {
                let _ = std::fs::remove_file(&lock_path);
            }
        }
    }
}

/// Loads `<session_dir>/sessions.json`. If the file is missing, returns an empty v1 registry.
/// If the file is corrupted/unreadable JSON, moves it to `<session_dir>/sessions.json.bad` and
/// writes a fresh empty `sessions.json`.
pub fn load_or_init_registry(session_dir: &Path) -> SessionRegistry {
    let path = session_dir.join("sessions.json");
    let Ok(raw) = std::fs::read_to_string(&path) else {
        return SessionRegistry::default();
    };
    match serde_json::from_str::<SessionRegistry>(&raw) {
        Ok(reg) => reg,
        Err(_) => {
            let bad_path = session_dir.join("sessions.json.bad");
            let _ = std::fs::rename(&path, &bad_path);
            let fresh = SessionRegistry::default();
            let _ = save_registry(session_dir, &fresh);
            fresh
        }
    }
}

/// Atomically writes `registry` to `<session_dir>/sessions.json`.
pub fn save_registry(session_dir: &Path, registry: &SessionRegistry) -> std::io::Result<()> {
    let path = session_dir.join("sessions.json");
    let bytes = serde_json::to_vec_pretty(registry).map_err(std::io::Error::other)?;
    atomic_write(&path, &bytes)
}

fn atomic_write(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension(format!("tmp.{}", std::process::id()));
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)
}

/// True for the built-in agent CLIs (`claude`, `codex`, `grok`, `agy`, `gemini`, `opencode`) that can be remembered
/// as `last_cli` in `sessions.json` (`#652` §3 rule 4, `#679` §3). `Shell` and `DesignOsHelp` are excluded.
pub fn is_agent_cli_preset(preset: AgentCliPreset) -> bool {
    matches!(
        preset,
        AgentCliPreset::Claude | AgentCliPreset::Codex | AgentCliPreset::Grok | AgentCliPreset::Antigravity | AgentCliPreset::Gemini | AgentCliPreset::OpenCode
    )
}

/// True only for agent CLIs whose sessions Studio can resume safely (`#653` §3 rules 4 & 6), so the newest entry is
/// always one that can be resumed.
/// - `Codex` is never registered: its `resume --last` takes no session id, so it continues the newest Codex
///   conversation whichever session was picked. Codex is registered once Studio captures a session id and resumes by
///   it (`#926`); verifying the cwd scope (`codex_cwd_scoped`) does not change that.
/// - `Antigravity` (`agy --continue`) is registered once `agy_cwd_scoped` is verified.
/// - `Shell` and `DesignOsHelp` are never registered.
pub fn is_registrable_cli_scoped(preset: AgentCliPreset, agy_cwd_scoped: bool) -> bool {
    match preset {
        AgentCliPreset::Claude | AgentCliPreset::Grok | AgentCliPreset::Gemini | AgentCliPreset::OpenCode => true,
        AgentCliPreset::Antigravity => agy_cwd_scoped,
        AgentCliPreset::Codex | AgentCliPreset::Shell | AgentCliPreset::DesignOsHelp => false,
    }
}

/// Default registrability check when `agy_cwd_scoped` has not been verified.
pub fn is_registrable_cli(preset: AgentCliPreset) -> bool {
    is_registrable_cli_scoped(preset, false)
}

/// True for CLIs that accept `--session-id <uuid>` on new start (`Claude` and `Grok`).
/// `Antigravity`, `Gemini`, and `OpenCode` record `session_id: None` (`Rule 6`); Codex is not recorded.
pub fn uses_preassigned_session_id(preset: AgentCliPreset) -> bool {
    matches!(preset, AgentCliPreset::Claude | AgentCliPreset::Grok)
}

/// Returns `true` iff `s` (after trimming whitespace) is a strict 36-character hex UUID
/// (`xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx` with ASCII hex digits and hyphens).
pub fn is_valid_session_uuid(s: &str) -> bool {
    let bytes = s.trim().as_bytes();
    if bytes.len() != 36 {
        return false;
    }
    for (i, &b) in bytes.iter().enumerate() {
        match i {
            8 | 13 | 18 | 23 => {
                if b != b'-' {
                    return false;
                }
            }
            _ => {
                if !b.is_ascii_hexdigit() {
                    return false;
                }
            }
        }
    }
    true
}

/// Builds the interactive launch command for a brand-new session of `preset`.
/// For `Claude` and `Grok`, `session_uuid` is passed via `--session-id <uuid>` when it is a valid UUID.
pub fn build_new_session_command(preset: AgentCliPreset, session_uuid: Option<&str>) -> String {
    let valid_uuid = session_uuid.map(str::trim).filter(|s| is_valid_session_uuid(s));
    match preset {
        AgentCliPreset::Claude => match valid_uuid {
            Some(uuid) => format!(
                "claude --session-id {uuid} --dangerously-skip-permissions --mcp-config \"$DESIGNOS_MCP_CONFIG\" --settings \"$DESIGNOS_SESSION_DIR/hooks.json\" --append-system-prompt \"You are inside DESIGN:OS Studio bound exclusively to file '$DESIGNOS_DOC_NAME' (session $DESIGNOS_SESSION_DIR). Use the pre-fetched bootstrap context (or call designos_get_app_state) for the current selected page, frames, tokens, and components, and deliver all designs onto that file and page via designos-mcp tools (designos_apply_spec, designos_create_node, designos_update_node) or by writing NodeSpec JSON to $DESIGNOS_LIVE_SPEC.\""
            ),
            None => AgentCliPreset::Claude.cli_launch_command().to_string(),
        },
        AgentCliPreset::Grok => match valid_uuid {
            Some(uuid) => format!("grok --session-id {uuid} --always-approve --cwd \"$DESIGNOS_SESSION_DIR\""),
            None => AgentCliPreset::Grok.cli_launch_command().to_string(),
        },
        AgentCliPreset::Codex
        | AgentCliPreset::Antigravity
        | AgentCliPreset::Gemini
        | AgentCliPreset::OpenCode
        | AgentCliPreset::Shell
        | AgentCliPreset::DesignOsHelp => preset.cli_launch_command().to_string(),
    }
}

/// Maps a recorded `cli` binary string (`"claude"`, `"codex"`, `"grok"`, `"agy"`, `"gemini"`, `"opencode"`) back to [`AgentCliPreset`].
pub fn preset_from_cli_name(cli: &str) -> Option<AgentCliPreset> {
    match cli.trim() {
        "claude" => Some(AgentCliPreset::Claude),
        "codex" => Some(AgentCliPreset::Codex),
        "grok" => Some(AgentCliPreset::Grok),
        "agy" => Some(AgentCliPreset::Antigravity),
        "gemini" => Some(AgentCliPreset::Gemini),
        "opencode" => Some(AgentCliPreset::OpenCode),
        _ => None,
    }
}

/// Builds the CLI resume command for `entry` with explicit `codex_cwd_scoped` and `agy_cwd_scoped` gates (`#653` §3 rule 6, `#679` §3).
/// The gates only decide whether an entry that already exists can be resumed. Codex entries are not recorded today
/// ([`is_registrable_cli_scoped`]), so `codex_cwd_scoped` resumes nothing in practice.
/// Rejects any non-empty `entry.session_id` that is not a valid 36-char UUID (`is_valid_session_uuid`) for Claude/Grok.
pub fn build_resume_command_scoped(entry: &SessionEntry, codex_cwd_scoped: bool, agy_cwd_scoped: bool) -> Option<String> {
    let preset = preset_from_cli_name(&entry.cli)?;
    match preset {
        AgentCliPreset::Claude => {
            let resume_flag = match entry.session_id.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
                Some(uuid) if is_valid_session_uuid(uuid) => format!("--resume {uuid}"),
                Some(_) => return None,
                None => "--continue".to_string(),
            };
            Some(format!(
                "claude {resume_flag} --dangerously-skip-permissions --mcp-config \"$DESIGNOS_MCP_CONFIG\" --settings \"$DESIGNOS_SESSION_DIR/hooks.json\" --append-system-prompt \"You are inside DESIGN:OS Studio bound exclusively to file '$DESIGNOS_DOC_NAME' (session $DESIGNOS_SESSION_DIR). Use the pre-fetched bootstrap context (or call designos_get_app_state) for the current selected page, frames, tokens, and components, and deliver all designs onto that file and page via designos-mcp tools (designos_apply_spec, designos_create_node, designos_update_node) or by writing NodeSpec JSON to $DESIGNOS_LIVE_SPEC.\""
            ))
        }
        AgentCliPreset::Codex => {
            if !codex_cwd_scoped {
                return None;
            }
            let base = AgentCliPreset::Codex.cli_launch_command();
            let overrides = base.strip_prefix("codex --dangerously-bypass-approvals-and-sandbox").unwrap_or("");
            Some(format!("codex resume --last --dangerously-bypass-approvals-and-sandbox{overrides}"))
        }
        AgentCliPreset::Grok => {
            let resume_flag = match entry.session_id.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
                Some(uuid) if is_valid_session_uuid(uuid) => format!("--resume {uuid}"),
                Some(_) => return None,
                None => "--continue".to_string(),
            };
            Some(format!("grok {resume_flag} --always-approve --cwd \"$DESIGNOS_SESSION_DIR\""))
        }
        AgentCliPreset::Antigravity => {
            if !agy_cwd_scoped {
                return None;
            }
            Some("agy --continue --dangerously-skip-permissions --add-dir \"$DESIGNOS_SESSION_DIR\"".to_string())
        }
        AgentCliPreset::Gemini => {
            let resume_target = entry.session_id.as_deref().filter(|s| !s.trim().is_empty()).unwrap_or("latest");
            Some(format!("gemini --resume {resume_target} --approval-mode yolo --include-directories \"$DESIGNOS_SESSION_DIR\""))
        }
        AgentCliPreset::OpenCode => {
            let resume_flag = match entry.session_id.as_deref().filter(|s| !s.trim().is_empty()) {
                Some(id) => format!("--session {id}"),
                None => "--continue".to_string(),
            };
            Some(format!("opencode {resume_flag}"))
        }
        AgentCliPreset::Shell | AgentCliPreset::DesignOsHelp => None,
    }
}

/// Builds the CLI resume command for `entry` according to §6 of `#653`, gating `Codex` and `Antigravity`
/// on `cwd_scoped`.
pub fn build_resume_command(entry: &SessionEntry, cwd_scoped: bool) -> Option<String> {
    build_resume_command_scoped(entry, cwd_scoped, cwd_scoped)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::App;
    use crate::terminal::TerminalState;
    use designos_doc::Document;
    use designos_library::Library;

    fn temp_test_dir(tag: &str) -> PathBuf {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("dos-653-{tag}-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp test dir");
        dir
    }

    /// Waits until the stub's `argv.log` ends in a newline, so the line it logged is whole, and returns the text. The stub's `printf` of a long
    /// command line can reach the file in more than one write. A reader that stops at the first byte can stop the stub half way through its line,
    /// and the next stub's line then joins it.
    fn wait_for_whole_logged_line(path: &Path) -> String {
        let mut text = String::new();
        crate::test_timing::poll_until(crate::test_timing::STUB_WAIT, || {
            text = std::fs::read_to_string(path).unwrap_or_default();
            text.ends_with('\n')
        });
        text
    }

    #[cfg(unix)]
    fn write_executable_script(dir: &Path, name: &str, body: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let p = dir.join(name);
        std::fs::write(&p, body).expect("write script");
        let mut perms = std::fs::metadata(&p).expect("meta").permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&p, perms).expect("chmod");
        p
    }

    /// The tab that holds a file's session folder is found by the file, whichever window id it bound with.
    #[test]
    fn an_open_file_binding_names_its_file_apart_from_the_window() {
        let root = Path::new("/lib");
        let library =
            OpenFileBinding { lib_root: root.to_path_buf(), lib_file_id: Some("f1".into()), disk_path: None, window_id: "w1".into(), file_id: "f1".into() };
        assert!(library.same_inputs(root, Some("f1"), None, "w1"));
        assert!(!library.same_inputs(root, Some("f1"), None, "w2"), "another window id is not the same inputs");
        assert!(library.same_file(root, Some("f1"), None), "but it is the same file");
        assert!(!library.same_file(root, Some("f2"), None));
        assert!(!library.same_file(Path::new("/other"), Some("f1"), None));
        assert!(!library.same_file(root, None, Some(Path::new("/a/b.designos"))));
        let on_disk = OpenFileBinding {
            lib_root: root.to_path_buf(),
            lib_file_id: None,
            disk_path: Some("/a/b.designos".into()),
            window_id: "w1".into(),
            file_id: "path-1".into(),
        };
        assert!(on_disk.same_file(root, None, Some(Path::new("/a/b.designos"))));
        assert!(!on_disk.same_file(root, None, Some(Path::new("/a/c.designos"))));
    }

    // 1. `session_dir_is_keyed_by_library_file_id`
    #[test]
    fn session_dir_is_keyed_by_library_file_id() {
        let root = temp_test_dir("keyed-by-file-id");
        let lib = Library::open(&root).expect("open library");
        let proj = lib.create_project("Drafts").expect("create project").id;
        let created = lib.create_file(&proj, None, "Initial Name", &Document::new("Initial Name")).expect("create file");

        let mut app = App::new(Document::new("Unsaved"));
        let unsaved_sid = app.tabs.active_session_id();
        app.terminal.bind_session(&unsaved_sid);
        assert_eq!(app.terminal.session_dir, crate::agent_session_files::ephemeral_session_dir(&unsaved_sid));

        app.attach_library(root.clone());
        app.open_library_file(&created.id);
        let expected = lib.agent_session_dir(&created.id);
        assert_eq!(app.terminal.session_dir, expected);
        assert!(expected.ends_with(format!("agent-sessions/{}", created.id)));

        // Renaming the file in the library does not move the session folder.
        lib.rename_file(&created.id, "Renamed Design").expect("rename file");
        app.open_library_file(&created.id);
        assert_eq!(app.terminal.session_dir, expected);

        app.terminal.shutdown();
        let _ = std::fs::remove_dir_all(&root);
    }

    /// The session skill's body prints the session folder's paths. None of them may carry text a document or a file can set,
    /// because a backtick, a `!` or a line break in a path would put a shell placeholder into the skill. A library file's
    /// folder keeps only `[A-Za-z0-9_-]` of its id, a disk file's is named by a hash of its path, and a window without a
    /// library file works in a folder named by the process and tab numbers.
    #[test]
    fn a_session_folder_is_never_named_by_a_document_or_file_name() {
        let lib_root = Path::new("/designos-lib");
        let hostile_id = "x` !`touch m` `y\n!`id`\u{2028}../../etc";
        let dir = stable_session_dir(lib_root, hostile_id);
        assert_eq!(dir.parent(), Some(lib_root.join("agent-sessions").as_path()), "the id cannot leave its folder: {}", dir.display());
        let name = dir.file_name().and_then(|n| n.to_str()).expect("a folder name");
        assert!(name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'), "the folder name holds {name:?}");

        let key = disk_file_session_key(Path::new("/tmp/a` !`touch m` b\n!`id`.fig"));
        assert!(key.starts_with("path-") && key["path-".len()..].chars().all(|c| c.is_ascii_hexdigit()), "{key}");

        assert_eq!(crate::terminal::tab_session_id("p123", 4), "p123-tab-4");
    }

    // 1b. `disk_file_session_key_is_a_stable_hash_of_the_canonical_path`
    #[test]
    fn disk_file_session_key_is_a_stable_hash_of_the_canonical_path() {
        let dir = temp_test_dir("disk-key");
        let file = dir.join("brand.designos");
        std::fs::write(&file, b"{}").expect("write file");
        let other = dir.join("other.designos");
        std::fs::write(&other, b"{}").expect("write other");

        let key = disk_file_session_key(&file);
        assert!(key.starts_with("path-"), "key shape: {key}");
        let hex = &key["path-".len()..];
        assert_eq!(hex.len(), 16, "16 lowercase hex digits: {key}");
        assert!(hex.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()), "hex digits only: {key}");
        assert_eq!(key, disk_file_session_key(&file), "deterministic");
        assert_ne!(key, disk_file_session_key(&other), "a different path is a different key (rename/move starts fresh)");
        let dotted = dir.join("sub").join("..").join("brand.designos");
        std::fs::create_dir_all(dir.join("sub")).expect("sub dir");
        assert_eq!(key, disk_file_session_key(&dotted), "the canonical path is hashed, not the spelling");
        // Pinned value: a changed hash would orphan every existing disk-file session after an upgrade.
        assert_eq!(disk_file_session_key(Path::new("/designos-pinned/does-not-exist.fig")), "path-236811fb7f1c15f7");
        let _ = std::fs::remove_dir_all(&dir);
    }

    // 1c. `disk_file_session_dir_survives_a_restart`
    #[test]
    fn disk_file_session_dir_survives_a_restart() {
        let root = temp_test_dir("disk-restart-lib");
        let work = temp_test_dir("disk-restart-files");
        let file = work.join("poster.designos");
        crate::io::save_path(&Document::new("Poster"), &file).expect("save disk file");
        let key = disk_file_session_key(&file);
        let expected = root.join("agent-sessions").join(&key);

        let first_dir = {
            let mut app = App::new(Document::new("Scratch"));
            app.attach_library(root.clone());
            app.open_path(file.clone());
            let mut tabs = std::mem::take(&mut app.tabs);
            tabs.reconcile_frame(&mut app);
            app.tabs = tabs;
            assert_eq!(app.terminal.session_dir, expected, "a disk file lives under agent-sessions/path-<hex>, not the temp folder");
            assert!(expected.join("session.lock").is_file(), "the folder is locked while open");
            app.terminal.shutdown();
            app.terminal.session_dir.clone()
        };

        // A new process: new App, same file, same folder.
        let mut app2 = App::new(Document::new("Scratch"));
        app2.attach_library(root.clone());
        app2.open_path(file.clone());
        let mut tabs = std::mem::take(&mut app2.tabs);
        tabs.reconcile_frame(&mut app2);
        app2.tabs = tabs;
        assert_eq!(app2.terminal.session_dir, first_dir);

        // Renaming (here: another path) starts a fresh history in another folder.
        let moved = work.join("poster-renamed.designos");
        std::fs::rename(&file, &moved).expect("rename");
        app2.terminal.shutdown();
        app2.open_path(moved.clone());
        let mut tabs = std::mem::take(&mut app2.tabs);
        tabs.reconcile_frame(&mut app2);
        app2.tabs = tabs;
        assert_eq!(app2.terminal.session_dir, root.join("agent-sessions").join(disk_file_session_key(&moved)));
        assert_ne!(app2.terminal.session_dir, first_dir);

        app2.terminal.shutdown();
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&work);
    }

    // 1d. an unsaved scratch document (no library id, no disk path) has no stable key
    #[test]
    fn unsaved_scratch_document_keeps_its_per_process_folder() {
        let root = temp_test_dir("scratch-no-key");
        let mut st = TerminalState::for_tab(1);
        st.bind_session("scratch-sid");
        let before = st.session_dir.clone();
        st.bind_open_file(&root, None, None, "scratch-sid");
        assert_eq!(st.session_dir, before);
        assert!(matches!(st.key, SessionKey::Fallback(_)));
        assert!(!root.join("agent-sessions").exists(), "no stable folder is created for a scratch document");
        let _ = std::fs::remove_dir_all(&root);
    }

    // An unchanged open file must cost no filesystem work per frame (#653 spec section 11).
    #[cfg(unix)]
    #[test]
    fn unchanged_disk_path_does_no_filesystem_work_between_frames() {
        let root = temp_test_dir("frame-cache-lib");
        let work = temp_test_dir("frame-cache-files");
        let real = work.join("real");
        std::fs::create_dir_all(&real).expect("create real dir");
        std::os::unix::fs::symlink(&real, work.join("link")).expect("symlink");
        crate::io::save_path(&Document::new("Poster"), &real.join("poster.designos")).expect("save disk file");
        let via_link = work.join("link").join("poster.designos");

        // The library holds Skip (Ask is the default), so a re-read of the Ask saved below would be visible.
        crate::agent_permissions::save_permission_mode(&root, crate::agent_permissions::AgentPermissionMode::Skip).expect("save Skip");
        let mut app = App::new(Document::new("Scratch"));
        app.attach_library(root.clone());
        app.open_path(via_link);
        let mut tabs = std::mem::take(&mut app.tabs);
        tabs.reconcile_frame(&mut app);
        let bound_key = app.terminal.key.clone();
        let bound_dir = app.terminal.session_dir.clone();
        assert!(matches!(bound_key, SessionKey::LibraryFile { .. }));
        assert!(app.terminal.lock_owner.is_some());
        let mode_before = app.terminal.permission_mode;

        // The file vanishes (an external unlink + rename save) and settings.json changes: a frame that
        // touched the filesystem would canonicalise the link path differently or reload the mode.
        std::fs::remove_file(real.join("poster.designos")).expect("remove file");
        crate::agent_permissions::save_permission_mode(&root, crate::agent_permissions::AgentPermissionMode::Ask).expect("save mode");
        for _ in 0..3 {
            tabs.reconcile_frame(&mut app);
        }
        app.tabs = tabs;
        assert_eq!(app.terminal.key, bound_key, "a transient canonicalize failure must not flip the key");
        assert_eq!(app.terminal.session_dir, bound_dir);
        assert!(app.terminal.lock_owner.is_some(), "the lock stays held");
        assert_eq!(app.terminal.permission_mode, mode_before, "settings.json is not re-read while nothing changed");
        app.terminal.shutdown();
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&work);
    }

    // File > Open of a disk file, then of an .svg (a scratch document): the SVG must not inherit
    // the file's session (rule 2 last sentence).
    #[test]
    fn disk_file_followed_by_scratch_document_drops_the_file_session() {
        let root = temp_test_dir("file-to-scratch-lib");
        let work = temp_test_dir("file-to-scratch-files");
        let file = work.join("poster.designos");
        crate::io::save_path(&Document::new("Poster"), &file).expect("save disk file");
        let stable = root.join("agent-sessions").join(disk_file_session_key(&file));

        let mut app = App::new(Document::new("Scratch"));
        app.attach_library(root.clone());
        app.open_path(file.clone());
        let mut tabs = std::mem::take(&mut app.tabs);
        tabs.reconcile_frame(&mut app);
        app.tabs = tabs;
        assert_eq!(app.terminal.session_dir, stable);
        assert!(stable.join("session.lock").is_file());

        app.replace_document(Document::new("Svg"), None);
        let mut tabs = std::mem::take(&mut app.tabs);
        tabs.reconcile_frame(&mut app);
        tabs.reconcile_frame(&mut app);
        app.tabs = tabs;
        assert!(matches!(app.terminal.key, SessionKey::Fallback(_)), "a scratch document leaves the file's key: {:?}", app.terminal.key);
        assert_ne!(app.terminal.session_dir, stable);
        assert!(app.terminal.lock_owner.is_none());
        assert!(!stable.join("session.lock").exists(), "the file's lock is released");
        assert!(!app.terminal.session_dir.starts_with(root.join("agent-sessions")));
        app.terminal.shutdown();
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&work);
    }

    // 2. `registry_round_trip_keeps_unknown_fields`
    #[test]
    fn registry_round_trip_keeps_unknown_fields() {
        let dir = temp_test_dir("registry-round-trip");
        let raw_json = r#"{
  "version": 1,
  "future_top_level": { "flag": true },
  "sessions": [
    {
      "tab_id": 1,
      "title": "Claude CLI #1",
      "cli": "claude",
      "session_id": "019a0102-0304-7506-8708-090a0b0c0d0e",
      "created_at": 1792380000,
      "last_active_at": 1792381234,
      "status": "exited",
      "future_entry_field": "preserve-me"
    }
  ]
}"#;
        std::fs::write(dir.join("sessions.json"), raw_json).expect("write sessions.json");

        let reg = load_or_init_registry(&dir);
        assert_eq!(reg.version, 1);
        assert_eq!(reg.sessions.len(), 1);
        assert_eq!(reg.extra.get("future_top_level"), Some(&serde_json::json!({ "flag": true })));
        assert_eq!(reg.sessions[0].extra.get("future_entry_field"), Some(&serde_json::json!("preserve-me")));

        save_registry(&dir, &reg).expect("save registry");
        let reloaded = load_or_init_registry(&dir);
        assert_eq!(reloaded, reg);

        // Truncated/corrupt sessions.json is backed up to sessions.json.bad and replaced with an empty registry.
        std::fs::write(dir.join("sessions.json"), b"{\"version\": 1, \"sessions\": [").expect("write truncated");
        let recovered = load_or_init_registry(&dir);
        assert_eq!(recovered.version, 1);
        assert!(recovered.sessions.is_empty());
        assert!(dir.join("sessions.json.bad").is_file(), "truncated file must be moved to sessions.json.bad");
        assert!(dir.join("sessions.json").is_file(), "fresh sessions.json must be written");

        let _ = std::fs::remove_dir_all(&dir);
    }

    // 3. `claude_new_start_passes_a_session_id_and_registers_it`
    #[cfg(unix)]
    #[test]
    fn claude_new_start_passes_a_session_id_and_registers_it() {
        let dir = temp_test_dir("claude-new-start");
        let bin_dir = dir.join("bin");
        std::fs::create_dir_all(&bin_dir).expect("bin dir");
        write_executable_script(&bin_dir, "claude", "#!/bin/sh\nprintf '%s\\n' \"$*\" > \"$DESIGNOS_SESSION_DIR/argv.log\"\nsleep 30\n");

        let mut st = TerminalState::for_tab(1);
        st.bind_library_file(&dir, "file-claude-1", "p1-w1-tab-1");
        st.path_prepend.push(bin_dir);
        st.spawn_tab(AgentCliPreset::Claude, None).expect("spawn claude");

        let uuid = st.tabs[0].session_uuid.clone().expect("claude tab should have a pre-assigned UUID v4");
        assert_eq!(uuid.len(), 36);
        assert_eq!(&uuid[14..15], "4", "UUID v4 version nibble: {uuid}");

        let cmd = build_new_session_command(AgentCliPreset::Claude, Some(&uuid));
        assert!(cmd.starts_with(&format!("claude --session-id {uuid} --dangerously-skip-permissions")));

        let on_disk = load_or_init_registry(&st.session_dir);
        assert_eq!(on_disk.sessions.len(), 1);
        assert_eq!(on_disk.sessions[0].cli, "claude");
        assert_eq!(on_disk.sessions[0].session_id.as_deref(), Some(uuid.as_str()));
        assert_eq!(on_disk.sessions[0].status, SessionEntryStatus::Active);

        st.shutdown();
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A Claude tab bound to a library with a stub `claude` on its PATH, started while the library still holds Skip.
    #[cfg(unix)]
    fn claude_window_started_in_skip(tag: &str) -> (PathBuf, TerminalState) {
        use crate::agent_permissions::AgentPermissionMode;
        let dir = temp_test_dir(tag);
        let bin_dir = dir.join("bin");
        std::fs::create_dir_all(&bin_dir).expect("bin dir");
        write_executable_script(&bin_dir, "claude", "#!/bin/sh\nsleep 30\n");
        crate::agent_permissions::save_permission_mode(&dir, AgentPermissionMode::Skip).expect("the library holds Skip");
        let mut st = TerminalState::for_tab(1);
        st.bind_library_file(&dir, "file-perm", "p1-w1-tab-1");
        st.path_prepend.push(bin_dir);
        assert_eq!(st.permission_mode, AgentPermissionMode::Skip, "this window cached Skip when it bound");
        (dir, st)
    }

    /// Another window (its own process) saves Ask after this window bound: the next Claude tab launches without
    /// the skip-permissions flag, not just with `tab.permission_mode` set.
    #[cfg(unix)]
    #[test]
    fn a_claude_tab_launches_in_ask_when_another_window_saved_ask() {
        use crate::agent_permissions::{save_permission_mode, AgentPermissionMode};
        let (dir, mut st) = claude_window_started_in_skip("claude-ask-launch");
        save_permission_mode(&dir, AgentPermissionMode::Ask).expect("the other window saves Ask");

        let idx = st.spawn_tab(AgentCliPreset::Claude, None).expect("spawn claude");
        let launched = st.tabs[idx].spawned_command.clone().expect("an agent tab records its launch command");
        assert!(launched.starts_with("claude --session-id "), "a new session starts: {launched}");
        assert!(!launched.contains("--dangerously-skip-permissions"), "Ask must not launch with the skip flag: {launched}");

        st.shutdown();
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A snapshot queued before an agent starts carries the run id and permission mode of that moment. It must not
    /// overwrite the `mcp-config.json` and `hooks.json` the start itself wrote: the agent's `DESIGNOS_AGENT_RUN_ID`
    /// would stop matching its tab, and an `Ask` session would lose its approval hook.
    #[cfg(unix)]
    #[test]
    fn a_snapshot_queued_before_a_spawn_does_not_overwrite_the_spawn_time_session_files() {
        use crate::agent_permissions::{save_permission_mode, AgentPermissionMode};
        let (dir, mut st) = claude_window_started_in_skip("snapshot-before-spawn");
        let mut doc = Document::new("Snapshot before spawn");
        let page1 = doc.add_page("Page 1");
        let page2 = doc.add_page("Page 2");
        st.spawn_tab(AgentCliPreset::Claude, None).expect("spawn the first claude in Skip");
        st.sync_live_file_context(&doc, page1, &[], None);
        st.flush_live_sync();

        // Hold the writer in the middle of a job queued while the window still holds Skip and the first tab's run id.
        let (release, hold) = std::sync::mpsc::channel::<()>();
        crate::terminal::set_before_rename_test_hook(
            &st.live_canvas_path,
            Box::new(move || {
                let _ = hold.recv_timeout(std::time::Duration::from_secs(10));
            }),
        );
        st.sync_live_file_context(&doc, page2, &[], None);
        let releaser = std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(500));
            let _ = release.send(());
        });

        save_permission_mode(&dir, AgentPermissionMode::Ask).expect("the other window saves Ask");
        let second = st.spawn_tab(AgentCliPreset::Claude, None).expect("spawn the second claude in Ask");
        let _ = releaser.join();
        st.flush_live_sync();
        st.sync_live_file_context(&doc, page2, &[], None);
        st.flush_live_sync();

        let config: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(st.session_dir.join("mcp-config.json")).expect("mcp-config.json")).unwrap();
        assert_eq!(
            config["mcpServers"]["designos"]["env"]["DESIGNOS_AGENT_RUN_ID"].as_str(),
            Some(st.tabs[second].run_id.as_str()),
            "the session's run id must be the started tab's, not the one a snapshot queued before the start carried"
        );
        let hooks = std::fs::read_to_string(crate::agent_permissions::claude_hooks_path(&st)).expect("hooks.json");
        assert!(hooks.contains("approve-hook.sh"), "an Ask session keeps its approval hook: {hooks}");

        st.shutdown();
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Resuming a recorded Claude session also honours the Ask another window saved.
    #[cfg(unix)]
    #[test]
    fn a_resumed_claude_tab_launches_in_ask_when_another_window_saved_ask() {
        use crate::agent_permissions::{save_permission_mode, AgentPermissionMode};
        let (dir, mut st) = claude_window_started_in_skip("claude-ask-resume");
        let first = st.spawn_tab(AgentCliPreset::Claude, None).expect("spawn claude in Skip");
        let first_launch = st.tabs[first].spawned_command.clone().expect("launch command");
        assert!(first_launch.contains("--dangerously-skip-permissions"), "the Skip window launches with the flag: {first_launch}");
        let recorded = st.session_registry.sessions.first().cloned().expect("the tab is recorded in sessions.json");
        st.session_registry.sessions[0].status = SessionEntryStatus::Exited;

        save_permission_mode(&dir, AgentPermissionMode::Ask).expect("the other window saves Ask");
        let resumed = st.resume_session_by_tab_id(recorded.tab_id, None).expect("resume the recorded session");
        let launched = st.tabs[resumed].spawned_command.clone().expect("a resumed tab records its launch command");
        assert!(launched.starts_with("claude --resume "), "the recorded session resumes: {launched}");
        assert!(!launched.contains("--dangerously-skip-permissions"), "Ask must not resume with the skip flag: {launched}");

        st.shutdown();
        let _ = std::fs::remove_dir_all(&dir);
    }

    // 4. `resume_commands_per_cli`
    #[test]
    fn resume_commands_per_cli() {
        let mk = |cli: &str, session_id: Option<&str>| SessionEntry {
            tab_id: 1,
            title: format!("{cli} #1"),
            cli: cli.to_string(),
            session_id: session_id.map(|s| s.to_string()),
            created_at: 100,
            last_active_at: 200,
            status: SessionEntryStatus::Exited,
            extra: BTreeMap::new(),
        };

        let claude_with_id = build_resume_command(&mk("claude", Some("11111111-2222-4333-8444-555555555555")), false).unwrap();
        assert!(claude_with_id.starts_with(
            "claude --resume 11111111-2222-4333-8444-555555555555 --dangerously-skip-permissions --mcp-config \"$DESIGNOS_MCP_CONFIG\" --settings \"$DESIGNOS_SESSION_DIR/hooks.json\" --append-system-prompt "
        ));

        let claude_no_id = build_resume_command(&mk("claude", None), false).unwrap();
        assert!(claude_no_id.starts_with(
            "claude --continue --dangerously-skip-permissions --mcp-config \"$DESIGNOS_MCP_CONFIG\" --settings \"$DESIGNOS_SESSION_DIR/hooks.json\" --append-system-prompt "
        ));

        assert!(build_resume_command(&mk("codex", None), false).is_none(), "codex resume must be suppressed unless codex_cwd_scoped is verified");
        let codex = build_resume_command(&mk("codex", None), true).unwrap();
        assert!(codex.starts_with("codex resume --last --dangerously-bypass-approvals-and-sandbox -c 'mcp_servers.designos.command="));

        let grok_with_id = build_resume_command(&mk("grok", Some("aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee")), false).unwrap();
        assert_eq!(grok_with_id, "grok --resume aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee --always-approve --cwd \"$DESIGNOS_SESSION_DIR\"");

        let grok_no_id = build_resume_command(&mk("grok", None), false).unwrap();
        assert_eq!(grok_no_id, "grok --continue --always-approve --cwd \"$DESIGNOS_SESSION_DIR\"");

        assert!(build_resume_command(&mk("agy", None), false).is_none(), "agy resume must be suppressed unless agy_cwd_scoped is verified");
        let agy_scoped = build_resume_command(&mk("agy", None), true).unwrap();
        assert_eq!(agy_scoped, "agy --continue --dangerously-skip-permissions --add-dir \"$DESIGNOS_SESSION_DIR\"");

        let gemini_latest = build_resume_command(&mk("gemini", None), false).unwrap();
        assert_eq!(gemini_latest, "gemini --resume latest --approval-mode yolo --include-directories \"$DESIGNOS_SESSION_DIR\"");

        let opencode_last = build_resume_command(&mk("opencode", None), false).unwrap();
        assert_eq!(opencode_last, "opencode --continue");

        let opencode_with_id = build_resume_command(&mk("opencode", Some("ses_12345")), false).unwrap();
        assert_eq!(opencode_with_id, "opencode --session ses_12345");
    }

    // 5. `shell_and_custom_cli_tabs_are_never_registered`
    #[test]
    fn shell_and_custom_cli_tabs_are_never_registered() {
        let dir = temp_test_dir("shell-never-registered");
        let mut st = TerminalState::for_tab(1);
        st.bind_library_file(&dir, "file-shell-1", "p1-w1-tab-1");

        st.spawn_tab(AgentCliPreset::Shell, None).expect("spawn shell");
        st.spawn_tab(AgentCliPreset::DesignOsHelp, None).expect("spawn help");
        assert!(st.session_registry.sessions.is_empty());
        assert!(!st.session_dir.join("sessions.json").exists(), "Shell/custom tabs must not create or touch sessions.json");

        st.shutdown();
        let _ = std::fs::remove_dir_all(&dir);
    }

    // 6. `codex_has_no_id_lookup`
    #[cfg(unix)]
    #[test]
    fn codex_has_no_id_lookup() {
        let dir = temp_test_dir("codex-no-id");
        let bin_dir = dir.join("bin");
        std::fs::create_dir_all(&bin_dir).expect("bin dir");
        write_executable_script(&bin_dir, "codex", "#!/bin/sh\nsleep 30\n");
        write_executable_script(&bin_dir, "agy", "#!/bin/sh\nsleep 30\n");

        let mut st = TerminalState::for_tab(1);
        st.bind_library_file(&dir, "file-codex-1", "p1-w1-tab-1");
        st.path_prepend.push(bin_dir);

        // Codex is recorded only with its own session id, and `codex resume --last` gives Studio none (`#926`).
        // So it is not recorded, and Antigravity is not either until `agy_cwd_scoped` is verified.
        st.spawn_tab(AgentCliPreset::Codex, None).expect("spawn codex");
        st.spawn_tab(AgentCliPreset::Antigravity, None).expect("spawn unscoped agy");
        assert!(st.session_registry.sessions.is_empty(), "codex has no session id and unverified agy is not scoped: neither is registered");

        // Verifying the cwd scope of `codex resume --last` does not make Codex recordable: it still has no id.
        st.codex_cwd_scoped = true;
        st.spawn_tab(AgentCliPreset::Codex, None).expect("spawn codex with a verified cwd scope");
        assert!(st.session_registry.sessions.is_empty(), "a verified cwd scope does not record Codex");
        assert!(load_or_init_registry(&st.session_dir).sessions.is_empty(), "and none reaches sessions.json (which only remembers `last_cli`)");

        // Antigravity is recorded once its scope is verified.
        st.agy_cwd_scoped = true;
        st.spawn_tab(AgentCliPreset::Antigravity, None).expect("spawn scoped agy");
        assert_eq!(st.session_registry.sessions.len(), 1);
        assert_eq!(st.session_registry.sessions[0].cli, "agy");

        st.shutdown();
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A Codex tab started after a Claude session is not recorded: only a session that can be resumed by its own id
    /// is (`#653` rules 4 and 6), and `codex resume --last` takes no id. So it never becomes the newest entry, and
    /// opening the file still resumes the Claude session.
    #[cfg(unix)]
    #[test]
    fn a_codex_tab_started_after_a_claude_session_leaves_auto_resume_on_the_claude_session() {
        let dir = temp_test_dir("codex-after-claude");
        let bin_dir = dir.join("bin");
        std::fs::create_dir_all(&bin_dir).expect("bin dir");
        write_executable_script(&bin_dir, "claude", "#!/bin/sh\nsleep 30\n");
        write_executable_script(&bin_dir, "codex", "#!/bin/sh\nsleep 30\n");

        let mut st = TerminalState::for_tab(1);
        st.bind_library_file(&dir, "file-codex-after-claude", "p1-w1-tab-1");
        st.path_prepend.push(bin_dir);

        st.spawn_tab(AgentCliPreset::Claude, None).expect("spawn claude");
        let claude_uuid = st.tabs[0].session_uuid.clone().expect("claude session id");
        st.spawn_tab(AgentCliPreset::Codex, None).expect("spawn codex after claude");
        assert_eq!(st.tabs.len(), 2);
        assert_eq!(st.session_registry.sessions.len(), 1, "only the Claude session is recorded");
        assert_eq!(st.session_registry.sessions[0].cli, "claude");

        // Close the Claude tab first and the Codex tab last, so a recorded Codex entry would be the newest.
        st.close_tab(0);
        st.close_tab(0);
        assert!(st.tabs.is_empty());

        st.open_for_agent_use(None);
        assert_eq!(st.tabs.len(), 1, "opening the Agents panel resumes a session");
        assert_eq!(st.tabs[0].spawn_preset, AgentCliPreset::Claude, "and it is the Claude session");
        assert!(st.tabs[0].is_resumed);
        assert_eq!(st.tabs[0].session_uuid.as_deref(), Some(claude_uuid.as_str()));

        st.shutdown();
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Sessions recorded before Claude was named "Claude Code" everywhere carry the old default title
    /// `Claude CLI #<n>`. Opening the file renames exactly those, once, in `sessions.json` too: a resumed tab and the
    /// Sessions list show the current name, and a title the user typed is never touched.
    #[cfg(unix)]
    #[test]
    fn opening_a_file_renames_old_default_titles_and_keeps_the_users_own() {
        let dir = temp_test_dir("legacy-titles");
        let bin_dir = dir.join("bin");
        std::fs::create_dir_all(&bin_dir).expect("bin dir");
        write_executable_script(&bin_dir, "claude", "#!/bin/sh\nsleep 30\n");
        let entry = |tab_id: u64, cli: &str, title: &str, session_id: Option<&str>| SessionEntry {
            tab_id,
            title: title.into(),
            cli: cli.into(),
            session_id: session_id.map(str::to_string),
            created_at: 50,
            last_active_at: 100 + tab_id,
            status: SessionEntryStatus::Exited,
            extra: BTreeMap::new(),
        };
        let uuid = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
        let session_dir = stable_session_dir(&dir, "file-legacy-titles");
        std::fs::create_dir_all(&session_dir).expect("session dir");
        let seeded = SessionRegistry {
            version: 1,
            last_cli: None,
            sessions: vec![
                entry(1, "claude", "Claude CLI #1", Some(uuid)),
                entry(2, "claude", "Claude CLI #2 notes", None),
                entry(3, "claude", "Checkout polish", None),
                entry(4, "grok", "Claude CLI #4", None),
                entry(5, "claude", "Claude Code #5", None),
            ],
            extra: BTreeMap::new(),
        };
        save_registry(&session_dir, &seeded).expect("seed sessions.json");

        let mut st = TerminalState::for_tab(1);
        st.bind_library_file(&dir, "file-legacy-titles", "p1-w1-tab-1");
        st.path_prepend.push(bin_dir);

        let titles = |reg: &SessionRegistry| reg.sessions.iter().map(|e| e.title.clone()).collect::<Vec<_>>();
        let expected = ["Claude Code #1", "Claude CLI #2 notes", "Checkout polish", "Claude CLI #4", "Claude Code #5"];
        assert_eq!(titles(&st.session_registry), expected, "only the exact old default of a Claude session is renamed");
        assert_eq!(titles(&load_or_init_registry(&session_dir)), expected, "and the file on disk carries the new names");
        assert_eq!(st.sessions_popover_rows()[4].label, "Claude Code #1", "the Sessions list shows the current name");
        assert!(!st.session_registry.rename_legacy_default_titles(), "a second pass finds nothing left to rename");

        st.resume_session_by_tab_id(1, None).expect("resume the renamed session");
        assert_eq!(st.tabs[0].title, "Claude Code #1", "a resumed tab is named with the current default");

        st.shutdown();
        let _ = std::fs::remove_dir_all(&dir);
    }

    // 7. `restart_resumes_the_previous_session`
    #[cfg(unix)]
    #[test]
    fn restart_resumes_the_previous_session() {
        let dir = temp_test_dir("restart-resumes");
        let bin_dir = dir.join("bin");
        std::fs::create_dir_all(&bin_dir).expect("bin dir");
        write_executable_script(
            &bin_dir,
            "claude",
            "#!/bin/sh\nprintf 'CWD=%s ARGS=%s\\n' \"$(pwd -P)\" \"$*\" >> \"$DESIGNOS_SESSION_DIR/argv.log\"\nsleep 30\n",
        );

        let first_uuid = {
            let mut st1 = TerminalState::for_tab(1);
            st1.bind_library_file(&dir, "file-restart-1", "p1-w1-tab-1");
            st1.path_prepend.push(bin_dir.clone());
            st1.spawn_tab(AgentCliPreset::Claude, None).expect("spawn first claude");
            let uuid = st1.tabs[0].session_uuid.clone().expect("uuid");
            wait_for_whole_logged_line(&st1.session_dir.join("argv.log"));
            st1.shutdown();
            uuid
        };

        let mut st2 = TerminalState::for_tab(1);
        st2.bind_library_file(&dir, "file-restart-1", "p1-w2-tab-1");
        st2.path_prepend.push(bin_dir);
        assert_eq!(st2.resumable_sessions().len(), 1);
        assert_eq!(st2.resumable_sessions()[0].session_id.as_deref(), Some(first_uuid.as_str()));

        st2.resume_last_session(None).expect("resume last session");
        let mut log_contents = String::new();
        crate::test_timing::poll_until(crate::test_timing::STUB_WAIT, || {
            log_contents = std::fs::read_to_string(st2.session_dir.join("argv.log")).unwrap_or_default();
            log_contents.ends_with('\n') && log_contents.lines().count() >= 2
        });

        let canonical_session_dir = st2.session_dir.canonicalize().unwrap_or_else(|_| st2.session_dir.clone());
        let lines: Vec<&str> = log_contents.lines().collect();
        assert!(lines.len() >= 2, "expected 2 invocations in argv.log, got:\n{log_contents}");
        assert!(lines[0].contains(&format!("--session-id {first_uuid}")), "first run must pass --session-id: {}", lines[0]);
        assert!(lines[1].contains(&format!("--resume {first_uuid}")), "resumed run must pass --resume {first_uuid}: {}", lines[1]);
        assert!(
            lines[1].contains(&format!("CWD={}", canonical_session_dir.display())),
            "resumed run must execute in session_dir ({}): {}",
            canonical_session_dir.display(),
            lines[1]
        );

        st2.shutdown();
        let _ = std::fs::remove_dir_all(&dir);
    }

    // 8. `second_window_does_not_share_the_folder`
    #[cfg(unix)]
    #[test]
    fn second_window_does_not_share_the_folder() {
        let dir = temp_test_dir("second-window-lock");
        let bin_dir = dir.join("bin");
        std::fs::create_dir_all(&bin_dir).expect("bin dir");
        write_executable_script(&bin_dir, "claude", "#!/bin/sh\nsleep 30\n");

        let mut st1 = TerminalState::for_tab(1);
        st1.bind_library_file(&dir, "shared-file", "p1-w1-tab-1");
        assert_eq!(st1.session_dir, stable_session_dir(&dir, "shared-file"));
        assert!(st1.session_dir.join("session.lock").is_file());

        let mut st2 = TerminalState::for_tab(1);
        st2.path_prepend.push(bin_dir);
        st2.bind_library_file(&dir, "shared-file", "p1-w2-tab-1");
        let st2_fallback_dir = st2.session_dir.clone();
        assert_ne!(st2.session_dir, st1.session_dir);
        assert_eq!(st2.session_dir, crate::agent_session_files::ephemeral_session_dir("p1-w2-tab-1"));
        assert_eq!(st2.status_msg.as_deref(), Some(ANOTHER_WINDOW_OWNS_SESSION_MSG));

        // Spawn a tab in the fallback window and shut it down while st1 has not written sessions.json yet.
        st2.spawn_tab(AgentCliPreset::Claude, None).expect("spawn claude in fallback window");
        st2.shutdown();
        assert!(
            !st1.session_dir.join("sessions.json").exists(),
            "fallback window must never write sessions.json into the stable folder owned by another window"
        );
        assert!(!st2_fallback_dir.exists(), "fallback window's ephemeral dir must be removed on shutdown");

        st1.shutdown();
        let _ = std::fs::remove_dir_all(&dir);
    }

    // 9. `stale_lock_is_taken_over`
    #[test]
    fn stale_lock_is_taken_over() {
        let dir = temp_test_dir("stale-lock");
        let target_dir = stable_session_dir(&dir, "stale-file");
        std::fs::create_dir_all(&target_dir).expect("create session dir");
        let stale = SessionLock { pid: 999_999_991, window: "dead-window".to_string(), started_at: 100 };
        std::fs::write(target_dir.join("session.lock"), serde_json::to_vec_pretty(&stale).unwrap()).expect("write stale lock");

        let mut st = TerminalState::for_tab(1);
        st.bind_library_file_with_liveness(&dir, "stale-file", "live-window", |_pid| false);
        assert_eq!(st.session_dir, target_dir);
        assert_eq!(st.lock_owner.as_deref(), Some("live-window"));

        let lock_now: SessionLock = serde_json::from_slice(&std::fs::read(target_dir.join("session.lock")).unwrap()).unwrap();
        assert_eq!(lock_now.pid, std::process::id());
        assert_eq!(lock_now.window, "live-window");

        st.shutdown();
        let _ = std::fs::remove_dir_all(&dir);
    }

    // 10. `resume_failure_marks_the_entry_missing`
    #[cfg(unix)]
    #[test]
    fn resume_failure_marks_the_entry_missing() {
        let dir = temp_test_dir("resume-failure");
        let bin_dir = dir.join("bin");
        std::fs::create_dir_all(&bin_dir).expect("bin dir");
        write_executable_script(&bin_dir, "claude", "#!/bin/sh\necho 'No conversation found to resume'\nexit 1\n");

        let mut st = TerminalState::for_tab(1);
        st.bind_library_file(&dir, "file-fail-resume", "p1-w1-tab-1");
        st.path_prepend.push(bin_dir);
        st.session_registry.sessions.push(SessionEntry {
            tab_id: 1,
            title: "Claude CLI #1".to_string(),
            cli: "claude".to_string(),
            session_id: Some("019a0102-0304-7506-8708-090a0b0c0d0e".to_string()),
            created_at: 100,
            last_active_at: 200,
            status: SessionEntryStatus::Exited,
            extra: BTreeMap::new(),
        });
        save_registry(&st.session_dir, &st.session_registry).expect("save initial registry");

        st.resume_last_session(None).expect("spawn resume");
        st.poll_until_start_failure(crate::test_timing::STUB_WAIT);

        let failure = st.active_tab().and_then(|t| t.start_failure.as_ref()).expect("failure card must be present");
        assert_eq!(failure.fix, crate::agent_start_failure::FixAction::StartNewSession(AgentCliPreset::Claude));
        assert_eq!(failure.fix_label(), "Start new session");

        let on_disk = load_or_init_registry(&st.session_dir);
        assert_eq!(on_disk.sessions[0].status, SessionEntryStatus::Missing);

        st.shutdown();
        let _ = std::fs::remove_dir_all(&dir);
    }

    // 11. `trash_empty_removes_the_session_folder`
    #[test]
    fn trash_empty_removes_the_session_folder() {
        let root = temp_test_dir("trash-empty");
        let lib = Library::open(&root).expect("open library");
        let proj = lib.create_project("Drafts").expect("create project").id;
        let f = lib.create_file(&proj, None, "Temp File", &Document::new("Temp File")).expect("create file");

        let sess_dir = lib.agent_session_dir(&f.id);
        std::fs::create_dir_all(&sess_dir).expect("create session dir");
        std::fs::write(sess_dir.join("sessions.json"), b"{\"version\":1,\"sessions\":[]}").expect("write sessions.json");
        assert!(sess_dir.is_dir());

        lib.trash_file(&f.id).expect("trash file");
        assert!(sess_dir.is_dir(), "soft-delete keeps the session folder until trash is emptied");

        lib.empty_trash().expect("empty trash");
        assert!(!sess_dir.exists(), "emptying the trash must remove <lib_root>/agent-sessions/<file_id>");

        let _ = std::fs::remove_dir_all(&root);
    }

    // 12. `launch_commands_use_only_flags_each_cli_lists`
    #[test]
    fn launch_commands_use_only_flags_each_cli_lists() {
        // Captured `--help` flags from §6 of docs/specs/v0.6/653-stable-per-file-agent-sessions.md:
        const CLAUDE_HELP: &str = "--session-id <uuid> --resume [value] -r --continue -c --dangerously-skip-permissions --mcp-config <configs...> --settings <file-or-json> --append-system-prompt <prompt>";
        const CODEX_HELP: &str = "resume --last --all --dangerously-bypass-approvals-and-sandbox -c --config <key=value>";
        const GROK_HELP: &str = "--always-approve -a --session-id <uuid> -s --resume [value] -r --continue -c --cwd <path>";
        const AGY_HELP: &str = "--dangerously-skip-permissions --continue -c --conversation <id> --add-dir <dir>";

        let check_flags = |cmd: &str, help: &str, allowed_flags: &[&str]| {
            for flag in allowed_flags {
                assert!(help.contains(flag), "flag {flag} missing from captured --help: {help}");
            }
            for token in cmd.split_whitespace() {
                if token.starts_with('-') {
                    assert!(allowed_flags.contains(&token), "command '{cmd}' uses unverified flag '{token}' not in {allowed_flags:?}");
                }
            }
        };

        let claude_flags =
            &["--session-id", "--resume", "--continue", "--dangerously-skip-permissions", "--mcp-config", "--settings", "--append-system-prompt"];
        check_flags(&build_new_session_command(AgentCliPreset::Claude, Some("019a0102-0304-7506-8708-090a0b0c0d0e")), CLAUDE_HELP, claude_flags);
        let claude_entry = SessionEntry {
            tab_id: 1,
            title: "Claude".into(),
            cli: "claude".into(),
            session_id: Some("019a0102-0304-7506-8708-090a0b0c0d0e".into()),
            created_at: 1,
            last_active_at: 1,
            status: SessionEntryStatus::Exited,
            extra: BTreeMap::new(),
        };
        check_flags(&build_resume_command(&claude_entry, false).unwrap(), CLAUDE_HELP, claude_flags);

        let codex_flags = &["--last", "--dangerously-bypass-approvals-and-sandbox", "-c"];
        check_flags(&build_new_session_command(AgentCliPreset::Codex, None), CODEX_HELP, codex_flags);
        let codex_entry = SessionEntry {
            tab_id: 1,
            title: "Codex".into(),
            cli: "codex".into(),
            session_id: None,
            created_at: 1,
            last_active_at: 1,
            status: SessionEntryStatus::Exited,
            extra: BTreeMap::new(),
        };
        check_flags(&build_resume_command(&codex_entry, true).unwrap(), CODEX_HELP, codex_flags);

        let grok_flags = &["--session-id", "--resume", "--continue", "--always-approve", "--cwd"];
        check_flags(&build_new_session_command(AgentCliPreset::Grok, Some("019a0102-0304-7506-8708-090a0b0c0d0e")), GROK_HELP, grok_flags);
        let grok_entry = SessionEntry {
            tab_id: 1,
            title: "Grok".into(),
            cli: "grok".into(),
            session_id: Some("019a0102-0304-7506-8708-090a0b0c0d0e".into()),
            created_at: 1,
            last_active_at: 1,
            status: SessionEntryStatus::Exited,
            extra: BTreeMap::new(),
        };
        check_flags(&build_resume_command(&grok_entry, false).unwrap(), GROK_HELP, grok_flags);

        let agy_flags = &["--continue", "--dangerously-skip-permissions", "--add-dir"];
        check_flags(&build_new_session_command(AgentCliPreset::Antigravity, None), AGY_HELP, agy_flags);
        let agy_entry = SessionEntry {
            tab_id: 1,
            title: "Antigravity".into(),
            cli: "agy".into(),
            session_id: None,
            created_at: 1,
            last_active_at: 1,
            status: SessionEntryStatus::Exited,
            extra: BTreeMap::new(),
        };
        check_flags(&build_resume_command(&agy_entry, true).unwrap(), AGY_HELP, agy_flags);
    }

    // 13. `malformed_session_id_is_rejected_by_resume_command`
    #[test]
    fn malformed_session_id_is_rejected_by_resume_command() {
        for bad_id in ["019a0102-0304-7506-8708-090a0b0c0d0e; touch /tmp/pwn", "$(touch /tmp/pwn)", "not-a-uuid", "019a0102-0304-7506-8708-090a0b0c0d0z"] {
            let claude_bad = SessionEntry {
                tab_id: 1,
                title: "Claude".into(),
                cli: "claude".into(),
                session_id: Some(bad_id.into()),
                created_at: 1,
                last_active_at: 1,
                status: SessionEntryStatus::Exited,
                extra: BTreeMap::new(),
            };
            assert!(build_resume_command(&claude_bad, false).is_none(), "malformed session_id '{bad_id}' must be rejected for claude");

            let grok_bad = SessionEntry { cli: "grok".into(), ..claude_bad };
            assert!(build_resume_command(&grok_bad, false).is_none(), "malformed session_id '{bad_id}' must be rejected for grok");
        }
    }

    // 14. `codex_is_never_registered_and_its_resume_stays_gated_on_cwd_scope`
    #[test]
    fn codex_is_never_registered_and_its_resume_stays_gated_on_cwd_scope() {
        assert!(!is_registrable_cli(AgentCliPreset::Codex), "Codex is not registered without a session id of its own");
        assert!(!is_registrable_cli(AgentCliPreset::Antigravity), "Antigravity must not register until cwd scope is verified");
        assert!(is_registrable_cli(AgentCliPreset::Claude));
        assert!(is_registrable_cli(AgentCliPreset::Grok));

        let codex_entry = SessionEntry {
            tab_id: 1,
            title: "Codex".into(),
            cli: "codex".into(),
            session_id: None,
            created_at: 1,
            last_active_at: 1,
            status: SessionEntryStatus::Exited,
            extra: BTreeMap::new(),
        };
        assert!(build_resume_command(&codex_entry, false).is_none(), "codex resume --last must not be offered when cwd scoping is unverified");
        assert!(build_resume_command(&codex_entry, true).is_some(), "codex resume --last is offered once cwd scoping is verified");
    }

    // 15. `reopening_agents_panel_resumes_previous_session_instead_of_spawning_shell`
    #[cfg(unix)]
    #[test]
    fn reopening_agents_panel_resumes_previous_session_instead_of_spawning_shell() {
        let dir = temp_test_dir("reopen-agents-panel");
        let bin_dir = dir.join("bin");
        std::fs::create_dir_all(&bin_dir).expect("bin dir");
        write_executable_script(&bin_dir, "claude", "#!/bin/sh\nprintf '%s\\n' \"$*\" >> \"$DESIGNOS_SESSION_DIR/argv.log\"\nsleep 30\n");

        let planted_uuid = "019a1111-2222-4333-8444-555555555555";
        let sess_dir = stable_session_dir(&dir, "file-agents-reopen");
        std::fs::create_dir_all(&sess_dir).expect("create session dir");
        let mut planted = SessionRegistry::default();
        planted.sessions.push(SessionEntry {
            tab_id: 1,
            title: "Claude CLI #1".to_string(),
            cli: "claude".to_string(),
            session_id: Some(planted_uuid.to_string()),
            created_at: 100,
            last_active_at: 200,
            status: SessionEntryStatus::Exited,
            extra: BTreeMap::new(),
        });
        save_registry(&sess_dir, &planted).expect("save planted registry");

        let mut st = TerminalState::for_tab(1);
        st.bind_library_file(&dir, "file-agents-reopen", "p1-w1-tab-1");
        st.path_prepend.push(bin_dir);

        // `open_for_agent_use` is the Agents-open path triggered by `RailItem::Agents` and Cmd+`.
        st.open_for_agent_use(None);
        assert_eq!(st.tabs.len(), 1);
        assert_eq!(st.tabs[0].preset, AgentCliPreset::Claude, "must resume Claude instead of spawning Shell");
        assert!(st.tabs[0].is_resumed);

        let argv_log = wait_for_whole_logged_line(&st.session_dir.join("argv.log"));
        assert!(argv_log.contains(&format!("--resume {planted_uuid}")), "child argv must contain --resume {planted_uuid}, got: {argv_log}");

        st.shutdown();
        let _ = std::fs::remove_dir_all(&dir);
    }

    // 15b. `closing_the_last_agent_tab_shows_the_picker_and_does_not_resume`
    #[cfg(unix)]
    #[test]
    fn closing_the_last_agent_tab_shows_the_picker_and_does_not_resume() {
        let dir = temp_test_dir("close-last-tab");
        let bin_dir = dir.join("bin");
        std::fs::create_dir_all(&bin_dir).expect("bin dir");
        write_executable_script(&bin_dir, "claude", "#!/bin/sh\nprintf '%s\\n' \"$*\" >> \"$DESIGNOS_SESSION_DIR/argv.log\"\nsleep 30\n");

        let uuid = "019a1111-2222-4333-8444-666666666666";
        let sess_dir = stable_session_dir(&dir, "file-close-last");
        std::fs::create_dir_all(&sess_dir).expect("create session dir");
        let mut planted = SessionRegistry::default();
        planted.sessions.push(SessionEntry {
            tab_id: 1,
            title: "Claude CLI #1".to_string(),
            cli: "claude".to_string(),
            session_id: Some(uuid.to_string()),
            created_at: 100,
            last_active_at: 200,
            status: SessionEntryStatus::Exited,
            extra: BTreeMap::new(),
        });
        save_registry(&sess_dir, &planted).expect("save planted registry");

        let mut st = TerminalState::for_tab(1);
        st.bind_library_file(&dir, "file-close-last", "p1-w1-tab-1");
        st.path_prepend.push(bin_dir);

        st.open_for_agent_use(None);
        assert_eq!(st.tabs.len(), 1, "opening resumes the file's session");
        let resumes = |st: &TerminalState| std::fs::read_to_string(st.session_dir.join("argv.log")).unwrap_or_default().matches("--resume").count();
        crate::test_timing::poll_until(crate::test_timing::STUB_WAIT, || resumes(&st) > 0);
        st.close_tab(0);
        assert!(st.tabs.is_empty(), "closing the last tab leaves no tab");
        assert!(st.picker_open, "closing the last tab shows the new-session picker");

        // Frames must not bring the closed conversation back.
        std::thread::sleep(std::time::Duration::from_millis(300));
        assert!(st.tabs.is_empty() && st.picker_open);
        assert_eq!(resumes(&st), 1, "only the open resumed the session; the close must not start `claude --resume` again");

        st.shutdown();
        let _ = std::fs::remove_dir_all(&dir);
    }

    // 16. `switching_library_files_shuts_down_previous_file_tabs_and_matches_registry_by_uuid`
    #[cfg(unix)]
    #[test]
    fn switching_library_files_shuts_down_previous_file_tabs_and_matches_registry_by_uuid() {
        let dir = temp_test_dir("switch-library-files");
        let bin_dir = dir.join("bin");
        std::fs::create_dir_all(&bin_dir).expect("bin dir");
        write_executable_script(&bin_dir, "claude", "#!/bin/sh\nsleep 30\n");

        let mut st = TerminalState::for_tab(1);
        st.path_prepend.push(bin_dir);
        st.bind_library_file(&dir, "file-a", "p1-w1-tab-1");
        st.spawn_tab(AgentCliPreset::Claude, None).expect("spawn claude in file-a");
        let uuid_a = st.tabs[0].session_uuid.clone().expect("uuid_a");
        assert_eq!(st.session_registry.sessions[0].status, SessionEntryStatus::Active);

        // Switching the same TerminalState to `file-b` must shut down `file-a`'s tabs and flush `file-a`'s registry.
        st.bind_library_file(&dir, "file-b", "p1-w1-tab-1");
        assert!(st.tabs.is_empty(), "file-a tabs must not remain in self.tabs after rebinding to file-b");
        assert!(st.session_registry.sessions.is_empty(), "file-b registry starts empty");

        let reg_a = load_or_init_registry(&stable_session_dir(&dir, "file-a"));
        assert_eq!(reg_a.sessions.len(), 1);
        assert_eq!(reg_a.sessions[0].session_id.as_deref(), Some(uuid_a.as_str()));
        assert_eq!(reg_a.sessions[0].status, SessionEntryStatus::Exited, "file-a session must be marked Exited on switch");

        st.shutdown();
        let _ = std::fs::remove_dir_all(&dir);
    }

    // 17. `unresumable_newest_entry_leaves_picker_open_and_does_not_skip_to_older_entry`
    #[test]
    fn unresumable_newest_entry_leaves_picker_open_and_does_not_skip_to_older_entry() {
        let dir = temp_test_dir("unresumable-newest");
        let mut st = TerminalState::for_tab(1);
        st.bind_library_file(&dir, "file-unresumable-top", "p1-w1-tab-1");
        st.session_registry.sessions.push(SessionEntry {
            tab_id: 1,
            title: "Claude CLI #1".to_string(),
            cli: "claude".to_string(),
            session_id: Some("019a1111-2222-4333-8444-555555555555".to_string()),
            created_at: 100,
            last_active_at: 200,
            status: SessionEntryStatus::Exited,
            extra: BTreeMap::new(),
        });
        st.session_registry.sessions.push(SessionEntry {
            tab_id: 2,
            title: "Antigravity CLI #2".to_string(),
            cli: "agy".to_string(),
            session_id: None,
            created_at: 250,
            last_active_at: 300,
            status: SessionEntryStatus::Exited,
            extra: BTreeMap::new(),
        });

        assert!(st.resumable_sessions().is_empty(), "unresumable newest entry must not skip to older claude entry");
        st.open_for_agent_use(None);
        assert!(st.tabs.is_empty(), "must not spawn older claude session or default shell");
        assert!(st.picker_open, "new-session picker must be left open when newest entry cannot be resumed");

        assert!(kill_zero_indicates_alive(false, b"kill: (1): Operation not permitted\n"));
        assert!(!kill_zero_indicates_alive(false, b"kill: (99999): No such process\n"));

        st.shutdown();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn resumed_cli_failing_from_login_stays_exited_but_a_bad_flag_marks_the_entry_missing() {
        let dir = temp_test_dir("resume-auth-and-flag");
        let bin_dir = dir.join("bin");
        std::fs::create_dir_all(&bin_dir).expect("bin dir");
        write_executable_script(&bin_dir, "claude", "#!/bin/sh\necho 'Error: not logged in — please sign in'\nexit 1\n");

        let mut st = TerminalState::for_tab(1);
        st.bind_library_file(&dir, "file-auth-resume", "p1-w1-tab-1");
        st.path_prepend.push(bin_dir.clone());
        st.session_registry.sessions.push(SessionEntry {
            tab_id: 1,
            title: "Claude CLI #1".to_string(),
            cli: "claude".to_string(),
            session_id: Some("019a0102-0304-7506-8708-090a0b0c0d0e".to_string()),
            created_at: 100,
            last_active_at: 200,
            status: SessionEntryStatus::Exited,
            extra: BTreeMap::new(),
        });
        save_registry(&st.session_dir, &st.session_registry).expect("save initial registry");

        st.resume_last_session(None).expect("spawn resume");
        st.poll_until_start_failure(crate::test_timing::STUB_WAIT);

        let failure = st.active_tab().and_then(|t| t.start_failure.as_ref()).expect("failure card must be present");
        assert_eq!(failure.cause, crate::agent_start_failure::StartFailureCause::NotLoggedIn);
        assert_eq!(failure.fix, crate::agent_start_failure::FixAction::RunLogin("claude auth login"));

        let on_disk = load_or_init_registry(&st.session_dir);
        assert_eq!(
            on_disk.sessions[0].status,
            SessionEntryStatus::Exited,
            "resumed session that failed due to NotLoggedIn must remain Exited so it can still be resumed after login"
        );
        assert_eq!(st.resumable_sessions().len(), 1);

        // A resume that dies on an unknown flag cannot be retried as it is: the entry is Missing and the panel
        // offers a new session (spec 653 rule 8 and 651 rule 7: only install and login keep the entry resumable).
        write_executable_script(&bin_dir, "claude", "#!/bin/sh\necho 'error: unknown option --bad-flag'\nexit 1\n");
        st.resume_last_session(None).expect("spawn resume for unknown flag");
        st.poll_until_start_failure(crate::test_timing::STUB_WAIT);
        let failure2 = st.active_tab().and_then(|t| t.start_failure.as_ref()).expect("failure card for unknown flag");
        assert_eq!(failure2.cause, crate::agent_start_failure::StartFailureCause::UnknownFlag);
        assert_eq!(failure2.fix, crate::agent_start_failure::FixAction::StartNewSession(AgentCliPreset::Claude));
        let on_disk2 = load_or_init_registry(&st.session_dir);
        assert_eq!(on_disk2.sessions[0].status, SessionEntryStatus::Missing, "a resume that fails on a flag marks the entry Missing");
        assert!(st.resumable_sessions().is_empty(), "the Missing entry is no longer offered for resume");

        st.shutdown();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn save_to_library_as_with_open_agent_tab_binds_stable_dir_and_keeps_session_on_close() {
        let dir = temp_test_dir("save-to-library-open-tab");
        let lib_root = dir.join("lib");
        let bin_dir = dir.join("bin");
        std::fs::create_dir_all(&bin_dir).expect("bin dir");
        write_executable_script(&bin_dir, "claude", "#!/bin/sh\nsleep 30\n");

        let lib = Library::open(&lib_root).expect("open library");
        let proj = lib.create_project("Test Project").expect("create project");

        let mut doc = Document::new("Unsaved");
        let page = doc.add_page("Page 1");
        let target_node = doc.alloc_id();
        doc.insert(page, designos_doc::Node::frame(target_node, designos_doc::Rect::new(0.0, 0.0, 120.0, 80.0))).expect("insert frame");

        let mut app = App::new(doc);
        app.lib = Some(lib.clone());
        app.terminal.path_prepend.push(bin_dir);

        // Start a Claude tab while the document is still unsaved (on Fallback session dir).
        app.terminal.spawn_tab(AgentCliPreset::Claude, None).expect("spawn claude on unsaved doc");
        let spawned_uuid = app.terminal.tabs[0].session_uuid.clone().expect("claude session uuid");
        let scratch_dir = app.terminal.session_dir.clone();
        let tab_patch_path = app.terminal.live_patch.path.clone();
        assert!(crate::agent_session_files::is_ephemeral_session_dir(&scratch_dir), "initial session_dir must be ephemeral scratch dir");

        // Save into the library while the agent tab is still open.
        app.save_to_library_as(&proj.id, None, "Saved With Running Agent");
        let lf = app.lib_file.clone().expect("lib_file set after save_to_library_as");
        let expected_stable_dir = lib.agent_session_dir(&lf.id);
        assert!(expected_stable_dir.join("sessions.json").is_file(), "sessions.json must exist in stable session dir after save_to_library_as");
        assert_eq!(
            app.terminal.session_dir, scratch_dir,
            "while agent tab is still running, session_dir must remain on the scratch folder so the running CLI is not cut off from the canvas"
        );
        assert_eq!(
            app.terminal.live_patch.path, tab_patch_path,
            "while agent tab is still running, live_patch path must still match the tab's DESIGNOS_LIVE_DOC_PATCH"
        );

        // Verify a doc-patch.jsonl line written by the running agent to its DESIGNOS_LIVE_DOC_PATCH is still applied after save_to_library_as.
        let patch_line = serde_json::to_string(&designos_doc::live_patch::LivePatch::new(vec![designos_doc::live_patch::LiveOp::Remove { id: target_node }]))
            .expect("serialize patch")
            + "\n";
        std::fs::write(&tab_patch_path, patch_line.as_bytes()).expect("write doc-patch.jsonl to tab's DESIGNOS_LIVE_DOC_PATCH");
        for action in app.terminal.poll_live_agent_bridge() {
            if let crate::terminal::TerminalAction::ApplyLiveLog(lines) = action {
                let _ = app.terminal.live_patch.ingest(lines);
                crate::live_patch_apply::apply_pending_live_patches(&mut app);
            }
        }
        assert!(
            app.doc.get(target_node).is_none(),
            "doc-patch.jsonl written to the running tab's DESIGNOS_LIVE_DOC_PATCH must still be applied after save_to_library_as"
        );

        // Closing the last open tab completes the deferred rebind to the stable dir and removes the scratch dir.
        app.terminal.close_tab(0);
        assert_eq!(app.terminal.session_dir, expected_stable_dir, "closing the last open tab must rebind session_dir to the stable dir");
        assert!(!scratch_dir.exists(), "closing the last open tab must remove the old scratch folder so it does not leak");
        assert!(expected_stable_dir.join("sessions.json").is_file(), "closing the tab must keep sessions.json in stable dir");
        assert_eq!(app.terminal.resumable_sessions().len(), 1, "session must remain resumable in the open window after closing the tab");

        // Shut down the first window (releasing session.lock) and reopen the library file in a new TerminalState.
        app.terminal.shutdown();

        let mut reopened = TerminalState::for_tab(2);
        reopened.bind_library_file(lib.root(), &lf.id, "p1-w2-tab-1");
        let resumable = reopened.resumable_sessions();
        assert_eq!(resumable.len(), 1, "reopened library file must have 1 resumable session");
        assert_eq!(resumable[0].session_id.as_deref(), Some(spawned_uuid.as_str()));

        reopened.shutdown();
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A window on a file opened from disk, with two earlier sessions in the file's path-keyed folder and a
    /// Claude tab running there, then "Save to library as" (the tab stays up).
    #[cfg(unix)]
    struct SavedDiskFile {
        dir: PathBuf,
        lib_root: PathBuf,
        key: String,
        disk_dir: PathBuf,
        earlier: [&'static str; 2],
        app: App,
    }

    #[cfg(unix)]
    fn disk_file_saved_to_library_with_a_running_tab(tag: &str) -> SavedDiskFile {
        let dir = temp_test_dir(tag);
        let lib_root = dir.join("lib");
        let bin_dir = dir.join("bin");
        std::fs::create_dir_all(&bin_dir).expect("bin dir");
        write_executable_script(&bin_dir, "claude", "#!/bin/sh\nsleep 30\n");
        let lib = Library::open(&lib_root).expect("open library");
        let proj = lib.create_project("Test Project").expect("create project");

        let mut doc = Document::new("On Disk");
        doc.add_page("Page 1");
        let disk_path = dir.join("foo.designos");
        std::fs::write(&disk_path, doc.to_json().expect("serialize doc")).expect("write disk file");
        let key = disk_file_session_key(&disk_path);
        let disk_dir = stable_session_dir(&lib_root, &key);
        let earlier = ["019a0102-0304-7506-8708-090a0b0c0d01", "019a0102-0304-7506-8708-090a0b0c0d02"];
        let entry = |tab_id: u64, uuid: &str| SessionEntry {
            tab_id,
            title: format!("Claude CLI #{tab_id}"),
            cli: "claude".to_string(),
            session_id: Some(uuid.to_string()),
            created_at: 100,
            last_active_at: 200 + tab_id,
            status: SessionEntryStatus::Exited,
            extra: BTreeMap::new(),
        };
        std::fs::create_dir_all(&disk_dir).expect("create the disk file's session folder");
        let registry = SessionRegistry { sessions: vec![entry(1, earlier[0]), entry(2, earlier[1])], ..SessionRegistry::default() };
        save_registry(&disk_dir, &registry).expect("seed earlier sessions");

        let mut app = App::new(doc);
        app.lib = Some(lib);
        app.file_path = Some(disk_path);
        app.terminal.path_prepend.push(bin_dir);
        let window = app.tabs.active_session_id();
        app.bind_terminal_to_open_file(&window);
        assert_eq!(app.terminal.session_dir, disk_dir, "precondition: the file opened from disk uses its path-keyed folder");
        assert_eq!(app.terminal.session_registry.sessions.len(), 2, "precondition: the earlier sessions are loaded");
        app.terminal.spawn_tab(AgentCliPreset::Claude, None).expect("spawn claude on the disk file");

        app.save_to_library_as(&proj.id, None, "Saved Copy");
        assert!(app.lib_file.is_some(), "the document is a library file now");
        SavedDiskFile { dir, lib_root, key, disk_dir, earlier, app }
    }

    /// A file opened from disk keeps its sessions in the folder keyed by its path. "Save to library" while an
    /// agent runs on it must keep that folder locked: the running tab still works there, so a second window
    /// must not take it over (two windows never share a folder).
    #[cfg(unix)]
    #[test]
    fn save_to_library_as_with_a_tab_on_a_disk_file_keeps_that_files_lock() {
        let mut saved = disk_file_saved_to_library_with_a_running_tab("save-disk-file-lock");
        assert_eq!(saved.app.terminal.session_dir, saved.disk_dir, "the running tab keeps its folder");

        let mut other_window = TerminalState::for_tab(2);
        other_window.bind_library_file(&saved.lib_root, &saved.key, "p1-w2-tab-1");
        assert_ne!(other_window.session_dir, saved.disk_dir, "a second window must not share the folder a tab still runs in");
        other_window.shutdown();

        saved.app.terminal.shutdown();
        let _ = std::fs::remove_dir_all(&saved.dir);
    }

    /// The registration of the next tab writes the registry to the terminal's folder: after the save that
    /// must still be the disk file's registry, with its earlier sessions in it.
    #[cfg(unix)]
    #[test]
    fn save_to_library_as_with_a_tab_on_a_disk_file_keeps_that_files_earlier_sessions() {
        let mut saved = disk_file_saved_to_library_with_a_running_tab("save-disk-file-sessions");
        saved.app.terminal.spawn_tab(AgentCliPreset::Claude, None).expect("spawn a second claude after the save");

        let on_disk = load_or_init_registry(&saved.disk_dir);
        for earlier_id in saved.earlier {
            assert!(
                on_disk.sessions.iter().any(|e| e.session_id.as_deref() == Some(earlier_id)),
                "the disk file lost its earlier session {earlier_id}; its registry now holds {:?}",
                on_disk.sessions.iter().map(|e| e.session_id.clone()).collect::<Vec<_>>()
            );
        }
        assert_eq!(on_disk.sessions.len(), 4, "both new tabs are recorded next to the earlier sessions");

        saved.app.terminal.shutdown();
        let _ = std::fs::remove_dir_all(&saved.dir);
    }

    #[test]
    fn build_resume_command_rejects_non_uuid_session_id() {
        let mk = |cli: &str, session_id: Option<&str>| SessionEntry {
            tab_id: 1,
            title: format!("{cli} #1"),
            cli: cli.to_string(),
            session_id: session_id.map(|s| s.to_string()),
            created_at: 100,
            last_active_at: 200,
            status: SessionEntryStatus::Exited,
            extra: BTreeMap::new(),
        };

        for bad_id in ["x; open -a Calculator", "x; open /Applications/Calculator.app; #", "not-a-uuid", "11111111-2222-4333-8444-55555555555g"] {
            assert!(
                build_resume_command(&mk("claude", Some(bad_id)), false).is_none(),
                "claude build_resume_command must reject non-UUID session_id: {bad_id}"
            );
            assert!(build_resume_command(&mk("grok", Some(bad_id)), false).is_none(), "grok build_resume_command must reject non-UUID session_id: {bad_id}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn failed_start_with_empty_output_does_not_sleep_on_ui_thread() {
        let dir = temp_test_dir("fast-silent-exit");
        let bin_dir = dir.join("bin");
        std::fs::create_dir_all(&bin_dir).expect("bin dir");
        write_executable_script(&bin_dir, "claude", "#!/bin/sh\nexit 1\n");

        let mut st = TerminalState::for_tab(1);
        st.bind_library_file(&dir, "file-silent-exit", "p1-w1-tab-1");
        st.path_prepend.push(bin_dir);

        // One start and one poll of the bridge. The stub exits at once with no output. The poll runs only after the PTY reader has
        // seen the end of the output, which is the state the old 100 ms sleep waited for, and only the poll is timed.
        let mut poll_after_silent_exit = || {
            st.spawn_tab(AgentCliPreset::Claude, None).expect("spawn silent exiting claude");
            let ended = crate::test_timing::poll_until(crate::test_timing::STUB_WAIT, || {
                let tab = st.active_tab_mut().unwrap();
                !tab.is_alive() && tab.reader_eof.load(std::sync::atomic::Ordering::Acquire)
            });
            assert!(ended, "the silent stub must exit and its output must end");
            st.active_tab_mut().unwrap().spawned_at = std::time::Instant::now();

            let t0 = std::time::Instant::now();
            let _ = st.poll_live_agent_bridge();
            let elapsed = t0.elapsed();
            assert!(st.active_tab().unwrap().start_failure.is_some(), "start_failure must be recorded");
            elapsed
        };

        // A poll that sleeps 100 ms on the UI thread is over the budget on every run. A scheduler stall on a loaded machine only
        // adds time to a run, so the poll is timed until one run is within budget, as `test_timing::fastest_within` does for a
        // pure function. (`failed_start_drain_in_the_bridge_poll_never_blocks_the_ui_thread` also guards the source.)
        //
        // Patience is a number of runs, not a number of seconds: a run starts a child process and waits for it to exit, which
        // takes tens of seconds on a machine that is busy enough, and a deadline in seconds would then end the loop after one run.
        const RUNS: usize = 60;
        let budget = std::time::Duration::from_millis(25);
        let mut fastest = std::time::Duration::MAX;
        for _ in 0..RUNS {
            fastest = fastest.min(poll_after_silent_exit());
            if fastest < budget {
                break;
            }
        }
        assert!(fastest < budget, "poll_live_agent_bridge must not sleep 100ms on the UI thread when raw_log is empty (took {fastest:?} at best of {RUNS})");

        st.shutdown();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn is_pid_alive_treats_eperm_as_alive_and_lock_takeover_checks_start_time() {
        assert!(is_pid_alive(1), "PID 1 (launchd/init) exists even when kill -0 returns EPERM");

        // Spawn a live child process so `pid != std::process::id()` exercises the external `ps` start-time query.
        let mut child = std::process::Command::new("sleep").arg("30").spawn().expect("spawn sleep child");
        let child_pid = child.id();
        assert_ne!(child_pid, std::process::id());

        let dir = temp_test_dir("recycled-pid-lock");
        let target_dir = stable_session_dir(&dir, "recycled-file");
        std::fs::create_dir_all(&target_dir).expect("create session dir");
        // Write a lock with the live child's PID (simulating OS PID recycling) but a stale start timestamp from days ago.
        let recycled_lock = SessionLock { pid: child_pid, window: "crashed-window-from-yesterday".to_string(), started_at: 1_000_000 };
        std::fs::write(target_dir.join("session.lock"), serde_json::to_vec_pretty(&recycled_lock).unwrap()).expect("write recycled lock");

        let outcome = acquire_stable_session_dir(&dir, "recycled-file", "current-window", "fallback-win");
        let _ = child.kill();
        let _ = child.wait();
        assert_eq!(
            outcome,
            LockOutcome::Acquired(target_dir.clone()),
            "lock whose non-self PID is alive but whose started_at does not match the process start time must be taken over"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }
}
