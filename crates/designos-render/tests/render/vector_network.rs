//! Rendering of vector networks: filled regions vs stroke-only segments and
//! per-vertex stroke caps.

use designos_doc::vector::VectorNetwork;
use designos_doc::*;
use designos_render::{render_page, tiny_skia::Pixmap, Viewport};

fn v(x: f32, y: f32) -> Vec2 {
    Vec2::new(x, y)
}

fn rect_path(x: f32, y: f32, w: f32, h: f32) -> Path {
    Path {
        cmds: vec![PathCmd::MoveTo(v(x, y)), PathCmd::LineTo(v(x + w, y)), PathCmd::LineTo(v(x + w, y + h)), PathCmd::LineTo(v(x, y + h)), PathCmd::Close],
        fill_rule: FillRule::NonZero,
    }
}

fn page_with(net: VectorNetwork, fill: Color, stroke_w: f32, cap: StrokeCap) -> (Document, NodeId) {
    let mut d = Document::new("t");
    let page = d.add_page("p");
    d.get_mut(page).unwrap().kind = NodeKind::Page { background: Color::WHITE };
    let id = d.alloc_id();
    let mut n = Node::new(id, NodeKind::Vector { paths: net.to_paths(), network: Some(net) });
    n.size = v(100.0, 100.0);
    n.fills = vec![Paint::solid(fill)];
    n.stroke.paints = vec![Paint::solid(Color::rgb(0, 0, 255))];
    n.stroke.weight = stroke_w;
    n.stroke.align = StrokeAlign::Center;
    n.stroke.cap = cap;
    d.insert(page, n).unwrap();
    (d, page)
}

fn render(d: &Document, page: NodeId) -> Pixmap {
    let mut pm = Pixmap::new(120, 120).unwrap();
    render_page(d, page, &Viewport::default(), &mut pm);
    pm
}

fn px(pm: &Pixmap, x: u32, y: u32) -> [u8; 4] {
    let p = pm.pixel(x, y).unwrap();
    [p.red(), p.green(), p.blue(), p.alpha()]
}

#[test]
fn unfilled_regions_are_not_painted_but_still_stroked() {
    let mut net = VectorNetwork::from_paths(&[rect_path(10.0, 10.0, 80.0, 80.0)]);
    let red = Color::rgb(255, 0, 0);
    let (d, page) = page_with(net.clone(), red, 4.0, StrokeCap::None);
    let pm = render(&d, page);
    assert_eq!(px(&pm, 50, 50), [255, 0, 0, 255], "filled");
    // unfill: the region goes, the segments stay
    net.regions.clear();
    let (d, page) = page_with(net, red, 4.0, StrokeCap::None);
    let pm = render(&d, page);
    assert_eq!(px(&pm, 50, 50), [255, 255, 255, 255], "inside the loop is not filled any more");
    assert_eq!(px(&pm, 10, 50), [0, 0, 255, 255], "the outline remains");
}

#[test]
fn only_filled_faces_of_a_network_are_painted() {
    let mut net = VectorNetwork::default();
    for p in [v(0.0, 0.0), v(50.0, 0.0), v(100.0, 0.0), v(0.0, 100.0), v(50.0, 100.0), v(100.0, 100.0)] {
        net.add_vertex(p);
    }
    for (a, b) in [(0, 1), (1, 2), (3, 4), (4, 5), (0, 3), (1, 4), (2, 5)] {
        net.connect(a, b);
    }
    let left = net.face_at(v(25.0, 50.0)).unwrap();
    net.toggle_face(&left);
    let (d, page) = page_with(net, Color::rgb(255, 0, 0), 0.0, StrokeCap::None);
    let pm = render(&d, page);
    assert_eq!(px(&pm, 25, 50), [255, 0, 0, 255]);
    assert_eq!(px(&pm, 75, 50), [255, 255, 255, 255]);
}

#[test]
fn per_vertex_cap_is_drawn_at_that_end_only() {
    let line = Path { cmds: vec![PathCmd::MoveTo(v(30.0, 50.0)), PathCmd::LineTo(v(80.0, 50.0))], fill_rule: FillRule::NonZero };
    let mut net = VectorNetwork::from_paths(&[line]);
    net.vertices[1].cap = Some(StrokeCap::Round);
    let (d, page) = page_with(net, Color::rgb(255, 0, 0), 20.0, StrokeCap::None);
    let pm = render(&d, page);
    assert_eq!(px(&pm, 85, 50), [0, 0, 255, 255], "round cap extends past the end by half the weight");
    assert_eq!(px(&pm, 25, 50), [255, 255, 255, 255], "the other end is butt");
    assert_eq!(px(&pm, 50, 50), [0, 0, 255, 255]);
}
