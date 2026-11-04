//! #520: CPU and GPU agree on the fixture pair of `520-spec.md` section 2 (white frame 1, fill-less
//! frame 2 under rule F2). Needs a hardware GPU adapter and skips without one.

mod common;

use designos_doc::*;
use designos_render::Viewport;

const SIZE: (u32, u32) = (640, 360);
const BLUE_A: Color = Color { r: 0x25 as f32 / 255.0, g: 0x63 as f32 / 255.0, b: 0xEB as f32 / 255.0, a: 1.0 };
const RED_B: Color = Color { r: 0xDC as f32 / 255.0, g: 0x26 as f32 / 255.0, b: 0x26 as f32 / 255.0, a: 0.5 };

fn frame(t: &mut common::T, x: f32, white: bool, show_behind: bool) {
    let f = t.add(None, |id| {
        let mut n = Node::frame(id, Rect::new(x, 60.0, 240.0, 160.0));
        n.fills = if white { vec![Paint::solid(Color::WHITE)] } else { vec![] };
        n.effects = vec![Effect::DropShadow {
            color: Color { r: 0.0, g: 0.0, b: 0.0, a: 0.5 },
            offset: Vec2::new(0.0, 12.0),
            radius: 24.0,
            spread: 0.0,
            visible: true,
            show_behind,
            blend: Default::default(),
        }];
        n
    });
    t.rect(Some(f), Rect::new(60.0, 50.0, 120.0, 60.0), BLUE_A);
    t.rect(Some(f), Rect::new(20.0, 110.0, 40.0, 30.0), RED_B);
}

fn pair(show_behind: bool) -> (Document, NodeId) {
    let mut t = common::T::new();
    frame(&mut t, 40.0, true, show_behind);
    frame(&mut t, 360.0, false, show_behind);
    (t.doc, t.page)
}

#[test]
fn cpu_and_gpu_match_on_the_fixture_pair() {
    let Some(mut g) = common::gpu() else { return };
    let (doc, page) = pair(false);
    let vp = Viewport { offset: Vec2::new(0.0, 0.0), zoom: 1.0, dpr: 1.0 };
    let c = common::compare(&mut g, "fillless-fixture-pair", &doc, page, vp, SIZE, 8);
    assert!(c.report.ratio < 0.02, "strict mismatch {:.3}%", c.report.ratio * 100.0);
    // frame 2 probes P1 (inside, under child A's shadow), P2 (centre of child B), P3 (below): the
    // GPU pixel is the CPU pixel, so the GPU follows rule F2 wherever the CPU does
    for (dx, dy) in [(120, 128), (40, 125), (120, 170)] {
        let (x, y) = (360 + dx, 60 + dy);
        let (a, b) = (c.cpu.pixel(x, y).unwrap(), c.gpu.pixel(x, y).unwrap());
        let d = [a.red().abs_diff(b.red()), a.green().abs_diff(b.green()), a.blue().abs_diff(b.blue())];
        assert!(d.iter().all(|v| *v <= 8), "probe ({dx}, {dy}): cpu {a:?} gpu {b:?}");
    }
    // P1 carries shadow on the GPU too: it is darker than the white page
    assert!(c.gpu.pixel(480, 188).unwrap().red() < 245, "frame 2 P1 keeps the shadow on the GPU");
}

#[test]
fn cpu_and_gpu_match_on_the_fixture_pair_with_show_behind() {
    let Some(mut g) = common::gpu() else { return };
    let (doc, page) = pair(true);
    let vp = Viewport { offset: Vec2::new(0.0, 0.0), zoom: 1.0, dpr: 1.0 };
    let c = common::compare(&mut g, "fillless-fixture-pair-behind", &doc, page, vp, SIZE, 8);
    assert!(c.report.ratio < 0.02, "strict mismatch {:.3}%", c.report.ratio * 100.0);
}

// ---------------------------------------------------------------------------
// Fill-less frame shadow knockout follows what the children paint
// ---------------------------------------------------------------------------

use designos_diff::{diff_image, DiffOptions};
use designos_render::tiny_skia::Pixmap;
use designos_render::{render_page, TileBudget, TileCache};

fn compare_doc(g: &mut designos_gpu::HeadlessGpu, name: &str, doc: &Document, page: NodeId, vp: Viewport, size: (u32, u32), tolerance: u8) -> common::Cmp {
    g.compiler = designos_gpu::SceneCompiler::new();
    g.compiler.opts.max_texture = g.renderer.max_texture_dimension();
    common::compare(g, name, doc, page, vp, size, tolerance)
}

fn px(pm: &Pixmap, x: u32, y: u32) -> [u8; 4] {
    let p = pm.pixel(x, y).unwrap();
    [p.red(), p.green(), p.blue(), p.alpha()]
}

/// Writes the CPU and GPU renders next to the other fidelity outputs when
/// `SAVE_SHADOW_KNOCKOUT_CROPS` is `before` or `after`.
fn maybe_save_crop(slug: &str, cpu: &Pixmap, gpu: &Pixmap) {
    let Ok(stage) = std::env::var("SAVE_SHADOW_KNOCKOUT_CROPS") else { return };
    if stage != "before" && stage != "after" {
        return;
    }
    let dir = common::out_dir();
    cpu.save_png(dir.join(format!("{slug}-cpu-{stage}.png"))).unwrap();
    gpu.save_png(dir.join(format!("{slug}-gpu-{stage}.png"))).unwrap();
}

fn right_shadow() -> Effect {
    Effect::DropShadow {
        color: Color { r: 0.0, g: 0.0, b: 0.0, a: 0.65 },
        offset: Vec2::new(18.0, 0.0),
        radius: 8.0,
        spread: 0.0,
        visible: true,
        show_behind: false,
        blend: Default::default(),
    }
}

fn case_a_doc(with_paint_free_children: bool) -> common::T {
    let mut t = common::T::new();
    let f = t.add(None, |id| {
        let mut n = Node::frame(id, Rect::new(20.0, 20.0, 120.0, 80.0));
        n.fills.clear();
        n.effects = vec![right_shadow()];
        n
    });
    t.rect(Some(f), Rect::new(0.0, 0.0, 50.0, 80.0), BLUE_A);
    if with_paint_free_children {
        t.add(Some(f), |id| {
            let mut n = Node::frame(id, Rect::new(50.0, 0.0, 70.0, 40.0));
            n.fills.clear();
            n
        });
        t.add(Some(f), |id| {
            let mut n = Node::frame(id, Rect::new(50.0, 40.0, 70.0, 40.0));
            let mut p = Paint::solid(Color::WHITE);
            p.opacity = 0.0;
            n.fills = vec![p];
            n
        });
    }
    t
}

#[test]
fn paint_free_and_zero_fill_children_keep_shadow() {
    let Some(mut g) = common::gpu() else { return };
    let actual = case_a_doc(true);
    let expected = case_a_doc(false);
    let vp = Viewport::default();
    let size = (160, 120);
    let c = compare_doc(&mut g, "knockout-paint-free-children", &actual.doc, actual.page, vp, size, 8);
    maybe_save_crop("case-a", &c.cpu, &c.gpu);
    let exp_c = compare_doc(&mut g, "knockout-paint-free-children-expected", &expected.doc, expected.page, vp, size, 8);
    assert!(c.report.ratio < 0.02, "CPU/GPU mismatch {:.3}%", c.report.ratio * 100.0);
    assert!(px(&c.cpu, 78, 40)[0] < 220, "CPU knocked out shadow under paint-free child frame: {:?}", px(&c.cpu, 78, 40));
    assert!(px(&c.cpu, 78, 80)[0] < 220, "CPU knocked out shadow under 0%-fill child frame: {:?}", px(&c.cpu, 78, 80));
    assert!(px(&c.gpu, 78, 40)[0] < 220, "GPU knocked out shadow under paint-free child frame: {:?}", px(&c.gpu, 78, 40));
    assert!(px(&c.gpu, 78, 80)[0] < 220, "GPU knocked out shadow under 0%-fill child frame: {:?}", px(&c.gpu, 78, 80));
    let opts = DiffOptions { tolerance: 0, detect_aa: false, ignore: vec![] };
    let (cpu_vs_exp, _) = diff_image(&common::to_img(&c.cpu), &common::to_img(&exp_c.cpu), &opts);
    let (gpu_vs_exp, _) = diff_image(&common::to_img(&c.gpu), &common::to_img(&exp_c.gpu), &opts);
    assert_eq!(cpu_vs_exp.mismatched, 0, "CPU differed from baseline by {} px", cpu_vs_exp.mismatched);
    assert_eq!(gpu_vs_exp.mismatched, 0, "GPU differed from baseline by {} px", gpu_vs_exp.mismatched);
}

fn case_b_doc(nested_in_fillless_child_frame: bool) -> common::T {
    let mut t = common::T::new();
    let f = t.add(None, |id| {
        let mut n = Node::frame(id, Rect::new(20.0, 20.0, 120.0, 80.0));
        n.fills.clear();
        n.effects = vec![right_shadow()];
        n
    });
    t.rect(Some(f), Rect::new(0.0, 0.0, 40.0, 80.0), BLUE_A);
    if nested_in_fillless_child_frame {
        let child_frame = t.add(Some(f), |id| {
            let mut n = Node::frame(id, Rect::new(40.0, 0.0, 80.0, 80.0));
            n.fills = vec![Paint::solid(Color::TRANSPARENT)];
            n
        });
        t.add(Some(child_frame), |id| {
            let mut n = Node::frame(id, Rect::new(0.0, 0.0, 40.0, 80.0));
            n.fills.clear();
            n
        });
        t.rect(Some(child_frame), Rect::new(40.0, 10.0, 40.0, 60.0), RED_B);
    } else {
        t.rect(Some(f), Rect::new(80.0, 10.0, 40.0, 60.0), RED_B);
    }
    t
}

#[test]
fn fillless_child_frame_with_painting_grandchild_knocks_out_only_grandchild() {
    let Some(mut g) = common::gpu() else { return };
    let actual = case_b_doc(true);
    let expected = case_b_doc(false);
    let vp = Viewport::default();
    let size = (160, 120);
    let c = compare_doc(&mut g, "knockout-grandchild", &actual.doc, actual.page, vp, size, 8);
    let exp_c = compare_doc(&mut g, "knockout-grandchild-expected", &expected.doc, expected.page, vp, size, 8);
    assert!(c.report.ratio < 0.02, "CPU/GPU mismatch {:.3}%", c.report.ratio * 100.0);
    assert!(px(&c.cpu, 68, 60)[0] < 220, "CPU knocked out shadow outside grandchild: {:?}", px(&c.cpu, 68, 60));
    assert!(px(&c.gpu, 68, 60)[0] < 220, "GPU knocked out shadow outside grandchild: {:?}", px(&c.gpu, 68, 60));
    assert!(px(&c.cpu, 115, 60)[1] >= 120, "CPU did not knock out grandchild shadow: {:?}", px(&c.cpu, 115, 60));
    assert!(px(&c.gpu, 115, 60)[1] >= 120, "GPU did not knock out grandchild shadow: {:?}", px(&c.gpu, 115, 60));
    let opts = DiffOptions { tolerance: 0, detect_aa: false, ignore: vec![] };
    let (cpu_vs_exp, _) = diff_image(&common::to_img(&c.cpu), &common::to_img(&exp_c.cpu), &opts);
    let (gpu_vs_exp, _) = diff_image(&common::to_img(&c.gpu), &common::to_img(&exp_c.gpu), &opts);
    assert_eq!(cpu_vs_exp.mismatched, 0, "CPU differed from baseline by {} px", cpu_vs_exp.mismatched);
    assert_eq!(gpu_vs_exp.mismatched, 0, "GPU differed from baseline by {} px", gpu_vs_exp.mismatched);
}

fn case_c_doc(use_mask: bool) -> common::T {
    let mut t = common::T::new();
    let f = t.add(None, |id| {
        let mut n = Node::frame(id, Rect::new(20.0, 20.0, 100.0, 100.0));
        n.fills.clear();
        n.effects = vec![Effect::DropShadow {
            color: Color { r: 0.0, g: 0.0, b: 0.0, a: 0.75 },
            offset: Vec2::ZERO,
            radius: 14.0,
            spread: 6.0,
            visible: true,
            show_behind: false,
            blend: Default::default(),
        }];
        n
    });
    if use_mask {
        t.add(Some(f), |id| {
            let mut m = Node::ellipse(id, Rect::new(10.0, 10.0, 80.0, 80.0));
            m.is_mask = true;
            m.fills = vec![Paint::solid(Color::WHITE)];
            m
        });
        t.rect(Some(f), Rect::new(10.0, 10.0, 80.0, 80.0), RED_B);
    } else {
        t.add(Some(f), |id| {
            let mut e = Node::ellipse(id, Rect::new(10.0, 10.0, 80.0, 80.0));
            e.fills = vec![Paint::solid(RED_B)];
            e
        });
    }
    t
}

#[test]
fn ellipse_mask_plus_rect_sibling_knocks_out_circle_not_rect() {
    let Some(mut g) = common::gpu() else { return };
    let actual = case_c_doc(true);
    let expected = case_c_doc(false);
    let vp = Viewport::default();
    let size = (140, 140);
    let c = compare_doc(&mut g, "knockout-ellipse-mask", &actual.doc, actual.page, vp, size, 8);
    maybe_save_crop("case-c", &c.cpu, &c.gpu);
    let exp_c = compare_doc(&mut g, "knockout-ellipse-mask-expected", &expected.doc, expected.page, vp, size, 8);
    assert!(c.report.ratio < 0.02, "CPU/GPU mismatch {:.3}%", c.report.ratio * 100.0);
    assert!(px(&c.cpu, 40, 40)[0] < 230, "CPU unmasked rect corner knocked out circular shadow: {:?}", px(&c.cpu, 40, 40));
    assert!(px(&c.gpu, 40, 40)[0] < 230, "GPU unmasked rect corner knocked out circular shadow: {:?}", px(&c.gpu, 40, 40));
    assert!(px(&c.cpu, 70, 70)[1] >= 120, "CPU circle interior did not knock out shadow: {:?}", px(&c.cpu, 70, 70));
    assert!(px(&c.gpu, 70, 70)[1] >= 120, "GPU circle interior did not knock out shadow: {:?}", px(&c.gpu, 70, 70));
    let opts = DiffOptions { tolerance: 2, detect_aa: false, ignore: vec![] };
    let (cpu_vs_exp, _) = diff_image(&common::to_img(&c.cpu), &common::to_img(&exp_c.cpu), &opts);
    let (gpu_vs_exp, _) = diff_image(&common::to_img(&c.gpu), &common::to_img(&exp_c.gpu), &opts);
    assert!(cpu_vs_exp.ratio < 0.005, "CPU masked rect vs circle differed by {:.4}%", cpu_vs_exp.ratio * 100.0);
    assert!(gpu_vs_exp.ratio < 0.005, "GPU masked rect vs circle differed by {:.4}%", gpu_vs_exp.ratio * 100.0);
}

fn case_d_doc(wrapped_in_group: bool) -> common::T {
    let mut t = common::T::new();
    let f = t.add(None, |id| {
        let mut n = Node::frame(id, Rect::new(20.0, 20.0, 120.0, 80.0));
        n.fills.clear();
        n.effects = vec![Effect::DropShadow {
            color: Color { r: 0.0, g: 0.0, b: 0.0, a: 0.75 },
            offset: Vec2::new(0.0, 8.0),
            radius: 12.0,
            spread: 0.0,
            visible: true,
            show_behind: false,
            blend: Default::default(),
        }];
        n
    });
    if wrapped_in_group {
        let grp = t.add(Some(f), |id| Node::new(id, NodeKind::Group));
        t.rect(Some(grp), Rect::new(10.0, 10.0, 100.0, 60.0), RED_B);
    } else {
        t.rect(Some(f), Rect::new(10.0, 10.0, 100.0, 60.0), RED_B);
    }
    t
}

#[test]
fn group_wrapped_50pct_child_removes_same_shadow_as_direct_child() {
    let Some(mut g) = common::gpu() else { return };
    let actual = case_d_doc(true);
    let expected = case_d_doc(false);
    let vp = Viewport::default();
    let size = (160, 120);
    let c = compare_doc(&mut g, "knockout-group-wrapped", &actual.doc, actual.page, vp, size, 8);
    maybe_save_crop("case-d", &c.cpu, &c.gpu);
    let exp_c = compare_doc(&mut g, "knockout-group-wrapped-expected", &expected.doc, expected.page, vp, size, 8);
    assert!(c.report.ratio < 0.02, "CPU/GPU mismatch {:.3}%", c.report.ratio * 100.0);
    assert!(px(&c.cpu, 80, 60)[1] >= 120, "CPU group-wrapped 50% child left shadow under itself: {:?}", px(&c.cpu, 80, 60));
    assert!(px(&c.gpu, 80, 60)[1] >= 120, "GPU group-wrapped 50% child left shadow under itself: {:?}", px(&c.gpu, 80, 60));
    let opts = DiffOptions { tolerance: 0, detect_aa: false, ignore: vec![] };
    let (cpu_vs_exp, _) = diff_image(&common::to_img(&c.cpu), &common::to_img(&exp_c.cpu), &opts);
    let (gpu_vs_exp, _) = diff_image(&common::to_img(&c.gpu), &common::to_img(&exp_c.gpu), &opts);
    assert_eq!(cpu_vs_exp.mismatched, 0, "CPU group wrapper changed {} px", cpu_vs_exp.mismatched);
    assert_eq!(gpu_vs_exp.mismatched, 0, "GPU group wrapper changed {} px", gpu_vs_exp.mismatched);
}

#[test]
fn gpu_skips_runtime_opacity_zero_child_like_cpu() {
    let Some(mut g) = common::gpu() else { return };
    let mut t = common::T::new();
    let f = t.add(None, |id| {
        let mut n = Node::frame(id, Rect::new(20.0, 20.0, 120.0, 80.0));
        n.fills.clear();
        n.effects = vec![right_shadow()];
        n
    });
    t.rect(Some(f), Rect::new(0.0, 0.0, 50.0, 80.0), BLUE_A);
    let c2 = t.rect(Some(f), Rect::new(50.0, 0.0, 70.0, 80.0), Color::WHITE);

    let vp = Viewport::default();
    let size = (160, 120);
    let mut cache = TileCache::new();
    cache.set_runtime_opacities(&t.doc, t.page, vec![(c2, 0.0)]);
    let tf = cache.render_tiles(&t.doc, t.page, &vp, size, 1, TileBudget::unlimited());
    let mut cpu = Pixmap::new(size.0, size.1).unwrap();
    for tile in &tf.tiles {
        let (x, y) = tile.key.device_origin();
        cpu.draw_pixmap(x, y, tile.pixmap.as_ref().as_ref(), &Default::default(), Default::default(), None);
    }

    g.compiler.opts.runtime_node_opacities = vec![(c2, 0.0)];
    let gpu = g.render_page(&t.doc, t.page, &vp, size);
    g.compiler.opts.runtime_node_opacities.clear();

    assert!(px(&cpu, 78, 60)[0] < 220, "CPU runtime-opacity-0 child knocked out shadow: {:?}", px(&cpu, 78, 60));
    assert!(px(&gpu, 78, 60)[0] < 220, "GPU runtime-opacity-0 child knocked out shadow: {:?}", px(&gpu, 78, 60));
    let opts = DiffOptions { tolerance: 8, detect_aa: false, ignore: vec![] };
    let (report, _) = diff_image(&common::to_img(&cpu), &common::to_img(&gpu), &opts);
    assert!(report.ratio < 0.02, "CPU vs GPU runtime-opacity-0 mismatch {:.3}% ({} px)", report.ratio * 100.0, report.mismatched);
}

#[test]
fn null_control_filled_and_plain_fillless_frames_unchanged() {
    let Some(mut g) = common::gpu() else { return };
    for (name, show_behind) in [("null-ctrl-no-behind", false), ("null-ctrl-behind", true)] {
        let (doc, page) = pair(show_behind);
        let vp = Viewport::default();
        let c = common::compare(&mut g, name, &doc, page, vp, SIZE, 8);
        assert!(c.report.ratio < 0.02, "{name} strict mismatch {:.3}%", c.report.ratio * 100.0);
    }
    let _ = render_page;
}

// ---------------------------------------------------------------------------
// Paint-free leaves, text children and luminance masks
// ---------------------------------------------------------------------------

/// Renders `t` on both backends and returns the pair `(cpu, gpu)`.
fn render_both(g: &mut designos_gpu::HeadlessGpu, name: &str, t: &common::T, size: (u32, u32)) -> (Pixmap, Pixmap) {
    let c = compare_doc(g, name, &t.doc, t.page, Viewport::default(), size, 8);
    assert!(c.report.ratio < 0.02, "{name}: CPU/GPU mismatch {:.3}%", c.report.ratio * 100.0);
    (c.cpu, c.gpu)
}

/// Pixels that differ at all.
fn differing_pixels(a: &Pixmap, b: &Pixmap) -> u64 {
    let opts = DiffOptions { tolerance: 0, detect_aa: false, ignore: vec![] };
    diff_image(&common::to_img(a), &common::to_img(b), &opts).0.mismatched
}

/// Both backends must leave `with` and `without` identical; one report names both counts.
fn assert_same_on_both(what: &str, (cpu, gpu): (&Pixmap, &Pixmap), (cpu0, gpu0): (&Pixmap, &Pixmap)) {
    let (c, g) = (differing_pixels(cpu, cpu0), differing_pixels(gpu, gpu0));
    assert!(c == 0 && g == 0, "{what}: {c} CPU and {g} GPU pixels differ");
}

fn close(a: [u8; 4], b: [u8; 4], tol: u8) -> bool {
    a.iter().zip(b).all(|(x, y)| x.abs_diff(y) <= tol)
}

const WHITE_PX: [u8; 4] = [255, 255, 255, 255];

fn zero_percent_paint() -> Paint {
    let mut p = Paint::solid(Color::WHITE);
    p.opacity = 0.0;
    p
}

fn offset_drop_shadow(radius: f32) -> Effect {
    Effect::DropShadow {
        color: Color { r: 0.0, g: 0.0, b: 0.0, a: 0.65 },
        offset: Vec2::new(16.0, 16.0),
        radius,
        spread: 0.0,
        visible: true,
        show_behind: false,
        blend: Default::default(),
    }
}

fn inner_shadow(radius: f32) -> Effect {
    Effect::InnerShadow {
        color: Color { r: 0.0, g: 0.0, b: 0.0, a: 0.65 },
        offset: Vec2::new(6.0, 6.0),
        radius,
        spread: 0.0,
        visible: true,
        blend: Default::default(),
    }
}

/// A frame whose only fill is visible but 0% opaque, holding one child painted `child`.
fn zero_fill_frame(effects: Vec<Effect>, child: Color) -> common::T {
    let mut t = common::T::new();
    let f = t.add(None, |id| {
        let mut n = Node::frame(id, Rect::new(30.0, 30.0, 100.0, 80.0));
        n.fills = vec![zero_percent_paint()];
        n.effects = effects;
        n
    });
    t.rect(Some(f), Rect::new(10.0, 10.0, 40.0, 40.0), child);
    t
}

#[test]
fn zero_fill_frame_with_children_keeps_an_empty_drop_shadow() {
    let Some(mut g) = common::gpu() else { return };
    let size = (160, 140);
    for (i, radius) in [0.0, 8.0].into_iter().enumerate() {
        let (cpu, gpu) = render_both(&mut g, &format!("zero-fill-drop-{i}"), &zero_fill_frame(vec![offset_drop_shadow(radius)], BLUE_A), size);
        let (cpu0, gpu0) = render_both(&mut g, &format!("zero-fill-drop-bare-{i}"), &zero_fill_frame(vec![], BLUE_A), size);
        assert_same_on_both(&format!("radius {radius}: a 0% fill frame cast a drop shadow"), (&cpu, &gpu), (&cpu0, &gpu0));
    }
}

#[test]
fn zero_fill_frame_with_children_draws_no_inner_shadow() {
    let Some(mut g) = common::gpu() else { return };
    let size = (160, 140);
    for (i, radius) in [0.0, 4.0].into_iter().enumerate() {
        // a 50% child, so an inner shadow drawn under it would show through
        let (cpu, gpu) = render_both(&mut g, &format!("zero-fill-inner-{i}"), &zero_fill_frame(vec![inner_shadow(radius)], RED_B), size);
        let (cpu0, gpu0) = render_both(&mut g, &format!("zero-fill-inner-bare-{i}"), &zero_fill_frame(vec![], RED_B), size);
        assert_same_on_both(&format!("radius {radius}: a 0% fill frame drew an inner shadow"), (&cpu, &gpu), (&cpu0, &gpu0));
    }
}

#[test]
fn zero_percent_stroke_frame_keeps_its_stroke_footprint_shadow() {
    let Some(mut g) = common::gpu() else { return };
    let build = |effects: Vec<Effect>| {
        let mut t = common::T::new();
        t.add(None, |id| {
            let mut n = Node::frame(id, Rect::new(30.0, 30.0, 100.0, 60.0));
            n.fills.clear();
            n.stroke.weight = 4.0;
            n.stroke.paints = vec![zero_percent_paint()];
            n.effects = effects;
            n
        });
        t
    };
    let (cpu, gpu) = render_both(&mut g, "zero-stroke-shadow", &build(vec![offset_drop_shadow(8.0)]), (160, 140));
    // the ring's shadow, shifted by 16, shows below the frame's bottom edge (y 90 -> 106)
    let (c, g) = (px(&cpu, 80, 106), px(&gpu, 80, 106));
    assert!(c[0] < 235 && g[0] < 235, "the stroke footprint cast no shadow: CPU {c:?}, GPU {g:?}");
}

const TEXT_CANVAS: (u32, u32) = (240, 230);

/// A fill-less frame (20, 20, 200 x 150): a blue band across the top whose shadow falls 70px
/// below it, and a text child standing in that shadow.
fn text_scene(text_fills: Vec<Paint>, text_stroke: bool, with_text: bool, with_shadow: bool) -> common::T {
    let mut t = common::T::new();
    let f = t.add(None, |id| {
        let mut n = Node::frame(id, Rect::new(20.0, 20.0, 200.0, 150.0));
        n.fills.clear();
        if with_shadow {
            n.effects = vec![Effect::DropShadow {
                color: Color { r: 0.0, g: 0.0, b: 0.0, a: 0.65 },
                offset: Vec2::new(0.0, 70.0),
                radius: 6.0,
                spread: 0.0,
                visible: true,
                show_behind: false,
                blend: Default::default(),
            }];
        }
        n
    });
    t.rect(Some(f), Rect::new(0.0, 0.0, 200.0, 60.0), BLUE_A);
    if with_text {
        t.add(Some(f), |id| {
            let style = TextStyle { font_size: 48.0, font_weight: 700, fills: text_fills, ..Default::default() };
            let mut n = Node::text(id, Rect::new(10.0, 72.0, 180.0, 60.0), "HHH", style);
            if text_stroke {
                n.stroke.weight = 1.0;
                n.stroke.paints = vec![Paint::solid(Color::BLACK)];
            }
            n
        });
    }
    t
}

fn black_at(opacity: f32) -> Paint {
    let mut p = Paint::solid(Color::BLACK);
    p.opacity = opacity;
    p
}

/// Pixel positions fully covered by a 50% black glyph: its interior, away from the rim.
fn glyph_interior(pm: &Pixmap) -> Vec<(u32, u32)> {
    let covered = |x: u32, y: u32| {
        let p = px(pm, x, y);
        y >= 90 && p[0].abs_diff(p[1]) <= 1 && p[1].abs_diff(p[2]) <= 1 && p[0] <= 129 && p[3] == 255
    };
    let mut out = Vec::new();
    for y in 1..pm.height() - 1 {
        for x in 1..pm.width() - 1 {
            if covered(x, y) && (-1i32..=1).all(|dy| (-1i32..=1).all(|dx| covered((x as i32 + dx) as u32, (y as i32 + dy) as u32))) {
                out.push((x, y));
            }
        }
    }
    out
}

#[test]
fn paint_free_text_children_add_nothing_to_the_knockout() {
    let Some(mut g) = common::gpu() else { return };
    let mut hidden = black_at(1.0);
    hidden.visible = false;
    let (cpu0, gpu0) = render_both(&mut g, "paint-free-text-bare", &text_scene(vec![], false, false, true), TEXT_CANVAS);
    for (i, (what, fills)) in [("0% fill", vec![black_at(0.0)]), ("hidden fill", vec![hidden]), ("no fill", vec![])].into_iter().enumerate() {
        let (cpu, gpu) = render_both(&mut g, &format!("paint-free-text-{i}"), &text_scene(fills, false, true, true), TEXT_CANVAS);
        assert_same_on_both(&format!("{what}: text that paints nothing changed the shadow"), (&cpu, &gpu), (&cpu0, &gpu0));
    }
}

#[test]
fn half_opaque_text_knocks_out_the_shadow_at_full_coverage() {
    let Some(mut g) = common::gpu() else { return };
    let (cpu, gpu) = render_both(&mut g, "half-text-shadow", &text_scene(vec![black_at(0.5)], false, true, true), TEXT_CANVAS);
    let (cpu0, gpu0) = render_both(&mut g, "half-text-bare", &text_scene(vec![black_at(0.5)], false, true, false), TEXT_CANVAS);
    for (what, shadowed, bare) in [("CPU", &cpu, &cpu0), ("GPU", &gpu, &gpu0)] {
        let interior = glyph_interior(bare);
        assert!(interior.len() > 100, "{what}: the glyphs must have interior pixels, found {}", interior.len());
        for (x, y) in &interior {
            assert!(
                close(px(shadowed, *x, *y), px(bare, *x, *y), 2),
                "{what} ({x}, {y}): shadow left under a 50% glyph: {:?} vs {:?}",
                px(shadowed, *x, *y),
                px(bare, *x, *y)
            );
        }
        assert!(px(shadowed, 24, 120)[0] < 235, "{what}: the fixture must cast a shadow: {:?}", px(shadowed, 24, 120));
    }
}

#[test]
fn stroke_only_text_still_knocks_out_its_glyph_interior() {
    let Some(mut g) = common::gpu() else { return };
    let (_, gpu_bare) = render_both(&mut g, "stroke-text-bare", &text_scene(vec![black_at(0.5)], false, true, false), TEXT_CANVAS);
    let (cpu, gpu) = render_both(&mut g, "stroke-text-shadow", &text_scene(vec![], true, true, true), TEXT_CANVAS);
    let interior = glyph_interior(&gpu_bare);
    assert!(interior.len() > 100);
    for (what, pm) in [("CPU", &cpu), ("GPU", &gpu)] {
        for (x, y) in &interior {
            assert!(close(px(pm, *x, *y), WHITE_PX, 2), "{what} ({x}, {y}): {:?}", px(pm, *x, *y));
        }
        assert!(px(pm, 24, 120)[0] < 235, "{what}: the fixture must cast a shadow: {:?}", px(pm, 24, 120));
    }
}

/// The ellipse mask of the mask case, but a mid-gray luminance mask (alpha 1, luminance 0.5).
fn luminance_scene(show_behind: Option<bool>) -> common::T {
    let mut t = common::T::new();
    let f = t.add(None, |id| {
        let mut n = Node::frame(id, Rect::new(20.0, 20.0, 100.0, 100.0));
        n.fills.clear();
        if let Some(show_behind) = show_behind {
            n.effects = vec![Effect::DropShadow {
                color: Color { r: 0.0, g: 0.0, b: 0.0, a: 0.75 },
                offset: Vec2::ZERO,
                radius: 14.0,
                spread: 6.0,
                visible: true,
                show_behind,
                blend: Default::default(),
            }];
        }
        n
    });
    t.add(Some(f), |id| {
        let mut m = Node::ellipse(id, Rect::new(10.0, 10.0, 80.0, 80.0));
        m.is_mask = true;
        m.mask_type = MaskType::Luminance;
        m.fills = vec![Paint::solid(Color { r: 0.5, g: 0.5, b: 0.5, a: 1.0 })];
        m
    });
    t.rect(Some(f), Rect::new(10.0, 10.0, 80.0, 80.0), RED_B);
    t
}

#[test]
fn luminance_mask_knocks_out_by_its_luminance() {
    let Some(mut g) = common::gpu() else { return };
    let size = (140, 140);
    let (cpu_k, gpu_k) = render_both(&mut g, "luminance-knockout", &luminance_scene(Some(false)), size);
    let (cpu_b, gpu_b) = render_both(&mut g, "luminance-show-behind", &luminance_scene(Some(true)), size);
    let (cpu_n, gpu_n) = render_both(&mut g, "luminance-no-shadow", &luminance_scene(None), size);
    for (what, k, b, n) in [("CPU", &cpu_k, &cpu_b, &cpu_n), ("GPU", &gpu_k, &gpu_b, &gpu_n)] {
        // at the centre, green rises with the share of shadow knocked out: bare = all, behind = nothing
        let (gk, gb, gn) = (px(k, 70, 70)[1] as f32, px(b, 70, 70)[1] as f32, px(n, 70, 70)[1] as f32);
        assert!(gn - gb >= 8.0, "{what}: the fixture must cast a visible shadow at the centre: shadowed {gb}, bare {gn}");
        let share = (gk - gb) / (gn - gb);
        assert!((0.3..=0.7).contains(&share), "{what}: a mid-gray luminance mask must knock out about half the shadow, got {share:.2} ({gb} {gk} {gn})");
        // outside the mask the rect's corner removes nothing
        assert!(close(px(k, 40, 40), px(b, 40, 40), 2), "{what}: {:?} vs {:?}", px(k, 40, 40), px(b, 40, 40));
    }
}
