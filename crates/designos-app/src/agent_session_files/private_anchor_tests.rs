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

/// The names Studio keeps in the folder of an approval channel besides the hook folder: the hook a CLI runs, the files it reads to
/// find that hook and the MCP server, the plugin it loads, Gemini's hooks file of a tab, and the folders of a tab's answers.
const CHANNEL_NAMES: [&str; 8] = [
    "approve-hook.sh",
    "hooks.json",
    "mcp-config.json",
    "claude-plugin/plugin.json",
    "claude-plugin/skills/x/SKILL.md",
    "tab-1/1.answer",
    "tab-1",
    "gemini-tab-1.json",
];

#[test]
fn nothing_of_the_approval_channel_but_the_hook_folder_is_a_place_for_a_routed_write() {
    let (session, _guard) = made_session_folder("channel-not-routed");
    let channel = crate::agent_approval_channel::channel_dir(&session).expect("a private channel");
    for name in CHANNEL_NAMES {
        let path = channel.join(name);
        assert_eq!(session_folder_of(&path), None, "{name} anchors at the channel");
        assert!(place_of_with(&path, false).is_err(), "{name} is a place for a routed helper");
        // Test builds let a scratch folder anchor at itself; a file of the approval channel is never that.
        assert!(crate::agent_session_files::atomic_write_session_file(&path, b"x").is_err(), "{name} was written by a routed helper");
        assert!(!path.exists(), "{name} is there");
    }
}

#[test]
fn the_hook_folder_itself_and_everything_below_it_still_anchor_at_the_channel() {
    let (session, _guard) = made_session_folder("hook-folder-anchors");
    let channel = crate::agent_approval_channel::channel_dir(&session).expect("a private channel");
    let dir = crate::agent_hook_outputs::DIR_NAME;
    for name in [dir.to_string(), format!("{dir}/agent-events.jsonl"), format!("{dir}/agent-events.jsonl.lock/x"), format!("{dir}/deeper/still")] {
        assert_eq!(session_folder_of(&channel.join(&name)), Some(channel.clone()), "{name}");
        assert!(place_of_with(&channel.join(&name), false).is_ok(), "{name}");
    }
}

#[test]
fn a_missing_channel_folder_is_made_private_by_a_routed_write_into_the_hook_folder() {
    use std::os::unix::fs::PermissionsExt;
    let (session, _guard) = made_session_folder("channel-recreated");
    let paths = crate::agent_hook_outputs::HookPaths::locate(&session);
    let channel = crate::agent_approval_channel::channel_dir(&session).expect("a private channel");
    crate::agent_approval_channel::remove_channel(&session);
    assert!(!channel.exists(), "the channel is gone");

    crate::agent_session_files::atomic_write_session_file(&paths.status(), b"{}").expect("a routed write makes the folders it needs");

    let mode = |path: &std::path::Path| std::fs::metadata(path).expect("a folder").permissions().mode() & 0o777;
    assert_eq!(mode(&channel), 0o700, "the channel folder of the session is owner-only, not what a plain create_dir_all makes");
    assert_eq!(mode(paths.dir()), 0o700, "so is the hook folder");
    crate::agent_approval_channel::remove_channel(&session);
}
