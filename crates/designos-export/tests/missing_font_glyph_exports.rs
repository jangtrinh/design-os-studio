//! A text layer whose font is not installed exports the glyph outlines Figma saved in the file, the same
//! shapes the canvas draws: SVG paths and PDF pages show the saved triangle, not a stand-in font's glyph.

#[path = "../../designos-fig/tests/common/missing_font_fig.rs"]
mod missing_font_fig;
mod pdf_probe;

use designos_doc::{Document, NodeId};
use designos_export::{export_pdf, export_svg, SvgOptions};
use missing_font_fig::*;
use pdf_probe::{page_pixels, Pixels};

fn icon_frame() -> (Document, NodeId) {
    let doc = designos_fig::import_fig(&icon_fixture(FAMILY)).unwrap();
    let frame = doc.nodes.values().find(|n| n.name == "Clipping frame").unwrap().id;
    (doc, frame)
}

/// Width and height of the box around the numbers of every `d="..."` path in the SVG.
fn path_extents(svg: &str) -> Vec<(f32, f32)> {
    svg.split(" d=\"")
        .skip(1)
        .map(|rest| {
            let d = &rest[..rest.find('"').unwrap()];
            let nums: Vec<f32> =
                d.split(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-')).filter(|s| !s.is_empty()).map(|s| s.parse().unwrap()).collect();
            let xs = nums.chunks_exact(2).map(|c| c[0]);
            let ys = nums.chunks_exact(2).map(|c| c[1]);
            (xs.clone().fold(f32::MIN, f32::max) - xs.fold(f32::MAX, f32::min), ys.clone().fold(f32::MIN, f32::max) - ys.fold(f32::MAX, f32::min))
        })
        .collect()
}

#[test]
fn svg_outlines_the_saved_glyph() {
    let (doc, frame) = icon_frame();
    let svg = export_svg(&doc, frame, &SvgOptions::default());
    assert!(path_extents(&svg).iter().any(|&(w, h)| (w - 20.0).abs() < 0.01 && (h - 16.0).abs() < 0.01), "the saved triangle (20 x 16) is in the SVG:\n{svg}");
}

#[test]
fn pdf_page_paints_the_saved_glyph() {
    let (doc, frame) = icon_frame();
    let bytes = export_pdf(&doc, &[frame]).unwrap();
    let (_kind, page) = page_pixels(&bytes, 0, 2.0);
    let canvas = designos_render::render_node(&doc, frame, 2.0).unwrap();
    let canvas = Pixels::from_pixmap_over_white(&canvas, canvas.width() as f32 / 2.0, canvas.height() as f32 / 2.0);
    for (x, y, inside) in [(10.0, 16.0, true), (10.0, 18.0, true), (2.0, 5.0, false)] {
        let (c, p) = (canvas.at(x, y), page.at(x, y));
        assert_eq!(c[0] < 64, inside, "the canvas draws the triangle at ({x}, {y}): {c:?}");
        assert!(c.iter().zip(&p).all(|(a, b)| a.abs_diff(*b) <= 24), "at ({x}, {y}) the canvas draws {c:?} but the PDF page shows {p:?}");
    }
}
