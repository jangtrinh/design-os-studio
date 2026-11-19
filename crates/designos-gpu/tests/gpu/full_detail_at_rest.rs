//! Zoom-out level of detail: an interactive compile (pan, zoom, drag) may drop
//! sub-pixel leaves, grey small text into bars and skip the children of tiny
//! containers; the idle refine compile of a still view draws full detail, and
//! so does a headless reference render. Compiling is CPU-only: no adapter needed.

use designos_doc::*;
use designos_gpu::{Frame, FrameStats, SceneCompiler};
use designos_render::Viewport;
use std::time::Duration;

const COUNT: usize = 40;
const SIZE: (u32, u32) = (1000, 1000);
/// Device scale of every view below: under the 0.45 zoom-out threshold, and every
/// node of the documents is a few device pixels at most.
const ZOOM: f32 = 0.2;

/// What the app's idle refine compile sets (`GpuView` in `canvas/gpu.rs`).
fn refine_options(c: &mut SceneCompiler) {
    c.opts.uncap_programs = true;
    c.opts.full_detail = true;
}

fn interactive_compiler() -> SceneCompiler {
    let mut c = SceneCompiler::new();
    c.geom.budget = Some(Duration::from_millis(8));
    c
}

fn refine_compiler() -> SceneCompiler {
    let mut c = interactive_compiler();
    refine_options(&mut c);
    c
}

fn headless_compiler() -> SceneCompiler {
    SceneCompiler::new()
}

/// Compiles until no node keeps deferred geometry (the app repaints until then)
/// and returns that frame's counters.
fn settled(c: &mut SceneCompiler, doc: &Document, page: NodeId) -> FrameStats {
    let vp = Viewport { offset: Vec2::new(0.0, 0.0), zoom: ZOOM, dpr: 1.0 };
    for _ in 0..200 {
        let frame: Frame = c.compile(doc, page, 1, &vp, SIZE);
        if c.geom.stale_hits == 0 {
            return frame.stats();
        }
    }
    panic!("geometry still deferred after 200 compiles");
}

fn grid_origin(i: usize, pitch: f32) -> (f32, f32) {
    ((i % 8) as f32 * pitch + 20.0, (i / 8) as f32 * pitch + 20.0)
}

/// `COUNT` solid rectangles of 4x4 world units: 0.8 device px at `ZOOM`.
fn sub_pixel_leaves() -> (Document, NodeId) {
    let mut doc = Document::new("sub-pixel leaves");
    let page = doc.add_page("Page");
    for i in 0..COUNT {
        let (x, y) = grid_origin(i, 60.0);
        let id = doc.alloc_id();
        let mut n = Node::rectangle(id, Rect::new(x, y, 4.0, 4.0));
        n.fills = vec![Paint::solid(Color::BLACK)];
        doc.insert(page, n).unwrap();
    }
    (doc, page)
}

/// `COUNT` filled frames of 20x20 world units (4 device px) that each hold one
/// 10x10 rectangle (2 device px).
fn small_containers() -> (Document, NodeId) {
    let mut doc = Document::new("small containers");
    let page = doc.add_page("Page");
    for i in 0..COUNT {
        let (x, y) = grid_origin(i, 60.0);
        let id = doc.alloc_id();
        let frame_id = doc.insert(page, Node::frame(id, Rect::new(x, y, 20.0, 20.0))).unwrap();
        let cid = doc.alloc_id();
        let mut child = Node::rectangle(cid, Rect::new(5.0, 5.0, 10.0, 10.0));
        child.fills = vec![Paint::solid(Color::BLACK)];
        doc.insert(frame_id, child).unwrap();
    }
    (doc, page)
}

/// `COUNT` one-word labels of 12 px type: 2.4 device px at `ZOOM`.
fn small_text() -> (Document, NodeId) {
    let mut doc = Document::new("small text");
    let page = doc.add_page("Page");
    for i in 0..COUNT {
        let (x, y) = grid_origin(i, 120.0);
        let id = doc.alloc_id();
        let style = TextStyle { font_size: 12.0, fills: vec![Paint::solid(Color::BLACK)], ..Default::default() };
        doc.insert(page, Node::text(id, Rect::new(x, y, 60.0, 16.0), "Label", style)).unwrap();
    }
    (doc, page)
}

#[test]
fn interactive_compile_drops_sub_pixel_leaves() {
    let (doc, page) = sub_pixel_leaves();
    let s = settled(&mut interactive_compiler(), &doc, page);
    assert_eq!(s.draws, 0, "{COUNT} leaves under 1.25 px are dropped while the view moves");
}

#[test]
fn refine_compile_keeps_sub_pixel_leaves() {
    let (doc, page) = sub_pixel_leaves();
    let s = settled(&mut refine_compiler(), &doc, page);
    assert_eq!(s.draws, COUNT, "a still view draws every one of the {COUNT} leaves");
}

#[test]
fn interactive_compile_skips_the_children_of_small_containers() {
    let (doc, page) = small_containers();
    let s = settled(&mut interactive_compiler(), &doc, page);
    assert_eq!(s.draws, COUNT, "only the {COUNT} container fills draw while the view moves");
}

#[test]
fn refine_compile_keeps_the_children_of_small_containers() {
    let (doc, page) = small_containers();
    let s = settled(&mut refine_compiler(), &doc, page);
    assert_eq!(s.draws, 2 * COUNT, "a still view draws each container and its child");
}

#[test]
fn interactive_compile_greeks_small_text_into_bars() {
    let (doc, page) = small_text();
    let s = settled(&mut interactive_compiler(), &doc, page);
    assert_eq!(s.draws, COUNT, "one bar per label");
    assert_eq!(s.triangles, 2 * COUNT, "a bar is one quad: no glyph outline is tessellated");
}

#[test]
fn refine_compile_draws_the_glyphs_of_small_text() {
    let (doc, page) = small_text();
    let s = settled(&mut refine_compiler(), &doc, page);
    assert!(s.triangles > 4 * COUNT, "a still view draws glyph outlines, not {} triangles of bars for {COUNT} labels", s.triangles);
}

#[test]
fn refine_compile_matches_the_headless_reference_draw_for_draw() {
    for (name, (doc, page)) in [("leaves", sub_pixel_leaves()), ("containers", small_containers()), ("text", small_text())] {
        let reference = settled(&mut headless_compiler(), &doc, page);
        let refined = settled(&mut refine_compiler(), &doc, page);
        assert_eq!((refined.draws, refined.triangles), (reference.draws, reference.triangles), "{name}: the refine draws what the reference draws");
    }
}

#[test]
fn leaving_the_refine_returns_to_the_shortcuts() {
    let (doc, page) = small_containers();
    let mut c = refine_compiler();
    let refined = settled(&mut c, &doc, page);
    c.opts = Default::default();
    let moving = settled(&mut c, &doc, page);
    assert_eq!((refined.draws, moving.draws), (2 * COUNT, COUNT), "the shortcuts are back as soon as the refine options are cleared");
}
