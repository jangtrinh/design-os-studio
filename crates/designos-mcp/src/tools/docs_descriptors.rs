//! MCP descriptors of `designos_docs_list` and `designos_docs_read`.

use crate::tools::docs_fence::{fenced_or_null_schema, fenced_schema, string_or_fenced_schema, RULE};
use crate::tools::errors::tool_output_schema;
use serde_json::{json, Value};

/// The two tool descriptors, in the registry's JSON shape.
pub fn tool_descriptors() -> Vec<Value> {
    let project = json!({
        "type": "string",
        "description": "Project id, or the exact name of one project. Omit it when the open file is in a project or the library has only one."
    });
    let read_only_note = format!(" Read-only. {RULE}");
    let budget = crate::tools::docs::READ_BUDGET_BYTES;
    vec![
        json!({
            "name": "designos_docs_list",
            "description": format!("List doc pages and pins. Pages come grouped as overview, spec, screens, states and discovery. Every page path comes back fenced; pass the string inside `value` to designos_docs_read. At most ~{budget} serialized bytes come back; next_cursor says where to continue.{read_only_note}"),
            "inputSchema": {"type": "object", "properties": {"project": project, "cursor": {"type": "string", "maxLength": 64, "description": "Where to continue: the next_cursor of the previous call, unchanged. Omit it for the first call. If the manifest changed since, it is refused: list again."}}, "required": [], "additionalProperties": false},
            "outputSchema": tool_output_schema(
                json!({
                    "project": {"type": "object"},
                    "has_manifest": {"type": "boolean"},
                    "manifest_hash": {"type": ["string", "null"]},
                    "total_pages": {"type": "integer"},
                    "total_pins": {"type": "integer"},
                    "truncated": {"type": "boolean"},
                    "next_cursor": {"type": ["string", "null"]},
                    "groups": {"type": "array", "items": {"type": "object", "properties": {
                        "id": {"type": "string"},
                        "pages": {"type": "array", "items": {"type": "object", "properties": {
                            "path": fenced_schema(),
                            "title": fenced_or_null_schema(),
                            "blocks": {"type": "integer"},
                            "front_matter_problems": {"type": "integer"},
                            "hash": {"type": "string"},
                            "error": fenced_schema()
                        }}}
                    }}},
                    "pins": {"type": "array", "items": {"type": "object", "properties": {
                        "id": fenced_schema(), "file_id": string_or_fenced_schema(), "node_id": string_or_fenced_schema(),
                        "name": fenced_schema(), "group": {"type": "string"},
                        "parent_screen": fenced_or_null_schema()
                    }}}
                }),
                &["project", "has_manifest", "groups", "pins"],
            ),
        }),
        json!({
            "name": "designos_docs_read",
            "description": format!("Read a doc page as blocks. Each has an index, kind, hash and Markdown. The index is the block's id. The page also has a hash, title, description, group and any front-matter problems. Blocks of kind raw hold Markdown Studio does not model, comments included. About {budget} serialized bytes come back; next_start says where to continue: pass it as range.start with the same page_hash. A single block over that is cut and flagged truncated, and its rest cannot be read here: ask the user to split it in Studio.{read_only_note}"),
            "inputSchema": {
                "type": "object",
                "properties": {
                    "page": {"type": "string", "minLength": 1, "maxLength": 512, "description": "Page path from designos_docs_list, such as spec/checkout.md. The listing fences every path: pass exactly the string inside `value`."},
                    "project": project,
                    "page_hash": {"type": "string", "maxLength": 128, "description": "The page_hash of the read you are continuing. If the page changed since, the call is refused: read it again from the start."},
                    "range": {
                        "type": "object",
                        "description": "Blocks to return: start (default 0) up to end (exclusive, default the last block).",
                        "properties": {"start": {"type": "integer", "minimum": 0}, "end": {"type": "integer", "minimum": 1}},
                        "additionalProperties": false
                    }
                },
                "required": ["page"],
                "additionalProperties": false
            },
            "outputSchema": tool_output_schema(
                json!({
                    "project": {"type": "object"},
                    "page": fenced_schema(),
                    "page_hash": {"type": "string"},
                    "title": fenced_or_null_schema(),
                    "description": fenced_or_null_schema(),
                    "group": {"type": ["string", "null"]},
                    "front_matter_problems": {"type": "array", "items": {"type": "object", "properties": {
                        "field": {"type": "string"}, "problem": fenced_schema(), "fix": {"type": "string"}
                    }}},
                    "front_matter_problems_total": {"type": "integer"},
                    "problems_truncated": {"type": "boolean"},
                    "total_blocks": {"type": "integer"},
                    "range": {"type": "object", "properties": {"start": {"type": "integer"}, "end": {"type": "integer"}}},
                    "next_start": {"type": ["integer", "null"]},
                    "blocks": {"type": "array", "items": {"type": "object", "properties": {
                        "index": {"type": "integer"}, "kind": {"type": "string"}, "hash": {"type": "string"}, "markdown": fenced_schema()
                    }}}
                }),
                &["project", "page", "page_hash", "front_matter_problems", "total_blocks", "range", "blocks"],
            ),
        }),
    ]
}
