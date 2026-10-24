//! The environment a Codex tab runs in (#930).
//!
//! Today's embedded Codex runs under `$SHELL -l -i -c`, so what the user's startup files export (nvm's `PATH`, `CODEX_HOME`,
//! proxies, API keys) reaches the process that runs the model. On the card path that process is the app-server, and the
//! version probe has to find the same `node`, so all of them get the same environment: the login shell's, captured once
//! per session, with Studio's own `DESIGNOS_*` values laid over it. If the capture fails for any reason the PTY
//! builder's environment is used and the tab works as before.
//!
//! The values are kept in memory only. They are never logged or written to disk, and `Debug` prints names only.

use super::server_process::MCP_ENV_VARS;
use portable_pty::CommandBuilder;
use std::collections::HashMap;
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::io::Read;
use std::os::unix::ffi::OsStringExt;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant};

/// Printed before the environment, so whatever the startup files print first (output, prompt escapes) is ignored.
pub const MARKER: &str = "__DESIGNOS_ENV_BEGIN__";
/// One argument, with no quote, backslash or `$`: fish, zsh and bash parse it the same way.
pub const CAPTURE_SCRIPT: &str = "/bin/echo __DESIGNOS_ENV_BEGIN__; /usr/bin/env -0";
pub const CAPTURE_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Clone)]
pub struct TabEnv {
    vars: Vec<(OsString, OsString)>,
}

impl fmt::Debug for TabEnv {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let names: Vec<_> = self.vars.iter().map(|(name, _)| name.to_string_lossy()).collect();
        f.debug_tuple("TabEnv").field(&names).finish()
    }
}

impl TabEnv {
    /// The builder's environment (the entries that are valid UTF-8).
    pub fn from_builder(builder: &CommandBuilder) -> Self {
        TabEnv { vars: builder.iter_full_env_as_str().map(|(name, value)| (name.into(), value.into())).collect() }
    }

    pub fn get(&self, name: &str) -> Option<&OsStr> {
        self.vars.iter().find(|(key, _)| key == name).map(|(_, value)| value.as_os_str())
    }

    pub fn path(&self) -> Option<&OsStr> {
        self.get("PATH")
    }

    pub fn vars(&self) -> impl Iterator<Item = (&OsStr, &OsStr)> {
        self.vars.iter().map(|(name, value)| (name.as_os_str(), value.as_os_str()))
    }
}

/// The shell the tab starts agents with: the builder's `SHELL`, else the terminal's own fallback.
pub fn tab_shell(builder: &CommandBuilder) -> PathBuf {
    match builder.get_env("SHELL").filter(|shell| !shell.is_empty()) {
        Some(shell) => PathBuf::from(shell),
        None if Path::new("/bin/zsh").exists() => PathBuf::from("/bin/zsh"),
        None => PathBuf::from("/bin/sh"),
    }
}

/// The variables after the marker line of `output`: NUL-separated `NAME=value` entries, anything else dropped.
pub fn parse_capture(output: &[u8]) -> Option<Vec<(OsString, OsString)>> {
    let marker = [MARKER.as_bytes(), b"\n"].concat();
    let start = output.windows(marker.len()).position(|window| window == marker)? + marker.len();
    let vars: Vec<_> = output[start..]
        .split(|byte| *byte == 0)
        .filter_map(|entry| {
            let at = entry.iter().position(|byte| *byte == b'=').filter(|at| *at > 0)?;
            Some((OsString::from_vec(entry[..at].to_vec()), OsString::from_vec(entry[at + 1..].to_vec())))
        })
        .collect();
    (!vars.is_empty()).then_some(vars)
}

/// Spawns `command`; a spawn that fails is tried once more after 50 ms, because a script written a moment ago can still
/// be busy while another thread forks.
pub fn spawn_retrying(command: &mut Command) -> std::io::Result<Child> {
    command.spawn().or_else(|_| {
        std::thread::sleep(Duration::from_millis(50));
        command.spawn()
    })
}

/// Kills a process group: the shell and everything its startup files started.
fn kill_group(pgid: u32) {
    let _ = Command::new("/bin/kill").args(["-KILL", "--", &format!("-{pgid}")]).stdout(Stdio::null()).stderr(Stdio::null()).status();
}

/// Runs `shell -l -i -c <script>` starting from the builder's environment (stdin closed, own process group, killed at
/// `timeout`) and returns the environment it printed, with the builder's `DESIGNOS_*` values and the MCP names laid
/// over it. `None` on any failure: no such shell, non-zero exit, timeout, no marker.
pub fn capture(builder: &CommandBuilder, shell: &Path, timeout: Duration) -> Option<TabEnv> {
    let base = TabEnv::from_builder(builder);
    let mut command = Command::new(shell);
    command
        .args(["-l", "-i", "-c", CAPTURE_SCRIPT])
        .env_clear()
        .envs(base.vars())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .process_group(0);
    let mut child = spawn_retrying(&mut command).ok()?;
    let (pgid, mut stdout) = (child.id(), child.stdout.take()?);
    let (sender, output) = channel();
    std::thread::Builder::new()
        .name("codex-env-reader".into())
        .spawn(move || {
            let mut bytes = Vec::new();
            let _ = stdout.read_to_end(&mut bytes);
            let _ = sender.send(bytes);
        })
        .ok()?;
    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if started.elapsed() < timeout => std::thread::sleep(Duration::from_millis(20)),
            _ => {
                kill_group(pgid);
                let _ = child.wait();
                return None;
            }
        }
    };
    // A startup file may leave a background process holding the pipe: wait for the end of the output only so long.
    let Ok(bytes) = output.recv_timeout(timeout.saturating_sub(started.elapsed()).max(Duration::from_millis(200))) else {
        kill_group(pgid);
        return None;
    };
    let mut vars = parse_capture(&bytes).filter(|_| status.success())?;
    for (name, value) in builder.iter_full_env_as_str().filter(|(name, _)| name.starts_with("DESIGNOS_") || MCP_ENV_VARS.contains(name)) {
        match vars.iter_mut().find(|(key, _)| key == name) {
            Some(entry) => entry.1 = value.into(),
            None => vars.push((name.into(), value.into())),
        }
    }
    Some(TabEnv { vars })
}

enum Slot {
    Capturing,
    Ready(Arc<TabEnv>),
}

fn slots() -> MutexGuard<'static, HashMap<String, Slot>> {
    static SLOTS: OnceLock<Mutex<HashMap<String, Slot>>> = OnceLock::new();
    SLOTS.get_or_init(Mutex::default).lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

pub enum EnvLookup {
    /// The capture is still running (or has just started).
    Pending,
    /// The session's environment: captured, or the builder's when the capture failed.
    Ready(Arc<TabEnv>),
}

/// The environment of `session`, without blocking. The first call starts the capture on its own thread; the answer is
/// kept for the session, so later calls (and later tabs) get the same environment.
pub fn lookup(session: &str, builder: &CommandBuilder, shell: &Path, timeout: Duration) -> EnvLookup {
    let mut guard = slots();
    match guard.get(session) {
        Some(Slot::Ready(env)) => return EnvLookup::Ready(env.clone()),
        Some(Slot::Capturing) => return EnvLookup::Pending,
        None => {}
    }
    let (key, owned, shell) = (session.to_string(), builder.clone(), shell.to_path_buf());
    let spawned = std::thread::Builder::new().name("codex-env-capture".into()).spawn(move || {
        let env = capture(&owned, &shell, timeout).unwrap_or_else(|| TabEnv::from_builder(&owned));
        slots().insert(key, Slot::Ready(Arc::new(env)));
    });
    if spawned.is_err() {
        return EnvLookup::Ready(Arc::new(TabEnv::from_builder(builder)));
    }
    guard.insert(session.to_string(), Slot::Capturing);
    EnvLookup::Pending
}

/// Drops the session's environment (the session ended).
pub fn forget(session: &str) {
    slots().remove(session);
}
