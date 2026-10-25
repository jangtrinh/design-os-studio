//! Text colour comes from `style.fills` only, as on the GPU canvas and in the .fig export:
//! `node.fills` on a text node never repaints its glyphs on the CPU renderer.

use designos_doc::*;
use designos_render::{render_page, tiny_skia::Pixmap, Viewport};

fn render_text(node_fills: Vec<Paint>) -> Pixmap {
    let mut doc = Document::new("text-fill-source");
    let page = doc.add_page("p");
    doc.get_mut(page).unwrap().kind = NodeKind::Page { background: Color::WHITE };
    let id = doc.alloc_id();
    let style = TextStyle { font_size: 24.0, ..Default::default() };
    let mut node = Node::text(id, Rect::new(4.0, 4.0, 120.0, 32.0), "Hi", style);
    node.fills = node_fills;
    doc.insert(page, node).unwrap();
    let mut pm = Pixmap::new(128, 40).unwrap();
    render_page(&doc, page, &Viewport::default(), &mut pm);
    pm
}

#[test]
fn text_node_fills_do_not_recolour_glyphs() {
    let plain = render_text(Vec::new());
    let with_node_fills = render_text(vec![Paint::solid(Color::rgb(0xE1, 0x1D, 0x48))]);
    assert!(plain.pixels().iter().any(|p| p.red() < 80), "fixture must draw dark glyphs");
    assert!(plain.data() == with_node_fills.data(), "node.fills must not change the CPU text colour");
}
