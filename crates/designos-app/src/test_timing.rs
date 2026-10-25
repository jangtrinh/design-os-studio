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

/// A [`TerminalState`] for a test that removes its session folder (`<tmp>/designos-sessions/p<pid>-tab-<id>`) when the test ends, passed or
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
}
