//! A large `.fig` file keeps the glyph outlines Figma stored for a text whose font is not installed, the way a small
//! file does, and keeps nothing for a text in a bundled family (it always shapes with the bundled face).
#[path = "common/missing_font_fig.rs"]
mod missing_font_fig;

use designos_doc::{Document, Node, NodeKind};
use designos_fig::kiwi::KiwiValue as V;
use designos_fig::testkit::{msg, node, vec2};
use designos_fig::{decode_fig_to_json, export_fig, import_fig};
use missing_font_fig::*;
use std::sync::Arc;

/// More node changes than the importer's large-file threshold (5,000), with a message payload far below 2 MB.
fn large_by_entry_count(texts: Vec<V>) -> Vec<u8> {
    let mut nodes = document_and_page();
    nodes.extend(texts);
    nodes.extend((1..=5_001u32).map(|i| node((2, i), (0, 1), "z", "FRAME", "Filler", vec![("size", vec2(4.0, 4.0))])));
    fig_file(nodes, vec![icon_blob()])
}

/// A handful of node changes whose message payload passes 2 MB (a padding blob), the other way a file is large.
fn large_by_payload(texts: Vec<V>) -> Vec<u8> {
    let mut nodes = document_and_page();
    nodes.extend(texts);
    fig_file(nodes, vec![icon_blob(), vec![0u8; 2_100_000]])
}

fn small(texts: Vec<V>) -> Vec<u8> {
    let mut nodes = document_and_page();
    nodes.extend(texts);
    fig_file(nodes, vec![icon_blob()])
}

fn missing_text() -> V {
    glyph_text((1, 2), (0, 1), "a", "Missing font text", FAMILY, "\u{e900}", &[(0, 0.0)], vec![])
}

fn bundled_text() -> V {
    glyph_text((1, 3), (0, 1), "b", "Bundled font text", "Inter", "\u{e900}", &[(0, 0.0)], vec![])
}

fn named<'a>(doc: &'a Document, name: &str) -> &'a Node {
    doc.nodes.values().find(|n| n.name == name).unwrap_or_else(|| panic!("no node named {name}"))
}

fn holds_derived_text(node: &Node) -> bool {
    node.fig_metadata.contains_key("derivedTextData")
}

/// Whether any outline drawn for `node` has the bounds of the fixture's saved triangle (x 0..20, y 4..20).
fn draws_saved_triangle(node: &Node) -> bool {
    designos_render::text_outlines_for_node(node).iter().any(|o| {
        let b = o.path.bounds();
        (b.left() - 0.0).abs() < 1e-4 && (b.right() - 20.0).abs() < 1e-4 && (b.top() - 4.0).abs() < 1e-4 && (b.bottom() - 20.0).abs() < 1e-4
    })
}

/// Alpha of the pixel at (x, y) of the text's own render at scale 1: inside the saved triangle and outside it.
fn alpha_at(doc: &Document, id: designos_doc::NodeId, x: u32, y: u32) -> u8 {
    let pm = designos_render::render_node(doc, id, 1.0).expect("the text renders");
    pm.pixel(x, y).expect("pixel in range").alpha()
}

fn assert_draws_stored_glyph(doc: &Document, name: &str) {
    let text = named(doc, name);
    let saved = text.fig_text_geometry.as_ref().unwrap_or_else(|| panic!("{name} keeps Figma's glyphs"));
    assert!(saved.matches(text));
    assert!(draws_saved_triangle(text), "{name} draws the saved outline");
    assert!(alpha_at(doc, text.id, 10, 16) > 200, "the triangle's inside is painted");
    assert_eq!(alpha_at(doc, text.id, 1, 6), 0, "outside the triangle stays empty");
}

#[test]
fn large_file_by_entry_count_draws_stored_glyphs_of_a_missing_font() {
    let doc = import_fig(&large_by_entry_count(vec![missing_text()])).unwrap();
    assert!(doc.nodes.len() > 5_000, "the fixture is a large file");
    assert_draws_stored_glyph(&doc, "Missing font text");
}

#[test]
fn large_file_by_payload_draws_stored_glyphs_of_a_missing_font() {
    let doc = import_fig(&large_by_payload(vec![missing_text()])).unwrap();
    assert_draws_stored_glyph(&doc, "Missing font text");
}

#[test]
fn large_file_keeps_nothing_for_a_text_in_a_bundled_family() {
    for (what, bytes) in
        [("entry count", large_by_entry_count(vec![missing_text(), bundled_text()])), ("payload", large_by_payload(vec![missing_text(), bundled_text()]))]
    {
        let doc = import_fig(&bytes).unwrap();
        let inter = named(&doc, "Bundled font text");
        assert!(inter.fig_text_geometry.is_none(), "large by {what}: a bundled family keeps no geometry");
        assert!(!holds_derived_text(inter), "large by {what}: a bundled family keeps no stored derived data");
        assert!(holds_derived_text(named(&doc, "Missing font text")), "large by {what}: the missing font keeps it");
    }
}

#[test]
fn small_file_keeps_stored_glyphs_for_every_text() {
    let doc = import_fig(&small(vec![missing_text(), bundled_text()])).unwrap();
    for name in ["Missing font text", "Bundled font text"] {
        let text = named(&doc, name);
        assert!(text.fig_text_geometry.is_some(), "{name}: a small file keeps the geometry");
        assert!(holds_derived_text(text), "{name}: a small file keeps the derived data");
    }
    assert_draws_stored_glyph(&doc, "Missing font text");
}

#[test]
fn large_file_shares_one_blob_between_texts_that_draw_the_same_glyph() {
    let second = glyph_text((1, 4), (0, 1), "c", "Second missing text", FAMILY, "\u{e900}", &[(0, 0.0)], vec![]);
    for bytes in [large_by_entry_count(vec![missing_text(), second.clone()]), large_by_payload(vec![missing_text(), second])] {
        let doc = import_fig(&bytes).unwrap();
        let a = named(&doc, "Missing font text").fig_text_geometry.as_ref().expect("first keeps glyphs");
        let b = named(&doc, "Second missing text").fig_text_geometry.as_ref().expect("second keeps glyphs");
        assert!(!Arc::ptr_eq(a, b), "each text has its own geometry");
        assert!(Arc::ptr_eq(&a.blobs[0], &b.blobs[0]), "one file blob is held once for both texts");
    }
}

fn derived_text_in(saved: &[u8], name: &str) -> Option<serde_json::Value> {
    let changes = decode_fig_to_json(saved).unwrap();
    changes["message"]["nodeChanges"].as_array().unwrap().iter().find(|n| n["name"] == name).and_then(|n| n.get("derivedTextData")).cloned()
}

#[test]
fn large_file_saves_back_the_stored_glyphs_it_kept() {
    let doc = import_fig(&large_by_entry_count(vec![missing_text(), bundled_text()])).unwrap();
    let saved = export_fig(&doc).unwrap();
    let derived = derived_text_in(&saved, "Missing font text").expect("a kept text writes its derived data back");
    assert_eq!(derived["glyphs"].as_array().map(Vec::len), Some(1));
    assert!(derived_text_in(&saved, "Bundled font text").is_none(), "a bundled-family text writes none, as before");

    let reopened = import_fig(&saved).unwrap();
    assert_eq!(named(&reopened, "Missing font text").fig_text_geometry, named(&doc, "Missing font text").fig_text_geometry);
    assert_draws_stored_glyph(&reopened, "Missing font text");
}

#[test]
fn derived_text_data_on_a_node_that_is_not_text_is_dropped_in_a_large_file() {
    let frame = node(
        (1, 9),
        (0, 1),
        "d",
        "FRAME",
        "Frame with derived text",
        vec![("size", vec2(20.0, 20.0)), ("derivedTextData", msg(vec![("layoutSize", vec2(20.0, 20.0))]))],
    );
    let doc = import_fig(&large_by_payload(vec![frame])).unwrap();
    let frame = named(&doc, "Frame with derived text");
    assert!(matches!(frame.kind, NodeKind::Frame(_)));
    assert!(!holds_derived_text(frame));
}
