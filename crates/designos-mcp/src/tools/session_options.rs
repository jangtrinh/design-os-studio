//! Per-session MCP options (`designos_configure_session`, E15-12).
//!
//! Controls the automatic per-call lint `issues` block (`issues.mode`, `issues.preset`,
//! `issues.max_items`) and reserves `auto_fix` configuration for E15-13. Persists to
//! `<session_dir>/session-options.json` when a session directory is configured and resets
//! on document switch (`designos_open_file`, `designos_new_document`).

use designos_doc::lint::LintConfig;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Controls whether and how mutating MCP calls append a lint `issues` block (`E15-12` §3.5).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IssuesMode {
    /// Run the configured preset (default `"agent"`) and return all severity levels.
    #[default]
    Preset,
    /// Run the configured preset and keep only `error`-severity findings.
    ErrorsOnly,
    /// Skip the per-call lint pass and omit all `issues*` keys from the reply.
    Off,
}

fn default_preset() -> String {
    "agent".to_string()
}

fn default_max_items() -> usize {
    20
}

/// Per-session configuration for the mutating-call `issues` block (`E15-12` §3.5, §4).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IssuesOptions {
    #[serde(default)]
    pub mode: IssuesMode,
    #[serde(default = "default_preset")]
    pub preset: String,
    #[serde(default = "default_max_items")]
    pub max_items: usize,
}

impl Default for IssuesOptions {
    fn default() -> Self {
        Self { mode: IssuesMode::Preset, preset: default_preset(), max_items: default_max_items() }
    }
}

/// Reserved per-session configuration for automatic safe mechanical fixes (`E15-13`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AutoFixOptions {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub rules: BTreeMap<String, bool>,
}

/// Full per-session options (`E15-12` §4).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionOptions {
    #[serde(default)]
    pub issues: IssuesOptions,
    #[serde(default)]
    pub auto_fix: AutoFixOptions,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct IssuesOptionsPatch {
    mode: Option<IssuesMode>,
    preset: Option<String>,
    max_items: Option<i64>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct AutoFixOptionsPatch {
    mode: Option<String>,
    rules: Option<BTreeMap<String, bool>>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct ConfigureSessionArgs {
    issues: Option<IssuesOptionsPatch>,
    auto_fix: Option<AutoFixOptionsPatch>,
}

/// Optional per-call `lint` override accepted on mutating tools (`true`, `false`, `"preset"`, `"errors_only"`, `"off"`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(untagged)]
pub enum PerCallLintOverride {
    Bool(bool),
    Mode(IssuesMode),
}

impl PerCallLintOverride {
    /// Resolves the effective [`IssuesMode`] for a single call given the session's configured mode.
    pub fn resolve(override_opt: Option<Self>, session_mode: IssuesMode) -> IssuesMode {
        match override_opt {
            Some(PerCallLintOverride::Bool(false)) | Some(PerCallLintOverride::Mode(IssuesMode::Off)) => IssuesMode::Off,
            Some(PerCallLintOverride::Bool(true)) => match session_mode {
                IssuesMode::Off => IssuesMode::Preset,
                other => other,
            },
            Some(PerCallLintOverride::Mode(m)) => m,
            None => session_mode,
        }
    }
}

impl SessionOptions {
    /// Path to `session-options.json` inside `session_dir`.
    pub fn file_path(session_dir: &Path) -> PathBuf {
        session_dir.join("session-options.json")
    }

    /// Loads `SessionOptions` from `<session_dir>/session-options.json` if present and valid,
    /// falling back to `SessionOptions::default()`.
    pub fn load_from_dir(session_dir: &Path) -> Self {
        let path = Self::file_path(session_dir);
        let Ok(raw) = fs::read_to_string(&path) else {
            return Self::default();
        };
        let Ok(opts) = serde_json::from_str::<Self>(&raw) else {
            return Self::default();
        };
        if !(1..=100).contains(&opts.issues.max_items) || LintConfig::from_preset_str(&opts.issues.preset).is_err() {
            return Self::default();
        }
        opts
    }

    /// Atomically persists `self` to `<session_dir>/session-options.json`.
    pub fn save_to_dir(&self, session_dir: &Path) -> Result<(), String> {
        let path = Self::file_path(session_dir);
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let bytes = serde_json::to_vec_pretty(self).map_err(|e| format!("serialize session-options.json: {e}"))?;
        let tmp_path = path.with_extension(format!("tmp-{}", std::process::id()));
        fs::write(&tmp_path, &bytes).map_err(|e| format!("write {}: {e}", tmp_path.display()))?;
        fs::rename(&tmp_path, &path).map_err(|e| format!("rename {} -> {}: {e}", tmp_path.display(), path.display()))
    }

    /// Validates and applies a `designos_configure_session` payload, returning the full effective options JSON.
    pub fn apply_patch(&mut self, args: Value) -> Result<Value, String> {
        let patch: ConfigureSessionArgs =
            serde_json::from_value(args).map_err(|e| format!("INVALID_ARGUMENTS: invalid arguments: {e} (safe_to_retry: true)"))?;

        let mut next = self.clone();
        if let Some(issues_patch) = patch.issues {
            if let Some(max_items) = issues_patch.max_items {
                if !(1..=100).contains(&max_items) {
                    return Err(format!("VALUE_OUT_OF_RANGE: issues.max_items must be between 1 and 100, got {max_items} (safe_to_retry: true)"));
                }
                next.issues.max_items = max_items as usize;
            }
            if let Some(preset) = issues_patch.preset {
                let trimmed = preset.trim();
                if trimmed.is_empty() {
                    return Err("UNKNOWN_PRESET: issues.preset must not be empty (safe_to_retry: true)".to_string());
                }
                LintConfig::from_preset_str(trimmed).map_err(|e| e.to_string())?;
                next.issues.preset = trimmed.to_string();
            }
            if let Some(mode) = issues_patch.mode {
                next.issues.mode = mode;
            }
        }

        if let Some(af_patch) = patch.auto_fix {
            if let Some(mode) = af_patch.mode {
                next.auto_fix.mode = Some(mode);
            }
            if let Some(rules) = af_patch.rules {
                for (k, v) in rules {
                    let Some(spec) = designos_doc::lint::resolve_rule(&k) else {
                        let err = designos_doc::lint::LintConfigError::UnknownRule { received: k, valid_options: designos_doc::lint::all_rule_ids() };
                        return Err(err.to_string());
                    };
                    next.auto_fix.rules.insert(spec.id.to_string(), v);
                }
            }
        }

        *self = next;
        serde_json::to_value(&*self).map_err(|e| e.to_string())
    }
}

/// JSON Schema descriptor for `designos_configure_session` (`E15-12` §5).
pub fn tool_descriptor() -> Value {
    json!({
        "name": "designos_configure_session",
        "description": "Configure per-session MCP options for the automatic mutating-call lint issues block (issues.mode: preset | errors_only | off, issues.preset, issues.max_items: 1..=100) and auto_fix settings. Persists across reconnects in the session directory and resets on document switch.",
        "inputSchema": {
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "issues": {
                    "type": "object",
                    "additionalProperties": false,
                    "properties": {
                        "mode": {
                            "type": "string",
                            "enum": ["preset", "errors_only", "off"]
                        },
                        "preset": {
                            "type": "string"
                        },
                        "max_items": {
                            "type": "integer",
                            "minimum": 1,
                            "maximum": 100
                        }
                    }
                },
                "auto_fix": {
                    "type": "object"
                }
            }
        },
        "annotations": {
            "readOnlyHint": false,
            "destructiveHint": false,
            "idempotentHint": true,
            "openWorldHint": false
        }
    })
}
