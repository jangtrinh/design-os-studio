//! A stored text builds its outline when it is drawn, from the unit path of each blob it names. A blob is decoded once:
//! drawing the same text again, or another text that names the same blob, decodes nothing. Own test binary: the
//! decode counter is process-wide.
#[path = "../../designos-fig/tests/common/missing_font_fig.rs"]
mod missing_font_fig;

use designos_fig::import_fig;
use designos_render::{render_node, stored_glyph_blob_decodes, stored_glyph_use, text_outlines_for_node, StoredGlyphUse};
use missing_font_fig::*;

#[test]
fn a_blob_is_decoded_once_however_often_texts_that_name_it_are_drawn() {
    let doc = import_fig(&golden_fixture()).unwrap();
    let text = |name: &str, nth: usize| {
        let mut found: Vec<_> = doc.nodes.values().filter(|n| n.name == name).collect();
        found.sort_by_key(|n| n.id);
        found[nth]
    };
    let (icon, mixed, copy) = (text("Icon text", 0), text("Mixed glyphs", 0), text("Glyph source", 1));
    for n in [icon, mixed, copy] {
        assert_eq!(stored_glyph_use(n), StoredGlyphUse::Drawn);
    }
    assert_eq!(stored_glyph_blob_decodes(), 0, "opening the file decoded no outline");

    let first = text_outlines_for_node(icon);
    assert_eq!(stored_glyph_blob_decodes(), 1, "the icon's one blob");
    let again = text_outlines_for_node(icon);
    assert_eq!(stored_glyph_blob_decodes(), 1, "the second draw of the same text decodes no blob");
    assert_eq!(first[0].path.bounds(), again[0].path.bounds());

    text_outlines_for_node(mixed);
    assert_eq!(stored_glyph_blob_decodes(), 2, "the mixed text names the icon blob again and a curve blob: only the curve is new");
    text_outlines_for_node(copy);
    render_node(&doc, mixed.id, 2.0).unwrap();
    assert_eq!(stored_glyph_blob_decodes(), 2, "an instance copy and a pixel render decode nothing either");
}

#[test]
fn the_reason_a_text_does_not_draw_stored_glyphs_is_named() {
    let doc = import_fig(&golden_fixture()).unwrap();
    let icon = doc.nodes.values().find(|n| n.name == "Icon text").unwrap();
    let mut edited = icon.clone();
    if let designos_doc::NodeKind::Text(t) = &mut edited.kind {
        t.content = "x".into();
    }
    assert_eq!(stored_glyph_use(&edited), StoredGlyphUse::Edited);
    let mut underlined = icon.clone();
    if let designos_doc::NodeKind::Text(t) = &mut underlined.kind {
        t.style.decoration = designos_doc::TextDecoration::Underline;
    }
    assert_eq!(stored_glyph_use(&underlined), StoredGlyphUse::Edited, "a decoration change also changes the glyph shapes");
    let mut bare = icon.clone();
    bare.fig_text_geometry = None;
    assert_eq!(stored_glyph_use(&bare), StoredGlyphUse::NoStoredData);
    let frame = doc.nodes.values().find(|n| n.name == "Icon frame").unwrap();
    assert_eq!(stored_glyph_use(frame), StoredGlyphUse::NoStoredData);
    // a file whose text is in a bundled family keeps its glyphs, and shaping draws it
    let inter = import_fig(&icon_fixture("Inter")).unwrap();
    let inter_icon = inter.nodes.values().find(|n| n.name == "Icon text").unwrap();
    assert_eq!(stored_glyph_use(inter_icon), StoredGlyphUse::Fallback("font installed"));
    // an imported underline is part of the snapshot, so the glyphs still match; the saved outlines carry no bar, shaping draws it
    let mut nodes = document_and_page();
    nodes.push(glyph_text((1, 2), (0, 1), "a", "Icon text", FAMILY, "\u{e900}", &[(0, 0.0)], vec![("textDecoration", designos_fig::testkit::en("UNDERLINE"))]));
    let underlined = import_fig(&fig_file(nodes, vec![icon_blob()])).unwrap();
    let underlined = underlined.nodes.values().find(|n| n.name == "Icon text").unwrap();
    assert_eq!(stored_glyph_use(underlined), StoredGlyphUse::Fallback("decoration"));
}
