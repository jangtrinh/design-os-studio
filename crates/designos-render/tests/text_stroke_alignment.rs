use designos_doc::*;
use designos_render::{render_page, tiny_skia::Pixmap, Viewport};

const FONT: f32 = 96.0;
const WEIGHT: f32 = 6.0;

fn render(align: Option<StrokeAlign>) -> Pixmap {
    let mut doc = Document::new("stroke-probe");
    let page = doc.add_page("p");
    doc.get_mut(page).unwrap().kind = NodeKind::Page { background: Color::WHITE };
    let id = doc.alloc_id();
    let mut n =
        Node::text(id, Rect::new(40.0, 24.0, 160.0, 140.0), "O", TextStyle { font_size: FONT, fills: vec![Paint::solid(Color::BLACK)], ..Default::default() });
    if let Some(align) = align {
        n.stroke.paints = vec![Paint::solid(Color::rgb(255, 0, 0))];
        n.stroke.weight = WEIGHT;
        n.stroke.align = align;
    }
    doc.insert(page, n).unwrap();
    let mut pm = Pixmap::new(260, 200).unwrap();
    render_page(&doc, page, &Viewport::default(), &mut pm);
    pm
}

fn px(pm: &Pixmap, x: u32, y: u32) -> (u8, u8, u8) {
    let p = pm.pixel(x, y).unwrap();
    (p.red(), p.green(), p.blue())
}

fn is_page(c: (u8, u8, u8)) -> bool {
    c.0 > 245 && c.1 > 245 && c.2 > 245
}

fn is_red(c: (u8, u8, u8)) -> bool {
    c.0 > 180 && c.1 < 60 && c.2 < 60
}

fn is_black(c: (u8, u8, u8)) -> bool {
    c.0 < 32 && c.1 < 32 && c.2 < 32
}

fn ink_runs(pm: &Pixmap, y: u32) -> Vec<(u32, u32)> {
    let mut runs = Vec::new();
    let mut start = None;
    for x in 0..pm.width() {
        let ink = !is_page(px(pm, x, y));
        match (start, ink) {
            (None, true) => start = Some(x),
            (Some(s), false) => {
                runs.push((s, x - 1));
                start = None;
            }
            _ => {}
        }
    }
    if let Some(s) = start {
        runs.push((s, pm.width() - 1));
    }
    runs
}

#[derive(Debug)]
struct Counter {
    y: u32,
    outer: u32,
    inner: u32,
    stem_mid: u32,
    counter_mid: u32,
}

fn counters(pm: &Pixmap) -> Vec<Counter> {
    let mut best_gap = 0u32;
    let mut best_y = 0u32;
    let mut best_runs = Vec::new();
    for y in 0..pm.height() {
        let runs = ink_runs(pm, y);
        if runs.len() < 2 || !runs.len().is_multiple_of(2) {
            continue;
        }
        let gap: u32 = runs.chunks(2).map(|c| c[1].0.saturating_sub(c[0].1)).sum();
        if gap > best_gap {
            best_gap = gap;
            best_y = y;
            best_runs = runs;
        }
    }
    assert!(!best_runs.is_empty(), "glyph has a counter");
    let y = best_y;
    let runs = best_runs;
    runs.chunks(2)
        .map(|c| {
            let (outer, inner) = c[0];
            let right = c[1].0;
            let mut solid = Vec::new();
            for x in outer..=inner {
                if is_black(px(pm, x, y)) {
                    solid.push(x);
                }
            }
            assert!(!solid.is_empty(), "stem has a solid fill");
            Counter { y, outer, inner, stem_mid: solid[solid.len() / 2], counter_mid: (inner + right) / 2 }
        })
        .collect()
}

fn assert_red(pm: &Pixmap, x: u32, y: u32, what: &str) {
    let c = px(pm, x, y);
    assert!(is_red(c), "{what} at ({x},{y}) is rgb{c:?}");
}

fn assert_page(pm: &Pixmap, x: u32, y: u32, what: &str) {
    let c = px(pm, x, y);
    assert!(is_page(c), "{what} at ({x},{y}) is rgb{c:?}");
}

fn assert_black(pm: &Pixmap, x: u32, y: u32, what: &str) {
    let c = px(pm, x, y);
    assert!(is_black(c), "{what} at ({x},{y}) is rgb{c:?}");
}

fn assert_o_stroke(pm: &Pixmap, c: &Counter, align: StrokeAlign) {
    let y = c.y;
    assert!(c.outer >= 4 && c.inner + 4 < pm.width(), "samples stay inside the pixmap: {c:?}");
    assert!(c.counter_mid > c.inner + 3, "counter is wider than the stroke");
    match align {
        StrokeAlign::Inside => {
            assert_red(pm, c.outer + 3, y, "red just inside the outer edge");
            assert_page(pm, c.outer - 3, y, "no red outside the outer edge");
            assert_page(pm, c.counter_mid, y, "no red in the counter center");
        }
        StrokeAlign::Outside => {
            assert_red(pm, c.outer - 3, y, "red just past the outer edge");
            assert_red(pm, c.inner + 3, y, "red in the counter next to the inner contour");
            assert_black(pm, c.stem_mid, y, "no red deep in the fill");
        }
        StrokeAlign::Center => {
            assert_red(pm, c.outer - 2, y, "red outside the edge");
            assert_red(pm, c.outer + 2, y, "red inside the edge");
        }
    }
}

#[test]
fn text_stroke_alignment_changes_rendered_footprint() {
    let fill = render(None);
    let rings = counters(&fill);
    assert_eq!(rings.len(), 1, "one counter");
    let ring = &rings[0];
    eprintln!("counter y={} outer={} inner={} stem_mid={} counter_mid={}", ring.y, ring.outer, ring.inner, ring.stem_mid, ring.counter_mid);
    for align in [StrokeAlign::Inside, StrokeAlign::Center, StrokeAlign::Outside] {
        assert_o_stroke(&render(Some(align)), ring, align);
    }
}
