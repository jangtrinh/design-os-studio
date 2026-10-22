//! MCP write guard for RD-3 frame ownership leases (`docs/specs/v0.6/RD-3-parallel-agents-frame-ownership.md` §3.2).

use designos_doc::live_patch::{root_frame_of, AffectedSet, Lease, LeasePurpose, LeaseTable, LeaseViolation};
use designos_doc::{Document, History, NodeId};
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Structured MCP error JSON string matching the §6 error taxonomy.
pub fn structured_error(code: &str, message: impl Into<String>, path: &str, valid_options: Option<Vec<String>>, safe_to_retry: bool) -> String {
    let msg = message.into();
    let mut obj = serde_json::Map::new();
    obj.insert("code".into(), json!(code));
    obj.insert("message".into(), json!(msg));
    obj.insert("path".into(), json!(path));
    if let Some(opts) = valid_options {
        obj.insert("valid_options".into(), json!(opts));
    }
    obj.insert("safe_to_retry".into(), json!(safe_to_retry));
    serde_json::to_string(&Value::Object(obj)).unwrap_or(msg)
}

/// Formats a `LeaseViolation` into a structured `OUTSIDE_OWNED_FRAMES` or `FRAME_LEASED` JSON error string (§3.2, §6).
pub fn format_lease_violation(doc: &Document, violation: &LeaseViolation, path_override: &str) -> String {
    match violation {
        LeaseViolation::OutsideOwnedFrames { run_id, node_id, owner_run_id, valid_frames, path } => {
            let caller = if run_id.is_empty() { "unknown" } else { run_id.as_str() };
            let valid_options: Vec<String> = valid_frames.iter().map(|id| id.0.to_string()).collect();
            let frames_list = valid_options.join(", ");
            let eff_path = if !path_override.is_empty() { path_override } else { path.as_str() };
            let rf = node_id.and_then(|nid| root_frame_of(doc, nid));
            let message = match (*node_id, rf, owner_run_id) {
                (Some(target), Some(root_f), Some(owner)) => {
                    let fname = doc.get(root_f).map(|n| n.name.as_str()).unwrap_or("Frame");
                    format!(
                        "node {} is under frame {} ('{}'), which is owned by {}; this run ({}) may only write inside frames [{}]",
                        target.0, root_f.0, fname, owner, caller, frames_list
                    )
                }
                (Some(target), Some(root_f), None) => {
                    let fname = doc.get(root_f).map(|n| n.name.as_str()).unwrap_or("Frame");
                    format!(
                        "node {} is under frame {} ('{}'), which is outside this run's ({}) owned frames [{}]",
                        target.0, root_f.0, fname, caller, frames_list
                    )
                }
                (Some(target), None, Some(owner)) => {
                    format!("node {} is owned by {}; this run ({}) may only write inside frames [{}]", target.0, owner, caller, frames_list)
                }
                (Some(target), None, None) => {
                    format!("node {} is not inside any owned frame; this run ({}) may only write inside frames [{}]", target.0, caller, frames_list)
                }
                (None, _, _) => {
                    format!(
                        "document-global mutation at '{}' is not permitted for non-lead run ({}); this run may only write inside frames [{}]",
                        eff_path, caller, frames_list
                    )
                }
            };
            structured_error("OUTSIDE_OWNED_FRAMES", message, eff_path, Some(valid_options), true)
        }
        LeaseViolation::FrameLeased { frame_id, owner_run_id } => {
            let eff_path = if !path_override.is_empty() { path_override } else { "/frames" };
            structured_error("FRAME_LEASED", format!("frame {} is already leased to active run '{}'", frame_id.0, owner_run_id), eff_path, None, false)
        }
    }
}

/// Manages reloading `<session_dir>/leases.json` on mtime changes and evaluating write guards.
#[derive(Clone, Debug, Default)]
pub struct LeaseGuard {
    pub session_dir: Option<PathBuf>,
    pub table: LeaseTable,
    pub epoch: u64,
    pub last_mtime: Option<SystemTime>,
    /// True when `table` was explicitly set in-memory (e.g. in tests or before `leases.json` is written).
    pub in_memory_override: bool,
}

impl LeaseGuard {
    pub fn new(session_dir: Option<PathBuf>) -> Self {
        let mut g = Self { session_dir, table: LeaseTable::default(), epoch: 0, last_mtime: None, in_memory_override: false };
        g.reload_if_needed();
        g
    }

    pub fn leases_path(&self) -> Option<PathBuf> {
        self.session_dir.as_ref().map(|d| d.join("leases.json"))
    }

    /// Reloads `<session_dir>/leases.json` if its mtime has changed since the last read.
    pub fn reload_if_needed(&mut self) {
        let Some(path) = self.leases_path() else {
            return;
        };
        let Ok(meta) = fs::metadata(&path) else {
            if !self.in_memory_override {
                self.table = LeaseTable::default();
                self.last_mtime = None;
            }
            return;
        };
        let Ok(mtime) = meta.modified() else {
            return;
        };
        let should_reload = match self.last_mtime {
            Some(prev) => mtime > prev,
            None => true,
        };
        if should_reload {
            if let Ok(raw) = fs::read_to_string(&path) {
                if let Ok(parsed) = serde_json::from_str::<LeaseTable>(&raw) {
                    self.table = parsed;
                    self.last_mtime = Some(mtime);
                    self.in_memory_override = false;
                }
            }
        }
    }

    /// Saves `self.table` atomically to `<session_dir>/leases.json` if `session_dir` is configured.
    pub fn save(&mut self) -> Result<(), String> {
        let Some(path) = self.leases_path() else {
            self.in_memory_override = true;
            return Ok(());
        };
        write_leases_atomic(&path, &self.table)?;
        self.last_mtime = fs::metadata(&path).and_then(|m| m.modified()).ok();
        self.in_memory_override = false;
        Ok(())
    }

    /// Sets the lease table in memory (and persists to disk if `session_dir` is present).
    pub fn set_table(&mut self, table: LeaseTable) {
        self.table = table;
        self.epoch += 1;
        self.in_memory_override = true;
        let _ = self.save();
    }
}

/// Atomically writes `LeaseTable` to `path` via a temp file + rename.
pub fn write_leases_atomic(path: &Path, table: &LeaseTable) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let bytes = serde_json::to_vec_pretty(table).map_err(|e| format!("serialize leases.json: {e}"))?;
    let tmp = path.with_extension(format!("tmp-{}", std::process::id()));
    fs::write(&tmp, &bytes).map_err(|e| format!("write {}: {e}", tmp.display()))?;
    fs::rename(&tmp, path).map_err(|e| format!("rename {} -> {}: {e}", tmp.display(), path.display()))
}

/// Computes the effective `LeaseTable` by combining `guard_table` with any `owned_frames`
/// or `DESIGNOS_OWNED_FRAMES` override on `Engine`.
pub fn effective_lease_table(guard_table: &LeaseTable, engine_owned_frames: Option<&[NodeId]>, caller_run_id: Option<&str>) -> LeaseTable {
    if !guard_table.leases.is_empty() {
        return guard_table.clone();
    }
    let env_owned: Option<Vec<NodeId>> = std::env::var("DESIGNOS_OWNED_FRAMES")
        .ok()
        .map(|s| s.split(',').filter_map(|p| p.trim().parse::<u64>().ok()).filter(|&id| id > 0).map(NodeId).collect())
        .filter(|v: &Vec<NodeId>| !v.is_empty());

    if let Some(frames) = engine_owned_frames.or(env_owned.as_deref()) {
        let rid = caller_run_id.unwrap_or("agent").to_string();
        return LeaseTable::with_leases(vec![Lease::new(rid, frames.to_vec(), LeasePurpose::Split)]);
    }
    LeaseTable::default()
}

/// Checks whether a mutating MCP tool invocation is permitted under the active `LeaseTable` (§3.2).
///
/// Returns `Ok(())` if permitted, or `Err(json_error_string)` with `code: "OUTSIDE_OWNED_FRAMES"`
/// before any mutation takes place.
pub fn check_tool_write_guard(
    doc: &Document,
    history: &History,
    default_page: Option<NodeId>,
    table: &LeaseTable,
    caller_run_id: Option<&str>,
    tool_name: &str,
    args: &Value,
) -> Result<(), String> {
    if table.leases.is_empty() {
        return Ok(());
    }

    let caller = caller_run_id.unwrap_or("lead");

    // Special check for undo / redo (§3.2 item 3):
    // allowed only if the top undo/redo entry was authored by the calling `run_id`
    // (or if caller holds no restrictive lease and top entry has no run_id).
    if matches!(tool_name, "designos_undo" | "designos_redo") {
        let caller_lease = table.get(caller);
        let allowed_frames: Vec<String> = caller_lease
            .map(|l| l.frames.iter().map(|id| id.0.to_string()).collect())
            .unwrap_or_else(|| table.unleased_root_frames(doc).iter().map(|id| id.0.to_string()).collect());
        if tool_name == "designos_undo" {
            if let Some(meta) = history.undo_metas().last() {
                let top_owner = meta.run_id.as_deref();
                let ok = top_owner == Some(caller) || (top_owner.is_none() && caller_lease.is_none());
                if !ok {
                    return Err(structured_error(
                        "OUTSIDE_OWNED_FRAMES",
                        format!("top undo step belongs to '{}'; run '{}' may only undo its own steps", top_owner.unwrap_or("editor"), caller),
                        "",
                        Some(allowed_frames),
                        true,
                    ));
                }
            }
        }
        return Ok(());
    }

    let Some((affected, path)) = compute_tool_affected_set(doc, default_page, tool_name, args) else {
        return Ok(());
    };

    table.allows(caller, doc, &affected).map_err(|v| format_lease_violation(doc, &v, &path))
}

/// Computes the [`AffectedSet`] and JSON pointer path for a mutating MCP tool call (§3.2).
pub fn compute_tool_affected_set(doc: &Document, default_page: Option<NodeId>, tool_name: &str, args: &Value) -> Option<(AffectedSet, String)> {
    match tool_name {
        "designos_open_file" | "designos_new_document" => {
            let mut set = AffectedSet::new();
            set.add_shared_path("/document");
            Some((set, "/document".to_string()))
        }
        "designos_create_node" | "designos_apply_spec" => {
            let parent_id = args.get("parent_id").and_then(Value::as_u64).filter(|&id| id != 0).map(NodeId).or(default_page)?;
            Some((AffectedSet::for_nodes([parent_id]), "/parent_id".to_string()))
        }
        "designos_update_node" | "designos_delete_node" | "designos_set_dev_status" => {
            let node_id = args.get("node_id").and_then(Value::as_u64).filter(|&id| id != 0).map(NodeId)?;
            Some((AffectedSet::for_nodes([node_id]), "/node_id".to_string()))
        }
        "designos_add_comment" => {
            args.get("node_id").and_then(Value::as_u64).filter(|&id| id != 0).map(|nid| (AffectedSet::for_nodes([NodeId(nid)]), "/node_id".to_string()))
        }
        "designos_reply_comment" | "designos_resolve_comment" => {
            let tid = args.get("thread_id").and_then(Value::as_str)?;
            let thread = doc.comments.iter().find(|t| t.id == tid)?;
            let nid = thread.anchor.node?;
            Some((AffectedSet::for_nodes([nid]), "/thread_id".to_string()))
        }
        "designos_apply_batch" => {
            let ops = args.get("ops").and_then(Value::as_array)?;
            let mut set = AffectedSet::new();
            let mut first_path = "/ops/0".to_string();
            for (idx, op) in ops.iter().enumerate() {
                let op_type = op.get("op").and_then(Value::as_str).unwrap_or("");
                match op_type {
                    "set_variables" => {
                        let p = format!("/ops/{idx}");
                        if set.nodes.is_empty() && set.shared_paths.is_empty() {
                            first_path = p.clone();
                        }
                        set.add_shared_path("/variables");
                    }
                    "set_style" => {
                        let has_upsert = op.get("upsert").and_then(Value::as_array).is_some_and(|a| !a.is_empty());
                        let has_delete = op.get("delete").and_then(Value::as_array).is_some_and(|a| !a.is_empty());
                        if has_upsert || has_delete {
                            let p = format!("/ops/{idx}");
                            if set.nodes.is_empty() && set.shared_paths.is_empty() {
                                first_path = p;
                            }
                            set.add_shared_path("/styles");
                        }
                        if let Some(apply_arr) = op.get("apply").and_then(Value::as_array) {
                            for item in apply_arr {
                                if let Some(id) = item.get("node").or_else(|| item.get("node_id")).and_then(Value::as_u64).filter(|&i| i > 0) {
                                    if set.nodes.is_empty() && set.shared_paths.is_empty() {
                                        first_path = format!("/ops/{idx}/apply");
                                    }
                                    set.add_node(NodeId(id));
                                }
                            }
                        }
                    }
                    "apply_spec" | "insert" => {
                        let p_val = op.get("parent").or_else(|| op.get("parent_id"));
                        match p_val {
                            Some(Value::Number(n)) => {
                                let id = n.as_u64().unwrap_or(0);
                                let resolved = if id == 0 { default_page } else { Some(NodeId(id)) };
                                if let Some(pid) = resolved {
                                    if set.nodes.is_empty() && set.shared_paths.is_empty() {
                                        first_path = format!("/ops/{idx}/parent");
                                    }
                                    set.add_node(pid);
                                }
                            }
                            None => {
                                if let Some(pid) = default_page {
                                    if set.nodes.is_empty() && set.shared_paths.is_empty() {
                                        first_path = format!("/ops/{idx}/parent");
                                    }
                                    set.add_node(pid);
                                }
                            }
                            Some(Value::String(_)) => {}
                            _ => {}
                        }
                    }
                    "update" | "delete" => {
                        if let Some(id) = op.get("node").or_else(|| op.get("node_id")).and_then(Value::as_u64).filter(|&i| i > 0) {
                            if set.nodes.is_empty() && set.shared_paths.is_empty() {
                                first_path = format!("/ops/{idx}/node");
                            }
                            set.add_node(NodeId(id));
                        }
                    }
                    "move" => {
                        if let Some(id) = op.get("node").or_else(|| op.get("node_id")).and_then(Value::as_u64).filter(|&i| i > 0) {
                            if set.nodes.is_empty() && set.shared_paths.is_empty() {
                                first_path = format!("/ops/{idx}/node");
                            }
                            set.add_node(NodeId(id));
                        }
                        if let Some(pid) = op.get("parent").or_else(|| op.get("parent_id")).and_then(Value::as_u64).filter(|&i| i > 0) {
                            if set.nodes.is_empty() && set.shared_paths.is_empty() {
                                first_path = format!("/ops/{idx}/parent");
                            }
                            set.add_node(NodeId(pid));
                        }
                    }
                    _ => {}
                }
            }
            if set.nodes.is_empty() && set.shared_paths.is_empty() {
                None
            } else {
                Some((set, first_path))
            }
        }
        _ => None,
    }
}

/// Checks a single resolved `NodeId` inside `designos_apply_batch` against the active `LeaseTable`.
pub fn check_batch_node_lease(
    doc: &Document,
    table: &LeaseTable,
    caller_run_id: Option<&str>,
    target: NodeId,
    op_idx: usize,
    op_name: &str,
    field: &str,
) -> Result<(), String> {
    if table.leases.is_empty() {
        return Ok(());
    }
    let caller = caller_run_id.unwrap_or("lead");
    let affected = AffectedSet::for_nodes([target]);
    if let Err(violation) = table.allows(caller, doc, &affected) {
        let valid_options: Vec<String> = violation.valid_frames().iter().map(|id| id.0.to_string()).collect();
        let path = format!("/ops/{op_idx}/{field}");
        let inner_str = format_lease_violation(doc, &violation, &path);
        let inner_val: Value = serde_json::from_str(&inner_str).unwrap_or_else(|_| json!({ "code": "OUTSIDE_OWNED_FRAMES" }));
        let msg = inner_val.get("message").and_then(Value::as_str).unwrap_or("target node is outside this agent run's owned frames").to_string();
        let envelope = json!({
            "code": "OUTSIDE_OWNED_FRAMES",
            "failed_op": op_idx,
            "op": op_name,
            "path": path,
            "message": msg,
            "valid_options": valid_options,
            "safe_to_retry": true,
            "cause": inner_val,
        });
        return Err(serde_json::to_string(&envelope).unwrap_or(msg));
    }
    Ok(())
}
