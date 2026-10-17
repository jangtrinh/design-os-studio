//! VRAM and program-count ceilings of the idle refine compile: with
//! `uncap_programs` set, offscreen programs are created only while their
//! accounted GPU bytes (including the root output target) stay within
//! `refine_vram_budget_bytes` AND total programs stay within
//! `refine_max_programs`. Also verifies that pooled GPU targets are trimmed to
//! one frame's working set after a refine and across a zoom sequence.

mod common;

use designos_doc::*;
use designos_gpu::{CompileOptions, Frame, SceneCompiler, DEFAULT_REFINE_MAX_PROGRAMS};
use designos_render::Viewport;
use std::time::Duration;

const BLURRED: usize = 100;
const SIZE: (u32, u32) = (1000, 1000);
const MIB: u64 = 1024 * 1024;
const ROOT_BYTES_1000: u64 = 1000 * 1000 * 4 * (4 + 1);

/// `count` layer-blurred rectangles inside the view.
fn blurred_page(count: usize) -> (Document, NodeId) {
    effect_page(count, Effect::LayerBlur { radius: 4.0, visible: true })
}

/// `count` rectangles inside the view, each carrying `effect`.
fn effect_page(count: usize, effect: Effect) -> (Document, NodeId) {
    let mut doc = Document::new("vram budget");
    let page = doc.add_page("Page");
    for i in 0..count {
        let id = doc.alloc_id();
        let (col, row) = ((i % 10) as f32, (i / 10) as f32);
        let mut n = Node::rectangle(id, Rect::new(col * 90.0 + 10.0, row * 90.0 + 10.0, 60.0, 60.0));
        n.fills = vec![Paint::solid(Color::rgb(0x33, 0x66, 0xCC))];
        n.effects = vec![effect.clone()];
        doc.insert(page, n).unwrap();
    }
    (doc, page)
}

fn interactive_compiler() -> SceneCompiler {
    let mut c = SceneCompiler::new();
    c.geom.budget = Some(Duration::from_millis(8));
    c
}

fn compile(c: &mut SceneCompiler, doc: &Document, page: NodeId) -> Frame {
    let vp = Viewport { offset: Vec2::new(0.0, 0.0), zoom: 1.0, dpr: 1.0 };
    c.compile(doc, page, 1, &vp, SIZE)
}

/// Pool rounding of `resources.rs`: 8..64 powers of two, then multiples of 64.
fn pool_extent(v: i32) -> u64 {
    let v = v.max(1) as u64;
    if v <= 64 {
        v.next_power_of_two().max(8)
    } else {
        v.div_ceil(64) * 64
    }
}

/// Bytes of the root target (`output` + `output_ms`, plus pooled root when `ss > 1`).
fn root_bytes(frame: &Frame) -> u64 {
    let out_px = frame.out_size.0.max(1) as u64 * frame.out_size.1.max(1) as u64;
    let ms_px = frame.size.0.max(1) as u64 * frame.size.1.max(1) as u64;
    let base = (out_px + ms_px * 4) * 4;
    if frame.ss > 1 {
        let px = pool_extent(frame.size.0 as i32) * pool_extent(frame.size.1 as i32);
        base + px * 4 * (4 + 1)
    } else {
        base
    }
}

/// Bytes of the root target plus every offscreen program of `frame`: MSAA
/// colour (4 samples) + resolve target, Rgba8Unorm, plus two plain targets per
/// blur. Only layer blurs occur in these documents.
fn recount(frame: &Frame) -> u64 {
    root_bytes(frame)
        + frame
            .programs
            .iter()
            .skip(1)
            .map(|p| {
                let px = pool_extent(p.rect.w()) * pool_extent(p.rect.h());
                px * 4 * (4 + 1) + px * 4 * 2 * p.post.len() as u64
            })
            .sum::<u64>()
}

#[test]
fn default_budget_is_512_mib() {
    assert_eq!(CompileOptions::default().refine_vram_budget_bytes, 512 * MIB);
    assert_eq!(CompileOptions::default().refine_max_programs, DEFAULT_REFINE_MAX_PROGRAMS);
}

#[test]
fn root_target_is_counted_in_vram_bytes() {
    let (doc, page) = blurred_page(0);
    let mut c = interactive_compiler();
    c.opts.uncap_programs = true;
    let frame = compile(&mut c, &doc, page);
    assert_eq!(frame.programs.len(), 1, "root only");
    assert_eq!(c.last_vram_bytes, ROOT_BYTES_1000);
    assert_eq!(recount(&frame), c.last_vram_bytes);

    // At 1440x900 @2x (2880x1800), the root target is 2880 * 1800 * 20 = 103,680,000 B (~98.88 MiB)
    let vp = Viewport { offset: Vec2::new(0.0, 0.0), zoom: 1.0, dpr: 2.0 };
    let retina = c.compile(&doc, page, 1, &vp, (2880, 1800));
    assert_eq!(c.last_vram_bytes, 2880 * 1800 * 20);
    assert_eq!(recount(&retina), c.last_vram_bytes);

    // When the budget equals the root target alone, no offscreen programs fit
    let (doc1, page1) = blurred_page(4);
    c.opts.refine_vram_budget_bytes = ROOT_BYTES_1000;
    let tight = compile(&mut c, &doc1, page1);
    assert_eq!(tight.programs.len(), 1, "no offscreen programs fit when budget equals root target");
    assert_eq!(c.last_vram_bytes, ROOT_BYTES_1000);
}

#[test]
fn one_blurred_node_costs_msaa_resolve_and_two_blur_targets() {
    // root (1000x1000x20 B) + layer 76x76 (60 + 2 * (1 + 7) px blur reach) -> pooled 128x128, 28 B/px
    let (doc, page) = blurred_page(1);
    let mut c = interactive_compiler();
    c.opts.uncap_programs = true;
    let frame = compile(&mut c, &doc, page);
    assert_eq!(frame.programs.len(), 2, "root + one layer");
    assert_eq!(c.last_vram_bytes, ROOT_BYTES_1000 + 128 * 128 * 28);
    assert_eq!(recount(&frame), c.last_vram_bytes);
}

#[test]
fn refine_stops_creating_programs_once_the_budget_is_reached() {
    let (doc, page) = blurred_page(BLURRED);
    let mut unbounded = interactive_compiler();
    unbounded.opts.uncap_programs = true;
    let full = compile(&mut unbounded, &doc, page);
    assert!(full.programs.len() > BLURRED, "default budget must admit all {BLURRED} layers");

    for offscreen_budget in [MIB, 4 * MIB, 16 * MIB] {
        let budget = ROOT_BYTES_1000 + offscreen_budget;
        let mut c = interactive_compiler();
        c.opts.uncap_programs = true;
        c.opts.refine_vram_budget_bytes = budget;
        let frame = compile(&mut c, &doc, page);
        let bytes = recount(&frame);
        assert!(frame.programs.len() > 1, "budget {budget}: at least one layer fits");
        assert!(frame.programs.len() < full.programs.len(), "budget {budget}: programs {} not below {}", frame.programs.len(), full.programs.len());
        assert!(bytes <= budget, "budget {budget}: frame holds {bytes} bytes");
        assert_eq!(c.last_vram_bytes, bytes, "running total must equal the recount");
        // a further layer (458_752 B) no longer fits: the budget is used up, not wasted
        assert!(bytes + 128 * 128 * 28 > budget, "budget {budget}: stopped early at {bytes} bytes");
    }
}

#[test]
fn bigger_budget_admits_more_programs() {
    let (doc, page) = blurred_page(BLURRED);
    let count = |budget: u64| {
        let mut c = interactive_compiler();
        c.opts.uncap_programs = true;
        c.opts.refine_vram_budget_bytes = budget;
        compile(&mut c, &doc, page).programs.len()
    };
    assert!(count(ROOT_BYTES_1000 + 2 * MIB) < count(ROOT_BYTES_1000 + 8 * MIB));
}

#[test]
fn capped_compile_ignores_the_vram_budget() {
    let (doc, page) = blurred_page(BLURRED);
    let capped = |budget: u64| {
        let mut c = interactive_compiler();
        c.opts.refine_vram_budget_bytes = budget;
        let f = compile(&mut c, &doc, page);
        let s = f.stats();
        (f.programs.len(), s.draws, s.composites, s.backdrops, s.triangles)
    };
    let (today, tiny, huge) = (capped(512 * MIB), capped(1), capped(u64::MAX));
    assert!(today.0 <= 64 && today.0 >= 60, "capped compile keeps the 64-program cap, got {}", today.0);
    assert_eq!(today, tiny, "budget must not change an interactive (capped) frame");
    assert_eq!(today, huge);
}

#[test]
fn headless_compile_stays_uncapped_whatever_the_budget() {
    let (doc, page) = blurred_page(BLURRED);
    let mut c = SceneCompiler::new();
    assert!(c.geom.budget.is_none());
    c.opts.uncap_programs = true;
    c.opts.refine_vram_budget_bytes = 1;
    c.opts.refine_max_programs = 2;
    let frame = compile(&mut c, &doc, page);
    assert!(frame.programs.len() > BLURRED, "headless frame must open a program per blurred node, got {}", frame.programs.len());
}

#[test]
fn effect_programs_of_a_node_are_budgeted_together() {
    // an inner shadow opens three programs (hole, grain layer, shape) and blurs one
    let shadow = Effect::InnerShadow { color: Color::rgba(0, 0, 0, 128), offset: Vec2::new(0.0, 2.0), radius: 6.0, spread: 0.0, visible: true };
    let (doc, page) = effect_page(BLURRED, shadow);
    let mut unbounded = interactive_compiler();
    unbounded.opts.uncap_programs = true;
    let full = compile(&mut unbounded, &doc, page).programs.len();
    assert!(full > 3 * BLURRED, "default budget must admit every inner shadow, got {full} programs");
    for offscreen_budget in [4 * MIB, 16 * MIB, 32 * MIB] {
        let budget = ROOT_BYTES_1000 + offscreen_budget;
        let mut c = interactive_compiler();
        c.opts.uncap_programs = true;
        c.opts.refine_vram_budget_bytes = budget;
        let frame = compile(&mut c, &doc, page);
        let bytes = recount(&frame);
        assert!(frame.programs.len() > 1 && frame.programs.len() < full, "budget {budget}: {} of {full} programs", frame.programs.len());
        assert!(bytes <= budget, "budget {budget}: frame holds {bytes} bytes");
        assert_eq!(c.last_vram_bytes, bytes);
    }
}

#[test]
fn four_hundred_thousand_tiny_effect_nodes_stay_under_the_program_ceiling() {
    // 400,000 tiny (2x2) blurred rectangles inside the 1000x1000 viewport:
    // each 8x8 pooled program costs only 1,792 bytes (1,280 B + 512 B blur), so
    // the 512 MiB byte budget alone would allow ~299,000 programs. The pass-count
    // backstop (`DEFAULT_REFINE_MAX_PROGRAMS`) caps the refine at 4,096 programs.
    let mut doc = Document::new("400k tiny effect nodes");
    let page = doc.add_page("Page");
    let blur = vec![Effect::LayerBlur { radius: 1.0, visible: true }];
    let fill = vec![Paint::solid(Color::rgb(0x33, 0x66, 0xCC))];
    for i in 0..400_000usize {
        let id = doc.alloc_id();
        let x = ((i % 640) as f32) * 1.5;
        let y = ((i / 640) as f32) * 1.5;
        let mut n = Node::rectangle(id, Rect::new(x, y, 2.0, 2.0));
        n.fills = fill.clone();
        n.effects = blur.clone();
        doc.insert(page, n).unwrap();
    }

    let mut c = interactive_compiler();
    c.opts.uncap_programs = true;
    let frame = compile(&mut c, &doc, page);
    assert!(
        frame.programs.len() <= DEFAULT_REFINE_MAX_PROGRAMS,
        "400k tiny effect nodes opened {} programs, exceeding ceiling {}",
        frame.programs.len(),
        DEFAULT_REFINE_MAX_PROGRAMS
    );
    assert_eq!(frame.programs.len(), DEFAULT_REFINE_MAX_PROGRAMS, "refine should use up the program ceiling");

    // Also verify a smaller custom program ceiling on the 100-node inner-shadow page (3 programs per node)
    let shadow = Effect::InnerShadow { color: Color::rgba(0, 0, 0, 128), offset: Vec2::new(0.0, 2.0), radius: 6.0, spread: 0.0, visible: true };
    let (sdoc, spage) = effect_page(BLURRED, shadow);
    let mut c2 = interactive_compiler();
    c2.opts.uncap_programs = true;
    c2.opts.refine_max_programs = 20;
    let sframe = compile(&mut c2, &sdoc, spage);
    assert!(sframe.programs.len() <= 20, "expected <= 20 programs, got {}", sframe.programs.len());
    // 1 root + 6 * 3 = 19 programs (the 7th inner-shadow node needs 3 programs and does not fit in 20)
    assert_eq!(sframe.programs.len(), 19);
}

#[test]
fn pools_trim_unused_targets_after_refine_and_zoom_sequence() {
    let Some(mut g) = common::gpu() else { return };
    let (doc, page) = blurred_page(40);
    g.compiler.geom.budget = Some(Duration::from_millis(8));
    let size = (1440, 900);

    // Measure the single-frame working set at zoom = 4.0 on a clean renderer
    let vp_final = Viewport { offset: Vec2::new(0.0, 0.0), zoom: 4.0, dpr: 2.0 };
    g.compiler.opts.uncap_programs = true;
    let final_frame = g.compiler.compile(&doc, page, 1, &vp_final, size);
    let _ = g.renderer.render_submitted(&final_frame);
    let single_frame_pool_bytes = g.renderer.stats.pool_bytes;
    assert!(single_frame_pool_bytes > 0, "expected non-zero pool working set");

    // Now run a zoom sequence 4.0 -> 1.9 -> 4.0 with the real app flow (capped frame, then refine)
    let zooms = [4.0f32, 3.5, 3.0, 2.5, 2.1, 1.9, 2.3, 2.8, 3.4, 4.0];
    let mut peak_pool_bytes = 0u64;
    for z in zooms {
        let vp = Viewport { offset: Vec2::new(0.0, 0.0), zoom: z, dpr: 2.0 };
        g.compiler.opts.uncap_programs = false;
        let capped = g.compiler.compile(&doc, page, 1, &vp, size);
        let _ = g.renderer.render_submitted(&capped);
        peak_pool_bytes = peak_pool_bytes.max(g.renderer.stats.pool_bytes);

        g.compiler.opts.uncap_programs = true;
        let refined = g.compiler.compile(&doc, page, 1, &vp, size);
        let _ = g.renderer.render_submitted(&refined);
        peak_pool_bytes = peak_pool_bytes.max(g.renderer.stats.pool_bytes);
    }
    let after_zoom_pool_bytes = g.renderer.stats.pool_bytes;
    assert_eq!(
        after_zoom_pool_bytes, single_frame_pool_bytes,
        "pool residency after zoom sequence ({after_zoom_pool_bytes} B) must return to one frame's working set ({single_frame_pool_bytes} B)"
    );

    // Real-world benchmark measurement on the 112,474-node (`723,136` Kiwi records) VSF - PCP fixture when present on disk
    let fig_path = std::path::Path::new("/Users/jangtrinh/Downloads/VSF - PCP.fig");
    if fig_path.exists() {
        let bytes = std::fs::read(fig_path).expect("read VSF - PCP.fig");
        let doc_fig = designos_fig::import_fig(&bytes).expect("import VSF - PCP.fig");
        let largest_page = doc_fig.pages.iter().copied().max_by_key(|&p| doc_fig.page_node_count(p)).unwrap();
        let mut idx = designos_render::SceneIndex::new();
        idx.ensure(&doc_fig, largest_page, 1);
        let mut bb = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        if let Some(p) = doc_fig.get(largest_page) {
            for &cid in &p.children {
                if let Some(b) = idx.bounds_of(cid) {
                    bb.0 = bb.0.min(b.0);
                    bb.1 = bb.1.min(b.1);
                    bb.2 = bb.2.max(b.2);
                    bb.3 = bb.3.max(b.3);
                }
            }
        }
        let center = Vec2::new(bb.0 + (bb.2 - bb.0) * 0.35, bb.1 + (bb.3 - bb.1) * 0.35);
        let mut max_progs = 0usize;
        for z in [0.05f32, 0.25, 0.5, 1.0, 1.9, 4.0] {
            let vp = Viewport { offset: center, zoom: z, dpr: 2.0 };
            g.compiler.opts.uncap_programs = false;
            let capped = g.compiler.compile(&doc_fig, largest_page, 1, &vp, size);
            let _ = g.renderer.render_submitted(&capped);
            g.compiler.opts.uncap_programs = true;
            let refined = g.compiler.compile(&doc_fig, largest_page, 1, &vp, size);
            max_progs = max_progs.max(refined.programs.len());
            let _ = g.renderer.render_submitted(&refined);
        }
        let final_fig_pool = g.renderer.stats.pool_bytes;
        let mut fresh_gpu = common::gpu().unwrap();
        fresh_gpu.compiler.geom.budget = Some(Duration::from_millis(8));
        fresh_gpu.compiler.opts.uncap_programs = true;
        let vp4 = Viewport { offset: center, zoom: 4.0, dpr: 2.0 };
        let f4 = fresh_gpu.compiler.compile(&doc_fig, largest_page, 1, &vp4, size);
        let _ = fresh_gpu.renderer.render_submitted(&f4);
        let single_fig_pool = fresh_gpu.renderer.stats.pool_bytes;
        eprintln!(
            "VSF - PCP.fig (112,474 nodes / 723,136 Kiwi records): max refine programs = {max_progs} (ceiling = {DEFAULT_REFINE_MAX_PROGRAMS}), pool after zoom sweep = {:.2} MiB vs single frame = {:.2} MiB",
            final_fig_pool as f64 / MIB as f64,
            single_fig_pool as f64 / MIB as f64
        );
        assert!(max_progs <= DEFAULT_REFINE_MAX_PROGRAMS);
        assert_eq!(final_fig_pool, single_fig_pool);
    }
}
