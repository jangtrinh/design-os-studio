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
pub(super) fn running_as_root() -> bool {
    // SAFETY: `geteuid` takes no arguments and cannot fail.
    unsafe { libc::geteuid() == 0 }
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
