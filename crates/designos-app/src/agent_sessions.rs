//! Stable per-file agent session folders, lockfiles, `sessions.json` registry, and CLI
//! new-session / resume command builders (`#653`).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::terminal::AgentCliPreset;

/// Status message shown when another live Studio window already holds `session.lock` for this file.
pub const ANOTHER_WINDOW_OWNS_SESSION_MSG: &str = "Another window owns this file's agent session";

/// How a `TerminalState` is bound to disk.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SessionKey {
    /// Bound to a library file at `<lib_root>/agent-sessions/<file_id>/`.
    LibraryFile { lib_root: PathBuf, file_id: String },
    /// Ephemeral fallback at `/tmp/designos-sessions/<session_id>`.
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

impl SessionRegistry {
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
    /// Another live window owns `session.lock`; caller must use the fallback `/tmp/designos-sessions/...` dir.
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
    let fallback_dir = std::env::temp_dir().join("designos-sessions").join(fallback_id);
    let _ = std::fs::create_dir_all(&fallback_dir);
    LockOutcome::Fallback { dir: fallback_dir, message: ANOTHER_WINDOW_OWNS_SESSION_MSG.to_string() }
}

/// Acquires `<lib_root>/agent-sessions/<file_id>/session.lock` for `window`, taking over stale locks
/// whose `pid` is no longer alive (tested via `pid_alive`), or returns a fallback `/tmp/designos-sessions/<fallback_id>`
/// directory if another live window already owns the folder.
pub fn acquire_stable_session_dir_with<F>(lib_root: &Path, file_id: &str, window: &str, fallback_id: &str, pid_alive: F) -> LockOutcome
where
    F: Fn(u32) -> bool,
{
    let dir = stable_session_dir(lib_root, file_id);
    let _ = std::fs::create_dir_all(&dir);
    let lock_path = dir.join("session.lock");
    let my_pid = std::process::id();
    let lock = SessionLock { pid: my_pid, window: window.to_string(), started_at: unix_timestamp_secs() };
    let bytes = serde_json::to_vec_pretty(&lock).unwrap_or_default();

    if try_create_lock_exclusively(&lock_path, &bytes).is_ok() {
        return LockOutcome::Acquired(dir);
    }

    if let Ok(raw) = std::fs::read_to_string(&lock_path) {
        if let Ok(existing) = serde_json::from_str::<SessionLock>(&raw) {
            if existing.pid == my_pid && existing.window == window {
                return LockOutcome::Acquired(dir);
            }
            if pid_alive(existing.pid) {
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

/// Validates that `s` is a canonical 36-character ASCII hexadecimal UUID (`8-4-4-4-12`) so a
/// corrupted or tampered `session_id` in `sessions.json` can never inject shell tokens into
/// `zsh -lc` launch or resume commands.
pub fn is_valid_session_uuid(s: &str) -> bool {
    let bytes = s.as_bytes();
    if bytes.len() != 36 {
        return false;
    }
    for (i, &b) in bytes.iter().enumerate() {
        if matches!(i, 8 | 13 | 18 | 23) {
            if b != b'-' {
                return false;
            }
        } else if !b.is_ascii_hexdigit() {
            return false;
        }
    }
    true
}

/// True for the built-in agent CLIs (`claude`, `codex`, `grok`, `agy`, `gemini`, `opencode`) that can be remembered
/// as `last_cli` in `sessions.json` (`#652` §3 rule 4, `#679` §3). `Shell` and `DesignOsHelp` are excluded.
pub fn is_agent_cli_preset(preset: AgentCliPreset) -> bool {
    matches!(
        preset,
        AgentCliPreset::Claude | AgentCliPreset::Codex | AgentCliPreset::Grok | AgentCliPreset::Antigravity | AgentCliPreset::Gemini | AgentCliPreset::OpenCode
    )
}

/// True only for agent CLIs whose resume command is verified to be scoped to the session folder (`#653` §3 rules 4 & 6).
/// `Codex` (`codex resume --last`) and `Antigravity` (`agy --continue`) are gated on `codex_cwd_scoped`
/// and `agy_cwd_scoped` respectively; `Shell` and `DesignOsHelp` are never registered.
pub fn is_registrable_cli_scoped(preset: AgentCliPreset, codex_cwd_scoped: bool, agy_cwd_scoped: bool) -> bool {
    match preset {
        AgentCliPreset::Claude | AgentCliPreset::Grok | AgentCliPreset::Gemini | AgentCliPreset::OpenCode => true,
        AgentCliPreset::Codex => codex_cwd_scoped,
        AgentCliPreset::Antigravity => agy_cwd_scoped,
        AgentCliPreset::Shell | AgentCliPreset::DesignOsHelp => false,
    }
}

/// Default registrability check when neither `codex_cwd_scoped` nor `agy_cwd_scoped` has been verified.
pub fn is_registrable_cli(preset: AgentCliPreset) -> bool {
    is_registrable_cli_scoped(preset, false, false)
}

/// True for CLIs that accept `--session-id <uuid>` on new start (`Claude` and `Grok`).
/// `Codex`, `Antigravity`, `Gemini`, and `OpenCode` record `session_id: None` (`Rule 6`).
pub fn uses_preassigned_session_id(preset: AgentCliPreset) -> bool {
    matches!(preset, AgentCliPreset::Claude | AgentCliPreset::Grok)
}

/// Builds the interactive launch command for a brand-new session of `preset`.
/// For `Claude` and `Grok`, `session_uuid` is passed via `--session-id <uuid>` when it is a valid UUID.
pub fn build_new_session_command(preset: AgentCliPreset, session_uuid: Option<&str>) -> String {
    let valid_uuid = session_uuid.map(str::trim).filter(|s| is_valid_session_uuid(s));
    match preset {
        AgentCliPreset::Claude => match valid_uuid {
            Some(uuid) => format!(
                "claude --session-id {uuid} --dangerously-skip-permissions --mcp-config \"$DESIGNOS_MCP_CONFIG\" --settings \"$DESIGNOS_SESSION_DIR/hooks.json\" --append-system-prompt \"You are inside DESIGN:OS Studio bound exclusively to file '$DESIGNOS_DOC_NAME' (session $DESIGNOS_SESSION_DIR). Always read $DESIGNOS_CONTEXT_JSON for the current selected page (current_page_id / current_page_name) and deliver all designs onto that file and page via designos-mcp tools (designos_apply_spec, designos_create_node, designos_update_node) or by writing NodeSpec JSON to $DESIGNOS_LIVE_SPEC.\""
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
                "claude {resume_flag} --dangerously-skip-permissions --mcp-config \"$DESIGNOS_MCP_CONFIG\" --settings \"$DESIGNOS_SESSION_DIR/hooks.json\" --append-system-prompt \"You are inside DESIGN:OS Studio bound exclusively to file '$DESIGNOS_DOC_NAME' (session $DESIGNOS_SESSION_DIR). Always read $DESIGNOS_CONTEXT_JSON for the current selected page (current_page_id / current_page_name) and deliver all designs onto that file and page via designos-mcp tools (designos_apply_spec, designos_create_node, designos_update_node) or by writing NodeSpec JSON to $DESIGNOS_LIVE_SPEC.\""
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
        assert_eq!(app.terminal.session_dir, std::env::temp_dir().join("designos-sessions").join(&unsaved_sid));

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

        // Before `codex_cwd_scoped` / `agy_cwd_scoped` are verified, neither CLI records an entry in `sessions`.
        st.spawn_tab(AgentCliPreset::Codex, None).expect("spawn unscoped codex");
        st.spawn_tab(AgentCliPreset::Antigravity, None).expect("spawn unscoped agy");
        assert!(st.session_registry.sessions.is_empty(), "unverified codex/agy must not be registered");

        // Once `codex_cwd_scoped` is verified, Codex registers with `session_id: null` and no SQLite query.
        st.codex_cwd_scoped = true;
        st.spawn_tab(AgentCliPreset::Codex, None).expect("spawn scoped codex");

        assert_eq!(st.session_registry.sessions.len(), 1);
        assert_eq!(st.session_registry.sessions[0].cli, "codex");
        assert_eq!(st.session_registry.sessions[0].session_id, None);

        let raw = std::fs::read_to_string(st.session_dir.join("sessions.json")).expect("read sessions.json");
        assert!(raw.contains("\"session_id\": null"), "Codex entry must serialize session_id: null, got:\n{raw}");

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
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
            while std::time::Instant::now() < deadline {
                if std::fs::read_to_string(st1.session_dir.join("argv.log")).map(|s| s.lines().count() >= 1).unwrap_or(false) {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(25));
            }
            st1.shutdown();
            uuid
        };

        let mut st2 = TerminalState::for_tab(1);
        st2.bind_library_file(&dir, "file-restart-1", "p1-w2-tab-1");
        st2.path_prepend.push(bin_dir);
        assert_eq!(st2.resumable_sessions().len(), 1);
        assert_eq!(st2.resumable_sessions()[0].session_id.as_deref(), Some(first_uuid.as_str()));

        st2.resume_last_session(None).expect("resume last session");
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        let mut log_contents = String::new();
        while std::time::Instant::now() < deadline {
            log_contents = std::fs::read_to_string(st2.session_dir.join("argv.log")).unwrap_or_default();
            if log_contents.lines().count() >= 2 {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(25));
        }

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
    #[test]
    fn second_window_does_not_share_the_folder() {
        let dir = temp_test_dir("second-window-lock");
        let mut st1 = TerminalState::for_tab(1);
        st1.bind_library_file(&dir, "shared-file", "p1-w1-tab-1");
        assert_eq!(st1.session_dir, stable_session_dir(&dir, "shared-file"));
        assert!(st1.session_dir.join("session.lock").is_file());

        let mut st2 = TerminalState::for_tab(1);
        st2.bind_library_file(&dir, "shared-file", "p1-w2-tab-1");
        assert_ne!(st2.session_dir, st1.session_dir);
        assert_eq!(st2.session_dir, std::env::temp_dir().join("designos-sessions").join("p1-w2-tab-1"));
        assert_eq!(st2.status_msg.as_deref(), Some(ANOTHER_WINDOW_OWNS_SESSION_MSG));

        st1.shutdown();
        st2.shutdown();
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&st2.session_dir);
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
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(4);
        while std::time::Instant::now() < deadline {
            let _ = st.poll_live_agent_bridge();
            if st.active_tab().is_some_and(|t| t.start_failure.is_some()) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(30));
        }

        let failure = st.active_tab().and_then(|t| t.start_failure.as_ref()).expect("failure card must be present");
        assert_eq!(failure.fix, crate::agent_start_failure::FixAction::StartNewSession(AgentCliPreset::Claude));
        assert_eq!(failure.fix_label(), "Start a new session");

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

    // 14. `codex_registration_is_gated_on_cwd_scope`
    #[test]
    fn codex_registration_is_gated_on_cwd_scope() {
        assert!(!is_registrable_cli(AgentCliPreset::Codex), "Codex must not register until cwd scope is verified");
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

        // `open_for_agent_use` is the Agents-open path triggered by `RailItem::Agents` and the dock.
        st.open_for_agent_use(None);
        assert_eq!(st.tabs.len(), 1);
        assert_eq!(st.tabs[0].preset, AgentCliPreset::Claude, "must resume Claude instead of spawning Shell");
        assert!(st.tabs[0].is_resumed);

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        let mut argv_log = String::new();
        while std::time::Instant::now() < deadline {
            argv_log = std::fs::read_to_string(st.session_dir.join("argv.log")).unwrap_or_default();
            if !argv_log.is_empty() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(25));
        }
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
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        while resumes(&st) == 0 && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(25));
        }
        st.close_tab(0);
        assert!(st.tabs.is_empty(), "closing the last tab leaves no tab");
        assert!(st.picker_open, "closing the last tab shows the new-session picker");

        // Frames and the chat shell path must not bring the closed conversation back.
        st.ensure_shell_for_chat(None);
        st.close_tab(0);
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
}
