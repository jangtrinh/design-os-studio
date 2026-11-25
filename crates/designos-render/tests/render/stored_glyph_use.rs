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
    // a file whose text is in a bundled family keeps its glyphs, and they draw: the font does not decide
    let inter = import_fig(&icon_fixture("Inter")).unwrap();
    let inter_icon = inter.nodes.values().find(|n| n.name == "Icon text").unwrap();
    assert_eq!(stored_glyph_use(inter_icon), StoredGlyphUse::Drawn);
    // an imported underline is part of the snapshot, so the glyphs still match; Figma stored no bar for it, so the text is shaped
    let mut nodes = document_and_page();
    nodes.push(glyph_text((1, 2), (0, 1), "a", "Icon text", FAMILY, "\u{e900}", &[(0, 0.0)], vec![("textDecoration", designos_fig::testkit::en("UNDERLINE"))]));
    let underlined = import_fig(&fig_file(nodes, vec![icon_blob()])).unwrap();
    let underlined = underlined.nodes.values().find(|n| n.name == "Icon text").unwrap();
    assert_eq!(stored_glyph_use(underlined), StoredGlyphUse::Fallback("decoration missing from stored data"));
}

/// The columns the ink of `node` rendered at 4x spans.
fn ink_columns(doc: &designos_doc::Document, node: designos_doc::NodeId) -> (u32, u32) {
    let pm = designos_render::render_node(doc, node, 4.0).expect("the node renders");
    let cols: Vec<u32> = (0..pm.width()).filter(|&x| (0..pm.height()).any(|y| pm.pixel(x, y).is_some_and(|p| p.alpha() > 0))).collect();
    (*cols.first().expect("ink"), *cols.last().expect("ink"))
}

#[test]
fn an_overridden_instance_text_is_drawn_from_its_own_stored_glyphs_in_the_pixels() {
    let doc = import_fig(&instance_nested_fixture()).unwrap();
    let main = doc.nodes.values().find(|n| n.name == "Icon main").unwrap().id;
    let instance = doc.nodes.values().find(|n| n.name == "Icon instance").unwrap().id;
    // the component's triangle spans x 0..20 and the instance's own curve x 2..18 (blob 1), at 4x
    let (main_left, main_right) = ink_columns(&doc, main);
    let (left, right) = ink_columns(&doc, instance);
    assert!(main_left <= 1 && main_right >= 78, "the triangle: {main_left}..{main_right}");
    assert!((7..=9).contains(&left) && (70..=73).contains(&right), "the curve: {left}..{right}");
    // and it is the stored data that draws it: shaped, the same copy looks different
    let mut shaped = doc.clone();
    for n in shaped.nodes.values_mut() {
        n.fig_text_geometry = None;
    }
    assert_ne!(ink_columns(&shaped, instance), (left, right), "without the stored glyphs the copy shapes its characters");
}
