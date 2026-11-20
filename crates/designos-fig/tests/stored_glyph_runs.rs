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

/// The ids of the rows of the saved `styleOverrideTable` of the node named `name`.
fn saved_style_rows(bytes: &[u8], name: &str) -> Vec<u64> {
    let node = decode_fig_to_json(bytes).unwrap()["message"]["nodeChanges"].as_array().unwrap().iter().find(|n| n["name"] == name).cloned().unwrap();
    node["textData"]["styleOverrideTable"].as_array().map_or_else(Vec::new, |rows| rows.iter().map(|r| r["styleID"].as_u64().unwrap()).collect())
}

/// Every style id the saved derived data names.
fn derived_style_ids(derived: &serde_json::Value) -> Vec<u64> {
    ["glyphs", "decorations"].iter().flat_map(|k| derived[*k].as_array().into_iter().flatten()).filter_map(|e| e["styleID"].as_u64()).collect()
}

#[test]
fn a_save_keeps_the_style_ids_figma_gave_the_runs_when_every_run_has_one_of_its_own() {
    let doc = import_fig(&two_run_fixture()).unwrap();
    let text = named(&doc, "Two runs");
    assert!(text.fig_text_geometry.as_ref().is_some_and(|g| g.matches(text)), "the glyphs are drawn");
    let saved = export_fig(&doc).unwrap();
    let derived = saved_derived(&saved, "Two runs").expect("an unedited text with runs is written back");
    let rows = saved_style_rows(&saved, "Two runs");
    assert_eq!(rows, vec![7], "the one row is Figma's id for the run");
    let ids = derived_style_ids(&derived);
    assert_eq!(ids, vec![7], "and the one bar names it, as Figma stored it: {derived}");
    assert!(ids.iter().all(|id| rows.contains(id)), "every style id written is a row written");
    let node = decode_fig_to_json(&saved).unwrap()["message"]["nodeChanges"].as_array().unwrap().iter().find(|n| n["name"] == "Two runs").cloned().unwrap();
    assert_eq!(node["textData"]["styleOverrideTable"][0]["textDecoration"], "UNDERLINE", "the row the bar names is the underlined run");
    assert_eq!(node["textData"]["characterStyleIDs"], serde_json::json!([0, 0, 7, 7]));
    assert_eq!(derived["decorations"][0]["rects"][0]["x"], 20.0, "the bar is the one Figma stored");
    assert!(derived["glyphs"][0].get("styleID").is_none(), "a glyph Figma wrote without a style id is written without one");

    // and it opens again with the same glyphs and the bar on the same run
    let reopened = import_fig(&saved).unwrap();
    let again = named(&reopened, "Two runs");
    assert_eq!(&*again.fig_text_geometry.as_ref().unwrap().run_sids, &[7]);
    assert_eq!(again.fig_text_geometry.as_ref().unwrap().decorations[0].style, 1);
    assert_eq!(designos_render::stored_glyph_use(again), designos_render::StoredGlyphUse::Drawn);
}

#[test]
fn runs_that_share_a_figma_id_and_a_style_are_written_as_the_one_row_figma_wrote() {
    // Figma's id 3 sits on two stretches: Studio has two runs of one style, and writes one row under 3, as Figma did
    let doc = import_fig(&repeated_id_fixture()).unwrap();
    let saved = export_fig(&doc).unwrap();
    assert_eq!(saved_style_rows(&saved, "Repeated id"), vec![3]);
    let derived = saved_derived(&saved, "Repeated id").expect("written back");
    assert_eq!(derived_style_ids(&derived), vec![3], "{derived}");
    assert_eq!(derived["decorations"][0]["rects"].as_array().map(Vec::len), Some(2), "both bars are kept");
    let node = decode_fig_to_json(&saved).unwrap()["message"]["nodeChanges"].as_array().unwrap().iter().find(|n| n["name"] == "Repeated id").cloned().unwrap();
    assert_eq!(node["textData"]["characterStyleIDs"], serde_json::json!([0, 0, 3, 3, 0, 0, 3, 3]));
}

#[test]
fn a_save_numbers_the_rows_and_maps_the_stored_ids_when_two_runs_that_share_an_id_no_longer_share_a_style() {
    // a fill edit keeps the glyphs but the two runs of Figma's id 3 now differ: two rows, so the ids are numbered 1 and 2 and the one
    // stored bar, which names Figma's id, must name a row that is written
    let mut doc = import_fig(&repeated_id_fixture()).unwrap();
    let id = named(&doc, "Repeated id").id;
    let NodeKind::Text(props) = &mut doc.get_mut(id).unwrap().kind else { unreachable!() };
    props.runs[1].style.fills = vec![designos_doc::Paint::solid(designos_doc::Color::rgb(0, 0, 255))];
    assert!(named(&doc, "Repeated id").fig_text_geometry.as_ref().unwrap().matches(named(&doc, "Repeated id")), "a fill keeps the glyphs");
    let saved = export_fig(&doc).unwrap();
    let rows = saved_style_rows(&saved, "Repeated id");
    assert_eq!(rows, vec![1, 2]);
    let derived = saved_derived(&saved, "Repeated id").expect("written back");
    let ids = derived_style_ids(&derived);
    assert_eq!(ids, vec![1], "Figma's id 3 became the row of the first run that had it: {derived}");
    assert!(ids.iter().all(|id| rows.contains(id)), "every style id written is a row written");
}

#[test]
fn a_stored_id_no_run_has_is_written_as_the_text_s_own_style_unless_the_text_has_rows_of_its_own() {
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
    let derived = saved_derived(&export_fig(&doc).unwrap(), "Whole text").expect("written back");
    assert_eq!(derived_style_ids(&derived), vec![0], "an id no run has belongs to the text's own style");

    // a layer linked to a text style that stores an override gets a row for its own characters: no single id names them
    let mut linked = doc.clone();
    let id = named(&linked, "Whole text").id;
    let node = linked.get_mut(id).unwrap();
    node.style_refs.insert("text".into(), "S:1".into());
    node.text_style_overrides.insert(designos_doc::OwnedField::Decoration);
    assert!(saved_derived(&export_fig(&linked).unwrap(), "Whole text").is_none(), "a text whose style ids cannot be mapped is left without derived data");
}

#[test]
fn a_text_with_two_stored_fonts_and_no_run_is_written_back() {
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
    let derived = saved_derived(&export_fig(&doc).unwrap(), "Two fonts").expect("what Figma stored is written back, drawn or not");
    assert_eq!(derived["fontMetaData"].as_array().map(Vec::len), Some(2));

    // and a plain single-font text as before
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

#[test]
fn a_text_without_an_outline_is_written_back_as_figma_stored_it() {
    // glyphs that all lack a blob (an emoji, spaces): the renderer shapes the text, and a save still writes what Figma stored
    let mut nodes = document_and_page();
    nodes.push(styled_glyph_text(
        (1, 2),
        (0, 1),
        "a",
        "Emoji only",
        FAMILY,
        "\u{1F600}",
        &[],
        vec![],
        vec![stored_glyph(None, 0.0, 0, vec![("emojiCodePoints", V::Array(vec![V::Uint(0x1F600)]))])],
        &[FAMILY],
        vec![],
        vec![],
    ));
    let doc = import_fig(&fig_file(nodes, vec![icon_blob()])).unwrap();
    let text = named(&doc, "Emoji only");
    assert!(text.fig_text_geometry.is_some(), "the importer keeps it");
    assert_eq!(designos_render::stored_glyph_use(text), designos_render::StoredGlyphUse::Fallback("emoji glyph"));
    let derived = saved_derived(&export_fig(&doc).unwrap(), "Emoji only").expect("the save writes what Figma stored");
    assert_eq!(derived["glyphs"][0]["emojiCodePoints"], serde_json::json!([128512]));
}

#[test]
fn a_component_text_and_the_copies_of_it_share_one_glyph_blob_in_a_save() {
    let mut nodes = document_and_page();
    nodes.push(designos_fig::testkit::node((1, 10), (0, 1), "a", "SYMBOL", "Icon main", vec![("size", designos_fig::testkit::vec2(20.0, 20.0))]));
    nodes.push(glyph_text((1, 11), (1, 10), "a", "Source text", FAMILY, "\u{e900}", &[(0, 0.0)], vec![]));
    for i in 0..3u32 {
        nodes.push(designos_fig::testkit::node(
            (1, 20 + i),
            (0, 1),
            "b",
            "INSTANCE",
            &format!("Copy {i}"),
            vec![
                ("size", designos_fig::testkit::vec2(20.0, 20.0)),
                ("symbolData", designos_fig::testkit::msg(vec![("symbolID", designos_fig::testkit::guid(1, 10))])),
            ],
        ));
    }
    let doc = import_fig(&fig_file(nodes, vec![icon_blob()])).unwrap();
    let texts: Vec<&Node> = doc.nodes.values().filter(|n| matches!(n.kind, NodeKind::Text(_))).collect();
    assert_eq!(texts.len(), 4, "the source and three copies hold the data");
    let (_, first) = (0, texts[0].fig_text_geometry.as_ref().unwrap());
    assert!(texts.iter().all(|t| std::sync::Arc::ptr_eq(t.fig_text_geometry.as_ref().unwrap(), first)), "one geometry, one set of wire bytes");
    // Studio writes the copies as records of their own, so each carries the data its geometry still matches; the glyph blob is one
    let saved = export_fig(&doc).unwrap();
    let all = decode_fig_to_json(&saved).unwrap();
    let blobs: Vec<u64> = all["message"]["nodeChanges"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|n| n["type"] == "TEXT")
        .filter_map(|n| n["derivedTextData"]["glyphs"][0]["commandsBlob"].as_u64())
        .collect();
    assert_eq!(blobs.len(), 4);
    assert!(blobs.iter().all(|&b| b == blobs[0]), "every record names the one blob");
    assert_eq!(all["message"]["blobs"].as_array().map(Vec::len), Some(1), "which is in the table once");
}
