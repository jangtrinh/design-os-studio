//! The native file dialog behind one seam.
//!
//! `rfd` returns `None` both when the user cancels and when no dialog can open at all (Linux without
//! xdg-desktop-portal or zenity). [`DialogOutcome`] keeps the two apart so a caller stays silent on a
//! cancel and says so on a failure. Callers take `&dyn FileDialogs`, so tests drive every outcome
//! without a real dialog.

use crate::app::App;
use std::path::PathBuf;

/// How a file dialog request ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DialogOutcome<T = PathBuf> {
    Picked(T),
    /// The user closed a working dialog. Nothing is written and nothing is said.
    Cancelled,
    /// No dialog could open on this machine.
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestKind {
    Save,
    Open,
    Folder,
}

/// A file dialog described without a toolkit type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileRequest {
    pub kind: RequestKind,
    pub title: Option<String>,
    pub file_name: Option<String>,
    pub filters: Vec<(String, Vec<String>)>,
}

impl FileRequest {
    fn new(kind: RequestKind) -> Self {
        Self { kind, title: None, file_name: None, filters: Vec::new() }
    }

    pub fn open() -> Self {
        Self::new(RequestKind::Open)
    }

    pub fn save(file_name: impl Into<String>) -> Self {
        Self { file_name: Some(file_name.into()), ..Self::new(RequestKind::Save) }
    }

    pub fn folder(title: impl Into<String>) -> Self {
        Self { title: Some(title.into()), ..Self::new(RequestKind::Folder) }
    }

    pub fn filter(mut self, label: &str, extensions: &[&str]) -> Self {
        self.filters.push((label.to_string(), extensions.iter().map(|e| e.to_string()).collect()));
        self
    }
}

pub trait FileDialogs {
    /// One file or folder, or the path a save goes to.
    fn pick(&self, request: &FileRequest) -> DialogOutcome;
    /// Several existing files.
    fn pick_many(&self, request: &FileRequest) -> DialogOutcome<Vec<PathBuf>>;
    /// The folder an export lands in when no save dialog can open.
    fn fallback_dir(&self) -> Option<PathBuf>;
}

/// The operating system's dialog, through `rfd`.
pub struct SystemDialogs;

impl FileDialogs for SystemDialogs {
    fn pick(&self, request: &FileRequest) -> DialogOutcome {
        match show_with_rfd(request) {
            Some(path) => DialogOutcome::Picked(path),
            None => DialogOutcome::Cancelled,
        }
    }

    fn pick_many(&self, request: &FileRequest) -> DialogOutcome<Vec<PathBuf>> {
        match rfd_dialog(request).pick_files() {
            Some(paths) => DialogOutcome::Picked(paths),
            None => DialogOutcome::Cancelled,
        }
    }

    fn fallback_dir(&self) -> Option<PathBuf> {
        None
    }
}

fn rfd_dialog(request: &FileRequest) -> rfd::FileDialog {
    let mut dialog = rfd::FileDialog::new();
    if let Some(title) = &request.title {
        dialog = dialog.set_title(title.as_str());
    }
    for (label, extensions) in &request.filters {
        dialog = dialog.add_filter(label.as_str(), extensions);
    }
    if let Some(name) = &request.file_name {
        dialog = dialog.set_file_name(name.as_str());
    }
    dialog
}

fn show_with_rfd(request: &FileRequest) -> Option<PathBuf> {
    let dialog = rfd_dialog(request);
    match request.kind {
        RequestKind::Save => dialog.save_file(),
        RequestKind::Open => dialog.pick_file(),
        RequestKind::Folder => dialog.pick_folder(),
    }
}

impl App {
    /// The path a dialog gave, or `None` after a cancel or a failure.
    pub fn picked_path(&mut self, outcome: DialogOutcome) -> Option<PathBuf> {
        match outcome {
            DialogOutcome::Picked(path) => Some(path),
            DialogOutcome::Cancelled | DialogOutcome::Unavailable => None,
        }
    }

    /// The paths a multi-select dialog gave, or `None` after a cancel or a failure.
    pub fn picked_paths(&mut self, outcome: DialogOutcome<Vec<PathBuf>>) -> Option<Vec<PathBuf>> {
        match outcome {
            DialogOutcome::Picked(paths) => Some(paths),
            DialogOutcome::Cancelled | DialogOutcome::Unavailable => None,
        }
    }
}

/// A dialog stand-in for tests: answers every request with one outcome and records what was asked.
#[cfg(test)]
pub struct ScriptedDialogs {
    pub outcome: DialogOutcome,
    pub fallback: Option<PathBuf>,
    pub requests: std::cell::RefCell<Vec<FileRequest>>,
}

#[cfg(test)]
impl ScriptedDialogs {
    pub fn answering(outcome: DialogOutcome) -> Self {
        Self { outcome, fallback: None, requests: std::cell::RefCell::new(Vec::new()) }
    }

    pub fn with_fallback(mut self, dir: PathBuf) -> Self {
        self.fallback = Some(dir);
        self
    }
}

#[cfg(test)]
impl FileDialogs for ScriptedDialogs {
    fn pick(&self, request: &FileRequest) -> DialogOutcome {
        self.requests.borrow_mut().push(request.clone());
        self.outcome.clone()
    }

    fn pick_many(&self, request: &FileRequest) -> DialogOutcome<Vec<PathBuf>> {
        self.requests.borrow_mut().push(request.clone());
        match &self.outcome {
            DialogOutcome::Picked(path) => DialogOutcome::Picked(vec![path.clone()]),
            DialogOutcome::Cancelled => DialogOutcome::Cancelled,
            DialogOutcome::Unavailable => DialogOutcome::Unavailable,
        }
    }

    fn fallback_dir(&self) -> Option<PathBuf> {
        self.fallback.clone()
    }
}
