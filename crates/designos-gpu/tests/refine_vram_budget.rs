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
    let shadow = Effect::InnerShadow {
        color: Color::rgba(0, 0, 0, 128),
        offset: Vec2::new(0.0, 2.0),
        radius: 6.0,
        spread: 0.0,
        visible: true,
        blend: Default::default(),
    };
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
    let shadow = Effect::InnerShadow {
        color: Color::rgba(0, 0, 0, 128),
        offset: Vec2::new(0.0, 2.0),
        radius: 6.0,
        spread: 0.0,
        visible: true,
        blend: Default::default(),
    };
    let (sdoc, spage) = effect_page(BLURRED, shadow);
    let mut c2 = interactive_compiler();
    c2.opts.uncap_programs = true;
    c2.opts.refine_max_programs = 20;
    let sframe = compile(&mut c2, &sdoc, spage);
    assert!(sframe.programs.len() <= 20, "expected <= 20 programs, got {}", sframe.programs.len());
    // 1 root + 6 * 3 = 19 programs (the 7th inner-shadow node needs 3 programs and does not fit in 20)
    assert_eq!(sframe.programs.len(), 19);
}

/// Children of the effect groups below: more than the program ceiling.
const GROUP_CHILDREN: usize = 5_000;

/// A group carrying `effect` over `count` 2x2 layer-blurred rectangles. A group
/// has no shape of its own, so its shadows, blurs, glass and grain draw the
/// children into silhouette programs.
fn effect_group_page(count: usize, effect: Effect) -> (Document, NodeId) {
    let mut doc = Document::new("effect group");
    let page = doc.add_page("Page");
    let mut group = Node::new(doc.alloc_id(), NodeKind::Group);
    group.size = Vec2::new(960.0, count.div_ceil(640) as f32 * 1.5 + 2.0);
    group.effects = vec![effect];
    let group = doc.insert(page, group).unwrap();
    for i in 0..count {
        let x = ((i % 640) as f32) * 1.5;
        let y = ((i / 640) as f32) * 1.5;
        let mut n = Node::rectangle(doc.alloc_id(), Rect::new(x, y, 2.0, 2.0));
        n.fills = vec![Paint::solid(Color::rgb(0x33, 0x66, 0xCC))];
        n.effects = vec![Effect::LayerBlur { radius: 1.0, visible: true }];
        doc.insert(group, n).unwrap();
    }
    (doc, page)
}

fn drop_shadow(show_behind: bool) -> Effect {
    Effect::DropShadow {
        color: Color::rgba(0, 0, 0, 128),
        offset: Vec2::new(4.0, 4.0),
        radius: 8.0,
        spread: 0.0,
        visible: true,
        show_behind,
        blend: Default::default(),
    }
}

fn inner_shadow() -> Effect {
    Effect::InnerShadow { color: Color::rgba(0, 0, 0, 128), offset: Vec2::new(0.0, 2.0), radius: 6.0, spread: 0.0, visible: true, blend: Default::default() }
}

/// Every group effect that draws the group's children into a silhouette program.
fn silhouette_effects() -> Vec<(&'static str, Effect)> {
    vec![
        ("drop shadow with the shape hidden behind it", drop_shadow(false)),
        ("drop shadow shown behind", drop_shadow(true)),
        ("inner shadow", inner_shadow()),
        ("background blur", Effect::BackgroundBlur { radius: 6.0, visible: true }),
        (
            "glass",
            Effect::Glass(GlassEffect { light_angle: 45.0, light_intensity: 0.7, refraction: 0.5, depth: 10.0, dispersion: 0.3, frost: 4.0, visible: true }),
        ),
        (
            "noise",
            Effect::Noise(NoiseEffect { mode: NoiseMode::Multi, size: 1.0, density: 0.5, opacity: 0.5, blend: BlendMode::Normal, seed: 7, visible: true }),
        ),
    ]
}

fn refine_compiler() -> SceneCompiler {
    let mut c = interactive_compiler();
    c.opts.uncap_programs = true;
    c
}

#[test]
fn effect_silhouette_subtrees_stay_under_the_program_ceiling() {
    let mut violations = Vec::new();
    for (name, effect) in silhouette_effects() {
        let (doc, page) = effect_group_page(GROUP_CHILDREN, effect);

        // without a ceiling the silhouette walks open one layer per child and more
        let mut open = refine_compiler();
        open.opts.refine_max_programs = usize::MAX;
        open.opts.refine_vram_budget_bytes = u64::MAX;
        let unbounded = compile(&mut open, &doc, page).programs.len();
        assert!(unbounded > DEFAULT_REFINE_MAX_PROGRAMS, "{name}: the page must need more than the ceiling, opened {unbounded}");

        let mut c = refine_compiler();
        let frame = compile(&mut c, &doc, page);
        if frame.programs.len() != DEFAULT_REFINE_MAX_PROGRAMS {
            violations.push(format!(
                "{name}: {GROUP_CHILDREN} blurred children opened {} programs, the refine must use up exactly the ceiling of {DEFAULT_REFINE_MAX_PROGRAMS}",
                frame.programs.len()
            ));
        }
    }
    assert!(violations.is_empty(), "{}", violations.join("\n"));
}

#[test]
fn effect_silhouette_subtrees_stay_within_the_vram_budget() {
    let mut violations = Vec::new();
    for (name, effect) in [("drop shadow", drop_shadow(false)), ("inner shadow", inner_shadow())] {
        let (doc, page) = effect_group_page(GROUP_CHILDREN, effect);
        for offscreen_budget in [8 * MIB, 12 * MIB, 16 * MIB, 24 * MIB] {
            let budget = ROOT_BYTES_1000 + offscreen_budget;
            let mut c = refine_compiler();
            c.opts.refine_vram_budget_bytes = budget;
            let frame = compile(&mut c, &doc, page);
            let bytes = recount(&frame);
            assert_eq!(c.last_vram_bytes, bytes, "{name}, budget {budget}: running total must equal the recount");
            if bytes > budget {
                violations.push(format!("{name}, budget {budget}: frame holds {bytes} bytes in {} programs", frame.programs.len()));
            }
            assert!(frame.programs.len() > 3, "{name}, budget {budget}: the effect and its children must still draw, {} programs", frame.programs.len());
        }
    }
    assert!(violations.is_empty(), "{}", violations.join("\n"));
}

#[test]
fn capped_and_headless_group_compiles_ignore_the_refine_limits() {
    for (name, effect) in silhouette_effects() {
        let (doc, page) = effect_group_page(600, effect);
        let stats = |c: &mut SceneCompiler| {
            let f = compile(c, &doc, page);
            let s = f.stats();
            (f.programs.len(), s.draws, s.composites, s.backdrops, s.triangles)
        };
        let mut capped = interactive_compiler();
        let today = stats(&mut capped);
        let mut tight = interactive_compiler();
        tight.opts.refine_vram_budget_bytes = 1;
        tight.opts.refine_max_programs = 2;
        assert_eq!(stats(&mut tight), today, "{name}: refine limits must not change a capped frame");

        let mut headless = SceneCompiler::new();
        let unlimited = stats(&mut headless);
        let mut limited = SceneCompiler::new();
        limited.opts.uncap_programs = true;
        limited.opts.refine_vram_budget_bytes = 1;
        limited.opts.refine_max_programs = 2;
        assert_eq!(stats(&mut limited), unlimited, "{name}: refine limits must not change a headless frame");
        assert!(unlimited.0 > 600, "{name}: headless frame opens a program per blurred child, got {}", unlimited.0);
    }
}

#[test]
fn capped_silhouette_subtrees_keep_the_interactive_program_cap() {
    // The cap of 64 is checked before an effect opens its programs, so the last
    // effect of a frame can end up to two programs past it.
    for (name, effect) in silhouette_effects() {
        let (doc, page) = effect_group_page(GROUP_CHILDREN, effect);
        let mut c = interactive_compiler();
        let n = compile(&mut c, &doc, page).programs.len();
        assert!((60..=66).contains(&n), "{name}: a capped frame opened {n} programs for {GROUP_CHILDREN} blurred children, cap is 64");
    }
}

/// A group with a layer blur over `count` blurred rectangles; `shadowed` puts it
/// inside a group with a drop shadow, so it is also drawn as a silhouette.
fn blurred_parent_page(count: usize, shadowed: bool) -> (Document, NodeId) {
    let mut doc = Document::new("blurred parent");
    let page = doc.add_page("Page");
    let outer = if shadowed {
        let mut shell = Node::new(doc.alloc_id(), NodeKind::Group);
        shell.size = Vec2::new(250.0, 250.0);
        shell.effects = vec![drop_shadow(false)];
        doc.insert(page, shell).unwrap()
    } else {
        page
    };
    let mut parent = Node::new(doc.alloc_id(), NodeKind::Group);
    parent.size = Vec2::new(250.0, 250.0);
    parent.effects = vec![Effect::LayerBlur { radius: 6.0, visible: true }];
    let parent = doc.insert(outer, parent).unwrap();
    for i in 0..count {
        let mut n = Node::rectangle(doc.alloc_id(), Rect::new((i % 10) as f32 * 22.0, (i / 10) as f32 * 22.0, 12.0, 12.0));
        n.effects = vec![Effect::LayerBlur { radius: 2.0, visible: true }];
        doc.insert(parent, n).unwrap();
    }
    (doc, page)
}

#[test]
fn a_blurred_layer_reserves_its_blur_targets_while_its_children_draw() {
    let mut violations = Vec::new();
    for shadowed in [false, true] {
        let (doc, page) = blurred_parent_page(100, shadowed);
        for step in 0..=100u64 {
            let budget = ROOT_BYTES_1000 + step * 100_000;
            let mut c = refine_compiler();
            c.opts.refine_vram_budget_bytes = budget;
            let frame = compile(&mut c, &doc, page);
            let bytes = recount(&frame);
            assert_eq!(c.last_vram_bytes, bytes, "shadowed {shadowed}, budget {budget}: running total must equal the recount");
            if bytes > budget {
                violations.push(format!("shadowed {shadowed}, budget {budget}: frame holds {bytes} bytes in {} programs", frame.programs.len()));
            }
        }
    }
    assert!(violations.is_empty(), "{}", violations.join("\n"));
}

/// `[blur, mask, blur, blur, blur]` and, inside a frame, the same again, so a
/// mask program is opened after the layer that holds the masked siblings and
/// again one level down.
fn masked_page() -> (Document, NodeId) {
    let mut doc = Document::new("masked");
    let page = doc.add_page("Page");
    let blurred = |doc: &mut Document, parent: NodeId, x: f32, y: f32| {
        let mut n = Node::rectangle(doc.alloc_id(), Rect::new(x, y, 60.0, 60.0));
        n.effects = vec![Effect::LayerBlur { radius: 4.0, visible: true }];
        doc.insert(parent, n).unwrap();
    };
    let mask = |doc: &mut Document, parent: NodeId, w: f32, h: f32| {
        let mut m = Node::rectangle(doc.alloc_id(), Rect::new(0.0, 0.0, w, h));
        m.is_mask = true;
        doc.insert(parent, m).unwrap();
    };
    blurred(&mut doc, page, 10.0, 10.0);
    mask(&mut doc, page, 900.0, 420.0);
    for x in [100.0, 200.0, 300.0] {
        blurred(&mut doc, page, x, 100.0);
    }
    let frame_id = doc.alloc_id();
    let frame = doc.insert(page, Node::frame(frame_id, Rect::new(500.0, 0.0, 400.0, 400.0))).unwrap();
    blurred(&mut doc, frame, 10.0, 10.0);
    mask(&mut doc, frame, 300.0, 300.0);
    for x in [100.0, 150.0, 200.0] {
        blurred(&mut doc, frame, x, 100.0);
    }
    (doc, page)
}

#[test]
fn mask_programs_are_counted_before_the_masked_siblings_are_drawn() {
    let (doc, page) = masked_page();
    let mut open = refine_compiler();
    let full = compile(&mut open, &doc, page).programs.len();
    assert!(full >= 12, "the page needs layers, mask layers and mask programs, got {full}");
    for max in 1..=full + 1 {
        let mut c = refine_compiler();
        c.opts.refine_max_programs = max;
        let frame = compile(&mut c, &doc, page);
        assert!(frame.programs.len() <= max, "refine_max_programs {max}: opened {} programs", frame.programs.len());
    }
}

#[test]
fn mask_programs_are_budgeted_before_the_masked_siblings_are_drawn() {
    let (doc, page) = masked_page();
    for step in 0..=60u64 {
        let budget = ROOT_BYTES_1000 + step * 400_000;
        let mut c = refine_compiler();
        c.opts.refine_vram_budget_bytes = budget;
        let frame = compile(&mut c, &doc, page);
        let bytes = recount(&frame);
        assert!(bytes <= budget, "budget {budget}: frame holds {bytes} bytes in {} programs", frame.programs.len());
        assert_eq!(c.last_vram_bytes, bytes, "budget {budget}: running total must equal the recount");
    }
}

#[test]
fn frames_at_the_refine_limits_render_on_the_gpu() {
    let Some(mut g) = common::gpu() else { return };
    g.compiler.geom.budget = Some(Duration::from_millis(8));
    g.compiler.opts.uncap_programs = true;
    let vp = Viewport { offset: Vec2::new(0.0, 0.0), zoom: 1.0, dpr: 1.0 };
    let (masked, masked_root) = masked_page();
    let (shadowed, shadowed_root) = effect_group_page(100, drop_shadow(false));
    // each document gets its own revision: the geometry cache is keyed by it
    for (rev, name, doc, page, limited) in [(1u64, "masks", &masked, masked_root, 4usize), (2, "shadowed-group", &shadowed, shadowed_root, 40)] {
        for (label, max) in [("limited", limited), ("full", DEFAULT_REFINE_MAX_PROGRAMS)] {
            g.compiler.opts.refine_max_programs = max;
            let frame = g.compiler.compile(doc, page, rev, &vp, SIZE);
            assert!(frame.programs.len() <= max, "{name} {label}: {} programs, limit {max}", frame.programs.len());
            let img = common::to_img(&g.render_frame(&frame, SIZE));
            img.save(common::out_dir().join(format!("refine-limits-{name}-{label}.png"))).unwrap();
            let background = *img.get_pixel(0, 0);
            let painted = img.pixels().filter(|p| **p != background).count();
            assert!(painted > 0, "{name} {label}: the frame is blank");
        }
    }
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
}

/// Name of the environment variable that points the real-document check at a
/// `.fig` file; without it the check is skipped.
const FIXTURE_ENV: &str = "DESIGNOS_REFINE_FIXTURE_FIG";

#[test]
fn pools_trim_on_a_real_document_when_a_fixture_is_given() {
    let Some(fig_path) = std::env::var_os(FIXTURE_ENV).map(std::path::PathBuf::from) else {
        eprintln!("skipped: set {FIXTURE_ENV} to a .fig file to measure the refine on a real document");
        return;
    };
    let bytes = match std::fs::read(&fig_path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("skipped: cannot read {}: {e}", fig_path.display());
            return;
        }
    };
    let doc_fig = match designos_fig::import_fig(&bytes) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("skipped: cannot import {}: {e}", fig_path.display());
            return;
        }
    };
    let Some(mut g) = common::gpu() else { return };
    let Some(mut fresh_gpu) = common::gpu() else { return };
    g.compiler.geom.budget = Some(Duration::from_millis(8));
    let size = (1440, 900);
    let Some(largest_page) = doc_fig.pages.iter().copied().max_by_key(|&p| doc_fig.page_node_count(p)) else {
        eprintln!("skipped: {} has no pages", fig_path.display());
        return;
    };
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
    fresh_gpu.compiler.geom.budget = Some(Duration::from_millis(8));
    fresh_gpu.compiler.opts.uncap_programs = true;
    let vp4 = Viewport { offset: center, zoom: 4.0, dpr: 2.0 };
    let f4 = fresh_gpu.compiler.compile(&doc_fig, largest_page, 1, &vp4, size);
    let _ = fresh_gpu.renderer.render_submitted(&f4);
    let single_fig_pool = fresh_gpu.renderer.stats.pool_bytes;
    eprintln!(
        "{}: max refine programs = {max_progs} (ceiling = {DEFAULT_REFINE_MAX_PROGRAMS}), pool after zoom sweep = {:.2} MiB vs single frame = {:.2} MiB",
        fig_path.display(),
        final_fig_pool as f64 / MIB as f64,
        single_fig_pool as f64 / MIB as f64
    );
    assert!(max_progs <= DEFAULT_REFINE_MAX_PROGRAMS);
    assert_eq!(final_fig_pool, single_fig_pool);
}
