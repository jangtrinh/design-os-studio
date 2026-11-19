//! `designos_update_node` resizes a vector the way the editor does (its path follows the new size and its
//! stroke keeps its weight) and leaves every other kind exactly as it was: the box changes, nothing else.
#![allow(clippy::result_large_err)]

use designos_doc::*;
use designos_mcp::engine::Engine;
use serde_json::json;

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

/// A 2px baked outline `(1, 1)..(19, 19)` around the centerline square `(2, 2)..(18, 18)`.
fn outline() -> Vec<Path> {
    let mut ring = square(1.0, 1.0, 19.0, 19.0);
    ring.cmds.extend(square(3.0, 3.0, 17.0, 17.0).cmds);
    vec![ring]
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

fn add(e: &mut Engine, parent: NodeId, build: impl FnOnce(NodeId) -> Node) -> NodeId {
    let id = e.doc.alloc_id();
    e.doc.insert(parent, build(id)).unwrap()
}

fn stroked(mut n: Node) -> Node {
    n.size = Vec2::new(20.0, 20.0);
    n.stroke.weight = 2.0;
    n.stroke.align = StrokeAlign::Center;
    n.stroke_geometry = Some(outline());
    n
}

fn resize(e: &mut Engine, id: NodeId) {
    e.call("designos_update_node", json!({"node_id": id.0, "width": 40.0, "height": 40.0})).unwrap();
}

#[test]
fn a_vector_keeps_its_stroke_weight_when_an_agent_resizes_it() {
    let mut e = Engine::new();
    let page = NodeId(e.resolve_default_page().unwrap().0);
    let net = VectorNetwork::from_paths(&[square(2.0, 2.0, 18.0, 18.0)]);
    let vector = add(&mut e, page, |id| stroked(Node::new(id, NodeKind::Vector { paths: outline(), network: Some(net) })));

    resize(&mut e, vector);

    let n = e.doc.get(vector).unwrap();
    assert_eq!(n.size, Vec2::new(40.0, 40.0));
    assert_eq!(n.stroke.weight, 2.0, "the weight is the vector's own");
    assert!(n.stroke_geometry.is_none(), "the stale outline is dropped, the renderers stroke the centerline");
    let NodeKind::Vector { paths, .. } = &n.kind else { panic!("a vector") };
    assert_eq!(bounds(paths), (4.0, 4.0, 36.0, 36.0), "the path is the centerline at the new size");
}

#[test]
fn a_boolean_group_is_resized_by_an_agent_exactly_as_before() {
    let mut e = Engine::new();
    let page = NodeId(e.resolve_default_page().unwrap().0);
    let group = add(&mut e, page, |id| stroked(Node::new(id, NodeKind::BooleanOp(BooleanOp::Union))));
    let child = add(&mut e, group, |id| Node::rectangle(id, Rect::new(0.0, 0.0, 12.0, 12.0)));
    let child_before = e.doc.get(child).unwrap().clone();

    resize(&mut e, group);

    let n = e.doc.get(group).unwrap();
    assert_eq!(n.size, Vec2::new(40.0, 40.0));
    assert_eq!(n.stroke_geometry, Some(outline()), "the group's baked outline is not touched");
    let c = e.doc.get(child).unwrap();
    assert_eq!((c.size, c.transform), (child_before.size, child_before.transform), "nor are its operands");
}

#[test]
fn a_rectangle_keeps_its_baked_geometry_when_an_agent_resizes_it() {
    let mut e = Engine::new();
    let page = NodeId(e.resolve_default_page().unwrap().0);
    let rect = add(&mut e, page, |id| {
        let mut n = stroked(Node::rectangle(id, Rect::new(0.0, 0.0, 20.0, 20.0)));
        n.fill_geometry = Some(outline());
        n
    });

    resize(&mut e, rect);

    let n = e.doc.get(rect).unwrap();
    assert_eq!(n.size, Vec2::new(40.0, 40.0));
    assert_eq!((n.fill_geometry.clone(), n.stroke_geometry.clone()), (Some(outline()), Some(outline())), "only the box changes");
}
