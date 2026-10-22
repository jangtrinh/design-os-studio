//! Two-stage nonparametric bootstrap confidence intervals and task/suite summary statistics (E15-25 §3.9).

use crate::rng::Xoshiro256StarStar;

/// Computes the arithmetic mean of `values`, returning `f64::NAN` when empty.
pub fn mean(values: &[f64]) -> f64 {
    if values.is_empty() {
        return f64::NAN;
    }
    values.iter().sum::<f64>() / (values.len() as f64)
}

/// Computes the sample standard deviation (`n - 1` denominator, or `0.0` when `n <= 1`).
pub fn sample_sd(values: &[f64]) -> f64 {
    if values.is_empty() {
        return f64::NAN;
    }
    if values.len() == 1 {
        return 0.0;
    }
    let m = mean(values);
    let var = values
        .iter()
        .map(|x| {
            let d = *x - m;
            d * d
        })
        .sum::<f64>()
        / ((values.len() - 1) as f64);
    var.sqrt()
}

/// Hyndman & Fan type-7 quantile interpolation on a non-empty sorted slice.
pub fn percentile_type7(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return f64::NAN;
    }
    if sorted.len() == 1 {
        return sorted[0];
    }
    let p = p.clamp(0.0, 1.0);
    let n = sorted.len();
    let h = (n - 1) as f64 * p;
    let lo = h.floor() as usize;
    let hi = h.ceil() as usize;
    if lo == hi {
        sorted[lo]
    } else {
        let frac = h - (lo as f64);
        sorted[lo] + frac * (sorted[hi] - sorted[lo])
    }
}

/// Returns `"bootstrap-two-stage"` when any task has more than 1 run, or `"bootstrap-tasks"` when
/// every task has at most 1 run.
pub fn ci_method_for_runs(per_task_runs: &[Vec<f64>]) -> &'static str {
    if per_task_runs.iter().any(|runs| runs.len() > 1) {
        "bootstrap-two-stage"
    } else {
        "bootstrap-tasks"
    }
}

/// Computes `(suite_mean, ci95_low, ci95_high)` via two-stage nonparametric bootstrap with `b`
/// resamples (default `10_000`) seeded by `seed` through `xoshiro256**`.
///
/// - Per task `t`: `x̄_t = mean(per_task_runs[t])`.
/// - Suite mean: `M = mean_t(x̄_t)`.
/// - In each of `b` resamples:
///   - Draws `n_tasks` tasks with replacement.
///   - If `max_runs > 1` (`bootstrap-two-stage`), draws `runs.len()` values with replacement within
///     each drawn task and averages them; if all tasks have 1 run (`bootstrap-tasks`), skips the
///     second stage and uses the drawn task's single run value directly.
///   - Records the mean over the `n_tasks` drawn task means.
/// - Returns `(M, p2.5, p97.5)` using type-7 percentile interpolation.
pub fn bootstrap_ci(per_task_runs: &[Vec<f64>], b: u32, seed: u64) -> (f64, f64, f64) {
    let valid_tasks: Vec<&Vec<f64>> = per_task_runs.iter().filter(|runs| !runs.is_empty() && runs.iter().all(|v| v.is_finite())).collect();
    if valid_tasks.is_empty() {
        return (f64::NAN, f64::NAN, f64::NAN);
    }

    let task_means: Vec<f64> = valid_tasks.iter().map(|r| mean(r)).collect();
    let suite_mean = mean(&task_means);
    let n_tasks = valid_tasks.len();
    let two_stage = valid_tasks.iter().any(|r| r.len() > 1);
    let b_count = b.max(1) as usize;

    let mut rng = Xoshiro256StarStar::seed_from_u64(seed);
    let mut boot_means = Vec::with_capacity(b_count);

    for _ in 0..b_count {
        let mut sum = 0.0f64;
        for _ in 0..n_tasks {
            let t_idx = rng.gen_index(n_tasks);
            let runs = valid_tasks[t_idx];
            if two_stage && runs.len() > 1 {
                let k = runs.len();
                let mut r_sum = 0.0f64;
                for _ in 0..k {
                    let r_idx = rng.gen_index(k);
                    r_sum += runs[r_idx];
                }
                sum += r_sum / (k as f64);
            } else {
                sum += runs[0];
            }
        }
        boot_means.push(sum / (n_tasks as f64));
    }

    boot_means.sort_by(|a, b| a.total_cmp(b));
    let ci_lo = percentile_type7(&boot_means, 0.025);
    let ci_hi = percentile_type7(&boot_means, 0.975);
    (suite_mean, ci_lo, ci_hi)
}
