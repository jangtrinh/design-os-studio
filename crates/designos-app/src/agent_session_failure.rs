//! Mid-session agent failure classification, status-page check, and banner UI (`#686`).
//!
//! Classifies mid-session agent failures (`RateLimited`, `LoginExpired`, `BillingOrQuota`,
//! `ProviderServerError`, `ContextFull`, `NetworkOffline`, `Unknown`) from hook error payloads and
//! per-CLI output patterns in `agent_failure_patterns.toml`, and renders a top-of-viewport banner
//! with one plain sentence, a "Copy details" button, and one recovery action.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

#[cfg(test)]
use egui::Id;
use egui::{Align2, Painter, Pos2, Rect, Stroke, StrokeKind, Ui, Vec2};

use crate::agent_failure_redact::format_copy_details;
use crate::agent_start_failure::{self, FixAction};
use crate::terminal::{strip_ansi_codes, AgentCliPreset};
use crate::{theme, widgets};

/// Maximum number of trailing output lines scanned when a failure trigger fires (`#686` §11).
pub const CLASSIFY_TAIL_LINES: usize = 200;

const PATTERNS_TOML: &str = include_str!("agent_failure_patterns.toml");

/// Mid-session failure classes (`#686` §3.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SessionFailureClass {
    RateLimited,
    LoginExpired,
    BillingOrQuota,
    ProviderServerError,
    ContextFull,
    NetworkOffline,
    Unknown,
}

impl SessionFailureClass {
    pub fn slug(self) -> &'static str {
        match self {
            Self::RateLimited => "rate_limited",
            Self::LoginExpired => "login_expired",
            Self::BillingOrQuota => "billing_or_quota",
            Self::ProviderServerError => "provider_server_error",
            Self::ContextFull => "context_full",
            Self::NetworkOffline => "network_offline",
            Self::Unknown => "unknown",
        }
    }

    pub fn from_slug(slug: &str) -> Option<Self> {
        match slug.trim() {
            "rate_limited" => Some(Self::RateLimited),
            "login_expired" => Some(Self::LoginExpired),
            "billing_or_quota" => Some(Self::BillingOrQuota),
            "provider_server_error" => Some(Self::ProviderServerError),
            "context_full" => Some(Self::ContextFull),
            "network_offline" => Some(Self::NetworkOffline),
            "unknown" => Some(Self::Unknown),
            _ => None,
        }
    }

    /// Maps a hook `StopFailure` error string (`rate_limit`, `authentication_failed`, etc.) to a
    /// known class. Returns `None` for `"unknown"` or unrecognized strings so classification falls
    /// through to the CLI's output patterns (`#686` §3.3).
    pub fn from_hook_error(err: &str) -> Option<Self> {
        match err.trim().to_ascii_lowercase().as_str() {
            "rate_limit" | "rate_limited" | "capacity" => Some(Self::RateLimited),
            "authentication_failed" | "login_expired" | "auth_expired" | "unauthorized" => Some(Self::LoginExpired),
            "billing" | "billing_or_quota" | "insufficient_quota" => Some(Self::BillingOrQuota),
            "server_error" | "provider_server_error" | "overloaded" => Some(Self::ProviderServerError),
            "max_output_tokens" | "context_full" | "context_length_exceeded" => Some(Self::ContextFull),
            "network_offline" | "connection_error" => Some(Self::NetworkOffline),
            _ => None,
        }
    }

    pub fn fix_for_preset(self, preset: AgentCliPreset) -> Option<FixAction> {
        match self {
            Self::RateLimited => Some(FixAction::RetryText),
            Self::LoginExpired => Some(FixAction::RunLogin(agent_start_failure::login_command(preset))),
            Self::BillingOrQuota => Some(FixAction::OpenBilling(agent_start_failure::billing_url(preset))),
            Self::ProviderServerError => Some(FixAction::RetryText),
            Self::ContextFull => Some(FixAction::NewSession),
            Self::NetworkOffline => Some(FixAction::RetryWhenOnline),
            Self::Unknown => None,
        }
    }
}

/// One pattern entry parsed from `agent_failure_patterns.toml`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PatternEntry {
    pub cli: String,
    pub class: SessionFailureClass,
    pub regex: String,
    pub fixture: String,
}

/// Parsed pattern table from `agent_failure_patterns.toml`.
#[derive(Clone, Debug, Default)]
pub struct PatternCatalog {
    pub entries: Vec<PatternEntry>,
}

impl PatternCatalog {
    pub fn for_preset(&self, preset: AgentCliPreset) -> impl Iterator<Item = &PatternEntry> {
        let key = match preset {
            AgentCliPreset::Claude => "claude",
            AgentCliPreset::Codex => "codex",
            AgentCliPreset::Grok => "grok",
            AgentCliPreset::Antigravity => "antigravity",
            AgentCliPreset::Gemini => "gemini",
            AgentCliPreset::OpenCode => "opencode",
            AgentCliPreset::Shell | AgentCliPreset::DesignOsHelp => "",
        };
        self.entries.iter().filter(move |e| e.cli == key)
    }
}

fn resolve_fixture_path(rel: &str) -> PathBuf {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    if let Some(workspace_root) = manifest_dir.parent().and_then(|p| p.parent()) {
        let candidate = workspace_root.join(rel);
        if candidate.exists() {
            return candidate;
        }
    }
    let local = manifest_dir.join(rel);
    if local.exists() {
        return local;
    }
    PathBuf::from(rel)
}

/// Parses and validates `agent_failure_patterns.toml`. Rejects any pattern without a `fixture`
/// attribute or (when `check_fixtures_on_disk` is true) whose fixture file does not exist on disk.
pub fn parse_patterns_toml(src: &str, check_fixtures_on_disk: bool) -> Result<PatternCatalog, String> {
    let mut entries = Vec::new();
    let mut current_cli: Option<String> = None;
    let mut current_class: Option<String> = None;
    let mut current_regex: Option<String> = None;
    let mut current_fixture: Option<String> = None;

    let flush = |cli: &Option<String>,
                 class: &mut Option<String>,
                 regex: &mut Option<String>,
                 fixture: &mut Option<String>,
                 entries: &mut Vec<PatternEntry>|
     -> Result<(), String> {
        let Some(cli_name) = cli.as_ref() else { return Ok(()) };
        let cls_str = class.take().ok_or_else(|| format!("pattern in cli.{cli_name} missing `class`"))?;
        let cls = SessionFailureClass::from_slug(&cls_str).ok_or_else(|| format!("pattern in cli.{cli_name} has unknown class {cls_str:?}"))?;
        let rgx = regex.take().ok_or_else(|| format!("pattern in cli.{cli_name} ({cls_str}) missing `regex`"))?;
        if rgx.trim().is_empty() {
            return Err(format!("pattern in cli.{cli_name} ({cls_str}) has empty `regex`"));
        }
        let fix = fixture.take().ok_or_else(|| format!("pattern in cli.{cli_name} ({cls_str}) missing `fixture`"))?;
        if fix.trim().is_empty() {
            return Err(format!("pattern in cli.{cli_name} ({cls_str}) has empty `fixture`"));
        }
        if check_fixtures_on_disk {
            let path = resolve_fixture_path(fix.trim());
            let content = std::fs::read_to_string(&path).map_err(|e| format!("fixture {:?} for cli.{cli_name} ({cls_str}) unreadable: {e}", fix.trim()))?;
            if content.trim().is_empty() {
                return Err(format!("fixture {:?} for cli.{cli_name} ({cls_str}) is empty", fix.trim()));
            }
        }
        entries.push(PatternEntry { cli: cli_name.clone(), class: cls, regex: rgx, fixture: fix });
        Ok(())
    };

    for (line_no, raw_line) in src.lines().enumerate() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with("[[") && line.ends_with("]]") {
            flush(&current_cli, &mut current_class, &mut current_regex, &mut current_fixture, &mut entries)?;
            let header = &line[2..line.len() - 2].trim();
            let parts: Vec<&str> = header.split('.').collect();
            if parts.len() != 3 || parts[0] != "cli" || parts[2] != "pattern" {
                return Err(format!("line {}: invalid table header {line:?}", line_no + 1));
            }
            current_cli = Some(parts[1].to_string());
            continue;
        }
        let Some((k, v)) = line.split_once('=') else {
            return Err(format!("line {}: expected key = value, got {line:?}", line_no + 1));
        };
        let key = k.trim();
        let val = parse_toml_string_literal(v.trim()).map_err(|e| format!("line {}: {e}", line_no + 1))?;
        match key {
            "class" => current_class = Some(val),
            "regex" => current_regex = Some(val),
            "fixture" => current_fixture = Some(val),
            other => return Err(format!("line {}: unknown key {other:?}", line_no + 1)),
        }
    }
    flush(&current_cli, &mut current_class, &mut current_regex, &mut current_fixture, &mut entries)?;
    Ok(PatternCatalog { entries })
}

fn parse_toml_string_literal(s: &str) -> Result<String, String> {
    if !s.starts_with('"') || !s.ends_with('"') || s.len() < 2 {
        return Err(format!("expected quoted string, got {s:?}"));
    }
    serde_json::from_str::<String>(s).map_err(|e| format!("invalid string literal {s:?}: {e}"))
}

pub fn builtin_catalog() -> &'static PatternCatalog {
    static CATALOG: OnceLock<PatternCatalog> = OnceLock::new();
    CATALOG.get_or_init(|| parse_patterns_toml(PATTERNS_TOML, false).expect("valid embedded agent_failure_patterns.toml"))
}

const RESET_CAPTURE_TOKEN: &str = "(?p<reset>\\d{1,2}:\\d{2})";

/// Matches one `PatternEntry`'s `regex` (case-insensitive alternation of branches supporting
/// `.*`, escaped punctuation `\.` `\(` `\)`, and `(?P<reset>\d{1,2}:\d{2})`) against `text_lower`.
/// Returns `(matched, optional_reset_hhmm)`.
fn match_pattern_regex(regex: &str, text_lower: &str) -> (bool, Option<String>) {
    let mut matched = false;
    let mut reset_time = None;
    for branch in regex.split('|') {
        let b_lower = branch.trim().to_ascii_lowercase();
        if b_lower.is_empty() {
            continue;
        }
        if let Some(prefix) = b_lower.strip_suffix(RESET_CAPTURE_TOKEN) {
            let clean_prefix = unescape_regex_literals(prefix);
            if let Some(pos) = text_lower.find(&clean_prefix) {
                let after = &text_lower[pos + clean_prefix.len()..];
                if let Some(hhmm) = extract_hhmm_prefix(after) {
                    matched = true;
                    reset_time = Some(hhmm);
                }
            }
        } else if branch_matches(text_lower, &b_lower) {
            matched = true;
        }
    }
    (matched, reset_time)
}

fn unescape_regex_literals(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            if let Some(next) = chars.next() {
                out.push(next);
            }
        } else {
            out.push(c);
        }
    }
    out
}

fn branch_matches(text_lower: &str, branch_lower: &str) -> bool {
    let mut cursor = 0usize;
    for seg in branch_lower.split(".*") {
        let literal = unescape_regex_literals(seg);
        if literal.is_empty() {
            continue;
        }
        let Some(rel) = text_lower[cursor..].find(&literal) else {
            return false;
        };
        cursor += rel + literal.len();
    }
    true
}

fn extract_hhmm_prefix(s: &str) -> Option<String> {
    let trimmed = s.trim_start();
    let colon = trimmed.find(':')?;
    if !(1..=2).contains(&colon) || trimmed.len() < colon + 3 {
        return None;
    }
    let hh = &trimmed[..colon];
    let mm = &trimmed[colon + 1..colon + 3];
    if hh.chars().all(|c| c.is_ascii_digit()) && mm.chars().all(|c| c.is_ascii_digit()) {
        let h: u32 = hh.parse().ok()?;
        let m: u32 = mm.parse().ok()?;
        if h < 24 && m < 60 {
            return Some(format!("{h:02}:{m:02}"));
        }
    }
    None
}

fn extract_any_reset_time(preset: AgentCliPreset, catalog: &PatternCatalog, text_lower: &str) -> Option<String> {
    for entry in catalog.for_preset(preset) {
        if entry.class == SessionFailureClass::RateLimited {
            let (_, reset) = match_pattern_regex(&entry.regex, text_lower);
            if reset.is_some() {
                return reset;
            }
        }
    }
    None
}

/// Returns a borrowed slice containing at most the last `max_lines` lines of `s` without allocating.
pub fn tail_slice_last_n_lines(s: &str, max_lines: usize) -> &str {
    if max_lines == 0 || s.is_empty() {
        return "";
    }
    let trimmed = s.strip_suffix('\n').unwrap_or(s);
    let mut newlines_found = 0usize;
    for (idx, b) in trimmed.bytes().enumerate().rev() {
        if b == b'\n' {
            newlines_found += 1;
            if newlines_found == max_lines {
                return &s[idx + 1..];
            }
        }
    }
    s
}

/// Returns the last `max_lines` non-empty lines of `raw_output` (ANSI escapes stripped).
pub fn tail_lines_for_classification(raw_output: &str, max_lines: usize) -> String {
    let tail = tail_slice_last_n_lines(raw_output, max_lines);
    strip_ansi_codes(tail)
}

/// Per-tab mid-session failure state (`#686` §4).
#[derive(Clone, Debug)]
pub struct SessionFailure {
    pub preset: AgentCliPreset,
    pub class: SessionFailureClass,
    pub detail_line: String,
    pub reset_at: Option<String>,
    pub fix: Option<FixAction>,
    pub output_tail: Vec<String>,
    pub raw_tail: String,
    pub cli_version: String,
    pub probe_url_override: Option<String>,
    pub incident_title: Arc<Mutex<Option<String>>>,
    pub status_fetch_started: Arc<AtomicBool>,
    pub status_fetch_count: Arc<AtomicUsize>,
    pub online_attempts: Arc<AtomicUsize>,
    /// `0` = idle, `1` = probing on background worker, `2` = probe succeeded, `3` = probe failed.
    pub online_probe_state: Arc<AtomicU8>,
}

impl PartialEq for SessionFailure {
    fn eq(&self, other: &Self) -> bool {
        self.preset == other.preset
            && self.class == other.class
            && self.detail_line == other.detail_line
            && self.reset_at == other.reset_at
            && self.fix == other.fix
            && self.output_tail == other.output_tail
    }
}

impl Eq for SessionFailure {}

/// Classifies a mid-session failure when a trigger (`StopFailure` hook or non-zero exit after the
/// start window) fires (`#686` §3.1–3.5). Shell tabs never return a failure.
pub fn classify_session_failure(preset: AgentCliPreset, hook_error: Option<&str>, raw_output: &str) -> Option<SessionFailure> {
    classify_with_catalog(preset, hook_error, raw_output, builtin_catalog())
}

pub fn classify_with_catalog(preset: AgentCliPreset, hook_error: Option<&str>, raw_output: &str, catalog: &PatternCatalog) -> Option<SessionFailure> {
    if matches!(preset, AgentCliPreset::Shell | AgentCliPreset::DesignOsHelp) {
        return None;
    }
    let tail = tail_lines_for_classification(raw_output, CLASSIFY_TAIL_LINES);
    let lower = tail.to_ascii_lowercase();

    let mut matched_class = hook_error.and_then(SessionFailureClass::from_hook_error);
    let mut reset_at = None;

    if matched_class == Some(SessionFailureClass::RateLimited) {
        reset_at = extract_any_reset_time(preset, catalog, &lower);
    }

    if matched_class.is_none() {
        for entry in catalog.for_preset(preset) {
            let (m, reset) = match_pattern_regex(&entry.regex, &lower);
            if m {
                matched_class = Some(entry.class);
                reset_at =
                    reset.or_else(|| if entry.class == SessionFailureClass::RateLimited { extract_any_reset_time(preset, catalog, &lower) } else { None });
                break;
            }
        }
    }

    let class = matched_class.unwrap_or(SessionFailureClass::Unknown);
    Some(SessionFailure::new(preset, class, reset_at, raw_output))
}

/// Trigger (c) for mid-turn pattern checks: matches only known failure patterns in output
/// that arrived after the last turn start. Normal output (including the word `"login"` without a
/// failure context phrase) returns `None` (`#686` §3.1, §8.6).
pub fn classify_turn_output_if_matched(preset: AgentCliPreset, turn_output: &str) -> Option<SessionFailure> {
    if matches!(preset, AgentCliPreset::Shell | AgentCliPreset::DesignOsHelp) {
        return None;
    }
    let tail = tail_lines_for_classification(turn_output, CLASSIFY_TAIL_LINES);
    let lower = tail.to_ascii_lowercase();
    let catalog = builtin_catalog();
    for entry in catalog.for_preset(preset) {
        let (m, reset) = match_pattern_regex(&entry.regex, &lower);
        if m {
            let reset_at =
                reset.or_else(|| if entry.class == SessionFailureClass::RateLimited { extract_any_reset_time(preset, catalog, &lower) } else { None });
            return Some(SessionFailure::new(preset, entry.class, reset_at, turn_output));
        }
    }
    None
}

/// Public machine-readable status page endpoints (Atlassian Statuspage v2 unresolved incidents).
pub fn status_page_incidents_url(preset: AgentCliPreset) -> Option<&'static str> {
    match preset {
        AgentCliPreset::Claude => Some("https://status.anthropic.com/api/v2/incidents/unresolved.json"),
        AgentCliPreset::Codex => Some("https://status.openai.com/api/v2/incidents/unresolved.json"),
        AgentCliPreset::Grok => Some("https://status.x.ai/api/v2/incidents/unresolved.json"),
        AgentCliPreset::Antigravity | AgentCliPreset::Gemini | AgentCliPreset::OpenCode | AgentCliPreset::Shell | AgentCliPreset::DesignOsHelp => None,
    }
}

/// Endpoint probed by `RetryWhenOnline` for any agent CLI (uses the provider status page when
/// available, or Google's `generate_204` connectivity endpoint for `Antigravity`).
pub fn connectivity_probe_url(preset: AgentCliPreset) -> &'static str {
    status_page_incidents_url(preset).unwrap_or("https://www.gstatic.com/generate_204")
}

pub fn parse_first_incident_title(json_body: &str) -> Option<String> {
    let val: serde_json::Value = serde_json::from_str(json_body).ok()?;
    let incidents = val.get("incidents")?.as_array()?;
    let first = incidents.first()?;
    let name = first.get("name")?.as_str()?.trim();
    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    }
}

/// Compares `HH:MM` strings across midnight: returns `true` when `reset_hhmm` is still in the
/// forward 18-hour window after `now_hhmm` (so a reset at `00:10` is treated as future at `23:50`).
pub fn is_reset_in_future(reset_hhmm: &str, now_hhmm: &str) -> bool {
    let parse_mins = |s: &str| -> Option<u32> {
        let (h, m) = s.trim().split_once(':')?;
        let hh = h.parse::<u32>().ok()?;
        let mm = m.parse::<u32>().ok()?;
        if hh < 24 && mm < 60 {
            Some(hh * 60 + mm)
        } else {
            None
        }
    };
    match (parse_mins(reset_hhmm), parse_mins(now_hhmm)) {
        (Some(r), Some(n)) => {
            let forward = (r + 1440 - n) % 1440;
            forward > 0 && forward <= 18 * 60
        }
        _ => false,
    }
}

/// Returns the current local wall-clock time as `"HH:MM"`, cached per epoch minute so calling it
/// during banner paint never spawns more than once a minute.
pub fn current_local_hhmm() -> String {
    static CACHE: OnceLock<Mutex<Option<(u64, String)>>> = OnceLock::new();
    let epoch_secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let epoch_min = epoch_secs / 60;
    let mutex = CACHE.get_or_init(|| Mutex::new(None));
    if let Ok(guard) = mutex.lock() {
        if let Some((cached_min, ref hhmm)) = *guard {
            if cached_min == epoch_min {
                return hhmm.clone();
            }
        }
    }
    let computed = query_system_local_hhmm().unwrap_or_else(|| {
        let mins = (epoch_min % 1440) as u32;
        format!("{:02}:{:02}", (mins / 60) % 24, mins % 60)
    });
    if let Ok(mut guard) = mutex.lock() {
        *guard = Some((epoch_min, computed.clone()));
    }
    computed
}

fn query_system_local_hhmm() -> Option<String> {
    let out = if cfg!(windows) {
        std::process::Command::new("powershell").args(["-NoProfile", "-Command", "Get-Date -Format HH:mm"]).output().ok()?
    } else {
        std::process::Command::new("date").arg("+%H:%M").output().ok()?
    };
    if !out.status.success() {
        return None;
    }
    extract_hhmm_prefix(&String::from_utf8_lossy(&out.stdout))
}

/// Extracts a semver-like token (`X.Y` or `X.Y.Z`) from a string.
pub fn extract_semver_token(s: &str) -> Option<String> {
    for raw_word in s.split(|c: char| c.is_whitespace() || matches!(c, '(' | ')' | '[' | ']' | ',' | ';' | '"' | '\'')) {
        let word = raw_word.trim_start_matches(['v', 'V']).trim_end_matches('.');
        let core = word.split(['-', '+']).next().unwrap_or("");
        let parts: Vec<&str> = core.split('.').collect();
        if (2..=4).contains(&parts.len()) && parts.iter().all(|p| !p.is_empty() && p.len() <= 4 && p.chars().all(|c| c.is_ascii_digit())) {
            return Some(word.to_string());
        }
    }
    None
}

/// Extracts a CLI version from `raw_output` if a startup banner line naming the CLI is present,
/// else reads the background-warmed `<binary> --version` cache, falling back to `"unknown"`.
pub fn detect_cli_version(preset: AgentCliPreset, raw_output: &str) -> String {
    let first_chunk_end = raw_output.char_indices().nth(4096).map(|(i, _)| i).unwrap_or(raw_output.len());
    let cleaned = strip_ansi_codes(&raw_output[..first_chunk_end]);
    let bin_lower = preset.binary_name().to_ascii_lowercase();
    let title_lower = preset.tab_title().to_ascii_lowercase();
    for line in cleaned.lines().take(40) {
        let lower = line.to_ascii_lowercase();
        if lower.contains(&bin_lower) || lower.contains(&title_lower) || lower.contains("version ") {
            if let Some(ver) = extract_semver_token(line) {
                return ver;
            }
        }
    }
    probe_installed_cli_version(preset).unwrap_or_else(|| "unknown".to_string())
}

fn version_cache() -> &'static Mutex<std::collections::HashMap<String, Option<String>>> {
    static VERSION_CACHE: OnceLock<Mutex<std::collections::HashMap<String, Option<String>>>> = OnceLock::new();
    VERSION_CACHE.get_or_init(|| Mutex::new(std::collections::HashMap::new()))
}

#[cfg(test)]
thread_local! {
    static LAST_WARMER_RESOLVED: std::cell::RefCell<Option<Option<PathBuf>>> = const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
pub(crate) fn take_last_warmer_resolved() -> Option<Option<PathBuf>> {
    LAST_WARMER_RESOLVED.with(|slot| slot.borrow_mut().take())
}

/// Warms the installed CLI `--version` cache on a background thread so `poll_live_agent_bridge`
/// never spawns a subprocess on the UI thread.
pub fn warm_cli_version_cache(preset: AgentCliPreset) {
    let _ = probe_installed_cli_version(preset);
}

/// The installed CLI's `--version` as the probe of [`warm_cli_version_cache`] found it: `None` until that probe has finished, and
/// when the CLI is not found or prints no version.
#[cfg_attr(test, allow(dead_code))] // tests name the version themselves (`gemini_dialog::with_gemini_version`)
pub(crate) fn installed_cli_version(preset: AgentCliPreset) -> Option<String> {
    probe_installed_cli_version(preset)
}

fn probe_installed_cli_version(preset: AgentCliPreset) -> Option<String> {
    if matches!(preset, AgentCliPreset::Shell | AgentCliPreset::DesignOsHelp) {
        return None;
    }
    // A test never starts a real CLI, not even for `--version`: nothing in a test depends on the installed version, and a CLI
    // found on the developer's machine would run against whatever home it finds. (`version_probe_command` still gives the
    // probe a scratch home, as defense in depth.)
    let bin = preset.binary_name();
    let sources = crate::cli_resolve::current_search_sources(None);
    let dirs = crate::cli_resolve::cli_search_dirs_with(&sources);
    let resolved_bin = resolve_cli_executable(bin, &sources);
    #[cfg(test)]
    LAST_WARMER_RESOLVED.with(|slot| *slot.borrow_mut() = Some(resolved_bin.clone()));
    if cfg!(test) {
        return None;
    }
    let path_key = format!("{bin}:{dirs:?}");
    let cache = version_cache();
    if let Ok(mut guard) = cache.lock() {
        if let Some(cached) = guard.get(&path_key) {
            return cached.clone();
        }
        guard.insert(path_key.clone(), None);
    }
    std::thread::spawn(move || {
        let Some(resolved_bin) = resolved_bin else { return };
        let Ok(out) = version_probe_command(&resolved_bin, preset).output() else { return };
        let combined = format!("{}\n{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
        let detected = extract_semver_token(&combined);
        if let Ok(mut guard) = version_cache().lock() {
            guard.insert(path_key, detected);
        }
    });
    None
}

/// The `--version` command for the installed CLI of `_preset` at `bin`. It is the same for every CLI.
pub(crate) fn version_probe_command(bin: &Path, _preset: AgentCliPreset) -> std::process::Command {
    let mut cmd = std::process::Command::new(bin);
    cmd.arg("--version");
    // Opening an agent tab runs this probe, and a CLI writes under the user's home even for `--version` (Gemini registers the
    // project, for one). A test must not do that to the real home, so the probe gets a scratch one.
    #[cfg(test)]
    crate::scratch_cli_home::isolate_cli_homes(&mut cmd);
    cmd
}

pub(crate) fn resolve_cli_executable(bin: &str, sources: &crate::cli_resolve::SearchSources) -> Option<PathBuf> {
    let dirs = crate::cli_resolve::cli_search_dirs_with(sources);
    crate::cli_resolve::resolve_in(&dirs, bin)
}

impl SessionFailure {
    pub fn new(preset: AgentCliPreset, class: SessionFailureClass, reset_at: Option<String>, raw_output: &str) -> Self {
        let detail_line = match class {
            SessionFailureClass::RateLimited => match &reset_at {
                Some(t) => format!("Rate limited until {t}"),
                None => "Rate limited".to_string(),
            },
            SessionFailureClass::LoginExpired => "Login expired".to_string(),
            SessionFailureClass::BillingOrQuota => "Billing or quota problem".to_string(),
            SessionFailureClass::ProviderServerError => "The provider has a problem.".to_string(),
            SessionFailureClass::ContextFull => "This conversation is full.".to_string(),
            SessionFailureClass::NetworkOffline => "No network".to_string(),
            // Rule 5: Unknown failure names no class and no cause; shows last output lines + Copy details.
            SessionFailureClass::Unknown => String::new(),
        };
        let fix = class.fix_for_preset(preset);
        let tail_slice = tail_slice_last_n_lines(raw_output, CLASSIFY_TAIL_LINES);
        let output_tail = agent_start_failure::output_excerpt(tail_slice, &[]);
        let raw_tail = strip_ansi_codes(tail_slice);
        let cli_version = detect_cli_version(preset, raw_output);
        Self {
            preset,
            class,
            detail_line,
            reset_at,
            fix,
            output_tail,
            raw_tail,
            cli_version,
            probe_url_override: None,
            incident_title: Arc::new(Mutex::new(None)),
            status_fetch_started: Arc::new(AtomicBool::new(false)),
            status_fetch_count: Arc::new(AtomicUsize::new(0)),
            online_attempts: Arc::new(AtomicUsize::new(0)),
            online_probe_state: Arc::new(AtomicU8::new(0)),
        }
    }

    /// Sentence shown in the banner (`#686` §3.4–3.5).
    pub fn banner_sentence(&self) -> String {
        if self.class == SessionFailureClass::ProviderServerError {
            if let Ok(guard) = self.incident_title.lock() {
                if let Some(title) = guard.as_ref() {
                    if !title.trim().is_empty() {
                        return format!("The provider has a problem ({title}).");
                    }
                }
            }
        }
        self.detail_line.clone()
    }

    /// Label of the primary action button when `self.fix` is present (`#686` §3.4).
    pub fn action_label(&self) -> Option<&'static str> {
        match self.class {
            SessionFailureClass::RateLimited => Some("Wait and retry"),
            SessionFailureClass::LoginExpired => Some("Run login"),
            SessionFailureClass::BillingOrQuota => Some("Open billing"),
            SessionFailureClass::ProviderServerError => Some("Retry"),
            SessionFailureClass::ContextFull => Some("Start new session"),
            SessionFailureClass::NetworkOffline => Some("Retry connection"),
            SessionFailureClass::Unknown => None,
        }
    }

    /// Whether the action button is enabled given `tab_alive` and optional `now_hhmm` (`#686` §3.4, §8.2, §8.8).
    /// On a dead tab the retry actions are disabled.
    pub fn is_action_enabled(&self, tab_alive: bool, now_hhmm: Option<&str>) -> bool {
        match &self.fix {
            None => false,
            Some(
                FixAction::NewSession
                | FixAction::StartNewSession(_)
                | FixAction::RunLogin(_)
                | FixAction::OpenBilling(_)
                | FixAction::OpenInstallPage(_)
                | FixAction::OpenShell,
            ) => true,
            // A dead tab has nothing to retry on (§8.8): only the actions that open or start something else stay.
            Some(FixAction::RetryWhenOnline) => tab_alive && self.online_probe_state.load(Ordering::SeqCst) != 1,
            Some(FixAction::RetryText) => {
                if !tab_alive {
                    return false;
                }
                if let Some(reset) = self.reset_at.as_deref() {
                    let fallback_now;
                    let now = match now_hhmm {
                        Some(s) => s,
                        None => {
                            fallback_now = current_local_hhmm();
                            fallback_now.as_str()
                        }
                    };
                    if is_reset_in_future(reset, now) {
                        return false;
                    }
                }
                true
            }
        }
    }

    /// Triggers at most one status-page check for `ProviderServerError` (`#686` §3.4, Test 7).
    /// Never blocks the UI thread, never polls on subsequent calls, and requests a UI repaint via `ctx` when an incident title arrives.
    pub fn ensure_status_page_checked_with_ctx(&self, ctx: Option<egui::Context>) {
        if self.class != SessionFailureClass::ProviderServerError {
            return;
        }
        let Some(url) = status_page_incidents_url(self.preset) else {
            return;
        };
        if self.status_fetch_started.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst).is_err() {
            return;
        }
        self.status_fetch_count.fetch_add(1, Ordering::SeqCst);
        if cfg!(test) {
            return;
        }
        let sink = Arc::clone(&self.incident_title);
        std::thread::spawn(move || {
            let agent = ureq::AgentBuilder::new().timeout_connect(Duration::from_secs(2)).timeout_read(Duration::from_secs(2)).build();
            if let Ok(resp) = agent.get(url).call() {
                if let Ok(body) = resp.into_string() {
                    if let Some(title) = parse_first_incident_title(&body) {
                        if let Ok(mut guard) = sink.lock() {
                            *guard = Some(title);
                        }
                        if let Some(c) = ctx {
                            c.request_repaint();
                        }
                    }
                }
            }
        });
    }

    /// Synchronous status-page check with an injected fetcher (used by tests to verify single-shot
    /// behavior without network I/O).
    #[cfg(test)]
    pub fn ensure_status_page_checked_with<F>(&self, fetcher: F)
    where
        F: FnOnce(&str) -> Option<String>,
    {
        if self.class != SessionFailureClass::ProviderServerError {
            return;
        }
        let Some(url) = status_page_incidents_url(self.preset) else {
            return;
        };
        if self.status_fetch_started.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst).is_err() {
            return;
        }
        self.status_fetch_count.fetch_add(1, Ordering::SeqCst);
        if let Some(body) = fetcher(url) {
            if let Some(title) = parse_first_incident_title(&body) {
                if let Ok(mut guard) = self.incident_title.lock() {
                    *guard = Some(title);
                }
            }
        }
    }

    /// Makes exactly one connectivity attempt for `RetryWhenOnline` (`#686` §3.4, Test 8).
    /// Returns `true` when the probe succeeded.
    #[cfg(test)]
    pub fn attempt_retry_when_online_with<F>(&self, probe: F) -> bool
    where
        F: FnOnce() -> bool,
    {
        self.online_attempts.fetch_add(1, Ordering::SeqCst);
        probe()
    }

    /// Default single-shot connectivity probe for `RetryWhenOnline` (`#686` §3.4).
    /// Probes every CLI (including `Antigravity` via `connectivity_probe_url`).
    #[cfg(test)]
    pub fn attempt_retry_when_online(&self) -> bool {
        let url = self.probe_url_override.clone().unwrap_or_else(|| connectivity_probe_url(self.preset).to_string());
        self.attempt_retry_when_online_with(|| {
            if cfg!(test) && self.probe_url_override.is_none() {
                return true;
            }
            let agent = ureq::AgentBuilder::new().timeout_connect(Duration::from_millis(800)).timeout_read(Duration::from_millis(800)).build();
            agent.get(&url).call().is_ok()
        })
    }

    /// Starts a single connectivity probe on a background worker thread (`#686` §11) so clicking
    /// `"Retry connection"` never blocks the UI thread.
    pub fn start_retry_when_online_probe(&self, ctx: Option<egui::Context>) -> bool {
        if self.online_probe_state.compare_exchange(0, 1, Ordering::SeqCst, Ordering::SeqCst).is_err() {
            return false;
        }
        self.online_attempts.fetch_add(1, Ordering::SeqCst);
        let url = self.probe_url_override.clone().unwrap_or_else(|| connectivity_probe_url(self.preset).to_string());
        let is_test_default = cfg!(test) && self.probe_url_override.is_none();
        let state_sink = Arc::clone(&self.online_probe_state);
        std::thread::spawn(move || {
            let ok = if is_test_default {
                true
            } else {
                let agent = ureq::AgentBuilder::new().timeout_connect(Duration::from_millis(800)).timeout_read(Duration::from_millis(800)).build();
                agent.get(&url).call().is_ok()
            };
            state_sink.store(if ok { 2 } else { 3 }, Ordering::SeqCst);
            if let Some(c) = ctx {
                c.request_repaint();
            }
        });
        true
    }

    /// Polls for completion of a background `RetryWhenOnline` worker probe (`Some(true)` = online,
    /// `Some(false)` = offline, `None` = no completed probe).
    pub fn poll_retry_when_online_probe(&self) -> Option<bool> {
        match self.online_probe_state.load(Ordering::SeqCst) {
            2 => {
                self.online_probe_state.store(0, Ordering::SeqCst);
                Some(true)
            }
            3 => {
                self.online_probe_state.store(0, Ordering::SeqCst);
                Some(false)
            }
            _ => None,
        }
    }

    /// Formats and redacts the 4-part "Copy details" clipboard string (`#686` §3.6).
    pub fn format_copy_details_payload(&self, cli_version: &str, session_id: &str) -> String {
        let trimmed = cli_version.trim();
        let base_version = if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("cli") { self.cli_version.as_str() } else { trimmed };
        let cached_fallback;
        let effective_version = if base_version == "unknown" {
            cached_fallback = probe_installed_cli_version(self.preset).unwrap_or_else(|| "unknown".to_string());
            cached_fallback.as_str()
        } else {
            base_version
        };
        let cli_header = format!("{} {}", self.preset.binary_name(), effective_version);
        format_copy_details(&cli_header, self.class.slug(), &self.raw_tail, session_id)
    }
}

/// User interaction returned by [`paint_banner`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BannerEvent {
    RunFix(FixAction),
    CopyDetails(String),
}

/// Paints the mid-session failure banner at the top of `viewport_rect` (`#686` §7) and returns
/// `(remaining_viewport_rect, optional_event)`.
pub fn paint_banner(
    ui: &mut Ui,
    viewport_rect: Rect,
    failure: &SessionFailure,
    session_id: &str,
    tab_alive: bool,
    now_hhmm: Option<&str>,
    surface_id: &'static str,
) -> (Rect, Option<BannerEvent>) {
    failure.ensure_status_page_checked_with_ctx(Some(ui.ctx().clone()));

    let p: Painter = ui.painter().clone();
    let inset = theme::INSET_FRAME;
    let is_unknown = failure.class == SessionFailureClass::Unknown;
    let tail_count = if is_unknown { failure.output_tail.len().min(3) } else { 0 };
    let banner_h = if is_unknown { inset * 2.0 + theme::CONTROL_XS.max((tail_count as f32) * theme::LINE_CAPTION) } else { inset * 2.0 + theme::CONTROL_XS };

    let warn = matches!(failure.class, SessionFailureClass::RateLimited | SessionFailureClass::NetworkOffline);
    let banner_rect = Rect::from_min_size(viewport_rect.min, Vec2::new(viewport_rect.width(), banner_h));
    p.rect_filled(banner_rect, 0.0, if warn { theme::BG_WARNING_PRIMARY } else { theme::BG_ERROR_PRIMARY });
    p.rect_stroke(
        banner_rect,
        0.0,
        Stroke::new(theme::BORDER_WIDTH, if warn { theme::BADGE_WARNING_BORDER } else { theme::BADGE_ERROR_BORDER }),
        StrokeKind::Inside,
    );
    widgets::record(ui, &format!("agents-session-failure-banner-{surface_id}"), banner_rect);

    let dot_center = Pos2::new(banner_rect.left() + inset + theme::SPACE_FACT, banner_rect.center().y);
    if warn {
        p.circle_filled(dot_center, theme::SPACE_FACT, theme::WARNING);
    } else {
        crate::widgets::kit::status_dot(&p, dot_center, crate::widgets::kit::DotTone::Error);
    }

    let mut right_x = banner_rect.right() - inset;
    let mut event: Option<BannerEvent> = None;

    if let (Some(label), Some(fix)) = (failure.action_label(), failure.fix.as_ref()) {
        let enabled = failure.is_action_enabled(tab_alive, now_hhmm);
        let btn_font = theme::font_text(theme::FONT_SIZE_TEXT_SM, theme::TextWeight::Semibold);
        let text_w = widgets::text_width(&p, label, &btn_font);
        let btn_w = (text_w + theme::INSET_BUTTON * 2.0).max(theme::TARGET_MIN);
        let btn_rect = Rect::from_min_size(Pos2::new(right_x - btn_w, banner_rect.center().y - theme::CONTROL_XS * 0.5), Vec2::new(btn_w, theme::CONTROL_XS));
        widgets::record(ui, &format!("agents-session-failure-action-{surface_id}"), btn_rect);
        let resp = crate::widgets::kit::button(
            ui,
            btn_rect,
            (surface_id, "agents-session-failure-action"),
            label,
            crate::widgets::kit::Hierarchy::Secondary,
            crate::widgets::kit::Size::Xs,
            enabled,
        );
        if enabled && resp.clicked() {
            event = Some(BannerEvent::RunFix(fix.clone()));
        }
        right_x = btn_rect.left() - theme::SPACE_INLINE;
    }

    let copy_label = "Copy details";
    let copy_font = theme::font_text(theme::FONT_SIZE_TEXT_SM, theme::TextWeight::Semibold);
    let copy_w = (widgets::text_width(&p, copy_label, &copy_font) + theme::INSET_BUTTON * 2.0).max(theme::TARGET_MIN);
    let copy_rect = Rect::from_min_size(Pos2::new(right_x - copy_w, banner_rect.center().y - theme::CONTROL_XS * 0.5), Vec2::new(copy_w, theme::CONTROL_XS));
    widgets::record(ui, &format!("agents-session-failure-copy-{surface_id}"), copy_rect);
    let copy_resp = crate::widgets::kit::button(
        ui,
        copy_rect,
        (surface_id, "agents-session-failure-copy"),
        copy_label,
        crate::widgets::kit::Hierarchy::Tertiary,
        crate::widgets::kit::Size::Xs,
        true,
    );
    if copy_resp.clicked() && event.is_none() {
        let payload = failure.format_copy_details_payload(&failure.cli_version, session_id);
        event = Some(BannerEvent::CopyDetails(payload));
    }
    right_x = copy_rect.left() - theme::SPACE_INLINE;

    let text_left = banner_rect.left() + inset + theme::SPACE_ROW * 2.0 + theme::SPACE_INLINE;
    let avail_w = (right_x - text_left).max(24.0);

    if is_unknown {
        let lines_to_show: Vec<String> = failure.output_tail.iter().rev().take(3).rev().cloned().collect();
        let total_h = (lines_to_show.len() as f32) * theme::LINE_CAPTION;
        let mut y = banner_rect.center().y - total_h * 0.5 + theme::LINE_CAPTION * 0.5;
        for line in &lines_to_show {
            let elided = widgets::elide(&p, line, &theme::font_mono(), avail_w);
            p.text(Pos2::new(text_left, y), Align2::LEFT_CENTER, elided, theme::font_mono(), theme::TEXT_SECONDARY);
            y += theme::LINE_CAPTION;
        }
    } else if failure.class == SessionFailureClass::RateLimited {
        if let Some(reset) = &failure.reset_at {
            let prefix = "Rate limited until ";
            let prefix_elided = widgets::elide(&p, prefix, &theme::font_body(), avail_w);
            let prefix_w = widgets::text_width(&p, &prefix_elided, &theme::font_body());
            p.text(Pos2::new(text_left, banner_rect.center().y), Align2::LEFT_CENTER, &prefix_elided, theme::font_body(), theme::TEXT);
            if prefix_w + 16.0 < avail_w {
                let time_str = widgets::elide(&p, &format!("{reset}."), &theme::font_mono(), avail_w - prefix_w);
                p.text(Pos2::new(text_left + prefix_w, banner_rect.center().y), Align2::LEFT_CENTER, time_str, theme::font_mono(), theme::TEXT);
            }
        } else {
            let shown = widgets::elide(&p, &failure.banner_sentence(), &theme::font_body(), avail_w);
            p.text(Pos2::new(text_left, banner_rect.center().y), Align2::LEFT_CENTER, shown, theme::font_body(), theme::TEXT);
        }
    } else {
        let shown = widgets::elide(&p, &failure.banner_sentence(), &theme::font_body(), avail_w);
        p.text(Pos2::new(text_left, banner_rect.center().y), Align2::LEFT_CENTER, shown, theme::font_body(), theme::TEXT);
    }

    let remaining = Rect::from_min_max(Pos2::new(viewport_rect.left(), banner_rect.bottom()), viewport_rect.max);
    (remaining, event)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_class_fixture_shows_its_banner_and_action() {
        let catalog = parse_patterns_toml(PATTERNS_TOML, true).expect("all pattern fixtures must exist and be non-empty");
        assert_eq!(catalog.entries.len(), 24, "4 CLIs x 6 classes = 24 fixture-backed patterns");

        for entry in &catalog.entries {
            let preset = match entry.cli.as_str() {
                "claude" => AgentCliPreset::Claude,
                "codex" => AgentCliPreset::Codex,
                "grok" => AgentCliPreset::Grok,
                "antigravity" => AgentCliPreset::Antigravity,
                other => panic!("unexpected cli {other}"),
            };
            let fixture_path = resolve_fixture_path(&entry.fixture);
            let fixture_text = std::fs::read_to_string(&fixture_path).expect("read fixture");
            let classified = classify_with_catalog(preset, None, &fixture_text, &catalog).expect("classified");
            assert_eq!(
                classified.class, entry.class,
                "fixture {} for {} classified as {:?}, expected {:?}",
                entry.fixture, entry.cli, classified.class, entry.class
            );
            assert_eq!(classified.fix, entry.class.fix_for_preset(preset));
            if entry.class == SessionFailureClass::RateLimited {
                assert!(classified.reset_at.is_some(), "rate-limited fixture {} must extract reset time", entry.fixture);
                assert!(classified.banner_sentence().starts_with("Rate limited until "));
            }
        }
    }

    #[test]
    fn hook_class_wins_over_output_patterns() {
        let login_like_output = "API Error: 401 OAuth token has expired. Please run /login to sign in again.";
        let failure = classify_session_failure(AgentCliPreset::Claude, Some("rate_limit"), login_like_output).expect("classified");
        assert_eq!(failure.class, SessionFailureClass::RateLimited, "StopFailure hook class must beat output patterns");
        assert_eq!(failure.fix, Some(FixAction::RetryText));
    }

    #[test]
    fn unknown_error_shows_no_class() {
        let raw = "fatal: unexpected internal panic at worker.rs:42\nstack backtrace: 0x1234\n";
        let failure = classify_session_failure(AgentCliPreset::Claude, Some("unknown"), raw).expect("classified");
        assert_eq!(failure.class, SessionFailureClass::Unknown);
        assert!(failure.fix.is_none(), "unknown failure must have no fix action");
        assert!(failure.banner_sentence().is_empty(), "unknown failure must name no class and no cause");

        let ctx = egui::Context::default();
        theme::install_fonts(&ctx);
        theme::apply(&ctx);
        let rect = Rect::from_min_size(Pos2::new(0.0, 0.0), Vec2::new(600.0, 300.0));
        let raw_in = egui::RawInput { screen_rect: Some(rect), ..Default::default() };
        let out = ctx.run_ui(raw_in, |ui| {
            let _ = paint_banner(ui, rect, &failure, "sess-1", false, None, "test-unknown");
        });
        let (texts, _) = crate::lint::scan_shapes(&out.shapes);
        let all: Vec<&str> = texts.iter().map(|t| t.text.as_str()).collect();
        assert!(all.contains(&"Copy details"), "Copy details button must be painted: {all:?}");
        assert!(all.iter().any(|t| t.contains("unexpected internal panic")), "last output lines must be painted: {all:?}");
        for forbidden in ["Unknown", "unknown", "Rate limited", "Login expired", "Billing", "provider has a problem"] {
            assert!(!all.iter().any(|t| t.contains(forbidden)), "unknown banner must not name a class or cause ({forbidden:?}): {all:?}");
        }
    }

    #[test]
    fn normal_output_with_the_word_login_raises_no_banner() {
        let normal = "Implemented the login form component with email and password fields and unit tests.";
        assert!(classify_turn_output_if_matched(AgentCliPreset::Codex, normal).is_none(), "normal output mentioning 'login' must not raise a banner");
    }

    #[test]
    fn status_page_is_fetched_once_and_never_polled() {
        let failure =
            classify_session_failure(AgentCliPreset::Claude, Some("server_error"), "API Error: 529 Overloaded: Anthropic API is temporarily overloaded.")
                .expect("classified");
        assert_eq!(failure.class, SessionFailureClass::ProviderServerError);

        let calls = AtomicUsize::new(0);
        for _ in 0..5 {
            failure.ensure_status_page_checked_with(|_url| {
                calls.fetch_add(1, Ordering::SeqCst);
                Some(r#"{"incidents":[{"name":"Elevated API error rates on Claude"}]}"#.to_string())
            });
        }
        assert_eq!(calls.load(Ordering::SeqCst), 1, "status page must be fetched exactly once across repeated paints");
        assert_eq!(failure.status_fetch_count.load(Ordering::SeqCst), 1);
        assert_eq!(failure.banner_sentence(), "The provider has a problem (Elevated API error rates on Claude).");
    }

    #[test]
    fn retry_when_online_makes_one_attempt_per_click() {
        let failure =
            classify_session_failure(AgentCliPreset::Claude, Some("network_offline"), "TypeError: fetch failed (ECONNREFUSED): unable to connect to network.")
                .expect("classified");
        assert_eq!(failure.class, SessionFailureClass::NetworkOffline);
        assert_eq!(failure.online_attempts.load(Ordering::SeqCst), 0);

        assert!(!failure.attempt_retry_when_online_with(|| false));
        assert_eq!(failure.online_attempts.load(Ordering::SeqCst), 1);

        assert!(failure.attempt_retry_when_online_with(|| true));
        assert_eq!(failure.online_attempts.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn pattern_without_a_fixture_is_rejected() {
        let missing_fixture_key = r#"
[[cli.claude.pattern]]
class = "rate_limited"
regex = "429.*rate limit"
"#;
        assert!(parse_patterns_toml(missing_fixture_key, true).is_err(), "pattern without fixture key must be rejected");

        let empty_fixture = r#"
[[cli.claude.pattern]]
class = "rate_limited"
regex = "429.*rate limit"
fixture = ""
"#;
        assert!(parse_patterns_toml(empty_fixture, true).is_err(), "pattern with empty fixture must be rejected");

        let nonexistent_fixture = r#"
[[cli.claude.pattern]]
class = "rate_limited"
regex = "429.*rate limit"
fixture = "tests/fixtures/agent-failures/does-not-exist.txt"
"#;
        assert!(parse_patterns_toml(nonexistent_fixture, true).is_err(), "pattern with missing fixture file must be rejected");
    }

    #[test]
    fn unknown_failure_with_empty_output_tail_paints_no_invented_cause() {
        let failure = classify_session_failure(AgentCliPreset::Claude, Some("unknown"), "").expect("classified");
        assert_eq!(failure.class, SessionFailureClass::Unknown);
        assert!(failure.output_tail.is_empty());

        let ctx = egui::Context::default();
        theme::install_fonts(&ctx);
        theme::apply(&ctx);
        let rect = Rect::from_min_size(Pos2::new(0.0, 0.0), Vec2::new(600.0, 300.0));
        let raw_in = egui::RawInput { screen_rect: Some(rect), ..Default::default() };
        let out = ctx.run_ui(raw_in, |ui| {
            let _ = paint_banner(ui, rect, &failure, "sess-empty", true, None, "test-unknown-empty");
        });
        let (texts, _) = crate::lint::scan_shapes(&out.shapes);
        let all: Vec<&str> = texts.iter().map(|t| t.text.as_str()).collect();
        assert_eq!(all, vec!["Copy details"], "Unknown with empty output_tail must paint only Copy details and no invented cause: {all:?}");
    }

    #[test]
    fn copy_details_includes_detected_cli_version_instead_of_literal_cli() {
        let raw = "Claude Code v2.1.112\nError: 429 rate_limit_error: Rate limit reached. Resets at 14:30\n";
        let failure = classify_session_failure(AgentCliPreset::Claude, None, raw).expect("classified");
        assert_eq!(failure.cli_version, "2.1.112");

        let ctx = egui::Context::default();
        theme::install_fonts(&ctx);
        theme::apply(&ctx);
        let rect = Rect::from_min_size(Pos2::new(0.0, 0.0), Vec2::new(600.0, 300.0));
        let copy_center = |ctx: &egui::Context| -> Pos2 {
            let r: Rect = ctx.data(|d| d.get_temp(Id::new(("test-rect", "agents-session-failure-copy-test-copy-ver".to_string())))).expect("copy button rect");
            r.center()
        };
        let mut captured_evt = None;
        for step in 0..4 {
            let events = match step {
                0 => vec![],
                1 => {
                    let at = copy_center(&ctx);
                    vec![
                        egui::Event::PointerMoved(at),
                        egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: true, modifiers: Default::default() },
                    ]
                }
                2 => {
                    let at = copy_center(&ctx);
                    vec![egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: false, modifiers: Default::default() }]
                }
                _ => vec![],
            };
            let raw_in = egui::RawInput { screen_rect: Some(rect), events, ..Default::default() };
            let _ = ctx.run_ui(raw_in, |ui| {
                let (_, evt) = paint_banner(ui, rect, &failure, "sess-ver", true, Some("15:00"), "test-copy-ver");
                if evt.is_some() {
                    captured_evt = evt;
                }
            });
        }
        let Some(BannerEvent::CopyDetails(payload)) = captured_evt else {
            panic!("expected CopyDetails event from clicking Copy details");
        };
        assert!(payload.starts_with("cli: claude 2.1.112\n"), "Copy details must include detected CLI version: {payload}");
        assert!(!payload.contains("cli: claude cli"), "Copy details must never hardcode literal 'cli': {payload}");
    }

    #[test]
    fn antigravity_retry_when_online_probes_network_and_returns_false_when_unreachable() {
        let mut failure = classify_session_failure(
            AgentCliPreset::Antigravity,
            Some("network_offline"),
            "Antigravity network error: ECONNREFUSED — unable to reach Google API endpoint.",
        )
        .expect("classified");
        assert_eq!(failure.class, SessionFailureClass::NetworkOffline);
        assert!(!connectivity_probe_url(AgentCliPreset::Antigravity).is_empty());
        failure.probe_url_override = Some("http://127.0.0.1:1".to_string());
        assert!(!failure.attempt_retry_when_online(), "offline Antigravity probe must return false when endpoint is unreachable");
        assert_eq!(failure.online_attempts.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn rate_limited_action_is_gated_against_current_clock_and_disabled_after_process_exit() {
        let failure =
            classify_session_failure(AgentCliPreset::Claude, None, "Error: 429 rate_limit_error: Rate limit reached. Resets at 23:59\n").expect("classified");
        assert_eq!(failure.reset_at.as_deref(), Some("23:59"));
        assert!(!failure.is_action_enabled(true, Some("12:00")), "future reset time must disable Wait and retry");
        assert!(failure.is_action_enabled(true, Some("23:59")), "reaching reset time must enable Wait and retry");
        assert!(!failure.is_action_enabled(false, Some("23:59")), "a dead tab has nothing to retry on");
    }

    /// Spec 686 §8.8: when the tab is dead, "Start a new session" still works and the retry actions are disabled.
    #[test]
    fn retry_actions_are_disabled_on_a_dead_tab_and_the_other_actions_stay_enabled() {
        let retrying = [("API Error: 529 overloaded_error\n", FixAction::RetryText), ("unable to connect to network\n", FixAction::RetryWhenOnline)];
        for (output, fix) in retrying {
            let failure = classify_session_failure(AgentCliPreset::Claude, None, output).expect("classified");
            assert_eq!(failure.fix, Some(fix.clone()));
            assert!(failure.is_action_enabled(true, Some("12:00")), "{fix:?} works on a live tab");
            assert!(!failure.is_action_enabled(false, Some("12:00")), "{fix:?} is disabled on a dead tab");
        }
        let lasting = [
            ("prompt is too long\n", "Start new session"),
            ("credit balance is too low\n", "Open billing"),
            ("Error: 401 authentication_error: Invalid API key\n", "Run login"),
        ];
        for (output, label) in lasting {
            let failure = classify_session_failure(AgentCliPreset::Claude, None, output).expect("classified");
            assert_eq!(failure.action_label(), Some(label));
            assert!(failure.is_action_enabled(false, Some("12:00")), "{label} still works on a dead tab");
        }
    }

    #[test]
    fn is_reset_in_future_handles_midnight_wraparound() {
        assert!(is_reset_in_future("00:10", "23:50"), "reset at 00:10 must still be in the future at 23:50");
        assert!(!is_reset_in_future("00:10", "00:10"), "reaching 00:10 must enable retry");
        assert!(!is_reset_in_future("00:10", "00:15"), "00:15 is past 00:10");
        assert!(!is_reset_in_future("23:50", "00:05"), "23:50 yesterday is in the past at 00:05");
    }
}
