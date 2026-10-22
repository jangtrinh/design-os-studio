//! Isolated per-run sandbox workspace creation and live-session environment variable scrubbing (E15-25 §3.3).

use crate::task::Task;
use serde_json::json;
use std::path::{Path, PathBuf};

/// Environment variables removed from child agent and MCP processes so an evaluation run can never
/// write to an open Studio editor's live session files (E15-25 §3.3).
pub const SCRUBBED_STUDIO_ENV_VARS: &[&str] = &[
    "DESIGNOS_SESSION_DIR",
    "DESIGNOS_LIVE_CANVAS",
    "DESIGNOS_LIVE_DOC_SYNC",
    "DESIGNOS_LIVE_DOC_PATCH",
    "DESIGNOS_LIVE_SPEC",
    "DESIGNOS_CONTEXT_JSON",
    "DESIGNOS_TARGET_PAGE_ID",
];

/// Isolated workspace directory and paths for a single task run.
#[derive(Clone, Debug)]
pub struct RunSandbox {
    pub workspace_dir: PathBuf,
    pub output_dir: PathBuf,
    pub mcp_config_path: PathBuf,
    pub prompt_file_path: PathBuf,
    pub transcript_path: PathBuf,
    pub output_doc_path: PathBuf,
}

impl RunSandbox {
    /// Creates a fresh sandbox directory under `out_root/<task_id>/<run_idx>`, copies the task's
    /// start document and input files into `workspace_dir`, and writes `mcp-config.json` and
    /// `prompt.md`.
    pub fn prepare(task: &Task, run_idx: u32, suite_dir: &Path, out_root: &Path, mcp_bin: &Path, full_prompt: &str) -> Result<Self, String> {
        let output_dir = out_root.join(&task.id).join(run_idx.to_string());
        let workspace_dir = output_dir.join("workspace");
        std::fs::create_dir_all(&workspace_dir).map_err(|e| format!("sandbox_io_error: cannot create '{}': {e}", workspace_dir.display()))?;

        prepare_workspace(task, suite_dir, &workspace_dir)?;

        let transcript_path = output_dir.join("transcript.jsonl");
        let output_doc_path = output_dir.join("output.designos");
        let prompt_file_path = output_dir.join("prompt.md");
        std::fs::write(&prompt_file_path, full_prompt).map_err(|e| format!("sandbox_io_error: cannot write prompt '{}': {e}", prompt_file_path.display()))?;

        let mcp_config_path = output_dir.join("mcp-config.json");
        let mcp_cfg = json!({
            "mcpServers": {
                "designos": {
                    "command": mcp_bin.to_string_lossy(),
                    "args": ["--workspace", workspace_dir.to_string_lossy()],
                    "env": {
                        "DESIGNOS_MCP_TRANSCRIPT": transcript_path.to_string_lossy()
                    }
                }
            }
        });
        std::fs::write(&mcp_config_path, serde_json::to_string_pretty(&mcp_cfg).unwrap())
            .map_err(|e| format!("sandbox_io_error: cannot write mcp-config '{}': {e}", mcp_config_path.display()))?;

        Ok(Self { workspace_dir, output_dir, mcp_config_path, prompt_file_path, transcript_path, output_doc_path })
    }

    /// Removes `DESIGNOS_SESSION_DIR`, `DESIGNOS_LIVE_*`, and `DESIGNOS_CONTEXT_JSON` from a
    /// `std::process::Command` and sets `DESIGNOS_MCP_TRANSCRIPT`.
    pub fn configure_child_env(&self, cmd: &mut std::process::Command) {
        for var in SCRUBBED_STUDIO_ENV_VARS {
            cmd.env_remove(var);
        }
        cmd.env("DESIGNOS_MCP_TRANSCRIPT", &self.transcript_path);
    }
}

/// Fills `workspace_dir` for a task: the start document as `start.designos` (unless `blank` or `gen:`) and the
/// declared input files under their relative paths. Runs and replays prepare their workspace the same way.
pub fn prepare_workspace(task: &Task, suite_dir: &Path, workspace_dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(workspace_dir).map_err(|e| format!("sandbox_io_error: cannot create '{}': {e}", workspace_dir.display()))?;
    // Copy start document if specified and not "blank" / "gen:..."
    if task.start != "blank" && !task.start.is_empty() && !task.start.starts_with("gen:") {
        let src = suite_dir.join(&task.start);
        let dst = workspace_dir.join("start.designos");
        std::fs::copy(&src, &dst).map_err(|e| format!("sandbox_io_error: cannot copy start doc '{}' -> '{}': {e}", src.display(), dst.display()))?;
    }

    // Copy declared task inputs preserving relative paths inside workspace_dir
    for rel in &task.inputs {
        let src = suite_dir.join(rel);
        let dst = workspace_dir.join(rel);
        if let Some(parent) = dst.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("sandbox_io_error: cannot create '{}': {e}", parent.display()))?;
        }
        std::fs::copy(&src, &dst).map_err(|e| format!("sandbox_io_error: cannot copy input '{}' -> '{}': {e}", src.display(), dst.display()))?;
    }
    Ok(())
}
