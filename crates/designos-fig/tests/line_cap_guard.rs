//! A Line's stored stroke outline is stretched on resize only when stretching draws what Figma draws.
//! A cap the stroke model has no name for (a REST arrow spelling, a decorative cap) is recorded on the
//! imported Line, so its resize drops the outline instead of stretching the arrowhead.

use designos_doc::*;
use designos_fig::{export_rest_json, import_fig_node_changes, import_rest_json};
use serde_json::{json, Value};

fn rest_line(cap: &str) -> String {
    json!({
        "name": "Lines",
        "document": {
            "id": "0:0", "type": "DOCUMENT", "name": "Document",
            "children": [{
                "id": "0:1", "type": "CANVAS", "name": "Page 1",
                "backgroundColor": {"r": 1, "g": 1, "b": 1, "a": 1},
                "children": [{
                    "id": "1:1", "type": "LINE", "name": "Line",
                    "absoluteBoundingBox": {"x": 40, "y": 40, "width": 100, "height": 0},
                    "size": {"x": 100, "y": 0},
                    "relativeTransform": [[1, 0, 40], [0, 1, 40]],
                    "strokes": [{"type": "SOLID", "color": {"r": 0, "g": 0, "b": 0, "a": 1}}],
                    "strokeWeight": 10,
                    "strokeAlign": "CENTER",
                    "strokeCap": cap,
                    "strokeGeometry": [{"path": "M 0 -10 L 100 -10 L 100 0 L 0 0 Z", "windingRule": "NONZERO"}]
                }]
            }]
        }
    })
    .to_string()
}

fn find<'a>(doc: &'a Document, name: &str) -> &'a Node {
    doc.nodes.values().find(|n| n.name == name).unwrap_or_else(|| panic!("no node named {name}"))
}

fn resize_threefold(doc: &mut Document, name: &str) -> Node {
    let id = find(doc, name).id;
    let n = doc.get_mut(id).unwrap();
    let (t, s) = (n.transform, n.size);
    apply_constrained_geometry(n, t, Vec2::new(s.x * 3.0, s.y));
    n.clone()
}

fn assert_cleared_after_resize(cap: &str) {
    let mut doc = import_rest_json(&rest_line(cap)).unwrap();
    let line = find(&doc, "Line");
    assert!(matches!(line.kind, NodeKind::Line));
    assert!(line.stroke_geometry.is_some(), "the import stores the outline");
    assert_eq!(line.fig_metadata.get("strokeCap").map(String::as_str), Some(cap), "the unmodelled cap is recorded");
    let after = resize_threefold(&mut doc, "Line");
    assert!(after.stroke_geometry.is_none(), "{cap}: the outline is cleared, not stretched");
}

#[test]
fn rest_line_arrow_resize_clears_the_outline() {
    assert_cleared_after_resize("LINE_ARROW");
}

#[test]
fn rest_triangle_arrow_resize_clears_the_outline() {
    assert_cleared_after_resize("TRIANGLE_ARROW");
}

#[test]
fn fig_line_with_a_decorative_cap_records_it_and_resize_clears_the_outline() {
    let changes = json!({
        "nodeChanges": [
            {"guid": {"sessionID": 0, "localID": 0}, "phase": "CREATED", "type": "DOCUMENT", "name": "Doc"},
            {"guid": {"sessionID": 0, "localID": 1}, "phase": "CREATED", "parentIndex": {"guid": {"sessionID": 0, "localID": 0}, "position": "!"}, "type": "CANVAS", "name": "Page"},
            {
                "guid": {"sessionID": 1, "localID": 1},
                "phase": "CREATED",
                "parentIndex": {"guid": {"sessionID": 0, "localID": 1}, "position": "!"},
                "type": "LINE",
                "name": "Line",
                "size": {"x": 100.0, "y": 0.0},
                "transform": {"m00": 1.0, "m01": 0.0, "m02": 40.0, "m10": 0.0, "m11": 1.0, "m12": 40.0},
                "strokeWeight": 10.0,
                "strokeCap": "HIGHLIGHT"
            }
        ]
    });
    let mut doc = import_fig_node_changes(&changes, "doc").expect("import");
    let id = find(&doc, "Line").id;
    assert_eq!(find(&doc, "Line").fig_metadata.get("strokeCap").map(String::as_str), Some("HIGHLIGHT"));
    let band = vec![Path {
        cmds: vec![
            PathCmd::MoveTo(Vec2::new(0.0, -10.0)),
            PathCmd::LineTo(Vec2::new(100.0, -10.0)),
            PathCmd::LineTo(Vec2::new(100.0, 0.0)),
            PathCmd::LineTo(Vec2::new(0.0, 0.0)),
            PathCmd::Close,
        ],
        fill_rule: FillRule::NonZero,
    }];
    doc.get_mut(id).unwrap().stroke_geometry = Some(band);
    let after = resize_threefold(&mut doc, "Line");
    assert!(after.stroke_geometry.is_none(), "the decorative cap drops the outline");
}

#[test]
fn rest_round_cap_line_still_stretches() {
    let mut doc = import_rest_json(&rest_line("ROUND")).unwrap();
    assert!(!find(&doc, "Line").fig_metadata.contains_key("strokeCap"));
    let after = resize_threefold(&mut doc, "Line");
    let xs: Vec<f32> = after
        .stroke_geometry
        .as_ref()
        .expect("a round cap keeps the outline")
        .iter()
        .flat_map(|p| p.cmds.iter())
        .filter_map(|c| match c {
            PathCmd::LineTo(p) => Some(p.x),
            _ => None,
        })
        .collect();
    assert!(xs.contains(&300.0), "the band end follows the line to 300, got {xs:?}");
}

#[test]
fn rest_export_writes_line_arrow_back_unchanged() {
    let doc = import_rest_json(&rest_line("LINE_ARROW")).unwrap();
    let out: Value = export_rest_json(&doc);
    let mut found = None;
    fn walk(v: &Value, found: &mut Option<String>) {
        if v.get("type").and_then(Value::as_str) == Some("LINE") {
            *found = v.get("strokeCap").and_then(Value::as_str).map(str::to_owned);
        }
        for c in v.get("children").and_then(Value::as_array).into_iter().flatten() {
            walk(c, found);
        }
    }
    walk(&out["document"], &mut found);
    assert_eq!(found.as_deref(), Some("LINE_ARROW"));
}
