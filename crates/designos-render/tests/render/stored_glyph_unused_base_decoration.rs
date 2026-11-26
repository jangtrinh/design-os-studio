//! Figma stores underline and strikethrough bars per style id, and stores none for an id no character has. A text whose
//! base style is decorated but whose characters all sit in style runs therefore has no base bar, and that is not a
//! decoration the stored data lacks.

use designos_doc::{FigTextGeometry, Node, NodeKind, Rect, StoredDecoration, StoredGlyph, TextDecoration, TextRun, TextStyle, Vec2};
use designos_render::{stored_glyph_use, StoredGlyphUse};
use std::sync::Arc;

/// "ab" with an underlined base style and one run over `run_end` bytes, holding glyphs in the run's style and `bars`.
fn text(run_end: usize, run_decoration: TextDecoration, bars: Vec<StoredDecoration>) -> Node {
    text_with_runs(&[(0, run_end)], run_decoration, bars)
}

/// The same, with one run over each of `ranges` (which a crafted payload may overlap).
fn text_with_runs(ranges: &[(usize, usize)], run_decoration: TextDecoration, bars: Vec<StoredDecoration>) -> Node {
    let base = TextStyle {
        font_family: "MissingFaceForTheUnusedBaseDecorationTest".into(),
        font_size: 14.0,
        decoration: TextDecoration::Underline,
        ..TextStyle::default()
    };
    let mut node = Node::text(designos_doc::NodeId(2), Rect::new(0.0, 0.0, 40.0, 20.0), "ab", base.clone());
    let NodeKind::Text(props) = &mut node.kind else { unreachable!() };
    for &(start, end) in ranges {
        props.runs.push(TextRun { start, end, style: TextStyle { decoration: run_decoration, ..base.clone() } });
    }
    let text = props.clone();
    let mut blob = vec![1u8];
    blob.extend([0u8; 8]);
    node.fig_text_geometry = Some(Arc::new(FigTextGeometry {
        text,
        size: Vec2::new(40.0, 20.0),
        blobs: vec![Arc::from(blob)],
        glyphs: vec![StoredGlyph { x: 0.0, y: 10.0, font_size: 14.0, blob: 0, first_char: 0, style: 1, flags: 0 }].into(),
        decorations: bars.into(),
        run_sids: Arc::from(vec![4u32]),
        ..Default::default()
    }));
    node
}

fn bar(style: u16) -> StoredDecoration {
    StoredDecoration { style, x: 0.0, y: 12.0, w: 10.0, h: 1.0 }
}

#[test]
fn a_decorated_base_with_no_character_in_it_needs_no_bar() {
    // every character is in the run, which is not decorated: Figma stores no bar at all
    let node = text(2, TextDecoration::None, vec![]);
    assert_eq!(stored_glyph_use(&node), StoredGlyphUse::Drawn);
}

#[test]
fn a_decorated_base_that_a_character_uses_still_needs_its_bar() {
    // "b" is in the base style, which is underlined, and the data holds no base bar
    let node = text(1, TextDecoration::None, vec![]);
    assert_eq!(stored_glyph_use(&node), StoredGlyphUse::Fallback("decoration missing from stored data"));
    let with_bar = text(1, TextDecoration::None, vec![bar(0)]);
    assert_eq!(stored_glyph_use(&with_bar), StoredGlyphUse::Drawn);
}

#[test]
fn a_decorated_run_still_needs_a_bar_of_its_own() {
    let node = text(2, TextDecoration::Underline, vec![]);
    assert_eq!(stored_glyph_use(&node), StoredGlyphUse::Fallback("decoration missing from stored data"));
    let with_bar = text(2, TextDecoration::Underline, vec![bar(1)]);
    assert_eq!(stored_glyph_use(&with_bar), StoredGlyphUse::Drawn);
}

#[test]
fn overlapping_runs_do_not_hide_a_character_that_is_in_the_base_style() {
    // two runs over the same first byte: their lengths add up to the text's, but "b" is still in the underlined base
    let node = text_with_runs(&[(0, 1), (0, 1)], TextDecoration::None, vec![]);
    assert_eq!(stored_glyph_use(&node), StoredGlyphUse::Fallback("decoration missing from stored data"));
    // runs that meet end to end, in any order, cover the text
    let node = text_with_runs(&[(1, 2), (0, 1)], TextDecoration::None, vec![]);
    assert_eq!(stored_glyph_use(&node), StoredGlyphUse::Drawn);
    // runs that overlap and still cover it
    let node = text_with_runs(&[(0, 2), (1, 2)], TextDecoration::None, vec![]);
    assert_eq!(stored_glyph_use(&node), StoredGlyphUse::Drawn);
}
