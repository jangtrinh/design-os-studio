//! `designos_docs_list`: the five groups with their pages, and the pinned frames, in pieces that fit one result.

use crate::engine::parse;
use crate::tools::docs::{MAX_META_CHARS, READ_BUDGET_BYTES};
use crate::tools::docs_errors::docs_error;
use crate::tools::docs_fence::{fence, fence_capped, file_id_value, node_id_value, MAX_ID_CHARS};
use crate::tools::docs_project::resolve_project;
use crate::tools::errors::{ErrorCode, ToolError};
use designos_docs::{DocsError, DocsStore, Manifest};
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::Path;

/// Longest pin name returned.
const MAX_PIN_NAME_CHARS: usize = 200;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ListArgs {
    project: Option<String>,
    cursor: Option<String>,
}

/// Characters of `manifest_hash` a cursor carries: enough that two manifests do not collide, short enough to read.
const CURSOR_HASH_CHARS: usize = 12;

/// `"<first 12 hex of manifest_hash>:<index>"`.
fn cursor_text(manifest_hash: &str, index: usize) -> String {
    format!("{}:{index}", &manifest_hash[..CURSOR_HASH_CHARS.min(manifest_hash.len())])
}

/// The position a cursor names, if it came from this manifest. Nothing of the cursor is echoed into a message.
fn cursor_position(cursor: &str, manifest_hash: Option<&str>) -> Result<usize, ToolError> {
    let bad = |message: &str| {
        ToolError::new(ErrorCode::ValueOutOfRange, "/cursor", message)
            .with_received(fence_capped(cursor, 200))
            .with_expected("the next_cursor of the previous designos_docs_list call, or omit cursor")
    };
    let parsed = cursor.split_once(':').filter(|(head, _)| head.len() == CURSOR_HASH_CHARS && head.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')));
    let Some((head, index)) = parsed else {
        return Err(bad("cursor is not a next_cursor from designos_docs_list; fix: pass next_cursor exactly as returned, or omit cursor"));
    };
    let Ok(index) = index.parse::<usize>() else {
        return Err(bad("cursor is not a next_cursor from designos_docs_list; fix: pass next_cursor exactly as returned, or omit cursor"));
    };
    if manifest_hash.is_none_or(|h| !h.starts_with(head)) {
        return Err(ToolError::new(
            ErrorCode::CursorStale,
            "/cursor",
            "cursor is from an earlier manifest: the manifest changed; list again from the start (omit cursor)",
        )
        .with_received(fence_capped(cursor, 200))
        .with_next_step("Call designos_docs_list again without a cursor."));
    }
    Ok(index)
}

/// `designos_docs_list`. The pages (in group order) and then the pins form one sequence; `cursor` is a position in it.
/// A call returns entries until the serialized result would pass [`READ_BUDGET_BYTES`], then says where to continue.
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
    let total_pages: usize = manifest.groups.iter().map(|g| g.pages.len()).sum();
    let total = total_pages + manifest.pins.len();
    let cursor = match a.cursor.as_deref() {
        None => 0,
        Some(text) => cursor_position(text, manifest_hash.as_deref())?,
    };
    if cursor > total {
        return Err(ToolError::new(ErrorCode::ValueOutOfRange, "/cursor", format!("cursor is past the end; the project has {total} pages and pins"))
            .with_received(fence_capped(a.cursor.as_deref().unwrap_or_default(), 200))
            .with_expected("the next_cursor of the previous designos_docs_list call, or omit cursor"));
    }

    let mut out = json!({
        "project": {"id": project.id, "name": fence(&project.name)},
        "has_manifest": manifest_hash.is_some(),
        "manifest_hash": manifest_hash,
        "total_pages": total_pages,
        "total_pins": manifest.pins.len(),
        "truncated": false,
        "next_cursor": Value::Null,
        "groups": [],
        "pins": [],
    });
    let mut left = READ_BUDGET_BYTES.saturating_sub(serde_json::to_string(&out).map_or(READ_BUDGET_BYTES, |s| s.len()) + 256);
    let mut next = None;
    let mut groups: Vec<Value> = manifest.groups.iter().map(|g| json!({"id": g.id.as_str(), "pages": []})).collect();
    let mut pins: Vec<Value> = Vec::new();
    let mut returned = 0usize;

    // `fits` is false once an entry would pass the budget, except that the first entry always goes in (progress).
    let mut fits = |entry: &Value| -> bool {
        let size = serde_json::to_string(entry).map_or(usize::MAX, |s| s.len() + 1);
        if returned > 0 && size > left {
            return false;
        }
        left = left.saturating_sub(size);
        returned += 1;
        true
    };
    let mut index = 0usize;
    'pages: for (g, group) in manifest.groups.iter().enumerate() {
        for rel in &group.pages {
            if index >= cursor {
                let entry = page_entry(&store, rel);
                if !fits(&entry) {
                    next = Some(index);
                    break 'pages;
                }
                groups[g]["pages"].as_array_mut().expect("pages array").push(entry);
            }
            index += 1;
        }
    }
    if next.is_none() {
        for pin in &manifest.pins {
            if index >= cursor {
                let entry = pin_entry(pin);
                if !fits(&entry) {
                    next = Some(index);
                    break;
                }
                pins.push(entry);
            }
            index += 1;
        }
    }
    out["groups"] = Value::Array(groups);
    out["pins"] = Value::Array(pins);
    out["truncated"] = json!(next.is_some());
    out["next_cursor"] = json!(next.zip(manifest_hash.as_deref()).map(|(index, hash)| cursor_text(hash, index)));
    Ok(out)
}

fn pin_entry(p: &designos_docs::Pin) -> Value {
    json!({
        "id": fence_capped(&p.id, MAX_ID_CHARS),
        "file_id": file_id_value(&p.file_id),
        "node_id": node_id_value(&p.node_id),
        "name": fence_capped(&p.name, MAX_PIN_NAME_CHARS),
        "group": p.group.as_str(),
        "parent_screen": p.parent_screen.as_deref().map(|s| fence_capped(s, MAX_ID_CHARS)),
    })
}

/// One listed page: its title and size, or why it cannot be read (the listing never fails for one bad page).
///
/// Every listed page is read whole. `DocsStore` has no partial read, and a prefix read made here would skip its
/// symlink checks; a page is capped at 8 MiB. `blocks` and `hash` need the whole page anyway. Pages past the budget
/// are not read at all: the cursor stops the loop first.
fn page_entry(store: &DocsStore, rel: &str) -> Value {
    match store.read_page(rel) {
        Ok(v) => {
            let page = &v.value;
            let title = page.front_matter().and_then(|f| f.title()).map(str::to_string).or_else(|| page.headings().into_iter().next().map(|h| h.text));
            json!({
                "path": fence(rel),
                "title": title.as_deref().map(|t| fence_capped(t, MAX_META_CHARS)),
                "blocks": page.blocks.len(),
                "front_matter_problems": page.front_matter().map_or(0, |f| f.problems().len()),
                "hash": v.hash.to_string(),
            })
        }
        Err(e) => json!({"path": fence(rel), "error": fence_capped(&e.to_string(), MAX_META_CHARS)}),
    }
}
