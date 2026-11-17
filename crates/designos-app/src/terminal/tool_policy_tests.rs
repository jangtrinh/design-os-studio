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
    let _ = std::fs::remove_dir_all(&st.session_dir);
    std::fs::create_dir_all(&st.session_dir).expect("session folder");
    st.permission_mode = mode;
    st
}

/// Removes everything a test made: the session folder, the approval channel and the folder Studio keeps the policy in.
fn finish(mut st: TerminalState) {
    st.stop_bootstrap_writes();
    let kept = st.session_env().mcp_config_path;
    if !kept.starts_with(&st.session_dir) {
        if let Some(dir) = kept.parent() {
            let _ = std::fs::remove_dir_all(dir);
        }
    }
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
fn forgeries() -> Vec<(&'static str, Box<dyn Fn(&mut Value)>)> {
    vec![
        ("a full tool list", Box::new(|cfg: &mut Value| cfg["designos"] = json!({ "enabled_tools": all_tools() }))),
        ("the designos key deleted", Box::new(|cfg: &mut Value| {
            cfg.as_object_mut().unwrap().remove("designos");
        })),
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

#[test]
fn a_session_on_the_default_stays_on_every_tool_whatever_the_agent_writes() {
    let mut st = fresh(AgentPermissionMode::Skip);
    let (doc, page) = one_page();
    launch(&mut st);
    let every: BTreeSet<String> = all_tools().into_iter().collect();
    assert_tools_stay(&mut st, &every, &doc, page, "a default session");
    assert!(read_json(&session_config(&st)).get("designos").is_none(), "the default is never written down");

    // An agent that narrows its own list in the copy is not what the user asked for either: the list that counts is the one it started with.
    let mut cfg = read_json(&session_config(&st));
    cfg["designos"] = json!({ "enabled_tools": ["designos_get_document"] });
    std::fs::write(session_config(&st), serde_json::to_vec_pretty(&cfg).unwrap()).expect("the agent's write");
    launch(&mut st);
    assert_tools_stay(&mut st, &every, &doc, page, "after the agent narrowed the copy");
    finish(st);
}
