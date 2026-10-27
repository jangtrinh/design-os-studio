//! The `codex app-server` child of one tab (#930).
//!
//! The server ignores EOF on its stdin and keeps running after its parent dies, so Studio starts it behind a small
//! `/bin/sh` watchdog in its own process group. The watchdog holds the read end of a pipe whose write end Studio keeps:
//! when Studio drops the handle (tab closed, quit) or dies (the kernel closes the pipe), the watchdog sends TERM to the
//! group (Codex removes its socket files on TERM) and KILL two seconds later. A thread reaps the watchdog afterwards.

use super::shell_env::TabEnv;
use portable_pty::CommandBuilder;
use std::ffi::OsString;
use std::fs::OpenOptions;
use std::io;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};

/// The `DESIGNOS_*` variables `designos-mcp` reads. Codex starts an MCP child with a scrubbed environment, so only
/// the ones named here reach it. The same list goes into today's embedded launch (`terminal.rs`); a test keeps the two
/// from drifting apart.
pub const MCP_ENV_VARS: [&str; 14] = [
    "DESIGNOS_SESSION_DIR",
    "DESIGNOS_MCP_CONFIG",
    "DESIGNOS_LIVE_CANVAS",
    "DESIGNOS_LIVE_DOC_SYNC",
    "DESIGNOS_LIVE_DOC_PATCH",
    "DESIGNOS_LIVE_SPEC",
    "DESIGNOS_CONTEXT_JSON",
    "DESIGNOS_TARGET_PAGE_ID",
    "DESIGNOS_TARGET_PAGE_NAME",
    "DESIGNOS_DOC_NAME",
    "DESIGNOS_FILE",
    "DESIGNOS_AGENT_RUN_ID",
    "DESIGNOS_AGENT_RUNTIME",
    "DESIGNOS_IN_PROGRESS_STALE_MINUTES",
];

/// `$@` is the server command. It starts first (before TERM is ignored) with stdin on `/dev/null`; the shell then
/// waits for the pipe from Studio to close.
const WATCHDOG_SCRIPT: &str = r#""$@" </dev/null & trap "" TERM; read _; kill -TERM 0; sleep 2; kill -KILL 0"#;

/// `O_NOFOLLOW` (`<fcntl.h>`): opening a link fails instead of following it. The card path runs on macOS only; the
/// Linux value keeps this module's tests honest there.
#[cfg(target_os = "macos")]
const O_NOFOLLOW: i32 = 0x0100;
#[cfg(not(target_os = "macos"))]
const O_NOFOLLOW: i32 = 0x2_0000;

/// A TOML basic string. JSON string escapes are a valid subset of TOML's (the same rendering `terminal.rs` uses).
fn toml_string(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_else(|_| "\"\"".into())
}

/// Everything needed to start the server of one tab.
pub struct ServerLaunch {
    codex_bin: PathBuf,
    rendezvous: PathBuf,
    mcp_bin: String,
    workspace: String,
    env: Vec<(OsString, OsString)>,
    cwd: PathBuf,
    log_path: PathBuf,
}

impl ServerLaunch {
    /// The working folder comes from the builder the tab's PTY is made from; the environment is the tab's (`env`), the
    /// same one the version probe ran with, so the server finds the same `node` and sees the same exports.
    pub fn from_command_builder(cmd: &CommandBuilder, env: &TabEnv, codex_bin: &Path, rendezvous: &Path, log_path: &Path) -> io::Result<Self> {
        let var = |name: &str| {
            env.get(name).and_then(|value| value.to_str()).map(str::to_string).ok_or_else(|| io::Error::other(format!("{name} is not set for the tab")))
        };
        Ok(Self {
            codex_bin: codex_bin.to_path_buf(),
            rendezvous: rendezvous.to_path_buf(),
            mcp_bin: var("DESIGNOS_MCP_BIN")?,
            workspace: var("DESIGNOS_WORKSPACE")?,
            env: env.vars().map(|(key, value)| (key.to_os_string(), value.to_os_string())).collect(),
            cwd: cmd.get_cwd().map(PathBuf::from).ok_or_else(|| io::Error::other("the tab has no working folder"))?,
            log_path: log_path.to_path_buf(),
        })
    }

    /// The folder the server (and the tab) runs in: the session folder.
    pub fn cwd(&self) -> &Path {
        &self.cwd
    }

    /// The server command: the Codex binary, `app-server --listen unix://<rendezvous>`, and the three overrides that
    /// register `designos-mcp`. Nothing else is passed: no approval, sandbox, trust or analytics setting.
    pub fn server_args(&self) -> io::Result<Vec<String>> {
        let text = |path: &Path| path.to_str().map(str::to_string).ok_or_else(|| io::Error::other(format!("{} is not UTF-8", path.display())));
        let names = MCP_ENV_VARS.iter().map(|name| toml_string(name)).collect::<Vec<_>>().join(",");
        Ok(vec![
            text(&self.codex_bin)?,
            "app-server".into(),
            "--listen".into(),
            format!("unix://{}", text(&self.rendezvous)?),
            "-c".into(),
            format!("mcp_servers.designos.command={}", toml_string(&self.mcp_bin)),
            "-c".into(),
            format!("mcp_servers.designos.args=[\"--workspace\",{}]", toml_string(&self.workspace)),
            "-c".into(),
            format!("mcp_servers.designos.env_vars=[{names}]"),
        ])
    }
}

/// A running server and its watchdog. Dropping it ends both.
pub struct ServerHandle {
    child: Option<Child>,
    stdin: Option<ChildStdin>,
}

impl ServerHandle {
    /// The watchdog's pid, which is also the process group id of the whole server.
    #[cfg(test)]
    pub fn pid(&self) -> u32 {
        self.child.as_ref().map_or(0, Child::id)
    }
}

impl Drop for ServerHandle {
    fn drop(&mut self) {
        // Closing the write end is the signal: the watchdog's `read` returns and it ends the group.
        drop(self.stdin.take());
        if let Some(mut child) = self.child.take() {
            // Dropping a `Child` does not wait on it, and the watchdog lives about two more seconds. A thread reaps it,
            // so no zombie is left and the caller (often the UI thread) never waits.
            let _ = std::thread::Builder::new().name("codex-server-reaper".into()).spawn(move || {
                let _ = child.wait();
            });
        }
    }
}

/// Starts the server behind its watchdog. Output goes to the owner-only log file named in the launch.
pub fn start(launch: &ServerLaunch) -> io::Result<ServerHandle> {
    let args = launch.server_args()?;
    let log = OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).custom_flags(O_NOFOLLOW).open(&launch.log_path)?;
    std::fs::set_permissions(&launch.log_path, std::fs::Permissions::from_mode(0o600))?;
    let mut child = Command::new("/bin/sh")
        .arg("-c")
        .arg(WATCHDOG_SCRIPT)
        .arg("designos-codex-app-server")
        .args(&args)
        .env_clear()
        .envs(launch.env.iter().map(|(key, value)| (key, value)))
        .current_dir(&launch.cwd)
        .stdin(Stdio::piped())
        .stdout(log.try_clone()?)
        .stderr(log)
        .process_group(0)
        .spawn()?;
    let stdin = child.stdin.take();
    Ok(ServerHandle { child: Some(child), stdin })
}
