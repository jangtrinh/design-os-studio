//! The unit paths of stored glyph blobs are cached for the whole process, up to a byte bound. A text whose blobs decode
//! to more than the bound still draws, and the cache stays at or under it. The bound holds at every moment, whatever else draws
//! stored text in the process.
use designos_doc::{FigTextGeometry, Node, NodeKind, Rect, StoredGlyph, TextStyle};
use designos_render::{stored_glyph_cache_bytes, text_outlines_for_node, STORED_GLYPH_CACHE_BYTES};
use std::sync::Arc;

/// A blob of `n` line commands, each at its own height so no two blobs are alike.
fn blob_of(n: usize, seed: usize) -> Arc<[u8]> {
    let mut b = vec![1u8];
    b.extend(0.0f32.to_le_bytes());
    b.extend(0.0f32.to_le_bytes());
    for i in 0..n {
        b.push(2);
        b.extend(((i + seed) as f32 * 1e-4).to_le_bytes());
        b.extend((1e-3 + i as f32 * 1e-4).to_le_bytes());
    }
    b.into()
}

fn text_of(blobs: Vec<Arc<[u8]>>) -> Node {
    let style = TextStyle { font_family: "MissingFaceForTheCacheBoundTest".into(), font_size: 10.0, ..TextStyle::default() };
    let mut node = Node::text(designos_doc::NodeId(1), Rect::new(0.0, 0.0, 500.0, 20.0), "x", style);
    let NodeKind::Text(text) = &node.kind else { unreachable!() };
    let glyphs: Vec<StoredGlyph> =
        (0..blobs.len() as u32).map(|i| StoredGlyph { x: i as f32 * 0.1, y: 10.0, font_size: 10.0, blob: i, first_char: 0, style: 0, flags: 0 }).collect();
    node.fig_text_geometry = Some(Arc::new(FigTextGeometry {
        text: text.clone(),
        size: node.size,
        path: Default::default(),
        blobs,
        glyph_blobs: Vec::new(),
        glyphs: glyphs.into(),
    }));
    node
}

#[test]
fn filling_the_cache_past_its_bound_keeps_it_at_or_under_the_bound() {
    // 3,000 blobs of 400 commands decode to 3,000 x 11 KB: about 34 MB of commands next to the 32 MiB bound, with the
    // blobs themselves on top
    let node = text_of((0..3_000).map(|i| blob_of(400, i)).collect());
    let outlines = text_outlines_for_node(&node);
    assert_eq!(outlines.len(), 1, "a text of more blobs than the cache holds still draws");
    let held = stored_glyph_cache_bytes();
    assert!(held > 0 && held <= STORED_GLYPH_CACHE_BYTES, "{held} bytes held against a bound of {STORED_GLYPH_CACHE_BYTES}");
    let more = text_of((0..3_000).map(|i| blob_of(400, 5_000 + i)).collect());
    text_outlines_for_node(&more);
    let held = stored_glyph_cache_bytes();
    assert!(held <= STORED_GLYPH_CACHE_BYTES, "{held} bytes held against a bound of {STORED_GLYPH_CACHE_BYTES} after a second text");
    eprintln!("MEASURE unit-path cache holds {held} of {STORED_GLYPH_CACHE_BYTES} bytes after drawing about 68 MB of decoded commands");
}
