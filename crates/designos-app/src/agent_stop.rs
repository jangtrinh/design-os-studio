//! Stop an agent from the panel (`#684` / `AX-684`):
//! - Interrupt key table (`INTERRUPT_KEY_TABLE`), one row per CLI (`Grok` grounded from its guide;
//!   `Claude`, `Codex`, `Antigravity` default `Esc` then `Ctrl+C`).
//! - Per-tab `StopState { sent_at, second_sent, key_table_row }` sequence with a 3-second retry.
//! - Turn close on stop: applies held live patches, folds the turn's contiguous agent undo steps into
//!   one `Command::Batch` labelled `"Agent turn (stopped)"` via `History::fold_since`, clears
//!   `Document::in_progress` flags for the agent, and records `"Stopped by you after <N>s"`.
//! - Secondary header buttons `"Stop"` and `"Stop all"` (when 2+ tabs run) and chrome-only `Esc`.

use crate::app::App;
use crate::terminal::{AgentCliPreset, AgentTabStatus, TerminalAction, TerminalState, TerminalTab};
use crate::{theme, widgets};
use egui::{Align2, Id, Painter, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, Vec2};

/// Undo step label applied by [`designos_doc::History::fold_since`] when a turn is stopped (`§3.4`).
pub const STOP_UNDO_LABEL: &str = "Agent turn (stopped)";

/// Delay in milliseconds before sending the second interrupt key (`Ctrl+C`) if still running (`§3.3`).
pub const SECOND_KEY_DELAY_MS: u64 = 3_000;

/// Notice shown when a CLI ignores both interrupt keys (`§8.1`).
pub const STILL_RUNNING_NOTICE: &str = "Still running. Close the tab to end it.";

/// How long the PTY must stay silent after the first interrupt key before the stop counts as taken (`§3.3`).
/// A CLI that does not fire a hook for a user interrupt (Claude Code skips its Stop hook) never reports the end
/// of the turn, so the quiet prompt is the only signal. Same window as the Codex output-burst heuristic.
pub const STOP_QUIET_CLOSE_MS: u64 = 600;

/// ASCII `ESC` (`0x1b`).
pub const KEY_ESC: u8 = 0x1b;

/// ASCII `ETX` / `Ctrl+C` (`0x03`).
pub const KEY_CTRL_C: u8 = 0x03;

/// One row of the per-CLI interrupt key table (`§3.2`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InterruptKeyRow {
    pub cli: AgentCliPreset,
    pub first_key: u8,
    pub second_key: u8,
    pub source: &'static str,
    pub grounded: bool,
    /// True when the CLI reports an interrupted turn through a hook record (Grok `StopCancelled`), so the
    /// hook, not a quiet PTY, closes the stop.
    pub fires_interrupt_hook: bool,
}

/// Canonical interrupt key table (`§3.2`). The keys live only in this table.
pub const INTERRUPT_KEY_TABLE: &[InterruptKeyRow] = &[
    InterruptKeyRow {
        cli: AgentCliPreset::Grok,
        first_key: KEY_CTRL_C,
        second_key: KEY_CTRL_C,
        source: "~/.grok/docs/user-guide/10-hooks.md:478 (Ctrl+C interrupts a turn; Esc never cancels a running turn)",
        grounded: true,
        fires_interrupt_hook: true,
    },
    InterruptKeyRow {
        cli: AgentCliPreset::Claude,
        first_key: KEY_ESC,
        second_key: KEY_CTRL_C,
        source: "issue #684 default (Esc then Ctrl+C)",
        grounded: false,
        fires_interrupt_hook: false,
    },
    InterruptKeyRow {
        cli: AgentCliPreset::Codex,
        first_key: KEY_ESC,
        second_key: KEY_CTRL_C,
        source: "issue #684 default (Esc then Ctrl+C)",
        grounded: false,
        fires_interrupt_hook: false,
    },
    InterruptKeyRow {
        cli: AgentCliPreset::Antigravity,
        first_key: KEY_ESC,
        second_key: KEY_CTRL_C,
        source: "issue #684 default (Esc then Ctrl+C)",
        grounded: false,
        fires_interrupt_hook: false,
    },
];

/// Looks up the [`INTERRUPT_KEY_TABLE`] index and row for `cli`. Shell and help tabs have no row (`§3.7`).
pub fn interrupt_row_for(cli: AgentCliPreset) -> Option<(usize, &'static InterruptKeyRow)> {
    INTERRUPT_KEY_TABLE.iter().enumerate().find(|(_, row)| row.cli == cli)
}

/// Per-tab stop sequence state in memory (`§4`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StopState {
    /// Timestamp (ms from [`designos_doc::now_ms`]) when the first interrupt key was sent.
    pub sent_at: u64,
    /// True once the 3-second follow-up `Ctrl+C` has been sent (`§3.3`).
    pub second_sent: bool,
    /// Index into [`INTERRUPT_KEY_TABLE`].
    pub key_table_row: usize,
    /// Monotonic time of the first key, to measure how long the PTY has been quiet since.
    pub sent_instant: std::time::Instant,
}

impl StopState {
    /// Button label for the Stop control (`"Stopping…"` once the stop sequence starts, `§3.3` & `§7`).
    pub fn button_label(&self) -> &'static str {
        "Stopping…"
    }
}

/// Formats the stopped-turn receipt line (`"Stopped by you after 42s"` or `"Stopped by you after 1m 20s"`, `§3.4` & `#683 §3.5`).
pub fn format_stopped_receipt(started_ms: u64, ended_ms: u64) -> String {
    let elapsed_ms = ended_ms.saturating_sub(started_ms);
    let secs = elapsed_ms.div_ceil(1_000).max(1);
    if secs < 60 {
        format!("Stopped by you after {secs}s")
    } else {
        format!("Stopped by you after {}m {}s", secs / 60, secs % 60)
    }
}

/// True when `tab` has a known running turn eligible for Stop (`§3.1`, `§3.7`, `§8.6`).
pub fn is_tab_turn_running(tab: &TerminalTab) -> bool {
    if tab.is_shell_process() || tab.resumed_without_known_turn {
        return false;
    }
    let explicit = tab.agent_status.lock().ok().map(|s| s.clone()).unwrap_or_default();
    match explicit {
        AgentTabStatus::Working(_) => true,
        AgentTabStatus::WaitingForInput(_) | AgentTabStatus::Done | AgentTabStatus::Error(_) | AgentTabStatus::Stopped => false,
        AgentTabStatus::Idle => {
            matches!(tab.spawn_preset, AgentCliPreset::Codex)
                && tab.last_output_at.lock().ok().and_then(|g| *g).is_some_and(|ts| ts.elapsed() < std::time::Duration::from_millis(600))
        }
    }
}

/// Number of tabs in `state` whose turn is currently running (`§3.6`).
pub fn running_tab_count(state: &TerminalState) -> usize {
    state.tabs.iter().filter(|t| is_tab_turn_running(t)).count()
}

/// Starts the stop sequence on `tab` at `now_ms` (`§3.3`).
///
/// Does nothing and sends zero bytes if:
/// - the tab's turn is not currently running (`§3.3`, `§8.2`),
/// - a stop sequence is already in flight on the tab (`§8.7`), or
/// - the tab is a shell process (`§3.7`).
pub fn start_stop_on_tab(tab: &mut TerminalTab, now_ms: u64) -> bool {
    if !is_tab_turn_running(tab) || tab.stop_state.is_some() {
        return false;
    }
    let Some((row_idx, row)) = interrupt_row_for(tab.spawn_preset) else {
        return false;
    };
    if tab.turn_started_at_ms.is_none() {
        tab.turn_started_at_ms = Some(now_ms);
    }
    tab.stop_in_flight.store(true, std::sync::atomic::Ordering::Relaxed);
    tab.write_bytes(&[row.first_key]);
    tab.stop_sent_keys.push(row.first_key);
    let sent_instant = std::time::Instant::now();
    tab.stop_sent_instant = Some(sent_instant);
    tab.stop_state = Some(StopState { sent_at: now_ms, second_sent: false, key_table_row: row_idx, sent_instant });
    true
}

/// Starts the stop sequence on the active tab in `state` at `now_ms`.
pub fn start_stop_active(state: &mut TerminalState, now_ms: u64) -> bool {
    let idx = state.active_tab;
    let Some(tab) = state.tabs.get_mut(idx) else {
        return false;
    };
    start_stop_on_tab(tab, now_ms)
}

/// Starts the stop sequence on every running tab in `state` (`"Stop all"`, `§3.6`).
/// Returns the number of tabs where a stop sequence was started.
pub fn stop_all(state: &mut TerminalState, now_ms: u64) -> usize {
    let mut started = 0;
    for tab in &mut state.tabs {
        if start_stop_on_tab(tab, now_ms) {
            started += 1;
        }
    }
    started
}

/// True when the tab's PTY has produced no output for [`STOP_QUIET_CLOSE_MS`] since the first interrupt key.
fn pty_quiet_since_stop(tab: &TerminalTab, sent_instant: std::time::Instant) -> bool {
    let last_output = tab.last_output_at.lock().ok().and_then(|g| *g).filter(|ts| *ts > sent_instant).unwrap_or(sent_instant);
    last_output.elapsed() >= std::time::Duration::from_millis(STOP_QUIET_CLOSE_MS)
}

/// Ends the pending stop on `tab` at `now_ms`: records the receipt `"Stopped by you after <N>s"`, latches
/// [`AgentTabStatus::Stopped`], releases `stop_in_flight`, and returns the action that folds the turn. Returns
/// `None` when no stop is pending.
pub fn close_pending_stop(tab: &mut TerminalTab, now_ms: u64) -> Option<TerminalAction> {
    let sent_at = tab.stop_state.as_ref()?.sent_at;
    let started_ms = tab.turn_started_at_ms.take().unwrap_or(sent_at);
    let depth = tab.turn_start_undo_depth;
    tab.stop_state = None;
    tab.stop_in_flight.store(false, std::sync::atomic::Ordering::Relaxed);
    tab.was_working = false;
    tab.still_running_notice = None;
    tab.last_receipt = Some(format_stopped_receipt(started_ms, now_ms));
    // The receipt row and header status of a hooked CLI read the turn tracker: it must say "stopped" too (`#683`).
    tab.turn_tracker.adopt_stop_by_user(sent_at as f64 / 1000.0, now_ms as f64 / 1000.0);
    if let Ok(mut status) = tab.agent_status.lock() {
        *status = AgentTabStatus::Stopped;
    }
    Some(TerminalAction::CloseStoppedTurn { tab_id: tab.id, turn_start_depth: depth, run_id: Some(tab.run_id.clone()) })
}

/// Advances the stop sequence on `tab` at `now_ms` (`§3.3`, `§3.4`, `§3.8`, `§8.1`).
///
/// - If `tab.stop_state` is active and the turn has ended (`!is_tab_turn_running(tab)` or `!tab.is_alive()`),
///   or the CLI fires no interrupt hook and its PTY has been quiet for [`STOP_QUIET_CLOSE_MS`] since the first
///   key, closes the stopped turn through [`close_pending_stop`].
/// - If the turn is still running at `now_ms >= sent_at + 3_000` and `!second_sent`, sends the second
///   interrupt key (`Ctrl+C`) once and sets `second_sent = true`.
/// - If still running after the second key was sent, sets `tab.still_running_notice` to [`STILL_RUNNING_NOTICE`].
pub fn tick_stop_on_tab(tab: &mut TerminalTab, now_ms: u64) -> Option<TerminalAction> {
    let alive = tab.is_alive();
    let running = alive && is_tab_turn_running(tab);
    let (sent_at, second_sent, key_table_row, sent_instant) = {
        let st = tab.stop_state.as_ref()?;
        (st.sent_at, st.second_sent, st.key_table_row, st.sent_instant)
    };
    let quiet_after_interrupt = running && !INTERRUPT_KEY_TABLE[key_table_row].fires_interrupt_hook && pty_quiet_since_stop(tab, sent_instant);

    if !running || quiet_after_interrupt {
        return close_pending_stop(tab, now_ms);
    }

    if !second_sent && now_ms.saturating_sub(sent_at) >= SECOND_KEY_DELAY_MS {
        let row = &INTERRUPT_KEY_TABLE[key_table_row];
        tab.write_bytes(&[row.second_key]);
        tab.stop_sent_keys.push(row.second_key);
        if let Some(st) = tab.stop_state.as_mut() {
            st.second_sent = true;
        }
    } else if second_sent && now_ms.saturating_sub(sent_at) > SECOND_KEY_DELAY_MS {
        tab.still_running_notice = Some(STILL_RUNNING_NOTICE.to_string());
    }
    None
}

/// Polls all tabs in `state` for stop sequence transitions at `now_ms`.
pub fn poll_stop_sequences(state: &mut TerminalState, now_ms: u64) -> Vec<TerminalAction> {
    let mut out = Vec::new();
    for tab in &mut state.tabs {
        if let Some(action) = tick_stop_on_tab(tab, now_ms) {
            out.push(action);
        }
    }
    if let Some(active) = state.tabs.get(state.active_tab) {
        if let Some(notice) = &active.still_running_notice {
            state.status_msg = Some(notice.clone());
        } else if let Some(receipt) = &active.last_receipt {
            if matches!(active.agent_status.lock().ok().as_deref(), Some(AgentTabStatus::Stopped)) {
                state.status_msg = Some(receipt.clone());
            }
        }
    }
    out
}

/// Handles an `Escape` key press in the Agents panel (`§3.1`, `§8.5`).
///
/// - When the terminal viewport has focus (`state.viewport_focused`), `Esc` is forwarded unchanged
///   (`0x1b`) to the active tab's PTY (or closes search if `state.search_open`), and NEVER stops the turn.
/// - When panel chrome has focus (`state.chrome_focused && !state.viewport_focused`) and the composer
///   text field is NOT focused (`!composer_focused`), `Esc` triggers [`start_stop_active`].
///
/// Returns `true` if `start_stop_active` was triggered.
pub fn handle_panel_escape(state: &mut TerminalState, composer_focused: bool, now_ms: u64) -> bool {
    if state.viewport_focused {
        if state.search_open {
            state.search_open = false;
        } else if let Some(tab) = state.active_tab_mut() {
            tab.write_bytes(&[KEY_ESC]);
            tab.stop_sent_keys.push(KEY_ESC);
        }
        return false;
    }
    if state.chrome_focused && !composer_focused {
        return start_stop_active(state, now_ms);
    }
    false
}

/// Deferred stop close parameters saved when a stop completes while a user canvas edit is active (`§3.4`, `§3.5`).
#[derive(Clone, Debug)]
pub struct PendingStopClose {
    pub tab_id: usize,
    /// Byte offset in `doc-patch.jsonl` where the stopped turn ends; patches from here on belong to no turn.
    pub end_patch_offset: u64,
    pub turn_start_depth: usize,
    pub pre_close_patch_count: usize,
    pub run_id: Option<String>,
    /// When the stop key was sent. A ledger turn opened after it belongs to the next prompt, not to the stop.
    pub stop_sent: Option<std::time::Instant>,
}

/// Clears the `Document::in_progress` flags of the stopped tab's own run (`§3.4`). Matching is by `run_id` only: another
/// tab running the same agent in parallel has a different run id and keeps its flags.
pub fn clear_stopped_in_progress(app: &mut App, run_id: Option<&str>) {
    let cleared = app.doc.clear_in_progress_for_run_or_agent(run_id, None);
    if !cleared.is_empty() {
        app.changed();
    }
}

/// Folds the stopped turn's agent undo steps (`depth..`) into one [`STOP_UNDO_LABEL`] step, keeps every
/// recorded history depth valid across the fold, ends the stopped tab's open ledger turn at
/// `end_patch_offset`, and clears the stopped run's `Document::in_progress` flags (`§3.4`).
///
/// Every tab's ledger and running-turn start depth index the one shared `app.hist`, so all of them are
/// remapped, not only the stopped tab's.
pub fn fold_stopped_turn(app: &mut App, depth: usize, close: Option<&PendingStopClose>) {
    let fold = app.hist.fold_since_range(depth, STOP_UNDO_LABEL);
    app.terminal.current_undo_depth = app.hist.undo_depth();
    if let Some((start, end)) = fold {
        app.terminal.turn_ledger.on_history_fold(start, end);
        for tab in &mut app.terminal.tabs {
            tab.turn_ledger.on_history_fold(start, end);
            // A turn that began before the fold ran (the next prompt closes the stop) started above the folded range.
            tab.turn_start_undo_depth = crate::agent_turn_ledger::depth_after_fold(tab.turn_start_undo_depth, start, end);
        }
    }
    let tab_idx = close.and_then(|c| app.terminal.tabs.iter().position(|t| t.id == c.tab_id));
    if let Some(idx) = tab_idx {
        let end_offset = close.map_or_else(|| app.terminal.live_patch.current_offset(), |c| c.end_patch_offset);
        let stop_sent = close.and_then(|c| c.stop_sent);
        app.terminal.ledger_for_tab_mut(idx).on_turn_stopped(end_offset, std::time::Instant::now(), stop_sent);
    }
    clear_stopped_in_progress(app, close.and_then(|c| c.run_id.as_deref()));
}

/// Closes a stopped agent turn on `app` (`§3.4`):
/// 1. Ingests any remaining lines from `doc-patch.jsonl` and applies held live patches (unless the user
///    is mid-edit, in which case the fold and post-replay `in_progress` clear are deferred to
///    `apply_pending_live_patches` when the edit ends).
/// 2. Folds the turn's contiguous agent undo steps since `turn_start_depth` into one `Command::Batch`
///    step labelled [`STOP_UNDO_LABEL`] and ends the tab's turn in the turn ledger (`#677`).
/// 3. Clears `Document::in_progress` flags owned by the stopped agent run.
pub fn close_stopped_turn_on_app(app: &mut App, tab_id: usize, turn_start_depth: usize, run_id: Option<&str>) {
    let (lines, errors) = app.terminal.live_patch.poll();
    for err in errors {
        app.show_toast(err, true);
    }
    if !lines.is_empty() {
        for err in app.terminal.live_patch.ingest(lines) {
            app.show_toast(err, true);
        }
    }
    let close = PendingStopClose {
        tab_id,
        end_patch_offset: app.terminal.live_patch.current_offset(),
        turn_start_depth,
        pre_close_patch_count: app.terminal.live_patch.pending.len(),
        run_id: run_id.map(str::to_string),
        stop_sent: app.terminal.tabs.iter().find(|t| t.id == tab_id).and_then(|t| t.stop_sent_instant),
    };
    if crate::live_patch_apply::user_edit_in_progress(app) {
        app.terminal.pending_stop_fold_depth = Some(turn_start_depth);
        clear_stopped_in_progress(app, run_id);
        app.terminal.pending_stop_close = Some(close);
    } else {
        // The fold runs inside `apply_pending_live_patches`, after the held patches are in, even when none are held.
        app.terminal.pending_stop_fold_depth = Some(turn_start_depth);
        app.terminal.pending_stop_close = Some(close);
        crate::live_patch_apply::apply_pending_live_patches(app);
    }
}

/// Paints the secondary `"Stop"` button (and `"Stop all"` when 2+ tabs are running) in the panel
/// header strip at the right (`§7`), returning the left edge of the painted control group so the
/// tab strip knows where to stop.
pub fn paint_header_stop_controls(ui: &mut Ui, p: &Painter, state: &mut TerminalState, bar_r: Rect, right_anchor_x: f32) -> f32 {
    let now = designos_doc::now_ms();
    let running_count = running_tab_count(state);
    let active_running = state.tabs.get(state.active_tab).is_some_and(is_tab_turn_running);
    let active_stopping = state.tabs.get(state.active_tab).is_some_and(|t| t.stop_state.is_some());

    if !active_running && running_count < 2 {
        return right_anchor_x;
    }

    let btn_h = theme::CONTROL_STANDARD;
    let btn_y = bar_r.center().y - btn_h * 0.5;
    let mut cursor_right = right_anchor_x - theme::SPACE_INLINE;

    if active_running {
        let label = state.tabs.get(state.active_tab).and_then(|t| t.stop_state.as_ref()).map_or("Stop", StopState::button_label);
        let btn_w = theme::CONTROL_ROW + theme::SPACE_SECTION;
        let stop_rect = Rect::from_min_size(Pos2::new(cursor_right - btn_w, btn_y), Vec2::new(btn_w, btn_h));
        let resp = ui.interact(stop_rect, Id::new("agents-header-stop-btn"), Sense::click());
        let bg = if !active_stopping && resp.hovered() { theme::BG_HOVER } else { theme::BG_SECONDARY };
        p.rect_filled(stop_rect, theme::cr(theme::RADIUS_CONTROL), bg);
        p.rect_stroke(stop_rect, theme::cr(theme::RADIUS_CONTROL), Stroke::new(theme::BORDER_WIDTH, theme::BORDER), StrokeKind::Inside);

        let sq_side = theme::SPACE_INLINE;
        let sq_center = Pos2::new(stop_rect.left() + theme::INSET_BUTTON, stop_rect.center().y);
        let sq_rect = Rect::from_center_size(sq_center, Vec2::splat(sq_side));
        let fg = if active_stopping { theme::TEXT_SECONDARY } else { theme::TEXT };
        p.rect_filled(sq_rect, theme::cr(theme::RADIUS_MARK), fg);
        p.text(Pos2::new(sq_rect.right() + theme::SPACE_FACT, stop_rect.center().y), Align2::LEFT_CENTER, label, theme::font_caption_medium(), fg);
        widgets::record(ui, "agents-header-stop-btn", stop_rect);
        if !active_stopping && resp.clicked() {
            start_stop_active(state, now);
        }
        cursor_right = stop_rect.left();
        if state.tabs.get(state.active_tab).is_some_and(|t| t.stop_state.is_some() && t.still_running_notice.is_none()) {
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(100));
        }
    }

    if running_count >= 2 {
        let btn_w = theme::CONTROL_ROW + theme::SPACE_SECTION + theme::SPACE_INLINE;
        let right_x = if active_running { cursor_right - theme::SPACE_INLINE } else { cursor_right };
        let all_rect = Rect::from_min_size(Pos2::new(right_x - btn_w, btn_y), Vec2::new(btn_w, btn_h));
        let resp = ui.interact(all_rect, Id::new("agents-header-stop-all-btn"), Sense::click());
        let bg = if resp.hovered() { theme::BG_HOVER } else { theme::BG_SECONDARY };
        p.rect_filled(all_rect, theme::cr(theme::RADIUS_CONTROL), bg);
        p.rect_stroke(all_rect, theme::cr(theme::RADIUS_CONTROL), Stroke::new(theme::BORDER_WIDTH, theme::BORDER), StrokeKind::Inside);
        p.text(all_rect.center(), Align2::CENTER_CENTER, "Stop all", theme::font_caption_medium(), theme::TEXT);
        widgets::record(ui, "agents-header-stop-all-btn", all_rect);
        if resp.clicked() {
            stop_all(state, now);
        }
        cursor_right = all_rect.left();
    }

    cursor_right
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canvas::Interaction;
    use designos_doc::live_patch::{LiveOp, LivePatch, SiblingHint};
    use designos_doc::{Command, Document, History, InProgress, Node, NodeId, Rect as DocRect};
    use portable_pty::CommandBuilder;

    fn new_test_app() -> App {
        let mut doc = Document::new("Test");
        doc.add_page("Page 1");
        App::new(doc)
    }

    /// Spawns `script` in a PTY-backed tab that reports `READY`, then marks the tab as running a turn.
    fn spawn_script_stub_tab(id: usize, preset: AgentCliPreset, script: &str) -> TerminalTab {
        let mut cmd = CommandBuilder::new("sh");
        cmd.arg("-c");
        cmd.arg(script);
        let mut tab = TerminalTab::spawn_test_stub(id, preset, cmd).expect("spawn stub tab");
        let ready_deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while std::time::Instant::now() < ready_deadline {
            if tab.raw_log.lock().ok().is_some_and(|l| l.contains("READY")) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        if let Ok(mut st) = tab.agent_status.lock() {
            *st = AgentTabStatus::Working(Some("Editing layers…".into()));
        }
        tab.turn_started_at_ms = Some(1_000);
        tab
    }

    /// A silent CLI: exits after reading `1` (or, with `ignore_first_key`, `2`) raw bytes.
    fn spawn_looping_stub_tab(id: usize, preset: AgentCliPreset, ignore_first_key: bool) -> TerminalTab {
        let reads = if ignore_first_key { 2 } else { 1 };
        let script = format!(
            "python3 -u -c 'import sys, tty\ntty.setraw(sys.stdin.fileno())\nsys.stdout.write(\"READY\\n\")\nsys.stdout.flush()\nfor _ in range({reads}):\n    sys.stdin.buffer.read(1)\nsys.exit(0)\n'"
        );
        spawn_script_stub_tab(id, preset, &script)
    }

    /// A CLI that keeps painting (a dot every 50 ms, like a spinner) and exits after reading `reads` raw bytes.
    fn spawn_chatty_stub_tab(id: usize, preset: AgentCliPreset, reads: usize) -> TerminalTab {
        let script = format!(
            "python3 -u -c 'import os, select, sys, time, tty\ntty.setraw(sys.stdin.fileno())\nsys.stdout.write(\"READY\\n\")\nsys.stdout.flush()\ngot = 0\nend = time.time() + 30\nwhile time.time() < end:\n    r, _, _ = select.select([0], [], [], 0.05)\n    if r:\n        os.read(0, 1)\n        got += 1\n        if got >= {reads}:\n            sys.exit(0)\n    sys.stdout.write(\".\")\n    sys.stdout.flush()\n'"
        );
        spawn_script_stub_tab(id, preset, &script)
    }

    /// A Claude-like CLI after Esc: it interrupts, repaints its prompt once, then stays alive and goes quiet.
    /// The Stop hook does not fire for a user interrupt, so no end-of-turn record is ever written.
    fn spawn_quiet_after_interrupt_stub_tab(id: usize, preset: AgentCliPreset) -> TerminalTab {
        let script = "python3 -u -c 'import sys, time, tty\ntty.setraw(sys.stdin.fileno())\nsys.stdout.write(\"READY\\n\")\nsys.stdout.flush()\nsys.stdin.buffer.read(1)\nsys.stdout.write(\"\\r\\n> \")\nsys.stdout.flush()\ntime.sleep(20)\n'";
        spawn_script_stub_tab(id, preset, script)
    }

    #[test]
    fn stop_ends_a_looping_stub_within_three_seconds() {
        designos_doc::set_mock_now_ms(Some(1_000));
        let mut tab = spawn_looping_stub_tab(1, AgentCliPreset::Claude, false);
        assert!(is_tab_turn_running(&tab));

        assert!(start_stop_on_tab(&mut tab, 1_000));
        assert_eq!(tab.stop_sent_keys, vec![KEY_ESC]);

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        let mut action = None;
        while std::time::Instant::now() < deadline {
            if let Some(act) = tick_stop_on_tab(&mut tab, 2_000) {
                action = Some(act);
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        assert!(action.is_some(), "looping stub should exit and close its turn within 3 seconds");
        assert_eq!(tab.agent_status.lock().unwrap().clone(), AgentTabStatus::Stopped);
        assert!(tab.stop_state.is_none());
        designos_doc::set_mock_now_ms(None);
    }

    #[test]
    fn second_key_is_sent_only_if_still_running() {
        // Case A: a stub that keeps painting and ignores the first key (Esc) -> gets Ctrl+C at t = 3s, and no third key.
        let mut tab_ignores = spawn_chatty_stub_tab(1, AgentCliPreset::Claude, 2);
        assert!(start_stop_on_tab(&mut tab_ignores, 1_000));
        assert_eq!(tab_ignores.stop_sent_keys, vec![KEY_ESC]);
        assert_eq!(tab_ignores.stop_state.as_ref().unwrap().button_label(), "Stopping…");

        // Before 3s: no second key sent.
        assert!(tick_stop_on_tab(&mut tab_ignores, 3_999).is_none());
        assert_eq!(tab_ignores.stop_sent_keys, vec![KEY_ESC]);

        // At t = 1_000 + 3_000 = 4_000: still running -> second key (Ctrl+C) sent once.
        assert!(tick_stop_on_tab(&mut tab_ignores, 4_000).is_none());
        assert_eq!(tab_ignores.stop_sent_keys, vec![KEY_ESC, KEY_CTRL_C]);
        assert_eq!(tab_ignores.stop_state.as_ref().unwrap().button_label(), "Stopping…");

        // Wait briefly for the stub to exit after receiving the second byte.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while std::time::Instant::now() < deadline && tab_ignores.is_alive() {
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        let closed = tick_stop_on_tab(&mut tab_ignores, 4_500);
        assert!(closed.is_some());
        assert_eq!(tab_ignores.stop_sent_keys.len(), 2, "never sends a third key");

        // Case B: Stub exits on the first key -> gets NO second key at t = 3s.
        let mut tab_exits = spawn_looping_stub_tab(2, AgentCliPreset::Claude, false);
        assert!(start_stop_on_tab(&mut tab_exits, 1_000));
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while std::time::Instant::now() < deadline && tab_exits.is_alive() {
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        let closed_b = tick_stop_on_tab(&mut tab_exits, 2_000);
        assert!(closed_b.is_some());
        // Advancing past 3s sends nothing because the turn already ended.
        assert!(tick_stop_on_tab(&mut tab_exits, 5_000).is_none());
        assert_eq!(tab_exits.stop_sent_keys, vec![KEY_ESC], "stub that exited after first key must not receive a second key");
    }

    #[test]
    fn no_key_is_sent_after_the_turn_ended() {
        let mut tab = spawn_looping_stub_tab(1, AgentCliPreset::Grok, false);
        if let Ok(mut st) = tab.agent_status.lock() {
            *st = AgentTabStatus::Done;
        }
        assert!(!start_stop_on_tab(&mut tab, 1_000), "must not start stop when turn already ended");
        assert!(tab.stop_sent_keys.is_empty(), "zero bytes sent after Done");
        assert!(tick_stop_on_tab(&mut tab, 5_000).is_none());
        assert!(tab.stop_sent_keys.is_empty());
    }

    #[test]
    fn fold_since_makes_the_tail_one_undo_step() {
        let mut d = Document::new("t");
        let page = d.add_page("P1");
        let mut h = History::default();

        let r0 = d.alloc_id();
        h.exec_user(&mut d, Command::Insert { parent: page, index: 0, nodes: vec![Node::rectangle(r0, DocRect::new(0.0, 0.0, 20.0, 20.0))] }).unwrap();
        let start_depth = h.undo_depth();
        let start_hash = crate::home::doc_hash(&d);

        for i in 0..3 {
            let rid = d.alloc_id();
            h.exec_agent(
                &mut d,
                Command::Insert { parent: page, index: i + 1, nodes: vec![Node::rectangle(rid, DocRect::new((i as f32) * 30.0, 0.0, 20.0, 20.0))] },
            )
            .unwrap();
        }
        assert_eq!(h.undo_depth(), start_depth + 3);

        let folded = h.fold_since(start_depth, STOP_UNDO_LABEL);
        assert_eq!(folded, 3);
        assert_eq!(h.undo_depth(), start_depth + 1);
        assert_eq!(h.undo_label(), Some(STOP_UNDO_LABEL));

        assert!(h.undo(&mut d).unwrap());
        assert_eq!(crate::home::doc_hash(&d), start_hash);
    }

    #[test]
    fn fold_stops_at_a_user_step() {
        let mut d = Document::new("t");
        let page = d.add_page("P1");
        let mut h = History::default();
        let start_depth = h.undo_depth();

        let a1 = d.alloc_id();
        h.exec_agent(&mut d, Command::Insert { parent: page, index: 0, nodes: vec![Node::rectangle(a1, DocRect::new(0.0, 0.0, 10.0, 10.0))] }).unwrap();
        let u1 = d.alloc_id();
        h.exec_user(&mut d, Command::Insert { parent: page, index: 1, nodes: vec![Node::rectangle(u1, DocRect::new(20.0, 0.0, 10.0, 10.0))] }).unwrap();
        let a2 = d.alloc_id();
        h.exec_agent(&mut d, Command::Insert { parent: page, index: 2, nodes: vec![Node::rectangle(a2, DocRect::new(40.0, 0.0, 10.0, 10.0))] }).unwrap();
        let a3 = d.alloc_id();
        h.exec_agent(&mut d, Command::Insert { parent: page, index: 3, nodes: vec![Node::rectangle(a3, DocRect::new(60.0, 0.0, 10.0, 10.0))] }).unwrap();

        let folded = h.fold_since(start_depth, STOP_UNDO_LABEL);
        assert_eq!(folded, 2);
        assert_eq!(h.undo_depth(), 3);

        assert!(h.undo(&mut d).unwrap());
        assert!(d.get(a2).is_none() && d.get(a3).is_none());
        assert!(d.get(u1).is_some() && d.get(a1).is_some());
    }

    #[test]
    fn held_patches_apply_before_the_fold() {
        let mut app = new_test_app();
        let page = app.page;
        let start_depth = app.hist.undo_depth();

        // Simulate user mid-edit (panning/dragging on canvas) while two agent patches are held.
        app.canvas.inter = Interaction::Pan;
        let n1 = Node::rectangle(designos_doc::NodeId(500), DocRect::new(10.0, 10.0, 80.0, 40.0));
        let n2 = Node::rectangle(designos_doc::NodeId(501), DocRect::new(100.0, 10.0, 80.0, 40.0));
        app.terminal.live_patch.pending.push(LivePatch::new(vec![LiveOp::Insert { parent: page, at: SiblingHint::default(), nodes: vec![n1] }]));
        app.terminal.live_patch.pending.push(LivePatch::new(vec![LiveOp::Insert { parent: page, at: SiblingHint::default(), nodes: vec![n2] }]));

        // Stop closes the turn while the user is still mid-edit.
        close_stopped_turn_on_app(&mut app, 1, start_depth, Some("run-held"));
        assert_eq!(app.terminal.pending_stop_fold_depth, Some(start_depth));

        // User finishes the edit -> held patches apply and fold into one undo step.
        app.canvas.inter = Interaction::None;
        crate::live_patch_apply::apply_pending_live_patches(&mut app);

        assert_eq!(app.doc.get(page).unwrap().children.len(), 2, "both held patches must be in the document");
        assert_eq!(app.hist.undo_depth(), start_depth + 1, "both held patches must fold into one undo step");
        assert_eq!(app.hist.undo_label(), Some(STOP_UNDO_LABEL));
    }

    fn add_frame(app: &mut App, x: f32) -> NodeId {
        let page = app.page;
        let id = app.doc.alloc_id();
        app.doc.insert(page, Node::frame(id, DocRect::new(x, 0.0, 400.0, 300.0))).unwrap()
    }

    /// Stamps `frame` with an `in_progress` flag owned by `run_id` / `agent`.
    fn flag_frame(app: &mut App, frame: NodeId, run_id: &str, agent: Option<&str>, status: &str) {
        app.doc.set_in_progress(
            frame,
            Some(InProgress {
                run_id: Some(run_id.into()),
                agent: agent.map(str::to_string),
                since_ms: 10_000,
                stale_after_s: Some(300),
                status: Some(status.into()),
            }),
        );
    }

    /// Stops `tab` the way the Stop button does and runs the resulting action through the app, as `shell.rs` does.
    fn stop_tab_and_close(app: &mut App, tab_idx: usize, depth: usize) {
        let tab = &mut app.terminal.tabs[tab_idx];
        assert!(start_stop_on_tab(tab, 1_000));
        if let Ok(mut st) = tab.agent_status.lock() {
            *st = AgentTabStatus::Done;
        }
        let action = tick_stop_on_tab(tab, 2_000).expect("a stopped turn closes with an action");
        let TerminalAction::CloseStoppedTurn { tab_id, run_id, .. } = action else { panic!("unexpected action {action:?}") };
        close_stopped_turn_on_app(app, tab_id, depth, run_id.as_deref());
    }

    #[test]
    fn in_progress_flags_clear_on_stop() {
        designos_doc::set_mock_now_ms(Some(10_000));
        let mut app = new_test_app();
        let mut tab = spawn_looping_stub_tab(1, AgentCliPreset::Claude, false);
        tab.run_id = "sess-shared-1-1".to_string();
        app.terminal.tabs.push(tab);
        let (frame, frame2) = (add_frame(&mut app, 0.0), add_frame(&mut app, 450.0));
        let hash_without_flags = crate::home::doc_hash(&app.doc);
        flag_frame(&mut app, frame, "sess-shared-1-1", Some("Claude Code"), "Building hero...");
        flag_frame(&mut app, frame2, "sess-shared-1-1", None, "Building nav...");
        let hash_with_flags = crate::home::doc_hash(&app.doc);
        assert_ne!(hash_with_flags, hash_without_flags, "doc_hash must include in_progress flags");
        assert!(app.doc.get_in_progress(frame).is_some());
        assert!(app.doc.get_in_progress(frame2).is_some());

        let depth = app.hist.undo_depth();
        stop_tab_and_close(&mut app, 0, depth);
        assert!(app.doc.get_in_progress(frame).is_none());
        assert!(app.doc.get_in_progress(frame2).is_none());
        assert!(app.doc.in_progress.is_empty());
        assert_eq!(crate::home::doc_hash(&app.doc), hash_without_flags, "clearing in_progress must change doc_hash back");
        let _ = app.terminal.tabs[0].child.kill();
        designos_doc::set_mock_now_ms(None);
    }

    #[test]
    fn live_spec_flag_without_run_id_clears_on_stop_of_the_writing_tab() {
        designos_doc::set_mock_now_ms(Some(10_000));
        let mut app = new_test_app();
        let mut tab = spawn_looping_stub_tab(1, AgentCliPreset::Claude, false);
        tab.run_id = "sess-live-1-1".to_string();
        app.terminal.tabs.push(tab);
        let spec_json = |ip: &str| format!(r#"{{"kind":"frame","name":"Live","width":200,"height":100,"in_progress":{ip}}}"#);
        let detailed: designos_doc::NodeSpec = serde_json::from_str(&spec_json(r#"{"agent":"claude","status":"Building..."}"#)).unwrap();
        let boolean: designos_doc::NodeSpec = serde_json::from_str(&spec_json("true")).unwrap();
        app.apply_node_spec(&detailed, "detailed");
        let detailed_root = app.selection[0];
        app.apply_node_spec(&boolean, "boolean");
        let boolean_root = app.selection[0];
        for id in [detailed_root, boolean_root] {
            assert_eq!(app.doc.get_in_progress(id).and_then(|ip| ip.run_id.as_deref()), Some("sess-live-1-1"), "writer run id stamped");
        }

        let depth = app.hist.undo_depth();
        stop_tab_and_close(&mut app, 0, depth);
        assert!(app.doc.in_progress.is_empty(), "Stop clears live-spec flags of the writing tab");
        let _ = app.terminal.tabs[0].child.kill();
        designos_doc::set_mock_now_ms(None);
    }

    /// An app whose only tab is a Claude agent tab with `run_id`, so a spec applied now is attributed to that tab.
    fn app_with_agent_tab(run_id: &str) -> App {
        let mut app = new_test_app();
        let mut tab = spawn_looping_stub_tab(1, AgentCliPreset::Claude, false);
        tab.run_id = run_id.to_string();
        app.terminal.tabs.push(tab);
        app
    }

    /// A frame carrying a live flag with no run id, as a Specs-panel apply or a host without a session leaves one.
    fn add_runless_flagged_frame(app: &mut App, x: f32) -> NodeId {
        let frame = add_frame(app, x);
        app.doc.set_in_progress(
            frame,
            Some(InProgress {
                run_id: None,
                agent: Some("Claude Code".into()),
                since_ms: 10_000,
                stale_after_s: Some(300),
                status: Some("Other writer building...".into()),
            }),
        );
        frame
    }

    fn apply_spec_json(app: &mut App, json: serde_json::Value, label: &str) -> NodeId {
        let spec: designos_doc::NodeSpec = serde_json::from_value(json).expect("valid NodeSpec");
        app.apply_node_spec(&spec, label);
        app.selection[0]
    }

    fn flagged_spec() -> serde_json::Value {
        serde_json::json!({ "kind": "frame", "name": "Live", "width": 200, "height": 100, "in_progress": { "agent": "claude", "status": "Building..." } })
    }

    fn kill_tabs(app: &mut App) {
        for tab in &mut app.terminal.tabs {
            let _ = tab.child.kill();
        }
    }

    #[test]
    fn applying_a_spec_does_not_give_the_tab_run_id_to_another_writers_flag_so_stop_leaves_it() {
        designos_doc::set_mock_now_ms(Some(10_000));
        let mut app = app_with_agent_tab("sess-live-1-1");
        let unrelated = add_runless_flagged_frame(&mut app, 0.0);

        let spec_root = apply_spec_json(&mut app, flagged_spec(), "live");
        assert_eq!(app.doc.get_in_progress(spec_root).and_then(|ip| ip.run_id.as_deref()), Some("sess-live-1-1"), "the spec's own flag gets the writer run id");
        assert_eq!(app.doc.get_in_progress(unrelated).map(|ip| ip.run_id.clone()), Some(None), "a flag the spec did not set keeps no run id");

        let depth = app.hist.undo_depth();
        stop_tab_and_close(&mut app, 0, depth);
        assert!(app.doc.get_in_progress(spec_root).is_none(), "Stop clears the flag the stopped tab set");
        assert!(app.doc.get_in_progress(unrelated).is_some(), "Stop leaves the flag another writer set");
        kill_tabs(&mut app);
        designos_doc::set_mock_now_ms(None);
    }

    #[test]
    fn applying_a_spec_does_not_give_the_tab_run_id_to_another_writers_flag_so_turn_end_leaves_it() {
        designos_doc::set_mock_now_ms(Some(10_000));
        let mut app = app_with_agent_tab("sess-live-1-1");
        let unrelated = add_runless_flagged_frame(&mut app, 0.0);
        let spec_root = apply_spec_json(&mut app, flagged_spec(), "live");

        let end = TerminalAction::ClearInProgress { run_id: Some("sess-live-1-1".into()), agent: Some("Claude".into()) };
        crate::shell::execute_terminal_actions(&mut app, vec![end]);
        assert!(app.doc.get_in_progress(spec_root).is_none(), "turn end clears the flag the tab set");
        assert_eq!(app.doc.get_in_progress(unrelated).map(|ip| ip.run_id.clone()), Some(None), "turn end leaves the flag another writer set");
        kill_tabs(&mut app);
        designos_doc::set_mock_now_ms(None);
    }

    #[test]
    fn nested_spec_flags_without_a_run_id_get_the_agent_tab_run_id_and_explicit_ones_keep_theirs() {
        designos_doc::set_mock_now_ms(Some(10_000));
        let mut app = app_with_agent_tab("sess-live-1-1");
        let unrelated = add_runless_flagged_frame(&mut app, 0.0);
        let section = apply_spec_json(
            &mut app,
            serde_json::json!({
                "kind": "section", "name": "Spec Section", "width": 800, "height": 600,
                "children": [
                    { "kind": "frame", "name": "Implicit", "width": 300, "height": 200, "in_progress": { "agent": "claude" } },
                    { "kind": "frame", "name": "Explicit", "width": 300, "height": 200, "in_progress": { "agent": "claude", "run_id": "run-explicit" } }
                ]
            }),
            "nested",
        );
        let children = app.doc.get(section).unwrap().children.clone();
        let (implicit, explicit) = (children[0], children[1]);
        assert_eq!(
            app.doc.get_in_progress(implicit).and_then(|ip| ip.run_id.as_deref()),
            Some("sess-live-1-1"),
            "a nested flag without a run id gets the writer's"
        );
        assert_eq!(app.doc.get_in_progress(explicit).and_then(|ip| ip.run_id.as_deref()), Some("run-explicit"), "an explicit run id is kept");
        assert_eq!(app.doc.get_in_progress(unrelated).map(|ip| ip.run_id.clone()), Some(None), "the unrelated flag is still untouched");

        let depth = app.hist.undo_depth();
        stop_tab_and_close(&mut app, 0, depth);
        assert!(app.doc.get_in_progress(implicit).is_none(), "Stop clears the nested flag stamped with the tab's run id");
        assert!(app.doc.get_in_progress(explicit).is_some(), "Stop keeps a nested flag owned by another run");
        assert!(app.doc.get_in_progress(unrelated).is_some());
        kill_tabs(&mut app);
        designos_doc::set_mock_now_ms(None);
    }

    #[test]
    fn stopping_one_tab_keeps_the_in_progress_flags_of_a_parallel_tab_of_the_same_agent() {
        designos_doc::set_mock_now_ms(Some(10_000));
        let mut app = new_test_app();
        for (id, run_id) in [(1, "sess-shared-1-1"), (2, "sess-shared-2-1")] {
            let mut tab = spawn_looping_stub_tab(id, AgentCliPreset::Claude, false);
            tab.run_id = run_id.to_string();
            app.terminal.tabs.push(tab);
        }
        let (stopped, stopped_sibling, parallel) = (add_frame(&mut app, 0.0), add_frame(&mut app, 450.0), add_frame(&mut app, 900.0));
        flag_frame(&mut app, stopped, "sess-shared-1-1", Some("Claude Code"), "Tab 1 building...");
        flag_frame(&mut app, stopped_sibling, "sess-shared-1-1", None, "Tab 1 nav...");
        flag_frame(&mut app, parallel, "sess-shared-2-1", Some("Claude Code"), "Tab 2 building...");

        let depth = app.hist.undo_depth();
        stop_tab_and_close(&mut app, 0, depth);

        assert!(app.doc.get_in_progress(stopped).is_none(), "the stopped tab's own flag clears");
        assert!(app.doc.get_in_progress(stopped_sibling).is_none(), "the stopped tab's second flag clears");
        assert_eq!(
            app.doc.get_in_progress(parallel).and_then(|ip| ip.run_id.as_deref()),
            Some("sess-shared-2-1"),
            "a parallel tab of the same agent keeps its flag"
        );
        for tab in &mut app.terminal.tabs {
            let _ = tab.child.kill();
        }
        designos_doc::set_mock_now_ms(None);
    }

    #[test]
    fn receipt_says_stopped_by_you() {
        assert_eq!(format_stopped_receipt(1_000, 43_000), "Stopped by you after 42s");

        let mut tab = spawn_looping_stub_tab(1, AgentCliPreset::Claude, false);
        tab.turn_started_at_ms = Some(1_000);
        assert!(start_stop_on_tab(&mut tab, 42_500));
        if let Ok(mut st) = tab.agent_status.lock() {
            *st = AgentTabStatus::Done;
        }
        let action = tick_stop_on_tab(&mut tab, 43_000);
        assert!(action.is_some());
        assert_eq!(tab.last_receipt.as_deref(), Some("Stopped by you after 42s"));
    }

    #[test]
    fn stop_after_user_shapes_and_earlier_turn_only_folds_active_turn_and_resets_receipt_clock() {
        designos_doc::set_mock_now_ms(Some(1_000));
        let mut app = new_test_app();
        let page = app.page;
        let _ = std::fs::create_dir_all(&app.terminal.session_dir);
        let events_file = app.terminal.session_dir.join("agent-events.jsonl");
        let _ = std::fs::remove_file(&events_file);
        let append_event = |t: f64, event: &str| {
            use std::io::Write;
            let mut f = std::fs::OpenOptions::new().create(true).append(true).open(&events_file).unwrap();
            writeln!(f, "{}", serde_json::json!({ "t": t, "e": event, "p": { "session_id": "sess-684" } })).unwrap();
        };

        // User draws two shapes via Tx::commit (and/or History::exec).
        for i in 0..2 {
            let mut tx = crate::edit::Tx::new();
            let uid = app.doc.alloc_id();
            tx.cmd(
                &mut app.doc,
                Command::Insert { parent: page, index: i, nodes: vec![Node::rectangle(uid, DocRect::new((i as f32) * 20.0, 0.0, 10.0, 10.0))] },
            )
            .unwrap();
            assert!(tx.commit(&mut app.doc, &mut app.hist).unwrap());
        }
        assert_eq!(app.hist.undo_depth(), 2);

        let tab = spawn_looping_stub_tab(1, AgentCliPreset::Claude, true);
        if let Ok(mut st) = tab.agent_status.lock() {
            *st = AgentTabStatus::Idle;
        }
        app.terminal.tabs.push(tab);
        app.terminal.active_tab = 0;

        // Turn 1 starts at t = 1_000, applies 2 agent patches, and completes normally at t = 10_000.
        crate::live_patch_apply::publish_live_context(&mut app);
        append_event(1.0, "UserPromptSubmit");
        let actions = app.terminal.poll_live_agent_bridge();
        crate::shell::execute_terminal_actions(&mut app, actions);

        let t1_a = Node::rectangle(designos_doc::NodeId(600), DocRect::new(100.0, 0.0, 20.0, 20.0));
        let t1_b = Node::rectangle(designos_doc::NodeId(601), DocRect::new(130.0, 0.0, 20.0, 20.0));
        crate::live_patch_apply::apply_live_patches(
            &mut app,
            &[
                LivePatch::new(vec![LiveOp::Insert { parent: page, at: SiblingHint::default(), nodes: vec![t1_a] }]),
                LivePatch::new(vec![LiveOp::Insert { parent: page, at: SiblingHint::default(), nodes: vec![t1_b] }]),
            ],
        );
        assert_eq!(app.hist.undo_depth(), 4);

        designos_doc::set_mock_now_ms(Some(10_000));
        crate::live_patch_apply::publish_live_context(&mut app);
        append_event(10.0, "Stop");
        let actions = app.terminal.poll_live_agent_bridge();
        crate::shell::execute_terminal_actions(&mut app, actions);

        // Turn 2 starts at t = 50_000, applies 2 agent patches, and is stopped at t = 55_000.
        designos_doc::set_mock_now_ms(Some(50_000));
        crate::live_patch_apply::publish_live_context(&mut app);
        append_event(50.0, "UserPromptSubmit");
        let actions = app.terminal.poll_live_agent_bridge();
        crate::shell::execute_terminal_actions(&mut app, actions);
        assert_eq!(app.terminal.tabs[0].turn_start_undo_depth, 4);
        assert_eq!(app.terminal.tabs[0].turn_started_at_ms, Some(50_000));

        let t2_a = Node::rectangle(designos_doc::NodeId(700), DocRect::new(200.0, 0.0, 20.0, 20.0));
        let t2_b = Node::rectangle(designos_doc::NodeId(701), DocRect::new(230.0, 0.0, 20.0, 20.0));
        crate::live_patch_apply::apply_live_patches(
            &mut app,
            &[
                LivePatch::new(vec![LiveOp::Insert { parent: page, at: SiblingHint::default(), nodes: vec![t2_a] }]),
                LivePatch::new(vec![LiveOp::Insert { parent: page, at: SiblingHint::default(), nodes: vec![t2_b] }]),
            ],
        );
        assert_eq!(app.hist.undo_depth(), 6);

        assert!(start_stop_active(&mut app.terminal, 54_000));
        designos_doc::set_mock_now_ms(Some(55_000));
        append_event(55.0, "Stop");
        let actions = app.terminal.poll_live_agent_bridge();
        crate::shell::execute_terminal_actions(&mut app, actions);

        assert_eq!(app.terminal.tabs[0].last_receipt.as_deref(), Some("Stopped by you after 5s"));
        assert_eq!(app.hist.undo_depth(), 5, "2 user shapes + 2 Turn 1 steps + 1 folded Turn 2 step");
        assert_eq!(app.hist.undo_label(), Some(STOP_UNDO_LABEL));

        // One Undo removes only Turn 2's 2 nodes, preserving Turn 1 and user shapes.
        assert!(app.hist.undo(&mut app.doc).unwrap());
        assert!(app.doc.get(designos_doc::NodeId(700)).is_none() && app.doc.get(designos_doc::NodeId(701)).is_none());
        assert!(app.doc.get(designos_doc::NodeId(600)).is_some() && app.doc.get(designos_doc::NodeId(601)).is_some());
        assert_eq!(app.doc.get(page).unwrap().children.len(), 4);

        let _ = std::fs::remove_file(&events_file);
        designos_doc::set_mock_now_ms(None);
    }

    #[test]
    fn osc_status_in_pty_does_not_erase_stopped_until_new_working_turn() {
        let script = "python3 -u -c 'import sys\nsys.stdout.write(\"READY\\n\")\nsys.stdout.flush()\nline = sys.stdin.readline()\nsys.stdout.write(\"\\x1b]9999;done\\x07AFTER_IDLE\\n\")\nsys.stdout.flush()\nline2 = sys.stdin.readline()\nsys.stdout.write(\"\\x1b]9999;working:Next turn\\x07AFTER_WORK\\n\")\nsys.stdout.flush()\n'";
        let mut cmd = CommandBuilder::new("sh");
        cmd.arg("-c");
        cmd.arg(script);
        let mut tab = TerminalTab::spawn_test_stub(1, AgentCliPreset::Claude, cmd).expect("spawn stub");
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while std::time::Instant::now() < deadline {
            if tab.raw_log.lock().ok().is_some_and(|l| l.contains("READY")) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        if let Ok(mut st) = tab.agent_status.lock() {
            *st = AgentTabStatus::Stopped;
        }
        tab.last_receipt = Some("Stopped by you after 3s".into());

        // Trigger the stub to emit a `done` OSC sequence while the tab is `Stopped`.
        tab.write_bytes(b"go\n");
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while std::time::Instant::now() < deadline {
            if tab.raw_log.lock().ok().is_some_and(|l| l.contains("AFTER_IDLE")) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert_eq!(tab.agent_status.lock().unwrap().clone(), AgentTabStatus::Stopped, "non-working OSC status must not erase Stopped");

        // Trigger the stub to emit a `working` OSC sequence -> transitions out of Stopped.
        tab.write_bytes(b"work\n");
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while std::time::Instant::now() < deadline {
            if tab.raw_log.lock().ok().is_some_and(|l| l.contains("AFTER_WORK")) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(matches!(tab.agent_status.lock().unwrap().clone(), AgentTabStatus::Working(_)), "a new Working OSC status must transition out of Stopped");
    }

    #[test]
    fn esc_in_the_terminal_reaches_the_cli_unchanged() {
        let mut state = TerminalState::default();
        let tab = spawn_looping_stub_tab(1, AgentCliPreset::Grok, true);
        state.tabs.push(tab);
        state.active_tab = 0;
        state.viewport_focused = true;
        state.chrome_focused = false;

        let stopped = handle_panel_escape(&mut state, false, 1_000);
        assert!(!stopped, "Esc inside the terminal must not trigger Stop");
        assert!(state.tabs[0].stop_state.is_none());
        assert_eq!(state.tabs[0].stop_sent_keys, vec![KEY_ESC], "0x1b must be written to the CLI unchanged");
    }

    #[test]
    fn esc_in_the_chrome_stops() {
        let mut state = TerminalState::default();
        let tab = spawn_looping_stub_tab(1, AgentCliPreset::Grok, true);
        state.tabs.push(tab);
        state.active_tab = 0;
        state.viewport_focused = false;
        state.chrome_focused = true;

        // Composer focused (§8.5): Esc leaves the field and does NOT stop.
        assert!(!handle_panel_escape(&mut state, true, 1_000));
        assert!(state.tabs[0].stop_state.is_none());

        // Chrome focused and composer not focused: Esc starts the stop sequence.
        assert!(handle_panel_escape(&mut state, false, 1_000));
        assert!(state.tabs[0].stop_state.is_some());
        assert_eq!(state.tabs[0].stop_sent_keys, vec![KEY_CTRL_C]);
    }

    #[test]
    fn stop_all_stops_every_running_tab() {
        let mut state = TerminalState::default();
        state.tabs.push(spawn_looping_stub_tab(1, AgentCliPreset::Claude, false));
        state.tabs.push(spawn_looping_stub_tab(2, AgentCliPreset::Grok, false));
        assert_eq!(running_tab_count(&state), 2);

        let count = stop_all(&mut state, 1_000);
        assert_eq!(count, 2);
        assert_eq!(state.tabs[0].stop_sent_keys, vec![KEY_ESC]);
        assert_eq!(state.tabs[1].stop_sent_keys, vec![KEY_CTRL_C]);

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        while std::time::Instant::now() < deadline && (state.tabs[0].is_alive() || state.tabs[1].is_alive()) {
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        let actions = poll_stop_sequences(&mut state, 2_500);
        assert_eq!(actions.len(), 2);
        assert_eq!(state.tabs[0].agent_status.lock().unwrap().clone(), AgentTabStatus::Stopped);
        assert_eq!(state.tabs[1].agent_status.lock().unwrap().clone(), AgentTabStatus::Stopped);
    }

    #[test]
    fn codex_heuristic_stop_folds_all_patches_and_preserves_elapsed_receipt() {
        designos_doc::set_mock_now_ms(Some(10_000));
        let mut app = new_test_app();
        let page = app.page;

        let mut tab = spawn_looping_stub_tab(1, AgentCliPreset::Codex, true);
        if let Ok(mut st) = tab.agent_status.lock() {
            *st = AgentTabStatus::Idle;
        }
        tab.was_working = false;
        tab.turn_started_at_ms = None;
        tab.turn_start_undo_depth = 0;
        if let Ok(mut last_out) = tab.last_output_at.lock() {
            *last_out = Some(std::time::Instant::now());
        }
        app.terminal.tabs.push(tab);
        app.terminal.active_tab = 0;

        // First poll at t = 10_000 while Codex is heuristically running (Idle + output < 600ms).
        let actions = app.terminal.poll_live_agent_bridge();
        crate::shell::execute_terminal_actions(&mut app, actions);

        // Codex applies three patches across polls without ever writing `working` to agent-status.json.
        for i in 0..3 {
            designos_doc::set_mock_now_ms(Some(12_000 + (i as u64) * 3_000));
            if let Ok(mut last_out) = app.terminal.tabs[0].last_output_at.lock() {
                *last_out = Some(std::time::Instant::now());
            }
            let nid = designos_doc::NodeId(800 + i as u64);
            let node = Node::rectangle(nid, DocRect::new((i as f32) * 30.0, 0.0, 20.0, 20.0));
            crate::live_patch_apply::apply_live_patches(
                &mut app,
                &[LivePatch::new(vec![LiveOp::Insert { parent: page, at: SiblingHint::default(), nodes: vec![node] }])],
            );
            let actions = app.terminal.poll_live_agent_bridge();
            crate::shell::execute_terminal_actions(&mut app, actions);
        }
        assert_eq!(app.hist.undo_depth(), 3);

        // User hits Stop at t = 21_000; Codex goes quiet and turn closes at t = 22_000 (12s elapsed).
        if let Ok(mut last_out) = app.terminal.tabs[0].last_output_at.lock() {
            *last_out = Some(std::time::Instant::now());
        }
        assert!(start_stop_active(&mut app.terminal, 21_000));
        if let Ok(mut last_out) = app.terminal.tabs[0].last_output_at.lock() {
            *last_out = None;
        }
        designos_doc::set_mock_now_ms(Some(22_000));
        let actions = app.terminal.poll_live_agent_bridge();
        crate::shell::execute_terminal_actions(&mut app, actions);

        assert_eq!(app.hist.undo_depth(), 1, "all 3 Codex patches must fold into 1 undo step on Stop");
        assert_eq!(app.hist.undo_label(), Some(STOP_UNDO_LABEL));
        assert_eq!(app.terminal.tabs[0].last_receipt.as_deref(), Some("Stopped by you after 12s"));
        designos_doc::set_mock_now_ms(None);
    }

    #[test]
    fn stop_during_drag_clears_in_progress_from_held_patches_and_keeps_post_close_patches_separate() {
        designos_doc::set_mock_now_ms(Some(10_000));
        let mut app = new_test_app();
        let page = app.page;
        let start_depth = app.hist.undo_depth();

        // User is dragging on the canvas.
        app.canvas.inter = Interaction::Pan;

        let fid = designos_doc::NodeId(900);
        let frame_node = Node::frame(fid, DocRect::new(0.0, 0.0, 300.0, 200.0));
        let child_node = Node::rectangle(designos_doc::NodeId(901), DocRect::new(10.0, 10.0, 50.0, 50.0));
        app.terminal.live_patch.pending.push(LivePatch::new(vec![
            LiveOp::Insert { parent: page, at: SiblingHint::default(), nodes: vec![frame_node] },
            LiveOp::SetInProgress {
                node: fid,
                state: Some(InProgress {
                    run_id: Some("run-drag-stop".into()),
                    agent: Some("claude".into()),
                    since_ms: 10_000,
                    stale_after_s: Some(300),
                    status: Some("Working...".into()),
                }),
            },
        ]));
        app.terminal.live_patch.pending.push(LivePatch::new(vec![LiveOp::Insert { parent: fid, at: SiblingHint::default(), nodes: vec![child_node] }]));

        // User presses Stop while still dragging.
        close_stopped_turn_on_app(&mut app, 1, start_depth, Some("run-drag-stop"));

        // A late patch arrives after the stopped turn has already closed, while the user is still dragging.
        let late_node = Node::rectangle(designos_doc::NodeId(902), DocRect::new(400.0, 0.0, 40.0, 40.0));
        app.terminal.live_patch.pending.push(LivePatch::new(vec![LiveOp::Insert { parent: page, at: SiblingHint::default(), nodes: vec![late_node] }]));

        // User releases the drag.
        app.canvas.inter = Interaction::None;
        crate::live_patch_apply::apply_pending_live_patches(&mut app);

        assert!(app.doc.get_in_progress(fid).is_none(), "in_progress flag from held pre-close patch must be cleared after deferred stop fold");
        assert!(app.doc.in_progress.is_empty());
        assert_eq!(app.hist.undo_depth(), start_depth + 2, "2 pre-close patches fold into 1 batch + 1 post-close patch stays a single step");
        designos_doc::set_mock_now_ms(None);
    }

    #[test]
    fn stop_preserves_user_redo_while_dropping_agent_redo() {
        let mut d = Document::new("t");
        let page = d.add_page("P1");
        let mut h = History::default();

        let a1 = d.alloc_id();
        h.exec_agent(&mut d, Command::Insert { parent: page, index: 0, nodes: vec![Node::rectangle(a1, DocRect::new(0.0, 0.0, 20.0, 20.0))] }).unwrap();
        let a2 = d.alloc_id();
        h.exec_agent(&mut d, Command::Insert { parent: page, index: 1, nodes: vec![Node::rectangle(a2, DocRect::new(30.0, 0.0, 20.0, 20.0))] }).unwrap();

        // User nudges `a1` to x = 150.0, then presses Undo before hitting Stop.
        let mut nudged = d.get(a1).unwrap().clone();
        nudged.transform.tx = 150.0;
        h.exec_user(&mut d, Command::Update { node: Box::new(nudged) }).unwrap();
        assert_eq!(d.get(a1).unwrap().transform.tx, 150.0);
        assert!(h.undo(&mut d).unwrap());
        assert_eq!(d.get(a1).unwrap().transform.tx, 0.0);
        assert!(h.can_redo());

        // Stop folds the two agent steps without erasing the user's redo entry.
        let folded = h.fold_since(0, STOP_UNDO_LABEL);
        assert_eq!(folded, 2);
        assert_eq!(h.undo_depth(), 1);
        assert!(h.can_redo(), "user redo entry must be preserved across fold_since");
        assert!(h.redo(&mut d).unwrap());
        assert_eq!(d.get(a1).unwrap().transform.tx, 150.0, "Cmd+Shift+Z must restore the user's undone nudge");
    }

    fn hook_record(t: f64, event: &str) -> crate::agent_turn_status::AgentEventRecord {
        crate::agent_turn_status::AgentEventRecord { t, e: event.to_string(), p: serde_json::json!({ "session_id": "sess-684" }) }
    }

    #[test]
    fn stopped_turn_reads_as_stopped_in_the_receipt_row_and_header_status() {
        let mut tab = spawn_looping_stub_tab(1, AgentCliPreset::Claude, false);
        tab.turn_tracker.on_event(&hook_record(1.0, "UserPromptSubmit"));
        assert!(tab.turn_tracker.in_turn);
        assert!(tab.header_status_indicator(10).is_some(), "a running turn shows a header status");

        assert!(start_stop_on_tab(&mut tab, 40_000));
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        let mut action = None;
        while std::time::Instant::now() < deadline && action.is_none() {
            action = tick_stop_on_tab(&mut tab, 43_000);
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        assert!(action.is_some(), "the stub exits on the first key and the turn closes");

        let receipt = tab.last_turn_receipt().expect("the stop leaves a turn receipt for the receipt row");
        assert_eq!(receipt.outcome, crate::agent_turn_status::TurnOutcome::StoppedByUser);
        assert_eq!(receipt.format_line(), "Stopped by you after 42s");
        assert_eq!(tab.effective_status(), AgentTabStatus::Stopped);
        assert!(tab.header_status_indicator(44).is_none(), "a stopped tab shows no running or waiting header status");
        assert!(!tab.turn_tracker.in_turn);
    }

    #[test]
    fn completed_receipt_that_raced_the_stop_becomes_stopped() {
        let mut tracker = crate::agent_turn_status::TabTurnTracker::default();
        tracker.on_event(&hook_record(100.0, "UserPromptSubmit"));
        tracker.on_event(&hook_record(130.0, "Stop"));
        assert_eq!(tracker.last_turn_receipt.as_ref().map(|r| r.outcome.clone()), Some(crate::agent_turn_status::TurnOutcome::Completed));

        // The user pressed Stop at t = 129.5 s; the hook's whole-second timestamp landed just after.
        let receipt = tracker.adopt_stop_by_user(129.5, 131.0).expect("racing completed receipt is adopted");
        assert_eq!(receipt.outcome, crate::agent_turn_status::TurnOutcome::StoppedByUser);
        assert_eq!(receipt.format_line(), "Stopped by you after 30s");

        // A turn that finished long before the stop is not rewritten.
        let mut older = crate::agent_turn_status::TabTurnTracker::default();
        older.on_event(&hook_record(10.0, "UserPromptSubmit"));
        older.on_event(&hook_record(20.0, "Stop"));
        assert!(older.adopt_stop_by_user(129.5, 131.0).is_none());
        assert_eq!(older.last_turn_receipt.as_ref().map(|r| r.outcome.clone()), Some(crate::agent_turn_status::TurnOutcome::Completed));
    }

    #[test]
    fn stopped_turn_closes_its_ledger_turn_and_revert_after_a_user_undo_still_covers_every_step() {
        let mut app = new_test_app();
        let page = app.page;
        app.terminal.tabs.push(spawn_looping_stub_tab(1, AgentCliPreset::Claude, false));
        app.terminal.active_tab = 0;
        let mark = crate::agent_turn_marks::TurnMarkLine {
            v: designos_doc::live_patch::LIVE_PATCH_VERSION,
            turn_mark: crate::agent_turn_marks::TurnPhase::Start,
            cli: Some("claude".into()),
            patch_bytes: 0,
        };
        app.terminal.ledger_for_tab_mut(0).ingest_marks([mark], std::time::Instant::now());

        let start_depth = app.hist.undo_depth();
        let ids = [NodeId(700), NodeId(701), NodeId(702)];
        let patches: Vec<LivePatch> = ids
            .iter()
            .enumerate()
            .map(|(i, id)| {
                let node = Node::rectangle(*id, DocRect::new(i as f32 * 100.0, 0.0, 80.0, 40.0));
                LivePatch::new(vec![LiveOp::Insert { parent: page, at: SiblingHint::default(), nodes: vec![node] }])
            })
            .collect();
        crate::live_patch_apply::apply_live_patches_with_offsets(&mut app, &patches, &[0, 100, 200]);
        assert_eq!(app.hist.undo_depth(), start_depth + 3);
        assert!(app.terminal.ledger_for_tab(0).turns.last().is_some_and(|t| t.end_patch_bytes.is_none()), "the hooked turn is still open");

        close_stopped_turn_on_app(&mut app, 1, start_depth, Some("run-ledger"));

        assert_eq!(app.hist.undo_depth(), start_depth + 1);
        let turn = app.terminal.ledger_for_tab(0).turns.last().expect("the stopped turn stays in the ledger").clone();
        assert!(turn.end_patch_bytes.is_some(), "stopping ends the ledger turn without waiting for a hook end mark");
        assert_eq!(turn.state, crate::agent_turn_ledger::TurnState::Completed);
        assert_eq!(turn.steps.len(), 3);
        let depths: Vec<usize> = turn.steps.iter().map(|s| s.hist_depth).collect();
        assert!(depths.iter().all(|d| *d == start_depth + 1), "folded steps share the batch's history depth: {depths:?}");

        // The user edits, then undoes that edit. Reverting the stopped turn must still undo all three steps.
        let user = app.doc.alloc_id();
        app.hist
            .exec_user(&mut app.doc, Command::Insert { parent: page, index: 3, nodes: vec![Node::rectangle(user, DocRect::new(400.0, 0.0, 10.0, 10.0))] })
            .unwrap();
        assert!(app.hist.undo(&mut app.doc).unwrap());
        assert!(app.hist.can_redo());
        let report = app.terminal.ledger_for_tab_mut(0).revert_turn(&mut app.doc, &mut app.hist, turn.id).unwrap();
        assert_eq!(report.reverted_steps, 3);
        assert!(ids.iter().all(|id| app.doc.get(*id).is_none()), "every layer of the stopped turn is reverted");
    }

    fn append_hook_event(path: &std::path::Path, t: f64, event: &str) {
        use std::io::Write;
        let mut f = std::fs::OpenOptions::new().create(true).append(true).open(path).unwrap();
        writeln!(f, "{}", serde_json::json!({ "t": t, "e": event, "p": { "session_id": "sess-684" } })).unwrap();
    }

    fn poll_and_execute(app: &mut App) {
        let actions = app.terminal.poll_live_agent_bridge();
        crate::shell::execute_terminal_actions(app, actions);
    }

    /// An app whose only tab is `tab`, with a hooked turn started by a `UserPromptSubmit` record.
    fn app_with_hooked_turn(tab: TerminalTab) -> (App, std::path::PathBuf) {
        let mut app = new_test_app();
        let _ = std::fs::create_dir_all(&app.terminal.session_dir);
        let events = app.terminal.session_dir.join("agent-events.jsonl");
        let _ = std::fs::remove_file(&events);
        if let Ok(mut st) = tab.agent_status.lock() {
            *st = AgentTabStatus::Idle;
        }
        app.terminal.tabs.push(tab);
        app.terminal.active_tab = 0;
        crate::live_patch_apply::publish_live_context(&mut app);
        append_hook_event(&events, 1.0, "UserPromptSubmit");
        poll_and_execute(&mut app);
        assert!(app.terminal.tabs[0].turn_tracker.in_turn, "the prompt hook starts a turn");
        (app, events)
    }

    fn apply_agent_rects(app: &mut App, ids: &[u64]) {
        let page = app.page;
        let patches: Vec<LivePatch> = ids
            .iter()
            .map(|id| {
                let node = Node::rectangle(NodeId(*id), DocRect::new(*id as f32, 0.0, 20.0, 20.0));
                LivePatch::new(vec![LiveOp::Insert { parent: page, at: SiblingHint::default(), nodes: vec![node] }])
            })
            .collect();
        crate::live_patch_apply::apply_live_patches(app, &patches);
    }

    fn stopped_steps(app: &App) -> usize {
        app.hist.undo_labels().filter(|l| *l == Some(STOP_UNDO_LABEL)).count()
    }

    #[test]
    fn stop_closes_when_the_cli_fires_no_interrupt_hook_and_goes_quiet() {
        designos_doc::set_mock_now_ms(Some(10_000));
        let (mut app, _events) = app_with_hooked_turn(spawn_quiet_after_interrupt_stub_tab(1, AgentCliPreset::Claude));
        let page = app.page;
        let start_depth = app.hist.undo_depth();
        assert_eq!(app.terminal.tabs[0].turn_start_undo_depth, start_depth);

        let fid = NodeId(900);
        let frame = Node::frame(fid, DocRect::new(0.0, 0.0, 300.0, 200.0));
        let in_progress = InProgress {
            run_id: Some(app.terminal.tabs[0].run_id.clone()),
            agent: Some("claude".into()),
            since_ms: 10_000,
            stale_after_s: Some(300),
            status: Some("Working...".into()),
        };
        crate::live_patch_apply::apply_live_patches(
            &mut app,
            &[LivePatch::new(vec![
                LiveOp::Insert { parent: page, at: SiblingHint::default(), nodes: vec![frame] },
                LiveOp::SetInProgress { node: fid, state: Some(in_progress) },
            ])],
        );
        apply_agent_rects(&mut app, &[701, 702]);
        assert_eq!(app.hist.undo_depth(), start_depth + 3);
        assert!(app.doc.get_in_progress(fid).is_some());

        assert!(start_stop_active(&mut app.terminal, 10_000));
        assert_eq!(app.terminal.tabs[0].stop_sent_keys, vec![KEY_ESC]);

        // No Stop record is ever written and the process stays alive: only the quiet PTY can end the stop.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        while std::time::Instant::now() < deadline && stopped_steps(&app) == 0 {
            poll_and_execute(&mut app);
            std::thread::sleep(std::time::Duration::from_millis(25));
        }
        assert!(app.terminal.tabs[0].is_alive(), "the CLI stays alive at its prompt");
        assert_eq!(stopped_steps(&app), 1, "the turn folds once");
        assert_eq!(app.hist.undo_depth(), start_depth + 1);
        assert!(app.doc.in_progress.is_empty(), "in-progress flags clear");
        let tab = &app.terminal.tabs[0];
        assert_eq!(tab.last_turn_receipt().map(|r| r.outcome.clone()), Some(crate::agent_turn_status::TurnOutcome::StoppedByUser));
        assert_eq!(tab.effective_status(), AgentTabStatus::Stopped);
        assert!(tab.still_running_notice.is_none(), "the header must not say Still running");
        assert!(tab.stop_state.is_none());
        assert!(!tab.stop_in_flight.load(std::sync::atomic::Ordering::Relaxed), "stop_in_flight releases");

        // Past the 3 s retry point nothing is sent to the idle prompt.
        designos_doc::set_mock_now_ms(Some(20_000));
        poll_and_execute(&mut app);
        assert_eq!(app.terminal.tabs[0].stop_sent_keys, vec![KEY_ESC], "no Ctrl+C reaches the idle prompt");
        assert_eq!(stopped_steps(&app), 1);
        let _ = app.terminal.tabs[0].child.kill();
        designos_doc::set_mock_now_ms(None);
    }

    #[test]
    fn a_new_prompt_closes_the_pending_stop_so_the_next_turn_never_folds_with_it() {
        designos_doc::set_mock_now_ms(Some(10_000));
        let (mut app, events) = app_with_hooked_turn(spawn_chatty_stub_tab(1, AgentCliPreset::Claude, 99));
        let start_depth = app.hist.undo_depth();
        apply_agent_rects(&mut app, &[701, 702]);
        assert_eq!(app.hist.undo_depth(), start_depth + 2);

        // Stop turn N. The CLI keeps painting, so only the next prompt can close the stop here.
        assert!(start_stop_active(&mut app.terminal, 10_000));
        designos_doc::set_mock_now_ms(Some(11_000));
        append_hook_event(&events, 11.0, "UserPromptSubmit");
        poll_and_execute(&mut app);

        assert_eq!(stopped_steps(&app), 1, "turn N folds when the next prompt arrives");
        assert_eq!(app.hist.undo_depth(), start_depth + 1);
        assert!(app.terminal.tabs[0].stop_state.is_none());
        assert!(!app.terminal.tabs[0].stop_in_flight.load(std::sync::atomic::Ordering::Relaxed));
        assert!(matches!(app.terminal.tabs[0].effective_status(), AgentTabStatus::Working(_)), "the new prompt starts turn N+1");
        assert_eq!(app.terminal.tabs[0].turn_start_undo_depth, start_depth + 1, "turn N+1 starts on top of the folded step");
        assert_eq!(app.terminal.tabs[0].stop_sent_keys, vec![KEY_ESC]);

        // Turn N+1 edits and ends with a real Stop hook.
        apply_agent_rects(&mut app, &[801, 802]);
        designos_doc::set_mock_now_ms(Some(20_000));
        append_hook_event(&events, 20.0, "Stop");
        poll_and_execute(&mut app);

        assert_eq!(stopped_steps(&app), 1, "exactly one stopped step, for turn N");
        assert_eq!(app.hist.undo_depth(), start_depth + 3, "turn N+1 keeps its two single steps");
        let receipt = app.terminal.tabs[0].last_turn_receipt().expect("turn N+1 receipt").clone();
        assert_eq!(receipt.outcome, crate::agent_turn_status::TurnOutcome::Completed);
        assert_eq!(app.terminal.tabs[0].effective_status(), AgentTabStatus::Done);
        let _ = app.terminal.tabs[0].child.kill();
        designos_doc::set_mock_now_ms(None);
    }

    #[test]
    fn a_receipt_of_a_turn_that_started_after_the_stop_is_not_adopted() {
        let mut tracker = crate::agent_turn_status::TabTurnTracker::default();
        tracker.on_event(&hook_record(200.0, "UserPromptSubmit"));
        tracker.on_event(&hook_record(210.0, "Stop"));
        assert!(tracker.adopt_stop_by_user(150.0, 211.0).is_none(), "a later turn is not the stopped one");
        assert_eq!(tracker.last_turn_receipt.as_ref().map(|r| r.outcome.clone()), Some(crate::agent_turn_status::TurnOutcome::Completed));
    }

    #[test]
    fn folding_a_stopped_turn_remaps_the_ledger_of_every_tab() {
        let mut app = new_test_app();
        app.terminal.tabs.push(spawn_looping_stub_tab(1, AgentCliPreset::Claude, true));
        app.terminal.tabs.push(spawn_looping_stub_tab(2, AgentCliPreset::Claude, true));
        let start_depth = app.hist.undo_depth();

        // The user watches tab 1 while tab 0's agent edits: the steps land in the active tab's ledger.
        app.terminal.active_tab = 1;
        apply_agent_rects(&mut app, &[701, 702, 703]);
        let before: Vec<usize> = app.terminal.tabs[1].turn_ledger.turns.iter().flat_map(|t| t.steps.iter().map(|s| s.hist_depth)).collect();
        assert_eq!(before, vec![start_depth + 1, start_depth + 2, start_depth + 3]);

        close_stopped_turn_on_app(&mut app, 1, start_depth, Some("run-all-tabs"));

        assert_eq!(app.hist.undo_depth(), start_depth + 1);
        let after: Vec<usize> = app.terminal.tabs[1].turn_ledger.turns.iter().flat_map(|t| t.steps.iter().map(|s| s.hist_depth)).collect();
        assert_eq!(after, vec![start_depth + 1; 3], "the other tab's recorded depths follow the fold");
        for tab in &mut app.terminal.tabs {
            let _ = tab.child.kill();
        }
    }
}
