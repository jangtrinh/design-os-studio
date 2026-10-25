//! What the tests of a link inside a session folder share: a session folder Studio made, a link planted in it, a folder made
//! read-only. It needs only what `folder_tests` has, so it also builds on the tree before the fix.

use super::folder_tests::Cleanup;
use super::*;
use crate::live_sync_tests::unique_tab_id;
use std::os::unix::fs::{symlink, PermissionsExt};

/// A session folder Studio made, removed when the returned guard drops.
pub(super) fn made_session_folder(tag: &str) -> (PathBuf, Cleanup) {
    let dir = ephemeral_session_dir(&format!("p{}-{tag}-{}", std::process::id(), unique_tab_id()));
    ensure_session_dir(&dir).expect("Studio makes the session folder");
    (dir.clone(), Cleanup(vec![dir]))
}

/// Plants a link at `<session>/<rel>` (its parent folders are real folders) that points at `target`.
pub(super) fn plant_link(session: &Path, rel: &str, target: &Path) {
    let link = session.join(rel);
    std::fs::create_dir_all(link.parent().expect("a parent")).expect("real parent folders");
    symlink(target, &link).expect("plant the link");
}

/// Makes the folder writable again when it drops, so a test that made it read-only can remove it.
pub(super) struct Writable(pub(super) PathBuf);

impl Drop for Writable {
    fn drop(&mut self) {
        let _ = std::fs::set_permissions(&self.0, std::fs::Permissions::from_mode(0o755));
    }
}

/// True for root, which writes into a read-only folder: a test of the refusal of such a write does not apply.
pub(crate) fn running_as_root() -> bool {
    // SAFETY: `geteuid` takes no arguments and cannot fail.
    unsafe { libc::geteuid() == 0 }
}

/// A tab state in `mode` with a session folder Studio made and no agent CLI that can start from its PATH.
pub(crate) fn tab_state_in(mode: crate::terminal::PermissionMode) -> crate::test_timing::ScopedTerminalState {
    let mut st = crate::test_timing::ScopedTerminalState::new(crate::terminal::TerminalState::for_tab(unique_tab_id()));
    st.permission_mode = mode;
    st.path_override = Some(std::ffi::OsString::new());
    ensure_session_dir(&st.session_dir).expect("the session folder");
    st
}

/// Writes `text` at `<session>/<rel>`, making the folders above it, the way an agent in its own folder would.
pub(crate) fn plant(session: &Path, rel: &str, text: &str) {
    let path = session.join(rel);
    std::fs::create_dir_all(path.parent().expect("a parent")).expect("the agent's folders");
    std::fs::write(&path, text).expect("the agent's file");
}

/// The names in the folder `dir`, sorted; empty when it is not there.
pub(crate) fn names_in(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir).into_iter().flatten().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    names.sort();
    names
}

/// The folders an Ask-mode start made below `<session>/.designos-set-aside`, one for each start that moved something aside.
pub(crate) fn stamp_folders(session: &Path) -> Vec<PathBuf> {
    names_in(&session.join(".designos-set-aside")).into_iter().map(|name| session.join(".designos-set-aside").join(name)).collect()
}

/// `rel` below the one folder a single start moved its entries into.
pub(crate) fn aside_path(session: &Path, rel: &str) -> PathBuf {
    let folders = stamp_folders(session);
    assert_eq!(folders.len(), 1, "one start, one folder: {folders:?}");
    folders[0].join(rel)
}

/// Whether `name` is the name of a set-aside folder: `<UTC date>-<UTC time>-<six hex digits>`, like `20261025-023045-a1b2c3`.
pub(crate) fn is_stamp(name: &str) -> bool {
    let parts: Vec<&str> = name.split('-').collect();
    matches!(parts.as_slice(), [date, time, id]
        if date.len() == 8 && date.bytes().all(|b| b.is_ascii_digit())
        && time.len() == 6 && time.bytes().all(|b| b.is_ascii_digit())
        && id.len() == 6 && id.bytes().all(|b| b.is_ascii_hexdigit()))
}

/// Whether anything, a link included, is at `path`.
pub(crate) fn is_there(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok()
}

/// Makes a FIFO at `path`, which a plain `open` for reading blocks on until something opens it for writing.
pub(crate) fn make_fifo(path: &Path) {
    let c = std::ffi::CString::new(std::os::unix::ffi::OsStrExt::as_bytes(path.as_os_str())).expect("a path without NUL");
    // SAFETY: `c` is a NUL-terminated path that outlives the call.
    assert_eq!(unsafe { libc::mkfifo(c.as_ptr(), 0o600) }, 0, "mkfifo {}", path.display());
}

/// Runs `work` on the calling thread and says how long it took. A watchdog opens `fifo` for writing once `budget` has passed, which
/// frees a reader blocked in `open`, so a hang ends as a measured delay and not as a test that never finishes.
pub(crate) fn timed_with_fifo_watchdog<T>(fifo: &Path, budget: Duration, work: impl FnOnce() -> T) -> (T, Duration) {
    use std::os::unix::fs::OpenOptionsExt;
    use std::sync::atomic::{AtomicBool, Ordering};
    let done = std::sync::Arc::new(AtomicBool::new(false));
    let watchdog = {
        let (done, fifo) = (done.clone(), fifo.to_path_buf());
        std::thread::spawn(move || {
            let deadline = Instant::now() + budget;
            while !done.load(Ordering::SeqCst) {
                // Without a reader waiting, a non-blocking open for writing fails at once; it succeeds when one is blocked.
                if Instant::now() >= deadline && std::fs::OpenOptions::new().write(true).custom_flags(libc::O_NONBLOCK).open(&fifo).is_ok() {
                    return;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        })
    };
    let started = Instant::now();
    let result = work();
    let elapsed = started.elapsed();
    done.store(true, Ordering::SeqCst);
    watchdog.join().expect("the watchdog ends");
    (result, elapsed)
}
