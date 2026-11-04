use designos_doc::*;
use designos_render::{render_page, tiny_skia::Pixmap, Viewport};

fn render_align(align: StrokeAlign) -> Pixmap {
    let mut doc = Document::new("stroke-probe");
    let page = doc.add_page("p");
    doc.get_mut(page).unwrap().kind = NodeKind::Page { background: Color::WHITE };
    let id = doc.alloc_id();
    let mut n =
        Node::text(id, Rect::new(16.0, 12.0, 120.0, 64.0), "H", TextStyle { font_size: 48.0, fills: vec![Paint::solid(Color::BLACK)], ..Default::default() });
    n.stroke.paints = vec![Paint::solid(Color::rgb(255, 0, 0))];
    n.stroke.weight = 6.0;
    n.stroke.align = align;
    doc.insert(page, n).unwrap();
    let mut pm = Pixmap::new(180, 100).unwrap();
    render_page(&doc, page, &Viewport::default(), &mut pm);
    pm
}

fn bbox(pm: &Pixmap) -> (u32, u32, u32, u32) {
    let mut x0 = pm.width();
    let mut y0 = pm.height();
    let mut x1 = 0;
    let mut y1 = 0;
    for y in 0..pm.height() {
        for x in 0..pm.width() {
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
fn text_stroke_alignment_changes_rendered_footprint() {
    let inside = render_align(StrokeAlign::Inside);
    let center = render_align(StrokeAlign::Center);
    let outside = render_align(StrokeAlign::Outside);
    let bi = bbox(&inside);
    let bc = bbox(&center);
    let bo = bbox(&outside);
    eprintln!("inside={bi:?} center={bc:?} outside={bo:?}");
    assert!(bi.0 > bc.0 && bi.2 < bc.2, "inside stroke must stay within the glyph edge: inside={bi:?} center={bc:?}");
    assert!(bo.0 < bc.0 && bo.2 > bc.2, "outside stroke must extend beyond the glyph edge: outside={bo:?} center={bc:?}");
}
