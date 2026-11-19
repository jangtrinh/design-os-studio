//! The SVG and PDF of a text that draws Figma's stored glyphs keep each run in its own fill, and the stored underline of
//! the second run with it. Own process (it sets `DESIGNOS_BUNDLED_FONTS_ONLY` before the first layout).
#[path = "../../designos-fig/tests/common/missing_font_fig.rs"]
mod missing_font_fig;

use designos_export::{export_pdf, export_svg, SvgOptions};
use missing_font_fig::*;

fn hermetic_fonts() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| std::env::set_var("DESIGNOS_BUNDLED_FONTS_ONLY", "1"));
}

#[test]
fn a_two_run_text_exports_one_path_per_run_in_that_runs_fill() {
    hermetic_fonts();
    let doc = designos_fig::import_fig(&two_run_fixture()).unwrap();
    let text = doc.nodes.values().find(|n| n.name == "Two runs").unwrap();
    assert_eq!(designos_render::stored_glyph_use(text), designos_render::StoredGlyphUse::Drawn);
    let svg = export_svg(&doc, text.id, &SvgOptions::default());
    let paths: Vec<String> = svg.split("<path").skip(1).map(str::to_lowercase).collect();
    assert_eq!(paths.len(), 2, "one path for the base style and one for the run: {svg}");
    assert!(paths[0].contains("#000000") && !paths[0].contains("#ff0000"), "the first path is black: {}", paths[0]);
    assert!(paths[1].contains("#ff0000"), "the second path is red: {}", paths[1]);
    let bar = "m20 22 l40 22 l40 24 l20 24 z";
    assert!(paths[1].contains(bar), "the stored bar is in the run's path: {}", paths[1]);
    assert!(!paths[0].contains(bar), "and not in the base path: {}", paths[0]);
    let frame = doc.nodes.values().find(|n| n.name == "Runs frame").unwrap().id;
    assert!(export_pdf(&doc, &[frame]).is_ok(), "the PDF writer takes the same runs");
}
