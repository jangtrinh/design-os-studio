//! Asking before unsaved edits are dropped.
//!
//! A document outside the library has no autosave, so its edits live only in the window. Closing its tab,
//! quitting, closing the other tabs and opening a file over it each end that: they ask "Save changes to
//! <name>?" first, once per document, in tab order. A library file autosaves and never asks, and a document
//! with no unsaved edits is not asked about. What counts as unsaved is [`crate::tabs::tab_is_unsaved`], the
//! same state the dot shows. What is under way (an open text session, a drag, a scrub) is ended where it is first,
//! as one undo step ([`App::commit_open_edits`]), so it is in the history and Save writes a settled document; an
//! edit that cannot be ended that way (a colour picker, a brush) still counts as unsaved.
//!
//! The actions themselves (`App::close_tab` and the like) stay unconditional; the places a user can reach
//! them from go through [`App::request_leave`].

use crate::app::App;
use std::collections::VecDeque;
use std::path::PathBuf;

/// What the user asked for, and what it would end.
#[derive(Clone, Debug, PartialEq)]
pub enum Leave {
    /// Close the tab with this id.
    CloseTab(u64),
    /// Close every unpinned tab but this one.
    CloseOthers(u64),
    /// Close the window.
    Quit,
    /// Open a file, which replaces the document on screen (or the open tab of that same file, from Home).
    OpenPath(PathBuf),
    /// Open a library file in the editor, which replaces the document on screen.
    OpenLibraryFile(String),
    /// Create a new file in the editor, which replaces the document on screen.
    NewFile(designos_doc::DocumentKind, Option<(String, Option<String>)>),
}

/// The answer to "Save changes to <name>?".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Answer {
    Save,
    DontSave,
    Cancel,
}

/// Where the user was when the questions began, to put them back if they cancel after a save moved them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Origin {
    Home,
    Tab(u64),
}

/// Who asked to leave. Human asks; agent follows the MCP unsaved policy and never opens the dialog.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LeaveFrom {
    Human,
    Agent,
}

struct Pending {
    action: Leave,
    /// Ids of the tabs still to ask about, in tab order. The one being asked about is at the front.
    queue: VecDeque<u64>,
    origin: Origin,
}

#[derive(Default)]
pub struct LeaveFlow {
    pending: Option<Pending>,
    /// Every question was answered: the window may close when the close request comes round again.
    pub quit_approved: bool,
    /// The open that is being run after its questions were answered must not ask again.
    confirmed_open: bool,
}

impl LeaveFlow {
    pub fn is_asking(&self) -> bool {
        self.pending.is_some()
    }
}

impl App {
    /// Runs `action` now when nothing unsaved would be lost, else asks first and runs it once every question
    /// is answered with Save or Don't save.
    pub fn request_leave(&mut self, ctx: Option<&egui::Context>, action: Leave) {
        self.leave_from(ctx, action, LeaveFrom::Human);
    }

    /// Agent-originated leave: never sets the asking state and never calls [`Self::ask_next`].
    /// Close of one document is `Leave::CloseTab` only, so other dirty tabs are not asked and quit does not start.
    pub fn request_leave_from_agent(&mut self, ctx: Option<&egui::Context>, action: Leave) {
        self.leave_from(ctx, action, LeaveFrom::Agent);
    }

    fn leave_from(&mut self, ctx: Option<&egui::Context>, action: Leave, from: LeaveFrom) {
        match from {
            LeaveFrom::Human => {
                if !self.begin_leave(ctx, action.clone()) {
                    self.perform_leave(ctx, action);
                }
            }
            LeaveFrom::Agent => {
                let Leave::CloseTab(_) = action else { return };
                self.commit_open_edits();
                self.perform_leave(ctx, action);
            }
        }
    }

    /// For a window close request: true when the window may close now. Otherwise the questions are open (or
    /// were already), and the caller cancels the close; it comes round again when they are answered. A question is
    /// asked over the editor, so the Present view, which draws no dialog, is left first.
    pub fn may_quit(&mut self, ctx: &egui::Context) -> bool {
        if self.leave.quit_approved {
            return true;
        }
        if self.leave.is_asking() {
            return false;
        }
        // An agent's document held behind a dialog is shown first, so the questions ask about it like any other.
        if self.terminal.live_patch.pending_doc.is_some() && self.dialog.is_some() {
            self.dialog = None;
            // A drag or a text session that is still going on holds the switch too: end it where it is first.
            self.commit_open_edits();
            crate::live_patch_apply::apply_pending_live_patches(self);
            let mut tabs = std::mem::take(&mut self.tabs);
            tabs.sync_from_app(self);
            self.tabs = tabs;
        }
        let asking = self.begin_leave(Some(ctx), Leave::Quit);
        if asking {
            crate::present::leave(self, ctx);
        }
        !asking
    }

    /// For [`App::open_path`]: true when it must wait for the answers (`open_path` returns, and runs again when they
    /// are in). Imports that do not replace a document never get here.
    pub(crate) fn ask_before_open(&mut self, path: &std::path::Path) -> bool {
        if !self.leave.confirmed_open && self.begin_leave(None, Leave::OpenPath(path.to_path_buf())) {
            return true;
        }
        self.show_the_tab_of(path);
        false
    }

    /// From Home, a file that is open in a tab is read again into that tab, which is then the one on screen: the
    /// tab is shown first, so its document is replaced in place like any open over the shown document.
    fn show_the_tab_of(&mut self, path: &std::path::Path) {
        if self.home.is_none() && !self.tabs.parked_in_home {
            return;
        }
        if let Some(idx) = self.tabs.entries.iter().position(|t| t.file_path.as_deref() == Some(path)) {
            self.switch_tab(idx);
        }
    }

    /// For the actions that replace the document on screen with another (opening a library file, a new file): true
    /// when they must wait for the answers. From Home they open a tab of their own and replace nothing.
    pub(crate) fn ask_before_replacing_the_shown_document(&mut self, action: Leave) -> bool {
        !self.leave.confirmed_open && self.begin_leave(None, action)
    }

    /// Starts the questions for `action`. False when there is nothing to ask about. While a question is open the
    /// answer is modal: a request that arrives meanwhile (a dropped file, another open) is ignored, so it does not
    /// replace the action that is pending, and counts as asked about (true) so that the caller does not go on.
    fn begin_leave(&mut self, ctx: Option<&egui::Context>, action: Leave) -> bool {
        if self.leave.is_asking() {
            return true;
        }
        // Typed text, a drag and a scrub are not in the history until they end: end them where they are, so that they
        // count, and so that Save writes a settled document.
        self.commit_open_edits();
        let queue: VecDeque<u64> = self.tabs_to_ask_about(&action).into();
        if queue.is_empty() {
            return false;
        }
        let origin = if self.home.is_some() { Origin::Home } else { self.tabs.entries.get(self.tabs.active_idx).map_or(Origin::Home, |t| Origin::Tab(t.id)) };
        self.leave.pending = Some(Pending { action, queue, origin });
        self.set_leave_asking_marker(true);
        self.ask_next(ctx);
        true
    }

    fn set_leave_asking_marker(&self, on: bool) {
        let path = self.terminal.session_dir.join("leave-asking");
        if on {
            let _ = crate::agent_session_files::atomic_write_session_file(&path, b"1");
        } else {
            let _ = crate::agent_session_files::remove_session_file(&path);
        }
    }

    /// The tabs that `action` would end and whose document is outside the library with edits no save has written.
    fn tabs_to_ask_about(&self, action: &Leave) -> Vec<u64> {
        let ended: Vec<usize> = match action {
            Leave::CloseTab(id) => self.tabs.entries.iter().position(|t| t.id == *id && !t.pinned).into_iter().collect(),
            Leave::CloseOthers(keep) => (0..self.tabs.entries.len()).filter(|&i| self.tabs.entries[i].id != *keep && !self.tabs.entries[i].pinned).collect(),
            Leave::Quit => (0..self.tabs.entries.len()).collect(),
            Leave::OpenPath(path) => {
                if self.home.is_some() || self.tabs.parked_in_home {
                    // From Home a file that is open already is read again into its own tab.
                    (0..self.tabs.entries.len()).filter(|&i| self.tabs.entries[i].file_path.as_deref() == Some(path.as_path())).collect()
                } else {
                    // Otherwise it replaces the document on screen.
                    vec![self.tabs.active_idx]
                }
            }
            Leave::OpenLibraryFile(_) | Leave::NewFile(..) => {
                if self.home.is_some() || self.tabs.parked_in_home {
                    Vec::new()
                } else {
                    vec![self.tabs.active_idx]
                }
            }
        };
        ended
            .into_iter()
            .filter(|&i| {
                let Some(tab) = self.tabs.entries.get(i) else { return false };
                let shown = self.home.is_none() && i == self.tabs.active_idx;
                // The shown document's library file is the live state: its tab entry is only refreshed by the tab bar.
                let outside_the_library = if shown { self.lib_file.is_none() } else { tab.lib_file.is_none() };
                outside_the_library && (crate::tabs::tab_is_unsaved(self, i) || (shown && crate::live_patch_apply::user_edit_in_progress(self)))
            })
            .map(|i| self.tabs.entries[i].id)
            .collect()
    }

    /// Opens the question for the first tab in the queue, or runs the action when none is left.
    fn ask_next(&mut self, ctx: Option<&egui::Context>) {
        loop {
            let Some(p) = self.leave.pending.as_mut() else { return };
            let Some(&id) = p.queue.front() else {
                let Pending { action, origin, .. } = self.leave.pending.take().expect("pending was just read");
                self.set_leave_asking_marker(false);
                self.perform_leave(ctx, action.clone());
                if matches!(action, Leave::CloseTab(_) | Leave::CloseOthers(_)) {
                    self.restore_origin(origin);
                }
                return;
            };
            let Some(tab) = self.tabs.entries.iter().find(|t| t.id == id) else {
                p.queue.pop_front();
                continue;
            };
            self.dialog = Some(super::dialog::Dialog::save_changes(&tab.title));
            if let Some(ctx) = ctx {
                ctx.request_repaint();
            }
            return;
        }
    }

    /// The answer to the question that is open.
    pub(super) fn answer_save_changes(&mut self, ctx: &egui::Context, answer: Answer) {
        self.answer_save_changes_with(Some(ctx), answer, &crate::io::SystemDialogs);
    }

    pub(crate) fn answer_save_changes_with(&mut self, ctx: Option<&egui::Context>, answer: Answer, dialogs: &dyn crate::io::FileDialogs) {
        let Some(front) = self.leave.pending.as_ref().and_then(|p| p.queue.front().copied()) else { return };
        match answer {
            Answer::Cancel => self.cancel_leave(),
            Answer::DontSave => {
                if let Some(p) = self.leave.pending.as_mut() {
                    p.queue.pop_front();
                }
                self.ask_next(ctx);
            }
            Answer::Save => {
                if self.save_tab_for_leave(front, dialogs) {
                    if let Some(p) = self.leave.pending.as_mut() {
                        p.queue.pop_front();
                    }
                    self.ask_next(ctx);
                } else {
                    // Not saved (the Save dialog was cancelled or could not open, the write failed, or it went to a
                    // format that drops content): the action does not go on, and the document is still unsaved.
                    self.cancel_leave();
                }
            }
        }
    }

    /// Cancel: nothing was dropped, so nothing is undone but where the user was.
    fn cancel_leave(&mut self) {
        let Some(p) = self.leave.pending.take() else { return };
        self.set_leave_asking_marker(false);
        self.restore_origin(p.origin);
    }

    /// Shows the tab `id` and runs Save on it. True when its document no longer has unsaved edits.
    fn save_tab_for_leave(&mut self, id: u64, dialogs: &dyn crate::io::FileDialogs) -> bool {
        let Some(idx) = self.tabs.entries.iter().position(|t| t.id == id) else { return true };
        if self.home.is_some() || idx != self.tabs.active_idx {
            self.switch_tab(idx);
        }
        self.save_with(false, dialogs);
        !self.has_unsaved_edits()
    }

    /// Puts the user back where the questions began, when that place is still there.
    fn restore_origin(&mut self, origin: Origin) {
        match origin {
            Origin::Home => {
                if self.home.is_none() {
                    let mut tabs = std::mem::take(&mut self.tabs);
                    tabs.enter_home(self);
                    self.tabs = tabs;
                }
            }
            Origin::Tab(id) => {
                if let Some(idx) = self.tabs.entries.iter().position(|t| t.id == id) {
                    if self.home.is_some() || idx != self.tabs.active_idx {
                        self.switch_tab(idx);
                    }
                }
            }
        }
    }

    fn perform_leave(&mut self, ctx: Option<&egui::Context>, action: Leave) {
        match action {
            Leave::CloseTab(id) => {
                if let Some(idx) = self.tabs.entries.iter().position(|t| t.id == id) {
                    self.close_tab(idx);
                }
            }
            Leave::CloseOthers(keep) => {
                if let Some(idx) = self.tabs.entries.iter().position(|t| t.id == keep) {
                    let mut tabs = std::mem::take(&mut self.tabs);
                    tabs.close_other_tabs(idx, self);
                    self.tabs = tabs;
                }
            }
            Leave::Quit => {
                self.leave.quit_approved = true;
                if let Some(ctx) = ctx {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
            Leave::OpenPath(path) => {
                self.leave.confirmed_open = true;
                self.open_path(path);
                self.leave.confirmed_open = false;
            }
            Leave::OpenLibraryFile(id) => {
                self.leave.confirmed_open = true;
                self.open_library_file(&id);
                self.leave.confirmed_open = false;
            }
            Leave::NewFile(kind, target) => {
                self.leave.confirmed_open = true;
                self.new_library_file_of(target, kind);
                self.leave.confirmed_open = false;
            }
        }
    }
}
