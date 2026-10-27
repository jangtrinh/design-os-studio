//! `session-options.json`, with a link planted at its name or at the name its write used to start from.
use super::scratch::{assert_untouched, old_temp_name, Scratch, VICTIM};
use designos_mcp::tools::session_options::SessionOptions;
use std::fs;

#[test]
fn a_save_never_writes_through_a_link_at_the_old_temp_name() {
    let scratch = Scratch::new();
    let victim = scratch.victim(VICTIM);
    scratch.link(&old_temp_name("session-options"), &victim);
    SessionOptions::default().save_to_dir(&scratch.session).unwrap();
    assert_untouched(&victim, VICTIM, "save");
}

#[test]
fn a_save_replaces_a_link_at_the_target_and_leaves_its_victim() {
    let scratch = Scratch::new();
    let victim = scratch.victim(VICTIM);
    let link = scratch.link("session-options.json", &victim);
    SessionOptions::default().save_to_dir(&scratch.session).unwrap();
    assert_untouched(&victim, VICTIM, "save");
    assert!(fs::symlink_metadata(&link).unwrap().file_type().is_file(), "the link is still there");
}
