//! Transactional batched agent write tool (`designos_apply_batch`, E15-4).
//!
//! Runs 1..=200 operations (`apply_spec`, `insert`, `update`, `move`, `delete`,
//! `set_variables`, `set_style`) against a single trial clone of the document,
//! resolving `@<as>` and `@<as>/<path>` references to earlier ops, and either
//! commits all operations as a single `Command::Batch` undo entry or leaves the
//! document and undo history untouched on any error or when `dry_run: true`.

use crate::engine::Engine;
use crate::tools::errors::{tool_output_schema_from_object, ErrorCode, ToolError};
use crate::tools::fill_write::{validate_fill_color, write_fill, FillTarget};
use crate::tools::{build_created_node, build_updated_node, validate_create_fields, validate_update_fields, UpdateFields};
use designos_doc::live_patch::{live_ops_for_command, LiveOp, SiblingHint};
use designos_doc::{
    apply_style_values, resolve_easeui_color, style_property, Color, Command, Document, Effect, LayoutGrid, NodeId, NodeKind, NodeSpec, Paint, Rect,
    SpecDataSnapshot, Style, StyleMeta, StyleProperty, TextAutoResize, TextStyle, Variable, VariableCollection, VariableKind, VariableMode, VariableValue,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::hash::{Hash, Hasher};

/// Maximum number of operations allowed in a single `designos_apply_batch` call.
pub const MAX_BATCH_OPS: usize = 200;

/// Maximum number of nodes a single `designos_apply_batch` call may create across all ops.
pub const MAX_BATCH_CREATED_NODES: usize = 20_000;

/// JSON Schema descriptor for `designos_apply_batch` (§3.5, §5).
pub fn tool_descriptor() -> Value {
    let node_or_ref = json!({
        "anyOf": [
            { "type": "integer", "minimum": 1 },
            { "type": "string", "pattern": "^(@[A-Za-z0-9_-]{1,64}|\\$prev)(/.+)?$" }
        ],
        "description": "Numeric node ID, '$prev', or '@<as>' / '@<as>/<name path>' reference to an earlier op in the same batch."
    });
    let coordinate = json!({ "type": "number", "minimum": -1000000, "maximum": 1000000 });
    let size = json!({ "type": "number", "minimum": 0, "maximum": 1000000 });
    let mut op_properties = serde_json::Map::new();
    op_properties.insert(
        "op".into(),
        json!({
            "type": "string",
            "enum": ["apply_spec", "insert", "create", "update", "move", "delete", "set_slot", "set_variables", "set_style", "set_in_progress"]
        }),
    );
    op_properties.insert("as".into(), json!({ "type": "string", "pattern": "^[A-Za-z0-9_-]{1,64}$" }));
    op_properties.insert("parent".into(), node_or_ref.clone());
    op_properties.insert("parent_id".into(), node_or_ref.clone());
    op_properties.insert("node".into(), node_or_ref.clone());
    op_properties.insert("node_id".into(), node_or_ref.clone());
    op_properties.insert("nodes".into(), json!({ "type": "array", "items": node_or_ref.clone(), "minItems": 1, "maxItems": 200 }));
    op_properties.insert("node_ids".into(), json!({ "type": "array", "items": node_or_ref, "minItems": 1, "maxItems": 200 }));
    op_properties.insert("value".into(), json!({ "type": "boolean" }));
    op_properties.insert("in_progress".into(), json!({ "type": "boolean" }));
    op_properties.insert("agent".into(), json!({ "type": "string", "maxLength": 256 }));
    op_properties.insert("status".into(), json!({ "type": "string", "maxLength": 512 }));
    op_properties.insert("run_id".into(), json!({ "type": "string", "maxLength": 256 }));
    op_properties.insert("stale_after_s".into(), json!({ "type": "integer", "minimum": 60, "maximum": 86400 }));
    op_properties.insert("force".into(), json!({ "type": "boolean", "default": false }));
    op_properties.insert("index".into(), json!({ "type": "integer", "minimum": 0 }));
    op_properties.insert("spec".into(), json!({ "type": "object" }));
    op_properties.insert("slot".into(), json!({ "type": "string" }));
    op_properties.insert("fill".into(), json!({ "maxLength": 1024 }));
    op_properties.insert("slots".into(), json!({ "type": "object" }));
    op_properties.insert("kind".into(), json!({ "type": "string", "enum": ["frame", "rectangle", "ellipse", "text"] }));
    op_properties.insert("name".into(), json!({ "type": "string", "maxLength": 1024 }));
    op_properties.insert("text".into(), json!({ "type": "string", "maxLength": 100000 }));
    op_properties.insert("x".into(), coordinate.clone());
    op_properties.insert("y".into(), coordinate);
    op_properties.insert("width".into(), size.clone());
    op_properties.insert("height".into(), size);
    op_properties.insert("visible".into(), json!({ "type": "boolean" }));
    op_properties.insert("opacity".into(), json!({ "type": "number", "minimum": 0, "maximum": 1 }));
    op_properties.insert("fills".into(), json!({ "type": "array", "items": { "type": "object" } }));
    op_properties.insert("upsert_collections".into(), json!({ "type": "array", "items": { "type": "object" } }));
    op_properties.insert("upsert_variables".into(), json!({ "type": "array", "items": { "type": "object" } }));
    op_properties.insert("delete_variables".into(), json!({ "type": "array", "items": { "type": "string" } }));
    op_properties.insert("upsert".into(), json!({ "type": "array", "items": { "type": "object" } }));
    op_properties.insert("delete".into(), json!({ "type": "array", "items": { "type": "string" } }));
    op_properties.insert("apply".into(), json!({ "type": "array", "items": { "type": "object" } }));
    let op_item_schema = json!({
        "type": "object",
        "required": ["op"],
        "additionalProperties": false,
        "properties": Value::Object(op_properties)
    });
    let examples = json!([
        {
            "ops": [
                { "op": "create", "parent": 1, "kind": "frame", "name": "Header", "width": 320, "height": 64 },
                { "op": "create", "parent": "$prev", "kind": "text", "name": "Title", "text": "Dashboard" }
            ]
        },
        {
            "ops": [
                {
                    "op": "apply_spec",
                    "as": "hero",
                    "parent": 1,
                    "spec": { "kind": "frame", "name": "Hero", "children": [{ "kind": "text", "name": "Child", "text": "Welcome" }] }
                },
                { "op": "update", "node": "@hero/Child", "opacity": 0.9 }
            ]
        },
        {
            "ops": [
                { "op": "apply_spec", "as": "card", "parent": 1, "spec": { "kind": "instance", "component": "Card" } },
                { "op": "set_slot", "node": "@card", "slot": "body", "fill": [{ "kind": "text", "name": "BodyText", "text": "Filled slot" }] }
            ]
        },
        {
            "dry_run": true,
            "ops": [
                { "op": "insert", "as": "preview", "parent": 1, "kind": "rectangle", "name": "PreviewBox", "width": 160, "height": 80 }
            ]
        }
    ]);
    let success_schema = json!({
        "type": "object",
        "required": ["dry_run", "created", "created_count", "name_to_id", "changed", "touched_roots", "touched_bounds", "undo_label", "issues", "issues_total"],
        "properties": {
            "dry_run": { "type": "boolean" },
            "created": { "type": "array", "items": { "type": "integer" } },
            "created_count": { "type": "integer", "minimum": 0 },
            "name_to_id": { "type": "object" },
            "changed": { "type": "object" },
            "touched_roots": { "type": "array", "items": { "type": "object" } },
            "touched_bounds": { "type": "object" },
            "undo_label": { "type": "string" },
            "issues": { "type": "array", "items": { "type": "object" } },
            "issues_total": { "type": "integer", "minimum": 0 }
        }
    });
    json!({
        "name": "designos_apply_batch",
        "description": "Execute 1..=200 write operations (apply_spec, insert/create, update, move, delete, set_slot, set_variables, set_style, set_in_progress) as a single atomic transaction and one undo group. Later ops may reference nodes created by earlier ops via '$prev', '@<as>', or '@<as>/<name path>'. With dry_run=true, computes and returns created IDs, name_to_id map, touched_roots, and touched_bounds without mutating the document or publishing a live patch. Pass idempotency_key to deduplicate retries (a key names one logical write; after a reconnect, call designos_get_node before trusting a replayed id). Note: in a live Studio session, if the editor remaps a colliding ID on replay, the reply holds the session IDs until the next canvas reload. Examples: (1) create + $prev: {\"ops\":[{\"op\":\"create\",\"parent\":1,\"kind\":\"frame\",\"name\":\"Header\"},{\"op\":\"create\",\"parent\":\"$prev\",\"kind\":\"text\",\"text\":\"Title\"}]}; (2) apply_spec with as + @handle/Child update: {\"ops\":[{\"op\":\"apply_spec\",\"as\":\"hero\",\"spec\":{\"kind\":\"frame\",\"name\":\"Hero\",\"children\":[{\"kind\":\"text\",\"name\":\"Child\",\"text\":\"Hi\"}]}},{\"op\":\"update\",\"node\":\"@hero/Child\",\"opacity\":0.9}]}; (3) set_slot: {\"ops\":[{\"op\":\"set_slot\",\"node\":\"@card\",\"slot\":\"body\",\"fill\":[{\"kind\":\"text\",\"text\":\"Body\"}]}]}; (4) dry_run: {\"dry_run\":true,\"ops\":[{\"op\":\"insert\",\"kind\":\"rectangle\",\"name\":\"Box\"}]}.",
        "inputSchema": {
            "type": "object",
            "required": ["ops"],
            "additionalProperties": false,
            "properties": {
                "ops": {
                    "type": "array",
                    "minItems": 1,
                    "maxItems": 200,
                    "items": op_item_schema
                },
                "dry_run": { "type": "boolean", "default": false },
                "idempotency_key": {
                    "type": "string",
                    "minLength": 1,
                    "maxLength": 128,
                    "pattern": "^[A-Za-z0-9._:-]{1,128}$",
                    "description": "Optional idempotency key (1..=128 chars in [A-Za-z0-9._:-]); a key names one logical write. After a reconnect, call designos_get_node before trusting a replayed id."
                },
                "diff_image": { "type": "boolean", "default": false },
                "lint": {
                    "anyOf": [
                        { "type": "boolean" },
                        { "type": "string", "enum": ["preset", "errors_only", "off"] }
                    ],
                    "description": "Optional per-call lint override (false or 'off' skips the per-call lint pass)."
                }
            },
            "examples": examples
        },
        "outputSchema": tool_output_schema_from_object(success_schema),
        "annotations": {
            "readOnlyHint": false,
            "destructiveHint": true,
            "idempotentHint": false,
            "openWorldHint": false
        }
    })
}

/// Computes the undo group label (`Agent: <run>`) from `DESIGNOS_AGENT_RUN_ID` or `"designos-mcp"` (§3.4).
pub fn agent_undo_label() -> String {
    let run = std::env::var("DESIGNOS_AGENT_RUN_ID").ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).unwrap_or_else(|| "designos-mcp".to_string());
    format!("Agent: {run}")
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BatchArgs {
    ops: Vec<Value>,
    #[serde(default)]
    dry_run: bool,
    #[serde(default)]
    idempotency_key: Option<String>,
    #[serde(default)]
    diff_image: bool,
    #[serde(default)]
    lint: Option<crate::tools::session_options::PerCallLintOverride>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(untagged)]
pub enum NodeRef {
    Id(u64),
    Ref(String),
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ApplySpecOp {
    #[allow(dead_code)]
    op: String,
    #[serde(rename = "as")]
    alias: Option<String>,
    #[serde(alias = "parent_id")]
    parent: Option<NodeRef>,
    spec: NodeSpec,
    index: Option<usize>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct InsertOp {
    #[allow(dead_code)]
    op: String,
    #[serde(rename = "as")]
    alias: Option<String>,
    #[serde(alias = "parent_id")]
    parent: Option<NodeRef>,
    kind: String,
    name: Option<String>,
    text: Option<String>,
    #[serde(default)]
    x: f32,
    #[serde(default)]
    y: f32,
    #[serde(default)]
    width: Option<f32>,
    #[serde(default)]
    height: Option<f32>,
    #[serde(default)]
    fill: Option<String>,
    #[serde(default)]
    fills: Option<Vec<Paint>>,
    index: Option<usize>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct UpdateOp {
    #[allow(dead_code)]
    op: String,
    #[serde(rename = "as")]
    alias: Option<String>,
    #[serde(alias = "node_id")]
    node: NodeRef,
    name: Option<String>,
    text: Option<String>,
    x: Option<f32>,
    y: Option<f32>,
    width: Option<f32>,
    height: Option<f32>,
    visible: Option<bool>,
    opacity: Option<f32>,
    #[serde(default)]
    fill: Option<String>,
    #[serde(default)]
    fills: Option<Vec<Paint>>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct MoveOp {
    #[allow(dead_code)]
    op: String,
    #[serde(rename = "as")]
    alias: Option<String>,
    #[serde(alias = "node_id")]
    node: NodeRef,
    #[serde(alias = "parent_id")]
    parent: NodeRef,
    index: Option<usize>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DeleteOp {
    #[allow(dead_code)]
    op: String,
    #[serde(rename = "as")]
    alias: Option<String>,
    #[serde(alias = "node_id")]
    node: NodeRef,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct UpsertCollectionInput {
    id: String,
    name: Option<String>,
    #[serde(default)]
    modes: Vec<VariableMode>,
    default_mode: Option<String>,
    #[serde(default)]
    variables: Option<Vec<String>>,
    key: Option<String>,
    source_library_key: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct UpsertVariableInput {
    id: String,
    name: Option<String>,
    collection: Option<String>,
    kind: Option<String>,
    #[serde(default)]
    values: BTreeMap<String, Value>,
    description: Option<String>,
    scopes: Option<Vec<String>>,
    hidden_from_publishing: Option<bool>,
    key: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SetVariablesOp {
    #[allow(dead_code)]
    op: String,
    #[serde(rename = "as")]
    alias: Option<String>,
    #[serde(default)]
    upsert_collections: Vec<UpsertCollectionInput>,
    #[serde(default)]
    upsert_variables: Vec<UpsertVariableInput>,
    #[serde(default)]
    delete_variables: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct UpsertStyleInput {
    key: Option<String>,
    id: Option<String>,
    name: Option<String>,
    property: Option<String>,
    description: Option<String>,
    style: Option<Style>,
    fill: Option<String>,
    paints: Option<Vec<Paint>>,
    text_style: Option<TextStyle>,
    effects: Option<Vec<Effect>>,
    grids: Option<Vec<LayoutGrid>>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ApplyStyleLinkInput {
    #[serde(alias = "node_id")]
    node: NodeRef,
    style: String,
    property: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SetStyleOp {
    #[allow(dead_code)]
    op: String,
    #[serde(rename = "as")]
    alias: Option<String>,
    #[serde(default)]
    upsert: Vec<UpsertStyleInput>,
    #[serde(default)]
    delete: Vec<String>,
    #[serde(default)]
    apply: Vec<ApplyStyleLinkInput>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SetSlotOp {
    #[allow(dead_code)]
    op: String,
    #[serde(rename = "as")]
    alias: Option<String>,
    #[serde(alias = "node_id", alias = "instance")]
    node: NodeRef,
    slot: String,
    #[serde(alias = "fill")]
    value: designos_doc::SlotFillSpec,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SetInProgressOp {
    #[allow(dead_code)]
    op: String,
    #[serde(rename = "as")]
    alias: Option<String>,
    #[serde(default, alias = "node_ids")]
    nodes: Option<Vec<NodeRef>>,
    #[serde(default, alias = "node_id")]
    node: Option<NodeRef>,
    #[serde(default)]
    value: Option<bool>,
    #[serde(default)]
    in_progress: Option<bool>,
    #[serde(default, alias = "agent_name")]
    agent: Option<String>,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    run_id: Option<String>,
    #[serde(default)]
    stale_after_s: Option<u32>,
    #[serde(default)]
    force: bool,
}

#[derive(Clone, Debug)]
enum ParsedOp {
    ApplySpec(Box<ApplySpecOp>),
    Insert(InsertOp),
    Update(UpdateOp),
    Move(MoveOp),
    Delete(DeleteOp),
    SetVariables(SetVariablesOp),
    SetStyle(SetStyleOp),
    SetSlot(SetSlotOp),
    SetInProgress(SetInProgressOp),
}

impl ParsedOp {
    fn op_name(&self) -> &'static str {
        match self {
            Self::ApplySpec(_) => "apply_spec",
            Self::Insert(_) => "insert",
            Self::Update(_) => "update",
            Self::Move(_) => "move",
            Self::Delete(_) => "delete",
            Self::SetVariables(_) => "set_variables",
            Self::SetStyle(_) => "set_style",
            Self::SetSlot(_) => "set_slot",
            Self::SetInProgress(_) => "set_in_progress",
        }
    }

    fn alias(&self) -> Option<&str> {
        match self {
            Self::ApplySpec(o) => o.alias.as_deref(),
            Self::Insert(o) => o.alias.as_deref(),
            Self::Update(o) => o.alias.as_deref(),
            Self::Move(o) => o.alias.as_deref(),
            Self::Delete(o) => o.alias.as_deref(),
            Self::SetVariables(o) => o.alias.as_deref(),
            Self::SetStyle(o) => o.alias.as_deref(),
            Self::SetSlot(o) => o.alias.as_deref(),
            Self::SetInProgress(o) => o.alias.as_deref(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChangedCounts {
    pub inserted: usize,
    pub updated: usize,
    pub moved: usize,
    pub deleted: usize,
    pub variables: usize,
    pub styles: usize,
}

impl ChangedCounts {
    pub fn any(&self) -> bool {
        self.inserted > 0 || self.updated > 0 || self.moved > 0 || self.deleted > 0 || self.variables > 0 || self.styles > 0
    }
}

struct CreatedRootRecord {
    alias: Option<String>,
    root_id: NodeId,
    created_ids: BTreeSet<NodeId>,
}

struct TouchedRootRecord {
    op: usize,
    node_id: NodeId,
    before_bounds: Rect,
}

fn batch_error(code: ErrorCode, failed_op: Option<usize>, op: Option<&str>, path: &str, message: impl Into<String>) -> ToolError {
    let mut err = ToolError::new(code, path, message);
    err.failed_op = failed_op;
    err.op = op.map(ToOwned::to_owned);
    err
}

fn op_failed_error(failed_op: usize, op: &str, inner_code: ErrorCode, path: &str, message: impl Into<String>) -> ToolError {
    let cause = ToolError::new(inner_code, path, message);
    ToolError::batch_op_failed(failed_op, op, cause)
}

fn wrap_op_err(failed_op: usize, op: &str, op_path: &str, mut cause: ToolError) -> ToolError {
    if cause.path.is_empty() {
        cause.path = op_path.to_string();
    } else if !cause.path.starts_with("/ops/") {
        if cause.path.starts_with('/') {
            cause.path = format!("{op_path}{}", cause.path);
        } else {
            cause.path = format!("{op_path}/{}", cause.path);
        }
    }
    ToolError::batch_op_failed(failed_op, op, cause)
}

fn is_valid_as(s: &str) -> bool {
    !s.is_empty() && s.len() <= 64 && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

fn validate_ref_pre_run(r: &NodeRef, declared_as: &BTreeSet<String>, has_prev: bool, op_idx: usize, op_name: &str, field: &str) -> Result<(), ToolError> {
    let path = format!("/ops/{op_idx}/{field}");
    match r {
        NodeRef::Id(0) => Err(op_failed_error(op_idx, op_name, ErrorCode::ValueOutOfRange, &path, "node IDs must be positive integers")),
        NodeRef::Id(_) => Ok(()),
        NodeRef::Ref(s) => {
            if s == "$prev" {
                if !has_prev {
                    let mut err = batch_error(
                        ErrorCode::NoPreviousNode,
                        Some(op_idx),
                        Some(op_name),
                        &path,
                        "$prev cannot be used before an earlier operation has produced a node",
                    );
                    err.corrected_example = Some(json!({ "parent": 1 }));
                    return Err(err);
                }
                return Ok(());
            }
            let valid: Vec<String> = declared_as.iter().map(|a| format!("@{a}")).collect();
            let first_ref = valid.first().cloned();
            let Some(rest) = s.strip_prefix('@') else {
                let mut err = batch_error(
                    ErrorCode::UnknownBatchHandle,
                    Some(op_idx),
                    Some(op_name),
                    &path,
                    format!("invalid reference '{s}'; references must start with '@'"),
                )
                .with_valid_options(valid);
                err.corrected_example = first_ref.map(|r| json!({ field: r }));
                return Err(err);
            };
            let (alias, subpath) = match rest.split_once('/') {
                Some((a, p)) => (a, Some(p)),
                None => (rest, None),
            };
            if alias.is_empty() || !declared_as.contains(alias) {
                let mut err = batch_error(ErrorCode::UnknownBatchHandle, Some(op_idx), Some(op_name), &path, format!("unknown or forward reference '{s}'"))
                    .with_valid_options(valid);
                err.corrected_example = first_ref.map(|r| json!({ field: r }));
                return Err(err);
            }
            if let Some(p) = subpath {
                if p.is_empty() || p.split('/').any(str::is_empty) {
                    return Err(batch_error(
                        ErrorCode::UnknownBatchHandle,
                        Some(op_idx),
                        Some(op_name),
                        &path,
                        format!("malformed descendant path in reference '{s}'"),
                    )
                    .with_valid_options(valid));
                }
            }
            Ok(())
        }
    }
}

fn parse_and_validate_ops(raw_ops: Vec<Value>) -> Result<Vec<ParsedOp>, ToolError> {
    if raw_ops.is_empty() {
        return Err(batch_error(ErrorCode::InvalidArguments, None, None, "/ops", "ops must contain at least 1 operation"));
    }
    if raw_ops.len() > MAX_BATCH_OPS {
        let mut err = batch_error(
            ErrorCode::BatchTooManyOps,
            None,
            None,
            "/ops",
            format!("batch contains {} ops, exceeding the {} op limit", raw_ops.len(), MAX_BATCH_OPS),
        );
        err.corrected_example = Some(json!({ "ops": [{ "op": "insert", "kind": "frame", "name": "Root" }] }));
        return Err(err);
    }

    let mut declared_as = BTreeSet::new();
    let mut has_prev = false;
    let mut parsed = Vec::with_capacity(raw_ops.len());

    for (idx, val) in raw_ops.into_iter().enumerate() {
        let op_str = val
            .get("op")
            .and_then(Value::as_str)
            .ok_or_else(|| batch_error(ErrorCode::InvalidArguments, Some(idx), None, &format!("/ops/{idx}/op"), "missing or non-string 'op' field"))?
            .to_string();

        let op_path = format!("/ops/{idx}");
        let raw_op = val.clone();
        let op = match op_str.as_str() {
            "apply_spec" => {
                if let Some(raw_spec) = val.get("spec") {
                    if let Err(se) = designos_doc::validate_raw_spec_slots(raw_spec) {
                        let cause = crate::tools::errors::from_spec_error(se, &format!("/ops/{idx}/spec"));
                        return Err(ToolError::batch_op_failed(idx, "apply_spec", cause));
                    }
                }
                let a: ApplySpecOp = serde_json::from_value(val).map_err(|e| {
                    let cause = crate::tools::errors::from_serde_error_with_args(&e, &op_path, true, Some(&raw_op));
                    ToolError::batch_op_failed(idx, "apply_spec", cause)
                })?;
                if let Some(spec_msg) = crate::tools::errors::describe_spec_args_error(&raw_op) {
                    let cause = ToolError::new(ErrorCode::InvalidArguments, &op_path, format!("Invalid arguments: {spec_msg}"));
                    return Err(ToolError::batch_op_failed(idx, "apply_spec", cause));
                }
                if let Some(p) = &a.parent {
                    if !matches!(p, NodeRef::Id(0)) {
                        validate_ref_pre_run(p, &declared_as, has_prev, idx, "apply_spec", "parent")?;
                    }
                }
                has_prev = true;
                ParsedOp::ApplySpec(Box::new(a))
            }
            "insert" | "create" => {
                let a: InsertOp = serde_json::from_value(val).map_err(|e| {
                    let cause = crate::tools::errors::from_serde_error_with_args(&e, &op_path, false, Some(&raw_op));
                    ToolError::batch_op_failed(idx, "insert", cause)
                })?;
                if let Some(p) = &a.parent {
                    validate_ref_pre_run(p, &declared_as, has_prev, idx, "insert", "parent")?;
                }
                validate_create_fields(&a.kind, a.name.as_deref(), a.text.as_deref(), a.x, a.y, a.width.unwrap_or(100.0), a.height.unwrap_or(100.0))
                    .map_err(|e| wrap_op_err(idx, "insert", &op_path, e))?;
                validate_fill_color(a.fill.as_deref()).map_err(|e| wrap_op_err(idx, "insert", &op_path, e))?;
                has_prev = true;
                ParsedOp::Insert(a)
            }
            "update" => {
                let a: UpdateOp = serde_json::from_value(val).map_err(|e| {
                    let cause = crate::tools::errors::from_serde_error_with_args(&e, &op_path, false, Some(&raw_op));
                    ToolError::batch_op_failed(idx, "update", cause)
                })?;
                validate_ref_pre_run(&a.node, &declared_as, has_prev, idx, "update", "node")?;
                validate_update_fields(&UpdateFields {
                    name: a.name.clone(),
                    text: a.text.clone(),
                    x: a.x,
                    y: a.y,
                    width: a.width,
                    height: a.height,
                    visible: a.visible,
                    opacity: a.opacity,
                    fill: a.fill.clone(),
                    fills: a.fills.clone(),
                })
                .map_err(|e| wrap_op_err(idx, "update", &op_path, e))?;
                has_prev = true;
                ParsedOp::Update(a)
            }
            "move" => {
                let a: MoveOp = serde_json::from_value(val).map_err(|e| {
                    let cause = crate::tools::errors::from_serde_error_with_args(&e, &op_path, false, Some(&raw_op));
                    ToolError::batch_op_failed(idx, "move", cause)
                })?;
                validate_ref_pre_run(&a.node, &declared_as, has_prev, idx, "move", "node")?;
                validate_ref_pre_run(&a.parent, &declared_as, has_prev, idx, "move", "parent")?;
                has_prev = true;
                ParsedOp::Move(a)
            }
            "delete" => {
                let a: DeleteOp = serde_json::from_value(val).map_err(|e| {
                    let cause = crate::tools::errors::from_serde_error_with_args(&e, &op_path, false, Some(&raw_op));
                    ToolError::batch_op_failed(idx, "delete", cause)
                })?;
                validate_ref_pre_run(&a.node, &declared_as, has_prev, idx, "delete", "node")?;
                ParsedOp::Delete(a)
            }
            "set_variables" => {
                let a: SetVariablesOp = serde_json::from_value(val).map_err(|e| {
                    let cause = crate::tools::errors::from_serde_error_with_args(&e, &op_path, false, Some(&raw_op));
                    ToolError::batch_op_failed(idx, "set_variables", cause)
                })?;
                ParsedOp::SetVariables(a)
            }
            "set_style" => {
                let a: SetStyleOp = serde_json::from_value(val).map_err(|e| {
                    let cause = crate::tools::errors::from_serde_error_with_args(&e, &op_path, false, Some(&raw_op));
                    ToolError::batch_op_failed(idx, "set_style", cause)
                })?;
                for (link_idx, link) in a.apply.iter().enumerate() {
                    validate_ref_pre_run(&link.node, &declared_as, has_prev, idx, "set_style", &format!("apply/{link_idx}/node"))?;
                }
                ParsedOp::SetStyle(a)
            }
            "set_slot" => {
                if let Some(raw_val) = val.get("value") {
                    if let Err(se) = designos_doc::validate_raw_spec_slots(raw_val) {
                        let cause = crate::tools::errors::from_spec_error(se, &format!("/ops/{idx}/value"));
                        return Err(ToolError::batch_op_failed(idx, "set_slot", cause));
                    }
                }
                let a: SetSlotOp = serde_json::from_value(val).map_err(|e| {
                    let cause = crate::tools::errors::from_serde_error_with_args(&e, &op_path, true, Some(&raw_op));
                    ToolError::batch_op_failed(idx, "set_slot", cause)
                })?;
                if let Some(spec_msg) = crate::tools::errors::describe_spec_args_error(&raw_op) {
                    let cause = ToolError::new(ErrorCode::InvalidArguments, &op_path, format!("Invalid arguments: {spec_msg}"));
                    return Err(ToolError::batch_op_failed(idx, "set_slot", cause));
                }
                validate_ref_pre_run(&a.node, &declared_as, has_prev, idx, "set_slot", "node")?;
                has_prev = true;
                ParsedOp::SetSlot(a)
            }
            "set_in_progress" => {
                let a: SetInProgressOp = serde_json::from_value(val).map_err(|e| {
                    let cause = crate::tools::errors::from_serde_error_with_args(&e, &op_path, false, Some(&raw_op));
                    ToolError::batch_op_failed(idx, "set_in_progress", cause)
                })?;
                if a.value.is_none() && a.in_progress.is_none() {
                    return Err(op_failed_error(idx, "set_in_progress", ErrorCode::MissingField, &format!("{op_path}/value"), "missing boolean field 'value'"));
                }
                if let Some(stale) = a.stale_after_s {
                    if !(60..=86_400).contains(&stale) {
                        return Err(op_failed_error(
                            idx,
                            "set_in_progress",
                            ErrorCode::ValueOutOfRange,
                            &format!("{op_path}/stale_after_s"),
                            "stale_after_s must be between 60 and 86400",
                        ));
                    }
                }
                let mut total_targets = 0usize;
                if let Some(list) = &a.nodes {
                    for (n_idx, r) in list.iter().enumerate() {
                        validate_ref_pre_run(r, &declared_as, has_prev, idx, "set_in_progress", &format!("nodes/{n_idx}"))?;
                        total_targets += 1;
                    }
                }
                if let Some(single) = &a.node {
                    validate_ref_pre_run(single, &declared_as, has_prev, idx, "set_in_progress", "node")?;
                    total_targets += 1;
                }
                if total_targets == 0 {
                    return Err(op_failed_error(
                        idx,
                        "set_in_progress",
                        ErrorCode::InvalidArguments,
                        &format!("{op_path}/nodes"),
                        "set_in_progress requires at least one target in 'nodes' or 'node'",
                    ));
                }
                if total_targets > 200 {
                    return Err(op_failed_error(
                        idx,
                        "set_in_progress",
                        ErrorCode::ValueOutOfRange,
                        &format!("{op_path}/nodes"),
                        "set_in_progress targets at most 200 nodes per op",
                    ));
                }
                has_prev = true;
                ParsedOp::SetInProgress(a)
            }
            other => {
                return Err(batch_error(
                    ErrorCode::InvalidArguments,
                    Some(idx),
                    Some(other),
                    &format!("/ops/{idx}/op"),
                    format!("unsupported batch op '{other}'"),
                )
                .with_valid_options(vec!["apply_spec", "insert", "update", "move", "delete", "set_variables", "set_style", "set_slot"]));
            }
        };

        if let Some(alias) = op.alias() {
            if !is_valid_as(alias) {
                return Err(op_failed_error(
                    idx,
                    op.op_name(),
                    ErrorCode::InvalidArguments,
                    &format!("/ops/{idx}/as"),
                    format!("invalid 'as' alias '{alias}'; must match ^[A-Za-z0-9_-]{{1,64}}$"),
                ));
            }
            if !declared_as.insert(alias.to_string()) {
                let mut err = batch_error(
                    ErrorCode::DuplicateBatchHandle,
                    Some(idx),
                    Some(op.op_name()),
                    &format!("/ops/{idx}/as"),
                    format!("duplicate 'as' alias '{alias}'"),
                );
                err.corrected_example = Some(json!({ "as": format!("{alias}_2") }));
                return Err(err);
            }
        }

        parsed.push(op);
    }

    Ok(parsed)
}

fn parse_indexed_segment(seg: &str) -> Option<(&str, usize)> {
    let rest = seg.strip_suffix(']')?;
    let (base, idx_str) = rest.rsplit_once('[')?;
    let idx: usize = idx_str.parse().ok()?;
    (idx >= 1 && !base.is_empty()).then_some((base, idx))
}

fn resolve_node_ref(
    doc: &Document,
    alias_roots: &BTreeMap<String, NodeId>,
    prev_node: Option<NodeId>,
    r: &NodeRef,
    op_idx: usize,
    op_name: &str,
    field: &str,
) -> Result<NodeId, ToolError> {
    let path = format!("/ops/{op_idx}/{field}");
    match r {
        NodeRef::Id(id) => {
            if *id == 0 {
                return Err(op_failed_error(op_idx, op_name, ErrorCode::ValueOutOfRange, &path, "node IDs must be positive integers"));
            }
            doc.get(NodeId(*id)).map(|n| n.id).ok_or_else(|| op_failed_error(op_idx, op_name, ErrorCode::NodeNotFound, &path, format!("node {id} not found")))
        }
        NodeRef::Ref(s) => {
            if s == "$prev" {
                let Some(id) = prev_node else {
                    return Err(batch_error(
                        ErrorCode::NoPreviousNode,
                        Some(op_idx),
                        Some(op_name),
                        &path,
                        "$prev cannot be used before an earlier operation has produced a node",
                    ));
                };
                if doc.get(id).is_none() {
                    return Err(op_failed_error(op_idx, op_name, ErrorCode::NodeNotFound, &path, format!("node {} not found", id.0)));
                }
                return Ok(id);
            }
            let valid: Vec<String> = alias_roots.keys().map(|a| format!("@{a}")).collect();
            let rest = s.strip_prefix('@').unwrap_or(s);
            let (alias, subpath) = match rest.split_once('/') {
                Some((a, p)) => (a, Some(p)),
                None => (rest, None),
            };
            let Some(&root_id) = alias_roots.get(alias) else {
                return Err(batch_error(ErrorCode::UnknownBatchHandle, Some(op_idx), Some(op_name), &path, format!("unknown reference '{s}'"))
                    .with_valid_options(valid));
            };
            if doc.get(root_id).is_none() {
                return Err(batch_error(
                    ErrorCode::UnknownBatchHandle,
                    Some(op_idx),
                    Some(op_name),
                    &path,
                    format!("reference '{s}' points to a deleted node"),
                )
                .with_valid_options(valid));
            }
            let Some(sub) = subpath else {
                return Ok(root_id);
            };
            let mut cur = root_id;
            for seg in sub.split('/') {
                let children = &doc
                    .get(cur)
                    .ok_or_else(|| {
                        batch_error(ErrorCode::UnknownBatchHandle, Some(op_idx), Some(op_name), &path, format!("unknown reference '{s}'"))
                            .with_valid_options(valid.clone())
                    })?
                    .children;
                if let Some((base, idx_1)) = parse_indexed_segment(seg) {
                    let matched = children.iter().copied().filter(|&cid| doc.get(cid).is_some_and(|n| n.name == base)).nth(idx_1 - 1);
                    cur = matched.ok_or_else(|| {
                        batch_error(
                            ErrorCode::UnknownBatchHandle,
                            Some(op_idx),
                            Some(op_name),
                            &path,
                            format!("descendant segment '{seg}' in '{s}' matched fewer than {idx_1} children"),
                        )
                    })?;
                } else {
                    let matches: Vec<NodeId> = children.iter().copied().filter(|&cid| doc.get(cid).is_some_and(|n| n.name == seg)).collect();
                    match matches.as_slice() {
                        [only] => cur = *only,
                        [] => {
                            let child_opts: Vec<String> = children
                                .iter()
                                .filter_map(|&cid| doc.get(cid))
                                .filter(|n| !n.name.is_empty())
                                .map(|n| format!("@{alias}/{}", n.name))
                                .collect();
                            return Err(batch_error(
                                ErrorCode::UnknownBatchHandle,
                                Some(op_idx),
                                Some(op_name),
                                &path,
                                format!("descendant segment '{seg}' not found in '{s}'"),
                            )
                            .with_valid_options(child_opts));
                        }
                        _ => {
                            return Err(batch_error(
                                ErrorCode::UnknownBatchHandle,
                                Some(op_idx),
                                Some(op_name),
                                &path,
                                format!(
                                    "descendant segment '{seg}' in '{s}' is ambiguous ({} siblings share that name; use '{seg}[1]', '{seg}[2]', ...)",
                                    matches.len()
                                ),
                            ))
                        }
                    }
                }
            }
            Ok(cur)
        }
    }
}

/// Checks RD-3 frame ownership lock when `engine.owned_frames` or `DESIGNOS_OWNED_FRAMES` is active (§13).
fn check_lease_guard(doc: &Document, owned_frames: Option<&[NodeId]>, target: NodeId, op_idx: usize, op_name: &str) -> Result<(), ToolError> {
    let env_owned: Option<Vec<NodeId>> = std::env::var("DESIGNOS_OWNED_FRAMES")
        .ok()
        .map(|s| s.split(',').filter_map(|p| p.trim().parse::<u64>().ok()).filter(|&id| id > 0).map(NodeId).collect())
        .filter(|v: &Vec<NodeId>| !v.is_empty());
    let Some(frames) = owned_frames.filter(|v| !v.is_empty()).or(env_owned.as_deref()) else {
        return Ok(());
    };
    let allowed = frames.iter().any(|&f| f == target || doc.is_ancestor(f, target));
    if !allowed {
        return Err(op_failed_error(
            op_idx,
            op_name,
            ErrorCode::OutsideOwnedFrames,
            &format!("/ops/{op_idx}"),
            format!("node {} is outside this agent run's owned frames", target.0),
        ));
    }
    Ok(())
}

/// Refuses document-wide edits (`set_variables`, `set_style` upsert/delete) while a frame lease is active (§13).
fn check_document_lease_guard(owned_frames: Option<&[NodeId]>, op_idx: usize, op_name: &str) -> Result<(), ToolError> {
    let env_owned: Option<Vec<NodeId>> = std::env::var("DESIGNOS_OWNED_FRAMES")
        .ok()
        .map(|s| s.split(',').filter_map(|p| p.trim().parse::<u64>().ok()).filter(|&id| id > 0).map(NodeId).collect())
        .filter(|v: &Vec<NodeId>| !v.is_empty());
    if owned_frames.is_some_and(|v| !v.is_empty()) || env_owned.is_some() {
        return Err(op_failed_error(
            op_idx,
            op_name,
            ErrorCode::OutsideOwnedFrames,
            &format!("/ops/{op_idx}"),
            format!("{op_name} modifies document-wide definitions outside this agent run's owned frames"),
        ));
    }
    Ok(())
}

/// Finds the top-most non-Page container ancestor of `id` so auto-layout re-solves the affected
/// top-level frame/component/section without walking unrelated top-level frames on a 1.2M-node page.
pub(crate) fn top_container_under_page(doc: &Document, id: NodeId) -> Option<NodeId> {
    let n = doc.get(id)?;
    if matches!(n.kind, NodeKind::Page { .. }) {
        return None;
    }
    let mut cur = id;
    while let Some(p) = doc.get(cur).and_then(|node| node.parent) {
        if doc.get(p).is_some_and(|pn| matches!(pn.kind, NodeKind::Page { .. })) {
            return Some(cur);
        }
        cur = p;
    }
    Some(cur)
}

pub(crate) fn relayout_for_node(doc: &mut Document, id: NodeId) {
    if let Some(top) = top_container_under_page(doc, id) {
        doc.relayout_subtree(top);
        doc.flush_pending_relayout();
    }
}

/// Page-space bounding box of `id` in `doc`. For a `Page` node (whose own `size` is `(0, 0)`),
/// returns the union of its direct children's page-space bounds when non-empty.
pub(crate) fn node_page_bounds(doc: &Document, id: NodeId) -> Rect {
    let Some(n) = doc.get(id) else {
        return Rect::new(0.0, 0.0, 0.0, 0.0);
    };
    if matches!(n.kind, NodeKind::Page { .. }) && n.size.x == 0.0 && n.size.y == 0.0 {
        let mut iter = n.children.iter().copied().filter_map(|cid| doc.get(cid).map(|_| node_page_bounds(doc, cid)));
        let Some(first) = iter.next() else {
            return Rect::new(0.0, 0.0, 0.0, 0.0);
        };
        return iter.fold(first, |acc, r| acc.union(&r));
    }
    let wt = doc.world_transform(id);
    let corners = [
        wt.apply(designos_doc::Vec2::new(0.0, 0.0)),
        wt.apply(designos_doc::Vec2::new(n.size.x, 0.0)),
        wt.apply(designos_doc::Vec2::new(0.0, n.size.y)),
        wt.apply(designos_doc::Vec2::new(n.size.x, n.size.y)),
    ];
    let min_x = corners.iter().map(|p| p.x).fold(f32::INFINITY, f32::min);
    let min_y = corners.iter().map(|p| p.y).fold(f32::INFINITY, f32::min);
    let max_x = corners.iter().map(|p| p.x).fold(f32::NEG_INFINITY, f32::max);
    let max_y = corners.iter().map(|p| p.y).fold(f32::NEG_INFINITY, f32::max);
    if min_x.is_finite() && min_y.is_finite() && max_x.is_finite() && max_y.is_finite() {
        Rect::new(min_x, min_y, (max_x - min_x).max(0.0), (max_y - min_y).max(0.0))
    } else {
        Rect::new(0.0, 0.0, 0.0, 0.0)
    }
}

fn bounds_json(r: Rect) -> Value {
    json!({
        "x": r.x,
        "y": r.y,
        "width": r.w,
        "height": r.h,
    })
}

fn parse_variable_kind(s: &str) -> Result<VariableKind, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "color" | "colour" => Ok(VariableKind::Color),
        "number" | "float" => Ok(VariableKind::Number),
        "string" | "text" => Ok(VariableKind::String),
        "boolean" | "bool" => Ok(VariableKind::Boolean),
        other => Err(format!("unknown variable kind '{other}' (expected Color, Number, String, or Boolean)")),
    }
}

fn parse_variable_value(val: &Value, kind: VariableKind) -> Result<VariableValue, (ErrorCode, String)> {
    if let Ok(vv) = serde_json::from_value::<VariableValue>(val.clone()) {
        return Ok(vv);
    }
    if let Some(obj) = val.as_object() {
        if let Some(alias) = obj.get("alias").or_else(|| obj.get("Alias")).and_then(Value::as_str) {
            return Ok(VariableValue::Alias(alias.to_string()));
        }
    }
    match kind {
        VariableKind::Color => {
            if let Some(s) = val.as_str() {
                if let Some(alias) = s.strip_prefix("var:") {
                    return Ok(VariableValue::Alias(alias.trim().to_string()));
                }
                let c = resolve_easeui_color(s).ok_or_else(|| (ErrorCode::InvalidColor, format!("invalid color value '{s}'")))?;
                Ok(VariableValue::Color(c))
            } else {
                let c: Color = serde_json::from_value(val.clone()).map_err(|e| (ErrorCode::InvalidColor, format!("invalid color value: {e}")))?;
                Ok(VariableValue::Color(c))
            }
        }
        VariableKind::Number => {
            let n = val.as_f64().ok_or_else(|| (ErrorCode::InvalidArguments, format!("expected number value, got {val}")))? as f32;
            if !n.is_finite() {
                return Err((ErrorCode::ValueOutOfRange, "variable number value must be finite".into()));
            }
            Ok(VariableValue::Number(n))
        }
        VariableKind::String => {
            let s = val.as_str().ok_or_else(|| (ErrorCode::InvalidArguments, format!("expected string value, got {val}")))?;
            Ok(VariableValue::String(s.to_string()))
        }
        VariableKind::Boolean => {
            let b = val.as_bool().ok_or_else(|| (ErrorCode::InvalidArguments, format!("expected boolean value, got {val}")))?;
            Ok(VariableValue::Boolean(b))
        }
    }
}

fn apply_set_variables_op(doc: &Document, op: &SetVariablesOp, op_idx: usize) -> Result<(Option<Command>, usize), ToolError> {
    let path = format!("/ops/{op_idx}");
    let mut collections = doc.variable_collections.clone();
    let mut variables = doc.variables.clone();

    for c_in in &op.upsert_collections {
        if c_in.id.trim().is_empty() {
            return Err(op_failed_error(op_idx, "set_variables", ErrorCode::InvalidArguments, &path, "collection id must not be empty"));
        }
        let existing = collections.get(&c_in.id).cloned();
        let modes = if !c_in.modes.is_empty() {
            c_in.modes.clone()
        } else if let Some(ex) = &existing {
            ex.modes.clone()
        } else {
            vec![VariableMode { id: "light".into(), name: "Light".into() }]
        };
        let default_mode = c_in
            .default_mode
            .clone()
            .or_else(|| existing.as_ref().map(|e| e.default_mode.clone()))
            .unwrap_or_else(|| modes.first().map(|m| m.id.clone()).unwrap_or_else(|| "light".into()));
        let name = c_in.name.clone().or_else(|| existing.as_ref().map(|e| e.name.clone())).unwrap_or_else(|| c_in.id.clone());
        let vars_list = c_in.variables.clone().or_else(|| existing.as_ref().map(|e| e.variables.clone())).unwrap_or_default();
        collections.insert(
            c_in.id.clone(),
            VariableCollection {
                id: c_in.id.clone(),
                name,
                modes,
                default_mode,
                variables: vars_list,
                key: c_in.key.clone().or_else(|| existing.as_ref().and_then(|e| e.key.clone())),
                source_library_key: c_in.source_library_key.clone().or_else(|| existing.as_ref().and_then(|e| e.source_library_key.clone())),
                extensions: existing.as_ref().map(|e| e.extensions.clone()).unwrap_or_default(),
            },
        );
    }

    for v_in in &op.upsert_variables {
        if v_in.id.trim().is_empty() {
            return Err(op_failed_error(op_idx, "set_variables", ErrorCode::InvalidArguments, &path, "variable id must not be empty"));
        }
        let existing = variables.get(&v_in.id).cloned();
        let kind = match (&v_in.kind, &existing) {
            (Some(k), _) => parse_variable_kind(k).map_err(|e| op_failed_error(op_idx, "set_variables", ErrorCode::InvalidArguments, &path, e))?,
            (None, Some(ex)) => ex.kind,
            (None, None) => VariableKind::Color,
        };
        let collection_id =
            v_in.collection.clone().or_else(|| existing.as_ref().map(|e| e.collection.clone())).or_else(|| collections.keys().next().cloned()).unwrap_or_else(
                || {
                    let default_col = "default".to_string();
                    collections.entry(default_col.clone()).or_insert_with(|| VariableCollection {
                        id: default_col.clone(),
                        name: "Default".into(),
                        modes: vec![VariableMode { id: "light".into(), name: "Light".into() }],
                        default_mode: "light".into(),
                        variables: Vec::new(),
                        key: None,
                        source_library_key: None,
                        extensions: Default::default(),
                    });
                    default_col
                },
            );
        if !collections.contains_key(&collection_id) {
            return Err(op_failed_error(
                op_idx,
                "set_variables",
                ErrorCode::UnknownVariableCollection,
                &path,
                format!("variable collection '{collection_id}' not found"),
            ));
        }
        let mut values = existing.as_ref().map(|e| e.values.clone()).unwrap_or_default();
        for (mode_id, raw_val) in &v_in.values {
            let parsed_val = parse_variable_value(raw_val, kind).map_err(|(code, msg)| op_failed_error(op_idx, "set_variables", code, &path, msg))?;
            values.insert(mode_id.clone(), parsed_val);
        }
        let name = v_in.name.clone().or_else(|| existing.as_ref().map(|e| e.name.clone())).unwrap_or_else(|| v_in.id.clone());
        let var = Variable {
            id: v_in.id.clone(),
            name,
            collection: collection_id.clone(),
            kind,
            values,
            description: v_in.description.clone().or_else(|| existing.as_ref().map(|e| e.description.clone())).unwrap_or_default(),
            scopes: v_in.scopes.clone().or_else(|| existing.as_ref().map(|e| e.scopes.clone())).unwrap_or_default(),
            hidden_from_publishing: v_in.hidden_from_publishing.or_else(|| existing.as_ref().map(|e| e.hidden_from_publishing)).unwrap_or(false),
            key: v_in.key.clone().or_else(|| existing.as_ref().and_then(|e| e.key.clone())),
            code_syntax: existing.as_ref().map(|e| e.code_syntax.clone()).unwrap_or_default(),
            extensions: existing.as_ref().map(|e| e.extensions.clone()).unwrap_or_default(),
            mode_extensions: existing.map(|e| e.mode_extensions).unwrap_or_default(),
        };
        variables.insert(v_in.id.clone(), var);
        if let Some(col) = collections.get_mut(&collection_id) {
            if !col.variables.contains(&v_in.id) {
                col.variables.push(v_in.id.clone());
            }
        }
    }

    for del_id in &op.delete_variables {
        variables.remove(del_id);
        for col in collections.values_mut() {
            col.variables.retain(|v| v != del_id);
        }
    }

    if variables == doc.variables && collections == doc.variable_collections {
        return Ok((None, 0));
    }
    let count = (op.upsert_collections.len() + op.upsert_variables.len() + op.delete_variables.len()).max(1);
    Ok((Some(Command::SetVariables { variables, collections }), count))
}

fn parse_style_property(s: &str) -> Result<StyleProperty, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "fill" => Ok(StyleProperty::Fill),
        "stroke" => Ok(StyleProperty::Stroke),
        "text" => Ok(StyleProperty::Text),
        "effect" => Ok(StyleProperty::Effect),
        "grid" => Ok(StyleProperty::Grid),
        other => Err(format!("unknown style property '{other}' (expected fill, stroke, text, effect, or grid)")),
    }
}

fn apply_set_style_op(
    doc: &Document,
    alias_roots: &BTreeMap<String, NodeId>,
    prev_node: Option<NodeId>,
    owned_frames: Option<&[NodeId]>,
    op: &SetStyleOp,
    op_idx: usize,
) -> Result<(Vec<Command>, usize, Option<NodeId>), ToolError> {
    if !op.upsert.is_empty() || !op.delete.is_empty() {
        check_document_lease_guard(owned_frames, op_idx, "set_style")?;
    }
    let path = format!("/ops/{op_idx}");
    let mut styles = doc.styles.clone();
    let mut style_meta = doc.style_meta.clone();
    let mut cmds = Vec::new();

    for s_in in &op.upsert {
        let key = s_in
            .key
            .clone()
            .or_else(|| s_in.id.clone())
            .or_else(|| s_in.name.clone())
            .ok_or_else(|| op_failed_error(op_idx, "set_style", ErrorCode::MissingField, &path, "style upsert entry requires 'key', 'id', or 'name'"))?;
        if key.trim().is_empty() {
            return Err(op_failed_error(op_idx, "set_style", ErrorCode::InvalidArguments, &path, "style key must not be empty"));
        }
        let existing_style = styles.get(&key).cloned();
        let existing_meta = style_meta.get(&key).cloned();
        let style_val = if let Some(st) = &s_in.style {
            st.clone()
        } else if let Some(fill_str) = &s_in.fill {
            let c = resolve_easeui_color(fill_str)
                .ok_or_else(|| op_failed_error(op_idx, "set_style", ErrorCode::InvalidColor, &path, format!("invalid style color '{fill_str}'")))?;
            Style::Fill(vec![Paint::solid(c)])
        } else if let Some(paints) = &s_in.paints {
            Style::Fill(paints.clone())
        } else if let Some(ts) = &s_in.text_style {
            Style::Text(ts.clone())
        } else if let Some(fx) = &s_in.effects {
            Style::Effect(fx.clone())
        } else if let Some(grids) = &s_in.grids {
            Style::Grid(grids.clone())
        } else if let Some(ex) = existing_style {
            ex
        } else {
            return Err(op_failed_error(
                op_idx,
                "set_style",
                ErrorCode::MissingField,
                &path,
                format!("style '{key}' requires 'style', 'fill', 'paints', 'text_style', 'effects', or 'grids'"),
            ));
        };

        let prop = match &s_in.property {
            Some(p) => parse_style_property(p).map_err(|e| op_failed_error(op_idx, "set_style", ErrorCode::InvalidArguments, &path, e))?,
            None => existing_meta.as_ref().map(|m| m.property).unwrap_or(match &style_val {
                Style::Fill(_) => StyleProperty::Fill,
                Style::Text(_) => StyleProperty::Text,
                Style::Effect(_) => StyleProperty::Effect,
                Style::Grid(_) => StyleProperty::Grid,
            }),
        };
        let name = s_in.name.clone().or_else(|| existing_meta.as_ref().map(|m| m.name.clone())).unwrap_or_else(|| key.clone());
        let description = s_in.description.clone().or_else(|| existing_meta.as_ref().map(|m| m.description.clone())).unwrap_or_default();
        let bindings = existing_meta.as_ref().map(|m| m.bindings.clone()).unwrap_or_default();
        let (lib_key, sort_position) = match existing_meta {
            Some(m) => (m.key, m.sort_position),
            None => (None, None),
        };
        styles.insert(key.clone(), style_val);
        style_meta.insert(key.clone(), StyleMeta { name, key: lib_key, description, property: prop, bindings, sort_position });
    }

    for del_key in &op.delete {
        styles.remove(del_key);
        style_meta.remove(del_key);
    }

    let styles_changed = styles != doc.styles || style_meta != doc.style_meta;
    if styles_changed {
        cmds.push(Command::SetStyles { styles: styles.clone(), style_meta: style_meta.clone() });
    }

    let mut first_applied_node = None;
    for (link_idx, link) in op.apply.iter().enumerate() {
        let node_id = resolve_node_ref(doc, alias_roots, prev_node, &link.node, op_idx, "set_style", &format!("apply/{link_idx}/node"))?;
        check_lease_guard(doc, owned_frames, node_id, op_idx, "set_style")?;
        if first_applied_node.is_none() {
            first_applied_node = Some(node_id);
        }
        let resolved_key = if styles.contains_key(&link.style) {
            link.style.clone()
        } else if let Some((k, _)) = style_meta.iter().find(|(_, m)| m.name == link.style) {
            k.clone()
        } else {
            return Err(op_failed_error(
                op_idx,
                "set_style",
                ErrorCode::UnknownStyle,
                &format!("/ops/{op_idx}/apply/{link_idx}/style"),
                format!("style '{}' not found", link.style),
            ));
        };
        let st = styles.get(&resolved_key).unwrap();
        let prop = match &link.property {
            Some(p) => parse_style_property(p).map_err(|e| op_failed_error(op_idx, "set_style", ErrorCode::InvalidArguments, &path, e))?,
            None => style_meta.get(&resolved_key).map(|m| m.property).unwrap_or_else(|| style_property(doc, &resolved_key)),
        };
        let ref_slot = match prop {
            StyleProperty::Fill => "fill",
            StyleProperty::Stroke => "stroke",
            StyleProperty::Text => "text",
            StyleProperty::Effect => "effect",
            StyleProperty::Grid => "grid",
        };
        let cur =
            doc.get(node_id).ok_or_else(|| op_failed_error(op_idx, "set_style", ErrorCode::NodeNotFound, &path, format!("node {} not found", node_id.0)))?;
        let mut updated = cur.clone();
        if prop == StyleProperty::Fill {
            // The style's paints replace the node's, so a variable bound to the old ones would keep drawing over them.
            designos_doc::detach_fill_paints(&mut updated);
        }
        updated.style_refs.insert(ref_slot.into(), resolved_key.clone());
        apply_style_values(&mut updated, st, &resolved_key);
        if prop == StyleProperty::Text {
            if let NodeKind::Text(tp) = &updated.kind {
                match tp.auto_resize {
                    TextAutoResize::WidthAndHeight => {
                        updated.size = designos_doc::text_measure::intrinsic_text_size(tp, None);
                    }
                    TextAutoResize::Height => {
                        updated.size.y = designos_doc::text_measure::intrinsic_text_size(tp, Some(updated.size.x.max(20.0))).y;
                    }
                    TextAutoResize::None | TextAutoResize::Truncate => {}
                }
            }
        }
        if &updated != cur {
            cmds.push(Command::Update { node: Box::new(updated) });
        }
    }

    if cmds.is_empty() {
        return Ok((cmds, 0, first_applied_node));
    }
    let count = (op.upsert.len() + op.delete.len() + op.apply.len()).max(1);
    Ok((cmds, count, first_applied_node))
}

fn build_name_to_id_map(doc: &Document, created_roots: &[CreatedRootRecord], all_created_ids: &BTreeSet<NodeId>) -> Map<String, Value> {
    let mut out = Map::new();

    // Count root names among unaliased roots so duplicate root names get [1], [2], [3], ...
    let mut root_name_totals: BTreeMap<String, usize> = BTreeMap::new();
    for cr in created_roots {
        if cr.alias.is_none() {
            if let Some(n) = doc.get(cr.root_id) {
                if !n.name.is_empty() {
                    *root_name_totals.entry(n.name.clone()).or_insert(0) += 1;
                }
            }
        }
    }
    let mut root_name_seen: BTreeMap<String, usize> = BTreeMap::new();

    for cr in created_roots {
        let Some(root_node) = doc.get(cr.root_id) else { continue };
        let root_key = if let Some(alias) = &cr.alias {
            alias.clone()
        } else if !root_node.name.is_empty() {
            let occ = root_name_seen.entry(root_node.name.clone()).or_insert(0);
            *occ += 1;
            if root_name_totals.get(&root_node.name).copied().unwrap_or(0) > 1 {
                format!("{}[{}]", root_node.name, *occ)
            } else {
                root_node.name.clone()
            }
        } else {
            continue;
        };

        out.insert(root_key.clone(), json!(cr.root_id.0));
        walk_named_descendants(doc, cr.root_id, &root_key, &cr.created_ids, all_created_ids, &mut out);
    }

    out
}

fn walk_named_descendants(
    doc: &Document,
    parent_id: NodeId,
    prefix: &str,
    root_created_ids: &BTreeSet<NodeId>,
    all_created_ids: &BTreeSet<NodeId>,
    out: &mut Map<String, Value>,
) {
    let Some(parent) = doc.get(parent_id) else { return };
    let mut name_totals: BTreeMap<String, usize> = BTreeMap::new();
    for &cid in &parent.children {
        if (root_created_ids.contains(&cid) || all_created_ids.contains(&cid)) && doc.get(cid).is_some() {
            if let Some(cn) = doc.get(cid) {
                if !cn.name.is_empty() {
                    *name_totals.entry(cn.name.clone()).or_insert(0) += 1;
                }
            }
        }
    }
    let mut name_seen: BTreeMap<String, usize> = BTreeMap::new();
    for &cid in &parent.children {
        if !(root_created_ids.contains(&cid) || all_created_ids.contains(&cid)) {
            continue;
        }
        let Some(cn) = doc.get(cid) else { continue };
        let seg = if !cn.name.is_empty() {
            let occ = name_seen.entry(cn.name.clone()).or_insert(0);
            *occ += 1;
            if name_totals.get(&cn.name).copied().unwrap_or(0) > 1 {
                format!("{}[{}]", cn.name, *occ)
            } else {
                cn.name.clone()
            }
        } else {
            continue;
        };
        let path = format!("{prefix}/{seg}");
        out.insert(path.clone(), json!(cid.0));
        walk_named_descendants(doc, cid, &path, root_created_ids, all_created_ids, out);
    }
}

/// Executes `designos_apply_batch` on `engine` (§3.1–§3.5).
pub fn apply(engine: &mut Engine, args: Value) -> Result<Value, ToolError> {
    let batch_args: BatchArgs = serde_json::from_value(args).map_err(|e| crate::tools::errors::from_serde_error(&e, "", false))?;
    crate::tools::ledger::validate_idempotency_key_opt(&batch_args.idempotency_key)?;

    // Step 1: Parse and validate every op's arguments, bounds, and @<as> references before running anything.
    let ops = parse_and_validate_ops(batch_args.ops)?;

    // Step 2: Clone the document ONCE into a trial and run the ops in order.
    let mut trial = engine.doc.clone();
    let recording_live = engine.live_ops.is_some();
    let mut collected_cmds: Vec<Command> = Vec::with_capacity(ops.len());
    let mut recorded_live_ops: Vec<LiveOp> = Vec::new();
    let mut relayout_targets: Vec<NodeId> = Vec::new();
    let mut alias_roots: BTreeMap<String, NodeId> = BTreeMap::new();
    let mut prev_node: Option<NodeId> = None;
    let mut created_roots: Vec<CreatedRootRecord> = Vec::new();
    let mut all_created_ids: BTreeSet<NodeId> = BTreeSet::new();
    let mut created_root_ids: Vec<u64> = Vec::new();
    let mut created_count: usize = 0;
    let mut changed = ChangedCounts::default();
    let mut touched_records: Vec<TouchedRootRecord> = Vec::with_capacity(ops.len());
    let mut deleted_ids: Vec<NodeId> = Vec::new();
    let mut measured_with_fallback: Vec<u64> = Vec::new();
    let mut batch_warnings: Vec<designos_doc::SpecWarning> = Vec::new();

    let default_page = engine.resolve_default_page().ok();

    for (op_idx, op) in ops.iter().enumerate() {
        let op_name = op.op_name();
        let op_path = format!("/ops/{op_idx}");

        match op {
            ParsedOp::ApplySpec(a) => {
                let parent = match &a.parent {
                    Some(NodeRef::Id(0)) | None => {
                        default_page.ok_or_else(|| op_failed_error(op_idx, op_name, ErrorCode::PageNotFound, &op_path, "Document has no pages"))?
                    }
                    Some(r) => resolve_node_ref(&trial, &alias_roots, prev_node, r, op_idx, op_name, "parent")?,
                };
                check_lease_guard(&trial, engine.owned_frames.as_deref(), parent, op_idx, op_name)?;
                let pnode = trial
                    .get(parent)
                    .ok_or_else(|| op_failed_error(op_idx, op_name, ErrorCode::NodeNotFound, &op_path, format!("node {} not found", parent.0)))?;
                if !pnode.kind.is_container() {
                    return Err(op_failed_error(op_idx, op_name, ErrorCode::NotAContainer, &op_path, "parent must identify a container"));
                }
                let prev_children = pnode.children.clone();
                let data_before = SpecDataSnapshot::of(&trial);
                let report = trial.apply_spec_with_report(parent, &a.spec).map_err(|se| {
                    let cause = crate::tools::errors::from_spec_error(designos_doc::SpecError::from(se), &format!("{op_path}/spec"));
                    ToolError::batch_op_failed(op_idx, op_name, cause)
                })?;
                for fid in &report.measured_with_fallback {
                    if !measured_with_fallback.contains(fid) {
                        measured_with_fallback.push(*fid);
                    }
                }
                for w in &report.warnings {
                    if w.code != "TEXT_MEASURED_WITH_FALLBACK_FONT" || !batch_warnings.iter().any(|bw| bw.code == "TEXT_MEASURED_WITH_FALLBACK_FONT") {
                        batch_warnings.push(w.clone());
                    }
                }
                let root_id = report.root_node_id;
                // A spec flag without a run id gets the writer's, as the single apply_spec does; Stop clears by run id only.
                for id in report.in_progress.keys() {
                    if trial.in_progress.get(id).is_some_and(|ip| ip.run_id.is_none()) {
                        if let (Some(rid), Some(ip)) = (engine.resolve_agent_run_id(None), trial.in_progress.get_mut(id)) {
                            ip.run_id = Some(rid);
                        }
                    }
                }
                // Variables, styles and library state the spec wrote (an `$easeui.*` token creates its collection) are
                // committed as their own commands ahead of the insert, so the bound nodes never outlive their variables
                // and one undo step removes both.
                let data_cmds = data_before.commands_to(&SpecDataSnapshot::of(&trial));
                if !data_cmds.is_empty() {
                    data_before.restore(&mut trial);
                    for cmd in data_cmds {
                        if recording_live {
                            let ops =
                                live_ops_for_command(&trial, &cmd).map_err(|e| op_failed_error(op_idx, op_name, ErrorCode::InternalError, &op_path, e))?;
                            recorded_live_ops.extend(ops);
                        }
                        cmd.clone().apply(&mut trial).map_err(|e| op_failed_error(op_idx, op_name, ErrorCode::InternalError, &op_path, e.to_string()))?;
                        collected_cmds.push(cmd);
                    }
                }
                let target_idx = a.index.unwrap_or(prev_children.len()).min(prev_children.len());
                if target_idx < prev_children.len() {
                    if let Some(p_mut) = trial.get_mut(parent) {
                        p_mut.children.retain(|&c| c != root_id);
                        p_mut.children.insert(target_idx, root_id);
                    }
                }
                relayout_for_node(&mut trial, parent);
                relayout_for_node(&mut trial, root_id);

                let mut nodes = Vec::new();
                let mut subtree_ids = BTreeSet::new();
                trial.walk(root_id, |n, _| {
                    subtree_ids.insert(n.id);
                    nodes.push(n.clone());
                });
                if created_count + nodes.len() > MAX_BATCH_CREATED_NODES {
                    return Err(batch_error(
                        ErrorCode::BatchTooManyOps,
                        Some(op_idx),
                        Some(op_name),
                        &op_path,
                        format!("batch creates over {MAX_BATCH_CREATED_NODES} nodes"),
                    ));
                }

                if recording_live {
                    let mut live_nodes = nodes.clone();
                    if let Some(first) = live_nodes.first_mut() {
                        first.parent = Some(parent);
                    }
                    recorded_live_ops.push(LiveOp::Insert { parent, at: SiblingHint::at(&prev_children, target_idx), nodes: live_nodes });
                    for &sid in &subtree_ids {
                        if let Some(ip) = trial.in_progress.get(&sid) {
                            recorded_live_ops.push(LiveOp::SetInProgress { node: sid, state: Some(ip.clone()) });
                        }
                    }
                }

                created_root_ids.push(root_id.0);
                created_count += nodes.len();
                changed.inserted += nodes.len();
                all_created_ids.extend(subtree_ids.iter().copied());
                if let Some(alias) = &a.alias {
                    alias_roots.insert(alias.clone(), root_id);
                }
                prev_node = Some(root_id);
                created_roots.push(CreatedRootRecord { alias: a.alias.clone(), root_id, created_ids: subtree_ids });
                collected_cmds.push(Command::Insert { parent, index: target_idx, nodes });
                relayout_targets.push(parent);
                relayout_targets.push(root_id);
                let before_bounds = node_page_bounds(&trial, root_id);
                touched_records.push(TouchedRootRecord { op: op_idx, node_id: root_id, before_bounds });
            }
            ParsedOp::Insert(a) => {
                let parent = match &a.parent {
                    None => default_page.ok_or_else(|| op_failed_error(op_idx, op_name, ErrorCode::PageNotFound, &op_path, "Document has no pages"))?,
                    Some(r) => resolve_node_ref(&trial, &alias_roots, prev_node, r, op_idx, op_name, "parent")?,
                };
                check_lease_guard(&trial, engine.owned_frames.as_deref(), parent, op_idx, op_name)?;
                if created_count + 1 > MAX_BATCH_CREATED_NODES {
                    return Err(batch_error(
                        ErrorCode::BatchTooManyOps,
                        Some(op_idx),
                        Some(op_name),
                        &op_path,
                        format!("batch creates over {MAX_BATCH_CREATED_NODES} nodes"),
                    ));
                }
                let (id, target_idx, mut node) =
                    build_created_node(&mut trial, parent, a.index, &a.kind, a.name.clone(), a.text.clone(), a.x, a.y, a.width, a.height)
                        .map_err(|e| wrap_op_err(op_idx, op_name, &op_path, e))?;
                if let NodeKind::Text(tp) = &node.kind {
                    if (a.width.is_none() || a.height.is_none())
                        && designos_doc::text_measure::uses_fallback_font(tp)
                        && !measured_with_fallback.contains(&id.0)
                    {
                        measured_with_fallback.push(id.0);
                    }
                }
                write_fill(&mut trial, &mut node, parent, FillTarget::NewLayer, a.fill.as_deref(), a.fills.clone(), &mut Vec::new())
                    .map_err(|e| wrap_op_err(op_idx, op_name, &op_path, e))?;

                let cmd = Command::Insert { parent, index: target_idx, nodes: vec![node] };
                if recording_live {
                    let ops = live_ops_for_command(&trial, &cmd).map_err(|e| op_failed_error(op_idx, op_name, ErrorCode::InternalError, &op_path, e))?;
                    recorded_live_ops.extend(ops);
                }
                cmd.clone().apply(&mut trial).map_err(|e| op_failed_error(op_idx, op_name, ErrorCode::InternalError, &op_path, e.to_string()))?;
                relayout_for_node(&mut trial, id);

                created_root_ids.push(id.0);
                created_count += 1;
                changed.inserted += 1;
                all_created_ids.insert(id);
                let mut set = BTreeSet::new();
                set.insert(id);
                if let Some(alias) = &a.alias {
                    alias_roots.insert(alias.clone(), id);
                }
                prev_node = Some(id);
                created_roots.push(CreatedRootRecord { alias: a.alias.clone(), root_id: id, created_ids: set });
                collected_cmds.push(cmd);
                relayout_targets.push(id);
                let before_bounds = node_page_bounds(&trial, id);
                touched_records.push(TouchedRootRecord { op: op_idx, node_id: id, before_bounds });
            }
            ParsedOp::Update(a) => {
                let id = resolve_node_ref(&trial, &alias_roots, prev_node, &a.node, op_idx, op_name, "node")?;
                check_lease_guard(&trial, engine.owned_frames.as_deref(), id, op_idx, op_name)?;
                let (updated_node, node_changed, _warnings) = build_updated_node(
                    &mut trial,
                    id,
                    UpdateFields {
                        name: a.name.clone(),
                        text: a.text.clone(),
                        x: a.x,
                        y: a.y,
                        width: a.width,
                        height: a.height,
                        visible: a.visible,
                        opacity: a.opacity,
                        fill: a.fill.clone(),
                        fills: a.fills.clone(),
                    },
                )
                .map_err(|e| wrap_op_err(op_idx, op_name, &op_path, e))?;

                let before_bounds = node_page_bounds(&trial, id);
                if let Some(alias) = &a.alias {
                    alias_roots.insert(alias.clone(), id);
                }
                prev_node = Some(id);
                if node_changed {
                    let cmd = Command::Update { node: Box::new(updated_node) };
                    if recording_live {
                        let ops = live_ops_for_command(&trial, &cmd).map_err(|e| op_failed_error(op_idx, op_name, ErrorCode::InternalError, &op_path, e))?;
                        recorded_live_ops.extend(ops);
                    }
                    cmd.clone().apply(&mut trial).map_err(|e| op_failed_error(op_idx, op_name, ErrorCode::InternalError, &op_path, e.to_string()))?;
                    relayout_for_node(&mut trial, id);
                    collected_cmds.push(cmd);
                    relayout_targets.push(id);
                    changed.updated += 1;
                }
                touched_records.push(TouchedRootRecord { op: op_idx, node_id: id, before_bounds });
            }
            ParsedOp::Move(a) => {
                let id = resolve_node_ref(&trial, &alias_roots, prev_node, &a.node, op_idx, op_name, "node")?;
                let new_parent = resolve_node_ref(&trial, &alias_roots, prev_node, &a.parent, op_idx, op_name, "parent")?;
                check_lease_guard(&trial, engine.owned_frames.as_deref(), id, op_idx, op_name)?;
                check_lease_guard(&trial, engine.owned_frames.as_deref(), new_parent, op_idx, op_name)?;

                let cur_node =
                    trial.get(id).ok_or_else(|| op_failed_error(op_idx, op_name, ErrorCode::NodeNotFound, &op_path, format!("node {} not found", id.0)))?;
                let Some(old_parent) = cur_node.parent else {
                    return Err(op_failed_error(op_idx, op_name, ErrorCode::PageRootProtected, &op_path, "page roots cannot be moved by this tool"));
                };
                let pnode = trial
                    .get(new_parent)
                    .ok_or_else(|| op_failed_error(op_idx, op_name, ErrorCode::NodeNotFound, &op_path, format!("node {} not found", new_parent.0)))?;
                if !pnode.kind.is_container() {
                    return Err(op_failed_error(op_idx, op_name, ErrorCode::NotAContainer, &op_path, "parent must identify a container"));
                }
                if id == new_parent || trial.is_ancestor(id, new_parent) {
                    return Err(op_failed_error(
                        op_idx,
                        op_name,
                        ErrorCode::MoveCycle,
                        &op_path,
                        format!("cannot move node {} into its own subtree ({})", id.0, new_parent.0),
                    ));
                }
                // a layer that holds an instance of a main does not go into that main, nor into a layer in it
                if designos_doc::reparent_would_hold_itself(&trial, &[id], new_parent) {
                    return Err(op_failed_error(
                        op_idx,
                        op_name,
                        ErrorCode::MoveCycle,
                        &op_path,
                        format!("cannot move node {} into {}: that would make a component hold an instance of itself", id.0, new_parent.0),
                    ));
                }

                let siblings_without_id = pnode.children.iter().filter(|&&c| c != id).count();
                let target_idx = a.index.unwrap_or(siblings_without_id).min(siblings_without_id);
                let cur_idx = trial.get(old_parent).and_then(|p| p.children.iter().position(|&c| c == id));
                let before_bounds = node_page_bounds(&trial, id);

                if let Some(alias) = &a.alias {
                    alias_roots.insert(alias.clone(), id);
                }
                prev_node = Some(id);
                if old_parent != new_parent || cur_idx != Some(target_idx) {
                    let cmd = Command::Reparent { id, parent: new_parent, index: target_idx };
                    if recording_live {
                        let ops = live_ops_for_command(&trial, &cmd).map_err(|e| op_failed_error(op_idx, op_name, ErrorCode::InternalError, &op_path, e))?;
                        recorded_live_ops.extend(ops);
                    }
                    cmd.clone().apply(&mut trial).map_err(|e| op_failed_error(op_idx, op_name, ErrorCode::MoveCycle, &op_path, e.to_string()))?;
                    relayout_for_node(&mut trial, old_parent);
                    relayout_for_node(&mut trial, new_parent);
                    collected_cmds.push(cmd);
                    relayout_targets.push(old_parent);
                    relayout_targets.push(new_parent);
                    changed.moved += 1;
                }
                touched_records.push(TouchedRootRecord { op: op_idx, node_id: id, before_bounds });
            }
            ParsedOp::Delete(a) => {
                let id = resolve_node_ref(&trial, &alias_roots, prev_node, &a.node, op_idx, op_name, "node")?;
                check_lease_guard(&trial, engine.owned_frames.as_deref(), id, op_idx, op_name)?;
                let cur_node =
                    trial.get(id).ok_or_else(|| op_failed_error(op_idx, op_name, ErrorCode::NodeNotFound, &op_path, format!("node {} not found", id.0)))?;
                let Some(former_parent) = cur_node.parent else {
                    return Err(op_failed_error(op_idx, op_name, ErrorCode::PageRootProtected, &op_path, "page roots cannot be deleted by this tool"));
                };
                let deleted_bounds = node_page_bounds(&trial, id);
                let parent_bounds_before = {
                    let pb = node_page_bounds(&trial, former_parent);
                    if pb.w == 0.0 && pb.h == 0.0 {
                        deleted_bounds
                    } else {
                        pb
                    }
                };
                let cmd = Command::Remove { id };
                if recording_live {
                    let ops = live_ops_for_command(&trial, &cmd).map_err(|e| op_failed_error(op_idx, op_name, ErrorCode::InternalError, &op_path, e))?;
                    recorded_live_ops.extend(ops);
                }
                cmd.clone().apply(&mut trial).map_err(|e| op_failed_error(op_idx, op_name, ErrorCode::InternalError, &op_path, e.to_string()))?;
                relayout_for_node(&mut trial, former_parent);
                collected_cmds.push(cmd);
                relayout_targets.push(former_parent);
                changed.deleted += 1;
                deleted_ids.push(id);
                touched_records.push(TouchedRootRecord { op: op_idx, node_id: former_parent, before_bounds: parent_bounds_before });
            }
            ParsedOp::SetVariables(a) => {
                check_document_lease_guard(engine.owned_frames.as_deref(), op_idx, op_name)?;
                let (cmd_opt, count) = apply_set_variables_op(&trial, a, op_idx)?;
                if let Some(cmd) = cmd_opt {
                    if recording_live {
                        let ops = live_ops_for_command(&trial, &cmd).map_err(|e| op_failed_error(op_idx, op_name, ErrorCode::InternalError, &op_path, e))?;
                        recorded_live_ops.extend(ops);
                    }
                    cmd.clone().apply(&mut trial).map_err(|e| op_failed_error(op_idx, op_name, ErrorCode::InternalError, &op_path, e.to_string()))?;
                    if let Some(p) = default_page {
                        if let Some(pnode) = trial.get(p) {
                            let children = pnode.children.clone();
                            for cid in children {
                                relayout_for_node(&mut trial, cid);
                                relayout_targets.push(cid);
                            }
                        }
                    }
                    collected_cmds.push(cmd);
                    changed.variables += count;
                }
                let root_node = default_page.unwrap_or(NodeId(1));
                let before_bounds = node_page_bounds(&trial, root_node);
                touched_records.push(TouchedRootRecord { op: op_idx, node_id: root_node, before_bounds });
            }
            ParsedOp::SetStyle(a) => {
                let (cmds, count, first_node) = apply_set_style_op(&trial, &alias_roots, prev_node, engine.owned_frames.as_deref(), a, op_idx)?;
                for cmd in cmds {
                    if recording_live {
                        let ops = live_ops_for_command(&trial, &cmd).map_err(|e| op_failed_error(op_idx, op_name, ErrorCode::InternalError, &op_path, e))?;
                        recorded_live_ops.extend(ops);
                    }
                    cmd.clone().apply(&mut trial).map_err(|e| op_failed_error(op_idx, op_name, ErrorCode::InternalError, &op_path, e.to_string()))?;
                    collected_cmds.push(cmd);
                }
                if let Some(nid) = first_node {
                    relayout_for_node(&mut trial, nid);
                    relayout_targets.push(nid);
                }
                changed.styles += count;
                let root_node = first_node.or(default_page).unwrap_or(NodeId(1));
                let before_bounds = node_page_bounds(&trial, root_node);
                touched_records.push(TouchedRootRecord { op: op_idx, node_id: root_node, before_bounds });
            }
            ParsedOp::SetSlot(a) => {
                let id = resolve_node_ref(&trial, &alias_roots, prev_node, &a.node, op_idx, op_name, "node")?;
                check_lease_guard(&trial, engine.owned_frames.as_deref(), id, op_idx, op_name)?;
                let cur_node = trial
                    .get(id)
                    .ok_or_else(|| op_failed_error(op_idx, op_name, ErrorCode::NodeNotFound, &op_path, format!("node {} not found", id.0)))?
                    .clone();
                let comp_id = designos_doc::instance_component(&cur_node).ok_or_else(|| {
                    op_failed_error(
                        op_idx,
                        op_name,
                        ErrorCode::InvalidNodeKind,
                        &format!("{op_path}/node"),
                        format!("node {} is not a component instance", id.0),
                    )
                })?;
                let mut slots_map = BTreeMap::new();
                slots_map.insert(a.slot.clone(), a.value.clone());
                let spec_node = designos_doc::NodeSpec {
                    kind: "instance".to_string(),
                    component: Some(format!("node:{}", comp_id.0)),
                    slots: slots_map,
                    ..Default::default()
                };
                let snapshots = engine.collect_library_snapshots(&spec_node);
                let mut temp_doc = trial.clone();
                let parent = cur_node.parent.or(default_page).unwrap_or(NodeId(1));
                let (_cmd, built_id) = designos_doc::apply_spec_command(&mut temp_doc, parent, &spec_node, &snapshots).map_err(|se| {
                    let cause = crate::tools::errors::from_spec_error(se, &format!("{op_path}/value"));
                    ToolError::batch_op_failed(op_idx, op_name, cause)
                })?;
                for fid in &built_id.measured_with_fallback {
                    if !measured_with_fallback.contains(fid) {
                        measured_with_fallback.push(*fid);
                    }
                }
                if let Some(alias) = &a.alias {
                    alias_roots.insert(alias.clone(), id);
                }
                prev_node = Some(id);
                let _ = built_id;
                let before_bounds = node_page_bounds(&trial, id);
                touched_records.push(TouchedRootRecord { op: op_idx, node_id: id, before_bounds });
            }
            ParsedOp::SetInProgress(a) => {
                let flag = a.value.or(a.in_progress).unwrap_or(false);
                let mut target_ids: Vec<NodeId> = Vec::new();
                if let Some(list) = &a.nodes {
                    for (n_idx, r) in list.iter().enumerate() {
                        let id = resolve_node_ref(&trial, &alias_roots, prev_node, r, op_idx, op_name, &format!("nodes/{n_idx}"))?;
                        if !target_ids.contains(&id) {
                            target_ids.push(id);
                        }
                    }
                }
                if let Some(single) = &a.node {
                    let id = resolve_node_ref(&trial, &alias_roots, prev_node, single, op_idx, op_name, "node")?;
                    if !target_ids.contains(&id) {
                        target_ids.push(id);
                    }
                }
                let effective_run_id = engine.resolve_agent_run_id(a.run_id.as_deref());
                let effective_agent = engine.resolve_agent_name(a.agent.as_deref());
                let effective_status = a.status.as_ref().map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
                let effective_stale_s = a.stale_after_s.unwrap_or_else(designos_doc::default_in_progress_stale_s);

                for &id in &target_ids {
                    check_lease_guard(&trial, engine.owned_frames.as_deref(), id, op_idx, op_name)?;
                    if flag {
                        if let Err(root_opt) = trial.is_in_progress_root(id) {
                            let msg = match root_opt {
                                Some(r) => format!("node {} is not a root frame; target root frame {} instead", id.0, r.0),
                                None => format!("node {} is not a root frame", id.0),
                            };
                            let cause = ToolError::new(ErrorCode::InProgressTargetNotRoot, &op_path, msg)
                                .with_received(json!({"node_id": id.0, "root_frame_id": root_opt.map(|r| r.0)}))
                                .with_valid_options(root_opt.into_iter().map(|r| r.0.to_string()));
                            return Err(ToolError::batch_op_failed(op_idx, op_name, cause));
                        }
                        if !a.force {
                            if let Some(existing) = trial.get_in_progress(id) {
                                if let Some(ex_run) = &existing.run_id {
                                    if effective_run_id.as_deref() != Some(ex_run.as_str()) {
                                        let msg = format!("node {} is already flagged in_progress by run '{ex_run}'", id.0);
                                        let cause = ToolError::new(ErrorCode::NodeInProgressByOtherRun, &op_path, msg).with_received(json!({
                                            "node_id": id.0,
                                            "existing_run_id": ex_run,
                                            "existing_agent": existing.agent,
                                        }));
                                        return Err(ToolError::batch_op_failed(op_idx, op_name, cause));
                                    }
                                }
                            }
                        }
                    }
                }

                let now = designos_doc::now_ms();
                for &id in &target_ids {
                    let state = flag.then(|| designos_doc::InProgress {
                        run_id: effective_run_id.clone(),
                        agent: effective_agent.clone(),
                        since_ms: now,
                        stale_after_s: Some(effective_stale_s),
                        status: effective_status.clone(),
                    });
                    let prev = trial.set_in_progress(id, state.clone());
                    if prev != state {
                        changed.updated += 1;
                        if recording_live {
                            recorded_live_ops.push(LiveOp::SetInProgress { node: id, state });
                        }
                    }
                    let before_bounds = node_page_bounds(&trial, id);
                    touched_records.push(TouchedRootRecord { op: op_idx, node_id: id, before_bounds });
                }
                if let (Some(alias), Some(&first_id)) = (&a.alias, target_ids.first()) {
                    alias_roots.insert(alias.clone(), first_id);
                }
                prev_node = target_ids.first().copied();
            }
        }
    }

    trial.flush_pending_relayout();

    // Build reply fields from the finished trial document (§3.3).
    let name_to_id = build_name_to_id_map(&trial, &created_roots, &all_created_ids);
    let mut touched_roots_json = Vec::with_capacity(touched_records.len());
    let mut union_bounds: Option<Rect> = None;
    let mut check_ids: Vec<NodeId> = Vec::with_capacity(touched_records.len() + created_root_ids.len() + deleted_ids.len());

    for rec in &touched_records {
        check_ids.push(rec.node_id);
        let bounds = match trial.get(rec.node_id) {
            Some(n) if !(matches!(n.kind, NodeKind::Page { .. }) && n.children.is_empty()) => {
                let post = node_page_bounds(&trial, rec.node_id);
                if post.w == 0.0 && post.h == 0.0 && (rec.before_bounds.w > 0.0 || rec.before_bounds.h > 0.0) {
                    rec.before_bounds
                } else {
                    post
                }
            }
            _ => rec.before_bounds,
        };
        union_bounds = Some(match union_bounds {
            Some(u) => u.union(&bounds),
            None => bounds,
        });
        touched_roots_json.push(json!({
            "op": rec.op,
            "node_id": rec.node_id.0,
            "bounds": bounds_json(bounds),
        }));
    }
    check_ids.extend(all_created_ids.iter().copied());
    check_ids.extend(deleted_ids.iter().copied());

    let touched_bounds_rect = union_bounds.unwrap_or(Rect::new(0.0, 0.0, 0.0, 0.0));
    let undo_label = agent_undo_label();
    let touched_root_ids: Vec<NodeId> = touched_records.iter().map(|r| r.node_id).collect();
    let baseline = crate::tools::issues::PreCallBaseline::capture_from_post_doc(
        &engine.doc,
        &trial,
        &touched_root_ids,
        &engine.session_options.issues,
        batch_args.lint,
        engine.lint_env(),
    );

    let mut reply = json!({
        "dry_run": batch_args.dry_run,
        "created": created_root_ids,
        "created_count": created_count,
        "name_to_id": name_to_id,
        "changed": changed,
        "touched_roots": touched_roots_json,
        "touched_bounds": bounds_json(touched_bounds_rect),
        "undo_label": undo_label,
    });
    if !measured_with_fallback.is_empty() {
        reply["measured_with_fallback"] = json!(measured_with_fallback);
        if !batch_warnings.iter().any(|w| w.code == "TEXT_MEASURED_WITH_FALLBACK_FONT") {
            batch_warnings.push(designos_doc::SpecWarning {
                code: "TEXT_MEASURED_WITH_FALLBACK_FONT".to_string(),
                path: "ops".to_string(),
                token: None,
                replacement: None,
                message: Some("One or more text nodes were measured with a fallback font because the requested font_family is not loaded.".to_string()),
            });
        }
    }
    if !batch_warnings.is_empty() {
        reply["warnings"] = json!(batch_warnings);
    }

    if batch_args.diff_image {
        crate::tools::screenshot::attach_diff_to_reply(&engine.doc, &trial, &check_ids, &mut reply);
    }

    // Step 4: Dry run returns the trial result without committing or publishing (§3.2.4).
    // Step 5 & Edge case 5: Empty effective change skips commit, undo entry, and patch line (§8.5).
    let in_progress_changed = trial.in_progress != engine.doc.in_progress;
    if !batch_args.dry_run {
        let cmds_committed = !collected_cmds.is_empty() && changed.any();
        if cmds_committed {
            engine
                .history
                .exec(&mut engine.doc, Command::Batch(collected_cmds))
                .map_err(|e| batch_error(ErrorCode::InternalError, None, None, "", e.to_string()))?;
            for target in &relayout_targets {
                relayout_for_node(&mut engine.doc, *target);
            }
            engine.settle_layout();
            engine.live_patch_label = Some(undo_label.clone());
        }
        if in_progress_changed {
            engine.doc.in_progress = trial.in_progress.clone();
            engine.doc.prune_in_progress();
        }
        if cmds_committed || in_progress_changed {
            engine.revision = engine.revision.wrapping_add(1);
            engine.record(recorded_live_ops);
            debug_assert_eq!(
                batch_state_hash(&engine.doc, &check_ids),
                batch_state_hash(&trial, &check_ids),
                "committed document hash must equal trial document hash"
            );
        }
    }

    trial.flush_pending_relayout();
    crate::tools::issues::attach_issues_block(
        &trial,
        &touched_root_ids,
        &engine.session_options.issues,
        batch_args.lint,
        &baseline,
        engine.lint_env(),
        &[],
        &mut reply,
    );
    Ok(reply)
}

/// Zero-allocation streaming hash of the document state (never calls `to_json`, CLAUDE.md §4.1).
/// On documents up to 5,000 nodes, hashes the entire node map; on larger documents, hashes
/// document metadata, variables, styles, and all `touched_ids` in $O(|touched|)$.
pub fn batch_state_hash(doc: &Document, touched_ids: &[NodeId]) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    doc.name.hash(&mut h);
    doc.nodes.len().hash(&mut h);
    doc.pages.hash(&mut h);
    if doc.nodes.len() <= 5_000 {
        let _ = (&doc.nodes, &doc.styles, &doc.style_meta, &doc.variables, &doc.variable_collections, &doc.in_progress).serialize(StreamHashSerializer(&mut h));
    } else {
        let _ = (&doc.styles, &doc.style_meta, &doc.variables, &doc.variable_collections, &doc.in_progress).serialize(StreamHashSerializer(&mut h));
        for &id in touched_ids {
            id.0.hash(&mut h);
            let _ = doc.get(id).serialize(StreamHashSerializer(&mut h));
        }
    }
    h.finish()
}

#[derive(Debug)]
struct StreamHashError;
impl std::fmt::Display for StreamHashError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("StreamHashError")
    }
}
impl std::error::Error for StreamHashError {}
impl serde::ser::Error for StreamHashError {
    fn custom<T: std::fmt::Display>(_: T) -> Self {
        Self
    }
}

struct StreamHashSerializer<'a>(&'a mut std::collections::hash_map::DefaultHasher);

impl<'a> serde::Serializer for StreamHashSerializer<'a> {
    type Ok = ();
    type Error = StreamHashError;
    type SerializeSeq = Self;
    type SerializeTuple = Self;
    type SerializeTupleStruct = Self;
    type SerializeTupleVariant = Self;
    type SerializeMap = Self;
    type SerializeStruct = Self;
    type SerializeStructVariant = Self;

    fn serialize_bool(self, v: bool) -> Result<(), Self::Error> {
        v.hash(self.0);
        Ok(())
    }
    fn serialize_i8(self, v: i8) -> Result<(), Self::Error> {
        v.hash(self.0);
        Ok(())
    }
    fn serialize_i16(self, v: i16) -> Result<(), Self::Error> {
        v.hash(self.0);
        Ok(())
    }
    fn serialize_i32(self, v: i32) -> Result<(), Self::Error> {
        v.hash(self.0);
        Ok(())
    }
    fn serialize_i64(self, v: i64) -> Result<(), Self::Error> {
        v.hash(self.0);
        Ok(())
    }
    fn serialize_u8(self, v: u8) -> Result<(), Self::Error> {
        v.hash(self.0);
        Ok(())
    }
    fn serialize_u16(self, v: u16) -> Result<(), Self::Error> {
        v.hash(self.0);
        Ok(())
    }
    fn serialize_u32(self, v: u32) -> Result<(), Self::Error> {
        v.hash(self.0);
        Ok(())
    }
    fn serialize_u64(self, v: u64) -> Result<(), Self::Error> {
        v.hash(self.0);
        Ok(())
    }
    fn serialize_f32(self, v: f32) -> Result<(), Self::Error> {
        let bits = if v == 0.0 { 0u32 } else { v.to_bits() };
        bits.hash(self.0);
        Ok(())
    }
    fn serialize_f64(self, v: f64) -> Result<(), Self::Error> {
        let bits = if v == 0.0 { 0u64 } else { v.to_bits() };
        bits.hash(self.0);
        Ok(())
    }
    fn serialize_char(self, v: char) -> Result<(), Self::Error> {
        v.hash(self.0);
        Ok(())
    }
    fn serialize_str(self, v: &str) -> Result<(), Self::Error> {
        v.hash(self.0);
        Ok(())
    }
    fn serialize_bytes(self, v: &[u8]) -> Result<(), Self::Error> {
        v.hash(self.0);
        Ok(())
    }
    fn serialize_none(self) -> Result<(), Self::Error> {
        0u8.hash(self.0);
        Ok(())
    }
    fn serialize_some<T: ?Sized + Serialize>(self, value: &T) -> Result<(), Self::Error> {
        1u8.hash(self.0);
        value.serialize(self)
    }
    fn serialize_unit(self) -> Result<(), Self::Error> {
        Ok(())
    }
    fn serialize_unit_struct(self, _: &'static str) -> Result<(), Self::Error> {
        Ok(())
    }
    fn serialize_unit_variant(self, _: &'static str, idx: u32, variant: &'static str) -> Result<(), Self::Error> {
        idx.hash(self.0);
        variant.hash(self.0);
        Ok(())
    }
    fn serialize_newtype_struct<T: ?Sized + Serialize>(self, _: &'static str, value: &T) -> Result<(), Self::Error> {
        value.serialize(self)
    }
    fn serialize_newtype_variant<T: ?Sized + Serialize>(self, _: &'static str, idx: u32, variant: &'static str, value: &T) -> Result<(), Self::Error> {
        idx.hash(self.0);
        variant.hash(self.0);
        value.serialize(self)
    }
    fn serialize_seq(self, len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        len.hash(self.0);
        Ok(self)
    }
    fn serialize_tuple(self, len: usize) -> Result<Self::SerializeTuple, Self::Error> {
        len.hash(self.0);
        Ok(self)
    }
    fn serialize_tuple_struct(self, _: &'static str, len: usize) -> Result<Self::SerializeTupleStruct, Self::Error> {
        len.hash(self.0);
        Ok(self)
    }
    fn serialize_tuple_variant(self, _: &'static str, idx: u32, variant: &'static str, len: usize) -> Result<Self::SerializeTupleVariant, Self::Error> {
        idx.hash(self.0);
        variant.hash(self.0);
        len.hash(self.0);
        Ok(self)
    }
    fn serialize_map(self, len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        len.hash(self.0);
        Ok(self)
    }
    fn serialize_struct(self, _: &'static str, len: usize) -> Result<Self::SerializeStruct, Self::Error> {
        len.hash(self.0);
        Ok(self)
    }
    fn serialize_struct_variant(self, _: &'static str, idx: u32, variant: &'static str, len: usize) -> Result<Self::SerializeStructVariant, Self::Error> {
        idx.hash(self.0);
        variant.hash(self.0);
        len.hash(self.0);
        Ok(self)
    }
}

impl<'a> serde::ser::SerializeSeq for StreamHashSerializer<'a> {
    type Ok = ();
    type Error = StreamHashError;
    fn serialize_element<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), Self::Error> {
        value.serialize(StreamHashSerializer(self.0))
    }
    fn end(self) -> Result<(), Self::Error> {
        Ok(())
    }
}
impl<'a> serde::ser::SerializeTuple for StreamHashSerializer<'a> {
    type Ok = ();
    type Error = StreamHashError;
    fn serialize_element<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), Self::Error> {
        value.serialize(StreamHashSerializer(self.0))
    }
    fn end(self) -> Result<(), Self::Error> {
        Ok(())
    }
}
impl<'a> serde::ser::SerializeTupleStruct for StreamHashSerializer<'a> {
    type Ok = ();
    type Error = StreamHashError;
    fn serialize_field<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), Self::Error> {
        value.serialize(StreamHashSerializer(self.0))
    }
    fn end(self) -> Result<(), Self::Error> {
        Ok(())
    }
}
impl<'a> serde::ser::SerializeTupleVariant for StreamHashSerializer<'a> {
    type Ok = ();
    type Error = StreamHashError;
    fn serialize_field<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), Self::Error> {
        value.serialize(StreamHashSerializer(self.0))
    }
    fn end(self) -> Result<(), Self::Error> {
        Ok(())
    }
}
impl<'a> serde::ser::SerializeMap for StreamHashSerializer<'a> {
    type Ok = ();
    type Error = StreamHashError;
    fn serialize_key<T: ?Sized + Serialize>(&mut self, key: &T) -> Result<(), Self::Error> {
        key.serialize(StreamHashSerializer(self.0))
    }
    fn serialize_value<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), Self::Error> {
        value.serialize(StreamHashSerializer(self.0))
    }
    fn end(self) -> Result<(), Self::Error> {
        Ok(())
    }
}
impl<'a> serde::ser::SerializeStruct for StreamHashSerializer<'a> {
    type Ok = ();
    type Error = StreamHashError;
    fn serialize_field<T: ?Sized + Serialize>(&mut self, key: &'static str, value: &T) -> Result<(), Self::Error> {
        key.hash(self.0);
        value.serialize(StreamHashSerializer(self.0))
    }
    fn end(self) -> Result<(), Self::Error> {
        Ok(())
    }
}
impl<'a> serde::ser::SerializeStructVariant for StreamHashSerializer<'a> {
    type Ok = ();
    type Error = StreamHashError;
    fn serialize_field<T: ?Sized + Serialize>(&mut self, key: &'static str, value: &T) -> Result<(), Self::Error> {
        key.hash(self.0);
        value.serialize(StreamHashSerializer(self.0))
    }
    fn end(self) -> Result<(), Self::Error> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::Session;
    use designos_doc::live_patch::LivePatch;
    use designos_doc::Node;
    use std::time::Instant;

    #[test]
    fn failed_last_op_leaves_document_identical() {
        let mut engine = Engine::default();
        let page = engine.resolve_default_page().unwrap();
        let seed = engine
            .call(
                "designos_create_node",
                json!({ "parent_id": page.0, "kind": "rectangle", "name": "Existing", "x": 10.0, "y": 20.0, "width": 80.0, "height": 40.0 }),
            )
            .unwrap();
        let existing_id = seed["node"]["id"].as_u64().unwrap();

        let before_json = engine.doc.to_json().unwrap();
        let before_depth = engine.history.undo_depth();

        let err_str = engine
            .call(
                "designos_apply_batch",
                json!({
                    "ops": [
                        { "op": "insert", "as": "box1", "parent": page.0, "kind": "rectangle", "name": "Box 1", "x": 0.0, "y": 0.0, "width": 100.0, "height": 50.0 },
                        { "op": "update", "node": existing_id, "name": "Renamed" },
                        { "op": "update", "node": "@box1", "opacity": 2.0 }
                    ]
                }),
            )
            .expect_err("op 2 has opacity 2.0 and must fail the whole batch");

        let err_val = err_str.to_json_value();
        assert_eq!(err_val["failed_op"], 2, "failed_op must be 2: {err_val}");
        assert_eq!(err_val["op"], "update");
        assert_eq!(err_val["safe_to_retry"], true);
        assert_eq!(engine.doc.to_json().unwrap(), before_json, "document must be byte-for-byte identical after failed batch");
        assert_eq!(engine.history.undo_depth(), before_depth, "history depth must be unchanged");
    }

    #[test]
    fn batch_is_one_undo_step() {
        let mut engine = Engine::default();
        let page = engine.resolve_default_page().unwrap();
        let pre_nodes = engine.doc.nodes.clone();
        let pre_vars = engine.doc.variables.clone();
        let pre_styles = engine.doc.styles.clone();
        let pre_depth = engine.history.undo_depth();

        let reply = engine
            .call(
                "designos_apply_batch",
                json!({
                    "ops": [
                        {
                            "op": "apply_spec",
                            "as": "card",
                            "parent": page.0,
                            "spec": {
                                "kind": "frame",
                                "name": "Card",
                                "width": 240.0,
                                "height": 120.0,
                                "children": [
                                    { "kind": "text", "name": "Heading", "text": "Hello" }
                                ]
                            }
                        },
                        {
                            "op": "set_variables",
                            "upsert_collections": [{ "id": "col:brand", "name": "Brand" }],
                            "upsert_variables": [{ "id": "var:primary", "name": "Primary", "collection": "col:brand", "kind": "Color", "values": { "light": "#0D99FF" } }]
                        },
                        {
                            "op": "set_style",
                            "upsert": [{ "key": "style:accent", "name": "Accent", "fill": "#0D99FF" }],
                            "apply": [{ "node": "@card", "style": "style:accent" }]
                        }
                    ]
                }),
            )
            .expect("batch should succeed");

        assert_eq!(engine.history.undo_depth(), pre_depth + 1, "entire batch must be exactly one undo step");
        assert_eq!(reply["changed"]["inserted"], 2);
        assert_eq!(reply["changed"]["variables"], 2);
        assert_eq!(reply["changed"]["styles"], 2);

        let post_nodes = engine.doc.nodes.clone();
        let post_vars = engine.doc.variables.clone();
        let post_styles = engine.doc.styles.clone();

        // One undo reverts all 3 ops.
        let undone = engine.call("designos_undo", json!({})).unwrap();
        assert_eq!(undone["changed"], true);
        assert_eq!(engine.doc.nodes, pre_nodes);
        assert_eq!(engine.doc.variables, pre_vars);
        assert_eq!(engine.doc.styles, pre_styles);

        // One redo restores all 3 ops.
        let redone = engine.call("designos_redo", json!({})).unwrap();
        assert_eq!(redone["changed"], true);
        assert_eq!(engine.doc.nodes, post_nodes);
        assert_eq!(engine.doc.variables, post_vars);
        assert_eq!(engine.doc.styles, post_styles);
    }

    #[test]
    fn apply_spec_op_with_easeui_token_commits_collection_and_one_undo_removes_both() {
        let mut engine = Engine { live_ops: Some(Vec::new()), ..Default::default() };
        let page = engine.resolve_default_page().unwrap();
        let pre_nodes = engine.doc.nodes.clone();
        let pre_vars = engine.doc.variables.clone();
        let pre_collections = engine.doc.variable_collections.clone();
        let pre_depth = engine.history.undo_depth();

        let reply = engine
            .call(
                "designos_apply_batch",
                json!({
                    "ops": [{
                        "op": "apply_spec",
                        "as": "panel",
                        "parent": page.0,
                        "spec": { "kind": "rectangle", "name": "Panel", "fill": "$easeui.bg.panel" }
                    }]
                }),
            )
            .expect("batch should succeed");

        let panel = NodeId(reply["created"][0].as_u64().unwrap());
        assert_eq!(
            engine.doc.get(panel).unwrap().variables.bindings.get("fills/0").map(String::as_str),
            Some("easeui-deprecated/bg/panel"),
            "the node binds the deprecated EaseUI token variable"
        );
        assert!(engine.doc.variable_collections.contains_key("easeui-deprecated"), "the collection the node binds to reaches the document");
        assert!(engine.doc.variables.contains_key("easeui-deprecated/bg/panel"));
        assert_eq!(engine.history.undo_depth(), pre_depth + 1, "nodes and variables are one undo step");

        let live = engine.take_live_ops();
        let variables_at = live.iter().position(|op| matches!(op, LiveOp::Variables { .. })).expect("the editor is told about the new variables");
        let insert_at = live.iter().position(|op| matches!(op, LiveOp::Insert { .. })).expect("the editor is told about the new node");
        assert!(variables_at < insert_at, "the editor holds the variable before the bound node arrives: {live:?}");

        engine.call("designos_undo", json!({})).unwrap();
        assert_eq!(engine.doc.nodes, pre_nodes);
        assert_eq!(engine.doc.variables, pre_vars);
        assert_eq!(engine.doc.variable_collections, pre_collections);
    }

    #[test]
    fn dry_run_matches_commit() {
        let mut engine = Engine::default();
        let page = engine.resolve_default_page().unwrap();
        let placeholder = engine
            .call(
                "designos_create_node",
                json!({ "parent_id": page.0, "kind": "rectangle", "name": "Placeholder", "x": 0.0, "y": 0.0, "width": 1440.0, "height": 900.0 }),
            )
            .unwrap()["node"]["id"]
            .as_u64()
            .unwrap();

        let before_json = engine.doc.to_json().unwrap();
        let before_depth = engine.history.undo_depth();

        let payload = |dry_run: bool| {
            json!({
                "ops": [
                    {
                        "op": "apply_spec",
                        "as": "hero",
                        "parent": page.0,
                        "spec": {
                            "kind": "frame",
                            "name": "Hero",
                            "layout": { "direction": "column", "gap": 16.0 },
                            "children": [
                                { "kind": "text", "name": "Title", "text": "Plan your week" }
                            ]
                        }
                    },
                    { "op": "update", "node": "@hero/Title", "opacity": 0.9 },
                    { "op": "delete", "node": placeholder }
                ],
                "dry_run": dry_run
            })
        };

        let dry = engine.call("designos_apply_batch", payload(true)).unwrap();
        assert_eq!(dry["dry_run"], true);
        assert_eq!(engine.doc.to_json().unwrap(), before_json, "document must be unchanged after dry_run");
        assert_eq!(engine.history.undo_depth(), before_depth, "history depth must be unchanged after dry_run");

        let committed = engine.call("designos_apply_batch", payload(false)).unwrap();
        assert_eq!(committed["dry_run"], false);
        assert_eq!(dry["created"], committed["created"], "dry_run and commit must produce identical created IDs");
        assert_eq!(dry["created_count"], committed["created_count"]);
        assert_eq!(dry["name_to_id"], committed["name_to_id"], "dry_run and commit must produce identical name_to_id");
        assert_eq!(dry["touched_roots"], committed["touched_roots"], "dry_run and commit must produce identical touched_roots");
        assert_eq!(dry["touched_bounds"], committed["touched_bounds"]);
    }

    #[test]
    fn name_map_resolves_every_named_node() {
        let mut engine = Engine::default();
        let page = engine.resolve_default_page().unwrap();

        let res = engine
            .call(
                "designos_apply_batch",
                json!({
                    "ops": [
                        {
                            "op": "apply_spec",
                            "as": "screen",
                            "parent": page.0,
                            "spec": {
                                "kind": "frame",
                                "name": "Screen",
                                "layout": { "direction": "column", "gap": 12.0 },
                                "children": [
                                    {
                                        "kind": "frame",
                                        "name": "Header",
                                        "children": [
                                            { "kind": "text", "name": "Logo", "text": "DESIGN:OS" },
                                            { "kind": "text", "name": "Subtitle", "text": "Studio" }
                                        ]
                                    },
                                    {
                                        "kind": "frame",
                                        "name": "Body",
                                        "children": [
                                            {
                                                "kind": "frame",
                                                "name": "CardA",
                                                "children": [
                                                    { "kind": "text", "name": "CardTitle", "text": "Analytics" },
                                                    { "kind": "text", "name": "CardMetric", "text": "99.9%" }
                                                ]
                                            },
                                            {
                                                "kind": "frame",
                                                "name": "CardB",
                                                "children": [
                                                    { "kind": "text", "name": "CardTitle", "text": "Latency" },
                                                    { "kind": "text", "name": "CardMetric", "text": "4.2ms" }
                                                ]
                                            }
                                        ]
                                    },
                                    {
                                        "kind": "frame",
                                        "name": "Footer",
                                        "children": [
                                            { "kind": "text", "name": "Copyright", "text": "2026" }
                                        ]
                                    }
                                ]
                            }
                        }
                    ]
                }),
            )
            .unwrap();

        let name_map = res["name_to_id"].as_object().expect("name_to_id object");
        // 1 root ("screen") + 12 named descendants = 13 entries
        assert_eq!(name_map.len(), 13, "expected root + 12 named descendants, got: {name_map:?}");
        for (path, id_val) in name_map {
            let id = id_val.as_u64().expect("numeric node id");
            let fetched = engine.call("designos_get_node", json!({ "node_id": id })).expect("node must exist");
            let actual_name = fetched["node"]["name"].as_str().unwrap();
            if path == "screen" {
                assert_eq!(actual_name, "Screen");
            } else {
                let last_seg = path.rsplit('/').next().unwrap();
                let base_name = parse_indexed_segment(last_seg).map(|(b, _)| b).unwrap_or(last_seg);
                assert_eq!(actual_name, base_name, "path {path} resolved to node {id} with name {actual_name}");
            }
        }
    }

    #[test]
    fn refs_to_earlier_ops() {
        let mut engine = Engine::default();
        let page = engine.resolve_default_page().unwrap();

        let res = engine
            .call(
                "designos_apply_batch",
                json!({
                    "ops": [
                        {
                            "op": "apply_spec",
                            "as": "hero",
                            "parent": page.0,
                            "spec": {
                                "kind": "frame",
                                "name": "Hero",
                                "children": [
                                    { "kind": "text", "name": "Title", "text": "Initial" }
                                ]
                            }
                        },
                        {
                            "op": "update",
                            "node": "@hero/Title",
                            "text": "Updated via @hero/Title",
                            "opacity": 0.85
                        }
                    ]
                }),
            )
            .unwrap();

        let title_id = res["name_to_id"]["hero/Title"].as_u64().unwrap();
        let node = engine.doc.get(NodeId(title_id)).unwrap();
        assert!((node.opacity - 0.85).abs() < 1e-6);
        match &node.kind {
            NodeKind::Text(tp) => assert_eq!(tp.content, "Updated via @hero/Title"),
            other => panic!("expected text node, got {other:?}"),
        }
    }

    #[test]
    fn unknown_ref_rejected_before_run() {
        let mut engine = Engine::default();
        let page = engine.resolve_default_page().unwrap();
        let before_json = engine.doc.to_json().unwrap();
        let before_depth = engine.history.undo_depth();

        let err_str = engine
            .call(
                "designos_apply_batch",
                json!({
                    "ops": [
                        { "op": "insert", "as": "first", "parent": page.0, "kind": "frame", "name": "First" },
                        { "op": "update", "node": "@nope", "opacity": 0.5 }
                    ]
                }),
            )
            .expect_err("unknown ref @nope must be rejected before anything runs");

        let err_val = err_str.to_json_value();
        assert_eq!(err_val["code"], "UNKNOWN_REF", "{err_val}");
        assert_eq!(err_val["failed_op"], 1);
        assert_eq!(err_val["safe_to_retry"], true);
        assert_eq!(engine.doc.to_json().unwrap(), before_json);
        assert_eq!(engine.history.undo_depth(), before_depth);
    }

    #[test]
    fn one_patch_line_with_label() {
        let nonce = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let ws_dir = std::env::temp_dir().join(format!("designos-batch-ws-{}-{nonce}", std::process::id()));
        let sess_dir = std::env::temp_dir().join(format!("designos-batch-sess-{}-{nonce}", std::process::id()));
        std::fs::create_dir_all(&ws_dir).unwrap();
        std::fs::create_dir_all(&sess_dir).unwrap();

        let mut doc = Document::new("LiveBatch.designos");
        let page = doc.add_page("Page 1");
        std::fs::write(sess_dir.join("canvas.designos"), doc.to_json().unwrap()).unwrap();

        let mut session = Session::with_session_dir(&ws_dir, &sess_dir).unwrap();
        let reply = session
            .call(
                "designos_apply_batch",
                json!({
                    "ops": [
                        { "op": "insert", "as": "hero", "parent": page.0, "kind": "frame", "name": "Hero", "width": 300.0, "height": 200.0 },
                        { "op": "insert", "parent": "@hero", "kind": "text", "name": "Heading", "text": "Ship fast", "width": 120.0, "height": 24.0 },
                        { "op": "update", "node": "@hero/Heading", "opacity": 0.95 }
                    ]
                }),
            )
            .unwrap();

        let label = reply["undo_label"].as_str().unwrap();
        assert!(label.starts_with("Agent: "), "unexpected undo_label: {label}");

        let patch_raw = std::fs::read_to_string(sess_dir.join("doc-patch.jsonl")).expect("doc-patch.jsonl written");
        let lines: Vec<&str> = patch_raw.lines().collect();
        assert_eq!(lines.len(), 1, "batch must write exactly one doc-patch.jsonl line");
        let patch: LivePatch = serde_json::from_str(lines[0]).expect("valid LivePatch JSON");
        assert_eq!(patch.label.as_deref(), Some(label));
        assert_eq!(patch.ops.len(), 3);

        let _ = std::fs::remove_dir_all(&ws_dir);
        let _ = std::fs::remove_dir_all(&sess_dir);
    }

    #[test]
    fn session_dir_batch_with_set_variables_and_set_style_replays_live_patch() {
        use designos_doc::live_patch::{replay_live_patch, LivePatchState};

        let nonce = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let ws_dir = std::env::temp_dir().join(format!("designos-batch-vars-ws-{}-{nonce}", std::process::id()));
        let sess_dir = std::env::temp_dir().join(format!("designos-batch-vars-sess-{}-{nonce}", std::process::id()));
        std::fs::create_dir_all(&ws_dir).unwrap();
        std::fs::create_dir_all(&sess_dir).unwrap();

        let mut editor_doc = Document::new("LiveBatchVarsStyles.designos");
        let page = editor_doc.add_page("Page 1");
        let placeholder = editor_doc.alloc_id();
        editor_doc.insert(page, Node::rectangle(placeholder, Rect::new(0.0, 0.0, 40.0, 40.0))).unwrap();
        std::fs::write(sess_dir.join("canvas.designos"), editor_doc.to_json().unwrap()).unwrap();

        let mut session = Session::with_session_dir(&ws_dir, &sess_dir).unwrap();
        let reply = session
            .call(
                "designos_apply_batch",
                json!({
                    "ops": [
                        {
                            "op": "insert",
                            "as": "section",
                            "parent": page.0,
                            "kind": "frame",
                            "name": "Hero Section",
                            "width": 320.0,
                            "height": 180.0
                        },
                        {
                            "op": "set_variables",
                            "upsert_collections": [
                                { "id": "brand", "name": "Brand", "modes": [{ "id": "light", "name": "Light" }], "default_mode": "light" }
                            ],
                            "upsert_variables": [
                                { "id": "brand/accent", "name": "Accent", "collection": "brand", "kind": "color", "values": { "light": "#E36323" } }
                            ]
                        },
                        {
                            "op": "set_style",
                            "upsert": [
                                { "key": "hero_surface", "name": "Hero Surface", "property": "fill", "fill": "#18181B" }
                            ],
                            "apply": [
                                { "node": "@section", "style": "Hero Surface" }
                            ]
                        },
                        {
                            "op": "delete",
                            "node": placeholder.0
                        }
                    ]
                }),
            )
            .expect("live session batch with set_variables and set_style must succeed");

        let label = reply["undo_label"].as_str().unwrap();
        assert!(label.starts_with("Agent: "));

        let patch_raw = std::fs::read_to_string(sess_dir.join("doc-patch.jsonl")).expect("doc-patch.jsonl written");
        let lines: Vec<&str> = patch_raw.lines().collect();
        assert_eq!(lines.len(), 1, "batch must write one labeled patch line");
        let patch: LivePatch = serde_json::from_str(lines[0]).expect("valid LivePatch JSON");
        assert_eq!(patch.label.as_deref(), Some(label));

        let mut state = LivePatchState::default();
        let mut editor_hist = designos_doc::History::default();
        let mut replayed_cmds = Vec::new();
        replay_live_patch(&mut editor_doc, &patch, &mut state, &mut |doc, cmd| {
            replayed_cmds.push(cmd.clone());
            editor_hist.exec(doc, cmd)
        })
        .expect("editor must replay LivePatch containing variables and styles");

        assert!(editor_doc.variables.contains_key("brand/accent"));
        assert!(editor_doc.variable_collections.contains_key("brand"));
        assert!(editor_doc.styles.contains_key("hero_surface"));
        assert!(editor_doc.get(placeholder).is_none());
        let sec_id = NodeId(reply["name_to_id"]["section"].as_u64().unwrap());
        assert_eq!(editor_doc.get(sec_id).unwrap().style_refs.get("fill").map(String::as_str), Some("hero_surface"));

        let _ = std::fs::remove_dir_all(&ws_dir);
        let _ = std::fs::remove_dir_all(&sess_dir);
    }

    #[test]
    fn duplicate_sibling_names_use_1_based_suffixes_and_resolve() {
        let mut engine = Engine::default();
        let page = engine.resolve_default_page().unwrap();

        let res = engine
            .call(
                "designos_apply_batch",
                json!({
                    "ops": [
                        {
                            "op": "apply_spec",
                            "as": "hero",
                            "parent": page.0,
                            "spec": {
                                "kind": "frame",
                                "name": "Hero",
                                "children": [
                                    { "kind": "text", "name": "Title", "text": "One" },
                                    { "kind": "text", "name": "Title", "text": "Two" }
                                ]
                            }
                        },
                        {
                            "op": "update",
                            "node": "@hero/Title[1]",
                            "text": "First Updated"
                        },
                        {
                            "op": "update",
                            "node": "@hero/Title[2]",
                            "text": "Second Updated"
                        }
                    ]
                }),
            )
            .unwrap();

        let name_map = res["name_to_id"].as_object().expect("name_to_id object");
        assert!(name_map.contains_key("hero/Title[1]"), "first repeated sibling must be suffixed [1], got {name_map:?}");
        assert!(name_map.contains_key("hero/Title[2]"), "second repeated sibling must be suffixed [2], got {name_map:?}");
        assert!(!name_map.contains_key("hero/Title"), "unsuffixed duplicate sibling key must not be emitted, got {name_map:?}");
    }

    #[test]
    fn lease_guard_enforced_on_set_style_and_set_variables() {
        let mut engine = Engine::default();
        let page = engine.resolve_default_page().unwrap();
        let leased_frame = engine.doc.alloc_id();
        let mut lf = Node::frame(leased_frame, Rect::new(0.0, 0.0, 200.0, 200.0));
        lf.name = "Leased".into();
        engine.doc.insert(page, lf).unwrap();

        let other_frame = engine.doc.alloc_id();
        let mut of = Node::frame(other_frame, Rect::new(300.0, 0.0, 200.0, 200.0));
        of.name = "Other".into();
        engine.doc.insert(page, of).unwrap();

        engine.doc.styles.insert("s_brand".into(), Style::Fill(vec![Paint::solid(Color::rgb(0x11, 0x22, 0x33))]));
        engine.doc.style_meta.insert(
            "s_brand".into(),
            StyleMeta {
                name: "Brand Fill".into(),
                key: None,
                description: String::new(),
                property: StyleProperty::Fill,
                bindings: Default::default(),
                ..Default::default()
            },
        );
        engine.owned_frames = Some(vec![leased_frame]);

        // 1. set_style apply to unleased node must be refused
        let err1 = engine
            .call(
                "designos_apply_batch",
                json!({
                    "ops": [
                        { "op": "set_style", "apply": [{ "node": other_frame.0, "style": "Brand Fill" }] }
                    ]
                }),
            )
            .expect_err("set_style apply on unleased node must fail");
        let v1 = err1.to_json_value();
        assert_eq!(v1["cause"]["code"], "OUTSIDE_OWNED_FRAMES", "{v1}");
        assert!(engine.doc.get(other_frame).unwrap().style_refs.is_empty());

        // 2. set_variables while frame lease is active must be refused
        let err2 = engine
            .call(
                "designos_apply_batch",
                json!({
                    "ops": [
                        {
                            "op": "set_variables",
                            "upsert_variables": [{ "id": "v1", "name": "V1", "values": { "light": "#ffffff" } }]
                        }
                    ]
                }),
            )
            .expect_err("set_variables must fail while frame lease is active");
        let v2 = err2.to_json_value();
        assert_eq!(v2["cause"]["code"], "OUTSIDE_OWNED_FRAMES", "{v2}");

        // 3. set_style apply to leased node (without document-wide style upserts/deletes) succeeds
        engine
            .call(
                "designos_apply_batch",
                json!({
                    "ops": [
                        { "op": "set_style", "apply": [{ "node": leased_frame.0, "style": "Brand Fill" }] }
                    ]
                }),
            )
            .expect("set_style apply on leased node must succeed");
        assert_eq!(engine.doc.get(leased_frame).unwrap().style_refs.get("fill").map(String::as_str), Some("s_brand"));
    }

    /// Deterministic synthetic document builder (README.md §Shared conventions, scaled from
    /// `crates/designos-gpu/tests/large_file_perf.rs`) producing an exact `target_nodes` count.
    fn build_synthetic_document(target_nodes: usize) -> (Document, NodeId, NodeId) {
        assert!(target_nodes >= 10, "target_nodes must be at least 10");
        let mut doc = Document::new(format!("Synthetic-{target_nodes}"));
        let page = doc.add_page("Page 1");
        // Pre-populate one target board with 10 cards so the 200-op batch has rich hierarchy to edit.
        let target_board = doc.alloc_id();
        let mut bnode = Node::frame(target_board, Rect::new(0.0, 0.0, 1600.0, 1200.0));
        bnode.name = "Target Board".into();
        doc.insert(page, bnode).unwrap();

        // Each board has cards, each card has leaf children, landing on `target_nodes` total nodes.
        let cards_per_board = 25usize;
        let leaves_per_card = 48usize;
        let nodes_per_full_board = 1 + cards_per_board * (1 + leaves_per_card); // 1226 nodes

        let mut remaining = target_nodes - doc.nodes.len();
        let mut board_idx = 0usize;
        while remaining > 0 {
            if remaining >= nodes_per_full_board {
                let bid = doc.alloc_id();
                let mut board = Node::frame(bid, Rect::new((board_idx % 10) as f32 * 1800.0, (board_idx / 10) as f32 * 1400.0, 1600.0, 1200.0));
                board.name = format!("Board {board_idx}");
                board.parent = Some(page);
                let mut board_children = Vec::with_capacity(cards_per_board);

                for c_idx in 0..cards_per_board {
                    let cid = doc.alloc_id();
                    let mut card = Node::frame(cid, Rect::new((c_idx % 5) as f32 * 310.0, (c_idx / 5) as f32 * 230.0, 290.0, 210.0));
                    card.name = format!("Card {board_idx}-{c_idx}");
                    card.parent = Some(bid);
                    let mut card_children = Vec::with_capacity(leaves_per_card);
                    for k in 0..leaves_per_card {
                        let lid = doc.alloc_id();
                        let mut leaf = Node::rectangle(lid, Rect::new((k % 6) as f32 * 40.0, (k / 6) as f32 * 28.0, 32.0, 18.0));
                        leaf.name = format!("Item {k}");
                        leaf.parent = Some(cid);
                        card_children.push(lid);
                        doc.nodes.insert(lid, leaf);
                    }
                    card.children = card_children;
                    board_children.push(cid);
                    doc.nodes.insert(cid, card);
                }
                board.children = board_children;
                doc.nodes.insert(bid, board);
                doc.get_mut(page).unwrap().children.push(bid);
                remaining -= nodes_per_full_board;
                board_idx += 1;
            } else {
                let lid = doc.alloc_id();
                let mut leaf = Node::rectangle(lid, Rect::new(10.0, 10.0, 32.0, 18.0));
                leaf.name = format!("Tail {remaining}");
                leaf.parent = Some(target_board);
                doc.nodes.insert(lid, leaf);
                doc.get_mut(target_board).unwrap().children.push(lid);
                remaining -= 1;
            }
        }
        assert_eq!(doc.nodes.len(), target_nodes);
        (doc, page, target_board)
    }

    fn build_200_ops_payload(target_board: NodeId) -> Value {
        let mut ops = Vec::with_capacity(200);
        // Op 0: create a container frame under target_board with alias `batch_root`.
        ops.push(json!({
            "op": "insert",
            "as": "batch_root",
            "parent": target_board.0,
            "kind": "frame",
            "name": "Batch Container",
            "x": 20.0,
            "y": 20.0,
            "width": 800.0,
            "height": 600.0
        }));
        // Ops 1..=100: insert 100 named child layers under `@batch_root`.
        for i in 1..=100 {
            ops.push(json!({
                "op": "insert",
                "as": format!("item_{i}"),
                "parent": "@batch_root",
                "kind": "rectangle",
                "name": format!("Item {i}"),
                "x": (i % 10) as f32 * 40.0,
                "y": (i / 10) as f32 * 30.0,
                "width": 32.0,
                "height": 20.0
            }));
        }
        // Ops 101..=199: update 99 of those layers via `@item_i`.
        for i in 1..=99 {
            ops.push(json!({
                "op": "update",
                "node": format!("@item_{i}"),
                "opacity": 0.8
            }));
        }
        assert_eq!(ops.len(), 200);
        json!({ "ops": ops })
    }

    #[test]
    fn trial_clone_budget_on_large_synthetic() {
        // Release budgets from §11: 100k < 400ms, 112,474 < 450ms, 1,200,000 < 5000ms.
        // In unoptimized debug test builds (`cfg!(debug_assertions)`), BTreeMap cloning of 1.2M
        // nodes has ~5x uninlined allocator/drop overhead, so we enforce the strict §11 budgets
        // in release builds and a generous ceiling in debug builds while printing exact timings.
        let scale = if cfg!(debug_assertions) { 10.0 } else { 1.0 };
        // The 1.2M case holds the document, the dry-run trial clone and the commit clone at once: about 6 GiB peak
        // RSS in a debug build, which kills a 16 GiB CI runner next to other tests (#1205). Debug builds run a
        // 250,000-node largest case instead (well under 2 GiB); release builds keep the full 1.2M budget.
        let largest = if cfg!(debug_assertions) { 250_000usize } else { 1_200_000usize };
        let cases = [(100_000usize, 400.0 * scale), (112_474usize, 450.0 * scale), (largest, 5_000.0 * scale)];

        for (target_nodes, budget_ms) in cases {
            let (doc, page, target_board) = build_synthetic_document(target_nodes);
            let approx_doc_bytes = doc.nodes.len() * std::mem::size_of::<Node>();
            let mut engine = Engine { doc, active_page_id: Some(page), ..Engine::default() };

            let pre_hash = batch_state_hash(&engine.doc, &[page, target_board]);
            let pre_len = engine.doc.nodes.len();
            let pre_epoch = engine.doc.mutation_epoch();

            let mut dry_payload = build_200_ops_payload(target_board);
            dry_payload["dry_run"] = json!(true);
            let t_dry = Instant::now();
            let dry_res = engine.call("designos_apply_batch", dry_payload).expect("200-op dry_run must succeed");
            let dry_ms = t_dry.elapsed().as_secs_f64() * 1000.0;

            assert_eq!(engine.doc.nodes.len(), pre_len, "node count unchanged after dry_run ({target_nodes})");
            assert_eq!(engine.doc.mutation_epoch(), pre_epoch, "mutation epoch unchanged after dry_run ({target_nodes})");
            assert_eq!(batch_state_hash(&engine.doc, &[page, target_board]), pre_hash, "document state unchanged after dry_run ({target_nodes})");
            assert_eq!(dry_res["created_count"], 101);

            let commit_payload = build_200_ops_payload(target_board);
            let t_commit = Instant::now();
            let commit_res = engine.call("designos_apply_batch", commit_payload).expect("200-op commit must succeed");
            let commit_ms = t_commit.elapsed().as_secs_f64() * 1000.0;

            assert_eq!(commit_res["created"], dry_res["created"]);
            assert_eq!(engine.doc.nodes.len(), target_nodes + 101);
            eprintln!(
                "[E15-4 bench] nodes={target_nodes} dry_run={dry_ms:.1}ms commit={commit_ms:.1}ms budget={budget_ms:.1}ms single_clone_bytes~={approx_doc_bytes}"
            );
            assert!(commit_ms <= budget_ms, "E15-4 commit for {target_nodes} nodes took {commit_ms:.1}ms, exceeding budget {budget_ms:.1}ms");
        }
    }

    #[test]
    fn recolour_bbox_equals_node_bounds() {
        let mut engine = Engine::default();
        let page = engine.resolve_default_page().unwrap();
        let setup = engine
            .call(
                "designos_apply_batch",
                json!({
                    "ops": [
                        { "op": "insert", "as": "board", "parent": page.0, "kind": "frame", "name": "Board", "x": 0.0, "y": 0.0, "width": 200.0, "height": 200.0, "fill": "#FFFFFF" },
                        { "op": "insert", "as": "rect", "parent": "@board", "kind": "rectangle", "name": "Target", "x": 20.0, "y": 30.0, "width": 60.0, "height": 40.0, "fill": "#FF0000" }
                    ]
                }),
            )
            .unwrap();
        let rect_id = setup["name_to_id"]["rect"].as_u64().unwrap();

        let reply = engine
            .call(
                "designos_apply_batch",
                json!({
                    "ops": [
                        { "op": "update", "node": rect_id, "fill": "#00FF00" }
                    ],
                    "diff_image": true
                }),
            )
            .unwrap();

        let diff = &reply["diff"];
        assert_eq!(diff["changed_pixels"], 60 * 40);
        assert_eq!(diff["changed_bbox"], json!({ "x": 20.0, "y": 30.0, "width": 60.0, "height": 40.0 }));
        let content = reply["content"].as_array().expect("3 image content blocks");
        assert_eq!(content.len(), 3);
        assert_eq!(content[0]["type"], "image");
        assert_eq!(content[0]["mimeType"], "image/png");
    }

    #[test]
    fn colour_change_highlights_only_the_rect() {
        recolour_bbox_equals_node_bounds();
    }

    #[test]
    fn noop_batch_reports_zero() {
        let mut engine = Engine::default();
        let page = engine.resolve_default_page().unwrap();
        let setup = engine
            .call(
                "designos_apply_batch",
                json!({
                    "ops": [
                        { "op": "insert", "as": "rect", "parent": page.0, "kind": "rectangle", "name": "Target", "x": 10.0, "y": 10.0, "width": 50.0, "height": 50.0, "fill": "#FF0000" }
                    ]
                }),
            )
            .unwrap();
        let rect_id = setup["name_to_id"]["rect"].as_u64().unwrap();

        let reply = engine
            .call(
                "designos_apply_batch",
                json!({
                    "ops": [
                        { "op": "update", "node": rect_id, "fill": "#FF0000" }
                    ],
                    "diff_image": true
                }),
            )
            .unwrap();

        let diff = &reply["diff"];
        assert_eq!(diff["changed_pixels"], 0);
        assert_eq!(diff["changed_ratio"], 0.0);
        assert!(diff["changed_bbox"].is_null());
        assert!(reply.get("content").is_none(), "no image blocks when changed_pixels == 0");
    }

    #[test]
    fn noop_batch_returns_zero_changed_pixels() {
        noop_batch_reports_zero();
    }

    #[test]
    fn move_includes_both_positions() {
        let mut engine = Engine::default();
        let page = engine.resolve_default_page().unwrap();
        let setup = engine
            .call(
                "designos_apply_batch",
                json!({
                    "ops": [
                        { "op": "insert", "as": "board", "parent": page.0, "kind": "frame", "name": "Board", "x": 0.0, "y": 0.0, "width": 300.0, "height": 200.0, "fill": "#FFFFFF" },
                        { "op": "insert", "as": "rect", "parent": "@board", "kind": "rectangle", "name": "Mover", "x": 10.0, "y": 10.0, "width": 40.0, "height": 40.0, "fill": "#FF0000" }
                    ]
                }),
            )
            .unwrap();
        let rect_id = setup["name_to_id"]["rect"].as_u64().unwrap();

        let reply = engine
            .call(
                "designos_apply_batch",
                json!({
                    "ops": [
                        { "op": "update", "node": rect_id, "x": 100.0, "y": 10.0 }
                    ],
                    "diff_image": true
                }),
            )
            .unwrap();

        let diff = &reply["diff"];
        assert_eq!(diff["changed_pixels"], 2 * 40 * 40);
        assert_eq!(diff["changed_bbox"], json!({ "x": 10.0, "y": 10.0, "width": 130.0, "height": 40.0 }));
        assert_eq!(reply["content"].as_array().unwrap().len(), 3);
    }

    #[test]
    fn move_node_unions_old_and_new_bounds() {
        move_includes_both_positions();
    }

    #[test]
    fn delete_node_captures_before_bounds() {
        let mut engine = Engine::default();
        let page = engine.resolve_default_page().unwrap();
        let setup = engine
            .call(
                "designos_apply_batch",
                json!({
                    "ops": [
                        { "op": "insert", "as": "board", "parent": page.0, "kind": "frame", "name": "Board", "x": 0.0, "y": 0.0, "width": 200.0, "height": 200.0, "fill": "#FFFFFF" },
                        { "op": "insert", "as": "rect", "parent": "@board", "kind": "rectangle", "name": "Target", "x": 40.0, "y": 60.0, "width": 50.0, "height": 30.0, "fill": "#FF0000" }
                    ]
                }),
            )
            .unwrap();
        let rect_id = setup["name_to_id"]["rect"].as_u64().unwrap();

        let reply = engine
            .call(
                "designos_apply_batch",
                json!({
                    "ops": [
                        { "op": "delete", "node": rect_id }
                    ],
                    "diff_image": true
                }),
            )
            .unwrap();

        let diff = &reply["diff"];
        assert_eq!(diff["changed_pixels"], 50 * 30);
        assert_eq!(diff["changed_bbox"], json!({ "x": 40.0, "y": 60.0, "width": 50.0, "height": 30.0 }));
        assert_eq!(reply["content"].as_array().unwrap().len(), 3);
    }

    #[test]
    fn dry_run_diff_leaves_document() {
        let mut engine = Engine::default();
        let page = engine.resolve_default_page().unwrap();
        let setup = engine
            .call(
                "designos_apply_batch",
                json!({
                    "ops": [
                        { "op": "insert", "as": "board", "parent": page.0, "kind": "frame", "name": "Board", "x": 0.0, "y": 0.0, "width": 200.0, "height": 200.0, "fill": "#FFFFFF" },
                        { "op": "insert", "as": "rect", "parent": "@board", "kind": "rectangle", "name": "Target", "x": 10.0, "y": 10.0, "width": 40.0, "height": 40.0, "fill": "#FF0000" }
                    ]
                }),
            )
            .unwrap();
        let rect_id = setup["name_to_id"]["rect"].as_u64().unwrap();
        let before_json = engine.doc.to_json().unwrap();
        let before_depth = engine.history.undo_depth();

        let reply = engine
            .call(
                "designos_apply_batch",
                json!({
                    "ops": [
                        { "op": "update", "node": rect_id, "fill": "#0000FF" }
                    ],
                    "dry_run": true,
                    "diff_image": true
                }),
            )
            .unwrap();

        assert_eq!(reply["dry_run"], true);
        assert_eq!(reply["diff"]["changed_pixels"], 40 * 40);
        assert_eq!(reply["content"].as_array().unwrap().len(), 3);
        assert_eq!(engine.doc.to_json().unwrap(), before_json, "dry_run with diff_image must not mutate document");
        assert_eq!(engine.history.undo_depth(), before_depth, "dry_run with diff_image must not mutate undo stack");
    }

    #[test]
    fn multi_page_batch_caps_at_four_pages() {
        let mut engine = Engine::default();
        let p1 = engine.resolve_default_page().unwrap();
        let p2 = engine.doc.add_page("Page 2");
        let p3 = engine.doc.add_page("Page 3");
        let p4 = engine.doc.add_page("Page 4");
        let p5 = engine.doc.add_page("Page 5");
        let pages = [p1, p2, p3, p4, p5];

        let mut ops = Vec::new();
        for (idx, p) in pages.iter().enumerate() {
            ops.push(json!({
                "op": "insert",
                "parent": p.0,
                "kind": "rectangle",
                "name": format!("Box {idx}"),
                "x": 10.0,
                "y": 10.0,
                "width": 30.0,
                "height": 30.0,
                "fill": "#FF0000"
            }));
        }

        let reply = engine
            .call(
                "designos_apply_batch",
                json!({
                    "ops": ops,
                    "diff_image": true
                }),
            )
            .unwrap();

        let diff = &reply["diff"];
        assert_eq!(diff["pages"].as_array().unwrap().len(), 0, "more than 4 touched pages must return no page diffs");
        assert_eq!(diff["omitted_pages"], 1);
        assert_eq!(diff["diff_skipped"], "more than 4 pages");
        assert!(reply.get("content").is_none(), "more than 4 touched pages must return no image content blocks");
    }

    #[test]
    fn large_doc_z_order_reorder_produces_diff_images() {
        let mut engine = Engine::default();
        let page = engine.resolve_default_page().unwrap();
        for i in 0..5_001 {
            let id = engine.doc.alloc_id();
            let mut r = Node::rectangle(id, Rect::new(0.0, 0.0, 4.0, 4.0));
            r.name = format!("Filler {i}");
            engine.doc.insert(page, r).unwrap();
        }
        let setup = engine
            .call(
                "designos_apply_batch",
                json!({
                    "ops": [
                        { "op": "insert", "as": "bottom", "parent": page.0, "kind": "rectangle", "name": "Bottom", "x": 100.0, "y": 100.0, "width": 40.0, "height": 40.0, "fill": "#FF0000" },
                        { "op": "insert", "as": "top", "parent": page.0, "kind": "rectangle", "name": "Top", "x": 100.0, "y": 100.0, "width": 40.0, "height": 40.0, "fill": "#0000FF" }
                    ]
                }),
            )
            .unwrap();
        let bottom_id = setup["name_to_id"]["bottom"].as_u64().unwrap();

        assert!(engine.doc.nodes.len() > 5_000);

        let reply = engine
            .call(
                "designos_apply_batch",
                json!({
                    "ops": [
                        { "op": "move", "node": bottom_id, "parent": page.0 }
                    ],
                    "diff_image": true
                }),
            )
            .unwrap();

        assert_eq!(reply["changed"]["moved"], 1);
        let diff = &reply["diff"];
        assert!(diff["changed_pixels"].as_u64().unwrap() > 0, "z-order reorder on >5000 node doc must detect changed pixels, got: {diff}");
        assert!(diff["changed_bbox"].is_object(), "changed_bbox must be non-null");
        assert_eq!(reply["content"].as_array().unwrap().len(), 3, "must return before/after/diff image blocks");
    }

    #[test]
    fn batch_apply_spec_in_progress_without_run_id_gets_the_writer_run_id() {
        let mut engine = Engine { default_run_id: Some("run_writer_7".to_string()), ..Engine::default() };
        let page = engine.resolve_default_page().unwrap();
        let res = engine
            .call(
                "designos_apply_batch",
                json!({
                    "ops": [{
                        "op": "apply_spec",
                        "parent": page.0,
                        "as": "hero",
                        "spec": {
                            "kind": "frame",
                            "name": "Hero Frame",
                            "width": 390.0,
                            "height": 844.0,
                            "in_progress": { "agent": "claude", "status": "Building..." }
                        }
                    }]
                }),
            )
            .expect("batch apply_spec with in_progress must succeed");
        let hero_id = NodeId(res["name_to_id"]["hero"].as_u64().unwrap());
        let ip = engine.doc.get_in_progress(hero_id).expect("hero frame should be flagged in_progress");
        assert_eq!(ip.run_id, engine.resolve_agent_run_id(None));
        assert!(ip.run_id.is_some(), "writer run id stamped");
    }

    #[test]
    fn batch_flag_and_clear_on_run_end() {
        designos_doc::set_thread_mock_now_ms(Some(2_000_000));
        let mut engine = Engine::default();
        let page = engine.resolve_default_page().unwrap();
        let res = engine
            .call(
                "designos_apply_batch",
                json!({
                    "ops": [
                        {
                            "op": "apply_spec",
                            "parent": page.0,
                            "as": "hero",
                            "spec": {
                                "kind": "frame",
                                "name": "Hero Frame",
                                "width": 390.0,
                                "height": 844.0,
                                "children": [
                                    { "kind": "text", "name": "Headline", "text": "Welcome" }
                                ]
                            }
                        },
                        {
                            "op": "set_in_progress",
                            "nodes": ["@hero"],
                            "value": true,
                            "run_id": "run_test_1",
                            "agent": "Claude Code",
                            "stale_after_s": 300,
                            "status": "Refining hero typography"
                        }
                    ]
                }),
            )
            .expect("batch with set_in_progress must succeed");

        let hero_id = NodeId(res["name_to_id"]["hero"].as_u64().unwrap());
        let ip = engine.doc.get_in_progress(hero_id).expect("hero frame should be flagged in_progress");
        assert_eq!(ip.run_id.as_deref(), Some("run_test_1"));
        assert_eq!(ip.agent.as_deref(), Some("Claude Code"));
        assert_eq!(ip.stale_after_s, Some(300));
        assert_eq!(ip.status.as_deref(), Some("Refining hero typography"));

        // Verify all read surfaces expose in_progress
        let doc_summary = engine.call("designos_get_document", json!({})).unwrap();
        assert_eq!(doc_summary["in_progress_count"], 1);
        assert_eq!(doc_summary["in_progress_nodes"][0]["node_id"], hero_id.0);

        let list = engine.call("designos_list_nodes", json!({ "query": "Hero Frame" })).unwrap();
        assert_eq!(list["nodes"][0]["in_progress"]["agent"], "Claude Code");

        let node_view = engine.call("designos_get_node", json!({ "node_id": hero_id.0 })).unwrap();
        assert_eq!(node_view["in_progress"]["run_id"], "run_test_1");

        let xpath_view = engine.call("designos_query_xpath", json!({ "selector": "//Frame[@name='Hero Frame']", "format": "objects" })).unwrap();
        assert_eq!(xpath_view["rows"][0]["id"], hero_id.0);
        assert_eq!(xpath_view["rows"][0]["in_progress"], true);

        let exported_spec = engine.call("designos_export_spec", json!({ "node_id": hero_id.0 })).unwrap();
        assert_eq!(exported_spec["spec"]["in_progress"]["agent"], "Claude Code");

        // Another run cannot overwrite without force:true
        let conflict = engine
            .call(
                "designos_apply_batch",
                json!({
                    "ops": [{
                        "op": "set_in_progress",
                        "nodes": [hero_id.0],
                        "value": true,
                        "run_id": "run_other",
                        "agent": "Codex"
                    }]
                }),
            )
            .unwrap_err();
        let conflict_json = conflict.to_json_value();
        assert_eq!(conflict_json["code"], "BATCH_OP_FAILED");
        assert_eq!(conflict_json["cause"]["code"], "NODE_IN_PROGRESS_BY_OTHER_RUN");
        assert_eq!(conflict_json["cause"]["received"]["existing_run_id"], "run_test_1");

        // Clearing on run end removes the entry
        let cleared = engine.doc.clear_in_progress_for_run_or_agent(Some("run_test_1"), Some("Claude Code"));
        assert_eq!(cleared, vec![hero_id]);
        assert!(engine.doc.get_in_progress(hero_id).is_none());
        designos_doc::set_thread_mock_now_ms(None);
    }

    #[test]
    fn undo_does_not_touch_flag() {
        designos_doc::set_thread_mock_now_ms(Some(2_100_000));
        let mut engine = Engine::default();
        let page = engine.resolve_default_page().unwrap();
        let created =
            engine.call("designos_create_node", json!({ "parent_id": page.0, "kind": "frame", "name": "Card", "width": 320.0, "height": 200.0 })).unwrap();
        let frame_id = created["node"]["id"].as_u64().unwrap();

        engine.call("designos_update_node", json!({ "node_id": frame_id, "name": "Card Edited" })).unwrap();
        let undo_before = engine.history.undo_depth();

        // Flag via standalone tool: must not push a History step
        let flag_res = engine.call("designos_set_in_progress", json!({ "node_ids": [frame_id], "value": true, "stale_after_s": 120 })).unwrap();
        assert_eq!(flag_res["changed"], true);
        assert_eq!(engine.history.undo_depth(), undo_before, "set_in_progress must not add an undo step");

        // Undo reverts the rename while keeping the in_progress flag intact
        let undo_res = engine.call("designos_undo", json!({})).unwrap();
        assert_eq!(undo_res["changed"], true);
        assert_eq!(engine.doc.get(NodeId(frame_id)).unwrap().name, "Card");
        assert!(engine.doc.get_in_progress(NodeId(frame_id)).is_some(), "undo must not clear in_progress flag");
        designos_doc::set_thread_mock_now_ms(None);
    }

    #[test]
    fn not_root_refused() {
        let mut engine = Engine::default();
        let page = engine.resolve_default_page().unwrap();
        let res = engine
            .call(
                "designos_apply_batch",
                json!({
                    "ops": [
                        {
                            "op": "insert",
                            "parent": page.0,
                            "as": "root_frame",
                            "kind": "frame",
                            "name": "Root",
                            "width": 300.0,
                            "height": 200.0
                        },
                        {
                            "op": "insert",
                            "parent": "@root_frame",
                            "as": "inner_text",
                            "kind": "text",
                            "name": "Inner Label",
                            "text": "Hello"
                        }
                    ]
                }),
            )
            .unwrap();
        let root_id = res["name_to_id"]["root_frame"].as_u64().unwrap();
        let text_id = res["name_to_id"]["inner_text"].as_u64().unwrap();

        // Standalone tool refuses non-root text and reports root_frame_id
        let err = engine.call("designos_set_in_progress", json!({ "node_ids": [text_id], "value": true })).unwrap_err();
        let err_json = err.to_json_value();
        assert_eq!(err_json["code"], "IN_PROGRESS_TARGET_NOT_ROOT");
        assert_eq!(err_json["received"]["root_frame_id"], root_id);
        assert_eq!(err_json["valid_options"], json!([root_id.to_string()]));

        // Batch op also refuses non-root text and reports root_frame_id
        let batch_err = engine
            .call(
                "designos_apply_batch",
                json!({
                    "ops": [{
                        "op": "set_in_progress",
                        "nodes": [text_id],
                        "value": true
                    }]
                }),
            )
            .unwrap_err();
        let batch_err_json = batch_err.to_json_value();
        assert_eq!(batch_err_json["code"], "BATCH_OP_FAILED");
        assert_eq!(batch_err_json["cause"]["code"], "IN_PROGRESS_TARGET_NOT_ROOT");
        assert_eq!(batch_err_json["cause"]["received"]["root_frame_id"], root_id);
        assert_eq!(batch_err_json["cause"]["valid_options"], json!([root_id.to_string()]));
    }

    #[test]
    fn mcp_set_in_progress_goes_stale_after_15_minutes_without_env_run_id() {
        designos_doc::set_thread_mock_now_ms(Some(5_000_000));
        std::env::remove_var("DESIGNOS_AGENT_RUN_ID");
        std::env::remove_var("DESIGNOS_AGENT_RUNTIME");
        let mut engine = Engine::default();
        let page = engine.resolve_default_page().unwrap();
        let created = engine
            .call("designos_create_node", json!({ "parent_id": page.0, "kind": "frame", "name": "Killed Agent Frame", "width": 320.0, "height": 200.0 }))
            .unwrap();
        let frame_id = created["node"]["id"].as_u64().unwrap();

        engine.call("designos_set_in_progress", json!({ "node_ids": [frame_id], "value": true })).unwrap();
        assert!(engine.doc.get_in_progress(NodeId(frame_id)).is_some());

        // Advance clock 15 minutes with no status file: MCP-set flag must go stale and prune.
        designos_doc::advance_thread_mock_now_ms(15 * 60 * 1000);
        assert!(engine.doc.get_in_progress(NodeId(frame_id)).is_none(), "MCP-set in_progress flag must go stale after 15 minutes");
        assert_eq!(engine.doc.prune_in_progress(), vec![NodeId(frame_id)]);

        // Also verify Studio-provisioned session dir (mcp-config.json / context.json) with DESIGNOS_AGENT_RUN_ID unset in process env.
        let tmp_dir = std::env::temp_dir().join(format!("designos_mcp_e15_7_sess_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp_dir);
        std::fs::create_dir_all(&tmp_dir).unwrap();
        std::fs::write(
            tmp_dir.join("mcp-config.json"),
            serde_json::to_vec(&json!({
                "mcpServers": {
                    "designos": {
                        "command": "designos-mcp",
                        "env": {
                            "DESIGNOS_AGENT_RUN_ID": "p100-tab-1-1-1",
                            "DESIGNOS_AGENT_RUNTIME": "claude"
                        }
                    }
                }
            }))
            .unwrap(),
        )
        .unwrap();
        let mut session = crate::server::Session::with_session_dir(&tmp_dir, &tmp_dir).unwrap();
        let spage = session.engine.resolve_default_page().unwrap();
        let screated = session
            .engine
            .call("designos_create_node", json!({ "parent_id": spage.0, "kind": "frame", "name": "Session Frame", "width": 320.0, "height": 200.0 }))
            .unwrap();
        let sframe_id = screated["node"]["id"].as_u64().unwrap();
        session.engine.call("designos_set_in_progress", json!({ "node_ids": [sframe_id], "value": true })).unwrap();
        let sip = session.engine.doc.get_in_progress(NodeId(sframe_id)).expect("session frame flagged");
        assert_eq!(sip.run_id.as_deref(), Some("p100-tab-1-1-1"));
        assert_eq!(sip.agent.as_deref(), Some("claude"));
        designos_doc::advance_thread_mock_now_ms(15 * 60 * 1000);
        assert!(session.engine.doc.get_in_progress(NodeId(sframe_id)).is_none(), "session flag must go stale after 15 minutes with no status file");
        let _ = std::fs::remove_dir_all(&tmp_dir);
        designos_doc::set_thread_mock_now_ms(None);
    }

    #[test]
    fn set_style_upsert_preserves_existing_sort_position_and_leaves_new_style_none() {
        let mut engine = Engine::default();
        let _ = engine.resolve_default_page().unwrap();
        engine.doc.styles.insert("2:202".into(), Style::Fill(vec![Paint::solid(Color::WHITE)]));
        engine.doc.style_meta.insert(
            "2:202".into(),
            StyleMeta {
                name: "Brand/Primary".into(),
                key: Some("lib-key".into()),
                description: "Primary fill".into(),
                property: StyleProperty::Fill,
                sort_position: Some("!~~p~oL".into()),
                ..Default::default()
            },
        );

        engine
            .call(
                "designos_apply_batch",
                json!({
                    "ops": [{
                        "op": "set_style",
                        "upsert": [
                            { "key": "2:202", "fill": "#112233" },
                            { "key": "brand:new", "name": "Brand/New", "fill": "#445566" }
                        ]
                    }]
                }),
            )
            .expect("set_style batch should succeed");

        assert_eq!(engine.doc.style_meta["2:202"].sort_position.as_deref(), Some("!~~p~oL"));
        assert_eq!(engine.doc.styles["2:202"], Style::Fill(vec![Paint::solid(Color::rgb(0x11, 0x22, 0x33))]));
        assert_eq!(engine.doc.style_meta["brand:new"].sort_position, None);
    }
}
