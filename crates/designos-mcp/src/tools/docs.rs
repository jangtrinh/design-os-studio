//! `designos_docs_list` and `designos_docs_read`: read a library project's docs (`<library>/projects/<id>/docs/`).
//!
//! Read-only. Everything goes through `designos-docs`, so the pages an agent reads are the ones Studio shows, and a
//! symlink out of the docs folder is refused there. All doc text leaves through the untrusted-data fence in
//! `docs_fence`; kinds, counts and hashes stay plain.

use crate::engine::parse;
use crate::tools::docs_errors::docs_error;
use crate::tools::docs_fence::{fence, fence_capped, id_value, path_value};
use crate::tools::docs_project::resolve_project;
use crate::tools::errors::{ErrorCode, ToolError};
use designos_docs::{Block, DocsError, DocsStore, Manifest, Page};
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::Path;

pub use crate::tools::docs_descriptors::tool_descriptors;

/// The serialized size no MCP result may pass (the document-text limit of #1289).
pub const MAX_RESULT_BYTES: usize = 100_000;

/// Serialized bytes of one `designos_docs_read` result the blocks may fill. It is measured on the JSON, where quotes and
/// backslashes double, not on the Markdown, and leaves room under [`MAX_RESULT_BYTES`] for a wrapper's own fields.
pub const READ_BUDGET_BYTES: usize = 90_000;

/// Longest title and description returned, so page-level fields cannot eat the block budget.
const MAX_META_CHARS: usize = 2_000;
const MAX_PROBLEM_CHARS: usize = 500;

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
    let store = DocsStore::for_project(root, &project.id).map_err(|e| docs_error("/project", e))?;
    // A project that has no manifest yet has no docs: five empty groups, nothing is created.
    let (manifest, manifest_hash) = match store.read_manifest() {
        Ok(v) => (v.value, Some(v.hash.to_string())),
        Err(DocsError::NotFound { .. }) => (Manifest::new(), None),
        Err(e) => return Err(docs_error("", e)),
    };
    let groups: Vec<Value> =
        manifest.groups.iter().map(|g| json!({"id": g.id.as_str(), "pages": g.pages.iter().map(|rel| page_entry(&store, rel)).collect::<Vec<_>>()})).collect();
    let pins: Vec<Value> = manifest
        .pins
        .iter()
        .map(|p| {
            json!({
                "id": id_value(&p.id),
                "file_id": id_value(&p.file_id),
                "node_id": id_value(&p.node_id),
                "name": fence(&p.name),
                "group": p.group.as_str(),
                "parent_screen": p.parent_screen.as_deref().map(id_value),
            })
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
///
/// Every listed page is read whole. `DocsStore` has no partial read, and a prefix read made here would skip its
/// symlink check; a page is capped at 8 MiB. `blocks` and `hash` need the whole page anyway.
fn page_entry(store: &DocsStore, rel: &str) -> Value {
    match store.read_page(rel) {
        Ok(v) => {
            let page = &v.value;
            let title = page.front_matter().and_then(|f| f.title()).map(str::to_string).or_else(|| page.headings().into_iter().next().map(|h| h.text));
            json!({
                "path": path_value(rel),
                "title": title.as_deref().map(|t| fence_capped(t, MAX_META_CHARS)),
                "blocks": page.blocks.len(),
                "front_matter_problems": page.front_matter().map_or(0, |f| f.problems().len()),
                "hash": v.hash.to_string(),
            })
        }
        Err(e) => json!({"path": path_value(rel), "error": fence(&e.to_string())}),
    }
}

/// `designos_docs_read`: the blocks of one page, each with its index, kind, hash and Markdown.
pub fn docs_read(root: &Path, open_file: Option<&str>, args: Value) -> Result<Value, ToolError> {
    let a: ReadArgs = parse(args)?;
    let project = resolve_project(root, a.project.as_deref(), open_file)?;
    let store = DocsStore::for_project(root, &project.id).map_err(|e| docs_error("/project", e))?;
    let versioned = store.read_page(&a.page).map_err(|e| docs_error("/page", e))?;
    let page = &versioned.value;
    let total = page.blocks.len();
    let (start, end) = range_bounds(a.range.as_ref(), total)?;

    let front = page.front_matter();
    let problems: Vec<Value> = front
        .map(|f| f.problems().iter().map(|p| json!({"field": p.field, "problem": fence_capped(&p.problem, MAX_PROBLEM_CHARS), "fix": p.fix})).collect())
        .unwrap_or_default();
    let mut out = json!({
        "project": {"id": project.id, "name": fence(&project.name)},
        "page": path_value(&a.page),
        "page_hash": versioned.hash.to_string(),
        "title": front.and_then(|f| f.title()).map(|t| fence_capped(t, MAX_META_CHARS)),
        "description": front.and_then(|f| f.description()).map(|t| fence_capped(t, MAX_META_CHARS)),
        "group": front.and_then(|f| f.group()).map(|g| g.as_str()),
        "front_matter_problems": problems,
        "total_blocks": total,
        "range": {"start": start, "end": end},
        "next_start": Value::Null,
        "blocks": [],
    });
    // The fields above are measured, with room for the range and cursor numbers, before the blocks get the rest.
    let budget = READ_BUDGET_BYTES.saturating_sub(serde_json::to_string(&out).map_or(READ_BUDGET_BYTES, |s| s.len()) + 64);

    let mut blocks = Vec::new();
    let mut used = 0usize;
    let mut next_start = None;
    for (index, block) in page.blocks.iter().enumerate().take(end).skip(start) {
        let remaining = budget.saturating_sub(used);
        let entry = block_entry(index, block, budget);
        let size = serde_json::to_string(&entry).map_or(usize::MAX, |s| s.len() + 1);
        // A block that does not fit whole waits for the next call, unless it is the first: then it is cut to fit.
        if !blocks.is_empty() && size > remaining {
            next_start = Some(index);
            break;
        }
        used += size;
        blocks.push(entry);
    }
    out["range"]["end"] = json!(next_start.unwrap_or(end));
    out["next_start"] = json!(next_start);
    out["blocks"] = Value::Array(blocks);
    Ok(out)
}

/// One block as a result entry, its Markdown cut (and flagged) only if the entry cannot fit in `budget` bytes.
fn block_entry(index: usize, block: &Block, budget: usize) -> Value {
    let markdown = block_markdown(block);
    let entry = |text: Value| json!({"index": index, "kind": block.kind().name(), "hash": block.hash().to_string(), "markdown": text});
    // Nothing past `budget` characters can fit, so a huge block is not serialized whole just to be measured.
    let mut keep = markdown.chars().count().min(budget);
    loop {
        let candidate = entry(fence_capped(&markdown, keep));
        let size = serde_json::to_string(&candidate).map_or(usize::MAX, |s| s.len() + 1);
        if size <= budget || keep == 0 {
            return candidate;
        }
        // Escapes make the JSON longer than the text: shrink in proportion to the overshoot, and always by at least one.
        keep = ((keep as f64 * budget as f64 / size as f64) * 0.97) as usize;
        keep = keep.min(markdown.chars().count().saturating_sub(1));
    }
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
    let end_arg = range.and_then(|r| r.end);
    let bad = |path: &str, message: String, received: usize| {
        ToolError::new(ErrorCode::ValueOutOfRange, path, message).with_received(json!(received)).with_expected(format!("block indices 0..{total}; start < end"))
    };
    if end_arg == Some(0) {
        return Err(bad("/range/end", "end must be at least 1; end is exclusive".into(), 0));
    }
    if start > 0 && start >= total {
        let have = if total == 0 {
            "the page has no blocks, so start must be 0".to_string()
        } else {
            format!("the page has {total} blocks (indices 0 to {})", total - 1)
        };
        return Err(bad("/range/start", format!("start {start} is past the last block; {have}"), start));
    }
    let end = end_arg.unwrap_or(total).min(total);
    if total > 0 && end <= start {
        return Err(bad("/range/end", format!("end {end} must be greater than start {start}; end is exclusive"), end));
    }
    Ok((start, end))
}
