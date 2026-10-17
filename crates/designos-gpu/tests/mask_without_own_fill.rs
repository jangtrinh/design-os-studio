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

// ----- interactive approximations must not empty a mask -----------------------
// The interactive compile defers uncached vector geometry past the frame budget
// and skips the children of tiny frames when zoomed far out. A fill-less frame
// mask takes its alpha from its children, so either approximation would leave
// the mask program empty and multiply the siblings it cuts by zero.

/// A square vector path spanning `size` x `size`.
fn square_vector(id: NodeId, size: f32) -> Node {
    let corner = |x: f32, y: f32| Vec2::new(x, y);
    let path = Path {
        cmds: vec![
            PathCmd::MoveTo(corner(0.0, 0.0)),
            PathCmd::LineTo(corner(size, 0.0)),
            PathCmd::LineTo(corner(size, size)),
            PathCmd::LineTo(corner(0.0, size)),
            PathCmd::Close,
        ],
        fill_rule: FillRule::NonZero,
    };
    let mut n = Node::new(id, NodeKind::Vector { paths: vec![path], network: None });
    n.size = Vec2::new(size, size);
    n.fills = vec![Paint::solid(BLUE)];
    n
}

/// A fill-less mask frame of `size` x `size` at the origin, then a red rectangle
/// over the whole 80 x 80 page. `child` fills the mask frame.
fn mask_frame_over_red(size: f32, radius: f32, child: impl FnOnce(NodeId) -> Node) -> (Document, NodeId) {
    let mut b = common::T::new();
    let mask = b.add(None, |id| {
        let mut n = Node::frame(id, Rect::new(0.0, 0.0, size, size));
        n.fills.clear();
        n.is_mask = true;
        n.corner_radii = [radius; 4];
        n
    });
    b.add(Some(mask), child);
    b.rect(None, Rect::new(0.0, 0.0, 80.0, 80.0), RED);
    (b.doc, b.page)
}

#[test]
fn a_deferred_vector_child_does_not_empty_the_mask() {
    let (doc, page) = mask_frame_over_red(60.0, 0.0, |id| square_vector(id, 60.0));
    let vp = Viewport { offset: Vec2::new(0.0, 0.0), zoom: 1.0, dpr: 1.0 };
    // a zero budget defers every uncached vector, as a frame that is already over budget does;
    // the headless render has no budget and builds everything. Each case starts on a fresh
    // device: a built vector stays cached and would hide the deferral from the next case.
    for (label, budget, uncap) in [("headless", None, false), ("capped", Some(Duration::ZERO), false), ("refined", Some(Duration::ZERO), true)] {
        let Some(mut g) = common::gpu() else { return };
        g.compiler.geom.budget = budget;
        g.compiler.opts.uncap_programs = uncap;
        let pm = g.render_page(&doc, page, &vp, SIZE);
        assert_eq!(px(&pm, 30, 30), [255, 0, 0, 255], "{label}: inside the mask frame, where its vector child is not built yet");
        assert_eq!(px(&pm, 70, 70), [255, 255, 255, 255], "{label}: outside the mask frame");
    }
}

#[test]
fn a_tiny_zoomed_out_mask_frame_keeps_cutting() {
    // 12 x 12 world units are 4.8 device pixels at zoom 0.4: the children of a frame that
    // small are skipped; radius 0 takes the unit-quad path, radius 3 the general one
    for radius in [0.0, 3.0] {
        let (doc, page) = mask_frame_over_red(12.0, radius, |id| {
            let mut n = Node::rectangle(id, Rect::new(0.0, 0.0, 12.0, 12.0));
            n.fills = vec![Paint::solid(BLUE)];
            n
        });
        let vp = Viewport { offset: Vec2::new(0.0, 0.0), zoom: 0.4, dpr: 1.0 };
        for (label, budget) in [("headless", None), ("interactive", Some(Duration::from_millis(8)))] {
            let Some(mut g) = common::gpu() else { return };
            g.compiler.geom.budget = budget;
            let pm = g.render_page(&doc, page, &vp, (40, 40));
            assert_eq!(px(&pm, 2, 2), [255, 0, 0, 255], "{label}, radius {radius}: inside the tiny mask frame");
            assert_eq!(px(&pm, 20, 20), [255, 255, 255, 255], "{label}, radius {radius}: outside the tiny mask frame");
        }
    }
}

#[test]
fn a_built_vector_child_still_cuts_by_its_shape() {
    let Some(mut g) = common::gpu() else { return };
    // the vector covers the top-left quarter of the mask frame
    let (doc, page) = mask_frame_over_red(60.0, 0.0, |id| square_vector(id, 30.0));
    let vp = Viewport { offset: Vec2::new(0.0, 0.0), zoom: 1.0, dpr: 1.0 };
    // a headless render builds the vector once, so the zero budget has nothing left to defer
    g.compiler.geom.budget = None;
    g.render_page(&doc, page, &vp, SIZE);
    for (label, uncap) in [("capped", false), ("refined", true)] {
        g.compiler.geom.budget = Some(Duration::ZERO);
        g.compiler.opts.uncap_programs = uncap;
        let pm = g.render_page(&doc, page, &vp, SIZE);
        assert_eq!(px(&pm, 15, 15), [255, 0, 0, 255], "{label}: inside the vector");
        assert_eq!(px(&pm, 45, 45), [255, 255, 255, 255], "{label}: inside the mask frame but outside the vector");
        assert_eq!(px(&pm, 70, 70), [255, 255, 255, 255], "{label}: outside the mask frame");
    }
}

#[test]
fn the_frame_box_stands_in_for_one_frame_only() {
    let Some(mut g) = common::gpu() else { return };
    // the vector covers the top-left quarter of the mask frame
    let (doc, page) = mask_frame_over_red(60.0, 0.0, |id| square_vector(id, 30.0));
    let vp = Viewport { offset: Vec2::new(0.0, 0.0), zoom: 1.0, dpr: 1.0 };

    // first frame: the vector is not built yet, only the box can cut the siblings
    g.compiler.geom.budget = Some(Duration::ZERO);
    let first = g.render_page(&doc, page, &vp, SIZE);
    assert_eq!(px(&first, 15, 15), [255, 0, 0, 255], "first frame: inside the vector");
    assert_eq!(px(&first, 45, 45), [255, 0, 0, 255], "first frame: the box stands in outside the unbuilt vector");
    assert_eq!(px(&first, 70, 70), [255, 255, 255, 255], "first frame: outside the mask frame");

    // next frame, with budget to build it: the mask follows the vector again
    g.compiler.geom.budget = Some(Duration::from_millis(8));
    let second = g.render_page(&doc, page, &vp, SIZE);
    assert_eq!(px(&second, 15, 15), [255, 0, 0, 255], "second frame: inside the vector");
    assert_eq!(px(&second, 45, 45), [255, 255, 255, 255], "second frame: the box is gone once the vector is built");
}

#[test]
fn standing_in_the_frame_box_opens_no_extra_program() {
    let (doc, page) = mask_frame_over_red(60.0, 0.0, |id| square_vector(id, 60.0));
    let programs = |budget: Duration, max: usize| {
        let mut c = refine_compiler();
        c.geom.budget = Some(budget);
        c.opts.refine_max_programs = max;
        compile(&mut c, &doc, page).programs.len()
    };
    // root, the layer of the cut siblings and the mask program
    let built = programs(Duration::from_millis(8), usize::MAX);
    assert_eq!(built, 3, "the vector builds within the budget");
    assert_eq!(programs(Duration::ZERO, usize::MAX), built, "the vector is deferred and the frame box stands in");
    for max in 1..=built + 1 {
        let n = programs(Duration::ZERO, max);
        assert!(n <= max, "refine_max_programs {max}: opened {n} programs");
    }
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
