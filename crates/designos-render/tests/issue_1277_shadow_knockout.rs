//! Regression fixtures for #1277: fill-less frame shadow knockout must follow
//! the pixels the children actually paint.

use designos_doc::*;
use designos_render::{render_page, tiny_skia::Pixmap, Viewport};

struct Fixture {
    doc: Document,
    page: NodeId,
}

impl Fixture {
    fn new() -> Self {
        let mut doc = Document::new("issue-1277");
        let page = doc.add_page("p");
        doc.get_mut(page).unwrap().kind = NodeKind::Page { background: Color::TRANSPARENT };
        Self { doc, page }
    }

    fn add(&mut self, parent: NodeId, make: impl FnOnce(NodeId) -> Node) -> NodeId {
        let id = self.doc.alloc_id();
        self.doc.insert(parent, make(id)).unwrap()
    }

    fn render(&self) -> Pixmap {
        let mut pm = Pixmap::new(280, 220).unwrap();
        render_page(&self.doc, self.page, &Viewport::default(), &mut pm);
        pm
    }
}

fn shadow() -> Effect {
    Effect::DropShadow {
        color: Color { r: 0.0, g: 0.0, b: 0.0, a: 0.7 },
        offset: Vec2::new(0.0, 18.0),
        radius: 18.0,
        spread: 0.0,
        visible: true,
        show_behind: false,
        blend: Default::default(),
    }
}

fn fillless_frame(f: &mut Fixture) -> NodeId {
    f.add(f.page, |id| {
        let mut n = Node::frame(id, Rect::new(20.0, 20.0, 220.0, 150.0));
        n.fills.clear();
        n.effects = vec![shadow()];
        n
    })
}

fn pixel(pm: &Pixmap, x: u32, y: u32) -> [u8; 4] {
    let p = pm.pixel(x, y).unwrap();
    [p.red(), p.green(), p.blue(), p.alpha()]
}

#[test]
fn mask_knockout_preserves_shadow_outside_masked_sibling() {
    let mut f = Fixture::new();
    let frame = fillless_frame(&mut f);
    f.add(frame, |id| {
        let mut n = Node::ellipse(id, Rect::new(50.0, 40.0, 80.0, 80.0));
        n.is_mask = true;
        n
    });
    f.add(frame, |id| {
        let mut n = Node::rectangle(id, Rect::new(50.0, 40.0, 140.0, 50.0));
        n.fills = vec![Paint::solid(Color::WHITE)];
        n
    });

    let pm = f.render();
    // The rectangle is clipped by the ellipse when it builds the shadow. Its
    // blurred right edge must therefore remain visible where the unmasked
    // rectangle would otherwise erase it during knockout.
    assert!(pixel(&pm, 160, 100)[3] > 0, "masked shadow outside ellipse was fully knocked out: {:?}", pixel(&pm, 160, 100));
}

#[test]
fn group_knockout_ignores_child_paint_alpha() {
    let mut f = Fixture::new();
    let frame = fillless_frame(&mut f);
    let group = f.add(frame, |id| {
        let mut n = Node::new(id, NodeKind::Group);
        n.transform = Affine::translate(70.0, 55.0);
        n.size = Vec2::new(100.0, 50.0);
        n
    });
    f.add(group, |id| {
        let mut n = Node::rectangle(id, Rect::new(0.0, 0.0, 100.0, 50.0));
        n.fills = vec![Paint::solid(Color { a: 0.5, ..Color::WHITE })];
        n
    });

    let pm = f.render();
    // A group is a non-fillable container. In knockout mode its painted child
    // contributes an opaque footprint, even when that child's paint is 50%.
    let p = pixel(&pm, 120, 120);
    assert!(p[0] >= 125 && p[0] <= 131 && p[3] <= 130, "group knockout left shadow under the child: {p:?}");
}

#[test]
fn paint_free_child_does_not_knock_out_shadow() {
    let mut f = Fixture::new();
    let frame = fillless_frame(&mut f);
    f.add(frame, |id| Node::rectangle(id, Rect::new(20.0, 50.0, 100.0, 70.0)));
    f.add(frame, |id| {
        let mut n = Node::frame(id, Rect::new(120.0, 50.0, 80.0, 70.0));
        n.fills.clear();
        n
    });

    let pm = f.render();
    // x=150 is inside the paint-free frame but just outside the painted sibling.
    // Its shadow blur still reaches the point, so a rectangle-shaped knockout
    // from the empty frame would incorrectly erase this pixel.
    assert!(pixel(&pm, 150, 100)[3] > 0, "paint-free child erased its whole frame: {:?}", pixel(&pm, 150, 100));
}
