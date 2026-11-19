//! The unit paths of stored glyph blobs are cached for the whole process, up to a byte bound. A text whose blobs decode
//! to more than the bound still draws, and the cache stays at or under it. The bound holds at every moment, whatever else draws
//! stored text in the process.
use designos_doc::{FigTextGeometry, Node, NodeKind, Rect, StoredGlyph, TextStyle};
use designos_render::{stored_glyph_cache_bytes, stored_glyph_use, text_outlines_for_node, StoredGlyphUse, STORED_GLYPH_CACHE_BYTES};
use std::sync::Arc;

/// A blob of `n` close commands, one byte each: the most commands a blob budget of 2 MB can buy, each 28 bytes decoded.
fn blob_of(n: usize) -> Arc<[u8]> {
    vec![0u8; n].into()
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
        ..Default::default()
    }));
    node
}

#[test]
fn filling_the_cache_past_its_bound_keeps_it_at_or_under_the_bound() {
    // 1,900 blobs of 1,000 bytes stay inside the 2 MB blob budget of one text, and decode to 1,900 x 28 KB: about 53 MB of
    // commands against the 32 MiB bound. The family is made up, so no installed font stands in for the stored glyphs.
    let texts = [text_of((0..1_900).map(|_| blob_of(1_000)).collect()), text_of((0..1_900).map(|_| blob_of(1_000)).collect())];
    for (n, node) in texts.iter().enumerate() {
        assert_eq!(stored_glyph_use(node), StoredGlyphUse::Drawn, "text {n} is within the limits, so its blobs are decoded and cached");
        text_outlines_for_node(node);
        let held = stored_glyph_cache_bytes();
        assert!(held > 0 && held <= STORED_GLYPH_CACHE_BYTES, "{held} bytes held against a bound of {STORED_GLYPH_CACHE_BYTES} after text {n}");
        eprintln!("MEASURE unit-path cache holds {held} of {STORED_GLYPH_CACHE_BYTES} bytes after text {n} (about 53 MB of decoded commands each)");
    }
}
