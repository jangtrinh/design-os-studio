//! Per-effect cost: one drop shadow (rounded card, ellipse, text), a layer blur
//! and a background blur, with the optimised pipeline against the original
//! implementation (`set_fast_effects(false)`).
//!
//! Run with `cargo run --release -p designos-render --example bench_shadow`.

use designos_doc::*;
use designos_render::tiny_skia::Pixmap;
use designos_render::{clear_shadow_cache, render_page, set_fast_effects, Viewport};
use std::time::Instant;

fn shadow(radius: f32, spread: f32) -> Effect {
    Effect::DropShadow {
        color: Color { r: 0.0, g: 0.0, b: 0.0, a: 0.25 },
        offset: Vec2::new(0.0, 4.0),
        radius,
        spread,
        visible: true,
        show_behind: false,
        blend: Default::default(),
    }
}

fn scene(kind: &str, radius: f32, count: usize) -> (Document, NodeId) {
    let mut doc = Document::new("b");
    let page = doc.add_page("P");
    doc.get_mut(page).unwrap().kind = NodeKind::Page { background: Color::rgb(0xE5, 0xE5, 0xE5) };
    for i in 0..count {
        let id = doc.alloc_id();
        let (x, y) = (20.0 + (i % 5) as f32 * 180.0, 20.0 + (i / 5) as f32 * 130.0);
        let mut n = match kind {
            "ellipse" => {
                let mut n = Node::new(id, NodeKind::Ellipse(Default::default()));
                n.transform = Affine::translate(x, y);
                n.size = Vec2::new(150.0, 100.0);
                n
            }
            _ => {
                let mut n = Node::rectangle(id, Rect::new(x, y, 150.0, 100.0));
                n.corner_radii = [12.0; 4];
                n
            }
        };
        n.fills = vec![Paint::solid(Color::WHITE)];
        match kind {
            "layer blur" => n.effects = vec![Effect::LayerBlur { radius, visible: true }],
            "background blur" => {
                n.fills = vec![Paint::solid(Color { r: 1.0, g: 1.0, b: 1.0, a: 0.3 })];
                n.effects = vec![Effect::BackgroundBlur { radius, visible: true }];
            }
            _ => n.effects = vec![shadow(radius, 0.0)],
        }
        doc.insert(page, n).unwrap();
    }
    (doc, page)
}

fn time(doc: &Document, page: NodeId, vp: &Viewport, pm: &mut Pixmap, reps: usize) -> f64 {
    render_page(doc, page, vp, pm);
    let s = Instant::now();
    for _ in 0..reps {
        render_page(doc, page, vp, pm);
    }
    s.elapsed().as_secs_f64() * 1000.0 / reps as f64
}

fn blur_table() {
    use designos_render::bench_blur;
    println!("\nraw blur of a 600x400 shape on an 800x600 pixmap");
    println!("{:>6} {:>12} {:>14} {:>14}", "sigma", "before ms", "after (exact)", "after (margin)");
    for sigma in [2.0f32, 4.0, 8.0, 16.0, 32.0, 64.0] {
        let mut src = Pixmap::new(800, 600).unwrap();
        let mut paint = designos_render::tiny_skia::Paint::default();
        paint.set_color_rgba8(0, 0, 0, 255);
        let rect = designos_render::tiny_skia::Rect::from_xywh(100.0, 100.0, 600.0, 400.0).unwrap();
        src.fill_rect(rect, &paint, designos_render::tiny_skia::Transform::identity(), None);
        let run = |reference: bool, margined: bool| {
            let mut pm = src.clone();
            bench_blur(&mut pm, sigma, reference, margined);
            let s = Instant::now();
            for _ in 0..5 {
                let mut pm = src.clone();
                bench_blur(&mut pm, sigma, reference, margined);
                std::hint::black_box(&pm);
            }
            s.elapsed().as_secs_f64() * 1000.0 / 5.0
        };
        println!("{sigma:>6} {:>12.2} {:>14.2} {:>14.2}", run(true, false), run(false, false), run(false, true));
    }
}

fn main() {
    blur_table();
    println!("\n{:<18} {:>6} {:>5} {:>13} {:>13} {:>9}", "effect", "radius", "zoom", "before ms/fx", "after ms/fx", "speedup");
    for (kind, radius) in [
        ("rounded shadow", 8.0f32),
        ("rounded shadow", 24.0),
        ("rounded shadow", 64.0),
        ("ellipse shadow", 8.0),
        ("ellipse shadow", 24.0),
        ("ellipse shadow", 64.0),
        ("layer blur", 8.0),
        ("layer blur", 40.0),
        ("background blur", 16.0),
        ("background blur", 64.0),
    ] {
        for zoom in [1.0f32, 4.0] {
            let count = 20;
            let sk = if kind == "rounded shadow" {
                "rect"
            } else if kind == "ellipse shadow" {
                "ellipse"
            } else {
                kind
            };
            let (doc, page) = scene(sk, radius, count);
            let vp = Viewport { offset: Vec2::ZERO, zoom, dpr: 1.0 };
            let (w, h) = ((900.0 * zoom).min(2400.0) as u32, (560.0 * zoom).min(1500.0) as u32);
            let mut pm = Pixmap::new(w, h).unwrap();
            // the fixed cost of drawing the same scene without effects
            let (plain, _) = {
                let (mut d2, p2) = scene(sk, radius, count);
                let ids: Vec<NodeId> = d2.get(p2).unwrap().children.clone();
                for id in ids {
                    d2.get_mut(id).unwrap().effects.clear();
                }
                let t = time(&d2, p2, &vp, &mut pm, 3);
                (t, ())
            };
            set_fast_effects(false);
            let before = (time(&doc, page, &vp, &mut pm, 3) - plain).max(0.001) / count as f64;
            set_fast_effects(true);
            clear_shadow_cache();
            let after = (time(&doc, page, &vp, &mut pm, 5) - plain).max(0.001) / count as f64;
            println!("{kind:<18} {radius:>6} {zoom:>5} {before:>13.3} {after:>13.3} {:>8.1}x", before / after);
        }
    }
}
