//! Reverse-engineering probe (ignored by default): where the renderer's live (non-stored) stroke of a
//! LINE layer lands, per cap kind and weight, against the axis the line runs along (local y = 0, +x along
//! the line, y grows downward). Pairs with `kit_line_stroke_band_probe`, which measures Figma's stored band.

use designos_doc::*;
use designos_render::tiny_skia::Pixmap;
use designos_render::{render_page, Viewport};

const AT: Vec2 = Vec2 { x: 60.0, y: 60.0 };
const LEN: f32 = 200.0;

fn doc_with_line(w: f32, cap: StrokeCap, align: StrokeAlign) -> (Document, NodeId) {
    let mut doc = Document::new("probe");
    let page = doc.add_page("p");
    doc.get_mut(page).unwrap().kind = NodeKind::Page { background: Color::WHITE };
    let id = doc.alloc_id();
    let mut n = Node::new(id, NodeKind::Line);
    n.transform = Affine::translate(AT.x, AT.y);
    n.size = Vec2::new(LEN, 0.0);
    n.fills.clear();
    n.stroke.paints = vec![Paint::solid(Color::rgb(0, 0, 0))];
    n.stroke.weight = w;
    n.stroke.cap = cap;
    n.stroke.align = align;
    doc.insert(page, n).unwrap();
    (doc, page)
}

fn render(doc: &Document, page: NodeId) -> Pixmap {
    let mut pm = Pixmap::new(360, 160).unwrap();
    render_page(doc, page, &Viewport::default(), &mut pm);
    pm
}

fn cov(pm: &Pixmap, x: u32, y: u32) -> f32 {
    1.0 - pm.pixel(x, y).unwrap().red() as f32 / 255.0
}

/// (thickness, centroid y) of the column at x, coverage-weighted, relative to the axis y = AT.y.
fn column_band(pm: &Pixmap, x: u32) -> (f32, f32) {
    let (mut sum, mut mom) = (0.0, 0.0);
    for y in 0..pm.height() {
        let c = cov(pm, x, y);
        sum += c;
        mom += c * (y as f32 + 0.5);
    }
    if sum <= 0.0 {
        return (0.0, 0.0);
    }
    (sum, mom / sum - AT.y)
}

/// Ink extent with a 50 % coverage threshold: (x0, x1, y0, y1) in pixel edges.
fn ink(pm: &Pixmap) -> Option<(u32, u32, u32, u32)> {
    let (mut x0, mut x1, mut y0, mut y1) = (u32::MAX, 0, u32::MAX, 0);
    for y in 0..pm.height() {
        for x in 0..pm.width() {
            if cov(pm, x, y) >= 0.5 {
                (x0, x1, y0, y1) = (x0.min(x), x1.max(x + 1), y0.min(y), y1.max(y + 1));
            }
        }
    }
    (x0 != u32::MAX).then_some((x0, x1, y0, y1))
}

fn r3(x: f32) -> String {
    format!("{:.3}", (x * 1000.0).round() / 1000.0 + 0.0)
}

#[test]
#[ignore = "probe: prints measurements"]
fn probe_live_line_stroke_band() {
    println!("live stroke of a Line with no stored outline; axis is y = {} (local y = 0); line runs x = {} .. {}", AT.y, AT.x, AT.x + LEN);
    let caps = [
        ("None", StrokeCap::None),
        ("Round", StrokeCap::Round),
        ("Square", StrokeCap::Square),
        ("ArrowLines", StrokeCap::ArrowLines),
        ("ArrowEquilateral", StrokeCap::ArrowEquilateral),
        ("TriangleFilled", StrokeCap::TriangleFilled),
        ("DiamondFilled", StrokeCap::DiamondFilled),
        ("CircleFilled", StrokeCap::CircleFilled),
    ];
    for align in [StrokeAlign::Center, StrokeAlign::Inside, StrokeAlign::Outside] {
        for (name, cap) in caps {
            for w in [1.0f32, 2.0, 10.0] {
                if matches!(align, StrokeAlign::Inside | StrokeAlign::Outside) && !(name == "None" || name == "Round") {
                    continue;
                }
                let (doc, page) = doc_with_line(w, cap, align);
                let pm = render(&doc, page);
                let mid = (AT.x + LEN / 2.0) as u32;
                let (thick, centre) = column_band(&pm, mid);
                let e = ink(&pm);
                match e {
                    Some((x0, x1, y0, y1)) => println!(
                        "  align={align:?} cap={name:<16} w={:<4} mid-column thickness/w={} centre y/w={}   ink x/w from line start={}..{} (line end at {}), ink y/w={}..{}",
                        r3(w),
                        r3(thick / w),
                        r3(centre / w),
                        r3((x0 as f32 - AT.x) / w),
                        r3((x1 as f32 - AT.x) / w),
                        r3(LEN / w),
                        r3((y0 as f32 - AT.y) / w),
                        r3((y1 as f32 - AT.y) / w)
                    ),
                    None => println!("  align={align:?} cap={name:<16} w={:<4} NO INK", r3(w)),
                }
            }
        }
    }
}
