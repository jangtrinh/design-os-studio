//! REST import/export of precomputed geometry, style metadata, linear blends and text truncation.
use designos_doc::*;
use designos_fig::{export_rest_json, import_rest_json};

const FILE: &str = r##"{
  "name": "Upgrade",
  "styles": {
    "S:1": {"key": "abc123", "name": "Brand/Primary", "description": "Main brand fill", "styleType": "FILL"},
    "S:2": {"key": "def456", "name": "Border/Strong", "description": "", "styleType": "FILL"},
    "S:3": {"key": "ghi789", "name": "Body", "description": "Body text", "styleType": "TEXT"},
    "S:4": {"name": "Soft shadow", "styleType": "EFFECT"},
    "S:5": {"name": "Columns", "styleType": "GRID"}
  },
  "document": {"id": "0:0", "type": "DOCUMENT", "children": [{"id": "0:1", "type": "CANVAS", "name": "P", "children": [
    {"id": "1:1", "type": "BOOLEAN_OPERATION", "name": "Bool", "booleanOperation": "SUBTRACT",
     "absoluteBoundingBox": {"x": 0, "y": 0, "width": 20, "height": 20},
     "fills": [{"type": "SOLID", "color": {"r": 1, "g": 0, "b": 0, "a": 1}, "blendMode": "LINEAR_BURN"}],
     "styles": {"fill": "S:1", "stroke": "S:2"},
     "strokes": [{"type": "SOLID", "color": {"r": 0, "g": 0, "b": 0, "a": 1}}],
     "fillGeometry": [{"path": "M 0 0 L 20 0 L 20 20 Z", "windingRule": "EVENODD"}],
     "strokeGeometry": [{"path": "M 0 0 L 20 0 L 20 20 L 0 20 Z", "windingRule": "NONZERO"}],
     "children": []},
    {"id": "1:2", "type": "STAR", "name": "Star", "pointCount": 5, "blendMode": "LINEAR_DODGE",
     "absoluteBoundingBox": {"x": 30, "y": 0, "width": 20, "height": 20},
     "styles": {"stroke": "S:2"},
     "fillGeometry": [{"path": "M 0 0 L 5 5 L 0 10 Z", "windingRule": "NONZERO"}]},
    {"id": "1:3", "type": "REGULAR_POLYGON", "name": "Poly", "pointCount": 6,
     "absoluteBoundingBox": {"x": 60, "y": 0, "width": 20, "height": 20},
     "fillGeometry": [{"path": "M 1 1 L 2 2 L 3 1 Z", "windingRule": "NONZERO"}],
     "strokeGeometry": []},
    {"id": "1:4", "type": "VECTOR", "name": "Vec",
     "absoluteBoundingBox": {"x": 90, "y": 0, "width": 20, "height": 20},
     "fillGeometry": [{"path": "M 0 0 L 4 0 L 4 4 Z", "windingRule": "NONZERO"}],
     "strokeGeometry": [{"path": "M 0 0 L 4 4", "windingRule": "NONZERO"}]},
    {"id": "1:5", "type": "RECTANGLE", "name": "Plain", "absoluteBoundingBox": {"x": 0, "y": 40, "width": 10, "height": 10}},
    {"id": "1:6", "type": "TEXT", "name": "Clamp", "characters": "hello world",
     "absoluteBoundingBox": {"x": 0, "y": 60, "width": 80, "height": 40},
     "styles": {"text": "S:3"},
     "style": {"fontFamily": "Inter", "fontSize": 12, "textAutoResize": "NONE", "textTruncation": "ENDING", "maxLines": 2},
     "fills": [{"type": "SOLID", "color": {"r": 0, "g": 0, "b": 0, "a": 1}}]},
    {"id": "1:7", "type": "TEXT", "name": "Free", "characters": "x",
     "absoluteBoundingBox": {"x": 0, "y": 120, "width": 80, "height": 40},
     "style": {"fontSize": 12, "textTruncation": "DISABLED"}},
    {"id": "1:8", "type": "FRAME", "name": "Frame", "absoluteBoundingBox": {"x": 0, "y": 200, "width": 100, "height": 100},
     "styles": {"effect": "S:4", "grid": "S:5"},
     "effects": [{"type": "LAYER_BLUR", "radius": 2, "visible": true}],
     "layoutGrids": [{"pattern": "COLUMNS", "count": 4, "gutterSize": 8, "offset": 2, "visible": true, "color": {"r": 1, "g": 0, "b": 0, "a": 0.1}}]}
  ]}]}
}"##;

fn find<'a>(doc: &'a Document, name: &str) -> &'a Node {
    doc.nodes.values().find(|n| n.name == name).unwrap_or_else(|| panic!("no node named {name}"))
}

#[test]
fn geometry_is_stored_on_every_node_that_has_it() {
    let doc = import_rest_json(FILE).unwrap();
    let b = find(&doc, "Bool");
    let fg = b.fill_geometry.as_ref().unwrap();
    assert_eq!(fg.len(), 1);
    assert_eq!(fg[0].fill_rule, FillRule::EvenOdd);
    assert_eq!(fg[0].cmds[1], PathCmd::LineTo(Vec2::new(20.0, 0.0)));
    assert_eq!(b.stroke_geometry.as_ref().unwrap()[0].fill_rule, FillRule::NonZero);
    assert!(find(&doc, "Star").fill_geometry.is_some());
    assert!(find(&doc, "Star").stroke_geometry.is_none());
    let poly = find(&doc, "Poly");
    assert!(poly.fill_geometry.is_some());
    assert!(poly.stroke_geometry.is_none(), "empty list means none");
    let v = find(&doc, "Vec");
    assert_eq!(v.fill_geometry.as_ref().unwrap().len(), 1);
    assert!(v.stroke_geometry.is_some());
    assert!(matches!(&v.kind, NodeKind::Vector { paths, .. } if paths == v.fill_geometry.as_ref().unwrap()));
    let plain = find(&doc, "Plain");
    assert!(plain.fill_geometry.is_none() && plain.stroke_geometry.is_none());
}

#[test]
fn geometry_round_trips_through_export() {
    let a = import_rest_json(FILE).unwrap();
    let json = export_rest_json(&a);
    let b = import_rest_json(&json.to_string()).unwrap();
    assert_eq!(a.nodes, b.nodes);
    assert_eq!(a.style_meta, b.style_meta);
    assert_eq!(json, export_rest_json(&b));
    let exported_bool = json["document"]["children"][0]["children"][0].clone();
    assert_eq!(exported_bool["fillGeometry"][0]["windingRule"], "EVENODD");
    assert!(exported_bool["strokeGeometry"][0]["path"].as_str().unwrap().starts_with('M'));
}

#[test]
fn style_metadata_and_property_per_use() {
    let doc = import_rest_json(FILE).unwrap();
    let m = &doc.style_meta;
    assert_eq!(
        m["S:1"],
        StyleMeta {
            name: "Brand/Primary".into(),
            key: Some("abc123".into()),
            description: "Main brand fill".into(),
            property: StyleProperty::Fill,
            bindings: Default::default(),
        }
    );
    // S:2 is a FILL style that is only referenced as a stroke.
    assert_eq!(m["S:2"].property, StyleProperty::Stroke);
    assert_eq!(m["S:2"].name, "Border/Strong");
    assert_eq!(m["S:3"].property, StyleProperty::Text);
    assert_eq!(m["S:3"].description, "Body text");
    assert_eq!(m["S:4"].property, StyleProperty::Effect);
    assert_eq!(m["S:4"].key, None);
    assert_eq!(m["S:5"].property, StyleProperty::Grid);
    assert_eq!(doc.style_name("S:1"), "Brand/Primary");
    // Export writes names/keys back.
    let json = export_rest_json(&doc);
    assert_eq!(json["styles"]["S:1"]["name"], "Brand/Primary");
    assert_eq!(json["styles"]["S:1"]["key"], "abc123");
    assert_eq!(json["styles"]["S:3"]["styleType"], "TEXT");
    assert!(json["styles"]["S:4"].get("key").is_none());
}

#[test]
fn linear_blend_modes() {
    let doc = import_rest_json(FILE).unwrap();
    assert_eq!(find(&doc, "Bool").fills[0].blend, BlendMode::LinearBurn);
    assert_eq!(find(&doc, "Star").blend, BlendMode::LinearDodge);
    let json = export_rest_json(&doc);
    assert_eq!(json["document"]["children"][0]["children"][1]["blendMode"], "LINEAR_DODGE");
}

#[test]
fn text_truncation_and_max_lines() {
    let doc = import_rest_json(FILE).unwrap();
    let NodeKind::Text(t) = &find(&doc, "Clamp").kind else { panic!() };
    assert_eq!(t.auto_resize, TextAutoResize::Truncate);
    assert_eq!(t.max_lines, Some(2));
    let NodeKind::Text(f) = &find(&doc, "Free").kind else { panic!() };
    assert_eq!(f.auto_resize, TextAutoResize::None);
    assert_eq!(f.max_lines, None);
    let again = import_rest_json(&export_rest_json(&doc).to_string()).unwrap();
    let NodeKind::Text(t2) = &find(&again, "Clamp").kind else { panic!() };
    assert_eq!((t2.auto_resize, t2.max_lines), (TextAutoResize::Truncate, Some(2)));
}

#[test]
fn grids_and_invisible_grids() {
    let doc = import_rest_json(FILE).unwrap();
    let f = find(&doc, "Frame").kind.frame_props().unwrap();
    assert!(matches!(f.grids[0], LayoutGrid::Columns { count: 4, gutter, offset, visible: true, .. } if gutter == 8.0 && offset == 2.0));
    let hidden = r#"{"document":{"type":"DOCUMENT","children":[{"type":"CANVAS","children":[{"type":"FRAME","layoutGrids":[{"pattern":"GRID","sectionSize":8,"visible":false}]}]}]}}"#;
    let d = import_rest_json(hidden).unwrap();
    let grids = &d.nodes.values().find_map(|n| n.kind.frame_props()).unwrap().grids;
    assert_eq!(grids.len(), 1);
    assert!(matches!(grids[0], LayoutGrid::Grid { size, visible: false, .. } if size == 8.0));
}
