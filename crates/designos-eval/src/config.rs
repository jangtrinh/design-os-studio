//! User agent CLI profile configuration (`designos-eval.toml`) and placeholder validation (E15-25 §3.5, §3.6).

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Supported token usage output formats from agent CLIs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum UsageFormat {
    #[serde(rename = "claude-stream-json")]
    ClaudeStreamJson,
    #[serde(rename = "codex-json")]
    CodexJson,
    #[default]
    #[serde(rename = "none")]
    None,
}

/// Allowed command template placeholders per E15-25 §3.5.
pub const ALLOWED_PLACEHOLDERS: &[&str] = &["prompt", "prompt_file", "mcp_config", "mcp_bin", "workspace", "model", "output_dir"];

/// Configured agent CLI profile (`[agent.<name>]` in `designos-eval.toml`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentProfile {
    pub command: Vec<String>,
    pub model: String,
    #[serde(default)]
    pub usage: UsageFormat,
}

/// Root configuration loaded from `designos-eval.toml`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvalConfig {
    #[serde(default)]
    pub agent: BTreeMap<String, AgentProfile>,
}

/// Values substituted into an [`AgentProfile::command`] template for a single run.
#[derive(Clone, Debug)]
pub struct CommandVars<'a> {
    pub prompt: &'a str,
    pub prompt_file: &'a Path,
    pub mcp_config: &'a Path,
    pub mcp_bin: &'a Path,
    pub workspace: &'a Path,
    pub model: &'a str,
    pub output_dir: &'a Path,
}

impl AgentProfile {
    /// Validates that `command` is non-empty and every `{...}` placeholder is in [`ALLOWED_PLACEHOLDERS`].
    pub fn validate(&self, profile_name: &str) -> Result<(), String> {
        if self.command.is_empty() {
            return Err(format!("invalid_config: agent profile '{profile_name}' has an empty command array"));
        }
        for arg in &self.command {
            let mut rest = arg.as_str();
            while let Some(open) = rest.find('{') {
                let after_open = &rest[open + 1..];
                let Some(close) = after_open.find('}') else {
                    return Err(format!("invalid_config: unclosed placeholder in agent profile '{profile_name}' arg '{arg}'"));
                };
                let token = &after_open[..close];
                if !ALLOWED_PLACEHOLDERS.contains(&token) {
                    return Err(format!(
                        "invalid_config: unknown placeholder '{{{token}}}' in agent profile '{profile_name}'; allowed: {:?}",
                        ALLOWED_PLACEHOLDERS
                    ));
                }
                rest = &after_open[close + 1..];
            }
        }
        Ok(())
    }

    /// Expands all `{placeholder}` tokens in `self.command` using `vars`.
    pub fn expand_command(&self, vars: &CommandVars<'_>) -> Vec<String> {
        self.command
            .iter()
            .map(|arg| {
                arg.replace("{prompt}", vars.prompt)
                    .replace("{prompt_file}", &vars.prompt_file.to_string_lossy())
                    .replace("{mcp_config}", &vars.mcp_config.to_string_lossy())
                    .replace("{mcp_bin}", &vars.mcp_bin.to_string_lossy())
                    .replace("{workspace}", &vars.workspace.to_string_lossy())
                    .replace("{model}", vars.model)
                    .replace("{output_dir}", &vars.output_dir.to_string_lossy())
            })
            .collect()
    }
}

impl EvalConfig {
    /// Loads and validates `designos-eval.toml` from `path`.
    pub fn load_from_path(path: &Path) -> Result<Self, String> {
        let raw = std::fs::read_to_string(path).map_err(|e| format!("config_not_found: cannot read config '{}': {e}", path.display()))?;
        Self::from_toml_str(&raw)
    }

    /// Parses and validates `designos-eval.toml` content.
    pub fn from_toml_str(raw: &str) -> Result<Self, String> {
        let cfg: Self = toml::from_str(raw).map_err(|e| format!("invalid_config: invalid TOML: {e}"))?;
        for (name, profile) in &cfg.agent {
            profile.validate(name)?;
        }
        Ok(cfg)
    }

    /// Resolves the default platform config path (`<config_dir>/designos/designos-eval.toml`).
    pub fn default_config_path() -> Option<PathBuf> {
        dirs::config_dir().map(|d| d.join("designos").join("designos-eval.toml"))
    }
}
