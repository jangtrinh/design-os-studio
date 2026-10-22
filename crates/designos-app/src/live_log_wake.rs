//! Wakes an idle window when an agent writes to its live edit log.
//!
//! The editor reads `doc-patch.jsonl` (and `live-spec.json`) inside a frame, and a window with nothing to
//! animate runs none. The Agents panel keeps frames coming with its own timer, but with that panel closed an
//! agent's edit would wait for the next mouse or key event before the editor even looks at the log. A
//! background thread looks at the files off the UI thread and asks egui for ONE repaint when one changed, so
//! an idle window stays idle (no timer on the UI thread, 0% CPU), the way the library file's `DiskWatcher`
//! does for edits made outside the editor.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, SystemTime};

/// How often the files are looked at: one `stat` per file, off the UI thread.
const POLL: Duration = Duration::from_millis(500);

/// Length and modification time: an append changes the first, a recreated or rewritten log the second.
type Signature = (u64, Option<SystemTime>);

struct Shared {
    /// The files to watch, and whether the owner is gone.
    state: Mutex<(Vec<PathBuf>, bool)>,
    cvar: Condvar,
}

/// Owns the watching thread; dropping it stops the thread. Starts on the first [`LogWake::watch`].
#[derive(Default)]
pub struct LogWake {
    shared: Option<Arc<Shared>>,
    /// The thread could not be started: not tried again by this owner (a rebound log gets a new one).
    start_failed: bool,
}

impl std::fmt::Debug for LogWake {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LogWake").field("running", &self.shared.is_some()).finish()
    }
}

impl Drop for LogWake {
    fn drop(&mut self) {
        if let Some(shared) = &self.shared {
            if let Ok(mut state) = shared.state.lock() {
                state.1 = true;
                shared.cvar.notify_all();
            }
        }
    }
}

impl LogWake {
    /// Watches exactly `paths` from now on (call it every frame: it only does work when they changed).
    /// `ctx` is woken once per change.
    pub fn watch(&mut self, ctx: &egui::Context, paths: &[&Path]) {
        self.watch_with(ctx, paths, |job| std::thread::Builder::new().name("designos-live-log-wake".into()).spawn(job).map(drop));
    }

    /// [`Self::watch`] with the thread start injected, so a failing start can be tested.
    fn watch_with(&mut self, ctx: &egui::Context, paths: &[&Path], spawn: impl FnOnce(Box<dyn FnOnce() + Send>) -> std::io::Result<()>) {
        if self.start_failed {
            return;
        }
        if let Some(shared) = &self.shared {
            let Ok(mut state) = shared.state.lock() else { return };
            if !state.0.iter().map(PathBuf::as_path).eq(paths.iter().copied()) {
                state.0 = paths.iter().map(|p| p.to_path_buf()).collect();
            }
            return;
        }
        let shared = Arc::new(Shared { state: Mutex::new((paths.iter().map(|p| p.to_path_buf()).collect(), false)), cvar: Condvar::new() });
        let (worker, ctx) = (shared.clone(), ctx.clone());
        match spawn(Box::new(move || run(&worker, &ctx))) {
            Ok(()) => self.shared = Some(shared),
            Err(e) => {
                self.start_failed = true;
                eprintln!("designos: agent edits will show on the next input, the log watcher did not start ({e})");
            }
        }
    }

    /// Whether the thread is running (tests).
    #[cfg(test)]
    pub fn is_running(&self) -> bool {
        self.shared.is_some()
    }
}

fn run(shared: &Shared, ctx: &egui::Context) {
    let mut seen: HashMap<PathBuf, Option<Signature>> = HashMap::new();
    loop {
        let paths = {
            let Ok(guard) = shared.state.lock() else { return };
            let Ok((guard, _)) = shared.cvar.wait_timeout(guard, POLL) else { return };
            if guard.1 {
                return;
            }
            guard.0.clone()
        };
        seen.retain(|p, _| paths.contains(p));
        let mut changed = false;
        for path in paths {
            let now = std::fs::metadata(&path).ok().map(|m| (m.len(), m.modified().ok()));
            match seen.insert(path, now) {
                // First look at a file that exists: it may hold lines no frame has read yet.
                None => changed |= now.is_some(),
                Some(before) => changed |= before != now,
            }
        }
        if changed {
            ctx.request_repaint();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Instant;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("designos-log-wake-{tag}-{}", crate::live_sync_tests::unique_tab_id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// A context that counts the repaints requested from outside a frame.
    fn counting_ctx() -> (egui::Context, Arc<AtomicUsize>) {
        let ctx = egui::Context::default();
        // egui asks for a few repaints of its own while it starts up (fonts, textures): not the watcher's.
        for _ in 0..4 {
            let _ = ctx.run_ui(egui::RawInput::default(), |_| {});
        }
        let wakeups = Arc::new(AtomicUsize::new(0));
        let seen = wakeups.clone();
        ctx.set_request_repaint_callback(move |info| {
            if info.delay == Duration::ZERO {
                seen.fetch_add(1, Ordering::SeqCst);
            }
        });
        (ctx, wakeups)
    }

    /// Runs the frames the event loop would run after each wake-up: egui tells the callback about a repaint
    /// request only once per frame, so a second wake-up is invisible until a frame has run.
    fn pump(ctx: &egui::Context, span: Duration) {
        let end = Instant::now() + span;
        while Instant::now() < end {
            let _ = ctx.run_ui(egui::RawInput::default(), |_| {});
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    fn wait_until(ctx: &egui::Context, what: &str, mut done: impl FnMut() -> bool) {
        // Generous: a loaded machine runs a debug build slowly.
        let deadline = Instant::now() + Duration::from_secs(20);
        while !done() {
            assert!(Instant::now() < deadline, "timed out waiting for {what}");
            pump(ctx, Duration::from_millis(20));
        }
    }

    /// Wake-ups so far, once the watcher has been quiet for a few polls.
    fn quiet_count(ctx: &egui::Context, wakeups: &AtomicUsize) -> usize {
        pump(ctx, POLL * 2);
        wakeups.load(Ordering::SeqCst)
    }

    #[test]
    fn an_append_to_the_log_wakes_the_window_and_an_idle_log_never_does() {
        let dir = temp_dir("append");
        let log = dir.join("doc-patch.jsonl");
        std::fs::write(&log, b"").unwrap();
        let (ctx, wakeups) = counting_ctx();
        let mut wake = LogWake::default();
        wake.watch(&ctx, &[&log]);
        // The first look at an existing file wakes once (it may hold unread lines); after that it stays quiet.
        wait_until(&ctx, "the first look", || wakeups.load(Ordering::SeqCst) >= 1);
        let settled = quiet_count(&ctx, &wakeups);
        assert_eq!(quiet_count(&ctx, &wakeups), settled, "an unchanged log must not wake the window, on a timer or otherwise");

        use std::io::Write;
        let mut f = std::fs::OpenOptions::new().append(true).open(&log).unwrap();
        f.write_all(b"{\"patch\":1}\n").unwrap();
        drop(f);
        wait_until(&ctx, "the append", || wakeups.load(Ordering::SeqCst) > settled);
        let after_append = quiet_count(&ctx, &wakeups);
        assert_eq!(quiet_count(&ctx, &wakeups), after_append, "the wake-ups stop once the log stops changing");
        drop(wake);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_log_that_appears_later_wakes_the_window() {
        let dir = temp_dir("appear");
        let log = dir.join("doc-patch.jsonl");
        let (ctx, wakeups) = counting_ctx();
        let mut wake = LogWake::default();
        wake.watch(&ctx, &[&log]);
        pump(&ctx, POLL * 2);
        assert_eq!(wakeups.load(Ordering::SeqCst), 0, "no file, nothing to read");
        std::fs::write(&log, b"{\"patch\":1}\n").unwrap();
        wait_until(&ctx, "the log to appear", || wakeups.load(Ordering::SeqCst) >= 1);
        drop(wake);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_failed_thread_start_is_tried_once_not_on_every_frame() {
        let ctx = egui::Context::default();
        let attempts = std::cell::Cell::new(0);
        let failing = |_job: Box<dyn FnOnce() + Send>| {
            attempts.set(attempts.get() + 1);
            Err(std::io::Error::other("no threads left"))
        };
        let mut wake = LogWake::default();
        for _ in 0..5 {
            wake.watch_with(&ctx, &[Path::new("doc-patch.jsonl")], failing);
        }
        assert_eq!(attempts.get(), 1, "a frame must not retry a thread start that failed");
        assert!(!wake.is_running());
        // A rebound log starts from a fresh `LogWake`, which tries again.
        let mut rebound = LogWake::default();
        rebound.watch_with(&ctx, &[Path::new("doc-patch.jsonl")], failing);
        assert_eq!(attempts.get(), 2);
    }

    #[test]
    fn dropping_the_owner_stops_the_thread() {
        let dir = temp_dir("drop");
        let log = dir.join("doc-patch.jsonl");
        let (ctx, wakeups) = counting_ctx();
        let mut wake = LogWake::default();
        wake.watch(&ctx, &[&log]);
        let shared = wake.shared.clone().expect("started");
        drop(wake);
        // The thread held one handle; once it has returned only this test's handle is left.
        wait_until(&ctx, "the thread to stop", || Arc::strong_count(&shared) == 1);
        std::fs::write(&log, b"x\n").unwrap();
        pump(&ctx, POLL * 2);
        assert_eq!(wakeups.load(Ordering::SeqCst), 0, "a stopped watcher wakes nobody");
        let _ = std::fs::remove_dir_all(dir);
    }
}
