//! The importer refuses a text whose stored glyphs it cannot draw. These are the cases it refused when it built every
//! outline at import; it must refuse them just the same now that it keeps only a table. Each case was checked against
//! the importer before the table, so this test cannot fail there. A refused text keeps a geometry without glyphs that
//! names the reason (or none, when it has nothing stored at all), so "refused" means no glyph is kept.
#[path = "common/missing_font_fig.rs"]
mod missing_font_fig;

use designos_doc::NodeKind;
use designos_fig::import_fig;
use designos_fig::testkit::square_blob;
use missing_font_fig::*;

/// A file with one text `Probe` of `glyphs`, over the blobs `[triangle, curve, square, ..extra]`.
fn probe(glyphs: &[(Option<u32>, f32, f32, f32)], extra_blobs: Vec<Vec<u8>>) -> Vec<u8> {
    let mut nodes = document_and_page();
    nodes.push(mixed_glyph_text((1, 3), (0, 1), "a", "Probe", FAMILY, glyphs));
    let mut blobs = vec![icon_blob(), curve_blob(), square_blob()];
    blobs.extend(extra_blobs);
    fig_file(nodes, blobs)
}

fn float_blob(op: u8, pts: &[f32]) -> Vec<u8> {
    let mut b = vec![op];
    b.extend(pts.iter().flat_map(|v| v.to_le_bytes()));
    b
}

fn keeps_glyphs(bytes: &[u8]) -> bool {
    let doc = import_fig(bytes).unwrap();
    let text = doc.nodes.values().find(|n| n.name == "Probe" && matches!(n.kind, NodeKind::Text(_))).expect("the probe text");
    text.fig_text_geometry.as_ref().is_some_and(|g| !g.glyphs.is_empty() && g.fallback.is_none())
}

#[test]
fn the_texts_that_were_refused_are_still_refused() {
    let nan = float_blob(1, &[f32::NAN, 0.0]);
    let infinite = [float_blob(1, &[0.0, 0.0]), float_blob(2, &[f32::INFINITY, 0.0])].concat();
    let nothing = vec![9u8, 9, 9];
    #[allow(clippy::type_complexity)]
    let cases: [(&str, Vec<(Option<u32>, f32, f32, f32)>, Vec<Vec<u8>>); 6] = [
        ("a blob with a NaN coordinate", vec![(Some(3), 0.0, 20.0, 20.0)], vec![nan]),
        ("a blob with an infinite coordinate", vec![(Some(0), 0.0, 20.0, 20.0), (Some(3), 5.0, 20.0, 20.0)], vec![infinite]),
        ("a blob that decodes to nothing", vec![(Some(3), 0.0, 20.0, 20.0)], vec![nothing]),
        ("glyphs without an outline", vec![(None, 0.0, 20.0, 20.0), (None, 9.0, 20.0, 20.0)], Vec::new()),
        ("an outline that leaves the finite range at the glyph's size", vec![(Some(2), 0.0, 20.0, 3.0e38)], Vec::new()),
        ("no glyphs at all", Vec::new(), Vec::new()),
    ];
    for (what, glyphs, extra) in cases {
        assert!(!keeps_glyphs(&probe(&glyphs, extra)), "{what}: the text keeps no stored glyphs");
    }
    assert!(keeps_glyphs(&probe(&[(Some(0), 0.0, 20.0, 20.0), (None, 9.0, 20.0, 20.0)], Vec::new())), "one outline and one space is kept");
    assert!(keeps_glyphs(&probe(&[(Some(1), 9.0, 20.0, 3.0e38)], Vec::new())), "the same size is fine for an outline of 0.9 em: 2.7e38 is finite");
}
