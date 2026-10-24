//! Why Studio refused a path as a session folder, and the line a user reads about it: the cause comes with the error, and the
//! caller that knows what the user was doing adds what to do next.

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

pub(super) fn refused(kind: Refusal, dir: &Path) -> io::Error {
    io::Error::new(io::ErrorKind::PermissionDenied, Refused { kind, dir: dir.to_path_buf() })
}

/// Why `e` refused a path, when it did (any other error, such as a full disk, is `None`).
pub fn refusal_of(e: &io::Error) -> Option<Refusal> {
    e.get_ref()?.downcast_ref::<Refused>().map(|refused| refused.kind)
}

impl Refusal {
    /// What to do about the refused path, then what to do next (`then`).
    fn recovery(self, then: &str) -> String {
        match self {
            Refusal::Link => format!("Delete the link and {then}"),
            Refusal::File => format!("Delete the file at that path and {then}"),
            Refusal::OtherOwner => format!("Move or delete the folder and {then}"),
        }
    }
}

/// `e` as a sentence without its last period, with what to do next when it is a refusal (any other error carries its own).
pub fn with_next_step(e: &io::Error) -> String {
    match refusal_of(e) {
        Some(kind) => format!("{e}. {}", kind.recovery("try again")),
        None => e.to_string(),
    }
}

/// The status line of a tab (`tab`) that could not start because of `e`, from [`super::ensure_session_dir`].
pub fn start_failure_message(tab: &str, e: &io::Error) -> String {
    format!("Couldn't start {tab}: {}.", with_next_step(e))
}

/// The status line of a library file whose own session folder was refused for `e`. Its agents start in a temporary folder,
/// which the line says only when that folder works (`fallback_works`).
pub fn library_fallback_message(e: &io::Error, fallback_works: bool) -> String {
    let lead = format!("Couldn't use this file's agent folder: {e}.");
    let lead = if fallback_works { format!("{lead} This file's agents start in a temporary folder that is not kept.") } else { lead };
    match refusal_of(e) {
        Some(kind) => format!("{lead} {}.", kind.recovery("reopen this design file")),
        None => lead,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_refusal_carries_its_cause_and_each_kind_has_its_own_recovery() {
        let dir = Path::new("/data/sessions/p1-tab-1");
        for (kind, cause, start, reopen) in [
            (Refusal::Link, "is a link, not a folder", "Delete the link and try again.", "Delete the link and reopen this design file."),
            (
                Refusal::File,
                "is a file, not a folder",
                "Delete the file at that path and try again.",
                "Delete the file at that path and reopen this design file.",
            ),
            (
                Refusal::OtherOwner,
                "belongs to another user",
                "Move or delete the folder and try again.",
                "Move or delete the folder and reopen this design file.",
            ),
        ] {
            let e = refused(kind, dir);
            assert_eq!(refusal_of(&e), Some(kind));
            assert_eq!(e.to_string(), format!("{} {cause}", dir.display()), "the error says what is wrong and nothing else");
            assert_eq!(start_failure_message("Claude Code", &e), format!("Couldn't start Claude Code: {} {cause}. {start}", dir.display()));
            let lead =
                format!("Couldn't use this file's agent folder: {} {cause}. This file's agents start in a temporary folder that is not kept.", dir.display());
            assert_eq!(library_fallback_message(&e, true), format!("{lead} {reopen}"));
            let no_fallback = format!("Couldn't use this file's agent folder: {} {cause}. {reopen}", dir.display());
            assert_eq!(library_fallback_message(&e, false), no_fallback, "no word on a temporary folder that cannot be used");
        }
    }

    #[test]
    fn another_error_carries_its_own_next_step_and_is_not_a_refusal() {
        let e = io::Error::other("disk full. Free some space and try again");
        assert_eq!(refusal_of(&e), None);
        assert_eq!(start_failure_message("Claude Code", &e), "Couldn't start Claude Code: disk full. Free some space and try again.");
        assert!(library_fallback_message(&e, true).ends_with("This file's agents start in a temporary folder that is not kept."));
        assert!(!library_fallback_message(&e, false).contains("temporary folder"));
    }
}
