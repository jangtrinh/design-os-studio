//! Outline stroke on a node turns its stroke into filled geometry that covers what the canvas strokes.
//! A rectangle-like shape with a dashed inside or outside stroke is drawn along its inset or outset
//! outline at the stroke's own weight; every other shape keeps the stroke on its boundary. Each test
//! renders the node with its stroke and a copy that holds only the outlined geometry as a fill, and
//! compares the two pictures.

use designos_doc::*;
use designos_render::render_node;
use designos_render::tiny_skia::Pixmap;
use designos_render::vectorops::{node_paths, outline_node_stroke, outline_stroke, paths_area};

const SCALE: f32 = 2.0;
const INK: [u8; 3] = [200, 30, 30];

fn ink() -> Paint {
    Paint::solid(Color::rgb(INK[0], INK[1], INK[2]))
}

fn stroke(weight: f32, align: StrokeAlign, cap: StrokeCap, dashes: &[f32]) -> Stroke {
    Stroke { paints: vec![ink()], weight, align, cap, dashes: dashes.to_vec(), miter_limit: 4.0, ..Default::default() }
}

/// A white 200 x 120 frame holding `shape`; returns the document, the frame and the shape ids.
fn framed(shape: impl FnOnce(NodeId) -> Node) -> (Document, NodeId, NodeId) {
    let mut doc = Document::new("t");
    let page = doc.add_page("p");
    let frame = doc.alloc_id();
    doc.insert(page, Node::frame(frame, Rect::new(0.0, 0.0, 200.0, 120.0))).unwrap();
    let id = doc.alloc_id();
    doc.insert(frame, shape(id)).unwrap();
    (doc, frame, id)
}

fn rectangle(id: NodeId, radius: f32, stroke: Stroke) -> Node {
    let mut n = Node::rectangle(id, Rect::new(30.0, 30.0, 100.0, 60.0));
    n.fills.clear();
    n.corner_radii = [radius; 4];
    n.stroke = stroke;
    n
}

/// The frame as the canvas draws it, and as drawn after the shape's stroke became a filled vector.
fn canvas_and_outlined(doc: &Document, frame: NodeId, shape: NodeId) -> (Pixmap, Pixmap) {
    let canvas = render_node(doc, frame, SCALE).expect("the frame renders");
    let n = doc.get(shape).unwrap();
    let outline = outline_node_stroke(doc, n);
    assert!(!outline.is_empty(), "the stroke has an outline");
    let mut after = doc.clone();
    after.get_mut(shape).unwrap().stroke.paints.clear();
    let vid = after.alloc_id();
    let mut vector = Node::new(vid, NodeKind::Vector { paths: outline, network: None });
    vector.size = n.size;
    vector.transform = n.transform;
    vector.fills = n.stroke.paints.clone();
    after.insert(frame, vector).unwrap();
    (canvas, render_node(&after, frame, SCALE).expect("the outlined frame renders"))
}

fn spread(a: &Pixmap, b: &Pixmap, x: u32, y: u32) -> u8 {
    let (pa, pb) = (a.pixel(x, y).unwrap(), b.pixel(x, y).unwrap());
    [pa.red().abs_diff(pb.red()), pa.green().abs_diff(pb.green()), pa.blue().abs_diff(pb.blue())].into_iter().max().unwrap()
}

/// Pixels where the pictures differ by more than `tolerance`, and the pixel count.
fn differing_pixels(a: &Pixmap, b: &Pixmap, tolerance: u8) -> (usize, usize) {
    assert_eq!((a.width(), a.height()), (b.width(), b.height()));
    let mut bad = 0;
    for y in 0..a.height() {
        for x in 0..a.width() {
            if spread(a, b, x, y) > tolerance {
                bad += 1;
            }
        }
    }
    (bad, (a.width() * a.height()) as usize)
}

/// The colour of the picture at design pixel `(x, y)`.
fn at(pm: &Pixmap, x: f32, y: f32) -> [u8; 3] {
    let p = pm.pixel((x * SCALE) as u32, (y * SCALE) as u32).unwrap();
    [p.red(), p.green(), p.blue()]
}

fn assert_ink(label: &str, pm: &Pixmap, p: (f32, f32)) {
    let got = at(pm, p.0, p.1);
    assert!(got.iter().zip(INK).all(|(g, w)| g.abs_diff(w) <= 3), "{label}: expected ink at {p:?}, found {got:?}");
}

fn assert_white(label: &str, pm: &Pixmap, p: (f32, f32)) {
    let got = at(pm, p.0, p.1);
    assert!(got.iter().all(|g| *g >= 252), "{label}: expected the frame's white at {p:?}, found {got:?}");
}

/// A dashed inside or outside stroke on a rectangle outlines to the pixels the canvas strokes. Straight
/// corners must agree to the pixel; with round corners the canvas itself places dashes a little differently
/// at every zoom, so a few dash ends may sit a fraction of a pixel apart.
#[test]
fn outline_of_a_dashed_inside_or_outside_rectangle_covers_what_the_canvas_strokes() {
    // (align, cap, weight, dashes, corner radius, a point in the middle of the first dash, allowed share of differing pixels)
    let cases = [
        (StrokeAlign::Inside, StrokeCap::Round, 8.0, [10.0, 14.0], 0.0, (39.0, 34.0), 0.001),
        (StrokeAlign::Outside, StrokeCap::Round, 8.0, [10.0, 14.0], 0.0, (31.0, 26.0), 0.001),
        (StrokeAlign::Outside, StrokeCap::None, 6.0, [9.0, 7.0], 12.0, (46.0, 27.0), 0.01),
    ];
    let mut failures = Vec::new();
    for (align, cap, weight, dashes, radius, first_dash, allowed) in cases {
        let label = format!("{align:?} {cap:?} dashed rectangle");
        let (doc, frame, id) = framed(|id| rectangle(id, radius, stroke(weight, align, cap, &dashes)));
        let (canvas, outlined) = canvas_and_outlined(&doc, frame, id);
        assert_ink(&format!("{label}, canvas"), &canvas, first_dash);
        assert_ink(&format!("{label}, outline"), &outlined, first_dash);
        let (bad, total) = differing_pixels(&canvas, &outlined, 24);
        if bad as f64 > total as f64 * allowed {
            failures.push(format!("{label}: {bad} of {total} pixels differ from the canvas"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// The first dash of an inside round-capped stroke ends in a cap of the stroke's own width, and the gap
/// after it stays open: a boundary stroke at twice the weight would have its caps run into each other.
#[test]
fn outline_of_a_dashed_inside_rectangle_keeps_the_cap_size_and_the_gap() {
    let (doc, frame, id) = framed(|id| rectangle(id, 0.0, stroke(8.0, StrokeAlign::Inside, StrokeCap::Round, &[10.0, 14.0])));
    let (canvas, outlined) = canvas_and_outlined(&doc, frame, id);
    for (label, pm) in [("canvas", &canvas), ("outline", &outlined)] {
        assert_ink(&format!("{label}: a dash"), pm, (39.0, 34.0));
        assert_ink(&format!("{label}: its round cap"), pm, (46.5, 34.0));
        assert_white(&format!("{label}: the gap"), pm, (51.0, 34.0));
    }
}

/// Where the corner radius is smaller than half the weight the inset outline has a square corner, and a
/// mitered dash that turns there points past the rounded boundary. The canvas clips that tip, so must the outline.
#[test]
fn outline_of_a_dashed_inside_stroke_is_clipped_at_a_tight_rounded_corner() {
    let miter = Stroke { join: StrokeJoin::Miter, ..stroke(8.0, StrokeAlign::Inside, StrokeCap::None, &[100.0, 20.0]) };
    let (doc, frame, id) = framed(|id| rectangle(id, 2.0, miter));
    let (canvas, outlined) = canvas_and_outlined(&doc, frame, id);
    // the first dash turns the top right corner; the top left corner (30, 30) is where the tip of the last dash points
    for (label, pm) in [("canvas", &canvas), ("outline", &outlined)] {
        assert_ink(&format!("{label}: the dash along the top edge"), pm, (60.0, 34.0));
        assert_white(&format!("{label}: outside the rounded corner"), pm, (130.0 - 0.3, 30.3));
    }
    let (bad, total) = differing_pixels(&canvas, &outlined, 24);
    assert!(bad as f64 <= total as f64 * 0.001, "{bad} of {total} pixels differ from the canvas");
}

/// A stroke that is not a dashed inside or outside rectangle is expanded exactly as before.
#[test]
fn other_strokes_outline_as_the_geometry_stroke() {
    for (label, stroke) in [
        ("solid inside", stroke(10.0, StrokeAlign::Inside, StrokeCap::Round, &[])),
        ("solid outside", stroke(10.0, StrokeAlign::Outside, StrokeCap::None, &[])),
        ("dashed centered", stroke(8.0, StrokeAlign::Center, StrokeCap::Round, &[10.0, 14.0])),
    ] {
        let (doc, _frame, id) = framed(|id| rectangle(id, 6.0, stroke.clone()));
        let n = doc.get(id).unwrap();
        let paths = node_paths(&doc, n).expect("a rectangle has geometry");
        assert_eq!(outline_node_stroke(&doc, n), outline_stroke(&paths, &n.stroke, None), "{label}");
    }
    let (doc, _frame, id) = framed(|id| rectangle(id, 0.0, stroke(10.0, StrokeAlign::Inside, StrokeCap::None, &[])));
    let area = paths_area(&outline_node_stroke(&doc, doc.get(id).unwrap()), 0.02);
    assert!((area - (100.0 * 60.0 - 80.0 * 40.0)).abs() <= 1.0, "a solid inside stroke of 10 on 100 x 60 covers 2800, got {area}");
}

/// A vector that happens to be a rectangle is not a rectangle layer: the canvas strokes its boundary at twice
/// the weight, so its outline must too.
#[test]
fn a_rectangle_shaped_vector_keeps_the_doubled_boundary_stroke() {
    let rect_path = Path {
        cmds: vec![
            PathCmd::MoveTo(Vec2::new(0.0, 0.0)),
            PathCmd::LineTo(Vec2::new(100.0, 0.0)),
            PathCmd::LineTo(Vec2::new(100.0, 60.0)),
            PathCmd::LineTo(Vec2::new(0.0, 60.0)),
            PathCmd::Close,
        ],
        fill_rule: FillRule::NonZero,
    };
    for align in [StrokeAlign::Inside, StrokeAlign::Outside] {
        let (doc, frame, id) = framed(|id| {
            let mut n = Node::new(id, NodeKind::Vector { paths: vec![rect_path.clone()], network: None });
            n.size = Vec2::new(100.0, 60.0);
            n.transform = Affine::translate(30.0, 30.0);
            n.stroke = stroke(8.0, align, StrokeCap::Round, &[10.0, 14.0]);
            n
        });
        let n = doc.get(id).unwrap();
        let paths = node_paths(&doc, n).expect("a vector has geometry");
        assert_eq!(outline_node_stroke(&doc, n), outline_stroke(&paths, &n.stroke, None), "{align:?}: the geometry stroke, unchanged");
        let (canvas, outlined) = canvas_and_outlined(&doc, frame, id);
        let (bad, total) = differing_pixels(&canvas, &outlined, 24);
        assert!(bad * 1000 <= total, "{align:?} dashed vector: {bad} of {total} pixels differ from the canvas");
    }
}
