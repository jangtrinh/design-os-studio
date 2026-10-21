//! Agent composer (`#689` / `AX-689`): per-tab multi-line composer (`1..=8` lines),
//! 100-entry Up/Down prompt history ring with draft stash, canvas layer reference chips,
//! bracketed-paste + delayed-Enter delivery into the active tab's live CLI session,
//! start/resume queue with failed-start draft recovery, and 10s unconfirmed receipts.

use std::collections::VecDeque;
use std::path::Path;
use std::time::{Duration, Instant};

use designos_doc::{Document, NodeId, NodeKind};
use egui::{Align2, Event, Id, Key, Pos2, Rect, RichText, Sense, Stroke, StrokeKind, TextEdit, Ui, Vec2};

use crate::icons::{self, Icon};
use crate::terminal::{AgentCliPreset, AgentTabStatus, TerminalAction, TerminalState, TerminalTab};
use crate::theme;
use crate::widgets;

/// Maximum number of sent prompts retained per tab (`#689` §3 rule 4 & §4).
pub const MAX_HISTORY_ENTRIES: usize = 100;

/// Maximum character count accepted by the composer (`#689` §3 rule 10).
pub const MAX_COMPOSER_CHARS: usize = 100_000;

/// Minimum visible line count of the composer text field (`#689` §3 rule 3 & §7).
pub const MIN_COMPOSER_LINES: usize = 1;

/// Maximum visible line count before the composer text field scrolls (`#689` §3 rule 3 & §7).
pub const MAX_COMPOSER_LINES: usize = 8;

/// Delay between bracketed paste and `\r` for Claude (`#689` §3 rule 1).
pub const CLAUDE_ENTER_DELAY: Duration = Duration::from_millis(50);

/// Base delay between bracketed paste and `\r` for non-Claude CLIs (`#689` §3 rule 1).
pub const OTHER_CLI_BASE_ENTER_DELAY: Duration = Duration::from_millis(500);

/// Bytes per additional millisecond of Enter delay on non-Claude CLIs (`#689` §3 rule 1).
pub const ENTER_DELAY_BYTES_PER_MS: usize = 4_096;

/// Quiet period after bracketed-paste mode is enabled before a queued send flushes (`#689` §3 rule 2).
pub const READY_QUIET_BRACKETED: Duration = Duration::from_millis(60);

/// Quiet period after initial PTY output before a queued send flushes (`#689` §3 rule 2).
pub const READY_QUIET_OUTPUT: Duration = Duration::from_millis(180);

/// Timeout after which an unacknowledged write shows the unconfirmed receipt (`#689` §3 rule 7a).
pub const UNCONFIRMED_TIMEOUT: Duration = Duration::from_secs(10);

const ZERO: f32 = 0.0;
const HALF: f32 = 0.5;
const FOCUS_RING_WIDTH: f32 = 1.5;
const CHIP_MAX_WIDTH: f32 = 148.0;
const CHIP_TEXT_PAD: f32 = theme::INSET_BUTTON * 2.0;
const PRD_CHIP_WIDTH: f32 = theme::CONTROL_ROW;
const MIN_CENTER_HEIGHT: f32 = theme::CONTROL_TOOL * 2.0;

/// Why [`TerminalState::send_to_agent`] refused to write a prompt (`#689` §3 rules 3, 7, 10 & §8).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RefuseReason {
    /// The prompt is empty (`#689` §3 rule 3 & §8 edge case 8).
    Empty,
    /// The prompt exceeds [`MAX_COMPOSER_CHARS`] (`#689` §3 rule 10).
    TooLong,
    /// The active tab runs a login shell rather than an agent CLI (`#689` §3 rule 7 & §8 edge case 9).
    NoAgent,
    /// The CLI is waiting at a permission or selection prompt (`#689` §3 rule 7).
    Permission,
    /// The tab is still starting and explicitly gated (`#689` §3 rule 7).
    NotReady,
    /// The PTY master stdin rejected the write (`#689` §3 rule 7).
    NotWritable,
    /// The tab is showing the start-failure surface (`#689` §8 edge case 5).
    StartFailed,
}

impl RefuseReason {
    /// User-facing status caption shown under the composer when a send is refused.
    pub fn message(&self) -> &'static str {
        match self {
            Self::Empty => "Enter a prompt to send",
            Self::TooLong => "Message exceeds 100,000 characters",
            Self::NoAgent => "Select an agent tab",
            Self::Permission => "Answer the permission prompt in the terminal",
            Self::NotReady => "Agent is still starting",
            Self::NotWritable => "Not delivered: check the terminal",
            Self::StartFailed => "Not delivered: check the terminal",
        }
    }
}

/// Receipt returned by [`TerminalState::send_to_agent`] (`#689` §3 rule 7a & §5).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SendOutcome {
    /// Bracketed paste was written to the active agent tab's PTY (`\r` follows on deadline).
    Written,
    /// No live agent tab was running; `open_for_agent_use` was triggered and the message is queued.
    Queued,
    /// Send was refused for [`RefuseReason`].
    Refused(RefuseReason),
    /// Sent more than 10 seconds ago with no turn-start hook or PTY response (`#689` §3 rule 7a).
    Unconfirmed,
}

/// A canvas layer reference pill attached to the composer by pasting copied layers (`#689` §3 rule 6 & §7).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LayerChip {
    pub node_id: NodeId,
    pub name: String,
    pub is_component: bool,
    /// `#655` canonical reference URI (`designos:node/<id>`).
    pub reference: String,
}

/// Document and page searched by the composer's `@`-mention picker (`#655`).
#[derive(Clone, Copy)]
pub struct MentionScope<'a> {
    pub doc: &'a Document,
    pub page: NodeId,
}

/// Per-tab composer state (`#689` §4).
#[derive(Clone, Debug, Default)]
pub struct ComposerState {
    /// Current unsent text in this tab's composer field.
    pub draft: String,
    /// Sent prompts for this tab, newest first (`history[0]`), capped at [`MAX_HISTORY_ENTRIES`].
    pub history: VecDeque<String>,
    /// Current index into `history` while browsing with Up/Down, or `None` when editing `draft`.
    pub cursor: Option<usize>,
    /// Half-typed `draft` stashed when history browsing begins (`cursor: None -> Some(0)`).
    pub stash: String,
    /// Prompt waiting for the tab's CLI to finish starting (`#689` §3 rule 2 & §4).
    pub pending_send: Option<String>,
    /// True when `pending_send` came from a system sender (image jobs): it never returns to the user's draft.
    pub pending_send_is_system: bool,
    /// Pasted canvas layer reference chips (`#689` §3 rule 6 & §7).
    pub chips: Vec<LayerChip>,
    /// Receipt of the most recent send in this tab (`#689` §3 rule 7a).
    pub last_outcome: Option<SendOutcome>,
    /// Caption shown beneath the composer (queued notice, refusal reason, or unconfirmed note).
    pub notice: Option<String>,
    /// Wall-clock deadline at which the trailing `\r` for the last bracketed paste must be written.
    pub pending_enter_deadline: Option<Instant>,
    /// Exact per-CLI delay (`CLAUDE_ENTER_DELAY` or `OTHER_CLI_BASE_ENTER_DELAY + 1ms/4096B`) required before writing `\r`.
    pub pending_enter_delay: Option<Duration>,
    /// Synthetic `egui` input time (`i.time`) when the paste was written, so headless `Sim` frames advance it after `pending_enter_delay`.
    pub pending_enter_sim_time: Option<f64>,
    /// When the last `Written` prompt was sent while waiting for turn-start or PTY output confirmation.
    pub awaiting_confirm_since: Option<Instant>,
    /// Length of `tab.raw_log` at the moment the prompt was written.
    pub log_len_at_send: usize,
    /// Prompt written while the tab was still inside `START_FAILURE_WINDOW`, restored if start fails.
    pub sent_in_start_window: Option<String>,
    /// Optional explicit gate set when the caller marks the tab as still starting (`not-ready`).
    pub not_ready_gate: bool,
    /// Total number of prompts routed through `send_to_agent` on this composer.
    pub send_count: usize,
    /// True once the single-line fallback status for unbracketed multiline paste has been shown (`#689` §8.1).
    pub unbracketed_newline_warned: bool,
}

impl ComposerState {
    /// Records `text` at the front of this tab's history ring (up to [`MAX_HISTORY_ENTRIES`]),
    /// skipping consecutive duplicates (`#689` §3 rule 4 & §8 edge case 3).
    pub fn record_sent(&mut self, text: &str) {
        let clean = text.trim();
        if clean.is_empty() {
            return;
        }
        if self.history.front().map(String::as_str) != Some(clean) {
            self.history.push_front(clean.to_string());
            while self.history.len() > MAX_HISTORY_ENTRIES {
                self.history.pop_back();
            }
        }
        self.cursor = None;
        self.stash.clear();
    }

    /// Walks one step backward (older) in this tab's prompt history (`#689` §3 rule 4).
    /// Stashes the current `draft` when entering history browsing from `cursor == None`.
    pub fn history_up(&mut self) -> bool {
        if self.history.is_empty() {
            return false;
        }
        match self.cursor {
            None => {
                self.stash = self.draft.clone();
                self.cursor = Some(0);
                self.draft = self.history[0].clone();
                true
            }
            Some(idx) => {
                let next = (idx + 1).min(self.history.len() - 1);
                self.cursor = Some(next);
                self.draft = self.history[next].clone();
                true
            }
        }
    }

    /// Walks one step forward (newer) in this tab's prompt history, restoring `stash` past index 0 (`#689` §3 rule 4).
    pub fn history_down(&mut self) -> bool {
        match self.cursor {
            None => false,
            Some(0) => {
                self.cursor = None;
                self.draft = std::mem::take(&mut self.stash);
                true
            }
            Some(idx) => {
                let prev = idx - 1;
                self.cursor = Some(prev);
                self.draft = self.history[prev].clone();
                true
            }
        }
    }

    /// Replaces `draft` and exits history browsing mode.
    pub fn set_draft(&mut self, text: impl Into<String>) {
        self.draft = text.into();
        self.cursor = None;
    }

    /// Handles text pasted into the composer (`#689` §3 rule 6):
    /// - If `pasted` is serialized canvas layers (`clipboard::from_json`), adds [`LayerChip`]s in `#655`
    ///   format (`designos:node/<id>`) and inserts only the plain-text layer names into `draft` (never raw JSON).
    /// - Otherwise appends `pasted` to `draft`.
    ///
    /// Returns `true` when `pasted` was recognized as canvas layers and converted into chips.
    pub fn paste_text(&mut self, pasted: &str) -> bool {
        if let Some(clip) = crate::clipboard::from_json(pasted) {
            let mut names = Vec::new();
            for root_id in &clip.roots {
                if let Some(node) = clip.nodes.iter().find(|n| n.id == *root_id) {
                    let name = if node.name.trim().is_empty() { format!("Layer {}", root_id.0) } else { node.name.trim().to_string() };
                    let is_component = matches!(node.kind, NodeKind::Component(_) | NodeKind::ComponentSet(_) | NodeKind::Instance { .. });
                    let reference = format!("designos:node/{}", root_id.0);
                    if !self.chips.iter().any(|c| c.node_id == *root_id) {
                        self.chips.push(LayerChip { node_id: *root_id, name: name.clone(), is_component, reference });
                    }
                    names.push(name);
                }
            }
            if !names.is_empty() {
                let joined = names.join(", ");
                if !self.draft.is_empty() && !self.draft.ends_with(char::is_whitespace) {
                    self.draft.push(' ');
                }
                self.draft.push_str(&joined);
                self.cursor = None;
                return true;
            }
        }
        self.draft.push_str(pasted);
        self.cursor = None;
        false
    }

    /// Restores an undelivered prompt back to `draft` and sets the refusal notice (`#689` §3 rules 2, 7a & §8.4).
    pub fn restore_undelivered(&mut self, text: String, notice: impl Into<String>, reason: RefuseReason) {
        if self.draft.trim().is_empty() {
            self.draft = text;
        }
        self.cursor = None;
        self.reset_send_receipt(notice, reason);
    }

    /// Drops an undelivered system prompt (image jobs): the notice and receipt are set, the user's draft is left alone.
    pub fn discard_undelivered_system(&mut self, notice: impl Into<String>, reason: RefuseReason) {
        self.reset_send_receipt(notice, reason);
    }

    fn reset_send_receipt(&mut self, notice: impl Into<String>, reason: RefuseReason) {
        self.pending_enter_deadline = None;
        self.pending_enter_delay = None;
        self.pending_enter_sim_time = None;
        self.awaiting_confirm_since = None;
        self.last_outcome = Some(SendOutcome::Refused(reason));
        self.notice = Some(notice.into());
    }
}

/// Computes the per-CLI delay between writing the bracketed paste and writing `\r` (`#689` §3 rule 1).
pub fn enter_delay_for_preset(preset: AgentCliPreset, paste_bytes_len: usize) -> Duration {
    match preset {
        AgentCliPreset::Claude => CLAUDE_ENTER_DELAY,
        _ => {
            let extra_ms = (paste_bytes_len / ENTER_DELAY_BYTES_PER_MS) as u64;
            OTHER_CLI_BASE_ENTER_DELAY + Duration::from_millis(extra_ms)
        }
    }
}

/// Sanitizes composer text before bracketed paste (`#689` §3 rule 10):
/// normalizes `\r\n`/`\r` to `\n`, replaces `ESC` (`\x1b`) with `U+241B` (`␛`), and strips other C0/DEL/C1 controls.
pub fn sanitize_composer_text(text: &str) -> String {
    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
    normalized
        .chars()
        .filter_map(|c| {
            if c == '\x1b' {
                Some('\u{241B}')
            } else if c == '\n' || c == '\t' {
                Some(c)
            } else if c.is_control() {
                None
            } else {
                Some(c)
            }
        })
        .collect()
}

/// Formats `text` as a bracketed paste (`\x1b[200~…\x1b[201~`) for the live agent PTY (`#689` §3 rule 1).
pub fn format_composer_paste_bytes(text: &str) -> Vec<u8> {
    let clean = sanitize_composer_text(text);
    let mut out = Vec::with_capacity(clean.len() + 12);
    out.extend_from_slice(b"\x1b[200~");
    out.extend_from_slice(clean.as_bytes());
    out.extend_from_slice(b"\x1b[201~");
    out
}

/// Number of visible lines (`1..=8`) for `text` inside the composer (`#689` §3 rule 3 & §7).
pub fn visible_line_count(text: &str) -> usize {
    let raw_lines = if text.is_empty() { 1 } else { text.split('\n').count() };
    raw_lines.clamp(MIN_COMPOSER_LINES, MAX_COMPOSER_LINES)
}

/// Height of the composer box for `text` and `has_chips` (`#689` §7 & §10 `field_grows_to_eight_lines_then_scrolls`).
/// Minimum height is `CONTROL_ROW` (40 px) plus the control row (`CONTROL_ROW`, 40 px), growing by `LINE_BODY`
/// per line from 1 to 8 lines and then capping while scrolling.
pub fn composer_box_height(text: &str, has_chips: bool) -> f32 {
    let lines = visible_line_count(text);
    let extra_lines = (lines.saturating_sub(MIN_COMPOSER_LINES)) as f32;
    let chips_h = if has_chips { theme::CONTROL_DENSE + theme::SPACE_ROW } else { ZERO };
    theme::CONTROL_ROW + theme::CONTROL_ROW + extra_lines * theme::LINE_BODY + chips_h
}

/// Total vertical space reserved at the bottom of `show_left` for the composer box and its margins/caption.
pub fn composer_reserved_height(state: &TerminalState) -> f32 {
    let comp = state.active_composer();
    let box_h = composer_box_height(&comp.draft, !comp.chips.is_empty());
    let notice_h = if comp.notice.is_some() { theme::LINE_CAPTION + theme::SPACE_FACT } else { ZERO };
    box_h + theme::SPACE_GROUP * 2.0 + notice_h
}

fn is_tab_ready_for_queued_send(tab: &mut TerminalTab) -> bool {
    if tab.is_shell_process() || tab.start_failure.is_some() || !tab.is_alive() {
        return false;
    }
    let explicit_status = tab.agent_status.lock().ok().map(|g| g.clone()).unwrap_or_default();
    if matches!(explicit_status, AgentTabStatus::WaitingForInput(_)) {
        return false;
    }
    let bracketed = tab.parser.lock().ok().map(|p| p.screen().bracketed_paste()).unwrap_or(false);
    let last_out_elapsed = tab.last_output_at.lock().ok().and_then(|g| *g).map(|t| t.elapsed());
    if bracketed && last_out_elapsed.is_some_and(|d| d >= READY_QUIET_BRACKETED) {
        return true;
    }
    if last_out_elapsed.is_some_and(|d| d >= READY_QUIET_OUTPUT) {
        return true;
    }
    tab.spawned_at.elapsed() >= crate::agent_start_failure::START_FAILURE_WINDOW
}

/// Writes `prompt` into the live agent tab. A `system` prompt (image jobs) is never restored into the user's draft.
fn write_prompt_to_live_tab(tab: &mut TerminalTab, prompt: &str, status_msg: &mut Option<String>, system: bool) -> SendOutcome {
    let bracketed = tab.parser.lock().ok().map(|p| p.screen().bracketed_paste()).unwrap_or(true);
    let payload_text = if !bracketed && prompt.contains('\n') {
        if !tab.composer.unbracketed_newline_warned {
            tab.composer.unbracketed_newline_warned = true;
            *status_msg = Some("Sent multiline prompt as one line (CLI has bracketed paste off)".to_string());
        }
        prompt.replace('\n', " ")
    } else {
        prompt.to_string()
    };
    let paste_bytes = format_composer_paste_bytes(&payload_text);
    if tab.try_write_bytes(&paste_bytes).is_err() {
        if system {
            tab.composer.discard_undelivered_system(RefuseReason::NotWritable.message(), RefuseReason::NotWritable);
        } else {
            tab.composer.restore_undelivered(prompt.to_string(), RefuseReason::NotWritable.message(), RefuseReason::NotWritable);
        }
        *status_msg = tab.composer.notice.clone();
        return SendOutcome::Refused(RefuseReason::NotWritable);
    }
    let delay = enter_delay_for_preset(tab.spawn_preset, paste_bytes.len());
    let now = Instant::now();
    tab.composer.pending_enter_deadline = Some(now + delay);
    tab.composer.pending_enter_delay = Some(delay);
    tab.composer.awaiting_confirm_since = Some(now);
    tab.composer.log_len_at_send = tab.raw_log.lock().ok().map(|l| l.len()).unwrap_or(0);
    if !system && tab.spawned_at.elapsed() <= crate::agent_start_failure::START_FAILURE_WINDOW {
        tab.composer.sent_in_start_window = Some(prompt.to_string());
    }
    tab.composer.last_outcome = Some(SendOutcome::Written);
    tab.composer.notice = None;
    *status_msg = Some(format!("Sent to {}", tab.title));
    SendOutcome::Written
}

/// Who is sending: the user's composer (consumes draft, chips, mentions, history) or a system sender (touches none of them).
#[derive(Clone, Copy, PartialEq, Eq)]
enum SendSource {
    Composer,
    System,
}

impl TerminalState {
    /// Returns a shared reference to the active tab's [`ComposerState`] (or `unattached_composer` when no tab is open).
    pub fn active_composer(&self) -> &ComposerState {
        self.active_tab().map(|t| &t.composer).unwrap_or(&self.unattached_composer)
    }

    /// Returns a mutable reference to the active tab's [`ComposerState`] (or `unattached_composer` when no tab is open).
    pub fn active_composer_mut(&mut self) -> &mut ComposerState {
        let idx = self.active_tab.min(self.tabs.len().saturating_sub(1));
        if let Some(tab) = self.tabs.get_mut(idx) {
            &mut tab.composer
        } else {
            &mut self.unattached_composer
        }
    }

    /// Current draft text of the active tab's composer (or the unattached composer when no tab is open).
    pub fn composer_draft(&self) -> &str {
        &self.active_composer().draft
    }

    /// Sets the draft text of the active tab's composer (or the unattached composer when no tab is open).
    pub fn set_composer_draft(&mut self, text: impl Into<String>) {
        self.active_composer_mut().set_draft(text);
    }

    /// Pastes `pasted` into the active tab's composer, turning copied canvas layers (`clipboard::from_json`)
    /// into [`LayerChip`]s and plain layer names (`#689` §3 rule 6).
    pub fn paste_into_composer(&mut self, pasted: &str) -> bool {
        self.active_composer_mut().paste_text(pasted)
    }

    /// Flushes any due delayed `\r` writes across tabs immediately.
    pub fn flush_pending_enter_now(&mut self) {
        for tab in &mut self.tabs {
            if tab.composer.pending_enter_deadline.take().is_some() {
                tab.composer.pending_enter_delay = None;
                tab.composer.pending_enter_sim_time = None;
                if tab.accepts_agent_input() {
                    let _ = tab.try_write_bytes(b"\r");
                }
            }
        }
    }

    /// Advances per-tab composer state on every bridge poll / frame:
    /// 1. Restores any queued or start-window prompt to `draft` if the tab failed to start or exited while queued.
    /// 2. Flushes `pending_send` once a newly started tab becomes ready.
    /// 3. Writes the delayed `\r` once `pending_enter_deadline` arrives.
    /// 4. Marks a write as [`SendOutcome::Unconfirmed`] after 10 seconds without a turn-start hook or new PTY output.
    pub fn tick_composer(&mut self) {
        let now = Instant::now();
        for tab in &mut self.tabs {
            // 1. Failed start or early exit recovery (`#689` §3 rules 2, 7a & §8 edge cases 4, 5).
            if tab.start_failure.is_some() {
                if let Some(undelivered) = tab.composer.pending_send.take().or_else(|| tab.composer.sent_in_start_window.take()) {
                    if std::mem::take(&mut tab.composer.pending_send_is_system) {
                        tab.composer.discard_undelivered_system("Not delivered: check the terminal", RefuseReason::StartFailed);
                    } else {
                        tab.composer.restore_undelivered(undelivered, "Not delivered: check the terminal", RefuseReason::StartFailed);
                    }
                    self.status_msg = tab.composer.notice.clone();
                }
                continue;
            }
            if !tab.is_shell_process() && !tab.is_alive() {
                if let Some(undelivered) = tab.composer.pending_send.take() {
                    if std::mem::take(&mut tab.composer.pending_send_is_system) {
                        tab.composer.discard_undelivered_system("Not delivered: check the terminal", RefuseReason::StartFailed);
                    } else {
                        tab.composer.restore_undelivered(undelivered, "Not delivered: check the terminal", RefuseReason::StartFailed);
                    }
                    self.status_msg = tab.composer.notice.clone();
                }
                continue;
            }
            if tab.spawned_at.elapsed() > crate::agent_start_failure::START_FAILURE_WINDOW {
                tab.composer.sent_in_start_window = None;
            }

            // 2. Flush queued `pending_send` once the tab is ready (`#689` §3 rule 2).
            if tab.composer.pending_send.is_some() && is_tab_ready_for_queued_send(tab) {
                if let Some(queued) = tab.composer.pending_send.take() {
                    let system = std::mem::take(&mut tab.composer.pending_send_is_system);
                    let _ = write_prompt_to_live_tab(tab, &queued, &mut self.status_msg, system);
                }
            }

            // 3. Delayed `\r` write (`#689` §3 rule 1 & §11).
            if let Some(deadline) = tab.composer.pending_enter_deadline {
                if now >= deadline {
                    tab.composer.pending_enter_deadline = None;
                    tab.composer.pending_enter_delay = None;
                    tab.composer.pending_enter_sim_time = None;
                    if tab.accepts_agent_input() {
                        let _ = tab.try_write_bytes(b"\r");
                    }
                }
            }

            // 4. Receipt confirmation vs 10s `Unconfirmed` notice (`#689` §3 rule 7a).
            if let Some(sent_at) = tab.composer.awaiting_confirm_since {
                let cur_log_len = tab.raw_log.lock().ok().map(|l| l.len()).unwrap_or(0);
                let status_confirmed = tab.agent_status.lock().ok().is_some_and(|st| matches!(*st, AgentTabStatus::Working(_) | AgentTabStatus::Done));
                if cur_log_len > tab.composer.log_len_at_send || status_confirmed {
                    tab.composer.awaiting_confirm_since = None;
                } else if now.saturating_duration_since(sent_at) >= UNCONFIRMED_TIMEOUT {
                    tab.composer.awaiting_confirm_since = None;
                    tab.composer.last_outcome = Some(SendOutcome::Unconfirmed);
                    let msg = "Sent, not confirmed: check the terminal".to_string();
                    tab.composer.notice = Some(msg.clone());
                    self.status_msg = Some(msg);
                }
            }
        }
    }

    /// Sends `text` into the active tab's live agent CLI session (`#689` §3 & §5).
    /// All entry points (composer Enter/Send, sample prompts, `"Make design"`, `dispatch_figma_ai_prompt`)
    /// share this single path (`#689` §3 rule 8).
    pub fn send_to_agent(&mut self, text: &str, file_path: Option<&Path>) -> SendOutcome {
        self.send_to_agent_from(text, file_path, SendSource::Composer)
    }

    /// Sends a system-generated prompt (image jobs) into the active agent tab. Unlike [`Self::send_to_agent`] it
    /// leaves the user's draft, layer chips, `@`-mentions and Up-arrow history alone, attaches none of their
    /// references, never starts a session by itself, and never returns an undelivered prompt to the draft.
    /// A tab that is still starting queues the prompt until it is ready.
    pub fn send_system_prompt(&mut self, text: &str, file_path: Option<&Path>) -> SendOutcome {
        self.send_to_agent_from(text, file_path, SendSource::System)
    }

    fn send_to_agent_from(&mut self, text: &str, file_path: Option<&Path>, source: SendSource) -> SendOutcome {
        // Poll first so any child that already exited within `START_FAILURE_WINDOW` has `start_failure` populated.
        let _ = self.poll_live_agent_bridge();
        let system = source == SendSource::System;

        let trimmed = text.trim();
        let chips_snapshot = if system { Vec::new() } else { self.active_composer().chips.clone() };
        if trimmed.is_empty() && chips_snapshot.is_empty() {
            let comp = self.active_composer_mut();
            comp.last_outcome = Some(SendOutcome::Refused(RefuseReason::Empty));
            return SendOutcome::Refused(RefuseReason::Empty);
        }
        if text.chars().count() > MAX_COMPOSER_CHARS {
            let msg = RefuseReason::TooLong.message().to_string();
            let comp = self.active_composer_mut();
            comp.last_outcome = Some(SendOutcome::Refused(RefuseReason::TooLong));
            comp.notice = Some(msg.clone());
            self.status_msg = Some(msg);
            return SendOutcome::Refused(RefuseReason::TooLong);
        }

        // Format payload including any `#655` `@`-mention references and layer chip references.
        let mut live_mentions = if system { Vec::new() } else { self.active_mentions.clone() };
        crate::agent_mention::prune_deleted_mentions(trimmed, &mut live_mentions);
        let mut payload = if system {
            trimmed.to_string()
        } else if trimmed.is_empty() {
            chips_snapshot.iter().map(|c| c.name.as_str()).collect::<Vec<_>>().join(", ")
        } else {
            crate::agent_mention::build_prompt_with_mentions(trimmed, &live_mentions)
        };
        if !chips_snapshot.is_empty() {
            let refs: Vec<String> =
                chips_snapshot.iter().map(|c| format!("@{} = {} ({})", c.name, c.reference, if c.is_component { "component" } else { "layer" })).collect();
            let footer = format!("References: {}", refs.join(", "));
            if !payload.contains(&footer) {
                payload = format!("{payload}\n{footer}");
            }
        }

        // Check active tab gates when a tab is already open (`#689` §3 rule 7 & §8 edge cases 5, 9).
        let active_idx = self.active_tab.min(self.tabs.len().saturating_sub(1));
        if let Some(tab) = self.tabs.get_mut(active_idx) {
            if let Some(failure) = &tab.start_failure {
                let headline = failure.headline();
                tab.composer.last_outcome = Some(SendOutcome::Refused(RefuseReason::StartFailed));
                tab.composer.notice = Some(headline.clone());
                self.status_msg = Some(headline);
                return SendOutcome::Refused(RefuseReason::StartFailed);
            }
            if tab.is_shell_process() {
                let msg = RefuseReason::NoAgent.message().to_string();
                tab.composer.last_outcome = Some(SendOutcome::Refused(RefuseReason::NoAgent));
                tab.composer.notice = Some(msg.clone());
                self.status_msg = Some(msg);
                return SendOutcome::Refused(RefuseReason::NoAgent);
            }
            if tab.composer.not_ready_gate {
                let msg = RefuseReason::NotReady.message().to_string();
                tab.composer.last_outcome = Some(SendOutcome::Refused(RefuseReason::NotReady));
                tab.composer.notice = Some(msg.clone());
                self.status_msg = Some(msg);
                return SendOutcome::Refused(RefuseReason::NotReady);
            }
            let waiting_permission = tab.agent_status.lock().ok().is_some_and(|st| matches!(*st, AgentTabStatus::WaitingForInput(_)));
            if waiting_permission {
                let msg = RefuseReason::Permission.message().to_string();
                tab.composer.last_outcome = Some(SendOutcome::Refused(RefuseReason::Permission));
                tab.composer.notice = Some(msg.clone());
                self.status_msg = Some(msg);
                return SendOutcome::Refused(RefuseReason::Permission);
            }
            if tab.is_alive() {
                if system {
                    if is_tab_ready_for_queued_send(tab) {
                        return write_prompt_to_live_tab(tab, &payload, &mut self.status_msg, true);
                    }
                    if tab.composer.pending_send.is_some() {
                        tab.composer.last_outcome = Some(SendOutcome::Refused(RefuseReason::NotReady));
                        return SendOutcome::Refused(RefuseReason::NotReady);
                    }
                    tab.composer.pending_send = Some(payload);
                    tab.composer.pending_send_is_system = true;
                    tab.composer.last_outcome = Some(SendOutcome::Queued);
                    return SendOutcome::Queued;
                }
                tab.composer.send_count += 1;
                tab.composer.record_sent(trimmed);
                tab.composer.draft.clear();
                tab.composer.chips.clear();
                self.active_mentions.clear();
                self.mention_popup.close();
                return write_prompt_to_live_tab(tab, &payload, &mut self.status_msg, false);
            }
        }

        if system {
            // A system sender picks or starts its own agent tab first; with none alive there is nothing to write to.
            let msg = RefuseReason::NoAgent.message().to_string();
            self.status_msg = Some(msg);
            return SendOutcome::Refused(RefuseReason::NoAgent);
        }

        // Not running (`#689` §3 rule 2): record in history, clear draft, run `open_for_agent_use`, and queue until ready.
        {
            let comp = self.active_composer_mut();
            comp.send_count += 1;
            comp.record_sent(trimmed);
            comp.draft.clear();
            comp.chips.clear();
        }
        self.active_mentions.clear();
        self.mention_popup.close();
        let tabs_before = self.tabs.len();
        self.open_for_agent_use(file_path);

        // If `open_for_agent_use` opened the picker because no previous session existed, but `active_runtime`
        // was explicitly set to a registrable agent CLI that is installed on PATH, spawn it now so the queued
        // prompt starts that agent.
        let runtime_installed = self.active_runtime.is_installed() || self.path_prepend.iter().any(|d| d.join(self.active_runtime.binary_name()).is_file());
        if self.tabs.len() == tabs_before && crate::agent_sessions::is_registrable_cli(self.active_runtime) && runtime_installed {
            let _ = self.spawn_tab(self.active_runtime, file_path);
        }

        let agent_name = self.active_tab().map(|t| t.spawn_preset.dropdown_entry().1).unwrap_or_else(|| {
            if crate::agent_sessions::is_registrable_cli(self.active_runtime) {
                self.active_runtime.dropdown_entry().1
            } else {
                "agent"
            }
        });
        let notice = format!("Starting {agent_name}… your message will be sent");
        let comp = self.active_composer_mut();
        comp.pending_send = Some(payload);
        comp.pending_send_is_system = false;
        comp.last_outcome = Some(SendOutcome::Queued);
        comp.notice = Some(notice.clone());
        self.status_msg = Some(notice);
        SendOutcome::Queued
    }
}

fn composer_popup_open(state: &TerminalState) -> bool {
    state.mention_popup.is_open() && state.mention_popup.mode == crate::agent_mention_popup::MentionPopupMode::Composer
}

/// Runs `f` with the active composer draft, the `@`-mention popup and the active mentions as disjoint borrows.
fn with_composer_draft<R>(
    state: &mut TerminalState,
    f: impl FnOnce(&mut String, &mut crate::agent_mention_popup::MentionPopupState, &mut Vec<crate::agent_mention::Mention>) -> R,
) -> R {
    let mut draft = std::mem::take(&mut state.active_composer_mut().draft);
    let result = f(&mut draft, &mut state.mention_popup, &mut state.active_mentions);
    state.active_composer_mut().draft = draft;
    result
}

/// Byte offset of the caret inside the composer field: the live `TextEdit` state, else the last observed offset, else the end.
fn composer_cursor_byte(ui: &Ui, input_id: Id, draft: &str, last_seen: Option<usize>) -> usize {
    egui::widgets::text_edit::TextEditState::load(ui.ctx(), input_id)
        .and_then(|s| s.cursor.char_range())
        .map(|r| draft.char_indices().nth(usize::from(r.primary.index)).map(|(b, _)| b).unwrap_or(draft.len()))
        .or(last_seen)
        .unwrap_or(draft.len())
        .min(draft.len())
}

/// Moves the caret behind a freshly picked `@`-mention.
fn apply_post_pick_cursor(ui: &Ui, state: &mut TerminalState, input_id: Id) {
    let Some(new_byte) = state.mention_popup.post_pick_cursor_byte.take() else {
        return;
    };
    state.composer_cursor_byte = Some(new_byte);
    let draft = state.composer_draft();
    let char_idx = draft.get(..new_byte).map(|s| s.chars().count()).unwrap_or_else(|| draft.chars().count());
    if let Some(mut text_state) = egui::widgets::text_edit::TextEditState::load(ui.ctx(), input_id) {
        text_state.cursor.set_char_range(Some(egui::text::CCursorRange::one(egui::text::CCursor::new(char_idx))));
        text_state.store(ui.ctx(), input_id);
    }
}

/// Renders the multi-line Agent composer box at the bottom of `panel_rect` (`#689` §7)
/// and returns the outer `composer_rect` so the center viewport/content area can bound above it.
/// `scope` is the open document and page the `@`-mention picker (`#655`) searches.
pub fn show_composer(
    state: &mut TerminalState,
    ui: &mut Ui,
    panel_rect: Rect,
    min_top_y: f32,
    file_path: Option<&Path>,
    scope: MentionScope<'_>,
    actions: &mut Vec<TerminalAction>,
) -> Rect {
    let MentionScope { doc, page: current_page } = scope;
    let sim_now = ui.input(|i| i.time);
    // Advance delayed `\r` writes when either real wall-clock or `Sim` frame time has elapsed by `pending_enter_delay`.
    for tab in &mut state.tabs {
        if let Some(deadline) = tab.composer.pending_enter_deadline {
            let sim_elapsed =
                tab.composer.pending_enter_sim_time.zip(tab.composer.pending_enter_delay).is_some_and(|(t0, d)| sim_now + 1e-6 >= t0 + d.as_secs_f64());
            if Instant::now() >= deadline || sim_elapsed {
                tab.composer.pending_enter_deadline = None;
                tab.composer.pending_enter_delay = None;
                tab.composer.pending_enter_sim_time = None;
                if tab.accepts_agent_input() {
                    let _ = tab.try_write_bytes(b"\r");
                }
            }
        }
    }
    state.tick_composer();

    let p = ui.painter().clone();
    let is_shell_tab = state.active_tab().is_some_and(|t| t.is_shell_process());
    let has_chips = !state.active_composer().chips.is_empty();
    let draft_snapshot = state.composer_draft().to_string();
    let box_h = composer_box_height(&draft_snapshot, has_chips);
    let notice_text = if is_shell_tab { Some(RefuseReason::NoAgent.message().to_string()) } else { state.active_composer().notice.clone() };
    let notice_h = if notice_text.is_some() { theme::LINE_CAPTION + theme::SPACE_FACT } else { ZERO };
    let margin = theme::SPACE_GROUP;
    let bottom_pad = margin + notice_h;
    let comp_top = (panel_rect.bottom() - bottom_pad - box_h).max(min_top_y + MIN_CENTER_HEIGHT);
    let comp_bottom = (comp_top + box_h).min(panel_rect.bottom() - theme::SPACE_FACT);
    let composer_rect = Rect::from_min_max(Pos2::new(panel_rect.left() + margin, comp_top), Pos2::new(panel_rect.right() - margin, comp_bottom));

    let input_id = Id::new("agents-ask-for-changes-input");
    let was_focused = ui.memory(|m| m.has_focus(input_id));

    // `@`-mention picker keys (`#655`) are consumed before the composer reads Enter/Up/Down so a pick never also submits.
    if !is_shell_tab {
        let draft_now = state.composer_draft().to_string();
        let pre_cursor = composer_cursor_byte(ui, input_id, &draft_now, state.composer_cursor_byte);
        state.mention_popup.sync_from_composer_text(&draft_now, pre_cursor, doc, current_page);
        if composer_popup_open(state) {
            let ctx = ui.ctx().clone();
            let _ = with_composer_draft(state, |draft, popup, mentions| popup.handle_keyboard(&ctx, draft, mentions, doc, current_page));
            apply_post_pick_cursor(ui, state, input_id);
        }
    }

    let mut submit_composer = false;
    if was_focused && !is_shell_tab {
        state.viewport_focused = false;
        let can_start_history = state.active_composer().draft.trim().is_empty() || state.active_composer().cursor.is_some();
        let browsing_history = state.active_composer().cursor.is_some();
        let mut walk_up = false;
        let mut walk_down = false;
        let mut pasted_payloads: Vec<String> = Vec::new();

        ui.input_mut(|i| {
            let mut next_events = Vec::with_capacity(i.events.len());
            for ev in i.events.drain(..) {
                match ev {
                    Event::Paste(ref text) if crate::clipboard::from_json(text).is_some() => {
                        pasted_payloads.push(text.clone());
                    }
                    Event::Key { key: Key::Enter, pressed: true, modifiers, physical_key, repeat } => {
                        if modifiers.shift {
                            next_events.push(Event::Key { key: Key::Enter, physical_key, pressed: true, repeat, modifiers: egui::Modifiers::NONE });
                        } else if !modifiers.command && !modifiers.ctrl && !modifiers.alt {
                            submit_composer = true;
                        } else {
                            next_events.push(ev);
                        }
                    }
                    Event::Text(ref t) if submit_composer && (t == "\n" || t == "\r") => {}
                    Event::Key { key: Key::ArrowUp, pressed: true, modifiers, .. }
                        if can_start_history && !modifiers.shift && !modifiers.command && !modifiers.ctrl && !modifiers.alt =>
                    {
                        walk_up = true;
                    }
                    Event::Key { key: Key::ArrowDown, pressed: true, modifiers, .. }
                        if browsing_history && !modifiers.shift && !modifiers.command && !modifiers.ctrl && !modifiers.alt =>
                    {
                        walk_down = true;
                    }
                    other => next_events.push(other),
                }
            }
            i.events = next_events;
        });

        for payload in pasted_payloads {
            state.paste_into_composer(&payload);
        }
        if walk_up {
            state.active_composer_mut().history_up();
        }
        if walk_down {
            state.active_composer_mut().history_down();
        }
    }

    // Composer container (`RADIUS_FRAME`, `BG_SECONDARY`, `BORDER` hairline, `BG_BRAND` focus ring — `#689` §7).
    p.rect_filled(composer_rect, theme::cr(theme::RADIUS_FRAME), theme::BG_SECONDARY);
    let stroke = if was_focused && !is_shell_tab { Stroke::new(FOCUS_RING_WIDTH, theme::BG_BRAND) } else { Stroke::new(theme::BORDER_WIDTH, theme::BORDER) };
    p.rect_stroke(composer_rect, theme::cr(theme::RADIUS_FRAME), stroke, StrokeKind::Inside);
    widgets::record(ui, "agents-composer-box", composer_rect);

    let mut content_top = composer_rect.top() + theme::INSET_CONTROL;

    // Optional reference chip row (`CONTROL_DENSE` 24 px, `cr_pill`, `BG_TERTIARY`, `theme::COMPONENT` cyan for components).
    let chips_snapshot = state.active_composer().chips.clone();
    let mut chip_to_remove: Option<NodeId> = None;
    if !chips_snapshot.is_empty() {
        let mut chip_x = composer_rect.left() + theme::INSET_BUTTON;
        let max_chip_right = composer_rect.right() - theme::INSET_BUTTON;
        for chip in &chips_snapshot {
            let text_w = widgets::text_width(&p, &chip.name, &theme::font_caption());
            let pill_w = (text_w + CHIP_TEXT_PAD).min(CHIP_MAX_WIDTH).min((max_chip_right - chip_x).max(theme::CONTROL_ROW));
            if chip_x + pill_w > max_chip_right {
                break;
            }
            let chip_rect = Rect::from_min_size(Pos2::new(chip_x, content_top), Vec2::new(pill_w, theme::CONTROL_DENSE));
            let chip_resp = ui.interact(chip_rect, Id::new(("agents-composer-chip", chip.node_id.0)), Sense::click());
            p.rect_filled(chip_rect, theme::cr_pill(theme::CONTROL_DENSE), theme::BG_TERTIARY);
            let fg = if chip.is_component { theme::COMPONENT } else { theme::TEXT };
            if chip.is_component {
                p.rect_stroke(chip_rect, theme::cr_pill(theme::CONTROL_DENSE), Stroke::new(theme::BORDER_WIDTH, theme::COMPONENT), StrokeKind::Inside);
            }
            let max_label_w = (pill_w - theme::INSET_BUTTON * 2.0).max(theme::SPACE_BLOCK);
            let elided = widgets::elide(&p, &chip.name, &theme::font_caption(), max_label_w);
            p.text(Pos2::new(chip_rect.left() + theme::INSET_CONTROL, chip_rect.center().y), Align2::LEFT_CENTER, elided, theme::font_caption(), fg);
            widgets::record(ui, "agents-composer-chip", chip_rect);
            widgets::record(ui, &format!("agents-composer-chip-{}", chip.node_id.0), chip_rect);
            if chip_resp.clicked() {
                chip_to_remove = Some(chip.node_id);
            }
            chip_x = chip_rect.right() + theme::SPACE_FACT;
        }
        if let Some(rm_id) = chip_to_remove {
            state.active_composer_mut().chips.retain(|c| c.node_id != rm_id);
        }
        content_top += theme::CONTROL_DENSE + theme::SPACE_ROW;
    }

    let control_row_top = composer_rect.bottom() - theme::CONTROL_ROW;
    let input_rect = Rect::from_min_max(
        Pos2::new(composer_rect.left() + theme::INSET_BUTTON, content_top),
        Pos2::new(composer_rect.right() - theme::INSET_BUTTON, control_row_top.max(content_top + theme::LINE_BODY)),
    );
    widgets::record(ui, "agents-composer-input", input_rect);

    let hint = if is_shell_tab { "Select an agent tab" } else { "Ask for changes" };
    let prev_draft = state.composer_draft().to_string();
    ui.scope_builder(egui::UiBuilder::new().max_rect(input_rect), |ui| {
        egui::ScrollArea::vertical().id_salt("agents-composer-scroll").max_height(input_rect.height()).show(ui, |ui| {
            let comp = state.active_composer_mut();
            let te = TextEdit::multiline(&mut comp.draft)
                .id(input_id)
                .desired_width(input_rect.width())
                .desired_rows(1)
                .frame(egui::Frame::NONE)
                .font(theme::font_body())
                .text_color(if is_shell_tab { theme::TEXT_DISABLED } else { theme::TEXT })
                .interactive(!is_shell_tab)
                .hint_text(RichText::new(hint).font(theme::font_body()).color(theme::TEXT_TERTIARY));
            let out = te.show(ui);
            if let Some(cr) = out.cursor_range {
                let draft = &state.active_composer().draft;
                state.composer_cursor_byte = Some(draft.char_indices().nth(usize::from(cr.primary.index)).map(|(b, _)| b).unwrap_or(draft.len()));
            }
            if out.response.clicked() || out.response.has_focus() {
                state.viewport_focused = false;
            }
        });
    });
    if !is_shell_tab {
        let draft_after = state.composer_draft().to_string();
        let post_cursor = state.composer_cursor_byte.unwrap_or(draft_after.len()).min(draft_after.len());
        state.mention_popup.sync_from_composer_text(&draft_after, post_cursor, doc, current_page);
        crate::agent_mention::prune_deleted_mentions(&draft_after, &mut state.active_mentions);
        if composer_popup_open(state) {
            let _ = with_composer_draft(state, |draft, popup, mentions| popup.show(ui, panel_rect, composer_rect, draft, mentions));
            apply_post_pick_cursor(ui, state, input_id);
        }
    }
    if state.composer_draft() != prev_draft {
        // User typed directly into the field; leave history cursor mode.
        let comp = state.active_composer_mut();
        if comp.cursor.is_some() && comp.history.get(comp.cursor.unwrap_or(0)).map(String::as_str) != Some(comp.draft.as_str()) {
            comp.cursor = None;
        }
    }

    let icon_y = composer_rect.bottom() - theme::CONTROL_ROW * HALF;

    // Bottom-left `+` attach button (`agents-composer-plus` / `agents-composer-attach`).
    let plus_center = Pos2::new(composer_rect.left() + theme::INSET_PANEL + theme::SPACE_FACT, icon_y);
    let plus_r = Rect::from_center_size(plus_center, Vec2::splat(theme::CONTROL_DENSE));
    let plus_resp = ui.interact(plus_r, Id::new("agents-composer-plus"), Sense::click());
    if plus_resp.hovered() {
        p.rect_filled(plus_r, theme::cr(theme::RADIUS_SEGMENT), theme::BG_HOVER);
    }
    icons::paint(&p, Icon::Plus, plus_r.center(), theme::LINE_CAPTION, theme::ICON_SECONDARY);
    widgets::record(ui, "agents-composer-plus", plus_r);
    widgets::record(ui, "agents-composer-attach", plus_r);
    if plus_resp.clicked() {
        let cur = state.composer_draft().trim().to_string();
        if !cur.contains("@selection") {
            let next = if cur.is_empty() { "@selection @prd ".to_string() } else { format!("@selection @prd {cur}") };
            state.set_composer_draft(next);
        }
        actions.push(TerminalAction::ExportSelectionToTerminal);
    }

    // Bottom-left open-book button (`agents-composer-book`).
    let book_center = Pos2::new(plus_r.right() + theme::SPACE_FACT + theme::CONTROL_DENSE * HALF, icon_y);
    let book_r = Rect::from_center_size(book_center, Vec2::splat(theme::CONTROL_DENSE));
    let book_resp = ui.interact(book_r, Id::new("agents-composer-book"), Sense::click());
    if book_resp.hovered() {
        p.rect_filled(book_r, theme::cr(theme::RADIUS_SEGMENT), theme::BG_HOVER);
    }
    icons::paint(&p, Icon::BookOpen, book_r.center(), theme::LINE_CAPTION, theme::ICON_SECONDARY);
    widgets::record(ui, "agents-composer-book", book_r);
    if book_resp.clicked() {
        state.side_panel_pty_open = !state.side_panel_pty_open;
    }

    // Bottom-left `PRD` context chip button (`agents-composer-prd` / `agents-composer-context`).
    let prd_center = Pos2::new(book_r.right() + theme::SPACE_INLINE + PRD_CHIP_WIDTH * HALF, icon_y);
    let prd_r = Rect::from_center_size(prd_center, Vec2::new(PRD_CHIP_WIDTH, theme::CONTROL_DENSE));
    let prd_resp = ui.interact(prd_r, Id::new("agents-composer-prd"), Sense::click());
    let lower_in = state.composer_draft().to_ascii_lowercase();
    let has_prd_tag = lower_in.contains("@prd") || lower_in.contains("@selection") || lower_in.contains("@tokens");
    let prd_bg = if has_prd_tag {
        theme::BG_SELECTED
    } else if prd_resp.hovered() {
        theme::BG_HOVER
    } else {
        theme::BG_TERTIARY
    };
    p.rect_filled(prd_r, theme::cr(theme::RADIUS_CONTROL), prd_bg);
    p.rect_stroke(
        prd_r,
        theme::cr(theme::RADIUS_CONTROL),
        Stroke::new(theme::BORDER_WIDTH, if has_prd_tag { theme::BORDER_SELECTED } else { theme::BORDER }),
        StrokeKind::Inside,
    );
    p.text(prd_r.center(), Align2::CENTER_CENTER, "PRD", theme::font_caption_strong(), if has_prd_tag { theme::TEXT_BRAND } else { theme::TEXT_SECONDARY });
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
        let cur = state.composer_draft().trim().to_string();
        if cur.is_empty() {
            state.set_composer_draft(format!("{next_tag} "));
        } else if !lower_in.contains(next_tag) {
            state.set_composer_draft(format!("{next_tag} {cur}"));
        }
    }

    // Bottom-right Send circle button (`CONTROL_STANDARD` 28 px, `BG_BRAND` with dark arrow; disabled and `TEXT_SECONDARY` when empty — `#689` §7).
    let has_sendable = !is_shell_tab && (!state.composer_draft().trim().is_empty() || !state.active_composer().chips.is_empty());
    let send_radius = theme::radius_full(theme::CONTROL_STANDARD);
    let send_center = Pos2::new(composer_rect.right() - theme::INSET_CONTROL - send_radius, icon_y);
    let send_r = Rect::from_center_size(send_center, Vec2::splat(theme::CONTROL_STANDARD));
    let send_resp = ui.interact(send_r, Id::new("agents-composer-send"), Sense::click());
    let send_bg = if has_sendable {
        if send_resp.hovered() {
            theme::BG_BRAND_HOVER
        } else {
            theme::BG_BRAND
        }
    } else {
        theme::BG_TERTIARY
    };
    let send_fg = if has_sendable { theme::TEXT_ON_LIGHT_CANVAS } else { theme::TEXT_SECONDARY };
    p.circle_filled(send_center, send_radius, send_bg);
    icons::paint(&p, Icon::ArrowUp, send_center, theme::LINE_CAPTION, send_fg);
    widgets::record(ui, "agents-composer-send", send_r);
    if send_resp.clicked() && has_sendable {
        submit_composer = true;
    }

    if submit_composer {
        let prompt = state.composer_draft().to_string();
        let outcome = state.send_to_agent(&prompt, file_path);
        if outcome == SendOutcome::Written {
            if let Some(tab) = state.active_tab_mut() {
                if tab.composer.pending_enter_deadline.is_some() {
                    tab.composer.pending_enter_sim_time = Some(sim_now);
                }
            }
        }
    }

    // Caption under the field for queued messages, refusal reasons, or unconfirmed receipts (`#689` §3 rules 7, 7a & §7).
    if let Some(note) = notice_text {
        let note_rect = Rect::from_min_size(
            Pos2::new(composer_rect.left() + theme::INSET_CONTROL, composer_rect.bottom() + theme::SPACE_FACT * HALF),
            Vec2::new(composer_rect.width() - theme::INSET_CONTROL * 2.0, theme::LINE_CAPTION),
        );
        let elided = widgets::elide(&p, &note, &theme::font_caption(), note_rect.width().max(theme::CONTROL_ROW));
        p.text(Pos2::new(note_rect.left(), note_rect.center().y), Align2::LEFT_CENTER, elided, theme::font_caption(), theme::TEXT_SECONDARY);
        widgets::record(ui, "agents-composer-notice", note_rect);
    }

    // Schedule repaint when any tab has a pending delayed `\r` or queued send (`#689` §3 rule 1 & §11).
    let now = Instant::now();
    let mut earliest_wait: Option<Duration> = None;
    for tab in &state.tabs {
        if let Some(deadline) = tab.composer.pending_enter_deadline {
            let rem = deadline.saturating_duration_since(now);
            earliest_wait = Some(earliest_wait.map_or(rem, |cur| cur.min(rem)));
        } else if tab.composer.pending_send.is_some() {
            let poll_wait = READY_QUIET_BRACKETED;
            earliest_wait = Some(earliest_wait.map_or(poll_wait, |cur| cur.min(poll_wait)));
        }
    }
    if let Some(wait) = earliest_wait {
        ui.ctx().request_repaint_after(wait);
    }

    composer_rect
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terminal::show_left;
    use designos_doc::Node;
    use std::io::Write;
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};

    struct SharedBufferWriter {
        buf: Arc<Mutex<Vec<u8>>>,
        _keep_pty_alive: Box<dyn Write + Send>,
    }

    impl Write for SharedBufferWriter {
        fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
            self.buf.lock().unwrap().extend_from_slice(data);
            Ok(data.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    fn attach_recording_writer(tab: &mut TerminalTab) -> Arc<Mutex<Vec<u8>>> {
        let buf = Arc::new(Mutex::new(Vec::new()));
        let prev = std::mem::replace(&mut tab.writer, Box::new(std::io::sink()));
        tab.writer = Box::new(SharedBufferWriter { buf: Arc::clone(&buf), _keep_pty_alive: prev });
        buf
    }

    fn temp_composer_dir(tag: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
        let dir = std::env::temp_dir().join(format!("dos-689-{tag}-{}-{nanos}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    fn write_executable_stub(bin_dir: &Path, name: &str, script_body: &str) -> PathBuf {
        std::fs::create_dir_all(bin_dir).expect("create bin dir");
        let path = bin_dir.join(name);
        let content = format!("#!/bin/sh\n{script_body}\n");
        std::fs::write(&path, content).expect("write stub script");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&path).expect("metadata").permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&path, perms).expect("chmod +x");
        }
        path
    }

    fn strip_cfg_test_module(src: &str) -> &str {
        src.find("#[cfg(test)]").map(|idx| &src[..idx]).unwrap_or(src)
    }

    // 1. `two_messages_in_a_row_reach_one_session`
    #[test]
    fn two_messages_in_a_row_reach_one_session() {
        let dir = temp_composer_dir("two-msgs-one-session");
        let bin_dir = dir.join("bin");
        let log_file = dir.join("session-recv.log");
        // Stub interactive CLI that turns on bracketed paste (?2004h), loops reading lines, and logs PID + line.
        let script = format!(
            "printf '\\033[?2004hReady> '\nwhile IFS= read -r line; do\n  printf 'PID=%s LINE=%s\\n' \"$$\" \"$line\" >> '{}'\n  printf 'ACK:%s\\n' \"$line\"\ndone",
            log_file.display()
        );
        write_executable_stub(&bin_dir, "claude", &script);

        let mut st = TerminalState::new_for_session("doc-689-1", "Session Continuity", Some(&dir));
        st.path_prepend.push(bin_dir);
        st.spawn_tab(AgentCliPreset::Claude, None).expect("spawn claude");
        assert_eq!(st.tabs.len(), 1);

        // Wait until the stub CLI prints `Ready>` and enables bracketed paste.
        let ready_deadline = Instant::now() + Duration::from_secs(4);
        while Instant::now() < ready_deadline {
            let _ = st.poll_live_agent_bridge();
            let ready = st.active_tab().is_some_and(|t| {
                t.parser.lock().ok().is_some_and(|p| p.screen().bracketed_paste()) && t.raw_log.lock().ok().is_some_and(|l| l.contains("Ready>"))
            });
            if ready {
                break;
            }
            std::thread::sleep(Duration::from_millis(25));
        }

        // Send message 1 and let the delayed `\r` fire through `tick_composer`.
        assert_eq!(st.send_to_agent("First message in session", None), SendOutcome::Written);
        std::thread::sleep(CLAUDE_ENTER_DELAY + Duration::from_millis(20));
        st.tick_composer();

        // Send message 2 into the same tab and let the delayed `\r` fire.
        assert_eq!(st.send_to_agent("Second message in session", None), SendOutcome::Written);
        std::thread::sleep(CLAUDE_ENTER_DELAY + Duration::from_millis(20));
        st.tick_composer();

        // Wait for both lines to be written by the single running process.
        let recv_deadline = Instant::now() + Duration::from_secs(4);
        let mut log_contents = String::new();
        while Instant::now() < recv_deadline {
            log_contents = std::fs::read_to_string(&log_file).unwrap_or_default();
            if log_contents.contains("First message in session") && log_contents.contains("Second message in session") {
                break;
            }
            std::thread::sleep(Duration::from_millis(30));
        }

        let raw_out = st.active_tab().and_then(|t| t.raw_log.lock().ok().map(|g| g.clone())).unwrap_or_default();
        assert_eq!(st.tabs.len(), 1, "Must still have one live session tab");
        assert!(log_contents.contains("First message in session"), "First message missing: log={log_contents:?}, raw_out={raw_out:?}");
        assert!(log_contents.contains("Second message in session"), "Second message missing: log={log_contents:?}, raw_out={raw_out:?}");

        let pids: Vec<&str> = log_contents.lines().filter_map(|l| l.strip_prefix("PID=").and_then(|r| r.split_whitespace().next())).collect();
        assert_eq!(pids.len(), 2, "Expected 2 logged lines in session-recv.log, got: {log_contents:?}");
        assert_eq!(pids[0], pids[1], "Both messages must reach the exact same CLI process PID");

        st.shutdown();
        let _ = std::fs::remove_dir_all(&dir);
    }

    // 2. `no_one_shot_call_remains_on_the_composer_path`
    #[test]
    fn no_one_shot_call_remains_on_the_composer_path() {
        let terminal_shipped = strip_cfg_test_module(include_str!("terminal.rs"));
        let composer_shipped = strip_cfg_test_module(include_str!("agent_composer.rs"));

        for retired in [
            "build_real_cli_command",
            "is_design_creation_prompt",
            "AgentChatTurn",
            "AgentChatRole",
            "chat_turns",
            "chat_scroll",
            "next_turn_id",
            "ensure_shell_for_chat",
            "sync_chat_from_pty",
            "wrap_chat_lines",
            "render_agent_chat_thread",
            "claude --dangerously-skip-permissions -p",
            "codex --dangerously-bypass-approvals-and-sandbox exec",
            "codex exec",
            "agy --dangerously-skip-permissions -p",
            "grok --always-approve -p",
        ] {
            assert!(!terminal_shipped.contains(retired), "Retired symbol/flag `{retired}` must not remain in shipped terminal.rs");
            assert!(!composer_shipped.contains(retired), "Retired symbol/flag `{retired}` must not appear in shipped agent_composer.rs");
        }
    }

    // 3. `enter_sends_shift_enter_adds_a_newline`
    #[test]
    fn enter_sends_shift_enter_adds_a_newline() {
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        crate::theme::apply(&ctx);

        let mut st = TerminalState::default();
        st.spawn_tab(AgentCliPreset::Shell, None).expect("spawn");
        {
            let tab = st.active_tab_mut().unwrap();
            tab.spawn_preset = AgentCliPreset::Claude;
            tab.preset = AgentCliPreset::Claude;
            if let Ok(mut p) = tab.parser.lock() {
                p.process(b"\x1b[?2004h");
            }
        }
        let rec = attach_recording_writer(st.active_tab_mut().unwrap());
        let panel_rect = Rect::from_min_size(Pos2::new(48.0, 0.0), Vec2::new(260.0, 720.0));
        let input_id = Id::new("agents-ask-for-changes-input");

        // Frame 1: focus the composer input and type "line one" + Shift+Enter + "line two".
        ctx.memory_mut(|m| m.request_focus(input_id));
        let raw_shift = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1200.0, 800.0))),
            time: Some(1.0),
            events: vec![
                Event::Text("line one".to_string()),
                Event::Key {
                    key: Key::Enter,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers { shift: true, ..Default::default() },
                },
                Event::Text("line two".to_string()),
            ],
            ..Default::default()
        };
        let _ = ctx.run_ui(raw_shift, |ui| {
            let mut acts = Vec::new();
            let _ = show_composer(&mut st, ui, panel_rect, 80.0, None, MentionScope { doc: &Document::new("Composer"), page: NodeId(1) }, &mut acts);
        });

        assert_eq!(st.composer_draft(), "line one\nline two", "Shift+Enter must insert a newline without sending");
        assert!(rec.lock().unwrap().is_empty(), "Nothing may be written to the PTY on Shift+Enter");

        // Frame 2: plain Enter submits the 2-line draft as bracketed paste.
        ctx.memory_mut(|m| m.request_focus(input_id));
        let raw_enter = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1200.0, 800.0))),
            time: Some(1.016),
            events: vec![Event::Key { key: Key::Enter, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::NONE }],
            ..Default::default()
        };
        let _ = ctx.run_ui(raw_enter, |ui| {
            let mut acts = Vec::new();
            let _ = show_composer(&mut st, ui, panel_rect, 80.0, None, MentionScope { doc: &Document::new("Composer"), page: NodeId(1) }, &mut acts);
        });
        assert_eq!(st.composer_draft(), "", "Plain Enter must clear the draft after sending");
        assert_eq!(&*rec.lock().unwrap(), b"\x1b[200~line one\nline two\x1b[201~", "Bracketed paste payload must be written first, before the delayed Enter");

        // Intermediate frame (+24ms < 50ms CLAUDE_ENTER_DELAY): `\r` must NOT flush early.
        let raw_early =
            egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1200.0, 800.0))), time: Some(1.040), ..Default::default() };
        let _ = ctx.run_ui(raw_early, |ui| {
            let mut acts = Vec::new();
            let _ = show_composer(&mut st, ui, panel_rect, 80.0, None, MentionScope { doc: &Document::new("Composer"), page: NodeId(1) }, &mut acts);
        });
        assert_eq!(&*rec.lock().unwrap(), b"\x1b[200~line one\nline two\x1b[201~", "Delayed \\r must not flush before CLAUDE_ENTER_DELAY (50ms) has elapsed");

        // Frame 3 (1.016 + CLAUDE_ENTER_DELAY = 1.066): delayed `\r` is flushed once the 50ms delay elapses.
        let raw_next = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1200.0, 800.0))),
            time: Some(1.016 + CLAUDE_ENTER_DELAY.as_secs_f64()),
            ..Default::default()
        };
        let _ = ctx.run_ui(raw_next, |ui| {
            let mut acts = Vec::new();
            let _ = show_composer(&mut st, ui, panel_rect, 80.0, None, MentionScope { doc: &Document::new("Composer"), page: NodeId(1) }, &mut acts);
        });
        assert_eq!(&*rec.lock().unwrap(), b"\x1b[200~line one\nline two\x1b[201~\r", "Delayed \\r must be written once CLAUDE_ENTER_DELAY elapses");

        // Also verify a non-Claude CLI (`Codex`, 500ms base delay) does not flush on an 83ms spinner frame.
        {
            let tab = st.active_tab_mut().unwrap();
            tab.spawn_preset = AgentCliPreset::Codex;
            tab.preset = AgentCliPreset::Codex;
        }
        rec.lock().unwrap().clear();
        st.set_composer_draft("codex line");
        ctx.memory_mut(|m| m.request_focus(input_id));
        let raw_codex_enter = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1200.0, 800.0))),
            time: Some(2.0),
            events: vec![Event::Key { key: Key::Enter, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::NONE }],
            ..Default::default()
        };
        let _ = ctx.run_ui(raw_codex_enter, |ui| {
            let mut acts = Vec::new();
            let _ = show_composer(&mut st, ui, panel_rect, 80.0, None, MentionScope { doc: &Document::new("Composer"), page: NodeId(1) }, &mut acts);
        });
        assert_eq!(&*rec.lock().unwrap(), b"\x1b[200~codex line\x1b[201~");

        // +83ms spinner frame (< 500ms OTHER_CLI_BASE_ENTER_DELAY): `\r` must NOT flush yet.
        let raw_spinner =
            egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1200.0, 800.0))), time: Some(2.083), ..Default::default() };
        let _ = ctx.run_ui(raw_spinner, |ui| {
            let mut acts = Vec::new();
            let _ = show_composer(&mut st, ui, panel_rect, 80.0, None, MentionScope { doc: &Document::new("Composer"), page: NodeId(1) }, &mut acts);
        });
        assert_eq!(
            &*rec.lock().unwrap(),
            b"\x1b[200~codex line\x1b[201~",
            "Non-Claude CLI must not flush \\r on an 83ms spinner frame before OTHER_CLI_BASE_ENTER_DELAY (500ms)"
        );

        // +500ms (2.0 + OTHER_CLI_BASE_ENTER_DELAY): `\r` flushes now.
        let raw_codex_due = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1200.0, 800.0))),
            time: Some(2.0 + OTHER_CLI_BASE_ENTER_DELAY.as_secs_f64()),
            ..Default::default()
        };
        let _ = ctx.run_ui(raw_codex_due, |ui| {
            let mut acts = Vec::new();
            let _ = show_composer(&mut st, ui, panel_rect, 80.0, None, MentionScope { doc: &Document::new("Composer"), page: NodeId(1) }, &mut acts);
        });
        assert_eq!(&*rec.lock().unwrap(), b"\x1b[200~codex line\x1b[201~\r");

        st.shutdown();
    }

    // 4. `field_grows_to_eight_lines_then_scrolls`
    #[test]
    fn field_grows_to_eight_lines_then_scrolls() {
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        crate::theme::apply(&ctx);
        let mut st = TerminalState::default();
        let panel_rect = Rect::from_min_size(Pos2::new(48.0, 0.0), Vec2::new(260.0, 720.0));

        let mut heights = Vec::new();
        for lines in 1..=12usize {
            let draft = (1..=lines).map(|i| format!("line {i}")).collect::<Vec<_>>().join("\n");
            st.set_composer_draft(draft);
            let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1200.0, 800.0))), ..Default::default() };
            let mut rect = Rect::NOTHING;
            let _ = ctx.run_ui(raw, |ui| {
                let mut acts = Vec::new();
                rect = show_composer(&mut st, ui, panel_rect, 80.0, None, MentionScope { doc: &Document::new("Composer"), page: NodeId(1) }, &mut acts);
            });
            heights.push((lines, rect.height()));
        }

        for w in heights[..8].windows(2) {
            let (l1, h1) = w[0];
            let (l2, h2) = w[1];
            assert!(h2 > h1, "Composer height must grow from {l1} lines ({h1}) to {l2} lines ({h2})");
        }
        let h8 = heights[7].1;
        for &(lines, h) in &heights[8..] {
            assert!((h - h8).abs() < 0.01, "Composer height at {lines} lines ({h}) must cap at 8 lines ({h8}) and scroll inside");
        }
    }

    // 5. `up_and_down_walk_history_and_restore_the_draft`
    #[test]
    fn up_and_down_walk_history_and_restore_the_draft() {
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        crate::theme::apply(&ctx);

        let mut st = TerminalState::default();
        st.spawn_tab(AgentCliPreset::Shell, None).expect("spawn");
        {
            let tab = st.active_tab_mut().unwrap();
            tab.spawn_preset = AgentCliPreset::Claude;
            tab.preset = AgentCliPreset::Claude;
        }
        let _rec = attach_recording_writer(st.active_tab_mut().unwrap());

        assert_eq!(st.send_to_agent("First sent prompt", None), SendOutcome::Written);
        assert_eq!(st.send_to_agent("Second sent prompt", None), SendOutcome::Written);

        let panel_rect = Rect::from_min_size(Pos2::new(48.0, 0.0), Vec2::new(260.0, 720.0));
        let input_id = Id::new("agents-ask-for-changes-input");
        let press_key = |ctx: &egui::Context, st: &mut TerminalState, key: Key| {
            ctx.memory_mut(|m| m.request_focus(input_id));
            let raw = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1200.0, 800.0))),
                events: vec![Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::NONE }],
                ..Default::default()
            };
            let _ = ctx.run_ui(raw, |ui| {
                let mut acts = Vec::new();
                let _ = show_composer(st, ui, panel_rect, 80.0, None, MentionScope { doc: &Document::new("Composer"), page: NodeId(1) }, &mut acts);
            });
        };

        // `#689` §3 rule 4 & §8 edge case 2: Up arrow in a non-empty draft (even a single line) must NOT
        // replace the draft with history; history starts only on an empty field (or while already browsing).
        st.set_composer_draft("make the button blue");
        press_key(&ctx, &mut st, Key::ArrowUp);
        assert_eq!(st.composer_draft(), "make the button blue", "Up arrow on a non-empty single-line draft must not overwrite it with history");
        assert!(st.active_composer().cursor.is_none(), "History browsing must not start when the field is non-empty");

        // Empty the field so Up arrow starts history browsing from the empty draft.
        st.set_composer_draft("");

        press_key(&ctx, &mut st, Key::ArrowUp);
        assert_eq!(st.composer_draft(), "Second sent prompt");

        press_key(&ctx, &mut st, Key::ArrowUp);
        assert_eq!(st.composer_draft(), "First sent prompt");

        press_key(&ctx, &mut st, Key::ArrowDown);
        assert_eq!(st.composer_draft(), "Second sent prompt");

        press_key(&ctx, &mut st, Key::ArrowDown);
        assert_eq!(st.composer_draft(), "", "Down past the newest entry must restore the empty draft that was in the field before Up");

        // Direct unit check on `ComposerState::history_up` / `history_down` stashing and restoring an unsent draft.
        st.set_composer_draft("Unsent third draft");
        assert!(st.active_composer_mut().history_up());
        assert_eq!(st.composer_draft(), "Second sent prompt");
        assert!(st.active_composer_mut().history_up());
        assert_eq!(st.composer_draft(), "First sent prompt");
        assert!(st.active_composer_mut().history_down());
        assert_eq!(st.composer_draft(), "Second sent prompt");
        assert!(st.active_composer_mut().history_down());
        assert_eq!(st.composer_draft(), "Unsent third draft", "Down past the end must restore the stashed unsent third message");

        st.shutdown();
    }

    // 6. `each_tab_keeps_its_draft`
    #[test]
    fn each_tab_keeps_its_draft() {
        let mut st = TerminalState::default();
        st.spawn_tab(AgentCliPreset::Shell, None).expect("spawn tab 0");
        st.spawn_tab(AgentCliPreset::Shell, None).expect("spawn tab 1");
        for tab in &mut st.tabs {
            tab.spawn_preset = AgentCliPreset::Claude;
            tab.preset = AgentCliPreset::Claude;
        }

        st.active_tab = 0;
        st.set_composer_draft("Draft for Claude tab 0");

        st.active_tab = 1;
        assert_eq!(st.composer_draft(), "", "Tab 1 must start with its own empty draft");
        st.set_composer_draft("Draft for Codex tab 1");

        st.active_tab = 0;
        assert_eq!(st.composer_draft(), "Draft for Claude tab 0", "Tab 0 must keep its draft");

        st.active_tab = 1;
        assert_eq!(st.composer_draft(), "Draft for Codex tab 1", "Tab 1 must keep its draft");

        st.shutdown();
    }

    // 7. `not_running_starts_then_sends_when_ready`
    #[test]
    fn not_running_starts_then_sends_when_ready() {
        let dir = temp_composer_dir("not-running-starts");
        let bin_dir = dir.join("bin");
        let recv_file = dir.join("queued-recv.txt");
        let script = format!("printf '\\033[?2004hReady> '\nIFS= read -r line\nprintf '%s' \"$line\" > '{}'\nsleep 2", recv_file.display());
        write_executable_stub(&bin_dir, "claude", &script);

        let mut st = TerminalState::new_for_session("doc-689-7", "Lazy Start", Some(&dir));
        st.path_prepend.push(bin_dir);
        st.active_runtime = AgentCliPreset::Claude;
        assert!(st.tabs.is_empty());

        let outcome = st.send_to_agent("Build hero card when ready", None);
        assert_eq!(outcome, SendOutcome::Queued);
        assert_eq!(st.tabs.len(), 1, "Sending while not running must start the agent tab");
        assert_eq!(st.active_composer().pending_send.as_deref(), Some("Build hero card when ready"));
        assert_eq!(st.active_composer().notice.as_deref(), Some("Starting Claude… your message will be sent"));

        // Poll bridge until the newly spawned agent becomes ready, receives the queued message, and gets `\r`.
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut delivered = String::new();
        while Instant::now() < deadline {
            let _ = st.poll_live_agent_bridge();
            delivered = std::fs::read_to_string(&recv_file).unwrap_or_default();
            if delivered.contains("Build hero card when ready") {
                break;
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        let raw_out = st.active_tab().and_then(|t| t.raw_log.lock().ok().map(|g| g.clone())).unwrap_or_default();
        assert!(st.active_composer().pending_send.is_none(), "Queued message must be flushed once ready (raw_out={raw_out:?})");
        assert!(delivered.contains("Build hero card when ready"), "Queued message must reach the started CLI: {delivered:?}, raw_out={raw_out:?}");

        st.shutdown();
        let _ = std::fs::remove_dir_all(&dir);
    }

    // 8. `failed_start_returns_the_text_to_the_draft`
    #[test]
    fn failed_start_returns_the_text_to_the_draft() {
        let dir = temp_composer_dir("failed-start-restores-draft");
        let bin_dir = dir.join("bin");
        write_executable_stub(&bin_dir, "claude", "echo 'Error: failed to initialize' >&2\nexit 1");

        let mut st = TerminalState::new_for_session("doc-689-8", "Failed Start", Some(&dir));
        st.path_prepend.push(bin_dir);
        st.active_runtime = AgentCliPreset::Claude;

        let outcome = st.send_to_agent("Restore this draft when start fails", None);
        assert_eq!(outcome, SendOutcome::Queued);
        assert_eq!(st.composer_draft(), "", "Draft is cleared while queued");

        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            if let Some(tab) = st.active_tab_mut() {
                if !tab.is_alive() {
                    tab.spawned_at = Instant::now();
                }
            }
            let _ = st.poll_live_agent_bridge();
            if st.active_tab().is_some_and(|t| t.start_failure.is_some()) {
                break;
            }
            std::thread::sleep(Duration::from_millis(25));
        }

        assert!(st.active_tab().is_some_and(|t| t.start_failure.is_some()), "Start failure must be recorded");
        assert_eq!(
            st.composer_draft(),
            "Restore this draft when start fails",
            "Undelivered message must be returned to the draft when the agent fails to start"
        );
        assert_eq!(st.active_composer().notice.as_deref(), Some("Not delivered: check the terminal"));

        st.shutdown();
        let _ = std::fs::remove_dir_all(&dir);
    }

    // 9. `pasting_copied_layers_makes_a_chip`
    #[test]
    fn pasting_copied_layers_makes_a_chip() {
        let mut st = TerminalState::default();
        st.spawn_tab(AgentCliPreset::Shell, None).expect("spawn");
        {
            let tab = st.active_tab_mut().unwrap();
            tab.spawn_preset = AgentCliPreset::Claude;
            tab.preset = AgentCliPreset::Claude;
            if let Ok(mut p) = tab.parser.lock() {
                p.process(b"\x1b[?2004h");
            }
        }
        let rec = attach_recording_writer(st.active_tab_mut().unwrap());

        let mut doc = designos_doc::Document::new("Chip Test");
        let page = doc.add_page("Page 1");
        let frame_node = Node::frame(NodeId(42), designos_doc::Rect::new(0.0, 0.0, 320.0, 200.0));
        let mut comp_node = Node::frame(NodeId(99), designos_doc::Rect::new(0.0, 0.0, 120.0, 40.0));
        comp_node.name = "Primary Button".to_string();
        comp_node.kind = NodeKind::Component(designos_doc::FrameProps::default());
        doc.insert(page, frame_node).expect("insert frame");
        doc.insert(page, comp_node).expect("insert component");
        let clip = crate::clipboard::capture(&doc, &[NodeId(42), NodeId(99)]).expect("capture clip");
        let clip_json = crate::clipboard::to_json(&clip).expect("clipboard json");

        assert!(st.paste_into_composer(&clip_json), "Pasting copied canvas layers must be recognized");
        let chips = &st.active_composer().chips;
        assert_eq!(chips.len(), 2);
        assert_eq!(chips[0].node_id, NodeId(42));
        assert_eq!(chips[0].reference, "designos:node/42");
        assert!(!chips[0].is_component);
        assert_eq!(chips[1].node_id, NodeId(99));
        assert_eq!(chips[1].name, "Primary Button");
        assert_eq!(chips[1].reference, "designos:node/99");
        assert!(chips[1].is_component);
        assert!(!st.composer_draft().contains("\"nodes\""), "Raw JSON must never be dumped into the draft");
        assert!(st.composer_draft().contains("Primary Button"), "Plain layer name must be inserted into the draft");

        let prompt = format!("{} — align spacing", st.composer_draft());
        assert_eq!(st.send_to_agent(&prompt, None), SendOutcome::Written);
        st.flush_pending_enter_now();
        let sent = String::from_utf8_lossy(&rec.lock().unwrap()).into_owned();
        assert!(sent.contains("designos:node/42"), "Sent payload must include #655 layer reference: {sent}");
        assert!(sent.contains("designos:node/99 (component)"), "Sent payload must include component reference: {sent}");

        st.shutdown();
    }

    // 10. `all_entry_points_use_one_path`
    #[test]
    fn all_entry_points_use_one_path() {
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        crate::theme::apply(&ctx);

        let mut st = TerminalState::default();
        st.spawn_tab(AgentCliPreset::Shell, None).expect("spawn");
        {
            let tab = st.active_tab_mut().unwrap();
            tab.spawn_preset = AgentCliPreset::Claude;
            tab.preset = AgentCliPreset::Claude;
            if let Ok(mut p) = tab.parser.lock() {
                p.process(b"\x1b[?2004h");
            }
        }
        let rec = attach_recording_writer(st.active_tab_mut().unwrap());
        let panel_rect = Rect::from_min_size(Pos2::new(48.0, 0.0), Vec2::new(260.0, 720.0));

        // Entry point 1: composer Send button.
        st.set_composer_draft("Entry 1 from composer button");
        let _ = ctx.run_ui(egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1200.0, 800.0))), ..Default::default() }, |ui| {
            let _ = show_left(&mut st, ui, panel_rect, None);
        });
        let send_rect: Rect = ctx.data(|d| d.get_temp(Id::new(("test-rect", "agents-composer-send".to_string())))).expect("agents-composer-send rect");
        let _ = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1200.0, 800.0))),
                events: vec![
                    Event::PointerMoved(send_rect.center()),
                    Event::PointerButton { pos: send_rect.center(), button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE },
                    Event::PointerButton { pos: send_rect.center(), button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE },
                ],
                ..Default::default()
            },
            |ui| {
                let _ = show_left(&mut st, ui, panel_rect, None);
            },
        );
        st.flush_pending_enter_now();
        assert_eq!(st.active_composer().send_count, 1);

        // Entry point 2: `submit_chat_prompt` (used by Specs "Ask @agent").
        st.submit_chat_prompt("Entry 2 from submit_chat_prompt", None);
        st.flush_pending_enter_now();
        assert_eq!(st.active_composer().send_count, 2);

        // Entry point 3: `dispatch_figma_ai_prompt` (used by Cmd+K / Make design / toolbar AI actions).
        st.dispatch_figma_ai_prompt("Make Design", "Entry 3 from dispatch_figma_ai_prompt", None);
        st.flush_pending_enter_now();
        assert_eq!(st.active_composer().send_count, 3);

        // Entry point 4: Sample prompt card in `show_left` (`term-ai-rename-layers`).
        st.side_panel_pty_open = false;
        st.expanded_agent_cat = Some(0);
        let _ = ctx.run_ui(egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1200.0, 800.0))), ..Default::default() }, |ui| {
            let _ = show_left(&mut st, ui, panel_rect, None);
        });
        let sample_rect: Rect = ctx.data(|d| d.get_temp(Id::new(("test-rect", "term-ai-rename-layers".to_string())))).expect("term-ai-rename-layers rect");
        let _ = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1200.0, 800.0))),
                events: vec![
                    Event::PointerMoved(sample_rect.center()),
                    Event::PointerButton { pos: sample_rect.center(), button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE },
                    Event::PointerButton { pos: sample_rect.center(), button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE },
                ],
                ..Default::default()
            },
            |ui| {
                let _ = show_left(&mut st, ui, panel_rect, None);
            },
        );
        st.flush_pending_enter_now();
        assert_eq!(st.active_composer().send_count, 4);

        let all_written = String::from_utf8_lossy(&rec.lock().unwrap()).into_owned();
        for expected in [
            "Entry 1 from composer button",
            "Entry 2 from submit_chat_prompt",
            "Entry 3 from dispatch_figma_ai_prompt",
            "Semantically rename generic Frame/Rect/Text layers across selection",
        ] {
            assert!(all_written.contains(expected), "Entry point prompt `{expected}` missing from PTY stream: {all_written:?}");
            assert!(st.active_composer().history.iter().any(|h| h.contains(expected)), "Entry point prompt `{expected}` missing from composer history");
        }

        st.shutdown();
    }
    /// Polls the bridge until `file` contains `needle` or `secs` elapse; returns the file text.
    fn wait_for_file_containing(st: &mut TerminalState, file: &Path, needle: &str, secs: u64) -> String {
        let deadline = Instant::now() + Duration::from_secs(secs);
        let mut text = String::new();
        while Instant::now() < deadline {
            let _ = st.poll_live_agent_bridge();
            text = std::fs::read_to_string(file).unwrap_or_default();
            if text.contains(needle) {
                break;
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        text
    }

    /// Stub agent CLI that enables bracketed paste and appends every submitted line to `recv`.
    fn recording_agent_script(recv: &Path) -> String {
        format!("printf '\\033[?2004hReady> '\nwhile IFS= read -r line; do\n  printf '%s\\n' \"$line\" >> '{}'\ndone", recv.display())
    }

    fn sample_chip() -> LayerChip {
        LayerChip { node_id: NodeId(4521), name: "Hero/Title".to_string(), is_component: false, reference: "designos:node/4521".to_string() }
    }

    fn sample_mention() -> crate::agent_mention::Mention {
        crate::agent_mention::Mention {
            label: "Hero/Title".to_string(),
            kind: crate::agent_mention::MentionKind::Node,
            id: "4521".to_string(),
            name: "Hero/Title".to_string(),
            page: Some("Landing".to_string()),
        }
    }

    #[test]
    fn system_image_prompt_with_no_tab_starts_the_agent_cli_never_a_shell() {
        let dir = temp_composer_dir("system-no-tab");
        let bin_dir = dir.join("bin");
        let recv = dir.join("recv.txt");
        write_executable_stub(&bin_dir, "claude", &recording_agent_script(&recv));

        let mut st = TerminalState::new_for_session("doc-system-no-tab", "System Send", Some(&dir));
        st.path_prepend.push(bin_dir);
        assert!(st.tabs.is_empty());

        st.submit_system_image_prompt(AgentCliPreset::Claude, "Image task 7: Remove background.", None);

        assert_eq!(st.tabs.len(), 1, "exactly one tab is opened");
        {
            let tab = st.active_tab_mut().expect("agent tab");
            assert!(!tab.is_shell_process(), "the tab must be an agent CLI, not a shell relabelled as one");
            assert_eq!(tab.spawn_preset, AgentCliPreset::Claude);
            assert!(tab.spawned_command.as_deref().is_some_and(|c| c.contains("claude")), "the tab must launch the claude binary: {:?}", tab.spawned_command);
        }
        let received = wait_for_file_containing(&mut st, &recv, "Image task 7", 6);
        assert!(received.contains("Image task 7: Remove background."), "the agent CLI must receive the prompt: {received:?}");

        // A second job reuses the same agent tab.
        st.submit_system_image_prompt(AgentCliPreset::Claude, "Image task 8: Isolate subject.", None);
        assert_eq!(st.tabs.len(), 1, "a ready agent tab of the same CLI is reused");
        let received = wait_for_file_containing(&mut st, &recv, "Image task 8", 6);
        assert!(received.contains("Image task 8: Isolate subject."), "second prompt reaches the same CLI: {received:?}");

        st.shutdown();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn system_image_prompt_leaves_the_users_other_tabs_untouched() {
        let dir = temp_composer_dir("system-keeps-tabs");
        let bin_dir = dir.join("bin");
        let recv = dir.join("recv.txt");
        write_executable_stub(&bin_dir, "claude", &recording_agent_script(&recv));
        write_executable_stub(&bin_dir, "codex", "sleep 5");

        let mut st = TerminalState::new_for_session("doc-system-keeps-tabs", "System Send", Some(&dir));
        st.path_prepend.push(bin_dir);
        st.spawn_tab(AgentCliPreset::Shell, None).expect("spawn shell");
        st.spawn_tab(AgentCliPreset::Codex, None).expect("spawn codex");
        st.active_tab = 0;
        let before: Vec<(AgentCliPreset, AgentCliPreset, String)> = st.tabs.iter().map(|t| (t.preset, t.spawn_preset, t.title.clone())).collect();

        st.submit_system_image_prompt(AgentCliPreset::Claude, "Image task 9: Remove background.", None);

        assert_eq!(st.tabs.len(), 3, "a Claude tab is added next to the user's Shell and Codex tabs");
        for (i, (preset, spawn_preset, title)) in before.iter().enumerate() {
            let tab = &st.tabs[i];
            assert_eq!((tab.preset, tab.spawn_preset, &tab.title), (*preset, *spawn_preset, title), "tab {i} keeps its preset and title");
            let written = String::from_utf8_lossy(&tab.written_bytes).into_owned();
            assert!(!written.contains("Image task"), "tab {i} must never receive the system prompt: {written:?}");
        }
        assert_eq!(st.active_tab, 2, "the new agent tab is active");
        let received = wait_for_file_containing(&mut st, &recv, "Image task 9", 6);
        assert!(received.contains("Image task 9"), "the Claude tab receives the prompt: {received:?}");

        st.shutdown();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn system_send_keeps_the_users_draft_chips_mentions_and_history() {
        let dir = temp_composer_dir("system-keeps-composer");
        let bin_dir = dir.join("bin");
        let recv = dir.join("recv.txt");
        write_executable_stub(&bin_dir, "claude", &recording_agent_script(&recv));

        let mut st = TerminalState::new_for_session("doc-system-keeps-composer", "System Send", Some(&dir));
        st.path_prepend.push(bin_dir);
        st.spawn_tab(AgentCliPreset::Claude, None).expect("spawn claude");
        let ready_deadline = Instant::now() + Duration::from_secs(4);
        while Instant::now() < ready_deadline && !st.active_tab().is_some_and(|t| t.raw_log.lock().ok().is_some_and(|l| l.contains("Ready>"))) {
            let _ = st.poll_live_agent_bridge();
            std::thread::sleep(Duration::from_millis(25));
        }
        st.active_composer_mut().record_sent("older prompt");
        st.set_composer_draft("half typed thought");
        st.active_composer_mut().chips.push(sample_chip());
        st.active_mentions.push(sample_mention());
        let sends_before = st.active_composer().send_count;

        st.submit_system_image_prompt(AgentCliPreset::Claude, "Image task 3: Remove background. Compare @Hero/Title.", None);

        let comp = st.active_composer();
        assert_eq!(comp.draft, "half typed thought", "a system send must not clear the draft");
        assert_eq!(comp.chips, vec![sample_chip()], "a system send must not consume layer chips");
        assert_eq!(comp.history.iter().map(String::as_str).collect::<Vec<_>>(), vec!["older prompt"], "a system send never enters Up-arrow history");
        assert_eq!(comp.send_count, sends_before, "a system send is not a composer send");
        assert_eq!(st.active_mentions, vec![sample_mention()], "a system send must not consume mentions");

        let received = wait_for_file_containing(&mut st, &recv, "Image task 3", 6);
        assert!(received.contains("Image task 3: Remove background. Compare @Hero/Title."), "prompt arrives verbatim: {received:?}");
        assert!(!received.contains("References:"), "the user's layer references must not be attached: {received:?}");
        assert!(!received.contains("designos:node/4521"), "no unrelated layer reference in the system prompt: {received:?}");

        st.shutdown();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn failed_start_of_a_system_send_never_lands_in_the_draft() {
        let dir = temp_composer_dir("system-failed-start");
        let bin_dir = dir.join("bin");
        write_executable_stub(&bin_dir, "claude", "echo 'Error: failed to initialize' >&2\nexit 1");

        let mut st = TerminalState::new_for_session("doc-system-failed-start", "System Send", Some(&dir));
        st.path_prepend.push(bin_dir);

        st.submit_system_image_prompt(AgentCliPreset::Claude, "Image task 5: Remove background.", None);

        let deadline = Instant::now() + Duration::from_secs(6);
        while Instant::now() < deadline {
            if let Some(tab) = st.active_tab_mut() {
                if !tab.is_alive() {
                    tab.spawned_at = Instant::now();
                }
            }
            let _ = st.poll_live_agent_bridge();
            if st.active_tab().is_some_and(|t| t.start_failure.is_some()) {
                break;
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        assert!(st.active_tab().is_some_and(|t| t.start_failure.is_some()), "the stub CLI fails to start");
        let _ = st.poll_live_agent_bridge();
        assert_eq!(st.composer_draft(), "", "a system prompt is dropped, never returned to the user's draft");
        assert!(st.active_composer().pending_send.is_none());

        st.shutdown();
        let _ = std::fs::remove_dir_all(&dir);
    }
}
