//! The layer hash behind kept instance geometry must see every field it stands for, and the write-back must restore
//! exactly the fields it compares.

use crate::branch_diff::StableHasher;
use crate::component_imported_layout_fields::{hash_layer, LayoutFields};
use crate::vector::{Region, Segment, Vertex};
use crate::{FillRule, Node, NodeId, NodeKind, Path, PathCmd, Stroke, StrokeCap, StrokeGeometryStamp, Vec2, VectorNetwork};

fn path(offset: f32) -> Path {
    Path {
        cmds: vec![
            PathCmd::MoveTo(Vec2::new(offset, 0.0)),
            PathCmd::LineTo(Vec2::new(10.0, 0.0)),
            PathCmd::QuadTo(Vec2::new(12.0, 2.0), Vec2::new(10.0, 4.0)),
            PathCmd::CubicTo(Vec2::new(8.0, 6.0), Vec2::new(4.0, 6.0), Vec2::new(0.0, 4.0)),
            PathCmd::Close,
        ],
        fill_rule: FillRule::NonZero,
    }
}

fn layer(kind: NodeKind) -> Node {
    let mut n = Node::new(NodeId(7), kind);
    n.component.source = Some(NodeId(3));
    n.children = vec![NodeId(8)];
    n.size = Vec2::new(40.0, 20.0);
    n.transform.tx = 4.0;
    n.transform.ty = 6.0;
    n.layout_child.stretched.w = Some((40.0, 32.0));
    n.fill_geometry = Some(vec![path(0.0)]);
    n.stroke_geometry = Some(vec![path(1.0)]);
    n.stroke_geometry_for = Some(StrokeGeometryStamp::of_size_and_stroke(n.size, &Stroke::default()));
    n.corner_radii = [2.0, 2.0, 2.0, 2.0];
    n
}

fn hash(n: &Node, is_root: bool) -> u64 {
    let mut h = StableHasher::new();
    hash_layer(&mut h, n, is_root);
    h.finish()
}

type Edit = (&'static str, fn(&mut Node));

const EDITS: &[Edit] = &[
    ("mirrored main layer", |n| n.component.source = Some(NodeId(4))),
    ("no mirrored main layer", |n| n.component.source = None),
    ("child count", |n| n.children.push(NodeId(9))),
    ("width", |n| n.size.x += 0.5),
    ("height", |n| n.size.y += 0.5),
    ("x", |n| n.transform.tx += 0.5),
    ("y", |n| n.transform.ty += 0.5),
    ("scale", |n| n.transform.a = 2.0),
    ("skew", |n| n.transform.b = 0.5),
    ("stretch width", |n| n.layout_child.stretched.w = Some((40.0, 30.0))),
    ("stretch height", |n| n.layout_child.stretched.h = Some((20.0, 18.0))),
    ("stretch axis", |n| n.layout_child.stretched.last_parent_horiz = Some(true)),
    ("fill outline point", |n| n.fill_geometry = Some(vec![path(0.5)])),
    ("fill outline rule", |n| n.fill_geometry.as_mut().unwrap()[0].fill_rule = FillRule::EvenOdd),
    ("fill outline command", |n| n.fill_geometry.as_mut().unwrap()[0].cmds[1] = PathCmd::MoveTo(Vec2::new(10.0, 0.0))),
    ("fill outline dropped", |n| n.fill_geometry = None),
    ("fill outline empty", |n| n.fill_geometry = Some(Vec::new())),
    ("stroke outline point", |n| n.stroke_geometry = Some(vec![path(1.5)])),
    ("stroke outline dropped", |n| n.stroke_geometry = None),
    ("stroke stamp", |n| n.stroke_geometry_for.as_mut().unwrap().weight = 3.0),
    ("corner radius", |n| n.corner_radii[2] = 4.0),
    ("corner smoothing", |n| n.corner_smoothing = 0.6),
    ("shape parameters", |n| n.kind = NodeKind::Line),
];

#[test]
fn every_layout_meaning_and_identity_field_changes_the_layer_hash() {
    let base = layer(NodeKind::Rectangle);
    let h0 = hash(&base, false);
    for (name, edit) in EDITS {
        let mut n = base.clone();
        edit(&mut n);
        assert_ne!(hash(&n, false), h0, "{name} left the layer hash unchanged");
    }
}

#[test]
fn vector_paths_change_the_layer_hash() {
    let base = layer(NodeKind::Vector { paths: vec![path(0.0)], network: None });
    let mut moved = base.clone();
    moved.kind = NodeKind::Vector { paths: vec![path(0.25)], network: None };
    assert_ne!(hash(&base, false), hash(&moved, false));
}

fn network() -> VectorNetwork {
    VectorNetwork {
        vertices: vec![Vertex::new(Vec2::new(0.0, 0.0)), Vertex::new(Vec2::new(10.0, 0.0)), Vertex::new(Vec2::new(10.0, 10.0))],
        segments: vec![
            Segment { a: 0, b: 1, ta: Vec2::ZERO, tb: Vec2::ZERO },
            Segment { a: 1, b: 2, ta: Vec2::new(2.0, 0.0), tb: Vec2::ZERO },
            Segment { a: 2, b: 0, ta: Vec2::ZERO, tb: Vec2::ZERO },
        ],
        regions: vec![Region { rule: FillRule::NonZero, loops: vec![vec![0, 1, 2]], filled: true }],
    }
}

type NetworkEdit = (&'static str, fn(&mut VectorNetwork));

const NETWORK_EDITS: &[NetworkEdit] = &[
    ("vertex position", |n| n.vertices[1].pos.x = 11.0),
    ("vertex corner", |n| n.vertices[1].corner_radius = 2.0),
    ("vertex cap", |n| n.vertices[0].cap = Some(StrokeCap::Round)),
    ("vertex added", |n| n.vertices.push(Vertex::new(Vec2::new(5.0, 5.0)))),
    ("segment end", |n| n.segments[2].b = 1),
    ("segment handle", |n| n.segments[1].ta = Vec2::new(3.0, 0.0)),
    ("region rule", |n| n.regions[0].rule = FillRule::EvenOdd),
    ("region loop", |n| n.regions[0].loops[0].reverse()),
];

#[test]
fn every_vector_network_field_changes_the_layer_hash() {
    let with = |net: Option<VectorNetwork>| layer(NodeKind::Vector { paths: vec![path(0.0)], network: net });
    let h0 = hash(&with(Some(network())), false);
    assert_ne!(hash(&with(None), false), h0, "a network added");
    for (name, edit) in NETWORK_EDITS {
        let mut net = network();
        edit(&mut net);
        assert_ne!(hash(&with(Some(net)), false), h0, "{name} left the layer hash unchanged");
    }
}

#[test]
fn root_position_is_its_own_and_not_hashed() {
    let base = layer(NodeKind::Rectangle);
    let mut moved = base.clone();
    moved.transform.tx += 100.0;
    assert_eq!(hash(&base, true), hash(&moved, true));
    assert_ne!(hash(&base, false), hash(&moved, false));
}

#[test]
fn written_back_fields_are_exactly_the_compared_ones() {
    let noted = layer(NodeKind::Vector { paths: vec![path(0.0)], network: None });
    let fields = LayoutFields::of(&noted);
    assert!(fields.held_by(&noted, false));
    let mut synced = noted.clone();
    synced.size.x = 60.0;
    synced.transform.tx = 9.0;
    synced.layout_child.stretched.w = None;
    synced.fill_geometry = None;
    synced.stroke_geometry = Some(vec![path(3.0)]);
    synced.stroke_geometry_for = None;
    synced.kind = NodeKind::Vector { paths: vec![path(2.0)], network: None };
    synced.opacity = 0.5;
    assert!(!fields.held_by(&synced, false));
    fields.write_to(&mut synced, false);
    assert!(fields.held_by(&synced, false));
    assert_eq!(hash(&synced, false), hash(&noted, false));
    assert_eq!(synced.opacity, 0.5, "the write-back touches layout fields only");
}

#[test]
fn an_outline_is_put_back_only_on_the_shape_it_was_drawn_for() {
    let noted = layer(NodeKind::Rectangle);
    let meaning = LayoutFields::of(&noted).outline_meaning.expect("a layer with a baked outline notes what it means");
    assert!(meaning.held_by(&noted));
    let edits: [Edit; 4] = [
        ("corner radius", |n| n.corner_radii[0] = 6.0),
        ("corner smoothing", |n| n.corner_smoothing = 0.6),
        ("shape", |n| n.kind = NodeKind::Line),
        ("rectangle became a frame", |n| n.kind = NodeKind::Frame(Default::default())),
    ];
    for (name, edit) in edits {
        let mut n = noted.clone();
        edit(&mut n);
        assert!(!meaning.held_by(&n), "{name} kept the outline");
    }
    let mut plain = noted.clone();
    plain.fill_geometry = None;
    plain.stroke_geometry = None;
    assert!(LayoutFields::of(&plain).outline_meaning.is_none(), "no outline, nothing to check");
}
