//! A standalone full ring (an ellipse with an inner radius of 100%) has no fill area, but its stroke is
//! still aligned in SVG like the canvas does: an Inside stroke is clipped to the ring and drawn at twice
//! the weight, an Outside stroke is masked and drawn at twice the weight. It must read the same as an
//! ordinary donut, not fall back to a centered stroke.

use designos_doc::*;
use designos_export::{export_svg, SvgOptions};

fn ellipse_svg(inner_radius: f32, align: StrokeAlign) -> String {
    let mut doc = Document::new("t");
    let page = doc.add_page("p");
    let id = doc.alloc_id();
    let mut n = Node::new(id, NodeKind::Ellipse(ArcData { start: 0.0, end: std::f32::consts::TAU, inner_radius }));
    n.size = Vec2::new(100.0, 100.0);
    n.stroke = Stroke { paints: vec![Paint::solid(Color::BLACK)], weight: 10.0, align, ..Default::default() };
    let id = doc.insert(page, n).unwrap();
    export_svg(&doc, id, &SvgOptions::default())
}

/// Markup that marks an aligned stroke: the wrapper group and the doubled stroke width.
fn alignment_markers(svg: &str, align: StrokeAlign) -> (bool, bool) {
    let wrapper = match align {
        StrokeAlign::Inside => svg.contains("<g clip-path=\"url(#"),
        StrokeAlign::Outside => svg.contains("<g mask=\"url(#"),
        StrokeAlign::Center => false,
    };
    (wrapper, svg.contains("stroke-width=\"20\""))
}

fn assert_ring_matches_donut(align: StrokeAlign) {
    let donut = ellipse_svg(0.5, align);
    assert_eq!(alignment_markers(&donut, align), (true, true), "reference donut is not aligned:\n{donut}");
    let ring = ellipse_svg(1.0, align);
    assert_eq!(alignment_markers(&ring, align), (true, true), "a full ring must keep its {align:?} stroke aligned:\n{ring}");
}

#[test]
fn a_full_ring_keeps_its_outside_stroke_aligned() {
    assert_ring_matches_donut(StrokeAlign::Outside);
}

#[test]
fn a_full_ring_keeps_its_inside_stroke_aligned() {
    assert_ring_matches_donut(StrokeAlign::Inside);
}
