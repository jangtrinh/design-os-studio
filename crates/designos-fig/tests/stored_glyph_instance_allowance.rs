//! The data Figma stores for the texts inside instances has an allowance of its own: what does not fit is counted, and it never
//! takes what the texts themselves keep.
#[path = "common/missing_font_fig.rs"]
mod missing_font_fig;

use designos_doc::{Document, Node, NodeKind};
use designos_fig::import_fig_with_stored_glyph_allowances;
use designos_render::{stored_glyph_use, StoredGlyphUse};
use missing_font_fig::*;

fn text_in<'a>(doc: &'a Document, holder: &str) -> &'a Node {
    let holder = doc.nodes.values().find(|n| n.name == holder).unwrap_or_else(|| panic!("no node named {holder}"));
    holder.children.iter().filter_map(|&c| doc.get(c)).find(|n| matches!(n.kind, NodeKind::Text(_))).expect("a text child")
}

/// Instances that each override the characters and carry their own curve, `x` apart, so every span has bytes of its own.
fn distinct_spans(count: u32) -> Vec<u8> {
    let mut nodes = document_and_page();
    nodes.extend(icon_main());
    for i in 0..count {
        let entry = nested_text_entry(&[(1, 11)], FAMILY, 20.0, 20.0, 1, 1.0 + i as f32);
        nodes.push(icon_instance((1, 20 + i), "b", &format!("Copy {i}"), Some("\u{e901}"), None, 20.0, vec![entry]));
    }
    // the payload passes 2 MB, which is the heavy decode the allowances belong to
    fig_file(nodes, vec![icon_blob(), curve_blob(), vec![0u8; 2_100_000]])
}

#[test]
fn nested_data_past_its_allowance_is_counted_and_never_takes_the_texts_own() {
    let bytes = distinct_spans(8);
    let drawn = |doc: &Document| (0..8).filter(|i| stored_glyph_use(text_in(doc, &format!("Copy {i}"))) == StoredGlyphUse::Drawn).count();
    let (doc, report) = import_fig_with_stored_glyph_allowances(&bytes, 1 << 30, 1 << 30).unwrap();
    assert_eq!((report.dropped_stored_glyphs, drawn(&doc)), (0, 8), "every span fits");
    let one_span = text_in(&doc, "Copy 0").fig_text_geometry.as_ref().unwrap().wire.len();
    // room for three spans and a little more
    let (doc, report) = import_fig_with_stored_glyph_allowances(&bytes, 1 << 30, one_span * 3 + one_span / 2).unwrap();
    assert_eq!((report.dropped_stored_glyphs, drawn(&doc)), (5, 3), "five spans do not fit, in file order the first three do");
    assert_eq!(stored_glyph_use(text_in(&doc, "Icon main")), StoredGlyphUse::Drawn, "the component's own text is untouched");
    let (doc, report) = import_fig_with_stored_glyph_allowances(&bytes, 1 << 30, 0).unwrap();
    assert_eq!((report.dropped_stored_glyphs, drawn(&doc)), (8, 0), "no allowance for them keeps none");
    assert_eq!(stored_glyph_use(text_in(&doc, "Icon main")), StoredGlyphUse::Drawn, "and the text still keeps its data");
    // the text allowance is its own: none for the texts leaves the instances' data alone
    let (doc, report) = import_fig_with_stored_glyph_allowances(&bytes, 0, 1 << 30).unwrap();
    assert_eq!(drawn(&doc), 8, "the nested data does not draw on the allowance of the texts");
    assert_eq!(report.dropped_stored_glyphs, 1, "only the component's text did not fit");
}
