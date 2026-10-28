//! The root of the session paths used when Studio has no private data folder. It must never name a place relative to Studio's
//! working folder: a relative one would make the close of a tab delete `<working folder>/<root>/<tab>`.

use super::folder::{create_session_folder_in, UNAVAILABLE_ROOT};
use super::{atomic_write_session_file, ephemeral_session_dir, with_next_step};
use std::path::Path;

#[test]
fn the_unavailable_root_is_absolute_so_nothing_below_it_resolves_against_the_working_folder() {
    let root = Path::new(UNAVAILABLE_ROOT);
    assert!(root.is_absolute(), "{UNAVAILABLE_ROOT} would resolve against the working folder of the process");
    assert!(root.join("p1-tab-1").is_absolute());
    assert!(!std::env::current_dir().is_ok_and(|cwd| root.join("p1-tab-1").starts_with(cwd)), "a path below the root lies below the working folder");
}

#[test]
fn nothing_is_made_or_written_below_the_unavailable_root() {
    let session = Path::new(UNAVAILABLE_ROOT).join("p1-tab-1");
    let err = create_session_folder_in(None, &session).expect_err("no data folder, no session folder");
    assert!(with_next_step(&err).ends_with("Make sure your home folder exists, then restart Studio"), "{err}");
    let err = atomic_write_session_file(&session.join("context.json"), b"{}").expect_err("no data folder, no session file");
    assert!(with_next_step(&err).ends_with("Make sure your home folder exists, then restart Studio"), "{err}");
    assert!(!session.exists());
    assert!(ephemeral_session_dir("p1-tab-1").is_absolute(), "the session folder of an id is an absolute path whatever the platform has");
}
