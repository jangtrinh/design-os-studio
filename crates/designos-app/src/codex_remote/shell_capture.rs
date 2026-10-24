//! Capturing the login shell's environment (#930): the one process Studio starts only to read variables.
//!
//! The shell is `$SHELL -l -i -c <script>`; the script prints a marker and then the environment (`env -0`). The marker is
//! random for each capture, so output a startup file wrote beforehand cannot pass for it, and the output must hold exactly
//! one marker line: a startup file or background job that learns the marker (it is in the script) and prints it as well
//! makes the capture fail, and the tab keeps the builder's environment. A startup file is the user's own code and can do
//! whatever the user can; this keeps its noise and accidents out, it is not a defence against it. The shell starts in the
//! user's home folder, never in a folder an agent can write.

use super::server_process::MCP_ENV_VARS;
use super::shell_env::TabEnv;
use portable_pty::CommandBuilder;
use std::ffi::OsString;
use std::io::Read;
use std::os::unix::ffi::OsStringExt;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::channel;
use std::time::{Duration, Instant};

pub const MARKER_PREFIX: &str = "__DESIGNOS_ENV_";

/// `__DESIGNOS_ENV_<32 random hex digits>__`: new for each capture, from 16 bytes of `/dev/urandom` (all 128 bits random,
/// no fixed digits, no clock-based fallback: a marker that can be guessed is one a startup file could print). `None` when
/// the system cannot give them, and the capture is then skipped. The marker has no character a shell would quote.
pub fn new_marker() -> Option<String> {
    let mut bytes = [0u8; 16];
    std::fs::File::open("/dev/urandom").and_then(|mut random| random.read_exact(&mut bytes)).ok()?;
    let hex: String = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
    Some(format!("{MARKER_PREFIX}{hex}__"))
}

/// The one argument given to the shell: prints the marker line, then the environment, NUL-separated.
pub fn capture_script(marker: &str) -> String {
    format!("/bin/echo {marker}; /usr/bin/env -0")
}

/// The entries after the marker line of `output`: NUL-separated `NAME=value`, anything else dropped. `None` unless the
/// output holds the marker line exactly once (which of two is the real one cannot be told).
pub fn parse_capture(output: &[u8], marker: &str) -> Option<Vec<(OsString, OsString)>> {
    let line = [marker.as_bytes(), b"\n"].concat();
    let mut found = output.windows(line.len()).enumerate().filter(|(_, window)| *window == line).map(|(at, _)| at);
    let start = found.next()? + line.len();
    if found.next().is_some() {
        return None;
    }
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
pub(super) fn kill_group(pgid: u32) {
    let _ = Command::new("/bin/kill").args(["-KILL", "--", &format!("-{pgid}")]).stdout(Stdio::null()).stderr(Stdio::null()).status();
}

/// The user's home folder: the builder's `HOME`, else the process's, else `/`. A folder that does not exist does not count.
fn home_folder(builder: &CommandBuilder) -> PathBuf {
    let usable = |dir: &PathBuf| dir.is_absolute() && dir.is_dir();
    builder.get_env("HOME").map(PathBuf::from).filter(usable).or_else(|| std::env::home_dir().filter(usable)).unwrap_or_else(|| PathBuf::from("/"))
}

/// Runs `shell -l -i -c <script>` from the builder's environment in the user's home folder (stdin closed, own process
/// group, killed at `timeout`) and returns the environment it printed, with the builder's `DESIGNOS_*` values and the MCP
/// names laid over it. `None` on any failure: no such shell, non-zero exit, timeout, no marker.
pub fn capture(builder: &CommandBuilder, shell: &Path, timeout: Duration) -> Option<TabEnv> {
    let (marker, base) = (new_marker()?, TabEnv::from_builder(builder));
    let mut command = Command::new(shell);
    command
        .args(["-l", "-i", "-c", &capture_script(&marker)])
        .current_dir(home_folder(builder))
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
    let mut vars = parse_capture(&bytes, &marker).filter(|_| status.success())?;
    // Studio's own values go last, so they win: `TabEnv` keeps the last entry of a name.
    vars.extend(
        builder
            .iter_full_env_as_str()
            .filter(|(name, _)| name.starts_with("DESIGNOS_") || MCP_ENV_VARS.contains(name))
            .map(|(name, value)| (name.into(), value.into())),
    );
    Some(TabEnv::from_vars(vars))
}
