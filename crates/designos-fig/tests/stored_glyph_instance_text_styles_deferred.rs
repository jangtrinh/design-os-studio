//! The style ids of the runs an override built for a nested copy go with the copy when an instance on a page that opens later
//! clones the component that holds it.
mod common;

use common::deferred_fig::*;
use common::fig_fixture::{fig_node, guid};
use designos_doc::{Document, Node, NodeId, NodeKind};
use designos_render::{stored_glyph_use, StoredGlyphUse};
use serde_json::json;

const WRAP: (u64, u64) = (1, 30);
const INNER: (u64, u64) = (1, 31);
const OUTER: (u64, u64) = (1, 40);

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

/// `Wrapper` and `Buttons/Button` sit on the first page; the wrapper holds an instance of the button whose override of the label
/// makes the last six characters an underlined run (id 5). An instance of the wrapper sits on the page that opens later, with
/// the data Figma stored for that label: its bar names id 5.
fn fixture_with_wrapper(underlined: bool) -> Vec<u8> {
    let mut label_override = json!({
        "guidPath": {"guids": [guid(LABEL.0, LABEL.1)]},
        "textData": {
            "characters": "Go back!",
            "characterStyleIDs": [0, 0, 5, 5, 5, 5, 5, 5],
            "styleOverrideTable": [{"styleID": 5, "textDecoration": "UNDERLINE"}]
        }
    });
    if !underlined {
        label_override["textData"].as_object_mut().unwrap().remove("characterStyleIDs");
        label_override["textData"].as_object_mut().unwrap().remove("styleOverrideTable");
    }
    let mut stored = json!({
        "guidPath": {"guids": [guid(INNER.0, INNER.1), guid(LABEL.0, LABEL.1)]},
        "size": {"x": 90.0, "y": 20.0},
        "derivedTextData": {
            "layoutSize": {"x": 90.0, "y": 20.0},
            "fontMetaData": [{"key": {"family": "Inter", "style": "Regular", "postscript": ""}}],
            "glyphs": [{"commandsBlob": 0, "position": {"x": 0.0, "y": 20.0}, "fontSize": 14.0, "firstCharacter": 0}],
            "decorations": [{"styleID": 5, "rects": [{"x": 20.0, "y": 19.0, "w": 50.0, "h": 1.0}]}]
        }
    });
    if !underlined {
        stored["derivedTextData"].as_object_mut().unwrap().remove("decorations");
    }
    let mut changes = vec![
        fig_node((0, 0), (0, 0), "DOCUMENT", "Document", json!({})),
        fig_node((0, 1), (0, 0), "CANVAS", "Cover", json!({})),
        fig_node((0, 2), (0, 0), "CANVAS", "Page 2", json!({})),
        fig_node((0, 3), (0, 0), "CANVAS", "404 pages", json!({})),
        fig_node(BUTTON, (0, 1), "SYMBOL", "Buttons/Button", json!({})),
        fig_node(LABEL, BUTTON, "TEXT", "Text", json!({"fontSize": 14.0, "textData": {"characters": "Button CTA"}})),
        fig_node(WRAP, (0, 1), "SYMBOL", "Wrapper", json!({})),
        fig_node(INNER, WRAP, "INSTANCE", "Inner button", json!({"symbolData": {"symbolID": guid(BUTTON.0, BUTTON.1), "symbolOverrides": [label_override]}})),
        fig_node(OUTER, (0, 3), "INSTANCE", "Outer wrapper", json!({"symbolData": {"symbolID": guid(WRAP.0, WRAP.1)}, "derivedSymbolData": [stored]})),
    ];
    changes.extend((0..25_000u64).map(|i| rect_nc((9, i + 1), (0, 1), "F")));
    fig_bytes(changes, vec![triangle()])
}

fn in_outer(doc: &Document, node: &Node) -> bool {
    let mut cur = node.parent;
    while let Some(p) = cur.and_then(|p| doc.get(p)) {
        if p.name == "Outer wrapper" {
            return true;
        }
        cur = p.parent;
    }
    false
}

fn outer_label(doc: &Document) -> NodeId {
    doc.nodes
        .values()
        .find(|n| matches!(&n.kind, NodeKind::Text(t) if t.content == "Go back!") && in_outer(doc, n))
        .map(|n| n.id)
        .expect("the label inside the outer instance")
}

#[test]
fn an_instance_on_a_page_that_opens_later_keeps_the_ids_of_the_runs_an_override_built_in_its_component() {
    let mut doc = designos_fig::import_fig(&fixture_with_wrapper(true)).unwrap();
    let page = page_named(&doc, "404 pages");
    assert!(!page_hydrated(&doc, page), "the page is deferred");
    assert!(doc.ensure_page_hydrated(page));
    let label = doc.get(outer_label(&doc)).unwrap();
    let NodeKind::Text(text) = &label.kind else { unreachable!() };
    assert_eq!(text.runs.len(), 1, "the override made one run");
    assert_eq!(stored_glyph_use(label), StoredGlyphUse::Drawn, "the stored text is read against the run ids of the override");
    let bars = &label.fig_text_geometry.as_ref().unwrap().decorations;
    assert_eq!(bars.iter().map(|b| b.style).collect::<Vec<_>>(), [1], "the bar belongs to the underlined run");
}

#[test]
fn the_same_instance_draws_when_the_override_built_no_runs() {
    let mut doc = designos_fig::import_fig(&fixture_with_wrapper(false)).unwrap();
    let page = page_named(&doc, "404 pages");
    assert!(doc.ensure_page_hydrated(page));
    let label = doc.get(outer_label(&doc)).unwrap();
    let NodeKind::Text(text) = &label.kind else { unreachable!() };
    assert!(text.runs.is_empty());
    assert_eq!(stored_glyph_use(label), StoredGlyphUse::Drawn, "the fixture itself draws: only the run ids differ");
}
