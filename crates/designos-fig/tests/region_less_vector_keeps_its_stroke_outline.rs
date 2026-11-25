//! A closed vector the file stores without regions and without a fill carries Figma's stroke outline as its geometry
//! (`strokeGeometry`, with no `fillGeometry`). The loop is still what a fill covers and what an outside or inside stroke
//! is clipped by, so the outline must never be taken for the fill: an outside stroke stays, an inside stroke is not
//! doubled, and a fill added later covers the inside of the loop.

use designos_doc::*;
use designos_fig::import_rest_json;
use designos_render::tiny_skia::Pixmap;
use designos_render::{render_page, Viewport};
use serde_json::{json, Value};

const CENTRE: f32 = 40.0;
const RADIUS: f32 = 20.0;
const WEIGHT: f32 = 4.0;

fn circle_path(r: f32, reversed: bool) -> String {
    let k = 0.552_284_8 * r;
    // the node is 60 x 60: the circle's centre is (30, 30) in node space
    let (cx, cy) = (30.0, 30.0);
    if reversed {
        format!(
            "M{} {} C{} {} {} {} {} {} C{} {} {} {} {} {} C{} {} {} {} {} {} C{} {} {} {} {} {} Z",
            cx + r,
            cy,
            cx + r,
            cy - k,
            cx + k,
            cy - r,
            cx,
            cy - r,
            cx - k,
            cy - r,
            cx - r,
            cy - k,
            cx - r,
            cy,
            cx - r,
            cy + k,
            cx - k,
            cy + r,
            cx,
            cy + r,
            cx + k,
            cy + r,
            cx + r,
            cy + k,
            cx + r,
            cy
        )
    } else {
        format!(
            "M{} {} C{} {} {} {} {} {} C{} {} {} {} {} {} C{} {} {} {} {} {} C{} {} {} {} {} {} Z",
            cx + r,
            cy,
            cx + r,
            cy + k,
            cx + k,
            cy + r,
            cx,
            cy + r,
            cx - k,
            cy + r,
            cx - r,
            cy + k,
            cx - r,
            cy,
            cx - r,
            cy - k,
            cx - k,
            cy - r,
            cx,
            cy - r,
            cx + k,
            cy - r,
            cx + r,
            cy - k,
            cx + r,
            cy
        )
    }
}

/// A 60 x 60 vector at page (10, 10): a circle of radius 20 as four bezier segments, no regions, a stroke of weight 4 at
/// half opacity aligned `align`, Figma's stored stroke band (twice the weight, centred on the circle) as `strokeGeometry`
/// and no `fillGeometry`; `fills` as given.
fn circle_file(align: &str, fills: Value) -> String {
    let k = 0.552_284_8 * RADIUS;
    let seg = |a: usize, b: usize, ta: (f32, f32), tb: (f32, f32)| json!({"start": a, "end": b, "tangentStart": {"x": ta.0, "y": ta.1}, "tangentEnd": {"x": tb.0, "y": tb.1}});
    let (cx, cy) = (30.0, 30.0);
    let band = format!("{} {}", circle_path(RADIUS + WEIGHT, false), circle_path(RADIUS - WEIGHT, true));
    json!({
        "name": "icons",
        "document": {"id": "0:0", "type": "DOCUMENT", "children": [
            {"id": "0:1", "name": "Page", "type": "CANVAS", "children": [{
                "id": "1:1", "name": "Icon", "type": "VECTOR",
                "size": {"x": 60.0, "y": 60.0},
                "relativeTransform": [[1, 0, 10], [0, 1, 10]],
                "fills": fills,
                "strokes": [{"type": "SOLID", "color": {"r": 0.0, "g": 0.0, "b": 1.0, "a": 1.0}, "opacity": 0.5}],
                "strokeWeight": WEIGHT, "strokeAlign": align, "strokeCap": "NONE", "strokeJoin": "MITER",
                "vectorNetwork": {
                    "vertices": [{"x": cx + RADIUS, "y": cy}, {"x": cx, "y": cy + RADIUS}, {"x": cx - RADIUS, "y": cy}, {"x": cx, "y": cy - RADIUS}],
                    "segments": [seg(0, 1, (0.0, k), (k, 0.0)), seg(1, 2, (-k, 0.0), (0.0, k)), seg(2, 3, (0.0, -k), (-k, 0.0)), seg(3, 0, (k, 0.0), (0.0, -k))],
                    "regions": []
                },
                "strokeGeometry": [{"path": band, "windingRule": "NONZERO"}]
            }]}
        ]}
    })
    .to_string()
}

/// The page at 4x: pixel of the point at `r` from the circle's centre (page (40, 40)) along +x.
fn picture(json: &str) -> impl Fn(f32) -> [u8; 4] {
    let doc = import_rest_json(json).unwrap();
    let page = doc.pages[0];
    let mut pm = Pixmap::new(400, 400).unwrap();
    render_page(&doc, page, &Viewport { zoom: 4.0, ..Viewport::default() }, &mut pm);
    move |r| {
        let p = pm.pixel(((CENTRE + r) * 4.0) as u32, (CENTRE * 4.0) as u32).unwrap();
        [p.red(), p.green(), p.blue(), p.alpha()]
    }
}

#[test]
fn an_outside_stroke_of_a_region_less_closed_vector_stays_outside_the_loop() {
    let at = picture(&circle_file("OUTSIDE", json!([])));
    assert_ne!(at(RADIUS + 2.0), at(45.0), "the stroke band outside the circle is drawn");
    assert_eq!(at(RADIUS - 2.0), at(45.0), "nothing inside the circle");
    assert_eq!(at(0.0), at(45.0));
    assert_eq!(at(RADIUS + 6.0), at(45.0), "nothing beyond the band");
}

#[test]
fn an_inside_stroke_of_a_region_less_closed_vector_is_drawn_once() {
    let inside = picture(&circle_file("INSIDE", json!([])));
    let outside = picture(&circle_file("OUTSIDE", json!([])));
    assert_eq!(inside(RADIUS + 2.0), inside(45.0), "nothing outside the circle");
    assert_ne!(inside(RADIUS - 2.0), inside(45.0), "the band inside the circle is drawn");
    assert_eq!(inside(RADIUS - 2.0), outside(RADIUS + 2.0), "one pass of the half-opacity stroke, not two");
    assert_eq!(inside(0.0), inside(45.0));
}

#[test]
fn a_fill_added_to_a_region_less_closed_vector_covers_the_inside_of_the_loop_not_the_stored_outline() {
    let fills = json!([{"type": "SOLID", "color": {"r": 1.0, "g": 0.0, "b": 0.0, "a": 1.0}}]);
    let at = picture(&circle_file("INSIDE", fills));
    let red = [255, 0, 0, 255];
    assert_eq!(at(0.0), red, "the fill covers the middle of the loop");
    assert_eq!(at(RADIUS - 8.0), red, "and everything inside it");
    assert_eq!(at(RADIUS + 2.0), at(45.0), "and nothing outside it");
}
