//! Screen-reader live announcements, region/button AccessKit labels, and keyboard focus toggle for the Agents panel (`#687`).

use std::cell::Cell;
use std::collections::VecDeque;

use egui::accesskit::{Live, Role};
use egui::{Context, Id, Key, Ui};

use crate::app::{App, RailItem};
use crate::terminal::{AgentCliPreset, AgentTabStatus, TerminalState};
use crate::widgets;

thread_local! {
    static ACCESSKIT_ACTIVE: Cell<bool> = const { Cell::new(false) };
}

/// Documented keyboard shortcut for moving focus to the Agents composer and back to the canvas.
pub const FOCUS_SHORTCUT_LABEL: &str = "Ctrl+Alt+J";
/// Action label shown in the Keyboard shortcuts dialog (`App::show_shortcuts`).
pub const FOCUS_SHORTCUT_ACTION: &str = "Focus Agents composer / canvas";

/// Syncs the per-thread AccessKit active flag from `ctx` at the start of a frame.
pub fn sync_frame(ctx: &Context) {
    let active = ctx.accesskit_node_builder(egui::accesskit_root_id(), |_| ()).is_some();
    ACCESSKIT_ACTIVE.with(|cell| cell.set(active));
}

/// Enables AccessKit on `ctx` and marks the current thread as active for immediate helper calls.
#[allow(dead_code)]
pub fn enable_accesskit(ctx: &Context) {
    ctx.enable_accesskit();
    ACCESSKIT_ACTIVE.with(|cell| cell.set(true));
}

/// Disables AccessKit on `ctx` and clears the current thread's active flag.
#[allow(dead_code)]
pub fn disable_accesskit(ctx: &Context) {
    ctx.disable_accesskit();
    ACCESSKIT_ACTIVE.with(|cell| cell.set(false));
}

/// True when AccessKit tree generation is active for the current frame.
#[inline]
pub fn is_accesskit_active() -> bool {
    ACCESSKIT_ACTIVE.with(Cell::get)
}

/// Shell-level live-region node ID (`Live::Polite`, `Role::Status`).
#[inline]
pub fn announcer_id() -> Id {
    Id::new("agents-a11y-announcer")
}

/// Widget ID of the Agents panel message composer input.
#[inline]
pub fn composer_id() -> Id {
    Id::new("agents-ask-for-changes-input")
}

/// Widget ID of the Agents panel interactive terminal viewport.
#[inline]
pub fn terminal_id() -> Id {
    Id::new(("agents-left-pty", "term-vt100-viewport"))
}

/// Widget ID of the main design canvas surface (`canvas::show`).
#[inline]
pub fn canvas_id() -> Id {
    Id::new((egui::ViewportId::ROOT, "__top_ui")).with("canvas")
}

/// True when keyboard focus is held by a widget other than the main canvas.
#[inline]
pub fn non_canvas_has_focus(ctx: &Context) -> bool {
    ctx.memory(|m| m.focused().is_some_and(|id| id != canvas_id()))
}

/// Normalized failure classes spoken when an agent tab enters an error state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorClass {
    RateLimited,
    LoginExpired,
    BillingOrQuota,
    ProviderServerError,
    ContextFull,
    NoNetwork,
    UnknownError,
}

impl ErrorClass {
    pub fn as_str(self) -> &'static str {
        match self {
            ErrorClass::RateLimited => "rate limited",
            ErrorClass::LoginExpired => "login expired",
            ErrorClass::BillingOrQuota => "billing or quota",
            ErrorClass::ProviderServerError => "provider server error",
            ErrorClass::ContextFull => "context full",
            ErrorClass::NoNetwork => "no network",
            ErrorClass::UnknownError => "unknown error",
        }
    }
}

fn has_status_code(lower: &str, code: &str) -> bool {
    lower.split(|c: char| !c.is_ascii_alphanumeric()).any(|tok| tok == code)
}

/// Classifies an optional raw error detail string into one of the seven spoken failure classes.
pub fn classify_error(detail: Option<&str>) -> ErrorClass {
    let Some(raw) = detail.map(str::trim).filter(|s| !s.is_empty()) else {
        return ErrorClass::UnknownError;
    };
    let lower = raw.to_ascii_lowercase();
    if lower.contains("rate limit")
        || lower.contains("rate_limit")
        || lower.contains("ratelimit")
        || lower.contains("too many requests")
        || has_status_code(&lower, "429")
    {
        ErrorClass::RateLimited
    } else if lower.contains("login expired")
        || lower.contains("login_expired")
        || lower.contains("not logged in")
        || lower.contains("unauthenticated")
        || lower.contains("unauthorized")
        || lower.contains("invalid api key")
        || lower.contains("authentication")
        || lower.contains("auth expired")
        || has_status_code(&lower, "401")
    {
        ErrorClass::LoginExpired
    } else if lower.contains("billing")
        || lower.contains("quota")
        || lower.contains("insufficient_quota")
        || lower.contains("credit")
        || lower.contains("payment")
        || has_status_code(&lower, "402")
    {
        ErrorClass::BillingOrQuota
    } else if lower.contains("context full")
        || lower.contains("context_full")
        || lower.contains("context_length")
        || lower.contains("context window")
        || lower.contains("maximum context")
        || lower.contains("prompt is too long")
        || lower.contains("token limit")
    {
        ErrorClass::ContextFull
    } else if lower.contains("no network")
        || lower.contains("no_network")
        || lower.contains("network")
        || lower.contains("connection refused")
        || lower.contains("connection reset")
        || lower.contains("dns")
        || lower.contains("offline")
        || lower.contains("timed out")
        || lower.contains("timeout")
    {
        ErrorClass::NoNetwork
    } else if lower.contains("provider server error")
        || lower.contains("server error")
        || lower.contains("server_error")
        || lower.contains("internal server error")
        || lower.contains("bad gateway")
        || lower.contains("service unavailable")
        || lower.contains("overloaded")
        || has_status_code(&lower, "500")
        || has_status_code(&lower, "502")
        || has_status_code(&lower, "503")
        || has_status_code(&lower, "529")
    {
        ErrorClass::ProviderServerError
    } else {
        ErrorClass::UnknownError
    }
}

/// One status transition observed on an agent tab.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StatusChange {
    pub tab_id: usize,
    pub preset: AgentCliPreset,
    pub title: String,
    pub prev: AgentTabStatus,
    pub next: AgentTabStatus,
}

fn is_default_tab_title(preset: AgentCliPreset, title: &str) -> bool {
    let Some((prefix, num)) = title.rsplit_once(" #") else {
        return false;
    };
    if num.is_empty() || !num.chars().all(|c| c.is_ascii_digit()) {
        return false;
    }
    prefix == preset.button_label() || prefix == preset.tab_title() || prefix == preset.display_name() || prefix == "Shell" || prefix == "Shell ($SHELL)"
}

/// Resolves the spoken/described agent name for a tab (`§3.1`, `§3.3`, `§8.3`).
///
/// Uses `tab.title` when non-empty and not an untouched default `<preset> #<id>` auto-title,
/// falling back to `preset.display_name()`.
pub fn resolved_agent_name(preset: AgentCliPreset, title: &str) -> &str {
    let trimmed = title.trim();
    if trimmed.is_empty() || is_default_tab_title(preset, trimmed) {
        preset.display_name()
    } else {
        trimmed
    }
}

/// Returns the polite screen-reader announcement string for `agent` and `status`, or `None` when silent.
pub fn announcement_for_status_named(agent: &str, status: &AgentTabStatus) -> Option<String> {
    match status {
        AgentTabStatus::Done => Some(format!("{agent} finished")),
        AgentTabStatus::WaitingForInput(_) => Some(format!("{agent} needs your input")),
        AgentTabStatus::Error(detail) => {
            let class = classify_error(detail.as_deref()).as_str();
            Some(format!("{agent} failed: {class}"))
        }
        AgentTabStatus::Idle | AgentTabStatus::Working(_) | AgentTabStatus::Stopped => None,
    }
}

/// Returns the polite screen-reader announcement string for `status`, or `None` when silent.
///
/// Only `Done`, `WaitingForInput`, and `Error` are announced; `Idle`, `Working`, and `Stopped` are silent.
#[cfg(test)]
pub fn announcement_for_status(preset: AgentCliPreset, status: &AgentTabStatus) -> Option<String> {
    announcement_for_status_named(preset.display_name(), status)
}

/// Shell-level polite live-region announcer queue (`#687`).
#[derive(Default, Debug, Clone)]
pub struct Announcer {
    pub queue: VecDeque<String>,
    pub shown: Option<(String, u64)>,
    frame: u64,
}

impl Announcer {
    /// Evaluates a tab status transition and queues an announcement when the target state is spoken.
    pub fn record_status_change(&mut self, change: &StatusChange) {
        if change.prev == change.next {
            return;
        }
        let agent = resolved_agent_name(change.preset, &change.title);
        if let Some(msg) = announcement_for_status_named(agent, &change.next) {
            self.queue.push_back(msg);
        }
    }

    /// Evaluates `status` for `preset` and queues an announcement if `status` is spoken.
    #[cfg(test)]
    pub fn notify_status(&mut self, preset: AgentCliPreset, status: &AgentTabStatus) {
        if let Some(msg) = announcement_for_status(preset, status) {
            self.queue.push_back(msg);
        }
    }

    /// Advances the announcement queue by one frame and updates the shell-level `Live::Polite` node.
    ///
    /// When AccessKit is disabled, returns without locking `ctx` or allocating strings.
    pub fn step(&mut self, ctx: &Context) {
        if !is_accesskit_active() {
            return;
        }
        self.frame = self.frame.wrapping_add(1);
        let now = self.frame;
        if let Some((_, since)) = &self.shown {
            let elapsed = now.saturating_sub(*since);
            if (!self.queue.is_empty() && elapsed >= 1) || elapsed >= 2 {
                self.shown = None;
            }
        }
        if self.shown.is_none() {
            if let Some(next) = self.queue.pop_front() {
                self.shown = Some((next, now));
            }
        }
        let shown_text = self.shown.as_ref().map(|(s, _)| s.clone());
        ctx.accesskit_node_builder(announcer_id(), |node| {
            node.set_role(Role::Status);
            node.set_live(Live::Polite);
            match shown_text {
                Some(text) => node.set_label(text),
                None => node.clear_label(),
            }
        });
    }
}

/// Drains any pending agent tab status transitions from `app.terminal` and steps `app.agent_a11y`.
pub fn poll_and_step(app: &mut App, ctx: &Context) {
    let changes = app.terminal.drain_status_changes();
    if !is_accesskit_active() {
        return;
    }
    for change in &changes {
        app.agent_a11y.record_status_change(change);
    }
    app.agent_a11y.step(ctx);
}

/// Human-readable description for the `"Agent terminal"` region naming the active agent (`agent`) and its status.
pub fn status_description_named(agent: &str, status: &AgentTabStatus) -> String {
    let state_text = match status {
        AgentTabStatus::Idle => "idle",
        AgentTabStatus::Working(_) => "working",
        AgentTabStatus::WaitingForInput(_) => "waiting for input",
        AgentTabStatus::Done => "finished",
        AgentTabStatus::Stopped => "stopped",
        AgentTabStatus::Error(detail) => {
            return format!("{agent}, failed: {}", classify_error(detail.as_deref()).as_str());
        }
    };
    format!("{agent}, {state_text}")
}

/// Registers the four Agents panel regions and icon button labels in visual order:
/// header controls -> tab strip -> terminal -> composer.
///
/// Only labels widget `Id`s that are actually created via `ui.interact` in the current frame.
pub fn label_agents_panel(state: &TerminalState, ui: &mut Ui) {
    // The composer `TextEdit` (`agent_composer::show_composer`) registers `composer_id()` itself in both
    // panel modes, so `request_focus(composer_id())` persists across frames without a placeholder widget.
    if !is_accesskit_active() {
        return;
    }

    let (active_agent, active_status) = state
        .active_tab()
        .map(|tab| {
            let st = tab.agent_status.lock().ok().map(|g| g.clone()).unwrap_or_default();
            (resolved_agent_name(tab.preset, &tab.title).to_string(), st)
        })
        .unwrap_or_else(|| (state.active_runtime.display_name().to_string(), AgentTabStatus::Idle));
    let term_desc = status_description_named(&active_agent, &active_status);

    // 1. Panel region + header controls created in this frame
    widgets::labelled(ui, Id::new("agents-panel-region"), Role::Region, "Agents panel");
    if state.side_panel_pty_open {
        widgets::labelled(ui, Id::new("term-left-dock-toggle"), Role::Button, "Toggle terminal dock");
    } else {
        widgets::labelled(ui, Id::new("agents-header-back"), Role::Button, "Back");
        widgets::labelled(ui, Id::new("agents-header-ai"), Role::Button, "Toggle AI chat");
        widgets::labelled(ui, Id::new("term-left-dock-toggle"), Role::Button, "Toggle terminal dock");
        widgets::labelled(ui, Id::new("agents-more-menu"), Role::Button, "More options");
    }

    // 2. Tab strip region + tab/runtime controls created in this frame
    widgets::labelled(ui, Id::new("agents-tab-strip-region"), Role::TabList, "Agent tabs");
    if state.side_panel_pty_open {
        let can_close = state.tabs.len() > 1;
        for tab in &state.tabs {
            widgets::labelled(ui, Id::new(("agents-left-tab-select", tab.id)), Role::Tab, &tab.title);
            if can_close {
                widgets::labelled(ui, Id::new(("agents-left-tab-close", tab.id)), Role::Button, "Close tab");
            }
        }
        widgets::labelled(ui, Id::new("agents-runtime-add-btn"), Role::Button, "New session");
    } else {
        widgets::labelled(ui, Id::new("agents-runtime-claude"), Role::Button, "Claude Code");
        widgets::labelled(ui, Id::new("agents-runtime-codex"), Role::Button, "Codex");
        widgets::labelled(ui, Id::new("agents-runtime-agy"), Role::Button, "Antigravity");
        widgets::labelled(ui, Id::new("agents-runtime-grok"), Role::Button, "Grok");
        widgets::labelled(ui, Id::new("agents-runtime-pty-toggle"), Role::Button, "Shell");
    }

    // 3. Terminal region with description naming active agent and status
    let term_node_id = terminal_id();
    ui.ctx().accesskit_node_builder(term_node_id, |node| {
        node.set_role(Role::Terminal);
        node.set_label("Agent terminal");
        node.set_description(term_desc);
    });

    // 4. Composer region + composer controls (created only in chat mode)
    widgets::labelled(ui, Id::new("agents-composer-region"), Role::Region, "Message composer");
    if !state.side_panel_pty_open {
        widgets::labelled(ui, composer_id(), Role::TextInput, "Ask for changes");
        widgets::labelled(ui, Id::new("agents-composer-plus"), Role::Button, "Attach context");
        widgets::labelled(ui, Id::new("agents-composer-book"), Role::Button, "Toggle terminal");
        widgets::labelled(ui, Id::new("agents-composer-prd"), Role::Button, "Attach PRD context");
        widgets::labelled(ui, Id::new("agents-composer-send"), Role::Button, "Send");
    }
}

/// Labels a docked or full-screen terminal surface (`show_dock` / `show_full`).
pub fn label_terminal_surface(state: &TerminalState, ui: &Ui, surface_id: &'static str) {
    if !is_accesskit_active() {
        return;
    }
    let (active_agent, active_status) = state
        .active_tab()
        .map(|tab| {
            let st = tab.agent_status.lock().ok().map(|g| g.clone()).unwrap_or_default();
            (resolved_agent_name(tab.preset, &tab.title).to_string(), st)
        })
        .unwrap_or_else(|| (state.active_runtime.display_name().to_string(), AgentTabStatus::Idle));
    let term_desc = status_description_named(&active_agent, &active_status);

    widgets::labelled(ui, Id::new((surface_id, "term-tab-list")), Role::TabList, "Agent tabs");
    for tab in &state.tabs {
        widgets::labelled(ui, Id::new((surface_id, "term-tab-select", tab.id)), Role::Tab, &tab.title);
        widgets::labelled(ui, Id::new((surface_id, "term-tab-close", tab.id)), Role::Button, "Close tab");
    }
    for preset in AgentCliPreset::QUICK_LAUNCH {
        widgets::labelled(ui, Id::new((surface_id, "term-preset-spawn", preset as usize)), Role::Button, "New session");
    }
    widgets::labelled(ui, Id::new((surface_id, "term-bar-sync")), Role::Button, "Sync Canvas");
    widgets::labelled(ui, Id::new((surface_id, "term-bar-apply-spec")), Role::Button, "Apply Spec from Output");
    widgets::labelled(ui, Id::new((surface_id, "term-bar-send-sel")), Role::Button, "Send Selection & Comments");

    ui.ctx().accesskit_node_builder(Id::new((surface_id, "term-vt100-viewport")), |node| {
        node.set_role(Role::Terminal);
        node.set_label("Agent terminal");
        node.set_description(term_desc);
    });
}

fn is_agents_panel_focus(id: Id) -> bool {
    id == composer_id()
        || id == terminal_id()
        || id == Id::new("agents-panel-region")
        || id == Id::new("agents-tab-strip-region")
        || id == Id::new("agents-composer-region")
        || id == Id::new("agents-composer-send")
        || id == Id::new("agents-runtime-add-btn")
        || id == Id::new("agents-runtime-pty-toggle")
}

/// Handles `Cmd+Alt+J` / `Ctrl+Alt+J` in `Shell::handle_keys`, moving focus to the Agents composer
/// (opening the Agents panel if needed) and returning focus to the canvas when pressed again.
///
/// Leaves the chord to `try_typography_shortcut` (`TextAlignH::Justified`) when `canvas.text` is
/// active or when the selection contains a text node.
pub fn try_focus_shortcut(app: &mut App, ctx: &Context) -> bool {
    let has_text =
        app.canvas.text.is_some() || app.selection.iter().any(|id| app.doc.get(*id).is_some_and(|n| matches!(n.kind, designos_doc::NodeKind::Text(_))));
    if has_text {
        return false;
    }

    let triggered = ctx.input(|i| {
        i.events.iter().any(|ev| {
            matches!(
                ev,
                egui::Event::Key {
                    key: Key::J,
                    pressed: true,
                    modifiers,
                    ..
                } if modifiers.command && modifiers.alt && !modifiers.shift
            )
        })
    });
    if !triggered {
        return false;
    }

    // With a shell tab the terminal holds the keyboard through `viewport_focused` alone (no egui focus id).
    let terminal_holds_keys = app.ui.rail == Some(RailItem::Agents)
        && app.terminal.side_panel_pty_open
        && app.terminal.viewport_focused
        && app.terminal.active_tab().is_some_and(|t| t.is_shell_process());
    let focused_in_agents = terminal_holds_keys || ctx.memory(|m| m.focused().is_some_and(is_agents_panel_focus));
    if focused_in_agents {
        app.terminal.viewport_focused = false;
        ctx.memory_mut(|m| m.request_focus(canvas_id()));
    } else {
        app.ui.rail = Some(RailItem::Agents);
        app.ui.left_collapsed = false;
        if app.terminal.tabs.is_empty() && !app.terminal.picker_open {
            let file = app.file_path.as_deref();
            app.terminal.open_for_agent_use(file);
        }
        // A shell tab disables the composer ("Select an agent tab"), so focus goes to the terminal itself.
        let composer_disabled = app.terminal.side_panel_pty_open && app.terminal.active_tab().is_some_and(|t| t.is_shell_process());
        if composer_disabled {
            app.terminal.viewport_focused = true;
            ctx.memory_mut(|m| m.surrender_focus(composer_id()));
        } else {
            app.terminal.viewport_focused = false;
            ctx.memory_mut(|m| m.request_focus(composer_id()));
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn needs_input_and_error_texts() {
        assert_eq!(announcement_for_status(AgentCliPreset::Claude, &AgentTabStatus::Done).as_deref(), Some("Claude Code finished"));
        assert_eq!(
            announcement_for_status(AgentCliPreset::Codex, &AgentTabStatus::WaitingForInput(Some("bash".into()))).as_deref(),
            Some("Codex needs your input")
        );
        assert_eq!(
            announcement_for_status(AgentCliPreset::Antigravity, &AgentTabStatus::WaitingForInput(None)).as_deref(),
            Some("Antigravity needs your input")
        );

        let error_cases = [
            (AgentCliPreset::Claude, Some("429 Too Many Requests: rate limit exceeded"), "Claude Code failed: rate limited"),
            (AgentCliPreset::Codex, Some("401 Unauthorized: login expired"), "Codex failed: login expired"),
            (AgentCliPreset::Grok, Some("402 Payment Required: insufficient_quota / billing"), "Grok failed: billing or quota"),
            (AgentCliPreset::Claude, Some("503 Service Unavailable: provider server error"), "Claude Code failed: provider server error"),
            (AgentCliPreset::Codex, Some("context_length_exceeded: context window is full"), "Codex failed: context full"),
            (AgentCliPreset::Claude, Some("prompt is too long: 150000 tokens"), "Claude Code failed: context full"),
            (AgentCliPreset::Antigravity, Some("connection refused: no network"), "Antigravity failed: no network"),
            (AgentCliPreset::Codex, Some("timed out after 5000ms"), "Codex failed: no network"),
            (AgentCliPreset::Grok, Some("unexpected error 150000"), "Grok failed: unknown error"),
            (AgentCliPreset::Shell, Some("unexpected crash"), "Shell failed: unknown error"),
            (AgentCliPreset::Claude, None, "Claude Code failed: unknown error"),
        ];
        for (preset, detail, expected) in error_cases {
            let status = AgentTabStatus::Error(detail.map(str::to_string));
            assert_eq!(announcement_for_status(preset, &status).as_deref(), Some(expected));
        }
    }

    #[test]
    fn running_and_stopped_are_not_announced() {
        let mut announcer = Announcer::default();
        for preset in [AgentCliPreset::Claude, AgentCliPreset::Codex, AgentCliPreset::Antigravity, AgentCliPreset::Grok, AgentCliPreset::Shell] {
            announcer.notify_status(preset, &AgentTabStatus::Idle);
            announcer.notify_status(preset, &AgentTabStatus::Working(None));
            announcer.notify_status(preset, &AgentTabStatus::Working(Some("designos_apply_spec".into())));
            announcer.notify_status(preset, &AgentTabStatus::Stopped);
        }
        assert!(announcer.queue.is_empty(), "Idle, Working, and Stopped must never queue announcements");
        assert!(announcer.shown.is_none());
    }

    #[test]
    fn two_changes_in_one_frame_are_both_spoken() {
        let ctx = Context::default();
        enable_accesskit(&ctx);
        let mut announcer = Announcer::default();

        // Trigger two tab transitions in the same frame (frame N).
        announcer.record_status_change(&StatusChange {
            tab_id: 1,
            preset: AgentCliPreset::Claude,
            title: String::new(),
            prev: AgentTabStatus::Working(None),
            next: AgentTabStatus::Done,
        });
        announcer.record_status_change(&StatusChange {
            tab_id: 2,
            preset: AgentCliPreset::Grok,
            title: String::new(),
            prev: AgentTabStatus::Working(None),
            next: AgentTabStatus::Done,
        });

        let out_n = ctx.run_ui(egui::RawInput::default(), |ui| {
            sync_frame(ui.ctx());
            announcer.step(ui.ctx());
        });
        let update_n = out_n.platform_output.accesskit_update.expect("accesskit enabled on frame N");
        let polite_n = update_n
            .nodes
            .iter()
            .find(|(id, n)| *id == announcer_id().accesskit_id() && n.live() == Some(Live::Polite))
            .and_then(|(_, n)| n.label().map(str::to_string));
        assert_eq!(polite_n.as_deref(), Some("Claude Code finished"), "first transition must be spoken on frame N");

        let out_n1 = ctx.run_ui(egui::RawInput::default(), |ui| {
            sync_frame(ui.ctx());
            announcer.step(ui.ctx());
        });
        let update_n1 = out_n1.platform_output.accesskit_update.expect("accesskit enabled on frame N + 1");
        let polite_n1 = update_n1
            .nodes
            .iter()
            .find(|(id, n)| *id == announcer_id().accesskit_id() && n.live() == Some(Live::Polite))
            .and_then(|(_, n)| n.label().map(str::to_string));
        assert_eq!(polite_n1.as_deref(), Some("Grok finished"), "second transition must be spoken on frame N + 1");
    }

    #[test]
    fn renamed_tab_uses_new_title_in_announcement_and_description() {
        assert_eq!(resolved_agent_name(AgentCliPreset::Claude, ""), "Claude Code");
        assert_eq!(resolved_agent_name(AgentCliPreset::Claude, "Claude CLI #1"), "Claude Code");
        assert_eq!(resolved_agent_name(AgentCliPreset::Claude, "UI Reviewer"), "UI Reviewer");

        let mut announcer = Announcer::default();
        announcer.record_status_change(&StatusChange {
            tab_id: 1,
            preset: AgentCliPreset::Claude,
            title: "UI Reviewer".to_string(),
            prev: AgentTabStatus::Working(None),
            next: AgentTabStatus::Done,
        });
        assert_eq!(announcer.queue.front().map(String::as_str), Some("UI Reviewer finished"));
        assert_eq!(status_description_named(resolved_agent_name(AgentCliPreset::Claude, "UI Reviewer"), &AgentTabStatus::Idle), "UI Reviewer, idle");
    }

    #[test]
    fn no_accesskit_means_no_cost() {
        let ctx = Context::default();
        disable_accesskit(&ctx);
        let mut announcer = Announcer::default();
        announcer.notify_status(AgentCliPreset::Claude, &AgentTabStatus::Done);

        let out = ctx.run_ui(egui::RawInput::default(), |ui| {
            sync_frame(ui.ctx());
            assert!(!is_accesskit_active());
            // Hold a read lock on Context via `ctx.input`: if `widgets::labelled` or `announcer.step`
            // attempted to acquire `ctx.write(...)` when AccessKit is disabled, this would deadlock.
            ui.ctx().input(|_| {
                widgets::labelled(ui, Id::new("no-cost-btn"), Role::Button, "Send");
                announcer.step(ui.ctx());
            });
        });
        assert!(out.platform_output.accesskit_update.is_none(), "accesskit_update must be None when AccessKit is disabled");
    }
}
