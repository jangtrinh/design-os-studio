//! Benchmark on a large page: 50 frames x 100 children mixing rects, text,
//! vectors, shadows and images (5,000+ nodes) at 1505x1125.
//!
//! Run with `cargo run --release -p designos-render --example bench_big`.

use designos_doc::*;
use designos_render::tiny_skia::Pixmap;
use designos_render::{render_page, render_page_indexed, SceneIndex, TileBudget, TileCache, Viewport};
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

/// Tiny deterministic generator so runs are comparable.
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
            // BENCH_ONLY=3,4,5 keeps only those i % 10 classes (profiling aid)
            if let Ok(only) = std::env::var("BENCH_ONLY") {
                if !only.split(',').any(|c| c.trim().parse::<usize>().ok() == Some(i % 10)) {
                    continue;
                }
            }
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

fn median(mut v: Vec<f64>) -> f64 {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[v.len() / 2]
}

fn stats(v: &[f64]) -> String {
    let mut s = v.to_vec();
    s.sort_by(|a, b| a.partial_cmp(b).unwrap());
    format!("median {:.2} ms, p95 {:.2} ms, max {:.1} ms", s[s.len() / 2], s[s.len() * 95 / 100], s[s.len() - 1])
}

fn main() {
    if std::env::var("BENCH_REFERENCE_FX").is_ok() {
        designos_render::set_fast_effects(false);
        println!("(original blur and shadow implementation)");
    }
    let (mut doc, page, leaves) = big_doc();
    println!("nodes: {}   viewport {W}x{H}   threads: {}", doc.nodes.len(), std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1));
    let mut pm = Pixmap::new(W, H).unwrap();

    // The viewport centres on the 2x2 frames block near the top-left of the page.
    let view = |zoom: f32| Viewport { offset: Vec2::new(900.0 + 400.0 - W as f32 / zoom / 2.0, 700.0 + 300.0 - H as f32 / zoom / 2.0), zoom, dpr: 1.0 };

    println!("\n== full render_page (baseline: whole viewport, one pixmap) ==");
    for z in [0.25f32, 1.0, 4.0] {
        let vp = view(z);
        render_page(&doc, page, &vp, &mut pm); // warm up (font + layout caches)
        let t: Vec<f64> = (0..5)
            .map(|_| {
                let s = Instant::now();
                render_page(&doc, page, &vp, &mut pm);
                ms(s.elapsed())
            })
            .collect();
        println!("zoom {z:>4}: median {:.1} ms", median(t));
    }

    println!("\n== render_page_indexed (spatial index, still one pixmap) ==");
    let mut idx = SceneIndex::new();
    let mut rev = 1u64;
    for z in [0.25f32, 1.0, 4.0] {
        let vp = view(z);
        render_page_indexed(&doc, page, &vp, &mut pm, &mut idx, rev);
        let t: Vec<f64> = (0..5)
            .map(|_| {
                let s = Instant::now();
                render_page_indexed(&doc, page, &vp, &mut pm, &mut idx, rev);
                ms(s.elapsed())
            })
            .collect();
        println!("zoom {z:>4}: median {:.1} ms", median(t));
    }

    println!("\n== TileCache (256px tiles, parallel) ==");
    let big = Duration::from_secs(60);
    let mut cache = TileCache::new();
    for z in [0.25f32, 1.0, 4.0] {
        let vp = view(z);
        cache.clear();
        let s = Instant::now();
        let f = cache.render_tiles(&doc, page, &vp, (W, H), rev, TileBudget::time(big));
        println!("zoom {z:>4}: cold viewport ({} tiles, incl. index build) {:.1} ms", f.tiles.len(), ms(s.elapsed()));
        // Pan 40 device px per frame with a 6 ms render budget (what the app uses).
        let mut times = Vec::new();
        let mut vp2 = vp;
        for _ in 0..60 {
            vp2.offset.x += 40.0 / z;
            let s = Instant::now();
            let f = cache.render_tiles(&doc, page, &vp2, (W, H), rev, TileBudget::time(Duration::from_millis(6)));
            std::hint::black_box(f.tiles.len());
            times.push(ms(s.elapsed()));
        }
        println!("zoom {z:>4}: pan frame (prefetch ring on)  {}", stats(&times));
        // worst case: no prefetch, every new column of tiles is rendered on demand
        cache.prefetch_ring = 0;
        let mut times = Vec::new();
        for _ in 0..60 {
            vp2.offset.x += 40.0 / z;
            let s = Instant::now();
            let f = cache.render_tiles(&doc, page, &vp2, (W, H), rev, TileBudget::time(Duration::from_millis(6)));
            std::hint::black_box(f.tiles.len());
            times.push(ms(s.elapsed()));
        }
        println!("zoom {z:>4}: pan frame (no prefetch)       {}", stats(&times));
        cache.prefetch_ring = 1;
        while cache.render_tiles(&doc, page, &vp2, (W, H), rev, TileBudget::unlimited()).prefetch_pending > 0 {}
        let s = Instant::now();
        let f = cache.render_tiles(&doc, page, &vp2, (W, H), rev, TileBudget::time(big));
        std::hint::black_box(f.tiles.len());
        println!("zoom {z:>4}: idle frame (all cached) {:.3} ms", ms(s.elapsed()));
    }

    println!("\n== TileCache steady-state costs ==");
    for z in [0.25f32, 1.0, 4.0] {
        let vp = view(z);
        cache.clear();
        rev += 1;
        while cache.render_tiles(&doc, page, &vp, (W, H), rev, TileBudget::unlimited()).prefetch_pending > 0 {}
        let t: Vec<f64> = (0..10)
            .map(|_| {
                rev += 1;
                let s = Instant::now();
                cache.render_tiles(&doc, page, &vp, (W, H), rev, TileBudget::time(big));
                ms(s.elapsed())
            })
            .collect();
        println!("zoom {z:>4}: revision bump, nothing changed (index rebuild + doc diff): median {:.2} ms", median(t));
        let t: Vec<f64> = (0..8)
            .map(|_| {
                cache.invalidate_all();
                let s = Instant::now();
                let f = cache.render_tiles(&doc, page, &vp, (W, H), rev, TileBudget::time(big));
                std::hint::black_box(f.rendered);
                ms(s.elapsed())
            })
            .collect();
        println!("zoom {z:>4}: re-render every visible tile (warm, parallel): median {:.1} ms", median(t));
        let mut one = Vec::new();
        for _ in 0..3 {
            cache.invalidate_all();
            loop {
                let s = Instant::now();
                let f = cache.render_tiles(&doc, page, &vp, (W, H), rev, TileBudget::tiles(1));
                if f.rendered == 0 {
                    break;
                }
                one.push(ms(s.elapsed()));
            }
        }
        println!("zoom {z:>4}: single tile on one thread: median {:.2} ms, max {:.1} ms", median(one.clone()), one.iter().cloned().fold(0.0, f64::max));
    }

    println!("\n== drag one node, zoom 1 ==");
    let vp = view(1.0);
    cache.clear();
    rev += 1;
    while cache.render_tiles(&doc, page, &vp, (W, H), rev, TileBudget::unlimited()).prefetch_pending > 0 {}
    // a leaf of the frame at (900, 700), inside the viewport
    let victim = leaves[100 + 3];
    let mut times = Vec::new();
    for _ in 0..40 {
        doc.get_mut(victim).unwrap().transform.tx += 5.0;
        rev += 1;
        let s = Instant::now();
        let f = cache.render_tiles(&doc, page, &vp, (W, H), rev, TileBudget::time(Duration::from_millis(6)));
        std::hint::black_box(f.tiles.len());
        times.push(ms(s.elapsed()));
    }
    println!("drag frame (diff doc + re-render dirty tiles): {}", stats(&times));

    println!("\n== drag one node, zoom 4 ==");
    let vp4 = view(4.0);
    cache.clear();
    rev += 1;
    while cache.render_tiles(&doc, page, &vp4, (W, H), rev, TileBudget::unlimited()).prefetch_pending > 0 {}
    // a shadowed rectangle inside the zoom-4 viewport (page x 1112..1488, y 860..1140)
    let victim = leaves[100..200]
        .iter()
        .copied()
        .find(|id| {
            let n = doc.get(*id).unwrap();
            let (x, y) = (900.0 + n.transform.tx, 700.0 + n.transform.ty);
            !n.effects.is_empty() && (1130.0..1400.0).contains(&x) && (880.0..1100.0).contains(&y)
        })
        .unwrap_or(victim);
    let mut times = Vec::new();
    for _ in 0..40 {
        doc.get_mut(victim).unwrap().transform.ty += 1.0;
        rev += 1;
        let s = Instant::now();
        let f = cache.render_tiles(&doc, page, &vp4, (W, H), rev, TileBudget::time(Duration::from_millis(6)));
        std::hint::black_box(f.tiles.len());
        times.push(ms(s.elapsed()));
    }
    println!("drag frame: {}", stats(&times));

    println!("\n== baseline drag: whole-viewport render_page per frame, zoom 1 ==");
    let mut times = Vec::new();
    for _ in 0..7 {
        doc.get_mut(victim).unwrap().transform.tx += 5.0;
        let s = Instant::now();
        render_page(&doc, page, &vp, &mut pm);
        times.push(ms(s.elapsed()));
    }
    println!("median {:.1} ms", median(times));
}
