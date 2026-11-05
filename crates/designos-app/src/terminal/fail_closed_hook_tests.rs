//! A hook that could not be given a folder to report to is not installed. Without a private data folder, or when the hook folder
//! cannot be made, no CLI gets a hook that writes or reads a file, in Ask or in Skip mode, and none falls back to the session folder,
//! which an agent can write. The only hook that may stay is Claude's approval handler: it reports to the approval channel folder.

use super::hook_output_tests::{commands_in, run_hook_with};
use super::*;
use crate::agent_approval_channel::{hooks_file_path, prepare_channel_dir, without_private_root};
use crate::agent_permissions::AgentPermissionMode;
use crate::agent_session_files::test_support::tab_state_in;
use std::collections::BTreeMap;

/// Every hook command Studio installed in the session folder `session` for a CLI that reads its hooks there, with the file it is in.
/// (Claude's hooks of an Ask tab are in the approval channel and are read by `channel_hooks`.)
fn installed_in_session(session: &Path) -> Vec<(String, String)> {
    let mut found = Vec::new();
    for rel in ["hooks.json", ".grok/hooks/designos-status.json", ".agents/hooks.json", ".gemini/settings.json"] {
        let Ok(text) = std::fs::read_to_string(session.join(rel)) else { continue };
        let value: serde_json::Value = serde_json::from_str(&text).unwrap_or_else(|e| panic!("{rel}: {e}"));
        // Gemini's settings also name the MCP server's command: only what sits under `hooks` is a hook.
        let hooks = if rel == ".gemini/settings.json" { value.get("hooks").cloned().unwrap_or(serde_json::Value::Null) } else { value };
        let mut commands = Vec::new();
        commands_in(&hooks, &mut commands);
        found.extend(commands.into_iter().map(|command| (rel.to_string(), command)));
    }
    found
}

/// The hook commands Claude reads from the approval channel of `session` (an Ask tab), when it has any.
fn channel_hooks(session: &Path) -> Vec<String> {
    let Some(path) = hooks_file_path(session).filter(|path| path.exists()) else { return Vec::new() };
    let mut commands = Vec::new();
    commands_in(&serde_json::from_str(&std::fs::read_to_string(path).expect("the hooks file")).expect("JSON"), &mut commands);
    commands
}

/// Every file below `dir` with its content, for a before and after.
fn snapshot(dir: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut files = BTreeMap::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        for entry in std::fs::read_dir(&current).into_iter().flatten().flatten() {
            let path = entry.path();
            match entry.file_type() {
                Ok(kind) if kind.is_dir() => stack.push(path),
                _ => {
                    files.insert(path.strip_prefix(dir).expect("below dir").to_path_buf(), std::fs::read(&path).unwrap_or_default());
                }
            }
        }
    }
    files
}

#[test]
fn without_a_private_data_folder_no_cli_gets_a_hook_in_ask_or_in_skip_mode() {
    without_private_root(|| {
        for mode in [PermissionMode::Ask, PermissionMode::AskRisky, PermissionMode::Skip] {
            let mut st = tab_state_in(mode);
            assert!(st.reset_agent_run(None).is_none(), "{mode:?}: the agent files are written");

            let installed = installed_in_session(&st.session_dir);
            assert!(installed.is_empty(), "{mode:?}: a hook is installed in the session folder, where an agent can write: {installed:#?}");
            assert!(channel_hooks(&st.session_dir).is_empty(), "{mode:?}: there is no channel to hold Claude's hooks");
            assert!(
                crate::gemini_status_hooks::prepare_for_tab(&st.session_dir, 1, AgentPermissionMode::Ask, AgentCliPreset::Gemini).is_none(),
                "{mode:?}: Gemini's status hooks are off"
            );
            for name in ["agent-status.json", "agent-events.jsonl", "turn-marks.jsonl", "claude-transcript.json"] {
                assert!(!st.session_dir.join(name).exists(), "{mode:?}: {name} is in the session folder");
            }
        }
    });
}

#[test]
fn when_only_the_hook_folder_cannot_be_made_the_approval_handler_is_the_one_hook_left_and_it_touches_no_session_file() {
    for mode in [PermissionMode::Ask, PermissionMode::AskRisky, PermissionMode::Skip] {
        let mut st = tab_state_in(mode);
        // A file where the hook folder goes: the approval channel is made, the hook folder cannot be.
        let channel = prepare_channel_dir(&st.session_dir).expect("the approval channel");
        std::fs::write(channel.join(crate::agent_hook_outputs::DIR_NAME), "in the way").expect("a file in the way");
        assert!(st.reset_agent_run(None).is_none(), "{mode:?}: the agent files are written");

        let installed = installed_in_session(&st.session_dir);
        assert!(installed.is_empty(), "{mode:?}: a hook is installed in the session folder, where an agent can write: {installed:#?}");
        let approval = channel_hooks(&st.session_dir);
        match mode {
            PermissionMode::Ask | PermissionMode::AskRisky => {
                assert_eq!(approval.len(), 1, "Ask keeps the approval handler and nothing else: {approval:#?}");
                assert!(approval[0].contains("approve-hook.sh"), "{approval:#?}");
                assert!(!approval[0].contains(st.session_dir.to_str().expect("a UTF-8 path")), "the handler names the session folder: {approval:#?}");
                let script =
                    std::fs::read_to_string(crate::agent_approval_channel::hook_script_path(&st.session_dir).expect("the script path")).expect("the script");
                assert!(!script.contains("STATUS_FILE"), "the handler has no status file to write when no hook has a folder");
            }
            PermissionMode::Skip => assert!(approval.is_empty(), "Skip installs no approval handler: {approval:#?}"),
        }
        assert!(
            crate::gemini_status_hooks::prepare_for_tab(&st.session_dir, 1, AgentPermissionMode::Ask, AgentCliPreset::Gemini).is_none(),
            "{mode:?}: Gemini's status hooks are off"
        );

        // Where a hook can be run, run it: the session folder is the same afterwards.
        let before = snapshot(&st.session_dir);
        for command in &approval {
            run_hook_with(command, &st.session_dir, "run-1", |_| {});
        }
        assert_eq!(snapshot(&st.session_dir), before, "{mode:?}: a hook changed the session folder");
    }
}
