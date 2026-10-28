//! GPU vs CPU tile cache on the `bench_big` page (50 frames x 100 children,
//! 5,000+ nodes) at 1505x1125: pan, zoom gesture, drag and Smart Animate frames.
//!
//! Run with `cargo run --release -p designos-gpu --example bench_gpu`.
//! "compile" is the CPU time to build the GPU frame, "frame" the whole frame
//! (compile + encode + submit + wait for the GPU to finish).

use designos_doc::*;
use designos_gpu::HeadlessGpu;
use designos_render::tiny_skia::Pixmap;
use designos_render::{render_node, TileBudget, TileCache, Viewport};
use std::time::{Duration, Instant};

const W: u32 = 1505;
const H: u32 = 1125;

fn checker() -> Vec<u8> {
    let mut pm = Pixmap::new(32, 32).unwrap();
    for y in 0..32u32 {
        for x in 0..32u32 {
            let on = ((x / 8) + (y / 8)) % 2 == 0;
            let c: [u8; 4] = if on { [0xF2, 0x4E, 0x1E, 255] } else { [0x0A, 0xCF, 0x83, 255] };
            let i = ((y * 32 + x) * 4) as usize;
            pm.data_mut()[i..i + 4].copy_from_slice(&c);
        }
    }
    pm.encode_png().unwrap()
}

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((self.0 >> 33) as f32) / (1u64 << 31) as f32
    }
}

fn star(w: f32, h: f32) -> Path {
    let mut cmds = Vec::new();
    for i in 0..10 {
        let a = i as f32 * std::f32::consts::PI / 5.0 - std::f32::consts::FRAC_PI_2;
        let r = if i % 2 == 0 { 0.5 } else { 0.22 };
        let p = Vec2::new(w * (0.5 + r * a.cos()), h * (0.5 + r * a.sin()));
        cmds.push(if i == 0 { PathCmd::MoveTo(p) } else { PathCmd::LineTo(p) });
    }
    cmds.push(PathCmd::Close);
    Path { cmds, fill_rule: FillRule::NonZero }
}

/// Same page as `designos-render/examples/bench_big.rs`.
fn big_doc() -> (Document, NodeId, Vec<NodeId>) {
    let mut doc = Document::new("big");
    let page = doc.add_page("Big");
    doc.get_mut(page).unwrap().kind = NodeKind::Page { background: Color::rgb(0xE5, 0xE5, 0xE5) };
    doc.images.insert("checker".into(), checker());
    let mut rng = Rng(7);
    let mut leaves = Vec::new();
    for f in 0..50 {
        let (col, row) = (f % 10, f / 10);
        let fid = doc.alloc_id();
        let mut frame = Node::frame(fid, Rect::new(col as f32 * 900.0, row as f32 * 700.0, 800.0, 600.0));
        frame.kind.frame_props_mut().unwrap().clips_content = true;
        frame.name = format!("Frame {f}");
        doc.insert(page, frame).unwrap();
        for i in 0..100 {
            let id = doc.alloc_id();
            let (x, y) = (rng.next() * 700.0, rng.next() * 520.0);
            let (w, h) = (30.0 + rng.next() * 120.0, 20.0 + rng.next() * 80.0);
            let color = Color::rgb((rng.next() * 255.0) as u8, (rng.next() * 255.0) as u8, (rng.next() * 255.0) as u8);
            let mut n = match i % 10 {
                0..=2 => {
                    let mut n = Node::rectangle(id, Rect::new(x, y, w, h));
                    n.fills = vec![Paint::solid(color)];
                    n.corner_radii = [6.0; 4];
                    n
                }
                3..=5 => {
                    let style = TextStyle { font_size: 14.0, fills: vec![Paint::solid(Color::rgb(0x22, 0x22, 0x22))], ..Default::default() };
                    Node::text(id, Rect::new(x, y, 160.0, 20.0), format!("Label {f}-{i} lorem ipsum"), style)
                }
                6 | 7 => {
                    let mut n = Node::new(id, NodeKind::Vector { paths: vec![star(w, h)], network: None });
                    n.transform = Affine::translate(x, y);
                    n.size = Vec2::new(w, h);
                    n.fills = vec![Paint::solid(color)];
                    n
                }
                8 => {
                    let mut n = Node::rectangle(id, Rect::new(x, y, w, h));
                    n.fills = vec![Paint::solid(Color::WHITE)];
                    n.effects = vec![Effect::DropShadow {
                        color: Color { r: 0.0, g: 0.0, b: 0.0, a: 0.25 },
                        offset: Vec2::new(0.0, 4.0),
                        radius: 8.0,
                        spread: 0.0,
                        visible: true,
                        show_behind: false,
                        blend: Default::default(),
                    }];
                    n
                }
                _ => {
                    let mut n = Node::rectangle(id, Rect::new(x, y, w, h));
                    n.fills = vec![Paint {
                        kind: PaintKind::Image { image_ref: "checker".into(), scale_mode: ImageScaleMode::Fill },
                        opacity: 1.0,
                        visible: true,
                        blend: BlendMode::Normal,
                    }];
                    n
                }
            };
            n.name = format!("n{f}-{i}");
            doc.insert(fid, n).unwrap();
            leaves.push(id);
        }
    }
    (doc, page, leaves)
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

fn stats(v: &[f64]) -> String {
    let mut s = v.to_vec();
    s.sort_by(|a, b| a.partial_cmp(b).unwrap());
    format!("median {:6.2} ms  p95 {:6.2} ms  max {:6.1} ms", s[s.len() / 2], s[s.len() * 95 / 100], s[s.len() - 1])
}

struct Gpu {
    g: HeadlessGpu,
}

impl Gpu {
    /// One frame: compile, encode, submit, wait. Returns (compile ms, total ms).
    fn frame(&mut self, doc: &Document, page: NodeId, rev: u64, vp: &Viewport) -> (f64, f64) {
        let t0 = Instant::now();
        let frame = self.g.compile(doc, page, rev, vp, (W, H));
        let t1 = Instant::now();
        self.submit(&frame);
        (ms(t1 - t0), ms(t0.elapsed()))
    }

    fn submit(&mut self, frame: &designos_gpu::Frame) {
        let mut enc = self.g.ctx.device.create_command_encoder(&Default::default());
        self.g.renderer.render(frame, &mut enc);
        self.g.ctx.queue.submit([enc.finish()]);
        self.g.renderer.wait_idle();
    }
}

fn report(name: &str, v: &[(f64, f64)]) {
    let c: Vec<f64> = v.iter().map(|x| x.0).collect();
    let t: Vec<f64> = v.iter().map(|x| x.1).collect();
    println!(
        "{name:<34} compile {}
{:<34} frame   {}",
        stats(&c),
        "",
        stats(&t)
    );
}

fn main() {
    let (mut doc, page, leaves) = big_doc();
    println!("nodes: {}   viewport {W}x{H}", doc.nodes.len());
    let mut gpu = match HeadlessGpu::new() {
        Ok(g) => Gpu { g },
        Err(e) => {
            println!("no GPU adapter ({e}); nothing to benchmark");
            return;
        }
    };
    println!("adapter: {} ({:?})\n", gpu.g.ctx.adapter_info.name, gpu.g.ctx.adapter_info.backend);
    // the same per-frame re-flattening budget the editor uses
    gpu.g.compiler.geom.budget = Some(Duration::from_millis(6));
    let view = |zoom: f32| Viewport { offset: Vec2::new(900.0 + 400.0 - W as f32 / zoom / 2.0, 700.0 + 300.0 - H as f32 / zoom / 2.0), zoom, dpr: 1.0 };
    let mut rev = 1u64;

    println!("== GPU ==");
    for z in [0.25f32, 1.0, 4.0] {
        let vp = view(z);
        let cold = gpu.frame(&doc, page, rev, &vp);
        println!("zoom {z:>4}: cold first frame (tessellate everything visible) compile {:.1} ms, frame {:.1} ms", cold.0, cold.1);
        let st = gpu.g.renderer.stats;
        let fs = gpu.g.compile(&doc, page, rev, &vp, (W, H)).stats();
        println!(
            "          steady frame: {} programs, {} draws, {} composites, {} tris; {} passes, {} draw calls",
            fs.programs, fs.draws, fs.composites, fs.triangles, st.passes, st.draw_calls
        );
        // pan 40 device px per frame
        let mut vp2 = vp;
        let t: Vec<_> = (0..60)
            .map(|_| {
                vp2.offset.x += 40.0 / z;
                gpu.frame(&doc, page, rev, &vp2)
            })
            .collect();
        report(&format!("zoom {z:>4}: pan"), &t);
        // zoom gesture: x1.03 per frame around the viewport centre
        let mut vp3 = vp;
        let t: Vec<_> = (0..30)
            .map(|_| {
                let c = Vec2::new(vp3.offset.x + W as f32 / vp3.zoom / 2.0, vp3.offset.y + H as f32 / vp3.zoom / 2.0);
                vp3.zoom *= 1.03;
                vp3.offset = Vec2::new(c.x - W as f32 / vp3.zoom / 2.0, c.y - H as f32 / vp3.zoom / 2.0);
                gpu.frame(&doc, page, rev, &vp3)
            })
            .collect();
        report(&format!("zoom {z:>4}: zoom gesture (x1.03/frame)"), &t);
        // drag one node: revision bump + transform change each frame
        let victim = leaves[100 + 3];
        let t: Vec<_> = (0..40)
            .map(|_| {
                doc.get_mut(victim).unwrap().transform.tx += 5.0;
                rev += 1;
                gpu.frame(&doc, page, rev, &vp)
            })
            .collect();
        report(&format!("zoom {z:>4}: drag one node"), &t);
        println!();
    }

    // Smart Animate: one 800x600 frame of 100 nodes, interpolated between two states
    let frame_id = doc.get(page).unwrap().children[1];
    let kids = doc.get(frame_id).unwrap().children.clone();
    let from: Vec<Node> = kids.iter().map(|k| doc.get(*k).unwrap().clone()).collect();
    let mut gpu_t = Vec::new();
    let mut cpu_t = Vec::new();
    let mut gs = gpu.g.compiler.compile_node(&doc, frame_id, rev, 1.0, (800, 600));
    gpu.submit(&gs);
    for step in 0..60 {
        let t = (step as f32 / 59.0).min(1.0);
        let e = t * t * (3.0 - 2.0 * t);
        for (k, n0) in kids.iter().zip(&from) {
            let n = doc.get_mut(*k).unwrap();
            n.transform.tx = n0.transform.tx + 120.0 * e;
            n.transform.ty = n0.transform.ty + 40.0 * e;
            n.size = Vec2::new(n0.size.x * (1.0 + 0.2 * e), n0.size.y * (1.0 + 0.2 * e));
            n.opacity = 1.0 - 0.4 * e;
        }
        rev += 1;
        let t0 = Instant::now();
        gs = gpu.g.compiler.compile_node(&doc, frame_id, rev, 1.0, (800, 600));
        let c = ms(t0.elapsed());
        gpu.submit(&gs);
        gpu_t.push((c, ms(t0.elapsed())));
        let t0 = Instant::now();
        let pm = render_node(&doc, frame_id, 1.0);
        std::hint::black_box(&pm);
        cpu_t.push(ms(t0.elapsed()));
    }
    println!("== Smart Animate frames (800x600 frame, 100 nodes interpolated per frame) ==");
    report("GPU (compile_node + render)", &gpu_t);
    println!("CPU render_node                    {}", stats(&cpu_t));

    println!("\n== CPU TileCache (the current canvas path) ==");
    let big = Duration::from_secs(60);
    let mut cache = TileCache::new();
    let (mut doc2, page2, leaves2) = big_doc();
    let mut rev2 = 1u64;
    for z in [0.25f32, 1.0, 4.0] {
        let vp = view(z);
        cache.clear();
        let s = Instant::now();
        let f = cache.render_tiles(&doc2, page2, &vp, (W, H), rev2, TileBudget::time(big));
        println!("zoom {z:>4}: cold viewport ({} tiles) {:.1} ms", f.tiles.len(), ms(s.elapsed()));
        let mut vp2 = vp;
        let t: Vec<f64> = (0..60)
            .map(|_| {
                vp2.offset.x += 40.0 / z;
                let s = Instant::now();
                let f = cache.render_tiles(&doc2, page2, &vp2, (W, H), rev2, TileBudget::time(Duration::from_millis(6)));
                std::hint::black_box(f.tiles.len());
                ms(s.elapsed())
            })
            .collect();
        println!("zoom {z:>4}: pan frame (6 ms budget)        {}", stats(&t));
        // zoom gesture: tiles of the settled zoom are scaled by the GPU, so a frame costs a cache lookup
        let t: Vec<f64> = (0..30)
            .map(|_| {
                let s = Instant::now();
                let f = cache.render_tiles(&doc2, page2, &vp, (W, H), rev2, TileBudget::time(Duration::from_millis(6)));
                std::hint::black_box(f.tiles.len());
                ms(s.elapsed())
            })
            .collect();
        println!("zoom {z:>4}: zoom gesture frame (cached)   {}  (+ cold re-render above when the zoom settles)", stats(&t));
        // drag
        let victim = leaves2[100 + 3];
        let t: Vec<f64> = (0..40)
            .map(|_| {
                doc2.get_mut(victim).unwrap().transform.tx += 5.0;
                rev2 += 1;
                let s = Instant::now();
                let f = cache.render_tiles(&doc2, page2, &vp, (W, H), rev2, TileBudget::time(Duration::from_millis(6)));
                std::hint::black_box(f.tiles.len());
                ms(s.elapsed())
            })
            .collect();
        println!("zoom {z:>4}: drag frame                     {}", stats(&t));
    }
}
