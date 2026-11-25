//! A `.fig` vector keeps its network in the normalised size. A line has a zero normalised height (or width), and
//! its other axis still scales to the node's size: a 640 wide network on a 120 wide node draws 120 wide. A dotted
//! line that kept its normalised width drew its dots well past the end of the layer once a vertex edit rebuilt its
//! outline from the network.

use designos_doc::*;
use designos_fig::container::{self, Compression};
use designos_fig::import_fig;
use designos_fig::kiwi::KiwiValue as V;
use designos_fig::testkit::{en, figma_like_schema, fl, matrix, msg, node, solid, st, vec2};
use designos_render::tiny_skia as sk;

/// A two-vertex line network `a -> b` in the normalised size.
fn line_blob(a: (f32, f32), b: (f32, f32)) -> Vec<u8> {
    let mut out = Vec::new();
    for n in [2u32, 1, 0] {
        out.extend(n.to_le_bytes());
    }
    for (x, y) in [a, b] {
        out.extend(0u32.to_le_bytes());
        out.extend(x.to_le_bytes());
        out.extend(y.to_le_bytes());
    }
    out.extend(0u32.to_le_bytes());
    out.extend(0u32.to_le_bytes());
    out.extend([0.0f32, 0.0].iter().flat_map(|f| f.to_le_bytes()));
    out.extend(1u32.to_le_bytes());
    out.extend([0.0f32, 0.0].iter().flat_map(|f| f.to_le_bytes()));
    out
}

/// A dotted vector (dash 0.1, gap 6, weight 2, round caps) of `size` whose network runs from `a` to `b` in the
/// normalised size `norm`.
fn dotted_fig(size: (f32, f32), norm: (f32, f32), a: (f32, f32), b: (f32, f32)) -> Vec<u8> {
    let changes = vec![
        node((0, 0), (0, 0), "!", "DOCUMENT", "Document", vec![]),
        node((0, 1), (0, 0), "a", "CANVAS", "Page", vec![]),
        node(
            (1, 1),
            (0, 1),
            "a",
            "VECTOR",
            "Dots",
            vec![
                ("size", vec2(size.0, size.1)),
                ("transform", matrix(0.0, 0.0)),
                ("strokePaints", V::Array(vec![solid(0.0, 0.0, 0.0)])),
                ("strokeWeight", fl(2.0)),
                ("strokeAlign", en("CENTER")),
                ("strokeCap", en("ROUND")),
                ("dashPattern", V::Array(vec![fl(0.1), fl(6.0)])),
                ("vectorData", msg(vec![("vectorNetworkBlob", V::Uint(0)), ("normalizedSize", vec2(norm.0, norm.1))])),
            ],
        ),
    ];
    let blobs = V::Array(vec![st(vec![("bytes", V::Bytes(line_blob(a, b)))])]);
    let message = msg(vec![("nodeChanges", V::Array(changes)), ("blobs", blobs)]);
    let schema = figma_like_schema();
    let root = schema.find("Message").unwrap();
    let body = schema.encode_message(root, &message).unwrap();
    container::write_container(&schema.encode(), &body, Compression::Zstd, 106)
}

fn dots(doc: &Document) -> (NodeId, VectorNetwork) {
    let n = doc.nodes.values().find(|n| n.name == "Dots").expect("the vector is imported");
    let NodeKind::Vector { network: Some(net), .. } = &n.kind else { panic!("imported with its network") };
    (n.id, net.clone())
}

/// Moves the second vertex by `by` and rebuilds the live outline from the network, as a vertex edit does.
fn nudge_second_vertex(doc: &mut Document, id: NodeId, by: Vec2) {
    let n = doc.get_mut(id).unwrap();
    n.drop_stroke_outline();
    let NodeKind::Vector { paths, network: Some(net) } = &mut n.kind else { panic!("a network vector") };
    net.vertices[1].pos = Vec2::new(net.vertices[1].pos.x + by.x, net.vertices[1].pos.y + by.y);
    *paths = net.to_paths();
}

/// Columns and rows of the 4x raster (origin at layer `(-8, -8)`, 4 px per unit) that hold ink.
fn ink_extent(doc: &Document, id: NodeId, w: f32, h: f32) -> (f32, f32, f32, f32) {
    let ts = sk::Transform::from_row(4.0, 0.0, 0.0, 4.0, 32.0, 32.0);
    let mut pm = sk::Pixmap::new(((w + 16.0) * 4.0) as u32, ((h + 16.0) * 4.0) as u32).unwrap();
    designos_render::render_node_into(doc, id, ts, ts, &mut pm);
    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for y in 0..pm.height() {
        for x in 0..pm.width() {
            if pm.pixel(x, y).unwrap().alpha() > 0 {
                let (ux, uy) = (x as f32 / 4.0 - 8.0, y as f32 / 4.0 - 8.0);
                (x0, y0, x1, y1) = (x0.min(ux), y0.min(uy), x1.max(ux + 0.25), y1.max(uy + 0.25));
            }
        }
    }
    (x0, y0, x1, y1)
}

#[test]
fn a_zero_height_line_scales_its_width_to_the_node() {
    let doc = import_fig(&dotted_fig((120.0, 0.0), (640.0, 0.0), (0.0, 0.0), (640.0, 0.0))).unwrap();
    let (_, net) = dots(&doc);
    assert_eq!(net.vertices[0].pos, Vec2::new(0.0, 0.0));
    assert_eq!(net.vertices[1].pos, Vec2::new(120.0, 0.0), "640 normalised on a 120 wide node");
}

#[test]
fn a_zero_width_line_scales_its_height_to_the_node() {
    let doc = import_fig(&dotted_fig((0.0, 80.0), (0.0, 400.0), (0.0, 0.0), (0.0, 400.0))).unwrap();
    let (_, net) = dots(&doc);
    assert_eq!(net.vertices[1].pos, Vec2::new(0.0, 80.0), "400 normalised on an 80 tall node");
}

#[test]
fn a_dotted_zero_height_line_draws_inside_its_node_after_a_vertex_nudge() {
    let mut doc = import_fig(&dotted_fig((120.0, 0.0), (640.0, 0.0), (0.0, 0.0), (640.0, 0.0))).unwrap();
    let (id, _) = dots(&doc);
    nudge_second_vertex(&mut doc, id, Vec2::new(0.25, 0.0));
    let (x0, y0, x1, y1) = ink_extent(&doc, id, 130.0, 0.0);
    assert!((-1.1..=-0.8).contains(&x0), "the first dot sits on the start of the line, ink starts at {x0}");
    assert!((120.0..=121.5).contains(&x1), "the last dot sits on the end of the line, ink ends at {x1}");
    assert!(y0 >= -1.1 && y1 <= 1.1, "a weight 2 line stays within 1 of its axis: {y0}..{y1}");
}

#[test]
fn a_dotted_zero_width_line_draws_inside_its_node_after_a_vertex_nudge() {
    let mut doc = import_fig(&dotted_fig((0.0, 80.0), (0.0, 400.0), (0.0, 0.0), (0.0, 400.0))).unwrap();
    let (id, _) = dots(&doc);
    nudge_second_vertex(&mut doc, id, Vec2::new(0.0, 0.25));
    let (x0, y0, x1, y1) = ink_extent(&doc, id, 0.0, 90.0);
    assert!(x0 >= -1.1 && x1 <= 1.1, "a weight 2 line stays within 1 of its axis: {x0}..{x1}");
    assert!((-1.1..=-0.8).contains(&y0), "ink starts at {y0}");
    assert!((80.0..=81.5).contains(&y1), "ink ends at {y1}");
}

#[test]
fn a_network_with_both_axes_still_scales_each_axis_on_its_own() {
    let doc = import_fig(&dotted_fig((120.0, 30.0), (60.0, 30.0), (0.0, 0.0), (60.0, 30.0))).unwrap();
    let (_, net) = dots(&doc);
    assert_eq!(net.vertices[1].pos, Vec2::new(120.0, 30.0));
}
