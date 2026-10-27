//! Grok and Antigravity (`agy`) run what the project folder of their working directory holds, as the user: Grok loads the hooks, MCP
//! servers, plugins, language servers and agent definitions there (Studio launches it with `GROK_FOLDER_TRUST=0`, which lifts the
//! trust gate that would otherwise stop them), and agy loads the hooks, MCP servers, plugins and subagent definitions of the `.agents`
//! folder that `--add-dir` names. That folder is the session folder, and an agent writes it. So an Ask-mode tab of either CLI starts
//! only when what the CLI would load there is exactly Studio's: every entry that is not Studio's is moved aside, Studio's own files are
//! rewritten and read back, and a failure of any step (a folder made read-only, a link where a folder should be, a file that is not
//! what was written) stops the tab. Nothing is deleted: the entries are kept in `.designos-set-aside` (see `SetAside`).
//!
//! Every access goes through the routed helpers of `agent_session_files`, which never follow a link below the session folder. This is
//! the check made once, as the tab starts; an agent that keeps writing to the folder afterwards is not stopped (#1048).
//!
//! Grok also sources `<session>/.envrc` with bash when a session starts (`xai-grok-workspace/src/envrc.rs`, called from
//! `xai-grok-shell/src/agent/mvp_agent/session_setup.rs:1142`; the folder-trust gate that `GROK_FOLDER_TRUST=0` lifts is its only guard,
//! `folder_trust.rs:497-501`). So `.envrc` is moved aside like the other sources, and every Grok tab, Ask or Skip, is launched with
//! `GROK_ENVRC_TIMEOUT_SECS=0` (`terminal.rs`), which disables every `.envrc` evaluation (`envrc.rs:160-163`) and so also covers a file
//! written after the start check. Whether agy sources an `.envrc` is unknown (its source is closed); Studio never writes one, so it is
//! moved aside for agy too.
//!
//! The files below are not moved, by the same public source (`xai-org/grok-build`, main `2bdd1d6a`):
//! - Grok's skills and commands (`.grok/skills`, `.claude/skills`, `.cursor/skills`, `.agents/skills`, `.grok/commands`): `allowed-tools`
//!   is parsed into the skill's data and shown in the extensions modal, and no code reads it to lift an approval; applying a skill
//!   substitutes `$ARGUMENTS`, `${SKILL_DIR}` and the like in its text and runs nothing (`xai-grok-tools/src/implementations/skills/skill.rs`,
//!   `apply_substitutions`).
//! - Grok's roles and personas (`.grok/roles/*.toml`, `.grok/personas/*.toml`): the fields are a description, a model, an effort, an
//!   isolation mode, a prompt file read as text and `default_capability_mode`, which is only a ceiling on the tool kinds a subagent gets
//!   (`xai-grok-subagent-resolution/src/config.rs:18-47`; `intersect_capability_modes` at `overrides.rs:22-37` never widens, and
//!   `handle_request.rs:778-782` intersects it with the agent definition's own mode). It filters which tools exist
//!   (`xai-grok-workspace/src/capability.rs:32`); approvals come from the agent definition's `permissionMode`
//!   (`handle_request.rs:1278-1296`), which is why the agent folders above are moved.

use crate::agent_session_files::{added_while_starting, set_aside_entries_except, set_aside_path, with_set_aside_note, write_session_file_verified, SetAside};
use std::io;
use std::path::Path;

/// A folder of the session whose entries are all Studio's or none: the names in `keep` stay, anything else is moved aside.
struct Folder {
    path: &'static str,
    keep: &'static [&'static str],
}

/// What a CLI loads from the working folder other than the files Studio writes: folders cleared of every entry but Studio's, and
/// paths that must not be there at all.
struct Sources {
    cli: &'static str,
    folders: &'static [Folder],
    absent: &'static [&'static str],
}

/// Grok's project sources that start a process: its hooks (`*.json` in the hooks folder), MCP servers (`.mcp.json`, and
/// `.cursor/mcp.json` through its compatibility layer), plugins, language servers (`.grok/lsp.json`), workflow scripts, agent
/// definitions (`.grok/agents`, `.claude/agents`: a project file shadows a built-in subagent such as `explore`, and may carry hooks,
/// `mcpServers` and `permissionMode: bypassPermissions`), the hooks it reads from a project's Claude and Cursor files, and the
/// `.envrc` it sources with bash. The project config, the one thing Studio writes here besides its hook file, is rewritten whole: it
/// holds only `[mcp_servers]`, so a `[plugins]` or `[permission]` section an agent added goes with it.
const GROK: Sources = Sources {
    cli: "Grok",
    folders: &[Folder { path: ".grok/hooks", keep: &["designos-status.json"] }],
    absent: &[
        ".mcp.json",
        ".grok/plugins",
        ".grok/lsp.json",
        ".grok/workflows",
        ".grok/agents",
        ".claude/agents",
        ".claude/settings.json",
        ".claude/settings.local.json",
        ".claude/plugins",
        ".cursor/hooks.json",
        ".cursor/mcp.json",
        ".envrc",
    ],
};

/// Antigravity's workspace sources that start a process: the hooks file, the workspace MCP file, every plugin (a plugin bundles its own
/// hooks and MCP servers), and subagent definitions (`.agents/agents`: they declare `mcpServers` and `commandExecutionPolicy`).
/// Studio's `designos` plugin keeps its two files and nothing else. `.envrc` is in the set in case agy sources it, as Grok does.
const AGY: Sources = Sources {
    cli: "Antigravity",
    folders: &[Folder { path: ".agents/plugins", keep: &["designos"] }, Folder { path: ".agents/plugins/designos", keep: &["plugin.json", "mcp_config.json"] }],
    absent: &[".agents/mcp_config.json", ".agents/agents", ".envrc"],
};

impl Sources {
    /// Moves every entry of the CLI's sources that is not Studio's aside.
    fn clear(&self, session: &Path, aside: &mut SetAside) -> io::Result<()> {
        for folder in self.folders {
            set_aside_entries_except(&session.join(folder.path), folder.keep, aside)?;
        }
        for rel in self.absent {
            set_aside_path(&session.join(rel), aside)?;
        }
        Ok(())
    }

    /// Clears the sources, runs `write` (Studio's own files), and clears them once more: an entry that came back while Studio was
    /// writing (another agent tab put it there) is set aside too and stops the tab. What to tell the user comes back, with the error
    /// too when the start fails after entries were moved.
    fn make_exact(&self, session: &Path, write: impl FnOnce() -> io::Result<()>) -> io::Result<Option<String>> {
        let mut aside = SetAside::new(session, self.cli);
        match self.clear_write_clear(session, &mut aside, write) {
            Ok(()) => Ok(aside.note()),
            Err(e) => Err(match aside.note() {
                Some(note) => with_set_aside_note(e, note),
                None => e,
            }),
        }
    }

    fn clear_write_clear(&self, session: &Path, aside: &mut SetAside, write: impl FnOnce() -> io::Result<()>) -> io::Result<()> {
        self.clear(session, aside)?;
        write()?;
        let first_pass = aside.moved().len();
        self.clear(session, aside)?;
        match aside.moved().get(first_pass) {
            Some(came_back) => Err(added_while_starting(&session.join(came_back), self.cli)),
            None => Ok(()),
        }
    }
}

/// Makes what Grok loads from `session` exactly Studio's: `config_toml` as the project config, `hooks_json` as the one hook file, and
/// none of the other sources of [`GROK`]. Comes back with what to tell the user when entries were moved aside.
pub(crate) fn make_grok_config_exact(session: &Path, config_toml: &[u8], hooks_json: &[u8]) -> io::Result<Option<String>> {
    GROK.make_exact(session, || {
        write_session_file_verified(&session.join(".grok/config.toml"), config_toml)?;
        write_session_file_verified(&session.join(".grok/hooks/designos-status.json"), hooks_json)
    })
}

/// Makes what Antigravity loads from `session` exactly Studio's: `hooks_json` as the hooks file, the `designos` plugin made of
/// `plugin_manifest` and `mcp_config`, and none of the other sources of [`AGY`]. Comes back with what to tell the user when entries
/// were moved aside.
pub(crate) fn make_agy_config_exact(session: &Path, hooks_json: &[u8], plugin_manifest: &[u8], mcp_config: &[u8]) -> io::Result<Option<String>> {
    AGY.make_exact(session, || {
        write_session_file_verified(&session.join(".agents/plugins/designos/plugin.json"), plugin_manifest)?;
        write_session_file_verified(&session.join(".agents/plugins/designos/mcp_config.json"), mcp_config)?;
        write_session_file_verified(&session.join(".agents/hooks.json"), hooks_json)
    })
}
