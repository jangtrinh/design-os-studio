//! A copy of a text inside an instance whose override restyles its runs: the style ids of its runs are the override's own, so
//! the data Figma stored for the copy (which names those ids) is read against them, not against the ids of the component's runs.
#[path = "common/missing_font_fig.rs"]
mod missing_font_fig;

use designos_doc::{Document, Node, NodeKind, StoredDecoration, TextDecoration};
use designos_fig::import_fig;
use designos_fig::kiwi::KiwiValue as V;
use designos_fig::testkit::{guid, matrix, msg, node, vec2};
use designos_render::{stored_glyph_use, StoredGlyphUse};
use missing_font_fig::*;

const MAIN: (u32, u32) = (1, 10);
const SOURCE: (u32, u32) = (1, 11);

/// The component `Styled main` (40 x 26) with the text `abcd` whose characters carry `main_ids`, the table `main_table`, and the
/// glyphs and bars Figma stored for it.
fn styled_main(main_ids: &[u32], main_table: Vec<V>, bars: Vec<V>) -> Vec<V> {
    vec![
        node(MAIN, (0, 1), "a", "SYMBOL", "Styled main", vec![("size", vec2(40.0, 26.0))]),
        styled_glyph_text(
            SOURCE,
            MAIN,
            "a",
            "Styled text",
            FAMILY,
            "abcd",
            main_ids,
            main_table,
            (0..4).map(|i| stored_glyph(Some(0), i as f32 * 10.0, i, vec![])).collect(),
            &[FAMILY, SECOND_FAMILY],
            vec![("decorations", V::Array(bars))],
            vec![],
        ),
    ]
}

/// An instance of [`styled_main`] whose override of the text sets `ids` and `table`, with the entry Figma stored for the copy.
fn styled_copy(name: &str, ids: &[u32], table: Vec<V>, bars: Vec<V>) -> V {
    let override_text = msg(vec![
        ("guidPath", msg(vec![("guids", V::Array(vec![guid(SOURCE.0, SOURCE.1)]))])),
        (
            "textData",
            msg(vec![
                ("characters", V::String("abcd".into())),
                ("characterStyleIDs", V::Array(ids.iter().map(|&i| V::Uint(i)).collect())),
                ("styleOverrideTable", V::Array(table)),
            ]),
        ),
    ]);
    let derived = msg(vec![
        ("layoutSize", vec2(40.0, 26.0)),
        ("fontMetaData", V::Array([FAMILY, SECOND_FAMILY].iter().map(|f| msg(vec![("key", font_key(f))])).collect())),
        ("glyphs", V::Array((0..4).map(|i| stored_glyph(Some(1), i as f32 * 10.0, i, vec![])).collect())),
        ("decorations", V::Array(bars)),
    ]);
    let entry =
        msg(vec![("guidPath", msg(vec![("guids", V::Array(vec![guid(SOURCE.0, SOURCE.1)]))])), ("size", vec2(40.0, 26.0)), ("derivedTextData", derived)]);
    node(
        (1, 20),
        (0, 1),
        "b",
        "INSTANCE",
        name,
        vec![
            ("size", vec2(40.0, 26.0)),
            ("transform", matrix(50.0, 0.0)),
            ("symbolData", msg(vec![("symbolID", guid(MAIN.0, MAIN.1)), ("symbolOverrides", V::Array(vec![override_text]))])),
            ("derivedSymbolData", V::Array(vec![entry])),
        ],
    )
}

fn file(copy: V, main_ids: &[u32], main_table: Vec<V>, main_bars: Vec<V>) -> Document {
    let mut nodes = document_and_page();
    nodes.extend(styled_main(main_ids, main_table, main_bars));
    nodes.push(copy);
    import_fig(&fig_file(nodes, vec![icon_blob(), curve_blob()])).unwrap()
}

fn text_in<'a>(doc: &'a Document, holder: &str) -> &'a Node {
    let holder = doc.nodes.values().find(|n| n.name == holder).unwrap_or_else(|| panic!("no node named {holder}"));
    holder.children.iter().filter_map(|&c| doc.get(c)).find(|n| matches!(n.kind, NodeKind::Text(_))).expect("a text child")
}

fn bars(node: &Node) -> Vec<StoredDecoration> {
    node.fig_text_geometry.as_ref().expect("stored data").decorations.to_vec()
}

fn underlined_runs(node: &Node) -> Vec<usize> {
    let NodeKind::Text(t) = &node.kind else { unreachable!() };
    t.runs.iter().enumerate().filter(|(_, r)| r.style.decoration != TextDecoration::None).map(|(i, _)| i + 1).collect()
}

#[test]
fn a_copy_whose_override_restyles_its_runs_draws_the_glyphs_figma_stored_for_it() {
    // the component has one run (ids 7 on `cd`); the override makes two: red `ab` (4) and underlined `cd` (9)
    let copy = styled_copy(
        "Restyled copy",
        &[4, 4, 9, 9],
        vec![style_override(4, FAMILY, (1.0, 0.0, 0.0), false), style_override(9, SECOND_FAMILY, (0.0, 0.0, 1.0), true)],
        vec![stored_decoration(9, &[(20.0, 22.0, 20.0, 2.0)])],
    );
    let doc = file(copy, &[0, 0, 7, 7], vec![style_override(7, SECOND_FAMILY, (1.0, 0.0, 0.0), true)], vec![stored_decoration(7, &[(20.0, 22.0, 20.0, 2.0)])]);
    let (main, copy) = (text_in(&doc, "Styled main"), text_in(&doc, "Restyled copy"));
    let NodeKind::Text(text) = &copy.kind else { unreachable!() };
    assert_eq!(text.runs.len(), 2, "the override made two runs where the component has one");
    assert_eq!(stored_glyph_use(main), StoredGlyphUse::Drawn);
    assert_eq!(stored_glyph_use(copy), StoredGlyphUse::Drawn, "the copy draws the stored glyphs");
    assert_eq!(*copy.fig_text_geometry.as_ref().unwrap().run_sids, [4, 9], "the runs carry the ids of the override");
    let first = copy.fig_text_geometry.as_ref().unwrap().glyphs.as_slice()[0].blob_slot().expect("an outline");
    assert_eq!(copy.fig_text_geometry.as_ref().unwrap().blobs[first as usize].to_vec(), curve_blob(), "from the blob Figma stored for the copy");
    assert_eq!(bars(copy).iter().map(|b| b.style).collect::<Vec<_>>(), [2], "the bar belongs to the second run");
}

#[test]
fn a_copy_with_as_many_runs_as_the_component_but_other_ids_takes_the_bar_of_its_own_run() {
    // the component underlines `cd` (id 7, its second run); the override swaps the two: `ab` is underlined (id 7), `cd` is plain (id 3)
    let main_table = vec![style_override(3, FAMILY, (1.0, 0.0, 0.0), false), style_override(7, SECOND_FAMILY, (0.0, 0.0, 1.0), true)];
    let copy_table = vec![style_override(7, SECOND_FAMILY, (0.0, 0.0, 1.0), true), style_override(3, FAMILY, (1.0, 0.0, 0.0), false)];
    let copy = styled_copy("Swapped copy", &[7, 7, 3, 3], copy_table, vec![stored_decoration(7, &[(0.0, 22.0, 20.0, 2.0)])]);
    let doc = file(copy, &[3, 3, 7, 7], main_table, vec![stored_decoration(7, &[(20.0, 22.0, 20.0, 2.0)])]);
    let (main, copy) = (text_in(&doc, "Styled main"), text_in(&doc, "Swapped copy"));
    assert_eq!(underlined_runs(main), [2]);
    assert_eq!(underlined_runs(copy), [1], "the copy underlines its first run");
    assert_eq!(*copy.fig_text_geometry.as_ref().unwrap().run_sids, [7, 3]);
    assert_eq!(stored_glyph_use(copy), StoredGlyphUse::Drawn);
    let copy_bars = bars(copy);
    assert_eq!((copy_bars.len(), copy_bars[0].style, copy_bars[0].x), (1, 1, 0.0), "the bar under `ab` is the first run's, not the component's second");
    assert_eq!(bars(main)[0].style, 2, "the component keeps its own");
}

#[test]
fn a_copy_that_keeps_the_components_runs_keeps_the_components_ids() {
    // an override that restates the component's own ids changes nothing
    let table = vec![style_override(7, SECOND_FAMILY, (0.0, 0.0, 1.0), true)];
    let copy = styled_copy("Same copy", &[0, 0, 7, 7], table.clone(), vec![stored_decoration(7, &[(20.0, 22.0, 20.0, 2.0)])]);
    let doc = file(copy, &[0, 0, 7, 7], table, vec![stored_decoration(7, &[(20.0, 22.0, 20.0, 2.0)])]);
    let copy = text_in(&doc, "Same copy");
    assert_eq!(stored_glyph_use(copy), StoredGlyphUse::Drawn);
    assert_eq!(*copy.fig_text_geometry.as_ref().unwrap().run_sids, [7]);
}
