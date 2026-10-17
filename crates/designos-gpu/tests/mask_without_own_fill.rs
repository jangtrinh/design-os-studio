//! A frame mask with no fill of its own takes its alpha from its children on the
//! GPU too (the same rule as the CPU renderer), in the headless render and in the
//! interactive compile once the idle refine lifts the program cap.
//! The pixel tests need a GPU adapter and skip without one; the ceiling tests
//! at the end only compile.

mod common;

use designos_doc::*;
use designos_gpu::{Frame, SceneCompiler, DEFAULT_REFINE_MAX_PROGRAMS};
use designos_render::tiny_skia::Pixmap;
use designos_render::Viewport;
use std::time::Duration;

const SIZE: (u32, u32) = (80, 80);
const RED: Color = Color { r: 1.0, g: 0.0, b: 0.0, a: 1.0 };
const BLUE: Color = Color { r: 0.0, g: 0.0, b: 1.0, a: 1.0 };

/// A mask frame with no fills whose only child covers the top-left quarter,
/// followed by a red rectangle over the whole page.
fn doc() -> (Document, NodeId) {
    let mut b = common::T::new();
    let mask = b.add(None, |id| {
        let mut n = Node::frame(id, Rect::new(0.0, 0.0, 80.0, 80.0));
        n.fills.clear();
        n.is_mask = true;
        n
    });
    b.rect(Some(mask), Rect::new(0.0, 0.0, 40.0, 40.0), BLUE);
    b.rect(None, Rect::new(0.0, 0.0, 80.0, 80.0), RED);
    (b.doc, b.page)
}

fn px(pm: &Pixmap, x: u32, y: u32) -> [u8; 4] {
    let p = pm.pixel(x, y).unwrap();
    [p.red(), p.green(), p.blue(), p.alpha()]
}

fn assert_cut_by_the_child(pm: &Pixmap, what: &str) {
    assert_eq!(px(pm, 20, 20), [255, 0, 0, 255], "{what}: inside the child of the mask");
    assert_eq!(px(pm, 60, 60), [255, 255, 255, 255], "{what}: inside the mask frame but outside its child");
    assert_eq!(px(pm, 60, 20), [255, 255, 255, 255], "{what}: right of the child");
}

#[test]
fn headless_gpu_matches_the_cpu_for_a_frame_mask_without_fill() {
    let Some(mut g) = common::gpu() else { return };
    let (doc, page) = doc();
    let vp = Viewport { offset: Vec2::new(0.0, 0.0), zoom: 1.0, dpr: 1.0 };
    let c = common::compare(&mut g, "mask-frame-without-fill", &doc, page, vp, SIZE, 4);
    assert_cut_by_the_child(&c.cpu, "cpu");
    assert_cut_by_the_child(&c.gpu, "gpu");
    assert_eq!(c.report.mismatched, 0, "gpu and cpu agree pixel for pixel");
}

#[test]
fn refined_interactive_gpu_frame_cuts_by_the_children() {
    let Some(mut g) = common::gpu() else { return };
    let (doc, page) = doc();
    let vp = Viewport { offset: Vec2::new(0.0, 0.0), zoom: 1.0, dpr: 1.0 };
    g.compiler.geom.budget = Some(Duration::from_millis(8));
    g.compiler.opts.uncap_programs = true;
    let pm = g.render_page(&doc, page, &vp, SIZE);
    assert_cut_by_the_child(&pm, "refined");
}

// ----- program and VRAM ceilings of the refine compile -----------------------
// A mask frame without fills draws its children into the mask program, so the
// children open programs of their own while the mask program is open.

const VIEW: (u32, u32) = (1000, 1000);
const MIB: u64 = 1024 * 1024;
/// Bytes of the root render target of a 1000x1000 frame: 4x MSAA colour plus
/// the resolved output, 4 bytes per pixel.
const ROOT_BYTES: u64 = 1000 * 1000 * 4 * (4 + 1);

fn refine_compiler() -> SceneCompiler {
    let mut c = SceneCompiler::new();
    c.geom.budget = Some(Duration::from_millis(8));
    c.opts.uncap_programs = true;
    c
}

fn compile(c: &mut SceneCompiler, doc: &Document, page: NodeId) -> Frame {
    let vp = Viewport { offset: Vec2::new(0.0, 0.0), zoom: 1.0, dpr: 1.0 };
    c.compile(doc, page, 1, &vp, VIEW)
}

fn blurred_rect(doc: &mut Document, parent: NodeId, r: Rect, radius: f32) {
    let mut n = Node::rectangle(doc.alloc_id(), r);
    n.effects = vec![Effect::LayerBlur { radius, visible: true }];
    doc.insert(parent, n).unwrap();
}

/// Inside `parent`: a fill-less mask frame of `w` x `h` holding one blurred
/// child, then `siblings` blurred rectangles the mask cuts.
fn fill_less_mask_group(doc: &mut Document, parent: NodeId, w: f32, h: f32, siblings: usize, radius: f32) {
    let mut mask = Node::frame(doc.alloc_id(), Rect::new(0.0, 0.0, w, h));
    mask.fills.clear();
    mask.is_mask = true;
    let mask = doc.insert(parent, mask).unwrap();
    blurred_rect(doc, mask, Rect::new(2.0, 2.0, w / 3.0, h / 3.0), radius);
    for i in 0..siblings {
        blurred_rect(doc, parent, Rect::new(2.0 + i as f32 * 3.0, h / 2.0, w / 3.0, h / 3.0), radius);
    }
}

/// `copies` small frames side by side, each holding a fill-less mask frame with
/// a blurred child followed by two blurred siblings.
fn masked_frame_grid(copies: usize) -> (Document, NodeId) {
    let mut doc = Document::new("masked frame grid");
    let page = doc.add_page("Page");
    for i in 0..copies {
        let (col, row) = ((i % 50) as f32, (i / 50) as f32);
        let id = doc.alloc_id();
        let cell = doc.insert(page, Node::frame(id, Rect::new(col * 20.0, row * 20.0, 18.0, 18.0))).unwrap();
        fill_less_mask_group(&mut doc, cell, 18.0, 18.0, 2, 1.0);
    }
    (doc, page)
}

/// Inside a frame and again at the page root: a blurred rectangle, a fill-less
/// mask frame with a blurred child, and three blurred siblings. The frame comes
/// first so the root-level mask does not cut it away.
fn fill_less_masked_page() -> (Document, NodeId) {
    let mut doc = Document::new("fill-less masked");
    let page = doc.add_page("Page");
    let id = doc.alloc_id();
    let frame = doc.insert(page, Node::frame(id, Rect::new(500.0, 0.0, 400.0, 400.0))).unwrap();
    blurred_rect(&mut doc, frame, Rect::new(10.0, 10.0, 60.0, 60.0), 4.0);
    fill_less_mask_group(&mut doc, frame, 300.0, 300.0, 3, 4.0);
    blurred_rect(&mut doc, page, Rect::new(10.0, 10.0, 60.0, 60.0), 4.0);
    fill_less_mask_group(&mut doc, page, 400.0, 300.0, 3, 4.0);
    (doc, page)
}

#[test]
fn many_fill_less_mask_frames_stay_under_the_default_program_ceiling() {
    const COPIES: usize = 1_500;
    let (doc, page) = masked_frame_grid(COPIES);

    let mut open = refine_compiler();
    open.opts.refine_max_programs = usize::MAX;
    open.opts.refine_vram_budget_bytes = u64::MAX;
    let unbounded = compile(&mut open, &doc, page).programs.len();
    assert!(unbounded > DEFAULT_REFINE_MAX_PROGRAMS, "the page must need more than the ceiling, opened {unbounded}");

    let mut c = refine_compiler();
    let frame = compile(&mut c, &doc, page);
    assert!(
        frame.programs.len() <= DEFAULT_REFINE_MAX_PROGRAMS,
        "{COPIES} fill-less mask frames opened {} programs, exceeding ceiling {DEFAULT_REFINE_MAX_PROGRAMS}",
        frame.programs.len()
    );
    assert!(frame.programs.len() > DEFAULT_REFINE_MAX_PROGRAMS / 2, "the refine must still draw masks, opened {}", frame.programs.len());
}

#[test]
fn fill_less_mask_frames_respect_every_program_ceiling() {
    let (doc, page) = fill_less_masked_page();
    let mut open = refine_compiler();
    let full = compile(&mut open, &doc, page).programs.len();
    assert!(full >= 14, "the page needs layers, mask layers, mask programs and child blurs, got {full}");
    for max in 1..=full + 1 {
        let mut c = refine_compiler();
        c.opts.refine_max_programs = max;
        let frame = compile(&mut c, &doc, page);
        assert!(frame.programs.len() <= max, "refine_max_programs {max}: opened {} programs", frame.programs.len());
    }
}

#[test]
fn fill_less_mask_frames_stay_within_the_vram_budget() {
    let (doc, page) = fill_less_masked_page();
    let mut open = refine_compiler();
    compile(&mut open, &doc, page);
    let full_bytes = open.last_vram_bytes;
    assert!(full_bytes > ROOT_BYTES + 4 * MIB, "the page must hold offscreen targets worth the sweep, got {full_bytes} bytes");
    for step in 0..=40u64 {
        let budget = ROOT_BYTES + step * 400_000;
        let mut c = refine_compiler();
        c.opts.refine_vram_budget_bytes = budget;
        let frame = compile(&mut c, &doc, page);
        assert!(c.last_vram_bytes <= budget, "budget {budget}: frame holds {} bytes in {} programs", c.last_vram_bytes, frame.programs.len());
    }
}
