//! Exporting board kinds: SVG gets the lowered shapes and text, PNG includes the sticky shadow.

use designos_doc::*;
use designos_export::{export_png, export_svg, render_export, SvgOptions};

fn board() -> (Document, NodeId, NodeId, NodeId, NodeId) {
    let mut d = Document::new("board");
    d.kind = DocumentKind::Board;
    let page = d.add_page("p");
    let a = d.alloc_id();
    let a = d.insert(page, Node::sticky(a, Rect::new(40.0, 40.0, 200.0, 200.0), "Hello board")).unwrap();
    let b = d.alloc_id();
    let b = d.insert(page, Node::shape_with_text(b, Rect::new(400.0, 40.0, 160.0, 100.0), ShapeKind::Diamond, "ok")).unwrap();
    let c = d.alloc_id();
    let mut props = ConnectorProps { start: ConnectorEnd::attached(a, Magnet::Auto), end: ConnectorEnd::attached(b, Magnet::Auto), ..Default::default() };
    props.end_cap = ConnectorCap::Triangle;
    let c = d.insert(page, Node::connector(c, props)).unwrap();
    sync_connector(&mut d, c);
    (d, page, a, b, c)
}

#[test]
fn svg_of_a_board_page_has_notes_shapes_connectors_and_text() {
    let (d, page, ..) = board();
    let svg = export_svg(&d, page, &SvgOptions::default());
    // sticky: rounded rect fill in its colour, with the shadow filter
    assert!(svg.contains("fill=\"#FFE58F\""), "{svg}");
    assert!(svg.contains("<filter"), "sticky shadow");
    assert!(svg.contains("feGaussianBlur"));
    // glyph outlines for the sticky text and the shape text
    assert!(svg.matches("<path").count() >= 6, "{svg}");
    // the connector stroke and its triangle cap
    assert!(svg.contains("stroke=\"#1E1E1E\""));
    // the diamond's polygon outline
    assert!(svg.contains("<path d=\"M"), "{svg}");
    // text mode keeps real text
    let text = export_svg(&d, page, &SvgOptions { outline_text: false });
    assert!(text.contains("<text") && text.contains("Hello") && text.contains("board"), "{text}");
}

#[test]
fn svg_of_a_single_sticky_covers_its_shadow() {
    let (d, _page, a, ..) = board();
    let svg = export_svg(&d, a, &SvgOptions::default());
    // viewBox grows beyond the 200x200 note to take the shadow
    let vb = svg.split("viewBox=\"").nth(1).unwrap().split('"').next().unwrap();
    let nums: Vec<f32> = vb.split(' ').map(|s| s.parse().unwrap()).collect();
    assert!(nums[0] < 0.0 && nums[1] < 0.0 && nums[2] > 200.0 && nums[3] > 200.0, "{vb}");
}

#[test]
fn png_of_board_nodes() {
    let (d, page, a, _b, c) = board();
    let png = export_png(&d, a, 1.0);
    assert!(png.starts_with(&[0x89, b'P', b'N', b'G']));
    let pm = render_export(&d, a, 1.0).unwrap();
    assert!(pm.width() > 200 && pm.height() > 200, "grown by the shadow: {}x{}", pm.width(), pm.height());
    // the connector exports on its own, too
    let pm = render_export(&d, c, 2.0).unwrap();
    assert!(pm.width() > 100);
    // and the whole page
    let pm = render_export(&d, page, 1.0).unwrap();
    assert!(pm.width() > 500);
}
