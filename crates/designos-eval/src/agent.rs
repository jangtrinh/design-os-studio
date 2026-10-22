//! Agent CLI runner, token usage parsers, and the scripted `stub-agent` (E15-25 §3.5, §3.6, §3.7, §3.10).

use crate::config::{AgentProfile, CommandVars, UsageFormat};
use crate::jsonl::RunExit;
use crate::mcp_client::McpClient;
use crate::sandbox::{RunSandbox, SCRUBBED_STUDIO_ENV_VARS};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// How long a timed-out agent has between SIGTERM and SIGKILL to flush `output.designos` (E15-25 §3.7).
pub const TERMINATE_GRACE: Duration = Duration::from_secs(5);
/// Agent stdout is only parsed for token usage; anything beyond this is drained and dropped.
const STDOUT_CAP_BYTES: usize = 64 * 1024 * 1024;
/// Only the tail of stderr is kept for the run note.
const STDERR_CAP_BYTES: usize = 64 * 1024;

/// Result of invoking an agent CLI process for one run.
#[derive(Clone, Debug)]
pub struct AgentRunOutcome {
    pub exit: RunExit,
    pub wall_ms: u64,
    pub tokens_in: Option<u64>,
    pub tokens_out: Option<u64>,
    pub stderr_note: Option<String>,
}

/// Checks whether a binary name or path exists and is executable on `PATH`.
pub fn binary_exists_on_path(bin: &str) -> bool {
    resolve_binary_path(bin).is_some()
}

/// Resolves a binary name to its full path: an explicit path, a sibling of the running executable
/// (so `designos-eval` finds a `designos-mcp` built beside it), or a `PATH` entry.
pub fn resolve_binary_path(bin: &str) -> Option<PathBuf> {
    let p = Path::new(bin);
    if p.components().count() > 1 {
        return p.is_file().then(|| p.to_path_buf());
    }
    if let Ok(exe) = std::env::current_exe() {
        if exe.file_stem().and_then(|s| s.to_str()) == Some(bin) {
            return Some(exe);
        }
        if let Some(dir) = exe.parent() {
            let sibling = dir.join(bin);
            if sibling.is_file() {
                return Some(sibling);
            }
            #[cfg(windows)]
            if dir.join(format!("{bin}.exe")).is_file() {
                return Some(dir.join(format!("{bin}.exe")));
            }
            if let Some(parent) = dir.parent() {
                let parent_sibling = parent.join(bin);
                if parent_sibling.is_file() {
                    return Some(parent_sibling);
                }
            }
        }
    }
    if let Some(path_os) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path_os) {
            let cand = dir.join(bin);
            if cand.is_file() {
                return Some(cand);
            }
            #[cfg(windows)]
            if dir.join(format!("{bin}.exe")).is_file() {
                return Some(dir.join(format!("{bin}.exe")));
            }
        }
    }
    None
}

/// Runs the configured agent CLI command in `sandbox` with `timeout_s`, returning [`AgentRunOutcome`].
pub fn execute_agent_profile(
    profile: &AgentProfile,
    vars: &CommandVars<'_>,
    sandbox: &RunSandbox,
    timeout_s: u64,
    stub_script_override: Option<&Path>,
) -> Result<AgentRunOutcome, String> {
    let expanded = profile.expand_command(vars);
    let Some(bin_name) = expanded.first() else {
        return Err("invalid_config: empty command after template expansion".into());
    };

    // `designos-eval stub-agent --script ...` runs in-process; it still drives a real `designos-mcp`.
    if bin_name == "stub-agent" || (expanded.get(1).map(String::as_str) == Some("stub-agent")) {
        let t0 = Instant::now();
        let script_path = if let Some(s) = stub_script_override {
            s.to_path_buf()
        } else {
            let idx = expanded.iter().position(|a| a == "--script").ok_or_else(|| "stub_agent_error: missing --script <file.jsonl>".to_string())?;
            PathBuf::from(expanded.get(idx + 1).ok_or_else(|| "stub_agent_error: missing path after --script".to_string())?)
        };
        let res = run_stub_agent(&script_path, vars.mcp_config);
        let wall_ms = t0.elapsed().as_millis() as u64;
        return match res {
            Ok(()) => Ok(AgentRunOutcome { exit: RunExit::Ok, wall_ms, tokens_in: None, tokens_out: None, stderr_note: None }),
            Err(e) => Ok(AgentRunOutcome { exit: RunExit::Crashed, wall_ms, tokens_in: None, tokens_out: None, stderr_note: Some(e) }),
        };
    }

    let Some(resolved_bin) = resolve_binary_path(bin_name) else {
        return Err(format!("agent_not_found: binary '{bin_name}' is not on PATH; install it or update designos-eval.toml"));
    };

    let t0 = Instant::now();
    let mut cmd = Command::new(&resolved_bin);
    cmd.args(&expanded[1..]).current_dir(&sandbox.workspace_dir).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    sandbox.configure_child_env(&mut cmd);

    let mut child = cmd.spawn().map_err(|e| format!("agent_not_found: failed to spawn '{}': {e}", resolved_bin.display()))?;
    // Both pipes are drained while the agent runs: a chatty CLI would otherwise block on a full pipe
    // and be recorded as a timeout although its work was finished.
    let stdout = Drain::start(child.stdout.take(), STDOUT_CAP_BYTES, false);
    let stderr = Drain::start(child.stderr.take(), STDERR_CAP_BYTES, true);

    let deadline = Duration::from_secs(timeout_s.max(1));
    let (status, timed_out) = loop {
        match child.try_wait() {
            Ok(Some(status)) => break (Ok(status), false),
            Ok(None) if t0.elapsed() >= deadline => {
                let status = terminate_gracefully(&mut child, TERMINATE_GRACE);
                break (status, true);
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(10)),
            Err(e) => break (Err(e), false),
        }
    };
    let wall_ms = t0.elapsed().as_millis() as u64;
    let stdout_str = String::from_utf8_lossy(&stdout.finish()).to_string();
    let stderr_str = String::from_utf8_lossy(&stderr.finish()).trim().to_string();
    let stderr_note = (!stderr_str.is_empty()).then_some(stderr_str);

    if timed_out {
        let (tokens_in, tokens_out) = parse_token_usage(profile.usage, &stdout_str);
        return Ok(AgentRunOutcome {
            exit: RunExit::Timeout,
            wall_ms,
            tokens_in,
            tokens_out,
            stderr_note: Some(format!("Timed out after {timeout_s}s{}", stderr_note.map(|s| format!("; stderr: {s}")).unwrap_or_default())),
        });
    }
    match status {
        Ok(status) => {
            let (tokens_in, tokens_out) = parse_token_usage(profile.usage, &stdout_str);
            let exit = if status.success() { RunExit::Ok } else { RunExit::Crashed };
            Ok(AgentRunOutcome { exit, wall_ms, tokens_in, tokens_out, stderr_note })
        }
        Err(e) => {
            Ok(AgentRunOutcome { exit: RunExit::Crashed, wall_ms, tokens_in: None, tokens_out: None, stderr_note: Some(format!("Process wait failed: {e}")) })
        }
    }
}

/// Background reader that keeps a pipe empty and remembers a bounded part of it.
struct Drain {
    handle: Option<JoinHandle<()>>,
    buffer: Arc<Mutex<Vec<u8>>>,
}

impl Drain {
    /// Starts reading `pipe` (when present). With `keep_tail` the newest `cap` bytes are kept, otherwise the first `cap`.
    fn start<R: Read + Send + 'static>(pipe: Option<R>, cap: usize, keep_tail: bool) -> Self {
        let buffer = Arc::new(Mutex::new(Vec::new()));
        let handle = pipe.map(|mut pipe| {
            let sink = Arc::clone(&buffer);
            std::thread::spawn(move || {
                let mut chunk = [0u8; 16 * 1024];
                loop {
                    let n = match pipe.read(&mut chunk) {
                        Ok(0) | Err(_) => return,
                        Ok(n) => n,
                    };
                    let Ok(mut buf) = sink.lock() else { return };
                    if keep_tail {
                        buf.extend_from_slice(&chunk[..n]);
                        if buf.len() > cap {
                            let excess = buf.len() - cap;
                            buf.drain(..excess);
                        }
                    } else if buf.len() < cap {
                        let room = cap - buf.len();
                        buf.extend_from_slice(&chunk[..n.min(room)]);
                    }
                }
            })
        });
        Self { handle, buffer }
    }

    /// Returns what was read. A descendant of the agent that still holds the pipe open must not hang the
    /// harness, so the reader gets a short moment to see EOF and is then abandoned.
    fn finish(self) -> Vec<u8> {
        if let Some(handle) = self.handle {
            let until = Instant::now() + Duration::from_secs(2);
            while !handle.is_finished() && Instant::now() < until {
                std::thread::sleep(Duration::from_millis(5));
            }
            if handle.is_finished() {
                let _ = handle.join();
            }
        }
        self.buffer.lock().map(|b| b.clone()).unwrap_or_default()
    }
}

/// Asks `child` to stop with SIGTERM, then kills it when it is still alive after `grace` (E15-25 §3.7).
/// Windows has no SIGTERM, so the child is killed at once there.
pub fn terminate_gracefully(child: &mut Child, grace: Duration) -> std::io::Result<std::process::ExitStatus> {
    #[cfg(unix)]
    {
        let signalled = Command::new("kill").arg("-TERM").arg(child.id().to_string()).stdout(Stdio::null()).stderr(Stdio::null()).status();
        if signalled.is_ok_and(|s| s.success()) {
            let until = Instant::now() + grace;
            while Instant::now() < until {
                if let Some(status) = child.try_wait()? {
                    return Ok(status);
                }
                std::thread::sleep(Duration::from_millis(10));
            }
        }
    }
    #[cfg(not(unix))]
    let _ = grace;
    child.kill()?;
    child.wait()
}

/// Extracts `(tokens_in, tokens_out)` from CLI stdout according to `format`.
pub fn parse_token_usage(format: UsageFormat, stdout: &str) -> (Option<u64>, Option<u64>) {
    match format {
        UsageFormat::None => (None, None),
        UsageFormat::ClaudeStreamJson | UsageFormat::CodexJson => {
            let mut total_in = 0u64;
            let mut total_out = 0u64;
            let mut found = false;
            for line in stdout.lines() {
                let Ok(val) = serde_json::from_str::<Value>(line.trim()) else {
                    continue;
                };
                if let Some(usage) = val.get("usage").or_else(|| val.pointer("/result/usage")) {
                    if let Some(inp) = usage.get("input_tokens").or_else(|| usage.get("prompt_tokens")).and_then(Value::as_u64) {
                        total_in += inp;
                        found = true;
                    }
                    if let Some(outp) = usage.get("output_tokens").or_else(|| usage.get("completion_tokens")).and_then(Value::as_u64) {
                        total_out += outp;
                        found = true;
                    }
                }
            }
            if found {
                (Some(total_in), Some(total_out))
            } else {
                (None, None)
            }
        }
    }
}

/// One scripted tool call line in a `stub-agent` JSONL script (`{ "tool": "...", "args": { ... } }`).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StubCall {
    pub tool: String,
    #[serde(default)]
    pub args: Value,
}

/// Starts the `designos-mcp` server described by the `mcpServers.designos` entry of `mcp_config_path` and
/// replays every `{tool, args}` line from `script_path` against it, exactly like a model-driven agent would.
/// The server itself writes the transcript (`DESIGNOS_MCP_TRANSCRIPT` in the config's `env`). A failing tool
/// is a recorded result, not a harness failure; a transport failure or a failed transcript append is.
pub fn run_stub_agent(script_path: &Path, mcp_config_path: &Path) -> Result<(), String> {
    let raw_cfg =
        std::fs::read_to_string(mcp_config_path).map_err(|e| format!("stub_agent_error: cannot read mcp config '{}': {e}", mcp_config_path.display()))?;
    let cfg_val: Value = serde_json::from_str(&raw_cfg).map_err(|e| format!("stub_agent_error: invalid mcp config JSON: {e}"))?;
    let server = cfg_val.pointer("/mcpServers/designos").ok_or("stub_agent_error: mcp config has no mcpServers.designos entry")?;
    let command = server.get("command").and_then(Value::as_str).ok_or("stub_agent_error: mcpServers.designos has no command")?;
    let args: Vec<String> =
        server.get("args").and_then(Value::as_array).map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_string)).collect()).unwrap_or_default();
    let env: BTreeMap<String, String> = server
        .get("env")
        .and_then(Value::as_object)
        .map(|o| o.iter().filter_map(|(k, v)| v.as_str().map(|v| (k.clone(), v.to_string()))).collect())
        .unwrap_or_default();

    let raw_script = std::fs::read_to_string(script_path).map_err(|e| format!("stub_agent_error: cannot read script '{}': {e}", script_path.display()))?;
    let mut calls = Vec::new();
    for (idx, line) in raw_script.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let call: StubCall =
            serde_json::from_str(trimmed).map_err(|e| format!("stub_agent_error: invalid JSON on line {} of '{}': {e}", idx + 1, script_path.display()))?;
        calls.push(call);
    }

    let mut client = McpClient::spawn(Path::new(command), &args, &env, SCRUBBED_STUDIO_ENV_VARS)?;
    for call in &calls {
        let out = client.call_tool(&call.tool, &call.args)?;
        if let Some(message) = out.error_message().filter(|m| m.starts_with("Transcript logging failed")) {
            return Err(message.to_string());
        }
    }
    client.shutdown();
    Ok(())
}
