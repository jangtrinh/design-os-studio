//! Own test binary with one test: it asserts exact values of the process-wide blob decode counter, which any other
//! stored text drawn in the process would move. A stored text builds its outline when it is drawn, from the unit path of each blob it
//! names. A blob is decoded once: drawing the same text again, or another text that names the same blob, decodes nothing.
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
