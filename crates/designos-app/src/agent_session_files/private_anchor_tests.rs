//! The hook folder lies in Studio's private channel root, where no agent grant reaches. The routed helpers anchor a path there the way
//! they anchor one in a session folder: from the open handle of `<root>/<sid>`, never through a link. Unit tests may anchor a path in no
//! known folder at its own parent, which would hide a missing anchor, so these ask the strict question.

use super::folder::session_folder_of;
use super::session_io::place_of_with;
use super::test_support::made_session_folder;

#[test]
fn a_path_in_the_hook_folder_anchors_at_the_channel_folder_of_its_session() {
    let (session, _guard) = made_session_folder("private-anchor");
    let channel = crate::agent_approval_channel::channel_dir(&session).expect("a private channel");
    let paths = crate::agent_hook_outputs::HookPaths::locate(&session);

    assert_eq!(session_folder_of(&paths.status()), Some(channel.clone()));
    let place = place_of_with(&paths.events(), false).expect("a path in the hook folder is anchored");
    assert_eq!(place.anchor, channel);
    assert_eq!(place.folders, [std::ffi::OsString::from(crate::agent_hook_outputs::DIR_NAME)]);
    assert_eq!(place.name, std::ffi::OsString::from(crate::agent_hook_outputs::EVENTS_FILE_NAME));
}

#[test]
fn a_path_beside_the_private_root_is_no_anchor() {
    let root = crate::agent_approval_channel::private_root().expect("a private root");
    let beside = root.parent().expect("a parent").join("elsewhere").join("agent-status.json");

    assert_eq!(session_folder_of(&beside), None);
    assert!(place_of_with(&beside, false).is_err(), "a file in a folder nothing anchors is refused");
}
