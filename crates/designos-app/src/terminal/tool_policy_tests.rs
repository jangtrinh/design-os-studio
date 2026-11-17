//! The user's `designos` tool list is a permission, so an agent that can write its session folder must not be able to widen it.
//! The session folder holds a copy of the list for the CLIs to discover, but the file the `designos-mcp` server reads
//! (`DESIGNOS_MCP_CONFIG`), the rules the agent is shown and the bootstrap block all come from what Studio kept outside that folder.
//! An agent that rewrites the copy, deletes it or forges a `known_tools` snapshot changes none of them, for the next sync and for the
//! next launch alike.

use super::*;
use crate::agent_permissions::AgentPermissionMode;
use designos_mcp::rules::EnabledToolsConfig;
use serde_json::{json, Value};
use std::collections::BTreeSet;

const RESTRICTED: [&str; 2] = ["designos_get_document", "designos_list_nodes"];

fn all_tools() -> Vec<String> {
    designos_mcp::server::tools().into_iter().map(|t| t.name.to_string()).collect()
}

/// A window on a cleared session folder, in `mode`, with nothing started yet.
fn fresh(mode: AgentPermissionMode) -> TerminalState {
    let mut st = TerminalState::for_tab(crate::live_sync_tests::unique_tab_id());
    crate::agent_tool_policy::remove(&st.session_dir);
    let _ = std::fs::remove_dir_all(&st.session_dir);
    std::fs::create_dir_all(&st.session_dir).expect("session folder");
    st.permission_mode = mode;
    st
}

/// Removes everything a test made: the session folder, the approval channel and the folder Studio keeps the policy in.
fn finish(mut st: TerminalState) {
    st.stop_bootstrap_writes();
    crate::agent_tool_policy::remove(&st.session_dir);
    crate::agent_approval_channel::remove_channel(&st.session_dir);
    let _ = std::fs::remove_dir_all(&st.session_dir);
}

fn session_config(st: &TerminalState) -> PathBuf {
    st.session_dir.join("mcp-config.json")
}

fn read_json(path: &Path) -> Value {
    serde_json::from_str(&std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))).expect("JSON")
}

/// What the user does by hand: the list in the session's `mcp-config.json`, before any agent runs.
fn user_restricts(st: &TerminalState, enabled: &[&str], known: Option<Vec<String>>) {
    let mut block = json!({ "enabled_tools": enabled });
    if let Some(known) = known {
        block["known_tools"] = json!(known);
    }
    std::fs::write(session_config(st), serde_json::to_vec_pretty(&json!({ "mcpServers": { "designos": { "env": {} } }, "designos": block })).unwrap())
        .expect("the user's list");
}

fn launch(st: &mut TerminalState) {
    assert!(st.provision_session_agent_files(None).is_none(), "the agent files are written");
}

/// The tools the `designos-mcp` server of this session would enable, read from the file its `DESIGNOS_MCP_CONFIG` names.
fn tools_the_server_enables(st: &TerminalState) -> BTreeSet<String> {
    let named = st.session_env().mcp_config_path;
    let config = EnabledToolsConfig::from_mcp_config_json(&std::fs::read_to_string(&named).expect("the server's config"));
    all_tools().into_iter().filter(|name| config.is_tool_enabled(name)).collect()
}

/// The file the server reads is one Studio wrote outside the session folder, and the server entry names that same file.
fn assert_server_config_is_out_of_reach(st: &TerminalState, when: &str) {
    let named = st.session_env().mcp_config_path;
    assert!(!named.starts_with(&st.session_dir), "{when}: the server reads its tool list from {}, a file the agent can write", named.display());
    let in_entry = read_json(&session_config(st))["mcpServers"]["designos"]["env"]["DESIGNOS_MCP_CONFIG"].as_str().map(PathBuf::from);
    assert_eq!(in_entry.as_deref(), Some(named.as_path()), "{when}: the server entry and the tab's environment name one file");
}

fn names(list: &[&str]) -> BTreeSet<String> {
    list.iter().map(ToString::to_string).collect()
}

/// Every place the tool list is read from stays on `expected`.
fn assert_tools_stay(st: &mut TerminalState, expected: &BTreeSet<String>, doc: &Document, page: NodeId, when: &str) {
    assert_eq!(&tools_the_server_enables(st), expected, "{when}: the tools the server enables");
    let skill = std::fs::read_to_string(st.session_dir.join("SKILL.md")).expect("SKILL.md");
    assert_eq!(skill.contains("### `designos_create_node`"), expected.contains("designos_create_node"), "{when}: the rules the agent is shown");
    let copy = EnabledToolsConfig::from_mcp_config_json(&std::fs::read_to_string(session_config(st)).expect("the session copy"));
    let in_copy: BTreeSet<String> = all_tools().into_iter().filter(|name| copy.is_tool_enabled(name)).collect();
    assert_eq!(&in_copy, expected, "{when}: the session folder's own copy");
    st.start_bootstrap_job(doc.clone_without_images(), page, None, None);
    let job = st.bootstrap_job.as_ref().expect("a bootstrap job");
    let in_bootstrap = job.enabled_tools.clone().unwrap_or_else(|| all_tools().into_iter().collect());
    assert_eq!(&in_bootstrap, expected, "{when}: the tools the bootstrap block documents");
}

fn one_page() -> (Document, NodeId) {
    let mut doc = Document::new("Tool policy");
    let page = doc.add_page("Page 1");
    (doc, page)
}

/// One way an agent edits the copy in its folder, and the name it is told by.
type Forge = Box<dyn Fn(&mut Value)>;

fn forgeries() -> Vec<(&'static str, Forge)> {
    vec![
        ("a full tool list", Box::new(|cfg: &mut Value| cfg["designos"] = json!({ "enabled_tools": all_tools() }))),
        (
            "the designos key deleted",
            Box::new(|cfg: &mut Value| {
                cfg.as_object_mut().unwrap().remove("designos");
            }),
        ),
        (
            "an empty known_tools snapshot, which makes every tool look new",
            Box::new(|cfg: &mut Value| cfg["designos"] = json!({ "enabled_tools": [], "known_tools": [] })),
        ),
    ]
}

#[test]
fn an_agent_edit_of_the_session_config_cannot_widen_the_tool_list() {
    for mode in [AgentPermissionMode::Skip, AgentPermissionMode::Ask] {
        for (what, forge) in forgeries() {
            let mut st = fresh(mode);
            let (doc, page) = one_page();
            user_restricts(&st, &RESTRICTED, None);
            launch(&mut st);
            let expected = names(&RESTRICTED);
            assert_tools_stay(&mut st, &expected, &doc, page, &format!("{mode:?}, before the edit"));

            // The agent runs and rewrites the copy in its folder.
            let mut cfg = read_json(&session_config(&st));
            forge(&mut cfg);
            std::fs::write(session_config(&st), serde_json::to_vec_pretty(&cfg).unwrap()).expect("the agent's write");

            // The next sync of the page, then the next launch of an agent.
            st.sync_live_file_context(&doc, page, &[], None);
            st.flush_live_sync();
            assert_tools_stay(&mut st, &expected, &doc, page, &format!("{mode:?}, after {what} and a sync"));
            launch(&mut st);
            assert_tools_stay(&mut st, &expected, &doc, page, &format!("{mode:?}, after {what} and a new launch"));
            finish(st);
        }
    }
}

/// The edit tests above prove the outcome; this one pins where the server's list lives, in every permission mode.
#[test]
fn the_server_reads_its_list_from_a_file_outside_the_session_folder_in_every_permission_mode() {
    for mode in [AgentPermissionMode::Skip, AgentPermissionMode::Ask, AgentPermissionMode::AskRisky] {
        let mut st = fresh(mode);
        launch(&mut st);
        assert_server_config_is_out_of_reach(&st, &format!("{mode:?}"));
        finish(st);
    }
}

#[test]
fn a_list_written_before_the_agent_starts_is_obeyed_in_every_permission_mode() {
    for mode in [AgentPermissionMode::Skip, AgentPermissionMode::Ask, AgentPermissionMode::AskRisky] {
        let mut st = fresh(mode);
        let (doc, page) = one_page();
        user_restricts(&st, &RESTRICTED, None);
        launch(&mut st);
        assert_tools_stay(&mut st, &names(&RESTRICTED), &doc, page, &format!("{mode:?}"));
        if mode.installs_approval_hook() {
            // The MCP config Claude starts its servers from in these modes carries the same list.
            let private = crate::agent_approval_channel::mcp_config_path(&st.session_dir).expect("a private channel");
            let config = EnabledToolsConfig::from_mcp_config_json(&std::fs::read_to_string(private).expect("Claude's MCP config"));
            assert!(config.is_tool_enabled("designos_get_document") && !config.is_tool_enabled("designos_create_node"), "{mode:?}");
        }
        assert_server_config_is_out_of_reach(&st, &format!("{mode:?}"));
        finish(st);
    }
}

#[test]
fn a_session_from_an_older_build_keeps_its_list_and_cannot_be_widened_after_the_update() {
    let mut st = fresh(AgentPermissionMode::Skip);
    let (doc, page) = one_page();
    // The folder an older build left: a restricted list written against a tool set without `designos_lint`, and nothing of Studio's elsewhere.
    let older: Vec<String> = all_tools().into_iter().filter(|name| name != "designos_lint").collect();
    user_restricts(&st, &["designos_get_document"], Some(older));
    launch(&mut st);
    // The restriction survives the update, and the tool that did not exist when the list was written is on, as it was before.
    let expected = names(&["designos_get_document", "designos_lint"]);
    assert_tools_stay(&mut st, &expected, &doc, page, "the first launch after the update");

    let mut cfg = read_json(&session_config(&st));
    cfg["designos"] = json!({ "enabled_tools": all_tools() });
    std::fs::write(session_config(&st), serde_json::to_vec_pretty(&cfg).unwrap()).expect("the agent's write");
    launch(&mut st);
    assert_tools_stay(&mut st, &expected, &doc, page, "the next launch");
    finish(st);
}

/// The folder's own copy as the user or an agent edits it by hand.
fn edit_copy(st: &TerminalState, edit: impl FnOnce(&mut Value)) {
    let mut cfg = read_json(&session_config(st));
    edit(&mut cfg);
    std::fs::write(session_config(st), serde_json::to_vec_pretty(&cfg).unwrap()).expect("a hand edit");
}

/// The tools the folder's own copy enables.
fn tools_in_the_copy(st: &TerminalState) -> BTreeSet<String> {
    let config = EnabledToolsConfig::from_mcp_config_json(&std::fs::read_to_string(session_config(st)).expect("the session copy"));
    all_tools().into_iter().filter(|name| config.is_tool_enabled(name)).collect()
}

fn sync(st: &mut TerminalState, doc: &Document, page: NodeId) {
    st.sync_live_file_context(doc, page, &[], None);
    st.flush_live_sync();
}

/// A session that has started on the default can still be narrowed by hand, and an agent that narrows its own copy only harms itself:
/// the narrower list is honoured and kept, and nobody can put the tools back in the folder's copy.
#[test]
fn a_session_on_the_default_honours_a_narrowing_and_keeps_it() {
    let mut st = fresh(AgentPermissionMode::Skip);
    let (doc, page) = one_page();
    launch(&mut st);
    let every: BTreeSet<String> = all_tools().into_iter().collect();
    assert_tools_stay(&mut st, &every, &doc, page, "a default session");
    assert!(read_json(&session_config(&st)).get("designos").is_none(), "the default is never written down");

    edit_copy(&st, |cfg| cfg["designos"] = json!({ "enabled_tools": ["designos_get_document"] }));
    launch(&mut st);
    let narrowed = names(&["designos_get_document"]);
    assert_tools_stay(&mut st, &narrowed, &doc, page, "after the copy was narrowed");

    // The copy is put back to every tool, three ways: the narrowing was kept.
    for (what, forge) in forgeries() {
        edit_copy(&st, |cfg| forge(cfg));
        sync(&mut st, &doc, page);
        assert_tools_stay(&mut st, &narrowed, &doc, page, &format!("after {what} and a sync"));
        launch(&mut st);
        assert_tools_stay(&mut st, &narrowed, &doc, page, &format!("after {what} and a new launch"));
    }
    finish(st);
}

/// By-hand edits are the only way the user sets the list, so one made after an agent has started counts at once, in every start.
#[test]
fn a_tool_the_user_takes_out_after_the_first_start_is_honoured_and_cannot_be_put_back() {
    for mode in [AgentPermissionMode::Skip, AgentPermissionMode::Ask] {
        let mut st = fresh(mode);
        let (doc, page) = one_page();
        user_restricts(&st, &RESTRICTED, None);
        launch(&mut st);
        let two = names(&RESTRICTED);
        assert_tools_stay(&mut st, &two, &doc, page, &format!("{mode:?}, the first start"));

        // The user takes one more tool out; the next sync honours it.
        edit_copy(&st, |cfg| cfg["designos"] = json!({ "enabled_tools": ["designos_get_document"] }));
        sync(&mut st, &doc, page);
        let one = names(&["designos_get_document"]);
        assert_tools_stay(&mut st, &one, &doc, page, &format!("{mode:?}, after the user narrowed it and a sync"));

        // An agent that puts the tools back, or takes the key out, changes nothing, for the next sync or the next start.
        for (what, forge) in forgeries() {
            edit_copy(&st, |cfg| forge(cfg));
            launch(&mut st);
            assert_tools_stay(&mut st, &one, &doc, page, &format!("{mode:?}, after {what} and a new launch"));
        }
        finish(st);
    }
}

/// An edit that puts tools back is ignored for those tools, the kept list stays as it was, and the user is told how the list changes.
#[test]
fn tools_put_back_after_the_first_start_stay_off_and_the_user_is_told() {
    let mut st = fresh(AgentPermissionMode::Skip);
    let (doc, page) = one_page();
    user_restricts(&st, &RESTRICTED, None);
    launch(&mut st);
    assert!(st.set_aside_note.is_none(), "nothing to say at the first start: {:?}", st.set_aside_note);
    let two = names(&RESTRICTED);

    edit_copy(&st, |cfg| cfg["designos"] = json!({ "enabled_tools": all_tools() }));
    launch(&mut st);
    let note = st.set_aside_note.clone().expect("a sentence for the start line");
    assert!(note.contains("can only be narrowed"), "{note}");
    assert!(note.contains("delete the folder"), "the reset is named: {note}");
    assert_tools_stay(&mut st, &two, &doc, page, "after the user tried to widen it");

    // The copy was written back as the list in force, so the next start has nothing more to say.
    launch(&mut st);
    assert!(st.set_aside_note.is_none(), "{:?}", st.set_aside_note);
    finish(st);
}

/// A sync writes the list in force into the user's file, never a wider one than the user's own.
#[test]
fn a_sync_never_writes_a_wider_list_over_the_users_file() {
    let mut st = fresh(AgentPermissionMode::Skip);
    let (mut doc, page) = one_page();
    user_restricts(&st, &RESTRICTED, None);
    launch(&mut st);

    edit_copy(&st, |cfg| cfg["designos"] = json!({ "enabled_tools": ["designos_get_document"] }));
    sync(&mut st, &doc, page);
    assert_eq!(tools_in_the_copy(&st), names(&["designos_get_document"]), "the user's narrower list is what stays in the file");

    // A wider file is not the user's list any more: what is written back is what is in force, which is inside the file's own list.
    let wide: BTreeSet<String> = all_tools().into_iter().collect();
    edit_copy(&st, |cfg| cfg["designos"] = json!({ "enabled_tools": wide }));
    // The document changed, so this sync writes the files again.
    doc.add_page("Page 2");
    sync(&mut st, &doc, page);
    let written = tools_in_the_copy(&st);
    assert!(written.is_subset(&wide) && written == names(&["designos_get_document"]), "{written:?}");
    finish(st);
}

/// A scratch folder that is deleted takes the list kept for it, so a later session on the same path starts from its own file.
#[test]
fn the_kept_list_goes_with_the_scratch_folder_when_a_saved_library_file_is_bound() {
    let mut st = fresh(AgentPermissionMode::Skip);
    user_restricts(&st, &RESTRICTED, None);
    launch(&mut st);
    let scratch = st.session_dir.clone();
    let kept = crate::agent_tool_policy::policy_dir(&scratch).expect("a policy folder");
    assert!(kept.exists());

    let lib = std::env::temp_dir().join(format!("designos-tool-policy-lib-{}", crate::live_sync_tests::unique_tab_id()));
    let _ = std::fs::remove_dir_all(&lib);
    std::fs::create_dir_all(&lib).expect("a library folder");
    // Binding a saved file keeps running tabs and does not shut the state down, so the folder is deleted by the bind itself.
    st.bind_library_file_inner(&lib, "file-x", "win-x", crate::agent_sessions::is_pid_alive, true);
    assert_ne!(st.session_dir, scratch, "the session moved to the library's folder");
    assert!(!scratch.exists(), "the scratch folder is gone");
    assert!(!kept.exists(), "so is the list kept for it");
    finish(st);
    let _ = std::fs::remove_dir_all(&lib);
}

/// `designos-mcp` enables every tool when its config is missing, so a start that cannot keep a restricted list private does not happen,
/// whichever way it is made and whichever permission mode is saved.
#[test]
fn no_agent_starts_when_the_tool_list_cannot_be_kept_private() {
    let mut st = fresh(AgentPermissionMode::Skip);
    user_restricts(&st, &RESTRICTED, None);
    crate::agent_approval_channel::without_private_root(|| {
        for preset in [AgentCliPreset::Shell, AgentCliPreset::Claude, AgentCliPreset::Codex] {
            let refused = st.spawn_tab(preset, None).expect_err("a restricted list has nowhere private to be kept");
            assert!(refused.contains("designos tool list"), "{preset:?}: {refused}");
            assert_eq!(st.status_msg.as_deref(), Some(refused.as_str()));
        }
        let custom = st.spawn_custom_cli_tab("Mine", "true", None).expect_err("a custom CLI is refused too");
        assert!(custom.contains("designos tool list"), "{custom}");
        assert!(st.tabs.is_empty(), "nothing started");
    });
    finish(st);
}

#[test]
fn no_agent_starts_when_the_kept_tool_list_is_damaged() {
    let mut st = fresh(AgentPermissionMode::Skip);
    user_restricts(&st, &RESTRICTED, None);
    launch(&mut st);
    let kept = crate::agent_tool_policy::policy_dir(&st.session_dir).expect("a policy folder").join("tool-policy.json");
    std::fs::write(&kept, "{\"designos\":{\"enabled_tools\":\"all\"}}").expect("a damaged policy");
    // The folder's own copy says every tool, and is not read in its place.
    std::fs::write(session_config(&st), "{}").expect("the agent's write");
    for preset in [AgentCliPreset::Shell, AgentCliPreset::Claude] {
        let refused = st.spawn_tab(preset, None).expect_err("the kept list cannot be read");
        assert!(refused.contains("designos tool list"), "{preset:?}: {refused}");
        // It says what to do: the folder to delete, as a path, with no hash to compute.
        let folder = crate::agent_tool_policy::policy_dir(&st.session_dir).expect("a policy folder");
        assert!(refused.contains("delete the folder") && refused.contains(&folder.display().to_string()), "{preset:?}: {refused}");
    }
    assert!(st.tabs.is_empty());
    finish(st);
}
