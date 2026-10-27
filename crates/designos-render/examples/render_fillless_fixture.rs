//! Saves the CPU render of the #520 fixture frames (`520-spec.md` section 2) as PNG:
//! `cargo run -p designos-render --example render_fillless_fixture -- <out-dir>`
//! writes `render-frame1.png` (white fill) and `render-frame2.png` (no fill, rule F2).

use designos_doc::*;
use designos_render::{render_page, tiny_skia::Pixmap, Viewport};

fn render(white: bool) -> Pixmap {
    let mut doc = Document::new("fixture");
    let page = doc.add_page("p");
    doc.get_mut(page).unwrap().kind = NodeKind::Page { background: Color::WHITE };
    let mut add = |parent: NodeId, f: &dyn Fn(NodeId) -> Node| {
        let id = doc.alloc_id();
        doc.insert(parent, f(id)).unwrap()
    };
    let frame = add(page, &|id| {
        let mut n = Node::frame(id, Rect::new(60.0, 40.0, 240.0, 160.0));
        n.fills = if white { vec![Paint::solid(Color::WHITE)] } else { vec![] };
        n.effects = vec![Effect::DropShadow {
            color: Color { r: 0.0, g: 0.0, b: 0.0, a: 0.5 },
            offset: Vec2::new(0.0, 12.0),
            radius: 24.0,
            spread: 0.0,
            visible: true,
            show_behind: false,
        }];
        n
    });
    let blue = Color::from_hex("#2563EB").unwrap();
    let red = Color { a: 0.5, ..Color::from_hex("#DC2626").unwrap() };
    for (r, c) in [(Rect::new(60.0, 50.0, 120.0, 60.0), blue), (Rect::new(20.0, 110.0, 40.0, 30.0), red)] {
        add(frame, &|id| {
            let mut n = Node::rectangle(id, r);
            n.fills = vec![Paint::solid(c)];
            n
        });
    }
    let mut pm = Pixmap::new(360, 260).unwrap();
    render_page(&doc, page, &Viewport::default(), &mut pm);
    pm
}

fn main() {
    let dir = std::path::PathBuf::from(std::env::args().nth(1).expect("usage: render_fillless_fixture <out-dir>"));
    for (name, white) in [("render-frame1.png", true), ("render-frame2.png", false)] {
        render(white).save_png(dir.join(name)).unwrap();
    }
}
