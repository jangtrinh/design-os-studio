//! Figma clipboard payloads ("Paste from Figma").
//!
//! Figma.app puts `text/html` on the system clipboard whose body carries two
//! base64 comments: `(figmeta)` holds a small JSON object, `(figma)` holds a
//! fig-kiwi archive (the same container family as `.fig`) with the copied
//! `nodeChanges` and `blobs`. Browsers and the macOS pasteboard may escape the
//! comment markers (`&lt;!--(figma)` ... `(/figma)--&gt;`); both forms parse.
//!
//! - [`looks_like_figma_clipboard`]: cheap marker probe.
//! - [`parse_figma_clipboard_html`]: HTML -> [`FigClipboard`] (decoded bytes).
//! - [`import_figma_clipboard`]: [`FigClipboard`] -> [`Document`] holding the
//!   copied nodes as top-level children of its page.

use crate::container;
use crate::fig::message_to_rest;
use crate::kiwi::{KiwiValue, Schema};
use crate::rest_import;
use crate::FigError;
use base64::engine::general_purpose::{GeneralPurpose, GeneralPurposeConfig};
use base64::engine::DecodePaddingMode;
use base64::{alphabet, Engine};
use designos_doc::Document;
use std::sync::Arc;

/// Cap on the decoded `(figma)` buffer. Real copies are a few MB; the container
/// adds its own inflate budgets on top when the archive is opened.
const MAX_BUFFER_BYTES: usize = 256 << 20;
/// Cap on the decoded `(figmeta)` JSON, which only holds a handful of keys.
const MAX_META_BYTES: usize = 1 << 20;

/// Standard-alphabet base64 that tolerates missing padding (JS `btoa` output
/// is padded, but a clipboard round trip may trim it).
const LENIENT_BASE64: GeneralPurpose =
    GeneralPurpose::new(&alphabet::STANDARD, GeneralPurposeConfig::new().with_decode_padding_mode(DecodePaddingMode::Indifferent));

/// A decoded Figma clipboard payload.
#[derive(Debug, Clone, PartialEq)]
pub struct FigClipboard {
    /// The `(figmeta)` JSON (`fileKey`, `pasteID`, `dataType`), when present.
    pub meta: Option<serde_json::Value>,
    /// The `(figma)` buffer: a fig-kiwi archive.
    pub buffer: Vec<u8>,
}

/// Whether `html` carries a Figma `(figma)` payload comment (raw or entity-escaped).
pub fn looks_like_figma_clipboard(html: &str) -> bool {
    find_open(html, "figma").is_some()
}

/// Extracts and decodes the `(figmeta)` and `(figma)` payloads of clipboard `html`.
pub fn parse_figma_clipboard_html(html: &str) -> Result<FigClipboard, FigError> {
    parse_with_limits(html, MAX_BUFFER_BYTES)
}

fn parse_with_limits(html: &str, max_buffer: usize) -> Result<FigClipboard, FigError> {
    let body = marked_payload(html, "figma")?.ok_or_else(|| FigError::Format("clipboard has no (figma) payload".into()))?;
    let buffer = decode_base64(body, max_buffer, "(figma)")?;
    let meta = match marked_payload(html, "figmeta")? {
        Some(m) => {
            let bytes = decode_base64(m, MAX_META_BYTES, "(figmeta)")?;
            Some(serde_json::from_slice(&bytes)?)
        }
        None => None,
    };
    Ok(FigClipboard { meta, buffer })
}

/// Start marker positions: `<!--(tag)` or its entity-escaped form. Returns `(index after marker)`.
fn find_open(html: &str, tag: &str) -> Option<usize> {
    [format!("<!--({tag})"), format!("&lt;!--({tag})")].iter().filter_map(|m| html.find(m.as_str()).map(|i| i + m.len())).min()
}

/// The text between `<!--(tag)` and `(/tag)-->` (either side may be entity-escaped), if the open marker exists.
fn marked_payload<'a>(html: &'a str, tag: &str) -> Result<Option<&'a str>, FigError> {
    let Some(start) = find_open(html, tag) else { return Ok(None) };
    let rest = &html[start..];
    let end = [format!("(/{tag})-->"), format!("(/{tag})--&gt;")]
        .iter()
        .filter_map(|m| rest.find(m.as_str()))
        .min()
        .ok_or_else(|| FigError::Format(format!("clipboard ({tag}) payload is not terminated")))?;
    Ok(Some(&rest[..end]))
}

fn decode_base64(text: &str, max_bytes: usize, what: &str) -> Result<Vec<u8>, FigError> {
    let clean: String = text.chars().filter(|c| !c.is_ascii_whitespace()).collect();
    // 4 base64 chars carry at most 3 bytes; reject before allocating the decode buffer.
    if clean.len() / 4 * 3 > max_bytes {
        return Err(FigError::Format(format!("clipboard {what} payload is too large")));
    }
    let bytes = LENIENT_BASE64.decode(clean.as_bytes()).map_err(|e| FigError::Format(format!("clipboard {what} payload is not base64: {e}")))?;
    if bytes.len() > max_bytes {
        return Err(FigError::Format(format!("clipboard {what} payload is too large")));
    }
    Ok(bytes)
}

/// Imports the copied nodes of `clip` as a [`Document`]; the nodes are the top-level children of its page(s).
///
/// A copy of part of a page has no `CANVAS` ancestor in its `nodeChanges`, so
/// a synthetic page adopts the orphaned roots. A payload that carries a whole
/// file tree (`DOCUMENT` / `CANVAS`) imports like a `.fig`. Images are embedded
/// when the archive ships them; hashes it does not ship stay unresolved.
pub fn import_figma_clipboard(clip: &FigClipboard) -> Result<Document, FigError> {
    let mut archive = container::open(&clip.buffer)?;
    let (schema_bin, msg_bin) = container::read_archive_chunks(&archive)?;
    archive.canvas = Vec::new();
    let schema = Schema::decode(&schema_bin).map_err(|e| FigError::Format(format!("kiwi schema: {e}")))?;
    drop(schema_bin);
    let root = schema.root_message().ok_or_else(|| FigError::Format("schema has no message definition".into()))?;
    let mut message = schema.decode_message_for_import(root, &msg_bin).map_err(|e| FigError::Format(format!("kiwi message: {e}")))?;
    drop(msg_bin);

    adopt_orphan_roots(&mut message);
    let blobs: Vec<&[u8]> = message
        .get("blobs")
        .and_then(KiwiValue::as_array)
        .map(|a| a.iter().map(|b| b.get("bytes").and_then(KiwiValue::as_bytes).unwrap_or(&[])).collect())
        .unwrap_or_default();
    let rest = message_to_rest(&message, "Figma clipboard");
    let mut doc = rest_import::import_rest_value_with_blobs(&rest, true, &blobs, None)?;
    doc.images = archive.images;
    Ok(doc)
}

fn guid_key(g: &KiwiValue) -> Option<(u64, u64)> {
    let n = |v: &KiwiValue| match v {
        KiwiValue::Uint(u) => Some(u64::from(*u)),
        KiwiValue::Uint64(u) => Some(*u),
        KiwiValue::Int(i) => u64::try_from(*i).ok(),
        KiwiValue::Int64(i) => u64::try_from(*i).ok(),
        _ => None,
    };
    Some((n(g.get("sessionID")?)?, n(g.get("localID")?)?))
}

fn is_page_level(change: &KiwiValue) -> bool {
    matches!(change.get("type"), Some(KiwiValue::Enum(t)) if matches!(&**t, "DOCUMENT" | "CANVAS"))
}

/// When no change is a `DOCUMENT` or `CANVAS`, appends a synthetic `CANVAS` and re-parents every node whose
/// parent is not part of the payload (the copied roots) under it, keeping their sibling positions.
fn adopt_orphan_roots(message: &mut KiwiValue) {
    let Some(KiwiValue::Array(changes)) = message.get_mut("nodeChanges") else { return };
    if changes.iter().any(is_page_level) {
        return;
    }
    let present: std::collections::HashSet<(u64, u64)> = changes.iter().filter_map(|c| c.get("guid").and_then(guid_key)).collect();
    // a guid in a session id no client uses, so the synthetic page cannot collide with a copied node
    let page: (u32, u32) = (0..=u32::MAX).map(|l| (u32::MAX, l)).find(|g| !present.contains(&(u64::from(g.0), u64::from(g.1)))).unwrap_or((u32::MAX, u32::MAX));
    let page_guid = || KiwiValue::Struct(vec![(Arc::from("sessionID"), KiwiValue::Uint(page.0)), (Arc::from("localID"), KiwiValue::Uint(page.1))]);
    let mut adopted = false;
    for c in changes.iter_mut() {
        let parent = c.get("parentIndex").and_then(|p| p.get("guid")).and_then(guid_key);
        if parent.is_some_and(|p| present.contains(&p)) {
            continue;
        }
        let position = c.get("parentIndex").and_then(|p| p.get("position")).cloned().unwrap_or(KiwiValue::String(String::new()));
        let index = KiwiValue::Struct(vec![(Arc::from("guid"), page_guid()), (Arc::from("position"), position)]);
        match c.get_mut("parentIndex") {
            Some(slot) => *slot = index,
            None => {
                if let KiwiValue::Message(fields) = c {
                    fields.push((Arc::from("parentIndex"), index));
                }
            }
        }
        adopted = true;
    }
    if adopted {
        changes.push(KiwiValue::Message(vec![
            (Arc::from("guid"), page_guid()),
            (Arc::from("type"), KiwiValue::Enum(Arc::from("CANVAS"))),
            (Arc::from("name"), KiwiValue::String("Pasted".into())),
        ]));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wrap_raw(b64_meta: &str, b64_buf: &str) -> String {
        format!(
            "<meta charset=\"utf-8\"><div><span data-metadata=\"<!--(figmeta){b64_meta}(/figmeta)-->\"></span><span data-buffer=\"<!--(figma){b64_buf}(/figma)-->\"></span></div><span style=\"white-space:pre-wrap;\">x</span>"
        )
    }

    #[test]
    fn raw_and_escaped_markers_decode_the_same_bytes() {
        // "{\"dataType\":\"scene\"}" and "hello"
        let meta = "eyJkYXRhVHlwZSI6InNjZW5lIn0=";
        let raw = wrap_raw(meta, "aGVsbG8=");
        let escaped = raw.replace("<!--", "&lt;!--").replace("-->", "--&gt;");
        for html in [raw, escaped] {
            assert!(looks_like_figma_clipboard(&html));
            let clip = parse_figma_clipboard_html(&html).unwrap();
            assert_eq!(clip.buffer, b"hello");
            assert_eq!(clip.meta.unwrap()["dataType"], "scene");
        }
    }

    #[test]
    fn missing_or_broken_payload_is_an_error() {
        assert!(!looks_like_figma_clipboard("<b>(figma) in prose</b>"));
        assert!(parse_figma_clipboard_html("<b>plain</b>").is_err());
        // figmeta alone is not enough
        assert!(parse_figma_clipboard_html("<!--(figmeta)e30=(/figmeta)-->").is_err());
        // unterminated
        assert!(parse_figma_clipboard_html("<!--(figma)aGVsbG8=").is_err());
        // not base64
        assert!(parse_figma_clipboard_html(&wrap_raw("e30=", "@@not base64@@")).is_err());
        // figmeta that is not JSON
        assert!(parse_figma_clipboard_html(&wrap_raw("aGVsbG8=", "aGVsbG8=")).is_err());
    }

    #[test]
    fn whitespace_and_missing_padding_are_tolerated() {
        let clip = parse_figma_clipboard_html("<!--(figma)aGVs\nbG8(/figma)-->").unwrap();
        assert_eq!(clip.buffer, b"hello");
        assert!(clip.meta.is_none());
    }

    #[test]
    fn decoded_size_is_capped_before_decoding() {
        let html = "<!--(figma)aGVsbG8gd29ybGQ=(/figma)-->"; // 11 bytes
        assert!(parse_with_limits(html, 12).is_ok());
        assert!(parse_with_limits(html, 8).is_err());
    }
}
