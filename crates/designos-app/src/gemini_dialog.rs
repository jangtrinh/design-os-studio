//! Gemini CLI's confirmation dialog, read from the tab's own screen.
//!
//! Gemini fires a `Notification` hook when it shows a tool-confirmation dialog, but no hook when the user answers No, and
//! one answer in a queue of dialogs ("1 of 2") leaves the next dialog open. So the hooks alone cannot say when the user is done.
//! The screen can: every dialog that offers options ends with the option `No, suggest changes (esc)` (`ToolConfirmationMessage.tsx`
//! of Gemini CLI 0.35.3, lines 270, 296, 321 and 352: edit, exec, info and MCP; of 0.65.0, lines 321, 347, 378, 403 and 433: edit,
//! sandbox expansion, exec, info and MCP; under the `N of M` header of `ToolConfirmationQueue.tsx`). While that line is on the tab's
//! visible screen a dialog is open, whatever the status says.
//!
//! * The composer sends nothing into a Gemini Ask tab while the line is on its screen, before the paste and before the delayed Enter,
//!   whatever the status says and whatever version of Gemini the tab runs, a version nobody could name included.
//! * A wait that a hook record started ends when the line was on screen at least once and has been gone for [`GONE_DEBOUNCE`], or when
//!   a tool, prompt or end-of-turn record arrives. If the line comes back before one of those records, the wait reopens: the next
//!   dialog of the queue is the same wait. A key press never ends a wait; it only restarts the quiet window.
//! * A tab with no hooks file (off Unix, or admin-managed Gemini) starts a wait only when the line is on screen.
//! * `ask_user` and `exit_plan_mode` dialogs draw no such line, and an edit open in the external editor draws `MODIFY_MARKER`
//!   instead: that keeps a wait open and never starts one. A dialog that never draws the line holds a hook-started wait until a hook
//!   record arrives.
//!
//! The wait logic reads Gemini's text and payloads, so it is pinned to the versions read from source ([`TESTED_GEMINI`]); outside
//! them the hooks alone drive the status. The screen is read only when the tab printed something since the last read.

use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use crate::agent_permissions::AgentPermissionMode;
use crate::gemini_screen::{dialog_view, DialogView};
use crate::terminal::{AgentCliPreset, AgentTabStatus, TerminalState, TerminalTab, NEEDS_APPROVAL_LABEL};

/// The Gemini CLI `(major, minor)` versions whose dialog text, hook events and payloads were read from source: 0.35.x and 0.65.x.
/// The versions between them were not read, so the wait logic leaves them to the hooks. The composer's guard does not look at it.
pub const TESTED_GEMINI: [(u32, u32); 2] = [(0, 35), (0, 65)];

/// How long the marker must stay off the screen before a wait ends. Gemini redraws the dialog on every answer, so a single frame
/// without it is not an answer.
pub const GONE_DEBOUNCE: Duration = Duration::from_millis(250);

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
    /// The screen revision (`TerminalTab::screen_rev`) of the last read, and what it showed.
    read: Option<(u64, DialogView)>,
    /// How many times the sync read the screen, since the last time the tab was neither idle nor waiting (tests only).
    #[cfg(test)]
    pub reads: usize,
}

/// True when `version` (as `gemini --version` prints it) is in [`TESTED_GEMINI`].
pub fn version_in_range(version: Option<&str>) -> bool {
    let mut parts = version.unwrap_or_default().trim().split('.').map(|part| part.parse::<u32>().ok());
    matches!((parts.next().flatten(), parts.next().flatten()), (Some(major), Some(minor)) if TESTED_GEMINI.contains(&(major, minor)))
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

impl TerminalTab {
    /// A Gemini tab in Ask mode: the one whose dialogs the composer guards.
    fn is_gemini_ask_tab(&self) -> bool {
        self.spawn_preset == AgentCliPreset::Gemini && self.permission_mode == AgentPermissionMode::Ask
    }

    fn gemini_dialog_view(&self) -> DialogView {
        match self.parser.lock() {
            Ok(mut parser) => dialog_view(&mut parser),
            Err(_) => DialogView::None,
        }
    }

    /// True while a Gemini tab in Ask mode shows a confirmation dialog on its screen, whatever its status and version say. The
    /// composer sends nothing into such a tab: a pasted prompt could select one of the dialog's options.
    pub fn gemini_dialog_open(&self) -> bool {
        self.is_gemini_ask_tab() && self.gemini_dialog_view() != DialogView::None
    }

    /// A key reached this tab's terminal. It never ends a wait: Gemini may still show another dialog. It restarts the quiet
    /// window, because the screen is about to change.
    pub fn note_gemini_key(&mut self) {
        self.gemini.gone_since = None;
    }

    /// The tab's dialog view, read again only when the tab printed or resized since the last read.
    fn gemini_dialog_view_cached(&mut self) -> DialogView {
        let revision = self.screen_rev.load(Ordering::Acquire);
        if let Some((read_at, view)) = self.gemini.read {
            if read_at == revision {
                return view;
            }
        }
        let view = self.gemini_dialog_view();
        self.gemini.read = Some((revision, view));
        #[cfg(test)]
        {
            self.gemini.reads += 1;
        }
        view
    }

    fn sync_gemini_dialog(&mut self, in_range: bool) {
        if !self.is_gemini_ask_tab() || !in_range || !self.is_alive() {
            return;
        }
        // Only a tab that is idle or waiting has a use for the view.
        let reads = self.agent_status.lock().is_ok_and(|status| matches!(&*status, AgentTabStatus::Idle | AgentTabStatus::WaitingForInput(_)));
        let view = if reads { self.gemini_dialog_view_cached() } else { DialogView::None };
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
        if !self.tabs.iter().any(TerminalTab::is_gemini_ask_tab) {
            return;
        }
        let in_range = version_in_range(installed_version().as_deref());
        for tab in &mut self.tabs {
            tab.sync_gemini_dialog(in_range);
        }
    }
}
