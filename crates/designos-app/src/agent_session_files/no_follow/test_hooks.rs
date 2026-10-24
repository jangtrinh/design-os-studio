//! Test-only hooks that put a swap of a folder at an exact point of opening it, so a race is not left to chance. A hook is keyed
//! by the path being opened, so parallel tests never run each other's.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// The points of opening a folder at which a test can change what its path holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::agent_session_files) enum Step {
    /// Just before each `open` or `openat`.
    BeforeOpen,
    /// After an `open` or `openat` failed, and before the entry is looked at to say why.
    AfterFailedOpen,
}

type Hook = Arc<dyn Fn(Step) + Send + Sync>;

static HOOKS: Mutex<Vec<(PathBuf, Hook)>> = Mutex::new(Vec::new());

/// Runs `hook` at every [`Step`] of every attempt to open the folder `path`, until [`clear_open_hook`].
pub(in crate::agent_session_files) fn set_open_hook(path: &Path, hook: impl Fn(Step) + Send + Sync + 'static) {
    HOOKS.lock().unwrap_or_else(|e| e.into_inner()).push((path.to_path_buf(), Arc::new(hook)));
}

pub(in crate::agent_session_files) fn clear_open_hook(path: &Path) {
    HOOKS.lock().unwrap_or_else(|e| e.into_inner()).retain(|(p, _)| p != path);
}

pub(super) fn run(path: &Path, step: Step) {
    let hook = HOOKS.lock().unwrap_or_else(|e| e.into_inner()).iter().find(|(p, _)| p == path).map(|(_, hook)| hook.clone());
    if let Some(hook) = hook {
        hook(step);
    }
}
