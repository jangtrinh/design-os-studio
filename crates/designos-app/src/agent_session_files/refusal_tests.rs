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

#[test]
fn a_locked_entry_and_an_entry_nothing_can_be_read_about_each_get_their_own_next_step() {
    let path = Path::new("/data/sessions/p1-tab-1/.mcp.json");
    let locked = failed(io::ErrorKind::PermissionDenied, format!("{} is locked, so Studio can't move it", path.display()), Fix::Unlock);
    assert_eq!(
        start_failure_message("Grok Build", &locked),
        "Couldn't start Grok Build: /data/sessions/p1-tab-1/.mcp.json is locked, so Studio can't move it. Unlock it, then try again."
    );
    let folder = path.parent().expect("a folder");
    let unknown = failed(
        io::ErrorKind::PermissionDenied,
        format!("{} can't be moved out of {}: permission denied", path.display(), folder.display()),
        Fix::CheckItAndItsFolder,
    );
    let said = start_failure_message("Grok Build", &unknown);
    assert_eq!(
        said,
        "Couldn't start Grok Build: /data/sessions/p1-tab-1/.mcp.json can't be moved out of /data/sessions/p1-tab-1: permission denied. \
         Check its permissions and those of that folder, then try again."
    );
    assert!(!said.contains("Make it writable"), "{said}");
}

#[test]
fn a_note_on_what_was_moved_aside_comes_after_the_cause_and_its_fix_and_changes_neither() {
    let dir = Path::new("/data/sessions/p1-tab-1");
    let note = "Moved .mcp.json to .designos-set-aside/20261025-101500-a1b2c3 in this file's agent folder, so Grok won't run it.";
    let refused_link = refused(Refusal::Link, dir);
    let noted = with_set_aside_note(refused(Refusal::Link, dir), note.to_string());
    assert_eq!(noted.kind(), refused_link.kind());
    assert_eq!(refusal_of(&noted), Some(Refusal::Link), "the refusal is still the refusal");
    assert_eq!(with_next_step(&noted), with_next_step(&refused_link), "the next step does not carry the note");
    assert_eq!(
        start_failure_message("Grok Build", &noted),
        format!("Couldn't start Grok Build: /data/sessions/p1-tab-1 is a link, not a folder. Delete the link, then try again. {note}")
    );

    let unwritable = cannot("moved", dir, io::Error::from(io::ErrorKind::PermissionDenied), Fix::MakeWritable);
    let noted = with_set_aside_note(cannot("moved", dir, io::Error::from(io::ErrorKind::PermissionDenied), Fix::MakeWritable), note.to_string());
    assert_eq!(with_next_step(&noted), with_next_step(&unwritable), "a fix is still found under the note");
    assert!(start_failure_message("Grok Build", &noted).ends_with(&format!("Make it writable or delete it, then try again. {note}")));

    assert!(!start_failure_message("Grok Build", &refused_link).contains("Moved"), "an error with no note says nothing about a move");
}
