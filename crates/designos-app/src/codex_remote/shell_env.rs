//! The environment a Codex tab runs in (#930).
//!
//! Today's embedded Codex runs under `$SHELL -l -i -c`, so what the user's startup files export (nvm's `PATH`, `CODEX_HOME`,
//! proxies, API keys) reaches the process that runs the model. On the card path that process is the app-server, and the
//! version probe has to find the same `node`, so all of them get the same environment: the login shell's (read by
//! `shell_capture`), captured once per session, with Studio's own `DESIGNOS_*` values laid over it. If the capture fails
//! for any reason the PTY builder's environment is used and the tab works as before.
//!
//! The values are kept in memory only. They are never logged or written to disk, and `Debug` prints names only.

use super::gate::RETRY_FAILED_AFTER;
use super::shell_capture::capture;
use portable_pty::CommandBuilder;
use std::collections::HashMap;
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant};

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
        Self::from_vars(builder.iter_full_env_as_str().map(|(name, value)| (name.into(), value.into())).collect())
    }

    /// These entries as an environment. A name that repeats keeps its last value, which is what a process started with
    /// all of them in order (`Command::envs`) would see.
    pub(super) fn from_vars(vars: Vec<(OsString, OsString)>) -> Self {
        let mut unique: Vec<(OsString, OsString)> = Vec::with_capacity(vars.len());
        for (name, value) in vars {
            match unique.iter_mut().find(|(key, _)| *key == name) {
                Some(entry) => entry.1 = value,
                None => unique.push((name, value)),
            }
        }
        TabEnv { vars: unique }
    }

    /// This environment with the `DESIGNOS_*` values of `builder` laid over it: a session keeps one captured environment,
    /// and each tab brings its own run id, page and files.
    pub fn overlaid_with(&self, builder: &CommandBuilder) -> TabEnv {
        let mut vars = self.vars.clone();
        vars.extend(builder.iter_full_env_as_str().filter(|(name, _)| name.starts_with("DESIGNOS_")).map(|(name, value)| (name.into(), value.into())));
        TabEnv::from_vars(vars)
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

enum Slot {
    /// The capture with this id is running.
    Capturing(u64),
    Ready(Arc<TabEnv>),
    /// The capture failed: the builder's environment is used until `retry_after` has passed since `since`, then a new
    /// capture starts (`retrying`, with its id) while this one keeps serving.
    Fallback {
        env: Arc<TabEnv>,
        since: Instant,
        retrying: Option<u64>,
    },
}

fn slots() -> MutexGuard<'static, HashMap<String, Slot>> {
    static SLOTS: OnceLock<Mutex<HashMap<String, Slot>>> = OnceLock::new();
    SLOTS.get_or_init(Mutex::default).lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// A new id for each capture. A capture stores its result only while its id is still the one in the session's slot, so a
/// session that was forgotten (or forgotten and started again) while it ran keeps nothing from it.
fn next_capture_id() -> u64 {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    NEXT.fetch_add(1, Ordering::Relaxed)
}

pub enum EnvLookup {
    /// The capture is still running (or has just started).
    Pending,
    /// The session's environment: captured, or the builder's while a capture keeps failing.
    Ready(Arc<TabEnv>),
}

/// The environment of `session`, without blocking. The first call starts the capture on its own thread. A captured
/// environment is kept for the session (later tabs get the same one). A failed capture serves the builder's environment
/// and is tried again after `retry_after` ([`lookup`] uses `RETRY_FAILED_AFTER`, 30 s), so a slow or loaded first start does not
/// settle the whole session.
pub fn lookup(session: &str, builder: &CommandBuilder, shell: &Path) -> EnvLookup {
    lookup_with(session, builder, shell, CAPTURE_TIMEOUT, RETRY_FAILED_AFTER)
}

/// [`lookup`] with the capture timeout and the retry delay given.
pub fn lookup_with(session: &str, builder: &CommandBuilder, shell: &Path, timeout: Duration, retry_after: Duration) -> EnvLookup {
    let mut guard = slots();
    let id = next_capture_id();
    let serving = match guard.get_mut(session) {
        Some(Slot::Ready(env)) => return EnvLookup::Ready(env.clone()),
        Some(Slot::Capturing(_)) => return EnvLookup::Pending,
        Some(Slot::Fallback { env, since, retrying }) if retrying.is_some() || since.elapsed() < retry_after => return EnvLookup::Ready(env.clone()),
        Some(Slot::Fallback { env, retrying, .. }) => {
            *retrying = Some(id);
            Some(env.clone())
        }
        None => None,
    };
    let (key, owned, shell) = (session.to_string(), builder.clone(), shell.to_path_buf());
    let spawned = std::thread::Builder::new().name("codex-env-capture".into()).spawn(move || {
        let slot = match capture(&owned, &shell, timeout) {
            Some(env) => Slot::Ready(Arc::new(env)),
            None => Slot::Fallback { env: Arc::new(TabEnv::from_builder(&owned)), since: Instant::now(), retrying: None },
        };
        let mut guard = slots();
        let mine = match guard.get(&key) {
            Some(Slot::Capturing(running)) => *running == id,
            Some(Slot::Fallback { retrying, .. }) => *retrying == Some(id),
            _ => false,
        };
        if mine {
            guard.insert(key, slot);
        }
    });
    match (spawned.is_ok(), serving) {
        (true, Some(env)) => EnvLookup::Ready(env),
        (true, None) => {
            guard.insert(session.to_string(), Slot::Capturing(id));
            EnvLookup::Pending
        }
        (false, serving) => {
            if let Some(Slot::Fallback { retrying, .. }) = guard.get_mut(session) {
                *retrying = None;
            }
            EnvLookup::Ready(serving.unwrap_or_else(|| Arc::new(TabEnv::from_builder(builder))))
        }
    }
}

/// Drops the session's environment (the session ended).
pub fn forget(session: &str) {
    slots().remove(session);
}
