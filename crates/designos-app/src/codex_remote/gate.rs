//! The launch gate (#930): which Codex tabs take the card path.
//!
//! A tab gets the app-server and the Approve/Deny cards only when the exact binary it will run is a build this code
//! was tested against, and the rendezvous socket path fits. Every other case starts the tab as today. The version
//! probe runs off the calling thread: the first question about a binary answers "not known yet" and starts the probe,
//! so the UI thread never waits on `codex --version`.

use super::FallbackReason;
use crate::agent_session_failure::extract_semver_token;
use std::collections::HashMap;
use std::ffi::OsStr;
use std::io::Read;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::PermissionsExt;
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

/// Raw `--version` output of `bin` when it exits 0 within `timeout`; a process that outlasts it is killed.
pub fn probe_version(bin: &Path, timeout: Duration) -> Option<String> {
    let mut child = Command::new(bin).arg("--version").stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().ok()?;
    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if started.elapsed() < timeout => std::thread::sleep(Duration::from_millis(20)),
            _ => {
                let _ = child.kill();
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
    Done(Option<String>),
}

/// (path, modified time, length): a binary that is replaced (an upgrade) is probed again.
type CacheKey = (PathBuf, SystemTime, u64);

fn lock_slots() -> std::sync::MutexGuard<'static, HashMap<CacheKey, Slot>> {
    static SLOTS: OnceLock<Mutex<HashMap<CacheKey, Slot>>> = OnceLock::new();
    SLOTS.get_or_init(Mutex::default).lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// What is known about `bin`'s version, without blocking. The first call for a binary state starts the probe.
pub fn lookup_version(bin: &Path) -> VersionLookup {
    let Ok(meta) = std::fs::metadata(bin) else { return VersionLookup::Known(None) };
    let Ok(modified) = meta.modified() else { return VersionLookup::Known(None) };
    let key: CacheKey = (bin.to_path_buf(), modified, meta.len());
    let mut guard = lock_slots();
    match guard.get(&key) {
        Some(Slot::Done(raw)) => return VersionLookup::Known(raw.clone()),
        Some(Slot::Probing) => return VersionLookup::Pending,
        None => {}
    }
    let (probe_key, probe_bin) = (key.clone(), bin.to_path_buf());
    let spawned = std::thread::Builder::new().name("codex-version-probe".into()).spawn(move || {
        let raw = probe_version(&probe_bin, PROBE_TIMEOUT);
        lock_slots().insert(probe_key, Slot::Done(raw));
    });
    if spawned.is_err() {
        return VersionLookup::Known(None);
    }
    guard.insert(key, Slot::Probing);
    VersionLookup::Pending
}

/// Starts the version probe for `bin` so a tab opened later finds the answer ready.
pub fn warm(bin: &Path) {
    let _ = lookup_version(bin);
}

/// Gate row: the version of the binary the tab will run. `VersionUnknown` while the probe is running or when it failed.
pub fn check_binary(bin: &Path) -> Result<(), FallbackReason> {
    match lookup_version(bin) {
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
