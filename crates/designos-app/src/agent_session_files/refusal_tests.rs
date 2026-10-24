//! The lines a user reads about a refused or failed session path: the cause, what happens, then what to do.

use super::refusal::*;
use std::io;
use std::path::Path;

#[test]
fn a_refusal_carries_its_cause_and_each_kind_has_its_own_recovery() {
    let dir = Path::new("/data/sessions/p1-tab-1");
    for (kind, cause, start, reopen) in [
        (Refusal::Link, "is a link, not a folder", "Delete the link, then try again.", "Delete the link and reopen this design file."),
        (
            Refusal::File,
            "is a file, not a folder",
            "Delete the file at that path, then try again.",
            "Delete the file at that path and reopen this design file.",
        ),
        (
            Refusal::OtherOwner,
            "belongs to another user",
            "Move or delete the folder, then try again.",
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
fn a_failure_with_a_known_fix_puts_the_consequence_before_the_recovery() {
    let dir = Path::new("/data/sessions/p1-tab-1");
    let denied = || io::Error::from(io::ErrorKind::PermissionDenied);
    let e = cannot("created", dir, denied(), Fix::CheckPermissions);
    assert_eq!(refusal_of(&e), None);
    assert_eq!(e.kind(), io::ErrorKind::PermissionDenied, "the kind of the cause is kept");
    let cause = format!("{} can't be created: {}", dir.display(), denied());
    assert_eq!(e.to_string(), cause, "the error says what failed and nothing else");
    assert_eq!(start_failure_message("Claude Code", &e), format!("Couldn't start Claude Code: {cause}. Check its permissions and free space, then try again."));
    assert_eq!(
        library_fallback_message(&e, true),
        format!(
            "Couldn't use this file's agent folder: {cause}. This file's agents start in a temporary folder that is not kept. \
             Check its permissions and free space, then reopen this design file."
        )
    );
    assert_eq!(
        library_fallback_message(&e, false),
        format!("Couldn't use this file's agent folder: {cause}. Check its permissions and free space, then reopen this design file.")
    );
}

#[test]
fn every_fix_names_what_to_do_and_a_missing_home_folder_always_needs_a_restart() {
    let path = Path::new("/s/.gemini/settings.json");
    let step = |fix| with_next_step(&cannot("written to", path, io::Error::from(io::ErrorKind::PermissionDenied), fix));
    assert!(step(Fix::MakeWritable).ends_with(". Make it writable or delete it, then try again"));
    assert!(step(Fix::CloseOtherTabs).ends_with(". Close this file's other agent tabs, then try again"));
    assert!(step(Fix::DeleteIt).ends_with(". Delete it, then try again"));
    assert!(step(Fix::DeleteLink).ends_with(". Delete the link, then try again"));
    assert!(step(Fix::ReopenFile).ends_with(". Reopen this design file, then try again"));
    let home = failed(io::ErrorKind::Other, "Studio has no private data folder".to_string(), Fix::HomeFolder);
    assert_eq!(
        library_fallback_message(&home, false),
        "Couldn't use this file's agent folder: Studio has no private data folder. Make sure your home folder exists, then restart Studio."
    );
}

#[test]
fn another_error_carries_its_own_next_step_and_is_not_a_refusal() {
    let e = io::Error::other("disk full. Free some space and try again");
    assert_eq!(refusal_of(&e), None);
    assert_eq!(start_failure_message("Claude Code", &e), "Couldn't start Claude Code: disk full. Free some space and try again.");
    assert!(library_fallback_message(&e, true).ends_with("This file's agents start in a temporary folder that is not kept."));
    assert!(!library_fallback_message(&e, false).contains("temporary folder"));
}

#[test]
fn the_line_of_files_not_updated_names_the_path_once() {
    let e = refused(Refusal::Link, Path::new("/data/sessions/p1-tab-1/.gemini"));
    let line = files_not_updated_message(&e);
    assert_eq!(line, "Couldn't update the agent files: /data/sessions/p1-tab-1/.gemini is a link, not a folder. Delete the link, then try again.");
    assert_eq!(line.matches("/.gemini").count(), 1);
}
