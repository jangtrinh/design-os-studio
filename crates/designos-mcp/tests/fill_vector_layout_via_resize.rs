//! An agent that resizes a column auto layout through `designos_update_node` gets a Fill-width vector whose
//! path follows its new box, the way the editor's layout does.
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

fn bounds(paths: &[Path]) -> (f32, f32, f32, f32) {
    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for cmd in paths.iter().flat_map(|p| &p.cmds) {
        if let PathCmd::MoveTo(p) | PathCmd::LineTo(p) = *cmd {
            (x0, y0, x1, y1) = (x0.min(p.x), y0.min(p.y), x1.max(p.x), y1.max(p.y));
        }
    }
    (x0, y0, x1, y1)
}

#[test]
fn a_fill_width_vector_follows_its_box_when_an_agent_resizes_the_column() {
    let mut e = Engine::new();
    let page = NodeId(e.resolve_default_page().unwrap().0);
    let column = e.doc.alloc_id();
    let mut f = Node::frame(column, Rect::new(0.0, 0.0, 200.0, 100.0));
    if let NodeKind::Frame(fp) = &mut f.kind {
        fp.auto_layout =
            Some(AutoLayout { direction: LayoutDirection::Vertical, primary_sizing: Sizing::Fixed, counter_sizing: Sizing::Fixed, ..AutoLayout::default() });
    }
    e.doc.insert(page, f).unwrap();
    let vector = e.doc.alloc_id();
    let mut v = Node::new(vector, NodeKind::Vector { paths: vec![square(0.0, 0.0, 20.0, 10.0)], network: None });
    v.size = Vec2::new(20.0, 10.0);
    v.layout_child = LayoutChild { horizontal: Sizing::Fill, vertical: Sizing::Fixed, ..LayoutChild::default() };
    e.doc.insert(column, v).unwrap();

    e.call("designos_update_node", json!({"node_id": column.0, "width": 320.0})).unwrap();

    let n = e.doc.get(vector).unwrap();
    assert_eq!(n.size, Vec2::new(320.0, 10.0), "Fill takes the resized column's width");
    let NodeKind::Vector { paths, .. } = &n.kind else { panic!("a vector") };
    assert_eq!(bounds(paths), (0.0, 0.0, 320.0, 10.0), "the path is scaled to the new box");
}
