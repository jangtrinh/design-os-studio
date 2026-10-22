//! Minimal stdio MCP client for a real `designos-mcp` process (E15-25 §3.3, §3.7, §3.10, §3.11).
//!
//! `designos-eval` must not depend on the `designos-mcp` crate (`designos-mcp` depends on `designos-eval`
//! for the judge library), so the harness speaks the wire protocol itself: newline-delimited JSON-RPC 2.0
//! over the child's stdin and stdout. The server writes the transcript (`DESIGNOS_MCP_TRANSCRIPT`).

use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

/// MCP protocol revision announced in `initialize`; the server answers with the revision it speaks.
const PROTOCOL_VERSION: &str = "2025-03-26";
/// A single tool call may build a large document, so the wait per request is generous but bounded.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(600);
/// After stdin closes the server gets this long to exit before it is killed.
const SHUTDOWN_GRACE: Duration = Duration::from_secs(5);

/// Result of one `tools/call`.
#[derive(Clone, Debug, PartialEq)]
pub struct ToolOutput {
    /// `structuredContent` when the server sent it, otherwise the text content parsed as JSON (or as a string).
    pub value: Value,
    /// The server's `isError` flag: a tool-level failure, not a transport failure.
    pub is_error: bool,
}

impl ToolOutput {
    /// The failure text of an `is_error` result: the server sends `{"error": "<message>"}`, older servers plain text.
    pub fn error_message(&self) -> Option<&str> {
        if !self.is_error {
            return None;
        }
        self.value.get("error").and_then(Value::as_str).or_else(|| self.value.as_str())
    }
}

/// One `designos-mcp` child process and the stdio JSON-RPC channel to it.
pub struct McpClient {
    child: Child,
    stdin: Option<ChildStdin>,
    incoming: Receiver<Value>,
    next_id: u64,
}

impl McpClient {
    /// Starts `command args…` with `env` added and `env_remove` scrubbed, then completes the MCP handshake.
    pub fn spawn(command: &Path, args: &[String], env: &BTreeMap<String, String>, env_remove: &[&str]) -> Result<Self, String> {
        let mut cmd = Command::new(command);
        cmd.args(args).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::inherit());
        for var in env_remove {
            cmd.env_remove(var);
        }
        cmd.envs(env);
        let mut child = cmd.spawn().map_err(|e| format!("mcp_not_found: cannot start '{}': {e}", command.display()))?;
        let stdin = child.stdin.take().ok_or("mcp_io_error: child stdin is not piped")?;
        let stdout = child.stdout.take().ok_or("mcp_io_error: child stdout is not piped")?;
        let (tx, incoming) = mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else { break };
                if let Ok(msg) = serde_json::from_str::<Value>(line.trim()) {
                    if tx.send(msg).is_err() {
                        break;
                    }
                }
            }
        });
        let mut client = Self { child, stdin: Some(stdin), incoming, next_id: 0 };
        client.request(
            "initialize",
            json!({ "protocolVersion": PROTOCOL_VERSION, "capabilities": {}, "clientInfo": { "name": "designos-eval", "version": env!("CARGO_PKG_VERSION") } }),
        )?;
        client.notify("notifications/initialized", json!({}))?;
        Ok(client)
    }

    /// Calls one tool. `Err` is a transport or protocol failure; a failing tool is `Ok` with `is_error`.
    pub fn call_tool(&mut self, name: &str, args: &Value) -> Result<ToolOutput, String> {
        let arguments = if args.is_null() { json!({}) } else { args.clone() };
        let result = self.request("tools/call", json!({ "name": name, "arguments": arguments }))?;
        let is_error = result.get("isError").and_then(Value::as_bool).unwrap_or(false);
        let value = match result.get("structuredContent") {
            Some(v) => v.clone(),
            None => {
                let text = result.pointer("/content/0/text").and_then(Value::as_str).unwrap_or_default();
                serde_json::from_str(text).unwrap_or_else(|_| Value::String(text.to_string()))
            }
        };
        Ok(ToolOutput { value, is_error })
    }

    fn send(&mut self, msg: &Value) -> Result<(), String> {
        let stdin = self.stdin.as_mut().ok_or("mcp_io_error: the session is already closed")?;
        let mut line = serde_json::to_string(msg).map_err(|e| format!("mcp_io_error: cannot encode request: {e}"))?;
        line.push('\n');
        stdin.write_all(line.as_bytes()).and_then(|()| stdin.flush()).map_err(|e| format!("mcp_io_error: cannot write to the server: {e}"))
    }

    fn notify(&mut self, method: &str, params: Value) -> Result<(), String> {
        self.send(&json!({ "jsonrpc": "2.0", "method": method, "params": params }))
    }

    fn request(&mut self, method: &str, params: Value) -> Result<Value, String> {
        self.next_id += 1;
        let id = self.next_id;
        self.send(&json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }))?;
        let deadline = Instant::now() + REQUEST_TIMEOUT;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            match self.incoming.recv_timeout(left) {
                Ok(msg) if msg.get("id").and_then(Value::as_u64) == Some(id) && msg.get("method").is_none() => {
                    if let Some(err) = msg.get("error") {
                        return Err(format!("mcp_protocol_error: {method} failed: {err}"));
                    }
                    return Ok(msg.get("result").cloned().unwrap_or(Value::Null));
                }
                Ok(_) => {} // notifications and unrelated messages
                Err(RecvTimeoutError::Timeout) => return Err(format!("mcp_timeout: no answer to {method} within {}s", REQUEST_TIMEOUT.as_secs())),
                Err(RecvTimeoutError::Disconnected) => return Err(format!("mcp_io_error: the server exited before answering {method}")),
            }
        }
    }

    /// Closes stdin so the server exits on its own, and kills it when it does not within the grace period.
    pub fn shutdown(&mut self) {
        self.stdin = None;
        let deadline = Instant::now() + SHUTDOWN_GRACE;
        while Instant::now() < deadline {
            if matches!(self.child.try_wait(), Ok(Some(_))) {
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for McpClient {
    fn drop(&mut self) {
        self.shutdown();
    }
}
