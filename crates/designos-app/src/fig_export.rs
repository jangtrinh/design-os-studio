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
    let path = crate::io::write_new_in(dir, &format!("{stem}.fig"), &bytes).map_err(|e| format!("cannot write to {}: {e}", e.path.display()))?;
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
                // Comments left out are a warning, the same flag a save to `.fig` raises.
                self.show_toast_with_reveal_status(format!("Exported .fig to {}{note}{comments_note}", done.path.display()), comments_left_out, done.path);
            }
            Err(e) => self.show_toast(format!("Export .fig failed: {e}"), true),
        }
    }
}

/// The folder that holds `file`, or `file` itself when it is a folder: what a failed reveal names.
fn reveal_folder(file: &Path) -> &Path {
    if file.is_dir() {
        file
    } else {
        file.parent().filter(|d| !d.as_os_str().is_empty()).unwrap_or(file)
    }
}

/// The error toast for a reveal that did not work: short, and it names the folder the user wanted to see.
pub(crate) fn reveal_failure_text(file: &Path, reason: &str) -> String {
    let folder = reveal_folder(file).display();
    let reason = reason.trim();
    if reason.is_empty() {
        format!("Couldn't open {folder}")
    } else {
        format!("Couldn't open {folder}: {reason}")
    }
}

/// The program and arguments that show `file` in the system file manager on `os` (a `std::env::consts::OS` value).
/// `file_exists` is false for a file that has been moved or deleted since the export: its folder opens instead.
fn reveal_command(os: &str, file: &Path, file_exists: bool) -> (&'static str, Vec<OsString>) {
    match os {
        "macos" if file_exists => ("open", vec!["-R".into(), file.into()]),
        "macos" => ("open", vec![reveal_folder(file).into()]),
        // The arguments are passed as written (see `spawn_reveal`). Explorer splits `/select,` from the path at the
        // first comma it finds outside quotes, so a path such as `C:\a,b\f.fig` only survives inside quotes.
        "windows" if file_exists => ("explorer", vec![windows_select_argument(file)]),
        "windows" => ("explorer", vec![windows_quoted(reveal_folder(file))]),
        _ => ("xdg-open", vec![reveal_folder(file).into()]),
    }
}

fn windows_quoted(path: &Path) -> OsString {
    let mut arg = OsString::from("\"");
    arg.push(path);
    // A backslash right before the closing quote reads as an escaped quote (`"D:\"`). `D:\.` is the same folder.
    if path.to_string_lossy().ends_with(['\\', '/']) {
        arg.push(".");
    }
    arg.push("\"");
    arg
}

/// `/select,"<path>"`: one argument, the form `explorer /select,"C:\My Design.fig"` expects.
fn windows_select_argument(file: &Path) -> OsString {
    let mut arg = OsString::from("/select,");
    arg.push(windows_quoted(file));
    arg
}

/// Launches the file manager. On Windows the arguments reach Explorer as written, because `Command::arg` would
/// escape the quotes that keep a comma in the path from splitting it.
fn spawn_reveal(program: &str, args: &[OsString]) -> std::io::Result<std::process::Child> {
    let mut command = std::process::Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        for arg in args {
            command.raw_arg(arg);
        }
    }
    #[cfg(not(windows))]
    command.args(args);
    command.spawn()
}

/// Launcher failures that showed up after the launcher had started, worded for the error toast. The UI thread
/// takes them each frame.
#[cfg(not(test))]
static LATE_REVEAL_FAILURES: std::sync::Mutex<Vec<(PathBuf, String)>> = std::sync::Mutex::new(Vec::new());

#[cfg(test)]
thread_local! {
    /// A test thread keeps its own failures, so tests that run side by side never take each other's.
    static TEST_LATE_FAILURES: std::cell::RefCell<Vec<(PathBuf, String)>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Records a failure for the file whose Show in folder it was.
pub(crate) fn push_late_reveal_failure(file: PathBuf, text: String) {
    #[cfg(test)]
    TEST_LATE_FAILURES.with(|f| f.borrow_mut().push((file, text)));
    #[cfg(not(test))]
    LATE_REVEAL_FAILURES.lock().unwrap_or_else(|e| e.into_inner()).push((file, text));
}

/// The next failure of a launcher that started but then did not open anything, with the file it was for.
pub(crate) fn take_late_reveal_failure() -> Option<(PathBuf, String)> {
    #[cfg(test)]
    return TEST_LATE_FAILURES.with(|f| {
        let mut failures = f.borrow_mut();
        (!failures.is_empty()).then(|| failures.remove(0))
    });
    #[cfg(not(test))]
    {
        let mut failures = LATE_REVEAL_FAILURES.lock().unwrap_or_else(|e| e.into_inner());
        (!failures.is_empty()).then(|| failures.remove(0))
    }
}

/// The window's context, so the watcher thread can wake the UI once when it has a failure to show. One wake, not a
/// repaint loop: the UI stays idle otherwise.
static REPAINT_CONTEXT: std::sync::OnceLock<egui::Context> = std::sync::OnceLock::new();

pub(crate) fn remember_repaint_context(ctx: &egui::Context) {
    let _ = REPAINT_CONTEXT.set(ctx.clone());
}

/// Why a launcher that has finished did not open anything, or `None` when it worked. Explorer exits with 1 even
/// when it worked, so `check_exit` is false there.
fn launcher_failure(status: std::io::Result<std::process::ExitStatus>, check_exit: bool) -> Option<String> {
    match status {
        Ok(status) if check_exit && !status.success() => Some(format!("the file manager exited with {status}")),
        Ok(_) => None,
        Err(e) => Some(e.to_string()),
    }
}

/// Collects the exit status off the UI thread so the finished launcher does not linger. A launcher that exits with
/// an error (`xdg-open` with no file manager to hand off to, `open` on a path that is gone) is reported through
/// [`take_late_reveal_failure`].
pub(crate) fn watch_reveal(mut child: std::process::Child, file: PathBuf, check_exit: bool) {
    std::thread::spawn(move || {
        if let Some(reason) = launcher_failure(child.wait(), check_exit) {
            eprintln!("designos: warn: show in folder failed for {}: {reason}", file.display());
            push_late_reveal_failure(file.clone(), reveal_failure_text(&file, ""));
            if let Some(ctx) = REPAINT_CONTEXT.get() {
                ctx.request_repaint();
            }
        }
    });
}

/// Shows `file` in the system file manager. A test never opens one: it replaces this through `App::reveal_file`.
/// The error is the reason only; the caller names the folder.
pub fn reveal_file(file: &Path) -> Result<(), String> {
    if cfg!(test) {
        return Err(format!("A test cannot open the file manager for {}", file.display()));
    }
    if !reveal_folder(file).is_dir() {
        return Err("the folder is gone".to_string());
    }
    let os = std::env::consts::OS;
    let (program, args) = reveal_command(os, file, file.exists());
    let child = spawn_reveal(program, &args).map_err(|e| {
        eprintln!("designos: warn: cannot start {program}: {e}");
        "no file manager".to_string()
    })?;
    watch_reveal(child, file.to_path_buf(), os != "windows");
    Ok(())
}

#[cfg(test)]
mod comment_notice_tests;
#[cfg(test)]
pub(crate) mod tests;
