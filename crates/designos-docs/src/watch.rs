//! Reports external edits to a docs folder from a background thread, in the style of the library file's
//! `DiskWatcher`: the thread sleeps on a condvar and stats the folder every `POLL`; the caller never touches the
//! disk and only drains an in-memory queue (and is woken through a callback, e.g. `ctx.request_repaint()`).
//!
//! Studio's own writes are reported too; recognise them by the hash `DocsStore::write_*` returned.

use crate::hash::ContentHash;
use crate::manifest::MANIFEST_FILE;
use crate::paths::page_path_problem;
use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, SystemTime};

/// How often the watcher thread looks at the folder. An edit is reported within this plus the time to hash it.
const POLL: Duration = Duration::from_millis(400);
const MAX_DEPTH: usize = 8;

/// A page (or the manifest) that changed on disk.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DocChange {
    /// Path relative to the docs folder, `/`-separated.
    pub rel: String,
    /// Hash of the new content, or `None` when the file was deleted.
    pub hash: Option<ContentHash>,
}

struct Shared {
    stop: Mutex<bool>,
    wake: Condvar,
    stopped: Arc<AtomicBool>,
}

pub struct DocsWatcher {
    rx: Mutex<Receiver<DocChange>>,
    shared: Arc<Shared>,
}

type Stat = (Option<SystemTime>, u64);

impl DocsWatcher {
    /// Starts watching `dir` (it may not exist yet). `wake` runs on the watcher thread after each batch of
    /// changes is queued. The folder's current state is the baseline, taken before this returns.
    pub fn start(dir: impl Into<std::path::PathBuf>, wake: impl Fn() + Send + 'static) -> Self {
        let dir = dir.into();
        let (tx, rx) = channel();
        let shared = Arc::new(Shared { stop: Mutex::new(false), wake: Condvar::new(), stopped: Arc::new(AtomicBool::new(false)) });
        let baseline: HashMap<String, Known> = scan(&dir).into_iter().map(|(rel, stat)| (rel, Known { stat, hash: None })).collect();
        let thread_shared = shared.clone();
        let spawned = std::thread::Builder::new().name("designos-docs-watch".into()).spawn(move || {
            run(&dir, baseline, &tx, &wake, &thread_shared);
            thread_shared.stopped.store(true, Ordering::Release);
        });
        if spawned.is_err() {
            // No thread, no events; the caller sees a watcher that reports nothing and reads the flag.
            shared.stopped.store(true, Ordering::Release);
        }
        Self { rx: Mutex::new(rx), shared }
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

    /// Becomes true once the watcher thread has exited (after this watcher is dropped, or if it could not start).
    pub fn stopped_flag(&self) -> Arc<AtomicBool> {
        self.shared.stopped.clone()
    }
}

impl Drop for DocsWatcher {
    fn drop(&mut self) {
        if let Ok(mut stop) = self.shared.stop.lock() {
            *stop = true;
            self.shared.wake.notify_all();
        }
    }
}

struct Known {
    stat: Stat,
    /// Hash last reported (or seen) for this file; `None` until the first change.
    hash: Option<ContentHash>,
}

fn run(dir: &Path, mut known: HashMap<String, Known>, tx: &Sender<DocChange>, wake: &(impl Fn() + ?Sized), shared: &Shared) {
    loop {
        {
            let Ok(guard) = shared.stop.lock() else { return };
            let Ok((guard, _)) = shared.wake.wait_timeout_while(guard, POLL, |stop| !*stop) else { return };
            if *guard {
                return;
            }
        }
        let now = scan(dir);
        let mut sent = false;
        let mut send = |change: DocChange| sent |= tx.send(change).is_ok();
        for (rel, stat) in &now {
            if known.get(rel).is_some_and(|k| k.stat == *stat) {
                continue;
            }
            // Read failure means it vanished again or is mid-rename: the next pass sees the settled state.
            let Ok(bytes) = std::fs::read(dir.join(rel)) else { continue };
            let hash = ContentHash::of(&bytes);
            let unchanged = known.get(rel).is_some_and(|k| k.hash == Some(hash));
            known.insert(rel.clone(), Known { stat: *stat, hash: Some(hash) });
            if !unchanged {
                send(DocChange { rel: rel.clone(), hash: Some(hash) });
            }
        }
        let gone: Vec<String> = known.keys().filter(|rel| !now.contains_key(*rel)).cloned().collect();
        for rel in gone {
            known.remove(&rel);
            send(DocChange { rel, hash: None });
        }
        if sent {
            wake();
        }
    }
}

/// Stat of every watched file: pages and the manifest. Hidden names, temp files and other extensions are not
/// watched. A missing folder is an empty one.
fn scan(dir: &Path) -> HashMap<String, Stat> {
    let mut out = HashMap::new();
    scan_into(dir, "", 0, &mut out);
    out
}

fn scan_into(dir: &Path, prefix: &str, depth: usize, out: &mut HashMap<String, Stat>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            continue;
        }
        let rel = format!("{prefix}{name}");
        let Ok(kind) = entry.file_type() else { continue };
        if kind.is_dir() {
            if depth < MAX_DEPTH {
                scan_into(&entry.path(), &format!("{rel}/"), depth + 1, out);
            }
        } else if kind.is_file() && (rel == MANIFEST_FILE || page_path_problem(&rel).is_none()) {
            if let Ok(meta) = entry.metadata() {
                out.insert(rel, (meta.modified().ok(), meta.len()));
            }
        }
    }
}
