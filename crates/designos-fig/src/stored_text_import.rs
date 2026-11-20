//! What an import keeps of the data Figma stored for each text: the wire bytes of its `derivedTextData` for a save, and
//! the glyph table the renderer draws from.
//!
//! Each text is handled alone: its wire bytes are decoded, the table is built from the decoded value, and the value is
//! dropped before the next text. Nothing is turned into a path here; the renderer builds outlines when a text is drawn.
//! A text whose data the renderer must not draw keeps its wire bytes and its blobs all the same, with the reason, so a
//! save writes what Figma stored whether or not Studio can draw it.

use crate::derived_text_wire::{decode_wire, wire_from_json};
use crate::kiwi::KiwiValue as K;
use crate::rest_import;
use crate::stored_glyphs::MAX_DRAWN_GLYPHS;
use designos_doc::{Document, FigTextGeometry, GlyphBlobExtent, Node, NodeId, NodeKind, StoredDecoration, StoredFallback, StoredGlyph};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// A glyph blob of the file, held once for every text that names it, with what the importer learned about it.
pub(crate) struct SharedBlob {
    bytes: Arc<[u8]>,
    /// `None` when a coordinate of the blob is not finite: no text can draw it.
    extent: Option<GlyphBlobExtent>,
}

/// Gives every text of `doc` the data Figma stored for it.
///
/// `wires` holds the wire bytes the binary import took from the node changes, by node. A text that is not in it may carry
/// its `derivedTextData` as JSON in `fig_metadata` (a REST-shaped payload or a fixture); that is turned into wire bytes
/// and leaves the metadata. `run_sids` holds, for a text with style runs, the style id Figma gave each run.
///
/// Runs on the importer's nodes before instances are expanded, so a copy of a component's text starts with the data of the
/// component's own text: an instance that changes the characters or the style then no longer matches it.
pub(crate) fn attach(doc: &mut Document, blobs: &[&[u8]], run_sids: &HashMap<NodeId, Vec<u32>>, mut wires: HashMap<NodeId, Arc<[u8]>>) {
    // the texts to give a geometry, each with the bytes of its data
    let mut work: Vec<(NodeId, Arc<[u8]>)> = Vec::new();
    for node in doc.nodes.values_mut() {
        if !matches!(node.kind, NodeKind::Text(_)) {
            continue;
        }
        let wire = match wires.remove(&node.id) {
            Some(wire) => wire,
            None => {
                let converted = node
                    .fig_metadata
                    .get("derivedTextData")
                    .and_then(|json| serde_json::from_str::<serde_json::Value>(json).ok())
                    .and_then(|value| wire_from_json(&value));
                let Some(bytes) = converted else { continue };
                node.fig_metadata.remove("derivedTextData");
                Arc::from(bytes)
            }
        };
        work.push((node.id, wire));
    }
    // Each text is built alone from its own bytes, so the texts are shared out between threads; only the blobs of the file are
    // shared, through one map, and each thread asks it once for a blob it has not seen.
    let shared: Mutex<HashMap<usize, Arc<SharedBlob>>> = Mutex::new(HashMap::new());
    let threads = if work.len() < 512 { 1 } else { std::thread::available_parallelism().map_or(1, |n| n.get()).min(8) };
    let chunk = work.len().div_ceil(threads).max(1);
    let nodes = &doc.nodes;
    let built: Vec<(NodeId, Option<FigTextGeometry>)> = std::thread::scope(|scope| {
        let handles: Vec<_> = work
            .chunks(chunk)
            .map(|texts| {
                let shared = &shared;
                scope.spawn(move || {
                    let mut local: HashMap<usize, Arc<SharedBlob>> = HashMap::new();
                    texts
                        .iter()
                        .map(|(id, wire)| {
                            let sids = run_sids.get(id).map_or(&[][..], Vec::as_slice);
                            (*id, nodes.get(id).and_then(|node| geometry(node, Arc::clone(wire), blobs, &mut BlobCache { shared, local: &mut local }, sids)))
                        })
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        handles.into_iter().flat_map(|h| h.join().expect("a text of the import is built without panicking")).collect()
    });
    for (id, geometry) in built {
        if let Some(node) = doc.nodes.get_mut(&id) {
            node.fig_text_geometry = geometry.map(Arc::new);
        }
    }
}

/// The blobs of the file one thread has met, in front of the map every thread shares.
struct BlobCache<'a> {
    shared: &'a Mutex<HashMap<usize, Arc<SharedBlob>>>,
    local: &'a mut HashMap<usize, Arc<SharedBlob>>,
}

impl BlobCache<'_> {
    /// The blob of file number `index`, built once for the whole import.
    fn get(&mut self, index: usize, blob: &[u8]) -> Arc<SharedBlob> {
        if let Some(hit) = self.local.get(&index) {
            return Arc::clone(hit);
        }
        let made = {
            let mut shared = self.shared.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            Arc::clone(shared.entry(index).or_insert_with(|| Arc::new(SharedBlob { bytes: Arc::from(blob), extent: designos_doc::glyph_blob_extent(blob) })))
        };
        self.local.insert(index, Arc::clone(&made));
        made
    }
}

fn num(v: &K, key: &str) -> Option<f32> {
    match v.get(key)? {
        K::Float(f) => Some(*f),
        K::Int(i) => Some(*i as f32),
        K::Uint(u) => Some(*u as f32),
        _ => None,
    }
}

fn uint(v: &K, key: &str) -> Option<u64> {
    match v.get(key)? {
        K::Uint(u) => Some(u64::from(*u)),
        K::Uint64(u) => Some(*u),
        K::Int(i) => u64::try_from(*i).ok(),
        K::Int64(i) => u64::try_from(*i).ok(),
        _ => None,
    }
}

fn array<'a>(v: &'a K, key: &str) -> &'a [K] {
    v.get(key).and_then(K::as_array).unwrap_or(&[])
}

/// The blobs the glyphs of one text name, in first-use order, and what the importer saw about them.
struct TextBlobs {
    /// Each distinct blob once, in first-use order.
    unique: Vec<Arc<[u8]>>,
    /// The number of each of [`unique`](Self::unique) in the file.
    files: Vec<u32>,
    /// The extent of each of [`unique`](Self::unique).
    extents: Vec<Option<GlyphBlobExtent>>,
    /// The slot of each glyph's blob, `None` for a glyph with none or whose blob the file does not have.
    slots: Vec<Option<u32>>,
    /// Blob bytes, counted once per glyph that names a blob: a shared blob costs its bytes for every glyph that draws it.
    bytes: usize,
    /// A glyph names a blob the file does not have.
    missing: bool,
}

fn collect_blobs(glyphs: &[K], blobs: &[&[u8]], shared: &mut BlobCache) -> TextBlobs {
    let mut out = TextBlobs { unique: Vec::new(), files: Vec::new(), extents: Vec::new(), slots: Vec::with_capacity(glyphs.len()), bytes: 0, missing: false };
    let mut slot_of: HashMap<usize, u32> = HashMap::new();
    for glyph in glyphs {
        let Some(index) = uint(glyph, "commandsBlob") else {
            out.slots.push(None);
            continue;
        };
        let Some((index, blob)) = usize::try_from(index).ok().and_then(|i| blobs.get(i).copied().map(|b| (i, b))) else {
            out.missing = true;
            out.slots.push(None);
            continue;
        };
        out.bytes = out.bytes.saturating_add(blob.len());
        let entry = shared.get(index, blob);
        let slot = match slot_of.get(&index) {
            Some(&slot) => Some(slot),
            None => match (u32::try_from(out.unique.len()), u32::try_from(index)) {
                (Ok(slot), Ok(file)) => {
                    out.unique.push(Arc::clone(&entry.bytes));
                    out.files.push(file);
                    out.extents.push(entry.extent);
                    slot_of.insert(index, slot);
                    Some(slot)
                }
                _ => {
                    out.missing = true;
                    None
                }
            },
        };
        out.slots.push(slot);
    }
    out
}

/// The geometry of a text from the wire bytes of its `derivedTextData`: the glyph table when the renderer may draw it,
/// and otherwise the reason ([`StoredFallback`]). `None` only when the bytes cannot be read, which cannot happen to bytes
/// this program wrote. `shared` holds each file blob once for the whole import (Figma shares one blob between every
/// occurrence of a glyph, and so do the geometries built from it).
fn geometry(node: &Node, wire: Arc<[u8]>, blobs: &[&[u8]], shared: &mut BlobCache, run_sids: &[u32]) -> Option<FigTextGeometry> {
    let NodeKind::Text(text) = &node.kind else { return None };
    let derived = decode_wire(&wire)?;
    let glyphs = array(&derived, "glyphs");
    let used = collect_blobs(glyphs, blobs, shared);
    let refused = |why: StoredFallback| {
        Some(FigTextGeometry {
            text: text.clone(),
            size: node.size,
            fallback: Some(why),
            blobs: used.unique.clone(),
            blob_files: used.files.clone().into(),
            wire: Arc::clone(&wire),
            ..Default::default()
        })
    };
    if text.content.is_empty() || glyphs.is_empty() {
        return refused(StoredFallback::NothingDrawn);
    }
    let truncated = matches!(derived.get("truncationStartIndex"), Some(K::Int(i)) if *i >= 0);
    let layout_ok = match derived.get("layoutSize") {
        Some(size) => matches!((num(size, "x"), num(size, "y")), (Some(x), Some(y)) if (x - node.size.x).abs() <= 0.1 && (y - node.size.y).abs() <= 0.1),
        None => false,
    };
    if !layout_ok {
        // a truncated text's stored size is the untruncated one: the cause is the truncation
        return refused(if truncated { StoredFallback::Truncated } else { StoredFallback::LayoutSize });
    }
    // The fonts Figma stored must include one the text uses (its own, or a run's): else the data is of another text.
    let own_fonts: Vec<String> = std::iter::once(&text.style).chain(text.runs.iter().map(|r| &r.style)).map(|s| s.font_family.to_lowercase()).collect();
    let stored_family = |font: &K| match font.get("key").and_then(|k| k.get("family")) {
        Some(K::String(family)) => Some(family.to_lowercase()),
        _ => None,
    };
    if !array(&derived, "fontMetaData").iter().filter_map(stored_family).any(|family| own_fonts.contains(&family)) {
        return refused(StoredFallback::Fonts);
    }
    if run_sids.len() != text.runs.len() {
        return refused(StoredFallback::RunIds);
    }
    if glyphs.len() > MAX_DRAWN_GLYPHS {
        return refused(StoredFallback::GlyphCount);
    }
    if used.bytes > designos_doc::MAX_BLOB_BYTES_PER_TEXT {
        return refused(StoredFallback::BlobBudget);
    }
    if used.missing {
        return refused(StoredFallback::Malformed);
    }
    // UTF-16 index of each character boundary, to put each glyph in the run that holds its first character
    let offsets = (!text.runs.is_empty()).then(|| rest_import::utf16_to_byte_offsets(&text.content));
    let units = text.content.chars().map(char::len_utf16).sum::<usize>();
    let style_of = |first: usize| -> u16 {
        let Some(offsets) = &offsets else { return 0 };
        let at = offsets[first];
        let run = text.runs.partition_point(|r| r.start <= at);
        run.checked_sub(1).filter(|&i| at < text.runs[i].end).map_or(0, |i| u16::try_from(i + 1).unwrap_or(0))
    };
    let mut table = Vec::with_capacity(glyphs.len());
    let mut drawn_commands = 0usize;
    let mut emoji = false;
    let mut outside = 0usize;
    for (glyph, slot) in glyphs.iter().zip(&used.slots) {
        let (Some(scale), Some(pos)) = (num(glyph, "fontSize"), glyph.get("position")) else { return refused(StoredFallback::Malformed) };
        let (Some(x), Some(y)) = (num(pos, "x"), num(pos, "y")) else { return refused(StoredFallback::Malformed) };
        if !scale.is_finite() || scale <= 0.0 || !x.is_finite() || !y.is_finite() {
            return refused(StoredFallback::Malformed);
        }
        if num(glyph, "rotation").unwrap_or(0.0).abs() > 1e-5 {
            return refused(StoredFallback::RotatedGlyph);
        }
        let first_char = uint(glyph, "firstCharacter").map_or(StoredGlyph::UNKNOWN_FIRST_CHAR, |c| u32::try_from(c).unwrap_or(StoredGlyph::UNKNOWN_FIRST_CHAR));
        let inside = first_char != StoredGlyph::UNKNOWN_FIRST_CHAR && (first_char as usize) < units;
        outside += usize::from(!inside);
        let is_emoji = !array(glyph, "emojiCodePoints").is_empty() || glyph.get("emojiImageSet").is_some();
        emoji |= is_emoji;
        let style = if inside { style_of(first_char as usize) } else { 0 };
        let flags = if is_emoji { StoredGlyph::EMOJI } else { 0 };
        let mut stored = StoredGlyph { x, y, font_size: scale, blob: StoredGlyph::NO_BLOB, first_char, style, flags };
        if let Some(slot) = *slot {
            stored.blob = slot;
            let Some(extent) = used.extents[slot as usize] else { return refused(StoredFallback::Malformed) };
            // A glyph is drawn at `x + p.x * scale`, `y - p.y * scale`. Both maps are monotonic per axis, so the corners of
            // the blob's box bound every point: when they are finite so is each outline point.
            if extent.commands > 0 {
                let corners = [x + extent.min.x * scale, x + extent.max.x * scale, y - extent.min.y * scale, y - extent.max.y * scale];
                if !corners.iter().all(|c| c.is_finite()) {
                    return refused(StoredFallback::Malformed);
                }
                drawn_commands += extent.commands;
            }
        }
        table.push(stored);
    }
    // a bar belongs to the run Figma gave its style id, and to the text's own style when no run has it
    let style_of_id = |sid: u64| -> u16 {
        u32::try_from(sid).ok().and_then(|sid| run_sids.iter().position(|&r| r == sid)).and_then(|i| u16::try_from(i + 1).ok()).unwrap_or(0)
    };
    let mut decorations = Vec::new();
    for deco in array(&derived, "decorations") {
        let style = style_of_id(uint(deco, "styleID").unwrap_or(0));
        for rect in array(deco, "rects") {
            let (Some(x), Some(y), Some(w), Some(h)) = (num(rect, "x"), num(rect, "y"), num(rect, "w"), num(rect, "h")) else {
                return refused(StoredFallback::Malformed);
            };
            if ![x, y, w, h].iter().all(|v| v.is_finite()) {
                return refused(StoredFallback::Malformed);
            }
            decorations.push(StoredDecoration { style, x, y, w, h });
        }
    }
    if drawn_commands == 0 && !emoji && decorations.is_empty() {
        return refused(StoredFallback::NothingDrawn);
    }
    let non_empty = |key: &str| !array(&derived, key).is_empty();
    // A list's markers are glyphs of no character of the text, one for each list paragraph (every list text of the design
    // kit stores them so, to the left of its first character). Any other glyph of no character is a data problem.
    let list_paragraphs = if text.lists.is_empty() { 0 } else { text.paragraph_lists().iter().flatten().count() };
    let fallback = if truncated {
        Some(StoredFallback::Truncated)
    } else if non_empty("blockquotes") || non_empty("checklistMarkers") {
        Some(StoredFallback::Markers)
    } else if outside != list_paragraphs {
        Some(StoredFallback::FirstCharacter)
    } else {
        None
    };
    Some(FigTextGeometry {
        text: text.clone(),
        size: node.size,
        blobs: used.unique,
        blob_files: used.files.into(),
        glyphs: table.into(),
        decorations: decorations.into(),
        run_sids: run_sids.into(),
        fallback,
        wire,
        ..Default::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use designos_doc::{ListKind, ListRange, Rect, TextStyle, Vec2};
    use serde_json::{json, Value};

    const FACE: &str = "Some Face";
    /// A closed triangle in font-em space.
    fn triangle() -> Vec<u8> {
        let mut blob = Vec::new();
        for (op, x, y) in [(1u8, 0.0f32, 0.0f32), (2, 1.0, 0.0), (2, 0.5, 0.8)] {
            blob.push(op);
            blob.extend(x.to_le_bytes());
            blob.extend(y.to_le_bytes());
        }
        blob.push(0);
        blob
    }

    fn text(content: &str) -> Node {
        Node::text(NodeId(1), Rect::new(0.0, 0.0, 40.0, 20.0), content, TextStyle { font_family: FACE.into(), font_size: 20.0, ..Default::default() })
    }

    /// A glyph at x with an optional blob; `first` is its first character, `None` for a glyph of no character.
    fn glyph(blob: Option<u32>, x: f32, first: Option<u32>) -> Value {
        let mut g = json!({"position": {"x": x, "y": 18.0}, "fontSize": 20.0});
        if let Some(b) = blob {
            g["commandsBlob"] = json!(b);
        }
        if let Some(f) = first {
            g["firstCharacter"] = json!(f);
        }
        g
    }

    fn derived(glyphs: Vec<Value>) -> Value {
        json!({"layoutSize": {"x": 40.0, "y": 20.0}, "fontMetaData": [{"key": {"family": FACE}}], "glyphs": glyphs})
    }

    fn build(node: &Node, derived: &Value) -> Option<FigTextGeometry> {
        let blob = triangle();
        let wire: Arc<[u8]> = wire_from_json(derived).expect("a derivedTextData").into();
        geometry(node, wire, &[&blob[..], &blob[..]], &mut BlobCache { shared: &Mutex::new(HashMap::new()), local: &mut HashMap::new() }, &[])
    }

    #[test]
    fn a_glyph_whose_first_character_is_not_given_gets_the_unknown_sentinel() {
        let node = text("abcde");
        let g = build(&node, &derived(vec![glyph(Some(0), 0.0, Some(3)), glyph(Some(0), 5.0, None), glyph(Some(0), 10.0, Some(0))])).unwrap();
        let first: Vec<u32> = g.glyphs.iter().map(|g| g.first_char).collect();
        assert_eq!(first, vec![3, StoredGlyph::UNKNOWN_FIRST_CHAR, 0], "only a number is an index; 0 is a real index");
    }

    #[test]
    fn a_text_the_renderer_must_not_draw_still_keeps_its_wire_bytes_and_blobs() {
        let mut node = text("ab");
        node.size = Vec2::new(60.0, 20.0);
        let d = derived(vec![glyph(Some(1), 0.0, Some(0)), glyph(Some(1), 5.0, Some(1))]);
        let g = build(&node, &d).expect("a geometry that names the reason");
        assert_eq!(g.fallback, Some(StoredFallback::LayoutSize));
        assert!(g.glyphs.is_empty(), "no table is built for data the renderer refuses");
        assert!(!g.wire.is_empty(), "the bytes are kept for a save");
        assert_eq!((g.blobs.len(), &*g.blob_files), (1, &[1u32][..]), "and the one blob the glyphs name, with its number in the file");
        assert!(g.is_table_form());
    }

    #[test]
    fn glyphs_that_draw_nothing_keep_the_data_with_a_reason() {
        let node = text("  ");
        let g = build(&node, &derived(vec![glyph(None, 0.0, Some(0)), glyph(None, 5.0, Some(1))])).unwrap();
        assert_eq!(g.fallback, Some(StoredFallback::NothingDrawn));
        let g = build(&text(""), &derived(vec![glyph(Some(0), 0.0, Some(0))])).unwrap();
        assert_eq!(g.fallback, Some(StoredFallback::NothingDrawn), "no characters");
        let g = build(&text("ab"), &derived(Vec::new())).unwrap();
        assert_eq!(g.fallback, Some(StoredFallback::NothingDrawn), "no glyphs");
        assert!(!g.wire.is_empty());
    }

    #[test]
    fn a_blob_the_file_does_not_have_is_malformed_but_the_data_is_kept() {
        let g = build(&text("ab"), &derived(vec![glyph(Some(0), 0.0, Some(0)), glyph(Some(9), 5.0, Some(1))])).unwrap();
        assert_eq!(g.fallback, Some(StoredFallback::Malformed));
        assert!(!g.wire.is_empty());
    }

    #[test]
    fn a_list_draws_when_it_has_one_marker_for_each_list_paragraph() {
        let mut node = text("ab\ncd");
        if let NodeKind::Text(t) = &mut node.kind {
            t.lists.push(ListRange { start: 0, end: 5, kind: ListKind::Bullet, level: 0 });
        }
        let marker = |x| glyph(Some(0), x, None);
        let two_markers = derived(vec![
            marker(-8.0),
            glyph(Some(0), 0.0, Some(0)),
            glyph(Some(0), 5.0, Some(1)),
            marker(-8.0),
            glyph(Some(0), 0.0, Some(3)),
            glyph(Some(0), 5.0, Some(4)),
        ]);
        assert_eq!(build(&node, &two_markers).unwrap().fallback, None, "two list paragraphs, two markers");
        let one_marker =
            derived(vec![marker(-8.0), glyph(Some(0), 0.0, Some(0)), glyph(Some(0), 5.0, Some(1)), glyph(Some(0), 0.0, Some(3)), glyph(Some(0), 5.0, Some(4))]);
        assert_eq!(build(&node, &one_marker).unwrap().fallback, Some(StoredFallback::FirstCharacter), "a marker missing: not drawn");
        let plain = text("ab");
        let stray = derived(vec![marker(-8.0), glyph(Some(0), 0.0, Some(0)), glyph(Some(0), 5.0, Some(1))]);
        assert_eq!(build(&plain, &stray).unwrap().fallback, Some(StoredFallback::FirstCharacter), "a glyph of no character in a text that is not a list");
    }

    #[test]
    fn a_truncated_text_is_named_whether_or_not_its_size_matches() {
        let mut d = derived(vec![glyph(Some(0), 0.0, Some(0)), glyph(Some(0), 5.0, Some(1))]);
        d["truncationStartIndex"] = json!(1);
        assert_eq!(build(&text("ab"), &d).unwrap().fallback, Some(StoredFallback::Truncated), "the table is kept");
        let mut taller = text("ab");
        taller.size = Vec2::new(40.0, 10.0);
        let g = build(&taller, &d).unwrap();
        assert_eq!((g.fallback, g.glyphs.len()), (Some(StoredFallback::Truncated), 0), "a stored size that is not the layer's is the truncation's doing");
    }
}
