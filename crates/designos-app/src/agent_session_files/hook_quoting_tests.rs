//! The hook commands Studio writes name files in the hook folder next to the approval channel, and measure one file of the session
//! folder. The data folder of a Mac user sits under `Application Support` and can hold an apostrophe (a home folder named after a
//! person), and so can the session folder, so every path in every hook command must survive both.

use super::folder_tests::{scratch_dir, Cleanup};
use super::*;
use designos_doc::Document;
use serde_json::Value;
use std::process::{Command, Stdio};

pub(super) fn job_in(session_dir: &Path) -> SnapshotJob {
    SnapshotJob {
        session_dir: session_dir.to_path_buf(),
        session_id: "hook-quoting".into(),
        key: (1, 0, 0),
        context_json_path: session_dir.join("context.json"),
        live_canvas_path: session_dir.join("canvas.designos"),
        live_spec_path: session_dir.join("live-spec.json"),
        live_doc_sync_path: session_dir.join("doc-sync.designos"),
        live_doc_patch_path: session_dir.join("doc-patch.jsonl"),
        context_bytes: b"{}".to_vec(),
        canvas_doc: Document::new("Hook Quoting"),
        doc_name: "Hook Quoting".into(),
        current_page_id: 0,
        current_page_name: "Page 1".into(),
        file_path: None,
        run_id: "hook-quoting-run".into(),
        bootstrap_run_id: 1,
        agent_runtime: "claude".into(),
        ask_mode: false,
        unread_probe: None,
        exact_project_config: None,
    }
}

/// Every `command` string of a hooks file, whatever the shape of the file.
fn commands_of(value: &Value, out: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            for (key, inner) in map {
                match inner {
                    Value::String(command) if key == "command" => out.push(command.clone()),
                    _ => commands_of(inner, out),
                }
            }
        }
        Value::Array(items) => items.iter().for_each(|item| commands_of(item, out)),
        _ => {}
    }
}

fn run_sh(command: &str, session_dir: &Path, stdin: &str) -> std::process::Output {
    let mut child = Command::new("/bin/sh")
        .args(["-c", command])
        .env("DESIGNOS_AGENT_RUN_ID", "run-1")
        .env("DESIGNOS_TAB_ID", "1")
        .current_dir(session_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start /bin/sh");
    std::io::Write::write_all(&mut child.stdin.take().expect("stdin"), stdin.as_bytes()).expect("feed stdin");
    child.wait_with_output().expect("run the hook")
}

#[test]
fn every_hook_command_survives_an_apostrophe_and_a_space_in_the_session_folder_and_in_the_data_folder() {
    let scratch = scratch_dir("hook-quoting");
    let _cleanup = Cleanup(vec![scratch.clone()]);
    let private_root = scratch.join("Jang's Application Support").join("ap");
    crate::agent_approval_channel::with_private_root_at(&private_root, || every_hook_command_survives_these_folders(&scratch));
}

fn every_hook_command_survives_these_folders(scratch: &Path) {
    let session_dir = scratch.join("Jang's folder").join("agent-sessions").join("file-1");
    ensure_session_dir(&session_dir).expect("the session folder");
    crate::terminal::write_session_agent_files_for_job(&job_in(&session_dir)).expect("write the session files");
    let hook_paths = crate::agent_hook_outputs::HookPaths::locate(&session_dir);
    assert!(
        hook_paths.dir().starts_with(scratch.join("Jang's Application Support")),
        "the hooks report below the private root: {}",
        hook_paths.dir().display()
    );

    let mut commands = Vec::new();
    for hooks_file in ["hooks.json", ".grok/hooks/designos-status.json", ".agents/hooks.json"] {
        let text = std::fs::read_to_string(session_dir.join(hooks_file)).unwrap_or_else(|e| panic!("{hooks_file}: {e}"));
        commands_of(&serde_json::from_str(&text).expect("hooks json"), &mut commands);
    }
    assert!(commands.len() > 20, "the hooks files hold their commands: {}", commands.len());
    for command in &commands {
        let check = Command::new("/bin/sh").args(["-n", "-c", command]).output().expect("run sh -n");
        assert!(check.status.success(), "/bin/sh rejects a hook command: {}\n{command}", String::from_utf8_lossy(&check.stderr));
    }

    // The commands also work on that folder: the status hook, the transcript capture and a skills reply handler.
    let claude: Value = serde_json::from_str(&std::fs::read_to_string(session_dir.join("hooks.json")).expect("hooks.json")).expect("json");
    let mut pre_tool = Vec::new();
    commands_of(&claude["hooks"]["PreToolUse"], &mut pre_tool);
    assert!(run_sh(&pre_tool[0], &session_dir, "{}").status.success(), "the status hook failed");
    let status = std::fs::read_to_string(hook_paths.status()).expect("the status hook wrote the status file");
    assert!(status.contains("\"state\":\"working\""), "{status}");
    assert!(hook_paths.events().is_file(), "the status hook appended its event");

    let capture = pre_tool.last().expect("capture command");
    run_sh(capture, &session_dir, r#"{"session_id":"s1","transcript_path":"/x/t.jsonl"}"#);
    let meta = std::fs::read_to_string(hook_paths.transcript()).expect("the capture wrote its file");
    assert!(meta.contains("\"session_id\":\"s1\""), "{meta}");

    // The prompt hook: the first turn prints the bootstrap reply and marks it delivered, a later turn prints the page reply.
    let mut prompt = Vec::new();
    commands_of(&claude["hooks"]["UserPromptSubmit"], &mut prompt);
    std::fs::write(hook_paths.bootstrap_reply(), "BOOTSTRAP-REPLY").expect("a bootstrap reply");
    let first = run_sh(&prompt[0], &session_dir, "{}");
    let first_out = String::from_utf8_lossy(&first.stdout);
    assert!(first.status.success() && first_out.contains("BOOTSTRAP-REPLY"), "the first prompt printed {first_out:?}");
    assert!(hook_paths.delivered_marker(1).exists(), "the first prompt did not mark the bootstrap as delivered");
    let later = run_sh(&prompt[0], &session_dir, "{}");
    let later_out = String::from_utf8_lossy(&later.stdout);
    assert!(
        later.status.success() && later_out.contains("additionalContext") && !later_out.contains("BOOTSTRAP-REPLY"),
        "a later prompt printed {later_out:?}"
    );

    assert!(crate::skills::hook_delivery::write_replies(hook_paths.dir(), 1, &["skill reply".to_string()], 0));
    let handler = &crate::skills::hook_delivery::handler_commands(hook_paths.dir())[0];
    let printed = run_sh(handler, &session_dir, "");
    assert!(String::from_utf8_lossy(&printed.stdout).contains("skill reply"), "the handler printed {:?}", String::from_utf8_lossy(&printed.stdout));
}
