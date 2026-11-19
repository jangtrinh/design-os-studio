//! Texts that draw Figma's stored glyphs (an icon font glyph, a missing-font text of several glyphs, an instance copy)
//! must look the same on the GPU as on the CPU reference. Prints a note and returns when the machine has no GPU adapter.
#[path = "../../../designos-fig/tests/common/missing_font_fig.rs"]
mod missing_font_fig;

use crate::common;

use common::*;
use designos_doc::*;
use designos_render::Viewport;
use missing_font_fig::{golden_fixture, two_run_fixture};

#[test]
fn stored_glyph_texts_match_the_cpu() {
    let Some(mut g) = gpu() else { return };
    let doc = designos_fig::import_fig(&golden_fixture()).unwrap();
    let page = doc.pages.first().copied().expect("the fixture has a page");
    let vp = Viewport { offset: Vec2::new(0.0, 0.0), zoom: 4.0, dpr: 1.0 };
    let c = compare(&mut g, "stored_glyph_texts", &doc, page, vp, (192, 112), 8);
    // recorded before the table change: 0.0093% of the pixels differ once anti-aliasing is forgiven
    assert!(c.report_aa.ratio < 0.002, "{:?}", c.report_aa);
    // the fixture really draws ink: the mixed text's second outline stands at x 9.5..18.5, y 2..20
    let ink = |pm: &designos_render::tiny_skia::Pixmap| {
        (0..pm.height()).flat_map(|y| (0..pm.width()).map(move |x| (x, y))).filter(|&(x, y)| pm.pixel(x, y).unwrap().alpha() > 0).count()
    };
    assert!(ink(&c.cpu) > 500 && ink(&c.gpu) > 500, "both renderers draw the stored glyphs");
}

#[test]
fn a_two_run_text_keeps_each_runs_fill_on_the_gpu() {
    let Some(mut g) = gpu() else { return };
    let doc = designos_fig::import_fig(&two_run_fixture()).unwrap();
    let page = doc.pages.first().copied().expect("the fixture has a page");
    let vp = Viewport { offset: Vec2::new(0.0, 0.0), zoom: 4.0, dpr: 1.0 };
    let c = compare(&mut g, "stored_glyph_two_runs", &doc, page, vp, (192, 112), 8);
    assert!(c.report_aa.ratio < 0.002, "{:?}", c.report_aa);
    // the base style is black and the second run, with its stored underline, is red
    let count = |pm: &designos_render::tiny_skia::Pixmap, red: bool| {
        (0..pm.height())
            .flat_map(|y| (0..pm.width()).map(move |x| (x, y)))
            .filter(|&(x, y)| {
                let p = pm.pixel(x, y).unwrap();
                p.alpha() == 255 && if red { p.red() > 200 && p.green() < 50 } else { p.red() < 50 && p.green() < 50 && p.blue() < 50 }
            })
            .count()
    };
    for (what, pm) in [("cpu", &c.cpu), ("gpu", &c.gpu)] {
        assert!(count(pm, false) > 300, "{what} draws the black base run");
        assert!(count(pm, true) > 300, "{what} draws the red second run and its bar");
    }
    // and it is Figma's outlines that draw them: without the stored glyphs the same text shapes differently
    let mut shaped = doc.clone();
    for n in shaped.nodes.values_mut() {
        n.fig_text_geometry = None;
    }
    let s = compare(&mut g, "stored_glyph_two_runs_shaped", &shaped, page, vp, (192, 112), 8);
    let differing = (0..c.cpu.height())
        .flat_map(|y| (0..c.cpu.width()).map(move |x| (x, y)))
        .filter(|&(x, y)| c.cpu.pixel(x, y).unwrap() != s.cpu.pixel(x, y).unwrap())
        .count();
    assert!(differing > 400, "the stored glyphs draw differently from shaping: {differing} pixels differ");
}
