//! Multi-agent orchestration MCP tools (`designos_spawn_agents`, `designos_assign_frames`,
//! `designos_list_agents`, `designos_stop_agents`) for RD-3 (`docs/specs/v0.6/RD-3-parallel-agents-frame-ownership.md` §§4–6).

use crate::engine::Engine;
use crate::tools::lease::{structured_error, LeaseGuard};
use designos_doc::live_patch::{is_root_frame, Lease, LeasePurpose};
use designos_doc::{Command, Document, Node, NodeId, NodeKind, Rect};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Maximum number of concurrent sub-agents (`§5`, `§11`).
pub const MAX_SUB_AGENTS: usize = 8;

/// Horizontal gap in canvas pixels between side-by-side variant frames (`§3.3`).
pub const SIDE_BY_SIDE_GAP_PX: f32 = 80.0;

/// Supported CLI agent runtimes (`§5`).
pub const SUPPORTED_RUNTIMES: &[&str] = &["claude", "codex", "antigravity", "grok", "gemini", "opencode"];

/// Persisted agent run state in `<session_dir>/agents.json`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentRunRecord {
    pub run_id: String,
    pub name: String,
    pub runtime: String,
    pub frames: Vec<NodeId>,
    pub lead: bool,
    pub purpose: LeasePurpose,
    /// `"running"`, `"done"`, `"failed"`, `"stopped"`, `"violated"`, `"rolled_back"`
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bootstrap_path: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AgentsStateFile {
    pub version: u32,
    pub next_agent_seq: u32,
    pub agents: Vec<AgentRunRecord>,
}

/// Request written to `<session_dir>/spawn-requests.jsonl` for Studio to process.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum SpawnControlRequest {
    Spawn {
        request_id: String,
        run_id: String,
        name: String,
        runtime: String,
        frames: Vec<NodeId>,
        purpose: LeasePurpose,
        prompt: String,
        bootstrap_path: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        critic_runtime: Option<String>,
    },
    Stop {
        request_id: String,
        run_ids: Vec<String>,
        rollback: bool,
    },
}

/// Confirmation written by Studio to `<session_dir>/spawn-results.jsonl`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SpawnControlResult {
    pub request_id: String,
    pub run_id: String,
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Persisted state for `iterate` mode (`§5.2`) in `<session_dir>/iterate-runs.json`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IterateLoopSpec {
    pub generator_run_id: String,
    pub frame_id: NodeId,
    pub prompt: String,
    pub current_round: u8,
    pub max_rounds: u8,
    pub generator_runtime: String,
    pub critic_runtime: String,
    pub active_phase: String,
    pub completed: bool,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpawnTaskInput {
    #[serde(default, alias = "label")]
    pub name: Option<String>,
    #[serde(default, alias = "frames")]
    pub frame_ids: Vec<u64>,
    pub prompt: String,
    #[serde(default)]
    pub runtime: Option<String>,
    #[serde(default)]
    pub allow_shared: bool,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpawnVariantInput {
    pub name: String,
    pub prompt: String,
    pub source_frame_id: Option<u64>,
    pub width: Option<f32>,
    pub height: Option<f32>,
}

fn default_max_rounds() -> u8 {
    3
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpawnIterateInput {
    pub frame_id: u64,
    pub prompt: String,
    #[serde(default = "default_max_rounds")]
    pub max_rounds: u8,
    pub critic_runtime: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpawnAgentsArgs {
    pub mode: String,
    pub runtime: Option<String>,
    #[serde(default)]
    pub agents: Option<Vec<SpawnTaskInput>>,
    #[serde(default)]
    pub tasks: Option<Vec<SpawnTaskInput>>,
    #[serde(default)]
    pub variants: Option<Vec<SpawnVariantInput>>,
    #[serde(default)]
    pub iterate: Option<SpawnIterateInput>,
    #[serde(default)]
    pub prompt: Option<String>,
    #[serde(default)]
    pub count: Option<usize>,
    #[serde(default)]
    pub anchor_frame: Option<u64>,
    #[serde(default)]
    pub max_rounds: Option<u8>,
    #[serde(default)]
    pub style_mode: Option<String>,
    #[serde(default)]
    pub idempotency_key: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FrameAssignmentInput {
    pub run_id: String,
    #[serde(alias = "frames")]
    pub frame_ids: Vec<u64>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssignFramesArgs {
    #[serde(default)]
    pub run_id: Option<String>,
    #[serde(default, alias = "frame_ids")]
    pub frames: Option<Vec<u64>>,
    #[serde(default)]
    pub assignments: Vec<FrameAssignmentInput>,
    #[serde(default)]
    pub revoke: Vec<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StopAgentsArgs {
    #[serde(default)]
    pub run_ids: Vec<String>,
    #[serde(default)]
    pub all: bool,
    #[serde(default)]
    pub rollback: bool,
}

/// JSON Schema descriptors for the 4 multi-agent MCP tools (`§5`).
pub fn tool_descriptors() -> Vec<Value> {
    let runtime_schema = json!({
        "type": "string",
        "enum": ["claude", "codex", "antigravity", "grok", "gemini", "opencode"]
    });
    vec![
        json!({
            "name": "designos_spawn_agents",
            "description": "Spawn 1..=8 parallel sub-agents with enforced frame ownership in 'split', 'side_by_side', or 'iterate' mode (lead agent only, requires an active DESIGN:OS Studio session).",
            "inputSchema": {
                "type": "object",
                "required": ["mode"],
                "additionalProperties": false,
                "properties": {
                    "mode": {
                        "type": "string",
                        "enum": ["split", "side_by_side", "iterate"]
                    },
                    "runtime": runtime_schema,
                    "agents": {
                        "type": "array",
                        "minItems": 1,
                        "maxItems": 8,
                        "items": {
                            "type": "object",
                            "required": ["prompt"],
                            "additionalProperties": false,
                            "properties": {
                                "prompt": { "type": "string", "maxLength": 20000 },
                                "frames": {
                                    "type": "array",
                                    "items": { "type": "integer", "minimum": 1 }
                                },
                                "runtime": runtime_schema,
                                "label": { "type": "string", "maxLength": 64 },
                                "allow_shared": { "type": "boolean", "default": false }
                            }
                        }
                    },
                    "prompt": { "type": "string", "maxLength": 20000 },
                    "count": { "type": "integer", "minimum": 1, "maximum": 8 },
                    "anchor_frame": { "type": "integer", "minimum": 1 },
                    "max_rounds": { "type": "integer", "minimum": 1, "maximum": 20 },
                    "style_mode": {
                        "type": "string",
                        "enum": ["same_style", "new_style"]
                    },
                    "idempotency_key": { "type": "string", "maxLength": 128 },
                    "tasks": {
                        "type": "array",
                        "minItems": 1,
                        "maxItems": 8,
                        "items": {
                            "type": "object",
                            "required": ["frame_ids", "prompt"],
                            "additionalProperties": false,
                            "properties": {
                                "name": { "type": "string", "maxLength": 256 },
                                "frame_ids": {
                                    "type": "array",
                                    "minItems": 1,
                                    "items": { "type": "integer", "minimum": 1 }
                                },
                                "prompt": { "type": "string", "minLength": 1, "maxLength": 32000 }
                            }
                        }
                    },
                    "variants": {
                        "type": "array",
                        "minItems": 1,
                        "maxItems": 8,
                        "items": {
                            "type": "object",
                            "required": ["name", "prompt"],
                            "additionalProperties": false,
                            "properties": {
                                "name": { "type": "string", "minLength": 1, "maxLength": 256 },
                                "prompt": { "type": "string", "minLength": 1, "maxLength": 32000 },
                                "source_frame_id": { "type": "integer", "minimum": 1 },
                                "width": { "type": "number", "minimum": 1, "maximum": 100000, "default": 1440 },
                                "height": { "type": "number", "minimum": 1, "maximum": 100000, "default": 900 }
                            }
                        }
                    },
                    "iterate": {
                        "type": "object",
                        "required": ["frame_id", "prompt"],
                        "additionalProperties": false,
                        "properties": {
                            "frame_id": { "type": "integer", "minimum": 1 },
                            "prompt": { "type": "string", "minLength": 1, "maxLength": 32000 },
                            "max_rounds": { "type": "integer", "minimum": 1, "maximum": 20, "default": 3 },
                            "critic_runtime": runtime_schema
                        }
                    }
                }
            },
            "annotations": {
                "readOnlyHint": false,
                "destructiveHint": false,
                "idempotentHint": false,
                "openWorldHint": false
            }
        }),
        json!({
            "name": "designos_assign_frames",
            "description": "Update or revoke root-frame ownership leases for active sub-agent runs (lead agent only).",
            "inputSchema": {
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "run_id": { "type": "string", "minLength": 1, "maxLength": 128 },
                    "frames": {
                        "type": "array",
                        "items": { "type": "integer", "minimum": 1 }
                    },
                    "assignments": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "required": ["run_id", "frame_ids"],
                            "additionalProperties": false,
                            "properties": {
                                "run_id": { "type": "string", "minLength": 1, "maxLength": 128 },
                                "frame_ids": {
                                    "type": "array",
                                    "items": { "type": "integer", "minimum": 1 }
                                }
                            }
                        }
                    },
                    "revoke": {
                        "type": "array",
                        "items": { "type": "string", "minLength": 1, "maxLength": 128 }
                    }
                }
            },
            "annotations": {
                "readOnlyHint": false,
                "destructiveHint": false,
                "idempotentHint": true,
                "openWorldHint": false
            }
        }),
        json!({
            "name": "designos_list_agents",
            "description": "List all agent runs in the current Studio session along with their status, owned frames, purpose, and edit count.",
            "inputSchema": {
                "type": "object",
                "additionalProperties": false,
                "properties": {}
            },
            "annotations": {
                "readOnlyHint": true,
                "destructiveHint": false,
                "idempotentHint": true,
                "openWorldHint": false
            }
        }),
        json!({
            "name": "designos_stop_agents",
            "description": "Stop one or more sub-agent runs, release their frame leases, and optionally roll back all edits made by those runs (lead agent only).",
            "inputSchema": {
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "run_ids": {
                        "type": "array",
                        "items": { "type": "string", "minLength": 1, "maxLength": 128 }
                    },
                    "all": { "type": "boolean", "default": false },
                    "rollback": { "type": "boolean", "default": false }
                }
            },
            "annotations": {
                "readOnlyHint": false,
                "destructiveHint": true,
                "idempotentHint": false,
                "openWorldHint": false
            }
        }),
    ]
}

/// Checks whether the caller is the lead agent (`NOT_LEAD` error if a non-lead sub-agent calls a lead-only tool).
pub fn ensure_caller_is_lead(guard: &LeaseGuard, caller_run_id: Option<&str>, lead_run_id: Option<&str>) -> Result<(), String> {
    if let Some(caller) = caller_run_id {
        if let Some(lead) = lead_run_id {
            if caller != lead {
                return Err(structured_error(
                    "NOT_LEAD",
                    format!("run '{caller}' is a sub-agent; only the lead agent ('{lead}') may call this tool"),
                    "",
                    None,
                    false,
                ));
            }
        }
        if guard.table.get(caller).is_some() {
            return Err(structured_error(
                "NOT_LEAD",
                format!("run '{caller}' holds a sub-agent frame lease; only the lead agent may call this tool"),
                "",
                None,
                false,
            ));
        }
    }
    Ok(())
}

/// Checks whether a runtime CLI binary (`claude`, `codex`, `gemini`, `opencode`) is installed on `PATH`.
pub fn check_runtime_installed(runtime: &str) -> Result<(), String> {
    if !SUPPORTED_RUNTIMES.contains(&runtime) {
        return Err(structured_error(
            "RUNTIME_NOT_INSTALLED",
            format!("runtime '{runtime}' is not supported or not installed (supported: {})", SUPPORTED_RUNTIMES.join(", ")),
            "/runtime",
            Some(SUPPORTED_RUNTIMES.iter().map(|s| (*s).to_string()).collect()),
            false,
        ));
    }

    if let Ok(mock) = std::env::var("DESIGNOS_AVAILABLE_RUNTIMES") {
        let allowed: Vec<&str> = mock.split(',').map(str::trim).filter(|s| !s.is_empty()).collect();
        if allowed.contains(&runtime) {
            return Ok(());
        } else {
            return Err(structured_error(
                "RUNTIME_NOT_INSTALLED",
                format!("requested runtime binary '{runtime}' is not on PATH"),
                "/runtime",
                Some(allowed.into_iter().map(String::from).collect()),
                false,
            ));
        }
    }

    if cfg!(test) && std::env::var("DESIGNOS_STRICT_PATH_ONLY").ok().as_deref() != Some("1") {
        return Ok(());
    }

    let path_var = std::env::var_os("PATH").unwrap_or_default();
    let mut search_dirs: Vec<PathBuf> = std::env::split_paths(&path_var).collect();
    // Include standard macOS/Unix user bin locations when running inside a GUI app bundle.
    if std::env::var("DESIGNOS_STRICT_PATH_ONLY").ok().as_deref() != Some("1") {
        for extra in ["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin"] {
            search_dirs.push(PathBuf::from(extra));
        }
        if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
            search_dirs.push(home.join(".local/bin"));
            search_dirs.push(home.join(".cargo/bin"));
        }
    }

    let found = search_dirs.iter().any(|dir| {
        let candidate = dir.join(runtime);
        candidate.is_file()
    });

    if !found {
        return Err(structured_error("RUNTIME_NOT_INSTALLED", format!("requested runtime binary '{runtime}' is not on PATH"), "/runtime", None, false));
    }
    Ok(())
}

pub fn load_agents_state(session_dir: &Path) -> AgentsStateFile {
    let path = session_dir.join("agents.json");
    if let Ok(raw) = fs::read_to_string(&path) {
        if let Ok(parsed) = serde_json::from_str::<AgentsStateFile>(&raw) {
            return parsed;
        }
    }
    AgentsStateFile { version: 1, next_agent_seq: 1, agents: Vec::new() }
}

pub fn save_agents_state(session_dir: &Path, state: &AgentsStateFile) -> Result<(), String> {
    let _ = fs::create_dir_all(session_dir);
    let path = session_dir.join("agents.json");
    let bytes = serde_json::to_vec_pretty(state).map_err(|e| format!("serialize agents.json: {e}"))?;
    let tmp = path.with_extension(format!("tmp-{}", std::process::id()));
    fs::write(&tmp, &bytes).map_err(|e| format!("write {}: {e}", tmp.display()))?;
    fs::rename(&tmp, &path).map_err(|e| format!("rename {} -> {}: {e}", tmp.display(), path.display()))
}

fn append_spawn_request(session_dir: &Path, req: &SpawnControlRequest) -> Result<(), String> {
    let _ = fs::create_dir_all(session_dir);
    let path = session_dir.join("spawn-requests.jsonl");
    let line = serde_json::to_string(req).map_err(|e| format!("serialize spawn request: {e}"))? + "\n";
    let mut f = fs::OpenOptions::new().create(true).append(true).open(&path).map_err(|e| format!("open {}: {e}", path.display()))?;
    f.write_all(line.as_bytes()).and_then(|_| f.flush()).map_err(|e| format!("write {}: {e}", path.display()))
}

#[allow(clippy::too_many_arguments)]
fn write_bootstrap_files(
    session_dir: &Path,
    doc: &Document,
    run_id: &str,
    agent_name: &str,
    mode: &str,
    purpose: &LeasePurpose,
    frames: &[NodeId],
    user_prompt: &str,
) -> Result<PathBuf, String> {
    let _ = fs::create_dir_all(session_dir);
    let frame_summaries: Vec<Value> = frames
        .iter()
        .filter_map(|&fid| {
            let n = doc.get(fid)?;
            Some(json!({
                "id": fid.0,
                "name": n.name,
                "x": n.transform.tx,
                "y": n.transform.ty,
                "width": n.size.x,
                "height": n.size.y,
                "child_count": n.children.len(),
            }))
        })
        .collect();
    let frame_ids_str = frames.iter().map(|f| f.0.to_string()).collect::<Vec<_>>().join(", ");

    let system_prompt = format!(
        "# DESIGN:OS Sub-Agent Bootstrap ({run_id})\n\n\
         - **Run ID**: `{run_id}`\n\
         - **Agent Name**: {agent_name}\n\
         - **Mode**: `{mode}`\n\
         - **Owned Root Frame IDs**: `[{frame_ids_str}]`\n\n\
         ## Frame Ownership Contract (MANDATORY)\n\
         You may ONLY create, update, move, or delete nodes inside your leased root frame(s): `[{frame_ids_str}]`.\n\
         Any attempt to mutate nodes outside `[{frame_ids_str}]` or perform document-global mutations will be refused with `OUTSIDE_OWNED_FRAMES`.\n\
         Always pass `parent_id` inside `[{frame_ids_str}]` when calling `designos_create_node`, `designos_apply_spec`, or `designos_apply_batch`.\n\n\
         ## Task\n\
         {user_prompt}\n"
    );

    let json_payload = json!({
        "run_id": run_id,
        "agent_name": agent_name,
        "mode": mode,
        "purpose": purpose,
        "owned_frames": frame_summaries,
        "variable_count": doc.variables.len(),
        "style_count": doc.styles.len(),
        "task_prompt": user_prompt,
        "bootstrap_markdown": system_prompt,
    });

    let json_path = session_dir.join(format!("bootstrap.{run_id}.json"));
    let md_path = session_dir.join(format!("bootstrap.{run_id}.md"));
    fs::write(&json_path, serde_json::to_vec_pretty(&json_payload).map_err(|e| e.to_string())?).map_err(|e| format!("write {}: {e}", json_path.display()))?;
    fs::write(&md_path, system_prompt.as_bytes()).map_err(|e| format!("write {}: {e}", md_path.display()))?;
    Ok(md_path)
}

/// Clones an existing root frame subtree with freshly allocated `NodeId`s on `doc`, positioned at `(x, y)`.
fn clone_frame_subtree(
    doc: &mut Document,
    source_frame_id: NodeId,
    new_name: &str,
    x: f32,
    y: f32,
    width_override: Option<f32>,
    height_override: Option<f32>,
) -> Result<Vec<Node>, String> {
    let mut orig_nodes = Vec::new();
    doc.walk(source_frame_id, |n, _| orig_nodes.push(n.clone()));
    if orig_nodes.is_empty() {
        return Err(format!("source frame {} not found", source_frame_id.0));
    }
    let mut remap = BTreeMap::new();
    for n in &orig_nodes {
        let fresh_id = doc.try_alloc_id().ok_or("Document has exhausted node IDs")?;
        remap.insert(n.id, fresh_id);
    }
    let map_id = |id: NodeId| remap.get(&id).copied().unwrap_or(id);
    for n in &mut orig_nodes {
        n.id = map_id(n.id);
        n.parent = n.parent.map(map_id);
        for c in &mut n.children {
            *c = map_id(*c);
        }
    }
    let root = &mut orig_nodes[0];
    root.name = new_name.to_string();
    root.transform.tx = x;
    root.transform.ty = y;
    if let Some(w) = width_override {
        root.size.x = w;
    }
    if let Some(h) = height_override {
        root.size.y = h;
    }
    Ok(orig_nodes)
}

/// Validates that `frame_id` exists in `doc` and is a top-level frame on a page.
fn validate_root_frame(doc: &Document, frame_id: u64, path: &str) -> Result<NodeId, String> {
    if frame_id == 0 {
        return Err(structured_error("VALUE_OUT_OF_RANGE", "frame_id must be a positive integer", path, None, false));
    }
    let fid = NodeId(frame_id);
    let Some(node) = doc.get(fid) else {
        return Err(structured_error("NODE_NOT_FOUND", format!("frame {} not found in document", frame_id), path, None, false));
    };
    if !is_root_frame(doc, fid) {
        return Err(structured_error(
            "INVALID_ARGUMENTS",
            format!("node {} ('{}') is not a top-level root frame on a page", frame_id, node.name),
            path,
            None,
            false,
        ));
    }
    Ok(fid)
}

/// Handles `designos_spawn_agents` (`§§4–5`).
pub fn spawn_agents(engine: &mut Engine, guard: &mut LeaseGuard, caller_run_id: Option<&str>, lead_run_id: Option<&str>, args: Value) -> Result<Value, String> {
    guard.reload_if_needed();
    ensure_caller_is_lead(guard, caller_run_id, lead_run_id)?;

    let Some(session_dir) = guard.session_dir.clone() else {
        return Err(structured_error(
            "SPAWN_REQUIRES_STUDIO",
            "designos_spawn_agents requires an active DESIGN:OS Studio session (DESIGNOS_SESSION_DIR is not set)",
            "",
            None,
            false,
        ));
    };

    let parsed: SpawnAgentsArgs =
        serde_json::from_value(args).map_err(|e| structured_error("INVALID_ARGUMENTS", format!("invalid arguments: {e}"), "", None, false))?;

    let runtime = parsed.runtime.as_deref().unwrap_or("claude").to_string();
    check_runtime_installed(&runtime)?;

    let mut agents_state = load_agents_state(&session_dir);
    let active_sub_agents = guard.table.leases.len();
    let effective_lead_id = lead_run_id.or(caller_run_id).unwrap_or("lead").to_string();

    match parsed.mode.as_str() {
        "split" => {
            let Some(tasks) = parsed.tasks.or(parsed.agents) else {
                return Err(structured_error(
                    "INVALID_ARGUMENTS",
                    "mode 'split' requires an 'agents' (or 'tasks') array of 1..=8 items",
                    "/agents",
                    None,
                    false,
                ));
            };
            if tasks.is_empty() || tasks.len() > MAX_SUB_AGENTS {
                return Err(structured_error(
                    "TOO_MANY_AGENTS",
                    format!("mode 'split' supports 1..={MAX_SUB_AGENTS} tasks, got {}", tasks.len()),
                    "/agents",
                    None,
                    false,
                ));
            }
            if active_sub_agents + tasks.len() > MAX_SUB_AGENTS {
                return Err(structured_error(
                    "TOO_MANY_AGENTS",
                    format!(
                        "spawning {} sub-agents would exceed the maximum of {MAX_SUB_AGENTS} concurrent sub-agents ({} already active)",
                        tasks.len(),
                        active_sub_agents
                    ),
                    "/agents",
                    None,
                    false,
                ));
            }

            // Validate all frame_ids and check for duplicates / existing leases before mutating anything.
            let mut seen_in_call = BTreeSet::new();
            let mut validated_tasks: Vec<(String, Vec<NodeId>, String, String, bool)> = Vec::with_capacity(tasks.len());
            for (idx, t) in tasks.into_iter().enumerate() {
                let task_rt = t.runtime.unwrap_or_else(|| runtime.clone());
                check_runtime_installed(&task_rt)?;
                if t.frame_ids.is_empty() {
                    return Err(structured_error(
                        "INVALID_ARGUMENTS",
                        "frames must contain at least 1 root frame ID",
                        &format!("/agents/{idx}/frames"),
                        None,
                        false,
                    ));
                }
                if t.prompt.trim().is_empty() {
                    return Err(structured_error("INVALID_ARGUMENTS", "prompt must not be empty", &format!("/agents/{idx}/prompt"), None, false));
                }
                let mut fids = Vec::with_capacity(t.frame_ids.len());
                for (f_idx, raw_fid) in t.frame_ids.iter().copied().enumerate() {
                    let fid = validate_root_frame(&engine.doc, raw_fid, &format!("/agents/{idx}/frames/{f_idx}"))?;
                    if !seen_in_call.insert(fid) {
                        return Err(structured_error(
                            "FRAME_LEASED",
                            format!("frame {} is claimed more than once in the same spawn call", fid.0),
                            &format!("/agents/{idx}/frames/{f_idx}"),
                            None,
                            false,
                        ));
                    }
                    if let Some(existing_owner) = guard.table.owner_of_frame(fid) {
                        return Err(structured_error(
                            "FRAME_LEASED",
                            format!("frame {} is already leased to active run '{}'", fid.0, existing_owner.run_id),
                            &format!("/agents/{idx}/frames/{f_idx}"),
                            None,
                            false,
                        ));
                    }
                    fids.push(fid);
                }
                let name = t.name.unwrap_or_else(|| format!("Sub-agent {}", idx + 1));
                validated_tasks.push((name, fids, t.prompt, task_rt, t.allow_shared));
            }

            guard.epoch += 1;

            let mut spawned_json = Vec::with_capacity(validated_tasks.len());
            for (name, fids, prompt, task_rt, allow_shared) in validated_tasks {
                let run_id = alloc_run_id(&mut agents_state, &guard.table);
                let purpose = LeasePurpose::Split;
                let md_path = write_bootstrap_files(&session_dir, &engine.doc, &run_id, &name, "split", &purpose, &fids, &prompt)?;
                let bootstrap_str = md_path.to_string_lossy().to_string();

                guard
                    .table
                    .leases
                    .push(Lease::new(run_id.clone(), fids.clone(), purpose).with_allow_shared(allow_shared).with_lead(Some(effective_lead_id.clone())));

                agents_state.agents.push(AgentRunRecord {
                    run_id: run_id.clone(),
                    name: name.clone(),
                    runtime: task_rt.clone(),
                    frames: fids.clone(),
                    lead: false,
                    purpose,
                    status: "running".to_string(),
                    prompt: Some(prompt.clone()),
                    bootstrap_path: Some(bootstrap_str.clone()),
                });

                let req = SpawnControlRequest::Spawn {
                    request_id: format!("req-{}-{}", guard.epoch, run_id),
                    run_id: run_id.clone(),
                    name: name.clone(),
                    runtime: task_rt.clone(),
                    frames: fids.clone(),
                    purpose,
                    prompt,
                    bootstrap_path: bootstrap_str.clone(),
                    critic_runtime: None,
                };
                append_spawn_request(&session_dir, &req)?;

                spawned_json.push(json!({
                    "run_id": run_id,
                    "name": name,
                    "label": name,
                    "runtime": task_rt,
                    "frames": fids.iter().map(|f| f.0).collect::<Vec<_>>(),
                    "purpose": purpose,
                    "status": "running",
                    "bootstrap_path": bootstrap_str,
                }));
            }

            guard.save()?;
            save_agents_state(&session_dir, &agents_state)?;
            engine.leases = guard.table.clone();

            Ok(json!({
                "mode": "split",
                "epoch": guard.epoch,
                "runs": spawned_json,
                "spawned": spawned_json,
                "changed": false,
            }))
        }
        "side_by_side" => {
            let variants: Vec<SpawnVariantInput> = if let Some(v) = parsed.variants {
                v
            } else {
                let count = parsed.count.or_else(|| parsed.agents.as_ref().map(Vec::len)).unwrap_or(0);
                if count == 0 {
                    return Err(structured_error("INVALID_ARGUMENTS", "mode 'side_by_side' requires 'count' (1..=8) or 'variants'", "/count", None, false));
                }
                if count > MAX_SUB_AGENTS {
                    return Err(structured_error(
                        "TOO_MANY_AGENTS",
                        format!("mode 'side_by_side' supports at most {MAX_SUB_AGENTS} concurrent runs, got {count}"),
                        "/count",
                        None,
                        false,
                    ));
                }
                let base_prompt =
                    parsed.prompt.clone().or_else(|| parsed.agents.as_ref().and_then(|a| a.first()).map(|t| t.prompt.clone())).unwrap_or_default();
                let mut synthesized = Vec::with_capacity(count);
                for idx in 0..count {
                    let agent_item = parsed.agents.as_ref().and_then(|a| a.get(idx));
                    let v_name = agent_item.and_then(|a| a.name.clone()).unwrap_or_else(|| format!("Variant {}", idx + 1));
                    let v_prompt = agent_item.map(|a| a.prompt.clone()).unwrap_or_else(|| base_prompt.clone());
                    synthesized.push(SpawnVariantInput { name: v_name, prompt: v_prompt, source_frame_id: parsed.anchor_frame, width: None, height: None });
                }
                synthesized
            };

            if variants.is_empty() || variants.len() > MAX_SUB_AGENTS {
                return Err(structured_error(
                    "TOO_MANY_AGENTS",
                    format!("mode 'side_by_side' requires 1..={MAX_SUB_AGENTS} variants, got {}", variants.len()),
                    "/variants",
                    None,
                    false,
                ));
            }
            if active_sub_agents + variants.len() > MAX_SUB_AGENTS {
                return Err(structured_error(
                    "TOO_MANY_AGENTS",
                    format!(
                        "spawning {} variant sub-agents would exceed the maximum of {MAX_SUB_AGENTS} concurrent sub-agents ({} already active)",
                        variants.len(),
                        active_sub_agents
                    ),
                    "/variants",
                    None,
                    false,
                ));
            }

            let page_id = engine.resolve_default_page().map_err(|e| structured_error("NODE_NOT_FOUND", e, "/variants", None, false))?;

            // Validate any source_frame_id references first.
            for (idx, v) in variants.iter().enumerate() {
                if v.name.trim().is_empty() {
                    return Err(structured_error("INVALID_ARGUMENTS", "variant name must not be empty", &format!("/variants/{idx}/name"), None, false));
                }
                if v.prompt.trim().is_empty() {
                    return Err(structured_error("INVALID_ARGUMENTS", "variant prompt must not be empty", &format!("/variants/{idx}/prompt"), None, false));
                }
                if let Some(src_raw) = v.source_frame_id {
                    validate_root_frame(&engine.doc, src_raw, &format!("/variants/{idx}/source_frame_id"))?;
                }
            }

            // Compute placement X and Y to the right of the rightmost existing root frame on `page_id` (§5.1).
            let page_children = engine.doc.get(page_id).map(|p| p.children.clone()).unwrap_or_default();
            let mut rightmost_edge: Option<f32> = None;
            let mut baseline_y: f32 = 0.0;
            for cid in &page_children {
                if let Some(cn) = engine.doc.get(*cid) {
                    if matches!(cn.kind, NodeKind::Frame(_)) {
                        let right = cn.transform.tx + cn.size.x;
                        if rightmost_edge.is_none_or(|prev| right > prev) {
                            rightmost_edge = Some(right);
                            baseline_y = cn.transform.ty;
                        }
                    }
                }
            }
            if let Some(first_src) = variants.first().and_then(|v| v.source_frame_id) {
                if let Some(src_node) = engine.doc.get(NodeId(first_src)) {
                    baseline_y = src_node.transform.ty;
                }
            }

            let mut next_x = match rightmost_edge {
                Some(edge) => edge + SIDE_BY_SIDE_GAP_PX,
                None => 0.0,
            };

            let mut trial = engine.doc.clone();
            let mut insert_cmds = Vec::with_capacity(variants.len());
            let mut created_frames: Vec<(NodeId, String, String)> = Vec::with_capacity(variants.len());

            for (idx, v) in variants.into_iter().enumerate() {
                let w = v.width.unwrap_or(1440.0);
                let h = v.height.unwrap_or(900.0);
                let insert_index = trial.get(page_id).map(|p| p.children.len()).unwrap_or(0);

                let nodes = if let Some(src_raw) = v.source_frame_id {
                    clone_frame_subtree(&mut trial, NodeId(src_raw), &v.name, next_x, baseline_y, v.width, v.height)?
                } else {
                    let fid = trial
                        .try_alloc_id()
                        .ok_or_else(|| structured_error("INTERNAL_ERROR", "Document has exhausted node IDs", &format!("/variants/{idx}"), None, false))?;
                    let mut frame = Node::frame(fid, Rect::new(next_x, baseline_y, w, h));
                    frame.name = v.name.clone();
                    frame.parent = Some(page_id);
                    vec![frame]
                };

                let frame_id = nodes[0].id;
                let frame_w = nodes[0].size.x;
                next_x += frame_w + SIDE_BY_SIDE_GAP_PX;

                let cmd = Command::Insert { parent: page_id, index: insert_index, nodes };
                cmd.clone().apply(&mut trial).map_err(|e| structured_error("INTERNAL_ERROR", e.to_string(), &format!("/variants/{idx}"), None, false))?;
                insert_cmds.push(cmd);
                created_frames.push((frame_id, v.name, v.prompt));
            }

            // Commit all variant frames as one atomic Command::Batch on Engine.
            engine.exec(Command::Batch(insert_cmds))?;
            engine.relayout(page_id);

            guard.epoch += 1;
            let variant_group = format!("vgroup-{}", guard.epoch);

            let mut spawned_json = Vec::with_capacity(created_frames.len());
            let mut created_frame_ids = Vec::with_capacity(created_frames.len());

            for (frame_id, v_name, v_prompt) in created_frames.into_iter() {
                created_frame_ids.push(frame_id.0);
                let run_id = alloc_run_id(&mut agents_state, &guard.table);
                let purpose = LeasePurpose::SideBySide;
                let fids = vec![frame_id];
                let md_path = write_bootstrap_files(&session_dir, &engine.doc, &run_id, &v_name, "side_by_side", &purpose, &fids, &v_prompt)?;
                let bootstrap_str = md_path.to_string_lossy().to_string();

                guard.table.leases.push(Lease::new(run_id.clone(), fids.clone(), purpose).with_lead(Some(effective_lead_id.clone())));

                agents_state.agents.push(AgentRunRecord {
                    run_id: run_id.clone(),
                    name: v_name.clone(),
                    runtime: runtime.clone(),
                    frames: fids.clone(),
                    lead: false,
                    purpose,
                    status: "running".to_string(),
                    prompt: Some(v_prompt.clone()),
                    bootstrap_path: Some(bootstrap_str.clone()),
                });

                let req = SpawnControlRequest::Spawn {
                    request_id: format!("req-{}-{}", guard.epoch, run_id),
                    run_id: run_id.clone(),
                    name: v_name.clone(),
                    runtime: runtime.clone(),
                    frames: fids.clone(),
                    purpose,
                    prompt: v_prompt,
                    bootstrap_path: bootstrap_str.clone(),
                    critic_runtime: None,
                };
                append_spawn_request(&session_dir, &req)?;

                spawned_json.push(json!({
                    "run_id": run_id,
                    "name": v_name,
                    "label": v_name,
                    "runtime": runtime,
                    "frame_id": frame_id.0,
                    "frames": [frame_id.0],
                    "purpose": purpose,
                    "status": "running",
                    "bootstrap_path": bootstrap_str,
                }));
            }

            guard.save()?;
            save_agents_state(&session_dir, &agents_state)?;
            engine.leases = guard.table.clone();

            Ok(json!({
                "mode": "side_by_side",
                "epoch": guard.epoch,
                "variant_group": variant_group,
                "created_frames": created_frame_ids,
                "runs": spawned_json,
                "spawned": spawned_json,
                "changed": true,
            }))
        }
        "iterate" => {
            let iter_cfg = if let Some(cfg) = parsed.iterate {
                cfg
            } else {
                let first_agent = parsed.agents.as_ref().and_then(|a| a.first());
                let fid = parsed.anchor_frame.or_else(|| first_agent.and_then(|a| a.frame_ids.first().copied())).unwrap_or(0);
                let prompt = parsed.prompt.or_else(|| first_agent.map(|a| a.prompt.clone())).unwrap_or_default();
                if fid == 0 || prompt.trim().is_empty() {
                    return Err(structured_error(
                        "INVALID_ARGUMENTS",
                        "mode 'iterate' requires 'iterate' or ('anchor_frame' + 'prompt')",
                        "/iterate",
                        None,
                        false,
                    ));
                }
                SpawnIterateInput { frame_id: fid, prompt, max_rounds: parsed.max_rounds.unwrap_or_else(default_max_rounds), critic_runtime: None }
            };
            if !(1..=20).contains(&iter_cfg.max_rounds) {
                return Err(structured_error("INVALID_ARGUMENTS", "iterate.max_rounds must be between 1 and 20", "/iterate/max_rounds", None, false));
            }
            if iter_cfg.prompt.trim().is_empty() {
                return Err(structured_error("INVALID_ARGUMENTS", "iterate.prompt must not be empty", "/iterate/prompt", None, false));
            }
            let critic_rt = iter_cfg.critic_runtime.clone().unwrap_or_else(|| runtime.clone());
            check_runtime_installed(&critic_rt)?;

            if active_sub_agents + 1 > MAX_SUB_AGENTS {
                return Err(structured_error(
                    "TOO_MANY_AGENTS",
                    format!("cannot start iterate loop: {active_sub_agents} sub-agents already active (max {MAX_SUB_AGENTS})"),
                    "/iterate",
                    None,
                    false,
                ));
            }

            let fid = validate_root_frame(&engine.doc, iter_cfg.frame_id, "/iterate/frame_id")?;
            if let Some(existing_owner) = guard.table.owner_of_frame(fid) {
                return Err(structured_error(
                    "FRAME_LEASED",
                    format!("frame {} is already leased to active run '{}'", fid.0, existing_owner.run_id),
                    "/iterate/frame_id",
                    None,
                    false,
                ));
            }

            guard.epoch += 1;

            let run_id = alloc_run_id(&mut agents_state, &guard.table);
            let name = format!("Iterate R1/{} ({})", iter_cfg.max_rounds, runtime);
            let purpose = LeasePurpose::Iterate;
            let fids = vec![fid];
            let md_path = write_bootstrap_files(&session_dir, &engine.doc, &run_id, &name, "iterate", &purpose, &fids, &iter_cfg.prompt)?;
            let bootstrap_str = md_path.to_string_lossy().to_string();

            guard.table.leases.push(Lease::new(run_id.clone(), fids.clone(), purpose).with_lead(Some(effective_lead_id.clone())));

            agents_state.agents.push(AgentRunRecord {
                run_id: run_id.clone(),
                name: name.clone(),
                runtime: runtime.clone(),
                frames: fids.clone(),
                lead: false,
                purpose,
                status: "running".to_string(),
                prompt: Some(iter_cfg.prompt.clone()),
                bootstrap_path: Some(bootstrap_str.clone()),
            });

            // Save iterate loop state for Studio's Generator -> Critic -> Generator supervisor (§5.2).
            let iter_path = session_dir.join("iterate-runs.json");
            let mut loops: Vec<IterateLoopSpec> = fs::read_to_string(&iter_path).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
            loops.push(IterateLoopSpec {
                generator_run_id: run_id.clone(),
                frame_id: fid,
                prompt: iter_cfg.prompt.clone(),
                current_round: 1,
                max_rounds: iter_cfg.max_rounds,
                generator_runtime: runtime.clone(),
                critic_runtime: critic_rt.clone(),
                active_phase: "generator".to_string(),
                completed: false,
            });
            let _ = fs::write(&iter_path, serde_json::to_vec_pretty(&loops).unwrap_or_default());

            let req = SpawnControlRequest::Spawn {
                request_id: format!("req-{}-{}", guard.epoch, run_id),
                run_id: run_id.clone(),
                name: name.clone(),
                runtime: runtime.clone(),
                frames: fids.clone(),
                purpose,
                prompt: iter_cfg.prompt,
                bootstrap_path: bootstrap_str.clone(),
                critic_runtime: Some(critic_rt.clone()),
            };
            append_spawn_request(&session_dir, &req)?;

            guard.save()?;
            save_agents_state(&session_dir, &agents_state)?;
            engine.leases = guard.table.clone();

            let entry = json!({
                "run_id": run_id,
                "name": name,
                "label": name,
                "runtime": runtime,
                "critic_runtime": critic_rt,
                "frames": [fid.0],
                "purpose": purpose,
                "status": "running",
                "bootstrap_path": bootstrap_str,
            });

            Ok(json!({
                "mode": "iterate",
                "epoch": guard.epoch,
                "runs": [entry.clone()],
                "spawned": [entry],
                "changed": false,
            }))
        }
        other => Err(structured_error(
            "INVALID_ARGUMENTS",
            format!("unknown mode '{other}'; expected 'split', 'side_by_side', or 'iterate'"),
            "/mode",
            Some(vec!["split".into(), "side_by_side".into(), "iterate".into()]),
            false,
        )),
    }
}

fn alloc_run_id(state: &mut AgentsStateFile, table: &designos_doc::live_patch::LeaseTable) -> String {
    loop {
        let seq = state.next_agent_seq.max(1);
        state.next_agent_seq = seq + 1;
        let candidate = format!("agent-{seq}");
        let used_in_state = state.agents.iter().any(|a| a.run_id == candidate);
        let used_in_table = table.leases.iter().any(|l| l.run_id == candidate);
        if !used_in_state && !used_in_table {
            return candidate;
        }
    }
}

/// Handles `designos_assign_frames` (`§5`).
pub fn assign_frames(
    engine: &mut Engine,
    guard: &mut LeaseGuard,
    caller_run_id: Option<&str>,
    lead_run_id: Option<&str>,
    args: Value,
) -> Result<Value, String> {
    guard.reload_if_needed();
    ensure_caller_is_lead(guard, caller_run_id, lead_run_id)?;

    let mut parsed: AssignFramesArgs =
        serde_json::from_value(args).map_err(|e| structured_error("INVALID_ARGUMENTS", format!("invalid arguments: {e}"), "", None, false))?;

    if let (Some(single_run), Some(single_frames)) = (parsed.run_id.take(), parsed.frames.take()) {
        parsed.assignments.push(FrameAssignmentInput { run_id: single_run, frame_ids: single_frames });
    }

    let effective_lead_id = lead_run_id.or(caller_run_id).unwrap_or("lead").to_string();

    // Validate all requested frame_ids first and verify none is leased to a different active run.
    let mut updated_table = guard.table.clone();
    for rev in &parsed.revoke {
        updated_table.leases.retain(|l| &l.run_id != rev);
    }

    for (a_idx, assign) in parsed.assignments.iter().enumerate() {
        let mut fids = Vec::with_capacity(assign.frame_ids.len());
        for (f_idx, &raw_fid) in assign.frame_ids.iter().enumerate() {
            let fid = validate_root_frame(&engine.doc, raw_fid, &format!("/assignments/{a_idx}/frame_ids/{f_idx}"))?;
            if let Some(owner) = updated_table.owner_of_frame(fid) {
                if owner.run_id != assign.run_id {
                    return Err(structured_error(
                        "FRAME_LEASED",
                        format!("frame {} is already leased to active run '{}'", fid.0, owner.run_id),
                        &format!("/assignments/{a_idx}/frame_ids/{f_idx}"),
                        None,
                        false,
                    ));
                }
            }
            if !fids.contains(&fid) {
                fids.push(fid);
            }
        }
        if let Some(existing) = updated_table.leases.iter_mut().find(|l| l.run_id == assign.run_id) {
            existing.frames = fids;
        } else {
            updated_table.leases.push(Lease::new(assign.run_id.clone(), fids, LeasePurpose::Split).with_lead(Some(effective_lead_id.clone())));
        }
    }

    guard.set_table(updated_table);
    let _ = guard.save();
    engine.leases = guard.table.clone();

    if let Some(session_dir) = &guard.session_dir {
        let mut state = load_agents_state(session_dir);
        for rev in &parsed.revoke {
            if let Some(a) = state.agents.iter_mut().find(|a| &a.run_id == rev) {
                a.frames.clear();
            }
        }
        for assign in &parsed.assignments {
            let fids: Vec<NodeId> = assign.frame_ids.iter().copied().map(NodeId).collect();
            if let Some(a) = state.agents.iter_mut().find(|a| a.run_id == assign.run_id) {
                a.frames = fids;
            } else {
                state.agents.push(AgentRunRecord {
                    run_id: assign.run_id.clone(),
                    name: assign.run_id.clone(),
                    runtime: "claude".to_string(),
                    frames: fids,
                    lead: false,
                    purpose: LeasePurpose::Split,
                    status: "running".to_string(),
                    prompt: None,
                    bootstrap_path: None,
                });
            }
        }
        let _ = save_agents_state(session_dir, &state);
    }

    Ok(json!({
        "epoch": guard.epoch,
        "leases": guard.table.leases,
        "changed": false,
    }))
}

/// Handles `designos_list_agents` (`§5`).
pub fn list_agents(engine: &Engine, guard: &mut LeaseGuard) -> Result<Value, String> {
    guard.reload_if_needed();
    let mut records: Vec<Value> = Vec::new();
    let mut seen_runs = BTreeSet::new();

    if let Some(session_dir) = &guard.session_dir {
        let state = load_agents_state(session_dir);
        for a in state.agents {
            let active_lease = guard.table.get(&a.run_id);
            let frames: Vec<u64> = active_lease.map(|l| l.frames.iter().map(|f| f.0).collect()).unwrap_or_else(|| a.frames.iter().map(|f| f.0).collect());
            let edit_count = engine.history.run_step_count(&a.run_id);
            seen_runs.insert(a.run_id.clone());
            records.push(json!({
                "run_id": a.run_id,
                "name": a.name,
                "runtime": a.runtime,
                "status": a.status,
                "frames": frames,
                "lead": a.lead,
                "purpose": a.purpose,
                "edit_count": edit_count,
            }));
        }
    }

    for l in &guard.table.leases {
        if !seen_runs.contains(&l.run_id) {
            let edit_count = engine.history.run_step_count(&l.run_id);
            records.push(json!({
                "run_id": l.run_id,
                "name": l.run_id,
                "runtime": "claude",
                "status": "running",
                "frames": l.frames.iter().map(|f| f.0).collect::<Vec<_>>(),
                "lead": false,
                "purpose": l.purpose,
                "edit_count": edit_count,
            }));
        }
    }

    Ok(json!({
        "epoch": guard.epoch,
        "agents": records,
    }))
}

/// Handles `designos_stop_agents` (`§5`).
pub fn stop_agents(engine: &mut Engine, guard: &mut LeaseGuard, caller_run_id: Option<&str>, lead_run_id: Option<&str>, args: Value) -> Result<Value, String> {
    guard.reload_if_needed();
    ensure_caller_is_lead(guard, caller_run_id, lead_run_id)?;

    let parsed: StopAgentsArgs =
        serde_json::from_value(args).map_err(|e| structured_error("INVALID_ARGUMENTS", format!("invalid arguments: {e}"), "", None, false))?;

    let target_runs: Vec<String> =
        if parsed.all || parsed.run_ids.is_empty() { guard.table.leases.iter().map(|l| l.run_id.clone()).collect() } else { parsed.run_ids.clone() };

    guard.table.leases.retain(|l| !target_runs.contains(&l.run_id));
    guard.epoch += 1;
    guard.save()?;
    engine.leases = guard.table.clone();

    let mut rolled_back_steps = 0usize;
    if parsed.rollback {
        for rid in &target_runs {
            rolled_back_steps += engine.undo_run_and_record(rid).map_err(|e| structured_error("INTERNAL_ERROR", e, "", None, false))?;
        }
    }

    if let Some(session_dir) = &guard.session_dir {
        let mut state = load_agents_state(session_dir);
        for a in &mut state.agents {
            if target_runs.contains(&a.run_id) {
                a.status = if parsed.rollback { "rolled_back".to_string() } else { "stopped".to_string() };
                a.frames.clear();
            }
        }
        let _ = save_agents_state(session_dir, &state);
        let req = SpawnControlRequest::Stop { request_id: format!("stop-{}", guard.epoch), run_ids: target_runs.clone(), rollback: parsed.rollback };
        let _ = append_spawn_request(session_dir, &req);
    }

    Ok(json!({
        "epoch": guard.epoch,
        "stopped": target_runs,
        "rollback": parsed.rollback,
        "rolled_back_steps": rolled_back_steps,
        "changed": rolled_back_steps > 0,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::Session;
    use designos_doc::live_patch::PLUGIN_DATA_RUN_KEY;
    use designos_doc::{Node, NodeId, Rect};
    use serde_json::json;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_SEQ: AtomicU64 = AtomicU64::new(1);

    fn temp_session_dir(tag: &str) -> PathBuf {
        let seq = TEST_SEQ.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("designos-rd3-test-{}-{}-{}", tag, std::process::id(), seq));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn write_outside_lease_refused() {
        let session_dir = temp_session_dir("outside-lease");
        let mut srv = Session::with_session_dir(&session_dir, &session_dir).unwrap();

        // Insert frame 10 and frame 20 on Page 1, plus a child rectangle under frame 20.
        let page_id = srv.engine.doc.pages[0];
        srv.engine.doc.reserve_ids(NodeId(20));
        srv.engine.doc.insert(page_id, Node::frame(NodeId(10), Rect::new(0.0, 0.0, 400.0, 300.0))).unwrap();
        srv.engine.doc.insert(page_id, Node::frame(NodeId(20), Rect::new(500.0, 0.0, 400.0, 300.0))).unwrap();
        let child_21 = NodeId(21);
        srv.engine.doc.reserve_ids(child_21);
        srv.engine.doc.insert(NodeId(20), Node::rectangle(child_21, Rect::new(10.0, 10.0, 80.0, 40.0))).unwrap();

        // Grant lease on frame [10] to agent-1.
        srv.lease_guard.table.grant(Lease::new("agent-1", vec![NodeId(10)], LeasePurpose::Split)).unwrap();
        srv.lease_guard.save().unwrap();
        srv.bridge.run_id = Some("agent-1".to_string());

        let rev_before = srv.engine.doc.mutation_epoch();
        let err = srv
            .call(
                "designos_update_node",
                json!({
                    "node_id": 21,
                    "name": "ShouldBeRefused"
                }),
            )
            .expect_err("update_node on frame 20's child must be refused");

        assert!(err.contains("OUTSIDE_OWNED_FRAMES"), "expected OUTSIDE_OWNED_FRAMES, got: {err}");
        assert_eq!(srv.engine.doc.mutation_epoch(), rev_before);
        assert_ne!(srv.engine.doc.get(child_21).unwrap().name, "ShouldBeRefused");
        let _ = fs::remove_dir_all(&session_dir);
    }

    #[test]
    fn side_by_side_creates_frames() {
        let session_dir = temp_session_dir("side-by-side");
        let mut srv = Session::with_session_dir(&session_dir, &session_dir).unwrap();
        srv.bridge.run_id = Some("lead".to_string());
        srv.bridge.lead_run_id = Some("lead".to_string());

        let page_id = srv.engine.doc.pages[0];
        let frames_before = srv.engine.doc.get(page_id).unwrap().children.len();

        let res = srv
            .call(
                "designos_spawn_agents",
                json!({
                    "mode": "side_by_side",
                    "prompt": "Design a pricing card variant",
                    "count": 3,
                    "runtime": "claude"
                }),
            )
            .expect("side_by_side spawn should succeed");

        let runs = res.get("runs").and_then(Value::as_array).unwrap();
        assert_eq!(runs.len(), 3);
        let frames_after = srv.engine.doc.get(page_id).unwrap().children.len();
        assert_eq!(frames_after - frames_before, 3);
        assert_eq!(srv.lease_guard.table.leases.len(), 3);

        let mut leased_frames = BTreeSet::new();
        for lease in &srv.lease_guard.table.leases {
            assert_eq!(lease.frames.len(), 1);
            assert_eq!(lease.purpose, LeasePurpose::SideBySide);
            leased_frames.insert(lease.frames[0]);
        }
        assert_eq!(leased_frames.len(), 3);
        let _ = fs::remove_dir_all(&session_dir);
    }

    #[test]
    fn failed_run_rolls_back() {
        let session_dir = temp_session_dir("failed-rollback");
        let mut srv = Session::with_session_dir(&session_dir, &session_dir).unwrap();
        srv.bridge.lead_run_id = Some("lead".to_string());

        let page_id = srv.engine.doc.pages[0];
        let frame_a = srv.engine.doc.alloc_id();
        let frame_b = srv.engine.doc.alloc_id();
        srv.engine.doc.insert(page_id, Node::frame(frame_a, Rect::new(0.0, 0.0, 400.0, 300.0))).unwrap();
        srv.engine.doc.insert(page_id, Node::frame(frame_b, Rect::new(500.0, 0.0, 400.0, 300.0))).unwrap();

        srv.lease_guard.table.grant(Lease::new("agent-a", vec![frame_a], LeasePurpose::Split)).unwrap();
        srv.lease_guard.table.grant(Lease::new("agent-b", vec![frame_b], LeasePurpose::Split)).unwrap();
        srv.lease_guard.save().unwrap();

        // Agent A writes 2 batches into frame_a; Agent B writes 5 batches into frame_b.
        srv.bridge.run_id = Some("agent-a".to_string());
        for i in 0..2 {
            srv.call(
                "designos_create_node",
                json!({
                    "parent_id": frame_a.0,
                    "kind": "rectangle",
                    "name": format!("a-{i}"),
                    "x": 0.0,
                    "y": 0.0,
                    "width": 20.0,
                    "height": 20.0
                }),
            )
            .unwrap();
        }

        srv.bridge.run_id = Some("agent-b".to_string());
        for i in 0..5 {
            srv.call(
                "designos_create_node",
                json!({
                    "parent_id": frame_b.0,
                    "kind": "rectangle",
                    "name": format!("b-{i}"),
                    "x": 0.0,
                    "y": 0.0,
                    "width": 20.0,
                    "height": 20.0
                }),
            )
            .unwrap();
        }
        assert_eq!(srv.engine.doc.get(frame_a).unwrap().children.len(), 2);
        assert_eq!(srv.engine.doc.get(frame_b).unwrap().children.len(), 5);

        // Agent B fails / is stopped with rollback = true -> all 5 batches reverted automatically, Agent A untouched.
        srv.bridge.run_id = Some("lead".to_string());
        let stop_res = srv
            .call(
                "designos_stop_agents",
                json!({
                    "run_ids": ["agent-b"],
                    "rollback": true
                }),
            )
            .unwrap();
        assert_eq!(stop_res.get("rolled_back_steps").and_then(Value::as_u64), Some(5));
        assert_eq!(srv.engine.doc.get(frame_b).unwrap().children.len(), 0);
        assert_eq!(srv.engine.doc.get(frame_a).unwrap().children.len(), 2);
        let _ = fs::remove_dir_all(&session_dir);
    }

    #[test]
    fn authorship_recorded() {
        let session_dir = temp_session_dir("authorship");
        let mut srv = Session::with_session_dir(&session_dir, &session_dir).unwrap();

        let page_id = srv.engine.doc.pages[0];
        let frame_a = srv.engine.doc.alloc_id();
        srv.engine.doc.insert(page_id, Node::frame(frame_a, Rect::new(0.0, 0.0, 400.0, 300.0))).unwrap();

        srv.lease_guard.table.grant(Lease::new("agent-a", vec![frame_a], LeasePurpose::Split)).unwrap();
        srv.lease_guard.save().unwrap();
        srv.bridge.run_id = Some("agent-a".to_string());

        let created = srv
            .call(
                "designos_create_node",
                json!({
                    "parent_id": frame_a.0,
                    "kind": "rectangle",
                    "name": "StampedCard",
                    "x": 10.0,
                    "y": 10.0,
                    "width": 100.0,
                    "height": 50.0
                }),
            )
            .unwrap();
        let nid = NodeId(created.get("node").and_then(|n| n.get("id")).and_then(Value::as_u64).unwrap());
        let node = srv.engine.doc.get(nid).unwrap();
        assert_eq!(node.plugin_data.get(PLUGIN_DATA_RUN_KEY).map(String::as_str), Some("agent-a"));
        let _ = fs::remove_dir_all(&session_dir);
    }

    #[test]
    fn error_codes_not_lead_and_too_many_agents_and_frame_leased() {
        let session_dir = temp_session_dir("err-codes");
        let mut srv = Session::with_session_dir(&session_dir, &session_dir).unwrap();
        srv.bridge.lead_run_id = Some("lead".to_string());

        let page_id = srv.engine.doc.pages[0];
        let f1 = srv.engine.doc.alloc_id();
        srv.engine.doc.insert(page_id, Node::frame(f1, Rect::new(0.0, 0.0, 400.0, 300.0))).unwrap();

        // Sub-agent calling spawn_agents -> NOT_LEAD
        srv.bridge.run_id = Some("agent-1".to_string());
        let err = srv
            .call(
                "designos_spawn_agents",
                json!({
                    "mode": "side_by_side",
                    "prompt": "try nested spawn",
                    "count": 2
                }),
            )
            .unwrap_err();
        assert!(err.contains("NOT_LEAD"), "expected NOT_LEAD, got: {err}");

        // Lead calling side_by_side with count > 8 -> TOO_MANY_AGENTS
        srv.bridge.run_id = Some("lead".to_string());
        let err = srv
            .call(
                "designos_spawn_agents",
                json!({
                    "mode": "side_by_side",
                    "prompt": "too many",
                    "count": 9
                }),
            )
            .unwrap_err();
        assert!(err.contains("TOO_MANY_AGENTS"), "expected TOO_MANY_AGENTS, got: {err}");

        // Grant f1 to agent-1, then try to assign f1 to agent-2 without revoking -> FRAME_LEASED
        srv.lease_guard.table.grant(Lease::new("agent-1", vec![f1], LeasePurpose::Split)).unwrap();
        srv.lease_guard.save().unwrap();
        let err = srv
            .call(
                "designos_assign_frames",
                json!({
                    "assignments": [
                        { "run_id": "agent-2", "frame_ids": [f1.0] }
                    ]
                }),
            )
            .unwrap_err();
        assert!(err.contains("FRAME_LEASED"), "expected FRAME_LEASED, got: {err}");
        let _ = fs::remove_dir_all(&session_dir);
    }
}
