//! Per-window / per-session agent file management, ephemeral session directory cleanup,
//! 7-day startup sweep with PID + start-time ownership markers, and off-UI-thread canvas
//! snapshot writer (`#690`). Where session folders live and how they are made: [`folder`].

mod folder;
#[cfg(unix)]
mod folder_owner;
#[cfg(unix)]
mod no_follow;
#[cfg(not(unix))]
#[path = "agent_session_files/no_follow_fallback.rs"]
mod no_follow;
mod refusal;
mod session_io;
pub use folder::{default_sessions_root, ephemeral_session_dir, ephemeral_sessions_root, sessions_root_beside, unavailable_path};
pub use refusal::{files_not_updated_message, library_fallback_message, start_failure_message, with_next_step, with_set_aside_note};
pub use session_io::{
    added_while_starting, atomic_write_session_file, create_session_dir_all, create_session_file_exclusive, open_plain_file, open_session_file,
    read_session_file, read_session_text, remove_session_dir_all, remove_session_file, rename_session_file, session_file_metadata, set_aside_entries_except,
    set_aside_path, write_session_file_unless, write_session_file_verified, SetAside,
};

use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::thread::JoinHandle;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use designos_doc::Document;
use serde::{Deserialize, Serialize};

/// Ephemeral session directories in [`ephemeral_sessions_root`] older than 7 days whose owning
/// Studio process is no longer alive are removed by the startup sweep (`#690` §5).
pub const STALE_SESSION_MAX_AGE: Duration = Duration::from_secs(7 * 24 * 60 * 60);

/// Ownership marker written to `<session_dir>/owner.json` when an ephemeral session directory is created.
/// Recording both `pid` and `started_ms` guards against OS PID recycling (`#690` §5).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OwnerMarker {
    pub pid: u32,
    pub started_ms: u64,
}

/// Returns this Studio process's start timestamp in Unix epoch milliseconds (cached for the lifetime
/// of the process).
pub fn current_process_started_ms() -> u64 {
    static STARTED_MS: OnceLock<u64> = OnceLock::new();
    *STARTED_MS.get_or_init(|| {
        query_process_started_ms(std::process::id()).unwrap_or_else(|| SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0))
    })
}

/// Parses a POSIX `ps -o etime=` elapsed-time string (`[[dd-]hh:]mm:ss`) into elapsed seconds.
pub(crate) fn parse_ps_etime_secs(s: &str) -> Option<u64> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return None;
    }
    let (days, rest) = match trimmed.split_once('-') {
        Some((d, r)) => (d.trim().parse::<u64>().ok()?, r.trim()),
        None => (0, trimmed),
    };
    let parts: Vec<&str> = rest.split(':').collect();
    let (hours, mins, secs) = match parts.as_slice() {
        [mm, ss] => (0, mm.trim().parse::<u64>().ok()?, ss.trim().parse::<u64>().ok()?),
        [hh, mm, ss] => (hh.trim().parse::<u64>().ok()?, mm.trim().parse::<u64>().ok()?, ss.trim().parse::<u64>().ok()?),
        _ => return None,
    };
    Some(days.saturating_mul(86_400).saturating_add(hours.saturating_mul(3_600)).saturating_add(mins.saturating_mul(60)).saturating_add(secs))
}

/// Queries the OS for the start timestamp (in Unix epoch ms) of `pid`, when supported.
fn query_process_started_ms(pid: u32) -> Option<u64> {
    if pid == 0 {
        return None;
    }
    #[cfg(unix)]
    {
        // Try `ps -o etimes= -p <pid>` (elapsed seconds on Linux procps) first, falling back to
        // POSIX `ps -o etime= -p <pid>` (`[[dd-]hh:]mm:ss`, supported on both macOS BSD `ps` and procps).
        let elapsed_secs = std::process::Command::new("ps")
            .args(["-o", "etimes=", "-p", &pid.to_string()])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .output()
            .ok()
            .filter(|out| out.status.success())
            .and_then(|out| String::from_utf8_lossy(&out.stdout).trim().parse::<u64>().ok())
            .or_else(|| {
                let out = std::process::Command::new("ps")
                    .args(["-o", "etime=", "-p", &pid.to_string()])
                    .stdout(std::process::Stdio::piped())
                    .stderr(std::process::Stdio::null())
                    .output()
                    .ok()?;
                if !out.status.success() {
                    return None;
                }
                parse_ps_etime_secs(&String::from_utf8_lossy(&out.stdout))
            })?;
        let now_ms = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0);
        Some(now_ms.saturating_sub(elapsed_secs.saturating_mul(1000)))
    }
    #[cfg(not(unix))]
    {
        let _ = pid;
        None
    }
}

/// Verifies that `started_at` (in Unix epoch seconds or milliseconds) matches the running OS
/// process `pid` within 60 seconds, guarding against OS PID recycling (`#690` / `#653`).
pub fn is_pid_start_time_matching(pid: u32, started_at: u64) -> bool {
    if started_at == 0 {
        return true;
    }
    let expected_ms = if started_at >= 1_000_000_000_000 { started_at } else { started_at.saturating_mul(1000) };
    if pid == std::process::id() {
        let ours = current_process_started_ms();
        return ours.abs_diff(expected_ms) <= 60_000;
    }
    if let Some(actual) = query_process_started_ms(pid) {
        // Allow generous clock/ps rounding slack (60s); a recycled PID days later differs by days.
        return actual.abs_diff(expected_ms) <= 60_000;
    }
    true
}

/// Default liveness predicate for [`sweep_stale_sessions`]: checks whether `pid` is alive and, when
/// `started_ms` is known, verifies that `pid` was not recycled by a much newer process.
pub fn is_owner_alive(pid: u32, expected_started_ms: Option<u64>) -> bool {
    if !crate::agent_sessions::is_pid_alive(pid) {
        return false;
    }
    let Some(expected) = expected_started_ms else {
        return true;
    };
    is_pid_start_time_matching(pid, expected)
}

/// Returns `true` iff `dir` is a direct child of [`ephemeral_sessions_root`] and has no `"agent-sessions"` path
/// component (`#690` §5). The folder is recognised by its exact location, not by a folder name: callers delete what
/// this accepts, and a folder in the temp folder (where earlier versions kept sessions) is never one of them.
///
/// Kept per-file session directories under `<lib_root>/agent-sessions/<file_id>` (`#653`) always
/// return `false`.
pub fn is_ephemeral_session_dir(dir: &Path) -> bool {
    if dir.components().any(|c| c.as_os_str() == "agent-sessions") {
        return false;
    }
    ephemeral_sessions_root().is_some_and(|root| dir.parent() == Some(root.as_path()))
}

/// Writes `<session_dir>/owner.json` with `{ "pid": <current pid>, "started_ms": <started_ms> }`.
pub fn write_owner_marker(session_dir: &Path) -> std::io::Result<()> {
    let marker = OwnerMarker { pid: std::process::id(), started_ms: current_process_started_ms() };
    let bytes = serde_json::to_vec_pretty(&marker).map_err(std::io::Error::other)?;
    atomic_write_session_file(&session_dir.join("owner.json"), &bytes)
}

/// Reads `<session_dir>/owner.json` if present and valid.
pub fn read_owner_marker(session_dir: &Path) -> Option<OwnerMarker> {
    let raw = read_session_file(&session_dir.join("owner.json")).ok()?;
    serde_json::from_slice::<OwnerMarker>(&raw).ok()
}

/// Creates `session_dir` and, when `session_dir` is an ephemeral session directory (a child of
/// [`ephemeral_sessions_root`]) without an `owner.json` marker yet, writes `<session_dir>/owner.json`.
///
/// The folder is made with an exclusive `mkdir` (0700). A path that exists already is accepted only when it is a real
/// folder owned by the user, so an existing link, file or foreign folder is an error and nothing is written there.
pub fn ensure_session_dir(session_dir: &Path) -> std::io::Result<()> {
    folder::create_session_folder(session_dir)?;
    if is_ephemeral_session_dir(session_dir) && !session_dir.join("owner.json").exists() {
        write_owner_marker(session_dir)?;
    }
    Ok(())
}

/// Extracts `<pid>` from a folder name starting with `p<pid>-` (e.g. `p12345-tab-1` or `p12345-w1-tab-1`).
pub fn parse_pid_from_folder_name(name: &str) -> Option<u32> {
    let rest = name.strip_prefix('p')?;
    let (digits, _) = rest.split_once('-')?;
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let pid = digits.parse::<u32>().ok()?;
    (pid > 0).then_some(pid)
}

fn entry_mtime(dir: &Path) -> Option<SystemTime> {
    let dir_mtime = std::fs::metadata(dir).and_then(|m| m.modified()).ok();
    #[cfg(windows)]
    {
        let marker_mtime = std::fs::metadata(dir.join("owner.json")).and_then(|m| m.modified()).ok();
        match (dir_mtime, marker_mtime) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (Some(a), None) => Some(a),
            (None, Some(b)) => Some(b),
            (None, None) => None,
        }
    }
    #[cfg(not(windows))]
    {
        dir_mtime
    }
}

/// Sweeps stale ephemeral session folders under `root` ([`ephemeral_sessions_root`]) and returns
/// the number of directories removed (`#690` §5). Folders that earlier versions left in the temp folder are not
/// swept: nothing is deleted through a path an agent could have redirected.
///
/// Safety invariants:
/// 1. Returns `0` immediately unless `root` is named `"sessions"` and has no
///    `"agent-sessions"` path component — `<lib_root>/agent-sessions/<file_id>` (`#653`) is never
///    touched.
/// 2. Removes an entry `<dir>` only when **both** hold:
///    - `<dir>`'s `mtime` is strictly older than 7 days (`now.duration_since(mtime) > 7 days`), and
///    - `<dir>` has either `<dir>/owner.json` or a `p<pid>-...` folder name whose `pid` is reported
///      dead by `is_alive(pid, started_ms)`.
/// 3. Foreign directories (neither `owner.json` nor `p<pid>-...` name) are never removed.
pub fn sweep_stale_sessions(root: &Path, now: SystemTime, is_alive: impl Fn(u32, Option<u64>) -> bool) -> usize {
    if root.file_name().and_then(|n| n.to_str()) != Some("sessions") {
        return 0;
    }
    if root.components().any(|c| c.as_os_str() == "agent-sessions") {
        return 0;
    }
    let Ok(entries) = std::fs::read_dir(root) else {
        return 0;
    };
    let mut removed = 0usize;
    for entry in entries.flatten() {
        let Ok(ft) = entry.file_type() else {
            continue;
        };
        if !ft.is_dir() {
            continue;
        }
        let dir = entry.path();
        let Some(mtime) = entry_mtime(&dir) else {
            continue;
        };
        let Ok(age) = now.duration_since(mtime) else {
            continue;
        };
        if age <= STALE_SESSION_MAX_AGE {
            continue;
        }
        let owner = if let Some(marker) = read_owner_marker(&dir) {
            Some((marker.pid, Some(marker.started_ms)))
        } else {
            dir.file_name().and_then(|n| n.to_str()).and_then(parse_pid_from_folder_name).map(|pid| (pid, None))
        };
        let Some((pid, started_ms)) = owner else {
            // Foreign directory without owner.json or p<pid>- prefix: leave untouched.
            continue;
        };
        if is_alive(pid, started_ms) {
            continue;
        }
        if std::fs::remove_dir_all(&dir).is_ok() {
            // The session's approval channel lives outside this folder (Studio-private); nobody uses it any more.
            crate::agent_approval_channel::remove_channel(&dir);
            removed += 1;
        }
    }
    removed
}

/// Spawns the one-shot background startup sweep of [`ephemeral_sessions_root`].
pub fn spawn_startup_sweep() {
    let Some(root) = ephemeral_sessions_root() else {
        return;
    };
    let _ = std::thread::Builder::new().name("designos-session-sweep".into()).spawn(move || {
        let _ = sweep_stale_sessions(&root, SystemTime::now(), is_owner_alive);
    });
}

/// One coalesced off-UI-thread snapshot job for [`SnapshotWorker`] (`#690` §6).
pub struct SnapshotJob {
    pub session_dir: PathBuf,
    pub session_id: String,
    pub key: (u64, u64, u64),
    pub context_json_path: PathBuf,
    pub live_canvas_path: PathBuf,
    pub live_spec_path: PathBuf,
    pub live_doc_sync_path: PathBuf,
    pub live_doc_patch_path: PathBuf,
    pub context_bytes: Vec<u8>,
    pub canvas_doc: Document,
    pub doc_name: String,
    pub current_page_id: u64,
    pub current_page_name: String,
    pub file_path: Option<PathBuf>,
    /// Run id and runtime of the active agent tab, written into the session's MCP server env.
    pub run_id: String,
    /// Agent run counter whose first-turn `bootstrap.delivered.<n>` marker the Claude prompt hook checks (in the hook folder, `agent_hook_outputs`).
    pub bootstrap_run_id: u64,
    pub agent_runtime: String,
    /// True when the session runs in `Ask` or `Ask for risky only`, so its hooks route approvals through the panel.
    pub ask_mode: bool,
    /// True when read and additive designos tools may auto-allow inside `approve-hook.sh`.
    pub ask_risky: bool,
    /// Read position of the session's edit log when the job was queued; the writer asks it again just
    /// before the canvas replaces the old one and holds the publish when the agent appended since.
    pub unread_probe: Option<crate::live_patch_log::UnreadProbe>,
    /// Set when an Ask-mode tab of Grok or Antigravity is about to start from this snapshot: its project config is then made
    /// exactly Studio's (see [`crate::agent_project_config`]) and the result of that is the one the write returns. `None` for every
    /// other write, a periodic sync included.
    pub exact_project_config: Option<ExactProjectConfig>,
}

/// A CLI that loads hooks, MCP servers and plugins as commands from the project folder of its working directory, which is a session
/// folder an agent writes, and that runs them as the user.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExactProjectConfig {
    Grok,
    Antigravity,
}

/// How an executed [`SnapshotJob`] ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SnapshotOutcome {
    Written,
    /// The agent appended an edit the editor has not read yet, so the old canvas stays in place and
    /// the publish is retried on a later frame.
    Held,
}

pub type SyncedSnapshotKey = (PathBuf, (u64, u64, u64));

/// Retry pacing for snapshot writes that failed (disk full, folder not writable; spec edge case 5). The same
/// content is tried again only after a pause that doubles from 1 s up to 30 s; a change of the published
/// content is tried at once. The error text is news only the first time it happens for a given content.
#[derive(Default)]
pub struct WriteBackoff {
    failed: Option<FailedWrite>,
}

struct FailedWrite {
    key: (u64, u64, u64),
    retry_at: Instant,
    attempts: u32,
    message: String,
}

impl WriteBackoff {
    /// True while a snapshot of `key` must not be queued because its last write failed a moment ago.
    pub fn blocks(&self, key: (u64, u64, u64), now: Instant) -> bool {
        self.failed.as_ref().is_some_and(|f| f.key == key && now < f.retry_at)
    }

    /// Records a failed write of `key`. Returns whether `message` is news: false when the same content
    /// already failed with the same message, so the status line is not rewritten for every retry.
    pub fn record_failure(&mut self, key: (u64, u64, u64), message: &str, now: Instant) -> bool {
        let previous = self.failed.take().filter(|f| f.key == key);
        let attempts = previous.as_ref().map_or(1, |f| f.attempts.saturating_add(1));
        let news = previous.is_none_or(|f| f.message != message);
        let pause = Duration::from_secs((1u64 << (attempts - 1).min(5)).min(30));
        self.failed = Some(FailedWrite { key, retry_at: now + pause, attempts, message: message.to_string() });
        news
    }

    /// Forgets the failure: a write succeeded, or the terminal was bound to another folder.
    pub fn clear(&mut self) {
        self.failed = None;
    }
}

#[derive(Default)]
pub struct SnapshotStatus {
    pub completed: Option<SyncedSnapshotKey>,
    pub failed: Option<SyncedSnapshotKey>,
    pub error: Option<String>,
}

#[derive(Default)]
struct WorkerShared {
    latest: Option<SnapshotJob>,
    in_flight: bool,
    generation: u64,
    completed_key: Option<SyncedSnapshotKey>,
    failed_key: Option<SyncedSnapshotKey>,
    last_error: Option<String>,
    shutdown: bool,
    #[cfg(test)]
    last_writer_thread: Option<std::thread::ThreadId>,
    #[cfg(test)]
    jobs_executed: u64,
}

type WorkerSync = Arc<(Mutex<WorkerShared>, Condvar, Condvar)>;

/// Single background writer thread per `TerminalState` with latest-wins coalescing (`#690` §6).
pub struct SnapshotWorker {
    shared: WorkerSync,
    thread: Option<JoinHandle<()>>,
}

impl Default for SnapshotWorker {
    fn default() -> Self {
        Self::new()
    }
}

impl SnapshotWorker {
    pub fn new() -> Self {
        Self { shared: Arc::new((Mutex::new(WorkerShared::default()), Condvar::new(), Condvar::new())), thread: None }
    }

    fn ensure_thread(&mut self) {
        if self.thread.is_some() {
            return;
        }
        let shared = Arc::clone(&self.shared);
        let handle =
            std::thread::Builder::new().name("designos-live-snapshot".into()).spawn(move || worker_loop(shared)).expect("spawn designos-live-snapshot worker");
        self.thread = Some(handle);
    }

    /// Enqueues `job` into the single-slot latest-wins queue and wakes the background worker without
    /// blocking the UI thread.
    pub fn enqueue(&mut self, job: SnapshotJob) {
        self.ensure_thread();
        let (lock, work_cv, _) = &*self.shared;
        let replaced = if let Ok(mut state) = lock.lock() {
            let prev = state.latest.replace(job);
            state.completed_key = None;
            state.failed_key = None;
            state.last_error = None;
            work_cv.notify_one();
            prev
        } else {
            None
        };
        drop(replaced);
    }

    /// Takes any completed/failed `(session_dir, key)` and error reported by the worker since the last poll.
    pub fn take_status(&self) -> SnapshotStatus {
        if self.thread.is_none() {
            return SnapshotStatus::default();
        }
        let (lock, _, _) = &*self.shared;
        let Ok(mut state) = lock.lock() else {
            return SnapshotStatus::default();
        };
        SnapshotStatus { completed: state.completed_key.take(), failed: state.failed_key.take(), error: state.last_error.take() }
    }

    /// Cancels any unstarted job and waits for any currently in-flight write to finish so the caller
    /// can safely remove an ephemeral `session_dir`.
    pub fn cancel_and_wait(&self) {
        if self.thread.is_none() {
            return;
        }
        let (lock, _, done_cv) = &*self.shared;
        let Ok(mut state) = lock.lock() else {
            return;
        };
        state.generation = state.generation.wrapping_add(1);
        let prev = state.latest.take();
        state.completed_key = None;
        state.failed_key = None;
        state.last_error = None;
        while state.in_flight {
            state = match done_cv.wait(state) {
                Ok(g) => g,
                Err(p) => p.into_inner(),
            };
        }
        drop(state);
        drop(prev);
    }

    /// Blocks until the worker has drained `latest` and finished any in-flight write.
    pub fn flush(&self) {
        if self.thread.is_none() {
            return;
        }
        let (lock, _, done_cv) = &*self.shared;
        let Ok(mut state) = lock.lock() else {
            return;
        };
        while state.latest.is_some() || state.in_flight {
            state = match done_cv.wait(state) {
                Ok(g) => g,
                Err(p) => p.into_inner(),
            };
        }
    }

    #[cfg(test)]
    pub fn last_writer_thread_id(&self) -> Option<std::thread::ThreadId> {
        let (lock, _, _) = &*self.shared;
        lock.lock().ok().and_then(|s| s.last_writer_thread)
    }

    #[cfg(test)]
    pub fn jobs_executed(&self) -> u64 {
        let (lock, _, _) = &*self.shared;
        lock.lock().map(|s| s.jobs_executed).unwrap_or(0)
    }
}

impl Drop for SnapshotWorker {
    fn drop(&mut self) {
        if let Some(handle) = self.thread.take() {
            let (lock, work_cv, _) = &*self.shared;
            let prev = if let Ok(mut state) = lock.lock() {
                state.shutdown = true;
                work_cv.notify_all();
                state.latest.take()
            } else {
                None
            };
            drop(prev);
            let _ = handle.join();
        }
    }
}

fn worker_loop(shared: WorkerSync) {
    let (lock, work_cv, done_cv) = &*shared;
    loop {
        let (job, job_gen) = {
            let mut state = match lock.lock() {
                Ok(g) => g,
                Err(p) => p.into_inner(),
            };
            while state.latest.is_none() && !state.shutdown {
                state = match work_cv.wait(state) {
                    Ok(g) => g,
                    Err(p) => p.into_inner(),
                };
            }
            if state.shutdown {
                state.in_flight = false;
                done_cv.notify_all();
                return;
            }
            let job = state.latest.take().expect("checked Some");
            state.in_flight = true;
            let gen = state.generation;
            (job, gen)
        };

        // A panic while writing must not leave `in_flight` set: every later wait (agent start, close, rebind,
        // flush) would block the UI thread for good. It becomes a failed job and the writer keeps running.
        let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| execute_snapshot_job(&job)))
            .unwrap_or_else(|payload| Err(format!("Snapshot writer panicked: {}", panic_payload_message(payload.as_ref()))));

        let mut state = match lock.lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };
        state.in_flight = false;
        #[cfg(test)]
        {
            state.last_writer_thread = Some(std::thread::current().id());
            state.jobs_executed += 1;
        }
        if state.generation == job_gen && state.latest.is_none() {
            match res {
                Ok(SnapshotOutcome::Written) => {
                    state.completed_key = Some((job.session_dir.clone(), job.key));
                    state.failed_key = None;
                    state.last_error = None;
                }
                Ok(SnapshotOutcome::Held) => {
                    state.completed_key = None;
                    state.failed_key = Some((job.session_dir.clone(), job.key));
                    state.last_error = None;
                }
                Err(err) => {
                    state.completed_key = None;
                    state.failed_key = Some((job.session_dir.clone(), job.key));
                    state.last_error = Some(err);
                }
            }
        }
        if state.latest.is_none() {
            done_cv.notify_all();
        }
    }
}

fn panic_payload_message(payload: &(dyn std::any::Any + Send)) -> String {
    payload.downcast_ref::<&str>().map(|s| (*s).to_string()).or_else(|| payload.downcast_ref::<String>().cloned()).unwrap_or_else(|| "unknown cause".into())
}

fn execute_snapshot_job(job: &SnapshotJob) -> Result<SnapshotOutcome, String> {
    // Every error of the writes below names the path it happened at, so the line names it once.
    ensure_session_dir(&job.session_dir).map_err(|e| files_not_updated_message(&e))?;
    atomic_write_session_file(&job.context_json_path, &job.context_bytes).map_err(|e| files_not_updated_message(&e))?;
    let canvas_json = job.canvas_doc.to_json().map_err(|e| format!("Failed to serialize canvas document: {e}"))?;
    let held = write_session_file_unless(&job.live_canvas_path, canvas_json.as_bytes(), || {
        #[cfg(test)]
        crate::terminal::run_before_rename_test_hook(&job.live_canvas_path);
        job.unread_probe.as_ref().is_some_and(|probe| probe.has_unread_complete_line())
    })
    .map_err(|e| files_not_updated_message(&e))?;
    if held {
        return Ok(SnapshotOutcome::Held);
    }
    crate::terminal::write_session_agent_files_for_job(job).map_err(|e| files_not_updated_message(&e))?;
    Ok(SnapshotOutcome::Written)
}

#[cfg(all(test, unix))]
mod agy_config_tests;
#[cfg(all(test, unix))]
mod canonical_spelling_tests;
#[cfg(all(test, unix))]
mod codex_probe_tests;
#[cfg(all(test, unix))]
mod exact_config_race_tests;
#[cfg(all(test, unix))]
mod fallback_set_aside_tests;
#[cfg(all(test, unix))]
#[allow(dead_code, unused_imports)]
#[path = "agent_session_files/no_follow_fallback.rs"]
mod fallback_under_test;
#[cfg(all(test, unix))]
mod fifo_tests;
#[cfg(all(test, unix))]
mod folder_message_tests;
#[cfg(all(test, unix))]
mod folder_tests;
#[cfg(all(test, unix))]
mod folder_unit_tests;
#[cfg(test)]
mod fs_scan;
#[cfg(test)]
mod fs_sites_scan_tests;
#[cfg(all(test, unix))]
mod gemini_config_tests;
#[cfg(all(test, unix))]
mod grok_config_tests;
#[cfg(all(test, unix))]
mod hook_quoting_tests;
#[cfg(all(test, unix))]
mod inner_link_race_tests;
#[cfg(all(test, unix))]
mod inner_link_tests;
#[cfg(all(test, unix))]
mod library_fallback_tests;
#[cfg(all(test, unix))]
mod linked_parent_tests;
#[cfg(all(test, unix))]
mod live_spec_tests;
#[cfg(all(test, target_vendor = "apple"))]
mod locked_entry_tests;
#[cfg(all(test, unix))]
mod open_race_tests;
#[cfg(all(test, unix))]
mod open_removed_link_tests;
#[cfg(all(test, unix))]
mod placement_tests;
#[cfg(all(test, unix))]
mod plain_open_tests;
#[cfg(all(test, unix))]
mod poll_readers_tests;
#[cfg(all(test, unix))]
mod private_anchor_tests;
#[cfg(test)]
mod refusal_tests;
#[cfg(all(test, unix))]
mod session_io_tests;
#[cfg(all(test, unix))]
mod set_aside_tests;
#[cfg(all(test, unix))]
mod swap_after_open_tests;
#[cfg(all(test, unix))]
mod tailer_link_tests;
#[cfg(test)]
mod test_root;
#[cfg(all(test, unix))]
pub(crate) mod test_support;
#[cfg(test)]
mod unavailable_root_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terminal::{export_selection_and_comments_to_pty, TerminalAction, TerminalState};
    use designos_doc::{Document, Node, Rect};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::Duration;

    fn unique_test_id(tag: &str) -> String {
        static SEQ: AtomicU64 = AtomicU64::new(1);
        let n = SEQ.fetch_add(1, Ordering::Relaxed);
        format!("p{}-{tag}-{n}", std::process::id())
    }

    fn set_dir_mtime(dir: &Path, mtime: SystemTime) {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                if let Ok(f) = std::fs::OpenOptions::new().write(true).open(entry.path()) {
                    let _ = f.set_modified(mtime);
                }
            }
        }
        if let Ok(f) = std::fs::File::open(dir) {
            let _ = f.set_modified(mtime);
        }
    }

    #[test]
    fn two_windows_never_touch_each_others_session_files() {
        let mut win1 = TerminalState::for_tab(1);
        win1.bind_session(&format!("p{}-w1-tab-1", std::process::id()));
        let mut win2 = TerminalState::for_tab(1);
        win2.bind_session(&format!("p{}-w2-tab-1", std::process::id()));
        assert_ne!(win1.session_dir, win2.session_dir);

        let mut doc1 = Document::new("Window One Doc");
        let p1 = doc1.add_page("Page A");
        let f1 = doc1.alloc_id();
        doc1.insert(p1, Node::frame(f1, Rect::new(0.0, 0.0, 200.0, 120.0))).unwrap();
        doc1.get_mut(f1).unwrap().name = "Frame One".into();

        let mut doc2 = Document::new("Window Two Doc");
        let p2 = doc2.add_page("Page B");
        let f2 = doc2.alloc_id();
        doc2.insert(p2, Node::frame(f2, Rect::new(10.0, 10.0, 300.0, 180.0))).unwrap();
        doc2.get_mut(f2).unwrap().name = "Frame Two".into();

        win1.sync_live_file_context(&doc1, p1, &[f1], None);
        win2.sync_live_file_context(&doc2, p2, &[f2], None);
        win1.flush_live_sync();
        win2.flush_live_sync();

        let sel1 = export_selection_and_comments_to_pty(&mut win1, &doc1, p1, &[f1], None).expect("export win1");
        let sel2 = export_selection_and_comments_to_pty(&mut win2, &doc2, p2, &[f2], None).expect("export win2");
        assert_eq!(sel1, win1.selection_spec_path);
        assert_eq!(sel2, win2.selection_spec_path);
        assert_ne!(sel1, sel2);

        let ctx1: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&win1.context_json_path).unwrap()).unwrap();
        let ctx2: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&win2.context_json_path).unwrap()).unwrap();
        assert_eq!(ctx1["doc_name"].as_str(), Some("Window One Doc"));
        assert_eq!(ctx2["doc_name"].as_str(), Some("Window Two Doc"));

        let c1 = Document::from_json(&std::fs::read_to_string(&win1.live_canvas_path).unwrap()).unwrap();
        let c2 = Document::from_json(&std::fs::read_to_string(&win2.live_canvas_path).unwrap()).unwrap();
        assert_eq!(c1.name, "Window One Doc");
        assert_eq!(c2.name, "Window Two Doc");

        let s1: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&sel1).unwrap()).unwrap();
        let s2: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&sel2).unwrap()).unwrap();
        assert_eq!(s1["designos_context"]["node_name"].as_str(), Some("Frame One"));
        assert_eq!(s2["designos_context"]["node_name"].as_str(), Some("Frame Two"));

        assert!(win1.poll_live_agent_bridge().is_empty());
        assert!(win2.poll_live_agent_bridge().is_empty());

        win1.shutdown();
        win2.shutdown();
    }

    #[test]
    fn mcp_without_a_session_dir_has_no_live_paths() {
        let dir = ephemeral_session_dir(&unique_test_id("mcp-env"));
        let bridge_with_dir = designos_mcp::server::LiveSessionBridge::for_session_dir(&dir);
        assert_eq!(bridge_with_dir.session_dir.as_deref(), Some(dir.as_path()));
        assert_eq!(bridge_with_dir.live_canvas_path, Some(dir.join("canvas.designos")));
        assert_eq!(bridge_with_dir.live_doc_sync_path, Some(dir.join("doc-sync.designos")));
        assert_eq!(bridge_with_dir.live_spec_path, Some(dir.join("live-spec.json")));
        assert_eq!(bridge_with_dir.context_json_path, Some(dir.join("context.json")));
        assert_eq!(bridge_with_dir.live_doc_patch_path, Some(dir.join("doc-patch.jsonl")));

        let bridge_empty = designos_mcp::server::LiveSessionBridge::from_env_vars(None, None, None, None, None, None);
        assert!(bridge_empty.session_dir.is_none());
        assert!(bridge_empty.live_canvas_path.is_none());
        assert!(bridge_empty.live_doc_sync_path.is_none());
        assert!(bridge_empty.live_spec_path.is_none());
        assert!(bridge_empty.context_json_path.is_none());
        assert!(bridge_empty.live_doc_patch_path.is_none());

        // Even when only one explicit path is supplied without DESIGNOS_SESSION_DIR, the remaining
        // unset paths stay None and never fall back to /tmp/designos-live-*.
        let only_spec = designos_mcp::server::LiveSessionBridge::from_env_vars(None, None, None, Some(dir.join("live-spec.json")), None, None);
        assert_eq!(only_spec.live_spec_path, Some(dir.join("live-spec.json")));
        assert!(only_spec.live_canvas_path.is_none());
        assert!(only_spec.live_doc_sync_path.is_none());
        assert!(only_spec.context_json_path.is_none());
        assert!(only_spec.live_doc_patch_path.is_none());
    }

    #[test]
    fn dropping_spec_into_window_a_never_applies_in_window_b() {
        let mut win_a = TerminalState::for_tab(1);
        win_a.bind_session(&unique_test_id("win-a"));
        let mut win_b = TerminalState::for_tab(1);
        win_b.bind_session(&unique_test_id("win-b"));

        std::fs::create_dir_all(&win_a.session_dir).unwrap();
        std::fs::create_dir_all(&win_b.session_dir).unwrap();
        std::fs::write(&win_a.live_spec_path, r#"{"name":"Window A Card","preset":"easeui-card"}"#).unwrap();

        let actions_a = win_a.poll_live_agent_bridge();
        let actions_b = win_b.poll_live_agent_bridge();
        assert_eq!(actions_a, vec![TerminalAction::ApplySpecFile(win_a.live_spec_path.clone())]);
        assert!(actions_b.is_empty(), "window B must not see window A's spec: {actions_b:?}");

        win_a.shutdown();
        win_b.shutdown();
    }

    #[test]
    fn closing_a_tab_removes_its_ephemeral_session_dir() {
        let mut st = TerminalState::for_tab(1);
        st.bind_session(&unique_test_id("ephemeral-close"));
        let session_dir = st.session_dir.clone();
        let mut doc = Document::new("Unsaved Tab");
        let page = doc.add_page("Page 1");

        st.sync_live_file_context(&doc, page, &[], None);
        st.flush_live_sync();
        assert!(session_dir.join("context.json").is_file());
        assert!(session_dir.join("canvas.designos").is_file());
        assert!(session_dir.join("owner.json").is_file());

        st.close_tab(0);
        assert!(!session_dir.exists(), "closing the tab must remove its ephemeral session dir {}", session_dir.display());
    }

    #[test]
    fn shutdown_removes_an_ephemeral_session_dir() {
        let mut st = TerminalState::for_tab(1);
        st.bind_session(&unique_test_id("ephemeral-shutdown"));
        let session_dir = st.session_dir.clone();
        let mut doc = Document::new("Unsaved Window");
        let page = doc.add_page("Page 1");

        st.sync_live_file_context(&doc, page, &[], None);
        st.flush_live_sync();
        assert!(session_dir.exists());
        st.shutdown();
        assert!(!session_dir.exists(), "shutdown must remove the ephemeral session dir {}", session_dir.display());
    }

    /// The frame that closes a window (or the last tab) goes on to publish the document: that publish must
    /// not bring back the folder the close just removed (spec rule 4, edge case 8).
    #[test]
    fn a_publish_after_the_last_tab_closed_does_not_recreate_the_session_folder() {
        let mut st = TerminalState::for_tab(1);
        st.bind_session(&unique_test_id("no-recreate-tab"));
        let session_dir = st.session_dir.clone();
        let mut doc = Document::new("Unsaved Tab");
        let page = doc.add_page("Page 1");

        st.sync_live_file_context(&doc, page, &[], None);
        st.flush_live_sync();
        st.spawn_tab(crate::terminal::AgentCliPreset::Shell, None).expect("spawn a shell tab");
        st.close_tab(0);
        assert!(!session_dir.exists(), "precondition: closing the last tab removed the folder");

        st.sync_live_file_context(&doc, page, &[], None);
        st.flush_live_sync();
        assert!(!session_dir.exists(), "the publish after the close re-created {}", session_dir.display());

        // A new tab gives the terminal a folder again, and the document is published into it.
        st.spawn_tab(crate::terminal::AgentCliPreset::Shell, None).expect("spawn a second shell tab");
        st.sync_live_file_context(&doc, page, &[], None);
        st.flush_live_sync();
        assert!(session_dir.join("canvas.designos").is_file(), "publishing resumes once a tab runs again");
        st.shutdown();
    }

    /// The first-turn bootstrap block is built on a background thread that writes into the session folder when
    /// it is done. A build still running when the window closes must not bring the removed folder back.
    #[test]
    fn a_bootstrap_build_that_finishes_after_shutdown_does_not_recreate_the_session_folder() {
        let mut st = TerminalState::for_tab(1);
        st.bind_session(&unique_test_id("bootstrap-after-close"));
        let session_dir = st.session_dir.clone();
        let mut doc = Document::new("Unsaved Window");
        let page = doc.add_page("Page 1");
        st.sync_live_file_context(&doc, page, &[], None);
        st.flush_live_sync();
        st.start_bootstrap_job(doc.clone(), page, None, Some(Duration::from_millis(400)));

        st.shutdown();
        assert!(!session_dir.exists(), "precondition: shutdown removed the folder");
        std::thread::sleep(Duration::from_millis(900));
        assert!(!session_dir.exists(), "the bootstrap build re-created {}", session_dir.display());
    }

    #[test]
    fn a_publish_after_shutdown_does_not_recreate_the_session_folder() {
        let mut st = TerminalState::for_tab(1);
        st.bind_session(&unique_test_id("no-recreate-window"));
        let session_dir = st.session_dir.clone();
        let mut doc = Document::new("Unsaved Window");
        let page = doc.add_page("Page 1");

        // An Ask-mode session installs its approval hook, outside the session folder, with the rest of its files.
        let channel = crate::agent_approval_channel::channel_dir(&session_dir).expect("a private channel folder");

        st.sync_live_file_context(&doc, page, &[], None);
        st.flush_live_sync();
        assert!(channel.join(crate::agent_approval_channel::HOOK_SCRIPT_NAME).is_file(), "the session installs its approval hook");
        st.shutdown();
        assert!(!session_dir.exists(), "precondition: shutdown removed the folder");
        assert!(!channel.exists(), "shutdown removes the approval channel with the folder");

        st.sync_live_file_context(&doc, page, &[], None);
        st.flush_live_sync();
        assert!(!session_dir.exists(), "the publish after shutdown re-created {}", session_dir.display());
        assert!(!channel.exists(), "the publish after shutdown re-created the approval channel {}", channel.display());

        // Binding to a session again (a document opened in this terminal) lifts the stop.
        st.bind_session(&unique_test_id("no-recreate-window-next"));
        let next_dir = st.session_dir.clone();
        st.sync_live_file_context(&doc, page, &[], None);
        st.flush_live_sync();
        assert!(next_dir.join("canvas.designos").is_file(), "a rebound terminal publishes again");
        st.shutdown();
        assert!(!crate::agent_approval_channel::channel_dir(&next_dir).expect("a private channel folder").exists());
    }

    fn writer_test_job(session_dir: &Path, key: (u64, u64, u64)) -> SnapshotJob {
        SnapshotJob {
            session_dir: session_dir.to_path_buf(),
            session_id: "writer-test".into(),
            key,
            context_json_path: session_dir.join("context.json"),
            live_canvas_path: session_dir.join("canvas.designos"),
            live_spec_path: session_dir.join("live-spec.json"),
            live_doc_sync_path: session_dir.join("doc-sync.designos"),
            live_doc_patch_path: session_dir.join("doc-patch.jsonl"),
            context_bytes: b"{}".to_vec(),
            canvas_doc: Document::new("Writer Test"),
            doc_name: "Writer Test".into(),
            current_page_id: 0,
            current_page_name: "Page 1".into(),
            file_path: None,
            run_id: "writer-test-run".into(),
            bootstrap_run_id: 1,
            agent_runtime: "claude".into(),
            ask_mode: false,
            ask_risky: false,
            unread_probe: None,
            exact_project_config: None,
        }
    }

    /// A panic while writing must not leave the worker marked as busy: every later wait (an agent start, a
    /// close, a rebind) would block the UI thread for good.
    #[test]
    fn a_panic_in_the_writer_does_not_hang_the_next_wait_or_stop_later_snapshots() {
        let dir = ephemeral_session_dir(&unique_test_id("writer-panic"));
        let mut worker = SnapshotWorker::new();
        let job = writer_test_job(&dir, (1, 0, 0));
        crate::terminal::set_before_rename_test_hook(&job.live_canvas_path, Box::new(|| panic!("writer test panic")));
        worker.enqueue(job);

        let worker = Arc::new(worker);
        let (done_tx, done_rx) = std::sync::mpsc::channel();
        let waiter = Arc::clone(&worker);
        std::thread::spawn(move || {
            waiter.flush();
            let _ = done_tx.send(());
        });
        assert!(done_rx.recv_timeout(Duration::from_secs(10)).is_ok(), "flush hung after the writer panicked");
        let status = worker.take_status();
        assert!(status.error.as_deref().is_some_and(|e| e.contains("panicked")), "the panic is reported as an error: {:?}", status.error);
        assert_eq!(status.failed, Some((dir.clone(), (1, 0, 0))), "the failed key is reported so the UI retries");

        // The writer survived: the next job lands.
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        let mut worker = worker;
        let mut worker = loop {
            match Arc::try_unwrap(worker) {
                Ok(w) => break w,
                Err(still_shared) => {
                    assert!(std::time::Instant::now() < deadline, "the flush thread never released the worker");
                    worker = still_shared;
                    std::thread::sleep(Duration::from_millis(5));
                }
            }
        };
        worker.enqueue(writer_test_job(&dir, (2, 0, 0)));
        worker.flush();
        assert_eq!(worker.take_status().completed, Some((dir.clone(), (2, 0, 0))), "a job queued after the panic is written");
        assert!(dir.join("canvas.designos").is_file());
        drop(worker);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A disk that refuses the writes must not be hit again on every painted frame: each frame would clone
    /// the document, serialize it on the writer and report the same error. The same content is retried only
    /// after a pause; a change of the published content (spec edge case 5) retries at once.
    #[test]
    fn a_failing_write_is_not_retried_every_frame() {
        let mut st = TerminalState::for_tab(1);
        st.bind_session(&unique_test_id("write-backoff"));
        let blocked = st.session_dir.clone();
        std::fs::create_dir_all(blocked.parent().expect("sessions root")).expect("sessions root");
        std::fs::write(&blocked, b"a file where the session folder should be").expect("block the folder");
        let mut doc = Document::new("Disk Full");
        let page = doc.add_page("Page 1");

        for _ in 0..10 {
            st.sync_live_file_context(&doc, page, &[], None);
            st.flush_live_sync();
        }
        assert_eq!(st.snapshot_jobs_executed(), 1, "ten frames with the same content must try the failing write once");
        assert!(st.status_msg.as_deref().is_some_and(|m| m.contains("Couldn't update the agent files")), "the failure is reported: {:?}", st.status_msg);

        st.status_msg = None;
        st.sync_live_file_context(&doc, page, &[page], None);
        st.flush_live_sync();
        assert_eq!(st.snapshot_jobs_executed(), 2, "new content is tried at once");

        // Once the disk takes the write again, the next change publishes.
        std::fs::remove_file(&blocked).expect("unblock the folder");
        st.sync_live_file_context(&doc, page, &[], None);
        st.flush_live_sync();
        assert!(blocked.join("canvas.designos").is_file(), "the canvas is published after the disk recovers");
        st.shutdown();
    }

    #[test]
    fn write_backoff_pauses_the_same_content_with_a_growing_delay_and_reports_it_once() {
        let t0 = Instant::now();
        let (a, b) = ((1, 1, 1), (2, 1, 1));
        let mut backoff = WriteBackoff::default();
        assert!(!backoff.blocks(a, t0), "nothing failed yet");

        assert!(backoff.record_failure(a, "disk full", t0), "the first failure is news");
        assert!(backoff.blocks(a, t0 + Duration::from_millis(999)), "the same content waits 1 s");
        assert!(!backoff.blocks(a, t0 + Duration::from_secs(1)), "and is tried again after it");
        assert!(!backoff.blocks(b, t0), "other content is tried at once");

        let t1 = t0 + Duration::from_secs(1);
        assert!(!backoff.record_failure(a, "disk full", t1), "the same failure is not news again");
        assert!(backoff.blocks(a, t1 + Duration::from_millis(1999)), "the pause doubles to 2 s");
        assert!(!backoff.blocks(a, t1 + Duration::from_secs(2)));

        assert!(backoff.record_failure(a, "permission denied", t1), "a different error text is news");

        let mut now = t1;
        for _ in 0..12 {
            backoff.record_failure(a, "disk full", now);
            now += Duration::from_secs(1);
        }
        let last = now - Duration::from_secs(1);
        assert!(backoff.blocks(a, last + Duration::from_secs(29)) && !backoff.blocks(a, last + Duration::from_secs(30)), "the pause stops at 30 s");

        assert!(backoff.record_failure(b, "disk full", now), "failing content that changed is news again");
        backoff.clear();
        assert!(!backoff.blocks(b, now), "a success clears the pause");
    }

    #[test]
    fn startup_sweep_removes_only_dead_sessions_older_than_7_days() {
        let temp_parent = std::env::temp_dir().join(unique_test_id("sweep-sandbox"));
        let root = temp_parent.join("sessions");
        std::fs::create_dir_all(&root).unwrap();

        let now = SystemTime::now();
        let eight_days_ago = now - Duration::from_secs(8 * 24 * 60 * 60);
        let one_hour_ago = now - Duration::from_secs(3600);
        let dead_pid = 999_991u32;
        let live_pid = std::process::id();

        // (a) dead PID + 8 days old -> removed
        let dir_dead_old = root.join(format!("p{dead_pid}-w1-tab-1"));
        std::fs::create_dir_all(&dir_dead_old).unwrap();
        let marker_dead = OwnerMarker { pid: dead_pid, started_ms: 1_000 };
        std::fs::write(dir_dead_old.join("owner.json"), serde_json::to_vec(&marker_dead).unwrap()).unwrap();
        set_dir_mtime(&dir_dead_old, eight_days_ago);

        // (b) dead PID + 1 hour old -> kept
        let dir_dead_recent = root.join(format!("p{dead_pid}-w1-tab-2"));
        std::fs::create_dir_all(&dir_dead_recent).unwrap();
        std::fs::write(dir_dead_recent.join("owner.json"), serde_json::to_vec(&marker_dead).unwrap()).unwrap();
        set_dir_mtime(&dir_dead_recent, one_hour_ago);

        // (c) live PID + 8 days old -> kept
        let dir_live_old = root.join(format!("p{live_pid}-w1-tab-1"));
        std::fs::create_dir_all(&dir_live_old).unwrap();
        let marker_live = OwnerMarker { pid: live_pid, started_ms: current_process_started_ms() };
        std::fs::write(dir_live_old.join("owner.json"), serde_json::to_vec(&marker_live).unwrap()).unwrap();
        set_dir_mtime(&dir_live_old, eight_days_ago);

        // (d) foreign folder without owner.json or p<pid>- prefix + 8 days old -> kept
        let dir_foreign = root.join("custom-user-notes");
        std::fs::create_dir_all(&dir_foreign).unwrap();
        std::fs::write(dir_foreign.join("notes.txt"), b"keep").unwrap();
        set_dir_mtime(&dir_foreign, eight_days_ago);

        // Each session of an Ask-mode agent has an approval channel outside its folder.
        let channels: Vec<_> = [&dir_dead_old, &dir_dead_recent, &dir_live_old]
            .iter()
            .map(|dir| crate::agent_approval_channel::prepare_channel_dir(dir).expect("a private channel folder"))
            .collect();

        let removed = sweep_stale_sessions(&root, now, |pid, _started| pid == live_pid);
        assert_eq!(removed, 1);
        assert!(!dir_dead_old.exists(), "dead + 8 days old must be removed");
        assert!(dir_dead_recent.exists(), "dead + 1 hour old must be kept");
        assert!(dir_live_old.exists(), "live PID + 8 days old must be kept");
        assert!(dir_foreign.exists(), "foreign folder must be kept");
        assert!(!channels[0].exists(), "the channel of a swept session goes with it");
        assert!(channels[1].exists() && channels[2].exists(), "the channels of kept sessions stay");

        crate::agent_approval_channel::remove_channel(&dir_dead_recent);
        crate::agent_approval_channel::remove_channel(&dir_live_old);
        let _ = std::fs::remove_dir_all(&temp_parent);
    }

    #[test]
    fn sweep_never_touches_library_agent_sessions() {
        let lib_root = std::env::temp_dir().join(unique_test_id("lib-sweep-guard"));
        let agent_sessions = lib_root.join("agent-sessions");
        let old_file_session = agent_sessions.join("p999992-file-1");
        std::fs::create_dir_all(&old_file_session).unwrap();
        let marker = OwnerMarker { pid: 999_992, started_ms: 1_000 };
        std::fs::write(old_file_session.join("owner.json"), serde_json::to_vec(&marker).unwrap()).unwrap();
        let now = SystemTime::now();
        let eight_days_ago = now - Duration::from_secs(8 * 24 * 60 * 60);
        set_dir_mtime(&old_file_session, eight_days_ago);

        // Also test a nested `sessions` folder inside `agent-sessions`
        let nested = agent_sessions.join("sessions");
        let nested_old = nested.join("p999992-tab-1");
        std::fs::create_dir_all(&nested_old).unwrap();
        set_dir_mtime(&nested_old, eight_days_ago);

        assert_eq!(sweep_stale_sessions(&agent_sessions, now, |_, _| false), 0);
        assert_eq!(sweep_stale_sessions(&nested, now, |_, _| false), 0);
        assert!(old_file_session.exists(), "library agent-sessions must never be deleted");
        assert!(nested_old.exists(), "path containing agent-sessions must never be deleted");

        let _ = std::fs::remove_dir_all(&lib_root);
    }

    #[test]
    fn ui_thread_cost_on_a_24k_node_document_stays_under_budget() {
        let mut doc = Document::new("24k Budget Doc");
        let page = doc.add_page("Page 1");
        // Build 24,000 nodes (within the <= 25,000 full-document threshold).
        let frames = 120usize;
        let children_per_frame = 199usize; // 1 page + 120 * (1 + 199) = 24,001 nodes
        for f in 0..frames {
            let fid = doc.alloc_id();
            doc.insert(page, Node::frame(fid, Rect::new(f as f32 * 200.0, 0.0, 180.0, 180.0))).unwrap();
            for c in 0..children_per_frame {
                let cid = doc.alloc_id();
                doc.insert(fid, Node::rectangle(cid, Rect::new(c as f32, 0.0, 10.0, 10.0))).unwrap();
            }
        }
        assert!(doc.nodes.len() >= 24_000 && doc.nodes.len() <= 25_000, "nodes={}", doc.nodes.len());

        let mut st = TerminalState::for_tab(1);
        st.bind_session(&unique_test_id("budget-24k"));

        // Warm up the worker thread once so thread spawn cost is not mixed into steady-state publish timing.
        st.sync_live_file_context(&doc, page, &[], None);
        st.flush_live_sync();

        // The absolute budgets hold on a fast machine. A slower or busier one stretches the sync and what it replaced (serializing the
        // whole document on the UI thread) together, so the budget never drops below a twelfth of that serialization, measured next to
        // the sync: a sync that serializes on the UI thread costs a whole one.
        let absolute_per_call_budget = if cfg!(debug_assertions) { Duration::from_millis(12) } else { Duration::from_millis(4) };
        let per_call_budget_for = |old_to_json: Duration| absolute_per_call_budget.max(old_to_json / 12);

        // One sample is five syncs plus one serialization, repeated until the fastest sync is within budget (noise only adds time, as
        // `test_timing::fastest_within` relies on, so both are taken at their fastest). Every sync hands the worker thread a snapshot to
        // serialize and write, which takes far longer than a sample. The worker is waited for after each sample, outside the timed
        // region: otherwise each sample is timed next to the worker of the one before it, and on a runner with few cores that worker
        // takes the UI thread's turn.
        let patience = std::time::Instant::now() + Duration::from_secs(15);
        let mut fastest_five = Duration::MAX;
        let mut old_to_json_elapsed = Duration::MAX;
        loop {
            st.last_synced_key = None;
            let started = std::time::Instant::now();
            for _ in 0..5 {
                st.sync_live_file_context(&doc, page, &[], None);
            }
            fastest_five = fastest_five.min(started.elapsed());
            st.flush_live_sync();
            let started = std::time::Instant::now();
            let _ = doc.to_json().expect("serialize 24k doc");
            old_to_json_elapsed = old_to_json_elapsed.min(started.elapsed());
            if fastest_five < per_call_budget_for(old_to_json_elapsed) * 4 || std::time::Instant::now() >= patience {
                break;
            }
        }
        let per_call_budget = per_call_budget_for(old_to_json_elapsed);
        let total_budget = per_call_budget * 4;
        let per_call = fastest_five / 5;
        eprintln!(
            "[24K LIVE SYNC BENCH] nodes={} | 5x sync_live_file_context={fastest_five:?} ({per_call:?}/call) | old synchronous doc.to_json()={old_to_json_elapsed:?}",
            doc.nodes.len()
        );
        assert!(
            fastest_five < total_budget && per_call < per_call_budget,
            "sync_live_file_context 5x UI-thread cost on 24k nodes took {fastest_five:?} ({per_call:?}/call; budgets {total_budget:?} total, {per_call_budget:?}/call; old to_json took {old_to_json_elapsed:?})"
        );
        st.shutdown();
    }

    #[test]
    fn snapshot_is_written_off_the_ui_thread() {
        let mut st = TerminalState::for_tab(1);
        st.bind_session(&unique_test_id("off-ui-thread"));
        let mut doc = Document::new("Off UI Thread Doc");
        let page = doc.add_page("Page 1");
        let rect_id = doc.alloc_id();
        doc.insert(page, Node::rectangle(rect_id, Rect::new(12.0, 34.0, 120.0, 80.0))).unwrap();

        let ui_thread_id = std::thread::current().id();
        st.sync_live_file_context(&doc, page, &[rect_id], None);
        st.flush_live_sync();

        let writer_thread_id = st.snapshot_worker_thread_id().expect("background snapshot worker must have recorded its ThreadId");
        assert_ne!(writer_thread_id, ui_thread_id, "canvas.designos must be serialized and written on the background worker thread, not the UI thread");

        let raw = std::fs::read_to_string(&st.live_canvas_path).expect("read canvas.designos");
        let round_trip = Document::from_json(&raw).expect("parse canvas.designos");
        assert_eq!(round_trip.name, doc.name);
        assert_eq!(round_trip.nodes.len(), doc.nodes.len());
        assert_eq!(round_trip.get(rect_id).unwrap().size, doc.get(rect_id).unwrap().size);

        st.shutdown();
    }

    #[test]
    fn coalesces_rapid_snapshots_to_the_latest_document() {
        let mut st = TerminalState::for_tab(1);
        st.bind_session(&unique_test_id("coalesce-10"));
        let mut doc = Document::new("v1");
        let page = doc.add_page("Page 1");

        for i in 1..=10 {
            doc.name = format!("v{i}");
            st.sync_live_file_context(&doc, page, &[], None);
        }
        st.flush_live_sync();

        let ctx: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&st.context_json_path).unwrap()).unwrap();
        assert_eq!(ctx["doc_name"].as_str(), Some("v10"));

        let saved = Document::from_json(&std::fs::read_to_string(&st.live_canvas_path).unwrap()).unwrap();
        assert_eq!(saved.name, "v10");

        st.shutdown();
    }

    #[test]
    fn snapshot_write_failure_does_not_mark_key_synced() {
        let bad_path = std::env::temp_dir().join(unique_test_id("unwritable-file-as-dir"));
        // Create a regular file where a directory is expected so create_dir_all(session_dir) fails.
        std::fs::write(&bad_path, b"not a directory").unwrap();

        let mut st = TerminalState::for_tab(1);
        st.session_dir = bad_path.clone();
        st.context_json_path = bad_path.join("context.json");
        st.live_canvas_path = bad_path.join("canvas.designos");
        st.live_spec_path = bad_path.join("live-spec.json");
        st.live_doc_sync_path = bad_path.join("doc-sync.designos");

        let mut doc = Document::new("Unwritable Session Doc");
        let page = doc.add_page("Page 1");

        st.sync_live_file_context(&doc, page, &[], None);
        st.flush_live_sync();

        assert!(st.last_synced_key.is_none(), "failed write must not mark last_synced_key as succeeded: {:?}", st.last_synced_key);
        assert!(
            st.status_msg.as_deref().is_some_and(|m| m.contains("Couldn't update the agent files")),
            "write failure must be surfaced in status_msg, got: {:?}",
            st.status_msg
        );

        let _ = std::fs::remove_file(&bad_path);
    }

    #[test]
    fn no_global_tmp_path_in_shipped_code() {
        fn collect_rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
            let Ok(entries) = std::fs::read_dir(dir) else { return };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    collect_rs_files(&path, out);
                } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
                    out.push(path);
                }
            }
        }

        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().and_then(|p| p.parent()).expect("workspace root");
        let mut files = Vec::new();
        collect_rs_files(&repo_root.join("crates/designos-app/src"), &mut files);
        collect_rs_files(&repo_root.join("crates/designos-mcp/src"), &mut files);
        assert!(!files.is_empty());

        for path in files {
            let src = std::fs::read_to_string(&path).expect("read rs file");
            // Skip test-only modules: `*_tests.rs` files and everything in a `tests` or `*_tests` folder.
            let test_only =
                path.components().any(|c| c.as_os_str().to_str().is_some_and(|n| n == "tests" || n.ends_with("_tests") || n.ends_with("_tests.rs")));
            if test_only {
                continue;
            }
            let crlf_src = src.replace("\r\n", "\n").replace('\n', "\r\n");
            for candidate in [src.as_str(), crlf_src.as_str()] {
                let shipped =
                    candidate.split("#[cfg(test)]\r\nmod tests").next().unwrap_or(candidate).split("#[cfg(test)]\nmod tests").next().unwrap_or(candidate);
                for line in shipped.lines() {
                    let code_line = line.split("//").next().unwrap_or("");
                    assert!(
                        !code_line.contains("/tmp/designos-live-"),
                        "{} still contains global /tmp/designos-live- path in shipped code: {line}",
                        path.display()
                    );
                    assert!(
                        !code_line.contains("designos-sessions"),
                        "{} builds a session path in the temp folder, which any sandboxed agent can write: {line}",
                        path.display()
                    );
                }
            }
        }
    }

    #[cfg(unix)]
    #[test]
    fn parse_ps_etime_secs_and_query_process_started_ms_work_on_unix() {
        assert_eq!(parse_ps_etime_secs("00:05"), Some(5));
        assert_eq!(parse_ps_etime_secs("  12:34 \n"), Some(12 * 60 + 34));
        assert_eq!(parse_ps_etime_secs("01:02:03"), Some(3_600 + 2 * 60 + 3));
        assert_eq!(parse_ps_etime_secs("2-03:04:05"), Some(2 * 86_400 + 3 * 3_600 + 4 * 60 + 5));
        assert_eq!(parse_ps_etime_secs(""), None);
        assert_eq!(parse_ps_etime_secs("invalid"), None);

        let mut child = std::process::Command::new("sleep").arg("30").spawn().expect("spawn sleep child");
        let child_pid = child.id();
        let started_ms = query_process_started_ms(child_pid);
        let _ = child.kill();
        let _ = child.wait();
        let now_ms = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0);
        let actual_ms = started_ms.expect("query_process_started_ms must succeed on unix/macOS via ps -o etime=");
        assert!(actual_ms.abs_diff(now_ms) <= 10_000, "child process start time {actual_ms} must be close to now {now_ms}");
    }
}
