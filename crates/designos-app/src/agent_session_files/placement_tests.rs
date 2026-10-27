//! Where a path lies decides what is checked below it. A path in no session folder is refused by a real build, and the session
//! folder of a path spelled through links is found.

use super::folder_tests::{scratch_dir, Cleanup};
use super::session_io::place_of_with;
use super::test_support::made_session_folder;
use super::*;

#[test]
fn a_path_in_no_session_folder_is_refused_by_name_unless_a_test_build_allows_it() {
    let scratch = scratch_dir("placement");
    let _scratch = Cleanup(vec![scratch.clone()]);
    let path = scratch.join("hooks").join("probe.json");

    let e = place_of_with(&path, false).err().expect("a real build refuses a path in no session folder");

    assert_eq!(
        with_next_step(&e),
        format!("{} is outside this file's agent folder, so Studio doesn't use it. Reopen this design file, then try again", path.display())
    );
    assert_eq!(place_of_with(&path, true).expect("a test build anchors it at its own folder").anchor, scratch.join("hooks"));
}

#[test]
fn a_session_path_is_found_by_either_spelling_of_the_sessions_folder() {
    let (session, _session) = made_session_folder("placement-spelling");
    let spelled = session.canonicalize().expect("the session folder, spelled without links");
    for folder in [&session, &spelled] {
        let place = place_of_with(&folder.join(".agents").join("hooks.json"), false).expect("a session path");
        assert_eq!((place.anchor.as_path(), place.folders.len()), (folder.as_path(), 1), "{}", folder.display());
    }
}
