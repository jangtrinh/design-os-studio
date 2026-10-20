//! Lint findings, fix suggestions, skipped rules, and report structures.

use super::registry::{Category, Severity};
use crate::{NodeId, Rect};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

/// Source of the active scale for an `off-scale` finding (`archetype`, `variables`, `styles`, or `default`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ScaleSource {
    Archetype,
    Variables,
    Styles,
    Default,
}

impl ScaleSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Archetype => "archetype",
            Self::Variables => "variables",
            Self::Styles => "styles",
            Self::Default => "default",
        }
    }
}

/// Structured fix suggestion attached to a [`Finding`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FixSuggestion {
    pub kind: String,
    pub property: String,
    pub from: Value,
    pub to: Value,
}

/// Alias for [`FixSuggestion`].
pub type LintFix = FixSuggestion;

/// One lint finding on a node, with page-space bounds and optional fix suggestion.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Finding {
    pub rule: &'static str,
    pub category: Category,
    pub severity: Severity,
    #[serde(rename = "node_id", serialize_with = "serialize_node_id")]
    pub node: NodeId,
    pub node_name: String,
    pub xpath: String,
    #[serde(serialize_with = "serialize_rect_wh")]
    pub bounds: Rect,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggested_fix: Option<FixSuggestion>,
    pub fixable: bool,
    /// Safe-fix tier (`true` for safe-tier findings: `hardcoded-value` with a single constant
    /// variable or whole-list style match, `sizing-conflict`; `false` for `off-scale`,
    /// mode-dependent `hardcoded-value`, and ambiguous matches).
    #[serde(default)]
    pub safe: bool,
    /// Optional target property key (e.g. `"itemSpacing"`, `"fills/0"`, `"fontSize"`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub property: Option<String>,
    /// Optional raw value inspected on the node (number or `#RRGGBB` string).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<Value>,
    /// Optional nearest step (number) or equidistant steps (`[a, b]`) for `off-scale`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nearest: Option<Value>,
    /// Optional scale source for `off-scale`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scale_source: Option<ScaleSource>,
    /// Optional candidate variable names when `hardcoded-value` is ambiguous.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub candidates: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub ignored: bool,
}

/// Alias for [`Finding`].
pub type LintFinding = Finding;

impl Finding {
    pub fn rule_id(&self) -> &'static str {
        self.rule
    }

    pub fn node_id(&self) -> NodeId {
        self.node
    }

    pub fn bbox(&self) -> Rect {
        self.bounds
    }

    pub fn suggestion(&self) -> Option<String> {
        self.suggested_fix.as_ref().map(|s| format!("{} {}: {} -> {}", s.kind, s.property, s.from, s.to))
    }
}

fn serialize_node_id<S: serde::Serializer>(id: &NodeId, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_u64(id.0)
}

fn serialize_rect_wh<S: serde::Serializer>(r: &Rect, s: S) -> Result<S::Ok, S::Error> {
    use serde::ser::SerializeMap;
    let mut m = s.serialize_map(Some(4))?;
    m.serialize_entry("x", &r.x)?;
    m.serialize_entry("y", &r.y)?;
    m.serialize_entry("width", &r.w)?;
    m.serialize_entry("height", &r.h)?;
    m.end()
}

/// A rule that was requested in the config but skipped because its required runtime capability
/// (e.g. `no_text_engine`) was not provided by the [`super::LintEnv`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkippedRule {
    pub rule: String,
    pub reason: String,
}

/// Aggregated counts for a [`LintReport`].
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LintSummary {
    pub total: usize,
    pub errors: usize,
    pub warnings: usize,
    pub infos: usize,
    pub by_severity: BTreeMap<String, usize>,
    pub by_rule: BTreeMap<String, usize>,
    pub by_category: BTreeMap<String, usize>,
}

/// Result of a lint run (full or paginated/time-sliced).
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct LintReport {
    pub findings: Vec<Finding>,
    pub total: usize,
    pub by_severity: BTreeMap<String, usize>,
    pub by_rule: BTreeMap<String, usize>,
    pub skipped_rules: Vec<SkippedRule>,
    pub scanned: usize,
    #[serde(skip_serializing)]
    pub scanned_nodes: usize,
    pub complete: bool,
    #[serde(skip_serializing)]
    pub truncated: bool,
    pub elapsed_ms: u64,
    pub cursor: Option<String>,
    #[serde(skip_serializing)]
    pub summary: LintSummary,
}

/// Alias for [`LintReport`].
pub type LintPage = LintReport;

impl LintReport {
    /// Converts the report into its canonical JSON representation (`designos_lint` output contract).
    pub fn to_value(&self) -> Value {
        serde_json::to_value(self).unwrap_or(Value::Null)
    }

    /// Returns a paginated view over `self.findings` starting at `offset` with at most `limit`
    /// items. When `cached_hit` is `true`, `scanned` and `elapsed_ms` are `0` because no nodes
    /// were rescanned.
    pub fn paginate(&self, offset: usize, limit: usize, cached_hit: bool) -> Self {
        self.paginate_with_prefix(offset, limit, cached_hit, None)
    }

    /// Paginates `self.findings` starting at `offset` with at most `limit` items, prefixing
    /// intermediate offset cursors with `stack_prefix` (`"stack:<id>+offset:<n>"`) when paging
    /// within a resumed chunk.
    pub fn paginate_with_prefix(&self, offset: usize, limit: usize, cached_hit: bool, stack_prefix: Option<&str>) -> Self {
        let total = self.findings.len();
        let start_idx = offset.min(total);
        let end_idx = (start_idx + limit).min(total);
        let findings: Vec<Finding> = self.findings[start_idx..end_idx].to_vec();
        let next_cursor = if end_idx < total {
            match stack_prefix {
                Some(prefix) => Some(format!("{prefix}+offset:{end_idx}")),
                None => Some(format!("offset:{end_idx}")),
            }
        } else if !self.complete {
            self.cursor.clone()
        } else {
            None
        };
        let truncated = !self.complete || findings.len() < total;
        Self {
            findings,
            total: self.total,
            by_severity: self.by_severity.clone(),
            by_rule: self.by_rule.clone(),
            skipped_rules: self.skipped_rules.clone(),
            scanned: if cached_hit { 0 } else { self.scanned },
            scanned_nodes: if cached_hit { 0 } else { self.scanned_nodes },
            complete: self.complete,
            truncated,
            elapsed_ms: if cached_hit { 0 } else { self.elapsed_ms },
            cursor: next_cursor,
            summary: self.summary.clone(),
        }
    }
}
