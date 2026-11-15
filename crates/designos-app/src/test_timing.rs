//! Wall-clock budgets for tests that guard the algorithmic cost of canvas operations on large pages.

use std::time::{Duration, Instant};

/// Runs `work` until one run finishes within `budget` or `patience` has passed. Returns the fastest run
/// and the value of the last one.
///
/// A lone timed call on a shared CI runner includes scheduler and disk stalls that are not the code's
/// cost, so a single sample against a millisecond budget flakes. Noise only ever adds time, which makes
/// the fastest run the best bound on the real cost. A change that makes the work itself slower (a linear
/// scan that became quadratic, say) stays over the budget on every run, however long it is sampled.
pub fn fastest_within<T>(budget: Duration, patience: Duration, mut work: impl FnMut() -> T) -> (Duration, T) {
    let deadline = Instant::now() + patience;
    let mut fastest = Duration::MAX;
    loop {
        let started = Instant::now();
        let out = work();
        fastest = fastest.min(started.elapsed());
        if fastest < budget || Instant::now() >= deadline {
            return (fastest, out);
        }
    }
}

/// Budget for a test that waits on a stub CLI child process (or on any other event a loaded machine can delay). Under a loaded
/// parallel run a spawned shell can take tens of seconds to start; every such wait returns as soon as its condition holds, so a healthy
/// run never pays for the budget. It only matters when the stub never runs.
pub const STUB_WAIT: Duration = Duration::from_secs(60);

/// Polls `condition` every 25 ms until it holds or `budget` passes. Returns whether it held.
pub fn poll_until(budget: Duration, mut condition: impl FnMut() -> bool) -> bool {
    let deadline = Instant::now() + budget;
    loop {
        if condition() {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}

/// Waits up to `budget` for `path` to hold non-empty text and returns what it holds (empty on timeout).
pub fn wait_for_nonempty_file(path: &std::path::Path, budget: Duration) -> String {
    let mut text = String::new();
    poll_until(budget, || {
        text = std::fs::read_to_string(path).unwrap_or_default();
        !text.is_empty()
    });
    text
}

/// Blocks on OS directory events on `channel_dir` and `tab_dir` (and on `child`'s exit via its piped `stderr` EOF)
/// until `condition` holds or `child` exits. Uses no wall-clock timer, sleep, or spin loop.
#[cfg(target_os = "macos")]
pub fn wait_for_dir_or_child_exit(
    channel_dir: &std::path::Path,
    tab_dir: &std::path::Path,
    child: &mut std::process::Child,
    mut condition: impl FnMut() -> bool,
) -> bool {
    use std::ffi::CString;
    use std::os::fd::AsRawFd;
    use std::os::unix::ffi::OsStrExt;

    struct FdGuard {
        kq: libc::c_int,
        channel_fd: libc::c_int,
        tab_fd: libc::c_int,
    }

    impl Drop for FdGuard {
        fn drop(&mut self) {
            unsafe {
                if self.tab_fd >= 0 {
                    libc::close(self.tab_fd);
                }
                if self.channel_fd >= 0 {
                    libc::close(self.channel_fd);
                }
                if self.kq >= 0 {
                    libc::close(self.kq);
                }
            }
        }
    }

    fn open_and_register_vnode(kq: libc::c_int, path: &std::path::Path) -> libc::c_int {
        let Ok(c_path) = CString::new(path.as_os_str().as_bytes()) else {
            return -1;
        };
        let fd = unsafe { libc::open(c_path.as_ptr(), libc::O_EVTONLY | libc::O_CLOEXEC) };
        if fd < 0 {
            return -1;
        }
        let change = libc::kevent {
            ident: fd as libc::uintptr_t,
            filter: libc::EVFILT_VNODE,
            flags: libc::EV_ADD | libc::EV_ENABLE | libc::EV_CLEAR,
            fflags: libc::NOTE_WRITE | libc::NOTE_EXTEND | libc::NOTE_ATTRIB | libc::NOTE_RENAME | libc::NOTE_DELETE,
            data: 0,
            udata: std::ptr::null_mut(),
        };
        let rc = unsafe { libc::kevent(kq, &change, 1, std::ptr::null_mut(), 0, std::ptr::null()) };
        if rc < 0 {
            unsafe {
                libc::close(fd);
            }
            return -1;
        }
        fd
    }

    if condition() {
        return true;
    }
    let kq = unsafe { libc::kqueue() };
    if kq < 0 {
        let _ = child.wait();
        return condition();
    }
    let channel_fd = open_and_register_vnode(kq, channel_dir);
    let mut guard = FdGuard { kq, channel_fd, tab_fd: -1 };

    // The child's exit is seen as EOF on its stderr pipe. Without that pipe, or when it cannot be watched, nothing would wake
    // the wait when the child exits without writing the file, so block on the exit itself.
    let stderr_fd = child.stderr.as_ref().map(|s| s.as_raw_fd()).unwrap_or(-1);
    if stderr_fd < 0 {
        let _ = child.wait();
        return condition();
    }
    let change = libc::kevent {
        ident: stderr_fd as libc::uintptr_t,
        filter: libc::EVFILT_READ,
        flags: libc::EV_ADD | libc::EV_ENABLE,
        fflags: 0,
        data: 0,
        udata: std::ptr::null_mut(),
    };
    if unsafe { libc::kevent(guard.kq, &change, 1, std::ptr::null_mut(), 0, std::ptr::null()) } < 0 {
        let _ = child.wait();
        return condition();
    }

    loop {
        if guard.tab_fd < 0 && tab_dir.is_dir() {
            guard.tab_fd = open_and_register_vnode(guard.kq, tab_dir);
        }
        if condition() {
            return true;
        }
        if child.try_wait().ok().flatten().is_some() {
            return condition();
        }
        let mut events = [libc::kevent { ident: 0, filter: 0, flags: 0, fflags: 0, data: 0, udata: std::ptr::null_mut() }; 4];
        let n = unsafe { libc::kevent(guard.kq, std::ptr::null(), 0, events.as_mut_ptr(), events.len() as libc::c_int, std::ptr::null()) };
        if n < 0 {
            if std::io::Error::last_os_error().raw_os_error() == Some(libc::EINTR) {
                continue;
            }
            let _ = child.wait();
            return condition();
        }
        let stderr_events = events[..n as usize].iter().filter(|ev| ev.ident == stderr_fd as libc::uintptr_t);
        let mut stderr_eof = false;
        for ev in stderr_events {
            stderr_eof |= (ev.flags & libc::EV_EOF) != 0;
            // Drain what the child wrote: a level-triggered read event left unread would wake this loop forever, and a full
            // pipe would block the child. `data` is the byte count ready, so these reads never block.
            let mut left = ev.data.max(0) as usize;
            let mut buf = [0u8; 4096];
            while left > 0 {
                let got = unsafe { libc::read(stderr_fd, buf.as_mut_ptr().cast(), left.min(buf.len())) };
                if got <= 0 {
                    break;
                }
                left -= got as usize;
            }
        }
        if stderr_eof {
            let _ = child.wait();
            return condition();
        }
    }
}

/// Blocks on OS directory events on `channel_dir` and `tab_dir` (and on `child`'s exit via its piped `stderr` EOF)
/// until `condition` holds or `child` exits. Uses no wall-clock timer, sleep, or spin loop.
#[cfg(all(unix, not(target_os = "macos")))]
pub fn wait_for_dir_or_child_exit(
    channel_dir: &std::path::Path,
    tab_dir: &std::path::Path,
    child: &mut std::process::Child,
    mut condition: impl FnMut() -> bool,
) -> bool {
    use std::ffi::CString;
    use std::os::fd::AsRawFd;
    use std::os::unix::ffi::OsStrExt;

    struct InotifyGuard(libc::c_int);
    impl Drop for InotifyGuard {
        fn drop(&mut self) {
            if self.0 >= 0 {
                unsafe {
                    libc::close(self.0);
                }
            }
        }
    }

    fn add_watch(ifd: libc::c_int, path: &std::path::Path) -> libc::c_int {
        let Ok(c_path) = CString::new(path.as_os_str().as_bytes()) else {
            return -1;
        };
        unsafe { libc::inotify_add_watch(ifd, c_path.as_ptr(), libc::IN_CREATE | libc::IN_MOVED_TO | libc::IN_MODIFY | libc::IN_CLOSE_WRITE | libc::IN_ATTRIB) }
    }

    if condition() {
        return true;
    }
    let ifd = unsafe { libc::inotify_init1(libc::IN_CLOEXEC | libc::IN_NONBLOCK) };
    if ifd < 0 {
        let _ = child.wait();
        return condition();
    }
    let _guard = InotifyGuard(ifd);
    let _ = add_watch(ifd, channel_dir);
    let mut tab_wd = -1;
    // The child's exit is seen as a hang-up on its stderr pipe; without that pipe, block on the exit itself.
    let stderr_fd = child.stderr.as_ref().map(|s| s.as_raw_fd()).unwrap_or(-1);
    if stderr_fd < 0 {
        let _ = child.wait();
        return condition();
    }

    loop {
        if tab_wd < 0 && tab_dir.is_dir() {
            tab_wd = add_watch(ifd, tab_dir);
        }
        if condition() {
            return true;
        }
        if child.try_wait().ok().flatten().is_some() {
            return condition();
        }
        let mut pfds =
            [libc::pollfd { fd: ifd, events: libc::POLLIN, revents: 0 }, libc::pollfd { fd: stderr_fd, events: libc::POLLIN | libc::POLLHUP, revents: 0 }];
        let rc = unsafe { libc::poll(pfds.as_mut_ptr(), 2, -1) };
        if rc < 0 {
            if std::io::Error::last_os_error().raw_os_error() == Some(libc::EINTR) {
                continue;
            }
            let _ = child.wait();
            return condition();
        }
        if (pfds[0].revents & libc::POLLIN) != 0 {
            let mut buf = [0u8; 512];
            while unsafe { libc::read(ifd, buf.as_mut_ptr().cast(), buf.len()) } > 0 {}
        }
        if (pfds[1].revents & libc::POLLIN) != 0 {
            // Drain what the child wrote, so a level-triggered POLLIN does not wake this loop forever and a full pipe does
            // not block the child. One read per wake never blocks after poll reported data.
            let mut buf = [0u8; 4096];
            let got = unsafe { libc::read(stderr_fd, buf.as_mut_ptr().cast(), buf.len()) };
            if got == 0 {
                let _ = child.wait();
                return condition();
            }
        }
        if (pfds[1].revents & (libc::POLLHUP | libc::POLLERR | libc::POLLNVAL)) != 0 {
            let _ = child.wait();
            return condition();
        }
    }
}

/// A [`TerminalState`] for a test that removes its session folder (`<sessions root>/p<pid>-tab-<id>`) when the test ends, passed or
/// panicked. Nothing else removes that folder, and one that outlives its test is what a later process with the same pid finds as stale
/// state; the folders also pile up by the hundred thousand on a machine that runs the suite all day.
///
/// It derefs to the state, so a test uses it as it would use the state itself.
pub struct ScopedTerminalState {
    state: crate::terminal::TerminalState,
    /// The folder the state was created with. A test that rebinds the state leaves this one behind, so it is removed on its own.
    created_folder: std::path::PathBuf,
}

impl ScopedTerminalState {
    pub fn new(state: crate::terminal::TerminalState) -> Self {
        let created_folder = state.session_dir.clone();
        assert!(
            crate::agent_session_files::is_ephemeral_session_dir(&created_folder),
            "only an ephemeral session folder is removed when the test ends: {}",
            created_folder.display()
        );
        Self { state, created_folder }
    }
}

impl Default for ScopedTerminalState {
    fn default() -> Self {
        Self::new(crate::terminal::TerminalState::default())
    }
}

impl std::ops::Deref for ScopedTerminalState {
    type Target = crate::terminal::TerminalState;
    fn deref(&self) -> &Self::Target {
        &self.state
    }
}

impl std::ops::DerefMut for ScopedTerminalState {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.state
    }
}

impl Drop for ScopedTerminalState {
    fn drop(&mut self) {
        // `shutdown` stops the snapshot and bootstrap workers before it removes a folder, so a late worker cannot bring the folder back.
        // This may run while a failed test unwinds, where a second panic would abort the whole test binary: it must not escape.
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| self.state.shutdown()));
        let _ = std::fs::remove_dir_all(&self.created_folder);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scoped_state_with_a_file() -> (ScopedTerminalState, std::path::PathBuf) {
        let st = ScopedTerminalState::new(crate::terminal::TerminalState::for_tab(crate::live_sync_tests::unique_tab_id()));
        let folder = st.session_dir.clone();
        std::fs::create_dir_all(&folder).expect("create the session folder");
        std::fs::write(folder.join("owner.json"), b"{}").expect("write into the session folder");
        (st, folder)
    }

    #[test]
    fn a_scoped_terminal_state_removes_its_session_folder_when_dropped() {
        let (st, folder) = scoped_state_with_a_file();
        assert!(folder.join("owner.json").is_file(), "precondition: the folder holds a file");
        drop(st);
        assert!(!folder.exists(), "the session folder outlived its test: {}", folder.display());
    }

    #[test]
    fn a_scoped_terminal_state_removes_its_session_folder_when_the_test_panics() {
        let (st, folder) = scoped_state_with_a_file();
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
            let _owned = st;
            panic!("the test failed");
        }));
        assert!(outcome.is_err(), "precondition: the closure panicked");
        assert!(!folder.exists(), "a failed test left its session folder behind: {}", folder.display());
    }

    #[test]
    fn a_stalled_first_run_does_not_fail_a_healthy_workload() {
        let mut calls = 0;
        let (fastest, calls_seen) = fastest_within(Duration::from_millis(5), Duration::from_secs(2), || {
            calls += 1;
            if calls == 1 {
                std::thread::sleep(Duration::from_millis(30));
            }
            calls
        });
        assert!(fastest < Duration::from_millis(5), "{fastest:?}");
        assert!(calls_seen >= 2, "keeps sampling past the stalled first run, saw {calls_seen}");
    }

    #[test]
    fn a_workload_that_is_really_slower_stays_over_the_budget() {
        let mut calls = 0;
        let (fastest, _) = fastest_within(Duration::from_millis(1), Duration::from_millis(60), || {
            calls += 1;
            std::thread::sleep(Duration::from_millis(3));
        });
        assert!(fastest >= Duration::from_millis(3), "{fastest:?}");
        assert!(calls > 1, "keeps sampling until patience runs out, ran {calls} time(s)");
    }

    #[cfg(unix)]
    #[test]
    fn wait_for_dir_or_child_exit_wakes_when_tab_dir_and_request_file_appear() {
        use std::io::Write;
        use std::process::{Command, Stdio};

        let channel_dir = std::env::temp_dir().join(format!("designos-dir-wait-wake-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&channel_dir);
        std::fs::create_dir_all(&channel_dir).expect("create channel dir");
        let tab_dir = channel_dir.join("tab-1");
        let req_tmp = tab_dir.join("req.request.tmp");
        let req_file = tab_dir.join("req.request.json");

        let script = format!(
            "cat >/dev/null; mkdir -p '{}'; printf '{{\"id\":\"req\"}}' > '{}'; mv -f '{}' '{}'",
            tab_dir.display(),
            req_tmp.display(),
            req_tmp.display(),
            req_file.display()
        );
        let mut child =
            Command::new("/bin/sh").arg("-c").arg(&script).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().expect("spawn child");
        let mut stdin = child.stdin.take().expect("piped stdin");
        let writer = std::thread::spawn(move || {
            let _ = stdin.write_all(b"go");
        });

        let found = wait_for_dir_or_child_exit(&channel_dir, &tab_dir, &mut child, || req_file.is_file());
        writer.join().expect("join writer");
        let _ = child.wait();
        let _ = std::fs::remove_dir_all(&channel_dir);
        assert!(found, "wait_for_dir_or_child_exit must block until the child creates tab_dir and renames the request file into place");
    }

    #[cfg(unix)]
    #[test]
    fn wait_for_dir_or_child_exit_returns_false_when_child_exits_without_writing_file() {
        use std::process::{Command, Stdio};

        let channel_dir = std::env::temp_dir().join(format!("designos-dir-wait-exit-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&channel_dir);
        std::fs::create_dir_all(&channel_dir).expect("create channel dir");
        let tab_dir = channel_dir.join("tab-1");
        let req_file = tab_dir.join("req.request.json");

        let mut child =
            Command::new("/bin/sh").arg("-c").arg("exit 0").stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().expect("spawn child");

        let found = wait_for_dir_or_child_exit(&channel_dir, &tab_dir, &mut child, || req_file.is_file());
        let _ = child.wait();
        let _ = std::fs::remove_dir_all(&channel_dir);
        assert!(!found, "wait_for_dir_or_child_exit must wake on child exit and return false when no request file was written");
    }
}
