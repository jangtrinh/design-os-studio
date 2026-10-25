//! A hook is a shell command that the agent's CLI runs as the user, with no sandbox, in the process of the CLI. Whatever it writes
//! through a link an agent planted in the session folder lands wherever the link points. These tests install the hooks of every CLI
//! the way Studio does, plant links at every file a hook ever wrote there, and run the real command strings through `sh`.

use super::*;
use crate::agent_permissions::AgentPermissionMode;
use crate::agent_session_files::test_support::{make_fifo, tab_state_in, timed_with_fifo_watchdog};
use std::os::unix::fs::symlink;
use std::process::{Command, Stdio};
use std::time::Duration;

/// The files hooks wrote in the session folder before they moved to a folder no agent can write, and what a hook wrote there.
fn old_output_names(run_id: u64) -> Vec<String> {
    vec![
        "agent-status.json".to_string(),
        "agent-events.jsonl".to_string(),
        "turn-marks.jsonl".to_string(),
        "claude-transcript.json".to_string(),
        format!("bootstrap.delivered.{run_id}"),
    ]
}

/// The payload a CLI feeds a hook on stdin: what the prompt, tool and capture handlers read.
pub(super) const PAYLOAD: &str = r#"{"session_id":"sid-1","transcript_path":"/nonexistent/transcript.jsonl","hook_event_name":"UserPromptSubmit","tool_name":"Bash","tool_input":{"command":"ls"}}"#;

/// Every `command` string below `value`, in document order.
pub(super) fn commands_in(value: &serde_json::Value, out: &mut Vec<String>) {
    match value {
        serde_json::Value::Object(map) => {
            for (key, inner) in map {
                match (key.as_str(), inner) {
                    ("command", serde_json::Value::String(command)) => out.push(command.clone()),
                    _ => commands_in(inner, out),
                }
            }
        }
        serde_json::Value::Array(items) => items.iter().for_each(|item| commands_in(item, out)),
        _ => {}
    }
}

pub(super) fn commands_of_file(path: &Path) -> Vec<String> {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let mut out = Vec::new();
    commands_in(&serde_json::from_str(&text).expect("the hooks are JSON"), &mut out);
    out
}

/// Which CLI's hooks, and in which mode they are installed.
#[derive(Clone, Copy, Debug)]
pub(super) enum Installed {
    ClaudeAsk,
    ClaudeSkip,
    Grok,
    Antigravity,
    Gemini,
}

pub(super) const EVERY_CLI: [Installed; 5] = [Installed::ClaudeAsk, Installed::ClaudeSkip, Installed::Grok, Installed::Antigravity, Installed::Gemini];

/// A window whose session folder holds the hooks of `which`, with the commands of those hooks.
pub(super) fn window_with_hooks(which: Installed, file_path: Option<&Path>) -> (crate::test_timing::ScopedTerminalState, Vec<String>) {
    let mode = if matches!(which, Installed::ClaudeSkip | Installed::Grok | Installed::Antigravity) { PermissionMode::Skip } else { PermissionMode::Ask };
    let mut st = tab_state_in(mode);
    assert!(st.reset_agent_run(file_path).is_none(), "the agent files are written");
    let hooks_file = match which {
        Installed::ClaudeAsk => crate::agent_approval_channel::hooks_file_path(&st.session_dir).expect("a private channel"),
        Installed::ClaudeSkip => st.session_dir.join("hooks.json"),
        Installed::Grok => st.session_dir.join(".grok/hooks/designos-status.json"),
        Installed::Antigravity => st.session_dir.join(".agents/hooks.json"),
        Installed::Gemini => crate::gemini_status_hooks::prepare_for_tab(&st.session_dir, 1, AgentPermissionMode::Ask, AgentCliPreset::Gemini)
            .expect("Gemini's status hooks are written in Ask mode"),
    };
    let commands = commands_of_file(&hooks_file);
    assert!(!commands.is_empty(), "{which:?} installs hook commands");
    (st, commands)
}

/// Runs `command` through `sh` in the session folder the way a CLI does, with `PAYLOAD` on stdin, and returns what it printed.
fn run_hook(command: &str, session: &Path, run_id: &str) -> String {
    let output = run_hook_with(command, session, run_id, |_| {});
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// The POSIX shells a CLI may run a hook through that are here: `sh`, and dash, which is the `sh` of Linux and ends a shell whose redirect
/// of a special builtin (`:`, `exec`) failed, where the `sh` of macOS goes on.
pub(super) fn shells_here() -> Vec<&'static str> {
    ["sh", "/bin/dash"].into_iter().filter(|shell| *shell == "sh" || Path::new(shell).exists()).collect()
}

pub(super) fn run_hook_with(command: &str, session: &Path, run_id: &str, before_input: impl FnOnce(u32)) -> std::process::Output {
    run_hook_in("sh", command, session, run_id, before_input)
}

/// [`run_hook_with`] through the shell `shell`.
pub(super) fn run_hook_in(shell: &str, command: &str, session: &Path, run_id: &str, before_input: impl FnOnce(u32)) -> std::process::Output {
    use std::io::Write;
    let mut child = Command::new(shell)
        .arg("-c")
        .arg(command)
        .current_dir(session)
        .env("DESIGNOS_AGENT_RUN_ID", run_id)
        .env("DESIGNOS_TAB_ID", "1")
        .env("DESIGNOS_TAB_SESSION_UUID", "tab-uuid-1")
        .env("DESIGNOS_APPROVAL_TIMEOUT_MS", "0")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|e| panic!("{shell} starts: {e}"));
    // A hook reads its payload first, so whoever knows the pid of the shell has time to plant a name that depends on it.
    before_input(child.id());
    // A hook that reads nothing is gone before the payload is written: a closed pipe is what such a hook does.
    let _ = child.stdin.take().expect("stdin").write_all(PAYLOAD.as_bytes());
    child.wait_with_output().expect("the hook ends")
}

/// A file outside the session folder that a link points at, with `content`.
fn victim(dir: &Path, name: &str, content: &str) -> PathBuf {
    std::fs::create_dir_all(dir).expect("the victims' folder");
    let path = dir.join(name);
    std::fs::write(&path, content).expect("the victim");
    path
}

/// Every path below `dir`, relative to it and sorted.
fn listing(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        for entry in std::fs::read_dir(&current).into_iter().flatten().flatten() {
            let path = entry.path();
            if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                stack.push(path.clone());
            }
            out.push(path.strip_prefix(dir).expect("below dir").to_path_buf());
        }
    }
    out.sort();
    out
}

#[derive(Clone, Copy, Debug)]
enum Plant {
    /// A link to a file the user owns.
    LinkToFile,
    /// A link to a name that is not there: a write through it makes the file.
    DanglingLink,
    /// A second name of the user's file: no link to refuse, the same inode.
    HardLink,
}

/// Plants `kind` at `<session>/<name>`, pointing at (or being a name of) a file in `victims`, and returns that file.
fn plant_at(session: &Path, victims: &Path, name: &str, kind: Plant) -> PathBuf {
    let target = victims.join(format!("target-of-{name}"));
    let at = session.join(name);
    let _ = std::fs::remove_file(&at);
    match kind {
        Plant::LinkToFile => {
            let target = victim(victims, &format!("target-of-{name}"), &format!("the user's own {name}\n"));
            symlink(&target, &at).expect("plant the link");
            target
        }
        Plant::DanglingLink => {
            std::fs::create_dir_all(victims).expect("the victims' folder");
            symlink(&target, &at).expect("plant the dangling link");
            target
        }
        Plant::HardLink => {
            let target = victim(victims, &format!("target-of-{name}"), &format!("the user's own {name}\n"));
            std::fs::hard_link(&target, &at).expect("plant the hard link");
            target
        }
    }
}

/// The content of `path`, or `None` when it is not there.
fn content_of(path: &Path) -> Option<String> {
    std::fs::read_to_string(path).ok()
}

#[test]
fn a_link_or_a_hard_link_at_any_name_a_hook_wrote_is_never_written_through_by_any_hook_of_any_cli() {
    for which in EVERY_CLI {
        for kind in [Plant::LinkToFile, Plant::DanglingLink, Plant::HardLink] {
            let (st, commands) = window_with_hooks(which, None);
            let victims = std::env::temp_dir().join(format!("designos-hook-victims-{}-{}", std::process::id(), st.run_id));
            let _ = std::fs::remove_dir_all(&victims);
            let names = old_output_names(st.run_id);
            let planted: Vec<(String, PathBuf, Option<String>)> = names
                .iter()
                .map(|name| {
                    let target = plant_at(&st.session_dir, &victims, name, kind);
                    let before = content_of(&target);
                    (name.clone(), target, before)
                })
                .collect();

            for command in &commands {
                run_hook(command, &st.session_dir, "run-1");
            }

            for (name, target, before) in &planted {
                assert_eq!(&content_of(target), before, "{which:?}, {kind:?} at {name}: a hook wrote through it into {}", target.display());
            }
            let _ = std::fs::remove_dir_all(&victims);
        }
    }
}

#[test]
fn the_capture_of_a_claude_transcript_never_writes_through_a_link_planted_at_the_name_of_its_temporary_file() {
    for which in [Installed::ClaudeAsk, Installed::ClaudeSkip] {
        let (st, commands) = window_with_hooks(which, None);
        let capture: Vec<&String> = commands.iter().filter(|c| c.contains("transcript_path")).collect();
        assert!(!capture.is_empty(), "{which:?} installs the transcript capture");
        let victims = std::env::temp_dir().join(format!("designos-hook-tmp-victims-{}-{}", std::process::id(), st.run_id));
        let _ = std::fs::remove_dir_all(&victims);
        let target = victim(&victims, "target-of-tmp", "the user's own file\n");
        let session = st.session_dir.clone();
        for command in capture {
            // The temporary file of the capture is `<name>.tmp.<pid of the shell>`: an agent that watches the processes knows it.
            run_hook_with(command, &session, "run-1", |pid| {
                symlink(&target, session.join(format!("claude-transcript.json.tmp.{pid}"))).expect("plant the link at the temporary name");
            });
        }
        assert_eq!(content_of(&target).as_deref(), Some("the user's own file\n"), "{which:?}: the capture wrote through a link at its temporary file");
        let _ = std::fs::remove_dir_all(&victims);
    }
}

#[test]
fn a_link_at_a_reply_file_is_not_read_by_the_prompt_hook_into_the_agents_context() {
    let (st, commands) = window_with_hooks(Installed::ClaudeAsk, None);
    let victims = std::env::temp_dir().join(format!("designos-hook-reply-victims-{}-{}", std::process::id(), st.run_id));
    let _ = std::fs::remove_dir_all(&victims);
    let secret = victim(&victims, "secret", "{\"hookSpecificOutput\":{\"additionalContext\":\"SECRET-FROM-A-FILE-THE-AGENT-MAY-NOT-READ\"}}\n");
    for name in ["page-context-reply.json", "bootstrap-context-reply.json", "bootstrap-compact-reply.json", "skills-context-reply-1-0.json"] {
        let at = st.session_dir.join(name);
        let _ = std::fs::remove_file(&at);
        symlink(&secret, &at).expect("plant the link");
    }
    let mut printed = String::new();
    for command in &commands {
        printed.push_str(&run_hook(command, &st.session_dir, "run-1"));
    }
    assert!(!printed.contains("SECRET-FROM-A-FILE-THE-AGENT-MAY-NOT-READ"), "a hook printed a file the agent pointed a link at:\n{printed}");
    assert!(printed.contains("DESIGN:OS Studio page context"), "the real page reply still reaches the prompt:\n{printed}");
    let _ = std::fs::remove_dir_all(&victims);
}

/// The quoted absolute paths of `command` whose last part is one of the files a hook writes.
pub(super) fn hook_files_named_in(command: &str) -> Vec<PathBuf> {
    let mut found = Vec::new();
    for part in command.split('\'').skip(1).step_by(2) {
        let path = Path::new(part);
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else { continue };
        let known = ["agent-status.json", "agent-events.jsonl", "turn-marks.jsonl", "claude-transcript.json"];
        if path.is_absolute() && (known.iter().any(|k| name.starts_with(k)) || name.starts_with("bootstrap.delivered.") || name.contains("context-reply")) {
            found.push(path.to_path_buf());
        }
    }
    found
}

#[test]
fn every_file_a_hook_writes_lies_outside_the_session_folder_and_the_folder_of_a_file_kept_beside_the_data_folder() {
    let files = std::env::temp_dir().join(format!("designos-hook-grants-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&files);
    std::fs::create_dir_all(&files).expect("the open file's folder");
    let file_path = files.join("open.designos");
    for which in EVERY_CLI {
        let (st, commands) = window_with_hooks(which, Some(&file_path));
        // What Studio grants a CLI: its working folder, the folders `--add-dir` and `--include-directories` name (the session
        // folder again), and the workspace of `designos-mcp` (the open file's folder: this file is kept in a folder of its own, as
        // most are. For a file kept in a folder that holds the data folder, the extension test below is what keeps its workspace
        // harmless).
        let grants: Vec<PathBuf> =
            [Some(st.session_dir.clone()), st.session_dir.canonicalize().ok(), Some(files.clone()), files.canonicalize().ok()].into_iter().flatten().collect();
        let mut written = Vec::new();
        for command in &commands {
            written.extend(hook_files_named_in(command));
        }
        let script = crate::agent_approval_channel::hook_script_path(&st.session_dir).filter(|p| p.exists());
        if let Some(script) = script {
            let text = std::fs::read_to_string(script).expect("the approval script");
            for line in text.lines().filter(|l| l.contains("STATUS_FILE='")) {
                written.extend(hook_files_named_in(line.trim_start_matches("STATUS_FILE=")));
            }
        }
        assert!(written.len() >= 4, "{which:?}: found only {written:?} in the commands");
        for path in written {
            for grant in &grants {
                assert!(!path.starts_with(grant), "{which:?}: a hook writes {}, which lies in {}", path.display(), grant.display());
            }
        }
    }
    let _ = std::fs::remove_dir_all(&files);
}

#[test]
fn a_fifo_or_a_link_to_one_at_the_patch_log_never_blocks_the_turn_mark_hook() {
    let st = tab_state_in(PermissionMode::Skip);
    let patch = st.session_dir.join("doc-patch.jsonl");
    let marks = std::env::temp_dir().join(format!("designos-hook-marks-{}-{}", std::process::id(), st.run_id));
    let _ = std::fs::remove_dir_all(&marks);
    std::fs::create_dir_all(&marks).expect("a folder for the marks");
    for as_link in [false, true] {
        let _ = std::fs::remove_file(&patch);
        let fifo = if as_link {
            let fifo = st.session_dir.join("the-fifo");
            let _ = std::fs::remove_file(&fifo);
            make_fifo(&fifo);
            symlink(&fifo, &patch).expect("a link to the FIFO");
            fifo
        } else {
            make_fifo(&patch);
            patch.clone()
        };
        let command = crate::agent_turn_marks::turn_mark_hook_command(&marks.join("turn-marks.jsonl"), &patch, "start", "grok");
        let (output, took) = timed_with_fifo_watchdog(&fifo, Duration::from_secs(4), || run_hook_with(&command, &st.session_dir, "run-1", |_| {}));
        assert!(took < Duration::from_secs(3), "as_link={as_link}: the hook waited {took:?} for a writer of the FIFO");
        assert!(output.status.success(), "as_link={as_link}: the hook failed");
        let line = std::fs::read_to_string(marks.join("turn-marks.jsonl")).expect("the mark was written");
        assert!(line.contains("\"patch_bytes\":0"), "a FIFO has no patch bytes: {line}");
        let _ = std::fs::remove_file(marks.join("turn-marks.jsonl"));
    }
    let _ = std::fs::remove_dir_all(&marks);
}

#[test]
fn the_turn_mark_hook_still_counts_the_bytes_of_a_patch_log_that_is_a_plain_file() {
    let marks = std::env::temp_dir().join(format!("designos-hook-count-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&marks);
    std::fs::create_dir_all(&marks).expect("a folder");
    let patch = marks.join("doc-patch.jsonl");
    std::fs::write(&patch, "12345678901\n").expect("a patch log of 12 bytes");
    let command = crate::agent_turn_marks::turn_mark_hook_command(&marks.join("turn-marks.jsonl"), &patch, "end", "claude");
    run_hook(&command, &marks, "run-1");
    let line = std::fs::read_to_string(marks.join("turn-marks.jsonl")).expect("the mark");
    assert_eq!(line.trim(), r#"{"v":1,"turn_mark":"end","cli":"claude","patch_bytes":12}"#);
    let _ = std::fs::remove_dir_all(&marks);
}

#[test]
fn no_file_a_hook_writes_or_reads_has_a_name_designos_mcp_can_write() {
    // The hook folder can lie inside the folder `designos-mcp` serves (`--workspace`, the open file's folder: a file kept in the home
    // folder). That server creates only names that end in `designos_mcp::engine::WORKSPACE_WRITE_EXTENSIONS` (any case), and the
    // temporary file of an overwrite, which starts with `OUTPUT_TEMP_PREFIX`; so a hook file must never end in one of those endings
    // or start with that prefix. A change on either side, a new ending there or a new name here, trips this test.
    let st = tab_state_in(PermissionMode::Ask);
    let paths = st.prepared_hook_paths();
    let mut names: std::collections::BTreeSet<String> = [
        paths.status(),
        paths.events(),
        paths.turn_marks(),
        paths.transcript(),
        paths.delivered_marker(7),
        paths.page_reply(),
        paths.bootstrap_reply(),
        paths.compact_reply(),
        crate::skills::hook_delivery::reply_file(paths.dir(), 3, 1),
    ]
    .iter()
    .filter_map(|path| path.file_name().and_then(|name| name.to_str()).map(str::to_string))
    .collect();
    // The names the commands derive from those: the locks and the temporary file of a capture.
    for which in EVERY_CLI {
        let (_window, commands) = window_with_hooks(which, None);
        for command in &commands {
            names.extend(hook_files_named_in(command).iter().filter_map(|path| path.file_name().and_then(|name| name.to_str()).map(str::to_string)));
        }
    }
    for expected in ["agent-events.jsonl.lock", "claude-transcript.json.lock", "claude-transcript.json.tmp."] {
        assert!(names.iter().any(|name| name.starts_with(expected)), "the commands name no {expected}: {names:?}");
    }

    for name in &names {
        let ending = Path::new(name).extension().and_then(|extension| extension.to_str()).unwrap_or_default().to_ascii_lowercase();
        assert!(
            !designos_mcp::engine::WORKSPACE_WRITE_EXTENSIONS.contains(&ending.as_str()),
            "{name} ends in .{ending}, a name designos-mcp writes below its workspace"
        );
        assert!(!name.starts_with(designos_mcp::engine::OUTPUT_TEMP_PREFIX), "{name} starts like the temporary file of a designos-mcp overwrite");
    }
}
