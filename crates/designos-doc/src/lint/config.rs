//! Configuration, presets, rule severity overrides, and thresholds for the unified linter.

use super::registry::{all_preset_names, all_rule_ids, resolve_rule, Category, LintPreset, Severity, RULES};
use crate::ContrastLevel;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Error returned when a caller specifies an unknown preset or unknown rule ID.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LintConfigError {
    UnknownPreset { received: String, valid_options: Vec<&'static str> },
    UnknownRule { received: String, valid_options: Vec<&'static str> },
}

impl std::fmt::Display for LintConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LintConfigError::UnknownPreset { received, valid_options } => {
                write!(f, "UNKNOWN_PRESET: unknown lint preset '{received}'; valid_options: {valid_options:?} (safe_to_retry: true)")
            }
            LintConfigError::UnknownRule { received, valid_options } => {
                write!(f, "UNKNOWN_RULE: unknown lint rule '{received}'; valid_options: {valid_options:?} (safe_to_retry: true)")
            }
        }
    }
}

impl std::error::Error for LintConfigError {}

/// Per-rule severity override (`"off"` / `false`, `"error"`, `"warning"`, `"info"`, or `true` to enable with default severity).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RuleSeverityConfig {
    Enabled(bool),
    Level(RuleSeverityLevel),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RuleSeverityLevel {
    Off,
    Error,
    Warning,
    #[serde(alias = "warn")]
    Info,
}

/// Configurable numeric thresholds used by geometric and accessibility rules.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LintThresholds {
    #[serde(default = "default_tolerance")]
    pub geometry_tolerance: f32,
    #[serde(default = "default_min_overlap_area")]
    pub min_overlap_area: f32,
    #[serde(default = "default_max_nesting_depth")]
    pub max_nesting_depth: usize,
    #[serde(default = "default_grid_step")]
    pub grid_step: f32,
    #[serde(default = "default_min_touch_target")]
    pub min_touch_target: f32,
    #[serde(default = "default_min_font_size")]
    pub min_font_size: f32,
    #[serde(default)]
    pub contrast_level: ContrastLevel,
}

fn default_tolerance() -> f32 {
    0.5
}
fn default_min_overlap_area() -> f32 {
    1.0
}
fn default_max_nesting_depth() -> usize {
    6
}
fn default_grid_step() -> f32 {
    4.0
}
fn default_min_touch_target() -> f32 {
    44.0
}
fn default_min_font_size() -> f32 {
    12.0
}

impl Default for LintThresholds {
    fn default() -> Self {
        Self {
            geometry_tolerance: default_tolerance(),
            min_overlap_area: default_min_overlap_area(),
            max_nesting_depth: default_max_nesting_depth(),
            grid_step: default_grid_step(),
            min_touch_target: default_min_touch_target(),
            min_font_size: default_min_font_size(),
            contrast_level: ContrastLevel::AA,
        }
    }
}

/// Active rule selection and options for a lint run.
#[derive(Clone, Debug, PartialEq)]
pub struct LintConfig {
    /// Active canonical rule IDs mapped to their effective severity.
    pub enabled_rules: BTreeMap<&'static str, Severity>,
    /// Include findings that match `doc.check_designs.ignored` (marked with `ignored: true`).
    pub include_ignored: bool,
    /// Include unoverridden layers inside component instances.
    pub include_instances: bool,
    /// Numeric thresholds for rules.
    pub thresholds: LintThresholds,
    /// Maximum number of findings to return in a single page (`None` = unlimited).
    pub limit: Option<usize>,
    /// Continuation cursor (`"offset:<n>"` or `"stack:<ids>"`).
    pub cursor: Option<String>,
}

impl Default for LintConfig {
    fn default() -> Self {
        Self::from_preset(LintPreset::Agent)
    }
}

impl LintConfig {
    /// Builds a configuration with the rules of a built-in [`LintPreset`].
    pub fn from_preset(preset: LintPreset) -> Self {
        let mut enabled_rules = BTreeMap::new();
        for rule in RULES {
            if rule.presets.contains(&preset) {
                enabled_rules.insert(rule.id, rule.default_severity);
            }
        }
        Self { enabled_rules, include_ignored: false, include_instances: false, thresholds: LintThresholds::default(), limit: None, cursor: None }
    }

    /// Parses a preset string:
    /// - `"agent"`, `"check-designs"`, `"recommended"`, `"strict"`
    /// - Category names: `"layout"`, `"text"`, `"tokens"`, `"components"`, `"content"`, `"naming"`, `"accessibility"` (`"a11y"`), `"structure"`
    /// - Comma-separated list of rule IDs or aliases (when containing `,` or matching a known rule ID/alias)
    pub fn from_preset_str(preset: &str) -> Result<Self, LintConfigError> {
        let trimmed = preset.trim();
        if trimmed.is_empty() {
            return Ok(Self::from_preset(LintPreset::Agent));
        }
        if let Some(p) = LintPreset::from_name(trimmed) {
            return Ok(Self::from_preset(p));
        }
        if let Some(cat) = Category::from_name(trimmed) {
            let mut enabled_rules = BTreeMap::new();
            for rule in RULES {
                if rule.category == cat {
                    enabled_rules.insert(rule.id, rule.default_severity);
                }
            }
            return Ok(Self {
                enabled_rules,
                include_ignored: false,
                include_instances: false,
                thresholds: LintThresholds::default(),
                limit: None,
                cursor: None,
            });
        }
        if trimmed.contains(',') {
            let parts: Vec<String> = trimmed.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
            return Self::from_rule_list(&parts);
        }
        if let Some(spec) = resolve_rule(trimmed) {
            let mut enabled_rules = BTreeMap::new();
            enabled_rules.insert(spec.id, spec.default_severity);
            return Ok(Self {
                enabled_rules,
                include_ignored: false,
                include_instances: false,
                thresholds: LintThresholds::default(),
                limit: None,
                cursor: None,
            });
        }
        Err(LintConfigError::UnknownPreset { received: trimmed.to_string(), valid_options: all_preset_names() })
    }

    /// Builds a configuration enabling only the given rule IDs / aliases.
    pub fn from_rule_list(rules: &[String]) -> Result<Self, LintConfigError> {
        let mut enabled_rules = BTreeMap::new();
        for name in rules {
            let Some(spec) = resolve_rule(name) else {
                return Err(LintConfigError::UnknownRule { received: name.clone(), valid_options: all_rule_ids() });
            };
            enabled_rules.insert(spec.id, spec.default_severity);
        }
        Ok(Self { enabled_rules, include_ignored: false, include_instances: false, thresholds: LintThresholds::default(), limit: None, cursor: None })
    }

    /// Replaces the enabled rules with the given explicit rule list (`rules` array in MCP).
    pub fn with_rules(mut self, rules: &[String]) -> Result<Self, LintConfigError> {
        let mut enabled_rules = BTreeMap::new();
        for name in rules {
            let Some(spec) = resolve_rule(name) else {
                return Err(LintConfigError::UnknownRule { received: name.clone(), valid_options: all_rule_ids() });
            };
            enabled_rules.insert(spec.id, spec.default_severity);
        }
        self.enabled_rules = enabled_rules;
        Ok(self)
    }

    /// Applies per-rule severity overrides on top of the current preset.
    pub fn with_rule_overrides(mut self, overrides: &BTreeMap<String, RuleSeverityConfig>) -> Result<Self, LintConfigError> {
        for (name, cfg) in overrides {
            let Some(spec) = resolve_rule(name) else {
                return Err(LintConfigError::UnknownRule { received: name.clone(), valid_options: all_rule_ids() });
            };
            match cfg {
                RuleSeverityConfig::Enabled(false) | RuleSeverityConfig::Level(RuleSeverityLevel::Off) => {
                    self.enabled_rules.remove(spec.id);
                }
                RuleSeverityConfig::Enabled(true) => {
                    self.enabled_rules.insert(spec.id, spec.default_severity);
                }
                RuleSeverityConfig::Level(RuleSeverityLevel::Error) => {
                    self.enabled_rules.insert(spec.id, Severity::Error);
                }
                RuleSeverityConfig::Level(RuleSeverityLevel::Warning) => {
                    self.enabled_rules.insert(spec.id, Severity::Warning);
                }
                RuleSeverityConfig::Level(RuleSeverityLevel::Info) => {
                    self.enabled_rules.insert(spec.id, Severity::Info);
                }
            }
        }
        Ok(self)
    }

    /// Removes any rules disabled in `disabled_rules` (resolving aliases to canonical IDs).
    pub fn without_disabled(mut self, disabled_rules: &BTreeSet<String>) -> Self {
        for disabled in disabled_rules {
            if let Some(spec) = resolve_rule(disabled) {
                self.enabled_rules.remove(spec.id);
            }
        }
        self
    }

    /// Returns the effective severity of `rule_id` (or alias) if enabled.
    pub fn rule_severity(&self, rule_id: &str) -> Option<Severity> {
        let spec = resolve_rule(rule_id)?;
        self.enabled_rules.get(spec.id).copied()
    }

    /// Whether `rule_id` (or alias) is enabled.
    pub fn is_enabled(&self, rule_id: &str) -> bool {
        self.rule_severity(rule_id).is_some()
    }
}
