//! Version history of library files: automatic and named versions, viewing an old
//! version read-only, and restoring it.
//!
//! Restoring replaces the document wholesale, which the command history cannot
//! express, so the pre-restore state (document, undo stack, page) is parked in
//! [`HistoryState::restore_undo`]; the first undo with nothing left to undo
//! swaps it back, and the next redo swaps forward again.

pub mod banner;
pub mod modal;
pub mod panel;
pub mod preset;
mod session;

use crate::app::App;
use crate::home::doc_hash;
use designos_doc::{Document, History, NodeId};
use designos_library::FileVersion;

/// Seconds of editing activity between automatic versions.
pub const AUTO_INTERVAL: f64 = 600.0;
/// Edits further apart than this count as a pause, not as activity.
const ACTIVITY_GAP_CAP: f64 = 60.0;

/// Counts editing time: the gaps between consecutive edit bursts, each capped.
#[derive(Default, Debug)]
pub struct Activity {
    last_edit: Option<f64>,
    active: f64,
}

impl Activity {
    pub fn note_edit(&mut self, now: f64) {
        if let Some(last) = self.last_edit {
            self.active += (now - last).clamp(0.0, ACTIVITY_GAP_CAP);
        }
        self.last_edit = Some(now);
    }

    pub fn due(&self) -> bool {
        self.active >= AUTO_INTERVAL
    }

    pub fn reset(&mut self) {
        self.active = 0.0;
    }
}

/// An editor state parked while another document is shown.
pub struct Snapshot {
    pub doc: Document,
    pub hist: History,
    pub page: NodeId,
    pub selection: Vec<NodeId>,
}

/// A version shown read-only on the canvas; `saved` is the live state to come back to.
pub struct Viewing {
    pub n: u32,
    pub title: String,
    saved: Snapshot,
}

pub struct SaveModal {
    pub name: String,
    pub description: String,
    pub fresh: bool,
}

#[derive(Default)]
pub struct HistoryState {
    /// The version history panel replaces the right panel.
    pub open: bool,
    pub list: Vec<FileVersion>,
    /// File the cached `list` belongs to.
    pub list_for: Option<String>,
    pub dirty: bool,
    pub viewing: Option<Viewing>,
    /// Context menu: version number and click position.
    pub menu: Option<(u32, egui::Pos2)>,
    pub save_modal: Option<SaveModal>,
    pub scroll: f32,
    pub activity: Activity,
    was_pending: bool,
    /// Hash of the document as last stored as a version (or as opened).
    pub last_version_hash: u64,
    /// The state before the last restore, for undo.
    pub restore_undo: Option<Snapshot>,
    /// The restored state, for redo, valid while the document still hashes to `.1`.
    pub restore_redo: Option<(Snapshot, u64)>,
}

impl HistoryState {
    /// Forgets everything tied to the document being replaced.
    pub fn reset(&mut self) {
        let open = self.open;
        *self = Self { open, dirty: true, ..Default::default() };
    }
}

/// Short stamp for a version without a name: `2026-10-11 09:30 UTC`.
pub fn stamp(ms: u64) -> String {
    let s = crate::comments::time::format_rfc3339(ms);
    format!("{} {} UTC", &s[..10], &s[11..16])
}

/// Group heading of the panel: how many whole days ago, measured from `now`.
pub fn day_label(now: u64, created: u64) -> String {
    let days = now.saturating_sub(created) / 86_400_000;
    match days {
        0 => "Today".into(),
        1 => "Yesterday".into(),
        n if n < 7 => format!("{n} days ago"),
        _ => crate::comments::time::format_rfc3339(created)[..10].to_string(),
    }
}

/// Title shown for a version: its name, else its saved time.
pub fn version_title(v: &FileVersion) -> String {
    v.label.clone().filter(|l| !l.trim().is_empty()).unwrap_or_else(|| stamp(v.created))
}

impl App {
    /// The open file lives in the library, so it has a version history.
    pub fn has_history(&self) -> bool {
        self.lib.is_some() && self.lib_file.is_some()
    }

    /// File -> Show version history.
    pub fn toggle_version_history(&mut self) {
        self.history.open = !self.history.open;
        if self.history.open {
            self.history.dirty = true;
            self.history.scroll = 0.0;
        } else {
            self.exit_version_view();
        }
    }

    /// Called when a library file was opened or created: a fresh history context.
    pub fn history_open_file(&mut self) {
        self.history.reset();
        self.history.last_version_hash = if self.autosave.saved_hash != 0 { self.autosave.saved_hash } else { doc_hash(&self.doc) };
    }

    /// Reloads the version list when it is stale.
    pub fn refresh_versions(&mut self) {
        let (Some(lib), Some(lf)) = (self.lib.as_ref(), self.lib_file.as_ref()) else {
            self.history.list.clear();
            self.history.list_for = None;
            return;
        };
        if self.history.dirty || self.history.list_for.as_deref() != Some(lf.id.as_str()) {
            self.history.list = lib.list_versions(&lf.id);
            self.history.list_for = Some(lf.id.clone());
            self.history.dirty = false;
        }
    }

    /// Stores the live document as an automatic version unless nothing changed since the last one.
    pub fn save_auto_version(&mut self) -> Option<FileVersion> {
        if self.history.viewing.is_some() {
            return None;
        }
        let (lib, lf) = (self.lib.clone()?, self.lib_file.clone()?);
        self.settle_pending_relayout();
        let hash = doc_hash(&self.doc);
        if hash == self.history.last_version_hash {
            return None;
        }
        match lib.save_version(&lf.id, &self.doc, None, true) {
            Ok(v) => {
                self.history.last_version_hash = hash;
                self.history.dirty = true;
                Some(v)
            }
            Err(e) => {
                self.show_toast(format!("Cannot save a version: {e}"), true);
                None
            }
        }
    }

    /// Called every frame: an automatic version after 10 minutes of editing.
    pub fn history_tick(&mut self) {
        if self.lib_file.is_none() || self.home.is_some() || self.history.viewing.is_some() {
            return;
        }
        let pending = self.autosave.pending();
        if pending && !self.history.was_pending {
            self.history.activity.note_edit(self.now);
        }
        self.history.was_pending = pending;
        if self.history.activity.due() {
            self.history.activity.reset();
            self.save_auto_version();
        }
    }

    /// On close (or leaving the file): a last automatic version of the edits since the previous one.
    pub fn save_close_version(&mut self) {
        self.exit_version_view();
        self.save_auto_version();
    }

    /// "Save to version history…": a named version with an optional description.
    pub fn save_named_version(&mut self, name: &str, description: &str) -> bool {
        if self.history.viewing.is_some() {
            self.show_toast("Go back to the current version first", true);
            return false;
        }
        let (Some(lib), Some(lf)) = (self.lib.clone(), self.lib_file.clone()) else {
            self.show_toast("Save to library to keep version history", true);
            return false;
        };
        let name = name.trim();
        if name.is_empty() {
            return false;
        }
        let desc = (!description.trim().is_empty()).then(|| description.trim().to_string());
        self.settle_pending_relayout();
        match lib.save_version_described(&lf.id, &self.doc, Some(name.to_string()), desc, false) {
            Ok(_) => {
                self.history.last_version_hash = doc_hash(&self.doc);
                self.history.dirty = true;
                self.show_toast(format!("Saved version \"{name}\""), false);
                true
            }
            Err(e) => {
                self.show_toast(format!("Cannot save the version: {e}"), true);
                false
            }
        }
    }

    /// Ctrl+Alt+S: opens the save dialog, or explains why there is no history.
    pub fn begin_save_version(&mut self) {
        if !self.has_history() {
            self.show_toast("Save to library to keep version history", true);
            return;
        }
        if self.history.viewing.is_some() {
            self.show_toast("Go back to the current version first", true);
            return;
        }
        self.history.save_modal = Some(SaveModal { name: String::new(), description: String::new(), fresh: true });
    }

    /// Gives a version a name (the context menu's "Name this version").
    pub fn name_version(&mut self, n: u32, name: &str) {
        let (Some(lib), Some(lf)) = (self.lib.clone(), self.lib_file.clone()) else { return };
        match lib.rename_version(&lf.id, n, Some(name.trim().to_string()), None) {
            Ok(()) => self.history.dirty = true,
            Err(e) => self.show_toast(format!("Cannot name the version: {e}"), true),
        }
    }

    pub fn ask_version_name(&mut self, n: u32) {
        self.refresh_versions();
        let current = self.history.list.iter().find(|v| v.n == n).and_then(|v| v.label.clone()).unwrap_or_default();
        self.dialog = Some(crate::home::Dialog::prompt(crate::home::Intent::NameVersion(n), "Name this version", &current, "Save"));
    }

    /// Moves a version to the trash.
    pub fn delete_version(&mut self, n: u32) {
        let (Some(lib), Some(lf)) = (self.lib.clone(), self.lib_file.clone()) else { return };
        if self.history.viewing.as_ref().is_some_and(|v| v.n == n) {
            self.exit_version_view();
        }
        match lib.delete_version(&lf.id, n) {
            Ok(()) => {
                self.history.dirty = true;
                self.show_toast("Version moved to the trash", false);
            }
            Err(e) => self.show_toast(format!("Cannot delete the version: {e}"), true),
        }
    }

    /// Creates a new library file from a version, next to the open one.
    pub fn duplicate_version(&mut self, n: u32) {
        let (Some(lib), Some(lf)) = (self.lib.clone(), self.lib_file.clone()) else { return };
        self.refresh_versions();
        let title = self.history.list.iter().find(|v| v.n == n).map(version_title).unwrap_or_else(|| format!("version {n}"));
        let result = lib.load_version(&lf.id, n).and_then(|mut doc| {
            let name = format!("{} ({title})", lf.name);
            doc.name = name.clone();
            lib.create_file(&lf.project_id, None, &name, &doc)
        });
        match result {
            Ok(e) => self.show_toast(format!("Created \"{}\" in {}", e.name, lf.project), false),
            Err(e) => self.show_toast(format!("Cannot duplicate the version: {e}"), true),
        }
    }
}

/// Runs the dialogs of this feature; false when `d` is not one of them.
pub fn finish_dialog(app: &mut App, d: &crate::home::Dialog) -> bool {
    use crate::home::Intent;
    let text = d.input.clone().unwrap_or_default().trim().to_string();
    match d.intent {
        Intent::AuthorName => app.set_author(&text),
        Intent::NameVersion(n) => app.name_version(n, &text),
        _ => return false,
    }
    true
}

#[cfg(test)]
mod tests;
