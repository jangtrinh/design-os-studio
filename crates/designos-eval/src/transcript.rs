//! MCP transcript JSONL parsing and per-run cost metrics computation (E15-25 §3.4, §3.8).

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeSet;
use std::path::Path;

/// One line logged by `designos-mcp` when `DESIGNOS_MCP_TRANSCRIPT` is set.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TranscriptLine {
    pub seq: u64,
    pub t_ms: u64,
    pub tool: String,
    #[serde(default)]
    pub args: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<Value>,
    pub is_error: bool,
    #[serde(default)]
    pub undo_depth: usize,
}

/// Cost and error metrics derived from a run's transcript (E15-25 §3.8).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CostMetrics {
    pub tool_calls: u64,
    pub tool_errors: u64,
    pub error_rate: f64,
    pub retries: u64,
    pub retry_rate: f64,
    pub undo_groups: i64,
}

/// Parses a `transcript.jsonl` file into ordered [`TranscriptLine`] records.
pub fn read_transcript(path: &Path) -> Result<Vec<TranscriptLine>, String> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let raw = std::fs::read_to_string(path).map_err(|e| format!("cannot read transcript '{}': {e}", path.display()))?;
    let mut out = Vec::new();
    for (idx, line) in raw.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let item: TranscriptLine = serde_json::from_str(trimmed).map_err(|e| format!("invalid transcript line {} in '{}': {e}", idx + 1, path.display()))?;
        out.push(item);
    }
    Ok(out)
}

/// Counts the number of differing top-level keys between two JSON argument objects.
/// Returns `usize::MAX` if either value is not a JSON object.
pub fn top_level_key_diff_count(a: &Value, b: &Value) -> usize {
    let (Some(ma), Some(mb)) = (a.as_object(), b.as_object()) else {
        return usize::from(a != b);
    };
    let keys: BTreeSet<&String> = ma.keys().chain(mb.keys()).collect();
    keys.into_iter().filter(|k| ma.get(*k) != mb.get(*k)).count()
}

/// Computes [`CostMetrics`] from a slice of [`TranscriptLine`]s per E15-25 §3.8:
/// - `retries`: a call within the next 3 calls after an error to the same tool whose args differ
///   from the failed call in at most 2 top-level keys (each failed call counts at most 1 retry).
/// - `undo_groups`: `final_undo_depth - initial_undo_depth` (`initial_undo_depth` defaults to `0`).
pub fn compute_cost_metrics(lines: &[TranscriptLine], initial_undo_depth: usize) -> CostMetrics {
    let tool_calls = lines.len() as u64;
    if tool_calls == 0 {
        return CostMetrics { tool_calls: 0, tool_errors: 0, error_rate: 0.0, retries: 0, retry_rate: 0.0, undo_groups: 0 };
    }

    let tool_errors = lines.iter().filter(|l| l.is_error).count() as u64;
    let mut retries = 0u64;
    for (i, line) in lines.iter().enumerate() {
        if !line.is_error {
            continue;
        }
        let window_end = (i + 1 + 3).min(lines.len());
        for cand in &lines[i + 1..window_end] {
            if cand.tool == line.tool && top_level_key_diff_count(&line.args, &cand.args) <= 2 {
                retries += 1;
                break;
            }
        }
    }

    let final_undo = lines.last().map(|l| l.undo_depth).unwrap_or(initial_undo_depth);
    let undo_groups = (final_undo as i64) - (initial_undo_depth as i64);
    let error_rate = (tool_errors as f64) / (tool_calls as f64);
    let retry_rate = (retries as f64) / (tool_calls as f64);

    CostMetrics { tool_calls, tool_errors, error_rate, retries, retry_rate, undo_groups }
}

/// True when some successful call added an undo step, i.e. the agent really changed the document.
/// Opening a file resets the history to zero, so reading and opening never count as an edit (E15-25 §8 edge 2).
pub fn had_document_edits(lines: &[TranscriptLine]) -> bool {
    let mut previous = 0usize;
    for line in lines {
        if !line.is_error && line.undo_depth > previous {
            return true;
        }
        previous = line.undo_depth;
    }
    false
}
