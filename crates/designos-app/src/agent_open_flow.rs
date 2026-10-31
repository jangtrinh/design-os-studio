//! Agent open flow (`#651` / `AX-651`): resume the file's most recent agent session or open the
//! new-session picker (`#652`), never auto-spawning a login shell on open, plus the `"Sessions"`
//! ghost button and popover for switching, renaming in place, and removing recorded file sessions.

use std::path::Path;

use egui::{Align2, Id, Key, Painter, Pos2, Rect, RichText, Sense, Stroke, StrokeKind, TextEdit, Ui, Vec2};

use crate::agent_sessions::{preset_from_cli_name, save_registry, unix_timestamp_secs, SessionEntryStatus};
use crate::terminal::{AgentCliPreset, AgentTabStatus, TerminalState, NEEDS_APPROVAL_LABEL, NEEDS_INPUT_LABEL};
use crate::theme;
use crate::widgets;

const STATUS_DOT_X: f32 = 8.0;
const PROVIDER_DOT_X: f32 = 16.0;
const TITLE_LEFT_X: f32 = 24.0;
const ACTION_BTN_H: f32 = 20.0;
/// Row actions of the Sessions popover. Their buttons are as wide as the label plus the segment inset on each side.
const RENAME_ACTION_LABEL: &str = "Rename";
const REMOVE_ACTION_LABEL: &str = "Remove from list";
const POPOVER_MIN_W: f32 = 216.0;
const POPOVER_MAX_W: f32 = 320.0;
const POPOVER_MIN_INNER_W: f32 = 140.0;
const RENAME_FIELD_MIN_W: f32 = 64.0;
const STATUS_SPINNER_RADIUS: f32 = 3.2;
const STATUS_WAIT_RADIUS: f32 = 3.1;
const STATUS_WAIT_INNER_RADIUS: f32 = 1.2;
const STATUS_DONE_RADIUS: f32 = 2.7;
const PROVIDER_BADGE_RADIUS: f32 = 2.3;
const GROK_SLASH_DELTA: f32 = 2.0;

/// Status shown for a row in the `"Sessions"` popover (`#651` §3 rule 5).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SessionRowStatus {
    /// The session's tab is currently running in Studio; carries its live [`AgentTabStatus`].
    Live(AgentTabStatus),
    /// The session is in the registry (`Active` or `Exited` on disk) with no running tab.
    NotRunning,
    /// A previous resume exited inside the 5-second start-failure window (`#653` rule 8).
    Missing,
}

impl SessionRowStatus {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Live(AgentTabStatus::Working(_)) => "Working…",
            Self::Live(AgentTabStatus::WaitingForInput(Some(detail))) if detail == NEEDS_APPROVAL_LABEL => NEEDS_APPROVAL_LABEL,
            Self::Live(AgentTabStatus::WaitingForInput(_)) => NEEDS_INPUT_LABEL,
            Self::Live(AgentTabStatus::Done) => "Done",
            Self::Live(AgentTabStatus::Idle) => "Idle",
            Self::Live(AgentTabStatus::Error(_)) => "Failed",
            Self::Live(AgentTabStatus::Stopped) => "Stopped",
            Self::NotRunning => "Not running",
            Self::Missing => "Resume failed",
        }
    }
}

/// One row in the `"Sessions for this file"` popover (`#651` §3 rule 5 & §7).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionPopoverRow {
    pub tab_id: u64,
    pub cli: String,
    pub cli_display_name: String,
    pub preset: Option<AgentCliPreset>,
    pub label: String,
    pub last_active_at: u64,
    pub relative_time: String,
    pub status: SessionRowStatus,
    pub live_tab_index: Option<usize>,
    pub is_focused_tab: bool,
}

/// Maps a persisted CLI binary name (`claude`, `codex`, `grok`, `agy`) to its display name.
pub fn cli_display_name(cli: &str) -> String {
    match cli.trim() {
        "claude" => "Claude Code".to_string(),
        "codex" => "Codex".to_string(),
        "grok" => "Grok".to_string(),
        "agy" => "Antigravity".to_string(),
        other => other.to_string(),
    }
}

/// Converts a Unix timestamp in seconds into a UTC `YYYY-MM-DD` string.
pub fn format_utc_date(unix_secs: u64) -> String {
    // Howard Hinnant's civil_from_days algorithm (UTC).
    let z = (unix_secs / 86_400) as i64 + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let yr = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if m <= 2 { yr + 1 } else { yr };
    format!("{year:04}-{m:02}-{d:02}")
}

/// Default session label when the user submits an empty rename (`<CLI> <date>`, `#651` §3 rule 6).
pub fn default_session_label(cli: &str, created_at_secs: u64) -> String {
    format!("{} {}", cli_display_name(cli), format_utc_date(created_at_secs))
}

/// Formats `last_active_at` relative to `now_secs` (`#651` §3 rule 5).
pub fn format_relative_time(last_active_at: u64, now_secs: u64) -> String {
    let diff = now_secs.saturating_sub(last_active_at);
    if diff < 5 {
        "just now".to_string()
    } else if diff < 60 {
        format!("{diff}s ago")
    } else if diff < 3_600 {
        format!("{}m ago", diff / 60)
    } else if diff < 86_400 {
        format!("{}h ago", diff / 3_600)
    } else {
        format!("{}d ago", diff / 86_400)
    }
}

impl crate::app::App {
    /// Shows the Agents panel on the left, without touching its sessions.
    pub fn show_agents_panel(&mut self) {
        self.ui.rail = Some(crate::app::RailItem::Agents);
        self.ui.left_collapsed = false;
    }

    /// What a click on the Agents rail button and the Cmd+` shortcut do: an open Agents panel closes; a closed one opens
    /// and then resumes the file's last session or shows the new-session list ([`TerminalState::open_for_agent_use`]).
    pub fn toggle_agents_panel(&mut self) {
        if self.ui.rail == Some(crate::app::RailItem::Agents) && !self.ui.left_collapsed {
            self.ui.rail = None;
        } else {
            self.show_agents_panel();
            self.terminal.open_for_agent_use(self.file_path.as_deref());
        }
    }
}

impl TerminalState {
    /// True when at least one tab in `self.tabs` has a running process or is still inside the
    /// 5-second startup window without a recorded start failure (`#651` §3 rule 2.1 & §8 edge case 4).
    pub fn has_live_tab(&mut self) -> bool {
        self.tabs.iter_mut().any(|t| {
            t.is_alive() || (t.start_failure.is_none() && !t.exited_notice_shown && t.spawned_at.elapsed() < crate::agent_start_failure::START_FAILURE_WINDOW)
        })
    }

    /// Opens the Agents panel for agent use, from the rail button or Cmd+` (`#651` §3 rule 2):
    /// 1. If a live tab exists, keep it (`picker_open = false`).
    /// 2. Else if `resumable_sessions()` is non-empty, resume the most recent one (`picker_open = false`).
    /// 3. Else start nothing and set `picker_open = true`.
    ///
    /// Never spawns a `Shell` tab.
    pub fn open_for_agent_use(&mut self, file_path: Option<&Path>) {
        if self.has_live_tab() {
            self.picker_open = false;
            return;
        }
        if !self.resumable_sessions().is_empty() && self.resume_last_session(file_path).is_ok() {
            self.picker_open = false;
            return;
        }
        self.picker_open = true;
    }

    /// Builds the ordered row list (newest first) for the `"Sessions"` popover (`#651` §3 rule 5).
    pub fn sessions_popover_rows(&mut self) -> Vec<SessionPopoverRow> {
        let now = unix_timestamp_secs();
        let active_tab_idx = self.active_tab;
        let mut live_by_tab_id: Vec<(u64, Option<String>, usize, AgentTabStatus)> = Vec::new();
        for (idx, tab) in self.tabs.iter_mut().enumerate() {
            let is_live = tab.is_alive()
                || (tab.start_failure.is_none() && !tab.exited_notice_shown && tab.spawned_at.elapsed() < crate::agent_start_failure::START_FAILURE_WINDOW);
            if is_live {
                let explicit = tab.agent_status.lock().ok().map(|g| g.clone()).unwrap_or_default();
                let recent_pty = tab.preset != AgentCliPreset::Shell
                    && tab.last_output_at.lock().ok().and_then(|g| *g).is_some_and(|ts| ts.elapsed() < std::time::Duration::from_millis(600));
                let effective = match explicit {
                    AgentTabStatus::Working(d) => AgentTabStatus::Working(d),
                    // Same predicate as the tab pill: a pending approval reads "Needs approval", any other wait "Needs input".
                    AgentTabStatus::WaitingForInput(_) if tab.needs_approval() => AgentTabStatus::WaitingForInput(Some(NEEDS_APPROVAL_LABEL.to_string())),
                    // A heuristic's text match (Codex, Skip mode) is not a pending approval: the row says `Needs input`.
                    AgentTabStatus::WaitingForInput(_) => AgentTabStatus::WaitingForInput(None),
                    _ if recent_pty => AgentTabStatus::Working(None),
                    other => other,
                };
                live_by_tab_id.push((tab.id as u64, tab.session_uuid.clone(), idx, effective));
            }
        }

        let mut entries = self.session_registry.sessions.clone();
        entries.sort_by(|a, b| (b.last_active_at, b.tab_id).cmp(&(a.last_active_at, a.tab_id)));

        entries
            .into_iter()
            .map(|entry| {
                let matched_live =
                    live_by_tab_id.iter().find(|(tid, suuid, _, _)| *tid == entry.tab_id || (entry.session_id.is_some() && *suuid == entry.session_id));
                let (status, live_tab_index, is_focused_tab) = if let Some((_, _, idx, st)) = matched_live {
                    (SessionRowStatus::Live(st.clone()), Some(*idx), *idx == active_tab_idx)
                } else if entry.status == SessionEntryStatus::Missing {
                    (SessionRowStatus::Missing, None, false)
                } else {
                    (SessionRowStatus::NotRunning, None, false)
                };
                SessionPopoverRow {
                    tab_id: entry.tab_id,
                    cli_display_name: cli_display_name(&entry.cli),
                    preset: preset_from_cli_name(&entry.cli),
                    cli: entry.cli,
                    label: entry.title,
                    last_active_at: entry.last_active_at,
                    relative_time: format_relative_time(entry.last_active_at, now),
                    status,
                    live_tab_index,
                    is_focused_tab,
                }
            })
            .collect()
    }

    /// Selects a row from the `"Sessions"` popover: focuses its running tab if live, or resumes it in a new tab (`#651` §3 rule 5 & §8 edge case 2).
    pub fn select_session_row(&mut self, entry_tab_id: u64, file_path: Option<&Path>) {
        let rows = self.sessions_popover_rows();
        if let Some(row) = rows.into_iter().find(|r| r.tab_id == entry_tab_id) {
            if let Some(live_idx) = row.live_tab_index {
                self.active_tab = live_idx;
                self.picker_open = false;
                self.sessions_popover_open = false;
                self.viewport_focused = true;
                return;
            }
            if row.status != SessionRowStatus::Missing {
                match self.resume_session_by_tab_id(entry_tab_id, file_path) {
                    Ok(_) => {
                        self.picker_open = false;
                        self.sessions_popover_open = false;
                    }
                    // Keep the popover open so the notice shows next to the row that was clicked.
                    Err(e) => self.set_resume_failure(&e),
                }
            }
        }
    }

    /// Records a resume failure as a short visible notice (the raw reason stays in `status_msg`).
    pub fn set_resume_failure(&mut self, raw_error: &str) {
        self.resume_notice = Some(resume_failure_copy(raw_error).to_string());
        self.status_msg = Some(raw_error.to_string());
    }

    /// Drops the resume notice once neither the Sessions popover nor the `+` menu is open, so a closed
    /// surface never leaves a stale error behind (called every frame before those surfaces paint).
    pub fn expire_resume_notice(&mut self) {
        if !self.sessions_popover_open && !self.runtime_dropdown_open {
            self.resume_notice = None;
        }
    }

    /// Begins inline rename for the session entry identified by `entry_tab_id` (`#651` §3 rule 6).
    pub fn start_rename_session(&mut self, entry_tab_id: u64) {
        if let Some(entry) = self.session_registry.sessions.iter().find(|s| s.tab_id == entry_tab_id) {
            self.renaming_session_tab_id = Some(entry_tab_id);
            self.rename_session_input = entry.title.clone();
            self.rename_session_focus_pending = true;
        }
    }

    /// Cancels any active inline session rename (`Esc`, `#651` §7).
    pub fn cancel_rename_session(&mut self) {
        self.renaming_session_tab_id = None;
        self.rename_session_input.clear();
        self.rename_session_focus_pending = false;
    }

    /// Commits the current inline session rename (`Enter`, `#651` §3 rule 6 & §7).
    pub fn commit_rename_session(&mut self) {
        self.rename_session_focus_pending = false;
        if let Some(tab_id) = self.renaming_session_tab_id.take() {
            let input = std::mem::take(&mut self.rename_session_input);
            self.rename_session_entry(tab_id, &input);
        }
    }

    /// Renames `entry_tab_id` in `sessions.json` and on any matching open tab (`#651` §3 rule 6).
    /// Empty or whitespace-only labels revert to `<CLI> <date>`.
    pub fn rename_session_entry(&mut self, entry_tab_id: u64, new_label: &str) {
        let trimmed = new_label.trim();
        let mut updated_label: Option<(String, Option<String>)> = None;
        if let Some(entry) = self.session_registry.sessions.iter_mut().find(|s| s.tab_id == entry_tab_id) {
            let resolved = if trimmed.is_empty() { default_session_label(&entry.cli, entry.created_at) } else { trimmed.to_string() };
            entry.title = resolved.clone();
            updated_label = Some((resolved, entry.session_id.clone()));
            let _ = save_registry(&self.session_dir, &self.session_registry);
        }
        if let Some((resolved, session_uuid)) = updated_label {
            for tab in &mut self.tabs {
                if tab.id as u64 == entry_tab_id || (session_uuid.is_some() && tab.session_uuid == session_uuid) {
                    tab.title = resolved.clone();
                }
            }
        }
    }

    /// Forgets Studio's registry entry for `entry_tab_id` while leaving the CLI's own conversation store untouched (`#651` §3 rule 6).
    pub fn remove_session_from_list(&mut self, entry_tab_id: u64) {
        let before = self.session_registry.sessions.len();
        self.session_registry.sessions.retain(|s| s.tab_id != entry_tab_id);
        if self.renaming_session_tab_id == Some(entry_tab_id) {
            self.cancel_rename_session();
        }
        if self.session_registry.sessions.len() != before {
            let _ = save_registry(&self.session_dir, &self.session_registry);
        }
    }
}

/// Maps a resume error from `terminal.rs` to one short sentence with a recovery. Each string must fit the
/// narrowest `+` menu footer (see `resume_notice_copy_fits_the_narrowest_menus_without_eliding`).
pub fn resume_failure_copy(raw_error: &str) -> &'static str {
    if raw_error.starts_with("No previous agent session") {
        "No earlier session. Start a new one."
    } else if raw_error.starts_with("No resumable session") {
        "Session is gone. Start a new one."
    } else if raw_error.contains("safe session resume") || raw_error.starts_with("Unsupported CLI") {
        "Can't resume. Start a new session."
    } else {
        "Resume failed. Start a new session."
    }
}

/// Paints the status dot and CLI provider badge inside a `"Sessions"` popover row (`#651` §7).
fn paint_session_row_indicators(ui: &Ui, p: &Painter, status: &SessionRowStatus, preset: Option<AgentCliPreset>, row_left: f32, cy: f32) {
    let status_center = Pos2::new(row_left + STATUS_DOT_X, cy);
    let provider_center = Pos2::new(row_left + PROVIDER_DOT_X, cy);

    match status {
        SessionRowStatus::Live(AgentTabStatus::Working(_)) => {
            let step = ((ui.input(|i| i.time) * 12.0).floor() as i32).rem_euclid(12);
            let base_angle = (step as f32) * (std::f32::consts::TAU / 12.0);
            let arc_span = std::f32::consts::TAU * 0.68;
            let spin_col = theme::INFO;
            let segs = 7usize;
            let mut prev = Pos2::new(status_center.x + STATUS_SPINNER_RADIUS * base_angle.cos(), status_center.y + STATUS_SPINNER_RADIUS * base_angle.sin());
            for i in 1..=segs {
                let a = base_angle + arc_span * (i as f32 / segs as f32);
                let next = Pos2::new(status_center.x + STATUS_SPINNER_RADIUS * a.cos(), status_center.y + STATUS_SPINNER_RADIUS * a.sin());
                p.line_segment([prev, next], Stroke::new(theme::BORDER_WIDTH, spin_col));
                prev = next;
            }
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(83));
        }
        SessionRowStatus::Live(AgentTabStatus::WaitingForInput(_)) => {
            let wait_col = theme::ICON_BRAND_PRIMARY;
            p.circle_stroke(status_center, STATUS_WAIT_RADIUS, Stroke::new(theme::BORDER_WIDTH, wait_col));
            p.circle_filled(status_center, STATUS_WAIT_INNER_RADIUS, wait_col);
        }
        SessionRowStatus::Live(AgentTabStatus::Done | AgentTabStatus::Idle) => {
            p.circle_filled(status_center, STATUS_DONE_RADIUS, theme::SUCCESS);
        }
        SessionRowStatus::Live(AgentTabStatus::Error(_)) => {
            p.circle_filled(status_center, STATUS_DONE_RADIUS, theme::DANGER);
        }
        SessionRowStatus::Live(AgentTabStatus::Stopped) => {
            p.circle_filled(status_center, STATUS_DONE_RADIUS, theme::ICON_TERTIARY);
        }
        SessionRowStatus::NotRunning => {
            p.circle_stroke(status_center, STATUS_DONE_RADIUS, Stroke::new(theme::BORDER_WIDTH, theme::TEXT_TERTIARY));
        }
        SessionRowStatus::Missing => {
            p.circle_filled(status_center, STATUS_DONE_RADIUS, theme::TEXT_DISABLED);
        }
    }

    match preset.unwrap_or(AgentCliPreset::Shell) {
        AgentCliPreset::Claude => {
            p.circle_filled(provider_center, PROVIDER_BADGE_RADIUS, theme::PROVIDER_CLAUDE);
        }
        AgentCliPreset::Codex | AgentCliPreset::OpenCode => {
            p.circle_stroke(provider_center, PROVIDER_BADGE_RADIUS, Stroke::new(theme::BORDER_WIDTH, theme::PROVIDER_CODEX));
        }
        AgentCliPreset::Grok => {
            p.line_segment(
                [
                    Pos2::new(provider_center.x - GROK_SLASH_DELTA, provider_center.y + GROK_SLASH_DELTA),
                    Pos2::new(provider_center.x + GROK_SLASH_DELTA, provider_center.y - GROK_SLASH_DELTA),
                ],
                Stroke::new(theme::BORDER_WIDTH, theme::PROVIDER_GROK),
            );
        }
        AgentCliPreset::Antigravity | AgentCliPreset::Gemini => {
            p.circle_filled(provider_center, PROVIDER_BADGE_RADIUS, theme::PROVIDER_GEMINI);
        }
        AgentCliPreset::Shell | AgentCliPreset::DesignOsHelp => {
            p.circle_filled(provider_center, PROVIDER_BADGE_RADIUS, theme::ICON_TERTIARY);
        }
    }
}

/// One footer row of the sessions popover. Returns whether it was clicked this frame.
fn sessions_footer_item(ui: &mut Ui, p: &Painter, rect: Rect, key: &str, label: &str, caption: Option<&str>) -> bool {
    let resp = ui.interact(rect, Id::new(key), Sense::click());
    let paint = widgets::kit::menu_row_look(false, resp.hovered(), false);
    if paint.fill.a() != 0 {
        p.rect_filled(rect, theme::cr(theme::RADIUS_CONTROL), paint.fill);
    }
    let font = theme::font_text(theme::FONT_SIZE_TEXT_XS, theme::TextWeight::Regular);
    p.text(Pos2::new(rect.left() + theme::INSET_FRAME, rect.center().y), Align2::LEFT_CENTER, label, font.clone(), paint.text);
    if let Some(caption) = caption {
        p.text(Pos2::new(rect.right() - theme::INSET_FRAME, rect.center().y), Align2::RIGHT_CENTER, caption, font, theme::TEXT_TERTIARY);
    }
    widgets::record(ui, key, rect);
    widgets::labelled(ui, Id::new(key), egui::accesskit::Role::Button, label);
    resp.clicked()
}

/// Renders the `"Sessions for this file"` popover (`#651` §3 rules 5-6 & §7).
pub fn render_sessions_popover(state: &mut TerminalState, ui: &mut Ui, panel_rect: Rect, top_y: f32, file_path: Option<&Path>) {
    let rows = state.sessions_popover_rows();
    let pad = theme::INSET_POPOVER;
    let row_h = theme::CONTROL_MD;
    let row_gap = theme::SPACE_FACT;
    let header_h = theme::FIELD_HEIGHT;
    let body_count = rows.len().max(1) as f32;
    let running = crate::agent_stop::running_tab_count(state);
    let show_stop_all = running >= 2;
    let footer_rows = if show_stop_all { 2.0 } else { 1.0 };
    let footer_h = theme::SPACE_ROW + footer_rows * theme::MENU_ITEM_HEIGHT;
    let pop_w = (panel_rect.width() - pad).clamp(POPOVER_MIN_W, POPOVER_MAX_W);
    let p = ui.painter().clone();
    let inner_w = (pop_w - pad * 2.0).max(POPOVER_MIN_INNER_W);
    // A failed-resume notice is wrapped, never elided: when one line is too short it takes a second and the popover grows.
    let notice = state.resume_notice.as_deref().map(|text| p.layout(text.to_string(), theme::font_caption(), theme::DANGER, inner_w - theme::INSET_CONTROL));
    let notice_h = notice.as_ref().map_or(0.0, |galley| header_h.max(galley.size().y + theme::SPACE_FACT * 2.0));
    let pop_h = pad * 2.0 + header_h + theme::SPACE_FACT + body_count * (row_h + row_gap) + notice_h + footer_h;
    let pop_x = (panel_rect.right() - pop_w - theme::SPACE_ROW).max(panel_rect.left() + theme::SPACE_ROW);
    let pop_r = Rect::from_min_size(Pos2::new(pop_x, top_y), Vec2::new(pop_w, pop_h));

    widgets::kit::popover_surface(&p, pop_r);
    widgets::record(ui, "agents-sessions-popover", pop_r);

    let inner_left = pop_r.left() + pad;
    let inner_right = pop_r.right() - pad;

    let hdr = widgets::elide(&p, "Sessions for this file", &theme::font_caption_medium(), inner_w);
    p.text(
        Pos2::new(inner_left, pop_r.top() + pad + header_h * 0.5 - theme::SPACE_FACT),
        Align2::LEFT_CENTER,
        hdr,
        theme::font_caption_medium(),
        theme::TEXT_SECONDARY,
    );
    let hline_y = pop_r.top() + pad + header_h;
    widgets::hline(&p, inner_left, inner_right, hline_y, theme::BORDER);

    let mut y = hline_y + theme::SPACE_FACT;
    let top_line_offset = theme::INSET_POPOVER;
    let bottom_line_offset = theme::INSET_CONTROL + theme::SPACE_FACT;
    let action_half_h = ACTION_BTN_H * 0.5;

    if rows.is_empty() {
        let empty_r = Rect::from_min_size(Pos2::new(inner_left, y), Vec2::new(inner_w, row_h));
        let empty_txt = widgets::elide(&p, "No recorded sessions for this file", &theme::font_caption(), inner_w);
        p.text(
            Pos2::new(empty_r.left() + theme::INSET_CONTROL, empty_r.center().y),
            Align2::LEFT_CENTER,
            empty_txt,
            theme::font_caption(),
            theme::TEXT_TERTIARY,
        );
        widgets::record(ui, "agents-sessions-empty", empty_r);
    }

    if let Some(galley) = notice {
        let notice_r = Rect::from_min_size(Pos2::new(inner_left, pop_r.bottom() - pad - footer_h - notice_h), Vec2::new(inner_w, notice_h));
        let pos = Pos2::new(notice_r.left() + theme::INSET_CONTROL, notice_r.center().y - galley.size().y * 0.5);
        p.galley(pos, galley, theme::DANGER);
        widgets::record(ui, "agents-sessions-notice", notice_r);
    }

    let mut select_tab_id: Option<u64> = None;
    let mut start_rename_id: Option<u64> = None;
    let mut remove_id: Option<u64> = None;
    let mut commit_rename = false;
    let mut cancel_rename = false;
    let mut same_frame_rename_text = String::new();
    let popup_enter_pressed = ui.input(|i| i.key_pressed(Key::Enter));
    let popup_escape_pressed = ui.input(|i| i.key_pressed(Key::Escape));
    ui.input(|i| {
        for event in &i.events {
            if let egui::Event::Text(text) = event {
                same_frame_rename_text.push_str(text);
            }
        }
    });

    let action_w = |label: &str| widgets::text_width(&p, label, &theme::font_caption()) + 2.0 * theme::INSET_SEGMENT;
    let (rename_w, remove_w) = (action_w(RENAME_ACTION_LABEL), action_w(REMOVE_ACTION_LABEL));

    for row in &rows {
        let r_rect = Rect::from_min_size(Pos2::new(inner_left, y), Vec2::new(inner_w, row_h));
        let row_widget_id = format!("agents-session-row-{}", row.tab_id);

        let remove_r = Rect::from_min_size(
            Pos2::new(r_rect.right() - theme::INSET_SEGMENT - remove_w, r_rect.bottom() - bottom_line_offset - action_half_h),
            Vec2::new(remove_w, ACTION_BTN_H),
        );
        let rename_r = Rect::from_min_size(Pos2::new(remove_r.left() - theme::SPACE_FACT - rename_w, remove_r.top()), Vec2::new(rename_w, ACTION_BTN_H));

        // Register `row_resp` first and child action buttons (`rename_resp`, `remove_resp`) after so the
        // child buttons win pointer hit-testing inside `r_rect` in egui 0.35.
        let row_resp = ui.interact(r_rect, Id::new(&row_widget_id), Sense::click());
        let rename_resp = ui.interact(rename_r, Id::new((&row_widget_id, "rename")), Sense::click());
        let remove_resp = ui.interact(remove_r, Id::new((&row_widget_id, "remove")), Sense::click());

        let bg = if row.is_focused_tab {
            theme::BG_SELECTED
        } else if row_resp.hovered() || rename_resp.hovered() || remove_resp.hovered() {
            theme::BG_MENU_HOVER
        } else {
            theme::BG_MENU
        };
        p.rect_filled(r_rect, theme::cr(theme::RADIUS_CONTROL), bg);

        let top_y_line = r_rect.top() + top_line_offset;
        let bottom_y_line = r_rect.bottom() - bottom_line_offset;
        paint_session_row_indicators(ui, &p, &row.status, row.preset, r_rect.left(), top_y_line);

        let cli_x = r_rect.left() + TITLE_LEFT_X;
        let cli_str = widgets::elide(&p, &row.cli_display_name, &theme::font_body_medium(), (inner_w * 0.38).max(theme::CONTROL_MD));
        let cli_w = widgets::text_width(&p, &cli_str, &theme::font_body_medium());
        p.text(Pos2::new(cli_x, top_y_line), Align2::LEFT_CENTER, &cli_str, theme::font_body_medium(), theme::TEXT);

        let label_x = cli_x + cli_w + theme::SPACE_INLINE;
        let label_avail_w = (r_rect.right() - theme::INSET_CONTROL - label_x).max(RENAME_FIELD_MIN_W);

        if state.renaming_session_tab_id == Some(row.tab_id) {
            let field_r = Rect::from_min_size(Pos2::new(label_x, top_y_line - theme::FIELD_HEIGHT * 0.5), Vec2::new(label_avail_w, theme::FIELD_HEIGHT));
            p.rect_filled(field_r, theme::cr(theme::RADIUS_CONTROL), theme::BG_TERTIARY);
            p.rect_stroke(field_r, theme::cr(theme::RADIUS_CONTROL), Stroke::new(theme::BORDER_WIDTH, theme::BORDER_SELECTED), StrokeKind::Inside);
            let field_inset = Vec2::new(theme::INSET_SEGMENT, theme::SPACE_FACT);
            ui.scope_builder(egui::UiBuilder::new().max_rect(field_r.shrink2(field_inset)), |ui| {
                let field_id = Id::new(("agents-session-rename-input", row.tab_id));
                if std::mem::take(&mut state.rename_session_focus_pending) {
                    // The caret goes into the field in the frame it appears, whoever held the keyboard before: the agent's
                    // terminal, or a field of a form that is hidden now. The old title is selected for the letters to replace.
                    ui.memory_mut(|m| m.request_focus(field_id));
                    widgets::field::select_value(ui.ctx(), field_id, state.rename_session_input.chars().count());
                }
                let te = TextEdit::singleline(&mut state.rename_session_input)
                    .id(field_id)
                    .desired_width((label_avail_w - theme::INSET_CONTROL).max(theme::TARGET_MIN))
                    .frame(egui::Frame::NONE)
                    .font(theme::font_body())
                    .text_color(theme::TEXT)
                    .hint_text(RichText::new("Session label").font(theme::font_body()).color(theme::TEXT_TERTIARY));
                let r = ui.add(te);
                if r.clicked() || r.has_focus() {
                    state.viewport_focused = false;
                }
                if ui.input(|i| i.key_pressed(Key::Enter)) {
                    commit_rename = true;
                } else if ui.input(|i| i.key_pressed(Key::Escape)) {
                    cancel_rename = true;
                }
            });
            widgets::record(ui, &format!("agents-session-rename-input-{}", row.tab_id), field_r);
        } else {
            let lbl_str = widgets::elide(&p, &row.label, &theme::font_body(), label_avail_w);
            p.text(Pos2::new(label_x, top_y_line), Align2::LEFT_CENTER, lbl_str, theme::font_body(), theme::TEXT);
        }

        // Line 2: relative time + status on the left, Rename + Remove from list on the right
        let meta_txt = format!("{} · {}", row.relative_time, row.status.label());
        let meta_avail_w = (rename_r.left() - theme::SPACE_INLINE - cli_x).max(theme::CONTROL_MD);
        let meta_elided = widgets::elide(&p, &meta_txt, &theme::font_caption(), meta_avail_w);
        p.text(Pos2::new(cli_x, bottom_y_line), Align2::LEFT_CENTER, meta_elided, theme::font_caption(), theme::TEXT_SECONDARY);

        if rename_resp.hovered() {
            p.rect_filled(rename_r, theme::cr(theme::RADIUS_CONTROL), theme::BG_HOVER);
        }
        p.text(rename_r.center(), Align2::CENTER_CENTER, RENAME_ACTION_LABEL, theme::font_caption(), theme::TEXT_SECONDARY);
        widgets::record(ui, &format!("agents-session-rename-{}", row.tab_id), rename_r);

        if remove_resp.hovered() {
            p.rect_filled(remove_r, theme::cr(theme::RADIUS_CONTROL), theme::BG_HOVER);
        }
        p.text(remove_r.center(), Align2::CENTER_CENTER, REMOVE_ACTION_LABEL, theme::font_caption(), theme::TEXT_SECONDARY);
        widgets::record(ui, &format!("agents-session-remove-{}", row.tab_id), remove_r);

        widgets::record(ui, &row_widget_id, r_rect);

        let pointer_in_action_btn = ui.input(|i| i.pointer.interact_pos()).is_some_and(|pos| rename_r.contains(pos) || remove_r.contains(pos));
        if remove_resp.clicked() {
            remove_id = Some(row.tab_id);
        } else if rename_resp.clicked() {
            start_rename_id = Some(row.tab_id);
        } else if row_resp.clicked() && !pointer_in_action_btn && state.renaming_session_tab_id != Some(row.tab_id) {
            select_tab_id = Some(row.tab_id);
        }

        y += row_h + row_gap;
    }

    let footer_top = pop_r.bottom() - pad - footer_rows * theme::MENU_ITEM_HEIGHT;
    widgets::hline(&p, inner_left, inner_right, footer_top - theme::SPACE_ROW * 0.5, widgets::kit::separator_look().fill);
    let mut footer_y = footer_top;
    let mut stop_all_clicked = false;
    if show_stop_all {
        let stop_r = Rect::from_min_size(Pos2::new(inner_left, footer_y), Vec2::new(inner_w, theme::MENU_ITEM_HEIGHT));
        let caption = format!("{running} running");
        stop_all_clicked = sessions_footer_item(ui, &p, stop_r, "agents-sessions-stop-all", "Stop all", Some(&caption));
        footer_y += theme::MENU_ITEM_HEIGHT;
    }
    let classic_r = Rect::from_min_size(Pos2::new(inner_left, footer_y), Vec2::new(inner_w, theme::MENU_ITEM_HEIGHT));
    let classic_clicked = sessions_footer_item(ui, &p, classic_r, "agents-sessions-classic-chat", "Classic chat", None);

    if popup_escape_pressed && state.renaming_session_tab_id.is_none() {
        state.sessions_popover_open = false;
        state.cancel_rename_session();
    } else if commit_rename {
        state.commit_rename_session();
    } else if cancel_rename {
        state.cancel_rename_session();
    } else if let Some(id) = remove_id {
        state.remove_session_from_list(id);
    } else if let Some(id) = start_rename_id {
        state.start_rename_session(id);
        if !same_frame_rename_text.is_empty() {
            state.rename_session_input = same_frame_rename_text;
        }
        if popup_enter_pressed {
            state.commit_rename_session();
        } else if popup_escape_pressed {
            state.cancel_rename_session();
        }
    } else if let Some(id) = select_tab_id {
        state.select_session_row(id, file_path);
    } else if stop_all_clicked {
        let _ = crate::agent_stop::stop_all(state, designos_doc::now_ms());
        state.sessions_popover_open = false;
        state.cancel_rename_session();
    } else if classic_clicked {
        state.side_panel_pty_open = false;
        state.sessions_popover_open = false;
        state.cancel_rename_session();
    } else if ui.input(|i| i.pointer.any_pressed()) {
        let opened_this_pass = ui.ctx().data(|d| d.get_temp::<u64>(Id::new("agents-sessions-opened-pass"))) == Some(ui.ctx().cumulative_pass_nr());
        if !opened_this_pass {
            if let Some(pos) = ui.input(|i| i.pointer.interact_pos()) {
                let more_btn = ui.ctx().data(|d| d.get_temp::<Rect>(Id::new("agents-more-btn-rect")));
                let in_btn = more_btn.map(|r| r.contains(pos)).unwrap_or(false);
                if !pop_r.contains(pos) && !in_btn {
                    state.sessions_popover_open = false;
                    state.cancel_rename_session();
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent_sessions::{load_or_init_registry, save_registry, SessionEntry, SessionEntryStatus};
    use crate::agent_start_failure::FixAction;
    use std::collections::BTreeMap;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_TEST_ID: AtomicU64 = AtomicU64::new(1);

    fn temp_test_dir(tag: &str) -> PathBuf {
        let id = NEXT_TEST_ID.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("designos-open-flow-{}-{}-{}", tag, std::process::id(), id));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp test dir");
        dir
    }

    fn write_stub_cli(bin_dir: &Path, name: &str, body: &str) -> PathBuf {
        std::fs::create_dir_all(bin_dir).expect("create bin_dir");
        let path = bin_dir.join(name);
        let script = format!("#!/bin/sh\n{body}\n");
        std::fs::write(&path, script).expect("write stub script");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&path).expect("meta").permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&path, perms).expect("chmod");
        }
        path
    }

    fn make_session_entry(
        tab_id: u64,
        title: &str,
        cli: &str,
        session_id: Option<&str>,
        created_at: u64,
        last_active_at: u64,
        status: SessionEntryStatus,
    ) -> SessionEntry {
        SessionEntry {
            tab_id,
            title: title.to_string(),
            cli: cli.to_string(),
            session_id: session_id.map(str::to_string),
            created_at,
            last_active_at,
            status,
            extra: BTreeMap::new(),
        }
    }

    // 1. `open_with_no_sessions_shows_the_picker_and_spawns_nothing` / `open_flow_shows_picker_when_no_resumable_sessions`
    #[test]
    fn open_with_no_sessions_shows_the_picker_and_spawns_nothing() {
        let dir = temp_test_dir("no-sessions");
        let mut st = TerminalState::new_for_session("doc-no-sess", "Landing", Some(&dir));
        assert!(st.tabs.is_empty());
        assert!(!st.picker_open);

        st.open_for_agent_use(None);
        assert!(st.tabs.is_empty(), "No process may be spawned when there are no resumable sessions");
        assert!(st.picker_open, "picker_open must be true when there are no resumable sessions");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn open_flow_shows_picker_when_no_resumable_sessions() {
        open_with_no_sessions_shows_the_picker_and_spawns_nothing();
    }

    // 2. `open_with_a_session_resumes_the_most_recent` / `open_flow_resumes_most_recent_session`
    #[test]
    fn open_with_a_session_resumes_the_most_recent() {
        let dir = temp_test_dir("resume-most-recent");
        let bin_dir = dir.join("bin");
        let argv_file = dir.join("argv.txt");
        write_stub_cli(&bin_dir, "claude", &format!("printf '%s' \"$*\" > '{}'\nsleep 5", argv_file.display()));

        let mut st = TerminalState::new_for_session("doc-resume", "Landing", Some(&dir));
        st.path_prepend.push(bin_dir);
        st.session_registry.sessions.push(make_session_entry(
            1,
            "Older Claude",
            "claude",
            Some("11111111-1111-4111-8111-111111111111"),
            100,
            200,
            SessionEntryStatus::Exited,
        ));
        st.session_registry.sessions.push(make_session_entry(
            2,
            "Newest Claude",
            "claude",
            Some("22222222-2222-4222-8222-222222222222"),
            150,
            500,
            SessionEntryStatus::Exited,
        ));
        save_registry(&st.session_dir, &st.session_registry).expect("save registry");

        st.open_for_agent_use(None);
        assert!(!st.picker_open);
        assert_eq!(st.tabs.len(), 1);
        assert_eq!(st.tabs[0].title, "Newest Claude");
        assert!(st.tabs[0].is_resumed);

        let recorded = crate::test_timing::wait_for_nonempty_file(&argv_file, crate::test_timing::STUB_WAIT);
        assert!(recorded.contains("--resume 22222222-2222-4222-8222-222222222222"), "Expected newest session UUID in resume argv, got: {recorded:?}");

        st.shutdown();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn open_flow_resumes_most_recent_session() {
        open_with_a_session_resumes_the_most_recent();
    }

    // 3. `open_never_spawns_a_shell` / `never_spawns_shell_on_open`
    #[test]
    fn open_never_spawns_a_shell() {
        let dir = temp_test_dir("never-shell");
        let mut st = TerminalState::new_for_session("doc-never-shell", "Landing", Some(&dir));

        // Simulate the Agents rail button, Cmd+`, home OpenAgents, and palette OpenSpecReader entry points
        for _ in 0..6 {
            st.open_for_agent_use(None);
            assert!(!st.tabs.iter().any(|t| t.is_shell_process()), "open_for_agent_use must never spawn a Shell process");
        }
        assert!(st.tabs.is_empty());
        assert!(st.picker_open);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn never_spawns_shell_on_open() {
        open_never_spawns_a_shell();
    }

    // 4. `per_frame_call_removed`
    #[test]
    fn per_frame_call_removed() {
        let dir = temp_test_dir("per-frame");
        let mut st = TerminalState::new_for_session("doc-per-frame", "Landing", Some(&dir));
        st.open_for_agent_use(None);
        assert!(st.tabs.is_empty());
        assert!(st.picker_open);

        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        crate::theme::apply(&ctx);
        let panel_rect = Rect::from_min_size(Pos2::new(48.0, 0.0), Vec2::new(280.0, 720.0));

        for _ in 0..5 {
            let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1440.0, 900.0))), ..Default::default() };
            let _ = ctx.run_ui(raw, |ui| {
                let _ = crate::terminal::show_left(&mut st, ui, panel_rect, None);
            });
        }
        assert!(st.tabs.is_empty(), "Rendering frames with the panel open must never spawn a tab");

        let _ = std::fs::remove_dir_all(&dir);
    }

    // 5. `live_tab_is_not_duplicated` / `open_flow_keeps_existing_live_tab`
    #[test]
    fn live_tab_is_not_duplicated() {
        let dir = temp_test_dir("live-tab-kept");
        let bin_dir = dir.join("bin");
        write_stub_cli(&bin_dir, "claude", "sleep 5");

        let mut st = TerminalState::new_for_session("doc-live", "Landing", Some(&dir));
        st.path_prepend.push(bin_dir);
        st.spawn_tab(AgentCliPreset::Claude, None).expect("spawn claude");
        assert_eq!(st.tabs.len(), 1);

        st.open_for_agent_use(None);
        st.open_for_agent_use(None);
        assert_eq!(st.tabs.len(), 1, "open_for_agent_use must keep the existing live tab without spawning a second tab");
        assert!(!st.picker_open);

        st.shutdown();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn open_flow_keeps_existing_live_tab() {
        live_tab_is_not_duplicated();
    }

    // 6. `open_flow_ignores_missing_session`
    #[test]
    fn open_flow_ignores_missing_session() {
        let dir = temp_test_dir("missing-session");
        let mut st = TerminalState::new_for_session("doc-missing", "Landing", Some(&dir));
        st.session_registry.sessions.push(make_session_entry(
            1,
            "Gone Claude",
            "claude",
            Some("33333333-3333-4333-8333-333333333333"),
            100,
            200,
            SessionEntryStatus::Missing,
        ));
        save_registry(&st.session_dir, &st.session_registry).expect("save registry");

        st.open_for_agent_use(None);
        assert!(st.tabs.is_empty(), "Missing session must not be resumed");
        assert!(st.picker_open, "Picker must open when the only session is Missing");

        let _ = std::fs::remove_dir_all(&dir);
    }

    // 7. `open_flow_ignores_non_resumable_custom_cli`
    #[test]
    fn open_flow_ignores_non_resumable_custom_cli() {
        let dir = temp_test_dir("custom-cli");
        let mut st = TerminalState::new_for_session("doc-custom", "Landing", Some(&dir));
        st.session_registry.sessions.push(make_session_entry(1, "Custom Aider", "aider", None, 100, 200, SessionEntryStatus::Exited));
        save_registry(&st.session_dir, &st.session_registry).expect("save registry");

        st.open_for_agent_use(None);
        assert!(st.tabs.is_empty(), "Non-resumable custom CLI entry must not be resumed");
        assert!(st.picker_open, "Picker must open when only non-resumable entries exist");

        let _ = std::fs::remove_dir_all(&dir);
    }

    // 8. `reopening_after_closing_all_tabs_resumes_again`
    #[test]
    fn reopening_after_closing_all_tabs_resumes_again() {
        let dir = temp_test_dir("reopen-resumes");
        let bin_dir = dir.join("bin");
        write_stub_cli(&bin_dir, "claude", "sleep 5");

        let mut st = TerminalState::new_for_session("doc-reopen", "Landing", Some(&dir));
        st.path_prepend.push(bin_dir);
        st.spawn_tab(AgentCliPreset::Claude, None).expect("spawn initial claude");
        assert_eq!(st.tabs.len(), 1);

        st.close_tab(0);
        assert!(st.tabs.is_empty());
        assert!(st.picker_open, "Closing the last tab must show the picker");
        assert_eq!(st.session_registry.sessions[0].status, SessionEntryStatus::Exited);

        // Reopening the panel resumes the exited session
        st.open_for_agent_use(None);
        assert!(!st.picker_open);
        assert_eq!(st.tabs.len(), 1);
        assert!(st.tabs[0].is_resumed);

        st.shutdown();
        let _ = std::fs::remove_dir_all(&dir);
    }

    // 9. `sessions_dropdown_lists_cli_label_time_status` / `sessions_popover_lists_entries_newest_first_with_live_and_not_running_status`
    #[test]
    fn sessions_dropdown_lists_cli_label_time_status() {
        let dir = temp_test_dir("popover-rows");
        let bin_dir = dir.join("bin");
        write_stub_cli(&bin_dir, "claude", "sleep 5");

        let mut st = TerminalState::new_for_session("doc-popover", "Landing", Some(&dir));
        st.path_prepend.push(bin_dir);
        st.session_registry.sessions.push(make_session_entry(1, "Exited Codex", "codex", None, 100, 200, SessionEntryStatus::Exited));
        st.session_registry.sessions.push(make_session_entry(
            2,
            "Missing Grok",
            "grok",
            Some("44444444-4444-4444-8444-444444444444"),
            100,
            300,
            SessionEntryStatus::Missing,
        ));
        st.next_tab_id = 3;
        st.spawn_tab(AgentCliPreset::Claude, None).expect("spawn live claude");
        if let Ok(mut status_guard) = st.tabs[0].agent_status.lock() {
            *status_guard = AgentTabStatus::Working(Some("Building".to_string()));
        }

        let rows = st.sessions_popover_rows();
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].cli_display_name, "Claude Code");
        assert_eq!(rows[0].status.label(), "Working…");
        assert!(rows[0].is_focused_tab);

        assert_eq!(rows[1].label, "Missing Grok");
        assert_eq!(rows[1].status, SessionRowStatus::Missing);
        assert_eq!(rows[1].status.label(), "Resume failed");

        assert_eq!(rows[2].label, "Exited Codex");
        assert_eq!(rows[2].status, SessionRowStatus::NotRunning);
        assert_eq!(rows[2].status.label(), "Not running");

        // A pending approval and a plain question are two states with two labels.
        assert_eq!(SessionRowStatus::Live(AgentTabStatus::WaitingForInput(Some("Needs approval".into()))).label(), "Needs approval");
        assert_eq!(SessionRowStatus::Live(AgentTabStatus::WaitingForInput(None)).label(), "Needs input");
        assert_eq!(SessionRowStatus::Live(AgentTabStatus::Error(None)).label(), "Failed");

        // The live row follows the tab pill: a pending approval reads "Needs approval", any other wait "Needs input".
        if let Ok(mut status_guard) = st.tabs[0].agent_status.lock() {
            *status_guard = AgentTabStatus::WaitingForInput(Some("Bash".to_string()));
        }
        assert_eq!(st.sessions_popover_rows()[0].status.label(), "Needs input");
        // A text-match wait that carries the approval label but has no pending request is still `Needs input`.
        if let Ok(mut status_guard) = st.tabs[0].agent_status.lock() {
            *status_guard = AgentTabStatus::WaitingForInput(Some(NEEDS_APPROVAL_LABEL.to_string()));
        }
        assert!(st.tabs[0].pending_approvals.is_empty());
        assert_eq!(st.sessions_popover_rows()[0].status.label(), "Needs input");
        st.tabs[0].pending_approvals.push(crate::agent_permissions::ApprovalRequest {
            id: "req-1".into(),
            tool_name: "Bash".into(),
            input_summary: "cargo test".into(),
            timestamp: 1,
            target: String::new(),
        });
        assert_eq!(st.sessions_popover_rows()[0].status.label(), "Needs approval");

        st.shutdown();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn sessions_popover_lists_entries_newest_first_with_live_and_not_running_status() {
        sessions_dropdown_lists_cli_label_time_status();
    }

    // 10. `rename_persists_across_restart` / `rename_session_in_place_updates_registry_and_reverts_empty_to_default`
    #[test]
    fn rename_persists_across_restart() {
        let dir = temp_test_dir("rename-persist");
        let bin_dir = dir.join("bin");
        write_stub_cli(&bin_dir, "claude", "sleep 5");

        let mut st = TerminalState::new_for_session("doc-rename", "Landing", Some(&dir));
        st.path_prepend.push(bin_dir);
        st.spawn_tab(AgentCliPreset::Claude, None).expect("spawn claude");
        let tab_id = st.tabs[0].id as u64;
        let created_at = st.session_registry.sessions[0].created_at;

        st.start_rename_session(tab_id);
        st.rename_session_input = "Checkout header polish".to_string();
        st.commit_rename_session();

        assert_eq!(st.tabs[0].title, "Checkout header polish");
        assert_eq!(st.session_registry.sessions[0].title, "Checkout header polish");

        let reloaded = load_or_init_registry(&st.session_dir);
        assert_eq!(reloaded.sessions[0].title, "Checkout header polish");

        // Empty rename reverts to `<CLI> <date>`
        st.rename_session_entry(tab_id, "   ");
        let expected_default = default_session_label("claude", created_at);
        assert_eq!(st.tabs[0].title, expected_default);
        assert_eq!(load_or_init_registry(&st.session_dir).sessions[0].title, expected_default);

        st.shutdown();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn rename_session_in_place_updates_registry_and_reverts_empty_to_default() {
        rename_persists_across_restart();
    }

    // 11. `closing_the_last_tab_shows_the_picker` / `closing_last_tab_shows_picker`
    #[test]
    fn closing_the_last_tab_shows_the_picker() {
        let dir = temp_test_dir("close-last");
        let bin_dir = dir.join("bin");
        write_stub_cli(&bin_dir, "claude", "sleep 5");

        let mut st = TerminalState::new_for_session("doc-close-last", "Landing", Some(&dir));
        st.path_prepend.push(bin_dir);
        st.spawn_tab(AgentCliPreset::Claude, None).expect("spawn claude");
        assert!(!st.picker_open);

        st.close_tab(0);
        assert!(st.tabs.is_empty(), "No shell tab may be auto-spawned when closing the last tab");
        assert!(st.picker_open, "Closing the last tab must set picker_open = true");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn closing_last_tab_shows_picker() {
        closing_the_last_tab_shows_the_picker();
    }

    // 12. `cmdk_with_no_live_tab_resumes_the_last_session` / `cmd_k_prompt_uses_resume_or_picker_never_first_installed`
    #[test]
    fn cmdk_with_no_live_tab_resumes_the_last_session() {
        let dir = temp_test_dir("cmdk-resume");
        let bin_dir = dir.join("bin");
        let argv_file = dir.join("cmdk-argv.txt");
        write_stub_cli(&bin_dir, "claude", &format!("printf '%s' \"$*\" > '{}'\ncat", argv_file.display()));

        // Case A: with a resumable session, dispatch_figma_ai_prompt resumes it and sends the prompt there
        let mut st = TerminalState::new_for_session("doc-cmdk-a", "Landing", Some(&dir));
        st.path_prepend.push(bin_dir.clone());
        st.session_registry.sessions.push(make_session_entry(
            1,
            "Claude Session",
            "claude",
            Some("55555555-5555-4555-8555-555555555555"),
            100,
            400,
            SessionEntryStatus::Exited,
        ));
        save_registry(&st.session_dir, &st.session_registry).expect("save registry");

        st.dispatch_figma_ai_prompt("Make Design", "Build pricing table", None);
        assert_eq!(st.tabs.len(), 1);
        assert!(st.tabs[0].is_resumed);
        assert!(!st.picker_open);

        let argv = crate::test_timing::wait_for_nonempty_file(&argv_file, crate::test_timing::STUB_WAIT);
        assert!(argv.contains("--resume 55555555-5555-4555-8555-555555555555"), "Expected resume argv, got: {argv:?}");
        st.shutdown();

        // Case B: with no resumable session, dispatch_figma_ai_prompt opens the picker and holds the prompt until a CLI is chosen
        let mut st_empty = TerminalState::new_for_session("doc-cmdk-b", "Landing", Some(&dir));
        st_empty.path_prepend.push(bin_dir);
        st_empty.dispatch_figma_ai_prompt("Make Design", "Build hero section", None);
        assert!(st_empty.tabs.is_empty(), "Must not auto-spawn first installed CLI when no resumable session exists");
        assert!(st_empty.picker_open, "Must open the picker when no resumable session exists");
        assert_eq!(st_empty.pending_prompt, Some(("Make Design".to_string(), "Build hero section".to_string())), "Prompt must be held until a CLI is chosen");

        // Choosing a CLI from the picker dispatches the held prompt
        st_empty.spawn_tab(AgentCliPreset::Claude, None).expect("spawn chosen cli");
        assert!(!st_empty.picker_open);
        assert!(st_empty.pending_prompt.is_none(), "Held prompt must be consumed once a CLI is spawned");
        assert!(
            st_empty.tabs[0].composer.pending_send.as_deref() == Some("Build hero section")
                || st_empty.tabs[0].composer.history.iter().any(|h| h.contains("Build hero section"))
        );

        st_empty.shutdown();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn cmd_k_prompt_uses_resume_or_picker_never_first_installed() {
        cmdk_with_no_live_tab_resumes_the_last_session();
    }

    // 13. `backtick_follows_the_panel_rules` / `backtick_follows_same_open_rules`
    #[test]
    fn backtick_follows_the_panel_rules() {
        let dir = temp_test_dir("backtick");
        let bin_dir = dir.join("bin");
        write_stub_cli(&bin_dir, "claude", "sleep 5");

        let mut st = TerminalState::new_for_session("doc-backtick", "Landing", Some(&dir));
        st.path_prepend.push(bin_dir);

        // Without session: the Agents rail button / Cmd+` call open_for_agent_use -> picker_open, 0 tabs
        st.open_for_agent_use(None);
        assert!(st.tabs.is_empty());
        assert!(st.picker_open);

        // With a recorded session: the rail button / Cmd+` resume that session, never a Shell
        st.session_registry.sessions.push(make_session_entry(
            1,
            "Claude Backtick",
            "claude",
            Some("66666666-6666-4666-8666-666666666666"),
            100,
            300,
            SessionEntryStatus::Exited,
        ));
        save_registry(&st.session_dir, &st.session_registry).expect("save registry");

        st.open_for_agent_use(None);
        assert!(!st.picker_open);
        assert_eq!(st.tabs.len(), 1);
        assert!(!st.tabs[0].is_shell_process());
        assert!(st.tabs[0].is_resumed);

        st.shutdown();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn backtick_follows_same_open_rules() {
        backtick_follows_the_panel_rules();
    }

    // 14. `shell_is_never_registered_or_resumed`
    #[test]
    fn shell_is_never_registered_or_resumed() {
        let dir = temp_test_dir("shell-not-registered");
        let bin_dir = dir.join("bin");
        write_stub_cli(&bin_dir, "claude", "sleep 5");

        let mut st = TerminalState::new_for_session("doc-shell-unreg", "Landing", Some(&dir));
        st.path_prepend.push(bin_dir);
        st.spawn_tab(AgentCliPreset::Claude, None).expect("spawn claude");
        st.close_tab(0);

        st.spawn_tab(AgentCliPreset::Shell, None).expect("spawn shell");
        st.close_tab(0);

        assert_eq!(st.session_registry.sessions.len(), 1, "Shell tab must never be registered in sessions.json");
        assert_eq!(st.session_registry.sessions[0].cli, "claude");

        st.open_for_agent_use(None);
        assert_eq!(st.tabs.len(), 1);
        assert_eq!(st.tabs[0].spawn_preset, AgentCliPreset::Claude);

        st.shutdown();
        let _ = std::fs::remove_dir_all(&dir);
    }

    // 15. `remove_from_list_forgets_only_the_registry_entry` / `remove_from_list_forgets_registry_entry_only`
    #[test]
    fn remove_from_list_forgets_only_the_registry_entry() {
        let dir = temp_test_dir("remove-entry");
        let external_cli_store = dir.join("claude-transcript.jsonl");
        std::fs::write(&external_cli_store, "{\"turn\":1}\n").expect("write mock CLI conversation store");

        let mut st = TerminalState::new_for_session("doc-remove", "Landing", Some(&dir));
        st.session_registry.sessions.push(make_session_entry(
            1,
            "Claude Session #1",
            "claude",
            Some("77777777-7777-4777-8777-777777777777"),
            100,
            200,
            SessionEntryStatus::Exited,
        ));
        save_registry(&st.session_dir, &st.session_registry).expect("save registry");

        st.remove_session_from_list(1);
        assert!(st.session_registry.sessions.is_empty());
        assert!(load_or_init_registry(&st.session_dir).sessions.is_empty());
        assert!(external_cli_store.exists(), "Removing from Studio's list must leave the CLI's own store untouched");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn remove_from_list_forgets_registry_entry_only() {
        remove_from_list_forgets_only_the_registry_entry();
    }

    // 16. `chat_submit_starts_its_shell_lazily` / `chat_submit_lazily_spawns_shell`
    #[test]
    fn chat_submit_starts_its_shell_lazily() {
        let dir = temp_test_dir("chat-lazy-shell");
        let bin_dir = dir.join("bin");
        write_stub_cli(&bin_dir, "claude", "sleep 5");
        let mut st = TerminalState::new_for_session("doc-chat-lazy", "Landing", Some(&dir));
        st.path_prepend.push(bin_dir);
        st.active_runtime = AgentCliPreset::Claude;
        st.open_for_agent_use(None);
        assert!(st.tabs.is_empty(), "No tab before chat submit");

        st.submit_chat_prompt("Create a card", None);
        assert_eq!(st.tabs.len(), 1, "submit_chat_prompt must lazily start the tab's agent on submit");
        assert!(!st.tabs[0].is_shell_process());
        assert_eq!(st.tabs[0].composer.pending_send.as_deref(), Some("Create a card"));

        st.shutdown();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn chat_submit_lazily_spawns_shell() {
        chat_submit_starts_its_shell_lazily();
    }

    // 17. `resume_failure_offers_a_new_session`
    #[test]
    fn resume_failure_offers_a_new_session() {
        let dir = temp_test_dir("resume-failure");
        let bin_dir = dir.join("bin");
        write_stub_cli(&bin_dir, "claude", "echo 'No conversation found' >&2\nexit 1");

        let mut st = TerminalState::new_for_session("doc-resume-fail", "Landing", Some(&dir));
        st.path_prepend.push(bin_dir);
        st.session_registry.sessions.push(make_session_entry(
            1,
            "Claude #1",
            "claude",
            Some("88888888-8888-4888-8888-888888888888"),
            100,
            200,
            SessionEntryStatus::Exited,
        ));
        save_registry(&st.session_dir, &st.session_registry).expect("save registry");

        st.open_for_agent_use(None);
        st.poll_until_start_failure(crate::test_timing::STUB_WAIT);

        let fix = st.active_tab().and_then(|t| t.start_failure.as_ref()).map(|f| f.fix.clone()).expect("start_failure must be recorded");
        assert_eq!(fix, FixAction::StartNewSession(AgentCliPreset::Claude));

        // Clicking "Start new session" closes the failed tab and opens the picker
        st.apply_start_fix(fix, None);
        assert!(st.tabs.is_empty());
        assert!(st.picker_open, "FixAction::StartNewSession must open the new-session picker");

        st.shutdown();
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn painted_texts(out: &egui::FullOutput) -> Vec<String> {
        crate::lint::scan_shapes(&out.shapes).0.into_iter().map(|t| t.text).collect()
    }

    /// The painted texts with wrapped rows joined: a notice that wraps onto a second line is still one sentence
    /// (the wrap follows a space, so joining the rows restores it).
    fn painted_sentences(out: &egui::FullOutput) -> Vec<String> {
        painted_texts(out).into_iter().map(|t| t.replace('\n', "")).collect()
    }

    fn click_at(ctx: &egui::Context, screen: Rect, pos: Pos2, mut frame: impl FnMut(&mut Ui)) {
        for pressed in [true, false] {
            let raw = egui::RawInput {
                screen_rect: Some(screen),
                events: vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE },
                ],
                ..Default::default()
            };
            let _ = ctx.run_ui(raw, &mut frame);
        }
    }

    const NOT_RESUMABLE_NOTICE: &str = "Can't resume. Start a new session.";

    #[test]
    fn clicking_a_non_resumable_session_row_shows_a_notice_and_keeps_the_popover_open() {
        let dir = temp_test_dir("row-not-resumable");
        let mut st = TerminalState::new_for_session("doc-row-fail", "Landing", Some(&dir));
        st.codex_cwd_scoped = false;
        st.session_registry.sessions.push(make_session_entry(1, "Codex #1", "codex", None, 100, 200, SessionEntryStatus::Exited));
        save_registry(&st.session_dir, &st.session_registry).expect("save registry");
        st.sessions_popover_open = true;

        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        crate::theme::apply(&ctx);
        let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(1440.0, 900.0));
        let panel_rect = Rect::from_min_size(Pos2::new(48.0, 0.0), Vec2::new(320.0, 720.0));
        let _ = ctx.run_ui(egui::RawInput { screen_rect: Some(screen), ..Default::default() }, |ui| {
            render_sessions_popover(&mut st, ui, panel_rect, 40.0, None);
        });
        let row_r = ctx.data_mut(|d| d.get_temp::<Rect>(Id::new(("test-rect", "agents-session-row-1".to_string())))).expect("row rect");

        // Click the row title area (away from Rename / Remove from list).
        click_at(&ctx, screen, Pos2::new(row_r.left() + 30.0, row_r.top() + 8.0), |ui| render_sessions_popover(&mut st, ui, panel_rect, 40.0, None));

        assert!(st.tabs.is_empty(), "nothing may spawn");
        assert!(st.sessions_popover_open, "popover stays open so the notice is seen");
        assert_eq!(st.resume_notice.as_deref(), Some(NOT_RESUMABLE_NOTICE));

        let out = ctx.run_ui(egui::RawInput { screen_rect: Some(screen), ..Default::default() }, |ui| {
            render_sessions_popover(&mut st, ui, panel_rect, 40.0, None);
        });
        assert!(painted_texts(&out).iter().any(|t| t == NOT_RESUMABLE_NOTICE), "notice must be painted in the popover: {:?}", painted_texts(&out));

        st.shutdown();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn resume_failure_copy_matches_the_errors_terminal_really_returns() {
        let dir = temp_test_dir("copy-real-errors");
        let mut st = TerminalState::new_for_session("doc-copy", "Landing", Some(&dir));
        st.codex_cwd_scoped = false;

        let none_recorded = st.resume_last_session(None).expect_err("empty registry has nothing to resume");
        assert_eq!(resume_failure_copy(&none_recorded), "No earlier session. Start a new one.");

        let gone = st.resume_session_by_tab_id(99, None).expect_err("unknown tab id is gone");
        assert_eq!(resume_failure_copy(&gone), "Session is gone. Start a new one.");

        st.session_registry.sessions.push(make_session_entry(1, "Codex #1", "codex", None, 100, 200, SessionEntryStatus::Exited));
        let unsupported = st.resume_session_by_tab_id(1, None).expect_err("codex without a scoped cwd cannot resume safely");
        assert_eq!(resume_failure_copy(&unsupported), NOT_RESUMABLE_NOTICE);
        let newest_unsupported = st.resume_last_session(None).expect_err("newest entry cannot resume safely");
        assert_eq!(resume_failure_copy(&newest_unsupported), NOT_RESUMABLE_NOTICE);

        // Spawn failures and anything unrecognised still get an honest recovery.
        assert_eq!(resume_failure_copy("PTY error: boom"), "Resume failed. Start a new session.");

        st.shutdown();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn resume_notice_copy_fits_the_narrowest_menus_without_eliding() {
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        crate::theme::apply(&ctx);
        let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(1440.0, 900.0));
        // 200 px is the narrowest width the Agents panel can be dragged to (`shell.rs` left panel clamp).
        let narrow_panel = Rect::from_min_size(Pos2::new(48.0, 0.0), Vec2::new(200.0, 720.0));
        let dir = temp_test_dir("copy-fit");
        let mut st = TerminalState::new_for_session("doc-copy-fit", "Landing", Some(&dir));
        for raw in
            ["No previous agent session to resume", "No resumable session with tab_id 4", "CLI 'x' does not support safe session resume", "PTY error: boom"]
        {
            st.set_resume_failure(raw);
            let notice = st.resume_notice.clone().expect("notice set");
            st.sessions_popover_open = true;
            st.runtime_dropdown_open = false;
            let out = ctx.run_ui(egui::RawInput { screen_rect: Some(screen), ..Default::default() }, |ui| {
                render_sessions_popover(&mut st, ui, narrow_panel, 40.0, None);
            });
            assert!(painted_sentences(&out).contains(&notice), "popover elides {notice:?}: {:?}", painted_texts(&out));
        }
        st.shutdown();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn resume_notice_does_not_outlive_the_menu_that_showed_it() {
        let dir = temp_test_dir("notice-lifecycle");
        let mut st = TerminalState::new_for_session("doc-notice-life", "Landing", Some(&dir));
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        crate::theme::apply(&ctx);
        let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(1600.0, 900.0));
        let panel_rect = Rect::from_min_size(Pos2::new(48.0, 0.0), Vec2::new(200.0, 800.0));
        let frame = |st: &mut TerminalState| {
            ctx.run_ui(egui::RawInput { screen_rect: Some(screen), ..Default::default() }, |ui| {
                let _ = crate::terminal::show_left(st, ui, panel_rect, None);
            })
        };

        st.set_resume_failure("CLI 'codex' does not support safe session resume");
        st.sessions_popover_open = true;
        let open = painted_sentences(&frame(&mut st));
        assert!(open.iter().any(|t| t == NOT_RESUMABLE_NOTICE), "notice shows in the open sessions popover: {open:?}");

        // Picking a row (or Esc, or an outside click) closes the menu: the notice must go with it.
        st.sessions_popover_open = false;
        let _ = frame(&mut st);
        assert!(st.resume_notice.is_none(), "closed popover must not keep a stale notice");

        st.sessions_popover_open = true;
        let reopened = painted_texts(&frame(&mut st));
        assert!(!reopened.iter().any(|t| t == NOT_RESUMABLE_NOTICE), "reopened popover must not replay the old notice: {reopened:?}");

        st.shutdown();
        let _ = std::fs::remove_dir_all(&dir);
    }

    // 18. `sessions_popover_rename_and_remove_buttons_win_hit_testing_over_row`
    #[test]
    fn sessions_popover_rename_and_remove_buttons_win_hit_testing_over_row() {
        let dir = temp_test_dir("popover-hit-test");
        let bin_dir = dir.join("bin");
        write_stub_cli(&bin_dir, "claude", "sleep 5");

        let mut st = TerminalState::new_for_session("doc-popover-hit", "Landing", Some(&dir));
        st.path_prepend.push(bin_dir);
        st.session_registry.sessions.push(make_session_entry(
            1,
            "Claude #1",
            "claude",
            Some("99999999-9999-4999-8999-999999999999"),
            100,
            200,
            SessionEntryStatus::Exited,
        ));
        save_registry(&st.session_dir, &st.session_registry).expect("save registry");
        st.sessions_popover_open = true;

        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        crate::theme::apply(&ctx);
        let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(1440.0, 900.0));
        let panel_rect = Rect::from_min_size(Pos2::new(48.0, 0.0), Vec2::new(320.0, 720.0));

        // Frame 1: lay out popover to record button rects
        let _ = ctx.run_ui(egui::RawInput { screen_rect: Some(screen), ..Default::default() }, |ui| {
            render_sessions_popover(&mut st, ui, panel_rect, 40.0, None);
        });
        let rename_r = ctx.data_mut(|d| d.get_temp::<Rect>(Id::new(("test-rect", "agents-session-rename-1".to_string())))).expect("rename button rect");
        let remove_r = ctx.data_mut(|d| d.get_temp::<Rect>(Id::new(("test-rect", "agents-session-remove-1".to_string())))).expect("remove button rect");

        // Click Rename (press + release on rename_r.center()) -> must start inline rename, NOT select/resume row
        let rename_pos = rename_r.center();
        for pressed in [true, false] {
            let raw = egui::RawInput {
                screen_rect: Some(screen),
                events: vec![
                    egui::Event::PointerMoved(rename_pos),
                    egui::Event::PointerButton { pos: rename_pos, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE },
                ],
                ..Default::default()
            };
            let _ = ctx.run_ui(raw, |ui| {
                render_sessions_popover(&mut st, ui, panel_rect, 40.0, None);
            });
        }
        assert_eq!(st.renaming_session_tab_id, Some(1), "Clicking Rename must start inline rename instead of being stolen by row_resp");
        assert!(st.tabs.is_empty(), "Clicking Rename must not trigger select_session_row");

        // Click Remove from list (press + release on remove_r.center()) -> must remove session from registry, NOT select/resume row
        let remove_pos = remove_r.center();
        for pressed in [true, false] {
            let raw = egui::RawInput {
                screen_rect: Some(screen),
                events: vec![
                    egui::Event::PointerMoved(remove_pos),
                    egui::Event::PointerButton { pos: remove_pos, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE },
                ],
                ..Default::default()
            };
            let _ = ctx.run_ui(raw, |ui| {
                render_sessions_popover(&mut st, ui, panel_rect, 40.0, None);
            });
        }
        assert!(st.session_registry.sessions.is_empty(), "Clicking Remove from list must remove the session entry");
        assert!(st.tabs.is_empty(), "Clicking Remove from list must not trigger select_session_row");

        st.shutdown();
        let _ = std::fs::remove_dir_all(&dir);
    }

    // 19. `sessions_button_single_frame_click_keeps_popover_open`
    // 19. `more_button_single_frame_click_keeps_popover_open`
    #[test]
    fn more_button_single_frame_click_keeps_popover_open() {
        let dir = temp_test_dir("btn-single-frame");
        let mut st = TerminalState::new_for_session("doc-btn-frame", "Landing", Some(&dir));

        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        crate::theme::apply(&ctx);
        let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(1440.0, 900.0));
        let panel_rect = Rect::from_min_size(Pos2::new(48.0, 0.0), Vec2::new(320.0, 720.0));

        // Initial layout frame
        let _ = ctx.run_ui(egui::RawInput { screen_rect: Some(screen), ..Default::default() }, |ui| {
            crate::agent_header::paint(&mut st, ui, panel_rect);
        });
        assert!(!st.sessions_popover_open);
        let btn_rect = ctx.data(|d| d.get_temp::<Rect>(Id::new("agents-more-btn-rect"))).expect("the more button is drawn in the header");

        // Single-frame synthetic click (both press and release in one frame)
        let pos = btn_rect.center();
        let raw = egui::RawInput {
            screen_rect: Some(screen),
            events: vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE },
                egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE },
            ],
            ..Default::default()
        };
        let _ = ctx.run_ui(raw, |ui| {
            crate::agent_header::paint(&mut st, ui, panel_rect);
            if st.sessions_popover_open {
                render_sessions_popover(&mut st, ui, panel_rect, 40.0, None);
            }
        });
        assert!(st.sessions_popover_open, "Opening the sessions popover must not immediately self-close on the same pass");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn sessions_popover_footer_lists_classic_chat_and_hides_stop_all_with_no_running_tabs() {
        let dir = temp_test_dir("popover-footer");
        let mut st = TerminalState::new_for_session("doc-popover-footer", "Landing", Some(&dir));
        st.sessions_popover_open = true;
        st.side_panel_pty_open = true;

        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        crate::theme::apply(&ctx);
        let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(1440.0, 900.0));
        let panel_rect = Rect::from_min_size(Pos2::new(48.0, 0.0), Vec2::new(320.0, 720.0));
        let out = ctx.run_ui(egui::RawInput { screen_rect: Some(screen), ..Default::default() }, |ui| {
            render_sessions_popover(&mut st, ui, panel_rect, 40.0, None);
        });
        let texts = painted_texts(&out);
        assert!(texts.iter().any(|t| t == "Classic chat"), "footer lists Classic chat: {texts:?}");
        assert!(!texts.iter().any(|t| t == "Stop all"), "Stop all stays hidden until two tabs can be stopped: {texts:?}");

        let classic = ctx.data_mut(|d| d.get_temp::<Rect>(Id::new(("test-rect", "agents-sessions-classic-chat".to_string())))).expect("classic chat row");
        click_at(&ctx, screen, classic.center(), |ui| render_sessions_popover(&mut st, ui, panel_rect, 40.0, None));
        assert!(!st.side_panel_pty_open, "Classic chat leaves the live terminal");
        assert!(!st.sessions_popover_open, "the popover closes after the choice");

        st.shutdown();
        let _ = std::fs::remove_dir_all(&dir);
    }

    // 20. `execute_cmdk_action_dispatches_prompt_when_no_live_tab_is_open`
    #[test]
    fn execute_cmdk_action_dispatches_prompt_when_no_live_tab_is_open() {
        let dir = temp_test_dir("shell-cmdk-empty-tabs");
        let bin_dir = dir.join("bin");
        write_stub_cli(&bin_dir, "claude", "cat");

        // Case A: no resumable session -> execute_cmdk_action opens picker and holds pending_prompt
        let mut app = crate::app::App::new(designos_doc::Document::new("Test"));
        app.terminal = TerminalState::new_for_session("doc-shell-cmdk-a", "Landing", Some(&dir));
        app.terminal.path_prepend.push(bin_dir.clone());
        assert!(app.terminal.tabs.is_empty());

        let spec: designos_doc::spec::NodeSpec =
            serde_json::from_str(r#"{"kind":"frame","name":"Pricing card","width":320,"height":200}"#).expect("parse test spec");
        let spec_name = spec.name.clone();
        crate::shell::execute_cmdk_action(&mut app, crate::cmdk::CmdKAction::ApplySpec(Box::new(spec)));
        assert!(app.terminal.tabs.is_empty(), "No process may auto-spawn when no resumable session exists");
        assert!(app.terminal.picker_open, "execute_cmdk_action must open the picker when no resumable session exists");
        assert_eq!(app.terminal.pending_prompt, Some(("Make Design".to_string(), spec_name)));

        // Case B: resumable session exists -> execute_cmdk_action resumes it and dispatches the prompt.
        // The auto-fix only tells the agent about changes it made, so the document has copy under the 11px floor.
        let mut doc = designos_doc::Document::new("Test");
        let page = doc.add_page("Page 1");
        let small_text = doc.alloc_id();
        let style = designos_doc::TextStyle { font_size: 8.0, ..designos_doc::TextStyle::default() };
        doc.insert(page, designos_doc::Node::text(small_text, designos_doc::Rect::new(0.0, 0.0, 80.0, 12.0), "Small", style)).expect("insert the small text");
        let mut app_resumable = crate::app::App::new(doc);
        app_resumable.terminal = TerminalState::new_for_session("doc-shell-cmdk-b", "Landing", Some(&dir));
        app_resumable.terminal.path_prepend.push(bin_dir);
        app_resumable.terminal.session_registry.sessions.push(make_session_entry(
            1,
            "Claude #1",
            "claude",
            Some("aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa"),
            100,
            300,
            SessionEntryStatus::Exited,
        ));
        save_registry(&app_resumable.terminal.session_dir, &app_resumable.terminal.session_registry).expect("save registry");

        {
            // the auto-fix takes the AI operation another test may have queued in this process
            let _ai_ops = crate::cmdk::ai_op_test_lock();
            crate::shell::execute_cmdk_action(&mut app_resumable, crate::cmdk::CmdKAction::AutoFixDesignGate);
        }
        assert_eq!(app_resumable.terminal.tabs.len(), 1, "execute_cmdk_action must resume the last session when tabs is empty");
        assert!(app_resumable.terminal.tabs[0].is_resumed);
        assert!(!app_resumable.terminal.picker_open);

        app_resumable.terminal.shutdown();
        let _ = std::fs::remove_dir_all(&dir);
    }
}
