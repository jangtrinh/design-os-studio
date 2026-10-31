//! Per-session ring of hashed MCP calls. Detects identical repeats, ping-pong, and repeated write errors.

use crate::rules::canonical_tool_name;
use crate::tools::errors::{ErrorCode, ToolError};
use crate::tools::ledger::payload_hash;
use serde_json::{json, Value};
use std::collections::{HashMap, VecDeque};
use std::sync::OnceLock;

pub const DEFAULT_K: usize = 3;
pub const DEFAULT_N: usize = 20;
pub const ENV_K: &str = "DESIGNOS_LOOP_GUARD_K";
pub const ENV_N: &str = "DESIGNOS_LOOP_GUARD_N";

const MAX_K: usize = 1_024;
const MAX_N: usize = 10_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoopVerdict {
    None,
    Advisory,
    Hard,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CallFingerprint {
    tool: String,
    args_hash: u64,
    result_hash: u64,
    error_code: Option<ErrorCode>,
    read_only: bool,
}

impl CallFingerprint {
    pub fn from_call(name: &str, args: &Value, outcome: &Result<Value, ToolError>) -> Self {
        let tool = canonical_tool_name(name).to_string();
        let args_hash = payload_hash(&tool, args);
        let (result_hash, error_code) = match outcome {
            Ok(value) => (payload_hash("\0result", value), None),
            Err(err) => (payload_hash("\0error", &json!({"code": err.code.as_str(), "message": err.message})), Some(err.code)),
        };
        Self { tool: tool.clone(), args_hash, result_hash, error_code, read_only: is_read_only_tool(&tool) }
    }

    fn identity_eq(&self, other: &Self) -> bool {
        self.tool == other.tool && self.args_hash == other.args_hash && self.result_hash == other.result_hash
    }
}

pub struct LoopGuard {
    k: usize,
    n: usize,
    buf: VecDeque<CallFingerprint>,
    anchor_revision: Option<u64>,
}

impl LoopGuard {
    pub fn from_env() -> Self {
        Self::from_overrides(std::env::var(ENV_K).ok().as_deref(), std::env::var(ENV_N).ok().as_deref())
    }

    pub fn from_overrides(k_raw: Option<&str>, n_raw: Option<&str>) -> Self {
        let k = parse_k(k_raw);
        let n = parse_n(n_raw, k);
        Self::with_thresholds(k, n)
    }

    pub fn with_thresholds(k: usize, n: usize) -> Self {
        let k = k.clamp(2, MAX_K);
        let n = n.clamp(k, MAX_N);
        Self { k, n, buf: VecDeque::with_capacity(n), anchor_revision: None }
    }

    pub fn k(&self) -> usize {
        self.k
    }

    #[cfg(test)]
    pub fn n(&self) -> usize {
        self.n
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.buf.len()
    }

    pub fn reset(&mut self) {
        self.buf.clear();
        self.anchor_revision = None;
    }

    pub fn note_revision(&mut self, revision: u64) {
        if let Some(prev) = self.anchor_revision {
            if prev != revision {
                self.buf.clear();
            }
        }
        self.anchor_revision = Some(revision);
    }

    pub fn observe(&mut self, rec: CallFingerprint) -> LoopVerdict {
        let successful_read = rec.read_only && rec.error_code.is_none();
        self.push(rec);
        if !self.is_loop() {
            LoopVerdict::None
        } else if successful_read {
            LoopVerdict::Advisory
        } else {
            LoopVerdict::Hard
        }
    }

    fn push(&mut self, rec: CallFingerprint) {
        self.buf.push_back(rec);
        while self.buf.len() > self.n {
            self.buf.pop_front();
        }
    }

    fn is_loop(&self) -> bool {
        self.repeat_k() || self.ping_pong_k() || self.write_error_k()
    }

    fn repeat_k(&self) -> bool {
        if self.buf.len() < self.k {
            return false;
        }
        let last = self.buf.back().expect("repeat_k checked len");
        self.buf.iter().rev().take(self.k).all(|r| r.identity_eq(last))
    }

    fn ping_pong_k(&self) -> bool {
        let need = self.k.saturating_mul(2);
        if self.buf.len() < need {
            return false;
        }
        let recs: Vec<&CallFingerprint> = self.buf.iter().rev().take(need).collect();
        let b = recs[0];
        let a = recs[1];
        if a.identity_eq(b) {
            return false;
        }
        recs.chunks(2).all(|pair| pair.len() == 2 && pair[0].identity_eq(b) && pair[1].identity_eq(a))
    }

    fn write_error_k(&self) -> bool {
        if self.buf.len() < self.k {
            return false;
        }
        let last = self.buf.back().expect("write_error_k checked len");
        let Some(code) = last.error_code else {
            return false;
        };
        if last.read_only {
            return false;
        }
        self.buf.iter().rev().take(self.k).all(|r| !r.read_only && r.error_code == Some(code) && r.tool == last.tool && r.args_hash == last.args_hash)
    }
}

pub fn loop_suspected_error(k: usize) -> ToolError {
    let times = k.max(2);
    ToolError::new(ErrorCode::LoopSuspected, "", format!("The same call has returned the same result {times} times"))
        .with_next_step("Change the arguments, read the document state, or ask the user.")
}

pub fn loop_suspected_warning(k: usize) -> Value {
    let err = loop_suspected_error(k);
    json!({
        "code": err.code.as_str(),
        "message": err.message,
        "next_step": err.next_step,
        "safe_to_retry": err.safe_to_retry,
    })
}

fn parse_k(raw: Option<&str>) -> usize {
    raw.and_then(|s| s.parse::<usize>().ok()).filter(|&v| (2..=MAX_K).contains(&v)).unwrap_or(DEFAULT_K)
}

fn parse_n(raw: Option<&str>, k: usize) -> usize {
    raw.and_then(|s| s.parse::<usize>().ok()).filter(|&v| v >= k && v <= MAX_N).unwrap_or(DEFAULT_N.max(k))
}

fn is_read_only_tool(canonical_name: &str) -> bool {
    static MAP: OnceLock<HashMap<String, bool>> = OnceLock::new();
    let map = MAP.get_or_init(|| {
        crate::server::tools()
            .into_iter()
            .map(|t| {
                let read_only = t.annotations.as_ref().and_then(|a| a.read_only_hint).unwrap_or(false);
                (t.name.to_string(), read_only)
            })
            .collect()
    });
    map.get(canonical_name).copied().unwrap_or(false)
}

#[cfg(test)]
fn fp(tool: &str, args: &Value, result: &Value, error: Option<ErrorCode>, read_only: bool) -> CallFingerprint {
    fp_msg(tool, args, result, error, "err", read_only)
}

#[cfg(test)]
fn fp_msg(tool: &str, args: &Value, result: &Value, error: Option<ErrorCode>, message: &str, read_only: bool) -> CallFingerprint {
    let tool = canonical_tool_name(tool).to_string();
    let args_hash = payload_hash(&tool, args);
    let result_hash = match error {
        Some(code) => payload_hash("\0error", &json!({"code": code.as_str(), "message": message})),
        None => payload_hash("\0result", result),
    };
    CallFingerprint { tool, args_hash, result_hash, error_code: error, read_only }
}

impl std::fmt::Debug for LoopGuard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LoopGuard").field("k", &self.k).field("n", &self.n).field("len", &self.buf.len()).field("buf", &self.buf).finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const SENTINEL: &str = "SENTINEL_ARG_TEXT_NOT_TO_BE_STORED_xyzzq1290";

    fn repeat_args() -> Value {
        json!({"node_id": 99, "name": SENTINEL})
    }

    #[test]
    fn repeat_identical_calls_fire_on_kth() {
        let mut g = LoopGuard::with_thresholds(3, 20);
        let a = json!({"node_id": 7});
        let rec = fp("designos_update_node", &a, &json!({"ok": false}), Some(ErrorCode::NodeNotFound), false);
        assert_eq!(g.observe(rec.clone()), LoopVerdict::None, "first occurrence never fires");
        assert_eq!(g.observe(rec.clone()), LoopVerdict::None, "second of K=3 does not fire");
        assert_eq!(g.observe(rec), LoopVerdict::Hard, "third identical call fires");
    }

    #[test]
    fn a_three_streak_after_a_different_call_fires_on_the_third_repeat() {
        let mut g = LoopGuard::with_thresholds(3, 20);
        let b = fp("designos_get_node", &json!({"node_id": 1}), &json!({"id": 1}), None, true);
        let a = fp("designos_get_node", &json!({"node_id": 2}), &json!({"id": 2}), None, true);
        assert_eq!(g.observe(b), LoopVerdict::None, "leading B never fires");
        assert_eq!(g.observe(a.clone()), LoopVerdict::None, "first A never fires");
        assert_eq!(g.observe(a.clone()), LoopVerdict::None, "second A of K=3 does not fire");
        assert_eq!(g.observe(a), LoopVerdict::Advisory, "third A after B fires");
    }

    #[test]
    fn repeat_identical_reads_are_advisory() {
        let mut g = LoopGuard::with_thresholds(3, 20);
        let rec = fp("designos_get_document", &json!({}), &json!({"n": 1}), None, true);
        assert_eq!(g.observe(rec.clone()), LoopVerdict::None);
        assert_eq!(g.observe(rec.clone()), LoopVerdict::None);
        assert_eq!(g.observe(rec.clone()), LoopVerdict::Advisory);
        assert_eq!(g.observe(rec), LoopVerdict::Advisory, "later identical reads stay advisory");
    }

    #[test]
    fn repeat_identical_failing_reads_are_hard() {
        let mut g = LoopGuard::with_thresholds(3, 20);
        let rec = fp("designos_get_node", &json!({"node_id": 1}), &json!({}), Some(ErrorCode::NodeNotFound), true);
        assert_eq!(g.observe(rec.clone()), LoopVerdict::None);
        assert_eq!(g.observe(rec.clone()), LoopVerdict::None);
        assert_eq!(g.observe(rec), LoopVerdict::Hard, "a failing read-only loop is a named error, not an advisory");
    }

    #[test]
    fn repeat_identical_write_ok_is_hard() {
        let mut g = LoopGuard::with_thresholds(3, 20);
        let rec = fp("designos_undo", &json!({}), &json!({"changed": false}), None, false);
        assert_eq!(g.observe(rec.clone()), LoopVerdict::None);
        assert_eq!(g.observe(rec.clone()), LoopVerdict::None);
        assert_eq!(g.observe(rec), LoopVerdict::Hard);
    }

    #[test]
    fn read_only_changed_result_never_fires() {
        let mut g = LoopGuard::with_thresholds(3, 20);
        let a = json!({});
        let r1 = fp("designos_get_document", &a, &json!({"n": 1}), None, true);
        let r2 = fp("designos_get_document", &a, &json!({"n": 2}), None, true);
        let r3 = fp("designos_get_document", &a, &json!({"n": 3}), None, true);
        assert_eq!(g.observe(r1), LoopVerdict::None);
        assert_eq!(g.observe(r2), LoopVerdict::None);
        assert_eq!(g.observe(r3), LoopVerdict::None);
    }

    #[test]
    fn ping_pong_fires_after_k_cycles() {
        let mut g = LoopGuard::with_thresholds(3, 20);
        let a = fp("designos_get_node", &json!({"node_id": 1}), &json!({"id": 1}), None, true);
        let b = fp("designos_get_node", &json!({"node_id": 2}), &json!({"id": 2}), None, true);
        for cycle in 0..3 {
            let fired_a = g.observe(a.clone());
            let fired_b = g.observe(b.clone());
            if cycle < 2 {
                assert_eq!(fired_a, LoopVerdict::None, "cycle {} must stay silent", cycle + 1);
                assert_eq!(fired_b, LoopVerdict::None, "cycle {} must stay silent", cycle + 1);
            } else {
                assert_eq!(fired_a, LoopVerdict::None);
                assert_eq!(fired_b, LoopVerdict::Advisory, "Kth ping-pong cycle of two reads is advisory");
            }
        }
    }

    #[test]
    fn write_error_repeat_fires_on_same_code() {
        let mut g = LoopGuard::with_thresholds(3, 20);
        let args = json!({"node_id": 1});
        let e1 = fp_msg("designos_update_node", &args, &json!({}), Some(ErrorCode::NodeNotFound), "missing-a", false);
        let e2 = fp_msg("designos_update_node", &args, &json!({}), Some(ErrorCode::NodeNotFound), "missing-b", false);
        let e3 = fp_msg("designos_update_node", &args, &json!({}), Some(ErrorCode::NodeNotFound), "missing-c", false);
        assert_eq!(g.observe(e1), LoopVerdict::None);
        assert_eq!(g.observe(e2), LoopVerdict::None);
        assert_eq!(g.observe(e3), LoopVerdict::Hard, "same write tool, args, and error code fire even when the message text changes");
    }

    #[test]
    fn different_args_reset_repeat_streak() {
        let mut g = LoopGuard::with_thresholds(3, 20);
        let a = fp("designos_update_node", &json!({"node_id": 1}), &json!({}), Some(ErrorCode::NodeNotFound), false);
        let b = fp("designos_update_node", &json!({"node_id": 9}), &json!({"x": 1}), Some(ErrorCode::InvalidArguments), false);
        assert_eq!(g.observe(a.clone()), LoopVerdict::None);
        assert_eq!(g.observe(a.clone()), LoopVerdict::None);
        assert_eq!(g.observe(b), LoopVerdict::None, "a different argument hash breaks the identical-repeat streak");
        assert_eq!(g.observe(a.clone()), LoopVerdict::None);
        assert_eq!(g.observe(a.clone()), LoopVerdict::None);
        assert_eq!(g.observe(a), LoopVerdict::Hard, "a fresh run of K identical calls still fires");
    }

    #[test]
    fn revision_change_clears_the_window() {
        let mut g = LoopGuard::with_thresholds(3, 20);
        let rec = fp("designos_update_node", &json!({"node_id": 1}), &json!({}), Some(ErrorCode::NodeNotFound), false);
        g.note_revision(0);
        assert_eq!(g.observe(rec.clone()), LoopVerdict::None);
        assert_eq!(g.observe(rec.clone()), LoopVerdict::None);
        g.note_revision(1);
        assert_eq!(g.observe(rec.clone()), LoopVerdict::None, "a revision change resets the window");
        assert_eq!(g.observe(rec.clone()), LoopVerdict::None);
        assert_eq!(g.observe(rec), LoopVerdict::Hard, "K identical calls after the reset fire");
    }

    #[test]
    fn once_fired_keeps_firing_while_the_loop_continues() {
        let mut g = LoopGuard::with_thresholds(3, 20);
        let rec = fp("designos_update_node", &json!({"node_id": 1}), &json!({}), Some(ErrorCode::NodeNotFound), false);
        assert_eq!(g.observe(rec.clone()), LoopVerdict::None);
        assert_eq!(g.observe(rec.clone()), LoopVerdict::None);
        assert_eq!(g.observe(rec.clone()), LoopVerdict::Hard);
        assert_eq!(g.observe(rec.clone()), LoopVerdict::Hard, "fourth identical call still fires");
        assert_eq!(g.observe(rec.clone()), LoopVerdict::Hard, "fifth still fires");
        assert_eq!(g.observe(rec), LoopVerdict::Hard, "sixth still fires");
    }

    #[test]
    fn ring_is_bounded_to_n() {
        let mut g = LoopGuard::with_thresholds(3, 20);
        let rec = fp("designos_get_document", &json!({}), &json!({"i": 0}), None, true);
        for i in 0..25 {
            let mut r = rec.clone();
            r.result_hash = i;
            g.observe(r);
        }
        assert_eq!(g.len(), 20);
        assert!(g.len() <= g.n());
    }

    #[test]
    fn k_two_via_env_override() {
        let g = LoopGuard::from_overrides(Some("2"), None);
        assert_eq!(g.k(), 2);
        assert_eq!(g.n(), DEFAULT_N);
        let mut g = g;
        let rec = fp("designos_update_node", &json!({"node_id": 1}), &json!({}), Some(ErrorCode::NodeNotFound), false);
        assert_eq!(g.observe(rec.clone()), LoopVerdict::None, "K=2 never fires on the first call");
        assert_eq!(g.observe(rec), LoopVerdict::Hard, "K=2 fires on the second identical call");
    }

    #[test]
    fn invalid_env_values_fall_back_to_defaults() {
        assert_eq!(LoopGuard::from_overrides(None, None).k(), DEFAULT_K);
        assert_eq!(LoopGuard::from_overrides(Some("nope"), Some("-1")).k(), DEFAULT_K);
        assert_eq!(LoopGuard::from_overrides(Some("1"), Some("0")).k(), DEFAULT_K);
        assert_eq!(LoopGuard::from_overrides(Some("0"), None).n(), DEFAULT_N);
        assert_eq!(LoopGuard::from_overrides(Some("3"), Some("2")).n(), DEFAULT_N);
    }

    #[test]
    fn ping_pong_ending_on_a_write_is_hard() {
        let mut g = LoopGuard::with_thresholds(2, 20);
        let a = fp("designos_get_document", &json!({}), &json!({"n": 1}), None, true);
        let b = fp("designos_undo", &json!({}), &json!({"changed": false}), None, false);
        assert_eq!(g.observe(a.clone()), LoopVerdict::None);
        assert_eq!(g.observe(b.clone()), LoopVerdict::None);
        assert_eq!(g.observe(a), LoopVerdict::None);
        assert_eq!(g.observe(b), LoopVerdict::Hard);
    }

    #[test]
    fn no_argument_text_kept() {
        let mut g = LoopGuard::with_thresholds(3, 20);
        let args = repeat_args();
        let rec = fp("designos_update_node", &args, &json!({}), Some(ErrorCode::NodeNotFound), false);
        g.observe(rec);
        let dumped = format!("{g:?}");
        assert!(!dumped.contains(SENTINEL), "ring Debug must not keep argument text: {dumped}");
        assert!(!format!("{:?}", g.buf).contains(SENTINEL));
    }
}
