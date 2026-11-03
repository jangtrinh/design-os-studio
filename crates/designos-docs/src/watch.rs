//! Reports external edits to a docs folder from a background thread. The caller never touches the disk: it drains
//! an in-memory queue and is woken through a callback (e.g. `ctx.request_repaint()`).
//!
//! The thread sleeps until the operating system says something under the folder changed (FSEvents on macOS,
//! inotify on Linux), so an idle project costs no CPU. Only when the native watcher cannot start, or reports that
//! it failed, does the thread fall back to looking at the folder every [`POLL`]; `mode()` says which one is in use.
//! The first look at the folder happens on the thread too, so `start` returns at once however many pages exist.
//!
//! Studio's own writes are reported too; recognise them by the hash `DocsStore::write_*` returned.

use crate::hash::ContentHash;
use crate::watch_scan::{baseline, check};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};

/// Fallback look interval, used only when the native watcher is unavailable.
const POLL: Duration = Duration::from_millis(400);
/// Events that follow each other closer than this are one batch (an atomic write is several events).
const BATCH_QUIET: Duration = Duration::from_millis(30);
const BATCH_MAX: Duration = Duration::from_millis(250);

/// A page (or the manifest) that changed on disk.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DocChange {
    /// Path relative to the docs folder, `/`-separated.
    pub rel: String,
    /// Hash of the new content, or `None` when the file was deleted.
    pub hash: Option<ContentHash>,
}

/// How a running watcher learns about changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WatchMode {
    /// The operating system tells the watcher; no work while nothing changes.
    Native,
    /// The watcher looks at the folder every 400 ms.
    Polling,
}

enum Msg {
    Event(notify::Result<notify::Event>),
    Stop,
}

const MODE_STARTING: u8 = 0;
const MODE_NATIVE: u8 = 1;
const MODE_POLLING: u8 = 2;

pub struct DocsWatcher {
    rx: Mutex<Receiver<DocChange>>,
    ctl: Sender<Msg>,
    stopped: Arc<AtomicBool>,
    mode: Arc<AtomicU8>,
}

impl DocsWatcher {
    /// Starts watching `dir` (it may not exist yet). `wake` runs on the watcher thread after each batch of
    /// changes is queued. Nothing is read from disk on the calling thread. Files written after this call are
    /// always reported, even when they land before the watcher thread has looked at the folder.
    pub fn start(dir: impl Into<PathBuf>, wake: impl Fn() + Send + 'static) -> Self {
        Self::spawn(dir.into(), wake, true)
    }

    /// Like [`start`](Self::start) but always looks at the folder on a timer. For folders where the system sends
    /// no change events (some network shares).
    pub fn start_polling(dir: impl Into<PathBuf>, wake: impl Fn() + Send + 'static) -> Self {
        Self::spawn(dir.into(), wake, false)
    }

    fn spawn(dir: PathBuf, wake: impl Fn() + Send + 'static, native: bool) -> Self {
        let started_at = SystemTime::now();
        let (tx, rx) = channel();
        let (ctl, ctl_rx) = channel();
        let stopped = Arc::new(AtomicBool::new(false));
        let mode = Arc::new(AtomicU8::new(MODE_STARTING));
        let job = Job { dir, tx, ctl: ctl.clone(), ctl_rx, wake: Box::new(wake), started_at, native, mode: mode.clone() };
        let guard = StopGuard(stopped.clone());
        let spawned = std::thread::Builder::new().name("designos-docs-watch".into()).spawn(move || {
            let _guard = guard; // sets the flag when the thread ends, however it ends (also when `wake` panics)
            run(job);
        });
        if spawned.is_err() {
            // No thread, no events; the caller sees a watcher that reports nothing and reads the flag.
            stopped.store(true, Ordering::Release);
        }
        Self { rx: Mutex::new(rx), ctl, stopped, mode }
    }

    /// Queued changes, oldest first. Never blocks and never touches the disk.
    pub fn drain(&self) -> Vec<DocChange> {
        let Ok(rx) = self.rx.lock() else { return Vec::new() };
        rx.try_iter().collect()
    }

    /// Waits up to `timeout` for the next change (for tests and headless callers; UI code uses `drain`).
    pub fn recv_timeout(&self, timeout: Duration) -> Option<DocChange> {
        self.rx.lock().ok()?.recv_timeout(timeout).ok()
    }

    /// Becomes true once the watcher thread has exited (after this watcher is dropped, or if it could not start,
    /// or if the wake callback panicked).
    pub fn stopped_flag(&self) -> Arc<AtomicBool> {
        self.stopped.clone()
    }

    /// How changes are being detected; `None` while the thread is still setting up.
    pub fn mode(&self) -> Option<WatchMode> {
        match self.mode.load(Ordering::Acquire) {
            MODE_NATIVE => Some(WatchMode::Native),
            MODE_POLLING => Some(WatchMode::Polling),
            _ => None,
        }
    }
}

impl Drop for DocsWatcher {
    fn drop(&mut self) {
        let _ = self.ctl.send(Msg::Stop); // fails only when the thread is already gone
    }
}

struct StopGuard(Arc<AtomicBool>);

impl Drop for StopGuard {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Release);
    }
}

struct Job {
    dir: PathBuf,
    tx: Sender<DocChange>,
    ctl: Sender<Msg>,
    ctl_rx: Receiver<Msg>,
    wake: Box<dyn Fn() + Send>,
    started_at: SystemTime,
    native: bool,
    mode: Arc<AtomicU8>,
}

/// The operating system's watch on the docs folder, or on its nearest existing parent while the folder is missing.
struct Native {
    watcher: RecommendedWatcher,
    /// Set while the docs folder does not exist yet and its parent is watched instead.
    parent: Option<PathBuf>,
}

impl Native {
    fn start(dir: &Path, ctl: Sender<Msg>) -> Option<Self> {
        let mut watcher = notify::recommended_watcher(move |event| {
            let _ = ctl.send(Msg::Event(event));
        })
        .ok()?;
        let parent = arm(&mut watcher, dir).ok()?;
        Some(Self { watcher, parent })
    }

    /// Once the docs folder exists, watch it itself instead of its parent.
    fn follow_folder(&mut self, dir: &Path) {
        let Some(parent) = &self.parent else { return };
        if dir.is_dir() {
            let _ = self.watcher.unwatch(parent);
            if let Ok(parent) = arm(&mut self.watcher, dir) {
                self.parent = parent;
            }
        }
    }
}

/// Watches `dir` (with everything below it) and returns `None`, or watches its nearest existing parent and returns it.
fn arm(watcher: &mut RecommendedWatcher, dir: &Path) -> notify::Result<Option<PathBuf>> {
    if dir.is_dir() {
        watcher.watch(dir, RecursiveMode::Recursive)?;
        return Ok(None);
    }
    let parent = dir.ancestors().skip(1).find(|p| p.is_dir()).ok_or_else(|| notify::Error::generic("no existing parent folder"))?;
    watcher.watch(parent, RecursiveMode::NonRecursive)?;
    Ok(Some(parent.to_path_buf()))
}

fn run(job: Job) {
    let Job { dir, tx, ctl, ctl_rx, wake, started_at, native, mode } = job;
    // Armed before the first look, so a change between the two is seen by one or the other.
    let mut watch = if native { Native::start(&dir, ctl) } else { None };
    mode.store(if watch.is_some() { MODE_NATIVE } else { MODE_POLLING }, Ordering::Release);
    let mut known = baseline(&dir, started_at);
    if check(&dir, &mut known, &tx) {
        wake();
    }
    loop {
        let message = if watch.is_some() { ctl_rx.recv().map_err(|_| RecvTimeoutError::Disconnected) } else { ctl_rx.recv_timeout(POLL) };
        match message {
            Ok(Msg::Stop) | Err(RecvTimeoutError::Disconnected) => return,
            Err(RecvTimeoutError::Timeout) => {}
            Ok(Msg::Event(first)) => {
                let mut failed = first.is_err();
                let mut relevant = first.as_ref().map_or(true, is_relevant);
                // Gather the rest of this burst of events before looking at the folder once.
                let burst_end = Instant::now() + BATCH_MAX;
                while Instant::now() < burst_end {
                    match ctl_rx.recv_timeout(BATCH_QUIET) {
                        Ok(Msg::Stop) | Err(RecvTimeoutError::Disconnected) => return,
                        Ok(Msg::Event(next)) => {
                            failed |= next.is_err();
                            relevant |= next.as_ref().map_or(true, is_relevant);
                        }
                        Err(RecvTimeoutError::Timeout) => break,
                    }
                }
                if failed {
                    // The system watcher broke (queue overflow, folder removed): look on a timer from now on.
                    watch = None;
                    mode.store(MODE_POLLING, Ordering::Release);
                } else if !relevant {
                    continue;
                }
                if let Some(native) = &mut watch {
                    native.follow_folder(&dir);
                }
            }
        }
        if check(&dir, &mut known, &tx) {
            wake();
        }
    }
}

/// Events about hidden files (the lock file, editors' swap files) cannot change a page.
fn is_relevant(event: &notify::Event) -> bool {
    event.paths.is_empty() || event.paths.iter().any(|p| !p.file_name().is_some_and(|n| n.to_string_lossy().starts_with('.')))
}
