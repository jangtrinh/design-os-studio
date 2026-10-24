//! Live agent turn status stream (`agent-events.jsonl`), session-id attribution, tool step
//! phrases, and per-turn receipts (`#683`).

use std::collections::BTreeSet;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::sync::{Arc, Mutex};
use std::time::{Instant, SystemTime};

use serde::{Deserialize, Serialize};

use crate::agent_notify::NotifyKind;
use crate::terminal::{AgentCliPreset, AgentTabStatus};

/// Threshold in seconds after which a running turn with no active tool name transitions its
/// header text from `"Working…"` to `"Still working…"` (`#683` §3.4, §11).
pub const STILL_WORKING_THRESHOLD_SECS: f64 = 30.0;

/// Pinned lifecycle hook events registered in Claude (`hooks.json`) and Grok (`.grok/hooks/designos-status.json`)
/// alongside their legacy `agent-status.json` fallback state word (`#683` §3.1, §3.3, §5).
pub const CLAUDE_GROK_HOOK_EVENTS: &[(&str, &str)] = &[
    ("UserPromptSubmit", "working"),
    ("PreToolUse", "working"),
    ("PostToolUse", "working"),
    ("PermissionRequest", "waiting"),
    ("Notification", "waiting"),
    ("Stop", "done"),
    ("StopFailure", "error"),
    ("StopCancelled", "stopped"),
];

/// Pinned lifecycle hook events registered in Antigravity (`.agents/hooks.json`) (`#683` §5).
pub const AGY_HOOK_EVENTS: &[(&str, &str)] = &[("PreInvocation", "working"), ("Stop", "done")];

/// Escapes `s` for safe inclusion inside a single-quoted POSIX shell argument.
pub fn sh_single_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// Builds the shell command that appends one wrapped hook record
/// `{"t":<unix seconds>,"e":"<event>","p":<stdin payload>}` to `events_path` (`#683` §3.1).
///
/// The hook never parses JSON: `p=$(cat)` captures the raw payload from stdin in memory first, and
/// a single `printf` writes the complete record in one atomic `O_APPEND` syscall so concurrent
/// hooks never interleave bytes.
pub fn event_append_command(events_path: &Path, event: &str) -> String {
    event_append_command_for_tab(events_path, event, None)
}

/// [`event_append_command`] whose record also names the tab it was written for, as
/// `{"t":…,"e":"<event>","tab":"<id>","p":…}`. Gemini CLI's hooks use it, because their payload carries nothing that says
/// which tab a record belongs to. With `tab` `None` the command is exactly [`event_append_command`]'s.
pub fn event_append_command_for_tab(events_path: &Path, event: &str, tab: Option<usize>) -> String {
    let quoted_path = sh_single_quote(&events_path.display().to_string());
    let quoted_lock = sh_single_quote(&format!("{}.lock", events_path.display()));
    // Digits only, so it can sit inside the single-quoted `printf` format.
    let tab_field = tab.map(|id| format!("\"tab\":\"{id}\",")).unwrap_or_default();
    format!(
        "{{ p=$(tr '\\r\\n' '  '); [ -z \"$p\" ] && p=null; line=$(printf '{{\"t\":%s,\"e\":\"{event}\",{tab_field}\"p\":%s}}' \"$(date +%s)\" \"$p\"); got_lock=0; i=0; while [ $i -lt 500 ]; do if mkdir {quoted_lock} 2>/dev/null; then got_lock=1; break; fi; sleep 0.01; i=$((i+1)); done; printf '%s\\n' \"$line\" >> {quoted_path}; if [ \"$got_lock\" = 1 ]; then rmdir {quoted_lock} 2>/dev/null || true; fi; }}"
    )
}

/// One wrapped hook record read from `agent-events.jsonl` (`#683` §3.1).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AgentEventRecord {
    pub t: f64,
    pub e: String,
    /// The tab the hook was written for. Only Gemini CLI's hooks set it (`gemini_status_hooks`); every other CLI's
    /// records are matched to a tab by what their payload says.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tab: Option<String>,
    #[serde(default)]
    pub p: serde_json::Value,
}

impl AgentEventRecord {
    /// Extracts the CLI session identifier from the payload (`sessionId` in Grok, `session_id` in Claude Code).
    pub fn session_id(&self) -> Option<&str> {
        self.p
            .get("sessionId")
            .or_else(|| self.p.get("session_id"))
            .or_else(|| self.p.get("conversationId"))
            .or_else(|| self.p.get("conversation_id"))
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
    }

    /// Extracts the tool name from a `PreToolUse` / `PostToolUse` / `PermissionRequest` payload.
    pub fn tool_name(&self) -> Option<&str> {
        self.p
            .get("toolName")
            .or_else(|| self.p.get("tool_name"))
            .or_else(|| self.p.get("tool"))
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
    }

    /// Extracts the stop or cancellation reason (`reason` / `stopReason` / `stop_reason`).
    pub fn reason(&self) -> Option<&str> {
        self.p
            .get("reason")
            .or_else(|| self.p.get("stopReason"))
            .or_else(|| self.p.get("stop_reason"))
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
    }

    /// Extracts the error class from a `StopFailure` payload (`rate_limit`, `authentication_failed`,
    /// `invalid_request`, `server_error`, `max_output_tokens`, `unknown`).
    pub fn error_class(&self) -> Option<String> {
        let val = self.p.get("error").or_else(|| self.p.get("errorType")).or_else(|| self.p.get("error_type")).or_else(|| self.p.get("class"))?;
        if let Some(s) = val.as_str() {
            let trimmed = s.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
        if let Some(obj) = val.as_object() {
            for key in ["type", "class", "code", "name"] {
                if let Some(s) = obj.get(key).and_then(|v| v.as_str()).map(str::trim).filter(|s| !s.is_empty()) {
                    return Some(s.to_string());
                }
            }
        }
        None
    }

    /// Extracts the notification type from a `Notification` payload (e.g. `"permission_prompt"`).
    pub fn notification_type(&self) -> Option<&str> {
        self.p
            .get("notificationType")
            .or_else(|| self.p.get("notification_type"))
            .or_else(|| self.p.get("type"))
            .or_else(|| self.p.get("matcher"))
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
    }

    /// Extracts an explicit `layers_changed` count if the payload carries one (`#683` §3.4).
    pub fn layers_changed_in_payload(&self) -> Option<usize> {
        self.p.get("layers_changed").or_else(|| self.p.get("layersChanged")).and_then(|v| v.as_u64()).map(|n| n as usize)
    }
}

/// Parses complete top-level JSON objects from `buf` (supporting multi-line payloads inside `"p"`),
/// returning the parsed [`AgentEventRecord`]s and the byte offset up to which `buf` was consumed.
pub fn parse_streaming_records(buf: &str) -> (Vec<AgentEventRecord>, usize) {
    let mut records = Vec::new();
    let bytes = buf.as_bytes();
    let len = bytes.len();
    let mut idx = 0usize;
    let mut consumed = 0usize;

    while idx < len {
        while idx < len && bytes[idx].is_ascii_whitespace() {
            idx += 1;
        }
        if idx >= len {
            consumed = len;
            break;
        }
        if bytes[idx] != b'{' {
            // Skip malformed bytes up to the next newline so a corrupt line never stalls the stream.
            if let Some(nl) = bytes[idx..].iter().position(|&b| b == b'\n') {
                idx += nl + 1;
                consumed = idx;
                continue;
            } else {
                break;
            }
        }

        let start = idx;
        let mut depth = 0i32;
        let mut in_str = false;
        let mut escaped = false;
        let mut end_pos: Option<usize> = None;

        for (offset, &b) in bytes[start..].iter().enumerate() {
            if in_str {
                if escaped {
                    escaped = false;
                } else if b == b'\\' {
                    escaped = true;
                } else if b == b'"' {
                    in_str = false;
                }
                continue;
            }
            match b {
                b'"' => in_str = true,
                b'{' => depth += 1,
                b'}' => {
                    depth -= 1;
                    if depth == 0 {
                        end_pos = Some(start + offset);
                        break;
                    }
                }
                _ => {}
            }
        }

        if let Some(end) = end_pos {
            let raw_slice = &buf[start..=end];
            // When a hook runs with empty stdin (`/dev/null`), `cat` writes 0 bytes leaving `"p":}`.
            let normalized = if raw_slice.ends_with("\"p\":}") { format!("{}{{}}", &raw_slice[..raw_slice.len() - 1]) } else { raw_slice.to_string() };
            if let Ok(rec) = serde_json::from_str::<AgentEventRecord>(&normalized) {
                records.push(rec);
            }
            idx = end + 1;
            while idx < len && (bytes[idx] == b'\r' || bytes[idx] == b'\n' || bytes[idx] == b' ' || bytes[idx] == b'\t') {
                idx += 1;
            }
            consumed = idx;
        } else {
            // Incomplete multi-line record at EOF; wait for remaining bytes on next poll.
            break;
        }
    }

    (records, consumed)
}

type FileIdentity = (u64, Option<SystemTime>);

fn file_identity(meta: &std::fs::Metadata) -> FileIdentity {
    #[cfg(unix)]
    let ino = std::os::unix::fs::MetadataExt::ino(meta);
    #[cfg(not(unix))]
    let ino = 0;
    (ino, meta.created().ok())
}

#[derive(Default)]
struct EventFileCursor {
    path: PathBuf,
    offset: u64,
    identity: Option<FileIdentity>,
    partial: String,
}

impl EventFileCursor {
    fn bind(path: PathBuf) -> Self {
        let meta = crate::agent_session_files::session_file_metadata(&path).ok();
        Self { offset: meta.as_ref().map_or(0, |m| m.len()), identity: meta.as_ref().map(file_identity), partial: String::new(), path }
    }

    fn read_available(&mut self) -> Vec<AgentEventRecord> {
        let Ok(meta) = crate::agent_session_files::session_file_metadata(&self.path) else {
            return Vec::new();
        };
        let len = meta.len();
        let id = file_identity(&meta);
        if self.identity.is_some_and(|old| old != id) || len < self.offset {
            self.offset = 0;
            self.partial.clear();
        }
        self.identity = Some(id);
        if len <= self.offset && self.partial.is_empty() {
            return Vec::new();
        }
        if len > self.offset {
            if let Ok(mut file) = crate::agent_session_files::open_session_file(&self.path) {
                if file.seek(SeekFrom::Start(self.offset)).is_ok() {
                    let to_read = (len - self.offset).min(16 << 20) as usize;
                    let mut buf = vec![0u8; to_read];
                    if let Ok(n) = file.read(&mut buf) {
                        self.offset += n as u64;
                        self.partial.push_str(&String::from_utf8_lossy(&buf[..n]));
                    }
                }
            }
        }
        if self.partial.is_empty() {
            return Vec::new();
        }
        let (records, consumed) = parse_streaming_records(&self.partial);
        if consumed > 0 {
            self.partial.drain(..consumed);
        }
        records
    }
}

/// Off-UI-thread reader for `<session_dir>/agent-events.jsonl` (`#683` §3.1, §11).
///
/// The background worker sleeps on an `mpsc` channel when idle (`0.00%` CPU) and parses
/// appended JSON records off the UI thread when signalled; the UI thread drains a channel.
pub struct AgentEventStream {
    pub path: PathBuf,
    #[cfg(test)]
    cursor: Arc<Mutex<EventFileCursor>>,
    rx: Receiver<AgentEventRecord>,
    wake_tx: SyncSender<()>,
}

impl std::fmt::Debug for AgentEventStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentEventStream").field("path", &self.path).finish()
    }
}

impl AgentEventStream {
    pub fn bind(path: PathBuf) -> Self {
        let cursor = Arc::new(Mutex::new(EventFileCursor::bind(path.clone())));
        let (record_tx, record_rx) = mpsc::channel::<AgentEventRecord>();
        let (wake_tx, wake_rx) = mpsc::sync_channel::<()>(1);
        let cursor_bg = Arc::clone(&cursor);
        let _ = std::thread::Builder::new().name("designos-agent-events".into()).spawn(move || {
            while wake_rx.recv().is_ok() {
                // Read and send under the cursor lock. A reader that takes the lock afterwards (the inline drain
                // in `poll` under `cfg(test)`) then finds every record the worker consumed already in the channel;
                // sending after releasing the lock let a poll miss them and see them one poll late.
                let Ok(mut guard) = cursor_bg.lock() else { continue };
                let records = guard.read_available();
                for rec in records {
                    if record_tx.send(rec).is_err() {
                        return;
                    }
                }
            }
        });
        Self {
            path,
            #[cfg(test)]
            cursor,
            rx: record_rx,
            wake_tx,
        }
    }

    /// Signals the worker thread (when `has_eligible_tabs` is true) and drains parsed records.
    pub fn poll(&self, has_eligible_tabs: bool) -> Vec<AgentEventRecord> {
        if !has_eligible_tabs {
            return Vec::new();
        }
        let _ = self.wake_tx.try_send(());
        #[cfg(not(test))]
        {
            self.rx.try_iter().collect()
        }
        #[cfg(test)]
        {
            let mut out: Vec<AgentEventRecord> = self.rx.try_iter().collect();
            if let Ok(mut c) = self.cursor.lock() {
                out.extend(c.read_available());
            }
            out.extend(self.rx.try_iter());
            out
        }
    }
}

/// Snapshot of a tab's identity used for session-id event attribution (`#683` §3.2).
#[derive(Clone, Debug)]
pub struct TabAttributionInfo {
    pub idx: usize,
    /// The tab's own id (`TerminalTab::id`), which a tagged record names; `idx` is its place in the strip.
    pub tab_id: usize,
    pub preset: AgentCliPreset,
    pub is_alive: bool,
    pub session_uuid: Option<String>,
}

fn is_hook_driven_preset(preset: AgentCliPreset) -> bool {
    matches!(preset, AgentCliPreset::Claude | AgentCliPreset::Grok | AgentCliPreset::Antigravity)
}

fn infer_record_cli(record: &AgentEventRecord) -> Option<AgentCliPreset> {
    if record.e == "PreInvocation" {
        return Some(AgentCliPreset::Antigravity);
    }
    if let Some(cli) = record.p.get("cli").and_then(|v| v.as_str()) {
        return match cli.trim() {
            "claude" => Some(AgentCliPreset::Claude),
            "grok" => Some(AgentCliPreset::Grok),
            "agy" | "antigravity" => Some(AgentCliPreset::Antigravity),
            _ => None,
        };
    }
    if record.p.get("hookEventName").is_some() || record.p.get("sessionId").is_some() || record.p.get("toolName").is_some() {
        return Some(AgentCliPreset::Grok);
    }
    if record.p.get("session_id").is_some() || record.p.get("tool_name").is_some() {
        return Some(AgentCliPreset::Claude);
    }
    None
}

/// Routes an [`AgentEventRecord`] to the owning tab index (`#683` §3.2, §8.1, §8.2):
/// 0. A record that names a tab (Gemini CLI's hooks) goes to that tab when it is a live Gemini tab, else to none.
/// 1. Matches the tab whose `session_uuid` equals the payload's session id.
/// 2. If the record's id matches no tab (or has no session id), routes to the single live tab
///    of that CLI, else to none. Never routes to `active_tab` by default.
/// 3. Codex, Shell and Gemini tabs never consume an untagged record: a Gemini tab is left out of the tabs the rules above
///    count, so it can neither take another CLI's record nor make that CLI's single candidate two.
pub fn route_record_to_tab(record: &AgentEventRecord, tabs: &[TabAttributionInfo]) -> Option<usize> {
    if let Some(tag) = record.tab.as_deref() {
        return tabs.iter().find(|t| t.preset == AgentCliPreset::Gemini && t.is_alive && t.tab_id.to_string() == tag).map(|t| t.idx);
    }
    let eligible: Vec<&TabAttributionInfo> = tabs.iter().filter(|t| is_hook_driven_preset(t.preset)).collect();
    if eligible.is_empty() {
        return None;
    }

    if let Some(sid) = record.session_id() {
        if let Some(matched) = eligible.iter().find(|t| t.session_uuid.as_deref() == Some(sid)) {
            return Some(matched.idx);
        }
        // Record carries a session id that matches no tab: route to the only live tab of that CLI
        // (provided it does not already hold a different session id), else to none.
        let live: Vec<&&TabAttributionInfo> = eligible.iter().filter(|t| t.is_alive).collect();
        if let Some(cli) = infer_record_cli(record) {
            let cli_live: Vec<&&&TabAttributionInfo> = live.iter().filter(|t| t.preset == cli).collect();
            if cli_live.len() == 1 && cli_live[0].session_uuid.is_none() {
                return Some(cli_live[0].idx);
            }
            return None;
        }
        if live.len() == 1 && live[0].session_uuid.is_none() {
            return Some(live[0].idx);
        }
        return None;
    }

    // Payload without a session id (e.g. Antigravity): single live tab of that CLI gets it, else none.
    let live: Vec<&&TabAttributionInfo> = eligible.iter().filter(|t| t.is_alive).collect();
    if let Some(cli) = infer_record_cli(record) {
        let cli_live: Vec<&&&TabAttributionInfo> = live.iter().filter(|t| t.preset == cli).collect();
        return if cli_live.len() == 1 { Some(cli_live[0].idx) } else { None };
    }
    let idless_live: Vec<&&&TabAttributionInfo> = live.iter().filter(|t| t.session_uuid.is_none()).collect();
    if idless_live.len() == 1 {
        return Some(idless_live[0].idx);
    }
    if live.len() == 1 {
        return Some(live[0].idx);
    }
    None
}

/// Returns true when `tool_name` is one of Studio's layer-mutation tools (`#683` §3.4).
pub fn is_layer_mutation_tool(tool_name: Option<&str>) -> bool {
    let Some(raw) = tool_name.map(str::trim).filter(|s| !s.is_empty()) else {
        return false;
    };
    let stripped = raw.strip_prefix("mcp__designos__").or_else(|| raw.strip_prefix("designos__")).or_else(|| raw.strip_prefix("designos:")).unwrap_or(raw);
    matches!(
        stripped,
        "designos_apply_spec"
            | "designos_create_node"
            | "designos_update_node"
            | "designos_delete_node"
            | "designos_group_nodes"
            | "designos_ungroup_node"
            | "designos_boolean_op"
            | "designos_set_variable"
            | "designos_add_comment"
            | "designos_reply_comment"
            | "designos_resolve_comment"
            | "designos_set_dev_status"
            | "designos_undo"
            | "designos_redo"
    )
}

/// Maps the current tool name (and elapsed turn seconds) to the panel header status phrase (`#683` §3.4).
///
/// - Studio's layer-mutation tools -> `"Editing layers…"`
/// - Studio's inspection/read tools -> `"Reading the file…"`
/// - Studio's lint/audit tools -> `"Checking the design…"`
/// - Any other tool -> `"Using <tool name>…"`
/// - No tool step -> `"Working…"` (or `"Still working…"` after 30 s)
pub fn status_phrase_for_tool(tool_name: Option<&str>, elapsed_secs: f64) -> String {
    let Some(raw) = tool_name.map(str::trim).filter(|s| !s.is_empty()) else {
        return if elapsed_secs >= STILL_WORKING_THRESHOLD_SECS { "Still working…".to_string() } else { "Working…".to_string() };
    };
    if is_layer_mutation_tool(Some(raw)) {
        return "Editing layers…".to_string();
    }

    let stripped = raw.strip_prefix("mcp__designos__").or_else(|| raw.strip_prefix("designos__")).or_else(|| raw.strip_prefix("designos:")).unwrap_or(raw);

    match stripped {
        "designos_get_document"
        | "designos_list_nodes"
        | "designos_get_node"
        | "designos_get_variables"
        | "designos_query_xpath"
        | "designos_export_spec"
        | "designos_list_comments"
        | "designos_open_file"
        | "designos_save_file"
        | "designos_new_document"
        | "designos_export_fig_file"
        | "designos_render_png"
        | "designos_generate_code" => "Reading the file…".to_string(),
        "designos_lint" | "designos_check_design" | "designos_check_designs" | "designos_audit" | "designos_audit_tokens" => "Checking the design…".to_string(),
        _ => format!("Using {raw}…"),
    }
}

/// Formats a turn duration in seconds (`#683` §3.5, §8.9, §10):
/// rounded up to whole seconds, minimum `1s`; `Ns` under a minute, else `Nm Ns` (even past 60m, e.g. `61m 5s`).
pub fn format_duration_secs(elapsed_s: f64) -> String {
    let clean = if elapsed_s.is_nan() || elapsed_s <= 0.0 {
        1u64
    } else {
        let rounded_ns = (elapsed_s * 1e9).round() / 1e9;
        (rounded_ns.ceil() as u64).max(1)
    };
    if clean < 60 {
        format!("{clean}s")
    } else {
        let mins = clean / 60;
        let secs = clean % 60;
        format!("{mins}m {secs}s")
    }
}

/// How a turn ended (`#683` §3.5, §8.3).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TurnOutcome {
    Completed,
    StoppedByUser,
    Failed { class: String },
    EndedWithoutStop,
}

/// Per-turn receipt shown under the last turn (`#683` §3.5, §4, §7).
#[derive(Clone, Debug, PartialEq)]
pub struct TurnReceipt {
    pub started_s: f64,
    pub ended_s: f64,
    pub tool_calls: Option<usize>,
    pub layers_changed: Option<usize>,
    pub outcome: TurnOutcome,
}

impl TurnReceipt {
    /// Formats the receipt as a single caption line (`#683` §3.5, §8.3, §8.5):
    /// - Completed: `Worked for 1m 20s · 12 tool calls · 7 layers changed` (omitting any unknown count)
    /// - Stopped: `Stopped by you after 42s`
    /// - Failed: `Failed after 42s · <class label>` (the same label the screen reader announces)
    /// - Process exited without `Stop`: `Ended without reporting a result`
    pub fn format_line(&self) -> String {
        let dur = format_duration_secs(self.ended_s - self.started_s);
        match &self.outcome {
            TurnOutcome::Completed => {
                let mut line = format!("Worked for {dur}");
                if let Some(tc) = self.tool_calls {
                    let noun = if tc == 1 { "tool call" } else { "tool calls" };
                    line.push_str(&format!(" · {tc} {noun}"));
                }
                if let Some(lc) = self.layers_changed {
                    let noun = if lc == 1 { "layer changed" } else { "layers changed" };
                    line.push_str(&format!(" · {lc} {noun}"));
                }
                line
            }
            TurnOutcome::StoppedByUser => format!("Stopped by you after {dur}"),
            TurnOutcome::Failed { class } => format!("Failed after {dur} · {}", crate::agent_a11y::classify_error(Some(class)).as_str()),
            TurnOutcome::EndedWithoutStop => "Ended without reporting a result".to_string(),
        }
    }

    /// Splits [`Self::format_line`] into `(run_text, is_mono)` runs so numeric tokens render in
    /// `theme::font_mono()` and prose words render in `theme::font_caption()` (`#683` §7).
    pub fn styled_runs(&self) -> Vec<(String, bool)> {
        let line = self.format_line();
        let mut runs: Vec<(String, bool)> = Vec::new();
        for token in line.split_inclusive(' ') {
            let trimmed = token.trim();
            let is_num = !trimmed.is_empty() && trimmed.chars().next().is_some_and(|c| c.is_ascii_digit());
            if let Some(last) = runs.last_mut() {
                if last.1 == is_num {
                    last.0.push_str(token);
                    continue;
                }
            }
            runs.push((token.to_string(), is_num));
        }
        runs
    }
}

/// Drained status transition emitted for `#687` (`#683` §3.9).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StatusChange {
    pub tab: usize,
    pub from: AgentTabStatus,
    pub to: AgentTabStatus,
    pub class: Option<String>,
}

/// Result of applying one [`AgentEventRecord`] to a tab's [`TabTurnTracker`].
#[derive(Clone, Debug, Default)]
pub struct TurnEventEffect {
    pub new_status: Option<AgentTabStatus>,
    pub error_class: Option<String>,
    pub receipt: Option<TurnReceipt>,
    pub notify: Option<NotifyKind>,
    pub mark_unread_if_hidden: bool,
}

/// Per-tab turn state machine tracking active turn start time, tool count, layer edits, and duplicate-Stop suppression.
#[derive(Clone, Debug, Default)]
pub struct TabTurnTracker {
    pub in_turn: bool,
    pub has_completed_turn: bool,
    pub turn_started_s: Option<f64>,
    pub turn_started_instant: Option<Instant>,
    pub current_tool_name: Option<String>,
    pub turn_tool_calls: usize,
    pub turn_layers_changed: Option<usize>,
    pub touched_layers: BTreeSet<u64>,
    pub last_turn_receipt: Option<TurnReceipt>,
    pub summary_open: bool,
}

impl TabTurnTracker {
    fn ensure_turn_started(&mut self, t: f64) {
        if !self.in_turn {
            self.in_turn = true;
            self.turn_started_s = Some(t);
            self.turn_started_instant = Some(Instant::now());
            self.current_tool_name = None;
            self.turn_tool_calls = 0;
            self.turn_layers_changed = None;
            self.touched_layers.clear();
            self.summary_open = false;
        }
    }

    fn effective_layers_changed(&self, record: &AgentEventRecord) -> Option<usize> {
        if let Some(from_payload) = record.layers_changed_in_payload() {
            Some(from_payload)
        } else if let Some(explicit) = self.turn_layers_changed {
            Some(explicit)
        } else if !self.touched_layers.is_empty() {
            Some(self.touched_layers.len())
        } else {
            None
        }
    }

    /// Records distinct `NodeId`s modified on the document during this turn.
    pub fn record_touched_layers(&mut self, node_ids: impl IntoIterator<Item = u64>) {
        for id in node_ids {
            self.touched_layers.insert(id);
        }
        if !self.touched_layers.is_empty() {
            self.turn_layers_changed = Some(self.touched_layers.len());
        }
    }

    /// Explicitly sets the distinct layer count for the current turn (from `#677`'s turn ledger).
    pub fn set_turn_layers_changed(&mut self, count: usize) {
        self.turn_layers_changed = Some(count);
    }

    /// Current header status text while the tab is running (`#683` §3.4).
    pub fn running_header_text(&self, now_s: Option<f64>) -> String {
        let elapsed = match (self.turn_started_s, now_s) {
            (Some(start), Some(now)) if now >= start => now - start,
            _ => self.turn_started_instant.map(|i| i.elapsed().as_secs_f64()).unwrap_or(0.0),
        };
        status_phrase_for_tool(self.current_tool_name.as_deref(), elapsed)
    }

    /// Applies one hook event record and returns the resulting status, receipt, and notification action.
    pub fn on_event(&mut self, record: &AgentEventRecord) -> TurnEventEffect {
        if let Some(lc) = record.layers_changed_in_payload() {
            self.turn_layers_changed = Some(lc);
        }

        match record.e.as_str() {
            "UserPromptSubmit" | "PreInvocation" => {
                self.in_turn = false;
                self.ensure_turn_started(record.t);
                if let Some(lc) = record.layers_changed_in_payload() {
                    self.turn_layers_changed = Some(lc);
                }
                let phrase = status_phrase_for_tool(None, 0.0);
                TurnEventEffect { new_status: Some(AgentTabStatus::Working(Some(phrase))), ..Default::default() }
            }
            "PreToolUse" => {
                self.ensure_turn_started(record.t);
                self.turn_tool_calls += 1;
                self.current_tool_name = record.tool_name().map(str::to_string);
                let elapsed = (record.t - self.turn_started_s.unwrap_or(record.t)).max(0.0);
                let phrase = status_phrase_for_tool(self.current_tool_name.as_deref(), elapsed);
                TurnEventEffect { new_status: Some(AgentTabStatus::Working(Some(phrase))), ..Default::default() }
            }
            "PostToolUse" => {
                self.ensure_turn_started(record.t);
                self.current_tool_name = None;
                let elapsed = (record.t - self.turn_started_s.unwrap_or(record.t)).max(0.0);
                let phrase = status_phrase_for_tool(None, elapsed);
                TurnEventEffect { new_status: Some(AgentTabStatus::Working(Some(phrase))), ..Default::default() }
            }
            "PermissionRequest" => {
                self.ensure_turn_started(record.t);
                let detail = record.tool_name().map(str::to_string);
                TurnEventEffect { new_status: Some(AgentTabStatus::WaitingForInput(detail)), notify: Some(NotifyKind::NeedsApproval), ..Default::default() }
            }
            "Notification" => {
                // `permission_prompt`: Claude and Grok. `ToolPermission`: Gemini CLI, when it shows a tool-confirmation prompt.
                if matches!(record.notification_type(), Some("permission_prompt" | "ToolPermission")) {
                    self.ensure_turn_started(record.t);
                    let detail = record.tool_name().map(str::to_string);
                    TurnEventEffect { new_status: Some(AgentTabStatus::WaitingForInput(detail)), notify: Some(NotifyKind::NeedsApproval), ..Default::default() }
                } else {
                    TurnEventEffect::default()
                }
            }
            "Stop" => {
                // Edge case 4 (§8.4): a second `Stop` with no `UserPromptSubmit` in between is ignored.
                if !self.in_turn && self.has_completed_turn {
                    return TurnEventEffect::default();
                }
                if record.reason() == Some("user_interrupt") {
                    return self.finish_stopped(record);
                }
                let start = self.turn_started_s.unwrap_or(record.t);
                let layers = self.effective_layers_changed(record);
                let receipt = TurnReceipt {
                    started_s: start,
                    ended_s: record.t,
                    tool_calls: Some(self.turn_tool_calls),
                    layers_changed: layers,
                    outcome: TurnOutcome::Completed,
                };
                self.in_turn = false;
                self.has_completed_turn = true;
                self.current_tool_name = None;
                self.last_turn_receipt = Some(receipt.clone());
                TurnEventEffect {
                    new_status: Some(AgentTabStatus::Done),
                    receipt: Some(receipt),
                    notify: Some(NotifyKind::Finished),
                    mark_unread_if_hidden: true,
                    ..Default::default()
                }
            }
            "StopFailure" => {
                let start = self.turn_started_s.unwrap_or(record.t);
                let class = record.error_class().unwrap_or_else(|| "unknown".to_string());
                let layers = self.effective_layers_changed(record);
                let receipt = TurnReceipt {
                    started_s: start,
                    ended_s: record.t,
                    tool_calls: Some(self.turn_tool_calls),
                    layers_changed: layers,
                    outcome: TurnOutcome::Failed { class: class.clone() },
                };
                self.in_turn = false;
                self.has_completed_turn = true;
                self.current_tool_name = None;
                self.last_turn_receipt = Some(receipt.clone());
                TurnEventEffect {
                    new_status: Some(AgentTabStatus::Error(Some(class.clone()))),
                    error_class: Some(class),
                    receipt: Some(receipt),
                    notify: Some(NotifyKind::Finished),
                    mark_unread_if_hidden: true,
                }
            }
            "StopCancelled" => self.finish_stopped(record),
            _ => TurnEventEffect::default(),
        }
    }

    fn finish_stopped(&mut self, record: &AgentEventRecord) -> TurnEventEffect {
        let start = self.turn_started_s.unwrap_or(record.t);
        let layers = self.effective_layers_changed(record);
        let receipt = TurnReceipt {
            started_s: start,
            ended_s: record.t,
            tool_calls: Some(self.turn_tool_calls),
            layers_changed: layers,
            outcome: TurnOutcome::StoppedByUser,
        };
        self.in_turn = false;
        self.has_completed_turn = true;
        self.current_tool_name = None;
        self.last_turn_receipt = Some(receipt.clone());
        TurnEventEffect { new_status: Some(AgentTabStatus::Stopped), receipt: Some(receipt), ..Default::default() }
    }

    /// Marks the active turn stopped by the user (`#684` / `#683` §3.3, §3.5).
    pub fn stop_by_user(&mut self, now_s: f64) -> TurnReceipt {
        let start = self.turn_started_s.unwrap_or(now_s);
        let receipt = TurnReceipt {
            started_s: start,
            ended_s: now_s,
            tool_calls: Some(self.turn_tool_calls),
            layers_changed: self.turn_layers_changed.or_else(|| (!self.touched_layers.is_empty()).then_some(self.touched_layers.len())),
            outcome: TurnOutcome::StoppedByUser,
        };
        self.in_turn = false;
        self.has_completed_turn = true;
        self.current_tool_name = None;
        self.last_turn_receipt = Some(receipt.clone());
        receipt
    }

    /// Records that the user stopped the turn from the panel (`#684`): an active turn ends as stopped; a turn
    /// the CLI reported as completed at or after `stop_sent_s` (the hook fired before the panel noticed) is
    /// re-labelled stopped, but only when that turn began before the stop (a later turn is not the one stopped).
    /// Any other receipt is left alone. Returns the receipt now standing for the stop.
    pub fn adopt_stop_by_user(&mut self, stop_sent_s: f64, now_s: f64) -> Option<TurnReceipt> {
        if self.in_turn {
            return Some(self.stop_by_user(now_s));
        }
        // Hook timestamps are whole seconds, so allow one second of slack.
        let receipt =
            self.last_turn_receipt.as_mut().filter(|r| r.outcome == TurnOutcome::Completed && r.started_s <= stop_sent_s && r.ended_s + 1.0 >= stop_sent_s)?;
        receipt.outcome = TurnOutcome::StoppedByUser;
        Some(receipt.clone())
    }

    /// Edge case 3 (`#683` §8.3): when the agent process exits while a turn is still running without
    /// having fired `Stop`, leaves running and records `"Ended without reporting a result"`.
    pub fn on_process_exit(&mut self, now_s: f64, current_status: &AgentTabStatus) -> Option<(AgentTabStatus, TurnReceipt)> {
        let was_running = self.in_turn || matches!(current_status, AgentTabStatus::Working(_) | AgentTabStatus::WaitingForInput(_));
        if !was_running {
            return None;
        }
        let start = self.turn_started_s.unwrap_or(now_s);
        let receipt = TurnReceipt {
            started_s: start,
            ended_s: now_s,
            tool_calls: Some(self.turn_tool_calls),
            layers_changed: self.turn_layers_changed.or_else(|| (!self.touched_layers.is_empty()).then_some(self.touched_layers.len())),
            outcome: TurnOutcome::EndedWithoutStop,
        };
        self.in_turn = false;
        self.current_tool_name = None;
        self.last_turn_receipt = Some(receipt.clone());
        Some((AgentTabStatus::Done, receipt))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_failed_receipt_names_the_class_in_words_not_as_a_hook_slug() {
        let failed = |class: &str| TurnReceipt {
            started_s: 100.0,
            ended_s: 142.0,
            tool_calls: None,
            layers_changed: None,
            outcome: TurnOutcome::Failed { class: class.to_string() },
        };
        assert_eq!(failed("rate_limit").format_line(), "Failed after 42s \u{00b7} rate limited");
        assert_eq!(failed("authentication_failed").format_line(), "Failed after 42s \u{00b7} login expired");
        assert_eq!(failed("unknown").format_line(), "Failed after 42s \u{00b7} unknown error");
    }

    #[test]
    fn streaming_parser_handles_multiline_and_incomplete_records() {
        let input =
            concat!("{\"t\":1.0,\"e\":\"PreToolUse\",\"p\":{\"tool_name\":\"Read\",\n\"note\":\"line1\\nline2\"}}\n", "{\"t\":2.0,\"e\":\"Stop\",\"p\":{}}");
        let (records, consumed) = parse_streaming_records(input);
        assert_eq!(records.len(), 2);
        assert_eq!(consumed, input.len());
        assert_eq!(records[0].e, "PreToolUse");
        assert_eq!(records[0].tool_name(), Some("Read"));
        assert_eq!(records[1].e, "Stop");

        let partial = "{\"t\":1.0,\"e\":\"Stop\",\"p\":{\"a\":1";
        let (records2, consumed2) = parse_streaming_records(partial);
        assert!(records2.is_empty());
        assert_eq!(consumed2, 0);
    }

    #[test]
    fn status_phrase_maps_studio_and_generic_tools() {
        assert_eq!(status_phrase_for_tool(Some("mcp__designos__designos_create_node"), 2.0), "Editing layers…");
        assert_eq!(status_phrase_for_tool(Some("designos_get_document"), 2.0), "Reading the file…");
        assert_eq!(status_phrase_for_tool(Some("designos_audit_tokens"), 2.0), "Checking the design…");
        assert_eq!(status_phrase_for_tool(Some("Bash"), 2.0), "Using Bash…");
        assert_eq!(status_phrase_for_tool(None, 5.0), "Working…");
        assert_eq!(status_phrase_for_tool(None, 30.0), "Still working…");
    }
}
