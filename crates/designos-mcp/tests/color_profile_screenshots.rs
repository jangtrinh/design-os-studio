//! A node screenshot and a page screenshot of one Display P3 document show the same colours: both
//! convert to sRGB, so an agent checking its work reads what the canvas shows.

use designos_doc::*;
use designos_mcp::tools::screenshot::{render_target, ScreenshotTarget};

fn p3_doc() -> (Document, NodeId, NodeId) {
    let mut doc = Document::new("t");
    let page = doc.add_page("p");
    doc.get_mut(page).unwrap().kind = NodeKind::Page { background: Color::WHITE };
    let id = doc.alloc_id();
    let mut n = Node::rectangle(id, Rect::new(0.0, 0.0, 40.0, 40.0));
    n.fills = vec![Paint::solid(Color { r: 0.5, g: 0.4, b: 0.8, a: 1.0 })];
    let id = doc.insert(page, n).unwrap();
    doc.color_profile = ColorProfile::DisplayP3;
    (doc, page, id)
}

fn centre(img: &designos_mcp::tools::screenshot::RenderedImage) -> [u8; 3] {
    let p = img.pixmap.pixel(img.width / 2, img.height / 2).unwrap().demultiply();
    [p.red(), p.green(), p.blue()]
}

#[test]
fn node_and_page_screenshots_of_a_display_p3_document_agree() {
    let (doc, page, id) = p3_doc();
    let node = render_target(&doc, ScreenshotTarget::Node(id), 1.0, 0).unwrap();
    let whole = render_target(&doc, ScreenshotTarget::Node(page), 1.0, 0).unwrap();
    let (a, b) = (centre(&node), centre(&whole));
    assert_eq!(a, b, "node {a:?} vs page {b:?}");
    assert!((a[0] as i32 - 132).abs() <= 1 && (a[1] as i32 - 101).abs() <= 1 && (a[2] as i32 - 211).abs() <= 1, "{a:?}");
}
