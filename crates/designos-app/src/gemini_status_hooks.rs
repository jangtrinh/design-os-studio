//! Status hooks for Gemini CLI in Ask mode.
//!
//! Gemini fires a `Notification` hook (`notification_type: "ToolPermission"`) when it shows a tool-confirmation prompt, and
//! lifecycle hooks around each prompt and tool call. Studio turns them into the tab's "Needs approval" state, turn states
//! and receipts, the way it does for Claude and Grok.
//!
//! Where they live: Gemini merges `hooks` arrays across its settings layers by concatenation, and the layer with the lowest
//! precedence is the system-defaults file, whose location `GEMINI_CLI_SYSTEM_DEFAULTS_PATH` names. So each Gemini tab gets
//! its own file in the Studio-private channel folder (`<channel>/gemini-tab-<id>.json`, 0600) and that variable points at it:
//! nothing is added to the session folder, which an agent can write, and the user's own settings and hooks stay as they are.
//!
//! The variable replaces whatever system defaults the machine has, so nothing is written when Gemini already has some of its
//! own: Studio's environment names that file, or a `system-defaults.json` sits where Gemini looks for it. Nothing is written
//! off Unix either: Gemini runs a hook through PowerShell on Windows, which cannot parse the POSIX command the handlers use.
//! The tab then starts its waits from the confirmation dialog on its screen instead (`gemini_dialog`).
//!
//! Gemini fires none of these hooks when the user answers No in its own confirmation dialog, so the dialog on the tab's screen
//! also decides when a wait ends (`gemini_dialog`).

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use crate::agent_permissions::AgentPermissionMode;
use crate::terminal::AgentCliPreset;

/// Environment variable that names Gemini's system-defaults file (the lowest-precedence settings layer).
pub const SYSTEM_DEFAULTS_ENV: &str = "GEMINI_CLI_SYSTEM_DEFAULTS_PATH";

/// Environment variable that names Gemini's system settings file. The system-defaults file defaults to a sibling of it.
const SYSTEM_SETTINGS_ENV: &str = "GEMINI_CLI_SYSTEM_SETTINGS_PATH";

/// Hook name Gemini lists the handler under.
const HOOK_NAME: &str = "designos-status";

/// How long Gemini lets a handler run, in milliseconds. The handler only appends one line to a file.
const HOOK_TIMEOUT_MS: u64 = 5000;

/// `(Gemini event, record label the turn tracker reads, legacy status word)`.
pub const GEMINI_HOOK_EVENTS: &[(&str, &str, &str)] = &[
    ("BeforeAgent", "UserPromptSubmit", "working"),
    ("BeforeTool", "PreToolUse", "working"),
    ("AfterTool", "PostToolUse", "working"),
    ("Notification", "Notification", "waiting"),
    ("AfterAgent", "Stop", "done"),
];

/// `<channel>/gemini-tab-<id>.json` of `session_dir`: the system-defaults file one Gemini tab reads.
pub fn tab_file_path(session_dir: &Path, tab_id: usize) -> Option<PathBuf> {
    crate::agent_approval_channel::channel_dir(session_dir).map(|dir| dir.join(format!("gemini-tab-{tab_id}.json")))
}

/// Gemini's settings shape for the status hooks of tab `tab_id`: one handler per event, each appending a record that
/// names the tab to the events file of the hook folder `paths` (`agent_hook_outputs`). A handler prints nothing, as Gemini reads any
/// output of a hook as JSON.
pub fn hooks_settings(paths: &crate::agent_hook_outputs::HookPaths, tab_id: usize) -> serde_json::Value {
    let hooks: serde_json::Map<String, serde_json::Value> = GEMINI_HOOK_EVENTS
        .iter()
        .map(|&(event, label, state)| {
            let command = crate::terminal::status_hook_command_for_tab(paths, label, state, Some(tab_id));
            let handler = serde_json::json!({ "type": "command", "name": HOOK_NAME, "command": command, "timeout": HOOK_TIMEOUT_MS });
            (event.to_string(), serde_json::json!([{ "matcher": "*", "hooks": [handler] }]))
        })
        .collect();
    serde_json::json!({ "hooks": hooks })
}

/// The folder Gemini reads its admin-owned files from on this platform (`getSystemSettingsPath` in Gemini CLI). Unit tests
/// never look at the real one: they name their own through [`with_system_config`].
#[cfg(not(test))]
fn platform_system_dir() -> PathBuf {
    if cfg!(target_os = "macos") {
        PathBuf::from("/Library/Application Support/GeminiCli")
    } else if cfg!(windows) {
        PathBuf::from(r"C:\ProgramData\gemini-cli")
    } else {
        PathBuf::from("/etc/gemini-cli")
    }
}

/// True when Gemini already has system defaults that setting `GEMINI_CLI_SYSTEM_DEFAULTS_PATH` would replace. It follows
/// Gemini's own lookup (`getSystemDefaultsPath`): the variable if it is set, else `system-defaults.json` beside the system
/// settings file, which the other variable or the platform folder names.
fn system_defaults_in_force(env: impl Fn(&str) -> Option<OsString>, platform_dir: &Path) -> bool {
    let set = |name: &str| env(name).filter(|value| !value.is_empty());
    if set(SYSTEM_DEFAULTS_ENV).is_some() {
        return true;
    }
    let settings = set(SYSTEM_SETTINGS_ENV).map(PathBuf::from).unwrap_or_else(|| platform_dir.join("settings.json"));
    settings.parent().is_some_and(|dir| dir.join("system-defaults.json").symlink_metadata().is_ok())
}

#[cfg(test)]
type Probe = (std::collections::HashMap<String, OsString>, PathBuf);

#[cfg(test)]
thread_local! {
    static PROBE: std::cell::RefCell<Option<Probe>> = const { std::cell::RefCell::new(None) };
}

/// Runs `body` on this thread with Studio's environment holding `env` and the platform folder at `platform_dir`, instead of
/// the machine's real ones. Outside it a test sees no variables and a platform folder that does not exist.
#[cfg(test)]
pub fn with_system_config<R>(env: &[(&str, &str)], platform_dir: &Path, body: impl FnOnce() -> R) -> R {
    let probe = (env.iter().map(|&(name, value)| (name.to_string(), OsString::from(value))).collect(), platform_dir.to_path_buf());
    PROBE.with(|slot| *slot.borrow_mut() = Some(probe));
    struct Restore;
    impl Drop for Restore {
        fn drop(&mut self) {
            PROBE.with(|slot| *slot.borrow_mut() = None);
        }
    }
    let _restore = Restore;
    body()
}

fn defaults_in_force_here() -> bool {
    #[cfg(test)]
    {
        PROBE.with(|slot| match slot.borrow().as_ref() {
            Some((env, dir)) => system_defaults_in_force(|name| env.get(name).cloned(), dir),
            None => system_defaults_in_force(|_| None, &std::env::temp_dir().join("designos-no-gemini-system-config")),
        })
    }
    #[cfg(not(test))]
    system_defaults_in_force(|name| std::env::var_os(name), &platform_system_dir())
}

/// Removes the status-hooks file of tab `tab_id`, if any: a closed tab's, or an earlier run's with the same id.
pub fn remove_tab_file(session_dir: &Path, tab_id: usize) {
    if let Some(path) = tab_file_path(session_dir, tab_id) {
        let _ = std::fs::remove_file(path);
    }
}

/// Writes the status-hooks file of a Gemini tab started in `mode` and returns its path, the value for
/// [`SYSTEM_DEFAULTS_ENV`]. `None` when the tab is not a Gemini tab in Ask mode, when Gemini has system defaults of its own,
/// when the hooks have no folder to report to (`HookPaths::prepare`), or when the file
/// cannot be written; the caller then leaves the tab's environment alone and the tab reads its waits from its
/// screen. Written before the tab starts, so the variable never names a file that is not there.
pub fn prepare_for_tab(session_dir: &Path, tab_id: usize, mode: AgentPermissionMode, preset: AgentCliPreset) -> Option<PathBuf> {
    prepare_for_tab_on(cfg!(unix), session_dir, tab_id, mode, preset)
}

/// [`prepare_for_tab`] with the platform's shell named: `posix_shell` is false where Gemini runs a hook through PowerShell,
/// which cannot parse the POSIX command the handlers use.
pub fn prepare_for_tab_on(posix_shell: bool, session_dir: &Path, tab_id: usize, mode: AgentPermissionMode, preset: AgentCliPreset) -> Option<PathBuf> {
    if preset != AgentCliPreset::Gemini {
        return None;
    }
    remove_tab_file(session_dir, tab_id);
    if mode != AgentPermissionMode::Ask || !posix_shell {
        return None;
    }
    if defaults_in_force_here() {
        eprintln!(
            "design:os studio: Gemini CLI has system defaults of its own, so its status hooks are off; the tab reads its approvals from the dialog on its screen"
        );
        return None;
    }
    let channel = match crate::agent_approval_channel::prepare_channel_dir(session_dir) {
        Ok(dir) => dir,
        Err(e) => {
            eprintln!("design:os studio: Gemini CLI status hooks are off for this session: {e}");
            return None;
        }
    };
    let hook_paths = match crate::agent_hook_outputs::HookPaths::prepare(session_dir) {
        Ok(paths) => paths,
        Err(e) => {
            eprintln!("design:os studio: Gemini CLI status hooks are off for this session, no hook can report: {e}");
            return None;
        }
    };
    let path = channel.join(format!("gemini-tab-{tab_id}.json"));
    let written = serde_json::to_vec_pretty(&hooks_settings(&hook_paths, tab_id))
        .map_err(std::io::Error::other)
        .and_then(|bytes| crate::agent_approval_channel::write_owner_only_file(&path, &bytes, false));
    match written {
        Ok(()) => Some(path),
        Err(e) => {
            eprintln!("design:os studio: could not write Gemini CLI's status hooks to {}: {e}", path.display());
            None
        }
    }
}
