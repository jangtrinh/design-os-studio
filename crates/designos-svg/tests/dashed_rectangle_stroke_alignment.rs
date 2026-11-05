//! A frame exported to SVG and drawn by resvg must show what the canvas shows for the same frame. A dashed
//! rectangle stroked inside or outside has its dashes, and their caps, on the inset or outset outline at the
//! stroke's own weight, exactly as the canvas draws it.

use designos_doc::*;
use designos_export::{export_svg, SvgOptions};
use tiny_skia::Pixmap;

const SCALE: f32 = 2.0;
const INK: [u8; 3] = [200, 30, 30];

fn stroke(weight: f32, align: StrokeAlign, cap: StrokeCap, dashes: &[f32]) -> Stroke {
    Stroke {
        paints: vec![Paint::solid(Color::rgb(INK[0], INK[1], INK[2]))],
        weight,
        align,
        cap,
        dashes: dashes.to_vec(),
        miter_limit: 4.0,
        ..Default::default()
    }
}

/// A white 200 x 120 frame holding a 100 x 60 rectangle at (30, 30) with `stroke`.
fn framed_rectangle(radius: f32, stroke: Stroke) -> (Document, NodeId) {
    let mut doc = Document::new("t");
    let page = doc.add_page("p");
    let frame = doc.alloc_id();
    doc.insert(page, Node::frame(frame, Rect::new(0.0, 0.0, 200.0, 120.0))).unwrap();
    let id = doc.alloc_id();
    let mut n = Node::rectangle(id, Rect::new(30.0, 30.0, 100.0, 60.0));
    n.fills.clear();
    n.corner_radii = [radius; 4];
    n.stroke = stroke;
    doc.insert(frame, n).unwrap();
    (doc, frame)
}

/// The exported SVG of `frame`, drawn by resvg at `SCALE`.
fn svg_picture(doc: &Document, frame: NodeId) -> Pixmap {
    let svg = export_svg(doc, frame, &SvgOptions { outline_text: false });
    let tree = usvg::Tree::from_str(&svg, &usvg::Options::default()).unwrap_or_else(|e| panic!("the export parses: {e}\n{svg}"));
    let size = tree.size().to_int_size();
    let mut pm = Pixmap::new((size.width() as f32 * SCALE) as u32, (size.height() as f32 * SCALE) as u32).unwrap();
    resvg::render(&tree, tiny_skia::Transform::from_scale(SCALE, SCALE), &mut pm.as_mut());
    pm
}

fn canvas_picture(doc: &Document, frame: NodeId) -> Pixmap {
    designos_render::render_node(doc, frame, SCALE).expect("the frame renders")
}

/// Pixels where the pictures differ by more than `tolerance` (over white), and the pixel count.
fn differing_pixels(a: &Pixmap, b: &Pixmap, tolerance: u8) -> (usize, usize) {
    assert_eq!((a.width(), a.height()), (b.width(), b.height()), "the pictures have the same size");
    let over_white = |p: tiny_skia::PremultipliedColorU8| {
        let k = 255 - u16::from(p.alpha());
        [u16::from(p.red()) + k, u16::from(p.green()) + k, u16::from(p.blue()) + k].map(|v| v.min(255) as u8)
    };
    let mut bad = 0;
    for y in 0..a.height() {
        for x in 0..a.width() {
            let (pa, pb) = (over_white(a.pixel(x, y).unwrap()), over_white(b.pixel(x, y).unwrap()));
            if pa.iter().zip(&pb).map(|(p, q)| p.abs_diff(*q)).max().unwrap() > tolerance {
                bad += 1;
            }
        }
    }
    (bad, (a.width() * a.height()) as usize)
}

fn at(pm: &Pixmap, x: f32, y: f32) -> [u8; 3] {
    let p = pm.pixel((x * SCALE) as u32, (y * SCALE) as u32).unwrap();
    let k = 255 - u16::from(p.alpha());
    [u16::from(p.red()) + k, u16::from(p.green()) + k, u16::from(p.blue()) + k].map(|v| v.min(255) as u8)
}

fn assert_ink(label: &str, pm: &Pixmap, p: (f32, f32)) {
    let got = at(pm, p.0, p.1);
    assert!(got.iter().zip(INK).all(|(g, w)| g.abs_diff(w) <= 3), "{label}: expected ink at {p:?}, found {got:?}");
}

fn assert_white(label: &str, pm: &Pixmap, p: (f32, f32)) {
    let got = at(pm, p.0, p.1);
    assert!(got.iter().all(|g| *g >= 252), "{label}: expected the frame's white at {p:?}, found {got:?}");
}

/// Inside and outside dashed rectangles with round caps: the SVG draws the canvas picture, with the dash,
/// its cap and the open gap after it where the canvas has them.
#[test]
fn dashed_inside_and_outside_rectangles_with_round_caps_match_the_canvas() {
    for (align, first_dash, cap_tip, gap) in
        [(StrokeAlign::Inside, (39.0, 34.0), (46.5, 34.0), (51.0, 34.0)), (StrokeAlign::Outside, (31.0, 26.0), (38.5, 26.0), (43.0, 26.0))]
    {
        let (doc, frame) = framed_rectangle(0.0, stroke(8.0, align, StrokeCap::Round, &[10.0, 14.0]));
        let (canvas, svg) = (canvas_picture(&doc, frame), svg_picture(&doc, frame));
        for (who, pm) in [("canvas", &canvas), ("svg", &svg)] {
            assert_ink(&format!("{align:?} {who}: a dash"), pm, first_dash);
            assert_ink(&format!("{align:?} {who}: its round cap"), pm, cap_tip);
            assert_white(&format!("{align:?} {who}: the gap"), pm, gap);
        }
        let (bad, total) = differing_pixels(&canvas, &svg, 24);
        assert!(bad * 1000 <= total, "{align:?} dashed rectangle: {bad} of {total} pixels differ from the canvas");
    }
}

/// With rounded corners the dash phase along the curves may sit a fraction of a pixel apart between renderers.
#[test]
fn dashed_inside_and_outside_rectangles_with_rounded_corners_match_the_canvas() {
    for (align, cap) in [(StrokeAlign::Inside, StrokeCap::Square), (StrokeAlign::Outside, StrokeCap::Round)] {
        let (doc, frame) = framed_rectangle(12.0, stroke(6.0, align, cap, &[9.0, 7.0]));
        let (canvas, svg) = (canvas_picture(&doc, frame), svg_picture(&doc, frame));
        let (bad, total) = differing_pixels(&canvas, &svg, 24);
        assert!(bad * 100 <= total, "{align:?} {cap:?} dashed rectangle: {bad} of {total} pixels differ from the canvas");
    }
}

/// A solid stroke is exported as it always was and still matches the canvas.
#[test]
fn solid_inside_rectangle_still_matches_the_canvas() {
    let (doc, frame) = framed_rectangle(0.0, stroke(8.0, StrokeAlign::Inside, StrokeCap::Round, &[]));
    let (canvas, svg) = (canvas_picture(&doc, frame), svg_picture(&doc, frame));
    let (bad, total) = differing_pixels(&canvas, &svg, 24);
    assert!(bad * 1000 <= total, "{bad} of {total} pixels differ from the canvas");
}
