//! `designos_docs_list` and `designos_docs_read`: read a library project's docs (`<library>/projects/<id>/docs/`).
//!
//! Read-only. Everything goes through `designos-docs`, so the pages an agent reads are the ones Studio shows. All doc
//! text leaves through the untrusted-data fence in `docs_fence`; ids, paths, kinds, counts and hashes stay plain.

use crate::engine::parse;
use crate::tools::docs_fence::{fence, fence_capped, fence_opt, fenced_or_null_schema, fenced_schema, RULE};
use crate::tools::docs_project::resolve_project;
use crate::tools::errors::{tool_output_schema, ErrorCode, ToolError};
use designos_docs::{Block, DocsError, DocsStore, Manifest, Page};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::path::Path;

/// Most characters of block Markdown one `designos_docs_read` returns. The rest is read with `range.start = next_start`.
pub const MAX_READ_CHARS: usize = 100_000;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ListArgs {
    project: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReadArgs {
    page: String,
    project: Option<String>,
    range: Option<RangeArg>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RangeArg {
    start: Option<usize>,
    end: Option<usize>,
}

/// `designos_docs_list`: the five groups with their pages, and the pinned frames.
pub fn docs_list(root: &Path, open_file: Option<&str>, args: Value) -> Result<Value, ToolError> {
    let a: ListArgs = parse(args)?;
    let project = resolve_project(root, a.project.as_deref(), open_file)?;
    let store = DocsStore::for_project(root, &project.id).map_err(|e| docs_error(None, "/project", e))?;
    // A project that has no manifest yet has no docs: five empty groups, nothing is created.
    let (manifest, manifest_hash) = match store.read_manifest() {
        Ok(v) => (v.value, Some(v.hash.to_string())),
        Err(DocsError::NotFound { .. }) => (Manifest::new(), None),
        Err(e) => return Err(docs_error(Some(&store), "", e)),
    };
    let groups: Vec<Value> =
        manifest.groups.iter().map(|g| json!({"id": g.id.as_str(), "pages": g.pages.iter().map(|rel| page_entry(&store, rel)).collect::<Vec<_>>()})).collect();
    let pins: Vec<Value> = manifest
        .pins
        .iter()
        .map(|p| {
            json!({"id": p.id, "file_id": p.file_id, "node_id": p.node_id, "name": fence(&p.name), "group": p.group.as_str(), "parent_screen": p.parent_screen})
        })
        .collect();
    Ok(json!({
        "project": {"id": project.id, "name": fence(&project.name)},
        "has_manifest": manifest_hash.is_some(),
        "manifest_hash": manifest_hash,
        "groups": groups,
        "pins": pins,
    }))
}

/// One listed page: its title and size, or why it cannot be read (the listing never fails for one bad page).
fn page_entry(store: &DocsStore, rel: &str) -> Value {
    match store.read_page(rel) {
        Ok(v) => {
            let page = &v.value;
            let title = page.front_matter().and_then(|f| f.title()).map(str::to_string).or_else(|| page.headings().into_iter().next().map(|h| h.text));
            json!({
                "path": rel,
                "title": fence_opt(title.as_deref()),
                "blocks": page.blocks.len(),
                "front_matter_problems": page.front_matter().map_or(0, |f| f.problems().len()),
                "hash": v.hash.to_string(),
            })
        }
        Err(e) => json!({"path": rel, "error": e.to_string()}),
    }
}

/// `designos_docs_read`: the blocks of one page, each with its index, kind, hash and Markdown.
pub fn docs_read(root: &Path, open_file: Option<&str>, args: Value) -> Result<Value, ToolError> {
    let a: ReadArgs = parse(args)?;
    let project = resolve_project(root, a.project.as_deref(), open_file)?;
    let store = DocsStore::for_project(root, &project.id).map_err(|e| docs_error(None, "/project", e))?;
    let versioned = store.read_page(&a.page).map_err(|e| docs_error(Some(&store), "/page", e))?;
    let page = &versioned.value;
    let total = page.blocks.len();
    let (start, end) = range_bounds(a.range.as_ref(), total)?;

    let mut blocks = Vec::new();
    let mut used = 0usize;
    let mut next_start = None;
    for (index, block) in page.blocks.iter().enumerate().take(end).skip(start) {
        let markdown = block_markdown(block);
        let len = markdown.chars().count().min(MAX_READ_CHARS);
        if !blocks.is_empty() && used + len > MAX_READ_CHARS {
            next_start = Some(index);
            break;
        }
        used += len;
        blocks.push(json!({
            "index": index,
            "kind": block.kind().name(),
            "hash": block.hash().to_string(),
            "markdown": fence_capped(&markdown, MAX_READ_CHARS),
        }));
    }
    let returned_end = next_start.unwrap_or(end);

    let front = page.front_matter();
    let problems: Vec<Value> =
        front.map(|f| f.problems().iter().map(|p| json!({"field": p.field, "problem": fence(&p.problem), "fix": p.fix})).collect()).unwrap_or_default();
    Ok(json!({
        "project": {"id": project.id, "name": fence(&project.name)},
        "page": a.page,
        "page_hash": versioned.hash.to_string(),
        "title": fence_opt(front.and_then(|f| f.title())),
        "description": fence_opt(front.and_then(|f| f.description())),
        "group": front.and_then(|f| f.group()).map(|g| g.as_str()),
        "front_matter_problems": problems,
        "total_blocks": total,
        "range": {"start": start, "end": returned_end},
        "next_start": next_start,
        "blocks": blocks,
    }))
}

/// The Markdown of one block, exactly as the page file holds it (without the blank lines around it).
fn block_markdown(block: &Block) -> String {
    match block.source() {
        Some(source) => source.trim_end_matches(['\r', '\n']).to_string(),
        None => {
            let mut one = Page::default();
            one.push_block(block.kind().clone());
            one.render_canonical().trim_end_matches(['\r', '\n']).to_string()
        }
    }
}

/// `[start, end)` of the blocks to return. `end` past the last block is clamped; a start outside the page is an error.
fn range_bounds(range: Option<&RangeArg>, total: usize) -> Result<(usize, usize), ToolError> {
    let start = range.and_then(|r| r.start).unwrap_or(0);
    let end = range.and_then(|r| r.end).unwrap_or(total).min(total);
    let bad = |path: &str, message: String, received: usize| {
        ToolError::new(ErrorCode::ValueOutOfRange, path, message).with_received(json!(received)).with_expected(format!("block indices 0..{total}; start < end"))
    };
    if total > 0 && start >= total {
        return Err(bad("/range/start", format!("start {start} is past the last block; the page has {total} blocks (indices 0 to {})", total - 1), start));
    }
    if end < start || (total > 0 && end == start) {
        return Err(bad("/range/end", format!("end {end} must be greater than start {start}; end is exclusive"), end));
    }
    Ok((start, end))
}

/// Turns a `designos-docs` failure into a tool error that names the fix. `path` is the argument that caused it; `store`
/// lets an unknown page list the pages that exist.
fn docs_error(store: Option<&DocsStore>, path: &str, err: DocsError) -> ToolError {
    match err {
        DocsError::NotFound { .. } if path == "/page" => {
            let mut known: BTreeSet<String> = BTreeSet::new();
            if let Some(store) = store {
                if let Ok(m) = store.read_manifest() {
                    known.extend(m.value.groups.iter().flat_map(|g| g.pages.iter().cloned()));
                }
                known.extend(store.list_page_files().unwrap_or_default());
            }
            ToolError::new(
                ErrorCode::InvalidArguments,
                path,
                "no such page in this project's docs; fix: pass a page path from valid_options, or call designos_docs_list",
            )
            .with_expected("a page path such as spec/checkout.md")
            .with_valid_options(known)
        }
        DocsError::InvalidPath { .. } => {
            ToolError::new(ErrorCode::InvalidArguments, path, err.to_string()).with_expected("a page path such as spec/checkout.md")
        }
        // The store refuses a file over its size cap with this wording (`read_limited`); a test pins it.
        DocsError::Io { ref source, .. } if source.kind() == std::io::ErrorKind::InvalidData && source.to_string().contains("byte limit") => {
            ToolError::new(ErrorCode::FileTooLarge, path, err.to_string()).with_next_step(
                "This page is over the 8 MiB limit and cannot be read. Tell the user to split it into smaller pages in Studio, or read another page.",
            )
        }
        DocsError::Io { ref source, .. } if source.kind() == std::io::ErrorKind::InvalidData => {
            ToolError::new(ErrorCode::DocumentInvalid, path, err.to_string())
        }
        DocsError::Manifest(_) => ToolError::new(ErrorCode::DocumentInvalid, path, err.to_string())
            .with_next_step("Stop and tell the user: docs-manifest.json is invalid; the message says which field and how to repair it."),
        other => ToolError::new(ErrorCode::InternalError, path, other.to_string()),
    }
}

/// The two tool descriptors, in the registry's JSON shape.
pub fn tool_descriptors() -> Vec<Value> {
    let project = json!({
        "type": "string",
        "description": "Project id, or the exact name of one project. Omit it when the open file is in a project or the library has only one."
    });
    let read_only_note = format!(" Read-only. {RULE}");
    vec![
        json!({
            "name": "designos_docs_list",
            "description": format!("List a project's docs: groups, pages and pinned frames. The groups are overview, spec, screens, states and discovery, in tree order.{read_only_note}"),
            "inputSchema": {"type": "object", "properties": {"project": project}, "required": [], "additionalProperties": false},
            "outputSchema": tool_output_schema(
                json!({
                    "project": {"type": "object"},
                    "has_manifest": {"type": "boolean"},
                    "manifest_hash": {"type": ["string", "null"]},
                    "groups": {"type": "array", "items": {"type": "object", "properties": {
                        "id": {"type": "string"},
                        "pages": {"type": "array", "items": {"type": "object", "properties": {
                            "path": {"type": "string"},
                            "title": fenced_or_null_schema(),
                            "blocks": {"type": "integer"},
                            "front_matter_problems": {"type": "integer"},
                            "hash": {"type": "string"},
                            "error": {"type": "string"}
                        }}}
                    }}},
                    "pins": {"type": "array", "items": {"type": "object", "properties": {
                        "id": {"type": "string"}, "file_id": {"type": "string"}, "node_id": {"type": "string"},
                        "name": fenced_schema(), "group": {"type": "string"}, "parent_screen": {"type": ["string", "null"]}
                    }}}
                }),
                &["project", "has_manifest", "groups", "pins"],
            ),
        }),
        json!({
            "name": "designos_docs_read",
            "description": format!("Read one doc page as blocks (index, kind, hash, Markdown). The index is the block's id. The page also has a hash, title, description, group and any front-matter problems. Blocks of kind raw hold Markdown Studio does not model, comments included. At most {MAX_READ_CHARS} characters come back; next_start says where to continue.{read_only_note}"),
            "inputSchema": {
                "type": "object",
                "properties": {
                    "page": {"type": "string", "minLength": 1, "maxLength": 512, "description": "Page path from designos_docs_list, such as spec/checkout.md."},
                    "project": project,
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
                    "page": {"type": "string"},
                    "page_hash": {"type": "string"},
                    "title": fenced_or_null_schema(),
                    "description": fenced_or_null_schema(),
                    "group": {"type": ["string", "null"]},
                    "front_matter_problems": {"type": "array", "items": {"type": "object", "properties": {
                        "field": {"type": "string"}, "problem": fenced_schema(), "fix": {"type": "string"}
                    }}},
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
