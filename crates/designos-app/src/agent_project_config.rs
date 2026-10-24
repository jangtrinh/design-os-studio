//! Grok and Antigravity (`agy`) run what the project folder of their working directory holds, as the user: Grok loads the hooks, MCP
//! servers, plugins and language servers there (Studio launches it with `GROK_FOLDER_TRUST=0`, which lifts the trust gate that would
//! otherwise stop them), and agy loads the hooks, MCP servers and plugins of the `.agents` folder that `--add-dir` names. That folder is
//! the session folder, and an agent writes it. So an Ask-mode tab of either CLI starts only when what the CLI would load there is
//! exactly Studio's: the entries that are not Studio's are removed, Studio's own files are rewritten and read back, and a failure of
//! any step (a folder made read-only, a link where a folder should be, a file that is not what was written) stops the tab.
//!
//! Every access goes through the routed helpers of `agent_session_files`, which never follow a link below the session folder. This is
//! the check made once, as the tab starts; what an agent does to these files afterwards is not covered (#1048).

use crate::agent_session_files::{changed_while_written, remove_session_entries_except, remove_session_path, write_session_file_verified};
use std::io;
use std::path::{Path, PathBuf};

/// A folder of the session whose entries are all Studio's or none: the names in `keep` stay, anything else is removed.
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
/// `.cursor/mcp.json` through its compatibility layer), plugins, language servers (`.grok/lsp.json`) and workflow scripts, and the
/// hooks it reads from a project's Claude and Cursor files. The project config, the one thing Studio writes here besides its hook
/// file, is rewritten whole: it holds only `[mcp_servers]`, so a `[plugins]` or `[permission]` section an agent added goes with it.
const GROK: Sources = Sources {
    cli: "Grok",
    folders: &[Folder { path: ".grok/hooks", keep: &["designos-status.json"] }],
    absent: &[
        ".mcp.json",
        ".grok/plugins",
        ".grok/lsp.json",
        ".grok/workflows",
        ".claude/settings.json",
        ".claude/settings.local.json",
        ".claude/plugins",
        ".cursor/hooks.json",
        ".cursor/mcp.json",
    ],
};

/// Antigravity's workspace sources that start a process: the hooks file, the workspace MCP file, and every plugin (a plugin bundles
/// its own hooks and MCP servers). Studio's `designos` plugin keeps its two files and nothing else.
const AGY: Sources = Sources {
    cli: "Antigravity",
    folders: &[Folder { path: ".agents/plugins", keep: &["designos"] }, Folder { path: ".agents/plugins/designos", keep: &["plugin.json", "mcp_config.json"] }],
    absent: &[".agents/mcp_config.json"],
};

impl Sources {
    /// Removes every entry of the CLI's sources that is not Studio's, and says which went. Each removal is reported on stderr: the
    /// folder is the agent's workspace, and what goes there is not silently lost.
    fn clear(&self, session: &Path) -> io::Result<Vec<PathBuf>> {
        let mut removed = Vec::new();
        for folder in self.folders {
            let path = session.join(folder.path);
            removed.extend(remove_session_entries_except(&path, folder.keep)?.into_iter().map(|name| path.join(name)));
        }
        for rel in self.absent {
            let path = session.join(rel);
            if remove_session_path(&path)? {
                removed.push(path);
            }
        }
        Ok(removed)
    }

    fn clear_and_report(&self, session: &Path) -> io::Result<()> {
        for path in self.clear(session)? {
            eprintln!("design:os studio: removed {} from the agent folder: Studio does not write it, and {} would run it", path.display(), self.cli);
        }
        Ok(())
    }

    /// Nothing that is not Studio's came back while its own files were being written (another agent tab put it there).
    fn settled(&self, session: &Path) -> io::Result<()> {
        match self.clear(session)?.first() {
            Some(back) => Err(changed_while_written(back)),
            None => Ok(()),
        }
    }
}

/// Makes what Grok loads from `session` exactly Studio's: `config_toml` as the project config, `hooks_json` as the one hook file, and
/// none of the other sources of [`GROK`].
pub(crate) fn make_grok_config_exact(session: &Path, config_toml: &[u8], hooks_json: &[u8]) -> io::Result<()> {
    GROK.clear_and_report(session)?;
    write_session_file_verified(&session.join(".grok/config.toml"), config_toml)?;
    write_session_file_verified(&session.join(".grok/hooks/designos-status.json"), hooks_json)?;
    GROK.settled(session)
}

/// Makes what Antigravity loads from `session` exactly Studio's: `hooks_json` as the hooks file, the `designos` plugin made of
/// `plugin_manifest` and `mcp_config`, and none of the other sources of [`AGY`].
pub(crate) fn make_agy_config_exact(session: &Path, hooks_json: &[u8], plugin_manifest: &[u8], mcp_config: &[u8]) -> io::Result<()> {
    AGY.clear_and_report(session)?;
    write_session_file_verified(&session.join(".agents/plugins/designos/plugin.json"), plugin_manifest)?;
    write_session_file_verified(&session.join(".agents/plugins/designos/mcp_config.json"), mcp_config)?;
    write_session_file_verified(&session.join(".agents/hooks.json"), hooks_json)?;
    AGY.settled(session)
}
