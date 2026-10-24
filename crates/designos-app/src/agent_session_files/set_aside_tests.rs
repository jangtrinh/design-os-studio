//! Nothing Studio did not write is deleted. When an Ask-mode Grok or Antigravity tab starts, each entry of the sources the CLI would run
//! that is not Studio's is moved, whole, into a folder made for that start, `<session>/.designos-set-aside/<stamp>/`, under its path with
//! `/` written `__` (`.claude/settings.local.json` is `.claude__settings.local.json`), and the Spawned line says so.

use super::test_support::{aside_path, is_stamp, is_there, make_fifo, names_in, plant, plant_link, stamp_folders, tab_state_in, timed_with_fifo_watchdog};
use crate::terminal::{AgentCliPreset, PermissionMode};
use std::os::unix::fs::FileTypeExt;
use std::time::Duration;

const GROK: AgentCliPreset = AgentCliPreset::Grok;
const AGY: AgentCliPreset = AgentCliPreset::Antigravity;

#[test]
fn an_entry_is_moved_with_its_content_below_a_new_folder_under_a_name_made_of_its_path() {
    let mut st = tab_state_in(PermissionMode::Ask);
    plant(&st.session_dir, ".claude/settings.local.json", r#"{"permissions":{"allow":["Bash"]}}"#);
    plant(&st.session_dir, ".grok/hooks/evil.json", r#"{"hooks":{}}"#);
    plant(&st.session_dir, ".mcp.json", r#"{"mcpServers":{}}"#);

    st.spawn_tab(GROK, None).expect("the entries are moved aside, which makes the folder Studio's");

    let folders = stamp_folders(&st.session_dir);
    assert_eq!(folders.len(), 1, "one start, one folder: {folders:?}");
    assert!(is_stamp(&folders[0].file_name().expect("a name").to_string_lossy()), "{folders:?}");
    assert_eq!(names_in(&folders[0]), [".claude__settings.local.json", ".grok__hooks__evil.json", ".mcp.json"]);
    assert_eq!(std::fs::read_to_string(aside_path(&st.session_dir, ".claude__settings.local.json")).expect("kept"), r#"{"permissions":{"allow":["Bash"]}}"#);
    assert_eq!(std::fs::read_to_string(aside_path(&st.session_dir, ".grok__hooks__evil.json")).expect("kept"), r#"{"hooks":{}}"#);
    assert!(!is_there(&st.session_dir.join(".claude/settings.local.json")) && !is_there(&st.session_dir.join(".grok/hooks/evil.json")));
}

#[test]
fn each_start_makes_a_new_folder_and_the_earlier_ones_stay() {
    let mut st = tab_state_in(PermissionMode::Ask);
    plant(&st.session_dir, ".mcp.json", "first");
    st.spawn_tab(GROK, None).expect("the first start");
    plant(&st.session_dir, ".mcp.json", "second");
    st.spawn_tab(GROK, None).expect("the second start");

    let folders = stamp_folders(&st.session_dir);
    assert_eq!(folders.len(), 2, "{folders:?}");
    let mut contents: Vec<String> = folders.iter().map(|f| std::fs::read_to_string(f.join(".mcp.json")).expect("kept")).collect();
    contents.sort();
    assert_eq!(contents, ["first", "second"], "the second start did not replace what the first kept");
}

#[test]
fn a_folder_and_a_link_are_moved_whole_without_being_walked_or_followed() {
    let mut st = tab_state_in(PermissionMode::Ask);
    let victim = st.session_dir.join("victim");
    plant(&victim, "users-own-file.txt", "keep me");
    plant(&st.session_dir, ".grok/plugins/evil/plugin.json", "{}");
    plant_link(&st.session_dir, ".grok/plugins/evil/inner-link", &victim);
    plant_link(&st.session_dir, ".cursor/mcp.json", &victim.join("users-own-file.txt"));

    st.spawn_tab(GROK, None).expect("the folder and the link are moved aside");

    assert!(is_there(&aside_path(&st.session_dir, ".grok__plugins/evil/plugin.json")), "the folder is kept with what is in it");
    let inner = std::fs::symlink_metadata(aside_path(&st.session_dir, ".grok__plugins/evil/inner-link")).expect("the link inside, still a link");
    assert!(inner.file_type().is_symlink());
    let top = std::fs::symlink_metadata(aside_path(&st.session_dir, ".cursor__mcp.json")).expect("the link, moved");
    assert!(top.file_type().is_symlink());
    assert_eq!(names_in(&victim), ["users-own-file.txt"]);
    assert_eq!(std::fs::read_to_string(victim.join("users-own-file.txt")).expect("the victim"), "keep me");
}

#[test]
fn a_fifo_is_moved_without_being_opened() {
    let mut st = tab_state_in(PermissionMode::Ask);
    std::fs::create_dir_all(st.session_dir.join(".grok/hooks")).expect("the agent's folder");
    let fifo = st.session_dir.join(".grok/hooks/evil.json");
    make_fifo(&fifo);

    let (result, took) = timed_with_fifo_watchdog(&fifo, Duration::from_secs(30), || st.spawn_tab(GROK, None));

    result.expect("a FIFO is moved aside like any entry");
    assert!(took < Duration::from_secs(20), "the start waited {took:?} on the FIFO");
    let kept = std::fs::symlink_metadata(aside_path(&st.session_dir, ".grok__hooks__evil.json")).expect("the FIFO, moved");
    assert!(kept.file_type().is_fifo());
}

#[test]
fn a_link_or_a_file_where_the_set_aside_folder_goes_stops_the_tab() {
    for as_link in [true, false] {
        let mut st = tab_state_in(PermissionMode::Ask);
        let victim = st.session_dir.join("victim");
        plant(&victim, "users-own-file.txt", "keep me");
        plant(&st.session_dir, ".mcp.json", "{}");
        if as_link {
            plant_link(&st.session_dir, ".designos-set-aside", &victim);
        } else {
            plant(&st.session_dir, ".designos-set-aside", "a file");
        }

        let message = st.spawn_tab(GROK, None).expect_err("an entry cannot be moved aside into a link or a file");

        let aside = st.session_dir.join(".designos-set-aside").display().to_string();
        let expected = if as_link {
            format!("Couldn't start Grok Build: {aside} is a link, not a folder. Delete the link, then try again.")
        } else {
            format!("Couldn't start Grok Build: {aside} is a file, not a folder. Delete the file at that path, then try again.")
        };
        assert_eq!(message, expected);
        assert!(st.tabs.is_empty(), "no tab was added");
        assert!(is_there(&st.session_dir.join(".mcp.json")), "nothing was moved");
        assert_eq!(names_in(&victim), ["users-own-file.txt"], "nothing was moved through the link");
    }
}

#[test]
fn the_spawned_line_says_what_was_moved_and_where() {
    let mut st = tab_state_in(PermissionMode::Ask);
    let named = format!("({} · {})", st.doc_name, st.current_page_name);
    plant(&st.session_dir, ".mcp.json", "{}");
    st.spawn_tab(GROK, None).expect("the start");
    let stamp = stamp_folders(&st.session_dir)[0].file_name().expect("a name").to_string_lossy().into_owned();
    assert_eq!(
        st.status_msg,
        Some(format!("Spawned Grok Build {named}. Moved .mcp.json to .designos-set-aside/{stamp} in this file's agent folder, so Grok won't run it."))
    );

    let mut st = tab_state_in(PermissionMode::Ask);
    let named = format!("({} · {})", st.doc_name, st.current_page_name);
    plant(&st.session_dir, ".grok/hooks/evil.json", "{}");
    plant(&st.session_dir, ".mcp.json", "{}");
    plant(&st.session_dir, ".cursor/mcp.json", "{}");
    st.spawn_tab(GROK, None).expect("the start");
    let stamp = stamp_folders(&st.session_dir)[0].file_name().expect("a name").to_string_lossy().into_owned();
    assert_eq!(
        st.status_msg,
        Some(format!("Spawned Grok Build {named}. Moved .grok/hooks/evil.json and 2 more to .designos-set-aside/{stamp} in this file's agent folder, so Grok won't run them."))
    );

    let mut st = tab_state_in(PermissionMode::Ask);
    let named = format!("({} · {})", st.doc_name, st.current_page_name);
    plant(&st.session_dir, ".agents/mcp_config.json", "{}");
    st.spawn_tab(AGY, None).expect("the start");
    let stamp = stamp_folders(&st.session_dir)[0].file_name().expect("a name").to_string_lossy().into_owned();
    assert_eq!(
        st.status_msg,
        Some(format!("Spawned Antigravity (agy) {named}. Moved .agents/mcp_config.json to .designos-set-aside/{stamp} in this file's agent folder, so Antigravity won't run it."))
    );
}

#[test]
fn the_spawned_line_says_nothing_about_a_set_aside_when_nothing_was_moved() {
    let mut st = tab_state_in(PermissionMode::Ask);
    let expected = format!("Spawned Grok Build ({} · {})", st.doc_name, st.current_page_name);

    st.spawn_tab(GROK, None).expect("a clean folder");

    assert_eq!(st.status_msg, Some(expected));
}

#[test]
fn an_entry_the_agent_takes_away_before_it_is_moved_counts_as_gone_not_as_a_failure() {
    let mut st = tab_state_in(PermissionMode::Ask);
    plant(&st.session_dir, ".mcp.json", "{}");
    plant(&st.session_dir, ".cursor/mcp.json", "{}");
    let named = format!("({} · {})", st.doc_name, st.current_page_name);
    let (gone, to_remove) = (st.session_dir.join(".mcp.json"), st.session_dir.join(".mcp.json"));
    super::no_follow::test_hooks::set_before_move_hook(&gone, Box::new(move || std::fs::remove_file(&to_remove).expect("the agent removes it first")));

    st.spawn_tab(GROK, None).expect("an entry that is gone is already out of the way");

    let stamp = stamp_folders(&st.session_dir)[0].clone();
    assert_eq!(names_in(&stamp), [".cursor__mcp.json"], "only what was still there was moved");
    let name = stamp.file_name().expect("a name").to_string_lossy().into_owned();
    assert_eq!(
        st.status_msg,
        Some(format!("Spawned Grok Build {named}. Moved .cursor/mcp.json to .designos-set-aside/{name} in this file's agent folder, so Grok won't run it."))
    );
}

#[test]
fn a_fifo_where_the_set_aside_folder_goes_stops_the_tab_at_once_without_being_opened() {
    let mut st = tab_state_in(PermissionMode::Ask);
    plant(&st.session_dir, ".mcp.json", "{}");
    let fifo = st.session_dir.join(".designos-set-aside");
    make_fifo(&fifo);

    let (result, took) = timed_with_fifo_watchdog(&fifo, Duration::from_secs(30), || st.spawn_tab(GROK, None));

    let message = result.expect_err("an entry cannot be moved aside into a FIFO");
    assert!(took < Duration::from_secs(20), "the start waited {took:?} on the FIFO");
    assert_eq!(message, format!("Couldn't start Grok Build: {} is a file, not a folder. Delete the file at that path, then try again.", fifo.display()));
    assert!(st.tabs.is_empty(), "no tab was added");
    assert!(is_there(&st.session_dir.join(".mcp.json")), "nothing was moved");
}

#[test]
fn a_name_an_agent_takes_between_the_choice_of_the_name_and_the_rename_is_never_replaced() {
    let mut st = tab_state_in(PermissionMode::Ask);
    plant(&st.session_dir, ".mcp.json", "the entry Studio moves");
    let entry = st.session_dir.join(".mcp.json");
    super::no_follow::test_hooks::set_before_rename_hook(
        &entry,
        Box::new(|taken: &std::path::Path| std::fs::write(taken, "the file the racing agent put there").expect("the racing agent takes the name")),
    );

    st.spawn_tab(GROK, None).expect("the next name is used");

    let stamp = stamp_folders(&st.session_dir)[0].clone();
    assert_eq!(names_in(&stamp), [".mcp.json", ".mcp.json.2"]);
    assert_eq!(
        std::fs::read_to_string(stamp.join(".mcp.json")).expect("the racer's file"),
        "the file the racing agent put there",
        "the racer's file is still there"
    );
    assert_eq!(std::fs::read_to_string(stamp.join(".mcp.json.2")).expect("the moved entry"), "the entry Studio moves", "the entry is kept under the next name");
}

#[test]
fn a_stamp_folder_that_lost_its_write_bit_is_named_and_not_the_folder_the_entry_is_in() {
    if super::test_support::running_as_root() {
        return;
    }
    use std::os::unix::fs::PermissionsExt;
    let mut st = tab_state_in(PermissionMode::Ask);
    plant(&st.session_dir, ".mcp.json", "{}");
    let (entry, session) = (st.session_dir.join(".mcp.json"), st.session_dir.clone());
    super::no_follow::test_hooks::set_before_move_hook(
        &entry,
        Box::new(move || {
            let stamp = stamp_folders(&session)[0].clone();
            std::fs::set_permissions(&stamp, std::fs::Permissions::from_mode(0o555)).expect("the agent makes the new folder read-only");
        }),
    );

    let message = st.spawn_tab(GROK, None).expect_err("an entry cannot be moved into a read-only folder");

    let stamp = stamp_folders(&st.session_dir)[0].clone();
    let _writable = super::test_support::Writable(stamp.clone());
    assert_eq!(
        message,
        format!(
            "Couldn't start Grok Build: {} can't be written to: Permission denied (os error 13). Make it writable or delete it, then try again.",
            stamp.display()
        )
    );
    assert!(is_there(&entry), "nothing was moved");
}
