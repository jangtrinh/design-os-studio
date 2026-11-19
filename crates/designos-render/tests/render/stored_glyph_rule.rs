//! The draw rule: an imported text draws the glyphs Figma stored with it whenever they still describe it, whatever its
//! fonts, runs or decorations. Every case uses a family name no host has, so the result does not depend on the machine,
//! and nothing here registers a font (the process-wide font set is the arrival test's own binary).

use crate::missing_font_fig::*;
use designos_doc::{Document, Node, NodeKind, TextRun};
use designos_fig::import_fig;
use designos_fig::kiwi::KiwiValue as V;
use designos_fig::testkit::{fl, msg, st, vec2};
use designos_render::{render_node, stored_glyph_use, text_outlines_for_node, StoredGlyphUse};
use std::sync::Arc;

fn text_in(file: Vec<u8>, name: &str) -> (Document, Node) {
    let doc = import_fig(&file).unwrap();
    let node = doc.nodes.values().find(|n| n.name == name).unwrap_or_else(|| panic!("no node named {name}")).clone();
    (doc, node)
}

/// `stored_glyph_use` of a 40 x 26 text "abcd" in `FAMILY`, one triangle glyph per character, with `derived_extra` added to its
/// derived data and `glyphs` replacing the default four.
fn one_text(glyphs: Option<Vec<V>>, fonts: &[&str], derived_extra: Vec<(&str, V)>, layer_extra: Vec<(&str, V)>) -> StoredGlyphUse {
    let glyphs = glyphs.unwrap_or_else(|| (0..4).map(|i| stored_glyph(Some(0), i as f32 * 10.0, i, vec![])).collect());
    let mut nodes = document_and_page();
    nodes.push(styled_glyph_text((1, 2), (0, 1), "a", "Case", FAMILY, "abcd", &[], vec![], glyphs, fonts, derived_extra, layer_extra));
    let (_, node) = text_in(fig_file(nodes, vec![icon_blob(), curve_blob()]), "Case");
    stored_glyph_use(&node)
}

fn fallback(reason: &'static str) -> StoredGlyphUse {
    StoredGlyphUse::Fallback(reason)
}

#[test]
fn a_bundled_family_draws_the_glyphs_figma_stored() {
    let (_, icon) = text_in(icon_fixture("Inter"), "Icon text");
    assert_eq!(stored_glyph_use(&icon), StoredGlyphUse::Drawn, "Inter ships with the app and still draws Figma's glyphs");
    let b = text_outlines_for_node(&icon)[0].path.bounds();
    assert!((b.left() - 0.0).abs() < 1e-3 && (b.right() - 20.0).abs() < 1e-3 && (b.top() - 4.0).abs() < 1e-3, "the saved triangle, not shaped text: {b:?}");
}

#[test]
fn a_two_run_text_draws_each_run_in_its_own_fill() {
    let (doc, text) = text_in(two_run_fixture(), "Two runs");
    assert_eq!(stored_glyph_use(&text), StoredGlyphUse::Drawn, "two fonts and two fills still draw Figma's glyphs");
    let outlines = text_outlines_for_node(&text);
    assert_eq!(outlines.len(), 2, "one path per style");
    let fill = |i: usize| match &outlines[i].fills[0].kind {
        designos_doc::PaintKind::Solid(c) => (c.r, c.g, c.b),
        other => panic!("not a solid fill: {other:?}"),
    };
    assert_eq!((fill(0), fill(1)), ((0.0, 0.0, 0.0), (1.0, 0.0, 0.0)), "the base style in black, the run in red");
    let (base, run) = (outlines[0].path.bounds(), outlines[1].path.bounds());
    assert!(base.right() <= 30.5 && base.bottom() <= 20.5, "the first two glyphs are the base style's: {base:?}");
    assert!(run.left() >= 20.0 && run.right() >= 39.9, "the last two glyphs and the bar are the run's: {run:?}");

    let pm = render_node(&doc, text.id, 1.0).expect("the text renders");
    let px = |x: u32, y: u32| {
        let p = pm.pixel(x, y).unwrap();
        (p.red(), p.green(), p.blue(), p.alpha())
    };
    assert_eq!(px(10, 15), (0, 0, 0, 255), "inside the first triangle: black");
    assert_eq!(px(30, 23), (255, 0, 0, 255), "on the second run's bar: red");
}

#[test]
fn an_underline_on_the_second_run_comes_from_the_stored_bar_and_stays_on_that_run() {
    let (_, text) = text_in(two_run_fixture(), "Two runs");
    let outlines = text_outlines_for_node(&text);
    let (base, run) = (outlines[0].path.bounds(), outlines[1].path.bounds());
    assert!((run.bottom() - 24.0).abs() < 1e-3, "the second run reaches the bar's lower edge: {run:?}");
    assert!(base.bottom() < 21.0, "the base run has no bar: {base:?}");
    let bar_points = |i: usize| {
        outlines[i].path.segments().filter(|s| matches!(s, tiny_skia::PathSegment::MoveTo(p) if (p.y - 22.0).abs() < 1e-3 && (p.x - 20.0).abs() < 1e-3)).count()
    };
    assert_eq!((bar_points(0), bar_points(1)), (0, 1), "the bar starts at the stored x, y and only in the run's path");
}

#[test]
fn a_style_id_figma_reuses_for_two_stretches_still_draws_both_bars_under_their_glyphs() {
    let (doc, text) = text_in(repeated_id_fixture(), "Repeated id");
    let NodeKind::Text(props) = &text.kind else { unreachable!() };
    assert_eq!(props.runs.len(), 2, "two stretches are two runs");
    assert_eq!(&*text.fig_text_geometry.as_ref().unwrap().run_sids, &[3, 3], "with one Figma id");
    assert_eq!(stored_glyph_use(&text), StoredGlyphUse::Drawn, "the second run is covered by the bar Figma stored for its id");
    let outlines = text_outlines_for_node(&text);
    let bar_starts = |i: usize, x: f32| {
        outlines.get(i).map_or(0, |o| {
            o.path.segments().filter(|s| matches!(s, tiny_skia::PathSegment::MoveTo(p) if (p.y - 22.0).abs() < 1e-3 && (p.x - x).abs() < 1e-3)).count()
        })
    };
    assert_eq!((bar_starts(1, 10.0), bar_starts(1, 30.0)), (1, 1), "both bars are in the id's first run, at their stored places");
    assert_eq!((bar_starts(0, 10.0), bar_starts(0, 30.0), bar_starts(2, 10.0), bar_starts(2, 30.0)), (0, 0, 0, 0), "and in no other path");
    // on the canvas each bar is under its own glyphs (stored x 10..20 under characters c, d; 30..40 under g, h), and in the
    // fill of the style both runs share
    let pm = render_node(&doc, text.id, 1.0).expect("the text renders");
    let px = |x: u32, y: u32| {
        let p = pm.pixel(x, y).unwrap();
        (p.red(), p.green(), p.blue(), p.alpha())
    };
    assert_eq!(px(15, 23), (255, 0, 0, 255), "the bar under the first stretch");
    assert_eq!(px(35, 23), (255, 0, 0, 255), "the bar under the second stretch");
    assert_eq!(px(25, 23).3, 0, "and none between them");
}

#[test]
fn recolouring_one_of_two_runs_that_share_a_bar_shapes_the_text() {
    let (_, mut text) = text_in(repeated_id_fixture(), "Repeated id");
    let NodeKind::Text(props) = &mut text.kind else { unreachable!() };
    props.runs[1].style.fills = vec![designos_doc::Paint::solid(designos_doc::Color::rgb(0, 0, 255))];
    assert_eq!(
        stored_glyph_use(&text),
        fallback("decoration fill differs between runs that share a bar"),
        "the bar is in the first run's path, so it would stay red under blue glyphs"
    );
}

#[test]
fn recolouring_both_runs_that_share_a_bar_the_same_draws_both_bars_in_that_colour() {
    let (doc, text) = text_in(repeated_id_fixture(), "Repeated id");
    let mut doc = doc;
    let blue = vec![designos_doc::Paint::solid(designos_doc::Color::rgb(0, 0, 255))];
    let node = doc.get_mut(text.id).unwrap();
    let NodeKind::Text(props) = &mut node.kind else { unreachable!() };
    props.runs[0].style.fills = blue.clone();
    props.runs[1].style.fills = blue;
    let text = doc.get(text.id).unwrap().clone();
    assert_eq!(stored_glyph_use(&text), StoredGlyphUse::Drawn);
    let pm = render_node(&doc, text.id, 1.0).expect("the text renders");
    let px = |x: u32, y: u32| {
        let p = pm.pixel(x, y).unwrap();
        (p.red(), p.green(), p.blue(), p.alpha())
    };
    assert_eq!(px(15, 23), (0, 0, 255, 255), "the bar under the first stretch is blue");
    assert_eq!(px(35, 23), (0, 0, 255, 255), "so is the bar under the second");
}

#[test]
fn recolouring_one_run_keeps_the_stored_glyphs() {
    let (_, mut text) = text_in(two_run_fixture(), "Two runs");
    let NodeKind::Text(props) = &mut text.kind else { unreachable!() };
    props.runs[0].style.fills = vec![designos_doc::Paint::solid(designos_doc::Color::rgb(0, 0, 255))];
    assert_eq!(stored_glyph_use(&text), StoredGlyphUse::Drawn);
    let fill = &text_outlines_for_node(&text)[1].fills[0];
    assert!(matches!(&fill.kind, designos_doc::PaintKind::Solid(c) if (c.r, c.g, c.b) == (0.0, 0.0, 1.0)), "the recolour shows");
}

#[test]
fn changing_where_a_run_starts_drops_the_stored_glyphs() {
    let (_, mut text) = text_in(two_run_fixture(), "Two runs");
    let NodeKind::Text(props) = &mut text.kind else { unreachable!() };
    let TextRun { start, .. } = &mut props.runs[0];
    *start += 1;
    assert_eq!(stored_glyph_use(&text), StoredGlyphUse::Edited);
}

// ---- each way the text stays shaped has its own name

#[test]
fn an_emoji_glyph_has_no_outline_to_draw() {
    let glyphs = vec![stored_glyph(Some(0), 0.0, 0, vec![]), stored_glyph(None, 10.0, 1, vec![("emojiCodePoints", V::Array(vec![V::Uint(0x1F600)]))])];
    assert_eq!(one_text(Some(glyphs), &[FAMILY], vec![], vec![]), fallback("emoji glyph"));
}

#[test]
fn a_rotated_glyph_stays_shaped() {
    let glyphs = vec![stored_glyph(Some(0), 0.0, 0, vec![("rotation", fl(1.5))])];
    assert_eq!(one_text(Some(glyphs), &[FAMILY], vec![], vec![]), fallback("rotated glyph"));
}

#[test]
fn more_glyphs_than_a_text_may_draw_stay_shaped() {
    let glyphs = (0..4097).map(|i| stored_glyph(if i == 0 { Some(0) } else { None }, 0.0, 0, vec![])).collect();
    assert_eq!(one_text(Some(glyphs), &[FAMILY], vec![], vec![]), fallback("too many stored glyphs"));
}

#[test]
fn a_text_whose_blobs_are_over_the_budget_stays_shaped() {
    // one 1,999 byte blob named by 1,001 glyphs: 2,000,999 bytes
    let mut long = vec![1u8];
    long.extend(0f32.to_le_bytes());
    long.extend(0f32.to_le_bytes());
    for i in 0..221 {
        long.push(2);
        long.extend((i as f32 / 221.0).to_le_bytes());
        long.extend(1f32.to_le_bytes());
    }
    long.push(0);
    let glyphs = (0..1001).map(|i| stored_glyph(Some(0), i as f32, 0, vec![])).collect();
    let mut nodes = document_and_page();
    nodes.push(styled_glyph_text((1, 2), (0, 1), "a", "Case", FAMILY, "a", &[], vec![], glyphs, &[FAMILY], vec![], vec![]));
    let (_, node) = text_in(fig_file(nodes, vec![long]), "Case");
    assert_eq!(stored_glyph_use(&node), fallback("stored blobs over budget"));
}

#[test]
fn a_truncated_text_stays_shaped() {
    assert_eq!(one_text(None, &[FAMILY], vec![("truncationStartIndex", V::Int(2))], vec![]), fallback("truncated text"));
    assert_eq!(one_text(None, &[FAMILY], vec![("truncationStartIndex", V::Int(-1))], vec![]), StoredGlyphUse::Drawn, "-1 is Figma's 'not truncated'");
}

#[test]
fn blockquote_bars_and_checklist_markers_stay_shaped() {
    let rect = st(vec![("x", fl(0.0)), ("y", fl(0.0)), ("w", fl(2.0)), ("h", fl(20.0))]);
    let bar = msg(vec![("verticalBar", rect.clone()), ("quoteMarkBounds", rect.clone()), ("styleID", V::Uint(0))]);
    assert_eq!(one_text(None, &[FAMILY], vec![("blockquotes", V::Array(vec![bar]))], vec![]), fallback("blockquote or checklist markers"));
    let marker = msg(vec![("bounds", rect), ("styleID", V::Uint(0)), ("lineIndex", V::Uint(0)), ("isChecked", V::Bool(true))]);
    assert_eq!(one_text(None, &[FAMILY], vec![("checklistMarkers", V::Array(vec![marker]))], vec![]), fallback("blockquote or checklist markers"));
}

#[test]
fn a_glyph_outside_the_text_stays_shaped() {
    let glyphs = vec![stored_glyph(Some(0), 0.0, 0, vec![]), stored_glyph(Some(0), 10.0, 99, vec![])];
    assert_eq!(one_text(Some(glyphs), &[FAMILY], vec![], vec![]), fallback("glyph first character outside the text"));
    let none = vec![msg(vec![("commandsBlob", V::Uint(0)), ("position", vec2(0.0, 20.0)), ("fontSize", fl(20.0))])];
    assert_eq!(one_text(Some(none), &[FAMILY], vec![], vec![]), fallback("glyph first character outside the text"), "a glyph with no character index");
}

#[test]
fn a_layout_size_that_is_not_the_layers_stays_shaped() {
    assert_eq!(one_text(None, &[FAMILY], vec![("layoutSize", vec2(41.0, 26.0))], vec![]), fallback("layout size missing or not the layer's"));
    assert_eq!(one_text(None, &[FAMILY], vec![("layoutSize", vec2(40.05, 26.0))], vec![]), StoredGlyphUse::Drawn, "a tenth of a unit is allowed");
}

#[test]
fn stored_fonts_that_are_none_of_the_texts_fonts_stay_shaped() {
    assert_eq!(one_text(None, &["SomeOtherFaceForTest"], vec![], vec![]), fallback("stored fonts do not meet the text's fonts"));
    assert_eq!(one_text(None, &[FAMILY.to_lowercase().as_str()], vec![], vec![]), StoredGlyphUse::Drawn, "the family is matched ignoring case");
}

#[test]
fn a_glyph_that_names_a_blob_the_file_lacks_stays_shaped() {
    let glyphs = vec![stored_glyph(Some(0), 0.0, 0, vec![]), stored_glyph(Some(9), 10.0, 1, vec![])];
    assert_eq!(one_text(Some(glyphs), &[FAMILY], vec![], vec![]), fallback("malformed stored glyph data"));
}

#[test]
fn a_decoration_the_stored_data_lacks_stays_shaped() {
    let mut nodes = document_and_page();
    nodes.push(glyph_text((1, 2), (0, 1), "a", "Case", FAMILY, "\u{e900}", &[(0, 0.0)], vec![("textDecoration", designos_fig::testkit::en("UNDERLINE"))]));
    let (_, node) = text_in(fig_file(nodes, vec![icon_blob()]), "Case");
    assert_eq!(stored_glyph_use(&node), fallback("decoration missing from stored data"), "the style is underlined and Figma stored no bar");
}

#[test]
fn a_list_stays_shaped_until_its_markers_are_known_to_be_among_the_glyphs() {
    let (_, mut node) = text_in(icon_fixture(FAMILY), "Icon text");
    let mut geometry = (**node.fig_text_geometry.as_ref().unwrap()).clone();
    let list = designos_doc::ListRange { start: 0, end: 3, kind: designos_doc::ListKind::Bullet, level: 0 };
    let NodeKind::Text(props) = &mut node.kind else { unreachable!() };
    props.lists = vec![list];
    geometry.text.lists = vec![list];
    node.fig_text_geometry = Some(Arc::new(geometry));
    assert_eq!(stored_glyph_use(&node), fallback("list"));
}

#[test]
fn a_list_text_is_named_by_its_list_though_its_marker_glyph_has_no_character() {
    // the shape of the kit's lists: a glyph for the marker whose first character is past the end of the text
    let lines = V::Array(vec![msg(vec![("lineType", designos_fig::testkit::en("UNORDERED_LIST")), ("indentationLevel", V::Int(0))])]);
    let glyphs = vec![stored_glyph(Some(0), 0.0, 2, vec![]), stored_glyph(Some(0), 10.0, 0, vec![]), stored_glyph(Some(0), 20.0, 1, vec![])];
    let text_data = msg(vec![("characters", V::String("ab".into())), ("lines", lines)]);
    let mut nodes = document_and_page();
    nodes.push(styled_glyph_text((1, 2), (0, 1), "a", "Case", FAMILY, "ab", &[], vec![], glyphs, &[FAMILY], vec![], vec![("textData", text_data)]));
    let (_, node) = text_in(fig_file(nodes, vec![icon_blob()]), "Case");
    let NodeKind::Text(props) = &node.kind else { unreachable!() };
    assert!(!props.lists.is_empty(), "the fixture imports as a list");
    assert_eq!(stored_glyph_use(&node), fallback("list"), "named by the cause, not by the marker glyph");
}

#[test]
fn a_geometry_that_names_a_style_the_text_does_not_have_stays_shaped() {
    let (_, mut node) = text_in(icon_fixture(FAMILY), "Icon text");
    let mut geometry = (**node.fig_text_geometry.as_ref().unwrap()).clone();
    let mut glyphs = geometry.glyphs.as_slice().to_vec();
    glyphs[0].style = 3;
    geometry.glyphs = glyphs.into();
    node.fig_text_geometry = Some(Arc::new(geometry));
    assert_eq!(stored_glyph_use(&node), fallback("stored glyph style missing"));
}

#[test]
fn a_geometry_of_a_text_with_no_characters_has_nothing_to_draw() {
    let (_, mut node) = text_in(icon_fixture(FAMILY), "Icon text");
    let mut geometry = (**node.fig_text_geometry.as_ref().unwrap()).clone();
    geometry.text.content.clear();
    let NodeKind::Text(props) = &mut node.kind else { unreachable!() };
    props.content.clear();
    node.fig_text_geometry = Some(Arc::new(geometry));
    assert_eq!(stored_glyph_use(&node), fallback("no characters"));
}
