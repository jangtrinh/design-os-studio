//! A text with style runs keeps Figma's glyphs: the importer puts each glyph in the run that holds its first character,
//! each stored bar in the run Figma gave its style id, and records the ids Figma used for the runs.
#[path = "common/missing_font_fig.rs"]
mod missing_font_fig;

use designos_doc::{Document, Node, NodeKind, StoredDecoration};
use designos_fig::kiwi::KiwiValue as V;
use designos_fig::testkit::en;
use designos_fig::{decode_fig_to_json, export_fig, import_fig};
use missing_font_fig::*;

fn named<'a>(doc: &'a Document, name: &str) -> &'a Node {
    doc.nodes.values().find(|n| n.name == name).unwrap_or_else(|| panic!("no node named {name}"))
}

#[test]
fn each_glyph_gets_the_run_that_holds_its_first_character() {
    let doc = import_fig(&two_run_fixture()).unwrap();
    let text = named(&doc, "Two runs");
    let saved = text.fig_text_geometry.as_ref().expect("a two-font, two-run text keeps its glyphs");
    assert!(saved.matches(text) && saved.fallback.is_none());
    assert_eq!(saved.glyphs.iter().map(|g| g.style).collect::<Vec<_>>(), vec![0, 0, 1, 1], "the run is style 1");
    assert_eq!(&*saved.run_sids, &[7], "Figma's id for the run, not Studio's number");
    assert_eq!(&*saved.decorations, &[StoredDecoration { style: 1, x: 20.0, y: 22.0, w: 20.0, h: 2.0 }], "the bar goes to the run that has its style id");
}

#[test]
fn a_character_outside_the_basic_plane_is_two_units_for_the_glyph_index() {
    // "a", a smiley (two UTF-16 units), "b": the run covers the smiley and "b"
    let mut nodes = document_and_page();
    nodes.push(styled_glyph_text(
        (1, 2),
        (0, 1),
        "a",
        "Surrogates",
        FAMILY,
        "a\u{1F600}b",
        &[0, 7, 7, 7],
        vec![style_override(7, FAMILY, (1.0, 0.0, 0.0), false)],
        vec![stored_glyph(Some(0), 0.0, 0, vec![]), stored_glyph(Some(0), 10.0, 1, vec![]), stored_glyph(Some(0), 20.0, 3, vec![])],
        &[FAMILY],
        vec![],
        vec![],
    ));
    let doc = import_fig(&fig_file(nodes, vec![icon_blob()])).unwrap();
    let saved = named(&doc, "Surrogates").fig_text_geometry.as_ref().unwrap().clone();
    assert_eq!(saved.glyphs.iter().map(|g| g.style).collect::<Vec<_>>(), vec![0, 1, 1]);
    assert_eq!(saved.glyphs.iter().map(|g| g.first_char).collect::<Vec<_>>(), vec![0, 1, 3]);
}

#[test]
fn a_bar_with_no_run_of_its_style_id_belongs_to_the_text_itself() {
    let mut nodes = document_and_page();
    nodes.push(styled_glyph_text(
        (1, 2),
        (0, 1),
        "a",
        "Whole text",
        FAMILY,
        "ab",
        &[],
        vec![],
        vec![stored_glyph(Some(0), 0.0, 0, vec![]), stored_glyph(Some(0), 10.0, 1, vec![])],
        &[FAMILY],
        vec![("decorations", V::Array(vec![stored_decoration(1, &[(0.0, 22.0, 20.0, 2.0)])]))],
        vec![("textDecoration", en("UNDERLINE"))],
    ));
    let doc = import_fig(&fig_file(nodes, vec![icon_blob()])).unwrap();
    let saved = named(&doc, "Whole text").fig_text_geometry.as_ref().unwrap().clone();
    assert!(saved.run_sids.is_empty());
    assert_eq!(saved.decorations.iter().map(|d| d.style).collect::<Vec<_>>(), vec![0]);
    assert_eq!(designos_render::stored_glyph_use(named(&doc, "Whole text")), designos_render::StoredGlyphUse::Drawn);
}

#[test]
fn run_ids_survive_the_document_file_and_the_older_key_stays_unwritten() {
    let doc = import_fig(&two_run_fixture()).unwrap();
    let text = named(&doc, "Two runs");
    let json = serde_json::to_value(text).unwrap();
    assert!(json.get("fig_text_geometry").is_none(), "0.7.17 reads the older key; the table form is under its own: {json}");
    assert_eq!(json["stored_text_glyphs"]["run_sids"], serde_json::json!([7]));
    let restored: Node = serde_json::from_value(json).unwrap();
    assert_eq!(restored.fig_text_geometry, text.fig_text_geometry);
    let file = Document::from_json(&doc.to_json().unwrap()).unwrap();
    assert_eq!(named(&file, "Two runs").fig_text_geometry, text.fig_text_geometry, "the whole document file keeps them");
}

fn saved_derived(bytes: &[u8], name: &str) -> Option<serde_json::Value> {
    decode_fig_to_json(bytes).unwrap()["message"]["nodeChanges"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["name"] == name)
        .and_then(|n| n.get("derivedTextData"))
        .cloned()
}

#[test]
fn a_save_writes_no_derived_data_for_a_text_whose_style_ids_it_would_not_map() {
    let doc = import_fig(&two_run_fixture()).unwrap();
    let text = named(&doc, "Two runs");
    assert!(text.fig_text_geometry.as_ref().is_some_and(|g| g.matches(text)), "the glyphs are drawn");
    let saved = export_fig(&doc).unwrap();
    assert!(saved_derived(&saved, "Two runs").is_none(), "Studio numbers its runs from 1: Figma's style ids would name other runs, or none");

    // two stored fonts and no run: an earlier version kept no glyphs for it, and wrote none
    let mut nodes = document_and_page();
    nodes.push(styled_glyph_text(
        (1, 2),
        (0, 1),
        "a",
        "Two fonts",
        FAMILY,
        "ab",
        &[],
        vec![],
        vec![stored_glyph(Some(0), 0.0, 0, vec![]), stored_glyph(Some(0), 10.0, 1, vec![])],
        &[FAMILY, SECOND_FAMILY],
        vec![],
        vec![],
    ));
    let doc = import_fig(&fig_file(nodes, vec![icon_blob()])).unwrap();
    assert!(saved_derived(&export_fig(&doc).unwrap(), "Two fonts").is_none());

    // and a plain single-font text is written back as before
    let doc = import_fig(&icon_fixture(FAMILY)).unwrap();
    assert!(saved_derived(&export_fig(&doc).unwrap(), "Icon text").is_some());
}

#[test]
fn a_text_with_runs_draws_the_same_after_a_round_trip_through_the_document_file() {
    let doc = import_fig(&two_run_fixture()).unwrap();
    let file = Document::from_json(&doc.to_json().unwrap()).unwrap();
    let (a, b) = (named(&doc, "Two runs"), named(&file, "Two runs"));
    assert!(matches!(a.kind, NodeKind::Text(_)));
    let bounds = |n: &Node| designos_render::text_outlines_for_node(n).iter().map(|o| o.path.bounds()).collect::<Vec<_>>();
    assert_eq!(bounds(a), bounds(b));
}
