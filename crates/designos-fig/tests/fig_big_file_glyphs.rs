//! A big `.fig` file (more than 5,000 entries, or a message payload over 2 MB) keeps the glyph outlines Figma stored for a text whose font is not installed, the way a small
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
fn big_by_entry_count(texts: Vec<V>) -> Vec<u8> {
    let mut nodes = document_and_page();
    nodes.extend(texts);
    nodes.extend((1..=5_001u32).map(|i| node((2, i), (0, 1), "z", "FRAME", "Filler", vec![("size", vec2(4.0, 4.0))])));
    fig_file(nodes, vec![icon_blob()])
}

/// A handful of node changes whose message payload passes 2 MB (a padding blob), the other way a file is large.
fn big_by_payload(texts: Vec<V>) -> Vec<u8> {
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
fn big_file_by_entry_count_draws_stored_glyphs_of_a_missing_font() {
    let doc = import_fig(&big_by_entry_count(vec![missing_text()])).unwrap();
    assert!(doc.nodes.len() > 5_000, "the fixture is a big file");
    assert_draws_stored_glyph(&doc, "Missing font text");
}

#[test]
fn big_file_by_payload_draws_stored_glyphs_of_a_missing_font() {
    let doc = import_fig(&big_by_payload(vec![missing_text()])).unwrap();
    assert_draws_stored_glyph(&doc, "Missing font text");
}

#[test]
fn big_file_keeps_nothing_for_a_text_in_a_bundled_family() {
    for (what, bytes) in
        [("entry count", big_by_entry_count(vec![missing_text(), bundled_text()])), ("payload", big_by_payload(vec![missing_text(), bundled_text()]))]
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
fn big_file_shares_one_blob_between_texts_that_draw_the_same_glyph() {
    let second = glyph_text((1, 4), (0, 1), "c", "Second missing text", FAMILY, "\u{e900}", &[(0, 0.0)], vec![]);
    for bytes in [big_by_entry_count(vec![missing_text(), second.clone()]), big_by_payload(vec![missing_text(), second])] {
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
fn big_file_saves_back_the_stored_glyphs_it_kept() {
    let doc = import_fig(&big_by_entry_count(vec![missing_text(), bundled_text()])).unwrap();
    let saved = export_fig(&doc).unwrap();
    let derived = derived_text_in(&saved, "Missing font text").expect("a kept text writes its derived data back");
    assert_eq!(derived["glyphs"].as_array().map(Vec::len), Some(1));
    assert!(derived_text_in(&saved, "Bundled font text").is_none(), "a bundled-family text writes none, as before");

    let reopened = import_fig(&saved).unwrap();
    assert_eq!(named(&reopened, "Missing font text").fig_text_geometry, named(&doc, "Missing font text").fig_text_geometry);
    assert_draws_stored_glyph(&reopened, "Missing font text");
}

#[test]
fn derived_text_data_on_a_node_that_is_not_text_is_dropped_in_a_big_file() {
    let frame = node(
        (1, 9),
        (0, 1),
        "d",
        "FRAME",
        "Frame with derived text",
        vec![("size", vec2(20.0, 20.0)), ("derivedTextData", msg(vec![("layoutSize", vec2(20.0, 20.0))]))],
    );
    let doc = import_fig(&big_by_payload(vec![frame])).unwrap();
    let frame = named(&doc, "Frame with derived text");
    assert!(matches!(frame.kind, NodeKind::Frame(_)));
    assert!(!holds_derived_text(frame));
}

/// The index each saved `commandsBlob` of the node named `name` has in `saved`'s blob table.
fn fill_blob_index(saved: &[u8], name: &str) -> u64 {
    let changes = decode_fig_to_json(saved).unwrap();
    let node = changes["message"]["nodeChanges"].as_array().unwrap().iter().find(|n| n["name"] == name).unwrap().clone();
    node["fillGeometry"][0]["commandsBlob"].as_u64().expect("the vector writes a fill blob")
}

#[test]
fn saving_stored_glyphs_never_renumbers_another_blob() {
    // The text is written before the vector. Blobs that other fields carry by number (the original file's numbers inside
    // an instance's overrides) must not move because a glyph was written.
    let mut nodes = document_and_page();
    nodes.push(missing_text());
    nodes.push(node(
        (1, 5),
        (0, 1),
        "b",
        "VECTOR",
        "Square",
        vec![
            ("size", vec2(10.0, 10.0)),
            ("fillPaints", V::Array(vec![designos_fig::testkit::solid(0.0, 0.0, 0.0)])),
            ("fillGeometry", V::Array(vec![msg(vec![("windingRule", designos_fig::testkit::en("NONZERO")), ("commandsBlob", V::Uint(1))])])),
        ],
    ));
    let doc = import_fig(&fig_file(nodes, vec![icon_blob(), designos_fig::testkit::square_blob()])).unwrap();
    let with_glyphs = export_fig(&doc).unwrap();
    let mut bare = doc.clone();
    let id = named(&bare, "Missing font text").id;
    bare.get_mut(id).unwrap().fig_text_geometry = None;
    let without_glyphs = export_fig(&bare).unwrap();

    assert_eq!(fill_blob_index(&with_glyphs, "Square"), fill_blob_index(&without_glyphs, "Square"), "the vector keeps its blob number");
    let glyph_slot = derived_text_in(&with_glyphs, "Missing font text").unwrap()["glyphs"][0]["commandsBlob"].as_u64().unwrap();
    let table = decode_fig_to_json(&without_glyphs).unwrap()["message"]["blobs"].as_array().map_or(0, Vec::len) as u64;
    assert_eq!(glyph_slot, table, "the glyph blob is the first one behind the rest of the table");
}
