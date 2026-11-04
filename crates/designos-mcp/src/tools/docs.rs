//! `designos_docs_list` and `designos_docs_read`: read a library project's docs (`<library>/projects/<id>/docs/`).
//!
//! Read-only. Everything goes through `designos-docs`, so the pages an agent reads are the ones Studio shows, and a
//! symlink out of the docs folder is refused there. All doc text leaves through the untrusted-data fence in
//! `docs_fence`; kinds, counts and hashes stay plain.

use crate::engine::parse;
use crate::tools::docs_errors::docs_error;
use crate::tools::docs_fence::{fence, fence_capped};
use crate::tools::docs_project::resolve_project;
use crate::tools::errors::{ErrorCode, ToolError};
use designos_docs::{Block, DocsStore, Page};
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::Path;

pub use crate::tools::docs_descriptors::tool_descriptors;
pub use crate::tools::docs_list::docs_list;

/// The serialized size no MCP result may pass (the document-text limit of #1289).
pub const MAX_RESULT_BYTES: usize = 100_000;

/// Serialized bytes of one `designos_docs_read` result the blocks may fill. It is measured on the JSON, where quotes and
/// backslashes double, not on the Markdown, and leaves room under [`MAX_RESULT_BYTES`] for a wrapper's own fields.
pub const READ_BUDGET_BYTES: usize = 90_000;

/// Longest title, description and page-level text returned, so page-level fields cannot eat the block budget.
pub const MAX_META_CHARS: usize = 500;
/// Front-matter problems returned per read; the rest are counted in `front_matter_problems_total`.
const MAX_PROBLEMS: usize = 20;
const MAX_PROBLEM_CHARS: usize = 200;
/// Longest page path echoed back.
const MAX_PAGE_ECHO_CHARS: usize = 500;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReadArgs {
    page: String,
    project: Option<String>,
    range: Option<RangeArg>,
    /// The `page_hash` of the read this one continues. When given, it must still be the page's hash.
    page_hash: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RangeArg {
    start: Option<usize>,
    end: Option<usize>,
}

/// `designos_docs_read`: the blocks of one page, each with its index, kind, hash and Markdown.
pub fn docs_read(root: &Path, open_file: Option<&str>, args: Value) -> Result<Value, ToolError> {
    let a: ReadArgs = parse(args)?;
    let project = resolve_project(root, a.project.as_deref(), open_file)?;
    let store = DocsStore::for_project(root, &project.id).map_err(|e| docs_error("/project", e))?;
    let versioned = store.read_page(&a.page).map_err(|e| docs_error("/page", e))?;
    check_page_hash(a.page_hash.as_deref(), &versioned.hash.to_string())?;
    let page = &versioned.value;
    let total = page.blocks.len();
    let (start, end) = range_bounds(a.range.as_ref(), total)?;

    let front = page.front_matter();
    let all_problems = front.map(|f| f.problems()).unwrap_or_default();
    let problems: Vec<Value> = all_problems
        .iter()
        .take(MAX_PROBLEMS)
        .map(|p| json!({"field": p.field, "problem": fence_capped(&p.problem, MAX_PROBLEM_CHARS), "fix": p.fix}))
        .collect();
    let mut out = json!({
        "project": {"id": project.id, "name": fence(&project.name)},
        "page": fence_capped(&a.page, MAX_PAGE_ECHO_CHARS),
        "page_hash": versioned.hash.to_string(),
        "title": front.and_then(|f| f.title()).map(|t| fence_capped(t, MAX_META_CHARS)),
        "description": front.and_then(|f| f.description()).map(|t| fence_capped(t, MAX_META_CHARS)),
        "group": front.and_then(|f| f.group()).map(|g| g.as_str()),
        "front_matter_problems": problems,
        "front_matter_problems_total": all_problems.len(),
        "problems_truncated": all_problems.len() > MAX_PROBLEMS,
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

/// A continuation that names the `page_hash` it was made from is refused when the page has changed since: block indices
/// shift when text above them is edited, so a stale `next_start` would repeat or skip blocks.
fn check_page_hash(given: Option<&str>, current: &str) -> Result<(), ToolError> {
    let Some(given) = given else { return Ok(()) };
    let bad = |message: &str| {
        ToolError::new(ErrorCode::ValueOutOfRange, "/page_hash", message)
            .with_received(fence_capped(given, 200))
            .with_expected("the page_hash of the previous designos_docs_read call, or omit page_hash")
    };
    if given.len() != current.len() || !given.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')) {
        return Err(bad("page_hash is not a page_hash from designos_docs_read; fix: pass page_hash exactly as returned, or omit it"));
    }
    if given != current {
        return Err(ToolError::new(
            ErrorCode::CursorStale,
            "/page_hash",
            "the page changed since that page_hash; read it again from the start (omit range and page_hash)",
        )
        .with_received(fence_capped(given, 200))
        .with_next_step("Call designos_docs_read again without range and page_hash."));
    }
    Ok(())
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
