//! The data Figma stores for a text inside an instance (`derivedTextData` in the instance's `derivedSymbolData`) is kept and
//! given to the copy it is for: an override, a scale or a swapped main that makes the copy differ from the component's own text no
//! longer shapes it. A copy that already matches the data it holds adds nothing, and equal data is kept once. The allowance of
//! the data is tested in `stored_glyph_instance_allowance`.
#[path = "common/missing_font_fig.rs"]
mod missing_font_fig;

mod common;

use common::deferred_fig::*;
use common::fig_fixture::guid as json_guid;
use designos_doc::{Document, Node, NodeKind};
use designos_fig::import_fig;
use designos_render::{stored_glyph_use, text_outlines_for_node, StoredGlyphUse};
use missing_font_fig::*;
use serde_json::json;
use std::sync::Arc;

fn text_in<'a>(doc: &'a Document, holder: &str) -> &'a Node {
    let holder = doc.nodes.values().find(|n| n.name == holder).unwrap_or_else(|| panic!("no node named {holder}"));
    holder.children.iter().filter_map(|&c| doc.get(c)).find(|n| matches!(n.kind, NodeKind::Text(_))).expect("a text child")
}

/// The left and right edge of everything drawn for the text.
fn drawn_span(node: &Node) -> (f32, f32) {
    let outlines = text_outlines_for_node(node);
    assert!(!outlines.is_empty(), "the text draws something");
    let bounds: Vec<_> = outlines.iter().map(|o| o.path.bounds()).collect();
    (bounds.iter().map(|b| b.left()).fold(f32::MAX, f32::min), bounds.iter().map(|b| b.right()).fold(f32::MIN, f32::max))
}

/// The blob bytes the first glyph of the copy draws from.
fn first_blob(node: &Node) -> Vec<u8> {
    let g = node.fig_text_geometry.as_ref().expect("stored data");
    let slot = g.glyphs.as_slice()[0].blob_slot().expect("the glyph has an outline");
    g.blobs[slot as usize].to_vec()
}

#[test]
fn an_instance_text_with_an_override_draws_the_glyphs_figma_stored_for_it() {
    let doc = import_fig(&instance_nested_fixture()).unwrap();
    let main = text_in(&doc, "Icon main");
    let copy = text_in(&doc, "Icon instance");
    let NodeKind::Text(text) = &copy.kind else { unreachable!() };
    assert_eq!(text.content, "\u{e901}", "the instance shows its own characters");
    assert_eq!(stored_glyph_use(main), StoredGlyphUse::Drawn);
    assert_eq!(stored_glyph_use(copy), StoredGlyphUse::Drawn, "the overridden copy draws stored glyphs too");
    assert_eq!(first_blob(main), icon_blob(), "the component draws its triangle");
    assert_eq!(first_blob(copy), curve_blob(), "the copy draws the curve Figma stored for it, not the component's triangle");
    let (main_span, copy_span) = (drawn_span(main), drawn_span(copy));
    assert!((main_span.0 - 0.0).abs() < 1e-3 && (main_span.1 - 20.0).abs() < 1e-3, "triangle: {main_span:?}");
    assert!((copy_span.0 - 2.0).abs() < 1e-3 && (copy_span.1 - 18.0).abs() < 1e-3, "curve: {copy_span:?}");
}

#[test]
fn a_copy_that_matches_the_component_keeps_the_components_geometry_and_adds_nothing() {
    let mut nodes = document_and_page();
    nodes.extend(icon_main());
    // the entry restates the component's own glyph through another blob of the same shape: bytes that differ
    nodes.push(icon_instance((1, 20), "b", "Plain copy", None, None, 20.0, vec![nested_text_entry(&[(1, 11)], FAMILY, 20.0, 20.0, 1, 0.0)]));
    let doc = import_fig(&fig_file(nodes, vec![icon_blob(), icon_blob()])).unwrap();
    let (main, copy) = (text_in(&doc, "Icon main"), text_in(&doc, "Plain copy"));
    let (main_geometry, copy_geometry) = (main.fig_text_geometry.as_ref().unwrap(), copy.fig_text_geometry.as_ref().unwrap());
    assert!(Arc::ptr_eq(main_geometry, copy_geometry), "the copy still shares the component's geometry");
    assert!(Arc::ptr_eq(&main_geometry.wire, &copy_geometry.wire), "and none of the nested bytes");
    assert_eq!(stored_glyph_use(copy), StoredGlyphUse::Drawn);
}

#[test]
fn two_instances_with_equal_nested_data_share_one_geometry() {
    let mut nodes = document_and_page();
    nodes.extend(icon_main());
    for (i, pos) in [(0u32, "b"), (1, "c")] {
        let entry = nested_text_entry(&[(1, 11)], FAMILY, 20.0, 20.0, 1, 0.0);
        nodes.push(icon_instance((1, 20 + i), pos, &format!("Copy {i}"), Some("\u{e901}"), None, 20.0, vec![entry]));
    }
    let doc = import_fig(&fig_file(nodes, vec![icon_blob(), curve_blob()])).unwrap();
    let (a, b) = (text_in(&doc, "Copy 0"), text_in(&doc, "Copy 1"));
    let (ga, gb) = (a.fig_text_geometry.as_ref().unwrap(), b.fig_text_geometry.as_ref().unwrap());
    assert!(Arc::ptr_eq(ga, gb), "equal spans on texts that are alike are one geometry");
    assert!(!Arc::ptr_eq(ga, text_in(&doc, "Icon main").fig_text_geometry.as_ref().unwrap()));
    assert_eq!(first_blob(a), curve_blob());
}

#[test]
fn a_text_in_a_scaled_instance_draws_the_glyphs_figma_stored_at_that_scale() {
    let mut nodes = document_and_page();
    nodes.extend(icon_main());
    // half the size: the curve at 10 px, in a 10 x 10 layout
    nodes.push(icon_instance((1, 20), "b", "Half copy", None, Some(0.5), 10.0, vec![nested_text_entry(&[(1, 11)], FAMILY, 10.0, 10.0, 1, 0.0)]));
    let doc = import_fig(&fig_file(nodes, vec![icon_blob(), curve_blob()])).unwrap();
    let copy = text_in(&doc, "Half copy");
    let NodeKind::Text(text) = &copy.kind else { unreachable!() };
    assert_eq!((text.style.font_size, copy.size.x, copy.size.y), (10.0, 10.0, 10.0), "the scale reached the copy");
    assert_eq!(stored_glyph_use(copy), StoredGlyphUse::Drawn, "a scaled text no longer shapes");
    assert_eq!(first_blob(copy), curve_blob());
    let (left, right) = drawn_span(copy);
    assert!((left - 1.0).abs() < 1e-3 && (right - 9.0).abs() < 1e-3, "the curve at 10 px: {left}..{right}");
}

#[test]
fn a_nested_span_for_another_size_leaves_the_copy_shaped() {
    let mut nodes = document_and_page();
    nodes.extend(icon_main());
    // the layout is 30 wide where the copy is 20: the data is of another box
    nodes.push(icon_instance(
        (1, 20),
        "b",
        "Wrong box",
        Some("\u{e901}"),
        None,
        20.0,
        vec![nested_text_entry_laid_out(&[(1, 11)], FAMILY, 20.0, 30.0, 20.0, 1, 0.0)],
    ));
    let doc = import_fig(&fig_file(nodes, vec![icon_blob(), curve_blob()])).unwrap();
    let copy = text_in(&doc, "Wrong box");
    assert_eq!(stored_glyph_use(copy), StoredGlyphUse::Edited, "the copy differs from the component's text and the data is not for it");
}

/// A deferred fixture whose instance carries the data Figma stored for the copy of the button's label (the label has none of
/// its own): 90 x 20, one glyph from blob 0.
fn deferred_with_nested_text() -> Vec<u8> {
    deferred_with_nested_text_sized(true)
}

/// The same fixture; without `size` the instance's entry carries the stored text and nothing else Studio reads, so the
/// copy keeps the main's 120 x 48 box and the stored layout is of that box.
fn deferred_with_nested_text_sized(sized: bool) -> Vec<u8> {
    let layout = if sized { json!({"x": 90.0, "y": 20.0}) } else { json!({"x": 120.0, "y": 48.0}) };
    let mut entry = json!({
        "guidPath": {"guids": [json_guid(LABEL.0, LABEL.1)]},
        "derivedTextData": {
            "layoutSize": layout,
            "fontMetaData": [{"key": {"family": "Inter", "style": "Regular", "postscript": ""}}],
            "glyphs": [{"commandsBlob": 0, "position": {"x": 0.0, "y": 20.0}, "fontSize": 14.0, "firstCharacter": 0}]
        }
    });
    if sized {
        entry["size"] = json!({"x": 90.0, "y": 20.0});
    }
    fixture_with(json!({"derivedSymbolData": [entry]}), text_and_stroke_overrides(), Vec::new(), vec![icon_blob()])
}

fn hydrated_label(doc: &Document) -> &Node {
    let instance = doc.get(variant_button(doc)).expect("the instance");
    instance.children.iter().filter_map(|&c| doc.get(c)).find(|n| matches!(n.kind, NodeKind::Text(_))).expect("the label")
}

#[test]
fn a_deferred_page_draws_the_stored_glyphs_of_its_instance_texts_when_it_opens() {
    let mut doc = import_fig(&deferred_with_nested_text()).unwrap();
    let page = page_named(&doc, "404 pages");
    assert!(!page_hydrated(&doc, page), "the page is deferred");
    assert!(doc.ensure_page_hydrated(page), "the page opens");
    let label = hydrated_label(&doc);
    assert_eq!(label_of(&doc, variant_button(&doc)), "Go back", "the override reached the copy");
    assert_eq!(label.size, designos_doc::Vec2::new(90.0, 20.0));
    assert_eq!(stored_glyph_use(label), StoredGlyphUse::Drawn);
    assert_eq!(first_blob(label), icon_blob(), "the blob came along with the data: the opened page has no blob table");
}

#[test]
fn a_deferred_page_whose_main_changed_shapes_its_instance_texts() {
    // the same file, left alone, draws: it is the change of the main that takes the data away
    let mut untouched = import_fig(&deferred_with_nested_text()).unwrap();
    let untouched_page = page_named(&untouched, "404 pages");
    assert!(untouched.ensure_page_hydrated(untouched_page));
    assert_eq!(stored_glyph_use(hydrated_label(&untouched)), StoredGlyphUse::Drawn);

    let mut doc = import_fig(&deferred_with_nested_text()).unwrap();
    let page = page_named(&doc, "404 pages");
    let main_label = doc
        .nodes
        .values()
        .find(|n| n.name == "Text" && n.parent.and_then(|p| doc.get(p)).is_some_and(|p| p.name == "Buttons/Button"))
        .map(|n| n.id)
        .expect("main label");
    doc.get_mut(main_label).unwrap().size = designos_doc::Vec2::new(125.0, 48.0);
    assert!(doc.ensure_page_hydrated(page));
    let label = hydrated_label(&doc);
    assert_eq!(label.size, designos_doc::Vec2::new(125.0, 48.0), "the stale derived size is refused with the rest of the derived data");
    assert_eq!(stored_glyph_use(label), StoredGlyphUse::NoStoredData, "and so is the stored text: the copy shapes");
}

#[test]
fn a_deferred_page_whose_main_changed_shapes_its_instance_texts_when_the_entry_holds_only_text() {
    let mut untouched = import_fig(&deferred_with_nested_text_sized(false)).unwrap();
    let untouched_page = page_named(&untouched, "404 pages");
    assert!(untouched.ensure_page_hydrated(untouched_page));
    assert_eq!(stored_glyph_use(hydrated_label(&untouched)), StoredGlyphUse::Drawn, "the entry alone is enough to draw");

    // a letter spacing change keeps the size, fonts and characters the stored data is checked against
    let mut doc = import_fig(&deferred_with_nested_text_sized(false)).unwrap();
    let page = page_named(&doc, "404 pages");
    let main_label = doc
        .nodes
        .values()
        .find(|n| n.name == "Text" && n.parent.and_then(|p| doc.get(p)).is_some_and(|p| p.name == "Buttons/Button"))
        .map(|n| n.id)
        .expect("main label");
    let NodeKind::Text(text) = &mut doc.get_mut(main_label).unwrap().kind else { unreachable!() };
    text.style.letter_spacing += 3.0;
    assert!(doc.ensure_page_hydrated(page));
    assert_eq!(stored_glyph_use(hydrated_label(&doc)), StoredGlyphUse::NoStoredData, "the main changed: the copy shapes");
}
