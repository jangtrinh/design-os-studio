//! Golden values for what Studio launches and writes for each agent CLI in Ask and in Skip mode. A denylist only
//! rejects today's spelling of a bypass (`--allowedTools`, `-y`, `--allow` would pass it), so these tests pin the whole
//! launch command of a new session, a session with an id and a resume, and every config file the CLI reads. Any added
//! flag, key or handler changes a value here and fails the test: adding one is then a reviewed edit of this file.

use super::*;
use std::collections::BTreeSet;

/// Claude's launch command after the flags that differ by mode and session: the same for every launch.
const CLAUDE_TAIL: &str = " --mcp-config \"$DESIGNOS_MCP_CONFIG\" --settings \"$DESIGNOS_SESSION_DIR/hooks.json\" --append-system-prompt \"You are inside DESIGN:OS Studio bound exclusively to file '$DESIGNOS_DOC_NAME' (session $DESIGNOS_SESSION_DIR). Use the pre-fetched bootstrap context (or call designos_get_app_state) for the current selected page, frames, tokens, and components, and deliver all designs onto that file and page via designos-mcp tools (designos_apply_spec, designos_create_node, designos_update_node) or by writing NodeSpec JSON to $DESIGNOS_LIVE_SPEC.\"";

/// The `-c` overrides that register `designos-mcp` with Codex. Written out, not taken from the macro that builds the real
/// command, so an override added there fails the test.
const CODEX_OVERRIDES: &str = " -c 'mcp_servers.designos.command=\"'\"$DESIGNOS_MCP_BIN\"'\"' -c 'mcp_servers.designos.args=[\"--workspace\",\"'\"$DESIGNOS_WORKSPACE\"'\"]' -c 'mcp_servers.designos.env_vars=[\"DESIGNOS_SESSION_DIR\",\"DESIGNOS_MCP_CONFIG\",\"DESIGNOS_LIVE_CANVAS\",\"DESIGNOS_LIVE_DOC_SYNC\",\"DESIGNOS_LIVE_DOC_PATCH\",\"DESIGNOS_LIVE_SPEC\",\"DESIGNOS_CONTEXT_JSON\",\"DESIGNOS_TARGET_PAGE_ID\",\"DESIGNOS_TARGET_PAGE_NAME\",\"DESIGNOS_DOC_NAME\",\"DESIGNOS_FILE\",\"DESIGNOS_AGENT_RUN_ID\",\"DESIGNOS_AGENT_RUNTIME\",\"DESIGNOS_IN_PROGRESS_STALE_MINUTES\"]'";

const GEMINI_DIRS: &str = " --include-directories \"$DESIGNOS_SESSION_DIR\"";

/// The commands, in the order of [`launch_commands`]: a new session, a new session with an id, a resume with an id, a resume
/// without one.
fn golden_launches(cli: &str, mode: PermissionMode) -> [String; 4] {
    let skip = mode == PermissionMode::Skip;
    let ids = UUID;
    match cli {
        "claude" => {
            let flag = if skip { " --dangerously-skip-permissions" } else { "" };
            [
                format!("claude{flag}{CLAUDE_TAIL}"),
                format!("claude --session-id {ids}{flag}{CLAUDE_TAIL}"),
                format!("claude --resume {ids}{flag}{CLAUDE_TAIL}"),
                format!("claude --continue{flag}{CLAUDE_TAIL}"),
            ]
        }
        "codex" => {
            let flag = if skip { " --dangerously-bypass-approvals-and-sandbox" } else { "" };
            [
                format!("codex{flag}{CODEX_OVERRIDES}"),
                format!("codex{flag}{CODEX_OVERRIDES}"),
                format!("codex resume --last{flag}{CODEX_OVERRIDES}"),
                format!("codex resume --last{flag}{CODEX_OVERRIDES}"),
            ]
        }
        "gemini" => {
            let approval = if skip { "yolo" } else { "default" };
            [
                format!("gemini --approval-mode {approval}{GEMINI_DIRS}"),
                format!("gemini --approval-mode {approval}{GEMINI_DIRS}"),
                format!("gemini --resume {ids} --approval-mode {approval}{GEMINI_DIRS}"),
                format!("gemini --resume latest --approval-mode {approval}{GEMINI_DIRS}"),
            ]
        }
        "agy" => {
            let flag = if skip { " --dangerously-skip-permissions" } else { "" };
            [
                format!("agy{flag} --add-dir \"$DESIGNOS_SESSION_DIR\""),
                format!("agy{flag} --add-dir \"$DESIGNOS_SESSION_DIR\""),
                format!("agy --continue{flag} --add-dir \"$DESIGNOS_SESSION_DIR\""),
                format!("agy --continue{flag} --add-dir \"$DESIGNOS_SESSION_DIR\""),
            ]
        }
        "grok" => {
            let flag = if skip { " --always-approve" } else { "" };
            [
                format!("grok{flag} --cwd \"$DESIGNOS_SESSION_DIR\""),
                format!("grok --session-id {ids}{flag} --cwd \"$DESIGNOS_SESSION_DIR\""),
                format!("grok --resume {ids}{flag} --cwd \"$DESIGNOS_SESSION_DIR\""),
                format!("grok --continue{flag} --cwd \"$DESIGNOS_SESSION_DIR\""),
            ]
        }
        other => panic!("no golden launch for {other}"),
    }
}

fn assert_launch_golden(preset: AgentCliPreset, cli: &str) {
    for mode in [PermissionMode::Ask, PermissionMode::Skip] {
        let actual = launch_commands(preset, cli, mode);
        let golden = golden_launches(cli, mode);
        assert_eq!(actual.len(), golden.len(), "{cli} {mode:?}: one command per launch path");
        for (i, (got, want)) in actual.iter().zip(golden.iter()).enumerate() {
            assert_eq!(got, want, "{cli} {mode:?}: launch {i} (new, new with id, resume with id, resume without id) changed");
        }
    }
}

/// `text` with machine-specific paths and the session id replaced by placeholders. `paths` pairs each path with its
/// placeholder, longest path first.
///
/// A path is written in two spellings: as it is, and with every backslash doubled inside a JSON or TOML string (a Windows
/// path). On Windows the session folder is also joined with `\` where the goldens write `/`, so a separator that follows a
/// placeholder becomes `/` in either spelling. Paths without backslashes are untouched by all of this.
fn normalize_text(raw: &str, paths: &[(String, &str)], session_id: &str) -> String {
    let mut text = raw.to_string();
    for (path, placeholder) in paths {
        text = text.replace(&path.replace('\\', "\\\\"), placeholder);
        text = text.replace(path.as_str(), placeholder);
    }
    for placeholder in ["<SESSION>", "<WORKSPACE>"] {
        text = text.replace(&format!("{placeholder}\\\\"), &format!("{placeholder}/"));
        text = text.replace(&format!("{placeholder}\\"), &format!("{placeholder}/"));
    }
    text.replace(session_id, "<SESSION_ID>")
}

/// `rel` of the session folder as text, with the values that differ per machine and run replaced.
fn normalized(st: &TerminalState, rel: &str) -> String {
    let raw = std::fs::read_to_string(st.session_dir.join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"));
    let mut paths = Vec::new();
    if let Ok(canon) = st.session_dir.canonicalize() {
        paths.push((canon.display().to_string(), "<SESSION>"));
    }
    paths.push((st.session_dir.display().to_string(), "<SESSION>"));
    if let Ok(cwd) = std::env::current_dir() {
        paths.push((cwd.display().to_string(), "<WORKSPACE>"));
    }
    normalize_text(&raw, &paths, &st.session_id)
}

fn normalized_json(st: &TerminalState, rel: &str) -> Value {
    let mut value: Value = serde_json::from_str(&normalized(st, rel)).unwrap_or_else(|e| panic!("{rel} is not JSON: {e}"));
    scrub_machine_values(&mut value);
    value
}

/// The `designos-mcp` binary is a path next to the test executable or a bare name, and the stale-minutes value can come
/// from the environment: neither is a setting Studio chooses per mode.
fn scrub_machine_values(value: &mut Value) {
    if let Some(entry) = value.pointer_mut("/mcpServers/designos").and_then(Value::as_object_mut) {
        entry.insert("command".into(), Value::String("<MCP_BIN>".into()));
        if let Some(env) = entry.get_mut("env").and_then(Value::as_object_mut) {
            env.insert("DESIGNOS_IN_PROGRESS_STALE_MINUTES".into(), Value::String("<MINUTES>".into()));
        }
    }
}

/// The `designos` MCP server entry every CLI is given.
fn golden_server_entry() -> Value {
    serde_json::json!({
        "args": ["--workspace", "<WORKSPACE>"],
        "command": "<MCP_BIN>",
        "env": {
            "DESIGNOS_AGENT_RUNTIME": "claude",
            "DESIGNOS_AGENT_RUN_ID": "<SESSION_ID>-1-1",
            "DESIGNOS_CONTEXT_JSON": "<SESSION>/context.json",
            "DESIGNOS_DOC_NAME": "Untitled",
            "DESIGNOS_FILE": "",
            "DESIGNOS_IN_PROGRESS_STALE_MINUTES": "<MINUTES>",
            "DESIGNOS_LIVE_CANVAS": "<SESSION>/canvas.designos",
            "DESIGNOS_LIVE_DOC_PATCH": "<SESSION>/doc-patch.jsonl",
            "DESIGNOS_LIVE_DOC_SYNC": "<SESSION>/doc-sync.designos",
            "DESIGNOS_LIVE_SPEC": "<SESSION>/live-spec.json",
            "DESIGNOS_MCP_CONFIG": "<SESSION>/mcp-config.json",
            "DESIGNOS_SESSION_DIR": "<SESSION>",
            "DESIGNOS_TARGET_PAGE_ID": "1",
            "DESIGNOS_TARGET_PAGE_NAME": "Page 1"
        }
    })
}

fn golden_servers() -> Value {
    serde_json::json!({ "mcpServers": { "designos": golden_server_entry() } })
}

/// Every leaf of `value` as a path with array positions collapsed to `[]`: the shape of a config without its values.
fn leaf_paths(value: &Value, path: &str, out: &mut BTreeSet<String>) {
    match value {
        Value::Object(map) => map.iter().for_each(|(k, v)| leaf_paths(v, &format!("{path}/{k}"), out)),
        Value::Array(items) => items.iter().for_each(|v| leaf_paths(v, &format!("{path}[]"), out)),
        _ => {
            out.insert(path.to_string());
        }
    }
}

fn shape_of(value: &Value) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    leaf_paths(value, "", &mut out);
    out
}

fn expected_shape(prefix: &str, events: &[&str], leaves: &[&str]) -> BTreeSet<String> {
    events.iter().flat_map(|e| leaves.iter().map(move |l| format!("{prefix}/{e}[]{l}"))).collect()
}

const CLAUDE_EVENTS: &[&str] = &[
    "Notification",
    "PermissionRequest",
    "PostToolUse",
    "PostToolUseFailure",
    "PreToolUse",
    "SessionStart",
    "Stop",
    "StopCancelled",
    "StopFailure",
    "UserPromptSubmit",
];
const CLAUDE_LEAVES: &[&str] = &["/hooks[]/command", "/hooks[]/type", "/matcher"];

/// Files each CLI reads that Studio writes, relative to the session folder. A file that is not listed here for any CLI is
/// not read by one, so a new file must be added here (and reviewed) before it can carry a setting.
fn config_dirs_in(st: &TerminalState) -> BTreeSet<String> {
    ["", ".claude", ".codex", ".gemini", ".grok", ".agents"]
        .iter()
        .flat_map(|dir| {
            let path = st.session_dir.join(dir);
            std::fs::read_dir(&path).into_iter().flatten().flatten().filter_map(move |e| {
                let name = e.file_name().to_string_lossy().into_owned();
                (name.ends_with(".json") || name.ends_with(".toml")).then(|| if dir.is_empty() { name } else { format!("{dir}/{name}") })
            })
        })
        .collect()
}

#[test]
fn claude_launch_and_config_match_the_golden() {
    assert_launch_golden(AgentCliPreset::Claude, "claude");
    for mode in [PermissionMode::Ask, PermissionMode::Skip] {
        let st = session_in(mode);
        let ask = mode == PermissionMode::Ask;
        assert_eq!(normalized_json(&st, "mcp-config.json"), golden_servers(), "Claude {mode:?}: mcp-config.json");

        let hooks = read_json(&st, "hooks.json");
        let mut expected = expected_shape("/hooks", CLAUDE_EVENTS, CLAUDE_LEAVES);
        if ask {
            expected.insert("/hooks/PermissionRequest[]/hooks[]/timeout".into());
        }
        assert_eq!(shape_of(&hooks), expected, "Claude {mode:?}: the keys of hooks.json changed");
        let handlers = hooks["hooks"]["PermissionRequest"][0]["hooks"].as_array().expect("PermissionRequest handlers");
        let approve: Vec<&Value> = handlers.iter().filter(|h| h.get("timeout").is_some()).collect();
        if ask {
            let script = st.session_dir.join("approve-hook.sh").display().to_string();
            assert_eq!(
                approve,
                [&serde_json::json!({ "type": "command", "command": format!("sh '{script}'"), "timeout": 300 })],
                "Claude Ask: the only handler with a decision is the in-panel approval hook"
            );
            let script = std::fs::read_to_string(st.session_dir.join("approve-hook.sh")).expect("approve-hook.sh");
            assert_eq!(script.matches("\"behavior\":\"allow\"").count(), 1, "the hook allows in exactly one place");
            assert!(script.contains("ANS_FILE=") && script.contains("allow)"), "and only from the answer the card wrote");
        } else {
            assert!(approve.is_empty(), "Claude Skip has no approval handler: {approve:?}");
        }
        let files = config_dirs_in(&st);
        for settings in [".claude/settings.json", ".claude/settings.local.json"] {
            assert!(!files.contains(settings), "Claude {mode:?}: Studio writes no {settings}");
        }
        finish(st);
    }
}

#[test]
fn codex_launch_and_config_match_the_golden() {
    assert_launch_golden(AgentCliPreset::Codex, "codex");
    for mode in [PermissionMode::Ask, PermissionMode::Skip] {
        let st = session_in(mode);
        let files = config_dirs_in(&st);
        assert!(files.iter().all(|f| !f.starts_with(".codex") && f != "config.toml"), "Codex reads no session config file: {files:?}");
        finish(st);
    }
}

#[test]
fn gemini_launch_and_config_match_the_golden_and_never_trust_the_server() {
    assert_launch_golden(AgentCliPreset::Gemini, "gemini");
    // Whatever the mode: Skip's `--approval-mode yolo` already allows every tool (policies/yolo.toml, priority 998), so the
    // server never needs `trust`, and a settings file that does not depend on the mode cannot start trusting a tab that
    // launched in Ask.
    for mode in [PermissionMode::Ask, PermissionMode::Skip] {
        let st = session_in(mode);
        assert_eq!(normalized_json(&st, ".gemini/settings.json"), golden_servers(), "Gemini {mode:?}: .gemini/settings.json");
        assert_eq!(
            normalized_json(&st, ".gemini/trustedFolders.json"),
            serde_json::json!({ "<SESSION>": "TRUST_FOLDER" }),
            "Gemini {mode:?}: trustedFolders.json"
        );
        finish(st);
    }
}

#[test]
fn antigravity_launch_and_config_match_the_golden() {
    assert_launch_golden(AgentCliPreset::Antigravity, "agy");
    for mode in [PermissionMode::Ask, PermissionMode::Skip] {
        let st = session_in(mode);
        assert_eq!(read_json(&st, ".agents/plugins/designos/plugin.json"), serde_json::json!({ "name": "designos" }));
        assert_eq!(normalized_json(&st, ".agents/plugins/designos/mcp_config.json"), golden_servers(), "agy {mode:?}: plugin mcp_config.json");
        let events = ["PostToolUse", "PostToolUseFailure", "PreInvocation", "PreToolUse", "Stop", "StopFailure"];
        let expected = expected_shape("/designos-status", &events, &["/command", "/type"]);
        assert_eq!(shape_of(&read_json(&st, ".agents/hooks.json")), expected, "agy {mode:?}: the keys of .agents/hooks.json changed");
        finish(st);
    }
}

#[test]
fn grok_launch_and_config_match_the_golden() {
    assert_launch_golden(AgentCliPreset::Grok, "grok");
    for mode in [PermissionMode::Ask, PermissionMode::Skip] {
        let st = session_in(mode);
        let toml: String = normalized(&st, ".grok/config.toml")
            .lines()
            .map(|l| if l.starts_with("command = ") { "command = \"<MCP_BIN>\"".to_string() } else { l.to_string() })
            .collect::<Vec<_>>()
            .join("\n");
        let toml = toml.replace("DESIGNOS_IN_PROGRESS_STALE_MINUTES = \"15\"", "DESIGNOS_IN_PROGRESS_STALE_MINUTES = \"<MINUTES>\"");
        let golden = "[mcp_servers.designos]\ncommand = \"<MCP_BIN>\"\nargs = [\"--workspace\", \"<WORKSPACE>\"]\nenabled = true\n\n[mcp_servers.designos.env]\nDESIGNOS_AGENT_RUNTIME = \"claude\"\nDESIGNOS_AGENT_RUN_ID = \"<SESSION_ID>-1-1\"\nDESIGNOS_CONTEXT_JSON = \"<SESSION>/context.json\"\nDESIGNOS_DOC_NAME = \"Untitled\"\nDESIGNOS_FILE = \"\"\nDESIGNOS_IN_PROGRESS_STALE_MINUTES = \"<MINUTES>\"\nDESIGNOS_LIVE_CANVAS = \"<SESSION>/canvas.designos\"\nDESIGNOS_LIVE_DOC_PATCH = \"<SESSION>/doc-patch.jsonl\"\nDESIGNOS_LIVE_DOC_SYNC = \"<SESSION>/doc-sync.designos\"\nDESIGNOS_LIVE_SPEC = \"<SESSION>/live-spec.json\"\nDESIGNOS_MCP_CONFIG = \"<SESSION>/mcp-config.json\"\nDESIGNOS_SESSION_DIR = \"<SESSION>\"\nDESIGNOS_TARGET_PAGE_ID = \"1\"\nDESIGNOS_TARGET_PAGE_NAME = \"Page 1\"";
        assert_eq!(toml.trim_end(), golden, "Grok {mode:?}: .grok/config.toml");
        let expected = expected_shape("/hooks", CLAUDE_EVENTS, CLAUDE_LEAVES);
        assert_eq!(shape_of(&read_json(&st, ".grok/hooks/designos-status.json")), expected, "Grok {mode:?}: the keys of the status hooks changed");
        finish(st);
    }
}

/// The paths of a Windows runner, as Studio writes them: plain, with every backslash doubled inside a JSON or TOML string,
/// verbatim (`\\?\`) after `canonicalize`, and joined with `\` where the goldens write `/`.
#[test]
fn windows_paths_are_normalized_in_every_spelling_they_are_written_in() {
    let session = r"C:\Users\runneradmin\AppData\Local\Temp\designos-sessions\p1-tab-2";
    let canonical = format!(r"\\?\{session}");
    let workspace = r"D:\a\design-os-studio\design-os-studio\crates\designos-app";
    let paths = vec![(canonical.clone(), "<SESSION>"), (session.to_string(), "<SESSION>"), (workspace.to_string(), "<WORKSPACE>")];
    let esc = |p: &str| p.replace('\\', "\\\\");

    let json = format!(
        r#"{{"args":["--workspace","{w}"],"env":{{"CTX":"{s}\\context.json","DIR":"{s}","ID":"p1-tab-2-1-1"}},"{c}":"TRUST_FOLDER","{s}":"TRUST_FOLDER"}}"#,
        w = esc(workspace),
        s = esc(session),
        c = esc(&canonical)
    );
    let got: Value = serde_json::from_str(&normalize_text(&json, &paths, "p1-tab-2")).expect("normalized JSON");
    let want = serde_json::json!({
        "args": ["--workspace", "<WORKSPACE>"],
        "env": { "CTX": "<SESSION>/context.json", "DIR": "<SESSION>", "ID": "<SESSION_ID>-1-1" },
        "<SESSION>": "TRUST_FOLDER",
    });
    assert_eq!(got, want);

    // TOML basic strings escape backslashes the same way.
    let toml = format!("args = [\"--workspace\", \"{w}\"]\nCTX = \"{s}\\\\context.json\"\n", w = esc(workspace), s = esc(session));
    assert_eq!(normalize_text(&toml, &paths, "p1-tab-2"), "args = [\"--workspace\", \"<WORKSPACE>\"]\nCTX = \"<SESSION>/context.json\"\n");
}

#[test]
fn unix_paths_are_normalized_as_before() {
    let paths = vec![("/tmp/designos-sessions/p1-tab-2".to_string(), "<SESSION>"), ("/work/crates/designos-app".to_string(), "<WORKSPACE>")];
    let text = r#"{"CTX":"/tmp/designos-sessions/p1-tab-2/context.json","W":"/work/crates/designos-app","ID":"p1-tab-2-1-1"}"#;
    assert_eq!(normalize_text(text, &paths, "p1-tab-2"), r#"{"CTX":"<SESSION>/context.json","W":"<WORKSPACE>","ID":"<SESSION_ID>-1-1"}"#);
}
