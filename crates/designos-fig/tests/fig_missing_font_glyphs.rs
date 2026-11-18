#[path = "common/missing_font_fig.rs"]
mod missing_font_fig;

use designos_doc::{Node, NodeKind, PathCmd, Vec2};
use designos_fig::{decode_fig_to_json, export_fig, import_fig};
use missing_font_fig::*;

fn fixture() -> Vec<u8> {
    icon_fixture(FAMILY)
}

fn icon_node(doc: &designos_doc::Document) -> &Node {
    doc.nodes.values().find(|node| node.name == "Icon text").unwrap()
}

#[test]
fn missing_icon_font_uses_embedded_glyph_without_losing_editability() {
    let doc = import_fig(&fixture()).unwrap();
    let icon = icon_node(&doc);
    let saved = icon.fig_text_geometry.as_ref().expect("embedded glyphs retained");
    assert_eq!(
        saved.path.cmds,
        vec![PathCmd::MoveTo(Vec2::new(0.0, 20.0)), PathCmd::LineTo(Vec2::new(20.0, 20.0)), PathCmd::LineTo(Vec2::new(10.0, 4.0)), PathCmd::Close,]
    );
    assert!(saved.matches(icon));
    let outlines = designos_render::text_outlines_for_node(icon);
    assert_eq!(outlines.len(), 1);
    let bounds = outlines[0].path.bounds();
    assert!((bounds.left() - 0.0).abs() < 1e-4 && (bounds.right() - 20.0).abs() < 1e-4);

    let mut edited = icon.clone();
    if let NodeKind::Text(text) = &mut edited.kind {
        text.content = "edited".into();
    }
    assert!(!saved.matches(&edited), "the cached icon cannot override an edited text layer");
    edited.size.x += 1.0;
    assert!(!saved.matches(&edited), "the cached icon cannot override a resized text layer");

    // The geometry also survives Studio's document serialization.
    let restored: Node = serde_json::from_str(&serde_json::to_string(icon).unwrap()).unwrap();
    assert_eq!(restored.fig_text_geometry, icon.fig_text_geometry);

    let reopened = import_fig(&export_fig(&doc).unwrap()).unwrap();
    assert_eq!(icon_node(&reopened).fig_text_geometry, icon.fig_text_geometry);

    let mut edited_doc = doc.clone();
    if let NodeKind::Text(text) = &mut edited_doc.get_mut(icon.id).unwrap().kind {
        text.content = "edited".into();
    }
    let reopened_edit = import_fig(&export_fig(&edited_doc).unwrap()).unwrap();
    assert!(icon_node(&reopened_edit).fig_text_geometry.is_none(), "edited text must not export stale icon geometry");
}

fn child_text<'a>(doc: &'a designos_doc::Document, holder: &str) -> &'a Node {
    let holder = doc.nodes.values().find(|n| n.name == holder).unwrap();
    holder.children.iter().filter_map(|&c| doc.get(c)).find(|n| matches!(n.kind, NodeKind::Text(_))).expect("a text child")
}

/// Whether any outline drawn for `node` has the bounds of the fixture's saved triangle (x 0..20, y 4..20).
fn draws_saved_triangle(node: &Node) -> bool {
    designos_render::text_outlines_for_node(node).iter().any(|o| {
        let b = o.path.bounds();
        (b.left() - 0.0).abs() < 1e-4 && (b.right() - 20.0).abs() < 1e-4 && (b.top() - 4.0).abs() < 1e-4 && (b.bottom() - 20.0).abs() < 1e-4
    })
}

#[test]
fn instance_text_override_does_not_draw_the_component_glyph() {
    let doc = import_fig(&instance_override_fixture()).unwrap();

    let main = child_text(&doc, "Icon main");
    assert!(main.fig_text_geometry.as_ref().is_some_and(|g| g.matches(main)), "the main component keeps its embedded glyph");
    assert!(draws_saved_triangle(main));

    let inst = child_text(&doc, "Icon instance");
    let NodeKind::Text(text) = &inst.kind else { unreachable!() };
    assert_eq!(text.content, "\u{e901}", "the instance shows its own characters");
    assert!(inst.fig_text_geometry.as_ref().is_none_or(|g| !g.matches(inst)), "the component's glyph must not stand in for the overridden characters");
    assert!(!draws_saved_triangle(inst), "the instance text takes the shaping path");
}

#[test]
fn fill_edit_keeps_the_embedded_glyph_and_geometry_edits_drop_it() {
    let doc = import_fig(&fixture()).unwrap();
    let icon = icon_node(&doc);
    assert!(draws_saved_triangle(icon));

    let edit = |f: &dyn Fn(&mut designos_doc::TextProps)| {
        let mut n = icon.clone();
        let NodeKind::Text(t) = &mut n.kind else { unreachable!() };
        f(t);
        n
    };
    let recoloured = edit(&|t| t.style.fills = vec![designos_doc::Paint::solid(designos_doc::Color::rgb(255, 0, 0))]);
    assert!(icon.fig_text_geometry.as_ref().unwrap().matches(&recoloured), "a colour change does not move a glyph");
    assert!(draws_saved_triangle(&recoloured), "the recoloured layer still draws Figma's glyph");

    for (what, n) in [
        ("font weight", edit(&|t| t.style.font_weight = 700)),
        ("font size", edit(&|t| t.style.font_size = 24.0)),
        ("letter spacing", edit(&|t| t.style.letter_spacing = 2.0)),
    ] {
        assert!(!icon.fig_text_geometry.as_ref().unwrap().matches(&n), "a {what} edit changes the glyph shapes");
        assert!(!draws_saved_triangle(&n), "a {what} edit drops the saved glyph");
    }
}

fn blob_count(bytes: &[u8]) -> usize {
    decode_fig_to_json(bytes).unwrap()["message"]["blobs"].as_array().map_or(0, Vec::len)
}

#[test]
fn glyph_blobs_are_deduplicated_and_reindexed_on_export() {
    let doc = import_fig(&shared_blob_fixture()).unwrap();
    let text = doc.nodes.values().find(|n| n.name == "Two glyphs").unwrap();
    let saved = text.fig_text_geometry.as_ref().expect("both glyphs are retained");
    assert_eq!(saved.path.cmds.len(), 8, "two triangles");

    let saved_bytes = export_fig(&doc).unwrap();
    let reopened = import_fig(&saved_bytes).unwrap();
    let again = reopened.nodes.values().find(|n| n.name == "Two glyphs").unwrap();
    assert_eq!(again.fig_text_geometry, text.fig_text_geometry, "the glyphs survive a save with the vector's blob ahead of them");

    // The same file without the glyph cache shows how many blobs the rest of the document needs.
    let mut bare = doc.clone();
    let id = text.id;
    bare.get_mut(id).unwrap().fig_text_geometry = None;
    assert_eq!(blob_count(&saved_bytes), blob_count(&export_fig(&bare).unwrap()) + 1, "one blob for the glyph shared by both occurrences");
}

#[test]
fn serialized_glyph_blobs_use_the_compact_hex_form() {
    let doc = import_fig(&fixture()).unwrap();
    let icon = icon_node(&doc);
    let json = serde_json::to_string(icon).unwrap();
    assert!(json.contains("hex:"), "blob bytes are written as hex strings: {json}");
    let restored: Node = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.fig_text_geometry, icon.fig_text_geometry);
}
