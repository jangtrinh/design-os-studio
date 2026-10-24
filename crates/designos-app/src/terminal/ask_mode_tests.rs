//! In Ask mode nothing Studio writes or passes auto-approves a `designos` tool call, for every agent CLI;
//! Skip mode keeps the CLI's own bypass. One test per agent checks the launch commands (new session and
//! resume) and every config file Studio writes for that CLI, in both modes; `golden` pins the full values.
//!
//! Why the Gemini file matters: in gemini-cli v0.35.3 an MCP server entry with `"trust": true` becomes an ALLOW policy
//! rule for all of its tools, with no check of the folder trust (`packages/core/src/policy/config.ts:484-500`). Studio
//! never writes it (see `write_gemini_session_config`).

use super::*;
use crate::agent_permissions::{build_new_session_command_for_mode, build_resume_command_for_mode};
use crate::agent_sessions::{SessionEntry, SessionEntryStatus};
use serde_json::Value;

mod golden;

const UUID: &str = "11111111-2222-4333-8444-555555555555";

/// Config keys that make an agent CLI run a tool without asking, in any of the CLIs' formats.
const AUTO_APPROVE_KEYS: &[&str] = &[
    "trust",
    "autoapprove",
    "auto_approve",
    "autoaccept",
    "alwaysallow",
    "always_allow",
    "allowed",
    "allowedtools",
    "permissions",
    "permission",
    "allow",
    "approval_mode",
    "defaultapprovalmode",
    "default_tools_approval_mode",
    "approval_policy",
];

/// A window whose agent files were written for an agent started in `mode`.
fn session_in(mode: PermissionMode) -> TerminalState {
    let mut st = TerminalState::for_tab(crate::live_sync_tests::unique_tab_id());
    let _ = std::fs::remove_dir_all(&st.session_dir);
    std::fs::create_dir_all(&st.session_dir).expect("session dir");
    st.permission_mode = mode;
    st.provision_session_agent_files(None);
    st
}

fn finish(st: TerminalState) {
    crate::agent_approval_channel::remove_channel(&st.session_dir);
    let _ = std::fs::remove_dir_all(&st.session_dir);
}

fn entry(cli: &str, session_id: Option<&str>) -> SessionEntry {
    SessionEntry {
        tab_id: 1,
        title: format!("{cli} #1"),
        cli: cli.into(),
        session_id: session_id.map(str::to_string),
        created_at: 1,
        last_active_at: 2,
        status: SessionEntryStatus::Exited,
        extra: Default::default(),
    }
}

/// Every command Studio can start `preset` with in `mode`: a new session, one with a session id, and a resume.
fn launch_commands(preset: AgentCliPreset, cli: &str, mode: PermissionMode) -> Vec<String> {
    let mut out = vec![build_new_session_command_for_mode(preset, mode, None), build_new_session_command_for_mode(preset, mode, Some(UUID))];
    for session_id in [Some(UUID), None] {
        out.extend(build_resume_command_for_mode(&entry(cli, session_id), true, true, mode));
    }
    out
}

/// The JSON file `rel` of the session folder. `hooks.json` is Claude's hooks, wherever Studio wrote them for the mode: in
/// the session folder in Skip mode, in the Studio-private folder in Ask mode.
fn read_json(st: &TerminalState, rel: &str) -> Value {
    let path = if rel == "hooks.json" { crate::agent_permissions::claude_hooks_path(st) } else { st.session_dir.join(rel) };
    let raw = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{rel}: {e}"));
    serde_json::from_str(&raw).unwrap_or_else(|e| panic!("{rel} is not JSON: {e}"))
}

fn read_json_at(path: &Path) -> Value {
    let raw = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    serde_json::from_str(&raw).unwrap_or_else(|e| panic!("{} is not JSON: {e}", path.display()))
}

/// Paths of every key in `value` whose name is one of [`AUTO_APPROVE_KEYS`] and whose value is not an empty, false or
/// null placeholder.
fn auto_approve_keys(value: &Value, path: &str, found: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                let here = format!("{path}/{key}");
                let inert = matches!(child, Value::Null | Value::Bool(false)) || child.as_object().is_some_and(|o| o.is_empty());
                if AUTO_APPROVE_KEYS.contains(&key.to_ascii_lowercase().as_str()) && !inert {
                    found.push(here.clone());
                }
                auto_approve_keys(child, &here, found);
            }
        }
        Value::Array(items) => items.iter().enumerate().for_each(|(i, child)| auto_approve_keys(child, &format!("{path}[{i}]"), found)),
        _ => {}
    }
}

fn assert_no_auto_approve_key(st: &TerminalState, rel: &str, who: &str) {
    let mut found = Vec::new();
    auto_approve_keys(&read_json(st, rel), rel, &mut found);
    assert!(found.is_empty(), "{who}: {rel} carries auto-approve settings in Ask mode: {found:?}");
}

/// The skill Studio wrote into the private plugin of an Ask-mode session after the window showed a document and a page named
/// as given. The plugin is what Ask-mode Claude loads Studio's skill from, so its frontmatter decides which tools the skill
/// may run without asking and which hooks it registers.
fn private_plugin_skill_after_showing(doc_name: &str, page_name: &str) -> String {
    let mut st = session_in(PermissionMode::Ask);
    show(&mut st, doc_name, page_name);
    let plugin = crate::agent_approval_channel::claude_plugin_dir_path(&st.session_dir).expect("a private channel folder");
    let skill_path = plugin.join(crate::agent_permissions::CLAUDE_PLUGIN_SKILL_PATH);
    let skill = std::fs::read_to_string(&skill_path).unwrap_or_else(|e| panic!("{}: {e}", skill_path.display()));
    finish(st);
    skill
}

/// Shows `doc_name` and a page named `page_name` in the window, as the live sync does, and waits for the files to be written.
fn show(st: &mut TerminalState, doc_name: &str, page_name: &str) {
    let mut doc = designos_doc::Document::new(doc_name);
    crate::io::ensure_page(&mut doc);
    let page = doc.pages[0];
    doc.get_mut(page).expect("the page").name = page_name.to_string();
    st.sync_live_file_context(&doc, page, &[], None);
    st.flush_live_sync();
}

/// Files Studio writes that no CLI loads by itself: the document's own data, which the agent reads on request or a hook prints
/// (they carry its text by design), and bookkeeping that changes on every write.
const DOCUMENT_DATA_FILES: &[&str] = &[
    "canvas.designos",
    "doc-sync.designos",
    "doc-patch.jsonl",
    "context.json",
    "bootstrap.md",
    "bootstrap.json",
    "bootstrap-context-reply.json",
    "bootstrap-compact-reply.json",
    "page-context-reply.json",
    "selection.spec.json",
    "live-spec.json",
    "owner.json",
];

/// Every file in the session folder and in the private channel folder that a CLI can load by itself: the skill copies, the
/// memory files (`AGENTS.md`, `GEMINI.md`), the MCP and hook configs, the private plugin. Keyed by where it is, valued by its
/// bytes. A file that is in neither [`DOCUMENT_DATA_FILES`] nor a per-tab request folder counts, so a new file that carries a
/// name fails the test that compares them.
fn files_a_cli_loads_by_itself(st: &TerminalState) -> std::collections::BTreeMap<String, Vec<u8>> {
    fn walk(root: &Path, dir: &Path, label: &str, out: &mut std::collections::BTreeMap<String, Vec<u8>>) {
        // (`root` is the folder the keys are relative to; `dir` is the one being read.)
        for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            if path.is_dir() {
                if !name.starts_with("tab-") {
                    walk(root, &path, label, out);
                }
            } else if !DOCUMENT_DATA_FILES.contains(&name.as_str()) && !name.contains(".tmp.") {
                let rel = path.strip_prefix(root).expect("under the root").to_string_lossy().replace('\\', "/");
                out.insert(format!("{label}/{rel}"), std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display())));
            }
        }
    }
    let mut out = std::collections::BTreeMap::new();
    walk(&st.session_dir, &st.session_dir, "session", &mut out);
    if let Some(private) = crate::agent_approval_channel::mcp_config_path(&st.session_dir).and_then(|p| p.parent().map(Path::to_path_buf)) {
        walk(&private, &private, "private", &mut out);
    }
    out
}

/// The files of `now` that are missing from, extra to or different from `before`.
fn changed_files(before: &std::collections::BTreeMap<String, Vec<u8>>, now: &std::collections::BTreeMap<String, Vec<u8>>) -> Vec<String> {
    before.keys().chain(now.keys()).collect::<std::collections::BTreeSet<_>>().into_iter().filter(|k| before.get(*k) != now.get(*k)).cloned().collect()
}

/// A document or page name is text that whoever wrote the document chose (a third-party file, or an agent that wrote the
/// document through the session folder). Claude acts on frontmatter keys, `!` placeholders and `@` paths in a skill, and
/// Gemini imports `@` paths from `GEMINI.md`, each when it loads the file. So no file a CLI loads by itself carries a name: the
/// files are the same bytes whatever the open file and its page are called.
#[test]
fn what_each_cli_loads_by_itself_is_the_same_whatever_the_document_and_page_are_named() {
    let mut st = session_in(PermissionMode::Ask);
    show(&mut st, "Baseline doc", "Baseline page");
    let baseline = files_a_cli_loads_by_itself(&st);
    for expected in [
        "session/AGENTS.md",
        "session/GEMINI.md",
        "session/.claude/skills/designos-studio/SKILL.md",
        "session/.agents/skills/designos-studio/SKILL.md",
        "session/.gemini/settings.json",
        "session/.grok/config.toml",
        "session/.agents/plugins/designos/mcp_config.json",
        "session/mcp-config.json",
    ] {
        assert!(baseline.contains_key(expected), "{expected} is not among the compared files: {:?}", baseline.keys().collect::<Vec<_>>());
    }
    assert!(baseline.keys().any(|k| k.starts_with("private/") && k.ends_with("SKILL.md")), "the private plugin skill is compared");

    let mut previous_context = std::fs::read(&st.context_json_path).expect("context.json");
    for &name in crate::live_sync_tests::HOSTILE_NAMES {
        for (doc, page) in [(name, "Page 1"), ("Kit", name)] {
            show(&mut st, doc, page);
            let context = std::fs::read(&st.context_json_path).expect("context.json");
            assert_ne!(context, previous_context, "the sync did not run for {doc:?} / {page:?}, so nothing was compared");
            previous_context = context;
            let changed = changed_files(&baseline, &files_a_cli_loads_by_itself(&st));
            assert!(changed.is_empty(), "files a CLI loads by itself differ for the document {doc:?} / page {page:?}: {changed:?}");
        }
    }
    finish(st);
}

/// The private plugin is what Ask-mode Claude loads Studio's skill from. Its frontmatter is a name and a fixed description, and
/// its text refers to the open file and page without naming them.
#[test]
fn the_private_plugin_skill_has_a_fixed_frontmatter_and_names_neither_the_file_nor_its_page() {
    let skill = private_plugin_skill_after_showing("Checkout: v2", "Step 2: pay");
    crate::live_sync_tests::assert_skill_frontmatter_is_name_and_fixed_description(&skill);
    assert!(skill.contains("the open file (`$DESIGNOS_DOC_NAME`; call `designos_get_app_state` for its name and pages)"), "{skill}");
    assert!(skill.contains("the current page (`NodeId("), "{skill}");
    assert!(!skill.contains("Checkout") && !skill.contains("Step 2"), "a name reached the skill: {skill}");
}

#[test]
fn the_scanner_flags_every_auto_approve_shape_and_ignores_inert_ones() {
    let bad = serde_json::json!({
        "mcpServers": { "designos": { "trust": true, "alwaysAllow": ["designos_apply_spec"], "nested": [{ "autoApprove": true }] } },
        "permissions": { "allow": ["mcp__designos"] },
        "tools": { "allowed": ["designos_apply_spec"], "autoAccept": true },
    });
    let mut found = Vec::new();
    auto_approve_keys(&bad, "", &mut found);
    assert_eq!(found.len(), 7, "trust, alwaysAllow, autoApprove, permissions, permissions/allow, tools/allowed and tools/autoAccept: {found:?}");

    let inert = serde_json::json!({ "mcpServers": { "designos": { "trust": false, "env": { "DESIGNOS_FILE": "" } } }, "permissions": {} });
    let mut found = Vec::new();
    auto_approve_keys(&inert, "", &mut found);
    assert!(found.is_empty(), "a false or empty placeholder grants nothing: {found:?}");
}

#[test]
fn claude_ask_launches_without_the_skip_flag_and_writes_no_allow_rule() {
    for cmd in launch_commands(AgentCliPreset::Claude, "claude", PermissionMode::Ask) {
        assert!(!cmd.contains("--dangerously-skip-permissions") && !cmd.contains("--permission-mode"), "Claude Ask must start in the default mode: {cmd}");
        assert!(cmd.contains("--mcp-config") && cmd.contains("--settings"), "Claude still loads the designos server and hooks: {cmd}");
    }
    for cmd in launch_commands(AgentCliPreset::Claude, "claude", PermissionMode::Skip) {
        assert!(cmd.contains("--dangerously-skip-permissions"), "Claude Skip keeps the bypass flag: {cmd}");
    }

    let ask = session_in(PermissionMode::Ask);
    let hooks = read_json(&ask, "hooks.json");
    assert_no_auto_approve_key(&ask, "hooks.json", "Claude");
    assert_no_auto_approve_key(&ask, "mcp-config.json", "Claude");
    let approve = hooks["hooks"]["PermissionRequest"][0]["hooks"]
        .as_array()
        .expect("PermissionRequest handlers")
        .iter()
        .any(|h| h["command"].as_str().is_some_and(|c| c.contains("approve-hook.sh")));
    assert!(approve, "Claude Ask routes each permission request through the in-panel card: {hooks}");
    for settings in [".claude/settings.json", ".claude/settings.local.json"] {
        assert!(!ask.session_dir.join(settings).exists(), "Studio writes no Claude settings with an allow list: {settings}");
    }
    finish(ask);

    let skip = session_in(PermissionMode::Skip);
    let hooks = read_json(&skip, "hooks.json");
    let approve = hooks["hooks"]["PermissionRequest"][0]["hooks"]
        .as_array()
        .expect("PermissionRequest handlers")
        .iter()
        .any(|h| h["command"].as_str().is_some_and(|c| c.contains("approve-hook.sh")));
    assert!(!approve, "Claude Skip has no approval card: {hooks}");
    finish(skip);
}

#[test]
fn codex_ask_launches_without_the_bypass_flag_and_sets_no_approval_override() {
    for cmd in launch_commands(AgentCliPreset::Codex, "codex", PermissionMode::Ask) {
        assert!(!cmd.contains("--dangerously-bypass-approvals-and-sandbox"), "Codex Ask must not bypass approvals: {cmd}");
        for word in ["approval", "trust", "--ask-for-approval", " -a ", " -s ", "--sandbox"] {
            assert!(!cmd.contains(word), "Codex Ask passes no approval or sandbox override ({word}): {cmd}");
        }
        assert!(cmd.contains("mcp_servers.designos.command="), "Codex still registers the designos server: {cmd}");
    }
    let new = build_new_session_command_for_mode(AgentCliPreset::Codex, PermissionMode::Ask, None);
    assert!(new.starts_with("codex -c 'mcp_servers.designos.command="), "Codex Ask is the plain `codex` plus the server overrides: {new}");
    let resume = build_resume_command_for_mode(&entry("codex", None), true, true, PermissionMode::Ask).expect("codex resume");
    assert!(resume.starts_with("codex resume --last -c 'mcp_servers.designos.command="), "Codex Ask resume: {resume}");

    for cmd in launch_commands(AgentCliPreset::Codex, "codex", PermissionMode::Skip) {
        assert!(cmd.contains("--dangerously-bypass-approvals-and-sandbox"), "Codex Skip keeps the bypass flag: {cmd}");
    }

    for mode in [PermissionMode::Ask, PermissionMode::Skip] {
        let st = session_in(mode);
        for config in [".codex", "config.toml", ".codex/config.toml"] {
            assert!(!st.session_dir.join(config).exists(), "Codex reads no session config file, so none may carry approval keys: {config}");
        }
        assert_no_auto_approve_key(&st, "mcp-config.json", "Codex");
        finish(st);
    }
}

#[test]
fn gemini_launches_in_the_mode_asked_for_and_never_trusts_the_server() {
    for cmd in launch_commands(AgentCliPreset::Gemini, "gemini", PermissionMode::Ask) {
        assert!(
            cmd.contains("--approval-mode default") && !cmd.contains("yolo") && !cmd.contains("--yolo"),
            "Gemini Ask uses the default approval mode: {cmd}"
        );
    }
    for cmd in launch_commands(AgentCliPreset::Gemini, "gemini", PermissionMode::Skip) {
        assert!(cmd.contains("--approval-mode yolo"), "Gemini Skip keeps yolo: {cmd}");
    }

    for mode in [PermissionMode::Ask, PermissionMode::Skip] {
        let st = session_in(mode);
        let settings = read_json(&st, ".gemini/settings.json");
        assert!(settings["mcpServers"]["designos"]["command"].as_str().is_some(), "Gemini still loads the designos server: {settings}");
        assert!(
            settings["mcpServers"]["designos"].get("trust").is_none(),
            "Gemini {mode:?} must not trust the server (trust is an allow rule for all of its tools): {settings}"
        );
        assert_no_auto_approve_key(&st, ".gemini/settings.json", "Gemini");
        // The folder stays trusted: without it Gemini ignores `<session_dir>/.gemini/settings.json` and never starts the
        // server. A trusted folder adds no allow rule, so tool calls still ask in Ask mode.
        let trusted = read_json(&st, ".gemini/trustedFolders.json");
        assert_eq!(
            trusted[st.session_dir.display().to_string()].as_str(),
            Some("TRUST_FOLDER"),
            "the session folder stays trusted so its settings load: {trusted}"
        );
        finish(st);
    }
}

#[test]
fn gemini_settings_stay_untrusted_when_the_mode_changes_under_a_running_tab() {
    // One settings file serves every Gemini tab of the window and is rewritten on each sync with the window's mode. A tab
    // that launched in Ask must not meet a trusted server because the user later switched the window to Skip.
    let mut st = session_in(PermissionMode::Ask);
    for mode in [PermissionMode::Skip, PermissionMode::Ask, PermissionMode::Skip] {
        st.permission_mode = mode;
        st.provision_session_agent_files(None);
        let settings = read_json(&st, ".gemini/settings.json");
        assert!(settings["mcpServers"]["designos"].get("trust").is_none(), "after switching to {mode:?}: {settings}");
    }
    finish(st);
}

#[test]
fn antigravity_ask_launches_without_the_skip_flag_and_its_plugin_config_trusts_nothing() {
    for cmd in launch_commands(AgentCliPreset::Antigravity, "agy", PermissionMode::Ask) {
        assert!(!cmd.contains("--dangerously-skip-permissions") && !cmd.contains("--mode"), "agy Ask starts in its default mode: {cmd}");
        assert!(cmd.contains("--add-dir"), "agy still discovers the session plugin through --add-dir: {cmd}");
    }
    for cmd in launch_commands(AgentCliPreset::Antigravity, "agy", PermissionMode::Skip) {
        assert!(cmd.contains("--dangerously-skip-permissions"), "agy Skip keeps the bypass flag: {cmd}");
    }

    for mode in [PermissionMode::Ask, PermissionMode::Skip] {
        let st = session_in(mode);
        assert_no_auto_approve_key(&st, ".agents/plugins/designos/mcp_config.json", "agy");
        assert_no_auto_approve_key(&st, ".agents/plugins/designos/plugin.json", "agy");
        // Its hooks answer `{}`: they never return a permission decision.
        let hooks = std::fs::read_to_string(st.session_dir.join(".agents/hooks.json")).expect("agy hooks");
        assert!(!hooks.contains("allow") && !hooks.contains("permissionDecision"), "agy hooks never allow a tool: {hooks}");
        finish(st);
    }
}

#[test]
fn grok_ask_launches_without_always_approve_and_writes_no_permission_rule() {
    for cmd in launch_commands(AgentCliPreset::Grok, "grok", PermissionMode::Ask) {
        assert!(!cmd.contains("--always-approve") && !cmd.contains("--yolo") && !cmd.contains("--trust"), "Grok Ask must ask for each tool: {cmd}");
        assert!(cmd.contains("--cwd \"$DESIGNOS_SESSION_DIR\""), "Grok still reads its project config from the session folder: {cmd}");
    }
    for cmd in launch_commands(AgentCliPreset::Grok, "grok", PermissionMode::Skip) {
        assert!(cmd.contains("--always-approve"), "Grok Skip keeps always-approve: {cmd}");
    }

    for mode in [PermissionMode::Ask, PermissionMode::Skip] {
        let st = session_in(mode);
        let toml = std::fs::read_to_string(st.session_dir.join(".grok/config.toml")).expect(".grok/config.toml");
        let parsed: toml::Value = toml::from_str(&toml).expect("valid TOML");
        let server = &parsed["mcp_servers"]["designos"];
        assert!(server.get("command").is_some() && server["enabled"].as_bool() == Some(true), "Grok still loads the designos server: {toml}");
        // Folder trust is off for the process (GROK_FOLDER_TRUST=0, set below) so project config loads; that un-gates project
        // permission rules, so the file must hold none and the session folder no Claude-style settings either.
        assert!(parsed.get("permission").is_none(), "no [permission] rules in the session config: {toml}");
        assert!(parsed.get("permissions").is_none() && server.get("trust").is_none() && server.get("auto_approve").is_none(), "{toml}");
        assert!(!st.session_dir.join(".claude/settings.json").exists() && !st.session_dir.join(".claude/settings.local.json").exists());
        let hooks = std::fs::read_to_string(st.session_dir.join(".grok/hooks/designos-status.json")).expect("grok hooks");
        assert!(!hooks.contains("\"allow\"") && !hooks.contains("approve-hook"), "Grok hooks never allow a tool: {hooks}");

        let mut cmd = CommandBuilder::new("grok");
        configure_pty_env_with_session(&mut cmd, AgentCliPreset::Grok, None, Some(&st.session_env()));
        assert_eq!(cmd.get_env("GROK_FOLDER_TRUST").and_then(|v| v.to_str()), Some("0"), "folder trust is off in {mode:?} so the project MCP config loads");
        finish(st);
    }
}

#[test]
fn a_tab_started_without_a_prepared_command_asks() {
    let agents =
        [AgentCliPreset::Claude, AgentCliPreset::Codex, AgentCliPreset::Antigravity, AgentCliPreset::Grok, AgentCliPreset::Gemini, AgentCliPreset::OpenCode];
    for preset in agents {
        let fallback = fallback_agent_launch_command(preset);
        assert_eq!(fallback, preset.cli_launch_command_for_mode(PermissionMode::Ask), "{preset:?}: the fallback is the Ask command");
        for flag in ["--dangerously-skip-permissions", "--dangerously-bypass-approvals-and-sandbox", "--always-approve", "--approval-mode yolo"] {
            assert!(!fallback.contains(flag), "{preset:?}: a caller that passed no command must never start an agent that skips approvals: {fallback}");
        }
    }
}
