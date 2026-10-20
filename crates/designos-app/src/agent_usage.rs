//! Per-tab CLI token and cost usage readers, formatters, and background probes (`#678`).
//!
//! Enforces the `#678` honesty rules:
//! 1. **Show only what a source proves**: only [`UsageSource::ClaudeTranscript`],
//!    [`UsageSource::ClaudeStatusLine`], and [`UsageSource::GrokUsage`] may produce a [`UsageReading`].
//!    No token or cost figure is ever estimated from prompt length, terminal output length, or time.
//! 2. **Silent hide on missing/unverified sources**: Codex (`0.163.0`), Antigravity (`1.4.2`), and
//!    plain shells return `None` and draw nothing in the header strip (never `0` or `-`).
//! 3. **Incremental, off-UI-thread I/O**:
//!    - Claude transcripts are captured once per session via hook stdin into
//!      `<session_dir>/claude-transcript.json`, then tailed incrementally by byte offset and
//!      deduplicated by `message.id` (keeping the last usage entry per `message.id`).
//!    - Grok usage is queried via `grok usage <session-id>` on a background thread with a 5-second
//!      timeout when a turn ends, when a tab is focused, or when the panel opens.

use crate::terminal::AgentCliPreset;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// File in `<session_dir>` where the Claude hook records `{"session_id", "transcript_path"}` once per session.
pub const CLAUDE_TRANSCRIPT_META_FILENAME: &str = "claude-transcript.json";

/// Optional cache file in `<session_dir>` storing the last known [`UsageReading`] so resumed tabs show usage immediately.
pub const USAGE_CACHE_FILENAME: &str = "usage.json";

/// Hard timeout for a background `grok usage <session-id>` subprocess (`#678` §3.5, §10).
pub const GROK_USAGE_TIMEOUT: Duration = Duration::from_secs(5);

/// Minimum interval between incremental transcript polls while a Claude tab is actively `Working` (`#678` §11).
pub const WORKING_POLL_INTERVAL: Duration = Duration::from_secs(5);

/// Maximum bytes read from a transcript file in a single incremental poll (64 MiB).
const MAX_TRANSCRIPT_POLL_BYTES: u64 = 64 << 20;

/// Leading bytes remembered to detect in-place transcript truncation/rewrite.
const HEAD_BYTES: usize = 256;

/// Authoritative source of a [`UsageReading`] (`#678` §4).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum UsageSource {
    ClaudeTranscript,
    ClaudeStatusLine,
    GrokUsage,
}

impl UsageSource {
    /// All documented `UsageSource` variants (`#678` §10 `sources_document_matches_code`).
    pub const ALL: [UsageSource; 3] = [UsageSource::ClaudeTranscript, UsageSource::ClaudeStatusLine, UsageSource::GrokUsage];

    /// Exact Rust identifier of this variant as recorded in `docs/agent-cli-usage-sources.md`.
    pub fn variant_name(self) -> &'static str {
        match self {
            UsageSource::ClaudeTranscript => "ClaudeTranscript",
            UsageSource::ClaudeStatusLine => "ClaudeStatusLine",
            UsageSource::GrokUsage => "GrokUsage",
        }
    }

    /// Human-readable provenance line for the tab tooltip (`#678` §7).
    pub fn tooltip_label(self) -> &'static str {
        match self {
            UsageSource::ClaudeTranscript => "From Claude's transcript",
            UsageSource::ClaudeStatusLine => "From Claude's statusLine",
            UsageSource::GrokUsage => "From grok usage",
        }
    }
}

/// Labelled session cost in USD (`#678` §3.3, §4). Never rendered as a bare `$`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum CostLabel {
    /// Client-side list-price estimate (e.g. Claude `statusLine` `cost.total_cost_usd`), labelled `est.`.
    Estimate(f64),
    /// List-price reference (e.g. Grok `costUsdTicks / 1e10`), labelled `ref.`.
    Reference(f64),
}

impl CostLabel {
    /// Raw USD amount.
    pub fn usd(self) -> f64 {
        match self {
            CostLabel::Estimate(v) | CostLabel::Reference(v) => v,
        }
    }

    /// Suffix tag (`"est."` or `"ref."`).
    pub fn suffix(self) -> &'static str {
        match self {
            CostLabel::Estimate(_) => "est.",
            CostLabel::Reference(_) => "ref.",
        }
    }

    /// Short caption segment, e.g. `"$0.42 ref."` or `"$0.42 est."`.
    pub fn format_short(self) -> String {
        format!("{} {}", format_usd_amount(self.usd()), self.suffix())
    }

    /// Mandatory explanation string included in the tooltip (`#678` §7).
    pub fn tooltip_explanation(self) -> &'static str {
        match self {
            CostLabel::Reference(_) => "list-price reference, not a bill",
            CostLabel::Estimate(_) => "estimated session cost in USD, computed client-side at list price",
        }
    }
}

/// Cumulative session token and optional cost reading for a single agent tab (`#678` §4).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UsageReading {
    /// Cumulative input tokens for the session (including cached reads and cache writes where reported).
    pub input: u64,
    /// Cumulative output tokens for the session (including reasoning tokens where reported).
    pub output: u64,
    /// Optional labelled session cost (`est.` or `ref.`).
    pub cost: Option<CostLabel>,
    /// Which verified CLI source produced this reading.
    pub source: UsageSource,
    /// Unix epoch timestamp in milliseconds when this reading was taken.
    pub as_of_ms: u64,
}

impl UsageReading {
    /// True if `self.source` is one of the three documented [`UsageSource`] variants.
    pub fn has_documented_source(&self) -> bool {
        UsageSource::ALL.contains(&self.source)
    }

    /// Formats the compact header/tab caption (`"in 12.4k · out 3.1k"` or `"in 12.4k · out 3.1k · $0.42 ref."`).
    pub fn format_caption(&self) -> String {
        let base = format!("in {} · out {}", format_compact_tokens(self.input), format_compact_tokens(self.output));
        match self.cost {
            Some(cost) if cost.usd().is_finite() && cost.usd() >= 0.0 => {
                format!("{base} · {}", cost.format_short())
            }
            _ => base,
        }
    }

    /// Formats the multi-line tab tooltip describing the reading, its source, timestamp, and cost disclaimer (`#678` §7).
    pub fn format_tooltip(&self) -> String {
        let mut lines = Vec::with_capacity(4);
        lines.push(self.format_caption());
        lines.push(self.source.tooltip_label().to_string());
        lines.push(format!("As of {}", format_timestamp_ms(self.as_of_ms)));
        if let Some(cost) = self.cost {
            lines.push(cost.tooltip_explanation().to_string());
        }
        lines.join("\n")
    }
}

/// Returns current Unix timestamp in milliseconds.
pub fn now_unix_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// Formats a token count in compact units (`0`, `850`, `12.4k`, `1.5M`) (`#678` §3.2).
pub fn format_compact_tokens(tokens: u64) -> String {
    if tokens < 1_000 {
        tokens.to_string()
    } else if tokens < 1_000_000 {
        let k = tokens as f64 / 1_000.0;
        let s = format!("{k:.1}");
        format!("{}k", s.strip_suffix(".0").unwrap_or(&s))
    } else {
        let m = tokens as f64 / 1_000_000.0;
        let s = format!("{m:.1}");
        format!("{}M", s.strip_suffix(".0").unwrap_or(&s))
    }
}

fn format_usd_amount(usd: f64) -> String {
    let clamped = if usd.is_finite() && usd >= 0.0 { usd } else { 0.0 };
    if clamped > 0.0 && clamped < 0.01 {
        format!("${clamped:.4}")
    } else {
        format!("${clamped:.2}")
    }
}

fn format_timestamp_ms(as_of_ms: u64) -> String {
    if as_of_ms == 0 {
        return "just now".to_string();
    }
    let total_secs = as_of_ms / 1_000;
    let secs_of_day = total_secs % 86_400;
    let hours = secs_of_day / 3_600;
    let minutes = (secs_of_day % 3_600) / 60;
    let seconds = secs_of_day % 60;
    format!("{hours:02}:{minutes:02}:{seconds:02} UTC")
}

/// Returns `true` only for CLI presets that have a verified, usable usage source in `#678` (`Claude` and `Grok`).
/// `Codex` (`0.163.0` unverified), `Antigravity` (`1.4.2` no source), and `Shell` return `false`.
pub fn cli_has_usable_usage_source(preset: AgentCliPreset) -> bool {
    matches!(preset, AgentCliPreset::Claude | AgentCliPreset::Grok)
}

/// Header strip readout for a tab. Returns `None` when the CLI has no usable source or when no valid
/// reading is available — never returns `"0"` or `"-"`.
pub fn header_usage_caption_for_tab(preset: AgentCliPreset, reading: Option<&UsageReading>) -> Option<String> {
    if !cli_has_usable_usage_source(preset) {
        return None;
    }
    let r = reading?;
    if !r.has_documented_source() {
        return None;
    }
    Some(r.format_caption())
}

/// Tooltip text for a tab (`#678` §3.7, §7).
/// - For CLIs without a usable source (`Codex`, `Antigravity`, `Shell`), returns `None`.
/// - For `Claude` / `Grok` with a valid reading, returns the formatted tooltip (caption, source, time, cost note).
/// - For `Claude` / `Grok` when the reading is missing/unreadable/timed out, returns `Some("Usage not available")`.
pub fn tooltip_usage_text_for_tab(preset: AgentCliPreset, reading: Option<&UsageReading>) -> Option<String> {
    if !cli_has_usable_usage_source(preset) {
        return None;
    }
    match reading {
        Some(r) if r.has_documented_source() => Some(r.format_tooltip()),
        _ => Some("Usage not available".to_string()),
    }
}

/// Metadata written by the Claude hook capture command into `<session_dir>/claude-transcript.json`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClaudeTranscriptMeta {
    pub session_id: String,
    pub transcript_path: PathBuf,
    #[serde(default)]
    pub sessions: BTreeMap<String, PathBuf>,
}

/// Builds the POSIX shell command that captures `session_id` and `transcript_path` from a Claude hook's
/// stdin JSON into `capture_path` (`<session_dir>/claude-transcript.json`) **once per session** (`#678` §3.4, §5, §8.2).
///
/// Keys the `"sessions"` map by `DESIGNOS_TAB_SESSION_UUID` when exported in the tab's PTY environment
/// (falling back to `session_id`), overwriting that tab's entry when `/clear` rotates `session_id` to a new
/// transcript file, and writes via an atomic temporary file under a lock directory so concurrent tabs cannot
/// clobber each other.
pub fn claude_transcript_capture_command(capture_path: &Path) -> String {
    let dest = capture_path.display();
    format!(
        "in=$(tr -d '\\r\\n'); \
         sid=$(printf '%s' \"$in\" | sed -n 's/.*\"session_id\"[[:space:]]*:[[:space:]]*\"\\([^\"]*\\)\".*/\\1/p'); \
         tp=$(printf '%s' \"$in\" | sed -n 's/.*\"transcript_path\"[[:space:]]*:[[:space:]]*\"\\([^\"]*\\)\".*/\\1/p'); \
         if [ -n \"$sid\" ] && [ -n \"$tp\" ]; then \
           key=\"${{DESIGNOS_TAB_SESSION_UUID:-$sid}}\"; \
           pair=\"\\\"$key\\\":\\\"$tp\\\"\"; \
           if [ ! -f '{dest}' ] || ! grep -Fq \"$pair\" '{dest}' 2>/dev/null; then \
             i=0; while ! mkdir '{dest}.lock' 2>/dev/null && [ $i -lt 50 ]; do i=$((i+1)); sleep 0.01; done; \
             prev=\"\"; \
             if [ -f '{dest}' ]; then \
               prev=$(sed -n 's/.*\"sessions\":{{//; s/}}}}[[:space:]]*$//p' '{dest}' 2>/dev/null); \
               prev=$(printf '%s' \"$prev\" | sed \"s/\\\"$key\\\":\\\"[^\\\"]*\\\"//g; s/,,/,/g; s/^,//; s/,$//\"); \
             fi; \
             if [ -n \"$prev\" ]; then \
               entries=\"$prev,$pair\"; \
             else \
               entries=\"$pair\"; \
             fi; \
             tmp='{dest}.tmp.'$$; \
             if printf '{{\"session_id\":\"%s\",\"transcript_path\":\"%s\",\"sessions\":{{%s}}}}\\n' \"$sid\" \"$tp\" \"$entries\" > \"$tmp\" 2>/dev/null; then \
               mv -f \"$tmp\" '{dest}' 2>/dev/null || rm -f \"$tmp\"; \
             fi; \
             rmdir '{dest}.lock' 2>/dev/null || true; \
           fi; \
         fi"
    )
}

/// Reads `<session_dir>/claude-transcript.json` if present and valid.
pub fn read_claude_transcript_meta(capture_path: &Path) -> Option<ClaudeTranscriptMeta> {
    let raw = std::fs::read_to_string(capture_path).ok()?;
    let meta: ClaudeTranscriptMeta = serde_json::from_str(raw.trim()).ok()?;
    if meta.session_id.trim().is_empty() || meta.transcript_path.as_os_str().is_empty() {
        return None;
    }
    Some(meta)
}

/// Reads `<session_dir>/claude-transcript.json` scoped to `target_session_id` when provided.
///
/// When `target_session_id` is `Some(sid)`, resolves `sid` from `meta.sessions` (or `meta.session_id == sid`),
/// and returns `None` if `capture_path` only records a different tab's session.
pub fn read_claude_transcript_meta_for(capture_path: &Path, target_session_id: Option<&str>) -> Option<ClaudeTranscriptMeta> {
    let meta = read_claude_transcript_meta(capture_path)?;
    if let Some(sid) = target_session_id.map(str::trim).filter(|s| !s.is_empty()) {
        if let Some(tp) = meta.sessions.get(sid) {
            if !tp.as_os_str().is_empty() {
                let resolved_session_id = if meta.transcript_path == *tp { meta.session_id } else { sid.to_string() };
                return Some(ClaudeTranscriptMeta { session_id: resolved_session_id, transcript_path: tp.clone(), sessions: meta.sessions });
            }
        }
        if meta.session_id == sid {
            return Some(meta);
        }
        return None;
    }
    Some(meta)
}

type FileIdentity = (u64, Option<SystemTime>);

fn file_identity(meta: &std::fs::Metadata) -> FileIdentity {
    #[cfg(unix)]
    let ino = std::os::unix::fs::MetadataExt::ino(meta);
    #[cfg(not(unix))]
    let ino = 0;
    (ino, meta.created().ok())
}

/// Incremental, offset-tracking reader for a Claude Code `.jsonl` transcript file (`#678` §3.4, §8.1–8.8).
///
/// Deduplicates assistant usage entries by `message.id`, keeping the **last** usage seen for each `message.id`.
#[derive(Debug, Clone)]
pub struct ClaudeTranscriptReader {
    pub path: PathBuf,
    pub session_id: Option<String>,
    offset: u64,
    identity: Option<FileIdentity>,
    head: Vec<u8>,
    /// Maps `message.id` -> `(input_total, output_total)` keeping the last value seen for each `message.id`.
    messages: BTreeMap<String, (u64, u64)>,
    bytes_read_last_poll: u64,
    last_as_of_ms: u64,
}

impl ClaudeTranscriptReader {
    /// Creates a new reader bound to `path` starting at byte offset `0` so resumed transcripts
    /// include earlier turns from the same session (`#678` §8.1).
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            session_id: None,
            offset: 0,
            identity: None,
            head: Vec::new(),
            messages: BTreeMap::new(),
            bytes_read_last_poll: 0,
            last_as_of_ms: 0,
        }
    }

    /// Updates the target `(session_id, transcript_path)` from `claude-transcript.json`.
    /// When `session_id` or `transcript_path` changes (e.g. `/clear` in `#678` §8.2), resets the offset
    /// and clears previously accumulated `message.id` counts, returning `true` if a non-empty previous
    /// target rotated to a new session or transcript path.
    pub fn set_target(&mut self, session_id: Option<String>, path: PathBuf) -> bool {
        let had_previous = !self.path.as_os_str().is_empty();
        let session_changed = session_id.is_some() && self.session_id.is_some() && session_id != self.session_id;
        let path_changed = self.path != path;
        let rotated = had_previous && (session_changed || path_changed);
        if session_changed || path_changed {
            self.path = path;
            self.offset = 0;
            self.identity = None;
            self.head.clear();
            self.messages.clear();
            self.bytes_read_last_poll = 0;
            self.last_as_of_ms = 0;
        }
        if session_id.is_some() {
            self.session_id = session_id;
        }
        rotated
    }

    /// Current byte offset in the transcript file.
    pub fn offset(&self) -> u64 {
        self.offset
    }

    /// Number of new bytes consumed during the most recent [`Self::poll`] call.
    pub fn bytes_read_last_poll(&self) -> u64 {
        self.bytes_read_last_poll
    }

    /// Number of distinct `message.id` entries accumulated so far.
    pub fn distinct_message_count(&self) -> usize {
        self.messages.len()
    }

    /// Polls the transcript file for newly appended lines since `self.offset`.
    /// Returns `None` if the transcript is missing, unreadable, or contains no valid assistant usage entries.
    pub fn poll(&mut self) -> Option<UsageReading> {
        self.bytes_read_last_poll = 0;
        if self.path.as_os_str().is_empty() {
            return None;
        }
        let Ok(meta) = std::fs::metadata(&self.path) else {
            self.offset = 0;
            self.identity = None;
            self.head.clear();
            self.messages.clear();
            return None;
        };
        let (len, id) = (meta.len(), file_identity(&meta));
        let rewritten = self.identity.is_some_and(|old| old != id) || len < self.offset || !self.head_unchanged();
        if rewritten {
            self.offset = 0;
            self.messages.clear();
        }
        self.identity = Some(id);
        if rewritten || self.head.len() < HEAD_BYTES {
            self.head = self.read_range(0, HEAD_BYTES as u64).unwrap_or_default();
        }
        if len > self.offset {
            let to_read = (len - self.offset).min(MAX_TRANSCRIPT_POLL_BYTES);
            let buf = self.read_range(self.offset, to_read)?;
            let consumed = self.ingest_lines(&buf);
            self.offset += consumed as u64;
            self.bytes_read_last_poll = consumed as u64;
            if consumed > 0 {
                self.last_as_of_ms = now_unix_ms();
            }
        }
        self.current_reading()
    }

    /// Returns the current deduplicated [`UsageReading`] without touching the filesystem.
    pub fn current_reading(&self) -> Option<UsageReading> {
        if self.messages.is_empty() {
            return None;
        }
        let (mut total_in, mut total_out) = (0u64, 0u64);
        for &(inp, out) in self.messages.values() {
            total_in = total_in.saturating_add(inp);
            total_out = total_out.saturating_add(out);
        }
        let as_of_ms = if self.last_as_of_ms > 0 { self.last_as_of_ms } else { now_unix_ms() };
        Some(UsageReading { input: total_in, output: total_out, cost: None, source: UsageSource::ClaudeTranscript, as_of_ms })
    }

    fn ingest_lines(&mut self, buf: &[u8]) -> usize {
        let mut pos = 0;
        while let Some(nl) = buf[pos..].iter().position(|&b| b == b'\n') {
            let line = &buf[pos..pos + nl];
            pos += nl + 1;
            if line.iter().all(u8::is_ascii_whitespace) {
                continue;
            }
            if let Some((msg_id, inp, out)) = parse_claude_transcript_line(line) {
                self.messages.insert(msg_id, (inp, out));
            }
        }
        pos
    }

    fn head_unchanged(&self) -> bool {
        if self.head.is_empty() {
            return true;
        }
        self.read_range(0, self.head.len() as u64).is_some_and(|now| now == self.head)
    }

    fn read_range(&self, start: u64, len: u64) -> Option<Vec<u8>> {
        if len == 0 {
            return Some(Vec::new());
        }
        let mut file = std::fs::File::open(&self.path).ok()?;
        file.seek(SeekFrom::Start(start)).ok()?;
        let mut buf = Vec::with_capacity(len.min(64 << 10) as usize);
        file.take(len).read_to_end(&mut buf).ok()?;
        Some(buf)
    }
}

/// Parses a single JSONL line from a Claude transcript, extracting `(message_id, total_input_tokens, output_tokens)`.
///
/// Total input tokens follow `#678` §2 & §3.2:
/// `input_tokens + cache_creation_input_tokens + cache_read_input_tokens`.
pub fn parse_claude_transcript_line(line: &[u8]) -> Option<(String, u64, u64)> {
    let val: serde_json::Value = serde_json::from_slice(line).ok()?;
    let obj = val.as_object()?;
    let msg_obj = obj.get("message").and_then(|v| v.as_object());

    let msg_id = msg_obj.and_then(|m| m.get("id")).and_then(|v| v.as_str()).or_else(|| obj.get("id").and_then(|v| v.as_str()))?.trim();
    if msg_id.is_empty() {
        return None;
    }

    let usage = msg_obj.and_then(|m| m.get("usage")).or_else(|| obj.get("usage"))?.as_object()?;

    let input_tokens = usage.get("input_tokens").and_then(|v| v.as_u64());
    let output_tokens = usage.get("output_tokens").and_then(|v| v.as_u64());
    if input_tokens.is_none() && output_tokens.is_none() {
        return None;
    }
    let cache_creation = usage.get("cache_creation_input_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
    let cache_read = usage.get("cache_read_input_tokens").and_then(|v| v.as_u64()).unwrap_or(0);

    let total_in = input_tokens.unwrap_or(0).saturating_add(cache_creation).saturating_add(cache_read);
    let total_out = output_tokens.unwrap_or(0);

    Some((msg_id.to_string(), total_in, total_out))
}

/// Parses a Claude `statusLine` stdin JSON payload (`#678` §2 row 2, `UsageSource::ClaudeStatusLine`).
pub fn parse_claude_status_line_json(raw: &str, as_of_ms: u64) -> Option<UsageReading> {
    let val: serde_json::Value = serde_json::from_str(raw.trim()).ok()?;
    let cw = val.get("context_window")?.as_object()?;
    let input = cw.get("total_input_tokens")?.as_u64()?;
    let output = cw.get("total_output_tokens")?.as_u64()?;
    let cost =
        val.get("cost").and_then(|c| c.get("total_cost_usd")).and_then(|v| v.as_f64()).filter(|&usd| usd.is_finite() && usd > 0.0).map(CostLabel::Estimate);
    Some(UsageReading { input, output, cost, source: UsageSource::ClaudeStatusLine, as_of_ms })
}

/// Parses the JSON stdout of `grok usage <session-id>` (`#678` §2 row 3, §3.2, §3.3, §3.5, §8.3).
///
/// - Reads `session.inputTokens`, `session.outputTokens`, `session.cachedReadTokens`,
///   `session.cacheCreationTokens`, `session.reasoningTokens`, `session.totalTokens`, and
///   optional `session.costUsdTicks`.
/// - `costUsdTicks` is divided by `1e10` (`10^10` ticks per USD) and wrapped in [`CostLabel::Reference`].
/// - When `costUsdTicks` is absent or `null`, `cost` is `None` and tokens are still returned.
pub fn parse_grok_usage_json(raw: &str, as_of_ms: u64) -> Option<UsageReading> {
    let val: serde_json::Value = serde_json::from_str(raw.trim()).ok()?;
    let session = val.get("session")?.as_object()?;

    let input_tokens = session.get("inputTokens")?.as_u64()?;
    let output_tokens = session.get("outputTokens")?.as_u64()?;
    let cached_read = session.get("cachedReadTokens").and_then(|v| v.as_u64()).unwrap_or(0);
    let cache_creation = session.get("cacheCreationTokens").and_then(|v| v.as_u64()).unwrap_or(0);
    let reasoning = session.get("reasoningTokens").and_then(|v| v.as_u64()).unwrap_or(0);
    let total_tokens = session.get("totalTokens").and_then(|v| v.as_u64());

    let additive_in = input_tokens.saturating_add(cached_read).saturating_add(cache_creation);
    let additive_out = output_tokens.saturating_add(reasoning);

    let (input, output) = match total_tokens {
        Some(total) if total == additive_in.saturating_add(additive_out) => (additive_in, additive_out),
        Some(total) if total == input_tokens.saturating_add(output_tokens) => (input_tokens, output_tokens),
        None if cached_read > 0 || cache_creation > 0 || reasoning > 0 => (additive_in, additive_out),
        _ => (additive_in, additive_out),
    };

    let cost = session
        .get("costUsdTicks")
        .and_then(|v| v.as_u64().map(|ticks| ticks as f64).or_else(|| v.as_f64()))
        .filter(|&ticks| ticks.is_finite() && ticks > 0.0)
        .map(|ticks| CostLabel::Reference(ticks / 1e10));

    Some(UsageReading { input, output, cost, source: UsageSource::GrokUsage, as_of_ms })
}

/// Runs `<grok_bin> usage <session_id>` synchronously with `timeout` (`GROK_USAGE_TIMEOUT` = 5 s).
/// Kills and reaps the subprocess if it does not finish within `timeout`.
///
/// Must only be called from a background worker thread (see [`TabUsageTracker::trigger_grok_usage`]).
pub fn run_grok_usage_with_timeout(grok_bin: &Path, session_id: &str, timeout: Duration) -> Option<UsageReading> {
    let clean_sid = session_id.trim();
    if clean_sid.is_empty() {
        return None;
    }
    let mut child = Command::new(grok_bin).arg("usage").arg(clean_sid).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().ok()?;

    let stdout_pipe = child.stdout.take()?;
    let reader_handle = std::thread::spawn(move || {
        let mut out = String::new();
        let mut limited = stdout_pipe.take(4 << 20);
        let _ = limited.read_to_string(&mut out);
        out
    });

    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                if !status.success() {
                    return None;
                }
                let stdout = reader_handle.join().ok()?;
                return parse_grok_usage_json(&stdout, now_unix_ms());
            }
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return None;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
}

/// Resolves the `grok` binary path from `PATH` or [`crate::terminal::extra_cli_path_dirs`].
pub fn resolve_grok_binary(extra_dirs: &[PathBuf]) -> PathBuf {
    let candidate_name = if cfg!(windows) { "grok.exe" } else { "grok" };
    for dir in extra_dirs {
        let p = dir.join(candidate_name);
        if p.is_file() {
            return p;
        }
    }
    if let Some(path_os) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path_os) {
            let p = dir.join(candidate_name);
            if p.is_file() {
                return p;
            }
        }
    }
    for dir in crate::terminal::extra_cli_path_dirs() {
        let p = dir.join(candidate_name);
        if p.is_file() {
            return p;
        }
    }
    PathBuf::from("grok")
}

#[derive(Default, Debug, Serialize, Deserialize)]
struct UsageCacheFile {
    #[serde(default)]
    entries: BTreeMap<String, UsageReading>,
}

/// Saves `reading` under `key` inside `<session_dir>/usage.json` (`#678` §4).
pub fn save_cached_usage(session_dir: &Path, key: &str, reading: &UsageReading) {
    static WRITE_LOCK: Mutex<u64> = Mutex::new(0);
    if key.trim().is_empty() || !reading.has_documented_source() {
        return;
    }
    let Ok(mut seq_guard) = WRITE_LOCK.lock() else {
        return;
    };
    *seq_guard = seq_guard.wrapping_add(1);
    let seq = *seq_guard;
    let path = session_dir.join(USAGE_CACHE_FILENAME);
    let mut cache = std::fs::read_to_string(&path).ok().and_then(|s| serde_json::from_str::<UsageCacheFile>(&s).ok()).unwrap_or_default();
    cache.entries.insert(key.trim().to_string(), reading.clone());
    if let Ok(bytes) = serde_json::to_vec_pretty(&cache) {
        let _ = std::fs::create_dir_all(session_dir);
        let tmp = path.with_extension(format!("tmp.{}.{seq}", std::process::id()));
        if std::fs::write(&tmp, &bytes).is_ok() {
            let _ = std::fs::rename(&tmp, &path);
        }
    }
}

/// Loads a previously cached [`UsageReading`] for `key` from `<session_dir>/usage.json`.
pub fn load_cached_usage(session_dir: &Path, key: &str) -> Option<UsageReading> {
    let clean = key.trim();
    if clean.is_empty() {
        return None;
    }
    let path = session_dir.join(USAGE_CACHE_FILENAME);
    let raw = std::fs::read_to_string(&path).ok()?;
    let cache: UsageCacheFile = serde_json::from_str(&raw).ok()?;
    cache.entries.get(clean).cloned().filter(UsageReading::has_documented_source)
}

#[derive(Debug, Default)]
struct AsyncSlotState {
    reading: Option<UsageReading>,
    in_flight: bool,
    last_poll_at: Option<Instant>,
    last_failed: bool,
}

/// Per-tab asynchronous usage tracker owned by [`crate::terminal::TerminalTab`].
///
/// All filesystem tailing and subprocess invocations run on short-lived background threads so the UI
/// thread never blocks (`#678` §3.4, §3.5, §10, §11).
#[derive(Clone, Debug)]
pub struct TabUsageTracker {
    state: Arc<Mutex<AsyncSlotState>>,
    claude_reader: Arc<Mutex<ClaudeTranscriptReader>>,
}

impl Default for TabUsageTracker {
    fn default() -> Self {
        Self::new(None)
    }
}

impl TabUsageTracker {
    pub fn new(initial: Option<UsageReading>) -> Self {
        let valid_initial = initial.filter(UsageReading::has_documented_source);
        Self {
            state: Arc::new(Mutex::new(AsyncSlotState { reading: valid_initial, in_flight: false, last_poll_at: None, last_failed: false })),
            claude_reader: Arc::new(Mutex::new(ClaudeTranscriptReader::new(PathBuf::new()))),
        }
    }

    /// Returns the most recent validated [`UsageReading`] for this tab without blocking the UI thread.
    pub fn reading(&self) -> Option<UsageReading> {
        self.state.lock().ok().and_then(|g| g.reading.clone()).filter(UsageReading::has_documented_source)
    }

    /// Sets the current reading directly (used in tests and when restoring from `usage.json`).
    pub fn set_reading(&self, reading: Option<UsageReading>) {
        if let Ok(mut g) = self.state.lock() {
            g.reading = reading.filter(UsageReading::has_documented_source);
        }
    }

    /// True while a background transcript tail or `grok usage` subprocess is running.
    pub fn is_in_flight(&self) -> bool {
        self.state.lock().map(|g| g.in_flight).unwrap_or(false)
    }

    /// True if the most recent background probe failed or timed out.
    pub fn last_failed(&self) -> bool {
        self.state.lock().map(|g| g.last_failed).unwrap_or(false)
    }

    /// Elapsed time since the last poll finished or started.
    pub fn elapsed_since_last_poll(&self) -> Option<Duration> {
        self.state.lock().ok().and_then(|g| g.last_poll_at.map(|t| t.elapsed()))
    }

    /// Triggers an incremental Claude transcript tail off the UI thread, reading
    /// `<session_dir>/claude-transcript.json` and updating the tab's [`UsageReading`].
    pub fn trigger_claude_transcript_poll(&self, session_dir: PathBuf, cache_key: Option<String>) {
        let Ok(mut st) = self.state.lock() else { return };
        if st.in_flight {
            return;
        }
        st.in_flight = true;
        st.last_poll_at = Some(Instant::now());
        drop(st);

        let state_bg = Arc::clone(&self.state);
        let reader_bg = Arc::clone(&self.claude_reader);
        let _ = std::thread::Builder::new().name("designos-claude-usage".into()).spawn(move || {
            let capture_path = session_dir.join(CLAUDE_TRANSCRIPT_META_FILENAME);
            let mut source_confirmed_gone = false;
            let mut target_rotated = false;
            let next_reading = if let Some(meta) = read_claude_transcript_meta_for(&capture_path, cache_key.as_deref()) {
                if !meta.transcript_path.exists() {
                    source_confirmed_gone = true;
                    None
                } else if let Ok(mut rdr) = reader_bg.lock() {
                    target_rotated = rdr.set_target(Some(meta.session_id.clone()), meta.transcript_path);
                    rdr.poll()
                } else {
                    None
                }
            } else {
                None
            };

            if let Some(ref r) = next_reading {
                let key = cache_key.as_deref().unwrap_or("claude");
                save_cached_usage(&session_dir, key, r);
            }
            if let Ok(mut st) = state_bg.lock() {
                st.last_failed = next_reading.is_none();
                if let Some(r) = next_reading {
                    st.reading = Some(r);
                } else if source_confirmed_gone || target_rotated {
                    st.reading = None;
                }
                st.in_flight = false;
            }
        });
    }

    /// Triggers a `grok usage <session_id>` subprocess off the UI thread with `timeout` (`#678` §3.5, §10).
    pub fn trigger_grok_usage(&self, grok_bin: PathBuf, session_id: String, timeout: Duration, session_dir: Option<PathBuf>) {
        let clean_sid = session_id.trim().to_string();
        if clean_sid.is_empty() {
            return;
        }
        let Ok(mut st) = self.state.lock() else { return };
        if st.in_flight {
            return;
        }
        st.in_flight = true;
        st.last_poll_at = Some(Instant::now());
        drop(st);

        let state_bg = Arc::clone(&self.state);
        let _ = std::thread::Builder::new().name("designos-grok-usage".into()).spawn(move || {
            let result = run_grok_usage_with_timeout(&grok_bin, &clean_sid, timeout);
            if let (Some(dir), Some(r)) = (&session_dir, &result) {
                save_cached_usage(dir, &clean_sid, r);
            }
            if let Ok(mut st) = state_bg.lock() {
                st.last_failed = result.is_none();
                if let Some(r) = result {
                    st.reading = Some(r);
                }
                st.in_flight = false;
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn temp_test_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("dos-678-{tag}-{}-{}", std::process::id(), now_unix_ms()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp test dir");
        dir
    }

    #[test]
    fn claude_transcript_tokens_are_deduplicated_by_message_id() {
        let dir = temp_test_dir("dedupe");
        let transcript = dir.join("session.jsonl");
        let lines = [
            r#"{"type":"assistant","message":{"id":"msg_01","usage":{"input_tokens":100,"cache_creation_input_tokens":50,"cache_read_input_tokens":200,"output_tokens":10}}}"#,
            r#"{"type":"assistant","message":{"id":"msg_01","usage":{"input_tokens":100,"cache_creation_input_tokens":50,"cache_read_input_tokens":200,"output_tokens":45}}}"#,
            r#"{"type":"assistant","message":{"id":"msg_01","usage":{"input_tokens":100,"cache_creation_input_tokens":50,"cache_read_input_tokens":200,"output_tokens":80}}}"#,
            r#"{"type":"assistant","message":{"id":"msg_02","usage":{"input_tokens":300,"cache_creation_input_tokens":0,"cache_read_input_tokens":400,"output_tokens":120}}}"#,
        ];
        std::fs::write(&transcript, format!("{}\n", lines.join("\n"))).unwrap();

        let mut reader = ClaudeTranscriptReader::new(&transcript);
        let reading = reader.poll().expect("reading from synthetic transcript");
        // msg_01 last value: in = 100 + 50 + 200 = 350, out = 80
        // msg_02 value:      in = 300 + 0 + 400  = 700, out = 120
        // Total:             in = 1050, out = 200
        assert_eq!(reader.distinct_message_count(), 2);
        assert_eq!(reading.input, 1_050);
        assert_eq!(reading.output, 200);
        assert_eq!(reading.cost, None);
        assert_eq!(reading.source, UsageSource::ClaudeTranscript);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn claude_transcript_missing_hides_the_field() {
        let dir = temp_test_dir("missing");
        let missing = dir.join("does-not-exist.jsonl");
        let mut reader = ClaudeTranscriptReader::new(&missing);
        let reading = reader.poll();
        assert_eq!(reading, None);
        assert_eq!(header_usage_caption_for_tab(AgentCliPreset::Claude, reading.as_ref()), None);
        assert_eq!(tooltip_usage_text_for_tab(AgentCliPreset::Claude, reading.as_ref()).as_deref(), Some("Usage not available"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn grok_usage_json_parses_tokens_and_cost() {
        let sample = r#"{
            "sessionId": "01a12e4d-36ae-79c2-9fd5-44e9bbf457c4",
            "updatedAt": "2026-10-19T12:00:00Z",
            "session": {
                "inputTokens": 10000,
                "outputTokens": 2500,
                "cachedReadTokens": 2000,
                "cacheCreationTokens": 400,
                "reasoningTokens": 600,
                "totalTokens": 15500,
                "modelCalls": 4,
                "costUsdTicks": 4200000000,
                "turnCount": 2,
                "primaryModelId": "grok-4.7-build"
            },
            "turns": []
        }"#;
        let reading = parse_grok_usage_json(sample, 1_760_000_000_000).expect("parse grok usage");
        assert_eq!(reading.input, 12_400);
        assert_eq!(reading.output, 3_100);
        assert_eq!(reading.source, UsageSource::GrokUsage);
        assert_eq!(reading.cost, Some(CostLabel::Reference(0.42)));
        assert_eq!(reading.format_caption(), "in 12.4k · out 3.1k · $0.42 ref.");
        let tooltip = reading.format_tooltip();
        assert!(tooltip.contains("From grok usage"), "{tooltip}");
        assert!(tooltip.contains("list-price reference, not a bill"), "{tooltip}");
    }

    #[test]
    fn grok_without_a_cost_shows_tokens_only() {
        let sample = r#"{
            "sessionId": "01a12e4d-36ae-79c2-9fd5-44e9bbf457c4",
            "updatedAt": "2026-10-19T12:00:00Z",
            "session": {
                "inputTokens": 8000,
                "outputTokens": 1500,
                "cachedReadTokens": 0,
                "cacheCreationTokens": 0,
                "reasoningTokens": 0,
                "totalTokens": 9500,
                "modelCalls": 2,
                "turnCount": 1,
                "primaryModelId": "grok-unpriced"
            },
            "turns": []
        }"#;
        let reading = parse_grok_usage_json(sample, 1_760_000_000_000).expect("parse grok usage");
        assert_eq!(reading.input, 8_000);
        assert_eq!(reading.output, 1_500);
        assert_eq!(reading.cost, None);
        assert_eq!(reading.format_caption(), "in 8k · out 1.5k");
        assert!(!reading.format_caption().contains('$'));
    }

    #[test]
    fn codex_and_agy_show_nothing() {
        let fake_reading = UsageReading { input: 5_000, output: 1_000, cost: None, source: UsageSource::ClaudeTranscript, as_of_ms: 1_760_000_000_000 };
        for preset in [AgentCliPreset::Codex, AgentCliPreset::Antigravity, AgentCliPreset::Shell] {
            assert!(!cli_has_usable_usage_source(preset), "{preset:?}");
            assert_eq!(header_usage_caption_for_tab(preset, None), None, "{preset:?}");
            assert_eq!(header_usage_caption_for_tab(preset, Some(&fake_reading)), None, "{preset:?} must never draw a usage caption");
            assert_eq!(tooltip_usage_text_for_tab(preset, None), None, "{preset:?}");
        }
    }

    #[test]
    fn hook_capture_writes_session_and_transcript_path_once() {
        let dir = temp_test_dir("hook-capture");
        let capture_path = dir.join(CLAUDE_TRANSCRIPT_META_FILENAME);
        let cmd = claude_transcript_capture_command(&capture_path);
        let stdin_json = r#"{"session_id":"sess-abc-123","transcript_path":"/tmp/claude/sess-abc-123.jsonl","hook_event_name":"UserPromptSubmit"}"#;

        let run_with_stdin = |payload: &str| {
            let mut child =
                Command::new("sh").arg("-c").arg(&cmd).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().expect("spawn sh");
            child.stdin.as_mut().unwrap().write_all(payload.as_bytes()).expect("write stdin");
            let out = child.wait_with_output().expect("wait hook");
            assert!(out.status.success(), "hook failed: {:?}", out);
        };

        run_with_stdin(stdin_json);
        let meta = read_claude_transcript_meta(&capture_path).expect("meta written");
        assert_eq!(meta.session_id, "sess-abc-123");
        assert_eq!(meta.transcript_path, PathBuf::from("/tmp/claude/sess-abc-123.jsonl"));

        // Make the file read-only (0444): a second hook invocation in the same session must NOT
        // attempt to rewrite it.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&capture_path).unwrap().permissions();
            perms.set_mode(0o444);
            std::fs::set_permissions(&capture_path, perms).unwrap();
        }
        run_with_stdin(stdin_json);
        let meta_again = read_claude_transcript_meta(&capture_path).expect("meta still readable");
        assert_eq!(meta_again, meta);

        // Restore write permissions and verify `/clear` (a new session_id) updates the capture file.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&capture_path).unwrap().permissions();
            perms.set_mode(0o644);
            std::fs::set_permissions(&capture_path, perms).unwrap();
        }
        let next_stdin = r#"{"session_id":"sess-def-456","transcript_path":"/tmp/claude/sess-def-456.jsonl","hook_event_name":"UserPromptSubmit"}"#;
        run_with_stdin(next_stdin);
        let meta_next = read_claude_transcript_meta(&capture_path).expect("meta updated on /clear");
        assert_eq!(meta_next.session_id, "sess-def-456");
        assert_eq!(meta_next.transcript_path, PathBuf::from("/tmp/claude/sess-def-456.jsonl"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn reader_tails_incrementally() {
        let dir = temp_test_dir("incremental");
        let transcript = dir.join("session.jsonl");
        let line1 = "{\"type\":\"assistant\",\"message\":{\"id\":\"m1\",\"usage\":{\"input_tokens\":400,\"output_tokens\":100}}}\n";
        std::fs::write(&transcript, line1).unwrap();

        let mut reader = ClaudeTranscriptReader::new(&transcript);
        let r1 = reader.poll().expect("first poll");
        assert_eq!(r1.input, 400);
        assert_eq!(r1.output, 100);
        assert_eq!(reader.bytes_read_last_poll(), line1.len() as u64);
        assert_eq!(reader.offset(), line1.len() as u64);

        // Poll with no new bytes: reads 0 bytes.
        let r_idle = reader.poll().expect("idle poll");
        assert_eq!(reader.bytes_read_last_poll(), 0);
        assert_eq!(r_idle.input, 400);

        // Append a second line: only the new line's bytes are read.
        let line2 = "{\"type\":\"assistant\",\"message\":{\"id\":\"m2\",\"usage\":{\"input_tokens\":600,\"output_tokens\":250}}}\n";
        let mut f = std::fs::OpenOptions::new().append(true).open(&transcript).unwrap();
        f.write_all(line2.as_bytes()).unwrap();
        f.flush().unwrap();

        let r2 = reader.poll().expect("second poll");
        assert_eq!(reader.bytes_read_last_poll(), line2.len() as u64, "only newly appended bytes must be read");
        assert_eq!(reader.offset(), (line1.len() + line2.len()) as u64);
        assert_eq!(r2.input, 1_000);
        assert_eq!(r2.output, 350);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn grok_call_runs_off_the_ui_thread_with_a_timeout() {
        let dir = temp_test_dir("grok-timeout");
        let stub = dir.join("grok");
        // Stub sleeps 7 seconds (past the 5 s GROK_USAGE_TIMEOUT) before printing.
        std::fs::write(&stub, "#!/bin/sh\nsleep 7\necho '{\"session\":{\"inputTokens\":999,\"outputTokens\":111,\"totalTokens\":1110}}'\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&stub).unwrap().permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&stub, perms).unwrap();
        }

        let tracker = TabUsageTracker::default();
        let t0 = Instant::now();
        tracker.trigger_grok_usage(stub.clone(), "01a12e4d-36ae-79c2-9fd5-44e9bbf457c4".into(), GROK_USAGE_TIMEOUT, None);
        let first_frame_reading = tracker.reading();
        let ui_elapsed = t0.elapsed();
        assert!(ui_elapsed < Duration::from_millis(200), "trigger_grok_usage + reading() must not block the first UI frame (took {ui_elapsed:?})");
        assert_eq!(first_frame_reading, None);
        assert!(tracker.is_in_flight());

        // Wait for the 5 s timeout to fire and terminate the sleeping stub.
        let wait_deadline = Instant::now() + Duration::from_secs(6);
        while tracker.is_in_flight() && Instant::now() < wait_deadline {
            std::thread::sleep(Duration::from_millis(25));
        }
        assert!(!tracker.is_in_flight(), "background grok usage call must terminate at the 5 s timeout");
        assert_eq!(tracker.reading(), None, "timed-out grok call must hide the field");
        assert!(tracker.last_failed());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn no_estimates() {
        // Every reading constructible from our parsers must carry one of the three documented sources,
        // and non-source CLIs (Codex, Antigravity, Shell) must never produce or display a reading.
        let allowed = UsageSource::ALL;
        assert_eq!(allowed.len(), 3);

        let dir = temp_test_dir("no-estimates");
        let transcript = dir.join("t.jsonl");
        std::fs::write(&transcript, "{\"message\":{\"id\":\"m1\",\"usage\":{\"input_tokens\":10,\"output_tokens\":5}}}\n").unwrap();
        let mut reader = ClaudeTranscriptReader::new(&transcript);
        let r_claude = reader.poll().expect("claude transcript reading");
        assert!(allowed.contains(&r_claude.source));
        assert!(r_claude.has_documented_source());

        let r_status =
            parse_claude_status_line_json(r#"{"context_window":{"total_input_tokens":100,"total_output_tokens":20},"cost":{"total_cost_usd":0.05}}"#, 1_000)
                .expect("statusline reading");
        assert!(allowed.contains(&r_status.source));
        assert!(r_status.has_documented_source());

        let r_grok = parse_grok_usage_json(r#"{"session":{"inputTokens":50,"outputTokens":10,"totalTokens":60}}"#, 1_000).expect("grok reading");
        assert!(allowed.contains(&r_grok.source));
        assert!(r_grok.has_documented_source());

        for preset in [AgentCliPreset::Codex, AgentCliPreset::Antigravity, AgentCliPreset::Shell, AgentCliPreset::DesignOsHelp] {
            assert!(!cli_has_usable_usage_source(preset));
            assert_eq!(header_usage_caption_for_tab(preset, None), None);
        }

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn sources_document_matches_code() {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let doc_path = manifest_dir.parent().and_then(|p| p.parent()).expect("repo root").join("docs").join("agent-cli-usage-sources.md");
        let doc = std::fs::read_to_string(&doc_path).unwrap_or_else(|e| panic!("failed to read {}: {e}", doc_path.display()));

        for source in UsageSource::ALL {
            let name = source.variant_name();
            assert!(doc.contains(name), "UsageSource::{name} must have a row in {}", doc_path.display());
        }
        for required_term in ["Claude Code", "Grok", "Codex", "Antigravity", "no source", "2026-10-19"] {
            assert!(doc.contains(required_term), "missing '{required_term}' in {}", doc_path.display());
        }
    }

    #[test]
    fn two_claude_sessions_in_one_session_dir_keep_distinct_transcript_pointers_and_readings() {
        let dir = temp_test_dir("two-claude-tabs");
        let capture_path = dir.join(CLAUDE_TRANSCRIPT_META_FILENAME);
        let cmd = claude_transcript_capture_command(&capture_path);

        let transcript_a = dir.join("tab-a.jsonl");
        let transcript_b = dir.join("tab-b.jsonl");
        std::fs::write(&transcript_a, "{\"message\":{\"id\":\"ma\",\"usage\":{\"input_tokens\":1000,\"output_tokens\":200}}}\n").unwrap();
        std::fs::write(&transcript_b, "{\"message\":{\"id\":\"mb\",\"usage\":{\"input_tokens\":9000,\"output_tokens\":800}}}\n").unwrap();

        let run_hook = |sid: &str, tp: &Path| {
            let payload = format!(r#"{{"session_id":"{sid}","transcript_path":"{}","hook_event_name":"UserPromptSubmit"}}"#, tp.display());
            let mut child =
                Command::new("sh").arg("-c").arg(&cmd).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().expect("spawn sh");
            child.stdin.as_mut().unwrap().write_all(payload.as_bytes()).expect("write stdin");
            let out = child.wait_with_output().expect("wait hook");
            assert!(out.status.success(), "hook failed: {out:?}");
        };

        // Tab A hook runs first, then Tab B hook runs in the same session_dir.
        run_hook("uuid-tab-a", &transcript_a);
        run_hook("uuid-tab-b", &transcript_b);

        let tracker_a = TabUsageTracker::default();
        let tracker_b = TabUsageTracker::default();
        tracker_a.trigger_claude_transcript_poll(dir.clone(), Some("uuid-tab-a".to_string()));
        tracker_b.trigger_claude_transcript_poll(dir.clone(), Some("uuid-tab-b".to_string()));

        let deadline = Instant::now() + Duration::from_secs(2);
        while (tracker_a.is_in_flight() || tracker_b.is_in_flight()) && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }

        let reading_a = tracker_a.reading().expect("tab A reading");
        let reading_b = tracker_b.reading().expect("tab B reading");
        assert_eq!((reading_a.input, reading_a.output), (1_000, 200), "tab A must read its own transcript, not tab B's");
        assert_eq!((reading_b.input, reading_b.output), (9_000, 800), "tab B must read its own transcript");

        let cached_a = load_cached_usage(&dir, "uuid-tab-a").expect("cached A");
        let cached_b = load_cached_usage(&dir, "uuid-tab-b").expect("cached B");
        assert_eq!((cached_a.input, cached_a.output), (1_000, 200));
        assert_eq!((cached_b.input, cached_b.output), (9_000, 800));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn claude_clear_rotates_session_id_and_restarts_tab_usage_reading() {
        let dir = temp_test_dir("claude-clear-rotate");
        let capture_path = dir.join(CLAUDE_TRANSCRIPT_META_FILENAME);
        let cmd = claude_transcript_capture_command(&capture_path);

        let transcript_before = dir.join("sess-before-clear.jsonl");
        let transcript_after = dir.join("sess-after-clear.jsonl");
        let transcript_other = dir.join("tab-b.jsonl");
        std::fs::write(&transcript_before, "{\"message\":{\"id\":\"m_old\",\"usage\":{\"input_tokens\":12400,\"output_tokens\":3100}}}\n").unwrap();
        std::fs::write(&transcript_after, "{\"message\":{\"id\":\"m_new\",\"usage\":{\"input_tokens\":500,\"output_tokens\":120}}}\n").unwrap();
        std::fs::write(&transcript_other, "{\"message\":{\"id\":\"m_b\",\"usage\":{\"input_tokens\":8000,\"output_tokens\":900}}}\n").unwrap();

        let run_hook_with_tab_env = |tab_uuid: &str, sid: &str, tp: &Path| {
            let payload = format!(r#"{{"session_id":"{sid}","transcript_path":"{}","hook_event_name":"UserPromptSubmit"}}"#, tp.display());
            let mut child = Command::new("sh")
                .arg("-c")
                .arg(&cmd)
                .env("DESIGNOS_TAB_SESSION_UUID", tab_uuid)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .expect("spawn sh");
            child.stdin.as_mut().unwrap().write_all(payload.as_bytes()).expect("write stdin");
            let out = child.wait_with_output().expect("wait hook");
            assert!(out.status.success(), "hook failed: {out:?}");
        };

        // 1. Tab A spawns with DESIGNOS_TAB_SESSION_UUID="tab-uuid-a" and initial session_id="tab-uuid-a".
        //    Tab B also runs in the same session_dir.
        run_hook_with_tab_env("tab-uuid-a", "tab-uuid-a", &transcript_before);
        run_hook_with_tab_env("tab-uuid-b", "tab-uuid-b", &transcript_other);

        let tracker_a = TabUsageTracker::default();
        tracker_a.trigger_claude_transcript_poll(dir.clone(), Some("tab-uuid-a".to_string()));
        let deadline = Instant::now() + Duration::from_secs(2);
        while tracker_a.is_in_flight() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        let r_before = tracker_a.reading().expect("reading before /clear");
        assert_eq!((r_before.input, r_before.output), (12_400, 3_100));

        // 2. User runs `/clear` in Tab A: Claude rotates to a new session_id ("sess-rotated-2") and new transcript file,
        //    while the PTY environment still carries DESIGNOS_TAB_SESSION_UUID="tab-uuid-a".
        run_hook_with_tab_env("tab-uuid-a", "sess-rotated-2", &transcript_after);

        let meta_a = read_claude_transcript_meta_for(&capture_path, Some("tab-uuid-a")).expect("resolved meta for tab-uuid-a");
        assert_eq!(meta_a.transcript_path, transcript_after, "tab-uuid-a must re-point to the post-/clear transcript");
        assert_eq!(meta_a.session_id, "sess-rotated-2");

        let meta_b = read_claude_transcript_meta_for(&capture_path, Some("tab-uuid-b")).expect("resolved meta for tab-uuid-b");
        assert_eq!(meta_b.transcript_path, transcript_other, "tab-uuid-b entry must remain intact when tab-uuid-a updates");

        tracker_a.trigger_claude_transcript_poll(dir.clone(), Some("tab-uuid-a".to_string()));
        let deadline2 = Instant::now() + Duration::from_secs(2);
        while tracker_a.is_in_flight() && Instant::now() < deadline2 {
            std::thread::sleep(Duration::from_millis(10));
        }
        let r_after = tracker_a.reading().expect("reading after /clear");
        assert_eq!((r_after.input, r_after.output), (500, 120), "readout must restart from the new transcript after /clear");

        let cached_a = load_cached_usage(&dir, "tab-uuid-a").expect("cached A updated");
        assert_eq!((cached_a.input, cached_a.output), (500, 120));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn failed_usage_probe_preserves_previous_reading_until_source_is_gone() {
        let dir = temp_test_dir("preserve-reading");
        let seed = UsageReading { input: 4_200, output: 600, cost: None, source: UsageSource::ClaudeTranscript, as_of_ms: 1_760_000_000_000 };
        let tracker = TabUsageTracker::new(Some(seed.clone()));

        // 1. Poll before claude-transcript.json exists (e.g. right on resume): must keep the restored reading.
        tracker.trigger_claude_transcript_poll(dir.clone(), Some("uuid-resumed".to_string()));
        let deadline = Instant::now() + Duration::from_secs(2);
        while tracker.is_in_flight() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(tracker.reading(), Some(seed), "transient missing capture file must not erase restored reading");

        // 2. Grok poll failure must also keep a restored reading.
        let grok_seed =
            UsageReading { input: 3_000, output: 500, cost: Some(CostLabel::Reference(0.12)), source: UsageSource::GrokUsage, as_of_ms: 1_760_000_000_000 };
        let grok_tracker = TabUsageTracker::new(Some(grok_seed.clone()));
        let failing_grok = dir.join("grok-fail");
        std::fs::write(&failing_grok, "#!/bin/sh\nexit 1\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&failing_grok).unwrap().permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&failing_grok, perms).unwrap();
        }
        grok_tracker.trigger_grok_usage(failing_grok, "uuid-grok".into(), Duration::from_millis(500), Some(dir.clone()));
        while grok_tracker.is_in_flight() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(grok_tracker.reading(), Some(grok_seed), "failed grok usage probe must not erase restored reading");

        // 3. When claude-transcript.json maps this tab's uuid to a transcript path that is confirmed gone, clear the reading.
        let capture_path = dir.join(CLAUDE_TRANSCRIPT_META_FILENAME);
        let gone_path = dir.join("confirmed-gone.jsonl");
        let meta_json = serde_json::json!({
            "session_id": "uuid-resumed",
            "transcript_path": gone_path,
            "sessions": {
                "uuid-resumed": gone_path
            }
        });
        std::fs::write(&capture_path, serde_json::to_vec(&meta_json).unwrap()).unwrap();
        tracker.trigger_claude_transcript_poll(dir.clone(), Some("uuid-resumed".to_string()));
        let deadline2 = Instant::now() + Duration::from_secs(2);
        while tracker.is_in_flight() && Instant::now() < deadline2 {
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(tracker.reading(), None, "reading must be cleared when this tab's own transcript file is confirmed gone");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
