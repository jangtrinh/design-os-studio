//! A Fill-sized vector laid out headless keeps its path in step with its box, the way the editor's
//! layout does, and kinds that carry no path are laid out exactly as before.

use designos_doc::*;

fn square(x0: f32, y0: f32, x1: f32, y1: f32) -> Path {
    Path {
        cmds: vec![
            PathCmd::MoveTo(Vec2::new(x0, y0)),
            PathCmd::LineTo(Vec2::new(x1, y0)),
            PathCmd::LineTo(Vec2::new(x1, y1)),
            PathCmd::LineTo(Vec2::new(x0, y1)),
            PathCmd::Close,
        ],
        fill_rule: FillRule::NonZero,
    }
}

fn bounds(paths: &[Path]) -> (f32, f32, f32, f32) {
    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for cmd in paths.iter().flat_map(|p| &p.cmds) {
        if let PathCmd::MoveTo(p) | PathCmd::LineTo(p) = *cmd {
            (x0, y0, x1, y1) = (x0.min(p.x), y0.min(p.y), x1.max(p.x), y1.max(p.y));
        }
    }
    (x0, y0, x1, y1)
}

struct Scene {
    doc: Document,
    frame: NodeId,
    vector: NodeId,
    rect: NodeId,
}

fn fill_width() -> LayoutChild {
    LayoutChild { horizontal: Sizing::Fill, vertical: Sizing::Fixed, ..LayoutChild::default() }
}

/// A column of 200px width holding a Fill-width vector (path = its 20x10 box) and a Fill-width rectangle.
fn scene() -> Scene {
    let mut doc = Document::new("headless-fill-vector");
    let page = doc.add_page("Page 1");
    let frame = doc.alloc_id();
    let mut f = Node::frame(frame, Rect::new(0.0, 0.0, 200.0, 100.0));
    if let NodeKind::Frame(fp) = &mut f.kind {
        fp.auto_layout =
            Some(AutoLayout { direction: LayoutDirection::Vertical, primary_sizing: Sizing::Fixed, counter_sizing: Sizing::Fixed, ..AutoLayout::default() });
    }
    doc.insert(page, f).unwrap();

    let vector = doc.alloc_id();
    let mut v = Node::new(vector, NodeKind::Vector { paths: vec![square(0.0, 0.0, 20.0, 10.0)], network: None });
    v.size = Vec2::new(20.0, 10.0);
    v.layout_child = fill_width();
    doc.insert(frame, v).unwrap();

    let rect = doc.alloc_id();
    let mut r = Node::rectangle(rect, Rect::new(0.0, 0.0, 20.0, 10.0));
    r.layout_child = fill_width();
    doc.insert(frame, r).unwrap();
    Scene { doc, frame, vector, rect }
}

#[test]
fn a_fill_width_vector_laid_out_headless_has_a_path_that_fits_its_box() {
    let mut s = scene();
    s.doc.relayout_subtree(s.frame);

    let n = s.doc.get(s.vector).unwrap();
    assert_eq!(n.size, Vec2::new(200.0, 10.0), "Fill takes the parent's width");
    let NodeKind::Vector { paths, .. } = &n.kind else { panic!("a vector") };
    assert_eq!(bounds(paths), (0.0, 0.0, 200.0, 10.0), "the path follows the box");
}

#[test]
fn resizing_the_parent_scales_the_fill_vector_path_again() {
    let mut s = scene();
    s.doc.relayout_subtree(s.frame);
    s.doc.get_mut(s.frame).unwrap().size = Vec2::new(300.0, 100.0);
    s.doc.relayout_subtree(s.frame);

    let n = s.doc.get(s.vector).unwrap();
    assert_eq!(n.size, Vec2::new(300.0, 10.0));
    let NodeKind::Vector { paths, .. } = &n.kind else { panic!("a vector") };
    assert_eq!(bounds(paths), (0.0, 0.0, 300.0, 10.0));
}

#[test]
fn a_fill_width_rectangle_is_laid_out_as_a_plain_box() {
    let mut s = scene();
    s.doc.relayout_subtree(s.frame);

    let n = s.doc.get(s.rect).unwrap();
    assert_eq!(n.size, Vec2::new(200.0, 10.0));
    assert!(n.fill_geometry.is_none() && n.stroke_geometry.is_none());
    assert!(matches!(n.kind, NodeKind::Rectangle));
}
