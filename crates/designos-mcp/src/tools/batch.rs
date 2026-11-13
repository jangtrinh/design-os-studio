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
use crate::tools::relayout_scope::{seeds_after_commands, update_affects_layout};
use crate::tools::text_remeasure::{resize_hug_text, BoundTextIndex, VariablesBefore};
use crate::tools::{build_created_node, build_updated_node, validate_create_fields, validate_update_fields, UpdateFields};
use designos_doc::live_patch::{live_ops_for_command, LiveOp, SiblingHint};
use designos_doc::{
    apply_style_values_with_fields, recorded_text_paragraph_fields, resolve_easeui_color, style_property, text_style_paragraph_fields_after_update, Color,
    Command, Document, Effect, LayoutGrid, NodeId, NodeKind, NodeSpec, Paint, Rect, SpecDataSnapshot, Style, StyleMeta, StyleProperty, TextStyle, Variable,
    VariableCollection, VariableKind, VariableMode, VariableStore, VariableValue,
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
    let connect_end = json!({
        "anyOf": [
            { "type": "integer", "minimum": 1 },
            { "type": "string", "pattern": "^(@[A-Za-z0-9_-]{1,64}|\\$prev)(/.+)?$" },
            {
                "type": "object",
                "required": ["point"],
                "additionalProperties": false,
                "properties": {
                    "point": {
                        "type": "array",
                        "items": coordinate.clone(),
                        "minItems": 2,
                        "maxItems": 2
                    }
                }
            }
        ],
        "description": "Connector endpoint: numeric node ID, '$prev', '@<as>' / '@<as>/<name path>', or {\"point\": [x, y]} in page space."
    });
    let magnet_schema = json!({
        "type": "string",
        "enum": ["auto", "top", "right", "bottom", "left", "center"]
    });
    let cap_schema = json!({
        "type": "string",
        "enum": ["none", "arrow", "triangle", "circle", "diamond"]
    });
    let mut op_properties = serde_json::Map::new();
    op_properties.insert(
        "op".into(),
        json!({
            "type": "string",
            "enum": ["apply_spec", "insert", "create", "update", "move", "delete", "set_slot", "set_variables", "set_style", "set_in_progress", "connect", "table"]
        }),
    );
    op_properties.insert("as".into(), json!({ "type": "string", "pattern": "^[A-Za-z0-9_-]{1,64}$" }));
    op_properties.insert("parent".into(), node_or_ref.clone());
    op_properties.insert("parent_id".into(), node_or_ref.clone());
    op_properties.insert("node".into(), node_or_ref.clone());
    op_properties.insert("node_id".into(), node_or_ref.clone());
    op_properties.insert("nodes".into(), json!({ "type": "array", "items": node_or_ref.clone(), "minItems": 1, "maxItems": 200 }));
    op_properties.insert("node_ids".into(), json!({ "type": "array", "items": node_or_ref, "minItems": 1, "maxItems": 200 }));
    op_properties.insert("from".into(), connect_end.clone());
    op_properties.insert("to".into(), connect_end);
    op_properties.insert("from_magnet".into(), magnet_schema.clone());
    op_properties.insert("to_magnet".into(), magnet_schema);
    op_properties.insert("line".into(), json!({ "type": "string", "enum": ["elbowed", "straight", "curved"] }));
    op_properties.insert("start_cap".into(), cap_schema.clone());
    op_properties.insert("end_cap".into(), cap_schema);
    op_properties.insert("label".into(), json!({ "type": "string", "maxLength": 10000 }));
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
    op_properties.insert("action".into(), json!({ "type": "string", "enum": crate::tools::batch_table::TABLE_ACTIONS }));
    op_properties.insert("rows".into(), json!({ "type": "integer", "minimum": 1, "maximum": 500 }));
    op_properties.insert("cols".into(), json!({ "type": "integer", "minimum": 1, "maximum": 500 }));
    op_properties.insert("col_width".into(), json!({ "type": "number", "minimum": 24, "maximum": 1000000 }));
    op_properties.insert("row_height".into(), json!({ "type": "number", "minimum": 24, "maximum": 1000000 }));
    op_properties.insert("row".into(), json!({ "type": "integer", "minimum": 0 }));
    op_properties.insert("col".into(), json!({ "type": "integer", "minimum": 0 }));
    op_properties.insert("at".into(), json!({ "type": "integer", "minimum": 0 }));
    op_properties.insert("from_index".into(), json!({ "type": "integer", "minimum": 0 }));
    op_properties.insert("to_index".into(), json!({ "type": "integer", "minimum": 0 }));
    op_properties.insert("count".into(), json!({ "type": "integer", "minimum": 1, "maximum": 500 }));
    op_properties.insert("size".into(), json!({ "type": "number", "minimum": 24, "maximum": 1000000 }));
    op_properties.insert("row_span".into(), json!({ "type": "integer", "minimum": 1 }));
    op_properties.insert("col_span".into(), json!({ "type": "integer", "minimum": 1 }));
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
            "ops": [
                { "op": "connect", "as": "ab", "from": "@a", "from_magnet": "right", "to": "@b", "to_magnet": "left", "line": "elbowed", "end_cap": "arrow", "label": "ok" }
            ]
        },
        {
            "ops": [
                { "op": "table", "as": "t", "action": "create", "parent": 1, "x": 40, "y": 40, "rows": 3, "cols": 3 },
                { "op": "table", "node": "@t", "action": "set_cell_text", "row": 0, "col": 0, "text": "Header" },
                { "op": "table", "node": "@t", "action": "merge", "row": 0, "col": 0, "row_span": 1, "col_span": 2 }
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
        "description": "Execute 1..=200 write operations (apply_spec, insert/create, update, move, delete, set_slot, set_variables, set_style, set_in_progress, connect, table) as a single atomic transaction and one undo group. Later ops may reference nodes created by earlier ops via '$prev', '@<as>', or '@<as>/<name path>'. With dry_run=true, computes and returns created IDs, name_to_id map, touched_roots, and touched_bounds without mutating the document or publishing a live patch. Pass idempotency_key to deduplicate retries (a key names one logical write; after a reconnect, call designos_get_node before trusting a replayed id). Note: in a live Studio session, if the editor remaps a colliding ID on replay, the reply holds the session IDs until the next canvas reload. Examples: (1) create + $prev: {\"ops\":[{\"op\":\"create\",\"parent\":1,\"kind\":\"frame\",\"name\":\"Header\"},{\"op\":\"create\",\"parent\":\"$prev\",\"kind\":\"text\",\"text\":\"Title\"}]}; (2) apply_spec with as + @handle/Child update: {\"ops\":[{\"op\":\"apply_spec\",\"as\":\"hero\",\"spec\":{\"kind\":\"frame\",\"name\":\"Hero\",\"children\":[{\"kind\":\"text\",\"name\":\"Child\",\"text\":\"Hi\"}]}},{\"op\":\"update\",\"node\":\"@hero/Child\",\"opacity\":0.9}]}; (3) set_slot: {\"ops\":[{\"op\":\"set_slot\",\"node\":\"@card\",\"slot\":\"body\",\"fill\":[{\"kind\":\"text\",\"text\":\"Body\"}]}]}; (4) dry_run: {\"dry_run\":true,\"ops\":[{\"op\":\"insert\",\"kind\":\"rectangle\",\"name\":\"Box\"}]}; (5) table: {\"ops\":[{\"op\":\"table\",\"as\":\"t\",\"action\":\"create\",\"parent\":1,\"rows\":3,\"cols\":3},{\"op\":\"table\",\"node\":\"@t\",\"action\":\"set_cell_text\",\"row\":0,\"col\":0,\"text\":\"Header\"}]}.",
        "inputSchema": {
            "type": "object",
            "required": ["ops"],
            "additionalProperties": false,
            "properties": {
                "ops": {
                    "type": "array",
                    "minItems": 1,
                    "maxItems": 200,
                    "description": "Ordered batch operations (apply_spec, insert/create, update, move, delete, set_slot, set_variables, set_style, set_in_progress, connect, table). Use op='connect' with from/to (node ID, '@handle', '$prev', or {\"point\":[x,y]}), optional from_magnet/to_magnet (auto|top|right|bottom|left|center), line (elbowed|straight|curved), start_cap/end_cap (none|arrow|triangle|circle|diamond), label, and node (existing connector ID to update). Use op='table' with action (create|insert_rows|delete_rows|insert_cols|delete_cols|move_row|move_col|resize_row|resize_col|resize|merge|unmerge|set_cell_text) and from_index/to_index (0-based) for move_row/move_col; reference cells via '@<table_as>/r<row>c<col>' (0-based, e.g. '@t/r0c0').",
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
        "outputSchema": tool_output_schema_from_object(success_schema)
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

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FreePointEnd {
    point: [f32; 2],
}

#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
enum ConnectEndInput {
    Node(NodeRef),
    Point(FreePointEnd),
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ConnectOp {
    #[allow(dead_code)]
    op: String,
    #[serde(rename = "as")]
    alias: Option<String>,
    #[serde(default, alias = "node_id")]
    node: Option<NodeRef>,
    #[serde(default)]
    from: Option<ConnectEndInput>,
    #[serde(default)]
    to: Option<ConnectEndInput>,
    #[serde(default)]
    from_magnet: Option<String>,
    #[serde(default)]
    to_magnet: Option<String>,
    #[serde(default)]
    line: Option<String>,
    #[serde(default)]
    start_cap: Option<String>,
    #[serde(default)]
    end_cap: Option<String>,
    #[serde(default)]
    label: Option<String>,
    #[serde(default)]
    name: Option<String>,
}

fn parse_magnet(s: &str, op_idx: usize, field: &str) -> Result<designos_doc::Magnet, ToolError> {
    match s.trim().to_ascii_lowercase().as_str() {
        "auto" => Ok(designos_doc::Magnet::Auto),
        "top" => Ok(designos_doc::Magnet::Top),
        "right" => Ok(designos_doc::Magnet::Right),
        "bottom" => Ok(designos_doc::Magnet::Bottom),
        "left" => Ok(designos_doc::Magnet::Left),
        "center" => Ok(designos_doc::Magnet::Center),
        other => Err(op_failed_error(
            op_idx,
            "connect",
            ErrorCode::InvalidArguments,
            &format!("/ops/{op_idx}/{field}"),
            format!("invalid magnet '{other}' (expected auto, top, right, bottom, left, or center)"),
        )),
    }
}

fn parse_connector_line(s: &str, op_idx: usize) -> Result<designos_doc::ConnectorLine, ToolError> {
    match s.trim().to_ascii_lowercase().as_str() {
        "elbowed" => Ok(designos_doc::ConnectorLine::Elbowed),
        "straight" => Ok(designos_doc::ConnectorLine::Straight),
        "curved" => Ok(designos_doc::ConnectorLine::Curved),
        other => Err(op_failed_error(
            op_idx,
            "connect",
            ErrorCode::InvalidArguments,
            &format!("/ops/{op_idx}/line"),
            format!("invalid connector line '{other}' (expected elbowed, straight, or curved)"),
        )),
    }
}

fn parse_connector_cap(s: &str, op_idx: usize, field: &str) -> Result<designos_doc::ConnectorCap, ToolError> {
    match s.trim().to_ascii_lowercase().as_str() {
        "none" => Ok(designos_doc::ConnectorCap::None),
        "arrow" => Ok(designos_doc::ConnectorCap::Arrow),
        "triangle" => Ok(designos_doc::ConnectorCap::Triangle),
        "circle" => Ok(designos_doc::ConnectorCap::Circle),
        "diamond" => Ok(designos_doc::ConnectorCap::Diamond),
        other => Err(op_failed_error(
            op_idx,
            "connect",
            ErrorCode::InvalidArguments,
            &format!("/ops/{op_idx}/{field}"),
            format!("invalid connector cap '{other}' (expected none, arrow, triangle, circle, or diamond)"),
        )),
    }
}

/// What a `connect` update does to one end the connector already stores.
enum ConnectEndChange {
    /// The op does not name the end: it stays as stored.
    Keep,
    /// The op names a layer: the end attaches to it.
    Attach(NodeId),
    /// The op names a page-space point: the end becomes free there.
    Free(designos_doc::Vec2),
}

impl ConnectEndChange {
    /// The layer the end rests on once the op is applied. A kept end whose layer is gone is free at
    /// its last point and names none.
    fn attached_node(&self, doc: &Document, stored: designos_doc::ConnectorEnd) -> Option<NodeId> {
        match self {
            Self::Attach(id) => Some(*id),
            Self::Keep => stored.node.filter(|id| doc.get(*id).is_some()),
            Self::Free(_) => None,
        }
    }
}

fn validate_connect_end_pre_run(end: &ConnectEndInput, declared_as: &BTreeSet<String>, has_prev: bool, op_idx: usize, field: &str) -> Result<(), ToolError> {
    match end {
        ConnectEndInput::Node(nr) => validate_ref_pre_run(nr, declared_as, has_prev, op_idx, "connect", field),
        ConnectEndInput::Point(p) => {
            let [x, y] = p.point;
            if !x.is_finite() || !y.is_finite() || !(-1_000_000.0..=1_000_000.0).contains(&x) || !(-1_000_000.0..=1_000_000.0).contains(&y) {
                return Err(op_failed_error(
                    op_idx,
                    "connect",
                    ErrorCode::ValueOutOfRange,
                    &format!("/ops/{op_idx}/{field}/point"),
                    "free endpoint coordinates must be finite numbers in -1000000..=1000000",
                ));
            }
            Ok(())
        }
    }
}

fn page_of_node(doc: &Document, id: NodeId) -> Option<NodeId> {
    let mut cur = Some(id);
    let mut steps = 0usize;
    let limit = doc.nodes.len();
    while let Some(cid) = cur {
        if steps >= limit {
            return None;
        }
        steps += 1;
        let n = doc.get(cid)?;
        if matches!(n.kind, NodeKind::Page { .. }) {
            return Some(cid);
        }
        cur = n.parent;
    }
    None
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
    Connect(ConnectOp),
    Table(crate::tools::batch_table::TableOp),
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
            Self::Connect(_) => "connect",
            Self::Table(_) => "table",
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
            Self::Connect(o) => o.alias.as_deref(),
            Self::Table(o) => o.alias.as_deref(),
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

fn flatten_batch_command(command: Command, out: &mut Vec<Command>) {
    match command {
        Command::Batch(commands) => {
            for command in commands {
                flatten_batch_command(command, out);
            }
        }
        command => out.push(command),
    }
}

fn slot_name_matches(candidate: &str, requested: &str) -> bool {
    candidate == requested
        || candidate.eq_ignore_ascii_case(requested)
        || candidate.split('#').next() == Some(requested)
        || candidate.split('#').next().is_some_and(|name| name.eq_ignore_ascii_case(requested))
}

/// Returns only the descendants owned by this instance's template. Nested instance
/// subtrees belong to their own component and must not satisfy a slot lookup on the
/// containing instance.
fn direct_instance_descendants(doc: &Document, instance: NodeId) -> Vec<NodeId> {
    let Some(root) = doc.get(instance) else {
        return Vec::new();
    };
    let mut descendants = Vec::new();
    let mut stack: Vec<NodeId> = root.children.iter().copied().rev().collect();
    while let Some(id) = stack.pop() {
        let Some(node) = doc.get(id) else {
            continue;
        };
        descendants.push(id);
        if !matches!(node.kind, NodeKind::Instance { .. }) {
            stack.extend(node.children.iter().copied().rev());
        }
    }
    descendants
}

fn slot_container_in_instance(doc: &Document, instance: NodeId, requested: &str) -> Option<NodeId> {
    for node_id in direct_instance_descendants(doc, instance) {
        let Some(node) = doc.get(node_id) else {
            continue;
        };
        let binding_matches = node.component.bindings.get(&designos_doc::OverrideField::SlotContent).is_some_and(|name| slot_name_matches(name, requested));
        let source_binding_matches = node
            .component
            .source
            .and_then(|source| doc.get(source))
            .and_then(|source| source.component.bindings.get(&designos_doc::OverrideField::SlotContent).filter(|name| slot_name_matches(name, requested)));
        if (binding_matches || source_binding_matches.is_some()) && designos_doc::is_slot_container(doc, node.id) {
            return Some(node.id);
        }
    }

    // Older component documents may mark a slot with `is_slot` but omit the
    // binding. Instance overrides can also rename the mirrored node while the
    // source slot keeps its canonical name. Keep the name fallback after the
    // explicit binding pass so a matching layer name cannot shadow a binding.
    for node_id in direct_instance_descendants(doc, instance) {
        let Some(node) = doc.get(node_id) else {
            continue;
        };
        let source_name_matches = node.component.source.and_then(|source| doc.get(source)).is_some_and(|source| slot_name_matches(&source.name, requested));
        if (slot_name_matches(&node.name, requested) || source_name_matches) && designos_doc::is_slot_container(doc, node.id) {
            return Some(node.id);
        }
    }
    None
}

fn slot_value_key(root: &designos_doc::Node, requested: &str) -> Option<String> {
    root.component.values.keys().find(|name| slot_name_matches(name, requested)).cloned()
}

fn resolve_slot_container(doc: &Document, instance: NodeId, requested: &str) -> Option<NodeId> {
    // Prefer the component binding/source metadata. A mirrored layer can be
    // renamed by an override, while an unrelated layer can still have the
    // requested slot's old display name.
    slot_container_in_instance(doc, instance, requested)
        .or_else(|| designos_doc::resolve_override_path(doc, instance, requested).ok().filter(|&id| designos_doc::is_slot_container(doc, id)))
}

fn batch_error(code: ErrorCode, failed_op: Option<usize>, op: Option<&str>, path: &str, message: impl Into<String>) -> ToolError {
    let mut err = ToolError::new(code, path, message);
    err.failed_op = failed_op;
    err.op = op.map(ToOwned::to_owned);
    err
}

pub(crate) fn op_failed_error(failed_op: usize, op: &str, inner_code: ErrorCode, path: &str, message: impl Into<String>) -> ToolError {
    let cause = ToolError::new(inner_code, path, message);
    ToolError::batch_op_failed(failed_op, op, cause)
}

pub(crate) fn wrap_op_err(failed_op: usize, op: &str, op_path: &str, mut cause: ToolError) -> ToolError {
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

pub(crate) fn validate_ref_pre_run(
    r: &NodeRef,
    declared_as: &BTreeSet<String>,
    has_prev: bool,
    op_idx: usize,
    op_name: &str,
    field: &str,
) -> Result<(), ToolError> {
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
                    bindings: None,
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
            "connect" => {
                let a: ConnectOp = serde_json::from_value(val).map_err(|e| {
                    let cause = crate::tools::errors::from_serde_error_with_args(&e, &op_path, false, Some(&raw_op));
                    ToolError::batch_op_failed(idx, "connect", cause)
                })?;
                if a.node.is_none() {
                    if a.from.is_none() {
                        return Err(op_failed_error(
                            idx,
                            "connect",
                            ErrorCode::MissingField,
                            &format!("{op_path}/from"),
                            "connect requires 'from' when creating a connector",
                        ));
                    }
                    if a.to.is_none() {
                        return Err(op_failed_error(
                            idx,
                            "connect",
                            ErrorCode::MissingField,
                            &format!("{op_path}/to"),
                            "connect requires 'to' when creating a connector",
                        ));
                    }
                }
                if let Some(nr) = &a.node {
                    validate_ref_pre_run(nr, &declared_as, has_prev, idx, "connect", "node")?;
                }
                if let Some(f) = &a.from {
                    validate_connect_end_pre_run(f, &declared_as, has_prev, idx, "from")?;
                }
                if let Some(t) = &a.to {
                    validate_connect_end_pre_run(t, &declared_as, has_prev, idx, "to")?;
                }
                if let Some(m) = &a.from_magnet {
                    parse_magnet(m, idx, "from_magnet")?;
                }
                if let Some(m) = &a.to_magnet {
                    parse_magnet(m, idx, "to_magnet")?;
                }
                if let Some(l) = &a.line {
                    parse_connector_line(l, idx)?;
                }
                if let Some(c) = &a.start_cap {
                    parse_connector_cap(c, idx, "start_cap")?;
                }
                if let Some(c) = &a.end_cap {
                    parse_connector_cap(c, idx, "end_cap")?;
                }
                has_prev = true;
                ParsedOp::Connect(a)
            }
            "table" => {
                let a = crate::tools::batch_table::parse_and_validate(val, &raw_op, idx, &declared_as, has_prev)?;
                has_prev = true;
                ParsedOp::Table(a)
            }
            other => {
                return Err(batch_error(
                    ErrorCode::InvalidArguments,
                    Some(idx),
                    Some(other),
                    &format!("/ops/{idx}/op"),
                    format!("unsupported batch op '{other}'"),
                )
                .with_valid_options(vec![
                    "apply_spec",
                    "insert",
                    "update",
                    "move",
                    "delete",
                    "set_variables",
                    "set_style",
                    "set_slot",
                    "connect",
                    "table",
                ]));
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
                if matches!(doc.get(cur).map(|n| &n.kind), Some(NodeKind::Table(_))) {
                    if let Some((r_idx, c_idx)) = crate::tools::batch_table::parse_table_cell_segment(seg) {
                        cur = crate::tools::batch_table::resolve_table_cell(doc, cur, r_idx, c_idx).ok_or_else(|| {
                            op_failed_error(
                                op_idx,
                                op_name,
                                ErrorCode::ValueOutOfRange,
                                &path,
                                format!("cell reference '{seg}' in '{s}' is out of bounds for table {}", cur.0),
                            )
                        })?;
                        continue;
                    }
                }
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
    relayout_for_nodes(doc, &[id]);
}

/// Re-solves the auto-layout frames in `ids` and in their parents, then climbs while a frame's own
/// size changes. For a layer whose own edit can change its size or place: the solver does not see a
/// leaf's size change, so the frame that holds it must run.
pub(crate) fn relayout_for_nodes(doc: &mut Document, ids: &[NodeId]) {
    relayout_seeds(doc, ids, &[]);
}

/// Re-solves the containers a layer left or joined and climbs only while a container's own size
/// changes. A frame that kept its size leaves the frames around it as they were stored.
pub(crate) fn relayout_containers(doc: &mut Document, containers: &[NodeId]) {
    relayout_seeds(doc, &[], containers);
}

/// One solve over `leaves` (with their parents) and `containers`, each frame once, deepest first.
pub(crate) fn relayout_seeds(doc: &mut Document, leaves: &[NodeId], containers: &[NodeId]) {
    #[cfg(test)]
    RELAYOUT_PASSES.with(|c| c.set(c.get() + 1));
    let mut seeds = Vec::with_capacity(leaves.len() * 2 + containers.len());
    for &id in leaves {
        seeds.push(id);
        let is_auto_layout = doc.get(id).and_then(|n| n.kind.frame_props()).and_then(|f| f.auto_layout.as_ref()).is_some();
        if !is_auto_layout {
            if let Some(parent) = doc.get(id).and_then(|n| n.parent) {
                seeds.push(parent);
            }
        }
    }
    seeds.extend_from_slice(containers);
    doc.relayout_seeds_upward(&seeds);
    doc.flush_pending_relayout();
}

#[cfg(test)]
thread_local! {
    static RELAYOUT_PASSES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Number of [`relayout_seeds`] passes on this thread since the last call.
#[cfg(test)]
pub(crate) fn take_relayout_passes() -> usize {
    RELAYOUT_PASSES.with(|c| c.replace(0))
}

/// Ids of the nodes `cmds` replace with an `Update`.
fn updated_node_ids(cmds: &[Command]) -> Vec<NodeId> {
    cmds.iter()
        .filter_map(|c| match c {
            Command::Update { node } => Some(node.id),
            _ => None,
        })
        .collect()
}

/// Re-lays out the auto-layout containers of every node in `ids` and their auto-layout ancestors,
/// and queues `ids` for the pass at commit. A hug frame around a label whose size an op changed
/// would otherwise keep its old size.
fn relayout_containers_of(doc: &mut Document, ids: &[NodeId], relayout_targets: &mut Vec<NodeId>) {
    relayout_for_nodes(doc, ids);
    for &id in ids {
        if !relayout_targets.contains(&id) {
            relayout_targets.push(id);
        }
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
    let mut unmapped_modes: Vec<(String, String)> = Vec::new();

    for c_in in &op.upsert_collections {
        if c_in.id.trim().is_empty() {
            return Err(op_failed_error(op_idx, "set_variables", ErrorCode::InvalidArguments, &path, "collection id must not be empty"));
        }
        let existing = collections.get(&c_in.id).cloned();
        if existing.as_ref().is_some_and(VariableCollection::is_library) || c_in.source_library_key.is_some() {
            return Err(op_failed_error(op_idx, "set_variables", ErrorCode::InvalidArguments, &path, "library variables are read-only"));
        }
        if existing.as_ref().is_some_and(|e| e.is_extension()) && (!c_in.modes.is_empty() || c_in.default_mode.is_some()) {
            return Err(op_failed_error(
                op_idx,
                "set_variables",
                ErrorCode::InvalidArguments,
                &path,
                format!("collection '{}' extends another collection: its modes follow the parent, set values per mode with upsert_variables", c_in.id),
            ));
        }
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
        if let Some(ex) = &existing {
            // modes the parent no longer has: its extensions stop mapping to them, as when a mode is deleted in the panel
            unmapped_modes.extend(ex.modes.iter().filter(|m| !modes.iter().any(|n| n.id == m.id)).map(|m| (c_in.id.clone(), m.id.clone())));
        }
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

    // (variable, mode, value); `None` is a null value: reset the override of an extended collection's mode
    let mut writes: Vec<(String, String, Option<VariableValue>)> = Vec::new();
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
        if existing.as_ref().is_some_and(|v| collections.get(&v.collection).is_some_and(VariableCollection::is_library))
            || collections.get(&collection_id).is_some_and(VariableCollection::is_library)
        {
            return Err(op_failed_error(op_idx, "set_variables", ErrorCode::InvalidArguments, &path, "library variables are read-only"));
        }
        // values are written below through the variable store (set_value, set_alias, reset_override), which owns the
        // rules for extended collections and alias cycles; this record only keeps what the variable already has
        let values = existing.as_ref().map(|e| e.values.clone()).unwrap_or_default();
        for (mode_id, raw_val) in &v_in.values {
            if raw_val.is_null() {
                writes.push((v_in.id.clone(), mode_id.clone(), None));
                continue;
            }
            let parsed_val = parse_variable_value(raw_val, kind).map_err(|(code, msg)| op_failed_error(op_idx, "set_variables", code, &path, msg))?;
            writes.push((v_in.id.clone(), mode_id.clone(), Some(parsed_val)));
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

    let rules_before = VariableStore { variables: doc.variables.clone(), collections: doc.variable_collections.clone() }.extension_violations();
    let mut store = VariableStore { variables, collections };
    for del_id in &op.delete_variables {
        if store.variables.get(del_id).is_some_and(|v| store.collections.get(&v.collection).is_some_and(VariableCollection::is_library)) {
            return Err(op_failed_error(op_idx, "set_variables", ErrorCode::InvalidArguments, &path, "library variables are read-only"));
        }
        store.forget_overrides_of(del_id);
        store.variables.remove(del_id);
        for col in store.collections.values_mut() {
            col.variables.retain(|v| v != del_id);
        }
    }
    for (coll, mode) in &unmapped_modes {
        store.unmap_parent_mode(coll, mode);
    }
    if let Err(why) = store.apply_value_writes(writes) {
        return Err(op_failed_error(op_idx, "set_variables", ErrorCode::InvalidArguments, &path, why));
    }
    // an import can leave a store that already breaks an extension rule; the op is refused only for a break it adds
    if let Err(why) = store.rules_added_since(&rules_before) {
        return Err(op_failed_error(op_idx, "set_variables", ErrorCode::InvalidArguments, &path, why));
    }
    let VariableStore { variables, collections } = store;

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
        "animation" => Ok(StyleProperty::Animation),
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
        } else if let Some(ex) = existing_style.clone() {
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
                Style::Animation(_) => StyleProperty::Animation,
            }),
        };
        let name = s_in.name.clone().or_else(|| existing_meta.as_ref().map(|m| m.name.clone())).unwrap_or_else(|| key.clone());
        let description = s_in.description.clone().or_else(|| existing_meta.as_ref().map(|m| m.description.clone())).unwrap_or_default();
        let bindings = existing_meta.as_ref().map(|m| m.bindings.clone()).unwrap_or_default();
        let (lib_key, sort_position, existing_fields) = match existing_meta.as_ref() {
            Some(m) => (m.key.clone(), m.sort_position.clone(), m.text_paragraph_fields),
            None => (None, None, None),
        };
        let ownership = match (&existing_style, &style_val) {
            // a text style this op creates owns every paragraph field
            (None, Style::Text(_)) => Some(0b111),
            // a stored one without metadata owns none until the op changes one
            (Some(Style::Text(_)), Style::Text(_)) => recorded_text_paragraph_fields(existing_meta.as_ref()),
            _ => existing_fields,
        };
        let paragraph_fields = text_style_paragraph_fields_after_update(styles.get(&key), &style_val, ownership);
        styles.insert(key.clone(), style_val);
        style_meta.insert(
            key.clone(),
            StyleMeta { name, key: lib_key, description, property: prop, bindings, sort_position, text_paragraph_fields: paragraph_fields },
        );
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
    // Node -> index in `cmds` of the `Update` this op already built for it.
    let mut pending_updates: BTreeMap<NodeId, usize> = BTreeMap::new();
    let mut applied_text_nodes: BTreeSet<NodeId> = BTreeSet::new();
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
            StyleProperty::Animation => "animation",
        };
        let cur =
            doc.get(node_id).ok_or_else(|| op_failed_error(op_idx, "set_style", ErrorCode::NodeNotFound, &path, format!("node {} not found", node_id.0)))?;
        let mut updated = cur.clone();
        if prop == StyleProperty::Fill {
            // The style's paints replace the node's, so a variable bound to the old ones would keep drawing over them.
            designos_doc::detach_fill_paints(&mut updated);
        }
        if prop == StyleProperty::Text && matches!(&updated.kind, NodeKind::Text(_)) {
            designos_doc::detach_text_style_bindings(&mut updated);
            if cur.style_refs.get("text") == Some(&resolved_key) {
                updated.hyperlink = None;
            }
            applied_text_nodes.insert(node_id);
        }
        updated.style_refs.insert(ref_slot.into(), resolved_key.clone());
        apply_style_values_with_fields(&mut updated, st, &resolved_key, style_meta.get(&resolved_key).and_then(|m| m.text_paragraph_fields));
        if prop == StyleProperty::Text {
            resize_hug_text(doc, &mut updated);
        }
        if &updated != cur {
            pending_updates.insert(node_id, cmds.len());
            cmds.push(Command::Update { node: Box::new(updated) });
        }
    }

    // Every text linked to an upserted text style takes its new values. A node this op already
    // rewrote (a style apply) is updated in place, so its earlier changes stay in the one `Update`.
    for (key, style) in &styles {
        let Style::Text(_) = style else { continue };
        if doc.styles.get(key) == Some(style) || !doc.styles.contains_key(key) {
            continue;
        }
        for nid in designos_doc::linked_style_nodes(doc, key) {
            if applied_text_nodes.contains(&nid) {
                continue;
            }
            let pending = pending_updates.get(&nid).copied();
            let base = match pending {
                Some(i) => match &cmds[i] {
                    Command::Update { node } => Some(node.as_ref()),
                    _ => None,
                },
                None => doc.get(nid),
            };
            let Some(base) = base else { continue };
            if !matches!(base.kind, NodeKind::Text(_)) || base.style_refs.get("text") != Some(key) {
                // Not a text, or this op linked it to another text style.
                continue;
            }
            let mut updated = base.clone();
            designos_doc::propagate_style_values_with_fields(
                &mut updated,
                doc.styles.get(key),
                style,
                key,
                style_meta.get(key).and_then(|m| m.text_paragraph_fields),
            );
            resize_hug_text(doc, &mut updated);
            match pending {
                Some(i) => cmds[i] = Command::Update { node: Box::new(updated) },
                None if doc.get(nid) != Some(&updated) => {
                    pending_updates.insert(nid, cmds.len());
                    cmds.push(Command::Update { node: Box::new(updated) });
                }
                None => {}
            }
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
    if let NodeKind::Table(tp) = &parent.kind {
        for &cid in &parent.children {
            let Some(cn) = doc.get(cid) else { continue };
            let NodeKind::TableCell(cp) = &cn.kind else { continue };
            if let (Some(r_idx), Some(c_idx)) = (tp.rows.iter().position(|t| t.id == cp.row), tp.cols.iter().position(|t| t.id == cp.col)) {
                out.insert(format!("{prefix}/r{r_idx}c{c_idx}"), json!(cid.0));
            }
        }
        return;
    }
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
    let mut container_targets: Vec<NodeId> = Vec::new();
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
    let mut batch_issues: Vec<designos_doc::SpecIssue> = Vec::new();

    let default_page = engine.resolve_default_page().ok();
    let mut bound_text = BoundTextIndex::default();

    for (op_idx, op) in ops.iter().enumerate() {
        let op_name = op.op_name();
        let op_path = format!("/ops/{op_idx}");
        if !matches!(op, ParsedOp::SetVariables(_)) {
            // Any other op can add, rebind or delete text.
            bound_text.invalidate();
        }

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
                let report = trial.apply_spec_with_measure(parent, &a.spec, &[], Some(&designos_render::RenderTextMeasure)).map_err(|se| {
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
                relayout_containers(&mut trial, &[parent]);
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
                container_targets.push(parent);
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
                    if (a.width.is_none() || a.height.is_none()) && crate::tools::text_fell_back(tp) && !measured_with_fallback.contains(&id.0) {
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
                        bindings: None,
                    },
                )
                .map_err(|e| wrap_op_err(op_idx, op_name, &op_path, e))?;

                let before_bounds = node_page_bounds(&trial, id);
                if let Some(alias) = &a.alias {
                    alias_roots.insert(alias.clone(), id);
                }
                prev_node = Some(id);
                if node_changed {
                    let affects_layout = trial.get(id).is_some_and(|old| update_affects_layout(&trial, old, &updated_node));
                    let cmd = Command::Update { node: Box::new(updated_node) };
                    if recording_live {
                        let ops = live_ops_for_command(&trial, &cmd).map_err(|e| op_failed_error(op_idx, op_name, ErrorCode::InternalError, &op_path, e))?;
                        recorded_live_ops.extend(ops);
                    }
                    cmd.clone().apply(&mut trial).map_err(|e| op_failed_error(op_idx, op_name, ErrorCode::InternalError, &op_path, e.to_string()))?;
                    if affects_layout {
                        relayout_for_node(&mut trial, id);
                        relayout_targets.push(id);
                    }
                    collected_cmds.push(cmd);
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
                if matches!(cur_node.kind, NodeKind::TableCell(_)) {
                    return Err(op_failed_error(
                        op_idx,
                        op_name,
                        ErrorCode::InvalidArguments,
                        &format!("{op_path}/node"),
                        "Move rows or columns with the batch `table` op (`move_row` / `move_col`).",
                    ));
                }
                let Some(old_parent) = cur_node.parent else {
                    return Err(op_failed_error(op_idx, op_name, ErrorCode::PageRootProtected, &op_path, "page roots cannot be moved by this tool"));
                };
                let pnode = trial
                    .get(new_parent)
                    .ok_or_else(|| op_failed_error(op_idx, op_name, ErrorCode::NodeNotFound, &op_path, format!("node {} not found", new_parent.0)))?;
                if matches!(pnode.kind, NodeKind::Table(_)) {
                    return Err(op_failed_error(
                        op_idx,
                        op_name,
                        ErrorCode::NotAContainer,
                        &format!("{op_path}/parent"),
                        "A table holds only cells. Use the batch `table` op to add or delete rows, columns or merges.",
                    ));
                }
                if matches!(pnode.kind, NodeKind::TableCell(_)) {
                    return Err(op_failed_error(
                        op_idx,
                        op_name,
                        ErrorCode::NotAContainer,
                        &format!("{op_path}/parent"),
                        "A table cell holds only its text. Use the batch `table` op (`set_cell_text`) to edit cell text.",
                    ));
                }
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
                if let Some((held_main, cycle_path)) = designos_doc::reparent_held_main_cycle_path(&trial, &[id], new_parent) {
                    let message =
                        designos_doc::cycle_refusal_message(designos_doc::CycleAction::Move, &designos_doc::cycle_node_label(&trial, held_main), &cycle_path);
                    return Err(op_failed_error(op_idx, op_name, ErrorCode::MoveCycle, &op_path, message));
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
                    relayout_containers(&mut trial, &[old_parent, new_parent]);
                    collected_cmds.push(cmd);
                    container_targets.push(old_parent);
                    container_targets.push(new_parent);
                    changed.moved += 1;
                }
                touched_records.push(TouchedRootRecord { op: op_idx, node_id: id, before_bounds });
            }
            ParsedOp::Delete(a) => {
                let id = resolve_node_ref(&trial, &alias_roots, prev_node, &a.node, op_idx, op_name, "node")?;
                check_lease_guard(&trial, engine.owned_frames.as_deref(), id, op_idx, op_name)?;
                let cur_node =
                    trial.get(id).ok_or_else(|| op_failed_error(op_idx, op_name, ErrorCode::NodeNotFound, &op_path, format!("node {} not found", id.0)))?;
                if matches!(cur_node.kind, NodeKind::TableCell(_)) {
                    return Err(op_failed_error(
                        op_idx,
                        op_name,
                        ErrorCode::InvalidArguments,
                        &format!("{op_path}/node"),
                        "Delete rows or columns with the batch `table` op (`delete_rows` / `delete_cols`).",
                    ));
                }
                let Some(former_parent) = cur_node.parent else {
                    return Err(op_failed_error(op_idx, op_name, ErrorCode::PageRootProtected, &op_path, "page roots cannot be deleted by this tool"));
                };
                if crate::engine::is_protected_instance_child(&trial, cur_node, former_parent) {
                    return Err(op_failed_error(
                        op_idx,
                        op_name,
                        ErrorCode::InstanceChild,
                        &format!("{op_path}/node"),
                        crate::engine::INSTANCE_CHILD_DELETE_MESSAGE,
                    ));
                }
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
                relayout_containers(&mut trial, &[former_parent]);
                collected_cmds.push(cmd);
                container_targets.push(former_parent);
                changed.deleted += 1;
                deleted_ids.push(id);
                touched_records.push(TouchedRootRecord { op: op_idx, node_id: former_parent, before_bounds: parent_bounds_before });
            }
            ParsedOp::SetVariables(a) => {
                check_document_lease_guard(engine.owned_frames.as_deref(), op_idx, op_name)?;
                let mut before_vars = VariablesBefore::capture(&trial);
                let (cmd_opt, count) = apply_set_variables_op(&trial, a, op_idx)?;
                if let Some(cmd) = cmd_opt {
                    if recording_live {
                        let ops = live_ops_for_command(&trial, &cmd).map_err(|e| op_failed_error(op_idx, op_name, ErrorCode::InternalError, &op_path, e))?;
                        recorded_live_ops.extend(ops);
                    }
                    cmd.clone().apply(&mut trial).map_err(|e| op_failed_error(op_idx, op_name, ErrorCode::InternalError, &op_path, e.to_string()))?;
                    let extras = bound_text.remeasure_after(&mut trial, &mut before_vars);
                    for extra in &extras {
                        if recording_live {
                            let ops =
                                live_ops_for_command(&trial, extra).map_err(|e| op_failed_error(op_idx, op_name, ErrorCode::InternalError, &op_path, e))?;
                            recorded_live_ops.extend(ops);
                        }
                        extra.clone().apply(&mut trial).map_err(|e| op_failed_error(op_idx, op_name, ErrorCode::InternalError, &op_path, e.to_string()))?;
                    }
                    let mut bound_nodes = before_vars.layout_bound_nodes(&mut trial);
                    bound_nodes.extend(updated_node_ids(&extras));
                    relayout_containers_of(&mut trial, &bound_nodes, &mut relayout_targets);
                    collected_cmds.push(cmd);
                    collected_cmds.extend(extras);
                    changed.variables += count;
                }
                let root_node = default_page.unwrap_or(NodeId(1));
                let before_bounds = node_page_bounds(&trial, root_node);
                touched_records.push(TouchedRootRecord { op: op_idx, node_id: root_node, before_bounds });
            }
            ParsedOp::SetStyle(a) => {
                let (cmds, count, first_node) = apply_set_style_op(&trial, &alias_roots, prev_node, engine.owned_frames.as_deref(), a, op_idx)?;
                let restyled = seeds_after_commands(&trial, &cmds);
                for cmd in cmds {
                    if recording_live {
                        let ops = live_ops_for_command(&trial, &cmd).map_err(|e| op_failed_error(op_idx, op_name, ErrorCode::InternalError, &op_path, e))?;
                        recorded_live_ops.extend(ops);
                    }
                    cmd.clone().apply(&mut trial).map_err(|e| op_failed_error(op_idx, op_name, ErrorCode::InternalError, &op_path, e.to_string()))?;
                    collected_cmds.push(cmd);
                }
                relayout_seeds(&mut trial, &restyled.leaves, &restyled.containers);
                for id in &restyled.leaves {
                    if !relayout_targets.contains(id) {
                        relayout_targets.push(*id);
                    }
                }
                for id in &restyled.containers {
                    if !container_targets.contains(id) {
                        container_targets.push(*id);
                    }
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
                let (generated_cmd, built_report) =
                    designos_doc::apply_spec_command_with(&mut temp_doc, parent, &spec_node, &snapshots, Some(&designos_render::RenderTextMeasure)).map_err(
                        |se| {
                            let cause = crate::tools::errors::from_spec_error(se, &format!("{op_path}/value"));
                            ToolError::batch_op_failed(op_idx, op_name, cause)
                        },
                    )?;
                // `apply_spec_command_with` validates/builds the fill and returns a command while
                // restoring its input document. Materialize that command on the temporary copy so
                // we can read the generated slot subtree, then replay every command except the
                // temporary instance insertion against the real trial document.
                generated_cmd.clone().apply(&mut temp_doc).map_err(|e| op_failed_error(op_idx, op_name, ErrorCode::InternalError, &op_path, e.to_string()))?;
                let built_id = built_report.root_node_id;
                for fid in &built_report.measured_with_fallback {
                    if !measured_with_fallback.contains(fid) {
                        measured_with_fallback.push(*fid);
                    }
                }
                for w in &built_report.warnings {
                    if w.code != "TEXT_MEASURED_WITH_FALLBACK_FONT" || !batch_warnings.iter().any(|bw| bw.code == "TEXT_MEASURED_WITH_FALLBACK_FONT") {
                        batch_warnings.push(w.clone());
                    }
                }

                let target_slot = resolve_slot_container(&trial, id, &a.slot).ok_or_else(|| {
                    op_failed_error(
                        op_idx,
                        op_name,
                        ErrorCode::SlotContainerNotFound,
                        &format!("{op_path}/slot"),
                        format!("slot '{}' was not found on instance {}", a.slot, id.0),
                    )
                })?;
                let built_slot = resolve_slot_container(&temp_doc, built_id, &a.slot).ok_or_else(|| {
                    op_failed_error(
                        op_idx,
                        op_name,
                        ErrorCode::InternalError,
                        &format!("{op_path}/slot"),
                        format!("validated slot '{}' disappeared from generated instance", a.slot),
                    )
                })?;

                // The spec builder reports slot findings against the temporary instance. The
                // generated slot roots keep their IDs when copied into the real instance, but
                // the temporary instance and slot containers do not. Remap those two owners so
                // the batch response points at nodes that exist after the operation.
                for mut issue in built_report.issues.clone() {
                    if issue.node_id == built_id.0 {
                        issue.node_id = id.0;
                    } else if issue.node_id == built_slot.0 {
                        issue.node_id = target_slot.0;
                    }
                    batch_issues.push(issue);
                }

                let mut generated_parts = Vec::new();
                flatten_batch_command(generated_cmd, &mut generated_parts);
                for part in generated_parts {
                    match &part {
                        Command::Insert { nodes, .. } if nodes.first().is_some_and(|node| node.id == built_id) => {}
                        _ => {
                            if recording_live {
                                let ops =
                                    live_ops_for_command(&trial, &part).map_err(|e| op_failed_error(op_idx, op_name, ErrorCode::InternalError, &op_path, e))?;
                                recorded_live_ops.extend(ops);
                            }
                            part.clone().apply(&mut trial).map_err(|e| op_failed_error(op_idx, op_name, ErrorCode::InternalError, &op_path, e.to_string()))?;
                            collected_cmds.push(part);
                        }
                    }
                }
                let built_slot_children = temp_doc.get(built_slot).map(|node| node.children.clone()).unwrap_or_default();
                let mut replacement_roots: Vec<(NodeId, Vec<designos_doc::Node>)> = Vec::with_capacity(built_slot_children.len());
                for root in built_slot_children {
                    let mut nodes = Vec::new();
                    temp_doc.walk(root, |node, _| nodes.push(node.clone()));
                    if let Some(first) = nodes.first_mut() {
                        first.parent = Some(target_slot);
                    }
                    replacement_roots.push((root, nodes));
                }

                match &a.value {
                    designos_doc::SlotFillSpec::Disabled(_) => {
                        let mut slot = trial.get(target_slot).cloned().ok_or_else(|| {
                            op_failed_error(op_idx, op_name, ErrorCode::NodeNotFound, &op_path, format!("slot node {} not found", target_slot.0))
                        })?;
                        slot.visible = false;
                        slot.component.overrides.insert(designos_doc::OverrideField::Visible);
                        if slot != *trial.get(target_slot).unwrap() {
                            let cmd = Command::Update { node: Box::new(slot) };
                            if recording_live {
                                let ops =
                                    live_ops_for_command(&trial, &cmd).map_err(|e| op_failed_error(op_idx, op_name, ErrorCode::InternalError, &op_path, e))?;
                                recorded_live_ops.extend(ops);
                            }
                            cmd.clone().apply(&mut trial).map_err(|e| op_failed_error(op_idx, op_name, ErrorCode::InternalError, &op_path, e.to_string()))?;
                            collected_cmds.push(cmd);
                            changed.updated += 1;
                        }
                    }
                    designos_doc::SlotFillSpec::Fill(_) => {
                        let old_children = trial.get(target_slot).map(|node| node.children.clone()).unwrap_or_default();
                        for old_child in old_children {
                            let cmd = Command::Remove { id: old_child };
                            if recording_live {
                                let ops =
                                    live_ops_for_command(&trial, &cmd).map_err(|e| op_failed_error(op_idx, op_name, ErrorCode::InternalError, &op_path, e))?;
                                recorded_live_ops.extend(ops);
                            }
                            cmd.clone().apply(&mut trial).map_err(|e| op_failed_error(op_idx, op_name, ErrorCode::InternalError, &op_path, e.to_string()))?;
                            collected_cmds.push(cmd);
                            deleted_ids.push(old_child);
                            changed.deleted += 1;
                        }

                        let mut slot = trial.get(target_slot).cloned().ok_or_else(|| {
                            op_failed_error(op_idx, op_name, ErrorCode::NodeNotFound, &op_path, format!("slot node {} not found", target_slot.0))
                        })?;
                        slot.component.overrides.insert(designos_doc::OverrideField::SlotContent);
                        // A fill undoes a hide set on this instance (`fill: false` or a Visible
                        // override). A visibility that the main component or a Boolean property
                        // gives the slot is kept.
                        let hidden_here = !slot.visible && slot.component.overrides.contains(&designos_doc::OverrideField::Visible);
                        if let Some(generated_slot) = temp_doc.get(built_slot).filter(|_| hidden_here) {
                            slot.visible = generated_slot.visible;
                            if generated_slot.component.overrides.contains(&designos_doc::OverrideField::Visible) {
                                slot.component.overrides.insert(designos_doc::OverrideField::Visible);
                            } else {
                                slot.component.overrides.remove(&designos_doc::OverrideField::Visible);
                            }
                        }
                        if slot != *trial.get(target_slot).unwrap() {
                            let slot_cmd = Command::Update { node: Box::new(slot) };
                            if recording_live {
                                let ops = live_ops_for_command(&trial, &slot_cmd)
                                    .map_err(|e| op_failed_error(op_idx, op_name, ErrorCode::InternalError, &op_path, e))?;
                                recorded_live_ops.extend(ops);
                            }
                            slot_cmd
                                .clone()
                                .apply(&mut trial)
                                .map_err(|e| op_failed_error(op_idx, op_name, ErrorCode::InternalError, &op_path, e.to_string()))?;
                            collected_cmds.push(slot_cmd);
                            changed.updated += 1;
                        }

                        for (index, (root, nodes)) in replacement_roots.into_iter().enumerate() {
                            let cmd = Command::Insert { parent: target_slot, index, nodes };
                            if recording_live {
                                let ops =
                                    live_ops_for_command(&trial, &cmd).map_err(|e| op_failed_error(op_idx, op_name, ErrorCode::InternalError, &op_path, e))?;
                                recorded_live_ops.extend(ops);
                            }
                            cmd.clone().apply(&mut trial).map_err(|e| op_failed_error(op_idx, op_name, ErrorCode::InternalError, &op_path, e.to_string()))?;
                            let mut subtree_ids = BTreeSet::new();
                            trial.walk(root, |node, _| {
                                subtree_ids.insert(node.id);
                            });
                            let count = subtree_ids.len();
                            if created_count + count > MAX_BATCH_CREATED_NODES {
                                return Err(batch_error(
                                    ErrorCode::BatchTooManyOps,
                                    Some(op_idx),
                                    Some(op_name),
                                    &op_path,
                                    format!("batch creates over {MAX_BATCH_CREATED_NODES} nodes"),
                                ));
                            }
                            collected_cmds.push(cmd);
                            created_root_ids.push(root.0);
                            created_count += count;
                            changed.inserted += count;
                            all_created_ids.extend(subtree_ids.clone());
                            created_roots.push(CreatedRootRecord { alias: None, root_id: root, created_ids: subtree_ids });
                        }

                        let generated_instance = temp_doc
                            .get(built_id)
                            .ok_or_else(|| op_failed_error(op_idx, op_name, ErrorCode::InternalError, &op_path, "generated instance disappeared"))?;
                        let key = slot_value_key(generated_instance, &a.slot).ok_or_else(|| {
                            op_failed_error(op_idx, op_name, ErrorCode::InternalError, &op_path, format!("generated slot value '{}' missing", a.slot))
                        })?;
                        let value = generated_instance
                            .component
                            .values
                            .get(&key)
                            .cloned()
                            .ok_or_else(|| op_failed_error(op_idx, op_name, ErrorCode::InternalError, &op_path, "generated slot value missing"))?;
                        let mut instance = trial
                            .get(id)
                            .cloned()
                            .ok_or_else(|| op_failed_error(op_idx, op_name, ErrorCode::NodeNotFound, &op_path, format!("instance node {} not found", id.0)))?;
                        instance.component.values.insert(key, value);
                        if instance != *trial.get(id).unwrap() {
                            let cmd = Command::Update { node: Box::new(instance) };
                            if recording_live {
                                let ops =
                                    live_ops_for_command(&trial, &cmd).map_err(|e| op_failed_error(op_idx, op_name, ErrorCode::InternalError, &op_path, e))?;
                                recorded_live_ops.extend(ops);
                            }
                            cmd.clone().apply(&mut trial).map_err(|e| op_failed_error(op_idx, op_name, ErrorCode::InternalError, &op_path, e.to_string()))?;
                            collected_cmds.push(cmd);
                            changed.updated += 1;
                        }
                    }
                }

                if let Some(alias) = &a.alias {
                    alias_roots.insert(alias.clone(), id);
                }
                prev_node = Some(id);
                let before_bounds = node_page_bounds(&trial, id);
                touched_records.push(TouchedRootRecord { op: op_idx, node_id: id, before_bounds });
                relayout_containers(&mut trial, &[target_slot, id]);
                container_targets.push(target_slot);
                container_targets.push(id);
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
            ParsedOp::Connect(a) => {
                trial.flush_pending_relayout();
                if let Some(target_ref) = &a.node {
                    let id = resolve_node_ref(&trial, &alias_roots, prev_node, target_ref, op_idx, op_name, "node")?;
                    check_lease_guard(&trial, engine.owned_frames.as_deref(), id, op_idx, op_name)?;
                    let cur_node = trial
                        .get(id)
                        .ok_or_else(|| op_failed_error(op_idx, op_name, ErrorCode::NodeNotFound, &op_path, format!("node {} not found", id.0)))?
                        .clone();
                    let NodeKind::Connector(mut props) = cur_node.kind.clone() else {
                        return Err(op_failed_error(
                            op_idx,
                            op_name,
                            ErrorCode::UnsupportedNodeKind,
                            &format!("{op_path}/node"),
                            format!("UNSUPPORTED_NODE_KIND: node {} is not a connector", id.0),
                        ));
                    };

                    // An end the op does not name keeps what the connector stores. Only an end passed in is an
                    // attach, so only that end has to be connectable: an existing attachment outlives its node
                    // being hidden, locked or deleted.
                    let from_change = match &a.from {
                        None => ConnectEndChange::Keep,
                        Some(ConnectEndInput::Node(r)) => {
                            let fid = resolve_node_ref(&trial, &alias_roots, prev_node, r, op_idx, op_name, "from")?;
                            if !designos_doc::connectable_node(&trial, fid) {
                                return Err(op_failed_error(
                                    op_idx,
                                    op_name,
                                    ErrorCode::UnsupportedNodeKind,
                                    &format!("{op_path}/from"),
                                    format!("UNSUPPORTED_NODE_KIND: node {} cannot be a connector endpoint", fid.0),
                                ));
                            }
                            ConnectEndChange::Attach(fid)
                        }
                        Some(ConnectEndInput::Point(p)) => ConnectEndChange::Free(designos_doc::Vec2::new(p.point[0], p.point[1])),
                    };

                    let to_change = match &a.to {
                        None => ConnectEndChange::Keep,
                        Some(ConnectEndInput::Node(r)) => {
                            let tid = resolve_node_ref(&trial, &alias_roots, prev_node, r, op_idx, op_name, "to")?;
                            if !designos_doc::connectable_node(&trial, tid) {
                                return Err(op_failed_error(
                                    op_idx,
                                    op_name,
                                    ErrorCode::UnsupportedNodeKind,
                                    &format!("{op_path}/to"),
                                    format!("UNSUPPORTED_NODE_KIND: node {} cannot be a connector endpoint", tid.0),
                                ));
                            }
                            ConnectEndChange::Attach(tid)
                        }
                        Some(ConnectEndInput::Point(p)) => ConnectEndChange::Free(designos_doc::Vec2::new(p.point[0], p.point[1])),
                    };
                    let keeps_both_ends = matches!((&from_change, &to_change), (ConnectEndChange::Keep, ConnectEndChange::Keep));
                    let from_attached = from_change.attached_node(&trial, props.start);
                    let to_attached = to_change.attached_node(&trial, props.end);

                    if let (Some(fid), Some(tid)) = (from_attached, to_attached) {
                        if fid == tid {
                            return Err(op_failed_error(
                                op_idx,
                                op_name,
                                ErrorCode::UnsupportedNodeKind,
                                &op_path,
                                format!("UNSUPPORTED_NODE_KIND: connector cannot connect node {} to itself", fid.0),
                            ));
                        }
                    }

                    let from_page = from_attached.and_then(|fid| page_of_node(&trial, fid));
                    let to_page = to_attached.and_then(|tid| page_of_node(&trial, tid));
                    if let (Some(pa), Some(pb), false) = (from_page, to_page, keeps_both_ends) {
                        if pa != pb {
                            return Err(op_failed_error(
                                op_idx,
                                op_name,
                                ErrorCode::NodesOnDifferentPages,
                                &op_path,
                                format!(
                                    "CONNECTOR_ENDS_ON_DIFFERENT_PAGES: connector ends {} (page {}) and {} (page {}) are on different pages",
                                    from_attached.unwrap().0,
                                    pa.0,
                                    to_attached.unwrap().0,
                                    pb.0
                                ),
                            ));
                        }
                    }

                    let page = from_page
                        .or(to_page)
                        .or_else(|| page_of_node(&trial, id))
                        .or(default_page)
                        .ok_or_else(|| op_failed_error(op_idx, op_name, ErrorCode::PageNotFound, &op_path, "Document has no pages"))?;
                    // The home is worked out again only when the op attaches or detaches an end.
                    let home = match (keeps_both_ends, cur_node.parent) {
                        (true, Some(parent)) => parent,
                        _ => designos_doc::connector_home(&trial, from_attached, to_attached, page),
                    };
                    check_lease_guard(&trial, engine.owned_frames.as_deref(), home, op_idx, op_name)?;
                    if let ConnectEndChange::Attach(fid) = from_change {
                        check_lease_guard(&trial, engine.owned_frames.as_deref(), fid, op_idx, op_name)?;
                    }
                    if let ConnectEndChange::Attach(tid) = to_change {
                        check_lease_guard(&trial, engine.owned_frames.as_deref(), tid, op_idx, op_name)?;
                    }

                    let home_inv = trial.world_transform(home).invert().unwrap_or(designos_doc::Affine::IDENTITY);
                    let old_world = cur_node.parent.map(|p| trial.world_transform(p)).unwrap_or(designos_doc::Affine::IDENTITY);
                    let rehomed = cur_node.parent != Some(home);
                    let from_mag = match &a.from_magnet {
                        Some(s) => parse_magnet(s, op_idx, "from_magnet")?,
                        None => props.start.magnet,
                    };
                    let to_mag = match &a.to_magnet {
                        Some(s) => parse_magnet(s, op_idx, "to_magnet")?,
                        None => props.end.magnet,
                    };
                    let build_end = |change: &ConnectEndChange, stored: designos_doc::ConnectorEnd, magnet: designos_doc::Magnet| match change {
                        ConnectEndChange::Attach(node) => designos_doc::ConnectorEnd { node: Some(*node), magnet, point: stored.point },
                        ConnectEndChange::Free(page_point) => designos_doc::ConnectorEnd { node: None, magnet, point: home_inv.apply(*page_point) },
                        ConnectEndChange::Keep => {
                            // A kept end that still rests on a layer is re-derived from it; a free or dangling one
                            // holds its last point, which changes space only when the connector changes container.
                            let rests_on_layer = stored.node.is_some_and(|id| trial.get(id).is_some());
                            let point = if rests_on_layer || !rehomed { stored.point } else { home_inv.apply(old_world.apply(stored.point)) };
                            designos_doc::ConnectorEnd { magnet, point, ..stored }
                        }
                    };
                    props.start = build_end(&from_change, props.start, from_mag);
                    props.end = build_end(&to_change, props.end, to_mag);
                    if let Some(l) = &a.line {
                        props.line = parse_connector_line(l, op_idx)?;
                    }
                    if let Some(sc) = &a.start_cap {
                        props.start_cap = parse_connector_cap(sc, op_idx, "start_cap")?;
                    }
                    if let Some(ec) = &a.end_cap {
                        props.end_cap = parse_connector_cap(ec, op_idx, "end_cap")?;
                    }
                    if let Some(lbl) = &a.label {
                        if lbl.is_empty() {
                            props.label = None;
                        } else {
                            let mut tp = props
                                .label
                                .clone()
                                .unwrap_or_else(|| designos_doc::board_text(14.0, designos_doc::TextAlignH::Center, designos_doc::TextAlignV::Center));
                            tp.content = lbl.clone();
                            props.label = Some(tp);
                        }
                    }

                    let before_bounds = node_page_bounds(&trial, id);
                    if cur_node.parent != Some(home) {
                        let old_parent = cur_node.parent;
                        let target_idx = trial.get(home).map(|p| p.children.iter().filter(|&&c| c != id).count()).unwrap_or(0);
                        let reparent_cmd = Command::Reparent { id, parent: home, index: target_idx };
                        if recording_live {
                            let ops = live_ops_for_command(&trial, &reparent_cmd)
                                .map_err(|e| op_failed_error(op_idx, op_name, ErrorCode::InternalError, &op_path, e))?;
                            recorded_live_ops.extend(ops);
                        }
                        reparent_cmd
                            .clone()
                            .apply(&mut trial)
                            .map_err(|e| op_failed_error(op_idx, op_name, ErrorCode::InternalError, &op_path, e.to_string()))?;
                        // Only a connector that sat in the old parent's flow changes it; the update below
                        // re-solves `home` when the connector leaves its flow.
                        if let (Some(op), false) = (old_parent, cur_node.layout_child.absolute) {
                            relayout_for_node(&mut trial, op);
                            relayout_targets.push(op);
                        }
                        collected_cmds.push(reparent_cmd);
                        changed.moved += 1;
                    }

                    let before_update = trial.get(id).unwrap().clone();
                    let mut updated_node = before_update.clone();
                    if let Some(name) = &a.name {
                        updated_node.name = name.clone();
                    }
                    updated_node.layout_child.absolute = true;
                    updated_node.kind = NodeKind::Connector(props);
                    if let Some(slot) = trial.get_mut(id) {
                        *slot = updated_node.clone();
                    }
                    if let Some(geom) = designos_doc::connector_geometry(&trial, id) {
                        geom.apply(&mut updated_node);
                    }
                    if let Some(slot) = trial.get_mut(id) {
                        *slot = before_update.clone();
                    }
                    if updated_node != before_update {
                        let affects_layout = update_affects_layout(&trial, &before_update, &updated_node);
                        let update_cmd = Command::Update { node: Box::new(updated_node) };
                        if recording_live {
                            let ops = live_ops_for_command(&trial, &update_cmd)
                                .map_err(|e| op_failed_error(op_idx, op_name, ErrorCode::InternalError, &op_path, e))?;
                            recorded_live_ops.extend(ops);
                        }
                        update_cmd
                            .clone()
                            .apply(&mut trial)
                            .map_err(|e| op_failed_error(op_idx, op_name, ErrorCode::InternalError, &op_path, e.to_string()))?;
                        if affects_layout {
                            relayout_for_node(&mut trial, home);
                            relayout_targets.push(home);
                        }
                        collected_cmds.push(update_cmd);
                        changed.updated += 1;
                    }
                    if let Some(alias) = &a.alias {
                        alias_roots.insert(alias.clone(), id);
                    }
                    prev_node = Some(id);
                    touched_records.push(TouchedRootRecord { op: op_idx, node_id: id, before_bounds });
                } else {
                    if created_count + 1 > MAX_BATCH_CREATED_NODES {
                        return Err(batch_error(
                            ErrorCode::BatchTooManyOps,
                            Some(op_idx),
                            Some(op_name),
                            &op_path,
                            format!("batch creates over {MAX_BATCH_CREATED_NODES} nodes"),
                        ));
                    }
                    let from_in = a
                        .from
                        .as_ref()
                        .ok_or_else(|| op_failed_error(op_idx, op_name, ErrorCode::MissingField, &format!("{op_path}/from"), "connect requires 'from'"))?;
                    let to_in =
                        a.to.as_ref()
                            .ok_or_else(|| op_failed_error(op_idx, op_name, ErrorCode::MissingField, &format!("{op_path}/to"), "connect requires 'to'"))?;

                    let (from_attached, from_free_page) = match from_in {
                        ConnectEndInput::Node(r) => {
                            let fid = resolve_node_ref(&trial, &alias_roots, prev_node, r, op_idx, op_name, "from")?;
                            if !designos_doc::connectable_node(&trial, fid) {
                                return Err(op_failed_error(
                                    op_idx,
                                    op_name,
                                    ErrorCode::UnsupportedNodeKind,
                                    &format!("{op_path}/from"),
                                    format!("UNSUPPORTED_NODE_KIND: node {} cannot be a connector endpoint", fid.0),
                                ));
                            }
                            (Some(fid), None)
                        }
                        ConnectEndInput::Point(p) => (None, Some(designos_doc::Vec2::new(p.point[0], p.point[1]))),
                    };

                    let (to_attached, to_free_page) = match to_in {
                        ConnectEndInput::Node(r) => {
                            let tid = resolve_node_ref(&trial, &alias_roots, prev_node, r, op_idx, op_name, "to")?;
                            if !designos_doc::connectable_node(&trial, tid) {
                                return Err(op_failed_error(
                                    op_idx,
                                    op_name,
                                    ErrorCode::UnsupportedNodeKind,
                                    &format!("{op_path}/to"),
                                    format!("UNSUPPORTED_NODE_KIND: node {} cannot be a connector endpoint", tid.0),
                                ));
                            }
                            (Some(tid), None)
                        }
                        ConnectEndInput::Point(p) => (None, Some(designos_doc::Vec2::new(p.point[0], p.point[1]))),
                    };

                    if let (Some(fid), Some(tid)) = (from_attached, to_attached) {
                        if fid == tid {
                            return Err(op_failed_error(
                                op_idx,
                                op_name,
                                ErrorCode::UnsupportedNodeKind,
                                &op_path,
                                format!("UNSUPPORTED_NODE_KIND: connector cannot connect node {} to itself", fid.0),
                            ));
                        }
                    }

                    let from_page = from_attached.and_then(|fid| page_of_node(&trial, fid));
                    let to_page = to_attached.and_then(|tid| page_of_node(&trial, tid));
                    if let (Some(pa), Some(pb)) = (from_page, to_page) {
                        if pa != pb {
                            return Err(op_failed_error(
                                op_idx,
                                op_name,
                                ErrorCode::NodesOnDifferentPages,
                                &op_path,
                                format!(
                                    "CONNECTOR_ENDS_ON_DIFFERENT_PAGES: connector ends {} (page {}) and {} (page {}) are on different pages",
                                    from_attached.unwrap().0,
                                    pa.0,
                                    to_attached.unwrap().0,
                                    pb.0
                                ),
                            ));
                        }
                    }

                    let page = from_page
                        .or(to_page)
                        .or(default_page)
                        .ok_or_else(|| op_failed_error(op_idx, op_name, ErrorCode::PageNotFound, &op_path, "Document has no pages"))?;
                    let home = designos_doc::connector_home(&trial, from_attached, to_attached, page);
                    check_lease_guard(&trial, engine.owned_frames.as_deref(), home, op_idx, op_name)?;
                    if let Some(fid) = from_attached {
                        check_lease_guard(&trial, engine.owned_frames.as_deref(), fid, op_idx, op_name)?;
                    }
                    if let Some(tid) = to_attached {
                        check_lease_guard(&trial, engine.owned_frames.as_deref(), tid, op_idx, op_name)?;
                    }

                    let home_inv = trial.world_transform(home).invert().unwrap_or(designos_doc::Affine::IDENTITY);
                    let from_mag = match &a.from_magnet {
                        Some(s) => parse_magnet(s, op_idx, "from_magnet")?,
                        None => designos_doc::Magnet::Auto,
                    };
                    let to_mag = match &a.to_magnet {
                        Some(s) => parse_magnet(s, op_idx, "to_magnet")?,
                        None => designos_doc::Magnet::Auto,
                    };
                    let mut props = designos_doc::ConnectorProps {
                        start: match from_attached {
                            Some(fid) => designos_doc::ConnectorEnd::attached(fid, from_mag),
                            None => designos_doc::ConnectorEnd {
                                node: None,
                                magnet: from_mag,
                                point: home_inv.apply(from_free_page.unwrap_or(designos_doc::Vec2::ZERO)),
                            },
                        },
                        end: match to_attached {
                            Some(tid) => designos_doc::ConnectorEnd::attached(tid, to_mag),
                            None => designos_doc::ConnectorEnd {
                                node: None,
                                magnet: to_mag,
                                point: home_inv.apply(to_free_page.unwrap_or(designos_doc::Vec2::ZERO)),
                            },
                        },
                        ..Default::default()
                    };
                    if let Some(l) = &a.line {
                        props.line = parse_connector_line(l, op_idx)?;
                    }
                    if let Some(sc) = &a.start_cap {
                        props.start_cap = parse_connector_cap(sc, op_idx, "start_cap")?;
                    }
                    if let Some(ec) = &a.end_cap {
                        props.end_cap = parse_connector_cap(ec, op_idx, "end_cap")?;
                    }
                    if let Some(lbl) = &a.label {
                        if !lbl.is_empty() {
                            let mut tp = designos_doc::board_text(14.0, designos_doc::TextAlignH::Center, designos_doc::TextAlignV::Center);
                            tp.content = lbl.clone();
                            props.label = Some(tp);
                        }
                    }

                    let id = trial.alloc_id();
                    let mut node = designos_doc::Node::connector(id, props);
                    node.stroke.paints = vec![designos_doc::Paint::solid(designos_doc::default_connector_ink(&trial, home))];
                    if let Some(name) = &a.name {
                        node.name = name.clone();
                    }
                    node.layout_child.absolute = true;
                    node.parent = Some(home);
                    trial.nodes.insert(id, node.clone());
                    if let Some(geom) = designos_doc::connector_geometry(&trial, id) {
                        geom.apply(&mut node);
                    }
                    trial.nodes.remove(&id);

                    let target_idx = trial.get(home).map(|p| p.children.len()).unwrap_or(0);
                    let cmd = Command::Insert { parent: home, index: target_idx, nodes: vec![node] };
                    if recording_live {
                        let ops = live_ops_for_command(&trial, &cmd).map_err(|e| op_failed_error(op_idx, op_name, ErrorCode::InternalError, &op_path, e))?;
                        recorded_live_ops.extend(ops);
                    }
                    cmd.clone().apply(&mut trial).map_err(|e| op_failed_error(op_idx, op_name, ErrorCode::InternalError, &op_path, e.to_string()))?;
                    // The connector is absolutely positioned: it takes no place in `home`'s flow, so `home` is not re-solved.

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
                    let before_bounds = node_page_bounds(&trial, id);
                    touched_records.push(TouchedRootRecord { op: op_idx, node_id: id, before_bounds });
                }
            }
            ParsedOp::Table(a) => {
                if a.action == "create" {
                    let parent = match &a.parent {
                        Some(p) => resolve_node_ref(&trial, &alias_roots, prev_node, p, op_idx, op_name, "parent")?,
                        None => default_page.ok_or_else(|| op_failed_error(op_idx, op_name, ErrorCode::PageNotFound, &op_path, "Document has no pages"))?,
                    };
                    check_lease_guard(&trial, engine.owned_frames.as_deref(), parent, op_idx, op_name)?;
                    let needed = 1 + (a.rows.unwrap_or(3) as usize) * (a.cols.unwrap_or(3) as usize);
                    if created_count + needed > MAX_BATCH_CREATED_NODES {
                        return Err(batch_error(
                            ErrorCode::BatchTooManyOps,
                            Some(op_idx),
                            Some(op_name),
                            &op_path,
                            format!("batch creates over {MAX_BATCH_CREATED_NODES} nodes"),
                        ));
                    }
                    let (table_id, cmd) = crate::tools::batch_table::execute_create(&trial, parent, a, op_idx)?;
                    let mut set = BTreeSet::new();
                    if let Command::Insert { nodes, .. } = &cmd {
                        for n in nodes {
                            set.insert(n.id);
                        }
                    }
                    if recording_live {
                        let ops = live_ops_for_command(&trial, &cmd).map_err(|e| op_failed_error(op_idx, op_name, ErrorCode::InternalError, &op_path, e))?;
                        recorded_live_ops.extend(ops);
                    }
                    cmd.clone().apply(&mut trial).map_err(|e| op_failed_error(op_idx, op_name, ErrorCode::InternalError, &op_path, e.to_string()))?;
                    relayout_for_node(&mut trial, table_id);

                    let count = set.len();
                    created_root_ids.push(table_id.0);
                    created_count += count;
                    changed.inserted += count;
                    all_created_ids.extend(set.iter().copied());
                    if let Some(alias) = &a.alias {
                        alias_roots.insert(alias.clone(), table_id);
                    }
                    prev_node = Some(table_id);
                    created_roots.push(CreatedRootRecord { alias: a.alias.clone(), root_id: table_id, created_ids: set });
                    collected_cmds.push(cmd);
                    relayout_targets.push(table_id);
                    let before_bounds = node_page_bounds(&trial, table_id);
                    touched_records.push(TouchedRootRecord { op: op_idx, node_id: table_id, before_bounds });
                } else {
                    let node_ref = a
                        .node
                        .as_ref()
                        .ok_or_else(|| op_failed_error(op_idx, op_name, ErrorCode::MissingField, &format!("{op_path}/node"), "table action requires 'node'"))?;
                    let table_id = resolve_node_ref(&trial, &alias_roots, prev_node, node_ref, op_idx, op_name, "node")?;
                    check_lease_guard(&trial, engine.owned_frames.as_deref(), table_id, op_idx, op_name)?;
                    let before_bounds = node_page_bounds(&trial, table_id);
                    if let Some(cmd) = crate::tools::batch_table::execute_mutation(&trial, table_id, a, op_idx)? {
                        let mut sub_cmds = Vec::new();
                        flatten_batch_command(cmd, &mut sub_cmds);
                        let added_nodes: usize = sub_cmds
                            .iter()
                            .map(|sc| match sc {
                                Command::Insert { nodes, .. } => nodes.len(),
                                _ => 0,
                            })
                            .sum();
                        if created_count + added_nodes > MAX_BATCH_CREATED_NODES {
                            return Err(batch_error(
                                ErrorCode::BatchTooManyOps,
                                Some(op_idx),
                                Some(op_name),
                                &op_path,
                                format!("batch creates over {MAX_BATCH_CREATED_NODES} nodes"),
                            ));
                        }
                        for sc in sub_cmds {
                            if recording_live {
                                let ops =
                                    live_ops_for_command(&trial, &sc).map_err(|e| op_failed_error(op_idx, op_name, ErrorCode::InternalError, &op_path, e))?;
                                recorded_live_ops.extend(ops);
                            }
                            match &sc {
                                Command::Insert { nodes, .. } => {
                                    let count = nodes.len();
                                    created_count += count;
                                    changed.inserted += count;
                                    for n in nodes {
                                        all_created_ids.insert(n.id);
                                        if let Some(cr) = created_roots.iter_mut().find(|cr| cr.root_id == table_id) {
                                            cr.created_ids.insert(n.id);
                                        }
                                    }
                                }
                                Command::Remove { id } => {
                                    deleted_ids.push(*id);
                                    changed.deleted += 1;
                                }
                                Command::Update { .. } => {
                                    changed.updated += 1;
                                }
                                _ => {}
                            }
                            sc.clone().apply(&mut trial).map_err(|e| op_failed_error(op_idx, op_name, ErrorCode::InternalError, &op_path, e.to_string()))?;
                            collected_cmds.push(sc);
                        }
                        relayout_for_node(&mut trial, table_id);
                        relayout_targets.push(table_id);
                    }
                    if let Some(alias) = &a.alias {
                        alias_roots.insert(alias.clone(), table_id);
                    }
                    prev_node = Some(table_id);
                    touched_records.push(TouchedRootRecord { op: op_idx, node_id: table_id, before_bounds });
                }
            }
        }
    }

    // Commit-point invariant check across every table touched by this batch.
    let mut touched_tables: Vec<(usize, NodeId)> = Vec::new();
    for rec in &touched_records {
        let table_id = match trial.get(rec.node_id).map(|n| (&n.kind, n.parent)) {
            Some((NodeKind::Table(_), _)) => Some(rec.node_id),
            Some((NodeKind::TableCell(_), Some(parent_id))) if matches!(trial.get(parent_id).map(|p| &p.kind), Some(NodeKind::Table(_))) => Some(parent_id),
            _ => None,
        };
        if let Some(tid) = table_id {
            if !touched_tables.iter().any(|&(_, id)| id == tid) {
                touched_tables.push((rec.op, tid));
            }
        }
    }
    for (op_idx, tid) in touched_tables {
        designos_doc::check_table_positions(&trial, tid)
            .map_err(|e| op_failed_error(op_idx, "table", ErrorCode::InternalError, &format!("/ops/{op_idx}"), e))?;
    }

    // The commit below is a history step, which re-applies the component properties bound to variables these ops moved;
    // the trial gets the same, so the reply and the checks describe the committed document.
    designos_doc::settle_bound_props_after(&engine.doc, &mut trial, &collected_cmds);
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
            relayout_seeds(&mut engine.doc, &relayout_targets, &container_targets);
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
        &batch_issues,
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
    fn set_slot_mutates_existing_instance_and_undoes_as_one_batch_step() {
        let mut engine = Engine { doc: designos_doc::fixtures::card_with_slots(), ..Engine::default() };
        let page = engine.doc.pages[0];
        let created =
            engine.call("designos_apply_spec", json!({"parent_id": page.0, "spec": {"kind": "instance", "name": "Card1", "component": "Card"}})).unwrap();
        let instance_id = created["root_node_id"].as_u64().unwrap();
        let mut before_json: Value = serde_json::from_str(&engine.doc.to_json().unwrap()).unwrap();
        let before_depth = engine.history.undo_depth();

        let reply = engine
            .call(
                "designos_apply_batch",
                json!({
                    "ops": [{
                        "op": "set_slot",
                        "node": instance_id,
                        "slot": "body",
                        "fill": [{"kind": "frame", "name": "Body"}]
                    }]
                }),
            )
            .expect("set_slot should commit");

        let body_id = designos_doc::resolve_override_path(&engine.doc, designos_doc::NodeId(instance_id), "body").unwrap();
        let body = engine.doc.get(body_id).unwrap();
        assert_eq!(body.children.len(), 1, "set_slot must populate the existing instance slot");
        let child_id = body.children[0];
        assert_eq!(engine.doc.get(child_id).unwrap().name, "Body");
        assert!(reply["created"].as_array().unwrap().iter().any(|id| id.as_u64() == Some(child_id.0)));
        assert_eq!(engine.history.undo_depth(), before_depth + 1, "set_slot must be one undo step");

        engine.call("designos_undo", json!({})).unwrap();
        let mut after_json: Value = serde_json::from_str(&engine.doc.to_json().unwrap()).unwrap();
        before_json.as_object_mut().unwrap().remove("next_id");
        after_json.as_object_mut().unwrap().remove("next_id");
        assert_eq!(after_json, before_json, "undo must restore the pre-slot document content exactly");
    }

    #[test]
    fn set_slot_prefers_bound_slot_over_same_named_ordinary_child() {
        let mut engine = Engine { doc: designos_doc::fixtures::card_with_slots(), ..Engine::default() };
        let page = engine.doc.pages[0];
        let created =
            engine.call("designos_apply_spec", json!({"parent_id": page.0, "spec": {"kind": "instance", "name": "Card1", "component": "Card"}})).unwrap();
        let instance_id = NodeId(created["root_node_id"].as_u64().unwrap());
        let body_slot_id = designos_doc::resolve_override_path(&engine.doc, instance_id, "body").unwrap();

        // A component override can rename the mirrored layer while its SlotContent binding
        // remains canonical. An ordinary child may then reuse the old display name.
        engine.doc.get_mut(body_slot_id).unwrap().name = "Body Container".into();
        let distractor_id = engine.doc.alloc_id();
        let mut distractor = designos_doc::Node::frame(distractor_id, designos_doc::Rect::new(0.0, 0.0, 20.0, 20.0));
        distractor.name = "body".into();
        engine.doc.insert(instance_id, distractor).unwrap();

        engine
            .call(
                "designos_apply_batch",
                json!({"ops": [{"op": "set_slot", "node": instance_id.0, "slot": "body", "fill": [{"kind": "frame", "name": "FilledBody"}]}]}),
            )
            .expect("set_slot should resolve the bound slot");

        assert_eq!(engine.doc.get(body_slot_id).unwrap().children.len(), 1, "bound slot must receive the fill");
        assert!(engine.doc.get(distractor_id).unwrap().children.is_empty(), "ordinary same-named child must remain untouched");
    }

    #[test]
    fn set_slot_forwards_fill_issues_with_live_node_ids() {
        let mut engine = Engine { doc: designos_doc::fixtures::card_with_slots(), ..Engine::default() };
        let page = engine.doc.pages[0];
        let created =
            engine.call("designos_apply_spec", json!({"parent_id": page.0, "spec": {"kind": "instance", "name": "Card1", "component": "Card"}})).unwrap();
        let instance_id = created["root_node_id"].as_u64().unwrap();

        let reply = engine
            .call(
                "designos_apply_batch",
                json!({"ops": [{"op": "set_slot", "node": instance_id, "slot": "body", "fill": [{"kind": "frame", "name": "AdhocBody"}]}]}),
            )
            .expect("set_slot should commit");
        let body_id = designos_doc::resolve_override_path(&engine.doc, NodeId(instance_id), "body").unwrap();
        let child_id = engine.doc.get(body_id).unwrap().children[0];
        let issues = reply["issues"].as_array().expect("issues array");
        let slot_issue = issues.iter().find(|issue| issue["rule"] == "slot-non-recommended").expect("slot warning must be returned");
        assert_eq!(slot_issue["node_id"], child_id.0, "slot warning must point at the inserted live node");
    }

    #[test]
    fn set_slot_disabled_is_idempotent_without_a_second_undo_step() {
        let mut engine = Engine { doc: designos_doc::fixtures::card_with_slots(), ..Engine::default() };
        let page = engine.doc.pages[0];
        let created =
            engine.call("designos_apply_spec", json!({"parent_id": page.0, "spec": {"kind": "instance", "name": "Card1", "component": "Card"}})).unwrap();
        let instance_id = created["root_node_id"].as_u64().unwrap();

        engine
            .call("designos_apply_batch", json!({"ops": [{"op": "set_slot", "node": instance_id, "slot": "footer", "fill": false}]}))
            .expect("first disable should commit");
        let depth_after_first = engine.history.undo_depth();
        let second = engine
            .call("designos_apply_batch", json!({"ops": [{"op": "set_slot", "node": instance_id, "slot": "footer", "fill": false}]}))
            .expect("repeated disable should be a no-op");
        assert_eq!(second["changed"], json!({"inserted": 0, "updated": 0, "moved": 0, "deleted": 0, "variables": 0, "styles": 0}));
        assert_eq!(engine.history.undo_depth(), depth_after_first, "repeated disable must not add history");
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
        let cases = [(100_000usize, 400.0 * scale), (112_474usize, 450.0 * scale), (1_200_000usize, 5_000.0 * scale)];

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
    fn batch_delete_rejects_a_linked_instance_child_and_leaves_the_document_unchanged() {
        let mut engine = Engine::default();
        let page = engine.resolve_default_page().unwrap();
        let component_id = engine.doc.alloc_id();
        let mut component = Node::new(component_id, designos_doc::NodeKind::Component(designos_doc::FrameProps::default()));
        component.name = "Card".into();
        engine.doc.insert(page, component).unwrap();
        let child_id = engine.doc.alloc_id();
        engine.doc.insert(component_id, Node::rectangle(child_id, designos_doc::Rect::new(0.0, 0.0, 80.0, 24.0))).unwrap();
        designos_doc::create_instance(&mut engine.doc, component_id, page, designos_doc::Vec2::new(160.0, 0.0)).apply(&mut engine.doc).unwrap();
        let instance_id = designos_doc::instances_of(&engine.doc, component_id)[0];
        let instance_child = engine.doc.get(instance_id).unwrap().children[0];
        let before = engine.doc.to_json().unwrap();

        let err = engine
            .call("designos_apply_batch", json!({ "ops": [{ "op": "delete", "node": instance_child.0 }] }))
            .expect_err("deleting a mirrored layer inside an instance must be refused");

        assert_eq!(err.code, ErrorCode::BatchOpFailed, "{err}");
        let cause = err.cause.as_deref().expect("the batch error names the cause");
        assert_eq!(cause.code, ErrorCode::InstanceChild, "{err}");
        assert!(!err.safe_to_retry, "the batch envelope takes the cause kind: {err}");
        assert_eq!(cause.path, "/ops/0/node");
        assert_eq!(engine.doc.to_json().unwrap(), before, "a refused batch must leave the document untouched");
        assert!(!engine.history.can_undo(), "a refused batch must not create history");

        let ok = engine.call("designos_apply_batch", json!({ "ops": [{ "op": "delete", "node": instance_id.0 }] }));
        assert!(ok.is_ok(), "deleting the instance root stays allowed: {ok:?}");
    }

    /// A component with one rectangle child and one instance of it, plus a free rectangle beside them.
    fn component_instance_and_free_rect(engine: &mut Engine) -> (NodeId, NodeId, NodeId, NodeId, NodeId) {
        let page = engine.resolve_default_page().unwrap();
        let component_id = engine.doc.alloc_id();
        let mut component = Node::new(component_id, designos_doc::NodeKind::Component(designos_doc::FrameProps::default()));
        component.name = "Card".into();
        engine.doc.insert(page, component).unwrap();
        let main_child = engine.doc.alloc_id();
        engine.doc.insert(component_id, Node::rectangle(main_child, designos_doc::Rect::new(0.0, 0.0, 80.0, 24.0))).unwrap();
        designos_doc::create_instance(&mut engine.doc, component_id, page, designos_doc::Vec2::new(160.0, 0.0)).apply(&mut engine.doc).unwrap();
        let instance_id = designos_doc::instances_of(&engine.doc, component_id)[0];
        let instance_child = engine.doc.get(instance_id).unwrap().children[0];
        let free = engine.doc.alloc_id();
        engine.doc.insert(page, Node::rectangle(free, designos_doc::Rect::new(0.0, 200.0, 40.0, 40.0))).unwrap();
        (main_child, instance_id, instance_child, free, component_id)
    }

    #[test]
    fn a_batch_with_one_mirrored_instance_layer_among_its_deletes_is_refused_whole() {
        let mut engine = Engine::default();
        let (_, _, instance_child, free, _) = component_instance_and_free_rect(&mut engine);
        let before = engine.doc.to_json().unwrap();

        let err = engine
            .call("designos_apply_batch", json!({ "ops": [{ "op": "delete", "node": free.0 }, { "op": "delete", "node": instance_child.0 }] }))
            .expect_err("the second delete is forbidden, so the whole batch is refused");

        assert_eq!(err.code, ErrorCode::BatchOpFailed, "{err}");
        assert_eq!(err.cause.as_deref().map(|c| c.path.as_str()), Some("/ops/1/node"));
        assert_eq!(engine.doc.to_json().unwrap(), before, "the free layer that was deleted first is back: nothing was committed");
        assert!(engine.doc.get(free).is_some());
        assert!(!engine.history.can_undo());
    }

    #[test]
    fn layers_of_the_main_component_stay_deletable_through_the_batch() {
        let mut engine = Engine::default();
        let (main_child, _, _, _, _) = component_instance_and_free_rect(&mut engine);

        let reply = engine.call("designos_apply_batch", json!({ "ops": [{ "op": "delete", "node": main_child.0 }] }));

        assert!(reply.is_ok(), "the main component is not an instance: {reply:?}");
        assert!(engine.doc.get(main_child).is_none());
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

    #[test]
    fn mcp_connect_creates_homed_connector_between_batch_handles() {
        let mut engine = Engine::default();
        let page = engine.resolve_default_page().unwrap();

        let res = engine
            .call(
                "designos_apply_batch",
                json!({
                    "ops": [
                        { "op": "insert", "as": "board", "parent": page.0, "kind": "frame", "name": "Board", "x": 40.0, "y": 50.0, "width": 600.0, "height": 400.0 },
                        { "op": "insert", "as": "a", "parent": "@board", "kind": "rectangle", "name": "Step A", "x": 20.0, "y": 40.0, "width": 100.0, "height": 60.0 },
                        { "op": "insert", "as": "b", "parent": "@board", "kind": "rectangle", "name": "Step B", "x": 260.0, "y": 40.0, "width": 100.0, "height": 60.0 },
                        {
                            "op": "connect",
                            "as": "ab",
                            "from": "@a",
                            "to": "$prev",
                            "from_magnet": "right",
                            "to_magnet": "left",
                            "line": "elbowed",
                            "start_cap": "circle",
                            "end_cap": "arrow",
                            "label": "Proceed"
                        }
                    ]
                }),
            )
            .expect("batch with connect op should succeed");

        let board_id = NodeId(res["name_to_id"]["board"].as_u64().unwrap());
        let a_id = NodeId(res["name_to_id"]["a"].as_u64().unwrap());
        let b_id = NodeId(res["name_to_id"]["b"].as_u64().unwrap());
        let conn_id = NodeId(res["name_to_id"]["ab"].as_u64().unwrap());

        let board = engine.doc.get(board_id).unwrap();
        assert_eq!(board.children.last().copied(), Some(conn_id), "new connector must be inserted last (topmost) in connector_home");

        let conn = engine.doc.get(conn_id).unwrap();
        assert_eq!(conn.parent, Some(board_id), "connector home must be the enclosing frame, not the page");
        assert!(conn.layout_child.absolute, "connector must have layout_child.absolute = true");
        assert_eq!(conn.stroke.weight, 2.0);

        let NodeKind::Connector(props) = &conn.kind else {
            panic!("expected Connector node kind, got {:?}", conn.kind);
        };
        assert_eq!(props.start.node, Some(a_id));
        assert_eq!(props.start.magnet, designos_doc::Magnet::Right);
        assert_eq!(props.end.node, Some(b_id));
        assert_eq!(props.end.magnet, designos_doc::Magnet::Left);
        assert_eq!(props.line, designos_doc::ConnectorLine::Elbowed);
        assert_eq!(props.start_cap, designos_doc::ConnectorCap::Circle);
        assert_eq!(props.end_cap, designos_doc::ConnectorCap::Arrow);
        assert_eq!(props.label.as_ref().map(|l| l.content.as_str()), Some("Proceed"));

        let expected_geom = designos_doc::connector_geometry(&engine.doc, conn_id).expect("connector_geometry");
        assert_eq!(conn.transform, expected_geom.transform, "stored transform must match connector_geometry right after connect");
        assert_eq!(conn.size, expected_geom.size, "stored size must match connector_geometry right after connect");
        assert_eq!(props.start.point, expected_geom.start_point);
        assert_eq!(props.end.point, expected_geom.end_point);
    }

    #[test]
    fn mcp_connect_without_stroke_uses_the_default_ink() {
        let mut engine = Engine::default();
        let page = engine.resolve_default_page().unwrap();
        let rect = |parent: &str, name: &str, x: f64| json!({ "op": "insert", "as": name, "parent": parent, "kind": "rectangle", "name": name, "x": x, "y": 40.0, "width": 80.0, "height": 60.0 });
        let frame = |name: &str, x: f64| json!({ "op": "insert", "as": name, "parent": page.0, "kind": "frame", "name": name, "x": x, "y": 50.0, "width": 400.0, "height": 240.0 });
        let res = engine
            .call(
                "designos_apply_batch",
                json!({ "ops": [
                    frame("day", 0.0), rect("@day", "a1", 20.0), rect("@day", "b1", 280.0),
                    frame("night", 500.0), rect("@night", "a2", 20.0), rect("@night", "b2", 280.0),
                    frame("left", 1000.0), frame("right", 1500.0),
                ] }),
            )
            .expect("fixture batch");
        let id = |name: &str| NodeId(res["name_to_id"][name].as_u64().unwrap());
        engine.doc.get_mut(id("day")).unwrap().fills = vec![Paint::solid(Color::WHITE)];
        engine.doc.get_mut(id("night")).unwrap().fills = vec![Paint::solid(Color::rgb(0x12, 0x12, 0x15))];

        let res = engine
            .call(
                "designos_apply_batch",
                json!({ "ops": [
                    { "op": "connect", "as": "in_day", "from": id("a1").0, "to": id("b1").0 },
                    { "op": "connect", "as": "in_night", "from": id("a2").0, "to": id("b2").0 },
                    { "op": "connect", "as": "on_page", "from": id("left").0, "to": id("right").0 },
                ] }),
            )
            .expect("connect batch");
        let ink = |name: &str| {
            let c = NodeId(res["name_to_id"][name].as_u64().unwrap());
            match &engine.doc.get(c).unwrap().stroke.paints[0].kind {
                designos_doc::PaintKind::Solid(col) => *col,
                k => panic!("expected a solid stroke, got {k:?}"),
            }
        };
        assert_eq!(ink("in_day"), Color::rgb(0x1E, 0x1E, 0x1E), "a white screen takes the dark ink");
        assert_eq!(ink("in_night"), Color::WHITE, "a dark screen takes white ink");
        assert_eq!(ink("on_page"), Color::WHITE, "the dark default page takes white ink");
    }

    #[test]
    fn mcp_connect_rejects_cross_page_and_same_node() {
        let mut engine = Engine::default();
        let page1 = engine.resolve_default_page().unwrap();
        let page2 = engine.doc.add_page("Page 2");

        let a = engine
            .call("designos_create_node", json!({ "parent_id": page1.0, "kind": "rectangle", "name": "A", "x": 0.0, "y": 0.0, "width": 80.0, "height": 50.0 }))
            .unwrap()["node"]["id"]
            .as_u64()
            .unwrap();
        let b = engine
            .call(
                "designos_create_node",
                json!({ "parent_id": page1.0, "kind": "rectangle", "name": "B", "x": 200.0, "y": 0.0, "width": 80.0, "height": 50.0 }),
            )
            .unwrap()["node"]["id"]
            .as_u64()
            .unwrap();
        let on_page2 = engine
            .call(
                "designos_create_node",
                json!({ "parent_id": page2.0, "kind": "rectangle", "name": "OtherPageBox", "x": 0.0, "y": 0.0, "width": 80.0, "height": 50.0 }),
            )
            .unwrap()["node"]["id"]
            .as_u64()
            .unwrap();

        // 1. Cross-page ends refused with CONNECTOR_ENDS_ON_DIFFERENT_PAGES.
        let cross_err = engine
            .call("designos_apply_batch", json!({ "ops": [{ "op": "connect", "from": a, "to": on_page2 }] }))
            .expect_err("cross-page connect must be refused");
        assert!(cross_err.contains("CONNECTOR_ENDS_ON_DIFFERENT_PAGES"), "expected CONNECTOR_ENDS_ON_DIFFERENT_PAGES in error: {cross_err}");

        // 2. Same node on both ends refused with UNSUPPORTED_NODE_KIND.
        let same_err =
            engine.call("designos_apply_batch", json!({ "ops": [{ "op": "connect", "from": a, "to": a }] })).expect_err("same-node connect must be refused");
        let same_json = same_err.to_json_value();
        assert_eq!(same_json["cause"]["code"], "UNSUPPORTED_NODE_KIND", "expected UNSUPPORTED_NODE_KIND cause: {same_json}");

        // 3. Connector as an end refused with UNSUPPORTED_NODE_KIND.
        let conn_res = engine.call("designos_apply_batch", json!({ "ops": [{ "op": "connect", "as": "c1", "from": a, "to": b }] })).expect("valid connect");
        let c1 = conn_res["name_to_id"]["c1"].as_u64().unwrap();
        let conn_end_err =
            engine.call("designos_apply_batch", json!({ "ops": [{ "op": "connect", "from": c1, "to": b }] })).expect_err("connector as an end must be refused");
        let conn_end_json = conn_end_err.to_json_value();
        assert_eq!(conn_end_json["cause"]["code"], "UNSUPPORTED_NODE_KIND", "expected UNSUPPORTED_NODE_KIND cause: {conn_end_json}");

        // 4. Hidden or locked node refused with UNSUPPORTED_NODE_KIND.
        engine.doc.get_mut(NodeId(b)).unwrap().locked = true;
        let locked_err = engine
            .call("designos_apply_batch", json!({ "ops": [{ "op": "connect", "from": a, "to": b }] }))
            .expect_err("locked node as an end must be refused");
        assert_eq!(locked_err.to_json_value()["cause"]["code"], "UNSUPPORTED_NODE_KIND");
    }

    #[test]
    fn mcp_export_spec_warns_that_connectors_are_skipped() {
        let mut engine = Engine::default();
        let page = engine.resolve_default_page().unwrap();

        let batch = engine
            .call(
                "designos_apply_batch",
                json!({
                    "ops": [
                        { "op": "insert", "as": "container", "parent": page.0, "kind": "frame", "name": "Flow", "width": 500.0, "height": 300.0 },
                        { "op": "insert", "as": "n1", "parent": "@container", "kind": "rectangle", "name": "Node 1", "x": 20.0, "y": 40.0, "width": 100.0, "height": 50.0 },
                        { "op": "insert", "as": "n2", "parent": "@container", "kind": "rectangle", "name": "Node 2", "x": 240.0, "y": 40.0, "width": 100.0, "height": 50.0 },
                        { "op": "connect", "from": "@n1", "to": "@n2" },
                        { "op": "connect", "from": "@n2", "to": { "point": [420.0, 65.0] } }
                    ]
                }),
            )
            .expect("create container with two connectors");
        let container_id = batch["name_to_id"]["container"].as_u64().unwrap();

        let exported = engine.call("designos_export_spec", json!({ "node_id": container_id })).expect("export_spec");
        let children = exported["spec"]["children"].as_array().expect("spec children");
        assert_eq!(children.len(), 2, "both connectors must be skipped from exported children: {exported}");
        assert!(children.iter().all(|c| c["kind"] != "connector"));

        let warnings = exported["warnings"].as_array().expect("warnings array must be present when connectors are skipped");
        assert!(!warnings.is_empty(), "warnings must not be empty");
        let warning_text = serde_json::to_string(warnings).unwrap();
        assert!(warning_text.contains('2'), "warning must name how many connectors were skipped (2): {warning_text}");
    }

    #[test]
    fn mcp_connect_with_free_point_ends_converts_page_space_to_home_space() {
        let mut engine = Engine::default();
        let page = engine.resolve_default_page().unwrap();

        let res = engine
            .call(
                "designos_apply_batch",
                json!({
                    "ops": [
                        { "op": "insert", "as": "frame", "parent": page.0, "kind": "frame", "name": "Frame", "x": 100.0, "y": 200.0, "width": 400.0, "height": 300.0 },
                        { "op": "insert", "as": "box", "parent": "@frame", "kind": "rectangle", "name": "Box", "x": 20.0, "y": 30.0, "width": 80.0, "height": 40.0 },
                        { "op": "connect", "as": "half_free", "from": "@box", "to": { "point": [350.0, 250.0] } },
                        { "op": "connect", "as": "both_free", "from": { "point": [10.0, 20.0] }, "to": { "point": [90.0, 20.0] }, "line": "straight" }
                    ]
                }),
            )
            .expect("connect with point ends");

        let frame_id = NodeId(res["name_to_id"]["frame"].as_u64().unwrap());
        let half_free_id = NodeId(res["name_to_id"]["half_free"].as_u64().unwrap());
        let both_free_id = NodeId(res["name_to_id"]["both_free"].as_u64().unwrap());

        let half_node = engine.doc.get(half_free_id).unwrap();
        assert_eq!(half_node.parent, Some(frame_id), "one attached end homes in that end's enclosing frame");
        let NodeKind::Connector(half_props) = &half_node.kind else { panic!("expected connector") };
        assert_eq!(half_props.end.node, None);
        // Page point (350, 250) inside frame at (100, 200) converts to local (250, 50).
        assert_eq!(half_props.end.point, designos_doc::Vec2::new(250.0, 50.0));
        let g1 = designos_doc::connector_geometry(&engine.doc, half_free_id).unwrap();
        assert_eq!(half_node.transform, g1.transform);
        assert_eq!(half_node.size, g1.size);

        let free_node = engine.doc.get(both_free_id).unwrap();
        assert_eq!(free_node.parent, Some(page), "two free ends home on the page");
        let NodeKind::Connector(free_props) = &free_node.kind else { panic!("expected connector") };
        assert_eq!(free_props.start.point, designos_doc::Vec2::new(10.0, 20.0));
        assert_eq!(free_props.end.point, designos_doc::Vec2::new(90.0, 20.0));
        let g2 = designos_doc::connector_geometry(&engine.doc, both_free_id).unwrap();
        assert_eq!(free_node.transform, g2.transform);
        assert_eq!(free_node.size, g2.size);
    }

    #[test]
    fn mcp_connect_with_node_updates_existing_connector() {
        let mut engine = Engine::default();
        let page = engine.resolve_default_page().unwrap();

        let res = engine
            .call(
                "designos_apply_batch",
                json!({
                    "ops": [
                        { "op": "insert", "as": "a", "parent": page.0, "kind": "rectangle", "name": "A", "x": 0.0, "y": 0.0, "width": 80.0, "height": 50.0 },
                        { "op": "insert", "as": "b", "parent": page.0, "kind": "rectangle", "name": "B", "x": 200.0, "y": 0.0, "width": 80.0, "height": 50.0 },
                        { "op": "insert", "as": "c", "parent": page.0, "kind": "rectangle", "name": "C", "x": 0.0, "y": 200.0, "width": 80.0, "height": 50.0 },
                        { "op": "connect", "as": "link", "from": "@a", "to": "@b" }
                    ]
                }),
            )
            .expect("initial connect");

        let a_id = NodeId(res["name_to_id"]["a"].as_u64().unwrap());
        let c_id = NodeId(res["name_to_id"]["c"].as_u64().unwrap());
        let link_id = NodeId(res["name_to_id"]["link"].as_u64().unwrap());
        let count_before = engine.doc.nodes.len();

        engine
            .call(
                "designos_apply_batch",
                json!({
                    "ops": [
                        {
                            "op": "connect",
                            "node": link_id.0,
                            "from": a_id.0,
                            "to": c_id.0,
                            "from_magnet": "bottom",
                            "to_magnet": "top",
                            "line": "curved",
                            "start_cap": "diamond",
                            "end_cap": "triangle",
                            "label": "Rerouted"
                        }
                    ]
                }),
            )
            .expect("update existing connector via connect op");

        assert_eq!(engine.doc.nodes.len(), count_before, "updating an existing connector must not allocate a new node");
        let conn = engine.doc.get(link_id).unwrap();
        let NodeKind::Connector(props) = &conn.kind else { panic!("expected connector") };
        assert_eq!(props.start.node, Some(a_id));
        assert_eq!(props.start.magnet, designos_doc::Magnet::Bottom);
        assert_eq!(props.end.node, Some(c_id));
        assert_eq!(props.end.magnet, designos_doc::Magnet::Top);
        assert_eq!(props.line, designos_doc::ConnectorLine::Curved);
        assert_eq!(props.start_cap, designos_doc::ConnectorCap::Diamond);
        assert_eq!(props.end_cap, designos_doc::ConnectorCap::Triangle);
        assert_eq!(props.label.as_ref().map(|l| l.content.as_str()), Some("Rerouted"));

        let expected_geom = designos_doc::connector_geometry(&engine.doc, link_id).unwrap();
        assert_eq!(conn.transform, expected_geom.transform);
        assert_eq!(conn.size, expected_geom.size);
    }

    #[test]
    fn mcp_connect_inside_auto_layout_frame_preserves_frame_size_and_siblings() {
        let mut engine = Engine::default();
        let page = engine.resolve_default_page().unwrap();

        let setup = engine
            .call(
                "designos_apply_batch",
                json!({
                    "ops": [
                        {
                            "op": "apply_spec",
                            "as": "row",
                            "parent": page.0,
                            "spec": {
                                "kind": "frame",
                                "name": "AutoRow",
                                "width_sizing": "hug",
                                "height_sizing": "hug",
                                "layout": { "direction": "row", "gap": 24.0, "padding": [16.0, 16.0, 16.0, 16.0] },
                                "children": [
                                    { "kind": "rectangle", "name": "Left", "width": 100.0, "height": 50.0 },
                                    { "kind": "rectangle", "name": "Right", "width": 100.0, "height": 50.0 }
                                ]
                            }
                        }
                    ]
                }),
            )
            .expect("setup auto-layout row");

        let row_id = NodeId(setup["name_to_id"]["row"].as_u64().unwrap());
        let left_id = NodeId(setup["name_to_id"]["row/Left"].as_u64().unwrap());
        let right_id = NodeId(setup["name_to_id"]["row/Right"].as_u64().unwrap());

        let row_size_before = engine.doc.get(row_id).unwrap().size;
        let left_xf_before = engine.doc.get(left_id).unwrap().transform;
        let right_xf_before = engine.doc.get(right_id).unwrap().transform;

        let conn_res = engine
            .call(
                "designos_apply_batch",
                json!({
                    "ops": [
                        { "op": "connect", "as": "edge", "from": left_id.0, "to": right_id.0 }
                    ]
                }),
            )
            .expect("connect inside auto-layout frame");

        let edge_id = NodeId(conn_res["name_to_id"]["edge"].as_u64().unwrap());
        let edge = engine.doc.get(edge_id).unwrap();
        assert_eq!(edge.parent, Some(row_id));
        assert!(edge.layout_child.absolute, "connector inside auto-layout frame must be absolute");

        assert_eq!(engine.doc.get(row_id).unwrap().size, row_size_before, "auto-layout frame size must not change when connector is added");
        assert_eq!(engine.doc.get(left_id).unwrap().transform, left_xf_before, "Left sibling position must not change");
        assert_eq!(engine.doc.get(right_id).unwrap().transform, right_xf_before, "Right sibling position must not change");
    }

    #[test]
    fn mcp_connect_single_undo_removes_connector_and_batch() {
        let mut engine = Engine::default();
        let page = engine.resolve_default_page().unwrap();
        let pre_nodes = engine.doc.nodes.clone();
        let pre_hash = batch_state_hash(&engine.doc, &[]);
        let depth_before = engine.history.undo_depth();

        let res = engine
            .call(
                "designos_apply_batch",
                json!({
                    "ops": [
                        { "op": "insert", "as": "a", "parent": page.0, "kind": "rectangle", "name": "A", "x": 10.0, "y": 10.0, "width": 80.0, "height": 40.0 },
                        { "op": "insert", "as": "b", "parent": page.0, "kind": "rectangle", "name": "B", "x": 180.0, "y": 10.0, "width": 80.0, "height": 40.0 },
                        { "op": "connect", "as": "ab", "from": "@a", "to": "@b" }
                    ]
                }),
            )
            .expect("batch create + connect");

        let a_id = NodeId(res["name_to_id"]["a"].as_u64().unwrap());
        let b_id = NodeId(res["name_to_id"]["b"].as_u64().unwrap());
        let conn_id = NodeId(res["name_to_id"]["ab"].as_u64().unwrap());
        assert_eq!(engine.history.undo_depth(), depth_before + 1, "entire batch including connect must be a single undo step");

        engine.call("designos_undo", json!({})).expect("single undo");
        assert!(engine.doc.get(conn_id).is_none(), "one undo must remove the connector");
        assert!(engine.doc.get(a_id).is_none(), "one undo must also remove the batch's earlier ops");
        assert!(engine.doc.get(b_id).is_none(), "one undo must also remove the batch's earlier ops");
        assert_eq!(engine.doc.nodes, pre_nodes);
        assert_eq!(batch_state_hash(&engine.doc, &[]), pre_hash);
    }

    #[test]
    fn mcp_connect_dry_run_mutates_nothing() {
        let mut engine = Engine::default();
        let page = engine.resolve_default_page().unwrap();
        let a = engine
            .call("designos_create_node", json!({ "parent_id": page.0, "kind": "rectangle", "name": "A", "x": 0.0, "y": 0.0, "width": 80.0, "height": 40.0 }))
            .unwrap()["node"]["id"]
            .as_u64()
            .unwrap();
        let b = engine
            .call("designos_create_node", json!({ "parent_id": page.0, "kind": "rectangle", "name": "B", "x": 200.0, "y": 0.0, "width": 80.0, "height": 40.0 }))
            .unwrap()["node"]["id"]
            .as_u64()
            .unwrap();

        let pre_nodes = engine.doc.nodes.clone();
        let pre_hash = batch_state_hash(&engine.doc, &[]);
        let depth_before = engine.history.undo_depth();

        let dry = engine
            .call(
                "designos_apply_batch",
                json!({
                    "dry_run": true,
                    "ops": [
                        { "op": "connect", "as": "preview_edge", "from": a, "to": b }
                    ]
                }),
            )
            .expect("dry_run connect");

        assert_eq!(dry["dry_run"], true);
        let would_be_id = dry["name_to_id"]["preview_edge"].as_u64().expect("dry_run returns would-be id");
        assert!(would_be_id > b);
        assert!(engine.doc.get(NodeId(would_be_id)).is_none(), "dry_run must not insert the connector");
        assert_eq!(engine.doc.nodes, pre_nodes, "dry_run must leave document untouched");
        assert_eq!(batch_state_hash(&engine.doc, &[]), pre_hash, "dry_run must leave document hash untouched");
        assert_eq!(engine.history.undo_depth(), depth_before, "dry_run must not push an undo step");
    }

    #[test]
    fn mcp_get_node_and_get_metadata_show_connector_ends_and_magnets() {
        let mut engine = Engine::default();
        let page = engine.resolve_default_page().unwrap();

        let res = engine
            .call(
                "designos_apply_batch",
                json!({
                    "ops": [
                        { "op": "insert", "as": "a", "parent": page.0, "kind": "rectangle", "name": "Source", "x": 10.0, "y": 20.0, "width": 80.0, "height": 40.0 },
                        { "op": "insert", "as": "b", "parent": page.0, "kind": "rectangle", "name": "Target", "x": 210.0, "y": 20.0, "width": 80.0, "height": 40.0 },
                        { "op": "connect", "as": "edge", "from": "@a", "to": "@b", "from_magnet": "right", "to_magnet": "left" }
                    ]
                }),
            )
            .expect("connect two nodes");

        let a_id = res["name_to_id"]["a"].as_u64().unwrap();
        let b_id = res["name_to_id"]["b"].as_u64().unwrap();
        let edge_id = res["name_to_id"]["edge"].as_u64().unwrap();

        let got_node = engine.call("designos_get_node", json!({ "node_id": edge_id })).expect("get_node");
        let c_json = &got_node["node"]["kind"]["Connector"];
        assert_eq!(c_json["start"]["node"], a_id);
        assert_eq!(c_json["start"]["magnet"], "Right");
        assert_eq!(c_json["end"]["node"], b_id);
        assert_eq!(c_json["end"]["magnet"], "Left");

        let meta = engine.call("designos_get_metadata", json!({ "node_id": page.0 })).expect("get_metadata");
        let xml = meta["xml"].as_str().expect("metadata xml");
        let conn_line = xml.lines().find(|l| l.contains("<connector ") && l.contains(&format!("id=\"{edge_id}\""))).expect("connector line in metadata xml");
        assert!(conn_line.contains(&format!("from=\"{a_id}\"")), "metadata xml must show connector start node: {conn_line}");
        assert!(conn_line.contains("from_magnet=\"right\""), "metadata xml must show connector start magnet: {conn_line}");
        assert!(conn_line.contains(&format!("to=\"{b_id}\"")), "metadata xml must show connector end node: {conn_line}");
        assert!(conn_line.contains("to_magnet=\"left\""), "metadata xml must show connector end magnet: {conn_line}");
    }

    /// Two page-level rectangles joined by a connector; returns `(a, b, link)`.
    fn page_connector_between_two_boxes(engine: &mut Engine, page: NodeId) -> (NodeId, NodeId, NodeId) {
        let res = engine
            .call(
                "designos_apply_batch",
                json!({
                    "ops": [
                        { "op": "insert", "as": "a", "parent": page.0, "kind": "rectangle", "name": "A", "x": 0.0, "y": 0.0, "width": 80.0, "height": 50.0 },
                        { "op": "insert", "as": "b", "parent": page.0, "kind": "rectangle", "name": "B", "x": 200.0, "y": 0.0, "width": 80.0, "height": 50.0 },
                        { "op": "connect", "as": "link", "from": "@a", "to": "@b" }
                    ]
                }),
            )
            .expect("two boxes and a connector");
        let id = |name: &str| NodeId(res["name_to_id"][name].as_u64().unwrap());
        (id("a"), id("b"), id("link"))
    }

    fn connector_ends(engine: &Engine, id: NodeId) -> (designos_doc::ConnectorEnd, designos_doc::ConnectorEnd) {
        let NodeKind::Connector(props) = &engine.doc.get(id).unwrap().kind else { panic!("expected connector") };
        (props.start, props.end)
    }

    fn connector_label(engine: &Engine, id: NodeId) -> Option<String> {
        let NodeKind::Connector(props) = &engine.doc.get(id).unwrap().kind else { panic!("expected connector") };
        props.label.as_ref().map(|l| l.content.clone())
    }

    fn assert_connector_matches_its_geometry(engine: &Engine, id: NodeId) {
        let node = engine.doc.get(id).unwrap();
        let geom = designos_doc::connector_geometry(&engine.doc, id).expect("connector geometry");
        assert_eq!(node.transform, geom.transform, "stored transform must match connector_geometry");
        assert_eq!(node.size, geom.size, "stored size must match connector_geometry");
        let (start, end) = connector_ends(engine, id);
        assert_eq!(start.point, geom.start_point, "stored start point must match connector_geometry");
        assert_eq!(end.point, geom.end_point, "stored end point must match connector_geometry");
    }

    fn label_only_update(engine: &mut Engine, link: NodeId) -> Result<Value, ToolError> {
        engine.call("designos_apply_batch", json!({ "ops": [{ "op": "connect", "node": link.0, "label": "kept" }] }))
    }

    #[test]
    fn mcp_connect_update_keeps_stored_ends_when_an_end_node_is_locked() {
        let mut engine = Engine::default();
        let page = engine.resolve_default_page().unwrap();
        let (a, b, link) = page_connector_between_two_boxes(&mut engine, page);
        engine.doc.get_mut(b).unwrap().locked = true;
        let ends_before = connector_ends(&engine, link);
        let parent_before = engine.doc.get(link).unwrap().parent;

        label_only_update(&mut engine, link).expect("an update that names no end must not re-check the stored ends");

        assert_eq!(connector_ends(&engine, link), ends_before, "stored ends must stay as they are");
        assert_eq!(connector_ends(&engine, link).1.node, Some(b));
        assert_eq!(engine.doc.get(link).unwrap().parent, parent_before);
        assert_eq!(connector_label(&engine, link).as_deref(), Some("kept"));

        // Passing the locked node again is a new attach and is still refused.
        let err = engine
            .call("designos_apply_batch", json!({ "ops": [{ "op": "connect", "node": link.0, "to": b.0 }] }))
            .expect_err("re-attaching a locked node must be refused");
        assert_eq!(err.to_json_value()["cause"]["code"], "UNSUPPORTED_NODE_KIND");
        let err = engine
            .call("designos_apply_batch", json!({ "ops": [{ "op": "connect", "node": link.0, "from": a.0, "to": b.0 }] }))
            .expect_err("re-attaching a locked node must be refused even when the other end is valid");
        assert_eq!(err.to_json_value()["cause"]["code"], "UNSUPPORTED_NODE_KIND");
    }

    #[test]
    fn mcp_connect_update_keeps_stored_ends_when_an_end_node_is_hidden() {
        let mut engine = Engine::default();
        let page = engine.resolve_default_page().unwrap();
        let (a, b, link) = page_connector_between_two_boxes(&mut engine, page);
        engine.doc.get_mut(a).unwrap().visible = false;
        let ends_before = connector_ends(&engine, link);

        label_only_update(&mut engine, link).expect("an update that names no end must not re-check the stored ends");

        assert_eq!(connector_ends(&engine, link), ends_before, "stored ends must stay as they are");
        assert_eq!(connector_ends(&engine, link).0.node, Some(a));
        assert_eq!(connector_ends(&engine, link).1.node, Some(b));
        assert_eq!(connector_label(&engine, link).as_deref(), Some("kept"));

        let err = engine
            .call("designos_apply_batch", json!({ "ops": [{ "op": "connect", "node": link.0, "from": a.0 }] }))
            .expect_err("re-attaching a hidden node must be refused");
        assert_eq!(err.to_json_value()["cause"]["code"], "UNSUPPORTED_NODE_KIND");
    }

    #[test]
    fn mcp_connect_update_keeps_stored_ends_when_an_end_node_is_deleted() {
        let mut engine = Engine::default();
        let page = engine.resolve_default_page().unwrap();
        let (a, b, link) = page_connector_between_two_boxes(&mut engine, page);
        engine.call("designos_apply_batch", json!({ "ops": [{ "op": "delete", "node": b.0 }] })).expect("delete the end node");
        assert!(engine.doc.get(b).is_none());
        let ends_before = connector_ends(&engine, link);
        assert_eq!(ends_before.1.node, Some(b), "a deleted end node stays in the connector as a dangling id");

        label_only_update(&mut engine, link).expect("an update that names no end must not re-check the stored ends");

        assert_eq!(connector_ends(&engine, link), ends_before, "stored ends, dangling id and last point included, must stay as they are");
        assert_eq!(connector_ends(&engine, link).0.node, Some(a));
        assert_eq!(connector_label(&engine, link).as_deref(), Some("kept"));
        assert_connector_matches_its_geometry(&engine, link);
    }

    #[test]
    fn mcp_connect_update_without_ends_leaves_the_connector_in_its_container() {
        let mut engine = Engine::default();
        let page = engine.resolve_default_page().unwrap();
        let setup = engine
            .call(
                "designos_apply_batch",
                json!({
                    "ops": [
                        { "op": "insert", "as": "left", "parent": page.0, "kind": "frame", "name": "Left", "x": 0.0, "y": 0.0, "width": 300.0, "height": 200.0 },
                        { "op": "insert", "as": "right", "parent": page.0, "kind": "frame", "name": "Right", "x": 500.0, "y": 0.0, "width": 300.0, "height": 200.0 },
                        { "op": "insert", "as": "a", "parent": "@left", "kind": "rectangle", "name": "A", "x": 20.0, "y": 20.0, "width": 80.0, "height": 50.0 },
                        { "op": "insert", "as": "b", "parent": "@right", "kind": "rectangle", "name": "B", "x": 20.0, "y": 20.0, "width": 80.0, "height": 50.0 },
                        { "op": "connect", "as": "link", "from": "@a", "to": "@b" }
                    ]
                }),
            )
            .expect("two frames joined by a connector");
        let b = NodeId(setup["name_to_id"]["b"].as_u64().unwrap());
        let link = NodeId(setup["name_to_id"]["link"].as_u64().unwrap());
        assert_eq!(engine.doc.get(link).unwrap().parent, Some(page), "a connector between two screens lives on the page");
        engine.call("designos_apply_batch", json!({ "ops": [{ "op": "delete", "node": b.0 }] })).expect("delete the end node");
        let ends_before = connector_ends(&engine, link);

        label_only_update(&mut engine, link).expect("label-only update");

        assert_eq!(engine.doc.get(link).unwrap().parent, Some(page), "an update that attaches nothing must not re-home the connector");
        assert_eq!(connector_ends(&engine, link), ends_before);
        assert_eq!(connector_label(&engine, link).as_deref(), Some("kept"));
    }

    #[test]
    fn mcp_connect_update_retargeting_both_ends_into_a_frame_rehomes_the_connector() {
        let mut engine = Engine::default();
        let page = engine.resolve_default_page().unwrap();
        let (_a, _b, link) = page_connector_between_two_boxes(&mut engine, page);
        assert_eq!(engine.doc.get(link).unwrap().parent, Some(page));
        let setup = engine
            .call(
                "designos_apply_batch",
                json!({
                    "ops": [
                        { "op": "insert", "as": "panel", "parent": page.0, "kind": "frame", "name": "Panel", "x": 100.0, "y": 300.0, "width": 400.0, "height": 300.0 },
                        { "op": "insert", "as": "x", "parent": "@panel", "kind": "rectangle", "name": "X", "x": 20.0, "y": 30.0, "width": 80.0, "height": 40.0 },
                        { "op": "insert", "as": "y", "parent": "@panel", "kind": "rectangle", "name": "Y", "x": 240.0, "y": 150.0, "width": 80.0, "height": 40.0 }
                    ]
                }),
            )
            .expect("frame with two boxes");
        let id = |name: &str| NodeId(setup["name_to_id"][name].as_u64().unwrap());
        let (panel, x, y) = (id("panel"), id("x"), id("y"));

        engine
            .call("designos_apply_batch", json!({ "ops": [{ "op": "connect", "node": link.0, "from": x.0, "to": y.0 }] }))
            .expect("retarget both ends into the frame");

        let conn = engine.doc.get(link).unwrap();
        assert_eq!(conn.parent, Some(panel), "retargeting both ends into one frame must move the connector into it");
        assert_eq!(engine.doc.get(panel).unwrap().children.last().copied(), Some(link), "the re-homed connector must be the topmost child");
        assert!(!engine.doc.get(page).unwrap().children.contains(&link), "the connector must leave the page");
        assert!(conn.layout_child.absolute, "a re-homed connector stays out of auto-layout flow");
        assert_connector_matches_its_geometry(&engine, link);
    }

    #[test]
    fn mcp_move_connector_endpoints_rehomes_detaches_warns_and_undoes_atomically() {
        let mut engine = Engine::default();
        let page = engine.resolve_default_page().unwrap();
        let page2 = engine.doc.add_page("Page 2");
        let setup = engine
            .call(
                "designos_apply_batch",
                json!({"ops":[
                    {"op":"insert","as":"frame","parent":page.0,"kind":"frame","name":"Frame","x":100.0,"y":100.0,"width":400.0,"height":300.0},
                    {"op":"insert","as":"a","parent":page.0,"kind":"rectangle","name":"A","x":120.0,"y":120.0,"width":80.0,"height":40.0},
                    {"op":"insert","as":"b","parent":page.0,"kind":"rectangle","name":"B","x":300.0,"y":240.0,"width":80.0,"height":40.0},
                    {"op":"connect","as":"link","from":"@a","to":"@b"}
                ]}),
            )
            .expect("setup connector fixture");
        let id = |name: &str| NodeId(setup["name_to_id"][name].as_u64().unwrap());
        let (frame, a, b, link) = (id("frame"), id("a"), id("b"), id("link"));
        engine.live_ops = Some(Vec::new());
        engine.take_live_ops();
        engine
            .call(
                "designos_apply_batch",
                json!({"ops":[
                    {"op":"move","node":a.0,"parent":frame.0}, {"op":"move","node":b.0,"parent":frame.0}
                ]}),
            )
            .expect("move both endpoints into frame");
        assert_eq!(engine.doc.get(link).unwrap().parent, Some(frame));
        assert_connector_matches_its_geometry(&engine, link);
        assert!(engine.take_live_ops().iter().any(|op| matches!(op, LiveOp::Reparent { id, parent, .. } if *id == link && *parent == frame)));
        let detached = engine.call("designos_apply_batch", json!({"ops":[{"op":"move","node":a.0,"parent":page2.0}]})).expect("move endpoint to page 2");
        let NodeKind::Connector(c) = &engine.doc.get(link).unwrap().kind else { panic!("expected connector") };
        assert_eq!(c.start.node, None);
        assert_eq!(c.end.node, Some(b));
        assert!(detached["warnings"].as_array().is_some_and(|ws| ws.iter().any(|w| w["code"] == "CONNECTOR_ENDPOINT_DETACHED")));
        assert_connector_matches_its_geometry(&engine, link);
        assert!(engine.take_live_ops().iter().any(|op| matches!(op, LiveOp::Update { id, .. } if *id == link)));
        engine.call("designos_undo", json!({})).expect("undo detach");
        assert_eq!(connector_ends(&engine, link).0.node, Some(a));
        assert_eq!(engine.doc.get(a).unwrap().parent, Some(frame));
    }

    #[test]
    fn a_connector_whose_other_end_moved_to_another_page_follows_the_end_that_stays() {
        let mut engine = Engine::default();
        let page = engine.resolve_default_page().unwrap();
        let page2 = engine.doc.add_page("Page 2");
        let setup = engine
            .call(
                "designos_apply_batch",
                json!({"ops":[
                    {"op":"insert","as":"a","parent":page.0,"kind":"rectangle","name":"A","x":120.0,"y":120.0,"width":80.0,"height":40.0},
                    {"op":"insert","as":"b","parent":page.0,"kind":"rectangle","name":"B","x":300.0,"y":240.0,"width":80.0,"height":40.0},
                    {"op":"connect","as":"link","from":"@a","to":"@b"}
                ]}),
            )
            .expect("setup connector fixture");
        let id = |name: &str| NodeId(setup["name_to_id"][name].as_u64().unwrap());
        let (a, b, link) = (id("a"), id("b"), id("link"));
        engine.call("designos_apply_batch", json!({"ops":[{"op":"move","node":a.0,"parent":page2.0}]})).expect("move one end to page 2");
        let (start_before, end_before) = connector_ends(&engine, link);
        assert_eq!(start_before.node, None, "the end on another page is detached");

        engine.call("designos_apply_batch", json!({"ops":[{"op":"update","node":b.0,"x":500.0,"y":340.0}]})).expect("move the end that stays");

        let (start, end) = connector_ends(&engine, link);
        assert_eq!(start.node, None);
        assert_eq!(start.point, start_before.point, "the detached end keeps its free point");
        assert_eq!(end.node, Some(b));
        assert_ne!(end.point, end_before.point, "the attached end follows its layer");
        assert_connector_matches_its_geometry(&engine, link);
    }

    #[test]
    fn mcp_connect_free_point_inside_a_rotated_frame_maps_back_to_the_page_point() {
        let mut engine = Engine::default();
        let page = engine.resolve_default_page().unwrap();
        let setup = engine
            .call(
                "designos_apply_batch",
                json!({
                    "ops": [
                        { "op": "insert", "as": "frame", "parent": page.0, "kind": "frame", "name": "Tilted", "x": 100.0, "y": 200.0, "width": 400.0, "height": 300.0 },
                        { "op": "insert", "as": "box", "parent": "@frame", "kind": "rectangle", "name": "Box", "x": 20.0, "y": 30.0, "width": 80.0, "height": 40.0 },
                        { "op": "insert", "as": "other", "parent": page.0, "kind": "rectangle", "name": "Other", "x": 900.0, "y": 0.0, "width": 80.0, "height": 40.0 },
                        { "op": "insert", "as": "far", "parent": page.0, "kind": "rectangle", "name": "Far", "x": 900.0, "y": 300.0, "width": 80.0, "height": 40.0 },
                        { "op": "connect", "as": "moved", "from": "@other", "to": "@far" }
                    ]
                }),
            )
            .expect("setup");
        let id = |name: &str| NodeId(setup["name_to_id"][name].as_u64().unwrap());
        let (frame, boxed, moved) = (id("frame"), id("box"), id("moved"));
        let rotated = designos_doc::Affine::translate(100.0, 200.0) * designos_doc::Affine::rotate(0.6);
        engine.doc.get_mut(frame).unwrap().transform = rotated;
        let page_point = designos_doc::Vec2::new(350.0, 250.0);
        let world = engine.doc.world_transform(frame);

        // Create path.
        let created = engine
            .call(
                "designos_apply_batch",
                json!({ "ops": [{ "op": "connect", "as": "made", "from": boxed.0, "to": { "point": [page_point.x, page_point.y] } }] }),
            )
            .expect("connect with a free end in a rotated frame");
        let made = NodeId(created["name_to_id"]["made"].as_u64().unwrap());
        assert_eq!(engine.doc.get(made).unwrap().parent, Some(frame));
        let back = world.apply(connector_ends(&engine, made).1.point);
        assert!((back.x - page_point.x).abs() < 0.01 && (back.y - page_point.y).abs() < 0.01, "created free end maps back to {page_point:?}, got {back:?}");

        // Update path: the connector is re-homed into the rotated frame and its free end is converted.
        engine
            .call(
                "designos_apply_batch",
                json!({ "ops": [{ "op": "connect", "node": moved.0, "from": boxed.0, "to": { "point": [page_point.x, page_point.y] } }] }),
            )
            .expect("retarget into a rotated frame");
        assert_eq!(engine.doc.get(moved).unwrap().parent, Some(frame));
        let back = world.apply(connector_ends(&engine, moved).1.point);
        assert!((back.x - page_point.x).abs() < 0.01 && (back.y - page_point.y).abs() < 0.01, "updated free end maps back to {page_point:?}, got {back:?}");
        assert_connector_matches_its_geometry(&engine, moved);
    }

    #[test]
    fn mcp_connect_update_that_rehomes_keeps_a_free_end_at_its_page_point() {
        let mut engine = Engine::default();
        let page = engine.resolve_default_page().unwrap();
        let setup = engine
            .call(
                "designos_apply_batch",
                json!({
                    "ops": [
                        { "op": "insert", "as": "frame", "parent": page.0, "kind": "frame", "name": "Tilted", "x": 100.0, "y": 200.0, "width": 400.0, "height": 300.0 },
                        { "op": "insert", "as": "box", "parent": "@frame", "kind": "rectangle", "name": "Box", "x": 20.0, "y": 30.0, "width": 80.0, "height": 40.0 },
                        { "op": "insert", "as": "other", "parent": page.0, "kind": "rectangle", "name": "Other", "x": 900.0, "y": 0.0, "width": 80.0, "height": 40.0 },
                        { "op": "connect", "as": "link", "from": "@other", "to": { "point": [350.0, 250.0] } }
                    ]
                }),
            )
            .expect("setup");
        let id = |name: &str| NodeId(setup["name_to_id"][name].as_u64().unwrap());
        let (frame, boxed, link) = (id("frame"), id("box"), id("link"));
        assert_eq!(engine.doc.get(link).unwrap().parent, Some(page));
        engine.doc.get_mut(frame).unwrap().transform = designos_doc::Affine::translate(100.0, 200.0) * designos_doc::Affine::rotate(0.6);

        engine
            .call("designos_apply_batch", json!({ "ops": [{ "op": "connect", "node": link.0, "from": boxed.0 }] }))
            .expect("attach the start into the rotated frame, leaving the free end alone");

        assert_eq!(engine.doc.get(link).unwrap().parent, Some(frame));
        let end = connector_ends(&engine, link).1;
        assert_eq!(end.node, None, "the free end stays free");
        let back = engine.doc.world_transform(frame).apply(end.point);
        assert!((back.x - 350.0).abs() < 0.01 && (back.y - 250.0).abs() < 0.01, "the free end must stay at its page point, got {back:?}");
        assert_connector_matches_its_geometry(&engine, link);
    }

    #[test]
    fn mcp_moving_a_connected_node_in_a_later_batch_reroutes_the_connector() {
        let mut engine = Engine::default();
        let page = engine.resolve_default_page().unwrap();
        let (_a, b, link) = page_connector_between_two_boxes(&mut engine, page);
        let transform_before = engine.doc.get(link).unwrap().transform;
        let size_before = engine.doc.get(link).unwrap().size;

        engine.call("designos_apply_batch", json!({ "ops": [{ "op": "update", "node": b.0, "x": 660.0 }] })).expect("move the end node");

        let conn = engine.doc.get(link).unwrap();
        assert!(conn.transform != transform_before || conn.size != size_before, "moving an end node must move or resize the connector");
        assert_connector_matches_its_geometry(&engine, link);
    }

    #[test]
    fn mcp_connect_then_move_in_one_batch_leaves_the_connector_on_its_route_and_undoes_together() {
        let mut engine = Engine::default();
        let page = engine.resolve_default_page().unwrap();
        let setup = engine
            .call(
                "designos_apply_batch",
                json!({
                    "ops": [
                        { "op": "insert", "as": "a", "parent": page.0, "kind": "rectangle", "name": "A", "x": 0.0, "y": 0.0, "width": 80.0, "height": 50.0 },
                        { "op": "insert", "as": "b", "parent": page.0, "kind": "rectangle", "name": "B", "x": 200.0, "y": 0.0, "width": 80.0, "height": 50.0 }
                    ]
                }),
            )
            .expect("two boxes");
        let a = NodeId(setup["name_to_id"]["a"].as_u64().unwrap());
        let b = NodeId(setup["name_to_id"]["b"].as_u64().unwrap());
        let pre_nodes = engine.doc.nodes.clone();
        let depth_before = engine.history.undo_depth();

        let res = engine
            .call(
                "designos_apply_batch",
                json!({
                    "ops": [
                        { "op": "connect", "as": "link", "from": a.0, "to": b.0 },
                        { "op": "update", "node": b.0, "x": 660.0 }
                    ]
                }),
            )
            .expect("connect then move in one batch");
        let link = NodeId(res["name_to_id"]["link"].as_u64().unwrap());

        assert_eq!(engine.history.undo_depth(), depth_before + 1, "connect, move and the re-route must be one undo step");
        assert_eq!(engine.doc.get(b).unwrap().transform.translation().x, 660.0);
        assert_connector_matches_its_geometry(&engine, link);

        engine.call("designos_undo", json!({})).expect("single undo");
        assert!(engine.doc.get(link).is_none(), "one undo must remove the connector");
        assert_eq!(engine.doc.nodes, pre_nodes, "one undo must also put the moved node back");
    }

    #[test]
    fn mcp_export_spec_of_a_connector_returns_no_spec_and_warns() {
        let mut engine = Engine::default();
        let page = engine.resolve_default_page().unwrap();
        let (_a, _b, link) = page_connector_between_two_boxes(&mut engine, page);

        let exported = engine.call("designos_export_spec", json!({ "node_id": link.0 })).expect("export_spec of a connector");

        assert!(exported["spec"].is_null(), "a connector must not be exported as a spec: {exported}");
        assert_eq!(exported["node_id"], link.0);
        let warnings = exported["warnings"].as_array().expect("warnings array");
        assert_eq!(warnings.len(), 1, "{exported}");
        assert_eq!(warnings[0]["code"], "CONNECTORS_SKIPPED");
        assert_eq!(warnings[0]["skipped"], 1);
    }

    #[test]
    fn mcp_export_spec_replies_validate_against_the_published_output_schema() {
        let tool = crate::engine::tools().into_iter().find(|t| t["name"] == "designos_export_spec").expect("designos_export_spec is declared");
        let validator = jsonschema::validator_for(&tool["outputSchema"]).expect("compile the export_spec output schema");

        let mut engine = Engine::default();
        let page = engine.resolve_default_page().unwrap();
        let (a, _b, link) = page_connector_between_two_boxes(&mut engine, page);
        for (what, node) in [("a connector root", link), ("a layer root", a)] {
            let reply = engine.call("designos_export_spec", json!({ "node_id": node.0 })).unwrap_or_else(|e| panic!("export_spec of {what}: {e:?}"));
            if let Err(e) = validator.validate(&reply) {
                panic!("the reply for {what} does not match the published output schema: {e}\n{reply}");
            }
        }
    }

    #[test]
    fn mcp_export_spec_description_says_what_a_connector_root_returns() {
        let tool = crate::engine::tools().into_iter().find(|t| t["name"] == "designos_export_spec").expect("designos_export_spec is declared");
        let description = tool["description"].as_str().expect("description");
        assert!(
            description.contains("null spec") && description.contains("CONNECTORS_SKIPPED"),
            "the description must say what a connector root returns: {description}"
        );
    }

    #[test]
    fn mcp_unknown_batch_op_lists_connect_among_the_valid_options() {
        let mut engine = Engine::default();
        let err = engine.call("designos_apply_batch", json!({ "ops": [{ "op": "conect" }] })).expect_err("unknown op must be refused");
        let json = err.to_json_value();
        let options = json["valid_options"].as_array().unwrap_or_else(|| panic!("valid_options expected: {json}"));
        assert!(options.iter().any(|o| o == "connect"), "valid_options must list connect: {json}");
    }

    #[test]
    fn mcp_set_style_keeps_text_layer_overrides_clears_them_on_reapply_and_undoes_cleanly() {
        let mut engine = Engine::default();
        let page = engine.resolve_default_page().unwrap();
        let ts_16 = serde_json::to_value(TextStyle { font_size: 16.0, font_weight: 400, ..Default::default() }).unwrap();
        let ts_24 = serde_json::to_value(TextStyle { font_size: 24.0, font_weight: 500, ..Default::default() }).unwrap();
        let setup = engine
            .call(
                "designos_apply_batch",
                json!({
                    "ops": [
                        {
                            "op": "set_style",
                            "upsert": [{
                                "key": "Body",
                                "name": "Body",
                                "text_style": ts_16
                            }]
                        },
                        { "op": "insert", "as": "plain", "parent": page.0, "kind": "text", "name": "Plain", "text": "Hello world", "x": 0.0, "y": 0.0 },
                        { "op": "insert", "as": "over", "parent": page.0, "kind": "text", "name": "Over", "text": "Hello world", "x": 0.0, "y": 40.0 },
                        {
                            "op": "set_style",
                            "apply": [
                                { "node": "@plain", "style": "Body" },
                                { "node": "@over", "style": "Body" }
                            ]
                        }
                    ]
                }),
            )
            .expect("setup linked text layers");
        let plain = NodeId(setup["name_to_id"]["plain"].as_u64().unwrap());
        let over = NodeId(setup["name_to_id"]["over"].as_u64().unwrap());

        // Give `over` a layer-wide weight override, a range italic override, and a hyperlink.
        {
            let n = engine.doc.get_mut(over).unwrap();
            if let NodeKind::Text(tp) = &mut n.kind {
                tp.style.set_font_weight(700);
                tp.runs.push(designos_doc::TextRun { start: 0, end: 5, style: TextStyle { italic: true, ..tp.style.clone() } });
            }
            n.text_style_overrides.insert(designos_doc::OwnedField::Weight);
            n.hyperlink = Some("https://example.com".into());
        }
        let before_plain = engine.doc.get(plain).unwrap().clone();
        let before_over = engine.doc.get(over).unwrap().clone();

        // 1. Editing the shared text style via `set_style` upsert keeps overrides and updates non-overridden properties.
        engine
            .call(
                "designos_apply_batch",
                json!({
                    "ops": [{
                        "op": "set_style",
                        "upsert": [{
                            "key": "Body",
                            "name": "Body",
                            "text_style": ts_24
                        }]
                    }]
                }),
            )
            .expect("edit Body style");

        let plain_tp = engine.doc.get(plain).unwrap().kind.any_text().unwrap();
        assert_eq!((plain_tp.style.font_size, plain_tp.style.font_weight), (24.0, 500));
        let over_node = engine.doc.get(over).unwrap();
        let over_tp = over_node.kind.any_text().unwrap();
        assert_eq!((over_tp.style.font_size, over_tp.style.font_weight), (24.0, 700), "overridden weight stays while size updates");
        assert!(over_tp.runs[0].style.italic && over_tp.runs[0].style.font_size == 24.0, "range italic override stays while size updates");
        assert_eq!(over_node.hyperlink.as_deref(), Some("https://example.com"));

        // 2. Undo of the style edit restores every layer as it was.
        engine.call("designos_undo", json!({})).expect("undo style edit");
        assert_eq!(engine.doc.get(plain).unwrap(), &before_plain);
        assert_eq!(engine.doc.get(over).unwrap(), &before_over);
        engine.call("designos_redo", json!({})).expect("redo style edit");

        // 3. Updating a variable bound on the text style keeps the layer's overridden weight and range italic.
        engine.doc.variable_collections.insert(
            "typo".into(),
            designos_doc::VariableCollection {
                id: "typo".into(),
                name: "Typo".into(),
                modes: vec![designos_doc::VariableMode { id: "default".into(), name: "Default".into() }],
                default_mode: "default".into(),
                variables: vec!["w".into()],
                key: None,
                source_library_key: None,
                extensions: Default::default(),
            },
        );
        engine.doc.variables.insert(
            "w".into(),
            designos_doc::Variable {
                id: "w".into(),
                name: "Weight".into(),
                collection: "typo".into(),
                kind: designos_doc::VariableKind::Number,
                values: [("default".into(), designos_doc::VariableValue::Number(500.0))].into_iter().collect(),
                description: String::new(),
                scopes: Vec::new(),
                hidden_from_publishing: false,
                key: None,
                code_syntax: Default::default(),
                extensions: Default::default(),
                mode_extensions: Default::default(),
            },
        );
        engine.doc.style_meta.get_mut("Body").unwrap().bindings.insert("fontWeight".into(), "w".into());
        engine
            .call(
                "designos_apply_batch",
                json!({
                    "ops": [{
                        "op": "set_variables",
                        "upsert_variables": [{ "id": "w", "name": "Weight", "collection": "typo", "kind": "number", "values": { "default": 600.0 } }]
                    }]
                }),
            )
            .expect("update style weight variable");
        assert_eq!(engine.doc.get(plain).unwrap().kind.any_text().unwrap().style.font_weight, 600, "plain layer follows the style variable");
        assert_eq!(engine.doc.get(over).unwrap().kind.any_text().unwrap().style.font_weight, 700, "overridden layer keeps its weight");
        assert!(engine.doc.get(over).unwrap().kind.any_text().unwrap().runs[0].style.italic, "overridden layer keeps its italic range");

        // 4. Reapplying the style via `set_style` `apply` clears all overrides (values, range, binding, hyperlink).
        engine.doc.get_mut(over).unwrap().variables.bindings.insert("fontWeight".into(), "w".into());
        let before_reapply = engine.doc.get(over).unwrap().clone();
        engine
            .call("designos_apply_batch", json!({ "ops": [{ "op": "set_style", "apply": [{ "node": over.0, "style": "Body" }] }] }))
            .expect("reapply Body style");
        let reapplied = engine.doc.get(over).unwrap();
        assert!(designos_doc::text_style_overrides(&engine.doc, reapplied).is_empty(), "reapply clears overrides");
        assert!(reapplied.hyperlink.is_none(), "reapply clears hyperlink");
        assert!(!reapplied.variables.bindings.contains_key("fontWeight"), "reapply clears owned typography binding");
        assert!(!reapplied.kind.any_text().unwrap().runs[0].style.italic, "reapply clears range override");

        engine.call("designos_undo", json!({})).expect("undo reapply");
        assert_eq!(engine.doc.get(over).unwrap(), &before_reapply);
    }
}
