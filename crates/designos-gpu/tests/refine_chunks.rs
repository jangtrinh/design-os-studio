//! The idle refine of a large page is spread over UI frames: deferred geometry is built
//! from the list the first refine compile records (no page walk per frame), and the
//! finished frame is cut into chunks that are submitted one per frame into a page
//! texture of their own. Compiling needs no adapter; the pixel test skips without one.

mod common;

use designos_doc::*;
use designos_gpu::SceneCompiler;
use designos_render::Viewport;
use std::time::Duration;

const SIZE: (u32, u32) = (1000, 1000);

fn refine_compiler(budget: Option<Duration>) -> SceneCompiler {
    let mut c = SceneCompiler::new();
    c.geom.budget = budget;
    c.opts.uncap_programs = true;
    c.opts.full_detail = true;
    c
}

/// `count` one-word labels, 12 px type, at a zoom where each is a few device pixels tall.
fn labels(count: usize) -> (Document, NodeId) {
    let mut doc = Document::new("labels");
    let page = doc.add_page("Page");
    for i in 0..count {
        let id = doc.alloc_id();
        let (x, y) = ((i % 8) as f32 * 120.0 + 20.0, (i / 8) as f32 * 40.0 + 20.0);
        let style = TextStyle { font_size: 12.0, fills: vec![Paint::solid(Color::BLACK)], ..Default::default() };
        // distinct words: identical labels would share one cached geometry and defer nothing
        doc.insert(page, Node::text(id, Rect::new(x, y, 60.0, 16.0), format!("Label {i}"), style)).unwrap();
    }
    (doc, page)
}

/// `count` layer-blurred rectangles: one offscreen program each.
fn blurred_rects(count: usize) -> (Document, NodeId) {
    let mut doc = Document::new("blurred");
    let page = doc.add_page("Page");
    for i in 0..count {
        let id = doc.alloc_id();
        let (col, row) = ((i % 10) as f32, (i / 10) as f32);
        let mut n = Node::rectangle(id, Rect::new(col * 90.0 + 10.0, row * 90.0 + 10.0, 60.0, 60.0));
        n.fills = vec![Paint::solid(Color::rgb(0x33, 0x66, 0xCC))];
        n.effects = vec![Effect::LayerBlur { radius: 4.0, visible: true }];
        doc.insert(page, n).unwrap();
    }
    (doc, page)
}

fn vp(zoom: f32) -> Viewport {
    Viewport { offset: Vec2::new(0.0, 0.0), zoom, dpr: 1.0 }
}

#[test]
fn compile_records_the_deferred_nodes_and_warming_builds_them_within_the_budget() {
    let (doc, page) = labels(60);
    let mut c = refine_compiler(Some(Duration::from_micros(1)));
    let first = c.compile(&doc, page, 1, &vp(0.2), SIZE);
    let stale = c.geom.stale_hits as usize;
    assert!(stale >= 50, "a 1 microsecond budget leaves almost every label deferred, got {stale}");
    assert_eq!(c.geom.deferred.len(), stale, "every deferred node is on the list");
    assert!(first.stats().triangles < 60 * 10, "the deferred labels drew nothing yet");
    let mut left = stale;
    let mut frames = 0;
    while left > 0 {
        let builds = c.geom.builds;
        let now = c.warm_deferred(&doc, 1);
        let built = (c.geom.builds - builds) as usize;
        assert!((1..=2).contains(&built), "a warm frame builds what one budget allows, built {built}");
        assert!(now < left, "every warm frame makes progress");
        left = now;
        frames += 1;
        assert!(frames < 200);
    }
    let last = c.compile(&doc, page, 1, &vp(0.2), SIZE);
    assert_eq!(c.geom.stale_hits, 0, "after warming, one compile finds everything cached");
    // headless (no budget): never simplifies, never defers
    let whole = SceneCompiler::new().compile(&doc, page, 1, &vp(0.2), SIZE);
    assert_eq!(
        (last.stats().draws, last.stats().triangles),
        (whole.stats().draws, whole.stats().triangles),
        "the warmed compile draws what an unbudgeted one draws"
    );
}

#[test]
fn warming_ignores_nodes_that_left_the_document() {
    let (mut doc, page) = labels(10);
    let mut c = refine_compiler(Some(Duration::from_micros(1)));
    let _ = c.compile(&doc, page, 1, &vp(0.2), SIZE);
    let ids: Vec<NodeId> = c.geom.deferred.iter().map(|(id, _)| *id).collect();
    for id in ids.iter().take(3) {
        doc.remove(*id).unwrap();
    }
    let mut left = c.geom.deferred.len();
    for _ in 0..50 {
        left = c.warm_deferred(&doc, 2);
        if left == 0 {
            break;
        }
    }
    assert_eq!(left, 0, "deleted nodes are skipped, not retried forever");
}

#[test]
fn plan_chunks_cuts_a_frame_into_ranges_under_the_budget() {
    let (doc, page) = blurred_rects(100);
    let mut c = refine_compiler(Some(Duration::from_millis(8)));
    let frame = c.compile(&doc, page, 1, &vp(1.0), SIZE);
    assert!(frame.stats().programs > 100, "one layer per blurred rectangle");
    let n_ops = frame.stats().composites + frame.stats().draws;
    let budget = 4_000.0;
    let ranges = frame.plan_chunks(budget).expect("a plain frame can be cut");
    assert!(ranges.len() >= 5, "100 layers of ~0.5 ms each need several chunks of 4 ms, got {}", ranges.len());
    // contiguous, in order, covering every op of the root
    assert_eq!(ranges.first().map(|r| r.start), Some(0));
    for w in ranges.windows(2) {
        assert_eq!(w[0].end, w[1].start, "ranges touch");
    }
    let root_ops = ranges.last().unwrap().end;
    assert!(root_ops > 0 && root_ops <= n_ops);
    for r in &ranges {
        let cost = frame.estimate_chunk_us(r.clone());
        assert!(cost <= budget || r.len() == 1, "chunk {r:?} is estimated at {cost} us, over {budget} us and not a single op");
    }
    let total: f32 = ranges.iter().map(|r| frame.estimate_chunk_us(r.clone())).sum();
    assert!(total >= frame.estimate_chunk_us(0..root_ops) * 0.999, "cutting repeats at most the programs shared across a boundary, never loses cost");
}

#[test]
fn a_roomy_budget_keeps_the_frame_whole_and_a_supersampled_frame_is_not_cut() {
    let (doc, page) = blurred_rects(10);
    let mut c = refine_compiler(Some(Duration::from_millis(8)));
    let frame = c.compile(&doc, page, 1, &vp(1.0), SIZE);
    let ranges = frame.plan_chunks(1.0e9).unwrap();
    assert_eq!(ranges.len(), 1, "everything fits one chunk");
    let mut ss = refine_compiler(Some(Duration::from_millis(8)));
    ss.opts.supersample = 2;
    let frame = ss.compile(&doc, page, 1, &vp(1.0), SIZE);
    assert!(frame.plan_chunks(4_000.0).is_none(), "supersampled frames are submitted whole");
}

/// On a real adapter: the page built chunk by chunk is the page the whole frame renders.
#[test]
fn chunked_render_is_pixel_identical_to_the_whole_frame() {
    let Some(mut g) = common::gpu() else { return };
    let (doc, page) = common::demo::demo_scene();
    let view = Viewport { offset: Vec2::ZERO, zoom: 1.0, dpr: 2.0 };
    let size = (1560, 2000);
    g.compiler.opts.uncap_programs = false;
    let frame = g.compile(&doc, page, 1, &view, size);
    let whole = g.render_frame(&frame, size);
    let ranges = frame.plan_chunks(1_500.0).expect("the demo frame can be cut");
    assert!(ranges.len() >= 3, "a small budget cuts the demo scene into several chunks, got {}", ranges.len());
    let mut page_tex = None;
    for (i, r) in ranges.iter().enumerate() {
        let last = i + 1 == ranges.len();
        let out = g.renderer.render_chunk(&frame, r.clone(), i, last, last);
        assert_eq!(out.is_some(), last, "the page is handed over with the last chunk only");
        page_tex = out.or(page_tex);
    }
    let pixels = g.renderer.read_output(&page_tex.expect("page"));
    assert_eq!(pixels.len(), whole.data().len());
    let differing = pixels.iter().zip(whole.data()).filter(|(a, b)| a != b).count();
    assert_eq!(differing, 0, "{differing} bytes differ between the chunked and the whole render ({} chunks)", ranges.len());
}

/// A chunk that never ran voids the rest of the page: nothing is handed over.
#[test]
fn a_skipped_chunk_hands_nothing_over() {
    let Some(mut g) = common::gpu() else { return };
    let (doc, page) = common::demo::demo_scene();
    let view = Viewport { offset: Vec2::ZERO, zoom: 1.0, dpr: 1.0 };
    let size = (780, 1000);
    let frame = g.compile(&doc, page, 1, &view, size);
    let ranges = frame.plan_chunks(1_000.0).unwrap();
    assert!(ranges.len() >= 3);
    assert!(g.renderer.render_chunk(&frame, ranges[0].clone(), 0, false, false).is_none());
    // chunk 1 is skipped
    assert!(g.renderer.render_chunk(&frame, ranges[2].clone(), 2, ranges.len() == 3, ranges.len() == 3).is_none());
    assert!(g.renderer.render_chunk(&frame, ranges.last().unwrap().clone(), ranges.len() - 1, true, true).is_none());
    // starting over works
    let mut out = None;
    for (i, r) in ranges.iter().enumerate() {
        out = g.renderer.render_chunk(&frame, r.clone(), i, i + 1 == ranges.len(), i + 1 == ranges.len());
    }
    assert!(out.is_some());
}

/// `count` clipped, rounded cards (one mask program each) holding a label and a blurred swatch.
fn cards(count: usize) -> (Document, NodeId) {
    let mut doc = Document::new("cards");
    let page = doc.add_page("Page");
    for i in 0..count {
        let (x, y) = ((i % 8) as f32 * 120.0 + 10.0, (i / 8) as f32 * 120.0 + 10.0);
        let id = doc.alloc_id();
        let mut card = Node::frame(id, Rect::new(x, y, 100.0, 100.0));
        card.corner_radii = [12.0; 4];
        if let Some(f) = card.kind.frame_props_mut() {
            f.clips_content = true;
        }
        let card_id = doc.insert(page, card).unwrap();
        let tid = doc.alloc_id();
        let style = TextStyle { font_size: 14.0, fills: vec![Paint::solid(Color::BLACK)], ..Default::default() };
        doc.insert(card_id, Node::text(tid, Rect::new(8.0, 8.0, 80.0, 18.0), format!("Card {i}"), style)).unwrap();
        let rid = doc.alloc_id();
        let mut swatch = Node::rectangle(rid, Rect::new(20.0, 40.0, 90.0, 50.0));
        swatch.fills = vec![Paint::solid(Color::rgb(0x33, (i * 5 % 255) as u8, 0xCC))];
        swatch.effects = vec![Effect::LayerBlur { radius: 3.0, visible: true }];
        doc.insert(card_id, swatch).unwrap();
    }
    (doc, page)
}

#[test]
fn slices_cover_the_top_level_nodes_in_order_and_stay_under_the_node_limit() {
    let (doc, page) = cards(40);
    let mut c = SceneCompiler::new();
    let top = c.visible_top_level(&doc, page, 1, &vp(1.0), (1200, 800)).expect("a page of 40 top-level nodes is drawn from the index");
    assert_eq!(top.len(), 40);
    let slices = designos_gpu::slice_top_level(&doc, &top, 10);
    assert!(slices.len() >= 12, "3 nodes a card, 10 a slice: about 13 slices, got {}", slices.len());
    assert_eq!(slices.first().map(|s| s.start), Some(0));
    assert_eq!(slices.last().map(|s| s.end), Some(40));
    for w in slices.windows(2) {
        assert_eq!(w[0].end, w[1].start);
    }
    for s in &slices {
        let nodes: usize = top[s.clone()].iter().map(|id| doc.page_node_count(*id)).sum();
        assert!(nodes <= 10 || s.len() == 1, "slice {s:?} holds {nodes} nodes");
    }
    // a node bigger than the limit is a slice of its own
    assert_eq!(designos_gpu::slice_top_level(&doc, &top, 1).len(), 40);
    // a small page is not drawn from the index
    let (small, small_page) = cards(5);
    assert!(SceneCompiler::new().visible_top_level(&small, small_page, 1, &vp(1.0), (1200, 800)).is_none());
}

/// On a real adapter: compiling and rendering the page slice by slice paints the page the whole compile paints.
#[test]
fn rendered_slice_by_slice_the_page_is_pixel_identical_to_the_whole_compile() {
    let Some(mut g) = common::gpu() else { return };
    let (doc, page) = cards(40);
    let view = Viewport { offset: Vec2::ZERO, zoom: 1.0, dpr: 1.0 };
    let size = (1000, 900);
    let whole = {
        let frame = g.compile(&doc, page, 1, &view, size);
        g.render_frame(&frame, size)
    };
    let top = g.compiler.visible_top_level(&doc, page, 1, &view, size).expect("indexed page");
    let slices = designos_gpu::slice_top_level(&doc, &top, 12);
    assert!(slices.len() >= 8);
    let mut index = 0;
    let mut result = None;
    for (si, s) in slices.iter().enumerate() {
        let frame = g.compiler.compile_slice(&doc, page, 1, &view, size, &top[s.clone()]);
        let ranges = frame.plan_chunks(1_200.0).expect("cuttable");
        for (ri, r) in ranges.iter().enumerate() {
            let last = si + 1 == slices.len() && ri + 1 == ranges.len();
            let out = g.renderer.render_chunk(&frame, r.clone(), index, last, ri + 1 == ranges.len());
            assert_eq!(out.is_some(), last);
            result = out.or(result);
            index += 1;
        }
    }
    let pixels = g.renderer.read_output(&result.expect("page"));
    let differing = pixels.iter().zip(whole.data()).filter(|(a, b)| a != b).count();
    assert_eq!(differing, 0, "{differing} bytes differ between {} slices in {index} chunks and the whole compile", slices.len());
}
