//! Offscreen-program cap: interactive frames stop opening layer programs at 64,
//! the idle refine pass lifts the cap, and headless renders never cap.
//! Compiling is CPU-only; only the last test needs a GPU adapter (it skips without one).

use crate::common;

use designos_diff::{diff_image, DiffOptions};
use designos_doc::*;
use designos_gpu::SceneCompiler;
use designos_render::{render_page, tiny_skia::Pixmap, Viewport};
use std::time::Duration;

const BLURRED: usize = 100;
const SIZE: (u32, u32) = (1000, 1000);

/// A page with `BLURRED` layer-blurred rectangles inside the view and `filler`
/// plain rectangles far outside it (they only raise the document node count).
fn doc_with_blurred_rects(filler: usize) -> (Document, NodeId) {
    let mut doc = Document::new("program cap");
    let page = doc.add_page("Page");
    for i in 0..BLURRED {
        let id = doc.alloc_id();
        let (col, row) = ((i % 10) as f32, (i / 10) as f32);
        let mut n = Node::rectangle(id, Rect::new(col * 90.0 + 10.0, row * 90.0 + 10.0, 60.0, 60.0));
        n.fills = vec![Paint::solid(Color::rgb(0x33, 0x66, 0xCC))];
        n.effects = vec![Effect::LayerBlur { radius: 4.0, visible: true }];
        doc.insert(page, n).unwrap();
    }
    for i in 0..filler {
        let id = doc.alloc_id();
        let mut n = Node::rectangle(id, Rect::new(50_000.0 + (i % 50) as f32 * 40.0, 50_000.0 + (i / 50) as f32 * 40.0, 20.0, 20.0));
        n.fills = vec![Paint::solid(Color::BLACK)];
        doc.insert(page, n).unwrap();
    }
    (doc, page)
}

fn programs(c: &mut SceneCompiler, doc: &Document, page: NodeId) -> usize {
    let vp = Viewport { offset: Vec2::new(0.0, 0.0), zoom: 1.0, dpr: 1.0 };
    c.compile(doc, page, 1, &vp, SIZE).programs.len()
}

fn interactive_compiler() -> SceneCompiler {
    let mut c = SceneCompiler::new();
    c.geom.budget = Some(Duration::from_millis(8));
    c
}

#[test]
fn interactive_compile_caps_offscreen_programs_at_64() {
    let (doc, page) = doc_with_blurred_rects(0);
    let n = programs(&mut interactive_compiler(), &doc, page);
    assert!(n <= 64, "interactive frame opened {n} programs, cap is 64");
    assert!(n >= 60, "cap should still allow layers up to the limit, got {n}");
}

#[test]
fn refine_flag_lifts_the_program_cap() {
    let (doc, page) = doc_with_blurred_rects(0);
    let mut c = interactive_compiler();
    c.opts.uncap_programs = true;
    let n = programs(&mut c, &doc, page);
    assert!(n > BLURRED, "refined frame must open a program per blurred node, got {n}");
}

#[test]
fn refine_flag_off_again_returns_to_the_cap() {
    let (doc, page) = doc_with_blurred_rects(0);
    let mut c = interactive_compiler();
    c.opts.uncap_programs = true;
    let uncapped = programs(&mut c, &doc, page);
    c.opts.uncap_programs = false;
    let capped = programs(&mut c, &doc, page);
    assert!(uncapped > 64 && capped <= 64, "uncapped={uncapped} capped={capped}");
}

#[test]
fn headless_compile_of_a_large_doc_is_never_capped() {
    // more than 2000 nodes, no geometry budget: must behave like the reference renderer
    let (doc, page) = doc_with_blurred_rects(2_100);
    assert!(doc.nodes.len() > 2_000);
    let mut c = SceneCompiler::new();
    assert!(c.geom.budget.is_none());
    let n = programs(&mut c, &doc, page);
    assert!(n > BLURRED, "headless frame must not cap programs, got {n}");
}

/// On a real adapter: the refined frame is closer to the CPU reference than the
/// capped one (the capped frame drops the layer blur on nodes past the cap).
/// Writes `capped`, `refined` and `cpu` PNGs to `PROGRAM_CAP_OUT_DIR`, or next to
/// the other fidelity outputs when it is unset.
#[test]
fn refined_frame_matches_cpu_reference_better_than_capped_frame() {
    let Some(mut g) = common::gpu() else { return };
    let (doc, page) = doc_with_blurred_rects(0);
    let vp = Viewport { offset: Vec2::new(0.0, 0.0), zoom: 1.0, dpr: 1.0 };
    g.compiler.geom.budget = Some(Duration::from_millis(8));
    let capped = g.render_page(&doc, page, &vp, SIZE);
    g.compiler.opts.uncap_programs = true;
    let refined = g.render_page(&doc, page, &vp, SIZE);
    let mut cpu = Pixmap::new(SIZE.0, SIZE.1).unwrap();
    render_page(&doc, page, &vp, &mut cpu);
    let opts = DiffOptions { tolerance: 8, detect_aa: false, ignore: vec![] };
    let ratio = |pm: &Pixmap| diff_image(&common::to_img(&cpu), &common::to_img(pm), &opts).0.ratio;
    let (r_capped, r_refined) = (ratio(&capped), ratio(&refined));
    let dir = std::env::var_os("PROGRAM_CAP_OUT_DIR").map(std::path::PathBuf::from).unwrap_or_else(common::out_dir);
    std::fs::create_dir_all(&dir).unwrap();
    capped.save_png(dir.join("program-cap-capped.png")).unwrap();
    refined.save_png(dir.join("program-cap-refined.png")).unwrap();
    cpu.save_png(dir.join("program-cap-cpu.png")).unwrap();
    eprintln!("mismatch vs CPU: capped {:.4}%, refined {:.4}%", r_capped * 100.0, r_refined * 100.0);
    assert!(r_capped > 0.0, "the capped frame is expected to drop blur layers");
    assert!(r_refined < r_capped, "refined {r_refined} must be closer to the CPU reference than capped {r_capped}");
}
