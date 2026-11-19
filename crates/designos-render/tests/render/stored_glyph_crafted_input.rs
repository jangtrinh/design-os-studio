//! A glyph table that arrives in a document file or a clipboard payload is not held to the importer's limits by loading.
//! The renderer checks them before it assembles an outline, names the reason, and shapes the text instead: nothing
//! near the product of glyphs and blob commands is allocated, and no non-finite point reaches an exporter.

use designos_doc::{Document, FigTextGeometry, Node, NodeKind, Path, Rect, StoredGlyph, TextProps, TextStyle, Vec2};
use designos_render::{stored_glyph_use, text_outlines_for_node, text_runs_for_node, StoredGlyphUse};
use std::sync::Arc;

/// A text in a font nobody has, holding `glyphs` over `blobs`, saved to a document file and loaded again.
fn loaded_text(glyphs: Vec<StoredGlyph>, blobs: Vec<Vec<u8>>, path: Path) -> Node {
    let mut doc = Document::new("crafted");
    let page = doc.add_page("Page");
    let id = doc.alloc_id();
    let style = TextStyle { font_family: "MissingFaceForTheCraftedInputTest".into(), font_size: 14.0, ..TextStyle::default() };
    let mut node = Node::text(id, Rect::new(0.0, 0.0, 40.0, 20.0), "ab", style);
    let NodeKind::Text(text) = &node.kind else { unreachable!() };
    let text: TextProps = text.clone();
    node.fig_text_geometry = Some(Arc::new(FigTextGeometry {
        text,
        size: Vec2::new(40.0, 20.0),
        path,
        blobs: blobs.into_iter().map(Arc::from).collect(),
        glyph_blobs: Vec::new(),
        glyphs: glyphs.into(),
    }));
    doc.insert(page, node).unwrap();
    let reloaded = Document::from_json(&doc.to_json().unwrap()).unwrap();
    reloaded.nodes.values().find(|n| matches!(n.kind, NodeKind::Text(_))).unwrap().clone()
}

fn glyph(x: f32, y: f32, font_size: f32, blob: u32) -> StoredGlyph {
    StoredGlyph { x, y, font_size, blob, first_char: 0, style: 0, flags: 0 }
}

/// A blob of `n` line commands.
fn lines(n: usize) -> Vec<u8> {
    let mut b = vec![1u8];
    b.extend([0u8; 8]);
    for i in 0..n {
        b.push(2);
        b.extend((i as f32).to_le_bytes());
        b.extend(1.0f32.to_le_bytes());
    }
    b
}

fn shaped_outline(node: &Node) -> Vec<designos_doc::PathCmd> {
    let mut bare = node.clone();
    bare.fig_text_geometry = None;
    text_runs_for_node(&bare).unwrap().lines.into_iter().flat_map(|l| l.spans).flat_map(|s| s.outline).collect()
}

fn drawn_outline(node: &Node) -> Vec<designos_doc::PathCmd> {
    text_runs_for_node(node).unwrap().lines.into_iter().flat_map(|l| l.spans).flat_map(|s| s.outline).collect()
}

#[test]
fn four_thousand_glyphs_over_one_large_blob_are_refused_before_anything_is_assembled() {
    // 4,096 glyphs x 1 MB of blob would be about 4.8e8 commands, 13 GB, if the table were taken at its word
    let big = lines(116_000);
    assert!(big.len() > 1_000_000);
    let node = loaded_text((0..4096).map(|i| glyph(i as f32 * 0.01, 10.0, 12.0, 0)).collect(), vec![big], Path::default());
    assert_eq!(stored_glyph_use(&node), StoredGlyphUse::Fallback("stored glyphs over budget"));
    // nothing is assembled: the exporters and the canvas get the shaped text
    assert_eq!(drawn_outline(&node), shaped_outline(&node));
    assert_eq!(text_outlines_for_node(&node).len(), 1);
}

#[test]
fn more_glyphs_than_the_importer_builds_are_refused() {
    let node = loaded_text((0..4097).map(|_| glyph(0.0, 10.0, 12.0, StoredGlyph::NO_BLOB)).collect(), Vec::new(), Path::default());
    assert_eq!(stored_glyph_use(&node), StoredGlyphUse::Fallback("stored glyphs over budget"));
}

#[test]
fn a_non_finite_or_zero_size_table_is_refused_and_no_such_point_reaches_an_exporter() {
    for (what, bad) in [
        ("NaN x", glyph(f32::NAN, 10.0, 12.0, 0)),
        ("infinite x", glyph(f32::INFINITY, 10.0, 12.0, 0)),
        ("NaN y", glyph(0.0, f32::NAN, 12.0, 0)),
        ("NaN size", glyph(0.0, 10.0, f32::NAN, 0)),
        ("infinite size", glyph(0.0, 10.0, f32::INFINITY, 0)),
        ("zero size", glyph(0.0, 10.0, 0.0, 0)),
        ("negative size", glyph(0.0, 10.0, -12.0, 0)),
    ] {
        let node = loaded_text(vec![glyph(0.0, 10.0, 12.0, 0), bad], vec![lines(8)], Path::default());
        assert_eq!(stored_glyph_use(&node), StoredGlyphUse::Fallback("stored glyphs not finite"), "{what}");
        let outline = drawn_outline(&node);
        assert_eq!(outline, shaped_outline(&node), "{what}: the shaped text is exported");
        let finite = |p: &Vec2| p.x.is_finite() && p.y.is_finite();
        assert!(
            outline.iter().all(|c| match c {
                designos_doc::PathCmd::MoveTo(p) | designos_doc::PathCmd::LineTo(p) => finite(p),
                designos_doc::PathCmd::QuadTo(a, b) => finite(a) && finite(b),
                designos_doc::PathCmd::CubicTo(a, b, c) => finite(a) && finite(b) && finite(c),
                designos_doc::PathCmd::Close => true,
            }),
            "{what}"
        );
    }
}

#[test]
fn a_finite_table_whose_outline_overflows_once_placed_is_shaped_instead() {
    // each number is finite and positive; 1e38 x a blob coordinate of 1000 leaves the range of f32
    let node = loaded_text(
        vec![glyph(0.0, 10.0, 1.0e38, 0)],
        vec![lines(4).into_iter().chain([2]).chain(1000.0f32.to_le_bytes()).chain(1000.0f32.to_le_bytes()).collect()],
        Path::default(),
    );
    assert_eq!(stored_glyph_use(&node), StoredGlyphUse::Drawn, "the cheap checks pass");
    assert_eq!(drawn_outline(&node), shaped_outline(&node), "assembly refuses the overflow and the text is shaped");
}

#[test]
fn a_geometry_with_neither_table_nor_path_is_not_a_stored_text() {
    // 0.7.17 keeps no geometry at all from a document written by this build, so this is not a file it leaves behind; it is
    // a table and a path emptied by hand or by another writer
    let node = loaded_text(Vec::new(), Vec::new(), Path::default());
    assert_eq!(stored_glyph_use(&node), StoredGlyphUse::Fallback("empty stored outline"));
    assert_eq!(drawn_outline(&node), shaped_outline(&node), "the shaped outlines stay in the export");
    assert!(!shaped_outline(&node).is_empty());
}

#[test]
fn an_honest_table_still_draws() {
    let node = loaded_text(vec![glyph(0.0, 10.0, 12.0, 0), glyph(5.0, 10.0, 12.0, 0)], vec![lines(4)], Path::default());
    assert_eq!(stored_glyph_use(&node), StoredGlyphUse::Drawn);
    assert_eq!(drawn_outline(&node).len(), 2 * 5);
}
