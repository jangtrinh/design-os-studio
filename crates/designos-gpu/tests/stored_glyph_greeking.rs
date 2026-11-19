//! Own test binary: it asserts exact values of the process-wide blob decode counter, which any other stored text drawn
//! in the process would move. Text greeking runs before any glyph geometry is built: a stored text too small to read
//! in a moving view draws as a bar and never builds its outline, while the compile of a still view builds it.
//! Compiling is CPU-only: no adapter needed.

use designos_doc::*;
use designos_gpu::SceneCompiler;
use designos_render::{stored_glyph_blob_decodes, Viewport};
use std::sync::Arc;
use std::time::Duration;

const SIZE: (u32, u32) = (400, 400);
/// Under the 0.45 zoom-out threshold, and the text's 10 px font is 2 device px.
const ZOOM: f32 = 0.2;

fn triangle_blob() -> Vec<u8> {
    let mut blob = Vec::new();
    for (op, x, y) in [(1u8, 0.0f32, 0.0f32), (2, 1.0, 0.0), (2, 0.5, 0.8)] {
        blob.push(op);
        blob.extend(x.to_le_bytes());
        blob.extend(y.to_le_bytes());
    }
    blob.push(0);
    blob
}

/// 20 x 20 stored texts in a font nobody has, at 10 px: 4 device px high and 2 px of font at `ZOOM`.
fn stored_texts() -> (Document, NodeId) {
    let mut doc = Document::new("stored texts");
    let page = doc.add_page("Page");
    for i in 0..6 {
        let id = doc.alloc_id();
        let style = TextStyle { font_family: "MissingFaceForGreekingTest".into(), font_size: 10.0, ..TextStyle::default() };
        let mut node = Node::text(id, Rect::new(20.0 + i as f32 * 40.0, 20.0, 20.0, 20.0), "\u{e900}", style);
        let NodeKind::Text(text) = &node.kind else { unreachable!() };
        node.fig_text_geometry = Some(Arc::new(FigTextGeometry {
            text: text.clone(),
            size: node.size,
            path: Path::default(),
            blobs: vec![Arc::from(triangle_blob())],
            glyph_blobs: Vec::new(),
            glyphs: vec![StoredGlyph { x: 0.0, y: 20.0, font_size: 10.0, blob: 0, first_char: 0, style: 0, flags: 0 }].into(),
            ..Default::default()
        }));
        doc.insert(page, node).unwrap();
    }
    (doc, page)
}

/// Compiles until no node keeps deferred geometry.
fn settle(c: &mut SceneCompiler, doc: &Document, page: NodeId) {
    let vp = Viewport { offset: Vec2::new(0.0, 0.0), zoom: ZOOM, dpr: 1.0 };
    for _ in 0..200 {
        c.compile(doc, page, 1, &vp, SIZE);
        if c.geom.stale_hits == 0 {
            return;
        }
    }
    panic!("geometry still deferred after 200 compiles");
}

#[test]
fn greeking_returns_before_a_stored_text_builds_any_geometry() {
    let (doc, page) = stored_texts();
    let before = stored_glyph_blob_decodes();

    let mut moving = SceneCompiler::new();
    moving.geom.budget = Some(Duration::from_millis(8));
    settle(&mut moving, &doc, page);
    assert_eq!(stored_glyph_blob_decodes(), before, "an interactive compile greeks the texts and decodes no blob");

    let mut still = SceneCompiler::new();
    still.geom.budget = Some(Duration::from_millis(8));
    still.opts.full_detail = true;
    settle(&mut still, &doc, page);
    assert!(stored_glyph_blob_decodes() > before, "the compile of a still view builds the outlines, so the counter sees them");
}
