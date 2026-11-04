mod common;

use common::*;
use designos_doc::*;
use designos_render::Viewport;

fn scene() -> T {
    let mut t = T::new();
    for (x, align) in [(16.0, StrokeAlign::Inside), (76.0, StrokeAlign::Center), (136.0, StrokeAlign::Outside)] {
        let id = t.add(None, |id| {
            Node::text(id, Rect::new(x, 12.0, 50.0, 64.0), "H", TextStyle { font_size: 48.0, fills: vec![Paint::solid(Color::BLACK)], ..Default::default() })
        });
        let n = t.node(id);
        n.stroke.paints = vec![Paint::solid(Color::rgb(255, 0, 0))];
        n.stroke.weight = 6.0;
        n.stroke.align = align;
    }
    t
}

fn bbox(pm: &designos_render::tiny_skia::Pixmap, x_start: u32, x_end: u32) -> (u32, u32, u32, u32) {
    let mut x0 = x_end;
    let mut y0 = pm.height();
    let mut x1 = x_start;
    let mut y1 = 0;
    for y in 0..pm.height() {
        for x in x_start..x_end {
            let p = pm.pixel(x, y).unwrap();
            if p.red() < 250 || p.green() < 250 || p.blue() < 250 {
                x0 = x0.min(x);
                y0 = y0.min(y);
                x1 = x1.max(x);
                y1 = y1.max(y);
            }
        }
    }
    (x0, y0, x1, y1)
}

#[test]
fn text_stroke_alignment_matches_cpu_reference() {
    let Some(mut g) = gpu() else { return };
    let t = scene();
    let c = compare(&mut g, "text_stroke_alignment", &t.doc, t.page, Viewport::default(), (220, 100), 8);
    assert!(c.report_aa.ratio < 0.03, "CPU/GPU mismatch {:.3}%", c.report_aa.ratio * 100.0);
    let inside = bbox(&c.gpu, 0, 60);
    let center = bbox(&c.gpu, 60, 120);
    let outside = bbox(&c.gpu, 120, 200);
    eprintln!("inside={inside:?} center={center:?} outside={outside:?}");
    assert!(inside.0 > 17 && inside.2 < 50, "inside GPU extent {inside:?}");
    assert_eq!(center, (77, 20, 110, 61), "center GPU extent");
    assert!(outside.0 < 137 && outside.2 > 170, "outside GPU extent {outside:?}");
}
