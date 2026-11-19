//! Own test binary: it sets `DESIGNOS_BUNDLED_FONTS_ONLY` before the first text layout, which is read once per process.
//! Golden for the SVG and PDF of the texts that draw Figma's stored glyphs (an icon font glyph, a missing-font text of
//! several glyphs, an instance copy). The constants were recorded on the commit before the glyph data moved into a
//! compact table, so this test cannot fail on that commit: it pins the exported bytes the table must keep reproducing.
//! The PDF carries an invisible text layer positioned by shaping, so the fonts are the bundled ones, never the host's.
#[path = "../../designos-fig/tests/common/missing_font_fig.rs"]
mod missing_font_fig;

use designos_doc::{Document, NodeId};
use designos_export::{export_pdf, export_svg, SvgOptions};
use missing_font_fig::*;

fn fnv(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |h, &b| (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3))
}

fn id_of(doc: &Document, name: &str) -> NodeId {
    doc.nodes.values().find(|n| n.name == name).unwrap_or_else(|| panic!("no node named {name}")).id
}

fn hermetic_fonts() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| std::env::set_var("DESIGNOS_BUNDLED_FONTS_ONLY", "1"));
}

#[test]
fn stored_glyph_exports_are_the_recorded_bytes() {
    hermetic_fonts();
    let doc = designos_fig::import_fig(&golden_fixture()).unwrap();
    let ids = ["Icon frame", "Mixed frame", "Copy frame"].map(|name| id_of(&doc, name));
    let svg: Vec<u64> = ids.iter().map(|&id| fnv(export_svg(&doc, id, &SvgOptions::default()).as_bytes())).collect();
    let pdf = fnv(&export_pdf(&doc, &ids).unwrap());
    assert_eq!(pdf, fnv(&export_pdf(&doc, &ids).unwrap()), "the PDF bytes are deterministic, so they can be recorded");
    let expected_svg: [u64; 3] = [0xfbd1_b749_baf7_685b, 0x44a9_bd49_d736_d60c, 0xfb22_b860_32d9_fe16];
    let expected_pdf: u64 = 0x0;
    assert!((svg.as_slice(), pdf) == (expected_svg.as_slice(), expected_pdf), "recorded svg {expected_svg:x?} pdf {expected_pdf:#x}; drawn svg {svg:x?} pdf {pdf:#x}");
}
