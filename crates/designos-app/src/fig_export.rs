//! Writing a `.fig` export. The Home menu, the File menu and the command palette share one destination rule,
//! never replace a file, and tell the user where the file went or why it did not.

use crate::app::App;
use designos_doc::Document;
use std::ffi::OsString;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Names tried before giving up: `Name.fig`, `Name 2.fig`, up to `Name 999.fig`.
const MAX_NAMES: u32 = 999;
/// Longest file name stem taken from a document name.
const MAX_STEM_CHARS: usize = 100;

/// A finished export.
pub struct Exported {
    pub path: PathBuf,
    /// Extension keys of the document that the `.fig` format cannot carry.
    pub omitted_keys: usize,
}

/// A file name stem for a document name: path separators, reserved and control characters become `_`,
/// leading dots go, and an empty result is `Untitled`.
pub fn safe_stem(name: &str) -> String {
    let cleaned: String = name.chars().map(|c| if c.is_control() || "/\\:*?\"<>|".contains(c) { '_' } else { c }).take(MAX_STEM_CHARS).collect();
    let trimmed = cleaned.trim().trim_start_matches('.').trim_end_matches(['.', ' ']);
    if trimmed.trim_matches('_').is_empty() {
        "Untitled".to_string()
    } else {
        trimmed.to_string()
    }
}

/// Writes `doc` as `<dir>/<stem>.fig`, or `<stem> 2.fig`, `<stem> 3.fig`, ... when that name is taken.
/// An existing file is never replaced, not even the one open in the editor: the name is claimed with
/// `create_new`. A write that fails leaves no partial file behind.
pub fn export_new(doc: &Document, dir: &Path, stem: &str) -> Result<Exported, String> {
    let (bytes, omitted) = designos_fig::export_fig_with_report(doc).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    for n in 1..=MAX_NAMES {
        let name = if n == 1 { format!("{stem}.fig") } else { format!("{stem} {n}.fig") };
        let path = dir.join(name);
        let mut file = match std::fs::OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(file) => file,
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(format!("cannot write {}: {e}", path.display())),
        };
        if let Err(e) = file.write_all(&bytes).and_then(|()| file.sync_all()) {
            drop(file);
            let _ = std::fs::remove_file(&path);
            return Err(format!("cannot write {}: {e}", path.display()));
        }
        return Ok(Exported { path, omitted_keys: omitted.len() });
    }
    Err(format!("{} already holds {MAX_NAMES} exports named {stem}", dir.display()))
}

impl App {
    /// Where an export of the open document goes and what it is called: next to a file opened from disk, otherwise
    /// the library `exports` folder (the folder the Home menu uses).
    pub(crate) fn fig_export_target(&self) -> Result<(PathBuf, String), String> {
        if let Some(file) = &self.file_path {
            let dir = file.parent().filter(|d| !d.as_os_str().is_empty()).map_or_else(|| PathBuf::from("."), Path::to_path_buf);
            let stem = file.file_stem().map(|s| s.to_string_lossy().into_owned()).filter(|s| !s.is_empty()).unwrap_or_else(|| safe_stem(&self.doc.name));
            return Ok((dir, stem));
        }
        match &self.lib {
            Some(lib) => Ok((lib.path("exports"), safe_stem(&self.doc.name))),
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
                self.show_toast_with_reveal(format!("Exported .fig to {}{note}", done.path.display()), done.path);
            }
            Err(e) => self.show_toast(format!("Export .fig failed: {e}"), true),
        }
    }
}

/// The program and arguments that show `file` in the system file manager on `os` (a `std::env::consts::OS` value).
fn reveal_command(os: &str, file: &Path) -> (&'static str, Vec<OsString>) {
    match os {
        "macos" => ("open", vec!["-R".into(), file.into()]),
        "windows" => {
            let mut select = OsString::from("/select,");
            select.push(file);
            ("explorer", vec![select])
        }
        _ => ("xdg-open", vec![file.parent().unwrap_or(file).into()]),
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
mod tests;
