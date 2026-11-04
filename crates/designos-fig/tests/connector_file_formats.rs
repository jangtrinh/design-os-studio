use designos_doc::vector::{Segment, Vertex};
use designos_doc::*;
use designos_fig::container::{self, Compression};
use designos_fig::kiwi::{KiwiValue, Schema};
use designos_fig::testkit::figma_like_schema;
use designos_fig::{decode_fig_to_json, export_fig, export_rest_json, import_fig, import_fig_node_changes, import_rest_json};
use serde_json::{json, Value};

fn text_props(s: &str) -> TextProps {
    TextProps { content: s.into(), ..Default::default() }
}

fn build_fig_from_message(schema: &Schema, msg: &KiwiValue) -> Vec<u8> {
    let root = schema.find("Message").unwrap();
    let body = schema.encode_message(root, msg).unwrap();
    container::write_container(&schema.encode(), &body, Compression::Deflate, 101)
}

fn find<'a>(doc: &'a Document, name: &str) -> &'a Node {
    doc.nodes.values().find(|n| n.name == name).unwrap_or_else(|| panic!("no node named {name}"))
}

fn decoded_changes(bytes: &[u8]) -> Vec<Value> {
    let v = decode_fig_to_json(bytes).expect("decode_fig_to_json");
    v["message"]["nodeChanges"].as_array().expect("nodeChanges").clone()
}

fn change_named<'a>(changes: &'a [Value], name: &str) -> &'a Value {
    changes.iter().find(|n| n["name"] == name).unwrap_or_else(|| panic!("no nodeChange named {name}"))
}

fn number_list(v: &Value) -> Vec<f64> {
    v.as_array().map(|a| a.iter().filter_map(Value::as_f64).collect()).unwrap_or_default()
}

fn studio_carrier_value<'a>(nc: &'a Value, field: &str) -> Option<&'a str> {
    let key = format!("field:{field}");
    nc["pluginData"].as_array()?.iter().find(|e| e["pluginID"] == "designos-studio" && e["key"] == key)?.get("value")?.as_str()
}

fn design_doc_with_connector(label: Option<&str>) -> (Document, NodeId, NodeId, NodeId) {
    let mut doc = Document::new("design-connectors");
    doc.kind = DocumentKind::Design;
    let page = doc.add_page("Page 1");
    // Allocate a few ids first so the frame ids in the source document differ from the
    // sequential ids allocated when importing into a fresh document.
    for _ in 0..4 {
        let _ = doc.alloc_id();
    }
    let a = doc.alloc_id();
    let mut fa = Node::frame(a, Rect::new(40.0, 60.0, 120.0, 80.0));
    fa.name = "Frame A".into();
    doc.insert(page, fa).unwrap();

    let b = doc.alloc_id();
    let mut fb = Node::frame(b, Rect::new(320.0, 180.0, 140.0, 90.0));
    fb.name = "Frame B".into();
    doc.insert(page, fb).unwrap();

    let c = doc.alloc_id();
    let props = ConnectorProps {
        start: ConnectorEnd::attached(a, Magnet::Right),
        end: ConnectorEnd::attached(b, Magnet::Left),
        line: ConnectorLine::Elbowed,
        start_cap: ConnectorCap::Circle,
        end_cap: ConnectorCap::Triangle,
        label: label.map(text_props),
    };
    let mut cn = Node::connector(c, props);
    cn.name = "Flow Arrow".into();
    cn.stroke.weight = 3.0;
    cn.stroke.paints = vec![Paint::solid(Color::rgb(0x1D, 0x4E, 0xD8))];
    doc.insert(page, cn).unwrap();
    sync_connector(&mut doc, c);
    (doc, a, b, c)
}

#[test]
fn fig_design_export_writes_vector_with_caps_and_carrier() {
    let (doc, _a, _b, _c) = design_doc_with_connector(None);
    let bytes = export_fig(&doc).expect("export_fig");
    let changes = decoded_changes(&bytes);

    assert!(changes.iter().all(|n| n["type"] != "CONNECTOR"), "design export must not emit CONNECTOR records");
    let vec_nc = change_named(&changes, "Flow Arrow");
    assert_eq!(vec_nc["type"], "VECTOR");

    let table = vec_nc["vectorData"]["styleOverrideTable"].as_array().expect("styleOverrideTable on exported connector vector");
    let caps: Vec<&str> = table.iter().filter_map(|e| e["strokeCap"].as_str()).collect();
    assert!(caps.contains(&"CIRCLE_FILLED") && caps.contains(&"ARROW_EQUILATERAL"), "expected per-vertex start/end caps in styleOverrideTable, got {caps:?}");

    let raw_carrier = studio_carrier_value(vec_nc, "studioConnector").expect("field:studioConnector present");
    let carried: Value = serde_json::from_str(raw_carrier).expect("valid carrier JSON");
    assert_eq!(carried["line"], "Elbowed");
    assert_eq!(carried["start_cap"], "Circle");
    assert_eq!(carried["end_cap"], "Triangle");
    assert_eq!(carried["strokeWeight"], 3.0);

    // REST JSON export for a design file also emits a VECTOR with strokeGeometry, caps, and carrier.
    let rest = export_rest_json(&doc);
    let page_children = rest["document"]["children"][0]["children"].as_array().expect("page children");
    let rest_vec = page_children.iter().find(|n| n["name"] == "Flow Arrow").expect("Flow Arrow in REST export");
    assert_eq!(rest_vec["type"], "VECTOR");
    assert!(rest_vec["strokeGeometry"].as_array().is_some_and(|a| !a.is_empty()), "REST vector export must include strokeGeometry");
    let verts = rest_vec["vectorNetwork"]["vertices"].as_array().expect("vectorNetwork vertices");
    assert_eq!(verts.first().unwrap()["strokeCap"], "CIRCLE_FILLED");
    assert_eq!(verts.last().unwrap()["strokeCap"], "TRIANGLE_FILLED");
}

#[test]
fn fig_design_export_keeps_label_as_group() {
    let (mut doc, a, b, c) = design_doc_with_connector(Some("approves"));
    {
        let stroke = &mut doc.get_mut(c).expect("connector node").stroke;
        stroke.join = StrokeJoin::Round;
        stroke.cap = StrokeCap::Round;
        stroke.dashes = vec![8.0, 4.0];
    }
    // Add a second connector with an empty label: it must export as a bare VECTOR, not a GROUP.
    let empty_id = doc.alloc_id();
    let mut empty_cn = Node::connector(
        empty_id,
        ConnectorProps {
            start: ConnectorEnd::attached(a, Magnet::Bottom),
            end: ConnectorEnd::attached(b, Magnet::Bottom),
            line: ConnectorLine::Straight,
            start_cap: ConnectorCap::None,
            end_cap: ConnectorCap::Arrow,
            label: Some(text_props("")),
        },
    );
    empty_cn.name = "Empty Label Flow".into();
    doc.insert(doc.pages[0], empty_cn).unwrap();
    sync_connector(&mut doc, empty_id);

    let bytes = export_fig(&doc).expect("export_fig");
    let changes = decoded_changes(&bytes);

    let grp = change_named(&changes, "Flow Arrow");
    assert_eq!(grp["type"], "GROUP", "labelled connector exports as GROUP");
    let grp_guid = &grp["guid"];
    let kids: Vec<&Value> = changes.iter().filter(|n| &n["parentIndex"]["guid"] == grp_guid).collect();
    assert_eq!(kids.len(), 3, "group of three: vector, chip rectangle, text");
    let kid_types: Vec<&str> = kids.iter().filter_map(|k| k["type"].as_str()).collect();
    assert_eq!(kid_types, vec!["VECTOR", "ROUNDED_RECTANGLE", "TEXT"]);
    assert_eq!(kids[2]["textData"]["characters"], "approves");
    assert_eq!(kids[0]["strokeJoin"], "ROUND", ".fig label-group vector keeps the stroke join");
    assert_eq!(number_list(&kids[0]["dashPattern"]), vec![8.0, 4.0], ".fig label-group vector keeps the dash pattern");

    // The REST group carries the same stroke on its vector child.
    let rest = export_rest_json(&doc);
    let rest_page = rest["document"]["children"][0]["children"].as_array().expect("REST page children");
    let rest_grp = rest_page.iter().find(|n| n["name"] == "Flow Arrow").expect("Flow Arrow in REST export");
    assert_eq!(rest_grp["type"], "GROUP");
    let rest_kids = rest_grp["children"].as_array().expect("REST group children");
    let rest_kid_types: Vec<&str> = rest_kids.iter().filter_map(|k| k["type"].as_str()).collect();
    assert_eq!(rest_kid_types, vec!["VECTOR", "RECTANGLE", "TEXT"]);
    let rest_vec = &rest_kids[0];
    assert_eq!(rest_vec["strokeJoin"], "ROUND", "REST label-group vector keeps the stroke join");
    assert_eq!(rest_vec["strokeCap"], "ROUND", "REST label-group vector keeps the node stroke cap");
    assert_eq!(number_list(&rest_vec["strokeDashes"]), vec![8.0, 4.0], "REST label-group vector keeps the dash pattern");

    let bare = change_named(&changes, "Empty Label Flow");
    assert_eq!(bare["type"], "VECTOR", "empty label exports the bare vector");
    let bare_guid = &bare["guid"];
    let bare_kids = changes.iter().filter(|n| &n["parentIndex"]["guid"] == bare_guid).count();
    assert_eq!(bare_kids, 0);

    // Importing the exported design file restores the group as one live connector with no stray chip or text.
    let back = import_fig(&bytes).expect("import_fig");
    let restored = find(&back, "Flow Arrow");
    let NodeKind::Connector(cp) = &restored.kind else {
        panic!("expected live Connector, got {:?}", restored.kind);
    };
    assert_eq!(cp.label.as_ref().map(|l| l.content.as_str()), Some("approves"));
    assert!(restored.children.is_empty());
    assert!(
        back.nodes.values().all(|n| !matches!(n.kind, NodeKind::Group | NodeKind::Text(_) | NodeKind::Rectangle)),
        "no stray group, chip rectangle, or text node in restored document"
    );
}

/// One connector, elbowed, with the given arrowheads and node stroke cap, placed between two frames.
fn design_doc_with_connector_caps(label: Option<&str>, start: ConnectorCap, end: ConnectorCap, node_cap: StrokeCap) -> Document {
    let (mut doc, _a, _b, c) = design_doc_with_connector(label);
    if let Some(NodeKind::Connector(p)) = doc.get_mut(c).map(|n| &mut n.kind) {
        p.start_cap = start;
        p.end_cap = end;
    }
    doc.get_mut(c).expect("connector node").stroke.cap = node_cap;
    sync_connector(&mut doc, c);
    doc
}

/// The caps in a `.fig` vector's style table, in vertex order.
fn fig_vertex_caps(vector_nc: &Value) -> Vec<String> {
    let table = vector_nc["vectorData"]["styleOverrideTable"].as_array();
    table.map(|t| t.iter().filter_map(|e| e["strokeCap"].as_str().map(str::to_owned)).collect()).unwrap_or_default()
}

/// The vector carrying the connector's line in a `.fig`: the node itself, or the first child of its label group.
fn fig_connector_vector(changes: &[Value], labelled: bool) -> &Value {
    let top = change_named(changes, "Flow Arrow");
    if !labelled {
        assert_eq!(top["type"], "VECTOR");
        return top;
    }
    assert_eq!(top["type"], "GROUP");
    changes.iter().find(|n| n["parentIndex"]["guid"] == top["guid"] && n["type"] == "VECTOR").expect("label group vector")
}

fn rest_vertex_cap(vertex: Option<&Value>) -> Option<&str> {
    vertex.and_then(|v| v["strokeCap"].as_str())
}

#[test]
fn fig_open_connector_ends_take_the_node_stroke_cap() {
    for label in [None, Some("approves")] {
        let labelled = label.is_some();
        // (start arrowhead, end arrowhead, node cap, expected caps at the first and last vertex, expected node-level cap)
        let cases: [(ConnectorCap, ConnectorCap, StrokeCap, &[&str], &str); 3] = [
            (ConnectorCap::None, ConnectorCap::None, StrokeCap::Round, &["ROUND", "ROUND"], "ROUND"),
            (ConnectorCap::None, ConnectorCap::Arrow, StrokeCap::Round, &["ROUND", "ARROW_LINES"], "ROUND"),
            (ConnectorCap::None, ConnectorCap::None, StrokeCap::None, &[], "NONE"),
        ];
        for (start, end, node_cap, expected_vertex_caps, expected_node_cap) in cases {
            let doc = design_doc_with_connector_caps(label, start, end, node_cap);
            let changes = decoded_changes(&export_fig(&doc).expect("export_fig"));
            let vector = fig_connector_vector(&changes, labelled);
            let ctx = format!("labelled={labelled} start={start:?} end={end:?} node_cap={node_cap:?}");
            assert_eq!(fig_vertex_caps(vector), expected_vertex_caps, ".fig vertex caps, {ctx}");
            assert_eq!(vector["strokeCap"], expected_node_cap, ".fig node-level cap, {ctx}");
        }
    }
}

#[test]
fn rest_open_connector_ends_take_the_node_stroke_cap() {
    for label in [None, Some("approves")] {
        let labelled = label.is_some();
        // (start arrowhead, end arrowhead, node cap, expected caps at the first and last vertex, expected node-level cap)
        let cases = [
            (ConnectorCap::None, ConnectorCap::None, StrokeCap::Round, [Some("ROUND"), Some("ROUND")], "ROUND"),
            (ConnectorCap::None, ConnectorCap::Arrow, StrokeCap::Round, [Some("ROUND"), Some("ARROW_LINES")], "ROUND"),
            (ConnectorCap::None, ConnectorCap::None, StrokeCap::None, [None, None], "NONE"),
        ];
        for (start, end, node_cap, expected_vertex_caps, expected_node_cap) in cases {
            let doc = design_doc_with_connector_caps(label, start, end, node_cap);
            let rest = export_rest_json(&doc);
            let page_children = rest["document"]["children"][0]["children"].as_array().expect("page children");
            let top = page_children.iter().find(|n| n["name"] == "Flow Arrow").expect("Flow Arrow in REST export");
            let vector = if labelled { &top["children"][0] } else { top };
            let ctx = format!("labelled={labelled} start={start:?} end={end:?} node_cap={node_cap:?}");
            assert_eq!(vector["type"], "VECTOR", "{ctx}");
            let vertices = vector["vectorNetwork"]["vertices"].as_array().expect("vectorNetwork vertices");
            assert_eq!([rest_vertex_cap(vertices.first()), rest_vertex_cap(vertices.last())], expected_vertex_caps, "REST vertex caps, {ctx}");
            assert_eq!(vector["strokeCap"], expected_node_cap, "REST node-level cap, {ctx}");
        }
    }
}

#[test]
fn fig_design_round_trip_restores_live_connector() {
    let (doc, a, b, c) = design_doc_with_connector(Some("yes"));
    let orig = doc.get(c).unwrap();
    let NodeKind::Connector(orig_props) = &orig.kind else { unreachable!() };

    // 1. .fig round trip
    let bytes = export_fig(&doc).expect("export_fig");
    let changes = decoded_changes(&bytes);
    assert!(changes.iter().all(|n| n["type"] != "CONNECTOR"), "design .fig must not contain CONNECTOR");
    let back_fig = import_fig(&bytes).expect("import_fig");
    let a_fig = find(&back_fig, "Frame A").id;
    let b_fig = find(&back_fig, "Frame B").id;
    assert_ne!(a_fig, a, "fresh import renumbers Frame A");
    assert_ne!(b_fig, b, "fresh import renumbers Frame B");

    let conn_fig = find(&back_fig, "Flow Arrow");
    let NodeKind::Connector(p_fig) = &conn_fig.kind else {
        panic!("expected live Connector after .fig round trip, got {:?}", conn_fig.kind);
    };
    assert_eq!(p_fig.start.node, Some(a_fig));
    assert_eq!(p_fig.end.node, Some(b_fig));
    assert_eq!(p_fig.start.magnet, orig_props.start.magnet);
    assert_eq!(p_fig.end.magnet, orig_props.end.magnet);
    assert_eq!(p_fig.line, orig_props.line);
    assert_eq!(p_fig.start_cap, orig_props.start_cap);
    assert_eq!(p_fig.end_cap, orig_props.end_cap);
    assert_eq!(p_fig.label, orig_props.label);
    assert_eq!(conn_fig.stroke.weight, 3.0);
    assert_eq!(conn_fig.stroke.paints, vec![Paint::solid(Color::rgb(0x1D, 0x4E, 0xD8))]);

    // 2. REST JSON round trip ( including a curved connector without a label )
    let (mut doc2, a2, b2, c2) = design_doc_with_connector(None);
    if let Some(NodeKind::Connector(p)) = doc2.get_mut(c2).map(|n| &mut n.kind) {
        p.line = ConnectorLine::Curved;
        p.start_cap = ConnectorCap::Diamond;
        p.end_cap = ConnectorCap::Arrow;
    }
    sync_connector(&mut doc2, c2);

    let rest_val = export_rest_json(&doc2);
    let rest_str = serde_json::to_string(&rest_val).unwrap();
    assert!(!rest_str.contains("\"type\":\"CONNECTOR\""));
    let back_rest = import_rest_json(&rest_str).expect("import_rest_json");
    let a_rest = find(&back_rest, "Frame A").id;
    let b_rest = find(&back_rest, "Frame B").id;
    assert_ne!(a_rest, a2);
    assert_ne!(b_rest, b2);

    let conn_rest = find(&back_rest, "Flow Arrow");
    let NodeKind::Connector(p_rest) = &conn_rest.kind else {
        panic!("expected live Connector after REST round trip, got {:?}", conn_rest.kind);
    };
    assert_eq!(p_rest.start.node, Some(a_rest));
    assert_eq!(p_rest.end.node, Some(b_rest));
    assert_eq!(p_rest.line, ConnectorLine::Curved);
    assert_eq!(p_rest.start_cap, ConnectorCap::Diamond);
    assert_eq!(p_rest.end_cap, ConnectorCap::Arrow);
    assert_eq!(conn_rest.stroke.weight, 3.0);
}

#[test]
fn fig_keeps_unmodelled_magnets_and_text_midpoint_and_control_points() {
    let schema: Schema = figma_like_schema();
    let guid = |s: u32, l: u32| KiwiValue::Struct(vec![("sessionID".into(), KiwiValue::Uint(s)), ("localID".into(), KiwiValue::Uint(l))]);
    let vec2 = |x: f32, y: f32| KiwiValue::Struct(vec![("x".into(), KiwiValue::Float(x)), ("y".into(), KiwiValue::Float(y))]);
    let p_idx = |s: u32, l: u32, pos: &str| KiwiValue::Struct(vec![("guid".into(), guid(s, l)), ("position".into(), KiwiValue::String(pos.into()))]);
    let en = |s: &str| KiwiValue::Enum(s.into());

    let msg = KiwiValue::Message(vec![
        ("type".into(), en("NODE_CHANGES")),
        ("sessionID".into(), KiwiValue::Uint(0)),
        ("ackID".into(), KiwiValue::Uint(0)),
        (
            "nodeChanges".into(),
            KiwiValue::Array(vec![
                KiwiValue::Message(vec![
                    ("guid".into(), guid(0, 0)),
                    ("phase".into(), en("CREATED")),
                    ("type".into(), en("DOCUMENT")),
                    ("name".into(), KiwiValue::String("Board".into())),
                ]),
                KiwiValue::Message(vec![
                    ("guid".into(), guid(0, 1)),
                    ("phase".into(), en("CREATED")),
                    ("parentIndex".into(), p_idx(0, 0, "!")),
                    ("type".into(), en("CANVAS")),
                    ("name".into(), KiwiValue::String("Page 1".into())),
                ]),
                KiwiValue::Message(vec![
                    ("guid".into(), guid(1, 10)),
                    ("phase".into(), en("CREATED")),
                    ("parentIndex".into(), p_idx(0, 1, "!")),
                    ("type".into(), en("STICKY")),
                    ("name".into(), KiwiValue::String("Sticky A".into())),
                    ("size".into(), vec2(200.0, 200.0)),
                ]),
                KiwiValue::Message(vec![
                    ("guid".into(), guid(1, 11)),
                    ("phase".into(), en("CREATED")),
                    ("parentIndex".into(), p_idx(0, 1, "\"")),
                    ("type".into(), en("STICKY")),
                    ("name".into(), KiwiValue::String("Sticky B".into())),
                    ("size".into(), vec2(200.0, 200.0)),
                ]),
                KiwiValue::Message(vec![
                    ("guid".into(), guid(1, 12)),
                    ("phase".into(), en("CREATED")),
                    ("parentIndex".into(), p_idx(0, 1, "#")),
                    ("type".into(), en("CONNECTOR")),
                    ("name".into(), KiwiValue::String("Diagram Edge".into())),
                    ("connectorLineStyle".into(), en("ELBOWED")),
                    ("connectorType".into(), en("DIAGRAM")),
                    (
                        "connectorStart".into(),
                        KiwiValue::Message(vec![
                            ("endpointNodeID".into(), guid(1, 10)),
                            ("position".into(), vec2(200.0, 100.0)),
                            ("magnet".into(), en("EDGE")),
                            ("relativePosition".into(), vec2(0.25, 0.75)),
                        ]),
                    ),
                    (
                        "connectorEnd".into(),
                        KiwiValue::Message(vec![
                            ("endpointNodeID".into(), guid(1, 11)),
                            ("position".into(), vec2(400.0, 100.0)),
                            ("magnet".into(), en("AUTO_HORIZONTAL")),
                            ("relativePosition".into(), vec2(0.5, 0.0)),
                        ]),
                    ),
                    (
                        "connectorTextMidpoint".into(),
                        KiwiValue::Message(vec![("section".into(), en("MIDDLE_TO_END")), ("offset".into(), KiwiValue::Float(0.5))]),
                    ),
                    (
                        "connectorControlPoints".into(),
                        KiwiValue::Array(vec![KiwiValue::Message(vec![("position".into(), vec2(300.0, 100.0)), ("axis".into(), vec2(1.0, 0.0))])]),
                    ),
                ]),
                KiwiValue::Message(vec![
                    ("guid".into(), guid(1, 13)),
                    ("phase".into(), en("CREATED")),
                    ("parentIndex".into(), p_idx(0, 1, "$")),
                    ("type".into(), en("CONNECTOR")),
                    ("name".into(), KiwiValue::String("Free Ends".into())),
                    ("connectorLineStyle".into(), en("STRAIGHT")),
                    (
                        "connectorStart".into(),
                        KiwiValue::Message(vec![("endpointNodeID".into(), guid(1, 10)), ("position".into(), vec2(50.0, 60.0)), ("magnet".into(), en("NONE"))]),
                    ),
                    (
                        "connectorEnd".into(),
                        KiwiValue::Message(vec![
                            ("endpointNodeID".into(), guid(1, 11)),
                            ("position".into(), vec2(250.0, 160.0)),
                            ("magnet".into(), en("ABSOLUTE")),
                        ]),
                    ),
                ]),
            ]),
        ),
        ("blobs".into(), KiwiValue::Array(vec![])),
    ]);

    let bytes = build_fig_from_message(&schema, &msg);
    let mut doc = import_fig(&bytes).expect("import_fig");
    doc.kind = DocumentKind::Board;

    let sticky_a = find(&doc, "Sticky A").id;
    let sticky_b = find(&doc, "Sticky B").id;
    let edge = find(&doc, "Diagram Edge");
    let NodeKind::Connector(cp) = &edge.kind else {
        panic!("expected Connector, got {:?}", edge.kind);
    };
    // EDGE and AUTO_HORIZONTAL map to Magnet::Auto while keeping the endpoint attachments.
    assert_eq!(cp.start.node, Some(sticky_a));
    assert_eq!(cp.start.magnet, Magnet::Auto);
    assert_eq!(cp.end.node, Some(sticky_b));
    assert_eq!(cp.end.magnet, Magnet::Auto);
    assert_eq!(edge.fig_metadata.get("connectorStartMagnet").map(String::as_str), Some("EDGE"));
    assert_eq!(edge.fig_metadata.get("connectorEndMagnet").map(String::as_str), Some("AUTO_HORIZONTAL"));
    assert!(edge.fig_metadata.contains_key("connectorStartRelativePosition"));
    assert!(edge.fig_metadata.contains_key("connectorEndRelativePosition"));
    assert!(edge.fig_metadata.contains_key("connectorTextMidpoint"));
    assert!(edge.fig_metadata.contains_key("connectorControlPoints"));
    assert_eq!(edge.fig_metadata.get("connectorType").map(String::as_str), Some("DIAGRAM"));

    // NONE and ABSOLUTE map to a free end at `position`.
    let free = find(&doc, "Free Ends");
    let NodeKind::Connector(fcp) = &free.kind else {
        panic!("expected Connector");
    };
    assert_eq!(fcp.start.node, None);
    assert_eq!(fcp.start.point, Vec2::new(50.0, 60.0));
    assert_eq!(fcp.end.node, None);
    assert_eq!(fcp.end.point, Vec2::new(250.0, 160.0));
    assert_eq!(free.fig_metadata.get("connectorStartMagnet").map(String::as_str), Some("NONE"));
    assert_eq!(free.fig_metadata.get("connectorEndMagnet").map(String::as_str), Some("ABSOLUTE"));

    // Export the board back to .fig and verify verbatim write-back + round-trip survival.
    let reexported = export_fig(&doc).expect("export_fig");
    let changes = decoded_changes(&reexported);
    let edge_nc = change_named(&changes, "Diagram Edge");
    assert_eq!(edge_nc["type"], "CONNECTOR");
    assert_eq!(edge_nc["connectorType"], "DIAGRAM");
    assert_eq!(edge_nc["connectorStart"]["magnet"], "EDGE");
    assert_eq!(edge_nc["connectorStart"]["relativePosition"]["x"], 0.25);
    assert_eq!(edge_nc["connectorStart"]["relativePosition"]["y"], 0.75);
    assert_eq!(edge_nc["connectorEnd"]["magnet"], "AUTO_HORIZONTAL");
    assert_eq!(edge_nc["connectorEnd"]["relativePosition"]["x"], 0.5);
    assert_eq!(edge_nc["connectorEnd"]["relativePosition"]["y"], 0.0);
    assert_eq!(edge_nc["connectorTextMidpoint"]["section"], "MIDDLE_TO_END");
    assert_eq!(edge_nc["connectorTextMidpoint"]["offset"], 0.5);
    assert_eq!(edge_nc["connectorControlPoints"][0]["position"]["x"], 300.0);

    let roundtripped = import_fig(&reexported).expect("re-import board .fig");
    let edge_rt = find(&roundtripped, "Diagram Edge");
    for key in [
        "connectorStartMagnet",
        "connectorEndMagnet",
        "connectorStartRelativePosition",
        "connectorEndRelativePosition",
        "connectorTextMidpoint",
        "connectorControlPoints",
        "connectorType",
    ] {
        assert_eq!(edge_rt.fig_metadata.get(key), edge.fig_metadata.get(key), "field {key} must survive board .fig round trip");
    }
}

#[test]
fn fig_import_does_not_read_connector_type_as_line_style() {
    let changes = json!({
        "nodeChanges": [
            {"guid": {"sessionID": 0, "localID": 0}, "phase": "CREATED", "type": "DOCUMENT", "name": "Doc"},
            {"guid": {"sessionID": 0, "localID": 1}, "phase": "CREATED", "parentIndex": {"guid": {"sessionID": 0, "localID": 0}, "position": "!"}, "type": "CANVAS", "name": "Page"},
            {
                "guid": {"sessionID": 1, "localID": 1},
                "phase": "CREATED",
                "parentIndex": {"guid": {"sessionID": 0, "localID": 1}, "position": "!"},
                "type": "CONNECTOR",
                "name": "Conn",
                "connectorType": "STRAIGHT",
                "connectorStart": {"position": {"x": 0.0, "y": 0.0}},
                "connectorEnd": {"position": {"x": 100.0, "y": 50.0}}
            }
        ]
    });
    let doc = import_fig_node_changes(&changes, "doc").expect("import_fig_node_changes");
    let conn = find(&doc, "Conn");
    let NodeKind::Connector(cp) = &conn.kind else {
        panic!("expected Connector");
    };
    assert_eq!(cp.line, ConnectorLine::Elbowed, "connectorType must not be used as fallback for connectorLineType");
    assert_eq!(conn.fig_metadata.get("connectorType").map(String::as_str), Some("STRAIGHT"));
}

#[test]
fn fig_design_export_vector_cap_geometry_delta() {
    let mut doc = Document::new("cap-delta");
    doc.kind = DocumentKind::Design;
    let page = doc.add_page("Page 1");
    let cid = doc.alloc_id();
    let props = ConnectorProps {
        start: ConnectorEnd::free(Vec2::new(0.0, 0.0)),
        end: ConnectorEnd::free(Vec2::new(100.0, 0.0)),
        line: ConnectorLine::Straight,
        start_cap: ConnectorCap::None,
        end_cap: ConnectorCap::Triangle,
        label: None,
    };
    let mut cn = Node::connector(cid, props);
    cn.name = "Straight Cap".into();
    cn.stroke.weight = 2.0;
    cn.stroke.join = StrokeJoin::Miter;
    doc.insert(page, cn).unwrap();
    sync_connector(&mut doc, cid);

    // Live connector cap length at w = 2.0 is max(4*w, 12) = 12.0.
    let live_cap_len = cap_size(2.0);
    assert_eq!(live_cap_len, 12.0);

    // Export as a design vector, strip the carrier so it imports as a plain vector,
    // and measure the vector arrowhead outline geometry (`5 * w = 10.0`).
    let mut rest = export_rest_json(&doc);
    let vec_json = &mut rest["document"]["children"][0]["children"][0];
    assert_eq!(vec_json["type"], "VECTOR");
    if let Some(obj) = vec_json.as_object_mut() {
        obj.remove("pluginData");
    }
    let as_vec_doc = import_rest_json(&rest.to_string()).expect("import vector");
    let vnode = find(&as_vec_doc, "Straight Cap");
    let NodeKind::Vector { network: Some(net), .. } = &vnode.kind else {
        panic!("expected Vector with network");
    };
    assert_eq!(net.vertices.len(), 2);
    assert_eq!(net.vertices[1].cap, Some(StrokeCap::TriangleFilled));

    let outlined = designos_render::vectorops::outline_stroke(&net.to_paths(), &vnode.stroke, Some(net));
    let cap_path = outlined.last().expect("cap outline path");
    let mut min_x = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut prev = Vec2::ZERO;
    for cmd in &cap_path.cmds {
        match *cmd {
            PathCmd::MoveTo(p) => prev = p,
            PathCmd::LineTo(p) => {
                if (p.x - prev.x).abs() > 0.01 && (p.y - prev.y).abs() > 0.01 {
                    min_x = min_x.min(prev.x).min(p.x);
                    max_x = max_x.max(prev.x).max(p.x);
                }
                prev = p;
            }
            _ => {}
        }
    }
    let vector_cap_len = max_x - min_x;
    assert!((vector_cap_len - 10.0).abs() < 1e-3, "vector cap len = {vector_cap_len}");
    assert!((live_cap_len - vector_cap_len - 2.0).abs() < 1e-3, "recorded cap geometry delta at w=2 is 2.0px (live 12.0 vs vector 10.0)");
}

#[test]
fn fig_import_without_carrier_keeps_a_vector() {
    let mut doc = Document::new("plain-vector");
    doc.kind = DocumentKind::Design;
    let page = doc.add_page("Page 1");
    let vid = doc.alloc_id();
    let mut v0 = Vertex::new(Vec2::new(0.0, 0.0));
    v0.cap = Some(StrokeCap::CircleFilled);
    let mut v1 = Vertex::new(Vec2::new(80.0, 0.0));
    v1.cap = Some(StrokeCap::ArrowEquilateral);
    let net = VectorNetwork { vertices: vec![v0, v1], segments: vec![Segment { a: 0, b: 1, ta: Vec2::ZERO, tb: Vec2::ZERO }], regions: Vec::new() };
    let mut v = Node::new(vid, NodeKind::Vector { network: Some(net), paths: Vec::new() });
    v.name = "Plain Arrow Vector".into();
    v.size = Vec2::new(80.0, 1.0);
    doc.insert(page, v).unwrap();

    let bytes = export_fig(&doc).expect("export_fig");
    let back = import_fig(&bytes).expect("import_fig");
    let restored = find(&back, "Plain Arrow Vector");
    assert!(matches!(restored.kind, NodeKind::Vector { .. }), "vector without studioConnector carrier must remain a Vector, got {:?}", restored.kind);
}

#[test]
fn fig_board_export_still_writes_connector_node() {
    let mut doc = Document::new("board-connectors");
    doc.kind = DocumentKind::Board;
    let page = doc.add_page("Board Page");
    let a = doc.alloc_id();
    doc.insert(page, Node::sticky(a, Rect::new(0.0, 0.0, 200.0, 200.0), "A")).unwrap();
    let b = doc.alloc_id();
    doc.insert(page, Node::sticky(b, Rect::new(400.0, 0.0, 200.0, 200.0), "B")).unwrap();
    let c = doc.alloc_id();
    let mut cn = Node::connector(
        c,
        ConnectorProps {
            start: ConnectorEnd::attached(a, Magnet::Right),
            end: ConnectorEnd::attached(b, Magnet::Left),
            line: ConnectorLine::Elbowed,
            start_cap: ConnectorCap::None,
            end_cap: ConnectorCap::Arrow,
            label: Some(text_props("board link")),
        },
    );
    cn.name = "Board Connector".into();
    doc.insert(page, cn).unwrap();
    sync_connector(&mut doc, c);

    let bytes = export_fig(&doc).expect("export_fig");
    let changes = decoded_changes(&bytes);
    let nc = change_named(&changes, "Board Connector");
    assert_eq!(nc["type"], "CONNECTOR");
    assert!(studio_carrier_value(nc, "studioConnector").is_none());

    let rest = export_rest_json(&doc);
    assert_eq!(rest["editorType"], "figjam");
    let page_kids = rest["document"]["children"][0]["children"].as_array().unwrap();
    let rest_conn = page_kids.iter().find(|n| n["name"] == "Board Connector").unwrap();
    assert_eq!(rest_conn["type"], "CONNECTOR");
}
