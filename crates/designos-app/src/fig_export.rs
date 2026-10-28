//! Writing a `.fig` export. The Home menu, the File menu and the command palette share one destination rule,
//! never replace a file, and tell the user where the file went or why it did not.

use crate::app::App;
use designos_doc::Document;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// A finished export.
pub struct Exported {
    pub path: PathBuf,
    /// Extension keys of the document that the `.fig` format cannot carry.
    pub omitted_keys: usize,
    /// Comment threads of the exported document, which the `.fig` format has no place for.
    pub omitted_comment_threads: usize,
}

/// Writes `doc` as `<dir>/<stem>.fig`, or `<stem> 2.fig`, `<stem> 3.fig`, ... when that name is taken.
/// The file name is cleaned and cut to fit the file system by [`crate::io::write_new_in`], which never replaces a
/// file (not even the one open in the editor) and leaves no partial file behind. A stem with nothing to name the
/// file by (blank, only separators) becomes `Untitled`.
pub fn export_new(doc: &Document, dir: &Path, stem: &str) -> Result<Exported, String> {
    let (bytes, omitted) = designos_fig::export_fig_with_report(doc).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    let clean = designos_export::sanitize_file_name(stem);
    let stem = if stem.trim().is_empty() || clean.trim_matches('_').is_empty() { "Untitled" } else { clean.as_str() };
    let path = crate::io::write_new_in(dir, &format!("{stem}.fig"), &bytes).map_err(|e| format!("cannot write to {}: {e}", dir.display()))?;
    let omitted_comment_threads = crate::io::comment_threads_left_out(doc, &path);
    Ok(Exported { path, omitted_keys: omitted.len(), omitted_comment_threads })
}

impl App {
    /// Where an export of the open document goes and what it is called: next to a file opened from disk, otherwise
    /// the library `exports` folder (the folder the Home menu uses).
    pub(crate) fn fig_export_target(&self) -> Result<(PathBuf, String), String> {
        if let Some(file) = &self.file_path {
            let dir = file.parent().filter(|d| !d.as_os_str().is_empty()).map_or_else(|| PathBuf::from("."), Path::to_path_buf);
            // The toast shows the full path, so a file opened by a relative path exports to an absolute one.
            let dir = std::path::absolute(&dir).unwrap_or(dir);
            let stem = file.file_stem().map(|s| s.to_string_lossy().into_owned()).filter(|s| !s.is_empty()).unwrap_or_else(|| self.doc.name.clone());
            return Ok((dir, stem));
        }
        match &self.lib {
            Some(lib) => Ok((lib.path("exports"), self.doc.name.clone())),
            None => Err("save the file first".to_string()),
        }
    }

    /// Tells the user how an export went: the full path with a "Show in folder" action, or why it failed.
    pub fn show_fig_export_result(&mut self, result: Result<Exported, String>) {
        match result {
            Ok(done) => {
                let note = match done.omitted_keys {
                    0 => String::new(),
                    1 => ". 1 extension key not carried".to_string(),
                    n => format!(". {n} extension keys not carried"),
                };
                let comments_left_out = done.omitted_comment_threads > 0;
                let comments_note = if comments_left_out { format!(". {}", crate::io::COMMENTS_ONLY_IN_NATIVE_NOTICE) } else { String::new() };
                self.show_toast_with_reveal(format!("Exported .fig to {}{note}{comments_note}", done.path.display()), done.path);
                // Comments left out are a warning, the same flag a save to `.fig` raises.
                if let Some(toast) = self.toast.as_mut().filter(|_| comments_left_out) {
                    toast.error = true;
                }
            }
            Err(e) => self.show_toast(format!("Export .fig failed: {e}"), true),
        }
    }
}

/// The program and arguments that show `file` in the system file manager on `os` (a `std::env::consts::OS` value).
fn reveal_command(os: &str, file: &Path) -> (&'static str, Vec<OsString>) {
    match os {
        "macos" => ("open", vec!["-R".into(), file.into()]),
        // Two arguments: a path with spaces is then quoted on its own, the form `explorer /select, "C:\My Design.fig"`.
        "windows" => ("explorer", vec!["/select,".into(), file.into()]),
        _ => {
            let dir = if file.is_dir() { file } else { file.parent().unwrap_or(file) };
            ("xdg-open", vec![dir.into()])
        }
    }
}

/// Shows `file` in the system file manager. A test never opens one: it replaces this through `App::reveal_file`.
pub fn reveal_file(file: &Path) -> Result<(), String> {
    if cfg!(test) {
        return Err(format!("A test cannot open the file manager for {}", file.display()));
    }
    let (program, args) = reveal_command(std::env::consts::OS, file);
    let mut child =
        std::process::Command::new(program).args(&args).spawn().map_err(|e| format!("Cannot open the folder: {e}. The file is at {}", file.display()))?;
    // Collect the exit status off the UI thread so the finished file manager launcher does not linger.
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}

#[cfg(test)]
mod comment_notice_tests;
#[cfg(test)]
pub(crate) mod tests;
