//! Gemini CLI's confirmation dialog, read from the tab's own screen.
//!
//! Gemini fires a `Notification` hook when it shows a tool-confirmation dialog, but no hook when the user answers No, and
//! one answer in a queue of dialogs ("1 of 2") leaves the next dialog open. So the hooks alone cannot say when the user is done.
//! The screen can: every dialog that offers options ends with the option `No, suggest changes (esc)` (`ToolConfirmationMessage.tsx`
//! lines 270, 296, 321 and 352 of Gemini CLI 0.35.3: edit, exec, info and MCP, under the `N of M` header of
//! `ToolConfirmationQueue.tsx:96-100`). While that line is on the tab's visible screen a dialog is open, whatever the status says.
//!
//! * A wait that a hook record started ends when the line was on screen at least once and has been gone for [`GONE_DEBOUNCE`],
//!   or when a tool, prompt or end-of-turn record arrives. If the line comes back before one of those records, the wait reopens:
//!   the next dialog of the queue is the same wait. A key press never ends a wait; it only restarts the quiet window.
//! * A tab with no hooks file (off Unix, or admin-managed Gemini) starts a wait only when the line is on screen.
//! * The composer sends nothing into a tab while the line is on its screen, and not just before its delayed Enter either.
//! * `ask_user` and `exit_plan_mode` dialogs draw no such line, and an edit open in the external editor draws
//!   [`MODIFY_MARKER`] instead: that keeps a wait open and never starts one. A dialog that never draws the line holds its wait
//!   until a hook record arrives.
//!
//! The text is Gemini's, so this is pinned to the tested range ([`TESTED_GEMINI`]); outside it nothing here is used and the hooks
//! alone drive the status. Gemini 0.36 to 0.40 still contain the line, but are not tested.

use std::time::{Duration, Instant};

use crate::agent_permissions::AgentPermissionMode;
use crate::terminal::{AgentCliPreset, AgentTabStatus, TerminalState, TerminalTab, NEEDS_APPROVAL_LABEL};

/// The last option of every Gemini tool-confirmation dialog that has options.
pub const DIALOG_MARKER: &str = "No, suggest changes (esc)";

/// What Gemini draws in place of the options while an edit is open in the external editor (`ToolConfirmationMessage.tsx:708-710`).
pub const MODIFY_MARKER: &str = "Save and close external editor to continue";

/// The Gemini CLI `(major, minor)` whose dialog text was read and tested: 0.35.x.
pub const TESTED_GEMINI: (u32, u32) = (0, 35);

/// How long the marker must stay off the screen before a wait ends. Gemini redraws the dialog on every answer, so a single frame
/// without it is not an answer.
pub const GONE_DEBOUNCE: Duration = Duration::from_millis(250);

/// What a Gemini tab's screen shows of a confirmation dialog.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DialogView {
    None,
    /// The options are on screen.
    Options,
    /// An edit is open in the external editor.
    Modifying,
}

/// Per-tab state of the screen reading.
#[derive(Debug, Default)]
pub struct GeminiTabState {
    /// A hooks file was written for the tab, so a hook record starts its waits.
    pub hooks: bool,
    /// A hook-started wait is open: a tool, prompt or end-of-turn record has not ended it.
    armed: bool,
    /// The dialog has been on screen since the wait began.
    marker_seen: bool,
    gone_since: Option<Instant>,
}

/// True when `version` (as `gemini --version` prints it) is in [`TESTED_GEMINI`].
pub fn version_in_range(version: Option<&str>) -> bool {
    let mut parts = version.unwrap_or_default().trim().split('.').map(|part| part.parse::<u32>().ok());
    (parts.next().flatten(), parts.next().flatten()) == (Some(TESTED_GEMINI.0), Some(TESTED_GEMINI.1))
}

#[cfg(test)]
thread_local! {
    static VERSION: std::cell::RefCell<Option<Option<String>>> = const { std::cell::RefCell::new(None) };
}

/// Runs `body` on this thread as if the installed Gemini printed `version`. A test never starts the real one, so without this
/// it sees 0.35.3.
#[cfg(test)]
pub fn with_gemini_version<R>(version: Option<&str>, body: impl FnOnce() -> R) -> R {
    VERSION.with(|slot| *slot.borrow_mut() = Some(version.map(str::to_string)));
    struct Restore;
    impl Drop for Restore {
        fn drop(&mut self) {
            VERSION.with(|slot| *slot.borrow_mut() = None);
        }
    }
    let _restore = Restore;
    body()
}

fn installed_version() -> Option<String> {
    #[cfg(test)]
    return VERSION.with(|slot| slot.borrow().clone()).unwrap_or_else(|| Some("0.35.3".to_string()));
    #[cfg(not(test))]
    crate::agent_session_failure::installed_cli_version(AgentCliPreset::Gemini)
}

/// What `parser`'s screen shows. Read at scrollback 0 whatever the user scrolled to, and put back under the same lock.
pub fn dialog_view(parser: &mut vt100::Parser) -> DialogView {
    let scrolled = parser.screen().scrollback();
    parser.set_scrollback(0);
    let text = parser.screen().contents();
    parser.set_scrollback(scrolled);
    // Words only: a narrow panel wraps the label across lines and draws border characters between the pieces.
    let words: Vec<&str> = text.split(|c: char| c.is_whitespace() || ('\u{2500}'..='\u{257F}').contains(&c)).filter(|word| !word.is_empty()).collect();
    let flat = words.join(" ");
    if flat.contains(DIALOG_MARKER) {
        DialogView::Options
    } else if flat.contains(MODIFY_MARKER) {
        DialogView::Modifying
    } else {
        DialogView::None
    }
}

impl TerminalTab {
    fn gemini_dialog_applies(&self) -> bool {
        self.spawn_preset == AgentCliPreset::Gemini && self.permission_mode == AgentPermissionMode::Ask && version_in_range(installed_version().as_deref())
    }

    fn gemini_dialog_view(&self) -> DialogView {
        match self.parser.lock() {
            Ok(mut parser) => dialog_view(&mut parser),
            Err(_) => DialogView::None,
        }
    }

    /// True while a Gemini tab in Ask mode shows a confirmation dialog on its screen, whatever its status says. The composer sends
    /// nothing into such a tab: a pasted prompt could select one of the dialog's options.
    pub fn gemini_dialog_open(&self) -> bool {
        self.gemini_dialog_applies() && self.gemini_dialog_view() != DialogView::None
    }

    /// A key reached this tab's terminal. It never ends a wait: Gemini may still show another dialog. It restarts the quiet
    /// window, because the screen is about to change.
    pub fn note_gemini_key(&mut self) {
        self.gemini.gone_since = None;
    }

    fn sync_gemini_dialog(&mut self, in_range: bool) {
        if self.spawn_preset != AgentCliPreset::Gemini || self.permission_mode != AgentPermissionMode::Ask || !in_range || !self.is_alive() {
            return;
        }
        let view = self.gemini_dialog_view();
        let Ok(mut status) = self.agent_status.lock() else { return };
        let gemini = &mut self.gemini;
        match &*status {
            AgentTabStatus::WaitingForInput(_) => {
                gemini.armed |= gemini.hooks;
                if view != DialogView::None {
                    gemini.marker_seen = true;
                    gemini.gone_since = None;
                } else if gemini.marker_seen && gemini.gone_since.get_or_insert_with(Instant::now).elapsed() >= GONE_DEBOUNCE {
                    *status = AgentTabStatus::Idle;
                    gemini.marker_seen = false;
                    gemini.gone_since = None;
                }
            }
            AgentTabStatus::Idle => {
                gemini.marker_seen = false;
                gemini.gone_since = None;
                if view == DialogView::Options && (!gemini.hooks || gemini.armed) {
                    *status = AgentTabStatus::WaitingForInput(Some(NEEDS_APPROVAL_LABEL.into()));
                    gemini.marker_seen = true;
                }
            }
            _ => {
                *gemini = GeminiTabState { hooks: gemini.hooks, ..GeminiTabState::default() };
            }
        }
    }
}

impl TerminalState {
    /// Brings every Gemini tab's wait in line with its screen. Runs on every bridge poll, for background tabs too: the reader
    /// thread feeds a tab's screen whether or not it is painted.
    pub fn sync_gemini_dialogs(&mut self) {
        // The installed version is only looked up when a Gemini Ask tab is open: a window without one never probes the CLI.
        if !self.tabs.iter().any(|tab| tab.spawn_preset == AgentCliPreset::Gemini && tab.permission_mode == AgentPermissionMode::Ask) {
            return;
        }
        let in_range = version_in_range(installed_version().as_deref());
        for tab in &mut self.tabs {
            tab.sync_gemini_dialog(in_range);
        }
    }
}
