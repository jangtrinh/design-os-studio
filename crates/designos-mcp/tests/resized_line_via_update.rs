//! Imported line geometry must follow agent and variable-driven resizes.
#![allow(clippy::result_large_err)]

use designos_doc::*;
use designos_mcp::engine::Engine;
use serde_json::json;

fn outline() -> Vec<Path> {
    vec![Path {
        cmds: vec![
            PathCmd::MoveTo(Vec2::new(-5.0, -10.0)),
            PathCmd::LineTo(Vec2::new(105.0, -10.0)),
            PathCmd::LineTo(Vec2::new(105.0, 0.0)),
            PathCmd::LineTo(Vec2::new(-5.0, 0.0)),
            PathCmd::Close,
        ],
        fill_rule: FillRule::NonZero,
    }]
}

fn sample_line(id: NodeId) -> Node {
    let mut line = Node::new(id, NodeKind::Line);
    line.size = Vec2::new(100.0, 0.0);
    line.stroke.weight = 10.0;
    line.stroke_geometry = Some(outline());
    line
}

#[test]
fn an_imported_line_scales_its_stored_outline_when_resized_by_mcp() {
    let mut e = Engine::new();
    let page = NodeId(e.resolve_default_page().unwrap().0);
    let id = e.doc.alloc_id();
    let line = e.doc.insert(page, sample_line(id)).unwrap();
    let mut expected = e.doc.get(line).unwrap().clone();
    let tf = expected.transform;
    apply_constrained_geometry(&mut expected, tf, Vec2::new(200.0, 0.0));

    e.call("designos_update_node", json!({"node_id": line.0, "width": 200.0, "height": 0.0})).unwrap();
    let n = e.doc.get(line).unwrap();
    assert_eq!(
        n.stroke_geometry,
        Some(vec![Path {
            cmds: vec![
                PathCmd::MoveTo(Vec2::new(-5.0, -10.0)),
                PathCmd::LineTo(Vec2::new(205.0, -10.0)),
                PathCmd::LineTo(Vec2::new(205.0, 0.0)),
                PathCmd::LineTo(Vec2::new(-5.0, 0.0)),
                PathCmd::Close,
            ],
            fill_rule: FillRule::NonZero,
        }])
    );
    assert_eq!(n, &expected);
}
