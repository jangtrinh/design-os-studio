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

type Pt = (f32, f32);
type Handles = (Pt, Pt);

/// An imported network (marked, no regions) from closed `loops`, each its own vertices and segments; `handles` gives the
/// bezier handles of the segments of the loop of the same index (none: straight).
fn imported(loops: &[Vec<Pt>], handles: &[Vec<Handles>]) -> VectorNetwork {
    let mut net = VectorNetwork::default();
    for (k, pts) in loops.iter().enumerate() {
        let first = net.vertices.len();
        for &(x, y) in pts {
            net.add_vertex(v(x, y));
        }
        for i in 0..pts.len() {
            let ((ax, ay), (bx, by)) = handles.get(k).and_then(|h| h.get(i)).copied().unwrap_or_default();
            net.segments.push(designos_doc::vector::Segment { a: first + i, b: first + (i + 1) % pts.len(), ta: v(ax, ay), tb: v(bx, by) });
        }
    }
    net.fills_closed_loops_without_regions = true;
    net
}

fn square(x: f32, y: f32, side: f32, reversed: bool) -> Vec<Pt> {
    let mut pts = vec![(x, y), (x + side, y), (x + side, y + side), (x, y + side)];
    if reversed {
        pts.reverse();
    }
    pts
}

fn circle(cx: f32, cy: f32, r: f32, reversed: bool) -> (Vec<Pt>, Vec<Handles>) {
    let k = 0.552_284_8 * r;
    let pts = vec![(cx + r, cy), (cx, cy + r), (cx - r, cy), (cx, cy - r)];
    let h = vec![((0.0, k), (k, 0.0)), ((-k, 0.0), (0.0, k)), ((0.0, -k), (-k, 0.0)), ((k, 0.0), (0.0, -k))];
    if reversed {
        (vec![pts[0], pts[3], pts[2], pts[1]], vec![((0.0, -k), (k, 0.0)), ((-k, 0.0), (0.0, -k)), ((0.0, k), (-k, 0.0)), ((k, 0.0), (0.0, k))])
    } else {
        (pts, h)
    }
}

/// The fill of `net` at 4x on a 120 x 120 page, as bytes.
fn fill_at_4x(net: &VectorNetwork) -> Vec<u8> {
    picture_at_4x(net, 0.0)
}

/// `net` at 4x on a 120 x 120 page with a blue stroke of `stroke_w` at half opacity (a translucent stroke shows any
/// outline painted twice), as bytes.
fn picture_at_4x(net: &VectorNetwork, stroke_w: f32) -> Vec<u8> {
    let (mut d, page) = page_with(net.clone(), Color::rgb(255, 0, 0), stroke_w, StrokeCap::None);
    let id = vector_id(&d, page);
    d.get_mut(id).unwrap().stroke.paints[0].opacity = 0.5;
    let mut pm = Pixmap::new(480, 480).unwrap();
    render_page(&d, page, &Viewport { zoom: 4.0, ..Viewport::default() }, &mut pm);
    pm.data().to_vec()
}

/// Shapes whose fill must not change when the loops get their regions: `(name, network, a point inside one loop)`.
fn materialise_fixtures() -> Vec<(&'static str, VectorNetwork, Vec2)> {
    let (c_outer, h_outer) = circle(60.3, 60.7, 40.0, false);
    let (c_in, h_in) = circle(60.3, 60.7, 15.0, false);
    let (c_in_rev, h_in_rev) = circle(60.3, 60.7, 15.0, true);
    let (c_mid, h_mid) = circle(60.3, 60.7, 28.0, true);
    let (ca, ha) = circle(45.3, 55.7, 30.0, false);
    let (cb, hb) = circle(75.1, 55.7, 30.0, true);
    let star = |r: f32| -> Vec<Pt> {
        let at = |k: usize| -> Pt {
            let a = -std::f32::consts::FRAC_PI_2 + k as f32 * 1.256_637;
            (60.3 + r * a.cos(), 62.7 + r * a.sin())
        };
        (0..5).map(|k| at((k * 2) % 5)).collect()
    };
    let (tip, tip_h) = circle(60.3, 28.0, 4.0, false);
    vec![
        ("disjoint squares", imported(&[square(5.3, 5.7, 40.0, false), square(60.3, 60.7, 40.0, true)], &[]), v(20.0, 20.0)),
        ("sliver and square", imported(&[vec![(10.3, 10.7), (10.7, 10.7), (10.7, 13.7), (10.3, 13.7)], square(50.3, 50.7, 30.0, false)], &[]), v(60.0, 60.0)),
        ("same-winding ring", imported(&[c_outer.clone(), c_in.clone()], &[h_outer.clone(), h_in.clone()]), v(60.3, 30.0)),
        ("opposite-wound ring", imported(&[c_outer.clone(), c_in_rev], &[h_outer.clone(), h_in_rev]), v(60.3, 30.0)),
        ("three deep", imported(&[c_outer, c_mid, c_in.clone()], &[h_outer, h_mid, h_in.clone()]), v(60.3, 25.0)),
        ("crescent", imported(&[vec![(10.3, 60.7), (90.9, 62.1)]], &[vec![((25.0, -20.0), (-25.0, -20.0)), ((-30.0, -70.0), (30.0, -70.0))]]), v(50.0, 40.0)),
        ("heart", imported(&[vec![(50.3, 90.7), (50.3, 30.1)]], &[vec![((65.0, -40.0), (55.0, -45.0)), ((-55.0, -45.0), (-65.0, -40.0))]]), v(70.0, 50.0)),
        (
            "figure 8 with no crossing vertex",
            imported(&[vec![(20.3, 50.7), (90.1, 50.7)]], &[vec![((10.0, -50.0), (-10.0, 50.0)), ((-10.0, -50.0), (10.0, 50.0))]]),
            v(35.0, 45.0),
        ),
        ("crossing circles, opposite winding", imported(&[ca, cb], &[ha, hb]), v(25.0, 56.0)),
        ("pentagram", imported(&[star(45.0)], &[]), v(60.3, 25.0)),
        ("pentagram with a circle in a tip", imported(&[star(45.0), tip], &[vec![], tip_h]), v(60.3, 28.0)),
        ("squares touching at a corner", imported(&[square(5.3, 5.7, 40.0, false), square(45.3, 45.7, 40.0, false)], &[]), v(20.0, 20.0)),
        (
            "close nested rings",
            imported(
                &[circle(60.3, 60.7, 20.0, false).0, circle(60.3, 60.7, 17.0, true).0],
                &[circle(60.3, 60.7, 20.0, false).1, circle(60.3, 60.7, 17.0, true).1],
            ),
            v(60.3, 42.2),
        ),
    ]
}

/// Giving the loops of an imported network their regions draws the picture the fallback drew, byte for byte at 4x, for
/// curved, nested, crossing and self-crossing loops: each region is a closed chain filled with the non-zero rule, the
/// fallback's own paths.
#[test]
fn materialising_draws_the_fallback_fill_byte_for_byte() {
    for (name, net, _) in materialise_fixtures() {
        let before = fill_at_4x(&net);
        assert!(before.chunks(4).any(|p| p[1] < 255), "{name}: something is drawn");
        let mut after = net;
        assert!(after.materialize_closed_loop_fill(), "{name}");
        assert!(!after.fills_closed_loops_without_regions && !after.regions.is_empty(), "{name}: regions now");
        assert!(fill_at_4x(&after) == before, "{name}: the picture changed");
    }
}

/// A translucent stroke over loops whose bands overlap (crossing circles, nested rings closer than the stroke, loops
/// touching, a loop crossing itself) is the same picture before and after the loops get their regions: the stroke runs
/// over the same per-chain paths either way, so materialising changes nothing. (Where two chains' bands overlap the
/// translucent stroke is painted once per chain, before and after alike.)
#[test]
fn materialising_keeps_a_translucent_stroke_over_overlapping_chains_byte_for_byte() {
    let mut checked = 0;
    for (name, net, _) in materialise_fixtures() {
        if !["crossing circles, opposite winding", "close nested rings", "squares touching at a corner", "pentagram", "same-winding ring"].contains(&name) {
            continue;
        }
        let before = picture_at_4x(&net, 6.0);
        let mut after = net;
        assert!(after.materialize_closed_loop_fill(), "{name}");
        assert!(picture_at_4x(&after, 6.0) == before, "{name}: the stroked picture changed");
        checked += 1;
    }
    assert_eq!(checked, 5);
}

/// What Flatten, Offset path, the shape builder and Boolean operands read (`node_paths`) is the union the canvas fills:
/// a region-less donut wound the other way is a solid disc there too.
#[test]
fn the_paths_flatten_reads_draw_the_canvas_fill() {
    for (name, net, _) in materialise_fixtures() {
        let canvas = fill_at_4x(&net);
        let (mut d, page) = page_with(net, Color::rgb(255, 0, 0), 0.0, StrokeCap::None);
        let id = vector_id(&d, page);
        let paths = designos_render::vectorops::node_paths(&d, d.get(id).unwrap()).expect("a vector has geometry");
        d.get_mut(id).unwrap().kind = NodeKind::Vector { paths, network: None };
        let mut pm = Pixmap::new(480, 480).unwrap();
        render_page(&d, page, &Viewport { zoom: 4.0, ..Viewport::default() }, &mut pm);
        assert!(pm.data() == canvas.as_slice(), "{name}: the paths a Flatten reads draw another picture than the canvas");
    }
}

/// The bucket on one loop never changes a pixel outside the face it clicked, and where the face is a loop of its own it
/// does change the pixels inside it (so the test cannot pass by the click doing nothing).
#[test]
fn the_bucket_changes_no_pixel_outside_the_face_it_clicked() {
    let mut loops_clicked = 0;
    for (name, net, click) in materialise_fixtures() {
        let before = fill_at_4x(&net);
        let mut edited = net;
        // a loop that crosses itself with no vertex where it does has no face to click (its signed area is nil)
        let Some(face) = edited.face_at(click) else {
            assert!(name.starts_with("figure 8"), "{name}: a face under the click");
            continue;
        };
        let (mut lo, mut hi) = (v(f32::MAX, f32::MAX), v(f32::MIN, f32::MIN));
        for p in edited.face_contours(&face).first().expect("an outer loop") {
            (lo, hi) = (v(lo.x.min(p.x), lo.y.min(p.y)), v(hi.x.max(p.x), hi.y.max(p.y)));
        }
        // whether the face is exactly one chain, which the click then flips
        let mut probe = edited.clone();
        assert!(probe.materialize_closed_loop_fill());
        let is_a_loop = probe.region_of_face(&face).is_some();
        edited.toggle_face(&face);
        let after = fill_at_4x(&edited);
        let (x0, y0, x1, y1) = ((lo.x * 4.0 - 4.0) as usize, (lo.y * 4.0 - 4.0) as usize, (hi.x * 4.0 + 4.0) as usize, (hi.y * 4.0 + 4.0) as usize);
        let mut changed_inside = 0;
        for y in 0..480usize {
            for x in 0..480usize {
                let i = (y * 480 + x) * 4;
                if (x0..=x1).contains(&x) && (y0..=y1).contains(&y) {
                    changed_inside += usize::from(before[i..i + 4] != after[i..i + 4]);
                    continue;
                }
                assert!(before[i..i + 4] == after[i..i + 4], "{name}: pixel ({x}, {y}) outside the clicked face changed");
            }
        }
        if is_a_loop {
            assert!(changed_inside > 0, "{name}: the click on a loop changed nothing");
            loops_clicked += 1;
        }
    }
    assert!(loops_clicked >= 6, "the bucket was tried on {loops_clicked} loops");
}

/// A sliver of 0.4 next to a square: the bucket unfills the square and the sliver stays.
#[test]
fn the_bucket_on_a_square_leaves_a_sliver_beside_it_filled() {
    let fixtures = materialise_fixtures();
    let (_, net, click) = fixtures.iter().find(|f| f.0 == "sliver and square").unwrap();
    let mut edited = net.clone();
    let face = edited.face_at(*click).unwrap();
    assert!(!edited.toggle_face(&face));
    let after = fill_at_4x(&edited);
    // the page is white and the fill red: a filled pixel has lost its green (the sliver is under 2 px wide at 4x, so partly)
    let filled = |x: f32, y: f32| after[((y * 4.0) as usize * 480 + (x * 4.0) as usize) * 4 + 1] < 200;
    assert!(!filled(60.0, 60.0), "the square is unfilled");
    assert!(filled(10.5, 12.0), "the sliver beside it is still filled");
}
