//! Figma's GRID frames: `primaryAxisSizingMode` sizes the width (columns) and `counterAxisSizingMode`
//! the height (rows). The importer, the doc model and the shared layout solver must agree on it.
use designos_doc::*;
use designos_fig::import_rest_json;
use serde_json::{json, Value};

fn cell(id: &str) -> Value {
    json!({"id": id, "name": id, "type": "RECTANGLE", "size": {"x": 50, "y": 20}})
}

fn grid(primary: &str, counter: &str) -> Value {
    json!({"id": "1:1", "name": "Grid", "type": "FRAME", "layoutMode": "GRID", "gridColumns": 3, "itemSpacing": 10,
        "primaryAxisSizingMode": primary, "counterAxisSizingMode": counter, "size": {"x": 400, "y": 300},
        "children": [cell("1:2"), cell("1:3"), cell("1:4")]})
}

fn file(nodes: Vec<Value>) -> String {
    json!({"name": "grid", "document": {"type": "DOCUMENT", "children": [{"id": "0:1", "type": "CANVAS", "name": "P", "children": nodes}]}}).to_string()
}

fn find<'a>(doc: &'a Document, name: &str) -> &'a Node {
    doc.nodes.values().find(|n| n.name == name).unwrap_or_else(|| panic!("no node named {name}"))
}

fn xs(doc: &Document, parent: &Node) -> Vec<f32> {
    parent.children.iter().map(|c| doc.get(*c).unwrap().transform.tx).collect()
}

fn assert_close(actual: &[f32], expected: &[f32]) {
    assert_eq!(actual.len(), expected.len());
    for (a, e) in actual.iter().zip(expected) {
        assert!((a - e).abs() < 1e-3, "{actual:?} != {expected:?}");
    }
}

#[test]
fn a_grid_that_hugs_its_width_reads_primary_as_horizontal_and_survives_relayout() {
    let mut doc = import_rest_json(&file(vec![grid("AUTO", "FIXED")])).unwrap();
    let id = find(&doc, "Grid").id;
    let al = find(&doc, "Grid").kind.frame_props().unwrap().auto_layout.unwrap();
    assert_eq!((al.primary_sizing, al.counter_sizing), (Sizing::Hug, Sizing::Fixed));
    let lc = find(&doc, "Grid").layout_child;
    assert_eq!((lc.horizontal, lc.vertical), (Sizing::Hug, Sizing::Fixed), "primary on a grid is the width");

    doc.relayout_subtree(id);
    let grid = find(&doc, "Grid");
    assert_eq!(grid.size, Vec2::new(170.0, 300.0), "3 x 50 + 2 x 10 wide, the fixed height untouched");
    assert_close(&xs(&doc, grid), &[0.0, 60.0, 120.0]);
}

#[test]
fn a_grid_that_hugs_its_height_keeps_its_width_and_columns_on_relayout() {
    let mut doc = import_rest_json(&file(vec![grid("FIXED", "AUTO")])).unwrap();
    let id = find(&doc, "Grid").id;
    let lc = find(&doc, "Grid").layout_child;
    assert_eq!((lc.horizontal, lc.vertical), (Sizing::Fixed, Sizing::Hug), "counter on a grid is the height");

    doc.relayout_subtree(id);
    let grid = find(&doc, "Grid");
    assert_eq!(grid.size, Vec2::new(400.0, 20.0), "fixed 400 wide, one 20 high row");
    // three equal flex columns of (400 - 20) / 3
    assert_close(&xs(&doc, grid), &[0.0, 380.0 / 3.0 + 10.0, 2.0 * (380.0 / 3.0 + 10.0)]);
}
