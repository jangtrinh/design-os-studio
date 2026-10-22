//! JSONL result schema (`meta`, `run`, `task`, `suite`), finite float sanitization, and JSON Schema validation (E15-25 §4).

use crate::split::Split;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;

/// Outcome status of a single run.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunExit {
    Ok,
    Timeout,
    Crashed,
    Budget,
}

/// Host OS and architecture recorded on the `meta` JSONL line.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostMeta {
    pub os: String,
    pub arch: String,
}

impl Default for HostMeta {
    fn default() -> Self {
        Self { os: std::env::consts::OS.to_string(), arch: std::env::consts::ARCH.to_string() }
    }
}

/// Evaluation mode (`tuning` refuses held-out tasks; `release` runs all tasks).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvalMode {
    Tuning,
    Release,
}

/// Schema v1 `meta` JSONL record.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Meta {
    pub schema: u32,
    pub release: String,
    pub git: String,
    pub preamble_sha1: String,
    pub started: String,
    pub agent: String,
    pub model: String,
    pub judge_model: Option<String>,
    pub mode: EvalMode,
    pub runs: u32,
    pub host: HostMeta,
}

/// Artefact paths relative to the suite output directory for a single run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunArtefacts {
    pub doc: String,
    pub renders: Vec<String>,
    pub transcript: String,
}

/// Schema v1 `run` JSONL record.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RunRecord {
    pub task: String,
    pub split: Split,
    pub run: u32,
    pub exit: RunExit,
    pub wall_ms: u64,
    pub tool_calls: u64,
    pub tool_errors: u64,
    pub error_rate: Option<f64>,
    pub retries: u64,
    pub retry_rate: Option<f64>,
    pub undo_groups: i64,
    pub tokens_in: Option<u64>,
    pub tokens_out: Option<u64>,
    pub metrics: BTreeMap<String, Option<f64>>,
    pub artefacts: RunArtefacts,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
}

/// Schema v1 `task` summary JSONL record.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TaskSummary {
    pub task: String,
    pub split: Split,
    pub runs: u32,
    pub mean: BTreeMap<String, Option<f64>>,
    pub sd: BTreeMap<String, Option<f64>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
}

/// Schema v1 `suite` summary JSONL record.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SuiteSummary {
    pub suite: String,
    pub split: String,
    pub n_tasks: usize,
    pub metric: String,
    pub mean: Option<f64>,
    pub ci95: [Option<f64>; 2],
    pub ci_method: String,
    #[serde(rename = "B")]
    pub b: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
}

/// Discriminated union of all JSONL records in `results/<release>/<suite>.jsonl`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EvalRecord {
    Meta(Meta),
    Run(RunRecord),
    Task(TaskSummary),
    Suite(SuiteSummary),
}

/// Converts a raw `f64` into a finite `Option<f64>`, pushing a diagnostic entry into `notes`
/// whenever a non-finite (`NaN` or `±Infinity`) value is converted to `None` (E15-25 §4).
pub fn sanitize_finite(val: f64, label: &str, notes: &mut Vec<String>) -> Option<f64> {
    if val.is_finite() {
        Some(val)
    } else {
        notes.push(format!("{label} was non-finite ({val}) and written as null"));
        None
    }
}

/// Embedded JSON Schema contract (`crates/designos-eval/schema/results-v1.json`).
pub const RESULTS_V1_SCHEMA_JSON: &str = include_str!("../schema/results-v1.json");

/// Validates a single JSONL line against the structural and type constraints of `schema/results-v1.json`.
pub fn validate_jsonl_line(line: &str) -> Result<EvalRecord, String> {
    let val: Value = serde_json::from_str(line).map_err(|e| format!("invalid JSON on JSONL line: {e}"))?;
    let obj = val.as_object().ok_or_else(|| "JSONL line must be a JSON object".to_string())?;
    let ty = obj.get("type").and_then(Value::as_str).ok_or_else(|| "missing string field 'type'".to_string())?;

    // Check against embedded schema's required fields for that variant
    let schema_val: Value = serde_json::from_str(RESULTS_V1_SCHEMA_JSON).expect("valid embedded results-v1.json");
    let variants = schema_val.get("oneOf").and_then(Value::as_array).expect("oneOf in results-v1.json");
    let matching_variant = variants
        .iter()
        .find(|v| v.pointer("/properties/type/const").and_then(Value::as_str) == Some(ty))
        .ok_or_else(|| format!("unknown record type '{ty}' (not in results-v1.json)"))?;

    if let Some(req_arr) = matching_variant.get("required").and_then(Value::as_array) {
        for req_key in req_arr.iter().filter_map(Value::as_str) {
            if !obj.contains_key(req_key) {
                return Err(format!("record of type '{ty}' is missing required field '{req_key}'"));
            }
        }
    }

    // Ensure metric keys are snake_case when present
    if ty == "run" {
        if let Some(metrics) = obj.get("metrics").and_then(Value::as_object) {
            for k in metrics.keys() {
                if !is_snake_case(k) {
                    return Err(format!("metric key '{k}' must be snake_case"));
                }
            }
        }
    }
    if ty == "suite" {
        if let Some(m) = obj.get("metric").and_then(Value::as_str) {
            if !is_snake_case(m) {
                return Err(format!("suite metric '{m}' must be snake_case"));
            }
        }
    }

    let record: EvalRecord = serde_json::from_value(json!(val)).map_err(|e| format!("record failed EvalRecord deserialization: {e}"))?;
    Ok(record)
}

fn is_snake_case(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}
