//! A `.fig` text that carries Figma's stored glyphs keeps its stored size when auto layout is solved again
//! with the app's text measurer installed, and still draws those glyphs. An edited text is shaped instead.
#[path = "common/missing_font_fig.rs"]
mod missing_font_fig;

use designos_doc::text_measure::{TextMeasure, ThreadLayoutMeasurerGuard};
use designos_doc::{Document, NodeId, NodeKind, Vec2};
use designos_fig::import_fig;
use designos_fig::kiwi::KiwiValue as V;
use designos_fig::testkit::{en, node, vec2};
use designos_render::RenderTextMeasure;
use missing_font_fig::*;
use std::sync::Arc;

/// A vertical frame as wide as the text, holding a stretched, auto-height text: the measurer is asked for its
/// height at the frame's width on every layout.
fn fixture() -> Vec<u8> {
    let mut nodes = document_and_page();
    nodes.push(node(
        (1, 1),
        (0, 1),
        "a",
        "FRAME",
        "Column",
        vec![("size", vec2(20.0, 20.0)), ("stackMode", en("VERTICAL")), ("stackPrimarySizing", en("FIXED")), ("stackCounterSizing", en("FIXED"))],
    ));
    let extra: Vec<(&str, V)> = vec![("textAutoResize", en("HEIGHT")), ("stackChildAlignSelf", en("STRETCH"))];
    nodes.push(glyph_text((1, 2), (1, 1), "a", "Icon text", FAMILY, "\u{e900}", &[(0, 0.0)], extra));
    fig_file(nodes, vec![icon_blob()])
}

fn ids(doc: &Document) -> (NodeId, NodeId) {
    let column = doc.nodes.values().find(|n| n.name == "Column").unwrap().id;
    let text = doc.nodes.values().find(|n| n.name == "Icon text").unwrap().id;
    (column, text)
}

fn draws_saved_triangle(doc: &Document, id: NodeId) -> bool {
    designos_render::text_outlines_for_node(doc.get(id).unwrap()).iter().any(|o| {
        let b = o.path.bounds();
        (b.left() - 0.0).abs() < 1e-4 && (b.right() - 20.0).abs() < 1e-4 && (b.top() - 4.0).abs() < 1e-4 && (b.bottom() - 20.0).abs() < 1e-4
    })
}

#[test]
fn imported_text_keeps_its_stored_size_and_glyphs_through_a_relayout_with_the_text_measurer() {
    let mut doc = import_fig(&fixture()).unwrap();
    let (column, text) = ids(&doc);
    let NodeKind::Text(tp) = &doc.get(text).unwrap().kind else { panic!("a text") };
    let shaped = RenderTextMeasure.measure(tp, Some(20.0));
    assert!(shaped.y != 20.0, "the measurer must disagree with the stored height for this test to say anything, got {shaped:?}");
    assert!(draws_saved_triangle(&doc, text));

    let _guard = ThreadLayoutMeasurerGuard::install(Arc::new(RenderTextMeasure));
    doc.relayout_subtree(column);
    let n = doc.get(text).unwrap();
    assert_eq!(n.size, Vec2::new(20.0, 20.0), "auto layout keeps the stored size");
    assert!(n.fig_text_geometry.as_ref().is_some_and(|g| g.matches(n)), "the stored glyphs still describe the text");
    assert!(draws_saved_triangle(&doc, text));
}

#[test]
fn edited_text_is_shaped_by_a_relayout_and_drops_the_stored_glyphs() {
    let mut doc = import_fig(&fixture()).unwrap();
    let (column, text) = ids(&doc);
    let Some(NodeKind::Text(tp)) = doc.get_mut(text).map(|n| &mut n.kind) else { panic!("a text") };
    tp.content = "\u{e900}\u{e900}".into();

    let _guard = ThreadLayoutMeasurerGuard::install(Arc::new(RenderTextMeasure));
    doc.relayout_subtree(column);
    let n = doc.get(text).unwrap();
    assert!(n.size.y != 20.0, "an edited text takes the measured height, got {:?}", n.size);
    assert!(n.fig_text_geometry.as_ref().is_none_or(|g| !g.matches(n)));
    assert!(!draws_saved_triangle(&doc, text));
}
