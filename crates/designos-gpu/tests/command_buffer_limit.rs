//! The Metal backend refuses to start the 4,097th command buffer of a queue
//! that has not been submitted yet, and the refusal is a lost device (#467).
//! wgpu-core records two command buffers per render pass, so one submission
//! holds at most 2,047 passes. A page with a few hundred blurred layers is past
//! that: the idle refine used to lose the device on it, and egui-wgpu then
//! panicked with "Failed to create staging buffer for index data".
//!
//! These tests need a hardware GPU and skip without one.

mod common;

use designos_doc::*;
use designos_gpu::{HeadlessGpu, HAL_BUFFERS_PER_PASS, MAX_HAL_COMMAND_BUFFERS, SUBMIT_HAL_BUDGET};
use designos_render::Viewport;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

const SIZE: (u32, u32) = (1000, 1000);
const COLUMNS: usize = 50;
const PITCH: f32 = 20.0;
const SIDE: f32 = 12.0;
const BLUE: [u8; 4] = [0x33, 0x66, 0xCC, 0xFF];

/// `count` layer-blurred squares on a 20 px grid inside a 1000 x 1000 view. A
/// blurred layer costs 4 render passes (the layer, two blur passes, the
/// composite), so `count` layers need about `4 * count` passes.
fn blurred_grid(count: usize) -> (Document, NodeId) {
    let mut doc = Document::new("command buffer limit");
    let page = doc.add_page("Page");
    for i in 0..count {
        let id = doc.alloc_id();
        let (col, row) = ((i % COLUMNS) as f32, (i / COLUMNS) as f32);
        let mut n = Node::rectangle(id, Rect::new(col * PITCH + 4.0, row * PITCH + 4.0, SIDE, SIDE));
        n.fills = vec![Paint::solid(Color::rgb(BLUE[0], BLUE[1], BLUE[2]))];
        n.effects = vec![Effect::LayerBlur { radius: 1.0, visible: true }];
        doc.insert(page, n).unwrap();
    }
    (doc, page)
}

/// A GPU whose device-lost state is observable.
fn watched_gpu() -> Option<(HeadlessGpu, Arc<AtomicBool>)> {
    let g = common::gpu()?;
    let lost = Arc::new(AtomicBool::new(false));
    let flag = lost.clone();
    g.ctx.device.set_device_lost_callback(move |reason, message| {
        eprintln!("device lost: {reason:?}: {message}");
        flag.store(true, Ordering::SeqCst);
    });
    Some((g, lost))
}

/// The compile the idle view runs: interactive, program cap lifted.
fn refine_frame(g: &mut HeadlessGpu, doc: &Document, page: NodeId) -> designos_gpu::Frame {
    g.compiler.geom.budget = Some(Duration::from_millis(8));
    let vp = Viewport { offset: Vec2::new(0.0, 0.0), zoom: 1.0, dpr: 1.0 };
    for _ in 0..100 {
        g.compiler.compile(doc, page, 1, &vp, SIZE);
        if g.compiler.geom.stale_hits == 0 {
            break;
        }
    }
    g.compiler.opts.uncap_programs = true;
    g.compiler.compile(doc, page, 1, &vp, SIZE)
}

/// Pixel `(x, y)` of a premultiplied RGBA readback.
fn pixel(rgba: &[u8], x: u32, y: u32) -> [u8; 4] {
    let i = ((y * SIZE.0 + x) * 4) as usize;
    [rgba[i], rgba[i + 1], rgba[i + 2], rgba[i + 3]]
}

fn near(a: [u8; 4], b: [u8; 4]) -> bool {
    a.iter().zip(b).all(|(x, y)| x.abs_diff(y) <= 3)
}

/// Every square must show its colour at the centre and nothing between squares.
fn assert_all_squares_drawn(rgba: &[u8], count: usize) {
    for i in 0..count {
        let (col, row) = ((i % COLUMNS) as f32, (i / COLUMNS) as f32);
        let (cx, cy) = ((col * PITCH + 4.0 + SIDE / 2.0) as u32, (row * PITCH + 4.0 + SIDE / 2.0) as u32);
        assert!(near(pixel(rgba, cx, cy), BLUE), "square {i} at ({cx}, {cy}) is {:?}, not blue", pixel(rgba, cx, cy));
    }
}

#[test]
fn a_frame_past_the_command_buffer_limit_renders_without_device_loss() {
    let Some((mut g, lost)) = watched_gpu() else { return };
    // 600 layers: about 2,400 passes, twice what one submission can hold
    let count = 600;
    let (doc, page) = blurred_grid(count);
    let frame = refine_frame(&mut g, &doc, page);
    assert!(frame.programs.len() > count, "one program per layer, got {}", frame.programs.len());
    let rgba = g.renderer.render_to_rgba(&frame);
    assert!(!lost.load(Ordering::SeqCst), "the device was lost while rendering {} programs", frame.programs.len());
    let stats = g.renderer.stats;
    eprintln!(
        "{count} blurred layers: {} programs, {} render passes in {} submissions, largest ~{} command buffers",
        frame.programs.len(),
        stats.passes,
        stats.submissions,
        stats.hal_buffers_max
    );
    assert!(stats.passes > 2047, "the page must need more passes than one submission holds, got {}", stats.passes);
    assert!(stats.submissions >= 2, "the frame must be split over several submissions, got {}", stats.submissions);
    assert!(stats.hal_buffers_max <= SUBMIT_HAL_BUDGET, "a submission holds {} command buffers, budget {SUBMIT_HAL_BUDGET}", stats.hal_buffers_max);
    assert_all_squares_drawn(&rgba, count);
}

#[test]
fn splitting_a_frame_into_submissions_changes_no_pixel() {
    let Some((mut g, lost)) = watched_gpu() else { return };
    // fits one submission, so it can be rendered both ways
    let count = 500;
    let (doc, page) = blurred_grid(count);
    let frame = refine_frame(&mut g, &doc, page);
    g.renderer.submit_budget = u32::MAX;
    let whole = g.renderer.render_to_rgba(&frame);
    assert_eq!(g.renderer.stats.submissions, 1, "the frame and the readback copy share one submission");
    g.renderer.submit_budget = 64;
    let split = g.renderer.render_to_rgba(&frame);
    let submissions = g.renderer.stats.submissions;
    assert!(submissions > 20, "a 64 command buffer budget must cut the frame into many submissions, got {submissions}");
    assert!(!lost.load(Ordering::SeqCst), "the device was lost");
    assert!(whole == split, "the {submissions}-submission render differs from the one-submission render");
    assert_all_squares_drawn(&split, count);
}

/// The limit itself, measured with bare wgpu: `MAX_COMMAND_BUFFERS` of
/// wgpu-hal's Metal backend is 4,096 unsubmitted command buffers per queue, and
/// every render pass takes two. When a wgpu upgrade moves these numbers,
/// `HAL_BUFFERS_PER_PASS` and `SUBMIT_HAL_BUDGET` need another look.
#[test]
fn metal_holds_2047_render_passes_in_one_submission_and_loses_the_device_at_2048() {
    let Some(g) = common::gpu() else { return };
    if g.ctx.adapter_info.backend != designos_gpu::wgpu::Backend::Metal {
        eprintln!("SKIP: the command buffer limit is a Metal backend limit, this is {:?}", g.ctx.adapter_info.backend);
        return;
    }
    drop(g);
    assert_eq!(MAX_HAL_COMMAND_BUFFERS, 4096);
    assert_eq!(HAL_BUFFERS_PER_PASS, 2);
    assert!(render_passes_survive(2047), "2,047 passes in one submission must survive");
    assert!(!render_passes_survive(2048), "2,048 passes in one submission must lose the device");
}

/// Records `passes` render passes into one encoder of a fresh device, submits it
/// and reports whether the device is still alive.
fn render_passes_survive(passes: u32) -> bool {
    use designos_gpu::wgpu;
    let ctx = designos_gpu::GpuContext::new().expect("gpu");
    let lost = Arc::new(AtomicBool::new(false));
    let flag = lost.clone();
    ctx.device.set_device_lost_callback(move |_, _| flag.store(true, Ordering::SeqCst));
    let texture = ctx.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("limit probe"),
        size: wgpu::Extent3d { width: 8, height: 8, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let view = texture.create_view(&Default::default());
    let mut enc = ctx.device.create_command_encoder(&Default::default());
    for _ in 0..passes {
        let _pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
    }
    ctx.queue.submit([enc.finish()]);
    // bounded: a device that was lost must not hang the test
    let _ = ctx.device.poll(wgpu::PollType::Wait { submission_index: None, timeout: Some(Duration::from_secs(10)) });
    std::thread::sleep(Duration::from_millis(50));
    let alive = !lost.load(Ordering::SeqCst);
    eprintln!("{passes} render passes in one submission: device {}", if alive { "alive" } else { "lost" });
    alive
}

#[test]
fn a_frame_just_under_the_command_buffer_limit_renders_without_device_loss() {
    let Some((mut g, lost)) = watched_gpu() else { return };
    // about 2,000 passes: the largest frame one submission can hold
    let count = 500;
    let (doc, page) = blurred_grid(count);
    let frame = refine_frame(&mut g, &doc, page);
    let mut enc = g.ctx.device.create_command_encoder(&Default::default());
    g.renderer.render(&frame, &mut enc);
    let passes = g.renderer.stats.passes;
    eprintln!("{count} blurred layers: {} programs, {passes} render passes in one submission", frame.programs.len());
    g.ctx.queue.submit([enc.finish()]);
    g.renderer.wait_idle();
    assert!(passes > 1900 && passes <= 2047, "the frame must sit just under 2,047 passes, got {passes}");
    assert!(!lost.load(Ordering::SeqCst), "the device was lost at {passes} passes in one submission");
}
