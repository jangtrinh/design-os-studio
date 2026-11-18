//! A face that arrives after a missing-font text drew Figma's embedded glyphs: the text switches to the real face.
//! Own test binary: registering fonts bumps a process-wide generation that would invalidate caches used by tests
//! running in parallel.

#[path = "../../designos-fig/tests/common/missing_font_fig.rs"]
mod missing_font_fig;

use designos_doc::{Document, Node};
use designos_fig::import_fig;
use designos_render::{register_font, text_outlines_for_node};
use missing_font_fig::*;

/// Eleven characters, the length of the bundled Roboto Mono's own family name, so it can be swapped in place.
const FAMILY_NAME: &str = "MissingIcon";

fn hermetic_fonts() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| std::env::set_var("DESIGNOS_BUNDLED_FONTS_ONLY", "1"));
}

/// The bundled Roboto Mono renamed to `family` in every ASCII and UTF-16BE spelling.
fn face_named(family: &str) -> Vec<u8> {
    assert_eq!(family.len(), "Roboto Mono".len());
    let mut data = designos_fonts::bundled().iter().find(|a| a.family == "Roboto Mono").expect("Roboto Mono is bundled").data.to_vec();
    let utf16 = |s: &str| s.encode_utf16().flat_map(u16::to_be_bytes).collect::<Vec<u8>>();
    for (from, to) in [("Roboto Mono".as_bytes().to_vec(), family.as_bytes().to_vec()), (utf16("Roboto Mono"), utf16(family))] {
        let mut i = 0;
        while i + from.len() <= data.len() {
            if data[i..i + from.len()] == from[..] {
                data[i..i + from.len()].copy_from_slice(&to);
                i += from.len();
            } else {
                i += 1;
            }
        }
    }
    data
}

fn letter_w() -> (Document, Node) {
    let mut nodes = document_and_page();
    nodes.push(glyph_text((1, 2), (0, 1), "a", "Letter", FAMILY_NAME, "W", &[(0, 0.0)], vec![]));
    let doc = import_fig(&fig_file(nodes, vec![icon_blob()])).unwrap();
    let node = doc.nodes.values().find(|n| n.name == "Letter").unwrap().clone();
    (doc, node)
}

/// The box around everything drawn for `node`: left, top, right, bottom.
fn ink(node: &Node) -> (f32, f32, f32, f32) {
    let mut b = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for o in text_outlines_for_node(node) {
        let r = o.path.bounds();
        b = (b.0.min(r.left()), b.1.min(r.top()), b.2.max(r.right()), b.3.max(r.bottom()));
    }
    b
}

#[test]
fn installing_the_face_replaces_the_saved_glyphs_with_real_shaping() {
    hermetic_fonts();
    let (_doc, node) = letter_w();
    let before = ink(&node);
    assert!(
        (before.0 - 0.0).abs() < 1e-3 && (before.2 - 20.0).abs() < 1e-3 && (before.1 - 4.0).abs() < 1e-3,
        "the saved triangle is drawn while the face is missing: {before:?}"
    );

    register_font(face_named(FAMILY_NAME));
    let after = ink(&node);
    assert!(after.2 > after.0, "the installed face draws a glyph: {after:?}");
    assert_ne!(before, after, "the saved glyph no longer stands in once the face is installed");
}
