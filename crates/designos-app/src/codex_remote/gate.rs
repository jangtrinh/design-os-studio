//! The launch gate (#930): which Codex tabs take the card path.
//!
//! A tab gets the app-server and the Approve/Deny cards only when the exact binary it will run is a build this code
//! was tested against, and the rendezvous socket path fits. Every other case starts the tab as today. The version
//! probe runs off the calling thread: the first question about a binary answers "not known yet" and starts the probe,
//! so the UI thread never waits on `codex --version`.
//!
//! The probe runs with the tab's environment, not Studio's: an npm `codex` is `#!/usr/bin/env node`, and a Studio started
//! from Finder or the Dock has launchd's `PATH`, which cannot find `node`. That `PATH` is part of the cache key. A probe
//! that failed is asked again after [`RETRY_FAILED_AFTER`], so one transient failure does not end the card path for
//! the rest of the session.

use super::shell_capture::{kill_group, spawn_retrying};
use super::shell_env::TabEnv;
use super::FallbackReason;
use crate::agent_session_failure::extract_semver_token;
use std::collections::HashMap;
use std::ffi::{OsStr, OsString};
use std::io::Read;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime};

/// The one Codex release line (`major.minor`) the card path is built and tested against. A version that does not
/// exist here turns the card path off for every tab: the kill switch.
pub const TESTED_CODEX: (u32, u32) = (0, 164);
/// The longest unix socket path macOS accepts (`sun_path` is 104 bytes, one of them the NUL).
pub const MAX_RENDEZVOUS_BYTES: usize = 103;
const PROBE_TIMEOUT: Duration = Duration::from_secs(5);
/// How long a failed or timed-out probe is remembered before the next lookup tries again.
pub const RETRY_FAILED_AFTER: Duration = Duration::from_secs(30);
const BINARY_NAME: &str = "codex";

#[derive(Debug, PartialEq)]
pub enum VersionVerdict {
    Supported,
    /// A readable version outside the tested line (a pre-release of it counts as outside).
    Outside(String),
    Unknown,
}

/// Judges the output of `codex --version`: a line `codex-cli X.Y.Z`. Anything else is `Unknown`.
pub fn judge_version(output: Option<&str>) -> VersionVerdict {
    let Some(rest) = output.and_then(|text| text.lines().find_map(|line| line.trim().strip_prefix("codex-cli "))) else { return VersionVerdict::Unknown };
    extract_semver_token(rest).map_or(VersionVerdict::Unknown, |token| judge_semver(&token))
}

/// Judges one version token (`X.Y.Z`): the tested release line, another one, or not a version.
pub fn judge_semver(token: &str) -> VersionVerdict {
    let token = token.to_string();
    let pre_release = token.contains(['-', '+']);
    let core = token.split(['-', '+']).next().unwrap_or_default();
    let parts: Option<Vec<u32>> = core.split('.').map(|part| part.parse().ok()).collect();
    match parts.as_deref() {
        Some([major, minor, _patch]) if (*major, *minor) == TESTED_CODEX && !pre_release => VersionVerdict::Supported,
        Some([_, _, _]) => VersionVerdict::Outside(token),
        _ => VersionVerdict::Unknown,
    }
}

/// The first executable `codex` in `path` (the `PATH` string the tab's PTY gets), as an absolute path. Relative and
/// empty entries never resolve. The path is not canonicalized: the same path goes to the server and the TUI.
pub fn resolve_in_path(path: &OsStr) -> Option<PathBuf> {
    std::env::split_paths(path)
        .filter(|dir| dir.is_absolute())
        .map(|dir| dir.join(BINARY_NAME))
        .find(|candidate| std::fs::metadata(candidate).is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0))
}

/// Raw `--version` output of `bin` run with `env` when it exits 0 within `timeout`; a process that outlasts it is killed.
/// A spawn that fails is tried once more after 50 ms (a script written a moment ago can still be busy).
pub fn probe_version(bin: &Path, env: &TabEnv, timeout: Duration) -> Option<String> {
    let mut command = Command::new(bin);
    command.arg("--version").env_clear().envs(env.vars()).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null()).process_group(0);
    let mut child = spawn_retrying(&mut command).ok()?;
    let pgid = child.id();
    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if started.elapsed() < timeout => std::thread::sleep(Duration::from_millis(20)),
            _ => {
                // The npm shim starts the native binary as a child: take the whole group down, not just the shim.
                kill_group(pgid);
                let _ = child.wait();
                return None;
            }
        }
    };
    let mut out = String::new();
    child.stdout.take()?.take(4096).read_to_string(&mut out).ok()?;
    status.success().then_some(out)
}

pub enum VersionLookup {
    /// The probe has not finished (or has just started).
    Pending,
    /// The probe finished: the raw output, or `None` when it failed.
    Known(Option<String>),
}

enum Slot {
    Probing,
    /// The raw output (`None`: the probe failed) and when it finished.
    Done(Option<String>, Instant),
}

/// (path, modified time, length, `PATH` of the environment): a binary that is replaced (an upgrade) is probed again, and
/// so is the same binary under a `PATH` that finds different interpreters.
type CacheKey = (PathBuf, SystemTime, u64, OsString);

fn lock_slots() -> std::sync::MutexGuard<'static, HashMap<CacheKey, Slot>> {
    static SLOTS: OnceLock<Mutex<HashMap<CacheKey, Slot>>> = OnceLock::new();
    SLOTS.get_or_init(Mutex::default).lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// What is known about `bin`'s version under `env`, without blocking. The first call for a binary state starts the probe.
pub fn lookup_version(bin: &Path, env: &TabEnv) -> VersionLookup {
    lookup_version_with(bin, env, RETRY_FAILED_AFTER)
}

/// [`lookup_version`] with a chosen delay before a failed probe is asked again.
pub fn lookup_version_with(bin: &Path, env: &TabEnv, retry_after: Duration) -> VersionLookup {
    let Ok(meta) = std::fs::metadata(bin) else { return VersionLookup::Known(None) };
    let Ok(modified) = meta.modified() else { return VersionLookup::Known(None) };
    let key: CacheKey = (bin.to_path_buf(), modified, meta.len(), env.path().unwrap_or_default().to_os_string());
    let mut guard = lock_slots();
    match guard.get(&key) {
        Some(Slot::Done(raw, at)) if raw.is_some() || at.elapsed() < retry_after => return VersionLookup::Known(raw.clone()),
        Some(Slot::Probing) => return VersionLookup::Pending,
        _ => {}
    }
    let (probe_key, probe_bin, probe_env) = (key.clone(), bin.to_path_buf(), env.clone());
    let spawned = std::thread::Builder::new().name("codex-version-probe".into()).spawn(move || {
        let raw = probe_version(&probe_bin, &probe_env, PROBE_TIMEOUT);
        lock_slots().insert(probe_key, Slot::Done(raw, Instant::now()));
    });
    if spawned.is_err() {
        return VersionLookup::Known(None);
    }
    guard.insert(key, Slot::Probing);
    VersionLookup::Pending
}

/// Starts the version probe for `bin` under `env` so a tab opened later finds the answer ready.
pub fn warm(bin: &Path, env: &TabEnv) {
    let _ = lookup_version(bin, env);
}

/// Gate row: the version of the binary the tab will run. `VersionUnknown` while the probe is running or when it failed.
pub fn check_binary(bin: &Path, env: &TabEnv) -> Result<(), FallbackReason> {
    match lookup_version(bin, env) {
        VersionLookup::Pending | VersionLookup::Known(None) => Err(FallbackReason::VersionUnknown),
        VersionLookup::Known(Some(raw)) => match judge_version(Some(&raw)) {
            VersionVerdict::Supported => Ok(()),
            VersionVerdict::Outside(version) => Err(FallbackReason::VersionOutsideRange(version)),
            VersionVerdict::Unknown => Err(FallbackReason::VersionUnknown),
        },
    }
}

/// `<channel>/cx-<tab id>.sock`, the path the TUI and the client both use, when it fits a socket address.
pub fn rendezvous_path(channel_dir: &Path, tab_id: usize) -> Result<PathBuf, FallbackReason> {
    let path = channel_dir.join(format!("cx-{tab_id}.sock"));
    if path.as_os_str().as_bytes().len() > MAX_RENDEZVOUS_BYTES {
        return Err(FallbackReason::PathTooLong);
    }
    Ok(path)
}
