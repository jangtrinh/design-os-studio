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

/// Unfills the face under `(x, y)` of the node's network, as the paint bucket does.
fn bucket(d: &mut Document, id: NodeId, x: f32, y: f32) -> bool {
    let NodeKind::Vector { network: Some(net), paths } = &mut d.get_mut(id).unwrap().kind else { panic!("a vector with a network") };
    let face = net.face_at(v(x, y)).expect("a face under the pointer");
    let filled = net.toggle_face(&face);
    *paths = net.to_paths();
    filled
}

fn vector_id(d: &Document, page: NodeId) -> NodeId {
    d.get(page).unwrap().children[0]
}

#[test]
fn unfilled_regions_are_not_painted_but_still_stroked() {
    // the only loop of a network: unfilling it leaves no fill, though the network then has no filled region
    let net = VectorNetwork::from_paths(&[rect_path(10.0, 10.0, 80.0, 80.0)]);
    let red = Color::rgb(255, 0, 0);
    let (mut d, page) = page_with(net, red, 4.0, StrokeCap::None);
    assert_eq!(px(&render(&d, page), 50, 50), [255, 0, 0, 255], "filled");
    let id = vector_id(&d, page);
    assert!(!bucket(&mut d, id, 50.0, 50.0));
    let pm = render(&d, page);
    assert_eq!(px(&pm, 50, 50), [255, 255, 255, 255], "inside the loop is not filled any more");
    assert_eq!(px(&pm, 10, 50), [0, 0, 255, 255], "the outline remains");

    // an unfilled loop next to a filled one
    let mut net = VectorNetwork::from_paths(&[rect_path(10.0, 10.0, 40.0, 80.0), rect_path(60.0, 10.0, 30.0, 80.0)]);
    let (d, page) = page_with(net.clone(), red, 4.0, StrokeCap::None);
    assert_eq!(px(&render(&d, page), 30, 50), [255, 0, 0, 255], "filled");
    net.regions.remove(0);
    let (d, page) = page_with(net, red, 4.0, StrokeCap::None);
    let pm = render(&d, page);
    assert_eq!(px(&pm, 30, 50), [255, 255, 255, 255], "inside the unfilled loop is not painted");
    assert_eq!(px(&pm, 75, 50), [255, 0, 0, 255], "the other loop keeps its region");
    assert_eq!(px(&pm, 10, 50), [0, 0, 255, 255], "the outline remains");
}

/// A native save writes the unfilled loop and a load draws it unfilled; filling it again brings the fill back.
#[test]
fn an_unfilled_only_loop_stays_unfilled_through_a_save_and_fills_again() {
    let net = VectorNetwork::from_paths(&[rect_path(10.0, 10.0, 80.0, 80.0)]);
    let (mut d, page) = page_with(net, Color::rgb(255, 0, 0), 4.0, StrokeCap::None);
    let id = vector_id(&d, page);
    assert!(!bucket(&mut d, id, 50.0, 50.0));
    let mut loaded = Document::from_json(&d.to_json().unwrap()).unwrap();
    assert_eq!(px(&render(&loaded, page), 50, 50), [255, 255, 255, 255], "unfilled after a save and a load");
    assert_eq!(px(&render(&loaded, page), 10, 50), [0, 0, 255, 255], "still stroked");
    assert!(bucket(&mut loaded, id, 50.0, 50.0), "the bucket fills it again");
    let pm = render(&loaded, page);
    assert_eq!(px(&pm, 50, 50), [255, 0, 0, 255], "filled again");
    assert_eq!(px(&pm, 10, 50), [0, 0, 255, 255]);
}

/// Figma stores a fill outline for the closed loops of a network that has no regions at all (a `.fig` holds a visible
/// fill for no other kind of region-less network), so an imported network, which the import marks, fills its
/// closed loops and not its open chains. A network Studio drew with its region dropped stays unfilled, as it always was.
#[test]
fn an_imported_network_without_regions_fills_its_closed_loops_only() {
    let mut net = VectorNetwork::from_paths(&[rect_path(10.0, 10.0, 80.0, 80.0)]);
    net.regions.clear();
    let red = Color::rgb(255, 0, 0);
    let (d, page) = page_with(net.clone(), red, 0.0, StrokeCap::None);
    assert_eq!(px(&render(&d, page), 50, 50), [255, 255, 255, 255], "a Studio network without a region is not filled");
    net.fills_closed_loops_without_regions = true;
    let (d, page) = page_with(net, red, 0.0, StrokeCap::None);
    assert_eq!(px(&render(&d, page), 50, 50), [255, 0, 0, 255], "the imported closed loop fills");

    let open =
        Path { cmds: vec![PathCmd::MoveTo(v(10.0, 10.0)), PathCmd::LineTo(v(90.0, 10.0)), PathCmd::LineTo(v(90.0, 90.0))], fill_rule: FillRule::NonZero };
    let mut net = VectorNetwork::from_paths(&[open]);
    net.fills_closed_loops_without_regions = true;
    let (d, page) = page_with(net, red, 0.0, StrokeCap::None);
    assert_eq!(px(&render(&d, page), 70, 30), [255, 255, 255, 255], "an open chain has no area to fill");
}

/// The bucket on one loop of an imported network with two loops and no region unfills that loop only, and a save keeps it so.
#[test]
fn the_bucket_on_one_imported_loop_leaves_the_other_filled_through_a_save() {
    let mut net = VectorNetwork::from_paths(&[rect_path(10.0, 10.0, 40.0, 80.0), rect_path(60.0, 10.0, 30.0, 80.0)]);
    net.regions.clear();
    net.fills_closed_loops_without_regions = true;
    let (mut d, page) = page_with(net, Color::rgb(255, 0, 0), 4.0, StrokeCap::None);
    let pm = render(&d, page);
    assert_eq!((px(&pm, 30, 50), px(&pm, 75, 50)), ([255, 0, 0, 255], [255, 0, 0, 255]), "both loops fill");
    let id = vector_id(&d, page);
    assert!(!bucket(&mut d, id, 30.0, 50.0));
    let loaded = Document::from_json(&d.to_json().unwrap()).unwrap();
    for doc in [&d, &loaded] {
        let pm = render(doc, page);
        assert_eq!(px(&pm, 30, 50), [255, 255, 255, 255], "the clicked loop is unfilled");
        assert_eq!(px(&pm, 75, 50), [255, 0, 0, 255], "the other loop is still filled");
    }
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

/// A vector of the Untitled UI kit (five vertices, five straight segments, no region, a visible fill) after a vertex
/// is nudged: the dog-eared page still fills inside its outline and not inside the cut corner.
#[test]
fn a_region_less_kit_outline_still_fills_after_a_vertex_is_nudged() {
    let verts = [(0.0, 0.0), (43.342762, 59.783123), (0.0, 59.783123), (33.340584, 0.0), (43.342762, 10.002176)];
    let mut net = VectorNetwork::default();
    for (x, y) in verts {
        net.add_vertex(v(x, y));
    }
    for (a, b) in [(1, 2), (2, 0), (0, 3), (4, 1), (3, 4)] {
        net.connect(a, b).unwrap();
    }
    assert!(net.regions.is_empty());
    net.fills_closed_loops_without_regions = true; // what the import marks
    net.vertices[2].pos = v(0.5, 59.283123);
    let (d, page) = page_with(net, Color::rgb(255, 0, 0), 0.0, StrokeCap::None);
    let pm = render(&d, page);
    assert_eq!(px(&pm, 10, 50), [255, 0, 0, 255], "inside the page");
    assert_eq!(px(&pm, 20, 20), [255, 0, 0, 255], "inside the page");
    assert_eq!(px(&pm, 42, 2), [255, 255, 255, 255], "the cut corner is outside it");
}
