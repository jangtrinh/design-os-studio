//! Why Studio refused a path as a session folder, or could not use one, and the line a user reads about it: the cause comes
//! with the error, and the caller that knows what the user was doing adds what to do next.

use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

/// What sits where the session folder should be.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(not(unix), allow(dead_code))]
pub enum Refusal {
    Link,
    File,
    OtherOwner,
}

/// The error a refused path carries inside its [`io::Error`]: the cause only, with no word on what to do.
#[derive(Debug)]
struct Refused {
    kind: Refusal,
    dir: PathBuf,
}

impl fmt::Display for Refused {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let dir = self.dir.display();
        match self.kind {
            Refusal::Link => write!(f, "{dir} is a link, not a folder"),
            Refusal::File => write!(f, "{dir} is a file, not a folder"),
            Refusal::OtherOwner => write!(f, "{dir} belongs to another user"),
        }
    }
}

impl std::error::Error for Refused {}

/// What a user can do about a path that failed for a reason other than a refusal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(not(unix), allow(dead_code))]
pub enum Fix {
    /// The folder cannot be made or read: its permissions, or a full disk.
    CheckPermissions,
    /// The folder the line names cannot be written into: an agent (or the user) made it read-only.
    MakeWritable,
    /// Studio has no private data folder: only a restart helps.
    HomeFolder,
    /// A file Studio had just written was replaced by something else: another agent tab of the same folder did it.
    CloseOtherTabs,
    /// A session path Studio does not recognise: opening the file again gives its tabs a session folder Studio made.
    ReopenFile,
    /// A link stands where a plain file should be: deleting it lets the agent or Studio write the file again.
    DeleteLink,
    /// A FIFO or a folder stands where a plain file should be.
    DeleteIt,
    /// An entry that is not Studio's stands where a CLI would run it and Studio cannot move it aside (a platform without `openat`, a
    /// mount point, a file system that cannot move without replacing): the user moves or deletes it.
    MoveOrDeleteIt,
    /// An entry or a folder is locked (macOS "Locked", `chflags uchg`), which no permission bit shows.
    #[cfg_attr(not(target_vendor = "apple"), allow(dead_code))]
    Unlock,
    /// Something refused a move and nothing Studio can read says which of an entry and the folder it is in (an ACL that denies the
    /// delete): the line names both, the folder by its path, and the user checks both.
    CheckItAndItsFolder,
}

impl Fix {
    /// What to do, then what to do next (`then`; a missing home folder needs a restart whatever the caller was doing).
    fn sentence(self, then: &str) -> String {
        match self {
            Fix::CheckPermissions => format!("Check its permissions and free space, then {then}"),
            Fix::MakeWritable => format!("Make it writable or delete it, then {then}"),
            Fix::HomeFolder => "Make sure your home folder exists, then restart Studio".to_string(),
            Fix::CloseOtherTabs => format!("Close this file's other agent tabs, then {then}"),
            Fix::ReopenFile => format!("Reopen this design file, then {then}"),
            Fix::DeleteLink => format!("Delete the link, then {then}"),
            Fix::DeleteIt => format!("Delete it, then {then}"),
            Fix::MoveOrDeleteIt => format!("Move or delete it, then {then}"),
            Fix::Unlock => format!("Unlock it, then {then}"),
            Fix::CheckItAndItsFolder => format!("Check its permissions and those of that folder, then {then}"),
        }
    }
}

/// The error of a path that could not be used when it is not a refusal: the cause only, and what to do about it apart.
#[derive(Debug)]
struct Failed {
    message: String,
    fix: Fix,
}

impl fmt::Display for Failed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for Failed {}

pub(super) fn refused(kind: Refusal, dir: &Path) -> io::Error {
    io::Error::new(io::ErrorKind::PermissionDenied, Refused { kind, dir: dir.to_path_buf() })
}

/// A start that moved entries aside before it failed: the error, and what to tell the user about the entries, since they were moved
/// whether or not the tab started.
#[derive(Debug)]
struct AfterMoves {
    cause: io::Error,
    note: String,
}

impl fmt::Display for AfterMoves {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.cause, f)
    }
}

impl std::error::Error for AfterMoves {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.cause)
    }
}

/// `e` together with `note`, which says what was moved aside before it happened. The cause, its kind and its fix are unchanged.
pub fn with_set_aside_note(e: io::Error, note: String) -> io::Error {
    io::Error::new(e.kind(), AfterMoves { cause: e, note })
}

/// The note of [`with_set_aside_note`] on `e`, if it has one.
fn note_of(e: &io::Error) -> Option<&str> {
    e.get_ref()?.downcast_ref::<AfterMoves>().map(|after| after.note.as_str())
}

/// `e` without the note of [`with_set_aside_note`]: the error whose refusal or fix a line is made of.
fn cause_of(e: &io::Error) -> &io::Error {
    match e.get_ref().and_then(|inner| inner.downcast_ref::<AfterMoves>()) {
        Some(after) => &after.cause,
        None => e,
    }
}

/// An error of `kind` whose cause is `message`, with the `fix` a caller adds to its own next step.
pub(super) fn failed(kind: io::ErrorKind, message: String, fix: Fix) -> io::Error {
    io::Error::new(kind, Failed { message, fix })
}

/// `e` with the path it happened at, "<path> can't be <what>: <e>", and the `fix` a caller adds to its own next step.
pub(super) fn cannot(what: &str, path: &Path, e: io::Error, fix: Fix) -> io::Error {
    failed(e.kind(), format!("{} can't be {what}: {e}", path.display()), fix)
}

/// Why `e` refused a path, when it did (any other error, such as a full disk, is `None`).
pub fn refusal_of(e: &io::Error) -> Option<Refusal> {
    cause_of(e).get_ref()?.downcast_ref::<Refused>().map(|refused| refused.kind)
}

impl Refusal {
    /// What to do about the refused path, then what to do next (`then`), in one clause: "Delete the link and reopen this design
    /// file". The library's own lines keep this form.
    fn recovery(self, then: &str) -> String {
        match self {
            Refusal::Link => format!("Delete the link and {then}"),
            Refusal::File => format!("Delete the file at that path and {then}"),
            Refusal::OtherOwner => format!("Move or delete the folder and {then}"),
        }
    }

    /// The same, in the form of every other line: "Delete the link, then try again".
    fn recovery_then(self, then: &str) -> String {
        match self {
            Refusal::Link => format!("Delete the link, then {then}"),
            Refusal::File => format!("Delete the file at that path, then {then}"),
            Refusal::OtherOwner => format!("Move or delete the folder, then {then}"),
        }
    }
}

/// What to do about `e`, then what to do next (`then`), when it is a refusal or a failure with a known fix (any other error
/// carries its own). `one_clause` is the form of the library's lines, "... and reopen this design file".
fn recovery_of(e: &io::Error, then: &str, one_clause: bool) -> Option<String> {
    if let Some(kind) = refusal_of(e) {
        return Some(if one_clause { kind.recovery(then) } else { kind.recovery_then(then) });
    }
    cause_of(e).get_ref()?.downcast_ref::<Failed>().map(|failed| failed.fix.sentence(then))
}

/// `e` as a sentence without its last period, with what to do next when it is a refusal or a failure with a known fix.
pub fn with_next_step(e: &io::Error) -> String {
    match recovery_of(e, "try again", false) {
        Some(recovery) => format!("{e}. {recovery}"),
        None => e.to_string(),
    }
}

/// The status line of a tab (`tab`) that could not start because of `e`, from [`super::ensure_session_dir`]. Entries moved aside before
/// it failed are named after the cause, so the user learns about both.
pub fn start_failure_message(tab: &str, e: &io::Error) -> String {
    let line = format!("Couldn't start {tab}: {}.", with_next_step(e));
    match note_of(e) {
        Some(note) => format!("{line} {note}"),
        None => line,
    }
}

/// The status line of the files Studio keeps in a session folder when they could not be updated because of `e`. The error
/// names its own path, so the line does not.
pub fn files_not_updated_message(e: &io::Error) -> String {
    format!("Couldn't update the agent files: {}.", with_next_step(e))
}

/// The status line of a library file whose own session folder was refused for `e`: the cause, then what happens to its agents
/// (only when the temporary folder they start in works, `fallback_works`), then what to do.
pub fn library_fallback_message(e: &io::Error, fallback_works: bool) -> String {
    let mut parts = vec![format!("Couldn't use this file's agent folder: {e}.")];
    if fallback_works {
        parts.push("This file's agents start in a temporary folder that is not kept.".to_string());
    }
    parts.extend(recovery_of(e, "reopen this design file", true).map(|recovery| format!("{recovery}.")));
    parts.join(" ")
}
