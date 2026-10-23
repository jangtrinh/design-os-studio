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
    /// More skills are attached than a run allows, or their text is too long (`#573`).
    SkillLimit,
}

impl RefuseReason {
    /// User-facing status caption shown under the composer when a send is refused.
    pub fn message(&self) -> &'static str {
        match self {
            Self::Empty => "Enter a prompt to send",
            Self::TooLong => "Message is over 100,000 characters. Shorten it.",
            Self::NoAgent => "Select an agent tab",
            Self::Permission => "Answer the permission prompt in the terminal",
            Self::NotReady => "Agent is still starting",
            Self::NotWritable => "Not delivered: check the terminal",
            Self::StartFailed => "Not delivered: check the terminal",
            Self::SkillLimit => "Remove a skill to send",
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
    /// Fingerprint of the skill set last attached to this tab (`#573`): the tab's first send attaches the selected skills, and
    /// so does the next send once the set differs from this one. `None` until a send attached (or recorded) a set.
    pub skills_sent: Option<u64>,
    /// Chunks of fenced skill blocks the skills hook replies carry for this Claude Code tab until its first prompt is submitted
    /// (`#573`); empty when nothing waits. Each chunk is one reply file and stays under Claude Code's hook size cap.
    pub skills_hook_chunks: Vec<String>,
    /// When the prompt that carries `skills_hook_chunks` was seen submitted; the chunks leave the replies a moment after.
    pub skills_hook_submitted_at: Option<Instant>,
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

/// Writes `prompt` into the live agent tab with the fenced `skills_block` in front of it (`#573`). The block is empty when this send
/// attaches nothing, and when Claude Code reads the skills from its `UserPromptSubmit` hook ([`crate::skills::delivery::SkillsAttachment::paste_block`]):
/// a second copy in the prompt would double their tokens. Control characters are stripped later, by [`format_composer_paste_bytes`].
/// A `system` prompt (image jobs) is never restored into the user's draft. Only `prompt` is ever restored, never the skill blocks.
fn write_prompt_to_live_tab(tab: &mut TerminalTab, prompt: &str, skills_block: &str, status_msg: &mut Option<String>, system: bool) -> SendOutcome {
    let bracketed = tab.parser.lock().ok().map(|p| p.screen().bracketed_paste()).unwrap_or(true);
    let wire = crate::skills::prepend_skill_blocks(skills_block, prompt);
    let payload_text = if !bracketed && wire.contains('\n') {
        if !tab.composer.unbracketed_newline_warned {
            tab.composer.unbracketed_newline_warned = true;
            *status_msg = Some("Sent multiline prompt as one line (CLI has bracketed paste off)".to_string());
        }
        wire.replace('\n', " ")
    } else {
        wire
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
    // A custom "Other CLI…" command is a non-Claude CLI, and a paste that carries skills (up to 8,000 tokens, also into a Claude Code
    // tab whose set the hook cannot carry) is as big as any other CLI's: both wait the size-scaled delay.
    let slow_paste = tab.is_custom_cli() || !skills_block.is_empty();
    let delay = enter_delay_for_preset(if slow_paste { AgentCliPreset::Shell } else { tab.spawn_preset }, paste_bytes.len());
    let now = Instant::now();
    tab.composer.pending_enter_deadline = Some(now + delay);
    tab.composer.pending_enter_delay = Some(delay);
    tab.composer.awaiting_confirm_since = Some(now);
    tab.composer.log_len_at_send = tab.raw_log.lock().ok().map(|l| l.len()).unwrap_or(0);
    if !system && tab.spawned_at.elapsed() <= crate::agent_start_failure::START_FAILURE_WINDOW {
        tab.composer.sent_in_start_window = Some(prompt.to_string());
    }
    // The sent text is what `Retry` resends (`#686`).
    tab.last_user_prompt = Some(prompt.to_string());
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
                    // The skills chosen now are the ones attached: the user may have changed or edited them while the CLI started.
                    if !system {
                        self.skills.reload_selected();
                    }
                    match (!system).then(|| self.skills.limit_violation()).flatten() {
                        Some(violation) => {
                            tab.composer.restore_undelivered(queued, violation, RefuseReason::SkillLimit);
                            self.status_msg = tab.composer.notice.clone();
                        }
                        None => {
                            let attachment = if system { None } else { crate::skills::delivery::plan_attachment(&self.skills, tab) };
                            let skills_block = attachment.as_ref().map_or("", |a| a.paste_block());
                            let outcome = write_prompt_to_live_tab(tab, &queued, skills_block, &mut self.status_msg, system);
                            if let (SendOutcome::Written, Some(attachment)) = (outcome, attachment) {
                                if let Err(e) = crate::skills::delivery::commit_attachment(&self.skills, tab, &attachment, &self.session_dir, &self.session_id)
                                {
                                    self.status_msg = Some(e);
                                }
                            }
                        }
                    }
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

        // 5. Skills a flush just attached to a Claude Code tab enter the hook reply; ones whose prompt was submitted, or whose tab
        // closed, leave it (`#573`).
        self.sync_hook_reply();
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

    /// A prompt that is written or queued starts a new turn, so it dismisses a mid-session failure banner
    /// (`#686`). Clearing at send time, not at the later queued write, keeps a failure that arrives while the
    /// prompt waits for a quiet tab.
    fn send_to_agent_from(&mut self, text: &str, file_path: Option<&Path>, source: SendSource) -> SendOutcome {
        let outcome = self.deliver_to_agent(text, file_path, source);
        if matches!(outcome, SendOutcome::Written | SendOutcome::Queued) {
            if let Some(tab) = self.active_tab_mut() {
                tab.clear_session_failure();
            }
        }
        outcome
    }

    fn deliver_to_agent(&mut self, text: &str, file_path: Option<&Path>, source: SendSource) -> SendOutcome {
        // Poll first so any child that already exited within `START_FAILURE_WINDOW` has `start_failure` populated.
        // The actions it returns belong to the next bridge poll, not to this send.
        let early_actions = self.poll_live_agent_bridge();
        self.deferred_bridge_actions.extend(early_actions);
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

        // More skills than a run allows (`#573`): refuse like any other unsendable state and keep the user's text.
        // The selected `SKILL.md` files are re-read first, so the text on disk is the text attached.
        if !system {
            self.skills.reload_selected();
            if let Some(violation) = self.skills.limit_violation() {
                let comp = self.active_composer_mut();
                comp.last_outcome = Some(SendOutcome::Refused(RefuseReason::SkillLimit));
                comp.notice = Some(violation.clone());
                self.status_msg = Some(violation);
                return SendOutcome::Refused(RefuseReason::SkillLimit);
            }
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
                        return write_prompt_to_live_tab(tab, &payload, "", &mut self.status_msg, true);
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
                // A run's first send attaches the selected skills, and so does the first send after the set changed (`#573`).
                let attachment = crate::skills::delivery::plan_attachment(&self.skills, tab);
                let skills_block = attachment.as_ref().map_or("", |a| a.paste_block());
                let outcome = write_prompt_to_live_tab(tab, &payload, skills_block, &mut self.status_msg, false);
                if let (SendOutcome::Written, Some(attachment)) = (outcome, attachment) {
                    if let Err(e) = crate::skills::delivery::commit_attachment(&self.skills, tab, &attachment, &self.session_dir, &self.session_id) {
                        self.status_msg = Some(e);
                    }
                }
                // Skills for a Claude Code tab go into its hook reply now, before the delayed Enter submits the prompt.
                self.sync_hook_reply();
                return outcome;
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
            // After `show`: a click elsewhere has already taken the focus by now, which is what the next frame starts from.
            state.record_composer_focus(ui.memory(|m| m.has_focus(input_id)));
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

    // `Skills` chip (`#573`) right of `PRD`: opens the picker of built-in and user `SKILL.md` files attached to a run.
    let skills_w = state.skills.chip_width(&p).max(PRD_CHIP_WIDTH);
    let skills_r = Rect::from_min_size(Pos2::new(prd_r.right() + theme::SPACE_ROW, prd_r.top()), Vec2::new(skills_w, prd_r.height()));
    state.skills.paint_chip(ui, &p, skills_r, "agents-composer-skills");

    // Bottom-right Send circle button (`CONTROL_STANDARD` 28 px, `BG_BRAND` with dark arrow; disabled and `TEXT_SECONDARY` when empty — `#689` §7).
    // Over the skill limit the circle turns red and stops being a button (hover only, with the reason); Enter still runs the send,
    // which names the limit in the notice and keeps the draft (`RD-4` §3.3: the start action is disabled).
    let skills_violation = state.skills.limit_violation();
    let skills_over_limit = skills_violation.is_some();
    let has_sendable = !is_shell_tab && (!state.composer_draft().trim().is_empty() || !state.active_composer().chips.is_empty());
    let send_radius = theme::radius_full(theme::CONTROL_STANDARD);
    let send_center = Pos2::new(composer_rect.right() - theme::INSET_CONTROL - send_radius, icon_y);
    let send_r = Rect::from_center_size(send_center, Vec2::splat(theme::CONTROL_STANDARD));
    let send_resp = ui.interact(send_r, Id::new("agents-composer-send"), if skills_over_limit { Sense::hover() } else { Sense::click() });
    let send_resp = match &skills_violation {
        Some(reason) => send_resp.on_hover_text(reason.as_str()),
        None => send_resp,
    };
    let send_bg = if skills_over_limit {
        crate::terminal::STATUS_BAD_BG
    } else if has_sendable {
        if send_resp.hovered() {
            theme::BG_BRAND_HOVER
        } else {
            theme::BG_BRAND
        }
    } else {
        theme::BG_TERTIARY
    };
    let send_fg = if skills_over_limit {
        crate::terminal::STATUS_BAD_TEXT
    } else if has_sendable {
        theme::TEXT_ON_LIGHT_CANVAS
    } else {
        theme::TEXT_SECONDARY
    };
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

    /// Polls the bridge every 25 ms until `condition` holds or `STUB_WAIT` passes, and returns whether it held. The stub CLIs
    /// are real child processes, and a loaded machine can take seconds just to start one, so the wait ends on the event it names.
    fn poll_bridge_until(st: &mut TerminalState, mut condition: impl FnMut(&mut TerminalState) -> bool) -> bool {
        crate::test_timing::poll_until(crate::test_timing::STUB_WAIT, || {
            let _ = st.poll_live_agent_bridge();
            condition(st)
        })
    }

    /// Leaves only the simulated frame time to decide when a pending delayed `\r` is written: the real-clock deadline, which a loaded
    /// machine can reach between two frames of a test, is pushed out of reach.
    fn defer_pending_enter_to_frame_time(st: &mut TerminalState) {
        for tab in &mut st.tabs {
            if tab.composer.pending_enter_deadline.is_some() {
                tab.composer.pending_enter_deadline = Some(Instant::now() + Duration::from_secs(3600));
            }
        }
    }

    /// Checks the real-clock deadline of the delayed `\r` the active tab holds right after a send, before
    /// [`defer_pending_enter_to_frame_time`] pushes it out of reach. `before` and `after` are read around the send.
    ///
    /// The delay is the CLI's own. The deadline is the instant the paste was written plus that delay, so it lies in
    /// `[before + delay, after + delay]` on the real clock, and it is exactly `awaiting_confirm_since + delay`. The second check holds
    /// however long a loaded machine stalled inside the send, so a deadline set to the send instant (no delay at all) fails it on every run.
    fn assert_enter_deadline_follows_the_send(st: &TerminalState, expected_delay: Duration, before: Instant, after: Instant, which: &str) {
        let tab = st.active_tab().expect("the active tab");
        let delay = tab.composer.pending_enter_delay.expect("a send leaves its Enter delay on the composer");
        assert_eq!(delay, expected_delay, "{which}: the Enter delay of this CLI");
        let deadline = tab.composer.pending_enter_deadline.expect("a send leaves its Enter deadline on the composer");
        assert!(
            deadline >= before + delay,
            "{which}: the Enter deadline must be at least {delay:?} after the send, but it is only {:?} after the moment before it",
            deadline.saturating_duration_since(before)
        );
        assert!(
            deadline <= after + delay,
            "{which}: the Enter deadline must be at most {delay:?} after the send, but it is {:?} after the moment it returned",
            deadline.saturating_duration_since(after)
        );
        let sent_at = tab.composer.awaiting_confirm_since.expect("a send stamps the instant it wrote the paste");
        assert_eq!(deadline, sent_at + delay, "{which}: the Enter deadline must be the instant the paste was written plus the delay");
    }

    /// Waits until the active tab's stub CLI has printed its `Ready>` prompt.
    fn wait_for_ready_prompt(st: &mut TerminalState) {
        let ready = poll_bridge_until(st, |st| st.active_tab().is_some_and(|t| t.raw_log.lock().ok().is_some_and(|l| l.contains("Ready>"))));
        assert!(ready, "the stub CLI must print its Ready> prompt");
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
        let ready = poll_bridge_until(&mut st, |st| {
            st.active_tab().is_some_and(|t| {
                t.parser.lock().ok().is_some_and(|p| p.screen().bracketed_paste()) && t.raw_log.lock().ok().is_some_and(|l| l.contains("Ready>"))
            })
        });
        assert!(ready, "the stub CLI must print Ready> and enable bracketed paste");

        // Send message 1 and let the delayed `\r` fire through `tick_composer`.
        assert_eq!(st.send_to_agent("First message in session", None), SendOutcome::Written);
        std::thread::sleep(CLAUDE_ENTER_DELAY + Duration::from_millis(20));
        st.tick_composer();

        // Send message 2 into the same tab and let the delayed `\r` fire.
        assert_eq!(st.send_to_agent("Second message in session", None), SendOutcome::Written);
        std::thread::sleep(CLAUDE_ENTER_DELAY + Duration::from_millis(20));
        st.tick_composer();

        // Wait for both lines to be written by the single running process.
        let mut log_contents = String::new();
        crate::test_timing::poll_until(crate::test_timing::STUB_WAIT, || {
            log_contents = std::fs::read_to_string(&log_file).unwrap_or_default();
            log_contents.contains("First message in session") && log_contents.contains("Second message in session")
        });

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

        let mut st = crate::test_timing::ScopedTerminalState::default();
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
        let before_send = Instant::now();
        let _ = ctx.run_ui(raw_enter, |ui| {
            let mut acts = Vec::new();
            let _ = show_composer(&mut st, ui, panel_rect, 80.0, None, MentionScope { doc: &Document::new("Composer"), page: NodeId(1) }, &mut acts);
        });
        let after_send = Instant::now();
        assert_eq!(st.composer_draft(), "", "Plain Enter must clear the draft after sending");
        assert_eq!(&*rec.lock().unwrap(), b"\x1b[200~line one\nline two\x1b[201~", "Bracketed paste payload must be written first, before the delayed Enter");
        // The real-clock delay is judged here, before the deadline is pushed out so that only the simulated frame times below decide the flush.
        assert_enter_deadline_follows_the_send(&st, CLAUDE_ENTER_DELAY, before_send, after_send, "Claude");
        defer_pending_enter_to_frame_time(&mut st);

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
        let before_codex_send = Instant::now();
        let _ = ctx.run_ui(raw_codex_enter, |ui| {
            let mut acts = Vec::new();
            let _ = show_composer(&mut st, ui, panel_rect, 80.0, None, MentionScope { doc: &Document::new("Composer"), page: NodeId(1) }, &mut acts);
        });
        let after_codex_send = Instant::now();
        assert_eq!(&*rec.lock().unwrap(), b"\x1b[200~codex line\x1b[201~");
        let codex_delay = enter_delay_for_preset(AgentCliPreset::Codex, rec.lock().unwrap().len());
        assert!(codex_delay >= OTHER_CLI_BASE_ENTER_DELAY, "a non-Claude CLI waits at least the base delay: {codex_delay:?}");
        assert_enter_deadline_follows_the_send(&st, codex_delay, before_codex_send, after_codex_send, "Codex");
        defer_pending_enter_to_frame_time(&mut st);

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
        let mut st = crate::test_timing::ScopedTerminalState::default();
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

        let mut st = crate::test_timing::ScopedTerminalState::default();
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
        let mut st = crate::test_timing::ScopedTerminalState::default();
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
        assert_eq!(st.active_composer().notice.as_deref(), Some("Starting Claude Code… your message will be sent"));

        // Poll bridge until the newly spawned agent becomes ready, receives the queued message, and gets `\r`.
        let mut delivered = String::new();
        poll_bridge_until(&mut st, |_| {
            delivered = std::fs::read_to_string(&recv_file).unwrap_or_default();
            delivered.contains("Build hero card when ready")
        });
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

        assert!(st.poll_until_start_failure(crate::test_timing::STUB_WAIT), "Start failure must be recorded");
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
        let mut st = crate::test_timing::ScopedTerminalState::default();
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

        let mut st = crate::test_timing::ScopedTerminalState::default();
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

        // Entry point 4: a sample prompt row fills the composer, and the Send button then sends it through the same path.
        st.side_panel_pty_open = false;
        let frame = |st: &mut TerminalState, events: Vec<Event>| {
            let _ = ctx.run_ui(
                egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1200.0, 800.0))), events, ..Default::default() },
                |ui| {
                    let _ = show_left(st, ui, panel_rect, None);
                },
            );
        };
        let click = |rect: Rect| {
            vec![
                Event::PointerMoved(rect.center()),
                Event::PointerButton { pos: rect.center(), button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE },
                Event::PointerButton { pos: rect.center(), button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE },
            ]
        };
        frame(&mut st, Vec::new());
        let sample_rect: Rect = ctx.data(|d| d.get_temp(Id::new(("test-rect", "agents-sample-prompt-0".to_string())))).expect("agents-sample-prompt-0 rect");
        frame(&mut st, click(sample_rect));
        let sample_text = crate::agent_sample_prompts::sample_prompts(st.prompt_context)[0].text.clone();
        assert_eq!(st.composer_draft(), sample_text, "the sample prompt row must only fill the composer");
        assert_eq!(st.active_composer().send_count, 3, "filling the composer must not send");
        frame(&mut st, Vec::new());
        let send_rect: Rect = ctx.data(|d| d.get_temp(Id::new(("test-rect", "agents-composer-send".to_string())))).expect("agents-composer-send rect");
        frame(&mut st, click(send_rect));
        st.flush_pending_enter_now();
        assert_eq!(st.active_composer().send_count, 4);

        let all_written = String::from_utf8_lossy(&rec.lock().unwrap()).into_owned();
        for expected in ["Entry 1 from composer button", "Entry 2 from submit_chat_prompt", "Entry 3 from dispatch_figma_ai_prompt", sample_text.as_str()] {
            assert!(all_written.contains(expected), "Entry point prompt `{expected}` missing from PTY stream: {all_written:?}");
            assert!(st.active_composer().history.iter().any(|h| h.contains(expected)), "Entry point prompt `{expected}` missing from composer history");
        }

        st.shutdown();
    }
    /// Polls the bridge until `file` contains `needle` or `STUB_WAIT` passes; returns the file text.
    fn wait_for_file_containing(st: &mut TerminalState, file: &Path, needle: &str) -> String {
        let mut text = String::new();
        poll_bridge_until(st, |_| {
            text = std::fs::read_to_string(file).unwrap_or_default();
            text.contains(needle)
        });
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
        let received = wait_for_file_containing(&mut st, &recv, "Image task 7");
        assert!(received.contains("Image task 7: Remove background."), "the agent CLI must receive the prompt: {received:?}");

        // A second job reuses the same agent tab.
        st.submit_system_image_prompt(AgentCliPreset::Claude, "Image task 8: Isolate subject.", None);
        assert_eq!(st.tabs.len(), 1, "a ready agent tab of the same CLI is reused");
        let received = wait_for_file_containing(&mut st, &recv, "Image task 8");
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
        let received = wait_for_file_containing(&mut st, &recv, "Image task 9");
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
        wait_for_ready_prompt(&mut st);
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

        let received = wait_for_file_containing(&mut st, &recv, "Image task 3");
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

        assert!(st.poll_until_start_failure(crate::test_timing::STUB_WAIT), "the stub CLI fails to start");
        let _ = st.poll_live_agent_bridge();
        assert_eq!(st.composer_draft(), "", "a system prompt is dropped, never returned to the user's draft");
        assert!(st.active_composer().pending_send.is_none());

        st.shutdown();
        let _ = std::fs::remove_dir_all(&dir);
    }
    /// Writes `skills/<folder>/SKILL.md` under `lib_root`, rescans the picker's skills and returns the selector `user:<folder>`.
    fn write_user_skill(st: &mut TerminalState, lib_root: &Path, folder: &str, body: &str) -> String {
        let skill_dir = lib_root.join("skills").join(folder);
        std::fs::create_dir_all(&skill_dir).expect("create skill dir");
        std::fs::write(skill_dir.join("SKILL.md"), format!("---\nname: {folder}\ndescription: Test skill\nversion: 1.0.0\n---\n\n{body}\n"))
            .expect("write skill");
        st.skills.reload_all();
        format!("user:{folder}")
    }

    /// Spawns a stub `preset` CLI that has bracketed paste on, waits until it is ready, and returns a recorder of every byte written to its PTY.
    fn spawn_ready_stub_cli(st: &mut TerminalState, dir: &Path, preset: AgentCliPreset) -> Arc<Mutex<Vec<u8>>> {
        let bin_dir = dir.join("bin");
        let recv = dir.join(format!("{}-recv.txt", preset.binary_name()));
        write_executable_stub(&bin_dir, preset.binary_name(), &recording_agent_script(&recv));
        st.path_prepend.push(bin_dir);
        st.spawn_tab(preset, None).expect("spawn stub cli");
        wait_for_ready_prompt(st);
        // The CLI counts as ready only after a quiet spell following its last output.
        std::thread::sleep(READY_QUIET_BRACKETED * 3);
        attach_recording_writer(st.active_tab_mut().expect("stub tab"))
    }

    #[test]
    fn a_chosen_skill_reaches_a_non_claude_cli_as_one_sanitized_paste() {
        let dir = temp_composer_dir("skill-reaches-codex");
        let mut st = TerminalState::new_for_session("doc-skill-codex", "Skill Send", Some(&dir));
        // A user skill whose body tries to end the bracketed paste early and to submit a line with a bare CR.
        let selector = write_user_skill(&mut st, &dir, "escapey", "line one\x1b[201~injected\rline two");
        st.skills.toggle_skill(&selector);
        assert_eq!(st.skills.selected_entries().len(), 1);
        let sent = spawn_ready_stub_cli(&mut st, &dir, AgentCliPreset::Codex);

        assert_eq!(st.send_to_agent("Make a pricing card", None), SendOutcome::Written);

        let bytes = sent.lock().unwrap().clone();
        let text = String::from_utf8_lossy(&bytes).to_string();
        assert!(text.contains("```designos-skill:escapey@1.0.0\n"), "the fenced skill block must reach the CLI: {text:?}");
        assert!(text.contains("line one\u{241B}[201~injected\nline two"), "the skill's ESC becomes a visible mark and its CR a newline: {text:?}");
        assert_eq!(text.matches("\x1b[201~").count(), 1, "only the closing paste marker may carry an ESC: {text:?}");
        assert_eq!(text.matches("\x1b[200~").count(), 1, "one paste, not several: {text:?}");
        assert!(text.ends_with("Make a pricing card\x1b[201~"), "the user's prompt follows the skill blocks inside the same paste: {text:?}");

        st.shutdown();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn claude_gets_skills_from_its_hook_file_and_not_in_the_prompt() {
        let dir = temp_composer_dir("skill-claude-hook");
        let mut st = TerminalState::new_for_session("doc-skill-claude", "Skill Send", Some(&dir));
        let selector = write_user_skill(&mut st, &dir, "claude-only", "Keep gutters on the 8px grid.");
        st.skills.toggle_skill(&selector);
        let sent = spawn_ready_stub_cli(&mut st, &dir, AgentCliPreset::Claude);

        assert_eq!(st.send_to_agent("Make a pricing card", None), SendOutcome::Written);

        let text = String::from_utf8_lossy(&sent.lock().unwrap()).to_string();
        assert!(!text.contains("designos-skill:"), "Claude reads skills from the hook reply, a second copy in the prompt doubles the tokens: {text:?}");
        assert!(text.contains("Make a pricing card"));
        let reply = hook_reply(&st);
        assert!(reply.contains("designos-skill:claude-only@1.0.0"), "the hook reply must carry the chosen skill: {reply}");
        let page_reply = std::fs::read_to_string(st.session_dir.join("page-context-reply.json")).expect("page reply file");
        assert!(!page_reply.contains("designos-skill:"), "the page reply stays the page context alone: {page_reply}");

        st.shutdown();
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Overwrites `skills/<folder>/SKILL.md` without rescanning, like an editor saving the file while Studio is not looking.
    fn edit_user_skill_on_disk(lib_root: &Path, folder: &str, body: &str) {
        std::fs::write(
            lib_root.join("skills").join(folder).join("SKILL.md"),
            format!("---\nname: {folder}\ndescription: Test skill\nversion: 1.0.0\n---\n\n{body}\n"),
        )
        .expect("edit skill");
    }

    #[test]
    fn a_skill_edited_after_the_last_scan_reaches_claude_through_the_hook_reply() {
        let dir = temp_composer_dir("skill-edited-before-send");
        let mut st = TerminalState::new_for_session("doc-skill-edited", "Skill Send", Some(&dir));
        let selector = write_user_skill(&mut st, &dir, "edited", "First draft of the rule.");
        st.skills.toggle_skill(&selector);
        let sent = spawn_ready_stub_cli(&mut st, &dir, AgentCliPreset::Claude);
        assert!(skills_reply_files(&st.session_dir).is_empty(), "nothing is attached before the first send");

        // The user edits the file: no toggle, no page change, no rescan.
        edit_user_skill_on_disk(&dir, "edited", "Second draft of the rule.");

        assert_eq!(st.send_to_agent("Make a pricing card", None), SendOutcome::Written);
        let reply = hook_reply(&st);
        assert!(reply.contains("Second draft of the rule."), "the hook reply must carry the text on disk, which run.json records: {reply}");
        assert!(!reply.contains("First draft of the rule."), "no stale copy of the skill may remain: {reply}");
        let on_disk = st.skills.entries.iter().find(|e| e.id == "edited").map(|e| e.sha256.clone()).expect("skill entry");
        let runs = run_records(&st.session_dir);
        assert_eq!(runs[0]["skills"][0]["sha256"].as_str(), Some(on_disk.as_str()), "run.json records the attached text: {runs:?}");
        assert!(sent.lock().unwrap().len() > 2);

        st.shutdown();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_claude_send_queued_while_the_cli_starts_attaches_the_text_on_disk_at_the_flush() {
        let _turn = one_pty_test_at_a_time();
        let dir = temp_composer_dir("skill-queued-claude");
        let bin_dir = dir.join("bin");
        let recv = dir.join("claude-recv.txt");
        write_executable_stub(&bin_dir, "claude", &recording_agent_script(&recv));
        let mut st = TerminalState::new_for_session("doc-skill-queued-claude", "Skill Send", Some(&dir));
        st.path_prepend.push(bin_dir);
        st.active_runtime = AgentCliPreset::Claude;
        let selector = write_user_skill(&mut st, &dir, "queued-edit", "First draft of the rule.");
        st.skills.toggle_skill(&selector);
        assert!(st.tabs.is_empty());

        assert_eq!(st.send_to_agent("Make a pricing card", None), SendOutcome::Queued);
        edit_user_skill_on_disk(&dir, "queued-edit", "Second draft of the rule.");

        let received = wait_for_file_containing(&mut st, &recv, "Make a pricing card");
        assert!(received.contains("Make a pricing card"), "the queued prompt was flushed: {received:?}");
        assert!(!received.contains("designos-skill:"), "Claude never gets the skills in the pasted prompt: {received:?}");
        let reply = hook_reply(&st);
        assert!(reply.contains("Second draft of the rule.") && !reply.contains("First draft of the rule."), "the flush attaches the text on disk: {reply}");
        let on_disk = st.skills.entries.iter().find(|e| e.id == "queued-edit").map(|e| e.sha256.clone()).expect("skill entry");
        assert_eq!(run_records(&st.session_dir)[0]["skills"][0]["sha256"].as_str(), Some(on_disk.as_str()), "run.json records the same text");

        st.shutdown();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn too_many_skills_refuse_the_send_and_keep_the_draft() {
        let dir = temp_composer_dir("skill-limit-refuses");
        let mut st = TerminalState::new_for_session("doc-skill-limit", "Skill Send", Some(&dir));
        for builtin in ["design-system-rhythm", "accessibility-wcag", "component-craft"] {
            st.skills.toggle_skill(builtin);
        }
        let extra = write_user_skill(&mut st, &dir, "one-too-many", "Extra body.");
        st.skills.toggle_skill(&extra);
        assert!(!st.skills.can_start_run(), "four skills exceed the three-skill cap");
        let sent = spawn_ready_stub_cli(&mut st, &dir, AgentCliPreset::Codex);
        st.set_composer_draft("Make a pricing card");

        let outcome = st.send_to_agent("Make a pricing card", None);

        assert_eq!(outcome, SendOutcome::Refused(RefuseReason::SkillLimit));
        assert_eq!(st.composer_draft(), "Make a pricing card", "a refused send keeps the user's text");
        assert!(st.active_composer().notice.as_deref().is_some_and(|n| n.contains("skills")), "the notice names the limit: {:?}", st.active_composer().notice);
        assert!(sent.lock().unwrap().is_empty(), "nothing may reach the CLI");
        assert!(!st.session_dir.join("run.json").exists(), "a refused send is not a run");

        st.shutdown();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_written_send_records_the_attached_skills_and_a_system_send_attaches_none() {
        let dir = temp_composer_dir("skill-run-record");
        let mut st = TerminalState::new_for_session("doc-skill-run", "Skill Send", Some(&dir));
        let selector = write_user_skill(&mut st, &dir, "recorded", "Body for the record.");
        st.skills.toggle_skill(&selector);
        let sent = spawn_ready_stub_cli(&mut st, &dir, AgentCliPreset::Codex);

        assert_eq!(st.send_system_prompt("Image task 1: Remove background.", None), SendOutcome::Written);
        let system_text = String::from_utf8_lossy(&sent.lock().unwrap()).to_string();
        assert!(!system_text.contains("designos-skill:"), "a system prompt carries no skills: {system_text:?}");
        assert!(!st.session_dir.join("run.json").exists(), "a system send does not record a skills run");

        sent.lock().unwrap().clear();
        assert_eq!(st.send_to_agent("Make a pricing card", None), SendOutcome::Written);
        let manifest = crate::skills::run_record::read_run_manifest(&st.session_dir).expect("run.json after a written send");
        assert_eq!(manifest.runs.len(), 1);
        let run = &manifest.runs[0];
        assert_eq!((run.runtime.as_str(), run.session_id.as_str()), ("codex", st.session_id.as_str()));
        assert_eq!(run.tab_id, st.active_tab().expect("tab").id);
        assert_eq!(run.skills.len(), 1);
        assert_eq!(run.skills[0].name, "recorded");
        assert_eq!(run.skills[0].sha256.len(), 64);

        st.shutdown();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_queued_send_attaches_the_skills_chosen_when_the_cli_is_ready() {
        let dir = temp_composer_dir("skill-queued");
        let bin_dir = dir.join("bin");
        let recv = dir.join("grok-recv.txt");
        write_executable_stub(&bin_dir, "grok", &recording_agent_script(&recv));
        let mut st = TerminalState::new_for_session("doc-skill-queued", "Skill Send", Some(&dir));
        st.path_prepend.push(bin_dir);
        st.active_runtime = AgentCliPreset::Grok;
        let selector = write_user_skill(&mut st, &dir, "queued-skill", "Queued skill body.");
        st.skills.toggle_skill(&selector);
        assert!(st.tabs.is_empty());

        assert_eq!(st.send_to_agent("Make a pricing card", None), SendOutcome::Queued);
        assert_eq!(st.active_composer().pending_send.as_deref(), Some("Make a pricing card"), "the queue keeps the user's text, not the skill blocks");

        let received = wait_for_file_containing(&mut st, &recv, "Make a pricing card");
        assert!(received.contains("designos-skill:queued-skill@1.0.0"), "the flushed prompt carries the skill block: {received:?}");
        assert!(received.contains("Queued skill body."), "the flushed prompt carries the skill body: {received:?}");
        let raw = std::fs::read_to_string(st.session_dir.join("run.json")).expect("run.json after the flush");
        assert!(raw.contains("queued-skill"), "the flush records the run: {raw}");

        st.shutdown();
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The skills tests below each start a login shell and a stub CLI. They take turns, so that on a loaded machine they add one more
    /// process burst at a time to the suite instead of several, which would starve the other tests' readiness waits.
    fn one_pty_test_at_a_time() -> std::sync::MutexGuard<'static, ()> {
        static TURN: Mutex<()> = Mutex::new(());
        TURN.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Everything written to the recorded PTY since the last call, as text.
    fn take_written(sent: &Arc<Mutex<Vec<u8>>>) -> String {
        let bytes = std::mem::take(&mut *sent.lock().unwrap());
        String::from_utf8_lossy(&bytes).into_owned()
    }

    /// The `runs` of `<session_dir>/run.json`, oldest first, read loosely the way a consumer of the file would.
    fn run_records(session_dir: &Path) -> Vec<serde_json::Value> {
        let raw = std::fs::read_to_string(session_dir.join("run.json")).unwrap_or_default();
        serde_json::from_str::<serde_json::Value>(&raw).ok().and_then(|v| v.get("runs").and_then(|r| r.as_array().cloned())).unwrap_or_default()
    }

    fn record_skill_names(record: &serde_json::Value) -> Vec<String> {
        record["skills"].as_array().map(|a| a.iter().filter_map(|s| s["name"].as_str().map(str::to_string)).collect()).unwrap_or_default()
    }

    /// Runs the status handler (first) and every skills handler of `event` in the session's `hooks.json` through `sh -c` in the
    /// environment of the active tab, as Claude Code runs a group's handlers, and returns their stdout: Claude receives every
    /// handler's `additionalContext`.
    fn run_session_hook(st: &TerminalState, event: &str) -> String {
        let hooks: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(crate::agent_permissions::claude_hooks_path(st)).expect("hooks.json")).expect("hooks json");
        let handlers = hooks["hooks"][event][0]["hooks"].as_array().expect("handlers");
        let tab_id = st.active_tab().map_or(0, |t| t.id);
        let mut stdout = String::new();
        for (i, handler) in handlers.iter().enumerate() {
            let command = handler["command"].as_str().expect("hook command");
            if i > 0 && !command.contains("skills-context-reply") {
                continue;
            }
            let out = std::process::Command::new("sh").arg("-c").arg(command).env("DESIGNOS_TAB_ID", tab_id.to_string()).output().expect("run hook");
            assert!(out.status.success(), "hook failed: {command}");
            stdout.push_str(&String::from_utf8_lossy(&out.stdout));
        }
        stdout
    }

    /// The skills hook replies: what the Claude prompt hook prints for attached skills, every tab's files together; empty while no
    /// skills wait.
    fn hook_reply(st: &TerminalState) -> String {
        skills_reply_files(&st.session_dir).iter().filter_map(|name| std::fs::read_to_string(st.session_dir.join(name)).ok()).collect::<Vec<_>>().join("\n")
    }

    /// Lets a submitted prompt's skills leave the reply: they stay a moment after its `UserPromptSubmit` was seen.
    fn settle_skills_hook(st: &mut TerminalState) {
        std::thread::sleep(Duration::from_millis(250));
        st.tick_composer();
    }

    struct FailingWriter;

    impl Write for FailingWriter {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::new(std::io::ErrorKind::BrokenPipe, "pty closed"))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn skills_attach_on_the_first_send_and_again_only_when_the_set_changes() {
        let _turn = one_pty_test_at_a_time();
        let dir = temp_composer_dir("skills-once-per-run");
        let mut st = TerminalState::new_for_session("doc-skills-once", "Skill Send", Some(&dir));
        let first = write_user_skill(&mut st, &dir, "first-skill", "First body.");
        st.skills.toggle_skill(&first);
        let sent = spawn_ready_stub_cli(&mut st, &dir, AgentCliPreset::Codex);

        // A send that never reached the PTY is not a run: nothing is recorded, and the set stays undelivered for the retry.
        let working = std::mem::replace(&mut st.active_tab_mut().expect("tab").writer, Box::new(FailingWriter));
        assert_eq!(st.send_to_agent("Make a pricing card", None), SendOutcome::Refused(RefuseReason::NotWritable));
        assert!(run_records(&st.session_dir).is_empty(), "a failed write is not a run");
        assert_eq!(st.active_tab().and_then(|t| t.composer.skills_sent), None);
        st.active_tab_mut().expect("tab").writer = working;

        assert_eq!(st.send_to_agent("Make a pricing card", None), SendOutcome::Written);
        st.flush_pending_enter_now();
        let first_paste = take_written(&sent);
        assert!(first_paste.contains("designos-skill:first-skill@1.0.0"), "the first send of a run carries the skills: {first_paste:?}");

        assert_eq!(st.send_to_agent("Now the footer", None), SendOutcome::Written);
        st.flush_pending_enter_now();
        let second_paste = take_written(&sent);
        assert!(second_paste.contains("Now the footer"), "{second_paste:?}");
        assert!(!second_paste.contains("designos-skill:"), "a second send of the same set must not paste the skills again: {second_paste:?}");
        assert_eq!(run_records(&st.session_dir).len(), 1, "an unchanged set is not a new run");

        // A new set counts as a new run start: all selected skills attach again and the earlier record stays.
        let second = write_user_skill(&mut st, &dir, "second-skill", "Second body.");
        st.skills.toggle_skill(&second);
        assert_eq!(st.send_to_agent("And the header", None), SendOutcome::Written);
        st.flush_pending_enter_now();
        let third_paste = take_written(&sent);
        assert!(third_paste.contains("designos-skill:first-skill@1.0.0") && third_paste.contains("designos-skill:second-skill@1.0.0"), "{third_paste:?}");
        let runs = run_records(&st.session_dir);
        assert_eq!(runs.len(), 2, "a set change appends a run: {runs:?}");
        assert_eq!(record_skill_names(&runs[0]), ["first-skill"], "the earlier run keeps its record: {runs:?}");
        assert_eq!(record_skill_names(&runs[1]), ["first-skill", "second-skill"], "{runs:?}");

        // Editing a selected skill changes its sha256, so that is a new set as well.
        write_user_skill(&mut st, &dir, "second-skill", "Second body, edited.");
        assert_eq!(st.send_to_agent("And the footer again", None), SendOutcome::Written);
        st.flush_pending_enter_now();
        let fourth_paste = take_written(&sent);
        assert!(fourth_paste.contains("Second body, edited."), "{fourth_paste:?}");
        assert_eq!(run_records(&st.session_dir).len(), 3);

        st.shutdown();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn claude_hook_reply_carries_the_skills_until_the_first_prompt_is_submitted() {
        let _turn = one_pty_test_at_a_time();
        let dir = temp_composer_dir("skills-claude-hook-life");
        let mut st = TerminalState::new_for_session("doc-skills-hook-life", "Skill Send", Some(&dir));
        let selector = write_user_skill(&mut st, &dir, "claude-once", "Keep gutters on the 8px grid.");
        st.skills.toggle_skill(&selector);
        let sent = spawn_ready_stub_cli(&mut st, &dir, AgentCliPreset::Claude);

        // Choosing a skill attaches nothing by itself, and an image job (system prompt) never carries one.
        assert!(!hook_reply(&st).contains("designos-skill:"), "the reply must not carry skills before a composer send: {}", hook_reply(&st));
        assert_eq!(st.send_system_prompt("Image task 1: Remove background.", None), SendOutcome::Written);
        st.flush_pending_enter_now();
        assert!(!hook_reply(&st).contains("designos-skill:"), "a system prompt must not arm the skills: {}", hook_reply(&st));
        assert!(run_records(&st.session_dir).is_empty(), "a system send is not a run");
        let _ = take_written(&sent);

        // The first composer send puts them in the reply, and the prompt hook hands them to Claude.
        assert_eq!(st.send_to_agent("Make a pricing card", None), SendOutcome::Written);
        assert!(hook_reply(&st).contains("designos-skill:claude-once@1.0.0"), "the first composer send arms the skills: {}", hook_reply(&st));
        assert!(!take_written(&sent).contains("designos-skill:"), "Claude never gets the skills in the pasted prompt");
        st.flush_pending_enter_now();
        // A run's first prompt also gets the bootstrap block instead of the page reply; the skills are a handler of their own and
        // arrive next to it, not inside a reply the first-turn branch would skip.
        let bootstrap_reply = r#"{"hookSpecificOutput":{"hookEventName":"UserPromptSubmit","additionalContext":"BOOTSTRAP-BLOCK"}}"#;
        std::fs::write(st.session_dir.join("bootstrap-context-reply.json"), bootstrap_reply).expect("bootstrap reply");
        let first_prompt_hook = run_session_hook(&st, "UserPromptSubmit");
        assert!(first_prompt_hook.contains("BOOTSTRAP-BLOCK"), "the first prompt gets the bootstrap block: {first_prompt_hook}");
        assert!(first_prompt_hook.contains("designos-skill:claude-once@1.0.0"), "the first prompt's hook must also return the skills: {first_prompt_hook}");

        // Once that prompt's `UserPromptSubmit` is seen, the skills leave the reply a moment later.
        let _ = st.poll_live_agent_bridge();
        settle_skills_hook(&mut st);
        assert!(!hook_reply(&st).contains("designos-skill:"), "the skills leave the reply after the first prompt: {}", hook_reply(&st));
        assert!(skills_reply_files(&st.session_dir).is_empty(), "no skills waiting, no file");

        assert_eq!(st.send_to_agent("Now the footer", None), SendOutcome::Written);
        st.flush_pending_enter_now();
        let second_prompt_hook = run_session_hook(&st, "UserPromptSubmit");
        assert!(
            !second_prompt_hook.contains("designos-skill:") && !second_prompt_hook.contains("BOOTSTRAP-BLOCK"),
            "a later prompt carries neither: {second_prompt_hook}"
        );
        assert_eq!(run_records(&st.session_dir).len(), 1, "one set, one run record");

        // A new set arms again; if its tab closes before the prompt is submitted, nothing waits for the skills any more.
        let second = write_user_skill(&mut st, &dir, "claude-twice", "Keep icons on a 24px box.");
        st.skills.toggle_skill(&second);
        assert_eq!(st.send_to_agent("And the header", None), SendOutcome::Written);
        assert!(hook_reply(&st).contains("designos-skill:claude-twice@1.0.0"), "a new set arms again: {}", hook_reply(&st));
        assert_eq!(run_records(&st.session_dir).len(), 2);
        st.close_tab(0);
        st.tick_composer();
        assert!(!hook_reply(&st).contains("designos-skill:"), "a closed tab takes its skills out of the reply: {}", hook_reply(&st));

        st.shutdown();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_custom_cli_tab_gets_the_skills_pasted_and_is_not_recorded_as_claude() {
        let _turn = one_pty_test_at_a_time();
        let dir = temp_composer_dir("skills-custom-cli");
        let bin_dir = dir.join("bin");
        let recv = dir.join("custom-recv.txt");
        let stub = write_executable_stub(&bin_dir, "custom-cli-stub", &recording_agent_script(&recv));
        let mut st = TerminalState::new_for_session("doc-skills-custom", "Skill Send", Some(&dir));
        let selector = write_user_skill(&mut st, &dir, "custom-skill", "Custom CLI body.");
        st.skills.toggle_skill(&selector);
        st.spawn_custom_cli_tab("Custom stub", &stub.display().to_string(), None).expect("spawn custom tab");
        wait_for_ready_prompt(&mut st);
        std::thread::sleep(READY_QUIET_BRACKETED * 3);
        {
            let tab = st.active_tab().expect("custom tab");
            assert!(tab.preset == AgentCliPreset::Shell && tab.spawned_command.is_some(), "this is the \"Other CLI…\" shape");
        }
        let sent = attach_recording_writer(st.active_tab_mut().expect("custom tab"));

        assert_eq!(st.send_to_agent("Make a pricing card", None), SendOutcome::Written);

        let text = take_written(&sent);
        assert!(
            text.contains("```designos-skill:custom-skill@1.0.0\n"),
            "a custom CLI never reads Claude's hook, so the fenced skill must be in the paste: {text:?}"
        );
        assert!(text.ends_with("Make a pricing card\x1b[201~"), "{text:?}");
        let runs = run_records(&st.session_dir);
        assert_eq!(runs.len(), 1, "{runs:?}");
        assert_ne!(runs[0]["runtime"].as_str(), Some("claude"), "a custom CLI must not be recorded as claude: {runs:?}");
        assert!(!hook_reply(&st).contains("designos-skill:"), "nothing waits in the Claude hook reply for a custom CLI: {}", hook_reply(&st));
        let delay = st.active_tab().and_then(|t| t.composer.pending_enter_delay).expect("pending Enter delay");
        assert!(delay >= OTHER_CLI_BASE_ENTER_DELAY, "a custom CLI is a non-Claude CLI for the Enter delay: {delay:?}");

        st.shutdown();
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Every skills handler of the session's `UserPromptSubmit` hook, run through `sh -c` in the environment of Claude tab `tab_id`,
    /// as Claude Code runs them. Returns the `additionalContext` each one printed; a handler that prints nothing adds nothing.
    fn skills_hook_contexts(st: &TerminalState, tab_id: usize) -> Vec<String> {
        let hooks: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(crate::agent_permissions::claude_hooks_path(st)).expect("hooks.json")).expect("hooks json");
        let handlers = hooks["hooks"]["UserPromptSubmit"][0]["hooks"].as_array().expect("handlers");
        handlers
            .iter()
            .filter_map(|h| h["command"].as_str().filter(|c| c.contains("skills-context-reply")))
            .filter_map(|command| {
                let out = std::process::Command::new("sh").arg("-c").arg(command).env("DESIGNOS_TAB_ID", tab_id.to_string()).output().expect("run hook");
                assert!(out.status.success(), "hook failed: {command}");
                let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
                if stdout.trim().is_empty() {
                    return None;
                }
                let reply: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|e| panic!("hook output is not JSON ({e}): {stdout}"));
                Some(reply["hookSpecificOutput"]["additionalContext"].as_str().expect("additionalContext").to_string())
            })
            .collect()
    }

    /// Names of the skills hook reply files in `dir`, whatever their per-tab or per-part naming.
    fn skills_reply_files(dir: &Path) -> Vec<String> {
        crate::skills::hook_delivery::reply_file_names(dir)
    }

    #[test]
    fn a_12_kb_skill_reaches_claude_whole_in_hook_replies_that_each_stay_under_the_cap() {
        let _turn = one_pty_test_at_a_time();
        let dir = temp_composer_dir("skills-claude-12kb");
        let mut st = TerminalState::new_for_session("doc-skills-12kb", "Skill Send", Some(&dir));
        let body = (0..175).map(|i| format!("Rule {i:03}: keep spacing on the 8px grid and align icons to a 24px box.")).collect::<Vec<_>>().join("\n");
        assert!(body.chars().count() > 12_000);
        let selector = write_user_skill(&mut st, &dir, "big-skill", &body);
        st.skills.toggle_skill(&selector);
        assert!(st.skills.can_start_run(), "12 KB is about 3,000 tokens, inside the 8,000 token limit");
        let _sent = spawn_ready_stub_cli(&mut st, &dir, AgentCliPreset::Claude);
        let tab_id = st.active_tab().expect("tab").id;
        let bootstrap_reply = r#"{"hookSpecificOutput":{"hookEventName":"UserPromptSubmit","additionalContext":"BOOTSTRAP-BLOCK"}}"#;
        std::fs::write(st.session_dir.join("bootstrap-context-reply.json"), bootstrap_reply).expect("bootstrap reply");

        assert_eq!(st.send_to_agent("Make a pricing card", None), SendOutcome::Written);

        // Claude Code caps each hook `additionalContext` at 10,000 characters and gives Claude only a 2,000 character preview of
        // a longer one, so every handler's reply must stay under the cap and the replies together must carry the whole body.
        let contexts = skills_hook_contexts(&st, tab_id);
        for context in &contexts {
            assert!(context.chars().count() <= 9_500, "a hook reply must stay under the cap: {} characters", context.chars().count());
        }
        assert!(contexts.len() >= 2, "a 12 KB skill needs more than one reply: {} replies", contexts.len());
        let mut parts: Vec<(usize, String)> = Vec::new();
        for context in &contexts {
            let lines: Vec<&str> = context.lines().collect();
            let (open, close) = (lines[0], *lines.last().expect("closing fence"));
            assert!(open.starts_with("```designos-skill:big-skill@1.0.0 part "), "a split skill names its part: {open}");
            assert_eq!(close, "```", "each part closes its own fence");
            let (index, of) = open.rsplit_once(" part ").and_then(|(_, p)| p.split_once('/')).expect("part i/n");
            assert_eq!(of.parse::<usize>().ok(), Some(contexts.len()), "n names how many parts there are: {open}");
            parts.push((index.parse().expect("part index"), lines[1..lines.len() - 1].join("\n")));
        }
        parts.sort();
        let whole = parts.iter().map(|(_, text)| text.as_str()).collect::<Vec<_>>().join("\n");
        assert_eq!(whole, body, "every byte of the skill reaches Claude");

        // The bootstrap handler is untouched: the first prompt still gets the bootstrap block, and it never carries skills.
        let hooks: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(crate::agent_permissions::claude_hooks_path(&st)).expect("hooks.json")).expect("hooks json");
        let first_handler = hooks["hooks"]["UserPromptSubmit"][0]["hooks"][0]["command"].as_str().expect("prompt handler").to_string();
        let out = std::process::Command::new("sh").arg("-c").arg(&first_handler).output().expect("run prompt hook");
        let printed = String::from_utf8_lossy(&out.stdout).into_owned();
        assert!(printed.contains("BOOTSTRAP-BLOCK") && !printed.contains("designos-skill:"), "{printed}");

        st.shutdown();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn skills_reply_files_are_deleted_when_the_terminal_shuts_down() {
        let _turn = one_pty_test_at_a_time();
        let dir = temp_composer_dir("skills-reply-shutdown");
        let mut st = TerminalState::new_for_session("doc-skills-shutdown", "Skill Send", Some(&dir));
        let selector = write_user_skill(&mut st, &dir, "armed-skill", "Keep gutters on the 8px grid.");
        st.skills.toggle_skill(&selector);
        let _sent = spawn_ready_stub_cli(&mut st, &dir, AgentCliPreset::Claude);
        assert_eq!(st.send_to_agent("Make a pricing card", None), SendOutcome::Written);
        let session_dir = st.session_dir.clone();
        assert!(!skills_reply_files(&session_dir).is_empty(), "the send arms the skills reply");

        // Closing the document shuts its terminal down before the first prompt was submitted: nothing may stay for a later prompt.
        st.shutdown();

        assert_eq!(skills_reply_files(&session_dir), Vec::<String>::new(), "a stale reply would reach every later Claude prompt of this file");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_skills_reply_left_by_an_earlier_run_is_deleted_when_the_file_is_bound() {
        let dir = temp_composer_dir("skills-reply-bind");
        let session_dir = dir.join("agent-sessions").join("doc-skills-bind");
        std::fs::create_dir_all(&session_dir).expect("session dir");
        for stale in ["skills-context-reply.json", "skills-context-reply-3-0.json", "skills-context-reply-3-1.json"] {
            std::fs::write(session_dir.join(stale), "{}").expect("stale reply");
        }
        std::fs::write(session_dir.join("bootstrap-context-reply.json"), "{}").expect("other reply");

        let st = TerminalState::new_for_session("doc-skills-bind", "Skill Send", Some(&dir));

        assert!(st.session_dir.join("bootstrap-context-reply.json").exists(), "only the skills replies go; the test bound the folder it planted in");
        assert_eq!(skills_reply_files(&st.session_dir), Vec::<String>::new(), "a reply left by a quit before the hook settled must not reach this run");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn another_claude_tab_never_receives_the_skills_armed_for_a_tab() {
        let _turn = one_pty_test_at_a_time();
        let dir = temp_composer_dir("skills-per-tab");
        let mut st = TerminalState::new_for_session("doc-skills-per-tab", "Skill Send", Some(&dir));
        let selector = write_user_skill(&mut st, &dir, "tab-a-skill", "Keep icons on a 24px box.");
        st.skills.toggle_skill(&selector);
        let _sent = spawn_ready_stub_cli(&mut st, &dir, AgentCliPreset::Claude);
        let tab_a = st.active_tab().expect("tab").id;
        assert_eq!(st.send_to_agent("Make a pricing card", None), SendOutcome::Written);

        let for_a = skills_hook_contexts(&st, tab_a).join("\n");
        assert!(for_a.contains("designos-skill:tab-a-skill@1.0.0"), "the tab the skills were attached to receives them: {for_a}");
        let for_b = skills_hook_contexts(&st, tab_a + 1);
        assert!(for_b.is_empty(), "a prompt submitted in another Claude tab must not receive tab A's skills: {for_b:?}");

        st.shutdown();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn two_armed_claude_tabs_each_get_only_their_own_chunks_under_the_id_their_process_exports() {
        let _turn = one_pty_test_at_a_time();
        let dir = temp_composer_dir("skills-two-tabs");
        let bin_dir = dir.join("bin");
        let ids_file = dir.join("tab-ids.txt");
        write_executable_stub(&bin_dir, "claude", &format!("printf '%s\\n' \"$DESIGNOS_TAB_ID\" >> '{}'\nsleep 30", ids_file.display()));
        let mut st = TerminalState::new_for_session("doc-skills-two-tabs", "Skill Send", Some(&dir));
        st.path_prepend.push(bin_dir);
        st.spawn_tab(AgentCliPreset::Claude, None).expect("spawn first claude");
        st.spawn_tab(AgentCliPreset::Claude, None).expect("spawn second claude");
        let ids: Vec<usize> = st.tabs.iter().map(|t| t.id).collect();
        assert_ne!(ids[0], ids[1]);

        // The hook handlers read the tab id from the environment of the CLI process: it must be the id the files are named after.
        crate::test_timing::poll_until(crate::test_timing::STUB_WAIT, || std::fs::read_to_string(&ids_file).is_ok_and(|raw| raw.lines().count() >= 2));
        let mut exported: Vec<usize> = std::fs::read_to_string(&ids_file).unwrap_or_default().lines().filter_map(|l| l.parse().ok()).collect();
        exported.sort_unstable();
        assert_eq!(exported, ids, "each Claude tab exports DESIGNOS_TAB_ID = its tab id");

        for (tab, chunks) in st.tabs.iter_mut().zip([vec!["one-a", "one-b"], vec!["two-a"]]) {
            tab.composer.skills_hook_chunks = chunks.into_iter().map(str::to_string).collect();
        }
        st.sync_hook_reply();
        assert_eq!(skills_hook_contexts(&st, ids[0]), ["one-a", "one-b"], "tab one gets its own chunks, in slot order");
        assert_eq!(skills_hook_contexts(&st, ids[1]), ["two-a"], "tab two gets its own chunk and not tab one's");

        // A shorter set for tab one leaves no slot of the longer one behind; tab two is untouched.
        st.tabs[0].composer.skills_hook_chunks = vec!["one-only".to_string()];
        st.sync_hook_reply();
        assert_eq!(skills_hook_contexts(&st, ids[0]), ["one-only"]);
        assert_eq!(skills_hook_contexts(&st, ids[1]), ["two-a"]);

        // Closing tab one takes only its files away.
        st.close_tab(0);
        st.sync_hook_reply();
        assert!(skills_hook_contexts(&st, ids[0]).is_empty(), "a closed tab's chunks are gone");
        assert_eq!(skills_hook_contexts(&st, ids[1]), ["two-a"]);

        st.shutdown();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_set_the_hook_cannot_carry_is_pasted_into_claude_with_the_size_scaled_enter_delay() {
        let _turn = one_pty_test_at_a_time();
        let dir = temp_composer_dir("skills-claude-paste-fallback");
        let mut st = TerminalState::new_for_session("doc-skills-fallback", "Skill Send", Some(&dir));
        // Nothing but backticks needs a fence as long as the body: within the picker's 8,000 token limit, past what the hook slots hold.
        let selector = write_user_skill(&mut st, &dir, "fence-heavy", &"`".repeat(crate::skills::MAX_COMBINED_TOKENS * crate::skills::CHARS_PER_TOKEN));
        st.skills.toggle_skill(&selector);
        assert!(st.skills.can_start_run());
        assert_eq!(st.skills.hook_chunks(), None, "the hook cannot carry this set");
        let sent = spawn_ready_stub_cli(&mut st, &dir, AgentCliPreset::Claude);

        assert_eq!(st.send_to_agent("Make a pricing card", None), SendOutcome::Written);

        let text = take_written(&sent);
        assert!(text.contains("designos-skill:fence-heavy@1.0.0\n"), "the skills are pasted, not dropped: {} bytes", text.len());
        assert!(text.ends_with("Make a pricing card\x1b[201~"));
        assert!(skills_reply_files(&st.session_dir).is_empty(), "nothing waits in the hook replies for a pasted set");
        let runs = run_records(&st.session_dir);
        assert_eq!(runs.len(), 1, "{runs:?}");
        assert_eq!(runs[0]["runtime"].as_str(), Some("claude"));
        assert_eq!(record_skill_names(&runs[0]), ["fence-heavy"]);
        let delay = st.active_tab().and_then(|t| t.composer.pending_enter_delay).expect("pending Enter delay");
        assert!(delay >= OTHER_CLI_BASE_ENTER_DELAY, "a big paste waits as long as for any other CLI: {delay:?}");

        st.shutdown();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_send_without_skills_selected_records_no_run_until_a_skill_is_chosen() {
        let _turn = one_pty_test_at_a_time();
        let dir = temp_composer_dir("skills-no-run");
        let mut st = TerminalState::new_for_session("doc-skills-no-run", "Skill Send", Some(&dir));
        assert!(st.skills.selected_entries().is_empty(), "no skill is chosen");
        let sent = spawn_ready_stub_cli(&mut st, &dir, AgentCliPreset::Codex);

        assert_eq!(st.send_to_agent("Make a pricing card", None), SendOutcome::Written);
        st.flush_pending_enter_now();
        assert!(take_written(&sent).contains("Make a pricing card"));
        assert!(!st.session_dir.join("run.json").exists(), "nothing was attached, so there is no run to record");

        // Choosing a skill afterwards is a new set: it attaches with the next send and that run is recorded.
        let selector = write_user_skill(&mut st, &dir, "later-skill", "Keep gutters on the 8px grid.");
        st.skills.toggle_skill(&selector);
        assert_eq!(st.send_to_agent("And the footer", None), SendOutcome::Written);
        st.flush_pending_enter_now();
        assert!(take_written(&sent).contains("designos-skill:later-skill@1.0.0"));
        let runs = run_records(&st.session_dir);
        assert_eq!(runs.len(), 1, "{runs:?}");
        assert_eq!(record_skill_names(&runs[0]), ["later-skill"], "{runs:?}");

        st.shutdown();
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Runs one `show_left` frame of `st` in a 1200x800 window with the panel at x=48, width 360 (a typical Agents panel), feeding `events`.
    fn panel_frame(ctx: &egui::Context, st: &mut TerminalState, events: Vec<Event>) {
        let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1200.0, 800.0))), events, ..Default::default() };
        let panel_rect = Rect::from_min_size(Pos2::new(48.0, 0.0), Vec2::new(360.0, 720.0));
        let _ = ctx.run_ui(raw, |ui| {
            let _ = show_left(st, ui, panel_rect, None);
        });
    }

    fn drawn_rect(ctx: &egui::Context, name: &str) -> Option<Rect> {
        ctx.data(|d| d.get_temp(Id::new(("test-rect", name.to_string()))))
    }

    fn click_events(rect: Rect) -> Vec<Event> {
        vec![
            Event::PointerMoved(rect.center()),
            Event::PointerButton { pos: rect.center(), button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE },
            Event::PointerButton { pos: rect.center(), button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE },
        ]
    }

    fn themed_context() -> egui::Context {
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        crate::theme::apply(&ctx);
        ctx
    }

    #[test]
    fn the_send_button_is_disabled_not_just_red_over_the_skill_limit() {
        let ctx = themed_context();
        let dir = temp_composer_dir("skills-send-disabled");
        let mut st = TerminalState::new_for_session("doc-skills-send-disabled", "Skill Send", Some(&dir));
        for builtin in ["design-system-rhythm", "accessibility-wcag", "component-craft"] {
            st.skills.toggle_skill(builtin);
        }
        let extra = write_user_skill(&mut st, &dir, "one-too-many", "Extra body.");
        st.skills.toggle_skill(&extra);
        assert!(!st.skills.can_start_run());
        st.set_composer_draft("Make a pricing card");

        panel_frame(&ctx, &mut st, Vec::new());
        let send = drawn_rect(&ctx, "agents-composer-send").expect("send button");
        panel_frame(&ctx, &mut st, click_events(send));
        assert_eq!(st.active_composer().last_outcome, None, "a disabled Send does not even try to send: {:?}", st.active_composer().last_outcome);
        assert_eq!(st.composer_draft(), "Make a pricing card");

        // Back within the limit, the same click sends (into the starting agent's queue).
        st.skills.toggle_skill(&extra);
        assert!(st.skills.can_start_run());
        panel_frame(&ctx, &mut st, Vec::new());
        let send = drawn_rect(&ctx, "agents-composer-send").expect("send button");
        panel_frame(&ctx, &mut st, click_events(send));
        assert!(st.active_composer().last_outcome.is_some(), "an enabled Send sends or queues");

        st.shutdown();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_agents_header_has_a_28px_skills_button_beside_the_runtime_selector_that_opens_the_picker() {
        let _turn = one_pty_test_at_a_time();
        let ctx = themed_context();
        let dir = temp_composer_dir("skills-header-button");
        let mut st = TerminalState::new_for_session("doc-skills-header", "Skill Send", Some(&dir));
        assert!(st.side_panel_pty_open, "the default Agents panel is the PTY view with the `+` runtime selector");

        panel_frame(&ctx, &mut st, Vec::new());
        let add = drawn_rect(&ctx, "agents-runtime-add-btn").expect("runtime selector");
        let skills = drawn_rect(&ctx, "agents-header-skills").expect("header Skills button");
        assert_eq!(skills.height(), theme::CONTROL_STANDARD, "RD-4 §7: a 28 px button");
        assert!(skills.left() >= add.right() && skills.left() - add.right() <= theme::SPACE_GROUP, "next to the runtime selector: {add:?} {skills:?}");
        assert!((skills.center().y - add.center().y).abs() < 1.0, "same row as the selector: {add:?} {skills:?}");
        assert!(!st.skills.picker_open);

        panel_frame(&ctx, &mut st, click_events(skills));
        assert!(st.skills.picker_open, "the button opens the same picker as the composer chip");
        panel_frame(&ctx, &mut st, Vec::new());
        assert!(drawn_rect(&ctx, "agents-skills-picker-popover").is_some(), "the picker is drawn");

        // The strip makes room for it: with a tab open the button still sits right of `+` and inside the panel (`panel_frame`: x 48..408).
        st.spawn_tab(AgentCliPreset::Shell, None).expect("spawn tab");
        panel_frame(&ctx, &mut st, Vec::new());
        let add = drawn_rect(&ctx, "agents-runtime-add-btn").expect("runtime selector");
        let skills = drawn_rect(&ctx, "agents-header-skills").expect("header Skills button");
        assert!(skills.left() >= add.right() && skills.right() <= 48.0 + 360.0, "{add:?} {skills:?}");

        st.shutdown();
        let _ = std::fs::remove_dir_all(&dir);
    }
}
