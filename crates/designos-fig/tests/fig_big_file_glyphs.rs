//! A big `.fig` file (more than 5,000 entries, or a message payload over 2 MB) keeps the glyph outlines Figma stored for a text whose font is not installed, the way a small
//! file does, and keeps nothing for a text in a bundled family (it always shapes with the bundled face).
#[path = "common/missing_font_fig.rs"]
mod missing_font_fig;

use designos_doc::{Document, Node, NodeKind};
use designos_fig::kiwi::KiwiValue as V;
use designos_fig::testkit::{msg, node, vec2};
use designos_fig::{decode_fig_to_json, export_fig, import_fig, import_fig_with_report, import_fig_with_stored_glyph_allowance};
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

/// Both ways a file is big at once: more than 5,000 entries and a payload over 2 MB.
fn big_by_both(texts: Vec<V>) -> Vec<u8> {
    let mut nodes = document_and_page();
    nodes.extend(texts);
    nodes.extend((1..=5_001u32).map(|i| node((2, i), (0, 1), "z", "FRAME", "Filler", vec![("size", vec2(4.0, 4.0))])));
    fig_file(nodes, vec![icon_blob(), vec![0u8; 2_100_000]])
}

/// Replaces the value of the field `name` of the node change `change`.
fn set_field(change: &mut V, name: &str, value: V) {
    let V::Message(fields) = change else { unreachable!() };
    fields.iter_mut().find(|(k, _)| &**k == name).expect("the field exists").1 = value;
}

/// A text in the missing font whose second character carries a style run.
fn styled_text() -> V {
    let mut text = glyph_text((1, 6), (0, 1), "d", "Styled text", FAMILY, "\u{e900}\u{e900}", &[(0, 0.0), (0, 10.0)], vec![]);
    let run = msg(vec![("styleID", V::Uint(1)), ("fontSize", designos_fig::testkit::fl(12.0))]);
    let data = msg(vec![
        ("characters", V::String("\u{e900}\u{e900}".into())),
        ("characterStyleIDs", V::Array(vec![V::Uint(0), V::Uint(1)])),
        ("styleOverrideTable", V::Array(vec![run])),
    ]);
    set_field(&mut text, "textData", data);
    text
}

/// A text in the missing font whose derived data names two fonts.
fn two_font_text() -> V {
    let mut text = glyph_text((1, 7), (0, 1), "e", "Two font text", FAMILY, "\u{e900}", &[(0, 0.0)], vec![]);
    let derived = match &text {
        V::Message(fields) => fields.iter().find(|(k, _)| &**k == "derivedTextData").unwrap().1.clone(),
        _ => unreachable!(),
    };
    let mut derived = derived;
    set_field(&mut derived, "fontMetaData", V::Array(vec![msg(vec![("key", font_key(FAMILY))]), msg(vec![("key", font_key("Another Missing Face"))])]));
    set_field(&mut text, "derivedTextData", derived);
    text
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
        vec![
            ("size", vec2(20.0, 20.0)),
            // a font that is not bundled, so only the node's type keeps this out
            ("fontName", font_key(FAMILY)),
            ("derivedTextData", msg(vec![("layoutSize", vec2(20.0, 20.0)), ("fontMetaData", V::Array(vec![msg(vec![("key", font_key(FAMILY))])]))])),
        ],
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

/// Every shape a file can be big in, with the name the failure message gives it.
fn big_shapes(texts: &dyn Fn() -> Vec<V>) -> Vec<(&'static str, Vec<u8>)> {
    vec![("entry count", big_by_entry_count(texts())), ("payload", big_by_payload(texts())), ("both", big_by_both(texts()))]
}

#[test]
fn big_file_keeps_only_texts_that_can_draw_their_stored_glyphs() {
    for (shape, bytes) in big_shapes(&|| vec![missing_text(), styled_text(), two_font_text()]) {
        let doc = import_fig(&bytes).unwrap();
        assert!(holds_derived_text(named(&doc, "Missing font text")), "big by {shape}: the control text keeps its data");
        assert!(!holds_derived_text(named(&doc, "Styled text")), "big by {shape}: a text with a style run is never drawn from stored glyphs");
        assert!(!holds_derived_text(named(&doc, "Two font text")), "big by {shape}: nor is a text whose derived data names two fonts");
    }
}

/// Texts in the missing font that `embedded_glyphs` or `fig_text_geometry` would never draw, each under its own name.
fn undrawable_texts() -> Vec<V> {
    let many = glyph_text((1, 12), (0, 1), "g", "Too many glyphs", FAMILY, "\u{e900}", &vec![(0, 0.0); 4097], vec![]);
    let mut other_family = glyph_text((1, 13), (0, 1), "h", "Other family", FAMILY, "\u{e900}", &[(0, 0.0)], vec![]);
    let V::Message(fields) = &mut other_family else { unreachable!() };
    let derived = fields.iter().find(|(k, _)| &**k == "derivedTextData").unwrap().1.clone();
    let mut derived = derived;
    set_field(&mut derived, "fontMetaData", V::Array(vec![msg(vec![("key", font_key("Another Missing Face"))])]));
    set_field(&mut other_family, "derivedTextData", derived);
    // Figma writes fields by ascending number, and `textDecoration` (35) comes before `derivedTextData` (359)
    let mut underlined = glyph_text((1, 14), (0, 1), "i", "Underlined", FAMILY, "\u{e900}", &[(0, 0.0)], vec![]);
    let V::Message(fields) = &mut underlined else { unreachable!() };
    let at = fields.iter().position(|(k, _)| &**k == "derivedTextData").unwrap();
    fields.insert(at, ("textDecoration".into(), designos_fig::testkit::en("UNDERLINE")));
    let mut resized = glyph_text((1, 15), (0, 1), "j", "Wrong size", FAMILY, "\u{e900}", &[(0, 0.0)], vec![]);
    set_field(&mut resized, "size", vec2(30.0, 20.0));
    vec![many, other_family, underlined, resized]
}

#[test]
fn big_file_keeps_nothing_for_a_text_the_drawing_code_would_refuse() {
    for (shape, bytes) in big_shapes(&|| {
        let mut texts = vec![missing_text()];
        texts.extend(undrawable_texts());
        texts
    }) {
        let doc = import_fig(&bytes).unwrap();
        assert!(holds_derived_text(named(&doc, "Missing font text")), "big by {shape}: the control text keeps its data");
        for name in ["Too many glyphs", "Other family", "Underlined", "Wrong size"] {
            assert!(!holds_derived_text(named(&doc, name)), "big by {shape}: {name} would never draw from stored glyphs");
        }
    }
}

#[test]
fn a_small_file_still_keeps_the_data_of_a_styled_text() {
    let doc = import_fig(&small(vec![styled_text()])).unwrap();
    assert!(holds_derived_text(named(&doc, "Styled text")), "a small file keeps every text's data, as before");
}

#[test]
fn the_import_report_counts_texts_the_allowance_leaves_out() {
    // a text that is bundled or styled is never counted: it would not have been kept
    let texts = || vec![missing_text(), bundled_text(), styled_text()];
    for (shape, bytes) in [("payload", big_by_payload(texts())), ("both", big_by_both(texts()))] {
        let (doc, report) = import_fig_with_report(&bytes).unwrap();
        assert_eq!(report.dropped_stored_glyphs, 0, "big by {shape}: the default allowance holds one small text");
        assert!(named(&doc, "Missing font text").fig_text_geometry.is_some());

        let (doc, report) = import_fig_with_stored_glyph_allowance(&bytes, 0).unwrap();
        assert_eq!(report.dropped_stored_glyphs, 1, "big by {shape}: with no allowance the one drawable text is counted");
        assert!(named(&doc, "Missing font text").fig_text_geometry.is_none() && !holds_derived_text(named(&doc, "Missing font text")));
    }
    // a payload under 2 MB is bounded by its own size: the decoder skips nothing, so there is nothing to count
    let (doc, report) = import_fig_with_stored_glyph_allowance(&big_by_entry_count(texts()), 0).unwrap();
    assert_eq!(report.dropped_stored_glyphs, 0);
    assert!(named(&doc, "Missing font text").fig_text_geometry.is_some(), "under 2 MB the allowance does not apply");
}

#[test]
fn the_allowance_is_spent_in_file_order() {
    let second = glyph_text((1, 4), (0, 1), "c", "Second missing text", FAMILY, "\u{e900}", &[(0, 0.0)], vec![]);
    let bytes = big_by_payload(vec![missing_text(), second]);
    let report_at = |allowance: usize| import_fig_with_stored_glyph_allowance(&bytes, allowance).unwrap();
    // the smallest allowance that keeps both, found by search so the test does not hard-code a wire size
    let both = (0..100_000).find(|&a| report_at(a).1.dropped_stored_glyphs == 0).expect("both texts fit some allowance");
    let (doc, report) = report_at(both - 1);
    assert_eq!(report.dropped_stored_glyphs, 1);
    assert!(named(&doc, "Missing font text").fig_text_geometry.is_some(), "the first text in the file is kept");
    assert!(named(&doc, "Second missing text").fig_text_geometry.is_none(), "the one that no longer fits is left out");
}

/// A component with a missing-font text, copied by `copies` instances. With `fillers` the file has more than 5,000
/// entries and fewer than 10,000 nodes, the size at which instance copies carry their source's metadata along; without
/// them it has a handful of entries and the payload passes 2 MB through a padding blob instead.
fn component_with_copies(copies: u32, fillers: bool) -> Vec<u8> {
    let mut nodes = document_and_page();
    nodes.push(node((1, 10), (0, 1), "a", "SYMBOL", "Icon main", vec![("size", vec2(20.0, 20.0))]));
    nodes.push(glyph_text((1, 11), (1, 10), "a", "Source text", FAMILY, "\u{e900}", &[(0, 0.0)], vec![]));
    for i in 0..copies {
        nodes.push(node(
            (1, 20 + i),
            (0, 1),
            "b",
            "INSTANCE",
            &format!("Copy {i}"),
            vec![("size", vec2(20.0, 20.0)), ("symbolData", msg(vec![("symbolID", designos_fig::testkit::guid(1, 10))]))],
        ));
    }
    if fillers {
        nodes.extend((1..=5_001u32).map(|i| node((2, i), (0, 1), "z", "FRAME", "Filler", vec![("size", vec2(4.0, 4.0))])));
        fig_file(nodes, vec![icon_blob()])
    } else {
        fig_file(nodes, vec![icon_blob(), vec![0u8; 2_100_000]])
    }
}

fn assert_copies_carry_no_json(doc: &Document, what: &str) {
    let texts: Vec<&Node> = doc.nodes.values().filter(|n| matches!(n.kind, NodeKind::Text(_))).collect();
    assert_eq!(texts.len(), 7, "{what}: the source and six copies");
    assert_eq!(texts.iter().filter(|n| holds_derived_text(n)).count(), 1, "{what}: only the source holds the stored JSON");
    let geometries: Vec<_> = texts.iter().map(|n| n.fig_text_geometry.as_ref().expect("every copy draws the stored glyphs")).collect();
    assert!(geometries.iter().all(|g| Arc::ptr_eq(g, geometries[0])), "{what}: the copies share the source's geometry");
}

#[test]
fn instance_copies_of_a_text_share_its_glyphs_and_carry_no_copy_of_its_json() {
    let doc = import_fig(&component_with_copies(6, true)).unwrap();
    assert!(doc.nodes.len() > 5_000 && doc.nodes.len() < 10_000, "the fixture is in the size where copies keep their metadata ({} nodes)", doc.nodes.len());
    assert_copies_carry_no_json(&doc, "more than 5,000 entries");
}

#[test]
fn instance_copies_carry_no_json_when_only_the_payload_is_heavy() {
    // 5,000 entries or fewer and a payload over 2 MB: the decoder keeps within its allowance, and copies must not multiply it
    let doc = import_fig(&component_with_copies(6, false)).unwrap();
    assert!(doc.nodes.len() < 5_000);
    assert_copies_carry_no_json(&doc, "payload over 2 MB");
}

#[test]
fn instance_copies_carry_no_json_in_a_heavy_paste() {
    let clip = designos_fig::FigClipboard { meta: None, buffer: component_with_copies(6, false) };
    let doc = designos_fig::import_figma_clipboard(&clip).unwrap();
    assert_copies_carry_no_json(&doc, "paste over 2 MB");
}

#[test]
fn a_heavy_paste_still_keeps_the_stored_glyphs_of_a_text_that_can_draw_them() {
    let clip = designos_fig::FigClipboard { meta: None, buffer: big_by_payload(vec![missing_text(), bundled_text()]) };
    let doc = designos_fig::import_figma_clipboard(&clip).unwrap();
    assert_draws_stored_glyph(&doc, "Missing font text");
    assert!(!holds_derived_text(named(&doc, "Bundled font text")), "a bundled family keeps nothing in a paste either");
}

#[test]
fn a_small_file_keeps_the_json_of_instance_copies() {
    // small files are unchanged: a copy keeps the metadata of its source, as before
    let mut nodes = document_and_page();
    nodes.push(node((1, 10), (0, 1), "a", "SYMBOL", "Icon main", vec![("size", vec2(20.0, 20.0))]));
    nodes.push(glyph_text((1, 11), (1, 10), "a", "Source text", FAMILY, "\u{e900}", &[(0, 0.0)], vec![]));
    nodes.push(node(
        (1, 20),
        (0, 1),
        "b",
        "INSTANCE",
        "Copy",
        vec![("size", vec2(20.0, 20.0)), ("symbolData", msg(vec![("symbolID", designos_fig::testkit::guid(1, 10))]))],
    ));
    let doc = import_fig(&fig_file(nodes, vec![icon_blob()])).unwrap();
    let held = doc.nodes.values().filter(|n| matches!(n.kind, NodeKind::Text(_)) && holds_derived_text(n)).count();
    assert_eq!(held, 2, "the source and its copy");
}

#[test]
fn big_file_saves_back_stored_glyphs_it_read_from_a_payload_over_2_mb() {
    // The saved file must also be read back the way a heavy file is: `type` and `fontName` have to come before
    // `derivedTextData` in what Studio writes. A node name pads both the first import and the saved file past 2 MB.
    let padding = node((1, 9), (0, 1), "f", "FRAME", &"x".repeat(2_100_000), vec![("size", vec2(4.0, 4.0))]);
    let doc = import_fig(&small(vec![missing_text(), padding])).unwrap();
    let saved = export_fig(&doc).unwrap();
    let reopened = import_fig(&saved).unwrap();
    assert!(named(&reopened, "Missing font text").fig_text_geometry.is_some(), "the glyphs survive a save read back as a heavy file");
    assert_eq!(named(&reopened, "Missing font text").fig_text_geometry, named(&doc, "Missing font text").fig_text_geometry);
    assert_draws_stored_glyph(&reopened, "Missing font text");
}
