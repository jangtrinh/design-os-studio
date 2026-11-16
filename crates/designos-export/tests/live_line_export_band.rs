use designos_doc::*;
use designos_export::{export_svg, SvgOptions};

fn line_doc(len: f32, w: f32, align: StrokeAlign, cap: StrokeCap) -> (Document, NodeId) {
    let mut doc = Document::new("t");
    let page = doc.add_page("p");
    let id = doc.alloc_id();
    let mut n = Node::new(id, NodeKind::Line);
    n.size = Vec2::new(len, 0.0);
    n.fills.clear();
    n.stroke = Stroke { paints: vec![Paint::solid(Color::BLACK)], weight: w, align, cap, ..Default::default() };
    let id = doc.insert(page, n).unwrap();
    (doc, id)
}

fn svg(len: f32, w: f32, align: StrokeAlign, cap: StrokeCap) -> String {
    let (doc, id) = line_doc(len, w, align, cap);
    export_svg(&doc, id, &SvgOptions::default())
}

#[test]
fn a_centred_line_exports_its_band_at_y_minus_weight_to_zero() {
    let s = svg(100.0, 10.0, StrokeAlign::Center, StrokeCap::None);
    assert!(s.contains("d=\"M0 -5 L100 -5\""), "{s}");
    // the viewBox covers the whole band [-10, 0] and nothing below the line
    assert!(s.contains("viewBox=\"-5 -10 110 10\""), "{s}");
}

#[test]
fn a_round_cap_line_exports_its_centreline_inset_by_half_the_weight() {
    let s = svg(100.0, 10.0, StrokeAlign::Center, StrokeCap::Round);
    assert!(s.contains("d=\"M5 -5 L95 -5\"") && s.contains("stroke-linecap=\"round\""), "{s}");
}

#[test]
fn an_inside_line_exports_unchanged() {
    let s = svg(100.0, 10.0, StrokeAlign::Inside, StrokeCap::None);
    assert!(s.contains("d=\"M0 0 L100 0\""), "{s}");
}
