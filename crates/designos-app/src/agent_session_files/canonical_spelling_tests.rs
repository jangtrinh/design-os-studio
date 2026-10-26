//! The sessions folder spelled through the links above it (`/private/var/...` for `/var/...` on macOS) is the same folder, so a
//! path spelled that way is still written below its session folder, and a link the agent planted in it is still refused.

use super::folder_tests::{only_entry_is_the_sentinel, target_with_sentinel};
use super::refusal::{refusal_of, Refusal};
use super::test_support::{made_session_folder, plant_link};
use super::*;

#[test]
fn a_session_path_spelled_through_the_links_above_its_folder_is_still_checked_below_the_session_folder() {
    let (target, _target) = target_with_sentinel("canonical-target");
    let (session, _session) = made_session_folder("canonical");
    let spelled = session.canonicalize().expect("the session folder, spelled without links");
    plant_link(&spelled, ".grok", &target);

    let result = atomic_write_session_file(&spelled.join(".grok").join("hooks").join("probe.json"), b"studio");

    let in_target: Vec<_> = std::fs::read_dir(&target).expect("the target").flatten().map(|e| e.file_name()).collect();
    assert!(only_entry_is_the_sentinel(&target), "the write went through the link into its target, which now holds {in_target:?}");
    let e = result.expect_err("a link in the path is refused whatever way the path is spelled");
    assert_eq!(refusal_of(&e), Some(Refusal::Link), "{e}");
}
