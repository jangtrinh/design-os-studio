//! Imported line geometry must follow agent and variable-driven resizes.
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

fn add_line(e: &mut Engine) -> NodeId {
    let page = NodeId(e.resolve_default_page().unwrap().0);
    let id = e.doc.alloc_id();
    let mut line = Node::new(id, NodeKind::Line);
    line.size = Vec2::new(20.0, 20.0);
    line.stroke.weight = 2.0;
    line.stroke_geometry = Some(outline());
    e.doc.insert(page, line).unwrap()
}

#[test]
fn an_imported_line_scales_its_stored_outline_when_resized_by_mcp() {
    let mut e = Engine::new();
    let line = add_line(&mut e);
    e.call("designos_update_node", json!({"node_id": line.0, "width": 40.0, "height": 40.0})).unwrap();
    let n = e.doc.get(line).unwrap();
    assert_eq!(n.size, Vec2::new(40.0, 40.0));
    assert_eq!(bounds(n.stroke_geometry.as_ref().unwrap()), (2.0, 2.0, 38.0, 38.0));
}
