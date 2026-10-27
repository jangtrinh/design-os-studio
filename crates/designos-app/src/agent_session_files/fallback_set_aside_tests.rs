//! Where Studio moves nothing (a platform without `openat`; the fallback module is mounted here under unix as `fallback_under_test`),
//! an entry that is not Studio's stops the tab: the fallback checks a folder and then acts on its path, and a folder an agent swaps for a
//! link in between would carry a move or a removal into a folder that is not the session's. So it only reads, and refuses.

use super::fallback_under_test::Dir;
use super::test_support::{is_there, made_session_folder, names_in, plant, plant_link};
use super::{set_aside_entries_except, set_aside_path, start_failure_message, SetAside};
use std::ffi::OsStr;

#[test]
fn an_entry_that_is_not_studios_is_refused_where_studio_moves_nothing_and_nothing_is_touched() {
    let (session, _guard) = made_session_folder("fallback-refuses");
    plant(&session, ".grok/hooks/designos-status.json", "{}");
    plant(&session, ".grok/hooks/evil.json", "{}");
    plant(&session, ".mcp.json", "{}");
    let mut aside = SetAside::new(&session, "Grok").refusing_moves();

    let in_folder = set_aside_entries_except(&session.join(".grok/hooks"), &["designos-status.json"], &mut aside).expect_err("evil.json is not Studio's");
    let at_name = set_aside_path(&session.join(".mcp.json"), &mut aside).expect_err(".mcp.json is not Studio's");

    for (e, path) in [(in_folder, session.join(".grok/hooks/evil.json")), (at_name, session.join(".mcp.json"))] {
        assert_eq!(
            start_failure_message("Grok Build", &e),
            format!("Couldn't start Grok Build: {} is not Studio's, and Grok would run it. Move or delete it, then try again.", path.display())
        );
    }
    assert_eq!(names_in(&session.join(".grok/hooks")), ["designos-status.json", "evil.json"], "nothing was moved or removed");
    assert!(is_there(&session.join(".mcp.json")) && !is_there(&session.join(".designos-set-aside")));
    assert!(aside.moved().is_empty() && aside.note().is_none());
}

#[test]
fn where_studio_moves_nothing_a_folder_with_only_studios_entries_is_fine() {
    let (session, _guard) = made_session_folder("fallback-clean");
    plant(&session, ".grok/hooks/designos-status.json", "{}");
    let mut aside = SetAside::new(&session, "Grok").refusing_moves();

    set_aside_entries_except(&session.join(".grok/hooks"), &["designos-status.json"], &mut aside).expect("nothing that is not Studio's");
    set_aside_path(&session.join(".mcp.json"), &mut aside).expect("nothing there");
    set_aside_entries_except(&session.join(".cursor"), &[], &mut aside).expect("no such folder");
}

#[test]
fn the_fallback_only_lists_and_a_folder_swapped_for_a_link_loses_nothing_that_is_not_studios_to_remove() {
    let (session, _guard) = made_session_folder("fallback-swap");
    plant(&session, ".grok/hooks/designos-status.json", "{}");
    plant(&session, ".grok/hooks/evil.json", "{}");
    let victim = session.join("users-own-folder");
    plant(&victim, "users-own-file.txt", "keep me");
    let hooks = Dir::open_session_folder(&session)
        .and_then(|d| d.open_child(OsStr::new(".grok"), false))
        .and_then(|d| d.open_child(OsStr::new("hooks"), false))
        .expect("the check passes: .grok/hooks is a real folder");
    assert_eq!(hooks.entries_not_in(&["designos-status.json"]).expect("listed"), [OsStr::new("evil.json")]);
    assert!(hooks.has_entry(OsStr::new("evil.json")).expect("checked"));

    // The agent swaps the checked folder for a link (a junction on Windows).
    std::fs::rename(session.join(".grok/hooks"), session.join(".grok/hooks.real")).expect("move");
    plant_link(&session, ".grok/hooks", &victim);
    let listed = hooks.entries_not_in(&["designos-status.json"]).expect("listed through the link");

    assert_eq!(listed, [OsStr::new("users-own-file.txt")], "a listing reads what the link points at, and that is all it does");
    assert_eq!(names_in(&victim), ["users-own-file.txt"], "nothing was removed from it");
}
