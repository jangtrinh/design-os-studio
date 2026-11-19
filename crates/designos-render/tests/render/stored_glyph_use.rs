//! Why a text does not draw its stored glyphs is named by `stored_glyph_use`.

use crate::missing_font_fig::*;
use designos_fig::import_fig;
use designos_render::{stored_glyph_use, StoredGlyphUse};

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
