//! Task schema, canonical TOML hashing, and E15-23 source register verification (E15-25 §3.1, §3.12).

use crate::hash::sha1_hex;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

fn default_surface() -> String {
    "none".to_string()
}
fn default_start() -> String {
    "blank".to_string()
}
fn default_design_system() -> String {
    "none".to_string()
}
fn default_max_tool_calls() -> u32 {
    400
}
fn default_timeout_s() -> u64 {
    900
}

/// Execution budget for a single evaluation task run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskBudget {
    #[serde(default = "default_max_tool_calls")]
    pub max_tool_calls: u32,
    #[serde(default = "default_timeout_s")]
    pub timeout_s: u64,
}

impl Default for TaskBudget {
    fn default() -> Self {
        Self { max_tool_calls: default_max_tool_calls(), timeout_s: default_timeout_s() }
    }
}

/// One evaluation task definition from `suites/<suite>/tasks.toml`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub suite: String,
    pub source: String,
    #[serde(default = "default_surface")]
    pub surface: String,
    #[serde(default)]
    pub brief: String,
    #[serde(default)]
    pub inputs: Vec<String>,
    #[serde(default = "default_start")]
    pub start: String,
    #[serde(default = "default_design_system")]
    pub design_system: String,
    #[serde(default)]
    pub checklist: Vec<String>,
    #[serde(default)]
    pub scope: Vec<String>,
    #[serde(default)]
    pub output: Vec<String>,
    #[serde(default)]
    pub budget: TaskBudget,
    /// Optional script path for `stub-agent` runs when a test per-task script is provided.
    #[serde(default)]
    pub stub_script: Option<String>,
}

/// Wrapper for `suites/<suite>/tasks.toml` (`[[task]]` array).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TasksFile {
    #[serde(default, rename = "task")]
    pub tasks: Vec<Task>,
}

impl Task {
    /// Serializes this task into canonical JSON/TOML order (all keys sorted) and returns its
    /// 40-char lowercase SHA-1 hex digest.
    pub fn canonical_toml_sha1(&self) -> String {
        let mut map = BTreeMap::<String, serde_json::Value>::new();
        map.insert("id".into(), serde_json::json!(self.id));
        map.insert("suite".into(), serde_json::json!(self.suite));
        map.insert("source".into(), serde_json::json!(self.source));
        map.insert("surface".into(), serde_json::json!(self.surface));
        map.insert("brief".into(), serde_json::json!(self.brief));
        map.insert("inputs".into(), serde_json::json!(self.inputs));
        map.insert("start".into(), serde_json::json!(self.start));
        map.insert("design_system".into(), serde_json::json!(self.design_system));
        map.insert("checklist".into(), serde_json::json!(self.checklist));
        map.insert("scope".into(), serde_json::json!(self.scope));
        map.insert("output".into(), serde_json::json!(self.output));
        let mut budget_map = BTreeMap::<String, serde_json::Value>::new();
        budget_map.insert("max_tool_calls".into(), serde_json::json!(self.budget.max_tool_calls));
        budget_map.insert("timeout_s".into(), serde_json::json!(self.budget.timeout_s));
        map.insert("budget".into(), serde_json::json!(budget_map));
        let canonical = serde_json::to_string(&map).expect("canonical task serialization");
        sha1_hex(canonical.as_bytes())
    }

    /// Computes `(canonical_toml_sha1, BTreeMap<input_rel_path, file_sha1>)` relative to `suite_dir`.
    pub fn compute_hashes(&self, suite_dir: &Path) -> Result<(String, BTreeMap<String, String>), String> {
        let toml_sha1 = self.canonical_toml_sha1();
        let mut input_hashes = BTreeMap::new();
        for rel in &self.inputs {
            let p = suite_dir.join(rel);
            let bytes = std::fs::read(&p).map_err(|e| format!("missing_input: cannot read input '{}' for task '{}': {e}", p.display(), self.id))?;
            input_hashes.insert(rel.clone(), sha1_hex(&bytes));
        }
        Ok((toml_sha1, input_hashes))
    }
}

/// Parsed source register from `docs/eval-licences.md`.
#[derive(Clone, Debug)]
pub struct SourceRegister {
    pub approved: BTreeSet<String>,
    pub known_non_approved: BTreeMap<String, String>,
    pub ban_regexes: Vec<Regex>,
}

#[derive(Deserialize)]
struct RawRegisterBlock {
    #[serde(default)]
    source: Vec<RawSourceEntry>,
    #[serde(default)]
    ban: Option<RawBanSection>,
}

#[derive(Deserialize)]
struct RawSourceEntry {
    id: String,
    review: String,
    #[serde(default)]
    r#use: String,
}

#[derive(Deserialize)]
struct RawBanSection {
    #[serde(default)]
    patterns: Vec<String>,
}

impl SourceRegister {
    /// Loads `docs/eval-licences.md` from `repo_root`. The register is the only authority on which sources may
    /// run, so a missing register or one without ban patterns is an error: there is no built-in fallback list.
    pub fn load(repo_root: Option<&Path>) -> Result<Self, String> {
        let root =
            repo_root.ok_or_else(|| "register_missing: no repository root to read docs/eval-licences.md from; run from the repository root".to_string())?;
        let md_path = root.join("docs/eval-licences.md");
        if !md_path.is_file() {
            return Err(format!("register_missing: '{}' does not exist; the E15-23 licence register is required to run tasks", md_path.display()));
        }
        let raw = std::fs::read_to_string(&md_path).map_err(|e| format!("cannot read {}: {e}", md_path.display()))?;
        Self::from_markdown(&raw)
    }

    /// Parses the fenced `toml` register block from `docs/eval-licences.md`.
    pub fn from_markdown(md: &str) -> Result<Self, String> {
        let mut toml_blocks = Vec::new();
        let mut in_toml = false;
        let mut cur = String::new();
        for line in md.lines() {
            let trimmed = line.trim();
            if !in_toml && trimmed.starts_with("```toml") {
                in_toml = true;
                cur.clear();
            } else if in_toml && trimmed == "```" {
                in_toml = false;
                toml_blocks.push(cur.clone());
            } else if in_toml {
                cur.push_str(line);
                cur.push('\n');
            }
        }
        if toml_blocks.is_empty() {
            return Err("register_parse_error: no ```toml block found in docs/eval-licences.md".into());
        }
        let parsed: RawRegisterBlock = toml::from_str(&toml_blocks[0]).map_err(|e| format!("register_parse_error: invalid TOML in register: {e}"))?;
        let mut approved = BTreeSet::new();
        let mut known_non_approved = BTreeMap::new();
        for src in parsed.source {
            if src.review == "approved" && src.r#use != "excluded" {
                approved.insert(src.id);
            } else {
                known_non_approved.insert(src.id, src.review);
            }
        }
        let patterns = parsed
            .ban
            .map(|b| b.patterns)
            .filter(|p| !p.is_empty())
            .ok_or_else(|| "register_parse_error: no [ban].patterns in docs/eval-licences.md".to_string())?;
        let mut ban_regexes = Vec::with_capacity(patterns.len());
        for pat in patterns {
            let re = Regex::new(&pat).map_err(|e| format!("register_parse_error: invalid ban regex '{pat}': {e}"))?;
            ban_regexes.push(re);
        }
        Ok(Self { approved, known_non_approved, ban_regexes })
    }

    /// Verifies that `task` uses an `approved` source and contains no banned client strings.
    pub fn verify_task(&self, task: &Task) -> Result<(), String> {
        for field in [task.id.as_str(), task.source.as_str(), task.brief.as_str(), task.start.as_str(), task.design_system.as_str()]
            .into_iter()
            .chain(task.inputs.iter().map(String::as_str))
        {
            for re in &self.ban_regexes {
                if re.is_match(field) {
                    return Err(format!("source_not_approved: task '{}' matches banned client pattern '{}' in '{}'", task.id, re.as_str(), field));
                }
            }
        }

        if !self.approved.contains(&task.source) {
            let state = self.known_non_approved.get(&task.source).map(String::as_str).unwrap_or("unregistered");
            return Err(format!(
                "source_not_approved: task '{}' uses source '{}' (status: {}); approve it in docs/eval-licences.md before running",
                task.id, task.source, state
            ));
        }
        Ok(())
    }
}
