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

#[cfg(test)]
mod tests {
    use super::*;

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
