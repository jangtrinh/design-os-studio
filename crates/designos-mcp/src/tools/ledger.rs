//! Session-scoped idempotency ledger for mutating MCP tools (E15-6).
//!
//! Stores `(key, tool, hash, undo_depth, auto_fix_undo_depth, dead, live, gone, reply, pre_auto_fix_reply, created_ms)`
//! per document session, persists entries and undo/redo state transitions to
//! `<session_dir>/idempotency-ledger.jsonl`, and verifies both undo-stack state and live node
//! presence before replaying a stored response.

use crate::tools::errors::{ErrorCode, ToolError};
use designos_doc::{Document, Node, NodeId};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Number, Value};
use std::collections::{BTreeSet, HashMap, VecDeque};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Ledger filename inside `<session_dir>` (§3.4).
pub const LEDGER_FILE_NAME: &str = "idempotency-ledger.jsonl";

/// Maximum number of entries retained in the session ledger (§3.5).
pub const MAX_LEDGER_ENTRIES: usize = 5_000;

/// Maximum cumulative byte size of stored replies in the session ledger (16 MiB, §3.5).
pub const MAX_LEDGER_REPLY_BYTES: usize = 16 * 1024 * 1024;

/// A node expected to exist in the open document for a stored write to remain valid (§3.7, §4).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LedgerNode {
    pub id: u64,
    pub kind: String,
}

/// One stored write in the session idempotency ledger (§4).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LedgerEntry {
    pub key: String,
    pub tool: String,
    pub hash: u64,
    pub undo_depth: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_fix_undo_depth: Option<usize>,
    #[serde(default)]
    pub dead: bool,
    #[serde(default)]
    pub live: Vec<LedgerNode>,
    #[serde(default)]
    pub gone: Vec<u64>,
    pub reply: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pre_auto_fix_reply: Option<Value>,
    pub created_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct LedgerStateTransition {
    key: String,
    state: String,
}

/// Outcome of `Ledger::lookup(key, current_undo_depth, &Document)` (§4).
#[derive(Clone, Debug, PartialEq)]
pub enum LedgerLookup<'a> {
    Valid { entry: &'a LedgerEntry, reply: &'a Value },
    Stale { entry: &'a LedgerEntry },
}

/// Decision returned before dispatching a mutating tool call (§3.3).
#[derive(Clone, Debug, PartialEq)]
pub enum CallLedgerDecision {
    /// Key is not in the ledger; execute the call and store on success.
    Absent,
    /// Key matches tool and payload hash, write is still applied and present in the document.
    Replay(Value),
    /// Key matches tool and payload hash, but the write was undone, killed by a new branch, or its nodes are missing.
    Stale,
    /// Key was already used for a different tool or payload hash.
    Reused { original_tool: String },
}

/// Returns `true` when `name` is a mutating write tool that accepts `idempotency_key` (§3.1).
pub fn is_idempotent_write_tool(name: &str) -> bool {
    matches!(
        name,
        "designos_create_node"
            | "designos_update_node"
            | "designos_delete_node"
            | "designos_apply_spec"
            | "designos_apply_batch"
            | "designos_add_comment"
            | "designos_reply_comment"
            | "designos_resolve_comment"
            | "designos_set_dev_status"
    )
}

/// Validates an `idempotency_key` string: `1..=128` ASCII characters in `[A-Za-z0-9._:-]` (§3.1).
pub fn is_valid_idempotency_key(key: &str) -> bool {
    !key.is_empty() && key.len() <= 128 && key.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b':' | b'-'))
}

/// Validates an optional `idempotency_key` field inside tool argument structs.
pub fn validate_idempotency_key_opt(key: &Option<String>) -> Result<(), ToolError> {
    if let Some(k) = key {
        validate_idempotency_key_str(k)?;
    }
    Ok(())
}

/// Validates a non-optional `idempotency_key` string and returns a structured [`ToolError`] on failure.
pub fn validate_idempotency_key_str(key: &str) -> Result<(), ToolError> {
    if key.len() > 128 {
        return Err(ToolError::new(ErrorCode::ValueOutOfRange, "/idempotency_key", "idempotency_key must be 1..=128 characters in [A-Za-z0-9._:-]")
            .with_received(json!(key.len()))
            .with_expected("1..=128 characters in [A-Za-z0-9._:-]")
            .with_corrected_example(json!({"idempotency_key": "run-1"})));
    }
    if !is_valid_idempotency_key(key) {
        return Err(ToolError::new(ErrorCode::InvalidArguments, "/idempotency_key", "idempotency_key must be 1..=128 characters in [A-Za-z0-9._:-]")
            .with_received(json!(key))
            .with_expected("1..=128 characters in [A-Za-z0-9._:-]")
            .with_corrected_example(json!({"idempotency_key": "run-1"})));
    }
    Ok(())
}

/// Extracts and removes `idempotency_key` from `args` (if `args` is a JSON object), validating its syntax (§3.1–§3.2).
pub fn extract_and_strip_idempotency_key(args: &mut Value) -> Result<Option<String>, ToolError> {
    let Some(map) = args.as_object_mut() else {
        return Ok(None);
    };
    let Some(raw) = map.remove("idempotency_key") else {
        return Ok(None);
    };
    match raw {
        Value::Null => Ok(None),
        Value::String(s) => {
            validate_idempotency_key_str(&s)?;
            Ok(Some(s))
        }
        other => Err(ToolError::new(ErrorCode::InvalidArguments, "/idempotency_key", "idempotency_key must be a string")
            .with_received(other)
            .with_expected("1..=128 characters in [A-Za-z0-9._:-]")
            .with_corrected_example(json!({"idempotency_key": "run-1"}))),
    }
}

/// Returns `key` suffixed with `-2` (clamped to 128 bytes) for `corrected_example` (§5).
pub fn suffixed_idempotency_key(key: &str) -> String {
    if key.len() <= 126 {
        format!("{key}-2")
    } else {
        format!("{}-2", &key[..126])
    }
}

/// Returns a copy of `raw_args` with `"idempotency_key"` replaced by `suffixed_idempotency_key(key)` (§5).
pub fn corrected_example_with_new_key(raw_args: &Value, key: &str) -> Value {
    let mut cloned = raw_args.clone();
    if let Some(map) = cloned.as_object_mut() {
        map.insert("idempotency_key".into(), Value::String(suffixed_idempotency_key(key)));
    }
    cloned
}

/// Normalizes a JSON [`Value`] so object keys are sorted (`serde_json::Map` is `BTreeMap`),
/// `-0.0` normalizes to `0`, and integer-valued finite floats (`10.0`) normalize to integers (`10`).
pub fn normalize_canonical_value(val: &Value) -> Value {
    match val {
        Value::Null | Value::Bool(_) | Value::String(_) => val.clone(),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                return Value::Number(Number::from(i));
            }
            if let Some(u) = n.as_u64() {
                return Value::Number(Number::from(u));
            }
            if let Some(f) = n.as_f64() {
                if f == 0.0 {
                    return Value::Number(Number::from(0i64));
                }
                if f.is_finite() && f.fract() == 0.0 && f >= (i64::MIN as f64) && f < (i64::MAX as f64) {
                    return Value::Number(Number::from(f as i64));
                }
            }
            Value::Number(n.clone())
        }
        Value::Array(items) => Value::Array(items.iter().map(normalize_canonical_value).collect()),
        Value::Object(map) => {
            let mut out = Map::new();
            for (k, v) in map {
                if k == "idempotency_key" {
                    continue;
                }
                out.insert(k.clone(), normalize_canonical_value(v));
            }
            Value::Object(out)
        }
    }
}

/// Computes the 64-bit FNV-1a hash over `tool` + canonical JSON of `args_without_key` (§3.2).
pub fn payload_hash(tool: &str, args_without_key: &Value) -> u64 {
    let normalized = normalize_canonical_value(args_without_key);
    let canonical = serde_json::to_vec(&normalized).unwrap_or_default();
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in tool.as_bytes().iter().chain(std::iter::once(&b':')).chain(canonical.iter()) {
        h = (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3);
    }
    h
}

/// Stable lowercase kind name for `LedgerNode` comparisons (§3.7).
pub fn node_kind_tag(node: &Node) -> String {
    node.kind.default_name().to_ascii_lowercase()
}

/// Extracts `(live, gone)` node sets after a mutating tool commits (§3.7).
///
/// - `live`: every node ID referenced by `reply.created`, `reply.name_to_id` values,
///   `reply.root_node_id`, `reply.node.id`, and (for non-delete tools) `reply.node_id` /
///   `args_without_key.node_id` that exists in `doc_after` (excluding nodes created and
///   deleted within the same batch).
/// - `gone`: every node ID that existed in `pre_node_ids` before the call and no longer
///   exists in `doc_after` (excluding nodes created and deleted within the same batch).
pub fn extract_live_and_gone(
    tool: &str,
    args_without_key: &Value,
    reply: &Value,
    pre_node_ids: &BTreeSet<u64>,
    doc_after: &Document,
) -> (Vec<LedgerNode>, Vec<u64>) {
    let mut candidate_live_ids = BTreeSet::new();

    if let Some(arr) = reply.get("created").and_then(Value::as_array) {
        for v in arr {
            if let Some(id) = v.as_u64() {
                candidate_live_ids.insert(id);
            }
        }
    }
    if let Some(map) = reply.get("name_to_id").and_then(Value::as_object) {
        for v in map.values() {
            if let Some(id) = v.as_u64() {
                candidate_live_ids.insert(id);
            }
        }
    }
    if let Some(id) = reply.get("root_node_id").and_then(Value::as_u64) {
        candidate_live_ids.insert(id);
    }
    if let Some(id) = reply.get("node").and_then(|n| n.get("id")).and_then(Value::as_u64) {
        candidate_live_ids.insert(id);
    }
    if tool != "designos_delete_node" {
        if let Some(id) = reply.get("node_id").and_then(Value::as_u64) {
            candidate_live_ids.insert(id);
        }
        if let Some(id) = args_without_key.get("node_id").and_then(Value::as_u64) {
            candidate_live_ids.insert(id);
        }
    }

    let mut live = Vec::new();
    for id in candidate_live_ids {
        if let Some(node) = doc_after.get(NodeId(id)) {
            live.push(LedgerNode { id, kind: node_kind_tag(node) });
        }
    }

    let mut gone: Vec<u64> = pre_node_ids.iter().copied().filter(|&id| doc_after.get(NodeId(id)).is_none()).collect();
    if tool == "designos_delete_node" {
        if let Some(id) = args_without_key.get("node_id").and_then(Value::as_u64).or_else(|| reply.get("node_id").and_then(Value::as_u64)) {
            if doc_after.get(NodeId(id)).is_none() && !gone.contains(&id) {
                gone.push(id);
                gone.sort_unstable();
            }
        }
    }

    (live, gone)
}

/// Checks whether `doc` still satisfies the `live` and `gone` invariants of `entry` (§3.7).
pub fn entry_matches_document(entry: &LedgerEntry, doc: &Document) -> bool {
    for ln in &entry.live {
        let Some(node) = doc.get(NodeId(ln.id)) else {
            return false;
        };
        if node_kind_tag(node) != ln.kind {
            return false;
        }
    }
    for &gone_id in &entry.gone {
        if doc.get(NodeId(gone_id)).is_some() {
            return false;
        }
    }
    true
}

fn entry_reply_bytes(entry: &LedgerEntry) -> usize {
    let r1 = serde_json::to_vec(&entry.reply).map(|v| v.len()).unwrap_or(0);
    let r2 = entry.pre_auto_fix_reply.as_ref().and_then(|v| serde_json::to_vec(v).ok()).map(|v| v.len()).unwrap_or(0);
    r1.saturating_add(r2)
}

pub fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// Session-scoped idempotency ledger (§3.4–§3.7, §4).
#[derive(Clone, Debug)]
pub struct Ledger {
    entries: HashMap<String, LedgerEntry>,
    order: VecDeque<String>,
    total_reply_bytes: usize,
    max_entries: usize,
    max_reply_bytes: usize,
    dropped_count: u64,
    corrupt_lines: u64,
    path: Option<PathBuf>,
}

impl Default for Ledger {
    fn default() -> Self {
        Self::new()
    }
}

impl Ledger {
    /// Creates an in-memory [`Ledger`] with default capacity limits (`5,000` entries / `16 MiB`).
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
            order: VecDeque::new(),
            total_reply_bytes: 0,
            max_entries: MAX_LEDGER_ENTRIES,
            max_reply_bytes: MAX_LEDGER_REPLY_BYTES,
            dropped_count: 0,
            corrupt_lines: 0,
            path: None,
        }
    }

    /// Loads `<path>` if it exists and binds this [`Ledger`] to append future entries and transitions to `<path>` (§3.4).
    pub fn load_from_path(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let mut ledger = Self::new();
        if let Ok(raw) = fs::read_to_string(&path) {
            for line in raw.lines() {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                let Ok(val) = serde_json::from_str::<Value>(trimmed) else {
                    ledger.corrupt_lines += 1;
                    continue;
                };
                if val.get("tool").is_none() && val.get("state").is_some() {
                    match serde_json::from_value::<LedgerStateTransition>(val) {
                        Ok(tr) if tr.state == "stale" => {
                            if let Some(entry) = ledger.entries.get_mut(&tr.key) {
                                entry.dead = true;
                            }
                        }
                        Ok(tr) if tr.state == "valid" => {
                            if let Some(entry) = ledger.entries.get_mut(&tr.key) {
                                entry.dead = false;
                            }
                        }
                        _ => {
                            ledger.corrupt_lines += 1;
                        }
                    }
                } else {
                    match serde_json::from_value::<LedgerEntry>(val) {
                        Ok(mut entry) => {
                            // Earlier process's in-memory undo stack is gone on reload (§3.6):
                            // loaded entries have `undo_depth = None` and follow presence + stale state.
                            entry.undo_depth = None;
                            entry.auto_fix_undo_depth = None;
                            ledger.insert_in_memory(entry);
                        }
                        Err(_) => {
                            ledger.corrupt_lines += 1;
                        }
                    }
                }
            }
        }
        ledger.path = Some(path);
        ledger
    }

    /// Overrides the entry-count and reply-bytes capacity limits (used by unit tests and diagnostics).
    pub fn with_limits(mut self, max_entries: usize, max_reply_bytes: usize) -> Self {
        self.max_entries = max_entries.max(1);
        self.max_reply_bytes = max_reply_bytes.max(1);
        self.evict_if_needed();
        self
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn total_reply_bytes(&self) -> usize {
        self.total_reply_bytes
    }

    pub fn max_entries(&self) -> usize {
        self.max_entries
    }

    pub fn max_reply_bytes(&self) -> usize {
        self.max_reply_bytes
    }

    pub fn dropped_count(&self) -> u64 {
        self.dropped_count
    }

    pub fn corrupt_lines(&self) -> u64 {
        self.corrupt_lines
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    pub fn get(&self, key: &str) -> Option<&LedgerEntry> {
        self.entries.get(key)
    }

    /// Looks up `key` in the ledger and evaluates undo-depth and document-presence rules (§4).
    pub fn lookup<'a>(&'a self, key: &str, current_undo_depth: usize, doc: &Document) -> Option<LedgerLookup<'a>> {
        let entry = self.entries.get(key)?;
        if entry.dead {
            return Some(LedgerLookup::Stale { entry });
        }
        match entry.undo_depth {
            Some(depth) => {
                if current_undo_depth < depth {
                    return Some(LedgerLookup::Stale { entry });
                }
            }
            None => {
                // Reloaded entry with no checkable IDs cannot verify presence (§3.7).
                if entry.live.is_empty() && entry.gone.is_empty() {
                    return Some(LedgerLookup::Stale { entry });
                }
            }
        }
        if !entry_matches_document(entry, doc) {
            return Some(LedgerLookup::Stale { entry });
        }
        let reply = if entry.auto_fix_undo_depth.is_some_and(|af_depth| current_undo_depth < af_depth) {
            entry.pre_auto_fix_reply.as_ref().unwrap_or(&entry.reply)
        } else {
            &entry.reply
        };
        Some(LedgerLookup::Valid { entry, reply })
    }

    /// Evaluates a mutating tool call's `(key, tool, hash)` before dispatch (§3.3).
    pub fn check_call(&self, key: &str, tool: &str, hash: u64, current_undo_depth: usize, doc: &Document) -> CallLedgerDecision {
        let Some(existing) = self.entries.get(key) else {
            return CallLedgerDecision::Absent;
        };
        if existing.tool != tool || existing.hash != hash {
            return CallLedgerDecision::Reused { original_tool: existing.tool.clone() };
        }
        match self.lookup(key, current_undo_depth, doc) {
            Some(LedgerLookup::Valid { reply, .. }) => CallLedgerDecision::Replay(reply.clone()),
            Some(LedgerLookup::Stale { .. }) => CallLedgerDecision::Stale,
            None => CallLedgerDecision::Absent,
        }
    }

    /// Checks only whether `key` is already stored with a different `tool` or `hash` (for `dry_run` calls, §8.5).
    pub fn check_mismatch(&self, key: &str, tool: &str, hash: u64) -> Option<String> {
        let existing = self.entries.get(key)?;
        if existing.tool != tool || existing.hash != hash {
            Some(existing.tool.clone())
        } else {
            None
        }
    }

    /// Stores `entry` in memory (evicting oldest entries if over capacity) and appends one JSON line + fsync to disk (§3.4–§3.5).
    /// Image content (`"content"`) is stripped before persistence per §8.6.
    pub fn store(&mut self, mut entry: LedgerEntry) {
        if let Some(obj) = entry.reply.as_object_mut() {
            obj.remove("content");
        }
        if let Some(pre) = entry.pre_auto_fix_reply.as_mut().and_then(Value::as_object_mut) {
            pre.remove("content");
        }
        self.append_json_line(&entry);
        self.insert_in_memory(entry);
    }

    fn insert_in_memory(&mut self, entry: LedgerEntry) {
        let key = entry.key.clone();
        let bytes = entry_reply_bytes(&entry);
        if let Some(prev) = self.entries.remove(&key) {
            let prev_bytes = entry_reply_bytes(&prev);
            self.total_reply_bytes = self.total_reply_bytes.saturating_sub(prev_bytes);
            self.order.retain(|k| k != &key);
        }
        self.total_reply_bytes = self.total_reply_bytes.saturating_add(bytes);
        self.order.push_back(key.clone());
        self.entries.insert(key, entry);
        self.evict_if_needed();
    }

    fn evict_if_needed(&mut self) {
        while self.entries.len() > self.max_entries || (self.entries.len() > 1 && self.total_reply_bytes > self.max_reply_bytes) {
            let Some(oldest_key) = self.order.pop_front() else {
                break;
            };
            if let Some(removed) = self.entries.remove(&oldest_key) {
                let removed_bytes = entry_reply_bytes(&removed);
                self.total_reply_bytes = self.total_reply_bytes.saturating_sub(removed_bytes);
                self.dropped_count += 1;
            }
        }
    }

    /// Marks every entry whose `undo_depth` is strictly above `depth` as permanently dead (`dead = true`)
    /// when a new undoable write discards the redo branch (§3.6, §4).
    pub fn mark_dead_above(&mut self, depth: usize) {
        let mut newly_stale = Vec::new();
        for key in &self.order {
            if let Some(entry) = self.entries.get_mut(key) {
                if !entry.dead && entry.undo_depth.is_some_and(|d| d > depth) {
                    entry.dead = true;
                    newly_stale.push(entry.key.clone());
                } else if !entry.dead && entry.auto_fix_undo_depth.is_some_and(|af| af > depth) {
                    if let Some(pre) = entry.pre_auto_fix_reply.take() {
                        entry.reply = pre;
                    }
                    entry.auto_fix_undo_depth = None;
                }
            }
        }
        for key in newly_stale {
            self.append_transition(&key, "stale");
        }
    }

    /// Appends `{"key": K, "state": "stale"}` transitions when `designos_undo` moves `current_undo_depth`
    /// below an entry's `undo_depth` (§3.6).
    pub fn on_undo(&mut self, current_undo_depth: usize) {
        let mut stale_keys = Vec::new();
        for key in &self.order {
            if let Some(entry) = self.entries.get(key) {
                if !entry.dead && entry.undo_depth.is_some_and(|d| current_undo_depth < d) {
                    stale_keys.push(entry.key.clone());
                }
            }
        }
        for key in stale_keys {
            self.append_transition(&key, "stale");
        }
    }

    /// Appends `{"key": K, "state": "valid"}` transitions when `designos_redo` restores `current_undo_depth`
    /// back to or above an entry's `undo_depth` (§3.6).
    pub fn on_redo(&mut self, current_undo_depth: usize) {
        let mut valid_keys = Vec::new();
        for key in &self.order {
            if let Some(entry) = self.entries.get(key) {
                if !entry.dead && entry.undo_depth.is_some_and(|d| current_undo_depth >= d) {
                    valid_keys.push(entry.key.clone());
                }
            }
        }
        for key in valid_keys {
            self.append_transition(&key, "valid");
        }
    }

    /// Reconciles the ledger after Studio writes a newer `canvas.designos` snapshot (§3.4, §3.6, §3.7).
    ///
    /// Studio republishes the canvas after it applies each of the agent's edits, so a reload must
    /// not clear the ledger while the agent's writes are still in the document.
    ///
    /// - A checkable applied entry has `!entry.dead && entry.undo_depth.is_none_or(|d| current_undo_depth >= d)`
    ///   and `!entry.live.is_empty() || !entry.gone.is_empty()`.
    /// - An applied entry whose result is missing from `doc` and whose `created_ms` is at or after
    ///   `canvas_mtime_ms` is pending: the snapshot was written before the agent's edit was
    ///   appended to the edit log, so it cannot hold the edit yet. A pending entry is kept as it
    ///   is (neither forgotten nor stale) and ignored by the rules below; the next snapshot,
    ///   written after the editor applied the edit, decides it. No time margin is used: a real
    ///   Studio undo happens after Studio read the edit, so its snapshot is newer than the entry.
    /// - If the canvas is not a scoped copy, there is at least one checkable applied entry (pending
    ///   ones aside) and every one fails `entry_matches_document(entry, doc)`, the canvas is a different document: the applied
    ///   entries are dropped from the ledger and rewritten out of its file. Entries the agent undid,
    ///   entries with nothing to check and entries already stale for good turn or stay stale, so
    ///   an undone write is not resurrected. Pending entries stay as they are.
    /// - Otherwise the ledger file is kept and the in-memory `undo_depth` is detached (`History` is
    ///   reset on canvas reload). Entries undone before the reload, or without checkable ids, turn
    ///   stale for good (`entry.dead = true` + `"stale"` transition).
    /// - A checkable applied entry whose result is missing from `doc` is a Studio-side undo, a layer
    ///   the user deleted, or something the canvas leaves out. Studio's undo takes back the newest
    ///   writes first, so when no later entry's result is still in `doc` and the canvas is not a
    ///   scoped copy, the entry is forgotten: the retry runs as a new write (§3.6, owner ruling
    ///   2026-10-19). That includes a user deleting the agent's newest layer by hand, which cannot
    ///   be told apart from an undo. Any other missing entry (a later write is still present, or
    ///   the canvas is scoped) turns stale: the editor deleted or left out that layer on its own.
    pub fn on_canvas_reload(&mut self, current_undo_depth: usize, doc: &Document, scoped_canvas: bool, canvas_mtime_ms: u64) {
        if self.entries.is_empty() {
            return;
        }
        let is_applied = |e: &LedgerEntry| !e.dead && e.undo_depth.is_none_or(|d| current_undo_depth >= d);
        let is_checkable = |e: &LedgerEntry| !e.live.is_empty() || !e.gone.is_empty();
        let is_pending = |e: &LedgerEntry| is_applied(e) && is_checkable(e) && canvas_mtime_ms <= e.created_ms && !entry_matches_document(e, doc);
        let applied_checkable = self.entries.values().filter(|e| is_applied(e) && is_checkable(e) && !is_pending(e)).count();
        let present_checkable = self.entries.values().filter(|e| is_applied(e) && is_checkable(e) && entry_matches_document(e, doc)).count();
        // A scoped copy leaves layers out on purpose, so absence there never means another document.
        let different_document = !scoped_canvas && applied_checkable > 0 && present_checkable == 0;
        // Position (in write order) of the newest entry whose result is still in the document.
        let newest_present =
            self.order.iter().rposition(|key| self.entries.get(key).is_some_and(|e| is_applied(e) && is_checkable(e) && entry_matches_document(e, doc)));
        let mut newly_stale = Vec::new();
        let mut forgotten = Vec::new();
        for (pos, key) in self.order.iter().enumerate() {
            let Some(entry) = self.entries.get_mut(key) else { continue };
            let undone_before_reload = entry.undo_depth.is_some_and(|d| current_undo_depth < d);
            let uncheckable = !is_checkable(entry);
            let presence_failed = !uncheckable && !entry_matches_document(entry, doc);
            if !entry.dead && !is_pending(entry) {
                if undone_before_reload || uncheckable {
                    // Undone by the agent, or nothing to check: stale for good, even on a different document.
                    entry.dead = true;
                    newly_stale.push(entry.key.clone());
                } else if different_document || (presence_failed && !scoped_canvas && newest_present.is_none_or(|newest| pos > newest)) {
                    forgotten.push(entry.key.clone());
                } else if presence_failed {
                    entry.dead = true;
                    newly_stale.push(entry.key.clone());
                }
            }
            entry.undo_depth = None;
            entry.auto_fix_undo_depth = None;
        }
        for key in &newly_stale {
            self.append_transition(key, "stale");
        }
        if !forgotten.is_empty() {
            self.forget(&forgotten);
        }
    }

    /// Removes `keys` for good, in memory and in the ledger file, so each behaves as unknown.
    fn forget(&mut self, keys: &[String]) {
        for key in keys {
            if let Some(removed) = self.entries.remove(key) {
                self.total_reply_bytes = self.total_reply_bytes.saturating_sub(entry_reply_bytes(&removed));
            }
        }
        self.order.retain(|k| !keys.contains(k));
        self.rewrite_file();
    }

    /// Rewrites `<session_dir>/idempotency-ledger.jsonl` from the entries held in memory (state
    /// transitions are folded into each entry's `dead` flag). Like every append, best effort.
    fn rewrite_file(&self) {
        let Some(path) = &self.path else {
            return;
        };
        let mut body = String::new();
        for entry in self.order.iter().filter_map(|k| self.entries.get(k)) {
            if let Ok(line) = serde_json::to_string(entry) {
                body.push_str(&line);
                body.push('\n');
            }
        }
        let tmp = path.with_extension(format!("tmp-{}", std::process::id()));
        let written = fs::File::create(&tmp).and_then(|mut f| f.write_all(body.as_bytes()).and_then(|_| f.sync_all()));
        if written.is_err() || fs::rename(&tmp, path).is_err() {
            let _ = fs::remove_file(&tmp);
        }
    }

    /// Clears all in-memory entries and truncates `<session_dir>/idempotency-ledger.jsonl` if present (§3.4, §4).
    pub fn clear(&mut self) {
        self.entries.clear();
        self.order.clear();
        self.total_reply_bytes = 0;
        if let Some(path) = &self.path {
            if path.exists() {
                if let Ok(file) = fs::OpenOptions::new().write(true).truncate(true).open(path) {
                    let _ = file.sync_all();
                }
            }
        }
    }

    fn append_transition(&self, key: &str, state: &str) {
        let tr = LedgerStateTransition { key: key.to_string(), state: state.to_string() };
        self.append_json_line(&tr);
    }

    fn append_json_line<T: Serialize>(&self, record: &T) {
        let Some(path) = &self.path else {
            return;
        };
        let Ok(mut line) = serde_json::to_string(record) else {
            return;
        };
        line.push('\n');
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Ok(mut file) = fs::OpenOptions::new().create(true).append(true).open(path) {
            if file.write_all(line.as_bytes()).is_ok() {
                let _ = file.sync_all();
            }
        }
    }
}

/// Alias type for callers referencing `IdempotencyCache`.
pub type IdempotencyCache = Ledger;
