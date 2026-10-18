//! A mask that is a frame (or instance) with children takes its alpha from the
//! fills of the frame plus its drawn subtree. Only a childless shape with no
//! visible fill stands in as an opaque stencil.

use designos_doc::*;
use designos_render::{render_page, tiny_skia::Pixmap, Viewport};

const WHITE: [u8; 4] = [255, 255, 255, 255];
const RED: Color = Color { r: 1.0, g: 0.0, b: 0.0, a: 1.0 };
const BLUE: Color = Color { r: 0.0, g: 0.0, b: 1.0, a: 1.0 };

struct T {
    doc: Document,
    page: NodeId,
}

impl T {
    fn new() -> Self {
        let mut doc = Document::new("mask without own fill");
        let page = doc.add_page("p");
        doc.get_mut(page).unwrap().kind = NodeKind::Page { background: Color::WHITE };
        Self { doc, page }
    }

    fn add(&mut self, parent: Option<NodeId>, f: impl FnOnce(NodeId) -> Node) -> NodeId {
        let id = self.doc.alloc_id();
        self.doc.insert(parent.unwrap_or(self.page), f(id)).unwrap()
    }

    fn rect(&mut self, parent: Option<NodeId>, r: Rect, c: Color) -> NodeId {
        self.add(parent, |id| {
            let mut n = Node::rectangle(id, r);
            n.fills = vec![Paint::solid(c)];
            n
        })
    }

    /// A mask frame with no fills of its own.
    fn mask_frame(&mut self, r: Rect) -> NodeId {
        self.add(None, |id| {
            let mut n = Node::frame(id, r);
            n.fills.clear();
            n.is_mask = true;
            n
        })
    }

    fn render(&self, size: u32) -> Pixmap {
        let mut pm = Pixmap::new(size, size).unwrap();
        render_page(&self.doc, self.page, &Viewport::default(), &mut pm);
        pm
    }
}

fn px(pm: &Pixmap, x: u32, y: u32) -> [u8; 4] {
    let p = pm.pixel(x, y).unwrap();
    [p.red(), p.green(), p.blue(), p.alpha()]
}

#[test]
fn frame_mask_without_fill_uses_the_alpha_of_its_children() {
    let mut t = T::new();
    let m = t.mask_frame(Rect::new(0.0, 0.0, 80.0, 80.0));
    // the stencil child covers only the top-left quarter of the mask frame
    t.rect(Some(m), Rect::new(0.0, 0.0, 40.0, 40.0), BLUE);
    t.rect(None, Rect::new(0.0, 0.0, 80.0, 80.0), RED);
    let pm = t.render(80);
    assert_eq!(px(&pm, 20, 20), [255, 0, 0, 255], "inside the child of the mask");
    assert_eq!(px(&pm, 60, 60), WHITE, "inside the mask frame but outside its child: not an opaque box");
    assert_eq!(px(&pm, 60, 20), WHITE, "right of the child");
}

#[test]
fn frame_mask_without_fill_keeps_partial_alpha_of_children() {
    let mut t = T::new();
    let m = t.mask_frame(Rect::new(0.0, 0.0, 80.0, 80.0));
    let half = Color { a: 0.5, ..BLUE };
    t.rect(Some(m), Rect::new(0.0, 0.0, 80.0, 80.0), half);
    t.rect(None, Rect::new(0.0, 0.0, 80.0, 80.0), RED);
    let pm = t.render(80);
    let p = px(&pm, 40, 40);
    assert!((p[1] as i32 - 128).abs() <= 2 && (p[2] as i32 - 128).abs() <= 2 && p[0] == 255, "red at half alpha over white, got {p:?}");
}

#[test]
fn frame_mask_with_children_still_adds_its_own_fill() {
    let mut t = T::new();
    let m = t.mask_frame(Rect::new(0.0, 0.0, 80.0, 80.0));
    t.node(m).fills = vec![Paint::solid(BLUE)];
    t.rect(Some(m), Rect::new(0.0, 0.0, 10.0, 10.0), BLUE);
    t.rect(None, Rect::new(0.0, 0.0, 80.0, 80.0), RED);
    let pm = t.render(80);
    assert_eq!(px(&pm, 60, 60), [255, 0, 0, 255], "the frame's own fill is part of the mask");
}

#[test]
fn childless_shape_without_fill_still_masks_as_an_opaque_stencil() {
    let mut t = T::new();
    let m = t.add(None, |id| Node::ellipse(id, Rect::new(0.0, 0.0, 80.0, 80.0)));
    t.node(m).fills.clear();
    t.node(m).is_mask = true;
    t.rect(None, Rect::new(0.0, 0.0, 80.0, 80.0), RED);
    let pm = t.render(80);
    assert_eq!(px(&pm, 40, 40), [255, 0, 0, 255], "inside the stencil");
    assert_eq!(px(&pm, 2, 2), WHITE, "outside the ellipse");
}

#[test]
fn childless_frame_without_fill_masks_as_an_opaque_box() {
    let mut t = T::new();
    t.mask_frame(Rect::new(0.0, 0.0, 40.0, 40.0));
    t.rect(None, Rect::new(0.0, 0.0, 80.0, 80.0), RED);
    let pm = t.render(80);
    assert_eq!(px(&pm, 20, 20), [255, 0, 0, 255], "inside the box");
    assert_eq!(px(&pm, 60, 60), WHITE, "outside the box");
}

#[test]
fn mask_frame_box_matches_alpha_only_from_children_rule() {
    let mut frame = Node::frame(NodeId(1), Rect::new(0.0, 0.0, 40.0, 40.0));
    frame.fills.clear();
    assert!(designos_render::mask_frame_box(&frame).is_none(), "childless frame gets its own fill from mask_stencil");

    frame.children = vec![NodeId(2)];
    let stand_in = designos_render::mask_frame_box(&frame).expect("fill-less frame with children has a stand-in box");
    assert!(stand_in.children.is_empty());
    assert_eq!(stand_in.fills, vec![Paint::solid(Color::BLACK)]);

    frame.fills = vec![Paint::solid(BLUE)];
    assert!(designos_render::mask_frame_box(&frame).is_none(), "frame with a visible fill already has its own alpha");

    let mut group = Node::new(NodeId(3), NodeKind::Group);
    group.children = vec![NodeId(4)];
    assert!(designos_render::mask_frame_box(&group).is_none(), "group has no box of its own");
}

impl T {
    fn node(&mut self, id: NodeId) -> &mut Node {
        self.doc.get_mut(id).unwrap()
    }
}
