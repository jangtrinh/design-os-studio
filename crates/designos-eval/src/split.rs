//! Held-out split assignment (`sha1(salt || id)`), `split.lock` generation, and tamper verification (E15-25 §3.2).

use crate::hash::sha1_bytes;
use crate::task::Task;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

/// Dataset split for an evaluation task.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Split {
    Dev,
    HeldOut,
}

impl Split {
    pub fn as_str(&self) -> &'static str {
        match self {
            Split::Dev => "dev",
            Split::HeldOut => "held_out",
        }
    }
}

/// Recorded lock entry for one task in `suites/<suite>/split.lock`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SplitEntry {
    pub id: String,
    pub split: Split,
    pub task_sha1: String,
    #[serde(default)]
    pub inputs_sha1: BTreeMap<String, String>,
}

/// On-disk `split.lock` file for a suite.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SplitLock {
    pub suite: String,
    pub salt: String,
    pub held_out_fraction: f64,
    #[serde(default, rename = "task")]
    pub tasks: Vec<SplitEntry>,
}

/// Deterministically assigns a task ID to `Split::HeldOut` (~30%) or `Split::Dev` (~70%)
/// per E15-25 §3.2:
/// `held_out = (u32::from_be_bytes(sha1(salt || id)[0..4]) as f64 / 2^32) < 0.30`.
pub fn assign_split(salt: &str, task_id: &str) -> Split {
    let mut bytes = Vec::with_capacity(salt.len() + task_id.len());
    bytes.extend_from_slice(salt.as_bytes());
    bytes.extend_from_slice(task_id.as_bytes());
    let digest = sha1_bytes(&bytes);
    let top = u32::from_be_bytes([digest[0], digest[1], digest[2], digest[3]]);
    let u = (top as f64) / 4_294_967_296.0;
    if u < 0.30 {
        Split::HeldOut
    } else {
        Split::Dev
    }
}

/// Checks whether the held-out fraction for `entries` is within `0.30 ± 0.05` (`[0.25, 0.35]`).
/// For tiny toy suites (`< 20` tasks in unit tests) when `enforce_bounds` is false, any fraction is allowed.
pub fn held_out_fraction_within_bounds(held_out_count: usize, total: usize) -> bool {
    if total == 0 {
        return false;
    }
    let frac = (held_out_count as f64) / (total as f64);
    (0.25..=0.35).contains(&frac)
}

/// Builds a [`SplitLock`] for `tasks` using `salt`. If `enforce_bounds` is `true` (or `tasks.len() >= 20`),
/// checks that the held-out fraction lies within `[0.25, 0.35]`, failing with `split_fraction_out_of_bounds`
/// and asking for a new salt otherwise.
pub fn build_split_lock(suite: &str, salt: &str, tasks: &[Task], suite_dir: &Path, enforce_bounds: bool) -> Result<SplitLock, String> {
    if tasks.is_empty() {
        return Err(format!("empty_suite: suite '{suite}' has 0 tasks; add tasks to tasks.toml"));
    }
    let mut entries = Vec::with_capacity(tasks.len());
    let mut held_out_count = 0usize;
    for t in tasks {
        let split = assign_split(salt, &t.id);
        if split == Split::HeldOut {
            held_out_count += 1;
        }
        let (task_sha1, inputs_sha1) = t.compute_hashes(suite_dir)?;
        entries.push(SplitEntry { id: t.id.clone(), split, task_sha1, inputs_sha1 });
    }
    entries.sort_by(|a, b| a.id.cmp(&b.id));
    let frac = (held_out_count as f64) / (tasks.len() as f64);
    if (enforce_bounds || tasks.len() >= 20) && !held_out_fraction_within_bounds(held_out_count, tasks.len()) {
        return Err(format!(
            "split_fraction_out_of_bounds: suite '{suite}' with salt '{salt}' has held-out fraction {:.3} ({held_out_count}/{}); choose a new salt so the fraction is within 0.30 ± 0.05",
            frac,
            tasks.len()
        ));
    }
    Ok(SplitLock { suite: suite.to_string(), salt: salt.to_string(), held_out_fraction: frac, tasks: entries })
}

/// Verifies `tasks` against a committed [`SplitLock`]. Any missing task or modified task/input hash
/// (especially on a held-out task) returns `Err("split_lock_mismatch: ...")`.
pub fn verify_split_lock(lock: &SplitLock, tasks: &[Task], suite_dir: &Path) -> Result<BTreeMap<String, Split>, String> {
    let by_id: BTreeMap<&str, &SplitEntry> = lock.tasks.iter().map(|e| (e.id.as_str(), e)).collect();
    if by_id.len() != tasks.len() {
        return Err(format!(
            "split_lock_mismatch: suite '{}' tasks.toml has {} tasks but split.lock has {}; run `designos-eval lock --suite {} --write`",
            lock.suite,
            tasks.len(),
            by_id.len(),
            lock.suite
        ));
    }

    let mut map = BTreeMap::new();
    for t in tasks {
        let Some(entry) = by_id.get(t.id.as_str()) else {
            return Err(format!("split_lock_mismatch: task '{}' is missing from split.lock; run `designos-eval lock --suite {} --write`", t.id, lock.suite));
        };
        let expected_split = assign_split(&lock.salt, &t.id);
        if entry.split != expected_split {
            return Err(format!("split_lock_mismatch: task '{}' split {:?} in split.lock disagrees with salt '{}'", t.id, entry.split, lock.salt));
        }
        let (task_sha1, inputs_sha1) = t.compute_hashes(suite_dir)?;
        if entry.task_sha1 != task_sha1 {
            return Err(format!(
                "split_lock_mismatch: task '{}' ({}) canonical TOML hash changed ({} != {}); regenerate lock with a reviewed commit",
                t.id,
                entry.split.as_str(),
                task_sha1,
                entry.task_sha1
            ));
        }
        if entry.inputs_sha1 != inputs_sha1 {
            return Err(format!(
                "split_lock_mismatch: task '{}' ({}) input file hashes changed; regenerate lock with a reviewed commit",
                t.id,
                entry.split.as_str()
            ));
        }
        map.insert(t.id.clone(), entry.split);
    }

    Ok(map)
}
